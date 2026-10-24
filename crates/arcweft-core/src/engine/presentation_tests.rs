use super::*;
use crate::presentation::resources::{
    PresentationLedger, PresentationLedgerError, PresentationLedgerLimits,
    PresentationRegistrationSite,
};
use arcweft_interaction_model::presentation::PresentationResourceKind;

fn engine() -> Engine {
    Engine::new(crate::plan::RuntimePlanBuilder::new().finish().unwrap())
}

fn ledger<V>(engine: &Engine) -> PresentationLedger<V> {
    PresentationLedger::new(
        engine.fiber.frame.program.clone(),
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([11; 32]).unwrap(),
        engine.fiber.execution,
        PresentationLedgerLimits {
            handles: 8,
            scopes: 8,
        },
    )
}

#[test]
fn presentation_grants_follow_reached_native_scopes_and_never_alias_reentry() {
    let mut engine = engine();
    let mut ledger = ledger(&engine);
    let program = engine.fiber.frame.program.clone();
    let root = engine.fiber.bind_presentation_scope(&mut ledger).unwrap();
    let root_again = engine.fiber.bind_presentation_scope(&mut ledger).unwrap();
    assert_eq!(root, root_again);
    engine.fiber.frame.env.push_scope();
    let first = engine.fiber.bind_presentation_scope(&mut ledger).unwrap();
    assert_ne!(root, first);
    let (token, _) = ledger
        .register(
            &program,
            first,
            PresentationRegistrationSite::Independent(PresentationResourceKind::View),
            17,
        )
        .unwrap();
    assert_eq!(
        engine
            .fiber
            .release_presentation_scope(&mut ledger)
            .unwrap(),
        [17]
    );
    engine.fiber.frame.env.pop_scope();
    engine.fiber.frame.env.push_scope();
    let second = engine.fiber.bind_presentation_scope(&mut ledger).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        ledger
            .register(
                &program,
                first,
                PresentationRegistrationSite::Independent(PresentationResourceKind::View),
                99
            )
            .unwrap_err()
            .cause,
        PresentationLedgerError::Owner
    );
    assert_eq!(
        ledger.release(&program, &token, Some(first)),
        Err(PresentationLedgerError::UnknownHandle)
    );
    assert_eq!(
        engine
            .fiber
            .release_presentation_scope(&mut ledger)
            .unwrap(),
        Vec::<i32>::new()
    );
    engine.fiber.frame.env.pop_scope();
    assert_eq!(
        engine.fiber.bind_presentation_scope(&mut ledger).unwrap(),
        root
    );
}

#[test]
fn native_presentation_scope_rejects_foreign_execution_before_touching_the_frame() {
    let mut engine = engine();
    let mut ledger = ledger::<i32>(&engine);
    engine.fiber.execution = crate::runtime_id::ExecutionInstanceId::from_allocated(
        std::num::NonZeroU64::new(2).unwrap(),
    );
    assert_eq!(
        engine.fiber.bind_presentation_scope(&mut ledger),
        Err(PresentationLedgerError::Execution)
    );
    assert!(
        engine
            .fiber
            .frame
            .env
            .presentation_scope_binding()
            .is_none()
    );
}

#[test]
fn native_rollback_requires_the_scope_ledger_until_cleanup_retires_its_grant() {
    let mut engine = engine();
    let mut ledger = ledger::<i32>(&engine);
    engine.fiber.bind_presentation_scope(&mut ledger).unwrap();
    let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
        engine.fiber.frame.program.clone(),
        crate::entry::RuntimeSchemaLimits::engine_default(),
    );
    assert!(matches!(
        engine.fiber.frame.env.admit_retained(&mut admission),
        Err(crate::value::AwbcRuntimeValueSnapshotError::PresentationScopeCustodyRequired { .. })
    ));
    assert!(engine.inert_rollback_image().is_err());
    engine
        .fiber
        .release_presentation_scope(&mut ledger)
        .unwrap();
    engine.fiber.frame.env.pop_scope();
    assert!(engine.inert_rollback_image().is_ok());
}

#[test]
fn native_presentation_scope_uses_exact_original_program_lease() {
    let original = engine();
    let mut other = engine();
    let mut ledger = ledger::<i32>(&original);
    assert_eq!(
        other.fiber.bind_presentation_scope(&mut ledger),
        Err(PresentationLedgerError::Program)
    );
    assert!(other.fiber.frame.env.presentation_scope_binding().is_none());
}

#[test]
fn native_child_capture_does_not_copy_the_parent_scope_grant() {
    let mut engine = engine();
    let mut ledger = ledger::<i32>(&engine);
    engine.fiber.bind_presentation_scope(&mut ledger).unwrap();
    let captured = engine
        .fiber
        .frame
        .env
        .try_capture_unrestricted(&[])
        .unwrap();
    assert!(captured.presentation_scope_binding().is_none());
    assert!(
        engine
            .fiber
            .frame
            .env
            .presentation_scope_binding()
            .is_some()
    );
}
