use super::*;
use crate::plan::RuntimePlanBuilder;
use crate::task::semantic::TaskSemanticMeter;

fn row(
    plan: &RuntimePlanInventory,
    owner: &RuntimeTaskPlanCoordinateOwner,
    function: RuntimeFunctionSiteId,
) -> RuntimeTaskPlan {
    let context = super::super::RuntimeBodySemanticContext::new(plan);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = context
        .producer_function(
            &mut meter,
            function,
            owner,
            &mut |_| Ok(owner.resolve(0).unwrap()),
            crate::plan::RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    RuntimeTaskPlan {
        producer_function: function,
        family: NeedProducerFamily::HostAdapterTask,
        class: TaskClass::Io,
        request_template: context
            .host_request_template(
                &mut meter,
                producer.endpoint(0).unwrap(),
                crate::plan::RuntimeTaskPlanSealLimits::default(),
            )
            .unwrap(),
        control_effect: RuntimeControlEffectContractId::for_index(0).unwrap(),
        binding: RuntimeTaskSemanticBinding::Ordinary,
    }
}

#[test]
fn task_image_moves_the_actual_inventory_and_keeps_expected_bytes_as_data() {
    let (plan, owner, function) = super::super::tests::producer_host_plan(true);
    let task = row(&plan, &owner, function);
    let original_site = std::ptr::from_ref(plan.function_sites().get(function).unwrap());
    let image = UnsealedRuntimePlanImage::new(
        plan.inventory,
        owner,
        Box::new([task]),
        Some(Box::new([ExpectedTaskPlanKey::new([0; 32])])),
    )
    .unwrap();
    assert_eq!(
        original_site,
        std::ptr::from_ref(image.inventory.function_sites().get(function).unwrap())
    );
    assert_eq!(image.expected_key(0).unwrap().bytes(), &[0; 32]);
    assert!(image.expected_key(1).is_none());
    assert_eq!(image.task_plans.len(), 1);
}

#[test]
fn task_image_coordinate_count_precedes_expected_key_count() {
    let (plan, owner, _) = super::super::tests::producer_host_plan(true);
    assert!(matches!(
        UnsealedRuntimePlanImage::new(
            plan.inventory,
            owner,
            Box::new([]),
            Some(Box::new([ExpectedTaskPlanKey::new([1; 32])]))
        ),
        Err(RuntimeTaskPlanImageError::CoordinateCount {
            rows: 0,
            coordinates: 1
        })
    ));
    let (plan, owner, function) = super::super::tests::producer_host_plan(true);
    let task = row(&plan, &owner, function);
    assert!(matches!(
        UnsealedRuntimePlanImage::new(plan.inventory, owner, Box::new([task]), Some(Box::new([]))),
        Err(RuntimeTaskPlanImageError::ExpectedKeyCount { rows: 1, keys: 0 })
    ));
    let builder = RuntimePlanBuilder::new();
    let owner = builder.task_coordinate_owner(0);
    let plan = builder.finish().unwrap();
    UnsealedRuntimePlanImage::new(plan.inventory, owner, Box::new([]), None).unwrap();
}

#[test]
fn task_preflight_counts_table_fourteen_before_any_reference_resolution() {
    let (plan, owner, function) = super::super::tests::producer_host_plan(true);
    let task = row(&plan, &owner, function);
    let image =
        UnsealedRuntimePlanImage::new(plan.inventory, owner, Box::new([task]), None).unwrap();
    let mut meter = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        image.preflight_rows(
            crate::plan::RuntimeTaskPlanSealLimits {
                max_task_plan_rows: 0,
                max_executable_rows: 0,
                ..crate::plan::RuntimeTaskPlanSealLimits::default()
            },
            &mut meter
        ),
        Err(RuntimeTaskPlanImageError::TaskRows {
            actual: 1,
            maximum: 0
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
    let mut meter = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        image.preflight_rows(
            crate::plan::RuntimeTaskPlanSealLimits {
                max_executable_rows: 3,
                ..crate::plan::RuntimeTaskPlanSealLimits::default()
            },
            &mut meter
        ),
        Err(RuntimeTaskPlanImageError::Body(
            super::super::RuntimeBodySemanticError::ExecutableRows {
                actual: 4,
                maximum: 3
            }
        ))
    ));
    let mut meter = TaskSemanticMeter::new(0, 0);
    image
        .preflight_rows(
            crate::plan::RuntimeTaskPlanSealLimits {
                max_executable_rows: 4,
                ..crate::plan::RuntimeTaskPlanSealLimits::default()
            },
            &mut meter,
        )
        .unwrap();
    // The fixture intentionally has no C row; count passes without resolving it.
    assert_eq!(meter.totals(), (0, 0));
}

#[test]
fn global_function_roles_precede_request_roles_and_semantic_type_errors() {
    let (plan, owner, function) = super::super::tests::producer_host_plan(true);
    let mut task = row(&plan, &owner, function);
    task.request_template = super::super::request::RuntimeTaskRequestTemplate::new(
        0,
        Box::new([super::super::request::RuntimeRequestArgument {
            role: super::super::request::RuntimeRequestArgumentRole::Positional,
            identity: None,
            ty: crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(
                std::num::NonZeroU32::new(99).unwrap(),
            ),
            source: super::super::request::RuntimeRequestValueSource::Literal,
            path: Box::new([]),
        }]),
        Box::new([]),
    );
    let image =
        UnsealedRuntimePlanImage::new(plan.inventory, owner, Box::new([task]), None).unwrap();
    let mut meter = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        image.preflight_roles(
            crate::plan::RuntimeTaskPlanSealLimits {
                max_function_roles: 0,
                max_request_roles: 0,
                ..crate::plan::RuntimeTaskPlanSealLimits::default()
            },
            &mut meter
        ),
        Err(RuntimeTaskPlanImageError::Body(
            super::super::RuntimeBodySemanticError::FunctionRoles {
                actual: 1,
                maximum: 0
            }
        ))
    ));
    assert!(meter.status().is_err());
    let mut meter = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        image.preflight_roles(
            crate::plan::RuntimeTaskPlanSealLimits {
                max_request_roles: 0,
                ..crate::plan::RuntimeTaskPlanSealLimits::default()
            },
            &mut meter
        ),
        Err(RuntimeTaskPlanImageError::Body(
            super::super::RuntimeBodySemanticError::RequestRoles {
                actual: 1,
                maximum: 0
            }
        ))
    ));
    let mut meter = TaskSemanticMeter::new(0, 0);
    image
        .preflight_roles(
            crate::plan::RuntimeTaskPlanSealLimits::default(),
            &mut meter,
        )
        .unwrap();
    assert_eq!(meter.totals(), (0, 0));
}

#[test]
fn common_preflight_children_precede_function_roles_without_hash_work() {
    let (plan, owner, function) = super::super::tests::producer_host_plan(true);
    let task = row(&plan, &owner, function);
    let image =
        UnsealedRuntimePlanImage::new(plan.inventory, owner, Box::new([task]), None).unwrap();
    let mut meter = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        image.preflight(
            crate::plan::RuntimeTaskPlanSealLimits {
                max_children_per_row: 0,
                max_function_roles: 0,
                ..crate::plan::RuntimeTaskPlanSealLimits::default()
            },
            &mut meter
        ),
        Err(RuntimeTaskPlanImageError::Children {
            table: 4,
            ordinal: 0,
            actual: 2,
            maximum: 0
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
}

#[test]
fn common_preflight_view_count_precedes_known_bytes_and_accepts_exact_empty_bound() {
    let builder = RuntimePlanBuilder::new();
    let owner = builder.task_coordinate_owner(0);
    let plan = builder.finish().unwrap();
    let image = UnsealedRuntimePlanImage::new(plan.inventory, owner, Box::new([]), None).unwrap();
    let known = b"arcweft.runtime-plan.executable-semantic.v1\0".len() as u64 + 2 + 15 * 5;
    let limits = crate::plan::RuntimeTaskPlanSealLimits {
        max_transcript_bytes: known,
        ..crate::plan::RuntimeTaskPlanSealLimits::default()
    };
    let mut meter = TaskSemanticMeter::new(0, known);
    image.preflight(limits, &mut meter).unwrap();
    assert_eq!(meter.totals(), (0, 0));
    let mut meter = TaskSemanticMeter::new(0, known - 1);
    assert!(matches!(
        image.preflight(limits, &mut meter),
        Err(RuntimeTaskPlanImageError::Encoding(
            crate::task::semantic::TaskSemanticEncodingError::TranscriptBytes
        ))
    ));
    let (plan, owner, function) = super::super::tests::producer_host_plan(true);
    let mut task = row(&plan, &owner, function);
    task.binding = RuntimeTaskSemanticBinding::View;
    let image =
        UnsealedRuntimePlanImage::new(plan.inventory, owner, Box::new([task]), None).unwrap();
    let mut meter = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        image.preflight(
            crate::plan::RuntimeTaskPlanSealLimits {
                max_view_bindings: 0,
                ..crate::plan::RuntimeTaskPlanSealLimits::default()
            },
            &mut meter
        ),
        Err(RuntimeTaskPlanImageError::ViewRows {
            actual: 1,
            maximum: 0
        })
    ));
    assert_eq!(meter.totals(), (0, 0));
}

#[test]
fn task_request_path_children_reject_before_unknown_type_completion() {
    use super::super::request::{
        RuntimeRequestArgument, RuntimeRequestArgumentRole, RuntimeRequestPathStep,
        RuntimeRequestValueSource,
    };
    let (plan, owner, function) = super::super::tests::producer_host_plan(true);
    let mut task = row(&plan, &owner, function);
    task.request_template = RuntimeTaskRequestTemplate::new(
        0,
        Box::new([RuntimeRequestArgument {
            role: RuntimeRequestArgumentRole::Positional,
            identity: None,
            ty: crate::runtime_id::RuntimePlanTypeId::from_accepted_ordinal(
                std::num::NonZeroU32::new(99).unwrap(),
            ),
            source: RuntimeRequestValueSource::Literal,
            path: Box::new([
                RuntimeRequestPathStep::Operand(0),
                RuntimeRequestPathStep::Tuple(1),
                RuntimeRequestPathStep::Tuple(2),
            ]),
        }]),
        Box::new([]),
    );
    let image =
        UnsealedRuntimePlanImage::new(plan.inventory, owner, Box::new([task]), None).unwrap();
    let mut meter = TaskSemanticMeter::new(0, 0);
    assert!(matches!(
        image.preflight(
            crate::plan::RuntimeTaskPlanSealLimits {
                max_children_per_row: 2,
                ..crate::plan::RuntimeTaskPlanSealLimits::default()
            },
            &mut meter
        ),
        Err(RuntimeTaskPlanImageError::Children {
            table: 14,
            ordinal: 0,
            actual: 3,
            maximum: 2
        })
    ));
}
