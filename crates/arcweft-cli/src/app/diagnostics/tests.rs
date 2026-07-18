use annotate_snippets::Renderer;
use arcweft_agent_repl::{AgentDiagnosticProjector, AgentParserDiagnosticProjection};
use arcweft_lang_syntax::{
    parser::parse_source,
    parser::recovery::{ParseError, ParseErrorKind},
    source::ParsedSource,
};
use arcweft_lsp::{
    diagnostics::DiagnosticProjector,
    positions::{LineIndex, PositionEncoding},
};
use arcweft_source::{
    Diagnostic, DiagnosticApplicability, DiagnosticLabel, DiagnosticSeverity, DiagnosticSuggestion,
    SourceDocument, SourceEdit, SourceName, SourceRange, SourceSpanValidationError,
};
use serde_json::{Value, json};

use super::{DiagnosticSource, diagnostic_groups};

const SOURCE: &str = "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";
const CORRECTED_SOURCE: &str =
    "pub view Card() {\n    export part タイトル as heading\n    Panel()\n}\n";

struct LogicalFixture {
    parsed: ParsedSource,
    error: ParseError,
    diagnostic: Diagnostic,
}

struct TestOnlyEditFixture {
    diagnostic: Diagnostic,
    edit: SourceEdit,
}

fn logical_fixture() -> LogicalFixture {
    let parsed = parse_source(SOURCE);
    let matching = parsed
        .errors()
        .iter()
        .filter(|error| error.kind() == ParseErrorKind::ViewExportPartMissingAs)
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected one missing-`as` diagnostic: {:?}",
        parsed.errors()
    );
    assert_eq!(parsed.errors().len(), 1);
    let error = (*matching[0]).clone();
    let diagnostic = error.diagnostic(parsed.document());
    LogicalFixture {
        parsed,
        error,
        diagnostic,
    }
}

fn test_only_edit_fixture(document: &SourceDocument) -> TestOnlyEditFixture {
    let edit = SourceEdit::new(
        document
            .span(SourceRange::new(47, 47))
            .expect("fixture insertion span"),
        "as ",
    );
    let diagnostic = Diagnostic::new(
        DiagnosticSeverity::Error,
        "View part export needs `as` before its public name",
    )
    .with_code("view::export_part_missing_as")
    .with_label(DiagnosticLabel::primary(
        document
            .span(SourceRange::new(47, 54))
            .expect("fixture diagnostic span"),
        None,
    ))
    .with_note("expected: as public_name")
    .with_suggestion(
        DiagnosticSuggestion::new(
            "insert missing `as` keyword",
            DiagnosticApplicability::MachineApplicable,
        )
        .with_edit(edit.clone()),
    );
    TestOnlyEditFixture { diagnostic, edit }
}

#[test]
fn adapter_parity_reuses_one_complete_source_derived_logical_fixture() {
    let fixture = logical_fixture();
    let error = &fixture.error;

    assert_eq!(error.kind(), ParseErrorKind::ViewExportPartMissingAs);
    assert_eq!(error.code(), "view::export_part_missing_as");
    assert_eq!(
        error.label(),
        ParseErrorKind::ViewExportPartMissingAs.label()
    );
    assert_eq!(error.range().start(), 47);
    assert_eq!(error.range().end(), 54);
    assert_eq!(error.expected(), &["as public_name"]);
    assert_eq!(error.found(), None);
    assert_eq!(
        error.message(),
        "View part export needs `as` before its public name"
    );
    assert_eq!(error.recovery().len(), 1);
    assert_eq!(
        error.recovery()[0].applicability(),
        DiagnosticApplicability::Unspecified
    );
    assert!(error.recovery()[0].edits().is_empty());

    let logical = &fixture.diagnostic;
    assert_eq!(logical.severity(), DiagnosticSeverity::Error);
    assert_eq!(
        logical.code().map(arcweft_source::DiagnosticCode::as_str),
        Some("view::export_part_missing_as")
    );
    assert_eq!(logical.labels().len(), 1);
    assert_eq!(logical.labels()[0].span().range(), SourceRange::new(47, 54));
    assert_eq!(logical.labels()[0].message(), None);
    assert_eq!(logical.notes(), &["expected: as public_name"]);
    assert_eq!(logical.suggestions().len(), 1);
    assert_eq!(
        logical.suggestions()[0].message(),
        "use as public_name syntax"
    );
    assert_eq!(
        logical.suggestions()[0].applicability(),
        DiagnosticApplicability::Unspecified
    );
    assert!(logical.suggestions()[0].edits().is_empty());

    let cli_source = DiagnosticSource::new(fixture.parsed.document());
    let cli_groups = diagnostic_groups(logical, &cli_source);
    assert_eq!(cli_groups.len(), 2);
    let cli_rendered = Renderer::plain().render(&cli_groups);
    assert!(cli_rendered.contains(
        "error[view::export_part_missing_as]: View part export needs `as` before its public name"
    ));
    assert!(cli_rendered.contains("expected: as public_name"));
    assert!(cli_rendered.contains("use as public_name syntax"));
    assert!(!cli_rendered.contains("found `"));
    assert!(
        !cli_rendered
            .lines()
            .any(|line| line.trim_start().starts_with('+'))
    );

    for (encoding, start, end) in [
        (PositionEncoding::Utf16, 21, 28),
        (PositionEncoding::Utf8, 29, 36),
    ] {
        let line_index = LineIndex::new(SOURCE, encoding);
        let lsp = DiagnosticProjector::new(fixture.parsed.document(), &line_index)
            .project(logical)
            .expect("same logical diagnostic projects to LSP");
        assert_eq!(lsp.source.as_deref(), Some("arcweft"));
        assert_eq!(
            serde_json::to_value(&lsp.code).expect("LSP code serializes"),
            json!("view::export_part_missing_as")
        );
        assert_eq!(lsp.range.start.line, 1);
        assert_eq!(lsp.range.start.character, start);
        assert_eq!(lsp.range.end.line, 1);
        assert_eq!(lsp.range.end.character, end);
        assert_eq!(
            lsp.data,
            Some(json!({
                "suggestions": [{
                    "message": "use as public_name syntax",
                    "applicability": "unspecified",
                    "edits": [],
                }],
            }))
        );
    }

    let agent_shared = AgentDiagnosticProjector::new(fixture.parsed.document())
        .project(logical)
        .expect("same logical diagnostic projects to Agent");
    assert_eq!(
        agent_shared.json()["range"],
        json!({
            "coordinate_space": "source_utf8_bytes",
            "start": 47,
            "end": 54,
        })
    );
    assert_eq!(
        agent_shared.json()["recovery"][0],
        json!({
            "message": "use as public_name syntax",
            "applicability": "unspecified",
            "edits": [],
        })
    );

    let agent_parser =
        AgentParserDiagnosticProjection::source_local(error, fixture.parsed.document())
            .expect("typed parser diagnostic projects to Agent");
    assert_eq!(
        agent_parser.json(),
        json!({
            "kind": ParseErrorKind::ViewExportPartMissingAs.label(),
            "code": "view::export_part_missing_as",
            "message": "View part export needs `as` before its public name",
            "range": {
                "coordinate_space": "source_utf8_bytes",
                "start": 47,
                "end": 54,
            },
            "expected": ["as public_name"],
            "found": Value::Null,
            "recovery": [{
                "message": "use as public_name syntax",
                "applicability": "unspecified",
                "edits": [],
            }],
        })
    );
    assert_eq!(
        agent_parser.human(),
        "error[view::export_part_missing_as] source_utf8_bytes 47..54: View part export needs `as` before its public name\nexpected: as public_name\nhelp[unspecified]: use as public_name syntax"
    );
}

#[test]
fn adapter_parity_keeps_the_concrete_edit_in_a_separate_typed_test_fixture() {
    let source_derived = logical_fixture();
    assert!(source_derived.error.recovery()[0].edits().is_empty());

    let document = source_derived.parsed.document();
    let fixture = test_only_edit_fixture(document);
    let suggestion = &fixture.diagnostic.suggestions()[0];
    assert_eq!(
        suggestion.applicability(),
        DiagnosticApplicability::MachineApplicable
    );
    assert_eq!(suggestion.edits(), std::slice::from_ref(&fixture.edit));
    assert_eq!(
        fixture
            .edit
            .apply(document)
            .expect("exact revision edit applies"),
        CORRECTED_SOURCE
    );

    let cli_source = DiagnosticSource::new(document);
    let cli_groups = diagnostic_groups(&fixture.diagnostic, &cli_source);
    let cli_rendered = Renderer::plain().render(&cli_groups);
    assert!(cli_rendered.contains("insert missing `as` keyword"));
    assert_eq!(cli_rendered.matches("as heading").count(), 1);

    for (encoding, character) in [(PositionEncoding::Utf16, 21), (PositionEncoding::Utf8, 29)] {
        let line_index = LineIndex::new(SOURCE, encoding);
        let lsp = DiagnosticProjector::new(document, &line_index)
            .project(&fixture.diagnostic)
            .expect("test-only edit projects to LSP");
        let edit = &lsp.data.as_ref().expect("suggestion data")["suggestions"][0]["edits"][0];
        assert_eq!(edit["range"]["start"]["line"], 1);
        assert_eq!(edit["range"]["start"]["character"], character);
        assert_eq!(edit["range"]["end"], edit["range"]["start"]);
        assert_eq!(edit["replacement"], "as ");
        assert_eq!(
            lsp.data.as_ref().expect("suggestion data")["suggestions"][0]["applicability"],
            "machine_applicable"
        );
    }

    let agent = AgentDiagnosticProjector::new(document)
        .project(&fixture.diagnostic)
        .expect("test-only edit projects to Agent");
    assert_eq!(
        agent.json()["recovery"][0],
        json!({
            "message": "insert missing `as` keyword",
            "applicability": "machine_applicable",
            "edits": [{
                "range": {
                    "coordinate_space": "source_utf8_bytes",
                    "start": 47,
                    "end": 47,
                },
                "replacement": "as ",
            }],
        })
    );
}

#[test]
fn adapter_parity_omits_or_rejects_stale_diagnostic_and_edit_spans() {
    let source_derived = logical_fixture();
    let document = source_derived.parsed.document();
    let fixture = test_only_edit_fixture(document);
    let current = SourceDocument::try_new(
        document.identity().id().clone(),
        SourceName::path("view.arcw"),
        CORRECTED_SOURCE,
    )
    .expect("current source revision");
    let line_index = LineIndex::new(CORRECTED_SOURCE, PositionEncoding::Utf16);

    assert!(matches!(
        DiagnosticProjector::new(&current, &line_index).project(&fixture.diagnostic),
        Err(SourceSpanValidationError::WrongRevision { expected, actual })
            if expected == current.identity().revision()
                && actual == document.identity().revision()
    ));
    assert!(matches!(
        AgentDiagnosticProjector::new(&current).project(&fixture.diagnostic),
        Err(SourceSpanValidationError::WrongRevision { expected, actual })
            if expected == current.identity().revision()
                && actual == document.identity().revision()
    ));

    let cli_source = DiagnosticSource::new(&current);
    let cli_groups = diagnostic_groups(&fixture.diagnostic, &cli_source);
    let cli_rendered = Renderer::plain().render(&cli_groups);
    assert!(cli_rendered.contains(
        "diagnostic span belongs to a different source revision; source excerpt was omitted"
    ));
    assert!(!cli_rendered.contains("as heading"));

    let current_diagnostic = Diagnostic::new(DiagnosticSeverity::Error, "current diagnostic")
        .with_label(DiagnosticLabel::primary(
            current
                .span(SourceRange::new(50, 57))
                .expect("current heading span"),
            None,
        ))
        .with_suggestion(
            DiagnosticSuggestion::new("stale edit", DiagnosticApplicability::MachineApplicable)
                .with_edit(fixture.edit),
        );
    let current_groups = diagnostic_groups(&current_diagnostic, &cli_source);
    let current_rendered = Renderer::plain().render(&current_groups);
    assert!(current_rendered.contains(
        "diagnostic span belongs to a different source revision; source excerpt was omitted"
    ));
    assert!(
        !current_rendered
            .lines()
            .any(|line| line.trim_start().starts_with('+'))
    );
}
