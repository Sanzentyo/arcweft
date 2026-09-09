//! Explicit live playback versus visual sampling for a prepared frame.

use arcweft_text_model::{DialogueRevealElapsed, DialogueRevealPolicy};
use thiserror::Error;

/// One visual clock for images, View animation and dialogue presentation.
/// Live playback retains each dialogue stage's logical elapsed time. Sampling
/// evaluates stage-local reveal and Fx at the requested elapsed time without
/// advancing or modifying the retained runtime state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerFrameTime {
    /// Render live state while retaining each dialogue's logical clock.
    Runtime { visual_millis: u64 },
    /// Evaluate a frozen presentation at a stage-local elapsed time.
    Sample {
        elapsed: DialogueRevealElapsed,
        reveal_policy: DialogueRevealPolicy,
    },
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PlayerFrameTimeError {
    #[error("frame sample time {millis} ms exceeds the exact nanosecond range")]
    SampleOutOfRange { millis: u64 },
}

impl PlayerFrameTime {
    #[must_use]
    pub const fn runtime(visual_millis: u64) -> Self {
        Self::Runtime { visual_millis }
    }

    /// Build a sample using the same exact nanosecond domain as dialogue reveal.
    ///
    /// # Errors
    /// Returns [`PlayerFrameTimeError::SampleOutOfRange`] when converting the
    /// requested milliseconds to nanoseconds would overflow.
    pub fn sample_millis(
        millis: u64,
        reveal_policy: DialogueRevealPolicy,
    ) -> Result<Self, PlayerFrameTimeError> {
        let nanos = millis
            .checked_mul(1_000_000)
            .ok_or(PlayerFrameTimeError::SampleOutOfRange { millis })?;
        Ok(Self::Sample {
            elapsed: DialogueRevealElapsed::from_nanos(nanos),
            reveal_policy,
        })
    }

    pub(super) const fn visual_millis(self) -> u64 {
        match self {
            Self::Runtime { visual_millis } => visual_millis,
            Self::Sample { elapsed, .. } => elapsed.as_nanos() / 1_000_000,
        }
    }

    pub(super) const fn dialogue_elapsed(
        self,
        retained: DialogueRevealElapsed,
    ) -> DialogueRevealElapsed {
        match self {
            Self::Runtime { .. } => retained,
            Self::Sample { elapsed, .. } => elapsed,
        }
    }

    pub(super) const fn dialogue_policy(
        self,
        retained: DialogueRevealPolicy,
    ) -> DialogueRevealPolicy {
        match self {
            Self::Runtime { .. } => retained,
            Self::Sample { reveal_policy, .. } => DialogueRevealPolicy {
                complete_stage: retained.complete_stage || reveal_policy.complete_stage,
                instant_characters: retained.instant_characters || reveal_policy.instant_characters,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_keep_exact_nanoseconds_and_reject_the_first_overflowing_millisecond() {
        let last_millis = 18_446_744_073_709;
        let last =
            PlayerFrameTime::sample_millis(last_millis, DialogueRevealPolicy::default()).unwrap();
        assert_eq!(last.visual_millis(), last_millis);
        assert_eq!(
            last.dialogue_elapsed(DialogueRevealElapsed::from_nanos(0))
                .as_nanos(),
            18_446_744_073_709_000_000
        );
        assert_eq!(
            PlayerFrameTime::sample_millis(last_millis + 1, DialogueRevealPolicy::default()),
            Err(PlayerFrameTimeError::SampleOutOfRange {
                millis: last_millis + 1
            })
        );
        assert_eq!(
            PlayerFrameTime::sample_millis(u64::MAX, DialogueRevealPolicy::default()),
            Err(PlayerFrameTimeError::SampleOutOfRange { millis: u64::MAX })
        );
    }
}
