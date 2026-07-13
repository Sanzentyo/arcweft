use arcweft_bundle::resource_codec::view::{
    ViewRuntimeControlBorderStyle, ViewRuntimeControlCornerFrameStyle,
    ViewRuntimeControlCornerRadius, ViewRuntimeControlFilter, ViewRuntimeControlFilterList,
    ViewRuntimeControlFocusRingStyle, ViewRuntimeControlRadii, ViewRuntimeControlVisualStyle,
    ViewRuntimeShadow, ViewRuntimeShadowKind,
};
use arcweft_presentation::appearance::PresentationColor;
use arcweft_render_wgpu::geometry::{
    PaintRectCornerRadius, PaintRectRadii, RenderControlBorderStyle, RenderControlCornerFrameStyle,
    RenderControlFilter, RenderControlFilterList, RenderControlFocusRingStyle, RenderControlShadow,
    RenderControlShadowKind, RenderControlVisualStyle,
};
use num_traits::ToPrimitive;

pub(crate) fn lower_control_style(
    style: &ViewRuntimeControlVisualStyle,
) -> RenderControlVisualStyle {
    RenderControlVisualStyle {
        fill: style.fill.map(rgba_f32),
        text: style.text.map(rgba_u8),
        placeholder: style.placeholder.map(rgba_u8),
        composition_underline: style.composition_underline.map(rgba_f32),
        font_family: style.font_family.clone(),
        font_size_px: style.font_size_milli.map(milli_u32_to_f32),
        line_height_px: style.line_height_milli.map(milli_u32_to_f32),
        letter_spacing_milli: style.letter_spacing_milli,
        font_weight: style.font_weight,
        selection: style.selection.map(rgba_f32),
        caret: style.caret.map(rgba_f32),
        border: style.border.map(lower_border),
        corner_frame: style.corner_frame.map(lower_corner_frame),
        focus_ring: style.focus_ring.map(lower_focus_ring),
        opacity: style.opacity_milli.map(|value| f32::from(value) / 1_000.0),
        radius_px: style.radius_milli.map(milli_u32_to_f32),
        radii_px: style.radii_milli.map(lower_radii),
        depth_milli: style.depth_milli,
        filters: style.filters.as_ref().map(lower_filter_list),
        backdrop_filters: style.backdrop_filters.as_ref().map(lower_filter_list),
        shadows: style.shadows.iter().copied().map(lower_shadow).collect(),
    }
}

fn lower_border(border: ViewRuntimeControlBorderStyle) -> RenderControlBorderStyle {
    RenderControlBorderStyle {
        color: rgba_f32(border.color),
        width_px: milli_u32_to_f32(border.width_milli),
    }
}

fn lower_corner_frame(frame: ViewRuntimeControlCornerFrameStyle) -> RenderControlCornerFrameStyle {
    RenderControlCornerFrameStyle {
        color: rgba_f32(frame.color),
        width_px: milli_u32_to_f32(frame.width_milli),
        length_px: milli_u32_to_f32(frame.length_milli),
        offset_px: milli_i32_to_f32(frame.offset_milli),
    }
}

fn lower_focus_ring(ring: ViewRuntimeControlFocusRingStyle) -> RenderControlFocusRingStyle {
    RenderControlFocusRingStyle {
        color: rgba_f32(ring.color),
        width_px: milli_u32_to_f32(ring.width_milli),
        offset_px: milli_i32_to_f32(ring.offset_milli),
    }
}

fn lower_radii(radii: ViewRuntimeControlRadii) -> PaintRectRadii {
    PaintRectRadii::new(
        lower_corner_radius(radii.top_left),
        lower_corner_radius(radii.top_right),
        lower_corner_radius(radii.bottom_right),
        lower_corner_radius(radii.bottom_left),
    )
}

fn lower_corner_radius(radius: ViewRuntimeControlCornerRadius) -> PaintRectCornerRadius {
    PaintRectCornerRadius::new(
        milli_u32_to_f32(radius.x_milli),
        milli_u32_to_f32(radius.y_milli),
    )
}

fn lower_filter_list(list: &ViewRuntimeControlFilterList) -> RenderControlFilterList {
    RenderControlFilterList {
        filters: list.filters.iter().copied().map(lower_filter).collect(),
    }
}

fn lower_filter(filter: ViewRuntimeControlFilter) -> RenderControlFilter {
    match filter {
        ViewRuntimeControlFilter::Brightness { factor_milli } => RenderControlFilter::Brightness {
            factor: milli_u32_to_f32(factor_milli),
        },
        ViewRuntimeControlFilter::Contrast { factor_milli } => RenderControlFilter::Contrast {
            factor: milli_u32_to_f32(factor_milli),
        },
        ViewRuntimeControlFilter::Grayscale { amount_milli } => RenderControlFilter::Grayscale {
            amount: f32::from(amount_milli) / 1_000.0,
        },
        ViewRuntimeControlFilter::Saturate { factor_milli } => RenderControlFilter::Saturate {
            factor: milli_u32_to_f32(factor_milli),
        },
        ViewRuntimeControlFilter::HueRotate { degrees_milli } => {
            RenderControlFilter::HueRotateDegrees {
                degrees: milli_i32_to_f32(degrees_milli),
            }
        }
        ViewRuntimeControlFilter::Invert { amount_milli } => RenderControlFilter::Invert {
            amount: f32::from(amount_milli) / 1_000.0,
        },
        ViewRuntimeControlFilter::Sepia { amount_milli } => RenderControlFilter::Sepia {
            amount: f32::from(amount_milli) / 1_000.0,
        },
        ViewRuntimeControlFilter::Opacity { amount_milli } => RenderControlFilter::Opacity {
            amount: f32::from(amount_milli) / 1_000.0,
        },
        ViewRuntimeControlFilter::Blur { radius_milli } => RenderControlFilter::Blur {
            radius_px: milli_u32_to_f32(radius_milli),
        },
    }
}

fn lower_shadow(shadow: ViewRuntimeShadow) -> RenderControlShadow {
    RenderControlShadow {
        offset_x_px: milli_i32_to_f32(shadow.offset_x_milli),
        offset_y_px: milli_i32_to_f32(shadow.offset_y_milli),
        blur_radius_px: milli_u32_to_f32(shadow.blur_milli),
        spread_radius_px: milli_i32_to_f32(shadow.spread_milli),
        border_radius_px: milli_u32_to_f32(shadow.radius_milli),
        color: rgba_u8(shadow.color),
        kind: match shadow.kind {
            ViewRuntimeShadowKind::Outer => RenderControlShadowKind::Outer,
            ViewRuntimeShadowKind::Inset => RenderControlShadowKind::Inset,
        },
    }
}

fn rgba_f32(color: PresentationColor) -> [f32; 4] {
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        f32::from(color.alpha) / 255.0,
    ]
}

fn rgba_u8(color: PresentationColor) -> [u8; 4] {
    [color.red, color.green, color.blue, color.alpha]
}

fn milli_i32_to_f32(value: i32) -> f32 {
    value.to_f32().unwrap_or_else(|| {
        if value.is_negative() {
            f32::MIN
        } else {
            f32::MAX
        }
    }) / 1_000.0
}

fn milli_u32_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX) / 1_000.0
}
