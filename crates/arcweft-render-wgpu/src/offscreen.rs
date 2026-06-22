use crate::geometry::PreparedFrame;
use crate::renderer::{SharedRenderer, SharedRendererError};
use std::sync::mpsc;
use thiserror::Error;

/// Unpadded RGBA8 capture produced by the shared renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedFrameCapture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Reusable native offscreen capture session for shared renderer parity checks.
pub struct SharedOffscreenCapture {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: SharedRenderer,
    format: wgpu::TextureFormat,
}

#[derive(Debug, Error)]
pub enum SharedOffscreenCaptureError {
    #[error("no native WebGPU adapter is available for offscreen capture")]
    AdapterUnavailable,
    #[error("offscreen WebGPU device acquisition failed: {0}")]
    DeviceRequest(String),
    #[error(transparent)]
    SharedRenderer(#[from] SharedRendererError),
    #[error("offscreen readback failed: {0}")]
    Readback(String),
}

impl SharedOffscreenCapture {
    pub async fn new(format: wgpu::TextureFormat) -> Result<Self, SharedOffscreenCaptureError> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .map_err(|_| SharedOffscreenCaptureError::AdapterUnavailable)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("arcweft-shared-offscreen-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .map_err(|error| SharedOffscreenCaptureError::DeviceRequest(error.to_string()))?;
        let renderer = SharedRenderer::new(&device, &queue, format);
        Ok(Self {
            device,
            queue,
            renderer,
            format,
        })
    }

    pub fn register_font_bytes(
        &mut self,
        bytes: Vec<u8>,
    ) -> Result<(), SharedOffscreenCaptureError> {
        self.renderer.register_font_bytes(bytes)?;
        Ok(())
    }

    pub fn capture_frame(
        &mut self,
        frame: &PreparedFrame,
    ) -> Result<SharedFrameCapture, SharedOffscreenCaptureError> {
        let width = frame.viewport.physical_width.max(1);
        let height = frame.viewport.physical_height.max(1);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("arcweft-shared-offscreen-target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer
            .render_to_view(&self.device, &self.queue, &view, frame)?;
        let rgba = readback_texture_rgba(&self.device, &self.queue, &texture, width, height)?;
        Ok(SharedFrameCapture {
            width,
            height,
            rgba,
        })
    }
}

fn readback_texture_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, SharedOffscreenCaptureError> {
    let padded_row_bytes = padded_rgba_row_bytes(width);
    let buffer_size = u64::from(padded_row_bytes).saturating_mul(u64::from(height));
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("arcweft-shared-offscreen-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("arcweft-shared-offscreen-readback-encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result.map_err(|error| error.to_string()));
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| SharedOffscreenCaptureError::Readback(error.to_string()))?;
    receiver
        .recv()
        .map_err(|error| SharedOffscreenCaptureError::Readback(error.to_string()))?
        .map_err(SharedOffscreenCaptureError::Readback)?;

    let mapped = slice.get_mapped_range();
    let rgba = unpad_rgba_rows(&mapped, width, height, padded_row_bytes);
    drop(mapped);
    readback.unmap();
    Ok(rgba)
}

fn padded_rgba_row_bytes(width: u32) -> u32 {
    let row_bytes = width.saturating_mul(4);
    row_bytes.saturating_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
        / wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}

fn unpad_rgba_rows(mapped: &[u8], width: u32, height: u32, padded_row_bytes: u32) -> Vec<u8> {
    let row_bytes = usize::try_from(width.saturating_mul(4)).unwrap_or(0);
    let padded = usize::try_from(padded_row_bytes).unwrap_or(row_bytes);
    (0..usize::try_from(height).unwrap_or(0))
        .flat_map(|row| {
            let start = row.saturating_mul(padded);
            let end = start.saturating_add(row_bytes).min(mapped.len());
            mapped[start..end].iter().copied()
        })
        .collect()
}
