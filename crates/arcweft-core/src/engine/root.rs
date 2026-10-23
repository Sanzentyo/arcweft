//! Native Root reducers enter the ordinary executor with their original role.

use super::*;
use crate::root::{RootPhase, RootTransitionOutcome};

#[cfg(test)]
mod tests;

impl Engine {
    pub(super) fn root_has_pending_reduction(&self) -> bool {
        self.root.as_ref().is_some_and(|root| {
            root.failure().is_none()
                && matches!(root.active().phase, RootPhase::Initialized { .. })
                && root.active().pending_event_count() != 0
        })
    }

    /// Frame preparation consumes no instruction and calls no backend. The
    /// queued event moves only after its complete borrowed input admission.
    pub(super) fn prepare_root_reducer(&mut self, output: &mut RuntimeStepOutput) -> bool {
        let Some(root) = &self.root else {
            return true;
        };
        if root.failure().is_some() {
            return false;
        }
        if matches!(root.active().phase, RootPhase::Initializing { .. })
            || root.active().reducer_active()
            || root.active().pending_event_count() == 0
            || matches!(
                output.root_transitions.last(),
                Some(RootTransitionOutcome::Rejected { .. })
            )
        {
            return true;
        }
        if let Err(message) = self.stage_root_reducer() {
            let root = self.root.as_mut().expect("selected Root remains owned");
            let error = RootCallableEvaluationError::new(message.clone());
            let program = crate::program_types::RuntimeProgramTypes::Plan(&self.plan);
            let outcome = if root.active().reducer_active() {
                root.complete_reduction(Err(error), program)
            } else {
                root.trap_pending_reduction(error, program)
            };
            match outcome {
                Ok(outcome) => output.root_transitions.push(outcome),
                Err(error) => output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Internal,
                    error.to_string(),
                )),
            }
            self.fail_eval(message, output);
            return false;
        }
        true
    }

    fn stage_root_reducer(&mut self) -> Result<(), String> {
        let plan = Arc::clone(&self.plan);
        let root = self.root.as_ref().expect("selected Root remains owned");
        let inputs = root
            .pending_reduction_inputs()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Root reducer has no pending event".to_owned())?;
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            crate::task::RuntimeProgramOwner::Plan(Arc::clone(&plan)),
            root.roles().command_policy.root_limits.schema,
        );
        admission
            .validate_metadata(&root.active().entry)
            .and_then(|()| admission.validate_metadata(inputs.callable))
            .map_err(|error| error.to_string())?;
        for value in inputs.arguments {
            admission
                .validate_owned_borrowed(value.into())
                .map_err(|error| error.to_string())?;
        }
        let site = plan
            .callable_executables()
            .iter()
            .find_map(|executable| {
                if executable.callable != inputs.callable.callable
                    || executable.contract != inputs.callable.contract
                {
                    return None;
                }
                match executable.code {
                    RuntimeCallableExecutableCode::FunctionSite(site) => Some(site),
                    RuntimeCallableExecutableCode::ControllerFlow(_) => None,
                }
            })
            .ok_or_else(|| "Root reducer has no original function site".to_owned())?;
        let declaration = plan
            .validate_function_site_input_refs(site, &[], &inputs.arguments)
            .map_err(|error| error.to_string())?;
        declaration
            .validate_owned_patterns(&[], &inputs.arguments)
            .map_err(|error| error.to_string())?;
        let (mut child, next_fiber_id) = self
            .prepare_child_fiber(std::collections::VecDeque::new(), &[])
            .map_err(|error| error.to_string())?;
        let entry = root.active().entry.clone();
        child.owner = FlowFiberOwner::RootReducer {
            entry: entry.clone(),
        };
        let (_, arguments) = self
            .root
            .as_mut()
            .expect("selected Root remains owned")
            .begin_reduction()
            .map_err(|error| error.to_string())?
            .expect("borrowed admission found the next reducer event")
            .into_parts();
        std::mem::swap(&mut self.fiber, &mut child);
        let staged = self.stage_admitted_function_site_call(
            declaration,
            Vec::new(),
            arguments,
            FunctionCallFrame::new(site, FunctionReturnContinuation::RootReducer { entry }),
            None,
        );
        std::mem::swap(&mut self.fiber, &mut child);
        staged.map_err(|error| error.to_string())?;
        self.next_fiber_id = next_fiber_id;
        self.child_fibers.push_front(child);
        Ok(())
    }

    pub(in crate::engine) fn finish_root_reduction(
        &mut self,
        entry: &EntryRuntimeId,
        returned: Result<RuntimeValue, RootCallableEvaluationError>,
        output: &mut RuntimeStepOutput,
    ) -> Result<(), String> {
        let root = self
            .root
            .as_mut()
            .filter(|root| root.active().entry == *entry)
            .ok_or_else(|| "native reducer return differs from its Root owner".to_owned())?;
        let published_before = root.active().pending_command_count();
        let outcome = root
            .complete_reduction(
                returned,
                crate::program_types::RuntimeProgramTypes::Plan(&self.plan),
            )
            .map_err(|error| error.to_string())?;
        output.root_transitions.push(outcome);
        output
            .root_commands
            .extend(root.committed_commands().skip(published_before).cloned());
        Ok(())
    }
}
