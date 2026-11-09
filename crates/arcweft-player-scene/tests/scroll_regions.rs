mod support;

use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::resource_codec::view::{
    ViewElementKind, ViewObserveClassification, ViewTextSelectionPolicy,
};
use arcweft_bundle::resource_codec::{
    ViewRuntimeControlVisualStyle, ViewRuntimeSurface, ViewRuntimeSurfaceBounds,
    ViewTextBlockBounds,
};
use arcweft_bundle::resource_codec::{ViewScrollAxis, ViewScrollOverflowPolicy, ViewScrollPolicy};
use arcweft_character::id::CharacterId;
use arcweft_core::plan::RuntimeLineId;
use arcweft_dialogue::InlineFailurePolicy;
use arcweft_id::PublicId;
use arcweft_id::TextKey;
use arcweft_id::presentation::PresentationHandleId;
use arcweft_id::view::ViewId;
use arcweft_player_scene::{
    fonts::{DEFAULT_PLAYER_FONT_BYTES, PlayerFontSet},
    frame::{PlayerFrameFit, PlayerFramePlanner, PlayerFramePlannerState, PlayerFrameRequest},
    images::BundleImageCatalog,
    input::{
        InputController, InputControllerSnapshot, InputControllerSnapshotError,
        InputPointerModifiers, InputScrollOffsetSnapshot,
    },
};
use arcweft_presentation::input::{InteractionTarget, PointerId, ViewportPoint};
use arcweft_render_text::{RuntimeLineContext, resolve_frame_with_template};
use arcweft_render_wgpu::geometry::{
    RenderPreferences, RenderScrollAxis, RenderScrollOverflow, RenderViewport,
};
use arcweft_render_wgpu::view_scene::ViewPrimitive;
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_runtime_driver::view_runtime::{
    BundleViewInstancePath, BundleViewMountOutput, BundleViewPaintItem, BundleViewStyleNode,
    BundleViewStyleNodeId, BundleViewStyleNodeKind, BundleViewTextOutput, BundleViewTextTarget,
    BundleViewTextValue,
};
use arcweft_source::{ProductSourceRef, SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentSpec, DialoguePresentationCharacter,
    RichTextControl, RichTextDocument, RichTextInlineDirection, RichTextLayoutUpdate, RichTextNode,
    RichTextStyle, RichTextWritingMode,
};
use arcweft_view::ViewMountId;
use arcweft_view::style::{
    ViewBoxAxisHostSeed, ViewBoxAxisSeedGeneration, ViewInheritedBoxAxes, ViewLengthMilli,
    ViewPropertyKind, ViewSpecifiedValue, ViewStyleApplication, ViewStyleApplicationTarget,
    ViewStyleAssignOp, ViewStyleBoundaryFacts, ViewStyleDeclaration, ViewStylePatch,
    ViewStylePatchId, ViewStyleProgram, ViewStyleScopeId, ViewStyleSourceId,
};

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
        CharacterDialoguePresentationConfig::standalone(
            arcweft_bundle::standard_view::dialogue_view_id(),
            InlineFailurePolicy::FailLine,
        ),
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
        action_buttons: Vec::new(),
        text_inputs: Vec::new(),
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
        events: Vec::new(),
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
            time: arcweft_player_scene::frame::PlayerFrameTime::runtime(0),
            preferences: RenderPreferences::default(),
        },
    )
    .expect("frame prepares");

    assert!(prepared.frame.view_scenes().is_empty());
}

fn retained_scroll_fixture() -> (BundlePresentationSnapshot, ViewStyleProgram) {
    let mut presentation = BundlePresentationSnapshot::default();
    for (target, policy) in [
        ("text.first", ViewScrollPolicy::default()),
        (
            "text.second",
            ViewScrollPolicy {
                axis: ViewScrollAxis::Horizontal,
                ..ViewScrollPolicy::default()
            },
        ),
        (
            "text.hidden",
            ViewScrollPolicy {
                overflow: ViewScrollOverflowPolicy::Hidden,
                ..ViewScrollPolicy::default()
            },
        ),
    ] {
        push_view_text(
            &mut presentation,
            "view.FeedbackPanel",
            target,
            None,
            "Scrollable content",
            ViewTextBlockBounds::from_px(0, 0, 240, 220),
            ViewTextSelectionPolicy::Disabled,
            ViewRuntimeControlVisualStyle::default(),
        );
        let mount = presentation.view.mounts.last_mut().unwrap();
        mount.style_nodes[0].instruction = 1;
        mount.style_nodes[0].parent = Some(BundleViewStyleNodeId {
            path: BundleViewInstancePath::default(),
            instruction: 0,
        });
        mount.style_nodes.insert(
            0,
            BundleViewStyleNode {
                path: BundleViewInstancePath::default(),
                instruction: 0,
                parent: None,
                kind: BundleViewStyleNodeKind::Scroll {
                    target: InteractionTarget::new(
                        PublicId::try_new_engine_owned(mount.scoped_id("scroll.body")).unwrap(),
                    ),
                    policy,
                },
                part: None,
                exported_part: None,
                applications: vec![ViewStyleApplication::new(
                    ViewStyleApplicationTarget::inline(ViewStylePatchId::new(0)),
                    ViewStyleScopeId::new(mount.mount.get()),
                    0,
                    0,
                    ViewStyleBoundaryFacts::SAME_VIEW,
                )],
            },
        );
    }
    let viewport_size = ViewStylePatch::new(
        ViewStylePatchId::new(0),
        [ViewPropertyKind::Width, ViewPropertyKind::Height]
            .into_iter()
            .map(|property| {
                ViewStyleDeclaration::new(
                    property,
                    ViewSpecifiedValue::Length {
                        value: ViewLengthMilli::new(100_000),
                    },
                    ViewStyleAssignOp::Replace,
                    ViewStyleSourceId::new(0),
                )
                .unwrap()
            })
            .collect(),
    );
    let style = ViewStyleProgram::try_new(Vec::new(), vec![viewport_size]).unwrap();
    (presentation, style)
}

#[test]
fn retained_scroll_routes_each_occurrence_and_uses_child_extents() {
    let (presentation, style) = retained_scroll_fixture();
    let images = BundleImageCatalog::empty();
    let request = PlayerFrameRequest {
        presentation: &presentation,
        fx_definitions: empty_fx_definitions(),
        images: &images,
        style_program: Some(&style),
        style_environment:
            &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
        style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
        viewport: RenderViewport {
            logical_width: 320.0,
            logical_height: 240.0,
            physical_width: 320,
            physical_height: 240,
            scale_factor: 1.0,
        },
        fit: PlayerFrameFit::raw(),
        time: arcweft_player_scene::frame::PlayerFrameTime::runtime(0),
        preferences: RenderPreferences::default(),
    };
    let mut input = InputController::default();
    let mut planner = PlayerFramePlannerState::new();
    planner
        .register_font_bytes(DEFAULT_PLAYER_FONT_BYTES.to_vec())
        .unwrap();
    let mut prepare = |input: &mut InputController, request| {
        let candidate = planner.prepare_candidate(input, request).unwrap();
        planner
            .publication_guard()
            .publish_with(candidate, input, |_| ())
            .unwrap()
            .0
    };
    let frame = prepare(&mut input, request).frame;
    let [first, second, hidden] = frame.scroll_regions.as_slice() else {
        panic!("one region per Scroll occurrence");
    };
    assert_eq!(first.id, "view_mount_1.scroll.body");
    assert_eq!(second.id, "view_mount_2.scroll.body");
    assert_eq!(first.axis, RenderScrollAxis::Vertical);
    assert_eq!(second.axis, RenderScrollAxis::Horizontal);
    assert_eq!(hidden.overflow, RenderScrollOverflow::Hidden);
    assert_px(first.max_offset_y(), 120.0);
    assert_px(first.max_offset_x(), 0.0);
    assert_px(second.max_offset_x(), 140.0);
    assert_px(second.max_offset_y(), 0.0);
    assert_px(hidden.max_offset_y(), 0.0);

    assert!(
        input
            .scroll_region_by_id(&frame, &first.id, 0.0, -40.0)
            .redraw
    );
    assert!(
        input
            .scroll_region_by_id(&frame, &second.id, -30.0, 0.0)
            .redraw
    );
    assert!(
        !input
            .scroll_region_by_id(&frame, &hidden.id, 0.0, -40.0)
            .redraw
    );
    let updated = prepare(&mut input, request).frame;
    assert_px(updated.scroll_regions[0].offset_y, 40.0);
    assert_px(updated.scroll_regions[1].offset_x, 30.0);
    assert_px(updated.scroll_regions[2].offset_y, 0.0);

    let mut shrunk = presentation.clone();
    shrunk.view.mounts[0].text[0].targets[0].bounds.height_milli = 110_000;
    let reflow = prepare(
        &mut input,
        PlayerFrameRequest {
            presentation: &shrunk,
            ..request
        },
    );
    assert_px(reflow.frame.scroll_regions[0].offset_y, 10.0);
    assert_px(reflow.frame.scroll_regions[1].offset_x, 30.0);
    let mount = &shrunk.view.mounts[0];
    let child = reflow
        .view_geometry()
        .final_geometry(&mount.style_nodes[1].style_node_key(mount.mount))
        .unwrap();
    assert_eq!(child.world_border_box.top_milli, -10_000);
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
        time: arcweft_player_scene::frame::PlayerFrameTime::runtime(0),
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
fn retained_text_ignores_a_scroll_alias_without_a_retained_ancestor() {
    let mut presentation = BundlePresentationSnapshot::default();
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
        time: arcweft_player_scene::frame::PlayerFrameTime::runtime(0),
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
                time: arcweft_player_scene::frame::PlayerFrameTime::runtime(0),
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
    presentation.view.mounts[0].style_nodes[0].kind = BundleViewStyleNodeKind::RichText {
        text_source: presentation.view.mounts[0].text[0].source_id.clone(),
    };
    presentation.view.mounts[0].text[0].value = BundleViewTextValue::RichTextDocument {
        document: Box::new(RichTextDocument::new(vec![RichTextNode::Scope {
            origin: arcweft_text_model::RichTextStyleOrigin::Generated,
            style: Box::new(RichTextStyle::Layout {
                layout: RichTextLayoutUpdate {
                    writing_mode: Some(RichTextWritingMode::VerticalRl),
                    direction: Some(RichTextInlineDirection::Rtl),
                    ..RichTextLayoutUpdate::default()
                },
            }),
            body: vec![RichTextNode::Ruby {
                body: vec![RichTextNode::Text {
                    text: "漢字".to_owned(),
                }],
                ruby: "かんじ".to_owned(),
            }],
        }])),
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
            time: arcweft_player_scene::frame::PlayerFrameTime::runtime(0),
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
    presentation.view.mounts[0].style_nodes[0].kind = BundleViewStyleNodeKind::RichText {
        text_source: presentation.view.mounts[0].text[0].source_id.clone(),
    };
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
    let template = arcweft_text_model::DialogueContentFragmentTemplate::try_new_canonical(
        arcweft_core::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
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
    )
    .expect("dialogue template");
    let spec = DialogueContentSpec::try_new(
        RuntimeLineId::from_runtime_line_value("say.typed_sources.display").unwrap(),
        TextKey::try_new("text.typed_sources.display").expect("text key"),
        &template,
        support::character_plan(),
        arcweft_text_model::DialoguePresentationSnapshot::new(
            support::dialogue_profile(),
            support::dialogue_profile_revision(),
        ),
        test_source_ref(),
    )
    .expect("dialogue spec");
    let content = arcweft_core::value::RuntimeDialogueContentValue::try_new(
        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x71; 32])
            .expect("fixture artifact"),
        template.id(),
        template.digest(),
        [],
    )
    .expect("fixture Content envelope");
    let display =
        resolve_frame_with_template(&spec, &template, &content, &test_line_context()).unwrap();
    presentation.view.mounts[1].style_nodes[0].kind = BundleViewStyleNodeKind::RichText {
        text_source: presentation.view.mounts[1].text[0].source_id.clone(),
    };
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
            time: arcweft_player_scene::frame::PlayerFrameTime::runtime(0),
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
