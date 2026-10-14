use super::*;
use crate::value::RuntimeRetainedValueAdmission;

#[test]
fn mixed_retained_roots_share_nodes_bytes_and_sticky_resource_failures() {
    let limits = RuntimeSchemaLimits::engine_default();
    for (limits, budget) in [
        (
            RuntimeSchemaLimits {
                max_nodes: 2,
                ..limits
            },
            "nodes",
        ),
        (
            RuntimeSchemaLimits {
                max_encoded_bytes: 4,
                ..limits
            },
            "encoded_bytes",
        ),
    ] {
        let mut admission = RuntimeRetainedValueAdmission::standalone(limits);
        admission.validate(&RuntimeValue::Bool(true)).unwrap();
        admission
            .validate_snapshot(&AwbcRuntimeValueSnapshot::Bool(false))
            .unwrap();
        let error = admission.validate(&RuntimeValue::Bool(false)).unwrap_err();
        assert_eq!(
            error,
            AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded { budget })
        );
        assert_eq!(
            admission.validate_typed(&RuntimeValue::Unit, semantic(99)),
            Err(error)
        );
    }
}

#[test]
fn enclosing_metadata_uses_the_retained_groups_work_and_remains_sticky() {
    let limits = RuntimeSchemaLimits {
        max_validation_work: 5,
        ..RuntimeSchemaLimits::engine_default()
    };
    let mut admission = RuntimeRetainedValueAdmission::standalone(limits);
    admission.validate_metadata_string("name").unwrap();
    admission.validate_metadata_collection(3).unwrap();
    let error = admission
        .validate_snapshot(&AwbcRuntimeValueSnapshot::Unit)
        .unwrap_err();
    assert_eq!(
        error,
        AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::ValidationWork {
            path: "$".into(),
            limit: 5,
            consumed: 6
        })
    );
    assert_eq!(admission.validate_metadata_collection(0), Err(error));
    let mut admission = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
        max_sequence_items: 2,
        ..limits
    });
    assert_eq!(
        admission.validate_metadata_collection(3),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            RuntimeSchemaError::BudgetExceeded {
                budget: "sequence_items"
            }
        ))
    );
}

#[test]
fn typed_metadata_grammar_shares_depth_collections_and_first_failure_with_values() {
    #[derive(serde::Serialize)]
    struct Metadata<'a> {
        names: &'a [&'a str],
        children: &'a [Vec<bool>],
    }
    let metadata = Metadata {
        names: &["name"],
        children: &[vec![true, false]],
    };
    let limits = RuntimeSchemaLimits::engine_default();
    RuntimeRetainedValueAdmission::standalone(limits)
        .validate_metadata(&metadata)
        .unwrap();
    for (limits, expected) in [
        (
            RuntimeSchemaLimits {
                max_sequence_items: 1,
                ..limits
            },
            "sequence_items",
        ),
        (
            RuntimeSchemaLimits {
                max_string_bytes: 3,
                ..limits
            },
            "string_bytes",
        ),
    ] {
        let mut admission = RuntimeRetainedValueAdmission::standalone(limits);
        let error = admission.validate_metadata(&metadata).unwrap_err();
        assert!(
            matches!(&error, AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded { budget }) if *budget == expected)
        );
        assert_eq!(admission.validate(&RuntimeValue::Unit), Err(error));
    }
    let mut admission = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
        max_depth: 1,
        ..limits
    });
    assert!(matches!(
        admission.validate_metadata(&metadata),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            RuntimeSchemaError::ValidationDepth { limit: 1, .. }
        ))
    ));
    let mut admission = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
        max_validation_work: 3,
        ..limits
    });
    admission.validate_metadata(&vec![true]).unwrap();
    admission
        .validate_snapshot(&AwbcRuntimeValueSnapshot::Unit)
        .unwrap();
    assert!(matches!(
        admission.validate_metadata(&false),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            RuntimeSchemaError::ValidationWork {
                limit: 3,
                consumed: 4,
                ..
            }
        ))
    ));
}

#[test]
fn retained_group_preserves_first_affine_failure_across_live_inert_switches() {
    let mut admission =
        RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits::engine_default());
    let need = RuntimeValue::NeedHandle(crate::tests::reusable_need("need.group-affine"));
    assert_eq!(
        admission.validate(&need),
        Err(AwbcRuntimeValueSnapshotError::AffineValue)
    );
    assert_eq!(
        admission.validate_snapshot(&AwbcRuntimeValueSnapshot::Unit),
        Err(AwbcRuntimeValueSnapshotError::AffineValue)
    );
}

#[test]
fn retained_group_requires_program_for_expected_types_and_keeps_original_nominal_rows() {
    let limits = RuntimeSchemaLimits::engine_default();
    let mut standalone = RuntimeRetainedValueAdmission::standalone(limits);
    assert_eq!(
        standalone.validate_typed(&RuntimeValue::Bool(true), semantic(1)),
        Err(AwbcRuntimeValueSnapshotError::ProgramOwnerRequired {
            expected: semantic(1)
        })
    );
    for owner in owners() {
        let value = owner
            .types()
            .try_record_value(semantic(1), vec![RuntimeValue::Bool(true)], limits)
            .unwrap();
        let snapshot =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
        let mut admission = RuntimeRetainedValueAdmission::for_program(owner, limits);
        admission.validate_typed(&value, semantic(1)).unwrap();
        admission
            .validate_typed_snapshot(&snapshot, semantic(1))
            .unwrap();
        let error = admission.validate_typed(&value, semantic(2)).unwrap_err();
        assert!(matches!(
            error,
            AwbcRuntimeValueSnapshotError::ProgramType(_)
        ));
        assert_eq!(
            admission.validate_snapshot(&AwbcRuntimeValueSnapshot::Unit),
            Err(error)
        );
    }
}

#[test]
fn scoped_retained_values_require_the_original_invocation_binding() {
    use crate::effect_row::{EffectFormula, EffectPredicate};
    use crate::plan::{RuntimeFunctionTypeContract, RuntimeTypeBinder, RuntimeTypeScope};
    use crate::program_types::RuntimeFunctionEffectInstantiation;
    let binder = RuntimeTypeBinder::new(0, 0, 1);
    let owner = RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::unit(),
            AwbcRuntimeType::new(
                semantic(31),
                AwbcRuntimeTypeShape::Function {
                    contract: RuntimeFunctionTypeContract::new(
                        binder,
                        EffectPredicate::unconstrained(),
                        EffectFormula::empty(),
                    ),
                    parameters: vec![],
                    result: AwbcTypeId(0),
                },
            ),
            AwbcRuntimeType::new(semantic(32), AwbcRuntimeTypeShape::String)
                .with_scope(RuntimeTypeScope::root().enter(binder).unwrap()),
        ],
        ..AwbcProgram::default()
    }));
    let binding: RuntimeFunctionEffectInstantiation =
        serde_json::from_value(serde_json::json!({"context": semantic(31), "effects": [[]]}))
            .unwrap();
    let limits = RuntimeSchemaLimits::engine_default();
    let value = RuntimeValue::String("scoped".into());
    let snapshot = AwbcRuntimeValueSnapshot::String("scoped".into());
    let mut missing = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
    assert!(matches!(
        missing.validate_typed_in_invocation(&value, semantic(32), None),
        Err(AwbcRuntimeValueSnapshotError::ProgramType(_))
    ));
    let mut admitted = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
    admitted
        .validate_typed_in_invocation(&value, semantic(32), Some(&binding))
        .unwrap();
    admitted
        .validate_typed_snapshot_in_invocation(&snapshot, semantic(32), Some(&binding))
        .unwrap();
    let invalid: RuntimeFunctionEffectInstantiation =
        serde_json::from_value(serde_json::json!({"context": semantic(99), "effects": [[]]}))
            .unwrap();
    let mut bounded = RuntimeRetainedValueAdmission::for_program(
        owner,
        RuntimeSchemaLimits {
            max_sequence_items: 0,
            ..limits
        },
    );
    let error = bounded
        .validate_typed_in_invocation(&value, semantic(32), Some(&invalid))
        .unwrap_err();
    let AwbcRuntimeValueSnapshotError::ProgramType(source) = error else {
        panic!("binding resource failure remains in the type phase: {error:?}");
    };
    let crate::program_types::RuntimeProgramTypeError::Resource(source) = *source else {
        panic!("resource failure precedes the invalid context lookup: {source:?}");
    };
    assert_eq!(
        *source,
        RuntimeSchemaError::BudgetExceeded {
            budget: "sequence_items"
        }
    );
}
