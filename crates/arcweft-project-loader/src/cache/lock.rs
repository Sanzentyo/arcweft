use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// Best-effort filesystem lock created with `create_new`.
#[derive(Debug)]
pub struct CacheLock {
    path: PathBuf,
    _file: File,
}

/// Cache lock acquisition or release failure.
#[derive(Debug, Error)]
pub enum CacheLockError {
    #[error("cache lock `{path}` is already held")]
    AlreadyHeld { path: PathBuf },
    #[error("failed to create cache lock directory `{path}`: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create cache lock `{path}`: {source}")]
    Create {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write cache lock `{path}`: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl CacheLock {
    /// Acquires a lock file and removes it on drop.
    pub fn acquire(path: impl AsRef<Path>, label: &str) -> Result<Self, CacheLockError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| CacheLockError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::AlreadyExists {
                    CacheLockError::AlreadyHeld { path: path.clone() }
                } else {
                    CacheLockError::Create {
                        path: path.clone(),
                        source,
                    }
                }
            })?;
        file.write_all(label.as_bytes())
            .map_err(|source| CacheLockError::Write {
                path: path.clone(),
                source,
            })?;
        Ok(Self { path, _file: file })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::{CacheLock, CacheLockError};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn lock_rejects_second_holder() {
        let path = std::env::temp_dir().join(format!(
            "arcweft-cache-lock-{}.lock",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let first = CacheLock::acquire(&path, "first").expect("first lock");
        assert!(matches!(
            CacheLock::acquire(&path, "second"),
            Err(CacheLockError::AlreadyHeld { .. })
        ));
        drop(first);
        CacheLock::acquire(&path, "third").expect("lock released");
    }
}
