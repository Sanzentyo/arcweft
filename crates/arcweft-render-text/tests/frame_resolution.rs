use arcweft_character::id::CharacterId;
use arcweft_core::value::{RuntimeBinding, RuntimeExpr, RuntimeValue};
use arcweft_core::{entry::RuntimeValueDigest, plan::RuntimeLineId};
use arcweft_dialogue::{FallbackStylePolicy, InlineFailurePolicy, InlineTextFailure};
use arcweft_id::TextKey;
use arcweft_render_text::{RuntimeLineContext, resolve_frame};
use arcweft_source::{ProductSourceRef, SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentSpec, DialogueHostEvent,
    DialoguePresentationCharacter, DialogueVoiceSource, RichTextColor, RichTextControl,
    RichTextControlMarker, RichTextDocument, RichTextFontFamily, RichTextNode,
    RichTextPresentation, RichTextRange, RichTextRubyAnnotation, RichTextStyle, RichTextTextSource,
};
use arcweft_view::ViewId;
use std::collections::BTreeMap;

fn line_id(value: &str) -> RuntimeLineId {
    RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
}

fn source_ref() -> ProductSourceRef {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("render-text-frame-resolution-test").expect("document ID"),
        SourceName::Memory,
        "frame resolution test",
    )
    .expect("test document");
    ProductSourceRef::try_for_identity(source.identity()).expect("product source identity")
}

fn context(bindings: Vec<RuntimeBinding>) -> RuntimeLineContext {
    context_with_styles(bindings, Vec::new())
}

fn context_with_styles(
    bindings: Vec<RuntimeBinding>,
    base_styles: Vec<RichTextStyle>,
) -> RuntimeLineContext {
    RuntimeLineContext::new(
        bindings,
        DialoguePresentationCharacter {
            id: CharacterId::try_new("character.alice").expect("character identity"),
            display_name: "Alice".to_owned(),
        },
        CharacterDialoguePresentationConfig {
            view: ViewId::try_new("view.frame-resolution.test").expect("View identity"),
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
        base_styles,
        Vec::new(),
    )
}

fn spec(nodes: Vec<RichTextNode>) -> DialogueContentSpec {
    spec_with_line("say.test", nodes)
}

fn spec_with_line(line: &str, nodes: Vec<RichTextNode>) -> DialogueContentSpec {
    DialogueContentSpec::new(
        line_id(line),
        TextKey::try_new(line.replacen("say.", "text.", 1)).expect("text key"),
        RichTextDocument::new(nodes),
        Vec::new(),
        source_ref(),
    )
}

#[test]
fn resolves_text_ruby_controls_and_interpolation() {
    let line = spec(vec![
        RichTextNode::Text {
            text: "Hi ".to_owned(),
        },
        RichTextNode::Interpolation {
            expr: RuntimeExpr::Local("player".to_owned()),
            label: "player".to_owned(),
            on_error: InlineFailurePolicy::FailLine,
        },
        RichTextNode::Ruby {
            base: "夢".to_owned(),
            ruby: "ゆめ".to_owned(),
        },
        RichTextNode::Control {
            control: RichTextControl::HardBreak,
        },
        RichTextNode::Control {
            control: RichTextControl::Raw {
                text: "[p]".to_owned(),
            },
        },
    ]);
    let frame = resolve_frame(
        &line,
        &context_with_styles(
            vec![RuntimeBinding {
                name: "player".to_owned(),
                value: RuntimeValue::String("Aoi".to_owned()),
            }],
            vec![RichTextStyle::Font {
                family: RichTextFontFamily::Monospace,
            }],
        ),
    )
    .expect("frame resolves");

    assert_eq!(frame.text, "Hi Aoi夢\n[p]");
    assert_eq!(
        frame
            .display_map
            .text_runs
            .iter()
            .map(|run| (run.source, run.range))
            .collect::<Vec<_>>(),
        vec![
            (RichTextTextSource::Text, RichTextRange::new(0, 3)),
            (RichTextTextSource::Interpolation, RichTextRange::new(3, 6)),
            (RichTextTextSource::RubyBase, RichTextRange::new(6, 9)),
            (
                RichTextTextSource::ControlHardBreak,
                RichTextRange::new(9, 10)
            ),
            (RichTextTextSource::ControlRaw, RichTextRange::new(10, 13)),
        ]
    );
    assert_eq!(
        frame.display_map.ruby_annotations,
        vec![RichTextRubyAnnotation {
            base_range: RichTextRange::new(6, 9),
            ruby: "ゆめ".to_owned(),
            node_index: 2,
            styles: vec![RichTextStyle::Font {
                family: RichTextFontFamily::Monospace
            }],
            presentation: RichTextPresentation::default(),
        }]
    );
    assert_eq!(
        frame.display_map.controls,
        vec![
            RichTextControlMarker {
                node_index: 3,
                text_offset: 9,
                control: RichTextControl::HardBreak,
                range: Some(RichTextRange::new(9, 10)),
            },
            RichTextControlMarker {
                node_index: 4,
                text_offset: 10,
                control: RichTextControl::Raw {
                    text: "[p]".to_owned()
                },
                range: Some(RichTextRange::new(10, 13)),
            },
        ]
    );
    assert!(frame.unresolved.is_empty());
    assert!(frame.inline_failures.is_empty());
}

#[test]
fn interpolation_failure_policy_can_discard_or_fallback() {
    let line = spec(vec![
        RichTextNode::Text {
            text: "A".to_owned(),
        },
        RichTextNode::Interpolation {
            expr: RuntimeExpr::Local("missing_discard".to_owned()),
            label: "missing_discard".to_owned(),
            on_error: InlineFailurePolicy::Discard,
        },
        RichTextNode::Interpolation {
            expr: RuntimeExpr::Local("missing_fallback".to_owned()),
            label: "missing_fallback".to_owned(),
            on_error: InlineFailurePolicy::fallback_text("?"),
        },
    ]);
    let frame = resolve_frame(&line, &context(Vec::new()))
        .expect("frame resolves with non-failing policies");

    assert_eq!(frame.text, "A?");
    assert_eq!(
        frame.unresolved,
        vec!["missing_discard", "missing_fallback"]
    );
    assert_eq!(
        frame.inline_failures,
        vec![
            InlineTextFailure {
                expr: "missing_discard".to_owned(),
                reason: "runtime interpolation value was not resolved".to_owned(),
                policy: InlineFailurePolicy::Discard
            },
            InlineTextFailure {
                expr: "missing_fallback".to_owned(),
                reason: "runtime interpolation value was not resolved".to_owned(),
                policy: InlineFailurePolicy::fallback_text("?")
            }
        ]
    );
}

#[test]
fn interpolation_failure_policy_can_fail_line() {
    let line = spec_with_line(
        "say.opening.003",
        vec![RichTextNode::Interpolation {
            expr: RuntimeExpr::Local("missing".to_owned()),
            label: "missing".to_owned(),
            on_error: InlineFailurePolicy::FailLine,
        }],
    );

    let error = resolve_frame(&line, &context(Vec::new())).expect_err("line fails");

    assert_eq!(error.line, line_id("say.opening.003"));
    assert_eq!(error.expr, "missing");
}

#[test]
fn interpolation_fallback_can_render_expr_or_call_source() {
    let line = spec(vec![
        RichTextNode::Interpolation {
            expr: RuntimeExpr::Local("missing_expr_fallback".to_owned()),
            label: "score".to_owned(),
            on_error: InlineFailurePolicy::fallback_expr_source(FallbackStylePolicy::Plain),
        },
        RichTextNode::Text {
            text: "|".to_owned(),
        },
        RichTextNode::Interpolation {
            expr: RuntimeExpr::Local("missing_call_fallback".to_owned()),
            label: "fmt(score, style = \"number\")".to_owned(),
            on_error: InlineFailurePolicy::fallback_call_source(FallbackStylePolicy::Plain),
        },
    ]);

    let frame = resolve_frame(&line, &context(Vec::new())).expect("fallback source frame resolves");

    assert_eq!(frame.text, "score|fmt(score, style = \"number\")");
}

#[test]
fn local_text_conditionals_render_selected_branch_and_reject_missing_bindings() {
    let line = spec(vec![
        RichTextNode::Text {
            text: "A".to_owned(),
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::ConditionalStart {
                condition: RuntimeExpr::Local("flag".to_owned()),
            },
        },
        RichTextNode::Text {
            text: "yes".to_owned(),
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::ConditionalElse,
        },
        RichTextNode::Text {
            text: "no".to_owned(),
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::ConditionalEnd,
        },
        RichTextNode::Text {
            text: "Z".to_owned(),
        },
    ]);

    let true_frame = resolve_frame(
        &line,
        &context(vec![RuntimeBinding {
            name: "flag".to_owned(),
            value: RuntimeValue::Bool(true),
        }]),
    )
    .expect("true branch resolves");
    let false_frame = resolve_frame(
        &line,
        &context(vec![RuntimeBinding {
            name: "flag".to_owned(),
            value: RuntimeValue::Bool(false),
        }]),
    )
    .expect("false branch resolves");
    let missing_error = resolve_frame(&line, &context(Vec::new()))
        .expect_err("missing typed condition binding is rejected");

    assert_eq!(true_frame.text, "AyesZ");
    assert_eq!(false_frame.text, "AnoZ");
    assert_eq!(missing_error.expr, "flag");
    assert_eq!(missing_error.reason, "unknown runtime binding `flag`");
    assert_eq!(true_frame.host_events.len(), 3);
    assert_eq!(false_frame.host_events.len(), 3);
}

#[test]
fn inactive_conditional_branch_suppresses_styles_interpolation_and_host_events() {
    let line = spec(vec![
        RichTextNode::HostEvent {
            event: DialogueHostEvent::ConditionalStart {
                condition: RuntimeExpr::Local("flag".to_owned()),
            },
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::Color {
                value: RichTextColor::Rgba8 {
                    value: [0xff, 0x00, 0x00, 0xff],
                },
            },
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::Voice {
                source: DialogueVoiceSource::Identity {
                    id: "hidden".to_owned(),
                },
            },
        },
        RichTextNode::Interpolation {
            expr: RuntimeExpr::Local("missing".to_owned()),
            label: "missing".to_owned(),
            on_error: InlineFailurePolicy::FailLine,
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::ConditionalElse,
        },
        RichTextNode::Text {
            text: "shown".to_owned(),
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::ConditionalEnd,
        },
        RichTextNode::Text {
            text: " plain".to_owned(),
        },
    ]);

    let frame = resolve_frame(
        &line,
        &context(vec![RuntimeBinding {
            name: "flag".to_owned(),
            value: RuntimeValue::Bool(false),
        }]),
    )
    .expect("inactive interpolation does not fail the line");

    assert_eq!(frame.text, "shown plain");
    assert!(frame.host_events.iter().all(|event| matches!(
        event,
        DialogueHostEvent::ConditionalStart { .. }
            | DialogueHostEvent::ConditionalElse
            | DialogueHostEvent::ConditionalEnd
    )));
    assert!(frame.inline_failures.is_empty());
    assert!(frame.unresolved.is_empty());
    assert!(frame.display_map.text_runs.iter().all(|run| {
        !run.styles
            .iter()
            .any(|style| matches!(style, RichTextStyle::Color { .. }))
    }));
}

#[test]
fn reset_control_clears_active_inline_styles_for_following_runs() {
    let line = spec(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::Color {
                value: RichTextColor::Rgba8 {
                    value: [0x80, 0xc0, 0xff, 0xff],
                },
            },
        },
        RichTextNode::Text {
            text: "blue".to_owned(),
        },
        RichTextNode::Control {
            control: RichTextControl::Reset,
        },
        RichTextNode::Text {
            text: "plain".to_owned(),
        },
    ]);
    let frame = resolve_frame(&line, &context(Vec::new())).expect("frame resolves");

    assert_eq!(frame.text, "blueplain");
    assert_eq!(frame.display_map.text_runs.len(), 2);
    assert!(
        frame.display_map.text_runs[0]
            .styles
            .iter()
            .any(|style| matches!(style, RichTextStyle::Color { .. }))
    );
    assert!(frame.display_map.text_runs[1].styles.is_empty());
    assert_eq!(
        frame.display_map.controls,
        vec![RichTextControlMarker {
            node_index: 2,
            text_offset: 4,
            control: RichTextControl::Reset,
            range: None,
        }]
    );
}
