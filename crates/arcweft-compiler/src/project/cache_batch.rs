use super::{CompiledProjectModule, ProjectCompileUnitFingerprint};
use std::collections::BTreeMap;
use thiserror::Error;

pub(crate) struct PendingProjectCompileStore {
    fingerprint: ProjectCompileUnitFingerprint,
    modules: Vec<CompiledProjectModule>,
}

pub(crate) enum PendingProjectCompileStores {
    Collecting(Vec<PendingProjectCompileStore>),
    Flushed,
    Discarded,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum PendingStoreTransitionError {
    #[error("pending project compile stores are already finalized")]
    AlreadyFinalized,
}

/// In-process cache boundary for independently lowered compile units.
///
/// A persistent cache adapter should store a stable serialized unit format, not
/// `HirModule` directly. This trait deliberately covers only the current
/// in-process vertical slice.
pub trait ProjectCompileCache {
    fn load(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
    ) -> Option<Vec<CompiledProjectModule>>;

    fn store(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
        modules: &[CompiledProjectModule],
    );
}

/// No-op cache used by the simple compiler entry point.
#[derive(Default)]
pub struct NoProjectCompileCache;

/// Deterministic in-memory unit cache for watch mode and tests.
#[derive(Default)]
pub struct InMemoryProjectCompileCache {
    units: BTreeMap<ProjectCompileUnitFingerprint, Vec<CompiledProjectModule>>,
}

impl PendingProjectCompileStores {
    pub(crate) const fn new() -> Self {
        Self::Collecting(Vec::new())
    }

    pub(crate) fn push(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
        modules: Vec<CompiledProjectModule>,
    ) -> Result<(), PendingStoreTransitionError> {
        let Self::Collecting(stores) = self else {
            return Err(PendingStoreTransitionError::AlreadyFinalized);
        };
        stores.push(PendingProjectCompileStore {
            fingerprint,
            modules,
        });
        Ok(())
    }

    pub(crate) fn flush<C: ProjectCompileCache>(
        &mut self,
        cache: &mut C,
    ) -> Result<(), PendingStoreTransitionError> {
        let Self::Collecting(stores) = self else {
            return Err(PendingStoreTransitionError::AlreadyFinalized);
        };
        let stores = std::mem::take(stores);
        *self = Self::Flushed;
        for store in stores {
            cache.store(store.fingerprint, &store.modules);
        }
        Ok(())
    }

    pub(crate) fn discard(&mut self) {
        if matches!(self, Self::Collecting(_)) {
            *self = Self::Discarded;
        }
    }
}

impl Drop for PendingProjectCompileStores {
    fn drop(&mut self) {
        if let Self::Collecting(stores) = self {
            stores.clear();
        }
    }
}

impl ProjectCompileCache for NoProjectCompileCache {
    fn load(
        &mut self,
        _fingerprint: ProjectCompileUnitFingerprint,
    ) -> Option<Vec<CompiledProjectModule>> {
        None
    }

    fn store(
        &mut self,
        _fingerprint: ProjectCompileUnitFingerprint,
        _modules: &[CompiledProjectModule],
    ) {
    }
}

impl ProjectCompileCache for InMemoryProjectCompileCache {
    fn load(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
    ) -> Option<Vec<CompiledProjectModule>> {
        self.units.get(&fingerprint).cloned()
    }

    fn store(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
        modules: &[CompiledProjectModule],
    ) {
        self.units.insert(fingerprint, modules.to_vec());
    }
}
