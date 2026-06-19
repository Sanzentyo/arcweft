use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Opaque capability granted by the host. Native paths never cross the boundary.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct FileGrantId(String);

impl FileGrantId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FileGrantIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FileGrantIdError::Empty);
        }
        if value.len() > 256 {
            return Err(FileGrantIdError::TooLong);
        }
        Ok(Self(value))
    }

    pub fn new(value: impl Into<String>) -> Option<Self> {
        Self::try_new(value).ok()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for FileGrantId {
    type Error = FileGrantIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<FileGrantId> for String {
    fn from(value: FileGrantId) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FileGrantIdError {
    #[error("file grant id cannot be empty")]
    Empty,
    #[error("file grant id is too long")]
    TooLong,
}

impl fmt::Display for FileGrantId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Slash-separated relative path below a directory grant.
///
/// Absolute paths, empty components, Windows prefixes, `.` and `..` are rejected.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct PortableRelativePath(String);

impl PortableRelativePath {
    pub fn new(value: impl Into<String>) -> Result<Self, RelativePathError> {
        let value = value.into();
        if value.is_empty() {
            return Err(RelativePathError::Empty);
        }
        if value.len() > 4096 {
            return Err(RelativePathError::TooLong);
        }
        if value.starts_with('/')
            || value.starts_with('\\')
            || value.contains('\\')
            || value.contains('\0')
            || value.contains(':')
        {
            return Err(RelativePathError::NotPortable);
        }
        if value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(RelativePathError::InvalidComponent);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

impl TryFrom<String> for PortableRelativePath {
    type Error = RelativePathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<PortableRelativePath> for String {
    fn from(value: PortableRelativePath) -> Self {
        value.0
    }
}

impl fmt::Display for PortableRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RelativePathError {
    #[error("relative path cannot be empty")]
    Empty,
    #[error("relative path is too long")]
    TooLong,
    #[error("relative path is not portable")]
    NotPortable,
    #[error("relative path contains an empty, current, or parent component")]
    InvalidComponent,
}

/// A grant root or one entry below a directory grant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GrantPath {
    pub grant: FileGrantId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative: Option<PortableRelativePath>,
}

impl GrantPath {
    pub fn root(grant: FileGrantId) -> Self {
        Self {
            grant,
            relative: None,
        }
    }

    pub fn child(grant: FileGrantId, relative: PortableRelativePath) -> Self {
        Self {
            grant,
            relative: Some(relative),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantAccess {
    Read,
    Write,
    ReadWrite,
}

impl GrantAccess {
    pub const fn permits_read(self) -> bool {
        matches!(self, Self::Read | Self::ReadWrite)
    }

    pub const fn permits_write(self) -> bool {
        matches!(self, Self::Write | Self::ReadWrite)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantLifetime {
    #[default]
    Session,
    Persistent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KnownDirectory {
    Home,
    Desktop,
    Documents,
    Downloads,
    Music,
    Pictures,
    Videos,
    Config,
    Cache,
    Data,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantOrigin {
    UserSelection,
    KnownDirectory(KnownDirectory),
    Restored,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileEntryKind {
    File,
    Directory,
    Other,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileGrant {
    pub id: FileGrantId,
    pub display_name: String,
    pub access: GrantAccess,
    pub lifetime: GrantLifetime,
    pub origin: GrantOrigin,
    pub entry_kind: FileEntryKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileDialogMode {
    OpenFile,
    OpenFiles,
    SaveFile,
    PickDirectory,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileDialogRequest {
    pub mode: FileDialogMode,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<FileFilter>,
    pub access: GrantAccess,
    #[serde(default)]
    pub lifetime: GrantLifetime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileMetadata {
    pub entry_kind: FileEntryKind,
    pub byte_len: Option<u64>,
    pub modified_unix_millis: Option<i64>,
    pub readonly: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DirectoryEntry {
    pub relative: PortableRelativePath,
    pub display_name: String,
    pub metadata: FileMetadata,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "operation")]
pub enum UserFileRequest {
    ShowDialog(FileDialogRequest),
    GrantKnownDirectory {
        directory: KnownDirectory,
        access: GrantAccess,
        lifetime: GrantLifetime,
    },
    ReadText {
        path: GrantPath,
    },
    ReadBytes {
        path: GrantPath,
    },
    WriteText {
        path: GrantPath,
        text: String,
    },
    WriteBytes {
        path: GrantPath,
        bytes: Vec<u8>,
    },
    Metadata {
        path: GrantPath,
    },
    ListDirectory {
        path: GrantPath,
    },
    Revoke {
        grant: FileGrantId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "result", content = "value")]
pub enum UserFileResponse {
    Grants(Vec<FileGrant>),
    Text(String),
    Bytes(Vec<u8>),
    Metadata(FileMetadata),
    DirectoryEntries(Vec<DirectoryEntry>),
    Applied,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_grant_id_deserialization_rejects_empty_values() {
        let error =
            serde_json::from_str::<FileGrantId>("\"\"").expect_err("empty grant id is invalid");
        assert!(error.to_string().contains("cannot be empty"));
    }
}
