use crate::geometry::{PreparedFrame, RenderViewport};
use crate::renderer::{SharedRenderer, SharedRendererError};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use num_traits::ToPrimitive;
use std::collections::HashSet;
use std::sync::mpsc;
use thiserror::Error;

/// One typed attachment produced by a shared offscreen capture.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CaptureAttachment {
    /// Exact RGBA output rendered by [`SharedRenderer`].
    Color,
    /// Caller-assigned object-id RGBA for each ordered capture region.
    ObjectId,
    /// Opaque white inside the capture scope and transparent black outside it.
    Mask,
}

/// One stable, logical region used to derive scoped capture attachments.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureRegion {
    pub id: PublicId,
    pub bounds: HitRect,
    pub object_id_rgba: [u8; 4],
}

impl CaptureRegion {
    #[must_use]
    pub const fn new(id: PublicId, bounds: HitRect, object_id_rgba: [u8; 4]) -> Self {
        Self {
            id,
            bounds,
            object_id_rgba,
        }
    }
}

/// Geometry included in a capture.
#[derive(Clone, Debug, PartialEq)]
pub enum CaptureScope {
    /// Preserve the complete rendered frame.
    WholeFrame,
    /// Preserve the union of these regions in their supplied painter order.
    Regions(Vec<CaptureRegion>),
}

/// Pixel extent returned by a capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureCropPolicy {
    /// Return the physical frame extent, with transparent pixels outside scope.
    FullFrame,
    /// Return the smallest physical-pixel rectangle enclosing the scope.
    ScopeBounds,
}

/// Typed request for one shared-renderer capture and its derived attachments.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureRequest {
    attachments: Vec<CaptureAttachment>,
    scope: CaptureScope,
    crop_policy: CaptureCropPolicy,
}

impl CaptureRequest {
    #[must_use]
    pub fn new(
        attachments: impl IntoIterator<Item = CaptureAttachment>,
        scope: CaptureScope,
        crop_policy: CaptureCropPolicy,
    ) -> Self {
        Self {
            attachments: attachments.into_iter().collect(),
            scope,
            crop_policy,
        }
    }

    #[must_use]
    pub fn whole_frame_color() -> Self {
        Self::new(
            [CaptureAttachment::Color],
            CaptureScope::WholeFrame,
            CaptureCropPolicy::FullFrame,
        )
    }

    #[must_use]
    pub fn attachments(&self) -> &[CaptureAttachment] {
        &self.attachments
    }

    #[must_use]
    pub const fn scope(&self) -> &CaptureScope {
        &self.scope
    }

    #[must_use]
    pub const fn crop_policy(&self) -> CaptureCropPolicy {
        self.crop_policy
    }
}

/// One unpadded RGBA8 attachment in a [`SharedFrameCapture`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedAttachment {
    pub attachment: CaptureAttachment,
    pub rgba: Vec<u8>,
}

/// Shared-renderer capture with an explicit physical crop origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedFrameCapture {
    pub origin_x: u32,
    pub origin_y: u32,
    pub width: u32,
    pub height: u32,
    pub attachments: Vec<CapturedAttachment>,
}

impl SharedFrameCapture {
    #[must_use]
    pub fn attachment(&self, attachment: CaptureAttachment) -> Option<&CapturedAttachment> {
        self.attachments
            .iter()
            .find(|captured| captured.attachment == attachment)
    }

    #[must_use]
    pub fn attachment_rgba(&self, attachment: CaptureAttachment) -> Option<&[u8]> {
        self.attachment(attachment)
            .map(|captured| captured.rgba.as_slice())
    }
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
    #[error("offscreen capture requires an RGBA8 target, not {format:?}")]
    UnsupportedTextureFormat { format: wgpu::TextureFormat },
    #[error("no native WebGPU adapter is available for offscreen capture")]
    AdapterUnavailable,
    #[error("offscreen WebGPU device acquisition failed: {0}")]
    DeviceRequest(String),
    #[error("capture request must include at least one attachment")]
    EmptyAttachments,
    #[error("capture request contains duplicate attachment {attachment:?}")]
    DuplicateAttachment { attachment: CaptureAttachment },
    #[error("capture viewport must have finite, positive logical and physical dimensions")]
    InvalidViewport,
    #[error("ordered-region capture scope must contain at least one region")]
    EmptyRegionScope,
    #[error("capture region `{id}` must have finite bounds with positive width and height")]
    InvalidRegionBounds { id: PublicId },
    #[error("capture region id `{id}` occurs more than once")]
    DuplicateRegionId { id: PublicId },
    #[error("object-id attachment requires an ordered-region capture scope")]
    ObjectIdRequiresRegions,
    #[error("capture region `{id}` uses transparent black, which is reserved for no object")]
    TransparentObjectId { id: PublicId },
    #[error("object-id RGBA {rgba:?} occurs more than once")]
    DuplicateObjectIdRgba { rgba: [u8; 4] },
    #[error("capture scope does not intersect the physical frame")]
    EmptyScopeBounds,
    #[error("capture extent {width}x{height} cannot be represented as an RGBA8 buffer")]
    CaptureExtentOverflow { width: u32, height: u32 },
    #[error("shared renderer returned {actual} color bytes; expected {expected}")]
    RenderedColorSizeMismatch { expected: usize, actual: usize },
    #[error(transparent)]
    SharedRenderer(#[from] SharedRendererError),
    #[error("offscreen readback failed: {0}")]
    Readback(String),
}

impl SharedOffscreenCapture {
    pub async fn new(format: wgpu::TextureFormat) -> Result<Self, SharedOffscreenCaptureError> {
        if !matches!(
            format,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb
        ) {
            return Err(SharedOffscreenCaptureError::UnsupportedTextureFormat { format });
        }

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

    /// Renders `frame` exactly once, then derives every requested attachment
    /// from those completed pixels and the supplied ordered region geometry.
    pub fn capture(
        &mut self,
        frame: &PreparedFrame,
        request: &CaptureRequest,
    ) -> Result<SharedFrameCapture, SharedOffscreenCaptureError> {
        let plan = CapturePlan::new(frame.viewport, request)?;
        let width = frame.viewport.physical_width;
        let height = frame.viewport.physical_height;
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
        let color = readback_texture_rgba(&self.device, &self.queue, &texture, width, height)?;
        plan.derive(&color)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PixelRect {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl PixelRect {
    const fn full(width: u32, height: u32) -> Self {
        Self {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        }
    }

    const fn width(self) -> u32 {
        self.right - self.left
    }

    const fn height(self) -> u32 {
        self.bottom - self.top
    }

    const fn is_empty(self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }

    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }
}

#[derive(Clone, Debug)]
struct RasterRegion {
    rect: PixelRect,
    object_id_rgba: [u8; 4],
}

#[derive(Clone, Debug)]
enum RasterScope {
    WholeFrame,
    Regions(Vec<RasterRegion>),
}

#[derive(Clone, Debug)]
struct CapturePlan {
    frame_width: u32,
    frame_height: u32,
    crop: PixelRect,
    attachments: Vec<CaptureAttachment>,
    scope: RasterScope,
}

impl CapturePlan {
    fn new(
        viewport: RenderViewport,
        request: &CaptureRequest,
    ) -> Result<Self, SharedOffscreenCaptureError> {
        validate_viewport(viewport)?;
        validate_attachments(&request.attachments)?;

        let scope = match &request.scope {
            CaptureScope::WholeFrame => {
                if request.attachments.contains(&CaptureAttachment::ObjectId) {
                    return Err(SharedOffscreenCaptureError::ObjectIdRequiresRegions);
                }
                RasterScope::WholeFrame
            }
            CaptureScope::Regions(regions) => {
                RasterScope::Regions(rasterize_regions(viewport, regions, &request.attachments)?)
            }
        };
        let full_frame = PixelRect::full(viewport.physical_width, viewport.physical_height);
        rgba_len(viewport.physical_width, viewport.physical_height)?;
        padded_rgba_row_bytes(viewport.physical_width)?;
        let crop = match (request.crop_policy, &scope) {
            (CaptureCropPolicy::FullFrame, _) | (_, RasterScope::WholeFrame) => full_frame,
            (CaptureCropPolicy::ScopeBounds, RasterScope::Regions(regions)) => regions
                .iter()
                .map(|region| region.rect)
                .filter(|rect| !rect.is_empty())
                .reduce(PixelRect::union)
                .ok_or(SharedOffscreenCaptureError::EmptyScopeBounds)?,
        };
        rgba_len(crop.width(), crop.height())?;

        Ok(Self {
            frame_width: viewport.physical_width,
            frame_height: viewport.physical_height,
            crop,
            attachments: request.attachments.clone(),
            scope,
        })
    }

    fn derive(
        &self,
        rendered_color: &[u8],
    ) -> Result<SharedFrameCapture, SharedOffscreenCaptureError> {
        let expected = rgba_len(self.frame_width, self.frame_height)?;
        if rendered_color.len() != expected {
            return Err(SharedOffscreenCaptureError::RenderedColorSizeMismatch {
                expected,
                actual: rendered_color.len(),
            });
        }

        let attachments = self
            .attachments
            .iter()
            .copied()
            .map(|attachment| {
                self.derive_attachment(attachment, rendered_color)
                    .map(|rgba| CapturedAttachment { attachment, rgba })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SharedFrameCapture {
            origin_x: self.crop.left,
            origin_y: self.crop.top,
            width: self.crop.width(),
            height: self.crop.height(),
            attachments,
        })
    }

    fn derive_attachment(
        &self,
        attachment: CaptureAttachment,
        rendered_color: &[u8],
    ) -> Result<Vec<u8>, SharedOffscreenCaptureError> {
        let mut rgba = vec![0; rgba_len(self.crop.width(), self.crop.height())?];
        match (&self.scope, attachment) {
            (RasterScope::WholeFrame, CaptureAttachment::Color) => {
                copy_color_rect(
                    &mut rgba,
                    self.crop,
                    rendered_color,
                    self.frame_width,
                    self.crop,
                );
            }
            (RasterScope::WholeFrame, CaptureAttachment::Mask) => {
                rgba.chunks_exact_mut(4)
                    .for_each(|pixel| pixel.copy_from_slice(&[u8::MAX; 4]));
            }
            (RasterScope::Regions(regions), CaptureAttachment::Color) => {
                for region in regions {
                    copy_color_rect(
                        &mut rgba,
                        self.crop,
                        rendered_color,
                        self.frame_width,
                        region.rect,
                    );
                }
            }
            (RasterScope::Regions(regions), CaptureAttachment::ObjectId) => {
                for region in regions {
                    fill_rect(&mut rgba, self.crop, region.rect, region.object_id_rgba);
                }
            }
            (RasterScope::Regions(regions), CaptureAttachment::Mask) => {
                for region in regions {
                    fill_rect(&mut rgba, self.crop, region.rect, [u8::MAX; 4]);
                }
            }
            (RasterScope::WholeFrame, CaptureAttachment::ObjectId) => {
                return Err(SharedOffscreenCaptureError::ObjectIdRequiresRegions);
            }
        }
        Ok(rgba)
    }
}

fn validate_viewport(viewport: RenderViewport) -> Result<(), SharedOffscreenCaptureError> {
    if viewport.logical_width.is_finite()
        && viewport.logical_height.is_finite()
        && viewport.logical_width > 0.0
        && viewport.logical_height > 0.0
        && viewport.physical_width > 0
        && viewport.physical_height > 0
    {
        Ok(())
    } else {
        Err(SharedOffscreenCaptureError::InvalidViewport)
    }
}

fn validate_attachments(
    attachments: &[CaptureAttachment],
) -> Result<(), SharedOffscreenCaptureError> {
    if attachments.is_empty() {
        return Err(SharedOffscreenCaptureError::EmptyAttachments);
    }
    let mut unique = HashSet::with_capacity(attachments.len());
    if let Some(attachment) = attachments
        .iter()
        .copied()
        .find(|attachment| !unique.insert(*attachment))
    {
        return Err(SharedOffscreenCaptureError::DuplicateAttachment { attachment });
    }
    Ok(())
}

fn rasterize_regions(
    viewport: RenderViewport,
    regions: &[CaptureRegion],
    attachments: &[CaptureAttachment],
) -> Result<Vec<RasterRegion>, SharedOffscreenCaptureError> {
    if regions.is_empty() {
        return Err(SharedOffscreenCaptureError::EmptyRegionScope);
    }
    let needs_object_id = attachments.contains(&CaptureAttachment::ObjectId);
    let mut ids = HashSet::with_capacity(regions.len());
    let mut object_ids = HashSet::with_capacity(regions.len());
    regions
        .iter()
        .map(|region| {
            if !ids.insert(region.id.clone()) {
                return Err(SharedOffscreenCaptureError::DuplicateRegionId {
                    id: region.id.clone(),
                });
            }
            if !valid_region_bounds(region.bounds) {
                return Err(SharedOffscreenCaptureError::InvalidRegionBounds {
                    id: region.id.clone(),
                });
            }
            if needs_object_id {
                if region.object_id_rgba == [0; 4] {
                    return Err(SharedOffscreenCaptureError::TransparentObjectId {
                        id: region.id.clone(),
                    });
                }
                if !object_ids.insert(region.object_id_rgba) {
                    return Err(SharedOffscreenCaptureError::DuplicateObjectIdRgba {
                        rgba: region.object_id_rgba,
                    });
                }
            }
            Ok(RasterRegion {
                rect: physical_rect(viewport, region.bounds),
                object_id_rgba: region.object_id_rgba,
            })
        })
        .collect()
}

fn valid_region_bounds(bounds: HitRect) -> bool {
    bounds.x.is_finite()
        && bounds.y.is_finite()
        && bounds.width.is_finite()
        && bounds.height.is_finite()
        && bounds.width > 0.0
        && bounds.height > 0.0
}

fn physical_rect(viewport: RenderViewport, bounds: HitRect) -> PixelRect {
    let scale_x = f64::from(viewport.physical_width) / f64::from(viewport.logical_width);
    let scale_y = f64::from(viewport.physical_height) / f64::from(viewport.logical_height);
    let logical_right = f64::from(bounds.x) + f64::from(bounds.width);
    let logical_bottom = f64::from(bounds.y) + f64::from(bounds.height);
    let left = (f64::from(bounds.x).clamp(0.0, f64::from(viewport.logical_width)) * scale_x)
        .floor()
        .to_u32()
        .expect("clamped horizontal capture edge fits u32");
    let top = (f64::from(bounds.y).clamp(0.0, f64::from(viewport.logical_height)) * scale_y)
        .floor()
        .to_u32()
        .expect("clamped vertical capture edge fits u32");
    let right = (logical_right.clamp(0.0, f64::from(viewport.logical_width)) * scale_x)
        .ceil()
        .to_u32()
        .expect("clamped horizontal capture edge fits u32");
    let bottom = (logical_bottom.clamp(0.0, f64::from(viewport.logical_height)) * scale_y)
        .ceil()
        .to_u32()
        .expect("clamped vertical capture edge fits u32");
    PixelRect {
        left,
        top,
        right,
        bottom,
    }
}

fn copy_color_rect(
    output: &mut [u8],
    crop: PixelRect,
    source: &[u8],
    source_width: u32,
    rect: PixelRect,
) {
    let clipped = intersect(rect, crop);
    for y in clipped.top..clipped.bottom {
        for x in clipped.left..clipped.right {
            let source_index = pixel_index(x, y, source_width);
            let output_index = pixel_index(x - crop.left, y - crop.top, crop.width());
            output[output_index..output_index + 4]
                .copy_from_slice(&source[source_index..source_index + 4]);
        }
    }
}

fn fill_rect(output: &mut [u8], crop: PixelRect, rect: PixelRect, value: [u8; 4]) {
    let clipped = intersect(rect, crop);
    for y in clipped.top..clipped.bottom {
        for x in clipped.left..clipped.right {
            let output_index = pixel_index(x - crop.left, y - crop.top, crop.width());
            output[output_index..output_index + 4].copy_from_slice(&value);
        }
    }
}

fn intersect(left: PixelRect, right: PixelRect) -> PixelRect {
    PixelRect {
        left: left.left.max(right.left),
        top: left.top.max(right.top),
        right: left.right.min(right.right),
        bottom: left.bottom.min(right.bottom),
    }
}

fn pixel_index(x: u32, y: u32, width: u32) -> usize {
    usize::try_from((u64::from(y) * u64::from(width) + u64::from(x)) * 4)
        .expect("validated RGBA extent fits usize")
}

fn rgba_len(width: u32, height: u32) -> Result<usize, SharedOffscreenCaptureError> {
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(SharedOffscreenCaptureError::CaptureExtentOverflow { width, height })
}

fn readback_texture_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, SharedOffscreenCaptureError> {
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
    let rgba = unpad_rgba_rows(&mapped, width, height, padded_row_bytes)?;
    drop(mapped);
    readback.unmap();
    Ok(rgba)
}

fn padded_rgba_row_bytes(width: u32) -> Result<u32, SharedOffscreenCaptureError> {
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

#[cfg(test)]
mod tests {
    use super::{
        CaptureAttachment, CaptureCropPolicy, CapturePlan, CaptureRegion, CaptureRequest,
        CaptureScope, SharedOffscreenCaptureError,
    };
    use crate::geometry::RenderViewport;
    use arcweft_id::PublicId;
    use arcweft_presentation::hit::HitRect;

    fn viewport() -> RenderViewport {
        RenderViewport {
            logical_width: 4.0,
            logical_height: 3.0,
            physical_width: 4,
            physical_height: 3,
            scale_factor: 1.0,
        }
    }

    fn region(id: &str, bounds: HitRect, object_id_rgba: [u8; 4]) -> CaptureRegion {
        CaptureRegion::new(
            PublicId::try_new(id).expect("test id is valid"),
            bounds,
            object_id_rgba,
        )
    }

    fn rendered_color() -> Vec<u8> {
        (0_u8..12)
            .flat_map(|value| [value, value.wrapping_add(40), 100, u8::MAX])
            .collect()
    }

    fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let index = usize::try_from((u64::from(y) * u64::from(width) + u64::from(x)) * 4)
            .expect("test extent fits usize");
        rgba[index..index + 4].try_into().expect("one pixel")
    }

    #[test]
    fn requested_attachment_order_is_preserved() {
        let request = CaptureRequest::new(
            [
                CaptureAttachment::Mask,
                CaptureAttachment::Color,
                CaptureAttachment::ObjectId,
            ],
            CaptureScope::Regions(vec![region(
                "capture.region",
                HitRect::new(0.0, 0.0, 1.0, 1.0),
                [1, 2, 3, u8::MAX],
            )]),
            CaptureCropPolicy::FullFrame,
        );
        let capture = CapturePlan::new(viewport(), &request)
            .expect("request validates")
            .derive(&rendered_color())
            .expect("attachments derive");

        assert_eq!(
            capture
                .attachments
                .iter()
                .map(|attachment| attachment.attachment)
                .collect::<Vec<_>>(),
            vec![
                CaptureAttachment::Mask,
                CaptureAttachment::Color,
                CaptureAttachment::ObjectId,
            ]
        );
    }

    #[test]
    fn later_region_wins_object_id_overlap() {
        let first = [10, 20, 30, u8::MAX];
        let second = [40, 50, 60, u8::MAX];
        let request = CaptureRequest::new(
            [CaptureAttachment::ObjectId],
            CaptureScope::Regions(vec![
                region("capture.first", HitRect::new(0.0, 0.0, 3.0, 2.0), first),
                region("capture.second", HitRect::new(1.0, 1.0, 3.0, 2.0), second),
            ]),
            CaptureCropPolicy::FullFrame,
        );
        let capture = CapturePlan::new(viewport(), &request)
            .expect("request validates")
            .derive(&rendered_color())
            .expect("object-id derives");
        let object_id = capture
            .attachment_rgba(CaptureAttachment::ObjectId)
            .expect("object-id attachment exists");

        assert_eq!(pixel(object_id, 4, 0, 0), first);
        assert_eq!(pixel(object_id, 4, 1, 1), second);
        assert_eq!(pixel(object_id, 4, 3, 2), second);
        assert_eq!(pixel(object_id, 4, 3, 0), [0; 4]);
    }

    #[test]
    fn scope_crop_reports_origin_and_preserves_transparent_gaps() {
        let request = CaptureRequest::new(
            [CaptureAttachment::Color, CaptureAttachment::Mask],
            CaptureScope::Regions(vec![
                region(
                    "capture.left",
                    HitRect::new(1.0, 1.0, 1.0, 1.0),
                    [1, 0, 0, u8::MAX],
                ),
                region(
                    "capture.right",
                    HitRect::new(3.0, 2.0, 1.0, 1.0),
                    [2, 0, 0, u8::MAX],
                ),
            ]),
            CaptureCropPolicy::ScopeBounds,
        );
        let source = rendered_color();
        let capture = CapturePlan::new(viewport(), &request)
            .expect("request validates")
            .derive(&source)
            .expect("attachments derive");

        assert_eq!((capture.origin_x, capture.origin_y), (1, 1));
        assert_eq!((capture.width, capture.height), (3, 2));
        let color = capture
            .attachment_rgba(CaptureAttachment::Color)
            .expect("color attachment exists");
        let mask = capture
            .attachment_rgba(CaptureAttachment::Mask)
            .expect("mask attachment exists");
        assert_eq!(pixel(color, 3, 0, 0), pixel(&source, 4, 1, 1));
        assert_eq!(pixel(color, 3, 1, 0), [0; 4]);
        assert_eq!(pixel(color, 3, 2, 1), pixel(&source, 4, 3, 2));
        assert_eq!(pixel(mask, 3, 0, 0), [u8::MAX; 4]);
        assert_eq!(pixel(mask, 3, 1, 0), [0; 4]);
        assert_eq!(pixel(mask, 3, 2, 1), [u8::MAX; 4]);
    }

    #[test]
    fn logical_regions_scale_to_physical_pixels() {
        let scaled = RenderViewport {
            physical_width: 8,
            physical_height: 6,
            scale_factor: 2.0,
            ..viewport()
        };
        let request = CaptureRequest::new(
            [CaptureAttachment::Mask],
            CaptureScope::Regions(vec![region(
                "capture.scaled",
                HitRect::new(1.0, 1.0, 1.0, 1.0),
                [1, 0, 0, u8::MAX],
            )]),
            CaptureCropPolicy::ScopeBounds,
        );
        let capture = CapturePlan::new(scaled, &request)
            .expect("request validates")
            .derive(&[0; 8 * 6 * 4])
            .expect("mask derives");

        assert_eq!((capture.origin_x, capture.origin_y), (2, 2));
        assert_eq!((capture.width, capture.height), (2, 2));
        assert!(
            capture
                .attachment_rgba(CaptureAttachment::Mask)
                .expect("mask exists")
                .chunks_exact(4)
                .all(|pixel| pixel == [u8::MAX; 4])
        );
    }

    #[test]
    fn invalid_requests_fail_with_typed_errors() {
        let empty = CaptureRequest::new([], CaptureScope::WholeFrame, CaptureCropPolicy::FullFrame);
        assert!(matches!(
            CapturePlan::new(viewport(), &empty),
            Err(SharedOffscreenCaptureError::EmptyAttachments)
        ));

        let duplicate = CaptureRequest::new(
            [CaptureAttachment::Color, CaptureAttachment::Color],
            CaptureScope::WholeFrame,
            CaptureCropPolicy::FullFrame,
        );
        assert!(matches!(
            CapturePlan::new(viewport(), &duplicate),
            Err(SharedOffscreenCaptureError::DuplicateAttachment {
                attachment: CaptureAttachment::Color
            })
        ));

        let object_without_regions = CaptureRequest::new(
            [CaptureAttachment::ObjectId],
            CaptureScope::WholeFrame,
            CaptureCropPolicy::FullFrame,
        );
        assert!(matches!(
            CapturePlan::new(viewport(), &object_without_regions),
            Err(SharedOffscreenCaptureError::ObjectIdRequiresRegions)
        ));

        let invalid_bounds = CaptureRequest::new(
            [CaptureAttachment::Mask],
            CaptureScope::Regions(vec![region(
                "capture.invalid",
                HitRect::new(0.0, 0.0, f32::NAN, 1.0),
                [1, 0, 0, u8::MAX],
            )]),
            CaptureCropPolicy::FullFrame,
        );
        assert!(matches!(
            CapturePlan::new(viewport(), &invalid_bounds),
            Err(SharedOffscreenCaptureError::InvalidRegionBounds { .. })
        ));

        let outside = CaptureRequest::new(
            [CaptureAttachment::Mask],
            CaptureScope::Regions(vec![region(
                "capture.outside",
                HitRect::new(10.0, 10.0, 1.0, 1.0),
                [1, 0, 0, u8::MAX],
            )]),
            CaptureCropPolicy::ScopeBounds,
        );
        assert!(matches!(
            CapturePlan::new(viewport(), &outside),
            Err(SharedOffscreenCaptureError::EmptyScopeBounds)
        ));
    }

    #[test]
    fn object_id_contract_rejects_ambiguous_region_metadata() {
        let transparent = CaptureRequest::new(
            [CaptureAttachment::ObjectId],
            CaptureScope::Regions(vec![region(
                "capture.transparent",
                HitRect::new(0.0, 0.0, 1.0, 1.0),
                [0; 4],
            )]),
            CaptureCropPolicy::FullFrame,
        );
        assert!(matches!(
            CapturePlan::new(viewport(), &transparent),
            Err(SharedOffscreenCaptureError::TransparentObjectId { .. })
        ));

        let duplicate_id = CaptureRequest::new(
            [CaptureAttachment::Mask],
            CaptureScope::Regions(vec![
                region(
                    "capture.duplicate",
                    HitRect::new(0.0, 0.0, 1.0, 1.0),
                    [1, 0, 0, u8::MAX],
                ),
                region(
                    "capture.duplicate",
                    HitRect::new(1.0, 1.0, 1.0, 1.0),
                    [2, 0, 0, u8::MAX],
                ),
            ]),
            CaptureCropPolicy::FullFrame,
        );
        assert!(matches!(
            CapturePlan::new(viewport(), &duplicate_id),
            Err(SharedOffscreenCaptureError::DuplicateRegionId { .. })
        ));

        let duplicate_rgba = CaptureRequest::new(
            [CaptureAttachment::ObjectId],
            CaptureScope::Regions(vec![
                region(
                    "capture.first_rgba",
                    HitRect::new(0.0, 0.0, 1.0, 1.0),
                    [7, 8, 9, u8::MAX],
                ),
                region(
                    "capture.second_rgba",
                    HitRect::new(1.0, 1.0, 1.0, 1.0),
                    [7, 8, 9, u8::MAX],
                ),
            ]),
            CaptureCropPolicy::FullFrame,
        );
        assert!(matches!(
            CapturePlan::new(viewport(), &duplicate_rgba),
            Err(SharedOffscreenCaptureError::DuplicateObjectIdRgba {
                rgba: [7, 8, 9, 255]
            })
        ));
    }

    #[test]
    fn unsupported_target_format_fails_before_device_acquisition() {
        let Err(error) = pollster::block_on(super::SharedOffscreenCapture::new(
            wgpu::TextureFormat::Bgra8Unorm,
        )) else {
            panic!("BGRA bytes must not be exposed as the promised RGBA attachment");
        };
        assert!(matches!(
            error,
            SharedOffscreenCaptureError::UnsupportedTextureFormat {
                format: wgpu::TextureFormat::Bgra8Unorm
            }
        ));
    }
}
