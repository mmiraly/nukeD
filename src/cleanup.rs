use std::path::Path;

use anyhow::{Context, Result};

pub trait Cleaner {
    fn trash(&self, path: &Path) -> Result<()>;
}

pub struct TrashCleaner;

impl Cleaner for TrashCleaner {
    fn trash(&self, path: &Path) -> Result<()> {
        trash::delete(path).with_context(|| format!("failed to move {} to trash", path.display()))
    }
}

#[cfg(test)]
pub mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    use anyhow::Result;

    use super::Cleaner;

    #[allow(dead_code)]
    #[derive(Default)]
    pub struct RecordingCleaner {
        pub paths: RefCell<Vec<PathBuf>>,
    }

    impl Cleaner for RecordingCleaner {
        fn trash(&self, path: &Path) -> Result<()> {
            self.paths.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
    }
}
