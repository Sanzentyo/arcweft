//! Facade for lowering authored views into bundle sidecar resources.

mod lowering;

pub(in crate::app) use lowering::{
    ViewSidecarError, expr_source, normalize_property_name, normalize_view_call, view_sidecars,
};
