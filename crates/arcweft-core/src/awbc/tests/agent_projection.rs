use super::*;
use crate::awbc::type_projection::AwbcTypeProjectionError;
use crate::plan::{
    RuntimeAgentTypeProjection, RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};

#[test]
fn agent_probe_result_survives_plan_and_awbc_projection() {
    let bool_identity = RuntimeSemanticTypeId::from_bytes([1; 32]);
    let probe_identity = RuntimeSemanticTypeId::from_bytes([2; 32]);
    let expected = RuntimeCheckedType::Agent(RuntimeAgentTypeProjection::Probe(Box::new(
        RuntimeCheckedType::Bool,
    )));
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(bool_identity, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    probe_identity,
                    RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Probe(
                        bool_identity,
                    )),
                ),
            ],
            [],
            [],
            [],
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let ty = plan.type_table().id_for_semantic(probe_identity).unwrap();
    assert_eq!(plan.checked_type(ty).unwrap(), Some(expected.clone()));

    let mut program = minimal_program();
    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Bool),
        runtime_type(
            2,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Probe(AwbcTypeId(0))),
        ),
    ];
    let encoded = program.encode_canonical().unwrap();
    let restored = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(restored.checked_type(AwbcTypeId(1)).unwrap(), expected);
    restored
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    assert_eq!(restored.encode_canonical().unwrap(), encoded);
    let string_probe = RuntimeCheckedType::Agent(RuntimeAgentTypeProjection::Probe(Box::new(
        RuntimeCheckedType::String,
    )));
    assert_ne!(
        expected.semantic_identity_digest(),
        string_probe.semantic_identity_digest()
    );
    let restored: RuntimeCheckedType =
        serde_json::from_slice(&serde_json::to_vec(&expected).unwrap()).unwrap();
    assert_eq!(restored, expected);
    assert!(
        serde_json::from_value::<RuntimeCheckedType>(serde_json::json!({"Agent":"Probe"})).is_err()
    );
}

#[test]
fn probe_rows_require_a_complete_acyclic_checked_result() {
    let mut program = minimal_program();
    program.runtime_types = vec![runtime_type(
        1,
        AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(RuntimeAgentOperationalType::Probe)),
    )];
    assert!(matches!(
        program.checked_type(AwbcTypeId(0)),
        Err(AwbcTypeProjectionError::UnsupportedCheckedType { index: 0 })
    ));
    assert!(
        matches!(program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(AwbcVerifyError::InvalidInvariant { at, .. }) if at == "runtime type 0")
    );
    program.runtime_types = vec![runtime_type(
        1,
        AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Probe(AwbcTypeId(0))),
    )];
    assert!(matches!(
        program.checked_type(AwbcTypeId(0)),
        Err(AwbcTypeProjectionError::CheckedTypeCycle { index: 0 })
    ));
    program.runtime_types = vec![runtime_type(
        1,
        AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Probe(AwbcTypeId(1))),
    )];
    assert!(matches!(
        program.checked_type(AwbcTypeId(0)),
        Err(AwbcTypeProjectionError::RuntimeTypeOutOfBounds { index: 1 })
    ));

    program.runtime_types = vec![
        runtime_type(1, AwbcRuntimeTypeShape::Bool),
        runtime_type(2, AwbcRuntimeTypeShape::Range(AwbcTypeId(0))),
        runtime_type(
            3,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Probe(AwbcTypeId(1))),
        ),
    ];
    assert!(matches!(
        program.checked_type(AwbcTypeId(2)),
        Err(AwbcTypeProjectionError::UnsupportedCheckedType { index: 1 })
    ));
}

#[test]
fn every_agent_leaf_reifies_with_its_existing_semantic_identity() {
    for tag in 0..=u8::MAX {
        let Some(kind) = RuntimeAgentOperationalType::from_semantic_tag(tag) else {
            continue;
        };
        if kind == RuntimeAgentOperationalType::Probe {
            assert!(
                RuntimeAgentTypeProjection::<Box<RuntimeCheckedType>>::try_leaf(kind).is_none()
            );
            continue;
        }
        let expected =
            RuntimeCheckedType::Agent(RuntimeAgentTypeProjection::try_leaf(kind).unwrap());
        let mut program = minimal_program();
        program.runtime_types = vec![runtime_type(
            1,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(kind)),
        )];
        assert_eq!(program.checked_type(AwbcTypeId(0)).unwrap(), expected);
        let mut encoder = crate::pattern::RuntimeSemanticTypeIdentityEncoder::new();
        encoder.write_tag(21);
        encoder.write_u8(tag);
        assert_eq!(expected.semantic_identity_digest(), encoder.finish());
    }
}
