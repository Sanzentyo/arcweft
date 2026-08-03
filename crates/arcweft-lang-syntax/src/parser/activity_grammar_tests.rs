use std::fmt::Write;

use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuild, GrammarBuildError, UnattachedGrammarEntry};
use crate::grammar::kinds::{ActivityPolicySyntaxValue, SyntaxKind, SyntaxRole};
use crate::incremental::SyntaxLimit;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-activity").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_shadow_document(&document(source), crate::parser::ParseOptions::default())
        .expect("Activity grammar builds")
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

fn nth_source_range(source: &str, fragment: &str, occurrence: usize) -> SourceRange {
    let start = source
        .match_indices(fragment)
        .nth(occurrence)
        .map(|(start, _)| start)
        .expect("fixture occurrence");
    SourceRange::new(start, start + fragment.len())
}

#[test]
fn canonical_activity_owns_abstract_ports_policies_and_contracts() {
    let source = concat!(
        "pub activity @activity.truck TruckGame {\n",
        "    mode = deterministic\n",
        "    lifecycle = snapshot\n",
        "    input {\n",
        "        controls: Stream<InputEvent, InputError>\n",
        "        seed: u64\n",
        "    }\n",
        "    output {\n",
        "        result: TruckResult\n",
        "    }\n",
        "    contract {\n",
        "        requires seed > 0\n",
        "        ensures result.score >= 0\n",
        "    }\n",
        "}\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ActivityDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::ActivityPort), 3);
    assert_eq!(count_kind(&built, SyntaxKind::RequiresClause), 1);
    assert_eq!(count_kind(&built, SyntaxKind::EnsuresClause), 1);
    assert_eq!(count_kind(&built, SyntaxKind::EqualsNode), 2);
    assert_eq!(count_kind(&built, SyntaxKind::ColonNode), 3);
    for value in [
        ActivityPolicySyntaxValue::ModeDeterministic,
        ActivityPolicySyntaxValue::LifecycleSnapshot,
    ] {
        assert!(built.index().entries().iter().any(|entry| {
            entry.kind() == SyntaxKind::NameReference
                && entry.role() == SyntaxRole::ActivityPolicyValue(value)
        }));
    }
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn activity_declaration_member_limit_counts_recovery_entries_transactionally() {
    let exact = activity_with_unknown_entries(SyntaxLimit::DeclarationMembers.maximum());
    let built = parse_shadow_document(&document(&exact), crate::parser::ParseOptions::default())
        .expect("exact Activity declaration-member limit builds");
    assert_eq!(
        count_kind(&built, SyntaxKind::ErrorDeclarationMember),
        SyntaxLimit::DeclarationMembers.maximum()
    );

    let one_over = activity_with_unknown_entries(SyntaxLimit::DeclarationMembers.maximum() + 1);
    assert!(matches!(
        parse_shadow_document(&document(&one_over), crate::parser::ParseOptions::default()),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::DeclarationMembers
        ))
    ));
    assert!(
        parse_shadow_document(
            &document("activity Ready {}\n"),
            crate::parser::ParseOptions::default()
        )
        .is_ok()
    );
}

fn activity_with_unknown_entries(count: usize) -> String {
    let mut source = String::from("activity Many {\n");
    for ordinal in 0..count {
        writeln!(source, "    unknown_{ordinal}").expect("String writes cannot fail");
    }
    source.push_str("}\n");
    source
}

#[test]
fn activity_port_limit_is_shared_across_input_and_output_sections() {
    let exact = activity_with_ports(128, 128);
    let built = parse_shadow_document(&document(&exact), crate::parser::ParseOptions::default())
        .expect("exact combined Activity port limit builds");
    assert_eq!(
        count_kind(&built, SyntaxKind::ActivityPort),
        SyntaxLimit::ActivityPorts.maximum()
    );

    let one_over = activity_with_ports(128, 129);
    assert!(matches!(
        parse_shadow_document(&document(&one_over), crate::parser::ParseOptions::default()),
        Err(GrammarBuildError::LimitExceeded(SyntaxLimit::ActivityPorts))
    ));
    assert!(
        parse_shadow_document(
            &document("activity Retry { input { ready: Input } }\n"),
            crate::parser::ParseOptions::default()
        )
        .is_ok()
    );
}

fn activity_with_ports(inputs: usize, outputs: usize) -> String {
    let mut source = String::from("activity ManyPorts {\n    input {\n");
    for ordinal in 0..inputs {
        writeln!(source, "        input_{ordinal}: Input").expect("String writes cannot fail");
    }
    source.push_str("    }\n    output {\n");
    for ordinal in 0..outputs {
        writeln!(source, "        output_{ordinal}: Output").expect("String writes cannot fail");
    }
    source.push_str("    }\n}\n");
    source
}

#[test]
fn activity_contract_limit_is_shared_across_requires_and_ensures() {
    let exact = activity_with_contracts(32, 32);
    let built = parse_shadow_document(&document(&exact), crate::parser::ParseOptions::default())
        .expect("exact combined Activity contract limit builds");
    assert_eq!(count_kind(&built, SyntaxKind::RequiresClause), 32);
    assert_eq!(count_kind(&built, SyntaxKind::EnsuresClause), 32);

    let one_over = activity_with_contracts(32, 33);
    assert!(matches!(
        parse_shadow_document(&document(&one_over), crate::parser::ParseOptions::default()),
        Err(GrammarBuildError::LimitExceeded(
            SyntaxLimit::ContractClauses
        ))
    ));
    assert!(
        parse_shadow_document(
            &document("activity Retry { contract { requires true } }\n"),
            crate::parser::ParseOptions::default()
        )
        .is_ok()
    );
}

#[test]
fn activity_contract_recovery_does_not_create_a_clause_ordinal_gap() {
    let source = concat!(
        "activity Recovered {\n",
        "    contract {\n",
        "        requires true\n",
        "        unknown_clause\n",
        "        ensures true\n",
        "    }\n",
        "}\n",
    );
    let built = parse(source);
    let contract_entries = built
        .index()
        .entries()
        .iter()
        .filter(|entry| {
            entry.kind().is_contract_clause() || entry.kind() == SyntaxKind::ErrorDeclarationMember
        })
        .map(|entry| (entry.kind(), entry.role()))
        .collect::<Vec<_>>();

    assert_eq!(
        contract_entries,
        vec![
            (SyntaxKind::RequiresClause, SyntaxRole::ContractClause(0)),
            (SyntaxKind::ErrorDeclarationMember, SyntaxRole::Recovery(0),),
            (SyntaxKind::EnsuresClause, SyntaxRole::ContractClause(1)),
        ]
    );
    assert_eq!(built.green().to_string(), source);
}

fn activity_with_contracts(requires: usize, ensures: usize) -> String {
    let mut source = String::from("activity ManyContracts {\n    contract {\n");
    for _ in 0..requires {
        source.push_str("        requires true\n");
    }
    for _ in 0..ensures {
        source.push_str("        ensures true\n");
    }
    source.push_str("    }\n}\n");
    source
}

#[test]
fn minimal_activity_body_is_typed_and_uses_semantic_defaults_by_omission() {
    let source = "activity MiniGame {}\n";
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ActivityDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::ActivityBody), 1);
    assert_eq!(count_kind(&built, SyntaxKind::ActivityModeMember), 0);
    assert_eq!(count_kind(&built, SyntaxKind::ActivityLifecycleMember), 0);
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn activity_rejects_an_unexpected_header_and_preserves_the_next_sibling() {
    let source = concat!(
        "activity MiniGame where T: Game {\n",
        "    mode = deterministic\n",
        "}\n",
        "signal ready: Watch<bool>\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ActivityDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 1);
    assert!(
        built
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code() == "syntax.declaration.unexpected_header" })
    );
    assert_eq!(built.green().to_string(), source);
}

#[test]
fn activity_section_port_and_contract_recovery_remains_typed() {
    let source = concat!(
        "activity Broken {\n",
        "    output {\n",
        "        shared: Result\n",
        "    }\n",
        "    input {\n",
        "        shared: Input = default\n",
        "    }\n",
        "    mode = unknown\n",
        "    mode = deterministic\n",
        "    contract {\n",
        "        ensures true\n",
        "        requires true\n",
        "        check true\n",
        "    }\n",
        "}\n",
    );
    let built = parse(source);
    for code in [
        "syntax.activity.section_order",
        "syntax.activity.duplicate_port",
        "syntax.activity.port_initializer_not_allowed",
        "syntax.activity.unknown_policy",
        "syntax.activity.duplicate_member",
        "syntax.activity.contract_order",
        "syntax.activity.unknown_contract_clause",
    ] {
        assert!(
            built
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == code),
            "missing {code}: {:?}",
            built.diagnostics()
        );
    }
    assert_eq!(count_kind(&built, SyntaxKind::ActivityPort), 2);
    let duplicate_port = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.activity.duplicate_port")
        .expect("duplicate port diagnostic");
    assert_eq!(
        duplicate_port.range(),
        nth_source_range(source, "shared", 1)
    );
    assert_eq!(
        duplicate_port.related_range(),
        Some(nth_source_range(source, "shared", 0))
    );
    let duplicate_mode = built
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code() == "syntax.activity.duplicate_member")
        .expect("duplicate mode diagnostic");
    assert_eq!(duplicate_mode.range(), nth_source_range(source, "mode", 1));
    assert_eq!(
        duplicate_mode.related_range(),
        Some(nth_source_range(source, "mode", 0))
    );
    assert_eq!(built.green().to_string(), source);
}
