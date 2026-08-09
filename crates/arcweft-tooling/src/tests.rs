use crate::{
    code_actions::source_code_actions,
    edit::apply_text_edits,
    format::format_document,
    model::{FormatOptions, TextEdit, ToolingEditReport, ToolingError},
};
use arcweft_lang_syntax::{
    attachment::TypedItemNode, incremental::SyntaxDatabase, parser::ParseOptions,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, identity::SourceSnapshotId};
use std::sync::Arc;

fn fixture_document(source: impl Into<Arc<str>>) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://tooling/format.arcw")
                .expect("fixture source ID"),
            SourceName::path("format.arcw"),
            source,
        )
        .expect("fixture source document"),
    )
}

fn format_fixture(source: &str, options: FormatOptions) -> Result<ToolingEditReport, ToolingError> {
    format_document(fixture_document(source), options)
}

#[test]
fn default_format_preserves_authoring_surface() {
    let source = "flow opening {\n    alice: hi[p]\n}\n";
    let report = format_fixture(source, FormatOptions::default()).expect("format report");
    assert!(!report.changed);
    assert_eq!(report.output, source);
}

#[test]
fn formatter_preserves_lossless_predicate_proof_nodes() {
    let source = concat!(
        "/// Predicate documentation stays attached.\n",
        "pub predicate ready(value: Bool) = value  // predicate tail\n",
        "\n",
        "// Inter-item trivia remains byte-for-byte lossless.\n",
        "proof readiness() { assert.prove(ready(true)); }\n",
    );
    let report = format_fixture(source, FormatOptions::default()).expect("format report");
    assert!(!report.changed);
    assert_eq!(report.output, source);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);

    let document = fixture_document(report.output.clone());
    let snapshot = SourceSnapshotId::initial(document.display_name().clone());
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let parsed = syntax
        .parse_initial(snapshot, document, ParseOptions::default())
        .expect("formatted source remains attached");
    let items = parsed.items().expect("typed formatted items");
    assert!(
        matches!(
            items.as_slice(),
            [TypedItemNode::Predicate(_), TypedItemNode::Proof(_)]
        ),
        "{items:?}"
    );
    assert_ne!(items[0].id(), items[1].id());
    assert!(items[0].range().end() <= items[1].range().start());
    assert_eq!(
        &parsed.source()[items[0].range().start()..items[0].range().end()],
        &source[items[0].range().start()..items[0].range().end()]
    );
    assert_eq!(
        &parsed.source()[items[1].range().start()..items[1].range().end()],
        &source[items[1].range().start()..items[1].range().end()]
    );
}

#[test]
fn format_accepts_controller_function_source_without_extension_dispatch() {
    let source = "fn opening() {\n    let frame = try observe(@flow.opening)\n}\n";
    let report = format_fixture(source, FormatOptions::default()).expect("format report");

    assert!(!report.changed);
    assert_eq!(report.output, source);
}

#[test]
fn agent_format_is_idempotent_for_action_resource_and_rag_samples() {
    let samples = [
        include_str!("../../../samples/agent-script/cli-pointer-click-smoke.awfagent"),
        include_str!("../../../samples/agent-script/cli-attach-resource-smoke.awfagent"),
        include_str!("../../../samples/agent-script/failure-investigation.awfagent"),
    ];

    for sample in samples {
        let first = format_fixture(sample, FormatOptions::default()).expect("format report");
        assert!(!first.changed);
        assert_eq!(first.output, sample);
        assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);

        let second =
            format_fixture(&first.output, FormatOptions::default()).expect("second format report");
        assert!(!second.changed);
        assert_eq!(second.output, first.output);
        assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    }
}

#[test]
fn text_edit_planning_preserves_structured_edit_errors() {
    let utf8_error = apply_text_edits(
        "é",
        &[TextEdit {
            start: 1,
            end: 1,
            replacement: "x".to_owned(),
        }],
    )
    .expect_err("mid-codepoint edit must fail");
    assert_eq!(
        utf8_error,
        ToolingError::InvalidCharBoundary { start: 1, end: 1 }
    );

    let overlap_error = apply_text_edits(
        "abcd",
        &[
            TextEdit {
                start: 0,
                end: 2,
                replacement: String::new(),
            },
            TextEdit {
                start: 1,
                end: 3,
                replacement: String::new(),
            },
        ],
    )
    .expect_err("overlap must fail");
    assert_eq!(
        overlap_error,
        ToolingError::OverlappingEdit { start: 1, end: 3 }
    );

    let range_error = apply_text_edits(
        "abc",
        &[TextEdit {
            start: 2,
            end: 4,
            replacement: String::new(),
        }],
    )
    .expect_err("out-of-range edit must fail");
    assert_eq!(
        range_error,
        ToolingError::RangeOutOfBounds {
            start: 2,
            end: 4,
            len: 3,
        }
    );
}

#[test]
fn attached_style_formatter_orders_fields_and_normalizes_percentage() {
    let source = r"pub style adaptive {
    when environment(text-scale>=125.0%, color-scheme == DARK) {
        Button { opacity = 90% }
    }
}
";
    let report = format_fixture(source, FormatOptions::default()).expect("format report");
    assert_eq!(
        report.output,
        r"pub style adaptive {
    when environment(
        color-scheme == dark,
        text-scale >= 125%,
    ) {
        Button { opacity = 90% }
    }
}
"
    );
    let second = format_fixture(&report.output, FormatOptions::default()).expect("second format");
    assert!(!second.changed);
    assert_eq!(second.output, report.output);
}

#[test]
fn attached_style_formatter_preserves_recovered_environment_nodes() {
    let source = r"pub style adaptive {
    when environment(text-scale == clamp(50%, 100%)) {
        Button { opacity = 90% }
    }
}
";
    let first = format_fixture(source, FormatOptions::default()).expect("format report");
    assert!(!first.changed);
    assert_eq!(first.output, source);
    let second = format_fixture(&first.output, FormatOptions::default()).expect("second format");
    assert_eq!(second.output, first.output);
}

#[test]
fn canonical_rich_text_expands_only_inferred_tag_families() {
    let source = "flow opening {\n    let line = alice[｜漢字《かんじ》 $(name)[.keyword][.sparkle amp=2px]there[/][.vertical_rl]縦[/][page]]\n}\n";
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains("｜漢字《かんじ》"));
    assert!(report.output.contains("$(name)"));
    assert!(report.output.contains("[.keyword]"));
    assert!(!report.output.contains("[mark .keyword]"));
    assert!(
        report
            .output
            .contains("[effect .sparkle amp=2px]there[/effect]")
    );
    assert!(report.output.contains("[layout .vertical_rl]縦[/layout]"));
    assert!(report.output.contains("[page]"));
}

#[test]
fn canonical_rich_text_attached_bracket_matrix_uses_typed_tag_families() {
    let source = concat!(
        "flow opening {\n",
        "    let line = alice[[fx warning(label=\"urgent warning\")]important[/fx]",
        "[.keyword]word[/][.sparkle note=\"contains ] safely\"]effect[/]",
        "[.vertical_rl]縦[/][p]]\n",
        "}\n",
    );
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[fx warning(label=\"urgent warning\")]important[/fx]")
    );
    assert!(report.output.contains("[.keyword]word[/]"));
    assert!(!report.output.contains("[mark .keyword]"));
    assert!(
        report
            .output
            .contains("[effect .sparkle note=\"contains ] safely\"]effect[/effect]")
    );
    assert!(
        report
            .output
            .contains("[layout .vertical_rl]縦[/layout][p]")
    );
}

#[test]
fn canonical_rich_text_attached_bracket_matrix_expands_typed_proxy_objects() {
    let source = concat!(
        "#[text_proxy(kind=\"keyword\", default_hit=true)]\n",
        "pub struct KeywordHit {\n",
        "    channel: String\n",
        "}\n\n",
        "#[rich_text_proxy(kind=\"hover\", default_hit=false)]\n",
        "pub struct HoverHit {\n",
        "    layer: String\n",
        "}\n\n",
        "flow opening {\n",
        "    let line = alice[[.hotspot type=KeywordHit channel=inventory]",
        "[.HoverHit tone=alert]multi[/][/][p]]\n",
        "}\n",
    );
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[object .hotspot type=KeywordHit channel=inventory]",)
    );
    assert!(
        report
            .output
            .contains("[object .HoverHit type=HoverHit tone=alert]multi[/object][/object]",)
    );
}

#[test]
fn canonical_rich_text_attached_bracket_matrix_visits_flow_else_branches() {
    let source = concat!(
        "flow opening {\n",
        "    if ready {\n",
        "        let then_line = alice[[.shake]then[/][p]]\n",
        "    } else {\n",
        "        let else_line = alice[[.pulse]else[/][p]]\n",
        "    }\n",
        "}\n",
    );
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains("[effect .shake]then[/effect][p]"));
    assert!(report.output.contains("[effect .pulse]else[/effect][p]"));
}

#[test]
fn canonical_rich_text_preserves_explicit_fx_and_quoted_brackets() {
    let source = "flow opening {\n    let line = alice[[fx warning(label=\"urgent warning\")]important[/fx][.sparkle note=\"contains ] safely\"]effect[/][p]]\n}\n";
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[fx warning(label=\"urgent warning\")]important[/fx]")
    );
    assert!(
        report
            .output
            .contains("[effect .sparkle note=\"contains ] safely\"]effect[/effect][p]")
    );
}

#[test]
fn canonical_rich_text_projects_indented_multiline_lf_and_crlf_edits() {
    let source_lf = "flow opening {\n    let line = alice[\n        Intro\n        [.sparkle amp=2px]effect[/][p]\n    ]\n}\n";
    for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
        let report = format_fixture(
            &source,
            FormatOptions {
                canonical_rich_text: true,
            },
        )
        .expect("format report");

        assert!(
            report
                .output
                .contains("[effect .sparkle amp=2px]effect[/effect][p]")
        );
        assert_eq!(report.output.contains("\r\n"), source.contains("\r\n"));
    }
}

#[test]
fn canonical_rich_text_visits_flow_else_branches() {
    let source = "flow opening {\n    if ready {\n        let then_line = alice[[.shake]then[/][p]]\n    } else {\n        let else_line = alice[[.pulse]else[/][p]]\n    }\n}\n";
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains("[effect .shake]then[/effect][p]"));
    assert!(report.output.contains("[effect .pulse]else[/effect][p]"));
}

#[test]
fn canonical_rich_text_uses_typed_dialogue_content_ranges() {
    let source = "flow opening {\n    let handles = render(\"[.shake]effect[/][p]\")()[[.shake]effect[/][p]]\n}\n";
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains("render(\"[.shake]effect[/][p]\")()"));
    assert!(
        report
            .output
            .contains("[[effect .shake]effect[/effect][p]]")
    );
    assert_eq!(report.output.matches("[effect .shake]").count(), 1);
}

#[test]
fn canonical_rich_text_projects_multiline_dialogue_application_across_crlf() {
    let source_lf = "flow opening {\n    let handles = alice()[\n        Intro\n        [.sparkle amp=2px]effect[/][p]\n    ]\n}\n";
    for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
        let report = format_fixture(
            &source,
            FormatOptions {
                canonical_rich_text: true,
            },
        )
        .expect("format report");

        assert!(
            report
                .output
                .contains("[effect .sparkle amp=2px]effect[/effect][p]")
        );
        assert_eq!(report.output.contains("\r\n"), source.contains("\r\n"));
    }
}

#[test]
fn canonical_rich_text_expands_nested_typed_proxy_objects() {
    let source = "#[text_proxy(kind=\"keyword\", default_hit=true)]\npub struct KeywordHit {\n    channel: String\n}\n\n#[rich_text_proxy(kind=\"hover\", default_hit=false)]\npub struct HoverHit {\n    layer: String\n}\n\nflow opening {\n    let line = alice[[.hotspot type=KeywordHit channel=inventory][.HoverHit tone=alert]multi[/][/][.sparkle amp=2px]effect[/][p]]\n}\n";
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains(
        "[object .hotspot type=KeywordHit channel=inventory][object .HoverHit type=HoverHit tone=alert]multi[/object][/object]"
    ));
    assert!(
        report
            .output
            .contains("[effect .sparkle amp=2px]effect[/effect]")
    );
    assert!(!report.output.contains("[/]"));
}

#[test]
fn canonical_rich_text_preserves_unknown_dot_selectors_without_marker_inference() {
    let source = "flow opening {\n    let line = alice[[.keyword]word[/][.mark ignored=value]mark[/][mark .checkpoint][.shake]there[/][p]]\n}\n";
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains("[.keyword]word[/]"));
    assert!(report.output.contains("[.mark ignored=value]mark[/]"));
    assert!(report.output.contains("[mark .checkpoint]"));
    assert!(!report.output.contains("[mark .keyword]"));
    assert!(!report.output.contains("[mark .mark]"));
    assert!(report.output.contains("[effect .shake]there[/effect]"));
}

#[test]
fn source_code_actions_include_only_canonical_rich_text_rewrite() {
    let source = "flow opening {\n    let line = alice[[.keyword][.vertical_rl]縦[/]]\n}\n";
    let actions = source_code_actions(fixture_document(source)).expect("source code actions");

    assert_eq!(actions.len(), 1);
    let action = &actions[0];
    assert_eq!(action.id, "arcweft.canonicalRichText");
    assert_eq!(action.label, "Canonicalize inferred rich-text tags");
    let edit = action.edit.as_ref().expect("canonical action has edit");
    assert_eq!(edit.start, 0);
    assert_eq!(edit.end, source.len());
    assert!(
        edit.replacement
            .contains("[layout .vertical_rl]縦[/layout]")
    );
    assert!(edit.replacement.contains("[.keyword]"));
    assert!(!edit.replacement.contains("[mark .keyword]"));
}

#[test]
fn canonical_rich_text_keeps_ranges_after_natural_apostrophes() {
    let source = "flow opening {\n    let handles = alice()[don't [fx warning()]stop[/fx] [.shake]now[/][p]]\n}\n";
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[don't [fx warning()]stop[/fx] [effect .shake]now[/effect][p]]")
    );
}
