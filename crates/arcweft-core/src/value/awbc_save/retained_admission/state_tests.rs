use super::*;

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
