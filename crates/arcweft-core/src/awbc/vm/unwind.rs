//! One exit step for an invocation stack, independent of its callback scheduler.

use super::*;

pub(super) fn step_exiting_scope(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    execution: &mut VmExecutionResources<'_>,
) -> Result<VmStepOutput, VmError> {
    let mut observations = Vec::new();
    if let Some(scope) = fiber.active_frame()?.scopes.last() {
        let id = scope.id;
        if scope.deferred.defers.is_empty() {
            execution.retire_exiting_scope(program, fiber)?;
            let (cleanups, failure) = fiber.pop_exiting_scope(program, Some(id))?;
            if let Some(failure) = failure {
                fiber.record_terminal_defer_failure(failure);
            }
            observations.extend(
                cleanups
                    .into_iter()
                    .rev()
                    .map(|cleanup| VmObservation::Effect {
                        effect: cleanup.effect,
                        args: cleanup.args,
                    }),
            );
            observations.push(VmObservation::Drop {
                policy: crate::effect::RuntimeDropPolicy::Default,
            });
        } else {
            observations.push(VmObservation::ScopedDeferUnwind {
                cursor: fiber.cursor,
                scope: Some(id),
            });
        }
    } else if !fiber.active_frame()?.root_deferred.defers.is_empty() {
        observations.push(VmObservation::ScopedDeferUnwind {
            cursor: fiber.cursor,
            scope: None,
        });
    } else {
        execution.retire_exiting_scope(program, fiber)?;
        let (_, failure) = fiber.pop_exiting_scope(program, None)?;
        if let Some(failure) = failure {
            fiber.record_terminal_defer_failure(failure);
        }
        observations.extend(fiber.take_active_frame_cleanups(program)?.into_iter().map(
            |cleanup| VmObservation::Effect {
                effect: cleanup.effect,
                args: cleanup.args,
            },
        ));
        if fiber.frames.len() > 1 {
            fiber.retire_exiting_callee()?;
        } else if fiber.pending_flow_transfer.is_some() {
            if fiber.status.is_terminal() {
                execution.discard_pending_flow_transfer(program, fiber)?;
            } else {
                let function = fiber.complete_flow_transfer(program)?;
                observations.push(VmObservation::Goto(function));
            }
        }
        observations.push(VmObservation::Drop {
            policy: crate::effect::RuntimeDropPolicy::Default,
        });
    }
    let exit = if fiber.pending_flow_transfer.is_some() || fiber.has_deferred_work() {
        VmExit::Running
    } else {
        terminal_exit(fiber)
    };
    Ok(VmStepOutput {
        executed: 1,
        exit,
        observations,
    })
}
