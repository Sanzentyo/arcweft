use super::*;

use arcweft_lang_syntax::attachment::{
    AttachedSourceBackpressurePolicy, AttachedSourceHandlerEvent, AttachedSourceId,
    AttachedSourceMember, SyntaxNodeId,
};
use arcweft_lang_syntax::incremental::SyntaxLimit;

use crate::identity::{ItemId, StmtId};
use crate::item::{
    HirSourceBackpressurePolicy, HirSourceBackpressureValue, HirSourceBody, HirSourceEventPattern,
    HirSourceExpressionValue, HirSourceHandler, HirSourceHandlerBody, HirSourceHeaders,
    HirSourceId, HirSourceItem, HirSourceOverflowPolicy, HirSourceOverflowValue,
    HirSourcePolicyBinding, HirSourcePrivacyPolicy, HirSourcePrivacyValue, HirSourceReplayPolicy,
    HirSourceReplayValue, HirSourceRequiredSlot,
};
use crate::leaf::{HirIdRef, HirIdRefInvariantError, HirIdRefIssue, HirIdRefShape, HirIdRefValue};

use super::super::source::preflight_source_members;

fn source_item(module: &HirModule, ordinal: usize) -> (ItemId, &HirItem, &HirSourceItem) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Source(source) = item.kind() else {
        panic!("source-ordered item {ordinal} must be a Source")
    };
    (owner, item, source)
}

fn required_source_body() -> &'static str {
    concat!(
        " {\n",
        "    from events()\n",
        "    backpressure = latest\n",
        "    replay = none\n",
        "    privacy = private\n",
        "}\n",
    )
}

#[derive(Default)]
struct SourceSyntaxInventory {
    from: Option<SyntaxNodeId>,
    duplicate_from: Option<SyntaxNodeId>,
    capacity: Option<SyntaxNodeId>,
    inline_expressions: Vec<SyntaxNodeId>,
    handler_owners: Vec<SyntaxNodeId>,
    contract_conditions: Vec<SyntaxNodeId>,
}

fn source_syntax_inventory(parsed: &ParsedSource) -> SourceSyntaxInventory {
    let attached = parsed
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Source(source) => Some(source.semantics().unwrap()),
            _ => None,
        })
        .expect("typed Source attachment");
    let mut inventory = SourceSyntaxInventory::default();
    let mut saw_from = false;
    for member in attached.body().members() {
        match member {
            AttachedSourceMember::From { value, .. } => {
                let id = value.syntax().id();
                if saw_from {
                    inventory.duplicate_from = Some(id);
                } else {
                    inventory.from = Some(id);
                    saw_from = true;
                }
            }
            AttachedSourceMember::Backpressure { policy, .. } => {
                inventory
                    .inline_expressions
                    .push(policy.expression().syntax().id());
                if let AttachedSourceBackpressurePolicy::Bounded {
                    capacity, overflow, ..
                } = policy.as_ref()
                {
                    inventory.capacity = capacity.value().map(|value| value.syntax().id());
                    if let Some(argument) = overflow.argument()
                        && let Some(value) = argument.value()
                    {
                        inventory.inline_expressions.push(value.syntax().id());
                    }
                }
            }
            AttachedSourceMember::Replay { policy, .. } => inventory
                .inline_expressions
                .push(policy.expression().syntax().id()),
            AttachedSourceMember::Privacy { policy, .. } => inventory
                .inline_expressions
                .push(policy.expression().syntax().id()),
            AttachedSourceMember::Handler { syntax, event, .. } => {
                inventory.handler_owners.push(syntax.id());
                match event {
                    AttachedSourceHandlerEvent::Item(pattern)
                    | AttachedSourceHandlerEvent::Error(pattern)
                    | AttachedSourceHandlerEvent::Progress(pattern) => {
                        inventory.inline_expressions.push(pattern.pattern().id());
                    }
                    AttachedSourceHandlerEvent::Disconnected(condition)
                    | AttachedSourceHandlerEvent::PermissionRevoked(condition)
                    | AttachedSourceHandlerEvent::End(condition)
                    | AttachedSourceHandlerEvent::Unknown { condition, .. } => {
                        inventory.inline_expressions.push(condition.syntax().id());
                    }
                }
            }
            AttachedSourceMember::UnsupportedContract { condition, .. } => {
                inventory.contract_conditions.push(condition.syntax().id());
            }
            AttachedSourceMember::Recovery { .. } => {}
        }
    }
    inventory
}

fn source_id_syntax(parsed: &ParsedSource) -> Option<SyntaxNodeId> {
    let attached = parsed
        .items()
        .unwrap()
        .into_iter()
        .find_map(|item| match item {
            TypedItemNode::Source(source) => Some(source.semantics().unwrap()),
            _ => None,
        })
        .expect("typed Source attachment");
    match attached.id() {
        AttachedSourceId::Absent => None,
        AttachedSourceId::Authored { syntax, .. } => Some(syntax.id()),
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the canonical Source test asserts one complete executable child and handler owner graph"
)]
fn source_lowers_only_executable_children_and_freezes_handler_owners() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-source-clean",
        concat!(
            "pub source @source.events: Source<Event, Error> {\n",
            "    from capture.events()\n",
            "    backpressure = bounded(capacity = 8, overflow = drop_oldest)\n",
            "    replay = hash_only\n",
            "    privacy = transient\n",
            "    on item event => consume(event)\n",
            "    on disconnected => reconnect()\n",
            "}\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let syntax = source_syntax_inventory(&parsed);

    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, source) = source_item(&module, 0);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert!(
        source
            .id()
            .is_some_and(HirSourceId::is_canonical_source_family)
    );
    assert!(source.name().is_none());
    assert!(matches!(
        source.body(),
        HirSourceBody::Braced { closed: true }
    ));
    assert!(module.declaration_members().arena(owner).is_none());
    assert!(item.members().is_empty());
    assert_source_backed_child(&module, source.source_type());

    let from = source.from().value().expect("required from");
    let HirSourceExpressionValue::Expression(from) = from else {
        panic!("clean Source from expression")
    };
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(syntax.from.unwrap()),
        Some(*from)
    );
    let backpressure = source
        .backpressure()
        .value()
        .expect("required backpressure")
        .value();
    let crate::item::HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Bounded {
        capacity,
        overflow,
        unexpected_arguments: false,
        recovered_call: false,
    }) = backpressure
    else {
        panic!("clean bounded policy")
    };
    let HirSourceExpressionValue::Expression(capacity) = capacity.value() else {
        panic!("authored bounded capacity")
    };
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(syntax.capacity.unwrap()),
        Some(*capacity)
    );
    assert!(matches!(
        overflow.value(),
        HirSourceOverflowValue::Resolved(HirSourceOverflowPolicy::DropOldest)
    ));
    assert!(matches!(
        source.replay().value().unwrap().value(),
        HirSourceReplayValue::Resolved(HirSourceReplayPolicy::HashOnly)
    ));
    assert!(matches!(
        source.privacy().value().unwrap().value(),
        HirSourcePrivacyValue::Resolved(HirSourcePrivacyPolicy::Transient)
    ));

    assert_eq!(source.handlers().len(), 2);
    assert!(matches!(
        source.handlers()[0].event(),
        HirSourceEventPattern::Item(crate::item::HirSourcePatternValue::Pattern(_))
    ));
    assert!(matches!(
        source.handlers()[1].event(),
        HirSourceEventPattern::Disconnected(crate::item::HirSourceChildState::Authored)
    ));
    for handler in source.handlers() {
        let scope = module
            .arenas()
            .scopes()
            .resolve(module.slots(), handler.scope())
            .unwrap();
        assert_eq!(scope.kind(), HirScopeKind::Block);
        assert_eq!(scope.parent(), Some(item.scope()));
        assert_eq!(scope.owner(), &HirScopeOwner::Item(owner));
        assert_source_backed_child(&module, handler.scope());
    }
    for syntax in syntax.handler_owners {
        assert_eq!(module.slots().prepared_source_owner::<StmtId>(syntax), None);
    }
    for syntax in syntax.inline_expressions {
        assert_eq!(module.slots().prepared_source_owner::<ExprId>(syntax), None);
    }
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the Source identity test exhausts the accepted root, marker, and typed ID matrix"
)]
fn source_identity_matrix_preserves_exact_final_hir_roots_and_marker_names() {
    enum ExpectedIdentity {
        NameOnly,
        Absolute,
        AbsoluteWithName,
        Relative { parent_depth: usize },
        FamilyRelative { parent_depth: usize },
        RelativeMarker,
        FamilyRelativeMarker,
    }

    for (case, header, expected) in [
        (
            "name-only",
            "source events: Source<Event, Error>",
            ExpectedIdentity::NameOnly,
        ),
        (
            "absolute",
            "source @source.events: Source<Event, Error>",
            ExpectedIdentity::Absolute,
        ),
        (
            "delimited-absolute",
            "source @<source.events>: Source<Event, Error>",
            ExpectedIdentity::Absolute,
        ),
        (
            "absolute-and-name",
            "pub source @source.events events: Source<Event, Error>",
            ExpectedIdentity::AbsoluteWithName,
        ),
        (
            "relative-zero",
            "source @.events: Source<Event, Error>",
            ExpectedIdentity::Relative { parent_depth: 0 },
        ),
        (
            "relative-parent",
            "source @..events: Source<Event, Error>",
            ExpectedIdentity::Relative { parent_depth: 1 },
        ),
        (
            "relative-super",
            "source @super.events: Source<Event, Error>",
            ExpectedIdentity::Relative { parent_depth: 1 },
        ),
        (
            "family-relative-zero",
            "source @source:.events: Source<Event, Error>",
            ExpectedIdentity::FamilyRelative { parent_depth: 0 },
        ),
        (
            "family-relative-parent",
            "source @source:..events: Source<Event, Error>",
            ExpectedIdentity::FamilyRelative { parent_depth: 1 },
        ),
        (
            "relative-marker",
            "source @. events: Source<Event, Error>",
            ExpectedIdentity::RelativeMarker,
        ),
        (
            "family-relative-marker",
            "source @source:. events: Source<Event, Error>",
            ExpectedIdentity::FamilyRelativeMarker,
        ),
    ] {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-source-id-{case}"),
            &format!("{header}{}", required_source_body()),
        );
        assert!(
            parsed.diagnostics().is_empty(),
            "{case}: {:?}",
            parsed.diagnostics()
        );
        let id_syntax = source_id_syntax(&parsed);
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (_, item, source) = source_item(&module, 0);
        assert_eq!(item.state(), &HirItemPoisonState::Clean, "{case}");
        if let Some(id_syntax) = id_syntax {
            assert_eq!(
                module.slots().prepared_source_owner::<ExprId>(id_syntax),
                None,
                "{case}: declaration ID must not become an expression"
            );
        }
        match expected {
            ExpectedIdentity::NameOnly => {
                assert!(source.id().is_none(), "{case}");
                assert!(matches!(
                    source.name(),
                    Some(HirRequiredName::Resolved(name)) if name.as_str() == "events"
                ));
            }
            ExpectedIdentity::Absolute | ExpectedIdentity::AbsoluteWithName => {
                let id = source.id().expect("absolute Source ID");
                assert!(id.is_canonical_source_family(), "{case}");
                assert!(!id.requires_name(), "{case}");
                assert!(!id.has_recovery(), "{case}");
                assert!(matches!(
                    id.value(),
                    HirIdRefValue::Resolved(HirIdRef::Absolute(reference))
                        if reference.as_str() == "source.events"
                ));
                assert_eq!(
                    source.name().is_some(),
                    matches!(expected, ExpectedIdentity::AbsoluteWithName),
                    "{case}"
                );
            }
            ExpectedIdentity::Relative { parent_depth } => {
                let id = source.id().expect("relative Source ID");
                assert!(id.is_canonical_source_family(), "{case}");
                assert!(!id.requires_name(), "{case}");
                assert!(!id.has_recovery(), "{case}");
                assert!(matches!(
                    id.value(),
                    HirIdRefValue::Resolved(HirIdRef::Relative(relative))
                        if relative.parent_depth() == parent_depth
                            && relative.suffix().as_str() == "events"
                ));
                assert!(source.name().is_none(), "{case}");
            }
            ExpectedIdentity::FamilyRelative { parent_depth } => {
                let id = source.id().expect("family-relative Source ID");
                assert!(id.is_canonical_source_family(), "{case}");
                assert!(!id.requires_name(), "{case}");
                assert!(!id.has_recovery(), "{case}");
                assert!(matches!(
                    id.value(),
                    HirIdRefValue::Resolved(HirIdRef::FamilyRelative(relative))
                        if relative.family().as_str() == "source"
                            && relative.relative().parent_depth() == parent_depth
                            && relative.relative().suffix().as_str() == "events"
                ));
                assert!(source.name().is_none(), "{case}");
            }
            ExpectedIdentity::RelativeMarker | ExpectedIdentity::FamilyRelativeMarker => {
                let id = source.id().expect("authored Source marker");
                assert!(id.is_canonical_source_family(), "{case}");
                assert!(id.requires_name(), "{case}");
                assert!(!id.has_recovery(), "{case}");
                let expected_shape = if matches!(expected, ExpectedIdentity::RelativeMarker) {
                    HirIdRefShape::Relative {
                        parent_depth: 0,
                        suffix_segment_count: 1,
                    }
                } else {
                    HirIdRefShape::FamilyRelative {
                        parent_depth: 0,
                        suffix_segment_count: 1,
                    }
                };
                assert!(
                    matches!(
                        id.value(),
                        HirIdRefValue::Recovered(recovery)
                            if recovery.shape() == expected_shape
                                && recovery.issue() == HirIdRefIssue::Missing
                    ),
                    "{case}: {:?}, expected {expected_shape:?}",
                    id.value()
                );
                assert!(matches!(
                    source.name(),
                    Some(HirRequiredName::Resolved(name)) if name.as_str() == "events"
                ));
            }
        }
    }
}

#[test]
fn source_identity_recovery_matrix_retains_shape_without_expression_allocation() {
    for (case, header, expected_issue, expected_shape) in [
        (
            "missing-name",
            "source : Source<Event, Error>",
            HirItemIssue::MissingName,
            None,
        ),
        (
            "missing-relative-suffix",
            "source @..: Source<Event, Error>",
            HirItemIssue::MalformedHeader,
            Some(HirIdRefShape::Relative {
                parent_depth: 1,
                suffix_segment_count: 1,
            }),
        ),
        (
            "wrong-family-marker",
            "source @flow:. events: Source<Event, Error>",
            HirItemIssue::MalformedHeader,
            Some(HirIdRefShape::FamilyRelative {
                parent_depth: 0,
                suffix_segment_count: 1,
            }),
        ),
        (
            "wrong-absolute-family",
            "source @<flow.events>: Source<Event, Error>",
            HirItemIssue::MalformedHeader,
            None,
        ),
    ] {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-source-id-recovery-{case}"),
            &format!("{header}{}", required_source_body()),
        );
        let id_syntax = source_id_syntax(&parsed);
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (_, item, source) = source_item(&module, 0);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(expected_issue),
            "{case}: {:?}",
            parsed.diagnostics()
        );
        if let Some(id_syntax) = id_syntax {
            assert_eq!(
                module.slots().prepared_source_owner::<ExprId>(id_syntax),
                None,
                "{case}: recovered declaration ID must not become an expression"
            );
        }
        match case {
            "missing-name" => {
                assert!(source.id().is_none());
                assert!(matches!(source.name(), Some(HirRequiredName::Missing)));
            }
            "wrong-absolute-family" => {
                let id = source.id().expect("wrong-family ID remains typed");
                assert!(!id.is_canonical_source_family());
                assert!(id.has_recovery());
                assert!(matches!(
                    id.value(),
                    HirIdRefValue::Resolved(HirIdRef::Absolute(reference))
                        if reference.as_str() == "flow.events"
                ));
            }
            _ => {
                let id = source.id().expect("recovered Source ID");
                assert!(!id.is_canonical_source_family());
                assert!(id.has_recovery());
                assert!(
                    matches!(
                        id.value(),
                        HirIdRefValue::Recovered(recovery)
                            if Some(recovery.shape()) == expected_shape
                                && recovery.issue() == HirIdRefIssue::Missing
                    ),
                    "{case}: {:?}, expected {expected_shape:?}",
                    id.value()
                );
            }
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the Source identity recovery test exhausts every root and ID issue family"
)]
fn source_identity_recovery_covers_every_root_and_id_issue_family() {
    for (case, header, expected_shape) in [
        (
            "relative-marker-missing-name",
            "source @.: Source<Event, Error>",
            HirIdRefShape::Relative {
                parent_depth: 0,
                suffix_segment_count: 1,
            },
        ),
        (
            "family-marker-missing-name",
            "source @source:.: Source<Event, Error>",
            HirIdRefShape::FamilyRelative {
                parent_depth: 0,
                suffix_segment_count: 1,
            },
        ),
    ] {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-source-id-{case}"),
            &format!("{header}{}", required_source_body()),
        );
        let id_syntax = source_id_syntax(&parsed).unwrap();
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (_, item, source) = source_item(&module, 0);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::MissingName),
            "{case}: {:?}",
            parsed.diagnostics()
        );
        let id = source.id().expect("canonical Source marker");
        assert!(id.is_canonical_source_family(), "{case}");
        assert!(id.requires_name(), "{case}");
        assert!(!id.has_recovery(), "{case}");
        assert!(matches!(source.name(), Some(HirRequiredName::Missing)));
        assert!(matches!(
            id.value(),
            HirIdRefValue::Recovered(recovery)
                if recovery.shape() == expected_shape
                    && recovery.issue() == HirIdRefIssue::Missing
        ));
        assert_eq!(
            module.slots().prepared_source_owner::<ExprId>(id_syntax),
            None,
            "{case}"
        );
    }

    for (case, header, expected_shape, expected_issue) in [
        (
            "absolute-missing",
            "source @<>: Source<Event, Error>",
            HirIdRefShape::Absolute { segment_count: 1 },
            HirIdRefIssue::Missing,
        ),
        (
            "invalid-family",
            "source @9:.events: Source<Event, Error>",
            HirIdRefShape::FamilyRelative {
                parent_depth: 0,
                suffix_segment_count: 1,
            },
            HirIdRefIssue::Invalid(HirIdRefInvariantError::InvalidFamily),
        ),
        (
            "absolute-invalid-segment",
            "source @source..events: Source<Event, Error>",
            HirIdRefShape::Absolute { segment_count: 3 },
            HirIdRefIssue::Invalid(HirIdRefInvariantError::InvalidSuffix),
        ),
        (
            "relative-invalid-segment",
            "source @..events..leaf: Source<Event, Error>",
            HirIdRefShape::Relative {
                parent_depth: 1,
                suffix_segment_count: 3,
            },
            HirIdRefIssue::Invalid(HirIdRefInvariantError::InvalidSuffix),
        ),
        (
            "family-relative-invalid-segment",
            "source @source:.events..leaf: Source<Event, Error>",
            HirIdRefShape::FamilyRelative {
                parent_depth: 0,
                suffix_segment_count: 3,
            },
            HirIdRefIssue::Invalid(HirIdRefInvariantError::InvalidSuffix),
        ),
    ] {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-source-id-{case}"),
            &format!("{header}{}", required_source_body()),
        );
        let id_syntax = source_id_syntax(&parsed).unwrap();
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (_, item, source) = source_item(&module, 0);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader),
            "{case}: {:?}",
            parsed.diagnostics()
        );
        let id = source.id().expect("recovered Source ID");
        assert!(!id.is_canonical_source_family(), "{case}");
        assert!(id.has_recovery(), "{case}");
        assert!(
            matches!(
                id.value(),
                HirIdRefValue::Recovered(recovery)
                    if recovery.shape() == expected_shape && recovery.issue() == expected_issue
            ),
            "{case}: {:?}",
            id.value()
        );
        assert_eq!(
            module.slots().prepared_source_owner::<ExprId>(id_syntax),
            None,
            "{case}"
        );
    }

    for (case, header, expected) in [
        (
            "wrong-family-relative",
            "source @flow:.events: Source<Event, Error>",
            "family-relative",
        ),
        (
            "unclosed-delimited-absolute",
            "source @<source.events: Source<Event, Error>",
            "absolute",
        ),
    ] {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-source-id-{case}"),
            &format!("{header}{}", required_source_body()),
        );
        let id_syntax = source_id_syntax(&parsed).unwrap();
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (_, item, source) = source_item(&module, 0);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader),
            "{case}: {:?}",
            parsed.diagnostics()
        );
        let id = source.id().expect("resolved but noncanonical Source ID");
        assert!(!id.is_canonical_source_family(), "{case}");
        assert!(id.has_recovery(), "{case}");
        match expected {
            "family-relative" => assert!(matches!(
                id.value(),
                HirIdRefValue::Resolved(HirIdRef::FamilyRelative(relative))
                    if relative.family().as_str() == "flow"
                        && relative.relative().parent_depth() == 0
                        && relative.relative().suffix().as_str() == "events"
            )),
            "absolute" => assert!(matches!(
                id.value(),
                HirIdRefValue::Resolved(HirIdRef::Absolute(reference))
                    if reference.as_str() == "source.events"
            )),
            _ => unreachable!(),
        }
        assert_eq!(
            module.slots().prepared_source_owner::<ExprId>(id_syntax),
            None,
            "{case}"
        );
    }
}

#[test]
fn source_header_and_body_recovery_remain_typed_without_fabricated_members() {
    for (case, source_text, expected_issue, missing_type, expected_body) in [
        (
            "missing-type-colon",
            format!(
                "source events Source<Event, Error>{}",
                required_source_body()
            ),
            HirItemIssue::MalformedHeader,
            false,
            HirSourceBody::Braced { closed: true },
        ),
        (
            "missing-type",
            format!("source events:{}", required_source_body()),
            HirItemIssue::MissingType,
            true,
            HirSourceBody::Braced { closed: true },
        ),
        (
            "missing-body",
            "source events: Source<Event, Error>\n".to_owned(),
            HirItemIssue::MissingBody,
            false,
            HirSourceBody::Missing,
        ),
        (
            "missing-type-and-body",
            "source events:\n".to_owned(),
            HirItemIssue::MissingType,
            true,
            HirSourceBody::Missing,
        ),
        (
            "unclosed-body",
            concat!(
                "source events: Source<Event, Error> {\n",
                "    from events()\n",
                "    backpressure = latest\n",
                "    replay = none\n",
                "    privacy = private\n",
            )
            .to_owned(),
            HirItemIssue::Recovery,
            false,
            HirSourceBody::Braced { closed: false },
        ),
    ] {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-source-header-{case}"),
            &source_text,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (owner, item, source) = source_item(&module, 0);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(expected_issue),
            "{case}: {:?}",
            parsed.diagnostics()
        );
        assert_eq!(source.body(), expected_body, "{case}");
        let source_type = module
            .arenas()
            .types()
            .resolve(module.slots(), source.source_type())
            .unwrap();
        assert_eq!(
            matches!(
                source_type.kind(),
                crate::type_ref::HirTypeKind::Recovery(_)
            ),
            missing_type,
            "{case}"
        );
        assert_source_backed_child(&module, source.source_type());
        if matches!(expected_body, HirSourceBody::Missing) {
            assert!(matches!(source.from(), HirSourceRequiredSlot::Missing));
            assert!(matches!(
                source.backpressure(),
                HirSourceRequiredSlot::Missing
            ));
            assert!(matches!(source.replay(), HirSourceRequiredSlot::Missing));
            assert!(matches!(source.privacy(), HirSourceRequiredSlot::Missing));
            assert!(source.handlers().is_empty());
        }
        assert!(module.declaration_members().arena(owner).is_none());
    }
}

#[test]
fn source_first_wins_and_contracts_never_allocate_executable_children() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-source-first-wins",
        concat!(
            "source events: Source<Event, Error> {\n",
            "    from first()\n",
            "    from second()\n",
            "    backpressure = latest\n",
            "    replay = none\n",
            "    privacy = private\n",
            "    requires ready\n",
            "    ensures finished\n",
            "}\n",
        ),
    );
    let syntax = source_syntax_inventory(&parsed);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, source) = source_item(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    assert!(source.from().is_duplicate());
    let HirSourceExpressionValue::Expression(first) = source.from().value().unwrap() else {
        panic!("first Source from must remain selected")
    };
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(syntax.from.unwrap()),
        Some(*first)
    );
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(syntax.duplicate_from.unwrap()),
        None
    );
    for condition in syntax.contract_conditions {
        assert_eq!(
            module.slots().prepared_source_owner::<ExprId>(condition),
            None
        );
    }
    assert!(module.declaration_members().arena(owner).is_none());
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn source_closed_policy_matrices_do_not_create_valid_recovery_defaults() {
    let overflows = [
        ("drop_oldest", HirSourceOverflowPolicy::DropOldest),
        ("drop_newest", HirSourceOverflowPolicy::DropNewest),
        ("error", HirSourceOverflowPolicy::Error),
        ("coalesce", HirSourceOverflowPolicy::Coalesce),
    ];
    let replays = [
        ("full", HirSourceReplayPolicy::Full),
        ("hash_only", HirSourceReplayPolicy::HashOnly),
        ("summary", HirSourceReplayPolicy::Summary),
        ("event_only", HirSourceReplayPolicy::EventOnly),
        ("none", HirSourceReplayPolicy::None),
    ];
    let privacies = [
        ("transient", HirSourcePrivacyPolicy::Transient),
        ("redacted", HirSourcePrivacyPolicy::Redacted),
        ("recordable", HirSourcePrivacyPolicy::Recordable),
        ("private", HirSourcePrivacyPolicy::Private),
    ];
    for (index, (overflow, expected_overflow)) in overflows.into_iter().enumerate() {
        let (replay, expected_replay) = replays[index % replays.len()];
        let (privacy, expected_privacy) = privacies[index % privacies.len()];
        let source = format!(
            "source events: Source<Event, Error> {{\n    from events()\n    backpressure = bounded(capacity = 8, overflow = {overflow})\n    replay = {replay}\n    privacy = {privacy}\n}}\n"
        );
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-source-policy-{index}"),
            &source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (_, item, declaration) = source_item(&module, 0);
        assert_eq!(item.state(), &HirItemPoisonState::Clean);
        let crate::item::HirSourceBackpressureValue::Resolved(
            HirSourceBackpressurePolicy::Bounded { overflow, .. },
        ) = declaration.backpressure().value().unwrap().value()
        else {
            panic!("bounded policy matrix row")
        };
        assert_eq!(
            overflow.value(),
            &HirSourceOverflowValue::Resolved(expected_overflow)
        );
        assert_eq!(
            declaration.replay().value().unwrap().value(),
            &HirSourceReplayValue::Resolved(expected_replay)
        );
        assert_eq!(
            declaration.privacy().value().unwrap().value(),
            &HirSourcePrivacyValue::Resolved(expected_privacy)
        );
    }

    let parsed = parse(
        "arcweft-test://proof/final-hir-source-policy-recovery",
        concat!(
            "source events: Source<Event, Error> {\n",
            "    from events()\n",
            "    backpressure = future_backpressure\n",
            "    replay =\n",
            "    privacy = future_privacy\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, declaration) = source_item(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    assert!(matches!(
        declaration.backpressure().value().unwrap().value(),
        crate::item::HirSourceBackpressureValue::Recovered {
            authored: Some(_),
            ..
        }
    ));
    assert!(matches!(
        declaration.replay().value().unwrap().value(),
        HirSourceReplayValue::Recovered { authored: None, .. }
    ));
    assert!(matches!(
        declaration.privacy().value().unwrap().value(),
        HirSourcePrivacyValue::Recovered {
            authored: Some(_),
            ..
        }
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the Source backpressure test exhausts the closed recovery and capacity ownership matrix"
)]
fn source_backpressure_recovery_matrix_retains_only_selected_capacity_ownership() {
    for (case, policy, clean) in [
        ("blocking", "blocking_not_allowed", true),
        ("missing-capacity", "bounded(overflow = drop_oldest)", false),
        (
            "recovered-capacity",
            "bounded(capacity = 8 +, overflow = drop_oldest)",
            false,
        ),
        (
            "duplicate-capacity",
            "bounded(capacity = 8, capacity = 9, overflow = drop_oldest)",
            false,
        ),
        ("missing-overflow", "bounded(capacity = 8)", false),
        (
            "unknown-overflow",
            "bounded(capacity = 8, overflow = future)",
            false,
        ),
        (
            "duplicate-overflow",
            "bounded(capacity = 8, overflow = drop_oldest, overflow = drop_newest)",
            false,
        ),
        (
            "unexpected-argument",
            "bounded(capacity = 8, overflow = drop_oldest, extra = 1)",
            false,
        ),
    ] {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-source-backpressure-{case}"),
            &format!(
                concat!(
                    "source events: Source<Event, Error> {{\n",
                    "    from events()\n",
                    "    backpressure = {policy}\n",
                    "    replay = none\n",
                    "    privacy = private\n",
                    "}}\n",
                ),
                policy = policy,
            ),
        );
        let syntax = source_syntax_inventory(&parsed);
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (_, item, source) = source_item(&module, 0);
        assert_eq!(
            item.state(),
            if clean {
                &HirItemPoisonState::Clean
            } else {
                &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
            },
            "{case}: {:?}",
            parsed.diagnostics()
        );
        let retained = source.backpressure().value().unwrap().value();
        if case == "blocking" {
            assert!(matches!(
                retained,
                HirSourceBackpressureValue::Resolved(
                    HirSourceBackpressurePolicy::BlockingNotAllowed
                )
            ));
        } else {
            let HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Bounded {
                capacity,
                overflow,
                unexpected_arguments,
                recovered_call: _,
            }) = retained
            else {
                panic!("{case}: bounded payload")
            };
            match case {
                "missing-capacity" => {
                    assert_eq!(capacity.value(), &HirSourceExpressionValue::Missing);
                }
                "recovered-capacity" => {
                    assert_eq!(capacity.value(), &HirSourceExpressionValue::Invalid);
                }
                "duplicate-capacity" => {
                    assert!(capacity.is_duplicate());
                    assert!(capacity.value().expression().is_some());
                }
                "missing-overflow" => assert!(matches!(
                    overflow.value(),
                    HirSourceOverflowValue::Recovered {
                        authored: None,
                        issue: crate::item::HirSourcePolicyIssue::Missing,
                    }
                )),
                "unknown-overflow" => assert!(matches!(
                    overflow.value(),
                    HirSourceOverflowValue::Recovered {
                        authored: Some(name),
                        issue: crate::item::HirSourcePolicyIssue::Unsupported,
                    } if name.as_str() == "future"
                )),
                "duplicate-overflow" => {
                    assert!(overflow.is_duplicate());
                    assert_eq!(
                        overflow.value(),
                        &HirSourceOverflowValue::Resolved(HirSourceOverflowPolicy::DropOldest)
                    );
                }
                "unexpected-argument" => assert!(*unexpected_arguments),
                _ => {}
            }
            if let Some(capacity_syntax) = syntax.capacity {
                assert_eq!(
                    module
                        .slots()
                        .prepared_source_owner::<ExprId>(capacity_syntax)
                        .is_some(),
                    capacity.value().expression().is_some(),
                    "{case}"
                );
            }
        }
        for inline in syntax.inline_expressions {
            assert_eq!(
                module.slots().prepared_source_owner::<ExprId>(inline),
                None,
                "{case}"
            );
        }
    }
}

#[test]
fn source_required_slots_remain_missing_without_fabricated_defaults() {
    let members = [
        ("from", "    from events()\n"),
        ("backpressure", "    backpressure = latest\n"),
        ("replay", "    replay = none\n"),
        ("privacy", "    privacy = private\n"),
    ];
    for missing in ["from", "backpressure", "replay", "privacy"] {
        let body = members
            .iter()
            .filter_map(|(name, source)| (*name != missing).then_some(*source))
            .collect::<String>();
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-source-missing-{missing}"),
            &format!("source events: Source<Event, Error> {{\n{body}}}\n"),
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (_, item, source) = source_item(&module, 0);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember),
            "{missing}"
        );
        assert_eq!(
            matches!(source.from(), HirSourceRequiredSlot::Missing),
            missing == "from",
            "{missing}"
        );
        assert_eq!(
            matches!(source.backpressure(), HirSourceRequiredSlot::Missing),
            missing == "backpressure",
            "{missing}"
        );
        assert_eq!(
            matches!(source.replay(), HirSourceRequiredSlot::Missing),
            missing == "replay",
            "{missing}"
        );
        assert_eq!(
            matches!(source.privacy(), HirSourceRequiredSlot::Missing),
            missing == "privacy",
            "{missing}"
        );
    }

    let parsed = parse(
        "arcweft-test://proof/final-hir-source-all-slots-missing",
        "source events: Source<Event, Error> {}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, source) = source_item(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    assert!(matches!(source.from(), HirSourceRequiredSlot::Missing));
    assert!(matches!(
        source.backpressure(),
        HirSourceRequiredSlot::Missing
    ));
    assert!(matches!(source.replay(), HirSourceRequiredSlot::Missing));
    assert!(matches!(source.privacy(), HirSourceRequiredSlot::Missing));
}

#[test]
fn source_all_singular_header_duplicates_are_first_wins() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-source-all-duplicates",
        concat!(
            "source events: Source<Event, Error> {\n",
            "    from first()\n",
            "    from second()\n",
            "    backpressure = latest\n",
            "    backpressure = blocking_not_allowed\n",
            "    replay = full\n",
            "    replay = none\n",
            "    privacy = private\n",
            "    privacy = transient\n",
            "}\n",
        ),
    );
    let syntax = source_syntax_inventory(&parsed);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, source) = source_item(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    for duplicate in [
        source.from().is_duplicate(),
        source.backpressure().is_duplicate(),
        source.replay().is_duplicate(),
        source.privacy().is_duplicate(),
    ] {
        assert!(duplicate);
    }
    assert!(matches!(
        source.backpressure().value().unwrap().value(),
        HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Latest)
    ));
    assert!(matches!(
        source.replay().value().unwrap().value(),
        HirSourceReplayValue::Resolved(HirSourceReplayPolicy::Full)
    ));
    assert!(matches!(
        source.privacy().value().unwrap().value(),
        HirSourcePrivacyValue::Resolved(HirSourcePrivacyPolicy::Private)
    ));
    assert_eq!(
        module
            .slots()
            .prepared_source_owner::<ExprId>(syntax.duplicate_from.unwrap()),
        None
    );
    for syntax in syntax.inline_expressions {
        assert_eq!(module.slots().prepared_source_owner::<ExprId>(syntax), None);
    }
}

#[test]
fn source_recovered_contract_conditions_never_become_executable_hir() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-source-contract-recovery",
        concat!(
            "source events: Source<Event, Error> {\n",
            "    from events()\n",
            "    backpressure = latest\n",
            "    replay = none\n",
            "    privacy = private\n",
            "    requires ready +\n",
            "    ensures\n",
            "}\n",
        ),
    );
    let syntax = source_syntax_inventory(&parsed);
    assert_eq!(syntax.contract_conditions.len(), 2);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, _) = source_item(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    for syntax in syntax.contract_conditions {
        assert_eq!(module.slots().prepared_source_owner::<ExprId>(syntax), None);
    }
    assert!(item.members().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());
    assert_item_owner_whole_recovery(&module, owner);
}

#[test]
fn source_handlers_preserve_closed_event_matrix_and_pattern_local_prefix() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-source-events",
        concat!(
            "source events: Source<Event, Error> {\n",
            "    from events()\n",
            "    backpressure = latest\n",
            "    replay = none\n",
            "    privacy = private\n",
            "    on item value => let inner = value\n",
            "    on error fault => consume(fault)\n",
            "    on progress amount => consume(amount)\n",
            "    on disconnected => reconnect()\n",
            "    on permission_revoked => stop()\n",
            "    on end => finish()\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, source) = source_item(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    let [
        item_handler,
        error,
        progress,
        disconnected,
        permission_revoked,
        end,
    ] = source.handlers()
    else {
        panic!("six Source event handlers")
    };
    assert!(matches!(
        item_handler.event(),
        HirSourceEventPattern::Item(_)
    ));
    assert!(matches!(error.event(), HirSourceEventPattern::Error(_)));
    assert!(matches!(
        progress.event(),
        HirSourceEventPattern::Progress(_)
    ));
    assert!(matches!(
        disconnected.event(),
        HirSourceEventPattern::Disconnected(_)
    ));
    assert!(matches!(
        permission_revoked.event(),
        HirSourceEventPattern::PermissionRevoked(_)
    ));
    assert!(matches!(end.event(), HirSourceEventPattern::End(_)));

    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), item_handler.scope())
        .unwrap();
    assert_eq!(scope.owner(), &HirScopeOwner::Item(owner));
    assert_eq!(scope.locals().len(), 2);
    let event_local = module
        .arenas()
        .locals()
        .resolve(module.slots(), scope.locals()[0])
        .unwrap();
    let statement_local = module
        .arenas()
        .locals()
        .resolve(module.slots(), scope.locals()[1])
        .unwrap();
    assert_eq!(event_local.name().as_str(), "value");
    assert_eq!(event_local.kind(), HirLocalKind::PatternBinding);
    assert_eq!(statement_local.name().as_str(), "inner");
    assert_eq!(statement_local.kind(), HirLocalKind::LetBinding);
    assert!(matches!(
        item_handler.body(),
        HirSourceHandlerBody::Statement(_)
    ));
    for handler in source.handlers() {
        if let HirSourceEventPattern::Item(crate::item::HirSourcePatternValue::Pattern(pattern))
        | HirSourceEventPattern::Error(crate::item::HirSourcePatternValue::Pattern(pattern))
        | HirSourceEventPattern::Progress(crate::item::HirSourcePatternValue::Pattern(
            pattern,
        )) = handler.event()
        {
            assert_source_backed_child(&module, *pattern);
            assert_eq!(
                module
                    .arenas()
                    .patterns()
                    .resolve(module.slots(), *pattern)
                    .unwrap()
                    .scope(),
                handler.scope()
            );
        }
        for statement in handler.body().statements() {
            assert_source_backed_child(&module, *statement);
        }
    }
}

#[test]
fn source_handler_recovery_preserves_typed_event_and_body_states() {
    for (case, handler) in [
        ("unknown", "    on future => finish()\n"),
        ("missing-pattern", "    on item => consume()\n"),
        ("missing-arrow-body", "    on progress value\n"),
        ("missing-body", "    on disconnected =>\n"),
    ] {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-source-handler-{case}"),
            &format!(
                concat!(
                    "source events: Source<Event, Error> {{\n",
                    "    from events()\n",
                    "    backpressure = latest\n",
                    "    replay = none\n",
                    "    privacy = private\n",
                    "{handler}",
                    "}}\n",
                ),
                handler = handler,
            ),
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let module = lower(&mut database, &parsed, &key);
        let (owner, item, source) = source_item(&module, 0);
        assert_eq!(
            item.state(),
            &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember),
            "{case}"
        );
        let [handler] = source.handlers() else {
            panic!("{case}: one retained handler")
        };
        let scope = module
            .arenas()
            .scopes()
            .resolve(module.slots(), handler.scope())
            .unwrap();
        assert_eq!(scope.owner(), &HirScopeOwner::Item(owner), "{case}");
        assert_source_backed_child(&module, handler.scope());
        match case {
            "unknown" => assert!(matches!(
                handler.event(),
                HirSourceEventPattern::Recovered {
                    authored: Some(name),
                    issue: crate::item::HirSourceEventIssue::Unsupported,
                    ..
                } if name.as_str() == "future"
            )),
            "missing-pattern" => assert!(matches!(
                handler.event(),
                HirSourceEventPattern::Item(crate::item::HirSourcePatternValue::Missing)
            )),
            "missing-arrow-body" => {
                assert!(matches!(
                    handler.event(),
                    HirSourceEventPattern::Progress(_)
                ));
                assert_eq!(
                    handler.arrow(),
                    crate::item::HirSourcePunctuationState::Missing
                );
                assert!(matches!(handler.body(), HirSourceHandlerBody::Missing));
            }
            "missing-body" => {
                assert!(matches!(
                    handler.event(),
                    HirSourceEventPattern::Disconnected(_)
                ));
                assert!(matches!(handler.body(), HirSourceHandlerBody::Missing));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn source_braced_handler_keeps_final_expression_as_an_ordered_statement() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-source-handler-statement-only-block",
        concat!(
            "source events: Source<Event, Error> {\n",
            "    from events()\n",
            "    backpressure = latest\n",
            "    replay = none\n",
            "    privacy = private\n",
            "    on item value => { let inner = value; consume(inner); finish(inner) }\n",
            "}\n",
        ),
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, source) = source_item(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    let [handler] = source.handlers() else {
        panic!("one braced handler")
    };
    let HirSourceHandlerBody::Block {
        statements,
        closed: true,
    } = handler.body()
    else {
        panic!("closed statement-only handler body")
    };
    assert_eq!(statements.len(), 3);
    let final_statement = module
        .arenas()
        .statements()
        .resolve(module.slots(), statements[2])
        .unwrap();
    assert!(matches!(
        final_statement.kind(),
        crate::stmt::HirStmtKind::Expression { .. }
    ));
    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), handler.scope())
        .unwrap();
    assert_eq!(scope.locals().len(), 2);
    let event_local = module
        .arenas()
        .locals()
        .resolve(module.slots(), scope.locals()[0])
        .unwrap();
    assert_eq!(event_local.name().as_str(), "value");
    assert_eq!(event_local.kind(), HirLocalKind::PatternBinding);
}

#[test]
fn source_exact_syntax_member_limit_lowers_as_one_final_hir_transaction() {
    let maximum = SyntaxLimit::DeclarationMembers.maximum();
    let mut source_text = String::from(concat!(
        "source events: Source<Event, Error> {\n",
        "    from events()\n",
        "    backpressure = latest\n",
        "    replay = none\n",
        "    privacy = private\n",
    ));
    for _ in 4..maximum {
        source_text.push_str("    on end => finish()\n");
    }
    source_text.push_str("}\n");

    let parsed = parse(
        "arcweft-test://proof/final-hir-source-exact-member-limit",
        &source_text,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, source) = source_item(&module, 0);
    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(source.handlers().len(), maximum - 4);
    assert_eq!(source.body(), HirSourceBody::Braced { closed: true });
    assert!(source.handlers().iter().all(|handler| {
        matches!(handler.event(), HirSourceEventPattern::End(_))
            && matches!(handler.body(), HirSourceHandlerBody::Statement(_))
    }));
}

#[test]
fn source_member_preflight_is_inclusive() {
    let maximum = HirLimit::Statements.maximum();
    assert!(preflight_source_members(maximum).is_ok());
    let Err(HirLowerFailure::Limit(error)) = preflight_source_members(maximum + 1) else {
        panic!("one-over Source member inventory must fail")
    };
    assert_eq!(error.limit(), HirLimit::Statements);
    assert_eq!(error.maximum(), maximum);
    assert_eq!(error.observed(), maximum + 1);
}

fn assert_source_freeze_rejects(
    case: &str,
    tamper: impl FnOnce(ItemId, &HirSourceItem) -> HirSourceItem,
) {
    assert_source_freeze_rejects_from_source(
        case,
        concat!(
            "source events: Source<Event, Error> {\n",
            "    from first()\n",
            "    backpressure = bounded(capacity = 8, overflow = drop_oldest)\n",
            "    replay = none\n",
            "    privacy = private\n",
            "    on item first => { let local = first; consume(local); finish(local) }\n",
            "    on error second => { consume(second) }\n",
            "}\n",
        ),
        tamper,
    );
}

fn assert_source_freeze_rejects_from_source(
    case: &str,
    source_text: &str,
    tamper: impl FnOnce(ItemId, &HirSourceItem) -> HirSourceItem,
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-source-freeze-{case}"),
        source_text,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{case}: {:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction.lower_parsed_source_items(&parsed).unwrap();
    let owner = transaction.source_ordered_items[0];
    let (slots, arenas) = transaction.storage_mut();
    let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
    let HirItemKind::Source(source) = original.kind() else {
        panic!("freeze fixture must lower a Source item")
    };
    let tampered = tamper(owner, source);
    let replacement = HirItem::try_new_with_state(
        owner,
        original.scope(),
        original.prefix().clone(),
        HirItemKind::Source(tampered),
        Box::new([]),
        *original.state(),
    )
    .unwrap();
    arenas
        .items()
        .revise_finalized(slots, owner, replacement)
        .unwrap_or_else(|error| panic!("{case}: {error:?}"));
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none(), "{case}");
}

fn rebuild_source(
    owner: ItemId,
    source: &HirSourceItem,
    headers: HirSourceHeaders,
    handlers: Box<[HirSourceHandler]>,
) -> HirSourceItem {
    HirSourceItem::try_new(
        owner.module(),
        source.id().cloned(),
        source.name().cloned(),
        source.source_type(),
        headers,
        handlers,
        source.body(),
    )
    .unwrap()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the Source freeze test exhausts policy, event, scope, pattern, and statement substitution"
)]
fn source_freeze_rejects_policy_event_scope_pattern_and_statement_substitution() {
    assert_source_freeze_rejects("replay-policy", |owner, source| {
        let replay = source.replay().value().expect("authored replay");
        let headers = HirSourceHeaders::new(
            source.from().clone(),
            source.backpressure().clone(),
            HirSourceRequiredSlot::authored(
                HirSourcePolicyBinding::new(
                    replay.assignment(),
                    HirSourceReplayValue::Resolved(HirSourceReplayPolicy::Full),
                ),
                source.replay().is_duplicate(),
            ),
            source.privacy().clone(),
        );
        rebuild_source(owner, source, headers, source.handlers().into())
    });

    assert_source_freeze_rejects("event-kind", |owner, source| {
        let mut handlers = source.handlers().to_vec();
        let first = &handlers[0];
        let HirSourceEventPattern::Item(pattern) = first.event() else {
            panic!("first freeze handler event")
        };
        handlers[0] = HirSourceHandler::new(
            HirSourceEventPattern::Progress(*pattern),
            first.arrow(),
            first.scope(),
            first.body().clone(),
        );
        rebuild_source(
            owner,
            source,
            source.headers().clone(),
            handlers.into_boxed_slice(),
        )
    });

    assert_source_freeze_rejects("handler-scope", |owner, source| {
        let mut handlers = source.handlers().to_vec();
        let first = handlers[0].clone();
        let second = handlers[1].clone();
        handlers[0] = HirSourceHandler::new(
            first.event().clone(),
            first.arrow(),
            second.scope(),
            first.body().clone(),
        );
        rebuild_source(
            owner,
            source,
            source.headers().clone(),
            handlers.into_boxed_slice(),
        )
    });

    assert_source_freeze_rejects("pattern-id", |owner, source| {
        let mut handlers = source.handlers().to_vec();
        let first = handlers[0].clone();
        let second = handlers[1].clone();
        let HirSourceEventPattern::Item(first_pattern) = first.event() else {
            panic!("first freeze pattern")
        };
        let HirSourceEventPattern::Error(second_pattern) = second.event() else {
            panic!("second freeze pattern")
        };
        handlers[0] = HirSourceHandler::new(
            HirSourceEventPattern::Item(*second_pattern),
            first.arrow(),
            first.scope(),
            first.body().clone(),
        );
        handlers[1] = HirSourceHandler::new(
            HirSourceEventPattern::Error(*first_pattern),
            second.arrow(),
            second.scope(),
            second.body().clone(),
        );
        rebuild_source(
            owner,
            source,
            source.headers().clone(),
            handlers.into_boxed_slice(),
        )
    });

    assert_source_freeze_rejects("statement-order", |owner, source| {
        let mut handlers = source.handlers().to_vec();
        let first = handlers[0].clone();
        let HirSourceHandlerBody::Block { statements, closed } = first.body() else {
            panic!("first freeze handler block")
        };
        let mut reordered = statements.to_vec();
        reordered.swap(0, 1);
        handlers[0] = HirSourceHandler::new(
            first.event().clone(),
            first.arrow(),
            first.scope(),
            HirSourceHandlerBody::Block {
                statements: reordered.into_boxed_slice(),
                closed: *closed,
            },
        );
        rebuild_source(
            owner,
            source,
            source.headers().clone(),
            handlers.into_boxed_slice(),
        )
    });
}

#[test]
fn source_freeze_rejects_selected_expression_and_duplicate_aggregate_substitution() {
    assert_source_freeze_rejects("selected-expressions", |owner, source| {
        let from = source.from().value().unwrap().expression().unwrap();
        let backpressure = source.backpressure().value().unwrap();
        let HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Bounded {
            capacity,
            overflow,
            unexpected_arguments,
            recovered_call,
        }) = backpressure.value()
        else {
            panic!("freeze bounded backpressure")
        };
        let capacity_expression = capacity.value().expression().unwrap();
        let swapped_backpressure = HirSourcePolicyBinding::new(
            backpressure.assignment(),
            HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Bounded {
                capacity: crate::item::HirSourceBoundedArgument::new(
                    HirSourceExpressionValue::Expression(from),
                    capacity.is_duplicate(),
                ),
                overflow: overflow.clone(),
                unexpected_arguments: *unexpected_arguments,
                recovered_call: *recovered_call,
            }),
        );
        let headers = HirSourceHeaders::new(
            HirSourceRequiredSlot::authored(
                HirSourceExpressionValue::Expression(capacity_expression),
                source.from().is_duplicate(),
            ),
            HirSourceRequiredSlot::authored(
                swapped_backpressure,
                source.backpressure().is_duplicate(),
            ),
            source.replay().clone(),
            source.privacy().clone(),
        );
        rebuild_source(owner, source, headers, source.handlers().into())
    });
}

#[test]
fn source_freeze_rejects_a_false_duplicate_aggregate_under_a_poisoned_slot() {
    assert_source_freeze_rejects_from_source(
        "duplicate-aggregate",
        concat!(
            "source events: Source<Event, Error> {\n",
            "    from first()\n",
            "    from second()\n",
            "    backpressure = latest\n",
            "    replay = none\n",
            "    privacy = private\n",
            "}\n",
        ),
        |owner, source| {
            assert!(source.from().is_duplicate());
            let headers = HirSourceHeaders::new(
                HirSourceRequiredSlot::authored(*source.from().value().unwrap(), false),
                source.backpressure().clone(),
                source.replay().clone(),
                source.privacy().clone(),
            );
            rebuild_source(owner, source, headers, source.handlers().into())
        },
    );
}
