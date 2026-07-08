//! wgpu compositor substrate for seq06.9a View paint nodes.
//!
//! `ViewCompositorPlan` is the deterministic, testable pass graph. `ViewCompositor`
//! owns the wgpu pipelines, offscreen texture pool, and callback boundaries that
//! let the existing renderer draw direct primitive ranges inside a group target.

use crate::renderer::SharedRenderer;
use crate::view_blend::{ViewBlendPassPlan, ViewBlendShaderMode};
use crate::view_box_shadow::{ViewBoxShadowPassPlan, ViewBoxShadowPlanError};
use crate::view_clip_path::{ViewClipGeometryPlan, ViewClipPathPlanError};
use crate::view_compositor_uniform::ViewCompositorUniform;
use crate::view_effects::{ViewEffectPass, ViewFilterPassPlan, ViewTextureExtent};
use crate::view_mask::{ViewMaskChainPlan, ViewMaskChannel, ViewMaskImagePlan, ViewMaskPlanError};
use crate::view_scene::{
    ViewBlendMode, ViewBoxShadowKind, ViewCompositingGroup, ViewFilterList, ViewMaskImage,
    ViewPaintNode, ViewPrimitiveRange, ViewScene, ViewSceneContext,
};
use arcweft_presentation::hit::HitRect;
use num_traits::ToPrimitive;
use thiserror::Error;
use wgpu::util::DeviceExt;

/// Pure compositor pass graph for one scene.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewCompositorPlan {
    root_extent: ViewTextureExtent,
    nodes: Vec<ViewCompositorNodePlan>,
    offscreen_target_count: usize,
    shader_pass_count: usize,
    backdrop_copy_count: usize,
}

/// Pure per-node plan used by tests and GPU execution.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewCompositorNodePlan {
    Direct {
        primitive_range: ViewPrimitiveRange,
    },
    Group {
        visual_extent: ViewTextureExtent,
        effects: Box<ViewGroupEffectPlan>,
        children: Vec<ViewCompositorNodePlan>,
    },
}

/// Effect plans attached to one compositing group.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewGroupEffectPlan {
    pub box_shadows: Result<ViewBoxShadowPassPlan, ViewBoxShadowPlanError>,
    pub filters: ViewFilterPassPlan,
    pub backdrop_filters: ViewFilterPassPlan,
    pub masks: ViewMaskChainPlan,
    pub clip_path: Result<ViewClipGeometryPlan, ViewClipPathPlanError>,
    pub blend: Option<ViewBlendPassPlan>,
    pub requires_offscreen: bool,
}

/// Frame counters exported by the executor.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewCompositorStats {
    pub direct_ranges: u32,
    pub offscreen_targets: u32,
    pub shader_passes: u32,
    pub backdrop_copies: u32,
    pub pool_reuses: u32,
    pub clip_passes: u32,
    pub box_shadow_passes: u32,
}

/// The target given to direct primitive renderers.
#[derive(Clone, Copy)]
pub struct ViewCompositorTarget<'a> {
    pub texture: &'a wgpu::Texture,
    pub view: &'a wgpu::TextureView,
    pub extent: ViewTextureExtent,
    pub origin_logical: [f32; 2],
    /// Logical coordinate span represented by the target texture.
    ///
    /// Root/runtime targets usually map a physical texture back to the design
    /// viewport. Offscreen group targets use their texture extent because they
    /// are rendered as target-local logical pixels with any bucketed slack left
    /// unused.
    pub logical_extent: [f32; 2],
}

/// All host-owned state needed to render one compositor frame.
pub struct ViewCompositorFrame<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub final_target: &'a wgpu::TextureView,
    pub scene: &'a ViewScene,
    pub target_extent: ViewTextureExtent,
    pub direct_renderer: &'a mut dyn ViewDirectPrimitiveRenderer,
    pub mask_textures: &'a mut dyn ViewMaskTextureProvider,
}

/// Inline backdrop filter request for prepared runtime controls.
pub(crate) struct ViewInlineBackdropFilterFrame<'a> {
    pub device: &'a wgpu::Device,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub source: ViewCompositorTarget<'a>,
    pub target: ViewCompositorTarget<'a>,
    pub bounds: HitRect,
    pub filters: &'a ViewFilterList,
    pub device_pixel_ratio: f32,
    pub logical_extent: [f32; 2],
}

/// Inline foreground filter request for prepared runtime controls.
pub(crate) struct ViewInlineForegroundFilterFrame<'a> {
    pub device: &'a wgpu::Device,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub source: ViewCompositorTarget<'a>,
    pub output: ViewCompositorTarget<'a>,
    pub bounds: HitRect,
    pub filters: &'a ViewFilterList,
    pub device_pixel_ratio: f32,
    pub logical_extent: [f32; 2],
}

/// Inline box-shadow request for prepared runtime controls.
pub(crate) struct ViewInlineBoxShadowFrame<'a> {
    pub device: &'a wgpu::Device,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub target: ViewCompositorTarget<'a>,
    pub plan: &'a ViewBoxShadowPassPlan,
    pub kind: ViewBoxShadowKind,
}

/// Draws a seq06.9a direct primitive range into the current target.
pub trait ViewDirectPrimitiveRenderer {
    fn render_direct_range(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &ViewScene,
        context: &ViewSceneContext,
        target: ViewCompositorTarget<'_>,
    ) -> Result<(), ViewCompositorError>;
}

/// Supplies mask textures owned by the host or renderer resource table.
pub trait ViewMaskTextureProvider {
    fn texture_for<'a>(&'a mut self, image: &ViewMaskImage) -> Option<ViewMaskTextureView<'a>>;
}

pub struct ViewMaskTextureView<'a> {
    pub view: &'a wgpu::TextureView,
    pub channel: ViewMaskChannel,
    pub extent: ViewTextureExtent,
}

/// No-op provider for scenes that do not reference external masks.
#[derive(Default)]
pub struct ViewNoMaskTextures;

/// wgpu compositor executor.
pub struct ViewCompositor {
    format: wgpu::TextureFormat,
    max_extent: ViewTextureExtent,
    pool: ViewRenderTargetPool,
    pipelines: ViewCompositorPipelines,
    defaults: ViewDefaultTextures,
}

#[derive(Debug, Error)]
pub enum ViewCompositorError {
    #[error("compositor texture extent {requested:?} exceeds configured maximum {maximum:?}")]
    ExtentTooLarge {
        requested: ViewTextureExtent,
        maximum: ViewTextureExtent,
    },
    #[error("mask texture is required for {0:?} but no provider returned one")]
    MissingMaskTexture(ViewMaskImage),
    #[error("blend mode {0:?} has no seq06.9b shader pass")]
    UnsupportedBlendMode(ViewBlendMode),
    #[error("clip-path plan failed: {0}")]
    ClipPath(#[from] ViewClipPathPlanError),
    #[error("mask plan failed: {0}")]
    MaskPlan(#[from] ViewMaskPlanError),
    #[error("box-shadow plan failed: {0}")]
    BoxShadowPlan(#[from] ViewBoxShadowPlanError),
    #[error("unsupported filter `{name}`: {reason}")]
    UnsupportedFilter { name: Box<str>, reason: Box<str> },
    #[error("view scene primitive range {start}..{end} is not present")]
    InvalidPrimitiveRange { start: u32, end: u32 },
    #[error("unsupported view primitive `{primitive}`: {reason}")]
    UnsupportedPrimitive {
        primitive: &'static str,
        reason: Box<str>,
    },
    #[error("missing view image resource for resource index {resource_index}")]
    MissingImageResource { resource_index: u32 },
    #[error("view glyph run {run_index} has no explicit PreparedFrame text handoff")]
    UnhandledGlyphRun { run_index: u32 },
    #[error("unsupported view clip: {reason}")]
    UnsupportedClip { reason: Box<str> },
}

#[derive(Default)]
struct ViewRenderTargetPool {
    available: Vec<ViewOffscreenTarget>,
    reused_this_frame: u32,
}

struct ViewOffscreenTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    extent: ViewTextureExtent,
    format: wgpu::TextureFormat,
}

struct ViewCompositorPipelines {
    bind_group_layout: wgpu::BindGroupLayout,
    replace_pipeline: wgpu::RenderPipeline,
    over_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

struct ViewDefaultTextures {
    white: ViewStaticTexture,
    transparent: ViewStaticTexture,
}

struct ViewStaticTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct ViewCompositorRenderState<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    encoder: &'a mut wgpu::CommandEncoder,
    scene: &'a ViewScene,
    direct_renderer: &'a mut dyn ViewDirectPrimitiveRenderer,
    mask_textures: &'a mut dyn ViewMaskTextureProvider,
    stats: ViewCompositorStats,
}

impl ViewCompositorPlan {
    pub fn from_scene(scene: &ViewScene, device_pixel_ratio: f32) -> Self {
        let root_extent = ViewTextureExtent::from_viewport(
            scene.viewport_width(),
            scene.viewport_height(),
            device_pixel_ratio,
        );
        let nodes = scene
            .paint_nodes()
            .iter()
            .map(|node| plan_node(node, device_pixel_ratio))
            .collect::<Vec<_>>();
        let mut plan = Self {
            root_extent,
            nodes,
            offscreen_target_count: 1,
            shader_pass_count: 1,
            backdrop_copy_count: 0,
        };
        plan.recount();
        plan
    }

    pub const fn root_extent(&self) -> ViewTextureExtent {
        self.root_extent
    }

    pub fn nodes(&self) -> &[ViewCompositorNodePlan] {
        &self.nodes
    }

    pub const fn offscreen_target_count(&self) -> usize {
        self.offscreen_target_count
    }

    pub const fn shader_pass_count(&self) -> usize {
        self.shader_pass_count
    }

    pub const fn backdrop_copy_count(&self) -> usize {
        self.backdrop_copy_count
    }

    fn recount(&mut self) {
        self.offscreen_target_count = 1;
        self.shader_pass_count = 1;
        self.backdrop_copy_count = 0;
        for node in &self.nodes {
            let counters = count_node(node);
            self.offscreen_target_count += counters.offscreen_targets;
            self.shader_pass_count += counters.shader_passes;
            self.backdrop_copy_count += counters.backdrop_copies;
        }
    }
}

impl SharedRenderer {
    /// Creates a compositor that uses the renderer's target format.
    pub fn create_view_compositor(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> ViewCompositor {
        ViewCompositor::new(device, queue, self.format())
    }
}

impl ViewCompositor {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            max_extent: ViewTextureExtent::MAX,
            pool: ViewRenderTargetPool::default(),
            pipelines: ViewCompositorPipelines::new(device, format),
            defaults: ViewDefaultTextures::new(device, queue),
        }
    }

    pub const fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub const fn max_extent(&self) -> ViewTextureExtent {
        self.max_extent
    }

    pub fn set_max_extent(&mut self, max_extent: ViewTextureExtent) {
        self.max_extent = max_extent.clamped(ViewTextureExtent::MAX);
    }

    pub fn render_scene(
        &mut self,
        frame: &mut ViewCompositorFrame<'_>,
    ) -> Result<ViewCompositorStats, ViewCompositorError> {
        self.pool.reused_this_frame = 0;
        let final_target = frame.final_target;
        let root_extent = frame.target_extent.clamped(self.max_extent);
        let root = self
            .pool
            .acquire(frame.device, self.format, root_extent, "arcweft-view-root");
        let mut state = ViewCompositorRenderState {
            device: frame.device,
            queue: frame.queue,
            encoder: &mut *frame.encoder,
            scene: frame.scene,
            direct_renderer: &mut *frame.direct_renderer,
            mask_textures: &mut *frame.mask_textures,
            stats: ViewCompositorStats::default(),
        };
        state.stats.offscreen_targets = state.stats.offscreen_targets.saturating_add(1);
        clear_target(state.encoder, &root.view);

        for node in state.scene.paint_nodes() {
            let root_target = root.as_target(
                [0.0, 0.0],
                [state.scene.viewport_width(), state.scene.viewport_height()],
            );
            self.render_node(&mut state, node, root_target)?;
        }

        self.run_shader_pass(
            state.device,
            state.encoder,
            &ShaderPassInputs {
                source: &root.view,
                backdrop: None,
                mask: None,
                output: final_target,
                uniform: ViewCompositorUniform::composite(1.0, ViewBlendShaderMode::Normal),
                load: wgpu::LoadOp::Load,
                blend_over_existing: true,
            },
        );
        state.stats.shader_passes = state.stats.shader_passes.saturating_add(1);
        state.stats.pool_reuses = self.pool.reused_this_frame;
        let frame_result = state.stats;
        self.pool.release(root);
        Ok(frame_result)
    }

    pub(crate) fn render_inline_backdrop_filter(
        &mut self,
        frame: &mut ViewInlineBackdropFilterFrame<'_>,
    ) -> Result<ViewCompositorStats, ViewCompositorError> {
        if frame.filters.is_empty() || frame.bounds.width <= 0.0 || frame.bounds.height <= 0.0 {
            return Ok(ViewCompositorStats::default());
        }
        let pool_reuses_at_start = self.pool.reused_this_frame;
        let mut stats = ViewCompositorStats::default();
        let mut backdrop = self.pool.acquire(
            frame.device,
            self.format,
            frame.target.extent,
            "arcweft-runtime-control-backdrop-copy",
        );
        stats.offscreen_targets = stats.offscreen_targets.saturating_add(1);
        frame.encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: frame.source.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &backdrop.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            extent3d(frame.target.extent),
        );
        stats.backdrop_copies = stats.backdrop_copies.saturating_add(1);
        backdrop = self.apply_filter_plan_to_target(
            frame.device,
            frame.encoder,
            &mut stats,
            backdrop,
            &ViewFilterPassPlan::from_filter_list_fixed_extent(
                frame.filters,
                frame.target.extent,
                frame.device_pixel_ratio,
            ),
        )?;
        self.run_shader_pass(
            frame.device,
            frame.encoder,
            &ShaderPassInputs {
                source: &backdrop.view,
                backdrop: None,
                mask: None,
                output: frame.target.view,
                uniform: ViewCompositorUniform::clipped_composite(
                    1.0,
                    ViewBlendShaderMode::Normal,
                    [
                        frame.bounds.x,
                        frame.bounds.y,
                        frame.bounds.width,
                        frame.bounds.height,
                    ],
                    frame.logical_extent,
                ),
                load: wgpu::LoadOp::Load,
                blend_over_existing: true,
            },
        );
        stats.shader_passes = stats.shader_passes.saturating_add(1);
        stats.pool_reuses = self
            .pool
            .reused_this_frame
            .saturating_sub(pool_reuses_at_start);
        self.pool.release(backdrop);
        Ok(stats)
    }

    pub(crate) fn render_inline_foreground_filter(
        &mut self,
        frame: &mut ViewInlineForegroundFilterFrame<'_>,
    ) -> Result<ViewCompositorStats, ViewCompositorError> {
        if frame.filters.is_empty() || frame.bounds.width <= 0.0 || frame.bounds.height <= 0.0 {
            return Ok(ViewCompositorStats::default());
        }
        let pool_reuses_at_start = self.pool.reused_this_frame;
        let mut stats = ViewCompositorStats::default();
        let mut foreground = self.pool.acquire(
            frame.device,
            self.format,
            frame.source.extent,
            "arcweft-runtime-control-foreground-copy",
        );
        stats.offscreen_targets = stats.offscreen_targets.saturating_add(1);
        frame.encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: frame.source.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &foreground.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            extent3d(frame.source.extent),
        );
        foreground = self.apply_filter_plan_to_target(
            frame.device,
            frame.encoder,
            &mut stats,
            foreground,
            &ViewFilterPassPlan::from_filter_list_fixed_extent(
                frame.filters,
                frame.source.extent,
                frame.device_pixel_ratio,
            ),
        )?;
        self.run_shader_pass(
            frame.device,
            frame.encoder,
            &ShaderPassInputs {
                source: &foreground.view,
                backdrop: None,
                mask: None,
                output: frame.output.view,
                uniform: ViewCompositorUniform::clipped_composite(
                    1.0,
                    ViewBlendShaderMode::Normal,
                    [
                        frame.bounds.x,
                        frame.bounds.y,
                        frame.bounds.width,
                        frame.bounds.height,
                    ],
                    frame.logical_extent,
                ),
                load: wgpu::LoadOp::Load,
                blend_over_existing: true,
            },
        );
        stats.shader_passes = stats.shader_passes.saturating_add(1);
        stats.pool_reuses = self
            .pool
            .reused_this_frame
            .saturating_sub(pool_reuses_at_start);
        self.pool.release(foreground);
        Ok(stats)
    }

    pub(crate) fn render_inline_box_shadow(
        &mut self,
        frame: &mut ViewInlineBoxShadowFrame<'_>,
    ) -> ViewCompositorStats {
        if frame.plan.is_empty() {
            return ViewCompositorStats::default();
        }
        let mut stats = ViewCompositorStats::default();
        for pass in frame.plan.passes_for_kind(frame.kind) {
            self.run_shader_pass(
                frame.device,
                frame.encoder,
                &ShaderPassInputs {
                    source: &self.defaults.transparent.view,
                    backdrop: None,
                    mask: None,
                    output: frame.target.view,
                    uniform: ViewCompositorUniform::box_shadow(
                        pass,
                        frame.target.origin_logical,
                        frame.target.logical_extent,
                    ),
                    load: wgpu::LoadOp::Load,
                    blend_over_existing: true,
                },
            );
            stats.shader_passes = stats.shader_passes.saturating_add(1);
            stats.box_shadow_passes = stats.box_shadow_passes.saturating_add(1);
        }
        stats
    }

    pub(crate) fn composite_texture_to_view(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        output: &wgpu::TextureView,
    ) {
        self.run_shader_pass(
            device,
            encoder,
            &ShaderPassInputs {
                source,
                backdrop: None,
                mask: None,
                output,
                uniform: ViewCompositorUniform::composite_to_final_target(
                    1.0,
                    ViewBlendShaderMode::Normal,
                    self.format,
                ),
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                blend_over_existing: false,
            },
        );
    }

    fn render_node(
        &mut self,
        state: &mut ViewCompositorRenderState<'_>,
        node: &ViewPaintNode,
        target: ViewCompositorTarget<'_>,
    ) -> Result<(), ViewCompositorError> {
        match node {
            ViewPaintNode::Direct(context) => {
                state.stats.direct_ranges = state.stats.direct_ranges.saturating_add(1);
                state.direct_renderer.render_direct_range(
                    state.device,
                    state.queue,
                    state.encoder,
                    state.scene,
                    context,
                    target,
                )
            }
            ViewPaintNode::Group(group) => self.render_group(state, group, target),
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Group rendering coordinates ordered compositor passes; splitting would obscure pass order."
    )]
    fn render_group(
        &mut self,
        state: &mut ViewCompositorRenderState<'_>,
        group: &ViewCompositingGroup,
        parent_target: ViewCompositorTarget<'_>,
    ) -> Result<(), ViewCompositorError> {
        let visual_bounds = group.visual_bounds();
        let group_extent = ViewTextureExtent::from_logical_bounds(visual_bounds, 1.0, 0.0)
            .bucketed(self.max_extent);
        let mut group_target = self.pool.acquire(
            state.device,
            self.format,
            group_extent,
            "arcweft-view-compositing-group",
        );
        state.stats.offscreen_targets = state.stats.offscreen_targets.saturating_add(1);
        clear_target(state.encoder, &group_target.view);

        let box_shadow_plan =
            ViewBoxShadowPassPlan::from_shadows(&group.effects.box_shadows, group.bounds)?;
        self.render_box_shadows(
            state,
            &group_target,
            group,
            &box_shadow_plan,
            ViewBoxShadowKind::Outer,
        );

        for child in &group.children {
            self.render_node(
                state,
                child,
                group_target.as_target(
                    [visual_bounds.x, visual_bounds.y],
                    logical_extent_from_texture(group_target.extent),
                ),
            )?;
        }

        self.render_box_shadows(
            state,
            &group_target,
            group,
            &box_shadow_plan,
            ViewBoxShadowKind::Inset,
        );

        group_target = self.apply_filter_plan(
            state,
            group_target,
            &ViewFilterPassPlan::from_filter_list(&group.effects.filters, group_extent, 1.0),
        )?;
        group_target = self.apply_clip_plan(state, group_target, group)?;
        group_target = self.apply_mask_plan(state, group_target, group)?;

        let mut backdrop_target = None;
        if !group.effects.backdrop_filters.is_empty() {
            let mut backdrop = self.pool.acquire(
                state.device,
                self.format,
                parent_target.extent,
                "arcweft-view-backdrop-copy",
            );
            state.stats.offscreen_targets = state.stats.offscreen_targets.saturating_add(1);
            state.encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: parent_target.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &backdrop.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                extent3d(parent_target.extent),
            );
            state.stats.backdrop_copies = state.stats.backdrop_copies.saturating_add(1);
            backdrop = self.apply_filter_plan(
                state,
                backdrop,
                &ViewFilterPassPlan::from_filter_list(
                    &group.effects.backdrop_filters,
                    parent_target.extent,
                    1.0,
                ),
            )?;
            backdrop_target = Some(backdrop);
        }

        let blend = ViewBlendPassPlan::from_mode(group.effects.blend_mode).ok_or(
            ViewCompositorError::UnsupportedBlendMode(group.effects.blend_mode),
        )?;
        self.run_shader_pass(
            state.device,
            state.encoder,
            &ShaderPassInputs {
                source: &group_target.view,
                backdrop: backdrop_target.as_ref().map(|target| &target.view),
                mask: None,
                output: parent_target.view,
                uniform: ViewCompositorUniform::composite(group.effects.opacity, blend.shader_mode),
                load: wgpu::LoadOp::Load,
                blend_over_existing: !blend.samples_backdrop,
            },
        );
        state.stats.shader_passes = state.stats.shader_passes.saturating_add(1);

        self.pool.release(group_target);
        if let Some(backdrop) = backdrop_target {
            self.pool.release(backdrop);
        }
        Ok(())
    }

    fn render_box_shadows(
        &mut self,
        state: &mut ViewCompositorRenderState<'_>,
        target: &ViewOffscreenTarget,
        group: &ViewCompositingGroup,
        plan: &ViewBoxShadowPassPlan,
        kind: ViewBoxShadowKind,
    ) {
        let visual_bounds = group.visual_bounds();
        let target = target.as_target(
            [visual_bounds.x, visual_bounds.y],
            logical_extent_from_texture(target.extent),
        );
        for pass in plan.passes_for_kind(kind) {
            self.run_shader_pass(
                state.device,
                state.encoder,
                &ShaderPassInputs {
                    source: &self.defaults.transparent.view,
                    backdrop: None,
                    mask: None,
                    output: target.view,
                    uniform: ViewCompositorUniform::box_shadow(
                        pass,
                        target.origin_logical,
                        target.logical_extent,
                    ),
                    load: wgpu::LoadOp::Load,
                    blend_over_existing: true,
                },
            );
            state.stats.shader_passes = state.stats.shader_passes.saturating_add(1);
            state.stats.box_shadow_passes = state.stats.box_shadow_passes.saturating_add(1);
        }
    }

    fn apply_filter_plan(
        &mut self,
        state: &mut ViewCompositorRenderState<'_>,
        source: ViewOffscreenTarget,
        plan: &ViewFilterPassPlan,
    ) -> Result<ViewOffscreenTarget, ViewCompositorError> {
        self.apply_filter_plan_to_target(
            state.device,
            state.encoder,
            &mut state.stats,
            source,
            plan,
        )
    }

    fn apply_filter_plan_to_target(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        stats: &mut ViewCompositorStats,
        mut source: ViewOffscreenTarget,
        plan: &ViewFilterPassPlan,
    ) -> Result<ViewOffscreenTarget, ViewCompositorError> {
        for pass in plan.passes() {
            let output_extent = match pass {
                ViewEffectPass::ColorMatrix(_) => source.extent,
                ViewEffectPass::Blur(plan) => plan.output_extent,
                ViewEffectPass::DropShadow(plan) => plan.shadow_extent,
                ViewEffectPass::Unsupported { name, reason } => {
                    return Err(ViewCompositorError::UnsupportedFilter {
                        name: name.clone(),
                        reason: reason.clone(),
                    });
                }
            };
            let output = self.pool.acquire(
                device,
                self.format,
                output_extent.bucketed(self.max_extent),
                "arcweft-view-effect-pass",
            );
            stats.offscreen_targets = stats.offscreen_targets.saturating_add(1);
            let uniform = match pass {
                ViewEffectPass::ColorMatrix(matrix) => ViewCompositorUniform::color_matrix(*matrix),
                ViewEffectPass::Blur(plan) => {
                    ViewCompositorUniform::blur(plan.direction, plan.radius_px, source.extent)
                }
                ViewEffectPass::DropShadow(plan) => ViewCompositorUniform::drop_shadow(
                    plan.offset_x_px,
                    plan.offset_y_px,
                    plan.blur_radius_px,
                    plan.tint,
                    source.extent,
                ),
                ViewEffectPass::Unsupported { .. } => unreachable!(),
            };
            self.run_shader_pass(
                device,
                encoder,
                &ShaderPassInputs {
                    source: &source.view,
                    backdrop: None,
                    mask: None,
                    output: &output.view,
                    uniform,
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    blend_over_existing: false,
                },
            );
            stats.shader_passes = stats.shader_passes.saturating_add(1);
            self.pool.release(source);
            source = output;
        }
        Ok(source)
    }

    fn apply_clip_plan(
        &mut self,
        state: &mut ViewCompositorRenderState<'_>,
        source: ViewOffscreenTarget,
        group: &ViewCompositingGroup,
    ) -> Result<ViewOffscreenTarget, ViewCompositorError> {
        let plan =
            ViewClipGeometryPlan::from_clip_path(group.effects.clip_path.as_deref(), group.bounds)?;
        if !plan.requires_geometry_pass() {
            return Ok(source);
        }
        let output = self.pool.acquire(
            state.device,
            self.format,
            source.extent,
            "arcweft-view-clip-pass",
        );
        let visual_bounds = group.visual_bounds();
        self.run_shader_pass(
            state.device,
            state.encoder,
            &ShaderPassInputs {
                source: &source.view,
                backdrop: None,
                mask: None,
                output: &output.view,
                uniform: ViewCompositorUniform::clip(
                    &plan,
                    logical_extent_from_texture(source.extent),
                    [visual_bounds.x, visual_bounds.y],
                ),
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                blend_over_existing: false,
            },
        );
        state.stats.shader_passes = state.stats.shader_passes.saturating_add(1);
        state.stats.clip_passes = state.stats.clip_passes.saturating_add(1);
        state.stats.offscreen_targets = state.stats.offscreen_targets.saturating_add(1);
        self.pool.release(source);
        Ok(output)
    }

    fn apply_mask_plan(
        &mut self,
        state: &mut ViewCompositorRenderState<'_>,
        mut source: ViewOffscreenTarget,
        group: &ViewCompositingGroup,
    ) -> Result<ViewOffscreenTarget, ViewCompositorError> {
        let mask_plan = ViewMaskChainPlan::from_masks(&group.effects.masks, ViewMaskChannel::Alpha);
        for pass in mask_plan.passes() {
            let output = self.pool.acquire(
                state.device,
                self.format,
                source.extent,
                "arcweft-view-mask-pass",
            );
            let (mask_view, mask_channel, mask_extent) = match &pass.image {
                ViewMaskImagePlan::None => (
                    Some(&self.defaults.white.view),
                    pass.channel,
                    ViewTextureExtent::new(1, 1),
                ),
                ViewMaskImagePlan::Texture { .. } => {
                    let image = &group.effects.masks[pass.mask_index].image;
                    let mask = state
                        .mask_textures
                        .texture_for(image)
                        .ok_or_else(|| ViewCompositorError::MissingMaskTexture(image.clone()))?;
                    (Some(mask.view), mask.channel, mask.extent)
                }
                ViewMaskImagePlan::Gradient(_) => (None, pass.channel, source.extent),
                ViewMaskImagePlan::Element(_) | ViewMaskImagePlan::Unsupported(_) => {
                    pass.sampling_plan(source.extent, ViewTextureExtent::new(1, 1))?;
                    unreachable!("unsupported mask image must return before sampling")
                }
            };
            let sampling = pass.sampling_plan(source.extent, mask_extent)?;
            let gradient = pass.gradient_plan(sampling.tile_size_px)?;
            let uniform = if let Some(gradient) = gradient.as_ref() {
                ViewCompositorUniform::gradient_mask(
                    mask_channel,
                    sampling,
                    gradient,
                    source.extent,
                )
            } else {
                ViewCompositorUniform::mask(mask_channel, sampling, source.extent)
            };
            self.run_shader_pass(
                state.device,
                state.encoder,
                &ShaderPassInputs {
                    source: &source.view,
                    backdrop: None,
                    mask: mask_view,
                    output: &output.view,
                    uniform,
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    blend_over_existing: false,
                },
            );
            state.stats.shader_passes = state.stats.shader_passes.saturating_add(1);
            state.stats.offscreen_targets = state.stats.offscreen_targets.saturating_add(1);
            self.pool.release(source);
            source = output;
        }
        Ok(source)
    }

    fn run_shader_pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        inputs: &ShaderPassInputs<'_>,
    ) {
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("arcweft-view-compositor-uniform"),
            contents: bytemuck::bytes_of(&inputs.uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-view-compositor-bind-group"),
            layout: &self.pipelines.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        inputs.backdrop.unwrap_or(&self.defaults.transparent.view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        inputs.mask.unwrap_or(&self.defaults.white.view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.pipelines.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let pipeline = if inputs.blend_over_existing {
            &self.pipelines.over_pipeline
        } else {
            &self.pipelines.replace_pipeline
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("arcweft-view-compositor-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: inputs.output,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: inputs.load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl ViewMaskTextureProvider for ViewNoMaskTextures {
    fn texture_for<'a>(&'a mut self, _image: &ViewMaskImage) -> Option<ViewMaskTextureView<'a>> {
        None
    }
}

struct ShaderPassInputs<'a> {
    source: &'a wgpu::TextureView,
    backdrop: Option<&'a wgpu::TextureView>,
    mask: Option<&'a wgpu::TextureView>,
    output: &'a wgpu::TextureView,
    uniform: ViewCompositorUniform,
    load: wgpu::LoadOp<wgpu::Color>,
    blend_over_existing: bool,
}

impl ViewRenderTargetPool {
    fn acquire(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        extent: ViewTextureExtent,
        label: &'static str,
    ) -> ViewOffscreenTarget {
        if let Some(index) = self
            .available
            .iter()
            .position(|target| target.extent == extent && target.format == format)
        {
            self.reused_this_frame = self.reused_this_frame.saturating_add(1);
            return self.available.swap_remove(index);
        }
        ViewOffscreenTarget::new(device, format, extent, label)
    }

    fn release(&mut self, target: ViewOffscreenTarget) {
        self.available.push(target);
    }
}

impl ViewOffscreenTarget {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        extent: ViewTextureExtent,
        label: &'static str,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: extent3d(extent),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            extent,
            format,
        }
    }

    fn as_target(
        &self,
        origin_logical: [f32; 2],
        logical_extent: [f32; 2],
    ) -> ViewCompositorTarget<'_> {
        ViewCompositorTarget {
            texture: &self.texture,
            view: &self.view,
            extent: self.extent,
            origin_logical,
            logical_extent,
        }
    }
}

impl ViewCompositorPipelines {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("arcweft-view-compositor-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("view_shaders/compositor.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("arcweft-view-compositor-bind-group-layout"),
            entries: &[
                texture_binding(0),
                texture_binding(1),
                texture_binding(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let replace_pipeline = compositor_pipeline(
            device,
            format,
            &shader,
            &bind_group_layout,
            None,
            "arcweft-view-compositor-replace-pipeline",
        );
        let over_pipeline = compositor_pipeline(
            device,
            format,
            &shader,
            &bind_group_layout,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            "arcweft-view-compositor-over-pipeline",
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("arcweft-view-compositor-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            bind_group_layout,
            replace_pipeline,
            over_pipeline,
            sampler,
        }
    }
}

impl ViewDefaultTextures {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            white: ViewStaticTexture::new(
                device,
                queue,
                [255, 255, 255, 255],
                "arcweft-view-white-mask",
            ),
            transparent: ViewStaticTexture::new(
                device,
                queue,
                [0, 0, 0, 0],
                "arcweft-view-transparent-backdrop",
            ),
        }
    }
}

impl ViewStaticTexture {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4], label: &'static str) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
        }
    }
}

fn plan_node(node: &ViewPaintNode, device_pixel_ratio: f32) -> ViewCompositorNodePlan {
    match node {
        ViewPaintNode::Direct(context) => ViewCompositorNodePlan::Direct {
            primitive_range: context.primitive_range,
        },
        ViewPaintNode::Group(group) => {
            let visual_extent = ViewTextureExtent::from_logical_bounds(
                group.visual_bounds(),
                device_pixel_ratio,
                0.0,
            );
            let filters = ViewFilterPassPlan::from_filter_list(
                &group.effects.filters,
                visual_extent,
                device_pixel_ratio,
            );
            let backdrop_filters = ViewFilterPassPlan::from_filter_list(
                &group.effects.backdrop_filters,
                visual_extent,
                device_pixel_ratio,
            );
            let masks = ViewMaskChainPlan::from_masks(&group.effects.masks, ViewMaskChannel::Alpha);
            let box_shadows =
                ViewBoxShadowPassPlan::from_shadows(&group.effects.box_shadows, group.bounds);
            let clip_path = ViewClipGeometryPlan::from_clip_path(
                group.effects.clip_path.as_deref(),
                group.bounds,
            );
            let blend = ViewBlendPassPlan::from_mode(group.effects.blend_mode);
            ViewCompositorNodePlan::Group {
                visual_extent,
                effects: Box::new(ViewGroupEffectPlan {
                    box_shadows,
                    filters,
                    backdrop_filters,
                    masks,
                    clip_path,
                    blend,
                    requires_offscreen: group.requires_offscreen_surface(),
                }),
                children: group
                    .children
                    .iter()
                    .map(|child| plan_node(child, device_pixel_ratio))
                    .collect(),
            }
        }
    }
}

#[derive(Default)]
struct PlanCounters {
    offscreen_targets: usize,
    shader_passes: usize,
    backdrop_copies: usize,
}

fn count_node(node: &ViewCompositorNodePlan) -> PlanCounters {
    match node {
        ViewCompositorNodePlan::Direct { .. } => PlanCounters::default(),
        ViewCompositorNodePlan::Group {
            effects, children, ..
        } => {
            let mut counters = PlanCounters {
                offscreen_targets: usize::from(effects.requires_offscreen),
                shader_passes: effects.filters.passes().len()
                    + effects.backdrop_filters.passes().len()
                    + effects
                        .box_shadows
                        .as_ref()
                        .map_or(0, |plan| plan.passes().len())
                    + effects.masks.passes().len()
                    + usize::from(
                        effects
                            .clip_path
                            .as_ref()
                            .is_ok_and(ViewClipGeometryPlan::requires_geometry_pass),
                    )
                    + usize::from(effects.blend.is_some()),
                backdrop_copies: usize::from(!effects.backdrop_filters.is_empty()),
            };
            for child in children {
                let child_counts = count_node(child);
                counters.offscreen_targets += child_counts.offscreen_targets;
                counters.shader_passes += child_counts.shader_passes;
                counters.backdrop_copies += child_counts.backdrop_copies;
            }
            counters
        }
    }
}

fn texture_binding(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn compositor_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader: &wgpu::ShaderModule,
    bind_group_layout: &wgpu::BindGroupLayout,
    blend: Option<wgpu::BlendState>,
    label: &'static str,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(bind_group_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
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

fn clear_target(encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("arcweft-view-compositor-clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
}

fn extent3d(extent: ViewTextureExtent) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: extent.width,
        height: extent.height,
        depth_or_array_layers: 1,
    }
}

fn logical_extent_from_texture(extent: ViewTextureExtent) -> [f32; 2] {
    [
        extent.width.max(1).to_f32().unwrap_or(f32::MAX),
        extent.height.max(1).to_f32().unwrap_or(f32::MAX),
    ]
}
