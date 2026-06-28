//! Windows Text Services Framework text-input adapter.
//!
//! Pure conversion, capability, serial, display-attribute, and geometry code is
//! available on all targets for fixture validation. Windows COM type aliases
//! are isolated in `bindings` and compiled only on Windows.

mod activation;
#[cfg(target_os = "windows")]
mod bindings;
mod capabilities;
mod display_attributes;
mod edit_session;
mod geometry;
mod range;

pub use activation::{WindowsTsfActivation, WindowsTsfActivationDiagnostic, WindowsTsfAdapter};
pub use capabilities::{
    WindowsTsfCapabilityEntry, WindowsTsfCapabilityReport, WindowsTsfDisplayAttributeState,
    WindowsTsfFeature, WindowsTsfFeatureStatus, WindowsTsfLayoutState, WindowsTsfReconversionState,
    WindowsTsfRuntimeFacts, WindowsTsfRuntimeState,
};
pub use display_attributes::{TsfDisplayAttributeClass, TsfDisplayAttributeSegment};
pub use edit_session::{
    WindowsTsfEditAccess, WindowsTsfEditSessionBuilder, WindowsTsfEventContext,
    WindowsTsfSerialAllocator,
};
pub use geometry::{TsfLayoutResult, TsfScreenRect, WindowsTsfGeometry};
pub use range::{TsfAcp, TsfAcpRange, TsfRangeError, TsfTextSnapshot};
