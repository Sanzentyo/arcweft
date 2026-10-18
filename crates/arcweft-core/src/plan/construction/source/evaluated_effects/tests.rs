use super::super::callable_states::tests as graph;
use super::super::expression::{
    RuntimeExpressionSourceHandles,
    tests::{decode, limits as expression_limits},
};
use super::super::pattern::tests::{meter, resources};
use super::*;
use crate::effect::{RuntimeAssertionGuardId, RuntimeAssertionProfile, RuntimeEffectExpr};
use crate::pattern::RuntimeCheckedType;
use crate::plan::*;
use crate::value::RuntimeValue;

fn value(ty: RuntimeCheckedType, value: RuntimeValue) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        ty.semantic_identity_digest(),
        RuntimeExprSeedKind::Value(value),
    )
}
fn string(text: &str) -> RuntimeExprSeed {
    value(
        RuntimeCheckedType::String,
        RuntimeValue::String(text.into()),
    )
}
fn boolean() -> RuntimeExprSeed {
    value(RuntimeCheckedType::Bool, RuntimeValue::Bool(true))
}
fn unit() -> RuntimeExprSeed {
    value(RuntimeCheckedType::Unit, RuntimeValue::Unit)
}
fn fixture() -> RuntimePlan {
    fixture_with_registry_key(arcweft_id::LifetimeRegistryKey::new(
        arcweft_id::LifetimeScopeKind::Flow,
        ["flags", "既読"]
            .into_iter()
            .map(|name| arcweft_id::DeclarationName::try_new(name).unwrap())
            .collect(),
    ))
}
fn fixture_with_registry_key(key: arcweft_id::LifetimeRegistryKey) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                (RuntimeCheckedType::Bool, RuntimePlanTypeProjection::Bool),
                (
                    RuntimeCheckedType::String,
                    RuntimePlanTypeProjection::String,
                ),
                (RuntimeCheckedType::Unit, RuntimePlanTypeProjection::Unit),
                (
                    RuntimeCheckedType::Duration,
                    RuntimePlanTypeProjection::Duration,
                ),
            ]
            .map(|(ty, projection)| {
                RuntimePlanTypeSeed::new(ty.semantic_identity_digest(), projection)
            }),
            [],
        )
        .unwrap();
    let prefix = graph::reserve_executable_function(
        &mut builder,
        232,
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
        233,
        &[],
        0,
        RuntimeCheckedType::Unit.semantic_identity_digest(),
    );
    use RuntimeEvaluatedEffectSeed as Effect;
    let mut effects = vec![
        Effect::Log {
            level: "情報".into(),
            message: string("original message"),
            fields: vec![
                RuntimeEffectFieldSeed {
                    name: "同名".into(),
                    value: boolean(),
                },
                RuntimeEffectFieldSeed {
                    name: "同名".into(),
                    value: string("second"),
                },
            ],
        },
        Effect::SignalWrite {
            target: string("signal"),
            value: boolean(),
        },
        Effect::MetricWrite {
            target: string("metric"),
            value: boolean(),
        },
        Effect::RegistryWrite {
            key,
            value: boolean(),
        },
        Effect::EmitEvent {
            event: string("event"),
            fields: vec![],
        },
        Effect::Panic(string("panic")),
        Effect::Fail(string("fail")),
        Effect::Bail(string("bail")),
        Effect::Ensure {
            condition: boolean(),
            message: string("ensure"),
        },
    ];
    for policy in [
        RuntimeDropPolicySeed::Default,
        RuntimeDropPolicySeed::Cancel,
        RuntimeDropPolicySeed::Stop {
            fade: value(
                RuntimeCheckedType::Duration,
                RuntimeValue::Duration(crate::time::LogicalDuration::from_nanos(7)),
            ),
        },
        RuntimeDropPolicySeed::Finish,
        RuntimeDropPolicySeed::Release,
        RuntimeDropPolicySeed::Detach,
    ] {
        effects.push(Effect::Drop {
            target: unit(),
            policy,
        });
    }
    for profile in [
        RuntimeAssertionProfile::Always,
        RuntimeAssertionProfile::DebugOnly,
    ] {
        effects.push(Effect::Assert {
            guard: RuntimeAssertionGuardId::try_from_bytes([237; 16]).unwrap(),
            condition: boolean(),
            message: "must hold".into(),
            profile,
        });
    }
    let ops = effects
        .into_iter()
        .map(RuntimeFlowOpSeed::EvaluatedEffect)
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
fn effects(plan: &RuntimePlan) -> impl Iterator<Item = &RuntimeEffectExpr> {
    plan.function_sites()
        .iter()
        .flat_map(|function| match function.body() {
            RuntimeFunctionSiteBody::Executable(body) => body.ops(),
            _ => &[],
        })
        .filter_map(|op| {
            if let FlowOp::EvaluatedEffect(effect) = op {
                Some(effect)
            } else {
                None
            }
        })
}
fn limits(plan: &RuntimePlan) -> RuntimeEvaluatedEffectSourceLimits {
    RuntimeEvaluatedEffectSourceLimits {
        nodes: 100_000,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        expression: expression_limits(plan),
    }
}
fn encode(effect: &RuntimeEffectExpr, plan: &RuntimePlan) -> Vec<u8> {
    effect
        .encode_source(limits(plan), &resources(), &mut meter())
        .unwrap()
        .into_bytes()
}
fn source(
    plan: &RuntimePlan,
    bank: TaskSemanticMeter,
) -> (
    super::super::RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
) {
    let (source, functions, states) = graph::source_with_meter(plan, &graph::encode(plan), bank);
    assert!(states.is_empty());
    (source, functions)
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
fn replay(
    plan: &RuntimePlan,
    source: &mut super::super::RuntimePlanSourceConstruction,
    functions: &[RuntimeFunctionSiteSeedId],
    bad_condition: bool,
) -> Result<(), RuntimePlanBuildError> {
    for (function, handle) in plan.function_sites().iter().zip(functions) {
        let RuntimeFunctionSiteBody::Executable(body) = function.body() else {
            unreachable!()
        };
        let mut ops = Vec::new();
        for op in body.ops() {
            ops.push(match op {
                FlowOp::Noop => RuntimeFlowOpSeed::Noop,
                FlowOp::ReturnExpr(expr) => {
                    RuntimeFlowOpSeed::ReturnExpr(decode(expr, plan, source, &handles()))
                }
                FlowOp::EvaluatedEffect(effect) => {
                    let mut bytes = encode(effect, plan);
                    if bad_condition && matches!(effect, RuntimeEffectExpr::Ensure { .. }) {
                        let row = cursor::Cursor::new(&bytes).row().unwrap();
                        let end = DOMAIN.len() + 2 + 4 + row.first.len();
                        let replacement = row.second.unwrap().to_vec();
                        let frame = (replacement.len() as u32)
                            .to_le_bytes()
                            .into_iter()
                            .chain(replacement);
                        bytes.splice(DOMAIN.len() + 2..end, frame);
                    }
                    let prepared = PreparedRuntimeEvaluatedEffectSource::preflight(
                        &bytes,
                        limits(plan),
                        &resources(),
                        &mut source.meter,
                    )
                    .unwrap();
                    RuntimeFlowOpSeed::EvaluatedEffect(
                        prepared
                            .materialize(source, &handles(), &mut resources())
                            .unwrap(),
                    )
                }
                _ => panic!(
                    "fixture only supplies evaluated effects and exact surrounding Noop/Return"
                ),
            });
        }
        source.builder.define_function_site_seed(
            handle,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: body.effects().clone(),
                ops: ops.into_boxed_slice(),
            }),
        )?;
    }
    Ok(())
}
fn finish(
    source: super::super::RuntimePlanSourceConstruction,
) -> Result<RuntimePlan, crate::plan::body_semantic::task_image::RuntimeTaskPlanSealError> {
    let seal = RuntimeTaskPlanSealLimits::default();
    Ok(source.prepare_image(seal)?.seal(seal, None)?.into_plan())
}
#[test]
fn evaluated_effect_sources_restore_all_kinds_six_drop_policies_assertion_profiles_fields_and_semantics()
 {
    let plan = fixture();
    assert_eq!(effects(&plan).count(), 17);
    let (mut source, functions) = source(&plan, meter());
    replay(&plan, &mut source, &functions, false).unwrap();
    let restored = finish(source).unwrap();
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    for (original, restored) in effects(&plan).zip(effects(&restored)) {
        assert_eq!(encode(original, &plan), encode(restored, &plan));
    }
}

#[test]
fn registry_effect_sources_preserve_all_selectors_and_refuse_context_free_execution() {
    let plan = fixture();
    let value = effects(&plan)
        .find_map(|effect| match effect {
            RuntimeEffectExpr::RegistryWrite { value, .. } => Some(value),
            _ => None,
        })
        .unwrap();
    for scope in [
        "frame",
        "tick",
        "cue",
        "line",
        "scene",
        "flow",
        "session",
        "global",
        "persistent",
        "章",
    ] {
        for parts in [vec![], vec!["旗", "既読"]] {
            let key = arcweft_id::LifetimeRegistryKey::new(
                arcweft_id::LifetimeScopeKind::parse(scope).unwrap(),
                parts
                    .into_iter()
                    .map(|name| arcweft_id::DeclarationName::try_new(name).unwrap())
                    .collect(),
            );
            let effect = RuntimeEffectExpr::RegistryWrite {
                key: key.clone(),
                value: value.clone(),
            };
            assert_eq!(effect.argument_exprs(), vec![value]);
            assert!(effect.host_descriptor().is_none());
            assert_eq!(
                effect.materialize(&[RuntimeValue::Bool(true)]),
                Err(crate::effect::RuntimeEffectMaterializeError::RegistryContextRequired)
            );
            let bytes = encode(&effect, &plan);
            let (mut source, _) = source(&plan, meter());
            let prepared = PreparedRuntimeEvaluatedEffectSource::preflight(
                &bytes,
                limits(&plan),
                &resources(),
                &mut source.meter,
            )
            .unwrap();
            let seed = prepared
                .materialize(&mut source, &handles(), &mut resources())
                .unwrap();
            let RuntimeEvaluatedEffectSeed::RegistryWrite {
                key: restored,
                value: restored_value,
            } = seed
            else {
                panic!("registry source must retain its actual operation family");
            };
            assert_eq!(restored, key);
            assert_eq!(
                restored_value.ty(),
                RuntimeCheckedType::Bool.semantic_identity_digest()
            );
        }
    }
}

#[test]
fn registry_effect_key_preflight_preserves_identity_errors_and_resource_precedence() {
    let plan = fixture();
    let effect = effects(&plan)
        .find(|effect| matches!(effect, RuntimeEffectExpr::RegistryWrite { .. }))
        .unwrap();
    let mut bytes = encode(effect, &plan);
    // Domain, version, operation tag, key frame length, scope string length.
    let scope = DOMAIN.len() + 2 + 4 + 4;
    bytes[scope..scope + 4].copy_from_slice(b"'bad");
    let mut bank = meter();
    assert!(matches!(PreparedRuntimeEvaluatedEffectSource::preflight(
        &bytes, limits(&plan), &resources(), &mut bank,
    ), Err(Error::RegistryKey(arcweft_id::DeclarationNameError::InvalidIdentifier { value })) if value == "'bad"));
    assert!(bank.status().is_err());
    let short = RuntimeEvaluatedEffectSourceLimits {
        elements: 1,
        ..limits(&plan)
    };
    assert!(matches!(
        PreparedRuntimeEvaluatedEffectSource::preflight(&bytes, short, &resources(), &mut meter(),),
        Err(Error::Limit {
            kind: "elements",
            ..
        })
    ));
}

#[test]
fn registry_effect_key_scope_order_and_boundaries_are_executable_semantic_identity() {
    let key = |scope, parts: &[&str]| {
        arcweft_id::LifetimeRegistryKey::new(
            scope,
            parts
                .iter()
                .map(|name| arcweft_id::DeclarationName::try_new(*name).unwrap())
                .collect(),
        )
    };
    let original =
        fixture_with_registry_key(key(arcweft_id::LifetimeScopeKind::Flow, &["flags", "seen"]));
    for changed in [
        key(arcweft_id::LifetimeScopeKind::Session, &["flags", "seen"]),
        key(arcweft_id::LifetimeScopeKind::Flow, &["seen", "flags"]),
        key(arcweft_id::LifetimeScopeKind::Flow, &["flags_seen"]),
        key(arcweft_id::LifetimeScopeKind::Flow, &[]),
    ] {
        let changed = fixture_with_registry_key(changed);
        assert_ne!(
            original.executable_semantic_digest(),
            changed.executable_semantic_digest()
        );
    }
}
#[test]
fn evaluated_effect_sources_share_exact_aggregate_caps_and_reject_one_less() {
    let plan = fixture();
    for effect in effects(&plan) {
        let encoded = effect
            .encode_source(limits(&plan), &resources(), &mut meter())
            .unwrap();
        let bytes = encoded.bytes();
        let prepared = PreparedRuntimeEvaluatedEffectSource::preflight(
            bytes,
            limits(&plan),
            &resources(),
            &mut meter(),
        )
        .unwrap();
        assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
        assert_eq!(prepared.elements(), encoded.elements());
        assert_eq!(prepared.node_count(), encoded.node_count());
        let exact = RuntimeEvaluatedEffectSourceLimits {
            nodes: prepared.node_count() as u32,
            elements: prepared.elements() as u32,
            encoded_bytes: bytes.len(),
            decoded_bytes: prepared.storage_bytes(),
            ..limits(&plan)
        };
        PreparedRuntimeEvaluatedEffectSource::preflight(bytes, exact, &resources(), &mut meter())
            .unwrap();
        effect
            .encode_source(exact, &resources(), &mut meter())
            .unwrap();
        for short in [
            RuntimeEvaluatedEffectSourceLimits {
                nodes: exact.nodes - 1,
                ..exact
            },
            RuntimeEvaluatedEffectSourceLimits {
                elements: exact.elements - 1,
                ..exact
            },
            RuntimeEvaluatedEffectSourceLimits {
                encoded_bytes: exact.encoded_bytes - 1,
                ..exact
            },
            RuntimeEvaluatedEffectSourceLimits {
                decoded_bytes: exact.decoded_bytes - 1,
                ..exact
            },
        ] {
            let mut bank = meter();
            assert!(
                PreparedRuntimeEvaluatedEffectSource::preflight(
                    bytes,
                    short,
                    &resources(),
                    &mut bank
                )
                .is_err()
            );
            assert!(bank.status().is_err());
            assert!(
                effect
                    .encode_source(short, &resources(), &mut meter())
                    .is_err()
            );
        }
    }
}
#[test]
fn evaluated_effect_sources_reject_every_truncation_bad_tags_utf8_policy_guard_profile_and_trailing_bytes()
 {
    let plan = fixture();
    for effect in effects(&plan) {
        let bytes = encode(effect, &plan);
        for end in 0..bytes.len() {
            assert!(
                PreparedRuntimeEvaluatedEffectSource::preflight(
                    &bytes[..end],
                    limits(&plan),
                    &resources(),
                    &mut meter()
                )
                .is_err(),
                "prefix {end}"
            );
        }
        for (at, byte) in [(0, 0), (DOMAIN.len(), 2), (DOMAIN.len() + 1, 255)] {
            let mut bad = bytes.clone();
            bad[at] = byte;
            assert!(
                PreparedRuntimeEvaluatedEffectSource::preflight(
                    &bad,
                    limits(&plan),
                    &resources(),
                    &mut meter()
                )
                .is_err()
            );
        }
        let row = cursor::Cursor::new(&bytes).row().unwrap();
        match row.kind {
            cursor::Kind::Log(level) => {
                let mut bad = bytes.clone();
                bad[level.as_ptr() as usize - bytes.as_ptr() as usize] = 255;
                assert!(matches!(
                    PreparedRuntimeEvaluatedEffectSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    ),
                    Err(Error::Utf8(_))
                ));
                let at = row.fields.as_ptr() as usize - bytes.as_ptr() as usize - 4;
                let mut huge = bytes.clone();
                huge[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
                assert!(
                    PreparedRuntimeEvaluatedEffectSource::preflight(
                        &huge,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    )
                    .is_err()
                );
            }
            cursor::Kind::Drop(_) => {
                let at = row.first.as_ptr() as usize - bytes.as_ptr() as usize + row.first.len();
                let mut bad = bytes.clone();
                bad[at] = 6;
                assert!(
                    PreparedRuntimeEvaluatedEffectSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    )
                    .is_err()
                );
            }
            cursor::Kind::Assert { message, .. } => {
                let at = DOMAIN.len() + 2;
                let mut bad = bytes.clone();
                bad[at..at + 16].fill(0);
                assert!(matches!(
                    PreparedRuntimeEvaluatedEffectSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    ),
                    Err(Error::Identity(_))
                ));
                let mut bad = bytes.clone();
                bad[at + 16] = 2;
                assert!(
                    PreparedRuntimeEvaluatedEffectSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    )
                    .is_err()
                );
                let mut bad = bytes.clone();
                bad[message.as_ptr() as usize - bytes.as_ptr() as usize] = 255;
                assert!(matches!(
                    PreparedRuntimeEvaluatedEffectSource::preflight(
                        &bad,
                        limits(&plan),
                        &resources(),
                        &mut meter()
                    ),
                    Err(Error::Utf8(_))
                ));
            }
            _ => {}
        }
        let mut bad = bytes;
        bad.push(0);
        assert!(matches!(
            PreparedRuntimeEvaluatedEffectSource::preflight(
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
fn evaluated_effect_sources_late_non_bool_condition_cannot_publish_an_earlier_actual_function() {
    let plan = fixture();
    let (mut source, functions) = source(&plan, meter());
    assert!(replay(&plan, &mut source, &functions, true).is_err());
    assert!(source.builder.function_sites[0].body.is_some());
    assert!(source.builder.function_sites[1].body.is_none());
    assert!(finish(source).is_err());
}
#[test]
fn evaluated_effect_sources_authenticate_the_actual_namespace_before_materializing_a_seed() {
    let plan = fixture();
    let bytes = encode(effects(&plan).next().unwrap(), &plan);
    for kind in ["type", "state"] {
        let (mut source, _) = source(&plan, meter());
        let mut declared = limits(&plan);
        match kind {
            "type" => declared.expression.type_count += 1,
            _ => declared.expression.state_count += 1,
        }
        let prepared = PreparedRuntimeEvaluatedEffectSource::preflight(
            &bytes,
            declared,
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        assert!(matches!(
            prepared.materialize(&mut source, &handles(), &mut resources()),
            Err(Error::Expression(RuntimeExpressionSourceError::Namespace { kind: actual, .. })) if actual == kind
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
}
#[test]
fn evaluated_effect_sources_preserve_the_original_work_bank_and_pre_reserved_output() {
    let plan = fixture();
    let effect = effects(&plan).next().unwrap();
    let bytes = encode(effect, &plan);
    let (trial, _) = source(&plan, meter());
    let used = trial.meter.totals().0;
    let (mut source, _) = source(&plan, TaskSemanticMeter::new(used + 1, u64::MAX));
    assert!(matches!(
        PreparedRuntimeEvaluatedEffectSource::preflight(
            &bytes,
            limits(&plan),
            &resources(),
            &mut source.meter
        ),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert!(finish(source).is_err());
    let encoded = effect
        .encode_source(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(encoded.bytes().len());
    output.push(89);
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [89]);
    let encoded = effect
        .encode_source(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(encoded.bytes().len() + 1);
    output.push(89);
    encoded.append_to(&mut output).unwrap();
    assert_eq!(output[0], 89);
    assert_eq!(output[1..], bytes);
}
