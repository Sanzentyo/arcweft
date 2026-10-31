use super::*;
use crate::entry::RuntimeSchemaLimits;
use crate::pattern::RuntimeSemanticTypeId;
use crate::runtime_id::{DialogueActivationId, RuntimeDialogueContentPlanId};
use crate::value::{RuntimeEnv, RuntimeValue};
use std::num::NonZeroU32;
use std::sync::Arc;

fn fixture() -> (
    RuntimeProgramOwner,
    RuntimeArtifactFingerprint,
    RuntimeSemanticTypeId,
    RuntimePresentationBankId,
) {
    let expected = RuntimeSemanticTypeId::from_bytes([0x71; 32]);
    let mut builder = crate::plan::RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [crate::plan::RuntimePlanTypeSeed::new(
                expected,
                crate::plan::RuntimePlanTypeProjection::Bool,
            )],
            std::iter::empty::<crate::plan::RuntimeLocalDeclarationSeed>(),
        )
        .unwrap();
    let mut plan = builder.finish().unwrap();
    let artifact = RuntimeArtifactFingerprint::try_from_bytes([0x72; 32]).unwrap();
    plan.bind_artifact(artifact).unwrap();
    let bank = RuntimePresentationBankId::Dialogue {
        activation: DialogueActivationId::new(
            artifact,
            RuntimePersistentFiberId::from_allocated(1),
            RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
            0,
        ),
    };
    (
        RuntimeProgramOwner::Plan(Arc::new(plan)),
        artifact,
        expected,
        bank,
    )
}

fn execution() -> ExecutionInstanceId {
    ExecutionInstanceId::from_allocated(NonZeroU64::MIN)
}
fn fiber() -> RuntimePersistentFiberId {
    RuntimePersistentFiberId::from_allocated(1)
}
fn body(expected: RuntimeSemanticTypeId, value: bool) -> PresentationBody<RuntimePayload> {
    PresentationBody {
        expected,
        binding: None,
        value: RuntimePayload(RuntimeValue::Bool(value)),
    }
}
fn site() -> PresentationRegistrationSite {
    PresentationRegistrationSite::Slot(
        PresentationSlotAddress::try_new(
            PublicId::try_new("target.bank").unwrap(),
            PublicId::try_new("slot.bank").unwrap(),
            PresentationResourceKind::Background,
        )
        .unwrap(),
    )
}
fn bind<V>(
    banks: &mut PresentationBanks<V>,
    bank: RuntimePresentationBankId,
    program: &RuntimeProgramOwner,
    env: &mut RuntimeEnv,
) -> PresentationScopeOwner {
    banks
        .ledger_mut(bank)
        .unwrap()
        .bind_scope(
            program,
            execution(),
            fiber(),
            env.presentation_scope_target().unwrap(),
        )
        .unwrap()
}

#[test]
fn bank_headers_merge_before_bodies_and_same_fiber_grants_round_trip_without_collision() {
    let (program, artifact, expected, dialogue) = fixture();
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 4,
        },
    );
    let mut root_env = RuntimeEnv::default();
    let mut dialogue_env = RuntimeEnv::default();
    let root_scope = bind(
        &mut banks,
        RuntimePresentationBankId::Root,
        &program,
        &mut root_env,
    );
    let dialogue_scope = bind(&mut banks, dialogue, &program, &mut dialogue_env);
    assert_eq!(root_scope.scope, dialogue_scope.scope);
    let (root_token, _) = banks
        .ledger_mut(RuntimePresentationBankId::Root)
        .unwrap()
        .register(&program, root_scope, site(), body(expected, true))
        .unwrap();
    let (dialogue_token, replaced) = banks
        .ledger_mut(dialogue)
        .unwrap()
        .register(&program, dialogue_scope, site(), body(expected, false))
        .unwrap();
    assert!(replaced.is_none());
    assert_eq!(root_token.issuance(), dialogue_token.issuance());
    assert_ne!(root_token, dialogue_token);
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    banks
        .admit_retained(&program, artifact, &mut admission)
        .unwrap();
    root_env.admit_retained(&mut admission).unwrap();
    dialogue_env.admit_retained(&mut admission).unwrap();
    admission.finish_presentation_custody().unwrap();
    let image = banks.snapshot_admitted().unwrap();
    let mut restored = image.clone().into_admitted_live(&program).unwrap();
    assert_eq!(restored.snapshot_admitted().unwrap(), image);
    restored
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            root_env.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert!(
        restored.ledgers[&dialogue].state.leases[&dialogue_token.issuance()]
            .body
            .is_some()
    );
    restored
        .retire_bound_scope(
            &program,
            execution(),
            fiber(),
            dialogue_env.presentation_scope_target().unwrap(),
        )
        .unwrap();
    assert_eq!(restored.retirements.len(), 2);
    assert_eq!(restored.retirements[0].token, root_token);
    assert_eq!(restored.retirements[1].token, dialogue_token);
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    restored
        .admit_retained(&program, artifact, &mut admission)
        .unwrap();
    admission.finish_presentation_custody().unwrap();
    let next_scope = bind(&mut restored, dialogue, &program, &mut dialogue_env);
    let (next_token, _) = restored
        .ledger_mut(dialogue)
        .unwrap()
        .register(&program, next_scope, site(), body(expected, true))
        .unwrap();
    assert!(next_token.issuance() > dialogue_token.issuance());
    assert!(next_scope.scope > dialogue_scope.scope);
}

#[test]
fn malformed_later_bank_header_rejects_before_earlier_bank_body_and_failure_sticks() {
    let (program, artifact, expected, dialogue) = fixture();
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 4,
            scopes: 4,
        },
    );
    let mut env = RuntimeEnv::default();
    let scope = bind(
        &mut banks,
        RuntimePresentationBankId::Root,
        &program,
        &mut env,
    );
    let (token, _) = banks
        .ledger_mut(RuntimePresentationBankId::Root)
        .unwrap()
        .register(
            &program,
            scope,
            site(),
            PresentationBody {
                expected,
                binding: None,
                value: RuntimePayload(RuntimeValue::Unit),
            },
        )
        .unwrap();
    banks.ledger_mut(dialogue).unwrap().state.artifact =
        RuntimeArtifactFingerprint::try_from_bytes([0x73; 32]).unwrap();
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    let error = banks
        .admit_retained(&program, artifact, &mut admission)
        .unwrap_err();
    assert_eq!(error, RuntimePresentationError::InvalidState.into());
    assert_eq!(admission.finish_presentation_custody().unwrap_err(), error);
    assert_eq!(
        banks.ledgers[&RuntimePresentationBankId::Root].state.leases[&token.issuance()]
            .body
            .as_ref()
            .unwrap()
            .value
            .value(),
        &RuntimeValue::Unit
    );
}

#[test]
fn capacity_is_shared_across_banks_and_foreign_bank_does_not_allocate() {
    let (program, artifact, expected, dialogue) = fixture();
    let mut banks = PresentationBanks::new(
        program.clone(),
        artifact,
        PresentationLedgerLimits {
            handles: 2,
            scopes: 2,
        },
    );
    let mut root = RuntimeEnv::default();
    let mut other = RuntimeEnv::default();
    let first = bind(
        &mut banks,
        RuntimePresentationBankId::Root,
        &program,
        &mut root,
    );
    banks
        .ledger_mut(RuntimePresentationBankId::Root)
        .unwrap()
        .register(&program, first, site(), body(expected, true))
        .unwrap();
    let second = bind(&mut banks, dialogue, &program, &mut other);
    banks
        .ledger_mut(dialogue)
        .unwrap()
        .register(&program, second, site(), body(expected, false))
        .unwrap();
    assert_eq!(
        banks.preflight_register(&program),
        Err(RuntimePresentationError::Capacity)
    );
    let RuntimePresentationBankId::Dialogue { activation } = dialogue else {
        unreachable!()
    };
    let foreign = RuntimePresentationBankId::Dialogue {
        activation: DialogueActivationId::new(
            RuntimeArtifactFingerprint::try_from_bytes([0x74; 32]).unwrap(),
            activation.owner_fiber(),
            activation.content(),
            activation.occurrence(),
        ),
    };
    assert_eq!(
        banks.ledger_mut(foreign).unwrap_err(),
        RuntimePresentationError::Program
    );
    assert_eq!(banks.ledgers.len(), 2);
}
