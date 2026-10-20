//! Typed recording and deterministic replay of durable root transitions.

mod admission;
mod execute;
mod model;
mod record;

pub use model::*;
