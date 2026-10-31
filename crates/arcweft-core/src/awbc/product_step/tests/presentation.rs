use super::*;
use crate::presentation::resources::{
    PresentationBanks, PresentationBody, PresentationLedgerLimits, PresentationRegistrationSite,
};
use crate::runtime_id::{RuntimePresentationBankId, RuntimePresentationHandleToken};
use arcweft_interaction_model::presentation::PresentationResourceKind;

fn fixture() -> (AwbcProductStepExecutor, RuntimePresentationHandleToken) {
    let mut program = return_program();
    let expected = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    program
        .runtime_types
        .push(AwbcRuntimeType::new(expected, AwbcRuntimeTypeShape::Bool));
    let mut executor = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let mut banks = PresentationBanks::new(
        owner.clone(),
        executor.artifact_fingerprint,
        PresentationLedgerLimits::for_schema(crate::entry::RuntimeSchemaLimits::engine_default()),
    );
    let ledger = banks.ledger_mut(RuntimePresentationBankId::Root).unwrap();
    let scope = executor
        .fiber
        .bind_presentation_scope(&owner, ledger)
        .unwrap();
    let (token, _) = ledger
        .register(
            &owner,
            scope,
            PresentationRegistrationSite::Independent(PresentationResourceKind::View),
            PresentationBody {
                expected,
                binding: None,
                value: RuntimePayload(RuntimeValue::Bool(true)),
            },
        )
        .unwrap();
    executor.presentation = Some(banks);
    (executor, token)
}

#[test]
fn product_public_save_restores_actual_scope_bank_body_and_original_program() {
    let (executor, _) = fixture();
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let value = serde_json::to_value(save).unwrap();
    assert!(
        value["presentation"]["ledgers"][0][1]
            .get("limits")
            .is_none()
    );
    assert!(value["presentation"].get("program").is_none());
    let save = serde_json::from_value(value).unwrap();
    let restored = executor.restore_inert_snapshot_owned(save).unwrap();
    assert_eq!(restored.snapshot().unwrap(), before);
    let mut bool_roots = 0;
    restored
        .visit_live_runtime_values(&mut |value: &RuntimeValue| {
            bool_roots += usize::from(matches!(value, RuntimeValue::Bool(true)));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(bool_roots, 1, "resource bodies are part of the live graph");
}

#[test]
fn product_public_restore_rejects_missing_bank_without_losing_the_live_owner() {
    let (executor, _) = fixture();
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let mut value = serde_json::to_value(save).unwrap();
    value["presentation"] = serde_json::Value::Null;
    let save = serde_json::from_value(value).unwrap();
    let (restored, error) = executor.restore_inert_snapshot_owned(save).unwrap_err();
    assert!(error.to_string().contains("presentation"), "{error}");
    assert_eq!(restored.snapshot().unwrap(), before);
}

#[test]
fn product_save_visitors_include_retirements_and_body_headers_reject_wrong_types() {
    let (mut executor, _) = fixture();
    let owner = crate::task::RuntimeProgramOwner::Awbc(executor.program_arc());
    let mut banks = executor.presentation.take().unwrap();
    let ledger = banks.ledger_mut(RuntimePresentationBankId::Root).unwrap();
    let retired = executor
        .fiber
        .release_presentation_scope(&owner, ledger)
        .unwrap();
    banks.retirements_mut().extend(retired);
    executor.presentation = Some(banks);
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let mut value = serde_json::to_value(save).unwrap();
    let restored = executor
        .restore_inert_snapshot_owned(serde_json::from_value(value.clone()).unwrap())
        .unwrap();
    assert_eq!(restored.snapshot().unwrap(), before);
    value["presentation"]["retirements"][0]["body"]["expected"] =
        serde_json::to_value(crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest())
            .unwrap();
    let (restored, _) = restored
        .restore_inert_snapshot_owned(serde_json::from_value(value).unwrap())
        .unwrap_err();
    assert_eq!(restored.snapshot().unwrap(), before);
    let mut bool_roots = 0;
    restored
        .visit_live_runtime_values(&mut |value: &RuntimeValue| {
            bool_roots += usize::from(matches!(value, RuntimeValue::Bool(true)));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(bool_roots, 1);
}

#[test]
fn public_restore_rejects_forged_bank_and_namespace_frontiers_atomically() {
    let (mut executor, _) = fixture();
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let original = serde_json::to_value(save).unwrap();
    let activation = crate::runtime_id::DialogueActivationId::new(
        executor.artifact_fingerprint,
        crate::runtime_id::RuntimePersistentFiberId::from_allocated(1),
        crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
            std::num::NonZeroU32::MIN,
        ),
        0,
    );
    let row = serde_json::json!({"owner": activation.owner_fiber(), "content": activation.content(), "next": 1});
    for mutation in 0..6 {
        let mut wire = original.clone();
        match mutation {
            0 => {
                wire["dialogue_occurrences"] = serde_json::json!([row]);
                wire["fiber"]["presentation_bank"] =
                    serde_json::to_value(RuntimePresentationBankId::Dialogue { activation })
                        .unwrap();
            }
            1 => wire["dialogue_occurrences"] = serde_json::json!([row, row]),
            2 => {
                wire["dialogue_occurrences"] = serde_json::json!([row]);
                wire["dialogue_occurrences"][0]["next"] = 0.into();
            }
            3 => {
                wire["dialogue_occurrences"] = serde_json::json!([row]);
                wire["dialogue_occurrences"][0]["owner"] = serde_json::to_value(
                    crate::runtime_id::RuntimePersistentFiberId::from_allocated(2),
                )
                .unwrap();
            }
            4 => {
                wire["fiber"]
                    .as_object_mut()
                    .unwrap()
                    .remove("presentation_bank");
            }
            5 => {
                wire["dialogue_occurrences"] = serde_json::json!([row, row]);
                let body = wire["presentation"]["ledgers"][0][1]["leases"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap();
                body["body"]["expected"] = serde_json::to_value(
                    crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest(),
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let save = match serde_json::from_value(wire) {
            Ok(save) => save,
            Err(_) if mutation == 4 => continue,
            Err(error) => panic!("mutation {mutation} must reach retained admission: {error}"),
        };
        let (retained, error) = executor.restore_inert_snapshot_owned(save).unwrap_err();
        if mutation == 5 {
            assert!(
                error.to_string().contains(
                    &crate::presentation::RuntimePresentationError::InvalidState.to_string()
                ),
                "metadata must reject before invalid body admission: {error}"
            );
        }
        executor = retained;
        assert_eq!(executor.snapshot().unwrap(), before, "mutation {mutation}");
    }
}

#[test]
fn dialogue_bank_roundtrips_and_duplicate_activation_fiber_rejection_keeps_the_owner() {
    let (executor, activation) = initial_actor_dialogue_executor();
    let bank = RuntimePresentationBankId::Dialogue { activation };
    let fiber = executor
        .dialogues
        .active_frame()
        .unwrap()
        .activation_fiber()
        .unwrap();
    assert_ne!(fiber.instance, executor.fiber.instance);
    assert_eq!(fiber.presentation_bank(), bank);
    let before = executor.snapshot().unwrap();
    let save = AwbcProductExecutorSaveSnapshot::from_live(&before, &executor.program).unwrap();
    let mut wire = serde_json::to_value(save).unwrap();
    let executor = executor
        .restore_inert_snapshot_owned(serde_json::from_value(wire.clone()).unwrap())
        .unwrap();
    assert_eq!(executor.snapshot().unwrap(), before);
    assert_eq!(
        executor
            .dialogues
            .active_frame()
            .unwrap()
            .activation_fiber()
            .unwrap()
            .presentation_bank(),
        bank
    );
    wire["dialogues"]["entries"][0]["Active"]["frame"]["phase"]["Activating"]["fiber"]["instance"] =
        serde_json::to_value(executor.fiber.instance).unwrap();
    let (executor, error) = executor
        .restore_inert_snapshot_owned(serde_json::from_value(wire).unwrap())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains(&crate::presentation::RuntimePresentationError::InvalidState.to_string()),
        "{error}"
    );
    assert_eq!(executor.snapshot().unwrap(), before);
}
