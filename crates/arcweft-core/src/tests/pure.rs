use std::sync::Arc;

use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeCallArgumentSeed, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeLocalDeclarationSeed,
    RuntimeLocalSeedId, RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection,
    RuntimePlanTypeSeed, RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureHelperSeed,
    RuntimePureInputType, RuntimePureOutputType,
};
use crate::pure::{
    AotPureFunctionBackend, PureFunctionBackend, PureFunctionBackendKind, PureFunctionRequest,
    RuntimeI64Args, RuntimePureCallBackend, RuntimePureHelperRef, VmPureFunctionBackend,
    VmPureFunctionScratch, VmRuntimePureCallBackend, compare_pure_function_backend,
};
use crate::value::{
    RuntimeBinaryOp, RuntimeCallArgumentMode, RuntimeEvalError, RuntimeSignedIntWidth, RuntimeValue,
};

const I64_SEMANTIC_MARKER: u8 = 1;
const BOOL_SEMANTIC_MARKER: u8 = 2;
const FUNCTION_SEMANTIC_MARKER: u8 = 3;

fn semantic_type(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn i64_semantic_type() -> RuntimeSemanticTypeId {
    semantic_type(I64_SEMANTIC_MARKER)
}

fn bool_semantic_type() -> RuntimeSemanticTypeId {
    semantic_type(BOOL_SEMANTIC_MARKER)
}

fn scalar_type_seeds() -> [RuntimePlanTypeSeed; 2] {
    [
        RuntimePlanTypeSeed::new(
            i64_semantic_type(),
            RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
        ),
        RuntimePlanTypeSeed::new(bool_semantic_type(), RuntimePlanTypeProjection::Bool),
    ]
}

fn i64_value(value: i64) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Value(RuntimeValue::i64(value)),
    )
}

fn i64_local(local: RuntimeLocalSeedId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(i64_semantic_type(), RuntimeExprSeedKind::Local(local))
}

fn i64_binary(lhs: RuntimeExprSeed, op: RuntimeBinaryOp, rhs: RuntimeExprSeed) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Binary {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        },
    )
}

fn bool_binary(lhs: RuntimeExprSeed, op: RuntimeBinaryOp, rhs: RuntimeExprSeed) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        bool_semantic_type(),
        RuntimeExprSeedKind::Binary {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
        },
    )
}

struct AdmittedHelper {
    plan: Arc<RuntimePlan>,
    helper: RuntimePureHelperId,
}

impl AdmittedHelper {
    fn helper_ref(&self) -> RuntimePureHelperRef<'_> {
        RuntimePureHelperRef::resolve(&self.plan, self.helper).expect("admitted helper")
    }

    fn request(&self, args: impl IntoIterator<Item = RuntimeValue>) -> PureFunctionRequest {
        PureFunctionRequest::try_new(Arc::clone(&self.plan), self.helper, args)
            .expect("well-typed helper request")
    }
}

fn admit_i64_helper(
    name: &str,
    arity: usize,
    body: impl FnOnce(&[RuntimeLocalSeedId]) -> RuntimeExprSeed,
) -> AdmittedHelper {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            scalar_type_seeds(),
            (0..arity).map(|_| RuntimeLocalDeclarationSeed::new(i64_semantic_type())),
            [],
            [],
        )
        .expect("semantic helper inputs");
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: name.to_owned(),
            inputs: admission.local_ids().to_vec().into_boxed_slice(),
            input_abi: vec![RuntimePureInputType::I64; arity],
            output_abi: RuntimePureOutputType::I64,
            body: body(admission.local_ids()),
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("typed helper admission");
    let plan = Arc::new(builder.finish().expect("sealed helper plan"));
    let helper = plan.pure_helpers()[0].id;
    AdmittedHelper { plan, helper }
}

fn admitted_add_helper() -> AdmittedHelper {
    admit_i64_helper("add", 2, |inputs| {
        i64_binary(
            i64_local(inputs[0].clone()),
            RuntimeBinaryOp::Add,
            i64_local(inputs[1].clone()),
        )
    })
}

#[test]
fn pure_request_is_qualified_by_the_admitted_plan_and_helper() {
    let helper = admitted_add_helper();
    let request = helper.request([RuntimeValue::i64(3), RuntimeValue::i64(4)]);

    assert!(Arc::ptr_eq(request.plan(), &helper.plan));
    assert_eq!(request.helper_id(), helper.helper);
    assert_eq!(
        request
            .bindings()
            .iter()
            .map(|binding| binding.local)
            .collect::<Vec<_>>(),
        helper.plan.pure_helpers()[0].input_locals.as_ref()
    );

    let result = VmPureFunctionBackend
        .evaluate(&request)
        .expect("plan-qualified VM evaluation");
    assert_eq!(result.backend, PureFunctionBackendKind::Vm);
    assert_eq!(result.value, RuntimeValue::i64(7));
    assert_eq!(result.stats.evaluated_binary_ops, 1);
}

#[test]
fn pure_request_rejects_a_value_outside_the_input_local_type() {
    let helper = admitted_add_helper();
    let input = helper.plan.pure_helpers()[0].input_locals[0];
    let input_ty = helper
        .plan
        .local_declarations()
        .get(input)
        .expect("input declaration")
        .ty();

    assert_eq!(
        PureFunctionRequest::try_new(
            Arc::clone(&helper.plan),
            helper.helper,
            [
                RuntimeValue::String("wrong".to_owned()),
                RuntimeValue::i64(1)
            ],
        ),
        Err(RuntimeEvalError::InvalidExpressionType(input_ty))
    );
}

#[test]
fn runtime_backend_accepts_only_a_plan_qualified_helper_handle() {
    let helper = admitted_add_helper();
    let mut backend = VmRuntimePureCallBackend::default();

    let value = backend
        .call_i64(helper.helper_ref(), RuntimeI64Args::new([9, 4, 0, 0], 2))
        .expect("runtime helper call");

    assert_eq!(value, Some(13));
    assert_eq!(backend.stats().pure_calls, 1);
    assert_eq!(backend.stats().vm_calls, 1);
    assert_eq!(backend.stats().arg_stack_packs, 1);
}

#[test]
fn runtime_backend_flat_batch_reuses_the_same_plan_qualified_helper() {
    let helper = admit_i64_helper("multiply", 2, |inputs| {
        i64_binary(
            i64_local(inputs[0].clone()),
            RuntimeBinaryOp::Mul,
            i64_local(inputs[1].clone()),
        )
    });
    let mut backend = VmRuntimePureCallBackend::default();
    let mut output = [0; 3];

    backend
        .call_i64_flat_batch(helper.helper_ref(), &[2, 3, 4, 5, 6, 7], 2, &mut output)
        .expect("typed flat batch");

    assert_eq!(output, [6, 20, 42]);
    assert_eq!(backend.stats().flat_batch_calls, 1);
    assert_eq!(backend.stats().flat_batch_items, 3);
}

#[test]
fn vm_scratch_rebinds_plan_local_inputs_between_calls() {
    let helper = admitted_add_helper();
    let mut scratch = VmPureFunctionScratch::default();

    assert_eq!(
        scratch
            .evaluate_i64_slice(&helper.plan, helper.helper, &[1, 2])
            .expect("first evaluation"),
        RuntimeValue::i64(3)
    );
    assert_eq!(
        scratch
            .evaluate_i64_slice(&helper.plan, helper.helper, &[10, 20])
            .expect("second evaluation"),
        RuntimeValue::i64(30)
    );
}

#[test]
fn aot_plan_uses_the_helpers_plan_local_input_coordinates() {
    let helper = admitted_add_helper();
    let request = helper.request([RuntimeValue::i64(0), RuntimeValue::i64(0)]);
    let input_locals = helper.plan.pure_helpers()[0].input_locals.clone();
    let plan = AotPureFunctionBackend::new()
        .compile_i64_with_inputs(&request, input_locals.iter().copied())
        .expect("typed AOT compilation");

    let (value, stats) = plan
        .call_with_inputs(&[12, 30])
        .expect("typed AOT invocation");

    assert_eq!(value, 42);
    assert_eq!(stats.evaluated_binary_ops, 1);
}

#[test]
fn aot_and_vm_compare_the_same_admitted_helper() {
    let helper = admit_i64_helper("conditional", 2, |inputs| {
        RuntimeExprSeed::new(
            i64_semantic_type(),
            RuntimeExprSeedKind::If {
                condition: Box::new(bool_binary(
                    i64_local(inputs[0].clone()),
                    RuntimeBinaryOp::Ge,
                    i64_local(inputs[1].clone()),
                )),
                then_expr: Box::new(i64_binary(
                    i64_local(inputs[0].clone()),
                    RuntimeBinaryOp::Mul,
                    i64_value(2),
                )),
                else_expr: Box::new(i64_local(inputs[1].clone())),
            },
        )
    });
    let request = helper.request([RuntimeValue::i64(7), RuntimeValue::i64(4)]);

    let comparison = compare_pure_function_backend(
        &VmPureFunctionBackend,
        &AotPureFunctionBackend::new(),
        &request,
    )
    .expect("VM/AOT comparison");

    assert!(comparison.matches_vm);
    assert_eq!(comparison.vm.value, RuntimeValue::i64(14));
    assert_eq!(comparison.candidate.value, RuntimeValue::i64(14));
}

#[test]
fn structured_closure_captures_the_exact_owning_plan() {
    let function_semantic_type = semantic_type(FUNCTION_SEMANTIC_MARKER);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [
                scalar_type_seeds()[0].clone(),
                RuntimePlanTypeSeed::new(
                    function_semantic_type,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([i64_semantic_type()]),
                        result: i64_semantic_type(),
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
                RuntimeLocalDeclarationSeed::new(function_semantic_type),
                RuntimeLocalDeclarationSeed::new(i64_semantic_type()),
            ],
            [],
            [],
        )
        .expect("closure type graph");
    let captured = admission.local_ids()[0].clone();
    let closure_binding = admission.local_ids()[1].clone();
    let parameter = admission.local_ids()[2].clone();
    let site = builder
        .push_function_site_seed(
            [parameter.clone()],
            [captured.clone()],
            i64_binary(
                i64_local(parameter),
                RuntimeBinaryOp::Add,
                i64_local(captured.clone()),
            ),
        )
        .expect("typed closure site");
    let closure = RuntimeExprSeed::new(function_semantic_type, RuntimeExprSeedKind::Function(site));
    let apply = RuntimeExprSeed::new(
        i64_semantic_type(),
        RuntimeExprSeedKind::Apply {
            callee: Box::new(RuntimeExprSeed::new(
                function_semantic_type,
                RuntimeExprSeedKind::Local(closure_binding.clone()),
            )),
            args: Box::new([RuntimeCallArgumentSeed::new(
                i64_value(3),
                RuntimeCallArgumentMode::Value,
            )]),
        },
    );
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "captured_add".to_owned(),
            inputs: Box::new([captured]),
            input_abi: vec![RuntimePureInputType::I64],
            output_abi: RuntimePureOutputType::I64,
            body: RuntimeExprSeed::new(
                i64_semantic_type(),
                RuntimeExprSeedKind::Let {
                    binding: closure_binding,
                    expr: Box::new(closure),
                    body: Box::new(apply),
                },
            ),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("closure helper admission");
    let plan = Arc::new(builder.finish().expect("sealed closure plan"));
    let helper = plan.pure_helpers()[0].id;

    let value = VmPureFunctionBackend
        .evaluate(
            &PureFunctionRequest::try_new(Arc::clone(&plan), helper, [RuntimeValue::i64(4)])
                .expect("closure request"),
        )
        .expect("closure evaluation")
        .value;

    assert_eq!(value, RuntimeValue::i64(7));
}
