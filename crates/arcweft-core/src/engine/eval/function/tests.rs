use std::sync::Arc;

use crate::engine::Engine;
use crate::plan::RuntimeFunctionSiteBodyKind;
use crate::tests::function_application::{returning_function_plan, returning_function_site};
use crate::value::{RuntimeEvalError, RuntimeFunctionValue, RuntimeValue};

#[test]
fn surplus_arguments_do_not_apply_the_returned_function() {
    let mut engine = Engine::new(returning_function_plan(
        RuntimeFunctionSiteBodyKind::Expression,
    ));
    let function = RuntimeFunctionValue::capture_site(
        Arc::clone(&engine.plan),
        returning_function_site(&engine.plan),
        [],
    )
    .unwrap();
    let before = engine.fiber().clone();
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    assert!(matches!(
        engine.apply_runtime_function(&function, &[RuntimeValue::Unit], &mut backend),
        Err(RuntimeEvalError::FunctionArgumentCount {
            expected: 0,
            found: 1
        })
    ));
    assert_eq!(engine.fiber(), &before);
    let RuntimeValue::Function(inner) = engine
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
