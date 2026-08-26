use super::control_style::{
    PreparedControlBackdrop, PreparedControlFilter, PreparedControlPaint, PreparedControlShadow,
    RenderControlVisualStyle, control_font_families, control_text_weight, fill_with_opacity,
    push_control_backdrop_plan, push_control_border, push_control_corner_frame,
    push_control_filter_plan, push_control_focus_ring, push_control_shadow_plan,
};
use super::{
    FramePlanError, PaintRect, Palette, PlannedFrameText, PlannedPlainText, PlannedTextOwner,
    PreparedTextDocumentRequest,
};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use arcweft_render_text::{ResolvedTextRunSource, TextSlant, TextWeight};
use arcweft_text_layout::{LayoutPoint, LayoutRect, LayoutSize};

/// Player-rendered action button lowered from product View resources.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderActionButton {
    pub target: InteractionTarget,
    pub label: String,
    pub enabled: bool,
    pub containing_scroll_region: Option<String>,
    pub bounds: HitRect,
    pub viewport_clip: Option<HitRect>,
    pub style: RenderControlVisualStyle,
    pub action: RenderActionButtonAction,
}

/// Action emitted by a player-rendered button.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenderActionButtonAction {
    Noop,
    ActionInvoke {
        action: PublicId,
        payload: Option<String>,
    },
    ViewHandler {
        event: arcweft_view::EventKind,
        route: arcweft_view::ViewHandlerRouteId,
    },
}

/// Prepared button hit-test and activation payload.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedActionButton {
    pub target: InteractionTarget,
    pub label: String,
    pub enabled: bool,
    pub action: RenderActionButtonAction,
}

pub(super) struct ActionButtonBuildOutput<'a> {
    pub(super) semantics: &'a mut SemanticTree,
    pub(super) rectangles: &'a mut Vec<PaintRect>,
    pub(super) text: &'a mut Vec<PlannedFrameText>,
    pub(super) control_backdrops: &'a mut Vec<PreparedControlBackdrop>,
    pub(super) control_shadows: &'a mut Vec<PreparedControlShadow>,
    pub(super) control_filters: &'a mut Vec<PreparedControlFilter>,
}

pub(super) fn action_button_depth_milli(button: &RenderActionButton) -> i32 {
    button.style.depth_milli.unwrap_or_default()
}

pub(super) fn build_action_button(
    layer: &LayerId,
    button: &RenderActionButton,
    output: ActionButtonBuildOutput<'_>,
    palette: &Palette,
    font_size: f32,
    line_height: f32,
) -> Result<(PreparedActionButton, PreparedControlPaint), FramePlanError> {
    let ActionButtonBuildOutput {
        semantics,
        rectangles,
        text,
        control_backdrops,
        control_shadows,
        control_filters,
    } = output;
    let visual = &button.style;
    let radii = visual.radii();
    let visible_bounds = visible_button_bounds(button).unwrap_or(button.bounds);
    let backdrop_start = control_backdrops.len();
    push_control_backdrop_plan(control_backdrops, &button.target, visible_bounds, visual);
    let shadow_start = control_shadows.len();
    push_control_shadow_plan(control_shadows, &button.target, visible_bounds, visual);
    let rectangle_start = rectangles.len();
    rectangles.push(PaintRect::with_radii(
        button.bounds,
        fill_with_opacity(visual.fill.unwrap_or(palette.choice_idle), visual.opacity),
        radii,
    ));
    push_control_border(rectangles, button.bounds, visual.border, radii);
    push_control_corner_frame(rectangles, button.bounds, visual.corner_frame);
    if let Some(ring) = visual.focus_ring {
        push_control_focus_ring(rectangles, button.bounds, ring, radii);
    }
    let text_start = text.len();
    push_action_button_text(
        text,
        button,
        visual,
        palette,
        font_size,
        line_height,
        visible_bounds,
    )?;
    let filter_start = control_filters.len();
    push_control_filter_plan(control_filters, &button.target, visible_bounds, visual);
    apply_viewport_clip_to_rectangles(&mut rectangles[rectangle_start..], button.viewport_clip);
    let paint = PreparedControlPaint {
        target: button.target.clone(),
        bounds: visible_bounds,
        rectangle_range: rectangle_start..rectangles.len(),
        text_range: text_start..text.len(),
        backdrop_range: backdrop_start..control_backdrops.len(),
        shadow_range: shadow_start..control_shadows.len(),
        filter_range: filter_start..control_filters.len(),
    };
    push_action_button_semantic(semantics, layer, button, visible_bounds);
    Ok((
        PreparedActionButton {
            target: button.target.clone(),
            label: button.label.clone(),
            enabled: button.enabled,
            action: button.action.clone(),
        },
        paint,
    ))
}

fn push_action_button_text(
    text: &mut Vec<PlannedFrameText>,
    button: &RenderActionButton,
    visual: &RenderControlVisualStyle,
    palette: &Palette,
    font_size: f32,
    line_height: f32,
    visible_bounds: HitRect,
) -> Result<(), FramePlanError> {
    if button.label.is_empty() {
        return Ok(());
    }
    let Some(clip_bounds) = clipped_viewport_bounds(button.bounds, button) else {
        return Ok(());
    };
    let text_bounds = HitRect::new(
        button.bounds.x + 18.0,
        button.bounds.y + (button.bounds.height - line_height) * 0.5,
        (button.bounds.width - 36.0).max(1.0),
        line_height,
    );
    let style = super::prepared_text::resolved_plain_style(
        control_font_families(visual),
        font_size,
        line_height,
        control_text_weight(visual, TextWeight::Bold),
        TextSlant::Upright,
        visual.text.unwrap_or(palette.choice_text),
    )?
    .with_spacing(visual.letter_spacing_milli.unwrap_or_default(), 0);
    text.push(PlannedFrameText::Plain(Box::new(PlannedPlainText {
        text: button.label.clone(),
        style,
        source: ResolvedTextRunSource::Generated,
        request: PreparedTextDocumentRequest {
            origin: LayoutPoint::new(text_bounds.x, text_bounds.y),
            size: LayoutSize::new(text_bounds.width, line_height),
            container_bounds: LayoutRect::new(
                text_bounds.x,
                text_bounds.y,
                text_bounds.width,
                text_bounds.height,
            ),
            clip: Some(LayoutRect::new(
                clip_bounds.x,
                clip_bounds.y,
                clip_bounds.width,
                clip_bounds.height,
            )),
            target: None,
            selection_enabled: false,
            selection: None,
            selection_rgba: palette.choice_active,
        },
        owner: PlannedTextOwner {
            semantic_id: button.target.id().clone(),
            object_bounds: visible_bounds,
        },
    })));
    Ok(())
}

fn push_action_button_semantic(
    semantics: &mut SemanticTree,
    layer: &LayerId,
    button: &RenderActionButton,
    visible_bounds: HitRect,
) {
    let mut node = SemanticNode::new(
        layer.clone(),
        button.target.clone(),
        SemanticRole::Button,
        visible_bounds,
    )
    .with_label(button.label.clone())
    .with_enabled(button.enabled);
    if let Some(action) = button.action.semantic_action_id() {
        node = node.with_action(action.clone());
    }
    semantics.push(node);
}

fn visible_button_bounds(button: &RenderActionButton) -> Option<HitRect> {
    button.viewport_clip.map_or(Some(button.bounds), |clip| {
        super::intersect_hit_rect(button.bounds, clip)
    })
}

fn clipped_viewport_bounds(bounds: HitRect, button: &RenderActionButton) -> Option<HitRect> {
    button
        .viewport_clip
        .map_or(Some(bounds), |clip| super::intersect_hit_rect(bounds, clip))
}

fn apply_viewport_clip_to_rectangles(rectangles: &mut [PaintRect], viewport_clip: Option<HitRect>) {
    let Some(viewport_clip) = viewport_clip else {
        return;
    };
    for rectangle in rectangles {
        let next_clip = rectangle.clip.map_or(
            Some(super::PaintRectClip {
                bounds: viewport_clip,
                radii: super::PaintRectRadii::ZERO,
            }),
            |clip| {
                super::intersect_hit_rect(clip.bounds, viewport_clip).map(|bounds| {
                    super::PaintRectClip {
                        bounds,
                        radii: clip.radii,
                    }
                })
            },
        );
        match next_clip {
            Some(clip) => {
                rectangle.clip = Some(clip);
            }
            None => {
                rectangle.rgba[3] = 0.0;
            }
        }
    }
}

impl RenderActionButtonAction {
    #[must_use]
    pub const fn semantic_action_id(&self) -> Option<&PublicId> {
        match self {
            Self::ActionInvoke { action, .. } => Some(action),
            Self::Noop | Self::ViewHandler { .. } => None,
        }
    }
}
