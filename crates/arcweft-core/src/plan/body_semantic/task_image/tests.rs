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
