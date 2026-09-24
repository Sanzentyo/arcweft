use std::sync::Arc;

use super::PureEvaluator;
use crate::plan::RuntimeFunctionSiteBodyKind;
use crate::tests::function_application::{returning_callable_state, returning_function_plan};
use crate::value::{RuntimeCallableValue, RuntimeEvalError, RuntimeValue};

#[test]
fn surplus_arguments_do_not_enter_either_function_body() {
    let plan = Arc::new(returning_function_plan(
        RuntimeFunctionSiteBodyKind::Expression,
    ));
    let function = RuntimeCallableValue::try_new(
        crate::task::RuntimeProgramOwner::Plan(Arc::clone(&plan)),
        returning_callable_state(&plan),
        [],
    )
    .unwrap();
    let mut evaluator = PureEvaluator::new_ref(&plan, &[]);
    assert!(matches!(
        evaluator.apply_runtime_function(&function, &[RuntimeValue::Unit]),
        Err(RuntimeEvalError::Callable(
            crate::value::RuntimeCallableValueError::ArgumentCount {
                expected: 0,
                actual: 1,
                ..
            }
        ))
    ));
    assert_eq!(evaluator.stats.evaluated_exprs, 0);
    let RuntimeValue::Callable(inner) = evaluator.apply_runtime_function(&function, &[]).unwrap()
    else {
        panic!("one group returns the remaining function");
    };
    assert_eq!(
        evaluator
            .apply_runtime_function(&inner, &[RuntimeValue::Unit])
            .unwrap(),
        RuntimeValue::Unit
    );
}
