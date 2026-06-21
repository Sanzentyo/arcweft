//! Typed interaction contracts shared by presentation routers and the runtime core.
//!
//! This crate owns no device I/O, windowing, rendering, or runtime execution.
//! Hosts translate raw platform events into [`RoutedInputEvent`] values before
//! passing them across the deterministic runtime step boundary.

pub mod action;
pub mod audio;
pub mod id;
pub mod input;
pub mod payload;
