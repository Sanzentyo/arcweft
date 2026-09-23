use super::*;
use crate::awbc::{codec::AwbcDecodeBudget, schema::AwbcInstruction};

#[test]
fn removed_constant_and_instruction_names_cannot_return_through_serde() {
    for (constant, variant, removed_field) in [
        (
            AwbcConstant::Record {
                ty: AwbcTypeId(3),
                fields: vec![],
            },
            "Record",
            "field_names",
        ),
        (
            AwbcConstant::Variant {
                ty: AwbcTypeId(4),
                case: 0,
                payload: None,
            },
            "Variant",
            "case_name",
        ),
    ] {
        let mut value = serde_json::to_value(&constant).unwrap();
        assert_eq!(
            serde_json::from_value::<AwbcConstant>(value.clone()).unwrap(),
            constant
        );
        value[variant]
            .as_object_mut()
            .unwrap()
            .insert(removed_field.to_owned(), serde_json::json!([]));
        assert!(serde_json::from_value::<AwbcConstant>(value).is_err());
    }
    let instruction = AwbcInstruction::MakeRecord {
        dst: AwbcRegisterId(1),
        ty: AwbcTypeId(2),
        fields: vec![],
    };
    let mut value = serde_json::to_value(&instruction).unwrap();
    assert_eq!(
        serde_json::from_value::<AwbcInstruction>(value.clone()).unwrap(),
        instruction
    );
    value["MakeRecord"]
        .as_object_mut()
        .unwrap()
        .insert("field_names".to_owned(), serde_json::json!([]));
    assert!(serde_json::from_value::<AwbcInstruction>(value).is_err());
}

#[test]
fn nominal_record_wire_retains_shared_shape_explicit_id_and_optional_name() {
    for (shape, fields, suffix) in [
        (RuntimeNominalRecordShape::Unit, vec![], vec![0, 0]),
        (RuntimeNominalRecordShape::Tuple, vec![], vec![1, 0]),
        (RuntimeNominalRecordShape::Record, vec![], vec![2, 0]),
        (
            RuntimeNominalRecordShape::Tuple,
            vec![(None, AwbcTypeId(2))],
            vec![1, 1, 1, 0, 2],
        ),
        (
            RuntimeNominalRecordShape::Record,
            vec![(Some(AwbcStringId(3)), AwbcTypeId(2))],
            vec![2, 1, 1, 1, 3, 2],
        ),
        (
            RuntimeNominalRecordShape::Newtype,
            vec![(None, AwbcTypeId(2))],
            vec![3, 1, 1, 0, 2],
        ),
    ] {
        let row = AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([0x11; 32]),
            AwbcRuntimeTypeShape::NominalRecord {
                public_id: AwbcStringId(0),
                layout: [0x22; 32],
                arguments: vec![],
                shape,
                fields: fields
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, (name, ty))| AwbcRecordField {
                        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                        name,
                        ty,
                    })
                    .collect(),
            },
        );
        let mut writer = Writer::default();
        row.write_wire(&mut writer).unwrap();
        let bytes = writer.into_bytes();
        let mut expected = vec![0x11; 32];
        expected.extend([24, 0]);
        expected.extend([0x22; 32]);
        expected.push(0); // Existing ordered generic-argument vector.
        expected.extend(suffix);
        expected.extend([0, 0]); // No source codec use or generic argument policy.
        assert_eq!(bytes, expected);
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(AwbcRuntimeType::read_wire(&mut reader).unwrap(), row);
        reader.finish().unwrap();
        for length in 0..bytes.len() {
            assert!(
                AwbcRuntimeType::read_wire(&mut Reader::new(
                    &bytes[..length],
                    &AwbcDecodeBudget::default()
                ))
                .is_err()
            );
        }
    }
}

#[test]
fn record_field_wire_rejects_zero_and_noncanonical_ids_and_unknown_shape() {
    let field = AwbcRecordField {
        field: RuntimeRecordFieldId::try_from_zero_based_ordinal(299).unwrap(),
        name: Some(AwbcStringId(300)),
        ty: AwbcTypeId(300),
    };
    let mut writer = Writer::default();
    field.write_wire(&mut writer).unwrap();
    assert_eq!(writer.into_bytes(), [0xac, 0x02, 1, 0xac, 0x02, 0xac, 0x02]);
    for bytes in [
        &[0, 0, 0][..],
        &[0x80, 0, 0, 0][..],
        &[1, 2, 0][..],
        &[1, 0, 0x80, 0][..],
    ] {
        assert!(
            AwbcRecordField::read_wire(&mut Reader::new(bytes, &AwbcDecodeBudget::default()))
                .is_err()
        );
    }
    assert!(matches!(
        RuntimeNominalRecordShape::read_wire(&mut Reader::new(&[4], &AwbcDecodeBudget::default())),
        Err(AwbcCodecError::UnknownTag {
            kind: "nominal record shape",
            tag: 4,
            offset: 0
        })
    ));
}

#[test]
fn constant_and_instruction_rows_reference_the_type_owned_field_and_case_inventory() {
    for (constant, expected) in [
        (
            AwbcConstant::Record {
                ty: AwbcTypeId(3),
                fields: vec![AwbcConstantId(2)],
            },
            vec![12, 3, 1, 2],
        ),
        (
            AwbcConstant::Variant {
                ty: AwbcTypeId(4),
                case: 0,
                payload: None,
            },
            vec![13, 4, 0, 0],
        ),
        (
            AwbcConstant::Variant {
                ty: AwbcTypeId(4),
                case: 0,
                payload: Some(AwbcConstantId(5)),
            },
            vec![13, 4, 0, 1, 5],
        ),
    ] {
        let mut writer = Writer::default();
        constant.write_wire(&mut writer).unwrap();
        assert_eq!(writer.into_bytes(), expected);
        let mut reader = Reader::new(&expected, &AwbcDecodeBudget::default());
        assert_eq!(AwbcConstant::read_wire(&mut reader).unwrap(), constant);
        reader.finish().unwrap();
    }
    let instruction = AwbcInstruction::MakeRecord {
        dst: AwbcRegisterId(1),
        ty: AwbcTypeId(2),
        fields: vec![AwbcRegisterId(3)],
    };
    let mut writer = Writer::default();
    instruction.write_wire(&mut writer).unwrap();
    let expected = [5, 1, 2, 1, 3];
    assert_eq!(writer.into_bytes(), expected);
    let mut reader = Reader::new(&expected, &AwbcDecodeBudget::default());
    assert_eq!(
        AwbcInstruction::read_wire(&mut reader).unwrap(),
        instruction
    );
    reader.finish().unwrap();
}
