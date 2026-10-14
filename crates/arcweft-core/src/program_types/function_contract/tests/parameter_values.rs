use super::*;
use crate::entry::RuntimeSchemaLimits;
use crate::program_types::RuntimeParameterValueAdmissionError;
use std::sync::Arc;

fn program() -> Arc<AwbcProgram> {
    let program = AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::unit(),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                AwbcRuntimeTypeShape::String,
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x12; 32]),
                AwbcRuntimeTypeShape::Function {
                    contract: RuntimeFunctionTypeContract::new(
                        RuntimeTypeBinder::new(0, 0, 1),
                        EffectPredicate::unconstrained(),
                        EffectFormula::empty(),
                    ),
                    parameters: vec![AwbcTypeId(1), AwbcTypeId(1)],
                    result: AwbcTypeId(0),
                },
            ),
        ],
        ..AwbcProgram::default()
    };
    Arc::new(program)
}

#[test]
fn parameter_value_group_shares_limits_and_rejects_duplicate_or_wrong_typed_inputs() {
    let program = program();
    let first = RuntimeValue::String("first".into());
    let second = RuntimeValue::String("second".into());
    let limits = RuntimeSchemaLimits {
        max_nodes: 2,
        ..RuntimeSchemaLimits::engine_default()
    };
    program
        .validate_parameter_contract_values(AwbcTypeId(2), [(0, &first), (1, &second)], limits)
        .unwrap();
    assert!(matches!(
        program.validate_parameter_contract_values(
            AwbcTypeId(2),
            [(0, &first), (1, &second)],
            RuntimeSchemaLimits {
                max_nodes: 1,
                ..limits
            }
        ),
        Err(RuntimeParameterValueAdmissionError::Value { ordinal: 1, .. })
    ));
    assert!(matches!(
        program.validate_parameter_contract_values(
            AwbcTypeId(2),
            [(0, &first), (0, &second)],
            limits
        ),
        Err(RuntimeParameterValueAdmissionError::Ordinal { ordinal: 0 })
    ));
    assert!(matches!(
        program.validate_parameter_contract_values(
            AwbcTypeId(2),
            [(0, &first), (1, &RuntimeValue::Bool(true))],
            limits
        ),
        Err(RuntimeParameterValueAdmissionError::Relation { ordinal: 1 })
    ));
}

#[test]
fn parameter_value_group_rejects_affine_input_before_relation_or_materialization() {
    let program = program();
    let value = RuntimeValue::NeedHandle(crate::tests::reusable_need("need.parameter-copy"));
    let error = program
        .validate_parameter_contract_values(
            AwbcTypeId(2),
            [(0, &value)],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap_err();
    assert!(
        matches!(error, RuntimeParameterValueAdmissionError::Value { ordinal: 0, source }
        if matches!(*source, crate::value::AwbcRuntimeValueSnapshotError::AffineValue))
    );
}

#[test]
fn parameter_group_value_work_failure_keeps_its_resource_cause() {
    let program = program();
    let error = program
        .validate_parameter_contract_values(
            AwbcTypeId(2),
            [(0, &RuntimeValue::String("first".into()))],
            RuntimeSchemaLimits {
                max_validation_work: 0,
                ..RuntimeSchemaLimits::engine_default()
            },
        )
        .unwrap_err();
    assert!(
        matches!(error, RuntimeParameterValueAdmissionError::Value { ordinal: 0, source }
        if matches!(*source, crate::value::AwbcRuntimeValueSnapshotError::Resource(
            crate::entry::RuntimeSchemaError::ValidationWork { limit: 0, .. }
        )))
    );
}
