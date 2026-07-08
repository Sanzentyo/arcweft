use super::control_style::{
    ControlInteractionStyleState, ControlPointerStyleState, PreparedControlBackdrop,
    PreparedControlFilter, PreparedControlPaint, PreparedControlShadow, RenderControlStyle,
    control_font_family, fill_with_opacity, push_control_backdrop_plan, push_control_border,
    push_control_corner_frame, push_control_filter_plan, push_control_focus_ring,
    push_control_shadow_plan, state_from_interaction,
};
use super::{
    FramePlanError, PaintRect, Palette, PreparedSelectableTextBlock, PreparedTextInputTarget,
    RenderTextBlock, RenderTextSelectionPolicy, RenderTextSlant, RenderTextWeight, RenderViewport,
};
use crate::font_family::{font_trace_enabled, render_font_family, trace_font_debug};
use crate::font_system::{load_font_data_and_maybe_set_primary_sans, new_font_system};
use crate::text_editor_geometry::{TextEditorGeometryContext, TextEditorGeometryPump};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use arcweft_presentation::text_editor::{TextEditorError, TextEditorState};
use arcweft_presentation::text_input::{
    TextByteOffset, TextCharacterBounds, TextGeometryTransform, TextInputOptions,
    TextInputSecurityPolicy, TextInputSessionId, TextRange, TextWritingMode,
};
use arcweft_render_text::{RichTextPresentation, RichTextRange, RichTextWritingMode};
use arcweft_text_layout::{
    GlyphOrientation, GlyphVerticalForm, LaidOutGlyph, LaidOutText, LayoutPoint, LayoutRect,
    LayoutSize,
};
use glyphon::{Attrs, Buffer, FontSystem, Metrics, Shaping, Style, Weight, Wrap, fontdb};
use num_traits::ToPrimitive;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

const TEXT_INSET_X: f32 = 8.0;
const TEXT_INSET_Y: f32 = 4.0;
const CARET_WIDTH: f32 = 2.0;
const TEXT_CONTROL_LAYOUT_CACHE_LIMIT: usize = 128;
const FONT_TRACE_FONT_SYSTEM_LIMIT: usize = 8;
const FONT_TRACE_SHAPE_REQUEST_LIMIT: usize = 8;
const FONT_TRACE_LAYOUT_LIMIT: usize = 8;
const FONT_TRACE_GLYPH_SAMPLE_LIMIT: usize = 16;
const FONT_TRACE_FACE_SAMPLE_LIMIT: usize = 24;

#[derive(Clone, Debug, PartialEq)]
struct TextControlVisualLayout {
    display_value: String,
    laid_out: LaidOutText,
    text_bounds: HitRect,
    clip_bounds: HitRect,
    buffer_size: LayoutSize,
}

#[derive(Debug)]
pub(super) struct TextControlFontContext {
    font_system: FontSystem,
    layout_cache: HashMap<TextControlLayoutCacheKey, TextControlVisualLayout>,
    layout_cache_hits: u64,
    layout_cache_misses: u64,
    registered_font_bytes: usize,
    font_trace_font_system_keys: HashSet<String>,
    font_trace_shape_requests: usize,
    font_trace_layouts: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TextControlLayoutCacheKey {
    target: InteractionTarget,
    value: String,
    selection_end: u32,
    bounds_x: u32,
    bounds_y: u32,
    bounds_width: u32,
    bounds_height: u32,
    font_size: u32,
    line_height: u32,
    font_family: super::RenderFontFamily,
    multiline: bool,
    secure: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextControlDisplayText {
    value: String,
    chars: Vec<TextControlDisplayChar>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextControlDisplayChar {
    display: Range<usize>,
    source: Range<usize>,
}

impl TextControlFontContext {
    pub(super) fn new() -> Self {
        let mut context = Self {
            font_system: new_font_system(),
            layout_cache: HashMap::new(),
            layout_cache_hits: 0,
            layout_cache_misses: 0,
            registered_font_bytes: 0,
            font_trace_font_system_keys: HashSet::new(),
            font_trace_shape_requests: 0,
            font_trace_layouts: 0,
        };
        trace_text_control_font_system_once(&mut context, "init", None);
        context
    }

    pub(super) fn register_font_bytes(&mut self, bytes: Vec<u8>) -> Result<(), FramePlanError> {
        if bytes.is_empty() {
            return Err(FramePlanError::EmptyFont);
        }
        let set_primary_sans = self.registered_font_bytes == 0;
        let byte_len = bytes.len();
        self.registered_font_bytes = self.registered_font_bytes.saturating_add(bytes.len());
        let font_report = load_font_data_and_maybe_set_primary_sans(
            &mut self.font_system,
            bytes,
            set_primary_sans,
        );
        trace_font_debug(format_args!(
            "text-control-font-register bytes={byte_len} faces_before={} faces_after={} primary_sans={:?} registered_bytes={}",
            font_report.before_faces,
            font_report.after_faces,
            font_report.primary_sans_family,
            self.registered_font_bytes,
        ));
        trace_text_control_font_system_once(self, "after-register", None);
        self.layout_cache.clear();
        Ok(())
    }

    pub(super) fn stats(&self) -> super::SharedFramePlanStats {
        super::SharedFramePlanStats {
            registered_font_bytes: self.registered_font_bytes,
            text_control_layout_cache_hits: self.layout_cache_hits,
            text_control_layout_cache_misses: self.layout_cache_misses,
            text_control_layout_cache_entries: self.layout_cache.len(),
        }
    }

    fn visual_layout(
        &mut self,
        key: &TextControlLayoutCacheKey,
    ) -> Option<TextControlVisualLayout> {
        let layout = self.layout_cache.get(key).cloned();
        if layout.is_some() {
            self.layout_cache_hits = self.layout_cache_hits.saturating_add(1);
        }
        layout
    }

    fn cache_visual_layout(
        &mut self,
        key: TextControlLayoutCacheKey,
        layout: TextControlVisualLayout,
    ) {
        self.layout_cache_misses = self.layout_cache_misses.saturating_add(1);
        if self.layout_cache.len() >= TEXT_CONTROL_LAYOUT_CACHE_LIMIT {
            self.layout_cache.clear();
        }
        self.layout_cache.insert(key, layout);
    }
}

impl Default for TextControlFontContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Real text-control input lowered from runtime/product View state.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderTextInputControl {
    pub target: InteractionTarget,
    pub session: TextInputSessionId,
    pub containing_scroll_region: Option<String>,
    pub value: String,
    pub selection: TextRange<TextByteOffset>,
    pub options: TextInputOptions,
    pub role: SemanticRole,
    pub bounds: HitRect,
    pub viewport_clip: Option<HitRect>,
    pub label: Option<String>,
    pub style: RenderControlStyle,
}

impl RenderTextInputControl {
    pub fn new(
        target: InteractionTarget,
        session: TextInputSessionId,
        value: impl Into<String>,
        selection: TextRange<TextByteOffset>,
        options: TextInputOptions,
        role: SemanticRole,
        bounds: HitRect,
    ) -> Self {
        Self {
            target,
            session,
            containing_scroll_region: None,
            value: value.into(),
            selection,
            options,
            role,
            bounds,
            viewport_clip: None,
            label: None,
            style: RenderControlStyle::default(),
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_containing_scroll_region(
        mut self,
        containing_scroll_region: impl Into<String>,
    ) -> Self {
        self.containing_scroll_region = Some(containing_scroll_region.into());
        self
    }

    #[must_use]
    pub const fn with_viewport_clip(mut self, viewport_clip: HitRect) -> Self {
        self.viewport_clip = Some(viewport_clip);
        self
    }

    #[must_use]
    pub fn with_style(mut self, style: RenderControlStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    #[must_use]
    pub const fn with_selection(mut self, selection: TextRange<TextByteOffset>) -> Self {
        self.selection = selection;
        self
    }

    #[must_use]
    pub fn with_options(mut self, options: TextInputOptions) -> Self {
        self.options = options;
        self
    }

    pub fn resolved_options(&self) -> Result<TextInputOptions, FramePlanError> {
        self.role
            .text_input_options(self.options.clone())
            .ok_or(FramePlanError::InvalidTextInputRole { role: self.role })
    }
}

pub(super) fn text_input_depth_milli(
    scene: &super::RenderScene,
    control: &RenderTextInputControl,
) -> i32 {
    let state = visual_state_for_control(scene, control);
    control
        .style
        .visual_for_state(state)
        .depth_milli
        .unwrap_or_default()
}

#[expect(
    clippy::too_many_arguments,
    reason = "The geometry sinks are intentionally explicit at this renderer boundary."
)]
pub(super) fn build_text_input(
    scene: &super::RenderScene,
    layer: &LayerId,
    control: &RenderTextInputControl,
    semantics: &mut SemanticTree,
    rectangles: &mut Vec<PaintRect>,
    text: &mut Vec<RenderTextBlock>,
    palette: &Palette,
    font_context: &mut TextControlFontContext,
    control_backdrops: &mut Vec<PreparedControlBackdrop>,
    control_shadows: &mut Vec<PreparedControlShadow>,
    control_filters: &mut Vec<PreparedControlFilter>,
) -> Result<(Option<PreparedTextInputTarget>, PreparedControlPaint), FramePlanError> {
    let options = control.resolved_options()?;
    let is_focused = scene.interaction.focused.as_ref() == Some(&control.target);
    let state = visual_state_for_control(scene, control);
    let visual = control.style.visual_for_state(state);
    let radii = visual.radii();
    let visual_layout = visual_layout_for_control(control, &options, &visual, font_context);
    let visible_bounds = visible_control_bounds(control).unwrap_or(control.bounds);
    let backdrop_start = control_backdrops.len();
    push_control_backdrop_plan(control_backdrops, &control.target, visible_bounds, &visual);
    let shadow_start = control_shadows.len();
    push_control_shadow_plan(control_shadows, &control.target, visible_bounds, &visual);
    let rectangle_start = rectangles.len();
    rectangles.push(PaintRect::with_radii(
        control.bounds,
        fill_with_opacity(
            visual.fill.unwrap_or(if is_focused {
                palette.choice_active
            } else {
                palette.choice_idle
            }),
            visual.opacity,
        ),
        radii,
    ));
    push_control_border(rectangles, control.bounds, visual.border, radii);
    push_control_corner_frame(rectangles, control.bounds, visual.corner_frame);
    if is_focused {
        if let Some(ring) = visual.focus_ring {
            push_control_focus_ring(rectangles, control.bounds, ring, radii);
        } else {
            super::push_focus_ring(rectangles, control.bounds, palette.focus_ring);
        }
        push_renderer_text_input_selection(
            rectangles,
            control,
            &visual_layout,
            visual.selection.unwrap_or(palette.choice_active),
            radii,
        );
        push_renderer_text_input_caret(
            rectangles,
            control,
            &visual_layout,
            visual.caret.unwrap_or(palette.focus_ring),
            radii,
        );
    }

    let text_start = text.len();
    if let Some(clip_bounds) = clipped_viewport_bounds(visual_layout.clip_bounds, control) {
        text.push(RenderTextBlock {
            target: None,
            text: visual_layout.display_value.clone(),
            bounds: visual_layout.text_bounds,
            clip_bounds: Some(clip_bounds),
            buffer_width: Some(visual_layout.buffer_size.width),
            buffer_height: Some(visual_layout.buffer_size.height),
            font_size: text_control_font_size(control, &visual),
            line_height: text_control_line_height(control, &visual),
            font_family: control_font_family(&visual),
            weight: RenderTextWeight::Regular,
            slant: RenderTextSlant::Upright,
            rgba: visual.text.unwrap_or(palette.choice_text),
            selection_policy: RenderTextSelectionPolicy::Disabled,
            selection: None,
            selection_rgba: visual.selection.unwrap_or(palette.choice_active),
        });
    }
    let filter_start = control_filters.len();
    push_control_filter_plan(control_filters, &control.target, visible_bounds, &visual);
    apply_viewport_clip_to_rectangles(&mut rectangles[rectangle_start..], control.viewport_clip);
    let paint = PreparedControlPaint {
        target: control.target.clone(),
        bounds: visible_bounds,
        rectangle_range: rectangle_start..rectangles.len(),
        text_range: text_start..text.len(),
        backdrop_range: backdrop_start..control_backdrops.len(),
        shadow_range: shadow_start..control_shadows.len(),
        filter_range: filter_start..control_filters.len(),
    };

    let mut node = SemanticNode::new(
        layer.clone(),
        control.target.clone(),
        control.role,
        visible_bounds,
    );
    if let Some(label) = &control.label {
        node = node.with_label(label.clone());
    }
    semantics.push(node);

    let focused_target = if is_focused {
        Some(prepare_text_input_target(
            scene.viewport,
            control,
            &options,
            &visual_layout.laid_out,
        )?)
    } else {
        None
    };
    Ok((focused_target, paint))
}

pub(super) fn build_selectable_text_block(
    block: &RenderTextBlock,
    font_context: &mut TextControlFontContext,
) -> Option<(PreparedSelectableTextBlock, Vec<PaintRect>)> {
    if !block.selection_policy.enabled() {
        return None;
    }
    let target = block.target.clone()?;
    let laid_out = laid_out_text_for_text_block(block, font_context);
    let character_bounds = laid_out
        .glyphs
        .iter()
        .map(|glyph| {
            TextCharacterBounds::new(
                text_range_from_rich(glyph.range),
                layout_rect_to_hit_rect(glyph.bounds),
            )
        })
        .collect();
    let selection_rects = block.selection.map_or_else(Vec::new, |selection| {
        text_range_rects(&laid_out, selection.start().get(), selection.end().get())
            .into_iter()
            .filter_map(|bounds| clip_text_block_rect(block, bounds))
            .map(|bounds| PaintRect::new(bounds, block.selection_rgba))
            .collect()
    });
    Some((
        PreparedSelectableTextBlock {
            target,
            text: block.text.clone(),
            bounds: block.bounds,
            clip_bounds: block.clip_bounds,
            character_bounds,
        },
        selection_rects,
    ))
}

fn laid_out_text_for_text_block(
    block: &RenderTextBlock,
    font_context: &mut TextControlFontContext,
) -> LaidOutText {
    let line_height = block.line_height.max(1.0);
    if block.text.is_empty() {
        return LaidOutText {
            glyphs: vec![text_control_caret_anchor(
                RichTextRange::new(0, 0),
                block.bounds.x,
                block.bounds.y,
                line_height,
            )],
            runs: Vec::new(),
            ruby: Vec::new(),
            bounds: None,
        };
    }

    let buffer = text_block_buffer(block, font_context);
    let line_offsets = display_line_start_offsets(&block.text);
    let mut glyphs = Vec::new();
    for (run_index, run) in buffer.layout_runs().enumerate() {
        let line_start = line_offsets.get(run.line_i).copied().unwrap_or_default();
        glyphs.extend(run.glyphs.iter().map(|glyph| {
            let range = RichTextRange::new(line_start + glyph.start, line_start + glyph.end);
            LaidOutGlyph {
                run_index,
                range,
                text: block
                    .text
                    .get(range.start..range.end)
                    .unwrap_or_default()
                    .to_owned(),
                origin: LayoutPoint::new(block.bounds.x + glyph.x, block.bounds.y + run.line_top),
                advance: LayoutSize::new(glyph.w, 0.0),
                bounds: LayoutRect::new(
                    block.bounds.x + glyph.x,
                    block.bounds.y + run.line_top,
                    glyph.w,
                    run.line_height,
                ),
                writing_mode: RichTextWritingMode::HorizontalTb,
                orientation: GlyphOrientation::Upright,
                vertical_form: GlyphVerticalForm::None,
                presentation: RichTextPresentation::default(),
            }
        }));
    }
    let display_text = identity_display_text(&block.text);
    push_newline_caret_anchors(
        &mut glyphs,
        &display_text,
        &line_offsets,
        block.bounds,
        line_height,
    );
    glyphs.sort_by_key(|glyph| (glyph.range.start, glyph.range.end));
    LaidOutText {
        bounds: union_layout_bounds(glyphs.iter().map(|glyph| glyph.bounds)),
        glyphs,
        runs: Vec::new(),
        ruby: Vec::new(),
    }
}

fn text_block_buffer(block: &RenderTextBlock, font_context: &mut TextControlFontContext) -> Buffer {
    let mut attrs = Attrs::new().family(render_font_family(&block.font_family));
    if block.weight == RenderTextWeight::Bold {
        attrs = attrs.weight(Weight::BOLD);
    }
    if block.slant == RenderTextSlant::Italic {
        attrs = attrs.style(Style::Italic);
    }
    let font_system = &mut font_context.font_system;
    let mut buffer = Buffer::new(
        font_system,
        Metrics::new(block.font_size.max(1.0), block.line_height.max(1.0)),
    );
    buffer.set_size(
        font_system,
        Some(block.buffer_width.unwrap_or(block.bounds.width).max(1.0)),
        Some(block.buffer_height.unwrap_or(block.bounds.height).max(1.0)),
    );
    buffer.set_text(font_system, &block.text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);
    buffer
}

fn identity_display_text(text: &str) -> TextControlDisplayText {
    TextControlDisplayText {
        value: text.to_owned(),
        chars: text
            .char_indices()
            .map(|(start, ch)| {
                let end = start.saturating_add(ch.len_utf8());
                TextControlDisplayChar {
                    display: start..end,
                    source: start..end,
                }
            })
            .collect(),
    }
}

fn text_range_from_rich(range: RichTextRange) -> TextRange<TextByteOffset> {
    TextRange::new(
        TextByteOffset(u32::try_from(range.start).unwrap_or(u32::MAX)),
        TextByteOffset(u32::try_from(range.end).unwrap_or(u32::MAX)),
    )
}

fn clip_text_block_rect(block: &RenderTextBlock, rect: HitRect) -> Option<HitRect> {
    block
        .clip_bounds
        .map_or(Some(rect), |clip| super::intersect_hit_rect(rect, clip))
}

fn visual_state_for_control(
    scene: &super::RenderScene,
    control: &RenderTextInputControl,
) -> super::RenderControlVisualState {
    let is_focused = scene.interaction.focused.as_ref() == Some(&control.target);
    let is_hovered = scene.interaction.hovered.as_ref() == Some(&control.target);
    let is_pressed = scene.interaction.pressed.as_ref() == Some(&control.target);
    state_from_interaction(ControlInteractionStyleState {
        enabled: true,
        focused: is_focused,
        pointer: ControlPointerStyleState::from_interaction(is_hovered, is_pressed),
    })
}

fn push_renderer_text_input_selection(
    rectangles: &mut Vec<PaintRect>,
    control: &RenderTextInputControl,
    visual_layout: &TextControlVisualLayout,
    color: [f32; 4],
    radii: super::PaintRectRadii,
) {
    let start = control.selection.start().get();
    let end = control.selection.end().get();
    if start == end {
        return;
    }
    rectangles.extend(
        text_range_rects(&visual_layout.laid_out, start, end)
            .into_iter()
            .filter_map(|bounds| {
                clip_text_local_rect_to_inner(control, bounds)
                    .map(|bounds| text_local_to_viewport_rect(control, bounds))
            })
            .map(|bounds| PaintRect::new(bounds, color).clipped_to_radii(control.bounds, radii)),
    );
}

fn push_renderer_text_input_caret(
    rectangles: &mut Vec<PaintRect>,
    control: &RenderTextInputControl,
    visual_layout: &TextControlVisualLayout,
    color: [f32; 4],
    radii: super::PaintRectRadii,
) {
    let caret = control.selection.end().get();
    if let Some(bounds) = clip_text_local_rect_to_inner(
        control,
        text_caret_rect(control, &visual_layout.laid_out, caret),
    ) {
        rectangles.push(
            PaintRect::new(text_local_to_viewport_rect(control, bounds), color)
                .clipped_to_radii(control.bounds, radii),
        );
    }
}

fn text_range_rects(layout: &LaidOutText, start: u32, end: u32) -> Vec<HitRect> {
    let range = RichTextRange::new(
        usize::try_from(start.min(end)).unwrap_or(usize::MAX),
        usize::try_from(start.max(end)).unwrap_or(usize::MAX),
    );
    layout
        .glyphs
        .iter()
        .filter(|glyph| rich_ranges_overlap(glyph.range, range))
        .map(|glyph| layout_rect_to_hit_rect(glyph.bounds))
        .collect()
}

fn text_caret_rect(control: &RenderTextInputControl, layout: &LaidOutText, offset: u32) -> HitRect {
    let offset = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(control.value.len());
    if let Some(glyph) = layout.glyphs.windows(2).find_map(|pair| {
        let previous = &pair[0];
        let next = &pair[1];
        (previous.range.end == offset
            && previous.bounds.width > 0.0
            && next.range.start == offset
            && next.bounds.y > previous.bounds.y)
            .then_some(previous)
    }) {
        return HitRect::new(
            glyph.bounds.x + glyph.bounds.width,
            glyph.bounds.y,
            CARET_WIDTH,
            glyph.bounds.height.max(1.0),
        );
    }
    if let Some(glyph) = layout
        .glyphs
        .iter()
        .find(|glyph| offset <= glyph.range.start)
    {
        return HitRect::new(
            glyph.bounds.x,
            glyph.bounds.y,
            CARET_WIDTH,
            glyph.bounds.height.max(1.0),
        );
    }
    layout.glyphs.last().map_or_else(
        || {
            let inner = text_local_inner_bounds(control);
            HitRect::new(
                inner.x,
                inner.y,
                CARET_WIDTH,
                text_control_line_height(control, &super::RenderControlVisualStyle::default()),
            )
        },
        |glyph| {
            HitRect::new(
                glyph.bounds.x + glyph.bounds.width,
                glyph.bounds.y,
                CARET_WIDTH,
                glyph.bounds.height.max(1.0),
            )
        },
    )
}

fn text_inner_bounds(control: &RenderTextInputControl) -> HitRect {
    let inner = text_local_inner_bounds(control);
    HitRect::new(
        control.bounds.x + inner.x,
        control.bounds.y + inner.y,
        inner.width,
        inner.height,
    )
}

fn text_local_inner_bounds(control: &RenderTextInputControl) -> HitRect {
    HitRect::new(
        TEXT_INSET_X,
        TEXT_INSET_Y,
        (control.bounds.width - TEXT_INSET_X * 2.0).max(0.0),
        (control.bounds.height - TEXT_INSET_Y * 2.0).max(1.0),
    )
}

fn text_local_to_viewport_rect(control: &RenderTextInputControl, rect: HitRect) -> HitRect {
    HitRect::new(
        control.bounds.x + rect.x,
        control.bounds.y + rect.y,
        rect.width,
        rect.height,
    )
}

fn clip_text_local_rect_to_inner(
    control: &RenderTextInputControl,
    rect: HitRect,
) -> Option<HitRect> {
    super::intersect_hit_rect(rect, text_local_inner_bounds(control))
}

fn visible_control_bounds(control: &RenderTextInputControl) -> Option<HitRect> {
    control.viewport_clip.map_or(Some(control.bounds), |clip| {
        super::intersect_hit_rect(control.bounds, clip)
    })
}

fn clipped_viewport_bounds(bounds: HitRect, control: &RenderTextInputControl) -> Option<HitRect> {
    control
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

fn text_control_font_size(
    control: &RenderTextInputControl,
    visual: &super::RenderControlVisualStyle,
) -> f32 {
    visual
        .font_size_px
        .unwrap_or_else(|| (control.bounds.height * 0.55).clamp(12.0, 28.0))
        .max(1.0)
}

fn text_control_line_height(
    control: &RenderTextInputControl,
    visual: &super::RenderControlVisualStyle,
) -> f32 {
    let inner_height = (control.bounds.height - TEXT_INSET_Y * 2.0).max(1.0);
    visual
        .line_height_px
        .unwrap_or_else(|| text_control_font_size(control, visual) * 1.25)
        .max(1.0)
        .min(inner_height)
}

fn prepare_text_input_target(
    viewport: RenderViewport,
    control: &RenderTextInputControl,
    options: &TextInputOptions,
    laid_out: &LaidOutText,
) -> Result<PreparedTextInputTarget, FramePlanError> {
    let editor = TextEditorState::from_text_control(
        control.session,
        control.target.clone(),
        control.value.clone(),
        control.selection,
        options.clone(),
    )?;
    let scale_factor = viewport.scale_factor.to_f32().unwrap_or(f32::MAX);
    let layout = TextEditorGeometryPump::layout_from_laid_out_text(
        editor.text(),
        laid_out,
        TextEditorGeometryContext::default()
            .with_text_local_control_rect(HitRect::new(
                0.0,
                0.0,
                control.bounds.width,
                control.bounds.height,
            ))
            .with_text_local_to_viewport(TextGeometryTransform::translation(
                control.bounds.x,
                control.bounds.y,
            ))
            .with_viewport_to_screen(TextGeometryTransform::scale(scale_factor, scale_factor))
            .with_writing_mode(TextWritingMode::HorizontalTb),
    )
    .map_err(TextEditorError::from)?;
    let (snapshot, geometry) = editor.snapshots(&layout)?.into_parts();
    let security = TextInputSecurityPolicy::from_options(options);
    Ok(PreparedTextInputTarget {
        snapshot: security.redact_snapshot(&snapshot),
        geometry: security.redact_geometry(&geometry),
    })
}

fn visual_layout_for_control(
    control: &RenderTextInputControl,
    options: &TextInputOptions,
    visual: &super::RenderControlVisualStyle,
    font_context: &mut TextControlFontContext,
) -> TextControlVisualLayout {
    let display_text = display_text_for_control(control, options);
    let key = TextControlLayoutCacheKey::new(control, options, visual);
    if let Some(layout) = font_context.visual_layout(&key) {
        return layout;
    }
    let unscrolled =
        laid_out_text_for_control(control, options, visual, &display_text, font_context);
    let inner = text_local_inner_bounds(control);
    let content_size = text_control_content_size(&unscrolled, inner);
    let caret = text_caret_rect(control, &unscrolled, control.selection.end().get());
    let scroll = text_control_scroll_offset(options, inner, content_size, caret);
    let laid_out = scroll_laid_out_text(unscrolled, scroll);
    let buffer_size = if options.is_multiline() {
        LayoutSize::new(inner.width.max(1.0), content_size.height.max(inner.height))
    } else {
        LayoutSize::new(content_size.width.max(inner.width), inner.height.max(1.0))
    };
    let layout = TextControlVisualLayout {
        display_value: display_text.value,
        laid_out,
        text_bounds: HitRect::new(
            control.bounds.x + inner.x - scroll.x,
            control.bounds.y + inner.y - scroll.y,
            buffer_size.width,
            buffer_size.height,
        ),
        clip_bounds: text_inner_bounds(control),
        buffer_size,
    };
    font_context.cache_visual_layout(key, layout.clone());
    layout
}

fn display_text_for_control(
    control: &RenderTextInputControl,
    options: &TextInputOptions,
) -> TextControlDisplayText {
    let mut value = String::with_capacity(control.value.len());
    let mut chars = Vec::new();
    for (source_start, ch) in control.value.char_indices() {
        let source_end = source_start.saturating_add(ch.len_utf8());
        let visual = visual_text_input_char(ch, options);
        let display_start = value.len();
        value.push(visual);
        let display_end = value.len();
        chars.push(TextControlDisplayChar {
            display: display_start..display_end,
            source: source_start..source_end,
        });
    }
    TextControlDisplayText { value, chars }
}

fn visual_text_input_char(ch: char, options: &TextInputOptions) -> char {
    if options.is_secure() {
        '*'
    } else if ch == '\n' && !options.is_multiline() {
        ' '
    } else {
        ch
    }
}

fn laid_out_text_for_control(
    control: &RenderTextInputControl,
    options: &TextInputOptions,
    visual: &super::RenderControlVisualStyle,
    display_text: &TextControlDisplayText,
    font_context: &mut TextControlFontContext,
) -> LaidOutText {
    let inner = text_local_inner_bounds(control);
    let line_height = text_control_line_height(control, visual);
    let mut glyphs = Vec::new();
    if control.value.is_empty() {
        glyphs.push(empty_text_control_caret_anchor(inner, line_height));
        return LaidOutText {
            glyphs,
            runs: Vec::new(),
            ruby: Vec::new(),
            bounds: None,
        };
    }

    let buffer = text_control_buffer(control, options, visual, display_text, font_context);
    let line_offsets = display_line_start_offsets(&display_text.value);
    for (run_index, run) in buffer.layout_runs().enumerate() {
        let line_start = line_offsets.get(run.line_i).copied().unwrap_or_default();
        glyphs.extend(run.glyphs.iter().map(|glyph| {
            let display_range = line_start + glyph.start..line_start + glyph.end;
            let source_range = source_range_for_display_range(display_text, display_range.clone());
            LaidOutGlyph {
                run_index,
                range: RichTextRange::new(source_range.start, source_range.end),
                text: display_text
                    .value
                    .get(display_range)
                    .unwrap_or_default()
                    .to_owned(),
                origin: LayoutPoint::new(inner.x + glyph.x, inner.y + run.line_top),
                advance: LayoutSize::new(glyph.w, 0.0),
                bounds: LayoutRect::new(
                    inner.x + glyph.x,
                    inner.y + run.line_top,
                    glyph.w,
                    run.line_height,
                ),
                writing_mode: RichTextWritingMode::HorizontalTb,
                orientation: GlyphOrientation::Upright,
                vertical_form: GlyphVerticalForm::None,
                presentation: RichTextPresentation::default(),
            }
        }));
    }
    push_newline_caret_anchors(&mut glyphs, display_text, &line_offsets, inner, line_height);
    glyphs.sort_by_key(|glyph| (glyph.range.start, glyph.range.end));
    LaidOutText {
        glyphs,
        runs: Vec::new(),
        ruby: Vec::new(),
        bounds: None,
    }
}

fn text_control_buffer(
    control: &RenderTextInputControl,
    options: &TextInputOptions,
    visual: &super::RenderControlVisualStyle,
    display_text: &TextControlDisplayText,
    font_context: &mut TextControlFontContext,
) -> Buffer {
    let font_size = text_control_font_size(control, visual);
    let line_height = text_control_line_height(control, visual);
    let inner = text_local_inner_bounds(control);
    let font_family = control_font_family(visual);
    trace_text_control_shape_request(font_context, control, options, &font_family, display_text);
    trace_text_control_font_system_once(font_context, "before-shape", Some(&font_family));
    let attrs = Attrs::new().family(render_font_family(&font_family));
    let buffer = {
        let font_system = &mut font_context.font_system;
        let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
        if options.is_multiline() {
            buffer.set_wrap(font_system, Wrap::WordOrGlyph);
            buffer.set_size(font_system, Some(inner.width.max(1.0)), None);
        } else {
            buffer.set_wrap(font_system, Wrap::None);
            buffer.set_size(font_system, None, Some(inner.height.max(1.0)));
        }
        buffer.set_text(
            font_system,
            &display_text.value,
            &attrs,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(font_system, false);
        buffer
    };
    trace_text_control_layout_runs(font_context, &buffer);
    buffer
}

fn trace_text_control_shape_request(
    font_context: &mut TextControlFontContext,
    control: &RenderTextInputControl,
    options: &TextInputOptions,
    font_family: &super::RenderFontFamily,
    display_text: &TextControlDisplayText,
) {
    if !font_trace_enabled() {
        return;
    }
    if font_context.font_trace_shape_requests >= FONT_TRACE_SHAPE_REQUEST_LIMIT {
        return;
    }
    font_context.font_trace_shape_requests += 1;
    let text_probe = if options.is_secure() {
        "<secure>".to_owned()
    } else {
        display_text
            .value
            .chars()
            .take(16)
            .map(|ch| format!("U+{:04X}", u32::from(ch)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    trace_font_debug(format_args!(
        "text-control-shape-request target={:?} multiline={} secure={} chars={} font_family={:?} codepoints={}",
        control.target,
        options.is_multiline(),
        options.is_secure(),
        display_text.value.chars().count(),
        font_family,
        text_probe
    ));
}

fn trace_text_control_font_system_once(
    font_context: &mut TextControlFontContext,
    stage: &str,
    selected_family: Option<&super::RenderFontFamily>,
) {
    if !font_trace_enabled() {
        return;
    }
    let key = format!(
        "{stage}|registered={}|selected={selected_family:?}",
        font_context.registered_font_bytes
    );
    if !font_context.font_trace_font_system_keys.contains(&key)
        && font_context.font_trace_font_system_keys.len() >= FONT_TRACE_FONT_SYSTEM_LIMIT
    {
        return;
    }
    if !font_context.font_trace_font_system_keys.insert(key) {
        return;
    }
    let font_system = &font_context.font_system;
    let total_faces = font_system.db().faces().count();
    let interesting_faces = font_system
        .db()
        .faces()
        .filter(|face| interesting_font_face(face))
        .take(FONT_TRACE_FACE_SAMPLE_LIMIT)
        .map(trace_face_label)
        .collect::<Vec<_>>()
        .join(" || ");
    trace_font_debug(format_args!(
        "text-control-font-system stage={stage} locale={} total_faces={total_faces} registered_bytes={} selected_family={selected_family:?} interesting_faces=[{interesting_faces}]",
        font_system.locale(),
        font_context.registered_font_bytes
    ));
}

fn interesting_font_face(face: &fontdb::FaceInfo) -> bool {
    face.families.iter().any(|(family, _)| {
        [
            "Arcweft Demo",
            "Yu Gothic",
            "Yu Gothic View",
            "Meiryo",
            "Meiryo View",
            "MS Gothic",
            "MS PGothic",
            "Noto Sans JP",
            "Noto Sans CJK JP",
            "Microsoft YaHei",
            "Microsoft YaHei View",
            "SimSun",
            "NSimSun",
            "MingLiU",
        ]
        .into_iter()
        .any(|candidate| family.eq_ignore_ascii_case(candidate))
    })
}

fn trace_face_label(face: &fontdb::FaceInfo) -> String {
    let families = face
        .families
        .iter()
        .take(3)
        .map(|(name, language)| format!("{name}:{language:?}"))
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "id={:?} index={} families={} postscript={} weight={} style={:?} source={}",
        face.id,
        face.index,
        families,
        face.post_script_name,
        face.weight.0,
        face.style,
        trace_font_source(&face.source)
    )
}

fn trace_font_source(source: &fontdb::Source) -> String {
    match source {
        fontdb::Source::Binary(bytes) => {
            format!("binary:{} bytes", bytes.as_ref().as_ref().len())
        }
        fontdb::Source::File(path) => format!("file:{}", path.display()),
        fontdb::Source::SharedFile(path, bytes) => {
            format!(
                "shared-file:{}:{} bytes",
                path.display(),
                bytes.as_ref().as_ref().len()
            )
        }
    }
}

fn trace_text_control_layout_runs(font_context: &mut TextControlFontContext, buffer: &Buffer) {
    if !font_trace_enabled() {
        return;
    }
    if font_context.font_trace_layouts >= FONT_TRACE_LAYOUT_LIMIT {
        return;
    }
    font_context.font_trace_layouts += 1;

    let mut font_ids = Vec::new();
    let mut glyph_samples = Vec::new();
    let mut glyph_count = 0usize;
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            glyph_count = glyph_count.saturating_add(1);
            let physical = glyph.physical((0.0, 0.0), 1.0);
            let font_id = format!("{:?}", physical.cache_key.font_id);
            if !font_ids.iter().any(|existing| existing == &font_id) {
                font_ids.push(font_id.clone());
            }
            if glyph_samples.len() < FONT_TRACE_GLYPH_SAMPLE_LIMIT {
                glyph_samples.push(format!(
                    "line={} cluster={} x={} y={} w={} font={} glyph={}",
                    run.line_i,
                    glyph.start,
                    physical.x,
                    physical.y,
                    glyph.w,
                    font_id,
                    physical.cache_key.glyph_id
                ));
            }
        }
    }
    let used_faces = trace_faces_for_font_ids(&font_context.font_system, &font_ids);
    trace_font_debug(format_args!(
        "text-control-layout-runs glyph_count={glyph_count} font_ids=[{}] samples=[{}] used_faces=[{}]",
        font_ids.join(", "),
        glyph_samples.join(" | "),
        used_faces
    ));
}

fn trace_faces_for_font_ids(font_system: &FontSystem, font_ids: &[String]) -> String {
    if font_ids.is_empty() {
        return String::new();
    }
    font_system
        .db()
        .faces()
        .filter(|face| {
            let face_id = format!("{:?}", face.id);
            font_ids.iter().any(|font_id| font_id == &face_id)
        })
        .map(trace_face_label)
        .collect::<Vec<_>>()
        .join(" || ")
}

fn display_line_start_offsets(value: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    offsets.extend(
        value
            .char_indices()
            .filter_map(|(index, ch)| (ch == '\n').then_some(index + ch.len_utf8())),
    );
    offsets
}

fn source_range_for_display_range(
    display_text: &TextControlDisplayText,
    display_range: Range<usize>,
) -> Range<usize> {
    let mut source_start = None;
    let mut source_end = None;
    for item in &display_text.chars {
        if item.display.end <= display_range.start {
            continue;
        }
        if item.display.start >= display_range.end {
            break;
        }
        source_start.get_or_insert(item.source.start);
        source_end = Some(item.source.end);
    }
    source_start.map_or_else(
        || {
            let offset = source_offset_for_display_offset(display_text, display_range.start);
            offset..offset
        },
        |start| start..source_end.unwrap_or(start),
    )
}

fn source_offset_for_display_offset(
    display_text: &TextControlDisplayText,
    display_offset: usize,
) -> usize {
    for item in &display_text.chars {
        if display_offset <= item.display.start {
            return item.source.start;
        }
        if display_offset < item.display.end {
            return item.source.end;
        }
    }
    display_text.chars.last().map_or(0, |item| item.source.end)
}

fn push_newline_caret_anchors(
    glyphs: &mut Vec<LaidOutGlyph>,
    display_text: &TextControlDisplayText,
    line_offsets: &[usize],
    inner: HitRect,
    line_height: f32,
) {
    for item in display_text
        .chars
        .iter()
        .filter(|item| display_text.value.get(item.display.clone()) == Some("\n"))
    {
        let next_line = line_offsets
            .iter()
            .position(|offset| *offset == item.display.end)
            .and_then(|line| line.to_f32())
            .unwrap_or_default();
        let y = inner.y + next_line * line_height;
        glyphs.push(text_control_caret_anchor(
            RichTextRange::new(item.source.end, item.source.end),
            inner.x,
            y,
            line_height,
        ));
    }
}

fn empty_text_control_caret_anchor(inner: HitRect, line_height: f32) -> LaidOutGlyph {
    text_control_caret_anchor(RichTextRange::new(0, 0), inner.x, inner.y, line_height)
}

fn text_control_caret_anchor(
    range: RichTextRange,
    x: f32,
    y: f32,
    line_height: f32,
) -> LaidOutGlyph {
    // Soft wraps have no source byte of their own, so a zero-width glyph keeps
    // caret painting and platform IME geometry on the same visual line start.
    LaidOutGlyph {
        run_index: 0,
        range,
        text: String::new(),
        origin: LayoutPoint::new(x, y),
        advance: LayoutSize::new(0.0, 0.0),
        bounds: LayoutRect::new(x, y, 0.0, line_height),
        writing_mode: RichTextWritingMode::HorizontalTb,
        orientation: GlyphOrientation::Upright,
        vertical_form: GlyphVerticalForm::None,
        presentation: RichTextPresentation::default(),
    }
}

fn text_control_content_size(layout: &LaidOutText, inner: HitRect) -> LayoutSize {
    let right = layout
        .glyphs
        .iter()
        .map(|glyph| glyph.bounds.x + glyph.bounds.width)
        .fold(inner.x, f32::max);
    let bottom = layout
        .glyphs
        .iter()
        .map(|glyph| glyph.bounds.y + glyph.bounds.height)
        .fold(inner.y + 1.0, f32::max);
    LayoutSize::new((right - inner.x).max(1.0), (bottom - inner.y).max(1.0))
}

fn text_control_scroll_offset(
    options: &TextInputOptions,
    inner: HitRect,
    content_size: LayoutSize,
    caret: HitRect,
) -> LayoutPoint {
    if options.is_multiline() {
        let max_y = (content_size.height - inner.height).max(0.0);
        let caret_bottom = caret.y + caret.height;
        let y = if caret_bottom > inner.y + inner.height {
            (caret_bottom - (inner.y + inner.height)).min(max_y)
        } else if caret.y < inner.y {
            (caret.y - inner.y).max(0.0)
        } else {
            0.0
        };
        LayoutPoint::new(0.0, y)
    } else {
        let max_x = (content_size.width - inner.width).max(0.0);
        let caret_right = caret.x + caret.width;
        let x = if caret_right > inner.x + inner.width {
            (caret_right - (inner.x + inner.width)).min(max_x)
        } else if caret.x < inner.x {
            (caret.x - inner.x).max(0.0)
        } else {
            0.0
        };
        LayoutPoint::new(x, 0.0)
    }
}

fn scroll_laid_out_text(mut layout: LaidOutText, scroll: LayoutPoint) -> LaidOutText {
    for glyph in &mut layout.glyphs {
        glyph.origin.x -= scroll.x;
        glyph.origin.y -= scroll.y;
        glyph.bounds.x -= scroll.x;
        glyph.bounds.y -= scroll.y;
    }
    layout.bounds = union_layout_bounds(layout.glyphs.iter().map(|glyph| glyph.bounds));
    layout
}

impl TextControlLayoutCacheKey {
    fn new(
        control: &RenderTextInputControl,
        options: &TextInputOptions,
        visual: &super::RenderControlVisualStyle,
    ) -> Self {
        Self {
            target: control.target.clone(),
            value: control.value.clone(),
            selection_end: control.selection.end().get(),
            bounds_x: f32_cache_key(control.bounds.x),
            bounds_y: f32_cache_key(control.bounds.y),
            bounds_width: f32_cache_key(control.bounds.width),
            bounds_height: f32_cache_key(control.bounds.height),
            font_size: f32_cache_key(text_control_font_size(control, visual)),
            line_height: f32_cache_key(text_control_line_height(control, visual)),
            font_family: control_font_family(visual),
            multiline: options.is_multiline(),
            secure: options.is_secure(),
        }
    }
}

fn f32_cache_key(value: f32) -> u32 {
    if value == 0.0 {
        0.0_f32.to_bits()
    } else {
        value.to_bits()
    }
}

fn union_layout_bounds(mut bounds: impl Iterator<Item = LayoutRect>) -> Option<LayoutRect> {
    let first = bounds.next()?;
    Some(bounds.fold(first, LayoutRect::union))
}

fn layout_rect_to_hit_rect(rect: LayoutRect) -> HitRect {
    HitRect::new(rect.x, rect.y, rect.width, rect.height)
}

fn rich_ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{
        ChoiceScroll, InteractionVisualState, RenderControlShadow, RenderControlShadowKind,
        RenderControlVisualStyle, RenderPreferences, RenderScene,
    };
    use arcweft_id::PublicId;

    fn target() -> InteractionTarget {
        InteractionTarget::new(PublicId::try_new("input.test").unwrap())
    }

    fn control(value: &str, selection: u32, height: f32) -> RenderTextInputControl {
        RenderTextInputControl::new(
            target(),
            TextInputSessionId(1),
            value,
            TextRange::new(TextByteOffset(selection), TextByteOffset(selection)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(40.0, 30.0, 420.0, height),
        )
    }

    fn narrow_multiline_control(value: &str, selection: u32) -> RenderTextInputControl {
        RenderTextInputControl::new(
            target(),
            TextInputSessionId(1),
            value,
            TextRange::new(TextByteOffset(selection), TextByteOffset(selection)),
            TextInputOptions::default().multiline(true),
            SemanticRole::TextArea,
            HitRect::new(40.0, 30.0, 64.0, 136.0),
        )
    }

    fn assert_f32_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "expected {actual} to equal {expected}"
        );
    }

    fn laid_out_for_test(control: &RenderTextInputControl) -> LaidOutText {
        let mut font_context = TextControlFontContext::new();
        laid_out_text_for_control(
            control,
            &control.options,
            &RenderControlVisualStyle::default(),
            &display_text_for_control(control, &control.options),
            &mut font_context,
        )
    }

    fn visual_layout_for_test(control: &RenderTextInputControl) -> TextControlVisualLayout {
        let mut font_context = TextControlFontContext::new();
        visual_layout_for_control(
            control,
            &control.options,
            &RenderControlVisualStyle::default(),
            &mut font_context,
        )
    }

    fn scene_with_control(control: RenderTextInputControl) -> RenderScene {
        RenderScene {
            dialogue: None,
            choices: Vec::new(),
            text_inputs: vec![control],
            action_buttons: Vec::new(),
            focus_groups: Vec::new(),
            focus_navigation: Vec::new(),
            images: Vec::new(),
            viewport: RenderViewport {
                logical_width: 800.0,
                logical_height: 450.0,
                physical_width: 800,
                physical_height: 450,
                scale_factor: 1.0,
            },
            visual_time_millis: 0,
            preferences: RenderPreferences::default(),
            interaction: InteractionVisualState::default(),
            choice_scroll: ChoiceScroll::default(),
            scroll_regions: Vec::new(),
        }
    }

    #[test]
    fn single_line_caret_uses_font_size_not_full_line_box_width() {
        let control = control("Tokyo", 5, 48.0);
        let layout = laid_out_for_test(&control);
        let caret = text_local_to_viewport_rect(&control, text_caret_rect(&control, &layout, 5));

        assert!(caret.x > 110.0);
        assert!(
            caret.x < 140.0,
            "caret should stay near rendered Latin text, got {}",
            caret.x
        );
    }

    #[test]
    fn single_line_caret_uses_shaped_width_for_narrow_latin_glyphs() {
        let narrow = control("llllllll", 8, 48.0);
        let wide = control("aaaaaaaa", 8, 48.0);
        let narrow_layout = laid_out_for_test(&narrow);
        let wide_layout = laid_out_for_test(&wide);
        let narrow_caret =
            text_local_to_viewport_rect(&narrow, text_caret_rect(&narrow, &narrow_layout, 8));
        let wide_caret =
            text_local_to_viewport_rect(&wide, text_caret_rect(&wide, &wide_layout, 8));

        assert!(
            narrow_caret.x + 20.0 < wide_caret.x,
            "`l` caret should follow shaped glyph advances instead of fixed ASCII width: narrow={narrow_caret:?}, wide={wide_caret:?}"
        );
    }

    #[test]
    fn multiline_caret_moves_to_following_visual_line_after_newline() {
        let value = "line one\nTokyo";
        let caret_offset = u32::try_from(value.len()).unwrap();
        let control = control(value, caret_offset, 136.0)
            .with_options(TextInputOptions::default().multiline(true));
        let layout = laid_out_for_test(&control);
        let first = text_local_to_viewport_rect(&control, text_caret_rect(&control, &layout, 0));
        let caret =
            text_local_to_viewport_rect(&control, text_caret_rect(&control, &layout, caret_offset));

        assert!(
            caret.y > first.y + 20.0,
            "caret did not move down: {caret:?}"
        );
        assert!(
            caret.x < first.x + 120.0,
            "caret should be relative to the second line, got {caret:?}"
        );
    }

    #[test]
    fn consecutive_empty_lines_have_distinct_caret_rows() {
        let value = "a\n\n\nb";
        let control =
            control(value, 0, 136.0).with_options(TextInputOptions::default().multiline(true));
        let layout = laid_out_for_test(&control);
        let inner = text_local_inner_bounds(&control);
        let line_height = text_control_line_height(
            &control,
            &crate::geometry::RenderControlVisualStyle::default(),
        );
        let after_first_newline = text_caret_rect(&control, &layout, 2);
        let after_second_newline = text_caret_rect(&control, &layout, 3);
        let after_third_newline = text_caret_rect(&control, &layout, 4);

        assert_f32_near(after_first_newline.x, inner.x);
        assert_f32_near(after_second_newline.x, inner.x);
        assert_f32_near(after_third_newline.x, inner.x);
        assert_f32_near(after_first_newline.y, inner.y + line_height);
        assert_f32_near(after_second_newline.y, inner.y + line_height * 2.0);
        assert_f32_near(after_third_newline.y, inner.y + line_height * 3.0);
    }

    #[test]
    fn soft_wrap_boundary_uses_previous_visual_line_end() {
        let wrap_offset = 3_u32;
        let wrap_byte_offset = usize::try_from(wrap_offset).unwrap();
        let control = narrow_multiline_control("abcdef", wrap_offset);
        let visual = visual_layout_for_test(&control);
        let previous = visual
            .laid_out
            .glyphs
            .windows(2)
            .find_map(|pair| {
                let previous = &pair[0];
                let next = &pair[1];
                (previous.range.end == wrap_byte_offset
                    && next.range.start == wrap_byte_offset
                    && next.bounds.y > previous.bounds.y)
                    .then_some(previous)
            })
            .expect("test text should soft-wrap at the third byte");
        let caret = text_caret_rect(&control, &visual.laid_out, wrap_offset);

        assert_f32_near(caret.x, previous.bounds.x + previous.bounds.width);
        assert_f32_near(caret.y, previous.bounds.y);
    }

    #[test]
    fn soft_wrap_boundary_ime_geometry_matches_renderer_caret() {
        let wrap_offset = 3_u32;
        let control = narrow_multiline_control("abcdef", wrap_offset);
        let visual = visual_layout_for_test(&control);
        let target = prepare_text_input_target(
            RenderViewport {
                logical_width: 800.0,
                logical_height: 450.0,
                physical_width: 800,
                physical_height: 450,
                scale_factor: 1.0,
            },
            &control,
            &control.options,
            &visual.laid_out,
        )
        .unwrap();
        let expected = text_local_to_viewport_rect(
            &control,
            text_caret_rect(&control, &visual.laid_out, wrap_offset),
        );
        let caret = target.geometry.viewport_caret_rect();

        assert_f32_near(caret.x, expected.x);
        assert_f32_near(caret.y, expected.y);
    }

    #[test]
    fn text_area_inner_bounds_allow_multiple_lines() {
        let control = control("line one\nTokyo", 14, 136.0);
        let inner = text_inner_bounds(&control);

        assert!(
            inner.height
                > text_control_line_height(
                    &control,
                    &crate::geometry::RenderControlVisualStyle::default(),
                ) * 2.0
        );
    }

    #[test]
    fn empty_text_field_caret_stays_at_visible_text_origin() {
        let control = control("", 0, 48.0);
        let visual = visual_layout_for_test(&control);
        let caret =
            text_local_to_viewport_rect(&control, text_caret_rect(&control, &visual.laid_out, 0));
        let expected = text_inner_bounds(&control);

        assert_eq!(visual.display_value, "");
        assert_f32_near(caret.x, expected.x);
        assert_f32_near(caret.y, expected.y);
    }

    #[test]
    fn empty_text_field_ime_geometry_uses_same_visible_origin() {
        let control = control("", 0, 48.0);
        let visual = visual_layout_for_test(&control);
        let target = prepare_text_input_target(
            RenderViewport {
                logical_width: 800.0,
                logical_height: 450.0,
                physical_width: 800,
                physical_height: 450,
                scale_factor: 1.0,
            },
            &control,
            &control.options,
            &visual.laid_out,
        )
        .unwrap();
        let expected = text_inner_bounds(&control);
        let caret = target.geometry.viewport_caret_rect();

        assert_f32_near(caret.x, expected.x);
        assert_f32_near(caret.y, expected.y);
    }

    #[test]
    fn text_control_paint_records_shadow_range() {
        let mut style = RenderControlStyle::default();
        style.normal.shadows.push(RenderControlShadow {
            offset_x_px: 0.0,
            offset_y_px: 8.0,
            blur_radius_px: 18.0,
            spread_radius_px: 0.0,
            border_radius_px: 8.0,
            color: [0, 0, 0, 128],
            kind: RenderControlShadowKind::Outer,
        });
        let control = control("shadow", 6, 48.0).with_style(style);
        let scene = scene_with_control(control.clone());
        let mut semantics = SemanticTree::default();
        let mut rectangles = Vec::new();
        let mut text = Vec::new();
        let mut backdrops = Vec::new();
        let mut shadows = Vec::new();
        let mut filters = Vec::new();
        let mut font_context = TextControlFontContext::new();

        let (_, paint) = build_text_input(
            &scene,
            &LayerId::new(PublicId::try_new("layer.test").unwrap()),
            &control,
            &mut semantics,
            &mut rectangles,
            &mut text,
            &Palette::from_preferences(RenderPreferences::default()),
            &mut font_context,
            &mut backdrops,
            &mut shadows,
            &mut filters,
        )
        .expect("text input builds");

        assert_eq!(paint.shadow_range, 0..1);
        assert_eq!(shadows.len(), 1);
    }

    #[test]
    fn secure_caret_uses_masked_visual_widths() {
        let value = "あい";
        let control = control(value, u32::try_from(value.len()).unwrap(), 48.0)
            .with_options(TextInputOptions::default().secure(true));
        let visual = visual_layout_for_test(&control);
        let caret = text_local_to_viewport_rect(
            &control,
            text_caret_rect(&control, &visual.laid_out, control.selection.end().get()),
        );

        assert_eq!(visual.display_value, "**");
        assert!(
            caret.x < 90.0,
            "secure caret should follow displayed mask glyphs, got {caret:?}"
        );

        let plain = self::control(value, u32::try_from(value.len()).unwrap(), 48.0);
        let plain_visual = visual_layout_for_test(&plain);
        let plain_caret = text_local_to_viewport_rect(
            &plain,
            text_caret_rect(&plain, &plain_visual.laid_out, plain.selection.end().get()),
        );
        assert!(
            caret.x + 10.0 < plain_caret.x,
            "secure caret should use the displayed `**` width, not the hidden source glyph width"
        );
    }

    #[test]
    fn multiline_text_control_wraps_and_scrolls_to_keep_caret_visible() {
        let value = "abcdefghijklmnopqrstuvwxyz0123456789";
        let control = RenderTextInputControl::new(
            target(),
            TextInputSessionId(1),
            value,
            TextRange::new(
                TextByteOffset(u32::try_from(value.len()).unwrap()),
                TextByteOffset(u32::try_from(value.len()).unwrap()),
            ),
            TextInputOptions::default().multiline(true),
            SemanticRole::TextArea,
            HitRect::new(40.0, 30.0, 110.0, 58.0),
        );
        let visual = visual_layout_for_test(&control);
        let first = visual.laid_out.glyphs.first().unwrap();
        let last = visual.laid_out.glyphs.last().unwrap();
        let caret = text_caret_rect(&control, &visual.laid_out, control.selection.end().get());
        let inner = text_local_inner_bounds(&control);

        assert!(
            last.bounds.y >= first.bounds.y,
            "long textarea text should wrap instead of overflowing horizontally"
        );
        assert!(
            caret.y + caret.height <= inner.y + inner.height,
            "textarea scroll should keep caret inside the visible control: {caret:?}"
        );
    }
}
