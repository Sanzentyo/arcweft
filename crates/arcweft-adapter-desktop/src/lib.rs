//! Arcweft host-adapter bindings for the portable desktop contract.
//!
//! Apply `patches/0001-host-adapter-pending-completions.patch` first. The
//! patch adds pending completion and cancellation without moving OS I/O into
//! `arcweft-core`.

mod adapter;
mod codec;
mod manifest;

pub use adapter::{DesktopAdapterSet, DesktopCoordinator};
pub use manifest::{
    DESKTOP_CAPABILITIES_CALL, DESKTOP_EXTERNAL_CONTROL_ADAPTER_ID, DESKTOP_EXTERNAL_CONTROL_CALL,
    DESKTOP_EXTERNAL_OBSERVE_ADAPTER_ID, DESKTOP_EXTERNAL_OBSERVE_CALL,
    DESKTOP_FILES_READ_ADAPTER_ID, DESKTOP_FILES_READ_CALL, DESKTOP_FILES_WRITE_ADAPTER_ID,
    DESKTOP_FILES_WRITE_CALL, DESKTOP_GLOBAL_POINTER_CONTROL_ADAPTER_ID,
    DESKTOP_GLOBAL_POINTER_CONTROL_CALL, DESKTOP_GLOBAL_POINTER_OBSERVE_ADAPTER_ID,
    DESKTOP_GLOBAL_POINTER_OBSERVE_CALL, DESKTOP_KNOWN_READ_ADAPTER_ID, DESKTOP_KNOWN_READ_CALL,
    DESKTOP_KNOWN_WRITE_ADAPTER_ID, DESKTOP_KNOWN_WRITE_CALL, DESKTOP_OWNED_CURSOR_CALL,
    DESKTOP_OWNED_WINDOW_ADAPTER_ID, DESKTOP_OWNED_WINDOW_CALL, DESKTOP_PLATFORM_ADAPTER_ID,
    all_desktop_manifests, desktop_external_control_manifest, desktop_external_observe_manifest,
    desktop_files_read_manifest, desktop_files_write_manifest,
    desktop_known_directory_read_manifest, desktop_known_directory_write_manifest,
    desktop_owned_window_manifest, desktop_platform_manifest,
    desktop_pointer_global_control_manifest, desktop_pointer_global_observe_manifest,
    standard_desktop_manifests,
};
