use super::super::RuntimePlanSourceConstruction;
use super::super::expression::{RuntimeExpressionSourceHandles, tests::source};
use super::super::flow::RuntimeFlowSourceHandles;
use super::super::pattern::tests::{meter, resources};
use super::*;
use crate::engine::Engine;
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions};
use crate::value::{RuntimeSignedIntWidth, RuntimeValue};
use arcweft_id::{EffectId, runtime_program::RuntimePureProgramId};
use std::sync::Arc;

fn integer() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest()
}
fn program() -> RuntimePureProgramId {
    RuntimePureProgramId::from_checked_digest([0xd0; 32])
}
fn value(number: i64) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        integer(),
        RuntimeExprSeedKind::Value(RuntimeValue::i64(number)),
    )
}
fn effects(labels: &[&str]) -> RuntimeEffectSet {
    RuntimeEffectSet::try_from_effects(labels.iter().map(|label| EffectId::parse(label).unwrap()))
        .unwrap()
}
fn reserve(
    builder: &mut RuntimePlanBuilder,
    index: u8,
    kind: RuntimeFunctionSiteBodyKind,
    permissions: RuntimeEffectSet,
) -> RuntimeFunctionSiteSeedId {
    builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([index; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs: Box::new([]),
            result: integer(),
            body_kind: kind,
            effects: permissions,
        })
        .unwrap()
}
fn builder() -> RuntimePlanBuilder {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                integer(),
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
            )],
            [],
        )
        .unwrap();
    builder
}
fn fixture() -> RuntimePlan {
    let mut builder = builder();
    let expression = reserve(
        &mut builder,
        0xd1,
        RuntimeFunctionSiteBodyKind::Expression,
        RuntimeEffectSet::empty(),
    );
    let root = reserve(
        &mut builder,
        0xd2,
        RuntimeFunctionSiteBodyKind::Executable,
        RuntimeEffectSet::empty(),
    );
    let scoped = reserve(
        &mut builder,
        0xd3,
        RuntimeFunctionSiteBodyKind::Executable,
        effects(&["io.read(resource)", "log.write", "metric.write"]),
    );
    builder
        .define_function_site_seed(&expression, value(17))
        .unwrap();
    builder
        .define_function_site_seed(
            &root,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(value(42))]),
            }),
        )
        .unwrap();
    builder
        .define_function_site_seed(
            &scoped,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: effects(&["io.read(resource.document)", "log.write"]),
                ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(value(9))]),
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
pub(in crate::plan::construction::source) fn limits(
    plan: &RuntimePlan,
) -> RuntimeFunctionBodySourceLimits {
    RuntimeFunctionBodySourceLimits {
        rows: 1000,
        functions: plan.function_sites().len(),
        nodes: 100_000,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        flow: super::super::flow::tests::limits(plan),
    }
}
fn expressions() -> RuntimeExpressionSourceHandles<'static> {
    RuntimeExpressionSourceHandles {
        states: &[],
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &[],
    }
}
fn handles<'a>(expression: &'a RuntimeExpressionSourceHandles<'a>) -> RuntimeFlowSourceHandles<'a> {
    RuntimeFlowSourceHandles {
        expression,
        contents: &[],
        defers: &[],
        tasks: &[],
    }
}
pub(in crate::plan::construction::source) fn restore(
    plan: &RuntimePlan,
    source: &mut RuntimePlanSourceConstruction,
    functions: &[RuntimeFunctionSiteSeedId],
    handles: &RuntimeFlowSourceHandles<'_>,
) {
    let bounds = limits(plan);
    let encoded = plan
        .encode_function_body_sources(bounds, &resources(), &mut source.meter)
        .unwrap();
    let prepared = PreparedRuntimeFunctionBodySources::preflight(
        encoded.bytes(),
        bounds,
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert_eq!(prepared.node_count(), encoded.node_count());
    assert_eq!(prepared.elements(), encoded.elements());
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    prepared
        .materialize(source, functions, handles, &mut resources())
        .unwrap();
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
    let (id, result) = engine.take_program_result().unwrap().unwrap();
    assert_eq!(id, program());
    result
}

#[test]
fn function_body_sources_restore_mixed_bodies_scoped_effect_subsets_and_actual_native_execution() {
    let plan = fixture();
    let (mut source, functions) = source(&plan);
    restore(&plan, &mut source, &functions, &handles(&expressions()));
    source
        .builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program: program(),
            site: functions[1].clone(),
        })
        .unwrap();
    let restored = super::super::invocations::tests::finish(source);
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    let original = plan
        .encode_function_body_sources(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let repeated = restored
        .encode_function_body_sources(limits(&restored), &resources(), &mut meter())
        .unwrap();
    assert_eq!(repeated.bytes(), original.bytes());
    let function = restored.function_sites().iter().nth(2).unwrap();
    assert_ne!(
        function.invocation_effects(),
        function.body().executable().unwrap().effects()
    );
    assert_eq!(run(plan), RuntimeValue::i64(42));
    assert_eq!(run(restored), RuntimeValue::i64(42));
}

#[test]
fn function_body_sources_admit_aggregate_exact_caps_and_preserve_first_resource_failure() {
    let plan = fixture();
    let encoded = plan
        .encode_function_body_sources(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let exact = RuntimeFunctionBodySourceLimits {
        rows: 3,
        nodes: u32::try_from(encoded.node_count()).unwrap(),
        elements: u32::try_from(encoded.elements()).unwrap(),
        encoded_bytes: encoded.bytes().len(),
        decoded_bytes: encoded.storage_bytes(),
        ..limits(&plan)
    };
    let (mut source, functions) = source(&plan);
    PreparedRuntimeFunctionBodySources::preflight(
        encoded.bytes(),
        exact,
        &resources(),
        &mut source.meter,
    )
    .unwrap()
    .materialize(
        &mut source,
        &functions,
        &handles(&expressions()),
        &mut resources(),
    )
    .unwrap();
    assert_eq!(
        plan.encode_function_body_sources(exact, &resources(), &mut meter())
            .unwrap()
            .bytes(),
        encoded.bytes()
    );
    for short in [
        RuntimeFunctionBodySourceLimits { rows: 2, ..exact },
        RuntimeFunctionBodySourceLimits {
            nodes: exact.nodes - 1,
            ..exact
        },
        RuntimeFunctionBodySourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeFunctionBodySourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeFunctionBodySourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = meter();
        assert!(
            PreparedRuntimeFunctionBodySources::preflight(
                encoded.bytes(),
                short,
                &resources(),
                &mut bank
            )
            .is_err()
        );
        assert!(bank.status().is_err());
        assert!(
            plan.encode_function_body_sources(short, &resources(), &mut meter())
                .is_err()
        );
    }
    let mut bank = TaskSemanticMeter::new(0, 0);
    let first = PreparedRuntimeFunctionBodySources::preflight(
        encoded.bytes(),
        exact,
        &resources(),
        &mut bank,
    )
    .err()
    .unwrap();
    let second = PreparedRuntimeFunctionBodySources::preflight(
        encoded.bytes(),
        exact,
        &resources(),
        &mut bank,
    )
    .err()
    .unwrap();
    assert_eq!(first, second);
}

#[test]
fn function_body_sources_reject_truncations_ordinals_flags_effects_and_unreserved_output() {
    let plan = fixture();
    let bounds = limits(&plan);
    let encoded = plan
        .encode_function_body_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let bytes = encoded.bytes().to_vec();
    for end in 0..bytes.len() {
        assert!(
            PreparedRuntimeFunctionBodySources::preflight(
                &bytes[..end],
                bounds,
                &resources(),
                &mut meter()
            )
            .is_err(),
            "accepted prefix {end}"
        );
    }
    let row = DOMAIN.len() + 5;
    for (offset, tag) in [
        (0, 0xff),
        (DOMAIN.len(), 2),
        (row, 0),
        (row + 36, 0xff),
        (row + 37, 1),
    ] {
        let mut malformed = bytes.clone();
        malformed[offset] = tag;
        assert!(
            PreparedRuntimeFunctionBodySources::preflight(
                &malformed,
                bounds,
                &resources(),
                &mut meter()
            )
            .is_err(),
            "offset {offset}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        PreparedRuntimeFunctionBodySources::preflight(
            &trailing,
            bounds,
            &resources(),
            &mut meter()
        ),
        Err(Error::Trailing { .. })
    ));
    let effect = bytes
        .windows(b"log.write".len())
        .position(|part| part == b"log.write")
        .unwrap();
    for replacement in [b"aa.aaaaaa", b"io. read "] {
        let mut malformed = bytes.clone();
        malformed[effect..effect + replacement.len()].copy_from_slice(replacement);
        assert!(
            PreparedRuntimeFunctionBodySources::preflight(
                &malformed,
                bounds,
                &resources(),
                &mut meter()
            )
            .is_err()
        );
    }
    let mut output = Vec::with_capacity(bytes.len() + 1);
    output.push(7);
    encoded.append_to(&mut output).unwrap();
    assert_eq!(&output[1..], bytes.as_slice());
    let encoded = plan
        .encode_function_body_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let mut insufficient = vec![9];
    assert!(matches!(
        encoded.append_to(&mut insufficient),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(insufficient, [9]);
}

#[test]
fn function_body_sources_authenticate_all_headers_before_any_definition_is_published() {
    let plan = fixture();
    let bounds = limits(&plan);
    let encoded = plan
        .encode_function_body_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let original = encoded.bytes();
    let last = original
        .windows(32)
        .position(|part| part == [0xd3; 32])
        .unwrap();
    let label = original
        .windows(b"resource.document".len())
        .position(|part| part == b"resource.document")
        .unwrap();
    for kind in 0..4 {
        let (mut source, functions) = source(&plan);
        let mut bytes = original.to_vec();
        match kind {
            0 => bytes[last] ^= 1,
            1 => {
                source.builder.function_sites[2].body_kind =
                    RuntimeFunctionSiteBodyKind::Expression;
            }
            2 => bytes[label..label + 8].copy_from_slice(b"external"),
            3 => {
                source
                    .builder
                    .define_function_site_seed(
                        &functions[2],
                        RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                            effects: RuntimeEffectSet::empty(),
                            ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(value(1))]),
                        }),
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let prepared = PreparedRuntimeFunctionBodySources::preflight(
            &bytes,
            bounds,
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        let error = prepared
            .materialize(
                &mut source,
                &functions,
                &handles(&expressions()),
                &mut resources(),
            )
            .unwrap_err();
        match kind {
            0 => assert!(matches!(error, Error::Definition { row: 2 })),
            1 => assert!(
                matches!(error,Error::Construction(error) if matches!(*error,RuntimePlanBuildError::FunctionSiteBodyKindMismatch { .. }))
            ),
            2 => assert!(
                matches!(error,Error::Construction(error) if matches!(*error,RuntimePlanBuildError::FunctionSiteEffectSetMismatch { .. }))
            ),
            3 => assert!(
                matches!(error,Error::Construction(error) if matches!(*error,RuntimePlanBuildError::DuplicateFunctionSiteDefinition { .. }))
            ),
            _ => unreachable!(),
        }
        assert!(
            source.builder.function_sites[..2]
                .iter()
                .all(|function| function.body.is_none())
        );
        assert!(source.meter.status().is_err());
    }
}

#[test]
fn function_body_sources_require_the_original_issuer_and_dense_handle_order() {
    let plan = fixture();
    let bounds = limits(&plan);
    let encoded = plan
        .encode_function_body_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let original = encoded.bytes();
    let (_, foreign) = source(&plan);
    for use_foreign in [true, false] {
        let (mut actual, mut functions) = source(&plan);
        if !use_foreign {
            functions.swap(0, 1);
        }
        let functions = if use_foreign { &*foreign } else { &*functions };
        let prepared = PreparedRuntimeFunctionBodySources::preflight(
            original,
            bounds,
            &resources(),
            &mut meter(),
        )
        .unwrap();
        assert!(matches!(
            prepared.materialize(
                &mut actual,
                functions,
                &handles(&expressions()),
                &mut resources()
            ),
            Err(Error::Handle)
        ));
        assert!(
            actual
                .builder
                .function_sites
                .iter()
                .all(|function| function.body.is_none())
        );
    }
}

#[test]
fn function_body_sources_accept_empty_tables_and_reject_a_different_function_namespace() {
    let plan = RuntimePlanBuilder::new().finish().unwrap();
    let bounds = RuntimeFunctionBodySourceLimits {
        rows: 0,
        nodes: 0,
        elements: 0,
        decoded_bytes: 0,
        ..limits(&plan)
    };
    let encoded = plan
        .encode_function_body_sources(bounds, &resources(), &mut meter())
        .unwrap();
    let (mut source, functions) = source(&plan);
    PreparedRuntimeFunctionBodySources::preflight(
        encoded.bytes(),
        bounds,
        &resources(),
        &mut source.meter,
    )
    .unwrap()
    .materialize(
        &mut source,
        &functions,
        &handles(&expressions()),
        &mut resources(),
    )
    .unwrap();
    assert_eq!(
        super::super::invocations::tests::finish(source).executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    let wrong = RuntimeFunctionBodySourceLimits {
        functions: 1,
        ..bounds
    };
    assert!(matches!(
        PreparedRuntimeFunctionBodySources::preflight(
            encoded.bytes(),
            wrong,
            &resources(),
            &mut meter()
        ),
        Err(Error::Namespace { .. })
    ));
    assert!(matches!(
        plan.encode_function_body_sources(wrong, &resources(), &mut meter()),
        Err(Error::Namespace { .. })
    ));
}

#[test]
fn function_body_factory_rejects_both_body_family_mismatches_before_lowering_and_poisons_builder() {
    for expected in [
        RuntimeFunctionSiteBodyKind::Expression,
        RuntimeFunctionSiteBodyKind::Executable,
    ] {
        let mut builder = builder();
        let function = reserve(&mut builder, 0xd4, expected, RuntimeEffectSet::empty());
        let wrong = match expected {
            RuntimeFunctionSiteBodyKind::Expression => {
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(value(1))]),
                })
            }
            RuntimeFunctionSiteBodyKind::Executable => {
                RuntimeFunctionSiteBodySeed::Expression(value(2))
            }
        };
        let actual = wrong.kind();
        assert!(
            matches!(builder.define_function_site_seed(&function,wrong),Err(RuntimePlanBuildError::FunctionSiteBodyKindMismatch { expected:found_expected,actual:found_actual,.. }) if found_expected == expected && found_actual == actual)
        );
        assert!(builder.function_sites[0].body.is_none());
        assert_eq!(builder.finish(), Err(RuntimePlanBuildError::Poisoned));
    }
}
