//! Surface- and event-loop-independent Arcweft wgpu renderer.
//!
//! This crate may know `wgpu::Device`, `wgpu::Queue`, `wgpu::TextureView`,
//! viewport dimensions, glyphon, text layout, image resources, and Arcweft
//! presentation data. It must not know winit, web-sys, a native filesystem,
//! browser fetch, surface creation, or blocking readback.

mod convert;
mod font_family;
mod font_system;

pub mod geometry;
pub mod offscreen;
pub mod renderer;
pub mod sample;
pub mod text_editor_geometry;
pub mod view;
pub mod view_blend;
pub mod view_box_shadow;
pub mod view_clip_path;
pub mod view_compositor;
mod view_compositor_uniform;
pub mod view_direct_renderer;
pub mod view_effects;
pub mod view_mask;
pub mod view_scene;
