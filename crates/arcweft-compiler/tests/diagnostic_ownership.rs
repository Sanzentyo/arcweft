use arcweft_lang_syntax::{
    incremental::{ParsedSource, SyntaxDatabase},
    lint::{SyntaxLint, SyntaxLintCode, SyntaxLintSeverity, lint_id_policy},
    parser::ParseOptions,
};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceRange, identity::SourceSnapshotId,
};
use std::sync::Arc;

fn parse_fixture(document: Arc<SourceDocument>) -> ParsedSource {
    let mut database = SyntaxDatabase::try_new().expect("test syntax database");
    database
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            document,
            ParseOptions::default(),
        )
        .expect("attached compiler syntax fixture")
}

#[test]
fn syntax_parser_preserves_statement_error_owner() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(
                "arcweft-test://compiler/diagnostic/statement-error-owner.arcw",
            )
            .expect("statement error fixture source ID"),
            SourceName::path("compiler/diagnostic/statement-error-owner.arcw"),
            "flow demo {\n    assert.assume(true)\n}\n",
        )
        .expect("statement error fixture source document"),
    );
    let parsed = parse_fixture(Arc::clone(&document));
    let [error] = parsed.diagnostics() else {
        panic!("expected one parser error, got {:?}", parsed.diagnostics());
    };

    assert_eq!(error.code(), "syntax.assert.unknown_mode");
    assert_eq!(error.primary().range(), SourceRange::new(23, 29));
    assert_eq!(error.message(), "unknown assertion mode");
}

#[test]
fn compiler_lint_facade_preserves_independent_lint_owner() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/diagnostic/lint-owner.arcw")
                .expect("lint fixture source ID"),
            SourceName::path("compiler/diagnostic/lint-owner.arcw"),
            "flow @flow.opening {\n}\n",
        )
        .expect("lint fixture source document"),
    );
    let parsed = parse_fixture(Arc::clone(&document));
    assert!(parsed.diagnostics().is_empty());

    let lints: Vec<SyntaxLint> = lint_id_policy(&parsed).expect("attached syntax lint projection");
    let lint = lints
        .iter()
        .find(|lint| lint.code() == SyntaxLintCode::ExplicitDeclId)
        .expect("explicit declaration id remains a syntax lint");

    assert_eq!(lint.code().stable_code(), "AWF0103");
    assert_eq!(lint.code().domain_name(), "style::explicit_decl_id");
    assert_eq!(lint.severity(), SyntaxLintSeverity::Hint);
}
