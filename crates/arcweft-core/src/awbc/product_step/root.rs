//! Product AWBC integration for the tier-neutral durable root transaction.

use super::{
    AwbcProductProgram, AwbcProductStepBuildError, AwbcProductStepExecutor, ProductStepError,
};
#[cfg(test)]
use crate::awbc::fiber::FiberState;
use crate::awbc::fiber::FiberTrap;
use crate::awbc::schema::{AwbcEntryId, AwbcProgram, AwbcTrapCode};
use crate::pure::RuntimeCallBackend;
#[cfg(test)]
use crate::root::RootStartup;
use crate::root::{
    RootCallableEvaluationError, RootRuntime, RootRuntimeError, RootStartupContract,
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
    pub(super) fn root_has_pending_reduction(&self) -> bool {
        self.root.as_ref().is_some_and(|root| {
            root.failure().is_none()
                && matches!(
                    root.active().phase,
                    crate::root::RootPhase::Initialized { .. }
                )
                && root.active().pending_event_count() != 0
        })
    }

    pub(super) fn prepare_root_reducer(&mut self, output: &mut RuntimeStepOutput) -> bool {
        let Some(root) = &self.root else {
            return true;
        };
        if root.failure().is_some() {
            return false;
        }
        if matches!(
            root.active().phase,
            crate::root::RootPhase::Initializing { .. }
        ) || root.active().reducer_active()
            || root.active().pending_event_count() == 0
            || matches!(
                output.root_transitions.last(),
                Some(crate::root::RootTransitionOutcome::Rejected { .. })
            )
        {
            return true;
        }
        if let Err(message) = self.stage_root_reducer() {
            let root = self.root.as_mut().expect("selected Root remains owned");
            match root.trap_pending_reduction(
                RootCallableEvaluationError::new(message.clone()),
                crate::program_types::RuntimeProgramTypes::Awbc(&self.program),
            ) {
                Ok(outcome) => output.root_transitions.push(outcome),
                Err(error) => output.diagnostics.push(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Internal,
                    error.to_string(),
                )),
            }
            self.fail_with_error(ProductStepError::Internal(message), output);
            return false;
        }
        true
    }

    fn stage_root_reducer(&mut self) -> Result<(), String> {
        let root = self.root.as_ref().expect("selected Root remains owned");
        let inputs = root
            .pending_reduction_inputs()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Root reducer has no pending event".to_owned())?;
        let entry = self
            .fiber
            .root
            .entry()
            .ok_or_else(|| "Root entry origin is missing".to_owned())?;
        let origin = crate::awbc::fiber::AwbcFiberRoot::RootReducer(entry);
        let function = origin
            .validate(&self.program)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Root reducer function is missing".to_owned())?;
        if !self.program.callable_executables.iter().any(|executable| {
            executable.role == *inputs.callable && executable.function == function
        }) {
            return Err("Root reducer origin differs from its original role".into());
        }
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            crate::task::RuntimeProgramOwner::Awbc(self.program_arc()),
            root.roles().command_policy.root_limits.schema,
        );
        admission
            .validate_metadata(&origin)
            .map_err(|error| error.to_string())?;
        for value in inputs.arguments {
            admission
                .validate_owned_borrowed(value.into())
                .map_err(|error| error.to_string())?;
        }
        let prepared = crate::awbc::fiber::validate_function_argument_value_refs(
            &self.program,
            function,
            &inputs.arguments,
        )
        .map_err(|error| error.to_string())?;
        let mut next_instance = self.next_fiber_instance;
        let instance = crate::runtime_id::RuntimeFiberInstanceId::from_allocated(
            next_instance
                .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
                .map_err(|error| error.to_string())?,
        );
        let (_, args) = self
            .root
            .as_mut()
            .expect("selected Root remains owned")
            .begin_reduction()
            .map_err(|error| error.to_string())?
            .expect("borrowed admission found the next event")
            .into_parts();
        let fiber = crate::awbc::fiber::FiberState::for_function_with_arguments_prepared(
            &self.program,
            origin,
            args,
            prepared,
            instance,
            self.runtime_generation.get(),
            self.fiber.budget.quantum.max(1),
        );
        self.next_fiber_instance = next_instance;
        self.child_fibers.push_front(super::ProductChildFiber {
            owner: super::ProductChildFiberOwner::Independent,
            fiber,
            runtime_generation: self.runtime_generation,
            pending_host_call: None,
        });
        Ok(())
    }

    pub(super) fn finish_root_reducer(
        &mut self,
        entry: AwbcEntryId,
        returned: Result<RuntimeValue, RootCallableEvaluationError>,
        output: &mut RuntimeStepOutput,
    ) -> Result<(), String> {
        let selected = self
            .program
            .entries
            .get(entry.index())
            .ok_or_else(|| "unknown Root entry".to_owned())?;
        let root = self
            .root
            .as_mut()
            .filter(|root| root.active().entry == selected.runtime_id)
            .ok_or_else(|| "Root reducer returned to a different entry".to_owned())?;
        let previous = root.active().pending_command_count();
        let outcome = root
            .complete_reduction(
                returned,
                crate::program_types::RuntimeProgramTypes::Awbc(&self.program),
            )
            .map_err(|error| error.to_string())?;
        output.root_transitions.push(outcome);
        output
            .root_commands
            .extend(root.committed_commands().skip(previous).cloned());
        if let Some(failure) = root.failure() {
            let message = failure.to_string();
            self.fail_with_error(ProductStepError::Internal(message), output);
        }
        Ok(())
    }

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
        let prepared = self.fiber.prepare_initial_flow(&program, entry, inputs)?;
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
        let staged = startup.stage_flow(|_, binding| {
            Ok::<_, std::convert::Infallible>(prepared.bind_state(binding.value))
        });
        let (root, frame, outcome) = match staged {
            Ok(prepared) => prepared,
            Err(rejected) => {
                let (_, impossible) = *rejected;
                match impossible {}
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
        self.fiber
            .root
            .active_entry_snapshot_identity(&self.program)
    }

    #[must_use]
    pub fn root_state_snapshot(
        &self,
    ) -> Result<Option<crate::root::RootStateSnapshotV1>, RootRuntimeError> {
        self.root
            .as_ref()
            .map(RootRuntime::snapshot_state)
            .transpose()
    }

    #[must_use]
    pub fn root_save_blockers(&self) -> Option<crate::root::RootSaveBlockers> {
        self.root.as_ref().map(RootRuntime::save_blockers)
    }

    pub(super) fn run_root_phase(
        &mut self,
        events: Vec<crate::root::RootEventInput>,
        output: &mut RuntimeStepOutput,
        _pure_backend: &mut impl RuntimeCallBackend,
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
        let result = if root.active().pending_command_count() != 0 {
            Err(RootRuntimeError::PendingCommandsAwaitingDispatch)
        } else {
            root.ingress(
                events,
                crate::program_types::RuntimeProgramTypes::Awbc(&self.program),
            )
        };
        match result {
            Ok(()) => true,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn reducer_fixture() -> (std::sync::Arc<AwbcProgram>, AwbcProductStepExecutor) {
        let mut raw = crate::awbc::stateful_root_fixture();
        let instruction = u32::try_from(raw.instructions.len()).unwrap();
        raw.instructions
            .push(crate::awbc::schema::AwbcInstruction::Nop);
        raw.blocks[2].instructions = crate::awbc::schema::AwbcTableRange::new(instruction, 1);
        let raw = std::sync::Arc::new(raw);
        let program = AwbcProductProgram::admit(
            std::sync::Arc::clone(&raw),
            crate::awbc::verify::AwbcVerifyBudget::default(),
            crate::awbc::verify::AwbcVerifyContext::default(),
            crate::plan::RuntimeTaskSourceArtifactLimits::default(),
            None,
        )
        .unwrap();
        let contract = startup_contract(&program, AwbcEntryId(0)).unwrap().unwrap();
        let types = crate::program_types::RuntimeProgramTypes::Awbc(&program);
        let state = types
            .try_record_value(
                contract.roles.state.semantic_identity,
                vec![],
                contract.roles.command_policy.root_limits.schema,
            )
            .unwrap();
        let startup = RootRuntime::prepare(contract, types)
            .unwrap()
            .complete_initialization(state, types)
            .unwrap();
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
        (raw, executor)
    }

    fn options(max_ops: usize) -> crate::step::RuntimeStepOptions {
        crate::step::RuntimeStepOptions {
            mode: crate::step::RuntimeStepMode::Drain,
            budget: crate::step::RuntimeStepBudget { max_ops },
            ..crate::step::RuntimeStepOptions::default()
        }
    }

    #[test]
    fn awbc_reducer_zero_budget_and_cold_restore_preserve_actual_instruction_frame() {
        let (raw, mut executor) = reducer_fixture();
        let roles = executor.root.as_ref().unwrap().roles();
        let event = crate::program_types::RuntimeProgramTypes::Awbc(&executor.program)
            .try_record_value(
                roles.event.semantic_identity,
                vec![],
                roles.command_policy.root_limits.schema,
            )
            .unwrap();
        let mut input = crate::step::RuntimeStepInput::default();
        input.root_events.push(crate::root::RootEventInput::new(
            crate::value::RuntimePayload(event),
        ));
        let zero = executor.step(input, options(0));
        assert_eq!(zero.stats.executed_ops, 0);
        assert!(zero.output.root_transitions.is_empty());
        assert!(!executor.root.as_ref().unwrap().active().reducer_active());
        let first = executor.step(crate::step::RuntimeStepInput::default(), options(1));
        assert_eq!(first.stats.executed_ops, 1);
        assert!(first.output.diagnostics.is_empty(), "{:?}", first.output);
        assert!(first.output.root_transitions.is_empty());
        assert!(executor.root.as_ref().unwrap().active().reducer_active());
        let image = executor.inert_rollback_image().unwrap();
        assert_eq!(image.child_fibers.len(), 1);
        assert!(matches!(
            image.child_fibers[0].fiber.root,
            crate::awbc::fiber::AwbcFiberRoot::RootReducer(AwbcEntryId(0))
        ));
        drop(executor);
        let mut restored = AwbcProductStepExecutor::for_entry_arc(raw, AwbcEntryId(0), 64).unwrap();
        restored = restored.restore_snapshot_owned(image.clone()).unwrap();
        assert_eq!(restored.inert_rollback_image().unwrap(), image);
        let done = restored.step(crate::step::RuntimeStepInput::default(), options(1));
        assert_eq!(done.stats.executed_ops, 1);
        assert!(matches!(&done.output.root_transitions[..],
            [crate::root::RootTransitionOutcome::Trapped { sequence, .. }] if sequence.get() == 0));
        assert!(!restored.root.as_ref().unwrap().active().reducer_active());
        assert!(restored.child_fibers.is_empty());
    }

    #[test]
    fn awbc_reducer_rollback_rejects_missing_and_duplicate_origins_without_mutation() {
        let (_, mut executor) = reducer_fixture();
        let roles = executor.root.as_ref().unwrap().roles();
        let event = crate::program_types::RuntimeProgramTypes::Awbc(&executor.program)
            .try_record_value(
                roles.event.semantic_identity,
                vec![],
                roles.command_policy.root_limits.schema,
            )
            .unwrap();
        let mut input = crate::step::RuntimeStepInput::default();
        input.root_events.push(crate::root::RootEventInput::new(
            crate::value::RuntimePayload(event),
        ));
        executor.step(input, options(1));
        let original = executor.inert_rollback_image().unwrap();
        let mut missing = original.clone();
        missing.child_fibers.clear();
        let (executor, _) = executor.restore_rollback_image(missing).unwrap_err();
        assert_eq!(executor.inert_rollback_image().unwrap(), original);
        let mut duplicate = original.clone();
        duplicate
            .child_fibers
            .push(duplicate.child_fibers[0].clone());
        let (executor, _) = executor.restore_rollback_image(duplicate).unwrap_err();
        assert_eq!(executor.inert_rollback_image().unwrap(), original);
    }

    #[test]
    fn awbc_root_product_rejection_preserves_the_complete_live_executor() {
        let (_, mut executor) = reducer_fixture();
        let roles = executor.root.as_ref().unwrap().roles();
        let event = crate::program_types::RuntimeProgramTypes::Awbc(&executor.program)
            .try_record_value(
                roles.event.semantic_identity,
                vec![],
                roles.command_policy.root_limits.schema,
            )
            .unwrap();
        let mut input = crate::step::RuntimeStepInput::default();
        input.root_events.push(crate::root::RootEventInput::new(
            crate::value::RuntimePayload(event),
        ));
        let first = executor.step(input, options(1));
        assert_eq!(first.stats.executed_ops, 1);
        let original = executor.inert_rollback_image().unwrap();
        assert_eq!(original.child_fibers.len(), 1);
        let mut invalid = original.clone();
        invalid.next_fiber_instance = crate::runtime_id::RuntimeIdCursor::initial();
        let (mut executor, _) = executor.restore_rollback_image(invalid).unwrap_err();
        assert_eq!(executor.inert_rollback_image().unwrap(), original);
        for instance in [
            original.fiber.instance,
            crate::runtime_id::RuntimeFiberInstanceId::from_allocated(
                original.next_fiber_instance.next().unwrap(),
            ),
        ] {
            let mut invalid = original.clone();
            invalid.child_fibers[0].fiber.instance = instance;
            let (recovered, _) = executor.restore_rollback_image(invalid).unwrap_err();
            executor = recovered;
            assert_eq!(executor.inert_rollback_image().unwrap(), original);
        }
        let mut invalid = original.clone();
        std::mem::swap(
            &mut invalid.fiber.instance,
            &mut invalid.child_fibers[0].fiber.instance,
        );
        let (recovered, error) = executor.restore_rollback_image(invalid).unwrap_err();
        executor = recovered;
        assert!(matches!(
            error,
            AwbcProductStepBuildError::RestoreSnapshot { ref message }
                if message.contains("main fiber")
        ));
        assert_eq!(executor.inert_rollback_image().unwrap(), original);
        executor = executor.restore_rollback_image(original.clone()).unwrap();
        assert_eq!(executor.inert_rollback_image().unwrap(), original);
        let mut invalid = original.clone();
        invalid.root = None;
        let (executor, error) = executor.restore_snapshot_owned(invalid).unwrap_err();
        assert!(matches!(
            error,
            AwbcProductStepBuildError::RootSnapshot { .. }
        ));
        assert_eq!(executor.inert_rollback_image().unwrap(), original);
        let executor = executor.restore_snapshot_owned(original.clone()).unwrap();
        assert_eq!(executor.inert_rollback_image().unwrap(), original);
    }

    #[test]
    fn awbc_failed_root_save_round_trip_preserves_the_complete_executor() {
        let (_, mut executor) = reducer_fixture();
        let roles = executor.root.as_ref().unwrap().roles();
        let event = crate::program_types::RuntimeProgramTypes::Awbc(&executor.program)
            .try_record_value(
                roles.event.semantic_identity,
                vec![],
                roles.command_policy.root_limits.schema,
            )
            .unwrap();
        let mut input = crate::step::RuntimeStepInput::default();
        input.root_events.push(crate::root::RootEventInput::new(
            crate::value::RuntimePayload(event),
        ));
        let trapped = executor.step(input, options(2));
        assert_eq!(trapped.stats.executed_ops, 2);
        assert!(matches!(
            trapped.output.root_transitions.as_slice(),
            [crate::root::RootTransitionOutcome::Trapped { .. }]
        ));
        assert!(executor.root.as_ref().unwrap().failure().is_some());
        let before = executor.snapshot().unwrap();
        let saved = super::super::AwbcProductExecutorSaveSnapshot::from_live(
            &before,
            executor.admitted_program(),
        )
        .unwrap();
        let bytes = serde_json::to_vec(&saved).unwrap();
        let decoded: super::super::AwbcProductExecutorSaveSnapshot =
            serde_json::from_slice(&bytes).unwrap();
        let image = decoded
            .into_live_for_program(executor.admitted_program())
            .unwrap();
        let executor = executor.restore_snapshot_owned(image).unwrap();
        assert_eq!(executor.snapshot().unwrap(), before);
    }

    #[test]
    fn awbc_snapshot_root_and_flow_share_one_value_allowance() {
        let (_, executor) = reducer_fixture();
        let snapshot = executor.snapshot().unwrap();
        let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
        for (max_nodes, accepted) in [(1, false), (2, true)] {
            let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                crate::entry::RuntimeSchemaLimits {
                    max_nodes,
                    ..crate::entry::RuntimeSchemaLimits::engine_default()
                },
            );
            assert_eq!(
                snapshot.admit_retained(&owner, &mut admission).is_ok(),
                accepted
            );
        }
    }

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
            .inert_rollback_image(&crate::task::RuntimeProgramOwner::Awbc(
                program.program_arc(),
            ))
            .unwrap();
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
        let before = executor.inert_rollback_image().unwrap();
        let mut invalid = before.clone();
        invalid.root = Some(pending);
        let (executor, error) = executor.restore_snapshot_owned(invalid).unwrap_err();
        assert!(matches!(
            error,
            AwbcProductStepBuildError::RootSnapshot { .. }
        ));
        assert_eq!(executor.inert_rollback_image().unwrap(), before);
    }
}
