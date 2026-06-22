use crate::convert::{f32_ceil_to_i32, f32_floor_to_i32};
use crate::geometry::{PaintRect, PreparedFrame, RenderImage, RenderTextBlock};
use bytemuck::{Pod, Zeroable};
use glyphon::{
    Attrs, Buffer, Cache, Color, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer, Viewport,
};
use std::borrow::Cow;
use thiserror::Error;
use wgpu::util::DeviceExt;

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

        let mut buffers = frame
            .text
            .iter()
            .map(|block| text_buffer(&mut self.font_system, block))
            .collect::<Vec<_>>();
        let areas = buffers
            .iter_mut()
            .zip(&frame.text)
            .map(|(buffer, block)| text_area(buffer, block))
            .collect::<Vec<_>>();
        self.text_renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                areas,
                &mut self.swash_cache,
            )
            .map_err(|error| SharedRendererError::TextPrepare(error.to_string()))?;

        let vertices = rectangle_vertices(
            &frame.rectangles,
            frame.viewport.logical_width,
            frame.viewport.logical_height,
        );
        let vertex_buffer = (!vertices.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("arcweft-shared-rectangles"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });
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
            if let Some(vertex_buffer) = &vertex_buffer {
                pass.set_pipeline(&self.rectangle_pipeline);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.draw(0..u32::try_from(vertices.len()).unwrap_or(u32::MAX), 0..1);
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
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .map_err(|error| SharedRendererError::TextRender(error.to_string()))?;
        }
        queue.submit([encoder.finish()]);
        self.atlas.trim();
        Ok(())
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
        Some(block.bounds.width),
        Some(block.bounds.height),
    );
    buffer.set_text(
        font_system,
        &block.text,
        &Attrs::new(),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(font_system, false);
    buffer
}

fn text_area<'a>(buffer: &'a Buffer, block: &RenderTextBlock) -> TextArea<'a> {
    TextArea {
        buffer,
        left: block.bounds.x,
        top: block.bounds.y,
        scale: 1.0,
        bounds: TextBounds {
            left: f32_floor_to_i32(block.bounds.x),
            top: f32_floor_to_i32(block.bounds.y),
            right: f32_ceil_to_i32(block.bounds.x + block.bounds.width),
            bottom: f32_ceil_to_i32(block.bounds.y + block.bounds.height),
        },
        default_color: Color::rgba(block.rgba[0], block.rgba[1], block.rgba[2], block.rgba[3]),
        custom_glyphs: &[],
    }
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

fn image_vertices(image: &RenderImage, width: f32, height: f32) -> [ImageVertex; 6] {
    let left = (image.bounds.x / width) * 2.0 - 1.0;
    let right = ((image.bounds.x + image.bounds.width) / width) * 2.0 - 1.0;
    let top = 1.0 - (image.bounds.y / height) * 2.0;
    let bottom = 1.0 - ((image.bounds.y + image.bounds.height) / height) * 2.0;
    [
        ImageVertex {
            position: [left, top],
            uv: [0.0, 0.0],
        },
        ImageVertex {
            position: [left, bottom],
            uv: [0.0, 1.0],
        },
        ImageVertex {
            position: [right, bottom],
            uv: [1.0, 1.0],
        },
        ImageVertex {
            position: [left, top],
            uv: [0.0, 0.0],
        },
        ImageVertex {
            position: [right, bottom],
            uv: [1.0, 1.0],
        },
        ImageVertex {
            position: [right, top],
            uv: [1.0, 0.0],
        },
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
