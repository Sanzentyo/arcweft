use super::*;
use crate::{
    awbc::schema::*,
    entry::schema::{RuntimeCodecUse, RuntimeFieldCodecUse},
    plan::{
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimePlan, RuntimePlanBuilder,
        RuntimePlanRecordField, RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed,
        RuntimePureHelperOrigin, RuntimePureHelperSeed, RuntimePureOutputType,
        RuntimePureProgramBindingSeed,
    },
    value::RuntimeRecordFieldId,
};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn program_id() -> RuntimePureProgramId {
    RuntimePureProgramId::from_checked_digest([9; 32])
}

fn codec() -> RuntimeCodecUse {
    RuntimeCodecUse::Record {
        name: "DefaultConfig".to_owned(),
        deny_unknown_fields: true,
        fields: vec![RuntimeFieldCodecUse {
            wire_name: "flag".to_owned(),
            has_default: true,
            default_program: Some(program_id()),
            skip: false,
            bytes_format: None,
            value: RuntimeCodecUse::Plain,
        }]
        .into(),
    }
}

fn programs() -> (RuntimePlan, AwbcProgram) {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    semantic(1),
                    Type::Record(Box::new([RuntimePlanRecordField::new("flag", semantic(2))])),
                )
                .with_data_codec(codec()),
                RuntimePlanTypeSeed::new(semantic(2), Type::Bool),
            ],
            [],
        )
        .unwrap();
    let helper = builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            name: "default_flag".to_owned(),
            inputs: Box::new([]),
            input_abi: vec![],
            output_abi: RuntimePureOutputType::Bool,
            body: RuntimeExprSeed::new(
                semantic(2),
                RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
            ),
            scalar_eval_supported: true,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .unwrap();
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program: program_id(),
            helper,
        })
        .unwrap();
    let awbc = AwbcProgram {
        strings: vec!["default_flag".to_owned(), "flag".to_owned()],
        runtime_types: vec![
            AwbcRuntimeType::new(
                semantic(1),
                AwbcRuntimeTypeShape::Record {
                    public_id: None,
                    fields: vec![AwbcRecordField {
                        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                        name: Some(AwbcStringId(1)),
                        ty: AwbcTypeId(1),
                    }],
                },
            )
            .with_data_codec(codec()),
            AwbcRuntimeType::new(semantic(2), AwbcRuntimeTypeShape::Bool),
        ],
        pure_programs: vec![AwbcPureProgramBinding {
            program: program_id(),
            helper: AwbcPureHelperId(0),
            input_types: vec![],
            result_type: semantic(2),
        }],
        pure_helpers: vec![AwbcPureHelper {
            public_id: AwbcStringId(0),
            signature: AwbcSignatureId(0),
            function: AwbcFunctionId(0),
            scalar_eval_supported: true,
            origin: AwbcPureHelperOrigin::EngineOwned,
        }],
        signatures: vec![AwbcSignature {
            params: vec![],
            result: Some(AwbcTypeId(1)),
            effects: AwbcEffectSetId(0),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            slots: vec![AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(1),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            }],
            max_scope_depth: 0,
        }],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::PureHelper,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(0)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        }],
        instructions: vec![AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        }],
        constants: vec![AwbcConstant::Bool(true)],
        ..AwbcProgram::default()
    };
    (builder.finish().unwrap(), awbc)
}

#[test]
fn field_default_requests_keep_the_exact_nullary_program_and_admit_its_result() {
    let (plan, awbc) = programs();
    let limits = RuntimeSchemaLimits::engine_default();
    for types in [
        RuntimeProgramTypes::Plan(&plan),
        RuntimeProgramTypes::Awbc(&awbc),
    ] {
        let shapes = RuntimeProgramDataShapes::new(types);
        let arcweft_data::ShapeRef::Id(root) = shapes.root(semantic(1), limits).unwrap() else {
            unreachable!()
        };
        let request = shapes.field_default_request(root, 0).unwrap().unwrap();
        assert_eq!(request.program(), program_id());
        assert_eq!(request.result_type(), semantic(2));
        request
            .validate_result(&RuntimeValue::Bool(true), limits)
            .unwrap();
        assert!(
            request
                .validate_result(&RuntimeValue::String("wrong return".to_owned()), limits)
                .is_err()
        );
    }
}

#[test]
fn field_default_admission_rejects_missing_and_tampered_program_proofs() {
    let (_, awbc) = programs();
    for defect in 0..6 {
        let mut candidate = awbc.clone();
        match defect {
            0 => candidate.pure_programs.clear(),
            1 => candidate
                .pure_programs
                .push(candidate.pure_programs[0].clone()),
            2 => candidate.pure_programs[0].input_types.push(semantic(2)),
            3 => candidate.pure_programs[0].result_type = semantic(1),
            4 => candidate.pure_helpers.clear(),
            5 => candidate.functions[0].kind = AwbcFunctionKind::Ordinary,
            _ => unreachable!(),
        }
        assert!(
            RuntimeProgramDataShapes::new(RuntimeProgramTypes::Awbc(&candidate))
                .validate_codec_uses(RuntimeSchemaLimits::engine_default())
                .is_err(),
            "defect {defect}"
        );
    }
    for (has_default, default_program) in [(true, None), (false, Some(program_id()))] {
        let mut candidate = awbc.clone();
        let mut policy = codec();
        let RuntimeCodecUse::Record { fields, .. } = &mut policy else {
            unreachable!()
        };
        fields[0].has_default = has_default;
        fields[0].default_program = default_program;
        candidate.runtime_types[0] =
            AwbcRuntimeType::new(semantic(1), candidate.runtime_types[0].shape().clone())
                .with_data_codec(policy);
        assert!(
            RuntimeProgramDataShapes::new(RuntimeProgramTypes::Awbc(&candidate))
                .validate_codec_uses(RuntimeSchemaLimits::engine_default())
                .is_err()
        );
    }
}
