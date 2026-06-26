use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::{DirEntry, WalkDir};

use crate::age::AgeThreshold;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    Node,
    Python,
    NpmCache,
    PipCache,
}

impl DependencyKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Python => "python",
            Self::NpmCache => "npm-cache",
            Self::PipCache => "pip-cache",
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
    pub use_nukedignore: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            now: SystemTime::now(),
            use_nukedignore: true,
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
    let ignore = if options.use_nukedignore {
        IgnoreRules::load(root)?
    } else {
        IgnoreRules::empty()
    };
    let mut walker = WalkDir::new(root).follow_links(false).into_iter();

    while let Some(entry) = walker.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if !entry.file_type().is_dir() {
            continue;
        }

        if entry.path() != root && ignore.is_ignored(root, entry.path()) {
            walker.skip_current_dir();
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
                || (has_requirement_signal(project_path) && has_python_env_signal(project_path))
        }
        DependencyKind::NpmCache | DependencyKind::PipCache => true,
    }
}

fn has_python_env_signal(project_path: &Path) -> bool {
    [".venv", "venv", ".env", "env", "virtualenv", ".virtualenv"]
        .iter()
        .any(|name| project_path.join(name).join("pyvenv.cfg").is_file())
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

    content
        .lines()
        .take(60)
        .any(|line| looks_like_requirement_line(line.trim()))
}

fn looks_like_requirement_line(line: &str) -> bool {
    if line.is_empty() || line.starts_with('#') {
        return false;
    }

    if line.contains("==")
        || line.contains(">=")
        || line.contains("<=")
        || line.contains("~=")
        || line.contains("!=")
        || line.starts_with("-r ")
        || line.starts_with("--index-url")
        || line.starts_with("--extra-index-url")
        || line.starts_with("git+")
    {
        return true;
    }

    let package = line
        .split_once(';')
        .map_or(line, |(package, _)| package)
        .split_once('[')
        .map_or(line, |(package, _)| package)
        .trim();

    !package.is_empty()
        && package.len() <= 80
        && package
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        && package.chars().any(|ch| ch.is_ascii_alphabetic())
        && !package.contains("..")
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
    let mut latest: Option<SystemTime> = None;
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

        if entry.path() != project_path {
            if entry.file_type().is_dir() && is_project_activity_noise_dir(entry.path()) {
                walker.skip_current_dir();
                continue;
            }

            if is_project_activity_noise_file(entry.path()) {
                continue;
            }
        }

        if entry.file_type().is_dir() {
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

fn is_project_activity_noise_dir(path: &Path) -> bool {
    dependency_kind(path).is_some()
        || matches!(
            path.file_name()
                .map(|name| name.to_string_lossy())
                .as_deref(),
            Some(
                ".git"
                    | ".hg"
                    | ".svn"
                    | "target"
                    | "dist"
                    | "build"
                    | ".cache"
                    | ".next"
                    | ".nuxt"
                    | ".vite"
                    | ".turbo"
                    | ".parcel-cache"
                    | ".pytest_cache"
                    | "__pycache__"
                    | ".mypy_cache"
                    | ".ruff_cache"
                    | ".tox"
            )
        )
}

fn is_project_activity_noise_file(path: &Path) -> bool {
    matches!(
        path.file_name()
            .map(|name| name.to_string_lossy())
            .as_deref(),
        Some(".DS_Store" | "Thumbs.db" | ".eslintcache")
    )
}

fn is_hidden_noise(entry: &DirEntry) -> bool {
    matches!(
        entry.file_name().to_string_lossy().as_ref(),
        ".git" | ".hg" | ".svn" | "target" | ".cache"
    )
}

struct IgnoreRules {
    set: GlobSet,
}

impl IgnoreRules {
    fn empty() -> Self {
        Self {
            set: GlobSetBuilder::new()
                .build()
                .expect("empty glob set is valid"),
        }
    }

    fn load(root: &Path) -> Result<Self> {
        let path = root.join(".nukedignore");
        if !path.is_file() {
            return Ok(Self::empty());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut builder = GlobSetBuilder::new();
        for line in content.lines() {
            let pattern = line.trim();
            if pattern.is_empty() || pattern.starts_with('#') {
                continue;
            }
            let pattern = pattern.trim_start_matches('/').trim_end_matches('/');
            if pattern.is_empty() {
                continue;
            }
            builder.add(Glob::new(pattern).with_context(|| {
                format!(
                    "invalid .nukedignore pattern {pattern:?} in {}",
                    path.display()
                )
            })?);
            if !pattern.contains('/') {
                builder.add(Glob::new(&format!("**/{pattern}")).with_context(|| {
                    format!(
                        "invalid .nukedignore pattern {pattern:?} in {}",
                        path.display()
                    )
                })?);
            }
        }

        Ok(Self {
            set: builder.build()?,
        })
    }

    fn is_ignored(&self, root: &Path, path: &Path) -> bool {
        path.strip_prefix(root)
            .ok()
            .is_some_and(|relative| self.set.is_match(relative))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime};

    use filetime::{FileTime, set_file_mtime};
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
        fs::write(project.join(".venv/pyvenv.cfg"), "home = /usr/bin\n").unwrap();
        fs::write(project.join("deps.txt"), "requests==2.0\n").unwrap();
        fs::write(project.join(".venv/lib/site.py"), "x").unwrap();

        let scan = scan_roots(&[tmp.path().to_path_buf()], ScanOptions::default()).unwrap();

        assert_eq!(scan.folders.len(), 1);
        assert_eq!(scan.folders[0].kind, DependencyKind::Python);
    }

    #[test]
    fn detects_python_env_with_bare_requirement_lines() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("bot");
        fs::create_dir_all(project.join("env/lib")).unwrap();
        fs::write(project.join("env/pyvenv.cfg"), "home = /usr/bin\n").unwrap();
        fs::write(
            project.join("requirements.txt"),
            "discord.py\npython-dotenv\naiohttp\n",
        )
        .unwrap();
        fs::write(project.join("env/lib/site.py"), "x").unwrap();

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
        let scan = scan_roots(
            &[tmp.path().to_path_buf()],
            ScanOptions {
                now,
                ..ScanOptions::default()
            },
        )
        .unwrap();

        assert!(scan.folders[0].is_older_than(AgeThreshold::days(30)));
    }

    #[test]
    fn project_age_ignores_os_noise_and_generated_dirs() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("app");
        fs::create_dir_all(project.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(project.join("dist")).unwrap();
        fs::write(project.join("package.json"), "{}").unwrap();
        fs::write(project.join("src.js"), "source").unwrap();
        fs::write(project.join("node_modules/pkg/index.js"), "x").unwrap();
        fs::write(project.join(".DS_Store"), "fresh noise").unwrap();
        fs::write(project.join("dist/bundle.js"), "fresh build").unwrap();

        let old = FileTime::from_unix_time(1_700_000_000, 0);
        let fresh = FileTime::from_unix_time(1_800_000_000, 0);
        set_file_mtime(project.join("package.json"), old).unwrap();
        set_file_mtime(project.join("src.js"), old).unwrap();
        set_file_mtime(project.join(".DS_Store"), fresh).unwrap();
        set_file_mtime(project.join("dist/bundle.js"), fresh).unwrap();

        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + 31 * 86_400);
        let scan = scan_roots(
            &[tmp.path().to_path_buf()],
            ScanOptions {
                now,
                ..ScanOptions::default()
            },
        )
        .unwrap();

        assert!(scan.folders[0].is_older_than(AgeThreshold::days(30)));
    }

    #[test]
    fn nukedignore_skips_dependency_discovery() {
        let tmp = TempDir::new().unwrap();
        let ignored = tmp.path().join("ignored");
        let kept = tmp.path().join("kept");
        fs::create_dir_all(ignored.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(kept.join("node_modules/pkg")).unwrap();
        fs::write(ignored.join("package.json"), "{}").unwrap();
        fs::write(kept.join("package.json"), "{}").unwrap();
        fs::write(tmp.path().join(".nukedignore"), "ignored/\n").unwrap();

        let scan = scan_roots(&[tmp.path().to_path_buf()], ScanOptions::default()).unwrap();

        assert_eq!(scan.folders.len(), 1);
        assert_eq!(scan.folders[0].project_path, kept.canonicalize().unwrap());
    }

    #[test]
    fn nukedignore_supports_nested_relative_patterns() {
        let tmp = TempDir::new().unwrap();
        let ignored = tmp.path().join("apps/api");
        let kept = tmp.path().join("apps/web");
        fs::create_dir_all(ignored.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(kept.join("node_modules/pkg")).unwrap();
        fs::write(ignored.join("package.json"), "{}").unwrap();
        fs::write(kept.join("package.json"), "{}").unwrap();
        fs::write(tmp.path().join(".nukedignore"), "apps/api\n").unwrap();

        let scan = scan_roots(&[tmp.path().to_path_buf()], ScanOptions::default()).unwrap();

        assert_eq!(scan.folders.len(), 1);
        assert_eq!(scan.folders[0].project_path, kept.canonicalize().unwrap());
    }
}
