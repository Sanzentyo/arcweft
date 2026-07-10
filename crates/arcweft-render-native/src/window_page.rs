use super::{
    NativeFrameContentBBox, NativeFrameDebugRegion, NativeFrameElement, NativeWindowError,
    RichTextEffectRegistry, RichTextMotionRegistry, RichTextShaderRegistry, RichTextStateStore,
    native_default_effect_registry, native_default_motion_registry, native_default_shader_registry,
    native_text_font_features, native_text_layout_config, page_local_layout_frame,
    prepare_window_text_buffers, surface_extent_f32, usize_to_f32_saturating,
    vertical_ruby_glyph_horizontal_align,
};
use arcweft_glyphon::VerticalGlyphHorizontalAlign;
use arcweft_render_text::{
    LineDisplayFrame, RichTextColor, RichTextControl, RichTextEffectDescriptor,
    RichTextEffectPhase, RichTextFontFamily, RichTextPresentation, RichTextRange,
    RichTextShaderRef, RichTextStyle, RichTextWritingMode,
};
use arcweft_text_layout::{LaidOutText, layout_frame};
use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache,
    TextAtlas, TextRenderer, Viewport, Weight,
};
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;
use wgpu::{
    CompositeAlphaMode, DeviceDescriptor, Instance, MultisampleState, PresentMode,
    RequestAdapterOptions, SurfaceConfiguration, TextureFormat, TextureUsages,
};
use winit::{
    dpi::PhysicalSize,
    event_loop::{ActiveEventLoop, EventLoop},
    window::Window,
};

pub(super) fn run_pages_window(
    title: &str,
    pages: Vec<WindowPage>,
) -> Result<(), NativeWindowError> {
    if pages.is_empty() {
        return Err(NativeWindowError::EmptyPages);
    }
    let event_loop =
        EventLoop::new().map_err(|error| NativeWindowError::EventLoop(error.to_string()))?;
    event_loop
        .run_app(Application {
            title: title.to_owned(),
            pages,
            page_index: 0,
            window_state: None,
        })
        .map_err(|error| NativeWindowError::EventLoop(error.to_string()))
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct WindowPage {
    pub(super) rich_text: WindowRichText,
    pub(super) layout_frame: Option<LineDisplayFrame>,
}

impl WindowPage {
    pub(super) fn plain(text: &str) -> Self {
        Self {
            rich_text: WindowRichText::plain(text),
            layout_frame: None,
        }
    }

    pub(super) fn from_frame(frame: &LineDisplayFrame) -> Vec<Self> {
        display_stage_ranges(frame)
            .into_iter()
            .filter_map(|range| page_from_display_map_range(frame, range))
            .collect()
    }
}

pub(super) fn window_page_has_timed_effects(page: &WindowPage) -> bool {
    page.layout_frame.as_ref().is_some_and(|frame| {
        frame
            .display_map
            .text_runs
            .iter()
            .any(|run| presentation_has_timed_effects(&run.presentation))
            || frame
                .display_map
                .ruby_annotations
                .iter()
                .any(|ruby| presentation_has_timed_effects(&ruby.presentation))
    })
}

pub(super) fn presentation_has_timed_effects(presentation: &RichTextPresentation) -> bool {
    !presentation.effects.is_empty()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WindowRichText {
    pub(super) text: String,
    pub(super) spans: Vec<WindowTextSpan>,
    pub(super) ruby_annotations: Vec<WindowRubyAnnotation>,
}

impl WindowRichText {
    pub(super) fn plain(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            spans: vec![WindowTextSpan {
                range: 0..text.len(),
                style: NativeTextStyle::default(),
            }],
            ruby_annotations: Vec::new(),
        }
    }
}

/// Completed visual range for every input-gated display stage.
///
/// `LineDisplayFrame::stages` owns the control semantics. A line wait therefore
/// retains the prefix from earlier stages on the same logical page, while a
/// page wait starts a fresh range. Clear controls move the visible origin but
/// do not introduce another user-input gate.
pub(super) fn display_stage_ranges(frame: &LineDisplayFrame) -> Vec<Range<usize>> {
    let mut page_index = None;
    let mut display_start = 0;
    frame
        .stages()
        .into_iter()
        .map(|stage| {
            let text_range = stage.text_range();
            if page_index != Some(stage.page_index()) {
                page_index = Some(stage.page_index());
                display_start = text_range.start;
            }
            for marker in stage.controls() {
                if matches!(marker.control, RichTextControl::Clear) {
                    display_start = text_range.start.saturating_add(marker.text_offset);
                }
            }
            display_start.min(text_range.end)..text_range.end
        })
        .collect()
}

pub(super) fn display_stage_range_at(
    frame: &LineDisplayFrame,
    stage_index: usize,
) -> Result<Range<usize>, NativeWindowError> {
    display_stage_ranges(frame)
        .into_iter()
        .nth(stage_index)
        .ok_or(NativeWindowError::EmptyPages)
}

pub(super) fn page_from_display_map_range(
    frame: &LineDisplayFrame,
    page_range: Range<usize>,
) -> Option<WindowPage> {
    let text = frame.text.get(page_range.clone())?.to_owned();
    let spans = display_map_spans_for_range(frame, &page_range);
    let spans = if spans.is_empty() {
        vec![WindowTextSpan {
            range: 0..text.len(),
            style: NativeTextStyle::default(),
        }]
    } else {
        spans
    };
    let ruby_annotations = display_map_ruby_for_range(frame, &page_range);
    Some(WindowPage {
        rich_text: WindowRichText {
            text,
            spans,
            ruby_annotations,
        },
        layout_frame: page_local_layout_frame(frame, page_range)
            .ok()
            .map(|(frame, _, _)| frame),
    })
}

pub(super) fn display_map_spans_for_range(
    frame: &LineDisplayFrame,
    page_range: &Range<usize>,
) -> Vec<WindowTextSpan> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter_map(|run| {
            let range = intersect_display_range(run.range, page_range)?;
            Some(WindowTextSpan {
                range: (range.start - page_range.start)..(range.end - page_range.start),
                style: native_style_from_styles(&run.styles),
            })
        })
        .collect()
}

pub(super) fn display_map_ruby_for_range(
    frame: &LineDisplayFrame,
    page_range: &Range<usize>,
) -> Vec<WindowRubyAnnotation> {
    frame
        .display_map
        .ruby_annotations
        .iter()
        .filter_map(|annotation| {
            let base_range = valid_display_range(annotation.base_range, &frame.text)?;
            if base_range.start < page_range.start || base_range.end > page_range.end {
                return None;
            }
            Some(WindowRubyAnnotation {
                base_range: (base_range.start - page_range.start)
                    ..(base_range.end - page_range.start),
                ruby: annotation.ruby.clone(),
                style: native_ruby_style_from_styles(&annotation.styles, &annotation.presentation),
                presentation: annotation.presentation.clone(),
            })
        })
        .collect()
}

pub(super) fn post_process_shaders_for_page(
    frame: &LineDisplayFrame,
    page_range: &Range<usize>,
) -> Vec<RichTextShaderRef> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| intersect_display_range(run.range, page_range).is_some())
        .flat_map(|run| post_process_shaders_from_presentation(&run.presentation))
        .chain(
            frame
                .display_map
                .ruby_annotations
                .iter()
                .filter(|ruby| intersect_display_range(ruby.base_range, page_range).is_some())
                .flat_map(|ruby| post_process_shaders_from_presentation(&ruby.presentation)),
        )
        .collect()
}

pub(super) fn post_process_effects_for_page(
    frame: &LineDisplayFrame,
    page_range: &Range<usize>,
) -> Vec<RichTextEffectDescriptor> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| intersect_display_range(run.range, page_range).is_some())
        .flat_map(|run| post_process_effects_from_presentation(&run.presentation))
        .chain(
            frame
                .display_map
                .ruby_annotations
                .iter()
                .filter(|ruby| intersect_display_range(ruby.base_range, page_range).is_some())
                .flat_map(|ruby| post_process_effects_from_presentation(&ruby.presentation)),
        )
        .collect()
}

pub(super) fn post_process_shaders_for_regions(
    frame: &LineDisplayFrame,
    page_range: &Range<usize>,
    regions: &[NativeFrameDebugRegion],
) -> Vec<RichTextShaderRef> {
    regions
        .iter()
        .flat_map(|region| match region.element {
            Some(NativeFrameElement::TextRun { index }) => frame
                .display_map
                .text_runs
                .get(index)
                .filter(|run| intersect_display_range(run.range, page_range).is_some())
                .map(|run| post_process_shaders_from_presentation(&run.presentation))
                .unwrap_or_default(),
            Some(NativeFrameElement::TextObjectProxy { run_index, .. }) => frame
                .display_map
                .text_runs
                .get(run_index)
                .filter(|run| intersect_display_range(run.range, page_range).is_some())
                .map(|run| post_process_shaders_from_presentation(&run.presentation))
                .unwrap_or_default(),
            Some(NativeFrameElement::GlyphCluster {
                range_start,
                range_end,
                ..
            }) => {
                let range = RichTextRange::new(range_start, range_end);
                frame
                    .display_map
                    .text_runs
                    .iter()
                    .find(|run| {
                        range.start >= run.range.start
                            && range.end <= run.range.end
                            && intersect_display_range(run.range, page_range).is_some()
                    })
                    .map(|run| post_process_shaders_from_presentation(&run.presentation))
                    .unwrap_or_default()
            }
            Some(NativeFrameElement::Ruby { index }) => frame
                .display_map
                .ruby_annotations
                .get(index)
                .filter(|ruby| intersect_display_range(ruby.base_range, page_range).is_some())
                .map(|ruby| post_process_shaders_from_presentation(&ruby.presentation))
                .unwrap_or_default(),
            None => Vec::new(),
        })
        .collect()
}

pub(super) fn post_process_effects_for_regions(
    frame: &LineDisplayFrame,
    page_range: &Range<usize>,
    regions: &[NativeFrameDebugRegion],
) -> Vec<RichTextEffectDescriptor> {
    regions
        .iter()
        .flat_map(|region| match region.element {
            Some(NativeFrameElement::TextRun { index }) => frame
                .display_map
                .text_runs
                .get(index)
                .filter(|run| intersect_display_range(run.range, page_range).is_some())
                .map(|run| post_process_effects_from_presentation(&run.presentation))
                .unwrap_or_default(),
            Some(NativeFrameElement::TextObjectProxy { run_index, .. }) => frame
                .display_map
                .text_runs
                .get(run_index)
                .filter(|run| intersect_display_range(run.range, page_range).is_some())
                .map(|run| post_process_effects_from_presentation(&run.presentation))
                .unwrap_or_default(),
            Some(NativeFrameElement::GlyphCluster {
                range_start,
                range_end,
                ..
            }) => {
                let range = RichTextRange::new(range_start, range_end);
                frame
                    .display_map
                    .text_runs
                    .iter()
                    .find(|run| {
                        range.start >= run.range.start
                            && range.end <= run.range.end
                            && intersect_display_range(run.range, page_range).is_some()
                    })
                    .map(|run| post_process_effects_from_presentation(&run.presentation))
                    .unwrap_or_default()
            }
            Some(NativeFrameElement::Ruby { index }) => frame
                .display_map
                .ruby_annotations
                .get(index)
                .filter(|ruby| intersect_display_range(ruby.base_range, page_range).is_some())
                .map(|ruby| post_process_effects_from_presentation(&ruby.presentation))
                .unwrap_or_default(),
            None => Vec::new(),
        })
        .collect()
}

pub(super) fn post_process_shaders_from_presentation(
    presentation: &RichTextPresentation,
) -> Vec<RichTextShaderRef> {
    presentation
        .shaders
        .iter()
        .filter(|shader| shader.phase == RichTextEffectPhase::PostProcess)
        .cloned()
        .collect()
}

pub(super) fn post_process_effects_from_presentation(
    presentation: &RichTextPresentation,
) -> Vec<RichTextEffectDescriptor> {
    presentation
        .effects
        .iter()
        .filter(|effect| effect.phase == RichTextEffectPhase::PostProcess)
        .cloned()
        .collect()
}

pub(super) fn debug_rich_text_for_regions(
    frame: &LineDisplayFrame,
    page_range: &Range<usize>,
    page_rich_text: &WindowRichText,
    regions: &[NativeFrameDebugRegion],
) -> Option<WindowRichText> {
    let selected_text = debug_selected_text_ranges(frame, page_range, regions);
    let selected_ruby = debug_selected_ruby_indices(regions);
    if selected_text.is_empty() && selected_ruby.is_empty() {
        return None;
    }
    let spans = debug_text_spans(page_rich_text, &selected_text);
    let ruby_annotations = frame
        .display_map
        .ruby_annotations
        .iter()
        .enumerate()
        .filter_map(|(index, annotation)| {
            let base_range = valid_display_range(annotation.base_range, &frame.text)?;
            if base_range.start < page_range.start || base_range.end > page_range.end {
                return None;
            }
            let mut style =
                native_ruby_style_from_styles(&annotation.styles, &annotation.presentation);
            style.color = selected_ruby
                .iter()
                .find_map(|(selected_index, color)| (*selected_index == index).then_some(*color))
                .map_or(NativeTextColor::rgba(0, 0, 0, 0), native_color_from_rgba);
            Some(WindowRubyAnnotation {
                base_range: (base_range.start - page_range.start)
                    ..(base_range.end - page_range.start),
                ruby: annotation.ruby.clone(),
                style,
                presentation: annotation.presentation.clone(),
            })
        })
        .collect();
    Some(WindowRichText {
        text: page_rich_text.text.clone(),
        spans,
        ruby_annotations,
    })
}

pub(super) fn color_rich_text_for_regions(
    frame: &LineDisplayFrame,
    page_range: &Range<usize>,
    page_rich_text: &WindowRichText,
    regions: &[NativeFrameDebugRegion],
) -> Option<WindowRichText> {
    let selected_text = color_selected_text_ranges(frame, page_range, regions);
    let selected_ruby = color_selected_ruby_indices(regions);
    if selected_text.is_empty() && selected_ruby.is_empty() {
        return None;
    }
    let spans = color_text_spans(page_rich_text, &selected_text);
    let ruby_annotations = frame
        .display_map
        .ruby_annotations
        .iter()
        .enumerate()
        .filter_map(|(index, annotation)| {
            let base_range = valid_display_range(annotation.base_range, &frame.text)?;
            if base_range.start < page_range.start || base_range.end > page_range.end {
                return None;
            }
            let mut style =
                native_ruby_style_from_styles(&annotation.styles, &annotation.presentation);
            if !selected_ruby.contains(&index) {
                style.color = NativeTextColor::rgba(0, 0, 0, 0);
            }
            Some(WindowRubyAnnotation {
                base_range: (base_range.start - page_range.start)
                    ..(base_range.end - page_range.start),
                ruby: annotation.ruby.clone(),
                style,
                presentation: annotation.presentation.clone(),
            })
        })
        .collect();
    Some(WindowRichText {
        text: page_rich_text.text.clone(),
        spans,
        ruby_annotations,
    })
}

pub(super) fn debug_selected_text_ranges(
    frame: &LineDisplayFrame,
    page_range: &Range<usize>,
    regions: &[NativeFrameDebugRegion],
) -> Vec<(Range<usize>, [u8; 4])> {
    regions
        .iter()
        .filter_map(|region| {
            let range = match region.element? {
                NativeFrameElement::TextRun { index } => {
                    let run = frame.display_map.text_runs.get(index)?;
                    intersect_display_range(run.range, page_range)?
                }
                NativeFrameElement::TextObjectProxy { run_index, .. } => {
                    let run = frame.display_map.text_runs.get(run_index)?;
                    intersect_display_range(run.range, page_range)?
                }
                NativeFrameElement::GlyphCluster {
                    range_start,
                    range_end,
                    ..
                } => {
                    intersect_display_range(RichTextRange::new(range_start, range_end), page_range)?
                }
                NativeFrameElement::Ruby { .. } => return None,
            };
            Some((
                (range.start - page_range.start)..(range.end - page_range.start),
                region.color,
            ))
        })
        .collect()
}

pub(super) fn color_selected_text_ranges(
    frame: &LineDisplayFrame,
    page_range: &Range<usize>,
    regions: &[NativeFrameDebugRegion],
) -> Vec<Range<usize>> {
    regions
        .iter()
        .filter_map(|region| {
            let range = match region.element? {
                NativeFrameElement::TextRun { index } => {
                    let run = frame.display_map.text_runs.get(index)?;
                    intersect_display_range(run.range, page_range)?
                }
                NativeFrameElement::TextObjectProxy { run_index, .. } => {
                    let run = frame.display_map.text_runs.get(run_index)?;
                    intersect_display_range(run.range, page_range)?
                }
                NativeFrameElement::GlyphCluster {
                    range_start,
                    range_end,
                    ..
                } => {
                    intersect_display_range(RichTextRange::new(range_start, range_end), page_range)?
                }
                NativeFrameElement::Ruby { .. } => return None,
            };
            Some((range.start - page_range.start)..(range.end - page_range.start))
        })
        .collect()
}

pub(super) fn debug_selected_ruby_indices(
    regions: &[NativeFrameDebugRegion],
) -> Vec<(usize, [u8; 4])> {
    regions
        .iter()
        .filter_map(|region| {
            let NativeFrameElement::Ruby { index } = region.element? else {
                return None;
            };
            Some((index, region.color))
        })
        .collect()
}

pub(super) fn color_selected_ruby_indices(regions: &[NativeFrameDebugRegion]) -> Vec<usize> {
    regions
        .iter()
        .filter_map(|region| {
            let NativeFrameElement::Ruby { index } = region.element? else {
                return None;
            };
            Some(index)
        })
        .collect()
}

pub(super) fn debug_text_spans(
    rich_text: &WindowRichText,
    selected: &[(Range<usize>, [u8; 4])],
) -> Vec<WindowTextSpan> {
    let mut boundaries = vec![0, rich_text.text.len()];
    boundaries.extend(
        rich_text
            .spans
            .iter()
            .flat_map(|span| [span.range.start, span.range.end]),
    );
    boundaries.extend(
        selected
            .iter()
            .flat_map(|(range, _)| [range.start, range.end]),
    );
    boundaries.retain(|offset| {
        *offset <= rich_text.text.len() && rich_text.text.is_char_boundary(*offset)
    });
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
        .windows(2)
        .filter_map(|window| {
            let start = window[0];
            let end = window[1];
            if start >= end {
                return None;
            }
            let mut style = rich_text
                .spans
                .iter()
                .find(|span| span.range.start <= start && end <= span.range.end)
                .map_or_else(NativeTextStyle::default, |span| span.style.clone());
            style.color = selected
                .iter()
                .find_map(|(range, color)| {
                    (range.start <= start && end <= range.end).then_some(*color)
                })
                .map_or(NativeTextColor::rgba(0, 0, 0, 0), native_color_from_rgba);
            Some(WindowTextSpan {
                range: start..end,
                style,
            })
        })
        .collect()
}

pub(super) fn color_text_spans(
    rich_text: &WindowRichText,
    selected: &[Range<usize>],
) -> Vec<WindowTextSpan> {
    let mut boundaries = vec![0, rich_text.text.len()];
    boundaries.extend(
        rich_text
            .spans
            .iter()
            .flat_map(|span| [span.range.start, span.range.end]),
    );
    boundaries.extend(selected.iter().flat_map(|range| [range.start, range.end]));
    boundaries.retain(|offset| {
        *offset <= rich_text.text.len() && rich_text.text.is_char_boundary(*offset)
    });
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
        .windows(2)
        .filter_map(|window| {
            let start = window[0];
            let end = window[1];
            if start >= end {
                return None;
            }
            let mut style = rich_text
                .spans
                .iter()
                .find(|span| span.range.start <= start && end <= span.range.end)
                .map_or_else(NativeTextStyle::default, |span| span.style.clone());
            if !selected
                .iter()
                .any(|range| range.start <= start && end <= range.end)
            {
                style.color = NativeTextColor::rgba(0, 0, 0, 0);
            }
            Some(WindowTextSpan {
                range: start..end,
                style,
            })
        })
        .collect()
}

pub(super) fn native_color_from_rgba(color: [u8; 4]) -> NativeTextColor {
    NativeTextColor::rgba(color[0], color[1], color[2], color[3])
}

pub(super) fn intersect_display_range(
    range: RichTextRange,
    page_range: &Range<usize>,
) -> Option<Range<usize>> {
    let start = range.start.max(page_range.start);
    let end = range.end.min(page_range.end);
    (start < end).then_some(start..end)
}

pub(super) fn valid_display_range(range: RichTextRange, text: &str) -> Option<Range<usize>> {
    if range.start <= range.end
        && range.end <= text.len()
        && text.is_char_boundary(range.start)
        && text.is_char_boundary(range.end)
    {
        Some(range.start..range.end)
    } else {
        None
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WindowTextSpan {
    pub(super) range: Range<usize>,
    pub(super) style: NativeTextStyle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WindowRubyAnnotation {
    pub(super) base_range: Range<usize>,
    pub(super) ruby: String,
    pub(super) style: NativeTextStyle,
    pub(super) presentation: RichTextPresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeTextColor {
    pub(super) red: u8,
    pub(super) green: u8,
    pub(super) blue: u8,
    pub(super) alpha: u8,
}

impl NativeTextColor {
    pub(super) const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 255,
        }
    }

    pub(super) const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub(super) fn from_render_color(color: &RichTextColor) -> Self {
        match color {
            RichTextColor::Rgb { red, green, blue } => Self::new(*red, *green, *blue),
            RichTextColor::Named { name } => match name.as_str() {
                "red" => Self::new(240, 110, 110),
                "green" => Self::new(120, 220, 150),
                "blue" => Self::new(130, 180, 255),
                "yellow" => Self::new(240, 220, 120),
                "muted" | "quiet" => Self::new(170, 170, 170),
                _ => Self::new(245, 245, 245),
            },
        }
    }

    pub(super) const fn into_glyphon(self) -> Color {
        Color::rgba(self.red, self.green, self.blue, self.alpha)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeTextStyle {
    pub(super) color: NativeTextColor,
    pub(super) family: NativeFontFamily,
    pub(super) weight: NativeTextWeight,
    pub(super) italic: bool,
    pub(super) size: Option<u16>,
}

impl NativeTextStyle {
    pub(super) fn attrs(&self) -> Attrs<'_> {
        self.attrs_with_metrics(Self::metrics_for_size)
    }

    pub(super) fn ruby_attrs(&self) -> Attrs<'_> {
        self.attrs_with_metrics(Self::ruby_metrics_for_size)
    }

    pub(super) fn attrs_with_metrics(&self, metrics_for_size: fn(u16) -> Metrics) -> Attrs<'_> {
        let mut attrs = Attrs::new()
            .family(self.family.as_glyphon_family())
            .color(self.color.into_glyphon())
            .font_features(native_text_font_features());
        if self.weight == NativeTextWeight::Bold {
            attrs = attrs.weight(Weight::BOLD);
        }
        if self.italic {
            attrs = attrs.style(Style::Italic);
        }
        if let Some(size) = self.size {
            attrs = attrs.metrics(metrics_for_size(size));
        }
        attrs
    }

    pub(super) fn metrics(&self) -> Metrics {
        self.size.map_or(Metrics::new(30.0, 42.0), |size| {
            Self::metrics_for_size(size)
        })
    }

    pub(super) fn ruby_metrics(&self) -> Metrics {
        self.size.map_or(Metrics::new(14.0, 14.0), |size| {
            Self::ruby_metrics_for_size(size)
        })
    }

    pub(super) fn metrics_for_size(size: u16) -> Metrics {
        let font_size = f32::from(size);
        Metrics::new(font_size, font_size * 1.35)
    }

    pub(super) fn ruby_metrics_for_size(size: u16) -> Metrics {
        let font_size = f32::from(size);
        Metrics::new(font_size, font_size)
    }
}

impl Default for NativeTextStyle {
    fn default() -> Self {
        Self {
            color: NativeTextColor::new(245, 245, 245),
            family: NativeFontFamily::SansSerif,
            weight: NativeTextWeight::Regular,
            italic: false,
            size: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum NativeFontFamily {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    Named(String),
}

impl NativeFontFamily {
    pub(super) fn from_render_family(family: &RichTextFontFamily) -> Self {
        match family {
            RichTextFontFamily::Serif => Self::Serif,
            RichTextFontFamily::SansSerif => Self::SansSerif,
            RichTextFontFamily::Monospace => Self::Monospace,
            RichTextFontFamily::Cursive => Self::Cursive,
            RichTextFontFamily::Fantasy => Self::Fantasy,
            RichTextFontFamily::Named { name } => Self::Named(name.clone()),
        }
    }

    pub(super) fn as_glyphon_family(&self) -> Family<'_> {
        match self {
            Self::Serif => Family::Serif,
            Self::SansSerif => Family::SansSerif,
            Self::Monospace => Family::Monospace,
            Self::Cursive => Family::Cursive,
            Self::Fantasy => Family::Fantasy,
            Self::Named(name) => Family::Name(name),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeTextWeight {
    Regular,
    Bold,
}

pub(super) fn native_style_from_styles<'a>(
    styles: impl IntoIterator<Item = &'a RichTextStyle>,
) -> NativeTextStyle {
    styles
        .into_iter()
        .fold(NativeTextStyle::default(), apply_style)
}

pub(super) fn native_ruby_style_from_base(
    base_style: NativeTextStyle,
    presentation: &RichTextPresentation,
) -> NativeTextStyle {
    let mut style = NativeTextStyle {
        color: NativeTextColor::new(170, 190, 220),
        size: Some(14),
        ..base_style
    };
    if let Some(size) = native_ruby_font_size(presentation) {
        style.size = Some(size);
    }
    style
}

pub(super) fn native_ruby_style_from_styles(
    styles: &[RichTextStyle],
    presentation: &RichTextPresentation,
) -> NativeTextStyle {
    native_ruby_style_from_base(native_style_from_styles(styles), presentation)
}

pub(super) fn native_ruby_font_size(presentation: &RichTextPresentation) -> Option<u16> {
    let value = presentation.layout.as_ref()?.ruby_font_size?.as_f32();
    if value.is_finite() && value >= 1.0 {
        value
            .round()
            .min(f32::from(u16::MAX))
            .to_string()
            .parse()
            .ok()
    } else {
        None
    }
}

pub(super) fn apply_style(mut native: NativeTextStyle, style: &RichTextStyle) -> NativeTextStyle {
    match style {
        RichTextStyle::Em { .. } | RichTextStyle::Italic { .. } | RichTextStyle::Oblique { .. } => {
            native.italic = true;
        }
        RichTextStyle::Strong { .. } => native.weight = NativeTextWeight::Bold,
        RichTextStyle::Color { value } => {
            native.color = NativeTextColor::from_render_color(value);
        }
        RichTextStyle::Font { family } => {
            native.family = NativeFontFamily::from_render_family(family);
        }
        RichTextStyle::Size { points, .. } => {
            native.size = *points;
        }
        RichTextStyle::Speed { .. }
        | RichTextStyle::Layout { .. }
        | RichTextStyle::Transform { .. }
        | RichTextStyle::Presentation { .. }
        | RichTextStyle::Effect { .. }
        | RichTextStyle::Shader { .. }
        | RichTextStyle::Object { .. }
        | RichTextStyle::Unknown { .. } => {}
    }
    native
}

pub(super) struct WindowState {
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) surface_config: SurfaceConfiguration,
    pub(super) font_system: FontSystem,
    pub(super) swash_cache: SwashCache,
    pub(super) viewport: Viewport,
    pub(super) atlas: TextAtlas,
    pub(super) text_renderer: TextRenderer,
    pub(super) text_buffer: Buffer,
    pub(super) ruby_buffers: Vec<WindowRubyBuffer>,
    pub(super) rich_text: WindowRichText,
    pub(super) layout_frame: Option<LineDisplayFrame>,
    pub(super) layout: Option<LaidOutText>,
    pub(super) effect_registry: RichTextEffectRegistry,
    pub(super) shader_registry: RichTextShaderRegistry,
    pub(super) motion_registry: RichTextMotionRegistry,
    pub(super) effect_state: RichTextStateStore,
    pub(super) animation_started_at: Instant,
    pub(super) has_timed_effects: bool,
    pub(super) window: Arc<dyn Window>,
}

pub(super) struct WindowRubyBuffer {
    pub(super) buffer: Buffer,
    pub(super) source_index: usize,
    pub(super) left: f32,
    pub(super) top: f32,
    pub(super) placement: RubyGlyphPlacement,
    pub(super) color: NativeTextColor,
    pub(super) presentation: RichTextPresentation,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum RubyGlyphPlacement {
    Horizontal {
        line_height: f32,
    },
    Vertical {
        cell_width: f32,
        vertical_advance: f32,
        horizontal_align: VerticalGlyphHorizontalAlign,
    },
}

impl WindowState {
    pub(super) async fn new(
        window: Arc<dyn Window>,
        _event_loop: &dyn ActiveEventLoop,
        page: &WindowPage,
    ) -> Self {
        let physical_size = window.surface_size();
        let instance = Instance::default();
        let adapter = instance
            .request_adapter(&RequestAdapterOptions::default())
            .await
            .expect("request graphics adapter");
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor::default())
            .await
            .expect("request graphics device");
        let surface = instance
            .create_surface(window.clone())
            .expect("create surface");
        let surface_format = TextureFormat::Bgra8UnormSrgb;
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: physical_size.width.max(1),
            height: physical_size.height.max(1),
            present_mode: PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, surface_format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);
        let text_buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));

        let mut state = Self {
            device,
            queue,
            surface,
            surface_config,
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            text_buffer,
            ruby_buffers: Vec::new(),
            rich_text: page.rich_text.clone(),
            layout_frame: page.layout_frame.clone(),
            layout: None,
            effect_registry: native_default_effect_registry(),
            shader_registry: native_default_shader_registry(),
            motion_registry: native_default_motion_registry(),
            effect_state: RichTextStateStore::default(),
            animation_started_at: Instant::now(),
            has_timed_effects: false,
            window,
        };
        state.set_page(page);
        state
    }

    pub(super) fn set_page(&mut self, page: &WindowPage) {
        self.rich_text = page.rich_text.clone();
        self.layout_frame.clone_from(&page.layout_frame);
        self.animation_started_at = Instant::now();
        self.effect_state.clear();
        self.has_timed_effects = window_page_has_timed_effects(page);
        self.prepare_rich_text();
        self.window.request_redraw();
    }

    pub(super) fn effect_time_seconds(&self) -> f32 {
        self.animation_started_at.elapsed().as_secs_f32()
    }

    pub(super) fn prepare_rich_text(&mut self) {
        prepare_window_text_buffers(
            &mut self.font_system,
            &mut self.text_buffer,
            &self.rich_text,
            self.surface_config.width,
            self.surface_config.height,
        );
        self.layout = self.layout_frame.as_ref().and_then(|frame| {
            layout_frame(
                frame,
                native_text_layout_config(
                    self.surface_config.width,
                    self.surface_config.height,
                    NATIVE_TEXT_LEFT,
                    NATIVE_TEXT_TOP,
                ),
            )
            .ok()
        });
        self.ruby_buffers = build_ruby_buffers(
            &mut self.font_system,
            &self.text_buffer,
            &self.rich_text,
            self.layout.as_ref(),
            self.surface_config.width,
            self.surface_config.height,
            NativeTextOrigin::default(),
        );
    }

    pub(super) fn resize(&mut self, size: PhysicalSize<u32>) {
        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        self.prepare_rich_text();
        self.window.request_redraw();
    }
}

pub(super) fn build_ruby_buffers(
    font_system: &mut FontSystem,
    text_buffer: &Buffer,
    rich_text: &WindowRichText,
    layout: Option<&LaidOutText>,
    width: u32,
    height: u32,
    origin: NativeTextOrigin,
) -> Vec<WindowRubyBuffer> {
    let mut buffers = Vec::new();
    for (ruby_index, annotation) in rich_text.ruby_annotations.iter().enumerate() {
        if let Some(layout) = layout {
            let segments = layout
                .ruby
                .iter()
                .filter(|ruby| ruby.ruby_index == ruby_index)
                .collect::<Vec<_>>();
            if !segments.is_empty() {
                buffers.extend(segments.into_iter().map(|segment| {
                    let ruby_char_count = segment.ruby.chars().count().max(1);
                    let placement =
                        if matches!(segment.writing_mode, RichTextWritingMode::HorizontalTb) {
                            RubyGlyphPlacement::Horizontal {
                                line_height: segment.ruby_bounds.height,
                            }
                        } else {
                            RubyGlyphPlacement::Vertical {
                                cell_width: segment.ruby_bounds.width,
                                vertical_advance: segment.ruby_bounds.height
                                    / usize_to_f32_saturating(ruby_char_count),
                                horizontal_align: vertical_ruby_glyph_horizontal_align(segment),
                            }
                        };
                    build_ruby_buffer(
                        font_system,
                        &annotation.style,
                        RubyBufferSpec {
                            ruby: &segment.ruby,
                            left: segment.ruby_bounds.x,
                            top: segment.ruby_bounds.y,
                            placement,
                            presentation: &annotation.presentation,
                            source_index: ruby_index,
                            width,
                            height,
                        },
                    )
                }));
                continue;
            }
        }

        let mut buffer = Buffer::new(font_system, annotation.style.ruby_metrics());
        buffer.set_size(
            font_system,
            Some(surface_extent_f32(width)),
            Some(surface_extent_f32(height)),
        );
        let attrs = annotation.style.ruby_attrs();
        let spans = [(annotation.ruby.as_str(), attrs.clone())];
        buffer.set_rich_text(font_system, spans, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(font_system, false);
        let Some((left, top)) = ruby_layout_geometry(layout, ruby_index).or_else(|| {
            let ruby_width = buffer.layout_runs().next().map_or(0.0, |run| run.line_w);
            ruby_overlay_geometry(text_buffer, rich_text, &annotation.base_range, origin).map(
                |(base_left, top, base_width)| {
                    (base_left + (base_width - ruby_width).max(0.0) / 2.0, top)
                },
            )
        }) else {
            continue;
        };
        buffers.push(WindowRubyBuffer {
            buffer,
            source_index: ruby_index,
            left,
            top,
            placement: RubyGlyphPlacement::Horizontal {
                line_height: annotation.style.ruby_metrics().font_size,
            },
            color: annotation.style.color,
            presentation: annotation.presentation.clone(),
        });
    }
    buffers
}

pub(super) fn build_ruby_buffer(
    font_system: &mut FontSystem,
    style: &NativeTextStyle,
    spec: RubyBufferSpec<'_>,
) -> WindowRubyBuffer {
    let mut buffer = Buffer::new(font_system, style.ruby_metrics());
    buffer.set_size(
        font_system,
        Some(surface_extent_f32(spec.width)),
        Some(surface_extent_f32(spec.height)),
    );
    let attrs = style.ruby_attrs();
    let spans = [(spec.ruby, attrs.clone())];
    buffer.set_rich_text(font_system, spans, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);
    WindowRubyBuffer {
        buffer,
        source_index: spec.source_index,
        left: spec.left,
        top: spec.top,
        placement: spec.placement,
        color: style.color,
        presentation: spec.presentation.clone(),
    }
}

#[derive(Clone, Copy)]
pub(super) struct RubyBufferSpec<'a> {
    pub(super) ruby: &'a str,
    pub(super) left: f32,
    pub(super) top: f32,
    pub(super) placement: RubyGlyphPlacement,
    pub(super) presentation: &'a RichTextPresentation,
    pub(super) source_index: usize,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) fn ruby_layout_geometry(
    layout: Option<&LaidOutText>,
    ruby_index: usize,
) -> Option<(f32, f32)> {
    let ruby = layout?
        .ruby
        .iter()
        .find(|ruby| ruby.ruby_index == ruby_index)?;
    Some((ruby.ruby_bounds.x, ruby.ruby_bounds.y))
}

pub(super) const NATIVE_TEXT_LEFT: f32 = 24.0;
pub(super) const NATIVE_TEXT_TOP: f32 = 24.0;
pub(super) const NATIVE_RUBY_BASELINE_OFFSET: f32 = 48.0;
pub(super) const NATIVE_GLYPHAREA_BASELINE_OFFSET: f32 = 30.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct NativeTextOrigin {
    pub(super) left: f32,
    pub(super) top: f32,
}

impl Default for NativeTextOrigin {
    fn default() -> Self {
        Self {
            left: NATIVE_TEXT_LEFT,
            top: NATIVE_TEXT_TOP,
        }
    }
}

pub(super) fn ruby_overlay_geometry(
    text_buffer: &Buffer,
    rich_text: &WindowRichText,
    base_range: &Range<usize>,
    origin: NativeTextOrigin,
) -> Option<(f32, f32, f32)> {
    let line_starts = text_line_start_offsets(&rich_text.text);
    for run in text_buffer.layout_runs() {
        let line_start = *line_starts.get(run.line_i)?;
        let line_end = line_starts
            .get(run.line_i + 1)
            .copied()
            .unwrap_or(rich_text.text.len());
        let start = base_range.start.max(line_start);
        let end = base_range.end.min(line_end);
        if start >= end {
            continue;
        }
        let local_start = start - line_start;
        let local_end = end - line_start;
        let mut left: Option<f32> = None;
        let mut right: Option<f32> = None;
        for glyph in run.glyphs {
            if glyph.end <= local_start || glyph.start >= local_end {
                continue;
            }
            let glyph_left = origin.left + glyph.x;
            let glyph_right = glyph_left + glyph.w;
            left = Some(left.map_or(glyph_left, |value| value.min(glyph_left)));
            right = Some(right.map_or(glyph_right, |value| value.max(glyph_right)));
        }
        let (Some(left), Some(right)) = (left, right) else {
            continue;
        };
        let top = (origin.top + run.line_y - NATIVE_RUBY_BASELINE_OFFSET).max(0.0);
        return Some((left, top, (right - left).max(1.0)));
    }
    None
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub(super) fn native_float_bbox(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    viewport_width: u32,
    viewport_height: u32,
) -> Option<NativeFrameContentBBox> {
    let x = x.floor().max(0.0) as u32;
    let y = y.floor().max(0.0) as u32;
    if x >= viewport_width || y >= viewport_height {
        return None;
    }
    let width = width.ceil().max(1.0).min(u32::MAX as f32) as u32;
    let height = height.ceil().max(1.0).min(u32::MAX as f32) as u32;
    Some(NativeFrameContentBBox {
        x,
        y,
        width: width.min(viewport_width.saturating_sub(x)).max(1),
        height: height.min(viewport_height.saturating_sub(y)).max(1),
    })
}

pub(super) fn text_line_start_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    offsets.extend(
        text.char_indices()
            .filter_map(|(index, ch)| (ch == '\n').then_some(index + ch.len_utf8())),
    );
    offsets
}

pub(super) struct Application {
    pub(super) title: String,
    pub(super) pages: Vec<WindowPage>,
    pub(super) page_index: usize,
    pub(super) window_state: Option<WindowState>,
}
