use super::*;
use crate::awbc::schema::AwbcVariantCase;
use crate::awbc::schema::{AwbcSignedIntKind, AwbcUnsignedIntKind};
use crate::entry::{RuntimeNominalTypeId, TypeLayoutHash};
use crate::pattern::RuntimeSemanticTypeId;
use crate::value::{
    RuntimeInt, RuntimeNominalRecordValue, RuntimeRecordFieldId, RuntimeRecordValue, RuntimeSeq,
    RuntimeUInt,
};

fn program(shapes: impl IntoIterator<Item = Type>) -> AwbcProgram {
    AwbcProgram {
        runtime_types: shapes
            .into_iter()
            .enumerate()
            .map(|(index, shape)| {
                AwbcRuntimeType::new(
                    RuntimeSemanticTypeId::from_bytes([u8::try_from(index).unwrap(); 32]),
                    shape,
                )
            })
            .collect(),
        ..AwbcProgram::default()
    }
}

fn admits(
    program: &AwbcProgram,
    ty: u32,
    value: &RuntimeValue,
) -> Result<RuntimeValueDigest, AwbcValueAdmissionError> {
    program.accepts_value(AwbcTypeId(ty), value, RuntimeSchemaLimits::engine_default())
}

#[test]
fn record_names_ids_order_and_nested_types_are_checked() {
    let mut program = program([
        Type::Bool,
        Type::Record {
            public_id: None,
            fields: vec![AwbcRecordField {
                field: RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                name: Some(AwbcStringId(0)),
                ty: AwbcTypeId(0),
            }],
        },
    ]);
    program.strings = vec!["flag".to_owned(), "different".to_owned()];
    let value = |name: &str, value| {
        RuntimeValue::Record(RuntimeRecordValue::try_new(vec![(name.to_owned(), value)]).unwrap())
    };
    let good = value("flag", RuntimeValue::Bool(true));
    assert_eq!(
        admits(&program, 1, &good).unwrap(),
        good.try_digest(10_000).unwrap()
    );
    assert!(matches!(
        admits(&program, 1, &value("different", RuntimeValue::Bool(true))),
        Err(AwbcValueAdmissionError::Value {
            source: RuntimeSchemaError::RecordField { ordinal: 0, .. },
            ..
        })
    ));
    assert!(
        matches!(admits(&program, 1, &value("flag", RuntimeValue::Unit)), Err(AwbcValueAdmissionError::Value { source: RuntimeSchemaError::Type { path, .. }, .. }) if path == "$.flag")
    );
    let Type::Record { mut fields, .. } = program.runtime_types[1].shape().clone() else {
        unreachable!()
    };
    fields[0].field = RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap();
    program.runtime_types[1] = AwbcRuntimeType::new(
        program.runtime_types[1].semantic_identity(),
        Type::Record {
            public_id: None,
            fields,
        },
    );
    assert!(admits(&program, 1, &good).is_err());
}

#[test]
fn opaque_nominal_headers_cannot_substitute_for_record_domains() {
    let mut program = program([Type::Nominal {
        public_id: AwbcStringId(0),
        layout: [5; 32],
        arguments: vec![],
    }]);
    program.strings.push("fixture.HeaderOnly".to_owned());
    let value = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
        RuntimeNominalTypeId::try_new("fixture.HeaderOnly").unwrap(),
        program.runtime_types[0].semantic_identity(),
        TypeLayoutHash::from_bytes([5; 32]),
        vec![RuntimeValue::Bool(true)],
    ));
    assert!(admits(&program, 0, &value).is_err());
    assert!(matches!(
        admits(&program, 99, &value),
        Err(AwbcValueAdmissionError::UnknownType { ty: AwbcTypeId(99) })
    ));
}

#[test]
fn borrowed_dense_bytes_and_maps_preserve_value_predicates() {
    use crate::entry::RuntimeTypeSchema;
    use crate::plan::{RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed};
    use crate::value::RuntimeSignedIntWidth;
    let program = program([
        Type::Bytes,
        Type::UInt(AwbcUnsignedIntKind::U8),
        Type::Sequence(AwbcTypeId(1)),
        Type::String,
        Type::Int(AwbcSignedIntKind::I16),
        Type::Map {
            kind: crate::entry::RuntimeMapKind::Ordered,
            key: AwbcTypeId(3),
            value: AwbcTypeId(4),
        },
    ]);
    let bytes = RuntimeValue::Seq(RuntimeSeq::values(vec![
        RuntimeValue::UInt(RuntimeUInt::U8(3)),
        RuntimeValue::UInt(RuntimeUInt::U8(4)),
    ]));
    assert_eq!(
        admits(&program, 0, &bytes).unwrap(),
        admits(&program, 2, &bytes).unwrap()
    );
    let dense = RuntimeValue::Seq(RuntimeSeq::dense_u8(vec![3, 4]));
    assert_eq!(
        admits(&program, 0, &bytes).unwrap(),
        admits(&program, 0, &dense).unwrap()
    );
    assert!(
        admits(
            &program,
            0,
            &RuntimeValue::Seq(RuntimeSeq::dense_u16(vec![3, 4]))
        )
        .is_err()
    );
    let map = |key, value| {
        RuntimeValue::Seq(RuntimeSeq::values(vec![RuntimeValue::Tuple(vec![
            key, value,
        ])]))
    };
    let source_schema = RuntimeTypeSchema::Map {
        kind: crate::entry::schema::RuntimeMapKind::Ordered,
        key: Box::new(RuntimeTypeSchema::String),
        value: Box::new(RuntimeTypeSchema::I16),
    };
    let mut builder = RuntimePlanBuilder::new();
    let identities = [3, 4, 5].map(|index| program.runtime_types[index].semantic_identity());
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(identities[0], RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    identities[1],
                    RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I16),
                ),
                RuntimePlanTypeSeed::new(
                    identities[2],
                    RuntimePlanTypeProjection::Map {
                        kind: crate::entry::RuntimeMapKind::Ordered,
                        key: identities[0],
                        value: identities[1],
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let plan_type = plan.type_table().id_for_semantic(identities[2]).unwrap();
    let good = map(
        RuntimeValue::String("key".to_owned()),
        RuntimeValue::Int(RuntimeInt::I16(12)),
    );
    let limits = RuntimeSchemaLimits::engine_default();
    let source_digest = source_schema.validate_value(&good, limits).unwrap();
    assert_eq!(
        plan.accepts_value(plan_type, &good, limits).unwrap(),
        source_digest
    );
    assert_eq!(admits(&program, 5, &good).unwrap(), source_digest);
    assert!(
        admits(
            &program,
            5,
            &map(
                RuntimeValue::String("key".to_owned()),
                RuntimeValue::Int(RuntimeInt::I16(12))
            )
        )
        .is_ok()
    );
    assert!(
        admits(
            &program,
            5,
            &map(
                RuntimeValue::Bool(true),
                RuntimeValue::Int(RuntimeInt::I16(12))
            )
        )
        .is_err()
    );
    assert!(
        admits(
            &program,
            5,
            &map(
                RuntimeValue::String("key".to_owned()),
                RuntimeValue::Int(RuntimeInt::I64(12))
            )
        )
        .is_err()
    );
    assert!(
        admits(
            &program,
            5,
            &RuntimeValue::Seq(RuntimeSeq::values(vec![RuntimeValue::Tuple(vec![])]))
        )
        .is_err()
    );
}

#[test]
fn choices_share_work_and_require_exactly_one_complete_match() {
    let program = program([
        Type::Bool,
        Type::String,
        Type::Dynamic,
        Type::Choice(vec![AwbcTypeId(0), AwbcTypeId(1)]),
        Type::Choice(vec![AwbcTypeId(0), AwbcTypeId(2)]),
        Type::Choice(vec![AwbcTypeId(5)]),
    ]);
    assert!(admits(&program, 3, &RuntimeValue::Bool(true)).is_ok());
    assert!(matches!(
        admits(&program, 4, &RuntimeValue::Bool(true)),
        Err(AwbcValueAdmissionError::Value {
            source: RuntimeSchemaError::ChoiceAmbiguous { .. },
            ..
        })
    ));
    assert!(matches!(
        admits(&program, 3, &RuntimeValue::Unit),
        Err(AwbcValueAdmissionError::Value {
            source: RuntimeSchemaError::ChoiceNoMatch { .. },
            ..
        })
    ));
    let limits = RuntimeSchemaLimits {
        max_validation_work: 3,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert!(matches!(
        program.accepts_value(AwbcTypeId(5), &RuntimeValue::Bool(true), limits),
        Err(AwbcValueAdmissionError::Value {
            source: RuntimeSchemaError::ValidationWork { consumed: 4, .. },
            ..
        })
    ));
}

#[test]
fn bounds_and_dangling_references_fail_without_a_digest() {
    let program = program([
        Type::String,
        Type::Tuple(vec![AwbcTypeId(99)]),
        Type::Sequence(AwbcTypeId(0)),
    ]);
    assert!(admits(&program, 1, &RuntimeValue::Tuple(vec![RuntimeValue::Unit])).is_err());
    let value = RuntimeValue::Seq(RuntimeSeq::values(vec![RuntimeValue::String(
        "four".to_owned(),
    )]));
    for limits in [
        RuntimeSchemaLimits {
            max_nodes: 1,
            ..RuntimeSchemaLimits::engine_default()
        },
        RuntimeSchemaLimits {
            max_string_bytes: 3,
            ..RuntimeSchemaLimits::engine_default()
        },
        RuntimeSchemaLimits {
            max_encoded_bytes: 1,
            ..RuntimeSchemaLimits::engine_default()
        },
    ] {
        assert!(
            program
                .accepts_value(AwbcTypeId(2), &value, limits)
                .is_err()
        );
    }
}

#[test]
fn nominal_variant_correlates_every_owner_atom_and_the_complete_case_row() {
    let owner = RuntimeVariantIdentity::Nominal {
        nominal: RuntimeNominalTypeId::try_new("fixture.Cases").unwrap(),
        semantic_identity: RuntimeSemanticTypeId::from_bytes([1; 32]),
        layout: TypeLayoutHash::from_bytes([7; 32]),
    };
    let mut program = program([
        Type::Bool,
        Type::Variant {
            owner: AwbcVariantIdentity::Nominal {
                public_id: AwbcStringId(0),
                layout: [7; 32],
            },
            arguments: vec![],
            cases: vec![
                AwbcVariantCase {
                    name: AwbcStringId(1),
                    payload: Some(AwbcTypeId(0)),
                },
                AwbcVariantCase {
                    name: AwbcStringId(2),
                    payload: None,
                },
            ],
        },
    ]);
    program.strings = vec![
        "fixture.Cases".to_owned(),
        "Full".to_owned(),
        "Empty".to_owned(),
        String::new(),
    ];
    let good = RuntimeValue::Variant {
        owner: owner.clone(),
        ordinal: 0,
        name: "Full".to_owned(),
        payload: Some(Box::new(RuntimeValue::Bool(true))),
    };
    assert!(admits(&program, 1, &good).is_ok());
    for changed in 0..6 {
        let mut value = good.clone();
        let RuntimeValue::Variant {
            owner:
                RuntimeVariantIdentity::Nominal {
                    nominal,
                    semantic_identity,
                    layout,
                },
            ordinal,
            name,
            payload,
        } = &mut value
        else {
            unreachable!()
        };
        match changed {
            0 => *nominal = RuntimeNominalTypeId::try_new("fixture.Other").unwrap(),
            1 => *semantic_identity = RuntimeSemanticTypeId::from_bytes([9; 32]),
            2 => *layout = TypeLayoutHash::from_bytes([9; 32]),
            3 => *ordinal = 1,
            4 => *name = "Empty".to_owned(),
            5 => *payload = Some(Box::new(RuntimeValue::Unit)),
            _ => unreachable!(),
        }
        assert!(admits(&program, 1, &value).is_err(), "{changed}");
    }
    for name in [AwbcStringId(1), AwbcStringId(3), AwbcStringId(99)] {
        let mut candidate = program.clone();
        let Type::Variant {
            owner,
            arguments,
            mut cases,
        } = candidate.runtime_types[1].shape().clone()
        else {
            unreachable!()
        };
        cases[1].name = name;
        candidate.runtime_types[1] = AwbcRuntimeType::new(
            candidate.runtime_types[1].semantic_identity(),
            Type::Variant {
                owner,
                arguments,
                cases,
            },
        );
        assert!(admits(&candidate, 1, &good).is_err());
        assert!(candidate.checked_type(AwbcTypeId(1)).is_err());
    }
}
