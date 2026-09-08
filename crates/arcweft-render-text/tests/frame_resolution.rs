use arcweft_character::id::CharacterId;
use arcweft_core::runtime_id::RuntimeDialogueValueSlotId;
use arcweft_core::value::{RuntimeDialogueContentBinding, RuntimeInlineTextValue, RuntimeValue};
use arcweft_core::{
    entry::RuntimeValueDigest,
    pattern::RuntimeCheckedType,
    plan::{RuntimeDialogueValueBinding, RuntimeDialogueValueRole, RuntimeLineId},
};
use arcweft_dialogue::{
    FallbackStylePolicy, InlineFailurePolicy, InlineFailureSelection, InlineTextFailure,
};
use arcweft_id::TextKey;
use arcweft_render_text::{RuntimeLineContext, resolve_frame_with_template};
use arcweft_source::{ProductSourceRef, SourceDocument, SourceDocumentId, SourceName};
use arcweft_text_model::{
    CharacterDialoguePresentationConfig, DialogueContentFragmentTemplate, DialogueContentSpec,
    DialoguePresentationCharacter, ResolvedRichTextNode, RichTextColor, RichTextControl,
    RichTextControlMarker, RichTextDocument, RichTextFontFamily, RichTextNode, RichTextNodeIndex,
    RichTextNodeRange, RichTextPresentation, RichTextRange, RichTextRubyAnnotation, RichTextStyle,
    RichTextTextRunRange, RichTextTextSource,
};
use arcweft_view::ViewId;
use std::collections::BTreeMap;

mod support;

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

fn slot(index: usize) -> RuntimeDialogueValueSlotId {
    RuntimeDialogueValueSlotId::from_zero_based(index).expect("test dialogue slot is valid")
}

fn binding(index: usize, value: RuntimeValue) -> RuntimeDialogueValueBinding {
    RuntimeDialogueValueBinding {
        slot: slot(index),
        role: RuntimeDialogueValueRole::Interpolation,
        value,
    }
}

fn context(bindings: Vec<RuntimeDialogueValueBinding>) -> RuntimeLineContext {
    context_with_styles(bindings, Vec::new())
}

fn context_with_styles(
    bindings: Vec<RuntimeDialogueValueBinding>,
    base_styles: Vec<RichTextStyle>,
) -> RuntimeLineContext {
    let typed_bindings = bindings
        .iter()
        .map(|binding| match binding.role {
            RuntimeDialogueValueRole::Interpolation => {
                RuntimeDialogueContentBinding::Interpolation {
                    slot: binding.slot,
                    semantic_type: arcweft_core::pattern::RuntimeCheckedType::String
                        .semantic_identity_digest(),
                    value: RuntimeInlineTextValue::try_format_runtime_value(
                        arcweft_core::pattern::RuntimeCheckedType::String
                            .semantic_identity_digest(),
                        &binding.value,
                    )
                    .expect("test interpolation binding is formatable"),
                }
            }
            RuntimeDialogueValueRole::Content => {
                panic!("test context does not use nested content bindings")
            }
        })
        .collect::<Vec<_>>();
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
    .with_materialized_bindings(&typed_bindings)
}

struct TestSpec {
    spec: DialogueContentSpec,
    template: DialogueContentFragmentTemplate,
}

fn resolve_test_frame(
    spec: &TestSpec,
    context: &RuntimeLineContext,
) -> Result<arcweft_text_model::LineDisplayFrame, arcweft_render_text::LineDisplayError> {
    let content = support::content_value(&spec.template);
    resolve_frame_with_template(&spec.spec, &spec.template, &content, context)
}

fn spec(nodes: Vec<RichTextNode>) -> TestSpec {
    spec_with_line("say.test", nodes)
}

fn spec_with_line(line: &str, nodes: Vec<RichTextNode>) -> TestSpec {
    let mut declared_slots = BTreeMap::new();
    for node in &nodes {
        let (slot, role, semantic_type) = match node {
            RichTextNode::Interpolation { slot, .. } => (
                *slot,
                RuntimeDialogueValueRole::Interpolation,
                RuntimeCheckedType::String.semantic_identity_digest(),
            ),
            _ => continue,
        };
        declared_slots.insert(slot, (role, semantic_type));
    }
    let slots = declared_slots
        .into_iter()
        .map(|(slot, (role, semantic_type))| {
            arcweft_text_model::DialogueContentTemplateSlot::new(slot, role, semantic_type)
        })
        .collect();
    let template = DialogueContentFragmentTemplate::try_new_canonical(
        arcweft_core::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0).unwrap(),
        slots,
        Vec::new(),
        Vec::new(),
        RichTextDocument::new(nodes),
    )
    .expect("test dialogue template");
    let spec = DialogueContentSpec::try_new(
        line_id(line),
        TextKey::try_new(line.replacen("say.", "text.", 1)).expect("text key"),
        &template,
        support::character_plan("character.test"),
        arcweft_text_model::DialoguePresentationSnapshot::new(
            support::dialogue_profile(),
            support::dialogue_profile_revision(),
        ),
        Vec::new(),
        source_ref(),
    )
    .expect("test dialogue spec");
    TestSpec { spec, template }
}

#[test]
fn resolves_text_ruby_controls_and_interpolation() {
    let line = spec(vec![
        RichTextNode::Text {
            text: "Hi ".to_owned(),
        },
        RichTextNode::Interpolation {
            slot: slot(0),
            label: "player".to_owned(),
            on_error: InlineFailureSelection::Explicit {
                policy: InlineFailurePolicy::FailLine,
            },
        },
        RichTextNode::Ruby {
            body: vec![RichTextNode::Text {
                text: "夢".to_owned(),
            }],
            ruby: "ゆめ".to_owned(),
        },
        RichTextNode::Control {
            control: RichTextControl::HardBreak,
        },
        RichTextNode::Raw {
            text: "[p]".to_owned(),
        },
    ]);
    let frame = resolve_test_frame(
        &line,
        &context_with_styles(
            vec![binding(0, RuntimeValue::String("Aoi".to_owned()))],
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
            (RichTextTextSource::Text, RichTextRange::new(6, 9)),
            (
                RichTextTextSource::ControlHardBreak,
                RichTextRange::new(9, 10)
            ),
            (RichTextTextSource::Raw, RichTextRange::new(10, 13)),
        ]
    );
    assert_eq!(
        frame.display_map.ruby_annotations,
        vec![RichTextRubyAnnotation {
            owner_node: RichTextNodeIndex::new(2),
            body_nodes: RichTextNodeRange::new(
                RichTextNodeIndex::new(3),
                RichTextNodeIndex::new(4),
            ),
            base_runs: RichTextTextRunRange::new(2, 3),
            base_range: RichTextRange::new(6, 9),
            ruby: "ゆめ".to_owned(),
            styles: vec![RichTextStyle::Font {
                family: RichTextFontFamily::Monospace
            }],
            presentation: RichTextPresentation::default(),
        }]
    );
    assert_eq!(
        frame.display_map.controls,
        vec![RichTextControlMarker {
            node_index: RichTextNodeIndex::new(4),
            text_offset: 9,
            control: RichTextControl::HardBreak,
            range: Some(RichTextRange::new(9, 10)),
        },]
    );
    assert!(frame.unresolved.is_empty());
    assert!(frame.inline_failures.is_empty());
}

#[test]
fn nested_ruby_owns_preorder_body_nodes_and_exact_run_slices() {
    let line = spec(vec![RichTextNode::Ruby {
        body: vec![
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::Ruby {
                body: vec![RichTextNode::Scope {
                    style: Box::new(RichTextStyle::Strong),
                    body: vec![RichTextNode::Text {
                        text: "BC".to_owned(),
                    }],
                }],
                ruby: "びーしー".to_owned(),
            },
            RichTextNode::Raw {
                text: "D".to_owned(),
            },
        ],
        ruby: "えーびーしーでー".to_owned(),
    }]);
    let frame = resolve_test_frame(&line, &context(Vec::new())).expect("frame resolves");

    assert_eq!(frame.display_map.source_node_count.get(), 6);
    assert_eq!(frame.display_map.text_runs.len(), 3);
    assert_eq!(
        frame
            .display_map
            .ruby_annotations
            .iter()
            .map(|ruby| (ruby.owner_node.get(), ruby.body_nodes, ruby.base_runs))
            .collect::<Vec<_>>(),
        vec![
            (
                0,
                RichTextNodeRange::new(RichTextNodeIndex::new(1), RichTextNodeIndex::new(6)),
                RichTextTextRunRange::new(0, 3),
            ),
            (
                2,
                RichTextNodeRange::new(RichTextNodeIndex::new(3), RichTextNodeIndex::new(5)),
                RichTextTextRunRange::new(1, 2),
            ),
        ]
    );
    frame.validate().expect("nested display map is coherent");
}

#[test]
fn discarded_interpolation_retains_an_omitted_preorder_node() {
    let line = spec(vec![
        RichTextNode::Text {
            text: "A".to_owned(),
        },
        RichTextNode::Interpolation {
            slot: slot(0),
            label: "missing".to_owned(),
            on_error: InlineFailureSelection::Explicit {
                policy: InlineFailurePolicy::Discard,
            },
        },
        RichTextNode::Text {
            text: "B".to_owned(),
        },
    ]);
    let frame = resolve_test_frame(&line, &context(Vec::new())).expect("frame resolves");

    assert_eq!(
        frame.nodes,
        vec![
            ResolvedRichTextNode::Text {
                text: "A".to_owned(),
            },
            ResolvedRichTextNode::Omitted,
            ResolvedRichTextNode::Text {
                text: "B".to_owned(),
            },
        ]
    );
    assert_eq!(frame.display_map.source_node_count.get(), 3);
    frame
        .validate()
        .expect("omitted node keeps tree cardinality");
}

#[test]
fn interpolation_failure_policy_can_discard_or_fallback() {
    let line = spec(vec![
        RichTextNode::Text {
            text: "A".to_owned(),
        },
        RichTextNode::Interpolation {
            slot: slot(0),
            label: "missing_discard".to_owned(),
            on_error: InlineFailureSelection::Explicit {
                policy: InlineFailurePolicy::Discard,
            },
        },
        RichTextNode::Interpolation {
            slot: slot(1),
            label: "missing_fallback".to_owned(),
            on_error: InlineFailureSelection::Explicit {
                policy: InlineFailurePolicy::fallback_text("?"),
            },
        },
    ]);
    let frame = resolve_test_frame(&line, &context(Vec::new()))
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
            slot: slot(0),
            label: "missing".to_owned(),
            on_error: InlineFailureSelection::Explicit {
                policy: InlineFailurePolicy::FailLine,
            },
        }],
    );

    let error = resolve_test_frame(&line, &context(Vec::new())).expect_err("line fails");

    assert_eq!(error.line, *line.spec.line());
    assert_eq!(error.expr, "missing");
}

#[test]
fn interpolation_fallback_can_render_expr_or_call_source() {
    let line = spec(vec![
        RichTextNode::Interpolation {
            slot: slot(0),
            label: "score".to_owned(),
            on_error: InlineFailureSelection::Explicit {
                policy: InlineFailurePolicy::fallback_expr_source(FallbackStylePolicy::Plain),
            },
        },
        RichTextNode::Text {
            text: "|".to_owned(),
        },
        RichTextNode::Interpolation {
            slot: slot(1),
            label: "fmt(score, style = \"number\")".to_owned(),
            on_error: InlineFailureSelection::Explicit {
                policy: InlineFailurePolicy::fallback_call_source(FallbackStylePolicy::Plain),
            },
        },
    ]);

    let frame =
        resolve_test_frame(&line, &context(Vec::new())).expect("fallback source frame resolves");

    assert_eq!(frame.text, "score|fmt(score, style = \"number\")");
}

#[test]
fn structural_scope_preserves_style_across_control_nodes() {
    let line = spec(vec![RichTextNode::Scope {
        style: Box::new(RichTextStyle::Color {
            value: RichTextColor::Rgba8 {
                value: [0x80, 0xc0, 0xff, 0xff],
            },
        }),
        body: vec![
            RichTextNode::Text {
                text: "blue".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Reset,
            },
            RichTextNode::Text {
                text: "plain".to_owned(),
            },
        ],
    }]);
    let frame = resolve_test_frame(&line, &context(Vec::new())).expect("frame resolves");

    assert_eq!(frame.text, "blueplain");
    assert_eq!(frame.display_map.text_runs.len(), 2);
    assert!(
        frame.display_map.text_runs[0]
            .styles
            .iter()
            .any(|style| matches!(style, RichTextStyle::Color { .. }))
    );
    assert!(
        frame.display_map.text_runs[1]
            .styles
            .iter()
            .any(|style| matches!(style, RichTextStyle::Color { .. }))
    );
    assert_eq!(
        frame.display_map.controls,
        vec![RichTextControlMarker {
            node_index: RichTextNodeIndex::new(2),
            text_offset: 4,
            control: RichTextControl::Reset,
            range: None,
        }]
    );
}
