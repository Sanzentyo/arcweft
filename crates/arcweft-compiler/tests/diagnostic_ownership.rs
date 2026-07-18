use arcweft_compiler::parse::{lint_source_tree, parse_source_text};
use arcweft_lang_syntax::{
    ast::common::TextRange,
    lint::{SyntaxLint, SyntaxLintCode, SyntaxLintSeverity},
    parser::recovery::ParseErrorKind,
};

#[test]
fn compiler_parser_facade_preserves_statement_error_owner() {
    let parsed = parse_source_text("flow demo {\n    assert.assume(true)\n}\n");
    let [error] = parsed.errors() else {
        panic!("expected one parser error");
    };

    assert_eq!(error.kind(), ParseErrorKind::AssertionUnknownMode);
    assert_eq!(error.code(), "syntax.assert.unknown_mode");
    assert_eq!(error.range(), &TextRange::new(23, 29));
    assert_eq!(error.message(), "unknown assertion mode");
}

#[test]
fn compiler_lint_facade_preserves_independent_lint_owner() {
    let parsed = parse_source_text("flow @flow.opening {\n}\n");
    assert!(parsed.errors().is_empty());

    let lints: Vec<SyntaxLint> = lint_source_tree(parsed.typed_tree());
    let lint = lints
        .iter()
        .find(|lint| lint.code() == SyntaxLintCode::ExplicitDeclId)
        .expect("explicit declaration id remains a syntax lint");

    assert_eq!(lint.code().stable_code(), "AWF0103");
    assert_eq!(lint.code().domain_name(), "style::explicit_decl_id");
    assert_eq!(lint.severity(), SyntaxLintSeverity::Hint);
}
