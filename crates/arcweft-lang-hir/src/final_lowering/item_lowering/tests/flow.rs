use super::*;

use crate::item::{
    HirContractMode, HirFlowContractClause, HirFlowIdentity, HirFlowIssueClass, HirFlowIssueOwner,
    HirFlowItem, HirFlowReturn,
};
use crate::source_index::{
    HirFlowContractSourcePart, HirFlowParameterSourcePart, HirFlowReturnSourcePart,
    HirFlowSourceRole, HirItemSourceRole, HirSourceCommitInvariantError, HirSourceLookup,
    HirSourceOwnerStatus, HirSourcePresence, HirSourceQuery, HirSourceQueryError,
    HirThreadBodySourceRole, HirThreadFlowItemSourcePart,
};

fn resolve_flow(
    module: &HirModule,
    ordinal: usize,
) -> (crate::identity::ItemId, &HirItem, &HirFlowItem) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Flow(flow) = item.kind() else {
        panic!("source-ordered item {ordinal} must be an ordinary Flow")
    };
    (owner, item, flow)
}

fn flow_query<'module>(
    module: &'module HirModule,
    parsed: &ParsedSource,
    owner: crate::identity::ItemId,
    role: HirFlowSourceRole,
) -> HirSourceLookup<'module> {
    flow_query_result(module, parsed, owner, role).unwrap()
}

fn flow_query_result<'module>(
    module: &'module HirModule,
    parsed: &ParsedSource,
    owner: crate::identity::ItemId,
    role: HirFlowSourceRole,
) -> Result<HirSourceLookup<'module>, HirSourceQueryError> {
    module.source_site(parsed.document().identity(), flow_source_query(owner, role))
}

const fn flow_source_query(
    owner: crate::identity::ItemId,
    role: HirFlowSourceRole,
) -> HirSourceQuery {
    HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Flow(role),
    }
}

#[test]
fn ordinary_flow_lowers_one_shared_signature_contract_and_body_graph() {
    let source = concat!(
        "pub flow @flow.ordered ordered<T>(value: T) -> T where T: Bound\n",
        "requires prove ready(value)\n",
        "effects { asset.read }\n",
        "ensures check result\n",
        "reads { value.field }\n",
        "invariant debug stable(value)\n",
        "ensures no_effect network.request\n",
        "modifies { value.field }\n",
        "assume external_ok\n",
        "decreases value.remaining\n",
        "{}\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-flow-clean", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Clean,
        "Flow poison: {:?}",
        flow.poison()
    );
    assert!(matches!(
        flow.identity(),
        HirFlowIdentity::PublicIdAndName { public_id, name }
            if public_id.absolute_family() == Some("flow") && name.as_str() == "ordered"
    ));
    assert_eq!(flow.generic_parameters().len(), 1);
    assert_eq!(flow.parameters().len(), 1);
    assert!(matches!(flow.result(), HirFlowReturn::Authored(_)));
    assert_eq!(flow.where_predicates().len(), 1);
    assert_eq!(flow.contracts().len(), 9);
    assert!(matches!(
        flow.contracts()[0],
        HirFlowContractClause::Requires(ref condition)
            if condition.mode() == HirContractMode::Prove
    ));
    assert!(matches!(
        flow.contracts()[2],
        HirFlowContractClause::Ensures(ref condition)
            if condition.mode() == HirContractMode::CheckRuntime
    ));
    assert!(matches!(
        flow.contracts()[4],
        HirFlowContractClause::Invariant(ref condition)
            if condition.mode() == HirContractMode::DebugCheck
    ));
    assert!(matches!(
        flow.contracts()[5],
        HirFlowContractClause::NoEffect { .. }
    ));
    assert!(flow.body().items().is_empty());

    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), flow.callable_scope())
        .unwrap();
    assert_eq!(callable.kind(), HirScopeKind::Callable);
    assert_eq!(callable.parent(), Some(item.scope()));
    assert_eq!(callable.owner(), &HirScopeOwner::Item(owner));
    assert_eq!(
        callable.children(),
        [
            flow.requires_scope(),
            flow.ensures_scope(),
            flow.body_scope()
        ]
    );
    assert_eq!(callable.locals(), flow.parameters()[0].locals());
    let requires = module
        .arenas()
        .scopes()
        .resolve(module.slots(), flow.requires_scope())
        .unwrap();
    assert_eq!(requires.kind(), HirScopeKind::ContractRequires);
    assert!(requires.locals().is_empty());
    let ensures = module
        .arenas()
        .scopes()
        .resolve(module.slots(), flow.ensures_scope())
        .unwrap();
    let [result] = ensures.locals() else {
        panic!("one shared postcondition result local")
    };
    assert_eq!(flow.result_local().unwrap().local(), *result);
    let result = module
        .arenas()
        .locals()
        .resolve(module.slots(), *result)
        .unwrap();
    assert_eq!(result.kind(), HirLocalKind::PostconditionResult);
    assert_eq!(result.annotation(), flow.result().authored_type());
    let body = module
        .arenas()
        .scopes()
        .resolve(module.slots(), flow.body_scope())
        .unwrap();
    assert_eq!(body.kind(), HirScopeKind::Flow);
    assert_eq!(body.parent(), Some(flow.callable_scope()));

    let whole = flow_query(&module, &parsed, owner, HirFlowSourceRole::Whole);
    assert!(matches!(whole.presence(), HirSourcePresence::Present(_)));
    assert_eq!(whole.owner_status(), HirSourceOwnerStatus::Clean);
    let clause_keyword = flow_query(
        &module,
        &parsed,
        owner,
        HirFlowSourceRole::ContractClause {
            ordinal: 5,
            part: HirFlowContractSourcePart::ClauseKeyword,
        },
    );
    let no_effect_keyword = flow_query(
        &module,
        &parsed,
        owner,
        HirFlowSourceRole::ContractClause {
            ordinal: 5,
            part: HirFlowContractSourcePart::NoEffectKeyword,
        },
    );
    let (HirSourcePresence::Present(clause_keyword), HirSourcePresence::Present(no_effect_keyword)) =
        (clause_keyword.presence(), no_effect_keyword.presence())
    else {
        panic!("both authored no-effect keywords must retain exact source sites")
    };
    assert_ne!(clause_keyword, no_effect_keyword);
}

#[test]
fn ordinary_flow_identity_matrix_retains_raw_ids_and_typed_poison() {
    for (ordinal, (source, expected_clean)) in [
        ("flow opening {}", true),
        ("flow @flow.opening {}", true),
        ("flow @flow.opening opening {}", true),
        ("flow @flow:. opening {}", true),
        ("flow @view.opening {}", false),
        ("flow @flow.opening start {}", false),
        ("flow {}", false),
    ]
    .into_iter()
    .enumerate()
    {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-flow-identity-{ordinal}"),
            source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let mut transaction = stage(&database, &parsed, &key);
        transaction
            .lower_attached_source_file_items(&parsed.tree())
            .unwrap();
        let module = transaction
            .finish(&mut database)
            .unwrap_or_else(|error| panic!("{source}: {error:?}"))
            .into_module();
        let (owner, item, flow) = resolve_flow(&module, 0);
        assert_eq!(!item.is_poisoned(), expected_clean, "{source}");
        assert_eq!(flow.poison().is_poisoned(), !expected_clean, "{source}");
        if !expected_clean {
            assert_eq!(
                flow.poison().primary().unwrap().class(),
                HirFlowIssueClass::Identity,
                "{source}"
            );
            assert_eq!(
                flow_query(&module, &parsed, owner, HirFlowSourceRole::Whole).owner_status(),
                HirSourceOwnerStatus::Poisoned,
                "{source}"
            );
        }
    }
}

#[test]
fn flow_id_name_mismatch_keeps_name_primary_and_public_id_related() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-identity-mismatch-order",
        "flow @flow.opening start {}",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);

    assert!(item.is_poisoned());
    let primary = flow.poison().primary().unwrap();
    assert_eq!(primary.class(), HirFlowIssueClass::Identity);
    assert_eq!(primary.owner(), HirFlowIssueOwner::Item(owner));
    assert_eq!(
        primary.source(),
        &flow_source_query(owner, HirFlowSourceRole::Name)
    );
    let [public_id] = flow.poison().related() else {
        panic!("ID/name mismatch must retain exactly one related public-ID issue")
    };
    assert_eq!(public_id.class(), HirFlowIssueClass::Identity);
    assert_eq!(public_id.owner(), HirFlowIssueOwner::Item(owner));
    assert_eq!(
        public_id.source(),
        &flow_source_query(owner, HirFlowSourceRole::PublicId)
    );
}

#[test]
fn flow_reserved_result_and_missing_body_commit_roleful_recovery() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-reserved-result",
        "flow constrained(result: Bool) ensures result {}",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, flow) = resolve_flow(&module, 0);
    assert!(item.is_poisoned());
    assert_eq!(
        flow.poison().primary().unwrap().class(),
        HirFlowIssueClass::Signature
    );
    let parameter_local = module
        .arenas()
        .locals()
        .resolve(module.slots(), flow.parameters()[0].locals()[0])
        .unwrap();
    assert_eq!(parameter_local.name().as_str(), "result");
    assert!(parameter_local.is_poisoned());
    assert!(flow.result_local().is_some());

    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-missing-body",
        "flow unfinished",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );
    assert_eq!(
        flow.poison().primary().unwrap().class(),
        HirFlowIssueClass::MissingBody
    );
    assert!(flow.body().items().is_empty());
    assert_eq!(
        module
            .arenas()
            .scopes()
            .resolve(module.slots(), flow.body_scope())
            .unwrap()
            .kind(),
        HirScopeKind::Flow
    );
    assert!(matches!(
        flow_query(&module, &parsed, owner, HirFlowSourceRole::Body).presence(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
    ));
}

#[test]
fn flow_absent_roles_are_not_manifest_rows_and_win_before_source_identity() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-inapplicable",
        "flow plain {}",
    );
    let wrong_source = parse(
        "arcweft-test://proof/final-hir-flow-inapplicable-wrong-source",
        "flow other {}",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, _, _) = resolve_flow(&module, 0);

    for role in [
        HirFlowSourceRole::Visibility,
        HirFlowSourceRole::PublicId,
        HirFlowSourceRole::GenericGroup,
        HirFlowSourceRole::ParameterGroup,
        HirFlowSourceRole::Return {
            part: HirFlowReturnSourcePart::Whole,
        },
        HirFlowSourceRole::Return {
            part: HirFlowReturnSourcePart::Arrow,
        },
        HirFlowSourceRole::Return {
            part: HirFlowReturnSourcePart::Type,
        },
        HirFlowSourceRole::WhereClause,
    ] {
        let query = flow_source_query(owner, role);
        assert_eq!(
            module.source_components().requirement(&query),
            None,
            "{role:?}"
        );
        assert!(matches!(
            module.source_site(wrong_source.document().identity(), query),
            Err(HirSourceQueryError::ItemRoleNotApplicable {
                owner: actual,
                role: HirItemSourceRole::Flow(actual_role),
            }) if actual == owner && actual_role == role
        ));
    }

    assert!(matches!(
        flow_query(&module, &parsed, owner, HirFlowSourceRole::Name).presence(),
        HirSourcePresence::Present(_)
    ));
}

#[test]
fn flow_role_validation_is_bounds_first_and_rejects_default_mode() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-role-order",
        "flow guarded\nrequires ready\nreads asset\n{}\n",
    );
    let wrong_source = parse(
        "arcweft-test://proof/final-hir-flow-role-order-wrong-source",
        "flow other {}",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, _, _) = resolve_flow(&module, 0);

    let mode = HirFlowSourceRole::ContractClause {
        ordinal: 0,
        part: HirFlowContractSourcePart::Mode,
    };
    assert_eq!(
        module
            .source_components()
            .requirement(&flow_source_query(owner, mode)),
        None
    );
    assert!(matches!(
        flow_query_result(&module, &wrong_source, owner, mode),
        Err(HirSourceQueryError::ItemRoleNotApplicable { .. })
    ));

    for part in [
        HirFlowContractSourcePart::OpenDelimiter,
        HirFlowContractSourcePart::CloseDelimiter,
    ] {
        let role = HirFlowSourceRole::ContractClause { ordinal: 1, part };
        assert_eq!(
            module
                .source_components()
                .requirement(&flow_source_query(owner, role)),
            None
        );
        assert!(matches!(
            flow_query_result(&module, &wrong_source, owner, role),
            Err(HirSourceQueryError::ItemRoleNotApplicable { .. })
        ));
    }

    for (role, length) in [
        (
            HirFlowSourceRole::Parameter {
                ordinal: 0,
                part: HirFlowParameterSourcePart::Whole,
            },
            0,
        ),
        (
            HirFlowSourceRole::ContractClause {
                ordinal: 2,
                part: HirFlowContractSourcePart::Whole,
            },
            2,
        ),
        (HirFlowSourceRole::TrailingRecovery { ordinal: 0 }, 0),
    ] {
        assert!(matches!(
            flow_query_result(&module, &wrong_source, owner, role),
            Err(HirSourceQueryError::ItemOrdinalOutOfBounds {
                owner: actual,
                role: HirItemSourceRole::Flow(actual_role),
                length: actual_length,
            }) if actual == owner && actual_role == role && actual_length == length
        ));
    }
}

#[test]
fn flow_signature_recovery_uses_one_committed_trailing_ordinal_family() {
    let source = "flow invalid(first: Int = make_value())(second: Int) -> Int {}";
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-signature-recovery-source",
        source,
    );
    let wrong_source = parse(
        "arcweft-test://proof/final-hir-flow-signature-recovery-wrong-source",
        "flow other {}",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, _, flow) = resolve_flow(&module, 0);

    assert_eq!(flow.parameters().len(), 1);
    let issues = flow
        .poison()
        .primary()
        .into_iter()
        .chain(flow.poison().related())
        .collect::<Vec<_>>();
    assert_eq!(issues.len(), 2);
    for (ordinal, expected_start) in [
        (0, source.find('=').unwrap()),
        (1, source.find("(second").unwrap()),
    ] {
        let issue = issues[usize::try_from(ordinal).unwrap()];
        assert_eq!(issue.class(), HirFlowIssueClass::Signature);
        assert_eq!(issue.owner(), HirFlowIssueOwner::Item(owner));
        assert_eq!(
            issue.source(),
            &flow_source_query(owner, HirFlowSourceRole::TrailingRecovery { ordinal },)
        );
        let lookup = flow_query(
            &module,
            &parsed,
            owner,
            HirFlowSourceRole::TrailingRecovery { ordinal },
        );
        let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
            panic!("signature recovery {ordinal} must retain an authored span")
        };
        assert_eq!(span.range().start(), expected_start);
    }

    assert!(matches!(
        flow_query_result(
            &module,
            &wrong_source,
            owner,
            HirFlowSourceRole::TrailingRecovery { ordinal: 2 },
        ),
        Err(HirSourceQueryError::ItemOrdinalOutOfBounds { length: 2, .. })
    ));
}

#[test]
fn flow_body_projects_the_shared_sixteen_variant_inventory_without_a_tail() {
    let source = format!("flow matrix {{\n{}}}\n", thread_flow_matrix_body());
    let parsed = parse("arcweft-test://proof/final-hir-flow-body-matrix", &source);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, flow) = resolve_flow(&module, 0);
    assert!(item.is_poisoned(), "the Error row is recovery-only");
    assert_eq!(flow.body().items().len(), 16);
    assert_eq!(
        flow.poison().primary().unwrap().class(),
        HirFlowIssueClass::BodyChild
    );
    assert!(matches!(
        flow.body().items()[0],
        crate::expr::HirThreadFlowItem::Statement(_)
    ));
    assert!(matches!(
        flow.body().items()[1],
        crate::expr::HirThreadFlowItem::DialogueApplication(_)
    ));
    assert!(matches!(
        flow.body().items()[15],
        crate::expr::HirThreadFlowItem::Error(_)
    ));
}

#[test]
fn flow_body_retains_every_recovered_child_before_the_missing_close() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-body-recovery-order",
        "flow recovered {\n    ???\n    ???\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);

    assert!(item.is_poisoned());
    assert_eq!(flow.body().items().len(), 2);
    let issues = flow
        .poison()
        .primary()
        .into_iter()
        .chain(flow.poison().related())
        .collect::<Vec<_>>();
    assert_eq!(issues.len(), 3);
    for (ordinal, issue) in issues[..2].iter().enumerate() {
        let ordinal = u32::try_from(ordinal).unwrap();
        assert_eq!(issue.class(), HirFlowIssueClass::BodyChild);
        assert!(matches!(issue.owner(), HirFlowIssueOwner::Stmt(_)));
        assert_eq!(
            issue.source(),
            &HirSourceQuery::ThreadBody {
                owner: crate::expr::HirThreadBodyOwner::Flow(owner),
                role: HirThreadBodySourceRole::Item {
                    ordinal,
                    part: HirThreadFlowItemSourcePart::ChildWhole,
                },
            }
        );
    }
    assert_eq!(issues[2].class(), HirFlowIssueClass::UnclosedBody);
    assert_eq!(issues[2].owner(), HirFlowIssueOwner::Item(owner));
    assert_eq!(
        issues[2].source(),
        &flow_source_query(owner, HirFlowSourceRole::BodyClose)
    );
}

#[test]
fn flow_contract_poison_retains_each_missing_operand_owner_in_clause_order() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-contract-recovery-order",
        "flow recovered_contracts()\nrequires\nensures no_effect\n{}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);

    assert!(item.is_poisoned());
    assert_eq!(flow.contracts().len(), 2);
    let issues = flow
        .poison()
        .primary()
        .into_iter()
        .chain(flow.poison().related())
        .collect::<Vec<_>>();
    assert_eq!(issues.len(), 2);
    for (ordinal, issue) in issues.iter().enumerate() {
        let ordinal = u16::try_from(ordinal).unwrap();
        assert_eq!(issue.class(), HirFlowIssueClass::Contract);
        assert!(matches!(issue.owner(), HirFlowIssueOwner::Expr(_)));
        assert_eq!(
            issue.source(),
            &flow_source_query(
                owner,
                HirFlowSourceRole::ContractClause {
                    ordinal,
                    part: HirFlowContractSourcePart::Operand { ordinal: 0 },
                },
            )
        );
    }
}

#[test]
fn duplicate_decreases_keeps_later_keyword_primary_and_first_keyword_related() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-duplicate-decreases",
        "flow measure()\ndecreases first\ndecreases second\n{}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);

    assert!(item.is_poisoned());
    assert_eq!(flow.contracts().len(), 2);
    let primary = flow.poison().primary().unwrap();
    assert_eq!(primary.class(), HirFlowIssueClass::Contract);
    assert_eq!(
        primary.source(),
        &flow_source_query(
            owner,
            HirFlowSourceRole::ContractClause {
                ordinal: 1,
                part: HirFlowContractSourcePart::ClauseKeyword,
            },
        )
    );
    let [first] = flow.poison().related() else {
        panic!("duplicate decreases must retain the first keyword as related evidence")
    };
    assert_eq!(first.class(), HirFlowIssueClass::Contract);
    assert_eq!(
        first.source(),
        &flow_source_query(
            owner,
            HirFlowSourceRole::ContractClause {
                ordinal: 0,
                part: HirFlowContractSourcePart::ClauseKeyword,
            },
        )
    );
}

#[test]
fn flow_source_freeze_rejects_typed_component_substitution_and_retries_deterministically() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-source-freeze",
        "flow frozen {}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .expect("valid Flow lowers before source substitution");
    let [failed_owner] = transaction.staged_source_ordered_items() else {
        panic!("source-freeze fixture must stage one ordinary Flow")
    };
    let failed_owner = *failed_owner;
    let failed_snapshot = transaction.snapshot_id();

    let items = parsed.tree().items().unwrap();
    let [attached_flow @ TypedItemNode::Flow(_)] = items.as_slice() else {
        panic!("source-freeze fixture must retain one typed Flow item")
    };
    let query = flow_source_query(failed_owner, HirFlowSourceRole::Name);
    assert_eq!(
        transaction
            .source_components()
            .inject_component_for_test(&query, HirSourceSite::Span(attached_flow.source_span()),),
        Err(HirSourceCommitInvariantError::ConflictingComponent {
            query: query.clone(),
        })
    );
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());

    let mut retry = stage(&database, &parsed, &key);
    assert_eq!(retry.snapshot_id(), failed_snapshot);
    retry
        .lower_attached_source_file_items(&parsed.tree())
        .expect("valid Flow retry after rejected source substitution");
    assert_eq!(retry.staged_source_ordered_items(), [failed_owner]);
    let accepted = retry.finish(&mut database).unwrap().into_module();
    assert_eq!(
        flow_query(&accepted, &parsed, failed_owner, HirFlowSourceRole::Name).owner_status(),
        HirSourceOwnerStatus::Clean
    );
}

#[test]
fn flow_source_queries_reject_wrong_document_and_stale_revision() {
    let name = SourceName::path("proof/final-hir-flow-source-query.arcw");
    let document_id = "arcweft-test://proof/final-hir-flow-source-query";
    let source = "flow stable {}\n";
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(source.len(), source.len()))
                    .unwrap(),
                " ",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&revised);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &revised, &key);
    let owner = module.source_ordered_items()[0];
    let query = flow_source_query(owner, HirFlowSourceRole::Name);

    assert!(matches!(
        module.source_site(initial.document().identity(), query.clone()),
        Err(HirSourceQueryError::StaleSourceRevision { expected, actual })
            if expected == revised.document().identity().revision()
                && actual == initial.document().identity().revision()
    ));

    let foreign = parse(
        "arcweft-test://proof/final-hir-flow-source-query-foreign",
        source,
    );
    assert!(matches!(
        module.source_site(foreign.document().identity(), query),
        Err(HirSourceQueryError::WrongSourceDocument { expected, actual })
            if expected == *revised.document().identity().id()
                && actual == *foreign.document().identity().id()
    ));
}

#[test]
fn rejected_flow_revision_preserves_prior_publication_and_retry_identity() {
    let name = SourceName::path("proof/final-hir-flow-publication.arcw");
    let document_id = "arcweft-test://proof/final-hir-flow-publication";
    let source = "flow stable {}\n";
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial.document().span(SourceRange::new(0, 0)).unwrap(),
                " ",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    assert_eq!(module_key(&revised), key);

    let mut database = HirDatabase::try_new().unwrap();
    let prior = lower(&mut database, &initial, &key);
    let prior_owner = prior.source_ordered_items()[0];
    let prior_snapshot = prior.snapshot_id();
    let prior_epoch = prior.invalidation_epoch();
    let prior_name_site =
        match flow_query(&prior, &initial, prior_owner, HirFlowSourceRole::Name).presence() {
            HirSourcePresence::Present(site) => site.clone(),
            presence => panic!("accepted Flow name must be present, got {presence:?}"),
        };
    let before = database.test_state();

    let mut rejected = stage(&database, &revised, &key);
    rejected
        .lower_attached_source_file_items(&revised.tree())
        .expect("revised Flow lowers before source-manifest rejection");
    let [failed_owner] = rejected.staged_source_ordered_items() else {
        panic!("revised fixture must stage one ordinary Flow")
    };
    let failed_owner = *failed_owner;
    let failed_snapshot = rejected.snapshot_id();
    assert!(
        rejected
            .source_components()
            .remove_staged_query(&flow_source_query(failed_owner, HirFlowSourceRole::Name))
    );
    assert!(matches!(
        rejected.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));

    assert_eq!(database.test_state(), before);
    let retained = database.current(&key).expect("prior Flow remains current");
    assert!(Arc::ptr_eq(&retained, &prior));
    assert_eq!(retained.snapshot_id(), prior_snapshot);
    assert_eq!(retained.invalidation_epoch(), prior_epoch);
    assert_eq!(
        flow_query(&retained, &initial, prior_owner, HirFlowSourceRole::Name,).presence(),
        HirSourcePresence::Present(&prior_name_site)
    );
    assert!(matches!(
        flow_query_result(
            &retained,
            &revised,
            prior_owner,
            HirFlowSourceRole::Name,
        ),
        Err(HirSourceQueryError::StaleSourceRevision { expected, actual })
            if expected == initial.document().identity().revision()
                && actual == revised.document().identity().revision()
    ));

    let mut retry = stage(&database, &revised, &key);
    assert_eq!(retry.snapshot_id(), failed_snapshot);
    retry
        .lower_attached_source_file_items(&revised.tree())
        .expect("valid Flow retry after rejected publication");
    assert_eq!(retry.staged_source_ordered_items(), [failed_owner]);
    let output = retry.finish(&mut database).unwrap();
    assert_eq!(output.invalidations().previous(), Some(prior_snapshot));
    assert_eq!(output.invalidations().current(), failed_snapshot);
    assert!(output.invalidations().is_empty());
    assert_eq!(
        output.module().invalidation_epoch().get(),
        prior_epoch.get() + 1
    );
    let accepted = database.current(&key).expect("retried Flow is current");
    assert!(Arc::ptr_eq(&accepted, output.module()));
    assert_eq!(
        flow_query(&accepted, &revised, failed_owner, HirFlowSourceRole::Name,).owner_status(),
        HirSourceOwnerStatus::Clean
    );
}

fn thread_flow_matrix_body() -> &'static str {
    concat!(
        "    return unit\n",
        "    alice[こんにちは。]\n",
        "    choice {}\n",
        "    if ready {}\n",
        "    if let value = source {}\n",
        "    match value { _ => {} }\n",
        "    loop {}\n",
        "    while ready {}\n",
        "    while let value = source {}\n",
        "    for value in source {}\n",
        "    select {}\n",
        "    source locale en-US {}\n",
        "    scope local {}\n",
        "    include @flow.shared\n",
        "    await task with {}\n",
        "    ???\n",
    )
}
