use super::*;

use crate::expr::HirExprKind;
use crate::identity::LocalGeneration;
use crate::item::{
    HirActivityDeclaration, HirActivityLifecycle, HirActivityMode, HirActivityPortMember,
    HirDeclarationMember, HirDeclarationMemberIssue,
};

fn activity(
    module: &HirModule,
    ordinal: usize,
) -> (
    crate::identity::ItemId,
    &HirItem,
    &crate::item::HirActivityDeclaration,
) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Activity(activity) = item.kind() else {
        panic!("source-ordered item {ordinal} must be an Activity")
    };
    (owner, item, activity)
}

fn member(module: &HirModule, id: crate::item::HirDeclarationMemberId) -> &HirDeclarationMember {
    module.declaration_members().resolve(id).unwrap()
}

fn source_range<I: HirTypedId>(module: &HirModule, id: I) -> SourceRange {
    match module.slots().resolve(id).unwrap().source_site() {
        HirSourceSite::Span(span) => span.range(),
        HirSourceSite::Insertion(insertion) => {
            SourceRange::new(insertion.offset(), insertion.offset())
        }
    }
}

fn port(member: &HirDeclarationMember) -> &HirActivityPortMember {
    match member.kind() {
        HirDeclarationMemberKind::ActivityInput(port)
        | HirDeclarationMemberKind::ActivityOutput(port) => port,
        other => panic!("expected Activity port, got {other:?}"),
    }
}

fn local_named(
    module: &HirModule,
    activity: &crate::item::HirActivityDeclaration,
    expected: &str,
) -> crate::identity::LocalId {
    let scope = module
        .arenas()
        .scopes()
        .resolve(module.slots(), activity.scopes().callable())
        .unwrap();
    scope
        .locals()
        .iter()
        .copied()
        .find(|local| {
            module
                .arenas()
                .locals()
                .resolve(module.slots(), *local)
                .is_ok_and(|payload| payload.name().as_str() == expected)
        })
        .unwrap_or_else(|| panic!("missing Activity local `{expected}`"))
}

fn assert_activity_freeze_rejects(
    case: &str,
    source: &str,
    tamper: impl FnOnce(&mut StagedHirModuleTransaction<'_>, crate::identity::ItemId),
) {
    let parsed = parse(
        &format!("arcweft-test://proof/final-hir-activity-{case}"),
        source,
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_attached_source_file_items(&parsed.tree())
        .unwrap();
    let owner = transaction.source_ordered_items[0];
    tamper(&mut transaction, owner);
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());
}

#[test]
fn canonical_activity_freezes_exact_scopes_members_locals_contracts_and_sites() {
    let source = concat!(
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
    let parsed = parse("arcweft-test://proof/final-hir-activity-clean", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, activity) = activity(&module, 0);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(activity.mode(), HirActivityMode::CheckpointedRealtime);
    assert_eq!(activity.lifecycle(), HirActivityLifecycle::Snapshot);
    assert_eq!(item.members().len(), 3);
    assert_eq!(activity.inputs(), &item.members()[..2]);
    assert_eq!(activity.outputs(), &item.members()[2..]);
    let arena = module.declaration_members().arena(owner).unwrap();
    assert_eq!(arena.members().len(), 3);

    let scopes = activity.scopes();
    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), scopes.callable())
        .unwrap();
    let requires = module
        .arenas()
        .scopes()
        .resolve(module.slots(), scopes.requires())
        .unwrap();
    let ensures = module
        .arenas()
        .scopes()
        .resolve(module.slots(), scopes.ensures())
        .unwrap();
    assert_eq!(callable.kind(), HirScopeKind::Callable);
    assert_eq!(callable.parent(), Some(item.scope()));
    assert_eq!(callable.owner(), &HirScopeOwner::Item(owner));
    assert_eq!(callable.children(), [scopes.requires(), scopes.ensures()]);
    assert_eq!(requires.kind(), HirScopeKind::ContractRequires);
    assert_eq!(requires.parent(), Some(scopes.callable()));
    assert_eq!(ensures.kind(), HirScopeKind::ContractEnsures);
    assert_eq!(ensures.parent(), Some(scopes.callable()));
    assert!(requires.locals().is_empty());
    assert!(ensures.locals().is_empty());
    let requires_start = source.find("requires").unwrap();
    let ensures_start = source.find("ensures").unwrap();
    assert_eq!(
        source_range(&module, scopes.requires()),
        SourceRange::new(requires_start, requires_start)
    );
    assert_eq!(
        source_range(&module, scopes.ensures()),
        SourceRange::new(ensures_start, ensures_start)
    );

    assert_eq!(callable.locals().len(), 3);
    for (position, member_id) in item.members().iter().copied().enumerate() {
        assert_eq!(member_id.ordinal(), u32::try_from(position).unwrap());
        let member = member(&module, member_id);
        assert_eq!(member.state(), HirDeclarationMemberPoisonState::Clean);
        let port = port(member);
        let local = port.local().expect("resolved Activity port local");
        let local_payload = module
            .arenas()
            .locals()
            .resolve(module.slots(), local)
            .unwrap();
        assert_eq!(local_payload.scope(), scopes.callable());
        assert_eq!(local_payload.kind(), HirLocalKind::Parameter);
        assert_eq!(local_payload.name(), port.name().resolved().unwrap());
        assert_eq!(
            local_payload.generation(),
            LocalGeneration::try_new(1).unwrap()
        );
        assert_eq!(local_payload.annotation(), Some(port.ty()));
        assert!(local_payload.pattern().is_none());
        assert!(!local_payload.is_mutable_binding());
        assert!(!local_payload.is_poisoned());
        assert_eq!(callable.locals()[position], local);
    }

    assert_eq!(activity.requires().len(), 1);
    assert_eq!(activity.ensures().len(), 1);
    for (expression, expected_scope) in [
        (activity.requires()[0], scopes.requires()),
        (activity.ensures()[0], scopes.ensures()),
    ] {
        assert_eq!(
            module
                .arenas()
                .expressions()
                .resolve(module.slots(), expression)
                .unwrap()
                .scope(),
            expected_scope
        );
        assert_source_backed_child(&module, expression);
    }
    assert_item_slot_whole(&module, &parsed, owner);
}

#[test]
fn empty_activity_uses_clean_defaults_and_header_end_contract_scope_fallbacks() {
    let source = "activity MiniGame {}\n";
    let parsed = parse("arcweft-test://proof/final-hir-activity-defaults", source);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, activity) = activity(&module, 0);

    assert_eq!(item.state(), &HirItemPoisonState::Clean);
    assert_eq!(activity.mode(), HirActivityMode::Deterministic);
    assert_eq!(activity.lifecycle(), HirActivityLifecycle::Stateless);
    assert!(item.members().is_empty());
    assert!(activity.inputs().is_empty());
    assert!(activity.outputs().is_empty());
    assert!(activity.requires().is_empty());
    assert!(activity.ensures().is_empty());
    assert!(module.declaration_members().arena(owner).is_none());

    let header_end = source.find('{').unwrap();
    let scopes = activity.scopes();
    assert_eq!(
        source_range(&module, scopes.requires()),
        SourceRange::new(header_end, header_end)
    );
    assert_eq!(
        source_range(&module, scopes.ensures()),
        SourceRange::new(header_end, header_end)
    );
    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), scopes.callable())
        .unwrap();
    assert!(callable.locals().is_empty());
}

#[test]
fn recovered_activity_retains_global_port_order_poisoned_duplicate_and_missing_condition() {
    let source = concat!(
        "activity Broken {\n",
        "    output {\n",
        "        shared: Result\n",
        "    }\n",
        "    input {\n",
        "        shared: Input\n",
        "    }\n",
        "    contract {\n",
        "        ensures true\n",
        "        requires\n",
        "    }\n",
        "}\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-activity-recovery", source);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_owner, item, activity) = activity(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::InvalidMember)
    );
    assert_eq!(activity.outputs(), &item.members()[..1]);
    assert_eq!(activity.inputs(), &item.members()[1..]);
    let first = member(&module, item.members()[0]);
    let duplicate = member(&module, item.members()[1]);
    assert_eq!(first.state(), HirDeclarationMemberPoisonState::Clean);
    assert_eq!(
        duplicate.state(),
        HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::Duplicate)
    );
    let first_local = port(first).local().unwrap();
    let duplicate_local = port(duplicate).local().unwrap();
    let first_local = module
        .arenas()
        .locals()
        .resolve(module.slots(), first_local)
        .unwrap();
    let duplicate_local = module
        .arenas()
        .locals()
        .resolve(module.slots(), duplicate_local)
        .unwrap();
    assert_eq!(
        first_local.generation(),
        LocalGeneration::try_new(1).unwrap()
    );
    assert_eq!(
        duplicate_local.generation(),
        LocalGeneration::try_new(2).unwrap()
    );
    assert!(!first_local.is_poisoned());
    assert!(duplicate_local.is_poisoned());

    assert_eq!(activity.requires().len(), 1);
    assert_eq!(activity.ensures().len(), 1);
    let missing = activity.requires()[0];
    let missing_payload = module
        .arenas()
        .expressions()
        .resolve(module.slots(), missing)
        .unwrap();
    assert_eq!(missing_payload.scope(), activity.scopes().requires());
    assert!(matches!(missing_payload.kind(), HirExprKind::Error(_)));
    assert!(module.slots().resolve(missing).unwrap().is_poisoned());
    assert_source_backed_child(&module, missing);

    let requires_start = source.find("requires").unwrap();
    let ensures_start = source.find("ensures").unwrap();
    assert_eq!(
        source_range(&module, activity.scopes().requires()),
        SourceRange::new(requires_start, requires_start)
    );
    assert_eq!(
        source_range(&module, activity.scopes().ensures()),
        SourceRange::new(ensures_start, ensures_start)
    );
}

#[test]
fn activity_freeze_rejects_policy_direction_and_contract_scope_tampering() {
    let source = concat!(
        "activity Guarded {\n",
        "    mode = deterministic\n",
        "    input { alpha: I32 }\n",
        "    output { omega: I64 }\n",
        "    contract { requires true ensures true }\n",
        "}\n",
    );

    assert_activity_freeze_rejects("mode-tamper", source, |transaction, owner| {
        let (slots, arenas) = transaction.storage_mut();
        let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
        let HirItemKind::Activity(activity) = original.kind() else {
            panic!("final Activity item")
        };
        let replacement_activity = HirActivityDeclaration::try_new(
            owner,
            activity.header().clone(),
            activity.scopes(),
            HirActivityMode::ExternalRealtime,
            activity.lifecycle(),
            activity.inputs().into(),
            activity.outputs().into(),
            activity.requires().into(),
            activity.ensures().into(),
        )
        .unwrap();
        let replacement = HirItem::try_new_with_state(
            owner,
            original.scope(),
            original.prefix().clone(),
            HirItemKind::Activity(replacement_activity),
            original.members().into(),
            *original.state(),
        )
        .unwrap();
        arenas
            .items()
            .revise_finalized(slots, owner, replacement)
            .unwrap();
    });

    assert_activity_freeze_rejects("direction-tamper", source, |transaction, owner| {
        let (slots, arenas) = transaction.storage_mut();
        let original = arenas.items().resolve_staged(slots, owner).unwrap().clone();
        let HirItemKind::Activity(activity) = original.kind() else {
            panic!("final Activity item")
        };
        let replacement_activity = HirActivityDeclaration::try_new(
            owner,
            activity.header().clone(),
            activity.scopes(),
            activity.mode(),
            activity.lifecycle(),
            activity.outputs().into(),
            activity.inputs().into(),
            activity.requires().into(),
            activity.ensures().into(),
        )
        .unwrap();
        let replacement = HirItem::try_new_with_state(
            owner,
            original.scope(),
            original.prefix().clone(),
            HirItemKind::Activity(replacement_activity),
            original.members().into(),
            *original.state(),
        )
        .unwrap();
        arenas
            .items()
            .revise_finalized(slots, owner, replacement)
            .unwrap();
    });

    assert_activity_freeze_rejects("contract-scope-tamper", source, |transaction, owner| {
        let (slots, arenas) = transaction.storage_mut();
        let item = arenas.items().resolve_staged(slots, owner).unwrap();
        let HirItemKind::Activity(activity) = item.kind() else {
            panic!("final Activity item")
        };
        let requires_scope = activity.scopes().requires();
        let original = arenas
            .scopes()
            .resolve_staged(slots, requires_scope)
            .unwrap()
            .clone();
        let replacement = HirScope::try_new(
            requires_scope.module(),
            HirScopeKind::Block,
            original.parent(),
            *original.owner(),
            original.children().into(),
            original.locals().into(),
        )
        .unwrap();
        arenas
            .scopes()
            .revise_finalized(slots, requires_scope, replacement)
            .unwrap();
    });
}

#[test]
fn incremental_activity_preserves_reconciled_owners_and_retires_replaced_port_sources() {
    let name = SourceName::path("proof/activity-incremental.arcw");
    let document_id = "arcweft-test://proof/activity-incremental";
    let initial_source = concat!(
        "activity First {\n",
        "    input {\n",
        "        alpha: I32\n",
        "        beta: I64\n",
        "    }\n",
        "    contract { requires true }\n",
        "}\n",
        "activity Second {}\n",
    );
    let reordered_source = concat!(
        "activity Second {}\n",
        "activity Inserted {}\n",
        "activity First {\n",
        "    input {\n",
        "        alpha: I32\n",
        "        beta: I64\n",
        "    }\n",
        "    contract { requires true }\n",
        "}\n",
    );
    let modified_source = concat!(
        "activity Second {}\n",
        "activity Inserted {}\n",
        "activity First {\n",
        "    input {\n",
        "        beta: I64\n",
        "        gamma: I16\n",
        "        alpha: I32\n",
        "    }\n",
        "    contract { requires true }\n",
        "}\n",
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, initial_source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let mut database = HirDatabase::try_new().unwrap();
    let first = lower(&mut database, &initial, &key);
    let (first_owner, _, first_activity) = activity(&first, 0);
    let (second_owner, _, second_activity) = activity(&first, 1);
    let first_scope = first_activity.scopes().callable();
    let second_scope = second_activity.scopes().callable();
    let alpha = local_named(&first, first_activity, "alpha");
    let beta = local_named(&first, first_activity, "beta");

    let reordered = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(0, initial_source.len()))
                    .unwrap(),
                reordered_source,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second = lower(&mut database, &reordered, &key);
    let (reordered_second, _, reordered_second_activity) = activity(&second, 0);
    let (inserted_owner, _, inserted_activity) = activity(&second, 1);
    let (reordered_first, reordered_first_item, reordered_first_activity) = activity(&second, 2);
    assert_eq!(reordered_second, second_owner);
    assert_eq!(reordered_first, first_owner);
    assert_eq!(reordered_second_activity.scopes().callable(), second_scope);
    assert_eq!(reordered_first_activity.scopes().callable(), first_scope);
    assert_ne!(inserted_activity.scopes().callable(), first_scope);
    assert_ne!(inserted_activity.scopes().callable(), second_scope);
    assert_eq!(
        local_named(&second, reordered_first_activity, "alpha"),
        alpha
    );
    assert_eq!(local_named(&second, reordered_first_activity, "beta"), beta);
    assert!(
        reordered_first_item
            .members()
            .iter()
            .copied()
            .enumerate()
            .all(|(position, id)| id.item() == first_owner
                && id.ordinal() == u32::try_from(position).unwrap())
    );

    let old_ports = "        alpha: I32\n        beta: I64\n";
    let new_ports = "        beta: I64\n        gamma: I16\n        alpha: I32\n";
    let port_start = reordered_source.find(old_ports).unwrap();
    let modified = syntax
        .reparse(
            &reordered,
            &[SourceEdit::new(
                reordered
                    .document()
                    .span(SourceRange::new(port_start, port_start + old_ports.len()))
                    .unwrap(),
                new_ports,
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    assert_eq!(modified.document().text(), modified_source);
    let third = lower(&mut database, &modified, &key);
    let (modified_first, modified_first_item, modified_first_activity) = activity(&third, 2);
    assert_eq!(modified_first, first_owner);
    assert_eq!(modified_first_activity.scopes().callable(), first_scope);
    let modified_alpha = local_named(&third, modified_first_activity, "alpha");
    let modified_beta = local_named(&third, modified_first_activity, "beta");
    let gamma = local_named(&third, modified_first_activity, "gamma");
    assert_ne!(modified_alpha, alpha);
    assert_ne!(modified_beta, beta);
    assert_ne!(modified_alpha, modified_beta);
    assert_ne!(modified_alpha, gamma);
    assert_ne!(modified_beta, gamma);
    for local in [modified_beta, gamma, modified_alpha] {
        let payload = third
            .arenas()
            .locals()
            .resolve(third.slots(), local)
            .unwrap();
        assert_eq!(payload.generation(), LocalGeneration::try_new(1).unwrap());
        assert_source_backed_child(&third, local);
    }
    for replaced in [alpha, beta] {
        assert!(
            third
                .arenas()
                .locals()
                .resolve(third.slots(), replaced)
                .is_err()
        );
    }
    assert!(
        modified_first_item
            .members()
            .iter()
            .copied()
            .enumerate()
            .all(|(position, id)| id.item() == first_owner
                && id.ordinal() == u32::try_from(position).unwrap())
    );

    let second_line = "activity Second {}\n";
    let beta_line = "        beta: I64\n";
    let beta_start = modified_source.find(beta_line).unwrap();
    let deleted = syntax
        .reparse(
            &modified,
            &[
                SourceEdit::new(
                    modified
                        .document()
                        .span(SourceRange::new(0, second_line.len()))
                        .unwrap(),
                    "",
                ),
                SourceEdit::new(
                    modified
                        .document()
                        .span(SourceRange::new(beta_start, beta_start + beta_line.len()))
                        .unwrap(),
                    "",
                ),
            ],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let fourth = lower(&mut database, &deleted, &key);
    let (remaining_inserted, _, _) = activity(&fourth, 0);
    let (remaining_first, _, remaining_first_activity) = activity(&fourth, 1);
    assert_eq!(remaining_inserted, inserted_owner);
    assert_eq!(remaining_first, first_owner);
    assert_eq!(remaining_first_activity.scopes().callable(), first_scope);
    let remaining_alpha = local_named(&fourth, remaining_first_activity, "alpha");
    let remaining_gamma = local_named(&fourth, remaining_first_activity, "gamma");
    assert_ne!(remaining_alpha, modified_alpha);
    assert_ne!(remaining_gamma, gamma);
    assert_ne!(remaining_alpha, remaining_gamma);
    for local in [remaining_gamma, remaining_alpha] {
        let payload = fourth
            .arenas()
            .locals()
            .resolve(fourth.slots(), local)
            .unwrap();
        assert_eq!(payload.generation(), LocalGeneration::try_new(1).unwrap());
        assert_source_backed_child(&fourth, local);
    }
    for replaced in [modified_alpha, modified_beta, gamma] {
        assert!(
            fourth
                .arenas()
                .locals()
                .resolve(fourth.slots(), replaced)
                .is_err()
        );
    }
    assert!(
        fourth
            .arenas()
            .items()
            .resolve(fourth.slots(), second_owner)
            .is_err()
    );

    assert_eq!(first.source_ordered_items(), [first_owner, second_owner]);
}
