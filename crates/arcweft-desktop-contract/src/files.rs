use crate::PermissionKind;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

const LOWER_HEX: &[u8; 16] = b"0123456789abcdef";

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

    pub fn from_entropy(lifetime: GrantLifetime, entropy: &[u8]) -> Result<Self, FileGrantIdError> {
        let expected = lifetime.entropy_bytes();
        if entropy.len() != expected {
            return Err(FileGrantIdError::InvalidEntropyLength {
                expected,
                actual: entropy.len(),
            });
        }

        let mut value = String::with_capacity(lifetime.grant_id_prefix().len() + expected * 2);
        value.push_str(lifetime.grant_id_prefix());
        for byte in entropy {
            value.push(char::from(LOWER_HEX[usize::from(byte >> 4)]));
            value.push(char::from(LOWER_HEX[usize::from(byte & 0x0f)]));
        }
        Self::try_new(value)
    }

    pub fn generated_lifetime(&self) -> Option<GrantLifetime> {
        [GrantLifetime::Session, GrantLifetime::Persistent]
            .into_iter()
            .find(|lifetime| lifetime.token_from_generated_id(self.as_str()).is_some())
    }

    pub fn persistent_token(&self) -> Option<&str> {
        GrantLifetime::Persistent.token_from_generated_id(self.as_str())
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
    #[error("file grant id entropy length is invalid: expected {expected}, got {actual}")]
    InvalidEntropyLength { expected: usize, actual: usize },
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

    pub const fn permits(self, required: Self) -> bool {
        match required {
            Self::Read => self.permits_read(),
            Self::Write => self.permits_write(),
            Self::ReadWrite => matches!(self, Self::ReadWrite),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantLifetime {
    #[default]
    Session,
    Persistent,
}

impl GrantLifetime {
    pub const fn is_persistent(self) -> bool {
        matches!(self, Self::Persistent)
    }

    pub const fn grant_id_prefix(self) -> &'static str {
        match self {
            Self::Session => "native-grant-",
            Self::Persistent => "native-grant-v1-p-",
        }
    }

    pub const fn entropy_bytes(self) -> usize {
        match self {
            Self::Session => 16,
            Self::Persistent => 32,
        }
    }

    pub fn token_from_generated_id(self, value: &str) -> Option<&str> {
        let token = value.strip_prefix(self.grant_id_prefix())?;
        if token.len() != self.entropy_bytes() * 2
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return None;
        }
        Some(token)
    }
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

impl GrantOrigin {
    pub const fn issued_permission(self) -> Option<PermissionKind> {
        match self {
            Self::UserSelection => Some(PermissionKind::UserFileSelection),
            Self::KnownDirectory(_) => Some(PermissionKind::KnownDirectoryAccess),
            Self::Restored => None,
        }
    }

    pub const fn is_valid_issuance_for(self, entry_kind: FileEntryKind) -> bool {
        match self {
            Self::UserSelection => true,
            Self::KnownDirectory(_) => entry_kind.is_directory(),
            Self::Restored => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileEntryKind {
    File,
    Directory,
    Other,
}

impl FileEntryKind {
    pub const fn is_directory(self) -> bool {
        matches!(self, Self::Directory)
    }
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

impl FileDialogMode {
    pub const fn accepts_access(self, access: GrantAccess) -> bool {
        match self {
            Self::OpenFile | Self::OpenFiles => access.permits_read(),
            Self::SaveFile => access.permits_write(),
            Self::PickDirectory => true,
        }
    }
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

impl UserFileRequest {
    pub const fn requested_grant_lifetime(&self) -> Option<GrantLifetime> {
        match self {
            Self::ShowDialog(request) => Some(request.lifetime),
            Self::GrantKnownDirectory { lifetime, .. } => Some(*lifetime),
            Self::ReadText { .. }
            | Self::ReadBytes { .. }
            | Self::WriteText { .. }
            | Self::WriteBytes { .. }
            | Self::Metadata { .. }
            | Self::ListDirectory { .. }
            | Self::Revoke { .. } => None,
        }
    }
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

    #[test]
    fn generated_grant_ids_preserve_session_format_and_classify_persistent_format() {
        let session = FileGrantId::from_entropy(GrantLifetime::Session, &[0x01; 16])
            .expect("session entropy has the required length");
        assert_eq!(
            session.as_str(),
            format!("native-grant-{}", "01".repeat(16))
        );
        assert_eq!(session.generated_lifetime(), Some(GrantLifetime::Session));

        let persistent = FileGrantId::from_entropy(GrantLifetime::Persistent, &[0xab; 32])
            .expect("persistent entropy has the required length");
        assert_eq!(
            persistent.as_str(),
            format!("native-grant-v1-p-{}", "ab".repeat(32))
        );
        assert_eq!(
            persistent.generated_lifetime(),
            Some(GrantLifetime::Persistent)
        );
        let token = "ab".repeat(32);
        assert_eq!(persistent.persistent_token(), Some(token.as_str()));
    }

    #[test]
    fn generated_id_parser_rejects_noncanonical_hex_and_wrong_lengths() {
        let uppercase = FileGrantId::try_new(format!("native-grant-v1-p-{}", "AB".repeat(32)))
            .expect("opaque ids may use host-defined text");
        assert_eq!(uppercase.generated_lifetime(), None);

        let short = FileGrantId::try_new("native-grant-v1-p-deadbeef")
            .expect("opaque ids may use host-defined text");
        assert_eq!(short.generated_lifetime(), None);
    }

    #[test]
    fn restored_origin_does_not_invent_issuance_permission() {
        assert_eq!(GrantOrigin::Restored.issued_permission(), None);
        assert_eq!(
            GrantOrigin::UserSelection.issued_permission(),
            Some(PermissionKind::UserFileSelection)
        );
        assert!(
            GrantOrigin::KnownDirectory(KnownDirectory::Documents)
                .is_valid_issuance_for(FileEntryKind::Directory)
        );
        assert!(
            !GrantOrigin::KnownDirectory(KnownDirectory::Documents)
                .is_valid_issuance_for(FileEntryKind::File)
        );
    }
}
