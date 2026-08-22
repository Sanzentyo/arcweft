//! Generation-local runtime image ownership for hot-swap execution.
//!
//! `ProgramGeneration` remains the deterministic fingerprint and compatibility
//! record. This module owns the runtime images that can actually execute a
//! generation. Keeping the table separate avoids turning compatibility metadata
//! into an executor/cache container and lets adapters choose VM, AOT, or future
//! generated images without changing swap fingerprints.

use arcweft_core::task::GenerationId;

use crate::swap::ProgramGeneration;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

/// Runtime image associated with one executable generation.
#[derive(Clone, Debug)]
pub struct GenerationRuntimeImage<R> {
    generation: Arc<ProgramGeneration>,
    runtime: R,
}

/// Runtime images keyed by generation id.
#[derive(Clone, Debug)]
pub struct GenerationRuntimeTable<R> {
    images: BTreeMap<GenerationId, GenerationRuntimeImage<R>>,
}

/// Error raised while looking up or inserting generation runtime images.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GenerationRuntimeError {
    #[error("generation runtime image {generation:?} is already registered")]
    DuplicateGeneration { generation: GenerationId },
    #[error("generation runtime image {generation:?} is not registered")]
    MissingGeneration { generation: GenerationId },
}

impl<R> GenerationRuntimeImage<R> {
    /// Creates a runtime image for `generation`.
    pub fn new(generation: Arc<ProgramGeneration>, runtime: R) -> Self {
        Self {
            generation,
            runtime,
        }
    }

    /// Returns the generation metadata owned by this image.
    pub const fn generation(&self) -> &Arc<ProgramGeneration> {
        &self.generation
    }

    /// Returns the stable generation id.
    pub fn generation_id(&self) -> GenerationId {
        self.generation.id
    }

    /// Returns the generation-local runtime payload.
    pub const fn runtime(&self) -> &R {
        &self.runtime
    }

    /// Returns the mutable generation-local runtime payload.
    pub fn runtime_mut(&mut self) -> &mut R {
        &mut self.runtime
    }

    /// Consumes the image and returns the runtime payload.
    pub fn into_runtime(self) -> R {
        self.runtime
    }
}

impl<R: Clone> GenerationRuntimeImage<R> {
    /// Clones the runtime payload. `BundleSession` uses this for new entry
    /// binding so a fresh executor can be spawned from the active generation
    /// template without mutating an existing fiber image.
    pub fn cloned_runtime(&self) -> R {
        self.runtime.clone()
    }
}

impl<R> GenerationRuntimeTable<R> {
    /// Creates a table containing the initial active generation runtime.
    pub fn new(active: GenerationRuntimeImage<R>) -> Self {
        let mut images = BTreeMap::new();
        images.insert(active.generation_id(), active);
        Self { images }
    }

    /// Inserts a new generation runtime image.
    pub fn insert(
        &mut self,
        image: GenerationRuntimeImage<R>,
    ) -> Result<(), GenerationRuntimeError> {
        let generation = image.generation_id();
        if self.images.contains_key(&generation) {
            return Err(GenerationRuntimeError::DuplicateGeneration { generation });
        }
        self.images.insert(generation, image);
        Ok(())
    }

    /// Replaces an existing generation runtime image or inserts a new one.
    pub fn replace(
        &mut self,
        image: GenerationRuntimeImage<R>,
    ) -> Option<GenerationRuntimeImage<R>> {
        self.images.insert(image.generation_id(), image)
    }

    /// Returns a registered generation runtime image.
    pub fn get(
        &self,
        generation: GenerationId,
    ) -> Result<&GenerationRuntimeImage<R>, GenerationRuntimeError> {
        self.images
            .get(&generation)
            .ok_or(GenerationRuntimeError::MissingGeneration { generation })
    }

    /// Returns a mutable registered generation runtime image.
    pub fn get_mut(
        &mut self,
        generation: GenerationId,
    ) -> Result<&mut GenerationRuntimeImage<R>, GenerationRuntimeError> {
        self.images
            .get_mut(&generation)
            .ok_or(GenerationRuntimeError::MissingGeneration { generation })
    }

    /// Removes a registered generation runtime image.
    pub fn remove(&mut self, generation: GenerationId) -> Option<GenerationRuntimeImage<R>> {
        self.images.remove(&generation)
    }

    /// Removes every runtime image whose generation id is not live.
    pub fn retain_generations(&mut self, live: &BTreeSet<GenerationId>) {
        self.images
            .retain(|generation, _| live.contains(generation));
    }

    /// Returns the registered generation ids in deterministic order.
    pub fn generation_ids(&self) -> impl Iterator<Item = GenerationId> + '_ {
        self.images.keys().copied()
    }

    /// Returns whether the table has a runtime for `generation`.
    pub fn contains_generation(&self, generation: GenerationId) -> bool {
        self.images.contains_key(&generation)
    }

    /// Returns the number of registered generation runtime images.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Returns whether no runtime images are registered.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::container::BundleDigest;

    fn generation(id: u64) -> Arc<ProgramGeneration> {
        Arc::new(ProgramGeneration::empty(
            GenerationId::new(id),
            BundleDigest::of(&id.to_le_bytes()),
            BundleDigest::of(&id.to_le_bytes()),
        ))
    }

    #[test]
    fn table_keeps_runtime_payloads_by_generation_id() {
        let active = GenerationRuntimeImage::new(generation(0), "active");
        let mut table = GenerationRuntimeTable::new(active);
        table
            .insert(GenerationRuntimeImage::new(generation(1), "next"))
            .expect("next generation inserts");

        assert_eq!(
            table.get(GenerationId::new(0)).expect("active").runtime(),
            &"active"
        );
        assert_eq!(
            table.get(GenerationId::new(1)).expect("next").runtime(),
            &"next"
        );
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn table_prunes_non_live_runtime_images() {
        let mut table = GenerationRuntimeTable::new(GenerationRuntimeImage::new(generation(0), 0));
        table
            .insert(GenerationRuntimeImage::new(generation(1), 1))
            .expect("next generation inserts");
        let live = BTreeSet::from([GenerationId::new(1)]);

        table.retain_generations(&live);

        assert!(!table.contains_generation(GenerationId::new(0)));
        assert!(table.contains_generation(GenerationId::new(1)));
    }

    #[test]
    fn table_reports_missing_generation_deterministically() {
        let table = GenerationRuntimeTable::new(GenerationRuntimeImage::new(generation(0), 0));

        let error = table
            .get(GenerationId::new(7))
            .expect_err("missing generation is typed");

        assert_eq!(
            error,
            GenerationRuntimeError::MissingGeneration {
                generation: GenerationId::new(7)
            }
        );
    }
}
