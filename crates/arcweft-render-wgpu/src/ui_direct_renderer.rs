//! Shared wgpu direct primitive renderer for `UiScene` paint ranges.
//!
//! This module is intentionally renderer-owned. It consumes Arcweft-owned
//! `UiPrimitive` values, prepared image/mask resources, and explicit text
//! handoff records. It does not parse CSS, inspect Takumi computed style, or
//! route UI through platform-specific DOM/canvas fallback paths.

use crate::geometry::{PreparedUiMaskResource, PreparedUiSceneResources, RenderImageFrame};
use crate::ui_compositor::{
    UiCompositorError, UiCompositorTarget, UiDirectPrimitiveRenderer, UiMaskTextureProvider,
    UiMaskTextureView,
};
use crate::ui_effects::UiTextureExtent;
use crate::ui_mask::UiMaskChannel;
use crate::ui_scene::{
    UiAffine2D, UiBorder, UiCaretPrimitive, UiClip, UiColorRgba8, UiCompositionUnderline,
    UiGlyphRun, UiImagePrimitive, UiLinearGradient, UiMaskImage, UiPrimitive, UiRoundedRect,
    UiScene, UiSceneContext, UiSelectionPrimitive, UiSolidRect, UiUnderlineStyle,
};
use arcweft_presentation::hit::HitRect;
use bytemuck::{Pod, Zeroable};
use num_traits::ToPrimitive;
use wgpu::util::DeviceExt;

const ROUNDED_CORNER_SEGMENTS: usize = 8;
const EPSILON: f32 = 0.0001;

/// Direct primitive renderer used by `SharedRenderer` when a prepared frame has
/// attached `UiScene`s.
pub struct WgpuUiDirectPrimitiveRenderer {
    color_pipeline: wgpu::RenderPipeline,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_pipeline: wgpu::RenderPipeline,
    image_sampler: wgpu::Sampler,
}

/// Renderer-owned mask texture provider for one prepared UI scene.
pub struct WgpuPreparedUiMaskTextureProvider {
    masks: Vec<WgpuPreparedUiMaskTexture>,
}

pub(crate) struct WgpuUiDirectPrimitiveRenderFrame<'a> {
    renderer: &'a WgpuUiDirectPrimitiveRenderer,
    resources: &'a PreparedUiSceneResources,
}

struct WgpuPreparedUiMaskTexture {
    image: UiMaskImage,
    channel: UiMaskChannel,
    extent: UiTextureExtent,
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct UiColorVertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct UiImageVertex {
    position: [f32; 2],
    uv: [f32; 2],
    opacity: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LogicalPoint {
    x: f32,
    y: f32,
}

struct DirectRenderContext<'a, 'target> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    encoder: &'a mut wgpu::CommandEncoder,
    scene: &'a UiScene,
    context: &'a UiSceneContext,
    target: UiCompositorTarget<'target>,
}

impl WgpuUiDirectPrimitiveRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let (image_bind_group_layout, image_pipeline, image_sampler) =
            image_pipeline(device, format);
        Self {
            color_pipeline: color_pipeline(device, format),
            image_bind_group_layout,
            image_pipeline,
            image_sampler,
        }
    }

    pub(crate) const fn for_resources<'a>(
        &'a self,
        resources: &'a PreparedUiSceneResources,
    ) -> WgpuUiDirectPrimitiveRenderFrame<'a> {
        WgpuUiDirectPrimitiveRenderFrame {
            renderer: self,
            resources,
        }
    }
}

impl WgpuUiDirectPrimitiveRenderFrame<'_> {
    fn color_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.renderer.color_pipeline
    }

    fn image_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.renderer.image_bind_group_layout
    }

    fn image_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.renderer.image_pipeline
    }

    fn image_sampler(&self) -> &wgpu::Sampler {
        &self.renderer.image_sampler
    }

    fn image_frame(&self, resource_index: u32) -> Option<&RenderImageFrame> {
        self.resources
            .images()
            .iter()
            .find(|resource| resource.resource_index == resource_index)
            .map(|resource| &resource.frame)
    }

    fn has_glyph_handoff(&self, run_index: u32) -> bool {
        self.resources
            .glyph_handoffs()
            .iter()
            .any(|handoff| handoff.run_index == run_index)
    }

    fn render_colored_vertices(
        &self,
        frame: &mut DirectRenderContext<'_, '_>,
        vertices: &[UiColorVertex],
    ) -> Result<(), UiCompositorError> {
        if vertices.is_empty() {
            return Ok(());
        }
        let buffer = frame
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("arcweft-ui-direct-color-vertices"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arcweft-ui-direct-color-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: frame.target.view,
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
        apply_context_scissor(&mut pass, frame.scene, frame.context, frame.target)?;
        pass.set_pipeline(self.color_pipeline());
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..u32::try_from(vertices.len()).unwrap_or(u32::MAX), 0..1);
        Ok(())
    }

    fn render_image_primitive(
        &self,
        frame: &mut DirectRenderContext<'_, '_>,
        image: &UiImagePrimitive,
    ) -> Result<(), UiCompositorError> {
        let image_frame = self.image_frame(image.resource_index).ok_or(
            UiCompositorError::MissingImageResource {
                resource_index: image.resource_index,
            },
        )?;
        if image_frame.width == 0 || image_frame.height == 0 || image_frame.rgba.is_empty() {
            return Ok(());
        }
        let texture = upload_rgba_texture(
            frame.device,
            frame.queue,
            image_frame,
            "arcweft-ui-image-resource",
        );
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = frame.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-ui-direct-image-bind-group"),
            layout: self.image_bind_group_layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(self.image_sampler()),
                },
            ],
        });
        let vertices = image_vertices(frame.scene, frame.context, frame.target, image);
        let vertex_buffer = frame
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("arcweft-ui-direct-image-vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arcweft-ui-direct-image-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: frame.target.view,
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
        apply_context_scissor(&mut pass, frame.scene, frame.context, frame.target)?;
        pass.set_pipeline(self.image_pipeline());
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..u32::try_from(vertices.len()).unwrap_or(u32::MAX), 0..1);
        Ok(())
    }
}

impl UiDirectPrimitiveRenderer for WgpuUiDirectPrimitiveRenderFrame<'_> {
    fn render_direct_range(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &UiScene,
        context: &UiSceneContext,
        target: UiCompositorTarget<'_>,
    ) -> Result<(), UiCompositorError> {
        let mut frame = DirectRenderContext {
            device,
            queue,
            encoder,
            scene,
            context,
            target,
        };
        let (start, end) = primitive_range_bounds(context)?;
        let primitives =
            scene
                .primitives()
                .get(start..end)
                .ok_or(UiCompositorError::InvalidPrimitiveRange {
                    start: context.primitive_range.start,
                    end: context.primitive_range.end,
                })?;
        let mut colored = Vec::new();
        for primitive in primitives {
            match primitive {
                UiPrimitive::SolidRect(rect) => {
                    push_solid_rect(scene, context, target, rect, &mut colored);
                }
                UiPrimitive::RoundedRect(rect) => {
                    push_rounded_rect(scene, context, target, rect, &mut colored);
                }
                UiPrimitive::Border(border) => {
                    push_border(scene, context, target, border, &mut colored);
                }
                UiPrimitive::LinearGradient(gradient) => {
                    push_linear_gradient(scene, context, target, gradient, &mut colored);
                }
                UiPrimitive::Image(image) => {
                    self.render_colored_vertices(&mut frame, &colored)?;
                    colored.clear();
                    self.render_image_primitive(&mut frame, image)?;
                }
                UiPrimitive::GlyphRun(run) => {
                    self.render_colored_vertices(&mut frame, &colored)?;
                    colored.clear();
                    ensure_glyph_handoff(self, run)?;
                }
                UiPrimitive::Selection(selection) => {
                    push_selection(scene, context, target, selection, &mut colored);
                }
                UiPrimitive::Caret(caret) => {
                    push_caret(scene, context, target, caret, &mut colored);
                }
                UiPrimitive::CompositionUnderline(underline) => {
                    push_composition_underline(scene, context, target, underline, &mut colored);
                }
            }
        }
        self.render_colored_vertices(&mut frame, &colored)
    }
}

impl WgpuPreparedUiMaskTextureProvider {
    pub fn prepare(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &PreparedUiSceneResources,
    ) -> Self {
        Self {
            masks: resources
                .masks()
                .iter()
                .map(|resource| upload_mask_resource(device, queue, resource))
                .collect(),
        }
    }
}

impl UiMaskTextureProvider for WgpuPreparedUiMaskTextureProvider {
    fn texture_for<'a>(&'a mut self, image: &UiMaskImage) -> Option<UiMaskTextureView<'a>> {
        self.masks
            .iter()
            .find(|mask| &mask.image == image)
            .map(|mask| UiMaskTextureView {
                view: &mask.view,
                channel: mask.channel,
                extent: mask.extent,
            })
    }
}

fn upload_mask_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resource: &PreparedUiMaskResource,
) -> WgpuPreparedUiMaskTexture {
    let texture = upload_rgba_texture(device, queue, &resource.frame, "arcweft-ui-mask-resource");
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    WgpuPreparedUiMaskTexture {
        image: resource.image.clone(),
        channel: resource.channel,
        extent: UiTextureExtent::new(resource.frame.width, resource.frame.height),
        _texture: texture,
        view,
    }
}

fn ensure_glyph_handoff(
    renderer: &WgpuUiDirectPrimitiveRenderFrame<'_>,
    run: &UiGlyphRun,
) -> Result<(), UiCompositorError> {
    if renderer.has_glyph_handoff(run.run_index) {
        Ok(())
    } else {
        Err(UiCompositorError::UnhandledGlyphRun {
            run_index: run.run_index,
        })
    }
}

fn primitive_range_bounds(context: &UiSceneContext) -> Result<(usize, usize), UiCompositorError> {
    let start = usize::try_from(context.primitive_range.start).map_err(|_| {
        UiCompositorError::InvalidPrimitiveRange {
            start: context.primitive_range.start,
            end: context.primitive_range.end,
        }
    })?;
    let end = usize::try_from(context.primitive_range.end).map_err(|_| {
        UiCompositorError::InvalidPrimitiveRange {
            start: context.primitive_range.start,
            end: context.primitive_range.end,
        }
    })?;
    Ok((start, end))
}

fn push_solid_rect(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    rect: &UiSolidRect,
    output: &mut Vec<UiColorVertex>,
) {
    push_rect_vertices(
        scene,
        context,
        target,
        rect.bounds,
        [color_to_f32(rect.color, context.opacity); 4],
        output,
    );
}

fn push_rounded_rect(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    rect: &UiRoundedRect,
    output: &mut Vec<UiColorVertex>,
) {
    let color = color_to_f32(rect.color, context.opacity);
    let points = rounded_rect_points(rect.bounds, rect.radius);
    push_polygon_fan(
        scene,
        context,
        target,
        rect.bounds.center(),
        &points,
        color,
        output,
    );
}

fn push_border(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    border: &UiBorder,
    output: &mut Vec<UiColorVertex>,
) {
    if border.width <= 0.0 {
        return;
    }
    let color = color_to_f32(border.color, context.opacity);
    let outer = rounded_rect_points(border.bounds, border.radius);
    let inner_bounds = inset_rect(border.bounds, border.width);
    if inner_bounds.width <= 0.0 || inner_bounds.height <= 0.0 {
        push_polygon_fan(
            scene,
            context,
            target,
            border.bounds.center(),
            &outer,
            color,
            output,
        );
        return;
    }
    let inner = rounded_rect_points(inner_bounds, (border.radius - border.width).max(0.0));
    for index in 0..outer.len() {
        let next = (index + 1) % outer.len();
        push_quad(
            scene,
            context,
            target,
            [outer[index], outer[next], inner[next], inner[index]],
            [color; 4],
            output,
        );
    }
}

fn push_linear_gradient(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    gradient: &UiLinearGradient,
    output: &mut Vec<UiColorVertex>,
) {
    let bounds = gradient.bounds;
    let corners = rect_corners(bounds);
    let colors = corners.map(|point| {
        let t = gradient_t(bounds, gradient.angle_degrees, point);
        gradient_color_at(gradient, t, context.opacity)
    });
    push_quad(scene, context, target, corners, colors, output);
}

fn push_selection(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    selection: &UiSelectionPrimitive,
    output: &mut Vec<UiColorVertex>,
) {
    push_rect_vertices(
        scene,
        context,
        target,
        selection.bounds,
        [color_to_f32(selection.color, context.opacity); 4],
        output,
    );
}

fn push_caret(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    caret: &UiCaretPrimitive,
    output: &mut Vec<UiColorVertex>,
) {
    push_rect_vertices(
        scene,
        context,
        target,
        caret.bounds,
        [color_to_f32(caret.color, context.opacity); 4],
        output,
    );
}

fn push_composition_underline(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    underline: &UiCompositionUnderline,
    output: &mut Vec<UiColorVertex>,
) {
    let thickness = underline.thickness.max(1.0);
    let y = underline.bounds.y + underline.bounds.height - thickness;
    let mut bounds = HitRect::new(underline.bounds.x, y, underline.bounds.width, thickness);
    if underline.style == UiUnderlineStyle::Dotted {
        let dot = thickness * 2.0;
        let mut x = bounds.x;
        while x < underline.bounds.x + underline.bounds.width {
            bounds.x = x;
            bounds.width = dot.min(underline.bounds.x + underline.bounds.width - x);
            push_rect_vertices(
                scene,
                context,
                target,
                bounds,
                [color_to_f32(underline.color, context.opacity); 4],
                output,
            );
            x += dot * 2.0;
        }
        return;
    }
    if underline.style == UiUnderlineStyle::Dashed {
        let dash = thickness * 4.0;
        let mut x = bounds.x;
        while x < underline.bounds.x + underline.bounds.width {
            bounds.x = x;
            bounds.width = dash.min(underline.bounds.x + underline.bounds.width - x);
            push_rect_vertices(
                scene,
                context,
                target,
                bounds,
                [color_to_f32(underline.color, context.opacity); 4],
                output,
            );
            x += dash * 1.5;
        }
        return;
    }
    push_rect_vertices(
        scene,
        context,
        target,
        bounds,
        [color_to_f32(underline.color, context.opacity); 4],
        output,
    );
}

fn push_rect_vertices(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    bounds: HitRect,
    colors: [[f32; 4]; 4],
    output: &mut Vec<UiColorVertex>,
) {
    push_quad(scene, context, target, rect_corners(bounds), colors, output);
}

fn push_quad(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    points: [LogicalPoint; 4],
    colors: [[f32; 4]; 4],
    output: &mut Vec<UiColorVertex>,
) {
    let vertices = [0, 1, 2, 0, 2, 3];
    output.extend(vertices.into_iter().map(|index| UiColorVertex {
        position: logical_to_ndc(scene, context, target, points[index]),
        color: colors[index],
    }));
}

fn push_polygon_fan(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    center: LogicalPoint,
    points: &[LogicalPoint],
    color: [f32; 4],
    output: &mut Vec<UiColorVertex>,
) {
    if points.len() < 3 {
        return;
    }
    for index in 0..points.len() {
        let next = (index + 1) % points.len();
        for point in [center, points[index], points[next]] {
            output.push(UiColorVertex {
                position: logical_to_ndc(scene, context, target, point),
                color,
            });
        }
    }
}

fn image_vertices(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    image: &UiImagePrimitive,
) -> [UiImageVertex; 6] {
    let corners = rect_corners(image.bounds);
    let positions = corners.map(|point| logical_to_ndc(scene, context, target, point));
    let opacity = [
        image.opacity.clamp(0.0, 1.0) * context.opacity.clamp(0.0, 1.0),
        0.0,
        0.0,
        0.0,
    ];
    [
        UiImageVertex {
            position: positions[0],
            uv: [0.0, 0.0],
            opacity,
        },
        UiImageVertex {
            position: positions[1],
            uv: [0.0, 1.0],
            opacity,
        },
        UiImageVertex {
            position: positions[2],
            uv: [1.0, 1.0],
            opacity,
        },
        UiImageVertex {
            position: positions[0],
            uv: [0.0, 0.0],
            opacity,
        },
        UiImageVertex {
            position: positions[2],
            uv: [1.0, 1.0],
            opacity,
        },
        UiImageVertex {
            position: positions[3],
            uv: [1.0, 0.0],
            opacity,
        },
    ]
}

fn logical_to_ndc(
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
    point: LogicalPoint,
) -> [f32; 2] {
    let transformed = apply_transform(context.transform, point);
    let scale_x = target_scale(
        target.extent.width,
        scene.viewport_width(),
        target.origin_logical[0],
    );
    let scale_y = target_scale(
        target.extent.height,
        scene.viewport_height(),
        target.origin_logical[1],
    );
    let x = (transformed.x - target.origin_logical[0]) * scale_x;
    let y = (transformed.y - target.origin_logical[1]) * scale_y;
    let target_width = u32_to_f32(target.extent.width.max(1));
    let target_height = u32_to_f32(target.extent.height.max(1));
    [
        (x / target_width) * 2.0 - 1.0,
        1.0 - (y / target_height) * 2.0,
    ]
}

fn target_scale(extent: u32, scene_dimension: f32, origin: f32) -> f32 {
    if origin.abs() <= EPSILON {
        u32_to_f32(extent.max(1)) / scene_dimension.max(EPSILON)
    } else {
        1.0
    }
}

fn u32_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}

fn usize_to_f32(value: usize) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}

fn nonnegative_floor_to_u32(value: f32) -> u32 {
    value.max(0.0).floor().to_u32().unwrap_or(u32::MAX)
}

fn nonnegative_ceil_to_u32(value: f32) -> u32 {
    value.max(0.0).ceil().to_u32().unwrap_or(u32::MAX)
}

fn apply_transform(transform: UiAffine2D, point: LogicalPoint) -> LogicalPoint {
    LogicalPoint {
        x: transform
            .m11
            .mul_add(point.x, transform.m21.mul_add(point.y, transform.tx)),
        y: transform
            .m12
            .mul_add(point.x, transform.m22.mul_add(point.y, transform.ty)),
    }
}

fn apply_context_scissor(
    pass: &mut wgpu::RenderPass<'_>,
    scene: &UiScene,
    context: &UiSceneContext,
    target: UiCompositorTarget<'_>,
) -> Result<(), UiCompositorError> {
    let Some(clip) = &context.clip else {
        return Ok(());
    };
    let bounds = match clip {
        UiClip::Rect(bounds) | UiClip::RoundedRect { bounds, .. } => *bounds,
    };
    let scale_x = target_scale(
        target.extent.width,
        scene.viewport_width(),
        target.origin_logical[0],
    );
    let scale_y = target_scale(
        target.extent.height,
        scene.viewport_height(),
        target.origin_logical[1],
    );
    let x = nonnegative_floor_to_u32((bounds.x - target.origin_logical[0]) * scale_x);
    let y = nonnegative_floor_to_u32((bounds.y - target.origin_logical[1]) * scale_y);
    let max_width = target.extent.width.saturating_sub(x);
    let max_height = target.extent.height.saturating_sub(y);
    let width = nonnegative_ceil_to_u32(bounds.width * scale_x).min(max_width);
    let height = nonnegative_ceil_to_u32(bounds.height * scale_y).min(max_height);
    if width == 0 || height == 0 {
        return Err(UiCompositorError::UnsupportedClip {
            reason: "clip resolved to an empty target scissor".into(),
        });
    }
    pass.set_scissor_rect(x, y, width, height);
    Ok(())
}

fn rect_corners(bounds: HitRect) -> [LogicalPoint; 4] {
    [
        LogicalPoint {
            x: bounds.x,
            y: bounds.y,
        },
        LogicalPoint {
            x: bounds.x,
            y: bounds.y + bounds.height,
        },
        LogicalPoint {
            x: bounds.x + bounds.width,
            y: bounds.y + bounds.height,
        },
        LogicalPoint {
            x: bounds.x + bounds.width,
            y: bounds.y,
        },
    ]
}

fn rounded_rect_points(bounds: HitRect, radius: f32) -> Vec<LogicalPoint> {
    let radius = radius.max(0.0).min(bounds.width.min(bounds.height) * 0.5);
    if radius <= EPSILON {
        return rect_corners(bounds).to_vec();
    }
    let centers = [
        (
            bounds.x + radius,
            bounds.y + radius,
            std::f32::consts::PI,
            1.5 * std::f32::consts::PI,
        ),
        (
            bounds.x + bounds.width - radius,
            bounds.y + radius,
            1.5 * std::f32::consts::PI,
            2.0 * std::f32::consts::PI,
        ),
        (
            bounds.x + bounds.width - radius,
            bounds.y + bounds.height - radius,
            0.0,
            0.5 * std::f32::consts::PI,
        ),
        (
            bounds.x + radius,
            bounds.y + bounds.height - radius,
            0.5 * std::f32::consts::PI,
            std::f32::consts::PI,
        ),
    ];
    centers
        .into_iter()
        .flat_map(|(cx, cy, start, end)| {
            (0..=ROUNDED_CORNER_SEGMENTS).map(move |step| {
                let t = usize_to_f32(step) / usize_to_f32(ROUNDED_CORNER_SEGMENTS);
                let angle = start + (end - start) * t;
                LogicalPoint {
                    x: cx + radius * angle.cos(),
                    y: cy + radius * angle.sin(),
                }
            })
        })
        .collect()
}

fn inset_rect(bounds: HitRect, inset: f32) -> HitRect {
    HitRect::new(
        bounds.x + inset,
        bounds.y + inset,
        (bounds.width - inset * 2.0).max(0.0),
        (bounds.height - inset * 2.0).max(0.0),
    )
}

fn gradient_t(bounds: HitRect, angle_degrees: f32, point: LogicalPoint) -> f32 {
    let radians = angle_degrees.to_radians();
    let direction = LogicalPoint {
        x: radians.sin(),
        y: -radians.cos(),
    };
    let center = bounds.center();
    let half = bounds.width.abs().hypot(bounds.height.abs()) * 0.5;
    if half <= EPSILON {
        return 0.0;
    }
    let projection = (point.x - center.x) * direction.x + (point.y - center.y) * direction.y;
    ((projection / (half * 2.0)) + 0.5).clamp(0.0, 1.0)
}

fn gradient_color_at(gradient: &UiLinearGradient, t: f32, opacity: f32) -> [f32; 4] {
    let Some(first) = gradient.stops.first() else {
        return [0.0, 0.0, 0.0, 0.0];
    };
    let Some(last) = gradient.stops.last() else {
        return color_to_f32(first.color, opacity);
    };
    let mut previous = first;
    for stop in &gradient.stops[1..] {
        if t <= stop.offset {
            let span = (stop.offset - previous.offset).max(EPSILON);
            let local = ((t - previous.offset) / span).clamp(0.0, 1.0);
            return mix_color(previous.color, stop.color, local, opacity);
        }
        previous = stop;
    }
    color_to_f32(last.color, opacity)
}

fn mix_color(a: UiColorRgba8, b: UiColorRgba8, t: f32, opacity: f32) -> [f32; 4] {
    let a = color_to_f32(a, opacity);
    let b = color_to_f32(b, opacity);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn color_to_f32(color: UiColorRgba8, opacity: f32) -> [f32; 4] {
    [
        f32::from(color.red) / 255.0,
        f32::from(color.green) / 255.0,
        f32::from(color.blue) / 255.0,
        (f32::from(color.alpha) / 255.0) * opacity.clamp(0.0, 1.0),
    ]
}

fn upload_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame: &RenderImageFrame,
    label: &'static str,
) -> wgpu::Texture {
    device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: frame.width.max(1),
                height: frame.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &frame.rgba,
    )
}

fn color_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("arcweft-ui-direct-color-shader"),
        source: wgpu::ShaderSource::Wgsl(COLOR_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("arcweft-ui-direct-color-layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("arcweft-ui-direct-color-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<UiColorVertex>() as u64,
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

fn image_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline, wgpu::Sampler) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("arcweft-ui-direct-image-shader"),
        source: wgpu::ShaderSource::Wgsl(IMAGE_SHADER.into()),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("arcweft-ui-direct-image-bind-group-layout"),
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
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("arcweft-ui-direct-image-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("arcweft-ui-direct-image-layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("arcweft-ui-direct-image-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<UiImageVertex>() as u64,
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
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: (std::mem::size_of::<[f32; 2]>() * 2) as u64,
                        shader_location: 2,
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
    (bind_group_layout, pipeline, sampler)
}

trait HitRectCenter {
    fn center(self) -> LogicalPoint;
}

impl HitRectCenter for HitRect {
    fn center(self) -> LogicalPoint {
        LogicalPoint {
            x: self.x + self.width * 0.5,
            y: self.y + self.height * 0.5,
        }
    }
}

const COLOR_SHADER: &str = r"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>) -> VertexOut {
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
@group(0) @binding(0) var ui_texture: texture_2d<f32>;
@group(0) @binding(1) var ui_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) opacity: vec4<f32>,
) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    out.opacity = opacity.x;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let color = textureSample(ui_texture, ui_sampler, in.uv);
    return vec4<f32>(color.rgb, color.a * in.opacity);
}
";
