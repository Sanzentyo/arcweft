use super::*;
use crate::plan::RuntimeTaskPlanSealLimits;

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
