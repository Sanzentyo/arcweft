//! Browser WebGPU surface host for Arcweft's shared renderer.
//!
//! The implementation is wasm-only. It owns canvas/window surface lifecycle,
//! resize, device health, and presentation; game semantics and GPU pipelines
//! remain in portable crates.

#[cfg(target_arch = "wasm32")]
pub mod web;
