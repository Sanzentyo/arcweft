use arcweft_core::plan::RuntimeLineId;
use arcweft_core::value::{RuntimeBinding, RuntimeValue};
use arcweft_render_text::{
    DialogueHostEvent, FallbackStylePolicy, InlineFailurePolicy, InlineTextFailure,
    LineDisplaySpec, RichTextColor, RichTextControl, RichTextControlMarker, RichTextDocument,
    RichTextFontFamily, RichTextNode, RichTextPresentation, RichTextRange, RichTextRubyAnnotation,
    RichTextStyle, RichTextTextSource, RuntimeLineContext,
};

fn line_id(value: &str) -> RuntimeLineId {
    RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
}

fn spec(nodes: Vec<RichTextNode>) -> LineDisplaySpec {
    LineDisplaySpec {
        line: line_id("say.test"),
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        view: arcweft_view::ViewId::try_new("view.frame-resolution.test").unwrap(),
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(nodes),
    }
}

#[test]
fn quoted_hex_color_uses_the_same_typed_rgb_value_as_unquoted_color() {
    let expected = RichTextColor::Rgb {
        red: 255,
        green: 64,
        blue: 80,
    };

    assert_eq!(RichTextColor::from_attrs("#ff4050"), expected);
    assert_eq!(RichTextColor::from_attrs("\"#ff4050\""), expected);
}

#[test]
fn non_ascii_six_byte_color_payload_remains_named_without_panicking() {
    let expected = RichTextColor::Named {
        name: "#€abc".to_owned(),
    };

    assert_eq!(RichTextColor::from_attrs("#€abc"), expected);
    assert_eq!(RichTextColor::from_attrs("\"#€abc\""), expected);
}

#[test]
fn canonical_scalar_tag_attrs_match_direct_values() {
    for (name, direct, canonical) in [
        ("color", "#a8b5ff", "value=\"#a8b5ff\""),
        ("font", "\"Yu Gothic\"", "value=\"Yu Gothic\""),
        ("size", "36", "value=36"),
    ] {
        assert_eq!(
            RichTextStyle::from_tag(name, direct),
            RichTextStyle::from_tag(name, canonical),
            "{name} direct and canonical scalar forms must lower identically"
        );
    }
}

#[test]
fn resolves_text_ruby_controls_and_interpolation() {
    let mut line = spec(vec![
        RichTextNode::Text {
            text: "Hi ".to_owned(),
        },
        RichTextNode::Interpolation {
            expr: "player".to_owned(),
            fallback_source: "player".to_owned(),
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
    line.base_styles = vec![RichTextStyle::from_tag("font", "monospace")];
    let frame = line
        .resolve_frame(&RuntimeLineContext::new(vec![RuntimeBinding {
            name: "player".to_owned(),
            value: RuntimeValue::String("Aoi".to_owned()),
        }]))
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
            expr: "missing_discard".to_owned(),
            fallback_source: "missing_discard".to_owned(),
            on_error: InlineFailurePolicy::Discard,
        },
        RichTextNode::Interpolation {
            expr: "missing_fallback".to_owned(),
            fallback_source: "missing_fallback".to_owned(),
            on_error: InlineFailurePolicy::fallback_text("?"),
        },
    ]);
    let frame = line
        .resolve_frame(&RuntimeLineContext::default())
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
    let mut line = spec(vec![RichTextNode::Interpolation {
        expr: "missing".to_owned(),
        fallback_source: "missing".to_owned(),
        on_error: InlineFailurePolicy::FailLine,
    }]);
    line.line = line_id("say.opening.003");

    let error = line
        .resolve_frame(&RuntimeLineContext::default())
        .expect_err("line fails");

    assert_eq!(error.line, line_id("say.opening.003"));
    assert_eq!(error.expr, "missing");
}

#[test]
fn interpolation_fallback_can_render_expr_or_call_source() {
    let line = spec(vec![
        RichTextNode::Interpolation {
            expr: "fmt(score, style = \"number\")".to_owned(),
            fallback_source: "score".to_owned(),
            on_error: InlineFailurePolicy::fallback_expr_source(FallbackStylePolicy::Plain),
        },
        RichTextNode::Text {
            text: "|".to_owned(),
        },
        RichTextNode::Interpolation {
            expr: "fmt(score, style = \"number\")".to_owned(),
            fallback_source: "score".to_owned(),
            on_error: InlineFailurePolicy::fallback_call_source(FallbackStylePolicy::Plain),
        },
    ]);

    let frame = line
        .resolve_frame(&RuntimeLineContext::default())
        .expect("fallback source frame resolves");

    assert_eq!(frame.text, "score|fmt(score, style = \"number\")");
}

#[test]
fn local_text_conditionals_render_only_selected_branch() {
    let line = spec(vec![
        RichTextNode::Text {
            text: "A".to_owned(),
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::Conditional {
                name: "if".to_owned(),
                attrs: "flag".to_owned(),
            },
        },
        RichTextNode::Text {
            text: "yes".to_owned(),
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::Conditional {
                name: "else".to_owned(),
                attrs: String::new(),
            },
        },
        RichTextNode::Text {
            text: "no".to_owned(),
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::Conditional {
                name: "endif".to_owned(),
                attrs: String::new(),
            },
        },
        RichTextNode::Text {
            text: "Z".to_owned(),
        },
    ]);

    let true_frame = line
        .resolve_frame(&RuntimeLineContext::new(vec![RuntimeBinding {
            name: "flag".to_owned(),
            value: RuntimeValue::Bool(true),
        }]))
        .expect("true branch resolves");
    let false_frame = line
        .resolve_frame(&RuntimeLineContext::new(vec![RuntimeBinding {
            name: "flag".to_owned(),
            value: RuntimeValue::Bool(false),
        }]))
        .expect("false branch resolves");
    let missing_frame = line
        .resolve_frame(&RuntimeLineContext::default())
        .expect("missing condition resolves as false");

    assert_eq!(true_frame.text, "AyesZ");
    assert_eq!(false_frame.text, "AnoZ");
    assert_eq!(missing_frame.text, "AnoZ");
    assert_eq!(true_frame.host_events.len(), 3);
    assert_eq!(false_frame.host_events.len(), 3);
}

#[test]
fn inactive_conditional_branch_suppresses_styles_interpolation_and_host_events() {
    let line = spec(vec![
        RichTextNode::HostEvent {
            event: DialogueHostEvent::Conditional {
                name: "if".to_owned(),
                attrs: "flag".to_owned(),
            },
        },
        RichTextNode::StyleStart {
            style: RichTextStyle::from_tag("color", "#ff0000"),
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::Voice {
                attrs: "hidden".to_owned(),
            },
        },
        RichTextNode::Interpolation {
            expr: "missing".to_owned(),
            fallback_source: "missing".to_owned(),
            on_error: InlineFailurePolicy::FailLine,
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::Conditional {
                name: "else".to_owned(),
                attrs: String::new(),
            },
        },
        RichTextNode::Text {
            text: "shown".to_owned(),
        },
        RichTextNode::HostEvent {
            event: DialogueHostEvent::Conditional {
                name: "endif".to_owned(),
                attrs: String::new(),
            },
        },
        RichTextNode::Text {
            text: " plain".to_owned(),
        },
    ]);

    let frame = line
        .resolve_frame(&RuntimeLineContext::new(vec![RuntimeBinding {
            name: "flag".to_owned(),
            value: RuntimeValue::Bool(false),
        }]))
        .expect("inactive interpolation does not fail the line");

    assert_eq!(frame.text, "shown plain");
    assert!(
        frame
            .host_events
            .iter()
            .all(|event| matches!(event, DialogueHostEvent::Conditional { .. }))
    );
    assert!(frame.inline_failures.is_empty());
    assert!(frame.unresolved.is_empty());
    assert!(frame.display_map.text_runs.iter().all(|run| {
        !run.styles
            .iter()
            .any(|style| matches!(style, RichTextStyle::Color { .. }))
    }));
}

#[test]
fn rich_text_style_parses_font_families_without_renderer_context() {
    assert_eq!(
        RichTextStyle::from_tag("font", "monospace"),
        RichTextStyle::Font {
            family: RichTextFontFamily::Monospace
        }
    );
    assert_eq!(
        RichTextStyle::from_tag("font", r#""Noto Sans JP""#),
        RichTextStyle::Font {
            family: RichTextFontFamily::Named {
                name: "Noto Sans JP".to_owned()
            }
        }
    );
}

#[test]
fn reset_control_clears_active_inline_styles_for_following_runs() {
    let line = spec(vec![
        RichTextNode::StyleStart {
            style: RichTextStyle::from_tag("color", "#80c0ff"),
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
    let frame = line
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves");

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
