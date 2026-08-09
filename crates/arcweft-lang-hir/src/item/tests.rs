use core::num::{NonZeroU32, NonZeroU64};

use arcweft_id::{DeclarationIdentityFamily, DeclarationName, PublicId};

use super::callable::{HirCallableSignature, HirContractScopes, HirWherePredicate};
use super::member_index::HirDeclarationMemberIndexResolveError;
use super::retained::{
    HirActivityPortMember, HirCharacterAssignmentState, HirCharacterDisplayNameMember,
    HirCharacterMemberRecovery, HirCharacterSurfaceAlias, HirDeclarationMember,
    HirDeclarationMemberIssue, HirDeclarationMemberKind, HirDeclarationMemberPoisonState,
    HirPublicIdOrigin, HirRetainedHeaderError, HirRetainedName, HirRetainedPublicId,
    HirRetainedPublicIdIssue, HirViewExportMember,
};
use super::*;
use crate::expr::{
    HirThreadBody, HirThreadBodyInvariantError, HirThreadBodyOwner, HirThreadFlowItem,
};
use crate::identity::{HirDatabaseId, HirIdKind, HirTypedId, LocalId, RawHirId, ScopeId};
use crate::leaf::{
    HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue, HirPathIssue, HirPathRecovery,
    HirPathRoot, HirPathSegment, HirPathValue,
};

fn module(database: u64, slot: u32) -> HirModuleId {
    HirModuleId::new(
        HirDatabaseId::from_raw_for_test(NonZeroU64::new(database).unwrap()),
        NonZeroU32::new(slot).unwrap(),
    )
}

fn typed_id<I: HirTypedId>(module: HirModuleId, slot: u32) -> I {
    I::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).unwrap(),
        I::KIND,
    ))
}

fn name(value: &str) -> HirName {
    HirName::try_new(value.into()).unwrap()
}

fn path(value: &str) -> HirPath {
    HirPath::try_new(
        HirPathRoot::ImplicitCrate,
        Box::new([HirPathSegment::Identifier(name(value))]),
    )
    .unwrap()
}

fn empty_prefix() -> HirItemPrefix {
    HirItemPrefix::new(None, Box::new([]), None)
}

#[test]
fn where_predicate_requires_at_least_one_typed_bound() {
    let local = module(1, 1);
    let subject = typed_id::<TypeId>(local, 1);
    assert_eq!(
        HirWherePredicate::try_new(subject, Box::new([])),
        Err(HirItemInvariantError::EmptyWhereBounds)
    );

    let bound = typed_id::<TypeId>(local, 2);
    let predicate = HirWherePredicate::try_new(subject, Box::new([bound])).unwrap();
    assert_eq!(predicate.subject(), subject);
    assert_eq!(predicate.bounds(), [bound]);
}

#[test]
fn source_item_inventory_is_exactly_the_26_attached_families() {
    assert_eq!(
        HirItemFamily::ALL,
        [
            HirItemFamily::Module,
            HirItemFamily::Use,
            HirItemFamily::Flow,
            HirItemFamily::Function,
            HirItemFamily::Predicate,
            HirItemFamily::Proof,
            HirItemFamily::Trait,
            HirItemFamily::Impl,
            HirItemFamily::Enum,
            HirItemFamily::Struct,
            HirItemFamily::TypeAlias,
            HirItemFamily::Resource,
            HirItemFamily::Character,
            HirItemFamily::View,
            HirItemFamily::Action,
            HirItemFamily::Activity,
            HirItemFamily::Signal,
            HirItemFamily::Metric,
            HirItemFamily::Layer,
            HirItemFamily::Entry,
            HirItemFamily::ExternCapability,
            HirItemFamily::Test,
            HirItemFamily::Bench,
            HirItemFamily::Source,
            HirItemFamily::Style,
            HirItemFamily::Error,
        ]
    );
    assert_eq!(HirItemFamily::ALL.len(), 26);
    assert!(HirItemFamily::ALL.contains(&HirItemFamily::Error));
}

#[test]
fn declaration_member_identity_is_item_plus_zero_based_ordinal() {
    let module = module(1, 1);
    let item = typed_id::<ItemId>(module, 1);
    let first = HirDeclarationMemberId::new(item, 0);
    let second = HirDeclarationMemberId::new(item, 1);

    assert_eq!(first.item(), item);
    assert_eq!(first.ordinal(), 0);
    assert_eq!(second.ordinal(), 1);
    assert_eq!(first.module(), module);
    assert_eq!(first.item().kind(), HirIdKind::Item);
}

#[test]
fn member_arena_requires_owner_contiguous_source_order() {
    let module = module(2, 1);
    let owner = typed_id::<ItemId>(module, 1);
    let first_id = HirDeclarationMemberId::new(owner, 0);
    let second_id = HirDeclarationMemberId::new(owner, 1);
    let first = HirDeclarationMember::try_new(
        first_id,
        HirDeclarationMemberKind::ViewExport(HirViewExportMember::new(
            HirPathValue::Resolved(path("panel")),
            HirPathValue::Resolved(path("dialogue_panel")),
        )),
        HirDeclarationMemberPoisonState::Clean,
    )
    .unwrap();
    let second = HirDeclarationMember::try_new(
        second_id,
        HirDeclarationMemberKind::ViewExport(HirViewExportMember::new(
            HirPathValue::Resolved(path("text")),
            HirPathValue::Resolved(path("dialogue_text")),
        )),
        HirDeclarationMemberPoisonState::Clean,
    )
    .unwrap();
    let arena = HirDeclarationMemberArena::try_new(
        owner,
        HirItemFamily::View,
        Box::new([first.clone(), second.clone()]),
    )
    .unwrap();

    assert_eq!(arena.members().len(), 2);
    assert_eq!(arena.resolve(second_id).unwrap(), &second);
    assert_eq!(
        HirDeclarationMemberArena::try_new(owner, HirItemFamily::View, Box::new([second, first]),),
        Err(HirItemInvariantError::NonContiguousMember {
            expected: first_id,
            actual: second_id,
        })
    );
}

#[test]
fn member_arena_rejects_foreign_children_and_wrong_family() {
    let local = module(3, 1);
    let foreign = module(4, 1);
    let owner = typed_id::<ItemId>(local, 1);
    let member_id = HirDeclarationMemberId::new(owner, 0);
    let foreign_type = typed_id::<TypeId>(foreign, 1);
    let foreign_local = typed_id::<LocalId>(foreign, 2);

    assert_eq!(
        HirDeclarationMember::try_new(
            member_id,
            HirDeclarationMemberKind::ActivityInput(
                HirActivityPortMember::try_new(
                    HirRequiredName::Resolved(name("route_seed")),
                    foreign_type,
                    Some(foreign_local),
                )
                .unwrap(),
            ),
            HirDeclarationMemberPoisonState::Clean,
        ),
        Err(HirItemInvariantError::ForeignChild {
            expected: local,
            actual: foreign,
        })
    );

    let member = HirDeclarationMember::try_new(
        member_id,
        HirDeclarationMemberKind::CharacterDisplayName(HirCharacterDisplayNameMember::new(
            HirCharacterAssignmentState::Present,
            Some(typed_id(local, 2)),
            false,
        )),
        HirDeclarationMemberPoisonState::Clean,
    )
    .unwrap();
    assert_eq!(
        HirDeclarationMemberArena::try_new(owner, HirItemFamily::Metric, Box::new([member]),),
        Err(HirItemInvariantError::WrongMemberFamily {
            member: member_id,
            family: HirItemFamily::Metric,
        })
    );
}

#[test]
fn activity_ports_require_exact_names_locals_and_disjoint_direction_rows() {
    let module_id = module(5, 1);
    let owner = typed_id::<ItemId>(module_id, 1);
    let ty = typed_id::<TypeId>(module_id, 2);
    let local = typed_id::<LocalId>(module_id, 3);

    assert_eq!(
        HirActivityPortMember::try_new(HirRequiredName::Resolved(name("route_seed")), ty, None,),
        Err(HirItemInvariantError::ActivityPortLocalMismatch)
    );
    assert_eq!(
        HirActivityPortMember::try_new(HirRequiredName::Missing, ty, Some(local)),
        Err(HirItemInvariantError::ActivityPortLocalMismatch)
    );

    let port = HirActivityPortMember::try_new(
        HirRequiredName::Resolved(name("route_seed")),
        ty,
        Some(local),
    )
    .unwrap();
    assert_eq!(port.name().resolved().unwrap().as_str(), "route_seed");
    assert_eq!(port.ty(), ty);
    assert_eq!(port.local(), Some(local));

    let header = HirRetainedHeader::try_new(
        DeclarationIdentityFamily::Activity,
        HirRetainedPublicId::Resolved {
            value: PublicId::try_new("activity.route_planner").unwrap(),
            origin: HirPublicIdOrigin::Explicit,
        },
        HirRetainedName::Resolved(DeclarationName::try_new("RoutePlanner").unwrap()),
    )
    .unwrap();
    let callable_scope = typed_id(module_id, 4);
    assert_eq!(
        HirContractScopes::try_new(callable_scope, callable_scope, typed_id(module_id, 6)),
        Err(HirItemInvariantError::ContractScopeIdentityCollision)
    );
    let scopes = HirContractScopes::try_new(
        callable_scope,
        typed_id(module_id, 5),
        typed_id(module_id, 6),
    )
    .unwrap();
    let member = HirDeclarationMemberId::new(owner, 0);
    assert_eq!(
        HirActivityDeclaration::try_new(
            owner,
            header,
            scopes,
            HirActivityMode::Deterministic,
            HirActivityLifecycle::Stateless,
            Box::new([member]),
            Box::new([member]),
            Box::new([]),
            Box::new([]),
        ),
        Err(HirItemInvariantError::DuplicateActivityPortMember)
    );
}

#[test]
fn layer_kind_defaults_and_member_recovery_are_owned_by_typed_payloads() {
    for (kind, phase) in [
        (HirLayerKind::Background, HirRenderPhase::Background),
        (HirLayerKind::World2d, HirRenderPhase::World),
        (HirLayerKind::Character, HirRenderPhase::Characters),
        (HirLayerKind::Effects, HirRenderPhase::Effects),
        (HirLayerKind::Dialogue, HirRenderPhase::Dialogue),
        (HirLayerKind::GameView, HirRenderPhase::GameView),
        (HirLayerKind::HtmlView, HirRenderPhase::HtmlView),
        (HirLayerKind::Activity, HirRenderPhase::GameView),
        (HirLayerKind::Modal, HirRenderPhase::Modal),
        (HirLayerKind::Overlay, HirRenderPhase::Modal),
        (HirLayerKind::Debug, HirRenderPhase::Debug),
        (HirLayerKind::Agent, HirRenderPhase::AgentOverlay),
        (HirLayerKind::Offscreen, HirRenderPhase::Background),
        (HirLayerKind::Custom, HirRenderPhase::World),
    ] {
        assert_eq!(kind.default_phase(), Some(phase));
    }
    assert_eq!(
        HirLayerKind::Recovered(HirLayerKindIssue::Missing).default_phase(),
        None
    );

    let module_id = module(6, 1);
    let owner = typed_id::<ItemId>(module_id, 1);
    let member_id = HirDeclarationMemberId::new(owner, 0);
    let expression = typed_id::<ExprId>(module_id, 2);
    let clean_payload = HirLayerMemberPayload::new(
        HirLayerAssignmentState::Present,
        HirLayerMemberValue::Present(expression),
        false,
    );
    let member = HirDeclarationMember::try_new(
        member_id,
        HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Z(clean_payload)),
        HirDeclarationMemberPoisonState::Clean,
    )
    .unwrap();
    assert_eq!(member.id(), member_id);

    let missing_payload = HirLayerMemberPayload::new(
        HirLayerAssignmentState::Present,
        HirLayerMemberValue::<ExprId>::Missing,
        false,
    );
    assert_eq!(
        HirDeclarationMember::try_new(
            member_id,
            HirDeclarationMemberKind::LayerExpression(
                HirLayerExpressionMember::Z(missing_payload,)
            ),
            HirDeclarationMemberPoisonState::Clean,
        ),
        Err(HirItemInvariantError::InvalidPoisonState)
    );
    HirDeclarationMember::try_new(
        member_id,
        HirDeclarationMemberKind::LayerExpression(HirLayerExpressionMember::Z(missing_payload)),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::MissingInitializer),
    )
    .unwrap();

    let header = HirRetainedHeader::try_new(
        DeclarationIdentityFamily::Layer,
        HirRetainedPublicId::Resolved {
            value: PublicId::try_new("layer.dialogue").unwrap(),
            origin: HirPublicIdOrigin::Explicit,
        },
        HirRetainedName::Resolved(DeclarationName::try_new("dialogue_ui").unwrap()),
    )
    .unwrap();
    let declaration =
        HirLayerDeclaration::try_new(owner, header, HirLayerKind::Dialogue, Box::new([member_id]))
            .unwrap();
    assert_eq!(
        declaration.header().family(),
        DeclarationIdentityFamily::Layer
    );
    assert_eq!(declaration.kind(), HirLayerKind::Dialogue);
    assert_eq!(declaration.members(), [member_id]);
}

#[test]
fn layer_item_rejects_transplanted_member_rows_and_clean_recovered_headers() {
    let module_id = module(7, 1);
    let first_owner = typed_id::<ItemId>(module_id, 1);
    let second_owner = typed_id::<ItemId>(module_id, 2);
    let scope = typed_id::<ScopeId>(module_id, 3);
    let first_member = HirDeclarationMemberId::new(first_owner, 0);
    let second_member = HirDeclarationMemberId::new(second_owner, 0);
    let header = HirRetainedHeader::try_new(
        DeclarationIdentityFamily::Layer,
        HirRetainedPublicId::Resolved {
            value: PublicId::try_new("layer.dialogue").unwrap(),
            origin: HirPublicIdOrigin::Explicit,
        },
        HirRetainedName::Resolved(DeclarationName::try_new("dialogue_ui").unwrap()),
    )
    .unwrap();
    let transplanted = HirLayerDeclaration::try_new(
        first_owner,
        header,
        HirLayerKind::Dialogue,
        Box::new([first_member]),
    )
    .unwrap();

    assert_eq!(
        HirItem::try_new(
            second_owner,
            scope,
            empty_prefix(),
            HirItemKind::Layer(transplanted),
            Box::new([second_member]),
        ),
        Err(HirItemInvariantError::ItemPayloadMemberRowMismatch {
            owner: second_owner,
        })
    );

    let recovered_header = HirRetainedHeader::try_new(
        DeclarationIdentityFamily::Layer,
        HirRetainedPublicId::Recovered(HirRetainedPublicIdIssue::Missing),
        HirRetainedName::Missing,
    )
    .unwrap();
    let recovered = HirLayerDeclaration::try_new(
        second_owner,
        recovered_header,
        HirLayerKind::Recovered(HirLayerKindIssue::Missing),
        Box::new([]),
    )
    .unwrap();
    assert_eq!(
        HirItem::try_new(
            second_owner,
            scope,
            empty_prefix(),
            HirItemKind::Layer(recovered.clone()),
            Box::new([]),
        ),
        Err(HirItemInvariantError::InvalidPoisonState)
    );
    assert!(
        HirItem::try_new_with_state(
            second_owner,
            scope,
            empty_prefix(),
            HirItemKind::Layer(recovered),
            Box::new([]),
            HirItemPoisonState::Poisoned(HirItemIssue::MalformedHeader),
        )
        .is_ok()
    );
}

#[test]
fn layer_reference_recovery_poison_propagates_to_its_item() {
    let module_id = module(8, 1);
    let owner = typed_id::<ItemId>(module_id, 1);
    let scope = typed_id::<ScopeId>(module_id, 2);
    let member_id = HirDeclarationMemberId::new(owner, 0);
    let recovered_reference = HirIdRefValue::Recovered(HirIdRefRecovery::new(
        HirIdRefShape::Missing,
        HirIdRefIssue::Missing,
    ));
    let reference_payload = HirLayerMemberPayload::new(
        HirLayerAssignmentState::Present,
        HirLayerMemberValue::Present(recovered_reference),
        false,
    );
    assert_eq!(
        HirDeclarationMember::try_new(
            member_id,
            HirDeclarationMemberKind::LayerReference(HirLayerReferenceMember::Parent(
                reference_payload.clone(),
            )),
            HirDeclarationMemberPoisonState::Clean,
        ),
        Err(HirItemInvariantError::InvalidPoisonState)
    );
    let member = HirDeclarationMember::try_new(
        member_id,
        HirDeclarationMemberKind::LayerReference(HirLayerReferenceMember::Parent(
            reference_payload,
        )),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::RecoveredChild),
    )
    .unwrap();
    let header = HirRetainedHeader::try_new(
        DeclarationIdentityFamily::Layer,
        HirRetainedPublicId::Resolved {
            value: PublicId::try_new("layer.dialogue").unwrap(),
            origin: HirPublicIdOrigin::Explicit,
        },
        HirRetainedName::Resolved(DeclarationName::try_new("dialogue_ui").unwrap()),
    )
    .unwrap();
    let declaration =
        HirLayerDeclaration::try_new(owner, header, HirLayerKind::Dialogue, Box::new([member_id]))
            .unwrap();
    let clean_item = HirItem::try_new(
        owner,
        scope,
        empty_prefix(),
        HirItemKind::Layer(declaration.clone()),
        Box::new([member_id]),
    )
    .unwrap();
    let arena =
        HirDeclarationMemberArena::try_new(owner, HirItemFamily::Layer, Box::new([member.clone()]))
            .unwrap();
    let mut builder = HirDeclarationMemberIndexBuilder::new(module_id);
    assert_eq!(
        builder.stage(owner, &clean_item, arena.clone()),
        Err(HirItemInvariantError::InvalidPoisonState)
    );

    let poisoned_item = HirItem::try_new_with_state(
        owner,
        scope,
        empty_prefix(),
        HirItemKind::Layer(declaration),
        Box::new([member_id]),
        HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember),
    )
    .unwrap();
    builder.stage(owner, &poisoned_item, arena).unwrap();
    assert_eq!(builder.freeze().resolve(member_id).unwrap(), &member);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one module-member index matrix freezes and resolves composite identities across all owned arenas"
)]
fn module_member_index_freezes_multiple_arenas_and_resolves_composite_ids() {
    let local = module(10, 1);
    let owner = typed_id::<ItemId>(local, 1);
    let scope = typed_id::<ScopeId>(local, 2);
    let member_id = HirDeclarationMemberId::new(owner, 0);
    let header = HirRetainedHeader::try_new(
        DeclarationIdentityFamily::Character,
        HirRetainedPublicId::Resolved {
            value: PublicId::try_new("character.Alice").unwrap(),
            origin: HirPublicIdOrigin::DerivedFromName,
        },
        HirRetainedName::Resolved(DeclarationName::try_new("Alice").unwrap()),
    )
    .unwrap();
    let item = HirItem::try_new(
        owner,
        scope,
        empty_prefix(),
        HirItemKind::Character(HirCharacterDeclaration::new(
            header,
            HirCharacterSurfaceAlias::Absent,
            Some(member_id),
        )),
        Box::new([member_id]),
    )
    .unwrap();
    let member = HirDeclarationMember::try_new(
        member_id,
        HirDeclarationMemberKind::CharacterDisplayName(HirCharacterDisplayNameMember::new(
            HirCharacterAssignmentState::Present,
            Some(typed_id(local, 3)),
            false,
        )),
        HirDeclarationMemberPoisonState::Clean,
    )
    .unwrap();
    let arena = HirDeclarationMemberArena::try_new(
        owner,
        HirItemFamily::Character,
        Box::new([member.clone()]),
    )
    .unwrap();
    let mut builder = HirDeclarationMemberIndexBuilder::new(local);
    builder.stage(owner, &item, arena.clone()).unwrap();
    assert_eq!(
        builder.stage(owner, &item, arena),
        Err(HirItemInvariantError::DuplicateMemberArenaOwner { owner })
    );

    let second_owner = typed_id::<ItemId>(local, 4);
    let second_member_id = HirDeclarationMemberId::new(second_owner, 0);
    let second_header = HirRetainedHeader::try_new(
        DeclarationIdentityFamily::Character,
        HirRetainedPublicId::Resolved {
            value: PublicId::try_new("character.Bob").unwrap(),
            origin: HirPublicIdOrigin::DerivedFromName,
        },
        HirRetainedName::Resolved(DeclarationName::try_new("Bob").unwrap()),
    )
    .unwrap();
    let second_item = HirItem::try_new(
        second_owner,
        scope,
        empty_prefix(),
        HirItemKind::Character(HirCharacterDeclaration::new(
            second_header,
            HirCharacterSurfaceAlias::Absent,
            Some(second_member_id),
        )),
        Box::new([second_member_id]),
    )
    .unwrap();
    let second_member = HirDeclarationMember::try_new(
        second_member_id,
        HirDeclarationMemberKind::CharacterDisplayName(HirCharacterDisplayNameMember::new(
            HirCharacterAssignmentState::Present,
            Some(typed_id(local, 5)),
            false,
        )),
        HirDeclarationMemberPoisonState::Clean,
    )
    .unwrap();
    let second_arena = HirDeclarationMemberArena::try_new(
        second_owner,
        HirItemFamily::Character,
        Box::new([second_member]),
    )
    .unwrap();
    builder
        .stage(second_owner, &second_item, second_arena)
        .unwrap();

    let index = builder.freeze();
    assert_eq!(index.module(), local);
    assert_eq!(index.resolve(member_id).unwrap(), &member);
    assert_eq!(index.arenas().len(), 2);
    assert_eq!(
        index.resolve(HirDeclarationMemberId::new(owner, 1)),
        Err(HirDeclarationMemberIndexResolveError::UnknownOrdinal { owner, ordinal: 1 })
    );
    let unknown_owner = typed_id::<ItemId>(local, 6);
    assert_eq!(
        index.resolve(HirDeclarationMemberId::new(unknown_owner, 0)),
        Err(HirDeclarationMemberIndexResolveError::UnknownOwner {
            owner: unknown_owner,
        })
    );
}

#[test]
fn module_member_index_rejects_foreign_family_and_order_mismatches_before_freeze() {
    let local = module(11, 1);
    let foreign = module(12, 1);
    let owner = typed_id::<ItemId>(local, 1);
    let scope = typed_id::<ScopeId>(local, 2);
    let item = HirItem::try_new(
        owner,
        scope,
        empty_prefix(),
        HirItemKind::Error(HirErrorItem::new()),
        Box::new([]),
    )
    .unwrap();
    let mut builder = HirDeclarationMemberIndexBuilder::new(local);
    let foreign_owner = typed_id::<ItemId>(foreign, 1);
    let foreign_arena =
        HirDeclarationMemberArena::try_new(foreign_owner, HirItemFamily::Error, Box::new([]))
            .unwrap();
    assert_eq!(
        builder.stage(foreign_owner, &item, foreign_arena),
        Err(HirItemInvariantError::ForeignChild {
            expected: local,
            actual: foreign,
        })
    );

    let wrong_family =
        HirDeclarationMemberArena::try_new(owner, HirItemFamily::Module, Box::new([])).unwrap();
    assert_eq!(
        builder.stage(owner, &item, wrong_family),
        Err(HirItemInvariantError::MemberArenaFamilyMismatch {
            owner,
            item_family: HirItemFamily::Error,
            arena_family: HirItemFamily::Module,
        })
    );
    let empty_matching =
        HirDeclarationMemberArena::try_new(owner, HirItemFamily::Error, Box::new([])).unwrap();
    assert_eq!(
        builder.stage(owner, &item, empty_matching),
        Err(HirItemInvariantError::MemberArenaNotRequired { owner })
    );

    let character_owner = typed_id::<ItemId>(local, 3);
    let character_member = HirDeclarationMemberId::new(character_owner, 0);
    let header = HirRetainedHeader::try_new(
        DeclarationIdentityFamily::Character,
        HirRetainedPublicId::Resolved {
            value: PublicId::try_new("character.Bob").unwrap(),
            origin: HirPublicIdOrigin::DerivedFromName,
        },
        HirRetainedName::Resolved(DeclarationName::try_new("Bob").unwrap()),
    )
    .unwrap();
    let character = HirItem::try_new(
        character_owner,
        scope,
        empty_prefix(),
        HirItemKind::Character(HirCharacterDeclaration::new(
            header,
            HirCharacterSurfaceAlias::Absent,
            Some(character_member),
        )),
        Box::new([character_member]),
    )
    .unwrap();
    let empty_character_arena =
        HirDeclarationMemberArena::try_new(character_owner, HirItemFamily::Character, Box::new([]))
            .unwrap();
    assert_eq!(
        builder.stage(character_owner, &character, empty_character_arena),
        Err(HirItemInvariantError::MemberArenaItemOrderMismatch {
            owner: character_owner,
        })
    );
}

#[test]
fn predicate_and_proof_keep_typed_contract_scopes_and_body_children() {
    let local_module = module(5, 1);
    let owner = typed_id::<ItemId>(local_module, 1);
    let callable_scope = typed_id::<ScopeId>(local_module, 2);
    let requires_scope = typed_id::<ScopeId>(local_module, 3);
    let ensures_scope = typed_id::<ScopeId>(local_module, 4);
    let body_scope = typed_id::<ScopeId>(local_module, 5);
    let return_type = typed_id::<TypeId>(local_module, 6);
    let requirement = typed_id::<ExprId>(local_module, 7);
    let guarantee = typed_id::<ExprId>(local_module, 8);
    let statement = typed_id::<StmtId>(local_module, 9);
    let tail = typed_id::<ExprId>(local_module, 10);
    let signature = HirCallableSignature::try_new(
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([requirement]),
        Box::new([guarantee]),
        return_type,
    )
    .unwrap();
    let contract_scopes =
        HirContractScopes::try_new(callable_scope, requires_scope, ensures_scope).unwrap();
    let predicate = HirPredicate::try_new(
        HirRequiredName::Resolved(name("ordered")),
        signature,
        HirPredicateBody::Block {
            scope: body_scope,
            statements: Box::new([statement]),
            tail,
        },
        contract_scopes,
    )
    .unwrap();
    let item = HirItem::try_new(
        owner,
        callable_scope,
        empty_prefix(),
        HirItemKind::Predicate(predicate),
        Box::new([]),
    )
    .unwrap();

    assert_eq!(item.family(), HirItemFamily::Predicate);
    let HirItemKind::Predicate(predicate) = item.kind() else {
        panic!("predicate payload");
    };
    assert_eq!(predicate.requires_scope(), requires_scope);
    assert_eq!(predicate.ensures_scope(), ensures_scope);

    let foreign = module(6, 1);
    let signature = HirCallableSignature::try_new(
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
        Box::new([]),
        return_type,
    )
    .unwrap();
    let proof = HirProof::try_new(
        HirRequiredName::Resolved(name("preserve_order")),
        None,
        signature,
        crate::proof_return::HirProofReturnSemanticClass::Poisoned,
        ProofTrust::Verified,
        HirProofBody::Error {
            scope: body_scope,
            expression: typed_id(foreign, 1),
        },
        contract_scopes,
    );
    assert_eq!(
        proof,
        Err(HirItemInvariantError::ForeignChild {
            expected: local_module,
            actual: foreign,
        })
    );
}

#[test]
fn method_block_must_reuse_its_callable_scope() {
    let local = module(8, 1);
    let callable_scope = typed_id::<ScopeId>(local, 1);
    let second_scope = typed_id::<ScopeId>(local, 2);
    let tail = typed_id::<ExprId>(local, 3);
    let parameters =
        HirMethodParameterGroup::try_new(local, Box::new([])).expect("empty method group");

    assert_eq!(
        HirTraitFunction::try_new(
            local,
            empty_prefix(),
            HirRequiredName::Resolved(name("read")),
            Box::new([]),
            Box::new([parameters]),
            Box::new([]),
            None,
            callable_scope,
            Some(HirFunctionBody::Block {
                scope: second_scope,
                statements: Box::new([]),
                tail,
            }),
        ),
        Err(HirItemInvariantError::MethodBodyScopeMismatch {
            callable: callable_scope,
            body: second_scope,
        })
    );
}

#[test]
fn shared_flow_body_rejects_foreign_children() {
    let local = module(8, 1);
    let foreign = module(9, 1);
    let owner = typed_id::<ItemId>(local, 1);
    let scope = typed_id::<ScopeId>(local, 2);

    assert_eq!(
        HirThreadBody::try_new(
            HirThreadBodyOwner::Flow(owner),
            scope,
            Box::new([HirThreadFlowItem::DialogueApplication(
                typed_id(foreign, 1,)
            )]),
        ),
        Err(HirThreadBodyInvariantError::ForeignReference {
            expected: local,
            actual: foreign,
        })
    );
}

#[test]
fn retained_identity_and_known_family_recovery_remain_typed() {
    let name = DeclarationName::try_new("Alice").unwrap();
    let public_id = PublicId::try_new("character.Alice").unwrap();
    let header = HirRetainedHeader::try_new(
        DeclarationIdentityFamily::Character,
        HirRetainedPublicId::Resolved {
            value: public_id.clone(),
            origin: HirPublicIdOrigin::DerivedFromName,
        },
        HirRetainedName::Resolved(name),
    )
    .unwrap();
    assert_eq!(header.public_id().resolved(), Some(&public_id));
    assert_eq!(
        header.public_id().origin(),
        Some(HirPublicIdOrigin::DerivedFromName)
    );
    assert_eq!(
        HirRetainedHeader::try_new(
            DeclarationIdentityFamily::Asset,
            HirRetainedPublicId::Resolved {
                value: PublicId::try_new("asset.room").unwrap(),
                origin: HirPublicIdOrigin::Explicit,
            },
            HirRetainedName::Resolved(DeclarationName::try_new("room").unwrap()),
        ),
        Err(HirRetainedHeaderError::AssetIsCatalogOwned)
    );

    let module_id = module(7, 1);
    let owner = typed_id::<ItemId>(module_id, 1);
    let scope = typed_id::<ScopeId>(module_id, 2);
    let recovered = HirItem::try_new_with_state(
        owner,
        scope,
        empty_prefix(),
        HirItemKind::Module(HirModuleDeclaration::new(HirPathValue::Recovered(
            HirPathRecovery::new(HirPathRoot::Crate, 0, HirPathIssue::Empty),
        ))),
        Box::new([]),
        HirItemPoisonState::Poisoned(HirItemIssue::MissingName),
    )
    .unwrap();
    let HirItemKind::Module(module_declaration) = recovered.kind() else {
        panic!("recognized module recovery payload");
    };
    assert!(module_declaration.path().recovery().is_some());
    assert_eq!(recovered.family(), HirItemFamily::Module);
    assert_eq!(
        recovered.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingName)
    );

    let generic_error = HirItem::try_new(
        typed_id(module_id, 3),
        scope,
        empty_prefix(),
        HirItemKind::Error(HirErrorItem::new()),
        Box::new([]),
    )
    .unwrap();
    assert_eq!(
        generic_error.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::UnclassifiedSyntax)
    );
}

#[test]
fn retained_header_preserves_recovery_without_fabricating_identity_or_name() {
    let recovered = HirRetainedHeader::try_new(
        DeclarationIdentityFamily::Character,
        HirRetainedPublicId::Recovered(HirRetainedPublicIdIssue::Missing),
        HirRetainedName::Missing,
    )
    .unwrap();
    assert_eq!(
        recovered.public_id(),
        &HirRetainedPublicId::Recovered(HirRetainedPublicIdIssue::Missing)
    );
    assert_eq!(recovered.name(), &HirRetainedName::Missing);

    let wrong_family = PublicId::try_new("view.Alice").unwrap();
    assert!(
        HirRetainedHeader::try_new(
            DeclarationIdentityFamily::Character,
            HirRetainedPublicId::Recovered(HirRetainedPublicIdIssue::WrongFamily(wrong_family,)),
            HirRetainedName::Resolved(DeclarationName::try_new("Alice").unwrap()),
        )
        .is_ok()
    );
    assert_eq!(
        HirRetainedHeader::try_new(
            DeclarationIdentityFamily::Character,
            HirRetainedPublicId::Recovered(HirRetainedPublicIdIssue::WrongFamily(
                PublicId::try_new("character.Alice").unwrap(),
            )),
            HirRetainedName::Resolved(DeclarationName::try_new("Alice").unwrap()),
        ),
        Err(HirRetainedHeaderError::RecoveredIdentityMatchesFamily)
    );
    assert_eq!(
        HirRetainedHeader::try_new(
            DeclarationIdentityFamily::Character,
            HirRetainedPublicId::Recovered(HirRetainedPublicIdIssue::DerivedFromRecoveredName,),
            HirRetainedName::Resolved(DeclarationName::try_new("Alice").unwrap()),
        ),
        Err(HirRetainedHeaderError::RecoveredDerivationHasResolvedName)
    );
}

#[test]
fn character_member_poison_state_is_derived_from_the_final_payload_shape() {
    let local = module(13, 1);
    let owner = typed_id::<ItemId>(local, 1);
    let id = HirDeclarationMemberId::new(owner, 0);
    let value = typed_id::<ExprId>(local, 2);

    let clean = HirDeclarationMember::try_new(
        id,
        HirDeclarationMemberKind::CharacterDisplayName(HirCharacterDisplayNameMember::new(
            HirCharacterAssignmentState::Present,
            Some(value),
            false,
        )),
        HirDeclarationMemberPoisonState::Clean,
    )
    .unwrap();
    assert!(!clean.is_poisoned());

    for (payload, issue) in [
        (
            HirCharacterDisplayNameMember::new(HirCharacterAssignmentState::Missing, None, false),
            HirDeclarationMemberIssue::MissingAssignment,
        ),
        (
            HirCharacterDisplayNameMember::new(HirCharacterAssignmentState::Present, None, false),
            HirDeclarationMemberIssue::MissingInitializer,
        ),
        (
            HirCharacterDisplayNameMember::new(
                HirCharacterAssignmentState::Present,
                Some(value),
                true,
            ),
            HirDeclarationMemberIssue::Duplicate,
        ),
    ] {
        let member = HirDeclarationMember::try_new(
            id,
            HirDeclarationMemberKind::CharacterDisplayName(payload),
            HirDeclarationMemberPoisonState::Poisoned(issue),
        )
        .unwrap();
        assert_eq!(
            member.state(),
            HirDeclarationMemberPoisonState::Poisoned(issue)
        );
    }

    assert_eq!(
        HirDeclarationMember::try_new(
            id,
            HirDeclarationMemberKind::CharacterDisplayName(HirCharacterDisplayNameMember::new(
                HirCharacterAssignmentState::Missing,
                None,
                false,
            )),
            HirDeclarationMemberPoisonState::Poisoned(
                HirDeclarationMemberIssue::MissingInitializer,
            ),
        ),
        Err(HirItemInvariantError::InvalidPoisonState)
    );
    assert!(
        HirDeclarationMember::try_new(
            id,
            HirDeclarationMemberKind::CharacterRecovery(HirCharacterMemberRecovery::Unknown),
            HirDeclarationMemberPoisonState::Poisoned(
                HirDeclarationMemberIssue::UnclassifiedSyntax,
            ),
        )
        .is_ok()
    );
}

#[test]
fn clean_use_declaration_requires_at_least_one_flattened_binding() {
    assert_eq!(
        HirUseDeclaration::try_new(Box::new([])),
        Err(HirItemInvariantError::EmptyUseDeclaration)
    );

    let binding = HirUseBinding::new(
        HirPathValue::Resolved(path("story")),
        None,
        HirUseBindingKind::Item,
    );
    let declaration = HirUseDeclaration::try_new(Box::new([binding])).unwrap();
    assert_eq!(declaration.bindings().len(), 1);
}
