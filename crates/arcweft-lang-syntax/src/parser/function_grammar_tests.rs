use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/function-shadow").unwrap(),
        SourceName::path("function-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn kinds(text: &str) -> Vec<SyntaxKind> {
    parse_shadow_document(&document(text))
        .unwrap()
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect()
}

#[test]
fn ordinary_function_owns_curried_signature_contracts_and_block_descendants() {
    let source = concat!(
        "/// Applies a route in two call groups.\n",
        "#[inline]\n",
        "pub fn apply<'a, T>(state: &'a State)(route: T) -> Result<T, Error>\n",
        "where T: Clone + Debug\n",
        "requires state.ready()\n",
        "ensures result == route\n",
        "{\n",
        "    let next: T = route\n",
        "    next\n",
        "}\n",
    );
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    for expected in [
        SyntaxKind::FunctionItem,
        SyntaxKind::DocBlock,
        SyntaxKind::OuterAttribute,
        SyntaxKind::Visibility,
        SyntaxKind::NameDefinition,
        SyntaxKind::GenericParameterGroup,
        SyntaxKind::LifetimeParameter,
        SyntaxKind::TypeParameter,
        SyntaxKind::ReturnType,
        SyntaxKind::WhereClause,
        SyntaxKind::RequiresClause,
        SyntaxKind::EnsuresClause,
        SyntaxKind::FunctionBody,
        SyntaxKind::Block,
        SyntaxKind::LetStatement,
        SyntaxKind::PathExpression,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::FixedParameterGroup)
            .count(),
        2
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_function_body_does_not_consume_the_following_proof() {
    let source = "fn missing(value: Int) -> Int\nproof next() = ()\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::FunctionItem)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::ProofItem)
            .count(),
        1
    );
    assert!(kinds.contains(&SyntaxKind::MissingBody));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.decl.missing_body")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_function_close_synchronizes_before_the_following_declaration() {
    let source = "fn broken(value: Int) -> Int { let local = value\nproof next() = ()\n";
    let next_start = source.find("proof next").unwrap();
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::FunctionItem)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::ProofItem)
            .count(),
        1
    );
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.function.missing_block_close"
            && diagnostic.range().start() == next_start
            && diagnostic.range().end() == next_start
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn function_header_without_parameters_gets_typed_missing_group_recovery() {
    let source = "fn missing -> Int {}\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let parsed_kinds = kinds(source);

    assert!(parsed_kinds.contains(&SyntaxKind::FixedParameterGroup));
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.decl.invalid_header")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn removed_function_role_spellings_do_not_form_function_items() {
    for role in ["task", "dialogue", "stream"] {
        let source = format!("{role} fn removed() -> Unit {{}}\n");
        let built = parse_shadow_document(&document(&source)).unwrap();
        let parsed_kinds = built
            .index()
            .entries()
            .iter()
            .map(UnattachedGrammarEntry::kind)
            .collect::<Vec<_>>();

        assert!(
            !parsed_kinds.contains(&SyntaxKind::FunctionItem),
            "removed `{role} fn` reached the function grammar: {parsed_kinds:?}"
        );
        assert_eq!(built.green().to_string(), source);
    }
}
