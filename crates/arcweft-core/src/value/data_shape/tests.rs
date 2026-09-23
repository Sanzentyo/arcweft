use std::sync::Arc;

use super::*;
use crate::{
    awbc::schema::{AwbcAgentTypeShape, AwbcRuntimeType, AwbcRuntimeTypeShape},
    pattern::RuntimeCheckedType,
    plan::{
        RuntimeAgentTypeProjection, RuntimePlanBuilder, RuntimePlanTypeProjection,
        RuntimePlanTypeSeed,
    },
    value::RuntimeAgentValue,
};

fn identity(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn owners() -> [RuntimeProgramOwner; 2] {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    identity(2),
                    RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::DataShape(
                        identity(1),
                    )),
                ),
                RuntimePlanTypeSeed::new(identity(3), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    identity(4),
                    RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::DataShape(
                        identity(3),
                    )),
                ),
            ],
            [],
        )
        .unwrap();
    let awbc = AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::new(identity(1), AwbcRuntimeTypeShape::Bool),
            AwbcRuntimeType::new(
                identity(2),
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(
                    crate::awbc::schema::AwbcTypeId(0),
                )),
            ),
            AwbcRuntimeType::new(identity(3), AwbcRuntimeTypeShape::String),
            AwbcRuntimeType::new(
                identity(4),
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(
                    crate::awbc::schema::AwbcTypeId(2),
                )),
            ),
        ],
        ..AwbcProgram::default()
    };
    [
        RuntimeProgramOwner::Plan(Arc::new(builder.finish().unwrap())),
        RuntimeProgramOwner::Awbc(Arc::new(awbc)),
    ]
}

#[test]
fn data_shape_retains_exact_source_type_and_selected_program() {
    let limits = RuntimeSchemaLimits::engine_default();
    for owner in owners() {
        let witness = RuntimeDataShape::bind(owner.clone(), identity(2)).unwrap();
        assert_eq!(witness.shape_type(), identity(2));
        assert_eq!(witness.value_type(), identity(1));
        assert_ne!(
            witness.value_type(),
            RuntimeCheckedType::Bool.semantic_identity_digest()
        );
        witness
            .validate_for(&owner, identity(2), identity(1))
            .unwrap();
        witness
            .validate_value(&RuntimeValue::Bool(true), limits)
            .unwrap();
        assert!(
            witness
                .validate_value(&RuntimeValue::String("wrong".to_owned()), limits)
                .is_err()
        );
        assert_eq!(witness, witness.clone());
        let value = RuntimeValue::Agent(RuntimeAgentValue::DataShape(witness.clone()));
        owner
            .types()
            .validate_live_value(identity(2), &value, limits)
            .unwrap();
        owner
            .types()
            .validate_snapshot_value(identity(2), &value, limits)
            .unwrap();
        assert!(
            owner
                .types()
                .validate_live_value(identity(4), &value, limits)
                .is_err()
        );
        assert!(
            witness
                .validate_for(&owner, identity(2), identity(3))
                .is_err()
        );
        assert!(RuntimeDataShape::bind(owner, identity(1)).is_err());
    }
}

#[test]
fn data_shape_rejects_equivalent_replacement_generations_and_cross_program_values() {
    let limits = RuntimeSchemaLimits::engine_default();
    for (original, replacement) in owners().into_iter().zip(owners()) {
        let witness = RuntimeDataShape::bind(original.clone(), identity(2)).unwrap();
        let next = RuntimeDataShape::bind(replacement.clone(), identity(2)).unwrap();
        assert_ne!(witness, next);
        assert_eq!(
            witness.validate_for(&replacement, identity(2), identity(1)),
            Err(RuntimeDataShapeError::ProgramMismatch)
        );
        let value = RuntimeValue::Agent(RuntimeAgentValue::DataShape(witness));
        assert!(
            replacement
                .types()
                .validate_live_value(identity(2), &value, limits)
                .is_err()
        );
        original
            .types()
            .validate_live_value(identity(2), &value, limits)
            .unwrap();
    }
}

#[test]
fn data_shape_generic_serde_and_unbound_persistence_cannot_create_authority() {
    for owner in owners() {
        let witness = RuntimeDataShape::bind(owner, identity(2)).unwrap();
        let value = RuntimeValue::Agent(RuntimeAgentValue::DataShape(witness));
        assert!(serde_json::to_value(&value).is_err());
        assert!(
            serde_json::from_value::<RuntimeValue>(serde_json::json!({"Agent":{"DataShape":{}}}))
                .is_err()
        );
        assert!(value.try_digest(100).is_err());
    }
}

#[test]
fn data_shape_rejects_erased_or_ambiguous_awbc_rows() {
    let mut program = AwbcProgram::default();
    program.runtime_types.push(AwbcRuntimeType::new(
        identity(2),
        AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
            crate::plan::RuntimeAgentOperationalType::DataShape,
        )),
    ));
    assert!(
        RuntimeDataShape::bind(RuntimeProgramOwner::Awbc(Arc::new(program)), identity(2)).is_err()
    );
    let [_, RuntimeProgramOwner::Awbc(program)] = owners() else {
        unreachable!()
    };
    let mut program = (*program).clone();
    program.runtime_types.push(program.runtime_types[1].clone());
    assert!(matches!(
        RuntimeDataShape::bind(RuntimeProgramOwner::Awbc(Arc::new(program)), identity(2)),
        Err(RuntimeDataShapeError::ProgramType(
            RuntimeProgramTypeError::Ambiguous { .. }
        ))
    ));
}

#[test]
fn data_shape_snapshot_rebinds_only_with_an_explicit_selected_program_and_exact_child() {
    for owner in owners() {
        let witness = RuntimeDataShape::bind(owner.clone(), identity(2)).unwrap();
        let value = RuntimeValue::Tuple(vec![RuntimeValue::option_some(RuntimeValue::Agent(
            RuntimeAgentValue::DataShape(witness),
        ))]);
        let saved = crate::value::AwbcRuntimeValueSnapshot::from_runtime_value(&value).unwrap();
        let encoded = serde_json::to_value(&saved).unwrap();
        let saved: crate::value::AwbcRuntimeValueSnapshot =
            serde_json::from_value(encoded.clone()).unwrap();
        assert!(serde_json::from_value::<RuntimeValue>(encoded.clone()).is_err());
        assert_eq!(saved.into_runtime_value_for_program(&owner).unwrap(), value);
        let mut wrong_child = encoded;
        wrong_child["Tuple"][0]["Variant"]["payload"]["Tuple"][0]["Agent"]["DataShape"]["value_type"] =
            serde_json::to_value(identity(3)).unwrap();
        let wrong_child: crate::value::AwbcRuntimeValueSnapshot =
            serde_json::from_value(wrong_child).unwrap();
        assert!(wrong_child.into_runtime_value_for_program(&owner).is_err());
    }
}
