//! Canonical typed Style result shared by every View runtime consumer.

use super::{ViewPropertyKind, ViewSpecifiedValue, ViewStyleInvalidationSet};
use std::collections::BTreeMap;

use super::cascade::{ViewStyleContributionSource, ViewStylePriority};

/// Revision carried by one computed result for parent/cache invalidation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComputedViewStyleRevision(u64);

/// One winning typed property together with deterministic cascade provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputedViewProperty {
    value: ViewSpecifiedValue,
    priority: ViewStylePriority,
    source: ViewStyleContributionSource,
}

/// Fully token-resolved Style for one retained View node and state snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComputedViewStyle {
    properties: BTreeMap<ViewPropertyKind, ComputedViewProperty>,
    revision: ComputedViewStyleRevision,
}

impl ComputedViewStyleRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ComputedViewProperty {
    pub(super) const fn new(
        value: ViewSpecifiedValue,
        priority: ViewStylePriority,
        source: ViewStyleContributionSource,
    ) -> Self {
        Self {
            value,
            priority,
            source,
        }
    }

    pub const fn value(&self) -> &ViewSpecifiedValue {
        &self.value
    }

    pub const fn priority(&self) -> ViewStylePriority {
        self.priority
    }

    pub const fn source(&self) -> &ViewStyleContributionSource {
        &self.source
    }
}

impl ComputedViewStyle {
    pub(super) const fn from_properties(
        properties: BTreeMap<ViewPropertyKind, ComputedViewProperty>,
        revision: ComputedViewStyleRevision,
    ) -> Self {
        Self {
            properties,
            revision,
        }
    }

    pub const fn revision(&self) -> ComputedViewStyleRevision {
        self.revision
    }

    pub fn property(&self, property: ViewPropertyKind) -> Option<&ComputedViewProperty> {
        self.properties.get(&property)
    }

    pub fn value(&self, property: ViewPropertyKind) -> Option<&ViewSpecifiedValue> {
        self.property(property).map(ComputedViewProperty::value)
    }

    pub fn properties(
        &self,
    ) -> impl ExactSizeIterator<Item = (ViewPropertyKind, &ComputedViewProperty)> {
        self.properties
            .iter()
            .map(|(property, value)| (*property, value))
    }

    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    /// Exact retained work caused by moving from `previous` to this result.
    pub fn invalidation_from(&self, previous: &Self) -> ViewStyleInvalidationSet {
        ViewPropertyKind::ALL
            .iter()
            .copied()
            .filter(|property| self.value(*property) != previous.value(*property))
            .fold(ViewStyleInvalidationSet::NONE, |invalidation, property| {
                invalidation.union(property.default_invalidation())
            })
    }
}
