use super::*;
use crate::awbc::schema::{AwbcAgentTypeShape, AwbcRuntimeType, AwbcRuntimeTypeShape};
use crate::plan::{
    RuntimeAgentOperationalType, RuntimeAgentTypeProjection, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};

#[test]
fn selected_programs_admit_only_registered_agent_protocol_record_carriers() {
    let resource = RuntimeSemanticTypeId::from_bytes([91; 32]);
    let debug_path = RuntimeSemanticTypeId::from_bytes([92; 32]);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    resource,
                    RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::Resource),
                ),
                RuntimePlanTypeSeed::new(
                    debug_path,
                    RuntimePlanTypeProjection::Agent(RuntimeAgentTypeProjection::DebugStatePath),
                ),
            ],
            [],
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let awbc = AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::new(
                resource,
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                    RuntimeAgentOperationalType::Resource,
                )),
            ),
            AwbcRuntimeType::new(
                debug_path,
                AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                    RuntimeAgentOperationalType::DebugStatePath,
                )),
            ),
        ],
        ..AwbcProgram::default()
    };
    let value = RuntimeValue::try_record(Vec::<(String, RuntimeValue)>::new()).unwrap();
    for types in [
        RuntimeProgramTypes::Plan(&plan),
        RuntimeProgramTypes::Awbc(&awbc),
    ] {
        assert!(
            types
                .validate_live_value(resource, &value, RuntimeSchemaLimits::engine_default())
                .is_ok()
        );
        assert!(
            types
                .validate_live_value(debug_path, &value, RuntimeSchemaLimits::engine_default())
                .is_err()
        );
    }
}

#[test]
fn each_selected_program_uses_source_identity_and_the_same_value_encoder() {
    let semantic = RuntimeSemanticTypeId::from_bytes([71; 32]);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                semantic,
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let awbc = AwbcProgram {
        runtime_types: vec![AwbcRuntimeType::new(semantic, AwbcRuntimeTypeShape::Bool)],
        ..AwbcProgram::default()
    };
    let value = RuntimeValue::Bool(true);
    let diagnostic_identity = RuntimeCheckedType::Bool.semantic_identity_digest();
    assert_ne!(semantic, diagnostic_identity);
    for program in [
        RuntimeProgramTypes::Plan(&plan),
        RuntimeProgramTypes::Awbc(&awbc),
    ] {
        assert_eq!(
            program.checked_type(semantic).unwrap(),
            RuntimeCheckedType::Bool
        );
        assert_eq!(
            program
                .accepts_value(semantic, &value, RuntimeSchemaLimits::engine_default())
                .unwrap(),
            value.try_digest(100).unwrap()
        );
        assert!(matches!(
            program.accepts_value(
                diagnostic_identity,
                &value,
                RuntimeSchemaLimits::engine_default()
            ),
            Err(RuntimeProgramTypeError::Missing { .. })
        ));
        assert!(
            program
                .accepts_value(
                    semantic,
                    &RuntimeValue::Unit,
                    RuntimeSchemaLimits::engine_default()
                )
                .is_err()
        );
    }
}

#[test]
fn ambiguous_awbc_identity_cannot_select_the_first_compatible_row() {
    let semantic = RuntimeSemanticTypeId::from_bytes([73; 32]);
    let program = AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::new(semantic, AwbcRuntimeTypeShape::Bool),
            AwbcRuntimeType::new(semantic, AwbcRuntimeTypeShape::String),
        ],
        ..AwbcProgram::default()
    };
    let context = RuntimeProgramTypes::Awbc(&program);
    assert!(matches!(
        context.checked_type(semantic),
        Err(RuntimeProgramTypeError::Ambiguous { .. })
    ));
    assert!(matches!(
        context.accepts_value(
            semantic,
            &RuntimeValue::Bool(true),
            RuntimeSchemaLimits::engine_default()
        ),
        Err(RuntimeProgramTypeError::Ambiguous { .. })
    ));
}

#[test]
fn bounded_persistent_digest_charges_the_complete_opaque_payload() {
    use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId};
    let value = RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("fixture.budget").unwrap(),
        RuntimeSemanticTypeId::from_bytes([80; 32]),
    )
    .try_wrap(RuntimeValue::Tuple(vec![RuntimeValue::Bool(true); 2]))
    .unwrap();
    let limits = RuntimeSchemaLimits {
        max_nodes: 4,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert_eq!(
        value.try_digest_with_limits(limits).unwrap(),
        value.try_digest(1024).unwrap()
    );
    assert!(matches!(
        value.try_digest_with_limits(RuntimeSchemaLimits {
            max_nodes: 3,
            ..limits
        }),
        Err(crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "nodes" })
    ));
    assert!(matches!(
        value.try_digest_with_limits(RuntimeSchemaLimits {
            max_sequence_items: 1,
            ..limits
        }),
        Err(crate::entry::RuntimeSchemaError::BudgetExceeded {
            budget: "sequence_items"
        })
    ));
}
