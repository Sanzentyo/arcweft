//! Session-local typed identity vocabulary for runtime assertions.

use arcweft_lang_hir::syntax::assertion::AssertionMode;
use thiserror::Error;

/// Runtime-capable assertion mode retained by a fresh compilation session.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAssertionMode {
    /// Release-visible runtime assertion.
    Check,
    /// Debug-profile-only runtime assertion.
    Debug,
}

impl RuntimeAssertionMode {
    /// Converts the typed source/HIR mode without inventing a runtime form for
    /// proof-only assertions.
    pub fn try_from_assertion_mode(mode: AssertionMode) -> Result<Self, RuntimeAssertionModeError> {
        match mode {
            AssertionMode::Check => Ok(Self::Check),
            AssertionMode::Debug => Ok(Self::Debug),
            AssertionMode::Prove => Err(RuntimeAssertionModeError::ProveHasNoRuntimeRepresentation),
        }
    }
}

/// Failure to convert a source assertion mode into runtime identity.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeAssertionModeError {
    /// `assert.prove` is verification-only and never owns a runtime guard.
    #[error("proof assertions have no runtime representation")]
    ProveHasNoRuntimeRepresentation,
}

/// Zero-based authored position of one condition in an assertion statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssertionConditionIndex(u8);

impl AssertionConditionIndex {
    /// Constructs an index after validating both the authored list size and
    /// the selected position against the language's 64-condition limit.
    pub fn try_new(
        index: usize,
        condition_count: usize,
    ) -> Result<Self, AssertionConditionIndexError> {
        if !(1..=64).contains(&condition_count) {
            return Err(AssertionConditionIndexError::InvalidConditionCount {
                count: condition_count,
            });
        }
        if index >= condition_count {
            return Err(AssertionConditionIndexError::OutOfBounds {
                index,
                count: condition_count,
            });
        }
        let narrowed =
            u8::try_from(index).map_err(|_| AssertionConditionIndexError::OutOfBounds {
                index,
                count: condition_count,
            })?;
        Ok(Self(narrowed))
    }

    /// Returns the zero-based authored condition position.
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Invalid assertion condition position or authored list size.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AssertionConditionIndexError {
    /// Runtime assertion statements must carry between one and 64 conditions.
    #[error("assertion condition count must be in 1..=64")]
    InvalidConditionCount { count: usize },
    /// The requested zero-based position is not present in the authored list.
    #[error("assertion condition index is outside the authored condition list")]
    OutOfBounds { index: usize, count: usize },
}

#[cfg(test)]
mod tests {
    use super::{
        AssertionConditionIndex, AssertionConditionIndexError, RuntimeAssertionMode,
        RuntimeAssertionModeError,
    };
    use arcweft_lang_hir::syntax::assertion::AssertionMode;

    #[test]
    fn only_runtime_capable_typed_modes_convert() {
        assert_eq!(
            RuntimeAssertionMode::try_from_assertion_mode(AssertionMode::Check),
            Ok(RuntimeAssertionMode::Check)
        );
        assert_eq!(
            RuntimeAssertionMode::try_from_assertion_mode(AssertionMode::Debug),
            Ok(RuntimeAssertionMode::Debug)
        );
        assert_eq!(
            RuntimeAssertionMode::try_from_assertion_mode(AssertionMode::Prove),
            Err(RuntimeAssertionModeError::ProveHasNoRuntimeRepresentation)
        );
    }

    #[test]
    fn condition_index_validates_authored_order_and_limit() {
        assert_eq!(AssertionConditionIndex::try_new(0, 1).unwrap().get(), 0);
        assert_eq!(AssertionConditionIndex::try_new(63, 64).unwrap().get(), 63);
        assert_eq!(
            AssertionConditionIndex::try_new(0, 0),
            Err(AssertionConditionIndexError::InvalidConditionCount { count: 0 })
        );
        assert_eq!(
            AssertionConditionIndex::try_new(0, 65),
            Err(AssertionConditionIndexError::InvalidConditionCount { count: 65 })
        );
        assert_eq!(
            AssertionConditionIndex::try_new(64, 64),
            Err(AssertionConditionIndexError::OutOfBounds {
                index: 64,
                count: 64,
            })
        );
    }
}
