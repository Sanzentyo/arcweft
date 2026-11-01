use super::*;
use crate::{
    plan::RuntimeLocalDeclarationSeed,
    presentation::{
        factory_test_support::{BOOL, HANDLE, fixture},
        resources::PresentationBanks,
    },
    runtime_id::RuntimeLocalDeclarationId,
    value::{RuntimeMutablePlace, RuntimeValueTarget},
};
use arcweft_interaction_model::presentation::PresentationHandleMethodId;

fn fixture_engine() -> (Engine, RuntimeLocalDeclarationId) {
    let (mut builder, _) = fixture();
    let admitted = builder
        .admit_type_batch(
            [],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_source("handle.receiver"),
                HANDLE,
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    builder.declare_test_input_locals(&[local]).unwrap();
    let mut plan = builder.finish().unwrap();
    let local =
        RuntimeLocalDeclarationId::from_accepted_ordinal(std::num::NonZeroU32::new(1).unwrap());
    assert!(plan.local_declarations().contains(local));
    plan.bind_artifact(
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([31; 32]).unwrap(),
    )
    .unwrap();
    let mut engine = Engine::new(plan);
    engine.presentation = Some(PresentationBanks::new(
        engine.fiber.frame.program.clone(),
        engine.plan.artifact().unwrap(),
        PresentationLedgerLimits {
            handles: 1,
            scopes: 8,
        },
    ));
    (engine, local)
}

fn register(
    engine: &mut Engine,
    local: RuntimeLocalDeclarationId,
) -> crate::runtime_id::RuntimePresentationHandleToken {
    let (token, previous) = engine
        .register_presentation_body(
            PresentationRegistrationSite::Independent(PresentationResourceKind::Image),
            BOOL,
            RuntimePayload::new(RuntimeValue::Bool(true)),
        )
        .unwrap();
    assert!(previous.is_none());
    let handle = crate::pattern::RuntimeOpaqueTypeOwner::presentation_handle(
        PresentationResourceKind::Image,
        HANDLE,
    )
    .try_wrap(token.encode_payload())
    .unwrap();
    engine.fiber.frame.env.set_root(local, handle);
    token
}

fn execute(
    engine: &mut Engine,
    local: RuntimeLocalDeclarationId,
    method: PresentationHandleMethodId,
    take: bool,
) -> Result<RuntimeValue, RuntimeEvalError> {
    let ty = engine.plan.type_table().id_for_semantic(HANDLE).unwrap();
    let place = RuntimeMutablePlace::Local(local);
    let target = if take {
        RuntimeValueTarget::Take { place, ty }
    } else {
        RuntimeValueTarget::Inspect { place, ty }
    };
    engine.evaluate_presentation_handle_method(
        method,
        &target,
        &mut crate::pure::VmRuntimePureCallBackend::default(),
    )
}

#[test]
fn native_handle_visibility_changes_the_original_lease_and_survives_rollback() {
    let (mut engine, local) = fixture_engine();
    let token = register(&mut engine, local);
    let before = engine.inert_rollback_image().unwrap();
    execute(&mut engine, local, PresentationHandleMethodId::Hide, false).unwrap();
    let hidden = engine.inert_rollback_image().unwrap();
    assert_ne!(
        before, hidden,
        "the selected operation must change the retained lease"
    );
    execute(&mut engine, local, PresentationHandleMethodId::Hide, false).unwrap();
    assert_eq!(engine.inert_rollback_image().unwrap(), hidden);
    let owner = engine.fiber.frame.program.clone();
    engine = Engine::from_rollback_image(hidden, &owner).unwrap();
    execute(&mut engine, local, PresentationHandleMethodId::Show, false).unwrap();
    assert_ne!(engine.inert_rollback_image().unwrap(), before);
    execute(
        &mut engine,
        local,
        PresentationHandleMethodId::Release,
        true,
    )
    .unwrap();
    assert!(engine.fiber.frame.env.get(local).is_none());
    let retired = engine.presentation.as_ref().unwrap().retirements();
    assert_eq!(retired.len(), 1);
    assert_eq!(retired[0].token, token);
    assert_eq!(retired[0].body.value.0, RuntimeValue::Bool(true));
}

#[test]
fn native_handle_retirement_capacity_rejects_before_moving_the_stored_receiver() {
    let (mut engine, local) = fixture_engine();
    let first = register(&mut engine, local);
    execute(
        &mut engine,
        local,
        PresentationHandleMethodId::Release,
        true,
    )
    .unwrap();
    let second = register(&mut engine, local);
    assert_ne!(first, second);
    let before = engine.inert_rollback_image().unwrap();
    for method in [
        PresentationHandleMethodId::Unmount,
        PresentationHandleMethodId::Release,
        PresentationHandleMethodId::Destroy,
    ] {
        let error = execute(&mut engine, local, method, true).unwrap_err();
        assert!(
            matches!(
                error,
                RuntimeEvalError::Presentation(RuntimePresentationError::RetirementCapacity)
            ),
            "{error:?}"
        );
        assert_eq!(engine.inert_rollback_image().unwrap(), before);
    }
    execute(&mut engine, local, PresentationHandleMethodId::Hide, false).unwrap();
    assert!(engine.fiber.frame.env.get(local).is_some());
}
