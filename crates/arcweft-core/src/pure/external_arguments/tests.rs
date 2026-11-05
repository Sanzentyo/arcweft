use super::*;
use crate::{
    entry::{RuntimeCallableId, RuntimeSchemaLimits},
    plan::{
        RuntimeCallArgumentSeed, RuntimeCallableParameterCoordinate as Coordinate, RuntimeExprSeed,
        RuntimeExprSeedKind, RuntimeFunctionDefinitionIdentity, RuntimeFunctionSemanticRole,
        RuntimePlan, RuntimePlanBuilder, RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed,
        RuntimePureProgramBindingSeed,
    },
    pure::{RuntimeExternalCallBackend, RuntimeProgramExecutionContext, VmRuntimePureCallBackend},
    task::RuntimeProgramOwner,
    value::{
        RuntimeCallArgumentMode, RuntimeExternalCallOperand, RuntimeExternalCallableDeclaration,
        RuntimeExternalCallableExport, RuntimeExternalCallableProvider, RuntimeFormatContext,
    },
};
use arcweft_id::{LocaleTag, runtime_program::RuntimePureProgramId};
use std::sync::Arc;

const BOOL: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([101; 32]);
const BOOL_TUPLE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([102; 32]);
const OPTION: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([103; 32]);
const OPTION_TUPLE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([104; 32]);
const NESTED_OPTION: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([105; 32]);
const STRING: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([106; 32]);
const PROGRAM: RuntimePureProgramId = RuntimePureProgramId::from_checked_digest([107; 32]);

fn target() -> RuntimeExternalCallable {
    RuntimeExternalCallable::new(
        RuntimeCallableId::from_checked_digest([108; 32]),
        RuntimeExternalCallableDeclaration::try_new(
            RuntimeExternalCallableProvider::Adapter("binding-fixture".into()),
            RuntimeExternalCallableExport::Function(Box::new(["choose".into()])),
            0,
        )
        .unwrap(),
    )
    .with_operands(
        [3, 2, 0, 1]
            .map(|position| RuntimeExternalCallOperand {
                projection: if position == 1 {
                    Projection::Value
                } else {
                    Projection::OptionalBinding
                },
                parameter: Some(Coordinate {
                    group: 0,
                    parameter: position,
                }),
                abi_position: position,
                expansion_width: 1,
            })
            .into(),
    )
    .unwrap()
}

fn arguments(text: &str) -> Vec<RuntimeValue> {
    vec![
        RuntimeValue::option_some(RuntimeValue::Bool(true)),
        RuntimeValue::String(text.into()),
        RuntimeValue::option_none(),
        RuntimeValue::option_some(RuntimeValue::option_none()),
    ]
}

fn plan() -> Arc<RuntimePlan> {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(BOOL, Type::Bool),
                RuntimePlanTypeSeed::new(BOOL_TUPLE, Type::Tuple(Box::new([BOOL]))),
                RuntimePlanTypeSeed::new(
                    OPTION,
                    Type::Option {
                        item: BOOL,
                        some_payload: BOOL_TUPLE,
                    },
                ),
                RuntimePlanTypeSeed::new(OPTION_TUPLE, Type::Tuple(Box::new([OPTION]))),
                RuntimePlanTypeSeed::new(
                    NESTED_OPTION,
                    Type::Option {
                        item: OPTION,
                        some_payload: OPTION_TUPLE,
                    },
                ),
                RuntimePlanTypeSeed::new(STRING, Type::String),
            ],
            [],
        )
        .unwrap();
    let types = [OPTION, STRING, OPTION, NESTED_OPTION];
    let mut values = arguments("kept").into_iter().map(Some).collect::<Vec<_>>();
    let args = [3, 2, 0, 1].map(|position| {
        RuntimeCallArgumentSeed::new(
            RuntimeExprSeed::new(
                types[position],
                RuntimeExprSeedKind::Value(values[position].take().unwrap()),
            ),
            RuntimeCallArgumentMode::Value,
            u32::try_from(position).unwrap(),
        )
    });
    let site = builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([109; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [],
            RuntimeExprSeed::new(
                BOOL,
                RuntimeExprSeedKind::Call {
                    callee: RuntimeCallTarget::External(target()),
                    args: args.into(),
                },
            ),
        )
        .unwrap();
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program: PROGRAM,
            site,
        })
        .unwrap();
    Arc::new(builder.finish().unwrap())
}

fn context(plan: Arc<RuntimePlan>, limits: RuntimeSchemaLimits) -> RuntimeExternalCallContext {
    RuntimeExternalCallContext::for_program(
        RuntimeProgramExecutionContext::native(
            plan,
            RuntimeFormatContext::new(LocaleTag::try_new("ja-JP").unwrap()),
        ),
        [OPTION, STRING, OPTION, NESTED_OPTION],
        BOOL,
        limits,
    )
    .unwrap()
}

#[test]
fn forwarded_order_changes_descriptors_while_abi_values_and_inner_none_stay_in_place() {
    use crate::value::{RuntimeExternalCallOperandOrder as OrderSource, RuntimeInputOrder};
    let plan = plan();
    let mut rows = target().operands().to_vec();
    rows.sort_unstable_by_key(|row| row.abi_position);
    let forwarded = target()
        .with_operands(rows.into_boxed_slice())
        .unwrap()
        .with_operand_order(OrderSource::InvocationInputs)
        .unwrap();
    let order = Arc::new(RuntimeInputOrder::try_new([3, 2, 0, 1]).unwrap());
    let execution =
        RuntimeProgramExecutionContext::native(Arc::clone(&plan), RuntimeFormatContext::default())
            .with_input_order(Some(&order));
    let original = RuntimeExternalCallContext::for_program(
        execution,
        [OPTION, STRING, OPTION, NESTED_OPTION],
        BOOL,
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let prepared = original
        .prepare_external_bindings(&forwarded, arguments("kept"))
        .unwrap();
    let RuntimeCallTarget::External(call) = prepared.callee else {
        panic!("external target")
    };
    assert_eq!(call.operand_order(), OrderSource::Authored);
    assert_eq!(
        call.operands()
            .iter()
            .map(|row| (row.parameter.unwrap().parameter, row.abi_position))
            .collect::<Vec<_>>(),
        [(3, 2), (0, 0), (1, 1)]
    );
    assert_eq!(
        prepared.arguments,
        [
            RuntimeValue::Bool(true),
            RuntimeValue::String("kept".into()),
            RuntimeValue::option_none()
        ]
    );
    assert_eq!(
        prepared.context.argument_types(),
        Some([BOOL, STRING, OPTION].as_slice())
    );
    assert_eq!(
        prepared
            .context
            .execution_context()
            .unwrap()
            .input_order()
            .unwrap()
            .slots(),
        [3, 2, 0, 1]
    );
}

#[test]
fn forwarding_requires_the_actual_complete_invocation_order() {
    use crate::value::{RuntimeExternalCallOperandOrder, RuntimeInputOrder};
    let plan = plan();
    let mut rows = target().operands().to_vec();
    rows.sort_unstable_by_key(|row| row.abi_position);
    let forwarded = target()
        .with_operands(rows.into_boxed_slice())
        .unwrap()
        .with_operand_order(RuntimeExternalCallOperandOrder::InvocationInputs)
        .unwrap();
    let missing = context(Arc::clone(&plan), RuntimeSchemaLimits::engine_default())
        .prepare_external_bindings(&forwarded, arguments("kept"))
        .unwrap_err();
    assert!(matches!(
        *missing,
        RuntimeExternalCallArgumentError::MissingInputOrder
    ));
    let order = Arc::new(RuntimeInputOrder::canonical(3));
    let execution = RuntimeProgramExecutionContext::native(plan, RuntimeFormatContext::default())
        .with_input_order(Some(&order));
    let wrong = RuntimeExternalCallContext::for_program(
        execution,
        [OPTION, STRING, OPTION, NESTED_OPTION],
        BOOL,
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap()
    .prepare_external_bindings(&forwarded, arguments("kept"))
    .unwrap_err();
    assert!(matches!(
        *wrong,
        RuntimeExternalCallArgumentError::InputOrder(crate::value::RuntimeInputOrderError::Arity {
            expected: 4,
            actual: 3
        })
    ));
}

#[test]
fn external_optional_bindings_preserve_owner_limits_source_order_and_inner_none() {
    let plan = plan();
    let limits = RuntimeSchemaLimits {
        max_string_bytes: 4,
        ..RuntimeSchemaLimits::engine_default()
    };
    let original = context(Arc::clone(&plan), limits);
    let prepared = original
        .prepare_external_bindings(&target(), arguments("kept"))
        .unwrap();
    let RuntimeProgramOwner::Plan(owner) = prepared.context.program_owner().unwrap() else {
        panic!("native owner")
    };
    assert!(Arc::ptr_eq(&owner, &plan));
    assert_eq!(prepared.context.limits(), Some(limits));
    assert_eq!(prepared.context.result_type(), Some(BOOL));
    assert_eq!(
        prepared
            .context
            .execution_context()
            .unwrap()
            .format_context(),
        original.execution_context().unwrap().format_context()
    );
    assert_eq!(
        prepared.context.argument_types(),
        Some([BOOL, STRING, OPTION].as_slice())
    );
    assert_eq!(
        original.argument_types(),
        Some([OPTION, STRING, OPTION, NESTED_OPTION].as_slice())
    );
    assert_eq!(
        prepared.arguments,
        [
            RuntimeValue::Bool(true),
            RuntimeValue::String("kept".into()),
            RuntimeValue::option_none()
        ]
    );
    let RuntimeCallTarget::External(call) = prepared.callee else {
        unreachable!()
    };
    assert_eq!(
        call.operands()
            .iter()
            .map(|row| (
                row.parameter.unwrap().parameter,
                row.abi_position,
                row.projection
            ))
            .collect::<Vec<_>>(),
        [
            (3, 2, Projection::Value),
            (0, 0, Projection::Value),
            (1, 1, Projection::Value)
        ]
    );
}

struct Backend {
    plan: Arc<RuntimePlan>,
    calls: usize,
}
impl RuntimeExternalCallBackend for Backend {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, crate::value::RuntimeEvalError>> {
        let RuntimeProgramOwner::Plan(owner) = context.program_owner().unwrap() else {
            panic!("native owner")
        };
        assert!(Arc::ptr_eq(&owner, &self.plan));
        assert_eq!(
            context.argument_types(),
            Some([BOOL, STRING, OPTION].as_slice())
        );
        let RuntimeCallTarget::External(call) = callee else {
            panic!("external call")
        };
        assert!(
            call.operands()
                .iter()
                .all(|row| row.projection == Projection::Value)
        );
        assert_eq!(
            args,
            [
                RuntimeValue::Bool(true),
                RuntimeValue::String("kept".into()),
                RuntimeValue::option_none()
            ]
        );
        self.calls += 1;
        Some(Ok(RuntimeValue::Bool(false)))
    }
}

#[test]
fn native_and_pure_dispatch_apply_optional_binding_projection_before_the_backend() {
    let plan = plan();
    let original = context(Arc::clone(&plan), RuntimeSchemaLimits::engine_default());
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(Backend {
        plan: Arc::clone(&plan),
        calls: 0,
    });
    assert_eq!(
        crate::engine::evaluate_runtime_call(
            &RuntimeCallTarget::External(target()),
            arguments("kept"),
            &original,
            &mut backend
        )
        .unwrap(),
        RuntimeValue::Bool(false)
    );
    assert_eq!(
        original
            .execution_context()
            .unwrap()
            .evaluate_pure_program(PROGRAM, &[], &mut backend)
            .unwrap(),
        RuntimeValue::Bool(false)
    );
    assert_eq!(backend.external.calls, 2);
}

#[test]
fn external_optional_binding_rejects_missing_context_wrong_shape_and_late_limit_failure() {
    let plan = plan();
    let error = RuntimeExternalCallContext::unbound()
        .prepare_external_bindings(&target(), arguments("kept"))
        .unwrap_err();
    assert_eq!(*error, RuntimeExternalCallArgumentError::Unbound);
    let original = context(
        Arc::clone(&plan),
        RuntimeSchemaLimits {
            max_string_bytes: 4,
            ..RuntimeSchemaLimits::engine_default()
        },
    );
    let mut backend = VmRuntimePureCallBackend::default().with_external_calls(Backend {
        plan: Arc::clone(&plan),
        calls: 0,
    });
    let error = crate::engine::evaluate_runtime_call(
        &RuntimeCallTarget::External(target()),
        arguments("longer"),
        &original,
        &mut backend,
    )
    .unwrap_err();
    assert!(
        matches!(error, crate::value::RuntimeEvalError::ExternalArguments(error) if matches!(error.as_ref(), RuntimeExternalCallArgumentError::Type { argument: 1, .. }))
    );
    assert_eq!(backend.external.calls, 0);
    let mut wrong = target();
    let mut rows = wrong.operands().to_vec();
    rows.iter_mut()
        .find(|row| row.abi_position == 1)
        .unwrap()
        .projection = Projection::OptionalBinding;
    wrong = wrong.with_operands(rows.into()).unwrap();
    let error = original
        .prepare_external_bindings(&wrong, arguments("kept"))
        .unwrap_err();
    assert!(matches!(
        *error,
        RuntimeExternalCallArgumentError::ExpectedOption {
            argument: 1,
            ty: STRING
        }
    ));
}
