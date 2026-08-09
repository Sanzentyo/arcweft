use core::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::attachment::source_file::SourceFileEntryNode;
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};

use crate::arena::{ArenaSnapshot, StagedArena};
use crate::identity::{HirLimit, HirModuleId, HirRevision, HirSnapshotId, ItemId, ScopeId};
use crate::item::{
    HirDeclarationMemberIndexBuilder, HirItem, HirItemKind, HirItemPrefix, HirModuleDeclaration,
};
use crate::leaf::{HirName, HirPath, HirPathRoot, HirPathSegment, HirPathValue};
use crate::lowering::{HirInvariantFailure, HirLimitError, HirLowerFailure, HirModuleKey};
use crate::module::{HirModule, HirModuleArenaParts, HirModuleArenas, HirModuleStatus};
use crate::scope::{HirScope, HirScopeKind, HirScopeOwner};
use crate::slot::{PreparedSlotCommit, StagedSlotTransaction};
use crate::source_index::{HirSourceIndex, HirSourceSite};
use crate::symbol::CallablePackageId;

use super::{HirDatabase, HirSnapshotLookupError, StagedModuleCommit};

fn module_key(document: &str) -> HirModuleKey {
    HirModuleKey::new(
        CallablePackageId::try_new("proof-database-tests").unwrap(),
        CanonicalModulePath::crate_root(),
        SourceDocumentId::try_new(document).unwrap(),
    )
}

fn validated(plan: &StagedModuleCommit) -> Arc<HirModule> {
    Arc::new(HirModule::from_validated_test(
        plan.snapshot_id(),
        plan.key().clone(),
        plan.invalidation_epoch(),
    ))
}

fn stage_and_commit(database: &mut HirDatabase, key: &HirModuleKey) -> Arc<HirModule> {
    let plan = database.stage_module(key).unwrap();
    let module = validated(&plan);
    database.commit_module(plan, module).unwrap()
}

fn publish_empty_slots(database: &mut HirDatabase, key: &HirModuleKey) -> Arc<HirModule> {
    let plan = database.stage_module(key).unwrap();
    let previous = plan.previous().cloned();
    let staged = match previous.as_ref() {
        Some(previous) => StagedSlotTransaction::from_snapshot(previous.slots(), plan.revision()),
        None => StagedSlotTransaction::new(plan.module_id(), plan.revision()),
    };
    let prepared = staged.prepare().unwrap();
    let module = Arc::new(
        HirModule::from_validated_parts(
            plan.snapshot_id(),
            plan.key().clone(),
            HirModuleStatus::Clean,
            Arc::clone(prepared.snapshot()),
            plan.invalidation_epoch(),
        )
        .unwrap(),
    );
    database
        .publish_module(plan, prepared, Arc::clone(&module))
        .unwrap()
        .into_module()
}

fn parsed_source_with(source: &str) -> ParsedSource {
    let name = SourceName::path("proof/database-atomic.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://proof/database-atomic").unwrap(),
            name.clone(),
            source,
        )
        .unwrap(),
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap()
}

fn parsed_source() -> ParsedSource {
    parsed_source_with("mod main\n")
}

fn parsed_source_with_trivia_revision() -> (ParsedSource, ParsedSource) {
    let name = SourceName::path("proof/database-atomic.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://proof/database-atomic").unwrap(),
            name.clone(),
            "mod main\n",
        )
        .unwrap(),
    );
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            document,
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let trivia = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial.document().span(SourceRange::new(0, 0)).unwrap(),
                " ",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    assert_eq!(initial.root_syntax().id(), trivia.root_syntax().id());
    assert_ne!(initial.document().identity(), trivia.document().identity());
    (initial, trivia)
}

fn item_path(name: &str) -> HirPath {
    HirPath::try_new(
        HirPathRoot::ImplicitCrate,
        Box::new([HirPathSegment::Identifier(
            HirName::try_new(name.into()).unwrap(),
        )]),
    )
    .unwrap()
}

fn stage_item_module(
    plan: &StagedModuleCommit,
    parsed: &ParsedSource,
    item_name: Option<&str>,
) -> (PreparedSlotCommit, Arc<HirModule>, Option<ItemId>) {
    let mut slots = match plan.previous() {
        Some(previous) => StagedSlotTransaction::from_snapshot(previous.slots(), plan.revision()),
        None => StagedSlotTransaction::new(plan.module_id(), plan.revision()),
    };
    let mut items = match plan.previous() {
        Some(previous) => StagedArena::from_snapshot(previous.arenas().items()),
        None => StagedArena::new(),
    };
    let mut scopes = match plan.previous() {
        Some(previous) => StagedArena::from_snapshot(previous.arenas().scopes()),
        None => StagedArena::new(),
    };
    let attached_item = parsed
        .entries()
        .unwrap()
        .into_iter()
        .find(|entry| !matches!(entry, SourceFileEntryNode::Attribute(_)))
        .unwrap();
    let source_site = HirSourceSite::Span(attached_item.source_span());
    let syntax = attached_item.id();
    let scope = scopes
        .allocate_source(
            &mut slots,
            syntax,
            source_site.clone(),
            HirScope::try_new(
                plan.module_id(),
                HirScopeKind::Module,
                None,
                HirScopeOwner::Module(plan.module_id()),
                Box::new([]),
                Box::new([]),
            )
            .unwrap(),
        )
        .unwrap();
    let item = item_name.map(|name| {
        let reservation = items
            .reserve_source(&mut slots, syntax, source_site.clone())
            .unwrap();
        let id = reservation.id();
        items
            .finalize(
                &mut slots,
                reservation,
                HirItem::try_new(
                    id,
                    scope,
                    HirItemPrefix::new(None, Box::new([]), None),
                    HirItemKind::Module(HirModuleDeclaration::new(HirPathValue::Resolved(
                        item_path(name),
                    ))),
                    Box::new([]),
                )
                .unwrap(),
            )
            .unwrap()
    });
    slots.retire_untouched().unwrap();
    let items = items.into_snapshot(&mut slots).unwrap();
    let scopes = scopes.into_snapshot(&mut slots).unwrap();
    let prepared = slots.prepare().unwrap();
    let arenas = HirModuleArenas::try_new(
        prepared.snapshot(),
        HirModuleArenaParts {
            items,
            scopes,
            locals: ArenaSnapshot::empty(prepared.snapshot()),
            expressions: ArenaSnapshot::empty(prepared.snapshot()),
            statements: ArenaSnapshot::empty(prepared.snapshot()),
            types: ArenaSnapshot::empty(prepared.snapshot()),
            patterns: ArenaSnapshot::empty(prepared.snapshot()),
            captures: ArenaSnapshot::empty(prepared.snapshot()),
        },
    )
    .unwrap();
    let diagnostics = parsed
        .diagnostics()
        .iter()
        .cloned()
        .map(crate::diagnostic::HirDiagnostic::Syntax)
        .collect::<Vec<_>>()
        .into();
    let module = Arc::new(
        HirModule::try_new(
            plan.snapshot_id(),
            plan.key().clone(),
            parsed,
            diagnostics,
            Arc::clone(prepared.snapshot()),
            arenas,
            item.iter().copied().collect::<Vec<_>>().into_boxed_slice(),
            HirDeclarationMemberIndexBuilder::new(plan.module_id()).freeze(),
            HirSourceIndex::empty(parsed.document().identity().clone(), prepared.snapshot()),
            plan.invalidation_epoch(),
        )
        .unwrap(),
    );
    (prepared, module, item)
}

#[test]
fn database_identities_are_process_local_and_distinct() {
    let first = HirDatabase::try_new().unwrap();
    let second = HirDatabase::try_new().unwrap();
    assert_ne!(first.database_id(), second.database_id());
}

#[test]
fn dropped_stage_consumes_neither_module_slot_nor_revision() {
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcw:/proof/database-stage");
    let dropped = database.stage_module(&key).unwrap();
    let expected_snapshot = dropped.snapshot_id();
    drop(dropped);

    let committed_plan = database.stage_module(&key).unwrap();
    assert_eq!(committed_plan.snapshot_id(), expected_snapshot);
    let module = validated(&committed_plan);
    let committed = database.commit_module(committed_plan, module).unwrap();
    assert!(Arc::ptr_eq(&committed, &database.current(&key).unwrap()));
    assert!(Arc::ptr_eq(
        &committed,
        &database.snapshot(expected_snapshot).unwrap()
    ));
}

#[test]
fn successful_revision_publish_retains_both_exact_arc_leases() {
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcw:/proof/database-revisions");
    let first = stage_and_commit(&mut database, &key);
    let dropped = database.stage_module(&key).unwrap();
    let expected_revision = dropped.revision();
    drop(dropped);
    let plan = database.stage_module(&key).unwrap();
    assert_eq!(plan.module_id(), first.snapshot_id().module());
    assert_eq!(plan.revision(), expected_revision);
    assert_eq!(
        plan.revision(),
        first.snapshot_id().revision().checked_next().unwrap()
    );
    assert_eq!(
        plan.invalidation_epoch().get(),
        first.invalidation_epoch().get() + 1
    );
    let module = validated(&plan);
    let second = database.commit_module(plan, module).unwrap();

    assert!(Arc::ptr_eq(&second, &database.current(&key).unwrap()));
    assert!(Arc::ptr_eq(
        &first,
        &database.snapshot(first.snapshot_id()).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &second,
        &database.snapshot(second.snapshot_id()).unwrap()
    ));
}

#[test]
fn new_item_publication_derives_the_exact_changed_inventory() {
    let parsed = parsed_source();
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcweft-test://proof/database-atomic");
    let plan = database.stage_module(&key).unwrap();
    let (prepared, module, item) = stage_item_module(&plan, &parsed, Some("first"));
    let item = item.unwrap();

    let output = database
        .publish_module(plan, prepared, Arc::clone(&module))
        .unwrap();

    assert!(Arc::ptr_eq(output.module(), &module));
    assert_eq!(output.invalidations().module(), module.module_id());
    assert_eq!(output.invalidations().previous(), None);
    assert_eq!(output.invalidations().current(), module.snapshot_id());
    assert_eq!(output.invalidations().changed_items(), [item]);
    assert!(output.invalidations().retired_items().is_empty());
    assert!(output.invalidations().symbol_revision_changed());
    assert!(!output.invalidations().executable_status_changed());
}

#[test]
fn item_equal_revision_is_empty_and_payload_update_is_exactly_changed() {
    let (parsed, trivia) = parsed_source_with_trivia_revision();
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcweft-test://proof/database-atomic");

    let first_plan = database.stage_module(&key).unwrap();
    let (first_prepared, first_module, first_item) =
        stage_item_module(&first_plan, &parsed, Some("first"));
    let first_item = first_item.unwrap();
    let first = database
        .publish_module(first_plan, first_prepared, first_module)
        .unwrap()
        .into_module();

    let equal_plan = database.stage_module(&key).unwrap();
    let (equal_prepared, equal_module, equal_item) =
        stage_item_module(&equal_plan, &trivia, Some("first"));
    assert_eq!(equal_item, Some(first_item));
    let equal = database
        .publish_module(equal_plan, equal_prepared, equal_module)
        .unwrap();
    assert_eq!(equal.invalidations().previous(), Some(first.snapshot_id()));
    assert!(equal.invalidations().is_empty());

    let changed_plan = database.stage_module(&key).unwrap();
    let previous = changed_plan.previous().unwrap().snapshot_id();
    let (changed_prepared, changed_module, changed_item) =
        stage_item_module(&changed_plan, &trivia, Some("second"));
    assert_eq!(changed_item, Some(first_item));
    let changed = database
        .publish_module(changed_plan, changed_prepared, changed_module)
        .unwrap();

    assert_eq!(changed.invalidations().previous(), Some(previous));
    assert_eq!(changed.invalidations().changed_items(), [first_item]);
    assert!(changed.invalidations().retired_items().is_empty());
    assert!(changed.invalidations().symbol_revision_changed());
    assert!(!changed.invalidations().executable_status_changed());
}

#[test]
fn item_retirement_is_derived_without_caller_facts() {
    let parsed = parsed_source();
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcweft-test://proof/database-atomic");

    let first_plan = database.stage_module(&key).unwrap();
    let (first_prepared, first_module, item) =
        stage_item_module(&first_plan, &parsed, Some("retired"));
    let item = item.unwrap();
    database
        .publish_module(first_plan, first_prepared, first_module)
        .unwrap();

    let retired_plan = database.stage_module(&key).unwrap();
    let (retired_prepared, retired_module, current_item) =
        stage_item_module(&retired_plan, &parsed, None);
    assert_eq!(current_item, None);
    let retired = database
        .publish_module(retired_plan, retired_prepared, retired_module)
        .unwrap();

    assert!(retired.invalidations().changed_items().is_empty());
    assert_eq!(retired.invalidations().retired_items(), [item]);
    assert!(retired.invalidations().symbol_revision_changed());
    assert!(!retired.invalidations().executable_status_changed());
}

#[test]
fn status_only_transition_invalidates_execution_and_symbols() {
    let clean = parsed_source();
    let recovered = parsed_source_with("fn {\n");
    assert!(!recovered.diagnostics().is_empty());
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcweft-test://proof/database-atomic");

    let clean_plan = database.stage_module(&key).unwrap();
    let (clean_prepared, clean_module, _) = stage_item_module(&clean_plan, &clean, None);
    database
        .publish_module(clean_plan, clean_prepared, clean_module)
        .unwrap();

    let recovered_plan = database.stage_module(&key).unwrap();
    let (recovered_prepared, recovered_module, _) =
        stage_item_module(&recovered_plan, &recovered, None);
    assert_eq!(recovered_module.status(), HirModuleStatus::Recovered);
    let output = database
        .publish_module(recovered_plan, recovered_prepared, recovered_module)
        .unwrap();

    assert!(output.invalidations().changed_items().is_empty());
    assert!(output.invalidations().retired_items().is_empty());
    assert!(output.invalidations().executable_status_changed());
    assert!(output.invalidations().symbol_revision_changed());
}

#[test]
fn database_publishes_the_exact_prepared_slot_and_module_leases_together() {
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcw:/proof/database-atomic-success");
    let plan = database.stage_module(&key).unwrap();
    let prepared = StagedSlotTransaction::new(plan.module_id(), plan.revision())
        .prepare()
        .unwrap();
    let module = Arc::new(
        HirModule::from_validated_parts(
            plan.snapshot_id(),
            plan.key().clone(),
            HirModuleStatus::Clean,
            Arc::clone(prepared.snapshot()),
            plan.invalidation_epoch(),
        )
        .unwrap(),
    );
    let output = database
        .publish_module(plan, prepared, Arc::clone(&module))
        .unwrap();

    assert!(Arc::ptr_eq(output.module(), &module));
    assert!(Arc::ptr_eq(&database.current(&key).unwrap(), &module));
    assert!(Arc::ptr_eq(
        module.slots(),
        database.current(&key).unwrap().slots()
    ));
}

#[test]
fn update_publication_requires_the_exact_current_slot_ancestry() {
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcw:/proof/database-slot-ancestry");

    let first_plan = database.stage_module(&key).unwrap();
    let first_prepared = StagedSlotTransaction::new(first_plan.module_id(), first_plan.revision())
        .prepare()
        .unwrap();
    let first_module = Arc::new(
        HirModule::from_validated_parts(
            first_plan.snapshot_id(),
            first_plan.key().clone(),
            HirModuleStatus::Clean,
            Arc::clone(first_prepared.snapshot()),
            first_plan.invalidation_epoch(),
        )
        .unwrap(),
    );
    let first = database
        .publish_module(first_plan, first_prepared, Arc::clone(&first_module))
        .unwrap()
        .into_module();

    let rejected_plan = database.stage_module(&key).unwrap();
    let unrelated = StagedSlotTransaction::new(rejected_plan.module_id(), rejected_plan.revision())
        .prepare()
        .unwrap();
    let unrelated_slots = Arc::clone(unrelated.snapshot());
    let rejected_module = Arc::new(
        HirModule::from_validated_parts(
            rejected_plan.snapshot_id(),
            rejected_plan.key().clone(),
            HirModuleStatus::Clean,
            Arc::clone(unrelated.snapshot()),
            rejected_plan.invalidation_epoch(),
        )
        .unwrap(),
    );
    assert!(matches!(
        database.publish_module(rejected_plan, unrelated, rejected_module),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleCommit
        ))
    ));
    assert_eq!(unrelated_slots.committed_slot_count(), 0);
    assert!(Arc::ptr_eq(&database.current(&key).unwrap(), &first));

    let accepted_plan = database.stage_module(&key).unwrap();
    let accepted_prepared =
        StagedSlotTransaction::from_snapshot(first.slots(), accepted_plan.revision())
            .prepare()
            .unwrap();
    let accepted_module = Arc::new(
        HirModule::from_validated_parts(
            accepted_plan.snapshot_id(),
            accepted_plan.key().clone(),
            HirModuleStatus::Clean,
            Arc::clone(accepted_prepared.snapshot()),
            accepted_plan.invalidation_epoch(),
        )
        .unwrap(),
    );
    let accepted = database
        .publish_module(
            accepted_plan,
            accepted_prepared,
            Arc::clone(&accepted_module),
        )
        .unwrap()
        .into_module();
    assert!(Arc::ptr_eq(&database.current(&key).unwrap(), &accepted));
}

#[test]
fn stale_slot_snapshot_cannot_publish_through_a_newer_module_plan() {
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcw:/proof/database-stale-slot-base");

    let first = publish_empty_slots(&mut database, &key);
    let second = publish_empty_slots(&mut database, &key);
    let plan = database.stage_module(&key).unwrap();
    let stale = StagedSlotTransaction::from_snapshot(first.slots(), plan.revision())
        .prepare()
        .unwrap();
    let stale_slots = Arc::clone(stale.snapshot());
    let module = Arc::new(
        HirModule::from_validated_parts(
            plan.snapshot_id(),
            plan.key().clone(),
            HirModuleStatus::Clean,
            Arc::clone(stale.snapshot()),
            plan.invalidation_epoch(),
        )
        .unwrap(),
    );
    assert!(matches!(
        database.publish_module(plan, stale, module),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleCommit
        ))
    ));
    assert_eq!(stale_slots.committed_slot_count(), 0);
    assert!(Arc::ptr_eq(&database.current(&key).unwrap(), &second));
}

#[test]
fn competing_plans_from_one_current_arc_publish_only_the_first() {
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcw:/proof/database-competing-plans");
    let first = publish_empty_slots(&mut database, &key);

    let plan_a = database.stage_module(&key).unwrap();
    let plan_b = database.stage_module(&key).unwrap();
    assert!(Arc::ptr_eq(plan_a.previous().unwrap(), &first));
    assert!(Arc::ptr_eq(plan_b.previous().unwrap(), &first));
    assert_eq!(plan_a.snapshot_id(), plan_b.snapshot_id());
    assert_eq!(plan_a.invalidation_epoch(), plan_b.invalidation_epoch());

    let prepared_a = StagedSlotTransaction::from_snapshot(first.slots(), plan_a.revision())
        .prepare()
        .unwrap();
    let module_a = Arc::new(
        HirModule::from_validated_parts(
            plan_a.snapshot_id(),
            plan_a.key().clone(),
            HirModuleStatus::Clean,
            Arc::clone(prepared_a.snapshot()),
            plan_a.invalidation_epoch(),
        )
        .unwrap(),
    );
    let prepared_b = StagedSlotTransaction::from_snapshot(first.slots(), plan_b.revision())
        .prepare()
        .unwrap();
    let observable_b_slots = Arc::clone(prepared_b.snapshot());
    let module_b = Arc::new(
        HirModule::from_validated_parts(
            plan_b.snapshot_id(),
            plan_b.key().clone(),
            HirModuleStatus::Clean,
            Arc::clone(prepared_b.snapshot()),
            plan_b.invalidation_epoch(),
        )
        .unwrap(),
    );
    let accepted_output = database
        .publish_module(plan_a, prepared_a, Arc::clone(&module_a))
        .unwrap();
    assert!(accepted_output.invalidations().is_empty());
    let accepted = accepted_output.into_module();
    assert!(Arc::ptr_eq(&accepted, &module_a));

    assert!(matches!(
        database.publish_module(plan_b, prepared_b, module_b),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleCommit
        ))
    ));
    assert_eq!(observable_b_slots.committed_slot_count(), 0);
    assert!(Arc::ptr_eq(&database.current(&key).unwrap(), &accepted));
    assert_eq!(
        database.current(&key).unwrap().snapshot_id(),
        accepted.snapshot_id()
    );
    assert_eq!(
        database.current(&key).unwrap().invalidation_epoch(),
        accepted.invalidation_epoch()
    );
}

#[test]
fn non_item_arena_change_does_not_fabricate_item_invalidations() {
    let parsed = parsed_source();
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcweft-test://proof/database-atomic");
    let plan = database.stage_module(&key).unwrap();
    let mut slots = StagedSlotTransaction::new(plan.module_id(), plan.revision());
    let mut scopes = StagedArena::<HirScope, ScopeId>::new();
    scopes
        .allocate_source(
            &mut slots,
            parsed.root_syntax().id(),
            HirSourceSite::Span(parsed.document().span(SourceRange::new(0, 0)).unwrap()),
            HirScope::try_new(
                plan.module_id(),
                HirScopeKind::Module,
                None,
                HirScopeOwner::Module(plan.module_id()),
                Box::new([]),
                Box::new([]),
            )
            .unwrap(),
        )
        .unwrap();
    let scopes = scopes.into_snapshot(&mut slots).unwrap();
    let prepared = slots.prepare().unwrap();
    let observable_slots = Arc::clone(prepared.snapshot());
    let arenas = HirModuleArenas::try_new(
        prepared.snapshot(),
        HirModuleArenaParts {
            items: ArenaSnapshot::empty(prepared.snapshot()),
            scopes,
            locals: ArenaSnapshot::empty(prepared.snapshot()),
            expressions: ArenaSnapshot::empty(prepared.snapshot()),
            statements: ArenaSnapshot::empty(prepared.snapshot()),
            types: ArenaSnapshot::empty(prepared.snapshot()),
            patterns: ArenaSnapshot::empty(prepared.snapshot()),
            captures: ArenaSnapshot::empty(prepared.snapshot()),
        },
    )
    .unwrap();
    let module = Arc::new(
        HirModule::try_new(
            plan.snapshot_id(),
            plan.key().clone(),
            &parsed,
            Arc::from([]),
            Arc::clone(prepared.snapshot()),
            arenas,
            Box::new([]),
            HirDeclarationMemberIndexBuilder::new(plan.module_id()).freeze(),
            HirSourceIndex::empty(parsed.document().identity().clone(), prepared.snapshot()),
            plan.invalidation_epoch(),
        )
        .unwrap(),
    );
    let output = database.publish_module(plan, prepared, module).unwrap();
    assert!(output.invalidations().is_empty());
    assert_eq!(observable_slots.committed_slot_count(), 1);
    assert!(Arc::ptr_eq(
        output.module(),
        &database.current(&key).unwrap()
    ));
}

#[test]
fn snapshot_lookup_distinguishes_database_module_and_revision() {
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcw:/proof/database-lookups");
    let current = stage_and_commit(&mut database, &key);
    let foreign = HirDatabase::try_new().unwrap();
    let foreign_module = HirModuleId::new(foreign.database_id(), NonZeroU32::MIN);
    let foreign_snapshot = HirSnapshotId::new(foreign_module, HirRevision::INITIAL);
    assert_eq!(
        database.snapshot(foreign_snapshot).err(),
        Some(HirSnapshotLookupError::WrongDatabase {
            expected: database.database_id(),
            actual: foreign.database_id(),
        })
    );

    let unknown_module = HirModuleId::new(
        database.database_id(),
        NonZeroU32::new(current.snapshot_id().module().slot().get() + 1).unwrap(),
    );
    let unknown_snapshot = HirSnapshotId::new(unknown_module, HirRevision::INITIAL);
    assert_eq!(
        database.snapshot(unknown_snapshot).err(),
        Some(HirSnapshotLookupError::UnknownModule {
            module: unknown_module,
        })
    );

    let unknown_revision = current.snapshot_id().revision().checked_next().unwrap();
    assert_eq!(
        database
            .snapshot(HirSnapshotId::new(
                current.snapshot_id().module(),
                unknown_revision,
            ))
            .err(),
        Some(HirSnapshotLookupError::UnknownRevision {
            module: current.snapshot_id().module(),
            revision: unknown_revision,
        })
    );
}

#[test]
fn mismatched_validated_snapshot_publishes_nothing() {
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcw:/proof/database-mismatch");
    let other_key = module_key("arcw:/proof/database-mismatch-other");
    let plan = database.stage_module(&key).unwrap();
    let expected_snapshot = plan.snapshot_id();
    let mismatched = Arc::new(HirModule::from_validated_test(
        plan.snapshot_id(),
        other_key,
        plan.invalidation_epoch(),
    ));

    assert!(matches!(
        database.commit_module(plan, mismatched),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidModuleCommit
        ))
    ));
    assert!(database.current(&key).is_none());
    assert_eq!(
        database.stage_module(&key).unwrap().snapshot_id(),
        expected_snapshot
    );
}

#[test]
fn module_limit_is_inclusive_and_atomic() {
    assert_eq!(HirLimit::ModulesPerDatabase.maximum(), 65_536);
    let first_key = module_key("arcw:/proof/database-limit-first");
    let second_key = module_key("arcw:/proof/database-limit-second");
    let mut limited = HirDatabase::with_test_module_limit(1);
    let first = stage_and_commit(&mut limited, &first_key);
    let Err(error) = limited.stage_module(&second_key) else {
        panic!("one-over module count must fail")
    };
    assert_eq!(
        error,
        HirLowerFailure::Limit(HirLimitError::with_maximum(
            HirLimit::ModulesPerDatabase,
            2,
            1,
        ))
    );
    assert!(Arc::ptr_eq(&first, &limited.current(&first_key).unwrap()));
}

#[test]
fn module_identity_exhaustion_is_atomic() {
    let first_key = module_key("arcw:/proof/database-exhaustion-first");
    let second_key = module_key("arcw:/proof/database-exhaustion-second");
    let mut exhausted = HirDatabase::try_new().unwrap();
    exhausted.seed_next_module_slot(NonZeroU32::new(u32::MAX).unwrap());
    let last = stage_and_commit(&mut exhausted, &first_key);
    assert!(matches!(
        exhausted.stage_module(&second_key),
        Err(HirLowerFailure::ModuleIdentityExhausted)
    ));
    assert!(Arc::ptr_eq(&last, &exhausted.current(&first_key).unwrap()));
}

#[test]
fn revision_exhaustion_is_atomic() {
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcw:/proof/database-revision-exhaustion");
    let initial = stage_and_commit(&mut database, &key);
    let exhausted_revision = HirRevision::from_raw_for_test(NonZeroU32::new(u32::MAX).unwrap());
    let exhausted_snapshot = HirSnapshotId::new(initial.snapshot_id().module(), exhausted_revision);
    let exhausted = Arc::new(HirModule::from_validated_test(
        exhausted_snapshot,
        key.clone(),
        NonZeroU64::MIN,
    ));
    let state = database.modules.get_mut(&key).unwrap();
    state.current = Arc::clone(&exhausted);
    state
        .snapshots
        .insert(exhausted_revision, Arc::clone(&exhausted));

    assert!(matches!(
        database.stage_module(&key),
        Err(HirLowerFailure::RevisionExhausted { module })
            if module == initial.snapshot_id().module()
    ));
    assert!(Arc::ptr_eq(&exhausted, &database.current(&key).unwrap()));
}

#[test]
fn cache_epoch_exhaustion_is_atomic() {
    let mut database = HirDatabase::try_new().unwrap();
    let key = module_key("arcw:/proof/database-epoch-exhaustion");
    let initial = stage_and_commit(&mut database, &key);
    let exhausted = Arc::new(HirModule::from_validated_test(
        initial.snapshot_id(),
        key.clone(),
        NonZeroU64::new(u64::MAX).unwrap(),
    ));
    let state = database.modules.get_mut(&key).unwrap();
    state.current = Arc::clone(&exhausted);
    state
        .snapshots
        .insert(initial.snapshot_id().revision(), Arc::clone(&exhausted));

    assert!(matches!(
        database.stage_module(&key),
        Err(HirLowerFailure::CacheEpochExhausted { module })
            if module == initial.snapshot_id().module()
    ));
    assert!(Arc::ptr_eq(&exhausted, &database.current(&key).unwrap()));
}
