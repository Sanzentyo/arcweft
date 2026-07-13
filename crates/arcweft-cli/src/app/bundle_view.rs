//! Facade for lowering authored views into bundle sidecar resources.

mod lowering;

pub(in crate::app) use lowering::{normalize_view_call, view_sidecars};
