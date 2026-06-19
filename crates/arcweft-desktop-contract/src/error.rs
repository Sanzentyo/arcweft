use crate::{DesktopFeature, PermissionKind, PlatformKind};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable, serializable failure domain crossing the host/runtime boundary.
#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DesktopError {
    #[error("feature {feature:?} is unsupported on {platform:?}: {detail}")]
    Unsupported {
        feature: DesktopFeature,
        platform: PlatformKind,
        detail: String,
    },
    #[error("permission {permission:?} is required: {detail}")]
    PermissionRequired {
        permission: PermissionKind,
        detail: String,
    },
    #[error("permission {permission:?} was denied: {detail}")]
    PermissionDenied {
        permission: PermissionKind,
        detail: String,
    },
    #[error("the user cancelled the operation")]
    UserCancelled,
    #[error("invalid argument `{field}`: {detail}")]
    InvalidArgument { field: String, detail: String },
    #[error("resource `{resource}` was not found")]
    NotFound { resource: String },
    #[error("opaque handle `{handle}` is stale or belongs to another host")]
    StaleHandle { handle: String },
    #[error("operation must run on the bound host main thread")]
    MainThreadRequired,
    #[error("backend `{backend}` is unavailable: {detail}")]
    BackendUnavailable { backend: String, detail: String },
    #[error("I/O operation `{operation}` failed: {detail}")]
    Io { operation: String, detail: String },
    #[error("platform operation `{operation}` failed: {detail}")]
    Platform {
        operation: String,
        code: Option<i64>,
        detail: String,
    },
    #[error("host response did not match request `{request}`")]
    ResponseMismatch { request: String },
}

impl DesktopError {
    /// Creates an I/O error without exposing a native absolute path.
    pub fn sanitized_io(operation: impl Into<String>, error: &std::io::Error) -> Self {
        Self::Io {
            operation: operation.into(),
            detail: format!("{:?}", error.kind()),
        }
    }
}
