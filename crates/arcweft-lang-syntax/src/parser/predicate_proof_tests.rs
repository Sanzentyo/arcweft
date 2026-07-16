use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use super::statement::parse_test_statement_block;
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

fn green_kind_count(node: &rowan::GreenNodeData, kind: SyntaxKind) -> usize {
    usize::from(node.kind() == rowan::SyntaxKind(kind as u16))
        + node
            .children()
            .map(|child| match child {
                rowan::NodeOrToken::Node(child) => green_kind_count(child, kind),
                rowan::NodeOrToken::Token(_) => 0,
            })
            .sum::<usize>()
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
fn canonical_multiline_contract_header_and_block_form_one_declaration() {
    let source = "pub predicate ordered<T>(pair: (T, T), cmp: Comparator<T>)\nwhere T: Ord\nrequires cmp.is_total()\nensures result\n{\n    let (left, right): (T, T) = pair\n    cmp.compare(left, right) <= 0\n}\nproof next() = ()\n";
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
            .filter(|kind| **kind == SyntaxKind::PredicateItem)
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
    for expected in [
        SyntaxKind::WhereClause,
        SyntaxKind::RequiresClause,
        SyntaxKind::EnsuresClause,
        SyntaxKind::PredicateBlock,
        SyntaxKind::LetStatement,
        SyntaxKind::BinaryExpression,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert!(!kinds.contains(&SyntaxKind::ErrorItem));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(green_kind_count(built.green(), SyntaxKind::LogicalLine), 6);
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn generic_header_angle_nesting_controls_logical_line_boundaries() {
    let source = "proof generic<\n    T: Ord,\n    U,\n>\n(\n    value: Result<T, U>\n)\n-> Result<\n    T,\n    U\n>\n= ()\n";
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
            .filter(|kind| **kind == SyntaxKind::ProofItem)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| {
                matches!(
                    **kind,
                    SyntaxKind::LifetimeParameter | SyntaxKind::TypeParameter
                )
            })
            .count(),
        2
    );
    assert!(kinds.contains(&SyntaxKind::ReturnType));
    assert!(kinds.contains(&SyntaxKind::GenericApplicationType));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(green_kind_count(built.green(), SyntaxKind::LogicalLine), 4);
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn documentation_and_outer_attributes_attach_to_the_following_proof() {
    let source = "/// Establishes the ordering lemma.\n/// Retains both documentation lines.\n#[verify]\n#[cfg(\n    debug\n)]\npub proof documented<T>(value: T)\nwhere T: Ord\n= ()\n";
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
            .filter(|kind| **kind == SyntaxKind::ProofItem)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::DocBlock)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::OuterAttribute)
            .count(),
        2
    );
    for expected in [
        SyntaxKind::Visibility,
        SyntaxKind::GenericParameterGroup,
        SyntaxKind::WhereClause,
        SyntaxKind::ExpressionBody,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert!(!kinds.contains(&SyntaxKind::ErrorItem));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(green_kind_count(built.green(), SyntaxKind::LogicalLine), 7);
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_body_does_not_consume_following_clean_declaration() {
    let source = "predicate missing(x: Bool)\nproof next() = ()\n";
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
            .filter(|kind| **kind == SyntaxKind::PredicateItem)
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
            .any(|diagnostic| diagnostic.code() == "syntax.predicate.missing_body")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_parameter_close_synchronizes_before_the_following_declaration() {
    let source = "proof broken(value: Int\nproof next() = ()\n";
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
            .filter(|kind| **kind == SyntaxKind::ProofItem)
            .count(),
        2
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(
                |diagnostic| diagnostic.code() == "syntax.proof.missing_parameter_close"
                    && diagnostic.range().start() == next_start
                    && diagnostic.range().end() == next_start
            )
    );
    assert!(
        built
            .missing_tokens()
            .iter()
            .any(|missing| missing.at() == next_start)
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_block_close_synchronizes_before_the_following_declaration() {
    let source = "proof broken() -> Int { let x = ;\nproof next() = ()\n";
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
            .filter(|kind| **kind == SyntaxKind::ProofItem)
            .count(),
        2
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(
                |diagnostic| diagnostic.code() == "syntax.proof.missing_block_close"
                    && diagnostic.range().start() == next_start
                    && diagnostic.range().end() == next_start
            )
    );
    assert!(
        built
            .missing_tokens()
            .iter()
            .any(|missing| missing.at() == next_start)
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_block_close_preserves_the_following_declarations_prefixes() {
    let source = "predicate broken() { let x = true\n/// The next proof remains documented.\n#[verify]\nproof next() = ()\n";
    let next_prefix_start = source.find("/// The next").unwrap();
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
            .filter(|kind| **kind == SyntaxKind::PredicateItem)
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
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::DocBlock)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::OuterAttribute)
            .count(),
        1
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(
                |diagnostic| diagnostic.code() == "syntax.predicate.missing_block_close"
                    && diagnostic.range().start() == next_prefix_start
            )
    );
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
fn control_expressions_emit_typed_conditions_patterns_branches_and_arms() {
    let source = "proof choose(value: Option<Int>, ready: Bool) -> Int = if let .Some(x) = value when ready { x } else { match value { .Some(v) when v > 0 => v, .None => 0 } }\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    for expected in [
        SyntaxKind::IfLetExpression,
        SyntaxKind::MatchExpression,
        SyntaxKind::BlockExpression,
        SyntaxKind::MatchArm,
        SyntaxKind::VariantPattern,
        SyntaxKind::BinaryExpression,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::BlockExpression)
            .count(),
        2
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::MatchArm)
            .count(),
        2
    );
    assert!(!kinds.contains(&SyntaxKind::ErrorExpression));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn comparison_operator_does_not_hide_if_expression_branches() {
    let source = "predicate less(a: Int, b: Int) = if a < b { true } else { false }\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert!(kinds.contains(&SyntaxKind::IfExpression));
    assert!(kinds.contains(&SyntaxKind::BinaryExpression));
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::BlockExpression)
            .count(),
        2
    );
    assert!(!kinds.contains(&SyntaxKind::ErrorExpression));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn closures_own_typed_parameters_return_types_bodies_and_grouping() {
    let source =
        "proof apply(value: Int) -> Int = (|x: Int| -> Int { let next = x + 1; next })(value)\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    for expected in [
        SyntaxKind::ClosureExpression,
        SyntaxKind::ClosureParameter,
        SyntaxKind::BindingPattern,
        SyntaxKind::PrimitiveType,
        SyntaxKind::ReturnType,
        SyntaxKind::BlockExpression,
        SyntaxKind::LetStatement,
        SyntaxKind::BinaryExpression,
        SyntaxKind::CallExpression,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(
        green_kind_count(built.green(), SyntaxKind::DelimitedGroup),
        1
    );
    assert!(!kinds.contains(&SyntaxKind::TupleExpression));
    assert!(!kinds.contains(&SyntaxKind::ErrorExpression));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn zero_parameter_closure_is_not_a_binary_or_expression() {
    let source = "proof ready() = || true\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert!(kinds.contains(&SyntaxKind::ClosureExpression));
    assert!(!kinds.contains(&SyntaxKind::ClosureParameter));
    assert!(!kinds.contains(&SyntaxKind::BinaryExpression));
    assert!(!kinds.contains(&SyntaxKind::ErrorExpression));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn bracket_families_and_call_argument_shapes_are_independently_typed() {
    let source = "proof containers(value: Int, count: Int, rest: [Int]) = consume([1, 2, 3], [value, count], [value; count], first = value, rest...)\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    for expected in [
        SyntaxKind::NumericBracketSequenceExpression,
        SyntaxKind::BracketSequenceExpression,
        SyntaxKind::ArrayRepeatExpression,
        SyntaxKind::CallExpression,
        SyntaxKind::CallArgument,
        SyntaxKind::NameReference,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::CallArgument)
            .count(),
        5
    );
    assert!(!kinds.contains(&SyntaxKind::ErrorExpression));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn mixed_integer_suffixes_remain_an_ordinary_bracket_sequence() {
    let source = "proof mixed() = [1u8, 2u16]\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert!(kinds.contains(&SyntaxKind::BracketSequenceExpression));
    assert!(!kinds.contains(&SyntaxKind::NumericBracketSequenceExpression));
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::LiteralExpression)
            .count(),
        2
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn record_and_named_block_families_share_typed_fields_and_blocks() {
    let source = "proof composites(value: Int) = (Point { x = value, y }, { first = value, second: value + 1 }, result { let computed = value; computed }, scope named { let local = value; local }, thread detached worker { let item = value; item })\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    for expected in [
        SyntaxKind::RecordExpression,
        SyntaxKind::RecordLiteralExpression,
        SyntaxKind::RecordField,
        SyntaxKind::ComputationBlockExpression,
        SyntaxKind::NamedBlockExpression,
        SyntaxKind::ThreadExpression,
        SyntaxKind::Block,
        SyntaxKind::LetStatement,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::RecordField)
            .count(),
        4
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::Block)
            .count(),
        3
    );
    assert!(!kinds.contains(&SyntaxKind::ErrorExpression));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn statement_shaped_braces_remain_a_block_expression() {
    let source = "proof block(value: Int) = { let local = value; local }\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert!(kinds.contains(&SyntaxKind::BlockExpression));
    assert!(kinds.contains(&SyntaxKind::LetStatement));
    assert!(!kinds.contains(&SyntaxKind::RecordLiteralExpression));
    assert!(!kinds.contains(&SyntaxKind::ErrorExpression));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
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
    let source = "{ let y: Int = x; target = y; 'line <- y; return y; out 'exit y; goto next; defer cleanup(); yield y; signal y; wait(y); on ready => lemma(y); close y; select y; break 'loop y; continue 'loop; lemma(y); y; }\n";
    let built = parse_test_statement_block(&document(source)).unwrap();
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
        SyntaxKind::ExpressionStatement,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn control_statements_own_conditions_patterns_blocks_and_match_arms() {
    let source = "{ if ready { lemma(1); } else { lemma(0); }; while ready { break; }; while let .Some(x) = next when ready { continue; }; for item in xs { lemma(item); }; loop { break 1; }; match next { .Some(x) when ready => lemma(x), .None => { return 0; } }; thread worker { yield 1; }; defer { close resource; }; unsafe lifetime @unsafe.test { lemma(1); }; }\n";
    let built = parse_test_statement_block(&document(source)).unwrap();
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
    let source = "{ let .Some(x) = value else { return; }; let picked = choice @choice.test { }; let scoped = scope named { 1 }; let repeated = loop { break 1; }; let waited = try await task(); let action = receive action(@action.ok); }\n";
    let built = parse_test_statement_block(&document(source)).unwrap();
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
    let source = "{ ???; lemma(); }\n";
    let built = parse_test_statement_block(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&SyntaxKind::ErrorStatement));
    assert!(kinds.contains(&SyntaxKind::ExpressionStatement));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn predicate_and_proof_blocks_reject_non_contract_statement_families() {
    let source = "predicate p(x: Bool) { if x { return; }; x }\nproof q() { let picked = choice @choice.test { }; while true { break; }; lemma(); }\n";
    let built = parse_shadow_document(&document(source)).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();
    assert!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::ErrorStatement)
            .count()
            >= 2
    );
    assert!(kinds.contains(&SyntaxKind::LetStatement));
    assert!(kinds.contains(&SyntaxKind::ProofCallStatement));
    assert!(!kinds.contains(&SyntaxKind::IfStatement));
    assert!(!kinds.contains(&SyntaxKind::WhileStatement));
    assert!(!kinds.contains(&SyntaxKind::LetChoiceStatement));
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
