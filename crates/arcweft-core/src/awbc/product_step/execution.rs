use crate::awbc::fiber::FiberState;
use crate::awbc::schema::{AwbcEntryId, AwbcFunctionId, AwbcProgram, AwbcStreamPlanId};
use crate::awbc::vm::{
    VmError, VmExecutionContext, VmExit, VmHost, VmNestedCallExit, VmStepOptions,
    step_with_host_context,
};
use crate::pure::{RuntimeCallBackend, RuntimeCompactPureHelper, RuntimeExternalCallContext};
use crate::step::{
    RuntimeStepInput, RuntimeStepOutput, input_event_text_payload, input_event_trigger_name,
};
use crate::value::RuntimeValue;
use crate::{entry::RuntimeSchemaLimits, stream::StreamRuntimeId, task::RuntimeProgramOwner};
use std::sync::Arc;

pub(super) struct ProductVmHost<'a, B> {
    pub(super) backend: &'a mut B,
    pub(super) fallback_stats: &'a mut crate::step::RuntimePureCallStats,
    pub(super) program_owner: RuntimeProgramOwner,
}

impl<B: RuntimeCallBackend> VmHost for ProductVmHost<'_, B> {
    fn produce_character_dialogue(
        &mut self,
        owner: &RuntimeProgramOwner,
        operation: arcweft_interaction_model::dialogue::CharacterDialogueOperation,
        target: RuntimeValue,
        fields: &[arcweft_interaction_model::dialogue::CharacterDialoguePatchField<RuntimeValue>],
        result_type: crate::pattern::RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, VmError> {
        if !owner.same_program(&self.program_owner) {
            return Err(VmError::Runtime(
                "CharacterDialogue producer received a foreign AWBC program".to_owned(),
            ));
        }
        self.backend
            .produce_character_dialogue(owner, operation, target, fields, result_type)
            .map_err(|error| VmError::Runtime(error.to_string()))
    }

    fn call_intrinsic(
        &mut self,
        program: &AwbcProgram,
        intrinsic: crate::awbc::schema::AwbcIntrinsicId,
        args: Vec<RuntimeValue>,
    ) -> Result<Option<RuntimeValue>, VmError> {
        let record = program
            .intrinsics
            .get(intrinsic.index())
            .ok_or(VmError::MissingIntrinsic(intrinsic))?;
        let external_context = if record.identity.as_intrinsic().is_some() {
            RuntimeExternalCallContext::unbound()
        } else {
            let signature = program
                .signatures
                .get(record.signature.index())
                .ok_or_else(|| {
                    VmError::Runtime(format!(
                        "AWBC intrinsic {} references missing signature {}",
                        intrinsic.0, record.signature.0
                    ))
                })?;
            if signature.params.len() != args.len() {
                return Err(VmError::FunctionArgumentCount {
                    expected: signature.params.len(),
                    actual: args.len(),
                });
            }
            let argument_types = signature
                .params
                .iter()
                .copied()
                .map(|ty| semantic_type_for_awbc(program, ty))
                .collect::<Result<Vec<_>, _>>()?;
            let result_type = signature.result.ok_or_else(|| {
                VmError::Runtime(format!(
                    "AWBC external intrinsic {} has no result type for its runtime call context",
                    record.identity.as_label()
                ))
            })?;
            let result_type = semantic_type_for_awbc(program, result_type)?;
            RuntimeExternalCallContext::for_program(
                self.program_owner.clone(),
                argument_types,
                result_type,
                RuntimeSchemaLimits::engine_default(),
            )
            .map_err(|error| {
                VmError::Runtime(format!(
                    "AWBC external intrinsic {} has an invalid program type context: {error}",
                    record.identity.as_label()
                ))
            })?
        };
        crate::engine::evaluate_runtime_call(
            &record.identity,
            args,
            &external_context,
            self.backend,
        )
        .map(Some)
        .map_err(VmError::Evaluation)
    }

    fn try_call_pure_helper(
        &mut self,
        program: &AwbcProgram,
        helper: crate::awbc::schema::AwbcPureHelperId,
        args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, VmError> {
        let record = program
            .pure_helpers
            .get(helper.index())
            .cloned()
            .ok_or_else(|| VmError::Runtime(format!("missing AWBC pure helper {}", helper.0)))?;
        let name = program
            .strings
            .get(record.public_id.index())
            .cloned()
            .ok_or(VmError::MissingString(record.public_id))?;
        let descriptor = RuntimeCompactPureHelper {
            id: helper.0,
            name,
            arity: args.len(),
            scalar_eval_supported: record.scalar_eval_supported,
        };
        if let Some(result) = self.backend.call_compact_values(&descriptor, args) {
            return result.map(Some).map_err(VmError::Evaluation);
        }
        self.fallback_stats.pure_calls = self.fallback_stats.pure_calls.saturating_add(1);
        self.fallback_stats.vm_calls = self.fallback_stats.vm_calls.saturating_add(1);
        self.fallback_stats.fallbacks = self.fallback_stats.fallbacks.saturating_add(1);
        Ok(None)
    }

    fn record_context_callback_vm_call(&mut self) {
        self.fallback_stats.pure_calls = self.fallback_stats.pure_calls.saturating_add(1);
        self.fallback_stats.vm_calls = self.fallback_stats.vm_calls.saturating_add(1);
        self.fallback_stats.fallbacks = self.fallback_stats.fallbacks.saturating_add(1);
    }
}

pub(super) fn run_function(
    program: &Arc<AwbcProgram>,
    function: AwbcFunctionId,
    args: Vec<RuntimeValue>,
    backend: &mut impl RuntimeCallBackend,
    fallback_stats: &mut crate::step::RuntimePureCallStats,
) -> Result<RuntimeValue, VmError> {
    let context = context_for_program(program)?;
    let mut host = ProductVmHost {
        backend,
        fallback_stats,
        program_owner: RuntimeProgramOwner::Awbc(Arc::clone(program)),
    };
    run_function_with_host(program, function, args, context, &mut host)
}

fn run_function_with_host(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    args: Vec<RuntimeValue>,
    context: VmExecutionContext,
    host: &mut impl VmHost,
) -> Result<RuntimeValue, VmError> {
    let mut fiber = FiberState::for_function(program, AwbcEntryId(0), function, 0, 1_000_000)?;
    fiber
        .active_frame_mut()?
        .bind_positional_arguments_owned(program, args)?;
    loop {
        let output = step_with_host_context(
            program,
            &mut fiber,
            VmStepOptions {
                max_instructions: 1024,
            },
            &context,
            host,
        )?;
        match output.exit {
            VmExit::Running => {}
            VmExit::Returned(value) => return Ok(value.unwrap_or(RuntimeValue::Unit)),
            VmExit::DialogueResultSelected(value) => {
                return Err(VmError::NestedCallExit(Box::new(
                    VmNestedCallExit::DialogueResultSelected(value),
                )));
            }
            VmExit::Cancelled => {
                return Err(VmError::NestedCallExit(Box::new(
                    VmNestedCallExit::Cancelled,
                )));
            }
            VmExit::Trapped(trap) => {
                return Err(VmError::NestedCallExit(Box::new(
                    VmNestedCallExit::Trapped(trap),
                )));
            }
            VmExit::Suspended(reason) => {
                return Err(VmError::NestedCallExit(Box::new(
                    VmNestedCallExit::Suspended(reason),
                )));
            }
            VmExit::BudgetYield(point) => {
                return Err(VmError::NestedCallExit(Box::new(
                    VmNestedCallExit::BudgetYield(point),
                )));
            }
        }
    }
}

fn context_for_program(program: &Arc<AwbcProgram>) -> Result<VmExecutionContext, VmError> {
    let encoded = program
        .encode_canonical()
        .map_err(|error| VmError::Runtime(error.to_string()))?;
    let artifact = crate::effect::RuntimeArtifactFingerprint::try_from_bytes(
        *blake3::hash(&encoded).as_bytes(),
    )
    .map_err(|error| VmError::Runtime(error.to_string()))?;
    Ok(VmExecutionContext::for_program(
        artifact,
        Arc::clone(program),
    ))
}

fn semantic_type_for_awbc(
    program: &AwbcProgram,
    ty: crate::awbc::schema::AwbcTypeId,
) -> Result<crate::pattern::RuntimeSemanticTypeId, VmError> {
    program
        .runtime_types
        .get(ty.index())
        .map(|runtime_type| runtime_type.semantic_identity())
        .ok_or(VmError::MissingType(ty))
}

#[cfg(test)]
mod same_fiber_tests {
    use super::*;

    #[test]
    fn unavailable_compact_backend_returns_to_the_vm_with_one_fallback_charge() {
        let mut program = AwbcProgram::default();
        program.strings.push("helper".to_owned());
        program
            .pure_helpers
            .push(crate::awbc::schema::AwbcPureHelper {
                public_id: crate::awbc::schema::AwbcStringId(0),
                signature: crate::awbc::schema::AwbcSignatureId(0),
                function: AwbcFunctionId(0),
                scalar_eval_supported: false,
                origin: crate::awbc::schema::AwbcPureHelperOrigin::EngineOwned,
            });
        let program = Arc::new(program);
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        let mut fallback_stats = crate::step::RuntimePureCallStats::default();
        let mut host = ProductVmHost {
            backend: &mut backend,
            fallback_stats: &mut fallback_stats,
            program_owner: RuntimeProgramOwner::Awbc(Arc::clone(&program)),
        };
        assert_eq!(
            host.try_call_pure_helper(&program, crate::awbc::schema::AwbcPureHelperId(0), &[])
                .expect("backend absence selects the VM frame"),
            None
        );
        assert_eq!(fallback_stats.pure_calls, 1);
        assert_eq!(fallback_stats.vm_calls, 1);
        assert_eq!(fallback_stats.fallbacks, 1);
    }
}

pub(super) fn stream_id_for(program: &AwbcProgram, stream: AwbcStreamPlanId) -> StreamRuntimeId {
    program
        .stream_plans
        .get(stream.index())
        .and_then(|plan| program.strings.get(plan.public_id.index()))
        .and_then(|label| StreamRuntimeId::from_runtime_target_value(label).ok())
        .unwrap_or_else(|| {
            StreamRuntimeId::canonical(&format!("awbc_stream_{}", stream.0))
                .expect("generated AWBC stream ID is canonical")
        })
}

pub(super) fn input_choice_selection(input: &RuntimeStepInput) -> Option<(Option<&str>, &str)> {
    input.input_events.iter().find_map(|event| {
        let trigger = input_event_trigger_name(event)?;
        let selection = input_event_text_payload(event)?;
        match trigger {
            "choice" | "select" => Some((None, selection)),
            trigger => trigger
                .strip_prefix("choice:")
                .or_else(|| trigger.strip_prefix("select:"))
                .map(|choice| (Some(choice), selection)),
        }
    })
}

pub(super) fn has_host_requests(output: &RuntimeStepOutput) -> bool {
    !output.requests.tasks.is_empty()
        || !output.requests.audio.is_empty()
        || !output.requests.cancel_scopes.is_empty()
        || !output.requests.ensure_content.is_empty()
        || !output.requests.host_calls.is_empty()
}

pub(super) fn has_visible_output(output: &RuntimeStepOutput) -> bool {
    !output.flow_events.is_empty()
        || !output.effects.line.is_empty()
        || !output.effects.stream_events.is_empty()
}
