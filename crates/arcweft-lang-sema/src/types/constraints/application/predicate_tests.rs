use super::*;
use crate::{
    effect_row::{
        EffectPredicate, EffectRow,
        prepared::{
            BoundScopedEffectCompletion, PreparedEffectFailure, PreparedEffectGraph,
            PreparedEffectInvariant, PreparedEffectPredicate, PreparedEffectRelation,
            PreparedEffectRow, PreparedEffectUnknownEligibility,
        },
    },
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericEffectParameterId, GenericParameterOwnerId,
        GenericScope,
        constraints::{
            NoConstraintClient, TypeConstraintError,
            context::{LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits},
        },
    },
};
use std::sync::atomic::AtomicBool;

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

fn context(cancellation: &AtomicBool) -> Context<'_> {
    Context::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(1_048_576, 524_288, 1024, 1024),
        cancellation,
    ))
}

fn binder() -> GenericBinder {
    GenericBinder::new(0, 0, 1)
}

fn formal() -> GenericEffectReference {
    GenericScope::default()
        .with_binder(binder())
        .bound_effect(0, 0)
        .unwrap()
}

fn scheme_parameters(
    predicate: PreparedEffectPredicate,
    context: &mut Context<'_>,
) -> TypeConstraintParameterScope {
    TypeConstraintParameterScope::for_function_scheme(binder(), predicate, &[], &[], &[], context)
        .unwrap()
}

fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}

fn row(labels: &[&str]) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::closed(
        EffectSet::from_labels(labels.iter().copied()).unwrap(),
    ))
}

fn complete(
    path: &crate::types::constraints::ConstraintPath<NoConstraintClient>,
    graph: &mut PreparedEffectGraph,
    context: &mut Context<'_>,
) -> crate::types::constraints::TypeConstraintSolution {
    let mut scope = ConstraintLexicalScope::default();
    path.applications
        .merge_lexical_scopes(&mut scope, context)
        .unwrap();
    let predicate = path
        .applications
        .prepare_effect_predicate(
            &path.effects.ambient,
            path.effects.ambient.original_predicate(),
            &scope,
            graph,
            context,
        )
        .unwrap();
    let receipt = graph
        .read()
        .scoped_constraint(&[], &truth(), &predicate, context)
        .unwrap();
    let original = graph
        .read()
        .freeze_scoped(&[], &[], &[&receipt], context)
        .unwrap();
    let original = graph
        .bind_original_scoped(&original, context)
        .unwrap()
        .freeze(graph.read(), context)
        .unwrap();
    let closed = crate::types::constraints::normalization::ConstraintClosedEffects::for_path(
        path,
        &original,
        &[],
        graph,
        context,
    )
    .unwrap();
    crate::types::constraints::TypeConstraintSolution::complete_application_with_effects(
        path,
        path.root_id(),
        &mut closed.projection(),
        graph.read(),
        context,
    )
    .unwrap()
}

#[test]
fn completed_solution_equality_uses_canonical_values_instead_of_prepared_term_ordinals() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let mut solutions = Vec::new();
    let mut templates = Vec::new();
    for label in ["same.value", "same.value", "different.value"] {
        let predicate = graph
            .relate(
                PreparedEffectRelation::Equal,
                PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), formal())),
                row(&[label]),
                &mut context,
            )
            .unwrap();
        templates.push(predicate.clone());
        let parameters = scheme_parameters(predicate, &mut context);
        let original = context.fixture_original_context(None).unwrap();
        let owner = context
            .application_scope((), parameters, original.clone(), &mut graph)
            .unwrap();
        let path = context
            .start_path_with_imported(owner, None, original, graph.read())
            .unwrap();
        solutions.push(complete(&path, &mut graph, &mut context));
    }
    assert_ne!(templates[0], templates[1]);
    assert!(
        solutions[0]
            .equal_with(&solutions[1], &mut context)
            .unwrap()
    );
    assert_eq!(solutions[0], solutions[1]);
    assert!(
        !solutions[0]
            .equal_with(&solutions[2], &mut context)
            .unwrap()
    );
    assert_ne!(solutions[0], solutions[2]);
}

#[test]
fn an_application_opens_its_original_predicate_once_before_actual_named_and_future_inputs() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let named_row = graph.application(&named, vec![], &mut context).unwrap();
    let future = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Future,
            &mut context,
        )
        .unwrap();
    let future_row = graph
        .unknown_application(&future, vec![], &mut context)
        .unwrap();
    let actual = graph
        .union(vec![named_row, future_row], &mut context)
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), formal())),
            actual,
            &mut context,
        )
        .unwrap();
    let template = predicate.clone();
    let parameters = scheme_parameters(predicate, &mut context);
    let variable = parameters.opened_effect_variables()[0].variable().clone();
    let original = context.fixture_original_context(None).unwrap();
    let owner = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    let function = owner.effect_function(&variable).unwrap().clone();
    let opened = owner.opened_effect_predicate().clone();
    let mut references = BTreeSet::new();
    graph
        .read()
        .visit_predicate_parameters(&opened, &mut context, &mut |reference, _| {
            references.insert(reference.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(references, BTreeSet::from([variable]));
    references.clear();
    graph
        .read()
        .visit_predicate_parameters(&template, &mut context, &mut |reference, _| {
            references.insert(reference.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(references, BTreeSet::from([formal()]));
    let retained = graph.retained_terms();
    let path = context
        .start_path_with_imported(owner, None, original, graph.read())
        .unwrap();
    assert_eq!(path.effects.ambient.original_predicate(), &opened);
    assert_eq!(graph.retained_terms(), retained);
    let mut scope = ConstraintLexicalScope::default();
    path.applications
        .merge_lexical_scopes(&mut scope, &mut context)
        .unwrap();
    let predicate = path
        .applications
        .prepare_effect_predicate(
            &path.effects.ambient,
            path.effects.ambient.original_predicate(),
            &scope,
            &mut graph,
            &mut context,
        )
        .unwrap();
    let receipt = graph
        .read()
        .scoped_constraint(&[], &truth(), &predicate, &mut context)
        .unwrap();
    let original = graph
        .read()
        .freeze_scoped(&[], &[&predicate], &[&receipt], &mut context)
        .unwrap();
    let original =
        BoundScopedEffectCompletion::pending(original, graph.read(), &mut context).unwrap();
    let future_value = EffectRow::closed(EffectSet::from_labels(["future.actual"]).unwrap());
    let inputs = graph
        .read()
        .actual_inputs(&[(&future, &future_value)], &mut context)
        .unwrap();
    assert!(matches!(
        original.close_original_with_inputs(&mut graph, &[], &[], &inputs, &mut context),
        Err(PreparedEffectFailure::ObservationNeedsBody(declaration)) if declaration == named
    ));
    graph
        .prepare_body(&named, row(&["named.body"]), vec![], &mut context)
        .unwrap();
    let missing = graph.read().actual_inputs(&[], &mut context).unwrap();
    assert!(matches!(
        original.close_original_with_inputs(&mut graph, &[], &[], &missing, &mut context),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::UnresolvedFuture
        ))
    ));
    let closed = original
        .close_original_with_inputs(&mut graph, &[], &[], &inputs, &mut context)
        .unwrap();
    assert_eq!(
        closed.unknown(&function).unwrap(),
        &EffectRow::closed(EffectSet::from_labels(["named.body", "future.actual"]).unwrap())
    );
    assert_eq!(
        closed.resolve_predicate(&predicate, &mut context).unwrap(),
        EffectPredicate::unconstrained()
    );
    graph.read().validate_retained_predicate(&template).unwrap();
}

#[test]
fn invalid_predicate_issuers_and_parameters_fail_before_an_application_is_published() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut foreign = PreparedEffectGraph::default();
    let named = foreign
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let actual = foreign.application(&named, vec![], &mut context).unwrap();
    let predicate = foreign
        .relate(
            PreparedEffectRelation::Equal,
            actual,
            row(&[]),
            &mut context,
        )
        .unwrap();
    let mut graph = PreparedEffectGraph::default();
    let original = context.fixture_original_context(None).unwrap();
    let retained = graph.retained_terms();
    let parameters = scheme_parameters(predicate, &mut context);
    assert!(matches!(
        context.application_scope((), parameters, original.clone(), &mut graph),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::ForeignIssuer
            ))
        ))
    ));
    assert_eq!(graph.retained_terms(), retained);
    let foreign_variable: GenericEffectReference = GenericEffectParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_945)),
        0,
    )
    .into();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            PreparedEffectRow::Canonical(EffectRow::open(
                EffectSet::new(),
                foreign_variable.clone(),
            )),
            row(&[]),
            &mut context,
        )
        .unwrap();
    let retained = graph.retained_terms();
    let parameters = scheme_parameters(predicate, &mut context);
    match context.application_scope((), parameters, original.clone(), &mut graph) {
        Err(error) => assert_eq!(
            error,
            crate::types::constraints::effect_invariant(
                crate::types::constraints::TypeConstraintEffectInvariantKind::ForeignVariable,
                Some(foreign_variable)
            )
        ),
        Ok(_) => panic!("foreign predicate parameter was admitted"),
    }
    assert_eq!(graph.retained_terms(), retained);
    let parameters = scheme_parameters(truth(), &mut context);
    let owner = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    context
        .start_path_with_imported(owner, None, original, graph.read())
        .unwrap();
}
