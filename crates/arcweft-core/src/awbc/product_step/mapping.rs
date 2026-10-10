use super::ProductStepError;
use crate::awbc::schema::{
    AwbcConstant, AwbcContentUnitId, AwbcEffectKind, AwbcEffectPlanId, AwbcProgram,
    AwbcResourceResidency, AwbcStringId, AwbcTaskPlanId, AwbcTaskRequestProjection,
};
use crate::awbc::vm::constant_value;
use crate::effect::{
    LineEffectRequest, RuntimeAssertion, RuntimeAssertionGuardId, RuntimeAssertionProfile,
    RuntimeAssignment, RuntimeCall, RuntimeEvent, RuntimeField, RuntimeLog, RuntimeWaitTarget,
};
use crate::line_task::LineOutRequest;
use crate::step::{
    RuntimeContentRequest, RuntimeContentResidency, RuntimeContentResourceRequest,
    RuntimeDiagnostic, RuntimeDiagnosticCategory, RuntimeDiagnosticSource,
};
use crate::task::HostTaskRequest;
use crate::value::{
    RuntimePayload, RuntimeValue, runtime_value_into_sequence_values, runtime_value_label,
};
use arcweft_interaction_model::audio::AudioCommand;

pub(super) enum MappedEffect {
    Omitted,
    Line(LineEffectRequest),
    Audio(AudioCommand),
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
        let dynamic_string = |index: usize| -> String {
            dynamic_args
                .get(index)
                .map(runtime_value_label)
                .unwrap_or_default()
        };
        let dynamic_fields = |static_start: usize, dynamic_start: usize| -> Vec<RuntimeField> {
            static_args[static_start..]
                .chunks(2)
                .zip(&dynamic_args[dynamic_start..])
                .filter_map(|(pair, value)| {
                    Some(RuntimeField {
                        name: runtime_value_label(pair.first()?),
                        value: runtime_value_label(value),
                    })
                })
                .collect()
        };
        let assertion_guard = |index: usize| -> Option<RuntimeAssertionGuardId> {
            let id = *plan.static_args.get(index)?;
            let AwbcConstant::Bytes(bytes) = program.constants.get(id.index())? else {
                return None;
            };
            let bytes: [u8; 16] = bytes.as_slice().try_into().ok()?;
            RuntimeAssertionGuardId::try_from_bytes(bytes).ok()
        };
        let mapped = match self {
            Self::Wait => match static_args.first() {
                Some(RuntimeValue::Duration(duration)) => {
                    LineEffectRequest::Wait(RuntimeWaitTarget::Duration(*duration))
                }
                Some(_) => {
                    return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Type,
                        "AWBC wait target must evaluate to Duration",
                    ));
                }
                None => {
                    return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Internal,
                        "AWBC wait effect is missing its Duration target",
                    ));
                }
            },
            Self::Audio => match plan
                .audio
                .and_then(|audio| program.audio_commands.get(audio.index()))
            {
                Some(command) => {
                    return command
                        .map_product_audio(program, dynamic_args)
                        .map_or_else(
                            |error| {
                                MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                                    error.category(),
                                    error.to_string(),
                                ))
                            },
                            MappedEffect::Audio,
                        );
                }
                None => {
                    return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Internal,
                        "AWBC audio effect is missing typed audio command payload",
                    ));
                }
            },
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
                message: if dynamic_args.is_empty() {
                    string(1)
                } else {
                    dynamic_string(0)
                },
                fields: if dynamic_args.is_empty() {
                    fields(2)
                } else {
                    dynamic_fields(2, 1)
                },
            }),
            Self::SignalWrite | Self::MetricWrite => {
                let values = if dynamic_args.is_empty() {
                    static_args.as_slice()
                } else {
                    dynamic_args
                };
                let [target, value] = values else {
                    return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Internal,
                        "AWBC signal and metric writes require exactly two evaluated arguments",
                    ));
                };
                let write = match RuntimeAssignment::try_copy(runtime_value_label(target), value) {
                    Ok(write) => write,
                    Err(error) => {
                        return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                            RuntimeDiagnosticCategory::Type,
                            error.to_string(),
                        ));
                    }
                };
                if self == Self::SignalWrite {
                    LineEffectRequest::SignalWrite(write)
                } else {
                    LineEffectRequest::MetricWrite(write)
                }
            }
            Self::EmitEvent => LineEffectRequest::EmitEvent(RuntimeEvent {
                event: if dynamic_args.is_empty() {
                    string(0)
                } else {
                    dynamic_string(0)
                },
                fields: if dynamic_args.is_empty() {
                    fields(1)
                } else {
                    dynamic_fields(1, 1)
                },
            }),
            Self::Out => LineEffectRequest::Out(LineOutRequest {
                label: optional_string(0),
                value: string(1),
            }),
            Self::Return => LineEffectRequest::Return(string(0)),
            Self::Goto => LineEffectRequest::Goto(string(0)),
            Self::Panic => LineEffectRequest::Panic(if dynamic_args.is_empty() {
                string(0)
            } else {
                dynamic_string(0)
            }),
            Self::Fail => LineEffectRequest::Fail(if dynamic_args.is_empty() {
                string(0)
            } else {
                dynamic_string(0)
            }),
            Self::Bail => LineEffectRequest::Bail(if dynamic_args.is_empty() {
                string(0)
            } else {
                dynamic_string(0)
            }),
            Self::Ensure => LineEffectRequest::Ensure {
                condition: if dynamic_args.is_empty() {
                    string(0)
                } else {
                    dynamic_string(0)
                },
                message: if dynamic_args.is_empty() {
                    string(1)
                } else {
                    dynamic_string(1)
                },
            },
            Self::Assert => {
                let Some(guard) = assertion_guard(0) else {
                    return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                        RuntimeDiagnosticCategory::Internal,
                        "malformed AWBC assertion guard",
                    ));
                };
                let profile = match string(3).as_str() {
                    "always" => RuntimeAssertionProfile::Always,
                    "debug_only" => RuntimeAssertionProfile::DebugOnly,
                    _ => {
                        return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                            RuntimeDiagnosticCategory::Internal,
                            "malformed AWBC assertion profile",
                        ));
                    }
                };
                if dynamic_args.is_empty() {
                    LineEffectRequest::Assert(RuntimeAssertion::new(
                        guard,
                        string(1),
                        string(2),
                        profile,
                    ))
                } else {
                    let Some(condition_value) = dynamic_args.first() else {
                        return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                            RuntimeDiagnosticCategory::Type,
                            "AWBC assertion condition must evaluate to Bool",
                        ));
                    };
                    let RuntimeValue::Bool(condition) = condition_value else {
                        return MappedEffect::Unsupported(RuntimeDiagnostic::categorized(
                            RuntimeDiagnosticCategory::Type,
                            "AWBC assertion condition must evaluate to Bool",
                        ));
                    };
                    if *condition {
                        return MappedEffect::Omitted;
                    }
                    LineEffectRequest::Assert(RuntimeAssertion::new(
                        guard,
                        runtime_value_label(condition_value),
                        string(2),
                        profile,
                    ))
                }
            }
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

pub(super) fn task_request(
    program: &AwbcProgram,
    plan: AwbcTaskPlanId,
    args: Vec<RuntimeValue>,
) -> Result<HostTaskRequest, ProductStepError> {
    let record = program
        .task_plans
        .get(plan.index())
        .ok_or_else(|| ProductStepError::Internal("task plan is absent".into()))?;
    let string = |id: AwbcStringId| {
        program
            .strings
            .get(id.index())
            .cloned()
            .ok_or_else(|| ProductStepError::Internal("task string is absent".into()))
    };
    let AwbcTaskRequestProjection::CustomCapability {
        capability,
        operation,
    } = record.request
    else {
        return Err(ProductStepError::Internal(
            "template requires a custom task request".into(),
        ));
    };
    if args.len() != record.arguments.len() {
        return Err(ProductStepError::Input(
            "task request kernel result has the wrong arity".into(),
        ));
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
    Ok(HostTaskRequest::custom_with_named_args(
        string(capability)?,
        string(operation)?,
        positional,
        named,
    ))
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

#[cfg(test)]
mod typed_assignment_tests {
    use super::*;
    use crate::awbc::schema::{AwbcConstantId, AwbcEffectPlan, AwbcSignatureId};
    use crate::effect::RuntimeEffectExpr;
    use crate::plan::{FlowRuntimeId, RuntimeFunctionSiteBodyKind};
    use crate::value::{RuntimeEntityReference, RuntimeImportedProjectEntityReference};

    fn native_materializer(kind: AwbcEffectKind) -> RuntimeEffectExpr {
        // Materialization consumes evaluated arguments. Its expression shell
        // borrows a real admitted expression rather than fabricating plan IDs.
        let plan = crate::tests::function_application::returning_function_plan(
            RuntimeFunctionSiteBodyKind::Expression,
        );
        let expression = plan
            .function_sites()
            .iter()
            .find_map(|site| site.body().expression())
            .unwrap()
            .clone();
        match kind {
            AwbcEffectKind::SignalWrite => RuntimeEffectExpr::SignalWrite {
                target: expression.clone(),
                value: expression,
            },
            AwbcEffectKind::MetricWrite => RuntimeEffectExpr::MetricWrite {
                target: expression.clone(),
                value: expression,
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn native_and_awbc_assignment_materializers_retain_the_same_typed_values() {
        let flow = FlowRuntimeId::from_checked_declaration_digest([7; 32], "flow.opening").unwrap();
        let imported = RuntimeImportedProjectEntityReference::try_new(
            arcweft_id::ProjectEntityReferenceFamily::Flow,
            arcweft_id::PublicId::try_new("flow.opening").unwrap(),
            [11; 32],
            [12; 32],
            [13; 32],
            Some(flow.clone()),
        )
        .unwrap();
        let cases = [
            (RuntimeValue::Bool(true), AwbcConstant::Bool(true)),
            (
                RuntimeValue::i64(42),
                AwbcConstant::Int {
                    kind: crate::awbc::schema::AwbcSignedIntKind::I64,
                    bits: 42_i128.to_le_bytes(),
                },
            ),
            (
                RuntimeValue::u64(u64::MAX),
                AwbcConstant::UInt {
                    kind: crate::awbc::schema::AwbcUnsignedIntKind::U64,
                    bits: u128::from(u64::MAX).to_le_bytes(),
                },
            ),
            (
                RuntimeValue::F32(0.5),
                AwbcConstant::F32Bits(0.5_f32.to_bits()),
            ),
            (
                RuntimeValue::String("true".to_owned()),
                AwbcConstant::String(AwbcStringId(1)),
            ),
            (
                RuntimeValue::String("42".to_owned()),
                AwbcConstant::String(AwbcStringId(2)),
            ),
            (
                RuntimeValue::String("@flow.opening".to_owned()),
                AwbcConstant::String(AwbcStringId(3)),
            ),
            (
                RuntimeValue::EntityRef(RuntimeEntityReference::StructuralFlow(flow.clone())),
                AwbcConstant::EntityRef(RuntimeEntityReference::StructuralFlow(flow)),
            ),
            (
                RuntimeValue::EntityRef(RuntimeEntityReference::ImportedProject(imported.clone())),
                AwbcConstant::EntityRef(RuntimeEntityReference::ImportedProject(imported)),
            ),
        ];
        for kind in [AwbcEffectKind::SignalWrite, AwbcEffectKind::MetricWrite] {
            for (value, constant) in &cases {
                let target = if kind == AwbcEffectKind::SignalWrite {
                    "signal.observed"
                } else {
                    "metric.observed"
                };
                let args = [RuntimeValue::String(target.to_owned()), value.clone()];
                let native = native_materializer(kind)
                    .materialize(&args)
                    .unwrap()
                    .unwrap();
                let program = AwbcProgram {
                    strings: vec![
                        target.to_owned(),
                        "true".to_owned(),
                        "42".to_owned(),
                        "@flow.opening".to_owned(),
                    ],
                    constants: vec![AwbcConstant::String(AwbcStringId(0)), constant.clone()],
                    effect_plans: vec![AwbcEffectPlan {
                        kind,
                        signature: AwbcSignatureId(0),
                        capability: None,
                        audio: None,
                        static_args: vec![AwbcConstantId(0), AwbcConstantId(1)],
                        resources: Vec::new(),
                    }],
                    ..AwbcProgram::default()
                };
                for dynamic in [&[][..], &args[..]] {
                    let MappedEffect::Line(awbc) =
                        kind.map_product_effect(&program, AwbcEffectPlanId(0), dynamic)
                    else {
                        panic!("typed AWBC assignment must materialize");
                    };
                    assert_eq!(awbc, native);
                    let mut state = crate::observation::RuntimeObservationState::default();
                    state.record_effect(&awbc);
                    let retained = if kind == AwbcEffectKind::SignalWrite {
                        state.signals()
                    } else {
                        state.metrics()
                    };
                    assert_eq!(retained[target].value(), value);
                }
            }
        }
    }

    #[test]
    fn assignment_materializers_refuse_affine_values_before_observation_copy() {
        let value = RuntimeValue::Tuple(vec![
            RuntimeValue::Bool(true),
            RuntimeValue::NeedHandle(crate::tests::reusable_need("need.observation")),
        ]);
        let args = [RuntimeValue::String("signal.current".to_owned()), value];
        let native = native_materializer(AwbcEffectKind::SignalWrite).materialize(&args);
        assert!(matches!(
            native,
            Err(crate::effect::RuntimeEffectMaterializeError::Assignment(
                crate::effect::RuntimeAssignmentError::AffineValue,
            )),
        ));
        let program = AwbcProgram {
            effect_plans: vec![AwbcEffectPlan {
                kind: AwbcEffectKind::SignalWrite,
                signature: AwbcSignatureId(0),
                capability: None,
                audio: None,
                static_args: Vec::new(),
                resources: Vec::new(),
            }],
            ..AwbcProgram::default()
        };
        let MappedEffect::Unsupported(error) =
            AwbcEffectKind::SignalWrite.map_product_effect(&program, AwbcEffectPlanId(0), &args)
        else {
            panic!("the AWBC adapter must refuse an affine observation");
        };
        assert_eq!(error.category, RuntimeDiagnosticCategory::Type);
        assert!(!args[1].ownership().permits_copy());
    }
}
