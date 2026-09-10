use std::sync::Arc;

use super::PureEvaluator;
use crate::plan::RuntimeFunctionSiteBodyKind;
use crate::tests::function_application::{returning_function_plan, returning_function_site};
use crate::value::{RuntimeEvalError, RuntimeFunctionValue, RuntimeValue};

#[test]
fn surplus_arguments_do_not_enter_either_function_body() {
    let plan = Arc::new(returning_function_plan(
        RuntimeFunctionSiteBodyKind::Expression,
    ));
    let function =
        RuntimeFunctionValue::capture_site(Arc::clone(&plan), returning_function_site(&plan), [])
            .unwrap();
    let mut evaluator = PureEvaluator::new_ref(&plan, &[]);
    assert!(matches!(
        evaluator.apply_runtime_function(&function, &[RuntimeValue::Unit]),
        Err(RuntimeEvalError::FunctionArgumentCount {
            expected: 0,
            found: 1
        })
    ));
    assert_eq!(evaluator.stats.evaluated_exprs, 0);
    let RuntimeValue::Function(inner) = evaluator.apply_runtime_function(&function, &[]).unwrap()
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
