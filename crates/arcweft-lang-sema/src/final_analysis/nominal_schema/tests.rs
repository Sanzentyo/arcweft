//! Focused construction tests for the C2.2a nominal projection context.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, Ordering},
};

use arcweft_data::TypeShape;

use super::{
    NominalProjectionLimitKind, NominalSchemaExpander, NominalSchemaPath,
    NominalSchemaProjectionError, ProjectionBudget, RuntimeNominalProjectionContext,
    RuntimeNominalProjectionRequest, RuntimeNominalProjectionRequestInventory,
};
use crate::{
    final_analysis::{
        CheckedProjectNominal, FinalSemanticAnalysis, FinalSemanticAnalysisControl,
        tests::{Fixture, analyze, fixture, project_nominal_expression_type},
    },
    nominal::{
        NominalAggregationLimitKind, NominalAggregationLimits, NominalResolutionLimitKind,
        NominalResolutionLimits,
    },
    types::{
        DetachedGenericOwnerId, GenericParameterOwnerId, GenericTypeParameterId,
        SemanticTypeDigest, TypeKind,
    },
};

fn projection_fixture() -> Fixture {
    fixture(
        concat!(
            "struct First { value: i64 }\n",
            "struct Second { value: bool }\n",
            "struct Nested { value: Option<i64> }\n",
            "struct Pair<T, U> { first: T, second: U }\n",
            "struct Single<T> { value: T }\n",
            "struct Sibling { first: Single<i64>, second: Single<bool> }\n",
            "struct Node { next: Option<Node> }\n",
            "enum ChoiceValue {\n",
            "    Count i64,\n",
            "    Flag bool,\n",
            "}\n",
            "fn first(value: First) -> First { value }\n",
            "fn second(value: Second) -> Second { value }\n",
            "fn nested(value: Nested) -> Nested { value }\n",
            "fn pair(value: Pair<i64, bool>) -> Pair<i64, bool> { value }\n",
            "fn sibling(value: Sibling) -> Sibling { value }\n",
            "fn node(value: Node) -> Node { value }\n",
            "fn choice_value(value: ChoiceValue) -> ChoiceValue { value }\n",
        ),
        None,
    )
}

#[test]
fn generic_argument_limit_is_per_application_across_siblings() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let sibling = checked(&fixture, &report, "Sibling");
    let cancellation = AtomicBool::new(false);
    let mut context = context(
        &fixture,
        &report,
        &cancellation,
        root_limits(8, 4, 1, 10),
        aggregate_limits(1),
    );

    context
        .project_checked(&sibling)
        .expect("two sibling applications may each exactly consume the per-application limit");
}

fn checked(fixture: &Fixture, report: &FinalSemanticAnalysis, name: &str) -> CheckedProjectNominal {
    let TypeKind::ProjectNominal(nominal) = project_nominal_expression_type(report, name) else {
        panic!("project nominal helper returned a non-project type")
    };
    let declaration = fixture
        .symbols
        .nominal(nominal.declaration())
        .expect("project nominal declaration");
    CheckedProjectNominal::new(
        nominal.declaration().clone(),
        declaration.owner(),
        TypeKind::ProjectNominal(nominal.clone()).semantic_identity_digest(),
        nominal.arguments().to_vec(),
    )
}

fn root_limits(
    nodes: u64,
    depth: u16,
    generic_arguments: u16,
    work: u64,
) -> NominalResolutionLimits {
    NominalResolutionLimits::try_new(nodes, depth, generic_arguments, 1, 1, 1, 1, work)
        .expect("valid focused projection limits")
}

fn aggregate_limits(work: u64) -> NominalAggregationLimits {
    let diagnostics = u16::try_from(work).unwrap_or(u16::MAX).max(1);
    NominalAggregationLimits::try_new(1, diagnostics, work).expect("valid focused aggregate limits")
}

fn context<'a>(
    fixture: &'a Fixture,
    report: &'a FinalSemanticAnalysis,
    cancellation: &'a AtomicBool,
    root: NominalResolutionLimits,
    aggregate: NominalAggregationLimits,
) -> RuntimeNominalProjectionContext<'a> {
    RuntimeNominalProjectionContext::new(
        fixture.symbols.as_ref(),
        report.accepted_types(),
        root,
        aggregate,
        FinalSemanticAnalysisControl::new(cancellation),
    )
}

#[test]
fn fresh_root_budget_accepts_two_independent_exact_limit_projections() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let first = checked(&fixture, &report, "First");
    let second = checked(&fixture, &report, "Second");
    let cancellation = AtomicBool::new(false);
    let mut context = context(
        &fixture,
        &report,
        &cancellation,
        root_limits(2, 2, 1, 2),
        aggregate_limits(2),
    );

    context
        .project_checked(&first)
        .expect("first root exactly consumes its independent budget");
    context
        .project_checked(&second)
        .expect("second root receives a fresh independent budget");

    assert_eq!(context.aggregate_work(), 2);
    assert_eq!(context.accepted_len(), 2);
}

#[test]
fn aggregate_budget_charges_cache_hits_and_rejects_one_over() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let first = checked(&fixture, &report, "First");
    let cancellation = AtomicBool::new(false);
    let mut context = context(
        &fixture,
        &report,
        &cancellation,
        root_limits(2, 2, 1, 2),
        aggregate_limits(2),
    );

    let initial = context
        .project_checked(&first)
        .expect("cache miss at the exact aggregate limit")
        .clone();
    let cached = context
        .project_checked(&first)
        .expect("cache hit at the exact aggregate limit")
        .clone();
    assert_eq!(initial, cached);
    assert_eq!(context.accepted_len(), 1);
    assert_eq!(context.aggregate_work(), 2);
    assert_eq!(
        context.project_checked(&first),
        Err(NominalSchemaProjectionError::LimitExceeded {
            kind: NominalProjectionLimitKind::Project(NominalAggregationLimitKind::WorkPerProject,),
            observed: 3,
            maximum: 2,
        })
    );
}

#[test]
fn root_node_depth_generic_and_work_limits_are_exact() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let first = checked(&fixture, &report, "First");
    let nested = checked(&fixture, &report, "Nested");
    let pair = checked(&fixture, &report, "Pair");
    let cancellation = AtomicBool::new(false);

    for (label, nominal, exact, one_over_kind, observed, maximum) in [
        (
            "nodes",
            &first,
            root_limits(2, 2, 1, 2),
            NominalResolutionLimitKind::TypeNodesPerReference,
            2,
            1,
        ),
        (
            "depth",
            &nested,
            root_limits(3, 3, 1, 3),
            NominalResolutionLimitKind::RecursiveTypeDepth,
            3,
            2,
        ),
        (
            "generic arguments",
            &pair,
            root_limits(5, 3, 2, 7),
            NominalResolutionLimitKind::GenericArgumentsPerApplication,
            2,
            1,
        ),
        (
            "work",
            &nested,
            root_limits(3, 3, 1, 3),
            NominalResolutionLimitKind::WorkPerReference,
            3,
            2,
        ),
    ] {
        let mut exact_context =
            context(&fixture, &report, &cancellation, exact, aggregate_limits(1));
        exact_context
            .project_checked(nominal)
            .unwrap_or_else(|error| panic!("{label} exact limit failed: {error}"));

        let failing = match one_over_kind {
            NominalResolutionLimitKind::TypeNodesPerReference => root_limits(
                maximum,
                exact.recursive_type_depth(),
                1,
                exact.work_per_reference(),
            ),
            NominalResolutionLimitKind::RecursiveTypeDepth => root_limits(
                exact.type_nodes_per_reference(),
                u16::try_from(maximum).expect("focused depth limit fits u16"),
                1,
                exact.work_per_reference(),
            ),
            NominalResolutionLimitKind::GenericArgumentsPerApplication => root_limits(
                exact.type_nodes_per_reference(),
                exact.recursive_type_depth(),
                u16::try_from(maximum).expect("focused generic limit fits u16"),
                exact.work_per_reference(),
            ),
            NominalResolutionLimitKind::WorkPerReference => root_limits(
                exact.type_nodes_per_reference(),
                exact.recursive_type_depth(),
                exact.generic_arguments_per_application(),
                maximum,
            ),
            other => panic!("unexpected focused root limit: {other:?}"),
        };
        let mut failing_context = context(
            &fixture,
            &report,
            &cancellation,
            failing,
            aggregate_limits(1),
        );
        assert_eq!(
            failing_context.project_checked(nominal),
            Err(NominalSchemaProjectionError::LimitExceeded {
                kind: NominalProjectionLimitKind::Root(one_over_kind),
                observed,
                maximum,
            }),
            "{label} one-over must fail before descent",
        );
    }
}

#[test]
fn cancellation_and_checked_overflow_precede_cache_or_descent() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let first = checked(&fixture, &report, "First");
    let cancellation = AtomicBool::new(true);
    let mut cancelled = context(
        &fixture,
        &report,
        &cancellation,
        root_limits(2, 2, 1, 2),
        aggregate_limits(1),
    );
    assert_eq!(
        cancelled.project_checked(&first),
        Err(NominalSchemaProjectionError::Cancelled)
    );
    assert_eq!(cancelled.aggregate_work(), 0);
    assert_eq!(cancelled.accepted_len(), 0);

    let mut cancelled_budget = ProjectionBudget::new(root_limits(2, 2, 1, 2));
    assert_eq!(
        cancelled_budget.enter_node(FinalSemanticAnalysisControl::new(&cancellation)),
        Err(NominalSchemaProjectionError::Cancelled)
    );
    assert_eq!(cancelled_budget.nodes, 0);
    assert_eq!(cancelled_budget.work, 0);

    cancellation.store(false, Ordering::Release);
    let mut overflow = context(
        &fixture,
        &report,
        &cancellation,
        root_limits(2, 2, 1, 2),
        aggregate_limits(1),
    );
    overflow.aggregate_work = u64::MAX;
    assert_eq!(
        overflow.project_checked(&first),
        Err(NominalSchemaProjectionError::ArithmeticOverflow)
    );

    let mut budget = ProjectionBudget::new(root_limits(2, 2, 1, 2));
    budget.nodes = u64::MAX;
    assert_eq!(
        budget.enter_node(FinalSemanticAnalysisControl::new(&cancellation)),
        Err(NominalSchemaProjectionError::ArithmeticOverflow)
    );
}

#[test]
fn generic_substitution_cycle_is_typed_and_leaves_no_cache_row() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let cancellation = AtomicBool::new(false);
    let control = FinalSemanticAnalysisControl::new(&cancellation);
    let expander =
        NominalSchemaExpander::new(fixture.symbols.as_ref(), report.accepted_types(), control);
    let parameter = GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(7)),
        0,
    );
    let substitutions =
        BTreeMap::from([(parameter.clone(), TypeKind::GenericParam(parameter.clone()))]);
    let mut budget = ProjectionBudget::new(NominalResolutionLimits::PRODUCTION);

    assert_eq!(
        expander.type_shape(
            &TypeKind::GenericParam(parameter.clone()),
            &substitutions,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut budget,
        ),
        Err(NominalSchemaProjectionError::CyclicGenericSubstitution {
            path: NominalSchemaPath::default(),
            parameter,
        })
    );
}

#[test]
fn checked_request_generation_owner_arity_and_identity_precedence_is_typed() {
    let primary_fixture = projection_fixture();
    let report = analyze(&primary_fixture).expect("projection fixture final analysis");
    let first = checked(&primary_fixture, &report, "First");
    let second = checked(&primary_fixture, &report, "Second");
    let pair = checked(&primary_fixture, &report, "Pair");
    let cancellation = AtomicBool::new(false);
    let mut context = context(
        &primary_fixture,
        &report,
        &cancellation,
        NominalResolutionLimits::PRODUCTION,
        aggregate_limits(4),
    );

    let wrong_owner = CheckedProjectNominal::new(
        first.declaration().clone(),
        second.owner(),
        first.identity(),
        first.arguments().to_vec(),
    );
    assert!(matches!(
        context.project_checked(&wrong_owner),
        Err(NominalSchemaProjectionError::OwnerMismatch { .. })
    ));

    let wrong_arity = CheckedProjectNominal::new(
        pair.declaration().clone(),
        pair.owner(),
        pair.identity(),
        [],
    );
    assert!(matches!(
        context.project_checked(&wrong_arity),
        Err(NominalSchemaProjectionError::WrongArity {
            expected: 2,
            actual: 0,
            ..
        })
    ));

    let wrong_identity = CheckedProjectNominal::new(
        first.declaration().clone(),
        first.owner(),
        SemanticTypeDigest::from_bytes([0x5A; 32]),
        first.arguments().to_vec(),
    );
    assert!(matches!(
        context.project_checked(&wrong_identity),
        Err(NominalSchemaProjectionError::IdentityMismatch { .. })
    ));

    let foreign_fixture = fixture(
        "struct Foreign { value: i64 }\nfn foreign(value: Foreign) -> Foreign { value }\n",
        None,
    );
    let foreign_report = analyze(&foreign_fixture).expect("foreign projection analysis");
    let foreign = checked(&foreign_fixture, &foreign_report, "Foreign");
    assert_eq!(
        context.project_checked(&foreign),
        Err(NominalSchemaProjectionError::GenerationMismatch)
    );
}

#[test]
fn retained_shape_recursive_name_layout_and_identity_are_checked() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let first = checked(&fixture, &report, "First");
    let second = checked(&fixture, &report, "Second");
    let node = checked(&fixture, &report, "Node");
    let cancellation = AtomicBool::new(false);
    let mut context = context(
        &fixture,
        &report,
        &cancellation,
        NominalResolutionLimits::PRODUCTION,
        aggregate_limits(4),
    );

    let first_projection = context
        .project_checked(&first)
        .expect("first retained projection")
        .clone();
    let second_projection = context
        .project_checked(&second)
        .expect("second retained projection")
        .clone();
    let recursive = context
        .project_checked(&node)
        .expect("legal recursive declaration uses a named schema leaf")
        .clone();

    assert!(matches!(first_projection.shape(), TypeShape::Record { .. }));
    assert_ne!(first_projection.layout(), second_projection.layout());
    let TypeShape::Record { fields, .. } = recursive.shape() else {
        panic!("recursive Node remains one retained record shape")
    };
    assert!(matches!(&fields[0].shape, TypeShape::Option(_)));

    let forged = CheckedProjectNominal::new(
        first.declaration().clone(),
        first.owner(),
        SemanticTypeDigest::from_bytes([0xA5; 32]),
        first.arguments().to_vec(),
    );
    assert!(matches!(
        context.project_checked(&forged),
        Err(NominalSchemaProjectionError::IdentityMismatch { .. })
    ));
}

#[test]
fn sealed_catalog_retains_instantiated_fields_and_variant_payloads() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let pair = checked(&fixture, &report, "Pair");
    let choice = checked(&fixture, &report, "ChoiceValue");
    let cancellation = AtomicBool::new(false);
    let mut inventory = RuntimeNominalProjectionRequestInventory::default();
    inventory
        .insert(RuntimeNominalProjectionRequest::new(pair.clone()))
        .expect("Pair request");
    inventory
        .insert(RuntimeNominalProjectionRequest::new(choice.clone()))
        .expect("ChoiceValue request");

    let seal = context(
        &fixture,
        &report,
        &cancellation,
        NominalResolutionLimits::PRODUCTION,
        aggregate_limits(2),
    )
    .project_inventory(inventory.clone())
    .expect("complete request inventory projects once");
    let catalog = seal
        .validate_final_inventory(inventory)
        .expect("the final replay is exactly the projected inventory")
        .finish();

    let pair_projection = catalog.get(&pair).expect("borrowed Pair projection");
    assert_eq!(
        pair_projection
            .record_fields()
            .iter()
            .map(|field| (field.declaration_ordinal(), field.ty()))
            .collect::<Vec<_>>(),
        [(0, &TypeKind::I64), (1, &TypeKind::Bool)]
    );
    assert!(
        pair_projection
            .record_fields()
            .iter()
            .all(|field| field.field_type() == field.ty().semantic_identity_digest())
    );

    let choice_projection = catalog
        .get(&choice)
        .expect("borrowed ChoiceValue projection");
    assert_eq!(
        choice_projection
            .variant_cases()
            .iter()
            .map(|case| (case.ordinal(), case.payload()))
            .collect::<Vec<_>>(),
        [(0, Some(&TypeKind::I64)), (1, Some(&TypeKind::Bool))]
    );
}

#[test]
fn projection_seal_rejects_missing_and_post_inventory_requests() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let first = checked(&fixture, &report, "First");
    let second = checked(&fixture, &report, "Second");
    let cancellation = AtomicBool::new(false);
    let mut projected = RuntimeNominalProjectionRequestInventory::default();
    projected
        .insert(RuntimeNominalProjectionRequest::new(first.clone()))
        .expect("First request");

    let missing = context(
        &fixture,
        &report,
        &cancellation,
        NominalResolutionLimits::PRODUCTION,
        aggregate_limits(1),
    )
    .project_inventory(projected.clone())
    .expect("First inventory projects")
    .validate_final_inventory(RuntimeNominalProjectionRequestInventory::default());
    assert!(matches!(
        missing,
        Err(NominalSchemaProjectionError::MissingFinalRequest { semantic_type })
            if semantic_type == first.identity()
    ));

    let mut expanded = projected.clone();
    expanded
        .insert(RuntimeNominalProjectionRequest::new(second.clone()))
        .expect("Second request");
    let unexpected = context(
        &fixture,
        &report,
        &cancellation,
        NominalResolutionLimits::PRODUCTION,
        aggregate_limits(1),
    )
    .project_inventory(projected)
    .expect("First inventory projects")
    .validate_final_inventory(expanded);
    assert!(matches!(
        unexpected,
        Err(NominalSchemaProjectionError::UnexpectedFinalRequest { semantic_type })
            if semantic_type == second.identity()
    ));
}

#[test]
fn missing_cached_projection_reports_the_first_semantic_digest() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let first = checked(&fixture, &report, "First");
    let second = checked(&fixture, &report, "Second");
    let cancellation = AtomicBool::new(false);
    let mut inventory = RuntimeNominalProjectionRequestInventory::default();
    inventory
        .insert(RuntimeNominalProjectionRequest::new(first))
        .expect("First request");
    inventory
        .insert(RuntimeNominalProjectionRequest::new(second))
        .expect("Second request");
    let expected = *inventory
        .by_semantic_type
        .keys()
        .next()
        .expect("two requested semantic digests");
    let mut seal = context(
        &fixture,
        &report,
        &cancellation,
        NominalResolutionLimits::PRODUCTION,
        aggregate_limits(2),
    )
    .project_inventory(inventory.clone())
    .expect("complete inventory projects");
    seal.accepted.clear();

    assert!(matches!(
        seal.validate_final_inventory(inventory),
        Err(NominalSchemaProjectionError::MissingCachedProjection { semantic_type })
            if semantic_type == expected
    ));
}

#[test]
fn projection_failure_precedence_uses_semantic_digest_not_insertion_order() {
    let fixture = projection_fixture();
    let report = analyze(&fixture).expect("projection fixture final analysis");
    let first = checked(&fixture, &report, "First");
    let second = checked(&fixture, &report, "Second");
    let cancellation = AtomicBool::new(false);
    let wrong_first = CheckedProjectNominal::new(
        first.declaration().clone(),
        second.owner(),
        first.identity(),
        first.arguments().to_vec(),
    );
    let wrong_second = CheckedProjectNominal::new(
        second.declaration().clone(),
        first.owner(),
        second.identity(),
        second.arguments().to_vec(),
    );
    let expected_nominal = if first.identity() < second.identity() {
        first.declaration().qualified_name()
    } else {
        second.declaration().qualified_name()
    };
    let mut inventory = RuntimeNominalProjectionRequestInventory::default();
    inventory
        .insert(RuntimeNominalProjectionRequest::new(wrong_second))
        .expect("later digest is inserted first");
    inventory
        .insert(RuntimeNominalProjectionRequest::new(wrong_first))
        .expect("earlier digest is inserted second");

    assert!(matches!(
        context(
            &fixture,
            &report,
            &cancellation,
            NominalResolutionLimits::PRODUCTION,
            aggregate_limits(2),
        )
        .project_inventory(inventory),
        Err(NominalSchemaProjectionError::OwnerMismatch { nominal, .. })
            if nominal == expected_nominal
    ));
}
