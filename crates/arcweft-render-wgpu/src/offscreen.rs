use crate::geometry::{PreparedFrame, RenderViewport};
use crate::renderer::{SharedRenderer, SharedRendererError};
use crate::view_scene::PreparedTextId;
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_render_text::TextColor;
use num_traits::ToPrimitive;
use std::collections::HashSet;
use thiserror::Error;

mod budget;
mod readback;

use budget::{PreparedTextCaptureBudget, validate_prepared_text_capture_budget};
use readback::{padded_rgba_row_bytes, readback_texture_alpha_rect, readback_texture_rgba};

/// Maximum number of independent prepared-text coverage passes in one capture.
///
/// One pass preserves exact per-region painter order and metadata without
/// encoding semantic identity into filtered color channels.
pub const MAX_PREPARED_TEXT_COVERAGE_PASSES: u64 = 128;

/// Maximum full-frame renderer work for prepared-text coverage, measured in
/// physical pixels. This is exactly 32 Full HD pass equivalents.
pub const MAX_PREPARED_TEXT_COVERAGE_RENDER_PIXELS: u64 = 32 * 1_920 * 1_080;

/// Maximum cumulative prepared-text coverage readback in one capture.
///
/// Coverage readback is cropped and single-channel, but this limit is stated
/// in transferred RGBA8 bytes because the GPU copy contract is RGBA8.
pub const MAX_PREPARED_TEXT_COVERAGE_READBACK_BYTES: u64 = 64 * 1024 * 1024;

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
    geometry: CaptureRegionGeometry,
}

/// Non-empty, duplicate-free prepared glyph selection from one canonical
/// frame-local text item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedTextSelection {
    text: PreparedTextId,
    glyph_indices: Box<[u32]>,
}

/// Invalid canonical prepared-text selection metadata.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PreparedTextSelectionError {
    #[error("prepared-text selection must contain at least one glyph")]
    Empty,
    #[error("prepared-text selection contains duplicate glyph index {glyph_index}")]
    DuplicateGlyph { glyph_index: u32 },
}

impl PreparedTextSelection {
    pub fn try_new(
        text: PreparedTextId,
        glyph_indices: impl IntoIterator<Item = u32>,
    ) -> Result<Self, PreparedTextSelectionError> {
        let mut glyph_indices = glyph_indices.into_iter().collect::<Vec<_>>();
        if glyph_indices.is_empty() {
            return Err(PreparedTextSelectionError::Empty);
        }
        glyph_indices.sort_unstable();
        if let Some(glyph_index) = glyph_indices
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then_some(pair[0]))
        {
            return Err(PreparedTextSelectionError::DuplicateGlyph { glyph_index });
        }
        Ok(Self {
            text,
            glyph_indices: glyph_indices.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn text(&self) -> PreparedTextId {
        self.text
    }

    #[must_use]
    pub const fn glyph_indices(&self) -> &[u32] {
        &self.glyph_indices
    }
}

#[derive(Clone, Debug, PartialEq)]
enum CaptureRegionGeometry {
    Bounds,
    PreparedText(PreparedTextSelection),
}

impl CaptureRegion {
    #[must_use]
    pub const fn new(id: PublicId, bounds: HitRect, object_id_rgba: [u8; 4]) -> Self {
        Self {
            id,
            bounds,
            object_id_rgba,
            geometry: CaptureRegionGeometry::Bounds,
        }
    }

    /// Uses selected glyphs from the canonical prepared-text batch as the
    /// attachment coverage while retaining `bounds` as the requested crop.
    #[must_use]
    pub fn prepared_text(
        id: PublicId,
        bounds: HitRect,
        object_id_rgba: [u8; 4],
        selection: PreparedTextSelection,
    ) -> Self {
        Self {
            id,
            bounds,
            object_id_rgba,
            geometry: CaptureRegionGeometry::PreparedText(selection),
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

/// Bounded-work dimension reported by a prepared-text capture error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedTextCaptureBudgetMetric {
    /// Independent shared-renderer coverage passes.
    Passes,
    /// Full-frame physical pixels submitted to the renderer.
    RenderPixels,
    /// Cropped RGBA8 bytes transferred from GPU textures.
    ReadbackBytes,
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
    #[error("prepared-text capture region `{id}` references missing batch item {text_index}")]
    MissingPreparedTextItem { id: PublicId, text_index: u32 },
    #[error(
        "prepared-text capture region `{id}` references glyph {glyph_index}, but item {text_index} has {glyph_count} glyphs"
    )]
    MissingPreparedTextGlyph {
        id: PublicId,
        text_index: u32,
        glyph_index: u32,
        glyph_count: usize,
    },
    #[error("prepared-text capture {metric:?} overflowed while measuring bounded work")]
    PreparedTextCoverageBudgetOverflow {
        metric: PreparedTextCaptureBudgetMetric,
    },
    #[error(
        "prepared-text capture requires {actual} coverage passes, exceeding the limit of {limit}"
    )]
    PreparedTextCoveragePassBudgetExceeded { actual: u64, limit: u64 },
    #[error(
        "prepared-text capture requires {actual} rendered pixels, exceeding the limit of {limit}"
    )]
    PreparedTextCoverageRenderBudgetExceeded { actual: u64, limit: u64 },
    #[error(
        "prepared-text capture requires {actual} readback bytes, exceeding the limit of {limit}"
    )]
    PreparedTextCoverageReadbackBudgetExceeded { actual: u64, limit: u64 },
    #[error("prepared-text capture region {region_index} is missing its alpha coverage")]
    MissingPreparedTextCoverage { region_index: usize },
    #[error("alpha coverage was supplied twice for prepared-text capture region {region_index}")]
    DuplicatePreparedTextCoverage { region_index: usize },
    #[error("alpha coverage was supplied for non-prepared capture region {region_index}")]
    UnexpectedPreparedTextCoverage { region_index: usize },
    #[error(
        "prepared-text capture region {region_index} returned {actual_width}x{actual_height} alpha coverage; expected {expected_width}x{expected_height}"
    )]
    PreparedTextCoverageExtentMismatch {
        region_index: usize,
        expected_width: u32,
        expected_height: u32,
        actual_width: u32,
        actual_height: u32,
    },
    #[error(
        "prepared-text capture region {region_index} returned alpha coverage at ({actual_x}, {actual_y}); expected ({expected_x}, {expected_y})"
    )]
    PreparedTextCoverageOriginMismatch {
        region_index: usize,
        expected_x: u32,
        expected_y: u32,
        actual_x: u32,
        actual_y: u32,
    },
    #[error(
        "prepared-text capture region {region_index} returned {actual} alpha samples; expected {expected}"
    )]
    PreparedTextCoverageSizeMismatch {
        region_index: usize,
        expected: usize,
        actual: usize,
    },
    #[error("object-id attachment requires an ordered-region capture scope")]
    ObjectIdRequiresRegions,
    #[error("capture region `{id}` uses non-opaque object-id RGBA {rgba:?}")]
    NonOpaqueObjectId { id: PublicId, rgba: [u8; 4] },
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

    /// Renders the canonical color frame once and derives the requested
    /// attachments from its pixels and ordered region geometry. Each
    /// prepared-text region receives an independent transparent shared-renderer
    /// pass and cropped readback. Coverage is stamped immediately in painter
    /// order, keeping memory bounded to one coverage crop without encoding
    /// semantic IDs into filtered RGB.
    pub fn capture(
        &mut self,
        frame: &PreparedFrame,
        request: &CaptureRequest,
    ) -> Result<SharedFrameCapture, SharedOffscreenCaptureError> {
        let plan = CapturePlan::new(frame, request)?;
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
        let mut capture = plan.begin_capture(&color)?;
        let RasterScope::Regions(regions) = &plan.scope else {
            return Ok(capture);
        };
        for (region_index, region) in regions.iter().enumerate() {
            match &region.coverage {
                RasterCoverage::Bounds => plan.stamp_bounds_region(&mut capture, region),
                RasterCoverage::PreparedText(selection) if plan.needs_debug_coverage() => {
                    let rect = intersect(region.rect, plan.crop);
                    if rect.is_empty() {
                        continue;
                    }
                    let coverage = self.render_prepared_text_coverage(
                        frame,
                        region_index,
                        selection,
                        rect,
                        width,
                        height,
                    )?;
                    plan.stamp_prepared_text_region(&mut capture, region_index, region, &coverage)?;
                }
                RasterCoverage::PreparedText(_) => {}
            }
        }
        Ok(capture)
    }

    fn render_prepared_text_coverage(
        &mut self,
        frame: &PreparedFrame,
        region_index: usize,
        selection: &PreparedTextSelection,
        rect: PixelRect,
        width: u32,
        height: u32,
    ) -> Result<PreparedTextAlphaCoverage, SharedOffscreenCaptureError> {
        let attachment_frame = prepared_text_coverage_frame(frame, selection);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("arcweft-shared-offscreen-prepared-text-coverage"),
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
        self.renderer.render_coverage_to_view(
            &self.device,
            &self.queue,
            &view,
            &attachment_frame,
        )?;
        Ok(PreparedTextAlphaCoverage {
            region_index,
            rect,
            alpha: readback_texture_alpha_rect(&self.device, &self.queue, &texture, rect)?,
        })
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
    coverage: RasterCoverage,
}

#[derive(Clone, Debug)]
enum RasterCoverage {
    Bounds,
    PreparedText(PreparedTextSelection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedTextAlphaCoverage {
    region_index: usize,
    rect: PixelRect,
    alpha: Vec<u8>,
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
        frame: &PreparedFrame,
        request: &CaptureRequest,
    ) -> Result<Self, SharedOffscreenCaptureError> {
        let viewport = frame.viewport;
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
                RasterScope::Regions(rasterize_regions(frame, regions, &request.attachments)?)
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
        validate_prepared_text_capture_budget(
            viewport.physical_width,
            viewport.physical_height,
            crop,
            &scope,
            &request.attachments,
            PreparedTextCaptureBudget::STANDARD,
        )?;

        Ok(Self {
            frame_width: viewport.physical_width,
            frame_height: viewport.physical_height,
            crop,
            attachments: request.attachments.clone(),
            scope,
        })
    }

    fn begin_capture(
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
                    (
                        RasterScope::Regions(_),
                        CaptureAttachment::ObjectId | CaptureAttachment::Mask,
                    ) => {}
                    (RasterScope::WholeFrame, CaptureAttachment::ObjectId) => {
                        return Err(SharedOffscreenCaptureError::ObjectIdRequiresRegions);
                    }
                }
                Ok(CapturedAttachment { attachment, rgba })
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

    fn stamp_bounds_region(&self, capture: &mut SharedFrameCapture, region: &RasterRegion) {
        for attachment in &mut capture.attachments {
            match attachment.attachment {
                CaptureAttachment::Color => {}
                CaptureAttachment::ObjectId => fill_rect(
                    &mut attachment.rgba,
                    self.crop,
                    region.rect,
                    region.object_id_rgba,
                ),
                CaptureAttachment::Mask => {
                    fill_rect(&mut attachment.rgba, self.crop, region.rect, [u8::MAX; 4]);
                }
            }
        }
    }

    fn stamp_prepared_text_region(
        &self,
        capture: &mut SharedFrameCapture,
        region_index: usize,
        region: &RasterRegion,
        coverage: &PreparedTextAlphaCoverage,
    ) -> Result<(), SharedOffscreenCaptureError> {
        self.validate_prepared_text_coverage(region_index, region, coverage)?;
        for attachment in &mut capture.attachments {
            let value = match attachment.attachment {
                CaptureAttachment::Color => continue,
                CaptureAttachment::ObjectId => region.object_id_rgba,
                CaptureAttachment::Mask => [u8::MAX; 4],
            };
            stamp_alpha_coverage(
                &mut attachment.rgba,
                self.crop,
                region.rect,
                coverage,
                value,
            );
        }
        Ok(())
    }

    #[cfg(test)]
    fn derive(
        &self,
        rendered_color: &[u8],
        prepared_text_coverages: &[PreparedTextAlphaCoverage],
    ) -> Result<SharedFrameCapture, SharedOffscreenCaptureError> {
        self.validate_prepared_text_coverages(prepared_text_coverages)?;
        let mut capture = self.begin_capture(rendered_color)?;
        if let RasterScope::Regions(regions) = &self.scope {
            for (region_index, region) in regions.iter().enumerate() {
                match &region.coverage {
                    RasterCoverage::Bounds => self.stamp_bounds_region(&mut capture, region),
                    RasterCoverage::PreparedText(_) if self.needs_debug_coverage() => {
                        let rect = intersect(region.rect, self.crop);
                        if rect.is_empty() {
                            continue;
                        }
                        self.stamp_prepared_text_region(
                            &mut capture,
                            region_index,
                            region,
                            Self::prepared_text_coverage(region_index, prepared_text_coverages)?,
                        )?;
                    }
                    RasterCoverage::PreparedText(_) => {}
                }
            }
        }
        Ok(capture)
    }

    fn needs_debug_coverage(&self) -> bool {
        self.attachments.iter().any(|attachment| {
            matches!(
                attachment,
                CaptureAttachment::ObjectId | CaptureAttachment::Mask
            )
        })
    }

    #[cfg(test)]
    fn needs_prepared_text_coverage(&self) -> bool {
        self.needs_debug_coverage()
            && self
                .prepared_text_regions()
                .any(|(region_index, _)| !self.prepared_text_region_rect(region_index).is_empty())
    }

    #[cfg(test)]
    fn prepared_text_regions(&self) -> impl Iterator<Item = (usize, &PreparedTextSelection)> {
        let regions = match &self.scope {
            RasterScope::Regions(regions) => regions.as_slice(),
            RasterScope::WholeFrame => &[],
        };
        regions
            .iter()
            .enumerate()
            .filter_map(|(region_index, region)| match &region.coverage {
                RasterCoverage::Bounds => None,
                RasterCoverage::PreparedText(selection) => Some((region_index, selection)),
            })
    }

    #[cfg(test)]
    fn prepared_text_region_rect(&self, region_index: usize) -> PixelRect {
        let RasterScope::Regions(regions) = &self.scope else {
            return PixelRect::full(0, 0);
        };
        regions
            .get(region_index)
            .map_or(PixelRect::full(0, 0), |region| {
                intersect(region.rect, self.crop)
            })
    }

    fn validate_prepared_text_coverage(
        &self,
        region_index: usize,
        region: &RasterRegion,
        coverage: &PreparedTextAlphaCoverage,
    ) -> Result<(), SharedOffscreenCaptureError> {
        if coverage.region_index != region_index {
            return Err(
                SharedOffscreenCaptureError::UnexpectedPreparedTextCoverage {
                    region_index: coverage.region_index,
                },
            );
        }
        let expected_rect = intersect(region.rect, self.crop);
        if coverage.rect.left != expected_rect.left || coverage.rect.top != expected_rect.top {
            return Err(
                SharedOffscreenCaptureError::PreparedTextCoverageOriginMismatch {
                    region_index,
                    expected_x: expected_rect.left,
                    expected_y: expected_rect.top,
                    actual_x: coverage.rect.left,
                    actual_y: coverage.rect.top,
                },
            );
        }
        if coverage.rect.width() != expected_rect.width()
            || coverage.rect.height() != expected_rect.height()
        {
            return Err(
                SharedOffscreenCaptureError::PreparedTextCoverageExtentMismatch {
                    region_index,
                    expected_width: expected_rect.width(),
                    expected_height: expected_rect.height(),
                    actual_width: coverage.rect.width(),
                    actual_height: coverage.rect.height(),
                },
            );
        }
        let expected =
            usize::try_from(u64::from(expected_rect.width()) * u64::from(expected_rect.height()))
                .map_err(|_| SharedOffscreenCaptureError::CaptureExtentOverflow {
                width: expected_rect.width(),
                height: expected_rect.height(),
            })?;
        if coverage.alpha.len() != expected {
            return Err(
                SharedOffscreenCaptureError::PreparedTextCoverageSizeMismatch {
                    region_index,
                    expected,
                    actual: coverage.alpha.len(),
                },
            );
        }
        Ok(())
    }

    #[cfg(test)]
    fn validate_prepared_text_coverages(
        &self,
        coverages: &[PreparedTextAlphaCoverage],
    ) -> Result<(), SharedOffscreenCaptureError> {
        if !self.needs_prepared_text_coverage() {
            if let Some(coverage) = coverages.first() {
                return Err(
                    SharedOffscreenCaptureError::UnexpectedPreparedTextCoverage {
                        region_index: coverage.region_index,
                    },
                );
            }
            return Ok(());
        }
        let expected_regions = self
            .prepared_text_regions()
            .map(|(region_index, _)| region_index)
            .filter(|region_index| !self.prepared_text_region_rect(*region_index).is_empty())
            .collect::<HashSet<_>>();
        let mut seen = HashSet::with_capacity(coverages.len());
        for coverage in coverages {
            let region_index = coverage.region_index;
            if !expected_regions.contains(&region_index) {
                return Err(
                    SharedOffscreenCaptureError::UnexpectedPreparedTextCoverage { region_index },
                );
            }
            if !seen.insert(region_index) {
                return Err(SharedOffscreenCaptureError::DuplicatePreparedTextCoverage {
                    region_index,
                });
            }
            let RasterScope::Regions(regions) = &self.scope else {
                unreachable!("prepared-text regions require a region scope");
            };
            self.validate_prepared_text_coverage(region_index, &regions[region_index], coverage)?;
        }
        if let Some((region_index, _)) = self.prepared_text_regions().find(|(region_index, _)| {
            !self.prepared_text_region_rect(*region_index).is_empty()
                && !seen.contains(region_index)
        }) {
            return Err(SharedOffscreenCaptureError::MissingPreparedTextCoverage { region_index });
        }
        Ok(())
    }

    #[cfg(test)]
    fn prepared_text_coverage(
        region_index: usize,
        coverages: &[PreparedTextAlphaCoverage],
    ) -> Result<&PreparedTextAlphaCoverage, SharedOffscreenCaptureError> {
        coverages
            .iter()
            .find(|coverage| coverage.region_index == region_index)
            .ok_or(SharedOffscreenCaptureError::MissingPreparedTextCoverage { region_index })
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
    frame: &PreparedFrame,
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
                if region.object_id_rgba[3] != u8::MAX {
                    return Err(SharedOffscreenCaptureError::NonOpaqueObjectId {
                        id: region.id.clone(),
                        rgba: region.object_id_rgba,
                    });
                }
                if !object_ids.insert(region.object_id_rgba) {
                    return Err(SharedOffscreenCaptureError::DuplicateObjectIdRgba {
                        rgba: region.object_id_rgba,
                    });
                }
            }
            let coverage = match &region.geometry {
                CaptureRegionGeometry::Bounds => RasterCoverage::Bounds,
                CaptureRegionGeometry::PreparedText(selection) => {
                    let item = frame.text.get(selection.text()).ok_or_else(|| {
                        SharedOffscreenCaptureError::MissingPreparedTextItem {
                            id: region.id.clone(),
                            text_index: selection.text().index(),
                        }
                    })?;
                    if let Some(glyph_index) =
                        selection
                            .glyph_indices()
                            .iter()
                            .copied()
                            .find(|glyph_index| {
                                usize::try_from(*glyph_index)
                                    .ok()
                                    .is_none_or(|index| index >= item.glyphs.len())
                            })
                    {
                        return Err(SharedOffscreenCaptureError::MissingPreparedTextGlyph {
                            id: region.id.clone(),
                            text_index: selection.text().index(),
                            glyph_index,
                            glyph_count: item.glyphs.len(),
                        });
                    }
                    RasterCoverage::PreparedText(selection.clone())
                }
            };
            Ok(RasterRegion {
                rect: physical_rect(frame.viewport, region.bounds),
                object_id_rgba: region.object_id_rgba,
                coverage,
            })
        })
        .collect()
}

fn prepared_text_coverage_frame(
    frame: &PreparedFrame,
    selection: &PreparedTextSelection,
) -> PreparedFrame {
    let mut attachment = frame.clone();
    attachment.retain_prepared_text_coverage_paint(selection.text());
    let text_ids = attachment
        .text
        .iter()
        .map(|(text, _)| text)
        .collect::<Vec<_>>();
    for text in text_ids {
        let Some(item) = attachment.text.get_mut(text) else {
            continue;
        };
        for (index, paint) in item.paint.glyphs.iter_mut().enumerate() {
            let selected = text == selection.text()
                && u32::try_from(index)
                    .ok()
                    .is_some_and(|glyph| selection.glyph_indices().binary_search(&glyph).is_ok());
            paint.visible &= selected;
            if selected {
                paint.color = TextColor::rgba(u8::MAX, u8::MAX, u8::MAX, u8::MAX);
            }
        }
        item.interaction = arcweft_glyphon::TextInteractionPlan::default();
    }
    attachment
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

fn stamp_alpha_coverage(
    output: &mut [u8],
    crop: PixelRect,
    region: PixelRect,
    coverage: &PreparedTextAlphaCoverage,
    value: [u8; 4],
) {
    let clipped = intersect(intersect(crop, region), coverage.rect);
    for y in clipped.top..clipped.bottom {
        for x in clipped.left..clipped.right {
            let source_index = usize::try_from(
                u64::from(y - coverage.rect.top) * u64::from(coverage.rect.width())
                    + u64::from(x - coverage.rect.left),
            )
            .expect("validated alpha extent fits usize");
            let Some(alpha) = coverage.alpha.get(source_index) else {
                continue;
            };
            if *alpha == 0 {
                continue;
            }
            let output_index = pixel_index(x - crop.left, y - crop.top, crop.width());
            output[output_index..output_index + 4].copy_from_slice(&value);
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

#[cfg(test)]
mod tests;
