use super::*;
use crate::awbc::schema::{AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcTypeId};
use crate::entry::RuntimeSchemaLimits;
use crate::value::RuntimeValue;
use std::num::NonZeroU32;
use std::sync::Arc;

fn activation(occurrence: u64) -> DialogueActivationId {
    DialogueActivationId::new(
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0xc7; 32]).unwrap(),
        crate::runtime_id::RuntimePersistentFiberId::from_allocated(1),
        crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
        occurrence,
    )
}

fn admission() -> RuntimeRetainedValueAdmission {
    RuntimeRetainedValueAdmission::for_program(
        RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram {
            runtime_types: vec![AwbcRuntimeType::new(
                crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest(),
                AwbcRuntimeTypeShape::Bool,
            )],
            ..AwbcProgram::default()
        })),
        RuntimeSchemaLimits::engine_default(),
    )
}

fn registry() -> RuntimeDialogueActivationRegistry<RuntimeValue, AwbcTypeId> {
    let mut registry = RuntimeDialogueActivationRegistry::default();
    registry
        .begin(activation(0), RuntimeValue::Bool(true))
        .unwrap();
    registry
}

#[test]
fn complete_contour_borrows_actual_withdrawn_and_active_frames_without_copying() {
    let mut registry = registry();
    registry
        .begin(activation(1), RuntimeValue::Bool(false))
        .unwrap();
    let mut transaction = registry.begin_transaction(&activation(0)).unwrap();
    transaction
        .line_mut()
        .commit_result(AwbcTypeId(0), RuntimeValue::Bool(true))
        .unwrap();
    let mut admission = admission();
    let contour = registry
        .retained_contour([&transaction].into_iter(), &mut admission)
        .unwrap();
    contour.admit_retained(&mut admission).unwrap();
    let mut visited = Vec::new();
    contour
        .try_visit_active(|id, frame, line| {
            if *id == activation(0) {
                assert!(std::ptr::eq(frame, transaction.frame()));
                assert!(std::ptr::eq(line, transaction.line()));
            }
            admission.validate(frame)?;
            visited.push(*id);
            Ok::<_, AwbcRuntimeValueSnapshotError>(())
        })
        .unwrap();
    assert_eq!(visited, [activation(0), activation(1)]);
    registry.restore_transaction(transaction).unwrap();
    assert_eq!(
        registry.active_frame(&activation(0)),
        Some(&RuntimeValue::Bool(true))
    );
}

#[test]
fn same_identity_and_revision_from_another_registry_cannot_supply_a_borrowed_owner() {
    let mut registry = registry();
    let mut foreign = super::tests::registry();
    let transaction = registry.begin_transaction(&activation(0)).unwrap();
    let other = foreign.begin_transaction(&activation(0)).unwrap();
    assert!(matches!(
        registry.inspect_commit(&other),
        Err(LineRuntimeError::StaleActivationTransaction)
    ));
    assert!(matches!(
        foreign.inspect_commit(&transaction),
        Err(LineRuntimeError::StaleActivationTransaction)
    ));
    let mut admission = admission();
    assert!(
        registry
            .retained_contour([&other].into_iter(), &mut admission)
            .is_err()
    );
    assert!(
        admission.validate(&RuntimeValue::Unit).is_err(),
        "the first contour failure remains final"
    );
    assert_eq!(transaction.frame(), &RuntimeValue::Bool(true));
    assert_eq!(other.frame(), &RuntimeValue::Bool(true));
    registry.restore_transaction(transaction).unwrap();
    foreign.restore_transaction(other).unwrap();
    assert_eq!(
        registry, foreign,
        "live reservations do not enter restored active state"
    );
}

#[test]
fn missing_and_duplicate_withdrawn_owners_are_rejected_before_a_frame_visitor() {
    let mut registry = registry();
    let transaction = registry.begin_transaction(&activation(0)).unwrap();
    for supplied in [vec![], vec![&transaction, &transaction]] {
        let mut admission = admission();
        assert!(
            registry
                .retained_contour(supplied.into_iter(), &mut admission)
                .is_err()
        );
        assert!(admission.validate(&RuntimeValue::Unit).is_err());
    }
    registry.restore_transaction(transaction).unwrap();
}

#[test]
fn borrowed_contour_metadata_uses_the_original_trusted_allowance() {
    let mut registry = registry();
    let transaction = registry.begin_transaction(&activation(0)).unwrap();
    let mut admission = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
        max_sequence_items: 0,
        ..RuntimeSchemaLimits::engine_default()
    });
    assert!(
        registry
            .retained_contour([&transaction].into_iter(), &mut admission)
            .is_err()
    );
    assert!(admission.validate(&RuntimeValue::Unit).is_err());
    registry.restore_transaction(transaction).unwrap();
}
