use arcweft_core::time::{LogicalDuration, TickId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Host-provided deterministic logical time for exactly one runtime step.
///
/// There is deliberately no `Default` implementation. A browser player must
/// quantize its animation/redraw clock before entering runtime semantics.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeClockStep {
    tick: TickId,
    dt: LogicalDuration,
}

/// Invalid logical clock input rejected before a VM step.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum RuntimeClockError {
    #[error("logical tick must be greater than zero")]
    ZeroTick,
    #[error("logical dt_millis must be greater than zero")]
    ZeroDelta,
    #[error("logical dt_millis cannot be represented as nanoseconds")]
    DeltaOverflow,
}

impl RuntimeClockStep {
    /// Creates a non-zero, millisecond-quantized logical clock step.
    pub fn from_millis(tick: u64, dt_millis: u32) -> Result<Self, RuntimeClockError> {
        if tick == 0 {
            return Err(RuntimeClockError::ZeroTick);
        }
        if dt_millis == 0 {
            return Err(RuntimeClockError::ZeroDelta);
        }
        let nanos = u64::from(dt_millis)
            .checked_mul(1_000_000)
            .ok_or(RuntimeClockError::DeltaOverflow)?;
        Ok(Self {
            tick: TickId(tick),
            dt: LogicalDuration::from_nanos(nanos),
        })
    }

    pub const fn tick(self) -> TickId {
        self.tick
    }

    pub const fn dt(self) -> LogicalDuration {
        self.dt
    }

    pub const fn dt_millis(self) -> u64 {
        self.dt.as_nanos() / 1_000_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_rejects_implicit_zero_time() {
        assert_eq!(
            RuntimeClockStep::from_millis(0, 16),
            Err(RuntimeClockError::ZeroTick)
        );
        assert_eq!(
            RuntimeClockStep::from_millis(1, 0),
            Err(RuntimeClockError::ZeroDelta)
        );
    }

    #[test]
    fn clock_converts_quantized_millis_to_core_time() {
        let clock = RuntimeClockStep::from_millis(7, 16).expect("clock is valid");
        assert_eq!(clock.tick(), TickId(7));
        assert_eq!(clock.dt().as_nanos(), 16_000_000);
    }
}
