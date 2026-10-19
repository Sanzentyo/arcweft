use super::*;
use crate::plan::RuntimePlanBuilder;
use crate::task::semantic::TaskSemanticEncodingError;
use crate::task::{NeedProducerFamily, TaskClass};

fn image(expected: Option<[u8; 32]>) -> UnsealedRuntimePlanImage {
    image_with_rows(1, expected.map(|key| vec![key]))
}

#[test]
fn consuming_seal_keeps_the_original_preparation_meter() {
    let mut image = image(None);
    let coordinate = image.coordinate_owner.resolve(0).unwrap();
    let mut meter = TaskSemanticMeter::new(3, 1_000_000);
    meter.charge_work(3).unwrap();
    image.preparation_meter = Some(meter);
    let error = image
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .err()
        .unwrap();
    assert!(
        matches!(
            error,
            RuntimeTaskPlanSealError::Control(
                crate::plan::RuntimeControlEffectContractError::WorkLimit
            ) | RuntimeTaskPlanSealError::Encoding(TaskSemanticEncodingError::SemanticWork)
                | RuntimeTaskPlanSealError::Body(RuntimeBodySemanticError::Encoding(
                    TaskSemanticEncodingError::SemanticWork
                ))
        ),
        "{error:?}"
    );
    assert!(coordinate.sealed_plan().is_none());
}

fn source_control(
    builder: &mut RuntimePlanBuilder,
) -> crate::plan::RuntimeControlEffectContractSeedId {
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
        .unwrap()
}

fn source_builder() -> (RuntimePlanBuilder, crate::plan::RuntimeTaskPlanSeed) {
    let mut control = None;
    let (builder, function) =
        crate::plan::body_semantic::tests::try_producer_request_builder_with_setup(
            true,
            None,
            |builder| {
                control = Some(source_control(builder));
                (vec![], Box::new([]))
            },
        )
        .unwrap();
    let seed = crate::plan::RuntimeTaskPlanSeed::new(
        function,
        NeedProducerFamily::HostAdapterTask,
        TaskClass::Io,
        crate::plan::RuntimeTaskRequestTemplateSeed::new(
            crate::plan::RuntimeTaskRequestSource::Operation(0),
            Box::new([]),
            Box::new([]),
        ),
        control.unwrap(),
        crate::plan::RuntimeTaskSemanticBinding::Ordinary,
    );
    (builder, seed)
}

#[test]
fn original_builder_seed_enters_the_same_image_and_consuming_seal() {
    let (mut builder, seed) = source_builder();
    let function = seed.producer_function.id();
    let coordinate = builder.reserve_task_plan().unwrap();
    builder.define_task_plan(&coordinate, seed).unwrap();
    let candidate = builder
        .prepare_task_image(RuntimeTaskPlanSealLimits::default())
        .unwrap();
    let sealed = candidate
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap();
    assert_eq!(sealed.task_plans.len(), 1);
    let task_source = sealed
        .task_plans
        .by_index(crate::plan::RuntimeTaskPlanIndex(0))
        .unwrap();
    let row = task_source.definition();
    let key = task_source.key();
    assert_eq!(row.family(), NeedProducerFamily::HostAdapterTask);
    assert_eq!(row.producer_function(), function);
    assert_eq!(
        row.request_template().source(),
        crate::plan::RuntimeTaskRequestSource::Operation(0)
    );
    assert!(std::ptr::eq(row, sealed.task_plans.by_digest(key).unwrap()));
}

#[test]
fn public_finish_owns_the_only_completed_table_and_executable_proof() {
    let (mut builder, seed) = source_builder();
    let function = seed.producer_function.id();
    let coordinate = builder.push_task_plan(seed).unwrap();
    assert!(coordinate.sealed_plan().is_none());
    let plan = builder.finish().unwrap();
    let task_source = coordinate.sealed_plan().unwrap();
    let row = task_source.definition();
    let key = task_source.key();
    assert_eq!(row.producer_function(), function);
    assert_eq!(plan.task_plans().len(), 1);
    assert!(std::ptr::eq(row, plan.task_plans().by_digest(key).unwrap()));
    let (mut same_source, seed) = source_builder();
    let other_coordinate = same_source.push_task_plan(seed).unwrap();
    let other = same_source.finish().unwrap();
    let task_source = other_coordinate.sealed_plan().unwrap();
    let other_row = task_source.definition();
    let other_key = task_source.key();
    assert_eq!(
        plan.executable_semantic_digest(),
        other.executable_semantic_digest()
    );
    assert_eq!(key, other_key);
    assert_ne!(coordinate, other_coordinate);
    assert!(!std::ptr::eq(row, other_row));
    assert!(plan.task_plan(&other_coordinate).is_none());
    assert!(other.task_plan(&coordinate).is_none());
    let original_source = plan.task_plan(&coordinate).unwrap();
    let foreign_source = other.task_plan(&other_coordinate).unwrap();
    assert!(original_source.belongs_to(&plan));
    assert!(!original_source.belongs_to(&other));
    assert!(foreign_source.belongs_to(&other));
    assert!(!foreign_source.belongs_to(&plan));
    assert_eq!(original_source.key(), foreign_source.key());
    assert_ne!(original_source, foreign_source);
    assert_eq!(coordinate.sealed_plan(), Some(original_source));
    let task_source = plan.task_plan(&coordinate).unwrap();
    let resolved = task_source.definition();
    let resolved_key = task_source.key();
    assert!(std::ptr::eq(row, resolved));
    assert_eq!(resolved_key, key);
}

#[test]
fn original_builder_projects_request_data_without_minting_f_q_or_task_keys() {
    let (mut builder, mut seed) = source_builder();
    let coordinate = builder.reserve_task_plan().unwrap();
    let projected = builder
        .task_request_template_for_source(
            &seed.producer_function,
            &coordinate,
            RuntimeTaskPlanSealLimits::default(),
        )
        .unwrap();
    assert_eq!(projected, seed.request_template);
    assert!(coordinate.sealed_plan().is_none());
    seed.request_template = projected;
    builder.define_task_plan(&coordinate, seed).unwrap();
    let plan = builder.finish().unwrap();
    assert_eq!(plan.task_plans().len(), 1);
    assert!(plan.task_plan(&coordinate).is_some());
}

#[test]
fn source_owned_reservation_uses_the_actual_function_and_request() {
    let (builder, seed) = source_builder();
    let coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    assert!(coordinate.sealed_plan().is_none());
    let plan = builder.finish().unwrap();
    let expected_request = seed
        .request_template
        .try_map_types(|semantic| plan.type_table().id_for_semantic(semantic).ok_or(()))
        .unwrap();
    let task_source = plan.task_plan(&coordinate).unwrap();
    let row = task_source.definition();
    let key = task_source.key();
    assert_eq!(row.producer_function(), seed.producer_function.id());
    assert_eq!(row.request_template(), &expected_request);
    assert_eq!(row.family(), NeedProducerFamily::HostAdapterTask);
    assert_eq!(
        coordinate
            .sealed_plan()
            .map(crate::plan::RuntimeTaskPlanRef::key),
        Some(key)
    );
}

#[test]
fn source_owned_reservation_requires_an_actual_source_occurrence() {
    let builder = RuntimePlanBuilder::new();
    let coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    assert!(
        matches!(builder.finish(), Err(crate::plan::RuntimePlanBuildError::TaskPlanSeal(error))
        if matches!(*error, RuntimeTaskPlanSealError::IncompleteDefinition { ordinal: 0 }))
    );
    assert!(coordinate.sealed_plan().is_none());
}

fn root_seed(
    seed: crate::plan::RuntimeTaskPlanSeed,
) -> crate::plan::RuntimeTaskPlanFunctionRootSeed {
    crate::plan::RuntimeTaskPlanFunctionRootSeed::new(
        seed.producer_function,
        seed.family,
        seed.class,
        seed.control_effect,
        seed.binding,
    )
}

#[test]
fn function_root_seed_projects_q_inside_the_consuming_seal() {
    let (mut builder, seed) = source_builder();
    let function = seed.producer_function.id();
    let coordinate = builder
        .push_task_plan_function_root(root_seed(seed))
        .unwrap();
    assert!(coordinate.sealed_plan().is_none());
    let plan = builder.finish().unwrap();
    let task_source = plan.task_plan(&coordinate).unwrap();
    let row = task_source.definition();
    let key = task_source.key();
    assert_eq!(row.producer_function(), function);
    assert_eq!(
        row.request_template().source(),
        crate::plan::RuntimeTaskRequestSource::FunctionRoot
    );
    assert!(row.request_template().arguments().is_empty());
    assert_eq!(
        coordinate
            .sealed_plan()
            .map(crate::plan::RuntimeTaskPlanRef::key),
        Some(key)
    );
}

#[test]
fn function_root_seed_rejects_an_equal_content_foreign_function() {
    let (mut builder, mut seed) = source_builder();
    let (_foreign, foreign_seed) = source_builder();
    seed.producer_function = foreign_seed.producer_function;
    let coordinate = builder
        .push_task_plan_function_root(root_seed(seed))
        .unwrap();
    assert_eq!(
        builder.finish().unwrap_err(),
        crate::plan::RuntimePlanBuildError::ForeignFunctionSiteSeed
    );
    assert!(coordinate.sealed_plan().is_none());
}

#[test]
fn function_root_seed_checks_actual_function_roles_before_projection() {
    let (mut builder, seed) = source_builder();
    let coordinate = builder
        .push_task_plan_function_root(root_seed(seed))
        .unwrap();
    assert!(
        matches!(builder.finish_with_seal_limits(RuntimeTaskPlanSealLimits {
        max_function_roles: 0, ..RuntimeTaskPlanSealLimits::default()
    }), Err(crate::plan::RuntimePlanBuildError::TaskPlanSeal(error))
        if matches!(*error, RuntimeTaskPlanSealError::Body(RuntimeBodySemanticError::FunctionRoles { maximum: 0, .. })))
    );
    assert!(coordinate.sealed_plan().is_none());
}

#[test]
fn original_builder_request_projection_rejects_foreign_function_and_coordinate_tokens() {
    let (mut builder, seed) = source_builder();
    let coordinate = builder.reserve_task_plan().unwrap();
    let (mut foreign, foreign_seed) = source_builder();
    let foreign_coordinate = foreign.reserve_task_plan().unwrap();
    assert!(matches!(builder.task_request_template_for_source(
        &foreign_seed.producer_function, &coordinate, RuntimeTaskPlanSealLimits::default(),
    ), Err(RuntimeTaskPlanSealError::Construction(source))
        if *source == crate::plan::RuntimePlanBuildError::ForeignFunctionSiteSeed));
    assert!(matches!(
        builder.task_request_template_for_source(
            &seed.producer_function,
            &foreign_coordinate,
            RuntimeTaskPlanSealLimits::default(),
        ),
        Err(RuntimeTaskPlanSealError::Body(
            RuntimeBodySemanticError::ForeignTaskCoordinate
        ))
    ));
    assert!(coordinate.sealed_plan().is_none());
    assert!(foreign_coordinate.sealed_plan().is_none());
    builder.define_task_plan(&coordinate, seed).unwrap();
    builder.finish().unwrap();
}

#[test]
fn original_builder_request_projection_bounds_work_before_source_materialization() {
    let (mut builder, seed) = source_builder();
    let coordinate = builder.reserve_task_plan().unwrap();
    let limits = RuntimeTaskPlanSealLimits {
        max_semantic_work: 0,
        ..Default::default()
    };
    assert!(matches!(
        builder.task_request_template_for_source(&seed.producer_function, &coordinate, limits),
        Err(RuntimeTaskPlanSealError::Body(
            RuntimeBodySemanticError::Encoding(TaskSemanticEncodingError::SemanticWork)
        ))
    ));
    assert!(coordinate.sealed_plan().is_none());
}

#[test]
fn source_construction_capabilities_share_one_registry_and_close_at_finish() {
    let (builder, seed) = source_builder();
    let source = builder.task_plan_construction();
    let alias = source.clone();
    let coordinate = source.reserve().unwrap();
    alias.define(&coordinate, seed).unwrap();
    let plan = builder.finish().unwrap();
    let task_source = coordinate.sealed_plan().unwrap();
    let row = task_source.definition();
    let key = task_source.key();
    assert!(std::ptr::eq(row, plan.task_plans().by_digest(key).unwrap()));
    assert!(matches!(
        alias.reserve(),
        Err(RuntimeTaskPlanSealError::ConstructionClosed)
    ));
    assert_eq!(coordinate.sealed_plan().unwrap().key(), key);
    assert_eq!(plan.task_plans().len(), 1);
}

#[test]
fn foreign_source_definition_poison_reaches_every_alias_and_the_original_builder() {
    let (builder, seed) = source_builder();
    let source = builder.task_plan_construction();
    let alias = source.clone();
    source.reserve().unwrap();
    let (mut foreign, _) = source_builder();
    let coordinate = foreign.reserve_task_plan().unwrap();
    assert!(matches!(
        source.define(&coordinate, seed),
        Err(RuntimeTaskPlanSealError::ForeignCoordinate)
    ));
    assert!(
        matches!(alias.reserve(), Err(RuntimeTaskPlanSealError::Construction(source))
        if *source == crate::plan::RuntimePlanBuildError::Poisoned)
    );
    assert!(matches!(
        builder.finish(),
        Err(crate::plan::RuntimePlanBuildError::Poisoned)
    ));
}

#[test]
fn failed_consuming_finish_closes_source_mutation_without_issuing_a_key() {
    let (builder, _) = source_builder();
    let source = builder.task_plan_construction();
    let coordinate = source.reserve().unwrap();
    assert!(
        matches!(builder.finish(), Err(crate::plan::RuntimePlanBuildError::TaskPlanSeal(source))
        if matches!(*source, RuntimeTaskPlanSealError::IncompleteDefinition { ordinal: 0 }))
    );
    assert!(matches!(
        source.reserve(),
        Err(RuntimeTaskPlanSealError::ConstructionClosed)
    ));
    assert!(coordinate.sealed_plan().is_none());
}

#[test]
fn source_construction_retains_the_existing_thread_transfer_contract() {
    let (builder, seed) = source_builder();
    let source = builder.task_plan_construction();
    let coordinate = std::thread::spawn(move || {
        let coordinate = source.reserve().unwrap();
        source.define(&coordinate, seed).unwrap();
        coordinate
    })
    .join()
    .unwrap();
    let plan = builder.finish().unwrap();
    let task_source = coordinate.sealed_plan().unwrap();
    let row = task_source.definition();
    let key = task_source.key();
    assert!(std::ptr::eq(row, plan.task_plans().by_digest(key).unwrap()));
}

#[test]
fn closed_source_mutation_does_not_poison_an_in_progress_semantic_seal() {
    let (builder, seed) = source_builder();
    let source = builder.task_plan_construction();
    let coordinate = source.reserve().unwrap();
    source.define(&coordinate, seed).unwrap();
    let image = builder
        .prepare_task_image(RuntimeTaskPlanSealLimits::default())
        .unwrap();
    assert!(matches!(
        source.reserve(),
        Err(RuntimeTaskPlanSealError::ConstructionClosed)
    ));
    assert!(matches!(
        source.reserve(),
        Err(RuntimeTaskPlanSealError::ConstructionClosed)
    ));
    let plan = image
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    let task_source = coordinate.sealed_plan().unwrap();
    let row = task_source.definition();
    let key = task_source.key();
    assert!(std::ptr::eq(row, plan.task_plans().by_digest(key).unwrap()));
}

#[test]
fn builder_seed_checks_original_function_and_control_namespaces() {
    for foreign_function in [true, false] {
        let (mut builder, mut seed) = source_builder();
        let (_, foreign) = source_builder();
        if foreign_function {
            seed.producer_function = foreign.producer_function;
        } else {
            seed.control_effect = foreign.control_effect;
        }
        builder.push_task_plan(seed).unwrap();
        let result = builder.prepare_task_image(RuntimeTaskPlanSealLimits::default());
        if foreign_function {
            assert!(matches!(
                result,
                Err(RuntimeTaskPlanSealError::Construction(error)) if matches!(error.as_ref(),
                    crate::plan::RuntimePlanBuildError::ForeignFunctionSiteSeed)
            ));
        } else {
            assert!(matches!(
                result,
                Err(RuntimeTaskPlanSealError::Construction(error)) if matches!(error.as_ref(),
                    crate::plan::RuntimePlanBuildError::ControlEffectContract(
                        crate::plan::RuntimeControlEffectContractError::ForeignSeed
                    )
                )
            ));
        }
    }
}

#[test]
fn builder_task_rows_enter_the_actual_executable_count_before_materialization() {
    let (mut builder, seed) = source_builder();
    builder.push_task_plan(seed).unwrap();
    assert!(matches!(
        builder.prepare_task_image(RuntimeTaskPlanSealLimits {
            max_executable_rows: 3,
            ..RuntimeTaskPlanSealLimits::default()
        }),
        Err(RuntimeTaskPlanSealError::Construction(error)) if matches!(error.as_ref(),
            crate::plan::RuntimePlanBuildError::ExecutableInventoryRowsLimit {
                actual: 4,
                maximum: 3
            }
        )
    ));
}

#[test]
fn reserved_task_definition_cannot_be_dropped_or_published_incomplete() {
    let (mut builder, _) = source_builder();
    builder.reserve_task_plan().unwrap();
    assert!(matches!(
        builder.prepare_task_image(RuntimeTaskPlanSealLimits::default()),
        Err(RuntimeTaskPlanSealError::IncompleteDefinition { ordinal: 0 })
    ));
    let (mut builder, _) = source_builder();
    builder.reserve_task_plan().unwrap();
    assert!(matches!(
        builder.finish(),
        Err(crate::plan::RuntimePlanBuildError::TaskPlanSeal(source))
            if matches!(*source, RuntimeTaskPlanSealError::IncompleteDefinition { ordinal: 0 })
    ));
}

#[test]
fn task_definition_rejects_foreign_coordinates_and_duplicate_assignment_stickily() {
    let (mut builder, seed) = source_builder();
    let (mut foreign, _) = source_builder();
    let coordinate = foreign.reserve_task_plan().unwrap();
    assert!(matches!(
        builder.define_task_plan(&coordinate, seed),
        Err(RuntimeTaskPlanSealError::ForeignCoordinate)
    ));
    assert!(matches!(
        builder.prepare_task_image(RuntimeTaskPlanSealLimits::default()),
        Err(RuntimeTaskPlanSealError::Construction(error)) if matches!(error.as_ref(),
            crate::plan::RuntimePlanBuildError::Poisoned)
    ));

    let (mut builder, seed) = source_builder();
    let coordinate = builder.push_task_plan(seed).unwrap();
    let (_, alternative) = source_builder();
    assert!(matches!(
        builder.define_task_plan(&coordinate, alternative),
        Err(RuntimeTaskPlanSealError::DuplicateDefinition { ordinal: 0 })
    ));
    assert!(matches!(
        builder.prepare_task_image(RuntimeTaskPlanSealLimits::default()),
        Err(RuntimeTaskPlanSealError::Construction(error)) if matches!(error.as_ref(),
            crate::plan::RuntimePlanBuildError::Poisoned)
    ));
}

#[test]
fn source_request_definitions_cannot_supply_roles_absent_from_the_original_endpoint() {
    let (mut builder, mut seed) = source_builder();
    seed.request_template = crate::plan::RuntimeTaskRequestTemplateSeed::new(
        crate::plan::RuntimeTaskRequestSource::Operation(0),
        Box::new([crate::plan::RuntimeRequestArgument {
            role: crate::plan::RuntimeRequestArgumentRole::Positional,
            identity: None,
            ty: crate::pattern::RuntimeSemanticTypeId::from_bytes([62; 32]),
            source: crate::plan::RuntimeRequestValueSource::Literal,
            path: Box::new([crate::plan::RuntimeRequestPathStep::Operand(0)]),
        }]),
        Box::new([]),
    );
    let coordinate = builder.push_task_plan(seed).unwrap();
    let candidate = builder
        .prepare_task_image(RuntimeTaskPlanSealLimits::default())
        .unwrap();
    assert!(matches!(
        candidate.seal(RuntimeTaskPlanSealLimits::default(), None),
        Err(RuntimeTaskPlanSealError::Body(
            RuntimeBodySemanticError::RequestTemplateMismatch {
                component: "argument count",
                ..
            }
        ))
    ));
    assert!(coordinate.sealed_plan().is_none());
}

#[test]
fn source_request_semantic_types_require_the_original_builder_type_authority() {
    let (mut builder, mut seed) = source_builder();
    let unknown = crate::pattern::RuntimeSemanticTypeId::from_bytes([191; 32]);
    seed.request_template = crate::plan::RuntimeTaskRequestTemplateSeed::new(
        crate::plan::RuntimeTaskRequestSource::Operation(0),
        Box::new([crate::plan::RuntimeRequestArgument {
            role: crate::plan::RuntimeRequestArgumentRole::Positional,
            identity: None,
            ty: unknown,
            source: crate::plan::RuntimeRequestValueSource::Literal,
            path: Box::new([crate::plan::RuntimeRequestPathStep::Operand(0)]),
        }]),
        Box::new([]),
    );
    builder.push_task_plan(seed).unwrap();
    assert!(
        matches!(builder.prepare_task_image(RuntimeTaskPlanSealLimits::default()),
        Err(RuntimeTaskPlanSealError::Construction(error)) if matches!(error.as_ref(),
            crate::plan::RuntimePlanBuildError::UnknownSeedType {
                context: "task request role", semantic_identity,
            } if *semantic_identity == unknown))
    );
}

pub(in crate::plan::body_semantic::task_image) fn image_with_rows(
    rows: u32,
    expected: Option<Vec<[u8; 32]>>,
) -> UnsealedRuntimePlanImage {
    let mut coordinate_owner = None;
    let (plan, _owner, function) =
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
            coordinate_owner = Some(builder.task_coordinate_owner(rows));
            (vec![], Box::new([]))
        });
    let owner = coordinate_owner.expect("the same builder supplies the complete task inventory");
    let rows = (0..rows)
        .map(|_| super::super::tests::row(&plan, &owner, function))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    UnsealedRuntimePlanImage::new_for_test(
        plan,
        owner,
        rows,
        expected.map(|keys| {
            keys.into_iter()
                .map(super::super::ExpectedTaskPlanKey::new)
                .collect::<Vec<_>>()
                .into_boxed_slice()
        }),
    )
    .unwrap()
}

fn complete(
    image: &UnsealedRuntimePlanImage,
    meter: &mut TaskSemanticMeter,
) -> Result<CompletedRuntimeTaskPlanSemantics, RuntimeTaskPlanSealError> {
    complete_with_authority(image, meter, None)
}

fn complete_with_authority(
    image: &UnsealedRuntimePlanImage,
    meter: &mut TaskSemanticMeter,
    authority: Option<&dyn ViewTaskPlanAuthority>,
) -> Result<CompletedRuntimeTaskPlanSemantics, RuntimeTaskPlanSealError> {
    image
        .prepare_semantics(RuntimeTaskPlanSealLimits::default(), meter)?
        .complete_tasks(authority)
}

struct CompletingViewAuthority;
impl ViewTaskPlanAuthority for CompletingViewAuthority {
    fn task_plan_semantic_digest(
        &self,
        request: crate::task::ViewTaskPlanDigestRequest<'_>,
    ) -> Result<crate::task::TaskPlanSemanticDigest, crate::task::ViewTaskPlanValidationError> {
        let base = request.base();
        assert_eq!(
            base.binding_marker(),
            crate::plan::RuntimeTaskSemanticBindingKind::View
        );
        assert_eq!(base.family(), NeedProducerFamily::ViewMatchSubscription);
        let mut hasher = blake3::Hasher::new();
        hasher.update(crate::task::TaskPlanSemanticDigest::DOMAIN);
        hasher.update(&[base.plan_owner_tag()]);
        hasher.update(base.executable_semantic_digest().as_bytes());
        hasher.update(base.producer_function_semantic_digest().as_bytes());
        hasher.update(&[
            base.family().semantic_tag(),
            base.task_class().semantic_tag(),
        ]);
        hasher.update(base.request_template_digest().as_bytes());
        hasher.update(base.control_effect_contract_digest().as_bytes());
        hasher.update(&[base.binding_marker().semantic_tag()]);
        hasher.update(&4u32.to_le_bytes());
        hasher.update(b"view");
        hasher.update(&[23; 32]);
        hasher.update(&[47; 32]);
        Ok(request.finish_authority_transcript(hasher))
    }
}

#[test]
fn view_completion_checks_current_capability_before_expected_assertions() {
    struct Replay(crate::task::TaskPlanSemanticDigest);
    impl ViewTaskPlanAuthority for Replay {
        fn task_plan_semantic_digest(
            &self,
            _request: crate::task::ViewTaskPlanDigestRequest<'_>,
        ) -> Result<crate::task::TaskPlanSemanticDigest, crate::task::ViewTaskPlanValidationError>
        {
            Ok(self.0)
        }
    }
    let mut candidate = image(None);
    candidate.task_plans[0].family = NeedProducerFamily::ViewMatchSubscription;
    candidate.task_plans[0].binding = RuntimeTaskSemanticBinding::View;
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let completed =
        complete_with_authority(&candidate, &mut meter, Some(&CompletingViewAuthority)).unwrap();
    let key = completed.keys().next().unwrap();
    // The assertion is independent of the executable and remains last in the
    // authority path, exactly as for ordinary rows.
    candidate.expected_task_plan_keys = Some(Box::new([super::super::ExpectedTaskPlanKey::new(
        *key.as_bytes(),
    )]));
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert_eq!(
        complete_with_authority(&candidate, &mut meter, Some(&CompletingViewAuthority))
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        &[key]
    );

    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        complete_with_authority(&candidate, &mut meter, Some(&Replay(key))),
        Err(RuntimeTaskPlanSealError::ViewAuthority {
            ordinal: 0,
            source: crate::task::ViewTaskPlanValidationError::InvalidCompletion
        })
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn view_authority_rejection_precedes_mismatching_expected_key_and_is_sticky() {
    struct Stale;
    impl ViewTaskPlanAuthority for Stale {
        fn task_plan_semantic_digest(
            &self,
            _request: crate::task::ViewTaskPlanDigestRequest<'_>,
        ) -> Result<crate::task::TaskPlanSemanticDigest, crate::task::ViewTaskPlanValidationError>
        {
            Err(crate::task::ViewTaskPlanValidationError::StaleAuthority)
        }
    }
    let mut candidate = image(Some([0; 32]));
    candidate.task_plans[0].family = NeedProducerFamily::ViewMatchSubscription;
    candidate.task_plans[0].binding = RuntimeTaskSemanticBinding::View;
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        complete_with_authority(&candidate, &mut meter, Some(&Stale)),
        Err(RuntimeTaskPlanSealError::ViewAuthority {
            ordinal: 0,
            source: crate::task::ViewTaskPlanValidationError::StaleAuthority
        })
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn consuming_seal_moves_original_definitions_into_the_only_completed_table() {
    let candidate = image(None);
    let executable = run(&candidate).0;
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let key = complete(&candidate, &mut meter)
        .unwrap()
        .keys()
        .next()
        .unwrap();
    let coordinate = candidate.coordinate_owner.resolve(0).unwrap();
    let function = candidate.task_plans[0].producer_function;
    assert!(coordinate.sealed_plan().is_none());
    let function_address =
        std::ptr::from_ref(candidate.inventory.function_sites().get(function).unwrap());
    let sealed = candidate
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap();
    assert_eq!(sealed.executable.as_bytes(), &executable);
    assert_eq!(
        std::ptr::from_ref(sealed.inventory.function_sites().get(function).unwrap()),
        function_address
    );
    assert_eq!(sealed.task_plans.len(), 1);
    let task_source = sealed
        .task_plans
        .by_index(crate::plan::RuntimeTaskPlanIndex(0))
        .unwrap();
    let row = task_source.definition();
    let indexed = task_source.key();
    assert_eq!(indexed, key);
    assert_eq!(row.producer_function, function);
    assert_eq!(row.family, NeedProducerFamily::HostAdapterTask);
    assert!(std::ptr::eq(row, sealed.task_plans.by_digest(key).unwrap()));
    let task_source = coordinate.sealed_plan().unwrap();
    let resolved = task_source.definition();
    let resolved_key = task_source.key();
    assert!(std::ptr::eq(row, resolved));
    assert_eq!(resolved_key, key);
    let row_address = std::ptr::from_ref(row);
    assert!(
        sealed
            .task_plans
            .by_index(crate::plan::RuntimeTaskPlanIndex(1))
            .is_none()
    );
    drop(sealed);
    let task_source = coordinate.sealed_plan().unwrap();
    let retained = task_source.definition();
    let retained_key = task_source.key();
    assert_eq!(std::ptr::from_ref(retained), row_address);
    assert_eq!(retained_key, key);
}

#[test]
fn consuming_seal_rejects_wrong_assertions_without_a_published_table() {
    let candidate = image(Some([0; 32]));
    let coordinate = candidate.coordinate_owner.resolve(0).unwrap();
    assert!(coordinate.sealed_plan().is_none());
    assert!(matches!(
        candidate.seal(RuntimeTaskPlanSealLimits::default(), None),
        Err(RuntimeTaskPlanSealError::ExpectedKeyMismatch { ordinal: 0 })
    ));
    assert!(coordinate.sealed_plan().is_none());
}

#[test]
fn failed_final_row_never_publishes_an_earlier_completed_coordinate() {
    let candidate = image_with_rows(2, None);
    let first = candidate.coordinate_owner.resolve(0).unwrap();
    let last = candidate.coordinate_owner.resolve(1).unwrap();
    assert!(matches!(
        candidate.seal(RuntimeTaskPlanSealLimits::default(), None),
        Err(RuntimeTaskPlanSealError::DuplicateKey { .. })
    ));
    assert!(first.sealed_plan().is_none());
    assert!(last.sealed_plan().is_none());
}

#[test]
fn completed_task_source_borrows_canonical_metadata_shared_by_distinct_rows() {
    let mut candidate = image_with_rows(2, None);
    candidate.task_plans[1].class = TaskClass::Cpu;
    let first = candidate.coordinate_owner.resolve(0).unwrap();
    let last = candidate.coordinate_owner.resolve(1).unwrap();
    let plan = candidate
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    let mut bank = TaskSemanticMeter::new(100_000, 0);
    let first_source = plan
        .task_source(&first, RuntimeTaskPlanSealLimits::default(), &mut bank)
        .unwrap();
    let last_source = plan
        .task_source(&last, RuntimeTaskPlanSealLimits::default(), &mut bank)
        .unwrap();
    assert_eq!(first_source.task_plan(), plan.task_plan(&first).unwrap());
    assert_eq!(last_source.task_plan(), plan.task_plan(&last).unwrap());
    assert_ne!(
        first_source.task_plan().key(),
        last_source.task_plan().key()
    );
    assert!(matches!(
        last_source.task_plan().definition().class(),
        TaskClass::Cpu
    ));
    assert_eq!(
        first_source.request_endpoint().request_source(),
        last_source.request_endpoint().request_source()
    );
    let Some(super::super::super::flow::RuntimeBodyTaskSource::Host(first_target)) =
        first_source.request_endpoint().task_source()
    else {
        panic!("the canonical Host metadata is required");
    };
    let Some(super::super::super::flow::RuntimeBodyTaskSource::Host(last_target)) =
        last_source.request_endpoint().task_source()
    else {
        panic!("the same canonical Host metadata is required");
    };
    assert!(std::ptr::eq(first_target, last_target));
    assert_eq!(last_target.producer.plan, first);
    assert_ne!(last_target.producer.plan, last);
    assert_eq!(bank.totals().1, 0);
}

#[test]
fn saved_key_assertions_resolve_only_through_the_actual_issued_table() {
    let candidate = image(None);
    let coordinate = candidate.coordinate_owner.resolve(0).unwrap();
    let sealed = candidate
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap();
    let task_source = sealed
        .task_plans
        .by_index(crate::plan::RuntimeTaskPlanIndex(coordinate.ordinal()))
        .unwrap();
    let row = task_source.definition();
    let key = task_source.key();
    let encoded =
        serde_json::to_vec(&crate::task::ExpectedTaskPlanKey::new(*key.as_bytes())).unwrap();
    let expected = serde_json::from_slice(&encoded).unwrap();
    let task_source = sealed.task_plans.by_expected_key(&expected).unwrap();
    let resolved = task_source.definition();
    let issued = task_source.key();
    assert!(std::ptr::eq(row, resolved));
    assert_eq!(issued, key);
    let mut changed = *key.as_bytes();
    changed[0] ^= 1;
    assert!(
        sealed
            .task_plans
            .by_expected_key(&crate::task::ExpectedTaskPlanKey::new(changed))
            .is_none()
    );
    // The assertion grammar has no zero sentinel. Membership remains a
    // separate check on the actual completed table.
    let zero = crate::task::ExpectedTaskPlanKey::new([0; 32]);
    assert_eq!(
        serde_json::from_slice::<crate::task::ExpectedTaskPlanKey>(
            &serde_json::to_vec(&zero).unwrap()
        )
        .unwrap(),
        zero
    );
}

#[test]
fn saved_producer_metadata_is_recovered_only_from_the_original_derivation_input() {
    let (mut builder, seed) = source_builder();
    let coordinate = builder.push_task_plan(seed).unwrap();
    let sealed = builder
        .prepare_task_image(RuntimeTaskPlanSealLimits::default())
        .unwrap()
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap();
    let key = sealed
        .task_plans
        .by_index(crate::plan::RuntimeTaskPlanIndex(coordinate.ordinal()))
        .unwrap()
        .key();
    let empty_arguments = crate::value::RuntimeValue::Tuple(vec![]);
    let arguments = crate::entry::schema::canonical_runtime_value_view_digest(
        empty_arguments.view(),
        crate::entry::RuntimeSchemaLimits::engine_default().platform_encoded_bytes(),
    )
    .unwrap();
    // These are the actual declared host/source fields of source_builder.
    let original = crate::task::NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        crate::task::NeedProducerContractDigest::from_bytes([1; 32]),
        key,
        crate::task::NeedProducerSiteDigest::from_bytes([3; 32]),
        crate::task::RuntimeTypeSemanticDigest::from_bytes([62; 32]),
        arguments,
    );
    let issued = crate::task::NeedProducerInstance::try_from(&original).unwrap();
    let bytes = serde_json::to_vec(&issued.plan_assertion()).unwrap();
    let decoded: crate::task::NeedProducerInstance<crate::task::ExpectedTaskPlanKey> =
        serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded.admit_original_spec(&original).unwrap(), issued);
    for (name, field) in [
        ("plan", crate::task::NeedProducerAssertionField::Plan),
        (
            "contract",
            crate::task::NeedProducerAssertionField::Contract,
        ),
        (
            "payload_type",
            crate::task::NeedProducerAssertionField::PayloadType,
        ),
        (
            "arguments",
            crate::task::NeedProducerAssertionField::Arguments,
        ),
        ("key", crate::task::NeedProducerAssertionField::Key),
    ] {
        let mut data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let byte = data[name][0].as_u64().unwrap() ^ 1;
        data[name][0] = serde_json::Value::from(byte);
        let decoded: crate::task::NeedProducerInstance<crate::task::ExpectedTaskPlanKey> =
            serde_json::from_value(data).unwrap();
        assert!(matches!(decoded.admit_original_spec(&original),
            Err(crate::task::NeedProducerAssertionError::Mismatch { field: actual }) if actual == field));
    }
}

#[test]
fn consuming_seal_completes_an_original_function_root_request_without_a_host_projection() {
    let mut candidate = image(None);
    let function = candidate.task_plans[0].producer_function;
    let context = RuntimeBodySemanticContext::new(&candidate.inventory);
    let limits = RuntimeTaskPlanSealLimits::default();
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let producer = context
        .producer_function(&mut meter, function, &candidate.coordinate_owner, limits)
        .unwrap();
    let root = producer
        .request_endpoint(crate::plan::RuntimeTaskRequestSource::FunctionRoot)
        .unwrap();
    candidate.task_plans[0].request_template = context
        .static_request_template(&mut meter, root, limits)
        .unwrap();
    let coordinate = candidate.coordinate_owner.resolve(0).unwrap();
    let sealed = candidate.seal(limits, None).unwrap();
    let task_source = sealed
        .task_plans
        .by_index(crate::plan::RuntimeTaskPlanIndex(coordinate.ordinal()))
        .unwrap();
    let row = task_source.definition();
    let key = task_source.key();
    assert_eq!(
        row.request_template().source(),
        crate::plan::RuntimeTaskRequestSource::FunctionRoot
    );
    assert!(std::ptr::eq(row, sealed.task_plans.by_digest(key).unwrap()));
    let plan = sealed.into_plan();
    let mut bank = TaskSemanticMeter::new(100_000, 0);
    let source = plan.task_source(&coordinate, limits, &mut bank).unwrap();
    assert_eq!(source.task_plan().key(), key);
    assert_eq!(
        source.request_endpoint().request_source(),
        crate::plan::RuntimeTaskRequestSource::FunctionRoot
    );
    assert!(
        source
            .request_endpoint()
            .function_inputs()
            .unwrap()
            .is_empty()
    );
    assert!(source.request_endpoint().task_source().is_none());
    assert_eq!(bank.totals().1, 0);
}

#[test]
fn view_hash_extension_uses_the_same_exact_work_and_byte_limits() {
    struct Tracking(std::cell::Cell<u32>);
    impl ViewTaskPlanAuthority for Tracking {
        fn task_plan_semantic_digest(
            &self,
            request: crate::task::ViewTaskPlanDigestRequest<'_>,
        ) -> Result<crate::task::TaskPlanSemanticDigest, crate::task::ViewTaskPlanValidationError>
        {
            self.0.set(self.0.get() + 1);
            CompletingViewAuthority.task_plan_semantic_digest(request)
        }
    }
    let mut candidate = image(None);
    candidate.task_plans[0].family = NeedProducerFamily::ViewMatchSubscription;
    candidate.task_plans[0].binding = RuntimeTaskSemanticBinding::View;
    let authority = Tracking(std::cell::Cell::new(0));
    let mut measured = TaskSemanticMeter::new(100_000, 1_000_000);
    complete_with_authority(&candidate, &mut measured, Some(&authority)).unwrap();
    let (work, bytes) = measured.totals();
    let mut exact = TaskSemanticMeter::new(work, bytes);
    complete_with_authority(&candidate, &mut exact, Some(&authority)).unwrap();
    assert_eq!(exact.totals(), (work, bytes));
    for (max_work, max_bytes, expected) in [
        (work - 1, bytes, TaskSemanticEncodingError::SemanticWork),
        (work, bytes - 1, TaskSemanticEncodingError::TranscriptBytes),
    ] {
        let before = authority.0.get();
        let mut limited = TaskSemanticMeter::new(max_work, max_bytes);
        assert!(
            matches!(complete_with_authority(&candidate, &mut limited, Some(&authority)),
            Err(RuntimeTaskPlanSealError::Encoding(error)) if error == expected)
        );
        assert_eq!(
            authority.0.get(),
            before + 1,
            "the upper extension exhausted the original meter"
        );
        assert_eq!(limited.status(), Err(expected));
    }
}

#[test]
fn view_completion_rejects_short_transcripts_and_substituted_issued_keys() {
    struct Short;
    impl ViewTaskPlanAuthority for Short {
        fn task_plan_semantic_digest(
            &self,
            request: crate::task::ViewTaskPlanDigestRequest<'_>,
        ) -> Result<crate::task::TaskPlanSemanticDigest, crate::task::ViewTaskPlanValidationError>
        {
            Ok(request.finish_authority_transcript(blake3::Hasher::new()))
        }
    }
    struct Substitute(crate::task::TaskPlanSemanticDigest);
    impl ViewTaskPlanAuthority for Substitute {
        fn task_plan_semantic_digest(
            &self,
            request: crate::task::ViewTaskPlanDigestRequest<'_>,
        ) -> Result<crate::task::TaskPlanSemanticDigest, crate::task::ViewTaskPlanValidationError>
        {
            let issued = CompletingViewAuthority.task_plan_semantic_digest(request)?;
            assert_ne!(issued, self.0);
            Ok(self.0)
        }
    }
    let ordinary = image(None);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let previous = complete(&ordinary, &mut meter)
        .unwrap()
        .keys()
        .next()
        .unwrap();
    let mut candidate = image(None);
    candidate.task_plans[0].family = NeedProducerFamily::ViewMatchSubscription;
    candidate.task_plans[0].binding = RuntimeTaskSemanticBinding::View;
    for authority in [&Short as &dyn ViewTaskPlanAuthority, &Substitute(previous)] {
        let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
        assert!(matches!(
            complete_with_authority(&candidate, &mut meter, Some(authority)),
            Err(RuntimeTaskPlanSealError::ViewAuthority {
                ordinal: 0,
                source: crate::task::ViewTaskPlanValidationError::InvalidCompletion
            })
        ));
        assert_eq!(
            meter.status(),
            Err(TaskSemanticEncodingError::OwnerRejected)
        );
    }
}

#[test]
fn core_completion_matches_one_expected_key_without_hashing_the_assertion() {
    let candidate = image(None);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let completed = complete(&candidate, &mut meter).unwrap();
    assert_eq!(completed.executable_bytes(), &run(&candidate).0);
    assert_eq!(completed.keys().len(), 1);
    let key = *completed.keys().next().unwrap().as_bytes();
    let accepted = image(Some(key));
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    let verified = complete(&accepted, &mut meter).unwrap();
    assert_eq!(
        verified.keys().collect::<Vec<_>>(),
        completed.keys().collect::<Vec<_>>()
    );
    let mut changed = key;
    changed[0] ^= 1;
    let rejected = image(Some(changed));
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        complete(&rejected, &mut meter),
        Err(RuntimeTaskPlanSealError::ExpectedKeyMismatch { ordinal: 0 })
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn core_completion_collects_duplicates_only_after_each_expected_key_check() {
    let candidate = image_with_rows(2, None);
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        complete(&candidate, &mut meter),
        Err(RuntimeTaskPlanSealError::DuplicateKey {
            first: 0,
            ordinal: 1
        })
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
    let rejected = image_with_rows(2, Some(vec![[0; 32]; 2]));
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        complete(&rejected, &mut meter),
        Err(RuntimeTaskPlanSealError::ExpectedKeyMismatch { ordinal: 0 })
    ));
}

#[test]
fn core_completion_never_issues_a_view_key_without_its_authority() {
    let mut candidate = image(None);
    candidate.task_plans[0].family = NeedProducerFamily::ViewMatchSubscription;
    candidate.task_plans[0].binding = RuntimeTaskSemanticBinding::View;
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        complete(&candidate, &mut meter),
        Err(RuntimeTaskPlanSealError::MissingViewAuthority { ordinal: 0 })
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

fn run(image: &UnsealedRuntimePlanImage) -> ([u8; 32], (u64, u64)) {
    let limits = RuntimeTaskPlanSealLimits::default();
    let mut meter = TaskSemanticMeter::new(limits.max_semantic_work, limits.max_transcript_bytes);
    let prepared = image.prepare_semantics(limits, &mut meter).unwrap();
    assert!(std::ptr::eq(prepared.image, image));
    (*prepared.executable_bytes(), prepared.meter.totals())
}

#[test]
fn empty_image_completes_all_fifteen_tables_and_the_version_one_envelope() {
    let builder = RuntimePlanBuilder::new();
    let owner = builder.task_coordinate_owner(0);
    let inventory = builder
        .prepare_inventory(RuntimeTaskPlanSealLimits::default(), 0)
        .unwrap();
    let image =
        UnsealedRuntimePlanImage::new_for_test(inventory, owner, Box::new([]), None).unwrap();
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
    let result = image.prepare_semantics(RuntimeTaskPlanSealLimits::default(), &mut meter);
    assert!(matches!(
        result,
        Err(RuntimeTaskPlanSealError::TaskDefinition {
            ordinal: 0,
            source: crate::plan::RuntimeTaskPlanValidationError::FamilyBindingMismatch {
                family: NeedProducerFamily::HostAdapterTask,
                binding: crate::plan::RuntimeTaskSemanticBindingKind::View,
            }
        })
    ));
    assert_eq!(
        meter.status(),
        Err(TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn complete_preparation_rejects_a_foreign_task_coordinate_without_issuing_evidence() {
    let mut image = image(None);
    let foreign = RuntimePlanBuilder::new().task_coordinate_owner(1);
    image.coordinate_owner = foreign;
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        image.prepare_semantics(RuntimeTaskPlanSealLimits::default(), &mut meter),
        Err(RuntimeTaskPlanSealError::Body(
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
    let mut prepared = image.prepare_semantics(limits, &mut meter).unwrap();
    let mut prefix = prepared.task_prefix(0).unwrap();
    prefix.tag(0);
    let digest = prefix.finish().unwrap();
    let total = prepared.meter.totals();
    drop(prepared);
    let mut exact = TaskSemanticMeter::new(total.0, total.1);
    let mut prepared = image.prepare_semantics(limits, &mut exact).unwrap();
    let mut prefix = prepared.task_prefix(0).unwrap();
    prefix.tag(0);
    assert_eq!(prefix.finish().unwrap(), digest);
    assert_eq!(prepared.meter.totals(), total);
    drop(prepared);
    let mut one_less = TaskSemanticMeter::new(total.0 - 1, total.1);
    let mut prepared = image.prepare_semantics(limits, &mut one_less).unwrap();
    let mut prefix = prepared.task_prefix(0).unwrap();
    prefix.tag(0);
    assert_eq!(
        prefix.finish(),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    assert!(matches!(
        prepared.task_prefix(0),
        Err(RuntimeTaskPlanSealError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    drop(prepared);
    let mut one_less = TaskSemanticMeter::new(total.0, total.1 - 1);
    let mut prepared = image.prepare_semantics(limits, &mut one_less).unwrap();
    let mut prefix = prepared.task_prefix(0).unwrap();
    prefix.tag(0);
    assert_eq!(
        prefix.finish(),
        Err(TaskSemanticEncodingError::TranscriptBytes)
    );
    assert!(matches!(
        prepared.task_prefix(0),
        Err(RuntimeTaskPlanSealError::Encoding(
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
            plan: crate::runtime_id::RuntimeLineTaskGroupId::from_zero_based(0).unwrap(),
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
                family.validate_runtime_task_binding(binding).is_ok(),
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
        plan: crate::runtime_id::RuntimeLineTaskGroupId::from_zero_based(0).unwrap(),
    };
    let mut meter = TaskSemanticMeter::new(100_000, 1_000_000);
    assert!(matches!(
        image.prepare_semantics(RuntimeTaskPlanSealLimits::default(), &mut meter),
        Err(RuntimeTaskPlanSealError::Body(
            RuntimeBodySemanticError::MissingRow {
                table: "line task groups",
                ordinal: 0
            }
        ))
    ));
}
