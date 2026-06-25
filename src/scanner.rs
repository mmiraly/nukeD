use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use walkdir::{DirEntry, WalkDir};

use crate::age::AgeThreshold;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    Node,
    Python,
}

impl DependencyKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Python => "python",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DependencyFolder {
    pub path: PathBuf,
    pub project_path: PathBuf,
    pub kind: DependencyKind,
    pub size_bytes: u64,
    pub project_modified: SystemTime,
    pub age: Duration,
}

impl DependencyFolder {
    pub fn is_older_than(&self, threshold: AgeThreshold) -> bool {
        self.age >= threshold.as_duration()
    }
}

#[derive(Debug, Clone)]
pub struct ScanSummary {
    pub roots: Vec<PathBuf>,
    pub folders: Vec<DependencyFolder>,
}

impl ScanSummary {
    pub fn selected_for(&self, threshold: AgeThreshold) -> Vec<&DependencyFolder> {
        self.folders
            .iter()
            .filter(|folder| folder.is_older_than(threshold))
            .collect()
    }

    pub fn total_size(&self) -> u64 {
        self.folders.iter().map(|folder| folder.size_bytes).sum()
    }

    pub fn total_for(&self, threshold: AgeThreshold) -> u64 {
        self.selected_for(threshold)
            .iter()
            .map(|folder| folder.size_bytes)
            .sum()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScanOptions {
    pub now: SystemTime,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            now: SystemTime::now(),
        }
    }
}

pub fn scan_roots(roots: &[PathBuf], options: ScanOptions) -> Result<ScanSummary> {
    let mut seen = HashSet::new();
    let mut folders = Vec::new();

    for root in roots {
        let root = root
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", root.display()))?;
        scan_root(&root, options, &mut seen, &mut folders)?;
    }

    folders.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then(a.path.cmp(&b.path)));

    Ok(ScanSummary {
        roots: roots.to_vec(),
        folders,
    })
}

fn scan_root(
    root: &Path,
    options: ScanOptions,
    seen: &mut HashSet<PathBuf>,
    folders: &mut Vec<DependencyFolder>,
) -> Result<()> {
    let mut walker = WalkDir::new(root).follow_links(false).into_iter();

    while let Some(entry) = walker.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if !entry.file_type().is_dir() {
            continue;
        }

        if is_hidden_noise(&entry) && entry.path() != root {
            walker.skip_current_dir();
            continue;
        }

        let Some(kind) = dependency_kind(entry.path()) else {
            continue;
        };

        walker.skip_current_dir();

        let dep_path = entry.path().to_path_buf();
        let canonical_dep = match dep_path.canonicalize() {
            Ok(path) => path,
            Err(_) => continue,
        };

        if !seen.insert(canonical_dep) {
            continue;
        }

        let Some(project_path) = dep_path.parent().map(Path::to_path_buf) else {
            continue;
        };

        if !is_project_for_kind(&project_path, kind) {
            continue;
        }

        let size_bytes = dir_size(&dep_path);
        let project_modified = latest_project_mtime(&project_path, &dep_path)
            .unwrap_or_else(|| fs_mtime(&project_path).unwrap_or(SystemTime::UNIX_EPOCH));
        let age = options
            .now
            .duration_since(project_modified)
            .unwrap_or_else(|_| Duration::from_secs(0));

        folders.push(DependencyFolder {
            path: dep_path,
            project_path,
            kind,
            size_bytes,
            project_modified,
            age,
        });
    }

    Ok(())
}

fn dependency_kind(path: &Path) -> Option<DependencyKind> {
    let name = path.file_name()?.to_string_lossy();
    if name == "node_modules" {
        return Some(DependencyKind::Node);
    }

    if matches!(
        name.as_ref(),
        ".venv" | "venv" | ".env" | "env" | "virtualenv" | ".virtualenv"
    ) {
        return Some(DependencyKind::Python);
    }

    None
}

fn is_project_for_kind(project_path: &Path, kind: DependencyKind) -> bool {
    match kind {
        DependencyKind::Node => {
            has_file(project_path, "package.json")
                || has_file(project_path, "package-lock.json")
                || has_file(project_path, "pnpm-lock.yaml")
                || has_file(project_path, "yarn.lock")
        }
        DependencyKind::Python => {
            has_file(project_path, "pyproject.toml")
                || has_file(project_path, "setup.py")
                || has_file(project_path, "setup.cfg")
                || has_file(project_path, "Pipfile")
                || has_file(project_path, "poetry.lock")
                || has_requirement_signal(project_path)
        }
    }
}

fn has_requirement_signal(project_path: &Path) -> bool {
    const REQUIREMENT_FILES: &[&str] = &[
        "requirements.txt",
        "requirements-dev.txt",
        "requirements-test.txt",
        "req.txt",
        "reqs.txt",
        "r.txt",
        "deps.txt",
        "dependencies.txt",
    ];

    REQUIREMENT_FILES
        .iter()
        .any(|file| looks_like_requirements_file(&project_path.join(file)))
}

fn looks_like_requirements_file(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };

    content.lines().take(60).any(|line| {
        let line = line.trim();
        !line.is_empty()
            && !line.starts_with('#')
            && (line.contains("==")
                || line.contains(">=")
                || line.contains("<=")
                || line.starts_with("-r ")
                || line.starts_with("--index-url")
                || line.starts_with("git+"))
    })
}

fn has_file(dir: &Path, file: &str) -> bool {
    dir.join(file).is_file()
}

fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}

fn latest_project_mtime(project_path: &Path, dependency_path: &Path) -> Option<SystemTime> {
    let mut latest = fs_mtime(project_path);
    let mut walker = WalkDir::new(project_path).follow_links(false).into_iter();

    while let Some(entry) = walker.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if entry.path() == dependency_path {
            walker.skip_current_dir();
            continue;
        }

        if entry.file_type().is_dir()
            && entry.path() != project_path
            && dependency_kind(entry.path()).is_some()
        {
            walker.skip_current_dir();
            continue;
        }

        if let Some(mtime) = fs_mtime(entry.path()) {
            latest = Some(latest.map_or(mtime, |current| current.max(mtime)));
        }
    }

    latest
}

fn fs_mtime(path: &Path) -> Option<SystemTime> {
    fs::symlink_metadata(path).ok()?.modified().ok()
}

fn is_hidden_noise(entry: &DirEntry) -> bool {
    matches!(
        entry.file_name().to_string_lossy().as_ref(),
        ".git" | ".hg" | ".svn" | "target" | ".cache"
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime};

    use tempfile::TempDir;

    use super::{DependencyKind, ScanOptions, scan_roots};
    use crate::age::AgeThreshold;

    #[test]
    fn detects_node_modules_only_with_project_signal() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("app");
        let stray = tmp.path().join("stray");
        fs::create_dir_all(project.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(stray.join("node_modules/pkg")).unwrap();
        fs::write(project.join("package.json"), "{}").unwrap();
        fs::write(project.join("node_modules/pkg/index.js"), "x").unwrap();
        fs::write(stray.join("node_modules/pkg/index.js"), "x").unwrap();

        let scan = scan_roots(&[tmp.path().to_path_buf()], ScanOptions::default()).unwrap();

        assert_eq!(scan.folders.len(), 1);
        assert_eq!(scan.folders[0].kind, DependencyKind::Node);
        assert_eq!(
            scan.folders[0].project_path,
            project.canonicalize().unwrap()
        );
    }

    #[test]
    fn detects_python_venv_with_requirement_alias_content() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("py");
        fs::create_dir_all(project.join(".venv/lib")).unwrap();
        fs::write(project.join("deps.txt"), "requests==2.0\n").unwrap();
        fs::write(project.join(".venv/lib/site.py"), "x").unwrap();

        let scan = scan_roots(&[tmp.path().to_path_buf()], ScanOptions::default()).unwrap();

        assert_eq!(scan.folders.len(), 1);
        assert_eq!(scan.folders[0].kind, DependencyKind::Python);
    }

    #[test]
    fn ignores_generic_requirement_alias_without_requirement_content() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("notes");
        fs::create_dir_all(project.join("venv/lib")).unwrap();
        fs::write(project.join("deps.txt"), "remember to buy batteries\n").unwrap();

        let scan = scan_roots(&[tmp.path().to_path_buf()], ScanOptions::default()).unwrap();

        assert!(scan.folders.is_empty());
    }

    #[test]
    fn project_age_ignores_dependency_folder_files() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("app");
        fs::create_dir_all(project.join("node_modules/pkg")).unwrap();
        fs::write(project.join("package.json"), "{}").unwrap();
        fs::write(project.join("node_modules/pkg/new.js"), "x").unwrap();

        let now = SystemTime::now() + Duration::from_secs(31 * 24 * 60 * 60);
        let scan = scan_roots(&[tmp.path().to_path_buf()], ScanOptions { now }).unwrap();

        assert!(scan.folders[0].is_older_than(AgeThreshold::days(30)));
    }
}
