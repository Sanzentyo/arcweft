use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    FlowOp, RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFlowOpSeed, RuntimeFunctionDefinitionIdentity, RuntimeFunctionSemanticRole,
    RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteBodySeed, RuntimeFunctionSiteDeclarationSeed,
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind, RuntimeLocalBindingStorage,
    RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource, RuntimeLocalReadSeed,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlanBuilder, RuntimePlanTypeSeed,
};
use crate::pure::RuntimePureFunctionBodyRef;
use std::sync::Arc;

fn source(name: &str) -> RuntimeLocalDeclarationSource {
    let mut hash = blake3::Hasher::new();
    hash.update(b"arcweft.manual-fixture-binding.v1\0");
    hash.update(name.as_bytes());
    RuntimeLocalDeclarationSource::Binding {
        identity: *hash.finalize().as_bytes(),
        declaration: RuntimeLocalBindingDeclaration::new(
            RuntimeLocalBindingKind::PatternBinding,
            true,
            RuntimeLocalBindingStorage::Derived,
        ),
    }
}
fn fixture() -> Arc<crate::plan::RuntimePlan> {
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let integer = RuntimeSemanticTypeId::from_bytes([0xb1; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    integer,
                    RuntimePlanTypeProjection::Signed(crate::value::RuntimeSignedIntWidth::I64),
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(source("scope.unit.parent"), unit),
                RuntimeLocalDeclarationSeed::new(source("scope.numeric.parent"), integer),
                RuntimeLocalDeclarationSeed::new(source("scope.unit.child"), unit),
                RuntimeLocalDeclarationSeed::new(source("scope.numeric.child"), integer),
                RuntimeLocalDeclarationSeed::new(source("scope.unit.result"), unit),
            ],
        )
        .unwrap();
    let ids = admission.local_ids();
    let bind = |ty, local| {
        RuntimePatternSeed::new(
            ty,
            RuntimePatternSeedKind::Bind {
                mutable: true,
                local,
            },
        )
    };
    let declaration = RuntimeFunctionSiteDeclarationSeed {
        definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xb2; 32]),
        role: RuntimeFunctionSemanticRole::Ordinary,
        function_type: None,
        inputs: Box::new([]),
        result: integer,
        body_kind: RuntimeFunctionSiteBodyKind::Executable,
        effects: RuntimeEffectSet::empty(),
    };
    let site = builder.reserve_function_site_seed(declaration).unwrap();
    builder
        .define_function_site_seed(
            &site,
            RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: vec![
                    RuntimeFlowOpSeed::Let {
                        pattern: bind(unit, ids[0].clone()),
                        expr: RuntimeExprSeed::new(
                            unit,
                            RuntimeExprSeedKind::Value(RuntimeValue::Unit),
                        ),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: bind(integer, ids[1].clone()),
                        expr: RuntimeExprSeed::new(
                            integer,
                            RuntimeExprSeedKind::Value(RuntimeValue::i64(7)),
                        ),
                    },
                    RuntimeFlowOpSeed::EnterScope {
                        identity: crate::scope::RuntimeScopeIdentity::Anonymous,
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: bind(unit, ids[2].clone()),
                        expr: RuntimeExprSeed::new(
                            unit,
                            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                ids[0].clone(),
                                RuntimeLocalReadMode::Move,
                            )),
                        ),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: bind(integer, ids[3].clone()),
                        expr: RuntimeExprSeed::new(
                            integer,
                            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                ids[1].clone(),
                                RuntimeLocalReadMode::Move,
                            )),
                        ),
                    },
                    RuntimeFlowOpSeed::ExitScopeBind {
                        pattern: bind(unit, ids[4].clone()),
                        expr: RuntimeExprSeed::new(
                            unit,
                            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                ids[2].clone(),
                                RuntimeLocalReadMode::Move,
                            )),
                        ),
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        integer,
                        RuntimeExprSeedKind::Value(RuntimeValue::i64(9)),
                    )),
                ]
                .into_boxed_slice(),
            }),
        )
        .unwrap();
    Arc::new(builder.finish().unwrap())
}
fn function(plan: &Arc<crate::plan::RuntimePlan>) -> RuntimePureFunctionRef<'_> {
    let (site, _) = plan.function_sites().iter_with_ids().next().unwrap();
    RuntimePureFunctionRef::resolve(plan, site).unwrap()
}
fn let_op(ops: &[FlowOp], index: usize) -> (&RuntimePattern, &RuntimeExpr) {
    let FlowOp::Let { pattern, expr } = &ops[index] else {
        panic!("admitted Let")
    };
    (pattern, expr)
}
fn bound(pattern: &RuntimePattern) -> RuntimeLocalDeclarationId {
    let RuntimePatternKind::Bind { binding, .. } = pattern.kind() else {
        panic!("admitted binder")
    };
    binding.local()
}

#[test]
fn physical_scope_exit_does_not_resurrect_moved_unit_or_numeric_parent_places() {
    let plan = fixture();
    let function = function(&plan);
    let RuntimePureFunctionBodyRef::Executable(body) = function.body else {
        panic!("executable owner")
    };
    let ops = body.ops();
    let mut control =
        RuntimePureControlBindings::new(function, BTreeMap::<RuntimeLocalDeclarationId, ()>::new());
    let (parent, unit) = let_op(ops, 0);
    let value = control.evaluate_unit(unit).unwrap().unwrap();
    control.bind_unit(parent, value).unwrap();
    let (numeric, _) = let_op(ops, 1);
    control.numeric_mut().insert(bound(numeric), ());
    control.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
    control.enter_scope(RuntimeScopeFrameKind::Control);
    let (child, moved) = let_op(ops, 3);
    let value = control.evaluate_unit(moved).unwrap().unwrap();
    control.bind_unit(child, value).unwrap();
    let (_, numeric_move) = let_op(ops, 4);
    control.consume_numeric_expression(numeric_move).unwrap();
    let FlowOp::ExitScopeBind { pattern, expr } = &ops[5] else {
        panic!("admitted scope continuation")
    };
    let value = control.evaluate_unit(expr).unwrap().unwrap();
    control
        .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
        .unwrap();
    control.bind_unit(pattern, value).unwrap();
    assert_eq!(control.scope_count(), 0);
    assert!(
        matches!(control.evaluate_unit(moved),Err(RuntimeEvalError::UninitializedLocal(local)) if local==bound(parent))
    );
    assert!(
        matches!(control.consume_numeric_expression(numeric_move),Err(RuntimeEvalError::UninitializedLocal(local)) if local==bound(numeric))
    );
    assert!(
        matches!(control.evaluate_unit(expr),Err(RuntimeEvalError::UninitializedLocal(local)) if local==bound(child))
    );
}

#[test]
fn physical_unit_whole_place_reinitialization_survives_its_child_scope_exit() {
    let plan = fixture();
    let function = function(&plan);
    let RuntimePureFunctionBodyRef::Executable(body) = function.body else {
        panic!("executable owner")
    };
    let ops = body.ops();
    let mut control = RuntimePureControlBindings::new(function, ());
    let (parent, literal) = let_op(ops, 0);
    let value = control.evaluate_unit(literal).unwrap().unwrap();
    control.bind_unit(parent, value).unwrap();
    control.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
    let (_, moved) = let_op(ops, 3);
    control.evaluate_unit(moved).unwrap().unwrap();
    let value = control.evaluate_unit(literal).unwrap().unwrap();
    control.bind_unit(parent, value).unwrap();
    control
        .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
        .unwrap();
    assert!(control.evaluate_unit(moved).unwrap().is_some());
    assert!(
        matches!(control.evaluate_unit(moved),Err(RuntimeEvalError::UninitializedLocal(local)) if local==bound(parent))
    );
}

#[test]
fn physical_control_exit_uses_its_exact_frame_and_rejects_a_missing_lexical_owner() {
    let plan = fixture();
    let function = function(&plan);
    let mut control = RuntimePureControlBindings::new(function, ());
    let outer = control.enter_scope(RuntimeScopeFrameKind::EmittedLexical);
    let first = control.enter_scope(RuntimeScopeFrameKind::Control);
    let second = control.enter_scope(RuntimeScopeFrameKind::Control);
    assert!(matches!(
        control.exit_scope(RuntimeScopeExitTarget::Frame(first)),
        Err(RuntimeEvalError::UnsupportedPure { .. })
    ));
    assert_eq!(control.scope_count(), 3);
    control
        .exit_scope(RuntimeScopeExitTarget::EmittedLexical)
        .unwrap();
    assert!(!control.contains_scope(outer));
    assert!(!control.contains_scope(first));
    assert!(!control.contains_scope(second));
    assert!(matches!(
        control.exit_scope(RuntimeScopeExitTarget::EmittedLexical),
        Err(RuntimeEvalError::UnsupportedPure { .. })
    ));
}

#[test]
fn physical_unit_scope_transfers_match_the_original_native_function_value() {
    let plan = fixture();
    let function = function(&plan);
    let request =
        crate::pure::PureFunctionRequest::try_new(Arc::clone(&plan), function.id(), []).unwrap();
    assert_eq!(function.exact_scalar_completion_control_ops(), Some(7));
    let native = crate::pure::VmPureFunctionBackend
        .evaluate_invocation(&request, crate::step::RuntimeStepBudget { max_ops: 32 })
        .unwrap();
    assert_eq!(native.value, RuntimeValue::i64(9));
    let physical = crate::pure::AotPureFunctionBackend::new()
        .compile_i64(&request)
        .unwrap();
    let (actual, _) = physical.call().unwrap();
    assert_eq!(RuntimeValue::i64(actual), native.value);
}
#[test]
fn physical_unit_publication_rejects_a_numeric_destination_row() {
    let plan = fixture();
    let function = function(&plan);
    let RuntimePureFunctionBodyRef::Executable(body) = function.body else {
        panic!("executable owner")
    };
    let mut control = RuntimePureControlBindings::new(function, ());
    let (pattern, literal) = let_op(body.ops(), 0);
    let (numeric, _) = let_op(body.ops(), 1);
    let value = control.evaluate_unit(literal).unwrap().unwrap();
    let mut destination = control.unit_destination(pattern).unwrap();
    destination.ty = numeric.ty();
    assert!(
        matches!(control.publish_unit(destination,value),Err(RuntimeEvalError::InvalidExpressionType(ty)) if ty==numeric.ty())
    );
}
