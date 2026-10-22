use crate::engine::Engine;
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::*;
use crate::step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions};
use crate::value::{
    RuntimeBinaryOp, RuntimeCallArgumentMode, RuntimeLocalReadMode, RuntimeSignedIntWidth,
    RuntimeStandardMapFamily, RuntimeValue,
};
use std::sync::Arc;

fn integer() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64).semantic_identity_digest()
}
fn unit() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Unit.semantic_identity_digest()
}
fn ty(byte: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([byte; 32])
}
fn program() -> arcweft_id::runtime_program::RuntimePureProgramId {
    arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([0x98; 32])
}
fn value(number: i64) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        integer(),
        RuntimeExprSeedKind::Value(RuntimeValue::i64(number)),
    )
}
fn read(
    local: &RuntimeLocalSeedId,
    ty: RuntimeSemanticTypeId,
    mode: RuntimeLocalReadMode,
) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        ty,
        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(local.clone(), mode)),
    )
}
fn bind(
    local: &RuntimeLocalSeedId,
    ty: RuntimeSemanticTypeId,
    mutable: bool,
) -> RuntimePatternSeed {
    RuntimePatternSeed::new(
        ty,
        RuntimePatternSeedKind::Bind {
            local: local.clone(),
            mutable,
        },
    )
}

#[derive(Clone, Copy)]
enum Mode {
    Folded,
    Source,
    FoldedTransform,
    SourceTransform,
    Empty,
    WrongReceiver,
    ExtraSuffix,
}

fn fixture(mode: Mode) -> Result<RuntimePlan, RuntimePlanBuildError> {
    let mut builder = RuntimePlanBuilder::new();
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([0x80; 32]);
    let mut locals = vec![RuntimeLocalDeclarationSeed::in_function(
        RuntimeLocalDeclarationSource::Parameter(parameter),
        integer(),
        ty(0x81),
    )];
    for (index, value_type, mutable) in [
        (1, ty(0x81), false),
        (2, ty(0x82), false),
        (3, ty(0x82), true),
        (4, integer(), false),
        (5, integer(), false),
    ] {
        locals.push(RuntimeLocalDeclarationSeed::new(
            RuntimeLocalDeclarationSource::Binding {
                identity: [index; 32],
                declaration: RuntimeLocalBindingDeclaration::new(
                    RuntimeLocalBindingKind::PatternBinding,
                    mutable,
                    RuntimeLocalBindingStorage::Derived,
                ),
            },
            value_type,
        ));
    }
    let admitted = builder.admit_type_batch(
        [
            RuntimePlanTypeSeed::new(
                integer(),
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
            ),
            RuntimePlanTypeSeed::new(unit(), RuntimePlanTypeProjection::Unit),
            RuntimePlanTypeSeed::new(
                ty(0x81),
                RuntimePlanTypeProjection::Function {
                    contract: RuntimeFunctionTypeContract::new(
                        RuntimeTypeBinder::new(0, 0, 0),
                        crate::effect_row::EffectPredicate::unconstrained(),
                        crate::effect_row::EffectFormula::empty(),
                    ),
                    parameters: Box::new([integer()]),
                    result: integer(),
                },
            ),
            RuntimePlanTypeSeed::new(
                ty(0x82),
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item: integer(),
                },
            ),
            RuntimePlanTypeSeed::new(ty(0x83), RuntimePlanTypeProjection::Iterator(integer())),
            RuntimePlanTypeSeed::new(
                ty(0x84),
                RuntimePlanTypeProjection::Tuple(Box::new([integer()])),
            ),
            RuntimePlanTypeSeed::new(
                ty(0x85),
                RuntimePlanTypeProjection::Option {
                    item: integer(),
                    some_payload: ty(0x84),
                },
            ),
            RuntimePlanTypeSeed::new(
                ty(0x86),
                RuntimePlanTypeProjection::Tuple(Box::new([ty(0x83), ty(0x85)])),
            ),
        ],
        locals,
    )?;
    let locals = admitted.local_ids();
    let (parameter_local, mapping, source, result, item, mapped) = (
        &locals[0], &locals[1], &locals[2], &locals[3], &locals[4], &locals[5],
    );
    let callback = builder.reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
        definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x90; 32]),
        role: RuntimeFunctionSemanticRole::Ordinary,
        function_type: Some(ty(0x81)),
        inputs: Box::new([RuntimeFunctionInputBindingSeed {
            transfer: RuntimeFunctionInputTransfer::Formal,
            origin: RuntimeFunctionInputOrigin::Parameter(parameter),
            source: RuntimeFunctionInputSource::Parameter {
                position: 0,
                passing: RuntimeFunctionParameterPassing::Value,
            },
            input_local: parameter_local.clone(),
            pattern: bind(parameter_local, integer(), false),
            ownership: RuntimeValueOwnershipRequirement::Unrestricted,
            unrestricted_bindings: Box::new([parameter_local.clone()]),
        }]),
        result: integer(),
        body_kind: RuntimeFunctionSiteBodyKind::Expression,
        effects: RuntimeEffectSet::empty(),
    })?;
    builder.define_function_site_seed(
        &callback,
        RuntimeExprSeed::new(
            integer(),
            RuntimeExprSeedKind::Binary {
                lhs: Box::new(read(parameter_local, integer(), RuntimeLocalReadMode::Copy)),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(value(1)),
            },
        ),
    )?;
    let mut mapping_body = vec![
        RuntimeFlowOpSeed::Let {
            pattern: bind(result, ty(0x82), true),
            expr: RuntimeExprSeed::new(ty(0x82), RuntimeExprSeedKind::BracketSeq(Box::new([]))),
        },
        RuntimeFlowOpSeed::For {
            pattern: bind(item, integer(), false),
            source: read(source, ty(0x82), RuntimeLocalReadMode::Move),
            evidence: RuntimeIteratorEvidenceSeed::Builtin(RuntimeBuiltinIteratorEvidenceSeed {
                family: RuntimeBuiltinIteratorFamily::Vec,
                item: integer(),
                iterator: ty(0x83),
                next_value: ty(0x85),
                step: ty(0x86),
            }),
            body: vec![
                RuntimeFlowOpSeed::ApplyFunction {
                    callee: read(mapping, ty(0x81), RuntimeLocalReadMode::Copy),
                    args: Box::new([RuntimeCallArgumentSeed::new(
                        read(item, integer(), RuntimeLocalReadMode::Copy),
                        RuntimeCallArgumentMode::Value,
                        0,
                    )]),
                    result: bind(mapped, integer(), false),
                },
                RuntimeFlowOpSeed::Let {
                    pattern: RuntimePatternSeed::new(unit(), RuntimePatternSeedKind::Discard),
                    expr: RuntimeExprSeed::new(
                        unit(),
                        RuntimeExprSeedKind::SequencePush {
                            place: RuntimeMutablePlaceSeed::Local(result.clone()),
                            value: Box::new(read(mapped, integer(), RuntimeLocalReadMode::Move)),
                        },
                    ),
                },
            ],
        },
    ];
    if matches!(mode, Mode::FoldedTransform) {
        mapping_body.insert(0, RuntimeFlowOpSeed::Noop);
    }
    let receiver = if matches!(mode, Mode::WrongReceiver) {
        source
    } else {
        result
    };
    let mut consumption = vec![RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
        integer(),
        RuntimeExprSeedKind::Sum {
            source: Box::new(read(receiver, ty(0x82), RuntimeLocalReadMode::Move)),
        },
    ))];
    if matches!(mode, Mode::Empty) {
        consumption.clear();
    }
    if matches!(mode, Mode::ExtraSuffix) {
        consumption.push(RuntimeFlowOpSeed::Noop);
    }
    let mut ops = vec![
        RuntimeFlowOpSeed::Let {
            pattern: bind(mapping, ty(0x81), false),
            expr: RuntimeExprSeed::new(
                ty(0x81),
                RuntimeExprSeedKind::Function {
                    site: callback,
                    captures: Box::new([]),
                },
            ),
        },
        RuntimeFlowOpSeed::Let {
            pattern: bind(source, ty(0x82), false),
            expr: RuntimeExprSeed::new(
                ty(0x82),
                RuntimeExprSeedKind::BracketSeq(Box::new([value(1), value(2)])),
            ),
        },
    ];
    if matches!(mode, Mode::Folded | Mode::FoldedTransform) {
        ops.push(RuntimeFlowOpSeed::StandardMap(
            RuntimeStandardMapControlSeed::collection(
                RuntimeStandardMapFamily::Vec,
                source.clone(),
                mapping.clone(),
                result.clone(),
                mapping_body,
            ),
        ));
        ops.extend(consumption);
    } else {
        let mut control = RuntimeStandardMapControlSeed::sum(
            RuntimeStandardMapFamily::Vec,
            source.clone(),
            mapping.clone(),
            result.clone(),
            mapping_body,
            consumption,
        );
        assert_eq!(
            RuntimeFlowOpSeed::free_locals(&[RuntimeFlowOpSeed::StandardMap(control.clone())])
                .as_ref(),
            &[source.clone(), mapping.clone()]
        );
        if matches!(mode, Mode::SourceTransform) {
            control = control
                .try_map_bodies::<std::convert::Infallible>(|mut body| {
                    if body.len() == 2 {
                        body.insert(0, RuntimeFlowOpSeed::Noop);
                    }
                    Ok(body)
                })
                .unwrap();
        }
        ops.push(RuntimeFlowOpSeed::StandardMap(control));
    }
    assert_eq!(RuntimeFlowOpSeed::free_locals(&ops).as_ref(), &[]);
    let root = builder.reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
        definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x91; 32]),
        role: RuntimeFunctionSemanticRole::Ordinary,
        function_type: None,
        inputs: Box::new([]),
        result: integer(),
        body_kind: RuntimeFunctionSiteBodyKind::Executable,
        effects: RuntimeEffectSet::empty(),
    })?;
    builder.define_function_site_seed(
        &root,
        RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
            effects: RuntimeEffectSet::empty(),
            ops: ops.into_boxed_slice(),
        }),
    )?;
    builder.push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
        program: program(),
        site: root,
    })?;
    builder.finish()
}

fn execute(plan: RuntimePlan) -> (RuntimeValue, crate::pure::PureFunctionStats) {
    let mut engine = Engine::for_program_invocation(Arc::new(plan), program(), vec![]).unwrap();
    let report = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 256 },
            ..RuntimeStepOptions::default()
        },
    );
    assert!(
        report.output.diagnostics.is_empty(),
        "{:?}",
        report.output.diagnostics
    );
    let (actual_program, result) = engine
        .take_program_result()
        .unwrap()
        .expect("the actual owned program completed");
    assert_eq!(actual_program, program());
    (result, engine.evaluation_stats().clone())
}

#[test]
fn complete_map_source_reuses_the_original_sum_admission_and_executes_the_original_callback() {
    for (ordinary, source) in [
        (Mode::Folded, Mode::Source),
        (Mode::FoldedTransform, Mode::SourceTransform),
    ] {
        let ordinary = fixture(ordinary).unwrap();
        let source = fixture(source).unwrap();
        assert_eq!(source.function_sites(), ordinary.function_sites());
        assert_eq!(
            source.executable_semantic_digest(),
            ordinary.executable_semantic_digest()
        );
        let ordinary = execute(ordinary);
        let source = execute(source);
        assert_eq!(source, ordinary);
        assert_eq!(source.0, RuntimeValue::i64(5));
    }
}

#[test]
fn complete_map_source_rejects_missing_foreign_and_partial_consumption_before_publication() {
    for mode in [Mode::Empty, Mode::WrongReceiver, Mode::ExtraSuffix] {
        assert!(matches!(
            fixture(mode),
            Err(RuntimePlanBuildError::InvalidStandardMapConsumption { .. })
        ));
    }
}
