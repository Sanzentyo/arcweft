use super::*;
use crate::{
    entry::RuntimeCallableId,
    presentation::factory_test_support::{ALIAS, HANDLE, fixture},
    pure::{RuntimeExternalCallBackend, RuntimeExternalCallContext, VmRuntimePureCallBackend},
    value::RuntimeExprKind,
};
use std::{cell::Cell, rc::Rc};
struct Probe {
    target: RuntimeCallTarget,
    calls: Rc<Cell<usize>>,
}

impl RuntimeExternalCallBackend for Probe {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        assert_eq!(callee, &self.target);
        assert!(context.is_program_bound());
        assert!(args.is_empty());
        self.calls.set(self.calls.get() + 1);
        Some(Ok(RuntimeValue::Bool(true)))
    }
}

#[test]
fn native_factory_rejects_input_metadata_before_evaluating_the_capture() {
    let (builder, call) = fixture();
    let plan = builder.finish().unwrap();
    let input_type = plan.type_table().id_for_semantic(ALIAS).unwrap();
    let result_type = plan.type_table().id_for_semantic(HANDLE).unwrap();
    let target = RuntimeCallTarget::callable(RuntimeCallableId::from_checked_digest([95; 32]));
    let expression = RuntimeExpr::from_admitted_parts(
        result_type,
        RuntimeExprKind::Call {
            callee: RuntimeCallTarget::PresentationFactory(call),
            args: vec![RuntimeCallArgument::from_admitted_parts(
                RuntimeExpr::from_admitted_parts(
                    input_type,
                    RuntimeExprKind::Call {
                        callee: target.clone(),
                        args: vec![],
                    },
                ),
                RuntimeCallArgumentMode::Value,
                0,
            )],
        },
    );
    let calls = Rc::new(Cell::new(0));
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(Probe {
        target,
        calls: Rc::clone(&calls),
    });
    let mut engine = Engine::new(plan);
    let error = engine
        .evaluate_expr_with_backend(&expression, &mut backend)
        .unwrap_err();
    assert!(error.to_string().contains("requires type"), "{error}");
    assert_eq!(
        calls.get(),
        0,
        "invalid static factory metadata must not evaluate a capture"
    );
}
