use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::scanner::{DependencyFolder, DependencyKind, ScanSummary};

#[derive(Debug, Clone)]
pub struct CacheCandidate {
    pub path: PathBuf,
    pub manager: CacheManager,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CacheManager {
    Npm,
    Pip,
}

impl CacheManager {
    pub const fn dependency_kind(self) -> DependencyKind {
        match self {
            Self::Npm => DependencyKind::NpmCache,
            Self::Pip => DependencyKind::PipCache,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheSummary {
    pub candidates: Vec<CacheCandidate>,
}

impl CacheSummary {
    pub fn total_size(&self) -> u64 {
        self.candidates
            .iter()
            .map(|candidate| candidate.size_bytes)
            .sum()
    }

    pub fn to_scan_summary(&self) -> ScanSummary {
        let cache_age = Duration::from_secs(365 * 86_400);
        let project_modified = SystemTime::now()
            .checked_sub(cache_age)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        ScanSummary {
            roots: self
                .candidates
                .iter()
                .map(|candidate| candidate.path.clone())
                .collect(),
            folders: self
                .candidates
                .iter()
                .map(|candidate| DependencyFolder {
                    path: candidate.path.clone(),
                    project_path: candidate.path.clone(),
                    kind: candidate.manager.dependency_kind(),
                    size_bytes: candidate.size_bytes,
                    project_modified,
                    age: cache_age,
                })
                .collect(),
        }
    }
}

pub fn scan_caches() -> CacheSummary {
    let mut candidates = Vec::new();
    for (manager, path) in candidate_paths() {
        if path.is_dir() {
            candidates.push(CacheCandidate {
                manager,
                size_bytes: dir_size(&path),
                path,
            });
        }
    }
    candidates.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then(a.path.cmp(&b.path)));
    CacheSummary { candidates }
}

fn candidate_paths() -> Vec<(CacheManager, PathBuf)> {
    let mut paths = Vec::new();
    if let Ok(npm_cache) = std::env::var("npm_config_cache") {
        paths.push((CacheManager::Npm, PathBuf::from(npm_cache)));
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        paths.push((CacheManager::Npm, home.join(".npm")));
        paths.push((CacheManager::Pip, home.join(".cache/pip")));
        paths.push((CacheManager::Pip, home.join("Library/Caches/pip")));
    }
    paths.sort();
    paths.dedup();
    paths
}

fn dir_size(path: &std::path::Path) -> u64 {
    walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{CacheCandidate, CacheManager, CacheSummary};

    #[test]
    fn cache_summary_maps_to_scan_summary_with_cache_kind() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("npm");
        fs::create_dir(&cache).unwrap();
        let summary = CacheSummary {
            candidates: vec![CacheCandidate {
                path: cache,
                manager: CacheManager::Npm,
                size_bytes: 10,
            }],
        };

        let scan = summary.to_scan_summary();

        assert_eq!(scan.folders[0].kind.label(), "npm-cache");
        assert_eq!(scan.total_size(), 10);
    }
}
