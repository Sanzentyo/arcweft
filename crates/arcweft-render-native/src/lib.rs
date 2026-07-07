//! Native wgpu/glyphon renderer and capture adapter for Arcweft presentation frames.

use arcweft_core::{
    plan::{RuntimePureHelper, RuntimePureInputType, RuntimePureOutputType},
    pure::VmPureFunctionScratch,
    value::RuntimeValue,
};
use arcweft_glyphon::{
    GlyphonAreaOptions, OwnedGlyphArea, ResolvedGlyph, VerticalGlyphHorizontalAlign,
    glyph_area_from_layout, horizontal_glyph_area_from_shaped_buffer,
    vertical_glyph_area_from_shaped_buffer,
};
use arcweft_render_text::{
    LineDisplayFrame, Milli, RichTextEffectDescriptor, RichTextEffectPhase, RichTextEffectTarget,
    RichTextPresentation, RichTextRange, RichTextShaderRef, RichTextTransformOrigin,
    RichTextWritingMode,
};
use arcweft_text_layout::{
    GlyphOrientation, GlyphVerticalForm, LaidOutGlyph, LaidOutText, LayoutRect, TextLayoutConfig,
};
use arcweft_view::{
    DisplayItemKind, DisplayList, ImageAlignment, ImageFit, LayoutBox, UiImageSourceTable,
    UiResolvedImageFrame,
};
use glyphon::{
    Affine2, Buffer, Cache, Color, FontSystem, GlyphInstance, GlyphTransform, Metrics, Point,
    Resolution, Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Vector,
    Viewport,
    cosmic_text::{FeatureTag, FontFeatures},
};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::mpsc;
use thiserror::Error;
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingResource, BindingType, BlendState, BufferDescriptor, BufferUsages,
    COPY_BYTES_PER_ROW_ALIGNMENT, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
    DeviceDescriptor, Extent3d, FilterMode, FragmentState, Instance, LoadOp, MapMode,
    MultisampleState, Operations, Origin3d, PipelineCompilationOptions, PipelineLayoutDescriptor,
    PollType, PrimitiveState, PrimitiveTopology, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipelineDescriptor, RequestAdapterOptions, SamplerBindingType, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, TexelCopyBufferInfo, TexelCopyBufferLayout,
    TexelCopyTextureInfo, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::Key,
    window::{Window, WindowAttributes},
};

mod capture_session;
mod effect_execution;
mod effects;
mod renderer;
mod visual_layout;
mod window_loop;
mod window_page;

use effect_execution::NativeEffectExecution;
use effects::{
    NativeSparkleEffect, apply_builtin_effect_post_process, apply_parametric_motion_sample,
    apply_presentation_effects_to_placement,
    apply_presentation_effects_to_placement_with_execution,
    apply_presentation_to_placement_with_effects, apply_pure_text_effect_color,
    apply_pure_text_effect_post_process, builtin_effect_phase_supported, deterministic_noise,
    effect_applies_to_glyph_mask, effect_applies_to_renderer_glyph,
    effect_phase_applies_to_renderer_glyph, is_builtin_effect_id, native_screen_tint_post_process,
    native_soft_glow_shader, native_warm_glow_shader, observe_layout_shaders, param_bool,
    param_milli, param_seed, pure_text_shader_glyph_passes, pure_text_shader_post_process,
    resolve_shader_filter, sample_breath_orbit, sample_elastic_bloom, sample_parametric_motion,
    shader_glyph_areas_for_ruby, shader_glyph_areas_for_text, shader_param_milli,
    shader_param_seed, shader_phase_known, stable_text_hash,
};
use renderer::{
    NativeOffscreenTextRenderer, NativeRenderLayout, NativeRenderTarget,
    apply_shaped_horizontal_origins_to_placements, clear_transparent_rgb, fill_native_rect,
    glyph_presentation_affine, key_advances_page, key_closes_window, native_frame_content_stats,
    native_image_rect_for_layout, native_image_transform_milli, native_text_font_features,
    prepare_window_text_buffers, presentation_affine, readback_texture_rgba,
    recolor_image_debug_quad, redraw, render_image_quads_texture, request_capture_device,
    rounded_u8, shaped_horizontal_glyph_metrics, solid_rgba, surface_extent_f32,
    typewriter_cursor_opacity, typewriter_visible_count, usize_to_f32_saturating,
    vertical_ruby_glyph_horizontal_align,
};
use visual_layout::{
    NativePageLayout, glyph_orientation_degrees, layout_page_range,
    layout_page_range_with_selected_text, native_element_bounds_from_layout_at,
    native_glyph_placements_for_layout, native_glyph_placements_for_layout_with_effects,
    native_text_layout_config, native_text_layout_config_at, page_local_layout_frame,
    visual_page_from_range,
};
pub use window_loop::{
    NativeWindowLoopControl, NativeWindowLoopDriver, NativeWindowLoopInput,
    run_driven_frames_window,
};
use window_page::{
    Application, NATIVE_GLYPHAREA_BASELINE_OFFSET, NATIVE_TEXT_LEFT, NATIVE_TEXT_TOP,
    NativeTextOrigin, NativeTextStyle, RubyGlyphPlacement, WindowPage, WindowRichText,
    WindowRubyBuffer, WindowState, build_ruby_buffers, color_rich_text_for_regions,
    color_selected_text_ranges, debug_rich_text_for_regions, debug_selected_text_ranges,
    display_map_non_empty_page_range_at, display_map_page_ranges, intersect_display_range,
    native_float_bbox, page_from_display_map_range, post_process_effects_for_page,
    post_process_effects_for_regions, post_process_shaders_for_page,
    post_process_shaders_for_regions, run_pages_window, text_line_start_offsets,
    valid_display_range,
};

/// Native player window error.
#[derive(Debug, Error)]
pub enum NativeWindowError {
    #[error("event loop error: {0}")]
    EventLoop(String),
    #[error("no display pages were provided")]
    EmptyPages,
    #[error("event-loop window driver failed: {0}")]
    Driver(String),
    #[error("readback failed: {0}")]
    Readback(String),
    #[error("text layout failed: {0}")]
    TextLayout(String),
    #[error("image render failed: {0}")]
    Image(String),
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
    /// Renderer diagnostics produced while preparing the captured glyph areas.
    pub diagnostics: Vec<NativeVisualDiagnostic>,
}

/// Pixel-space destination rectangle for a native image quad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeImageRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Affine transform applied to a native image quad in viewport pixel space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeImageTransform {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub tx: f32,
    pub ty: f32,
}

/// RGBA8 image frame submitted as a textured quad to the native renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeImageQuad<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub opacity_milli: u16,
    pub dst: NativeImageRect,
    pub transform: NativeImageTransform,
}

/// Image quad recolored for native mask or object-id capture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeImageDebugQuad<'a> {
    pub quad: NativeImageQuad<'a>,
    pub color: [u8; 4],
}

impl NativeImageTransform {
    pub const fn identity() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }
}

/// Pixel-space bounds of non-background framebuffer content.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct NativeFrameContentBBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeRenderReadback {
    rgba: Vec<u8>,
    diagnostics: Vec<NativeVisualDiagnostic>,
}

/// Native rich-text element kinds addressable by Agent debug captures.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NativeFrameElement {
    TextRun {
        index: usize,
    },
    TextObjectProxy {
        run_index: usize,
        proxy_index: usize,
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
    pub diagnostics: Vec<NativeVisualDiagnostic>,
}

/// Diagnostic produced while resolving a native rich-text visual plan.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct NativeVisualDiagnostic {
    pub severity: NativeVisualDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub effect_id: Option<String>,
}

/// Native visual-plan diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeVisualDiagnosticSeverity {
    Info,
    Warning,
    Error,
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
    pub skew_x_degrees: f32,
    pub skew_y_degrees: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affine_origin: Option<RichTextTransformOrigin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affine_target: Option<RichTextEffectTarget>,
    pub vertical_form: GlyphVerticalForm,
    pub scale_x: f32,
    pub scale_y: f32,
    pub opacity: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 4]>,
}

/// Host-resolved shader/filter reference for one native page.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct NativeResolvedShaderFilter {
    pub id: String,
    pub phase: RichTextEffectPhase,
    pub amount: f32,
    pub direction: [f32; 2],
}

/// One renderer-owned glyph-area pass emitted by a rich-text shader registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeShaderGlyphPass {
    pub offset: [f32; 2],
    pub color: [u8; 4],
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

    pub fn clear(&mut self) {
        self.values.clear();
    }
}

/// Per-glyph effect context supplied to renderer-local effect implementations.
pub struct TextEffectGlyphContext<'a> {
    pub effect: &'a RichTextEffectDescriptor,
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

    fn post_process(
        &mut self,
        _ctx: &mut TextEffectPostProcessContext<'_>,
        _rgba: &mut [u8],
    ) -> bool {
        false
    }
}

pub type GlyphLambda = Box<dyn FnMut(&mut TextEffectGlyphContext<'_>) + Send + 'static>;
pub type EffectPostProcessLambda =
    Box<dyn FnMut(&mut TextEffectPostProcessContext<'_>, &mut [u8]) + Send + 'static>;

pub enum RegisteredTextEffect {
    Class(Box<dyn RichTextEffectClass>),
    Lambda(GlyphLambda),
    PostProcessLambda(EffectPostProcessLambda),
    Combined {
        glyph: GlyphLambda,
        post_process: EffectPostProcessLambda,
    },
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

    pub fn insert_post_process_lambda(
        &mut self,
        id: impl Into<String>,
        effect: impl FnMut(&mut TextEffectPostProcessContext<'_>, &mut [u8]) + Send + 'static,
    ) {
        self.effects.insert(
            id.into(),
            RegisteredTextEffect::PostProcessLambda(Box::new(effect)),
        );
    }

    pub fn insert_combined_lambda(
        &mut self,
        id: impl Into<String>,
        glyph: impl FnMut(&mut TextEffectGlyphContext<'_>) + Send + 'static,
        post_process: impl FnMut(&mut TextEffectPostProcessContext<'_>, &mut [u8]) + Send + 'static,
    ) {
        self.effects.insert(
            id.into(),
            RegisteredTextEffect::Combined {
                glyph: Box::new(glyph),
                post_process: Box::new(post_process),
            },
        );
    }

    pub fn contains(&self, id: &str) -> bool {
        self.effects.contains_key(id)
    }

    pub fn supports_phase(&self, id: &str, phase: RichTextEffectPhase) -> bool {
        let Some(effect) = self.effects.get(id) else {
            return false;
        };
        match effect {
            RegisteredTextEffect::Class(_) | RegisteredTextEffect::Combined { .. } => {
                phase == RichTextEffectPhase::PostProcess
                    || effect_phase_applies_to_renderer_glyph(phase)
            }
            RegisteredTextEffect::Lambda(_) => effect_phase_applies_to_renderer_glyph(phase),
            RegisteredTextEffect::PostProcessLambda(_) => phase == RichTextEffectPhase::PostProcess,
        }
    }

    pub fn apply_host_effect(&mut self, id: &str, ctx: &mut TextEffectGlyphContext<'_>) -> bool {
        let Some(effect) = self.effects.get_mut(id) else {
            return false;
        };
        match effect {
            RegisteredTextEffect::Class(effect) => effect.apply_glyph(ctx),
            RegisteredTextEffect::Lambda(effect) => effect(ctx),
            RegisteredTextEffect::PostProcessLambda(_) => return false,
            RegisteredTextEffect::Combined { glyph, .. } => glyph(ctx),
        }
        true
    }

    pub fn post_process(
        &mut self,
        id: &str,
        ctx: &mut TextEffectPostProcessContext<'_>,
        rgba: &mut [u8],
    ) -> Option<bool> {
        let effect = self.effects.get_mut(id)?;
        Some(match effect {
            RegisteredTextEffect::Class(effect) => effect.post_process(ctx, rgba),
            RegisteredTextEffect::Lambda(_) => false,
            RegisteredTextEffect::PostProcessLambda(effect) => {
                effect(ctx, rgba);
                true
            }
            RegisteredTextEffect::Combined { post_process, .. } => {
                post_process(ctx, rgba);
                true
            }
        })
    }
}

/// Per-effect context supplied to renderer-local effect post-process passes.
pub struct TextEffectPostProcessContext<'a> {
    pub effect: &'a RichTextEffectDescriptor,
    pub time_seconds: f32,
    pub line_id: &'a str,
    pub width: u32,
    pub height: u32,
    pub state: &'a mut RichTextStateStore,
}

/// Per-shader context supplied to renderer-local shader implementations.
pub struct TextShaderContext<'a> {
    pub shader: &'a RichTextShaderRef,
}

/// Per-post-process context supplied to renderer-local shader implementations.
pub struct TextShaderPostProcessContext<'a> {
    pub shader: &'a RichTextShaderRef,
    pub width: u32,
    pub height: u32,
    pub time_seconds: f32,
}

/// Renderer-local rich-text shader.
pub trait RichTextShaderClass: Send {
    fn glyph_passes(&mut self, ctx: &TextShaderContext<'_>) -> Vec<NativeShaderGlyphPass>;

    fn post_process(&mut self, _ctx: &TextShaderPostProcessContext<'_>, _rgba: &mut [u8]) -> bool {
        false
    }
}

pub type ShaderLambda =
    Box<dyn FnMut(&TextShaderContext<'_>) -> Vec<NativeShaderGlyphPass> + Send + 'static>;
pub type ShaderPostProcessLambda =
    Box<dyn FnMut(&TextShaderPostProcessContext<'_>, &mut [u8]) + Send + 'static>;

pub enum RegisteredTextShader {
    Class(Box<dyn RichTextShaderClass>),
    Lambda(ShaderLambda),
    PostProcessLambda(ShaderPostProcessLambda),
    Combined {
        glyph: ShaderLambda,
        post_process: ShaderPostProcessLambda,
    },
}

/// Registry that resolves `RichTextShaderRef::id` to native shader passes.
#[derive(Default)]
pub struct RichTextShaderRegistry {
    shaders: BTreeMap<String, RegisteredTextShader>,
}

impl RichTextShaderRegistry {
    pub fn insert_class(
        &mut self,
        id: impl Into<String>,
        shader: impl RichTextShaderClass + 'static,
    ) {
        self.shaders
            .insert(id.into(), RegisteredTextShader::Class(Box::new(shader)));
    }

    pub fn insert_lambda(
        &mut self,
        id: impl Into<String>,
        shader: impl FnMut(&TextShaderContext<'_>) -> Vec<NativeShaderGlyphPass> + Send + 'static,
    ) {
        self.shaders
            .insert(id.into(), RegisteredTextShader::Lambda(Box::new(shader)));
    }

    pub fn insert_post_process_lambda(
        &mut self,
        id: impl Into<String>,
        shader: impl FnMut(&TextShaderPostProcessContext<'_>, &mut [u8]) + Send + 'static,
    ) {
        self.shaders.insert(
            id.into(),
            RegisteredTextShader::PostProcessLambda(Box::new(shader)),
        );
    }

    pub fn insert_combined_lambda(
        &mut self,
        id: impl Into<String>,
        glyph: impl FnMut(&TextShaderContext<'_>) -> Vec<NativeShaderGlyphPass> + Send + 'static,
        post_process: impl FnMut(&TextShaderPostProcessContext<'_>, &mut [u8]) + Send + 'static,
    ) {
        self.shaders.insert(
            id.into(),
            RegisteredTextShader::Combined {
                glyph: Box::new(glyph),
                post_process: Box::new(post_process),
            },
        );
    }

    pub fn contains(&self, id: &str) -> bool {
        self.shaders.contains_key(id)
    }

    pub fn supports_phase(&self, id: &str, phase: RichTextEffectPhase) -> bool {
        let Some(shader) = self.shaders.get(id) else {
            return false;
        };
        match shader {
            RegisteredTextShader::Class(_) => matches!(
                phase,
                RichTextEffectPhase::RunOffscreenPass
                    | RichTextEffectPhase::GlyphColor
                    | RichTextEffectPhase::PostProcess
            ),
            RegisteredTextShader::Lambda(_) => matches!(
                phase,
                RichTextEffectPhase::RunOffscreenPass | RichTextEffectPhase::GlyphColor
            ),
            RegisteredTextShader::PostProcessLambda(_) => phase == RichTextEffectPhase::PostProcess,
            RegisteredTextShader::Combined { .. } => matches!(
                phase,
                RichTextEffectPhase::RunOffscreenPass
                    | RichTextEffectPhase::GlyphColor
                    | RichTextEffectPhase::PostProcess
            ),
        }
    }

    pub fn glyph_passes(
        &mut self,
        id: &str,
        ctx: &TextShaderContext<'_>,
    ) -> Option<Vec<NativeShaderGlyphPass>> {
        let shader = self.shaders.get_mut(id)?;
        Some(match shader {
            RegisteredTextShader::Class(shader) => shader.glyph_passes(ctx),
            RegisteredTextShader::Lambda(shader) => shader(ctx),
            RegisteredTextShader::PostProcessLambda(_) => Vec::new(),
            RegisteredTextShader::Combined { glyph, .. } => glyph(ctx),
        })
    }

    pub fn post_process(
        &mut self,
        id: &str,
        ctx: &TextShaderPostProcessContext<'_>,
        rgba: &mut [u8],
    ) -> Option<bool> {
        let shader = self.shaders.get_mut(id)?;
        Some(match shader {
            RegisteredTextShader::Class(shader) => shader.post_process(ctx, rgba),
            RegisteredTextShader::Lambda(_) => false,
            RegisteredTextShader::PostProcessLambda(shader) => {
                shader(ctx, rgba);
                true
            }
            RegisteredTextShader::Combined { post_process, .. } => {
                post_process(ctx, rgba);
                true
            }
        })
    }
}

/// A normalized animation sample returned by renderer-local rich-text motion functions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeAnimationSample {
    pub translate: [f32; 2],
    pub rotate: f32,
    pub scale: f32,
}

impl Default for NativeAnimationSample {
    fn default() -> Self {
        Self {
            translate: [0.0, 0.0],
            rotate: 0.0,
            scale: 0.0,
        }
    }
}

/// Per-motion context supplied to renderer-local animation functions.
pub struct TextMotionContext<'a> {
    pub effect: &'a RichTextEffectDescriptor,
    pub function: &'a str,
    pub sample_time: f32,
    pub line_id: &'a str,
    pub run_index: usize,
    pub glyph_index: usize,
    pub glyph_count: usize,
    pub noise: [f32; 2],
}

/// Renderer-local rich-text motion function.
pub trait RichTextMotionClass: Send {
    fn sample(&mut self, ctx: &TextMotionContext<'_>) -> NativeAnimationSample;
}

pub type MotionLambda = Box<dyn FnMut(&TextMotionContext<'_>) -> NativeAnimationSample + Send>;

pub enum RegisteredTextMotion {
    Class(Box<dyn RichTextMotionClass>),
    Lambda(MotionLambda),
}

/// Registry that resolves `.motion fn=...` references to native animation functions.
#[derive(Default)]
pub struct RichTextMotionRegistry {
    functions: BTreeMap<String, RegisteredTextMotion>,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RichTextMotionExportError {
    #[error(
        "text motion function `{name}` must have signature fn(t: f32, glyph: f32, seed: f32) -> f32"
    )]
    UnsupportedSignature { name: String },
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RichTextEffectExportError {
    #[error(
        "text effect function `{name}` must have signature fn(t: f32, glyph: f32, seed: f32) -> f32"
    )]
    UnsupportedSignature { name: String },
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RichTextShaderExportError {
    #[error(
        "text shader function `{name}` must have signature fn(t: f32, glyph: f32, seed: f32) -> f32"
    )]
    UnsupportedSignature { name: String },
}

impl RichTextMotionRegistry {
    pub fn insert_class(
        &mut self,
        id: impl Into<String>,
        function: impl RichTextMotionClass + 'static,
    ) {
        self.functions
            .insert(id.into(), RegisteredTextMotion::Class(Box::new(function)));
    }

    pub fn insert_lambda(
        &mut self,
        id: impl Into<String>,
        function: impl FnMut(&TextMotionContext<'_>) -> NativeAnimationSample + Send + 'static,
    ) {
        self.functions
            .insert(id.into(), RegisteredTextMotion::Lambda(Box::new(function)));
    }

    pub fn contains(&self, id: &str) -> bool {
        self.functions.contains_key(id)
    }

    pub fn sample(
        &mut self,
        id: &str,
        ctx: &TextMotionContext<'_>,
    ) -> Option<NativeAnimationSample> {
        let function = self.functions.get_mut(id)?;
        Some(match function {
            RegisteredTextMotion::Class(function) => function.sample(ctx),
            RegisteredTextMotion::Lambda(function) => function(ctx),
        })
    }
}

pub fn register_arcweft_pure_text_shaders(
    registry: &mut RichTextShaderRegistry,
    helpers: &[RuntimePureHelper],
) -> Result<usize, RichTextShaderExportError> {
    helpers.iter().try_fold(0usize, |exported, helper| {
        register_arcweft_pure_text_shader(registry, helper)?;
        Ok(exported.saturating_add(1))
    })
}

fn register_arcweft_pure_text_shader(
    registry: &mut RichTextShaderRegistry,
    helper: &RuntimePureHelper,
) -> Result<(), RichTextShaderExportError> {
    if !arcweft_text_pure_f32_triplet_signature_supported(helper) {
        return Err(RichTextShaderExportError::UnsupportedSignature {
            name: helper.name.clone(),
        });
    }
    let glyph_helper = helper.clone();
    let post_process_helper = glyph_helper.clone();
    let mut glyph_scratch = VmPureFunctionScratch::default();
    let mut post_process_scratch = VmPureFunctionScratch::default();
    registry.insert_combined_lambda(
        helper.name.clone(),
        move |ctx| {
            let seed = shader_param_seed(ctx.shader, "seed").map_or(0.0, seed_bucket_as_f32);
            let time = shader_param_milli(ctx.shader, "time")
                .or_else(|| shader_param_milli(ctx.shader, "t"))
                .or_else(|| shader_param_milli(ctx.shader, "phase"))
                .unwrap_or_default()
                .as_f32();
            let phase = glyph_scratch
                .evaluate_f32_slice(&glyph_helper, &[time, 0.0, seed])
                .ok()
                .as_ref()
                .and_then(runtime_value_as_f32)
                .filter(|value| value.is_finite())
                .unwrap_or(time);
            pure_text_shader_glyph_passes(ctx.shader, phase)
        },
        move |ctx, rgba| {
            let seed = shader_param_seed(ctx.shader, "seed").map_or(0.0, seed_bucket_as_f32);
            let time = shader_param_milli(ctx.shader, "time")
                .or_else(|| shader_param_milli(ctx.shader, "t"))
                .or_else(|| shader_param_milli(ctx.shader, "phase"))
                .map_or(ctx.time_seconds, |value| ctx.time_seconds + value.as_f32());
            let phase = post_process_scratch
                .evaluate_f32_slice(&post_process_helper, &[time, 0.0, seed])
                .ok()
                .as_ref()
                .and_then(runtime_value_as_f32)
                .filter(|value| value.is_finite())
                .unwrap_or(time);
            pure_text_shader_post_process(ctx.shader, phase, rgba);
        },
    );
    Ok(())
}

pub fn register_arcweft_pure_text_effects(
    registry: &mut RichTextEffectRegistry,
    helpers: &[RuntimePureHelper],
) -> Result<usize, RichTextEffectExportError> {
    helpers.iter().try_fold(0usize, |exported, helper| {
        register_arcweft_pure_text_effect(registry, helper)?;
        Ok(exported.saturating_add(1))
    })
}

fn register_arcweft_pure_text_effect(
    registry: &mut RichTextEffectRegistry,
    helper: &RuntimePureHelper,
) -> Result<(), RichTextEffectExportError> {
    if !arcweft_text_pure_f32_triplet_signature_supported(helper) {
        return Err(RichTextEffectExportError::UnsupportedSignature {
            name: helper.name.clone(),
        });
    }
    let glyph_helper = helper.clone();
    let post_process_helper = glyph_helper.clone();
    let mut glyph_scratch = VmPureFunctionScratch::default();
    let mut post_process_scratch = VmPureFunctionScratch::default();
    registry.insert_combined_lambda(
        helper.name.clone(),
        move |ctx| {
            let seed = param_seed(ctx.effect, "seed").map_or(0.0, seed_bucket_as_f32);
            let phase = glyph_scratch
                .evaluate_f32_slice(
                    &glyph_helper,
                    &[
                        ctx.time_seconds,
                        usize_to_f32_saturating(ctx.glyph_index),
                        seed,
                    ],
                )
                .ok()
                .as_ref()
                .and_then(runtime_value_as_f32)
                .filter(|value| value.is_finite())
                .unwrap_or(ctx.time_seconds);
            if ctx.effect.phase == RichTextEffectPhase::GlyphColor {
                apply_pure_text_effect_color(ctx, phase);
                return;
            }
            let effect_seed = param_seed(ctx.effect, "seed").unwrap_or(0)
                ^ stable_text_hash(&glyph_helper.name)
                ^ u64::try_from(ctx.run_index).unwrap_or(u64::MAX);
            let noise = deterministic_noise(effect_seed, ctx.line_id, ctx.glyph_index, phase);
            let sample = sample_parametric_motion(ctx.effect, phase, noise);
            apply_parametric_motion_sample(ctx.effect, sample, ctx.placement);
        },
        move |ctx, rgba| {
            let seed = param_seed(ctx.effect, "seed").map_or(0.0, seed_bucket_as_f32);
            let phase = post_process_scratch
                .evaluate_f32_slice(&post_process_helper, &[ctx.time_seconds, 0.0, seed])
                .ok()
                .as_ref()
                .and_then(runtime_value_as_f32)
                .filter(|value| value.is_finite())
                .unwrap_or(ctx.time_seconds);
            apply_pure_text_effect_post_process(ctx.effect, phase, rgba);
        },
    );
    Ok(())
}

pub fn register_arcweft_pure_text_motions(
    registry: &mut RichTextMotionRegistry,
    helpers: &[RuntimePureHelper],
) -> Result<usize, RichTextMotionExportError> {
    helpers.iter().try_fold(0usize, |exported, helper| {
        register_arcweft_pure_text_motion(registry, helper)?;
        Ok(exported.saturating_add(1))
    })
}

fn register_arcweft_pure_text_motion(
    registry: &mut RichTextMotionRegistry,
    helper: &RuntimePureHelper,
) -> Result<(), RichTextMotionExportError> {
    if !arcweft_text_pure_f32_triplet_signature_supported(helper) {
        return Err(RichTextMotionExportError::UnsupportedSignature {
            name: helper.name.clone(),
        });
    }
    let helper = helper.clone();
    let mut scratch = VmPureFunctionScratch::default();
    registry.insert_lambda(helper.name.clone(), move |ctx| {
        let seed = param_seed(ctx.effect, "seed").map_or(0.0, seed_bucket_as_f32);
        let phase = scratch
            .evaluate_f32_slice(
                &helper,
                &[
                    ctx.sample_time,
                    usize_to_f32_saturating(ctx.glyph_index),
                    seed,
                ],
            )
            .ok()
            .as_ref()
            .and_then(runtime_value_as_f32)
            .filter(|value| value.is_finite())
            .unwrap_or(ctx.sample_time);
        sample_parametric_motion(ctx.effect, phase, ctx.noise)
    });
    Ok(())
}

fn arcweft_text_pure_f32_triplet_signature_supported(helper: &RuntimePureHelper) -> bool {
    helper.input_types
        == [
            RuntimePureInputType::F32,
            RuntimePureInputType::F32,
            RuntimePureInputType::F32,
        ]
        && helper.output_type == RuntimePureOutputType::F32
        && helper.scalar_eval_supported
}

fn runtime_value_as_f32(value: &RuntimeValue) -> Option<f32> {
    match value {
        RuntimeValue::F32(value) => Some(*value),
        _ => None,
    }
}

fn seed_bucket_as_f32(seed: u64) -> f32 {
    f32::from(u16::try_from(seed & 0xffff).expect("masked seed fits u16"))
}

/// Builds the default native host effect registry used by native renderers.
pub fn native_default_effect_registry() -> RichTextEffectRegistry {
    let mut registry = RichTextEffectRegistry::default();
    register_native_default_text_effects(&mut registry);
    registry
}

/// Builds the default native shader registry used by native renderers.
pub fn native_default_shader_registry() -> RichTextShaderRegistry {
    let mut registry = RichTextShaderRegistry::default();
    register_native_default_text_shaders(&mut registry);
    registry
}

/// Builds the default native motion registry used by native renderers.
pub fn native_default_motion_registry() -> RichTextMotionRegistry {
    let mut registry = RichTextMotionRegistry::default();
    register_native_default_text_motions(&mut registry);
    registry
}

/// Registers native host effects that are available without external adapters.
pub fn register_native_default_text_effects(registry: &mut RichTextEffectRegistry) {
    registry.insert_class("sparkle", NativeSparkleEffect);
}

/// Registers native shaders that are available without external adapters.
pub fn register_native_default_text_shaders(registry: &mut RichTextShaderRegistry) {
    registry.insert_lambda("soft_glow", native_soft_glow_shader);
    registry.insert_lambda("warm_glow", native_warm_glow_shader);
    registry.insert_post_process_lambda("screen_tint", native_screen_tint_post_process);
}

/// Registers native motion functions that are available without external adapters.
pub fn register_native_default_text_motions(registry: &mut RichTextMotionRegistry) {
    registry.insert_lambda("breath_orbit", |ctx| {
        sample_breath_orbit(ctx.sample_time, ctx.noise)
    });
    registry.insert_lambda("fx.breath_orbit", |ctx| {
        sample_breath_orbit(ctx.sample_time, ctx.noise)
    });
    registry.insert_lambda("elastic_bloom", |ctx| {
        sample_elastic_bloom(ctx.sample_time, ctx.noise)
    });
    registry.insert_lambda("fx.elastic_bloom", |ctx| {
        sample_elastic_bloom(ctx.sample_time, ctx.noise)
    });
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
    effect_registry: RichTextEffectRegistry,
    shader_registry: RichTextShaderRegistry,
    motion_registry: RichTextMotionRegistry,
    effect_state: RichTextStateStore,
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

/// Renders image quads into an offscreen native RGBA framebuffer.
pub fn capture_image_quads_rgba(
    quads: &[NativeImageQuad<'_>],
    width: u32,
    height: u32,
) -> Result<NativeFrameCapture, NativeWindowError> {
    NativeOffscreenCaptureSession::new()?.capture_image_quads_rgba(quads, width, height)
}

/// Resolves UI image display items into native textured-quad submissions.
///
/// Frame selection is driven only by the supplied presentation visual time, so
/// static and animated image capture remains deterministic and replayable.
pub fn native_image_quads_from_display_list<'a>(
    display: &DisplayList,
    images: &'a UiImageSourceTable,
    visual_time_millis: u64,
) -> Result<Vec<NativeImageQuad<'a>>, NativeWindowError> {
    display
        .as_slice()
        .iter()
        .filter_map(|item| match item.kind() {
            DisplayItemKind::Image(image) => Some(
                images
                    .resolve_frame(image, item.layout(), visual_time_millis)
                    .map_err(|error| NativeWindowError::Image(error.to_string()))
                    .and_then(native_image_quad_from_resolved_frame),
            ),
            DisplayItemKind::Text(_)
            | DisplayItemKind::RichText(_)
            | DisplayItemKind::Custom(_) => None,
        })
        .collect()
}

/// Converts one resolved UI image frame into the native renderer's quad shape.
pub fn native_image_quad_from_resolved_frame(
    resolved: UiResolvedImageFrame<'_>,
) -> Result<NativeImageQuad<'_>, NativeWindowError> {
    let frame = resolved.frame();
    let dimensions = frame.dimensions();
    let transform = resolved.transform();
    Ok(NativeImageQuad {
        width: dimensions.width(),
        height: dimensions.height(),
        rgba: frame.rgba(),
        opacity_milli: resolved.opacity_milli(),
        dst: native_image_rect_for_layout(
            resolved.layout(),
            resolved.fit(),
            resolved.alignment(),
            dimensions.width(),
            dimensions.height(),
        )?,
        transform: native_image_transform_milli([
            transform.m11_milli,
            transform.m12_milli,
            transform.m21_milli,
            transform.m22_milli,
            transform.tx_milli,
            transform.ty_milli,
        ]),
    })
}

/// Renders recolored image quads into an offscreen native RGBA framebuffer.
pub fn capture_image_debug_quads_rgba(
    quads: &[NativeImageDebugQuad<'_>],
    width: u32,
    height: u32,
) -> Result<NativeFrameCapture, NativeWindowError> {
    NativeOffscreenCaptureSession::new()?.capture_image_debug_quads_rgba(quads, width, height)
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
    measure_frame_elements_at_page_with_time(frame, width, height, left, top, page_index, 0.0)
}

/// Measures native rich-text element bounds for a rendered page at an effect time.
pub fn measure_frame_elements_at_page_with_time(
    frame: &LineDisplayFrame,
    width: u32,
    height: u32,
    left: f32,
    top: f32,
    page_index: usize,
    time_seconds: f32,
) -> Result<Vec<NativeFrameElementBounds>, NativeWindowError> {
    let mut state = RichTextStateStore::default();
    let mut shader_registry = native_default_shader_registry();
    let mut motion_registry = native_default_motion_registry();
    measure_frame_elements_at_page_with_effects(
        frame,
        NativeCaptureViewport::new(width, height, left, top, page_index),
        time_seconds,
        None,
        Some(&mut shader_registry),
        Some(&mut motion_registry),
        &mut state,
    )
}

/// Measures native rich-text element bounds using renderer-local custom effects.
pub fn measure_frame_elements_with_effect_registry(
    frame: &LineDisplayFrame,
    viewport: NativeCaptureViewport,
    registry: &mut RichTextEffectRegistry,
    state: &mut RichTextStateStore,
) -> Result<Vec<NativeFrameElementBounds>, NativeWindowError> {
    let mut shader_registry = native_default_shader_registry();
    let mut motion_registry = native_default_motion_registry();
    measure_frame_elements_at_page_with_effects(
        frame,
        viewport,
        viewport.time_seconds,
        Some(registry),
        Some(&mut shader_registry),
        Some(&mut motion_registry),
        state,
    )
}

fn measure_frame_elements_at_page_with_effects(
    frame: &LineDisplayFrame,
    viewport: NativeCaptureViewport,
    time_seconds: f32,
    registry: Option<&mut RichTextEffectRegistry>,
    shader_registry: Option<&mut RichTextShaderRegistry>,
    motion_registry: Option<&mut RichTextMotionRegistry>,
    state: &mut RichTextStateStore,
) -> Result<Vec<NativeFrameElementBounds>, NativeWindowError> {
    let page_range = display_map_non_empty_page_range_at(frame, viewport.page_index)?;
    let mut effects = NativeEffectExecution::new(registry, shader_registry, motion_registry, state);
    let page_layout = layout_page_range(
        frame,
        page_range,
        native_text_layout_config_at(
            viewport.width.max(1),
            viewport.height.max(1),
            viewport.left,
            viewport.top,
            time_seconds,
        ),
    )?;
    Ok(native_element_bounds_from_layout_at(
        &page_layout,
        viewport.width.max(1),
        viewport.height.max(1),
        time_seconds,
        Some(&mut effects),
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
    let mut state = RichTextStateStore::default();
    let mut shader_registry = native_default_shader_registry();
    let mut motion_registry = native_default_motion_registry();
    visual_plan_from_frame_with_effects(
        frame,
        time_seconds,
        None,
        Some(&mut shader_registry),
        Some(&mut motion_registry),
        &mut state,
    )
}

/// Builds a deterministic native visual plan using renderer-local custom effects.
pub fn visual_plan_from_frame_with_effect_registry(
    frame: &LineDisplayFrame,
    time_seconds: f32,
    registry: &mut RichTextEffectRegistry,
    state: &mut RichTextStateStore,
) -> NativeVisualPlan {
    let mut shader_registry = native_default_shader_registry();
    let mut motion_registry = native_default_motion_registry();
    visual_plan_from_frame_with_effects(
        frame,
        time_seconds,
        Some(registry),
        Some(&mut shader_registry),
        Some(&mut motion_registry),
        state,
    )
}

fn visual_plan_from_frame_with_effects(
    frame: &LineDisplayFrame,
    time_seconds: f32,
    registry: Option<&mut RichTextEffectRegistry>,
    shader_registry: Option<&mut RichTextShaderRegistry>,
    motion_registry: Option<&mut RichTextMotionRegistry>,
    state: &mut RichTextStateStore,
) -> NativeVisualPlan {
    let mut effects = NativeEffectExecution::new(registry, shader_registry, motion_registry, state);
    let mut pages = Vec::new();
    let page_ranges = display_map_page_ranges(frame)
        .into_iter()
        .enumerate()
        .collect::<Vec<_>>();
    for (page_index, page_range) in page_ranges {
        if let Some(page) =
            visual_page_from_range(frame, page_index, page_range, time_seconds, &mut effects)
        {
            pages.push(page);
        }
    }
    NativeVisualPlan {
        pages,
        diagnostics: effects.into_diagnostics(),
    }
}

/// Test-facing helper for deterministic visual-plan snapshots.
pub fn visual_plan_from_frame_for_test(
    frame: &LineDisplayFrame,
    time_seconds: f32,
) -> NativeVisualPlan {
    visual_plan_from_frame(frame, time_seconds)
}

#[cfg(test)]
mod tests;
