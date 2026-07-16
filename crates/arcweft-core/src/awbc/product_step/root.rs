//! Product AWBC integration for the tier-neutral durable root transaction.

use super::{AwbcProductStepBuildError, AwbcProductStepExecutor, ProductStepError, run_function};
use crate::awbc::fiber::{FiberState, FiberTrap};
use crate::awbc::schema::{AwbcEntryId, AwbcProgram, AwbcTrapCode};
use crate::pure::RuntimeCallBackend;
use crate::root::{
    RootCallableEvaluationError, RootCallableEvaluator, RootRuntime, RootRuntimeError, RootStartup,
    RootStartupContract,
};
use crate::step::{RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeStepOutput};
use crate::value::RuntimeValue;

pub(super) fn prepare_startup(
    program: &AwbcProgram,
    entry: AwbcEntryId,
) -> Result<Option<RootStartup>, AwbcProductStepBuildError> {
    let Some(contract) = startup_contract(program, entry).map_err(|error| {
        AwbcProductStepBuildError::RootStartup {
            message: error.to_string(),
        }
    })?
    else {
        return Ok(None);
    };
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    let mut fallback_stats = crate::step::RuntimePureCallStats::default();
    let mut evaluator = ProductRootEvaluator {
        program,
        backend: &mut backend,
        fallback_stats: &mut fallback_stats,
    };
    RootRuntime::start(contract, &mut evaluator)
        .map(Some)
        .map_err(|error| AwbcProductStepBuildError::RootStartup {
            message: error.to_string(),
        })
}

fn startup_contract(
    program: &AwbcProgram,
    entry: AwbcEntryId,
) -> Result<Option<RootStartupContract>, RootRuntimeError> {
    let Some(entry_record) = program.entries.get(entry.index()) else {
        return Err(RootRuntimeError::MissingEntry(entry.0.to_string()));
    };
    let crate::plan::RuntimeEntryRoles::Stateful(roles) = &entry_record.roles else {
        return Ok(None);
    };
    let initial_flow = program
        .flow_executables
        .iter()
        .find(|executable| {
            executable.metadata.flow == roles.initial_flow.flow
                && executable.metadata.contract == roles.initial_flow.contract
        })
        .map(|executable| executable.metadata.clone())
        .ok_or_else(|| {
            RootRuntimeError::MissingInitialFlowExecutable(
                entry_record.runtime_id.canonical_label(),
            )
        })?;
    Ok(Some(RootStartupContract {
        entry: entry_record.runtime_id.clone(),
        roles: roles.as_ref().clone(),
        initial_flow,
    }))
}

pub(super) fn bind_startup(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    startup: Option<&RootStartup>,
) -> Result<(), AwbcProductStepBuildError> {
    let Some(startup) = startup else {
        return Ok(());
    };
    fiber
        .bind_entry_arguments(
            program,
            std::slice::from_ref(&startup.initial_state_binding),
        )
        .map_err(|error| AwbcProductStepBuildError::FiberState {
            message: error.to_string(),
        })
}

impl AwbcProductStepExecutor {
    pub(super) fn bind_facade_inputs_preserving_root_flow(
        &mut self,
        root_bindings: &[crate::value::RuntimeBinding],
        input_bindings: &[crate::value::RuntimeBinding],
    ) {
        let protected = self.root_flow_binding_name.as_ref().and_then(|name| {
            self.facade_fiber
                .env
                .get_cloned(name)
                .map(|value| crate::value::RuntimeBinding {
                    name: name.clone(),
                    value,
                })
        });
        self.facade_fiber.env.bind_all_root_ref(root_bindings);
        self.facade_fiber.env.bind_all_root_ref(input_bindings);
        if let Some(binding) = protected {
            self.facade_fiber.env.set_root(binding.name, binding.value);
        }
    }

    pub(super) fn install_root_startup(&mut self, startup: RootStartup) {
        self.facade_fiber.env.set_root(
            startup.initial_state_binding.name.clone(),
            startup.initial_state_binding.value,
        );
        self.root_flow_binding_name = Some(startup.initial_state_binding.name);
        self.root = Some(startup.root);
        self.entry_bound = true;
    }

    #[must_use]
    pub const fn root(&self) -> Option<&RootRuntime> {
        self.root.as_ref()
    }

    pub fn acknowledge_root_commands(
        &mut self,
        accepted: &[crate::root::RuntimeCommandEnvelope],
    ) -> Result<(), RootRuntimeError> {
        match self.root.as_mut() {
            Some(root) => root.acknowledge_published_commands(accepted),
            None if accepted.is_empty() => Ok(()),
            None => Err(RootRuntimeError::CommandAcknowledgementMismatch),
        }
    }

    pub fn active_entry_snapshot_identity(
        &self,
    ) -> Result<crate::entry::ActiveEntrySnapshotV1, RootRuntimeError> {
        let entry = self
            .program
            .entries
            .get(self.fiber.entry.index())
            .ok_or_else(|| RootRuntimeError::MissingEntry(self.fiber.entry.0.to_string()))?;
        let kind = entry
            .kind
            .runtime_kind(&self.program.strings)
            .ok_or_else(|| RootRuntimeError::MissingEntry(entry.runtime_id.canonical_label()))?;
        Ok(crate::entry::ActiveEntrySnapshotV1 {
            id: entry.runtime_id.clone(),
            kind,
            binding: entry.binding,
        })
    }

    #[must_use]
    pub fn root_state_snapshot(&self) -> Option<crate::root::RootStateSnapshotV1> {
        self.root.as_ref().map(RootRuntime::snapshot_state)
    }

    #[must_use]
    pub fn root_save_blockers(&self) -> Option<crate::root::RootSaveBlockers> {
        self.root.as_ref().map(RootRuntime::save_blockers)
    }

    pub fn restore_root_snapshot(
        &mut self,
        active: &crate::entry::ActiveEntrySnapshotV1,
        snapshot: Option<crate::root::RootStateSnapshotV1>,
    ) -> Result<(), RootRuntimeError> {
        let entry = self
            .program
            .entries
            .get(self.fiber.entry.index())
            .ok_or_else(|| RootRuntimeError::MissingEntry(self.fiber.entry.0.to_string()))?;
        let kind = entry
            .kind
            .runtime_kind(&self.program.strings)
            .ok_or_else(|| RootRuntimeError::MissingEntry(entry.runtime_id.canonical_label()))?;
        if active.id != entry.runtime_id || active.kind != kind || active.binding != entry.binding {
            return Err(RootRuntimeError::SnapshotRoleMismatch("active entry"));
        }
        let contract = startup_contract(&self.program, self.fiber.entry)?;
        let candidate = match (contract, snapshot) {
            (Some(contract), Some(snapshot)) => {
                Some(RootRuntime::from_snapshot(contract, snapshot)?)
            }
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => {
                return Err(RootRuntimeError::SnapshotRoleMismatch("root presence"));
            }
        };
        self.root = candidate;
        Ok(())
    }

    pub(super) fn run_root_phase(
        &mut self,
        events: Vec<crate::root::RootEventInput>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> bool {
        let Some(root) = self.root.as_mut() else {
            if events.is_empty() {
                return true;
            }
            output.diagnostics.push(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Input,
                "non-stateful runtime entry cannot accept root events",
            ));
            return false;
        };
        let result = {
            let mut evaluator = ProductRootEvaluator {
                program: &self.program,
                backend: pure_backend,
                fallback_stats: &mut self.compact_pure_stats,
            };
            root.step(events, &mut evaluator)
        };
        match result {
            Ok(result) => {
                let failed = result.failed;
                output.root_transitions.extend(result.outcomes);
                output.root_commands.extend(result.commands);
                if failed {
                    let message = root
                        .failure()
                        .map_or_else(|| "root reducer trapped".to_owned(), ToString::to_string);
                    self.fail_with_error(ProductStepError::Internal(message), output);
                    false
                } else {
                    true
                }
            }
            Err(error) => {
                let step_input_rejection = matches!(
                    &error,
                    RootRuntimeError::InvalidEvent(_)
                        | RootRuntimeError::TransitionSequenceExhausted
                );
                let runtime_failed = matches!(&error, RootRuntimeError::Failed);
                let category = if step_input_rejection {
                    RuntimeDiagnosticCategory::Input
                } else {
                    RuntimeDiagnosticCategory::Internal
                };
                let message = error.to_string();
                output
                    .diagnostics
                    .push(RuntimeDiagnostic::categorized(category, message.clone()));
                if runtime_failed {
                    self.fiber.mark_trapped(FiberTrap {
                        code: AwbcTrapCode::InternalInvariant,
                        message: Some(message),
                        source_map: None,
                    });
                }
                false
            }
        }
    }
}

struct ProductRootEvaluator<'a, B> {
    program: &'a AwbcProgram,
    backend: &'a mut B,
    fallback_stats: &'a mut crate::step::RuntimePureCallStats,
}

impl<B: RuntimeCallBackend> RootCallableEvaluator for ProductRootEvaluator<'_, B> {
    fn evaluate_root_callable(
        &mut self,
        callable: &crate::plan::RuntimeCallableRole,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, RootCallableEvaluationError> {
        let executable = self
            .program
            .callable_executables
            .iter()
            .find(|executable| executable.role == *callable)
            .ok_or_else(|| {
                RootCallableEvaluationError::new(format!(
                    "missing Product AWBC callable `{}` with contract {:?}",
                    callable.callable.as_str(),
                    callable.contract
                ))
            })?;
        run_function(
            self.program,
            executable.function,
            args,
            self.backend,
            self.fallback_stats,
        )
        .map_err(|error| RootCallableEvaluationError::new(error.to_string()))
    }
}
