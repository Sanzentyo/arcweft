use super::*;

fn assert_metadata_bytes(value: &(impl serde::Serialize + ?Sized), bytes: u64) {
    let limits = RuntimeSchemaLimits {
        max_nodes: 0,
        max_encoded_bytes: bytes,
        ..RuntimeSchemaLimits::engine_default()
    };
    RuntimeRetainedValueAdmission::standalone(limits)
        .validate_metadata(value)
        .unwrap();
    if let Some(short) = bytes.checked_sub(1) {
        assert_eq!(
            RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
                max_encoded_bytes: short,
                ..limits
            })
            .validate_metadata(value),
            Err(AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded {
                    budget: "encoded_bytes"
                }
            ))
        );
    }
}

struct SequenceWithLengthHint<'a> {
    values: &'a [u8],
    hint: Option<usize>,
}

impl serde::Serialize for SequenceWithLengthHint<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut sequence = serializer.serialize_seq(self.hint)?;
        for value in self.values {
            sequence.serialize_element(value)?;
        }
        sequence.end()
    }
}

struct MetadataBlob<'a>(&'a [u8]);

impl serde::Serialize for MetadataBlob<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.0)
    }
}

#[test]
fn retained_metadata_counts_actual_members_and_length_prefixes() {
    let values = vec![7_u8; 150];
    assert_metadata_bytes(&values, 152);
    for hint in [Some(150), None, Some(0)] {
        assert_metadata_bytes(
            &SequenceWithLengthHint {
                values: &values,
                hint,
            },
            152,
        );
    }
    assert_metadata_bytes(&[(); 32], 1);
    assert_metadata_bytes(&MetadataBlob(&[1, 2, 3, 4]), 5);
    assert_metadata_bytes("é", 3);
}

#[test]
fn retained_metadata_scalars_and_variants_use_the_same_byte_bank() {
    #[derive(serde::Serialize)]
    enum MetadataVariant {
        Empty,
        Payload(u64),
        Pair(u16, u8),
        Fields { enabled: bool },
    }
    assert_metadata_bytes(&(), 0);
    assert_metadata_bytes(&u128::MAX, 16);
    assert_metadata_bytes(&'🦀', 4);
    assert_metadata_bytes(&None::<u128>, 1);
    assert_metadata_bytes(&Some(u128::MAX), 17);
    assert_metadata_bytes(&MetadataVariant::Empty, 4);
    assert_metadata_bytes(&MetadataVariant::Payload(10), 13);
    assert_metadata_bytes(&MetadataVariant::Pair(10, 1), 8);
    assert_metadata_bytes(&MetadataVariant::Fields { enabled: true }, 6);
}

#[test]
fn retained_metadata_shares_value_bytes_across_transfer_and_first_failure() {
    let limits = RuntimeSchemaLimits {
        max_nodes: 1,
        max_encoded_bytes: 23,
        ..RuntimeSchemaLimits::engine_default()
    };
    let mut admission = RuntimeRetainedValueAdmission::standalone(limits);
    admission.validate_metadata(&[u128::MAX]).unwrap(); // 16 + count prefix
    admission.validate_metadata_string("name").unwrap(); // 4 + length prefix
    admission.validate(&RuntimeValue::Unit).unwrap();
    let mut resumed = transfer(admission).resume();
    let error =
        AwbcRuntimeValueSnapshotError::Resource(crate::entry::RuntimeSchemaError::BudgetExceeded {
            budget: "encoded_bytes",
        });
    assert_eq!(resumed.validate_metadata_string(""), Err(error.clone()));
    assert_eq!(resumed.validate_metadata(&()), Err(error.clone()));
    assert_eq!(
        transfer(resumed).resume().validate_metadata_collection(0),
        Err(error)
    );
}

#[test]
fn retained_metadata_strings_accumulate_without_consuming_value_nodes() {
    let mut admission = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
        max_nodes: 0,
        max_string_bytes: 4,
        max_encoded_bytes: 10,
        ..RuntimeSchemaLimits::engine_default()
    });
    admission.validate_metadata_string("name").unwrap();
    admission.validate_metadata_string("same").unwrap();
    assert_eq!(
        admission.validate_metadata(&"more"),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            crate::entry::RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes"
            },
        ))
    );
}

fn transfer(admission: RuntimeRetainedValueAdmission) -> RuntimeRetainedValueAdmissionState {
    match admission.try_into_state() {
        Ok(state) => state,
        Err(_) => panic!("completed validation must release its temporary work handles"),
    }
}

#[test]
fn retained_bank_transfer_preserves_node_bytes_and_sticky_failure() {
    for limits in [
        RuntimeSchemaLimits {
            max_nodes: 1,
            ..RuntimeSchemaLimits::engine_default()
        },
        RuntimeSchemaLimits {
            max_encoded_bytes: 5,
            ..RuntimeSchemaLimits::engine_default()
        },
    ] {
        let value = RuntimeValue::String("x".to_owned());
        let mut admission = RuntimeRetainedValueAdmission::standalone(limits);
        admission.validate(&value).unwrap();
        let state = transfer(admission);
        fn require_send<T: Send>(_: &T) {}
        require_send(&state);
        let mut admission = state.resume();
        let error = admission
            .validate(&value)
            .expect_err("the original bank must not reset");
        let mut admission = transfer(admission).resume();
        assert_eq!(admission.validate(&RuntimeValue::Unit), Err(error));
    }
}

#[test]
fn retained_bank_cannot_transfer_while_a_validator_still_borrows_its_work() {
    let mut admission =
        RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits::engine_default());
    admission.validate(&RuntimeValue::Bool(true)).unwrap();
    let consumed = admission.resources.work().consumed();
    let temporary = admission.resources.work();
    let admission = match admission.try_into_state() {
        Err(admission) => admission,
        Ok(_) => panic!("a retained work handle must prevent exclusive bank transfer"),
    };
    assert_eq!(admission.resources.work().consumed(), consumed);
    drop(temporary);
    let admission = transfer(admission).resume();
    assert_eq!(admission.resources.work().consumed(), consumed);
}

#[test]
fn retained_bank_transfer_keeps_the_exact_original_program() {
    for owner in crate::value::awbc_save::tests::owners() {
        let admission = RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            RuntimeSchemaLimits::engine_default(),
        );
        let admission = transfer(admission).resume();
        assert!(admission.program_owner().unwrap().same_program(&owner));
    }
}

#[test]
fn retained_program_scopes_preserve_outer_owner_and_consumed_budget_after_unwind() {
    let owners = crate::value::awbc_save::tests::owners();
    for outer in &owners {
        for retained in &owners {
            let mut admission = RuntimeRetainedValueAdmission::for_program(
                outer.clone(),
                RuntimeSchemaLimits {
                    max_nodes: 1,
                    ..RuntimeSchemaLimits::engine_default()
                },
            );
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                admission.with_program_owner::<()>(retained.clone(), |admission| {
                    assert!(admission.program_owner().unwrap().same_program(retained));
                    admission.validate(&RuntimeValue::Bool(true)).unwrap();
                    panic!("interrupt retained-field admission");
                })
            }));
            assert!(result.is_err());
            assert!(admission.program_owner().unwrap().same_program(outer));
            let consumed = admission.work().consumed();
            assert!(consumed > 0);
            let error = admission
                .with_program_owner(retained.clone(), |admission| {
                    admission.validate(&RuntimeValue::Bool(false))
                })
                .unwrap_err();
            assert_eq!(
                error,
                AwbcRuntimeValueSnapshotError::Resource(
                    crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "nodes" }
                )
            );
            assert!(admission.program_owner().unwrap().same_program(outer));
            assert_eq!(admission.status(), Err(error.clone()));
            let mut resumed = transfer(admission).resume();
            assert!(resumed.program_owner().unwrap().same_program(outer));
            assert_eq!(resumed.validate(&RuntimeValue::Unit), Err(error));
            assert_eq!(resumed.work().consumed(), consumed);
        }
    }
}
