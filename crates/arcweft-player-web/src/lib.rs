//! WebGPU-first Arcweft browser player.
//!
//! DOM is used only to locate the winit canvas and surface fatal startup
//! errors. Dialogue, choices, focus, hover, and normal game input are rendered
//! and routed through Arcweft presentation data on the canvas.

pub mod clock;
pub mod edit_context;
pub mod host;
pub mod parity;
pub mod report;
pub mod runtime_text_input;
pub mod web_text_input;

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod clipboard;
#[cfg(target_arch = "wasm32")]
mod inset_shadow_exact_capture;

#[cfg(target_arch = "wasm32")]
pub use app::{
    ArcweftWebPlayerHandle, arcweft_player_handle, create_arcweft_player,
    create_arcweft_player_with_options, start_arcweft_player, start_arcweft_player_with_options,
    stop_arcweft_player,
};
#[cfg(target_arch = "wasm32")]
pub use inset_shadow_exact_capture::capture_seq06_13e1_inset_box_shadow_exact_png;
