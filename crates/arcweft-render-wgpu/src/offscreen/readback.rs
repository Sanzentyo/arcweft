use super::{PixelRect, SharedOffscreenCaptureError};
use std::sync::mpsc;

pub(super) fn readback_texture_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, SharedOffscreenCaptureError> {
    readback_texture_rect(
        device,
        queue,
        texture,
        PixelRect::full(width, height),
        unpad_rgba_rows,
    )
}

pub(super) fn readback_texture_alpha_rect(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    rect: PixelRect,
) -> Result<Vec<u8>, SharedOffscreenCaptureError> {
    if rect.is_empty() {
        return Ok(Vec::new());
    }
    readback_texture_rect(device, queue, texture, rect, unpad_alpha_rows)
}

fn readback_texture_rect<T>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    rect: PixelRect,
    decode: impl FnOnce(&[u8], u32, u32, u32) -> Result<T, SharedOffscreenCaptureError>,
) -> Result<T, SharedOffscreenCaptureError> {
    let width = rect.width();
    let height = rect.height();
    let padded_row_bytes = padded_rgba_row_bytes(width)?;
    let buffer_size = u64::from(padded_row_bytes)
        .checked_mul(u64::from(height))
        .ok_or(SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?;
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
            origin: wgpu::Origin3d {
                x: rect.left,
                y: rect.top,
                z: 0,
            },
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
    let decoded = decode(&mapped, width, height, padded_row_bytes)?;
    drop(mapped);
    readback.unmap();
    Ok(decoded)
}

pub(super) fn padded_rgba_row_bytes(width: u32) -> Result<u32, SharedOffscreenCaptureError> {
    let row_bytes = width
        .checked_mul(4)
        .ok_or(SharedOffscreenCaptureError::CaptureExtentOverflow { width, height: 1 })?;
    row_bytes
        .checked_add(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
        .map(|aligned| {
            aligned / wgpu::COPY_BYTES_PER_ROW_ALIGNMENT * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
        })
        .ok_or(SharedOffscreenCaptureError::CaptureExtentOverflow { width, height: 1 })
}

fn unpad_rgba_rows(
    mapped: &[u8],
    width: u32,
    height: u32,
    padded_row_bytes: u32,
) -> Result<Vec<u8>, SharedOffscreenCaptureError> {
    let row_bytes = usize::try_from(
        width
            .checked_mul(4)
            .ok_or(SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?,
    )
    .map_err(|_| SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?;
    let padded = usize::try_from(padded_row_bytes)
        .map_err(|_| SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?;
    let height_usize = usize::try_from(height)
        .map_err(|_| SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?;
    let expected_mapped = padded
        .checked_mul(height_usize)
        .ok_or(SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?;
    if mapped.len() < expected_mapped {
        return Err(SharedOffscreenCaptureError::Readback(format!(
            "mapped buffer has {} bytes; expected at least {expected_mapped}",
            mapped.len()
        )));
    }
    Ok(mapped
        .chunks_exact(padded)
        .take(height_usize)
        .flat_map(|row| row[..row_bytes].iter().copied())
        .collect())
}

fn unpad_alpha_rows(
    mapped: &[u8],
    width: u32,
    height: u32,
    padded_row_bytes: u32,
) -> Result<Vec<u8>, SharedOffscreenCaptureError> {
    let row_bytes = usize::try_from(
        width
            .checked_mul(4)
            .ok_or(SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?,
    )
    .map_err(|_| SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?;
    let padded = usize::try_from(padded_row_bytes)
        .map_err(|_| SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?;
    let height_usize = usize::try_from(height)
        .map_err(|_| SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?;
    let expected_mapped = padded
        .checked_mul(height_usize)
        .ok_or(SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?;
    if mapped.len() < expected_mapped {
        return Err(SharedOffscreenCaptureError::Readback(format!(
            "mapped buffer has {} bytes; expected at least {expected_mapped}",
            mapped.len()
        )));
    }
    let alpha_len = usize::try_from(
        u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or(SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?,
    )
    .map_err(|_| SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })?;
    let mut alpha = Vec::with_capacity(alpha_len);
    for row in mapped.chunks_exact(padded).take(height_usize) {
        alpha.extend(row[..row_bytes].chunks_exact(4).map(|pixel| pixel[3]));
    }
    Ok(alpha)
}
