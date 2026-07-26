use crate::{
    code_actions::source_code_actions,
    edit::apply_text_edits,
    format::format_document,
    model::{FormatOptions, TextEdit, ToolingEditReport, ToolingError},
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
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
    let source = "flow @flow.opening opening {\n    alice: hi[p]\n}\n";
    let report = format_fixture(source, FormatOptions::default()).expect("format report");
    assert!(!report.changed);
    assert_eq!(report.output, source);
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
fn canonical_rich_text_expands_only_inferred_tag_families() {
    let source = "flow @flow.opening opening {\n    alice: ｜漢字《かんじ》 $(name)[.keyword][.sparkle amp=2px]there[/][.vertical_rl]縦[/][page]\n}\n";
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains("｜漢字《かんじ》"));
    assert!(report.output.contains("$(name)"));
    assert!(report.output.contains("[mark .keyword]"));
    assert!(
        report
            .output
            .contains("[effect .sparkle amp=2px]there[/effect]")
    );
    assert!(report.output.contains("[layout .vertical_rl]縦[/layout]"));
    assert!(report.output.contains("[page]"));
}

#[test]
fn canonical_rich_text_preserves_explicit_fx_and_quoted_brackets() {
    let source = "flow @flow.opening opening {\n    alice: [fx warning(label=\"urgent warning\")]important[/fx][.sparkle note=\"contains ] safely\"]effect[/][p]\n}\n";
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
    let source_lf = "flow @flow.opening opening {\n    alice:\n        Intro\n        [.sparkle amp=2px]effect[/][p]\n}\n";
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
    let source = "flow @flow.opening opening {\n    if ready {\n        alice: [.shake]then[/][p]\n    } else {\n        alice: [.pulse]else[/][p]\n    }\n}\n";
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
    let source = "flow @flow.opening opening {\n    let handles = render(\"[.shake]effect[/][p]\")()[[.shake]effect[/][p]]\n}\n";
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
    let source_lf = "flow @flow.opening opening {\n    let handles = alice()[\n        Intro\n        [.sparkle amp=2px]effect[/][p]\n    ]\n}\n";
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
    let source = "#[text_proxy(kind=\"keyword\", default_hit=true)]\npub struct KeywordHit {\n    channel: String\n}\n\n#[rich_text_proxy(kind=\"hover\", default_hit=false)]\npub struct HoverHit {\n    layer: String\n}\n\nflow @flow.opening opening {\n    alice: [.hotspot type=KeywordHit channel=inventory][.HoverHit tone=alert]multi[/][/][.sparkle amp=2px]effect[/][p]\n}\n";
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
fn canonical_rich_text_removes_marker_close_and_uses_reserved_classification() {
    let source = "flow @flow.opening opening {\n    alice: [.keyword]word[/][.mark ignored=value]mark[/][.shake]there[/][p]\n}\n";
    let report = format_fixture(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains("[mark .keyword]word"));
    assert!(report.output.contains("[mark .mark]mark"));
    assert!(report.output.contains("[effect .shake]there[/effect]"));
    assert!(!report.output.contains("[/]"));
}

#[test]
fn source_code_actions_include_only_canonical_rich_text_rewrite() {
    let source = "flow @flow.opening opening {\n    alice: [.keyword][.vertical_rl]縦[/]\n}\n";
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
    assert!(edit.replacement.contains("[mark .keyword]"));
    assert!(!edit.replacement.contains("[/]"));
}

#[test]
fn canonical_rich_text_keeps_ranges_after_natural_apostrophes() {
    let source = "flow @flow.opening opening {\n    let handles = alice()[don't [fx warning()]stop[/fx] [.shake]now[/][p]]\n}\n";
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
