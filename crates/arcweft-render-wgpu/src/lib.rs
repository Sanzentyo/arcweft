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
pub mod text_editor_geometry;
pub mod ui;
pub mod ui_blend;
pub mod ui_clip_path;
pub mod ui_compositor;
mod ui_compositor_uniform;
pub mod ui_direct_renderer;
pub mod ui_effects;
pub mod ui_mask;
pub mod ui_scene;
