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
fn nested_type_and_pattern_families_have_independent_events() {
    let source = "proof nested((head, [first, ..rest], TruckResult { score, rank: mut r, .. }, ev .Choice(value)): (&'a mut Comparator<Option<(Int, String)> | [U8; 32]>) -> Result<Bool, Error>, .Some(left) | .None: Option<Int>) where Comparator<Option<Int>>: Callable<(Int, String)> + Send = true\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    for expected in [
        SyntaxKind::FunctionType,
        SyntaxKind::ReferenceType,
        SyntaxKind::GenericApplicationType,
        SyntaxKind::SumType,
        SyntaxKind::TupleType,
        SyntaxKind::ArrayType,
        SyntaxKind::TypeArgument,
        SyntaxKind::WherePredicate,
        SyntaxKind::TuplePattern,
        SyntaxKind::SequencePattern,
        SyntaxKind::RecordPattern,
        SyntaxKind::RecordPatternField,
        SyntaxKind::MutableBindingPattern,
        SyntaxKind::WholeBindingPattern,
        SyntaxKind::VariantPattern,
        SyntaxKind::RestPattern,
        SyntaxKind::OrPattern,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::VariantPattern)
            .count(),
        3
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::RestPattern)
            .count(),
        2
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn shared_statement_families_keep_typed_identity_and_children() {
    let source = "proof statements(x: Int) { let y: Int = x; target = y; 'line <- y; return y; out 'exit y; goto next; defer cleanup(); yield y; signal y; wait(y); on ready => lemma(y); close y; select y; break 'loop y; continue 'loop; lemma(y); y; }\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    for expected in [
        SyntaxKind::LetStatement,
        SyntaxKind::AssignmentStatement,
        SyntaxKind::LifetimeSetStatement,
        SyntaxKind::ReturnStatement,
        SyntaxKind::OutStatement,
        SyntaxKind::GotoStatement,
        SyntaxKind::DeferStatement,
        SyntaxKind::YieldStatement,
        SyntaxKind::SignalStatement,
        SyntaxKind::WaitStatement,
        SyntaxKind::OnStatement,
        SyntaxKind::CloseStatement,
        SyntaxKind::SelectStatement,
        SyntaxKind::BreakStatement,
        SyntaxKind::ContinueStatement,
        SyntaxKind::ProofCallStatement,
        SyntaxKind::ExpressionStatement,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn control_statements_own_conditions_patterns_blocks_and_match_arms() {
    let source = "proof control(xs: List<Int>, ready: Bool) { if ready { lemma(1); } else { lemma(0); }; while ready { break; }; while let .Some(x) = next when ready { continue; }; for item in xs { lemma(item); }; loop { break 1; }; match next { .Some(x) when ready => lemma(x), .None => { return 0; } }; thread worker { yield 1; }; defer { close resource; }; unsafe lifetime @unsafe.test { lemma(1); }; }\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    for expected in [
        SyntaxKind::IfStatement,
        SyntaxKind::WhileStatement,
        SyntaxKind::WhileLetStatement,
        SyntaxKind::ForStatement,
        SyntaxKind::LoopStatement,
        SyntaxKind::MatchStatement,
        SyntaxKind::MatchArm,
        SyntaxKind::ThreadStatement,
        SyntaxKind::DeferBlockStatement,
        SyntaxKind::UnsafeLifetimeStatement,
        SyntaxKind::VariantPattern,
        SyntaxKind::Block,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::MatchArm)
            .count(),
        2
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn let_statement_variants_share_pattern_and_initializer_authority() {
    let source = "proof lets(value: Option<Int>) { let .Some(x) = value else { return; }; let picked = choice @choice.test { }; let scoped = scope named { 1 }; let repeated = loop { break 1; }; let waited = try await task(); let action = receive action(@action.ok); }\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    for expected in [
        SyntaxKind::LetElseStatement,
        SyntaxKind::LetChoiceStatement,
        SyntaxKind::LetScopeStatement,
        SyntaxKind::LetLoopStatement,
        SyntaxKind::LetAwaitStatement,
        SyntaxKind::LetActionReceiveStatement,
        SyntaxKind::VariantPattern,
        SyntaxKind::Block,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn malformed_statement_is_typed_without_consuming_following_sibling() {
    let source = "proof recovered() { ???; lemma(); }\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&SyntaxKind::ErrorStatement));
    assert!(kinds.contains(&SyntaxKind::ProofCallStatement));
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
