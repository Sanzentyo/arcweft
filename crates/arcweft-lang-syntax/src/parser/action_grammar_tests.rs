use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_document;
use crate::grammar::build::{GrammarBuild, UnattachedGrammarEntry};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-action").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_document(&document(source), crate::parser::ParseOptions::default())
        .expect("Action grammar builds")
}

fn count_kind(built: &GrammarBuild, kind: SyntaxKind) -> usize {
    built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .filter(|actual| *actual == kind)
        .count()
}

fn source_range(source: &str, fragment: &str) -> SourceRange {
    let start = source.find(fragment).expect("fixture fragment");
    SourceRange::new(start, start + fragment.len())
}

#[test]
fn canonical_action_owns_a_typed_bodyless_channel_signature() {
    let source = "pub action feedback_submit(value: Feedback, count: Count)\n";
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ActionDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::ActionSignature), 1);
    assert_eq!(count_kind(&built, SyntaxKind::Parameter), 2);
    assert_eq!(count_kind(&built, SyntaxKind::ColonNode), 2);
    assert_eq!(count_kind(&built, SyntaxKind::PathType), 2);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn zero_parameter_action_is_a_clean_unit_payload_channel() {
    let source = "action Continue()\n";
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ActionDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::FixedParameterGroup), 1);
    assert_eq!(count_kind(&built, SyntaxKind::Parameter), 0);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn action_defaults_return_types_and_bodies_are_rejected_without_raw_reparse() {
    let source = concat!(
        "action Submit(value: String = \"x\")\n",
        "action Query() -> String\n",
        "action Run() { return }\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ActionDeclarationItem), 3);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.action.default_not_allowed")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.action.return_not_allowed")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.action.body_not_allowed")
    );
    let default = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.action.default_not_allowed")
        .expect("default diagnostic");
    assert_eq!(default.range(), source_range(source, "= \"x\""));
    let return_type = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.action.return_not_allowed")
        .expect("return diagnostic");
    assert_eq!(return_type.range(), source_range(source, "-> String"));
    let body = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.action.body_not_allowed")
        .expect("body diagnostic");
    assert_eq!(body.range(), source_range(source, "{ return }"));
    assert!(count_kind(&built, SyntaxKind::ErrorNode) >= 3);
    assert_eq!(count_kind(&built, SyntaxKind::EqualsNode), 1);
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn action_missing_group_and_non_binding_parameter_remain_typed_recovery() {
    let source = concat!("action Missing\n", "action Invalid((left, right): Pair)\n");
    let built = parse(source);
    assert!(count_kind(&built, SyntaxKind::OpenParenNode) >= 2);
    assert!(count_kind(&built, SyntaxKind::CloseParenNode) >= 2);
    assert!(count_kind(&built, SyntaxKind::TuplePattern) >= 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.action.missing_parameters")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.action.invalid_parameter")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn action_parameters_own_authored_and_missing_punctuation() {
    let source = concat!(
        "action Typed(value: String)\n",
        "action MissingType(value:)\n",
        "action MissingColon(value)\n",
        "action Defaulted(value: String = make())\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ColonNode), 4);
    assert_eq!(count_kind(&built, SyntaxKind::EqualsNode), 1);
    assert!(built.index().entries().iter().all(|entry| {
        entry.kind() != SyntaxKind::ColonNode || entry.role() == SyntaxRole::Colon
    }));
    assert!(built.index().entries().iter().all(|entry| {
        entry.kind() != SyntaxKind::EqualsNode || entry.role() == SyntaxRole::Equals
    }));
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.parameter.missing_type")
            .count(),
        2
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn action_nonempty_invalid_type_keeps_the_authored_parameter_close() {
    let source = "action Broken(value: @)\n";
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ErrorType), 1);
    assert_eq!(count_kind(&built, SyntaxKind::CloseParenNode), 1);
    assert_eq!(built.diagnostics().len(), 1);
    assert_eq!(built.diagnostics()[0].code(), "syntax.type.invalid");
    assert_eq!(built.diagnostics()[0].range(), source_range(source, "@"));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn action_trailing_syntax_uses_the_shared_declaration_diagnostic() {
    let source = "action Continue() effects { ui.write }\n";
    let built = parse(source);
    let diagnostic = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.declaration.trailing_syntax")
        .expect("trailing syntax diagnostic");
    assert_eq!(
        diagnostic.range(),
        source_range(source, "effects { ui.write }")
    );
    assert_eq!(count_kind(&built, SyntaxKind::ErrorNode), 1);
    assert_eq!(built.green().to_string(), source);
}
