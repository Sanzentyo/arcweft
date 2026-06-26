use super::{AwbcProductStepExecutor, ProductStepError};
use crate::awbc::schema::{
    AwbcContentUnitId, AwbcEffectKind, AwbcEffectPlanId, AwbcProgram, AwbcResourceResidency,
    AwbcStringId, AwbcTaskPlanId, AwbcTaskPolicy,
};
use crate::awbc::vm::constant_value;
use crate::effect::{
    LineEffectRequest, RuntimeAssertion, RuntimeAssertionProfile, RuntimeAssignment, RuntimeCall,
    RuntimeEvent, RuntimeField, RuntimeLog, RuntimeWaitTarget,
};
use crate::line_task::LineOutRequest;
use crate::step::{
    RuntimeContentRequest, RuntimeContentResidency, RuntimeContentResourceRequest,
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeDiagnosticSource,
};
use crate::task::{
    CancelScopeId, HostTaskRequest, NeedId, TaskId, TaskKey, TaskPolicy, TaskPriority, TaskSpec,
};
use crate::value::{
    RuntimePayload, RuntimeValue, runtime_value_into_sequence_values, runtime_value_label,
};

pub(super) enum MappedEffect {
    Line(LineEffectRequest),
    Unsupported(RuntimeDiagnostic),
}

impl AwbcEffectKind {
    #[allow(clippy::too_many_lines)]
    pub(super) fn map_product_effect(
        self,
        program: &AwbcProgram,
        effect: AwbcEffectPlanId,
        dynamic_args: &[RuntimeValue],
    ) -> MappedEffect {
        let Some(plan) = program.effect_plans.get(effect.index()) else {
            return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                RuntimeDiagnosticCategory::Internal,
                format!("missing AWBC effect plan {}", effect.0),
            ));
        };
        let static_args = plan
            .static_args
            .iter()
            .filter_map(|constant| constant_value(program, *constant).ok())
            .collect::<Vec<_>>();
        let string = |index: usize| -> String {
            static_args
                .get(index)
                .map(runtime_value_label)
                .unwrap_or_default()
        };
        let optional_string = |index: usize| -> Option<String> {
            static_args.get(index).and_then(|value| match value {
                RuntimeValue::Unit => None,
                value => Some(runtime_value_label(value)),
            })
        };
        let fields = |start: usize| -> Vec<RuntimeField> {
            static_args[start..]
                .chunks(2)
                .filter_map(|pair| {
                    Some(RuntimeField {
                        name: runtime_value_label(pair.first()?),
                        value: runtime_value_label(pair.get(1)?),
                    })
                })
                .collect()
        };
        let mapped = match self {
            Self::RegisterHandle => LineEffectRequest::RegisterHandle {
                key: string(0),
                handle: string(1),
            },
            Self::DropHandle => LineEffectRequest::DropHandle { key: string(0) },
            Self::Wait => match static_args.first() {
                Some(RuntimeValue::Duration(duration)) => {
                    LineEffectRequest::Wait(RuntimeWaitTarget::Duration(*duration))
                }
                Some(value) => {
                    LineEffectRequest::Wait(RuntimeWaitTarget::Mark(runtime_value_label(value)))
                }
                None => LineEffectRequest::Wait(RuntimeWaitTarget::Expr(String::new())),
            },
            Self::Audio => {
                return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                    RuntimeDiagnosticCategory::Capability,
                    format!(
                        "AWBC audio effect `{}` requires a typed audio command payload",
                        string(0)
                    ),
                ));
            }
            Self::Call => LineEffectRequest::Call(RuntimeCall {
                callee: string(0),
                args: static_args[1..]
                    .iter()
                    .chain(dynamic_args)
                    .map(runtime_value_label)
                    .collect(),
            }),
            Self::Log => LineEffectRequest::Log(RuntimeLog {
                level: string(0),
                message: string(1),
                fields: fields(2),
            }),
            Self::SignalWrite => LineEffectRequest::SignalWrite(RuntimeAssignment {
                target: string(0),
                value: string(1),
            }),
            Self::MetricWrite => LineEffectRequest::MetricWrite(RuntimeAssignment {
                target: string(0),
                value: string(1),
            }),
            Self::EmitEvent => LineEffectRequest::EmitEvent(RuntimeEvent {
                event: string(0),
                fields: fields(1),
            }),
            Self::Out => LineEffectRequest::Out(LineOutRequest {
                label: optional_string(0),
                value: string(1),
            }),
            Self::Return => LineEffectRequest::Return(string(0)),
            Self::Goto => LineEffectRequest::Goto(string(0)),
            Self::Panic => LineEffectRequest::Panic(string(0)),
            Self::Fail => LineEffectRequest::Fail(string(0)),
            Self::Bail => LineEffectRequest::Bail(string(0)),
            Self::Ensure => LineEffectRequest::Ensure {
                condition: string(0),
                message: string(1),
            },
            Self::Assert => LineEffectRequest::Assert(RuntimeAssertion {
                condition: string(0),
                message: string(1),
                profile: RuntimeAssertionProfile::Always,
            }),
            Self::Close => LineEffectRequest::Close(string(0)),
            Self::Select => LineEffectRequest::Select(string(0)),
            Self::Break => LineEffectRequest::Break {
                label: optional_string(0),
                value: optional_string(1),
            },
            Self::Continue => LineEffectRequest::Continue {
                label: optional_string(0),
            },
        };
        MappedEffect::Line(mapped)
    }
}

pub(super) fn content_request(
    program: &AwbcProgram,
    content: AwbcContentUnitId,
) -> Result<RuntimeContentRequest, ProductStepError> {
    let record = program
        .content_units
        .get(content.index())
        .ok_or_else(|| ProductStepError::Internal(format!("missing AWBC content {}", content.0)))?;
    let content = program
        .strings
        .get(record.public_id.index())
        .cloned()
        .ok_or_else(|| ProductStepError::Internal("missing content public id".to_owned()))?;
    let resources = record
        .resources
        .iter()
        .map(|resource| {
            let resource = program.resources.get(resource.index()).ok_or_else(|| {
                ProductStepError::Internal("missing AWBC content resource".to_owned())
            })?;
            Ok(RuntimeContentResourceRequest {
                public_id: program
                    .strings
                    .get(resource.public_id.index())
                    .cloned()
                    .unwrap_or_else(|| "awbc.resource".to_owned()),
                kind: program
                    .strings
                    .get(resource.kind.index())
                    .cloned()
                    .unwrap_or_else(|| "resource".to_owned()),
                digest: resource.digest.0,
                decoded_len: resource.decoded_len,
                residency: match resource.residency {
                    AwbcResourceResidency::Startup => RuntimeContentResidency::Startup,
                    AwbcResourceResidency::OnDemand => RuntimeContentResidency::OnDemand,
                    AwbcResourceResidency::Streaming => RuntimeContentResidency::Streaming,
                },
            })
        })
        .collect::<Result<Vec<_>, ProductStepError>>()?;
    Ok(RuntimeContentRequest { content, resources })
}

pub(super) fn task_spec(
    program: &AwbcProgram,
    plan: AwbcTaskPlanId,
    task_id: &TaskId,
    args: Vec<RuntimeValue>,
) -> Result<(NeedId, TaskSpec), ProductStepError> {
    let record = program
        .task_plans
        .get(plan.index())
        .ok_or_else(|| ProductStepError::Internal(format!("missing AWBC task plan {}", plan.0)))?;
    let string = |id: AwbcStringId| {
        program
            .strings
            .get(id.index())
            .cloned()
            .ok_or_else(|| ProductStepError::Internal("missing AWBC task string".to_owned()))
    };
    let capability = string(record.capability)?;
    let operation = string(record.operation)?;
    let need = NeedId(string(record.need_id)?);
    if args.len() != record.arguments.len() {
        return Err(ProductStepError::Input(format!(
            "AWBC task `{}` expects {} arguments, received {}",
            task_id.0,
            record.arguments.len(),
            args.len()
        )));
    }
    let mut positional = Vec::new();
    let mut named = Vec::new();
    for (argument, value) in record.arguments.iter().zip(args) {
        if argument.spread {
            let values = runtime_value_into_sequence_values(value).map_err(|value| {
                ProductStepError::Type(format!(
                    "spread task argument requires sequence, found {}",
                    runtime_value_label(&value)
                ))
            })?;
            positional.extend(values.into_iter().map(RuntimePayload::from));
        } else if let Some(name) = argument.name {
            named.push((string(name)?, RuntimePayload::from(value)));
        } else {
            positional.push(RuntimePayload::from(value));
        }
    }
    let request = HostTaskRequest::custom_with_named_args(capability, operation, positional, named);
    let class = request.task_class();
    let spec = TaskSpec::new(
        task_id.clone(),
        TaskKey(task_id.0.clone()),
        class,
        TaskPriority(record.priority),
        CancelScopeId(string(record.cancel_scope)?),
        task_policy(record.policy),
        request,
    );
    Ok((need, spec))
}

pub(super) fn source_diagnostic(
    program: &AwbcProgram,
    source_map: Option<crate::awbc::schema::AwbcSourceMapId>,
    category: RuntimeDiagnosticCategory,
    message: impl Into<String>,
) -> RuntimeDiagnostic {
    let mut diagnostic = RuntimeDiagnostic::categorized(category, message);
    if let Some(source) = source_map.and_then(|id| program.source_map.get(id.index())) {
        diagnostic = diagnostic.with_source(RuntimeDiagnosticSource {
            label: program
                .strings
                .get(source.source_file.index())
                .cloned()
                .unwrap_or_else(|| "<awbc>".to_owned()),
            start: source.start,
            end: source.end,
            anchor: source
                .anchor
                .and_then(|id| program.strings.get(id.index()).cloned()),
        });
    }
    diagnostic
}

const fn task_policy(policy: AwbcTaskPolicy) -> TaskPolicy {
    match policy {
        AwbcTaskPolicy::JoinSameKey => TaskPolicy::JoinSameKey,
        AwbcTaskPolicy::AlwaysStart => TaskPolicy::AlwaysStart,
    }
}

impl AwbcProductStepExecutor {
    pub(super) fn task_public_id(&self, plan: AwbcTaskPlanId) -> String {
        self.program
            .task_plans
            .get(plan.index())
            .and_then(|record| self.program.strings.get(record.public_id.index()))
            .cloned()
            .unwrap_or_else(|| format!("awbc.task.{}", plan.0))
    }
}
