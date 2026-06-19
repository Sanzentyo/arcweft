//! Host-side dispatch mechanics for desktop adapters.
//!
//! The queue makes main-thread affinity and asynchronous completion explicit.
//! It does not spawn threads and it does not perform platform I/O by itself.

pub mod backend;
pub mod dispatcher;
pub mod memory;

pub use backend::{BackendCompletion, BackendSubmission, DesktopBackend, ExecutionLane};
pub use dispatcher::{DesktopHost, DesktopSubmission, DesktopTaskId, PumpReport};
pub use memory::MemoryDesktopBackend;
