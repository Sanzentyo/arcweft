use super::*;
use crate::entry::RuntimeSchemaLimits;
use std::num::NonZeroU32;

fn new_admission() -> RuntimeRetainedValueAdmission {
    RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits::engine_default())
}

fn fiber(id: u64) -> RuntimeFiberInstanceId {
    RuntimeFiberInstanceId::from_allocated(NonZeroU64::new(id).unwrap())
}

fn activation(occurrence: u64) -> DialogueActivationId {
    DialogueActivationId::new(
        RuntimeArtifactFingerprint::try_from_bytes([0x74; 32]).unwrap(),
        RuntimePersistentFiberId::from_allocated(1),
        RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
        occurrence,
    )
}

#[test]
fn namespace_frontiers_reject_duplicates_zero_and_unallocated_owners_stickily() {
    let act = activation(0);
    for rows in [
        vec![
            (act.owner_fiber(), act.content(), 1),
            (act.owner_fiber(), act.content(), 2),
        ],
        vec![(act.owner_fiber(), act.content(), 0)],
        vec![(
            RuntimePersistentFiberId::from_allocated(0),
            act.content(),
            1,
        )],
        vec![(
            RuntimePersistentFiberId::from_allocated(4),
            act.content(),
            1,
        )],
    ] {
        let mut admission = new_admission();
        assert!(
            PresentationExecutionAdmission::new(
                NonZeroU64::new(3),
                rows.iter()
                    .map(|(owner, content, next)| (*owner, *content, next)),
                &mut admission,
            )
            .is_err()
        );
        assert!(
            admission
                .validate(&crate::value::RuntimeValue::Bool(true))
                .is_err()
        );
    }
}

#[test]
fn physical_inventory_includes_dialogue_parents_and_detached_bank_lifetimes() {
    let act = activation(0);
    let next = 1;
    let bank = RuntimePresentationBankId::Dialogue { activation: act };
    let mut admission = new_admission();
    let mut context = PresentationExecutionAdmission::new(
        NonZeroU64::new(4),
        [(act.owner_fiber(), act.content(), &next)].into_iter(),
        &mut admission,
    )
    .unwrap();
    context
        .register_fiber(
            fiber(1),
            RuntimePresentationBankId::Root,
            PresentationExecutionRole::Root,
            &mut admission,
        )
        .unwrap();
    context
        .register_fiber(
            fiber(2),
            bank,
            PresentationExecutionRole::Dialogue(act),
            &mut admission,
        )
        .unwrap();
    context
        .register_fiber(
            fiber(3),
            bank,
            PresentationExecutionRole::ProgramChild { parent: fiber(2) },
            &mut admission,
        )
        .unwrap();
    context
        .register_fiber(
            fiber(4),
            bank,
            PresentationExecutionRole::Independent,
            &mut admission,
        )
        .unwrap();
    context.finish([bank].into_iter(), &mut admission).unwrap();

    // No active Dialogue frame is needed once a line child is detached.
    let mut admission = new_admission();
    let mut context = PresentationExecutionAdmission::new(
        NonZeroU64::new(4),
        [(act.owner_fiber(), act.content(), &next)].into_iter(),
        &mut admission,
    )
    .unwrap();
    context
        .register_fiber(
            fiber(4),
            bank,
            PresentationExecutionRole::Independent,
            &mut admission,
        )
        .unwrap();
    context.finish([bank].into_iter(), &mut admission).unwrap();
}

#[test]
fn physical_inventory_rejects_duplicate_or_unallocated_fibers_and_wrong_roles() {
    let act = activation(0);
    let next = 2;
    let bank = RuntimePresentationBankId::Dialogue { activation: act };
    for (id, bank, role) in [
        (1, bank, PresentationExecutionRole::Dialogue(act)),
        (4, bank, PresentationExecutionRole::Dialogue(act)),
        (2, bank, PresentationExecutionRole::Root),
        (
            2,
            RuntimePresentationBankId::Root,
            PresentationExecutionRole::Dialogue(act),
        ),
        (2, bank, PresentationExecutionRole::Dialogue(activation(1))),
        (
            2,
            RuntimePresentationBankId::Dialogue {
                activation: activation(2),
            },
            PresentationExecutionRole::Independent,
        ),
    ] {
        let mut admission = new_admission();
        let mut context = PresentationExecutionAdmission::new(
            NonZeroU64::new(3),
            [(act.owner_fiber(), act.content(), &next)].into_iter(),
            &mut admission,
        )
        .unwrap();
        context
            .register_fiber(
                fiber(1),
                RuntimePresentationBankId::Root,
                PresentationExecutionRole::Root,
                &mut admission,
            )
            .unwrap();
        assert!(
            context
                .register_fiber(fiber(id), bank, role, &mut admission)
                .is_err()
        );
        assert!(
            admission
                .validate(&crate::value::RuntimeValue::Bool(true))
                .is_err()
        );
    }
}

#[test]
fn program_children_require_the_actual_parent_and_inherited_bank() {
    let act = activation(0);
    let next = 1;
    for parent in [1, 2, 3] {
        let mut admission = new_admission();
        let mut context = PresentationExecutionAdmission::new(
            NonZeroU64::new(3),
            [(act.owner_fiber(), act.content(), &next)].into_iter(),
            &mut admission,
        )
        .unwrap();
        context
            .register_fiber(
                fiber(1),
                RuntimePresentationBankId::Root,
                PresentationExecutionRole::Root,
                &mut admission,
            )
            .unwrap();
        context
            .register_fiber(
                fiber(3),
                RuntimePresentationBankId::Dialogue { activation: act },
                PresentationExecutionRole::ProgramChild {
                    parent: fiber(parent),
                },
                &mut admission,
            )
            .unwrap();
        assert!(context.finish(std::iter::empty(), &mut admission).is_err());
        assert!(
            admission
                .validate(&crate::value::RuntimeValue::Bool(true))
                .is_err()
        );
    }
}

#[test]
fn bank_tombstones_cannot_reference_unissued_dialogue_namespaces() {
    let mut admission = new_admission();
    let context =
        PresentationExecutionAdmission::new(NonZeroU64::new(1), std::iter::empty(), &mut admission)
            .unwrap();
    assert!(
        context
            .finish(
                [RuntimePresentationBankId::Dialogue {
                    activation: activation(0)
                }]
                .into_iter(),
                &mut admission
            )
            .is_err()
    );
    assert!(
        admission
            .validate(&crate::value::RuntimeValue::Bool(true))
            .is_err()
    );
}
