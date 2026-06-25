use std::time::{Duration, SystemTime};

use humansize::{BINARY, format_size};

use crate::age::AgeThreshold;
use crate::fuzzy::matching_indices;
use crate::scanner::{DependencyKind, ScanSummary};

pub fn print_report(scan: &ScanSummary, threshold: AgeThreshold, filter: Option<&str>) {
    let indices = matching_indices(&scan.folders, filter.unwrap_or_default());
    let filtered = filter.is_some_and(|filter| !filter.trim().is_empty());
    let eligible_count = indices
        .iter()
        .filter(|idx| scan.folders[**idx].is_older_than(threshold))
        .count();
    let matching_size = total_size_for_indices(scan, &indices);
    println!("nukeD scan");
    println!("roots: {}", scan.roots.len());
    println!("detected folders: {}", scan.folders.len());
    if let Some(filter) = filter.filter(|filter| !filter.trim().is_empty()) {
        println!("filter: {filter}");
        println!("matching folders: {}", indices.len());
        println!("matching dependency size: {}", bytes(matching_size));
    }
    println!("eligible at {threshold}: {eligible_count}");
    println!("total dependency size: {}", bytes(scan.total_size()));
    println!(
        "eligible reclaimable: {}",
        bytes(total_for_indices(scan, &indices, threshold))
    );
    println!();

    println!("age presets     reclaimable    saved");
    let chart_max = if filtered {
        matching_size
    } else {
        scan.total_size()
    };
    for preset in AgeThreshold::presets() {
        let total = if filtered {
            total_for_indices(scan, &indices, preset)
        } else {
            scan.total_for(preset)
        };
        println!(
            "  {:>4}  {:>12}  {:>5}  {}",
            preset,
            bytes(total),
            percent(total, chart_max),
            bar(total, chart_max, 24)
        );
    }
    println!();

    println!("by ecosystem");
    for kind in [DependencyKind::Node, DependencyKind::Python] {
        let total: u64 = scan
            .folders
            .iter()
            .enumerate()
            .filter(|(idx, _)| !filtered || indices.contains(idx))
            .map(|(_, folder)| folder)
            .filter(|folder| folder.kind == kind)
            .map(|folder| folder.size_bytes)
            .sum();
        println!("  {:>6}  {}", kind.label(), bytes(total));
    }
    println!();

    println!(
        "{:<7} {:>12} {:>8} {:<6} {:>10}  path",
        "kind", "size", "age", "status", "touched"
    );
    for idx in indices {
        let folder = &scan.folders[idx];
        println!(
            "{:<7} {:>12} {:>8} {:<6} {:>10}  {}",
            folder.kind.label(),
            bytes(folder.size_bytes),
            age_label(folder.age),
            status_label(folder.is_older_than(threshold)),
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

fn total_size_for_indices(scan: &ScanSummary, indices: &[usize]) -> u64 {
    indices
        .iter()
        .map(|idx| scan.folders[*idx].size_bytes)
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

pub fn status_label(is_eligible: bool) -> &'static str {
    if is_eligible { "ready" } else { "newer" }
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

pub fn percent(value: u64, max: u64) -> String {
    if max == 0 {
        return "0%".to_string();
    }
    format!("{:.0}%", (value as f64 / max as f64) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::{bar, percent};

    #[test]
    fn bars_keep_fixed_width() {
        assert_eq!(bar(0, 100, 12).chars().count(), 12);
        assert_eq!(bar(50, 100, 12).chars().count(), 12);
        assert_eq!(bar(100, 100, 12).chars().count(), 12);
    }

    #[test]
    fn percent_handles_empty_totals() {
        assert_eq!(percent(0, 0), "0%");
        assert_eq!(percent(25, 100), "25%");
    }
}
