//! Native scope cleanup for fibers outside a dialogue activation.

use super::{
    NativeDeferInFlight, NativeDeferSource, NativeLexicalDeferred, inspect_defer_captures,
    take_defer_captures,
};
use crate::engine::{
    Engine, FlowControlStackEntryKind, FlowFiberId, FlowFiberOwner, FlowFiberStatus,
    RuntimeStepOutput, code_cursor::NativeSourceOperation, dialogue::DialogueExecutionError,
};
use crate::line_task::{LineRuntimeError, RuntimeLineDeferredRegistration};
use crate::plan::{FlowOp, RuntimeDeferOwner};
use crate::pure::RuntimeCallBackend;
use crate::runtime_id::{RuntimeDeferRegistrationId, RuntimeDeferSiteId};
use crate::scope::ScopeExit;

impl Engine {
    pub(in crate::engine) fn register_flow_defer(
        &mut self,
        source: NativeSourceOperation,
    ) -> Result<(), DialogueExecutionError> {
        let Some(FlowOp::RegisterDefer {
            site,
            outcome,
            captures,
            owner: RuntimeDeferOwner::CurrentScope,
        }) = source.operation(&self.plan)
        else {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        };
        let Some(FlowControlStackEntryKind::Scope { defers, .. }) = self
            .fiber
            .frame
            .control_stack
            .last_mut()
            .map(|entry| &mut entry.kind)
        else {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        };
        if defers.exit.is_some() {
            return Err(LineRuntimeError::InvalidDeferredTransition.into());
        }
        let identity = self
            .flow_defer_frontier
            .prepare()
            .ok_or(LineRuntimeError::DeferRegistrationIdentityExhausted)?;
        let values = inspect_defer_captures(&self.plan, &self.fiber.frame.env, *site, captures)?;
        for value in values {
            if !super::super::dialogue::unique_affine_line_handles(value)?.is_empty() {
                return Err(LineRuntimeError::WrongOwner.into());
            }
        }
        let values = take_defer_captures(&mut self.fiber.frame.env, captures);
        let id = self.flow_defer_frontier.commit(identity);
        defers.deferred.push(NativeLexicalDeferred {
            packet: RuntimeLineDeferredRegistration::new(id, *site, *outcome, values),
            source: NativeDeferSource::Control(source),
        });
        Ok(())
    }

    /// Runs one scope transition under the same checkpoint as the fiber's
    /// other operations. A child holds the sole packet until completion.
    pub(in crate::engine) fn step_flow_defer_scope(
        &mut self,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> Result<bool, DialogueExecutionError> {
        if !matches!(
            self.fiber.owner,
            FlowFiberOwner::Executor | FlowFiberOwner::FlowDefer { .. }
        ) {
            return Ok(false);
        }
        if !self.fiber.frame.scope_defer_exit_pending() {
            return Ok(false);
        }
        let terminal = matches!(
            self.fiber.frame.status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        let requested_exit = if matches!(self.fiber.frame.status, FlowFiberStatus::Failed(_)) {
            ScopeExit::Failed
        } else {
            ScopeExit::Completed
        };
        let Some(scope) = self.fiber.frame.control_stack.len().checked_sub(1) else {
            return Err(LineRuntimeError::InvalidDeferredTransition.into());
        };
        let (exit, prepared) = match &self.fiber.frame.control_stack[scope].kind {
            FlowControlStackEntryKind::Scope { defers, .. } => {
                if defers.inflight.is_some() {
                    return Ok(false);
                }
                let exit = defers.exit.unwrap_or(requested_exit);
                let prepared = defers
                    .deferred
                    .iter()
                    .rposition(|registration| registration.packet.outcome_filter().matches(exit))
                    .map(|index| {
                        (
                            index,
                            self.inspect_deferred_line_child(&defers.deferred[index].packet),
                        )
                    });
                (exit, prepared)
            }
            _ => (requested_exit, None),
        };
        let mut drained = None;
        if let FlowControlStackEntryKind::Scope { defers, .. } =
            &mut self.fiber.frame.control_stack[scope].kind
        {
            if let Some((index, prepared)) = prepared {
                defers.freeze_exit(exit);
                defers.deferred.truncate(index + 1);
                let NativeLexicalDeferred { packet, source } = defers
                    .deferred
                    .pop()
                    .expect("preflight selected one cleanup");
                let prepared = match prepared {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        // An unstartable cleanup fails once. Keep the exit filter
                        // and drain the remaining registrations before reporting it.
                        defers.failure.get_or_insert_with(|| error.to_string());
                        return Ok(true);
                    }
                };
                let registration = packet.id();
                let site = packet.site();
                defers.inflight = Some(NativeDeferInFlight {
                    id: registration,
                    site,
                    source,
                });
                let next_fiber_id = prepared.next_fiber_id;
                let child = self.build_prepared_defer_child(
                    self.fiber.id,
                    self.fiber.frame.presentation_bank,
                    packet,
                    prepared,
                    FlowFiberOwner::FlowDefer {
                        scope,
                        registration,
                        site,
                    },
                );
                self.child_fibers.push_back(child);
                self.next_fiber_id = next_fiber_id;
                self.run_child_next = true;
                return Ok(true);
            }
            drained = Some(std::mem::replace(
                defers,
                super::NativeLexicalDeferScope::new(),
            ));
        }
        let failure = drained.as_ref().and_then(|scope| scope.failure.clone());
        if let Err(error) = self.complete_deferred_scope_step(terminal, failure, output, backend) {
            if let Some(drained) = drained
                && let Some(entry) = self.fiber.frame.control_stack.get_mut(scope)
                && let FlowControlStackEntryKind::Scope { defers, .. } = &mut entry.kind
            {
                *defers = drained;
            }
            return Err(error.into());
        }
        if !self.fiber.frame.scope_defer_exit_pending()
            && matches!(self.fiber.owner, FlowFiberOwner::Executor)
            && let FlowFiberStatus::Failed(message) = &self.fiber.frame.status
        {
            Self::diagnose_runtime_error(message, output);
        }
        Ok(true)
    }

    pub(in crate::engine) fn complete_flow_defer_child(
        &mut self,
        parent: Option<FlowFiberId>,
        scope: usize,
        registration: RuntimeDeferRegistrationId,
        site: RuntimeDeferSiteId,
        failure: Option<String>,
    ) -> Result<(), LineRuntimeError> {
        let parent = parent.ok_or(LineRuntimeError::InvalidDeferredTransition)?;
        let fiber = if self.fiber.id == parent {
            &mut self.fiber
        } else {
            self.child_fibers
                .iter_mut()
                .find(|fiber| fiber.id == parent)
                .ok_or(LineRuntimeError::InvalidDeferredTransition)?
        };
        if fiber.frame.control_stack.len().checked_sub(1) != Some(scope) {
            return Err(LineRuntimeError::InvalidDeferredTransition);
        }
        let FlowControlStackEntryKind::Scope { defers, .. } =
            &mut fiber.frame.control_stack[scope].kind
        else {
            return Err(LineRuntimeError::InvalidDeferredTransition);
        };
        if defers
            .inflight
            .as_ref()
            .is_none_or(|row| row.id != registration || row.site != site)
        {
            return Err(LineRuntimeError::InvalidDeferredTransition);
        }
        defers.inflight = None;
        if let Some(failure) = failure {
            defers.failure.get_or_insert(failure);
        }
        Ok(())
    }
}
