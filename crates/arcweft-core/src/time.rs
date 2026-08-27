use serde::{Deserialize, Serialize};

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
