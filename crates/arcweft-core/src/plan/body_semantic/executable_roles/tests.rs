use super::super::entry_fixtures::controller_plan;
use super::*;
use crate::entry::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::*;
use crate::runtime_id::RuntimeFunctionSiteId;
use crate::task::semantic::TaskSemanticEncodingError;
use crate::value::RuntimeValue;

fn staged_callable(
    helper: bool,
    value: bool,
) -> (RuntimePlanInventory, RuntimeTaskPlanCoordinateOwner) {
    let mut builder = RuntimePlanBuilder::new();
    let boolean = RuntimeSemanticTypeId::from_bytes([61; 32]);
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                boolean,
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    let body = RuntimeExprSeed::new(
        boolean,
        RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)),
    );
    let definition = RuntimeFunctionDefinitionIdentity::from_accepted_identity([71; 32]);
    let code = if helper {
        RuntimeCallableExecutableSeedCode::PureHelper(
            builder
                .push_pure_helper_seed(RuntimePureHelperSeed {
                    definition,
                    name: "diagnostic".into(),
                    inputs: Box::new([]),
                    output_abi: RuntimePureOutputType::Bool,
                    body,
                    scalar_eval_supported: true,
                    origin: RuntimePureHelperOrigin::Annotated,
                })
                .unwrap(),
        )
    } else {
        RuntimeCallableExecutableSeedCode::FunctionSite(
            builder
                .push_function_site_seed(
                    definition,
                    RuntimeFunctionSemanticRole::Ordinary,
                    [],
                    body,
                )
                .unwrap(),
        )
    };
    builder
        .push_callable_executable_seed(RuntimeCallableExecutableSeed {
            callable: RuntimeCallableId::from_checked_digest([81; 32]),
            contract: CallableContractHash::from_bytes([82; 32]),
            code,
        })
        .unwrap();
    let owner = builder.task_coordinate_owner(0);
    (
        builder
            .prepare_inventory(RuntimeTaskPlanSealLimits::default())
            .unwrap(),
        owner,
    )
}

fn staged_row(
    inventory: &RuntimePlanInventory,
    owner: &RuntimeTaskPlanCoordinateOwner,
) -> blake3::Hash {
    let context = RuntimeBodySemanticContext::new(inventory);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    match inventory.callable_executables()[0].code {
        RuntimeCallableExecutableCode::PureHelper(id) => {
            let proof = inventory.pure_helpers()[id.0]
                .executable_semantic(&context, &mut meter)
                .unwrap();
            context
                .callable_executable_row_digest(
                    &mut meter,
                    0,
                    ExecutableCallableCodeSemantic::PureHelper(&proof),
                )
                .unwrap()
        }
        RuntimeCallableExecutableCode::FunctionSite(id) => {
            let proof = context
                .producer_function(
                    &mut meter,
                    id,
                    owner,
                    &mut |_| panic!("no task reference"),
                    RuntimeTaskPlanSealLimits::default(),
                )
                .unwrap();
            context
                .callable_executable_row_digest(
                    &mut meter,
                    0,
                    ExecutableCallableCodeSemantic::FunctionSite(&proof),
                )
                .unwrap()
        }
        RuntimeCallableExecutableCode::ControllerFlow(_) => panic!("staged noncontroller fixture"),
    }
}

#[test]
fn staged_noncontroller_rows_bind_actual_code_but_cannot_publish_without_entry_reachability() {
    for helper in [false, true] {
        let first = staged_callable(helper, true);
        let changed = staged_callable(helper, false);
        assert!(matches!(
            first.0.verify(),
            Err(RuntimePlanError::UnreachableCallableExecutable(_))
        ));
        assert!(matches!(
            changed.0.verify(),
            Err(RuntimePlanError::UnreachableCallableExecutable(_))
        ));
        assert_ne!(
            staged_row(&first.0, &first.1),
            staged_row(&changed.0, &changed.1)
        );
    }
    let (inventory, _) = staged_callable(true, true);
    let context = RuntimeBodySemanticContext::new(&inventory);
    let mut preparation = TaskSemanticMeter::new(100_000, 1_000_000);
    let proof = inventory.pure_helpers()[0]
        .executable_semantic(&context, &mut preparation)
        .unwrap();
    let foreign = inventory.clone();
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        RuntimeBodySemanticContext::new(&foreign).callable_executable_row_digest(
            &mut meter,
            0,
            ExecutableCallableCodeSemantic::PureHelper(&proof)
        ),
        Err(RuntimeBodySemanticError::InvalidCallableExecutableProducer { ordinal: 0 })
    ));
    assert_eq!(meter.totals(), (0, 0));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

fn controller_rows(
    plan: &RuntimePlan,
    owner: &RuntimeTaskPlanCoordinateOwner,
    function: RuntimeFunctionSiteId,
    work: u64,
    bytes: u64,
) -> (
    Result<(blake3::Hash, blake3::Hash), RuntimeBodySemanticError>,
    (u64, u64),
) {
    let context = RuntimeBodySemanticContext::new(plan);
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let result = context
        .producer_function(
            &mut meter,
            function,
            owner,
            &mut |_| panic!("no task reference"),
            RuntimeTaskPlanSealLimits::default(),
        )
        .and_then(|producer| {
            let callable = context.callable_executable_row_digest(
                &mut meter,
                0,
                ExecutableCallableCodeSemantic::ControllerFlow(&producer),
            )?;
            let flow = context.flow_executable_row_digest(&mut meter, 0, &producer)?;
            Ok((callable, flow))
        });
    (result, meter.totals())
}

#[test]
fn admitted_executable_roles_commit_body_and_parameter_passing_without_debug_or_arena_ids() {
    let first = controller_plan(false, true, RuntimeFunctionParameterPassing::Value, "input");
    let padded = controller_plan(
        true,
        true,
        RuntimeFunctionParameterPassing::Value,
        "renamed",
    );
    let digest = |fixture: &(
        RuntimePlan,
        RuntimeTaskPlanCoordinateOwner,
        RuntimeFunctionSiteId,
    )| {
        controller_rows(&fixture.0, &fixture.1, fixture.2, 100_000, 1_000_000)
            .0
            .unwrap()
    };
    let expected = digest(&first);
    assert_eq!(expected, digest(&padded));
    for changed in [
        controller_plan(
            false,
            false,
            RuntimeFunctionParameterPassing::Value,
            "input",
        ),
        controller_plan(
            false,
            true,
            RuntimeFunctionParameterPassing::Shared,
            "input",
        ),
    ] {
        let actual = digest(&changed);
        assert_ne!(expected.0, actual.0);
        assert_ne!(expected.1, actual.1);
    }
    assert_ne!(expected.0, expected.1);
}

#[test]
fn executable_role_contracts_are_semantic_on_the_actual_accepted_rows() {
    let (plan, owner, function) =
        controller_plan(false, true, RuntimeFunctionParameterPassing::Value, "input");
    let expected = controller_rows(&plan, &owner, function, 100_000, 1_000_000)
        .0
        .unwrap();
    let mut changed = plan.clone();
    let contract = CallableContractHash::from_bytes([91; 32]);
    changed.inventory.callable_executables[0].contract = contract;
    changed.inventory.flow_executables[0]
        .controller
        .as_mut()
        .unwrap()
        .contract = contract;
    let RuntimeEntryRoles::Agent(roles) = &mut changed.inventory.entries[0].roles else {
        panic!("agent")
    };
    roles.controller.contract = contract;
    changed.verify().unwrap();
    let actual = controller_rows(&changed, &owner, function, 100_000, 1_000_000)
        .0
        .unwrap();
    assert_ne!(expected.0, actual.0);
    assert_ne!(expected.1, actual.1);
    let mut changed = plan.clone();
    changed.inventory.flow_executables[0].contract = FlowContractHash::from_bytes([92; 32]);
    changed.verify().unwrap();
    let actual = controller_rows(&changed, &owner, function, 100_000, 1_000_000)
        .0
        .unwrap();
    assert_eq!(expected.0, actual.0);
    assert_ne!(expected.1, actual.1);
}

#[test]
fn executable_roles_use_one_meter_and_reject_foreign_missing_or_wrong_code_proofs() {
    let (plan, owner, function) =
        controller_plan(false, true, RuntimeFunctionParameterPassing::Value, "input");
    let (expected, (work, bytes)) = controller_rows(&plan, &owner, function, 100_000, 1_000_000);
    assert_eq!(
        expected.unwrap(),
        controller_rows(&plan, &owner, function, work, bytes)
            .0
            .unwrap()
    );
    for (work, bytes, error) in [
        (work - 1, bytes, TaskSemanticEncodingError::SemanticWork),
        (work, bytes - 1, TaskSemanticEncodingError::TranscriptBytes),
    ] {
        assert!(
            matches!(controller_rows(&plan, &owner, function, work, bytes).0,
            Err(RuntimeBodySemanticError::Encoding(actual)) if actual == error)
        );
    }
    let context = RuntimeBodySemanticContext::new(&plan);
    let mut preparation = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = context
        .producer_function(
            &mut preparation,
            function,
            &owner,
            &mut |_| panic!("no task reference"),
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    let foreign = plan.clone();
    let foreign_context = RuntimeBodySemanticContext::new(&foreign);
    for flow in [false, true] {
        let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
        let result = if flow {
            foreign_context.flow_executable_row_digest(&mut meter, 0, &producer)
        } else {
            foreign_context.callable_executable_row_digest(
                &mut meter,
                0,
                ExecutableCallableCodeSemantic::ControllerFlow(&producer),
            )
        };
        assert!(matches!(
            result,
            Err(
                RuntimeBodySemanticError::InvalidCallableExecutableProducer { .. }
                    | RuntimeBodySemanticError::InvalidFlowExecutableProducer { .. }
            )
        ));
        assert_eq!(meter.totals(), (0, 0));
        assert_eq!(
            meter.status(),
            Err(TaskSemanticEncodingError::OwnerRejected)
        );
    }
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        context.callable_executable_row_digest(
            &mut meter,
            0,
            ExecutableCallableCodeSemantic::FunctionSite(&producer)
        ),
        Err(RuntimeBodySemanticError::InvalidCallableExecutableProducer { ordinal: 0 })
    ));
    assert_eq!(meter.totals(), (0, 0));
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        context.flow_executable_row_digest(&mut meter, 1, &producer),
        Err(RuntimeBodySemanticError::MissingRow {
            table: "Flow executables",
            ordinal: 1
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
    let mut meter = TaskSemanticMeter::new(0, 1_000_000);
    meter.charge_work(1).unwrap_err();
    assert!(matches!(
        foreign_context.flow_executable_row_digest(&mut meter, 99, &producer),
        Err(RuntimeBodySemanticError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(meter.totals(), (0, 0));
}
