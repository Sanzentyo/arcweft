use super::super::callable_states::tests as graph;
use super::super::expression::RuntimeExpressionSourceHandles;
use super::super::expression::tests::decode;
use super::super::pattern::tests::{meter, resources};
use super::*;
use crate::effect::*;
use crate::entry::{RuntimeNominalRecordShape, RuntimeSchemaLimits};
use crate::line_task::LineOutRequest;
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::program_types::RuntimeProgramTypes;
use crate::time::LogicalDuration;
use crate::value::RuntimeValue;

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn unit() -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        RuntimeCheckedType::Unit.semantic_identity_digest(),
        RuntimeExprSeedKind::Value(RuntimeValue::Unit),
    )
}
fn builder(fields: usize) -> RuntimePlanBuilder {
    let mut builder = crate::plan::type_table::source_codec::domains::tests::builder(
        RuntimeNominalRecordShape::Record,
        fields,
        true,
    );
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                RuntimeCheckedType::Unit.semantic_identity_digest(),
                RuntimePlanTypeProjection::Unit,
            )],
            [],
        )
        .unwrap();
    builder
}
fn fields() -> Vec<RuntimeField> {
    vec![
        RuntimeField {
            name: "same".into(),
            value: "first".into(),
        },
        RuntimeField {
            name: "same".into(),
            value: "第二\0".into(),
        },
    ]
}
fn requests(record: RuntimeValue) -> Vec<LineEffectRequest> {
    use LineEffectRequest as E;
    let mut effects = vec![
        E::Wait(RuntimeWaitTarget::Duration(LogicalDuration::from_nanos(
            u64::MAX,
        ))),
        E::Call(RuntimeCall {
            callee: "flow.original".into(),
            args: vec![],
        }),
        E::Call(RuntimeCall {
            callee: "flow.original".into(),
            args: vec!["first".into(), "第二".into()],
        }),
        E::Log(RuntimeLog {
            level: "debug".into(),
            message: "original".into(),
            fields: vec![],
        }),
        E::Log(RuntimeLog {
            level: "debug".into(),
            message: "original".into(),
            fields: fields(),
        }),
        E::SignalWrite(
            RuntimeAssignment::try_new(
                "signal.original".into(),
                RuntimeValue::String("value".into()),
            )
            .unwrap(),
        ),
        E::SignalWrite(RuntimeAssignment::try_copy("signal.original".into(), &record).unwrap()),
        E::MetricWrite(
            RuntimeAssignment::try_new(
                "metric.original".into(),
                RuntimeValue::Tuple(vec![record.clone(), RuntimeValue::u64(u64::MAX)]),
            )
            .unwrap(),
        ),
        E::MetricWrite(
            RuntimeAssignment::try_new("metric.original".into(), RuntimeValue::option_some(record))
                .unwrap(),
        ),
        E::EmitEvent(RuntimeEvent {
            event: "event.original".into(),
            fields: vec![],
        }),
        E::EmitEvent(RuntimeEvent {
            event: "event.original".into(),
            fields: fields(),
        }),
        E::Out(LineOutRequest {
            label: None,
            value: "out.original".into(),
        }),
        E::Out(LineOutRequest {
            label: Some(String::new()),
            value: "out.original".into(),
        }),
        E::Return("return.original".into()),
        E::Goto("goto.original".into()),
        E::Panic("panic.original".into()),
        E::Fail("fail.original".into()),
        E::Bail("bail.original".into()),
        E::Close("close.original".into()),
        E::Select("select.original".into()),
        E::Ensure {
            condition: "condition.original".into(),
            message: "message.original".into(),
        },
    ];
    for profile in [
        RuntimeAssertionProfile::Always,
        RuntimeAssertionProfile::DebugOnly,
    ] {
        effects.push(E::Assert(RuntimeAssertion::new(
            RuntimeAssertionGuardId::try_from_bytes([7; 16]).unwrap(),
            "condition.original".into(),
            "message.original".into(),
            profile,
        )));
    }
    for label in [None, Some(String::new())] {
        for value in [None, Some("0".into())] {
            effects.push(E::Break {
                label: label.clone(),
                value,
            });
        }
    }
    for label in [None, Some("ループ".into())] {
        effects.push(E::Continue { label });
    }
    effects
}
fn fixture(field_count: usize) -> RuntimePlan {
    let inventory = builder(field_count)
        .prepare_inventory(RuntimeTaskPlanSealLimits::default(), 0)
        .unwrap();
    let record = RuntimeProgramTypes::Plan(&inventory)
        .try_record_value(
            semantic(1),
            vec![RuntimeValue::Bool(true); field_count],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
    let mut builder = builder(field_count);
    let prefix = graph::reserve_executable_function(
        &mut builder,
        244,
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
        245,
        &[],
        0,
        RuntimeCheckedType::Unit.semantic_identity_digest(),
    );
    builder
        .define_function_site_seed(
            &root,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: requests(record)
                    .into_iter()
                    .map(|effect| RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Static(effect)))
                    .chain([RuntimeFlowOpSeed::ReturnExpr(unit())])
                    .collect(),
            }),
        )
        .unwrap();
    builder.finish().unwrap()
}
fn effects(plan: &RuntimePlan) -> impl Iterator<Item = &LineEffectRequest> {
    plan.function_sites()
        .iter()
        .flat_map(|function| match function.body() {
            RuntimeFunctionSiteBody::Executable(body) => body.ops(),
            _ => &[],
        })
        .filter_map(|op| match op {
            FlowOp::Effect(effect) => Some(effect),
            _ => None,
        })
}
fn limits() -> RuntimeStaticEffectSourceLimits {
    RuntimeStaticEffectSourceLimits {
        nodes: 100_000,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        literal: RuntimeLiteralSourceLimits {
            encoded_bytes: 1 << 24,
            decoded_bytes: 1 << 28,
            elements: 1_000_000,
        },
    }
}
fn encode(effect: &LineEffectRequest) -> Vec<u8> {
    effect
        .encode_static_source(limits(), &resources(), &mut meter())
        .unwrap()
        .into_bytes()
}
fn restore_source(
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
    let limits = RuntimeTaskPlanSealLimits::default();
    Ok(source
        .prepare_image(limits)?
        .seal(limits, None)?
        .into_plan())
}
fn define(
    plan: &RuntimePlan,
    source: &mut RuntimePlanSourceConstruction,
    handle: &RuntimeFunctionSiteSeedId,
    function: &RuntimeFunctionSite,
) {
    let RuntimeFunctionSiteBody::Executable(body) = function.body() else {
        unreachable!()
    };
    let handles = RuntimeExpressionSourceHandles {
        states: &[],
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &[],
    };
    let ops = body
        .ops()
        .iter()
        .map(|op| match op {
            FlowOp::Noop => RuntimeFlowOpSeed::Noop,
            FlowOp::ReturnExpr(expr) => {
                RuntimeFlowOpSeed::ReturnExpr(decode(expr, plan, source, &handles))
            }
            FlowOp::Effect(effect) => {
                let bytes = encode(effect);
                let prepared = PreparedRuntimeStaticEffectSource::preflight(
                    &bytes,
                    limits(),
                    &resources(),
                    &mut source.meter,
                )
                .unwrap();
                RuntimeFlowOpSeed::Effect(prepared.materialize(source, &mut resources()).unwrap())
            }
            _ => panic!("static source fixture owns exact Effect/Noop/Return operations"),
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

#[test]
fn static_effect_sources_restore_all_eighteen_kinds_optional_roles_and_full_original_semantics() {
    let plan = fixture(2);
    assert_eq!(effects(&plan).count(), 29);
    let (mut source, functions) = restore_source(&plan, meter());
    for (function, handle) in plan.function_sites().iter().zip(&functions) {
        define(&plan, &mut source, handle, function);
    }
    let restored = finish(source).unwrap();
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    for (original, restored) in effects(&plan).zip(effects(&restored)) {
        assert_eq!(encode(original), encode(restored));
    }
}
#[test]
fn static_effect_sources_share_exact_and_one_less_aggregate_limits_for_every_kind() {
    let plan = fixture(2);
    for effect in effects(&plan) {
        let encoded = effect
            .encode_static_source(limits(), &resources(), &mut meter())
            .unwrap();
        let bytes = encoded.bytes();
        let prepared = PreparedRuntimeStaticEffectSource::preflight(
            bytes,
            limits(),
            &resources(),
            &mut meter(),
        )
        .unwrap();
        assert_eq!(
            (
                prepared.node_count(),
                prepared.elements(),
                prepared.storage_bytes()
            ),
            (
                encoded.node_count(),
                encoded.elements(),
                encoded.storage_bytes()
            )
        );
        let exact = RuntimeStaticEffectSourceLimits {
            nodes: prepared.node_count() as u32,
            elements: prepared.elements() as u32,
            encoded_bytes: bytes.len(),
            decoded_bytes: prepared.storage_bytes(),
            ..limits()
        };
        PreparedRuntimeStaticEffectSource::preflight(bytes, exact, &resources(), &mut meter())
            .unwrap();
        effect
            .encode_static_source(exact, &resources(), &mut meter())
            .unwrap();
        for short in [
            RuntimeStaticEffectSourceLimits {
                nodes: exact.nodes - 1,
                ..exact
            },
            RuntimeStaticEffectSourceLimits {
                elements: exact.elements - 1,
                ..exact
            },
            RuntimeStaticEffectSourceLimits {
                encoded_bytes: exact.encoded_bytes - 1,
                ..exact
            },
            RuntimeStaticEffectSourceLimits {
                decoded_bytes: exact.decoded_bytes - 1,
                ..exact
            },
        ] {
            let mut bank = meter();
            assert!(
                PreparedRuntimeStaticEffectSource::preflight(bytes, short, &resources(), &mut bank)
                    .is_err()
            );
            assert!(bank.status().is_err());
            assert!(
                effect
                    .encode_static_source(short, &resources(), &mut meter())
                    .is_err()
            );
        }
    }
}
#[test]
fn static_effect_sources_reject_every_truncation_and_malformed_metadata_before_retention() {
    let plan = fixture(2);
    for effect in effects(&plan) {
        let bytes = encode(effect);
        for end in 0..bytes.len() {
            assert!(
                PreparedRuntimeStaticEffectSource::preflight(
                    &bytes[..end],
                    limits(),
                    &resources(),
                    &mut meter()
                )
                .is_err()
            );
        }
        let mut mutations = Vec::new();
        for (offset, value) in [
            (0, 255),
            (DOMAIN.len(), 2),
            (DOMAIN.len() + 1, 255),
            (DOMAIN.len() + 1, 1),
        ] {
            let mut bad = bytes.clone();
            bad[offset] = value;
            mutations.push(bad);
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        mutations.push(trailing);
        let at = DOMAIN.len() + 2;
        match effect {
            LineEffectRequest::Out(_)
            | LineEffectRequest::Break { .. }
            | LineEffectRequest::Continue { .. } => {
                let mut bad = bytes.clone();
                bad[at] = 2;
                mutations.push(bad);
            }
            LineEffectRequest::Assert(_) => {
                let mut bad = bytes.clone();
                bad[at..at + 16].fill(0);
                mutations.push(bad);
                let mut bad = bytes.clone();
                bad[at + 16] = 2;
                mutations.push(bad);
            }
            LineEffectRequest::Call(call) => {
                let mut bad = bytes.clone();
                bad[at + 4] = 255;
                mutations.push(bad);
                let mut bad = bytes.clone();
                let count = at + 4 + call.callee.len();
                bad[count..count + 4].fill(255);
                mutations.push(bad);
            }
            LineEffectRequest::Log(log) => {
                let mut bad = bytes.clone();
                let count = at + 8 + log.level.len() + log.message.len();
                bad[count..count + 4].fill(255);
                mutations.push(bad);
            }
            LineEffectRequest::EmitEvent(event) => {
                let mut bad = bytes.clone();
                let count = at + 4 + event.event.len();
                bad[count..count + 4].fill(255);
                mutations.push(bad);
            }
            _ => {}
        }
        for bad in mutations {
            assert!(
                PreparedRuntimeStaticEffectSource::preflight(
                    &bad,
                    limits(),
                    &resources(),
                    &mut meter()
                )
                .is_err()
            );
        }
    }
}
#[test]
fn static_effect_sources_use_original_domains_and_cannot_publish_a_retained_prefix_on_bad_value() {
    let plan = fixture(2);
    let nominal = effects(&plan)
        .find_map(|effect| match effect {
            LineEffectRequest::SignalWrite(value) => match value.value() {
                RuntimeValue::NominalRecord(record) => Some(record),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    let bad = nominal
        .clone()
        .try_map_fields(|_| {
            Ok::<_, std::convert::Infallible>(RuntimeValue::String("wrong bool".into()))
        })
        .unwrap_or_else(|never| match never {});
    let bad = LineEffectRequest::SignalWrite(
        RuntimeAssignment::try_new("signal.original".into(), RuntimeValue::NominalRecord(bad))
            .unwrap(),
    );
    let bytes = encode(&bad);
    let (mut source, functions) = restore_source(&plan, meter());
    define(
        &plan,
        &mut source,
        &functions[0],
        plan.function_sites().iter().next().unwrap(),
    );
    let prepared = PreparedRuntimeStaticEffectSource::preflight(
        &bytes,
        limits(),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &mut resources()),
        Err(Error::Value(_))
    ));
    assert!(source.builder.function_sites[0].body.is_some());
    assert!(source.builder.function_sites[1].body.is_none());
    assert!(finish(source).is_err());
    let different = fixture(1);
    let (mut source, _) = restore_source(&different, meter());
    let bytes = encode(effects(&plan).find(|effect| matches!(effect, LineEffectRequest::SignalWrite(value) if matches!(value.value(), RuntimeValue::NominalRecord(_)))).unwrap());
    let prepared = PreparedRuntimeStaticEffectSource::preflight(
        &bytes,
        limits(),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &mut resources()),
        Err(Error::Value(_))
    ));
    assert!(
        source
            .builder
            .function_sites
            .iter()
            .all(|function| function.body.is_none())
    );
    assert!(finish(source).is_err());
}
#[test]
fn static_effect_sources_reject_raw_audio_on_both_source_boundaries() {
    let plan = fixture(2);
    let expression = plan
        .function_sites()
        .iter()
        .find_map(|function| match function.body() {
            RuntimeFunctionSiteBody::Executable(body) => body.ops().iter().find_map(|op| {
                if let FlowOp::ReturnExpr(expr) = op {
                    Some(expr.clone())
                } else {
                    None
                }
            }),
            _ => None,
        })
        .unwrap();
    let raw = LineEffectRequest::Audio(Box::new(
        crate::audio::RuntimeAudioCommand::StopMicrophone {
            capture: expression,
        },
    ));
    let mut bank = meter();
    assert!(matches!(
        raw.encode_static_source(limits(), &resources(), &mut bank),
        Err(Error::Invalid {
            kind: "raw Audio expression carrier",
            ..
        })
    ));
    assert!(bank.status().is_err());
    let mut bytes = DOMAIN.to_vec();
    bytes.extend([1, 1]);
    assert!(matches!(
        PreparedRuntimeStaticEffectSource::preflight(&bytes, limits(), &resources(), &mut meter()),
        Err(Error::Invalid {
            kind: "raw Audio expression carrier",
            ..
        })
    ));
}
#[test]
fn static_effect_sources_preserve_output_prefixes_and_use_the_finite_original_bank() {
    let plan = fixture(2);
    for effect in effects(&plan) {
        let bytes = encode(effect);
        let mut too_small = vec![7];
        too_small.shrink_to_fit();
        let before = too_small.clone();
        assert!(matches!(
            effect
                .encode_static_source(limits(), &resources(), &mut meter())
                .unwrap()
                .append_to(&mut too_small),
            Err(Error::OutputCapacity { .. })
        ));
        assert_eq!(too_small, before);
        let mut output = Vec::with_capacity(bytes.len() + 1);
        output.push(7);
        effect
            .encode_static_source(limits(), &resources(), &mut meter())
            .unwrap()
            .append_to(&mut output)
            .unwrap();
        assert_eq!(&output[1..], bytes);
        assert_eq!(output[0], 7);
    }
    let effect = effects(&plan).next().unwrap();
    let bytes = encode(effect);
    let (mut source, _) = restore_source(&plan, TaskSemanticMeter::new(1_000_000, 1_000_000));
    let used = source.meter.totals().0;
    assert!(used > 0 && used < 1_000_000);
    // The declaration prelude and request share this exact finite allowance.
    source.meter.charge_work(1_000_000 - used).unwrap();
    assert_eq!(source.meter.totals().0, 1_000_000);
    assert!(
        PreparedRuntimeStaticEffectSource::preflight(
            &bytes,
            limits(),
            &resources(),
            &mut source.meter
        )
        .is_err()
    );
    assert!(finish(source).is_err());
}

#[test]
fn static_effect_source_rows_share_the_original_parent_value_allowance() {
    let plan = fixture(2);
    let effect = effects(&plan).find(|effect| matches!(effect, LineEffectRequest::SignalWrite(value) if matches!(value.value(), RuntimeValue::NominalRecord(_)))).unwrap();
    let bytes = encode(effect);
    let (mut source, _) = restore_source(&plan, meter());
    let mut resources = ValueResourceAdmission::snapshot(
        RuntimeSchemaLimits {
            max_nodes: 3,
            ..RuntimeSchemaLimits::engine_default()
        },
        crate::entry::schema::value_encoding::ValueAdmissionOrigin::InertSnapshot,
    );
    let prepared = PreparedRuntimeStaticEffectSource::preflight(
        &bytes,
        limits(),
        &resources,
        &mut source.meter,
    )
    .unwrap();
    prepared.materialize(&mut source, &mut resources).unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &mut resources),
        Err(Error::Value(_))
    ));
    assert!(resources.work().failure().is_some());
    assert!(source.meter.status().is_err());
    assert!(finish(source).is_err());
}
