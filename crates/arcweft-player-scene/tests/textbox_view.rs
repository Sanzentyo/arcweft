use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_core::plan::RuntimeLineId;
use arcweft_player_scene::{
    frame::{PlayerFrameFit, PlayerFramePlanner, PlayerFrameRequest, PlayerPreparedFrame},
    images::BundleImageCatalog,
    input::InputController,
};
use arcweft_render_text::{
    LineDisplaySpec, RichTextDocument, RichTextInlineDirection, RichTextLayout, RichTextNode,
    RichTextStyle, RichTextWritingMode, RuntimeLineContext,
};
use arcweft_render_wgpu::{
    geometry::{PreparedTextBoxPart, PreparedTextOwnerKind, RenderPreferences, RenderViewport},
    view_scene::ViewPrimitive,
};
use arcweft_runtime_driver::{
    dialogue::TextBoxPresentationOperation, display::BundlePresentationSnapshot,
};
use arcweft_view::ViewMountAllocator;

#[test]
fn standard_textbox_is_a_view_scene_with_canonical_vertical_ruby_text() {
    let presentation = vertical_ruby_textbox();
    let prepared = prepare(&presentation);

    assert_eq!(prepared.scene.content_avoidance_regions.len(), 1);
    assert_eq!(prepared.frame.textboxes().len(), 1);
    assert_eq!(prepared.frame.text.len(), 2);
    assert_eq!(prepared.frame.prepared_text_owners().len(), 2);

    let view = prepared
        .frame
        .view_scenes()
        .first()
        .expect("standard TextBox View scene");
    assert!(matches!(
        view.scene.primitives(),
        [
            ViewPrimitive::SolidRect(_),
            ViewPrimitive::Text(_),
            ViewPrimitive::Text(_)
        ]
    ));

    let body = prepared
        .frame
        .prepared_text_owners()
        .iter()
        .find(|owner| {
            matches!(
                owner.kind,
                PreparedTextOwnerKind::TextBox {
                    part: PreparedTextBoxPart::Body,
                    ..
                }
            )
        })
        .expect("body owner");
    let body = prepared
        .frame
        .text
        .get(body.text)
        .expect("body prepared text");
    assert_eq!(body.layout.ruby.len(), 1);
    assert_eq!(
        body.layout.runs[0].writing_mode,
        RichTextWritingMode::VerticalRl
    );
}

fn vertical_ruby_textbox() -> BundlePresentationSnapshot {
    let line = RuntimeLineId::from_runtime_line_value("say.vertical_ruby").expect("line id");
    let frame = LineDisplaySpec {
        line: line.clone(),
        callee: "narrator".to_owned(),
        speaker_label: Some("語り手".to_owned()),
        text_key: None,
        window: Some("textbox.vertical".to_owned()),
        voice: None,
        look: None,
        style: None,
        base_styles: vec![RichTextStyle::Layout {
            layout: RichTextLayout {
                writing_mode: RichTextWritingMode::VerticalRl,
                direction: RichTextInlineDirection::Rtl,
                ..RichTextLayout::default()
            },
        }],
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![RichTextNode::Ruby {
            base: "漢字".to_owned(),
            ruby: "かんじ".to_owned(),
        }]),
    }
    .resolve_frame(&RuntimeLineContext::new(Vec::new()))
    .expect("display frame resolves");
    let mut presentation = BundlePresentationSnapshot::default();
    let mut mounts = ViewMountAllocator::default();
    presentation
        .textboxes
        .apply_operations(
            &[TextBoxPresentationOperation::append(
                "textbox.vertical",
                frame,
            )],
            &mut mounts,
        )
        .expect("TextBox mounts");
    presentation
        .textboxes
        .synchronize_waiting_line(Some(&line))
        .expect("waiting entry synchronizes");
    presentation
}

fn prepare(presentation: &BundlePresentationSnapshot) -> PlayerPreparedFrame {
    let images = BundleImageCatalog::empty();
    let mut input = InputController::default();
    PlayerFramePlanner::prepare(
        &mut input,
        PlayerFrameRequest {
            presentation,
            fx_definitions: &FxDefinitions::default(),
            images: &images,
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
    .expect("TextBox frame prepares")
}
