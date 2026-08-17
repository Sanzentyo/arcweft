use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_document;
use crate::grammar::build::{GrammarBuildError, UnattachedGrammarEntry};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::incremental::SyntaxLimit;

fn document(text: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("arcw:/flow-shadow").unwrap(),
        SourceName::path("flow-shadow.arcw"),
        text,
    )
    .unwrap()
}

fn kind_count(entries: &[UnattachedGrammarEntry], kind: SyntaxKind) -> usize {
    entries.iter().filter(|entry| entry.kind() == kind).count()
}

#[test]
fn flow_receiver_shape_requires_a_typed_pattern_annotation() {
    let source = "flow invalid(self) {}\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();
    let pattern = entries
        .iter()
        .find(|entry| entry.kind() == SyntaxKind::BindingPattern)
        .expect("receiver-shaped source retains a Binding Pattern");

    assert!(pattern.pattern_projection().is_some());
    assert_eq!(kind_count(entries, SyntaxKind::MissingType), 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.parameter.missing_type")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn flow_header_and_body_share_typed_declaration_descendants() {
    let source = r"/// Opens the generated route.
#[generated]
pub flow @flow.opening opening<'a, T>(state: &'a State) -> Result<T, Error>
where T: Clone + Debug
effects { asset.read, audio.play }
requires state.ready()
ensures result.is_ok()
{
    let next: T = state.current
    return next
}
";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    for expected in [
        SyntaxKind::FlowItem,
        SyntaxKind::DocBlock,
        SyntaxKind::OuterAttribute,
        SyntaxKind::Visibility,
        SyntaxKind::NameDefinition,
        SyntaxKind::GenericParameterGroup,
        SyntaxKind::LifetimeParameter,
        SyntaxKind::TypeParameter,
        SyntaxKind::FixedParameterGroup,
        SyntaxKind::ReturnType,
        SyntaxKind::WhereClause,
        SyntaxKind::RequiresClause,
        SyntaxKind::EnsuresClause,
        SyntaxKind::FlowBody,
        SyntaxKind::Block,
        SyntaxKind::LetStatement,
        SyntaxKind::ReturnStatement,
    ] {
        assert!(
            entries.iter().any(|entry| entry.kind() == expected),
            "missing {expected:?}: kinds={:?}, diagnostics={:?}",
            entries
                .iter()
                .map(UnattachedGrammarEntry::kind)
                .collect::<Vec<_>>(),
            built.diagnostics(),
        );
    }
    assert_eq!(kind_count(entries, SyntaxKind::FixedParameterGroup), 1);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn flow_contract_interleaving() {
    let source = r"flow contract_matrix(state: State)
requires state.ready
effects { asset.read }
ensures state.ok
reads { state.value }
invariant state.valid
ensures no_effect network.request
modifies { state.value }
assume external_ok
decreases state.remaining
{}
";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let contracts = built
        .index()
        .entries()
        .iter()
        .filter(|entry| entry.kind().is_contract_clause())
        .map(|entry| (entry.kind(), entry.role()))
        .collect::<Vec<_>>();

    assert_eq!(
        contracts,
        vec![
            (SyntaxKind::RequiresClause, SyntaxRole::ContractClause(0)),
            (SyntaxKind::EffectsClause, SyntaxRole::ContractClause(1)),
            (SyntaxKind::EnsuresClause, SyntaxRole::ContractClause(2)),
            (SyntaxKind::ReadsClause, SyntaxRole::ContractClause(3)),
            (SyntaxKind::InvariantClause, SyntaxRole::ContractClause(4)),
            (SyntaxKind::NoEffectClause, SyntaxRole::ContractClause(5)),
            (SyntaxKind::ModifiesClause, SyntaxRole::ContractClause(6)),
            (SyntaxKind::AssumeClause, SyntaxRole::ContractClause(7)),
            (SyntaxKind::DecreasesClause, SyntaxRole::ContractClause(8)),
        ]
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn flow_contract_modes_are_closed_tokens_not_name_references() {
    let source = concat!(
        "flow contract_modes()\n",
        "requires prove true\n",
        "ensures check false\n",
        "invariant debug true\n",
        "{}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::RequiresClause), 1);
    assert_eq!(kind_count(entries, SyntaxKind::EnsuresClause), 1);
    assert_eq!(kind_count(entries, SyntaxKind::InvariantClause), 1);
    assert_eq!(kind_count(entries, SyntaxKind::NameReference), 0);
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.role().class()
                == crate::grammar::kinds::SyntaxRoleClass::ContractOperand)
            .count(),
        3
    );
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn debug_assertion_mode_keeps_its_exact_name_owner() {
    let source = "flow checks() { assert.debug(true) }\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    let assertion = entries
        .iter()
        .find(|entry| entry.kind() == SyntaxKind::AssertionStatement)
        .expect("typed assertion statement");
    assert_eq!(
        assertion.assertion_projection().and_then(
            super::super::grammar::assertion_projection::PendingAssertionProjection::mode
        ),
        Some(crate::assertion::AssertionMode::Debug)
    );
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::NameReference && entry.role() == SyntaxRole::Name
    }));
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_flow_contract_list_does_not_consume_the_flow_body() {
    let source = "flow missing_effects()\neffects\n{}\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::EffectsClause), 1);
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::MissingExpression
            && entry.role() == SyntaxRole::ContractOperand(0)
    }));
    assert_eq!(kind_count(entries, SyntaxKind::FlowBody), 1);
    assert_eq!(kind_count(entries, SyntaxKind::Block), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingBody), 0);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.contract.missing_expression" })
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unclosed_flow_contract_list_stops_before_the_next_clause_and_body() {
    let source = concat!(
        "flow unclosed_effects()\n",
        "effects { asset.read\n",
        "requires state.ready\n",
        "{}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::EffectsClause), 1);
    assert_eq!(kind_count(entries, SyntaxKind::RequiresClause), 1);
    assert_eq!(kind_count(entries, SyntaxKind::FlowBody), 1);
    assert_eq!(kind_count(entries, SyntaxKind::Block), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingBody), 0);
    assert_eq!(built.missing_tokens().len(), 1);
    assert_eq!(
        built.missing_tokens()[0].at(),
        source.find("requires").unwrap()
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.contract.missing_list_close" })
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn every_flow_contract_list_family_uses_the_same_unclosed_list_boundary() {
    for (keyword, kind) in [
        ("reads", SyntaxKind::ReadsClause),
        ("effects", SyntaxKind::EffectsClause),
        ("modifies", SyntaxKind::ModifiesClause),
    ] {
        let source =
            format!("flow unclosed_{keyword}()\n{keyword} {{ state.value\nrequires ready\n{{}}\n");
        let built =
            parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();

        assert_eq!(kind_count(built.index().entries(), kind), 1, "{keyword}");
        assert_eq!(
            kind_count(built.index().entries(), SyntaxKind::RequiresClause),
            1,
            "{keyword}"
        );
        assert_eq!(
            kind_count(built.index().entries(), SyntaxKind::Block),
            1,
            "{keyword}"
        );
        assert_eq!(built.missing_tokens().len(), 1, "{keyword}");
        assert_eq!(
            built.missing_tokens()[0].at(),
            source.find("requires").unwrap(),
            "{keyword}"
        );
        assert_eq!(built.green().to_string(), source, "{keyword}");
    }
}

#[test]
fn unclosed_flow_contract_list_stops_before_the_flow_body() {
    let source = "flow body_boundary()\nreads { state.value\n{ return }\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();

    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::ReadsClause),
        1
    );
    assert_eq!(kind_count(built.index().entries(), SyntaxKind::FlowBody), 1);
    assert_eq!(kind_count(built.index().entries(), SyntaxKind::Block), 1);
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::ReturnStatement),
        1
    );
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::MissingBody),
        0
    );
    assert_eq!(built.missing_tokens().len(), 1);
    assert_eq!(built.missing_tokens()[0].at(), source.rfind('{').unwrap());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn contract_list_recovery_waits_for_nested_multiline_delimiters() {
    let source = concat!(
        "flow nested_list()\n",
        "effects { combine(\n",
        "    asset.read,\n",
        "    audio.play\n",
        ")\n",
        "requires ready\n",
        "{}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();

    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::CallExpression),
        1
    );
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::RequiresClause),
        1
    );
    assert_eq!(built.missing_tokens().len(), 1);
    assert_eq!(
        built.missing_tokens()[0].at(),
        source.find("requires").unwrap()
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn closed_multiline_flow_contract_list_needs_no_recovery() {
    let source = concat!(
        "flow closed_list()\n",
        "effects {\n",
        "    asset.read,\n",
        "    audio.play\n",
        "}\n",
        "requires ready\n",
        "{}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();

    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::EffectsClause),
        1
    );
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::RequiresClause),
        1
    );
    assert_eq!(kind_count(built.index().entries(), SyntaxKind::Block), 1);
    assert!(built.missing_tokens().is_empty());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_no_effect_operand_preserves_both_keywords_and_the_flow_body() {
    let source = "flow missing_no_effect()\nensures no_effect\n{}\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::NoEffectClause), 1);
    assert!(entries.iter().any(|entry| {
        entry.kind() == SyntaxKind::MissingExpression
            && entry.role() == SyntaxRole::ContractOperand(0)
    }));
    assert_eq!(kind_count(entries, SyntaxKind::FlowBody), 1);
    assert_eq!(kind_count(entries, SyntaxKind::Block), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingBody), 0);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.contract.missing_expression" })
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn flow_identity_forms_distinguish_authored_and_implicit_names() {
    let source = concat!(
        "flow opening {}\n",
        "flow @flow.other {}\n",
        "flow @flow.generated generated {}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::FlowItem), 3);
    assert_eq!(kind_count(entries, SyntaxKind::NameDefinition), 2);
    assert_eq!(kind_count(entries, SyntaxKind::MissingName), 0);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn curried_flow_group_is_recovery_and_does_not_hide_the_following_item() {
    let source = concat!(
        "flow invalid(first: Int)(second: Int) -> Int { return first }\n",
        "proof next() = ()\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::FlowItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::FixedParameterGroup), 3);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == SyntaxKind::ErrorNode)
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "flow.signature.curried_flow")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn missing_flow_identity_and_body_recover_before_the_following_item() {
    let source = "flow\nproof next() = ()\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::FlowItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingName), 1);
    assert_eq!(kind_count(entries, SyntaxKind::MissingBody), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ProofItem), 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "flow.identity.missing")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.flow.missing_body")
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn flow_parameter_default_is_typed_recovery_and_preserves_the_body() {
    let source = "flow invalid(value: Int = make_value()) { return value }\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::FlowItem), 1);
    assert_eq!(kind_count(entries, SyntaxKind::Parameter), 1);
    assert_eq!(kind_count(entries, SyntaxKind::EqualsNode), 1);
    assert_eq!(kind_count(entries, SyntaxKind::CallExpression), 1);
    assert_eq!(kind_count(entries, SyntaxKind::ReturnStatement), 1);
    assert!(built.diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == "flow.signature.parameter_default_not_admitted"
    }));
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn unclosed_second_flow_parameter_group_stops_before_contract_and_body() {
    let source = concat!(
        "flow invalid(first: Int)(second: Int\n",
        "requires first.ready\n",
        "{}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::FixedParameterGroup), 2);
    assert_eq!(kind_count(entries, SyntaxKind::RequiresClause), 1);
    assert_eq!(kind_count(entries, SyntaxKind::FlowBody), 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "flow.signature.curried_flow")
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.decl.unclosed_recovered_parameters" })
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn closed_flow_groups_do_not_treat_nested_arrows_or_default_blocks_as_boundaries() {
    let source = concat!(
        "flow first(callback: ((Int) -> Int), value: Int = make { nested() }) {}\n",
        "flow second(ok: Int)(callback: ((Int) -> Int), value: Int = make { nested() }) {}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    assert_eq!(kind_count(entries, SyntaxKind::FlowItem), 2);
    assert_eq!(kind_count(entries, SyntaxKind::FixedParameterGroup), 3);
    assert_eq!(kind_count(entries, SyntaxKind::FlowBody), 2);
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "flow.signature.curried_flow")
            .count(),
        1
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn flow_fixed_parameter_limit_is_inclusive_transactional_and_ignores_rejected_groups() {
    let limit = SyntaxLimit::FixedParameters;
    let accepted = flow_with_parameters(limit.maximum());
    let built = parse_document(&document(&accepted), crate::parser::ParseOptions::default())
        .expect("the exact Flow parameter limit must build");
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::Parameter),
        limit.maximum()
    );
    assert_eq!(built.green().to_string(), accepted);

    let rejected = flow_with_parameters(limit.maximum() + 1);
    assert_eq!(
        parse_document(&document(&rejected), crate::parser::ParseOptions::default()).unwrap_err(),
        GrammarBuildError::LimitExceeded(limit)
    );
    assert!(
        parse_document(
            &document("flow ready(value: Int) {}\n"),
            crate::parser::ParseOptions::default(),
        )
        .is_ok(),
        "one-over rejection must leave the next parse clean"
    );

    let rejected_group = format!(
        "flow recovered(ok: Int)({}) {{}}\n",
        flow_parameter_list(limit.maximum() + 1)
    );
    let recovered = parse_document(
        &document(&rejected_group),
        crate::parser::ParseOptions::default(),
    )
    .expect("a rejected second group must not consume the admitted parameter budget");
    assert_eq!(
        kind_count(recovered.index().entries(), SyntaxKind::Parameter),
        limit.maximum() + 2
    );
    assert!(
        recovered
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "flow.signature.curried_flow")
    );
}

fn flow_with_parameters(count: usize) -> String {
    format!("flow bounded({}) {{}}\n", flow_parameter_list(count))
}

fn flow_parameter_list(count: usize) -> String {
    (0..count)
        .map(|ordinal| format!("p{ordinal}: Int"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn loop_family_missing_bodies_retain_typed_heads_and_zero_width_body_recovery() {
    let source = concat!(
        "flow recovered {\n",
        "    loop\n",
        "    while\n",
        "    while let = when\n",
        "    for in\n",
        "}\n",
    );
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
    let entries = built.index().entries();

    for kind in [
        SyntaxKind::LoopExpression,
        SyntaxKind::WhileStatement,
        SyntaxKind::WhileLetStatement,
        SyntaxKind::ForStatement,
    ] {
        assert_eq!(kind_count(entries, kind), 1);
    }
    assert_eq!(
        entries
            .iter()
            .filter(|entry| {
                entry.kind() == SyntaxKind::MissingBody && entry.role() == SyntaxRole::Body
            })
            .count(),
        4
    );
    assert_eq!(kind_count(entries, SyntaxKind::MissingExpression), 4);
    assert_eq!(kind_count(entries, SyntaxKind::MissingPattern), 2);
    assert_eq!(
        built
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == "syntax.statement.missing_body")
            .count(),
        3
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn loop_family_unclosed_body_uses_current_grammar_close_recovery() {
    let source = "flow unclosed {\n    while ready {\n        return unit\n";
    let built = parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();

    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::WhileStatement),
        1
    );
    assert_eq!(
        kind_count(built.index().entries(), SyntaxKind::MissingBody),
        0
    );
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "syntax.statement.missing_block_close")
    );
    assert_eq!(built.green().to_string(), source);
}
