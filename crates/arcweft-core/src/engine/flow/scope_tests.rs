use super::*;
use crate::plan::{
    RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionSiteDeclarationSeed, RuntimePatternSeed, RuntimePatternSeedKind,
    RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use crate::scope::RuntimeScopeFrameOrigin;
use crate::step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions};
fn engine() -> Engine {
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let boolean = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
            ],
            [],
        )
        .unwrap();
    let value = || RuntimeExprSeed::new(unit, RuntimeExprSeedKind::Value(RuntimeValue::Unit));
    let flow = crate::plan::FlowRuntimeId::canonical("snapshot.scoped").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .unwrap();
    let branch = || {
        vec![
            RuntimeFlowOpSeed::ExitScopeBind {
                pattern: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
                expr: value(),
            },
            RuntimeFlowOpSeed::ReturnExpr(value()),
        ]
    };
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xb8; 32]),
                None,
                Box::new([]),
                unit,
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: vec![
                    RuntimeFlowOpSeed::EnterScope {
                        identity: RuntimeScopeIdentity::Anonymous,
                    },
                    RuntimeFlowOpSeed::If {
                        condition: RuntimeExprSeed::new(
                            boolean,
                            RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                        ),
                        then_ops: branch(),
                        else_ops: branch(),
                    },
                ]
                .into_boxed_slice(),
            },
        ))
        .unwrap();
    Engine::for_flow(builder.finish().unwrap(), &flow).unwrap()
}
fn at_control_frame() -> Engine {
    let mut engine = engine();
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    };
    for _ in 0..16 {
        let step = engine.step(Default::default(), options);
        assert!(step.output.diagnostics.is_empty(), "{step:?}");
        if matches!(engine.fiber.control_stack.last().map(|entry|&entry.kind),Some(FlowControlStackEntryKind::Scope {origin:RuntimeScopeFrameOrigin::Scheduled(token),..}) if token.kind()==crate::scope::RuntimeScopeFrameKind::Control)
            && matches!(
                engine.fiber.pending_ops.front(),
                Some(FlowOp::ExitScopeBind { .. })
            )
        {
            return engine;
        }
    }
    panic!("real emitted body did not reach its generated branch frame")
}
#[test]
fn native_scope_snapshot_retains_exact_closes_and_consumes_them_once_after_emitted_exit() {
    let engine = at_control_frame();
    let image = engine.inert_rollback_image().unwrap();
    let mut restored = Engine::from_rollback_image(image).unwrap();
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    };
    let step = restored.step(Default::default(), options);
    assert!(step.output.diagnostics.is_empty(), "{step:?}");
    assert!(restored.fiber.control_stack.is_empty());
    assert!(
        !restored
            .fiber
            .pending_ops
            .iter()
            .any(|op| matches!(op, FlowOp::ExitScheduledScope { .. }))
    );
    let image = restored.inert_rollback_image().unwrap();
    let mut restored = Engine::from_rollback_image(image).unwrap();
    let step = restored.step(Default::default(), options);
    assert!(step.output.diagnostics.is_empty(), "{step:?}");
    assert!(matches!(step.fiber_status, FlowFiberStatus::Done(_)));
}
#[test]
fn native_scope_snapshot_rejects_missing_duplicate_and_foreign_close_tokens() {
    use crate::engine::FlowOpRollbackImage;
    let engine = at_control_frame();
    let image = engine.inert_rollback_image().unwrap();
    let mut missing = image.clone();
    missing
        .fiber
        .pending_ops
        .retain(|op| !matches!(op, FlowOpRollbackImage::ExitScheduledScope { .. }));
    assert_eq!(
        Engine::from_rollback_image(missing).unwrap_err(),
        "native scope frames and queued exact close markers are not bijective"
    );
    let mut duplicate = image.clone();
    let close = duplicate
        .fiber
        .pending_ops
        .iter()
        .find(|op| matches!(op, FlowOpRollbackImage::ExitScheduledScope { .. }))
        .unwrap()
        .clone();
    duplicate.fiber.pending_ops.push_back(close);
    assert_eq!(
        Engine::from_rollback_image(duplicate).unwrap_err(),
        "native pending scope close has a foreign, repeated, or unallocated token"
    );
    let mut foreign = image;
    let close = foreign
        .fiber
        .pending_ops
        .iter_mut()
        .find(|op| matches!(op, FlowOpRollbackImage::ExitScheduledScope { .. }))
        .unwrap();
    let FlowOpRollbackImage::ExitScheduledScope { token } = close else {
        unreachable!()
    };
    *token = crate::scope::RuntimeScheduledScopeToken::from_runtime(
        foreign.fiber.execution,
        crate::runtime_id::RuntimePersistentFiberId::from_allocated(
            foreign.fiber.persistent_id.get() + 1,
        ),
        token.ordinal(),
        token.kind(),
    );
    assert_eq!(
        Engine::from_rollback_image(foreign).unwrap_err(),
        "native pending scope close has a foreign, repeated, or unallocated token"
    );
}
