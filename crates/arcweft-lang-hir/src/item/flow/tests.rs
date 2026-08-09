use core::num::{NonZeroU32, NonZeroU64};

use super::*;
use crate::expr::{HirThreadBodyOwner, HirThreadFlowItem};
use crate::identity::{HirDatabaseId, HirTypedId, RawHirId};
use crate::leaf::HirEntityReference;
use crate::source_index::HirThreadBodySourceRole;

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

fn public_id(value: &str) -> HirIdRef {
    HirIdRef::absolute(HirEntityReference::try_new(value.into()).unwrap())
}

fn scopes(module: HirModuleId) -> HirContractScopes {
    HirContractScopes::try_new(
        typed_id(module, 10),
        typed_id(module, 11),
        typed_id(module, 12),
    )
    .unwrap()
}

fn body(owner: ItemId, scope: ScopeId) -> HirThreadBody {
    HirThreadBody::try_new(HirThreadBodyOwner::Flow(owner), scope, Box::new([])).unwrap()
}

fn flow_issue(
    owner: ItemId,
    class: HirFlowIssueClass,
    issue_owner: HirFlowIssueOwner,
) -> HirFlowIssue {
    HirFlowIssue::new(
        class,
        issue_owner,
        HirSourceQuery::ThreadBody {
            owner: HirThreadBodyOwner::Flow(owner),
            role: HirThreadBodySourceRole::Whole,
        },
    )
}

#[test]
fn flow_identity_retains_exact_four_states_without_fabricating_an_id() {
    let name_only = HirFlowIdentity::Name {
        name: name("opening"),
    };
    let id_only = HirFlowIdentity::PublicId {
        public_id: public_id("flow.opening"),
    };
    let both = HirFlowIdentity::PublicIdAndName {
        public_id: public_id("flow.opening"),
        name: name("opening"),
    };
    let missing = HirFlowIdentity::Missing;

    assert_eq!(name_only.name().unwrap().as_str(), "opening");
    assert!(name_only.public_id().is_none());
    assert!(id_only.name().is_none());
    assert_eq!(id_only.public_id().unwrap().absolute_family(), Some("flow"));
    assert_eq!(both.name().unwrap().as_str(), "opening");
    assert_eq!(both.public_id().unwrap().absolute_family(), Some("flow"));
    assert!(missing.is_missing());
    assert!(missing.name().is_none());
    assert!(missing.public_id().is_none());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one contract matrix verifies all nine typed variants and their single source order"
)]
fn flow_contracts_preserve_all_nine_variants_in_one_source_ordered_slice() {
    let module = module(1, 1);
    let owner = typed_id::<ItemId>(module, 1);
    let expressions = (20..=30)
        .map(|slot| typed_id::<ExprId>(module, slot))
        .collect::<Vec<_>>();
    let contracts = Box::new([
        HirFlowContractClause::Requires(HirContractCondition::new(
            HirContractMode::Default,
            expressions[0],
        )),
        HirFlowContractClause::Ensures(HirContractCondition::new(
            HirContractMode::Prove,
            expressions[1],
        )),
        HirFlowContractClause::Invariant(HirContractCondition::new(
            HirContractMode::CheckRuntime,
            expressions[2],
        )),
        HirFlowContractClause::Assume {
            expression: expressions[3],
        },
        HirFlowContractClause::Reads(
            HirContractOperandList::try_new(module, Box::new([expressions[4], expressions[5]]))
                .unwrap(),
        ),
        HirFlowContractClause::Effects(
            HirContractOperandList::try_new(module, Box::new([expressions[6]])).unwrap(),
        ),
        HirFlowContractClause::NoEffect {
            expression: expressions[7],
        },
        HirFlowContractClause::Modifies(
            HirContractOperandList::try_new(module, Box::new([expressions[8]])).unwrap(),
        ),
        HirFlowContractClause::Decreases {
            expression: expressions[9],
        },
    ]);
    let contract_scopes = scopes(module);
    let result_local = HirFlowResultLocal::new(typed_id(module, 40));
    let declaration = HirFlowItem::try_new(
        owner,
        HirFlowIdentity::Name {
            name: name("opening"),
        },
        Box::new([]),
        Box::new([]),
        HirFlowReturn::Authored(typed_id(module, 41)),
        Box::new([]),
        contract_scopes,
        Some(result_local),
        contracts,
        body(owner, typed_id(module, 13)),
        HirFlowPoison::clean(),
    )
    .unwrap();

    assert_eq!(declaration.contracts().len(), 9);
    assert!(matches!(
        &declaration.contracts()[0],
        HirFlowContractClause::Requires(_)
    ));
    assert!(matches!(
        &declaration.contracts()[1],
        HirFlowContractClause::Ensures(_)
    ));
    assert!(matches!(
        &declaration.contracts()[2],
        HirFlowContractClause::Invariant(_)
    ));
    assert!(matches!(
        &declaration.contracts()[3],
        HirFlowContractClause::Assume { .. }
    ));
    assert!(matches!(
        &declaration.contracts()[4],
        HirFlowContractClause::Reads(_)
    ));
    assert!(matches!(
        &declaration.contracts()[5],
        HirFlowContractClause::Effects(_)
    ));
    assert!(matches!(
        &declaration.contracts()[6],
        HirFlowContractClause::NoEffect { .. }
    ));
    assert!(matches!(
        &declaration.contracts()[7],
        HirFlowContractClause::Modifies(_)
    ));
    assert!(matches!(
        &declaration.contracts()[8],
        HirFlowContractClause::Decreases { .. }
    ));
    let HirFlowContractClause::Ensures(condition) = &declaration.contracts()[1] else {
        unreachable!();
    };
    assert_eq!(condition.mode(), HirContractMode::Prove);
    assert_eq!(condition.expression(), expressions[1]);
    let HirFlowContractClause::Reads(reads) = &declaration.contracts()[4] else {
        unreachable!();
    };
    assert_eq!(reads.operands(), &expressions[4..=5]);
    assert_eq!(declaration.result_local(), Some(result_local));
    assert_eq!(
        declaration.result().authored_type(),
        Some(typed_id(module, 41))
    );
    assert_eq!(declaration.callable_scope(), contract_scopes.callable());
    assert_eq!(declaration.requires_scope(), contract_scopes.requires());
    assert_eq!(declaration.ensures_scope(), contract_scopes.ensures());
    assert_eq!(declaration.body_scope(), typed_id(module, 13));
}

#[test]
fn result_local_exists_exactly_when_an_ensures_clause_exists() {
    let module = module(2, 1);
    let owner = typed_id::<ItemId>(module, 1);
    let contract_scopes = scopes(module);
    let result_local = HirFlowResultLocal::new(typed_id(module, 30));
    let ensures = Box::new([HirFlowContractClause::Ensures(HirContractCondition::new(
        HirContractMode::DebugCheck,
        typed_id(module, 31),
    ))]);

    assert_eq!(
        HirFlowItem::try_new(
            owner,
            HirFlowIdentity::Name { name: name("main") },
            Box::new([]),
            Box::new([]),
            HirFlowReturn::OmittedUnit,
            Box::new([]),
            contract_scopes,
            None,
            ensures.clone(),
            body(owner, typed_id(module, 13)),
            HirFlowPoison::clean(),
        ),
        Err(HirItemInvariantError::InvalidFlowResultLocal)
    );
    assert_eq!(
        HirFlowItem::try_new(
            owner,
            HirFlowIdentity::Name { name: name("main") },
            Box::new([]),
            Box::new([]),
            HirFlowReturn::OmittedUnit,
            Box::new([]),
            contract_scopes,
            Some(result_local),
            Box::new([]),
            body(owner, typed_id(module, 13)),
            HirFlowPoison::clean(),
        ),
        Err(HirItemInvariantError::InvalidFlowResultLocal)
    );
    let valid = HirFlowItem::try_new(
        owner,
        HirFlowIdentity::Name { name: name("main") },
        Box::new([]),
        Box::new([]),
        HirFlowReturn::OmittedUnit,
        Box::new([]),
        contract_scopes,
        Some(result_local),
        ensures,
        body(owner, typed_id(module, 13)),
        HirFlowPoison::clean(),
    )
    .unwrap();
    assert_eq!(valid.result_local().unwrap().local(), typed_id(module, 30));
    assert_eq!(valid.result(), &HirFlowReturn::OmittedUnit);
}

#[test]
fn flow_rejects_callable_parameter_shapes_not_admitted_by_flow() {
    let module = module(3, 1);
    let owner = typed_id::<ItemId>(module, 1);
    let pattern = typed_id::<PatternId>(module, 20);
    let ty = typed_id::<TypeId>(module, 21);
    let local = typed_id::<LocalId>(module, 22);
    let default = typed_id::<ExprId>(module, 23);
    let rest = HirParameter::try_new(
        pattern,
        ty,
        HirParameterKind::RestPositional,
        None,
        Box::new([local]),
    )
    .unwrap();
    let defaulted = HirParameter::try_new(
        pattern,
        ty,
        HirParameterKind::Fixed,
        Some(default),
        Box::new([local]),
    )
    .unwrap();

    for parameter in [rest, defaulted] {
        assert_eq!(
            HirFlowItem::try_new(
                owner,
                HirFlowIdentity::Name { name: name("main") },
                Box::new([]),
                Box::new([parameter]),
                HirFlowReturn::OmittedUnit,
                Box::new([]),
                scopes(module),
                None,
                Box::new([]),
                body(owner, typed_id(module, 13)),
                HirFlowPoison::clean(),
            ),
            Err(HirItemInvariantError::InvalidFlowParameterShape)
        );
    }
}

#[test]
fn flow_constructor_rejects_foreign_children_and_scope_collisions() {
    let local = module(4, 1);
    let foreign = module(5, 1);
    let owner = typed_id::<ItemId>(local, 1);
    let contract_scopes = scopes(local);
    let foreign_contract = HirFlowContractClause::Assume {
        expression: typed_id(foreign, 20),
    };

    assert_eq!(
        HirFlowItem::try_new(
            owner,
            HirFlowIdentity::Name { name: name("main") },
            Box::new([]),
            Box::new([]),
            HirFlowReturn::OmittedUnit,
            Box::new([]),
            contract_scopes,
            None,
            Box::new([foreign_contract]),
            body(owner, typed_id(local, 13)),
            HirFlowPoison::clean(),
        ),
        Err(HirItemInvariantError::ForeignChild {
            expected: local,
            actual: foreign,
        })
    );

    assert_eq!(
        HirFlowItem::try_new(
            owner,
            HirFlowIdentity::Name { name: name("main") },
            Box::new([]),
            Box::new([]),
            HirFlowReturn::OmittedUnit,
            Box::new([]),
            contract_scopes,
            None,
            Box::new([]),
            body(owner, contract_scopes.callable()),
            HirFlowPoison::clean(),
        ),
        Err(HirItemInvariantError::FlowScopeIdentityCollision)
    );
}

#[test]
fn flow_poison_retains_canonical_primary_and_requires_item_poison_propagation() {
    use super::super::{HirItem, HirItemIssue, HirItemKind, HirItemPoisonState, HirItemPrefix};

    let module = module(6, 1);
    let owner = typed_id::<ItemId>(module, 1);
    let signature_issue = flow_issue(
        owner,
        HirFlowIssueClass::Signature,
        HirFlowIssueOwner::Type(typed_id(module, 20)),
    );
    let body_issue = flow_issue(
        owner,
        HirFlowIssueClass::BodyChild,
        HirFlowIssueOwner::Stmt(typed_id(module, 21)),
    );
    let poison =
        HirFlowPoison::from_ordered_issues(Box::new([signature_issue.clone(), body_issue.clone()]));
    assert_eq!(poison.primary(), Some(&signature_issue));
    assert_eq!(poison.related(), [body_issue]);

    let declaration = HirFlowItem::try_new(
        owner,
        HirFlowIdentity::Name { name: name("main") },
        Box::new([]),
        Box::new([]),
        HirFlowReturn::OmittedUnit,
        Box::new([]),
        scopes(module),
        None,
        Box::new([]),
        body(owner, typed_id(module, 13)),
        poison,
    )
    .unwrap();
    let item_scope = typed_id::<ScopeId>(module, 2);
    let prefix = HirItemPrefix::new(None, Box::new([]), None);
    assert_eq!(
        HirItem::try_new(
            owner,
            item_scope,
            prefix.clone(),
            HirItemKind::Flow(declaration.clone()),
            Box::new([]),
        ),
        Err(HirItemInvariantError::InvalidPoisonState)
    );
    assert!(
        HirItem::try_new_with_state(
            owner,
            item_scope,
            prefix,
            HirItemKind::Flow(declaration),
            Box::new([]),
            HirItemPoisonState::Poisoned(HirItemIssue::Recovery),
        )
        .is_ok()
    );

    let clean_declaration = HirFlowItem::try_new(
        owner,
        HirFlowIdentity::Name { name: name("main") },
        Box::new([]),
        Box::new([]),
        HirFlowReturn::OmittedUnit,
        Box::new([]),
        scopes(module),
        None,
        Box::new([]),
        body(owner, typed_id(module, 13)),
        HirFlowPoison::clean(),
    )
    .unwrap();
    assert_eq!(
        HirItem::try_new_with_state(
            owner,
            item_scope,
            HirItemPrefix::new(None, Box::new([]), None),
            HirItemKind::Flow(clean_declaration),
            Box::new([]),
            HirItemPoisonState::Poisoned(HirItemIssue::Recovery),
        ),
        Err(HirItemInvariantError::InvalidPoisonState)
    );
}

#[test]
fn flow_poison_canonicalizes_classes_without_losing_same_class_order() {
    let module = module(6, 2);
    let owner = typed_id::<ItemId>(module, 1);
    let body = flow_issue(
        owner,
        HirFlowIssueClass::BodyChild,
        HirFlowIssueOwner::Stmt(typed_id(module, 20)),
    );
    let identity_name = flow_issue(
        owner,
        HirFlowIssueClass::Identity,
        HirFlowIssueOwner::Item(owner),
    );
    let identity_public_id = flow_issue(
        owner,
        HirFlowIssueClass::Identity,
        HirFlowIssueOwner::Item(owner),
    );
    let signature = flow_issue(
        owner,
        HirFlowIssueClass::Signature,
        HirFlowIssueOwner::Type(typed_id(module, 21)),
    );
    let contract_later = flow_issue(
        owner,
        HirFlowIssueClass::Contract,
        HirFlowIssueOwner::Expr(typed_id(module, 22)),
    );
    let contract_first = flow_issue(
        owner,
        HirFlowIssueClass::Contract,
        HirFlowIssueOwner::Expr(typed_id(module, 23)),
    );
    let unclosed = flow_issue(
        owner,
        HirFlowIssueClass::UnclosedBody,
        HirFlowIssueOwner::Item(owner),
    );

    let poison = HirFlowPoison::from_ordered_issues(Box::new([
        body.clone(),
        contract_later.clone(),
        identity_name.clone(),
        signature.clone(),
        identity_public_id.clone(),
        contract_first.clone(),
        unclosed.clone(),
    ]));
    let issues = poison
        .primary()
        .into_iter()
        .chain(poison.related())
        .collect::<Vec<_>>();

    assert_eq!(
        issues,
        [
            &identity_name,
            &identity_public_id,
            &signature,
            &contract_later,
            &contract_first,
            &body,
            &unclosed,
        ]
    );
}

#[test]
fn missing_identity_requires_typed_identity_poison_and_issue_owners_are_bound() {
    let local = module(7, 1);
    let foreign = module(8, 1);
    let owner = typed_id::<ItemId>(local, 1);
    let common = || {
        (
            Box::new([]),
            Box::new([]),
            HirFlowReturn::OmittedUnit,
            Box::new([]),
            scopes(local),
            None,
            Box::new([]),
            body(owner, typed_id(local, 13)),
        )
    };
    let (generics, parameters, result, where_predicates, scopes, result_local, contracts, body) =
        common();
    assert_eq!(
        HirFlowItem::try_new(
            owner,
            HirFlowIdentity::Missing,
            generics,
            parameters,
            result,
            where_predicates,
            scopes,
            result_local,
            contracts,
            body,
            HirFlowPoison::clean(),
        ),
        Err(HirItemInvariantError::InvalidFlowPoison)
    );

    let identity_issue = flow_issue(
        owner,
        HirFlowIssueClass::Identity,
        HirFlowIssueOwner::Item(owner),
    );
    let (generics, parameters, result, where_predicates, scopes, result_local, contracts, body) =
        common();
    assert!(
        HirFlowItem::try_new(
            owner,
            HirFlowIdentity::Missing,
            generics,
            parameters,
            result,
            where_predicates,
            scopes,
            result_local,
            contracts,
            body,
            HirFlowPoison::from_ordered_issues(Box::new([identity_issue])),
        )
        .is_ok()
    );

    let foreign_issue = flow_issue(
        owner,
        HirFlowIssueClass::Signature,
        HirFlowIssueOwner::Item(typed_id(foreign, 1)),
    );
    let (generics, parameters, result, where_predicates, scopes, result_local, contracts, body) =
        common();
    assert_eq!(
        HirFlowItem::try_new(
            owner,
            HirFlowIdentity::Name { name: name("main") },
            generics,
            parameters,
            result,
            where_predicates,
            scopes,
            result_local,
            contracts,
            body,
            HirFlowPoison::from_ordered_issues(Box::new([foreign_issue])),
        ),
        Err(HirItemInvariantError::ForeignChild {
            expected: local,
            actual: foreign,
        })
    );
}

#[test]
fn operand_list_and_thread_body_keep_module_and_order_invariants() {
    let local = module(9, 1);
    let foreign = module(10, 1);
    let first = typed_id::<ExprId>(local, 1);
    let second = typed_id::<ExprId>(local, 2);
    let operands = HirContractOperandList::try_new(local, Box::new([first, second])).unwrap();
    assert_eq!(operands.operands(), [first, second]);
    assert_eq!(
        HirContractOperandList::try_new(local, Box::new([typed_id(foreign, 1)])),
        Err(HirItemInvariantError::ForeignChild {
            expected: local,
            actual: foreign,
        })
    );

    let owner = typed_id::<ItemId>(local, 3);
    let statement = typed_id::<StmtId>(local, 4);
    let body = HirThreadBody::try_new(
        HirThreadBodyOwner::Flow(owner),
        typed_id(local, 5),
        Box::new([HirThreadFlowItem::Statement(statement)]),
    )
    .unwrap();
    assert_eq!(body.items(), [HirThreadFlowItem::Statement(statement)]);
}
