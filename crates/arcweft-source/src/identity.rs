//! In-memory source lineage identities.

use crate::SourceName;
use core::num::NonZeroU32;
use thiserror::Error;

/// Monotonic generation within one in-memory source lineage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceGeneration(NonZeroU32);

impl SourceGeneration {
    /// Generation assigned to the first successfully parsed source snapshot.
    pub const INITIAL: Self = Self(NonZeroU32::MIN);

    /// Returns the non-zero numeric generation for diagnostics and tests.
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Advances the lineage without wrapping or reusing a generation.
    pub fn checked_next(self) -> Result<Self, SourceGenerationExhausted> {
        self.0
            .get()
            .checked_add(1)
            .and_then(NonZeroU32::new)
            .map(Self)
            .ok_or(SourceGenerationExhausted)
    }
}

/// Failure to advance an exhausted source lineage.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("source generation is exhausted")]
pub struct SourceGenerationExhausted;

/// Source name paired with its generation in one parse-database lineage.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceSnapshotId {
    name: SourceName,
    generation: SourceGeneration,
}

impl SourceSnapshotId {
    /// Starts a source lineage at generation one.
    pub fn initial(name: SourceName) -> Self {
        Self {
            name,
            generation: SourceGeneration::INITIAL,
        }
    }

    /// Returns the source-name authority retained by this snapshot.
    pub const fn name(&self) -> &SourceName {
        &self.name
    }

    /// Returns the snapshot generation.
    pub const fn generation(&self) -> SourceGeneration {
        self.generation
    }

    /// Advances this lineage while retaining the exact source name.
    pub fn checked_next(&self) -> Result<Self, SourceGenerationExhausted> {
        Ok(Self {
            name: self.name.clone(),
            generation: self.generation.checked_next()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceGeneration, SourceGenerationExhausted, SourceSnapshotId};
    use crate::SourceName;
    use core::num::NonZeroU32;

    #[test]
    fn source_snapshot_generations_start_at_one_and_never_wrap() {
        let initial = SourceSnapshotId::initial(SourceName::path("story.arcw"));
        assert_eq!(initial.generation(), SourceGeneration::INITIAL);
        assert_eq!(initial.generation().get(), 1);

        let next = initial.checked_next().expect("generation advances");
        assert_eq!(next.name(), initial.name());
        assert_eq!(next.generation().get(), 2);

        let exhausted = SourceGeneration(NonZeroU32::new(u32::MAX).unwrap());
        assert_eq!(exhausted.checked_next(), Err(SourceGenerationExhausted));
    }
}
