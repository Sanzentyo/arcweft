//! A returning task keeps its value in the actual fiber until its attached
//! descendants finish. Only the common producer registry receives that owner.

use super::*;
use crate::awbc::fiber::{FiberStatus, FiberTerminalValue};
use crate::task::{ProgramTaskOutcome, RuntimeTaskFailure, RuntimeTaskFailureKind};

impl AwbcProductStepExecutor {
    pub(in crate::awbc::product_step) fn has_program_children(
        &self,
        parent: RuntimeFiberInstanceId,
    ) -> bool {
        self.child_fibers.iter().any(|child| {
            matches!(&child.owner,
            ProductChildFiberOwner::Program { parent: owner, .. } if *owner == parent)
        })
    }

    pub(in crate::awbc::product_step) fn child_has_internal_work(
        &self,
        child: &ProductChildFiber,
    ) -> bool {
        child.has_internal_work()
            && !(matches!(
                child.fiber.status,
                FiberStatus::Returned | FiberStatus::Cancelled | FiberStatus::Trapped
            ) && self.has_program_children(child.fiber.instance))
    }

    pub(in crate::awbc::product_step) fn retire_program_child(
        &mut self,
        mut child: ProductChildFiber,
        output: &mut RuntimeStepOutput,
    ) {
        let ProductChildFiberOwner::Program { launch, .. } = &child.owner else {
            unreachable!("Program retirement owns an accepted launch")
        };
        if self.has_program_children(child.fiber.instance) {
            self.child_fibers.push_back(child);
            return;
        }
        let terminal = child.fiber.terminal.take();
        let (outcome, fallback, explicit) = match terminal {
            Some(FiberTerminalValue::Returned(value)) => {
                let explicit = value.is_some();
                (
                    ProgramTaskOutcome::Returned(value.unwrap_or(RuntimeValue::Unit)),
                    FiberTerminalValue::Returned(None),
                    explicit,
                )
            }
            Some(FiberTerminalValue::Cancelled) => (
                ProgramTaskOutcome::Cancelled,
                FiberTerminalValue::Cancelled,
                false,
            ),
            Some(FiberTerminalValue::Trapped(trap)) => {
                let failure = RuntimeTaskFailure {
                    kind: RuntimeTaskFailureKind::WorkerFailure,
                    diagnostic: crate::task::BoundedRuntimeDiagnostic::from_borrowed_message(
                        trap.message
                            .as_deref()
                            .unwrap_or("AWBC Program task trapped"),
                    ),
                };
                (
                    ProgramTaskOutcome::Failed(failure),
                    FiberTerminalValue::Trapped(trap),
                    false,
                )
            }
            terminal => {
                child.fiber.terminal = terminal;
                self.child_fibers.push_front(child);
                self.fail_with_error(
                    ProductStepError::Internal(
                        "Program retirement has no owned body outcome".into(),
                    ),
                    output,
                );
                return;
            }
        };
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            launch.source().program_owner(),
            RuntimeSchemaLimits::engine_default(),
        );
        match self
            .need_producers
            .complete_program_task(launch, outcome, &mut admission)
        {
            Ok(report) => output.requests.program_task_completions.push(report),
            Err(rejected) => {
                child.fiber.terminal = Some(match rejected.outcome {
                    ProgramTaskOutcome::Returned(value) if explicit => {
                        FiberTerminalValue::Returned(Some(value))
                    }
                    _ => fallback,
                });
                self.child_fibers.push_front(child);
                self.fail_with_error(
                    ProductStepError::Internal(rejected.reason.to_string()),
                    output,
                );
            }
        }
    }
}
