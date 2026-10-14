use super::*;

#[test]
fn retained_copies_require_original_nominal_type_and_preserve_both_program_owners() {
    let limits = RuntimeSchemaLimits::engine_default();
    for owner in owners() {
        let value = owner
            .types()
            .try_record_value(semantic(1), vec![RuntimeValue::Bool(true)], limits)
            .unwrap();
        assert_eq!(
            value
                .copy_unrestricted_typed_for_program_with_limits(&owner, semantic(1), limits,)
                .unwrap(),
            value
        );
        assert!(matches!(
            value.copy_unrestricted_typed_for_program_with_limits(&owner, semantic(2), limits,),
            Err(AwbcRuntimeValueSnapshotError::ProgramType(_))
        ));
        let snapshot =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
        assert_eq!(
            snapshot
                .copy_unrestricted_runtime_value_for_program_with_limits(&owner, limits)
                .unwrap(),
            value
        );
        assert_eq!(
            snapshot
                .copy_unrestricted_typed_runtime_value_for_program_with_limits(
                    &owner,
                    semantic(1),
                    limits,
                )
                .unwrap(),
            value
        );
        assert!(matches!(
            snapshot.copy_unrestricted_typed_runtime_value_for_program_with_limits(
                &owner,
                semantic(2),
                limits,
            ),
            Err(AwbcRuntimeValueSnapshotError::ProgramType(_))
        ));
    }
}

#[test]
fn retained_copy_rejects_later_foreign_callable_and_affine_values_before_copying() {
    let plan = Arc::new(crate::tests::function_application::returning_function_plan(
        crate::plan::RuntimeFunctionSiteBodyKind::Expression,
    ));
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let foreign = RuntimeProgramOwner::Plan(Arc::new(
        crate::tests::function_application::returning_function_plan(
            crate::plan::RuntimeFunctionSiteBodyKind::Expression,
        ),
    ));
    let value = RuntimeValue::Tuple(vec![
        RuntimeValue::String("first".into()),
        RuntimeValue::Callable(
            RuntimeCallableValue::try_new(
                foreign,
                crate::tests::function_application::returning_callable_state(&plan),
                [],
            )
            .unwrap(),
        ),
    ]);
    let limits = RuntimeSchemaLimits::engine_default();
    assert!(matches!(
        value.copy_unrestricted_for_program_with_limits(&owner, limits),
        Err(AwbcRuntimeValueSnapshotError::Message { .. })
    ));
    let value = RuntimeValue::Tuple(vec![
        RuntimeValue::String("first".into()),
        RuntimeValue::NeedHandle(crate::tests::reusable_need("need.retained-copy")),
    ]);
    assert_eq!(
        value.copy_unrestricted_for_program_with_limits(&owner, limits),
        Err(AwbcRuntimeValueSnapshotError::AffineValue)
    );
}

#[test]
fn retained_copy_enforces_exact_graph_limits_before_expected_type_resolution() {
    let owner = RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram::default()));
    let value = RuntimeValue::Tuple(vec![
        RuntimeValue::String("first".into()),
        RuntimeValue::Bool(true),
    ]);
    let exact = RuntimeSchemaLimits {
        max_nodes: 3,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert_eq!(
        value
            .copy_unrestricted_for_program_with_limits(&owner, exact)
            .unwrap(),
        value
    );
    let short = RuntimeSchemaLimits {
        max_nodes: 2,
        ..exact
    };
    assert!(matches!(
        value.copy_unrestricted_typed_for_program_with_limits(&owner, semantic(99), short),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            RuntimeSchemaError::BudgetExceeded { budget: "nodes" }
        ))
    ));
    let snapshot =
        AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
    assert!(matches!(
        snapshot.copy_unrestricted_runtime_value_for_program_with_limits(&owner, short),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            RuntimeSchemaError::BudgetExceeded { budget: "nodes" }
        ))
    ));
}
