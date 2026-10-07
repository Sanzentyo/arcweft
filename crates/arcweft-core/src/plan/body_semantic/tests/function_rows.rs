use super::*;
use crate::plan::RuntimeTaskPlanSealLimits;

fn flow_row(
    plan: &RuntimePlan,
    owner: &crate::plan::RuntimeTaskPlanCoordinateOwner,
    function: crate::runtime_id::RuntimeFunctionSiteId,
) -> blake3::Hash {
    let context = RuntimeBodySemanticContext::new(plan);
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            owner,
            &mut |_| panic!("no task edge"),
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    producer
        .executable_flow_row_digest(&context, &mut meter, 0)
        .unwrap()
}

#[test]
fn executable_flow_row_reuses_root_and_ignores_function_arena_padding() {
    let role = crate::plan::RuntimeFunctionSemanticRole::Flow;
    let first = producer_expression_plan(false, true, role);
    let padded = producer_expression_plan(true, true, role);
    let changed = producer_expression_plan(false, false, role);
    assert_eq!(
        flow_row(&first.0, &first.1, first.2),
        flow_row(&padded.0, &padded.1, padded.2)
    );
    assert_ne!(
        flow_row(&first.0, &first.1, first.2),
        flow_row(&changed.0, &changed.1, changed.2)
    );
}

#[test]
fn executable_flow_row_commits_runtime_identity_and_excludes_public_label() {
    let (plan, owner, function) =
        producer_expression_plan(false, true, crate::plan::RuntimeFunctionSemanticRole::Flow);
    let expected = flow_row(&plan, &owner, function);
    let mut diagnostic = plan.clone();
    let identity = diagnostic.flows()[0].id.canonical_label();
    let renamed =
        crate::plan::FlowRuntimeId::from_runtime_contract(&identity, "diagnostic-only").unwrap();
    diagnostic.inventory.flows[0].id = renamed.clone();
    diagnostic.inventory.flow_schemas[0].flow = renamed;
    diagnostic.verify().unwrap();
    assert_eq!(expected, flow_row(&diagnostic, &owner, function));
    let mut semantic = plan.clone();
    let renamed = crate::plan::FlowRuntimeId::canonical("other_runtime_identity").unwrap();
    semantic.inventory.flows[0].id = renamed.clone();
    semantic.inventory.flow_schemas[0].flow = renamed;
    semantic.verify().unwrap();
    assert_ne!(expected, flow_row(&semantic, &owner, function));
}

#[test]
fn executable_flow_row_rejects_wrong_function_foreign_inventory_and_missing_row() {
    let (plan, owner, _) =
        producer_expression_plan(true, true, crate::plan::RuntimeFunctionSemanticRole::Flow);
    let context = RuntimeBodySemanticContext::new(&plan);
    let other = crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(NonZeroU32::MIN);
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    let producer = context
        .producer_function(
            &mut meter,
            other,
            &owner,
            &mut |_| panic!("no task edge"),
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let before = meter.totals();
    assert!(matches!(
        producer.executable_flow_row_digest(&context, &mut meter, 0),
        Err(RuntimeBodySemanticError::InvalidFlowProducer { ordinal: 0 })
    ));
    assert_eq!(meter.totals(), before);
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    let foreign = plan.clone();
    assert!(matches!(
        producer.executable_flow_row_digest(
            &RuntimeBodySemanticContext::new(&foreign),
            &mut meter,
            0
        ),
        Err(RuntimeBodySemanticError::ForeignFunctionTranscript)
    ));
    assert_eq!(meter.totals(), (0, 0));
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    assert!(matches!(
        producer.executable_flow_row_digest(&context, &mut meter, 99),
        Err(RuntimeBodySemanticError::MissingRow {
            table: "flows",
            ordinal: 99
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
}

#[test]
fn executable_flow_row_obeys_exact_shared_limits_and_inherited_poison() {
    let (plan, owner, function) =
        producer_expression_plan(false, true, crate::plan::RuntimeFunctionSemanticRole::Flow);
    let context = RuntimeBodySemanticContext::new(&plan);
    let run = |work, bytes| {
        let mut meter = TaskSemanticMeter::new(work, bytes);
        let result = context
            .producer_function(
                &mut meter,
                function,
                &owner,
                &mut |_| panic!("no task edge"),
                RuntimeTaskPlanSealLimits::default(),
            )
            .and_then(|producer| producer.executable_flow_row_digest(&context, &mut meter, 0));
        (result, meter.totals())
    };
    let (expected, (work, bytes)) = run(10_000, 100_000);
    assert_eq!(expected.unwrap(), run(work, bytes).0.unwrap());
    assert!(matches!(
        run(work - 1, bytes).0,
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert!(matches!(
        run(work, bytes - 1).0,
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::TranscriptBytes
        ))
    ));
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            &owner,
            &mut |_| panic!("no task edge"),
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let mut poisoned = TaskSemanticMeter::new(0, 100_000);
    poisoned.charge_work(1).unwrap_err();
    assert!(matches!(
        producer.executable_flow_row_digest(&context, &mut poisoned, 0),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(poisoned.totals(), (0, 0));
}

#[test]
fn executable_function_rows_cover_all_roles_and_commit_body_without_arena_padding() {
    let mut kinds = std::collections::BTreeSet::new();
    for role in crate::plan::RuntimeFunctionSemanticRole::ALL {
        let first = producer_expression_plan(false, true, *role);
        let padded = producer_expression_plan(true, true, *role);
        let changed = producer_expression_plan(false, false, *role);
        let hash = |(plan, owner, function): &(RuntimePlan, _, _)| {
            let mut meter = TaskSemanticMeter::new(10_000, 100_000);
            RuntimeBodySemanticContext::new(plan)
                .function_row_digest(
                    &mut meter,
                    *function,
                    owner,
                    &mut |_| panic!("no task edge"),
                    RuntimeTaskPlanSealLimits::default(),
                )
                .unwrap()
        };
        let original = hash(&first);
        assert_eq!(original, hash(&padded));
        assert_ne!(original, hash(&changed));
        assert!(kinds.insert(*original.as_bytes()));
    }
}

#[test]
fn executable_function_row_reuses_actual_body_and_endpoint_paths() {
    let (plan, owner, function) = producer_host_plan(true);
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let mut resolutions = 0;
    let producer = context
        .producer_function(
            &mut meter,
            function,
            &owner,
            &mut |_| {
                resolutions += 1;
                Ok(owner.resolve(0).unwrap())
            },
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    assert_eq!(resolutions, 1);
    let before = meter.totals();
    let first = producer
        .executable_row_digest(&context, &mut meter)
        .unwrap();
    let middle = meter.totals();
    let second = producer
        .executable_row_digest(&context, &mut meter)
        .unwrap();
    let after = meter.totals();
    assert_eq!(first, second);
    assert_ne!(first.as_bytes(), producer.digest().as_bytes());
    assert_eq!(resolutions, 1);
    assert_eq!(
        (middle.0 - before.0, middle.1 - before.1),
        (after.0 - middle.0, after.1 - middle.1)
    );
    assert!(middle.0 > before.0 && middle.1 > before.1);
}

#[test]
fn executable_function_row_commits_endpoint_branch_and_preflights_role_limit() {
    let first = producer_host_plan(true);
    let second = producer_host_plan(false);
    let hash = |(plan, owner, function): &(RuntimePlan, _, _)| {
        let mut meter = TaskSemanticMeter::new(10_000, 100_000);
        RuntimeBodySemanticContext::new(plan)
            .function_row_digest(
                &mut meter,
                *function,
                owner,
                &mut |_| Ok(owner.resolve(0).unwrap()),
                RuntimeTaskPlanSealLimits::default(),
            )
            .unwrap()
    };
    assert_ne!(hash(&first), hash(&second));
    let mut meter = TaskSemanticMeter::new(10_000, 100_000);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&first.0).function_row_digest(
            &mut meter,
            first.2,
            &first.1,
            &mut |_| panic!("quota precedes resolver"),
            RuntimeTaskPlanSealLimits {
                max_function_roles: 0,
                ..Default::default()
            },
        ),
        Err(RuntimeBodySemanticError::FunctionRoles {
            actual: 1,
            maximum: 0
        })
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn producer_and_executable_row_share_exact_pass_work_and_byte_limits() {
    let (plan, owner, function) = producer_host_plan(true);
    let context = RuntimeBodySemanticContext::new(&plan);
    let run = |work, bytes| {
        let mut meter = TaskSemanticMeter::new(work, bytes);
        let result = context.function_row_digest(
            &mut meter,
            function,
            &owner,
            &mut |_| Ok(owner.resolve(0).unwrap()),
            RuntimeTaskPlanSealLimits::default(),
        );
        (result, meter.totals())
    };
    let (expected, (work, bytes)) = run(100_000, 1_000_000);
    assert_eq!(run(work, bytes).0.unwrap(), expected.unwrap());
    assert!(matches!(
        run(work - 1, bytes).0,
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert!(matches!(
        run(work, bytes - 1).0,
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::TranscriptBytes
        ))
    ));
}

#[test]
fn executable_function_row_rejects_foreign_inventory_and_inherited_poison() {
    let (plan, owner, function) = producer_host_plan(true);
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            &owner,
            &mut |_| Ok(owner.resolve(0).unwrap()),
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let foreign = plan.clone();
    let mut rejected = TaskSemanticMeter::new(10_000, 100_000);
    assert!(matches!(
        producer.executable_row_digest(&RuntimeBodySemanticContext::new(&foreign), &mut rejected,),
        Err(RuntimeBodySemanticError::ForeignFunctionTranscript)
    ));
    assert_eq!(rejected.totals(), (0, 0));
    assert_eq!(
        rejected.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );

    let mut poisoned = TaskSemanticMeter::new(0, 100_000);
    poisoned.charge_work(1).unwrap_err();
    let before = poisoned.totals();
    assert!(matches!(
        producer.executable_row_digest(&context, &mut poisoned),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(poisoned.totals(), before);
}
