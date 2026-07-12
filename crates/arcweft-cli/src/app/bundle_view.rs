//! Facade for lowering authored views into bundle sidecar resources.

mod lowering;

pub(in crate::app) use lowering::{
    ViewSidecarError, expr_source, inline_style_properties, normalize_property_name,
    normalize_view_call, style_layout_length_u32, view_sidecars,
};
