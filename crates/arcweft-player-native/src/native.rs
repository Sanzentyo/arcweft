//! Minimal native window renderer for rich-text player frames.

use arcweft_glyphon::{
    GlyphonAreaOptions, OwnedGlyphArea, ResolvedGlyph, glyph_area_from_layout,
    glyph_area_from_shaped_buffer, vertical_glyph_area_from_shaped_buffer,
};
use arcweft_render_text::{
    LineDisplayFrame, Milli, RichTextColor, RichTextControl, RichTextDisplayMap,
    RichTextEffectDescriptor, RichTextEffectPhase, RichTextEffectTarget, RichTextFontFamily,
    RichTextNode, RichTextParam, RichTextPresentation, RichTextRange, RichTextShaderRef,
    RichTextStyle, RichTextWritingMode, parse_decimal_milli, presentation_from_styles,
};
use arcweft_text_layout::{
    GlyphOrientation, GlyphVerticalForm, LaidOutGlyph, LaidOutText, LayoutPoint, LayoutRect,
    LayoutSize, TextLayoutConfig, layout_frame,
};
use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, Style,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Vector, Viewport, Weight,
    cosmic_text::{FeatureTag, FontFeatures},
};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use std::sync::mpsc;
use thiserror::Error;
use wgpu::{
    BufferDescriptor, BufferUsages, COPY_BYTES_PER_ROW_ALIGNMENT, CommandEncoderDescriptor,
    CompositeAlphaMode, DeviceDescriptor, Extent3d, Instance, LoadOp, MapMode, MultisampleState,
    Operations, Origin3d, PollType, PresentMode, RenderPassColorAttachment, RenderPassDescriptor,
    RequestAdapterOptions, SurfaceConfiguration, TexelCopyBufferInfo, TexelCopyBufferLayout,
    TexelCopyTextureInfo, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureViewDescriptor,
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes},
};

/// Native player window error.
#[derive(Debug, Error)]
pub enum NativeWindowError {
    #[error("event loop error: {0}")]
    EventLoop(String),
    #[error("no display pages were provided")]
    EmptyPages,
    #[error("readback failed: {0}")]
    Readback(String),
    #[error("text layout failed: {0}")]
    TextLayout(String),
}

/// Raw framebuffer capture produced by the native rich-text renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFrameCapture {
    /// Capture width in pixels.
    pub width: u32,
    /// Capture height in pixels.
    pub height: u32,
    /// Unpadded RGBA8 pixels in row-major order.
    pub rgba: Vec<u8>,
    /// Bounding box of pixels that differ from the clear background.
    pub content_bbox: Option<NativeFrameContentBBox>,
    /// Count of pixels that differ from the clear background.
    pub content_pixels: u64,
}

/// Pixel-space bounds of non-background framebuffer content.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct NativeFrameContentBBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Native rich-text element kinds addressable by Agent debug captures.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NativeFrameElement {
    TextRun {
        index: usize,
    },
    Ruby {
        index: usize,
    },
    GlyphCluster {
        index: usize,
        range_start: usize,
        range_end: usize,
    },
}

/// Glyph orientation metadata attached to native glyph-cluster bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGlyphOrientation {
    Upright,
    SidewaysCw,
    TextCombineUpright,
}

impl From<GlyphOrientation> for NativeGlyphOrientation {
    fn from(value: GlyphOrientation) -> Self {
        match value {
            GlyphOrientation::Upright => Self::Upright,
            GlyphOrientation::SidewaysCw => Self::SidewaysCw,
            GlyphOrientation::TextCombineUpright => Self::TextCombineUpright,
        }
    }
}

/// Vertical alternate shaping metadata attached to native glyph-cluster bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGlyphVerticalForm {
    None,
    UprightAlternate,
    RotatedAlternate,
}

impl From<GlyphVerticalForm> for NativeGlyphVerticalForm {
    fn from(value: GlyphVerticalForm) -> Self {
        match value {
            GlyphVerticalForm::None => Self::None,
            GlyphVerticalForm::UprightAlternate => Self::UprightAlternate,
            GlyphVerticalForm::RotatedAlternate => Self::RotatedAlternate,
        }
    }
}

/// Glyph-cluster debug metadata returned alongside native element bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGlyphClusterMetadata {
    pub orientation: NativeGlyphOrientation,
    pub vertical_form: NativeGlyphVerticalForm,
}

/// Native ruby geometry before it is unioned into an object crop bbox.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRubyElementGeometry {
    pub base_bbox: NativeFrameContentBBox,
    pub annotation_bbox: NativeFrameContentBBox,
}

/// Pixel-space bounds for one native rich-text element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeFrameElementBounds {
    pub element: NativeFrameElement,
    pub bbox: NativeFrameContentBBox,
    pub glyph: Option<NativeGlyphClusterMetadata>,
    pub ruby: Option<NativeRubyElementGeometry>,
}

/// Debug-image region rendered by the native adapter for Agent capture tools.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeFrameDebugRegion {
    /// Optional native rich-text element whose shaped bounds should override the fallback box.
    pub element: Option<NativeFrameElement>,
    /// Fallback bounds used for non-text objects or when an element is not visible on this page.
    pub fallback_bbox: NativeFrameContentBBox,
    /// RGBA color written into the debug image.
    pub color: [u8; 4],
}

/// Deterministic rich-text visual plan consumed by native debug and slow render paths.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct NativeVisualPlan {
    pub pages: Vec<NativeVisualPage>,
}

/// One rendered text page in native visual coordinates.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct NativeVisualPage {
    pub page_index: usize,
    pub text: String,
    pub runs: Vec<NativeVisualRun>,
    pub glyphs: Vec<NativeGlyphPlacement>,
    pub shaders: Vec<NativeResolvedShaderFilter>,
}

/// One rich-text display-map run with presentation metadata.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct NativeVisualRun {
    pub source_run_index: usize,
    pub range: Range<usize>,
    pub local_range: Range<usize>,
    pub presentation: RichTextPresentation,
}

/// CPU-side glyph placement used for deterministic debugging and effect tests.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct NativeGlyphPlacement {
    pub run_index: usize,
    pub glyph_index: usize,
    pub range: Range<usize>,
    pub x: f32,
    pub y: f32,
    pub rotate_degrees: f32,
    pub vertical_form: GlyphVerticalForm,
    pub scale_x: f32,
    pub scale_y: f32,
    pub opacity: f32,
}

/// Host-resolved shader/filter reference for one native page.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct NativeResolvedShaderFilter {
    pub id: String,
    pub phase: RichTextEffectPhase,
    pub amount: f32,
    pub direction: [f32; 2],
}

/// Key used by renderer-local rich-text state stores.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RichTextStateScopeKey {
    Document,
    Line(String),
    Sentence {
        line: String,
        sentence_index: usize,
    },
    Run {
        line: String,
        run_index: usize,
    },
    Glyph {
        line: String,
        run_index: usize,
        glyph_index: usize,
    },
}

/// Renderer-local shared value for stateful rich-text effects.
#[derive(Clone, Debug, PartialEq)]
pub enum SharedTextValue {
    Bool(bool),
    I64(i64),
    F32(f32),
    Vec2([f32; 2]),
    Text(String),
}

/// Renderer-local state store. It is intentionally not part of `LineDisplayFrame`.
#[derive(Default)]
pub struct RichTextStateStore {
    values: BTreeMap<(RichTextStateScopeKey, String), SharedTextValue>,
}

impl RichTextStateStore {
    pub fn get(&self, scope: &RichTextStateScopeKey, name: &str) -> Option<&SharedTextValue> {
        self.values.get(&(scope.clone(), name.to_owned()))
    }

    pub fn set(
        &mut self,
        scope: RichTextStateScopeKey,
        name: impl Into<String>,
        value: SharedTextValue,
    ) {
        self.values.insert((scope, name.into()), value);
    }
}

/// Per-glyph effect context supplied to renderer-local effect implementations.
pub struct TextEffectGlyphContext<'a> {
    pub time_seconds: f32,
    pub line_id: &'a str,
    pub run_index: usize,
    pub glyph_index: usize,
    pub glyph_count: usize,
    pub state: &'a mut RichTextStateStore,
    pub placement: &'a mut NativeGlyphPlacement,
}

/// Renderer-local stateful rich-text effect.
pub trait RichTextEffectClass: Send {
    fn apply_glyph(&mut self, ctx: &mut TextEffectGlyphContext<'_>);
}

pub type GlyphLambda = Box<dyn FnMut(&mut TextEffectGlyphContext<'_>) + Send + 'static>;

pub enum RegisteredTextEffect {
    Class(Box<dyn RichTextEffectClass>),
    Lambda(GlyphLambda),
}

/// Registry that resolves `RichTextEffectDescriptor::id` to native execution.
#[derive(Default)]
pub struct RichTextEffectRegistry {
    effects: BTreeMap<String, RegisteredTextEffect>,
}

impl RichTextEffectRegistry {
    pub fn insert_class(
        &mut self,
        id: impl Into<String>,
        effect: impl RichTextEffectClass + 'static,
    ) {
        self.effects
            .insert(id.into(), RegisteredTextEffect::Class(Box::new(effect)));
    }

    pub fn insert_lambda(
        &mut self,
        id: impl Into<String>,
        effect: impl FnMut(&mut TextEffectGlyphContext<'_>) + Send + 'static,
    ) {
        self.effects
            .insert(id.into(), RegisteredTextEffect::Lambda(Box::new(effect)));
    }

    pub fn apply_host_effect(&mut self, id: &str, ctx: &mut TextEffectGlyphContext<'_>) {
        let Some(effect) = self.effects.get_mut(id) else {
            return;
        };
        match effect {
            RegisteredTextEffect::Class(effect) => effect.apply_glyph(ctx),
            RegisteredTextEffect::Lambda(effect) => effect(ctx),
        }
    }
}

/// Viewport and page selection for native offscreen captures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCaptureViewport {
    pub width: u32,
    pub height: u32,
    pub left: f32,
    pub top: f32,
    pub page_index: usize,
    pub time_seconds: f32,
}

impl NativeCaptureViewport {
    /// Builds a capture viewport for one rendered rich-text page.
    pub const fn new(width: u32, height: u32, left: f32, top: f32, page_index: usize) -> Self {
        Self {
            width,
            height,
            left,
            top,
            page_index,
            time_seconds: 60.0,
        }
    }

    /// Overrides the capture time used by visibility-only glyph effects.
    #[must_use]
    pub const fn with_time_seconds(mut self, time_seconds: f32) -> Self {
        self.time_seconds = time_seconds;
        self
    }
}

/// Reusable offscreen native text capture state for repeated debug reads.
///
/// The one-shot `capture_frame_*` helpers create this internally. Long-lived
/// tooling adapters should keep a session and call its methods so repeated
/// framebuffer, layer, and object captures reuse the same device and text atlas.
pub struct NativeOffscreenCaptureSession {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: TextureFormat,
    renderer: NativeOffscreenTextRenderer,
}

impl NativeOffscreenCaptureSession {
    /// Creates a reusable offscreen capture session.
    pub fn new() -> Result<Self, NativeWindowError> {
        let (device, queue) = pollster::block_on(request_capture_device())?;
        let format = TextureFormat::Rgba8UnormSrgb;
        let renderer = NativeOffscreenTextRenderer::new(&device, &queue, format);
        Ok(Self {
            device,
            queue,
            format,
            renderer,
        })
    }

    /// Renders the first page of a rich-text frame at a viewport origin.
    pub fn capture_frame_rgba_at(
        &mut self,
        frame: &LineDisplayFrame,
        width: u32,
        height: u32,
        left: f32,
        top: f32,
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        self.capture_frame_rgba_in(
            frame,
            NativeCaptureViewport::new(width, height, left, top, 0),
        )
    }

    /// Renders a page of a rich-text frame within a viewport.
    pub fn capture_frame_rgba_in(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let page_range = display_map_non_empty_page_range_at(frame, viewport.page_index)?;
        let page_layout = layout_page_range(
            frame,
            page_range.clone(),
            native_text_layout_config(width, height, viewport.left, viewport.top),
        )?;
        let Some(page) = page_from_display_map_range(frame, page_range) else {
            return Err(NativeWindowError::EmptyPages);
        };
        self.capture_rich_text_rgba(
            &page.rich_text,
            NativeRenderLayout::glyph_area(&page_layout.layout),
            width,
            height,
            NativeTextOrigin {
                left: viewport.left,
                top: viewport.top,
            },
            viewport.time_seconds,
        )
    }

    /// Builds a native-layout debug capture for object-id and mask capture modes.
    pub fn capture_frame_debug_regions_at(
        &mut self,
        frame: &LineDisplayFrame,
        width: u32,
        height: u32,
        left: f32,
        top: f32,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        self.capture_frame_debug_regions_in(
            frame,
            NativeCaptureViewport::new(width, height, left, top, 0),
            regions,
        )
    }

    /// Builds a native-layout debug capture for object-id and mask capture modes on a page.
    pub fn capture_frame_debug_regions_in(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let background = [0, 0, 0, 0];
        let mut rgba = self
            .capture_debug_text_regions_rgba_at(frame, viewport, regions)?
            .unwrap_or_else(|| solid_rgba(width, height, background));
        for region in regions {
            if region.element.is_none() {
                fill_native_rect(&mut rgba, width, height, region.fallback_bbox, region.color);
            }
        }
        let stats = native_frame_content_stats(&rgba, width, height, background);
        Ok(NativeFrameCapture {
            width,
            height,
            rgba,
            content_bbox: stats.content_bbox,
            content_pixels: stats.content_pixels,
        })
    }

    /// Builds an isolated native-layout color capture for selected rich-text regions.
    pub fn capture_frame_color_regions_at(
        &mut self,
        frame: &LineDisplayFrame,
        width: u32,
        height: u32,
        left: f32,
        top: f32,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        self.capture_frame_color_regions_in(
            frame,
            NativeCaptureViewport::new(width, height, left, top, 0),
            regions,
        )
    }

    /// Builds an isolated native-layout color capture for selected rich-text regions on a page.
    pub fn capture_frame_color_regions_in(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let background = [0, 0, 0, 0];
        let rgba = self
            .capture_color_text_regions_rgba_at(frame, viewport, regions)?
            .unwrap_or_else(|| solid_rgba(width, height, background));
        let stats = native_frame_content_stats(&rgba, width, height, background);
        Ok(NativeFrameCapture {
            width,
            height,
            rgba,
            content_bbox: stats.content_bbox,
            content_pixels: stats.content_pixels,
        })
    }

    fn capture_rich_text_rgba(
        &mut self,
        rich_text: &WindowRichText,
        layout: NativeRenderLayout<'_>,
        width: u32,
        height: u32,
        origin: NativeTextOrigin,
        time_seconds: f32,
    ) -> Result<NativeFrameCapture, NativeWindowError> {
        let rgba = self.render_rich_text_rgba_with_clear(
            rich_text,
            layout,
            NativeRenderTarget {
                width,
                height,
                origin,
                time_seconds,
                force_alpha_mask: false,
            },
            wgpu::Color::BLACK,
        )?;
        let stats = native_frame_content_stats(&rgba, width, height, [0, 0, 0, 255]);
        Ok(NativeFrameCapture {
            width,
            height,
            rgba,
            content_bbox: stats.content_bbox,
            content_pixels: stats.content_pixels,
        })
    }

    fn capture_debug_text_regions_rgba_at(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<Option<Vec<u8>>, NativeWindowError> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let origin = NativeTextOrigin {
            left: viewport.left,
            top: viewport.top,
        };
        let page_range = display_map_non_empty_page_range_at(frame, viewport.page_index)?;
        let page_layout = layout_page_range(
            frame,
            page_range.clone(),
            native_text_layout_config(width, height, origin.left, origin.top),
        )?;
        let Some(page) = page_from_display_map_range(frame, page_range.clone()) else {
            return Err(NativeWindowError::EmptyPages);
        };
        let Some(rich_text) =
            debug_rich_text_for_regions(frame, &page_range, &page.rich_text, regions)
        else {
            return Ok(None);
        };
        let mut rgba = self.render_rich_text_rgba_with_clear(
            &rich_text,
            NativeRenderLayout::glyph_area(&page_layout.layout),
            NativeRenderTarget {
                width,
                height,
                origin,
                time_seconds: viewport.time_seconds,
                force_alpha_mask: true,
            },
            wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
        )?;
        clear_transparent_rgb(&mut rgba);
        Ok(Some(rgba))
    }

    fn capture_color_text_regions_rgba_at(
        &mut self,
        frame: &LineDisplayFrame,
        viewport: NativeCaptureViewport,
        regions: &[NativeFrameDebugRegion],
    ) -> Result<Option<Vec<u8>>, NativeWindowError> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let origin = NativeTextOrigin {
            left: viewport.left,
            top: viewport.top,
        };
        let page_range = display_map_non_empty_page_range_at(frame, viewport.page_index)?;
        let page_layout = layout_page_range(
            frame,
            page_range.clone(),
            native_text_layout_config(width, height, origin.left, origin.top),
        )?;
        let Some(page) = page_from_display_map_range(frame, page_range.clone()) else {
            return Err(NativeWindowError::EmptyPages);
        };
        let Some(rich_text) =
            color_rich_text_for_regions(frame, &page_range, &page.rich_text, regions)
        else {
            return Ok(None);
        };
        let mut rgba = self.render_rich_text_rgba_with_clear(
            &rich_text,
            NativeRenderLayout::glyph_area(&page_layout.layout),
            NativeRenderTarget {
                width,
                height,
                origin,
                time_seconds: viewport.time_seconds,
                force_alpha_mask: false,
            },
            wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
        )?;
        clear_transparent_rgb(&mut rgba);
        Ok(Some(rgba))
    }

    fn render_rich_text_rgba_with_clear(
        &mut self,
        rich_text: &WindowRichText,
        layout: NativeRenderLayout<'_>,
        target: NativeRenderTarget,
        clear: wgpu::Color,
    ) -> Result<Vec<u8>, NativeWindowError> {
        self.renderer
            .prepare(&self.device, &self.queue, rich_text, layout, target)?;
        let texture = self.renderer.render_texture_with_clear(
            &self.device,
            &self.queue,
            target.width,
            target.height,
            self.format,
            clear,
        )?;
        readback_texture_rgba(
            &self.device,
            &self.queue,
            &texture,
            target.width,
            target.height,
        )
    }
}

/// Opens a native window and renders one text frame.
pub fn run_text_window(title: &str, text: &str) -> Result<(), NativeWindowError> {
    run_pages_window(title, vec![WindowPage::plain(text)])
}

/// Opens a native window and renders one rich text frame.
pub fn run_frame_window(title: &str, frame: &LineDisplayFrame) -> Result<(), NativeWindowError> {
    run_pages_window(title, WindowPage::from_frame(frame))
}

/// Opens a native window and renders rich text frames with page advancement.
pub fn run_frames_window(
    title: &str,
    frames: &[LineDisplayFrame],
) -> Result<(), NativeWindowError> {
    run_pages_window(
        title,
        frames.iter().flat_map(WindowPage::from_frame).collect(),
    )
}

/// Renders the first page of a rich-text frame to an offscreen texture and reads it back.
pub fn capture_frame_rgba(
    frame: &LineDisplayFrame,
    width: u32,
    height: u32,
) -> Result<NativeFrameCapture, NativeWindowError> {
    capture_frame_rgba_at(frame, width, height, NATIVE_TEXT_LEFT, NATIVE_TEXT_TOP)
}

/// Renders the first page of a rich-text frame at a viewport origin and reads it back.
pub fn capture_frame_rgba_at(
    frame: &LineDisplayFrame,
    width: u32,
    height: u32,
    left: f32,
    top: f32,
) -> Result<NativeFrameCapture, NativeWindowError> {
    capture_frame_rgba_at_page(frame, width, height, left, top, 0)
}

/// Renders a page of a rich-text frame at a viewport origin and reads it back.
pub fn capture_frame_rgba_at_page(
    frame: &LineDisplayFrame,
    width: u32,
    height: u32,
    left: f32,
    top: f32,
    page_index: usize,
) -> Result<NativeFrameCapture, NativeWindowError> {
    NativeOffscreenCaptureSession::new()?.capture_frame_rgba_in(
        frame,
        NativeCaptureViewport::new(width, height, left, top, page_index),
    )
}

/// Measures first-page rich-text element bounds using the same native text layout as rendering.
pub fn measure_frame_elements_at(
    frame: &LineDisplayFrame,
    width: u32,
    height: u32,
    left: f32,
    top: f32,
) -> Result<Vec<NativeFrameElementBounds>, NativeWindowError> {
    measure_frame_elements_at_page(frame, width, height, left, top, 0)
}

/// Measures native rich-text element bounds for a rendered page.
pub fn measure_frame_elements_at_page(
    frame: &LineDisplayFrame,
    width: u32,
    height: u32,
    left: f32,
    top: f32,
    page_index: usize,
) -> Result<Vec<NativeFrameElementBounds>, NativeWindowError> {
    let page_range = display_map_non_empty_page_range_at(frame, page_index)?;
    let page_layout = layout_page_range(
        frame,
        page_range,
        native_text_layout_config(width.max(1), height.max(1), left, top),
    )?;
    Ok(native_element_bounds_from_layout(
        &page_layout,
        width.max(1),
        height.max(1),
    ))
}

/// Builds a native-layout debug capture for object-id and mask capture modes.
pub fn capture_frame_debug_regions_at(
    frame: &LineDisplayFrame,
    width: u32,
    height: u32,
    left: f32,
    top: f32,
    regions: &[NativeFrameDebugRegion],
) -> Result<NativeFrameCapture, NativeWindowError> {
    capture_frame_debug_regions_at_page(frame, width, height, left, top, 0, regions)
}

/// Builds a native-layout debug capture for object-id and mask capture modes on a page.
pub fn capture_frame_debug_regions_at_page(
    frame: &LineDisplayFrame,
    width: u32,
    height: u32,
    left: f32,
    top: f32,
    page_index: usize,
    regions: &[NativeFrameDebugRegion],
) -> Result<NativeFrameCapture, NativeWindowError> {
    NativeOffscreenCaptureSession::new()?.capture_frame_debug_regions_in(
        frame,
        NativeCaptureViewport::new(width, height, left, top, page_index),
        regions,
    )
}

/// Builds an isolated native-layout color capture for selected rich-text regions.
pub fn capture_frame_color_regions_at(
    frame: &LineDisplayFrame,
    width: u32,
    height: u32,
    left: f32,
    top: f32,
    regions: &[NativeFrameDebugRegion],
) -> Result<NativeFrameCapture, NativeWindowError> {
    capture_frame_color_regions_at_page(frame, width, height, left, top, 0, regions)
}

/// Builds an isolated native-layout color capture for selected rich-text regions on a page.
pub fn capture_frame_color_regions_at_page(
    frame: &LineDisplayFrame,
    width: u32,
    height: u32,
    left: f32,
    top: f32,
    page_index: usize,
    regions: &[NativeFrameDebugRegion],
) -> Result<NativeFrameCapture, NativeWindowError> {
    NativeOffscreenCaptureSession::new()?.capture_frame_color_regions_in(
        frame,
        NativeCaptureViewport::new(width, height, left, top, page_index),
        regions,
    )
}

/// Builds a deterministic native visual plan for a rich-text frame.
pub fn visual_plan_from_frame(frame: &LineDisplayFrame, time_seconds: f32) -> NativeVisualPlan {
    let pages = display_map_page_ranges(frame)
        .into_iter()
        .enumerate()
        .filter_map(|(page_index, page_range)| {
            visual_page_from_range(frame, page_index, page_range, time_seconds)
        })
        .collect();
    NativeVisualPlan { pages }
}

/// Test-facing helper for deterministic visual-plan snapshots.
pub fn visual_plan_from_frame_for_test(
    frame: &LineDisplayFrame,
    time_seconds: f32,
) -> NativeVisualPlan {
    visual_plan_from_frame(frame, time_seconds)
}

fn visual_page_from_range(
    frame: &LineDisplayFrame,
    page_index: usize,
    page_range: Range<usize>,
    time_seconds: f32,
) -> Option<NativeVisualPage> {
    let page_layout = layout_page_range(
        frame,
        page_range.clone(),
        TextLayoutConfig {
            origin: LayoutPoint::new(0.0, 0.0),
            size: LayoutSize::new(720.0, 360.0),
            ..TextLayoutConfig::default()
        },
    )
    .ok()?;
    if page_layout.frame.text.is_empty() {
        return None;
    }
    let runs = native_visual_runs_from_layout(&page_layout, page_range.start);
    let glyphs =
        native_glyph_placements_from_layout(&page_layout, &runs, page_range.start, time_seconds);
    let shaders = runs
        .iter()
        .flat_map(|run| run.presentation.shaders.iter().map(resolve_shader_filter))
        .collect();
    Some(NativeVisualPage {
        page_index,
        text: page_layout.frame.text,
        runs,
        glyphs,
        shaders,
    })
}

struct NativePageLayout {
    frame: LineDisplayFrame,
    page_start: usize,
    layout: LaidOutText,
    text_run_indices: Vec<usize>,
    ruby_indices: Vec<usize>,
}

fn layout_page_range(
    frame: &LineDisplayFrame,
    page_range: Range<usize>,
    config: TextLayoutConfig,
) -> Result<NativePageLayout, NativeWindowError> {
    let page_start = page_range.start;
    let (page_frame, text_run_indices, ruby_indices) = page_local_layout_frame(frame, page_range)?;
    let layout = layout_frame(&page_frame, config)
        .map_err(|error| NativeWindowError::TextLayout(error.to_string()))?;
    Ok(NativePageLayout {
        frame: page_frame,
        page_start,
        layout,
        text_run_indices,
        ruby_indices,
    })
}

fn page_local_layout_frame(
    frame: &LineDisplayFrame,
    page_range: Range<usize>,
) -> Result<(LineDisplayFrame, Vec<usize>, Vec<usize>), NativeWindowError> {
    let text = frame
        .text
        .get(page_range.clone())
        .ok_or(NativeWindowError::EmptyPages)?
        .to_owned();
    if text.is_empty() {
        return Err(NativeWindowError::EmptyPages);
    }

    let mut text_run_indices = Vec::new();
    let text_runs = frame
        .display_map
        .text_runs
        .iter()
        .enumerate()
        .filter_map(|(index, run)| {
            let range = intersect_display_range(run.range, &page_range)?;
            text_run_indices.push(index);
            let mut run = run.clone();
            run.range =
                RichTextRange::new(range.start - page_range.start, range.end - page_range.start);
            Some(run)
        })
        .collect();

    let mut ruby_indices = Vec::new();
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
            ruby_indices.push(index);
            let mut annotation = annotation.clone();
            annotation.base_range = RichTextRange::new(
                base_range.start - page_range.start,
                base_range.end - page_range.start,
            );
            Some(annotation)
        })
        .collect();

    Ok((
        LineDisplayFrame {
            line: frame.line.clone(),
            callee: frame.callee.clone(),
            text,
            base_styles: frame.base_styles.clone(),
            default_inline_failure_policy: frame.default_inline_failure_policy.clone(),
            style_contributions: frame.style_contributions.clone(),
            nodes: Vec::new(),
            display_map: RichTextDisplayMap {
                text_runs,
                ruby_annotations,
                controls: Vec::new(),
                host_events: Vec::new(),
            },
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        },
        text_run_indices,
        ruby_indices,
    ))
}

fn native_visual_runs_from_layout(
    page_layout: &NativePageLayout,
    page_start: usize,
) -> Vec<NativeVisualRun> {
    page_layout
        .layout
        .runs
        .iter()
        .filter_map(|run| {
            let source_run_index = *page_layout.text_run_indices.get(run.run_index)?;
            Some(NativeVisualRun {
                source_run_index,
                range: (page_start + run.range.start)..(page_start + run.range.end),
                local_range: run.range.start..run.range.end,
                presentation: run.presentation.clone(),
            })
        })
        .collect()
}

fn native_glyph_placements_from_layout(
    page_layout: &NativePageLayout,
    runs: &[NativeVisualRun],
    page_start: usize,
    time_seconds: f32,
) -> Vec<NativeGlyphPlacement> {
    let mut run_counts = BTreeMap::<usize, usize>::new();
    for glyph in &page_layout.layout.glyphs {
        *run_counts.entry(glyph.run_index).or_default() += 1;
    }

    let mut next_glyph_indices = BTreeMap::<usize, usize>::new();
    page_layout
        .layout
        .glyphs
        .iter()
        .filter_map(|glyph| {
            let source_run_index = *page_layout.text_run_indices.get(glyph.run_index)?;
            let glyph_index = next_glyph_indices.entry(glyph.run_index).or_default();
            let range = (page_start + glyph.range.start)..(page_start + glyph.range.end);
            let mut placement = NativeGlyphPlacement {
                run_index: source_run_index,
                glyph_index: *glyph_index,
                range,
                x: glyph.origin.x,
                y: glyph.origin.y,
                rotate_degrees: glyph_orientation_degrees(glyph.orientation),
                vertical_form: glyph.vertical_form,
                scale_x: 1.0,
                scale_y: 1.0,
                opacity: 1.0,
            };
            *glyph_index += 1;
            let run = runs
                .iter()
                .find(|run| run.source_run_index == source_run_index)?;
            apply_presentation_to_placement(
                &page_layout.frame.line.0,
                run,
                *run_counts.get(&glyph.run_index).unwrap_or(&1),
                time_seconds,
                &mut placement,
            );
            Some(placement)
        })
        .collect()
}

fn native_element_bounds_from_layout(
    page_layout: &NativePageLayout,
    width: u32,
    height: u32,
) -> Vec<NativeFrameElementBounds> {
    let mut bounds = page_layout
        .layout
        .runs
        .iter()
        .filter_map(|run| {
            let index = *page_layout.text_run_indices.get(run.run_index)?;
            Some(NativeFrameElementBounds {
                element: NativeFrameElement::TextRun { index },
                bbox: native_bbox_from_layout_rect(run.bounds, width, height)?,
                glyph: None,
                ruby: None,
            })
        })
        .collect::<Vec<_>>();
    bounds.extend(
        page_layout
            .layout
            .glyphs
            .iter()
            .enumerate()
            .filter_map(|(index, glyph)| {
                let range_start = page_layout.page_start + glyph.range.start;
                let range_end = page_layout.page_start + glyph.range.end;
                Some(NativeFrameElementBounds {
                    element: NativeFrameElement::GlyphCluster {
                        index,
                        range_start,
                        range_end,
                    },
                    bbox: native_bbox_from_layout_rect(glyph.bounds, width, height)?,
                    glyph: Some(NativeGlyphClusterMetadata {
                        orientation: glyph.orientation.into(),
                        vertical_form: glyph.vertical_form.into(),
                    }),
                    ruby: None,
                })
            }),
    );
    let ruby_bounds_by_index = page_layout
        .layout
        .ruby
        .iter()
        .filter_map(|ruby| {
            let index = *page_layout.ruby_indices.get(ruby.ruby_index)?;
            Some((
                index,
                NativeRubyLayoutGeometry {
                    object: ruby.base_bounds.union(ruby.ruby_bounds),
                    base: ruby.base_bounds,
                    annotation: ruby.ruby_bounds,
                },
            ))
        })
        .fold(
            BTreeMap::<usize, NativeRubyLayoutGeometry>::new(),
            |mut bounds, (index, geometry)| {
                bounds
                    .entry(index)
                    .and_modify(|existing| *existing = existing.union(geometry))
                    .or_insert(geometry);
                bounds
            },
        );
    bounds.extend(
        ruby_bounds_by_index
            .into_iter()
            .filter_map(|(index, geometry)| {
                let bounds =
                    inflate_layout_rect_asymmetric(geometry.object, 16.0, 16.0, 16.0, 16.0);
                Some(NativeFrameElementBounds {
                    element: NativeFrameElement::Ruby { index },
                    bbox: native_bbox_from_layout_rect(bounds, width, height)?,
                    glyph: None,
                    ruby: Some(NativeRubyElementGeometry {
                        base_bbox: native_bbox_from_layout_rect(geometry.base, width, height)?,
                        annotation_bbox: native_bbox_from_layout_rect(
                            geometry.annotation,
                            width,
                            height,
                        )?,
                    }),
                })
            }),
    );
    bounds
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct NativeRubyLayoutGeometry {
    object: LayoutRect,
    base: LayoutRect,
    annotation: LayoutRect,
}

impl NativeRubyLayoutGeometry {
    fn union(self, other: Self) -> Self {
        Self {
            object: self.object.union(other.object),
            base: self.base.union(other.base),
            annotation: self.annotation.union(other.annotation),
        }
    }
}

fn native_text_layout_config(width: u32, height: u32, left: f32, top: f32) -> TextLayoutConfig {
    TextLayoutConfig {
        origin: LayoutPoint::new(left, top),
        size: LayoutSize::new(
            (surface_extent_f32(width) - left).max(1.0),
            (surface_extent_f32(height) - top).max(1.0),
        ),
        ..TextLayoutConfig::default()
    }
}

fn native_bbox_from_layout_rect(
    rect: LayoutRect,
    width: u32,
    height: u32,
) -> Option<NativeFrameContentBBox> {
    native_float_bbox(rect.x, rect.y, rect.width, rect.height, width, height)
}

fn inflate_layout_rect_asymmetric(
    rect: LayoutRect,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
) -> LayoutRect {
    LayoutRect::new(
        rect.x - left,
        rect.y - top,
        rect.width + left + right,
        rect.height + top + bottom,
    )
}

const fn glyph_orientation_degrees(orientation: GlyphOrientation) -> f32 {
    match orientation {
        GlyphOrientation::Upright | GlyphOrientation::TextCombineUpright => 0.0,
        GlyphOrientation::SidewaysCw => 90.0,
    }
}

fn apply_presentation_to_placement(
    line_id: &str,
    run: &NativeVisualRun,
    glyph_count: usize,
    time_seconds: f32,
    placement: &mut NativeGlyphPlacement,
) {
    if let Some(transform) = &run.presentation.transform {
        placement.x += transform.translate.x.as_f32();
        placement.y += transform.translate.y.as_f32();
        placement.rotate_degrees += transform.rotate.as_degrees_f32();
        placement.scale_x *= transform.scale.x.as_f32();
        placement.scale_y *= transform.scale.y.as_f32();
    }
    if let Some(opacity) = run.presentation.opacity {
        placement.opacity *= opacity.as_f32();
    }
    for effect in &run.presentation.effects {
        apply_builtin_descriptor(line_id, effect, glyph_count, time_seconds, placement);
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn apply_builtin_descriptor(
    line_id: &str,
    effect: &RichTextEffectDescriptor,
    glyph_count: usize,
    time_seconds: f32,
    placement: &mut NativeGlyphPlacement,
) {
    if !matches!(
        effect.target,
        RichTextEffectTarget::Run | RichTextEffectTarget::Glyph
    ) {
        return;
    }
    match effect.id.as_str() {
        "wave" => {
            let amplitude = param_milli(effect, "amp").unwrap_or(Milli(4000)).as_f32();
            let period = param_milli(effect, "period")
                .unwrap_or(Milli(12000))
                .as_f32()
                .max(0.001);
            let speed = param_milli(effect, "speed").unwrap_or(Milli::ONE).as_f32();
            let phase = param_milli(effect, "phase").unwrap_or_default().as_f32();
            let direction = param_vec2(effect, "dir")
                .or_else(|| axis_direction(effect))
                .unwrap_or([0.0, 1.0]);
            let t = (usize_to_f32_saturating(placement.glyph_index) / period
                + time_seconds * speed
                + phase)
                * std::f32::consts::TAU;
            let delta = amplitude * t.sin();
            placement.x += direction[0] * delta;
            placement.y += direction[1] * delta;
        }
        "shake" | "jitter" => {
            let amplitude = param_milli(effect, "amp").unwrap_or(Milli(2000)).as_f32();
            let speed = param_milli(effect, "speed")
                .unwrap_or(Milli(16000))
                .as_f32();
            let noise_seed = param_seed(effect, "seed").unwrap_or(0);
            let time_bucket = if effect.id == "jitter" {
                0.0
            } else {
                time_seconds * speed
            };
            let noise =
                deterministic_noise(noise_seed, line_id, placement.glyph_index, time_bucket);
            placement.x += (noise[0] * 2.0 - 1.0) * amplitude;
            placement.y += (noise[1] * 2.0 - 1.0) * amplitude;
        }
        "arc" => {
            let radius = param_milli(effect, "radius")
                .unwrap_or(Milli(120_000))
                .as_f32();
            let start = param_milli(effect, "start").unwrap_or_default().as_f32();
            let step = param_milli(effect, "step").unwrap_or(Milli(8000)).as_f32();
            let angle =
                (start + step * usize_to_f32_saturating(placement.glyph_index)).to_radians();
            placement.x += radius * angle.cos();
            placement.y += radius * angle.sin();
            placement.rotate_degrees += angle.to_degrees() + 90.0;
        }
        "typewriter" => {
            let cps = param_milli(effect, "cps").unwrap_or(Milli(28000)).as_f32();
            let visible = (time_seconds * cps).floor() as usize;
            if placement.glyph_index >= visible.min(glyph_count) {
                placement.opacity = 0.0;
            }
        }
        _ => {}
    }
}

fn resolve_shader_filter(shader: &RichTextShaderRef) -> NativeResolvedShaderFilter {
    NativeResolvedShaderFilter {
        id: shader.id.clone(),
        phase: shader.phase,
        amount: shader_param_milli(shader, "amount")
            .unwrap_or(Milli::ONE)
            .as_f32(),
        direction: shader_param_vec2(shader, "dir").unwrap_or([0.0, 1.0]),
    }
}

fn param_milli(effect: &RichTextEffectDescriptor, name: &str) -> Option<Milli> {
    param_as_milli(effect.params.get(name)?)
}

fn param_seed(effect: &RichTextEffectDescriptor, name: &str) -> Option<u64> {
    effect.params.get(name).map(param_as_seed)
}

fn param_vec2(effect: &RichTextEffectDescriptor, name: &str) -> Option<[f32; 2]> {
    param_as_vec2(effect.params.get(name)?)
}

fn shader_param_milli(shader: &RichTextShaderRef, name: &str) -> Option<Milli> {
    param_as_milli(shader.params.get(name)?)
}

fn shader_param_vec2(shader: &RichTextShaderRef, name: &str) -> Option<[f32; 2]> {
    param_as_vec2(shader.params.get(name)?)
}

fn param_as_milli(param: &RichTextParam) -> Option<Milli> {
    match param {
        RichTextParam::Milli { value } => Some(*value),
        RichTextParam::Int { value } => {
            Some(Milli(i32::try_from(*value).ok()?.saturating_mul(1000)))
        }
        RichTextParam::Raw { value } | RichTextParam::Text { value } => parse_raw_milli(value),
        _ => None,
    }
}

fn param_as_seed(param: &RichTextParam) -> u64 {
    match param {
        RichTextParam::Bool { value } => u64::from(*value),
        RichTextParam::Int { value } => u64::from_ne_bytes(value.to_ne_bytes()),
        RichTextParam::Milli { value } => u64::from_ne_bytes(i64::from(value.0).to_ne_bytes()),
        RichTextParam::Vec2 { value } => {
            u64::from_ne_bytes(i64::from(value.x.0).to_ne_bytes())
                ^ u64::from_ne_bytes(i64::from(value.y.0).to_ne_bytes()).rotate_left(17)
        }
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => stable_text_hash(value),
        RichTextParam::Expr { source } => stable_text_hash(source),
    }
}

fn param_as_vec2(param: &RichTextParam) -> Option<[f32; 2]> {
    match param {
        RichTextParam::Vec2 { value } => Some([value.x.as_f32(), value.y.as_f32()]),
        RichTextParam::Raw { value } | RichTextParam::Text { value } => parse_raw_vec2(value),
        _ => None,
    }
}

fn parse_raw_milli(value: &str) -> Option<Milli> {
    let trimmed = value.trim();
    let numeric = trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("deg"))
        .or_else(|| trimmed.strip_suffix("ch"))
        .unwrap_or(trimmed)
        .trim();
    parse_decimal_milli(numeric)
}

fn parse_raw_vec2(value: &str) -> Option<[f32; 2]> {
    let (x, y) = value.split_once(',')?;
    Some([parse_raw_milli(x)?.as_f32(), parse_raw_milli(y)?.as_f32()])
}

fn axis_direction(effect: &RichTextEffectDescriptor) -> Option<[f32; 2]> {
    match effect.params.get("axis")? {
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => match value.as_str() {
            "x" | ".x" => Some([1.0, 0.0]),
            "y" | ".y" => Some([0.0, 1.0]),
            _ => None,
        },
        _ => None,
    }
}

fn stable_text_hash(value: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn deterministic_noise(seed: u64, line_id: &str, glyph_index: usize, time_bucket: f32) -> [f32; 2] {
    let mut hash =
        seed ^ glyph_index as u64 ^ (time_bucket.floor() as u64).wrapping_mul(0x9E37_79B9);
    for byte in line_id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x1000_0000_01B3);
    }
    let x = ((hash & 0xffff) as f32) / 65535.0;
    hash = hash.rotate_left(17).wrapping_mul(0xD6E8_FD50_9A2C_8395);
    let y = ((hash & 0xffff) as f32) / 65535.0;
    [x, y]
}

fn run_pages_window(title: &str, pages: Vec<WindowPage>) -> Result<(), NativeWindowError> {
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
struct WindowPage {
    rich_text: WindowRichText,
    layout_frame: Option<LineDisplayFrame>,
}

impl WindowPage {
    fn plain(text: &str) -> Self {
        Self {
            rich_text: WindowRichText::plain(text),
            layout_frame: None,
        }
    }

    fn from_frame(frame: &LineDisplayFrame) -> Vec<Self> {
        WindowPageBuilder::from_frame(frame)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WindowRichText {
    text: String,
    spans: Vec<WindowTextSpan>,
    ruby_annotations: Vec<WindowRubyAnnotation>,
}

impl WindowRichText {
    fn plain(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            spans: vec![WindowTextSpan {
                range: 0..text.len(),
                style: NativeTextStyle::default(),
            }],
            ruby_annotations: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

#[derive(Default)]
struct WindowPageBuilder {
    pages: Vec<WindowPage>,
    current: WindowRichTextBuilder,
}

impl WindowPageBuilder {
    fn from_frame(frame: &LineDisplayFrame) -> Vec<WindowPage> {
        if has_display_map(&frame.display_map) {
            return pages_from_display_map(frame);
        }
        let mut builder = Self {
            current: WindowRichTextBuilder::with_base_styles(frame.base_styles.clone()),
            ..Self::default()
        };
        for node in &frame.nodes {
            builder.push_node(node);
        }
        builder.finish()
    }

    fn push_node(&mut self, node: &RichTextNode) {
        if let RichTextNode::Control(RichTextControl::Page | RichTextControl::LineWait) = node {
            self.flush_page();
            return;
        }
        self.current.push_node(node);
    }

    fn flush_page(&mut self) {
        let base_styles = self.current.base_styles.clone();
        let current = std::mem::replace(
            &mut self.current,
            WindowRichTextBuilder::with_base_styles(base_styles),
        )
        .finish();
        if !current.is_empty() {
            self.pages.push(WindowPage {
                rich_text: current,
                layout_frame: None,
            });
        }
    }

    fn finish(mut self) -> Vec<WindowPage> {
        self.flush_page();
        self.pages
    }
}

fn has_display_map(display_map: &RichTextDisplayMap) -> bool {
    !display_map.text_runs.is_empty()
        || !display_map.ruby_annotations.is_empty()
        || !display_map.controls.is_empty()
}

fn pages_from_display_map(frame: &LineDisplayFrame) -> Vec<WindowPage> {
    display_map_page_ranges(frame)
        .into_iter()
        .filter_map(|range| page_from_display_map_range(frame, range))
        .collect()
}

fn display_map_page_ranges(frame: &LineDisplayFrame) -> Vec<Range<usize>> {
    let mut break_offsets = frame
        .display_map
        .controls
        .iter()
        .filter(|marker| {
            matches!(
                marker.control,
                RichTextControl::Page | RichTextControl::LineWait | RichTextControl::Clear
            )
        })
        .map(|marker| display_map_offset_before_node(frame, marker.node_index))
        .map(|offset| display_map_offset_after_atomic_ruby_base(frame, offset))
        .filter(|offset| *offset <= frame.text.len() && frame.text.is_char_boundary(*offset))
        .collect::<Vec<_>>();
    break_offsets.sort_unstable();
    break_offsets.dedup();

    let mut start = 0;
    let mut ranges = Vec::with_capacity(break_offsets.len() + 1);
    for end in break_offsets {
        if start <= end {
            ranges.push(start..end);
            start = end;
        }
    }
    ranges.push(start..frame.text.len());
    ranges
}

fn display_map_offset_after_atomic_ruby_base(frame: &LineDisplayFrame, offset: usize) -> usize {
    let mut adjusted = offset;
    loop {
        let Some(range) = frame
            .display_map
            .ruby_annotations
            .iter()
            .filter_map(|annotation| valid_display_range(annotation.base_range, &frame.text))
            .find(|range| range.start < adjusted && adjusted < range.end)
        else {
            return adjusted;
        };
        adjusted = range.end;
    }
}

fn display_map_non_empty_page_range_at(
    frame: &LineDisplayFrame,
    page_index: usize,
) -> Result<Range<usize>, NativeWindowError> {
    display_map_page_ranges(frame)
        .into_iter()
        .filter(|range| !range.is_empty())
        .nth(page_index)
        .ok_or(NativeWindowError::EmptyPages)
}

fn display_map_offset_before_node(frame: &LineDisplayFrame, node_index: usize) -> usize {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| run.node_index < node_index)
        .map(|run| run.range.end)
        .max()
        .unwrap_or(0)
}

fn page_from_display_map_range(
    frame: &LineDisplayFrame,
    page_range: Range<usize>,
) -> Option<WindowPage> {
    let text = frame.text.get(page_range.clone())?.to_owned();
    if text.is_empty() {
        return None;
    }

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

fn display_map_spans_for_range(
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

fn display_map_ruby_for_range(
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

fn debug_rich_text_for_regions(
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

fn color_rich_text_for_regions(
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

fn debug_selected_text_ranges(
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

fn color_selected_text_ranges(
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

fn debug_selected_ruby_indices(regions: &[NativeFrameDebugRegion]) -> Vec<(usize, [u8; 4])> {
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

fn color_selected_ruby_indices(regions: &[NativeFrameDebugRegion]) -> Vec<usize> {
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

fn debug_text_spans(
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

fn color_text_spans(rich_text: &WindowRichText, selected: &[Range<usize>]) -> Vec<WindowTextSpan> {
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

fn native_color_from_rgba(color: [u8; 4]) -> NativeTextColor {
    NativeTextColor::rgba(color[0], color[1], color[2], color[3])
}

fn intersect_display_range(
    range: RichTextRange,
    page_range: &Range<usize>,
) -> Option<Range<usize>> {
    let start = range.start.max(page_range.start);
    let end = range.end.min(page_range.end);
    (start < end).then_some(start..end)
}

fn valid_display_range(range: RichTextRange, text: &str) -> Option<Range<usize>> {
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
struct WindowTextSpan {
    range: Range<usize>,
    style: NativeTextStyle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WindowRubyAnnotation {
    base_range: Range<usize>,
    ruby: String,
    style: NativeTextStyle,
    presentation: RichTextPresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeTextColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl NativeTextColor {
    const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 255,
        }
    }

    const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    fn from_render_color(color: &RichTextColor) -> Self {
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

    const fn into_glyphon(self) -> Color {
        Color::rgba(self.red, self.green, self.blue, self.alpha)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeTextStyle {
    color: NativeTextColor,
    family: NativeFontFamily,
    weight: NativeTextWeight,
    italic: bool,
    size: Option<u16>,
}

impl NativeTextStyle {
    fn attrs(&self) -> Attrs<'_> {
        self.attrs_with_metrics(Self::metrics_for_size)
    }

    fn ruby_attrs(&self) -> Attrs<'_> {
        self.attrs_with_metrics(Self::ruby_metrics_for_size)
    }

    fn attrs_with_metrics(&self, metrics_for_size: fn(u16) -> Metrics) -> Attrs<'_> {
        let mut attrs = Attrs::new()
            .family(self.family.as_glyphon_family())
            .color(self.color.into_glyphon());
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

    fn metrics(&self) -> Metrics {
        self.size.map_or(Metrics::new(30.0, 42.0), |size| {
            Self::metrics_for_size(size)
        })
    }

    fn ruby_metrics(&self) -> Metrics {
        self.size.map_or(Metrics::new(14.0, 14.0), |size| {
            Self::ruby_metrics_for_size(size)
        })
    }

    fn metrics_for_size(size: u16) -> Metrics {
        let font_size = f32::from(size);
        Metrics::new(font_size, font_size * 1.35)
    }

    fn ruby_metrics_for_size(size: u16) -> Metrics {
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
enum NativeFontFamily {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    Named(String),
}

impl NativeFontFamily {
    fn from_render_family(family: &RichTextFontFamily) -> Self {
        match family {
            RichTextFontFamily::Serif => Self::Serif,
            RichTextFontFamily::SansSerif => Self::SansSerif,
            RichTextFontFamily::Monospace => Self::Monospace,
            RichTextFontFamily::Cursive => Self::Cursive,
            RichTextFontFamily::Fantasy => Self::Fantasy,
            RichTextFontFamily::Named { name } => Self::Named(name.clone()),
        }
    }

    fn as_glyphon_family(&self) -> Family<'_> {
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
enum NativeTextWeight {
    Regular,
    Bold,
}

#[derive(Default)]
struct WindowRichTextBuilder {
    text: String,
    spans: Vec<WindowTextSpan>,
    ruby_annotations: Vec<WindowRubyAnnotation>,
    base_styles: Vec<RichTextStyle>,
    active_styles: Vec<RichTextStyle>,
}

impl WindowRichTextBuilder {
    fn with_base_styles(base_styles: Vec<RichTextStyle>) -> Self {
        Self {
            base_styles,
            ..Self::default()
        }
    }

    fn push_node(&mut self, node: &RichTextNode) {
        match node {
            RichTextNode::Text { text } => {
                self.push_text(text, self.current_style());
            }
            RichTextNode::Ruby { base, ruby } => {
                let base_style = self.current_style();
                let base_range = self.push_text(base, base_style.clone());
                let presentation = presentation_from_styles(
                    self.base_styles.iter().chain(self.active_styles.iter()),
                );
                let ruby_style = native_ruby_style_from_base(base_style, &presentation);
                self.ruby_annotations.push(WindowRubyAnnotation {
                    base_range,
                    ruby: ruby.clone(),
                    style: ruby_style,
                    presentation,
                });
            }
            RichTextNode::StyleStart { style } => self.active_styles.push(style.clone()),
            RichTextNode::StyleEnd { name } => {
                if let Some(index) = self
                    .active_styles
                    .iter()
                    .rposition(|style| style.tag_name() == name)
                {
                    self.active_styles.remove(index);
                }
            }
            RichTextNode::Control(control) => self.push_control(control),
            RichTextNode::Interpolation { expr, .. } => {
                self.push_text(expr, self.current_style());
            }
            RichTextNode::HostEvent(_) => {}
        }
    }

    fn push_control(&mut self, control: &RichTextControl) {
        match control {
            RichTextControl::HardBreak | RichTextControl::Page | RichTextControl::LineWait => {
                self.push_text("\n", self.current_style());
            }
            RichTextControl::Raw { text } => {
                self.push_text(text, self.current_style());
            }
            RichTextControl::TimedWait { .. }
            | RichTextControl::Clear
            | RichTextControl::Mark { .. }
            | RichTextControl::Unknown { .. } => {}
            RichTextControl::Reset => self.active_styles.clear(),
        }
    }

    fn push_text(&mut self, text: &str, style: NativeTextStyle) -> Range<usize> {
        if text.is_empty() {
            return self.text.len()..self.text.len();
        }
        let start = self.text.len();
        self.text.push_str(text);
        let range = start..self.text.len();
        self.spans.push(WindowTextSpan {
            range: range.clone(),
            style,
        });
        range
    }

    fn current_style(&self) -> NativeTextStyle {
        native_style_from_styles(self.base_styles.iter().chain(self.active_styles.iter()))
    }

    fn finish(self) -> WindowRichText {
        WindowRichText {
            text: self.text,
            spans: self.spans,
            ruby_annotations: self.ruby_annotations,
        }
    }
}

fn native_style_from_styles<'a>(
    styles: impl IntoIterator<Item = &'a RichTextStyle>,
) -> NativeTextStyle {
    styles
        .into_iter()
        .fold(NativeTextStyle::default(), apply_style)
}

fn native_ruby_style_from_base(
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

fn native_ruby_style_from_styles(
    styles: &[RichTextStyle],
    presentation: &RichTextPresentation,
) -> NativeTextStyle {
    native_ruby_style_from_base(native_style_from_styles(styles), presentation)
}

fn native_ruby_font_size(presentation: &RichTextPresentation) -> Option<u16> {
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

fn apply_style(mut native: NativeTextStyle, style: &RichTextStyle) -> NativeTextStyle {
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
        | RichTextStyle::Effect { .. }
        | RichTextStyle::Shader { .. }
        | RichTextStyle::Unknown { .. } => {}
    }
    native
}

struct WindowState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: SurfaceConfiguration,
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    text_buffer: Buffer,
    ruby_buffers: Vec<WindowRubyBuffer>,
    rich_text: WindowRichText,
    layout_frame: Option<LineDisplayFrame>,
    layout: Option<LaidOutText>,
    window: Arc<dyn Window>,
}

struct WindowRubyBuffer {
    buffer: Buffer,
    left: f32,
    top: f32,
    placement: RubyGlyphPlacement,
    color: NativeTextColor,
    presentation: RichTextPresentation,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RubyGlyphPlacement {
    Horizontal,
    Vertical {
        cell_width: f32,
        vertical_advance: f32,
    },
}

impl WindowState {
    async fn new(
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
            window,
        };
        state.set_page(page);
        state
    }

    fn set_page(&mut self, page: &WindowPage) {
        self.rich_text = page.rich_text.clone();
        self.layout_frame.clone_from(&page.layout_frame);
        self.prepare_rich_text();
        self.window.request_redraw();
    }

    fn prepare_rich_text(&mut self) {
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

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        self.prepare_rich_text();
        self.window.request_redraw();
    }
}

fn build_ruby_buffers(
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
                            RubyGlyphPlacement::Horizontal
                        } else {
                            RubyGlyphPlacement::Vertical {
                                cell_width: segment.ruby_bounds.width,
                                vertical_advance: segment.ruby_bounds.height
                                    / usize_to_f32_saturating(ruby_char_count),
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
            left,
            top,
            placement: RubyGlyphPlacement::Horizontal,
            color: annotation.style.color,
            presentation: annotation.presentation.clone(),
        });
    }
    buffers
}

fn build_ruby_buffer(
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
        left: spec.left,
        top: spec.top,
        placement: spec.placement,
        color: style.color,
        presentation: spec.presentation.clone(),
    }
}

#[derive(Clone, Copy)]
struct RubyBufferSpec<'a> {
    ruby: &'a str,
    left: f32,
    top: f32,
    placement: RubyGlyphPlacement,
    presentation: &'a RichTextPresentation,
    width: u32,
    height: u32,
}

fn ruby_layout_geometry(layout: Option<&LaidOutText>, ruby_index: usize) -> Option<(f32, f32)> {
    let ruby = layout?
        .ruby
        .iter()
        .find(|ruby| ruby.ruby_index == ruby_index)?;
    Some((ruby.ruby_bounds.x, ruby.ruby_bounds.y))
}

const NATIVE_TEXT_LEFT: f32 = 24.0;
const NATIVE_TEXT_TOP: f32 = 24.0;
const NATIVE_RUBY_BASELINE_OFFSET: f32 = 48.0;
const NATIVE_GLYPHAREA_BASELINE_OFFSET: f32 = 30.0;

#[derive(Clone, Copy, Debug)]
struct NativeTextOrigin {
    left: f32,
    top: f32,
}

impl Default for NativeTextOrigin {
    fn default() -> Self {
        Self {
            left: NATIVE_TEXT_LEFT,
            top: NATIVE_TEXT_TOP,
        }
    }
}

fn ruby_overlay_geometry(
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
fn native_float_bbox(
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

fn text_line_start_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    offsets.extend(
        text.char_indices()
            .filter_map(|(index, ch)| (ch == '\n').then_some(index + ch.len_utf8())),
    );
    offsets
}

struct Application {
    title: String,
    pages: Vec<WindowPage>,
    page_index: usize,
    window_state: Option<WindowState>,
}

impl Application {
    fn current_page(&self) -> &WindowPage {
        &self.pages[self.page_index]
    }

    fn advance_page(&mut self) -> Option<WindowPage> {
        let next_index = self.page_index + 1;
        if next_index >= self.pages.len() {
            return None;
        }
        self.page_index = next_index;
        Some(self.current_page().clone())
    }
}

impl ApplicationHandler for Application {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.window_state.is_some() {
            return;
        }
        let window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_surface_size(LogicalSize::new(960.0, 540.0))
                    .with_title(self.title.clone()),
            )
            .expect("create window");
        let window = Arc::<dyn Window>::from(window);
        self.window_state = Some(pollster::block_on(WindowState::new(
            window,
            event_loop,
            self.current_page(),
        )));
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::KeyboardInput {
            event,
            is_synthetic: false,
            ..
        } = &event
        {
            if key_closes_window(event) {
                event_loop.exit();
                return;
            }
            if key_advances_page(event) {
                let Some(page) = self.advance_page() else {
                    event_loop.exit();
                    return;
                };
                if let Some(state) = self.window_state.as_mut() {
                    state.set_page(&page);
                }
                return;
            }
        }
        let Some(state) = self.window_state.as_mut() else {
            return;
        };
        match event {
            WindowEvent::SurfaceResized(size) => {
                state.resize(size);
            }
            WindowEvent::RedrawRequested => redraw(state),
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}

fn key_advances_page(event: &KeyEvent) -> bool {
    if !event.state.is_pressed() {
        return false;
    }
    match event.key_without_modifiers.as_ref() {
        Key::Named(NamedKey::Enter) => true,
        Key::Character(value) => value == " " || value.eq_ignore_ascii_case("n"),
        _ => false,
    }
}

fn key_closes_window(event: &KeyEvent) -> bool {
    event.state.is_pressed()
        && matches!(
            event.key_without_modifiers.as_ref(),
            Key::Named(NamedKey::Escape)
        )
}

async fn request_capture_device() -> Result<(wgpu::Device, wgpu::Queue), NativeWindowError> {
    let instance = Instance::default();
    let adapter = instance
        .request_adapter(&RequestAdapterOptions::default())
        .await
        .map_err(|error| NativeWindowError::Readback(error.to_string()))?;
    adapter
        .request_device(&DeviceDescriptor::default())
        .await
        .map_err(|error| NativeWindowError::Readback(error.to_string()))
}

struct NativeOffscreenTextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    text_buffer: Buffer,
    ruby_buffers: Vec<WindowRubyBuffer>,
}

#[derive(Clone, Copy, Debug)]
struct NativeRenderTarget {
    width: u32,
    height: u32,
    origin: NativeTextOrigin,
    time_seconds: f32,
    force_alpha_mask: bool,
}

#[derive(Clone, Copy, Debug)]
struct NativeRenderLayout<'a> {
    layout: &'a LaidOutText,
}

impl<'a> NativeRenderLayout<'a> {
    const fn glyph_area(layout: &'a LaidOutText) -> Self {
        Self { layout }
    }
}

impl NativeOffscreenTextRenderer {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: TextureFormat,
    ) -> NativeOffscreenTextRenderer {
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);
        let mut font_system = FontSystem::new();
        let text_buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
        Self {
            font_system,
            swash_cache: SwashCache::new(),
            viewport: Viewport::new(device, &cache),
            atlas,
            text_renderer,
            text_buffer,
            ruby_buffers: Vec::new(),
        }
    }

    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rich_text: &WindowRichText,
        layout: NativeRenderLayout<'_>,
        target: NativeRenderTarget,
    ) -> Result<(), NativeWindowError> {
        prepare_window_text_buffers(
            &mut self.font_system,
            &mut self.text_buffer,
            rich_text,
            target.width,
            target.height,
        );
        self.ruby_buffers = build_ruby_buffers(
            &mut self.font_system,
            &self.text_buffer,
            rich_text,
            Some(layout.layout),
            target.width,
            target.height,
            target.origin,
        );
        self.viewport.update(
            queue,
            Resolution {
                width: target.width,
                height: target.height,
            },
        );
        let cache_keys = layout_glyph_cache_keys(
            &mut self.font_system,
            &self.text_buffer,
            rich_text,
            layout.layout,
        );
        let bounds = native_text_bounds(target.width, target.height);
        let mut glyph_area = glyph_area_from_layout(
            layout.layout,
            GlyphonAreaOptions {
                bounds,
                origin_offset: Vector::new(0.0, NATIVE_GLYPHAREA_BASELINE_OFFSET),
                force_alpha_mask: target.force_alpha_mask,
                ..GlyphonAreaOptions::default()
            },
            |index, glyph| cache_keys_for_layout_glyph(index, glyph.range, &cache_keys),
        )
        .map_err(|error| NativeWindowError::Readback(error.to_string()))?;
        apply_text_colors_to_glyph_area(
            &mut glyph_area,
            rich_text,
            layout.layout,
            target.time_seconds,
        );
        let ruby_glyph_areas = ruby_glyph_areas(
            &self.ruby_buffers,
            target.width,
            target.height,
            target.time_seconds,
            target.force_alpha_mask,
        );
        let mut glyph_areas = Vec::with_capacity(1 + ruby_glyph_areas.len());
        glyph_areas.push(glyph_area.as_glyph_area());
        glyph_areas.extend(ruby_glyph_areas.iter().map(OwnedGlyphArea::as_glyph_area));
        self.text_renderer
            .prepare_text_and_glyph_areas(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                std::iter::empty::<TextArea<'_>>(),
                glyph_areas,
                &mut self.swash_cache,
            )
            .map_err(|error| NativeWindowError::Readback(error.to_string()))
    }

    fn render_texture_with_clear(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        format: TextureFormat,
        clear: wgpu::Color,
    ) -> Result<wgpu::Texture, NativeWindowError> {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("arcweft native capture texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("arcweft native capture encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("arcweft native capture pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .map_err(|error| NativeWindowError::Readback(error.to_string()))?;
        }
        queue.submit(Some(encoder.finish()));
        self.atlas.trim();
        Ok(texture)
    }
}

fn readback_texture_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, NativeWindowError> {
    let padded_row_bytes = padded_rgba_row_bytes(width);
    let buffer_size = u64::from(padded_row_bytes).saturating_mul(u64::from(height));
    let readback = device.create_buffer(&BufferDescriptor {
        label: Some("arcweft native capture readback"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("arcweft native readback encoder"),
    });
    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &readback,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(MapMode::Read, move |result| {
        let _ = sender.send(result.map_err(|error| error.to_string()));
    });
    device
        .poll(PollType::wait_indefinitely())
        .map_err(|error| NativeWindowError::Readback(error.to_string()))?;
    receiver
        .recv()
        .map_err(|error| NativeWindowError::Readback(error.to_string()))?
        .map_err(NativeWindowError::Readback)?;

    let mapped = slice.get_mapped_range();
    let rgba = unpad_rgba_rows(&mapped, width, height, padded_row_bytes);
    drop(mapped);
    readback.unmap();
    Ok(rgba)
}

fn prepare_window_text_buffers(
    font_system: &mut FontSystem,
    text_buffer: &mut Buffer,
    rich_text: &WindowRichText,
    width: u32,
    height: u32,
) {
    text_buffer.set_size(
        font_system,
        Some(surface_extent_f32(width)),
        Some(surface_extent_f32(height)),
    );
    let default_style = NativeTextStyle::default();
    let default_attrs = default_style.attrs();
    let spans = rich_text
        .spans
        .iter()
        .map(|span| (&rich_text.text[span.range.clone()], span.style.attrs()));
    text_buffer.set_rich_text(font_system, spans, &default_attrs, Shaping::Advanced, None);
    text_buffer.shape_until_scroll(font_system, false);
}

fn window_text_areas<'a>(
    text_buffer: &'a Buffer,
    ruby_buffers: &'a [WindowRubyBuffer],
    width: u32,
    height: u32,
    origin: NativeTextOrigin,
) -> Vec<TextArea<'a>> {
    let bounds = TextBounds {
        left: 0,
        top: 0,
        right: surface_extent_i32(width),
        bottom: surface_extent_i32(height),
    };
    let mut areas = Vec::with_capacity(1 + ruby_buffers.len());
    areas.push(TextArea {
        buffer: text_buffer,
        left: origin.left,
        top: origin.top,
        scale: 1.0,
        bounds,
        default_color: Color::rgb(245, 245, 245),
        custom_glyphs: &[],
    });
    areas.extend(ruby_buffers.iter().map(|ruby| TextArea {
        buffer: &ruby.buffer,
        left: ruby.left,
        top: ruby.top,
        scale: 1.0,
        bounds,
        default_color: Color::rgb(170, 190, 220),
        custom_glyphs: &[],
    }));
    areas
}

fn ruby_glyph_areas(
    ruby_buffers: &[WindowRubyBuffer],
    width: u32,
    height: u32,
    time_seconds: f32,
    force_alpha_mask: bool,
) -> Vec<OwnedGlyphArea> {
    let bounds = native_text_bounds(width, height);
    ruby_buffers
        .iter()
        .map(|ruby| {
            let mut area = match ruby.placement {
                RubyGlyphPlacement::Horizontal => glyph_area_from_shaped_buffer(
                    &ruby.buffer,
                    ruby_glyph_area_options(bounds, ruby.left, ruby.top, force_alpha_mask),
                ),
                RubyGlyphPlacement::Vertical {
                    cell_width,
                    vertical_advance,
                } => vertical_glyph_area_from_shaped_buffer(
                    &ruby.buffer,
                    ruby_glyph_area_options(bounds, ruby.left, ruby.top, force_alpha_mask),
                    cell_width,
                    vertical_advance,
                ),
            };
            let mut color = ruby.color;
            color.alpha = scaled_alpha(
                color.alpha,
                presentation_alpha_for_visibility_time(&ruby.presentation, time_seconds),
            );
            let color = color.into_glyphon();
            area.set_default_color(color);
            area.set_color_for_all_glyphs(color);
            area
        })
        .collect()
}

fn ruby_glyph_area_options(
    bounds: TextBounds,
    left: f32,
    top: f32,
    force_alpha_mask: bool,
) -> GlyphonAreaOptions {
    GlyphonAreaOptions {
        bounds,
        default_color: Color::rgb(170, 190, 220),
        left,
        top,
        force_alpha_mask,
        ..GlyphonAreaOptions::default()
    }
}

fn native_text_bounds(width: u32, height: u32) -> TextBounds {
    TextBounds {
        left: 0,
        top: 0,
        right: surface_extent_i32(width),
        bottom: surface_extent_i32(height),
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct LayoutGlyphCacheKeys {
    shaped: Vec<(RichTextRange, ResolvedGlyph)>,
    vertical_alternates: BTreeMap<usize, Vec<ResolvedGlyph>>,
}

fn layout_glyph_cache_keys(
    font_system: &mut FontSystem,
    text_buffer: &Buffer,
    rich_text: &WindowRichText,
    layout: &LaidOutText,
) -> LayoutGlyphCacheKeys {
    let shaped = text_buffer_cache_keys(text_buffer, rich_text);
    let vertical_alternates = layout
        .glyphs
        .iter()
        .enumerate()
        .filter(|(_, glyph)| glyph.vertical_form != GlyphVerticalForm::None)
        .filter_map(|(glyph_index, glyph)| {
            let style = native_style_for_display_range(rich_text, glyph.range);
            let cache_keys = vertical_form_cache_keys(font_system, glyph, &style);
            (!cache_keys.is_empty()).then_some((glyph_index, cache_keys))
        })
        .collect();
    LayoutGlyphCacheKeys {
        shaped,
        vertical_alternates,
    }
}

fn text_buffer_cache_keys(
    text_buffer: &Buffer,
    rich_text: &WindowRichText,
) -> Vec<(RichTextRange, ResolvedGlyph)> {
    let line_starts = text_line_start_offsets(&rich_text.text);
    text_buffer
        .layout_runs()
        .flat_map(|run| {
            let line_start = line_starts.get(run.line_i).copied().unwrap_or(0);
            run.glyphs.iter().filter_map(move |glyph| {
                let start = line_start.saturating_add(glyph.start);
                let end = line_start.saturating_add(glyph.end);
                let physical = glyph.physical((0.0, 0.0), 1.0);
                #[allow(clippy::cast_precision_loss)]
                let offset = Vector::new(physical.x as f32, physical.y as f32);
                (start < end).then_some((
                    RichTextRange::new(start, end),
                    ResolvedGlyph {
                        cache_key: physical.cache_key,
                        advance: Vector::new(glyph.w, 0.0),
                        offset,
                    },
                ))
            })
        })
        .collect()
}

fn native_style_for_display_range(
    rich_text: &WindowRichText,
    range: RichTextRange,
) -> NativeTextStyle {
    rich_text
        .spans
        .iter()
        .find(|span| span.range.start <= range.start && range.end <= span.range.end)
        .map_or_else(NativeTextStyle::default, |span| span.style.clone())
}

fn vertical_form_cache_keys(
    font_system: &mut FontSystem,
    glyph: &arcweft_text_layout::LaidOutGlyph,
    style: &NativeTextStyle,
) -> Vec<ResolvedGlyph> {
    let mut buffer = Buffer::new(font_system, style.metrics());
    let attrs = style
        .attrs()
        .font_features(vertical_form_font_features(glyph.vertical_form));
    let spans = [(glyph.text.as_str(), attrs.clone())];
    buffer.set_rich_text(font_system, spans, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);
    text_buffer_cache_keys_for_text(&buffer)
}

fn text_buffer_cache_keys_for_text(buffer: &Buffer) -> Vec<ResolvedGlyph> {
    buffer
        .layout_runs()
        .flat_map(|run| {
            run.glyphs.iter().map(|glyph| {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                #[allow(clippy::cast_precision_loss)]
                let offset = Vector::new(physical.x as f32, physical.y as f32);
                ResolvedGlyph {
                    cache_key: physical.cache_key,
                    advance: Vector::new(glyph.w, 0.0),
                    offset,
                }
            })
        })
        .collect()
}

fn vertical_form_font_features(vertical_form: GlyphVerticalForm) -> FontFeatures {
    let mut features = FontFeatures::new();
    match vertical_form {
        GlyphVerticalForm::None => {}
        GlyphVerticalForm::UprightAlternate => {
            features.enable(FeatureTag::new(b"vert"));
        }
        GlyphVerticalForm::RotatedAlternate => {
            features.enable(FeatureTag::new(b"vrtr"));
        }
    }
    features
}

fn cache_keys_for_layout_glyph(
    glyph_index: usize,
    range: RichTextRange,
    cache_keys: &LayoutGlyphCacheKeys,
) -> Vec<ResolvedGlyph> {
    if let Some(cache_keys) = cache_keys.vertical_alternates.get(&glyph_index) {
        return normalize_resolved_glyph_offsets(cache_keys.iter().copied());
    }
    normalize_resolved_glyph_offsets(cache_keys.shaped.iter().filter_map(
        |(candidate, resolved)| {
            (candidate.start < range.end && range.start < candidate.end).then_some(*resolved)
        },
    ))
}

fn normalize_resolved_glyph_offsets(
    resolved: impl IntoIterator<Item = ResolvedGlyph>,
) -> Vec<ResolvedGlyph> {
    let mut resolved = resolved.into_iter().collect::<Vec<_>>();
    let Some(anchor_x) = resolved.iter().map(|glyph| glyph.offset.x).reduce(f32::min) else {
        return resolved;
    };
    let anchor_y = resolved
        .iter()
        .map(|glyph| glyph.offset.y)
        .reduce(f32::min)
        .unwrap_or(0.0);
    for glyph in &mut resolved {
        glyph.offset.x -= anchor_x;
        glyph.offset.y -= anchor_y;
    }
    resolved
}

fn padded_rgba_row_bytes(width: u32) -> u32 {
    let row_bytes = width.saturating_mul(4);
    row_bytes.saturating_add(COPY_BYTES_PER_ROW_ALIGNMENT - 1) / COPY_BYTES_PER_ROW_ALIGNMENT
        * COPY_BYTES_PER_ROW_ALIGNMENT
}

fn unpad_rgba_rows(mapped: &[u8], width: u32, height: u32, padded_row_bytes: u32) -> Vec<u8> {
    let row_bytes = usize::try_from(width.saturating_mul(4)).unwrap_or(0);
    let padded_row_bytes = usize::try_from(padded_row_bytes).unwrap_or(0);
    let mut rgba =
        Vec::with_capacity(row_bytes.saturating_mul(usize::try_from(height).unwrap_or(0)));
    for row in 0..usize::try_from(height).unwrap_or(0) {
        let start = row.saturating_mul(padded_row_bytes);
        let end = start.saturating_add(row_bytes);
        if let Some(bytes) = mapped.get(start..end) {
            rgba.extend_from_slice(bytes);
        }
    }
    rgba
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeFrameContentStats {
    content_bbox: Option<NativeFrameContentBBox>,
    content_pixels: u64,
}

fn native_frame_content_stats(
    rgba: &[u8],
    width: u32,
    height: u32,
    background: [u8; 4],
) -> NativeFrameContentStats {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut count = 0_u64;
    for y in 0..height {
        for x in 0..width {
            let index = usize::try_from(y)
                .unwrap_or(0)
                .saturating_mul(usize::try_from(width).unwrap_or(0))
                .saturating_add(usize::try_from(x).unwrap_or(0))
                .saturating_mul(4);
            let Some(pixel) = rgba.get(index..index.saturating_add(4)) else {
                continue;
            };
            if pixel == background {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            count = count.saturating_add(1);
        }
    }
    NativeFrameContentStats {
        content_bbox: (count > 0).then_some(NativeFrameContentBBox {
            x: min_x,
            y: min_y,
            width: max_x.saturating_sub(min_x).saturating_add(1),
            height: max_y.saturating_sub(min_y).saturating_add(1),
        }),
        content_pixels: count,
    }
}

fn solid_rgba(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let pixel_count = usize::try_from(width)
        .unwrap_or(0)
        .saturating_mul(usize::try_from(height).unwrap_or(0));
    let mut rgba = Vec::with_capacity(pixel_count.saturating_mul(4));
    for _ in 0..pixel_count {
        rgba.extend_from_slice(&color);
    }
    rgba
}

fn clear_transparent_rgb(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] == 0 {
            pixel.copy_from_slice(&[0, 0, 0, 0]);
        }
    }
}

fn fill_native_rect(
    rgba: &mut [u8],
    bitmap_width: u32,
    bitmap_height: u32,
    bbox: NativeFrameContentBBox,
    color: [u8; 4],
) {
    if bitmap_width == 0 || bitmap_height == 0 {
        return;
    }
    let x = bbox.x.min(bitmap_width.saturating_sub(1));
    let y = bbox.y.min(bitmap_height.saturating_sub(1));
    let x_end = x.saturating_add(bbox.width).min(bitmap_width);
    let y_end = y.saturating_add(bbox.height).min(bitmap_height);
    let bitmap_width = usize::try_from(bitmap_width).unwrap_or(0);
    for py in y..y_end {
        let py = usize::try_from(py).unwrap_or(0);
        for px in x..x_end {
            let px = usize::try_from(px).unwrap_or(0);
            let index = py
                .saturating_mul(bitmap_width)
                .saturating_add(px)
                .saturating_mul(4);
            if let Some(pixel) = rgba.get_mut(index..index.saturating_add(4)) {
                pixel.copy_from_slice(&color);
            }
        }
    }
}

fn redraw(state: &mut WindowState) {
    if prepare_window_text_renderer(state).is_err() {
        return;
    }
    let frame = match state.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) => frame,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
            state.window.request_redraw();
            return;
        }
        wgpu::CurrentSurfaceTexture::Outdated
        | wgpu::CurrentSurfaceTexture::Lost
        | wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
            state
                .surface
                .configure(&state.device, &state.surface_config);
            state.window.request_redraw();
            return;
        }
        wgpu::CurrentSurfaceTexture::Validation => return,
    };
    let view = frame.texture.create_view(&TextureViewDescriptor::default());
    let mut encoder = state
        .device
        .create_command_encoder(&CommandEncoderDescriptor { label: None });
    {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let _ = state
            .text_renderer
            .render(&state.atlas, &state.viewport, &mut pass);
    }
    state.queue.submit(Some(encoder.finish()));
    frame.present();
    state.atlas.trim();
}

fn apply_text_colors_to_glyph_area(
    glyph_area: &mut OwnedGlyphArea,
    rich_text: &WindowRichText,
    layout: &LaidOutText,
    time_seconds: f32,
) {
    let mut glyph_index_by_run = BTreeMap::<usize, usize>::new();
    let glyph_count_by_run =
        layout
            .glyphs
            .iter()
            .fold(BTreeMap::<usize, usize>::new(), |mut counts, glyph| {
                *counts.entry(glyph.run_index).or_default() += 1;
                counts
            });
    for (glyph_index, glyph) in layout.glyphs.iter().enumerate() {
        let run_glyph_index = glyph_index_by_run.entry(glyph.run_index).or_default();
        let glyph_count = *glyph_count_by_run.get(&glyph.run_index).unwrap_or(&1);
        let mut color = rich_text
            .spans
            .iter()
            .find(|span| span.range.start <= glyph.range.start && glyph.range.end <= span.range.end)
            .map_or_else(|| NativeTextStyle::default().color, |span| span.style.color);
        color.alpha = scaled_alpha(
            color.alpha,
            glyph_alpha_for_time(glyph, *run_glyph_index, glyph_count, time_seconds),
        );
        glyph_area.set_color_for_layout_glyph(glyph_index, color.into_glyphon());
        *run_glyph_index += 1;
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn glyph_alpha_for_time(
    glyph: &LaidOutGlyph,
    glyph_index: usize,
    glyph_count: usize,
    time_seconds: f32,
) -> u8 {
    let mut alpha = glyph
        .presentation
        .opacity
        .map_or(1.0, Milli::as_f32)
        .clamp(0.0, 1.0);
    for effect in &glyph.presentation.effects {
        if effect.id != "typewriter"
            || !matches!(
                effect.target,
                RichTextEffectTarget::Run | RichTextEffectTarget::Glyph
            )
        {
            continue;
        }
        let cps = param_milli(effect, "cps")
            .unwrap_or(Milli(28000))
            .as_f32()
            .max(0.0);
        let visible = (time_seconds.max(0.0) * cps).floor() as usize;
        if glyph_index >= visible.min(glyph_count) {
            alpha = 0.0;
        }
    }
    (alpha * 255.0).round().clamp(0.0, 255.0) as u8
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn presentation_alpha_for_visibility_time(
    presentation: &RichTextPresentation,
    time_seconds: f32,
) -> u8 {
    let mut alpha = presentation
        .opacity
        .map_or(1.0, Milli::as_f32)
        .clamp(0.0, 1.0);
    for effect in &presentation.effects {
        if effect.id != "typewriter"
            || !matches!(
                effect.target,
                RichTextEffectTarget::Run | RichTextEffectTarget::Glyph
            )
        {
            continue;
        }
        let cps = param_milli(effect, "cps")
            .unwrap_or(Milli(28000))
            .as_f32()
            .max(0.0);
        if (time_seconds.max(0.0) * cps).floor() < 1.0 {
            alpha = 0.0;
        }
    }
    (alpha * 255.0).round().clamp(0.0, 255.0) as u8
}

fn scaled_alpha(base: u8, factor: u8) -> u8 {
    let scaled = u16::from(base) * u16::from(factor);
    u8::try_from((scaled + 127) / 255).unwrap_or(u8::MAX)
}

fn prepare_window_text_renderer(state: &mut WindowState) -> Result<(), ()> {
    state.viewport.update(
        &state.queue,
        Resolution {
            width: state.surface_config.width,
            height: state.surface_config.height,
        },
    );
    if let Some(layout) = state.layout.as_ref() {
        let cache_keys = layout_glyph_cache_keys(
            &mut state.font_system,
            &state.text_buffer,
            &state.rich_text,
            layout,
        );
        let bounds = native_text_bounds(state.surface_config.width, state.surface_config.height);
        let Ok(mut glyph_area) = glyph_area_from_layout(
            layout,
            GlyphonAreaOptions {
                bounds,
                origin_offset: Vector::new(0.0, NATIVE_GLYPHAREA_BASELINE_OFFSET),
                ..GlyphonAreaOptions::default()
            },
            |index, glyph| cache_keys_for_layout_glyph(index, glyph.range, &cache_keys),
        ) else {
            return Err(());
        };
        apply_text_colors_to_glyph_area(&mut glyph_area, &state.rich_text, layout, 60.0);
        let ruby_glyph_areas = ruby_glyph_areas(
            &state.ruby_buffers,
            state.surface_config.width,
            state.surface_config.height,
            60.0,
            false,
        );
        let mut glyph_areas = Vec::with_capacity(1 + ruby_glyph_areas.len());
        glyph_areas.push(glyph_area.as_glyph_area());
        glyph_areas.extend(ruby_glyph_areas.iter().map(OwnedGlyphArea::as_glyph_area));
        state
            .text_renderer
            .prepare_text_and_glyph_areas(
                &state.device,
                &state.queue,
                &mut state.font_system,
                &mut state.atlas,
                &state.viewport,
                std::iter::empty::<TextArea<'_>>(),
                glyph_areas,
                &mut state.swash_cache,
            )
            .map_err(|_| ())
    } else {
        let text_areas = window_text_areas(
            &state.text_buffer,
            &state.ruby_buffers,
            state.surface_config.width,
            state.surface_config.height,
            NativeTextOrigin::default(),
        );
        state
            .text_renderer
            .prepare(
                &state.device,
                &state.queue,
                &mut state.font_system,
                &mut state.atlas,
                &state.viewport,
                text_areas,
                &mut state.swash_cache,
            )
            .map_err(|_| ())
    }
}

fn surface_extent_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

fn surface_extent_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn usize_to_f32_saturating(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::plan::RuntimeLineId;
    use arcweft_render_text::{
        LineDisplaySpec, RichTextDocument, RichTextLayout, RichTextWritingMode, RuntimeLineContext,
    };

    fn styled_ruby_test_frame() -> LineDisplayFrame {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.001".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: vec![RichTextStyle::from_tag("color", "#aabedc")],
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::Text {
                    text: "Hello ".to_owned(),
                },
                RichTextNode::StyleStart {
                    style: RichTextStyle::from_tag("color", "#80c0ff"),
                },
                RichTextNode::StyleStart {
                    style: RichTextStyle::from_tag("font", "monospace"),
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "ゆめ".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "font".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "color".to_owned(),
                },
            ]),
        };
        let mut frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        frame.nodes.clear();
        frame
    }

    fn vertical_ruby_text_combine_frame(writing_mode: RichTextWritingMode) -> LineDisplayFrame {
        let spec = LineDisplaySpec {
            line: RuntimeLineId(format!(
                "say.test.vertical.{}.window.ruby.combine",
                match writing_mode {
                    RichTextWritingMode::VerticalRl => "rl",
                    RichTextWritingMode::VerticalLr => "lr",
                    RichTextWritingMode::HorizontalTb => "horizontal",
                }
            )),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::Text {
                    text: "天地".to_owned(),
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "ゆめ".to_owned(),
                },
                RichTextNode::Text {
                    text: "2026Z".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "layout".to_owned(),
                },
            ]),
        };
        spec.resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves")
    }

    #[test]
    fn window_rich_text_uses_display_map_for_style_spans_and_ruby_hint() {
        let frame = styled_ruby_test_frame();
        let pages = WindowPage::from_frame(&frame);
        assert!(
            pages[0].layout_frame.is_some(),
            "display-map pages retain page-local layout source for window GlyphArea rendering"
        );
        let rich_text = &pages[0].rich_text;

        assert_eq!(rich_text.text, "Hello 夢");
        assert_eq!(
            rich_text.ruby_annotations,
            vec![WindowRubyAnnotation {
                base_range: "Hello ".len().."Hello 夢".len(),
                ruby: "ゆめ".to_owned(),
                style: NativeTextStyle {
                    color: NativeTextColor::new(170, 190, 220),
                    family: NativeFontFamily::Monospace,
                    weight: NativeTextWeight::Regular,
                    italic: false,
                    size: Some(14),
                },
                presentation: RichTextPresentation::default(),
            }]
        );
        assert!(rich_text.spans.iter().any(|span| {
            &rich_text.text[span.range.clone()] == "夢"
                && span.style.color == NativeTextColor::new(128, 192, 255)
                && span.style.family == NativeFontFamily::Monospace
        }));

        let mut font_system = FontSystem::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
        buffer.set_size(&mut font_system, Some(800.0), Some(600.0));
        let default_style = NativeTextStyle::default();
        let default_attrs = default_style.attrs();
        let spans = rich_text
            .spans
            .iter()
            .map(|span| (&rich_text.text[span.range.clone()], span.style.attrs()));
        buffer.set_rich_text(
            &mut font_system,
            spans,
            &default_attrs,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut font_system, false);
        let measured = ruby_overlay_geometry(
            &buffer,
            rich_text,
            &rich_text.ruby_annotations[0].base_range,
            NativeTextOrigin::default(),
        )
        .expect("ruby base has shaped glyph geometry");
        assert!(measured.2 > 1.0);

        assert_ruby_glyph_areas_use_absolute_glypharea(
            &mut font_system,
            &buffer,
            rich_text,
            &pages,
        );
    }

    #[test]
    fn ruby_buffers_without_layout_require_shaped_base_geometry() {
        let frame = styled_ruby_test_frame();
        let pages = WindowPage::from_frame(&frame);
        let rich_text = &pages[0].rich_text;
        let mut font_system = FontSystem::new();
        let empty_buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));

        let ruby_buffers = build_ruby_buffers(
            &mut font_system,
            &empty_buffer,
            rich_text,
            None,
            800,
            600,
            NativeTextOrigin::default(),
        );

        assert!(
            ruby_buffers.is_empty(),
            "ruby buffers without layout should not fall back to estimated positions"
        );
    }

    fn assert_ruby_glyph_areas_use_absolute_glypharea(
        font_system: &mut FontSystem,
        text_buffer: &Buffer,
        rich_text: &WindowRichText,
        pages: &[WindowPage],
    ) {
        let layout = layout_frame(
            pages[0].layout_frame.as_ref().expect("layout frame"),
            native_text_layout_config(800, 600, 0.0, 0.0),
        )
        .expect("layout resolves");
        let ruby_buffers = build_ruby_buffers(
            font_system,
            text_buffer,
            rich_text,
            Some(&layout),
            800,
            600,
            NativeTextOrigin::default(),
        );
        let ruby_glyph_areas = ruby_glyph_areas(&ruby_buffers, 800, 600, 60.0, false);
        assert_eq!(ruby_glyph_areas.len(), 1);
        assert!(!ruby_glyph_areas[0].is_empty());
        assert!((ruby_glyph_areas[0].as_glyph_area().left - 0.0).abs() < f32::EPSILON);
        assert!((ruby_glyph_areas[0].as_glyph_area().top - 0.0).abs() < f32::EPSILON);
        assert!(ruby_glyph_areas[0].glyphs()[0].origin.x >= layout.ruby[0].ruby_bounds.x.floor());
    }

    #[test]
    fn window_pages_keep_vertical_layout_source_for_glyph_area_rendering() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.vertical.window".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode: RichTextWritingMode::VerticalRl,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::Text {
                    text: "縦Ａ。ー".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "/".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let page = WindowPage::from_frame(&frame)
            .into_iter()
            .next()
            .expect("page exists");
        let layout_frame = page
            .layout_frame
            .as_ref()
            .expect("page keeps layout source");
        let layout = layout_frame
            .display_map
            .text_runs
            .iter()
            .find_map(|run| run.presentation.layout.as_ref())
            .expect("layout presentation is preserved");

        assert_eq!(page.rich_text.text, "縦Ａ。ー");
        assert_eq!(layout.writing_mode, RichTextWritingMode::VerticalRl);

        let plan = visual_plan_from_frame_for_test(&frame, 0.0);
        let visual_page = plan.pages.first().expect("visual page exists");
        let vertical_form_for = |text: &str| {
            visual_page
                .glyphs
                .iter()
                .find(|glyph| visual_page.text.get(glyph.range.clone()) == Some(text))
                .map(|glyph| glyph.vertical_form)
                .expect("glyph placement exists")
        };

        assert_eq!(vertical_form_for("。"), GlyphVerticalForm::UprightAlternate);
        assert_eq!(vertical_form_for("ー"), GlyphVerticalForm::RotatedAlternate);
    }

    #[test]
    fn window_pages_keep_vertical_ruby_text_combine_source_for_glyph_area_rendering() {
        for writing_mode in [
            RichTextWritingMode::VerticalRl,
            RichTextWritingMode::VerticalLr,
        ] {
            let frame = vertical_ruby_text_combine_frame(writing_mode);
            let page = WindowPage::from_frame(&frame)
                .into_iter()
                .next()
                .expect("page exists");

            assert_eq!(page.rich_text.text, "天地夢2026Z");
            assert_eq!(page.rich_text.ruby_annotations.len(), 1);
            assert_eq!(page.rich_text.ruby_annotations[0].base_range, 6..9);

            let page_layout_frame = page
                .layout_frame
                .as_ref()
                .expect("window page keeps page-local layout source");
            let layout_presentation = page_layout_frame
                .display_map
                .text_runs
                .iter()
                .find_map(|run| run.presentation.layout.as_ref())
                .expect("layout presentation is preserved");
            assert_eq!(layout_presentation.writing_mode, writing_mode);

            let layout = layout_frame(
                page_layout_frame,
                native_text_layout_config(800, 600, 96.0, 572.0),
            )
            .expect("layout resolves");
            let combine_index = layout
                .glyphs
                .iter()
                .position(|glyph| glyph.text == "2026")
                .expect("text-combine glyph exists");
            assert_eq!(
                layout.glyphs[combine_index].orientation,
                GlyphOrientation::TextCombineUpright
            );

            let mut font_system = FontSystem::new();
            let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
            prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);
            let cache_keys =
                layout_glyph_cache_keys(&mut font_system, &buffer, &page.rich_text, &layout);
            let glyph_area = glyph_area_from_layout(
                &layout,
                GlyphonAreaOptions {
                    bounds: native_text_bounds(800, 600),
                    origin_offset: Vector::new(0.0, NATIVE_GLYPHAREA_BASELINE_OFFSET),
                    ..GlyphonAreaOptions::default()
                },
                |index, glyph| cache_keys_for_layout_glyph(index, glyph.range, &cache_keys),
            )
            .expect("window layout source adapts to glyph area");
            assert_eq!(
                glyph_area
                    .glyphs()
                    .iter()
                    .filter(|glyph| glyph.metadata == combine_index)
                    .count(),
                4,
                "{writing_mode:?} text-combine cluster should expand to one glyph instance per digit"
            );

            let ruby_buffers = build_ruby_buffers(
                &mut font_system,
                &buffer,
                &page.rich_text,
                Some(&layout),
                800,
                600,
                NativeTextOrigin::default(),
            );
            assert_eq!(ruby_buffers.len(), 1);
            assert!(matches!(
                ruby_buffers[0].placement,
                RubyGlyphPlacement::Vertical { .. }
            ));
            assert_eq!(layout.ruby[0].writing_mode, writing_mode);
            match writing_mode {
                RichTextWritingMode::VerticalRl => {
                    assert!(
                        layout.ruby[0].ruby_bounds.x >= layout.ruby[0].base_bounds.right(),
                        "vertical_rl ruby should render on the right annotation track"
                    );
                }
                RichTextWritingMode::VerticalLr => {
                    assert!(
                        layout.ruby[0].ruby_bounds.right() <= layout.ruby[0].base_bounds.x,
                        "vertical_lr ruby should render on the left annotation track"
                    );
                }
                RichTextWritingMode::HorizontalTb => unreachable!("test uses vertical modes"),
            }
            let ruby_glyph_areas = ruby_glyph_areas(&ruby_buffers, 800, 600, 60.0, false);
            assert_eq!(ruby_glyph_areas.len(), 1);
            assert!(
                ruby_glyph_areas[0]
                    .glyphs()
                    .iter()
                    .all(|glyph| glyph.origin.x >= layout.ruby[0].ruby_bounds.x.floor())
            );
        }
    }

    #[test]
    fn overheight_vertical_ruby_segments_render_as_multiple_glyph_areas() {
        for (writing_mode, continuation_moves_right) in [
            (RichTextWritingMode::VerticalRl, true),
            (RichTextWritingMode::VerticalLr, false),
        ] {
            assert_overheight_vertical_ruby_glyph_areas(writing_mode, continuation_moves_right);
        }
    }

    fn assert_overheight_vertical_ruby_glyph_areas(
        writing_mode: RichTextWritingMode,
        continuation_moves_right: bool,
    ) {
        let spec = LineDisplaySpec {
            line: RuntimeLineId(format!("say.test.vertical.ruby.split.{writing_mode:?}")),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "あいうえお".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "/".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let page = WindowPage::from_frame(&frame)
            .into_iter()
            .next()
            .expect("page exists");
        let layout = layout_frame(
            page.layout_frame.as_ref().expect("layout frame"),
            TextLayoutConfig {
                size: LayoutSize::new(160.0, 42.0),
                ruby_font_size: 14.0,
                ..native_text_layout_config(800, 600, 0.0, 0.0)
            },
        )
        .expect("layout resolves");
        assert_eq!(layout.ruby.len(), 2);
        let mut font_system = FontSystem::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
        prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);
        let ruby_buffers = build_ruby_buffers(
            &mut font_system,
            &buffer,
            &page.rich_text,
            Some(&layout),
            800,
            600,
            NativeTextOrigin::default(),
        );
        let ruby_glyph_areas = ruby_glyph_areas(&ruby_buffers, 800, 600, 60.0, false);
        assert_ruby_continuation_track(&ruby_buffers, continuation_moves_right);
        assert_vertical_ruby_glyph_placement(&ruby_buffers[0].placement, &layout);
        assert_eq!(ruby_glyph_areas.len(), 2);
        assert_split_ruby_glyph_area_geometry(&ruby_glyph_areas, &layout, continuation_moves_right);
    }

    fn assert_ruby_continuation_track(
        ruby_buffers: &[WindowRubyBuffer],
        continuation_moves_right: bool,
    ) {
        if continuation_moves_right {
            assert!(ruby_buffers[1].left > ruby_buffers[0].left);
        } else {
            assert!(ruby_buffers[1].left < ruby_buffers[0].left);
        }
    }

    fn assert_vertical_ruby_glyph_placement(placement: &RubyGlyphPlacement, layout: &LaidOutText) {
        let RubyGlyphPlacement::Vertical {
            cell_width: w,
            vertical_advance: advance,
        } = *placement
        else {
            panic!("vertical layout ruby should use vertical glyph placement");
        };
        assert!((w - layout.ruby[0].ruby_bounds.width).abs() < f32::EPSILON);
        assert!((advance - layout.ruby[0].ruby_bounds.height / 3.0).abs() < 0.0001);
    }

    fn assert_split_ruby_glyph_area_geometry(
        ruby_glyph_areas: &[OwnedGlyphArea],
        layout: &LaidOutText,
        continuation_moves_right: bool,
    ) {
        assert!(
            ruby_glyph_areas[0].glyphs()[1].origin.y > ruby_glyph_areas[0].glyphs()[0].origin.y,
            "vertical ruby glyphs should advance downward inside each segment"
        );
        assert!(
            (ruby_glyph_areas[0].glyphs()[1].origin.x - ruby_glyph_areas[0].glyphs()[0].origin.x)
                .abs()
                <= layout.ruby[0].ruby_bounds.width,
            "vertical ruby glyphs should remain in the same annotation track"
        );
        if continuation_moves_right {
            assert!(
                ruby_glyph_areas[1].glyphs()[0].origin.x > ruby_glyph_areas[0].glyphs()[0].origin.x
            );
        } else {
            assert!(
                ruby_glyph_areas[1].glyphs()[0].origin.x < ruby_glyph_areas[0].glyphs()[0].origin.x
            );
        }
    }

    #[test]
    fn ruby_glyph_areas_apply_typewriter_visibility_alpha() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.vertical.ruby.typewriter.alpha".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode: RichTextWritingMode::VerticalRl,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::StyleStart {
                    style: RichTextStyle::Effect {
                        effect: RichTextEffectDescriptor {
                            id: "typewriter".to_owned(),
                            params: BTreeMap::from([(
                                "cps".to_owned(),
                                RichTextParam::Milli { value: Milli::ONE },
                            )]),
                            target: RichTextEffectTarget::Run,
                            phase: RichTextEffectPhase::GlyphMask,
                            state_scope: arcweft_render_text::RichTextStateScope::Run,
                        },
                    },
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "ながいよみ".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "effect".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "layout".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let page = WindowPage::from_frame(&frame)
            .into_iter()
            .next()
            .expect("page exists");
        let layout = layout_frame(
            page.layout_frame.as_ref().expect("layout frame"),
            native_text_layout_config(800, 600, 0.0, 0.0),
        )
        .expect("layout resolves");

        let mut font_system = FontSystem::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
        prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);
        let ruby_buffers = build_ruby_buffers(
            &mut font_system,
            &buffer,
            &page.rich_text,
            Some(&layout),
            800,
            600,
            NativeTextOrigin::default(),
        );

        let hidden = ruby_glyph_areas(&ruby_buffers, 800, 600, 0.0, false);
        let visible = ruby_glyph_areas(&ruby_buffers, 800, 600, 4.0, false);

        assert!(!hidden.is_empty());
        assert!(
            hidden
                .iter()
                .flat_map(OwnedGlyphArea::glyphs)
                .all(|glyph| glyph.color == Some(Color::rgba(170, 190, 220, 0)))
        );
        assert!(
            visible
                .iter()
                .flat_map(OwnedGlyphArea::glyphs)
                .all(|glyph| glyph.color == Some(Color::rgba(170, 190, 220, 255)))
        );
    }

    #[test]
    fn native_bounds_union_overheight_ruby_segments_by_object_index() {
        for writing_mode in [
            RichTextWritingMode::VerticalRl,
            RichTextWritingMode::VerticalLr,
        ] {
            let spec = LineDisplaySpec {
                line: RuntimeLineId(format!(
                    "say.test.vertical.ruby.bounds.split.{writing_mode:?}"
                )),
                callee: "alice".to_owned(),
                text_key: None,
                window: None,
                voice: None,
                look: None,
                style: None,
                base_styles: Vec::new(),
                default_inline_failure_policy: None,
                style_contributions: Vec::new(),
                args: Vec::new(),
                content: RichTextDocument::new(vec![
                    RichTextNode::StyleStart {
                        style: RichTextStyle::Layout {
                            layout: RichTextLayout {
                                writing_mode,
                                ..RichTextLayout::default()
                            },
                        },
                    },
                    RichTextNode::Ruby {
                        base: "夢".to_owned(),
                        ruby: "あいうえおかきくけこ".to_owned(),
                    },
                    RichTextNode::StyleEnd {
                        name: "/".to_owned(),
                    },
                ]),
            };
            let frame = spec
                .resolve_frame(&RuntimeLineContext::default())
                .expect("frame resolves");
            let layout = layout_page_range(
                &frame,
                0.."夢".len(),
                TextLayoutConfig {
                    size: LayoutSize::new(160.0, 90.0),
                    ruby_font_size: 14.0,
                    ..native_text_layout_config(160, 90, 0.0, 0.0)
                },
            )
            .expect("page layout resolves");
            assert!(layout.layout.ruby.len() > 1);

            let bounds = native_element_bounds_from_layout(&layout, 220, 120);
            let ruby_bounds = bounds
                .iter()
                .filter(|bounds| matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }))
                .collect::<Vec<_>>();

            assert_eq!(ruby_bounds.len(), 1);
            assert!(
                ruby_bounds[0].bbox.width > 40,
                "{writing_mode:?} ruby object bounds should union split annotation columns"
            );
        }
    }

    #[test]
    fn native_debug_capture_unions_overheight_ruby_segments_by_object_index() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.vertical.ruby.debug.split".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode: RichTextWritingMode::VerticalLr,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::Text {
                    text: "天地".to_owned(),
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "あいうえおかきくけこ".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "/".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let page_layout = layout_page_range(
            &frame,
            0.."天地夢".len(),
            native_text_layout_config(220, 120, 48.0, 0.0),
        )
        .expect("page layout resolves");
        assert!(page_layout.layout.ruby.len() > 1);
        let bounds = native_element_bounds_from_layout(&page_layout, 220, 120);
        let ruby = bounds
            .iter()
            .find(|bounds| matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }))
            .expect("ruby element has native bounds");
        let ruby_geometry = ruby.ruby.expect("ruby geometry is reported");
        assert!(
            ruby_geometry.annotation_bbox.width > 14,
            "over-height ruby bounds should union split annotation tracks"
        );
        let fallback_bbox = NativeFrameContentBBox {
            x: 1,
            y: 1,
            width: 8,
            height: 8,
        };
        let capture = capture_frame_debug_regions_at(
            &frame,
            220,
            120,
            48.0,
            0.0,
            &[NativeFrameDebugRegion {
                element: Some(NativeFrameElement::Ruby { index: 0 }),
                fallback_bbox,
                color: [255, 255, 255, 255],
            }],
        )
        .expect("over-height ruby debug capture resolves");

        let content = capture
            .content_bbox
            .expect("over-height ruby debug capture has visible content");
        assert_ne!(content, fallback_bbox);
        assert!(content.x >= ruby.bbox.x);
        assert!(content.y >= ruby.bbox.y);
        assert!(content.x.saturating_add(content.width) <= ruby.bbox.x + ruby.bbox.width);
        assert!(content.y.saturating_add(content.height) <= ruby.bbox.y + ruby.bbox.height);
        assert!(
            content.width > 14,
            "over-height ruby debug content should include split annotation columns"
        );
        assert!(capture.content_pixels > 0);
    }

    #[test]
    fn native_text_style_metrics_follow_size_style() {
        let style = RichTextStyle::Size {
            points: Some(48),
            raw: "48".to_owned(),
        };
        let native = native_style_from_styles([&style]);
        let metrics = native.metrics();

        assert!((metrics.font_size - 48.0).abs() < f32::EPSILON);
        assert!((metrics.line_height - 64.8).abs() <= 0.0001);
    }

    #[test]
    fn native_ruby_style_uses_tight_line_height() {
        let presentation = RichTextPresentation {
            layout: Some(RichTextLayout {
                ruby_font_size: Some(arcweft_render_text::Milli(11000)),
                ..RichTextLayout::default()
            }),
            ..RichTextPresentation::default()
        };
        let native = native_ruby_style_from_styles(&[], &presentation);
        let metrics = native.ruby_metrics();

        assert!((metrics.font_size - 11.0).abs() < f32::EPSILON);
        assert!((metrics.line_height - 11.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vertical_alternate_glyphs_use_feature_shaped_cache_keys() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.vertical.features".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: vec![RichTextStyle::Size {
                points: Some(48),
                raw: "48".to_owned(),
            }],
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode: RichTextWritingMode::VerticalRl,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::Text {
                    text: "縦Ａ。ー".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "/".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let page = WindowPage::from_frame(&frame)
            .into_iter()
            .next()
            .expect("page exists");
        let layout = layout_frame(
            page.layout_frame.as_ref().expect("layout frame"),
            native_text_layout_config(800, 600, 0.0, 0.0),
        )
        .expect("layout resolves");
        let mut font_system = FontSystem::new();
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
        prepare_window_text_buffers(&mut font_system, &mut buffer, &page.rich_text, 800, 600);

        let cache_keys =
            layout_glyph_cache_keys(&mut font_system, &buffer, &page.rich_text, &layout);
        let upright_index = layout
            .glyphs
            .iter()
            .position(|glyph| glyph.text == "。")
            .expect("upright alternate glyph exists");
        let rotated_index = layout
            .glyphs
            .iter()
            .position(|glyph| glyph.text == "ー")
            .expect("rotated alternate glyph exists");

        assert!(cache_keys.vertical_alternates.contains_key(&upright_index));
        assert!(cache_keys.vertical_alternates.contains_key(&rotated_index));
        let upright_style =
            native_style_for_display_range(&page.rich_text, layout.glyphs[upright_index].range);
        assert!((upright_style.metrics().font_size - 48.0).abs() < f32::EPSILON);
        let default_upright_keys = vertical_form_cache_keys(
            &mut font_system,
            &layout.glyphs[upright_index],
            &NativeTextStyle::default(),
        );
        let sized_upright_keys = cache_keys
            .vertical_alternates
            .get(&upright_index)
            .expect("sized upright alternate keys exist");
        assert!(
            sized_upright_keys[0].advance.x > default_upright_keys[0].advance.x,
            "vertical alternate shaping should use the rich-text size style"
        );
        assert_eq!(
            vertical_form_font_features(GlyphVerticalForm::UprightAlternate).features[0].tag,
            FeatureTag::new(b"vert")
        );
        assert_eq!(
            vertical_form_font_features(GlyphVerticalForm::RotatedAlternate).features[0].tag,
            FeatureTag::new(b"vrtr")
        );
    }

    #[test]
    fn native_layout_reports_text_run_and_ruby_element_bounds() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.bounds".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::Text {
                    text: "Hello ".to_owned(),
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "ゆめ".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let page_range = display_map_non_empty_page_range_at(&frame, 0).expect("page range");
        let page_layout = layout_page_range(
            &frame,
            page_range,
            native_text_layout_config(800, 600, 96.0, 572.0),
        )
        .expect("page layout resolves");
        assert_eq!(page_layout.layout.ruby.len(), 1);
        let bounds = measure_frame_elements_at(&frame, 800, 600, 96.0, 572.0)
            .expect("native layout bounds resolve");

        assert!(bounds.iter().any(|bounds| {
            matches!(bounds.element, NativeFrameElement::TextRun { index: 0 | 1 })
                && bounds.bbox.x >= 96
                && bounds.bbox.y >= 540
        }));
        let cluster = bounds
            .iter()
            .find(|bounds| {
                matches!(
                    bounds.element,
                    NativeFrameElement::GlyphCluster {
                        index: 0,
                        range_start: 0,
                        range_end: 1
                    }
                )
            })
            .expect("first glyph cluster has native bounds");
        assert!(cluster.bbox.x >= 96);
        assert!(cluster.bbox.y >= 540);
        assert!(cluster.bbox.width > 0);
        assert!(cluster.bbox.height > 0);
        let ruby = bounds
            .iter()
            .find(|bounds| matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }))
            .expect("ruby element has native bounds");
        assert!(ruby.bbox.width < 180);
        assert!(ruby.bbox.height < 120);
    }

    #[test]
    fn native_layout_reports_vertical_typewriter_ruby_element_bounds() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.vertical.typewriter.ruby.bounds".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode: RichTextWritingMode::VerticalRl,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::Text {
                    text: "天地春夏秋冬".to_owned(),
                },
                RichTextNode::StyleStart {
                    style: RichTextStyle::Effect {
                        effect: RichTextEffectDescriptor {
                            id: "typewriter".to_owned(),
                            params: BTreeMap::from([(
                                "cps".to_owned(),
                                RichTextParam::Milli { value: Milli::ONE },
                            )]),
                            target: RichTextEffectTarget::Run,
                            phase: RichTextEffectPhase::GlyphMask,
                            state_scope: arcweft_render_text::RichTextStateScope::Run,
                        },
                    },
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "ながいながいよみ".to_owned(),
                },
                RichTextNode::Text {
                    text: "人外".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "effect".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "layout".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let bounds = measure_frame_elements_at(&frame, 1280, 720, 120.0, 572.0)
            .expect("native layout bounds resolve");

        assert!(
            bounds
                .iter()
                .any(|bounds| { matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }) })
        );
    }

    #[test]
    fn native_layout_reports_short_vertical_rl_ruby_at_viewport_edge() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.vertical.short.ruby.edge".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode: RichTextWritingMode::VerticalRl,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::Text {
                    text: "天地春夏秋冬".to_owned(),
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "ゆめ".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "layout".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let bounds = measure_frame_elements_at(&frame, 800, 600, 96.0, 572.0)
            .expect("native layout bounds resolve");

        let ruby = bounds
            .iter()
            .find(|bounds| matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }))
            .expect("short vertical_rl ruby remains observable at the viewport edge");
        let geometry = ruby.ruby.expect("ruby geometry is reported");
        assert!(geometry.annotation_bbox.x >= geometry.base_bbox.x);
        assert!(geometry.annotation_bbox.x < 800);
    }

    #[test]
    fn native_debug_capture_uses_layout_bounds_for_text_elements() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.debug".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::Text {
                    text: "Hello ".to_owned(),
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "ゆめ".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let fallback_bbox = NativeFrameContentBBox {
            x: 1,
            y: 1,
            width: 8,
            height: 8,
        };
        let capture = capture_frame_debug_regions_at(
            &frame,
            800,
            600,
            96.0,
            572.0,
            &[NativeFrameDebugRegion {
                element: Some(NativeFrameElement::Ruby { index: 0 }),
                fallback_bbox,
                color: [255, 255, 255, 255],
            }],
        )
        .expect("debug capture resolves");

        let bbox = capture
            .content_bbox
            .expect("debug capture has visible content");
        assert_ne!(bbox, fallback_bbox);
        assert!(bbox.x >= 96);
        assert!(bbox.y >= 520);
        let bbox_area = u64::from(bbox.width) * u64::from(bbox.height);
        assert!(capture.content_pixels > 0);
        assert!(capture.content_pixels < bbox_area);
    }

    #[test]
    fn native_debug_capture_uses_glyph_area_for_vertical_clusters() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.vertical.cluster.debug".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode: RichTextWritingMode::VerticalRl,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::Text {
                    text: "吾輩".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "layout".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let bounds = measure_frame_elements_at(&frame, 800, 600, 96.0, 572.0)
            .expect("native layout bounds resolve");
        let cluster = bounds
            .iter()
            .find(|bounds| {
                matches!(
                    bounds.element,
                    NativeFrameElement::GlyphCluster {
                        index: 0,
                        range_start: 0,
                        range_end: 3
                    }
                )
            })
            .expect("first vertical glyph cluster has native bounds");
        let capture = capture_frame_debug_regions_at(
            &frame,
            800,
            600,
            96.0,
            572.0,
            &[NativeFrameDebugRegion {
                element: Some(NativeFrameElement::GlyphCluster {
                    index: 0,
                    range_start: 0,
                    range_end: 3,
                }),
                fallback_bbox: NativeFrameContentBBox {
                    x: 1,
                    y: 1,
                    width: 8,
                    height: 8,
                },
                color: [255, 255, 255, 255],
            }],
        )
        .expect("debug capture resolves");

        let bbox = capture
            .content_bbox
            .expect("vertical glyph cluster debug capture has visible content");
        assert!(bbox.x >= cluster.bbox.x);
        assert!(bbox.y >= cluster.bbox.y);
        assert!(bbox.x.saturating_add(bbox.width) <= cluster.bbox.x + cluster.bbox.width);
        assert!(bbox.y.saturating_add(bbox.height) <= cluster.bbox.y + cluster.bbox.height);
        assert!(capture.content_pixels > 0);
    }

    #[test]
    fn native_color_region_capture_preserves_selected_text_style() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.color.region".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: vec![RichTextStyle::from_tag("color", "#ff0000")],
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::Text {
                    text: "Red ".to_owned(),
                },
                RichTextNode::Text {
                    text: "Hidden".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let fallback_bbox = NativeFrameContentBBox {
            x: 1,
            y: 1,
            width: 8,
            height: 8,
        };
        let capture = capture_frame_color_regions_at(
            &frame,
            800,
            600,
            96.0,
            572.0,
            &[NativeFrameDebugRegion {
                element: Some(NativeFrameElement::TextRun { index: 0 }),
                fallback_bbox,
                color: [0, 0, 0, 0],
            }],
        )
        .expect("color region capture resolves");

        let bbox = capture
            .content_bbox
            .expect("color region capture has visible content");
        assert_ne!(bbox, fallback_bbox);
        assert!(bbox.x >= 96);
        assert!(bbox.y >= 540);
        assert!(capture.content_pixels > 0);
        assert!(capture.rgba.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(40)
                && pixel[0] > pixel[2].saturating_add(40)
                && pixel[3] > 0
        }));
    }

    #[test]
    fn native_offscreen_capture_session_reuses_renderer_for_multiple_capture_modes() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.session".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: vec![RichTextStyle::from_tag("color", "#ff0000")],
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::Text {
                    text: "Red ".to_owned(),
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "ゆめ".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let fallback_bbox = NativeFrameContentBBox {
            x: 1,
            y: 1,
            width: 8,
            height: 8,
        };
        let regions = [NativeFrameDebugRegion {
            element: Some(NativeFrameElement::TextRun { index: 0 }),
            fallback_bbox,
            color: [255, 255, 255, 255],
        }];
        let mut session = NativeOffscreenCaptureSession::new().expect("capture session");

        let full = session
            .capture_frame_rgba_at(&frame, 800, 600, 96.0, 572.0)
            .expect("full capture resolves");
        let debug = session
            .capture_frame_debug_regions_at(&frame, 800, 600, 96.0, 572.0, &regions)
            .expect("debug capture resolves");
        let color = session
            .capture_frame_color_regions_at(&frame, 800, 600, 96.0, 572.0, &regions)
            .expect("color capture resolves");

        assert_eq!((full.width, full.height), (800, 600));
        assert!(full.content_pixels > 0);
        assert!(debug.content_pixels > 0);
        assert!(color.content_pixels > 0);
        assert_ne!(debug.content_bbox, Some(fallback_bbox));
        assert!(color.rgba.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(40)
                && pixel[0] > pixel[2].saturating_add(40)
                && pixel[3] > 0
        }));
    }

    #[test]
    fn native_typewriter_capture_changes_visibility_without_relayout() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.typewriter.vertical".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode: RichTextWritingMode::VerticalRl,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::StyleStart {
                    style: RichTextStyle::Effect {
                        effect: RichTextEffectDescriptor {
                            id: "typewriter".to_owned(),
                            params: BTreeMap::from([(
                                "cps".to_owned(),
                                RichTextParam::Milli { value: Milli::ONE },
                            )]),
                            target: RichTextEffectTarget::Run,
                            phase: RichTextEffectPhase::GlyphTransform,
                            state_scope: arcweft_render_text::RichTextStateScope::Run,
                        },
                    },
                },
                RichTextNode::Text {
                    text: "吾輩".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "effect".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "layout".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let at_zero = visual_plan_from_frame_for_test(&frame, 0.0);
        let at_later = visual_plan_from_frame_for_test(&frame, 4.0);
        assert_eq!(
            at_zero.pages[0].glyphs.len(),
            at_later.pages[0].glyphs.len()
        );
        for (hidden, visible) in at_zero.pages[0]
            .glyphs
            .iter()
            .zip(&at_later.pages[0].glyphs)
        {
            assert_eq!(hidden.range, visible.range);
            assert!((hidden.x - visible.x).abs() < f32::EPSILON);
            assert!((hidden.y - visible.y).abs() < f32::EPSILON);
        }
        assert!(
            at_zero.pages[0]
                .glyphs
                .iter()
                .all(|glyph| glyph.opacity == 0.0)
        );
        assert!(
            at_later.pages[0]
                .glyphs
                .iter()
                .all(|glyph| glyph.opacity > 0.0)
        );

        let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
        let hidden = session
            .capture_frame_rgba_in(
                &frame,
                NativeCaptureViewport::new(800, 600, 96.0, 572.0, 0).with_time_seconds(0.0),
            )
            .expect("hidden typewriter capture resolves");
        let visible = session
            .capture_frame_rgba_in(
                &frame,
                NativeCaptureViewport::new(800, 600, 96.0, 572.0, 0).with_time_seconds(4.0),
            )
            .expect("visible typewriter capture resolves");

        assert_eq!(hidden.content_pixels, 0);
        assert!(visible.content_pixels > 0);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn native_debug_ruby_capture_applies_typewriter_visibility() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.typewriter.vertical.ruby.debug".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Layout {
                        layout: RichTextLayout {
                            writing_mode: RichTextWritingMode::VerticalRl,
                            ..RichTextLayout::default()
                        },
                    },
                },
                RichTextNode::Text {
                    text: "天地春夏秋冬".to_owned(),
                },
                RichTextNode::StyleStart {
                    style: RichTextStyle::Effect {
                        effect: RichTextEffectDescriptor {
                            id: "typewriter".to_owned(),
                            params: BTreeMap::from([(
                                "cps".to_owned(),
                                RichTextParam::Milli { value: Milli::ONE },
                            )]),
                            target: RichTextEffectTarget::Run,
                            phase: RichTextEffectPhase::GlyphMask,
                            state_scope: arcweft_render_text::RichTextStateScope::Run,
                        },
                    },
                },
                RichTextNode::Ruby {
                    base: "夢".to_owned(),
                    ruby: "ながいながいよみ".to_owned(),
                },
                RichTextNode::Text {
                    text: "人外".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "effect".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "layout".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let bounds =
            measure_frame_elements_at(&frame, 1280, 720, 120.0, 572.0).expect("bounds resolve");
        let ruby = bounds
            .iter()
            .find(|bounds| matches!(bounds.element, NativeFrameElement::Ruby { index: 0 }))
            .expect("ruby element is observed");
        let region = NativeFrameDebugRegion {
            element: Some(NativeFrameElement::Ruby { index: 0 }),
            fallback_bbox: ruby.bbox,
            color: [255, 255, 255, 255],
        };
        let page_range = display_map_non_empty_page_range_at(&frame, 0).expect("page range");
        let page = page_from_display_map_range(&frame, page_range.clone()).expect("page");
        let debug_rich_text =
            debug_rich_text_for_regions(&frame, &page_range, &page.rich_text, &[region])
                .expect("debug rich text");
        assert!(
            debug_rich_text
                .spans
                .iter()
                .all(|span| span.style.color.alpha == 0)
        );
        assert_eq!(debug_rich_text.ruby_annotations.len(), 1);
        assert_eq!(
            presentation_alpha_for_visibility_time(
                &debug_rich_text.ruby_annotations[0].presentation,
                0.0
            ),
            0
        );
        let mut session = NativeOffscreenCaptureSession::new().expect("capture session");
        let hidden = session
            .capture_frame_debug_regions_in(
                &frame,
                NativeCaptureViewport::new(1280, 720, 120.0, 572.0, 0).with_time_seconds(0.0),
                &[region],
            )
            .expect("hidden ruby debug capture resolves");
        let visible = session
            .capture_frame_debug_regions_in(
                &frame,
                NativeCaptureViewport::new(1280, 720, 120.0, 572.0, 0).with_time_seconds(4.0),
                &[region],
            )
            .expect("visible ruby debug capture resolves");

        assert_eq!(hidden.content_pixels, 0);
        assert!(visible.content_pixels > 0);
    }

    #[test]
    fn window_pages_split_on_display_map_page_line_wait_and_clear_controls() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.002".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: vec![RichTextStyle::from_tag("font", "serif")],
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::Text {
                    text: "one".to_owned(),
                },
                RichTextNode::Control(RichTextControl::Page),
                RichTextNode::Text {
                    text: "two".to_owned(),
                },
                RichTextNode::Control(RichTextControl::LineWait),
                RichTextNode::Text {
                    text: "three".to_owned(),
                },
                RichTextNode::Control(RichTextControl::Clear),
                RichTextNode::Text {
                    text: "four".to_owned(),
                },
            ]),
        };
        let mut frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        frame.nodes.clear();

        let pages = WindowPage::from_frame(&frame);

        assert_eq!(pages.len(), 4);
        assert_eq!(pages[0].rich_text.text, "one");
        assert_eq!(pages[1].rich_text.text, "two");
        assert_eq!(pages[2].rich_text.text, "three");
        assert_eq!(pages[3].rich_text.text, "four");
        assert!(pages.iter().all(|page| {
            page.rich_text
                .spans
                .iter()
                .all(|span| span.style.family == NativeFontFamily::Serif)
        }));
    }

    #[test]
    fn display_map_page_ranges_do_not_split_ruby_base_ranges() {
        let frame = LineDisplayFrame {
            line: RuntimeLineId("say.test.page.ruby.atomic".to_owned()),
            callee: "alice".to_owned(),
            text: "ABCDE".to_owned(),
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            nodes: Vec::new(),
            display_map: arcweft_render_text::RichTextDisplayMap {
                text_runs: vec![
                    arcweft_render_text::RichTextTextRun {
                        range: RichTextRange::new(0, 2),
                        source: arcweft_render_text::RichTextTextSource::Text,
                        node_index: 0,
                        styles: Vec::new(),
                        presentation: RichTextPresentation::default(),
                    },
                    arcweft_render_text::RichTextTextRun {
                        range: RichTextRange::new(2, 5),
                        source: arcweft_render_text::RichTextTextSource::Text,
                        node_index: 2,
                        styles: Vec::new(),
                        presentation: RichTextPresentation::default(),
                    },
                ],
                ruby_annotations: vec![arcweft_render_text::RichTextRubyAnnotation {
                    base_range: RichTextRange::new(1, 4),
                    ruby: "ruby".to_owned(),
                    node_index: 1,
                    styles: Vec::new(),
                    presentation: RichTextPresentation::default(),
                }],
                controls: vec![arcweft_render_text::RichTextControlMarker {
                    node_index: 2,
                    control: RichTextControl::Page,
                    range: None,
                }],
                host_events: Vec::new(),
            },
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        };

        assert_eq!(display_map_page_ranges(&frame), vec![0..4, 4..5]);
        let pages = WindowPage::from_frame(&frame);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].rich_text.text, "ABCD");
        assert_eq!(pages[0].rich_text.ruby_annotations.len(), 1);
        assert_eq!(pages[0].rich_text.ruby_annotations[0].base_range, 1..4);
        assert!(pages[1].rich_text.ruby_annotations.is_empty());
    }

    #[test]
    fn native_capture_content_stats_measure_non_background_bounds() {
        let mut rgba = (0..12).flat_map(|_| [0, 0, 0, 255]).collect::<Vec<_>>();
        let width = 4;
        for (x, y) in [(1_u32, 1_u32), (2, 1), (2, 2)] {
            let index = usize::try_from(y)
                .unwrap()
                .saturating_mul(usize::try_from(width).unwrap())
                .saturating_add(usize::try_from(x).unwrap())
                .saturating_mul(4);
            rgba[index..index + 4].copy_from_slice(&[245, 245, 245, 255]);
        }

        let stats = native_frame_content_stats(&rgba, width, 3, [0, 0, 0, 255]);

        assert_eq!(
            stats.content_bbox,
            Some(NativeFrameContentBBox {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            })
        );
        assert_eq!(stats.content_pixels, 3);
    }

    #[test]
    fn native_capture_row_padding_uses_wgpu_alignment() {
        assert_eq!(padded_rgba_row_bytes(1), COPY_BYTES_PER_ROW_ALIGNMENT);
        assert_eq!(padded_rgba_row_bytes(64), COPY_BYTES_PER_ROW_ALIGNMENT);
        assert_eq!(padded_rgba_row_bytes(65), COPY_BYTES_PER_ROW_ALIGNMENT * 2);
    }

    #[test]
    fn native_visual_plan_reads_raw_effect_params_at_builtin_boundary() {
        let spec = LineDisplaySpec {
            line: RuntimeLineId("say.test.raw.effect".to_owned()),
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![
                RichTextNode::StyleStart {
                    style: RichTextStyle::Effect {
                        effect: RichTextEffectDescriptor {
                            id: "wave".to_owned(),
                            params: BTreeMap::from([
                                (
                                    "amp".to_owned(),
                                    RichTextParam::Raw {
                                        value: "2px".to_owned(),
                                    },
                                ),
                                (
                                    "dir".to_owned(),
                                    RichTextParam::Raw {
                                        value: "0,1".to_owned(),
                                    },
                                ),
                            ]),
                            target: RichTextEffectTarget::Run,
                            phase: RichTextEffectPhase::GlyphTransform,
                            state_scope: arcweft_render_text::RichTextStateScope::Run,
                        },
                    },
                },
                RichTextNode::Text {
                    text: "A".to_owned(),
                },
                RichTextNode::StyleEnd {
                    name: "/".to_owned(),
                },
            ]),
        };
        let frame = spec
            .resolve_frame(&RuntimeLineContext::default())
            .expect("frame resolves");
        let plan = visual_plan_from_frame_for_test(&frame, 0.25);
        let glyph = plan.pages[0].glyphs.first().expect("glyph placement");

        assert!(glyph.x.abs() < f32::EPSILON);
        assert!(glyph.y.abs() > 0.1);
        assert_eq!(plan.pages[0].runs[0].presentation.effects[0].id, "wave");
    }
}
