use super::{
    CaptureAttachment, CaptureCropPolicy, CapturePlan, CaptureRegion, CaptureRequest, CaptureScope,
    PixelRect, PreparedTextAlphaCoverage, PreparedTextCaptureBudget,
    PreparedTextCaptureBudgetMetric, PreparedTextSelection, PreparedTextSelectionError,
    RasterCoverage, RasterRegion, RasterScope, SharedOffscreenCaptureError,
    validate_prepared_text_capture_budget,
};
use crate::geometry::{
    ChoiceScroll, InteractionVisualState, PreparedFrame, RenderPreferences, RenderScene,
    RenderViewport, SharedFramePlanContext,
};
use crate::view_scene::PreparedTextId;
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

fn prepared_frame(viewport: RenderViewport) -> PreparedFrame {
    SharedFramePlanContext::new()
        .prepare(&RenderScene {
            content_avoidance_regions: Vec::new(),
            choices: Vec::new(),
            text_inputs: Vec::new(),
            action_buttons: Vec::new(),
            focus_groups: Vec::new(),
            focus_navigation: Vec::new(),
            images: Vec::new(),
            viewport,
            visual_time_millis: 0,
            preferences: RenderPreferences::default(),
            interaction: InteractionVisualState::default(),
            choice_scroll: ChoiceScroll::default(),
            scroll_regions: Vec::new(),
        })
        .expect("empty test frame prepares without project fonts")
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

fn prepared_raster_region(
    text_index: u32,
    glyph_index: u32,
    object_id_rgba: [u8; 4],
) -> RasterRegion {
    RasterRegion {
        rect: PixelRect::full(4, 3),
        object_id_rgba,
        coverage: RasterCoverage::PreparedText(
            PreparedTextSelection::try_new(PreparedTextId::from_index(text_index), [glyph_index])
                .expect("test selection is valid"),
        ),
    }
}

fn alpha_coverage(region_index: usize, opaque_pixels: &[(u32, u32)]) -> PreparedTextAlphaCoverage {
    let mut alpha = vec![0; 12];
    for &(x, y) in opaque_pixels {
        let index =
            usize::try_from(u64::from(y) * 4 + u64::from(x)).expect("test pixel index fits usize");
        alpha[index] = u8::MAX;
    }
    PreparedTextAlphaCoverage {
        region_index,
        rect: PixelRect::full(4, 3),
        alpha,
    }
}

fn direct_plan(attachments: Vec<CaptureAttachment>, regions: Vec<RasterRegion>) -> CapturePlan {
    CapturePlan {
        frame_width: 4,
        frame_height: 3,
        crop: PixelRect::full(4, 3),
        attachments,
        scope: RasterScope::Regions(regions),
    }
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
    let capture = CapturePlan::new(&prepared_frame(viewport()), &request)
        .expect("request validates")
        .derive(&rendered_color(), &[])
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
    let capture = CapturePlan::new(&prepared_frame(viewport()), &request)
        .expect("request validates")
        .derive(&rendered_color(), &[])
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
fn prepared_and_bounds_coverage_replay_in_region_order() {
    let prepared_id = [10, 20, 30, u8::MAX];
    let bounds_id = [40, 50, 60, u8::MAX];
    let bounds = RasterRegion {
        rect: PixelRect {
            left: 1,
            top: 1,
            right: 2,
            bottom: 2,
        },
        object_id_rgba: bounds_id,
        coverage: RasterCoverage::Bounds,
    };

    let prepared_then_bounds = direct_plan(
        vec![CaptureAttachment::ObjectId],
        vec![prepared_raster_region(0, 0, prepared_id), bounds.clone()],
    )
    .derive(&rendered_color(), &[alpha_coverage(0, &[(1, 1)])])
    .expect("ordered attachment derives");
    assert_eq!(
        pixel(
            prepared_then_bounds
                .attachment_rgba(CaptureAttachment::ObjectId)
                .expect("object-id exists"),
            4,
            1,
            1,
        ),
        bounds_id
    );

    let bounds_then_prepared = direct_plan(
        vec![CaptureAttachment::ObjectId],
        vec![bounds, prepared_raster_region(0, 0, prepared_id)],
    )
    .derive(&rendered_color(), &[alpha_coverage(1, &[(1, 1)])])
    .expect("ordered attachment derives");
    assert_eq!(
        pixel(
            bounds_then_prepared
                .attachment_rgba(CaptureAttachment::ObjectId)
                .expect("object-id exists"),
            4,
            1,
            1,
        ),
        prepared_id
    );
}

#[test]
fn later_prepared_coverage_wins_without_rgb_decoding() {
    let first = [7, 19, 113, u8::MAX];
    let second = [211, 31, 5, u8::MAX];
    let capture = direct_plan(
        vec![CaptureAttachment::ObjectId],
        vec![
            prepared_raster_region(0, 0, first),
            prepared_raster_region(1, 0, second),
        ],
    )
    .derive(
        &rendered_color(),
        &[alpha_coverage(0, &[(2, 1)]), alpha_coverage(1, &[(2, 1)])],
    )
    .expect("prepared coverages derive");
    assert_eq!(
        pixel(
            capture
                .attachment_rgba(CaptureAttachment::ObjectId)
                .expect("object-id exists"),
            4,
            2,
            1,
        ),
        second
    );
}

#[test]
fn prepared_mask_uses_alpha_coverage_not_object_id_alpha() {
    let capture = direct_plan(
        vec![CaptureAttachment::Mask],
        vec![prepared_raster_region(0, 0, [17, 29, 41, 0])],
    )
    .derive(&rendered_color(), &[alpha_coverage(0, &[(3, 2)])])
    .expect("mask derives from alpha");
    let mask = capture
        .attachment_rgba(CaptureAttachment::Mask)
        .expect("mask exists");
    assert_eq!(pixel(mask, 4, 3, 2), [u8::MAX; 4]);
    assert_eq!(pixel(mask, 4, 0, 0), [0; 4]);
}

#[test]
fn prepared_coverage_uses_its_cropped_frame_origin() {
    let mut region = prepared_raster_region(0, 0, [9, 8, 7, u8::MAX]);
    region.rect = PixelRect {
        left: 1,
        top: 1,
        right: 2,
        bottom: 2,
    };
    let capture = direct_plan(vec![CaptureAttachment::ObjectId], vec![region])
        .derive(
            &rendered_color(),
            &[PreparedTextAlphaCoverage {
                region_index: 0,
                rect: PixelRect {
                    left: 1,
                    top: 1,
                    right: 2,
                    bottom: 2,
                },
                alpha: vec![u8::MAX],
            }],
        )
        .expect("cropped coverage derives");
    let object_id = capture
        .attachment_rgba(CaptureAttachment::ObjectId)
        .expect("object-id exists");
    assert_eq!(pixel(object_id, 4, 0, 0), [0; 4]);
    assert_eq!(pixel(object_id, 4, 1, 1), [9, 8, 7, u8::MAX]);
}

#[test]
fn prepared_coverage_extent_and_size_mismatch_are_typed() {
    let plan = direct_plan(
        vec![CaptureAttachment::Mask],
        vec![prepared_raster_region(0, 0, [1, 2, 3, u8::MAX])],
    );
    assert!(matches!(
        plan.derive(
            &rendered_color(),
            &[PreparedTextAlphaCoverage {
                region_index: 0,
                rect: PixelRect {
                    left: 0,
                    top: 0,
                    right: 3,
                    bottom: 3,
                },
                alpha: vec![u8::MAX; 9],
            }],
        ),
        Err(SharedOffscreenCaptureError::PreparedTextCoverageExtentMismatch { .. })
    ));
    assert!(matches!(
        plan.derive(
            &rendered_color(),
            &[PreparedTextAlphaCoverage {
                region_index: 0,
                rect: PixelRect::full(4, 3),
                alpha: vec![u8::MAX; 11],
            }],
        ),
        Err(SharedOffscreenCaptureError::PreparedTextCoverageSizeMismatch { .. })
    ));
    assert!(matches!(
        plan.derive(&rendered_color(), &[]),
        Err(SharedOffscreenCaptureError::MissingPreparedTextCoverage { region_index: 0 })
    ));

    let color_only = direct_plan(
        vec![CaptureAttachment::Color],
        vec![prepared_raster_region(0, 0, [1, 2, 3, u8::MAX])],
    );
    assert!(matches!(
        color_only.derive(&rendered_color(), &[alpha_coverage(0, &[(0, 0)])]),
        Err(SharedOffscreenCaptureError::UnexpectedPreparedTextCoverage { region_index: 0 })
    ));
}

#[test]
fn prepared_coverage_origin_mismatch_is_typed() {
    let plan = direct_plan(
        vec![CaptureAttachment::Mask],
        vec![prepared_raster_region(0, 0, [1, 2, 3, u8::MAX])],
    );
    assert!(matches!(
        plan.derive(
            &rendered_color(),
            &[PreparedTextAlphaCoverage {
                region_index: 0,
                rect: PixelRect {
                    left: 1,
                    top: 0,
                    right: 5,
                    bottom: 3,
                },
                alpha: vec![u8::MAX; 12],
            }],
        ),
        Err(
            SharedOffscreenCaptureError::PreparedTextCoverageOriginMismatch {
                region_index: 0,
                expected_x: 0,
                expected_y: 0,
                actual_x: 1,
                actual_y: 0,
            }
        )
    ));
}

#[test]
fn prepared_text_capture_budget_accepts_exact_limits_and_rejects_one_over() {
    let two_regions = RasterScope::Regions(vec![
        prepared_raster_region(0, 0, [1, 0, 0, u8::MAX]),
        prepared_raster_region(1, 0, [2, 0, 0, u8::MAX]),
    ]);
    let exact = PreparedTextCaptureBudget {
        passes: 2,
        render_pixels: 24,
        readback_bytes: 1_536,
    };
    validate_prepared_text_capture_budget(
        4,
        3,
        PixelRect::full(4, 3),
        &two_regions,
        &[CaptureAttachment::Mask],
        exact,
    )
    .expect("exact pass, render-pixel, and padded RGBA readback limits are accepted");

    let three_regions = RasterScope::Regions(vec![
        prepared_raster_region(0, 0, [1, 0, 0, u8::MAX]),
        prepared_raster_region(1, 0, [2, 0, 0, u8::MAX]),
        prepared_raster_region(2, 0, [3, 0, 0, u8::MAX]),
    ]);
    assert!(matches!(
        validate_prepared_text_capture_budget(
            4,
            3,
            PixelRect::full(4, 3),
            &three_regions,
            &[CaptureAttachment::Mask],
            PreparedTextCaptureBudget {
                passes: 2,
                render_pixels: u64::MAX,
                readback_bytes: u64::MAX,
            },
        ),
        Err(
            SharedOffscreenCaptureError::PreparedTextCoveragePassBudgetExceeded {
                actual: 3,
                limit: 2,
            }
        )
    ));
    assert!(matches!(
        validate_prepared_text_capture_budget(
            4,
            3,
            PixelRect::full(4, 3),
            &three_regions,
            &[CaptureAttachment::Mask],
            PreparedTextCaptureBudget {
                passes: 3,
                render_pixels: 24,
                readback_bytes: u64::MAX,
            },
        ),
        Err(
            SharedOffscreenCaptureError::PreparedTextCoverageRenderBudgetExceeded {
                actual: 36,
                limit: 24,
            }
        )
    ));
    assert!(matches!(
        validate_prepared_text_capture_budget(
            4,
            3,
            PixelRect::full(4, 3),
            &three_regions,
            &[CaptureAttachment::Mask],
            PreparedTextCaptureBudget {
                passes: 3,
                render_pixels: 36,
                readback_bytes: 1_536,
            },
        ),
        Err(
            SharedOffscreenCaptureError::PreparedTextCoverageReadbackBudgetExceeded {
                actual: 2_304,
                limit: 1_536,
            }
        )
    ));
}

#[test]
fn prepared_text_capture_budget_counts_padded_rows_for_thin_tall_regions() {
    let thin_tall_region = RasterRegion {
        rect: PixelRect::full(1, 1_024),
        object_id_rgba: [1, 0, 0, u8::MAX],
        coverage: RasterCoverage::PreparedText(
            PreparedTextSelection::try_new(PreparedTextId::from_index(0), [0]).expect("selection"),
        ),
    };
    let scope = RasterScope::Regions(vec![thin_tall_region]);
    let exact_readback_bytes = 256 * 1_024;
    let exact = PreparedTextCaptureBudget {
        passes: 1,
        render_pixels: 1_024,
        readback_bytes: exact_readback_bytes,
    };
    validate_prepared_text_capture_budget(
        1,
        1_024,
        PixelRect::full(1, 1_024),
        &scope,
        &[CaptureAttachment::Mask],
        exact,
    )
    .expect("one-byte-wide rows are charged at the 256-byte GPU transfer alignment");

    assert!(matches!(
        validate_prepared_text_capture_budget(
            1,
            1_024,
            PixelRect::full(1, 1_024),
            &scope,
            &[CaptureAttachment::Mask],
            PreparedTextCaptureBudget {
                readback_bytes: exact_readback_bytes - 1,
                ..exact
            },
        ),
        Err(
            SharedOffscreenCaptureError::PreparedTextCoverageReadbackBudgetExceeded {
                actual: 262_144,
                limit: 262_143,
            }
        )
    ));
}

#[test]
fn prepared_text_capture_budget_reports_checked_arithmetic_overflow() {
    let scope = RasterScope::Regions(vec![
        prepared_raster_region(0, 0, [1, 0, 0, u8::MAX]),
        prepared_raster_region(1, 0, [2, 0, 0, u8::MAX]),
    ]);
    assert!(matches!(
        validate_prepared_text_capture_budget(
            u32::MAX,
            u32::MAX,
            PixelRect::full(4, 3),
            &scope,
            &[CaptureAttachment::Mask],
            PreparedTextCaptureBudget {
                passes: u64::MAX,
                render_pixels: u64::MAX,
                readback_bytes: u64::MAX,
            },
        ),
        Err(
            SharedOffscreenCaptureError::PreparedTextCoverageBudgetOverflow {
                metric: PreparedTextCaptureBudgetMetric::RenderPixels,
            }
        )
    ));

    let huge_region = RasterRegion {
        rect: PixelRect::full(u32::MAX, u32::MAX),
        object_id_rgba: [1, 0, 0, u8::MAX],
        coverage: RasterCoverage::PreparedText(
            PreparedTextSelection::try_new(PreparedTextId::from_index(0), [0]).expect("selection"),
        ),
    };
    assert!(matches!(
        validate_prepared_text_capture_budget(
            1,
            1,
            PixelRect::full(u32::MAX, u32::MAX),
            &RasterScope::Regions(vec![huge_region]),
            &[CaptureAttachment::Mask],
            PreparedTextCaptureBudget {
                passes: u64::MAX,
                render_pixels: u64::MAX,
                readback_bytes: u64::MAX,
            },
        ),
        Err(
            SharedOffscreenCaptureError::PreparedTextCoverageBudgetOverflow {
                metric: PreparedTextCaptureBudgetMetric::ReadbackBytes,
            }
        )
    ));

    let maximum_aligned_width = 1_073_741_760;
    let maximum_sized_region = || RasterRegion {
        rect: PixelRect::full(maximum_aligned_width, u32::MAX),
        object_id_rgba: [1, 0, 0, u8::MAX],
        coverage: RasterCoverage::PreparedText(
            PreparedTextSelection::try_new(PreparedTextId::from_index(0), [0]).expect("selection"),
        ),
    };
    assert!(matches!(
        validate_prepared_text_capture_budget(
            1,
            1,
            PixelRect::full(maximum_aligned_width, u32::MAX),
            &RasterScope::Regions(vec![maximum_sized_region(), maximum_sized_region()]),
            &[CaptureAttachment::Mask],
            PreparedTextCaptureBudget {
                passes: u64::MAX,
                render_pixels: u64::MAX,
                readback_bytes: u64::MAX,
            },
        ),
        Err(
            SharedOffscreenCaptureError::PreparedTextCoverageBudgetOverflow {
                metric: PreparedTextCaptureBudgetMetric::ReadbackBytes,
            }
        )
    ));
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
    let capture = CapturePlan::new(&prepared_frame(viewport()), &request)
        .expect("request validates")
        .derive(&source, &[])
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
    let capture = CapturePlan::new(&prepared_frame(scaled), &request)
        .expect("request validates")
        .derive(&[0; 8 * 6 * 4], &[])
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
        CapturePlan::new(&prepared_frame(viewport()), &empty),
        Err(SharedOffscreenCaptureError::EmptyAttachments)
    ));

    let duplicate = CaptureRequest::new(
        [CaptureAttachment::Color, CaptureAttachment::Color],
        CaptureScope::WholeFrame,
        CaptureCropPolicy::FullFrame,
    );
    assert!(matches!(
        CapturePlan::new(&prepared_frame(viewport()), &duplicate),
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
        CapturePlan::new(&prepared_frame(viewport()), &object_without_regions),
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
        CapturePlan::new(&prepared_frame(viewport()), &invalid_bounds),
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
        CapturePlan::new(&prepared_frame(viewport()), &outside),
        Err(SharedOffscreenCaptureError::EmptyScopeBounds)
    ));

    assert_eq!(
        PreparedTextSelection::try_new(PreparedTextId::from_index(0), []),
        Err(PreparedTextSelectionError::Empty)
    );
    assert_eq!(
        PreparedTextSelection::try_new(PreparedTextId::from_index(0), [3, 3]),
        Err(PreparedTextSelectionError::DuplicateGlyph { glyph_index: 3 })
    );
    assert_eq!(
        PreparedTextSelection::try_new(PreparedTextId::from_index(0), [3, 1, 2])
            .expect("selection canonicalizes")
            .glyph_indices(),
        &[1, 2, 3]
    );

    let missing_prepared_text = CaptureRequest::new(
        [CaptureAttachment::Mask],
        CaptureScope::Regions(vec![CaptureRegion::prepared_text(
            PublicId::try_new("capture.missing_text").expect("test id is valid"),
            HitRect::new(0.0, 0.0, 1.0, 1.0),
            [1, 0, 0, u8::MAX],
            PreparedTextSelection::try_new(PreparedTextId::from_index(0), [0])
                .expect("selection is valid"),
        )]),
        CaptureCropPolicy::FullFrame,
    );
    assert!(matches!(
        CapturePlan::new(&prepared_frame(viewport()), &missing_prepared_text),
        Err(SharedOffscreenCaptureError::MissingPreparedTextItem { .. })
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
        CapturePlan::new(&prepared_frame(viewport()), &transparent),
        Err(SharedOffscreenCaptureError::NonOpaqueObjectId {
            rgba: [0, 0, 0, 0],
            ..
        })
    ));

    let translucent = CaptureRequest::new(
        [CaptureAttachment::ObjectId],
        CaptureScope::Regions(vec![region(
            "capture.translucent",
            HitRect::new(0.0, 0.0, 1.0, 1.0),
            [1, 2, 3, 254],
        )]),
        CaptureCropPolicy::FullFrame,
    );
    assert!(matches!(
        CapturePlan::new(&prepared_frame(viewport()), &translucent),
        Err(SharedOffscreenCaptureError::NonOpaqueObjectId {
            rgba: [1, 2, 3, 254],
            ..
        })
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
        CapturePlan::new(&prepared_frame(viewport()), &duplicate_id),
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
        CapturePlan::new(&prepared_frame(viewport()), &duplicate_rgba),
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
