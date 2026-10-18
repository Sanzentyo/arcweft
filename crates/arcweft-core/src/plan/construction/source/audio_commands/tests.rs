use super::super::callable_states::tests as graph;
use super::super::expression::tests::{decode, limits as expression_limits};
use super::super::pattern::tests::{meter, resources};
use super::*;
use crate::effect::LineEffectRequest;
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::value::{RuntimeSignedIntWidth, RuntimeValue};
use arcweft_interaction_model::audio::{
    AudioEffectParameterKind as Parameter, AudioLoopMode as Loop, MicrophoneConstraints,
};

fn scalar(ty: RuntimeCheckedType, value: RuntimeValue) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        ty.semantic_identity_digest(),
        RuntimeExprSeedKind::Value(value),
    )
}
fn string(text: &str) -> RuntimeExprSeed {
    scalar(
        RuntimeCheckedType::String,
        RuntimeValue::String(text.into()),
    )
}
fn integer(value: i64) -> RuntimeExprSeed {
    scalar(
        RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64),
        RuntimeValue::i64(value),
    )
}
fn boolean(value: bool) -> RuntimeExprSeed {
    scalar(RuntimeCheckedType::Bool, RuntimeValue::Bool(value))
}
fn unit() -> RuntimeExprSeed {
    scalar(RuntimeCheckedType::Unit, RuntimeValue::Unit)
}
fn microphone(preferred: bool) -> MicrophoneConstraints {
    MicrophoneConstraints {
        channels: if preferred { 2 } else { 1 },
        preferred_sample_rate_hz: preferred.then_some(48_000),
        echo_cancellation: preferred,
        noise_suppression: !preferred,
        auto_gain_control: preferred,
    }
}
fn commands() -> Vec<RuntimeAudioCommandSeed> {
    use RuntimeAudioCommandSeed as C;
    let mut commands = Vec::new();
    for loop_mode in [Loop::None, Loop::Whole, Loop::region(2, 9).unwrap()] {
        commands.push(C::Play {
            voice: string("voice"),
            resource: string("resource"),
            bus: string("bus"),
            gain_db_milli: integer(-3),
            pan_milli: integer(4),
            loop_mode,
            start_frame: integer(5),
            fade_in_millis: integer(6),
        });
    }
    commands.extend([
        C::Stop {
            voice: string("voice"),
            fade_out_millis: integer(7),
        },
        C::StopAll {
            fade_out_millis: integer(8),
        },
        C::SetVoiceGain {
            voice: string("voice"),
            gain_db_milli: integer(-9),
            transition_millis: integer(10),
        },
        C::SetVoicePan {
            voice: string("voice"),
            pan_milli: integer(11),
            transition_millis: integer(12),
        },
        C::SetBusGain {
            bus: string("bus"),
            gain_db_milli: integer(-13),
            transition_millis: integer(14),
        },
        C::SetBusMute {
            bus: string("bus"),
            muted: boolean(true),
        },
        C::SetEffectEnabled {
            bus: string("bus"),
            effect: string("effect"),
            enabled: boolean(false),
        },
    ]);
    for parameter in (0..=u8::MAX).filter_map(Parameter::from_semantic_tag) {
        let value = [-18_000, 0, 1, 500, 1_000, 100_000]
            .into_iter()
            .find(|&value| parameter.from_i64(value).is_ok())
            .unwrap();
        commands.push(C::SetEffectParameter {
            bus: string("bus"),
            effect: string("effect"),
            parameter,
            value: integer(value),
            transition_millis: integer(15),
        });
    }
    commands.push(C::ApplySnapshot {
        snapshot: string("snapshot"),
        transition_millis: integer(16),
    });
    for preferred in [false, true] {
        commands.push(C::RequestMicrophone {
            capture: string("capture"),
            constraints: microphone(preferred),
        });
    }
    commands.push(C::StopMicrophone {
        capture: string("capture"),
    });
    for bus in [None, Some(string("bus"))] {
        commands.push(C::SetCaptureMonitor {
            capture: string("capture"),
            bus,
            gain_db_milli: integer(-17),
        });
    }
    commands
}
fn builder() -> (RuntimePlanBuilder, RuntimeFunctionSiteSeedId) {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                (RuntimeCheckedType::Bool, RuntimePlanTypeProjection::Bool),
                (
                    RuntimeCheckedType::String,
                    RuntimePlanTypeProjection::String,
                ),
                (
                    RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64),
                    RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
                ),
                (RuntimeCheckedType::Unit, RuntimePlanTypeProjection::Unit),
            ]
            .map(|(ty, projection)| {
                RuntimePlanTypeSeed::new(ty.semantic_identity_digest(), projection)
            }),
            [],
        )
        .unwrap();
    let prefix = graph::reserve_executable_function(
        &mut builder,
        238,
        &[],
        0,
        RuntimeCheckedType::Unit.semantic_identity_digest(),
    );
    builder
        .define_function_site_seed(
            &prefix,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::Noop,
                    RuntimeFlowOpSeed::ReturnExpr(unit()),
                ]),
            }),
        )
        .unwrap();
    let root = graph::reserve_executable_function(
        &mut builder,
        239,
        &[],
        0,
        RuntimeCheckedType::Unit.semantic_identity_digest(),
    );
    (builder, root)
}
fn fixture() -> RuntimePlan {
    let (mut builder, root) = builder();
    let ops = commands()
        .into_iter()
        .map(|command| RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Audio(Box::new(command))))
        .chain([RuntimeFlowOpSeed::ReturnExpr(unit())])
        .collect();
    builder
        .define_function_site_seed(
            &root,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops,
            }),
        )
        .unwrap();
    builder.finish().unwrap()
}
fn audio(plan: &RuntimePlan) -> impl Iterator<Item = &RuntimeAudioCommand> {
    plan.function_sites()
        .iter()
        .flat_map(|function| match function.body() {
            RuntimeFunctionSiteBody::Executable(body) => body.ops(),
            _ => &[],
        })
        .filter_map(|op| {
            if let FlowOp::Effect(LineEffectRequest::Audio(command)) = op {
                Some(command.as_ref())
            } else {
                None
            }
        })
}
fn limits(plan: &RuntimePlan) -> RuntimeAudioCommandSourceLimits {
    RuntimeAudioCommandSourceLimits {
        nodes: 100_000,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        expression: expression_limits(plan),
    }
}
fn encode(command: &RuntimeAudioCommand, plan: &RuntimePlan) -> Vec<u8> {
    command
        .encode_source(limits(plan), &resources(), &mut meter())
        .unwrap()
        .into_bytes()
}
fn handles() -> RuntimeExpressionSourceHandles<'static> {
    RuntimeExpressionSourceHandles {
        states: &[],
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &[],
    }
}
fn source(
    plan: &RuntimePlan,
    bank: TaskSemanticMeter,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
) {
    let (source, functions, states) = graph::source_with_meter(plan, &graph::encode(plan), bank);
    assert!(states.is_empty());
    (source, functions)
}
fn finish(
    source: RuntimePlanSourceConstruction,
) -> Result<RuntimePlan, crate::plan::body_semantic::task_image::RuntimeTaskPlanSealError> {
    let seal = RuntimeTaskPlanSealLimits::default();
    Ok(source.prepare_image(seal)?.seal(seal, None)?.into_plan())
}
#[test]
fn audio_sources_restore_all_thirteen_commands_metadata_and_original_expression_order_then_seal() {
    let plan = fixture();
    assert_eq!(audio(&plan).count(), 31);
    let (mut source, functions) = source(&plan, meter());
    for (function, handle) in plan.function_sites().iter().zip(&functions) {
        let RuntimeFunctionSiteBody::Executable(body) = function.body() else {
            unreachable!()
        };
        let ops = body
            .ops()
            .iter()
            .map(|op| match op {
                FlowOp::Noop => RuntimeFlowOpSeed::Noop,
                FlowOp::ReturnExpr(expr) => {
                    RuntimeFlowOpSeed::ReturnExpr(decode(expr, &plan, &mut source, &handles()))
                }
                FlowOp::Effect(LineEffectRequest::Audio(command)) => {
                    let bytes = encode(command, &plan);
                    let prepared = PreparedRuntimeAudioCommandSource::preflight(
                        &bytes,
                        limits(&plan),
                        &resources(),
                        &mut source.meter,
                    )
                    .unwrap();
                    RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Audio(Box::new(
                        prepared
                            .materialize(&mut source, &handles(), &mut resources())
                            .unwrap(),
                    )))
                }
                _ => panic!("fixture owns exact Audio/Noop/Return operations"),
            })
            .collect();
        source
            .builder
            .define_function_site_seed(
                handle,
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: body.effects().clone(),
                    ops,
                }),
            )
            .unwrap();
    }
    let restored = finish(source).unwrap();
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    for (original, restored) in audio(&plan).zip(audio(&restored)) {
        assert_eq!(encode(original, &plan), encode(restored, &plan));
    }
}
#[test]
fn audio_sources_share_exact_aggregate_caps_and_reject_one_less_for_every_command() {
    let plan = fixture();
    for command in audio(&plan) {
        let encoded = command
            .encode_source(limits(&plan), &resources(), &mut meter())
            .unwrap();
        let bytes = encoded.bytes();
        let prepared = PreparedRuntimeAudioCommandSource::preflight(
            bytes,
            limits(&plan),
            &resources(),
            &mut meter(),
        )
        .unwrap();
        assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
        assert_eq!(prepared.elements(), encoded.elements());
        assert_eq!(prepared.node_count(), encoded.node_count());
        let exact = RuntimeAudioCommandSourceLimits {
            nodes: prepared.node_count() as u32,
            elements: prepared.elements() as u32,
            encoded_bytes: bytes.len(),
            decoded_bytes: prepared.storage_bytes(),
            ..limits(&plan)
        };
        PreparedRuntimeAudioCommandSource::preflight(bytes, exact, &resources(), &mut meter())
            .unwrap();
        command
            .encode_source(exact, &resources(), &mut meter())
            .unwrap();
        for short in [
            RuntimeAudioCommandSourceLimits {
                nodes: exact.nodes - 1,
                ..exact
            },
            RuntimeAudioCommandSourceLimits {
                elements: exact.elements - 1,
                ..exact
            },
            RuntimeAudioCommandSourceLimits {
                encoded_bytes: exact.encoded_bytes - 1,
                ..exact
            },
            RuntimeAudioCommandSourceLimits {
                decoded_bytes: exact.decoded_bytes - 1,
                ..exact
            },
        ] {
            let mut bank = meter();
            assert!(
                PreparedRuntimeAudioCommandSource::preflight(bytes, short, &resources(), &mut bank)
                    .is_err()
            );
            assert!(bank.status().is_err());
            assert!(
                command
                    .encode_source(short, &resources(), &mut meter())
                    .is_err()
            );
        }
    }
}
#[test]
fn audio_sources_reject_all_truncations_unknown_tags_flags_and_invalid_owned_metadata() {
    let plan = fixture();
    for command in audio(&plan) {
        let bytes = encode(command, &plan);
        for end in 0..bytes.len() {
            assert!(
                PreparedRuntimeAudioCommandSource::preflight(
                    &bytes[..end],
                    limits(&plan),
                    &resources(),
                    &mut meter()
                )
                .is_err(),
                "prefix {end}"
            );
        }
        for (at, value) in [(0, 0), (DOMAIN.len(), 2), (DOMAIN.len() + 1, 13)] {
            let mut bad = bytes.clone();
            bad[at] = value;
            assert!(
                PreparedRuntimeAudioCommandSource::preflight(
                    &bad,
                    limits(&plan),
                    &resources(),
                    &mut meter()
                )
                .is_err()
            );
        }
        let at = DOMAIN.len() + 2;
        match command {
            RuntimeAudioCommand::Play { loop_mode, .. } => {
                let mut bad = bytes.clone();
                bad[at] = 3;
                assert!(
                    PreparedRuntimeAudioCommandSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    )
                    .is_err()
                );
                if matches!(loop_mode, Loop::Region { .. }) {
                    let mut bad = bytes.clone();
                    bad[at + 1..at + 9].copy_from_slice(&9u64.to_le_bytes());
                    assert!(matches!(
                        PreparedRuntimeAudioCommandSource::preflight(
                            &bad,
                            limits(&plan),
                            &resources(),
                            &mut meter()
                        ),
                        Err(Error::Metadata(_))
                    ));
                }
            }
            RuntimeAudioCommand::SetEffectParameter { .. } => {
                let mut bad = bytes.clone();
                bad[at] = 255;
                assert!(
                    PreparedRuntimeAudioCommandSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    )
                    .is_err()
                );
            }
            RuntimeAudioCommand::RequestMicrophone { constraints, .. } => {
                let mut bad = bytes.clone();
                bad[at..at + 2].fill(0);
                assert!(matches!(
                    PreparedRuntimeAudioCommandSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    ),
                    Err(Error::Metadata(_))
                ));
                let mut bad = bytes.clone();
                bad[at + 2] = 2;
                assert!(
                    PreparedRuntimeAudioCommandSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    )
                    .is_err()
                );
                let flag = at
                    + 3
                    + if constraints.preferred_sample_rate_hz.is_some() {
                        4
                    } else {
                        0
                    };
                let mut bad = bytes.clone();
                bad[flag] = 2;
                assert!(
                    PreparedRuntimeAudioCommandSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    )
                    .is_err()
                );
                if constraints.preferred_sample_rate_hz.is_some() {
                    let mut bad = bytes.clone();
                    bad[at + 3..at + 7].fill(0);
                    assert!(matches!(
                        PreparedRuntimeAudioCommandSource::preflight(
                            &bad,
                            limits(&plan),
                            &resources(),
                            &mut meter()
                        ),
                        Err(Error::Metadata(_))
                    ));
                }
            }
            RuntimeAudioCommand::SetCaptureMonitor { .. } => {
                let mut bad = bytes.clone();
                bad[at] = 2;
                assert!(
                    PreparedRuntimeAudioCommandSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    )
                    .is_err()
                );
            }
            _ => {}
        }
        let mut bad = bytes;
        bad.push(0);
        assert!(matches!(
            PreparedRuntimeAudioCommandSource::preflight(
                &bad,
                limits(&plan),
                &resources(),
                &mut meter()
            ),
            Err(Error::Trailing { .. })
        ));
    }
}
#[test]
fn audio_metadata_is_checked_before_child_admission_and_late_failure_cannot_publish_prefix() {
    let invalid = || {
        RuntimeExprSeed::new(
            RuntimeSemanticTypeId::from_bytes([255; 32]),
            RuntimeExprSeedKind::Value(RuntimeValue::Unit),
        )
    };
    for kind in 0..3 {
        let (mut builder, root) = builder();
        let command = if kind == 0 {
            RuntimeAudioCommandSeed::Play {
                voice: invalid(),
                resource: unit(),
                bus: unit(),
                gain_db_milli: unit(),
                pan_milli: unit(),
                loop_mode: Loop::Region {
                    start_frame: 9,
                    end_frame: 3,
                },
                start_frame: unit(),
                fade_in_millis: unit(),
            }
        } else {
            let mut constraints = microphone(true);
            if kind == 1 {
                constraints.channels = 0;
            } else {
                constraints.preferred_sample_rate_hz = Some(0);
            }
            RuntimeAudioCommandSeed::RequestMicrophone {
                capture: invalid(),
                constraints,
            }
        };
        let result = builder.define_function_site_seed(
            &root,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Audio(
                    Box::new(command),
                ))]),
            }),
        );
        assert!(matches!(
            result,
            Err(RuntimePlanBuildError::AudioMetadata(_))
        ));
        assert!(builder.function_sites[0].body.is_some());
        assert!(builder.function_sites[1].body.is_none());
        assert!(builder.finish().is_err());
    }
}
#[test]
fn audio_sources_authenticate_actual_namespaces_before_materializing_the_first_child() {
    let plan = fixture();
    let bytes = encode(audio(&plan).next().unwrap(), &plan);
    let (mut source, _) = source(&plan, meter());
    let mut declared = limits(&plan);
    declared.expression.type_count += 1;
    let prepared = PreparedRuntimeAudioCommandSource::preflight(
        &bytes,
        declared,
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &handles(), &mut resources()),
        Err(Error::Expression(RuntimeExpressionSourceError::Namespace {
            kind: "type",
            ..
        }))
    ));
    assert!(
        source
            .builder
            .function_sites
            .iter()
            .all(|row| row.body.is_none())
    );
    assert!(source.meter.status().is_err());
    assert!(finish(source).is_err());
}
#[test]
fn audio_sources_keep_the_original_finite_work_bank_and_reserved_output_prefix() {
    let plan = fixture();
    let command = audio(&plan).next().unwrap();
    let bytes = encode(command, &plan);
    let (trial, _) = source(&plan, meter());
    let used = trial.meter.totals().0;
    let (mut source, _) = source(&plan, TaskSemanticMeter::new(used + 1, u64::MAX));
    assert!(matches!(
        PreparedRuntimeAudioCommandSource::preflight(
            &bytes,
            limits(&plan),
            &resources(),
            &mut source.meter
        ),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert!(finish(source).is_err());
    let encoded = command
        .encode_source(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(encoded.bytes().len());
    output.push(97);
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [97]);
    let encoded = command
        .encode_source(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(encoded.bytes().len() + 1);
    output.push(97);
    encoded.append_to(&mut output).unwrap();
    assert_eq!(output[1..], bytes);
}
#[test]
fn canonical_audio_mapping_visits_once_in_abi_order_and_stops_on_first_rejection() {
    let command = RuntimeAudioCommand::Play {
        voice: 0,
        resource: 1,
        bus: 2,
        gain_db_milli: 3,
        pan_milli: 4,
        loop_mode: Loop::Whole,
        start_frame: 5,
        fade_in_millis: 6,
    };
    let mut visited = Vec::new();
    let result = command.expression_refs().try_map_expressions(|&value| {
        visited.push(value);
        if value == 3 { Err(()) } else { Ok(value + 10) }
    });
    assert!(result.is_err());
    assert_eq!(visited, [0, 1, 2, 3]);
    let mapped = command
        .try_map_expressions(|value| Ok::<_, ()>(value + 10))
        .unwrap();
    assert!(matches!(
        mapped,
        RuntimeAudioCommand::Play {
            voice: 10,
            resource: 11,
            bus: 12,
            gain_db_milli: 13,
            pan_milli: 14,
            loop_mode: Loop::Whole,
            start_frame: 15,
            fade_in_millis: 16
        }
    ));
}
#[test]
fn phase_parameterized_audio_still_cannot_enter_through_a_raw_static_expression_carrier() {
    let plan = fixture();
    let command = audio(&plan).next().unwrap().clone();
    let (mut builder, root) = builder();
    let result = builder.define_function_site_seed(
        &root,
        RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
            effects: RuntimeEffectSet::empty(),
            ops: Box::new([RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Static(
                LineEffectRequest::Audio(Box::new(command)),
            ))]),
        }),
    );
    assert!(matches!(
        result,
        Err(RuntimePlanBuildError::RawExpressionCarrier {
            context: "static line effect Audio"
        })
    ));
    assert!(builder.finish().is_err());
}
