//! Typed recording and deterministic replay of durable root transitions.

mod admission;
mod codec;
mod execute;
mod model;
mod record;

pub use model::*;
