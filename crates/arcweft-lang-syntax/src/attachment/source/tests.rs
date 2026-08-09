use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    AttachedSourceBackpressurePolicy, AttachedSourceBody, AttachedSourceExpression,
    AttachedSourceHandlerBody, AttachedSourceHandlerEvent, AttachedSourceId, AttachedSourceMember,
    AttachedSourceOverflowPolicy, AttachedSourcePrivacyPolicy, AttachedSourceReplayPolicy,
    AttachedSourceType,
};
use crate::attachment::node::SourceItemKind;
use crate::attachment::{
    AstNode, AttachedTypeFamily, GrammarIdentityMap, SyntaxDatabaseId, SyntaxLineageId,
    SyntaxNodeId, SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
};
use crate::grammar::kinds::SyntaxKind;
use crate::parser::{ParseOptions, parse_document};

fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcw:/source-attachment-test").unwrap(),
            SourceName::path("source-attachment-test.arcw"),
            text,
        )
        .unwrap(),
    );
    let build = parse_document(&document, ParseOptions::default()).unwrap();
    let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(191).unwrap());
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

fn source(snapshot: &Arc<SyntaxSnapshotData>) -> AstNode<SourceItemKind> {
    snapshot
        .nodes()
        .find(|node| node.kind() == SyntaxKind::SourceItem)
        .expect("Source declaration")
        .cast()
        .unwrap()
}

#[test]
fn source_attachment_preserves_header_policy_and_handler_inventory() {
    let snapshot = attach(concat!(
        "pub source @source.events: Source<Event, Error> {\n",
        "    from capture.events()\n",
        "    backpressure = bounded(capacity = 8, overflow = drop_oldest)\n",
        "    replay = hash_only\n",
        "    privacy = transient\n",
        "    on item event => yield event\n",
        "    on disconnected => reconnect()\n",
        "}\n",
    ));
    let declaration = source(&snapshot).semantics().unwrap();
    let AttachedSourceId::Authored {
        canonical_source_family,
        requires_name,
        ..
    } = declaration.id()
    else {
        panic!("authored Source ID")
    };
    assert!(*canonical_source_family);
    assert!(!requires_name);
    assert!(!declaration.source_type().has_recovery());
    let AttachedSourceBody::Braced { members, .. } = declaration.body() else {
        panic!("authored Source body")
    };
    assert_eq!(members.len(), 6);
    assert!(matches!(members[0], AttachedSourceMember::From { .. }));
    let AttachedSourceMember::Backpressure { policy, .. } = &members[1] else {
        panic!("backpressure member")
    };
    let AttachedSourceBackpressurePolicy::Bounded {
        capacity, overflow, ..
    } = policy.as_ref()
    else {
        panic!("bounded policy")
    };
    assert!(capacity.value().is_some());
    assert!(matches!(
        overflow.as_ref(),
        AttachedSourceOverflowPolicy::DropOldest(_)
    ));
    assert!(matches!(
        &members[2],
        AttachedSourceMember::Replay {
            policy: AttachedSourceReplayPolicy::HashOnly(_),
            ..
        }
    ));
    assert!(matches!(
        &members[3],
        AttachedSourceMember::Privacy {
            policy: AttachedSourcePrivacyPolicy::Transient(_),
            ..
        }
    ));
    assert!(matches!(
        &members[4],
        AttachedSourceMember::Handler {
            event: AttachedSourceHandlerEvent::Item(_),
            ..
        }
    ));
    assert!(matches!(
        &members[5],
        AttachedSourceMember::Handler {
            event: AttachedSourceHandlerEvent::Disconnected(_),
            ..
        }
    ));
    assert!(!declaration.has_recovery());
}

#[test]
fn source_missing_type_uses_the_canonical_typed_recovery_projection() {
    let snapshot = attach("source events: {}\n");
    let declaration = source(&snapshot).semantics().unwrap();
    let AttachedSourceType::Missing { syntax, node } = declaration.source_type() else {
        panic!("missing Source type must remain a typed recovery node")
    };
    assert_eq!(syntax.id(), node.id());
    assert_eq!(node.family(), AttachedTypeFamily::Recovery);
    assert!(declaration.has_recovery());
}

#[test]
fn source_attachment_keeps_duplicates_and_unsupported_contracts_as_recovery() {
    let snapshot = attach(concat!(
        "source events: Source<Event, Error> {\n",
        "    from first()\n",
        "    from second()\n",
        "    replay = future_mode\n",
        "    replay = none\n",
        "    requires ready\n",
        "    ensures finished\n",
        "}\n",
    ));
    let declaration = source(&snapshot).semantics().unwrap();
    let members = declaration.body().members();
    assert_eq!(members.len(), 6);
    assert!(matches!(
        &members[1],
        AttachedSourceMember::From {
            duplicate: true,
            ..
        }
    ));
    assert!(matches!(
        &members[2],
        AttachedSourceMember::Replay {
            policy: AttachedSourceReplayPolicy::Unknown { .. },
            ..
        }
    ));
    assert!(matches!(
        &members[3],
        AttachedSourceMember::Replay {
            duplicate: true,
            ..
        }
    ));
    assert!(matches!(
        &members[4],
        AttachedSourceMember::UnsupportedContract { .. }
    ));
    assert!(matches!(
        &members[5],
        AttachedSourceMember::UnsupportedContract { .. }
    ));
    assert!(declaration.has_recovery());
}

#[test]
fn source_contract_recovery_retains_missing_and_invalid_typed_conditions() {
    let snapshot = attach(concat!(
        "source events: Source<Event, Error> {\n",
        "    requires ready +\n",
        "    ensures\n",
        "}\n",
    ));
    let declaration = source(&snapshot).semantics().unwrap();
    let members = declaration.body().members();
    assert!(matches!(
        &members[0],
        AttachedSourceMember::UnsupportedContract {
            condition: AttachedSourceExpression::Recovered(_),
            ..
        }
    ));
    assert!(matches!(
        &members[1],
        AttachedSourceMember::UnsupportedContract {
            condition: AttachedSourceExpression::Missing(_),
            ..
        }
    ));
    assert!(declaration.has_recovery());
}

#[test]
fn source_handler_braced_body_is_statement_only_and_has_no_value_tail() {
    let snapshot = attach(concat!(
        "source events: Source<Event, Error> {\n",
        "    on end => { finish() }\n",
        "}\n",
    ));
    let declaration = source(&snapshot).semantics().unwrap();
    let [AttachedSourceMember::Handler { body, .. }] = declaration.body().members() else {
        panic!("one handler")
    };
    let AttachedSourceHandlerBody::Block {
        syntax,
        statements,
        closed,
    } = body
    else {
        panic!("statement-only block")
    };
    assert!(*closed);
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0].kind(), SyntaxKind::ExpressionStatement);
    assert!(syntax.optional_tail().unwrap().is_none());
}

#[test]
fn source_policy_missing_and_unknown_values_never_gain_valid_defaults() {
    let snapshot = attach(concat!(
        "source events: Source<Event, Error> {\n",
        "    backpressure = future_policy\n",
        "    replay =\n",
        "    privacy future_privacy\n",
        "}\n",
    ));
    let declaration = source(&snapshot).semantics().unwrap();
    let members = declaration.body().members();
    let AttachedSourceMember::Backpressure { policy, .. } = &members[0] else {
        panic!("backpressure member")
    };
    assert!(matches!(
        policy.as_ref(),
        AttachedSourceBackpressurePolicy::Unknown { .. }
    ));
    assert!(matches!(
        &members[1],
        AttachedSourceMember::Replay {
            policy: AttachedSourceReplayPolicy::Missing(_),
            ..
        }
    ));
    assert!(matches!(
        &members[2],
        AttachedSourceMember::Privacy {
            policy: AttachedSourcePrivacyPolicy::Unknown { .. },
            ..
        }
    ));
    assert!(declaration.has_recovery());
}

#[test]
fn bounded_policy_keeps_a_recovered_capacity_under_the_typed_call_owner() {
    let snapshot = attach(concat!(
        "source events: Source<Event, Error> {\n",
        "    backpressure = bounded(capacity = 8 +, overflow = drop_oldest)\n",
        "}\n",
    ));
    let declaration = source(&snapshot).semantics().unwrap();
    let [AttachedSourceMember::Backpressure { policy, .. }] = declaration.body().members() else {
        panic!("one bounded Source policy")
    };
    let AttachedSourceBackpressurePolicy::Bounded {
        expression,
        capacity,
        overflow,
        ..
    } = policy.as_ref()
    else {
        panic!("typed bounded policy")
    };
    assert!(matches!(expression, AttachedSourceExpression::Authored(_)));
    assert!(matches!(
        capacity.value(),
        Some(AttachedSourceExpression::Recovered(_))
    ));
    assert!(matches!(
        overflow.as_ref(),
        AttachedSourceOverflowPolicy::DropOldest(_)
    ));
    assert!(declaration.has_recovery());
}
