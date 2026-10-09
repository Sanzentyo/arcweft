use super::*;
use arcweft_core::effect::{LineEffectRequest, RuntimeLog};
use arcweft_core::engine::Engine;
use arcweft_core::entry::{
    EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
};
use arcweft_core::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeEffectSet, RuntimeEntryKind, RuntimeEntrySpec,
    RuntimeEntryTarget, RuntimeExecutableBodySeed, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFlowOpSeed, RuntimeFlowSchema, RuntimeFlowSeed, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionSiteDeclarationSeed, RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind,
    RuntimeLocalBindingStorage, RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource,
    RuntimeLocalReadSeed, RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan,
    RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use arcweft_core::scope::RuntimeScopeIdentity;
use arcweft_core::step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions};
use arcweft_core::value::{RuntimeLocalReadMode, RuntimeValue};
use std::sync::Arc;

fn local_source(name: &str) -> RuntimeLocalDeclarationSource {
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(format!("arcweft-runtime-plan.fixture.lexical_scope.{name}").as_bytes());
    RuntimeLocalDeclarationSource::Binding {
        identity: *identity.finalize().as_bytes(),
        declaration: RuntimeLocalBindingDeclaration::new(
            RuntimeLocalBindingKind::PatternBinding,
            false,
            RuntimeLocalBindingStorage::Derived,
        ),
    }
}
fn log(message: &str) -> LineEffectRequest {
    LineEffectRequest::Log(RuntimeLog {
        level: "info".into(),
        message: message.into(),
        fields: Vec::new(),
    })
}
#[derive(Clone, Copy)]
enum BodyLayout {
    Flat,
    AggregateFallthrough,
}
fn plan(
    condition: bool,
) -> (
    RuntimePlan,
    FlowRuntimeId,
    arcweft_core::runtime_id::RuntimeLocalDeclarationId,
) {
    plan_with_layout(condition, BodyLayout::Flat)
}
fn plan_with_layout(
    condition: bool,
    layout: BodyLayout,
) -> (
    RuntimePlan,
    FlowRuntimeId,
    arcweft_core::runtime_id::RuntimeLocalDeclarationId,
) {
    let unit = RuntimeCheckedType::Unit.semantic_identity_digest();
    let boolean = RuntimeCheckedType::Bool.semantic_identity_digest();
    let integer = RuntimeSemanticTypeId::from_bytes([0xa2; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    integer,
                    RuntimePlanTypeProjection::Signed(
                        arcweft_core::value::RuntimeSignedIntWidth::I64,
                    ),
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(local_source("inside"), integer),
                RuntimeLocalDeclarationSeed::new(local_source("result"), unit),
            ],
        )
        .unwrap();
    let inside = admission.local_ids()[0].clone();
    let result = admission.local_ids()[1].clone();
    let bind = |ty, local| {
        RuntimePatternSeed::new(
            ty,
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        )
    };
    let unit_value = || RuntimeExprSeed::new(unit, RuntimeExprSeedKind::Value(RuntimeValue::Unit));
    let branch = || {
        let mut ops = vec![
            RuntimeFlowOpSeed::ExitScopeBind {
                pattern: bind(unit, result.clone()),
                expr: unit_value(),
            },
            RuntimeFlowOpSeed::Let {
                pattern: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
                expr: RuntimeExprSeed::new(
                    unit,
                    RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                        result.clone(),
                        RuntimeLocalReadMode::Move,
                    )),
                ),
            },
            RuntimeFlowOpSeed::Effect(arcweft_core::plan::RuntimeLineEffectSeed::Static(log(
                "after",
            ))),
        ];
        if matches!(layout, BodyLayout::Flat) {
            ops.push(RuntimeFlowOpSeed::ReturnExpr(unit_value()));
        }
        ops
    };
    let flow = FlowRuntimeId::canonical("lexical_scope").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .unwrap();
    builder
        .push_flow_executable(RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([0xf0; 32]),
            controller: None,
        })
        .unwrap();
    let effects = RuntimeEffectSet::try_from_effects([arcweft_id::EffectId::log_write()]).unwrap();
    let identity =
        RuntimeScopeIdentity::Named(arcweft_id::DeclarationName::try_new("owned_scope").unwrap());
    let inner = vec![
        RuntimeFlowOpSeed::Let {
            pattern: bind(integer, inside),
            expr: RuntimeExprSeed::new(integer, RuntimeExprSeedKind::Value(RuntimeValue::i64(9))),
        },
        RuntimeFlowOpSeed::RegisterCleanup {
            key: "scope_cleanup".into(),
            effect: arcweft_core::plan::RuntimeLineEffectSeed::Static(log("closed")),
        },
        RuntimeFlowOpSeed::If {
            condition: RuntimeExprSeed::new(
                boolean,
                RuntimeExprSeedKind::Value(RuntimeValue::Bool(condition)),
            ),
            then_ops: branch(),
            else_ops: branch(),
        },
    ];
    let ops = match layout {
        BodyLayout::Flat => std::iter::once(RuntimeFlowOpSeed::EnterScope { identity })
            .chain(inner)
            .collect::<Vec<_>>(),
        BodyLayout::AggregateFallthrough => vec![
            RuntimeFlowOpSeed::Scope {
                identity,
                body: inner,
            },
            RuntimeFlowOpSeed::ReturnExpr(unit_value()),
        ],
    };
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xa3; 32]),
                None,
                Box::new([]),
                unit,
                effects.clone(),
            ),
            RuntimeExecutableBodySeed {
                effects,
                ops: ops.into_boxed_slice(),
            },
        ))
        .unwrap();
    builder
        .push_entry(RuntimeEntrySpec {
            id: EntryRuntimeId::canonical("scope.lexical").unwrap(),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([0xa4; 32]),
            target: RuntimeEntryTarget::Flow(flow.clone()),
            roles: RuntimeEntryRoles::None,
        })
        .unwrap();
    let plan = builder.finish().unwrap();
    let op = match &plan.flows()[0].body().ops()[0] {
        arcweft_core::plan::FlowOp::Scope { body, .. } => &body[0],
        _ => &plan.flows()[0].body().ops()[1],
    };
    let inside = match op {
        arcweft_core::plan::FlowOp::Let { pattern, .. } => match pattern.kind() {
            arcweft_core::pattern::RuntimePatternKind::Bind { binding, .. } => binding.local(),
            _ => panic!("actual admitted local"),
        },
        _ => panic!("actual owning Let"),
    };
    (plan, flow, inside)
}

#[test]
fn emitted_unit_scope_exits_before_outer_continuation_and_cleanup_native_awbc() {
    for condition in [false, true] {
        let (plan, flow, inside) = plan(condition);
        let lower = AwbcLowerer::new(&plan, &DialogueContentCatalog::new(), "unit-scope.arcw")
            .lower()
            .unwrap();
        assert!(lower.diagnostics.is_empty(), "{:?}", lower.diagnostics);
        let encoded = lower.program.encode_canonical().unwrap();
        let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
            &encoded,
            arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
        )
        .unwrap();
        assert_eq!(decoded.encode_canonical().unwrap(), encoded);
        decoded
            .verify(
                arcweft_core::awbc::verify::AwbcVerifyBudget::default(),
                Default::default(),
            )
            .unwrap();
        let mut native = Engine::for_flow(plan, &flow).unwrap();
        let options = RuntimeStepOptions {
            mode: RuntimeStepMode::OneOp,
            budget: RuntimeStepBudget { max_ops: 1 },
            ..Default::default()
        };
        let mut native_logs = Vec::new();
        let mut outside = false;
        for _ in 0..64 {
            let step = native.step(Default::default(), options);
            assert!(
                step.output.diagnostics.is_empty(),
                "{:?}",
                step.output.diagnostics
            );
            for effect in step.output.effects.line {
                if let LineEffectRequest::Log(log) = effect {
                    if log.message == "after" {
                        assert!(
                            native.fiber().env.get(inside).is_none(),
                            "emitted scope locals must be gone before the outside continuation"
                        );
                        outside = true;
                    }
                    native_logs.push(log.message);
                }
            }
            if matches!(
                step.fiber_status,
                arcweft_core::engine::FlowFiberStatus::Done(_)
            ) {
                break;
            }
        }
        assert!(outside);
        assert_eq!(
            native_logs,
            ["closed", "after"],
            "the emitted scope's cleanup runs once before its outside continuation"
        );
        let mut product = arcweft_core::awbc::product_step::AwbcProductStepExecutor::for_entry_arc(
            Arc::new(decoded),
            arcweft_core::awbc::schema::AwbcEntryId(0),
            1,
        )
        .unwrap();
        let mut awbc_logs = Vec::new();
        for _ in 0..64 {
            let step = product.step(Default::default(), options);
            assert!(
                step.output.diagnostics.is_empty(),
                "{:?}",
                step.output.diagnostics
            );
            for effect in step.output.effects.line {
                if let LineEffectRequest::Log(log) = effect {
                    awbc_logs.push(log.message)
                }
            }
            if matches!(
                step.fiber_status,
                arcweft_core::engine::FlowFiberStatus::Done(_)
            ) {
                break;
            }
        }
        assert_eq!(
            awbc_logs, native_logs,
            "canonical AWBC retains the same actual scope cleanup owner/order"
        );
    }
}

#[test]
fn awbc_scope_exit_rejects_a_lexical_target_while_its_generated_child_is_active() {
    let (plan, _, _) = plan(true);
    let lower = AwbcLowerer::new(&plan, &DialogueContentCatalog::new(), "unit-scope.arcw")
        .lower()
        .unwrap();
    assert!(lower.diagnostics.is_empty(), "{:?}", lower.diagnostics);
    lower
        .program
        .verify(
            arcweft_core::awbc::verify::AwbcVerifyBudget::default(),
            Default::default(),
        )
        .unwrap();
    let mut program = lower.program;
    let lexical = program
        .frame_layouts
        .iter()
        .flat_map(|layout| layout.scopes.iter().enumerate())
        .find_map(|(index, scope)| {
            (scope.kind == arcweft_core::scope::RuntimeScopeFrameKind::EmittedLexical).then_some(
                arcweft_core::awbc::schema::AwbcScopeId(u32::try_from(index).unwrap()),
            )
        })
        .unwrap();
    let close=program.instructions.iter_mut().find(|instruction|matches!(instruction,arcweft_core::awbc::schema::AwbcInstruction::ExitScope {scope} if *scope!=lexical)).unwrap();
    *close = arcweft_core::awbc::schema::AwbcInstruction::ExitScope { scope: lexical };
    assert!(
        matches!(program.verify(arcweft_core::awbc::verify::AwbcVerifyBudget::default(),Default::default()),Err(arcweft_core::awbc::verify::AwbcVerifyError::ScopeDiscipline {message,..}) if message==format!("scope {} is not the active scope",lexical.0))
    );
}

#[test]
fn awbc_scope_admission_rejects_an_authored_namespace_on_a_generated_control_frame() {
    let (plan, _, _) = plan(true);
    let lower = AwbcLowerer::new(&plan, &DialogueContentCatalog::new(), "unit-scope.arcw")
        .lower()
        .unwrap();
    lower
        .program
        .verify(
            arcweft_core::awbc::verify::AwbcVerifyBudget::default(),
            Default::default(),
        )
        .unwrap();
    let mut program = lower.program;
    let (layout_index, scope_index) = program
        .frame_layouts
        .iter()
        .enumerate()
        .find_map(|(layout, rows)| {
            rows.scopes
                .iter()
                .position(|row| row.kind == arcweft_core::scope::RuntimeScopeFrameKind::Control)
                .map(|scope| (layout, scope))
        })
        .unwrap();
    program.frame_layouts[layout_index].scopes[scope_index].identity =
        RuntimeScopeIdentity::Named(arcweft_id::DeclarationName::try_new("authored").unwrap());
    assert!(
        matches!(program.verify(arcweft_core::awbc::verify::AwbcVerifyBudget::default(),Default::default()),Err(arcweft_core::awbc::verify::AwbcVerifyError::InvalidInvariant {at,message}) if at==format!("frame layout {layout_index} scope {scope_index}") && message=="generated control scope cannot carry an authored namespace")
    );
}

#[test]
fn aggregate_emitted_scope_cancels_its_own_implicit_close_before_outer_continuation() {
    for condition in [false, true] {
        let (plan, flow, inside) = plan_with_layout(condition, BodyLayout::AggregateFallthrough);
        let lower = AwbcLowerer::new(
            &plan,
            &DialogueContentCatalog::new(),
            "aggregate-scope.arcw",
        )
        .lower()
        .unwrap();
        assert!(lower.diagnostics.is_empty(), "{:?}", lower.diagnostics);
        let encoded = lower.program.encode_canonical().unwrap();
        let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
            &encoded,
            arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
        )
        .unwrap();
        assert_eq!(decoded.encode_canonical().unwrap(), encoded);
        decoded
            .verify(
                arcweft_core::awbc::verify::AwbcVerifyBudget::default(),
                Default::default(),
            )
            .unwrap();
        let options = RuntimeStepOptions {
            mode: RuntimeStepMode::OneOp,
            budget: RuntimeStepBudget { max_ops: 1 },
            ..Default::default()
        };
        let mut engine = Engine::for_flow(plan, &flow).unwrap();
        let mut actual = Vec::new();
        for _ in 0..64 {
            let step = engine.step(Default::default(), options);
            assert!(step.output.diagnostics.is_empty(), "{step:?}");
            for effect in step.output.effects.line {
                if let LineEffectRequest::Log(log) = effect {
                    if log.message == "after" {
                        assert!(engine.fiber().env.get(inside).is_none());
                        assert!(!engine.fiber().pending_ops.iter().any(|op| matches!(
                            op,
                            arcweft_core::plan::FlowOp::ExitScheduledScope { .. }
                        )));
                    }
                    actual.push(log.message);
                }
            }
            if matches!(
                step.fiber_status,
                arcweft_core::engine::FlowFiberStatus::Done(_)
            ) {
                break;
            }
        }
        assert_eq!(actual, ["closed", "after"]);
        let mut product = arcweft_core::awbc::product_step::AwbcProductStepExecutor::for_entry_arc(
            Arc::new(decoded),
            arcweft_core::awbc::schema::AwbcEntryId(0),
            1,
        )
        .unwrap();
        let mut bytecode = Vec::new();
        for _ in 0..64 {
            let step = product.step(Default::default(), options);
            assert!(step.output.diagnostics.is_empty(), "{step:?}");
            for effect in step.output.effects.line {
                if let LineEffectRequest::Log(log) = effect {
                    bytecode.push(log.message)
                }
            }
            if matches!(
                step.fiber_status,
                arcweft_core::engine::FlowFiberStatus::Done(_)
            ) {
                break;
            }
        }
        assert_eq!(bytecode, actual);
    }
}
