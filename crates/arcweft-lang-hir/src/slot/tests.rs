use core::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use arcweft_lang_syntax::attachment::SyntaxNodeId;
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::identity::{
    CaptureId, ExprId, HirDatabaseId, HirIdKind, HirLimit, HirModuleId, HirRevision, HirTypedId,
    IdResolveError, ItemId, LocalId, PatternId, RawHirId, ScopeId, StmtId, SyntheticKey,
    SyntheticKeyError, SyntheticOwner, SyntheticRole, TypeId,
};
use crate::lowering::HirLimitError;
use crate::source_index::{HirInsertionPoint, HirSourceSite};

use super::{
    HirOrigin, HirSlotError, HirSlotLifetime, KeyOnlyReservation, SlotSnapshot, SourceKey,
    StagedSlotTransaction, resolve_lifetime,
};

struct Fixture {
    parsed: ParsedSource,
}

impl Fixture {
    fn new() -> Self {
        let name = SourceName::path("proof/slot.arcw");
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://proof/slot").unwrap(),
                name.clone(),
                "fn main() {}\n",
            )
            .unwrap(),
        );
        let mut database = SyntaxDatabase::try_new().unwrap();
        let parsed = database
            .parse_initial(
                SourceSnapshotId::initial(name),
                document,
                arcweft_lang_syntax::parser::ParseOptions::default(),
            )
            .unwrap();
        Self { parsed }
    }

    fn syntax(&self) -> SyntaxNodeId {
        self.parsed.root_syntax().id()
    }

    fn source(&self, start: usize) -> HirSourceSite {
        HirSourceSite::Span(
            self.parsed
                .document()
                .span(SourceRange::new(start, start))
                .unwrap(),
        )
    }

    fn insertion(&self, offset: usize) -> HirSourceSite {
        HirSourceSite::Insertion(
            HirInsertionPoint::try_new(self.parsed.document(), offset).unwrap(),
        )
    }
}

fn database(value: u64) -> HirDatabaseId {
    HirDatabaseId::from_raw_for_test(NonZeroU64::new(value).unwrap())
}

fn module(database: u64) -> HirModuleId {
    HirModuleId::new(self::database(database), NonZeroU32::new(1).unwrap())
}

fn revision(value: u32) -> HirRevision {
    HirRevision::from_raw_for_test(NonZeroU32::new(value).unwrap())
}

fn key(owner: SyntheticOwner, role: SyntheticRole, ordinal: u32) -> SyntheticKey {
    SyntheticKey::try_new(owner, role, ordinal).unwrap()
}

fn reserve_reused_expr_child(
    transaction: &mut StagedSlotTransaction,
    synthetic_key: SyntheticKey,
    source_site: HirSourceSite,
    poisoned: bool,
) -> ExprId {
    let first = transaction
        .reserve_synthetic::<ExprId>(synthetic_key, source_site.clone(), poisoned)
        .unwrap();
    assert!(first.is_first_touch());
    let repeated = transaction
        .reserve_synthetic::<ExprId>(synthetic_key, source_site, poisoned)
        .unwrap();
    assert!(!repeated.is_first_touch());
    assert_eq!(repeated.id(), first.id());
    first.id()
}

fn assert_slot_identity_exhaustion_is_atomic_for<I: HirTypedId + core::fmt::Debug>() {
    let fixture = Fixture::new();
    let owner_module = module(91);
    let empty = SlotSnapshot::empty(owner_module, revision(1));
    let initial_lifetimes = empty.lifetime_test_state();

    let mut exhausted = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    exhausted.exhaust_next_slot_identity_for_test();
    assert_eq!(
        exhausted
            .reserve_source::<I>(fixture.syntax(), fixture.source(0), false)
            .unwrap_err(),
        HirSlotError::SlotIdentityExhausted {
            module: owner_module,
            kind: I::KIND,
        }
    );
    assert!(matches!(
        exhausted.reserve_source::<I>(fixture.syntax(), fixture.source(0), false),
        Err(HirSlotError::TransactionPoisoned)
    ));
    assert!(matches!(
        exhausted.prepare(),
        Err(HirSlotError::TransactionPoisoned)
    ));

    assert_eq!(empty.lifetime_test_state(), initial_lifetimes);
    assert_eq!(empty.committed_slot_count(), 0);
    assert_eq!(empty.source_key_count(), 0);
    assert_eq!(empty.synthetic_pair_count(), 0);

    let mut control = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let control_id = control
        .reserve_source::<I>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    assert_eq!(control_id.raw().slot(), NonZeroU32::MIN);
    let committed = control.commit().unwrap();
    assert!(committed.resolve(control_id).is_ok());
    assert_eq!(committed.committed_slot_count(), 1);
}

#[test]
fn slot_identity_exhaustion_is_atomic() {
    assert_slot_identity_exhaustion_is_atomic_for::<ItemId>();
    assert_slot_identity_exhaustion_is_atomic_for::<ScopeId>();
    assert_slot_identity_exhaustion_is_atomic_for::<LocalId>();
    assert_slot_identity_exhaustion_is_atomic_for::<ExprId>();
    assert_slot_identity_exhaustion_is_atomic_for::<StmtId>();
    assert_slot_identity_exhaustion_is_atomic_for::<TypeId>();
    assert_slot_identity_exhaustion_is_atomic_for::<PatternId>();
    assert_slot_identity_exhaustion_is_atomic_for::<CaptureId>();
}

#[test]
fn same_pair_reuses_one_id_and_different_child_kind_is_distinct() {
    let fixture = Fixture::new();
    let mut transaction = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let owner_reservation = transaction
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(1), false)
        .unwrap();
    assert!(owner_reservation.is_first_touch());
    let owner_id = owner_reservation.id();
    let owner = SyntheticOwner::Expr(owner_id);
    let key = key(owner, SyntheticRole::RecoveryOperand, 0);

    let expression_reservation = transaction
        .reserve_synthetic::<ExprId>(key, fixture.source(2), false)
        .unwrap();
    assert!(expression_reservation.is_first_touch());
    let expression = expression_reservation.id();
    let repeated_reservation = transaction
        .reserve_synthetic::<ExprId>(key, fixture.source(2), false)
        .unwrap();
    assert!(!repeated_reservation.is_first_touch());
    let repeated = repeated_reservation.id();
    let statement = transaction
        .reserve_synthetic::<StmtId>(key, fixture.source(4), true)
        .unwrap()
        .id();

    assert_eq!(expression, repeated);
    assert_ne!(expression.kind(), statement.kind());
    assert_eq!(transaction.synthetic_count(owner), 2);
    assert_eq!(transaction.staged_slot_count(), 3);

    let snapshot = transaction.commit().unwrap();
    let expression_metadata = snapshot.resolve(expression).unwrap();
    assert_eq!(expression_metadata.origin(), &HirOrigin::Synthetic(key));
    assert_eq!(expression_metadata.source_site(), &fixture.source(2));
    assert!(!expression_metadata.is_poisoned());
    let statement_metadata = snapshot.resolve(statement).unwrap();
    assert_eq!(statement_metadata.origin(), &HirOrigin::Synthetic(key));
    assert_eq!(statement_metadata.source_site(), &fixture.source(4));
    assert!(statement_metadata.is_poisoned());
    assert_eq!(snapshot.synthetic_pair_count(), 2);
}

#[test]
fn scope_owned_tail_rolls_back_with_its_owner_and_insertion() {
    let fixture = Fixture::new();
    let empty = SlotSnapshot::empty(module(8), revision(1));

    let mut abandoned = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let abandoned_scope = abandoned
        .reserve_source::<ScopeId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let abandoned_key = key(
        SyntheticOwner::Scope(abandoned_scope),
        SyntheticRole::MissingRequiredTail,
        0,
    );
    let abandoned_tail =
        reserve_reused_expr_child(&mut abandoned, abandoned_key, fixture.insertion(2), true);
    assert_eq!(abandoned.synthetic_count(abandoned_key.owner()), 1);
    drop(abandoned);

    assert_eq!(empty.committed_slot_count(), 0);
    assert_eq!(empty.synthetic_pair_count(), 0);
    assert!(matches!(
        empty.resolve(abandoned_scope),
        Err(HirSlotError::OwnerNotReserved { .. })
    ));
    assert!(matches!(
        empty.resolve(abandoned_tail),
        Err(HirSlotError::OwnerNotReserved { .. })
    ));

    let mut retry = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let retry_scope = retry
        .reserve_source::<ScopeId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let retry_tail = reserve_reused_expr_child(
        &mut retry,
        key(
            SyntheticOwner::Scope(retry_scope),
            SyntheticRole::MissingRequiredTail,
            0,
        ),
        fixture.insertion(2),
        true,
    );
    assert_eq!(retry_scope, abandoned_scope);
    assert_eq!(retry_tail, abandoned_tail);
}

#[test]
fn exact_zero_tail_pairs_reuse_for_expr_and_scope_owners_with_child_insertions() {
    let fixture = Fixture::new();
    let empty = SlotSnapshot::empty(module(8), revision(1));

    let mut transaction = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let scope_owner = transaction
        .reserve_source::<ScopeId>(fixture.syntax(), fixture.source(1), false)
        .unwrap()
        .id();
    let expression_owner = transaction
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();

    let expression_key = key(
        SyntheticOwner::Expr(expression_owner),
        SyntheticRole::ImplicitUnitTail,
        0,
    );
    let scope_key = key(
        SyntheticOwner::Scope(scope_owner),
        SyntheticRole::MissingRequiredTail,
        0,
    );
    assert_eq!(
        SyntheticKey::try_new(
            SyntheticOwner::Expr(expression_owner),
            SyntheticRole::ImplicitUnitTail,
            1,
        ),
        Err(SyntheticKeyError::InvalidOrdinal {
            role: SyntheticRole::ImplicitUnitTail,
            ordinal: 1,
        })
    );
    assert_eq!(
        SyntheticKey::try_new(
            SyntheticOwner::Scope(scope_owner),
            SyntheticRole::MissingRequiredTail,
            1,
        ),
        Err(SyntheticKeyError::InvalidOrdinal {
            role: SyntheticRole::MissingRequiredTail,
            ordinal: 1,
        })
    );

    let expression_tail = reserve_reused_expr_child(
        &mut transaction,
        expression_key,
        fixture.insertion(3),
        false,
    );
    let scope_tail =
        reserve_reused_expr_child(&mut transaction, scope_key, fixture.insertion(4), true);

    assert_ne!(expression_tail, scope_tail);
    assert_eq!(transaction.synthetic_count(expression_key.owner()), 1);
    assert_eq!(transaction.synthetic_count(scope_key.owner()), 1);
    let snapshot = transaction.commit().unwrap();
    assert_eq!(snapshot.synthetic_pair_count(), 2);
    assert_eq!(snapshot.committed_slot_count(), 4);

    let expression_tail_metadata = snapshot.resolve(expression_tail).unwrap();
    assert_eq!(
        expression_tail_metadata.origin(),
        &HirOrigin::Synthetic(expression_key)
    );
    assert_eq!(
        expression_tail_metadata.source_site(),
        &fixture.insertion(3)
    );
    assert!(!expression_tail_metadata.is_poisoned());

    let scope_tail_metadata = snapshot.resolve(scope_tail).unwrap();
    assert_eq!(scope_tail.kind(), HirIdKind::Expr);
    assert_eq!(
        scope_tail_metadata.origin(),
        &HirOrigin::Synthetic(scope_key)
    );
    assert_eq!(scope_tail_metadata.source_site(), &fixture.insertion(4));
    assert!(scope_tail_metadata.is_poisoned());
}

#[test]
fn wrong_module_is_checked_before_slot() {
    let fixture = Fixture::new();
    let local_module = module(9);
    let foreign_module = module(10);
    let snapshot = crate::identity::HirSnapshotId::new(local_module, revision(2));
    let source_origin = HirOrigin::Source(SourceKey::new(fixture.syntax(), HirIdKind::Stmt));

    let foreign = RawHirId::new(foreign_module, NonZeroU32::MIN, HirIdKind::Expr);
    assert_eq!(
        resolve_lifetime(
            snapshot,
            foreign,
            HirIdKind::Expr,
            Some(HirSlotLifetime {
                kind: HirIdKind::Stmt,
                born: revision(3),
                retired_at: Some(revision(4)),
                origin: source_origin.clone(),
            }),
        ),
        Err(HirSlotError::Resolve(IdResolveError::WrongModule {
            expected: local_module,
            actual: foreign_module,
        }))
    );

    let local = RawHirId::new(local_module, NonZeroU32::MIN, HirIdKind::Expr);
    assert_eq!(
        resolve_lifetime(
            snapshot,
            local,
            HirIdKind::Expr,
            Some(HirSlotLifetime {
                kind: HirIdKind::Stmt,
                born: revision(3),
                retired_at: Some(revision(4)),
                origin: source_origin.clone(),
            }),
        ),
        Err(HirSlotError::Resolve(IdResolveError::NotYetLive {
            id: local.view(),
            snapshot,
            born: revision(3),
        }))
    );
    assert_eq!(
        resolve_lifetime(
            snapshot,
            local,
            HirIdKind::Expr,
            Some(HirSlotLifetime {
                kind: HirIdKind::Stmt,
                born: revision(1),
                retired_at: Some(revision(2)),
                origin: source_origin.clone(),
            }),
        ),
        Err(HirSlotError::Resolve(IdResolveError::Retired {
            id: local.view(),
            snapshot,
            retired_at: revision(2),
        }))
    );
    assert_eq!(
        resolve_lifetime(
            snapshot,
            local,
            HirIdKind::Expr,
            Some(HirSlotLifetime {
                kind: HirIdKind::Stmt,
                born: revision(1),
                retired_at: None,
                origin: source_origin,
            }),
        ),
        Err(HirSlotError::Resolve(IdResolveError::KindMismatch {
            id: local.view(),
            expected: HirIdKind::Expr,
            actual: HirIdKind::Stmt,
        }))
    );
}

#[test]
fn elided_region_key_only_identity_reuses_persists_and_rolls_back_with_its_owner() {
    let fixture = Fixture::new();
    let empty = SlotSnapshot::empty(module(41), revision(1));

    let mut dropped = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let dropped_owner = dropped
        .reserve_source::<TypeId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let dropped_key = key(
        SyntheticOwner::Type(dropped_owner),
        SyntheticRole::ElidedRegion,
        0,
    );
    assert_eq!(
        dropped.stage_elided_region_key(dropped_key),
        Ok(KeyOnlyReservation::FirstTouch)
    );
    assert_eq!(dropped.synthetic_count(dropped_key.owner()), 1);
    drop(dropped);
    assert_eq!(empty.committed_slot_count(), 0);
    assert_eq!(empty.key_only_synthetic_count(), 0);

    let mut initial = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let owner = initial
        .reserve_source::<TypeId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    assert_eq!(owner, dropped_owner);
    let key = key(SyntheticOwner::Type(owner), SyntheticRole::ElidedRegion, 0);
    assert_eq!(
        initial.stage_elided_region_key(key),
        Ok(KeyOnlyReservation::FirstTouch)
    );
    assert_eq!(
        initial.stage_elided_region_key(key),
        Ok(KeyOnlyReservation::Reused)
    );
    assert_eq!(initial.synthetic_count(key.owner()), 1);
    assert_eq!(initial.staged_slot_count(), 1);
    let first = initial.commit().unwrap();
    assert!(first.contains_key_only_synthetic(key));
    assert_eq!(first.key_only_synthetic_count(), 1);
    assert_eq!(first.synthetic_pair_count(), 0);

    let mut revised = StagedSlotTransaction::from_snapshot(&first, revision(2));
    assert_eq!(
        revised
            .reserve_source::<TypeId>(fixture.syntax(), fixture.source(1), false)
            .unwrap()
            .id(),
        owner
    );
    assert_eq!(
        revised.stage_elided_region_key(key),
        Ok(KeyOnlyReservation::FirstTouch)
    );
    assert_eq!(
        revised.stage_elided_region_key(key),
        Ok(KeyOnlyReservation::Reused)
    );
    assert_eq!(revised.synthetic_count(key.owner()), 1);
    assert!(revised.retire_untouched().unwrap().is_empty());
    let second = revised.commit().unwrap();
    assert!(second.contains_key_only_synthetic(key));
    assert_eq!(second.key_only_synthetic_count(), 1);
}

#[test]
fn key_only_staging_rejects_a_role_that_requires_an_arena_child() {
    let fixture = Fixture::new();
    let mut transaction = StagedSlotTransaction::new(module(42), revision(1));
    let owner = transaction
        .reserve_source::<TypeId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let key =
        SyntheticKey::try_new(SyntheticOwner::Type(owner), SyntheticRole::ElidedRegion, 0).unwrap();
    assert_eq!(
        transaction.stage_elided_region_key(key),
        Ok(KeyOnlyReservation::FirstTouch)
    );

    let expression = transaction
        .reserve_source::<ExprId>(fixture.parsed.root_syntax().id(), fixture.source(1), false)
        .unwrap()
        .id();
    let child_role = SyntheticKey::try_new(
        SyntheticOwner::Expr(expression),
        SyntheticRole::RecoveryOperand,
        0,
    )
    .unwrap();
    assert_eq!(
        transaction.stage_elided_region_key(child_role),
        Err(HirSlotError::InvalidKeyOnlyRole {
            role: SyntheticRole::RecoveryOperand,
        })
    );
    assert!(matches!(
        transaction.commit(),
        Err(HirSlotError::TransactionPoisoned)
    ));
}

#[test]
fn source_key_reuses_one_typed_id_and_retains_exact_origin() {
    let fixture = Fixture::new();
    let mut transaction = StagedSlotTransaction::new(module(1), revision(1));
    let first_reservation = transaction
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap();
    assert!(first_reservation.is_first_touch());
    let first = first_reservation.id();
    let repeated_reservation = transaction
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap();
    assert!(!repeated_reservation.is_first_touch());
    let repeated = repeated_reservation.id();

    assert_eq!(first, repeated);
    assert_eq!(transaction.staged_slot_count(), 1);
    let snapshot = transaction.commit().unwrap();
    let metadata = snapshot.resolve(first).unwrap();
    let expected = SourceKey::new(fixture.syntax(), HirIdKind::Expr);
    assert_eq!(metadata.origin(), &HirOrigin::Source(expected.clone()));
    assert_eq!(metadata.source_site(), &fixture.source(0));
    assert!(!metadata.is_poisoned());
    assert_eq!(snapshot.source_key_count(), 1);
    assert_eq!(expected.syntax(), fixture.syntax());
    assert_eq!(expected.kind(), HirIdKind::Expr);
}

#[test]
fn untouched_owner_retirement_reaches_touched_synthetic_descendants_to_fixed_point() {
    let fixture = Fixture::new();
    let module = module(31);
    let mut first = StagedSlotTransaction::new(module, revision(1));
    let owner = first
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let synthetic_key = key(
        SyntheticOwner::Expr(owner),
        SyntheticRole::RecoveryOperand,
        0,
    );
    let child = first
        .reserve_synthetic::<ExprId>(synthetic_key, fixture.source(1), false)
        .unwrap()
        .id();
    let grandchild_key = key(
        SyntheticOwner::Expr(child),
        SyntheticRole::RecoveryOperand,
        0,
    );
    let grandchild = first
        .reserve_synthetic::<StmtId>(grandchild_key, fixture.source(2), false)
        .unwrap()
        .id();
    let first = first.commit().unwrap();

    let mut second = StagedSlotTransaction::from_snapshot(&first, revision(2));
    assert_eq!(
        second
            .reserve_synthetic::<ExprId>(synthetic_key, fixture.source(1), false)
            .unwrap()
            .id(),
        child
    );
    assert_eq!(
        second
            .reserve_synthetic::<StmtId>(grandchild_key, fixture.source(2), false)
            .unwrap()
            .id(),
        grandchild
    );
    assert_eq!(
        second.retire_untouched().unwrap().as_ref(),
        [
            SyntheticOwner::Expr(owner),
            SyntheticOwner::Expr(child),
            SyntheticOwner::Stmt(grandchild),
        ]
    );
    let second = second.commit().unwrap();

    assert!(first.resolve(owner).is_ok());
    assert!(first.resolve(child).is_ok());
    assert!(first.resolve(grandchild).is_ok());
    assert!(matches!(
        second.resolve(owner),
        Err(HirSlotError::Resolve(IdResolveError::Retired { id, .. }))
            if id == owner.raw().view()
    ));
    assert!(matches!(
        second.resolve(child),
        Err(HirSlotError::Resolve(IdResolveError::Retired { id, .. }))
            if id == child.raw().view()
    ));
    assert!(matches!(
        second.resolve(grandchild),
        Err(HirSlotError::Resolve(IdResolveError::Retired { id, .. }))
            if id == grandchild.raw().view()
    ));
}

#[test]
fn retired_synthetic_pair_is_readmitted_with_a_fresh_slot_without_double_charging_its_key() {
    let fixture = Fixture::new();
    let module = module(32);
    let mut initial = StagedSlotTransaction::new(module, revision(1));
    let owner = initial
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let owner = SyntheticOwner::Expr(owner);
    let synthetic_key = key(owner, SyntheticRole::RecoveryOperand, 0);
    let first_child = initial
        .reserve_synthetic::<ExprId>(synthetic_key, fixture.source(1), false)
        .unwrap()
        .id();
    assert_eq!(initial.synthetic_count(owner), 1);
    let first = initial.commit().unwrap();

    let mut removal = StagedSlotTransaction::from_snapshot(&first, revision(2));
    assert_eq!(
        SyntheticOwner::Expr(
            removal
                .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
                .unwrap()
                .id()
        ),
        owner
    );
    assert_eq!(
        removal.retire_untouched().unwrap().as_ref(),
        [SyntheticOwner::Expr(first_child)]
    );
    let second = removal.commit().unwrap();
    assert_eq!(second.synthetic_pair_count(), 1);
    assert!(matches!(
        second.resolve_prepared_synthetic::<ExprId>(synthetic_key),
        Err(HirSlotError::Resolve(IdResolveError::Retired { id, .. }))
            if id == first_child.raw().view()
    ));

    let mut readmission = StagedSlotTransaction::from_snapshot(&second, revision(3));
    assert_eq!(
        SyntheticOwner::Expr(
            readmission
                .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
                .unwrap()
                .id()
        ),
        owner
    );
    assert_eq!(readmission.synthetic_count(owner), 1);
    let replacement = readmission
        .reserve_synthetic::<ExprId>(synthetic_key, fixture.source(2), true)
        .unwrap();
    assert!(replacement.is_first_touch());
    let replacement_child = replacement.id();
    assert_ne!(replacement_child, first_child);
    let repeated = readmission
        .reserve_synthetic::<ExprId>(synthetic_key, fixture.source(2), true)
        .unwrap();
    assert!(!repeated.is_first_touch());
    assert_eq!(repeated.id(), replacement_child);
    assert_eq!(readmission.synthetic_count(owner), 1);
    assert_eq!(readmission.staged_slot_count(), 1);
    assert!(readmission.retire_untouched().unwrap().is_empty());
    let third = readmission.commit().unwrap();

    assert_eq!(first.synthetic_pair_count(), 1);
    assert_eq!(
        first
            .resolve_prepared_synthetic::<ExprId>(synthetic_key)
            .unwrap(),
        Some(first_child)
    );
    assert!(first.resolve(first_child).is_ok());
    assert!(matches!(
        second.resolve(first_child),
        Err(HirSlotError::Resolve(IdResolveError::Retired { id, .. }))
            if id == first_child.raw().view()
    ));
    assert!(matches!(
        third.resolve(first_child),
        Err(HirSlotError::Resolve(IdResolveError::Retired { id, .. }))
            if id == first_child.raw().view()
    ));
    assert_eq!(third.synthetic_pair_count(), 1);
    assert_eq!(third.committed_slot_count(), 3);
    assert_eq!(
        third
            .resolve_prepared_synthetic::<ExprId>(synthetic_key)
            .unwrap(),
        Some(replacement_child)
    );
    let replacement_metadata = third.resolve(replacement_child).unwrap();
    assert_eq!(replacement_metadata.born(), revision(3));
    assert_eq!(replacement_metadata.retired_at(), None);
    assert_eq!(
        replacement_metadata.origin(),
        &HirOrigin::Synthetic(synthetic_key)
    );
    assert_eq!(replacement_metadata.source_site(), &fixture.source(2));
    assert!(replacement_metadata.is_poisoned());
}

#[test]
fn retained_synthetic_pair_refreshes_one_snapshot_view_and_rejects_conflicts() {
    let fixture = Fixture::new();
    let mut initial = StagedSlotTransaction::new(module(1), revision(1));
    let owner_id = initial
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let owner = SyntheticOwner::Expr(owner_id);
    let key = key(owner, SyntheticRole::RecoveryOperand, 0);
    let child = initial
        .reserve_synthetic::<ExprId>(key, fixture.source(1), false)
        .unwrap()
        .id();
    let first = initial.commit().unwrap();

    let mut refresh = StagedSlotTransaction::from_snapshot(&first, revision(2));
    let refreshed_reservation = refresh
        .reserve_synthetic::<ExprId>(key, fixture.source(2), true)
        .unwrap();
    assert!(refreshed_reservation.is_first_touch());
    let refreshed = refreshed_reservation.id();
    let repeated_reservation = refresh
        .reserve_synthetic::<ExprId>(key, fixture.source(2), true)
        .unwrap();
    assert!(!repeated_reservation.is_first_touch());
    let repeated = repeated_reservation.id();
    assert_eq!(refreshed, child);
    assert_eq!(repeated, child);
    assert_eq!(refresh.synthetic_count(owner), 1);
    let second = refresh.commit().unwrap();
    let metadata = second.resolve(child).unwrap();
    assert_eq!(metadata.born(), revision(1));
    assert_eq!(metadata.source_site(), &fixture.source(2));
    assert!(metadata.is_poisoned());

    let mut conflict = StagedSlotTransaction::from_snapshot(&second, revision(3));
    conflict
        .reserve_synthetic::<ExprId>(key, fixture.source(3), false)
        .unwrap();
    assert_eq!(
        conflict.reserve_synthetic::<ExprId>(key, fixture.source(4), true),
        Err(HirSlotError::ConflictingSlotView {
            id: child.raw().view(),
        })
    );
    assert!(matches!(
        conflict.commit(),
        Err(HirSlotError::TransactionPoisoned)
    ));
    assert_eq!(
        second.resolve(child).unwrap().source_site(),
        &fixture.source(2)
    );
    assert!(second.resolve(child).unwrap().is_poisoned());
}

#[test]
fn snapshot_resolution_cross_checks_metadata_against_lifetime() {
    let fixture = Fixture::new();
    let mut transaction = StagedSlotTransaction::new(module(1), revision(1));
    let id = transaction
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let mut snapshot = transaction.commit().unwrap();
    snapshot.corrupt_metadata_kind(id.raw(), HirIdKind::Stmt);

    assert_eq!(
        snapshot.resolve(id),
        Err(HirSlotError::MetadataMismatch {
            id: id.raw().view(),
        })
    );
}

#[test]
fn foreign_owner_fails_before_slot_source_or_count_staging() {
    let fixture = Fixture::new();
    let mut foreign = StagedSlotTransaction::new(module(2), HirRevision::INITIAL);
    let foreign_id = foreign
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let mut local = StagedSlotTransaction::new(module(1), HirRevision::INITIAL);
    let owner = SyntheticOwner::Expr(foreign_id);

    assert_eq!(
        local.reserve_synthetic::<ExprId>(
            key(owner, SyntheticRole::RecoveryOperand, 0),
            fixture.source(1),
            false,
        ),
        Err(HirSlotError::Resolve(IdResolveError::WrongModule {
            expected: module(1),
            actual: module(2),
        }))
    );
    assert_eq!(local.staged_slot_count(), 0);
    assert_eq!(local.synthetic_count(owner), 0);
    assert!(matches!(
        local.commit(),
        Err(HirSlotError::TransactionPoisoned)
    ));
}

#[test]
fn not_yet_live_owner_reports_exact_snapshot_and_birth() {
    let fixture = Fixture::new();
    let initial = StagedSlotTransaction::new(module(1), revision(1))
        .commit()
        .unwrap();
    let mut future = StagedSlotTransaction::from_snapshot(&initial, revision(2));
    let future_id = future
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(1), false)
        .unwrap()
        .id();
    future.commit().unwrap();

    let owner = SyntheticOwner::Expr(future_id);
    let mut old = StagedSlotTransaction::from_snapshot(&initial, revision(1));
    assert_eq!(
        old.reserve_synthetic::<ExprId>(
            key(owner, SyntheticRole::RecoveryOperand, 0),
            fixture.source(2),
            false,
        ),
        Err(HirSlotError::Resolve(IdResolveError::NotYetLive {
            id: future_id.raw().view(),
            snapshot: initial.snapshot_id(),
            born: revision(2),
        }))
    );
    assert_eq!(old.staged_slot_count(), 0);
    assert_eq!(old.synthetic_count(owner), 0);
}

#[test]
fn retired_owner_reports_exact_snapshot_and_retirement() {
    let fixture = Fixture::new();
    let mut initial = StagedSlotTransaction::new(module(1), revision(1));
    let owner_id = initial
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(1), false)
        .unwrap()
        .id();
    let live = initial.commit().unwrap();
    let mut retirement = StagedSlotTransaction::from_snapshot(&live, revision(2));
    retirement.retire(owner_id).unwrap();
    let retired = retirement.commit().unwrap();

    let owner = SyntheticOwner::Expr(owner_id);
    let mut transaction = StagedSlotTransaction::from_snapshot(&retired, revision(2));
    assert_eq!(
        transaction.reserve_synthetic::<ExprId>(
            key(owner, SyntheticRole::RecoveryOperand, 0),
            fixture.source(2),
            false,
        ),
        Err(HirSlotError::Resolve(IdResolveError::Retired {
            id: owner_id.raw().view(),
            snapshot: retired.snapshot_id(),
            retired_at: revision(2),
        }))
    );
    assert_eq!(transaction.staged_slot_count(), 0);
    assert_eq!(transaction.synthetic_count(owner), 0);
}

#[test]
fn old_snapshot_resolves_live_interval() {
    let fixture = Fixture::new();
    let pre_birth = StagedSlotTransaction::new(module(44), revision(1))
        .commit()
        .unwrap();

    let mut birth = StagedSlotTransaction::from_snapshot(&pre_birth, revision(2));
    let owner = birth
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(1), false)
        .unwrap()
        .id();
    let born = birth.commit().unwrap();

    let mut living = StagedSlotTransaction::from_snapshot(&born, revision(3));
    assert_eq!(
        living
            .reserve_source::<ExprId>(fixture.syntax(), fixture.source(1), false)
            .unwrap()
            .id(),
        owner
    );
    let live = living.commit().unwrap();

    let mut retirement = StagedSlotTransaction::from_snapshot(&live, revision(4));
    retirement.retire(owner).unwrap();
    let retired = retirement.commit().unwrap();

    assert!(matches!(
        pre_birth.resolve(owner),
        Err(HirSlotError::Resolve(IdResolveError::NotYetLive {
            id,
            snapshot,
            born,
        })) if id == owner.raw().view()
            && snapshot == pre_birth.snapshot_id()
            && born == revision(2)
    ));
    assert_eq!(born.resolve(owner).unwrap().born(), revision(2));
    assert_eq!(live.resolve(owner).unwrap().born(), revision(2));
    assert!(matches!(
        retired.resolve(owner),
        Err(HirSlotError::Resolve(IdResolveError::Retired {
            id,
            snapshot,
            retired_at,
        })) if id == owner.raw().view()
            && snapshot == retired.snapshot_id()
            && retired_at == revision(4)
    ));
}

#[test]
fn wrong_kind_corruption_hook_never_panics() {
    let fixture = Fixture::new();
    let initial = StagedSlotTransaction::new(module(1), revision(1))
        .commit()
        .unwrap();
    let mut future = StagedSlotTransaction::from_snapshot(&initial, revision(2));
    let owner_id = future
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(1), false)
        .unwrap()
        .id();
    let owner = SyntheticOwner::Expr(owner_id);
    future.corrupt_kind(owner, HirIdKind::Stmt);
    let mismatched = future.commit().unwrap();

    let mut old = StagedSlotTransaction::from_snapshot(&initial, revision(1));
    assert!(matches!(
        old.reserve_synthetic::<ExprId>(
            key(owner, SyntheticRole::RecoveryOperand, 0),
            fixture.source(2),
            false,
        ),
        Err(HirSlotError::Resolve(IdResolveError::NotYetLive { .. }))
    ));

    let mut live = StagedSlotTransaction::from_snapshot(&mismatched, revision(2));
    assert_eq!(
        live.reserve_synthetic::<ExprId>(
            key(owner, SyntheticRole::RecoveryOperand, 0),
            fixture.source(2),
            false,
        ),
        Err(HirSlotError::Resolve(IdResolveError::KindMismatch {
            id: owner_id.raw().view(),
            expected: HirIdKind::Expr,
            actual: HirIdKind::Stmt,
        }))
    );
}

#[test]
fn only_an_owner_reserved_by_the_same_transaction_is_admitted() {
    let fixture = Fixture::new();
    let empty = SlotSnapshot::empty(module(1), revision(1));
    let mut abandoned = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let abandoned_id = abandoned
        .reserve_source::<ScopeId>(fixture.syntax(), fixture.source(1), false)
        .unwrap()
        .id();
    drop(abandoned);
    assert_eq!(empty.committed_slot_count(), 0);

    let owner = SyntheticOwner::Scope(abandoned_id);
    let synthetic = key(owner, SyntheticRole::ImplicitUnitTail, 0);
    let mut before_reservation = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    assert_eq!(
        before_reservation.reserve_synthetic::<ExprId>(synthetic, fixture.source(2), false),
        Err(HirSlotError::OwnerNotReserved {
            id: abandoned_id.raw().view(),
        })
    );
    assert!(matches!(
        before_reservation.commit(),
        Err(HirSlotError::TransactionPoisoned)
    ));
    assert_eq!(empty.committed_slot_count(), 0);

    let mut reserved = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let reserved_id = reserved
        .reserve_source::<ScopeId>(fixture.syntax(), fixture.source(1), false)
        .unwrap()
        .id();
    assert_eq!(reserved_id, abandoned_id);
    let child = reserved
        .reserve_synthetic::<ExprId>(
            key(
                SyntheticOwner::Scope(reserved_id),
                SyntheticRole::ImplicitUnitTail,
                0,
            ),
            fixture.source(2),
            false,
        )
        .unwrap()
        .id();
    let committed = reserved.commit().unwrap();
    assert!(committed.resolve(reserved_id).is_ok());
    assert!(committed.resolve(child).is_ok());
}

#[test]
fn exactly_1024_fresh_pairs_commit_for_one_owner() {
    let fixture = Fixture::new();
    let mut transaction = StagedSlotTransaction::new(module(1), revision(1));
    let owner_id = transaction
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(1), false)
        .unwrap()
        .id();
    let owner = SyntheticOwner::Expr(owner_id);

    for ordinal in 0..1_024 {
        transaction
            .reserve_synthetic::<ExprId>(
                key(owner, SyntheticRole::RecoveryOperand, ordinal),
                fixture.source(2),
                false,
            )
            .unwrap();
    }

    assert_eq!(transaction.synthetic_count(owner), 1_024);
    let committed = transaction.commit().unwrap();
    assert_eq!(committed.synthetic_pair_count(), 1_024);
    assert_eq!(committed.committed_slot_count(), 1_025);
}

#[test]
fn prepared_slots_publish_nothing_until_the_outer_commit_consumes_them() {
    let fixture = Fixture::new();
    let empty = SlotSnapshot::empty(module(11), revision(1));
    let mut staged = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let proposed = staged
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let prepared = staged.prepare().unwrap();

    assert_eq!(prepared.snapshot().snapshot_id(), empty.snapshot_id());
    assert_eq!(empty.committed_slot_count(), 0);
    drop(prepared);
    assert_eq!(empty.committed_slot_count(), 0);

    let mut retry = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let retried = retry
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    assert_eq!(retried, proposed);
    let committed = retry.prepare().unwrap().publish().unwrap();
    assert!(committed.resolve(retried).is_ok());
    assert_eq!(committed.committed_slot_count(), 1);
}

#[test]
fn pair_1025_poisons_and_rolls_back_the_complete_prefix() {
    let fixture = Fixture::new();
    let empty = SlotSnapshot::empty(module(1), revision(1));
    let mut transaction = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let owner_id = transaction
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(1), false)
        .unwrap()
        .id();
    let owner = SyntheticOwner::Expr(owner_id);

    for ordinal in 0..1_024 {
        transaction
            .reserve_synthetic::<ExprId>(
                key(owner, SyntheticRole::RecoveryOperand, ordinal),
                fixture.source(2),
                false,
            )
            .unwrap();
    }
    assert_eq!(
        transaction.reserve_synthetic::<ExprId>(
            key(owner, SyntheticRole::PostfixIndexCandidateExpression, 0),
            fixture.source(3),
            true,
        ),
        Err(HirSlotError::Limit(HirLimitError::with_maximum(
            HirLimit::SyntheticDescendantsPerOwner,
            1_025,
            1_024,
        )))
    );
    assert_eq!(transaction.synthetic_count(owner), 1_024);
    assert!(matches!(
        transaction.commit(),
        Err(HirSlotError::TransactionPoisoned)
    ));
    assert_eq!(empty.committed_slot_count(), 0);
    assert_eq!(empty.synthetic_pair_count(), 0);
    assert_eq!(empty.source_key_count(), 0);
}
