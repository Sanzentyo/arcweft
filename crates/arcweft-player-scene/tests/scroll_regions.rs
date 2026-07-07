use arcweft_bundle::resource_codec::{
    ViewRuntimeControlStyle, ViewRuntimeScrollRegion, ViewRuntimeScrollRegionBounds,
    ViewRuntimeTextBlock, ViewRuntimeTextBlockBounds,
};
use arcweft_bundle::resource_codec::{ViewScrollAxis, ViewScrollOverflowPolicy};
use arcweft_player_scene::{
    frame::{PlayerFrameFit, PlayerFramePlanner, PlayerFrameRequest},
    images::BundleImageCatalog,
    input::{
        InputController, InputControllerSnapshot, InputControllerSnapshotError,
        InputScrollOffsetSnapshot,
    },
};
use arcweft_presentation::input::{PointerId, ViewportPoint};
use arcweft_render_wgpu::geometry::{RenderPreferences, RenderViewport};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;

#[test]
fn player_frame_plans_runtime_scroll_regions_and_applies_input_offset() {
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.scroll_regions.push(ViewRuntimeScrollRegion {
        public_id: "scroll.feedback.body".to_owned(),
        target: "scroll.feedback.body".to_owned(),
        view: Some("view.FeedbackPanel".to_owned()),
        bounds: ViewRuntimeScrollRegionBounds::new(48_000, 64_000, 420_000, 120_000),
        content_width_milli: 420_000,
        content_height_milli: 360_000,
        axis: ViewScrollAxis::Vertical,
        overflow: ViewScrollOverflowPolicy::Auto,
    });
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        images: &images,
        viewport: RenderViewport {
            logical_width: 1280.0,
            logical_height: 720.0,
            physical_width: 1280,
            physical_height: 720,
            scale_factor: 1.0,
        },
        fit: PlayerFrameFit::raw(),
        image_time_millis: 0,
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
    };

    let prepared = PlayerFramePlanner::prepare(&mut input, request).expect("frame prepares");
    let region = prepared
        .frame
        .scroll_regions
        .first()
        .expect("scroll region");
    assert_eq!(region.id, "scroll.feedback.body");
    assert!((region.bounds.x - 48.0).abs() < f32::EPSILON);
    assert!((region.bounds.y - 64.0).abs() < f32::EPSILON);
    assert!((region.bounds.width - 420.0).abs() < f32::EPSILON);
    assert!((region.bounds.height - 120.0).abs() < f32::EPSILON);
    assert!((region.content_width - 420.0).abs() < f32::EPSILON);
    assert!((region.content_height - 360.0).abs() < f32::EPSILON);
    assert!(region.offset_x.abs() < f32::EPSILON);
    assert!(region.offset_y.abs() < f32::EPSILON);

    input.pointer_move(
        &prepared.frame,
        PointerId(0),
        ViewportPoint::new(64.0, 80.0),
    );
    input.wheel(&prepared.frame, -90.0);

    let prepared = PlayerFramePlanner::prepare(&mut input, request).expect("frame re-prepares");
    let region = prepared
        .frame
        .scroll_regions
        .first()
        .expect("scroll region");
    assert!((region.offset_y - 90.0).abs() < f32::EPSILON);

    let snapshot = input.snapshot();
    assert_eq!(
        snapshot.scroll_offsets,
        vec![InputScrollOffsetSnapshot {
            region_id: "scroll.feedback.body".to_owned(),
            offset_x: 0.0,
            offset_y: 90.0,
        }]
    );

    let mut restored_input = InputController::default();
    restored_input
        .restore_snapshot(snapshot)
        .expect("input snapshot restores");
    let prepared =
        PlayerFramePlanner::prepare(&mut restored_input, request).expect("restored frame prepares");
    let region = prepared
        .frame
        .scroll_regions
        .first()
        .expect("scroll region");
    assert!((region.offset_y - 90.0).abs() < f32::EPSILON);
}

#[test]
fn hidden_overflow_scroll_region_keeps_offset_at_zero() {
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.scroll_regions.push(ViewRuntimeScrollRegion {
        public_id: "scroll.feedback.body".to_owned(),
        target: "scroll.feedback.body".to_owned(),
        view: Some("view.FeedbackPanel".to_owned()),
        bounds: ViewRuntimeScrollRegionBounds::new(48_000, 64_000, 420_000, 120_000),
        content_width_milli: 420_000,
        content_height_milli: 360_000,
        axis: ViewScrollAxis::Vertical,
        overflow: ViewScrollOverflowPolicy::Hidden,
    });
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        images: &images,
        viewport: RenderViewport {
            logical_width: 1280.0,
            logical_height: 720.0,
            physical_width: 1280,
            physical_height: 720,
            scale_factor: 1.0,
        },
        fit: PlayerFrameFit::raw(),
        image_time_millis: 0,
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
    };

    let prepared = PlayerFramePlanner::prepare(&mut input, request).expect("frame prepares");
    input.pointer_move(
        &prepared.frame,
        PointerId(0),
        ViewportPoint::new(64.0, 80.0),
    );
    input.wheel(&prepared.frame, -90.0);

    let prepared = PlayerFramePlanner::prepare(&mut input, request).expect("frame re-prepares");
    let region = prepared
        .frame
        .scroll_regions
        .first()
        .expect("scroll region");
    assert!(region.offset_y.abs() < f32::EPSILON);
    assert!(input.snapshot().scroll_offsets.is_empty());
}

#[test]
fn horizontal_scroll_region_tracks_x_offset_and_snapshot() {
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.scroll_regions.push(ViewRuntimeScrollRegion {
        public_id: "scroll.gallery".to_owned(),
        target: "scroll.gallery".to_owned(),
        view: Some("view.Gallery".to_owned()),
        bounds: ViewRuntimeScrollRegionBounds::new(48_000, 64_000, 240_000, 120_000),
        content_width_milli: 640_000,
        content_height_milli: 120_000,
        axis: ViewScrollAxis::Horizontal,
        overflow: ViewScrollOverflowPolicy::Auto,
    });
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        images: &images,
        viewport: RenderViewport {
            logical_width: 1280.0,
            logical_height: 720.0,
            physical_width: 1280,
            physical_height: 720,
            scale_factor: 1.0,
        },
        fit: PlayerFrameFit::raw(),
        image_time_millis: 0,
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
    };

    let prepared = PlayerFramePlanner::prepare(&mut input, request).expect("frame prepares");
    input.pointer_move(
        &prepared.frame,
        PointerId(0),
        ViewportPoint::new(64.0, 80.0),
    );
    input.wheel(&prepared.frame, -180.0);

    let prepared = PlayerFramePlanner::prepare(&mut input, request).expect("frame re-prepares");
    let region = prepared
        .frame
        .scroll_regions
        .first()
        .expect("scroll region");
    assert!((region.offset_x - 180.0).abs() < f32::EPSILON);
    assert!(region.offset_y.abs() < f32::EPSILON);
    assert!((input.scroll_offset_x("scroll.gallery") - 180.0).abs() < f32::EPSILON);
    assert!(input.scroll_offset_y("scroll.gallery").abs() < f32::EPSILON);

    assert_eq!(
        input.snapshot().scroll_offsets,
        vec![InputScrollOffsetSnapshot {
            region_id: "scroll.gallery".to_owned(),
            offset_x: 180.0,
            offset_y: 0.0,
        }]
    );
}

#[test]
fn player_frame_offsets_and_clips_scroll_contained_text_blocks() {
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.scroll_regions.push(ViewRuntimeScrollRegion {
        public_id: "scroll.notes".to_owned(),
        target: "scroll.notes".to_owned(),
        view: Some("view.NotesPanel".to_owned()),
        bounds: ViewRuntimeScrollRegionBounds::new(48_000, 64_000, 240_000, 64_000),
        content_width_milli: 240_000,
        content_height_milli: 180_000,
        axis: ViewScrollAxis::Vertical,
        overflow: ViewScrollOverflowPolicy::Auto,
    });
    presentation.text_blocks.push(ViewRuntimeTextBlock {
        public_id: "text.block.NotesPanel.0".to_owned(),
        target: "text.block.NotesPanel.0".to_owned(),
        view: Some("view.NotesPanel".to_owned()),
        containing_scroll_region: Some("scroll.notes".to_owned()),
        text: "Arcweft Concierge".to_owned(),
        bounds: ViewRuntimeTextBlockBounds::from_px(56, 112, 220, 24),
        style: ViewRuntimeControlStyle::default(),
    });
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        images: &images,
        viewport: RenderViewport {
            logical_width: 1280.0,
            logical_height: 720.0,
            physical_width: 1280,
            physical_height: 720,
            scale_factor: 1.0,
        },
        fit: PlayerFrameFit::raw(),
        image_time_millis: 0,
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
    };

    let prepared = PlayerFramePlanner::prepare(&mut input, request).expect("frame prepares");
    input.pointer_move(
        &prepared.frame,
        PointerId(0),
        ViewportPoint::new(64.0, 80.0),
    );
    input.wheel(&prepared.frame, -32.0);

    let prepared = PlayerFramePlanner::prepare(&mut input, request).expect("frame re-prepares");
    let text = prepared.frame.text.first().expect("text block");
    assert_eq!(text.text, "Arcweft Concierge");
    assert!((text.bounds.y - 80.0).abs() < f32::EPSILON);
    assert_eq!(
        text.clip_bounds,
        Some(prepared.frame.scroll_regions[0].bounds)
    );
}

#[test]
fn input_snapshot_rejects_non_finite_scroll_offsets() {
    let mut input = InputController::default();
    let error = input
        .restore_snapshot(InputControllerSnapshot {
            choice_scroll_offset_y: 0.0,
            scroll_offsets: vec![InputScrollOffsetSnapshot {
                region_id: "scroll.feedback.body".to_owned(),
                offset_x: 0.0,
                offset_y: f32::NAN,
            }],
        })
        .expect_err("non-finite scroll offset rejects");

    assert!(matches!(
        error,
        InputControllerSnapshotError::NonFiniteScrollOffset { .. }
    ));
}

#[test]
fn input_snapshot_rejects_pre_xy_scroll_offset_shape() {
    let json = r#"{
        "scroll_offsets": [
            {
                "region_id": "scroll.feedback.body",
                "offset_y": 90.0
            }
        ]
    }"#;

    assert!(
        serde_json::from_str::<InputControllerSnapshot>(json).is_err(),
        "scroll offset snapshots must require explicit offset_x and offset_y",
    );
}
