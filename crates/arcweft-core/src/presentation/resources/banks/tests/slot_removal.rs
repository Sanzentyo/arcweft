use super::*;
use crate::plan::{RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed};

const OPTION: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([0x74; 32]);
const PAYLOAD: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([0x75; 32]);

fn typed_fixture() -> (
    RuntimeProgramOwner,
    RuntimeArtifactFingerprint,
    RuntimeSemanticTypeId,
    RuntimePresentationBankId,
) {
    let (_, artifact, boolean, dialogue) = fixture();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    PAYLOAD,
                    RuntimePlanTypeProjection::Tuple(Box::new([boolean])),
                ),
                RuntimePlanTypeSeed::new(
                    OPTION,
                    RuntimePlanTypeProjection::Option {
                        item: boolean,
                        some_payload: PAYLOAD,
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let mut plan = builder.finish().unwrap();
    plan.bind_artifact(artifact).unwrap();
    (
        RuntimeProgramOwner::Plan(Arc::new(plan)),
        artifact,
        boolean,
        dialogue,
    )
}

fn address() -> PresentationSlotAddress {
    let PresentationRegistrationSite::Slot(address) = site() else {
        unreachable!()
    };
    address
}

#[test]
fn slot_removal_preview_counts_the_future_option_once_with_all_other_banks() {
    let (program, artifact, boolean, dialogue) = typed_fixture();
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 4,
        },
    );
    let mut root = RuntimeEnv::default();
    let mut child = RuntimeEnv::default();
    for (bank, env, value) in [
        (RuntimePresentationBankId::Root, &mut root, false),
        (dialogue, &mut child, true),
    ] {
        let scope = bind(&mut banks, bank, &program, env);
        banks
            .register_and_retire(&program, scope, site(), body(boolean, value))
            .unwrap();
    }
    let before = banks.snapshot_admitted().unwrap();
    let address = address();
    // The actual other-bank Bool plus Some -> unary tuple -> selected Bool.
    // Test both live and inert storage without activating an inert descendant.
    for nodes in [3, 4] {
        for inert in [false, true] {
            let mut admission = RuntimeRetainedValueAdmission::for_program(
                program.clone(),
                RuntimeSchemaLimits {
                    max_nodes: nodes,
                    ..RuntimeSchemaLimits::engine_default()
                },
            );
            let removal = Some(PresentationSlotRemovalAdmission {
                bank: dialogue,
                address: &address,
                result: OPTION,
                binding: None,
            });
            let result = if inert {
                before.admit_retained_for_slot_removal(&program, artifact, &mut admission, removal)
            } else {
                banks.admit_retained_for_slot_removal(&program, artifact, &mut admission, removal)
            };
            if nodes == 3 {
                let error = result.unwrap_err();
                assert_eq!(
                    error,
                    AwbcRuntimeValueSnapshotError::Resource(
                        crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "nodes" }
                    )
                );
                assert_eq!(admission.status(), Err(error));
            } else {
                result.unwrap();
                root.admit_retained(&mut admission).unwrap();
                child.admit_retained(&mut admission).unwrap();
                admission.finish_presentation_custody().unwrap();
                assert!(
                    admission.validate(&RuntimeValue::Bool(false)).is_err(),
                    "no fresh allowance after the prospective result"
                );
            }
            assert_eq!(banks.snapshot_admitted().unwrap(), before);
        }
    }
    let removed = banks
        .clear_slot(&program, dialogue, &address)
        .unwrap()
        .unwrap();
    assert_eq!(removed.value.value(), &RuntimeValue::Bool(true));
    assert!(
        banks
            .clear_slot(&program, dialogue, &address)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        banks
            .clear_slot(&program, RuntimePresentationBankId::Root, &address)
            .unwrap()
            .unwrap()
            .value
            .value(),
        &RuntimeValue::Bool(false)
    );
}

#[test]
fn slot_removal_preview_keeps_the_original_stored_type_obligation() {
    let (program, artifact, _, dialogue) = typed_fixture();
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 4,
        },
    );
    let mut env = RuntimeEnv::default();
    let scope = bind(&mut banks, dialogue, &program, &mut env);
    // The prospective Option<Bool> is valid, but this stored body's original
    // expected Option type cannot admit the Bool. It must still be checked.
    banks
        .register_and_retire(&program, scope, site(), body(OPTION, true))
        .unwrap();
    let before = banks.snapshot_admitted().unwrap();
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    let address = address();
    let error = banks
        .admit_retained_for_slot_removal(
            &program,
            artifact,
            &mut admission,
            Some(PresentationSlotRemovalAdmission {
                bank: dialogue,
                address: &address,
                result: OPTION,
                binding: None,
            }),
        )
        .unwrap_err();
    assert!(
        matches!(&error, AwbcRuntimeValueSnapshotError::ProgramType(_)),
        "{error:?}"
    );
    assert_eq!(admission.status(), Err(error));
    assert_eq!(banks.snapshot_admitted().unwrap(), before);
}

#[test]
fn empty_slot_removal_never_creates_a_bank_and_rejects_a_foreign_lease() {
    let (program, artifact, _, dialogue) = typed_fixture();
    let mut banks: PresentationBanks<PresentationBody<RuntimePayload>> = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 4,
        },
    );
    let before = banks.snapshot_admitted().unwrap();
    let address = address();
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits {
            max_nodes: 1,
            ..RuntimeSchemaLimits::engine_default()
        },
    );
    banks
        .admit_retained_for_slot_removal(
            &program,
            artifact,
            &mut admission,
            Some(PresentationSlotRemovalAdmission {
                bank: dialogue,
                address: &address,
                result: OPTION,
                binding: None,
            }),
        )
        .unwrap();
    admission.finish_presentation_custody().unwrap();
    assert!(
        banks
            .clear_slot(&program, dialogue, &address)
            .unwrap()
            .is_none()
    );
    assert_eq!(banks.snapshot_admitted().unwrap(), before);
    let RuntimeProgramOwner::Plan(plan) = program else {
        unreachable!()
    };
    let foreign = RuntimeProgramOwner::Plan(Arc::new((*plan).clone()));
    assert!(matches!(
        banks.clear_slot(&foreign, dialogue, &address),
        Err(RuntimePresentationError::Program)
    ));
    assert_eq!(banks.snapshot_admitted().unwrap(), before);
}
