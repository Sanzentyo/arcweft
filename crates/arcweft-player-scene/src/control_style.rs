use arcweft_bundle::resource_codec::ui::{
    RgbaColor, UiRuntimeControlBorderStyle, UiRuntimeControlFocusRingStyle, UiRuntimeControlStyle,
    UiRuntimeControlVisualStyle, UiRuntimeShadow, UiRuntimeShadowKind,
};
use arcweft_render_wgpu::geometry::{
    RenderControlBorderStyle, RenderControlFocusRingStyle, RenderControlShadow,
    RenderControlShadowKind, RenderControlStyle, RenderControlVisualStyle,
};
use num_traits::ToPrimitive;

pub(crate) fn lower_control_style(style: &UiRuntimeControlStyle) -> RenderControlStyle {
    RenderControlStyle {
        normal: lower_visual_style(&style.normal),
        hover: style.hover.as_ref().map(lower_visual_style),
        pressed: style.pressed.as_ref().map(lower_visual_style),
        focus_visible: style.focus_visible.as_ref().map(lower_visual_style),
        disabled: style.disabled.as_ref().map(lower_visual_style),
    }
}

fn lower_visual_style(style: &UiRuntimeControlVisualStyle) -> RenderControlVisualStyle {
    RenderControlVisualStyle {
        fill: style.fill.map(rgba_f32),
        text: style.text.map(rgba_u8),
        selection: style.selection.map(rgba_f32),
        caret: style.caret.map(rgba_f32),
        border: style.border.map(lower_border),
        focus_ring: style.focus_ring.map(lower_focus_ring),
        opacity: style.opacity_milli.map(|value| f32::from(value) / 1_000.0),
        radius_px: style.radius_milli.map(milli_u32_to_f32),
        shadows: style.shadows.iter().copied().map(lower_shadow).collect(),
    }
}

fn lower_border(border: UiRuntimeControlBorderStyle) -> RenderControlBorderStyle {
    RenderControlBorderStyle {
        color: rgba_f32(border.color),
        width_px: milli_u32_to_f32(border.width_milli),
    }
}

fn lower_focus_ring(ring: UiRuntimeControlFocusRingStyle) -> RenderControlFocusRingStyle {
    RenderControlFocusRingStyle {
        color: rgba_f32(ring.color),
        width_px: milli_u32_to_f32(ring.width_milli),
        offset_px: milli_i32_to_f32(ring.offset_milli),
    }
}

fn lower_shadow(shadow: UiRuntimeShadow) -> RenderControlShadow {
    RenderControlShadow {
        offset_x_px: milli_i32_to_f32(shadow.offset_x_milli),
        offset_y_px: milli_i32_to_f32(shadow.offset_y_milli),
        blur_radius_px: milli_u32_to_f32(shadow.blur_milli),
        spread_radius_px: milli_i32_to_f32(shadow.spread_milli),
        border_radius_px: milli_u32_to_f32(shadow.radius_milli),
        color: rgba_u8(shadow.color),
        kind: match shadow.kind {
            UiRuntimeShadowKind::Outer => RenderControlShadowKind::Outer,
            UiRuntimeShadowKind::Inset => RenderControlShadowKind::Inset,
        },
    }
}

fn rgba_f32(color: RgbaColor) -> [f32; 4] {
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        f32::from(color.alpha) / 255.0,
    ]
}

fn rgba_u8(color: RgbaColor) -> [u8; 4] {
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
