//! Native text-input adapter entry points.
//!
//! Platform-specific modules normalize native IME callbacks into
//! `arcweft_presentation::text_input` values. Native handles stay in the owning
//! adapter module and never enter Sans I/O payloads.

pub mod windows_tsf;

#[cfg(all(target_os = "macos", feature = "macos-text-input"))]
pub mod macos_text_input;

#[cfg(all(target_os = "macos", feature = "macos-appkit-text-input"))]
pub mod macos_appkit_bridge;
