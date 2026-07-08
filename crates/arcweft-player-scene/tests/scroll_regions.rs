use arcweft_bundle::resource_codec::view::{
    RgbaColor, ViewElementKind, ViewRuntimeControlCornerRadius, ViewRuntimeControlRadii,
    ViewTextSelectionPolicy,
};
use arcweft_bundle::resource_codec::{
    ViewRuntimeControlStyle, ViewRuntimeControlVisualStyle, ViewRuntimeScrollRegion,
    ViewRuntimeScrollRegionBounds, ViewRuntimeShadow, ViewRuntimeShadowKind, ViewRuntimeSurface,
    ViewRuntimeSurfaceBounds, ViewRuntimeTextBlock, ViewRuntimeTextBlockBounds,
};
use arcweft_bundle::resource_codec::{ViewScrollAxis, ViewScrollOverflowPolicy};
use arcweft_player_scene::{
    frame::{PlayerFrameFit, PlayerFramePlanner, PlayerFrameRequest},
    images::BundleImageCatalog,
    input::{
        InputController, InputControllerSnapshot, InputControllerSnapshotError,
        InputPointerModifiers, InputScrollOffsetSnapshot,
    },
};
use arcweft_presentation::input::{PointerId, ViewportPoint};
use arcweft_render_wgpu::geometry::{RenderPreferences, RenderViewport};
use arcweft_render_wgpu::view_scene::{ViewPaintNode, ViewPrimitive};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;

fn assert_px(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.001,
        "expected {expected}px, got {actual}px"
    );
}

#[test]
fn player_frame_lowers_runtime_surfaces_to_view_scene() {
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.surfaces.push(ViewRuntimeSurface {
        public_id: "surface.feedback.card".to_owned(),
        target: "surface.feedback.card".to_owned(),
        view: Some("view.FeedbackPanel".to_owned()),
        containing_scroll_region: None,
        element: ViewElementKind::Panel,
        bounds: ViewRuntimeSurfaceBounds::from_px(24, 32, 112, 72),
        style: ViewRuntimeControlStyle {
            normal: ViewRuntimeControlVisualStyle {
                fill: Some(RgbaColor::rgb(36, 42, 54)),
                radii_milli: Some(ViewRuntimeControlRadii::new(
                    ViewRuntimeControlCornerRadius::new(18_000, 12_000),
                    ViewRuntimeControlCornerRadius::new(10_000, 6_000),
                    ViewRuntimeControlCornerRadius::new(14_000, 8_000),
                    ViewRuntimeControlCornerRadius::new(6_000, 4_000),
                )),
                shadows: vec![ViewRuntimeShadow {
                    offset_x_milli: 0,
                    offset_y_milli: 3_000,
                    blur_milli: 12_000,
                    spread_milli: 2_000,
                    radius_milli: 14_000,
                    color: RgbaColor::rgba(0, 0, 0, 143),
                    kind: ViewRuntimeShadowKind::Inset,
                }],
                ..ViewRuntimeControlVisualStyle::default()
            },
            ..ViewRuntimeControlStyle::default()
        },
    });
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();

    let prepared = PlayerFramePlanner::prepare(
        &mut input,
        PlayerFrameRequest {
            presentation: &presentation,
            images: &images,
            viewport: RenderViewport {
                logical_width: 320.0,
                logical_height: 180.0,
                physical_width: 320,
                physical_height: 180,
                scale_factor: 1.0,
            },
            fit: PlayerFrameFit::raw(),
            image_time_millis: 0,
            visual_time_millis: 0,
            preferences: RenderPreferences::default(),
        },
    )
    .expect("frame prepares");

    let view_scene = prepared.frame.view_scenes().first().expect("surface scene");
    assert_eq!(view_scene.scene.primitives().len(), 1);
    assert_eq!(view_scene.scene.paint_nodes().len(), 1);
    let ViewPrimitive::RoundedRect(rect) = &view_scene.scene.primitives()[0] else {
        panic!("surface fill lowers to a rounded rect primitive");
    };
    assert_px(rect.radii.top_left.x_px, 18.0);
    assert_px(rect.radii.top_left.y_px, 12.0);
    assert_px(rect.radii.top_right.x_px, 10.0);
    assert_px(rect.radii.bottom_right.y_px, 8.0);
    let ViewPaintNode::Group(group) = &view_scene.scene.paint_nodes()[0] else {
        panic!("surface with shadow lowers to a compositing group");
    };
    assert_eq!(group.effects.box_shadows.shadows().len(), 1);
    assert_px(
        group.effects.box_shadows.shadows()[0]
            .border_radii
            .top_left
            .x_px,
        18.0,
    );
    assert_px(
        group.effects.box_shadows.shadows()[0]
            .border_radii
            .bottom_left
            .y_px,
        4.0,
    );
    assert_eq!(group.children.len(), 1);
}

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
fn selectable_runtime_text_block_drag_adds_selection_rectangles() {
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.text_blocks.push(ViewRuntimeTextBlock {
        public_id: "text.block.copyable".to_owned(),
        target: "text.block.copyable".to_owned(),
        view: Some("view.CopyPanel".to_owned()),
        containing_scroll_region: None,
        text: "Alpha Beta".to_owned(),
        bounds: ViewRuntimeTextBlockBounds::from_px(40, 48, 260, 40),
        selection_policy: ViewTextSelectionPolicy::Enabled,
        style: ViewRuntimeControlStyle::default(),
    });
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        images: &images,
        viewport: RenderViewport {
            logical_width: 320.0,
            logical_height: 180.0,
            physical_width: 320,
            physical_height: 180,
            scale_factor: 1.0,
        },
        fit: PlayerFrameFit::raw(),
        image_time_millis: 0,
        visual_time_millis: 0,
        preferences: RenderPreferences::default(),
    };

    let prepared = PlayerFramePlanner::prepare(&mut input, request).expect("frame prepares");
    let block = prepared
        .frame
        .selectable_text_blocks
        .first()
        .expect("selectable text block");
    let first = block
        .character_bounds
        .first()
        .expect("text block has glyph bounds")
        .bounds;
    let last = block
        .character_bounds
        .iter()
        .rev()
        .find(|bounds| bounds.bounds.width > 0.0)
        .expect("text block has visible glyph bounds")
        .bounds;
    let start = ViewportPoint::new(first.x + 1.0, first.y + first.height * 0.5);
    let end = ViewportPoint::new(last.x + last.width + 1.0, last.y + last.height * 0.5);

    input.pointer_down(
        &prepared.frame,
        PointerId(4),
        start,
        InputPointerModifiers::NONE,
    );
    input.pointer_move(&prepared.frame, PointerId(4), end);
    input.pointer_up(
        &prepared.frame,
        PointerId(4),
        end,
        InputPointerModifiers::NONE,
    );

    let selected = PlayerFramePlanner::prepare(&mut input, request).expect("selected frame");
    assert!(
        selected.frame.rectangles.len() > prepared.frame.rectangles.len(),
        "selection should add highlight rectangles"
    );
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
        selection_policy: ViewTextSelectionPolicy::Disabled,
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
