use super::{ReleaseRemoteBackendError, ReleaseRemoteObjectKey, ReleaseRemotePublicationBackend};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Remote-like filesystem backend used as the first deterministic publication
/// backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseObjectDirectoryBackend {
    root: PathBuf,
}

impl ReleaseObjectDirectoryBackend {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn committed_path(&self, key: &ReleaseRemoteObjectKey) -> PathBuf {
        self.path_for(key)
    }

    fn path_for(&self, key: &ReleaseRemoteObjectKey) -> PathBuf {
        self.root.join(key.as_path())
    }
}

impl ReleaseRemotePublicationBackend for ReleaseObjectDirectoryBackend {
    fn backend_id(&self) -> &'static str {
        "object_directory"
    }

    fn put_object(
        &mut self,
        key: &ReleaseRemoteObjectKey,
        bytes: &[u8],
    ) -> Result<(), ReleaseRemoteBackendError> {
        let path = self.path_for(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| io_backend_error(parent, &source))?;
        }
        let tmp_path = temporary_object_path(&path);
        fs::write(&tmp_path, bytes).map_err(|source| io_backend_error(&tmp_path, &source))?;
        fs::rename(&tmp_path, &path).map_err(|source| io_backend_error(&path, &source))
    }

    fn read_object(
        &self,
        key: &ReleaseRemoteObjectKey,
    ) -> Result<Vec<u8>, ReleaseRemoteBackendError> {
        let path = self.path_for(key);
        fs::read(&path).map_err(|source| io_backend_error(&path, &source))
    }

    fn copy_object(
        &mut self,
        from: &ReleaseRemoteObjectKey,
        to: &ReleaseRemoteObjectKey,
    ) -> Result<(), ReleaseRemoteBackendError> {
        let from_path = self.path_for(from);
        let to_path = self.path_for(to);
        if to_path.exists() {
            return Err(ReleaseRemoteBackendError::non_retryable(format!(
                "destination object already exists: {}",
                to_path.display()
            )));
        }
        let bytes = fs::read(&from_path).map_err(|source| io_backend_error(&from_path, &source))?;
        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent).map_err(|source| io_backend_error(parent, &source))?;
        }
        let tmp_path = temporary_object_path(&to_path);
        fs::write(&tmp_path, bytes).map_err(|source| io_backend_error(&tmp_path, &source))?;
        fs::rename(&tmp_path, &to_path).map_err(|source| io_backend_error(&to_path, &source))
    }

    fn delete_object(
        &mut self,
        key: &ReleaseRemoteObjectKey,
    ) -> Result<(), ReleaseRemoteBackendError> {
        let path = self.path_for(key);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == ErrorKind::NotFound => Ok(()),
            Err(source) => Err(io_backend_error(&path, &source)),
        }
    }

    fn object_exists(
        &self,
        key: &ReleaseRemoteObjectKey,
    ) -> Result<bool, ReleaseRemoteBackendError> {
        let path = self.path_for(key);
        match fs::metadata(&path) {
            Ok(metadata) => Ok(metadata.is_file()),
            Err(source) if source.kind() == ErrorKind::NotFound => Ok(false),
            Err(source) => Err(io_backend_error(&path, &source)),
        }
    }
}

fn io_backend_error(path: &Path, source: &std::io::Error) -> ReleaseRemoteBackendError {
    let message = format!("{}: {source}", path.display());
    match source.kind() {
        ErrorKind::AlreadyExists
        | ErrorKind::InvalidInput
        | ErrorKind::NotFound
        | ErrorKind::PermissionDenied => ReleaseRemoteBackendError::non_retryable(message),
        _ => ReleaseRemoteBackendError::retryable(message),
    }
}

fn temporary_object_path(path: &Path) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("object");
    path.with_file_name(format!("{file_name}.{suffix}.tmp"))
}
