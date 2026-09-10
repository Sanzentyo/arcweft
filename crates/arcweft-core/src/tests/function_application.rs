//! A real two-arrow callable for application-boundary tests in each evaluator.

use crate::{
    pattern::{RuntimeCheckedType, RuntimePattern, RuntimeSemanticTypeId},
    plan::{
        FlowOp, FlowRuntimeId, RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeExprSeed,
        RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed,
        RuntimeFunctionInputBindingSeed, RuntimeFunctionInputSource, RuntimeFunctionSiteBodyKind,
        RuntimeFunctionSiteBodySeed, RuntimeFunctionSiteDeclarationSeed,
        RuntimeLocalDeclarationSeed, RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan,
        RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    },
    runtime_id::RuntimeFunctionSiteId,
    value::{RuntimeExprKind, RuntimeValue},
};

#[expect(
    clippy::too_many_lines,
    reason = "one complete typed plan is shared by the evaluator admission tests"
)]
pub(crate) fn returning_function_plan(body_kind: RuntimeFunctionSiteBodyKind) -> RuntimePlan {
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    let inner = RuntimeSemanticTypeId::from_bytes([0x71; 32]);
    let outer = RuntimeSemanticTypeId::from_bytes([0x72; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    inner,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([unit]),
                        result: unit,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    outer,
                    RuntimePlanTypeProjection::Function {
                        parameters: Box::new([]),
                        result: inner,
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(unit)],
            [],
            [],
        )
        .expect("nested function types admit");
    let inner_site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            inputs: Box::new([RuntimeFunctionInputBindingSeed {
                source: RuntimeFunctionInputSource::Parameter { position: 0 },
                input_local: admission.local_ids()[0].clone(),
                pattern: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
            }]),
            result: unit,
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .expect("inner function reserves");
    builder
        .define_function_site_seed(
            &inner_site,
            RuntimeFunctionSiteBodySeed::Expression(RuntimeExprSeed::new(
                unit,
                RuntimeExprSeedKind::Value(RuntimeValue::Unit),
            )),
        )
        .expect("inner function defines");
    let outer_site = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            inputs: Box::new([]),
            result: inner,
            body_kind,
            effects: RuntimeEffectSet::empty(),
        })
        .expect("outer function reserves");
    let body = RuntimeExprSeed::new(
        inner,
        RuntimeExprSeedKind::Function {
            site: inner_site,
            captures: Box::new([]),
        },
    );
    let body = match body_kind {
        RuntimeFunctionSiteBodyKind::Expression => RuntimeFunctionSiteBodySeed::Expression(body),
        RuntimeFunctionSiteBodyKind::Executable => {
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(body)]),
            })
        }
    };
    builder
        .define_function_site_seed(&outer_site, body)
        .expect("outer function defines");
    let entry = FlowRuntimeId::canonical("application_arity").expect("fixture flow");
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: entry.clone(),
            parameters: Vec::new(),
        })
        .expect("fixture flow schema");
    let outer_value = RuntimeExprSeed::new(
        outer,
        RuntimeExprSeedKind::Function {
            site: outer_site,
            captures: Box::new([]),
        },
    );
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry,
            [],
            RuntimeEffectSet::empty(),
            vec![
                RuntimeFlowOpSeed::Let {
                    pattern: RuntimePatternSeed::new(outer, RuntimePatternSeedKind::Discard),
                    expr: outer_value.clone(),
                },
                RuntimeFlowOpSeed::ApplyFunction {
                    callee: outer_value,
                    args: Box::new([]),
                    result: RuntimePatternSeed::new(inner, RuntimePatternSeedKind::Discard),
                },
            ],
        ))
        .expect("exact-group caller admits");
    builder.finish().expect("application fixture seals")
}

pub(crate) fn returning_function_site(plan: &RuntimePlan) -> RuntimeFunctionSiteId {
    let FlowOp::Let { expr, .. } = &plan.flows()[0].body().ops()[0] else {
        panic!("fixture retains the exact function expression");
    };
    let RuntimeExprKind::Function { site, .. } = expr.kind() else {
        panic!("fixture function expression");
    };
    *site
}

pub(crate) fn returning_function_result(plan: &RuntimePlan) -> RuntimePattern {
    let FlowOp::ApplyFunction { result, .. } = &plan.flows()[0].body().ops()[1] else {
        panic!("fixture retains the exact result pattern");
    };
    result.clone()
}
