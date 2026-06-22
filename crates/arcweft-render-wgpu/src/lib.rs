//! Surface- and event-loop-independent Arcweft wgpu renderer.
//!
//! This crate may know `wgpu::Device`, `wgpu::Queue`, `wgpu::TextureView`,
//! viewport dimensions, glyphon, text layout, image resources, and Arcweft
//! presentation data. It must not know winit, web-sys, a native filesystem,
//! browser fetch, surface creation, or blocking readback.

mod convert;

pub mod geometry;
pub mod offscreen;
pub mod renderer;
pub mod sample;
pub mod ui;
