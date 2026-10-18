use super::super::expression::RuntimeExpressionSourceHandles;
use super::super::pattern::tests::{declarations, frames, meter, resources};
use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::*;
use crate::stream::StreamRuntimeId;
use crate::value::*;

const BOOL: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([201; 32]);
const UNIT: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([202; 32]);
const STREAM: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([203; 32]);
fn boolean(value: bool) -> RuntimeExprSeed {
    RuntimeExprSeed::new(BOOL, RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)))
}
fn local(id: &RuntimeLocalSeedId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        BOOL,
        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
            id.clone(),
            RuntimeLocalReadMode::Copy,
        )),
    )
}
fn bind(id: &RuntimeLocalSeedId) -> RuntimePatternSeed {
    RuntimePatternSeed::new(
        BOOL,
        RuntimePatternSeedKind::Bind {
            mutable: false,
            local: id.clone(),
        },
    )
}
fn upstream() -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        STREAM,
        RuntimeExprSeedKind::Call {
            callee: RuntimeCallTarget::External(RuntimeExternalCallable::new(
                crate::entry::RuntimeCallableId::from_checked_digest([204; 32]),
                RuntimeExternalCallableDeclaration::try_new(
                    RuntimeExternalCallableProvider::Adapter("fixture.stream".into()),
                    RuntimeExternalCallableExport::Function(Box::new(["upstream".into()])),
                    0,
                )
                .unwrap(),
            )),
            args: Box::new([]),
        },
    )
}
fn fixture() -> RuntimePlan {
    use RuntimeStreamOpSeed as Op;
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(BOOL, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(UNIT, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    STREAM,
                    RuntimePlanTypeProjection::Stream {
                        item: BOOL,
                        error: UNIT,
                    },
                ),
            ],
            (0..3).map(|i| {
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Binding {
                        identity: [210 + i; 32],
                        declaration: RuntimeLocalBindingDeclaration::new(
                            RuntimeLocalBindingKind::PatternBinding,
                            false,
                            RuntimeLocalBindingStorage::Derived,
                        ),
                    },
                    BOOL,
                )
            }),
        )
        .unwrap();
    let locals = admitted.local_ids();
    let ops = vec![
        Op::Let {
            pattern: bind(&locals[0]),
            expr: boolean(true),
        },
        Op::If {
            condition: local(&locals[0]),
            then_ops: vec![],
            else_ops: vec![Op::Yield {
                expr: boolean(false),
            }],
        },
        Op::Match {
            scrutinee: local(&locals[0]),
            arms: vec![
                RuntimeStreamMatchArmSeed {
                    pattern: bind(&locals[1]),
                    guard: Some(local(&locals[1])),
                    ops: vec![Op::Yield {
                        expr: local(&locals[1]),
                    }],
                },
                RuntimeStreamMatchArmSeed {
                    pattern: RuntimePatternSeed::new(BOOL, RuntimePatternSeedKind::Discard),
                    guard: None,
                    ops: vec![],
                },
            ],
        },
        Op::ForNext {
            pattern: bind(&locals[2]),
            source: upstream(),
            body: vec![Op::If {
                condition: local(&locals[2]),
                then_ops: vec![Op::Yield {
                    expr: local(&locals[2]),
                }],
                else_ops: vec![],
            }],
        },
        Op::Close { source: upstream() },
        Op::Return,
    ];
    builder
        .push_stream_plan_seed(RuntimeStreamPlanSeed {
            id: StreamRuntimeId::canonical("alpha").unwrap(),
            item_ty: BOOL,
            error_ty: UNIT,
            ops,
        })
        .unwrap();
    builder
        .push_stream_plan_seed(RuntimeStreamPlanSeed {
            id: StreamRuntimeId::from_source_entity_body("stream.bravo").unwrap(),
            item_ty: BOOL,
            error_ty: UNIT,
            ops: vec![Op::Return],
        })
        .unwrap();
    builder.finish().unwrap()
}
fn limits(plan: &RuntimePlan) -> RuntimeStreamSourceLimits {
    RuntimeStreamSourceLimits {
        rows: 100,
        nodes: 10_000,
        depth: 128,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        expression: RuntimeExpressionSourceLimits {
            nodes: 10_000,
            depth: 128,
            elements: 100_000,
            encoded_bytes: 1 << 22,
            decoded_bytes: 1 << 24,
            type_count: plan.type_table().len(),
            local_count: plan.local_declarations().len(),
            state_count: 0,
            specialization_count: 0,
            helper_count: 0,
            method_count: 0,
            template_count: 0,
            attempt_count: 0,
        },
    }
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
    mut bank: TaskSemanticMeter,
) -> super::super::RuntimePlanSourceConstruction {
    let bytes = frames(plan);
    declarations(&bytes, &mut bank)
        .into_construction(bank)
        .unwrap()
}
fn encode(plan: &RuntimePlan) -> Vec<u8> {
    plan.encode_stream_sources(limits(plan), &resources(), &mut meter())
        .unwrap()
        .into_bytes()
}
fn finish(source: super::super::RuntimePlanSourceConstruction) -> RuntimePlan {
    let seal = RuntimeTaskPlanSealLimits::default();
    source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan()
}
#[test]
fn stream_sources_restore_all_operations_bindings_guards_empty_branches_and_original_identity_then_seal()
 {
    let plan = fixture();
    let bytes = encode(&plan);
    let encoded = plan
        .encode_stream_sources(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let mut source = source(&plan, meter());
    let prepared = PreparedRuntimeStreamSources::preflight(
        &bytes,
        limits(&plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    assert_eq!(prepared.elements(), encoded.elements());
    assert_eq!(prepared.node_count(), encoded.node_count());
    prepared
        .materialize(&mut source, &handles(), &mut resources())
        .unwrap();
    let restored = finish(source);
    assert_eq!(restored.stream_plans(), plan.stream_plans());
    assert_eq!(restored.local_declarations(), plan.local_declarations());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(encode(&restored), bytes);
}
#[test]
fn stream_sources_share_all_aggregate_caps_and_accept_exact_then_reject_one_less() {
    let plan = fixture();
    let bytes = encode(&plan);
    let prepared =
        PreparedRuntimeStreamSources::preflight(&bytes, limits(&plan), &resources(), &mut meter())
            .unwrap();
    let exact = RuntimeStreamSourceLimits {
        rows: 2,
        nodes: prepared.node_count() as u32,
        elements: prepared.elements() as u32,
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        ..limits(&plan)
    };
    PreparedRuntimeStreamSources::preflight(&bytes, exact, &resources(), &mut meter()).unwrap();
    plan.encode_stream_sources(exact, &resources(), &mut meter())
        .unwrap();
    for short in [
        RuntimeStreamSourceLimits { rows: 1, ..exact },
        RuntimeStreamSourceLimits {
            nodes: exact.nodes - 1,
            ..exact
        },
        RuntimeStreamSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeStreamSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeStreamSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
        RuntimeStreamSourceLimits { depth: 0, ..exact },
    ] {
        let mut bank = meter();
        assert!(
            PreparedRuntimeStreamSources::preflight(&bytes, short, &resources(), &mut bank)
                .is_err()
        );
        assert!(bank.status().is_err());
        assert!(
            plan.encode_stream_sources(short, &resources(), &mut meter())
                .is_err()
        );
    }
}
#[test]
fn stream_sources_reject_every_truncated_prefix_and_malformed_domain_version_refs_tags_children() {
    let plan = fixture();
    let bytes = encode(&plan);
    for end in 0..bytes.len() {
        assert!(
            PreparedRuntimeStreamSources::preflight(
                &bytes[..end],
                limits(&plan),
                &resources(),
                &mut meter()
            )
            .is_err(),
            "prefix {end}"
        );
    }
    let item = DOMAIN.len() + 1 + 4 + 4 + 4 + "alpha".len();
    for (offset, replacement) in [
        (0, 0),
        (DOMAIN.len(), 2),
        (item, 0),
        (bytes.len() - 9, 255),
        (bytes.len() - 4, 1),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = replacement;
        assert!(
            PreparedRuntimeStreamSources::preflight(
                &bad,
                limits(&plan),
                &resources(),
                &mut meter()
            )
            .is_err(),
            "offset {offset}"
        );
    }
    let mut bad = bytes.clone();
    bad.push(0);
    assert!(matches!(
        PreparedRuntimeStreamSources::preflight(&bad, limits(&plan), &resources(), &mut meter()),
        Err(Error::Trailing { .. })
    ));
    let mut bad = bytes;
    bad[item + 12..item + 16].copy_from_slice(&u32::MAX.to_le_bytes());
    let generous = RuntimeStreamSourceLimits {
        nodes: u32::MAX,
        decoded_bytes: usize::MAX,
        ..limits(&plan)
    };
    assert!(matches!(
        PreparedRuntimeStreamSources::preflight(&bad, generous, &resources(), &mut meter()),
        Err(Error::Invalid {
            kind: "operation prefix size",
            ..
        })
    ));
}
#[test]
fn stream_sources_late_failure_cannot_publish_earlier_valid_streams() {
    let plan = fixture();
    let mut bytes = encode(&plan);
    let start = bytes.windows(5).rposition(|text| text == b"bravo").unwrap();
    bytes[start..start + 5].copy_from_slice(b"alpha");
    let mut source = source(&plan, meter());
    let prepared = PreparedRuntimeStreamSources::preflight(
        &bytes,
        limits(&plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &handles(), &mut resources()),
        Err(Error::Construction(
            RuntimePlanBuildError::DuplicateStreamDefinition { .. }
        ))
    ));
    assert_eq!(source.builder.stream_plans.len(), 1);
    assert!(source.meter.status().is_err());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}
#[test]
fn stream_sources_authenticate_the_original_namespace_before_retaining_operations() {
    let plan = fixture();
    let bytes = encode(&plan);
    let mut source = source(&plan, meter());
    let wrong = RuntimeStreamSourceLimits {
        expression: RuntimeExpressionSourceLimits {
            state_count: 1,
            ..limits(&plan).expression
        },
        ..limits(&plan)
    };
    let prepared =
        PreparedRuntimeStreamSources::preflight(&bytes, wrong, &resources(), &mut source.meter)
            .unwrap();
    assert!(matches!(
        prepared.materialize(&mut source, &handles(), &mut resources()),
        Err(Error::Expression(RuntimeExpressionSourceError::Namespace {
            kind: "state",
            ..
        }))
    ));
    assert!(source.builder.stream_plans.is_empty());
    assert!(source.meter.status().is_err());
}
#[test]
fn stream_sources_keep_the_original_work_bank_and_reserved_output_prefix() {
    let plan = fixture();
    let bytes = encode(&plan);
    let trial = source(&plan, meter());
    let used = trial.meter.totals().0;
    let mut source = source(&plan, TaskSemanticMeter::new(used + 1, u64::MAX));
    assert!(matches!(
        PreparedRuntimeStreamSources::preflight(
            &bytes,
            limits(&plan),
            &resources(),
            &mut source.meter
        ),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
    let encoded = plan
        .encode_stream_sources(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(encoded.bytes().len());
    output.push(17);
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [17]);
}
#[test]
fn empty_stream_inventory_round_trips_without_inventing_an_executable_owner() {
    let plan = RuntimePlanBuilder::new().finish().unwrap();
    let bytes = encode(&plan);
    let exact = RuntimeStreamSourceLimits {
        rows: 0,
        nodes: 0,
        elements: 0,
        decoded_bytes: 0,
        encoded_bytes: bytes.len(),
        ..limits(&plan)
    };
    let mut source = source(&plan, meter());
    PreparedRuntimeStreamSources::preflight(&bytes, exact, &resources(), &mut source.meter)
        .unwrap()
        .materialize(&mut source, &handles(), &mut resources())
        .unwrap();
    assert_eq!(finish(source), plan);
}
