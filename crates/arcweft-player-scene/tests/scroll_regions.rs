mod support;

use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::resource_codec::view::{
    ViewElementKind, ViewObserveClassification, ViewTextSelectionPolicy,
};
use arcweft_bundle::resource_codec::{
    ViewFocusAutoScrollPolicy, ViewScrollAxis, ViewScrollIndicatorsPolicy,
    ViewScrollOverflowPolicy, ViewScrollOverscrollPolicy,
};
use arcweft_bundle::resource_codec::{
    ViewRuntimeControlVisualStyle, ViewRuntimeScrollRegion, ViewRuntimeScrollRegionBounds,
    ViewRuntimeSurface, ViewRuntimeSurfaceBounds, ViewTextBlockBounds,
};
use arcweft_character::id::CharacterId;
use arcweft_core::{entry::RuntimeValueDigest, plan::RuntimeLineId};
use arcweft_dialogue::InlineFailurePolicy;
use arcweft_id::TextKey;
use arcweft_player_scene::{
    fonts::{DEFAULT_PLAYER_FONT_BYTES, PlayerFontSet},
    frame::{PlayerFrameFit, PlayerFramePlanner, PlayerFramePlannerState, PlayerFrameRequest},
    images::BundleImageCatalog,
    input::{
        InputController, InputControllerSnapshot, InputControllerSnapshotError,
        InputPointerModifiers, InputScrollOffsetSnapshot,
    },
};
use arcweft_presentation::input::{PointerId, ViewportPoint};
use arcweft_render_text::{RuntimeLineContext, resolve_frame};
use arcweft_render_wgpu::geometry::{RenderPreferences, RenderViewport};
use arcweft_render_wgpu::view_scene::ViewPrimitive;
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_runtime_driver::presentation_handles::PresentationHandleId;
use arcweft_runtime_driver::view_runtime::{
    BundleViewInstancePath, BundleViewMountOutput, BundleViewPaintItem, BundleViewStyleNode,
    BundleViewStyleNodeKind, BundleViewTextOutput, BundleViewTextTarget, BundleViewTextValue,
};
use arcweft_source::{ProductSourceRef, SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentSpec, DialoguePresentationCharacter,
    RichTextControl, RichTextDocument, RichTextInlineDirection, RichTextLayout, RichTextNode,
    RichTextStyle, RichTextWritingMode,
};
use arcweft_view::style::{ViewBoxAxisHostSeed, ViewBoxAxisSeedGeneration, ViewInheritedBoxAxes};
use arcweft_view::{ViewId, ViewMountId};
use std::collections::BTreeMap;

fn test_source_ref() -> ProductSourceRef {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("player-scene-scroll-regions-test").expect("document ID"),
        SourceName::Memory,
        "scroll regions test",
    )
    .expect("test document");
    ProductSourceRef::try_for_identity(source.identity()).expect("product source reference")
}

fn test_line_context() -> RuntimeLineContext {
    RuntimeLineContext::new(
        Vec::new(),
        DialoguePresentationCharacter {
            id: CharacterId::try_new("character.narrator").expect("character identity"),
            display_name: "Narrator".to_owned(),
        },
        CharacterDialoguePresentationConfig {
            view: arcweft_bundle::standard_view::dialogue_view_id(),
            voice: None,
            look: None,
            stage: None,
            portrait: None,
            focus: None,
            cleanup: None,
            source_locale: None,
            hooks: Vec::new(),
            inline_failure: InlineFailurePolicy::FailLine,
            custom: BTreeMap::new(),
            config_digest: RuntimeValueDigest::ZERO,
        },
        Vec::new(),
        Vec::new(),
    )
}

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
    let mount = ViewMountId::from_raw(
        u64::try_from(presentation.view.mounts.len()).expect("mount count fits u64") + 1,
    );
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
        style_nodes: vec![BundleViewStyleNode {
            path: BundleViewInstancePath::default(),
            instruction: 0,
            parent: None,
            kind: BundleViewStyleNodeKind::Text {
                text_source: format!("source.{target}"),
            },
            part: None,
            exported_part: None,
            applications: Vec::new(),
        }],
    });
}

#[test]
fn product_only_surface_is_not_treated_as_retained_geometry() {
    let mut presentation = BundlePresentationSnapshot::default();
    presentation.surfaces.push(ViewRuntimeSurface {
        public_id: "surface.feedback.card".to_owned(),
        target: "surface.feedback.card".to_owned(),
        view: Some("view.FeedbackPanel".to_owned()),
        containing_scroll_region: None,
        element: ViewElementKind::Panel,
        bounds: ViewRuntimeSurfaceBounds::from_px(24, 32, 112, 72),
        style: ViewRuntimeControlVisualStyle::default(),
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

    assert!(prepared.frame.view_scenes().is_empty());
}

#[test]
fn product_only_scroll_region_is_not_treated_as_retained_geometry() {
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
        },
    )
    .expect("frame prepares");

    assert!(prepared.frame.scroll_regions.is_empty());
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
fn product_only_hidden_scroll_region_is_not_retained_geometry() {
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
        },
    )
    .expect("frame prepares");

    assert!(prepared.frame.scroll_regions.is_empty());
}

#[test]
fn product_only_horizontal_scroll_region_is_not_retained_geometry() {
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
        },
    )
    .expect("frame prepares");

    assert!(prepared.frame.scroll_regions.is_empty());
}

#[test]
fn retained_text_ignores_unretained_product_scroll_metadata() {
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
    let text = prepared.frame.text.items().first().expect("prepared text");
    assert_eq!(text.interaction.text, "Arcweft Concierge");
    assert_px(text.interaction.container_bounds.unwrap().y, 0.0);
    assert!(text.clip.is_none());
    assert!(prepared.frame.scroll_regions.is_empty());
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

    let candidate = planner
        .prepare_candidate(
            &input,
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
    let prepared = planner
        .publication_guard()
        .publish_with(candidate, &mut input, |_| ())
        .expect("registered frame publishes")
        .0;

    assert_eq!(prepared.frame.text.len(), 1);
    let item = &prepared.frame.text.items()[0];
    assert_eq!(item.interaction.text, "Prepared text");
    assert!(item.interaction.selection_enabled);
    assert_px(item.interaction.container_bounds.unwrap().x, 0.0);
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
    let display = resolve_frame(
        &DialogueContentSpec::new(
            RuntimeLineId::from_runtime_line_value("say.typed_sources.display").unwrap(),
            TextKey::try_new("text.typed_sources.display").expect("text key"),
            RichTextDocument::new(vec![
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
            support::character_plan(),
            arcweft_text_model::DialoguePresentationSnapshot::new(
                support::dialogue_profile(),
                support::dialogue_profile_revision(),
            ),
            Vec::new(),
            test_source_ref(),
        ),
        &test_line_context(),
    )
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
fn input_snapshot_preserves_signed_scroll_offsets() {
    let mut input = InputController::default();
    input
        .restore_snapshot(InputControllerSnapshot {
            choice_scroll_offset_y: 0.0,
            scroll_offsets: vec![InputScrollOffsetSnapshot {
                region_id: "scroll.bidirectional".to_owned(),
                offset_x: -12.5,
                offset_y: -48.0,
            }],
        })
        .expect("finite signed scroll offsets are valid persisted state");

    assert_eq!(
        input.scroll_offset_x("scroll.bidirectional").to_bits(),
        (-12.5_f32).to_bits()
    );
    assert_eq!(
        input.scroll_offset_y("scroll.bidirectional").to_bits(),
        (-48.0_f32).to_bits()
    );
    assert_eq!(
        input.snapshot().scroll_offsets,
        vec![InputScrollOffsetSnapshot {
            region_id: "scroll.bidirectional".to_owned(),
            offset_x: -12.5,
            offset_y: -48.0,
        }]
    );
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
