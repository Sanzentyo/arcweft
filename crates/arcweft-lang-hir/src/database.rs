//! Transactional publication of immutable HIR module snapshots.

use core::num::{NonZeroU32, NonZeroU64};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::SourceDocumentId;
use thiserror::Error;

use crate::identity::{
    HirDatabaseCreateError, HirDatabaseId, HirLimit, HirModuleId, HirRevision, HirSnapshotId,
    ItemId,
};
use crate::lowering::{HirInvariantFailure, HirLimitError, HirLowerFailure, HirModuleKey};
use crate::module::HirModule;
use crate::slot::PreparedSlotCommit;
#[cfg(test)]
use crate::slot::SlotLifetimeTestState;
use crate::symbol::CallablePackageId;

struct ModuleState {
    module: HirModuleId,
    current: Arc<HirModule>,
    snapshots: BTreeMap<HirRevision, Arc<HirModule>>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct HirModuleRegistryKey {
    package: CallablePackageId,
    path: CanonicalModulePath,
    document: SourceDocumentId,
}

impl From<&HirModuleKey> for HirModuleRegistryKey {
    fn from(key: &HirModuleKey) -> Self {
        Self {
            package: key.package().clone(),
            path: key.path().clone(),
            document: key.source().id().clone(),
        }
    }
}

/// Exact immutable database state used only by transactional unit tests.
///
/// Retaining the accepted module leases by `Arc` identity covers their slot
/// lifetimes, typed arenas, diagnostics, source index, invalidation epoch, and
/// provenance without adding a second field-by-field database model.
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct HirDatabaseTestState {
    database: HirDatabaseId,
    modules: Box<[HirDatabaseModuleTestState]>,
    next_module_slot: NonZeroU32,
    module_limit: usize,
}

#[cfg(test)]
#[derive(Clone)]
struct HirDatabaseModuleTestState {
    key: HirModuleKey,
    module: HirModuleId,
    current: Arc<HirModule>,
    snapshots: Box<[(HirRevision, Arc<HirModule>)]>,
    lifetimes: SlotLifetimeTestState,
}

#[cfg(test)]
impl PartialEq for HirDatabaseTestState {
    fn eq(&self, other: &Self) -> bool {
        self.database == other.database
            && self.next_module_slot == other.next_module_slot
            && self.module_limit == other.module_limit
            && self.modules.len() == other.modules.len()
            && self
                .modules
                .iter()
                .zip(&other.modules)
                .all(|(left, right)| left.same_state(right))
    }
}

#[cfg(test)]
impl Eq for HirDatabaseTestState {}

#[cfg(test)]
impl core::fmt::Debug for HirDatabaseTestState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let modules = self
            .modules
            .iter()
            .map(|state| {
                (
                    &state.key,
                    state.module,
                    state.current.snapshot_id(),
                    Arc::as_ptr(&state.current),
                    &state.lifetimes,
                    state
                        .snapshots
                        .iter()
                        .map(|(revision, module)| (*revision, Arc::as_ptr(module)))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        formatter
            .debug_struct("HirDatabaseTestState")
            .field("database", &self.database)
            .field("modules", &modules)
            .field("next_module_slot", &self.next_module_slot)
            .field("module_limit", &self.module_limit)
            .finish()
    }
}

#[cfg(test)]
impl HirDatabaseModuleTestState {
    fn same_state(&self, other: &Self) -> bool {
        self.key == other.key
            && self.module == other.module
            && Arc::ptr_eq(&self.current, &other.current)
            && self.lifetimes == other.lifetimes
            && self.snapshots.len() == other.snapshots.len()
            && self.snapshots.iter().zip(&other.snapshots).all(
                |((left_revision, left), (right_revision, right))| {
                    left_revision == right_revision && Arc::ptr_eq(left, right)
                },
            )
    }
}

/// Immutable identity and revision proposed by one private lowering transaction.
///
/// Staging and dropping this value consume neither a module slot nor a
/// revision. Publication consumes the proposal only after the lowering owner
/// has constructed a validated immutable [`HirModule`].
pub(crate) struct StagedModuleCommit {
    key: HirModuleKey,
    snapshot: HirSnapshotId,
    invalidation_epoch: NonZeroU64,
    previous: Option<Arc<HirModule>>,
}

pub(crate) struct PreparedHirModuleCommit {
    plan: StagedModuleCommit,
    slots: PreparedSlotCommit,
    module: Arc<HirModule>,
}

impl PreparedHirModuleCommit {
    pub(crate) fn new(
        plan: StagedModuleCommit,
        slots: PreparedSlotCommit,
        module: Arc<HirModule>,
    ) -> Self {
        Self {
            plan,
            slots,
            module,
        }
    }
}

impl StagedModuleCommit {
    pub(crate) const fn module_id(&self) -> HirModuleId {
        self.snapshot.module()
    }

    pub(crate) const fn revision(&self) -> HirRevision {
        self.snapshot.revision()
    }

    pub(crate) const fn snapshot_id(&self) -> HirSnapshotId {
        self.snapshot
    }

    pub(crate) const fn key(&self) -> &HirModuleKey {
        &self.key
    }

    pub(crate) const fn invalidation_epoch(&self) -> NonZeroU64 {
        self.invalidation_epoch
    }

    pub(crate) const fn previous(&self) -> Option<&Arc<HirModule>> {
        self.previous.as_ref()
    }
}

/// Exact cache invalidation facts published only by a successful lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirInvalidationSet {
    module: HirModuleId,
    previous: Option<HirSnapshotId>,
    current: HirSnapshotId,
    changed_items: Box<[ItemId]>,
    retired_items: Box<[ItemId]>,
    symbol_revision_changed: bool,
    executable_status_changed: bool,
}

impl HirInvalidationSet {
    pub const fn module(&self) -> HirModuleId {
        self.module
    }

    pub const fn previous(&self) -> Option<HirSnapshotId> {
        self.previous
    }

    pub const fn current(&self) -> HirSnapshotId {
        self.current
    }

    pub fn changed_items(&self) -> &[ItemId] {
        &self.changed_items
    }

    pub fn retired_items(&self) -> &[ItemId] {
        &self.retired_items
    }

    pub const fn symbol_revision_changed(&self) -> bool {
        self.symbol_revision_changed
    }

    pub const fn executable_status_changed(&self) -> bool {
        self.executable_status_changed
    }

    pub fn is_empty(&self) -> bool {
        self.changed_items.is_empty()
            && self.retired_items.is_empty()
            && !self.symbol_revision_changed
            && !self.executable_status_changed
    }

    fn derive(
        previous: Option<&Arc<HirModule>>,
        current: &HirModule,
    ) -> Result<Self, HirLowerFailure> {
        if previous.is_some_and(|module| module.module_id() != current.module_id()) {
            return Err(HirInvariantFailure::InvalidModuleCommit.into());
        }

        let current_items = current
            .arenas()
            .items()
            .try_iter_prepared(current.slots())
            .map_err(|_| HirInvariantFailure::InvalidModuleCommit)?
            .collect::<BTreeMap<_, _>>();
        let previous_items = previous
            .map(|module| {
                module
                    .arenas()
                    .items()
                    .try_iter(module.slots())
                    .map(std::iter::Iterator::collect::<BTreeMap<_, _>>)
                    .map_err(|_| HirInvariantFailure::InvalidModuleCommit)
            })
            .transpose()?
            .unwrap_or_default();

        let changed_items = current_items
            .iter()
            .filter_map(|(id, item)| {
                let item_is_equal = previous_items.get(id) == Some(item);
                let members_are_equal = previous
                    .and_then(|module| module.declaration_members().arena(*id))
                    == current.declaration_members().arena(*id);
                (!(item_is_equal && members_are_equal)).then_some(*id)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let retired_items = previous_items
            .keys()
            .filter(|id| !current_items.contains_key(id))
            .copied()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let executable_status_changed =
            previous.is_some_and(|module| module.status() != current.status());
        // No accepted contract defines a complete narrower symbol projection
        // for every final item family. The item and its per-item member arena
        // are one semantic publication unit: the item intentionally retains
        // only stable member IDs, so comparing the item record alone would
        // miss a retained member payload edit. Invalidating for every such
        // semantic item delta is conservative; exact-equal trivia-only
        // revisions stay hot.
        let symbol_revision_changed =
            executable_status_changed || !changed_items.is_empty() || !retired_items.is_empty();

        Ok(Self {
            module: current.module_id(),
            previous: previous.map(|module| module.snapshot_id()),
            current: current.snapshot_id(),
            changed_items,
            retired_items,
            symbol_revision_changed,
            executable_status_changed,
        })
    }
}

/// One accepted immutable module lease and its only invalidation publication.
#[derive(Clone)]
pub struct HirLowerOutput {
    module: Arc<HirModule>,
    invalidations: HirInvalidationSet,
}

impl HirLowerOutput {
    fn new(module: Arc<HirModule>, invalidations: HirInvalidationSet) -> Self {
        Self {
            module,
            invalidations,
        }
    }

    pub const fn module(&self) -> &Arc<HirModule> {
        &self.module
    }

    pub fn into_module(self) -> Arc<HirModule> {
        self.module
    }

    pub const fn invalidations(&self) -> &HirInvalidationSet {
        &self.invalidations
    }

    pub fn into_parts(self) -> (Arc<HirModule>, HirInvalidationSet) {
        (self.module, self.invalidations)
    }
}

/// Failure to find an immutable HIR snapshot in one database.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirSnapshotLookupError {
    #[error("HIR snapshot belongs to another database")]
    WrongDatabase {
        expected: HirDatabaseId,
        actual: HirDatabaseId,
    },
    #[error("HIR module is not present in this database")]
    UnknownModule { module: HirModuleId },
    #[error("HIR revision is not retained for this module")]
    UnknownRevision {
        module: HirModuleId,
        revision: HirRevision,
    },
}

/// Process-local identity authority and immutable module-snapshot registry.
pub struct HirDatabase {
    id: HirDatabaseId,
    modules: BTreeMap<HirModuleRegistryKey, ModuleState>,
    next_module_slot: NonZeroU32,
    module_limit: usize,
}

impl HirDatabase {
    /// Allocates a fresh process-local database identity without wrapping.
    pub fn try_new() -> Result<Self, HirDatabaseCreateError> {
        Self::with_module_limit(HirLimit::ModulesPerDatabase.maximum())
    }

    fn with_module_limit(module_limit: usize) -> Result<Self, HirDatabaseCreateError> {
        Ok(Self {
            id: HirDatabaseId::allocate()?,
            modules: BTreeMap::new(),
            next_module_slot: NonZeroU32::MIN,
            module_limit,
        })
    }

    /// Identity qualifying every module and typed HIR ID owned here.
    pub const fn database_id(&self) -> HirDatabaseId {
        self.id
    }

    /// Returns the exact accepted lease for a module key.
    pub fn current(&self, key: &HirModuleKey) -> Option<Arc<HirModule>> {
        self.modules
            .get(&HirModuleRegistryKey::from(key))
            .filter(|state| state.current.key() == key)
            .map(|state| Arc::clone(&state.current))
    }

    pub(crate) fn current_lineage(&self, key: &HirModuleKey) -> Option<Arc<HirModule>> {
        self.modules
            .get(&HirModuleRegistryKey::from(key))
            .map(|state| Arc::clone(&state.current))
    }

    #[cfg(test)]
    pub(crate) fn test_state(&self) -> HirDatabaseTestState {
        HirDatabaseTestState {
            database: self.id,
            modules: self
                .modules
                .values()
                .map(|state| HirDatabaseModuleTestState {
                    key: state.current.key().clone(),
                    module: state.module,
                    current: Arc::clone(&state.current),
                    snapshots: state
                        .snapshots
                        .iter()
                        .map(|(revision, module)| (*revision, Arc::clone(module)))
                        .collect(),
                    lifetimes: state.current.slots().lifetime_test_state(),
                })
                .collect(),
            next_module_slot: self.next_module_slot,
            module_limit: self.module_limit,
        }
    }

    /// Returns one retained immutable snapshot without rebasing its IDs.
    pub fn snapshot(&self, id: HirSnapshotId) -> Result<Arc<HirModule>, HirSnapshotLookupError> {
        let actual = id.module().database();
        if actual != self.id {
            return Err(HirSnapshotLookupError::WrongDatabase {
                expected: self.id,
                actual,
            });
        }

        let state = self
            .modules
            .values()
            .find(|state| state.module == id.module())
            .ok_or(HirSnapshotLookupError::UnknownModule {
                module: id.module(),
            })?;
        state.snapshots.get(&id.revision()).cloned().ok_or(
            HirSnapshotLookupError::UnknownRevision {
                module: id.module(),
                revision: id.revision(),
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn stage_module(
        &self,
        key: &HirModuleKey,
    ) -> Result<StagedModuleCommit, HirLowerFailure> {
        let registry = HirModuleRegistryKey::from(key);
        if let Some(state) = self.modules.get(&registry) {
            let revision = state
                .current
                .snapshot_id()
                .revision()
                .checked_next()
                .ok_or(HirLowerFailure::RevisionExhausted {
                    module: state.module,
                })?;
            let invalidation_epoch = state
                .current
                .invalidation_epoch()
                .get()
                .checked_add(1)
                .and_then(NonZeroU64::new)
                .ok_or(HirLowerFailure::CacheEpochExhausted {
                    module: state.module,
                })?;
            return Ok(StagedModuleCommit {
                key: key.clone(),
                snapshot: HirSnapshotId::new(state.module, revision),
                invalidation_epoch,
                previous: Some(Arc::clone(&state.current)),
            });
        }

        let observed = self.modules.len().saturating_add(1);
        if observed > self.module_limit {
            return Err(HirLimitError::with_maximum(
                HirLimit::ModulesPerDatabase,
                observed,
                self.module_limit,
            )
            .into());
        }

        if self
            .modules
            .values()
            .any(|state| state.module.slot() == self.next_module_slot)
        {
            return Err(HirLowerFailure::ModuleIdentityExhausted);
        }

        Ok(StagedModuleCommit {
            key: key.clone(),
            snapshot: HirSnapshotId::new(
                HirModuleId::new(self.id, self.next_module_slot),
                HirRevision::INITIAL,
            ),
            invalidation_epoch: NonZeroU64::MIN,
            previous: None,
        })
    }

    /// Proposes every module identity/revision for one unpublished project in
    /// deterministic key order. New modules receive distinct consecutive
    /// identities without mutating the database during reservation.
    pub(crate) fn stage_project_modules(
        &self,
        keys: impl IntoIterator<Item = HirModuleKey>,
    ) -> Result<Vec<StagedModuleCommit>, HirLowerFailure> {
        let mut keys = keys.into_iter().collect::<Vec<_>>();
        if keys.is_empty() {
            return Err(HirLowerFailure::EmptyProjectTransaction);
        }
        keys.sort();
        let mut unique = BTreeSet::new();
        for key in &keys {
            if !unique.insert(HirModuleRegistryKey::from(key)) {
                return Err(HirLowerFailure::DuplicateModuleRequest {
                    module: key.path().clone(),
                });
            }
        }
        let new_count = keys
            .iter()
            .filter(|key| !self.modules.contains_key(&HirModuleRegistryKey::from(*key)))
            .count();
        let observed = self.modules.len().saturating_add(new_count);
        if observed > self.module_limit {
            return Err(HirLimitError::with_maximum(
                HirLimit::ModulesPerDatabase,
                observed,
                self.module_limit,
            )
            .into());
        }

        let mut next_new_slot = self.next_module_slot;
        let mut plans = Vec::with_capacity(keys.len());
        for key in keys {
            let registry = HirModuleRegistryKey::from(&key);
            if let Some(state) = self.modules.get(&registry) {
                let revision = state
                    .current
                    .snapshot_id()
                    .revision()
                    .checked_next()
                    .ok_or(HirLowerFailure::RevisionExhausted {
                        module: state.module,
                    })?;
                let invalidation_epoch = state
                    .current
                    .invalidation_epoch()
                    .get()
                    .checked_add(1)
                    .and_then(NonZeroU64::new)
                    .ok_or(HirLowerFailure::CacheEpochExhausted {
                        module: state.module,
                    })?;
                plans.push(StagedModuleCommit {
                    key,
                    snapshot: HirSnapshotId::new(state.module, revision),
                    invalidation_epoch,
                    previous: Some(Arc::clone(&state.current)),
                });
                continue;
            }
            if self
                .modules
                .values()
                .any(|state| state.module.slot() == next_new_slot)
            {
                return Err(HirLowerFailure::ModuleIdentityExhausted);
            }
            plans.push(StagedModuleCommit {
                key,
                snapshot: HirSnapshotId::new(
                    HirModuleId::new(self.id, next_new_slot),
                    HirRevision::INITIAL,
                ),
                invalidation_epoch: NonZeroU64::MIN,
                previous: None,
            });
            next_new_slot = next_new_slot
                .get()
                .checked_add(1)
                .and_then(NonZeroU32::new)
                .ok_or(HirLowerFailure::ModuleIdentityExhausted)?;
        }
        Ok(plans)
    }

    /// Publishes one fully frozen module and its slot-lifetime ledger as the
    /// database's only observable mutation.
    ///
    /// Every fallible database, module, slot, and invalidation check runs
    /// before the shared lifetime ledger changes. After that ledger accepts
    /// the proposal, inserting the already validated module is infallible.
    #[cfg(test)]
    pub(crate) fn publish_module(
        &mut self,
        plan: &StagedModuleCommit,
        prepared_slots: PreparedSlotCommit,
        module: Arc<HirModule>,
    ) -> Result<HirLowerOutput, HirLowerFailure> {
        self.validate_module_commit(plan, &module)?;
        if !Arc::ptr_eq(module.slots(), prepared_slots.snapshot()) {
            return Err(HirInvariantFailure::InvalidModuleCommit.into());
        }
        let previous_slots = plan.previous().map(|previous| previous.slots().as_ref());
        if !prepared_slots.validates_ancestry(previous_slots) {
            return Err(HirInvariantFailure::InvalidModuleCommit.into());
        }
        let previous = self
            .modules
            .get(&HirModuleRegistryKey::from(plan.key()))
            .map(|state| &state.current);
        let invalidations = HirInvalidationSet::derive(previous, &module)?;

        let published_slots = prepared_slots
            .publish()
            .map_err(|_| HirInvariantFailure::InvalidModuleCommit)?;
        debug_assert!(Arc::ptr_eq(module.slots(), &published_slots));
        self.insert_validated_module(plan, &module);
        Ok(HirLowerOutput::new(module, invalidations))
    }

    /// Atomically publishes every already-frozen module in one project
    /// transaction. All database, ancestry, invalidation, and lifetime-ledger
    /// checks complete before the first observable mutation.
    pub(crate) fn publish_project_modules(
        &mut self,
        prepared: Vec<PreparedHirModuleCommit>,
    ) -> Result<Vec<HirLowerOutput>, HirLowerFailure> {
        if prepared.is_empty() {
            return Err(HirLowerFailure::EmptyProjectTransaction);
        }
        let mut keys = BTreeSet::new();
        let mut new_module_slot = self.next_module_slot;
        let mut invalidations = Vec::with_capacity(prepared.len());
        for commit in &prepared {
            let registry = HirModuleRegistryKey::from(commit.plan.key());
            if !keys.insert(registry.clone()) {
                return Err(HirLowerFailure::DuplicateModuleRequest {
                    module: commit.plan.key().path().clone(),
                });
            }
            if self.modules.contains_key(&registry) {
                self.validate_module_commit(&commit.plan, &commit.module)?;
            } else {
                if commit.plan.previous().is_some()
                    || commit.plan.module_id().database() != self.id
                    || commit.plan.module_id().slot() != new_module_slot
                    || commit.plan.revision() != HirRevision::INITIAL
                    || commit.module.key() != commit.plan.key()
                    || commit.module.snapshot_id() != commit.plan.snapshot_id()
                    || commit.module.invalidation_epoch() != commit.plan.invalidation_epoch()
                {
                    return Err(HirInvariantFailure::InvalidModuleCommit.into());
                }
                new_module_slot = new_module_slot
                    .get()
                    .checked_add(1)
                    .and_then(NonZeroU32::new)
                    .ok_or(HirLowerFailure::ModuleIdentityExhausted)?;
            }
            if !Arc::ptr_eq(commit.module.slots(), commit.slots.snapshot()) {
                return Err(HirInvariantFailure::InvalidModuleCommit.into());
            }
            let previous_slots = commit
                .plan
                .previous()
                .map(|previous| previous.slots().as_ref());
            if !commit.slots.validates_ancestry(previous_slots) {
                return Err(HirInvariantFailure::InvalidModuleCommit.into());
            }
            commit
                .slots
                .validate_publish()
                .map_err(|_| HirInvariantFailure::InvalidModuleCommit)?;
            let previous = self.modules.get(&registry).map(|state| &state.current);
            invalidations.push(HirInvalidationSet::derive(previous, &commit.module)?);
        }

        let mut outputs = Vec::with_capacity(prepared.len());
        for (commit, invalidations) in prepared.into_iter().zip(invalidations) {
            let published_slots = commit
                .slots
                .publish()
                .expect("project slot proposals were prevalidated before publication");
            debug_assert!(Arc::ptr_eq(commit.module.slots(), &published_slots));
            let module = Arc::clone(&commit.module);
            self.insert_validated_module(&commit.plan, &module);
            outputs.push(HirLowerOutput::new(module, invalidations));
        }
        Ok(outputs)
    }

    #[cfg(test)]
    pub(crate) fn publish_prepared_module(
        &mut self,
        commit: PreparedHirModuleCommit,
    ) -> Result<HirLowerOutput, HirLowerFailure> {
        self.publish_module(&commit.plan, commit.slots, commit.module)
    }

    fn validate_module_commit(
        &self,
        plan: &StagedModuleCommit,
        module: &HirModule,
    ) -> Result<(), HirLowerFailure> {
        if module.key() != plan.key()
            || module.snapshot_id() != plan.snapshot_id()
            || module.invalidation_epoch() != plan.invalidation_epoch()
        {
            return Err(HirInvariantFailure::InvalidModuleCommit.into());
        }

        let registry = HirModuleRegistryKey::from(&plan.key);
        if let Some(state) = self.modules.get(&registry) {
            let Some(previous) = plan.previous.as_ref() else {
                return Err(HirInvariantFailure::InvalidModuleCommit.into());
            };
            let Some(expected_revision) = state.current.snapshot_id().revision().checked_next()
            else {
                return Err(HirLowerFailure::RevisionExhausted {
                    module: state.module,
                });
            };
            if state.module != plan.module_id()
                || !Arc::ptr_eq(previous, &state.current)
                || expected_revision != plan.revision()
                || state.current.invalidation_epoch().get().checked_add(1)
                    != Some(plan.invalidation_epoch().get())
                || state.snapshots.contains_key(&plan.revision())
            {
                return Err(HirInvariantFailure::InvalidModuleCommit.into());
            }
        } else if plan.previous.is_some()
            || plan.module_id().database() != self.id
            || plan.module_id().slot() != self.next_module_slot
            || plan.revision() != HirRevision::INITIAL
        {
            return Err(HirInvariantFailure::InvalidModuleCommit.into());
        }

        Ok(())
    }

    fn insert_validated_module(&mut self, plan: &StagedModuleCommit, module: &Arc<HirModule>) {
        let registry = HirModuleRegistryKey::from(&plan.key);
        if let Some(state) = self.modules.get_mut(&registry) {
            state.snapshots.insert(plan.revision(), Arc::clone(module));
            state.current = Arc::clone(module);
        } else {
            let module_id = plan.module_id();
            let revision = plan.revision();
            let mut snapshots = BTreeMap::new();
            snapshots.insert(revision, Arc::clone(module));
            self.modules.insert(
                registry,
                ModuleState {
                    module: module_id,
                    current: Arc::clone(module),
                    snapshots,
                },
            );
            if let Some(next) = self
                .next_module_slot
                .get()
                .checked_add(1)
                .and_then(NonZeroU32::new)
            {
                self.next_module_slot = next;
            }
        }
    }

    #[cfg(test)]
    fn commit_module(
        &mut self,
        plan: &StagedModuleCommit,
        module: Arc<HirModule>,
    ) -> Result<Arc<HirModule>, HirLowerFailure> {
        self.validate_module_commit(plan, &module)?;
        self.insert_validated_module(plan, &module);
        Ok(module)
    }

    #[cfg(test)]
    fn with_test_module_limit(module_limit: usize) -> Self {
        Self::with_module_limit(module_limit).unwrap()
    }

    #[cfg(test)]
    fn seed_next_module_slot(&mut self, slot: NonZeroU32) {
        self.next_module_slot = slot;
    }
}

#[cfg(test)]
#[path = "database/tests.rs"]
mod tests;
