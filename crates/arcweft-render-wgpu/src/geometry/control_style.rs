use super::{PaintRect, PaintRectRadii, RenderFontFamily};
use crate::ui_box_shadow::UiBoxShadowPassPlan;
use crate::ui_scene::{
    UiBoxShadow, UiBoxShadowCornerRadius, UiBoxShadowList, UiBoxShadowRadii, UiColorRgba8,
    UiFilter, UiFilterList,
};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use std::ops::Range;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderControlStyle {
    pub normal: RenderControlVisualStyle,
    pub hover: Option<RenderControlVisualStyle>,
    pub pressed: Option<RenderControlVisualStyle>,
    pub focus_visible: Option<RenderControlVisualStyle>,
    pub disabled: Option<RenderControlVisualStyle>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderControlVisualStyle {
    pub fill: Option<[f32; 4]>,
    pub text: Option<[u8; 4]>,
    pub font_family: Option<String>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderControlVisualState {
    Normal,
    Hover,
    Pressed,
    FocusVisible,
    Disabled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedControlShadow {
    pub target: InteractionTarget,
    pub plan: UiBoxShadowPassPlan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedControlBackdrop {
    pub target: InteractionTarget,
    pub bounds: HitRect,
    pub filters: UiFilterList,
    pub sample_policy: RuntimeControlBackdropSamplePolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedControlFilter {
    pub target: InteractionTarget,
    pub bounds: HitRect,
    pub filters: UiFilterList,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ControlInteractionStyleState {
    pub enabled: bool,
    pub focused: bool,
    pub pointer: ControlPointerStyleState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ControlPointerStyleState {
    #[default]
    Idle,
    Hovered,
    Pressed,
}

impl ControlPointerStyleState {
    pub(super) const fn from_interaction(hovered: bool, pressed: bool) -> Self {
        if pressed {
            Self::Pressed
        } else if hovered {
            Self::Hovered
        } else {
            Self::Idle
        }
    }
}

impl RenderControlStyle {
    #[must_use]
    pub fn visual_for_state(&self, state: RenderControlVisualState) -> RenderControlVisualStyle {
        let mut visual = self.normal.clone();
        match state {
            RenderControlVisualState::Normal => None,
            RenderControlVisualState::Hover => self.hover.as_ref(),
            RenderControlVisualState::Pressed => self.pressed.as_ref(),
            RenderControlVisualState::FocusVisible => self.focus_visible.as_ref(),
            RenderControlVisualState::Disabled => self.disabled.as_ref(),
        }
        .into_iter()
        .for_each(|patch| visual.overlay(patch));
        visual
    }
}

impl RenderControlVisualStyle {
    pub fn radii(&self) -> PaintRectRadii {
        self.radii_px
            .unwrap_or_else(|| PaintRectRadii::uniform(self.radius_px.unwrap_or_default()))
    }

    fn overlay(&mut self, patch: &Self) {
        if patch.fill.is_some() {
            self.fill = patch.fill;
        }
        if patch.text.is_some() {
            self.text = patch.text;
        }
        if patch.font_family.is_some() {
            self.font_family.clone_from(&patch.font_family);
        }
        if patch.selection.is_some() {
            self.selection = patch.selection;
        }
        if patch.caret.is_some() {
            self.caret = patch.caret;
        }
        if patch.border.is_some() {
            self.border = patch.border;
        }
        if patch.corner_frame.is_some() {
            self.corner_frame = patch.corner_frame;
        }
        if patch.focus_ring.is_some() {
            self.focus_ring = patch.focus_ring;
        }
        if patch.opacity.is_some() {
            self.opacity = patch.opacity;
        }
        if patch.radius_px.is_some() {
            self.radius_px = patch.radius_px;
            self.radii_px = None;
        }
        if patch.radii_px.is_some() {
            self.radii_px = patch.radii_px;
            self.radius_px = None;
        }
        if patch.depth_milli.is_some() {
            self.depth_milli = patch.depth_milli;
        }
        if patch.filters.is_some() {
            self.filters.clone_from(&patch.filters);
        }
        if patch.backdrop_filters.is_some() {
            self.backdrop_filters.clone_from(&patch.backdrop_filters);
        }
        if !patch.shadows.is_empty() {
            self.shadows.clone_from(&patch.shadows);
        }
    }
}

impl RenderControlShadow {
    fn ui_box_shadow(self, border_radii: UiBoxShadowRadii) -> UiBoxShadow {
        let color = UiColorRgba8 {
            red: self.color[0],
            green: self.color[1],
            blue: self.color[2],
            alpha: self.color[3],
        };
        match self.kind {
            RenderControlShadowKind::Outer => UiBoxShadow::outer_with_radii(
                self.offset_x_px,
                self.offset_y_px,
                self.blur_radius_px,
                self.spread_radius_px,
                border_radii,
                color,
            ),
            RenderControlShadowKind::Inset => UiBoxShadow::inset_with_radii(
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
    fn ui_filter_list(&self) -> UiFilterList {
        UiFilterList::from_filters(
            self.filters
                .iter()
                .copied()
                .map(RenderControlFilter::ui_filter)
                .collect(),
        )
    }
}

impl RenderControlFilter {
    const fn ui_filter(self) -> UiFilter {
        match self {
            Self::Brightness { factor } => UiFilter::Brightness(factor),
            Self::Contrast { factor } => UiFilter::Contrast(factor),
            Self::Grayscale { amount } => UiFilter::Grayscale(amount),
            Self::Saturate { factor } => UiFilter::Saturate(factor),
            Self::HueRotateDegrees { degrees } => UiFilter::HueRotateDegrees(degrees),
            Self::Invert { amount } => UiFilter::Invert(amount),
            Self::Sepia { amount } => UiFilter::Sepia(amount),
            Self::Opacity { amount } => UiFilter::Opacity(amount),
            Self::Blur { radius_px } => UiFilter::Blur { radius_px },
        }
    }
}

pub(super) fn fill_with_opacity(fill: [f32; 4], opacity: Option<f32>) -> [f32; 4] {
    let mut fill = fill;
    fill[3] *= opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    fill
}

pub(super) fn control_font_family(visual: &RenderControlVisualStyle) -> RenderFontFamily {
    visual
        .font_family
        .as_ref()
        .map_or(RenderFontFamily::SansSerif, |font_family| {
            RenderFontFamily::Named(font_family.clone())
        })
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
    let filters = filters.ui_filter_list();
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
    let filters = filters.ui_filter_list();
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
    let shadows = UiBoxShadowList::new(visual.shadows.iter().copied().map(|shadow| {
        let border_radii = if has_control_radii {
            paint_radii_to_box_shadow(control_radii)
        } else {
            UiBoxShadowRadii::uniform(shadow.border_radius_px)
        };
        shadow.ui_box_shadow(border_radii)
    }));
    if shadows.is_empty() {
        return;
    }
    if let Ok(plan) = UiBoxShadowPassPlan::from_shadows(&shadows, bounds)
        && !plan.is_empty()
    {
        output.push(PreparedControlShadow {
            target: target.clone(),
            plan,
        });
    }
}

fn paint_radii_to_box_shadow(radii: PaintRectRadii) -> UiBoxShadowRadii {
    UiBoxShadowRadii::from_corners(
        paint_corner_radius_to_box_shadow(radii.top_left),
        paint_corner_radius_to_box_shadow(radii.top_right),
        paint_corner_radius_to_box_shadow(radii.bottom_right),
        paint_corner_radius_to_box_shadow(radii.bottom_left),
    )
}

fn paint_corner_radius_to_box_shadow(
    radius: super::PaintRectCornerRadius,
) -> UiBoxShadowCornerRadius {
    UiBoxShadowCornerRadius::new(radius.x_px, radius.y_px)
}

pub(super) fn state_from_interaction(
    interaction: ControlInteractionStyleState,
) -> RenderControlVisualState {
    if !interaction.enabled {
        RenderControlVisualState::Disabled
    } else if interaction.pointer == ControlPointerStyleState::Pressed {
        RenderControlVisualState::Pressed
    } else if interaction.focused {
        RenderControlVisualState::FocusVisible
    } else if interaction.pointer == ControlPointerStyleState::Hovered {
        RenderControlVisualState::Hover
    } else {
        RenderControlVisualState::Normal
    }
}
