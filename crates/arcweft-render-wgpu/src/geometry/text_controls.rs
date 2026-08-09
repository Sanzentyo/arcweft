use super::control_style::{
    PreparedControlBackdrop, PreparedControlFilter, PreparedControlPaint, PreparedControlShadow,
    RenderControlVisualStyle, control_font_families, control_text_weight, fill_with_opacity,
    push_control_backdrop_plan, push_control_border, push_control_corner_frame,
    push_control_filter_plan, push_control_focus_ring, push_control_shadow_plan,
};
use super::{
    FramePlanError, PaintRect, Palette, PlannedFrameText, PlannedTextOwner,
    PreparedFrameViewportMapping, PreparedTextInputTarget, RenderViewport,
};
use crate::text_editor_geometry::{TextEditorGeometryContext, TextEditorGeometryPump};
use arcweft_glyphon::{
    GlyphonTextEngine, PreparedTextItem, TextCaretPaint, TextCompositionUnderline,
    TextInteractionPlan, TextPaintPlan,
};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use arcweft_presentation::text_editor::{TextEditorError, TextEditorState};
use arcweft_presentation::text_input::{
    TextByteOffset, TextGeometryTransform, TextInputOptions, TextInputSecurityPolicy,
    TextInputSessionId, TextRange, TextWritingMode,
};
use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRun, ResolvedTextRunSource, ResolvedTextStyle, TextColor,
    TextDocumentRevision, TextSlant, TextWeight,
};
use arcweft_text_layout::{
    HorizontalWrap, LayoutPoint, LayoutRect, LayoutSize, TextLayout, TextLayoutGlyphSource,
    TextLayoutRequest, TextLayoutSourceMap, layout_document,
};
use arcweft_text_model::{RichTextPresentation, RichTextRange};

const TEXT_INSET_X: f32 = 8.0;
const TEXT_INSET_Y: f32 = 4.0;
const CARET_WIDTH: f32 = 2.0;

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
    pub style: RenderControlVisualStyle,
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
            style: RenderControlVisualStyle::default(),
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
    pub fn with_style(mut self, style: RenderControlVisualStyle) -> Self {
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

pub(super) struct PlannedTextInput {
    control: RenderTextInputControl,
    options: TextInputOptions,
    style: ResolvedTextStyle,
    selection_rgba: [f32; 4],
    caret: TextColor,
    composition_underline: TextColor,
    owner: PlannedTextOwner,
    focused: bool,
}

pub(super) struct PreparedPlannedTextInput {
    pub(super) item: PreparedTextItem,
    pub(super) owner: PlannedTextOwner,
    pub(super) focused_target: Option<PreparedTextInputTarget>,
}

impl PlannedTextInput {
    pub(super) fn mapped(
        mut self,
        mapping: PreparedFrameViewportMapping,
    ) -> Result<Self, FramePlanError> {
        self.control.bounds = mapping.rect(self.control.bounds);
        self.control.viewport_clip = self.control.viewport_clip.map(|clip| mapping.rect(clip));
        self.style = super::mapped_text_style(self.style, mapping.text_scale)?;
        self.owner.object_bounds = mapping.rect(self.owner.object_bounds);
        Ok(self)
    }

    pub(super) fn prepare(
        self,
        engine: &mut GlyphonTextEngine,
        viewport: RenderViewport,
    ) -> Result<PreparedPlannedTextInput, FramePlanError> {
        let display = TextControlDisplayText::for_control(&self.control, &self.options);
        let inner = text_inner_bounds(&self.control);
        let document = display.document(self.style.clone())?;
        let unscrolled = layout_text_input(
            engine,
            &document,
            inner,
            self.options.is_multiline(),
            LayoutPoint::new(0.0, 0.0),
        )?;
        let mut editor_unscrolled = unscrolled.clone();
        display.remap_layout_for_editor(&mut editor_unscrolled);
        let content_size = text_control_content_size(&unscrolled, inner);
        let caret = text_caret_rect(
            &self.control,
            &editor_unscrolled,
            self.control.selection.end().get(),
        );
        let scroll = text_control_scroll_offset(&self.options, inner, content_size, caret);
        let layout = if scroll.x == 0.0 && scroll.y == 0.0 {
            unscrolled
        } else {
            layout_text_input(
                engine,
                &document,
                inner,
                self.options.is_multiline(),
                scroll,
            )?
        };
        let mut editor_layout_source = layout.clone();
        display.remap_layout_for_editor(&mut editor_layout_source);
        let (focused_target, raw_geometry) = prepare_text_input_target(
            viewport,
            &self.control,
            &self.options,
            &editor_layout_source,
            self.focused,
        )?;
        let display_text = display.value;
        let mut interaction =
            TextInteractionPlan::from_layout(&layout, Some(self.control.target.clone()))
                .with_text_and_selection_color(display_text, self.selection_rgba)
                .with_container_bounds(LayoutRect::new(
                    self.control.bounds.x,
                    self.control.bounds.y,
                    self.control.bounds.width,
                    self.control.bounds.height,
                ));
        if self.focused {
            interaction.selection_rects = raw_geometry
                .viewport_selection_rects()
                .iter()
                .map(|rect| hit_rect_to_layout(rect.bounds))
                .collect();
            interaction.caret = Some(TextCaretPaint {
                bounds: hit_rect_to_layout(raw_geometry.viewport_caret_rect()),
                color: self.caret,
                visible: true,
            });
            interaction.composition_underlines = raw_geometry
                .viewport_composition_rects()
                .iter()
                .map(|rect| TextCompositionUnderline {
                    source_range: RichTextRange::new(
                        usize::try_from(rect.range.start().get()).unwrap_or(usize::MAX),
                        usize::try_from(rect.range.end().get()).unwrap_or(usize::MAX),
                    ),
                    bounds: hit_rect_to_layout(rect.bounds),
                    color: self.composition_underline,
                    thickness: 1.0,
                })
                .collect();
        }
        let paint = TextPaintPlan::from_layout(&layout);
        let clip = clipped_viewport_bounds(inner, &self.control)
            .unwrap_or(HitRect::new(inner.x, inner.y, 0.0, 0.0));
        let item = engine.prepare_text_item(
            layout,
            paint,
            interaction,
            Some(hit_rect_to_layout(clip)),
            viewport.physical_scale_factor_f32(),
        )?;
        Ok(PreparedPlannedTextInput {
            item,
            owner: self.owner,
            focused_target,
        })
    }
}

pub(super) fn text_input_depth_milli(control: &RenderTextInputControl) -> i32 {
    control.style.depth_milli.unwrap_or_default()
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
    text: &mut Vec<PlannedFrameText>,
    palette: &Palette,
    control_backdrops: &mut Vec<PreparedControlBackdrop>,
    control_shadows: &mut Vec<PreparedControlShadow>,
    control_filters: &mut Vec<PreparedControlFilter>,
) -> Result<PreparedControlPaint, FramePlanError> {
    let options = control.resolved_options()?;
    let focused = scene.interaction.focused.as_ref() == Some(&control.target);
    let visual = &control.style;
    let radii = visual.radii();
    let visible_bounds = visible_control_bounds(control).unwrap_or(control.bounds);
    let backdrop_start = control_backdrops.len();
    push_control_backdrop_plan(control_backdrops, &control.target, visible_bounds, visual);
    let shadow_start = control_shadows.len();
    push_control_shadow_plan(control_shadows, &control.target, visible_bounds, visual);
    let rectangle_start = rectangles.len();
    rectangles.push(PaintRect::with_radii(
        control.bounds,
        fill_with_opacity(visual.fill.unwrap_or(palette.choice_idle), visual.opacity),
        radii,
    ));
    push_control_border(rectangles, control.bounds, visual.border, radii);
    push_control_corner_frame(rectangles, control.bounds, visual.corner_frame);
    if let Some(ring) = visual.focus_ring {
        push_control_focus_ring(rectangles, control.bounds, ring, radii);
    }

    let text_start = text.len();
    let text_rgba =
        if control.value.is_empty() && control.label.as_deref().is_some_and(|v| !v.is_empty()) {
            visual
                .placeholder
                .or(visual.text)
                .unwrap_or(palette.choice_text)
        } else {
            visual.text.unwrap_or(palette.choice_text)
        };
    let style = super::prepared_text::resolved_plain_style(
        control_font_families(visual),
        text_control_font_size(control, visual),
        text_control_line_height(control, visual),
        control_text_weight(visual, TextWeight::Normal),
        TextSlant::Upright,
        text_rgba,
    )?
    .with_spacing(visual.letter_spacing_milli.unwrap_or_default(), 0);
    text.push(PlannedFrameText::TextInput(Box::new(PlannedTextInput {
        control: control.clone(),
        options,
        style,
        selection_rgba: visual.selection.unwrap_or(palette.choice_active),
        caret: text_color(visual.caret.unwrap_or(palette.focus_ring)),
        composition_underline: text_color(
            visual
                .composition_underline
                .or(visual.caret)
                .unwrap_or(palette.focus_ring),
        ),
        owner: PlannedTextOwner {
            semantic_id: control.target.id().clone(),
            object_bounds: visible_bounds,
        },
        focused,
    })));
    let filter_start = control_filters.len();
    push_control_filter_plan(control_filters, &control.target, visible_bounds, visual);
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
    Ok(paint)
}

fn layout_text_input(
    engine: &mut GlyphonTextEngine,
    document: &ResolvedTextDocument<'_>,
    inner: HitRect,
    multiline: bool,
    scroll: LayoutPoint,
) -> Result<TextLayout, FramePlanError> {
    layout_document(
        document,
        TextLayoutRequest {
            origin: LayoutPoint::new(inner.x - scroll.x, inner.y - scroll.y),
            size: LayoutSize::new(inner.width.max(1.0), inner.height.max(1.0)),
            horizontal_wrap: if multiline {
                HorizontalWrap::Wrap
            } else {
                HorizontalWrap::NoWrap
            },
            ..TextLayoutRequest::default()
        },
        engine,
    )
    .map_err(FramePlanError::from)
}

#[derive(Clone, Debug)]
struct TextControlDisplayText {
    value: String,
    chars: Vec<TextControlDisplayChar>,
}

#[derive(Clone, Debug)]
struct TextControlDisplayChar {
    display: std::ops::Range<usize>,
    source: std::ops::Range<usize>,
}

impl TextControlDisplayText {
    fn for_control(control: &RenderTextInputControl, options: &TextInputOptions) -> Self {
        if control.value.is_empty()
            && let Some(placeholder) = control.label.as_deref().filter(|value| !value.is_empty())
        {
            return Self::placeholder(placeholder);
        }
        Self::new(&control.value, options)
    }

    fn placeholder(placeholder: &str) -> Self {
        let chars = placeholder
            .char_indices()
            .map(|(start, ch)| TextControlDisplayChar {
                display: start..start.saturating_add(ch.len_utf8()),
                source: 0..0,
            })
            .collect();
        Self {
            value: placeholder.to_owned(),
            chars,
        }
    }

    fn new(source: &str, options: &TextInputOptions) -> Self {
        let mut value = String::with_capacity(source.len());
        let mut chars = Vec::new();
        for (source_start, ch) in source.char_indices() {
            let source_end = source_start.saturating_add(ch.len_utf8());
            let visual = if options.is_secure() {
                '*'
            } else if ch == '\n' && !options.is_multiline() {
                ' '
            } else {
                ch
            };
            let display_start = value.len();
            value.push(visual);
            chars.push(TextControlDisplayChar {
                display: display_start..value.len(),
                source: source_start..source_end,
            });
        }
        Self { value, chars }
    }

    fn document(
        &self,
        style: ResolvedTextStyle,
    ) -> Result<ResolvedTextDocument<'_>, FramePlanError> {
        let runs = if self.value.is_empty() {
            Vec::new()
        } else {
            let range = RichTextRange::new(0, self.value.len());
            vec![ResolvedTextRun::new(
                range,
                range,
                style,
                RichTextPresentation::default(),
                ResolvedTextRunSource::Editable,
            )?]
        };
        ResolvedTextDocument::new(
            &self.value,
            0,
            runs,
            Vec::new(),
            TextDocumentRevision::new(0),
        )
        .map_err(FramePlanError::from)
    }

    fn remap_layout_for_editor(&self, layout: &mut TextLayout) {
        for line in &mut layout.lines {
            line.source_range = self.source_range(line.source_range);
        }
        for run in &mut layout.runs {
            run.source_range = self.source_range(run.source_range);
        }
        for glyph in &mut layout.glyphs {
            glyph.source_range = self.source_range(glyph.source_range);
        }
        layout.source_map = TextLayoutSourceMap::new(
            layout
                .glyphs
                .iter()
                .map(|glyph| TextLayoutGlyphSource {
                    run_index: glyph.run_index,
                    source_range: glyph.source_range,
                    line_index: glyph.line_index,
                    cluster_index: glyph.cluster_index,
                    logical_ordinal: glyph.logical_ordinal,
                })
                .collect(),
        );
    }

    fn source_range(&self, display: RichTextRange) -> RichTextRange {
        let mut source_start = None;
        let mut source_end = None;
        for item in &self.chars {
            if item.display.end <= display.start {
                continue;
            }
            if item.display.start >= display.end {
                break;
            }
            source_start.get_or_insert(item.source.start);
            source_end = Some(item.source.end);
        }
        source_start.map_or_else(
            || {
                let offset = self.source_offset(display.start);
                RichTextRange::new(offset, offset)
            },
            |start| RichTextRange::new(start, source_end.unwrap_or(start)),
        )
    }

    fn source_offset(&self, display_offset: usize) -> usize {
        for item in &self.chars {
            if display_offset <= item.display.start {
                return item.source.start;
            }
            if display_offset < item.display.end {
                return item.source.end;
            }
        }
        self.chars.last().map_or(0, |item| item.source.end)
    }
}

fn prepare_text_input_target(
    viewport: RenderViewport,
    control: &RenderTextInputControl,
    options: &TextInputOptions,
    layout: &TextLayout,
    focused: bool,
) -> Result<
    (
        Option<PreparedTextInputTarget>,
        arcweft_presentation::text_input::TextInputGeometrySnapshot,
    ),
    FramePlanError,
> {
    let editor = TextEditorState::from_text_control(
        control.session,
        control.target.clone(),
        control.value.clone(),
        control.selection,
        options.clone(),
    )?;
    let scale_factor = num_traits::ToPrimitive::to_f32(&viewport.scale_factor).unwrap_or(f32::MAX);
    let editor_layout = TextEditorGeometryPump::layout_from_text_layout(
        editor.text(),
        layout,
        TextEditorGeometryContext::default()
            .with_text_local_control_rect(HitRect::new(
                0.0,
                0.0,
                control.bounds.width,
                control.bounds.height,
            ))
            .with_layout_to_text_local(TextGeometryTransform::translation(
                -control.bounds.x,
                -control.bounds.y,
            ))
            .with_text_local_to_viewport(TextGeometryTransform::translation(
                control.bounds.x,
                control.bounds.y,
            ))
            .with_viewport_to_screen(TextGeometryTransform::scale(scale_factor, scale_factor))
            .with_writing_mode(TextWritingMode::HorizontalTb),
    )
    .map_err(TextEditorError::from)?;
    let (snapshot, geometry) = editor.snapshots(&editor_layout)?.into_parts();
    let focused_target = focused.then(|| {
        let security = TextInputSecurityPolicy::from_options(options);
        PreparedTextInputTarget {
            snapshot: security.redact_snapshot(&snapshot),
            geometry: security.redact_geometry(&geometry),
        }
    });
    Ok((focused_target, geometry))
}

fn text_caret_rect(control: &RenderTextInputControl, layout: &TextLayout, offset: u32) -> HitRect {
    let offset = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(control.value.len());
    if let Some(glyph) = layout.glyphs.windows(2).find_map(|pair| {
        let previous = &pair[0];
        let next = &pair[1];
        (previous.source_range.end == offset
            && previous.layout_bounds.width > 0.0
            && next.source_range.start == offset
            && next.layout_bounds.y > previous.layout_bounds.y)
            .then_some(previous)
    }) {
        return HitRect::new(
            glyph.layout_bounds.right(),
            glyph.layout_bounds.y,
            CARET_WIDTH,
            glyph.layout_bounds.height.max(1.0),
        );
    }
    if let Some(glyph) = layout
        .glyphs
        .iter()
        .find(|glyph| offset <= glyph.source_range.start)
    {
        return HitRect::new(
            glyph.layout_bounds.x,
            glyph.layout_bounds.y,
            CARET_WIDTH,
            glyph.layout_bounds.height.max(1.0),
        );
    }
    layout.glyphs.last().map_or_else(
        || {
            let inner = text_inner_bounds(control);
            HitRect::new(inner.x, inner.y, CARET_WIDTH, inner.height.max(1.0))
        },
        |glyph| {
            HitRect::new(
                glyph.layout_bounds.right(),
                glyph.layout_bounds.y,
                CARET_WIDTH,
                glyph.layout_bounds.height.max(1.0),
            )
        },
    )
}

fn text_control_content_size(layout: &TextLayout, inner: HitRect) -> LayoutSize {
    let right = layout
        .glyphs
        .iter()
        .map(|glyph| glyph.layout_bounds.right())
        .fold(inner.x, f32::max);
    let bottom = layout
        .glyphs
        .iter()
        .map(|glyph| glyph.layout_bounds.bottom())
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

fn text_inner_bounds(control: &RenderTextInputControl) -> HitRect {
    HitRect::new(
        control.bounds.x + TEXT_INSET_X,
        control.bounds.y + TEXT_INSET_Y,
        (control.bounds.width - TEXT_INSET_X * 2.0).max(0.0),
        (control.bounds.height - TEXT_INSET_Y * 2.0).max(1.0),
    )
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
            Some(clip) => rectangle.clip = Some(clip),
            None => rectangle.rgba[3] = 0.0,
        }
    }
}

fn hit_rect_to_layout(rect: HitRect) -> LayoutRect {
    LayoutRect::new(rect.x, rect.y, rect.width, rect.height)
}

fn text_color(rgba: [f32; 4]) -> TextColor {
    TextColor::rgba(
        unit_channel(rgba[0]),
        unit_channel(rgba[1]),
        unit_channel(rgba[2]),
        unit_channel(rgba[3]),
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn unit_channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
