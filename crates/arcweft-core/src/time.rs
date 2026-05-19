#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TickId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogicalDuration {
    nanos: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
}

impl Default for LogicalDuration {
    fn default() -> Self {
        Self::from_nanos(0)
    }
}
