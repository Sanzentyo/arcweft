use super::{
    ActiveEventLoop, Affine2, Application, ApplicationHandler, BTreeMap, BindGroupDescriptor,
    BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType,
    BlendState, Buffer, BufferDescriptor, BufferUsages, COPY_BYTES_PER_ROW_ALIGNMENT, Cache, Color,
    ColorTargetState, ColorWrites, CommandEncoderDescriptor, DeviceDescriptor, Extent3d,
    FeatureTag, FilterMode, FontFeatures, FontSystem, FragmentState, GlyphInstance,
    GlyphOrientation, GlyphTransform, GlyphVerticalForm, GlyphonAreaOptions, ImageAlignment,
    ImageFit, Instance, Key, KeyEvent, LaidOutGlyph, LaidOutText, LayoutBox, LayoutRect, LoadOp,
    LogicalSize, MapMode, Metrics, Milli, MultisampleState, NATIVE_GLYPHAREA_BASELINE_OFFSET,
    NativeEffectExecution, NativeFrameContentBBox, NativeGlyphPlacement, NativeImageDebugQuad,
    NativeImageQuad, NativeImageRect, NativeImageTransform, NativePageLayout, NativeTextOrigin,
    NativeTextStyle, NativeWindowError, Operations, Origin3d, OwnedGlyphArea,
    PipelineCompilationOptions, PipelineLayoutDescriptor, Point, PollType, PrimitiveState,
    PrimitiveTopology, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
    RequestAdapterOptions, Resolution, ResolvedGlyph, RichTextEffectDescriptor,
    RichTextEffectTarget, RichTextPresentation, RichTextRange, RichTextTransformOrigin,
    RichTextWritingMode, RubyGlyphPlacement, SamplerBindingType, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, Shaping, SwashCache, TexelCopyBufferInfo,
    TexelCopyBufferLayout, TexelCopyTextureInfo, TextArea, TextAtlas, TextBounds, TextLayoutConfig,
    TextRenderer, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureViewDescriptor, TextureViewDimension, Vector,
    VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
    VerticalGlyphHorizontalAlign, Viewport, Window, WindowAttributes, WindowEvent, WindowPage,
    WindowRichText, WindowRubyBuffer, WindowState, apply_presentation_effects_to_placement,
    apply_presentation_effects_to_placement_with_execution, build_ruby_buffers,
    effect_applies_to_glyph_mask, glyph_area_from_layout, glyph_orientation_degrees,
    horizontal_glyph_area_from_shaped_buffer, mpsc, native_glyph_placements_for_layout,
    native_glyph_placements_for_layout_with_effects, observe_layout_shaders, param_bool,
    param_milli, shader_glyph_areas_for_ruby, shader_glyph_areas_for_text, text_line_start_offsets,
    vertical_glyph_area_from_shaped_buffer,
};
use std::sync::Arc;
use winit::keyboard::NamedKey;

impl Application {
    pub(super) fn current_page(&self) -> &WindowPage {
        &self.pages[self.page_index]
    }

    pub(super) fn advance_page(&mut self) -> Option<WindowPage> {
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

    fn about_to_wait(&mut self, _event_loop: &dyn ActiveEventLoop) {
        if let Some(state) = self.window_state.as_ref()
            && state.has_timed_effects
        {
            state.window.request_redraw();
        }
    }
}

pub(super) fn key_advances_page(event: &KeyEvent) -> bool {
    if !event.state.is_pressed() || event.repeat {
        return false;
    }
    match event.key_without_modifiers.as_ref() {
        Key::Named(NamedKey::Enter) => true,
        Key::Character(value) => value == " " || value.eq_ignore_ascii_case("n"),
        _ => false,
    }
}

pub(super) fn key_closes_window(event: &KeyEvent) -> bool {
    event.state.is_pressed()
        && matches!(
            event.key_without_modifiers.as_ref(),
            Key::Named(NamedKey::Escape)
        )
}

pub(super) async fn request_capture_device()
-> Result<(wgpu::Device, wgpu::Queue), NativeWindowError> {
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

pub(super) struct NativeOffscreenTextRenderer {
    pub(super) font_system: FontSystem,
    pub(super) swash_cache: SwashCache,
    pub(super) viewport: Viewport,
    pub(super) atlas: TextAtlas,
    pub(super) text_renderer: TextRenderer,
    pub(super) text_buffer: Buffer,
    pub(super) ruby_buffers: Vec<WindowRubyBuffer>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NativeRenderTarget {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) origin: NativeTextOrigin,
    pub(super) time_seconds: f32,
    pub(super) force_alpha_mask: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NativeRenderLayout<'a> {
    pub(super) layout: &'a LaidOutText,
}

impl<'a> NativeRenderLayout<'a> {
    pub(super) const fn glyph_area(layout: &'a LaidOutText) -> Self {
        Self { layout }
    }
}

impl NativeOffscreenTextRenderer {
    pub(super) fn new(
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

    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rich_text: &WindowRichText,
        layout: NativeRenderLayout<'_>,
        target: NativeRenderTarget,
        mut effects: Option<&mut NativeEffectExecution<'_>>,
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
        let mut glyph_area = self.prepare_glyph_area(rich_text, layout, target)?;
        apply_text_colors_to_glyph_area(
            &mut glyph_area,
            rich_text,
            layout.layout,
            target.time_seconds,
        );
        if let Some(effects) = effects.as_deref_mut() {
            observe_layout_shaders(
                effects,
                layout.layout,
                rich_text
                    .ruby_annotations
                    .iter()
                    .map(|ruby| &ruby.presentation),
            );
            apply_text_transforms_to_glyph_area_with_effects(
                &mut glyph_area,
                &rich_text.text,
                layout.layout,
                target.time_seconds,
                effects,
            );
        } else {
            apply_text_transforms_to_glyph_area(
                &mut glyph_area,
                &rich_text.text,
                layout.layout,
                target.time_seconds,
            );
        }
        let text_shader_glyph_areas = effects.as_deref_mut().map_or_else(Vec::new, |effects| {
            shader_glyph_areas_for_text(&glyph_area, layout.layout, effects)
        });
        let ruby_glyph_areas = ruby_glyph_areas(
            &self.ruby_buffers,
            &rich_text.text,
            target.width,
            target.height,
            target.time_seconds,
            target.force_alpha_mask,
            effects.as_deref_mut(),
        );
        let ruby_shader_glyph_areas = effects.map_or_else(Vec::new, |effects| {
            shader_glyph_areas_for_ruby(&ruby_glyph_areas, &self.ruby_buffers, effects)
        });
        let glyph_areas = native_glyph_area_submission_list(
            &text_shader_glyph_areas,
            &glyph_area,
            &ruby_shader_glyph_areas,
            &ruby_glyph_areas,
        );
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

    pub(super) fn prepare_glyph_area(
        &mut self,
        rich_text: &WindowRichText,
        layout: NativeRenderLayout<'_>,
        target: NativeRenderTarget,
    ) -> Result<OwnedGlyphArea, NativeWindowError> {
        let cache_keys = layout_glyph_cache_keys(
            &mut self.font_system,
            &self.text_buffer,
            rich_text,
            layout.layout,
        );
        let mut glyph_area = glyph_area_from_layout(
            layout.layout,
            GlyphonAreaOptions {
                bounds: native_text_bounds(target.width, target.height),
                origin_offset: Vector::new(0.0, NATIVE_GLYPHAREA_BASELINE_OFFSET),
                force_alpha_mask: target.force_alpha_mask,
                ..GlyphonAreaOptions::default()
            },
            |index, glyph| cache_keys_for_layout_glyph(index, glyph.range, &cache_keys),
        )
        .map_err(|error| NativeWindowError::Readback(error.to_string()))?;
        apply_shaped_horizontal_origins_to_glyph_area(&mut glyph_area, layout.layout, &cache_keys);
        Ok(glyph_area)
    }

    pub(super) fn render_texture_with_clear(
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

pub(super) const IMAGE_QUAD_SHADER: &str = r"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    return output;
}

@group(0) @binding(0)
var image_texture: texture_2d<f32>;

@group(0) @binding(1)
var image_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(image_texture, image_sampler, input.uv);
}
";

pub(super) fn render_image_quads_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    quads: &[NativeImageQuad<'_>],
    width: u32,
    height: u32,
    format: TextureFormat,
    clear: wgpu::Color,
) -> Result<wgpu::Texture, NativeWindowError> {
    for quad in quads {
        validate_native_image_quad(*quad)?;
    }

    let output = create_image_output_texture(device, width, height, format);
    let output_view = output.create_view(&TextureViewDescriptor::default());
    let (bind_group_layout, pipeline, sampler) = create_image_quad_pipeline(device, format);
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("arcweft native image capture encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("arcweft native image capture pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &output_view,
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
        pass.set_pipeline(&pipeline);
        for quad in quads {
            let texture = upload_native_image_quad(device, queue, *quad);
            let texture_view = texture.create_view(&TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("arcweft native image bind group"),
                layout: &bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&sampler),
                    },
                ],
            });
            let vertices = image_quad_vertices(*quad, width, height);
            let vertex_bytes = bytemuck::cast_slice(&vertices);
            let vertex_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("arcweft native image vertex buffer"),
                size: u64::try_from(vertex_bytes.len()).map_err(|_| {
                    NativeWindowError::Image("image vertex buffer is too large".to_owned())
                })?,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&vertex_buffer, 0, vertex_bytes);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..6, 0..1);
        }
    }
    queue.submit(Some(encoder.finish()));
    Ok(output)
}

pub(super) fn create_image_output_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&TextureDescriptor {
        label: Some("arcweft native image capture texture"),
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
    })
}

pub(super) fn create_image_quad_pipeline(
    device: &wgpu::Device,
    format: TextureFormat,
) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline, wgpu::Sampler) {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("arcweft native image quad shader"),
        source: ShaderSource::Wgsl(IMAGE_QUAD_SHADER.into()),
    });
    let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("arcweft native image bind group layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("arcweft native image pipeline layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let vertex_layout = VertexBufferLayout {
        array_stride: 16,
        step_mode: VertexStepMode::Vertex,
        attributes: &[
            VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
        ],
    };
    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("arcweft native image pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[vertex_layout],
            compilation_options: PipelineCompilationOptions::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::default(),
            })],
            compilation_options: PipelineCompilationOptions::default(),
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("arcweft native image sampler"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..Default::default()
    });
    (bind_group_layout, pipeline, sampler)
}

pub(super) fn upload_native_image_quad(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    quad: NativeImageQuad<'_>,
) -> wgpu::Texture {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("arcweft native image source texture"),
        size: Extent3d {
            width: quad.width,
            height: quad.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let upload_rgba = image_quad_upload_rgba(quad);
    queue.write_texture(
        TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        upload_rgba.as_ref(),
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(quad.width.saturating_mul(4)),
            rows_per_image: Some(quad.height),
        },
        Extent3d {
            width: quad.width,
            height: quad.height,
            depth_or_array_layers: 1,
        },
    );
    texture
}

pub(super) fn image_quad_upload_rgba(quad: NativeImageQuad<'_>) -> std::borrow::Cow<'_, [u8]> {
    if quad.opacity_milli >= 1_000 {
        return std::borrow::Cow::Borrowed(quad.rgba);
    }
    let rgba = quad
        .rgba
        .chunks_exact(4)
        .flat_map(|pixel| {
            [
                pixel[0],
                pixel[1],
                pixel[2],
                scaled_alpha_milli(pixel[3], quad.opacity_milli),
            ]
        })
        .collect::<Vec<_>>();
    std::borrow::Cow::Owned(rgba)
}

pub(super) fn native_image_rect_for_layout(
    layout: LayoutBox,
    fit: ImageFit,
    alignment: ImageAlignment,
    image_width: u32,
    image_height: u32,
) -> Result<NativeImageRect, NativeWindowError> {
    let origin = (
        layout_milli_to_pixel(layout.origin.x.0),
        layout_milli_to_pixel(layout.origin.y.0),
    );
    let outer = (
        layout_milli_to_pixel(layout.size.width.0),
        layout_milli_to_pixel(layout.size.height.0),
    );
    let intrinsic = (u32_to_f32(image_width), u32_to_f32(image_height));
    if outer.0 <= 0.0 || outer.1 <= 0.0 {
        return Err(NativeWindowError::Image(
            "image layout dimensions must be positive".to_owned(),
        ));
    }
    if intrinsic.0 <= 0.0 || intrinsic.1 <= 0.0 {
        return Err(NativeWindowError::Image(
            "image frame dimensions must be positive".to_owned(),
        ));
    }

    let fitted = match fit {
        ImageFit::Stretch => outer,
        ImageFit::Intrinsic => intrinsic,
        ImageFit::Contain => fit_preserving_aspect(outer, intrinsic, AspectFitMode::Contain),
        ImageFit::Cover => fit_preserving_aspect(outer, intrinsic, AspectFitMode::Cover),
    };
    let offset = (
        alignment_offset(outer.0, fitted.0, alignment.x_milli()),
        alignment_offset(outer.1, fitted.1, alignment.y_milli()),
    );

    Ok(NativeImageRect {
        x: origin.0 + offset.0,
        y: origin.1 + offset.1,
        width: fitted.0,
        height: fitted.1,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AspectFitMode {
    Contain,
    Cover,
}

pub(super) fn fit_preserving_aspect(
    outer: (f32, f32),
    intrinsic: (f32, f32),
    mode: AspectFitMode,
) -> (f32, f32) {
    let scale = match mode {
        AspectFitMode::Contain => (outer.0 / intrinsic.0).min(outer.1 / intrinsic.1),
        AspectFitMode::Cover => (outer.0 / intrinsic.0).max(outer.1 / intrinsic.1),
    };
    (intrinsic.0 * scale, intrinsic.1 * scale)
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn layout_milli_to_pixel(value: i32) -> f32 {
    value as f32 / 1_000.0
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn u32_to_f32(value: u32) -> f32 {
    value as f32
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn native_image_transform_milli(values: [i32; 6]) -> NativeImageTransform {
    let [m11, m12, m21, m22, translate_x, translate_y] = values;
    NativeImageTransform {
        m11: m11 as f32 / 1_000.0,
        m12: m12 as f32 / 1_000.0,
        m21: m21 as f32 / 1_000.0,
        m22: m22 as f32 / 1_000.0,
        tx: translate_x as f32 / 1_000.0,
        ty: translate_y as f32 / 1_000.0,
    }
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn alignment_offset(outer: f32, fitted: f32, alignment_milli: i32) -> f32 {
    (outer - fitted) * (alignment_milli as f32 / 1_000.0)
}

pub(super) fn validate_native_image_quad(
    quad: NativeImageQuad<'_>,
) -> Result<(), NativeWindowError> {
    if quad.width == 0 || quad.height == 0 {
        return Err(NativeWindowError::Image(
            "image quad dimensions must be non-zero".to_owned(),
        ));
    }
    if quad.dst.width <= 0.0 || quad.dst.height <= 0.0 {
        return Err(NativeWindowError::Image(
            "image quad destination dimensions must be positive".to_owned(),
        ));
    }
    let expected = usize::try_from(u64::from(quad.width) * u64::from(quad.height) * 4)
        .map_err(|_| NativeWindowError::Image("image quad is too large".to_owned()))?;
    if quad.rgba.len() != expected {
        return Err(NativeWindowError::Image(format!(
            "image quad RGBA length {} does not match expected {expected}",
            quad.rgba.len()
        )));
    }
    Ok(())
}

pub(super) fn recolor_image_debug_quad(
    quad: NativeImageDebugQuad<'_>,
) -> Result<Vec<u8>, NativeWindowError> {
    validate_native_image_quad(quad.quad)?;
    let mut rgba = Vec::with_capacity(quad.quad.rgba.len());
    for pixel in quad.quad.rgba.chunks_exact(4) {
        let source_alpha = scaled_alpha_milli(pixel[3], quad.quad.opacity_milli);
        if source_alpha == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            rgba.extend_from_slice(&[
                quad.color[0],
                quad.color[1],
                quad.color[2],
                scaled_alpha_u8(quad.color[3], source_alpha),
            ]);
        }
    }
    Ok(rgba)
}

pub(super) fn scaled_alpha_milli(source_alpha: u8, opacity_milli: u16) -> u8 {
    let opacity = opacity_milli.min(1_000);
    let value = u32::from(source_alpha).saturating_mul(u32::from(opacity)) / 1_000;
    u8::try_from(value).unwrap_or(u8::MAX)
}

pub(super) fn scaled_alpha_u8(color_alpha: u8, source_alpha: u8) -> u8 {
    let value = u16::from(color_alpha).saturating_mul(u16::from(source_alpha)) / 255;
    u8::try_from(value).unwrap_or(u8::MAX)
}

pub(super) fn image_quad_vertices(
    quad: NativeImageQuad<'_>,
    width: u32,
    height: u32,
) -> [[f32; 4]; 6] {
    let p0 = transform_image_point(quad.transform, quad.dst.x, quad.dst.y);
    let p1 = transform_image_point(quad.transform, quad.dst.x + quad.dst.width, quad.dst.y);
    let p2 = transform_image_point(
        quad.transform,
        quad.dst.x + quad.dst.width,
        quad.dst.y + quad.dst.height,
    );
    let p3 = transform_image_point(quad.transform, quad.dst.x, quad.dst.y + quad.dst.height);
    let x0 = pixel_x_to_ndc(p0.0, width);
    let y0 = pixel_y_to_ndc(p0.1, height);
    let x1 = pixel_x_to_ndc(p1.0, width);
    let y1 = pixel_y_to_ndc(p1.1, height);
    let x2 = pixel_x_to_ndc(p2.0, width);
    let y2 = pixel_y_to_ndc(p2.1, height);
    let x3 = pixel_x_to_ndc(p3.0, width);
    let y3 = pixel_y_to_ndc(p3.1, height);
    [
        [x0, y0, 0.0, 0.0],
        [x1, y1, 1.0, 0.0],
        [x2, y2, 1.0, 1.0],
        [x0, y0, 0.0, 0.0],
        [x2, y2, 1.0, 1.0],
        [x3, y3, 0.0, 1.0],
    ]
}

pub(super) fn transform_image_point(transform: NativeImageTransform, x: f32, y: f32) -> (f32, f32) {
    (
        transform
            .m11
            .mul_add(x, transform.m12.mul_add(y, transform.tx)),
        transform
            .m21
            .mul_add(x, transform.m22.mul_add(y, transform.ty)),
    )
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn pixel_x_to_ndc(x: f32, width: u32) -> f32 {
    (x / width.max(1) as f32) * 2.0 - 1.0
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn pixel_y_to_ndc(y: f32, height: u32) -> f32 {
    1.0 - (y / height.max(1) as f32) * 2.0
}

pub(super) fn readback_texture_rgba(
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

pub(super) fn prepare_window_text_buffers(
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

pub(super) fn window_text_areas<'a>(
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

pub(super) fn ruby_glyph_areas(
    ruby_buffers: &[WindowRubyBuffer],
    line_key: &str,
    width: u32,
    height: u32,
    time_seconds: f32,
    force_alpha_mask: bool,
    mut effects: Option<&mut NativeEffectExecution<'_>>,
) -> Vec<OwnedGlyphArea> {
    let bounds = native_text_bounds(width, height);
    ruby_buffers
        .iter()
        .map(|ruby| {
            let mut area = match ruby.placement {
                RubyGlyphPlacement::Horizontal { line_height } => {
                    horizontal_glyph_area_from_shaped_buffer(
                        &ruby.buffer,
                        ruby_glyph_area_options(bounds, ruby.left, ruby.top, force_alpha_mask),
                        line_height,
                    )
                }
                RubyGlyphPlacement::Vertical {
                    cell_width,
                    vertical_advance,
                    horizontal_align,
                } => vertical_glyph_area_from_shaped_buffer(
                    &ruby.buffer,
                    ruby_glyph_area_options(bounds, ruby.left, ruby.top, force_alpha_mask),
                    cell_width,
                    vertical_advance,
                    horizontal_align,
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
            if let Some(effects) = effects.as_deref_mut() {
                apply_ruby_transforms_to_glyph_area_with_effects(
                    &mut area,
                    line_key,
                    ruby,
                    time_seconds,
                    effects,
                );
            } else {
                apply_ruby_transforms_to_glyph_area(&mut area, line_key, ruby, time_seconds);
            }
            area
        })
        .collect()
}

pub(super) fn apply_ruby_transforms_to_glyph_area(
    glyph_area: &mut OwnedGlyphArea,
    line_key: &str,
    ruby: &WindowRubyBuffer,
    time_seconds: f32,
) {
    apply_ruby_transforms_to_glyph_area_inner(glyph_area, line_key, ruby, time_seconds, None);
}

pub(super) fn apply_ruby_transforms_to_glyph_area_with_effects(
    glyph_area: &mut OwnedGlyphArea,
    line_key: &str,
    ruby: &WindowRubyBuffer,
    time_seconds: f32,
    effects: &mut NativeEffectExecution<'_>,
) {
    apply_ruby_transforms_to_glyph_area_inner(
        glyph_area,
        line_key,
        ruby,
        time_seconds,
        Some(effects),
    );
}

pub(super) fn apply_ruby_transforms_to_glyph_area_inner(
    glyph_area: &mut OwnedGlyphArea,
    line_key: &str,
    ruby: &WindowRubyBuffer,
    time_seconds: f32,
    mut effects: Option<&mut NativeEffectExecution<'_>>,
) {
    let glyph_count = glyph_area.len().max(1);
    for (glyph_index, instance) in glyph_area.glyphs_mut().iter_mut().enumerate() {
        let original_origin = instance.origin;
        let mut placement = NativeGlyphPlacement {
            run_index: ruby.source_index,
            glyph_index,
            range: glyph_index..glyph_index + 1,
            x: original_origin.x,
            y: original_origin.y,
            rotate_degrees: 0.0,
            skew_x_degrees: 0.0,
            skew_y_degrees: 0.0,
            affine_origin: None,
            affine_target: None,
            vertical_form: GlyphVerticalForm::None,
            scale_x: 1.0,
            scale_y: 1.0,
            opacity: 1.0,
            color: None,
        };
        if let Some(effects) = effects.as_deref_mut() {
            apply_presentation_effects_to_placement_with_execution(
                line_key,
                &ruby.presentation,
                glyph_count,
                time_seconds,
                effects,
                &mut placement,
            );
        } else {
            apply_presentation_effects_to_placement(
                line_key,
                &ruby.presentation,
                glyph_count,
                time_seconds,
                &mut placement,
            );
        }

        let affine = ruby_glyph_presentation_affine(&placement, &ruby.presentation, instance);
        instance.origin = Point::new(placement.x, placement.y);
        if let Some(affine) = affine {
            let current = glyph_transform_affine(instance.transform);
            instance.transform =
                GlyphTransform::Affine(Affine2::new(compose_affine(affine, current)));
        }
        apply_placement_color_override(instance, &placement);
    }
}

pub(super) fn vertical_ruby_glyph_horizontal_align(
    segment: &arcweft_text_layout::LaidOutRuby,
) -> VerticalGlyphHorizontalAlign {
    let ruby_center = segment.ruby_bounds.x + segment.ruby_bounds.width * 0.5;
    let base_center = segment.base_bounds.x + segment.base_bounds.width * 0.5;
    if ruby_center > base_center {
        VerticalGlyphHorizontalAlign::Start
    } else if ruby_center < base_center {
        VerticalGlyphHorizontalAlign::End
    } else {
        VerticalGlyphHorizontalAlign::Center
    }
}

pub(super) fn ruby_glyph_area_options(
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

pub(super) fn native_glyph_area_submission_list<'a>(
    text_shader_glyph_areas: &'a [OwnedGlyphArea],
    text_glyph_area: &'a OwnedGlyphArea,
    ruby_shader_glyph_areas: &'a [OwnedGlyphArea],
    ruby_glyph_areas: &'a [OwnedGlyphArea],
) -> Vec<glyphon::GlyphArea<'a>> {
    let mut glyph_areas = Vec::with_capacity(
        text_shader_glyph_areas
            .len()
            .saturating_add(ruby_shader_glyph_areas.len())
            .saturating_add(1)
            .saturating_add(ruby_glyph_areas.len()),
    );
    glyph_areas.extend(
        text_shader_glyph_areas
            .iter()
            .map(OwnedGlyphArea::as_glyph_area),
    );
    glyph_areas.push(text_glyph_area.as_glyph_area());
    glyph_areas.extend(
        ruby_shader_glyph_areas
            .iter()
            .map(OwnedGlyphArea::as_glyph_area),
    );
    glyph_areas.extend(ruby_glyph_areas.iter().map(OwnedGlyphArea::as_glyph_area));
    glyph_areas
}

pub(super) fn native_text_bounds(width: u32, height: u32) -> TextBounds {
    TextBounds {
        left: 0,
        top: 0,
        right: surface_extent_i32(width),
        bottom: surface_extent_i32(height),
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct LayoutGlyphCacheKeys {
    pub(super) shaped: Vec<(RichTextRange, ResolvedGlyph)>,
    pub(super) vertical_alternates: BTreeMap<usize, Vec<ResolvedGlyph>>,
    pub(super) per_glyph: BTreeMap<usize, Vec<ResolvedGlyph>>,
}

pub(super) fn layout_glyph_cache_keys(
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
    let mut cache_keys = LayoutGlyphCacheKeys {
        shaped,
        vertical_alternates,
        per_glyph: BTreeMap::new(),
    };
    cache_keys.per_glyph = per_glyph_cache_keys(font_system, rich_text, layout, &cache_keys);
    cache_keys
}

pub(super) fn text_buffer_cache_keys(
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

pub(super) fn native_style_for_display_range(
    rich_text: &WindowRichText,
    range: RichTextRange,
) -> NativeTextStyle {
    rich_text
        .spans
        .iter()
        .find(|span| span.range.start <= range.start && range.end <= span.range.end)
        .map_or_else(NativeTextStyle::default, |span| span.style.clone())
}

pub(super) fn vertical_form_cache_keys(
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

pub(super) fn per_glyph_cache_keys(
    font_system: &mut FontSystem,
    rich_text: &WindowRichText,
    layout: &LaidOutText,
    cache_keys: &LayoutGlyphCacheKeys,
) -> BTreeMap<usize, Vec<ResolvedGlyph>> {
    layout
        .glyphs
        .iter()
        .enumerate()
        .filter(|(glyph_index, glyph)| {
            cache_keys
                .vertical_alternates
                .get(glyph_index)
                .is_none_or(Vec::is_empty)
                && cache_keys_for_shaped_layout_glyph(glyph.range, cache_keys).is_empty()
        })
        .filter_map(|(glyph_index, glyph)| {
            let style = native_style_for_display_range(rich_text, glyph.range);
            let fallback = shaped_cache_keys_for_text(font_system, glyph.text.as_str(), &style);
            (!fallback.is_empty()).then_some((glyph_index, fallback))
        })
        .collect()
}

pub(super) fn shaped_cache_keys_for_text(
    font_system: &mut FontSystem,
    text: &str,
    style: &NativeTextStyle,
) -> Vec<ResolvedGlyph> {
    let mut buffer = Buffer::new(font_system, style.metrics());
    let attrs = style.attrs();
    let spans = [(text, attrs.clone())];
    buffer.set_rich_text(font_system, spans, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(font_system, false);
    text_buffer_cache_keys_for_text(&buffer)
}

pub(super) fn text_buffer_cache_keys_for_text(buffer: &Buffer) -> Vec<ResolvedGlyph> {
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

pub(super) fn vertical_form_font_features(vertical_form: GlyphVerticalForm) -> FontFeatures {
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

pub(super) fn native_text_font_features() -> FontFeatures {
    let mut features = FontFeatures::new();
    features
        .disable(FeatureTag::new(b"liga"))
        .disable(FeatureTag::new(b"clig"));
    features
}

pub(super) fn cache_keys_for_layout_glyph(
    glyph_index: usize,
    range: RichTextRange,
    cache_keys: &LayoutGlyphCacheKeys,
) -> Vec<ResolvedGlyph> {
    if let Some(cache_keys) = cache_keys.vertical_alternates.get(&glyph_index) {
        return normalize_resolved_glyph_offsets(cache_keys.iter().copied());
    }
    if let Some(cache_keys) = cache_keys.per_glyph.get(&glyph_index) {
        return normalize_resolved_glyph_offsets(cache_keys.iter().copied());
    }
    cache_keys_for_shaped_layout_glyph(range, cache_keys)
}

pub(super) fn cache_keys_for_shaped_layout_glyph(
    range: RichTextRange,
    cache_keys: &LayoutGlyphCacheKeys,
) -> Vec<ResolvedGlyph> {
    normalize_resolved_glyph_offsets(cache_keys.shaped.iter().filter_map(
        |(candidate, resolved)| {
            (candidate.start < range.end && range.start < candidate.end).then_some(*resolved)
        },
    ))
}

pub(super) fn apply_shaped_horizontal_origins_to_glyph_area(
    glyph_area: &mut OwnedGlyphArea,
    layout: &LaidOutText,
    cache_keys: &LayoutGlyphCacheKeys,
) {
    let mut run_cursor = ShapedHorizontalRunCursor::default();
    for (glyph_index, glyph) in layout.glyphs.iter().enumerate() {
        if !is_shaped_horizontal_origin_candidate(glyph, layout) {
            run_cursor.reset();
            continue;
        }
        if run_cursor.starts_new_segment(glyph) {
            run_cursor.start_segment(glyph);
        }
        let shaped_origin_x = run_cursor.cursor_x;
        let shaped_advance =
            shaped_horizontal_advance_for_layout_glyph(glyph_index, glyph, cache_keys)
                .unwrap_or(glyph.advance.width);
        offset_glyph_area_metadata_x(glyph_area, glyph_index, shaped_origin_x - glyph.origin.x);
        run_cursor.cursor_x = shaped_origin_x + shaped_advance.max(1.0);
    }
}

#[derive(Clone, Debug)]
pub(super) struct ShapedHorizontalGlyphMetrics {
    pub(super) origins: Vec<Option<f32>>,
    pub(super) advances: Vec<Option<f32>>,
}

impl ShapedHorizontalGlyphMetrics {
    pub(super) fn empty(glyph_count: usize) -> Self {
        Self {
            origins: vec![None; glyph_count],
            advances: vec![None; glyph_count],
        }
    }

    pub(super) fn origin(&self, glyph_index: usize) -> Option<f32> {
        self.origins.get(glyph_index).copied().flatten()
    }

    pub(super) fn advance(&self, glyph_index: usize) -> Option<f32> {
        self.advances.get(glyph_index).copied().flatten()
    }
}

pub(super) fn shaped_horizontal_glyph_metrics(
    page_layout: &NativePageLayout,
) -> ShapedHorizontalGlyphMetrics {
    let Some(page) = WindowPage::from_frame(&page_layout.frame)
        .into_iter()
        .next()
    else {
        return ShapedHorizontalGlyphMetrics::empty(page_layout.layout.glyphs.len());
    };
    let (width, height) = text_buffer_extents_for_layout_config(page_layout.config);
    let mut font_system = FontSystem::new();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));
    prepare_window_text_buffers(
        &mut font_system,
        &mut buffer,
        &page.rich_text,
        width,
        height,
    );
    let cache_keys = layout_glyph_cache_keys(
        &mut font_system,
        &buffer,
        &page.rich_text,
        &page_layout.layout,
    );
    shaped_horizontal_glyph_metrics_from_cache(&page_layout.layout, &cache_keys)
}

pub(super) fn shaped_horizontal_glyph_metrics_from_cache(
    layout: &LaidOutText,
    cache_keys: &LayoutGlyphCacheKeys,
) -> ShapedHorizontalGlyphMetrics {
    let mut metrics = ShapedHorizontalGlyphMetrics::empty(layout.glyphs.len());
    let mut run_cursor = ShapedHorizontalRunCursor::default();
    for (glyph_index, glyph) in layout.glyphs.iter().enumerate() {
        if !is_shaped_horizontal_origin_candidate(glyph, layout) {
            run_cursor.reset();
            continue;
        }
        if run_cursor.starts_new_segment(glyph) {
            run_cursor.start_segment(glyph);
        }
        let shaped_origin_x = run_cursor.cursor_x;
        let shaped_advance =
            shaped_horizontal_advance_for_layout_glyph(glyph_index, glyph, cache_keys)
                .unwrap_or(glyph.advance.width);
        metrics.origins[glyph_index] = Some(shaped_origin_x);
        metrics.advances[glyph_index] = Some(shaped_advance.max(1.0));
        run_cursor.cursor_x = shaped_origin_x + shaped_advance.max(1.0);
    }
    metrics
}

pub(super) fn apply_shaped_horizontal_origins_to_placements(
    placements: &mut [NativeGlyphPlacement],
    layout: &LaidOutText,
    metrics: &ShapedHorizontalGlyphMetrics,
) {
    for (glyph_index, (glyph, placement)) in
        layout.glyphs.iter().zip(placements.iter_mut()).enumerate()
    {
        let Some(shaped_origin_x) = metrics.origin(glyph_index) else {
            continue;
        };
        placement.x += shaped_origin_x - glyph.origin.x;
    }
}

pub(super) fn text_buffer_extents_for_layout_config(config: TextLayoutConfig) -> (u32, u32) {
    (
        text_buffer_extent_from_f32(config.origin.x + config.size.width),
        text_buffer_extent_from_f32(config.origin.y + config.size.height),
    )
}

pub(super) fn text_buffer_extent_from_f32(value: f32) -> u32 {
    if !value.is_finite() || value <= 1.0 {
        return 1;
    }
    let value = f64::from(value).ceil();
    if value >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        value.to_string().parse().unwrap_or(u32::MAX)
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ShapedHorizontalRunCursor {
    pub(super) run_index: Option<usize>,
    pub(super) line_y: f32,
    pub(super) cursor_x: f32,
}

impl ShapedHorizontalRunCursor {
    pub(super) fn reset(&mut self) {
        self.run_index = None;
    }

    pub(super) fn starts_new_segment(&self, glyph: &LaidOutGlyph) -> bool {
        self.run_index != Some(glyph.run_index) || (self.line_y - glyph.origin.y).abs() > 0.5
    }

    pub(super) fn start_segment(&mut self, glyph: &LaidOutGlyph) {
        self.run_index = Some(glyph.run_index);
        self.line_y = glyph.origin.y;
        self.cursor_x = glyph.origin.x;
    }
}

pub(super) fn is_shaped_horizontal_origin_candidate(
    glyph: &LaidOutGlyph,
    layout: &LaidOutText,
) -> bool {
    glyph.writing_mode == RichTextWritingMode::HorizontalTb
        && glyph.orientation == GlyphOrientation::Upright
        && glyph.vertical_form == GlyphVerticalForm::None
        && !layout
            .ruby
            .iter()
            .any(|ruby| ranges_overlap(glyph.range, ruby.base_range))
}

pub(super) fn shaped_horizontal_advance_for_layout_glyph(
    glyph_index: usize,
    glyph: &LaidOutGlyph,
    cache_keys: &LayoutGlyphCacheKeys,
) -> Option<f32> {
    let advance = cache_keys_for_layout_glyph(glyph_index, glyph.range, cache_keys)
        .iter()
        .map(|resolved| resolved.advance.x.max(0.0))
        .sum::<f32>();
    (advance > 0.0).then_some(advance)
}

pub(super) fn offset_glyph_area_metadata_x(
    glyph_area: &mut OwnedGlyphArea,
    glyph_index: usize,
    offset_x: f32,
) {
    if offset_x.abs() <= f32::EPSILON {
        return;
    }
    for glyph in glyph_area
        .glyphs_mut()
        .iter_mut()
        .filter(|glyph| glyph.metadata == glyph_index)
    {
        glyph.origin.x += offset_x;
    }
}

pub(super) const fn ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

pub(super) fn normalize_resolved_glyph_offsets(
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

pub(super) fn padded_rgba_row_bytes(width: u32) -> u32 {
    let row_bytes = width.saturating_mul(4);
    row_bytes.saturating_add(COPY_BYTES_PER_ROW_ALIGNMENT - 1) / COPY_BYTES_PER_ROW_ALIGNMENT
        * COPY_BYTES_PER_ROW_ALIGNMENT
}

pub(super) fn unpad_rgba_rows(
    mapped: &[u8],
    width: u32,
    height: u32,
    padded_row_bytes: u32,
) -> Vec<u8> {
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
pub(super) struct NativeFrameContentStats {
    pub(super) content_bbox: Option<NativeFrameContentBBox>,
    pub(super) content_pixels: u64,
}

pub(super) fn native_frame_content_stats(
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

pub(super) fn solid_rgba(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let pixel_count = usize::try_from(width)
        .unwrap_or(0)
        .saturating_mul(usize::try_from(height).unwrap_or(0));
    let mut rgba = Vec::with_capacity(pixel_count.saturating_mul(4));
    for _ in 0..pixel_count {
        rgba.extend_from_slice(&color);
    }
    rgba
}

pub(super) fn clear_transparent_rgb(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] == 0 {
            pixel.copy_from_slice(&[0, 0, 0, 0]);
        }
    }
}

pub(super) fn fill_native_rect(
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

pub(super) fn redraw(state: &mut WindowState) {
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

pub(super) fn apply_text_colors_to_glyph_area(
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

pub(super) fn apply_text_transforms_to_glyph_area(
    glyph_area: &mut OwnedGlyphArea,
    line_key: &str,
    layout: &LaidOutText,
    time_seconds: f32,
) {
    let transforms = native_glyph_placements_for_layout(line_key, layout, time_seconds);
    apply_text_transform_placements_to_glyph_area(glyph_area, layout, &transforms);
}

pub(super) fn apply_text_transforms_to_glyph_area_with_effects(
    glyph_area: &mut OwnedGlyphArea,
    line_key: &str,
    layout: &LaidOutText,
    time_seconds: f32,
    effects: &mut NativeEffectExecution<'_>,
) {
    let transforms = native_glyph_placements_for_layout_with_effects(
        line_key,
        layout,
        time_seconds,
        Some(effects),
    );
    apply_text_transform_placements_to_glyph_area(glyph_area, layout, &transforms);
}

pub(super) fn apply_text_transform_placements_to_glyph_area(
    glyph_area: &mut OwnedGlyphArea,
    layout: &LaidOutText,
    transforms: &[NativeGlyphPlacement],
) {
    for instance in glyph_area.glyphs_mut() {
        let Some(placement) = transforms.get(instance.metadata) else {
            continue;
        };
        let Some(glyph) = layout.glyphs.get(instance.metadata) else {
            continue;
        };
        instance.origin = Point::new(
            instance.origin.x + placement.x - glyph.origin.x,
            instance.origin.y + placement.y - glyph.origin.y,
        );
        if let Some(affine) = glyph_presentation_affine(placement, glyph, layout) {
            let current = glyph_transform_affine(instance.transform);
            instance.transform =
                GlyphTransform::Affine(Affine2::new(compose_affine(affine, current)));
        }
        apply_placement_color_override(instance, placement);
    }
}

pub(super) fn apply_placement_color_override(
    instance: &mut GlyphInstance,
    placement: &NativeGlyphPlacement,
) {
    let Some([red, green, blue, alpha]) = placement.color else {
        return;
    };
    instance.color = Some(Color::rgba(
        red,
        green,
        blue,
        scale_alpha_by_opacity(alpha, placement.opacity),
    ));
}

pub(super) fn glyph_presentation_affine(
    placement: &NativeGlyphPlacement,
    glyph: &LaidOutGlyph,
    layout: &LaidOutText,
) -> Option<[f32; 6]> {
    let base_rotation = glyph_orientation_degrees(glyph.orientation);
    presentation_affine(
        placement,
        base_rotation,
        glyph_transform_pivot(placement, glyph, layout),
    )
}

pub(super) fn ruby_glyph_presentation_affine(
    placement: &NativeGlyphPlacement,
    presentation: &RichTextPresentation,
    glyph: &GlyphInstance,
) -> Option<[f32; 6]> {
    presentation_affine(
        placement,
        0.0,
        ruby_glyph_transform_pivot(placement, presentation, glyph),
    )
}

pub(super) fn presentation_affine(
    placement: &NativeGlyphPlacement,
    base_rotation: f32,
    pivot: Vector,
) -> Option<[f32; 6]> {
    let has_transform = (placement.rotate_degrees - base_rotation).abs() > f32::EPSILON
        || placement.skew_x_degrees.abs() > f32::EPSILON
        || placement.skew_y_degrees.abs() > f32::EPSILON
        || (placement.scale_x - 1.0).abs() > f32::EPSILON
        || (placement.scale_y - 1.0).abs() > f32::EPSILON;
    if !has_transform {
        return None;
    }

    let radians = (placement.rotate_degrees - base_rotation).to_radians();
    let (sin, cos) = radians.sin_cos();
    let skew_x = placement.skew_x_degrees.to_radians().tan();
    let skew_y = placement.skew_y_degrees.to_radians().tan();
    let scaled_a = placement.scale_x;
    let scaled_b = skew_y * placement.scale_x;
    let scaled_c = skew_x * placement.scale_y;
    let scaled_d = placement.scale_y;
    let matrix_a = cos.mul_add(scaled_a, -sin * scaled_b);
    let matrix_b = sin.mul_add(scaled_a, cos * scaled_b);
    let matrix_c = cos.mul_add(scaled_c, -sin * scaled_d);
    let matrix_d = sin.mul_add(scaled_c, cos * scaled_d);
    let matrix_e = pivot.x - matrix_a.mul_add(pivot.x, matrix_c * pivot.y);
    let matrix_f = pivot.y - matrix_b.mul_add(pivot.x, matrix_d * pivot.y);
    Some([matrix_a, matrix_b, matrix_c, matrix_d, matrix_e, matrix_f])
}

pub(super) fn glyph_transform_pivot(
    placement: &NativeGlyphPlacement,
    glyph: &LaidOutGlyph,
    layout: &LaidOutText,
) -> Vector {
    let (origin, target) =
        if let (Some(origin), Some(target)) = (placement.affine_origin, placement.affine_target) {
            (origin, target)
        } else if let Some(transform) = &glyph.presentation.transform {
            (transform.origin, transform.target)
        } else {
            return Vector::new(0.0, 0.0);
        };
    let target_bounds = transform_target_bounds(target, glyph, layout);
    match origin {
        RichTextTransformOrigin::BaselineStart => Vector::new(
            target_bounds.x - glyph.origin.x,
            target_bounds.y - glyph.origin.y,
        ),
        RichTextTransformOrigin::BaselineCenter
        | RichTextTransformOrigin::Center
        | RichTextTransformOrigin::GlyphCenter => Vector::new(
            target_bounds.x + target_bounds.width * 0.5 - glyph.origin.x,
            target_bounds.y + target_bounds.height * 0.5 - glyph.origin.y,
        ),
    }
}

pub(super) fn transform_target_bounds(
    target: RichTextEffectTarget,
    glyph: &LaidOutGlyph,
    layout: &LaidOutText,
) -> LayoutRect {
    match target {
        RichTextEffectTarget::Glyph => glyph.bounds,
        RichTextEffectTarget::Run => layout
            .runs
            .get(glyph.run_index)
            .map_or(glyph.bounds, |run| run.bounds),
        RichTextEffectTarget::Document
        | RichTextEffectTarget::Line
        | RichTextEffectTarget::Sentence
        | RichTextEffectTarget::TextBox
        | RichTextEffectTarget::Screen => layout.bounds.unwrap_or(glyph.bounds),
    }
}

pub(super) fn ruby_glyph_transform_pivot(
    placement: &NativeGlyphPlacement,
    presentation: &RichTextPresentation,
    glyph: &GlyphInstance,
) -> Vector {
    let origin = if let Some(origin) = placement.affine_origin {
        origin
    } else if let Some(transform) = &presentation.transform {
        transform.origin
    } else {
        return Vector::new(0.0, 0.0);
    };
    match origin {
        RichTextTransformOrigin::BaselineStart => Vector::new(0.0, 0.0),
        RichTextTransformOrigin::BaselineCenter => {
            Vector::new(glyph.advance.x * 0.5, glyph.advance.y * 0.5)
        }
        RichTextTransformOrigin::Center | RichTextTransformOrigin::GlyphCenter => Vector::new(
            glyph.ink_bounds.width() * 0.5,
            glyph.ink_bounds.height() * 0.5,
        ),
    }
}

pub(super) fn glyph_transform_affine(transform: GlyphTransform) -> [f32; 6] {
    match transform {
        GlyphTransform::Identity => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        GlyphTransform::Rotate90Cw => [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
        GlyphTransform::Rotate90Ccw => [0.0, -1.0, 1.0, 0.0, 0.0, 0.0],
        GlyphTransform::Affine(affine) => affine.values,
        GlyphTransform::Rotate90CwThenAffine(affine) => {
            compose_affine(affine.values, [0.0, 1.0, -1.0, 0.0, 0.0, 0.0])
        }
        GlyphTransform::Rotate90CcwThenAffine(affine) => {
            compose_affine(affine.values, [0.0, -1.0, 1.0, 0.0, 0.0, 0.0])
        }
    }
}

pub(super) fn compose_affine(left: [f32; 6], right: [f32; 6]) -> [f32; 6] {
    [
        left[0].mul_add(right[0], left[2] * right[1]),
        left[1].mul_add(right[0], left[3] * right[1]),
        left[0].mul_add(right[2], left[2] * right[3]),
        left[1].mul_add(right[2], left[3] * right[3]),
        left[0].mul_add(right[4], left[2].mul_add(right[5], left[4])),
        left[1].mul_add(right[4], left[3].mul_add(right[5], left[5])),
    ]
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn glyph_alpha_for_time(
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
        if effect.id != "typewriter" {
            continue;
        }
        if !effect_applies_to_glyph_mask(effect) {
            continue;
        }
        let cps = param_milli(effect, "cps")
            .unwrap_or(Milli(28000))
            .as_f32()
            .max(0.0);
        let visible = typewriter_visible_count_from_cps(effect, time_seconds, glyph_count, cps);
        if glyph_index >= visible.min(glyph_count) {
            alpha *= typewriter_cursor_opacity(effect, glyph_index, visible, glyph_count);
        }
    }
    (alpha * 255.0).round().clamp(0.0, 255.0) as u8
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn presentation_alpha_for_visibility_time(
    presentation: &RichTextPresentation,
    time_seconds: f32,
) -> u8 {
    let mut alpha = presentation
        .opacity
        .map_or(1.0, Milli::as_f32)
        .clamp(0.0, 1.0);
    for effect in &presentation.effects {
        if effect.id != "typewriter" {
            continue;
        }
        if !effect_applies_to_glyph_mask(effect) {
            continue;
        }
        let cps = param_milli(effect, "cps")
            .unwrap_or(Milli(28000))
            .as_f32()
            .max(0.0);
        let visible = typewriter_visible_count_from_cps(effect, time_seconds, 1, cps);
        if visible == 0 {
            alpha *= typewriter_cursor_opacity(effect, 0, visible, 1);
        }
    }
    (alpha * 255.0).round().clamp(0.0, 255.0) as u8
}

pub(super) fn typewriter_visible_count(
    effect: &RichTextEffectDescriptor,
    time_seconds: f32,
    glyph_count: usize,
) -> usize {
    let cps = param_milli(effect, "cps")
        .unwrap_or(Milli(28000))
        .as_f32()
        .max(0.0);
    typewriter_visible_count_from_cps(effect, time_seconds, glyph_count, cps)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn typewriter_visible_count_from_cps(
    effect: &RichTextEffectDescriptor,
    time_seconds: f32,
    glyph_count: usize,
    cps: f32,
) -> usize {
    let delay = param_milli(effect, "delay")
        .or_else(|| param_milli(effect, "start"))
        .unwrap_or_default()
        .as_f32()
        .max(0.0);
    let elapsed = (time_seconds.max(0.0) - delay).max(0.0);
    ((elapsed * cps).floor() as usize).min(glyph_count)
}

pub(super) fn typewriter_cursor_opacity(
    effect: &RichTextEffectDescriptor,
    glyph_index: usize,
    visible_count: usize,
    glyph_count: usize,
) -> f32 {
    if glyph_index != visible_count || visible_count >= glyph_count {
        return 0.0;
    }
    if !param_bool(effect, "cursor").unwrap_or(false) {
        return 0.0;
    }
    param_milli(effect, "cursor_alpha")
        .or_else(|| param_milli(effect, "cursor_opacity"))
        .unwrap_or(Milli(350))
        .as_f32()
        .clamp(0.0, 1.0)
}

pub(super) fn scaled_alpha(base: u8, factor: u8) -> u8 {
    let scaled = u16::from(base) * u16::from(factor);
    u8::try_from((scaled + 127) / 255).unwrap_or(u8::MAX)
}

pub(super) fn scale_alpha_by_opacity(alpha: u8, opacity: f32) -> u8 {
    let factor = rounded_u8(opacity.clamp(0.0, 1.0) * 255.0);
    scaled_alpha(alpha, factor)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn rounded_u8(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

pub(super) fn prepare_window_text_renderer(state: &mut WindowState) -> Result<(), ()> {
    let time_seconds = state.effect_time_seconds();
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
        apply_shaped_horizontal_origins_to_glyph_area(&mut glyph_area, layout, &cache_keys);
        apply_text_colors_to_glyph_area(&mut glyph_area, &state.rich_text, layout, time_seconds);
        let mut effects = NativeEffectExecution::new(
            Some(&mut state.effect_registry),
            Some(&mut state.shader_registry),
            Some(&mut state.motion_registry),
            &mut state.effect_state,
        );
        observe_layout_shaders(
            &mut effects,
            layout,
            state.ruby_buffers.iter().map(|ruby| &ruby.presentation),
        );
        apply_text_transforms_to_glyph_area_with_effects(
            &mut glyph_area,
            &state.rich_text.text,
            layout,
            time_seconds,
            &mut effects,
        );
        let text_shader_glyph_areas =
            shader_glyph_areas_for_text(&glyph_area, layout, &mut effects);
        let ruby_glyph_areas = ruby_glyph_areas(
            &state.ruby_buffers,
            &state.rich_text.text,
            state.surface_config.width,
            state.surface_config.height,
            time_seconds,
            false,
            Some(&mut effects),
        );
        let ruby_shader_glyph_areas =
            shader_glyph_areas_for_ruby(&ruby_glyph_areas, &state.ruby_buffers, &mut effects);
        let glyph_areas = native_glyph_area_submission_list(
            &text_shader_glyph_areas,
            &glyph_area,
            &ruby_shader_glyph_areas,
            &ruby_glyph_areas,
        );
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

pub(super) fn surface_extent_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

pub(super) fn surface_extent_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

pub(super) fn usize_to_f32_saturating(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}
