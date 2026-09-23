use super::*;
use crate::{
    entry::RuntimeCallableId,
    plan::{
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimePlanBuilder, RuntimePlanTypeSeed,
        RuntimePureHelperOrigin, RuntimePureHelperSeed, RuntimePureProgramBindingSeed,
    },
};

fn fixture() -> (Arc<RuntimePlan>, RuntimePureProgramId, RuntimeCallTarget) {
    let semantic = RuntimeSemanticTypeId::from_bytes([37; 32]);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                semantic,
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    let target = RuntimeCallTarget::callable(RuntimeCallableId::from_checked_digest([44; 32]));
    let helper = builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "registered.default".to_owned(),
            inputs: Box::new([]),
            input_abi: vec![],
            output_abi: RuntimePureOutputType::Value,
            body: RuntimeExprSeed::new(
                semantic,
                RuntimeExprSeedKind::Call {
                    callee: target.clone(),
                    args: Box::new([]),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .unwrap();
    let program = RuntimePureProgramId::from_checked_digest([38; 32]);
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, helper })
        .unwrap();
    (Arc::new(builder.finish().unwrap()), program, target)
}

struct External {
    plan: Arc<RuntimePlan>,
    target: RuntimeCallTarget,
    value: RuntimeValue,
    calls: usize,
}
impl RuntimeExternalCallBackend for External {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        target: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        assert_eq!(target, &self.target);
        assert!(args.is_empty());
        let RuntimeProgramOwner::Plan(plan) = context.program_owner().unwrap() else {
            panic!("selected Plan")
        };
        assert!(Arc::ptr_eq(plan, &self.plan));
        assert_eq!(
            context.result_type(),
            Some(RuntimeSemanticTypeId::from_bytes([37; 32]))
        );
        self.calls += 1;
        Some(Ok(self.value.clone()))
    }
}

#[test]
fn pure_program_external_calls_keep_selected_owner_and_validate_results() {
    let (plan, program, target) = fixture();
    let mut absent = VmRuntimePureCallBackend::default();
    assert!(evaluate_pure_program_with_backend(&plan, program, &[], &mut absent).is_err());
    let mut backend = absent.with_external_calls(External {
        plan: Arc::clone(&plan),
        target,
        value: RuntimeValue::Bool(true),
        calls: 0,
    });
    assert_eq!(
        evaluate_pure_program_with_backend(&plan, program, &[], &mut backend).unwrap(),
        RuntimeValue::Bool(true)
    );
    assert_eq!(backend.external.calls, 1);
    backend.external.value = RuntimeValue::String("wrong result".to_owned());
    assert!(evaluate_pure_program_with_backend(&plan, program, &[], &mut backend).is_err());
    assert!(
        evaluate_pure_program_with_backend(
            &plan,
            RuntimePureProgramId::from_checked_digest([0; 32]),
            &[],
            &mut backend
        )
        .is_err()
    );
    assert!(
        evaluate_pure_program_with_backend(
            &plan,
            program,
            &[RuntimeValue::Bool(false)],
            &mut backend
        )
        .is_err()
    );
    assert_eq!(
        backend.external.calls, 2,
        "bad bindings and arity do not reach the external implementation"
    );
}
