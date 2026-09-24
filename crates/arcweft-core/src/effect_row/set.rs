//! Canonical finite effect sets shared by semantic and executable contracts.

use arcweft_id::{EffectId, EffectIdError};
use std::{collections::BTreeSet, fmt};
use thiserror::Error;
/// Parse failure while constructing an effect set from source labels.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("invalid effect at index {index}: {source}")]
pub struct EffectSetParseError {
    index: usize,
    #[source]
    source: EffectIdError,
}

/// Deterministically ordered set of canonical effects.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectSet(BTreeSet<EffectId>);

impl EffectSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_labels<I, S>(labels: I) -> Result<Self, EffectSetParseError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        labels
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                EffectId::parse(label).map_err(|source| EffectSetParseError { index, source })
            })
            .collect()
    }

    pub fn insert(&mut self, effect: EffectId) -> bool {
        self.0.insert(effect)
    }

    pub fn contains(&self, effect: &EffectId) -> bool {
        self.0.contains(effect)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &EffectId> + DoubleEndedIterator {
        self.0.iter()
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }

    pub fn union_with(&mut self, other: &Self) -> bool {
        let previous_len = self.len();
        self.0.extend(other.iter().cloned());
        self.len() != previous_len
    }

    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        self.iter().chain(other.iter()).cloned().collect()
    }

    #[must_use]
    pub fn difference(&self, other: &Self) -> Self {
        self.0.difference(&other.0).cloned().collect()
    }

    #[must_use]
    pub fn effects_not_covered_by(&self, covering: &Self) -> Self {
        self.iter()
            .filter(|effect| !covering.iter().any(|candidate| candidate.covers(effect)))
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self {
        self.0.intersection(&other.0).cloned().collect()
    }

    pub fn to_labels(&self) -> Vec<String> {
        self.iter().map(ToString::to_string).collect()
    }
}

impl FromIterator<EffectId> for EffectSet {
    fn from_iter<T: IntoIterator<Item = EffectId>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for EffectSet {
    type Item = EffectId;
    type IntoIter = std::collections::btree_set::IntoIter<EffectId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a EffectSet {
    type Item = &'a EffectId;
    type IntoIter = std::collections::btree_set::Iter<'a, EffectId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl fmt::Display for EffectSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("{")?;
        for (index, effect) in self.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{effect}")?;
        }
        formatter.write_str("}")
    }
}
