use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        flow::{FlowItem, Stmt},
        items::Item,
    },
    expr::{ExprParseError, parse_expr},
    parser::recovery::{ParseError, ParseErrorKind},
};
use arcweft_source::DiagnosticCode;

const UNKNOWN_MODE_SOURCE: &str = "flow demo {\n    assert.assume(true)\n}\n";

#[test]
fn statement_unknown_mode_preserves_typed_error_projection_and_raw_recovery() {
    let parsed = parse_diagnostic_fixture(UNKNOWN_MODE_SOURCE);
    let errors: &[ParseError] = parsed.errors();
    let [error] = errors else {
        panic!("expected exactly one typed parser error, got {errors:?}");
    };

    assert_eq!(error.kind(), ParseErrorKind::AssertionUnknownMode);
    assert_eq!(error.code(), "syntax.assert.unknown_mode");
    assert_eq!(error.range(), &TextRange::new(23, 29));
    assert_eq!(error.message(), "unknown assertion mode");
    assert!(error.expected().is_empty());
    assert_eq!(error.found(), None);
    assert!(error.recovery().is_empty());

    let diagnostic = error.diagnostic(parsed.document());
    assert_eq!(
        diagnostic.code().map(DiagnosticCode::as_str),
        Some("syntax.assert.unknown_mode")
    );
    assert_eq!(diagnostic.message(), "unknown assertion mode");
    assert_eq!(diagnostic.labels()[0].span().range().as_range(), 23..29);

    let [Item::Flow(flow)] = parsed.typed_tree().items() else {
        panic!("expected one flow");
    };
    let [FlowItem::Stmt(Stmt::Raw(raw))] = flow.body() else {
        panic!("failed assertion must remain one raw statement");
    };
    assert_eq!(raw.source(), "assert.assume(true)");
    assert_eq!(raw.range(), Some(TextRange::new(16, 35)));
}

#[test]
fn statement_recovery_keeps_following_flow_items_parseable() {
    let parsed =
        parse_diagnostic_fixture("flow demo {\n    assert.assume(true)\n    return \"done\"\n}\n");
    assert_eq!(parsed.errors().len(), 1);

    let [Item::Flow(flow)] = parsed.typed_tree().items() else {
        panic!("expected one flow");
    };
    let [
        FlowItem::Stmt(Stmt::Raw(raw)),
        FlowItem::Stmt(Stmt::Return { .. }),
    ] = flow.body()
    else {
        panic!("expected raw assertion recovery followed by typed return");
    };
    assert_eq!(raw.source(), "assert.assume(true)");
    assert_eq!(raw.range(), Some(TextRange::new(16, 35)));
}

#[test]
fn unknown_mode_code_is_intentionally_shared_across_distinct_owners() {
    let parsed = parse_diagnostic_fixture(UNKNOWN_MODE_SOURCE);
    let statement_errors: &[ParseError] = parsed.errors();
    let [statement_error] = statement_errors else {
        panic!("expected one statement parser error");
    };
    let expression_error: ExprParseError =
        parse_expr("assert.assume(true)").expect_err("reserved expression call must fail");

    assert_eq!(statement_error.kind(), ParseErrorKind::AssertionUnknownMode);
    assert_eq!(expression_error.code(), statement_error.code());
    assert_eq!(expression_error.code(), "syntax.assert.unknown_mode");
    assert_eq!(statement_error.message(), "unknown assertion mode");
    assert_eq!(expression_error.to_string(), "unknown assertion mode");
    assert_eq!(statement_error.range(), &TextRange::new(23, 29));
    assert_eq!(expression_error.range(), TextRange::new(0, 19));
}

fn parse_diagnostic_fixture(
    source: impl Into<String>,
) -> arcweft_lang_syntax::source::ParsedSource {
    let document = std::sync::Arc::new(
        arcweft_source::SourceDocument::try_new(
            arcweft_source::SourceDocumentId::try_new("arcweft-test://syntax/diagnostic-ownership")
                .expect("fixed test document ID is valid"),
            arcweft_source::SourceName::path("diagnostic-ownership.arcw"),
            source.into(),
        )
        .expect("test source document"),
    );
    arcweft_lang_syntax::parser::parse_document_with_source(
        document,
        arcweft_lang_syntax::parser::ParseOptions::default(),
    )
}
