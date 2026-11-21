use super::*;
use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericConstParameterId, GenericParameterOwnerId,
        GenericTypeParameterId,
        constraints::{
            ConstraintAcceptance, NoConstraintClient, TypeConstraintConstEligibility,
            TypeConstraintEffectScope, TypeConstraintParameterEligibility,
            TypeConstraintParameterScope, TypeConstraintProjectionClosure,
            context::{
                LocalConstraintAccounting, TypeConstraintConstParameterScopeRow,
                TypeConstraintLimits, TypeConstraintTypeParameterScopeRow,
            },
            test_support::ConstraintTestSetup,
            transaction::TypeConstraintTransaction,
        },
    },
};
use std::sync::atomic::{AtomicBool, Ordering};

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;
type Component = OpenConstraintComponent<NoConstraintClient, &'static str>;

fn context(cancellation: &AtomicBool) -> Context<'_> {
    ConstraintTestSetup::new(
        TypeConstraintLimits::new(65536, 65536, 1024, 1024),
        cancellation,
    )
    .into_parts()
    .0
}

fn parameters(
    types: u16,
    lengths: u16,
) -> (
    TypeConstraintParameterScope,
    Vec<GenericTypeReference>,
    Vec<GenericConstReference>,
) {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(9481));
    let types: Vec<_> = (0..types)
        .map(|slot| GenericTypeParameterId::new(owner.clone(), slot))
        .collect();
    let lengths: Vec<_> = (0..lengths)
        .map(|slot| GenericConstParameterId::new(owner.clone(), slot))
        .collect();
    let scope = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        types.iter().map(|parameter| {
            TypeConstraintTypeParameterScopeRow::new(
                parameter.clone(),
                TypeConstraintParameterEligibility::Bindable,
            )
        }),
        lengths.iter().map(|parameter| {
            TypeConstraintConstParameterScopeRow::new(
                parameter.clone(),
                TypeConstraintConstEligibility::Bindable,
            )
        }),
        TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
        [],
        [],
    )
    .unwrap();
    let types = types
        .into_iter()
        .map(|parameter| scope.type_reference(&parameter.into()).unwrap())
        .collect();
    let lengths = lengths
        .into_iter()
        .map(|parameter| scope.const_reference(&parameter.into()).unwrap())
        .collect();
    (scope, types, lengths)
}

fn frontier(
    context: &mut Context<'_>,
    graph: &mut PreparedEffectGraph,
    scope: TypeConstraintParameterScope,
    equations: &[(TypeKind, TypeKind)],
) -> SuspendedConstraintFrontier<NoConstraintClient> {
    frontier_with_imported(context, graph, scope, equations, None)
}

fn frontier_with_imported(
    context: &mut Context<'_>,
    graph: &mut PreparedEffectGraph,
    scope: TypeConstraintParameterScope,
    equations: &[(TypeKind, TypeKind)],
    imported: Option<crate::types::constraints::ImportedGenericParameterScopeLease>,
) -> SuspendedConstraintFrontier<NoConstraintClient> {
    frontier_equations(
        context,
        graph,
        scope,
        equations,
        imported,
        EquationOrigin::Original,
    )
}

enum EquationOrigin {
    Original,
    Application,
}

fn frontier_application(
    context: &mut Context<'_>,
    graph: &mut PreparedEffectGraph,
    scope: TypeConstraintParameterScope,
    equations: &[(TypeKind, TypeKind)],
) -> SuspendedConstraintFrontier<NoConstraintClient> {
    frontier_equations(
        context,
        graph,
        scope,
        equations,
        None,
        EquationOrigin::Application,
    )
}

fn frontier_equations(
    context: &mut Context<'_>,
    graph: &mut PreparedEffectGraph,
    scope: TypeConstraintParameterScope,
    equations: &[(TypeKind, TypeKind)],
    imported: Option<crate::types::constraints::ImportedGenericParameterScopeLease>,
    origin: EquationOrigin,
) -> SuspendedConstraintFrontier<NoConstraintClient> {
    let mut lower = TypeConstraintTransaction::initialize_with_imported_fixture(
        context,
        graph,
        (),
        scope,
        None,
        imported,
    )
    .unwrap();
    for (pattern, actual) in equations {
        let pattern = context.prepare_fixture_type(pattern).unwrap();
        let actual = context.prepare_fixture_type(actual).unwrap();
        match origin {
            EquationOrigin::Original => lower.constrain(
                context,
                graph,
                &pattern,
                &actual,
                ConstraintAcceptance::PatternAcceptsActual,
            ),
            EquationOrigin::Application => lower.constrain_application_template(
                context,
                graph,
                &pattern,
                &actual,
                ConstraintAcceptance::PatternAcceptsActual,
            ),
        }
    }
    lower.request_projection(
        context,
        graph,
        (),
        &TypeKind::I64,
        TypeConstraintProjectionClosure::Closed,
    );
    lower.suspend_result(context, ()).unwrap()
}

fn array(length: ArrayLength) -> TypeKind {
    TypeKind::Array {
        item: Box::new(TypeKind::Bool),
        len: length,
    }
}

#[test]
fn shared_original_type_and_const_aliases_are_unified_without_overwriting_either_root() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let (scope, ty, length) = parameters(2, 2);
    let first = frontier_application(
        &mut context,
        &mut effects,
        scope.clone(),
        &[
            (
                TypeKind::GenericParam(ty[0].clone()),
                TypeKind::GenericParam(ty[1].clone()),
            ),
            (
                array(ArrayLength::Generic(length[0].clone())),
                array(ArrayLength::Generic(length[1].clone())),
            ),
        ],
    );
    let first = component
        .admit_fixture_roots(&mut context, &mut effects, vec![first], "T=U,N=M")
        .unwrap();
    let second = frontier_application(
        &mut context,
        &mut effects,
        scope,
        &[
            (TypeKind::GenericParam(ty[0].clone()), TypeKind::Bool),
            (
                array(ArrayLength::Generic(length[0].clone())),
                array(ArrayLength::Const(7)),
            ),
        ],
    );
    component
        .admit_fixture_roots(&mut context, &mut effects, vec![second], "T=bool,N=7")
        .unwrap();
    let forest = component
        .infer_original_products(&mut context, &mut effects)
        .unwrap();
    assert_eq!(forest.products.len(), 1);
    let product = &forest.products[0];
    assert_eq!(product.members.len(), 2);
    let solutions = product.result.as_ref().ok().expect("one joint MGU exists");
    assert_eq!(solutions.len(), 1);
    let solved = &solutions[0];
    assert!(Arc::ptr_eq(&solved.branch.lineage, &product.members));
    for parameter in &ty {
        assert_eq!(
            solved
                .branch
                .bindings
                .get(parameter)
                .map(|value| value.value()),
            Some(&TypeKind::Bool)
        );
    }
    for parameter in &length {
        assert_eq!(
            solved
                .branch
                .const_bindings
                .get(parameter)
                .map(|value| value.value()),
            Some(&ArrayLength::Const(7))
        );
    }
    let ValueDefinition::Constraint { frontiers, .. } =
        &forest.original.value(&first).unwrap().definition
    else {
        unreachable!()
    };
    let original = &frontiers[0].transaction.frontier[0];
    assert_eq!(
        original.bindings.get(&ty[0]).map(|value| value.value()),
        Some(&TypeKind::GenericParam(ty[1].clone()))
    );
    assert_eq!(
        original
            .const_bindings
            .get(&length[0])
            .map(|value| value.value()),
        Some(&ArrayLength::Generic(length[1].clone()))
    );
    assert!(!original.bindings.contains_key(&ty[1]));
    assert!(!original.const_bindings.contains_key(&length[1]));
    let forest = forest.close(&[], &mut effects, &mut context).unwrap();
    let solution = &forest.products[0]
        .result
        .as_ref()
        .expect("closed joint product")[0];
    assert!(solution.effects.predicate().value().is_unconstrained());
}

#[test]
fn original_products_keep_conflict_addresses_and_exclude_diagnostic_witnesses() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let (scope, ty, length) = parameters(1, 1);
    let equations = |value, count| {
        vec![
            (TypeKind::GenericParam(ty[0].clone()), value),
            (
                array(ArrayLength::Generic(length[0].clone())),
                array(ArrayLength::Const(count)),
            ),
        ]
    };
    let root = frontier(
        &mut context,
        &mut effects,
        scope.clone(),
        &equations(TypeKind::Bool, 3),
    );
    component
        .admit_fixture_roots(&mut context, &mut effects, vec![root], "first")
        .unwrap();
    let mut roots = Vec::new();
    for (value, count) in [
        (TypeKind::Bool, 3),
        (TypeKind::String, 3),
        (TypeKind::Bool, 4),
    ] {
        roots.push(frontier(
            &mut context,
            &mut effects,
            scope.clone(),
            &equations(value, count),
        ));
    }
    let second = component
        .admit_fixture_roots(&mut context, &mut effects, roots, "second")
        .unwrap();
    let forest = component
        .infer_original_products(&mut context, &mut effects)
        .unwrap();
    assert_eq!(forest.products.len(), 3);
    assert!(forest.products[0].result.is_ok());
    let failure = forest.products[1].result.as_ref().err().unwrap();
    assert!(
        matches!(&failure.check, OriginalProductCheck::TypeBinding {member,parameter} if member.producer == second.ordinal && member.frontier == 1 && parameter == &ty[0])
    );
    assert_eq!(failure.reason, TypeConstraintRejection::Mismatch);
    let failure = forest.products[2].result.as_ref().err().unwrap();
    assert!(
        matches!(&failure.check, OriginalProductCheck::ConstBinding {member,parameter} if member.producer == second.ordinal && member.frontier == 2 && parameter == &length[0])
    );
    assert_eq!(failure.reason, TypeConstraintRejection::Mismatch);
}

#[test]
fn cross_root_structural_occurs_cycles_remain_original_product_rejections() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let (scope, ty, _) = parameters(2, 0);
    for (left, right) in [(0, 1), (1, 0)] {
        let root = frontier_equations(
            &mut context,
            &mut effects,
            scope.clone(),
            &[(
                TypeKind::GenericParam(ty[left].clone()),
                TypeKind::Tuple(vec![TypeKind::GenericParam(ty[right].clone())]),
            )],
            None,
            EquationOrigin::Application,
        );
        component
            .admit_fixture_roots(&mut context, &mut effects, vec![root], "acyclic original")
            .unwrap();
    }
    let forest = component
        .infer_original_products(&mut context, &mut effects)
        .unwrap();
    assert_eq!(forest.products.len(), 1);
    assert!(matches!(
        forest.products[0].result.as_ref().err().unwrap().reason,
        TypeConstraintRejection::CyclicInstantiation { .. }
    ));
    assert_eq!(forest.products[0].members.len(), 2);
}

#[test]
fn an_unchosen_original_cannot_lend_required_parameters_to_another_product() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let first = frontier(
        &mut context,
        &mut effects,
        TypeConstraintParameterScope::empty(),
        &[],
    );
    let first_id = first.transaction.application;
    let (scope, _, _) = parameters(1, 0);
    let required = scope.iter().next().unwrap().0.clone();
    let second = frontier(&mut context, &mut effects, scope, &[]);
    let second_id = second.transaction.application;
    component
        .admit_fixture_roots(
            &mut context,
            &mut effects,
            vec![first, second],
            "initializer",
        )
        .unwrap();
    let forest = component
        .infer_original_products(&mut context, &mut effects)
        .unwrap();
    assert_eq!(forest.products.len(), 2);
    let solved = &forest.products[0].result.as_ref().ok().unwrap()[0];
    assert!(solved.branch.applications.application(first_id).is_some());
    assert!(solved.branch.applications.application(second_id).is_none());
    assert!(
        matches!(&forest.products[1].result.as_ref().err().unwrap().reason,TypeConstraintRejection::IncompleteInstantiation {parameter: crate::types::constraints::ConstraintGenericParameterId::Type(parameter)} if parameter == &required)
    );
    let original = context.fixture_original_context(None).unwrap();
    let mut component = Component::new(&mut context, &effects).unwrap();
    component
        .admit_roots(
            &mut context,
            &mut effects,
            original,
            Vec::new(),
            "rejected original",
        )
        .unwrap();
    assert!(
        component
            .infer_original_products(&mut context, &mut effects)
            .unwrap()
            .products
            .is_empty()
    );
}

#[test]
fn one_application_opening_cannot_claim_independently_opened_ambient_binders() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let (scope, ty, _) = parameters(1, 0);
    for _ in 0..2 {
        let enclosing = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
        let value = TypeKind::GenericParam(context.lexical_scope().bound_type(0, 0).unwrap());
        let root = frontier(
            &mut context,
            &mut effects,
            scope.clone(),
            &[(TypeKind::GenericParam(ty[0].clone()), value)],
        );
        component
            .admit_fixture_roots(&mut context, &mut effects, vec![root], "ambient")
            .unwrap();
        context.restore_binder_scope(enclosing);
    }
    assert!(matches!(component.infer_original_products(&mut context, &mut effects),
        Err(PreparedValueFailure::Constraint(TypeConstraintError::Invariant(
            crate::types::constraints::TypeConstraintInvariant::ParameterScope(
                crate::types::constraints::TypeConstraintParameterScopeInvariant::ApplicationContractConflict
            )
        )))
    ));
    assert!(context.lexical_scope().binders().is_empty());
}

fn scoped_function(scope: &GenericScope, ambient_depth: u32) -> TypeKind {
    let binder = GenericBinder::new(1, 1, 1);
    let nested = scope.with_binder(binder);
    TypeKind::function_with_binder(
        binder,
        [
            TypeKind::GenericParam(nested.bound_type(0, 0).unwrap()),
            TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(
                    nested.bound_type(ambient_depth, 0).unwrap(),
                )),
                len: ArrayLength::Generic(nested.bound_const(ambient_depth, 0).unwrap()),
            },
        ],
        TypeKind::Array {
            item: Box::new(TypeKind::Bool),
            len: ArrayLength::Generic(nested.bound_const(0, 0).unwrap()),
        },
        EffectRow::open(
            EffectSet::new(),
            nested.bound_effect(ambient_depth, 0).unwrap(),
        ),
    )
}

#[test]
fn shared_ambient_frames_transport_under_own_binders_across_different_scope_shapes() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let (scope, ty, _) = parameters(1, 0);
    let root_scope = context.enter_binder_scope(GenericBinder::new(1, 1, 1));
    let value = scoped_function(context.lexical_scope(), 1);
    let prepared_value = context.prepare_fixture_type(&value).unwrap();
    let first = frontier(
        &mut context,
        &mut effects,
        scope.clone(),
        &[(TypeKind::GenericParam(ty[0].clone()), value.clone())],
    );
    let lease = first.transaction.frontier[0]
        .applications
        .import_parameters(&[ty[0].clone().into()])
        .unwrap();
    component
        .admit_fixture_roots(&mut context, &mut effects, vec![first], "outer")
        .unwrap();
    let outer = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let expected = scoped_function(context.lexical_scope(), 2);
    let prepared_expected = context.prepare_fixture_type(&expected).unwrap();
    let second = frontier_with_imported(
        &mut context,
        &mut effects,
        TypeConstraintParameterScope::empty(),
        &[(TypeKind::GenericParam(ty[0].clone()), expected.clone())],
        Some(lease),
    );
    component
        .admit_fixture_roots(
            &mut context,
            &mut effects,
            vec![second],
            "outer beneath another frame",
        )
        .unwrap();
    context.restore_binder_scope(outer);
    context.restore_binder_scope(root_scope);
    let forest = component
        .infer_original_products(&mut context, &mut effects)
        .unwrap();
    let product = &forest.products[0];
    assert_eq!(product.scope.shape().binders().len(), 2);
    let solution = &product
        .result
        .as_ref()
        .ok()
        .expect("same original ambient binder")[0];
    assert_eq!(
        solution
            .branch
            .bindings
            .get(&ty[0])
            .map(|value| value.value()),
        Some(&prepared_value)
    );
    let projected = context
        .with_original_scope(product.scope.clone(), |context| {
            crate::types::constraints::normalization::project_type(
                &TypeKind::GenericParam(ty[0].clone()),
                solution.branch.projection_view(),
                crate::types::constraints::ConstraintClosurePolicy::ProjectionClosed,
                &mut effects,
                context,
            )
        })
        .unwrap();
    assert_eq!(projected.value, prepared_expected);
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn live_use_tickets_and_cancellation_cannot_publish_a_partial_original_forest() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let root = frontier(
        &mut context,
        &mut effects,
        TypeConstraintParameterScope::empty(),
        &[],
    );
    let value = component
        .admit_fixture_roots(&mut context, &mut effects, vec![root], "initializer")
        .unwrap();
    let use_ = component.issue_fixture_use(&mut context, &[value]).unwrap();
    assert!(matches!(
        component.infer_original_products(&mut context, &mut effects),
        Err(PreparedValueFailure::UseState)
    ));
    drop(use_);
    let component = Component::new(&mut context, &effects).unwrap();
    cancellation.store(true, Ordering::Relaxed);
    assert!(matches!(
        component.infer_original_products(&mut context, &mut effects),
        Err(PreparedValueFailure::Constraint(
            TypeConstraintError::Abort(_)
        ))
    ));
    cancellation.store(false, Ordering::Relaxed);
    assert!(context.lexical_scope().binders().is_empty());
}

#[test]
fn cloned_imported_leases_share_inference_without_making_the_parent_a_source_root() {
    let mut effects = PreparedEffectGraph::default();
    use crate::types::constraints::{
        ConstraintGenericParameterId, application::ConstraintApplicationScope,
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let (scope, ty, _) = parameters(2, 0);
    let opening = ConstraintApplicationScope::test_root((), scope, &mut effects, &mut context);
    let parent = context.start_path(opening, &mut effects).unwrap();
    let parent_id = parent.root_id();
    let lease = parent
        .applications
        .import_parameters(
            &ty.iter()
                .cloned()
                .map(ConstraintGenericParameterId::Type)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    drop(parent);
    for actual in [TypeKind::GenericParam(ty[1].clone()), TypeKind::Bool] {
        let mut lower = TypeConstraintTransaction::initialize_with_imported_fixture(
            &mut context,
            &mut effects,
            (),
            TypeConstraintParameterScope::empty(),
            None,
            Some(lease.clone()),
        )
        .unwrap();
        lower.constrain(
            &mut context,
            &mut effects,
            &TypeKind::GenericParam(ty[0].clone()),
            &actual,
            ConstraintAcceptance::PatternAcceptsActual,
        );
        lower.request_projection(
            &mut context,
            &mut effects,
            (),
            &TypeKind::I64,
            TypeConstraintProjectionClosure::Closed,
        );
        let root = lower.suspend_result(&mut context, ()).unwrap();
        component
            .admit_fixture_roots(&mut context, &mut effects, vec![root], "imported original")
            .unwrap();
    }
    let forest = component
        .infer_original_products(&mut context, &mut effects)
        .unwrap();
    assert_eq!(forest.products.len(), 1);
    assert_eq!(forest.products[0].members.len(), 2);
    let solution = &forest.products[0].result.as_ref().ok().unwrap()[0];
    assert!(
        solution
            .branch
            .applications
            .application(parent_id)
            .is_none()
    );
    for parameter in &ty {
        assert_eq!(
            solution
                .branch
                .bindings
                .get(parameter)
                .map(|value| value.value()),
            Some(&TypeKind::Bool)
        );
    }
}

#[test]
fn exact_agreement_keeps_whole_choice_types_and_rejects_bottom_subtyping() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let choice = TypeKind::Tuple(vec![TypeKind::Choice(vec![
        TypeKind::Bool,
        TypeKind::String,
    ])]);
    let function =
        |result| TypeKind::function_with_effects([], result, EffectRow::closed(EffectSet::new()));
    for (left, right, succeeds) in [
        (choice.clone(), choice, true),
        (function(TypeKind::Never), function(TypeKind::Bool), false),
    ] {
        let mut context = context(&cancellation);
        let mut component = Component::new(&mut context, &effects).unwrap();
        let (scope, ty, _) = parameters(1, 0);
        for value in [left, right] {
            let root = frontier(
                &mut context,
                &mut effects,
                scope.clone(),
                &[(TypeKind::GenericParam(ty[0].clone()), value)],
            );
            component
                .admit_fixture_roots(&mut context, &mut effects, vec![root], "whole type")
                .unwrap();
        }
        let forest = component
            .infer_original_products(&mut context, &mut effects)
            .unwrap();
        assert_eq!(forest.products.len(), 1);
        assert_eq!(forest.products[0].result.is_ok(), succeeds);
        if !succeeds {
            assert_eq!(
                forest.products[0].result.as_ref().err().unwrap().reason,
                TypeConstraintRejection::Mismatch
            );
        }
    }
}

#[test]
fn another_original_binding_can_resolve_a_nonprincipal_effect_relation_before_completion() {
    let mut effects = PreparedEffectGraph::default();
    use crate::{
        effect_row::{EffectConstraintEligibility, EffectConstraintVariable},
        types::GenericEffectParameterId,
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(9482));
    let ty = GenericTypeParameterId::new(owner.clone(), 0);
    let declarations: Vec<crate::types::GenericEffectReference> = (0..2)
        .map(|slot| GenericEffectParameterId::new(owner.clone(), slot).into())
        .collect();
    let scope = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [TypeConstraintTypeParameterScopeRow::new(
            ty.clone(),
            TypeConstraintParameterEligibility::Bindable,
        )],
        [],
        TypeConstraintEffectScope::seal_call_scope(
            declarations.iter().map(|reference| {
                EffectConstraintVariable::new(
                    reference.clone(),
                    EffectConstraintEligibility::Bindable,
                )
            }),
            [],
        )
        .unwrap(),
        [],
        [],
    )
    .unwrap();
    let ty = scope.type_reference(&ty.into()).unwrap();
    let effect_keys: Vec<_> = declarations
        .iter()
        .map(|reference| scope.effect_reference(reference).unwrap())
        .collect();
    let row = |index: usize| EffectRow::open(EffectSet::new(), effect_keys[index].clone());
    let function = |row| TypeKind::function_with_effects([], TypeKind::Bool, row);
    let log = EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap());
    let mut first = frontier_application(
        &mut context,
        &mut effects,
        scope.clone(),
        &[(TypeKind::GenericParam(ty.clone()), function(row(0)))],
    );
    let union = row(0).union(&row(1), &mut context).unwrap();
    first.transaction.frontier[0]
        .effects
        .ambient
        .constrain_fixture_subset(&mut effects, &log, &union, &mut context)
        .unwrap();
    assert!(matches!(
        first.transaction.frontier[0].close_original_path_effects(&mut effects, &mut context),
        Err(TypeConstraintError::Rejected(_))
    ));
    let first = component
        .admit_fixture_roots(
            &mut context,
            &mut effects,
            vec![first],
            "original disjunction",
        )
        .unwrap();
    let second = frontier_application(
        &mut context,
        &mut effects,
        scope,
        &[(TypeKind::GenericParam(ty), function(log.clone()))],
    );
    component
        .admit_fixture_roots(
            &mut context,
            &mut effects,
            vec![second],
            "original resolves e1",
        )
        .unwrap();
    let forest = component
        .infer_original_products(&mut context, &mut effects)
        .unwrap();
    let forest = forest.close(&[], &mut effects, &mut context).unwrap();
    let solution = &forest.products[0]
        .result
        .as_ref()
        .ok()
        .expect("joint effect inference is principal")[0];
    assert_eq!(
        solution.effects.row(&effect_keys[0]).map(|row| row.value()),
        Some(&log)
    );
    assert_eq!(
        solution.effects.row(&effect_keys[1]).map(|row| row.value()),
        Some(&EffectRow::closed(EffectSet::new()))
    );
    let ValueDefinition::Constraint { frontiers, .. } =
        &forest.original.value(&first).unwrap().definition
    else {
        unreachable!()
    };
    assert!(matches!(
        frontiers[0].transaction.frontier[0]
            .close_original_path_effects(&mut effects, &mut context),
        Err(TypeConstraintError::Rejected(_))
    ));
}

#[test]
fn a_later_false_fit_and_different_use_result_do_not_constrain_original_inference() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let (scope, ty, _) = parameters(1, 0);
    let root = frontier(
        &mut context,
        &mut effects,
        scope,
        &[(TypeKind::GenericParam(ty[0].clone()), TypeKind::Bool)],
    );
    let producer = component
        .admit_fixture_roots(&mut context, &mut effects, vec![root], "original")
        .unwrap();
    let mut use_ = component
        .issue_fixture_use(&mut context, &[producer])
        .unwrap();
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
        .unwrap()
    {
        let outcome = if alternative.blocked {
            PreparedValueOutcome::Blocked("producer diagnostic")
        } else {
            PreparedValueOutcome::Conditional {
                fits: PreparedEffectPredicate::Canonical(EffectPredicate::impossible()),
                accepted: PreparedValueOutput {
                    ty: TypeKind::String
                        .prepare_effects(&GenericScope::default(), &mut context)
                        .unwrap(),
                    original: use_.original.clone(),
                    preparation: "later result",
                },
                rejected: "false fit",
            }
        };
        component
            .submit_use(&mut context, &mut use_, alternative, outcome)
            .unwrap();
    }
    let use_ = component.finish_use(use_).unwrap();
    component
        .retain_use(&mut context, &mut effects, use_)
        .unwrap();
    let forest = component
        .infer_original_products(&mut context, &mut effects)
        .unwrap();
    assert_eq!(forest.products.len(), 1);
    let solution = &forest.products[0].result.as_ref().ok().unwrap()[0];
    assert_eq!(
        solution
            .branch
            .bindings
            .get(&ty[0])
            .map(|value| value.value()),
        Some(&TypeKind::Bool)
    );
    assert_eq!(forest.original.values.len(), 2);
}

#[test]
fn original_product_publication_obeys_an_inclusive_shared_node_budget() {
    let mut effects = PreparedEffectGraph::default();
    use crate::types::constraints::context::{
        TypeConstraintAccounting, TypeConstraintContextIssuer, TypeConstraintWorkReport,
    };
    use std::cell::Cell;
    struct Accounting<'a> {
        nodes: Rc<Cell<u64>>,
        cancellation: &'a AtomicBool,
        limits: TypeConstraintLimits,
    }
    impl TypeConstraintAccounting for Accounting<'_> {
        fn charge_constraint(
            &mut self,
            delta: &TypeConstraintWorkReport,
            _: TypeConstraintLimits,
        ) -> Result<(), TypeConstraintError> {
            self.nodes.set(self.nodes.get() + delta.nodes());
            Ok(())
        }
        fn commit(&mut self) {}
    }
    impl<'a> TypeConstraintContextIssuer<'a> for Accounting<'a> {
        fn context_limits(&self) -> TypeConstraintLimits {
            self.limits
        }
        fn context_cancellation(&self) -> &'a AtomicBool {
            self.cancellation
        }
    }
    let cancellation = AtomicBool::new(false);
    let mut execute = |limit| {
        let nodes = Rc::new(Cell::new(0));
        let mut context =
            TypeConstraintContext::<_, NoConstraintClient>::with_accounting(Accounting {
                nodes: Rc::clone(&nodes),
                cancellation: &cancellation,
                limits: TypeConstraintLimits::new(65536, limit, 1024, 1024),
            });
        let component = Component::new(&mut context, &effects).unwrap();
        let result = component.infer_original_products(&mut context, &mut effects);
        (nodes.get(), result)
    };
    let (required, result) = execute(65536);
    assert!(result.is_ok());
    assert!(required > 3);
    let (used, result) = execute(required);
    assert!(result.is_ok());
    assert_eq!(used, required);
    let (_, result) = execute(required - 1);
    assert!(matches!(
        result,
        Err(PreparedValueFailure::Constraint(
            TypeConstraintError::Abort(
                crate::types::constraints::TypeConstraintAbort::NodeLimit { .. }
            )
        ))
    ));
}

#[test]
fn joint_inference_retains_an_imported_owner_frame_absent_from_the_receiving_original() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let (scope, ty, _) = parameters(1, 0);
    let enclosing = context.enter_binder_scope(GenericBinder::new(1, 0, 0));
    let first = frontier(&mut context, &mut effects, scope, &[]);
    let owner = first.transaction.frontier[0].scope.clone();
    let lease = first.transaction.frontier[0]
        .applications
        .import_parameters(&[ty[0].clone().into()])
        .unwrap();
    let actual = TypeKind::GenericParam(context.lexical_scope().bound_type(0, 0).unwrap());
    component
        .admit_fixture_roots(
            &mut context,
            &mut effects,
            vec![first],
            "no binding in its own frame",
        )
        .unwrap();
    context.restore_binder_scope(enclosing);
    let mut lower = TypeConstraintTransaction::initialize_with_imported_fixture(
        &mut context,
        &mut effects,
        (),
        TypeConstraintParameterScope::empty(),
        None,
        Some(lease),
    )
    .unwrap();
    context
        .with_original_scope(owner, |context| {
            lower.constrain(
                context,
                &mut effects,
                &TypeKind::GenericParam(ty[0].clone()),
                &actual,
                ConstraintAcceptance::PatternAcceptsActual,
            );
            Ok::<_, TypeConstraintError>(())
        })
        .unwrap();
    lower.request_projection(
        &mut context,
        &mut effects,
        (),
        &TypeKind::I64,
        TypeConstraintProjectionClosure::Closed,
    );
    let second = lower.suspend_result(&mut context, ()).unwrap();
    assert!(
        second.transaction.frontier[0]
            .scope
            .shape()
            .binders()
            .is_empty()
    );
    component
        .admit_fixture_roots(
            &mut context,
            &mut effects,
            vec![second],
            "receiver retains the original owner's lease",
        )
        .unwrap();
    let forest = component
        .infer_original_products(&mut context, &mut effects)
        .unwrap();
    let solutions = forest.products[0]
        .result
        .as_ref()
        .ok()
        .expect("the exact imported owner authorizes this dependency");
    assert_eq!(solutions.len(), 1);
    assert_eq!(forest.products[0].scope.shape().binders().len(), 1);
    assert_eq!(
        solutions[0]
            .branch
            .bindings
            .get(&ty[0])
            .map(|value| value.value()),
        Some(&actual)
    );
    assert!(context.lexical_scope().binders().is_empty());
}
