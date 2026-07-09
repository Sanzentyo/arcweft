use crate::convert::{pixel_ceil_as_i32, pixel_floor_as_i32};
use crate::font_family::render_font_family;
use crate::font_system::{load_font_data_and_maybe_set_primary_sans, new_font_system};
use crate::geometry::{
    PaintRect, PreparedControlPaint, PreparedControlShadow, PreparedFrame, PreparedViewScene,
    RenderFontFamily, RenderGlyphMotion, RenderGlyphTransformSpan, RenderImage,
    RenderStyledParagraph, RenderStyledTextSpan, RenderTextBlock, RenderTextSlant, RenderTextStyle,
    RenderTextWeight, RuntimeControlBackdropSamplePolicy,
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
use arcweft_presentation::hit::HitRect;
use arcweft_render_text::RichTextRange;
use bytemuck::{Pod, Zeroable};
use glyphon::cosmic_text::Align;
use glyphon::{
    Attrs, Buffer, Cache, Color, FontSystem, Metrics, Resolution, Shaping, Style, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight,
};
use num_traits::ToPrimitive;
use std::borrow::Cow;
use std::ops::Range;
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
    aux_text_renderers: Vec<TextRenderer>,
    view_compositor: ViewCompositor,
    view_direct_renderer: WgpuViewDirectPrimitiveRenderer,
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
    #[error("view compositor failed: {0}")]
    ViewCompositor(#[from] ViewCompositorError),
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

struct TextRangeRenderRequest<'a> {
    target: &'a wgpu::TextureView,
    frame: &'a PreparedFrame,
    text_range: Range<usize>,
    styled_paragraph_range: Range<usize>,
    excluded_text_ranges: &'a [Range<usize>],
}

struct TextRenderState<'a> {
    font_system: &'a mut FontSystem,
    atlas: &'a mut TextAtlas,
    swash_cache: &'a mut SwashCache,
    viewport: &'a Viewport,
}

#[derive(Clone, Debug)]
struct MotionTextOverlay {
    text: String,
    left: f32,
    top: f32,
    clip_bounds: HitRect,
    style: RenderTextStyle,
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
        let view_compositor = ViewCompositor::new(device, queue, format);
        let view_direct_renderer = WgpuViewDirectPrimitiveRenderer::new(device, format);
        Self {
            format,
            rectangle_pipeline: rectangle_pipeline(device, format),
            image_bind_group_layout,
            image_pipeline,
            image_sampler,
            viewport: Viewport::new(device, &glyphon_cache),
            _glyphon_cache: glyphon_cache,
            font_system: new_font_system(),
            swash_cache: SwashCache::new(),
            atlas,
            text_renderer,
            aux_text_renderers: Vec::new(),
            view_compositor,
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
        let set_primary_sans = self.registered_font_bytes == 0;
        self.registered_font_bytes = self.registered_font_bytes.saturating_add(bytes.len());
        load_font_data_and_maybe_set_primary_sans(&mut self.font_system, bytes, set_primary_sans);
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
        let target_extent = ViewTextureExtent::new(
            frame.viewport.physical_width.max(1),
            frame.viewport.physical_height.max(1),
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

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("arcweft-shared-render-frame"),
        });
        self.render_background_and_images(device, queue, &mut encoder, &scene_view, frame);
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
        self.view_compositor
            .composite_texture_to_view(device, &mut encoder, &scene_view, target);
        queue.submit([encoder.finish()]);
        self.atlas.trim();
        Ok(())
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
            WgpuPreparedViewMaskTextureProvider::prepare(device, queue, &prepared_view.resources);
        let mut direct_renderer = self
            .view_direct_renderer
            .for_resources(&prepared_view.resources);
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
            direct_renderer: &mut direct_renderer,
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
    ) {
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
        self.aux_text_renderers.clear();
        let mut next_rectangle = 1;
        let styled_paragraphs = 0..frame.styled_paragraphs.len();
        let mut controls = frame.control_paints.iter().collect::<Vec<_>>();
        controls.sort_by_key(|paint| (paint.rectangle_range.start, paint.text_range.start));
        let filtered_text_ranges = controls
            .iter()
            .filter(|paint| {
                !slice_range(&frame.control_filters, paint.filter_range.clone()).is_empty()
            })
            .map(|paint| paint.text_range.clone())
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
        self.render_text_ranges(
            device,
            queue,
            encoder,
            TextRangeRenderRequest {
                target: target_view,
                frame,
                text_range: 0..frame.text.len(),
                styled_paragraph_range: styled_paragraphs,
                excluded_text_ranges: &filtered_text_ranges,
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
        self.aux_text_renderers.push(TextRenderer::new(
            &mut self.atlas,
            device,
            wgpu::MultisampleState::default(),
            None,
        ));
        let text_renderer = self
            .aux_text_renderers
            .last_mut()
            .expect("auxiliary text renderer was just pushed");
        render_text_ranges_with_renderer(
            device,
            queue,
            encoder,
            text_renderer,
            TextRenderState {
                font_system: &mut self.font_system,
                atlas: &mut self.atlas,
                swash_cache: &mut self.swash_cache,
                viewport: &self.viewport,
            },
            TextRangeRenderRequest {
                target: &control_view,
                frame,
                text_range: paint.text_range.clone(),
                styled_paragraph_range: 0..0,
                excluded_text_ranges: &[],
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

    fn render_text_ranges(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        request: TextRangeRenderRequest<'_>,
    ) -> Result<(), SharedRendererError> {
        render_text_ranges_with_renderer(
            device,
            queue,
            encoder,
            &mut self.text_renderer,
            TextRenderState {
                font_system: &mut self.font_system,
                atlas: &mut self.atlas,
                swash_cache: &mut self.swash_cache,
                viewport: &self.viewport,
            },
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
        let Some(vertices) = image_vertices(image, logical_width, logical_height) else {
            return;
        };
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

fn render_text_ranges_with_renderer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    text_renderer: &mut TextRenderer,
    state: TextRenderState<'_>,
    request: TextRangeRenderRequest<'_>,
) -> Result<(), SharedRendererError> {
    let TextRenderState {
        font_system,
        atlas,
        swash_cache,
        viewport,
    } = state;
    let text_range = request.text_range.clone();
    let text_blocks = slice_range(&request.frame.text, text_range.clone())
        .iter()
        .enumerate()
        .filter_map(|(offset, block)| {
            let index = text_range.start + offset;
            (!text_index_is_excluded(index, request.excluded_text_ranges)).then_some(block)
        })
        .collect::<Vec<_>>();
    let styled_paragraphs = slice_range(
        &request.frame.styled_paragraphs,
        request.styled_paragraph_range,
    );
    if text_blocks.is_empty() && styled_paragraphs.is_empty() {
        return Ok(());
    }
    let mut block_buffers = text_blocks
        .iter()
        .map(|&block| text_buffer(font_system, block))
        .collect::<Vec<_>>();
    let text_scale_factor = request.frame.viewport.physical_scale_factor_f32();
    let text_areas = block_buffers
        .iter_mut()
        .zip(text_blocks.iter().copied())
        .map(|(buffer, block)| text_area(buffer, block, text_scale_factor))
        .collect::<Vec<_>>();
    let paragraph_buffers = styled_paragraphs
        .iter()
        .map(|paragraph| styled_paragraph_render_buffer(font_system, paragraph))
        .collect::<Vec<_>>();
    let motion_overlays = styled_paragraphs
        .iter()
        .filter(|paragraph| !paragraph.glyph_transforms.is_empty())
        .flat_map(|paragraph| {
            let buffer = styled_paragraph_buffer(font_system, paragraph);
            styled_paragraph_motion_overlays(&buffer, paragraph)
        })
        .collect::<Vec<_>>();
    let mut motion_overlay_buffers = motion_overlays
        .iter()
        .map(|overlay| motion_overlay_buffer(font_system, overlay))
        .collect::<Vec<_>>();
    let paragraph_areas = paragraph_buffers
        .iter()
        .zip(styled_paragraphs)
        .map(|(buffer, paragraph)| styled_paragraph_text_area(buffer, paragraph, text_scale_factor))
        .collect::<Vec<_>>();
    let motion_overlay_areas = motion_overlay_buffers
        .iter_mut()
        .zip(&motion_overlays)
        .map(|(buffer, overlay)| motion_overlay_text_area(buffer, overlay, text_scale_factor))
        .collect::<Vec<_>>();
    let text_areas = text_areas
        .into_iter()
        .chain(paragraph_areas)
        .chain(motion_overlay_areas)
        .collect::<Vec<_>>();
    text_renderer
        .prepare(
            device,
            queue,
            font_system,
            atlas,
            viewport,
            text_areas,
            swash_cache,
        )
        .map_err(|error| SharedRendererError::TextPrepare(error.to_string()))
        .and_then(|()| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arcweft-shared-text-range-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: request.target,
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
            text_renderer
                .render(atlas, viewport, &mut pass)
                .map_err(|error| SharedRendererError::TextRender(error.to_string()))
        })
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
    [
        frame.viewport.logical_width.max(0.0001),
        frame.viewport.logical_height.max(0.0001),
    ]
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
        Some(Align::Left),
    );
    buffer.shape_until_scroll(font_system, false);
    buffer
}

fn styled_paragraph_buffer(
    font_system: &mut FontSystem,
    paragraph: &RenderStyledParagraph,
) -> Buffer {
    styled_paragraph_buffer_with_hidden_ranges(font_system, paragraph, &[])
}

fn styled_paragraph_render_buffer(
    font_system: &mut FontSystem,
    paragraph: &RenderStyledParagraph,
) -> Buffer {
    let hidden_ranges = paragraph
        .glyph_transforms
        .iter()
        .map(|transform| transform.range)
        .collect::<Vec<_>>();
    styled_paragraph_buffer_with_hidden_ranges(font_system, paragraph, &hidden_ranges)
}

fn styled_paragraph_buffer_with_hidden_ranges(
    font_system: &mut FontSystem,
    paragraph: &RenderStyledParagraph,
    hidden_ranges: &[RichTextRange],
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
    let spans = styled_paragraph_attr_spans(paragraph, hidden_ranges);
    buffer.set_rich_text(
        font_system,
        spans,
        &default_attrs,
        Shaping::Advanced,
        Some(Align::Left),
    );
    buffer.shape_until_scroll(font_system, false);
    buffer
}

fn styled_paragraph_attr_spans<'a>(
    paragraph: &'a RenderStyledParagraph,
    hidden_ranges: &[RichTextRange],
) -> Vec<(&'a str, Attrs<'a>)> {
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
                hidden_ranges,
            );
        }
        if start < end {
            push_revealed_attr_span(
                &mut output,
                paragraph,
                start,
                end,
                &span.style,
                hidden_ranges,
            );
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
            hidden_ranges,
        );
    }
    if output.is_empty() {
        push_revealed_attr_span(
            &mut output,
            paragraph,
            0,
            paragraph.text.len(),
            &paragraph.default_style,
            hidden_ranges,
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
    hidden_ranges: &[RichTextRange],
) {
    if start >= end {
        return;
    }
    let reveal = paragraph.reveal.visible_end.min(paragraph.text.len());
    let mut points = vec![start, end];
    if start < reveal && reveal < end {
        points.push(reveal);
    }
    for range in hidden_ranges {
        let hidden_start = range.start.max(start).min(end);
        let hidden_end = range.end.max(start).min(end);
        if hidden_start < hidden_end {
            points.push(hidden_start);
            points.push(hidden_end);
        }
    }
    points.sort_unstable();
    points.dedup();

    for pair in points.windows(2) {
        let segment_start = pair[0];
        let segment_end = pair[1];
        let hidden = hidden_ranges
            .iter()
            .any(|range| range.start < segment_end && segment_start < range.end);
        let alpha = if hidden || segment_start >= reveal {
            TRANSPARENT_ALPHA
        } else {
            style.color[3]
        };
        push_attr_span(output, paragraph, segment_start, segment_end, style, alpha);
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

fn styled_paragraph_motion_overlays(
    buffer: &Buffer,
    paragraph: &RenderStyledParagraph,
) -> Vec<MotionTextOverlay> {
    if paragraph.glyph_transforms.is_empty() {
        return Vec::new();
    }
    let visible_end = paragraph.reveal.visible_end.min(paragraph.text.len());
    let seconds = visual_seconds(paragraph.visual_time_millis);
    let line_starts = text_line_start_offsets(&paragraph.text);
    buffer
        .layout_runs()
        .flat_map(|run| {
            let source_line_start = line_starts.get(run.line_i).copied().unwrap_or_default();
            run.glyphs.iter().filter_map(move |glyph| {
                let range = RichTextRange::new(
                    source_line_start
                        .saturating_add(glyph.start)
                        .min(paragraph.text.len()),
                    source_line_start
                        .saturating_add(glyph.end)
                        .min(paragraph.text.len()),
                );
                if range.start >= visible_end || range.end > visible_end {
                    return None;
                }
                let transform = paragraph
                    .glyph_transforms
                    .iter()
                    .find(|transform| ranges_intersect(transform.range, range))?;
                let text = paragraph.text.get(range.start..range.end)?;
                if text.trim().is_empty() {
                    return None;
                }
                let offset_y = transform.motion.offset_y(seconds, range.start);
                Some(MotionTextOverlay {
                    text: text.to_owned(),
                    left: paragraph.bounds.x + glyph.x,
                    top: paragraph.bounds.y + run.line_top + offset_y,
                    clip_bounds: paragraph.bounds,
                    style: style_for_source_range(paragraph, range).clone(),
                })
            })
        })
        .collect()
}

fn motion_overlay_buffer(font_system: &mut FontSystem, overlay: &MotionTextOverlay) -> Buffer {
    let mut buffer = Buffer::new(
        font_system,
        Metrics::new(overlay.style.font_size, overlay.style.line_height),
    );
    buffer.set_size(
        font_system,
        Some(overlay.clip_bounds.width),
        Some(overlay.clip_bounds.height),
    );
    buffer.set_text(
        font_system,
        &overlay.text,
        &attrs_from_style(&overlay.style),
        Shaping::Advanced,
        Some(Align::Left),
    );
    buffer.shape_until_scroll(font_system, false);
    buffer
}

fn motion_overlay_text_area<'a>(
    buffer: &'a Buffer,
    overlay: &MotionTextOverlay,
    scale_factor: f32,
) -> TextArea<'a> {
    let scale_factor = scale_factor.max(f32::EPSILON);
    TextArea {
        buffer,
        left: overlay.left * scale_factor,
        top: overlay.top * scale_factor,
        scale: scale_factor,
        bounds: scale_text_bounds(overlay.clip_bounds, scale_factor),
        default_color: Color::rgba(
            overlay.style.color[0],
            overlay.style.color[1],
            overlay.style.color[2],
            overlay.style.color[3],
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
    registered_font_bytes: usize,
}

impl StyledParagraphEvidenceFontContext {
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_system: new_font_system(),
            registered_font_bytes: 0,
        }
    }

    pub fn register_font_bytes(&mut self, bytes: Vec<u8>) -> Result<(), SharedRendererError> {
        if bytes.is_empty() {
            return Err(SharedRendererError::EmptyFont);
        }
        let set_primary_sans = self.registered_font_bytes == 0;
        self.registered_font_bytes = self.registered_font_bytes.saturating_add(bytes.len());
        load_font_data_and_maybe_set_primary_sans(&mut self.font_system, bytes, set_primary_sans);
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
    Rendered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyledParagraphGlyphTransformRenderSupport {
    MetadataOnlyUnsupported,
    Rendered,
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
    let line_starts = text_line_start_offsets(&paragraph.text);
    let mut line_boxes = Vec::new();
    let mut glyph_bounds = Vec::new();
    for (line_index, run) in buffer.layout_runs().enumerate() {
        let source_line_start = line_starts.get(run.line_i).copied().unwrap_or_default();
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
            let source_range = RichTextRange::new(
                source_line_start
                    .saturating_add(glyph.start)
                    .min(paragraph.text.len()),
                source_line_start
                    .saturating_add(glyph.end)
                    .min(paragraph.text.len()),
            );
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
            StyledParagraphTransformSupport::Rendered
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
            rendered: true,
            render_support: StyledParagraphGlyphTransformRenderSupport::Rendered,
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
        rendered: true,
        render_support: StyledParagraphGlyphTransformRenderSupport::Rendered,
    }
}

fn ranges_intersect(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

fn text_line_start_offsets(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.char_indices().filter_map(|(index, character)| {
            (character == '\n').then_some(index + character.len_utf8())
        }))
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{RenderGlyphTransformKind, RenderTextReveal, RenderTextSelectionPolicy};

    #[test]
    fn plain_text_block_spacing_stays_compact_in_wide_buffer() {
        let mut font_system = new_font_system();
        let block = RenderTextBlock {
            target: None,
            text: "Alpha beta".to_owned(),
            bounds: HitRect::new(0.0, 0.0, 400.0, 40.0),
            clip_bounds: None,
            buffer_width: Some(400.0),
            buffer_height: Some(40.0),
            font_size: 20.0,
            line_height: 24.0,
            font_family: RenderFontFamily::SansSerif,
            weight: RenderTextWeight::Regular,
            slant: RenderTextSlant::Upright,
            rgba: [255, 255, 255, 255],
            selection_policy: RenderTextSelectionPolicy::Disabled,
            selection: None,
            selection_rgba: [0.0, 0.0, 0.0, 0.0],
        };

        let buffer = text_buffer(&mut font_system, &block);
        let right_edge = layout_text_right_edge(&buffer);

        assert!(right_edge > 20.0, "text did not produce visible glyphs");
        assert!(
            right_edge < 220.0,
            "text layout should not stretch word spacing across the full buffer: {right_edge}"
        );
    }

    #[test]
    fn styled_paragraph_spacing_stays_compact_in_wide_buffer() {
        let mut font_system = new_font_system();
        let paragraph = RenderStyledParagraph {
            text: "Alpha beta".to_owned(),
            bounds: HitRect::new(0.0, 0.0, 400.0, 40.0),
            default_style: RenderTextStyle {
                font_size: 20.0,
                line_height: 24.0,
                color: [255, 255, 255, 255],
                font_family: RenderFontFamily::SansSerif,
                weight: RenderTextWeight::Regular,
                slant: RenderTextSlant::Upright,
            },
            spans: Vec::new(),
            reveal: RenderTextReveal { visible_end: 10 },
            glyph_transforms: Vec::new(),
            visual_time_millis: 0,
        };

        let buffer = styled_paragraph_buffer(&mut font_system, &paragraph);
        let right_edge = layout_text_right_edge(&buffer);

        assert!(right_edge > 20.0, "text did not produce visible glyphs");
        assert!(
            right_edge < 220.0,
            "styled paragraph layout should not stretch word spacing across the full buffer: {right_edge}"
        );
    }

    #[test]
    fn motion_overlay_keeps_transformed_text_after_hard_break() {
        let mut font_system = new_font_system();
        let text = "Captured the view-backed brief.\nIdea42".to_owned();
        let brief_start = "Captured the view-backed brief.\n".len();
        let brief_end = text.len();
        let brief_style = RenderTextStyle {
            font_size: 38.0,
            line_height: 51.3,
            color: [255, 64, 80, 255],
            font_family: RenderFontFamily::SansSerif,
            weight: RenderTextWeight::Bold,
            slant: RenderTextSlant::Italic,
        };
        let paragraph = RenderStyledParagraph {
            text,
            bounds: HitRect::new(32.0, 300.0, 760.0, 180.0),
            default_style: RenderTextStyle {
                font_size: 25.0,
                line_height: 34.0,
                color: [255, 255, 255, 255],
                font_family: RenderFontFamily::SansSerif,
                weight: RenderTextWeight::Regular,
                slant: RenderTextSlant::Upright,
            },
            spans: vec![RenderStyledTextSpan {
                range: RichTextRange::new(brief_start, brief_end),
                style: brief_style,
                node_index: 2,
            }],
            reveal: RenderTextReveal {
                visible_end: brief_end,
            },
            glyph_transforms: vec![RenderGlyphTransformSpan {
                range: RichTextRange::new(brief_start, brief_end),
                motion: RenderGlyphMotion {
                    kind: RenderGlyphTransformKind::Wave,
                    amplitude: 5.0,
                    frequency: 7.0,
                },
                node_index: 2,
            }],
            visual_time_millis: 1_000,
        };

        let layout_buffer = styled_paragraph_buffer(&mut font_system, &paragraph);
        let overlays = styled_paragraph_motion_overlays(&layout_buffer, &paragraph);
        let overlay_text = overlays
            .iter()
            .map(|overlay| overlay.text.as_str())
            .collect::<String>();

        assert_eq!(overlay_text, "Idea42");
        assert!(overlays.iter().all(|overlay| {
            overlay.top > paragraph.bounds.y + paragraph.default_style.line_height * 0.5
        }));
    }

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

    fn layout_text_right_edge(buffer: &Buffer) -> f32 {
        buffer
            .layout_runs()
            .flat_map(|run| run.glyphs.iter())
            .map(|glyph| glyph.x + glyph.w)
            .fold(0.0_f32, f32::max)
    }
}
