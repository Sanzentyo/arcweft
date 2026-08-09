use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_bundle::resource_codec::view::ViewRuntimeActionButtonAction;
use arcweft_bundle::resource_codec::{ValidatedViewProduct, ViewProductValidationLimits};
use arcweft_bundle::standard_view::{dialogue_program, dialogue_style, dialogue_text};
use arcweft_character::id::CharacterId;
use arcweft_core::{entry::RuntimeValueDigest, plan::RuntimeLineId};
use arcweft_dialogue::InlineFailurePolicy;
use arcweft_id::TextKey;
use arcweft_player_scene::{
    frame::{PlayerFrameFit, PlayerFramePlanner, PlayerFrameRequest, PlayerPreparedFrame},
    images::BundleImageCatalog,
    input::InputController,
};
use arcweft_render_text::{RuntimeLineContext, resolve_frame};
use arcweft_render_wgpu::{
    geometry::{PreparedTextOwnerKind, RenderPreferences, RenderViewport},
    view_scene::ViewPrimitive,
};
use arcweft_runtime_driver::{
    dialogue::{DialoguePresentationOperation, DialogueViewDefinition},
    display::BundlePresentationSnapshot,
    view_runtime::BundleViewRuntime,
};
use arcweft_source::{ProductSourceRef, SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentSpec, DialoguePresentationCharacter,
    LineDisplayFrame, RichTextDocument, RichTextInlineDirection, RichTextLayout, RichTextNode,
    RichTextStyle, RichTextWritingMode,
};
use std::collections::BTreeMap;

fn test_source_ref() -> ProductSourceRef {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("player-scene-dialogue-view-test").expect("document ID"),
        SourceName::Memory,
        "dialogue view test",
    )
    .expect("test document");
    ProductSourceRef::try_for_identity(source.identity()).expect("product source reference")
}

#[test]
fn standard_dialogue_view_preserves_vertical_ruby_and_final_panel_geometry() {
    let presentation = vertical_ruby_dialogue_view();
    let prepared = prepare(&presentation);

    assert_eq!(prepared.scene.content_avoidance_regions.len(), 1);
    assert_eq!(prepared.frame.dialogue_views().len(), 1);
    assert_eq!(prepared.frame.text.len(), 2);
    let dialogue = prepared
        .frame
        .latest_dialogue_view()
        .expect("standard dialogue View state");
    assert!((dialogue.bounds.x - 57.6).abs() < 0.001);
    assert!((dialogue.bounds.y - 460.8).abs() < 0.001);
    assert!((dialogue.bounds.width - 1_164.8).abs() < 0.001);
    assert!((dialogue.bounds.height - 201.6).abs() < 0.001);
    assert!(dialogue.primary_action.is_some());

    let text_primitives = prepared
        .frame
        .view_scenes()
        .iter()
        .flat_map(|view| view.scene.primitives())
        .filter(|primitive| matches!(primitive, ViewPrimitive::Text(_)))
        .count();
    assert_eq!(text_primitives, 2);
    assert!(prepared.frame.view_scenes().iter().any(|view| {
        view.scene
            .primitives()
            .iter()
            .any(|primitive| matches!(primitive, ViewPrimitive::SolidRect(_)))
    }));

    let speaker = prepared
        .frame
        .prepared_text_owners()
        .iter()
        .find(|owner| matches!(owner.kind, PreparedTextOwnerKind::View { .. }))
        .expect("dialogue speaker owner");
    let speaker = prepared
        .frame
        .text
        .get(speaker.text)
        .expect("prepared speaker");
    assert_eq!(speaker.layout.runs[0].style.font_size_milli(), 25_000);
    assert_eq!(speaker.layout.runs[0].style.line_height_milli(), 34_000);

    let body = prepared
        .frame
        .prepared_text_owners()
        .iter()
        .find(|owner| matches!(owner.kind, PreparedTextOwnerKind::DialogueView { .. }))
        .expect("dialogue content owner");
    assert!((body.object_bounds.x - 57.6).abs() < 0.001);
    assert!((body.object_bounds.y - 460.8).abs() < 0.001);
    assert!((body.object_bounds.width - 1_164.8).abs() < 0.001);
    assert!((body.object_bounds.height - 201.6).abs() < 0.001);
    let body = prepared
        .frame
        .text
        .get(body.text)
        .expect("prepared content");
    assert_eq!(body.layout.ruby.len(), 1);
    assert_eq!(
        body.layout.runs[0].writing_mode,
        RichTextWritingMode::VerticalRl
    );
}

fn vertical_ruby_dialogue_view() -> BundlePresentationSnapshot {
    let line = RuntimeLineId::from_runtime_line_value("say.vertical_ruby").expect("line id");
    let frame = vertical_ruby_frame(&line);

    let program = dialogue_program();
    let text = dialogue_text();
    let style = dialogue_style();
    let mut presentation = BundlePresentationSnapshot::default();
    presentation
        .dialogue
        .apply_operations(&[DialoguePresentationOperation::append(
            DialogueViewDefinition::new(arcweft_bundle::standard_view::dialogue_view_id()),
            frame,
        )])
        .expect("dialogue append applies");
    presentation
        .dialogue
        .synchronize_waiting_line(Some(&line))
        .expect("waiting entry synchronizes");

    let style_source = arcweft_bundle::standard_view::dialogue_style_source_document();
    let source_map =
        arcweft_bundle::resource_codec::SourceMapSection::try_from_documents(&[&style_source])
            .expect("standard dialogue Style source map");
    let product = ValidatedViewProduct::try_new(
        Some(source_map),
        Some(program.clone()),
        Some(style),
        ViewProductValidationLimits::default(),
    )
    .expect("standard View product");
    let mut runtime =
        BundleViewRuntime::try_new(product, Some(text.clone())).expect("standard View runtime");
    presentation.view =
        runtime.evaluate_with_dialogue(&[], &presentation.dialogue.view_inputs(), &[], false);
    assert!(presentation.view.diagnostics.is_empty());
    let mount = presentation.view.mounts[0].clone();

    presentation.surfaces = program
        .runtime_surfaces()
        .into_iter()
        .map(|mut surface| {
            surface.public_id = mount.scoped_id(&surface.public_id);
            surface.target = mount.scoped_id(&surface.target);
            surface.view = Some(mount.scoped_id(mount.view.as_str()));
            surface
        })
        .collect();
    presentation.action_buttons = program
        .runtime_action_buttons(Some(&text))
        .into_iter()
        .map(|mut button| {
            button.public_id = mount.scoped_id(&button.public_id);
            button.target = mount.scoped_id(&button.target);
            button.view = Some(mount.scoped_id(mount.view.as_str()));
            if let ViewRuntimeActionButtonAction::DialoguePrimaryAction { target, .. } =
                &mut button.action
            {
                *target = mount
                    .dialogue
                    .and_then(|dialogue| dialogue.primary_action.target);
                button.enabled &= target.is_some();
            }
            button
        })
        .collect();
    presentation
}

fn vertical_ruby_frame(line: &RuntimeLineId) -> LineDisplayFrame {
    resolve_frame(
        &DialogueContentSpec::new(
            line.clone(),
            TextKey::try_new("text.vertical_ruby").expect("text key"),
            RichTextDocument::new(vec![RichTextNode::Ruby {
                base: "漢字".to_owned(),
                ruby: "かんじ".to_owned(),
            }]),
            Vec::new(),
            test_source_ref(),
        ),
        &RuntimeLineContext::new(
            Vec::new(),
            DialoguePresentationCharacter {
                id: CharacterId::try_new("character.narrator").expect("character identity"),
                display_name: "語り手".to_owned(),
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
            vec![RichTextStyle::Layout {
                layout: RichTextLayout {
                    writing_mode: RichTextWritingMode::VerticalRl,
                    direction: RichTextInlineDirection::Rtl,
                    ..RichTextLayout::default()
                },
            }],
            Vec::new(),
        ),
    )
    .expect("display frame resolves")
}

fn prepare(presentation: &BundlePresentationSnapshot) -> PlayerPreparedFrame {
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    let style = dialogue_style();
    PlayerFramePlanner::prepare(
        &mut input,
        PlayerFrameRequest {
            presentation,
            fx_definitions: &FxDefinitions::default(),
            images: &images,
            style_program: Some(&style.program),
            style_environment:
                &arcweft_presentation::appearance::PresentationEnvironment::ENGINE_DEFAULT,
            style_palettes: &arcweft_presentation::appearance::SystemPaletteSet::ENGINE_DEFAULT,
            viewport: RenderViewport {
                logical_width: 1_280.0,
                logical_height: 720.0,
                physical_width: 1_280,
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
    .expect("dialogue View frame prepares")
}
