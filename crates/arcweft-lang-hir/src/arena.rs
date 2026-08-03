//! Immutable paged storage for typed HIR records.

use core::marker::PhantomData;
use core::num::NonZeroU32;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_lang_syntax::attachment::SyntaxNodeId;
use thiserror::Error;

use crate::identity::{
    HirIdKind, HirLimit, HirSnapshotId, HirTypedId, RawHirId, RawHirIdView, SyntheticKey,
};
use crate::lower::HirLimitError;
use crate::slot::{
    HirSlotError, HirSlotTransactionLease, SlotReservation, SlotSnapshot, StagedSlotTransaction,
};
use crate::source_index::HirSourceSite;

const ARENA_PAGE_CAPACITY: usize = 256;
const ARENA_PAGE_CAPACITY_U32: u32 = 256;

/// Semantic recovery state supplied by the final payload record.
///
/// Arena callers do not pass a second poison bit. Finalization projects the
/// state from the record that will actually be published and binds that state
/// to the slot metadata in the same transaction.
pub(crate) trait HirArenaPayload {
    fn is_poisoned(&self) -> bool;
}

const fn page_key(slot: NonZeroU32) -> u32 {
    (slot.get() - 1) / ARENA_PAGE_CAPACITY_U32
}

struct ArenaEntry<T> {
    slot: NonZeroU32,
    value: T,
}

/// One immutable, snapshot-bound arena for a single typed HIR ID kind.
pub(crate) struct ArenaSnapshot<T, I> {
    snapshot: HirSnapshotId,
    transaction: Arc<HirSlotTransactionLease>,
    pages: Arc<[Arc<[ArenaEntry<T>]>]>,
    len: u32,
    marker: PhantomData<fn() -> I>,
}

impl<T, I> Clone for ArenaSnapshot<T, I> {
    fn clone(&self) -> Self {
        Self {
            snapshot: self.snapshot,
            transaction: Arc::clone(&self.transaction),
            pages: Arc::clone(&self.pages),
            len: self.len,
            marker: PhantomData,
        }
    }
}

impl<T, I: HirTypedId> ArenaSnapshot<T, I> {
    #[cfg(test)]
    pub(crate) fn empty(slots: &SlotSnapshot) -> Self {
        Self {
            snapshot: slots.snapshot_id(),
            transaction: Arc::clone(slots.transaction_lease()),
            pages: Arc::from([]),
            len: 0,
            marker: PhantomData,
        }
    }

    pub(crate) fn resolve(&self, slots: &SlotSnapshot, id: I) -> Result<&T, HirArenaError> {
        self.validate_snapshot(slots)?;
        slots.resolve(id)?;
        self.entry(id.raw().slot())
            .map(|entry| &entry.value)
            .ok_or(HirArenaError::MissingPayload {
                id: id.raw().view(),
            })
    }

    /// Resolves a frozen arena entry against its unpublished slot proposal.
    ///
    /// The outer module transaction uses this only while validating the exact
    /// immutable module that will be published with the proposal. Ordinary
    /// readers must use [`Self::resolve`] after publication.
    pub(crate) fn resolve_prepared(
        &self,
        slots: &SlotSnapshot,
        id: I,
    ) -> Result<&T, HirArenaError> {
        self.validate_snapshot(slots)?;
        slots.resolve_prepared(id)?;
        self.entry(id.raw().slot())
            .map(|entry| &entry.value)
            .ok_or(HirArenaError::MissingPayload {
                id: id.raw().view(),
            })
    }

    pub(crate) fn try_iter<'a>(
        &'a self,
        slots: &'a SlotSnapshot,
    ) -> Result<ArenaIter<'a, T, I>, HirArenaError> {
        self.try_iter_with(slots, ArenaResolution::Published)
    }

    /// Traverses entries live in the frozen, not-yet-published slot proposal.
    pub(crate) fn try_iter_prepared<'a>(
        &'a self,
        slots: &'a SlotSnapshot,
    ) -> Result<ArenaIter<'a, T, I>, HirArenaError> {
        self.try_iter_with(slots, ArenaResolution::Prepared)
    }

    /// Validates exact bidirectional coverage against the frozen slot
    /// proposal and rejects an arena frozen by another transaction even when
    /// its numeric snapshot identity is equal.
    pub(crate) fn validates_prepared(&self, slots: &SlotSnapshot) -> bool {
        if self.validate_snapshot(slots).is_err() {
            return false;
        }
        let mut entries = BTreeSet::new();
        for entry in self.pages.iter().flat_map(|page| page.iter()) {
            let id = I::from_raw(RawHirId::new(self.snapshot.module(), entry.slot, I::KIND));
            if !entries.insert(entry.slot) || slots.resolve_prepared(id).is_err() {
                return false;
            }
        }
        if entries.len() != self.len as usize {
            return false;
        }
        let live = slots
            .prepared_live_ids::<I>()
            .map(|id| id.raw().slot())
            .collect::<BTreeSet<_>>();
        entries == live
    }

    fn try_iter_with<'a>(
        &'a self,
        slots: &'a SlotSnapshot,
        resolution: ArenaResolution,
    ) -> Result<ArenaIter<'a, T, I>, HirArenaError> {
        self.validate_snapshot(slots)?;
        let remaining = self
            .pages
            .iter()
            .flat_map(|page| page.iter())
            .filter(|entry| {
                let id = I::from_raw(RawHirId::new(self.snapshot.module(), entry.slot, I::KIND));
                resolution.resolve(slots, id).is_ok()
            })
            .count();
        Ok(ArenaIter {
            pages: &self.pages,
            slots,
            resolution,
            module: self.snapshot.module(),
            page_index: 0,
            entry_index: 0,
            remaining,
            marker: PhantomData,
        })
    }

    fn validate_snapshot(&self, slots: &SlotSnapshot) -> Result<(), HirArenaError> {
        let actual = slots.snapshot_id();
        if actual != self.snapshot {
            return Err(HirArenaError::SnapshotMismatch {
                expected: self.snapshot,
                actual,
            });
        }
        if !Arc::ptr_eq(&self.transaction, slots.transaction_lease()) {
            return Err(HirArenaError::TransactionMismatch {
                snapshot: self.snapshot,
            });
        }
        Ok(())
    }

    fn entry(&self, slot: NonZeroU32) -> Option<&ArenaEntry<T>> {
        let page_index = self.pages.partition_point(|page| {
            page.last()
                .is_some_and(|entry| entry.slot.get() < slot.get())
        });
        let page = self.pages.get(page_index)?;
        page.binary_search_by_key(&slot, |entry| entry.slot)
            .ok()
            .map(|entry_index| &page[entry_index])
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.pages.iter().map(|page| page.len()).sum()
    }

    #[cfg(test)]
    fn page_lengths(&self) -> impl Iterator<Item = usize> + '_ {
        self.pages.iter().map(|page| page.len())
    }

    #[cfg(test)]
    fn shares_pages_with(&self, other: &Self) -> bool {
        self.pages.len() == other.pages.len()
            && self
                .pages
                .iter()
                .zip(other.pages.iter())
                .all(|(left, right)| Arc::ptr_eq(left, right))
    }

    #[cfg(test)]
    fn shares_page_with(&self, other: &Self, page: usize) -> bool {
        self.pages
            .get(page)
            .zip(other.pages.get(page))
            .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
    }
}

#[derive(Clone, Copy)]
enum ArenaResolution {
    Published,
    Prepared,
}

impl ArenaResolution {
    fn resolve<I: HirTypedId>(self, slots: &SlotSnapshot, id: I) -> Result<(), HirSlotError> {
        match self {
            Self::Published => slots.resolve(id).map(|_| ()),
            Self::Prepared => slots.resolve_prepared(id).map(|_| ()),
        }
    }
}

/// Raw-slot-ordered traversal over records live in one immutable snapshot.
pub(crate) struct ArenaIter<'a, T, I> {
    pages: &'a [Arc<[ArenaEntry<T>]>],
    slots: &'a SlotSnapshot,
    resolution: ArenaResolution,
    module: crate::identity::HirModuleId,
    page_index: usize,
    entry_index: usize,
    remaining: usize,
    marker: PhantomData<fn() -> I>,
}

impl<'a, T, I: HirTypedId> Iterator for ArenaIter<'a, T, I> {
    type Item = (I, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(page) = self.pages.get(self.page_index) {
            if let Some(entry) = page.get(self.entry_index) {
                self.entry_index += 1;
                let id = I::from_raw(RawHirId::new(self.module, entry.slot, I::KIND));
                if self.resolution.resolve(self.slots, id).is_err() {
                    continue;
                }
                self.remaining -= 1;
                return Some((id, &entry.value));
            }
            self.page_index += 1;
            self.entry_index = 0;
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T, I: HirTypedId> ExactSizeIterator for ArenaIter<'_, T, I> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl<T, I: HirTypedId> core::iter::FusedIterator for ArenaIter<'_, T, I> {}

/// Transaction-private arena failure. No variant is a recoverable diagnostic.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirArenaError {
    #[error(transparent)]
    Slot(#[from] HirSlotError),
    #[error(transparent)]
    Limit(#[from] HirLimitError),
    #[error("HIR arena reservation for {id:?} is invalid")]
    InvalidReservation { id: RawHirIdView },
    #[error("HIR {kind:?} arena has {count} unfinalized reservations")]
    UnfinalizedReservations { kind: HirIdKind, count: usize },
    #[error("HIR {kind:?} arena coverage differs: {live} live slots, {staged} payloads")]
    CoverageMismatch {
        kind: HirIdKind,
        live: usize,
        staged: usize,
    },
    #[error("HIR arena belongs to snapshot {expected:?}, not {actual:?}")]
    SnapshotMismatch {
        expected: HirSnapshotId,
        actual: HirSnapshotId,
    },
    #[error("HIR arena and slot proposal were frozen by different transactions for {snapshot:?}")]
    TransactionMismatch { snapshot: HirSnapshotId },
    #[error("HIR arena has no payload for live ID {id:?}")]
    MissingPayload { id: RawHirIdView },
    #[error("HIR arena base snapshot {base:?} cannot stage snapshot {current:?}")]
    BaseSnapshotMismatch {
        base: HirSnapshotId,
        current: HirSnapshotId,
    },
    #[error("HIR arena transaction is poisoned")]
    TransactionPoisoned,
}

enum ReservationState {
    FirstTouch,
    Reused,
}

/// One typed slot reservation coupled to its arena first-touch receipt.
pub(crate) struct ArenaReservation<I> {
    id: I,
    state: ReservationState,
}

impl<I: Copy> ArenaReservation<I> {
    pub(crate) const fn id(&self) -> I {
        self.id
    }

    pub(crate) const fn is_first_touch(&self) -> bool {
        matches!(self.state, ReservationState::FirstTouch)
    }
}

/// Mutable all-or-nothing payload staging for one typed HIR arena.
pub(crate) struct StagedArena<T, I> {
    entries: BTreeMap<NonZeroU32, T>,
    reservations: BTreeSet<NonZeroU32>,
    base_snapshot: Option<HirSnapshotId>,
    base_pages: Arc<[Arc<[ArenaEntry<T>]>]>,
    limit: HirLimit,
    maximum: usize,
    poisoned: bool,
    marker: PhantomData<fn() -> I>,
}

impl<T, I: HirTypedId> StagedArena<T, I> {
    pub(crate) fn new() -> Self {
        let limit = I::KIND.allocation_limit();
        Self::with_maximum(limit.maximum())
    }

    fn with_maximum(maximum: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            reservations: BTreeSet::new(),
            base_snapshot: None,
            base_pages: Arc::from([]),
            limit: I::KIND.allocation_limit(),
            maximum,
            poisoned: false,
            marker: PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn set_maximum_for_test(&mut self, maximum: usize) {
        self.maximum = maximum;
    }

    pub(crate) fn from_snapshot(previous: &ArenaSnapshot<T, I>) -> Self {
        let mut staged = Self::new();
        staged.base_snapshot = Some(previous.snapshot);
        staged.base_pages = Arc::clone(&previous.pages);
        staged
    }

    /// Resolves a current or retained payload through the exact mutable slot
    /// proposal owned by the enclosing module transaction.
    pub(crate) fn resolve_staged(
        &self,
        slots: &StagedSlotTransaction,
        id: I,
    ) -> Result<&T, HirArenaError> {
        slots.resolve_staged(id)?;
        let slot = id.raw().slot();
        if let Some(value) = self.entries.get(&slot) {
            return Ok(value);
        }
        let page_index = self.base_pages.partition_point(|page| {
            page.last()
                .is_some_and(|entry| entry.slot.get() < slot.get())
        });
        let page = self
            .base_pages
            .get(page_index)
            .ok_or(HirArenaError::MissingPayload {
                id: id.raw().view(),
            })?;
        page.binary_search_by_key(&slot, |entry| entry.slot)
            .ok()
            .and_then(|index| page.get(index))
            .map(|entry| &entry.value)
            .ok_or(HirArenaError::MissingPayload {
                id: id.raw().view(),
            })
    }

    pub(crate) fn allocate_source(
        &mut self,
        slots: &mut StagedSlotTransaction,
        syntax: SyntaxNodeId,
        source_site: HirSourceSite,
        value: T,
    ) -> Result<I, HirArenaError>
    where
        T: HirArenaPayload,
    {
        let reservation = self.reserve_source(slots, syntax, source_site)?;
        self.finalize(slots, reservation, value)
    }

    pub(crate) fn allocate_synthetic(
        &mut self,
        slots: &mut StagedSlotTransaction,
        key: SyntheticKey,
        source_site: HirSourceSite,
        value: T,
    ) -> Result<I, HirArenaError>
    where
        T: HirArenaPayload,
    {
        let reservation = self.reserve_synthetic(slots, key, source_site)?;
        self.finalize(slots, reservation, value)
    }

    /// Reserves a source-backed ID before recursively lowering its children.
    pub(crate) fn reserve_source(
        &mut self,
        slots: &mut StagedSlotTransaction,
        syntax: SyntaxNodeId,
        source_site: HirSourceSite,
    ) -> Result<ArenaReservation<I>, HirArenaError> {
        self.ensure_open()?;
        let receipt = match slots.reserve_source::<I>(syntax, source_site, false) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.poisoned = true;
                return Err(error.into());
            }
        };
        self.record_receipt(slots, receipt)
    }

    /// Reserves a synthetic ID before recursively lowering its children.
    pub(crate) fn reserve_synthetic(
        &mut self,
        slots: &mut StagedSlotTransaction,
        key: SyntheticKey,
        source_site: HirSourceSite,
    ) -> Result<ArenaReservation<I>, HirArenaError> {
        self.ensure_open()?;
        let receipt = match slots.reserve_synthetic::<I>(key, source_site, false) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.poisoned = true;
                return Err(error.into());
            }
        };
        self.record_receipt(slots, receipt)
    }

    /// Completes one first-touch reservation with this snapshot's payload.
    ///
    /// A reuse receipt consumes and discards `value`: the pair already has one
    /// staged payload and must not be inserted twice.
    pub(crate) fn finalize(
        &mut self,
        slots: &mut StagedSlotTransaction,
        reservation: ArenaReservation<I>,
        value: T,
    ) -> Result<I, HirArenaError>
    where
        T: HirArenaPayload,
    {
        self.ensure_open()?;
        let ArenaReservation { id, state } = reservation;
        if let Err(error) = slots.bind_payload_poison(id, value.is_poisoned()) {
            return self.reject(slots, error.into());
        }
        let slot = id.raw().slot();
        match state {
            ReservationState::Reused => {
                if self.entries.contains_key(&slot) || self.reservations.contains(&slot) {
                    return Ok(id);
                }
            }
            ReservationState::FirstTouch => {
                if self.reservations.contains(&slot) && !self.entries.contains_key(&slot) {
                    self.reservations.remove(&slot);
                    self.entries.insert(slot, value);
                    return Ok(id);
                }
            }
        }
        self.reject(
            slots,
            HirArenaError::InvalidReservation {
                id: id.raw().view(),
            },
        )
    }

    /// Replaces a payload already finalized by this same unpublished
    /// transaction.
    ///
    /// Recursive owners use this only to close source-ordered membership that
    /// cannot be known until their descendants have lowered. Retained base
    /// payloads and unfinalized reservations are deliberately not mutable
    /// through this boundary.
    pub(crate) fn revise_finalized(
        &mut self,
        slots: &mut StagedSlotTransaction,
        id: I,
        value: T,
    ) -> Result<(), HirArenaError>
    where
        T: HirArenaPayload,
    {
        self.ensure_open()?;
        if let Err(error) = slots.resolve_staged(id) {
            return self.reject(slots, error.into());
        }
        let slot = id.raw().slot();
        if !self.entries.contains_key(&slot) || self.reservations.contains(&slot) {
            return self.reject(
                slots,
                HirArenaError::InvalidReservation {
                    id: id.raw().view(),
                },
            );
        }
        if let Err(error) = slots.bind_payload_poison(id, value.is_poisoned()) {
            return self.reject(slots, error.into());
        }
        self.entries.insert(slot, value);
        Ok(())
    }

    /// Validates complete current-snapshot coverage and freezes 256-entry pages.
    pub(crate) fn into_snapshot(
        mut self,
        slots: &mut StagedSlotTransaction,
    ) -> Result<ArenaSnapshot<T, I>, HirArenaError>
    where
        T: PartialEq,
    {
        self.ensure_open()?;
        if let Some(base) = self.base_snapshot {
            let current = slots.snapshot_id();
            if base.module() != current.module() || base.revision() >= current.revision() {
                let error = HirArenaError::BaseSnapshotMismatch { base, current };
                slots.poison();
                self.poisoned = true;
                return Err(error);
            }
        }
        if !self.reservations.is_empty() {
            let error = HirArenaError::UnfinalizedReservations {
                kind: I::KIND,
                count: self.reservations.len(),
            };
            slots.poison();
            self.poisoned = true;
            return Err(error);
        }

        let live = slots
            .live_ids::<I>()
            .map(|id| id.raw().slot())
            .collect::<BTreeSet<_>>();
        let staged = self.entries.keys().copied().collect::<BTreeSet<_>>();
        if live != staged {
            let error = HirArenaError::CoverageMismatch {
                kind: I::KIND,
                live: live.len(),
                staged: staged.len(),
            };
            slots.poison();
            self.poisoned = true;
            return Err(error);
        }

        let observed = self.entries.len();
        let Ok(len) = u32::try_from(self.entries.len()) else {
            return self.reject(
                slots,
                HirLimitError::with_maximum(self.limit, observed, self.maximum).into(),
            );
        };
        let mut base_pages = self
            .base_pages
            .iter()
            .filter_map(|page| {
                page.first()
                    .map(|entry| (page_key(entry.slot), Arc::clone(page)))
            })
            .collect::<BTreeMap<_, _>>();
        let mut staged_pages = BTreeMap::<u32, Vec<ArenaEntry<T>>>::new();
        for (slot, value) in self.entries {
            staged_pages
                .entry(page_key(slot))
                .or_insert_with(|| Vec::with_capacity(ARENA_PAGE_CAPACITY))
                .push(ArenaEntry { slot, value });
        }
        let mut pages = Vec::with_capacity(staged_pages.len());
        for (key, staged_page) in staged_pages {
            let previous = base_pages.remove(&key);
            let unchanged = previous.as_ref().is_some_and(|previous| {
                previous.len() == staged_page.len()
                    && previous
                        .iter()
                        .zip(staged_page.iter())
                        .all(|(previous, staged)| {
                            previous.slot == staged.slot && previous.value == staged.value
                        })
            });
            match previous {
                Some(previous) if unchanged => pages.push(previous),
                _ => pages.push(Arc::from(staged_page)),
            }
        }

        Ok(ArenaSnapshot {
            snapshot: slots.snapshot_id(),
            transaction: Arc::clone(slots.transaction_lease()),
            pages: pages.into(),
            len,
            marker: PhantomData,
        })
    }

    fn record_receipt(
        &mut self,
        slots: &mut StagedSlotTransaction,
        receipt: SlotReservation<I>,
    ) -> Result<ArenaReservation<I>, HirArenaError> {
        let id = receipt.id();
        let slot = id.raw().slot();
        if receipt.is_first_touch() {
            let observed = self
                .entries
                .len()
                .checked_add(self.reservations.len())
                .and_then(|count| count.checked_add(1))
                .unwrap_or(usize::MAX);
            if observed > self.maximum {
                return self.reject(
                    slots,
                    HirLimitError::with_maximum(self.limit, observed, self.maximum).into(),
                );
            }
            if self.entries.contains_key(&slot) || !self.reservations.insert(slot) {
                return self.reject(
                    slots,
                    HirArenaError::InvalidReservation {
                        id: id.raw().view(),
                    },
                );
            }
            return Ok(ArenaReservation {
                id,
                state: ReservationState::FirstTouch,
            });
        }

        if self.entries.contains_key(&slot) || self.reservations.contains(&slot) {
            Ok(ArenaReservation {
                id,
                state: ReservationState::Reused,
            })
        } else {
            self.reject(
                slots,
                HirArenaError::InvalidReservation {
                    id: id.raw().view(),
                },
            )
        }
    }

    const fn ensure_open(&self) -> Result<(), HirArenaError> {
        if self.poisoned {
            Err(HirArenaError::TransactionPoisoned)
        } else {
            Ok(())
        }
    }

    fn reject<R>(
        &mut self,
        slots: &mut StagedSlotTransaction,
        error: HirArenaError,
    ) -> Result<R, HirArenaError> {
        self.poisoned = true;
        slots.poison();
        Err(error)
    }
}

#[cfg(test)]
#[path = "arena/tests.rs"]
mod tests;
