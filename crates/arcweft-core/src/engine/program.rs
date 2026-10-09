//! Owned program activation through the selected plan's FunctionSite ABI.

use std::sync::Arc;

use arcweft_id::runtime_program::RuntimePureProgramId;

use super::{
    Engine, FlowFiberStatus, FunctionCallFrame, FunctionReturnContinuation, NativeInvocationRoot,
};
use crate::pattern::inspect_runtime_pattern_owned;
use crate::plan::{RuntimeFunctionInputSource, RuntimePlan};
use crate::value::{RuntimeEvalError, RuntimeValue};

/// Rejected activation retains every input; no execution or backend call occurs.
#[derive(Debug)]
pub struct RuntimeProgramInvocationError {
    reason: RuntimeEvalError,
    inputs: Vec<RuntimeValue>,
}

impl RuntimeProgramInvocationError {
    pub fn into_parts(self) -> (RuntimeEvalError, Vec<RuntimeValue>) {
        (self.reason, self.inputs)
    }
}

fn prepare_program_inputs(
    plan: &RuntimePlan,
    program: RuntimePureProgramId,
    inputs: &[&RuntimeValue],
) -> Result<crate::runtime_id::RuntimeFunctionSiteId, RuntimeEvalError> {
    let error = |reason: &str| RuntimeEvalError::UnsupportedPure {
        name: program.to_string(),
        reason: reason.to_owned(),
    };
    let binding = plan.resolve_pure_program(program).map_err(|reason| {
        error(match reason {
            crate::plan::RuntimePureProgramLookupError::Missing => {
                "program is absent from the selected plan"
            }
            crate::plan::RuntimePureProgramLookupError::Ambiguous => "program binding is ambiguous",
        })
    })?;
    let site = binding.site();
    prepare_function_inputs(plan, site, inputs, &program.to_string())
}

fn prepare_function_inputs(
    plan: &RuntimePlan,
    site: crate::runtime_id::RuntimeFunctionSiteId,
    inputs: &[&RuntimeValue],
    label: &str,
) -> Result<crate::runtime_id::RuntimeFunctionSiteId, RuntimeEvalError> {
    let error = |reason: &str| RuntimeEvalError::UnsupportedPure {
        name: label.to_owned(),
        reason: reason.to_owned(),
    };
    let declaration = plan
        .function_sites()
        .get(site)
        .ok_or_else(|| error("program function site is absent"))?;
    if inputs.len() != declaration.inputs().len() {
        return Err(RuntimeEvalError::TooManyPureArgs {
            helper: label.to_owned(),
            max: declaration.inputs().len(),
            found: inputs.len(),
        });
    }
    let captures = declaration
        .inputs()
        .iter()
        .zip(inputs)
        .filter_map(|(input, value)| {
            matches!(
                input.source(),
                RuntimeFunctionInputSource::Capture { .. }
                    | RuntimeFunctionInputSource::CapturedParameter { .. }
            )
            .then_some(*value)
        })
        .collect::<Vec<_>>();
    let parameters = declaration
        .inputs()
        .iter()
        .zip(inputs)
        .filter_map(|(input, value)| {
            matches!(input.source(), RuntimeFunctionInputSource::Parameter { .. }).then_some(*value)
        })
        .collect::<Vec<_>>();
    let admission = plan.validate_function_site_input_refs(site, &captures, &parameters)?;
    for (input, value) in declaration.inputs().iter().zip(inputs) {
        if !inspect_runtime_pattern_owned(
            plan,
            input.pattern(),
            value,
            admission.type_instantiation.as_deref(),
        )? {
            return Err(RuntimeEvalError::PatternMismatch(format!(
                "function {site} input {:?}",
                input.source()
            )));
        }
    }
    Ok(site)
}

impl Engine {
    /// Internal deterministic evaluation enters the exact site on its retained
    /// Arc. Inputs are owned packets; all full formals, patterns and detached
    /// custody are checked before transfer into the normal Engine.
    pub(crate) fn for_function_invocation(
        function: crate::pure::RuntimePureFunctionRef<'_>,
        inputs: Vec<RuntimeValue>,
    ) -> Result<Self, RuntimeEvalError> {
        let crate::pure::RuntimePureFunctionId::Function(site) = function.id() else {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: function.name.to_owned(),
                reason: "a synthetic recipe has no structured function root".to_owned(),
            });
        };
        for (position, value) in inputs.iter().enumerate() {
            value.validate_detached_custody().map_err(|reason| {
                RuntimeEvalError::UnsupportedPure {
                    name: function.name.to_owned(),
                    reason: format!(
                        "function input {position} has invalid detached custody: {reason}"
                    ),
                }
            })?;
        }
        prepare_function_inputs(
            function.plan(),
            site,
            &inputs.iter().collect::<Vec<_>>(),
            function.name,
        )?;
        let mut engine = Self::new_with_shared_plan(
            Arc::clone(function.plan()),
            crate::task::GenerationId::new(0),
        );
        engine.activate_function_prepared(NativeInvocationRoot::Function(site), site, inputs);
        Ok(engine)
    }

    pub(crate) fn take_function_result(
        &mut self,
        site: crate::runtime_id::RuntimeFunctionSiteId,
    ) -> Result<Option<RuntimeValue>, RuntimeEvalError> {
        let Some((target, value)) = &self.invocation_result else {
            return Ok(None);
        };
        if *target != NativeInvocationRoot::Function(site) {
            return Err(RuntimeEvalError::UnsupportedPure {
                name: "structured.function".to_owned(),
                reason: "function result belongs to a different invocation root".to_owned(),
            });
        }
        value.validate_detached_custody()?;
        Ok(self.invocation_result.take().map(|(_, value)| value))
    }

    pub(super) fn main_fiber_line_handle_owners(
        &self,
    ) -> Result<
        std::collections::BTreeMap<
            crate::runtime_id::RuntimeLineHandleToken,
            crate::value::ownership::RuntimeOwnedSlotId,
        >,
        crate::line_task::LineRuntimeError,
    > {
        let mut owners = super::flow_fiber_line_handle_owners(&self.fiber)?;
        if let Some((_, value)) = &self.invocation_result {
            let owner = crate::value::ownership::RuntimeOwnedSlotId::ProgramResult {
                execution: self.fiber.execution,
                fiber: self.fiber.persistent_id,
            };
            for handle in value
                .affine_line_handles()
                .map_err(|_| crate::line_task::LineRuntimeError::InvalidHandlePayload)?
            {
                if owners.insert(handle.token().clone(), owner).is_some() {
                    return Err(crate::line_task::LineRuntimeError::DuplicateHandleOccurrence);
                }
            }
        }
        Ok(owners)
    }

    /// Activates the exact program on its retained plan. Inputs transfer only
    /// after the complete ABI and all binding patterns have been checked.
    /// Stepping uses the normal engine budget and return continuation.
    pub fn for_program_invocation(
        plan: Arc<RuntimePlan>,
        program: RuntimePureProgramId,
        inputs: Vec<RuntimeValue>,
    ) -> Result<Self, RuntimeProgramInvocationError> {
        let prepared = (|| {
            for (position, value) in inputs.iter().enumerate() {
                value.validate_detached_custody().map_err(|source| {
                    RuntimeEvalError::ProgramInputCustody {
                        program,
                        position,
                        source,
                    }
                })?;
            }
            prepare_program_inputs(&plan, program, &inputs.iter().collect::<Vec<_>>())
        })();
        let site = match prepared {
            Ok(site) => site,
            Err(reason) => return Err(RuntimeProgramInvocationError { reason, inputs }),
        };
        let mut engine = Self::new_with_shared_plan(plan, crate::task::GenerationId::new(0));
        engine.activate_program_prepared(program, site, inputs);
        Ok(engine)
    }

    fn activate_program_prepared(
        &mut self,
        program: RuntimePureProgramId,
        site: crate::runtime_id::RuntimeFunctionSiteId,
        inputs: Vec<RuntimeValue>,
    ) {
        self.activate_function_prepared(NativeInvocationRoot::Program(program), site, inputs);
    }

    fn activate_function_prepared(
        &mut self,
        root: NativeInvocationRoot,
        site: crate::runtime_id::RuntimeFunctionSiteId,
        inputs: Vec<RuntimeValue>,
    ) {
        let mut captures = Vec::new();
        let mut parameters = Vec::new();
        for (input, value) in self
            .plan
            .function_sites()
            .get(site)
            .expect("validated program site")
            .inputs()
            .iter()
            .zip(inputs)
        {
            match input.source() {
                RuntimeFunctionInputSource::Capture { .. }
                | RuntimeFunctionInputSource::CapturedParameter { .. } => captures.push(value),
                RuntimeFunctionInputSource::Parameter { .. } => parameters.push(value),
            }
        }
        self.main_started = true;
        self.fiber.status = FlowFiberStatus::Running;
        let mut output = crate::step::RuntimeStepOutput::default();
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        self.start_function_site_call(
            captures,
            parameters,
            FunctionCallFrame::new(
                site,
                None,
                match root {
                    NativeInvocationRoot::Program(program) => {
                        FunctionReturnContinuation::Program { program }
                    }
                    NativeInvocationRoot::Function(site) => {
                        FunctionReturnContinuation::Function { site }
                    }
                },
            ),
            &mut output,
            &mut backend,
        )
        .expect("complete borrowed preflight admits the same owned program inputs");
        debug_assert_eq!(output, crate::step::RuntimeStepOutput::default());
    }

    /// Consumes a completed program executor into another admitted program.
    /// Its ledger, Need producers, publications and identity remain owned by
    /// this same executor. Detached inputs never introduce another ledger.
    pub fn continue_program(
        mut self,
        program: RuntimePureProgramId,
        inputs: Vec<crate::program_invocation::RuntimeProgramInput>,
    ) -> Result<Self, crate::program_invocation::RuntimeProgramContinuationError<Self>> {
        use crate::program_invocation::{
            RuntimeProgramContinuationError, RuntimeProgramContinuationFailure,
            RuntimeProgramInputRollback, input_refs, into_values,
        };
        let prepared = (|| {
            if !matches!(self.fiber.status, FlowFiberStatus::Done(_))
                || !self.child_fibers.is_empty()
                || self.root.is_some()
                || !self.fiber.pending_ops.is_empty()
                || !self.fiber.control_stack.is_empty()
                || self.fiber.await_observer.is_some()
                || !self.fiber.root_cleanups.is_empty()
            {
                return Err(RuntimeProgramContinuationFailure::NotCompleted);
            }
            let (root, result) = self
                .invocation_result
                .as_ref()
                .ok_or(RuntimeProgramContinuationFailure::NotCompleted)?;
            if !matches!(root, NativeInvocationRoot::Program(_)) {
                return Err(RuntimeProgramContinuationFailure::NotCompleted);
            }
            let refs = input_refs(&inputs, result)?;
            let site = prepare_program_inputs(&self.plan, program, &refs)?;
            let before = self.main_fiber_line_handle_owners()?;
            self.dialogue_activations
                .inspect_parent_fiber_reconciliation(
                    self.fiber.execution,
                    &before,
                    &before,
                    &Default::default(),
                )?;
            let image = self
                .inert_rollback_image()
                .map_err(|message| RuntimeProgramContinuationFailure::Snapshot { message })?;
            let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan));
            let input_image = RuntimeProgramInputRollback::capture(&inputs, &owner)?;
            Ok::<_, RuntimeProgramContinuationFailure>((site, before, image, owner, input_image))
        })();
        let (site, before, image, owner, input_image) = match prepared {
            Ok(prepared) => prepared,
            Err(reason) => {
                return Err(RuntimeProgramContinuationError {
                    reason,
                    executor: Box::new(self),
                    inputs,
                });
            }
        };
        let custody = match std::mem::take(&mut self.dialogue_activations).into_published() {
            Ok(custody) => custody,
            Err((store, reason)) => {
                self.dialogue_activations = store;
                return Err(RuntimeProgramContinuationError {
                    reason: reason.into(),
                    executor: Box::new(self),
                    inputs,
                });
            }
        };
        self.dialogue_activations =
            super::dialogue::DialogueActivationStore::from_published(custody);
        let (_, result) = self
            .invocation_result
            .take()
            .expect("borrowed continuation preflight retains its result");
        self.activate_program_prepared(program, site, into_values(inputs, result));
        let committed = (|| {
            let after = self.main_fiber_line_handle_owners()?;
            let receipt = self.dialogue_activations.reconcile_parent_fiber(
                self.fiber.execution,
                &before,
                &after,
                &Default::default(),
            )?;
            assert!(
                receipt.into_commands().is_empty(),
                "program activation transfers ownership without dropping resources"
            );
            Ok::<_, crate::line_task::LineRuntimeError>(())
        })();
        if let Err(reason) = committed {
            drop(self);
            let executor = Self::from_rollback_image(image).expect(
                "the original completed executor restores after the candidate owner is dropped",
            );
            return Err(RuntimeProgramContinuationError {
                reason: reason.into(),
                executor: Box::new(executor),
                inputs: RuntimeProgramInputRollback::restore(input_image, &owner),
            });
        }
        Ok(self)
    }
}
