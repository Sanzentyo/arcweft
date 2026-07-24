use crate::awbc::fiber::{FiberState, FiberStateError};
use crate::awbc::schema::{
    AwbcBackpressurePolicy, AwbcEntryId, AwbcFunctionId, AwbcOverflowPolicy, AwbcPrivacyPolicy,
    AwbcProgram, AwbcReplayPolicy, AwbcSourcePlanId, AwbcStreamPlanId,
};
use crate::awbc::vm::{VmError, VmExit, VmHost, VmStepOptions, step_with_host};
use crate::pure::{RuntimeCallBackend, RuntimeCompactPureHelper};
use crate::source::{
    BackpressurePolicy, OverflowPolicy, PrivacyPolicy, ReplayPolicy, SourceId, SourcePolicy,
};
use crate::step::{
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeStepInput, RuntimeStepOutput,
    input_event_text_payload, input_event_trigger_name,
};
use crate::stream::StreamRuntimeId;
use crate::value::{RuntimeCallTarget, RuntimeValue};

pub(super) struct ProductVmHost<'a, B> {
    pub(super) backend: &'a mut B,
    pub(super) fallback_stats: &'a mut crate::step::RuntimePureCallStats,
}

impl<B: RuntimeCallBackend> VmHost for ProductVmHost<'_, B> {
    fn call_intrinsic(
        &mut self,
        program: &AwbcProgram,
        intrinsic: crate::awbc::schema::AwbcIntrinsicId,
        args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, VmError> {
        let record = program
            .intrinsics
            .get(intrinsic.index())
            .ok_or(VmError::MissingIntrinsic(intrinsic))?;
        let name = program
            .strings
            .get(record.public_id.index())
            .ok_or(VmError::MissingString(record.public_id))?;
        let target = RuntimeCallTarget::from_label(name.clone());
        Ok(Some(crate::engine::evaluate_runtime_call(
            &target,
            args,
            self.backend,
        )))
    }

    fn call_pure_helper(
        &mut self,
        program: &AwbcProgram,
        helper: crate::awbc::schema::AwbcPureHelperId,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, VmError> {
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
            return result.map_err(|error| VmError::Runtime(error.to_string()));
        }
        self.fallback_stats.pure_calls = self.fallback_stats.pure_calls.saturating_add(1);
        self.fallback_stats.vm_calls = self.fallback_stats.vm_calls.saturating_add(1);
        self.fallback_stats.fallbacks = self.fallback_stats.fallbacks.saturating_add(1);
        run_function_with_host(program, record.function, args, self)
    }
}

pub(super) fn run_function(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    args: &[RuntimeValue],
    backend: &mut impl RuntimeCallBackend,
    fallback_stats: &mut crate::step::RuntimePureCallStats,
) -> Result<RuntimeValue, VmError> {
    let mut host = ProductVmHost {
        backend,
        fallback_stats,
    };
    run_function_with_host(program, function, args, &mut host)
}

fn run_function_with_host(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    args: &[RuntimeValue],
    host: &mut impl VmHost,
) -> Result<RuntimeValue, VmError> {
    let mut fiber = FiberState::for_function(program, AwbcEntryId(0), function, 0, 1_000_000)?;
    fiber
        .active_frame_mut()?
        .bind_positional_arguments(program, args)?;
    loop {
        let output = step_with_host(
            program,
            &mut fiber,
            VmStepOptions {
                max_instructions: 1024,
            },
            host,
        )?;
        match output.exit {
            VmExit::Running => {}
            VmExit::Returned(value) => return Ok(value.unwrap_or(RuntimeValue::Unit)),
            VmExit::Cancelled => {
                return Err(VmError::Runtime(
                    "pure helper execution was cancelled".to_owned(),
                ));
            }
            VmExit::Trapped(trap) => {
                return Err(VmError::Runtime(trap.message.unwrap_or_else(|| {
                    format!("AWBC pure helper trap {:?}", trap.code)
                })));
            }
            VmExit::Suspended(reason) => {
                return Err(VmError::Runtime(format!(
                    "pure helper suspended at {reason:?}"
                )));
            }
            VmExit::BudgetYield(_) => {
                return Err(VmError::Runtime(
                    "pure helper exhausted compact VM budget".to_owned(),
                ));
            }
        }
    }
}

impl crate::awbc::schema::AwbcSourcePolicy {
    pub(super) fn runtime_policy(&self) -> SourcePolicy {
        SourcePolicy {
            backpressure: match self.backpressure {
                AwbcBackpressurePolicy::LatestOnly => BackpressurePolicy::LatestOnly,
                AwbcBackpressurePolicy::BoundedQueue { capacity, overflow } => {
                    BackpressurePolicy::BoundedQueue {
                        capacity: capacity as usize,
                        on_overflow: match overflow {
                            AwbcOverflowPolicy::DropOldest => OverflowPolicy::DropOldest,
                            AwbcOverflowPolicy::DropNewest => OverflowPolicy::DropNewest,
                            AwbcOverflowPolicy::Error => OverflowPolicy::Error,
                            AwbcOverflowPolicy::Coalesce => OverflowPolicy::Coalesce,
                        },
                    }
                }
                AwbcBackpressurePolicy::BlockingNotAllowed => {
                    BackpressurePolicy::BlockingNotAllowed
                }
            },
            replay: match self.replay {
                AwbcReplayPolicy::Full => ReplayPolicy::Full,
                AwbcReplayPolicy::HashOnly => ReplayPolicy::HashOnly,
                AwbcReplayPolicy::Summary => ReplayPolicy::Summary,
                AwbcReplayPolicy::EventOnly => ReplayPolicy::EventOnly,
                AwbcReplayPolicy::None => ReplayPolicy::None,
            },
            privacy: match self.privacy {
                AwbcPrivacyPolicy::Transient => PrivacyPolicy::Transient,
                AwbcPrivacyPolicy::Redacted => PrivacyPolicy::Redacted,
                AwbcPrivacyPolicy::Recordable => PrivacyPolicy::Recordable,
                AwbcPrivacyPolicy::Private => PrivacyPolicy::Private,
            },
            max_queue: self.max_queue as usize,
        }
    }
}

pub(super) fn source_id_for(program: &AwbcProgram, source: AwbcSourcePlanId) -> SourceId {
    program
        .source_plans
        .get(source.index())
        .and_then(|plan| program.strings.get(plan.public_id.index()))
        .cloned()
        .map_or_else(|| SourceId(format!("awbc.source.{}", source.0)), SourceId)
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

pub(super) fn entry_argument_diagnostic(error: &FiberStateError) -> RuntimeDiagnostic {
    let category = match error {
        FiberStateError::EntryArgumentType { .. } => RuntimeDiagnosticCategory::Type,
        FiberStateError::EntryArgumentCount { .. }
        | FiberStateError::DuplicateEntryArgument { .. }
        | FiberStateError::UnknownEntryArgument { .. } => RuntimeDiagnosticCategory::Input,
        _ => RuntimeDiagnosticCategory::Internal,
    };
    RuntimeDiagnostic::categorized(category, error.to_string())
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
        || !output.requests.source_close.is_empty()
        || !output.requests.ensure_content.is_empty()
        || !output.requests.host_calls.is_empty()
}

pub(super) fn has_visible_output(output: &RuntimeStepOutput) -> bool {
    !output.flow_events.is_empty()
        || !output.effects.line.is_empty()
        || !output.effects.source_events.is_empty()
        || !output.effects.stream_events.is_empty()
}
