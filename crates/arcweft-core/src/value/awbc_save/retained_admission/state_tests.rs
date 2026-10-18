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
