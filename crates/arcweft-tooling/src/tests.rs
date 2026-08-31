use crate::{
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
