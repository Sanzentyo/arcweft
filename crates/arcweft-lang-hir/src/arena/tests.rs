use core::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use arcweft_lang_syntax::attachment::SyntaxNodeId;
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

use crate::identity::{
    CaptureId, ExprId, HirDatabaseId, HirIdKind, HirLimit, HirModuleId, HirRevision, HirTypedId,
    ItemId, LocalId, PatternId, ScopeId, StmtId, SyntheticKey, SyntheticOwner, SyntheticRole,
    TypeId,
};
use crate::lower::HirLimitError;
use crate::slot::{HirOrigin, HirSlotError, SlotSnapshot, StagedSlotTransaction};
use crate::source_index::{HirInsertionPoint, HirSourceSite};

use super::{HirArenaError, HirArenaPayload, StagedArena};

impl HirArenaPayload for u8 {
    fn is_poisoned(&self) -> bool {
        false
    }
}

impl HirArenaPayload for u32 {
    fn is_poisoned(&self) -> bool {
        false
    }
}

impl HirArenaPayload for &'static str {
    fn is_poisoned(&self) -> bool {
        false
    }
}

#[derive(PartialEq)]
struct RecoveryRecord {
    poisoned: bool,
}

impl HirArenaPayload for RecoveryRecord {
    fn is_poisoned(&self) -> bool {
        self.poisoned
    }
}

struct Fixture {
    parsed: ParsedSource,
}

impl Fixture {
    fn new() -> Self {
        let name = SourceName::path("proof/arena.arcw");
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://proof/arena").unwrap(),
                name.clone(),
                "fn main() { let value = 1 + 2; value }\n",
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
        self.parsed.tree().root().id()
    }

    fn syntax_ids(&self) -> [SyntaxNodeId; 2] {
        let root = self.parsed.tree().root().syntax();
        let mut ids = root
            .rowan()
            .descendants()
            .filter_map(|node| self.parsed.bind_rowan(&node).ok())
            .map(|node| node.id());
        let first = ids.next().unwrap();
        let second = ids.find(|candidate| *candidate != first).unwrap();
        [first, second]
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

fn module(value: u64) -> HirModuleId {
    HirModuleId::new(database(value), NonZeroU32::MIN)
}

fn revision(value: u32) -> HirRevision {
    HirRevision::from_raw_for_test(NonZeroU32::new(value).unwrap())
}

fn key(owner: SyntheticOwner, role: SyntheticRole, ordinal: u32) -> SyntheticKey {
    SyntheticKey::try_new(owner, role, ordinal).unwrap()
}

#[test]
fn paged_arena_uses_immutable_256_entry_pages_in_raw_slot_order() {
    let fixture = Fixture::new();
    let mut slots = StagedSlotTransaction::new(module(1), revision(1));
    let mut arena = StagedArena::<u32, ExprId>::new();
    let owner_id = arena
        .allocate_source(&mut slots, fixture.syntax(), fixture.source(0), u32::MAX)
        .unwrap();
    let owner = SyntheticOwner::Expr(owner_id);
    let mut children = Vec::new();
    for ordinal in 0..256 {
        children.push(
            arena
                .allocate_synthetic(
                    &mut slots,
                    key(owner, SyntheticRole::RecoveryOperand, ordinal),
                    fixture.source(1),
                    ordinal,
                )
                .unwrap(),
        );
    }

    let arena = arena.into_snapshot(&mut slots).unwrap();
    let slots = slots.commit().unwrap();
    let cloned = arena.clone();
    assert_eq!(arena.len(), 257);
    assert_eq!(arena.page_lengths().collect::<Vec<_>>(), [256, 1]);
    assert!(arena.shares_pages_with(&cloned));

    let ordered = arena
        .try_iter(&slots)
        .unwrap()
        .map(|(id, value)| (id, *value))
        .collect::<Vec<_>>();
    assert_eq!(ordered[0], (owner_id, u32::MAX));
    assert_eq!(
        ordered[1..].iter().map(|row| row.0).collect::<Vec<_>>(),
        children
    );
    assert_eq!(
        ordered[1..].iter().map(|row| row.1).collect::<Vec<_>>(),
        (0..256).collect::<Vec<_>>()
    );
}

#[test]
fn staged_resolution_reads_retained_payloads_through_current_liveness() {
    let fixture = Fixture::new();
    let owner = module(51);
    let mut first_slots = StagedSlotTransaction::new(owner, revision(1));
    let mut first_arena = StagedArena::<u32, ExprId>::new();
    let id = first_arena
        .allocate_source(&mut first_slots, fixture.syntax(), fixture.source(0), 42)
        .unwrap();
    let first_arena = first_arena.into_snapshot(&mut first_slots).unwrap();
    let first_slots = first_slots.prepare().unwrap().publish().unwrap();

    let mut second_slots = StagedSlotTransaction::from_snapshot(&first_slots, revision(2));
    let second_arena = StagedArena::from_snapshot(&first_arena);
    assert_eq!(second_arena.resolve_staged(&second_slots, id), Ok(&42));

    second_slots.retire_untouched().unwrap();
    assert!(matches!(
        second_arena.resolve_staged(&second_slots, id),
        Err(HirArenaError::Slot(HirSlotError::Resolve(
            crate::identity::IdResolveError::Retired { .. }
        )))
    ));
}

#[test]
fn same_transaction_pair_reuse_does_not_insert_or_replace_payload() {
    let fixture = Fixture::new();
    let mut slots = StagedSlotTransaction::new(module(2), revision(1));
    let mut arena = StagedArena::<&str, ExprId>::new();
    let owner_id = arena
        .allocate_source(&mut slots, fixture.syntax(), fixture.source(0), "owner")
        .unwrap();
    let synthetic = key(
        SyntheticOwner::Expr(owner_id),
        SyntheticRole::RecoveryOperand,
        0,
    );
    let first_reservation = arena
        .reserve_synthetic(&mut slots, synthetic, fixture.source(1))
        .unwrap();
    let first = first_reservation.id();
    let reused_reservation = arena
        .reserve_synthetic(&mut slots, synthetic, fixture.source(1))
        .unwrap();
    let reused = reused_reservation.id();
    assert_eq!(first, reused);
    arena
        .finalize(&mut slots, reused_reservation, "replacement-must-not-win")
        .unwrap();
    arena
        .finalize(&mut slots, first_reservation, "first")
        .unwrap();

    let arena = arena.into_snapshot(&mut slots).unwrap();
    let slots = slots.commit().unwrap();
    assert_eq!(arena.len(), 2);
    assert_eq!(arena.resolve(&slots, first), Ok(&"first"));
}

#[test]
fn scope_owned_tail_reuses_one_expr_payload_and_keeps_its_insertion() {
    let fixture = Fixture::new();
    let mut slots = StagedSlotTransaction::new(module(52), revision(1));
    let mut scopes = StagedArena::<&str, ScopeId>::new();
    let mut expressions = StagedArena::<&str, ExprId>::new();

    let scope_reservation = scopes
        .reserve_source(&mut slots, fixture.syntax(), fixture.source(0))
        .unwrap();
    let scope = scope_reservation.id();
    let tail_key = key(
        SyntheticOwner::Scope(scope),
        SyntheticRole::MissingRequiredTail,
        0,
    );
    let first_reservation = expressions
        .reserve_synthetic(&mut slots, tail_key, fixture.insertion(4))
        .unwrap();
    let tail = first_reservation.id();
    let repeated_reservation = expressions
        .reserve_synthetic(&mut slots, tail_key, fixture.insertion(4))
        .unwrap();
    assert_eq!(repeated_reservation.id(), tail);
    assert!(!repeated_reservation.is_first_touch());

    expressions
        .finalize(&mut slots, repeated_reservation, "replacement-must-not-win")
        .unwrap();
    expressions
        .finalize(&mut slots, first_reservation, "tail")
        .unwrap();
    scopes
        .finalize(&mut slots, scope_reservation, "scope")
        .unwrap();

    let expressions = expressions.into_snapshot(&mut slots).unwrap();
    let scopes = scopes.into_snapshot(&mut slots).unwrap();
    let slots = slots.commit().unwrap();
    assert_eq!(expressions.len(), 1);
    assert_eq!(scopes.len(), 1);
    assert_eq!(expressions.resolve(&slots, tail), Ok(&"tail"));
    assert_eq!(scopes.resolve(&slots, scope), Ok(&"scope"));
    let tail_metadata = slots.resolve(tail).unwrap();
    assert_eq!(tail.kind(), HirIdKind::Expr);
    assert_eq!(tail_metadata.origin(), &HirOrigin::Synthetic(tail_key));
    assert_eq!(tail_metadata.source_site(), &fixture.insertion(4));
}

#[test]
fn parent_reservation_finalizes_after_child_without_changing_slot_order() {
    let fixture = Fixture::new();
    let mut slots = StagedSlotTransaction::new(module(3), revision(1));
    let mut arena = StagedArena::<&str, ExprId>::new();
    let parent = arena
        .reserve_source(&mut slots, fixture.syntax(), fixture.source(0))
        .unwrap();
    let parent_id = parent.id();
    let child_id = arena
        .allocate_synthetic(
            &mut slots,
            key(
                SyntheticOwner::Expr(parent_id),
                SyntheticRole::ImplicitUnitTail,
                0,
            ),
            fixture.source(1),
            "child",
        )
        .unwrap();
    arena.finalize(&mut slots, parent, "parent").unwrap();

    let arena = arena.into_snapshot(&mut slots).unwrap();
    let slots = slots.commit().unwrap();
    assert_eq!(
        arena
            .try_iter(&slots)
            .unwrap()
            .map(|(id, value)| (id, *value))
            .collect::<Vec<_>>(),
        [(parent_id, "parent"), (child_id, "child")]
    );
}

#[test]
fn finalized_payload_can_close_recursive_membership_before_freeze() {
    let fixture = Fixture::new();
    let mut slots = StagedSlotTransaction::new(module(31), revision(1));
    let mut arena = StagedArena::<u32, ExprId>::new();
    let owner = arena
        .allocate_source(&mut slots, fixture.syntax(), fixture.source(0), 1)
        .unwrap();

    arena.revise_finalized(&mut slots, owner, 2).unwrap();

    let arena = arena.into_snapshot(&mut slots).unwrap();
    let slots = slots.commit().unwrap();
    assert_eq!(*arena.resolve(&slots, owner).unwrap(), 2);
}

#[test]
fn frozen_arena_resolves_only_through_the_prepared_view_before_publication() {
    let fixture = Fixture::new();
    let mut slots = StagedSlotTransaction::new(module(31), revision(1));
    let mut arena = StagedArena::<&str, ExprId>::new();
    let id = arena
        .allocate_source(&mut slots, fixture.syntax(), fixture.source(0), "prepared")
        .unwrap();
    let arena = arena.into_snapshot(&mut slots).unwrap();
    let prepared = slots.prepare().unwrap();

    assert!(matches!(
        arena.resolve(prepared.snapshot(), id),
        Err(HirArenaError::Slot(HirSlotError::OwnerNotReserved { .. }))
    ));
    assert_eq!(
        arena.resolve_prepared(prepared.snapshot(), id),
        Ok(&"prepared")
    );
    assert_eq!(
        arena
            .try_iter_prepared(prepared.snapshot())
            .unwrap()
            .map(|(entry_id, value)| (entry_id, *value))
            .collect::<Vec<_>>(),
        [(id, "prepared")]
    );

    let published = prepared.publish().unwrap();
    assert_eq!(arena.resolve(&published, id), Ok(&"prepared"));
}

#[test]
fn final_payload_is_the_only_arena_poison_authority() {
    let fixture = Fixture::new();
    let mut slots = StagedSlotTransaction::new(module(32), revision(1));
    let mut arena = StagedArena::<RecoveryRecord, ExprId>::new();
    let id = arena
        .allocate_source(
            &mut slots,
            fixture.syntax(),
            fixture.source(0),
            RecoveryRecord { poisoned: true },
        )
        .unwrap();
    let arena = arena.into_snapshot(&mut slots).unwrap();
    let prepared = slots.prepare().unwrap();

    assert!(
        arena
            .resolve_prepared(prepared.snapshot(), id)
            .unwrap()
            .is_poisoned()
    );
    assert!(
        prepared
            .snapshot()
            .resolve_prepared(id)
            .unwrap()
            .is_poisoned()
    );
}

#[test]
fn reused_poisoned_allocation_accepts_the_same_final_payload_state() {
    let fixture = Fixture::new();
    let mut slots = StagedSlotTransaction::new(module(34), revision(1));
    let mut arena = StagedArena::<RecoveryRecord, ExprId>::new();
    let first = arena
        .allocate_source(
            &mut slots,
            fixture.syntax(),
            fixture.source(0),
            RecoveryRecord { poisoned: true },
        )
        .unwrap();
    let reused = arena
        .allocate_source(
            &mut slots,
            fixture.syntax(),
            fixture.source(0),
            RecoveryRecord { poisoned: true },
        )
        .unwrap();
    assert_eq!(reused, first);
    let child_key = key(
        SyntheticOwner::Expr(first),
        SyntheticRole::RecoveryOperand,
        0,
    );
    let child = arena
        .allocate_synthetic(
            &mut slots,
            child_key,
            fixture.insertion(1),
            RecoveryRecord { poisoned: true },
        )
        .unwrap();
    let reused_child = arena
        .allocate_synthetic(
            &mut slots,
            child_key,
            fixture.insertion(1),
            RecoveryRecord { poisoned: true },
        )
        .unwrap();
    assert_eq!(reused_child, child);

    let arena = arena.into_snapshot(&mut slots).unwrap();
    let prepared = slots.prepare().unwrap();
    assert!(
        arena
            .resolve_prepared(prepared.snapshot(), reused)
            .unwrap()
            .is_poisoned()
    );
    assert!(
        arena
            .resolve_prepared(prepared.snapshot(), reused_child)
            .unwrap()
            .is_poisoned()
    );
    assert!(
        prepared
            .snapshot()
            .resolve_prepared(reused)
            .unwrap()
            .is_poisoned()
    );
    assert!(
        prepared
            .snapshot()
            .resolve_prepared(reused_child)
            .unwrap()
            .is_poisoned()
    );
}

#[test]
fn reused_allocation_rejects_a_conflicting_payload_poison_state() {
    let fixture = Fixture::new();
    let mut slots = StagedSlotTransaction::new(module(33), revision(1));
    let mut arena = StagedArena::<RecoveryRecord, ExprId>::new();
    arena
        .allocate_source(
            &mut slots,
            fixture.syntax(),
            fixture.source(0),
            RecoveryRecord { poisoned: false },
        )
        .unwrap();

    assert!(matches!(
        arena.allocate_source(
            &mut slots,
            fixture.syntax(),
            fixture.source(0),
            RecoveryRecord { poisoned: true },
        ),
        Err(HirArenaError::Slot(
            HirSlotError::ConflictingSlotView { .. }
        ))
    ));
    assert!(matches!(
        slots.prepare(),
        Err(HirSlotError::TransactionPoisoned)
    ));
}

#[test]
fn unfinalized_reservation_prevents_slot_publication() {
    let fixture = Fixture::new();
    let empty = SlotSnapshot::empty(module(4), revision(1));
    let mut slots = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let mut arena = StagedArena::<u8, ExprId>::new();
    let reservation = arena
        .reserve_source(&mut slots, fixture.syntax(), fixture.source(0))
        .unwrap();
    let id = reservation.id();
    drop(reservation);

    assert!(matches!(
        arena.into_snapshot(&mut slots),
        Err(HirArenaError::UnfinalizedReservations {
            kind: HirIdKind::Expr,
            count: 1,
        })
    ));
    assert!(matches!(
        slots.commit(),
        Err(HirSlotError::TransactionPoisoned)
    ));
    assert!(matches!(
        empty.resolve(id),
        Err(HirSlotError::OwnerNotReserved { .. })
    ));
}

#[test]
fn current_snapshot_requires_payload_coverage_for_every_live_id() {
    let fixture = Fixture::new();
    let empty = SlotSnapshot::empty(module(5), revision(1));
    let mut slots = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let id = slots
        .reserve_source::<ExprId>(fixture.syntax(), fixture.source(0), false)
        .unwrap()
        .id();
    let arena = StagedArena::<u8, ExprId>::new();

    let Err(error) = arena.into_snapshot(&mut slots) else {
        panic!("missing arena payload must prevent snapshot construction")
    };
    assert_eq!(
        error,
        HirArenaError::CoverageMismatch {
            kind: HirIdKind::Expr,
            live: 1,
            staged: 0,
        }
    );
    assert!(matches!(
        slots.commit(),
        Err(HirSlotError::TransactionPoisoned)
    ));
    assert!(matches!(
        empty.resolve(id),
        Err(HirSlotError::OwnerNotReserved { .. })
    ));
}

#[derive(PartialEq)]
struct NonCloneRecord(&'static str);

impl HirArenaPayload for NonCloneRecord {
    fn is_poisoned(&self) -> bool {
        false
    }
}

#[test]
fn retained_id_restages_non_clone_payload_and_old_snapshot_stays_immutable() {
    let fixture = Fixture::new();
    let mut first_slots = StagedSlotTransaction::new(module(6), revision(1));
    let mut first_arena = StagedArena::<NonCloneRecord, ExprId>::new();
    let first_id = first_arena
        .allocate_source(
            &mut first_slots,
            fixture.syntax(),
            fixture.source(0),
            NonCloneRecord("old"),
        )
        .unwrap();
    let first_arena = first_arena.into_snapshot(&mut first_slots).unwrap();
    let first_slots = first_slots.commit().unwrap();

    let mut second_slots = StagedSlotTransaction::from_snapshot(&first_slots, revision(2));
    let mut second_arena = StagedArena::from_snapshot(&first_arena);
    let second_id = second_arena
        .allocate_source(
            &mut second_slots,
            fixture.syntax(),
            fixture.source(1),
            NonCloneRecord("new"),
        )
        .unwrap();
    assert_eq!(first_id, second_id);
    let second_arena = second_arena.into_snapshot(&mut second_slots).unwrap();
    let second_slots = second_slots.commit().unwrap();

    assert_eq!(
        first_arena.resolve(&first_slots, first_id).unwrap().0,
        "old"
    );
    assert_eq!(
        second_arena.resolve(&second_slots, second_id).unwrap().0,
        "new"
    );
    assert!(!first_arena.shares_pages_with(&second_arena));
    let Err(error) = first_arena.resolve(&second_slots, first_id) else {
        panic!("an arena cannot resolve through another snapshot's slot view")
    };
    assert_eq!(
        error,
        HirArenaError::SnapshotMismatch {
            expected: first_slots.snapshot_id(),
            actual: second_slots.snapshot_id(),
        }
    );
}

#[derive(PartialEq)]
struct NonClonePageRecord(u32);

impl HirArenaPayload for NonClonePageRecord {
    fn is_poisoned(&self) -> bool {
        false
    }
}

#[test]
fn revision_cow_shares_unchanged_non_clone_page_and_rebuilds_changed_page() {
    let fixture = Fixture::new();
    let mut first_slots = StagedSlotTransaction::new(module(7), revision(1));
    let mut first_arena = StagedArena::<NonClonePageRecord, ExprId>::new();
    let owner_id = first_arena
        .allocate_source(
            &mut first_slots,
            fixture.syntax(),
            fixture.source(0),
            NonClonePageRecord(u32::MAX),
        )
        .unwrap();
    let owner = SyntheticOwner::Expr(owner_id);
    let mut children = Vec::new();
    for ordinal in 0..256 {
        children.push(
            first_arena
                .allocate_synthetic(
                    &mut first_slots,
                    key(owner, SyntheticRole::RecoveryOperand, ordinal),
                    fixture.source(1),
                    NonClonePageRecord(ordinal),
                )
                .unwrap(),
        );
    }
    let first_arena = first_arena.into_snapshot(&mut first_slots).unwrap();
    let first_slots = first_slots.commit().unwrap();

    let mut second_slots = StagedSlotTransaction::from_snapshot(&first_slots, revision(2));
    let mut second_arena = StagedArena::from_snapshot(&first_arena);
    assert_eq!(
        second_arena
            .allocate_source(
                &mut second_slots,
                fixture.syntax(),
                fixture.source(0),
                NonClonePageRecord(u32::MAX),
            )
            .unwrap(),
        owner_id
    );
    for ordinal in 0..256 {
        let value = if ordinal == 255 { 999 } else { ordinal };
        assert_eq!(
            second_arena
                .allocate_synthetic(
                    &mut second_slots,
                    key(owner, SyntheticRole::RecoveryOperand, ordinal),
                    fixture.source(1),
                    NonClonePageRecord(value),
                )
                .unwrap(),
            children[ordinal as usize]
        );
    }
    let second_arena = second_arena.into_snapshot(&mut second_slots).unwrap();
    let second_slots = second_slots.commit().unwrap();

    assert_eq!(first_arena.page_lengths().collect::<Vec<_>>(), [256, 1]);
    assert_eq!(second_arena.page_lengths().collect::<Vec<_>>(), [256, 1]);
    assert!(first_arena.shares_page_with(&second_arena, 0));
    assert!(!first_arena.shares_page_with(&second_arena, 1));
    assert_eq!(
        first_arena.resolve(&first_slots, children[255]).unwrap().0,
        255
    );
    assert_eq!(
        second_arena
            .resolve(&second_slots, children[255])
            .unwrap()
            .0,
        999
    );

    let mut third_slots = StagedSlotTransaction::from_snapshot(&second_slots, revision(3));
    third_slots.retire(children[255]).unwrap();
    let mut third_arena = StagedArena::from_snapshot(&second_arena);
    third_arena
        .allocate_source(
            &mut third_slots,
            fixture.syntax(),
            fixture.source(0),
            NonClonePageRecord(u32::MAX),
        )
        .unwrap();
    for ordinal in 0..255 {
        third_arena
            .allocate_synthetic(
                &mut third_slots,
                key(owner, SyntheticRole::RecoveryOperand, ordinal),
                fixture.source(1),
                NonClonePageRecord(ordinal),
            )
            .unwrap();
    }
    let third_arena = third_arena.into_snapshot(&mut third_slots).unwrap();
    let third_slots = third_slots.commit().unwrap();
    assert_eq!(third_arena.page_lengths().collect::<Vec<_>>(), [256]);
    assert!(second_arena.shares_page_with(&third_arena, 0));
    assert_eq!(
        second_arena
            .resolve(&second_slots, children[255])
            .unwrap()
            .0,
        999
    );
    assert!(matches!(
        third_arena.resolve(&third_slots, children[255]),
        Err(HirArenaError::Slot(HirSlotError::Resolve(
            crate::identity::IdResolveError::Retired { .. }
        )))
    ));
}

fn assert_typed_limit<I: HirTypedId>() {
    let fixture = Fixture::new();
    let [first_syntax, second_syntax] = fixture.syntax_ids();
    let maximum = 1;

    let mut exact_slots = StagedSlotTransaction::new(module(10), revision(1));
    let mut exact_arena = StagedArena::<u8, I>::with_maximum(maximum);
    let exact_id = exact_arena
        .allocate_source(&mut exact_slots, first_syntax, fixture.source(0), 1)
        .unwrap();
    let exact_arena = exact_arena.into_snapshot(&mut exact_slots).unwrap();
    let exact_slots = exact_slots.commit().unwrap();
    assert_eq!(exact_arena.resolve(&exact_slots, exact_id), Ok(&1));

    let empty = SlotSnapshot::empty(module(30), revision(1));
    let mut over_slots = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let mut over_arena = StagedArena::<u8, I>::with_maximum(maximum);
    let first_id = over_arena
        .allocate_source(&mut over_slots, first_syntax, fixture.source(0), 1)
        .unwrap();
    let Err(error) =
        over_arena.allocate_source(&mut over_slots, second_syntax, fixture.source(1), 2)
    else {
        panic!("one-over typed arena allocation must fail")
    };
    assert_eq!(
        error,
        HirArenaError::Limit(HirLimitError::with_maximum(
            I::KIND.allocation_limit(),
            2,
            maximum,
        ))
    );
    assert!(matches!(
        over_slots.commit(),
        Err(HirSlotError::TransactionPoisoned)
    ));
    assert!(matches!(
        empty.resolve(first_id),
        Err(HirSlotError::OwnerNotReserved { .. })
    ));
}

fn stage_capture_rows(
    count: usize,
    fixture: &Fixture,
    slots: &mut StagedSlotTransaction,
    owner_arena: &mut StagedArena<u8, ExprId>,
    capture_arena: &mut StagedArena<u8, CaptureId>,
) -> Result<(), HirArenaError> {
    let root = owner_arena.allocate_source(slots, fixture.syntax(), fixture.source(0), 0)?;
    let per_owner = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let owner_count = count.div_ceil(per_owner);
    let mut owners = Vec::with_capacity(owner_count);
    for ordinal in 0..owner_count {
        owners.push(owner_arena.allocate_synthetic(
            slots,
            key(
                SyntheticOwner::Expr(root),
                SyntheticRole::RecoveryOperand,
                u32::try_from(ordinal).unwrap(),
            ),
            fixture.insertion(0),
            0,
        )?);
    }
    for index in 0..count {
        capture_arena.allocate_synthetic(
            slots,
            key(
                SyntheticOwner::Expr(owners[index / per_owner]),
                SyntheticRole::ClosureCapture,
                u32::try_from(index % per_owner).unwrap(),
            ),
            fixture.insertion(0),
            0,
        )?;
    }
    Ok(())
}

#[test]
fn capture_arena_enforces_the_production_exact_and_one_over_limit_atomically() {
    let fixture = Fixture::new();
    let maximum = HirLimit::Captures.maximum();

    let mut exact_slots = StagedSlotTransaction::new(module(40), revision(1));
    let mut exact_owners = StagedArena::<u8, ExprId>::new();
    let mut exact_captures = StagedArena::<u8, CaptureId>::new();
    stage_capture_rows(
        maximum,
        &fixture,
        &mut exact_slots,
        &mut exact_owners,
        &mut exact_captures,
    )
    .unwrap();
    let exact_owners = exact_owners.into_snapshot(&mut exact_slots).unwrap();
    let exact_captures = exact_captures.into_snapshot(&mut exact_slots).unwrap();
    let exact_slots = exact_slots.commit().unwrap();
    assert_eq!(exact_owners.len(), 65);
    assert_eq!(exact_captures.len(), maximum);
    assert_eq!(exact_slots.committed_slot_count(), maximum + 65);

    let empty = SlotSnapshot::empty(module(41), revision(1));
    let mut over_slots = StagedSlotTransaction::from_snapshot(&empty, revision(1));
    let mut over_owners = StagedArena::<u8, ExprId>::new();
    let mut over_captures = StagedArena::<u8, CaptureId>::new();
    let error = stage_capture_rows(
        maximum + 1,
        &fixture,
        &mut over_slots,
        &mut over_owners,
        &mut over_captures,
    )
    .unwrap_err();
    assert_eq!(
        error,
        HirArenaError::Limit(HirLimitError::with_maximum(
            HirLimit::Captures,
            maximum + 1,
            maximum,
        ))
    );
    assert!(matches!(
        over_slots.commit(),
        Err(HirSlotError::TransactionPoisoned)
    ));
    assert_eq!(empty.committed_slot_count(), 0);
}

#[test]
fn every_typed_arena_enforces_its_exact_and_one_over_limit_atomically() {
    assert_eq!(HirLimit::Items.maximum(), 16_384);
    assert_eq!(HirLimit::Scopes.maximum(), 16_384);
    assert_eq!(HirLimit::LocalsPerModule.maximum(), 65_536);
    assert_eq!(HirLimit::Expressions.maximum(), 262_144);
    assert_eq!(HirLimit::Statements.maximum(), 65_536);
    assert_eq!(HirLimit::Types.maximum(), 131_072);
    assert_eq!(HirLimit::Patterns.maximum(), 131_072);
    assert_eq!(HirLimit::Captures.maximum(), 65_536);

    assert_typed_limit::<ItemId>();
    assert_typed_limit::<ScopeId>();
    assert_typed_limit::<LocalId>();
    assert_typed_limit::<ExprId>();
    assert_typed_limit::<StmtId>();
    assert_typed_limit::<TypeId>();
    assert_typed_limit::<PatternId>();
    assert_typed_limit::<CaptureId>();
}
