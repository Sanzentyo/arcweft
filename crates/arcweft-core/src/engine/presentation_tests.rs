use super::*;
use crate::presentation::RuntimePresentationError;
fn bodies<V>(retired: Vec<crate::presentation::resources::PresentationRetirement<V>>) -> Vec<V> {
    retired
        .into_iter()
        .map(|retired| retired.into_body())
        .collect()
}
use crate::presentation::resources::{
    PresentationLedger, PresentationLedgerLimits, PresentationRegistrationSite,
};
use arcweft_interaction_model::presentation::PresentationResourceKind;

fn engine() -> Engine {
    Engine::new(crate::plan::RuntimePlanBuilder::new().finish().unwrap())
}

fn ledger<V>(engine: &Engine) -> PresentationLedger<V> {
    PresentationLedger::new(
        engine.fiber.frame.program.clone(),
        crate::effect::RuntimeArtifactFingerprint::try_from_bytes([11; 32]).unwrap(),
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
        bodies(
            engine
                .fiber
                .release_presentation_scope(&mut ledger)
                .unwrap()
        ),
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
        RuntimePresentationError::Owner
    );
    assert_eq!(
        ledger.release(&program, &token, Some(first)),
        Err(RuntimePresentationError::UnknownHandle)
    );
    assert_eq!(
        bodies(
            engine
                .fiber
                .release_presentation_scope(&mut ledger)
                .unwrap()
        ),
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
        Err(RuntimePresentationError::Execution)
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
    bodies(
        engine
            .fiber
            .release_presentation_scope(&mut ledger)
            .unwrap(),
    );
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
        Err(RuntimePresentationError::Program)
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

fn typed_engine(artifact: bool) -> (Engine, crate::pattern::RuntimeSemanticTypeId) {
    use crate::plan::{
        RuntimeLocalDeclarationSeed, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    };
    let ty = crate::pattern::RuntimeSemanticTypeId::from_bytes([91; 32]);
    let mut builder = crate::plan::RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                ty,
                RuntimePlanTypeProjection::Bool,
            )],
            std::iter::empty::<RuntimeLocalDeclarationSeed>(),
        )
        .unwrap();
    let mut plan = builder.finish().unwrap();
    if artifact {
        plan.bind_artifact(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([31; 32]).unwrap(),
        )
        .unwrap();
    }
    (Engine::new(plan), ty)
}

fn view_site() -> PresentationRegistrationSite {
    PresentationRegistrationSite::Independent(PresentationResourceKind::View)
}

#[test]
fn engine_owned_presentation_survives_rollback_and_ordinary_scope_exit() {
    let (mut engine, ty) = typed_engine(true);
    engine.push_scope_frame(crate::scope::RuntimeScopeIdentity::Anonymous);
    let (token, replaced) = engine
        .register_presentation_body(view_site(), ty, RuntimePayload(RuntimeValue::Bool(true)))
        .unwrap();
    assert!(replaced.is_none());
    assert_eq!(token.execution(), engine.fiber.execution);
    let owner = engine.fiber.frame.program.clone();
    let image = engine.inert_rollback_image().unwrap();
    let mut restored = Engine::from_rollback_image(image.clone(), &owner).unwrap();
    assert_eq!(restored.inert_rollback_image().unwrap(), image);
    restored
        .exit_emitted_scope(
            &mut RuntimeStepOutput::default(),
            &mut crate::pure::VmRuntimePureCallBackend::default(),
        )
        .unwrap();
    assert!(
        restored
            .fiber
            .frame
            .env
            .presentation_scope_binding()
            .is_none()
    );
    assert_eq!(restored.presentation_retirements.len(), 1);
    assert_eq!(restored.presentation_retirements[0].token, token);
    assert_eq!(
        restored.presentation_retirements[0].body.value.value(),
        &RuntimeValue::Bool(true)
    );
    let retired_image = restored.inert_rollback_image().unwrap();
    let retired = Engine::from_rollback_image(retired_image.clone(), &owner).unwrap();
    assert_eq!(retired.inert_rollback_image().unwrap(), retired_image);
    assert_eq!(retired.presentation_retirements.len(), 1);
    restored.push_scope_frame(crate::scope::RuntimeScopeIdentity::Anonymous);
    let (next, _) = restored
        .register_presentation_body(view_site(), ty, RuntimePayload(RuntimeValue::Bool(false)))
        .unwrap();
    assert!(next.issuance() > token.issuance());
    restored
        .exit_emitted_scope(
            &mut RuntimeStepOutput::default(),
            &mut crate::pure::VmRuntimePureCallBackend::default(),
        )
        .unwrap();
    assert_eq!(restored.presentation_retirements.len(), 2);
}

#[test]
fn rejected_presentation_body_returns_custody_without_binding_or_allocating() {
    let (mut engine, ty) = typed_engine(true);
    let rejected = engine
        .register_presentation_body(view_site(), ty, RuntimePayload(RuntimeValue::Unit))
        .unwrap_err();
    assert_eq!(rejected.body.value(), &RuntimeValue::Unit);
    assert!(engine.presentation.is_none());
    assert!(
        engine
            .fiber
            .frame
            .env
            .presentation_scope_binding()
            .is_none()
    );
    let (mut engine, ty) = typed_engine(false);
    let rejected = engine
        .register_presentation_body(view_site(), ty, RuntimePayload(RuntimeValue::Bool(true)))
        .unwrap_err();
    assert!(matches!(
        rejected.cause,
        crate::value::AwbcRuntimeValueSnapshotError::Presentation(
            RuntimePresentationError::MissingArtifact
        )
    ));
    assert_eq!(rejected.body.value(), &RuntimeValue::Bool(true));
    assert!(engine.presentation.is_none());
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
fn rollback_rejects_missing_ledger_duplicate_retirement_and_foreign_program() {
    let (mut engine, ty) = typed_engine(true);
    engine.push_scope_frame(crate::scope::RuntimeScopeIdentity::Anonymous);
    engine
        .register_presentation_body(view_site(), ty, RuntimePayload(RuntimeValue::Bool(true)))
        .unwrap();
    let owner = engine.fiber.frame.program.clone();
    let image = engine.inert_rollback_image().unwrap();
    let mut missing = image.clone();
    missing.presentation = None;
    assert!(Engine::from_rollback_image(missing, &owner).is_err());
    let mut orphan = image.clone();
    orphan.fiber.frame.env = crate::value::RuntimeEnv::default()
        .inert_rollback_image(&owner)
        .unwrap();
    assert!(Engine::from_rollback_image(orphan, &owner).is_err());
    let (foreign, _) = typed_engine(true);
    assert!(Engine::from_rollback_image(image.clone(), &foreign.fiber.frame.program).is_err());
    assert_eq!(engine.inert_rollback_image().unwrap(), image);
    engine
        .exit_emitted_scope(
            &mut RuntimeStepOutput::default(),
            &mut crate::pure::VmRuntimePureCallBackend::default(),
        )
        .unwrap();
    let valid = engine.inert_rollback_image().unwrap();
    let mut duplicate = valid.clone();
    duplicate
        .presentation_retirements
        .push(duplicate.presentation_retirements[0].clone());
    assert!(Engine::from_rollback_image(duplicate, &owner).is_err());
    assert_eq!(engine.inert_rollback_image().unwrap(), valid);
}

#[test]
fn native_completion_and_child_close_retire_nested_presentation_scopes_in_order() {
    for close_child in [false, true] {
        let (mut engine, ty) = typed_engine(true);
        let (outer, _) = engine
            .register_presentation_body(view_site(), ty, RuntimePayload(RuntimeValue::Bool(false)))
            .unwrap();
        engine.push_scope_frame(crate::scope::RuntimeScopeIdentity::Anonymous);
        let (inner, _) = engine
            .register_presentation_body(view_site(), ty, RuntimePayload(RuntimeValue::Bool(true)))
            .unwrap();
        let mut output = RuntimeStepOutput::default();
        let mut backend = crate::pure::VmRuntimePureCallBackend::default();
        if close_child {
            engine.close_active_child_fiber(&mut output, &mut backend);
        } else {
            engine.finish(&mut output, &mut backend);
        }
        assert!(matches!(
            engine.fiber.frame.status,
            FlowFiberStatus::Done(_)
        ));
        assert!(engine.fiber.frame.control_stack.is_empty());
        assert_eq!(engine.presentation_retirements.len(), 2);
        assert_eq!(engine.presentation_retirements[0].token, inner);
        assert_eq!(engine.presentation_retirements[1].token, outer);
        assert!(engine.inert_rollback_image().is_ok());
    }
}
