use super::*;
use crate::{
    entry::RuntimeCallableId,
    presentation::factory_test_support::{ALIAS, BOOL, HANDLE, fixture},
    pure::{RuntimeExternalCallBackend, RuntimeExternalCallContext, VmRuntimePureCallBackend},
    value::{RuntimeExprKind, RuntimePayload},
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

#[derive(Clone, Copy, Debug)]
enum InvalidContext {
    MissingArtifact,
    ForeignProgram,
    ForeignExecution,
    MissingLedger,
    MissingParentLedger,
}

fn rejects_context_before_capture(fault: InvalidContext) {
    let (builder, call) = fixture();
    let mut plan = builder.finish().unwrap();
    if !matches!(fault, InvalidContext::MissingArtifact) {
        plan.bind_artifact(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([97; 32]).unwrap(),
        )
        .unwrap();
    }
    let input_type = plan.type_table().id_for_semantic(BOOL).unwrap();
    let result_type = plan.type_table().id_for_semantic(HANDLE).unwrap();
    let mut engine = Engine::new(plan);
    match fault {
        InvalidContext::MissingArtifact => {}
        InvalidContext::ForeignProgram => {
            let foreign = Arc::new((*engine.plan).clone());
            assert_eq!(*foreign, *engine.plan);
            assert!(!Arc::ptr_eq(&foreign, &engine.plan));
            engine.fiber.frame.program = crate::task::RuntimeProgramOwner::Plan(foreign);
        }
        InvalidContext::ForeignExecution => {
            engine.fiber.execution = crate::runtime_id::ExecutionInstanceId::from_allocated(
                std::num::NonZeroU64::new(2).unwrap(),
            );
        }
        InvalidContext::MissingLedger | InvalidContext::MissingParentLedger => {
            let mut ledger =
                crate::presentation::resources::PresentationLedger::<RuntimePayload>::new(
                    engine.fiber.frame.program.clone(),
                    engine.plan.artifact().unwrap(),
                    crate::runtime_id::RuntimePresentationBankId::Root,
                    crate::presentation::resources::PresentationLedgerLimits::for_schema(
                        crate::entry::RuntimeSchemaLimits::engine_default(),
                    ),
                );
            engine.fiber.bind_presentation_scope(&mut ledger).unwrap();
            if matches!(fault, InvalidContext::MissingParentLedger) {
                engine.fiber.frame.env.push_scope();
                assert!(
                    engine
                        .fiber
                        .frame
                        .env
                        .presentation_scope_binding()
                        .is_none()
                );
            }
        }
    }
    let before = engine
        .fiber
        .frame
        .env
        .presentation_scope_bindings()
        .cloned()
        .collect::<Vec<_>>();
    let target = RuntimeCallTarget::callable(RuntimeCallableId::from_checked_digest([98; 32]));
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
    let error = engine
        .evaluate_expr_with_backend(&expression, &mut backend)
        .unwrap_err();
    assert_eq!(calls.get(), 0, "{fault:?}: capture ran before {error}");
    assert!(engine.presentation.is_none(), "{fault:?}");
    assert_eq!(
        engine
            .fiber
            .frame
            .env
            .presentation_scope_bindings()
            .cloned()
            .collect::<Vec<_>>(),
        before,
        "{fault:?}: rejection changed the original grants"
    );
}

#[test]
fn native_factory_rejects_missing_artifact_before_capture() {
    rejects_context_before_capture(InvalidContext::MissingArtifact);
}

#[test]
fn native_factory_rejects_equal_bytes_foreign_program_before_capture() {
    rejects_context_before_capture(InvalidContext::ForeignProgram);
}

#[test]
fn native_factory_rejects_foreign_execution_before_capture() {
    rejects_context_before_capture(InvalidContext::ForeignExecution);
}

#[test]
fn native_factory_rejects_missing_ledger_before_capture() {
    rejects_context_before_capture(InvalidContext::MissingLedger);
}

#[test]
fn native_factory_rejects_missing_parent_ledger_before_capture() {
    rejects_context_before_capture(InvalidContext::MissingParentLedger);
}
