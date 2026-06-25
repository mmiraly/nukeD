use std::time::{Duration, SystemTime};

use humansize::{BINARY, format_size};

use crate::age::AgeThreshold;
use crate::fuzzy::matching_indices;
use crate::scanner::{DependencyKind, ScanSummary};

pub fn print_report(scan: &ScanSummary, threshold: AgeThreshold, filter: Option<&str>) {
    let indices = matching_indices(&scan.folders, filter.unwrap_or_default());
    println!("nukeD scan");
    println!("roots: {}", scan.roots.len());
    println!("dependency folders: {}", scan.folders.len());
    if let Some(filter) = filter.filter(|filter| !filter.trim().is_empty()) {
        println!("filter: {filter}");
        println!("matching folders: {}", indices.len());
    }
    println!("total dependency size: {}", bytes(scan.total_size()));
    println!(
        "reclaimable at {threshold}: {}",
        bytes(total_for_indices(scan, &indices, threshold))
    );
    println!();

    println!("age presets");
    for preset in AgeThreshold::presets() {
        let total = scan.total_for(preset);
        println!(
            "  {:>4}  {:>10}  {}",
            preset,
            bytes(total),
            bar(total, scan.total_size(), 24)
        );
    }
    println!();

    println!("by ecosystem");
    for kind in [DependencyKind::Node, DependencyKind::Python] {
        let total: u64 = scan
            .folders
            .iter()
            .filter(|folder| folder.kind == kind)
            .map(|folder| folder.size_bytes)
            .sum();
        println!("  {:>6}  {}", kind.label(), bytes(total));
    }
    println!();

    for idx in indices {
        let folder = &scan.folders[idx];
        if !folder.is_older_than(threshold) {
            continue;
        }
        println!(
            "{}  {:>10}  {:>6} old  touched {}  {}",
            folder.kind.label(),
            bytes(folder.size_bytes),
            age_label(folder.age),
            modified_label(folder.project_modified),
            folder.path.display()
        );
    }
}

fn total_for_indices(scan: &ScanSummary, indices: &[usize], threshold: AgeThreshold) -> u64 {
    indices
        .iter()
        .map(|idx| &scan.folders[*idx])
        .filter(|folder| folder.is_older_than(threshold))
        .map(|folder| folder.size_bytes)
        .sum()
}

pub fn bytes(value: u64) -> String {
    format_size(value, BINARY)
}

pub fn age_label(age: Duration) -> String {
    let days = age.as_secs() / 86_400;
    if days >= 365 {
        format!("{}y", days / 365)
    } else {
        format!("{days}d")
    }
}

pub fn modified_label(time: SystemTime) -> String {
    match SystemTime::now().duration_since(time) {
        Ok(age) => format!("{} ago", age_label(age)),
        Err(_) => "in future".to_string(),
    }
}

pub fn bar(value: u64, max: u64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if max == 0 {
        return " ".repeat(width);
    }
    let filled = ((value as f64 / max as f64) * width as f64).round() as usize;
    format!(
        "{}{}",
        "█".repeat(filled.min(width)),
        "░".repeat(width.saturating_sub(filled))
    )
}
