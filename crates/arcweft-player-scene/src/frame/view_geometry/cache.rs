//! Exact staged geometry caches and committed frame ownership.

use super::conversion::consumer_hit_rect;
use super::error::{ViewGeometryRuntimeError, ViewGeometryTargetKey};
use arcweft_view::geometry::{
    ViewFinalGeometry, ViewFinalGeometryKey, ViewGeometryConsumer, ViewMeasuredBox,
    ViewMeasuredGeometryKey, ViewOuterSize, ViewPlacedGeometryKey,
};
use arcweft_view::style::ViewStyleNodeKey;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryGeneration(u64);

impl ViewGeometryGeneration {
    pub const ZERO: Self = Self(0);

    pub const fn value(self) -> u64 {
        self.0
    }

    pub(crate) fn checked_next(self) -> Result<Self, ViewGeometryRuntimeError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ViewGeometryRuntimeError::GenerationOverflow { current: self })
    }

    #[cfg(test)]
    pub(crate) const fn from_value(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ViewMeasureCacheEntry {
    pub key: ViewMeasuredGeometryKey,
    pub measured: ViewMeasuredBox,
    pub outer: ViewOuterSize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ViewPlaceCacheEntry {
    pub key: ViewPlacedGeometryKey,
    pub placement: arcweft_view::geometry::ViewBoxPlacement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ViewFinalCacheEntry {
    pub key: ViewFinalGeometryKey,
    pub geometry: ViewFinalGeometry,
}

#[derive(Debug, Default)]
pub(crate) struct PlayerViewGeometryState {
    generation: ViewGeometryGeneration,
    measure: BTreeMap<ViewStyleNodeKey, ViewMeasureCacheEntry>,
    place: BTreeMap<ViewStyleNodeKey, ViewPlaceCacheEntry>,
    final_geometry: BTreeMap<ViewStyleNodeKey, ViewFinalCacheEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewCommittedGeometryFrame {
    generation: ViewGeometryGeneration,
    viewport: arcweft_view::geometry::ViewGeometryRect,
    final_nodes: BTreeMap<ViewStyleNodeKey, ViewFinalGeometry>,
    transparent_nodes: BTreeSet<ViewStyleNodeKey>,
    suppressed_nodes: BTreeSet<ViewStyleNodeKey>,
    targets: BTreeMap<ViewGeometryTargetKey, ViewStyleNodeKey>,
}

impl ViewCommittedGeometryFrame {
    pub const fn generation(&self) -> ViewGeometryGeneration {
        self.generation
    }

    pub const fn viewport(&self) -> arcweft_view::geometry::ViewGeometryRect {
        self.viewport
    }

    pub fn final_geometry(&self, node: &ViewStyleNodeKey) -> Option<&ViewFinalGeometry> {
        self.final_nodes.get(node)
    }

    pub fn final_nodes(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ViewStyleNodeKey, &ViewFinalGeometry)> {
        self.final_nodes.iter()
    }

    pub(crate) fn target_node(&self, target: &ViewGeometryTargetKey) -> Option<&ViewStyleNodeKey> {
        self.targets.get(target)
    }

    pub(crate) fn target_geometry(
        &self,
        target: &ViewGeometryTargetKey,
    ) -> Option<(&ViewStyleNodeKey, &ViewFinalGeometry)> {
        let node = self.target_node(target)?;
        self.final_nodes.get(node).map(|geometry| (node, geometry))
    }

    pub(crate) fn target_consumer_hit_rect(
        &self,
        target: &ViewGeometryTargetKey,
        consumer: ViewGeometryConsumer,
    ) -> Result<Option<arcweft_presentation::hit::HitRect>, ViewGeometryRuntimeError> {
        let Some((node, geometry)) = self.target_geometry(target) else {
            return Ok(None);
        };
        consumer_hit_rect(node, geometry, consumer).map_err(|source| {
            ViewGeometryRuntimeError::Conversion {
                node: Some(node.clone()),
                consumer,
                source,
            }
        })
    }

    pub fn is_transparent(&self, node: &ViewStyleNodeKey) -> bool {
        self.transparent_nodes.contains(node)
    }

    pub fn is_suppressed(&self, node: &ViewStyleNodeKey) -> bool {
        self.suppressed_nodes.contains(node)
    }

    #[cfg(test)]
    pub(crate) fn empty_for_test() -> Self {
        Self::new(
            ViewGeometryGeneration::ZERO,
            arcweft_view::geometry::ViewGeometryRect {
                left_milli: 0,
                top_milli: 0,
                right_milli: 0,
                bottom_milli: 0,
            },
            BTreeMap::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeMap::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ViewGeometryGeneration;
    use crate::frame::ViewGeometryRuntimeError;

    #[test]
    fn generation_overflow_is_rejected_before_staging() {
        let current = ViewGeometryGeneration::from_value(u64::MAX);
        assert_eq!(
            current.checked_next(),
            Err(ViewGeometryRuntimeError::GenerationOverflow { current })
        );
    }
}

#[derive(Debug)]
pub(crate) struct ViewGeometryPreparedFrame {
    base_generation: ViewGeometryGeneration,
    next_generation: ViewGeometryGeneration,
    live_nodes: BTreeSet<ViewStyleNodeKey>,
    staged_measure: BTreeMap<ViewStyleNodeKey, ViewMeasureCacheEntry>,
    staged_place: BTreeMap<ViewStyleNodeKey, ViewPlaceCacheEntry>,
    staged_final: BTreeMap<ViewStyleNodeKey, ViewFinalCacheEntry>,
    committed: Arc<ViewCommittedGeometryFrame>,
}

impl PlayerViewGeometryState {
    pub(crate) const fn generation(&self) -> ViewGeometryGeneration {
        self.generation
    }

    pub(super) fn measure_entry(&self, node: &ViewStyleNodeKey) -> Option<&ViewMeasureCacheEntry> {
        self.measure.get(node)
    }

    pub(super) fn place_entry(&self, node: &ViewStyleNodeKey) -> Option<&ViewPlaceCacheEntry> {
        self.place.get(node)
    }

    pub(super) fn final_entry(&self, node: &ViewStyleNodeKey) -> Option<&ViewFinalCacheEntry> {
        self.final_geometry.get(node)
    }

    #[cfg(test)]
    pub(super) fn cache_counts(&self) -> (usize, usize, usize) {
        (
            self.measure.len(),
            self.place.len(),
            self.final_geometry.len(),
        )
    }

    pub(crate) fn commit(&mut self, prepared: ViewGeometryPreparedFrame) {
        debug_assert_eq!(prepared.base_generation, self.generation);
        debug_assert_eq!(prepared.next_generation, prepared.committed.generation);
        debug_assert_eq!(prepared.live_nodes.len(), prepared.staged_final.len());
        self.generation = prepared.next_generation;
        self.measure = prepared.staged_measure;
        self.place = prepared.staged_place;
        self.final_geometry = prepared.staged_final;
    }
}

impl ViewGeometryPreparedFrame {
    pub(super) fn new(
        base_generation: ViewGeometryGeneration,
        next_generation: ViewGeometryGeneration,
        live_nodes: BTreeSet<ViewStyleNodeKey>,
        staged_measure: BTreeMap<ViewStyleNodeKey, ViewMeasureCacheEntry>,
        staged_place: BTreeMap<ViewStyleNodeKey, ViewPlaceCacheEntry>,
        staged_final: BTreeMap<ViewStyleNodeKey, ViewFinalCacheEntry>,
        committed: ViewCommittedGeometryFrame,
    ) -> Self {
        Self {
            base_generation,
            next_generation,
            live_nodes,
            staged_measure,
            staged_place,
            staged_final,
            committed: Arc::new(committed),
        }
    }

    pub(crate) const fn base_generation(&self) -> ViewGeometryGeneration {
        self.base_generation
    }

    pub(crate) const fn next_generation(&self) -> ViewGeometryGeneration {
        self.next_generation
    }

    pub(crate) fn committed(&self) -> &Arc<ViewCommittedGeometryFrame> {
        &self.committed
    }
}

impl ViewCommittedGeometryFrame {
    pub(super) fn new(
        generation: ViewGeometryGeneration,
        viewport: arcweft_view::geometry::ViewGeometryRect,
        final_nodes: BTreeMap<ViewStyleNodeKey, ViewFinalGeometry>,
        transparent_nodes: BTreeSet<ViewStyleNodeKey>,
        suppressed_nodes: BTreeSet<ViewStyleNodeKey>,
        targets: BTreeMap<ViewGeometryTargetKey, ViewStyleNodeKey>,
    ) -> Self {
        debug_assert!(
            final_nodes
                .iter()
                .all(|(node, geometry)| node == &geometry.node)
        );
        debug_assert!(targets.values().all(|node| final_nodes.contains_key(node)));
        Self {
            generation,
            viewport,
            final_nodes,
            transparent_nodes,
            suppressed_nodes,
            targets,
        }
    }
}
