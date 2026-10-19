use super::super::callable_states::tests::reserve_executable_function;
use super::super::expression::tests::limits as expression_limits;
use super::super::functions::RuntimeFunctionSourceLimits;
use super::super::pattern::tests::{declarations, frames, meter, resources};
use super::*;
use crate::engine::Engine;
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions};
use crate::value::{RuntimeSignedIntWidth, RuntimeValue};
use arcweft_id::runtime_program::RuntimePureProgramId;
use std::sync::Arc;

fn boolean() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Bool.semantic_identity_digest()
}
fn integer() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest()
}
fn never() -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([0xe1; 32])
}
fn program() -> RuntimePureProgramId {
    RuntimePureProgramId::from_checked_digest([0xe2; 32])
}
fn value(number: i64) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        integer(),
        RuntimeExprSeedKind::Value(RuntimeValue::i64(number)),
    )
}
fn bool_value(value: bool) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        boolean(),
        RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)),
    )
}
fn empty_expression_handles() -> RuntimeExpressionSourceHandles<'static> {
    RuntimeExpressionSourceHandles {
        states: &[],
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &[],
    }
}
pub(in crate::plan::construction::source) fn limits(plan: &RuntimePlan) -> RuntimeFlowSourceLimits {
    RuntimeFlowSourceLimits {
        nodes: 100_000,
        depth: 128,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        task_count: plan.task_plans().len(),
        content_count: plan.dialogue_content().rows().len(),
        defer_count: plan.defer_sites().len(),
        project_count: plan.project_call_sites().len(),
        expression: expression_limits(plan),
    }
}
fn fixture(condition: Option<bool>) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(boolean(), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    integer(),
                    RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
                ),
                RuntimePlanTypeSeed::new(never(), RuntimePlanTypeProjection::Never),
            ],
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalDeclarationSource::Binding {
                    identity: [0xe3; 32],
                    declaration: RuntimeLocalBindingDeclaration::new(
                        RuntimeLocalBindingKind::PatternBinding,
                        false,
                        RuntimeLocalBindingStorage::Derived,
                    ),
                },
                boolean(),
            )],
        )
        .unwrap();
    let root = reserve_executable_function(&mut builder, 0xe4, &[], 0, integer());
    let discard = || RuntimePatternSeed::new(boolean(), RuntimePatternSeedKind::Discard);
    let arms = vec![
        RuntimeFlowMatchArmSeed {
            pattern: discard(),
            guard: Some(RuntimeFlowMatchGuardSeed {
                candidate: admitted.local_ids()[0].clone(),
                result: if condition.is_some() {
                    boolean()
                } else {
                    never()
                },
                condition: condition.map(bool_value),
                copy_locals: Box::new([]),
                ops: if condition.is_none() {
                    vec![RuntimeFlowOpSeed::ReturnExpr(value(7))]
                } else {
                    vec![]
                },
            }),
            ops: vec![RuntimeFlowOpSeed::ReturnExpr(value(42))],
        },
        RuntimeFlowMatchArmSeed {
            pattern: discard(),
            guard: None,
            ops: vec![RuntimeFlowOpSeed::ReturnExpr(value(100))],
        },
    ];
    builder
        .define_function_site_seed(
            &root,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::Scope {
                    identity: crate::scope::RuntimeScopeIdentity::Named(
                        arcweft_id::DeclarationName::try_new("flow_source").unwrap(),
                    ),
                    body: vec![RuntimeFlowOpSeed::If {
                        condition: bool_value(true),
                        then_ops: vec![RuntimeFlowOpSeed::Match {
                            scrutinee: bool_value(true),
                            arms,
                        }],
                        else_ops: vec![RuntimeFlowOpSeed::ReturnExpr(value(-1))],
                    }],
                }]),
            }),
        )
        .unwrap();
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program: program(),
            site: root,
        })
        .unwrap();
    builder.finish().unwrap()
}
fn ops(plan: &RuntimePlan) -> &[FlowOp] {
    plan.function_sites()
        .iter()
        .next()
        .unwrap()
        .body()
        .executable()
        .unwrap()
        .ops()
}
fn source(
    plan: &RuntimePlan,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
) {
    let mut bank = meter();
    let frames = frames(plan);
    let declarations = declarations(&frames, &mut bank);
    let bounds = RuntimeFunctionSourceLimits {
        rows: 100,
        elements: 1_000_000,
        pattern_nodes: 100_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        type_count: plan.type_table().len(),
        local_count: plan.local_declarations().len(),
    };
    let encoded = plan
        .function_sites()
        .prepare_declaration_sources_encoding(bounds, &resources(), &mut bank)
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
    encoded.append_to(&mut bytes).unwrap();
    let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        bounds,
        &resources(),
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let functions = prepared.reserve(&mut source, &mut resources()).unwrap();
    (source, functions)
}
fn run(plan: RuntimePlan) -> RuntimeValue {
    let mut engine = Engine::for_program_invocation(Arc::new(plan), program(), vec![]).unwrap();
    let report = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 64 },
            ..RuntimeStepOptions::default()
        },
    );
    assert!(
        report.output.diagnostics.is_empty(),
        "{:?}",
        report.output.diagnostics
    );
    let (id, result) = engine
        .take_program_result()
        .unwrap()
        .expect("actual function completed");
    assert_eq!(id, program());
    result
}

#[test]
fn flow_source_preserves_nested_guard_bodies_never_results_and_actual_native_execution() {
    for (condition, expected) in [(None, 7), (Some(true), 42), (Some(false), 100)] {
        let plan = fixture(condition);
        let bounds = limits(&plan);
        let encoded = plan
            .encode_flow_source(ops(&plan), bounds, &resources(), &mut meter())
            .unwrap();
        let (mut source, functions) = source(&plan);
        let prepared = PreparedRuntimeFlowSource::preflight(
            encoded.bytes(),
            bounds,
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        assert_eq!(prepared.node_count(), encoded.node_count());
        assert_eq!(prepared.elements(), encoded.elements());
        assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
        let expressions = empty_expression_handles();
        let handles = RuntimeFlowSourceHandles {
            expression: &expressions,
            contents: &[],
            defers: &[],
            tasks: &[],
        };
        let seeds = prepared
            .materialize(&mut source, &handles, &mut resources())
            .unwrap();
        source
            .builder
            .define_function_site_seed(
                &functions[0],
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: seeds.into_boxed_slice(),
                }),
            )
            .unwrap();
        source
            .builder
            .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
                program: program(),
                site: functions[0].clone(),
            })
            .unwrap();
        let seal = RuntimeTaskPlanSealLimits::default();
        let restored = source
            .prepare_image(seal)
            .unwrap()
            .seal(seal, None)
            .unwrap()
            .into_plan();
        assert_eq!(plan.function_sites(), restored.function_sites());
        assert_eq!(
            plan.executable_semantic_digest(),
            restored.executable_semantic_digest()
        );
        assert_eq!(
            restored
                .encode_flow_source(ops(&restored), bounds, &resources(), &mut meter())
                .unwrap()
                .bytes(),
            encoded.bytes()
        );
        assert_eq!(run(plan), RuntimeValue::i64(expected));
        assert_eq!(run(restored), RuntimeValue::i64(expected));
    }
}

#[test]
fn flow_source_checks_aggregate_quotas_and_preserves_terminal_resource_failure() {
    let plan = fixture(Some(true));
    let bounds = limits(&plan);
    let encoded = plan
        .encode_flow_source(ops(&plan), bounds, &resources(), &mut meter())
        .unwrap();
    let exact = RuntimeFlowSourceLimits {
        nodes: u32::try_from(encoded.node_count()).unwrap(),
        elements: u32::try_from(encoded.elements()).unwrap(),
        encoded_bytes: encoded.bytes().len(),
        decoded_bytes: encoded.storage_bytes(),
        ..bounds
    };
    PreparedRuntimeFlowSource::preflight(encoded.bytes(), exact, &resources(), &mut meter())
        .unwrap();
    let (mut source, _) = source(&plan);
    let prepared = PreparedRuntimeFlowSource::preflight(
        encoded.bytes(),
        exact,
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    let expressions = empty_expression_handles();
    let handles = RuntimeFlowSourceHandles {
        expression: &expressions,
        contents: &[],
        defers: &[],
        tasks: &[],
    };
    assert!(
        !prepared
            .materialize(&mut source, &handles, &mut resources())
            .unwrap()
            .is_empty()
    );
    for bounds in [
        RuntimeFlowSourceLimits {
            nodes: exact.nodes - 1,
            ..exact
        },
        RuntimeFlowSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeFlowSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeFlowSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
        RuntimeFlowSourceLimits { depth: 2, ..exact },
    ] {
        let mut bank = meter();
        assert!(
            PreparedRuntimeFlowSource::preflight(encoded.bytes(), bounds, &resources(), &mut bank)
                .is_err()
        );
        assert!(bank.status().is_err());
        assert!(
            PreparedRuntimeFlowSource::preflight(encoded.bytes(), exact, &resources(), &mut bank)
                .is_err()
        );
    }
    let mut bank = TaskSemanticMeter::new(0, 0);
    let first =
        PreparedRuntimeFlowSource::preflight(encoded.bytes(), exact, &resources(), &mut bank)
            .err()
            .unwrap();
    let second =
        PreparedRuntimeFlowSource::preflight(encoded.bytes(), exact, &resources(), &mut bank)
            .err()
            .unwrap();
    assert_eq!(first, second);
}

#[test]
fn flow_source_rejects_truncation_bad_prefixes_continuations_and_insufficient_output() {
    let plan = fixture(Some(true));
    let bounds = limits(&plan);
    let encoded = plan
        .encode_flow_source(ops(&plan), bounds, &resources(), &mut meter())
        .unwrap();
    for end in 0..encoded.bytes().len() {
        assert!(
            PreparedRuntimeFlowSource::preflight(
                &encoded.bytes()[..end],
                bounds,
                &resources(),
                &mut meter()
            )
            .is_err(),
            "accepted truncation {end}"
        );
    }
    let header = DOMAIN.len() + 1;
    for (offset, replacement) in [
        (0, 0xff),
        (DOMAIN.len(), 2),
        (header, 0),
        (header + 4, 0),
        (header + 8, 0),
        (header + 8, 0xff),
    ] {
        let mut bad = encoded.bytes().to_vec();
        bad[offset] = replacement;
        assert!(
            PreparedRuntimeFlowSource::preflight(&bad, bounds, &resources(), &mut meter()).is_err(),
            "accepted mutation {offset}:{replacement}"
        );
    }
    let mut trailing = encoded.bytes().to_vec();
    trailing.push(0);
    assert!(matches!(
        PreparedRuntimeFlowSource::preflight(&trailing, bounds, &resources(), &mut meter()),
        Err(Error::Trailing { .. })
    ));
    for tag in [0, 3, 21, 23, 25, 27, 30, 44, 47, 48] {
        let mut bytes = DOMAIN.to_vec();
        bytes.push(1);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.push(tag);
        bytes.extend_from_slice(&[0; 12]);
        assert!(matches!(
            PreparedRuntimeFlowSource::preflight(&bytes, bounds, &resources(), &mut meter()),
            Err(Error::Continuation)
        ));
    }
    let mut output = Vec::new();
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert!(output.is_empty());
}

#[test]
fn flow_source_authenticates_complete_task_namespace_before_materialization() {
    let plan = fixture(Some(true));
    let mut bounds = limits(&plan);
    bounds.task_count = 1;
    let mut bytes = DOMAIN.to_vec();
    bytes.push(1);
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    for foreign in [false, true] {
        let (mut source, _) = source(&plan);
        let own = source.builder.reserve_task_plan().unwrap();
        let other = RuntimePlanBuilder::new().reserve_task_plan().unwrap();
        let coordinates = [if foreign { other } else { own }];
        let expressions = empty_expression_handles();
        let handles = RuntimeFlowSourceHandles {
            expression: &expressions,
            contents: &[],
            defers: &[],
            tasks: &coordinates,
        };
        let prepared =
            PreparedRuntimeFlowSource::preflight(&bytes, bounds, &resources(), &mut source.meter)
                .unwrap();
        let result = prepared.materialize(&mut source, &handles, &mut resources());
        if foreign {
            assert!(matches!(result, Err(Error::Handle { kind: "task" })));
            assert!(source.meter.status().is_err());
        } else {
            assert!(result.unwrap().is_empty());
        }
    }
}

#[test]
fn flow_source_preflights_deep_prefixes_iteratively_and_retains_zero_based_line_sites() {
    let plan = fixture(Some(true));
    let bounds = limits(&plan);
    let depth = 4097u32;
    let mut bytes = DOMAIN.to_vec();
    bytes.push(1);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&depth.to_le_bytes());
    for _ in 1..depth {
        bytes.push(29); // anonymous source Scope, exactly one child root
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
    }
    bytes.push(46);
    bytes.extend_from_slice(&[0; 12]);
    let deep = RuntimeFlowSourceLimits { depth, ..bounds };
    PreparedRuntimeFlowSource::preflight(&bytes, deep, &resources(), &mut meter()).unwrap();
    assert!(matches!(
        PreparedRuntimeFlowSource::preflight(
            &bytes,
            RuntimeFlowSourceLimits {
                depth: depth - 1,
                ..deep
            },
            &resources(),
            &mut meter()
        ),
        Err(Error::Limit { kind: "depth", .. })
    ));

    let mut bytes = DOMAIN.to_vec();
    bytes.push(1);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.push(6);
    bytes.extend_from_slice(&6u32.to_le_bytes());
    bytes.push(0); // no result binding
    bytes.push(3); // voice handle, genuine zero-based static site data
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 8]);
    let (mut source, _) = source(&plan);
    let expressions = empty_expression_handles();
    let handles = RuntimeFlowSourceHandles {
        expression: &expressions,
        contents: &[],
        defers: &[],
        tasks: &[],
    };
    let seed =
        PreparedRuntimeFlowSource::preflight(&bytes, bounds, &resources(), &mut source.meter)
            .unwrap()
            .materialize(&mut source, &handles, &mut resources())
            .unwrap()
            .pop()
            .unwrap();
    assert!(
        matches!(seed, RuntimeFlowOpSeed::LineOperation { binding: None,
        operation: RuntimeLineOperationSeed::VoiceHandle { site } } if site.get() == 0)
    );
    // This is original static site data. Actual Line admission and executable
    // authority still require the enclosing group and the common program seal.
}

#[test]
fn flow_source_restores_complete_host_bodies_and_actual_completed_program_keys() {
    use super::super::host_calls::tests::{fixture, launch, replay_flow};
    use crate::step::RuntimeHostCallMode;
    for mode in [RuntimeHostCallMode::Immediate, RuntimeHostCallMode::Suspend] {
        for contract in [false, true] {
            for binding in [false, true] {
                for empty in [false, true] {
                    let plan = fixture(mode, contract, binding, empty);
                    let restored = replay_flow(&plan);
                    let original_bytes = plan
                        .encode_flow_source(ops(&plan), limits(&plan), &resources(), &mut meter())
                        .unwrap();
                    let restored_bytes = restored
                        .encode_flow_source(
                            ops(&restored),
                            limits(&restored),
                            &resources(),
                            &mut meter(),
                        )
                        .unwrap();
                    assert_eq!(original_bytes.bytes(), restored_bytes.bytes());
                    let FlowOp::HostCall { target, .. } = &ops(&plan)[0] else {
                        unreachable!()
                    };
                    assert!(restored.task_plan(&target.producer.plan).is_none());
                    assert_eq!(
                        plan.executable_semantic_digest(),
                        restored.executable_semantic_digest()
                    );
                    let (_original_engine, original) = launch(plan);
                    let (_restored_engine, restored) = launch(restored);
                    assert_eq!(original.args(), restored.args());
                    assert_eq!(original.mode, restored.mode);
                    assert_eq!(original.contract(), restored.contract());
                    assert_eq!(
                        original.submission().spec().producer.plan(),
                        restored.submission().spec().producer.plan()
                    );
                }
            }
        }
    }
}
