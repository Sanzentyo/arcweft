//! Portable Arcweft bundle execution driver.
//!
//! This crate owns no filesystem, wall clock, thread pool, GPU, audio device,
//! window, or browser API. Native and Web players provide a quantized logical
//! clock, normalized input, and deterministic host-task completions.

pub mod clock;
pub mod display;
pub mod session;
pub mod swap;
pub mod task;
