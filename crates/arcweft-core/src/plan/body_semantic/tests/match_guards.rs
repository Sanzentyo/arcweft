use crate::engine::Engine;
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::body_semantic::RuntimeBodySemanticContext;
use crate::plan::construction::task_coordinates::RuntimeTaskPlanCoordinateOwner;
use crate::plan::{
    FlowOp, RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFlowMatchArmSeed, RuntimeFlowMatchGuardSeed, RuntimeFlowOpSeed,
    RuntimeFunctionDefinitionIdentity, RuntimeFunctionSemanticRole, RuntimeFunctionSiteBodyKind,
    RuntimeFunctionSiteBodySeed, RuntimeFunctionSiteDeclarationSeed,
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind, RuntimeLocalBindingStorage,
    RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource, RuntimePatternSeed,
    RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuildError, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimePureProgramBindingSeed,
    RuntimeTaskPlanSealLimits,
};
use crate::step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions};
use crate::task::semantic::TaskSemanticMeter;
use crate::value::{RuntimeSignedIntWidth, RuntimeValue};
use arcweft_id::runtime_program::RuntimePureProgramId;
use std::sync::Arc;

fn boolean() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Bool.semantic_identity_digest()
}

fn integer() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest()
}

fn never(byte: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([byte; 32])
}

fn program() -> RuntimePureProgramId {
    RuntimePureProgramId::from_checked_digest([0xb1; 32])
}

fn value(number: i64) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        integer(),
        RuntimeExprSeedKind::Value(RuntimeValue::i64(number)),
    )
}

fn fixture(
    result: RuntimeSemanticTypeId,
    condition: Option<bool>,
    padding: bool,
) -> Result<(RuntimePlan, RuntimeTaskPlanCoordinateOwner), RuntimePlanBuildError> {
    let mut builder = RuntimePlanBuilder::new();
    let mut types = Vec::new();
    if padding {
        types.push(RuntimePlanTypeSeed::new(
            RuntimeCheckedType::Unit.semantic_identity_digest(),
            RuntimePlanTypeProjection::Unit,
        ));
    }
    types.extend([
        RuntimePlanTypeSeed::new(boolean(), RuntimePlanTypeProjection::Bool),
        RuntimePlanTypeSeed::new(
            integer(),
            RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
        ),
        RuntimePlanTypeSeed::new(never(0xb2), RuntimePlanTypeProjection::Never),
        RuntimePlanTypeSeed::new(never(0xb3), RuntimePlanTypeProjection::Never),
    ]);
    let admitted = builder.admit_type_batch(
        types,
        [RuntimeLocalDeclarationSeed::new(
            RuntimeLocalDeclarationSource::Binding {
                identity: [0xb4; 32],
                declaration: RuntimeLocalBindingDeclaration::new(
                    RuntimeLocalBindingKind::PatternBinding,
                    false,
                    RuntimeLocalBindingStorage::Derived,
                ),
            },
            boolean(),
        )],
    )?;
    let candidate = admitted.local_ids()[0].clone();
    let owner = builder.task_coordinate_owner(0);
    let root = builder.reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
        definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0xb5; 32]),
        role: RuntimeFunctionSemanticRole::Ordinary,
        function_type: None,
        inputs: Box::new([]),
        result: integer(),
        body_kind: RuntimeFunctionSiteBodyKind::Executable,
        effects: RuntimeEffectSet::empty(),
    })?;
    let discard = || RuntimePatternSeed::new(boolean(), RuntimePatternSeedKind::Discard);
    builder.define_function_site_seed(
        &root,
        RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
            effects: RuntimeEffectSet::empty(),
            ops: Box::new([RuntimeFlowOpSeed::Match {
                scrutinee: RuntimeExprSeed::new(
                    boolean(),
                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                ),
                arms: vec![
                    RuntimeFlowMatchArmSeed {
                        pattern: discard(),
                        guard: Some(RuntimeFlowMatchGuardSeed {
                            candidate,
                            result,
                            condition: condition.map(|value| {
                                RuntimeExprSeed::new(
                                    boolean(),
                                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)),
                                )
                            }),
                            copy_locals: Box::new([]),
                            ops: if condition.is_none() {
                                vec![RuntimeFlowOpSeed::ReturnExpr(value(7))]
                            } else {
                                vec![]
                            },
                        }),
                        ops: vec![RuntimeFlowOpSeed::ReturnExpr(value(42))],
                    },
                    RuntimeFlowMatchArmSeed {
                        pattern: discard(),
                        guard: None,
                        ops: vec![RuntimeFlowOpSeed::ReturnExpr(value(100))],
                    },
                ],
            }]),
        }),
    )?;
    builder.push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
        program: program(),
        site: root,
    })?;
    Ok((builder.finish()?, owner))
}

fn declared_result(plan: &RuntimePlan) -> RuntimeSemanticTypeId {
    let function = plan.function_sites().iter().next().unwrap();
    let FlowOp::Match { arms, .. } = &function.body().executable().unwrap().ops()[0] else {
        panic!("original Match body was not retained")
    };
    plan.type_table()
        .get(arms[0].guard.as_ref().unwrap().result)
        .unwrap()
        .semantic_identity()
}

fn function_digest(plan: &RuntimePlan, owner: &RuntimeTaskPlanCoordinateOwner) -> blake3::Hash {
    let function = plan.function_sites().iter_with_ids().next().unwrap().0;
    let context = RuntimeBodySemanticContext::new(plan);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    context
        .producer_function(
            &mut meter,
            function,
            owner,
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap()
        .executable_row_digest(&context, &mut meter)
        .unwrap()
}

fn execute(plan: RuntimePlan) -> RuntimeValue {
    let mut engine = Engine::for_program_invocation(Arc::new(plan), program(), vec![]).unwrap();
    let report = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 64 },
            ..RuntimeStepOptions::default()
        },
    );
    assert!(
        report.output.diagnostics.is_empty(),
        "{:?}",
        report.output.diagnostics
    );
    let (identity, result) = engine
        .take_program_result()
        .unwrap()
        .expect("guard execution must complete");
    assert_eq!(identity, program());
    result
}

#[test]
fn non_returning_guard_retains_its_original_type_in_the_function_transcript_and_seal() {
    let (first, first_owner) = fixture(never(0xb2), None, false).unwrap();
    let (second, second_owner) = fixture(never(0xb3), None, false).unwrap();
    assert_eq!(first.type_table(), second.type_table());
    assert_eq!(declared_result(&first), never(0xb2));
    assert_eq!(declared_result(&second), never(0xb3));
    assert_ne!(
        function_digest(&first, &first_owner),
        function_digest(&second, &second_owner)
    );
    assert_ne!(
        first.executable_semantic_digest(),
        second.executable_semantic_digest()
    );
    assert_eq!(execute(first), RuntimeValue::i64(7));
    assert_eq!(execute(second), RuntimeValue::i64(7));
}

#[test]
fn guard_result_transcripts_resolve_types_and_keep_actual_control_transfers() {
    for (result, condition, expected) in [
        (never(0xb2), None, 7),
        (boolean(), Some(true), 42),
        (boolean(), Some(false), 100),
    ] {
        let (ordinary, owner) = fixture(result, condition, false).unwrap();
        let (padded, padded_owner) = fixture(result, condition, true).unwrap();
        assert_eq!(declared_result(&ordinary), result);
        assert_eq!(declared_result(&padded), result);
        assert_eq!(
            function_digest(&ordinary, &owner),
            function_digest(&padded, &padded_owner)
        );
        assert_eq!(execute(ordinary), RuntimeValue::i64(expected));
        assert_eq!(execute(padded), RuntimeValue::i64(expected));
    }
}

#[test]
fn guard_result_admission_rejects_non_never_divergence_and_mismatched_bool_before_sealing() {
    assert!(matches!(
        fixture(boolean(), None, false),
        Err(RuntimePlanBuildError::InvalidTypeProjection {
            context: "non-returning guard",
            ..
        })
    ));
    assert!(matches!(
        fixture(integer(), Some(true), false),
        Err(RuntimePlanBuildError::TypeMismatch {
            context: "flow match guard result",
            ..
        })
    ));
}
