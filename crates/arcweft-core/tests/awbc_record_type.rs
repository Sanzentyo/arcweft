use arcweft_core::awbc::{
    codec::AwbcDecodeBudget,
    schema::{
        AwbcProgram, AwbcRecordField, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcStringId,
        AwbcTypeId,
    },
};
use arcweft_core::pattern::{
    RuntimeCheckedRecordTypeError, RuntimeCheckedType, RuntimeSemanticTypeId,
};
use arcweft_core::value::{RuntimeRecordFieldId, RuntimeSignedIntWidth};

fn identity(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn structural_record_program(first_name: &str, second_name: &str) -> AwbcProgram {
    let mut program = AwbcProgram::default();
    program.strings = vec![
        first_name.to_owned(),
        second_name.to_owned(),
        "nested-value".to_owned(),
    ];
    program.runtime_types = vec![
        AwbcRuntimeType::new(identity(1), AwbcRuntimeTypeShape::Bool),
        AwbcRuntimeType::new(
            identity(2),
            AwbcRuntimeTypeShape::Record {
                public_id: None,
                fields: vec![AwbcRecordField {
                    name: AwbcStringId(2),
                    ty: AwbcTypeId(0),
                }],
            },
        ),
        AwbcRuntimeType::new(
            identity(3),
            AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0), AwbcTypeId(1)]),
        ),
        AwbcRuntimeType::new(
            identity(4),
            AwbcRuntimeTypeShape::Record {
                public_id: None,
                fields: vec![
                    AwbcRecordField {
                        name: AwbcStringId(0),
                        ty: AwbcTypeId(0),
                    },
                    AwbcRecordField {
                        name: AwbcStringId(1),
                        ty: AwbcTypeId(2),
                    },
                ],
            },
        ),
    ];
    program
}

fn checked_record_after_roundtrip(first_name: &str, second_name: &str) -> RuntimeCheckedType {
    let bytes = structural_record_program(first_name, second_name)
        .encode_canonical()
        .expect("structural record AWBC encodes");
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default())
        .expect("structural record AWBC decodes");
    decoded
        .checked_type(AwbcTypeId(3))
        .expect("structural record projects to checked type")
}

#[test]
fn awbc_record_roundtrip_preserves_ordered_ids_and_recursive_types() {
    let checked = checked_record_after_roundtrip("first", "pair");
    let expected = RuntimeCheckedType::try_record([
        (
            RuntimeRecordFieldId::try_from_zero_based_ordinal(0).expect("first field"),
            "first".to_owned(),
            RuntimeCheckedType::Bool,
        ),
        (
            RuntimeRecordFieldId::try_from_zero_based_ordinal(1).expect("second field"),
            "pair".to_owned(),
            RuntimeCheckedType::Tuple(vec![
                RuntimeCheckedType::Bool,
                RuntimeCheckedType::try_record([(
                    RuntimeRecordFieldId::try_from_zero_based_ordinal(0).expect("nested field"),
                    "nested-value".to_owned(),
                    RuntimeCheckedType::Bool,
                )])
                .expect("nested checked record"),
            ]),
        ),
    ])
    .expect("expected checked record");

    assert_eq!(checked, expected);
    let RuntimeCheckedType::Record(fields) = checked else {
        panic!("AWBC structural record projects to a record");
    };
    assert_eq!(fields[0].field().zero_based(), 0);
    assert_eq!(fields[1].field().zero_based(), 1);
    assert_eq!(fields[0].diagnostic_name(), "first");
    assert_eq!(fields[1].diagnostic_name(), "pair");
    assert_eq!(
        fields[1].ty(),
        &RuntimeCheckedType::Tuple(vec![
            RuntimeCheckedType::Bool,
            RuntimeCheckedType::try_record([(
                RuntimeRecordFieldId::try_from_zero_based_ordinal(0).expect("nested field"),
                "nested-value".to_owned(),
                RuntimeCheckedType::Bool,
            )])
            .expect("nested checked record"),
        ])
    );
}

#[test]
fn awbc_record_diagnostic_renames_do_not_change_semantic_identity_or_equality() {
    let original = checked_record_after_roundtrip("first", "pair");
    let renamed = checked_record_after_roundtrip("renamed-first", "renamed-pair");

    assert_eq!(original, renamed);
    assert_eq!(
        original.semantic_identity_digest(),
        renamed.semantic_identity_digest()
    );

    let RuntimeCheckedType::Record(original_fields) = original else {
        panic!("original AWBC record projection");
    };
    let RuntimeCheckedType::Record(renamed_fields) = renamed else {
        panic!("renamed AWBC record projection");
    };
    assert_eq!(original_fields[0].diagnostic_name(), "first");
    assert_eq!(original_fields[1].diagnostic_name(), "pair");
    assert_eq!(renamed_fields[0].diagnostic_name(), "renamed-first");
    assert_eq!(renamed_fields[1].diagnostic_name(), "renamed-pair");
}

#[test]
fn checked_record_constructor_rejects_duplicate_and_out_of_order_ids() {
    let first = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).expect("first field");
    let second = RuntimeRecordFieldId::try_from_zero_based_ordinal(1).expect("second field");

    assert_eq!(
        RuntimeCheckedType::try_record([
            (first, "first".to_owned(), RuntimeCheckedType::Bool),
            (first, "duplicate".to_owned(), RuntimeCheckedType::Bool),
        ]),
        Err(RuntimeCheckedRecordTypeError::InvalidFieldCoordinate {
            expected: second,
            actual: first,
        })
    );
    assert_eq!(
        RuntimeCheckedType::try_record([(
            second,
            "out-of-order".to_owned(),
            RuntimeCheckedType::Signed(RuntimeSignedIntWidth::I64),
        )]),
        Err(RuntimeCheckedRecordTypeError::InvalidFieldCoordinate {
            expected: first,
            actual: second,
        })
    );
}
