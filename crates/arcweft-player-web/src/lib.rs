//! WebGPU-first Arcweft browser player.
//!
//! DOM is used only to locate the winit canvas and surface fatal startup
//! errors. Dialogue, choices, focus, hover, and normal game input are rendered
//! and routed through Arcweft presentation data on the canvas.

pub mod clock;
pub mod host;
pub mod images;
pub mod input;
pub mod parity;
pub mod report;

#[cfg(target_arch = "wasm32")]
mod app;

#[cfg(target_arch = "wasm32")]
pub use app::start_arcweft_player;
