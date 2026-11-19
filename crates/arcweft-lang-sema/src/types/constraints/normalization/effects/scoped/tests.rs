use super::*;
use crate::{
    effect_row::{
        EffectConstraintVariable, EffectFormula,
        prepared::{
            EffectResidualPredicateView, PreparedEffectRelation, PreparedEffectRow,
            ScopedEffectCompletion,
        },
    },
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericDeclarationBinder, GenericEffectParameterId,
        GenericParameterOwnerId, GenericScope, GenericScopeError,
        constraints::{
            NoConstraintClient, OriginalProducerSource, OriginalRigidScope, TypeConstraintAbort,
            TypeConstraintEffectScope, TypeConstraintParameterScope,
            context::{LocalConstraintAccounting, TypeConstraintLimits},
        },
    },
};
use std::{
    any::Any,
    borrow::Cow,
    sync::atomic::{AtomicBool, Ordering},
};

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

#[derive(Debug)]
struct Source {
    identity: Arc<()>,
    effects: [GenericEffectReference; 2],
    domain: EffectPredicate,
}

impl OriginalProducerSource for Source {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn same_source(&self, other: &dyn OriginalProducerSource) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| Arc::ptr_eq(&self.identity, &other.identity))
    }
    fn rigid_scope(&self) -> OriginalRigidScope<'_> {
        OriginalRigidScope {
            types: &[],
            consts: &[],
            effects: &self.effects,
            imported: None,
        }
    }
    fn declaration_formals(&self) -> Result<GenericDeclarationBinder, GenericScopeError> {
        GenericDeclarationBinder::new(
            GenericScope::default(),
            Box::new([]),
            Box::new([]),
            Box::new([]),
        )
    }
    fn domain(&self) -> Cow<'_, EffectPredicate> {
        Cow::Borrowed(&self.domain)
    }
}

fn context(cancellation: &AtomicBool) -> Context<'_> {
    TypeConstraintContext::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(131_072, 65_536, 1024, 1024),
        cancellation,
    ))
}

fn path(
    context: &mut Context<'_>,
    graph: &mut PreparedEffectGraph,
    constrained: bool,
    inferred: bool,
) -> (
    Arc<Source>,
    ConstraintPath<NoConstraintClient>,
    Option<GenericEffectReference>,
) {
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_701));
    let effects: [GenericEffectReference; 2] =
        [0, 1].map(|slot| GenericEffectParameterId::new(owner.clone(), slot).into());
    let domain = if constrained {
        EffectFormula::variable(effects[0].clone(), context)
            .unwrap()
            .subset(&EffectFormula::empty(), context)
            .unwrap()
    } else {
        EffectPredicate::unconstrained()
    };
    let source = Arc::new(Source {
        identity: Arc::new(()),
        effects,
        domain,
    });
    let original = context.original_producer_context(&source).unwrap();
    let parameters = if inferred {
        TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [],
            [],
            TypeConstraintEffectScope::seal_call_scope(
                [EffectConstraintVariable::new(
                    source.effects[0].clone(),
                    EffectConstraintEligibility::Bindable,
                )],
                [],
            )
            .unwrap(),
            [],
            [],
        )
        .unwrap()
    } else {
        TypeConstraintParameterScope::empty()
    };
    let variable = inferred.then(|| parameters.effect_reference(&source.effects[0]).unwrap());
    let owner = context
        .application_scope((), parameters, original.clone(), graph)
        .unwrap();
    let path = context
        .start_path_with_imported(owner, None, original, graph.read())
        .unwrap();
    (source, path, variable)
}

fn open(reference: GenericEffectReference) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), reference))
}

fn quantified_inputs(frozen: &ScopedEffectCompletion) -> Vec<Vec<GenericEffectReference>> {
    let mut todo = vec![frozen.satisfiable()];
    let mut scopes = Vec::new();
    while let Some(predicate) = todo.pop() {
        match predicate.view().unwrap() {
            EffectResidualPredicateView::And(children) => todo.extend(children),
            EffectResidualPredicateView::Not(body)
            | EffectResidualPredicateView::ExistsLocalFunctions { body, .. }
            | EffectResidualPredicateView::ExistsUniformChoices { body, .. }
            | EffectResidualPredicateView::SubstituteLower { body, .. } => todo.push(body),
            EffectResidualPredicateView::ForAllFiniteRows { parameters, .. } => {
                scopes.push(parameters.to_vec());
            }
            _ => {}
        }
    }
    scopes
}

#[test]
fn the_actual_source_domain_controls_r_and_unused_gamma_is_retained() {
    for constrained in [false, true] {
        let cancellation = AtomicBool::new(false);
        let mut context = context(&cancellation);
        let mut graph = PreparedEffectGraph::default();
        context
            .with_binder(GenericBinder::new(0, 0, 2), |context| {
                let (source, mut path, _) = path(context, &mut graph, constrained, false);
                path.effects.constrain_relation(
                    &mut graph,
                    PreparedEffectRelation::Equal,
                    &open(source.effects[0].clone()),
                    &PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
                    context,
                )?;
                let before = path.effects.clone();
                let receipt = path.prepare_effect_constraint(&mut graph, context)?;
                let frozen = graph.read().freeze_scoped(&[], &[], &[&receipt], context)?;
                let mut expected = vec![
                    path.lineage.scope.shape().bound_effect(0, 0)?,
                    path.lineage.scope.shape().bound_effect(0, 1)?,
                ];
                expected.extend(source.effects.iter().cloned());
                assert_eq!(quantified_inputs(&frozen), vec![expected]);
                assert_eq!(
                    frozen.reduce(context)?.satisfiable().truth_value(),
                    Some(constrained)
                );
                assert_eq!(path.effects, before);
                Ok::<_, TypeConstraintError>(())
            })
            .unwrap();
    }
}

#[test]
fn inference_inputs_use_actual_functions_and_do_not_become_universal_rows() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let (source, mut path, variable) = path(context, &mut graph, false, true);
            let variable = variable.unwrap();
            let function = path
                .applications
                .require_application(path.root_id())?
                .effect_function(&variable)
                .unwrap()
                .clone();
            path.effects.constrain_relation(
                &mut graph,
                PreparedEffectRelation::Equal,
                &open(variable.clone()),
                &open(source.effects[0].clone()),
                context,
            )?;
            let receipt = path.prepare_effect_constraint(&mut graph, context)?;
            let frozen = graph.read().freeze_scoped(&[], &[], &[&receipt], context)?;
            let parameters = quantified_inputs(&frozen);
            assert_eq!(parameters.len(), 1);
            assert_eq!(parameters[0].len(), 3);
            assert!(!parameters[0].contains(&variable));
            assert_eq!(
                frozen
                    .unknown(&function)?
                    .residual()
                    .function
                    .parameters()
                    .len(),
                3
            );
            assert_eq!(
                frozen.reduce(context)?.satisfiable().truth_value(),
                Some(true)
            );
            Ok::<_, TypeConstraintError>(())
        })
        .unwrap();
}

#[test]
fn preparation_borrows_the_retained_frames_and_cancellation_preserves_the_branch() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let original_scope = context.enter_binder_scope(GenericBinder::new(0, 0, 1));
    let (_, path, _) = path(&mut context, &mut graph, false, true);
    context.restore_binder_scope(original_scope);
    let other_scope = context.enter_binder_scope(GenericBinder::new(0, 0, 1));
    let other = context.lexical_frames().clone();
    assert_ne!(&other, &path.lineage.scope);
    path.prepare_effect_constraint(&mut graph, &mut context)
        .unwrap();
    assert_eq!(context.lexical_frames(), &other);
    let before = path.effects.clone();
    let retained = graph.retained_terms();
    cancellation.store(true, Ordering::Relaxed);
    assert!(matches!(
        path.prepare_effect_constraint(&mut graph, &mut context),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
    assert_eq!(path.effects, before);
    assert_eq!(graph.retained_terms(), retained);
    assert_eq!(context.lexical_frames(), &other);
    context.restore_binder_scope(other_scope);
}

#[test]
fn final_rows_keep_the_original_frames_and_only_their_actual_free_support() {
    for bound in [false, true] {
        let cancellation = AtomicBool::new(false);
        let mut context = context(&cancellation);
        let mut graph = PreparedEffectGraph::default();
        let previous = context.enter_binder_scope(GenericBinder::new(0, 0, 1));
        let (source, mut path, key) = path(&mut context, &mut graph, false, true);
        let key = key.unwrap();
        let expected = if bound {
            path.lineage.scope.shape().bound_effect(0, 0).unwrap()
        } else {
            source.effects[0].clone()
        };
        path.effects
            .constrain_relation(
                &mut graph,
                PreparedEffectRelation::Equal,
                &open(key.clone()),
                &open(expected.clone()),
                &mut context,
            )
            .unwrap();
        let receipt = path
            .prepare_effect_constraint(&mut graph, &mut context)
            .unwrap();
        let frozen = graph
            .read()
            .freeze_scoped(&[], &[], &[&receipt], &mut context)
            .unwrap();
        let bound_original = graph
            .bind_original_scoped(&frozen, &mut context)
            .unwrap()
            .freeze(graph.read(), &mut context)
            .unwrap();
        context.restore_binder_scope(previous);
        let previous = context.enter_binder_scope(GenericBinder::new(0, 0, 1));
        let caller_frames = context.lexical_frames().clone();
        assert_ne!(&caller_frames, &path.lineage.scope);
        let effects = ConstraintClosedEffects::for_path(
            &path,
            &bound_original,
            &[],
            &mut graph,
            &mut context,
        )
        .unwrap();
        let row = effects.row(&key).unwrap();
        assert_eq!(row.value(), &EffectRow::open(EffectSet::new(), expected));
        assert_eq!(row.scope(), &path.lineage.scope);
        assert_eq!(effects.context.original_context(), &path.lineage.original);
        let support = effects
            .context
            .support(row.value().variables().unwrap(), &mut context)
            .unwrap();
        assert!(!support.admits_original_parameter(&source.effects[1].clone().into()));
        assert_eq!(context.lexical_frames(), &caller_frames);
        context.restore_binder_scope(previous);
    }
}

#[test]
fn an_older_minimum_cannot_silently_admit_a_new_source_fact() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let (source, mut path, key) = path(&mut context, &mut graph, false, true);
    let key = key.unwrap();
    let application = path.applications.effect_application(&key).unwrap();
    let mut edit = graph.edit_terms();
    let query = application
        .transport(&path.lineage.scope, &mut edit, &mut context)
        .unwrap();
    edit.commit().unwrap();
    let receipt = path
        .prepare_effect_constraint(&mut graph, &mut context)
        .unwrap();
    let frozen = graph
        .read()
        .freeze_scoped(&[&query], &[], &[&receipt], &mut context)
        .unwrap();
    let bound_original = graph
        .bind_original_scoped(&frozen, &mut context)
        .unwrap()
        .freeze(graph.read(), &mut context)
        .unwrap();
    path.effects
        .constrain_relation(
            &mut graph,
            PreparedEffectRelation::Equal,
            &open(key),
            &open(source.effects[0].clone()),
            &mut context,
        )
        .unwrap();
    let before = path.effects.clone();
    let retained = graph.retained_terms();
    assert!(
        ConstraintClosedEffects::for_path(&path, &bound_original, &[], &mut graph, &mut context,)
            .is_err()
    );
    assert_eq!(path.effects, before);
    assert_eq!(graph.retained_terms(), retained);
}
