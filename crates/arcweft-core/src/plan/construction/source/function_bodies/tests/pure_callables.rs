use super::super::super::pure_callables::{
    PreparedRuntimePureCallableSources, tests as declarations,
};
use super::*;

fn construction(
    plan: &RuntimePlan,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
    super::super::super::pure_callables::RuntimePureCallableSourceHandles,
) {
    let (mut source, functions) = declarations::source(plan);
    let bytes = declarations::encode(plan);
    let handles = PreparedRuntimePureCallableSources::preflight(
        &bytes,
        declarations::limits(plan),
        &mut source.meter,
    )
    .unwrap()
    .reserve(&mut source, &functions)
    .unwrap();
    (source, functions, handles)
}
fn expression_handles(
    callables: &super::super::super::pure_callables::RuntimePureCallableSourceHandles,
) -> RuntimeExpressionSourceHandles<'_> {
    RuntimeExpressionSourceHandles {
        helpers: &callables.helpers,
        methods: &callables.methods,
        ..expressions()
    }
}
fn invoke(plan: RuntimePlan, input: bool) -> RuntimeValue {
    let program = plan.pure_programs()[0].program();
    let mut engine =
        Engine::for_program_invocation(Arc::new(plan), program, vec![RuntimeValue::Bool(input)])
            .unwrap();
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
    let (actual, result) = engine.take_program_result().unwrap().unwrap();
    assert_eq!(actual, program);
    result
}
fn assert_undefined(source: &RuntimePlanSourceConstruction) {
    assert!(
        source
            .builder
            .function_sites
            .iter()
            .all(|row| row.body.is_none())
    );
    assert!(
        source
            .builder
            .pure_helpers
            .iter()
            .all(|row| row.body.is_none())
    );
    assert!(
        source
            .builder
            .trait_methods
            .iter()
            .all(|row| row.body.is_none())
    );
}

#[test]
fn function_body_sources_restore_forward_helpers_and_trait_method_through_actual_native_calls_at_exact_caps()
 {
    let plan = declarations::fixture();
    let encoded = plan
        .encode_function_body_sources(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let exact = RuntimeFunctionBodySourceLimits {
        rows: 4,
        nodes: u32::try_from(encoded.node_count()).unwrap(),
        elements: u32::try_from(encoded.elements()).unwrap(),
        encoded_bytes: encoded.bytes().len(),
        decoded_bytes: encoded.storage_bytes(),
        ..limits(&plan)
    };
    let (mut source, functions, callables) = construction(&plan);
    let expressions = expression_handles(&callables);
    let prepared = PreparedRuntimeFunctionBodySources::preflight(
        encoded.bytes(),
        exact,
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert_eq!(prepared.node_count(), encoded.node_count());
    assert_eq!(prepared.elements(), encoded.elements());
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    prepared
        .materialize(
            &mut source,
            &functions,
            &handles(&expressions),
            &mut resources(),
        )
        .unwrap();
    let restored = super::super::super::invocations::tests::finish(source);
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(restored.pure_helpers(), plan.pure_helpers());
    assert_eq!(restored.trait_methods(), plan.trait_methods());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(
        restored
            .encode_function_body_sources(exact, &resources(), &mut meter())
            .unwrap()
            .bytes(),
        encoded.bytes()
    );
    for input in [false, true] {
        assert_eq!(
            invoke(declarations::fixture(), input),
            RuntimeValue::Bool(input)
        );
        assert_eq!(invoke(restored.clone(), input), RuntimeValue::Bool(input));
    }
    for short in [
        RuntimeFunctionBodySourceLimits { rows: 3, ..exact },
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
}

#[test]
fn function_body_sources_check_every_pure_definition_and_body_family_before_any_body() {
    let plan = declarations::fixture();
    let bounds = limits(&plan);
    let encoded = plan
        .encode_function_body_sources(bounds, &resources(), &mut meter())
        .unwrap();
    for definition in [
        plan.pure_helpers()[1].definition,
        plan.trait_methods()[0].definition,
    ] {
        let offset = encoded
            .bytes()
            .windows(32)
            .position(|bytes| bytes == definition.as_bytes())
            .unwrap();
        let (mut source, functions, callables) = construction(&plan);
        let expressions = expression_handles(&callables);
        let mut bytes = encoded.bytes().to_vec();
        bytes[offset] ^= 1;
        let prepared = PreparedRuntimeFunctionBodySources::preflight(
            &bytes,
            bounds,
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        assert!(matches!(
            prepared.materialize(
                &mut source,
                &functions,
                &handles(&expressions),
                &mut resources()
            ),
            Err(Error::Definition { .. })
        ));
        assert_undefined(&source);
        assert!(source.meter.status().is_err());
        let mut bytes = encoded.bytes().to_vec();
        bytes[offset + 32] = 1;
        assert!(matches!(
            PreparedRuntimeFunctionBodySources::preflight(
                &bytes,
                bounds,
                &resources(),
                &mut meter()
            ),
            Err(Error::PureBodyKind { .. })
        ));
    }
    for end in 0..encoded.bytes().len() {
        assert!(
            PreparedRuntimeFunctionBodySources::preflight(
                &encoded.bytes()[..end],
                bounds,
                &resources(),
                &mut meter()
            )
            .is_err(),
            "prefix {end}"
        );
    }
}

#[test]
fn function_body_sources_authenticate_pure_handle_issuers_order_and_existing_bodies_before_publication()
 {
    let plan = declarations::fixture();
    let bounds = limits(&plan);
    let encoded = plan
        .encode_function_body_sources(bounds, &resources(), &mut meter())
        .unwrap();
    for case in 0..3 {
        let (mut source, functions, mut callables) = construction(&plan);
        let (_, _, foreign) = construction(&plan);
        match case {
            0 => callables.helpers[0] = foreign.helpers[0].clone(),
            1 => callables.helpers.swap(0, 1),
            2 => callables.methods[0] = foreign.methods[0].clone(),
            _ => unreachable!(),
        }
        let expressions = expression_handles(&callables);
        let prepared = PreparedRuntimeFunctionBodySources::preflight(
            encoded.bytes(),
            bounds,
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        assert!(
            prepared
                .materialize(
                    &mut source,
                    &functions,
                    &handles(&expressions),
                    &mut resources()
                )
                .is_err()
        );
        assert_undefined(&source);
    }
    for method in [false, true] {
        let (mut source, functions, callables) = construction(&plan);
        let ty = source
            .builder
            .types
            .get(plan.trait_methods()[0].body.ty())
            .unwrap()
            .semantic_identity();
        let body = RuntimeExprSeed::new(ty, RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)));
        if method {
            source
                .builder
                .define_trait_method_seed(&callables.methods[0], body)
                .unwrap();
        } else {
            source
                .builder
                .define_pure_helper_seed(&callables.helpers[1], body)
                .unwrap();
        }
        let expressions = expression_handles(&callables);
        let prepared = PreparedRuntimeFunctionBodySources::preflight(
            encoded.bytes(),
            bounds,
            &resources(),
            &mut source.meter,
        )
        .unwrap();
        let error = prepared
            .materialize(
                &mut source,
                &functions,
                &handles(&expressions),
                &mut resources(),
            )
            .unwrap_err();
        assert!(
            matches!(error,Error::Construction(error) if matches!(*error,RuntimePlanBuildError::DuplicatePureHelperDefinition { .. }|RuntimePlanBuildError::DuplicateTraitMethodDefinition { .. }))
        );
        assert!(
            source
                .builder
                .function_sites
                .iter()
                .all(|row| row.body.is_none())
        );
        assert!(source.builder.pure_helpers[0].body.is_none());
        assert!(source.meter.status().is_err());
    }
}
