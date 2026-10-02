//! Owned program activation through the selected plan's FunctionSite ABI.

use std::sync::Arc;

use arcweft_id::runtime_program::RuntimePureProgramId;

use super::{Engine, FlowFiberStatus, FunctionCallFrame, FunctionReturnContinuation};
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

impl Engine {
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
        if let Some((_, value)) = &self.program_result {
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
            let error = |reason: &str| RuntimeEvalError::UnsupportedPure {
                name: program.to_string(),
                reason: reason.to_owned(),
            };
            let mut bindings = plan
                .pure_programs()
                .iter()
                .filter(|binding| binding.program() == program);
            let binding = bindings
                .next()
                .ok_or_else(|| error("program is absent from the selected plan"))?;
            if bindings.next().is_some() {
                return Err(error("program binding is ambiguous"));
            }
            let site = binding.site();
            let declaration = plan
                .function_sites()
                .get(site)
                .ok_or_else(|| error("program function site is absent"))?;
            if inputs.len() != declaration.inputs().len() {
                return Err(RuntimeEvalError::TooManyPureArgs {
                    helper: program.to_string(),
                    max: declaration.inputs().len(),
                    found: inputs.len(),
                });
            }
            let captures = declaration
                .inputs()
                .iter()
                .zip(&inputs)
                .filter_map(|(input, value)| {
                    matches!(input.source(), RuntimeFunctionInputSource::Capture { .. })
                        .then_some(value)
                })
                .collect::<Vec<_>>();
            let parameters = declaration
                .inputs()
                .iter()
                .zip(&inputs)
                .filter_map(|(input, value)| {
                    matches!(input.source(), RuntimeFunctionInputSource::Parameter { .. })
                        .then_some(value)
                })
                .collect::<Vec<_>>();
            plan.validate_function_site_input_refs(site, &captures, &parameters)?;
            for (input, value) in declaration.inputs().iter().zip(&inputs) {
                if !inspect_runtime_pattern_owned(&plan, input.pattern(), value)? {
                    return Err(RuntimeEvalError::PatternMismatch(format!(
                        "program {program} input {:?}",
                        input.source()
                    )));
                }
            }
            Ok(site)
        })();
        let site = match prepared {
            Ok(site) => site,
            Err(reason) => return Err(RuntimeProgramInvocationError { reason, inputs }),
        };
        let mut captures = Vec::new();
        let mut parameters = Vec::new();
        for (input, value) in plan
            .function_sites()
            .get(site)
            .expect("validated program site")
            .inputs()
            .iter()
            .zip(inputs)
        {
            match input.source() {
                RuntimeFunctionInputSource::Capture { .. } => captures.push(value),
                RuntimeFunctionInputSource::Parameter { .. } => parameters.push(value),
            }
        }
        let mut engine = Self::new_with_shared_plan(plan, crate::task::GenerationId::new(0));
        engine.main_started = true;
        engine.fiber.status = FlowFiberStatus::Running;
        let mut output = crate::step::RuntimeStepOutput::default();
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        engine
            .start_function_site_call(
                captures,
                parameters,
                FunctionCallFrame::new(site, None, FunctionReturnContinuation::Program { program }),
                &mut output,
                &mut backend,
            )
            .expect("complete borrowed preflight admits the same owned program inputs");
        debug_assert_eq!(output, crate::step::RuntimeStepOutput::default());
        Ok(engine)
    }
}
