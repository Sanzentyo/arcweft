//! Relative logical-time suspension owned by the existing Native execution frame.

use super::{Engine, FlowFiberStatus};
use crate::effect::{LineEffectRequest, RuntimeWaitTarget};
use crate::pure::RuntimeCallBackend;
use crate::step::RuntimeStepOutput;
use crate::time::{LogicalDuration, LogicalWait};

impl<A, M> FlowFiberStatus<A, M> {
    /// Latches only a suspension already present at runtime step ingress.
    pub(in crate::engine) fn advance_duration_wait(&mut self, elapsed: LogicalDuration) {
        if let Self::DurationWaiting(wait) = self {
            *self = wait
                .advance(elapsed)
                .map_or(Self::Running, Self::DurationWaiting);
        }
    }
}

impl Engine {
    pub(super) fn latch_duration_waits(&mut self, elapsed: LogicalDuration) {
        self.fiber.frame.status.advance_duration_wait(elapsed);
        for child in &mut self.child_fibers {
            child.frame.status.advance_duration_wait(elapsed);
        }
    }

    /// Instruction dispatch owns suspension; static observation/cleanup packets
    /// do not acquire an execution frame through ordinary effect publication.
    pub(super) fn emit_execution_effect(
        &mut self,
        effect: LineEffectRequest,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        if let LineEffectRequest::Wait(RuntimeWaitTarget::Duration(duration)) = &effect {
            self.fiber.frame.status = LogicalWait::start(*duration)
                .map_or(FlowFiberStatus::Running, FlowFiberStatus::DurationWaiting);
        }
        self.emit_line_effect(effect, output, pure_backend);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_duration_wait_private_rollback_preserves_remaining_time_and_owner() {
        let mut engine = Engine::new(crate::plan::RuntimePlanBuilder::new().finish().unwrap());
        engine.fiber.frame.status = FlowFiberStatus::DurationWaiting(
            LogicalWait::start(LogicalDuration::from_nanos(u64::MAX)).unwrap(),
        );
        engine.latch_duration_waits(LogicalDuration::from_nanos(1));
        let owner = crate::task::RuntimeProgramOwner::Plan(engine.program_plan());
        let image = engine.inert_rollback_image().unwrap();
        let mut restored = Engine::from_rollback_image(image.clone(), &owner).unwrap();
        assert_eq!(restored, engine);
        restored.latch_duration_waits(LogicalDuration::from_nanos(u64::MAX - 2));
        let FlowFiberStatus::DurationWaiting(wait) = &restored.fiber.frame.status else {
            panic!("one nanosecond remains")
        };
        assert_eq!(wait.remaining().as_nanos(), 1);
        restored.latch_duration_waits(LogicalDuration::from_nanos(2));
        assert_eq!(restored.fiber.frame.status, FlowFiberStatus::Running);
        let foreign = crate::task::RuntimeProgramOwner::Plan(std::sync::Arc::new(
            crate::plan::RuntimePlanBuilder::new().finish().unwrap(),
        ));
        assert!(Engine::from_rollback_image(image, &foreign).is_err());
    }
}
