//! Calls use the same real Flow function row as root execution.

use super::{flow_entry, flow_executable, flow_id, flow_schema, string_expr, type_id, unit_expr};
use crate::awbc_lower::AwbcLowerer;
use arcweft_core::awbc::{product_step::AwbcProductProgram, schema::AwbcProgram};
use arcweft_core::effect_row::{EffectFormula, EffectPredicate};
use arcweft_core::engine::{FlowExit, FlowFiberStatus};
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeCallableAttachedContract, RuntimeCallablePosition,
    RuntimeCallableStateDefinition, RuntimeCallableTransition, RuntimeEffectSet,
    RuntimeExecutableBodySeed, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed,
    RuntimeFlowSeed, RuntimeFunctionDefinitionIdentity, RuntimeFunctionSiteDeclarationSeed,
    RuntimeFunctionTypeContract, RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan,
    RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimeTypeBinder,
};
use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions};
use std::sync::Arc;

#[derive(Clone, Copy)]
enum Completion {
    Implicit,
    ExplicitUnit,
    ExplicitValue,
}

impl Completion {
    fn result(self) -> arcweft_core::pattern::RuntimeSemanticTypeId {
        type_id(if matches!(self, Self::ExplicitValue) {
            1
        } else {
            2
        })
    }

    fn body(self) -> Box<[RuntimeFlowOpSeed]> {
        match self {
            Self::Implicit => Box::new([]),
            Self::ExplicitUnit => Box::new([RuntimeFlowOpSeed::ReturnExpr(unit_expr())]),
            Self::ExplicitValue => Box::new([RuntimeFlowOpSeed::ReturnExpr(string_expr("callee"))]),
        }
    }
}

fn plan_builder(completion: Completion) -> RuntimePlanBuilder {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(type_id(2), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    type_id(4),
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            RuntimeTypeBinder::EMPTY,
                            EffectPredicate::unconstrained(),
                            EffectFormula::empty(),
                        ),
                        parameters: Box::new([]),
                        result: completion.result(),
                    },
                ),
            ],
            [],
        )
        .unwrap();
    builder
}

fn plan(completion: Completion) -> (RuntimePlan, FlowRuntimeId) {
    let mut builder = plan_builder(completion);
    let target = flow_id("snippet");
    builder.push_flow_schema(flow_schema(&target)).unwrap();
    let function = builder
        .push_flow_seed(RuntimeFlowSeed::new(
            target,
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xd1; 32]),
                Some(type_id(4)),
                Box::new([]),
                completion.result(),
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: completion.body(),
            },
        ))
        .unwrap();
    let state = builder.reserve_callable_state_seed().unwrap();
    builder
        .define_callable_state_seed(
            &state,
            RuntimeCallableStateDefinition {
                function_type: type_id(4),
                origin: state.clone(),
                position: RuntimeCallablePosition::Unapplied,
                retained: Box::new([]),
                parameters: Box::new([]),
                result: completion.result(),
                attached: RuntimeCallableAttachedContract::None,
                transition: RuntimeCallableTransition::Invoke {
                    function,
                    captures: Box::new([]),
                    arguments: Box::new([]),
                },
                partials: Box::new([]),
            },
        )
        .unwrap();
    let caller = flow_id("caller");
    builder.push_flow_schema(flow_schema(&caller)).unwrap();
    builder
        .push_flow_executable(flow_executable(&caller))
        .unwrap();
    builder
        .push_entry(flow_entry("caller", caller.clone()))
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            caller.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xd2; 32]),
                None,
                Box::new([]),
                type_id(1),
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::ApplyFunction {
                        callee: RuntimeExprSeed::new(
                            type_id(4),
                            RuntimeExprSeedKind::MakeCallable {
                                state,
                                captures: Box::new([]),
                            },
                        ),
                        args: Box::new([]),
                        result: RuntimePatternSeed::new(
                            completion.result(),
                            RuntimePatternSeedKind::Discard,
                        ),
                    },
                    RuntimeFlowOpSeed::ReturnExpr(string_expr("after")),
                ]),
            },
        ))
        .unwrap();
    (builder.finish().unwrap(), caller)
}

fn execute(completion: Completion, product: bool) {
    let (plan, caller) = plan(completion);
    let mut executor = if product {
        let lowered = AwbcLowerer::new(
            &plan,
            &arcweft_text_model::DialogueContentCatalog::new(),
            "flow-call",
        )
        .lower()
        .unwrap();
        let decoded = AwbcProgram::decode_canonical(
            &lowered.program.encode_canonical().unwrap(),
            Default::default(),
        )
        .unwrap();
        let product = AwbcProductProgram::admit(
            Arc::new(decoded),
            Default::default(),
            Default::default(),
            Default::default(),
            None,
        )
        .unwrap();
        ArcweftRuntimeExecutor::from_admitted_awbc_product_flow_invocation(
            product,
            plan.seal_flow_invocation(caller, []).unwrap(),
        )
        .unwrap()
    } else {
        ArcweftRuntimeExecutor::from_runtime_flow_invocation(
            plan.seal_flow_invocation(caller, []).unwrap(),
            ArcweftExecutionTier::RuntimePlanVm,
        )
        .unwrap()
    };
    // Keep one operation per step: the decoded VM charges value loads, moves,
    // call/return terminators and pattern binding separately.
    for _ in 0..16 {
        let output = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(output.stats.executed_ops <= 1);
        assert!(
            output.output.diagnostics.is_empty(),
            "{:?}",
            output.output.diagnostics
        );
        if matches!(
            executor.fiber().frame.status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        ) {
            assert_eq!(
                executor.fiber().frame.status,
                FlowFiberStatus::Done(FlowExit::Return("after".into()))
            );
            return;
        }
    }
    panic!("Flow invocation did not return within its fixture allowance");
}

#[test]
fn unit_flow_function_call_returns_to_its_native_caller() {
    execute(Completion::Implicit, false);
}

#[test]
fn unit_flow_function_call_returns_to_its_decoded_awbc_caller() {
    execute(Completion::Implicit, true);
}

#[test]
fn explicit_unit_flow_function_call_returns_to_its_native_caller() {
    execute(Completion::ExplicitUnit, false);
}

#[test]
fn explicit_unit_flow_function_call_returns_to_its_decoded_awbc_caller() {
    execute(Completion::ExplicitUnit, true);
}

#[test]
fn explicit_value_flow_function_call_returns_to_its_native_caller() {
    execute(Completion::ExplicitValue, false);
}

#[test]
fn explicit_value_flow_function_call_returns_to_its_decoded_awbc_caller() {
    execute(Completion::ExplicitValue, true);
}
