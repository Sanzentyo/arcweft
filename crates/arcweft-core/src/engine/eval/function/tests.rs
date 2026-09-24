use std::sync::Arc;

use crate::engine::Engine;
use crate::plan::RuntimeFunctionSiteBodyKind;
use crate::tests::function_application::{returning_callable_state, returning_function_plan};
use crate::value::{RuntimeCallableValue, RuntimeEvalError, RuntimeValue};

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
    let before = engine.fiber().clone();
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    assert!(matches!(
        engine.apply_runtime_function(&function, &[RuntimeValue::Unit], &mut backend),
        Err(RuntimeEvalError::Callable(
            crate::value::RuntimeCallableValueError::ArgumentCount {
                expected: 0,
                actual: 1,
                ..
            }
        ))
    ));
    assert_eq!(engine.fiber(), &before);
    let RuntimeValue::Callable(inner) = engine
        .apply_runtime_function(&function, &[], &mut backend)
        .unwrap()
    else {
        panic!("one group returns the remaining function");
    };
    assert_eq!(
        engine
            .apply_runtime_function(&inner, &[RuntimeValue::Unit], &mut backend)
            .unwrap(),
        RuntimeValue::Unit
    );
}
