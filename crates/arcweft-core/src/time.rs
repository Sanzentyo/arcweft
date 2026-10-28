use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct TickId(pub u64);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct LogicalDuration {
    nanos: u64,
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct LogicalTime {
    tick: TickId,
    elapsed: LogicalDuration,
}

/// A suspended relative wait. Zero duration completes without a suspended
/// state; each later runtime ingress supplies its logical elapsed duration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalWait {
    remaining_nanos: NonZeroU64,
}

impl LogicalWait {
    pub const fn start(duration: LogicalDuration) -> Option<Self> {
        match NonZeroU64::new(duration.as_nanos()) {
            Some(remaining_nanos) => Some(Self { remaining_nanos }),
            None => None,
        }
    }

    pub const fn remaining(self) -> LogicalDuration {
        LogicalDuration::from_nanos(self.remaining_nanos.get())
    }

    /// Consumes elapsed time only for a wait that already existed at ingress.
    /// Returning None retires the wait before execution resumes.
    pub const fn advance(self, elapsed: LogicalDuration) -> Option<Self> {
        Self::start(LogicalDuration::from_nanos(
            self.remaining_nanos
                .get()
                .saturating_sub(elapsed.as_nanos()),
        ))
    }
}

impl LogicalTime {
    pub const fn new(tick: TickId, elapsed: LogicalDuration) -> Self {
        Self { tick, elapsed }
    }

    pub const fn tick(self) -> TickId {
        self.tick
    }

    pub const fn elapsed(self) -> LogicalDuration {
        self.elapsed
    }
}

impl LogicalDuration {
    pub const fn from_nanos(nanos: u64) -> Self {
        Self { nanos }
    }

    pub const fn as_nanos(self) -> u64 {
        self.nanos
    }

    /// Adds logical elapsed time without wrapping at the representation limit.
    #[must_use]
    pub const fn saturating_add(self, other: Self) -> Self {
        Self::from_nanos(self.nanos.saturating_add(other.nanos))
    }

    /// Adds logical elapsed time without changing overflow into a valid
    /// deadline.
    #[must_use]
    pub const fn checked_add(self, other: Self) -> Option<Self> {
        match self.nanos.checked_add(other.nanos) {
            Some(nanos) => Some(Self::from_nanos(nanos)),
            None => None,
        }
    }
}

impl Default for LogicalDuration {
    fn default() -> Self {
        Self::from_nanos(0)
    }
}

#[cfg(test)]
mod tests {
    use super::{LogicalDuration, LogicalWait};

    #[test]
    fn relative_wait_completes_at_zero_and_preserves_remaining_time_without_overflow() {
        let nanos = LogicalDuration::from_nanos;
        assert_eq!(LogicalWait::start(nanos(0)), None);
        let wait = LogicalWait::start(nanos(u64::MAX)).unwrap();
        assert_eq!(wait.advance(nanos(0)), Some(wait));
        let wait = wait.advance(nanos(u64::MAX - 10)).unwrap();
        assert_eq!(wait.remaining(), nanos(10));
        assert_eq!(wait.advance(nanos(9)).unwrap().remaining(), nanos(1));
        assert_eq!(wait.advance(nanos(10)), None);
        assert_eq!(wait.advance(nanos(u64::MAX)), None);
    }

    #[test]
    fn relative_wait_wire_preserves_remaining_time_and_rejects_zero_suspension() {
        let wait = LogicalWait::start(LogicalDuration::from_nanos(17)).unwrap();
        let bytes = serde_json::to_vec(&wait).unwrap();
        assert_eq!(serde_json::from_slice::<LogicalWait>(&bytes).unwrap(), wait);
        assert!(serde_json::from_str::<LogicalWait>(r#"{"remaining_nanos":0}"#).is_err());
    }
}
