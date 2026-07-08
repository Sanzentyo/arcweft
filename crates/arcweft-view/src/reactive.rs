//! Reactive dependency metadata for retained View property sources.

use crate::{DirtyFlags, PropertyBindingTable, RawEntity, ValueSourceId, ViewError};
use std::collections::BTreeMap;

/// Monotonic revision for a dynamic value source.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Revision(pub u64);

/// One entity invalidated by a source revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityInvalidation {
    entity: RawEntity,
    dirty: DirtyFlags,
}

/// Result of invalidating a dynamic value source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactiveInvalidation {
    source: ValueSourceId,
    revision: Revision,
    entities: Vec<EntityInvalidation>,
}

/// Deterministic dependency graph from dynamic value sources to View entities.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReactiveGraph {
    dependencies: BTreeMap<ValueSourceId, BTreeMap<RawEntity, DirtyFlags>>,
    revisions: BTreeMap<ValueSourceId, Revision>,
}

impl Revision {
    pub fn next(self) -> Result<Self, ViewError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ViewError::CapacityExceeded)
    }
}

impl EntityInvalidation {
    pub const fn new(entity: RawEntity, dirty: DirtyFlags) -> Self {
        Self { entity, dirty }
    }

    pub const fn entity(self) -> RawEntity {
        self.entity
    }

    pub const fn dirty(self) -> DirtyFlags {
        self.dirty
    }
}

impl ReactiveInvalidation {
    pub const fn source(&self) -> ValueSourceId {
        self.source
    }

    pub const fn revision(&self) -> Revision {
        self.revision
    }

    pub fn entities(&self) -> &[EntityInvalidation] {
        &self.entities
    }
}

impl ReactiveGraph {
    pub fn watch_entity(&mut self, source: ValueSourceId, entity: RawEntity, dirty: DirtyFlags) {
        let entry = self
            .dependencies
            .entry(source)
            .or_default()
            .entry(entity)
            .or_insert(DirtyFlags::NONE);
        entry.insert(dirty);
    }

    pub fn watch_property_table(&mut self, entity: RawEntity, table: &PropertyBindingTable) {
        for binding in table.as_slice() {
            self.watch_entity(binding.source(), entity, binding.dirty_flags());
        }
    }

    pub fn invalidate(&mut self, source: ValueSourceId) -> Result<ReactiveInvalidation, ViewError> {
        let revision = self
            .revisions
            .get(&source)
            .copied()
            .unwrap_or_default()
            .next()?;
        self.revisions.insert(source, revision);
        let entities = self
            .dependencies
            .get(&source)
            .map(|dependencies| {
                dependencies
                    .iter()
                    .map(|(entity, dirty)| EntityInvalidation::new(*entity, *dirty))
                    .collect()
            })
            .unwrap_or_default();
        Ok(ReactiveInvalidation {
            source,
            revision,
            entities,
        })
    }

    pub fn revision(&self, source: ValueSourceId) -> Revision {
        self.revisions.get(&source).copied().unwrap_or_default()
    }
}
