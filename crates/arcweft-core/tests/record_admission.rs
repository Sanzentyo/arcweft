use arcweft_core::value::{
    AwbcRuntimeValueSnapshot, RuntimeRecordAdmissionError, RuntimeSeq, RuntimeSeqError,
    RuntimeValue,
};

#[test]
fn awbc_snapshot_reconstruction_admits_the_same_row_and_column_inventory() {
    let row = RuntimeValue::try_record(vec![
        ("z".to_owned(), RuntimeValue::Unit),
        ("a".to_owned(), RuntimeValue::Unit),
    ])
    .unwrap();
    let columns = RuntimeValue::Seq(
        RuntimeSeq::record_columns(
            1,
            vec![
                ("z".to_owned(), RuntimeSeq::values(vec![RuntimeValue::Unit])),
                ("a".to_owned(), RuntimeSeq::values(vec![RuntimeValue::Unit])),
            ],
        )
        .unwrap(),
    );
    for (value, fields_path) in [(row, "/Record"), (columns, "/Seq/RecordColumns/fields")] {
        let snapshot = AwbcRuntimeValueSnapshot::from_runtime_value(&value).unwrap();
        let encoded = serde_json::to_value(&snapshot).unwrap();
        let decoded: AwbcRuntimeValueSnapshot = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(decoded.into_runtime_value().unwrap(), value);
        for (ordinal, field, replacement) in [
            (0, "field", serde_json::json!(2)),
            (1, "field", serde_json::json!(1)),
            (0, "name", serde_json::json!("")),
            (1, "name", serde_json::json!("z")),
        ] {
            let mut malformed = encoded.clone();
            malformed.pointer_mut(fields_path).unwrap()[ordinal][field] = replacement;
            let snapshot: AwbcRuntimeValueSnapshot = serde_json::from_value(malformed).unwrap();
            assert!(snapshot.into_runtime_value().is_err());
        }
    }
}

#[test]
fn anonymous_record_admission_assigns_authored_one_based_identities() {
    let value = RuntimeValue::try_record(vec![
        ("z".to_owned(), RuntimeValue::i32(1)),
        ("a".to_owned(), RuntimeValue::i32(2)),
    ])
    .unwrap();
    let RuntimeValue::Record(fields) = value else {
        panic!("record admission returns a record");
    };
    assert_eq!(fields.fields()[0].field().get().get(), 1);
    assert_eq!(fields.fields()[0].name(), "z");
    assert_eq!(fields.fields()[1].field().get().get(), 2);
    assert_eq!(fields.fields()[1].name(), "a");
}

#[test]
fn anonymous_record_admission_rejects_first_repeated_name() {
    let error = RuntimeValue::try_record(vec![
        ("a".to_owned(), RuntimeValue::Unit),
        ("b".to_owned(), RuntimeValue::Unit),
        ("a".to_owned(), RuntimeValue::Unit),
    ])
    .unwrap_err();
    assert_eq!(
        error,
        RuntimeRecordAdmissionError::DuplicateName {
            name: "a".to_owned(),
        }
    );
    assert!(matches!(
        RuntimeValue::try_record(Vec::new()).unwrap(),
        RuntimeValue::Record(fields) if fields.is_empty()
    ));
}

#[test]
fn record_columns_preserve_ids_and_reconstruct_rows() {
    let sequence = RuntimeSeq::record_columns(
        2,
        vec![
            (
                "z".to_owned(),
                RuntimeSeq::values(vec![RuntimeValue::i32(1), RuntimeValue::i32(2)]),
            ),
            (
                "a".to_owned(),
                RuntimeSeq::values(vec![RuntimeValue::i32(3), RuntimeValue::i32(4)]),
            ),
        ],
    )
    .unwrap();
    let RuntimeSeq::RecordColumns(columns) = &sequence else {
        panic!("record column admission returns record columns");
    };
    assert_eq!(columns.fields()[0].field().get().get(), 1);
    assert_eq!(columns.fields()[0].name(), "z");
    assert_eq!(columns.fields()[1].field().get().get(), 2);
    assert_eq!(columns.fields()[1].name(), "a");

    let RuntimeValue::Record(row) = sequence.value_at(1) else {
        panic!("record columns reconstruct a record row");
    };
    assert_eq!(row.fields()[0].field(), columns.fields()[0].field());
    assert_eq!(row.fields()[0].value(), &RuntimeValue::i32(2));
    assert_eq!(row.fields()[1].field(), columns.fields()[1].field());
    assert_eq!(row.fields()[1].value(), &RuntimeValue::i32(4));
}

#[test]
fn record_column_length_precedes_duplicate_name() {
    let error = RuntimeSeq::record_columns(
        2,
        vec![
            (
                "a".to_owned(),
                RuntimeSeq::values(vec![RuntimeValue::Unit; 2]),
            ),
            ("a".to_owned(), RuntimeSeq::values(vec![RuntimeValue::Unit])),
        ],
    )
    .unwrap_err();
    assert_eq!(
        error,
        RuntimeSeqError::ColumnLength {
            ordinal: 1,
            expected: 2,
            actual: 1,
        }
    );

    let duplicate = RuntimeSeq::record_columns(
        2,
        vec![
            (
                "a".to_owned(),
                RuntimeSeq::values(vec![RuntimeValue::Unit; 2]),
            ),
            (
                "a".to_owned(),
                RuntimeSeq::values(vec![RuntimeValue::Unit; 2]),
            ),
        ],
    )
    .unwrap_err();
    assert_eq!(
        duplicate,
        RuntimeSeqError::RecordFields(RuntimeRecordAdmissionError::DuplicateName {
            name: "a".to_owned(),
        })
    );
}

#[test]
fn first_record_column_length_mismatch_reports_ordinal_zero() {
    assert_eq!(
        RuntimeSeq::record_columns(
            2,
            vec![("a".to_owned(), RuntimeSeq::values(vec![RuntimeValue::Unit]))],
        )
        .unwrap_err(),
        RuntimeSeqError::ColumnLength {
            ordinal: 0,
            expected: 2,
            actual: 1,
        }
    );
}
