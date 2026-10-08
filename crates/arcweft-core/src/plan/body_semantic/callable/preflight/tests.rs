use super::*;
use crate::plan::*;

#[test]
fn callable_child_preflight_shares_dag_suffix_and_repeated_roots() {
    let (plan, root) = crate::plan::body_semantic::tests::callable_graph_fixture(128, 3);
    let mut pass = RuntimeCallableChildPreflight::new(&plan.inventory);
    let mut widths = Vec::new();
    pass.state(root, &mut |count| {
        widths.push(count);
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(widths.len(), 127 * 11 + 5);
    assert_eq!(&widths[..5], &[0, 0, 0, 0, 3]);
    assert_eq!(pass.visited.len(), 129);
    pass.state(root, &mut |_| Err("repeat must not traverse"))
        .unwrap();
}

#[test]
fn callable_child_preflight_limits_precede_unknown_type_and_origin_cycle() {
    let (mut plan, root) = crate::plan::body_semantic::tests::callable_graph_fixture(2, 1);
    let mut rows = plan.callable_states().iter().cloned().collect::<Vec<_>>();
    let row = &mut rows[root.index()];
    row.origin = root;
    row.transition = RuntimeCallableTransition::Retain {
        state: root,
        values: Box::new([]),
    };
    row.function_type = crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(
        std::num::NonZeroU32::new(99).unwrap(),
    );
    row.position = RuntimeCallablePosition::WithinGroup {
        group: 0,
        bound: (0..3)
            .map(|parameter| RuntimeCallableParameterCoordinate {
                group: 0,
                parameter,
            })
            .collect(),
    };
    plan.inventory.callable_states = RuntimeCallableStateTable::from_admitted(rows);
    let mut pass = RuntimeCallableChildPreflight::new(&plan.inventory);
    let mut widths = Vec::new();
    assert_eq!(
        pass.state(root, &mut |count| {
            widths.push(count);
            if count > 2 { Err(count) } else { Ok(()) }
        }),
        Err(3)
    );
    assert_eq!(widths, [3]);
}

#[test]
fn callable_child_preflight_deep_graph_is_iterative_and_defers_missing_rows() {
    let (plan, root) = crate::plan::body_semantic::tests::callable_graph_fixture(20_000, 1);
    let mut pass = RuntimeCallableChildPreflight::new(&plan.inventory);
    pass.state(root, &mut |_| Ok::<(), ()>(())).unwrap();
    assert_eq!(pass.visited.len(), 20_001);
    let absent = RuntimeCallableStateId::for_index(30_000).unwrap();
    pass.state(absent, &mut |_| Err("missing owner must be structural"))
        .unwrap();
}

#[test]
fn specialization_child_preflight_precedes_type_resolution_and_reuses_states() {
    let (mut plan, root) = crate::plan::body_semantic::tests::callable_graph_fixture(1, 1);
    let ty = plan.callable_states().get(root).unwrap().function_type;
    plan.inventory.callable_specializations = vec![RuntimeCallableSpecializationDefinition {
        source_type: ty,
        target_type: ty,
        arguments: RuntimeFunctionSpecializationArguments {
            types: Box::new([ty; 3]),
            const_lengths: Box::new([RuntimeArrayLength::Constant(1); 4]),
            effects: Box::new([]),
        },
        states: Box::new([RuntimeCallableSpecializationState {
            source: root,
            target: root,
        }]),
    }]
    .into();
    let id = RuntimeCallableSpecializationId::from_zero_based(0).unwrap();
    let mut pass = RuntimeCallableChildPreflight::new(&plan.inventory);
    let mut widths = Vec::new();
    pass.specialization(id, &mut |count| {
        widths.push(count);
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(widths, [3, 4, 0, 1, 0, 0, 0, 0, 0]);
    pass.specialization(id, &mut |_| Err("repeat must not traverse"))
        .unwrap();
    let mut pass = RuntimeCallableChildPreflight::new(&plan.inventory);
    assert_eq!(
        pass.specialization(id, &mut |count| if count > 2 { Err(count) } else { Ok(()) }),
        Err(3)
    );
}

#[test]
fn callable_count_includes_default_capture_and_partial_metadata_in_field_order() {
    let (plan, root) = crate::plan::body_semantic::tests::callable_graph_fixture(1, 1);
    let mut row = plan.callable_states().get(root).unwrap().clone();
    let RuntimeCallableTransition::Invoke { function, .. } = row.transition else {
        panic!("actual Invoke fixture")
    };
    let coordinate = RuntimeCallableParameterCoordinate {
        group: 0,
        parameter: 0,
    };
    let input = RuntimeCallableInputSource::Attached;
    row.position = RuntimeCallablePosition::WithinGroup {
        group: 0,
        bound: Box::new([coordinate]),
    };
    row.retained = vec![
        RuntimeCallableRetainedInput {
            role: RuntimeCallableRetainedRole::Parameter(coordinate),
            ty: row.result
        };
        2
    ]
    .into();
    row.parameters = vec![
        RuntimeCallableParameterInput {
            coordinate,
            kind: RuntimeCallableParameterKind::Fixed,
            abi_ty: row.result,
            binding_ty: row.result
        };
        3
    ]
    .into();
    row.attached = RuntimeCallableAttachedContract::Defaulted {
        ty: row.result,
        default: RuntimeCallableDefault::Body {
            function,
            captures: Box::new([input; 4]),
        },
    };
    row.transition = RuntimeCallableTransition::Invoke {
        function,
        captures: Box::new([input; 5]),
        arguments: Box::new([input; 6]),
    };
    row.partials = Box::new([RuntimeCallablePartialTransition {
        state: root,
        parameters: Box::new([coordinate; 7]),
        values: Box::new([input; 8]),
    }]);
    let mut widths = Vec::new();
    row.try_visit_semantic_child_counts(&mut |count| {
        widths.push(count);
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(widths, [1, 2, 3, 4, 5, 6, 1, 7, 8]);
    row.transition = RuntimeCallableTransition::Retain {
        state: root,
        values: Box::new([input; 9]),
    };
    assert_eq!(
        row.try_visit_semantic_child_counts(&mut |count| if count > 8 {
            Err(count)
        } else {
            Ok(())
        }),
        Err(9)
    );
}
