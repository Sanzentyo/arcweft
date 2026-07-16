use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::UnattachedGrammarEntry;
use crate::grammar::kinds::SyntaxKind;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/predicate-proof-shadow").unwrap(),
        SourceName::path("predicate-proof-shadow.arcw"),
        text,
    )
    .unwrap()
}

#[test]
fn complete_headers_emit_distinct_typed_descendant_families_losslessly() {
    let source = "pub proof ordered<'a, T>((left, right): (T, T), cmp: Comparator<T>) -> Bool where T: Ord requires cmp.ready() ensures result = left == right\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();
    for expected in [
        SyntaxKind::ProofItem,
        SyntaxKind::Visibility,
        SyntaxKind::NameDefinition,
        SyntaxKind::GenericParameterGroup,
        SyntaxKind::LifetimeParameter,
        SyntaxKind::TypeParameter,
        SyntaxKind::FixedParameterGroup,
        SyntaxKind::Parameter,
        SyntaxKind::TuplePattern,
        SyntaxKind::TupleType,
        SyntaxKind::ReturnType,
        SyntaxKind::WhereClause,
        SyntaxKind::RequiresClause,
        SyntaxKind::EnsuresClause,
        SyntaxKind::ExpressionBody,
        SyntaxKind::BinaryExpression,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn proof_block_separates_statements_tail_braces_and_omitted_tail() {
    let with_tail = "proof p() -> Int { let x: Int = 1; lemma(x); assert.prove(x == 1); x }\n";
    let built = parse_shadow_document(&document(with_tail)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&SyntaxKind::ProofBlock));
    assert!(kinds.contains(&SyntaxKind::OpenBraceNode));
    assert!(kinds.contains(&SyntaxKind::CloseBraceNode));
    assert!(kinds.contains(&SyntaxKind::LetStatement));
    assert!(kinds.contains(&SyntaxKind::ProofCallStatement));
    assert!(kinds.contains(&SyntaxKind::CallExpression));
    assert!(kinds.contains(&SyntaxKind::AssertionStatement));
    assert!(kinds.contains(&SyntaxKind::PathExpression));
    assert!(!kinds.contains(&SyntaxKind::OmittedBlockTail));
    assert_eq!(built.green().to_string(), with_tail);

    let empty = parse_shadow_document(&document("proof unit() {}\n")).unwrap();
    assert!(
        empty
            .index()
            .entries()
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::OmittedBlockTail)
    );
    assert_eq!(empty.green().to_string(), "proof unit() {}\n");
}

#[test]
fn expression_events_preserve_precedence_arguments_and_postfix_identity() {
    let source =
        "proof p(a: Int, b: Int, c: Int, list: List<Int>) = lemma(a + b * c, list[0]?.field)?\n";
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
            .filter(|kind| **kind == SyntaxKind::BinaryExpression)
            .count(),
        2
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::CallArgument)
            .count(),
        2
    );
    for expected in [
        SyntaxKind::CallExpression,
        SyntaxKind::IndexExpression,
        SyntaxKind::SelectExpression,
        SyntaxKind::TryExpression,
        SyntaxKind::Path,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn entity_style_proof_name_uses_ordinary_error_item_recovery() {
    let source = "proof @legacy.fact() {}\nproof current() = ()\n";
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
            .filter(|kind| matches!(kind, SyntaxKind::ProofItem | SyntaxKind::ErrorItem))
            .copied()
            .collect::<Vec<_>>(),
        [SyntaxKind::ErrorItem, SyntaxKind::ProofItem]
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn current_header_recovery_retains_missing_nodes_and_order_diagnostics() {
    let missing_name = parse_shadow_document(&document("proof () = ()\n")).unwrap();
    assert!(
        missing_name
            .index()
            .entries()
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::MissingName)
    );
    assert_eq!(missing_name.missing_tokens().len(), 1);
    assert_eq!(
        missing_name.diagnostics()[0].code(),
        "syntax.proof.missing_name"
    );

    let missing_parameters = parse_shadow_document(&document("predicate ready = true\n")).unwrap();
    assert_eq!(missing_parameters.missing_tokens().len(), 2);
    assert!(
        missing_parameters
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.predicate.missing_parameters")
    );

    let malformed =
        parse_shadow_document(&document("proof p()() ensures true requires true = ()\n")).unwrap();
    let codes = malformed
        .diagnostics()
        .iter()
        .map(crate::grammar::event::PendingSyntaxDiagnostic::code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&"syntax.proof.malformed_header"));
    assert!(codes.contains(&"syntax.contract.invalid_clause_order"));
    assert_eq!(
        malformed.green().to_string(),
        "proof p()() ensures true requires true = ()\n"
    );
}

#[test]
fn predicate_authored_return_is_retained_as_current_typed_recovery() {
    let source = "predicate positive(x: Int) -> Bool = x > 0\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    assert!(
        built
            .index()
            .entries()
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::ReturnType)
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.predicate.return_not_allowed")
    );
    assert_eq!(built.green().to_string(), source);
}
