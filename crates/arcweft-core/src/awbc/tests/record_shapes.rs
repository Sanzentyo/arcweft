use super::*;
use crate::entry::{
    RuntimeNominalRecordShape as Shape, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
    RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
    RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema,
};
use crate::value::RuntimeRecordFieldId;

fn record_program(shape: Shape, count: usize) -> (AwbcProgram, RuntimeNominalSchemaGraph) {
    let identity = RuntimeSemanticTypeId::from_bytes([37; 32]);
    let nominal = RuntimeNominalTypeId::from_checked_digest(*identity.as_bytes());
    let names = ["zeta", "alpha", "beta"];
    let fields = (0..count)
        .map(|ordinal| {
            RuntimeNominalSchemaField::new(
                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                (shape == Shape::Record).then(|| names[ordinal].to_owned()),
                RuntimeTypeSchema::Bool,
            )
        })
        .collect::<Vec<_>>();
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalSchemaIdentity::new(nominal.clone(), identity),
            vec![],
            RuntimeNominalSchemaBody::Record {
                shape,
                fields: fields.into_boxed_slice(),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let mut program = minimal_program();
    program.strings.push(nominal.as_str().to_owned());
    program
        .strings
        .extend(names.iter().map(|name| (*name).to_owned()));
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Bool),
        AwbcRuntimeType::new(
            identity,
            AwbcRuntimeTypeShape::NominalRecord {
                public_id: AwbcStringId(1),
                layout: *graph.try_layout_hash(identity).unwrap().as_bytes(),
                arguments: vec![],
                shape,
                fields: (0..count)
                    .map(|ordinal| AwbcRecordField {
                        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                        name: (shape == Shape::Record)
                            .then(|| AwbcStringId(u32::try_from(ordinal + 2).unwrap())),
                        ty: AwbcTypeId(0),
                    })
                    .collect(),
            },
        ),
    ];
    program.constants = vec![
        AwbcConstant::Bool(true),
        AwbcConstant::Record {
            ty: AwbcTypeId(1),
            fields: vec![AwbcConstantId(0); count],
        },
    ];
    program.frame_layouts[0].slots = [AwbcTypeId(0), AwbcTypeId(1)]
        .into_iter()
        .map(|ty| AwbcFrameSlot {
            name: None,
            ty,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        })
        .collect();
    program.instructions = vec![
        AwbcInstruction::LoadConst {
            dst: AwbcRegisterId(0),
            constant: AwbcConstantId(0),
        },
        AwbcInstruction::MakeRecord {
            dst: AwbcRegisterId(1),
            ty: AwbcTypeId(1),
            fields: vec![AwbcRegisterId(0); count],
        },
    ];
    program.blocks[0].instructions = AwbcTableRange::new(0, 2);
    program.canonicalize_string_table();
    (program, graph)
}

#[test]
fn all_record_shapes_survive_wire_layout_constant_and_vm_construction() {
    for (shape, count) in [
        (Shape::Unit, 0),
        (Shape::Tuple, 0),
        (Shape::Tuple, 1),
        (Shape::Tuple, 3),
        (Shape::Record, 0),
        (Shape::Record, 1),
        (Shape::Record, 3),
        (Shape::Newtype, 1),
    ] {
        let (program, graph) = record_program(shape, count);
        let bytes = program.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
        decoded
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .unwrap();
        assert_eq!(decoded.encode_canonical().unwrap(), bytes);
        let layout = decoded
            .nominal_record_layout(AwbcTypeId(1))
            .unwrap()
            .unwrap();
        assert_eq!(layout.shape(), shape);
        assert_eq!(layout.fields().len(), count);
        for (ordinal, field) in layout.fields().iter().enumerate() {
            assert_eq!(field.field().zero_based() as usize, ordinal);
            assert_eq!(
                field.name(),
                (shape == Shape::Record).then(|| ["zeta", "alpha", "beta"][ordinal])
            );
        }
        let value = crate::awbc::vm::constant_value(&decoded, AwbcConstantId(1)).unwrap();
        let source_digest = graph
            .accepts_value(
                RuntimeSemanticTypeId::from_bytes([37; 32]),
                &value,
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap();
        assert_eq!(
            decoded
                .accepts_value(AwbcTypeId(1), &value, RuntimeSchemaLimits::engine_default())
                .unwrap(),
            source_digest,
        );
        let mut fiber = FiberState::for_entry(&decoded, AwbcEntryId(0), 1, 64).unwrap();
        let output = crate::awbc::vm::step(
            &decoded,
            &mut fiber,
            crate::awbc::vm::VmStepOptions {
                max_instructions: 2,
            },
        )
        .unwrap();
        assert_eq!(output.exit, crate::awbc::vm::VmExit::Running);
        assert_eq!(
            fiber
                .active_frame()
                .unwrap()
                .register(AwbcRegisterId(1))
                .unwrap(),
            &value
        );
    }
}

#[test]
fn record_rows_reject_invalid_coordinates_names_and_source_shapes() {
    let (valid, _) = record_program(Shape::Record, 3);
    for defect in 0..7 {
        let mut bad = valid.clone();
        let AwbcRuntimeTypeShape::NominalRecord {
            public_id,
            layout,
            arguments,
            mut fields,
            ..
        } = bad.runtime_types[1].shape().clone()
        else {
            unreachable!()
        };
        let mut shape = Shape::Record;
        match defect {
            0 => fields[1].field = fields[0].field,
            1 => fields[0].field = RuntimeRecordFieldId::try_from_zero_based_ordinal(3).unwrap(),
            2 => fields[0].name = None,
            3 => fields[1].name = fields[0].name,
            4 => shape = Shape::Unit,
            5 => shape = Shape::Tuple,
            6 => shape = Shape::Newtype,
            _ => unreachable!(),
        }
        bad.runtime_types[1] = AwbcRuntimeType::new(
            valid.runtime_types[1].semantic_identity(),
            AwbcRuntimeTypeShape::NominalRecord {
                public_id,
                layout,
                arguments,
                shape,
                fields,
            },
        );
        assert!(
            bad.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                .is_err(),
            "defect {defect}"
        );
        assert!(
            bad.nominal_record_layout(AwbcTypeId(1)).is_err(),
            "defect {defect}"
        );
    }
}

#[test]
fn source_layout_and_structural_cycles_are_checked_at_their_own_boundaries() {
    use crate::awbc::type_projection::AwbcTypeProjectionError as Error;
    let (valid, graph) = record_program(Shape::Record, 1);
    let RuntimeValue::NominalRecord(value) =
        crate::awbc::vm::constant_value(&valid, AwbcConstantId(1)).unwrap()
    else {
        panic!("nominal record")
    };
    let mut layout = *value.layout().as_bytes();
    layout[0] ^= 1;
    let wrong_layout = RuntimeValue::NominalRecord(crate::value::RuntimeNominalRecordValue::new(
        value.type_id().clone(),
        value.semantic_identity(),
        crate::entry::TypeLayoutHash::from_bytes(layout),
        value.fields().to_vec(),
    ));
    assert!(
        !valid
            .checked_type(AwbcTypeId(1))
            .unwrap()
            .accepts_value(&wrong_layout)
    );
    assert!(
        graph
            .accepts_value(
                RuntimeSemanticTypeId::from_bytes([37; 32]),
                &wrong_layout,
                RuntimeSchemaLimits::engine_default()
            )
            .is_err()
    );
    let AwbcRuntimeTypeShape::NominalRecord { public_id, .. } = valid.runtime_types[1].shape()
    else {
        unreachable!()
    };
    for shape in [
        AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(2)]),
        AwbcRuntimeTypeShape::Function {
            contract: crate::plan::RuntimeFunctionTypeContract::default(),
            parameters: vec![],
            result: AwbcTypeId(2),
        },
        AwbcRuntimeTypeShape::Nominal {
            public_id: *public_id,
            layout: *value.layout().as_bytes(),
            arguments: vec![AwbcTypeId(2)],
        },
    ] {
        let mut bad = valid.clone();
        bad.runtime_types.push(runtime_type(42, shape));
        assert!(matches!(
            bad.validate_type_graph(),
            Err(Error::CheckedTypeCycle { index: 2 })
        ));
        assert!(
            bad.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                .is_err()
        );
    }
    valid
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
}

#[test]
fn structural_graph_validation_is_iterative_and_does_not_expand_shared_children() {
    let mut program = AwbcProgram::default();
    program.runtime_types.clear();
    for index in 0_u32..10_000 {
        let mut identity = [0; 32];
        identity[..4].copy_from_slice(&index.to_le_bytes());
        let shape = if index == 0 {
            AwbcRuntimeTypeShape::Bool
        } else {
            AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(index - 1); 2])
        };
        program.runtime_types.push(AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes(identity),
            shape,
        ));
    }
    program.validate_type_graph().unwrap();
    program.runtime_types[0] = AwbcRuntimeType::new(
        program.runtime_types[0].semantic_identity(),
        AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(9_999)]),
    );
    assert!(matches!(
        program.validate_type_graph(),
        Err(crate::awbc::type_projection::AwbcTypeProjectionError::CheckedTypeCycle { .. })
    ));
}

#[test]
fn recursive_nominal_schema_verifies_without_unfolding_a_checked_type_tree() {
    let identity = RuntimeSemanticTypeId::from_bytes([77; 32]);
    let nominal = RuntimeNominalTypeId::from_checked_digest(*identity.as_bytes());
    let reference = RuntimeNominalSchemaIdentity::new(nominal.clone(), identity);
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            reference.clone(),
            vec![],
            RuntimeNominalSchemaBody::Record {
                shape: Shape::Record,
                fields: vec![RuntimeNominalSchemaField::new(
                    RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                    Some("next".to_owned()),
                    RuntimeTypeSchema::option(RuntimeTypeSchema::NominalRef(reference)),
                )]
                .into_boxed_slice(),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let mut program = minimal_program();
    program.strings.extend([
        nominal.as_str().to_owned(),
        "next".to_owned(),
        "Some".to_owned(),
        "None".to_owned(),
    ]);
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Bool),
        AwbcRuntimeType::new(
            identity,
            AwbcRuntimeTypeShape::NominalRecord {
                public_id: AwbcStringId(1),
                layout: *graph.try_layout_hash(identity).unwrap().as_bytes(),
                arguments: vec![],
                shape: Shape::Record,
                fields: vec![AwbcRecordField {
                    field: RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                    name: Some(AwbcStringId(2)),
                    ty: AwbcTypeId(2),
                }],
            },
        ),
        runtime_type(
            2,
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Builtin(
                    crate::pattern::RuntimeBuiltinVariantIdentity::Option,
                ),
                arguments: vec![],
                cases: vec![
                    AwbcVariantCase {
                        name: AwbcStringId(3),
                        payload: Some(AwbcTypeId(3)),
                    },
                    AwbcVariantCase {
                        name: AwbcStringId(4),
                        payload: None,
                    },
                ],
            },
        ),
        runtime_type(3, AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(1)])),
    ];
    program.constants = vec![
        AwbcConstant::Variant {
            ty: AwbcTypeId(2),
            case: 1,
            payload: None,
        },
        AwbcConstant::Record {
            ty: AwbcTypeId(1),
            fields: vec![AwbcConstantId(0)],
        },
    ];
    program.canonicalize_string_table();
    let decoded = AwbcProgram::decode_canonical(
        &program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let value = crate::awbc::vm::constant_value(&decoded, AwbcConstantId(1)).unwrap();
    let source_digest = graph
        .accepts_value(identity, &value, RuntimeSchemaLimits::engine_default())
        .unwrap();
    assert_eq!(
        decoded
            .accepts_value(AwbcTypeId(1), &value, RuntimeSchemaLimits::engine_default())
            .unwrap(),
        source_digest
    );

    let mut nested = value;
    for _ in 0..90 {
        nested = RuntimeValue::NominalRecord(crate::value::RuntimeNominalRecordValue::new(
            nominal.clone(),
            identity,
            graph.try_layout_hash(identity).unwrap(),
            vec![RuntimeValue::option_some(nested)],
        ));
    }
    let limits = RuntimeSchemaLimits {
        max_depth: 400,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert_eq!(
        decoded
            .accepts_value(AwbcTypeId(1), &nested, limits)
            .unwrap(),
        graph.accepts_value(identity, &nested, limits).unwrap()
    );
    assert!(
        decoded
            .accepts_value(
                AwbcTypeId(1),
                &nested,
                RuntimeSchemaLimits::engine_default()
            )
            .is_err()
    );

    let malformed = RuntimeValue::NominalRecord(crate::value::RuntimeNominalRecordValue::new(
        nominal,
        identity,
        graph.try_layout_hash(identity).unwrap(),
        vec![RuntimeValue::option_some(RuntimeValue::Bool(false))],
    ));
    assert!(
        decoded
            .accepts_value(AwbcTypeId(1), &malformed, limits)
            .is_err()
    );
}

#[test]
fn anonymous_record_and_variant_constants_read_names_only_from_the_type_table() {
    let variant_identity = RuntimeSemanticTypeId::from_bytes([3; 32]);
    let variant_graph = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalSchemaIdentity::new(
                RuntimeNominalTypeId::try_new("Cases").unwrap(),
                variant_identity,
            ),
            vec![],
            RuntimeNominalSchemaBody::Variant {
                cases: vec![
                    RuntimeNominalSchemaCase::new(
                        0,
                        "Full".to_owned(),
                        Some(RuntimeTypeSchema::Bool),
                    ),
                    RuntimeNominalSchemaCase::new(1, "Empty".to_owned(), None),
                ]
                .into_boxed_slice(),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let mut program = minimal_program();
    program.strings.extend([
        "zeta".to_owned(),
        "alpha".to_owned(),
        "Cases".to_owned(),
        "Full".to_owned(),
        "Empty".to_owned(),
    ]);
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Bool),
        runtime_type(
            2,
            AwbcRuntimeTypeShape::Record {
                public_id: None,
                fields: vec![
                    AwbcRecordField {
                        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                        name: Some(AwbcStringId(1)),
                        ty: AwbcTypeId(0),
                    },
                    AwbcRecordField {
                        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap(),
                        name: Some(AwbcStringId(2)),
                        ty: AwbcTypeId(0),
                    },
                ],
            },
        ),
        runtime_type(
            3,
            AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::Nominal {
                    public_id: AwbcStringId(3),
                    layout: *variant_graph
                        .try_layout_hash(variant_identity)
                        .unwrap()
                        .as_bytes(),
                },
                arguments: vec![],
                cases: vec![
                    AwbcVariantCase {
                        name: AwbcStringId(4),
                        payload: Some(AwbcTypeId(0)),
                    },
                    AwbcVariantCase {
                        name: AwbcStringId(5),
                        payload: None,
                    },
                ],
            },
        ),
    ];
    program.constants = vec![
        AwbcConstant::Bool(true),
        AwbcConstant::Record {
            ty: AwbcTypeId(1),
            fields: vec![AwbcConstantId(0); 2],
        },
        AwbcConstant::Variant {
            ty: AwbcTypeId(2),
            case: 0,
            payload: Some(AwbcConstantId(0)),
        },
    ];
    program.canonicalize_string_table();
    let decoded = AwbcProgram::decode_canonical(
        &program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let RuntimeValue::Record(record) =
        crate::awbc::vm::constant_value(&decoded, AwbcConstantId(1)).unwrap()
    else {
        panic!("record")
    };
    assert_eq!(
        record
            .iter()
            .map(crate::value::RuntimeFieldValue::name)
            .collect::<Vec<_>>(),
        ["zeta", "alpha"]
    );
    let RuntimeValue::Variant {
        name,
        ordinal,
        payload,
        ..
    } = crate::awbc::vm::constant_value(&decoded, AwbcConstantId(2)).unwrap()
    else {
        panic!("variant")
    };
    assert_eq!(
        (name.as_str(), ordinal, payload.as_deref()),
        ("Full", 0, Some(&RuntimeValue::Bool(true)))
    );
    let mut wrong_owner = decoded.clone();
    wrong_owner.runtime_types[1] = runtime_type(2, AwbcRuntimeTypeShape::Dynamic);
    assert!(
        wrong_owner
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .is_err()
    );
    assert!(crate::awbc::vm::constant_value(&wrong_owner, AwbcConstantId(1)).is_err());
}
