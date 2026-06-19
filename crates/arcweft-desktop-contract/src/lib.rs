//! Sans I/O data contract shared by Arcweft desktop adapters.
//!
//! This crate intentionally contains no filesystem, window-system, pointer, or
//! event-loop calls. It is safe to use from deterministic runtime and tooling
//! layers.

pub mod error;
pub mod files;
pub mod geometry;
pub mod platform;
pub mod pointer;
pub mod request;
pub mod window;

pub use error::DesktopError;
pub use files::{
    DirectoryEntry, FileDialogMode, FileDialogRequest, FileEntryKind, FileFilter, FileGrant,
    FileGrantId, FileGrantIdError, FileMetadata, GrantAccess, GrantLifetime, GrantOrigin,
    GrantPath, KnownDirectory, PortableRelativePath, RelativePathError, UserFileRequest,
    UserFileResponse,
};
pub use geometry::{
    GeometryError, LogicalPosition, LogicalSize, PhysicalPosition, PhysicalRect, PhysicalSize,
    ScaleFactor,
};
pub use platform::{
    DesktopCapabilities, DesktopFeature, FeatureSupport, PermissionKind, PlatformKind, SupportLevel,
};
pub use pointer::{
    CursorGrabMode, CursorIcon, GlobalPointerRequest, GlobalPointerResponse, OwnedCursorRequest,
    PointerCoordinateSpace, PointerPosition,
};
pub use request::{DesktopRequest, DesktopResponse};
pub use window::{
    ExternalWindowRequest, ExternalWindowResponse, OwnedWindowRequest, OwnedWindowResponse,
    WindowId, WindowIdError, WindowMode, WindowScope, WindowSnapshot, WindowTarget,
};
