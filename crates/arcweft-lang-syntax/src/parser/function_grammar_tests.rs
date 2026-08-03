use std::fmt::Write as _;

use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuildError, UnattachedGrammarEntry};
use crate::grammar::kinds::SyntaxKind;
use crate::incremental::SyntaxLimit;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/function-shadow").unwrap(),
        SourceName::path("function-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn kinds(text: &str) -> Vec<SyntaxKind> {
    parse_shadow_document(&document(text), crate::parser::ParseOptions::default())
        .unwrap()
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect()
}

#[test]
fn top_level_function_receiver_shape_requires_a_typed_pattern_annotation() {
    let source = "fn invalid(self) -> Unit { () }\n";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();
    let pattern = entries
        .iter()
        .find(|entry| entry.kind() == SyntaxKind::BindingPattern)
        .expect("receiver-shaped source retains a Binding Pattern");

    assert!(pattern.pattern_projection().is_some());
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::MissingType)
            .count(),
        1
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.parameter.missing_type")
    );
    assert_eq!(built.green().to_string(), source);
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
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
fn ordinary_function_parameters_retain_typed_fixed_default_and_rest_children() {
    let source = "fn staged(first: I64)(second: I64 = seed + 1)(tail: ...I64) -> I64 { first }\n";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let parsed_kinds = built
        .index()
        .entries()
        .iter()
        .map(UnattachedGrammarEntry::kind)
        .collect::<Vec<_>>();

    assert_eq!(
        parsed_kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::FixedParameterGroup)
            .count(),
        3
    );
    assert_eq!(
        parsed_kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::Parameter)
            .count(),
        3
    );
    assert_eq!(
        parsed_kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::EqualsNode)
            .count(),
        1
    );
    assert_eq!(
        parsed_kinds
            .iter()
            .filter(|kind| **kind == SyntaxKind::RestParameterMarker)
            .count(),
        1
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn invalid_rest_shapes_remain_lossless_typed_parameter_trees() {
    for (source, parameter_count, default_count) in [
        (
            "fn misplaced(values: ...I64, tail: I64) -> I64 { tail }\n",
            2,
            0,
        ),
        (
            "fn nonfinal(values: ...I64)(tail: I64) -> I64 { tail }\n",
            2,
            0,
        ),
        (
            "fn defaulted(values: ...I64 = fallback) -> I64 { fallback }\n",
            1,
            1,
        ),
    ] {
        let built =
            parse_shadow_document(&document(source), crate::parser::ParseOptions::default())
                .expect("invalid rest shape remains a recoverable document");
        let parsed_kinds = built
            .index()
            .entries()
            .iter()
            .map(UnattachedGrammarEntry::kind)
            .collect::<Vec<_>>();

        assert_eq!(
            parsed_kinds
                .iter()
                .filter(|kind| **kind == SyntaxKind::Parameter)
                .count(),
            parameter_count
        );
        assert_eq!(
            parsed_kinds
                .iter()
                .filter(|kind| **kind == SyntaxKind::RestParameterMarker)
                .count(),
            1
        );
        assert_eq!(
            parsed_kinds
                .iter()
                .filter(|kind| **kind == SyntaxKind::EqualsNode)
                .count(),
            default_count
        );
        assert_eq!(built.green().to_string(), source);
    }
}

fn function_with_parameters(group_lengths: &[usize]) -> String {
    let mut source = String::from("fn bounded");
    let mut source_ordinal = 0;
    for group_length in group_lengths {
        source.push('(');
        for parameter_ordinal in 0..*group_length {
            if parameter_ordinal != 0 {
                source.push_str(", ");
            }
            write!(&mut source, "p{source_ordinal}: I64")
                .expect("writing a parameter fixture to String cannot fail");
            source_ordinal += 1;
        }
        source.push(')');
    }
    source.push_str(" {}\n");
    source
}

#[test]
fn function_parameter_budget_is_inclusive_across_all_curried_groups() {
    let limit = SyntaxLimit::FixedParameters;
    let accepted = function_with_parameters(&[128, limit.maximum() - 128]);
    let built = parse_shadow_document(&document(&accepted), crate::parser::ParseOptions::default())
        .expect("the exact cross-group Function parameter limit must build");
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::Parameter)
            .count(),
        limit.maximum()
    );
    assert_eq!(built.green().to_string(), accepted);

    let rejected = function_with_parameters(&[128, limit.maximum() - 127]);
    assert_eq!(
        parse_shadow_document(&document(&rejected), crate::parser::ParseOptions::default())
            .unwrap_err(),
        GrammarBuildError::LimitExceeded(limit)
    );
    assert!(
        parse_shadow_document(
            &document("fn ready(value: I64) {}\n"),
            crate::parser::ParseOptions::default()
        )
        .is_ok(),
        "one-over rejection must leave the next parse clean"
    );
}

fn function_with_empty_groups(group_count: usize) -> String {
    format!("fn bounded{} {{}}\n", "()".repeat(group_count))
}

#[test]
fn function_empty_parameter_group_budget_is_inclusive_and_transactional() {
    let limit = SyntaxLimit::FixedParameters;
    let accepted = function_with_empty_groups(limit.maximum());
    let built = parse_shadow_document(&document(&accepted), crate::parser::ParseOptions::default())
        .expect("the exact empty Function parameter-group limit must build");
    assert_eq!(
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.kind() == SyntaxKind::FixedParameterGroup)
            .count(),
        limit.maximum()
    );
    assert_eq!(built.green().to_string(), accepted);

    let rejected = function_with_empty_groups(limit.maximum() + 1);
    assert_eq!(
        parse_shadow_document(&document(&rejected), crate::parser::ParseOptions::default())
            .unwrap_err(),
        GrammarBuildError::LimitExceeded(limit)
    );
    assert!(
        parse_shadow_document(
            &document("fn ready() {}\n"),
            crate::parser::ParseOptions::default()
        )
        .is_ok(),
        "one-over rejection must leave the next parse clean"
    );
}

#[test]
fn missing_function_body_does_not_consume_the_following_proof() {
    let source = "fn missing(value: Int) -> Int\nproof next() = ()\n";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
fn trailing_function_syntax_is_one_typed_recovery_owner() {
    let source = "fn recovered() {} trailing\n";
    let trailing_start = source.find("trailing").unwrap();
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let recoveries = built
        .index()
        .entries()
        .iter()
        .filter(|entry| entry.kind() == SyntaxKind::ErrorNode)
        .collect::<Vec<_>>();

    let [recovery] = recoveries.as_slice() else {
        panic!("ordinary Function trailing text must have one recovery owner")
    };
    assert_eq!(recovery.role(), crate::grammar::SyntaxRole::Recovery(0));
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "syntax.declaration.trailing_syntax"
            && diagnostic.range().start() == trailing_start
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn function_header_without_parameters_gets_typed_missing_group_recovery() {
    let source = "fn missing -> Int {}\n";
    let built =
        parse_shadow_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
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
        let built =
            parse_shadow_document(&document(&source), crate::parser::ParseOptions::default())
                .unwrap();
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
