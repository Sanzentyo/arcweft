use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_document;
use super::statement::parse_test_statement_block;
use crate::grammar::build::{GrammarBuildError, UnattachedGrammarEntry};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::incremental::SyntaxLimit;

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

fn comma_separated(count: usize, element: impl Fn(usize) -> String) -> String {
    (0..count).map(element).collect::<Vec<_>>().join(", ")
}

#[test]
fn trusted_proof_attribute_accepts_exactly_one_decoded_reason() {
    let source = "#[verify.trusted(reason = \"reviewed ✓\")]\nproof admitted() = ()\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();

    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn trusted_proof_attribute_emits_only_the_eight_owned_diagnostic_codes() {
    let rows = [
        (
            "#[verify.trusted(reason = \"x\")]\npredicate denied() = true\n",
            "syntax.proof.trusted.not_proof",
        ),
        (
            "#[verify.trusted(reason = \"first\")]\n#[verify.trusted(reason = \"second\")]\nproof duplicate() = ()\n",
            "syntax.proof.trusted.duplicate",
        ),
        (
            "#[verify.trusted]\nproof missing() = ()\n",
            "syntax.proof.trusted.reason_missing",
        ),
        (
            "#[verify.trusted(reason = \"first\", reason = \"second\")]\nproof duplicate_reason() = ()\n",
            "syntax.proof.trusted.reason_duplicate",
        ),
        (
            "#[verify.trusted(reason = 1)]\nproof not_string() = ()\n",
            "syntax.proof.trusted.reason_not_string",
        ),
        (
            "#[verify.trusted(reason = \" \\t\")]\nproof empty() = ()\n",
            "syntax.proof.trusted.reason_empty",
        ),
        (
            "#[verify.trusted(evidence = \"x\")]\nproof unknown() = ()\n",
            "syntax.proof.trusted.unknown_argument",
        ),
        (
            "#[verify.trusted(\"x\")]\nproof positional() = ()\n",
            "syntax.proof.trusted.positional_argument",
        ),
    ];

    for (source, expected) in rows {
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        let trusted_codes = built
            .diagnostics()
            .iter()
            .map(super::super::grammar::event::PendingSyntaxDiagnostic::code)
            .filter(|code| code.starts_with("syntax.proof.trusted."))
            .collect::<Vec<_>>();
        assert_eq!(trusted_codes, [expected], "{source}");
        assert_eq!(built.green().to_string(), source);
    }
}

#[test]
fn receiver_shaped_predicate_and_proof_parameters_retain_typed_recovery() {
    for keyword in ["predicate", "proof"] {
        for receiver in ["self", "mut self", "&self", "&mut self"] {
            let body = if keyword == "predicate" { "true" } else { "()" };
            let source =
                format!("{keyword} recovered({receiver}) = {body}\nproof following() = ()\n");
            let built =
                parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();
            let entries = built.index().entries();
            let patterns = entries
                .iter()
                .filter(|entry| entry.role() == SyntaxRole::ParameterPattern)
                .collect::<Vec<_>>();

            assert_eq!(patterns.len(), 1, "{keyword}({receiver})");
            assert!(
                patterns[0].pattern_projection().is_some(),
                "{keyword}({receiver}) must retain the parser-owned Pattern projection"
            );
            assert_eq!(
                entries
                    .iter()
                    .filter(|entry| {
                        entry.kind() == SyntaxKind::MissingType
                            && entry.role() == SyntaxRole::ParameterType
                    })
                    .count(),
                1,
                "{keyword}({receiver})"
            );
            assert_eq!(
                built
                    .diagnostics()
                    .iter()
                    .filter(|diagnostic| diagnostic.code() == "syntax.parameter.missing_type")
                    .count(),
                1,
                "{keyword}({receiver})"
            );
            assert_eq!(built.green().to_string(), source);
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "predicate/proof header grammar rows form one closed acceptance matrix"
)]
fn predicate_proof_complete_header_grammar_matrix() {
    let rows = [
        (
            "predicate plain(value: Int) = value > 0\n",
            SyntaxKind::PredicateItem,
            SyntaxKind::ExpressionBody,
        ),
        (
            concat!(
                "pub predicate visible<'a, T>((left, right): (T, T)) where T: Ord\n",
                "requires true\n",
                "ensures result\n",
                "{ left == right }\n",
            ),
            SyntaxKind::PredicateItem,
            SyntaxKind::PredicateBlock,
        ),
        (
            "pub(crate) proof crate_visible<T>(value: T) -> T where T: Ord requires true ensures result == value = value\n",
            SyntaxKind::ProofItem,
            SyntaxKind::ExpressionBody,
        ),
        (
            concat!(
                "pub(super) proof parent_visible<'a, T>((left, right): (T, T), cmp: Comparator<T>) -> Bool where T: Ord\n",
                "requires cmp.ready()\n",
                "ensures result\n",
                "{ left == right }\n",
            ),
            SyntaxKind::ProofItem,
            SyntaxKind::ProofBlock,
        ),
    ];

    for (source, item_kind, body_kind) in rows {
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        let kinds = built
            .index()
            .entries()
            .iter()
            .map(UnattachedGrammarEntry::kind)
            .collect::<Vec<_>>();
        assert_eq!(
            kinds.iter().filter(|kind| **kind == item_kind).count(),
            1,
            "{source}"
        );
        assert_eq!(
            kinds
                .iter()
                .filter(|kind| **kind == SyntaxKind::FixedParameterGroup)
                .count(),
            1,
            "{source}"
        );
        assert!(
            kinds.contains(&body_kind),
            "missing {body_kind:?}: {source}"
        );
        assert!(!kinds.contains(&SyntaxKind::ErrorItem), "{source}");
        assert!(
            built.diagnostics().is_empty(),
            "{source}: {:?}",
            built.diagnostics()
        );
        assert_eq!(built.green().to_string(), source);
    }

    let malformed_source = "proof staged()(value: Int) = ()\n";
    let malformed = parse_document(
        &document(malformed_source),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(
        malformed
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::FixedParameterGroup)
            .count(),
        2
    );
    assert_eq!(
        malformed
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::ErrorNode)
            .count(),
        1
    );
    assert!(
        malformed
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.proof.malformed_header" })
    );
    assert_eq!(malformed.green().to_string(), malformed_source);

    for (source, expected_code, expected_recovery_kind, expected_proofs) in [
        (
            "proof () = ()\nproof following() = ()\n",
            "syntax.declaration.missing_name",
            SyntaxKind::MissingName,
            2,
        ),
        (
            "proof broken<, T>() = ()\nproof following() = ()\n",
            "syntax.generic.missing_name",
            SyntaxKind::MissingName,
            2,
        ),
        (
            "proof broken(value: Int\nproof following() = ()\n",
            "syntax.proof.missing_parameter_close",
            SyntaxKind::CloseParenNode,
            2,
        ),
        (
            "proof broken() where T = ()\nproof following() = ()\n",
            "syntax.where.missing_colon",
            SyntaxKind::ColonNode,
            2,
        ),
        (
            "predicate broken()\nproof following() = ()\n",
            "syntax.predicate.missing_body",
            SyntaxKind::MissingBody,
            1,
        ),
    ] {
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        assert!(
            built
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == expected_code),
            "{source}: {:?}",
            built.diagnostics()
        );
        assert!(
            built
                .index()
                .entries()
                .iter()
                .any(|entry| entry.kind() == expected_recovery_kind),
            "{source}"
        );
        assert_eq!(
            built
                .index()
                .entries()
                .iter()
                .filter(|entry| entry.kind() == SyntaxKind::ProofItem)
                .count(),
            expected_proofs,
            "{source}"
        );
        if expected_recovery_kind != SyntaxKind::MissingBody {
            assert!(!built.missing_tokens().is_empty(), "{source}");
        }
        assert_eq!(built.green().to_string(), source);
    }
}

#[test]
fn canonical_multiline_contract_header_and_block_form_one_declaration() {
    let source = "pub predicate ordered<T>(pair: (T, T), cmp: Comparator<T>)\nwhere T: Ord\nrequires cmp.is_total()\nensures result\n{\n    let (left, right): (T, T) = pair\n    cmp.compare(left, right) <= 0\n}\nproof next() = ()\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
fn missing_block_close_uses_zero_width_delimiter_node() {
    let source = "proof broken() -> Int { let x = ;\nproof next() = ()\n";
    let next_start = source.find("proof next").unwrap();
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    assert_eq!(
        green_kind_count(built.green(), SyntaxKind::CloseBraceNode),
        1,
        "the recovered block must own one parser-inserted close delimiter"
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_block_close_preserves_the_following_declarations_prefixes() {
    let source = "predicate broken() { let x = true\n/// The next proof remains documented.\n#[verify]\nproof next() = ()\n";
    let next_prefix_start = source.find("/// The next").unwrap();
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built =
        parse_document(&document(with_tail), crate::parser::ParseOptions::default()).unwrap();
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

    let empty = parse_document(
        &document("proof unit() {}\n"),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
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
fn expression_events_preserve_precedence_arguments_and_bracket_select_identity() {
    let source =
        "proof p(a: Int, b: Int, c: Int, list: List<Int>) = try lemma(a + b * c, list[0].field)\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
        SyntaxKind::PostfixBracketExpression,
        SyntaxKind::SelectExpression,
        SyntaxKind::TryExpression,
        SyntaxKind::Path,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn prefix_try_uses_the_ordinary_expression_grammar_and_missing_operand_recovery() {
    let valid_source = "proof unwrap(value: Result<Int, Error>) = try value\n";
    let valid = parse_document(
        &document(valid_source),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(
        valid
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::TryExpression)
            .count(),
        1
    );
    assert_eq!(
        valid
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::PathExpression)
            .count(),
        1
    );
    assert_eq!(valid.green().to_string(), valid_source);

    let missing_source = "proof missing() = try\n";
    let missing = parse_document(
        &document(missing_source),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(
        missing
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::TryExpression)
            .count(),
        1
    );
    assert!(
        missing
            .index()
            .entries()
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::MissingExpression)
    );
    assert_eq!(missing.green().to_string(), missing_source);

    let plain_source = "proof plain(value: Result<Int, Error>) = value\n";
    let plain = parse_document(
        &document(plain_source),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    assert!(
        plain
            .index()
            .entries()
            .iter()
            .all(|entry| entry.kind() != SyntaxKind::TryExpression)
    );
}

#[test]
fn control_expressions_emit_typed_conditions_patterns_branches_and_arms() {
    let source = "proof choose(value: Option<Int>, ready: Bool) -> Int = if let .Some(x) = value when ready { x } else { match value { .Some(v) when v > 0 => v, .None => 0 } }\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
        SyntaxKind::PathType,
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
fn mixed_integer_suffixes_retain_typed_numeric_recovery() {
    let source = "proof mixed() = [1u8, 2u16]\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert!(!kinds.contains(&SyntaxKind::BracketSequenceExpression));
    assert!(kinds.contains(&SyntaxKind::NumericBracketSequenceExpression));
    let numeric = built
        .index()
        .entries()
        .iter()
        .find(|entry| entry.kind() == SyntaxKind::NumericBracketSequenceExpression)
        .and_then(UnattachedGrammarEntry::expression_projection)
        .expect("numeric sequence owns one parser-selected projection");
    assert!(matches!(
        numeric.projection(),
        crate::expressions::ExpressionProjection::NumericBracketSequence(sequence)
            if matches!(
                sequence.recovery(),
                crate::expressions::SyntaxNumericSequenceRecovery::ConflictingSuffix {
                    ordinal: 1,
                    first: crate::literal::IntSuffix::U8,
                    conflicting: crate::literal::IntSuffix::U16,
                }
            )
    ));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn record_and_named_block_families_share_typed_fields_and_blocks() {
    let source = "proof composites(value: Int) = (Point { x = value, y }, { first = value, second: value + 1 }, result { let computed = value; computed }, scope named { let local = value; local }, thread detached worker { let item = value; item })\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let source = "proof nested((head, [first, ..rest], TruckResult { score, rank: mut r, .. }, ev .Choice(value)): (&'a mut Comparator<Option<(Int, String)> | [U8]>) -> Result<Bool, Error>, .Some(left) | .None: Option<Int>) where Comparator<Option<Int>>: Callable<(Int, String)> + Send = true\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let source = "{ let y: Int = x; target = y; 'line <- y; return y; out 'exit y; goto next; defer cleanup(); yield y; signal changed <- y; wait(y); on ready => lemma(y); close y; select y; break 'loop y; continue 'loop; lemma(y); y; }\n";
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
        SyntaxKind::OnStatement,
        SyntaxKind::CloseStatement,
        SyntaxKind::SelectStatement,
        SyntaxKind::BreakStatement,
        SyntaxKind::ContinueStatement,
        SyntaxKind::ExpressionStatement,
    ] {
        assert!(kinds.contains(&expected), "missing {expected:?}: {kinds:?}");
    }
    assert!(!kinds.contains(&SyntaxKind::WaitStatement));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn on_mark_statement_uses_the_typed_mark_trigger_shape() {
    let source = "{ on mark(@.checkpoint) => lemma(); }\n";
    let built = parse_test_statement_block(&document(source)).unwrap();
    let mark = built
        .index()
        .entries()
        .iter()
        .find(|entry| {
            entry.kind() == SyntaxKind::MarkTriggerPattern && entry.role() == SyntaxRole::Condition
        })
        .expect("on mark condition remains a typed trigger node");
    let mark_path = mark.path().elements();
    assert!(built.index().entries().iter().any(|child| {
        child.kind() == SyntaxKind::EntityReferencePattern
            && child.role() == SyntaxRole::Pattern
            && child.path().elements().starts_with(mark_path)
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn required_operand_statements_retain_exact_missing_slots_and_wait_punctuation() {
    let authored = "{ return 'lease; yield @entity.value; wait(target); close resource; select choice.member; }\n";
    let built = super::statement::parse_test_flow_statement_block(&document(authored)).unwrap();
    let entries = built.index().entries();
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::LifetimePathExpression && entry.role() == SyntaxRole::Operand
    }));
    assert_eq!(
        entries
            .iter()
            .filter(|entry| {
                entry.kind() == SyntaxKind::OpenParenNode
                    && entry.role() == SyntaxRole::OpenDelimiter
            })
            .count(),
        1
    );
    assert_eq!(
        entries
            .iter()
            .filter(|entry| {
                entry.kind() == SyntaxKind::CloseParenNode
                    && entry.role() == SyntaxRole::CloseDelimiter
            })
            .count(),
        1
    );
    assert_eq!(built.green().to_string(), authored);

    let missing = "{ return; yield; wait(); close; select; }\n";
    let built = super::statement::parse_test_flow_statement_block(&document(missing)).unwrap();
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| {
                entry.kind() == SyntaxKind::MissingExpression && entry.role() == SyntaxRole::Operand
            })
            .count(),
        5
    );
    assert_eq!(built.green().to_string(), missing);

    let recovered = "{ wait target; }\n";
    let built = super::statement::parse_test_flow_statement_block(&document(recovered)).unwrap();
    let codes = built
        .diagnostics()
        .iter()
        .map(super::super::grammar::event::PendingSyntaxDiagnostic::code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&"syntax.statement.missing_wait_open"));
    assert!(codes.contains(&"syntax.statement.missing_wait_close"));
    assert_eq!(built.green().to_string(), recovered);

    let missing_close = "{ wait(target }\n";
    let built =
        super::statement::parse_test_flow_statement_block(&document(missing_close)).unwrap();
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.statement.missing_wait_close")
    );
    assert_eq!(built.green().to_string(), missing_close);
}

#[test]
fn control_statements_and_thread_expression_own_typed_children() {
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
        SyntaxKind::ExpressionStatement,
        SyntaxKind::LoopExpression,
        SyntaxKind::MatchStatement,
        SyntaxKind::MatchArm,
        SyntaxKind::ThreadExpression,
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
        SyntaxKind::LetStatement,
        SyntaxKind::LoopExpression,
        SyntaxKind::AwaitExpression,
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
fn malformed_statement_and_tail_are_poisoned_but_queryable() {
    let source = concat!(
        "proof broken() -> Int { let value: Int = ; ??? }\n",
        "proof following() = ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert!(kinds.contains(&SyntaxKind::LetStatement));
    assert!(kinds.contains(&SyntaxKind::MissingExpression));
    assert!(kinds.contains(&SyntaxKind::ErrorExpression));
    assert!(!kinds.contains(&SyntaxKind::ErrorStatement));
    assert!(!kinds.contains(&SyntaxKind::OmittedBlockTail));
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::ProofItem)
            .count(),
        2
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn predicate_and_proof_blocks_reject_non_contract_statement_families() {
    let source = "predicate p(x: Bool) { if x { return; }; x }\nproof q() { let picked = choice @choice.test { }; while true { break; }; lemma(); }\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
fn entity_style_proof_name_recovers_as_a_proof_item() {
    let source = "proof @legacy.fact() {}\nproof current() = ()\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
        [SyntaxKind::ProofItem, SyntaxKind::ProofItem]
    );
    assert!(
        built
            .index()
            .entries()
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::MissingName)
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn explicit_proof_identity_is_retained_before_the_local_name() {
    let source = "proof @proof:.hoge hoge() = ()\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::ProofItem)
            .count(),
        1
    );
    assert!(
        !entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::ErrorItem)
    );
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::DeclarationPublicId && entry.role() == SyntaxRole::PublicId
    }));
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::NameDefinition && entry.role() == SyntaxRole::Name
    }));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn malformed_header_recovery_keeps_following_declaration() {
    let missing_name = parse_document(
        &document("proof () = ()\n"),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
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
        "syntax.declaration.missing_name"
    );

    let missing_parameters = parse_document(
        &document("predicate ready = true\n"),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    assert_eq!(missing_parameters.missing_tokens().len(), 2);
    assert!(
        missing_parameters
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.predicate.missing_parameters")
    );

    let malformed_source = "proof p()() ensures true requires true = ()\nproof following() = ()\n";
    let malformed = parse_document(
        &document(malformed_source),
        crate::parser::ParseOptions::default(),
    )
    .unwrap();
    let codes = malformed
        .diagnostics()
        .iter()
        .map(crate::grammar::event::PendingSyntaxDiagnostic::code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&"syntax.proof.malformed_header"));
    assert!(codes.contains(&"syntax.contract.invalid_clause_order"));
    assert_eq!(
        malformed
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::ProofItem)
            .count(),
        2
    );
    assert_eq!(malformed.green().to_string(), malformed_source);
}

#[test]
fn requires_must_precede_ensures() {
    let source = "proof ordered() ensures true requires true = ()\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let clauses = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .filter(|kind| matches!(kind, SyntaxKind::RequiresClause | SyntaxKind::EnsuresClause))
        .collect::<Vec<_>>();

    assert_eq!(
        clauses,
        [SyntaxKind::EnsuresClause, SyntaxKind::RequiresClause]
    );
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.contract.invalid_clause_order")
            .count(),
        1
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn words_before_contract_values_use_ordinary_expression_recovery() {
    let source = concat!(
        "predicate ready(value: Bool)\n",
        "requires check value\n",
        "= value\n",
        "proof established(value: Bool)\n",
        "ensures prove value\n",
        "= ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::ErrorExpression)
            .count(),
        2
    );
    assert!(built.diagnostics().is_empty());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_contract_expression_has_the_shared_canonical_diagnostic() {
    let source = "proof missing()\nrequires\n= ()\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    assert!(
        built
            .index()
            .entries()
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::MissingExpression)
    );
    let diagnostic = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.contract.missing_expression")
        .expect("missing contract expression diagnostic");
    assert_eq!(diagnostic.range().start(), source.find("= ()").unwrap());
    assert_eq!(diagnostic.range().start(), diagnostic.range().end());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn predicate_authored_return_is_retained_as_current_typed_recovery() {
    let source = "predicate positive(x: Int) -> Bool = x > 0\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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

fn assert_still_removed_forms_preserve_following_declarations(following: &str) {
    let removed = [
        "borrow resource as view: View { view }",
        "trusted axiom @axiom.legacy",
        "invariant true",
        "calc { 1 == 1 }",
    ];

    for removed_form in removed {
        let source = format!("{removed_form}\n{following}");
        let built =
            parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();
        let item_kinds = built
            .index()
            .entries()
            .iter()
            .map(UnattachedGrammarEntry::kind)
            .filter(|kind| {
                matches!(
                    kind,
                    SyntaxKind::FunctionItem
                        | SyntaxKind::PredicateItem
                        | SyntaxKind::ProofItem
                        | SyntaxKind::ErrorItem
                )
            })
            .collect::<Vec<_>>();
        let expected = if following.starts_with("fn ") {
            SyntaxKind::FunctionItem
        } else if following.starts_with("predicate ") {
            SyntaxKind::PredicateItem
        } else {
            SyntaxKind::ProofItem
        };

        assert_eq!(item_kinds.last(), Some(&expected), "{source}");
        assert!(item_kinds.contains(&SyntaxKind::ErrorItem), "{source}");
        assert!(
            built.diagnostics().iter().all(|diagnostic| {
                matches!(
                    diagnostic.code(),
                    "syntax.parse"
                        | "syntax.item.expected_declaration"
                        | "syntax.item.unexpected_token"
                        | "syntax.statement.unexpected_token"
                )
            }),
            "{source}: {:?}",
            built.diagnostics()
        );
        assert_eq!(built.green().to_string(), source);
    }
}

#[test]
fn removed_forms_use_ordinary_current_grammar_recovery() {
    assert_still_removed_forms_preserve_following_declarations("proof next() = ()\n");

    // A later accepted identity contract deliberately retains explicit Proof IDs.
    // It is therefore current typed syntax, not a removed-form recovery case.
    let current = "proof @proof:.identified identified() = ()\nproof next() = ()\n";
    let built = parse_document(&document(current), crate::parser::ParseOptions::default()).unwrap();
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::ProofItem)
            .count(),
        2
    );
    assert!(built.index().entries().iter().any(|entry| {
        entry.kind() == SyntaxKind::DeclarationPublicId && entry.role() == SyntaxRole::PublicId
    }));
}

#[test]
fn malformed_removed_form_does_not_hide_following_current_declarations() {
    for following in [
        "fn next() {}\n",
        "predicate next() = true\n",
        "proof next() = ()\n",
    ] {
        assert_still_removed_forms_preserve_following_declarations(following);
    }
}

#[test]
fn predicate_and_proof_parameter_limits_are_inclusive() {
    for (keyword, limit) in [
        ("predicate", SyntaxLimit::PredicateParameters),
        ("proof", SyntaxLimit::ProofParameters),
    ] {
        let exact = comma_separated(limit.maximum(), |index| format!("p{index}: Bool"));
        let source = format!("{keyword} within({exact}) = true\n");
        assert_eq!(
            parse_document(&document(&source), crate::parser::ParseOptions::default())
                .expect("the exact parameter limit must build")
                .green()
                .to_string(),
            source
        );

        let over = comma_separated(limit.maximum() + 1, |index| format!("p{index}: Bool"));
        let source = format!("{keyword} over({over}) = true\n");
        assert_eq!(
            parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap_err(),
            GrammarBuildError::LimitExceeded(limit)
        );
    }
}

#[test]
fn generic_where_and_contract_limits_are_per_declaration_and_inclusive() {
    let generic_limit = SyntaxLimit::GenericParameters;
    let exact_generics = comma_separated(generic_limit.maximum(), |index| format!("T{index}"));
    let source =
        format!("proof first<{exact_generics}>() = ()\nproof second<{exact_generics}>() = ()\n");
    parse_document(&document(&source), crate::parser::ParseOptions::default())
        .expect("each declaration owns an independent exact generic budget");
    let over_generics = comma_separated(generic_limit.maximum() + 1, |index| format!("T{index}"));
    let source = format!("proof over<{over_generics}>() = ()\n");
    assert_eq!(
        parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap_err(),
        GrammarBuildError::LimitExceeded(generic_limit)
    );

    let where_limit = SyntaxLimit::WherePredicates;
    let exact_where = comma_separated(where_limit.maximum(), |index| format!("T{index}: Ord"));
    let source = format!("proof within() where {exact_where} = ()\n");
    parse_document(&document(&source), crate::parser::ParseOptions::default())
        .expect("the exact where-predicate limit must build");
    let over_where = comma_separated(where_limit.maximum() + 1, |index| format!("T{index}: Ord"));
    let source = format!("proof over() where {over_where} = ()\n");
    assert_eq!(
        parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap_err(),
        GrammarBuildError::LimitExceeded(where_limit)
    );

    let clause_limit = SyntaxLimit::ContractClauses;
    let exact_clauses = "requires true\n".repeat(clause_limit.maximum());
    let source = format!("proof within()\n{exact_clauses}= ()\n");
    parse_document(&document(&source), crate::parser::ParseOptions::default())
        .expect("the exact contract-clause limit must build");
    let over_clauses = "requires true\n".repeat(clause_limit.maximum() + 1);
    let source = format!("proof over()\n{over_clauses}= ()\n");
    assert_eq!(
        parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap_err(),
        GrammarBuildError::LimitExceeded(clause_limit)
    );
}

#[test]
fn assertion_conditions_are_independent_typed_expressions_with_an_inclusive_limit() {
    let limit = SyntaxLimit::AssertionConditions;
    let exact = comma_separated(limit.maximum(), |index| format!("condition_{index}"));
    let source = format!("proof within() {{ assert.prove({exact}) }}\n");
    let built = parse_document(&document(&source), crate::parser::ParseOptions::default())
        .expect("the exact assertion-condition limit must build");
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::PathExpression)
            .count(),
        limit.maximum()
    );
    assert_eq!(built.green().to_string(), source);

    let over = comma_separated(limit.maximum() + 1, |index| format!("condition_{index}"));
    let source = format!("proof over() {{ assert.prove({over}) }}\n");
    assert_eq!(
        parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap_err(),
        GrammarBuildError::LimitExceeded(limit)
    );
}
