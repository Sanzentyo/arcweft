//! Stable identities carried by authored dialogue View occurrences and actions.

use serde::{Deserialize, Serialize};

/// Stable runtime identity for one independently targeted dialogue history.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DialoguePresentationId(u64);

impl DialoguePresentationId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable identity for one retained dialogue occurrence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DialogueEntryId(u64);

impl DialogueEntryId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Monotonic identity for one execution of a dialogue line.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct DialogueInstanceId(u64);

impl DialogueInstanceId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Zero-based input-gated stage within one dialogue occurrence.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct DialogueStageIndex(u32);

impl DialogueStageIndex {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn as_usize(self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }

    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Monotonic mutation revision for one dialogue presentation.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct DialogueRevision(u64);

impl DialogueRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Stale-safe action target captured from one authored dialogue View frame.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DialogueAdvanceTarget {
    pub dialogue: DialoguePresentationId,
    pub entry: DialogueEntryId,
    pub instance: DialogueInstanceId,
    pub stage: DialogueStageIndex,
    pub revision: DialogueRevision,
}

impl DialogueAdvanceTarget {
    #[must_use]
    pub const fn new(
        dialogue: DialoguePresentationId,
        entry: DialogueEntryId,
        instance: DialogueInstanceId,
        stage: DialogueStageIndex,
        revision: DialogueRevision,
    ) -> Self {
        Self {
            dialogue,
            entry,
            instance,
            stage,
            revision,
        }
    }
}
