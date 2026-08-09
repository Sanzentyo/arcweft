//! Transactional source-backed and synthetic HIR slot ownership.

use core::num::NonZeroU32;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use arcweft_lang_syntax::attachment::SyntaxNodeId;
use arcweft_lang_syntax::incremental::ParsedSource;
use thiserror::Error;

use crate::identity::{
    CaptureId, ExprId, HirIdKind, HirLimit, HirModuleId, HirRevision, HirSnapshotId, HirTypedId,
    IdResolveError, ItemId, LocalId, PatternId, RawHirId, RawHirIdView, ScopeId, StmtId,
    SyntheticKey, SyntheticOwner, SyntheticRole, TypeId,
};
use crate::lowering::HirLimitError;
use crate::source_index::HirSourceSite;

type SyntheticLedgerKey = (SyntheticKey, HirIdKind);

/// Opaque proof that all frozen slot and arena views came from one lowering
/// transaction. It has no serialized or semantic identity.
pub(crate) struct HirSlotTransactionLease {
    _private: (),
}

/// Exact source-backed allocation key within one HIR module.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceKey {
    syntax: SyntaxNodeId,
    kind: HirIdKind,
}

impl SourceKey {
    const fn new(syntax: SyntaxNodeId, kind: HirIdKind) -> Self {
        Self { syntax, kind }
    }

    /// Returns the attached syntax identity that owns this allocation.
    pub const fn syntax(&self) -> SyntaxNodeId {
        self.syntax
    }

    /// Returns the typed HIR arena selected by this allocation.
    pub const fn kind(&self) -> HirIdKind {
        self.kind
    }
}

/// Source-backed or lowering-synthetic origin of one HIR slot.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirOrigin {
    /// Exact attached syntax node and typed HIR arena.
    Source(SourceKey),
    /// Structurally validated lowering-synthetic identity.
    Synthetic(SyntheticKey),
}

/// Revision-bound metadata published with one HIR slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSlotMetadata {
    kind: HirIdKind,
    born: HirRevision,
    retired_at: Option<HirRevision>,
    origin: HirOrigin,
    source_site: HirSourceSite,
    poisoned: bool,
}

impl HirSlotMetadata {
    /// Returns the arena kind fixed for this slot's lifetime.
    pub const fn kind(&self) -> HirIdKind {
        self.kind
    }

    /// Returns the first revision in which this slot is live.
    pub const fn born(&self) -> HirRevision {
        self.born
    }

    /// Returns the first revision in which this slot is retired.
    pub const fn retired_at(&self) -> Option<HirRevision> {
        self.retired_at
    }

    /// Returns the exact source view retained for this snapshot.
    pub const fn source_site(&self) -> &HirSourceSite {
        &self.source_site
    }

    /// Returns whether recovery poisoned this slot for executable consumers.
    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Returns the source-backed or lowering-synthetic allocation origin.
    pub const fn origin(&self) -> &HirOrigin {
        &self.origin
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HirSlotLifetime {
    kind: HirIdKind,
    born: HirRevision,
    retired_at: Option<HirRevision>,
    origin: HirOrigin,
}

struct SharedSlotLifetimes {
    committed: RwLock<Arc<[HirSlotLifetime]>>,
}

/// Immutable test-only projection of the shared committed lifetime ledger.
#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SlotLifetimeTestState {
    committed: Arc<[HirSlotLifetime]>,
}

impl SharedSlotLifetimes {
    fn new() -> Self {
        Self {
            committed: RwLock::new(Arc::from([])),
        }
    }

    fn lifetime(&self, slot: NonZeroU32) -> Option<HirSlotLifetime> {
        let committed = match self.committed.read() {
            Ok(committed) => committed,
            Err(poisoned) => poisoned.into_inner(),
        };
        committed.get(slot.get() as usize - 1).cloned()
    }

    fn publish(&self, proposed: Arc<[HirSlotLifetime]>) -> Result<(), HirSlotError> {
        let mut committed = match self.committed.write() {
            Ok(committed) => committed,
            Err(poisoned) => poisoned.into_inner(),
        };
        validate_lifetime_update(&committed, &proposed)?;
        *committed = proposed;
        Ok(())
    }

    fn validates_publish(&self, proposed: &[HirSlotLifetime]) -> Result<(), HirSlotError> {
        let committed = match self.committed.read() {
            Ok(committed) => committed,
            Err(poisoned) => poisoned.into_inner(),
        };
        validate_lifetime_update(&committed, proposed)
    }

    fn len(&self) -> usize {
        let committed = match self.committed.read() {
            Ok(committed) => committed,
            Err(poisoned) => poisoned.into_inner(),
        };
        committed.len()
    }

    #[cfg(test)]
    fn test_state(&self) -> SlotLifetimeTestState {
        let committed = match self.committed.read() {
            Ok(committed) => Arc::clone(&committed),
            Err(poisoned) => Arc::clone(&poisoned.into_inner()),
        };
        SlotLifetimeTestState { committed }
    }
}

fn validate_lifetime_update(
    committed: &[HirSlotLifetime],
    proposed: &[HirSlotLifetime],
) -> Result<(), HirSlotError> {
    if proposed.len() < committed.len() {
        return Err(HirSlotError::CommitConflict);
    }

    if proposed.iter().any(|lifetime| {
        lifetime
            .retired_at
            .is_some_and(|retired| retired <= lifetime.born)
    }) {
        return Err(HirSlotError::InvalidRetirement);
    }

    for (committed, proposed) in committed.iter().zip(proposed) {
        if committed.kind != proposed.kind
            || committed.born != proposed.born
            || committed.origin != proposed.origin
        {
            return Err(HirSlotError::CommitConflict);
        }
        match (committed.retired_at, proposed.retired_at) {
            (Some(previous), Some(current)) if previous == current => {}
            (None, _) => {}
            _ => return Err(HirSlotError::CommitConflict),
        }
    }

    Ok(())
}

/// Typed failure from a synthetic-slot transaction.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirSlotError {
    #[error(transparent)]
    Resolve(#[from] IdResolveError),
    #[error("HIR owner {id:?} is not committed or reserved by this transaction")]
    OwnerNotReserved { id: RawHirIdView },
    #[error("HIR slot {id:?} received conflicting source or poison views in one transaction")]
    ConflictingSlotView { id: RawHirIdView },
    #[error("HIR snapshot metadata does not match the committed lifetime for slot {id:?}")]
    MetadataMismatch { id: RawHirIdView },
    #[error("synthetic role {role:?} requires an arena child and cannot use key-only staging")]
    InvalidKeyOnlyRole { role: SyntheticRole },
    #[error(transparent)]
    Limit(#[from] HirLimitError),
    #[error("HIR slot identity allocation is exhausted for module {module:?} and kind {kind:?}")]
    SlotIdentityExhausted {
        module: HirModuleId,
        kind: HirIdKind,
    },
    #[error("HIR slot retirement interval is invalid")]
    InvalidRetirement,
    #[error("HIR slot transaction conflicts with a newer committed lifetime ledger")]
    CommitConflict,
    #[error("HIR slot transaction is poisoned")]
    TransactionPoisoned,
}

/// Result of reserving one typed slot in the current lowering transaction.
///
/// A first touch requires the owning typed arena to stage this snapshot's
/// payload. Reuse identifies the same source/synthetic pair after that first
/// touch and must not insert a second payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SlotReservation<I> {
    FirstTouch(I),
    Reused(I),
}

/// Receipt for a synthetic identity that has no independent arena child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyOnlyReservation {
    FirstTouch,
    Reused,
}

impl<I: Copy> SlotReservation<I> {
    pub(crate) const fn id(self) -> I {
        match self {
            Self::FirstTouch(id) | Self::Reused(id) => id,
        }
    }

    pub(crate) const fn is_first_touch(self) -> bool {
        matches!(self, Self::FirstTouch(_))
    }
}

/// One immutable snapshot over the module-global slot lifetime registry.
#[derive(Clone)]
pub(crate) struct SlotSnapshot {
    snapshot: HirSnapshotId,
    transaction: Arc<HirSlotTransactionLease>,
    metadata: Arc<[HirSlotMetadata]>,
    source: Arc<BTreeMap<SourceKey, RawHirId>>,
    synthetic: Arc<BTreeMap<SyntheticLedgerKey, RawHirId>>,
    key_only_synthetic: Arc<BTreeSet<SyntheticKey>>,
    lifetimes: Arc<SharedSlotLifetimes>,
}

impl SlotSnapshot {
    pub(crate) fn empty(module: HirModuleId, revision: HirRevision) -> Self {
        Self {
            snapshot: HirSnapshotId::new(module, revision),
            transaction: Arc::new(HirSlotTransactionLease { _private: () }),
            metadata: Arc::from([]),
            source: Arc::new(BTreeMap::new()),
            synthetic: Arc::new(BTreeMap::new()),
            key_only_synthetic: Arc::new(BTreeSet::new()),
            lifetimes: Arc::new(SharedSlotLifetimes::new()),
        }
    }

    pub(crate) const fn snapshot_id(&self) -> HirSnapshotId {
        self.snapshot
    }

    pub(crate) const fn transaction_lease(&self) -> &Arc<HirSlotTransactionLease> {
        &self.transaction
    }

    pub(crate) fn prepared_live_ids<I: HirTypedId>(&self) -> impl Iterator<Item = I> + '_ {
        self.metadata
            .iter()
            .enumerate()
            .filter(|(_, metadata)| {
                metadata.kind == I::KIND
                    && metadata.born <= self.snapshot.revision()
                    && metadata
                        .retired_at
                        .is_none_or(|retired_at| self.snapshot.revision() < retired_at)
            })
            .filter_map(|(index, _)| {
                let slot = u32::try_from(index + 1).ok().and_then(NonZeroU32::new)?;
                Some(I::from_raw(RawHirId::new(
                    self.snapshot.module(),
                    slot,
                    I::KIND,
                )))
            })
    }

    /// Resolves the live typed HIR owner allocated from one exact syntax node
    /// in this frozen proposal.
    pub(crate) fn prepared_source_owner<I: HirTypedId>(&self, syntax: SyntaxNodeId) -> Option<I> {
        let raw = self.source.get(&SourceKey::new(syntax, I::KIND)).copied()?;
        let owner = I::from_raw(raw);
        self.resolve_prepared(owner).ok().map(|_| owner)
    }

    /// Validates every live slot against the exact attached syntax and source
    /// revision retained by the module being frozen.
    pub(crate) fn validates_provenance(&self, parsed: &ParsedSource) -> bool {
        self.metadata
            .iter()
            .filter(|metadata| {
                metadata.born <= self.snapshot.revision()
                    && metadata
                        .retired_at
                        .is_none_or(|retired_at| self.snapshot.revision() < retired_at)
            })
            .all(|metadata| {
                metadata.source_site.source_identity() == parsed.document().identity()
                    && match &metadata.origin {
                        HirOrigin::Source(key) => parsed.syntax_node(key.syntax()).is_ok(),
                        HirOrigin::Synthetic(_) => true,
                    }
            })
    }

    pub(crate) fn has_poisoned_live_slots(&self) -> bool {
        self.metadata.iter().any(|metadata| {
            metadata.poisoned
                && metadata.born <= self.snapshot.revision()
                && metadata
                    .retired_at
                    .is_none_or(|retired_at| self.snapshot.revision() < retired_at)
        })
    }

    pub(crate) fn poisoned_live_owners(&self) -> impl Iterator<Item = SyntheticOwner> + '_ {
        self.metadata
            .iter()
            .enumerate()
            .filter(|(_, metadata)| {
                metadata.poisoned
                    && metadata.born <= self.snapshot.revision()
                    && metadata
                        .retired_at
                        .is_none_or(|retired_at| self.snapshot.revision() < retired_at)
            })
            .filter_map(|(index, metadata)| {
                owner_for_slot(self.snapshot.module(), index, metadata.kind)
            })
    }

    pub(crate) fn resolve<I: HirTypedId>(&self, id: I) -> Result<&HirSlotMetadata, HirSlotError> {
        let raw = id.raw();
        let lifetime = resolve_lifetime(
            self.snapshot,
            raw,
            I::KIND,
            self.lifetimes.lifetime(raw.slot()),
        )?;

        let metadata = self
            .metadata
            .get(raw.slot().get() as usize - 1)
            .ok_or(HirSlotError::OwnerNotReserved { id: raw.view() })?;
        if metadata.kind != lifetime.kind
            || metadata.born != lifetime.born
            || metadata.origin != lifetime.origin
        {
            return Err(HirSlotError::MetadataMismatch { id: raw.view() });
        }
        Ok(metadata)
    }

    /// Resolves against the frozen proposal itself before its shared lifetime
    /// ledger becomes observable. This is used only by the outer module
    /// transaction's final validation.
    pub(crate) fn resolve_prepared<I: HirTypedId>(
        &self,
        id: I,
    ) -> Result<&HirSlotMetadata, HirSlotError> {
        let raw = id.raw();
        if raw.module() != self.snapshot.module() {
            return Err(IdResolveError::WrongModule {
                expected: self.snapshot.module(),
                actual: raw.module(),
            }
            .into());
        }
        let metadata = self
            .metadata
            .get(raw.slot().get() as usize - 1)
            .ok_or(HirSlotError::OwnerNotReserved { id: raw.view() })?;
        if self.snapshot.revision() < metadata.born {
            return Err(IdResolveError::NotYetLive {
                id: raw.view(),
                snapshot: self.snapshot,
                born: metadata.born,
            }
            .into());
        }
        if let Some(retired_at) = metadata.retired_at
            && retired_at <= self.snapshot.revision()
        {
            return Err(IdResolveError::Retired {
                id: raw.view(),
                snapshot: self.snapshot,
                retired_at,
            }
            .into());
        }
        if metadata.kind != I::KIND {
            return Err(IdResolveError::KindMismatch {
                id: raw.view(),
                expected: I::KIND,
                actual: metadata.kind,
            }
            .into());
        }
        Ok(metadata)
    }

    /// Resolves the typed arena ID owned by one prepared synthetic key.
    ///
    /// Absence is distinct from a malformed or foreign retained slot so final
    /// manifest validation can reject missing recovery children without
    /// scanning arena order.
    pub(crate) fn resolve_prepared_synthetic<I: HirTypedId>(
        &self,
        key: SyntheticKey,
    ) -> Result<Option<I>, HirSlotError> {
        let Some(raw) = self.synthetic.get(&(key, I::KIND)).copied() else {
            return Ok(None);
        };
        let id = I::from_raw(raw);
        self.resolve_prepared(id)?;
        Ok(Some(id))
    }

    #[cfg(test)]
    pub(crate) fn committed_slot_count(&self) -> usize {
        self.lifetimes.len()
    }

    #[cfg(test)]
    pub(crate) fn lifetime_test_state(&self) -> SlotLifetimeTestState {
        self.lifetimes.test_state()
    }

    #[cfg(test)]
    fn synthetic_pair_count(&self) -> usize {
        self.synthetic.len()
    }

    pub(crate) fn contains_key_only_synthetic(&self, key: SyntheticKey) -> bool {
        self.key_only_synthetic.contains(&key)
    }

    pub(crate) fn key_only_synthetic_keys(&self) -> impl Iterator<Item = SyntheticKey> + '_ {
        self.key_only_synthetic.iter().copied()
    }

    #[cfg(test)]
    fn key_only_synthetic_count(&self) -> usize {
        self.key_only_synthetic.len()
    }

    #[cfg(test)]
    fn source_key_count(&self) -> usize {
        self.source.len()
    }

    #[cfg(test)]
    fn corrupt_metadata_kind(&mut self, id: RawHirId, kind: HirIdKind) {
        if let Some(metadata) =
            Arc::make_mut(&mut self.metadata).get_mut(id.slot().get() as usize - 1)
        {
            metadata.kind = kind;
        }
    }
}

fn resolve_lifetime(
    snapshot: HirSnapshotId,
    raw: RawHirId,
    expected: HirIdKind,
    lifetime: Option<HirSlotLifetime>,
) -> Result<HirSlotLifetime, HirSlotError> {
    if raw.module() != snapshot.module() {
        return Err(IdResolveError::WrongModule {
            expected: snapshot.module(),
            actual: raw.module(),
        }
        .into());
    }

    let Some(lifetime) = lifetime else {
        return Err(HirSlotError::OwnerNotReserved { id: raw.view() });
    };
    if snapshot.revision() < lifetime.born {
        return Err(IdResolveError::NotYetLive {
            id: raw.view(),
            snapshot,
            born: lifetime.born,
        }
        .into());
    }
    if let Some(retired_at) = lifetime.retired_at
        && retired_at <= snapshot.revision()
    {
        return Err(IdResolveError::Retired {
            id: raw.view(),
            snapshot,
            retired_at,
        }
        .into());
    }
    if lifetime.kind != expected {
        return Err(IdResolveError::KindMismatch {
            id: raw.view(),
            expected,
            actual: lifetime.kind,
        }
        .into());
    }

    Ok(lifetime)
}

fn owner_for_slot(module: HirModuleId, index: usize, kind: HirIdKind) -> Option<SyntheticOwner> {
    let slot = u32::try_from(index + 1).ok().and_then(NonZeroU32::new)?;
    let raw = RawHirId::new(module, slot, kind);
    Some(match kind {
        HirIdKind::Item => SyntheticOwner::Item(ItemId::from_raw(raw)),
        HirIdKind::Scope => SyntheticOwner::Scope(ScopeId::from_raw(raw)),
        HirIdKind::Local => SyntheticOwner::Local(LocalId::from_raw(raw)),
        HirIdKind::Expr => SyntheticOwner::Expr(ExprId::from_raw(raw)),
        HirIdKind::Stmt => SyntheticOwner::Stmt(StmtId::from_raw(raw)),
        HirIdKind::Type => SyntheticOwner::Type(TypeId::from_raw(raw)),
        HirIdKind::Pattern => SyntheticOwner::Pattern(PatternId::from_raw(raw)),
        HirIdKind::Capture => SyntheticOwner::Capture(CaptureId::from_raw(raw)),
    })
}

fn raw_for_owner(owner: SyntheticOwner) -> RawHirId {
    match owner {
        SyntheticOwner::Item(id) => id.raw(),
        SyntheticOwner::Scope(id) => id.raw(),
        SyntheticOwner::Local(id) => id.raw(),
        SyntheticOwner::Expr(id) => id.raw(),
        SyntheticOwner::Stmt(id) => id.raw(),
        SyntheticOwner::Type(id) => id.raw(),
        SyntheticOwner::Pattern(id) => id.raw(),
        SyntheticOwner::Capture(id) => id.raw(),
    }
}

/// Mutable all-or-nothing slot allocation transaction.
pub(crate) struct StagedSlotTransaction {
    snapshot: HirSnapshotId,
    base_snapshot: Option<HirSnapshotId>,
    transaction: Arc<HirSlotTransactionLease>,
    base_len: usize,
    metadata: Vec<HirSlotMetadata>,
    source: BTreeMap<SourceKey, RawHirId>,
    synthetic: BTreeMap<SyntheticLedgerKey, RawHirId>,
    key_only_synthetic: BTreeSet<SyntheticKey>,
    synthetic_counts: BTreeMap<SyntheticOwner, usize>,
    touched_source: BTreeSet<SourceKey>,
    touched_synthetic: BTreeSet<SyntheticLedgerKey>,
    touched_key_only_synthetic: BTreeSet<SyntheticKey>,
    payload_poison: BTreeMap<RawHirId, bool>,
    lifetimes: Arc<SharedSlotLifetimes>,
    #[cfg(test)]
    total_slot_maximum: usize,
    #[cfg(test)]
    next_slot_identity: Option<u64>,
    poisoned: bool,
}

/// Side-effect-free frozen slot proposal awaiting the database's final
/// publication mutation.
///
/// Dropping this value publishes neither liveness nor snapshot state. The
/// outer lowering transaction validates every arena, source component, and
/// module field before consuming it through [`Self::publish`].
pub(crate) struct PreparedSlotCommit {
    snapshot: Arc<SlotSnapshot>,
    base_snapshot: Option<HirSnapshotId>,
    proposed_lifetimes: Arc<[HirSlotLifetime]>,
}

impl PreparedSlotCommit {
    pub(crate) const fn snapshot(&self) -> &Arc<SlotSnapshot> {
        &self.snapshot
    }

    /// Verifies that this proposal was staged from the database's exact
    /// currently accepted slot snapshot. Snapshot identity rejects stale
    /// ancestry; the shared-lifetime lease rejects an unrelated ledger that
    /// happens to reuse the same module and revision values.
    pub(crate) fn validates_ancestry(&self, previous: Option<&SlotSnapshot>) -> bool {
        match (self.base_snapshot, previous) {
            (None, None) => self.snapshot.lifetimes.len() == 0,
            (Some(base), Some(previous)) => {
                base == previous.snapshot_id()
                    && Arc::ptr_eq(&self.snapshot.lifetimes, &previous.lifetimes)
            }
            _ => false,
        }
    }

    pub(crate) fn validate_publish(&self) -> Result<(), HirSlotError> {
        self.snapshot
            .lifetimes
            .validates_publish(&self.proposed_lifetimes)
    }

    pub(crate) fn publish(self) -> Result<Arc<SlotSnapshot>, HirSlotError> {
        self.snapshot.lifetimes.publish(self.proposed_lifetimes)?;
        Ok(self.snapshot)
    }
}

impl StagedSlotTransaction {
    pub(crate) fn new(module: HirModuleId, revision: HirRevision) -> Self {
        Self::from_base(&SlotSnapshot::empty(module, revision), revision, None)
    }

    pub(crate) fn from_snapshot(snapshot: &SlotSnapshot, revision: HirRevision) -> Self {
        Self::from_base(snapshot, revision, Some(snapshot.snapshot_id()))
    }

    fn from_base(
        snapshot: &SlotSnapshot,
        revision: HirRevision,
        base_snapshot: Option<HirSnapshotId>,
    ) -> Self {
        let source = snapshot.source.as_ref().clone();
        let synthetic = snapshot.synthetic.as_ref().clone();
        let key_only_synthetic = snapshot.key_only_synthetic.as_ref().clone();
        let synthetic_counts = synthetic
            .keys()
            .map(|(key, _)| *key)
            .chain(key_only_synthetic.iter().copied())
            .fold(BTreeMap::new(), |mut counts, key| {
                *counts.entry(key.owner()).or_insert(0) += 1;
                counts
            });
        Self {
            snapshot: HirSnapshotId::new(snapshot.snapshot.module(), revision),
            base_snapshot,
            transaction: Arc::new(HirSlotTransactionLease { _private: () }),
            base_len: snapshot.metadata.len(),
            metadata: snapshot.metadata.to_vec(),
            source,
            synthetic,
            key_only_synthetic,
            synthetic_counts,
            touched_source: BTreeSet::new(),
            touched_synthetic: BTreeSet::new(),
            touched_key_only_synthetic: BTreeSet::new(),
            payload_poison: BTreeMap::new(),
            lifetimes: Arc::clone(&snapshot.lifetimes),
            #[cfg(test)]
            total_slot_maximum: HirLimit::TotalSlotsPerModule.maximum(),
            #[cfg(test)]
            next_slot_identity: None,
            poisoned: false,
        }
    }

    /// Lowers the real total-slot owner for a bounded production-path test.
    ///
    /// This changes neither the committed count nor any allocation result by
    /// itself. Tests must still reach the boundary through ordinary typed slot
    /// reservations and revision retirement.
    #[cfg(test)]
    pub(crate) fn set_total_slot_maximum_for_test(&mut self, maximum: usize) {
        assert!(maximum <= HirLimit::TotalSlotsPerModule.maximum());
        assert!(self.metadata.len() <= maximum);
        self.total_slot_maximum = maximum;
    }

    /// Seeds only the next raw identity conversion with the first value that
    /// cannot be represented by the production `NonZeroU32` slot owner.
    ///
    /// The allocator still performs ordinary total-slot accounting, error
    /// construction, and transaction poisoning. The hook cannot manufacture
    /// a successful or non-contiguous ID.
    #[cfg(test)]
    pub(crate) fn exhaust_next_slot_identity_for_test(&mut self) {
        self.next_slot_identity = Some(u64::from(u32::MAX) + 1);
    }

    pub(crate) const fn transaction_lease(&self) -> &Arc<HirSlotTransactionLease> {
        &self.transaction
    }

    pub(crate) fn reserve_source<I: HirTypedId>(
        &mut self,
        syntax: SyntaxNodeId,
        source_site: HirSourceSite,
        poisoned: bool,
    ) -> Result<SlotReservation<I>, HirSlotError> {
        self.ensure_open()?;
        let key = SourceKey::new(syntax, I::KIND);
        if let Some(raw) = self.source.get(&key).copied() {
            let touched = self.touched_source.contains(&key);
            if let Err(error) = self.resolve_raw(raw, I::KIND) {
                return self.reject(error);
            }
            if let Err(error) = self.refresh_or_validate_view(raw, source_site, poisoned, touched) {
                return self.reject(error);
            }
            self.touched_source.insert(key);
            let id = I::from_raw(raw);
            return Ok(if touched {
                SlotReservation::Reused(id)
            } else {
                SlotReservation::FirstTouch(id)
            });
        }

        let origin = HirOrigin::Source(key.clone());
        let raw = match self.allocate(I::KIND, origin, source_site, poisoned) {
            Ok(raw) => raw,
            Err(error) => return self.reject(error),
        };
        self.source.insert(key.clone(), raw);
        self.touched_source.insert(key);
        Ok(SlotReservation::FirstTouch(I::from_raw(raw)))
    }

    pub(crate) fn reserve_synthetic<I: HirTypedId>(
        &mut self,
        key: SyntheticKey,
        source_site: HirSourceSite,
        poisoned: bool,
    ) -> Result<SlotReservation<I>, HirSlotError> {
        self.ensure_open()?;
        if let Err(error) = self.resolve_owner(key.owner()) {
            return self.reject(error);
        }

        let ledger_key = (key, I::KIND);
        if let Some(raw) = self.synthetic.get(&ledger_key).copied() {
            let touched = self.touched_synthetic.contains(&ledger_key);
            if let Err(error) = self.resolve_raw(raw, I::KIND) {
                if matches!(
                    error,
                    HirSlotError::Resolve(IdResolveError::Retired {
                        id,
                        retired_at,
                        ..
                    }) if id == raw.view() && retired_at < self.snapshot.revision()
                ) {
                    let replacement = match self.allocate(
                        I::KIND,
                        HirOrigin::Synthetic(key),
                        source_site,
                        poisoned,
                    ) {
                        Ok(replacement) => replacement,
                        Err(error) => return self.reject(error),
                    };
                    self.synthetic.insert(ledger_key, replacement);
                    self.touched_synthetic.insert(ledger_key);
                    return Ok(SlotReservation::FirstTouch(I::from_raw(replacement)));
                }
                return self.reject(error);
            }
            if let Err(error) = self.refresh_or_validate_view(raw, source_site, poisoned, touched) {
                return self.reject(error);
            }
            self.touched_synthetic.insert(ledger_key);
            let id = I::from_raw(raw);
            return Ok(if touched {
                SlotReservation::Reused(id)
            } else {
                SlotReservation::FirstTouch(id)
            });
        }

        let current = self
            .synthetic_counts
            .get(&key.owner())
            .copied()
            .unwrap_or(0);
        let Some(observed) = current.checked_add(1) else {
            let limit = HirLimit::SyntheticDescendantsPerOwner;
            return self
                .reject(HirLimitError::with_maximum(limit, usize::MAX, limit.maximum()).into());
        };
        let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
        if observed > maximum {
            return self.reject(
                HirLimitError::with_maximum(
                    HirLimit::SyntheticDescendantsPerOwner,
                    observed,
                    maximum,
                )
                .into(),
            );
        }

        let raw = match self.allocate(I::KIND, HirOrigin::Synthetic(key), source_site, poisoned) {
            Ok(raw) => raw,
            Err(error) => return self.reject(error),
        };
        self.synthetic.insert(ledger_key, raw);
        self.touched_synthetic.insert(ledger_key);
        self.synthetic_counts.insert(key.owner(), observed);
        Ok(SlotReservation::FirstTouch(I::from_raw(raw)))
    }

    /// Stages the TypeId-owned elided-region key that has no arena child.
    ///
    /// This entry shares typed-owner liveness and aggregate descendant
    /// accounting with `(SyntheticKey, HirIdKind)` child slots. The distinct
    /// set records that no child kind or raw slot exists for this identity.
    pub(crate) fn stage_elided_region_key(
        &mut self,
        key: SyntheticKey,
    ) -> Result<KeyOnlyReservation, HirSlotError> {
        self.ensure_open()?;
        if key.role() != SyntheticRole::ElidedRegion {
            return self.reject(HirSlotError::InvalidKeyOnlyRole { role: key.role() });
        }
        if let Err(error) = self.resolve_owner(key.owner()) {
            return self.reject(error);
        }
        if self.key_only_synthetic.contains(&key) {
            let first_touch = self.touched_key_only_synthetic.insert(key);
            return Ok(if first_touch {
                KeyOnlyReservation::FirstTouch
            } else {
                KeyOnlyReservation::Reused
            });
        }

        let current = self
            .synthetic_counts
            .get(&key.owner())
            .copied()
            .unwrap_or(0);
        let Some(observed) = current.checked_add(1) else {
            let limit = HirLimit::SyntheticDescendantsPerOwner;
            return self
                .reject(HirLimitError::with_maximum(limit, usize::MAX, limit.maximum()).into());
        };
        let limit = HirLimit::SyntheticDescendantsPerOwner;
        let maximum = limit.maximum();
        if observed > maximum {
            return self.reject(HirLimitError::with_maximum(limit, observed, maximum).into());
        }

        self.key_only_synthetic.insert(key);
        self.touched_key_only_synthetic.insert(key);
        self.synthetic_counts.insert(key.owner(), observed);
        Ok(KeyOnlyReservation::FirstTouch)
    }

    pub(crate) const fn snapshot_id(&self) -> HirSnapshotId {
        self.snapshot
    }

    pub(crate) fn live_ids<I: HirTypedId>(&self) -> impl Iterator<Item = I> + '_ {
        self.metadata
            .iter()
            .enumerate()
            .filter(|(_, metadata)| {
                metadata.kind == I::KIND
                    && metadata.born <= self.snapshot.revision()
                    && metadata
                        .retired_at
                        .is_none_or(|retired_at| self.snapshot.revision() < retired_at)
            })
            .filter_map(|(index, _)| {
                u32::try_from(index + 1)
                    .ok()
                    .and_then(NonZeroU32::new)
                    .map(|slot| I::from_raw(RawHirId::new(self.snapshot.module(), slot, I::KIND)))
            })
    }

    /// Resolves one typed ID against this transaction's current proposal.
    ///
    /// Unlike the immutable snapshot reader, this observes same-transaction
    /// reservations and retirements without consulting or mutating the shared
    /// committed lifetime ledger.
    pub(crate) fn resolve_staged<I: HirTypedId>(
        &self,
        id: I,
    ) -> Result<&HirSlotMetadata, HirSlotError> {
        self.ensure_open()?;
        let raw = id.raw();
        if raw.module() != self.snapshot.module() {
            return Err(IdResolveError::WrongModule {
                expected: self.snapshot.module(),
                actual: raw.module(),
            }
            .into());
        }
        let metadata = self
            .metadata
            .get(raw.slot().get() as usize - 1)
            .ok_or(HirSlotError::OwnerNotReserved { id: raw.view() })?;
        if self.snapshot.revision() < metadata.born {
            return Err(IdResolveError::NotYetLive {
                id: raw.view(),
                snapshot: self.snapshot,
                born: metadata.born,
            }
            .into());
        }
        if let Some(retired_at) = metadata.retired_at
            && retired_at <= self.snapshot.revision()
        {
            return Err(IdResolveError::Retired {
                id: raw.view(),
                snapshot: self.snapshot,
                retired_at,
            }
            .into());
        }
        if metadata.kind != I::KIND {
            return Err(IdResolveError::KindMismatch {
                id: raw.view(),
                expected: I::KIND,
                actual: metadata.kind,
            }
            .into());
        }
        Ok(metadata)
    }

    pub(crate) fn poison(&mut self) {
        self.poisoned = true;
    }

    /// Binds slot recovery state to the final semantic payload exactly once.
    pub(crate) fn bind_payload_poison<I: HirTypedId>(
        &mut self,
        id: I,
        poisoned: bool,
    ) -> Result<(), HirSlotError> {
        self.ensure_open()?;
        let raw = id.raw();
        if let Err(error) = self.resolve_raw(raw, I::KIND) {
            return self.reject(error);
        }
        if let Some(previous) = self.payload_poison.get(&raw).copied() {
            if previous != poisoned {
                return self.reject(HirSlotError::ConflictingSlotView { id: raw.view() });
            }
            return Ok(());
        }
        let Some(metadata) = self.metadata.get_mut(raw.slot().get() as usize - 1) else {
            return self.reject(HirSlotError::OwnerNotReserved { id: raw.view() });
        };
        metadata.poisoned = poisoned;
        self.payload_poison.insert(raw, poisoned);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn retire<I: HirTypedId>(&mut self, id: I) -> Result<(), HirSlotError> {
        self.ensure_open()?;
        let raw = id.raw();
        if let Err(error) = self.resolve_raw(raw, I::KIND) {
            return self.reject(error);
        }
        let Some(metadata) = self.metadata.get_mut(raw.slot().get() as usize - 1) else {
            return self.reject(HirSlotError::OwnerNotReserved { id: raw.view() });
        };
        if self.snapshot.revision() <= metadata.born || metadata.retired_at.is_some() {
            return self.reject(HirSlotError::InvalidRetirement);
        }
        metadata.retired_at = Some(self.snapshot.revision());
        Ok(())
    }

    /// Retires every previously live allocation not reached by the current
    /// direct lowering traversal, then cascades retirement to any synthetic
    /// child whose exact typed owner was retired in this revision.
    pub(crate) fn retire_untouched(&mut self) -> Result<Box<[SyntheticOwner]>, HirSlotError> {
        self.ensure_open()?;
        let touched = self
            .touched_source
            .iter()
            .filter_map(|key| self.source.get(key).copied())
            .chain(
                self.touched_synthetic
                    .iter()
                    .filter_map(|key| self.synthetic.get(key).copied()),
            )
            .collect::<BTreeSet<_>>();
        let revision = self.snapshot.revision();

        for index in 0..self.base_len {
            let kind = self.metadata[index].kind;
            let Some(owner) = owner_for_slot(self.snapshot.module(), index, kind) else {
                return self.reject(HirSlotError::SlotIdentityExhausted {
                    module: self.snapshot.module(),
                    kind,
                });
            };
            let raw = raw_for_owner(owner);
            let born = self.metadata[index].born;
            let should_retire =
                self.metadata[index].retired_at.is_none() && !touched.contains(&raw);
            if should_retire {
                if revision <= born {
                    return self.reject(HirSlotError::InvalidRetirement);
                }
                self.metadata[index].retired_at = Some(revision);
            }
        }

        loop {
            let retired_raw = self
                .metadata
                .iter()
                .enumerate()
                .filter(|(_, metadata)| metadata.retired_at == Some(revision))
                .filter_map(|(index, metadata)| {
                    owner_for_slot(self.snapshot.module(), index, metadata.kind).map(raw_for_owner)
                })
                .collect::<BTreeSet<_>>();
            let mut changed = false;
            for metadata in &mut self.metadata {
                if metadata.retired_at.is_some() {
                    continue;
                }
                let HirOrigin::Synthetic(key) = &metadata.origin else {
                    continue;
                };
                if retired_raw.contains(&raw_for_owner(key.owner())) {
                    metadata.retired_at = Some(revision);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        self.key_only_synthetic.retain(|key| {
            self.touched_key_only_synthetic.contains(key)
                && self
                    .metadata
                    .get(raw_for_owner(key.owner()).slot().get() as usize - 1)
                    .is_some_and(|metadata| metadata.retired_at.is_none())
        });

        Ok(self
            .metadata
            .iter()
            .enumerate()
            .filter(|(_, metadata)| metadata.retired_at == Some(revision))
            .filter_map(|(index, metadata)| {
                owner_for_slot(self.snapshot.module(), index, metadata.kind)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }

    /// Freezes the proposed slot snapshot without updating the shared lifetime
    /// ledger. Only the database's final module publication may consume the
    /// returned proposal.
    pub(crate) fn prepare(self) -> Result<PreparedSlotCommit, HirSlotError> {
        if self.poisoned {
            return Err(HirSlotError::TransactionPoisoned);
        }

        let proposed: Arc<[HirSlotLifetime]> = self
            .metadata
            .iter()
            .map(|metadata| HirSlotLifetime {
                kind: metadata.kind,
                born: metadata.born,
                retired_at: metadata.retired_at,
                origin: metadata.origin.clone(),
            })
            .collect::<Vec<_>>()
            .into();
        Ok(PreparedSlotCommit {
            snapshot: Arc::new(SlotSnapshot {
                snapshot: self.snapshot,
                transaction: self.transaction,
                metadata: self.metadata.into(),
                source: Arc::new(self.source),
                synthetic: Arc::new(self.synthetic),
                key_only_synthetic: Arc::new(self.key_only_synthetic),
                lifetimes: self.lifetimes,
            }),
            base_snapshot: self.base_snapshot,
            proposed_lifetimes: proposed,
        })
    }

    #[cfg(test)]
    pub(crate) fn commit(self) -> Result<SlotSnapshot, HirSlotError> {
        self.prepare()?.publish().map(Arc::unwrap_or_clone)
    }

    fn resolve_owner(&self, owner: SyntheticOwner) -> Result<(), HirSlotError> {
        match owner {
            SyntheticOwner::Item(id) => self.resolve_typed(id),
            SyntheticOwner::Scope(id) => self.resolve_typed(id),
            SyntheticOwner::Local(id) => self.resolve_typed(id),
            SyntheticOwner::Expr(id) => self.resolve_typed(id),
            SyntheticOwner::Stmt(id) => self.resolve_typed(id),
            SyntheticOwner::Type(id) => self.resolve_typed(id),
            SyntheticOwner::Pattern(id) => self.resolve_typed(id),
            SyntheticOwner::Capture(id) => self.resolve_typed(id),
        }
    }

    fn resolve_typed<I: HirTypedId>(&self, id: I) -> Result<(), HirSlotError> {
        self.resolve_raw(id.raw(), I::KIND).map(|_| ())
    }

    fn resolve_raw(
        &self,
        raw: RawHirId,
        expected: HirIdKind,
    ) -> Result<HirSlotLifetime, HirSlotError> {
        let index = raw.slot().get() as usize - 1;
        let staged = (index >= self.base_len)
            .then(|| self.metadata.get(index))
            .flatten()
            .map(|metadata| HirSlotLifetime {
                kind: metadata.kind,
                born: metadata.born,
                retired_at: metadata.retired_at,
                origin: metadata.origin.clone(),
            });
        let lifetime = staged.or_else(|| self.lifetimes.lifetime(raw.slot()));
        resolve_lifetime(self.snapshot, raw, expected, lifetime)
    }

    fn refresh_or_validate_view(
        &mut self,
        raw: RawHirId,
        source_site: HirSourceSite,
        poisoned: bool,
        touched: bool,
    ) -> Result<(), HirSlotError> {
        let payload_poison_is_bound = self.payload_poison.contains_key(&raw);
        let Some(metadata) = self.metadata.get_mut(raw.slot().get() as usize - 1) else {
            return Err(HirSlotError::OwnerNotReserved { id: raw.view() });
        };
        if touched {
            // Arena reservations do not know recovery state until the final
            // semantic payload is available. Once that payload has bound the
            // slot state, a repeated reservation validates only the source
            // view; `bind_payload_poison` remains the sole poison authority
            // and rejects a conflicting repeated payload.
            if metadata.source_site != source_site
                || (!payload_poison_is_bound && metadata.poisoned != poisoned)
            {
                return Err(HirSlotError::ConflictingSlotView { id: raw.view() });
            }
            return Ok(());
        }

        metadata.source_site = source_site;
        metadata.poisoned = poisoned;
        Ok(())
    }

    fn allocate(
        &mut self,
        kind: HirIdKind,
        origin: HirOrigin,
        source_site: HirSourceSite,
        poisoned: bool,
    ) -> Result<RawHirId, HirSlotError> {
        let observed = self.metadata.len().saturating_add(1);
        let maximum = {
            #[cfg(test)]
            {
                self.total_slot_maximum
            }
            #[cfg(not(test))]
            {
                HirLimit::TotalSlotsPerModule.maximum()
            }
        };
        if observed > maximum {
            return Err(HirLimitError::with_maximum(
                HirLimit::TotalSlotsPerModule,
                observed,
                maximum,
            )
            .into());
        }
        let slot = {
            #[cfg(test)]
            {
                self.next_slot_identity.take().map_or_else(
                    || u32::try_from(observed).ok(),
                    |identity| u32::try_from(identity).ok(),
                )
            }
            #[cfg(not(test))]
            {
                u32::try_from(observed).ok()
            }
        }
        .and_then(NonZeroU32::new)
        .ok_or(HirSlotError::SlotIdentityExhausted {
            module: self.snapshot.module(),
            kind,
        })?;
        let raw = RawHirId::new(self.snapshot.module(), slot, kind);
        self.metadata.push(HirSlotMetadata {
            kind,
            born: self.snapshot.revision(),
            retired_at: None,
            origin,
            source_site,
            poisoned,
        });
        Ok(raw)
    }

    const fn ensure_open(&self) -> Result<(), HirSlotError> {
        if self.poisoned {
            Err(HirSlotError::TransactionPoisoned)
        } else {
            Ok(())
        }
    }

    fn reject<T>(&mut self, error: HirSlotError) -> Result<T, HirSlotError> {
        self.poisoned = true;
        Err(error)
    }

    #[cfg(test)]
    fn staged_slot_count(&self) -> usize {
        self.metadata.len().saturating_sub(self.base_len)
    }

    #[cfg(test)]
    fn synthetic_count(&self, owner: SyntheticOwner) -> usize {
        self.synthetic_counts.get(&owner).copied().unwrap_or(0)
    }

    #[cfg(test)]
    fn corrupt_kind(&mut self, owner: SyntheticOwner, kind: HirIdKind) {
        let raw = match owner {
            SyntheticOwner::Item(id) => id.raw(),
            SyntheticOwner::Scope(id) => id.raw(),
            SyntheticOwner::Local(id) => id.raw(),
            SyntheticOwner::Expr(id) => id.raw(),
            SyntheticOwner::Stmt(id) => id.raw(),
            SyntheticOwner::Type(id) => id.raw(),
            SyntheticOwner::Pattern(id) => id.raw(),
            SyntheticOwner::Capture(id) => id.raw(),
        };
        if let Some(metadata) = self.metadata.get_mut(raw.slot().get() as usize - 1) {
            metadata.kind = kind;
        }
    }
}

#[cfg(test)]
#[path = "slot/tests.rs"]
mod tests;
