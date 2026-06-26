use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::age::AgeThreshold;
use crate::cache::CacheSummary;
use crate::display::status_label;
use crate::fuzzy::matching_indices;
use crate::scanner::{DependencyFolder, ScanSummary};

#[derive(Debug, Serialize)]
pub struct ProjectReport {
    mode: &'static str,
    roots: Vec<String>,
    filter: Option<String>,
    threshold: String,
    totals: ReportTotals,
    folders: Vec<FolderReport>,
}

#[derive(Debug, Serialize)]
pub struct CacheReport {
    mode: &'static str,
    totals: ReportTotals,
    candidates: Vec<FolderReport>,
}

#[derive(Debug, Serialize)]
struct ReportTotals {
    detected_count: usize,
    total_size_bytes: u64,
    eligible_count: usize,
    eligible_size_bytes: u64,
}

#[derive(Debug, Serialize)]
struct FolderReport {
    ecosystem: String,
    path: String,
    project_path: String,
    size_bytes: u64,
    age_seconds: u64,
    status: String,
}

pub fn project_report(
    scan: &ScanSummary,
    threshold: AgeThreshold,
    filter: Option<&str>,
) -> ProjectReport {
    let indices = matching_indices(&scan.folders, filter.unwrap_or_default());
    let folders: Vec<&DependencyFolder> = indices.iter().map(|idx| &scan.folders[*idx]).collect();
    let eligible: Vec<&&DependencyFolder> = folders
        .iter()
        .filter(|folder| folder.is_older_than(threshold))
        .collect();

    ProjectReport {
        mode: "project",
        roots: scan
            .roots
            .iter()
            .map(|root| root.display().to_string())
            .collect(),
        filter: filter
            .filter(|filter| !filter.trim().is_empty())
            .map(str::to_string),
        threshold: threshold.to_string(),
        totals: ReportTotals {
            detected_count: folders.len(),
            total_size_bytes: folders.iter().map(|folder| folder.size_bytes).sum(),
            eligible_count: eligible.len(),
            eligible_size_bytes: eligible.iter().map(|folder| folder.size_bytes).sum(),
        },
        folders: folders
            .into_iter()
            .map(|folder| folder_report(folder, threshold))
            .collect(),
    }
}

pub fn cache_report(summary: &CacheSummary) -> CacheReport {
    let scan = summary.to_scan_summary();
    let folders = scan
        .folders
        .iter()
        .map(|folder| folder_report(folder, AgeThreshold::days(1)))
        .collect();

    CacheReport {
        mode: "cache",
        totals: ReportTotals {
            detected_count: summary.candidates.len(),
            total_size_bytes: summary.total_size(),
            eligible_count: summary.candidates.len(),
            eligible_size_bytes: summary.total_size(),
        },
        candidates: folders,
    }
}

pub fn print_json<T: Serialize>(report: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(report)?);
    Ok(())
}

pub fn write_json<T: Serialize>(path: &Path, report: &T) -> Result<()> {
    let content = serde_json::to_string_pretty(report)?;
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}

fn folder_report(folder: &DependencyFolder, threshold: AgeThreshold) -> FolderReport {
    FolderReport {
        ecosystem: folder.kind.label().to_string(),
        path: folder.path.display().to_string(),
        project_path: folder.project_path.display().to_string(),
        size_bytes: folder.size_bytes,
        age_seconds: folder.age.as_secs(),
        status: status_label(folder.is_older_than(threshold)).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use super::project_report;
    use crate::age::AgeThreshold;
    use crate::scanner::{DependencyFolder, DependencyKind, ScanSummary};

    #[test]
    fn project_report_has_expected_shape() {
        let scan = ScanSummary {
            roots: vec![PathBuf::from("/tmp")],
            folders: vec![DependencyFolder {
                path: PathBuf::from("/tmp/app/node_modules"),
                project_path: PathBuf::from("/tmp/app"),
                kind: DependencyKind::Node,
                size_bytes: 100,
                project_modified: SystemTime::UNIX_EPOCH,
                age: Duration::from_secs(10 * 86_400),
            }],
        };

        let report = project_report(&scan, AgeThreshold::days(7), None);
        let json = serde_json::to_value(report).unwrap();

        assert_eq!(json["mode"], "project");
        assert_eq!(json["totals"]["detected_count"], 1);
        assert_eq!(json["folders"][0]["ecosystem"], "node");
    }
}
