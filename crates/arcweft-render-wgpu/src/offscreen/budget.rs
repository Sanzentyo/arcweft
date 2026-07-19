use super::{
    CaptureAttachment, MAX_PREPARED_TEXT_COVERAGE_PASSES,
    MAX_PREPARED_TEXT_COVERAGE_READBACK_BYTES, MAX_PREPARED_TEXT_COVERAGE_RENDER_PIXELS, PixelRect,
    PreparedTextCaptureBudgetMetric, RasterCoverage, RasterScope, SharedOffscreenCaptureError,
    intersect, readback::padded_rgba_row_bytes,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreparedTextCaptureBudget {
    pub(super) passes: u64,
    pub(super) render_pixels: u64,
    pub(super) readback_bytes: u64,
}

impl PreparedTextCaptureBudget {
    pub(super) const STANDARD: Self = Self {
        passes: MAX_PREPARED_TEXT_COVERAGE_PASSES,
        render_pixels: MAX_PREPARED_TEXT_COVERAGE_RENDER_PIXELS,
        readback_bytes: MAX_PREPARED_TEXT_COVERAGE_READBACK_BYTES,
    };
}

pub(super) fn validate_prepared_text_capture_budget(
    frame_width: u32,
    frame_height: u32,
    crop: PixelRect,
    scope: &RasterScope,
    attachments: &[CaptureAttachment],
    budget: PreparedTextCaptureBudget,
) -> Result<(), SharedOffscreenCaptureError> {
    let needs_coverage = attachments.iter().any(|attachment| {
        matches!(
            attachment,
            CaptureAttachment::ObjectId | CaptureAttachment::Mask
        )
    });
    let RasterScope::Regions(regions) = scope else {
        return Ok(());
    };
    if !needs_coverage {
        return Ok(());
    }

    let frame_pixels = u64::from(frame_width)
        .checked_mul(u64::from(frame_height))
        .ok_or(
            SharedOffscreenCaptureError::PreparedTextCoverageBudgetOverflow {
                metric: PreparedTextCaptureBudgetMetric::RenderPixels,
            },
        )?;
    let mut passes = 0_u64;
    let mut render_pixels = 0_u64;
    let mut readback_bytes = 0_u64;
    for region in regions {
        if !matches!(&region.coverage, RasterCoverage::PreparedText(_)) {
            continue;
        }
        let rect = intersect(region.rect, crop);
        if rect.is_empty() {
            continue;
        }

        passes = passes.checked_add(1).ok_or(
            SharedOffscreenCaptureError::PreparedTextCoverageBudgetOverflow {
                metric: PreparedTextCaptureBudgetMetric::Passes,
            },
        )?;
        if passes > budget.passes {
            return Err(
                SharedOffscreenCaptureError::PreparedTextCoveragePassBudgetExceeded {
                    actual: passes,
                    limit: budget.passes,
                },
            );
        }

        render_pixels = render_pixels.checked_add(frame_pixels).ok_or(
            SharedOffscreenCaptureError::PreparedTextCoverageBudgetOverflow {
                metric: PreparedTextCaptureBudgetMetric::RenderPixels,
            },
        )?;
        if render_pixels > budget.render_pixels {
            return Err(
                SharedOffscreenCaptureError::PreparedTextCoverageRenderBudgetExceeded {
                    actual: render_pixels,
                    limit: budget.render_pixels,
                },
            );
        }

        let padded_row_bytes = padded_rgba_row_bytes(rect.width()).map_err(|_| {
            SharedOffscreenCaptureError::PreparedTextCoverageBudgetOverflow {
                metric: PreparedTextCaptureBudgetMetric::ReadbackBytes,
            }
        })?;
        let region_readback_bytes = u64::from(padded_row_bytes)
            .checked_mul(u64::from(rect.height()))
            .ok_or(
                SharedOffscreenCaptureError::PreparedTextCoverageBudgetOverflow {
                    metric: PreparedTextCaptureBudgetMetric::ReadbackBytes,
                },
            )?;
        readback_bytes = readback_bytes.checked_add(region_readback_bytes).ok_or(
            SharedOffscreenCaptureError::PreparedTextCoverageBudgetOverflow {
                metric: PreparedTextCaptureBudgetMetric::ReadbackBytes,
            },
        )?;
        if readback_bytes > budget.readback_bytes {
            return Err(
                SharedOffscreenCaptureError::PreparedTextCoverageReadbackBudgetExceeded {
                    actual: readback_bytes,
                    limit: budget.readback_bytes,
                },
            );
        }
    }
    Ok(())
}
