//! Browser WebGPU math benchmark harness.

pub mod model;
pub mod policy;
pub mod recommend;
pub mod stability;
pub mod stats;

#[cfg(target_arch = "wasm32")]
pub mod correctness;

#[cfg(target_arch = "wasm32")]
pub mod runner;

#[cfg(test)]
mod tests;
