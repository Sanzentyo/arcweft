use super::temp::TempDir;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct TestWorkspace {
    root: TempDir,
}

impl TestWorkspace {
    pub fn new(label: &str) -> io::Result<Self> {
        TempDir::new(label).map(|root| Self { root })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn write(&self, relative: impl AsRef<Path>, content: &str) -> io::Result<PathBuf> {
        let path = self.root().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(path)
    }
}
