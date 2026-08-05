use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use super::{
    ActivityDeclarationItemKind, AstNode, AttachedActivityBody, AttachedActivityContractBody,
    AttachedActivityContractCondition, AttachedActivityContractEntry, AttachedActivityEntry,
    AttachedActivityLifecycle, AttachedActivityMode,
};
use crate::attachment::{
    GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId, SyntaxNodeId, SyntaxSnapshotData,
    SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::kinds::SyntaxKind;
use crate::parser::{ParseOptions, parse_shadow_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/activity-attachment-test").unwrap(),
            SourceName::path("activity-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_shadow_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(137).unwrap());
    let lineage = SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(1).unwrap());
    let snapshot = SyntaxSnapshotId::new(
        lineage,
        SourceSnapshotId::initial(document.display_name().clone()),
    );
    let identities = build
        .index()
        .entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            (
                entry.path().clone(),
                SyntaxNodeId::new(
                    lineage,
                    NonZeroU64::new(u64::try_from(index).unwrap() + 1).unwrap(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    attach_typed_tree(
        &build,
        &GrammarIdentityMap::new(identities),
        snapshot,
        document,
    )
    .unwrap()
}

fn activities(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<ActivityDeclarationItemKind>> {
    snapshot
        .nodes()
        .filter(|node| node.kind() == SyntaxKind::ActivityDeclarationItem)
        .map(|node| node.cast().unwrap())
        .collect()
}

#[test]
fn activity_attachment_owns_closed_policies_ports_and_contract_order() {
    let source = concat!(
        "/// Abstract game boundary\n",
        "pub activity TruckGame {\n",
        "    mode = checkpointed_realtime\n",
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
    let snapshot = attach(source);
    let declaration = activities(&snapshot)[0].semantics().unwrap();
    assert_eq!(
        declaration.prefix().documentation().unwrap().markdown(),
        "Abstract game boundary"
    );
    assert!(declaration.declaration_recoveries().is_empty());
    let entries = declaration.body().entries();
    assert_eq!(entries.len(), 5);

    let AttachedActivityEntry::Mode(mode) = &entries[0] else {
        panic!("mode entry");
    };
    assert!(matches!(
        mode.value(),
        AttachedActivityMode::CheckpointedRealtime(_)
    ));
    assert!(!mode.assignment().is_missing());
    assert!(!mode.state().has_recovery());

    let AttachedActivityEntry::Lifecycle(lifecycle) = &entries[1] else {
        panic!("lifecycle entry");
    };
    assert!(matches!(
        lifecycle.value(),
        AttachedActivityLifecycle::Snapshot(_)
    ));

    let AttachedActivityEntry::Input(input) = &entries[2] else {
        panic!("input entry");
    };
    assert_eq!(input.body().ports().len(), 2);
    assert!(input.body().ports().iter().all(|port| {
        !port.has_recovery() && !port.colon().is_missing() && port.name().value().is_some()
    }));

    let AttachedActivityEntry::Output(output) = &entries[3] else {
        panic!("output entry");
    };
    assert_eq!(output.body().ports().len(), 1);

    let AttachedActivityEntry::Contract(contract) = &entries[4] else {
        panic!("contract entry");
    };
    assert_eq!(contract.body().entries().len(), 2);
    assert!(contract.body().entries().iter().all(|entry| matches!(
        entry,
        AttachedActivityContractEntry::Clause(clause)
            if matches!(clause.condition(), AttachedActivityContractCondition::Authored(_))
                && !clause.is_out_of_order()
    )));
    assert_eq!(
        declaration.requires_scope_source_span().range(),
        SourceRange::new(
            source.find("requires").unwrap(),
            source.find("requires").unwrap()
        )
    );
    assert_eq!(
        declaration.ensures_scope_source_span().range(),
        SourceRange::new(
            source.find("ensures").unwrap(),
            source.find("ensures").unwrap()
        )
    );
}

#[test]
fn activity_attachment_exposes_missing_invalid_duplicate_and_order_recovery() {
    let snapshot = attach(concat!(
        "activity Broken {\n",
        "    output {\n",
        "        shared: Result\n",
        "    }\n",
        "    input {\n",
        "        shared: Input = default\n",
        "        unnamed\n",
        "    }\n",
        "    mode = unknown\n",
        "    mode\n",
        "    lifecycle =\n",
        "    contract {\n",
        "        ensures true\n",
        "        requires\n",
        "        check true\n",
        "    }\n",
        "    contract\n",
        "}\n",
        "activity Missing\n",
    ));
    let declarations = activities(&snapshot)
        .iter()
        .map(AstNode::<ActivityDeclarationItemKind>::semantics)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let entries = declarations[0].body().entries();
    assert_eq!(entries.len(), 7);

    let AttachedActivityEntry::Output(output) = &entries[0] else {
        panic!("output entry");
    };
    assert!(!output.state().has_recovery());
    let AttachedActivityEntry::Input(input) = &entries[1] else {
        panic!("input entry");
    };
    assert!(input.state().is_out_of_order());
    let ports = input.body().ports();
    assert!(ports[0].is_duplicate());
    assert!(ports[0].initializer_recovery().is_some());
    assert!(ports[1].colon().is_missing());

    let AttachedActivityEntry::Mode(invalid_mode) = &entries[2] else {
        panic!("invalid mode entry");
    };
    assert!(matches!(
        invalid_mode.value(),
        AttachedActivityMode::Invalid(_)
    ));
    let AttachedActivityEntry::Mode(missing_mode) = &entries[3] else {
        panic!("missing mode entry");
    };
    assert!(missing_mode.state().is_duplicate());
    assert!(missing_mode.assignment().is_missing());
    assert!(matches!(
        missing_mode.value(),
        AttachedActivityMode::Missing(_)
    ));

    let AttachedActivityEntry::Lifecycle(lifecycle) = &entries[4] else {
        panic!("lifecycle entry");
    };
    assert!(matches!(
        lifecycle.value(),
        AttachedActivityLifecycle::Missing(_)
    ));

    let AttachedActivityEntry::Contract(contract) = &entries[5] else {
        panic!("contract entry");
    };
    let contract_entries = contract.body().entries();
    assert_eq!(contract_entries.len(), 3);
    assert!(matches!(
        &contract_entries[1],
        AttachedActivityContractEntry::Clause(clause)
            if clause.is_out_of_order()
                && matches!(clause.condition(), AttachedActivityContractCondition::Missing(condition)
                    if condition.syntax().kind() == SyntaxKind::MissingExpression
                        && matches!(condition.projection(), crate::expressions::ExpressionProjection::Error)
                        && condition.whole_source_span().range().is_empty())
    ));
    assert!(matches!(
        contract_entries[2],
        AttachedActivityContractEntry::Recovery { .. }
    ));

    let AttachedActivityEntry::Contract(missing_contract) = &entries[6] else {
        panic!("missing contract entry");
    };
    assert!(missing_contract.state().is_duplicate());
    assert!(matches!(
        missing_contract.body(),
        AttachedActivityContractBody::Missing(_)
    ));
    assert!(matches!(
        declarations[1].body(),
        AttachedActivityBody::Missing(_)
    ));
}

#[test]
fn activity_attachment_keeps_header_and_trailing_recovery_in_source_order() {
    let snapshot = attach("activity Broken where T: Game {} trailing\n");
    let declaration = activities(&snapshot)[0].semantics().unwrap();
    let recoveries = declaration.declaration_recoveries();
    assert_eq!(recoveries.len(), 2);
    assert_eq!(recoveries[0].source_text(), "where T: Game ");
    assert_eq!(recoveries[1].source_text(), "trailing\n");
    assert_eq!(
        declaration
            .unexpected_header_recovery()
            .expect("unexpected Activity header")
            .source_text(),
        "where T: Game "
    );
    assert_eq!(
        declaration
            .trailing_recovery()
            .expect("trailing Activity recovery")
            .source_text(),
        "trailing\n"
    );
}
