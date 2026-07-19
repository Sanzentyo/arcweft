mod prepared_text;
mod view_text;

use prepared_text::render_prepared_text_range_with_renderer;
use view_text::WgpuViewPreparedTextRenderer;

use crate::geometry::{
    PaintRect, PreparedControlPaint, PreparedControlShadow, PreparedFrame, PreparedViewScene,
    RenderImage, RuntimeControlBackdropSamplePolicy,
};
use crate::view_compositor::{
    ViewCompositor, ViewCompositorError, ViewCompositorFrame, ViewCompositorTarget,
    ViewInlineBackdropFilterFrame, ViewInlineBoxShadowFrame, ViewInlineForegroundFilterFrame,
};
use crate::view_direct_renderer::{
    WgpuPreparedViewMaskTextureProvider, WgpuViewDirectPrimitiveRenderer,
};
use crate::view_effects::ViewTextureExtent;
use crate::view_scene::ViewBoxShadowKind;
use arcweft_glyphon::{GlyphonTextEngine, GlyphonTextEngineError};
use bytemuck::{Pod, Zeroable};
use glyphon::{Cache, Resolution, TextAtlas, TextRenderer, Viewport};
use std::borrow::Cow;
use std::ops::Range;
use thiserror::Error;
use wgpu::util::DeviceExt;

/// GPU renderer shared by native surfaces, browser surfaces, and offscreen tests.
pub struct SharedRenderer {
    format: wgpu::TextureFormat,
    rectangle_pipeline: wgpu::RenderPipeline,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_pipeline: wgpu::RenderPipeline,
    image_sampler: wgpu::Sampler,
    glyphon_cache: Cache,
    prepared_text_engine: Option<GlyphonTextEngine>,
    viewport: Viewport,
    atlas: TextAtlas,
    /// Per-submission renderers kept alive until the frame command buffer is submitted.
    ///
    /// `glyphon::TextRenderer::prepare_*` mutates or replaces its vertex buffer. Reusing one
    /// renderer for multiple passes in a single command buffer would therefore invalidate or
    /// overwrite buffers referenced by earlier passes.
    aux_text_renderers: Vec<TextRenderer>,
    view_compositor: ViewCompositor,
    view_text_effect_compositor: ViewCompositor,
    view_direct_renderer: WgpuViewDirectPrimitiveRenderer,
    registered_font_bytes: usize,
}

/// Shared renderer failure. Platform surface errors are intentionally absent.
#[derive(Debug, Error)]
pub enum SharedRendererError {
    #[error("font bytes must not be empty")]
    EmptyFont,
    #[error("prepared text requires registered project fonts")]
    MissingPreparedTextFonts,
    #[error("prepared text font registration failed: {0}")]
    PreparedTextFont(#[from] GlyphonTextEngineError),
    #[error("glyphon text preparation failed: {0}")]
    TextPrepare(String),
    #[error("glyphon text rendering failed: {0}")]
    TextRender(String),
    #[error("view compositor failed: {0}")]
    ViewCompositor(#[from] ViewCompositorError),
    #[error("renderer target extent must be non-zero, got {width}x{height}")]
    InvalidTargetExtent { width: u32, height: u32 },
    #[error("renderer target extent is not exactly representable as f32, got {width}x{height}")]
    InexactTargetExtent { width: u32, height: u32 },
    #[error(
        "renderer logical viewport must be finite and positive, got bits {width_bits:#010x}x{height_bits:#010x}"
    )]
    InvalidLogicalExtent { width_bits: u32, height_bits: u32 },
    #[error("renderer scale factor must be finite, positive, and fit f32: {value_bits:#018x}")]
    InvalidScaleFactor { value_bits: u64 },
    #[error("renderer rectangle vertex count exceeds u32 for {rectangles} rectangles")]
    RectangleVertexCount { rectangles: usize },
    #[error("renderer image row byte count overflows u32 for width {width}")]
    ImageRowBytesRange { width: u32 },
}

/// Fully encoded renderer work whose only remaining operation is infallible
/// queue submission.
pub struct PreparedSharedRenderSubmission {
    command_buffer: wgpu::CommandBuffer,
}

impl PreparedSharedRenderSubmission {
    pub fn submit(self, queue: &wgpu::Queue) {
        queue.submit([self.command_buffer]);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct RectVertex {
    position: [f32; 2],
    color: [f32; 4],
    local: [f32; 2],
    size: [f32; 2],
    radii_top: [f32; 4],
    radii_bottom: [f32; 4],
    clip_local: [f32; 2],
    clip_size: [f32; 2],
    clip_radii_top: [f32; 4],
    clip_radii_bottom: [f32; 4],
    clip_params: [f32; 2],
    stroke_width: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ImageVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

struct PreparedTextRangeRenderRequest<'a> {
    target: &'a wgpu::TextureView,
    frame: &'a PreparedFrame,
    range: Range<usize>,
    excluded_ranges: &'a [Range<usize>],
}

impl SharedRenderer {
    /// Creates pipelines for a host-selected render-target format.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let glyphon_cache = Cache::new(device);
        let atlas = TextAtlas::new(device, queue, &glyphon_cache, format);
        let (image_bind_group_layout, image_pipeline, image_sampler) =
            image_quad_pipeline(device, format);
        let view_compositor = ViewCompositor::new(device, queue, format);
        let view_text_effect_compositor = ViewCompositor::new(device, queue, format);
        let view_direct_renderer = WgpuViewDirectPrimitiveRenderer::new(device, format);
        Self {
            format,
            rectangle_pipeline: rectangle_pipeline(device, format),
            image_bind_group_layout,
            image_pipeline,
            image_sampler,
            viewport: Viewport::new(device, &glyphon_cache),
            glyphon_cache,
            prepared_text_engine: None,
            atlas,
            aux_text_renderers: Vec::new(),
            view_compositor,
            view_text_effect_compositor,
            view_direct_renderer,
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
        let byte_len = bytes.len();
        if let Some(engine) = &mut self.prepared_text_engine {
            engine.register_project_font(bytes)?;
        } else {
            self.prepared_text_engine =
                Some(GlyphonTextEngine::from_project_fonts("und", vec![bytes])?);
        }
        self.registered_font_bytes = self.registered_font_bytes.saturating_add(byte_len);
        Ok(())
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
        let submission = self.prepare_to_view(device, queue, target, frame)?;
        submission.submit(queue);
        Ok(())
    }

    /// Renders a prepared coverage frame over transparent black through the
    /// same compositor, glyph atlas, transforms, and clips as normal output.
    pub(crate) fn render_coverage_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        frame: &PreparedFrame,
    ) -> Result<(), SharedRendererError> {
        let submission = self.prepare_to_view_with_clear(
            device,
            queue,
            target,
            frame,
            wgpu::Color::TRANSPARENT,
        )?;
        submission.submit(queue);
        Ok(())
    }

    /// Completes every fallible renderer operation without submitting GPU work.
    pub fn prepare_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        frame: &PreparedFrame,
    ) -> Result<PreparedSharedRenderSubmission, SharedRendererError> {
        self.prepare_to_view_with_clear(device, queue, target, frame, wgpu::Color::BLACK)
    }

    fn prepare_to_view_with_clear(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        frame: &PreparedFrame,
        clear: wgpu::Color,
    ) -> Result<PreparedSharedRenderSubmission, SharedRendererError> {
        if frame.viewport.physical_width == 0 || frame.viewport.physical_height == 0 {
            return Err(SharedRendererError::InvalidTargetExtent {
                width: frame.viewport.physical_width,
                height: frame.viewport.physical_height,
            });
        }
        if exact_extent_f32(frame.viewport.physical_width).is_none()
            || exact_extent_f32(frame.viewport.physical_height).is_none()
        {
            return Err(SharedRendererError::InexactTargetExtent {
                width: frame.viewport.physical_width,
                height: frame.viewport.physical_height,
            });
        }
        if !frame.viewport.logical_width.is_finite()
            || frame.viewport.logical_width <= 0.0
            || !frame.viewport.logical_height.is_finite()
            || frame.viewport.logical_height <= 0.0
        {
            return Err(SharedRendererError::InvalidLogicalExtent {
                width_bits: frame.viewport.logical_width.to_bits(),
                height_bits: frame.viewport.logical_height.to_bits(),
            });
        }
        if !frame.viewport.scale_factor.is_finite()
            || frame.viewport.scale_factor <= 0.0
            || frame.viewport.scale_factor > f64::from(f32::MAX)
        {
            return Err(SharedRendererError::InvalidScaleFactor {
                value_bits: frame.viewport.scale_factor.to_bits(),
            });
        }
        let rectangle_vertex_count = frame.rectangles.len().checked_mul(6).ok_or(
            SharedRendererError::RectangleVertexCount {
                rectangles: frame.rectangles.len(),
            },
        )?;
        u32::try_from(rectangle_vertex_count).map_err(|_| {
            SharedRendererError::RectangleVertexCount {
                rectangles: frame.rectangles.len(),
            }
        })?;
        let target_extent = ViewTextureExtent::new(
            frame.viewport.physical_width,
            frame.viewport.physical_height,
        );
        let scene_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("arcweft-shared-runtime-scene-target"),
            size: wgpu::Extent3d {
                width: target_extent.width,
                height: target_extent.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let scene_view = scene_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.viewport.update(
            queue,
            Resolution {
                width: frame.viewport.physical_width,
                height: frame.viewport.physical_height,
            },
        );
        self.aux_text_renderers.clear();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("arcweft-shared-render-frame"),
        });
        self.render_background_and_images(device, queue, &mut encoder, &scene_view, frame, clear)?;
        for view_scene in frame.view_scenes() {
            self.render_view_scene(device, queue, &mut encoder, &scene_view, frame, view_scene)?;
        }
        self.render_ordered_frame_content(
            device,
            queue,
            &mut encoder,
            &scene_texture,
            &scene_view,
            target_extent,
            frame,
        )?;
        self.view_compositor.composite_texture_to_view(
            device,
            &mut encoder,
            &scene_view,
            target,
            clear,
        );
        self.atlas.trim();
        Ok(PreparedSharedRenderSubmission {
            command_buffer: encoder.finish(),
        })
    }

    fn render_view_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        final_target: &wgpu::TextureView,
        frame: &PreparedFrame,
        prepared_view: &PreparedViewScene,
    ) -> Result<(), SharedRendererError> {
        let mut mask_textures =
            WgpuPreparedViewMaskTextureProvider::prepare(device, queue, &prepared_view.resources)?;
        let mut direct_renderer = self
            .view_direct_renderer
            .for_resources(&prepared_view.resources);
        let mut text_renderer = WgpuViewPreparedTextRenderer::new(
            frame,
            self.prepared_text_engine.as_mut(),
            &self.glyphon_cache,
            &mut self.atlas,
            &mut self.aux_text_renderers,
            &mut self.view_text_effect_compositor,
            &self.view_direct_renderer,
            frame.viewport.physical_scale_factor_f32(),
        );
        let result = self.view_compositor.render_scene(&mut ViewCompositorFrame {
            device,
            queue,
            encoder,
            final_target,
            scene: &prepared_view.scene,
            target_extent: ViewTextureExtent::new(
                frame.viewport.physical_width,
                frame.viewport.physical_height,
            ),
            device_pixel_ratio: frame.viewport.physical_scale_factor_f32(),
            direct_renderer: &mut direct_renderer,
            text_renderer: &mut text_renderer,
            mask_textures: &mut mask_textures,
        });
        result?;
        Ok(())
    }

    fn render_background_and_images(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        frame: &PreparedFrame,
        clear: wgpu::Color,
    ) -> Result<(), SharedRendererError> {
        let background_vertex_buffer = rectangle_vertex_buffer(
            device,
            "arcweft-shared-background-rectangle",
            frame.rectangles.get(..1).unwrap_or_default(),
            frame.viewport.logical_width,
            frame.viewport.logical_height,
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("arcweft-shared-render-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
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
            )?;
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Rendering one prepared frame needs the caller-owned GPU handles and target state."
    )]
    fn render_ordered_frame_content(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_texture: &wgpu::Texture,
        target_view: &wgpu::TextureView,
        target_extent: ViewTextureExtent,
        frame: &PreparedFrame,
    ) -> Result<(), SharedRendererError> {
        let mut next_rectangle = 1;
        let mut controls = frame.control_paints.iter().collect::<Vec<_>>();
        controls.sort_by_key(|paint| (paint.rectangle_range.start, paint.text_range.start));
        let excluded_prepared_text_ranges = controls
            .iter()
            .filter(|paint| {
                !slice_range(&frame.control_filters, paint.filter_range.clone()).is_empty()
            })
            .map(|paint| paint.text_range.clone())
            .chain(
                frame
                    .view_scenes()
                    .iter()
                    .flat_map(|prepared| prepared.scene.prepared_text_ids())
                    .filter_map(|text| usize::try_from(text.index()).ok())
                    .filter_map(|index| index.checked_add(1).map(|end| index..end)),
            )
            .collect::<Vec<_>>();
        let runtime_backdrop_source = self.prepare_runtime_control_backdrop_source(
            device,
            encoder,
            target_texture,
            target_view,
            target_extent,
            frame,
            &controls,
            &mut next_rectangle,
        );

        for paint in controls {
            self.render_runtime_control_paint(
                device,
                queue,
                encoder,
                target_texture,
                target_view,
                target_extent,
                frame,
                runtime_backdrop_source.as_ref(),
                paint,
                &mut next_rectangle,
            )?;
        }

        self.render_rectangle_range(
            device,
            encoder,
            target_view,
            frame,
            next_rectangle..frame.rectangles.len(),
            "arcweft-shared-post-control-rectangles",
        );
        self.render_prepared_text_range(
            device,
            queue,
            encoder,
            &PreparedTextRangeRenderRequest {
                target: target_view,
                frame,
                range: 0..frame.text.len(),
                excluded_ranges: &excluded_prepared_text_ranges,
            },
        )?;
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Runtime controls are interleaved with shared frame spans."
    )]
    fn render_runtime_control_paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_texture: &wgpu::Texture,
        target_view: &wgpu::TextureView,
        target_extent: ViewTextureExtent,
        frame: &PreparedFrame,
        runtime_backdrop_source: Option<&RuntimeControlBackdropSource>,
        paint: &PreparedControlPaint,
        next_rectangle: &mut usize,
    ) -> Result<(), SharedRendererError> {
        self.render_rectangle_range(
            device,
            encoder,
            target_view,
            frame,
            *next_rectangle..paint.rectangle_range.start,
            "arcweft-shared-pre-control-rectangles",
        );
        *next_rectangle = (*next_rectangle).max(paint.rectangle_range.start);

        let shadow_target = RuntimeControlShadowTarget {
            texture: target_texture,
            view: target_view,
            extent: target_extent,
            logical_extent: frame_logical_extent(frame),
        };
        self.render_control_shadows(
            device,
            encoder,
            shadow_target,
            frame,
            paint,
            ViewBoxShadowKind::Outer,
        );
        self.render_control_backdrops(
            device,
            encoder,
            target_texture,
            target_view,
            target_extent,
            frame,
            runtime_backdrop_source,
            paint,
        )?;
        if slice_range(&frame.control_filters, paint.filter_range.clone()).is_empty() {
            self.render_rectangle_range(
                device,
                encoder,
                target_view,
                frame,
                paint.rectangle_range.clone(),
                "arcweft-shared-runtime-control-rectangles",
            );
            self.render_control_shadows(
                device,
                encoder,
                shadow_target,
                frame,
                paint,
                ViewBoxShadowKind::Inset,
            );
        } else {
            self.render_filtered_control(
                device,
                queue,
                encoder,
                target_texture,
                target_view,
                target_extent,
                frame,
                paint,
            )?;
            self.render_control_shadows(
                device,
                encoder,
                shadow_target,
                frame,
                paint,
                ViewBoxShadowKind::Inset,
            );
        }
        *next_rectangle = (*next_rectangle).max(paint.rectangle_range.end);
        Ok(())
    }

    fn render_control_shadows(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: RuntimeControlShadowTarget<'_>,
        frame: &PreparedFrame,
        paint: &PreparedControlPaint,
        kind: ViewBoxShadowKind,
    ) {
        for shadow in slice_range(&frame.control_shadows, paint.shadow_range.clone()) {
            self.render_control_shadow(device, encoder, target, shadow, kind);
        }
    }

    fn render_control_shadow(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: RuntimeControlShadowTarget<'_>,
        shadow: &PreparedControlShadow,
        kind: ViewBoxShadowKind,
    ) {
        let target = ViewCompositorTarget {
            texture: target.texture,
            view: target.view,
            extent: target.extent,
            origin_logical: [0.0, 0.0],
            logical_extent: target.logical_extent,
        };
        let mut request = ViewInlineBoxShadowFrame {
            device,
            encoder,
            target,
            plan: &shadow.plan,
            kind,
        };
        let _ = self.view_compositor.render_inline_box_shadow(&mut request);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "The source snapshot has to be taken between ordered frame spans."
    )]
    fn prepare_runtime_control_backdrop_source(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target_texture: &wgpu::Texture,
        target_view: &wgpu::TextureView,
        target_extent: ViewTextureExtent,
        frame: &PreparedFrame,
        controls: &[&PreparedControlPaint],
        next_rectangle: &mut usize,
    ) -> Option<RuntimeControlBackdropSource> {
        let first_control = controls.first().copied()?;
        self.render_rectangle_range(
            device,
            encoder,
            target_view,
            frame,
            *next_rectangle..first_control.rectangle_range.start,
            "arcweft-shared-pre-control-rectangles",
        );
        *next_rectangle = (*next_rectangle).max(first_control.rectangle_range.start);
        controls_need_prior_frame_backdrop_source(controls, frame).then(|| {
            runtime_control_backdrop_source_texture(
                device,
                encoder,
                self.format,
                target_texture,
                target_extent,
                frame_logical_extent(frame),
            )
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "The compositor call needs the prepared control span and active target."
    )]
    fn render_control_backdrops(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target_texture: &wgpu::Texture,
        target_view: &wgpu::TextureView,
        target_extent: ViewTextureExtent,
        frame: &PreparedFrame,
        runtime_backdrop_source: Option<&RuntimeControlBackdropSource>,
        paint: &PreparedControlPaint,
    ) -> Result<(), SharedRendererError> {
        let backdrops = slice_range(&frame.control_backdrops, paint.backdrop_range.clone());
        let logical_extent = frame_logical_extent(frame);
        for backdrop in backdrops {
            let target = ViewCompositorTarget {
                texture: target_texture,
                view: target_view,
                extent: target_extent,
                origin_logical: [0.0, 0.0],
                logical_extent,
            };
            let source = match backdrop.sample_policy {
                RuntimeControlBackdropSamplePolicy::PriorFrameContent => {
                    runtime_backdrop_source.map_or(target, RuntimeControlBackdropSource::as_target)
                }
                RuntimeControlBackdropSamplePolicy::PriorFrameContentAndEarlierRuntimeControls => {
                    target
                }
            };
            let mut request = ViewInlineBackdropFilterFrame {
                device,
                encoder: &mut *encoder,
                source,
                target,
                bounds: backdrop.bounds,
                filters: &backdrop.filters,
                device_pixel_ratio: frame.viewport.physical_scale_factor_f32(),
                logical_extent,
            };
            self.view_compositor
                .render_inline_backdrop_filter(&mut request)?;
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Filtered runtime-control replay needs source and destination GPU handles."
    )]
    fn render_filtered_control(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target_texture: &wgpu::Texture,
        target_view: &wgpu::TextureView,
        target_extent: ViewTextureExtent,
        frame: &PreparedFrame,
        paint: &PreparedControlPaint,
    ) -> Result<(), SharedRendererError> {
        let control_texture = runtime_control_filter_texture(
            device,
            self.format,
            target_extent,
            "arcweft-shared-runtime-control-filter-source",
        );
        let control_view = control_texture.create_view(&wgpu::TextureViewDescriptor::default());
        clear_texture_view(
            encoder,
            &control_view,
            wgpu::Color::TRANSPARENT,
            "arcweft-shared-runtime-control-filter-clear",
        );
        self.render_rectangle_range(
            device,
            encoder,
            &control_view,
            frame,
            paint.rectangle_range.clone(),
            "arcweft-shared-runtime-control-filter-rectangles",
        );
        render_prepared_text_range_with_renderer(
            device,
            queue,
            encoder,
            &self.rectangle_pipeline,
            &mut self.aux_text_renderers,
            self.prepared_text_engine.as_mut(),
            &mut self.atlas,
            &self.viewport,
            &mut self.view_compositor,
            &PreparedTextRangeRenderRequest {
                target: &control_view,
                frame,
                range: paint.text_range.clone(),
                excluded_ranges: &[],
            },
        )?;
        let logical_extent = frame_logical_extent(frame);
        let source = ViewCompositorTarget {
            texture: &control_texture,
            view: &control_view,
            extent: target_extent,
            origin_logical: [0.0, 0.0],
            logical_extent,
        };
        let output = ViewCompositorTarget {
            texture: target_texture,
            view: target_view,
            extent: target_extent,
            origin_logical: [0.0, 0.0],
            logical_extent,
        };
        for filter in slice_range(&frame.control_filters, paint.filter_range.clone()) {
            let mut request = ViewInlineForegroundFilterFrame {
                device,
                encoder: &mut *encoder,
                source,
                output,
                bounds: filter.bounds,
                filters: &filter.filters,
                device_pixel_ratio: frame.viewport.physical_scale_factor_f32(),
                logical_extent,
            };
            self.view_compositor
                .render_inline_foreground_filter(&mut request)?;
        }
        Ok(())
    }

    fn render_rectangle_range(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        frame: &PreparedFrame,
        range: Range<usize>,
        label: &'static str,
    ) {
        let rectangles = slice_range(&frame.rectangles, range);
        let Some((vertex_buffer, vertex_count)) = rectangle_vertex_buffer(
            device,
            label,
            rectangles,
            frame.viewport.logical_width,
            frame.viewport.logical_height,
        ) else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("arcweft-shared-rectangle-range-render-pass"),
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
        draw_rectangle_buffer(
            &mut pass,
            &self.rectangle_pipeline,
            &vertex_buffer,
            vertex_count,
        );
    }

    fn render_prepared_text_range(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        request: &PreparedTextRangeRenderRequest<'_>,
    ) -> Result<(), SharedRendererError> {
        render_prepared_text_range_with_renderer(
            device,
            queue,
            encoder,
            &self.rectangle_pipeline,
            &mut self.aux_text_renderers,
            self.prepared_text_engine.as_mut(),
            &mut self.atlas,
            &self.viewport,
            &mut self.view_compositor,
            request,
        )
    }

    fn render_image_quad(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        image: &RenderImage,
        logical_width: f32,
        logical_height: f32,
    ) -> Result<(), SharedRendererError> {
        if image.frame.width == 0 || image.frame.height == 0 || image.frame.rgba.is_empty() {
            return Ok(());
        }
        let texture = upload_image_quad(device, queue, image)?;
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
        let Some(vertices) = image_vertices(image, logical_width, logical_height) else {
            return Ok(());
        };
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("arcweft-shared-image-vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        pass.set_pipeline(&self.image_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..6, 0..1);
        Ok(())
    }
}

fn text_index_is_excluded(index: usize, excluded: &[Range<usize>]) -> bool {
    excluded.iter().any(|range| range.contains(&index))
}

fn slice_range<T>(items: &[T], range: Range<usize>) -> &[T] {
    let start = range.start.min(items.len());
    let end = range.end.min(items.len()).max(start);
    &items[start..end]
}

fn frame_logical_extent(frame: &PreparedFrame) -> [f32; 2] {
    [frame.viewport.logical_width, frame.viewport.logical_height]
}

fn exact_extent_f32(value: u32) -> Option<f32> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "the round-trip check below rejects every inexact target extent"
    )]
    let converted = value as f32;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the finite nonnegative f32 originates from u32 and is used only for round-trip validation"
    )]
    (converted as u32 == value).then_some(converted)
}

#[derive(Clone, Copy)]
struct RuntimeControlShadowTarget<'a> {
    texture: &'a wgpu::Texture,
    view: &'a wgpu::TextureView,
    extent: ViewTextureExtent,
    logical_extent: [f32; 2],
}

struct RuntimeControlBackdropSource {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    extent: ViewTextureExtent,
    logical_extent: [f32; 2],
}

impl RuntimeControlBackdropSource {
    fn as_target(&self) -> ViewCompositorTarget<'_> {
        ViewCompositorTarget {
            texture: &self.texture,
            view: &self.view,
            extent: self.extent,
            origin_logical: [0.0, 0.0],
            logical_extent: self.logical_extent,
        }
    }
}

fn controls_need_prior_frame_backdrop_source(
    controls: &[&PreparedControlPaint],
    frame: &PreparedFrame,
) -> bool {
    controls.iter().any(|paint| {
        slice_range(&frame.control_backdrops, paint.backdrop_range.clone())
            .iter()
            .any(|backdrop| {
                backdrop.sample_policy == RuntimeControlBackdropSamplePolicy::PriorFrameContent
            })
    })
}

fn runtime_control_backdrop_source_texture(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    format: wgpu::TextureFormat,
    source_texture: &wgpu::Texture,
    extent: ViewTextureExtent,
    logical_extent: [f32; 2],
) -> RuntimeControlBackdropSource {
    let texture = runtime_control_texture(
        device,
        format,
        extent,
        wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        "arcweft-shared-runtime-control-backdrop-source",
    );
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: source_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        texture_extent_3d(extent),
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    RuntimeControlBackdropSource {
        texture,
        view,
        extent,
        logical_extent,
    }
}

fn runtime_control_filter_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    extent: ViewTextureExtent,
    label: &'static str,
) -> wgpu::Texture {
    runtime_control_texture(
        device,
        format,
        extent,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        label,
    )
}

fn runtime_control_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    extent: ViewTextureExtent,
    usage: wgpu::TextureUsages,
    label: &'static str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: extent.width,
            height: extent.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn texture_extent_3d(extent: ViewTextureExtent) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: extent.width,
        height: extent.height,
        depth_or_array_layers: 1,
    }
}

fn clear_texture_view(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    color: wgpu::Color,
    label: &'static str,
) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(color),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
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
        let count = u32::try_from(vertices.len())
            .expect("rectangle vertex count was validated before renderer preparation");
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
            let size = [rect.bounds.width, rect.bounds.height];
            let rect_radii = rect
                .radii
                .normalized_for(rect.bounds.width, rect.bounds.height);
            let (clip_bounds, clip_radii, clip_enabled) = rect.clip.map_or(
                (rect.bounds, crate::geometry::PaintRectRadii::ZERO, 0.0),
                |clip| {
                    (
                        clip.bounds,
                        clip.radii
                            .normalized_for(clip.bounds.width, clip.bounds.height),
                        1.0,
                    )
                },
            );
            let (radii_top, radii_bottom) = rectangle_radii_vertices(rect_radii);
            let (clip_radii_top, clip_radii_bottom) = rectangle_radii_vertices(clip_radii);
            let vertices = [
                ([left, top], [0.0, 0.0]),
                ([left, bottom], [0.0, rect.bounds.height]),
                ([right, bottom], [rect.bounds.width, rect.bounds.height]),
                ([left, top], [0.0, 0.0]),
                ([right, bottom], [rect.bounds.width, rect.bounds.height]),
                ([right, top], [rect.bounds.width, 0.0]),
            ];
            vertices.map(|(position, local)| RectVertex {
                position,
                color,
                local,
                size,
                radii_top,
                radii_bottom,
                clip_local: [
                    rect.bounds.x + local[0] - clip_bounds.x,
                    rect.bounds.y + local[1] - clip_bounds.y,
                ],
                clip_size: [clip_bounds.width, clip_bounds.height],
                clip_radii_top,
                clip_radii_bottom,
                clip_params: [clip_enabled, 0.0],
                stroke_width: rect.stroke_width_px.max(0.0),
            })
        })
        .collect()
}

fn rectangle_radii_vertices(radii: crate::geometry::PaintRectRadii) -> ([f32; 4], [f32; 4]) {
    (
        [
            radii.top_left.x_px,
            radii.top_left.y_px,
            radii.top_right.x_px,
            radii.top_right.y_px,
        ],
        [
            radii.bottom_right.x_px,
            radii.bottom_right.y_px,
            radii.bottom_left.x_px,
            radii.bottom_left.y_px,
        ],
    )
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
                        offset: std::mem::offset_of!(RectVertex, color) as u64,
                        shader_location: 1,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: std::mem::offset_of!(RectVertex, local) as u64,
                        shader_location: 2,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: std::mem::offset_of!(RectVertex, size) as u64,
                        shader_location: 3,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: std::mem::offset_of!(RectVertex, radii_top) as u64,
                        shader_location: 4,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: std::mem::offset_of!(RectVertex, radii_bottom) as u64,
                        shader_location: 5,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: std::mem::offset_of!(RectVertex, clip_local) as u64,
                        shader_location: 6,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: std::mem::offset_of!(RectVertex, clip_size) as u64,
                        shader_location: 7,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: std::mem::offset_of!(RectVertex, clip_radii_top) as u64,
                        shader_location: 8,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: std::mem::offset_of!(RectVertex, clip_radii_bottom) as u64,
                        shader_location: 9,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: std::mem::offset_of!(RectVertex, clip_params) as u64,
                        shader_location: 10,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32,
                        offset: std::mem::offset_of!(RectVertex, stroke_width) as u64,
                        shader_location: 11,
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
) -> Result<wgpu::Texture, SharedRendererError> {
    let bytes_per_row =
        image
            .frame
            .width
            .checked_mul(4)
            .ok_or(SharedRendererError::ImageRowBytesRange {
                width: image.frame.width,
            })?;
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
            bytes_per_row: Some(bytes_per_row),
            rows_per_image: Some(image.frame.height),
        },
        wgpu::Extent3d {
            width: image.frame.width,
            height: image.frame.height,
            depth_or_array_layers: 1,
        },
    );
    Ok(texture)
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

fn image_vertices(image: &RenderImage, width: f32, height: f32) -> Option<[ImageVertex; 6]> {
    let quad = image.visible_quad()?;
    let top_left = image.transform_point(quad.rect.x, quad.rect.y);
    let bottom_left = image.transform_point(quad.rect.x, quad.rect.y + quad.rect.height);
    let bottom_right = image.transform_point(
        quad.rect.x + quad.rect.width,
        quad.rect.y + quad.rect.height,
    );
    let top_right = image.transform_point(quad.rect.x + quad.rect.width, quad.rect.y);
    let top_left = normalized_point(top_left, width, height);
    let bottom_left = normalized_point(bottom_left, width, height);
    let bottom_right = normalized_point(bottom_right, width, height);
    let top_right = normalized_point(top_right, width, height);
    Some([
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
    ])
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
    @location(1) local: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) radii_top: vec4<f32>,
    @location(4) radii_bottom: vec4<f32>,
    @location(5) clip_local: vec2<f32>,
    @location(6) clip_size: vec2<f32>,
    @location(7) clip_radii_top: vec4<f32>,
    @location(8) clip_radii_bottom: vec4<f32>,
    @location(9) clip_params: vec2<f32>,
    @location(10) stroke_width: f32,
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) local: vec2<f32>,
    @location(3) size: vec2<f32>,
    @location(4) radii_top: vec4<f32>,
    @location(5) radii_bottom: vec4<f32>,
    @location(6) clip_local: vec2<f32>,
    @location(7) clip_size: vec2<f32>,
    @location(8) clip_radii_top: vec4<f32>,
    @location(9) clip_radii_bottom: vec4<f32>,
    @location(10) clip_params: vec2<f32>,
    @location(11) stroke_width: f32,
) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.local = local;
    out.size = size;
    out.radii_top = radii_top;
    out.radii_bottom = radii_bottom;
    out.clip_local = clip_local;
    out.clip_size = clip_size;
    out.clip_radii_top = clip_radii_top;
    out.clip_radii_bottom = clip_radii_bottom;
    out.clip_params = clip_params;
    out.stroke_width = stroke_width;
    return out;
}

fn ellipse_corner_alpha(local: vec2<f32>, center: vec2<f32>, radius: vec2<f32>) -> f32 {
    let safe_radius = max(radius, vec2<f32>(0.0001, 0.0001));
    let normalized = (local - center) / safe_radius;
    let signed_distance = (length(normalized) - 1.0) * min(safe_radius.x, safe_radius.y);
    return 1.0 - smoothstep(0.0, 1.0, signed_distance);
}

fn rounded_alpha(
    local: vec2<f32>,
    size: vec2<f32>,
    radii_top: vec4<f32>,
    radii_bottom: vec4<f32>,
) -> f32 {
    let safe_size = max(size, vec2<f32>(0.0001, 0.0001));
    if (local.x < 0.0 || local.y < 0.0 || local.x > safe_size.x || local.y > safe_size.y) {
        return 0.0;
    }
    let tl = radii_top.xy;
    let tr = radii_top.zw;
    let br = radii_bottom.xy;
    let bl = radii_bottom.zw;
    if (max(max(max(tl.x, tl.y), max(tr.x, tr.y)), max(max(br.x, br.y), max(bl.x, bl.y))) <= 0.0001) {
        return 1.0;
    }
    if (local.x < tl.x && local.y < tl.y) {
        return ellipse_corner_alpha(local, tl, tl);
    }
    if (local.x > safe_size.x - tr.x && local.y < tr.y) {
        return ellipse_corner_alpha(local, vec2<f32>(safe_size.x - tr.x, tr.y), tr);
    }
    if (local.x > safe_size.x - br.x && local.y > safe_size.y - br.y) {
        return ellipse_corner_alpha(local, safe_size - br, br);
    }
    if (local.x < bl.x && local.y > safe_size.y - bl.y) {
        return ellipse_corner_alpha(local, vec2<f32>(bl.x, safe_size.y - bl.y), bl);
    }
    return 1.0;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    var alpha = rounded_alpha(in.local, in.size, in.radii_top, in.radii_bottom);
    if (in.stroke_width > 0.0001) {
        let stroke_width = min(in.stroke_width, min(in.size.x, in.size.y) * 0.5);
        let inner_size = in.size - vec2<f32>(stroke_width * 2.0);
        var inner_alpha = 0.0;
        if (inner_size.x > 0.0001 && inner_size.y > 0.0001) {
            inner_alpha = rounded_alpha(
                in.local - vec2<f32>(stroke_width),
                inner_size,
                max(in.radii_top - vec4<f32>(stroke_width), vec4<f32>(0.0)),
                max(in.radii_bottom - vec4<f32>(stroke_width), vec4<f32>(0.0))
            );
        }
        alpha = max(alpha - inner_alpha, 0.0);
    }
    if (in.clip_params.x > 0.5) {
        alpha = alpha * rounded_alpha(
            in.clip_local,
            in.clip_size,
            in.clip_radii_top,
            in.clip_radii_bottom,
        );
    }
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
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
