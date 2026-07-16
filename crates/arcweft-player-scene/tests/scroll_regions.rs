use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::resource_codec::view::{
    ViewElementKind, ViewObserveClassification, ViewRuntimeControlCornerRadius,
    ViewRuntimeControlRadii, ViewTextSelectionPolicy,
};
use arcweft_bundle::resource_codec::{
    ViewFocusAutoScrollPolicy, ViewScrollAxis, ViewScrollIndicatorsPolicy,
    ViewScrollOverflowPolicy, ViewScrollOverscrollPolicy,
};
use arcweft_bundle::resource_codec::{
    ViewRuntimeControlVisualStyle, ViewRuntimeScrollRegion, ViewRuntimeScrollRegionBounds,
    ViewRuntimeShadow, ViewRuntimeShadowKind, ViewRuntimeSurface, ViewRuntimeSurfaceBounds,
    ViewTextBlockBounds,
};
use arcweft_core::plan::RuntimeLineId;
use arcweft_player_scene::{
    fonts::{DEFAULT_PLAYER_FONT_BYTES, PlayerFontSet},
    frame::{PlayerFrameFit, PlayerFramePlanner, PlayerFramePlannerState, PlayerFrameRequest},
    images::BundleImageCatalog,
    input::{
        InputController, InputControllerSnapshot, InputControllerSnapshotError,
        InputPointerModifiers, InputScrollOffsetSnapshot,
    },
};
use arcweft_presentation::appearance::PresentationColor;
use arcweft_presentation::input::{PointerId, ViewportPoint};
use arcweft_render_text::{
    LineDisplaySpec, RichTextControl, RichTextDocument, RichTextInlineDirection, RichTextLayout,
    RichTextNode, RichTextStyle, RichTextWritingMode, RuntimeLineContext,
};
use arcweft_render_wgpu::geometry::{
    RenderPreferences, RenderScrollIndicatorsPolicy, RenderScrollOverscrollPolicy, RenderViewport,
};
use arcweft_render_wgpu::view_scene::{ViewPaintNode, ViewPrimitive};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_runtime_driver::presentation_handles::PresentationHandleId;
use arcweft_runtime_driver::view_runtime::{
    BundleViewInstancePath, BundleViewMountOutput, BundleViewPaintItem, BundleViewTextOutput,
    BundleViewTextTarget, BundleViewTextValue,
};
use arcweft_view::style::{ViewBoxAxisHostSeed, ViewBoxAxisSeedGeneration, ViewInheritedBoxAxes};
use arcweft_view::{ViewId, ViewMountId};

fn assert_px(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.001,
        "expected {expected}px, got {actual}px"
    );
}

fn empty_fx_definitions() -> &'static FxDefinitions {
    static DEFINITIONS: std::sync::OnceLock<FxDefinitions> = std::sync::OnceLock::new();
    DEFINITIONS.get_or_init(FxDefinitions::default)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the test fixture mirrors the complete typed mounted-text target contract"
)]
fn push_view_text(
    presentation: &mut BundlePresentationSnapshot,
    view: &str,
    target: &str,
    containing_scroll_region: Option<&str>,
    text: &str,
    bounds: ViewTextBlockBounds,
    selection_policy: ViewTextSelectionPolicy,
    style: ViewRuntimeControlVisualStyle,
) {
    let source_id = format!("source.{target}");
    let mount = ViewMountId::from_raw(0);
    presentation.view.mounts.push(BundleViewMountOutput {
        dialogue: None,
        handle: PresentationHandleId::try_new(format!("handle.{target}")).expect("handle id"),
        mount,
        host_axis_seed: Some(ViewInheritedBoxAxes::for_host_seed(
            mount,
            ViewBoxAxisSeedGeneration::INITIAL,
            ViewBoxAxisHostSeed::Default,
        )),
        view: ViewId::try_new(view).unwrap(),
        path: BundleViewInstancePath::default(),
        active_targets: vec![target.to_owned()],
        active_images: Vec::new(),
        paint: vec![BundleViewPaintItem::Text {
            source_id: source_id.clone(),
            target: target.to_owned(),
        }],
        text: vec![BundleViewTextOutput {
            source_id,
            targets: vec![BundleViewTextTarget {
                public_id: target.to_owned(),
                containing_scroll_region: containing_scroll_region.map(str::to_owned),
                bounds,
                selection_policy,
                style,
            }],
            value: BundleViewTextValue::Plain {
                value: text.to_owned(),
            },
            classification: ViewObserveClassification::default(),
            replacement: None,
        }],
        fx: Vec::new(),
        style_nodes: Vec::new(),
    });
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
        style: ViewRuntimeControlVisualStyle {
            fill: Some(PresentationColor::rgba(36, 42, 54, 255)),
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
                color: PresentationColor::rgba(0, 0, 0, 143),
                kind: ViewRuntimeShadowKind::Inset,
            }],
            ..ViewRuntimeControlVisualStyle::default()
        },
    });
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();

    let prepared = PlayerFramePlanner::prepare(
        &mut input,
        PlayerFrameRequest {
            presentation: &presentation,
            fx_definitions: empty_fx_definitions(),
            images: &images,
            style_program: None,
            style_environment:
                &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
            style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
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
            dialogue_reveal_complete: false,
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
        indicators: ViewScrollIndicatorsPolicy::Auto,
        overscroll: ViewScrollOverscrollPolicy::Clamp,
        auto_scroll_focus: ViewFocusAutoScrollPolicy::Nearest,
    });
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        fx_definitions: empty_fx_definitions(),
        images: &images,
        style_program: None,
        style_environment:
            &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
        style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
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
        dialogue_reveal_complete: false,
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
    assert_eq!(region.indicators, RenderScrollIndicatorsPolicy::Auto);
    assert_eq!(region.overscroll, RenderScrollOverscrollPolicy::Clamp);
    assert!(prepared.frame.scroll_indicators.is_empty());

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
    assert_eq!(prepared.frame.scroll_indicators.len(), 1);
    assert_eq!(
        prepared.frame.scroll_indicators[0].region_id,
        "scroll.feedback.body"
    );

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
    push_view_text(
        &mut presentation,
        "view.CopyPanel",
        "text.block.copyable",
        None,
        "Alpha Beta",
        ViewTextBlockBounds::from_px(40, 48, 260, 40),
        ViewTextSelectionPolicy::Enabled,
        ViewRuntimeControlVisualStyle::default(),
    );
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        fx_definitions: empty_fx_definitions(),
        images: &images,
        style_program: None,
        style_environment:
            &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
        style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
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
        dialogue_reveal_complete: false,
        preferences: RenderPreferences::default(),
    };

    let prepared = PlayerFramePlanner::prepare(&mut input, request).expect("frame prepares");
    let block = prepared
        .frame
        .text
        .items()
        .iter()
        .find(|item| item.interaction.selection_enabled)
        .expect("selectable text block");
    let first = block
        .interaction
        .character_bounds
        .first()
        .expect("text block has glyph bounds")
        .bounds;
    let last = block
        .interaction
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
        !selected.frame.text.items()[0]
            .interaction
            .selection_rects
            .is_empty(),
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
        indicators: ViewScrollIndicatorsPolicy::Auto,
        overscroll: ViewScrollOverscrollPolicy::Clamp,
        auto_scroll_focus: ViewFocusAutoScrollPolicy::Nearest,
    });
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        fx_definitions: empty_fx_definitions(),
        images: &images,
        style_program: None,
        style_environment:
            &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
        style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
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
        dialogue_reveal_complete: false,
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
        indicators: ViewScrollIndicatorsPolicy::Auto,
        overscroll: ViewScrollOverscrollPolicy::Clamp,
        auto_scroll_focus: ViewFocusAutoScrollPolicy::Nearest,
    });
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        fx_definitions: empty_fx_definitions(),
        images: &images,
        style_program: None,
        style_environment:
            &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
        style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
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
        dialogue_reveal_complete: false,
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
        indicators: ViewScrollIndicatorsPolicy::Auto,
        overscroll: ViewScrollOverscrollPolicy::Clamp,
        auto_scroll_focus: ViewFocusAutoScrollPolicy::Nearest,
    });
    push_view_text(
        &mut presentation,
        "view.NotesPanel",
        "text.block.NotesPanel.0",
        Some("scroll.notes"),
        "Arcweft Concierge",
        ViewTextBlockBounds::from_px(56, 112, 220, 24),
        ViewTextSelectionPolicy::Disabled,
        ViewRuntimeControlVisualStyle::default(),
    );
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        fx_definitions: empty_fx_definitions(),
        images: &images,
        style_program: None,
        style_environment:
            &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
        style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
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
        dialogue_reveal_complete: false,
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
    let text = prepared.frame.text.items().first().expect("prepared text");
    assert_eq!(text.interaction.text, "Arcweft Concierge");
    assert!((text.interaction.container_bounds.unwrap().y - 80.0).abs() < f32::EPSILON);
    let clip = text.clip.expect("scroll clip");
    assert_px(clip.x, prepared.frame.scroll_regions[0].bounds.x);
    assert_px(clip.y, prepared.frame.scroll_regions[0].bounds.y);
    assert_px(clip.width, prepared.frame.scroll_regions[0].bounds.width);
    assert_px(clip.height, prepared.frame.scroll_regions[0].bounds.height);
}

#[test]
fn registered_player_planner_prepares_runtime_text_in_canonical_batch() {
    let mut presentation = BundlePresentationSnapshot::default();
    push_view_text(
        &mut presentation,
        "view.PreparedText",
        "text.block.prepared",
        None,
        "Prepared text",
        ViewTextBlockBounds::from_px(24, 32, 240, 48),
        ViewTextSelectionPolicy::Enabled,
        ViewRuntimeControlVisualStyle::default(),
    );
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let mut planner = PlayerFramePlannerState::new();
    PlayerFontSet::single(DEFAULT_PLAYER_FONT_BYTES.to_vec())
        .register_with_planner(&mut planner)
        .expect("project font registers");

    let prepared = planner
        .prepare(
            &mut input,
            PlayerFrameRequest {
                presentation: &presentation,
                fx_definitions: empty_fx_definitions(),
                images: &images,
                style_program: None,
                style_environment:
                    &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
                style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
                viewport: RenderViewport {
                    logical_width: 640.0,
                    logical_height: 360.0,
                    physical_width: 1_280,
                    physical_height: 720,
                    scale_factor: 2.0,
                },
                fit: PlayerFrameFit::raw(),
                image_time_millis: 0,
                visual_time_millis: 0,
                dialogue_reveal_complete: false,
                preferences: RenderPreferences::default(),
            },
        )
        .expect("registered frame prepares");

    assert_eq!(prepared.frame.text.len(), 1);
    let item = &prepared.frame.text.items()[0];
    assert_eq!(item.interaction.text, "Prepared text");
    assert!(item.interaction.selection_enabled);
    assert_px(item.interaction.container_bounds.unwrap().x, 24.0);
    assert!((item.submission().raster_scale() - 2.0).abs() < f32::EPSILON);
}

#[test]
fn mounted_view_rich_text_preserves_vertical_ruby_in_prepared_painter_order() {
    let mut presentation = BundlePresentationSnapshot::default();
    push_view_text(
        &mut presentation,
        "view.VerticalRuby",
        "text.block.vertical_ruby",
        None,
        "漢字",
        ViewTextBlockBounds::from_px(24, 20, 180, 220),
        ViewTextSelectionPolicy::Disabled,
        ViewRuntimeControlVisualStyle::default(),
    );
    presentation.view.mounts[0].text[0].value = BundleViewTextValue::RichTextDocument {
        document: Box::new(RichTextDocument::new(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalRl,
                        direction: RichTextInlineDirection::Rtl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Ruby {
                base: "漢字".to_owned(),
                ruby: "かんじ".to_owned(),
            },
        ])),
    };
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let prepared = PlayerFramePlanner::prepare(
        &mut input,
        PlayerFrameRequest {
            presentation: &presentation,
            fx_definitions: empty_fx_definitions(),
            images: &images,
            style_program: None,
            style_environment:
                &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
            style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
            viewport: RenderViewport {
                logical_width: 320.0,
                logical_height: 260.0,
                physical_width: 320,
                physical_height: 260,
                scale_factor: 1.0,
            },
            fit: PlayerFrameFit::raw(),
            image_time_millis: 0,
            visual_time_millis: 0,
            dialogue_reveal_complete: false,
            preferences: RenderPreferences::default(),
        },
    )
    .expect("vertical RichText prepares");

    assert_eq!(prepared.frame.text.len(), 1);
    let item = &prepared.frame.text.items()[0];
    assert_eq!(item.layout.ruby.len(), 1);
    assert_eq!(
        item.layout.runs[0].writing_mode,
        RichTextWritingMode::VerticalRl
    );
    let view_scene = prepared.frame.view_scenes().first().expect("View scene");
    assert!(matches!(
        view_scene.scene.primitives(),
        [ViewPrimitive::Text(_)]
    ));
}

#[test]
fn mounted_view_localized_and_display_stage_sources_prepare_without_plain_fallback() {
    let mut presentation = BundlePresentationSnapshot::default();
    push_view_text(
        &mut presentation,
        "view.TypedSources",
        "text.block.localized",
        None,
        "placeholder",
        ViewTextBlockBounds::from_px(20, 20, 260, 48),
        ViewTextSelectionPolicy::Disabled,
        ViewRuntimeControlVisualStyle::default(),
    );
    presentation.view.mounts[0].text[0].value = BundleViewTextValue::Localized {
        key: "text.greeting".to_owned(),
        locale: Some("ja-JP".to_owned()),
        document: Box::new(RichTextDocument::new(vec![RichTextNode::Text {
            text: "こんにちは".to_owned(),
        }])),
    };
    push_view_text(
        &mut presentation,
        "view.TypedSources",
        "text.block.display",
        None,
        "placeholder",
        ViewTextBlockBounds::from_px(20, 80, 260, 48),
        ViewTextSelectionPolicy::Disabled,
        ViewRuntimeControlVisualStyle::default(),
    );
    let display = LineDisplaySpec {
        line: RuntimeLineId::from_runtime_line_value("say.typed_sources.display").unwrap(),
        callee: "narrator".to_owned(),
        speaker_label: None,
        text_key: None,
        view: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![
            RichTextNode::Text {
                text: "Stage one".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Page,
            },
            RichTextNode::Text {
                text: "Stage two".to_owned(),
            },
        ]),
    }
    .resolve_frame(&RuntimeLineContext::new(Vec::new()))
    .unwrap();
    presentation.view.mounts[1].text[0].value = BundleViewTextValue::DisplayFrame {
        frame: Box::new(display),
        stage_index: 0,
    };
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let prepared = PlayerFramePlanner::prepare(
        &mut input,
        PlayerFrameRequest {
            presentation: &presentation,
            fx_definitions: empty_fx_definitions(),
            images: &images,
            style_program: None,
            style_environment:
                &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
            style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
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
            dialogue_reveal_complete: false,
            preferences: RenderPreferences::default(),
        },
    )
    .expect("typed View sources prepare");

    assert_eq!(prepared.frame.text.len(), 2);
    assert_eq!(
        prepared.frame.text.items()[0].interaction.text,
        "こんにちは"
    );
    assert_eq!(prepared.frame.text.items()[1].interaction.text, "Stage one");
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
