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
    let owner = admission.program_owner().unwrap();
    let snapshot_frame = |frame: &RuntimeValue| Ok(frame == &RuntimeValue::Bool(true));
    let withdrawn = contour
        .snapshot_admitted(Some(owner), |_, frame| snapshot_frame(frame))
        .unwrap();
    registry.restore_transaction(transaction).unwrap();
    assert_eq!(
        withdrawn,
        registry
            .snapshot_admitted(Some(owner), snapshot_frame)
            .unwrap(),
        "projection preserves the original revision, frame, and line without the live reservation",
    );
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

#[test]
fn line_registry_save_roundtrips_under_its_original_activation_and_effect() {
    use crate::awbc::schema::{
        AwbcEffectPayload, AwbcEffectPlan, AwbcEffectPlanId, AwbcSignatureId,
    };
    use crate::effect::{RuntimeRegistryOwner, RuntimeRegistryWriteSource};
    use arcweft_id::{DeclarationName, LifetimeRegistryKey, LifetimeScopeKind};

    let id = activation(0);
    let key = LifetimeRegistryKey::new(
        LifetimeScopeKind::Line,
        vec![DeclarationName::try_new("focus").unwrap()].into_boxed_slice(),
    );
    let ty = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    let program = RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram {
        runtime_types: vec![AwbcRuntimeType::new(ty, AwbcRuntimeTypeShape::Bool)],
        effect_plans: vec![AwbcEffectPlan {
            signature: AwbcSignatureId(0),
            capability: None,
            payload: AwbcEffectPayload::RegistryWrite {
                key: key.clone(),
                value_type: AwbcTypeId(0),
            },
            resources: Vec::new(),
        }],
        ..AwbcProgram::default()
    }));
    let mut registry = registry();
    let mut transaction = registry.begin_transaction(&id).unwrap();
    let source = RuntimeRegistryWriteSource::AwbcLine {
        effect: AwbcEffectPlanId(0),
        activation: id,
    };
    let mut admission = RuntimeRetainedValueAdmission::for_program(
        program.clone(),
        RuntimeSchemaLimits::engine_default(),
    );
    let rows = transaction.line_mut().line_registry_mut().unwrap();
    rows.prepare_write(
        RuntimeRegistryOwner::Dialogue(id),
        &key,
        ty,
        None,
        source,
        &mut admission,
    )
    .unwrap()
    .capture(&RuntimeValue::Bool(true))
    .unwrap()
    .commit(rows)
    .unwrap();
    registry.commit(transaction).unwrap();

    let snapshot = registry
        .to_rollback_snapshot(&program, |frame| Ok(frame == &RuntimeValue::Bool(true)))
        .unwrap();
    let wire = serde_json::to_value(snapshot).unwrap();
    let restore = |wire| {
        RuntimeDialogueActivationRegistry::<RuntimeValue, AwbcTypeId>::from_save_snapshot_with_deferred_children(
            serde_json::from_value(wire).unwrap(),
            &program,
            &BTreeMap::new(),
            |_, frame, _| Ok(RuntimeValue::Bool(frame)),
        )
    };

    let mut restored = restore(wire.clone()).unwrap();
    assert_eq!(restored, registry);
    let mut foreign_activation = wire.clone();
    foreign_activation["entries"][0]["Active"]["activation"] =
        serde_json::to_value(activation(1)).unwrap();
    let error = restore(foreign_activation).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("registry row requires its original executable and activation"),
        "{error}"
    );

    let mut foreign_effect = wire;
    foreign_effect["entries"][0]["Active"]["line"]["line_registry"][0]["source"]["AwbcLine"]["effect"] =
        serde_json::json!(1);
    let error = restore(foreign_effect).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("registry row differs from its original write"),
        "{error}"
    );

    let mut transaction = restored.begin_transaction(&id).unwrap();
    transaction.line_mut().release_frame().unwrap();
    assert!(matches!(
        transaction.line().line_registry(),
        Err(LineRuntimeError::ActivationFrameReleased)
    ));
    restored.restore_transaction(transaction).unwrap();
    let released = restored.to_rollback_snapshot(&program, |_| Ok(())).unwrap();
    let released = serde_json::to_value(released).unwrap();
    assert!(
        released["entries"][0]["Active"]["line"]
            .get("line_registry")
            .is_none()
    );
}
