use super::{PaintRect, PaintRectRadii};
use crate::view_box_shadow::ViewBoxShadowPassPlan;
use crate::view_scene::{
    ViewBoxShadow, ViewBoxShadowCornerRadius, ViewBoxShadowList, ViewBoxShadowRadii,
    ViewColorRgba8, ViewFilter, ViewFilterList,
};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_render_text::{TextFontFamily, TextWeight};
use std::ops::Range;

/// Renderer packet for one control's already-resolved current-frame appearance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderControlVisualStyle {
    pub fill: Option<[f32; 4]>,
    pub text: Option<[u8; 4]>,
    pub placeholder: Option<[u8; 4]>,
    pub composition_underline: Option<[f32; 4]>,
    pub font_family: Option<String>,
    pub font_size_px: Option<f32>,
    pub line_height_px: Option<f32>,
    pub letter_spacing_milli: Option<i32>,
    pub font_weight: Option<u16>,
    pub selection: Option<[f32; 4]>,
    pub caret: Option<[f32; 4]>,
    pub border: Option<RenderControlBorderStyle>,
    pub corner_frame: Option<RenderControlCornerFrameStyle>,
    pub focus_ring: Option<RenderControlFocusRingStyle>,
    pub opacity: Option<f32>,
    pub radius_px: Option<f32>,
    pub radii_px: Option<PaintRectRadii>,
    pub depth_milli: Option<i32>,
    pub filters: Option<RenderControlFilterList>,
    pub backdrop_filters: Option<RenderControlFilterList>,
    pub shadows: Vec<RenderControlShadow>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderControlBorderStyle {
    pub color: [f32; 4],
    pub width_px: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderControlCornerFrameStyle {
    pub color: [f32; 4],
    pub width_px: f32,
    pub length_px: f32,
    pub offset_px: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderControlFocusRingStyle {
    pub color: [f32; 4],
    pub width_px: f32,
    pub offset_px: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderControlShadow {
    pub offset_x_px: f32,
    pub offset_y_px: f32,
    pub blur_radius_px: f32,
    pub spread_radius_px: f32,
    pub border_radius_px: f32,
    pub color: [u8; 4],
    pub kind: RenderControlShadowKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderControlShadowKind {
    #[default]
    Outer,
    Inset,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderControlFilterList {
    pub filters: Vec<RenderControlFilter>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RenderControlFilter {
    Brightness { factor: f32 },
    Contrast { factor: f32 },
    Grayscale { amount: f32 },
    Saturate { factor: f32 },
    HueRotateDegrees { degrees: f32 },
    Invert { amount: f32 },
    Sepia { amount: f32 },
    Opacity { amount: f32 },
    Blur { radius_px: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedControlShadow {
    pub target: InteractionTarget,
    pub plan: ViewBoxShadowPassPlan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedControlBackdrop {
    pub target: InteractionTarget,
    pub bounds: HitRect,
    pub filters: ViewFilterList,
    pub sample_policy: RuntimeControlBackdropSamplePolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedControlFilter {
    pub target: InteractionTarget,
    pub bounds: HitRect,
    pub filters: ViewFilterList,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedControlPaint {
    pub target: InteractionTarget,
    pub bounds: HitRect,
    pub rectangle_range: Range<usize>,
    pub text_range: Range<usize>,
    pub backdrop_range: Range<usize>,
    pub shadow_range: Range<usize>,
    pub filter_range: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeControlBackdropSamplePolicy {
    PriorFrameContent,
    PriorFrameContentAndEarlierRuntimeControls,
}

impl RenderControlVisualStyle {
    pub fn radii(&self) -> PaintRectRadii {
        self.radii_px
            .unwrap_or_else(|| PaintRectRadii::uniform(self.radius_px.unwrap_or_default()))
    }
}

impl RenderControlShadow {
    fn view_box_shadow(self, border_radii: ViewBoxShadowRadii) -> ViewBoxShadow {
        let color = ViewColorRgba8 {
            red: self.color[0],
            green: self.color[1],
            blue: self.color[2],
            alpha: self.color[3],
        };
        match self.kind {
            RenderControlShadowKind::Outer => ViewBoxShadow::outer_with_radii(
                self.offset_x_px,
                self.offset_y_px,
                self.blur_radius_px,
                self.spread_radius_px,
                border_radii,
                color,
            ),
            RenderControlShadowKind::Inset => ViewBoxShadow::inset_with_radii(
                self.offset_x_px,
                self.offset_y_px,
                self.blur_radius_px,
                self.spread_radius_px,
                border_radii,
                color,
            ),
        }
    }
}

impl RenderControlFilterList {
    fn view_filter_list(&self) -> ViewFilterList {
        ViewFilterList::from_filters(
            self.filters
                .iter()
                .copied()
                .map(RenderControlFilter::view_filter)
                .collect(),
        )
    }
}

impl RenderControlFilter {
    const fn view_filter(self) -> ViewFilter {
        match self {
            Self::Brightness { factor } => ViewFilter::Brightness(factor),
            Self::Contrast { factor } => ViewFilter::Contrast(factor),
            Self::Grayscale { amount } => ViewFilter::Grayscale(amount),
            Self::Saturate { factor } => ViewFilter::Saturate(factor),
            Self::HueRotateDegrees { degrees } => ViewFilter::HueRotateDegrees(degrees),
            Self::Invert { amount } => ViewFilter::Invert(amount),
            Self::Sepia { amount } => ViewFilter::Sepia(amount),
            Self::Opacity { amount } => ViewFilter::Opacity(amount),
            Self::Blur { radius_px } => ViewFilter::Blur { radius_px },
        }
    }
}

pub(super) fn fill_with_opacity(fill: [f32; 4], opacity: Option<f32>) -> [f32; 4] {
    let mut fill = fill;
    fill[3] *= opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    fill
}

pub(super) fn control_font_families(visual: &RenderControlVisualStyle) -> Vec<TextFontFamily> {
    super::prepared_text::font_families_from_stack(visual.font_family.as_deref())
}

pub(super) fn control_text_weight(
    visual: &RenderControlVisualStyle,
    fallback: TextWeight,
) -> TextWeight {
    match visual.font_weight {
        Some(1..=149) => TextWeight::Thin,
        Some(150..=249) => TextWeight::ExtraLight,
        Some(250..=349) => TextWeight::Light,
        Some(350..=449) => TextWeight::Normal,
        Some(450..=549) => TextWeight::Medium,
        Some(550..=649) => TextWeight::SemiBold,
        Some(650..=749) => TextWeight::Bold,
        Some(750..=849) => TextWeight::ExtraBold,
        Some(850..=1_000) => TextWeight::Black,
        Some(_) | None => fallback,
    }
}

pub(super) fn push_control_border(
    rectangles: &mut Vec<PaintRect>,
    bounds: HitRect,
    border: Option<RenderControlBorderStyle>,
    radii: PaintRectRadii,
) {
    let Some(border) = border else {
        return;
    };
    let width = border
        .width_px
        .max(0.0)
        .min(bounds.width * 0.5)
        .min(bounds.height * 0.5);
    if width <= f32::EPSILON || border.color[3] <= f32::EPSILON {
        return;
    }
    rectangles.push(PaintRect::stroke(bounds, border.color, radii, width));
}

pub(super) fn push_control_corner_frame(
    rectangles: &mut Vec<PaintRect>,
    bounds: HitRect,
    frame: Option<RenderControlCornerFrameStyle>,
) {
    let Some(frame) = frame else {
        return;
    };
    let width = frame
        .width_px
        .max(0.0)
        .min(bounds.width * 0.5)
        .min(bounds.height * 0.5);
    let length = frame
        .length_px
        .max(width)
        .min(bounds.width.max(bounds.height));
    if width <= f32::EPSILON || length <= f32::EPSILON || frame.color[3] <= f32::EPSILON {
        return;
    }
    let rect = bounds.outset(frame.offset_px);
    let horizontal = length.min(rect.width.max(0.0));
    let vertical = length.min(rect.height.max(0.0));
    let right = rect.x + rect.width - width;
    let bottom = rect.y + rect.height - width;
    let horizontal_right = rect.x + rect.width - horizontal;
    let vertical_bottom = rect.y + rect.height - vertical;
    [
        HitRect::new(rect.x, rect.y, horizontal, width),
        HitRect::new(rect.x, rect.y, width, vertical),
        HitRect::new(horizontal_right, rect.y, horizontal, width),
        HitRect::new(right, rect.y, width, vertical),
        HitRect::new(horizontal_right, bottom, horizontal, width),
        HitRect::new(right, vertical_bottom, width, vertical),
        HitRect::new(rect.x, bottom, horizontal, width),
        HitRect::new(rect.x, vertical_bottom, width, vertical),
    ]
    .into_iter()
    .filter(|segment| segment.width > 0.0 && segment.height > 0.0)
    .for_each(|segment| rectangles.push(PaintRect::new(segment, frame.color)));
}

pub(super) fn push_control_focus_ring(
    rectangles: &mut Vec<PaintRect>,
    bounds: HitRect,
    ring: RenderControlFocusRingStyle,
    radii: PaintRectRadii,
) {
    let width = ring.width_px.max(0.0);
    if width <= f32::EPSILON || ring.color[3] <= f32::EPSILON {
        return;
    }
    let outer = bounds.outset(ring.offset_px + width);
    let outer_radii = radii.outset(ring.offset_px + width);
    rectangles.push(PaintRect::stroke(outer, ring.color, outer_radii, width));
}

pub(super) fn push_control_backdrop_plan(
    output: &mut Vec<PreparedControlBackdrop>,
    target: &InteractionTarget,
    bounds: HitRect,
    visual: &RenderControlVisualStyle,
) {
    let Some(filters) = &visual.backdrop_filters else {
        return;
    };
    let filters = filters.view_filter_list();
    if filters.is_empty() {
        return;
    }
    output.push(PreparedControlBackdrop {
        target: target.clone(),
        bounds,
        filters,
        sample_policy: RuntimeControlBackdropSamplePolicy::PriorFrameContent,
    });
}

pub(super) fn push_control_filter_plan(
    output: &mut Vec<PreparedControlFilter>,
    target: &InteractionTarget,
    bounds: HitRect,
    visual: &RenderControlVisualStyle,
) {
    let Some(filters) = &visual.filters else {
        return;
    };
    let filters = filters.view_filter_list();
    if filters.is_empty() {
        return;
    }
    output.push(PreparedControlFilter {
        target: target.clone(),
        bounds,
        filters,
    });
}

pub(super) fn push_control_shadow_plan(
    output: &mut Vec<PreparedControlShadow>,
    target: &InteractionTarget,
    bounds: HitRect,
    visual: &RenderControlVisualStyle,
) {
    let control_radii = visual.radii();
    let has_control_radii = visual.radius_px.is_some() || visual.radii_px.is_some();
    let shadows = ViewBoxShadowList::new(visual.shadows.iter().copied().map(|shadow| {
        let border_radii = if has_control_radii {
            paint_radii_to_box_shadow(control_radii)
        } else {
            ViewBoxShadowRadii::uniform(shadow.border_radius_px)
        };
        shadow.view_box_shadow(border_radii)
    }));
    if shadows.is_empty() {
        return;
    }
    if let Ok(plan) = ViewBoxShadowPassPlan::from_shadows(&shadows, bounds)
        && !plan.is_empty()
    {
        output.push(PreparedControlShadow {
            target: target.clone(),
            plan,
        });
    }
}

fn paint_radii_to_box_shadow(radii: PaintRectRadii) -> ViewBoxShadowRadii {
    ViewBoxShadowRadii::from_corners(
        paint_corner_radius_to_box_shadow(radii.top_left),
        paint_corner_radius_to_box_shadow(radii.top_right),
        paint_corner_radius_to_box_shadow(radii.bottom_right),
        paint_corner_radius_to_box_shadow(radii.bottom_left),
    )
}

fn paint_corner_radius_to_box_shadow(
    radius: super::PaintRectCornerRadius,
) -> ViewBoxShadowCornerRadius {
    ViewBoxShadowCornerRadius::new(radius.x_px, radius.y_px)
}
