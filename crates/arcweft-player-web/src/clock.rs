use arcweft_runtime_driver::clock::{RuntimeClockError, RuntimeClockStep};
use thiserror::Error;

/// Converts host redraw timestamps to fixed logical runtime steps.
#[derive(Clone, Debug, PartialEq)]
pub struct LogicalClockQuantizer {
    quantum_millis: u32,
    maximum_catch_up_steps: u32,
    last_host_millis: Option<f64>,
    accumulated_millis: f64,
    next_tick: u64,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum LogicalClockQuantizerError {
    #[error("clock quantum must be greater than zero")]
    ZeroQuantum,
    #[error("maximum catch-up steps must be greater than zero")]
    ZeroCatchUpLimit,
    #[error(transparent)]
    Clock(#[from] RuntimeClockError),
}

impl LogicalClockQuantizer {
    pub fn new(
        quantum_millis: u32,
        maximum_catch_up_steps: u32,
    ) -> Result<Self, LogicalClockQuantizerError> {
        if quantum_millis == 0 {
            return Err(LogicalClockQuantizerError::ZeroQuantum);
        }
        if maximum_catch_up_steps == 0 {
            return Err(LogicalClockQuantizerError::ZeroCatchUpLimit);
        }
        Ok(Self {
            quantum_millis,
            maximum_catch_up_steps,
            last_host_millis: None,
            accumulated_millis: 0.0,
            next_tick: 1,
        })
    }

    /// Returns one or more fixed logical steps. Host wall-clock values determine
    /// only how many quanta are due; exact timestamps never enter VM semantics.
    pub fn advance(
        &mut self,
        host_millis: f64,
    ) -> Result<Vec<RuntimeClockStep>, LogicalClockQuantizerError> {
        let Some(previous) = self.last_host_millis.replace(host_millis) else {
            return self.take_steps(1);
        };
        let elapsed = (host_millis - previous).clamp(0.0, 1_000.0);
        self.accumulated_millis += elapsed;
        let mut due = 0;
        while due < self.maximum_catch_up_steps
            && self.accumulated_millis >= f64::from(self.quantum_millis)
        {
            self.accumulated_millis -= f64::from(self.quantum_millis);
            due += 1;
        }
        if due == 0 {
            return Ok(Vec::new());
        }
        self.take_steps(due)
    }

    fn take_steps(
        &mut self,
        count: u32,
    ) -> Result<Vec<RuntimeClockStep>, LogicalClockQuantizerError> {
        (0..count)
            .map(|_| {
                let clock = RuntimeClockStep::from_millis(self.next_tick, self.quantum_millis)?;
                self.next_tick = self.next_tick.saturating_add(1);
                Ok(clock)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantizer_never_forwards_fractional_wall_time() {
        let mut clock = LogicalClockQuantizer::new(16, 4).expect("clock");
        assert_eq!(clock.advance(100.25).expect("first").len(), 1);
        assert!(clock.advance(107.75).expect("not due").is_empty());
        let due = clock.advance(116.50).expect("due");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].dt_millis(), 16);
    }
}
