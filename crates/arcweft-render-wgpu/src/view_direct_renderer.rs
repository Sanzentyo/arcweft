//! Shared wgpu direct primitive renderer for `ViewScene` paint ranges.
//!
//! This module is intentionally renderer-owned. It consumes Arcweft-owned
//! `ViewPrimitive` values, prepared image/mask resources, and explicit text
//! handoff records. It does not parse CSS, inspect Takumi computed style, or
//! route UI through platform-specific DOM/canvas fallback paths.

use crate::geometry::{PreparedViewMaskResource, PreparedViewSceneResources, RenderImageFrame};
use crate::view_compositor::{
    ViewCompositorError, ViewCompositorTarget, ViewDirectPrimitiveRenderer,
    ViewMaskTextureProvider, ViewMaskTextureView,
};
use crate::view_effects::ViewTextureExtent;
use crate::view_mask::ViewMaskChannel;
use crate::view_scene::{
    ViewAffine2D, ViewBorder, ViewCaretPrimitive, ViewClip, ViewColorRgba8,
    ViewCompositionUnderline, ViewCornerRadii, ViewCornerRadius, ViewGlyphRun, ViewImagePrimitive,
    ViewLinearGradient, ViewMaskImage, ViewPrimitive, ViewRoundedRect, ViewScene, ViewSceneContext,
    ViewSelectionPrimitive, ViewSolidRect, ViewUnderlineStyle,
};
use arcweft_presentation::hit::HitRect;
use bytemuck::{Pod, Zeroable};
use num_traits::ToPrimitive;
use wgpu::util::DeviceExt;

const ROUNDED_CORNER_SEGMENTS: usize = 8;
const EPSILON: f32 = 0.0001;

/// Direct primitive renderer used by `SharedRenderer` when a prepared frame has
/// attached `ViewScene`s.
pub struct WgpuViewDirectPrimitiveRenderer {
    color_pipeline: wgpu::RenderPipeline,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_pipeline: wgpu::RenderPipeline,
    image_sampler: wgpu::Sampler,
}

/// Renderer-owned direct primitive frame bound to one prepared UI scene resource table.
pub struct WgpuViewDirectPrimitiveRenderFrame<'a> {
    renderer: &'a WgpuViewDirectPrimitiveRenderer,
    resources: &'a PreparedViewSceneResources,
}

/// Renderer-owned mask texture provider for one prepared UI scene.
pub struct WgpuPreparedViewMaskTextureProvider {
    masks: Vec<WgpuPreparedViewMaskTexture>,
}

struct WgpuPreparedViewMaskTexture {
    image: ViewMaskImage,
    channel: ViewMaskChannel,
    extent: ViewTextureExtent,
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ViewColorVertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ViewImageVertex {
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
    scene: &'a ViewScene,
    context: &'a ViewSceneContext,
    target: ViewCompositorTarget<'target>,
}

impl WgpuViewDirectPrimitiveRenderer {
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

    pub const fn for_resources<'a>(
        &'a self,
        resources: &'a PreparedViewSceneResources,
    ) -> WgpuViewDirectPrimitiveRenderFrame<'a> {
        WgpuViewDirectPrimitiveRenderFrame {
            renderer: self,
            resources,
        }
    }
}

impl WgpuViewDirectPrimitiveRenderFrame<'_> {
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
        vertices: &[ViewColorVertex],
    ) -> Result<(), ViewCompositorError> {
        if vertices.is_empty() {
            return Ok(());
        }
        let buffer = frame
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("arcweft-view-direct-color-vertices"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arcweft-view-direct-color-pass"),
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
        image: &ViewImagePrimitive,
    ) -> Result<(), ViewCompositorError> {
        let image_frame = self.image_frame(image.resource_index).ok_or(
            ViewCompositorError::MissingImageResource {
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
            "arcweft-view-image-resource",
        );
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = frame.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arcweft-view-direct-image-bind-group"),
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
                label: Some("arcweft-view-direct-image-vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arcweft-view-direct-image-pass"),
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

impl ViewDirectPrimitiveRenderer for WgpuViewDirectPrimitiveRenderFrame<'_> {
    fn render_direct_range(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &ViewScene,
        context: &ViewSceneContext,
        target: ViewCompositorTarget<'_>,
    ) -> Result<(), ViewCompositorError> {
        let mut frame = DirectRenderContext {
            device,
            queue,
            encoder,
            scene,
            context,
            target,
        };
        let (start, end) = primitive_range_bounds(context)?;
        let primitives = scene.primitives().get(start..end).ok_or(
            ViewCompositorError::InvalidPrimitiveRange {
                start: context.primitive_range.start,
                end: context.primitive_range.end,
            },
        )?;
        let mut colored = Vec::new();
        for primitive in primitives {
            match primitive {
                ViewPrimitive::SolidRect(rect) => {
                    push_solid_rect(scene, context, target, rect, &mut colored);
                }
                ViewPrimitive::RoundedRect(rect) => {
                    push_rounded_rect(scene, context, target, rect, &mut colored);
                }
                ViewPrimitive::Border(border) => {
                    push_border(scene, context, target, border, &mut colored);
                }
                ViewPrimitive::LinearGradient(gradient) => {
                    push_linear_gradient(scene, context, target, gradient, &mut colored);
                }
                ViewPrimitive::Image(image) => {
                    self.render_colored_vertices(&mut frame, &colored)?;
                    colored.clear();
                    self.render_image_primitive(&mut frame, image)?;
                }
                ViewPrimitive::GlyphRun(run) => {
                    self.render_colored_vertices(&mut frame, &colored)?;
                    colored.clear();
                    ensure_glyph_handoff(self, run)?;
                }
                ViewPrimitive::Selection(selection) => {
                    push_selection(scene, context, target, selection, &mut colored);
                }
                ViewPrimitive::Caret(caret) => {
                    push_caret(scene, context, target, caret, &mut colored);
                }
                ViewPrimitive::CompositionUnderline(underline) => {
                    push_composition_underline(scene, context, target, underline, &mut colored);
                }
            }
        }
        self.render_colored_vertices(&mut frame, &colored)
    }
}

impl WgpuPreparedViewMaskTextureProvider {
    pub fn prepare(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &PreparedViewSceneResources,
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

impl ViewMaskTextureProvider for WgpuPreparedViewMaskTextureProvider {
    fn texture_for<'a>(&'a mut self, image: &ViewMaskImage) -> Option<ViewMaskTextureView<'a>> {
        self.masks
            .iter()
            .find(|mask| &mask.image == image)
            .map(|mask| ViewMaskTextureView {
                view: &mask.view,
                channel: mask.channel,
                extent: mask.extent,
            })
    }
}

fn upload_mask_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resource: &PreparedViewMaskResource,
) -> WgpuPreparedViewMaskTexture {
    let texture = upload_rgba_texture(device, queue, &resource.frame, "arcweft-view-mask-resource");
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    WgpuPreparedViewMaskTexture {
        image: resource.image.clone(),
        channel: resource.channel,
        extent: ViewTextureExtent::new(resource.frame.width, resource.frame.height),
        _texture: texture,
        view,
    }
}

fn ensure_glyph_handoff(
    renderer: &WgpuViewDirectPrimitiveRenderFrame<'_>,
    run: &ViewGlyphRun,
) -> Result<(), ViewCompositorError> {
    if renderer.has_glyph_handoff(run.run_index) {
        Ok(())
    } else {
        Err(ViewCompositorError::UnhandledGlyphRun {
            run_index: run.run_index,
        })
    }
}

fn primitive_range_bounds(
    context: &ViewSceneContext,
) -> Result<(usize, usize), ViewCompositorError> {
    let start = usize::try_from(context.primitive_range.start).map_err(|_| {
        ViewCompositorError::InvalidPrimitiveRange {
            start: context.primitive_range.start,
            end: context.primitive_range.end,
        }
    })?;
    let end = usize::try_from(context.primitive_range.end).map_err(|_| {
        ViewCompositorError::InvalidPrimitiveRange {
            start: context.primitive_range.start,
            end: context.primitive_range.end,
        }
    })?;
    Ok((start, end))
}

fn push_solid_rect(
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    rect: &ViewSolidRect,
    output: &mut Vec<ViewColorVertex>,
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
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    rect: &ViewRoundedRect,
    output: &mut Vec<ViewColorVertex>,
) {
    let color = color_to_f32(rect.color, context.opacity);
    let points = rounded_rect_points(rect.bounds, rect.radii);
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
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    border: &ViewBorder,
    output: &mut Vec<ViewColorVertex>,
) {
    if border.width <= 0.0 {
        return;
    }
    let color = color_to_f32(border.color, context.opacity);
    let outer = rounded_rect_points(border.bounds, ViewCornerRadii::uniform(border.radius));
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
    let inner = rounded_rect_points(
        inner_bounds,
        ViewCornerRadii::uniform((border.radius - border.width).max(0.0)),
    );
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
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    gradient: &ViewLinearGradient,
    output: &mut Vec<ViewColorVertex>,
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
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    selection: &ViewSelectionPrimitive,
    output: &mut Vec<ViewColorVertex>,
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
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    caret: &ViewCaretPrimitive,
    output: &mut Vec<ViewColorVertex>,
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
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    underline: &ViewCompositionUnderline,
    output: &mut Vec<ViewColorVertex>,
) {
    let thickness = underline.thickness.max(1.0);
    let y = underline.bounds.y + underline.bounds.height - thickness;
    let mut bounds = HitRect::new(underline.bounds.x, y, underline.bounds.width, thickness);
    if underline.style == ViewUnderlineStyle::Dotted {
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
    if underline.style == ViewUnderlineStyle::Dashed {
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
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    bounds: HitRect,
    colors: [[f32; 4]; 4],
    output: &mut Vec<ViewColorVertex>,
) {
    push_quad(scene, context, target, rect_corners(bounds), colors, output);
}

fn push_quad(
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    points: [LogicalPoint; 4],
    colors: [[f32; 4]; 4],
    output: &mut Vec<ViewColorVertex>,
) {
    let vertices = [0, 1, 2, 0, 2, 3];
    output.extend(vertices.into_iter().map(|index| ViewColorVertex {
        position: logical_to_ndc(scene, context, target, points[index]),
        color: colors[index],
    }));
}

fn push_polygon_fan(
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    center: LogicalPoint,
    points: &[LogicalPoint],
    color: [f32; 4],
    output: &mut Vec<ViewColorVertex>,
) {
    if points.len() < 3 {
        return;
    }
    for index in 0..points.len() {
        let next = (index + 1) % points.len();
        for point in [center, points[index], points[next]] {
            output.push(ViewColorVertex {
                position: logical_to_ndc(scene, context, target, point),
                color,
            });
        }
    }
}

fn image_vertices(
    scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    image: &ViewImagePrimitive,
) -> [ViewImageVertex; 6] {
    let corners = rect_corners(image.bounds);
    let positions = corners.map(|point| logical_to_ndc(scene, context, target, point));
    let opacity = [
        image.opacity.clamp(0.0, 1.0) * context.opacity.clamp(0.0, 1.0),
        0.0,
        0.0,
        0.0,
    ];
    [
        ViewImageVertex {
            position: positions[0],
            uv: [0.0, 0.0],
            opacity,
        },
        ViewImageVertex {
            position: positions[1],
            uv: [0.0, 1.0],
            opacity,
        },
        ViewImageVertex {
            position: positions[2],
            uv: [1.0, 1.0],
            opacity,
        },
        ViewImageVertex {
            position: positions[0],
            uv: [0.0, 0.0],
            opacity,
        },
        ViewImageVertex {
            position: positions[2],
            uv: [1.0, 1.0],
            opacity,
        },
        ViewImageVertex {
            position: positions[3],
            uv: [1.0, 0.0],
            opacity,
        },
    ]
}

fn logical_to_ndc(
    _scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
    point: LogicalPoint,
) -> [f32; 2] {
    let transformed = apply_transform(context.transform, point);
    let scale_x = target_axis_scale(target.extent.width, target.logical_extent[0]);
    let scale_y = target_axis_scale(target.extent.height, target.logical_extent[1]);
    let x = (transformed.x - target.origin_logical[0]) * scale_x;
    let y = (transformed.y - target.origin_logical[1]) * scale_y;
    let target_width = u32_to_f32(target.extent.width.max(1));
    let target_height = u32_to_f32(target.extent.height.max(1));
    [
        (x / target_width) * 2.0 - 1.0,
        1.0 - (y / target_height) * 2.0,
    ]
}

fn target_axis_scale(extent: u32, logical_extent: f32) -> f32 {
    u32_to_f32(extent.max(1)) / logical_extent.max(EPSILON)
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

fn apply_transform(transform: ViewAffine2D, point: LogicalPoint) -> LogicalPoint {
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
    _scene: &ViewScene,
    context: &ViewSceneContext,
    target: ViewCompositorTarget<'_>,
) -> Result<(), ViewCompositorError> {
    let Some(clip) = &context.clip else {
        return Ok(());
    };
    let bounds = match clip {
        ViewClip::Rect(bounds) | ViewClip::RoundedRect { bounds, .. } => *bounds,
    };
    let scale_x = target_axis_scale(target.extent.width, target.logical_extent[0]);
    let scale_y = target_axis_scale(target.extent.height, target.logical_extent[1]);
    let x = nonnegative_floor_to_u32((bounds.x - target.origin_logical[0]) * scale_x);
    let y = nonnegative_floor_to_u32((bounds.y - target.origin_logical[1]) * scale_y);
    let max_width = target.extent.width.saturating_sub(x);
    let max_height = target.extent.height.saturating_sub(y);
    let width = nonnegative_ceil_to_u32(bounds.width * scale_x).min(max_width);
    let height = nonnegative_ceil_to_u32(bounds.height * scale_y).min(max_height);
    if width == 0 || height == 0 {
        return Err(ViewCompositorError::UnsupportedClip {
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

fn rounded_rect_points(bounds: HitRect, radii: ViewCornerRadii) -> Vec<LogicalPoint> {
    let radii = radii.clamped_to_rect(bounds);
    if rounded_radii_are_empty(radii) {
        return rect_corners(bounds).to_vec();
    }
    let corners = [
        (
            bounds.x + radii.top_left.x_px,
            bounds.y + radii.top_left.y_px,
            radii.top_left,
            std::f32::consts::PI,
            1.5 * std::f32::consts::PI,
        ),
        (
            bounds.x + bounds.width - radii.top_right.x_px,
            bounds.y + radii.top_right.y_px,
            radii.top_right,
            1.5 * std::f32::consts::PI,
            2.0 * std::f32::consts::PI,
        ),
        (
            bounds.x + bounds.width - radii.bottom_right.x_px,
            bounds.y + bounds.height - radii.bottom_right.y_px,
            radii.bottom_right,
            0.0,
            0.5 * std::f32::consts::PI,
        ),
        (
            bounds.x + radii.bottom_left.x_px,
            bounds.y + bounds.height - radii.bottom_left.y_px,
            radii.bottom_left,
            0.5 * std::f32::consts::PI,
            std::f32::consts::PI,
        ),
    ];
    corners
        .into_iter()
        .flat_map(|(cx, cy, radius, start, end)| {
            (0..=ROUNDED_CORNER_SEGMENTS).map(move |step| {
                let t = usize_to_f32(step) / usize_to_f32(ROUNDED_CORNER_SEGMENTS);
                let angle = start + (end - start) * t;
                LogicalPoint {
                    x: cx + radius.x_px * angle.cos(),
                    y: cy + radius.y_px * angle.sin(),
                }
            })
        })
        .collect()
}

fn rounded_radii_are_empty(radii: ViewCornerRadii) -> bool {
    corner_radius_is_empty(radii.top_left)
        && corner_radius_is_empty(radii.top_right)
        && corner_radius_is_empty(radii.bottom_right)
        && corner_radius_is_empty(radii.bottom_left)
}

fn corner_radius_is_empty(radius: ViewCornerRadius) -> bool {
    radius.x_px <= EPSILON || radius.y_px <= EPSILON
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

fn gradient_color_at(gradient: &ViewLinearGradient, t: f32, opacity: f32) -> [f32; 4] {
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

fn mix_color(a: ViewColorRgba8, b: ViewColorRgba8, t: f32, opacity: f32) -> [f32; 4] {
    let a = color_to_f32(a, opacity);
    let b = color_to_f32(b, opacity);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn color_to_f32(color: ViewColorRgba8, opacity: f32) -> [f32; 4] {
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
        label: Some("arcweft-view-direct-color-shader"),
        source: wgpu::ShaderSource::Wgsl(COLOR_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("arcweft-view-direct-color-layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("arcweft-view-direct-color-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<ViewColorVertex>() as u64,
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
        label: Some("arcweft-view-direct-image-shader"),
        source: wgpu::ShaderSource::Wgsl(IMAGE_SHADER.into()),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("arcweft-view-direct-image-bind-group-layout"),
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
        label: Some("arcweft-view-direct-image-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("arcweft-view-direct-image-layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("arcweft-view-direct-image-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<ViewImageVertex>() as u64,
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
