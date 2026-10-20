//! Product AWBC integration for the tier-neutral durable root transaction.

use super::{
    AwbcProductProgram, AwbcProductStepBuildError, AwbcProductStepExecutor, ProductStepError,
    run_function,
};
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
    program: &AwbcProductProgram,
    entry: AwbcEntryId,
) -> Result<Option<RootRuntime>, AwbcProductStepBuildError> {
    let Some(contract) = startup_contract(program, entry).map_err(|error| {
        AwbcProductStepBuildError::RootStartup {
            message: error.to_string(),
        }
    })?
    else {
        return Ok(None);
    };
    RootRuntime::prepare(
        contract,
        crate::program_types::RuntimeProgramTypes::Awbc(program),
    )
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
    let crate::plan::RuntimeEntryRoles::Stateful(_) = &entry_record.roles else {
        return Ok(None);
    };
    RootStartupContract::from_program(
        crate::program_types::RuntimeProgramTypes::Awbc(program),
        &entry_record.runtime_id,
    )
    .map(Some)
}

#[cfg(test)]
fn bind_startup(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    startup: Option<&RootStartup>,
) -> Result<(), AwbcProductStepBuildError> {
    let Some(startup) = startup else {
        return Ok(());
    };
    fiber
        .bind_flow_parameter_coordinates(program, vec![startup.initial_state_binding.clone()])
        .map_err(|error| AwbcProductStepBuildError::FiberState {
            message: error.to_string(),
        })
}

impl AwbcProductStepExecutor {
    pub(super) fn complete_root_initializer(
        &mut self,
        output: &mut RuntimeStepOutput,
    ) -> Result<(), ProductStepError> {
        use crate::awbc::fiber::{
            AwbcFiberRoot, FiberTerminalValue, validate_function_argument_value_refs,
        };
        let AwbcFiberRoot::RootInitializer(entry) = self.fiber.root else {
            return Err(RootRuntimeError::SnapshotRoleMismatch("initializer origin").into());
        };
        let program = self.program.clone();
        let root = self
            .root
            .as_ref()
            .ok_or(RootRuntimeError::SnapshotRoleMismatch("initializer root"))?;
        let selected = program
            .entries
            .get(entry.index())
            .ok_or_else(|| RootRuntimeError::MissingEntry(entry.0.to_string()))?;
        if root.active().entry != selected.runtime_id
            || !matches!(
                root.active().phase,
                crate::root::RootPhase::Initializing { .. }
            )
        {
            return Err(RootRuntimeError::SnapshotRoleMismatch("initializer phase").into());
        }
        let flow = &root.roles().initial_flow;
        let function = program
            .flow_executables
            .iter()
            .find(|candidate| {
                candidate.metadata.flow == flow.flow && candidate.metadata.contract == flow.contract
            })
            .ok_or_else(|| {
                RootRuntimeError::MissingInitialFlowExecutable(
                    selected.runtime_id.canonical_label(),
                )
            })?
            .function;
        let Some(FiberTerminalValue::Returned(Some(value))) = &self.fiber.terminal else {
            return Err(ProductStepError::Internal(
                "initializer returned without its typed value".into(),
            ));
        };
        let inputs = validate_function_argument_value_refs(&program, function, &[value])?;
        let Some(FiberTerminalValue::Returned(Some(value))) = self.fiber.terminal.take() else {
            return Err(ProductStepError::Internal(
                "initializer return owner changed during admission".into(),
            ));
        };
        let root = self
            .root
            .take()
            .ok_or(RootRuntimeError::SnapshotRoleMismatch("initializer root"))?;
        let startup = match root.complete_initialization(
            value,
            crate::program_types::RuntimeProgramTypes::Awbc(&program),
        ) {
            Ok(startup) => startup,
            Err(rejected) => {
                let crate::root::RejectedRootInitialization { root, error } = *rejected;
                self.root = Some(root);
                return Err(error.into());
            }
        };
        let prepared = startup.stage_flow(|_, binding| {
            self.fiber
                .prepare_initial_flow(&program, entry, inputs, vec![binding.value])
        });
        let (root, frame, outcome) = match prepared {
            Ok(prepared) => prepared,
            Err(rejected) => {
                let (root, error) = *rejected;
                self.root = Some(root);
                return Err(error.into());
            }
        };
        self.fiber.commit_initial_flow(frame);
        self.root = Some(root);
        output.root_initializations.push(outcome);
        Ok(())
    }

    #[cfg(test)]
    fn install_root_startup(&mut self, startup: RootStartup) {
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
        let selected = self
            .fiber
            .root
            .entry()
            .ok_or(RootRuntimeError::SnapshotRoleMismatch("entry origin"))?;
        let entry = self
            .program
            .entries
            .get(selected.index())
            .ok_or_else(|| RootRuntimeError::MissingEntry(selected.0.to_string()))?;
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
        let selected = self
            .fiber
            .root
            .entry()
            .ok_or(RootRuntimeError::SnapshotRoleMismatch("entry origin"))?;
        let entry = self
            .program
            .entries
            .get(selected.index())
            .ok_or_else(|| RootRuntimeError::MissingEntry(selected.0.to_string()))?;
        let kind = entry
            .kind
            .runtime_kind(&self.program.strings)
            .ok_or_else(|| RootRuntimeError::MissingEntry(entry.runtime_id.canonical_label()))?;
        if active.id != entry.runtime_id || active.kind != kind || active.binding != entry.binding {
            return Err(RootRuntimeError::SnapshotRoleMismatch("active entry"));
        }
        let initializing = snapshot.as_ref().is_some_and(|snapshot| {
            matches!(snapshot.phase, crate::root::RootPhase::Initializing { .. })
        });
        if initializing
            != matches!(
                self.fiber.root,
                crate::awbc::fiber::AwbcFiberRoot::RootInitializer(_)
            )
        {
            return Err(RootRuntimeError::SnapshotRoleMismatch("initializer frame"));
        }
        let contract = startup_contract(&self.program, selected)?;
        let candidate = match (contract, snapshot) {
            (Some(contract), Some(snapshot)) => Some(RootRuntime::from_snapshot(
                contract,
                snapshot,
                crate::program_types::RuntimeProgramTypes::Awbc(&self.program),
            )?),
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
            if matches!(
                root.active().phase,
                crate::root::RootPhase::Initializing { .. }
            ) {
                return match root.ingress(
                    events,
                    crate::program_types::RuntimeProgramTypes::Awbc(&self.program),
                ) {
                    Ok(()) => true,
                    Err(error) => {
                        output.diagnostics.push(RuntimeDiagnostic::categorized(
                            RuntimeDiagnosticCategory::Input,
                            error.to_string(),
                        ));
                        false
                    }
                };
            }
            let mut evaluator = ProductRootEvaluator {
                program: &self.program,
                format_context: &self.format_context,
                backend: pure_backend,
                fallback_stats: &mut self.compact_pure_stats,
            };
            root.step(
                events,
                &mut evaluator,
                crate::program_types::RuntimeProgramTypes::Awbc(&self.program),
            )
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
    program: &'a AwbcProductProgram,
    format_context: &'a crate::value::RuntimeFormatContext,
    backend: &'a mut B,
    fallback_stats: &'a mut crate::step::RuntimePureCallStats,
}

impl<B: RuntimeCallBackend> RootCallableEvaluator for ProductRootEvaluator<'_, B> {
    fn evaluate_root_callable(
        &mut self,
        callable: &crate::plan::RuntimeCallableRole,
        args: Vec<RuntimeValue>,
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
            self.format_context,
            executable.function,
            args,
            self.backend,
            self.fallback_stats,
        )
        .map_err(|error| RootCallableEvaluationError::new(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_restore_rejects_initializing_state_for_flow_origin_without_mutation() {
        let raw = std::sync::Arc::new(crate::awbc::stateful_root_fixture());
        let program = AwbcProductProgram::admit(
            raw,
            crate::awbc::verify::AwbcVerifyBudget::default(),
            crate::awbc::verify::AwbcVerifyContext::default(),
            crate::plan::RuntimeTaskSourceArtifactLimits::default(),
            None,
        )
        .unwrap();
        let contract = startup_contract(&program, AwbcEntryId(0)).unwrap().unwrap();
        let types = crate::program_types::RuntimeProgramTypes::Awbc(&program);
        let value = types
            .try_record_value(
                contract.roles.state.semantic_identity,
                vec![],
                contract.roles.command_policy.root_limits.schema,
            )
            .unwrap();
        let startup = RootRuntime::prepare(contract.clone(), types)
            .unwrap()
            .complete_initialization(value, types)
            .unwrap();
        let pending = RootRuntime::prepare(contract, types)
            .unwrap()
            .snapshot_state();
        let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 0, 64).unwrap();
        bind_startup(&program, &mut fiber, Some(&startup)).unwrap();
        let fingerprint = AwbcProductStepExecutor::artifact_fingerprint(&program).unwrap();
        let mut executor = AwbcProductStepExecutor::for_fiber(
            program,
            fiber,
            fingerprint,
            crate::task::GenerationId::new(0),
        );
        executor.install_root_startup(startup);
        let active = executor.active_entry_snapshot_identity().unwrap();
        let before = executor.inert_rollback_image().unwrap();
        assert!(matches!(
            executor.restore_root_snapshot(&active, Some(pending)),
            Err(RootRuntimeError::SnapshotRoleMismatch("initializer frame"))
        ));
        assert_eq!(executor.inert_rollback_image().unwrap(), before);
    }
}
