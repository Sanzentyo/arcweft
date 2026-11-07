use super::*;
use crate::effect::{LineEffectRequest, RuntimeLog};
use crate::engine::{Engine, FlowExit, FlowFiberStatus};
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::scope::{RuntimeDeferOutcomeFilter, RuntimeScopeFrameOrigin};
use crate::step::{RuntimeStepInput, RuntimeStepOptions};
use crate::task::RuntimeProgramOwner;
use crate::value::RuntimeValue;

fn log(message: &str) -> RuntimeFlowOpSeed {
    RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Static(LineEffectRequest::Log(
        RuntimeLog {
            level: "info".into(),
            message: message.into(),
            fields: Vec::new(),
        },
    )))
}

fn function_body_engine(with_defer: bool, cleanup_fails: bool) -> Engine {
    let mut builder = RuntimePlanBuilder::new();
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    let function_type = RuntimeSemanticTypeId::from_bytes([137; 32]);
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    function_type,
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::default(),
                        parameters: Box::new([]),
                        result: unit,
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let mut registrations = Vec::new();
    for (index, message) in ["caller cleanup", "callee cleanup"].into_iter().enumerate() {
        let callback = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [138 + index as u8; 32],
                ),
                role: RuntimeFunctionSemanticRole::Effect,
                function_type: None,
                inputs: Box::new([]),
                result: unit,
                body_kind: RuntimeFunctionSiteBodyKind::Executable,
                effects: RuntimeEffectSet::empty(),
            })
            .unwrap();
        let mut ops = vec![log(message)];
        if cleanup_fails && index == 1 {
            ops.push(RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Static(
                LineEffectRequest::Fail("callee cleanup failed".into()),
            )));
        }
        ops.push(RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
            unit,
            RuntimeExprSeedKind::Value(RuntimeValue::Unit),
        )));
        builder
            .define_function_site_seed(
                &callback,
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: ops.into_boxed_slice(),
                }),
            )
            .unwrap();
        registrations.push(RuntimeFlowOpSeed::RegisterDefer {
            site: builder.reserve_defer_site_seed(&callback).unwrap(),
            outcome: RuntimeDeferOutcomeFilter::Always,
            captures: Vec::new(),
            owner: RuntimeDeferOwner::CurrentScope,
        });
    }
    let callee = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed::flow(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([140; 32]),
            Some(function_type),
            Box::new([]),
            unit,
            RuntimeEffectSet::empty(),
        ))
        .unwrap();
    let mut callee_ops = Vec::new();
    if with_defer {
        callee_ops.push(registrations.pop().unwrap());
    }
    callee_ops.push(log("callee body"));
    builder
        .define_function_site_seed(
            &callee,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: callee_ops.into_boxed_slice(),
            }),
        )
        .unwrap();
    let callee_flow = FlowRuntimeId::canonical("test.function_body_callee").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: callee_flow.clone(),
            parameters: Vec::new(),
        })
        .unwrap();
    builder
        .bind_flow_function_seed(callee_flow, &callee)
        .unwrap();
    let flow = FlowRuntimeId::canonical("test.function_body_scope").unwrap();
    let mut ops = Vec::new();
    if with_defer {
        ops.push(registrations.remove(0));
    }
    ops.push(log("caller body"));
    ops.push(RuntimeFlowOpSeed::ApplyFunction {
        callee: RuntimeExprSeed::new(
            function_type,
            RuntimeExprSeedKind::Function {
                site: callee,
                captures: Box::new([]),
            },
        ),
        args: Box::new([]),
        result: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
    });
    ops.push(log("caller resumed"));
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([141; 32]),
                None,
                Box::new([]),
                unit,
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: ops.into_boxed_slice(),
            },
        ))
        .unwrap();
    Engine::for_flow(builder.finish().unwrap(), &flow).unwrap()
}

#[test]
fn implicit_function_scopes_preserve_budget_and_finish_after_lifo_cleanup() {
    for with_defer in [false, true] {
        let mut engine = function_body_engine(with_defer, false);
        let owner = RuntimeProgramOwner::Plan(engine.program_plan());
        let mut logs = Vec::new();
        let mut ops = 0;
        let mut done = false;
        let mut checked_fallthrough = false;
        for _ in 0..96 {
            let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
            assert!(result.stats.executed_ops <= 1);
            assert!(result.output.diagnostics.is_empty(), "{:?}", result.output);
            ops += result.stats.executed_ops;
            logs.extend(
                result
                    .output
                    .effects
                    .line
                    .into_iter()
                    .filter_map(|effect| match effect {
                        LineEffectRequest::Log(row) => Some(row.message),
                        _ => None,
                    }),
            );
            let image = engine.inert_rollback_image().unwrap();
            if let Some(exit) = &image.fiber.frame.lexical_exit
                && let super::super::defer::NativeLexicalExitAction::Fallthrough { site } =
                    exit.action
            {
                checked_fallthrough = true;
                let other = engine
                    .plan
                    .function_sites()
                    .iter_with_ids()
                    .find(|(candidate, _)| *candidate != site)
                    .unwrap()
                    .0;
                let mut wrong = image.clone();
                let super::super::defer::NativeLexicalExitAction::Fallthrough { site } =
                    &mut wrong.fiber.frame.lexical_exit.as_mut().unwrap().action
                else {
                    unreachable!()
                };
                *site = other;
                assert!(Engine::from_rollback_image(wrong, &owner).is_err());
                let mut wrong = image.clone();
                wrong.fiber.frame.lexical_exit.as_mut().unwrap().target = usize::MAX;
                assert!(Engine::from_rollback_image(wrong, &owner).is_err());
                let mut wrong = image.clone();
                wrong
                    .fiber
                    .frame
                    .pending_ops
                    .front_mut()
                    .unwrap()
                    .position_mut()
                    .unwrap()
                    .set_operation(0);
                assert!(Engine::from_rollback_image(wrong, &owner).is_err());
            }
            engine = Engine::from_rollback_image(image, &owner).unwrap();
            if matches!(result.fiber_status, FlowFiberStatus::Done(FlowExit::Done)) {
                done = true;
                break;
            }
            assert!(matches!(result.fiber_status, FlowFiberStatus::Running));
        }
        assert!(done, "{logs:?}");
        if with_defer {
            assert!(checked_fallthrough);
            assert_eq!(
                logs,
                [
                    "caller body",
                    "callee body",
                    "callee cleanup",
                    "caller resumed",
                    "caller cleanup"
                ]
            );
        } else {
            assert!(!checked_fallthrough);
            assert_eq!(logs, ["caller body", "callee body", "caller resumed"]);
            assert_eq!(ops, 6, "four authored operations and two EOF transitions");
        }
    }
}

#[test]
fn failed_implicit_function_cleanup_does_not_resume_the_caller() {
    let mut engine = function_body_engine(true, true);
    let owner = RuntimeProgramOwner::Plan(engine.program_plan());
    let mut logs = Vec::new();
    for _ in 0..96 {
        let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(result.stats.executed_ops <= 1);
        logs.extend(
            result
                .output
                .effects
                .line
                .into_iter()
                .filter_map(|effect| match effect {
                    LineEffectRequest::Log(row) => Some(row.message),
                    _ => None,
                }),
        );
        engine =
            Engine::from_rollback_image(engine.inert_rollback_image().unwrap(), &owner).unwrap();
        if let FlowFiberStatus::Failed(message) = result.fiber_status {
            assert_eq!(message, "callee cleanup failed");
            assert_eq!(
                logs,
                [
                    "caller body",
                    "callee body",
                    "callee cleanup",
                    "caller cleanup"
                ]
            );
            return;
        }
    }
    panic!("cleanup failure did not finish: {logs:?}");
}

#[test]
fn function_body_scope_restore_rejects_other_sites_and_duplicate_scopes() {
    let engine = function_body_engine(true, false);
    let owner = RuntimeProgramOwner::Plan(engine.program_plan());
    let image = engine.inert_rollback_image().unwrap();
    let mut missing = image.clone();
    missing.fiber.frame.control_stack.clear();
    assert!(Engine::from_rollback_image(missing, &owner).is_err());
    let mut wrong = image.clone();
    let FlowControlStackEntryKind::Scope { origin, .. } = &mut wrong.fiber.frame.control_stack[0]
    else {
        panic!("function body scope")
    };
    let RuntimeScopeFrameOrigin::FunctionBody(site) = origin else {
        panic!("original function")
    };
    *site = engine
        .plan
        .function_sites()
        .iter_with_ids()
        .find(|(candidate, _)| candidate != site)
        .unwrap()
        .0;
    assert!(Engine::from_rollback_image(wrong, &owner).is_err());
    let mut wrong = image;
    wrong
        .fiber
        .frame
        .control_stack
        .push(wrong.fiber.frame.control_stack[0].clone());
    assert!(Engine::from_rollback_image(wrong, &owner).is_err());
}
