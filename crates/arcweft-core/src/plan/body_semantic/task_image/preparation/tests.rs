use super::*;
use crate::plan::RuntimePlanBuilder;
use crate::task::semantic::TaskSemanticEncodingError;
use crate::task::{NeedProducerFamily, TaskClass};

fn image(expected: Option<[u8; 32]>) -> UnsealedRuntimePlanImage {
    let (plan, owner, function) =
        crate::plan::body_semantic::tests::producer_host_plan_with_setup(true, |builder| {
            builder
                .push_control_effect_contract(crate::plan::RuntimeControlEffectContractDefinition {
                    mode: crate::plan::RuntimeTaskControlMode::MustSuspend,
                    effects: Box::new([crate::plan::RuntimeControlEffectRow {
                        kind: crate::plan::RuntimeControlEffectKind::HostOperation,
                        identity: None,
                        inputs: Box::new([]),
                        output: Some(crate::pattern::RuntimeSemanticTypeId::from_bytes([62; 32])),
                        cardinality: crate::plan::RuntimeControlEffectCardinality::ExactlyOnce,
                        ordering: crate::plan::RuntimeControlEffectOrdering::SingleTerminal,
                        cancellation: crate::plan::RuntimeControlEffectCancellation::NotObserved,
                        terminal: crate::plan::RuntimeControlEffectTerminalBehavior::Value,
                    }]),
                    children: Box::new([]),
                })
                .unwrap();
            (vec![], Box::new([]))
        });
    let row = super::super::tests::row(&plan, &owner, function);
    UnsealedRuntimePlanImage::new(
        plan.inventory,
        owner,
        Box::new([row]),
        expected.map(|key| Box::new([super::super::ExpectedTaskPlanKey::new(key)]) as Box<[_]>),
    )
    .unwrap()
}

fn run(image: &UnsealedRuntimePlanImage) -> ([u8; 32], (u64, u64)) {
    let limits = RuntimeTaskPlanSealLimits::default();
    let mut meter = TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes);
    let prepared = image
        .prepare_semantics(limits, &mut meter, &mut |_| {
            Ok(image.coordinate_owner.resolve(0).unwrap())
        })
        .unwrap();
    assert!(std::ptr::eq(prepared.image, image));
    (*prepared.executable_bytes(), prepared.meter.totals())
}

#[test]
fn empty_image_completes_all_fifteen_tables_and_the_version_one_envelope() {
    let builder = RuntimePlanBuilder::new();
    let owner = builder.task_coordinate_owner(0);
    let plan = builder.finish().unwrap();
    let image = UnsealedRuntimePlanImage::new(plan.inventory, owner, Box::new([]), None).unwrap();
    let (actual, _) = run(&image);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.runtime-plan.executable-semantic.v1\0");
    hasher.update(&[0, 15]);
    for table in 0..15 {
        hasher.update(&[table]);
        hasher.update(&0u32.to_le_bytes());
    }
    assert_eq!(actual, *hasher.finalize().as_bytes());
}

#[test]
fn task_base_commits_family_class_and_binding_but_excludes_expected_codec_keys() {
    let original = image(Some([0; 32]));
    let (base, cost) = run(&original);
    let changed_key = image(Some([42; 32]));
    assert_eq!(run(&changed_key), (base, cost));
    assert_eq!(original.expected_key(0).unwrap().bytes(), &[0; 32]);
    assert_eq!(changed_key.expected_key(0).unwrap().bytes(), &[42; 32]);
    let mut changed_class = image(None);
    changed_class.task_plans[0].class = TaskClass::Cpu;
    assert_ne!(run(&changed_class).0, base);
    let mut changed_family = image(None);
    changed_family.task_plans[0].family = NeedProducerFamily::SelectedCallable;
    assert_ne!(run(&changed_family).0, base);
    let mut changed_binding = image(None);
    changed_binding.task_plans[0].family = NeedProducerFamily::AwaitManyBase;
    changed_binding.task_plans[0].binding = RuntimeTaskSemanticBinding::AwaitManyBase;
    assert_ne!(run(&changed_binding).0, base);
}

#[test]
fn family_binding_rejection_precedes_missing_producer_resolution_and_poison_is_sticky() {
    let mut image = image(None);
    image.task_plans[0].binding = RuntimeTaskSemanticBinding::View;
    image.task_plans[0].producer_function =
        crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(
            std::num::NonZeroU32::new(99).unwrap(),
        );
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let result = image.prepare_semantics(
        RuntimeTaskPlanSealLimits::default(),
        &mut meter,
        &mut |_| panic!("family is checked before body resolution"),
    );
    assert!(matches!(
        result,
        Err(RuntimeTaskPlanImageError::FamilyBinding {
            ordinal: 0,
            family: NeedProducerFamily::HostAdapterTask,
            binding: 1
        })
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn complete_preparation_rejects_a_foreign_task_coordinate_without_issuing_evidence() {
    let image = image(None);
    let foreign = RuntimePlanBuilder::new().task_coordinate_owner(1);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        image.prepare_semantics(
            RuntimeTaskPlanSealLimits::default(),
            &mut meter,
            &mut |_| Ok(foreign.resolve(0).unwrap())
        ),
        Err(RuntimeTaskPlanImageError::Body(
            RuntimeBodySemanticError::ForeignTaskCoordinate
        ))
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn task_prefix_consumes_the_original_preparation_meter_at_the_exact_work_limit() {
    let image = image(None);
    let limits = RuntimeTaskPlanSealLimits::default();
    let mut meter = TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes);
    let mut prepared = image
        .prepare_semantics(limits, &mut meter, &mut |_| {
            Ok(image.coordinate_owner.resolve(0).unwrap())
        })
        .unwrap();
    let mut prefix = prepared.task_prefix(0).unwrap();
    prefix.tag(0);
    let digest = prefix.finish().unwrap();
    let total = prepared.meter.totals();
    drop(prepared);
    let mut exact = TaskSemanticMeter::new(total.0, total.1);
    let mut prepared = image
        .prepare_semantics(limits, &mut exact, &mut |_| {
            Ok(image.coordinate_owner.resolve(0).unwrap())
        })
        .unwrap();
    let mut prefix = prepared.task_prefix(0).unwrap();
    prefix.tag(0);
    assert_eq!(prefix.finish().unwrap(), digest);
    assert_eq!(prepared.meter.totals(), total);
    drop(prepared);
    let mut one_less = TaskSemanticMeter::new(total.0 - 1, total.1);
    let mut prepared = image
        .prepare_semantics(limits, &mut one_less, &mut |_| {
            Ok(image.coordinate_owner.resolve(0).unwrap())
        })
        .unwrap();
    let mut prefix = prepared.task_prefix(0).unwrap();
    prefix.tag(0);
    assert_eq!(
        prefix.finish(),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    assert!(matches!(
        prepared.task_prefix(0),
        Err(RuntimeTaskPlanImageError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    drop(prepared);
    let mut one_less = TaskSemanticMeter::new(total.0, total.1 - 1);
    let mut prepared = image
        .prepare_semantics(limits, &mut one_less, &mut |_| {
            Ok(image.coordinate_owner.resolve(0).unwrap())
        })
        .unwrap();
    let mut prefix = prepared.task_prefix(0).unwrap();
    prefix.tag(0);
    assert_eq!(
        prefix.finish(),
        Err(TaskSemanticEncodingError::TranscriptBytes)
    );
    assert!(matches!(
        prepared.task_prefix(0),
        Err(RuntimeTaskPlanImageError::Encoding(
            TaskSemanticEncodingError::TranscriptBytes
        ))
    ));
}

#[test]
fn every_current_producer_family_has_one_explicit_structured_binding_domain() {
    use NeedProducerFamily as F;
    let families = [
        F::StructuredTaskPlan,
        F::AwbcTaskPlan,
        F::ViewMatchSubscription,
        F::AwaitManyBase,
        F::AwaitManyChild,
        F::Timeout,
        F::LineTask,
        F::HostAdapterTask,
        F::MakeNeedHandle,
        F::SelectedCallable,
    ];
    let bindings = [
        RuntimeTaskSemanticBinding::Ordinary,
        RuntimeTaskSemanticBinding::View,
        RuntimeTaskSemanticBinding::AwaitManyBase,
        RuntimeTaskSemanticBinding::AwaitManyChild,
        RuntimeTaskSemanticBinding::Timeout {
            contract: crate::task::NeedTimeoutContractDigest::from_bytes([5; 32]),
        },
        RuntimeTaskSemanticBinding::Line {
            group: crate::runtime_id::RuntimeLineTaskGroupId::from_zero_based(0).unwrap(),
        },
    ];
    for family in families {
        let expected = match family {
            F::StructuredTaskPlan
            | F::HostAdapterTask
            | F::MakeNeedHandle
            | F::SelectedCallable => Some(0),
            F::AwbcTaskPlan => None,
            F::ViewMatchSubscription => Some(1),
            F::AwaitManyBase => Some(2),
            F::AwaitManyChild => Some(3),
            F::Timeout => Some(4),
            F::LineTask => Some(5),
        };
        for binding in &bindings {
            assert_eq!(
                family.accepts_runtime_task_binding(binding),
                expected == Some(binding.semantic_tag()),
                "{family:?}/{}",
                binding.semantic_tag()
            );
        }
    }
}

#[test]
fn line_binding_resolves_the_original_group_instead_of_accepting_a_digest_placeholder() {
    let mut image = image(None);
    image.task_plans[0].family = NeedProducerFamily::LineTask;
    image.task_plans[0].binding = RuntimeTaskSemanticBinding::Line {
        group: crate::runtime_id::RuntimeLineTaskGroupId::from_zero_based(0).unwrap(),
    };
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        image.prepare_semantics(
            RuntimeTaskPlanSealLimits::default(),
            &mut meter,
            &mut |_| Ok(image.coordinate_owner.resolve(0).unwrap())
        ),
        Err(RuntimeTaskPlanImageError::Body(
            RuntimeBodySemanticError::MissingRow {
                table: "line task groups",
                ordinal: 0
            }
        ))
    ));
}
