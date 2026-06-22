//! Safe native implementation for Arcweft desktop adapters.
//!
//! One backend is shared by several logical manifests, but authority remains
//! split at the Arcweft adapter boundary: owned windows, user files, global
//! pointer, external-window observation, and external-window control.

mod backend;
mod capabilities;
mod config;
mod driver;
mod external;
mod files;
mod grant_store;
mod persistent_grants;
mod platform;
mod pointer;

pub use backend::{NativeDesktopBackend, NativeDesktopBuilder};
pub use config::{GlobalPointerPolicy, NativeDesktopOptions};
pub use driver::{ExternalWindowControlDriver, OwnedWindowDriver};
pub use persistent_grants::{PersistentGrantConfig, PersistentGrantServices};
pub use platform::native_platform_kind;
