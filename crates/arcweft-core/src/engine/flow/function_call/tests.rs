use std::sync::Arc;

use crate::engine::Engine;
use crate::plan::RuntimeFunctionSiteBodyKind;
use crate::step::RuntimeStepOutput;
use crate::tests::function_application::{
    returning_function_plan, returning_function_result, returning_function_site,
};
use crate::value::{RuntimeEvalError, RuntimeFunctionValue, RuntimeValue};

#[test]
fn surplus_arguments_reject_before_creating_an_executable_frame() {
    let mut engine = Engine::new(returning_function_plan(
        RuntimeFunctionSiteBodyKind::Executable,
    ));
    let function = RuntimeFunctionValue::capture_site(
        Arc::clone(&engine.plan),
        returning_function_site(&engine.plan),
        [],
    )
    .unwrap();
    let result = returning_function_result(&engine.plan);
    let before = engine.fiber().clone();
    let mut backend = crate::pure::VmRuntimePureCallBackend::default();
    let mut output = RuntimeStepOutput::default();
    assert!(matches!(
        engine.start_function_value_call(
            RuntimeValue::Function(function),
            vec![RuntimeValue::Unit],
            result,
            None,
            &mut output,
            &mut backend,
        ),
        Err(RuntimeEvalError::FunctionArgumentCount {
            expected: 0,
            found: 1
        })
    ));
    assert_eq!(engine.fiber(), &before);
    assert_eq!(output, RuntimeStepOutput::default());
}
