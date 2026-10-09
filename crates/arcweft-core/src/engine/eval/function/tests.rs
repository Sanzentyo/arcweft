use std::sync::Arc;

use crate::engine::Engine;
use crate::plan::RuntimeFunctionSiteBodyKind;
use crate::tests::function_application::{returning_callable_state, returning_function_plan};
use crate::value::{RuntimeCallableValue, RuntimeEvalError, RuntimeValue};

#[test]
fn numeric_closure_uses_its_binder_while_ordinary_site_enters_the_scalar_backend() {
    use crate::plan::{
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionDefinitionIdentity,
        RuntimeFunctionInputBindingSeed, RuntimeFunctionInputOrigin, RuntimeFunctionInputSource,
        RuntimeFunctionInputTransfer, RuntimeFunctionParameterIdentity,
        RuntimeFunctionParameterPassing, RuntimeFunctionSemanticRole,
        RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind, RuntimeLocalBindingStorage,
        RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource, RuntimeLocalReadSeed,
        RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlanBuilder, RuntimePlanTypeProjection,
        RuntimePlanTypeSeed,
    };
    use crate::pure::RuntimePureCallBackend;
    use crate::value::{RuntimeLocalReadMode, RuntimeSignedIntWidth};

    let ty = crate::pattern::RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64)
        .semantic_identity_digest();
    for (role, expected_calls) in [
        (RuntimeFunctionSemanticRole::Closure, 0),
        (RuntimeFunctionSemanticRole::Ordinary, 1),
    ] {
        let mut builder = RuntimePlanBuilder::new();
        let admission = builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    ty,
                    RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
                )],
                [RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Binding {
                        identity: [83; 32],
                        declaration: RuntimeLocalBindingDeclaration::new(
                            RuntimeLocalBindingKind::PatternBinding,
                            false,
                            RuntimeLocalBindingStorage::Derived,
                        ),
                    },
                    ty,
                )],
            )
            .unwrap();
        let local = admission.local_ids()[0].clone();
        builder
            .push_function_site_seed(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([84; 32]),
                role,
                [RuntimeFunctionInputBindingSeed {
                    transfer: RuntimeFunctionInputTransfer::Formal,
                    origin: RuntimeFunctionInputOrigin::Parameter(
                        RuntimeFunctionParameterIdentity::from_accepted_identity([85; 32]),
                    ),
                    source: RuntimeFunctionInputSource::Parameter {
                        position: 0,
                        passing: RuntimeFunctionParameterPassing::Value,
                    },
                    input_local: local.clone(),
                    pattern: RuntimePatternSeed::new(
                        ty,
                        RuntimePatternSeedKind::Bind {
                            local: local.clone(),
                            mutable: false,
                        },
                    ),
                    ownership: Default::default(),
                    unrestricted_bindings: Box::new([]),
                }],
                RuntimeExprSeed::new(
                    ty,
                    RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                        local,
                        RuntimeLocalReadMode::Move,
                    )),
                ),
            )
            .unwrap();
        let mut engine = Engine::new(builder.finish().unwrap());
        let site = engine
            .plan
            .function_sites()
            .iter_with_ids()
            .next()
            .unwrap()
            .0;
        let before = engine.fiber().env.bindings_snapshot();
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        assert_eq!(
            engine
                .evaluate_function_site(site, vec![], vec![RuntimeValue::i64(42)], &mut backend)
                .unwrap(),
            RuntimeValue::i64(42)
        );
        assert_eq!(backend.stats().pure_calls, expected_calls);
        assert_eq!(
            engine.fiber().env.bindings_snapshot(),
            before,
            "the owning frame releases its input bindings"
        );
    }
}

#[test]
fn surplus_arguments_do_not_apply_the_returned_function() {
    let mut engine = Engine::new(returning_function_plan(
        RuntimeFunctionSiteBodyKind::Expression,
    ));
    let function = RuntimeCallableValue::try_new(
        crate::task::RuntimeProgramOwner::Plan(Arc::clone(&engine.plan)),
        returning_callable_state(&engine.plan),
        [],
    )
    .unwrap();
    let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&engine.plan));
    let before = engine.fiber().inert_rollback_image(&owner).unwrap();
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    assert!(matches!(
        engine.apply_runtime_function(
            function.try_duplicate_unrestricted().unwrap(),
            vec![RuntimeValue::Unit],
            &mut backend,
        ),
        Err(RuntimeEvalError::Callable(
            crate::value::RuntimeCallableValueError::ArgumentCount {
                expected: 0,
                actual: 1,
                ..
            }
        ))
    ));
    assert_eq!(engine.fiber().inert_rollback_image(&owner).unwrap(), before);
    let RuntimeValue::Callable(inner) = engine
        .apply_runtime_function(function, vec![], &mut backend)
        .unwrap()
    else {
        panic!("one group returns the remaining function");
    };
    assert_eq!(
        engine
            .apply_runtime_function(inner, vec![RuntimeValue::Unit], &mut backend)
            .unwrap(),
        RuntimeValue::Unit
    );
}
