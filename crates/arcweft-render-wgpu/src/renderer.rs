use crate::convert::{pixel_ceil_as_i32, pixel_floor_as_i32};
use crate::geometry::{
    PaintRect, PreparedFrame, PreparedUiScene, RenderFontFamily, RenderGlyphMotion,
    RenderGlyphTransformSpan, RenderImage, RenderStyledParagraph, RenderStyledTextSpan,
    RenderTextBlock, RenderTextSlant, RenderTextStyle, RenderTextWeight,
};
use crate::ui_compositor::{UiCompositor, UiCompositorError, UiCompositorFrame};
use crate::ui_direct_renderer::{WgpuPreparedUiMaskTextureProvider, WgpuUiDirectPrimitiveRenderer};
use crate::ui_effects::UiTextureExtent;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::image::ImageObjectFit;
use arcweft_render_text::RichTextRange;
use bytemuck::{Pod, Zeroable};
use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, Style,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight,
};
use num_traits::ToPrimitive;
use std::borrow::Cow;
use thiserror::Error;
use wgpu::util::DeviceExt;

const TRANSPARENT_ALPHA: u8 = 0;

/// GPU renderer shared by native surfaces, browser surfaces, and offscreen tests.
pub struct SharedRenderer {
    format: wgpu::TextureFormat,
    rectangle_pipeline: wgpu::RenderPipeline,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_pipeline: wgpu::RenderPipeline,
    image_sampler: wgpu::Sampler,
    _glyphon_cache: Cache,
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    ui_compositor: UiCompositor,
    ui_direct_renderer: WgpuUiDirectPrimitiveRenderer,
    registered_font_bytes: usize,
}

/// Shared renderer failure. Platform surface errors are intentionally absent.
#[derive(Debug, Error)]
pub enum SharedRendererError {
    #[error("font bytes must not be empty")]
    EmptyFont,
    #[error("glyphon text preparation failed: {0}")]
    TextPrepare(String),
    #[error("glyphon text rendering failed: {0}")]
    TextRender(String),
    #[error("ui compositor failed: {0}")]
    UiCompositor(#[from] UiCompositorError),
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct RectVertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ImageVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

impl SharedRenderer {
    /// Creates pipelines for a host-selected render-target format.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let glyphon_cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &glyphon_cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        let (image_bind_group_layout, image_pipeline, image_sampler) =
            image_quad_pipeline(device, format);
        let ui_compositor = UiCompositor::new(device, queue, format);
        let ui_direct_renderer = WgpuUiDirectPrimitiveRenderer::new(device, format);
        Self {
            format,
            rectangle_pipeline: rectangle_pipeline(device, format),
            image_bind_group_layout,
            image_pipeline,
            image_sampler,
            viewport: Viewport::new(device, &glyphon_cache),
            _glyphon_cache: glyphon_cache,
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            atlas,
            text_renderer,
            ui_compositor,
            ui_direct_renderer,
            registered_font_bytes: 0,
        }
    }

    pub const fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub const fn registered_font_bytes(&self) -> usize {
        self.registered_font_bytes
    }

    /// Registers project-owned font bytes. Native and Web parity tests must feed
    /// the same bytes through this API rather than relying on host system fonts.
    pub fn register_font_bytes(&mut self, bytes: Vec<u8>) -> Result<(), SharedRendererError> {
        if bytes.is_empty() {
            return Err(SharedRendererError::EmptyFont);
        }
        self.registered_font_bytes = self.registered_font_bytes.saturating_add(bytes.len());
        self.font_system.db_mut().load_font_data(bytes);
        Ok(())
    }

    /// Extracts renderer-owned styled paragraph layout evidence with the same
    /// registered font system that prepares text for rendering.
    #[must_use]
    pub fn frame_styled_paragraph_layout_evidence(
        &mut self,
        frame: &PreparedFrame,
    ) -> Vec<StyledParagraphLayoutEvidence> {
        frame_styled_paragraph_layout_evidence(&mut self.font_system, frame)
    }

    /// Renders one prepared Arcweft frame into a caller-supplied target.
    ///
    /// Surface acquisition, presentation, resize, device loss, and redraw
    /// scheduling remain the platform host's responsibility.
    pub fn render_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        frame: &PreparedFrame,
    ) -> Result<(), SharedRendererError> {
        self.viewport.update(
            queue,
            Resolution {
                width: frame.viewport.physical_width,
                height: frame.viewport.physical_height,
            },
        );

        self.prepare_text_content(device, queue, frame)?;

        let background_vertex_buffer = rectangle_vertex_buffer(
            device,
            "arcweft-shared-background-rectangle",
            frame.rectangles.get(..1).unwrap_or_default(),
            frame.viewport.logical_width,
            frame.viewport.logical_height,
        );
        let overlay_vertex_buffer = rectangle_vertex_buffer(
            device,
            "arcweft-shared-overlay-rectangles",
            frame.rectangles.get(1..).unwrap_or_default(),
            frame.viewport.logical_width,
            frame.viewport.logical_height,
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("arcweft-shared-render-frame"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arcweft-shared-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some((vertex_buffer, vertex_count)) = &background_vertex_buffer {
                draw_rectangle_buffer(
                    &mut pass,
                    &self.rectangle_pipeline,
                    vertex_buffer,
                    *vertex_count,
                );
            }
            for image in &frame.images {
                self.render_image_quad(
                    device,
                    queue,
                    &mut pass,
                    image,
                    frame.viewport.logical_width,
                    frame.viewport.logical_height,
                );
            }
        }
        for ui_scene in frame.ui_scenes() {
            self.render_ui_scene(device, queue, &mut encoder, target, frame, ui_scene)?;
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arcweft-shared-overlay-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some((vertex_buffer, vertex_count)) = &overlay_vertex_buffer {
                draw_rectangle_buffer(
                    &mut pass,
                    &self.rectangle_pipeline,
                    vertex_buffer,
                    *vertex_count,
                );
            }
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .map_err(|error| SharedRendererError::TextRender(error.to_string()))?;
        }
        queue.submit([encoder.finish()]);
        self.atlas.trim();
        Ok(())
    }

    fn render_ui_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        final_target: &wgpu::TextureView,
        frame: &PreparedFrame,
        prepared_ui: &PreparedUiScene,
    ) -> Result<(), SharedRendererError> {
        let mut mask_textures =
            WgpuPreparedUiMaskTextureProvider::prepare(device, queue, &prepared_ui.resources);
        let mut direct_renderer = self
            .ui_direct_renderer
            .for_resources(&prepared_ui.resources);
        let result = self.ui_compositor.render_scene(&mut UiCompositorFrame {
            device,
            queue,
            encoder,
            final_target,
            scene: &prepared_ui.scene,
            target_extent: UiTextureExtent::new(
                frame.viewport.physical_width,
                frame.viewport.physical_height,
            ),
            direct_renderer: &mut direct_renderer,
            mask_textures: &mut mask_textures,
        });
        result?;
        Ok(())
    }

    fn prepare_text_content(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &PreparedFrame,
    ) -> Result<(), SharedRendererError> {
        let mut block_buffers = frame
            .text
            .iter()
            .map(|block| text_buffer(&mut self.font_system, block))
            .collect::<Vec<_>>();
        let text_scale_factor = frame.viewport.physical_scale_factor_f32();
        let text_areas = block_buffers
            .iter_mut()
            .zip(&frame.text)
            .map(|(buffer, block)| text_area(buffer, block, text_scale_factor))
            .collect::<Vec<_>>();
        let paragraph_buffers = frame
            .styled_paragraphs
            .iter()
            .map(|paragraph| styled_paragraph_buffer(&mut self.font_system, paragraph))
            .collect::<Vec<_>>();
        let paragraph_areas = paragraph_buffers
            .iter()
            .zip(&frame.styled_paragraphs)
            .map(|(buffer, paragraph)| {
                styled_paragraph_text_area(buffer, paragraph, text_scale_factor)
            })
            .collect::<Vec<_>>();
        let text_areas = text_areas
            .into_iter()
            .chain(paragraph_areas)
            .collect::<Vec<_>>();
        self.text_renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                text_areas,
                &mut self.swash_cache,
            )
            .map_err(|error| SharedRendererError::TextPrepare(error.to_string()))
    }

    fn render_image_quad(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        image: &RenderImage,
        logical_width: f32,
        logical_height: f32,
    ) {
        if image.frame.width == 0 || image.frame.height == 0 || image.frame.rgba.is_empty() {
            return;
        }
        let texture = upload_image_quad(device, queue, image);
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-shared-image-bind-group"),
            layout: &self.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.image_sampler),
                },
            ],
        });
        let vertices = image_vertices(image, logical_width, logical_height);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("arcweft-shared-image-vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        pass.set_pipeline(&self.image_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..u32::try_from(vertices.len()).unwrap_or(u32::MAX), 0..1);
    }
}

fn text_buffer(font_system: &mut FontSystem, block: &RenderTextBlock) -> Buffer {
    let mut buffer = Buffer::new(
        font_system,
        Metrics::new(block.font_size, block.line_height),
    );
    buffer.set_size(
        font_system,
        Some(block.buffer_width.unwrap_or(block.bounds.width)),
        Some(block.buffer_height.unwrap_or(block.bounds.height)),
    );
    buffer.set_text(
        font_system,
        &block.text,
        &text_attrs(block),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(font_system, false);
    buffer
}

fn styled_paragraph_buffer(
    font_system: &mut FontSystem,
    paragraph: &RenderStyledParagraph,
) -> Buffer {
    let mut buffer = Buffer::new(
        font_system,
        Metrics::new(
            paragraph.default_style.font_size,
            paragraph.default_style.line_height,
        ),
    );
    buffer.set_size(
        font_system,
        Some(paragraph.bounds.width),
        Some(paragraph.bounds.height),
    );
    let default_attrs = attrs_from_style(&paragraph.default_style);
    let spans = styled_paragraph_attr_spans(paragraph);
    buffer.set_rich_text(font_system, spans, &default_attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);
    buffer
}

fn styled_paragraph_attr_spans(paragraph: &RenderStyledParagraph) -> Vec<(&str, Attrs<'_>)> {
    let mut output = Vec::new();
    let mut cursor = 0;
    let mut spans = paragraph.spans.iter().collect::<Vec<_>>();
    spans.sort_by_key(|span| span.range.start);
    for span in spans {
        let start = span.range.start.min(paragraph.text.len()).max(cursor);
        let end = span.range.end.min(paragraph.text.len());
        if cursor < start {
            push_revealed_attr_span(
                &mut output,
                paragraph,
                cursor,
                start,
                &paragraph.default_style,
            );
        }
        if start < end {
            push_revealed_attr_span(&mut output, paragraph, start, end, &span.style);
            cursor = end;
        }
    }
    if cursor < paragraph.text.len() {
        push_revealed_attr_span(
            &mut output,
            paragraph,
            cursor,
            paragraph.text.len(),
            &paragraph.default_style,
        );
    }
    if output.is_empty() {
        push_revealed_attr_span(
            &mut output,
            paragraph,
            0,
            paragraph.text.len(),
            &paragraph.default_style,
        );
    }
    output
}

fn push_revealed_attr_span<'a>(
    output: &mut Vec<(&'a str, Attrs<'a>)>,
    paragraph: &'a RenderStyledParagraph,
    start: usize,
    end: usize,
    style: &'a RenderTextStyle,
) {
    if start >= end {
        return;
    }
    let reveal = paragraph.reveal.visible_end.min(paragraph.text.len());
    if end <= reveal {
        push_attr_span(output, paragraph, start, end, style, style.color[3]);
    } else if start >= reveal {
        push_attr_span(output, paragraph, start, end, style, TRANSPARENT_ALPHA);
    } else {
        push_attr_span(output, paragraph, start, reveal, style, style.color[3]);
        push_attr_span(output, paragraph, reveal, end, style, TRANSPARENT_ALPHA);
    }
}

fn push_attr_span<'a>(
    output: &mut Vec<(&'a str, Attrs<'a>)>,
    paragraph: &'a RenderStyledParagraph,
    start: usize,
    end: usize,
    style: &'a RenderTextStyle,
    alpha: u8,
) {
    if let Some(text) = paragraph.text.get(start..end) {
        output.push((text, attrs_from_style_with_alpha(style, alpha)));
    }
}

fn styled_paragraph_text_area<'a>(
    buffer: &'a Buffer,
    paragraph: &RenderStyledParagraph,
    scale_factor: f32,
) -> TextArea<'a> {
    let scale_factor = scale_factor.max(f32::EPSILON);
    TextArea {
        buffer,
        left: paragraph.bounds.x * scale_factor,
        top: paragraph.bounds.y * scale_factor,
        scale: scale_factor,
        bounds: scale_text_bounds(paragraph.bounds, scale_factor),
        default_color: Color::rgba(
            paragraph.default_style.color[0],
            paragraph.default_style.color[1],
            paragraph.default_style.color[2],
            paragraph.default_style.color[3],
        ),
        custom_glyphs: &[],
    }
}

fn text_attrs(block: &RenderTextBlock) -> Attrs<'_> {
    let mut attrs = Attrs::new().family(render_font_family(&block.font_family));
    if block.weight == RenderTextWeight::Bold {
        attrs = attrs.weight(Weight::BOLD);
    }
    if block.slant == RenderTextSlant::Italic {
        attrs = attrs.style(Style::Italic);
    }
    attrs
}

fn attrs_from_style(style: &RenderTextStyle) -> Attrs<'_> {
    attrs_from_style_with_alpha(style, style.color[3])
}

fn attrs_from_style_with_alpha(style: &RenderTextStyle, alpha: u8) -> Attrs<'_> {
    let mut attrs = Attrs::new()
        .family(render_font_family(&style.font_family))
        .color(Color::rgba(
            style.color[0],
            style.color[1],
            style.color[2],
            alpha,
        ))
        .metrics(Metrics::new(style.font_size, style.line_height));
    if style.weight == RenderTextWeight::Bold {
        attrs = attrs.weight(Weight::BOLD);
    }
    if style.slant == RenderTextSlant::Italic {
        attrs = attrs.style(Style::Italic);
    }
    attrs
}

fn render_font_family(family: &RenderFontFamily) -> Family<'_> {
    match family {
        RenderFontFamily::Serif => Family::Serif,
        RenderFontFamily::SansSerif => Family::SansSerif,
        RenderFontFamily::Monospace => Family::Monospace,
        RenderFontFamily::Cursive => Family::Cursive,
        RenderFontFamily::Fantasy => Family::Fantasy,
        RenderFontFamily::Named(name) => Family::Name(name),
    }
}

fn text_area<'a>(buffer: &'a Buffer, block: &RenderTextBlock, scale_factor: f32) -> TextArea<'a> {
    let scale_factor = scale_factor.max(f32::EPSILON);
    let scaled_bounds = scale_text_bounds(block.clip_bounds.unwrap_or(block.bounds), scale_factor);
    TextArea {
        buffer,
        left: block.bounds.x * scale_factor,
        top: block.bounds.y * scale_factor,
        scale: scale_factor,
        bounds: scaled_bounds,
        default_color: Color::rgba(block.rgba[0], block.rgba[1], block.rgba[2], block.rgba[3]),
        custom_glyphs: &[],
    }
}

fn scale_text_bounds(bounds: HitRect, scale_factor: f32) -> TextBounds {
    let scale_factor = scale_factor.max(f32::EPSILON);
    TextBounds {
        left: pixel_floor_as_i32(bounds.x * scale_factor),
        top: pixel_floor_as_i32(bounds.y * scale_factor),
        right: pixel_ceil_as_i32((bounds.x + bounds.width) * scale_factor),
        bottom: pixel_ceil_as_i32((bounds.y + bounds.height) * scale_factor),
    }
}

/// Font context used by tools/adapters that need renderer-owned paragraph
/// evidence without owning a `SharedRenderer` instance.
///
/// The context is Sans I/O. Callers provide already-loaded font bytes.
#[derive(Debug)]
pub struct StyledParagraphEvidenceFontContext {
    font_system: FontSystem,
}

impl StyledParagraphEvidenceFontContext {
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
        }
    }

    pub fn register_font_bytes(&mut self, bytes: Vec<u8>) -> Result<(), SharedRendererError> {
        if bytes.is_empty() {
            return Err(SharedRendererError::EmptyFont);
        }
        self.font_system.db_mut().load_font_data(bytes);
        Ok(())
    }

    #[must_use]
    pub fn frame_styled_paragraph_layout_evidence(
        &mut self,
        frame: &PreparedFrame,
    ) -> Vec<StyledParagraphLayoutEvidence> {
        frame_styled_paragraph_layout_evidence(&mut self.font_system, frame)
    }

    #[must_use]
    pub fn styled_paragraph_layout_evidence(
        &mut self,
        paragraph: &RenderStyledParagraph,
    ) -> StyledParagraphLayoutEvidence {
        styled_paragraph_layout_evidence(&mut self.font_system, paragraph)
    }
}

impl Default for StyledParagraphEvidenceFontContext {
    fn default() -> Self {
        Self::new()
    }
}

fn frame_styled_paragraph_layout_evidence(
    font_system: &mut FontSystem,
    frame: &PreparedFrame,
) -> Vec<StyledParagraphLayoutEvidence> {
    frame
        .styled_paragraphs
        .iter()
        .map(|paragraph| styled_paragraph_layout_evidence(font_system, paragraph))
        .collect()
}

/// Paragraph-wide renderer-owned layout evidence consumed by text raster parity.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledParagraphLayoutEvidence {
    pub bounds: HitRect,
    pub text_len: usize,
    pub visible_end: usize,
    pub default_style: StyledParagraphStyleEvidence,
    pub spans: Vec<StyledParagraphSpanEvidence>,
    pub line_boxes: Vec<StyledParagraphLineBox>,
    pub glyph_bounds: Vec<StyledParagraphGlyphBounds>,
    pub glyph_transforms: Vec<StyledParagraphGlyphTransformEvidence>,
    pub transform_support: StyledParagraphTransformSupport,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyledParagraphStyleEvidence {
    pub font_size: f32,
    pub line_height: f32,
    pub rgba: [u8; 4],
    pub font_family: RenderFontFamily,
    pub weight: RenderTextWeight,
    pub slant: RenderTextSlant,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyledParagraphSpanEvidence {
    pub range: RichTextRange,
    pub node_index: usize,
    pub style: StyledParagraphStyleEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyledParagraphLineBox {
    pub line_index: usize,
    pub bounds: HitRect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyledParagraphRevealState {
    Visible,
    PartiallyVisible,
    Hidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyledParagraphTransformSupport {
    NoTransforms,
    MetadataOnlyUnsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyledParagraphGlyphTransformRenderSupport {
    MetadataOnlyUnsupported,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyledParagraphGlyphTransformEvidence {
    pub range: RichTextRange,
    pub node_index: usize,
    pub motion: RenderGlyphMotion,
    pub sampled_offset_y: f32,
    pub rendered: bool,
    pub render_support: StyledParagraphGlyphTransformRenderSupport,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyledParagraphGlyphBounds {
    pub line_index: usize,
    pub source_range: RichTextRange,
    pub bounds: HitRect,
    pub visible: bool,
    pub reveal_state: StyledParagraphRevealState,
    pub style: StyledParagraphStyleEvidence,
    pub glyph_transform: Option<StyledParagraphGlyphTransformEvidence>,
}

pub fn styled_paragraph_layout_evidence(
    font_system: &mut FontSystem,
    paragraph: &RenderStyledParagraph,
) -> StyledParagraphLayoutEvidence {
    let buffer = styled_paragraph_buffer(font_system, paragraph);
    let visible_end = paragraph.reveal.visible_end.min(paragraph.text.len());
    let seconds = visual_seconds(paragraph.visual_time_millis);
    let mut line_boxes = Vec::new();
    let mut glyph_bounds = Vec::new();
    for (line_index, run) in buffer.layout_runs().enumerate() {
        line_boxes.push(StyledParagraphLineBox {
            line_index,
            bounds: HitRect::new(
                paragraph.bounds.x,
                paragraph.bounds.y + run.line_top,
                run.line_w,
                run.line_height,
            ),
        });
        glyph_bounds.extend(run.glyphs.iter().map(|glyph| {
            let source_range = RichTextRange::new(glyph.start, glyph.end);
            let reveal_state = reveal_state(source_range, visible_end);
            StyledParagraphGlyphBounds {
                line_index,
                source_range,
                bounds: HitRect::new(
                    paragraph.bounds.x + glyph.x,
                    paragraph.bounds.y + run.line_top,
                    glyph.w,
                    run.line_height,
                ),
                visible: !matches!(reveal_state, StyledParagraphRevealState::Hidden),
                reveal_state,
                style: style_evidence(style_for_source_range(paragraph, source_range)),
                glyph_transform: glyph_transform_evidence_for_range(
                    paragraph,
                    source_range,
                    seconds,
                ),
            }
        }));
    }
    let glyph_transforms = paragraph
        .glyph_transforms
        .iter()
        .map(|transform| glyph_transform_span_evidence(transform, seconds))
        .collect::<Vec<_>>();
    StyledParagraphLayoutEvidence {
        bounds: paragraph.bounds,
        text_len: paragraph.text.len(),
        visible_end,
        default_style: style_evidence(&paragraph.default_style),
        spans: paragraph.spans.iter().map(span_evidence).collect(),
        line_boxes,
        glyph_bounds,
        transform_support: if glyph_transforms.is_empty() {
            StyledParagraphTransformSupport::NoTransforms
        } else {
            StyledParagraphTransformSupport::MetadataOnlyUnsupported
        },
        glyph_transforms,
    }
}

fn span_evidence(span: &RenderStyledTextSpan) -> StyledParagraphSpanEvidence {
    StyledParagraphSpanEvidence {
        range: span.range,
        node_index: span.node_index,
        style: style_evidence(&span.style),
    }
}

fn style_evidence(style: &RenderTextStyle) -> StyledParagraphStyleEvidence {
    StyledParagraphStyleEvidence {
        font_size: style.font_size,
        line_height: style.line_height,
        rgba: style.color,
        font_family: style.font_family.clone(),
        weight: style.weight,
        slant: style.slant,
    }
}

fn style_for_source_range(
    paragraph: &RenderStyledParagraph,
    range: RichTextRange,
) -> &RenderTextStyle {
    let byte = range.start.min(paragraph.text.len().saturating_sub(1));
    paragraph
        .spans
        .iter()
        .find(|span| span.range.start <= byte && byte < span.range.end)
        .map_or(&paragraph.default_style, |span| &span.style)
}

fn reveal_state(range: RichTextRange, visible_end: usize) -> StyledParagraphRevealState {
    if range.end <= visible_end {
        StyledParagraphRevealState::Visible
    } else if range.start >= visible_end {
        StyledParagraphRevealState::Hidden
    } else {
        StyledParagraphRevealState::PartiallyVisible
    }
}

fn glyph_transform_evidence_for_range(
    paragraph: &RenderStyledParagraph,
    range: RichTextRange,
    seconds: f32,
) -> Option<StyledParagraphGlyphTransformEvidence> {
    paragraph
        .glyph_transforms
        .iter()
        .find(|transform| ranges_intersect(transform.range, range))
        .map(|transform| StyledParagraphGlyphTransformEvidence {
            range,
            node_index: transform.node_index,
            motion: transform.motion,
            sampled_offset_y: transform.motion.offset_y(seconds, range.start),
            rendered: false,
            render_support: StyledParagraphGlyphTransformRenderSupport::MetadataOnlyUnsupported,
        })
}

fn glyph_transform_span_evidence(
    transform: &RenderGlyphTransformSpan,
    seconds: f32,
) -> StyledParagraphGlyphTransformEvidence {
    StyledParagraphGlyphTransformEvidence {
        range: transform.range,
        node_index: transform.node_index,
        motion: transform.motion,
        sampled_offset_y: transform.motion.offset_y(seconds, transform.range.start),
        rendered: false,
        render_support: StyledParagraphGlyphTransformRenderSupport::MetadataOnlyUnsupported,
    }
}

fn ranges_intersect(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

fn visual_seconds(visual_time_millis: u64) -> f32 {
    visual_time_millis.to_f32().unwrap_or(f32::MAX) / 1_000.0
}

fn rectangle_vertex_buffer(
    device: &wgpu::Device,
    label: &'static str,
    rectangles: &[PaintRect],
    width: f32,
    height: f32,
) -> Option<(wgpu::Buffer, u32)> {
    let vertices = rectangle_vertices(rectangles, width, height);
    (!vertices.is_empty()).then(|| {
        let count = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        (buffer, count)
    })
}

fn draw_rectangle_buffer(
    pass: &mut wgpu::RenderPass<'_>,
    pipeline: &wgpu::RenderPipeline,
    vertex_buffer: &wgpu::Buffer,
    vertex_count: u32,
) {
    pass.set_pipeline(pipeline);
    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    pass.draw(0..vertex_count, 0..1);
}

fn rectangle_vertices(rectangles: &[PaintRect], width: f32, height: f32) -> Vec<RectVertex> {
    rectangles
        .iter()
        .flat_map(|rect| {
            let left = (rect.bounds.x / width) * 2.0 - 1.0;
            let right = ((rect.bounds.x + rect.bounds.width) / width) * 2.0 - 1.0;
            let top = 1.0 - (rect.bounds.y / height) * 2.0;
            let bottom = 1.0 - ((rect.bounds.y + rect.bounds.height) / height) * 2.0;
            let color = rect.rgba;
            [
                RectVertex {
                    position: [left, top],
                    color,
                },
                RectVertex {
                    position: [left, bottom],
                    color,
                },
                RectVertex {
                    position: [right, bottom],
                    color,
                },
                RectVertex {
                    position: [left, top],
                    color,
                },
                RectVertex {
                    position: [right, bottom],
                    color,
                },
                RectVertex {
                    position: [right, top],
                    color,
                },
            ]
        })
        .collect()
}

fn rectangle_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("arcweft-shared-rectangle-shader"),
        source: wgpu::ShaderSource::Wgsl(RECTANGLE_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("arcweft-shared-rectangle-layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("arcweft-shared-rectangle-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<RectVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: std::mem::size_of::<[f32; 2]>() as u64,
                        shader_location: 1,
                    },
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn image_quad_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline, wgpu::Sampler) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("arcweft-shared-image-shader"),
        source: wgpu::ShaderSource::Wgsl(IMAGE_SHADER.into()),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("arcweft-shared-image-bind-group-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("arcweft-shared-image-pipeline-layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("arcweft-shared-image-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<ImageVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: std::mem::size_of::<[f32; 2]>() as u64,
                        shader_location: 1,
                    },
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("arcweft-shared-image-sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    (bind_group_layout, pipeline, sampler)
}

fn upload_image_quad(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    image: &RenderImage,
) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("arcweft-shared-image-texture"),
        size: wgpu::Extent3d {
            width: image.frame.width,
            height: image.frame.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let rgba = image_upload_rgba(image);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba.as_ref(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(image.frame.width.saturating_mul(4)),
            rows_per_image: Some(image.frame.height),
        },
        wgpu::Extent3d {
            width: image.frame.width,
            height: image.frame.height,
            depth_or_array_layers: 1,
        },
    );
    texture
}

fn image_upload_rgba(image: &RenderImage) -> Cow<'_, [u8]> {
    if image.opacity_milli >= 1_000 {
        return Cow::Borrowed(image.frame.rgba.as_slice());
    }
    Cow::Owned(
        image
            .frame
            .rgba
            .chunks_exact(4)
            .flat_map(|pixel| {
                [
                    pixel[0],
                    pixel[1],
                    pixel[2],
                    scaled_alpha_milli(pixel[3], image.opacity_milli),
                ]
            })
            .collect(),
    )
}

fn scaled_alpha_milli(alpha: u8, opacity_milli: u16) -> u8 {
    let value = u32::from(alpha) * u32::from(opacity_milli) / 1_000;
    u8::try_from(value.min(u32::from(u8::MAX))).unwrap_or(u8::MAX)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ImageQuad {
    rect: HitRect,
    uv_left: f32,
    uv_top: f32,
    uv_right: f32,
    uv_bottom: f32,
}

fn image_vertices(image: &RenderImage, width: f32, height: f32) -> [ImageVertex; 6] {
    let quad = image_quad(image);
    let top_left = transform_image_point(image, quad.rect.x, quad.rect.y);
    let bottom_left = transform_image_point(image, quad.rect.x, quad.rect.y + quad.rect.height);
    let bottom_right = transform_image_point(
        image,
        quad.rect.x + quad.rect.width,
        quad.rect.y + quad.rect.height,
    );
    let top_right = transform_image_point(image, quad.rect.x + quad.rect.width, quad.rect.y);
    let top_left = normalized_point(top_left, width, height);
    let bottom_left = normalized_point(bottom_left, width, height);
    let bottom_right = normalized_point(bottom_right, width, height);
    let top_right = normalized_point(top_right, width, height);
    [
        ImageVertex {
            position: top_left,
            uv: [quad.uv_left, quad.uv_top],
        },
        ImageVertex {
            position: bottom_left,
            uv: [quad.uv_left, quad.uv_bottom],
        },
        ImageVertex {
            position: bottom_right,
            uv: [quad.uv_right, quad.uv_bottom],
        },
        ImageVertex {
            position: top_left,
            uv: [quad.uv_left, quad.uv_top],
        },
        ImageVertex {
            position: bottom_right,
            uv: [quad.uv_right, quad.uv_bottom],
        },
        ImageVertex {
            position: top_right,
            uv: [quad.uv_right, quad.uv_top],
        },
    ]
}

#[allow(clippy::cast_precision_loss)]
fn image_quad(image: &RenderImage) -> ImageQuad {
    let bounds = image.bounds;
    let source_width = image.frame.width.max(1) as f32;
    let source_height = image.frame.height.max(1) as f32;
    let align_x = alignment_factor(image.alignment.x_milli());
    let align_y = alignment_factor(image.alignment.y_milli());
    match image.fit {
        ImageObjectFit::Stretch => ImageQuad {
            rect: bounds,
            uv_left: 0.0,
            uv_top: 0.0,
            uv_right: 1.0,
            uv_bottom: 1.0,
        },
        ImageObjectFit::Intrinsic => {
            aligned_quad(bounds, source_width, source_height, align_x, align_y)
        }
        ImageObjectFit::Contain => {
            let scale = (bounds.width / source_width)
                .min(bounds.height / source_height)
                .max(f32::EPSILON);
            aligned_quad(
                bounds,
                source_width * scale,
                source_height * scale,
                align_x,
                align_y,
            )
        }
        ImageObjectFit::Cover => cover_quad(bounds, source_width, source_height, align_x, align_y),
    }
}

fn aligned_quad(bounds: HitRect, width: f32, height: f32, align_x: f32, align_y: f32) -> ImageQuad {
    ImageQuad {
        rect: HitRect::new(
            bounds.x + (bounds.width - width) * align_x,
            bounds.y + (bounds.height - height) * align_y,
            width,
            height,
        ),
        uv_left: 0.0,
        uv_top: 0.0,
        uv_right: 1.0,
        uv_bottom: 1.0,
    }
}

fn cover_quad(
    bounds: HitRect,
    source_width: f32,
    source_height: f32,
    align_x: f32,
    align_y: f32,
) -> ImageQuad {
    let source_ratio = source_width / source_height;
    let target_ratio = bounds.width / bounds.height;
    if source_ratio > target_ratio {
        let visible_width = (target_ratio / source_ratio).clamp(0.0, 1.0);
        let uv_left = (1.0 - visible_width) * align_x;
        ImageQuad {
            rect: bounds,
            uv_left,
            uv_top: 0.0,
            uv_right: uv_left + visible_width,
            uv_bottom: 1.0,
        }
    } else {
        let visible_height = (source_ratio / target_ratio).clamp(0.0, 1.0);
        let uv_top = (1.0 - visible_height) * align_y;
        ImageQuad {
            rect: bounds,
            uv_left: 0.0,
            uv_top,
            uv_right: 1.0,
            uv_bottom: uv_top + visible_height,
        }
    }
}

#[allow(clippy::cast_precision_loss)]
fn alignment_factor(milli: i32) -> f32 {
    (milli.clamp(0, 1_000) as f32) / 1_000.0
}

#[allow(clippy::cast_precision_loss)]
fn transform_image_point(image: &RenderImage, x: f32, y: f32) -> [f32; 2] {
    let transform = image.transform;
    let m11 = transform.m11_milli as f32 / 1_000.0;
    let m12 = transform.m12_milli as f32 / 1_000.0;
    let m21 = transform.m21_milli as f32 / 1_000.0;
    let m22 = transform.m22_milli as f32 / 1_000.0;
    let tx = transform.tx_milli as f32 / 1_000.0;
    let ty = transform.ty_milli as f32 / 1_000.0;
    [m11 * x + m12 * y + tx, m21 * x + m22 * y + ty]
}

fn normalized_point(point: [f32; 2], width: f32, height: f32) -> [f32; 2] {
    [
        (point[0] / width) * 2.0 - 1.0,
        1.0 - (point[1] / height) * 2.0,
    ]
}

const RECTANGLE_SHADER: &str = r"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
";

const IMAGE_SHADER: &str = r"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0)
var image_texture: texture_2d<f32>;

@group(0) @binding(1)
var image_sampler: sampler;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(image_texture, image_sampler, in.uv);
}
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_bounds_are_scaled_to_physical_pixels() {
        let bounds = HitRect::new(10.25, 20.5, 100.25, 40.25);

        assert_eq!(
            scale_text_bounds(bounds, 2.0),
            TextBounds {
                left: 20,
                top: 41,
                right: 221,
                bottom: 122,
            }
        );
    }

    #[test]
    fn text_bounds_keep_default_scale_pixel_rounding() {
        let bounds = HitRect::new(10.25, 20.5, 100.25, 40.25);

        assert_eq!(
            scale_text_bounds(bounds, 1.0),
            TextBounds {
                left: 10,
                top: 20,
                right: 111,
                bottom: 61,
            }
        );
    }
}
