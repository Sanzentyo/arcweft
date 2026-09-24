use std::sync::Arc;

use crate::engine::Engine;
use crate::plan::RuntimeFunctionSiteBodyKind;
use crate::step::RuntimeStepOutput;
use crate::tests::function_application::{
    returning_callable_state, returning_function_plan, returning_function_result,
};
use crate::value::{RuntimeCallableValue, RuntimeEvalError, RuntimeValue};

#[test]
fn surplus_arguments_reject_before_creating_an_executable_frame() {
    let mut engine = Engine::new(returning_function_plan(
        RuntimeFunctionSiteBodyKind::Executable,
    ));
    let function = RuntimeCallableValue::try_new(
        crate::task::RuntimeProgramOwner::Plan(Arc::clone(&engine.plan)),
        returning_callable_state(&engine.plan),
        [],
    )
    .unwrap();
    let result = returning_function_result(&engine.plan);
    let before = engine.fiber().clone();
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    let mut output = RuntimeStepOutput::default();
    assert!(matches!(
        engine.start_function_value_call(
            RuntimeValue::Callable(function),
            vec![RuntimeValue::Unit],
            result,
            None,
            &mut output,
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
    assert_eq!(engine.fiber(), &before);
    assert_eq!(output, RuntimeStepOutput::default());
}
