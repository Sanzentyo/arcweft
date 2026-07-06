//! Portable Arcweft bundle execution driver.
//!
//! This crate owns no filesystem, wall clock, thread pool, GPU, audio device,
//! window, or browser API. Native and Web players provide a quantized logical
//! clock, normalized input, and deterministic host-task completions.

pub mod clock;
pub mod display;
pub mod generation_runtime;
pub mod presentation_handles;
pub mod session;
pub mod session_save;
pub mod swap;
pub mod task;
pub mod text_control_writeback;
