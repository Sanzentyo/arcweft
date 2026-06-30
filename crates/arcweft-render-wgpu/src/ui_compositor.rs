//! wgpu compositor substrate for seq06.9a UI paint nodes.
//!
//! `UiCompositorPlan` is the deterministic, testable pass graph. `UiCompositor`
//! owns the wgpu pipelines, offscreen texture pool, and callback boundaries that
//! let the existing renderer draw direct primitive ranges inside a group target.

use crate::renderer::SharedRenderer;
use crate::ui_blend::{UiBlendPassPlan, UiBlendShaderMode};
use crate::ui_clip_path::{UiClipGeometryPlan, UiClipPathPlanError};
use crate::ui_effects::{
    UiBlurDirection, UiColorMatrix, UiEffectPass, UiFilterPassPlan, UiTextureExtent,
};
use crate::ui_mask::{UiMaskChainPlan, UiMaskChannel};
use crate::ui_scene::{
    UiBlendMode, UiCompositingGroup, UiMaskImage, UiPaintNode, UiPrimitiveRange, UiScene,
    UiSceneContext,
};
use bytemuck::{Pod, Zeroable};
use num_traits::ToPrimitive;
use thiserror::Error;
use wgpu::util::DeviceExt;

const PASS_COMPOSITE: u32 = 0;
const PASS_COLOR_MATRIX: u32 = 1;
const PASS_BLUR: u32 = 2;
const PASS_DROP_SHADOW: u32 = 3;
const PASS_MASK: u32 = 4;
const PASS_BLEND: u32 = 5;

/// Pure compositor pass graph for one scene.
#[derive(Clone, Debug, PartialEq)]
pub struct UiCompositorPlan {
    root_extent: UiTextureExtent,
    nodes: Vec<UiCompositorNodePlan>,
    offscreen_target_count: usize,
    shader_pass_count: usize,
    backdrop_copy_count: usize,
}

/// Pure per-node plan used by tests and GPU execution.
#[derive(Clone, Debug, PartialEq)]
pub enum UiCompositorNodePlan {
    Direct {
        primitive_range: UiPrimitiveRange,
    },
    Group {
        visual_extent: UiTextureExtent,
        effects: UiGroupEffectPlan,
        children: Vec<UiCompositorNodePlan>,
    },
}

/// Effect plans attached to one compositing group.
#[derive(Clone, Debug, PartialEq)]
pub struct UiGroupEffectPlan {
    pub filters: UiFilterPassPlan,
    pub backdrop_filters: UiFilterPassPlan,
    pub masks: UiMaskChainPlan,
    pub clip_path: Result<UiClipGeometryPlan, UiClipPathPlanError>,
    pub blend: Option<UiBlendPassPlan>,
    pub requires_offscreen: bool,
}

/// Frame counters exported by the executor.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiCompositorStats {
    pub direct_ranges: u32,
    pub offscreen_targets: u32,
    pub shader_passes: u32,
    pub backdrop_copies: u32,
    pub pool_reuses: u32,
}

/// The target given to direct primitive renderers.
#[derive(Clone, Copy)]
pub struct UiCompositorTarget<'a> {
    pub texture: &'a wgpu::Texture,
    pub view: &'a wgpu::TextureView,
    pub extent: UiTextureExtent,
    pub origin_logical: [f32; 2],
}

/// All host-owned state needed to render one compositor frame.
pub struct UiCompositorFrame<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub final_target: &'a wgpu::TextureView,
    pub scene: &'a UiScene,
    pub target_extent: UiTextureExtent,
    pub direct_renderer: &'a mut dyn UiDirectPrimitiveRenderer,
    pub mask_textures: &'a mut dyn UiMaskTextureProvider,
}

/// Draws a seq06.9a direct primitive range into the current target.
pub trait UiDirectPrimitiveRenderer {
    fn render_direct_range(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &UiScene,
        context: &UiSceneContext,
        target: UiCompositorTarget<'_>,
    ) -> Result<(), UiCompositorError>;
}

/// Supplies mask textures owned by the host or renderer resource table.
pub trait UiMaskTextureProvider {
    fn texture_for<'a>(&'a mut self, image: &UiMaskImage) -> Option<UiMaskTextureView<'a>>;
}

pub struct UiMaskTextureView<'a> {
    pub view: &'a wgpu::TextureView,
    pub channel: UiMaskChannel,
}

/// No-op provider for scenes that do not reference external masks.
#[derive(Default)]
pub struct UiNoMaskTextures;

/// wgpu compositor executor.
pub struct UiCompositor {
    format: wgpu::TextureFormat,
    max_extent: UiTextureExtent,
    pool: UiRenderTargetPool,
    pipelines: UiCompositorPipelines,
    defaults: UiDefaultTextures,
}

#[derive(Debug, Error)]
pub enum UiCompositorError {
    #[error("compositor texture extent {requested:?} exceeds configured maximum {maximum:?}")]
    ExtentTooLarge {
        requested: UiTextureExtent,
        maximum: UiTextureExtent,
    },
    #[error("mask texture is required for {0:?} but no provider returned one")]
    MissingMaskTexture(UiMaskImage),
    #[error("blend mode {0:?} has no seq06.9b shader pass")]
    UnsupportedBlendMode(UiBlendMode),
    #[error("clip-path plan failed: {0}")]
    ClipPath(#[from] UiClipPathPlanError),
    #[error("unsupported filter `{name}`: {reason}")]
    UnsupportedFilter { name: Box<str>, reason: Box<str> },
    #[error("ui scene primitive range {start}..{end} is not present")]
    InvalidPrimitiveRange { start: u32, end: u32 },
    #[error("unsupported ui primitive `{primitive}`: {reason}")]
    UnsupportedPrimitive {
        primitive: &'static str,
        reason: Box<str>,
    },
    #[error("missing ui image resource for resource index {resource_index}")]
    MissingImageResource { resource_index: u32 },
    #[error("ui glyph run {run_index} has no explicit PreparedFrame text handoff")]
    UnhandledGlyphRun { run_index: u32 },
    #[error("unsupported ui clip: {reason}")]
    UnsupportedClip { reason: Box<str> },
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct UiCompositorUniform {
    matrix: [[f32; 4]; 4],
    offset: [f32; 4],
    params0: [f32; 4],
    params1: [f32; 4],
    pass_kind: u32,
    _padding: [u32; 3],
}

#[derive(Default)]
struct UiRenderTargetPool {
    available: Vec<UiOffscreenTarget>,
    reused_this_frame: u32,
}

struct UiOffscreenTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    extent: UiTextureExtent,
    format: wgpu::TextureFormat,
}

struct UiCompositorPipelines {
    bind_group_layout: wgpu::BindGroupLayout,
    replace_pipeline: wgpu::RenderPipeline,
    over_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

struct UiDefaultTextures {
    white: UiStaticTexture,
    transparent: UiStaticTexture,
}

struct UiStaticTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct UiCompositorRenderState<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    encoder: &'a mut wgpu::CommandEncoder,
    scene: &'a UiScene,
    direct_renderer: &'a mut dyn UiDirectPrimitiveRenderer,
    mask_textures: &'a mut dyn UiMaskTextureProvider,
    stats: UiCompositorStats,
}

impl UiCompositorPlan {
    pub fn from_scene(scene: &UiScene, device_pixel_ratio: f32) -> Self {
        let root_extent = UiTextureExtent::from_viewport(
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

    pub const fn root_extent(&self) -> UiTextureExtent {
        self.root_extent
    }

    pub fn nodes(&self) -> &[UiCompositorNodePlan] {
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
    pub fn create_ui_compositor(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> UiCompositor {
        UiCompositor::new(device, queue, self.format())
    }
}

impl UiCompositor {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            max_extent: UiTextureExtent::MAX,
            pool: UiRenderTargetPool::default(),
            pipelines: UiCompositorPipelines::new(device, format),
            defaults: UiDefaultTextures::new(device, queue),
        }
    }

    pub const fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub const fn max_extent(&self) -> UiTextureExtent {
        self.max_extent
    }

    pub fn set_max_extent(&mut self, max_extent: UiTextureExtent) {
        self.max_extent = max_extent.clamped(UiTextureExtent::MAX);
    }

    pub fn render_scene(
        &mut self,
        frame: &mut UiCompositorFrame<'_>,
    ) -> Result<UiCompositorStats, UiCompositorError> {
        let final_target = frame.final_target;
        let root_extent = frame.target_extent.clamped(self.max_extent);
        let root = self
            .pool
            .acquire(frame.device, self.format, root_extent, "arcweft-ui-root");
        let mut state = UiCompositorRenderState {
            device: frame.device,
            queue: frame.queue,
            encoder: &mut *frame.encoder,
            scene: frame.scene,
            direct_renderer: &mut *frame.direct_renderer,
            mask_textures: &mut *frame.mask_textures,
            stats: UiCompositorStats::default(),
        };
        state.stats.offscreen_targets = state.stats.offscreen_targets.saturating_add(1);
        clear_target(state.encoder, &root.view);

        for node in state.scene.paint_nodes() {
            let root_target = root.as_target([0.0, 0.0]);
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
                uniform: UiCompositorUniform::composite(1.0, UiBlendShaderMode::Normal),
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

    fn render_node(
        &mut self,
        state: &mut UiCompositorRenderState<'_>,
        node: &UiPaintNode,
        target: UiCompositorTarget<'_>,
    ) -> Result<(), UiCompositorError> {
        match node {
            UiPaintNode::Direct(context) => {
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
            UiPaintNode::Group(group) => self.render_group(state, group, target),
        }
    }

    fn render_group(
        &mut self,
        state: &mut UiCompositorRenderState<'_>,
        group: &UiCompositingGroup,
        parent_target: UiCompositorTarget<'_>,
    ) -> Result<(), UiCompositorError> {
        let visual_bounds = group.visual_bounds();
        let group_extent =
            UiTextureExtent::from_logical_bounds(visual_bounds, 1.0, 0.0).bucketed(self.max_extent);
        let mut group_target = self.pool.acquire(
            state.device,
            self.format,
            group_extent,
            "arcweft-ui-compositing-group",
        );
        state.stats.offscreen_targets = state.stats.offscreen_targets.saturating_add(1);
        clear_target(state.encoder, &group_target.view);

        for child in &group.children {
            self.render_node(
                state,
                child,
                group_target.as_target([visual_bounds.x, visual_bounds.y]),
            )?;
        }

        group_target = self.apply_filter_plan(
            state,
            group_target,
            &UiFilterPassPlan::from_filter_list(&group.effects.filters, group_extent, 1.0),
        )?;
        group_target = self.apply_mask_plan(state, group_target, group)?;

        let mut backdrop_target = None;
        if !group.effects.backdrop_filters.is_empty() {
            let mut backdrop = self.pool.acquire(
                state.device,
                self.format,
                parent_target.extent,
                "arcweft-ui-backdrop-copy",
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
                &UiFilterPassPlan::from_filter_list(
                    &group.effects.backdrop_filters,
                    parent_target.extent,
                    1.0,
                ),
            )?;
            backdrop_target = Some(backdrop);
        }

        let blend = UiBlendPassPlan::from_mode(group.effects.blend_mode).ok_or(
            UiCompositorError::UnsupportedBlendMode(group.effects.blend_mode),
        )?;
        self.run_shader_pass(
            state.device,
            state.encoder,
            &ShaderPassInputs {
                source: &group_target.view,
                backdrop: backdrop_target.as_ref().map(|target| &target.view),
                mask: None,
                output: parent_target.view,
                uniform: UiCompositorUniform::composite(group.effects.opacity, blend.shader_mode),
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

    fn apply_filter_plan(
        &mut self,
        state: &mut UiCompositorRenderState<'_>,
        mut source: UiOffscreenTarget,
        plan: &UiFilterPassPlan,
    ) -> Result<UiOffscreenTarget, UiCompositorError> {
        for pass in plan.passes() {
            let output_extent = match pass {
                UiEffectPass::ColorMatrix(_) => source.extent,
                UiEffectPass::Blur(plan) => plan.output_extent,
                UiEffectPass::DropShadow(plan) => plan.shadow_extent,
                UiEffectPass::Unsupported { name, reason } => {
                    return Err(UiCompositorError::UnsupportedFilter {
                        name: name.clone(),
                        reason: reason.clone(),
                    });
                }
            };
            let output = self.pool.acquire(
                state.device,
                self.format,
                output_extent.bucketed(self.max_extent),
                "arcweft-ui-effect-pass",
            );
            state.stats.offscreen_targets = state.stats.offscreen_targets.saturating_add(1);
            let uniform = match pass {
                UiEffectPass::ColorMatrix(matrix) => UiCompositorUniform::color_matrix(*matrix),
                UiEffectPass::Blur(plan) => {
                    UiCompositorUniform::blur(plan.direction, plan.radius_px, source.extent)
                }
                UiEffectPass::DropShadow(plan) => UiCompositorUniform::drop_shadow(
                    plan.offset_x_px,
                    plan.offset_y_px,
                    plan.blur_radius_px,
                    plan.tint,
                    source.extent,
                ),
                UiEffectPass::Unsupported { .. } => unreachable!(),
            };
            self.run_shader_pass(
                state.device,
                state.encoder,
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
            state.stats.shader_passes = state.stats.shader_passes.saturating_add(1);
            self.pool.release(source);
            source = output;
        }
        Ok(source)
    }

    fn apply_mask_plan(
        &mut self,
        state: &mut UiCompositorRenderState<'_>,
        mut source: UiOffscreenTarget,
        group: &UiCompositingGroup,
    ) -> Result<UiOffscreenTarget, UiCompositorError> {
        let mask_plan = UiMaskChainPlan::from_masks(&group.effects.masks, UiMaskChannel::Alpha);
        for pass in mask_plan.passes() {
            let output = self.pool.acquire(
                state.device,
                self.format,
                source.extent,
                "arcweft-ui-mask-pass",
            );
            let mask = match &group.effects.masks[pass.mask_index].image {
                UiMaskImage::None => UiMaskTextureView {
                    view: &self.defaults.white.view,
                    channel: pass.channel,
                },
                image => state
                    .mask_textures
                    .texture_for(image)
                    .ok_or_else(|| UiCompositorError::MissingMaskTexture(image.clone()))?,
            };
            self.run_shader_pass(
                state.device,
                state.encoder,
                &ShaderPassInputs {
                    source: &source.view,
                    backdrop: None,
                    mask: Some(mask.view),
                    output: &output.view,
                    uniform: UiCompositorUniform::mask(mask.channel),
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
            label: Some("arcweft-ui-compositor-uniform"),
            contents: bytemuck::bytes_of(&inputs.uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-ui-compositor-bind-group"),
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
            label: Some("arcweft-ui-compositor-pass"),
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

impl UiMaskTextureProvider for UiNoMaskTextures {
    fn texture_for<'a>(&'a mut self, _image: &UiMaskImage) -> Option<UiMaskTextureView<'a>> {
        None
    }
}

impl UiCompositorUniform {
    fn composite(opacity: f32, blend: UiBlendShaderMode) -> Self {
        Self {
            params0: [opacity.clamp(0.0, 1.0), shader_mode_to_f32(blend), 0.0, 0.0],
            pass_kind: if blend == UiBlendShaderMode::Normal {
                PASS_COMPOSITE
            } else {
                PASS_BLEND
            },
            ..Self::from_matrix(UiColorMatrix::identity())
        }
    }

    fn color_matrix(matrix: UiColorMatrix) -> Self {
        Self {
            pass_kind: PASS_COLOR_MATRIX,
            ..Self::from_matrix(matrix)
        }
    }

    fn blur(direction: UiBlurDirection, radius_px: f32, extent: UiTextureExtent) -> Self {
        let (step_x, step_y) = match direction {
            UiBlurDirection::Horizontal => (1.0 / dimension_to_f32(extent.width), 0.0),
            UiBlurDirection::Vertical => (0.0, 1.0 / dimension_to_f32(extent.height)),
        };
        Self {
            params0: [step_x, step_y, radius_px.max(0.0), 0.0],
            pass_kind: PASS_BLUR,
            ..Self::from_matrix(UiColorMatrix::identity())
        }
    }

    fn drop_shadow(
        horizontal_offset_px: f32,
        vertical_offset_px: f32,
        blur_radius_px: f32,
        tint: crate::ui_scene::UiColorRgba8,
        extent: UiTextureExtent,
    ) -> Self {
        Self {
            params0: [
                horizontal_offset_px / dimension_to_f32(extent.width),
                vertical_offset_px / dimension_to_f32(extent.height),
                blur_radius_px.max(0.0),
                0.0,
            ],
            params1: [
                f32::from(tint.red) / 255.0,
                f32::from(tint.green) / 255.0,
                f32::from(tint.blue) / 255.0,
                f32::from(tint.alpha) / 255.0,
            ],
            pass_kind: PASS_DROP_SHADOW,
            ..Self::from_matrix(UiColorMatrix::identity())
        }
    }

    fn mask(channel: UiMaskChannel) -> Self {
        Self {
            params0: [
                match channel {
                    UiMaskChannel::Alpha => 0.0,
                    UiMaskChannel::Luminance => 1.0,
                },
                0.0,
                0.0,
                0.0,
            ],
            pass_kind: PASS_MASK,
            ..Self::from_matrix(UiColorMatrix::identity())
        }
    }

    fn from_matrix(matrix: UiColorMatrix) -> Self {
        Self {
            matrix: matrix.matrix,
            offset: matrix.offset,
            params0: [0.0; 4],
            params1: [0.0; 4],
            pass_kind: PASS_COMPOSITE,
            _padding: [0; 3],
        }
    }
}

struct ShaderPassInputs<'a> {
    source: &'a wgpu::TextureView,
    backdrop: Option<&'a wgpu::TextureView>,
    mask: Option<&'a wgpu::TextureView>,
    output: &'a wgpu::TextureView,
    uniform: UiCompositorUniform,
    load: wgpu::LoadOp<wgpu::Color>,
    blend_over_existing: bool,
}

impl UiRenderTargetPool {
    fn acquire(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        extent: UiTextureExtent,
        label: &'static str,
    ) -> UiOffscreenTarget {
        if let Some(index) = self
            .available
            .iter()
            .position(|target| target.extent == extent && target.format == format)
        {
            self.reused_this_frame = self.reused_this_frame.saturating_add(1);
            return self.available.swap_remove(index);
        }
        UiOffscreenTarget::new(device, format, extent, label)
    }

    fn release(&mut self, target: UiOffscreenTarget) {
        self.available.push(target);
    }
}

impl UiOffscreenTarget {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        extent: UiTextureExtent,
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

    fn as_target(&self, origin_logical: [f32; 2]) -> UiCompositorTarget<'_> {
        UiCompositorTarget {
            texture: &self.texture,
            view: &self.view,
            extent: self.extent,
            origin_logical,
        }
    }
}

impl UiCompositorPipelines {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("arcweft-ui-compositor-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui_shaders/compositor.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("arcweft-ui-compositor-bind-group-layout"),
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
            "arcweft-ui-compositor-replace-pipeline",
        );
        let over_pipeline = compositor_pipeline(
            device,
            format,
            &shader,
            &bind_group_layout,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            "arcweft-ui-compositor-over-pipeline",
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("arcweft-ui-compositor-sampler"),
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

impl UiDefaultTextures {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            white: UiStaticTexture::new(
                device,
                queue,
                [255, 255, 255, 255],
                "arcweft-ui-white-mask",
            ),
            transparent: UiStaticTexture::new(
                device,
                queue,
                [0, 0, 0, 0],
                "arcweft-ui-transparent-backdrop",
            ),
        }
    }
}

impl UiStaticTexture {
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

fn plan_node(node: &UiPaintNode, device_pixel_ratio: f32) -> UiCompositorNodePlan {
    match node {
        UiPaintNode::Direct(context) => UiCompositorNodePlan::Direct {
            primitive_range: context.primitive_range,
        },
        UiPaintNode::Group(group) => {
            let visual_extent = UiTextureExtent::from_logical_bounds(
                group.visual_bounds(),
                device_pixel_ratio,
                0.0,
            );
            let filters = UiFilterPassPlan::from_filter_list(
                &group.effects.filters,
                visual_extent,
                device_pixel_ratio,
            );
            let backdrop_filters = UiFilterPassPlan::from_filter_list(
                &group.effects.backdrop_filters,
                visual_extent,
                device_pixel_ratio,
            );
            let masks = UiMaskChainPlan::from_masks(&group.effects.masks, UiMaskChannel::Alpha);
            let clip_path = UiClipGeometryPlan::from_clip_path(
                group.effects.clip_path.as_deref(),
                group.bounds,
            );
            let blend = UiBlendPassPlan::from_mode(group.effects.blend_mode);
            UiCompositorNodePlan::Group {
                visual_extent,
                effects: UiGroupEffectPlan {
                    filters,
                    backdrop_filters,
                    masks,
                    clip_path,
                    blend,
                    requires_offscreen: group.requires_offscreen_surface(),
                },
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

fn count_node(node: &UiCompositorNodePlan) -> PlanCounters {
    match node {
        UiCompositorNodePlan::Direct { .. } => PlanCounters::default(),
        UiCompositorNodePlan::Group {
            effects, children, ..
        } => {
            let mut counters = PlanCounters {
                offscreen_targets: usize::from(effects.requires_offscreen),
                shader_passes: effects.filters.passes().len()
                    + effects.backdrop_filters.passes().len()
                    + effects.masks.passes().len()
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
        label: Some("arcweft-ui-compositor-clear"),
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

fn extent3d(extent: UiTextureExtent) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: extent.width,
        height: extent.height,
        depth_or_array_layers: 1,
    }
}

fn dimension_to_f32(value: u32) -> f32 {
    value.max(1).to_f32().unwrap_or(f32::MAX)
}

fn shader_mode_to_f32(mode: UiBlendShaderMode) -> f32 {
    mode.as_shader_u32().to_f32().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_scene::{
        UiBlendMode, UiColorRgba8, UiCompositingEffects, UiCompositingGroup, UiFilter,
        UiFilterList, UiPaintNode, UiPrimitiveRange, UiSceneContext,
    };
    use arcweft_presentation::hit::HitRect;

    fn direct_node(start: u32, end: u32) -> UiPaintNode {
        UiPaintNode::Direct(UiSceneContext {
            transform: crate::ui_scene::UiAffine2::IDENTITY,
            opacity: 1.0,
            clip: None,
            primitive_range: UiPrimitiveRange { start, end },
        })
    }

    #[test]
    fn direct_scene_plan_does_not_add_group_effect_passes() {
        let mut scene = UiScene::new(320.0, 180.0);
        scene.push_paint_node(direct_node(0, 1));

        let plan = UiCompositorPlan::from_scene(&scene, 1.0);

        assert_eq!(plan.root_extent(), UiTextureExtent::new(320, 180));
        assert_eq!(plan.offscreen_target_count(), 1);
        assert_eq!(plan.shader_pass_count(), 1);
        assert_eq!(plan.backdrop_copy_count(), 0);
    }

    #[test]
    fn blur_shadow_mask_and_blend_count_deterministic_passes() {
        let mut scene = UiScene::new(320.0, 180.0);
        let effects = UiCompositingEffects {
            filters: UiFilterList::new([
                UiFilter::Blur { radius_px: 4.0 },
                UiFilter::DropShadow {
                    offset_x_px: 2.0,
                    offset_y_px: 6.0,
                    blur_radius_px: 3.0,
                    color: UiColorRgba8 {
                        red: 0,
                        green: 0,
                        blue: 0,
                        alpha: 192,
                    },
                },
            ]),
            blend_mode: UiBlendMode::Multiply,
            ..UiCompositingEffects::default()
        };
        scene.push_paint_node(UiPaintNode::Group(
            UiCompositingGroup::new(HitRect::new(10.0, 20.0, 100.0, 50.0), effects)
                .with_children(vec![direct_node(0, 1)]),
        ));

        let plan = UiCompositorPlan::from_scene(&scene, 1.0);

        assert_eq!(plan.backdrop_copy_count(), 0);
        assert!(plan.shader_pass_count() >= 5);
        assert!(plan.offscreen_target_count() >= 2);
    }
}
