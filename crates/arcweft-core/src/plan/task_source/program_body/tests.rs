use super::*;

fn context() -> RuntimeTaskSourceReadContext {
    RuntimeTaskSourceReadContext::new(RuntimeTaskPlanSealLimits::default())
}

fn thread_coordinate(plan: &RuntimePlan) -> &RuntimeTaskPlanBuildCoordinate {
    plan.function_sites()
        .iter_with_ids()
        .find_map(|(_, function)| {
            function.body().executable()?.ops().iter().find_map(|op| {
                if let FlowOp::ProgramTask { producer, .. } = op {
                    Some(&producer.plan)
                } else {
                    None
                }
            })
        })
        .expect("the fixture owns an original Thread")
}

#[test]
fn compiler_program_body_borrows_the_original_operation_and_enclosing_function() {
    let owner = crate::plan::task_source::template_tests::thread();
    let plan = owner.original();
    let coordinate = thread_coordinate(plan);
    let source = plan.task_program_body(coordinate, &mut context()).unwrap();
    assert_eq!(source.task_plan(), plan.task_plan(coordinate).unwrap());
    let [
        _,
        FlowOp::ProgramTask {
            producer,
            captures,
            body,
            ..
        },
        _,
    ] = source.function().body().executable().unwrap().ops()
    else {
        panic!("the enclosing fixture function owns that actual Thread")
    };
    assert!(std::ptr::eq(source.producer(), producer));
    assert!(std::ptr::eq(source.captures(), captures.as_slice()));
    assert!(std::ptr::eq(source.body(), body.as_slice()));
    let other = crate::plan::task_source::template_tests::thread();
    let mut work = context();
    assert!(matches!(
        other.original().task_program_body(coordinate, &mut work),
        Err(RuntimeTaskSourceError::ForeignCoordinate)
    ));
    assert!(
        plan.task_program_body(coordinate, &mut work).is_err(),
        "a foreign source rejects the same shared allowance"
    );
}

#[test]
fn compiler_program_body_cannot_bypass_the_original_source_work_limit() {
    let owner = crate::plan::task_source::template_tests::thread();
    let plan = owner.original();
    let mut work = RuntimeTaskSourceReadContext::new(RuntimeTaskPlanSealLimits {
        max_semantic_work: 0,
        max_transcript_bytes: 0,
        ..RuntimeTaskPlanSealLimits::default()
    });
    assert!(
        plan.task_program_body(thread_coordinate(plan), &mut work)
            .is_err()
    );
    assert!(
        plan.task_program_body(thread_coordinate(plan), &mut work)
            .is_err()
    );
}

#[test]
fn compiler_program_body_rejects_a_genuine_host_endpoint_without_refreshing_work() {
    let host = crate::plan::construction::source::host_calls::tests::fixture(
        crate::step::RuntimeHostCallMode::Suspend,
        true,
        true,
        false,
    );
    let function = host.function_sites().iter().next().unwrap();
    let FlowOp::HostCall { target, .. } = &function.body().executable().unwrap().ops()[0] else {
        panic!("the fixture owns an actual Host endpoint")
    };
    let mut work = context();
    assert!(matches!(
        host.task_program_body(&target.producer.plan, &mut work),
        Err(RuntimeTaskSourceError::NotProgramBody)
    ));
    let owner = crate::plan::task_source::template_tests::thread();
    let plan = owner.original();
    assert!(
        plan.task_program_body(thread_coordinate(plan), &mut work)
            .is_err()
    );
}

#[test]
fn compiler_program_body_bounds_capture_roles_before_exposing_an_ingress_packet() {
    let owner = crate::plan::task_source::template_tests::thread_with_captures(5);
    let plan = owner.original();
    let coordinate = thread_coordinate(plan);
    let limits = RuntimeTaskPlanSealLimits {
        max_function_roles: 4,
        ..RuntimeTaskPlanSealLimits::default()
    };
    assert!(
        plan.task_source(
            coordinate,
            limits,
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000)
        )
        .is_ok(),
        "enclosing function endpoint count is distinct from child capture ingress"
    );
    let mut work = RuntimeTaskSourceReadContext::new(limits);
    assert!(matches!(
        plan.task_program_body(coordinate, &mut work),
        Err(RuntimeTaskSourceError::Function(
            crate::plan::RuntimeBodySemanticError::FunctionRoles {
                actual: 5,
                maximum: 4
            }
        ))
    ));
    assert!(plan.task_program_body(coordinate, &mut work).is_err());
    let mut exact = RuntimeTaskSourceReadContext::new(RuntimeTaskPlanSealLimits {
        max_function_roles: 5,
        ..limits
    });
    assert_eq!(
        plan.task_program_body(coordinate, &mut exact)
            .unwrap()
            .captures()
            .len(),
        5
    );
}
