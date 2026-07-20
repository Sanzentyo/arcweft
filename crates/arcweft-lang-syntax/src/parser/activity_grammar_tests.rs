use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::document::parse_shadow_document;
use crate::grammar::build::{GrammarBuild, UnattachedGrammarEntry};
use crate::grammar::kinds::SyntaxKind;

fn document(source: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new("memory:retained-activity").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap()
}

fn parse(source: &str) -> GrammarBuild {
    parse_shadow_document(&document(source)).expect("Activity grammar builds")
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
    assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
    assert_eq!(built.green().to_string(), source);
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
fn activity_rejects_concrete_origins_and_preserves_the_next_sibling() {
    let source = concat!(
        "activity MiniGame from rust \"truck\" {\n",
        "    mode = deterministic\n",
        "}\n",
        "signal ready: Watch<bool>\n",
    );
    let built = parse(source);
    assert_eq!(count_kind(&built, SyntaxKind::ActivityDeclarationItem), 1);
    assert_eq!(count_kind(&built, SyntaxKind::SignalDeclarationItem), 1);
    assert!(
        built.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == "syntax.activity.concrete_origin_not_allowed"
        })
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
        "syntax.activity.out_of_order_member",
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
    assert_eq!(built.green().to_string(), source);
}
