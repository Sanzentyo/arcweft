use super::*;
use crate::{
    effect_row::{
        EffectConstraintEligibility, EffectFormula, EffectRow,
        prepared::{EffectResidualPredicateView, PreparedEffectRelation, PreparedEffectRow},
    },
    effects::EffectSet,
    types::{
        DetachedGenericOwnerId, GenericDeclarationBinder, GenericEffectParameterId,
        GenericEffectReference, GenericParameterOwnerId, GenericScopeError,
        constraints::{
            NoConstraintClient, OriginalProducerSource, OriginalRigidScope,
            TypeConstraintParameterScope, TypeConstraintProjectionClosure,
            context::{LocalConstraintAccounting, TypeConstraintLimits},
            transaction::TypeConstraintTransaction,
        },
    },
};
use std::{any::Any, borrow::Cow, sync::atomic::AtomicBool};

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;
type Component = OpenConstraintComponent<NoConstraintClient, ()>;

mod completed_members;
mod materialization;
mod projection_ports;

#[derive(Debug)]
struct Source {
    identity: Arc<()>,
    inputs: [GenericEffectReference; 1],
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
            effects: &self.inputs,
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
        TypeConstraintLimits::new(1_048_576, 524_288, 1024, 1024),
        cancellation,
    ))
}
fn input() -> GenericEffectReference {
    GenericEffectParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_801)),
        0,
    )
    .into()
}
fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}

fn root(
    component: &mut Component,
    context: &mut Context<'_>,
    graph: &mut PreparedEffectGraph,
    domain: EffectPredicate,
    invalid: bool,
) -> OriginalProductMember {
    root_with_projection(
        component,
        context,
        graph,
        domain,
        invalid,
        &TypeKind::<PreparedEffects>::I64,
        None,
    )
}

fn root_with_projection(
    component: &mut Component,
    context: &mut Context<'_>,
    graph: &mut PreparedEffectGraph,
    domain: EffectPredicate,
    invalid: bool,
    projection: &TypeKind<PreparedEffects>,
    expected: Option<&TypeKind<PreparedEffects>>,
) -> OriginalProductMember {
    let source = Arc::new(Source {
        identity: Arc::new(()),
        inputs: [input()],
        domain,
    });
    let original = context.original_producer_context(&source).unwrap();
    let owner = context
        .application_scope(
            (),
            TypeConstraintParameterScope::empty(),
            original.clone(),
            graph,
        )
        .unwrap();
    let mut path = context
        .start_path_with_imported(owner, None, original.clone(), graph.read())
        .unwrap();
    if invalid {
        path.effects
            .constrain_relation(
                graph,
                PreparedEffectRelation::Equal,
                &PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), input())),
                &PreparedEffectRow::Canonical(EffectRow::closed(
                    EffectSet::from_labels(["log.write"]).unwrap(),
                )),
                context,
            )
            .unwrap();
    }
    let application = path.root_id();
    let mut transaction =
        TypeConstraintTransaction::from_path(context, graph, application, path, None).unwrap();
    transaction.request_projection_with_input(
        context,
        graph,
        (),
        projection,
        TypeConstraintProjectionClosure::Closed,
        expected,
    );
    let frontier = transaction.suspend_result(context, ()).unwrap();
    let value = component
        .admit_roots(context, graph, original, vec![frontier], ())
        .unwrap();
    OriginalProductMember {
        producer: value.ordinal,
        frontier: 0,
        path: 0,
    }
}
fn branch(
    component: &Component,
    members: &Arc<[OriginalProductMember]>,
    graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    context: &mut Context<'_>,
) -> JointBranch<NoConstraintClient> {
    let mut applications = ConstraintApplicationInventory::empty();
    for member in members.iter() {
        applications
            .merge_original(
                &component.original_path(*member).unwrap().applications,
                context,
            )
            .unwrap();
    }
    JointBranch {
        lineage: Arc::clone(members),
        applications: Arc::new(applications),
        bindings: BTreeMap::new(),
        const_bindings: BTreeMap::new(),
        effects: EffectConstraintEnvironment::new_prepared(
            &[EffectConstraintVariable::new(
                input(),
                EffectConstraintEligibility::Rigid,
            )],
            &crate::effect_row::prepared::PreparedEffectPredicate::Canonical(
                EffectPredicate::unconstrained(),
            ),
            graph,
            context,
        )
        .unwrap(),
        equations: Vec::new(),
        choice_key: Vec::new(),
        deferred_cycles: DeferredCycleWitness::default(),
    }
}

#[test]
fn contradictory_joint_premises_cannot_erase_an_individual_sources_failure() {
    for invalid in [false, true] {
        let cancellation = AtomicBool::new(false);
        let mut context = context(&cancellation);
        let mut graph = PreparedEffectGraph::default();
        let mut component = Component::new(&mut context, &graph).unwrap();
        let parameter = EffectFormula::variable(input(), &mut context).unwrap();
        let first_d = parameter
            .subset(&EffectFormula::empty(), &mut context)
            .unwrap();
        let second_d = EffectFormula::literal(EffectSet::from_labels(["log.write"]).unwrap(), None)
            .subset(&parameter, &mut context)
            .unwrap();
        let members: Arc<[OriginalProductMember]> = vec![
            root(&mut component, &mut context, &mut graph, first_d, invalid),
            root(&mut component, &mut context, &mut graph, second_d, false),
        ]
        .into();
        let original = component
            .prepare_original_effects(
                &members,
                &ConstraintLexicalScope::default(),
                &mut graph,
                &mut context,
            )
            .unwrap();
        let mut branch = branch(&component, &members, graph.read(), &mut context);
        let impossible = graph.not(truth(), &mut context).unwrap();
        branch
            .effects
            .restore_predicate(&impossible, &mut graph, &mut context)
            .unwrap();
        let frozen = component
            .freeze_original_effects(
                &branch,
                &members,
                &ConstraintLexicalScope::default(),
                &original,
                &mut graph,
                &mut context,
            )
            .unwrap();
        assert_eq!(
            frozen
                .reduce(&mut context)
                .unwrap()
                .satisfiable()
                .truth_value(),
            Some(!invalid)
        );
        // Its sources are still independently retained, not rewritten by a
        // later product's joint facts.
        assert_eq!(original.sources.len(), 2);
    }
}

fn support(frozen: &ScopedEffectCompletion) -> std::collections::BTreeSet<String> {
    let mut result = std::collections::BTreeSet::new();
    let mut todo = vec![frozen.satisfiable()];
    while let Some(predicate) = todo.pop() {
        match predicate.view().unwrap() {
            EffectResidualPredicateView::And(children) => todo.extend(children),
            EffectResidualPredicateView::Not(body)
            | EffectResidualPredicateView::ExistsUniformChoices { body, .. }
            | EffectResidualPredicateView::SubstituteLower { body, .. }
            | EffectResidualPredicateView::ForAllFiniteRows { body, .. } => todo.push(body),
            EffectResidualPredicateView::ExistsLocalFunctions { body, support, .. } => {
                result.extend(support.retained.iter().map(ToString::to_string));
                todo.push(body);
            }
            EffectResidualPredicateView::Implies {
                domain,
                consequence,
            } => todo.extend([domain, consequence]),
            _ => {}
        }
    }
    result
}

#[test]
fn a_later_use_is_not_an_original_label_or_type_root() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let mut component = Component::new(&mut context, &graph).unwrap();
    let member = root(
        &mut component,
        &mut context,
        &mut graph,
        EffectPredicate::unconstrained(),
        false,
    );
    let members: Arc<[OriginalProductMember]> = vec![member].into();
    // This explicit recorded-use fixture isolates root selection. It does not
    // stand in for the use-ticket protocol or source permission acceptance.
    let original_source = component
        .original_path(member)
        .unwrap()
        .lineage
        .original
        .clone();
    component
        .insert(
            &mut context,
            RegisteredValue {
                original: original_source,
                definition: ValueDefinition::Use(Box::new([])),
                preparations: Box::new([()]),
                alternatives: Box::new([ValueAlternative {
                    origins: Box::new([]),
                    predicate: truth(),
                    preparation: 0,
                    value: ValueContent::Available {
                        ty: TypeKind::function_with_contract(
                            crate::types::GenericBinder::EMPTY,
                            truth(),
                            [],
                            TypeKind::I64,
                            PreparedEffectRow::Canonical(EffectRow::closed(
                                EffectSet::from_labels(["later.label"]).unwrap(),
                            )),
                        ),
                    },
                }]),
            },
        )
        .unwrap();
    let original = component
        .prepare_original_effects(
            &members,
            &ConstraintLexicalScope::default(),
            &mut graph,
            &mut context,
        )
        .unwrap();
    let branch = branch(&component, &members, graph.read(), &mut context);
    let frozen = component
        .freeze_original_effects(
            &branch,
            &members,
            &ConstraintLexicalScope::default(),
            &original,
            &mut graph,
            &mut context,
        )
        .unwrap();
    assert!(support(&frozen).is_empty());
}

#[test]
fn independent_bound_premises_remain_independent_in_the_joint_domain() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let mut component = Component::new(&mut context, &graph).unwrap();
    let mut members = Vec::new();
    for lower_bound in [false, true] {
        let previous = context.enter_binder_scope(crate::types::GenericBinder::new(0, 0, 1));
        let key = context.lexical_frames().shape().bound_effect(0, 0).unwrap();
        let parameter = EffectFormula::variable(key, &mut context).unwrap();
        let domain = if lower_bound {
            EffectFormula::literal(EffectSet::from_labels(["log.write"]).unwrap(), None)
                .subset(&parameter, &mut context)
                .unwrap()
        } else {
            parameter
                .subset(&EffectFormula::empty(), &mut context)
                .unwrap()
        };
        members.push(root(
            &mut component,
            &mut context,
            &mut graph,
            domain,
            false,
        ));
        context.restore_binder_scope(previous);
    }
    let members: Arc<[OriginalProductMember]> = members.into();
    let mut scope = ConstraintLexicalScope::default();
    for member in members.iter() {
        let path = component.original_path(*member).unwrap();
        scope.merge(&path.lineage.scope, &mut context).unwrap();
        path.applications
            .merge_lexical_scopes(&mut scope, &mut context)
            .unwrap();
    }
    assert_eq!(scope.shape().binders().len(), 2);
    let original = component
        .prepare_original_effects(&members, &scope, &mut graph, &mut context)
        .unwrap();
    let mut branch = branch(&component, &members, graph.read(), &mut context);
    let impossible = graph.not(truth(), &mut context).unwrap();
    branch
        .effects
        .restore_predicate(&impossible, &mut graph, &mut context)
        .unwrap();
    let frozen = component
        .freeze_original_effects(
            &branch,
            &members,
            &scope,
            &original,
            &mut graph,
            &mut context,
        )
        .unwrap();
    // The two premises are jointly satisfiable. A false R must fail; equal
    // depth/slot spelling must not conflate the frames and make D vacuous.
    assert_eq!(
        frozen
            .reduce(&mut context)
            .unwrap()
            .satisfiable()
            .truth_value(),
        Some(false)
    );
}

#[test]
fn final_forest_closure_waits_for_a_body_and_returns_the_same_forest_for_retry() {
    use crate::effect_row::prepared::PreparedEffectInvariant;
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let mut component = Component::new(&mut context, &graph).unwrap();
    let member = root(
        &mut component,
        &mut context,
        &mut graph,
        EffectPredicate::unconstrained(),
        false,
    );
    let forest = component
        .infer_original_products(&mut context, &mut graph)
        .unwrap();
    let members = Arc::clone(&forest.products[0].members);
    let retained = graph.retained_terms();
    let failure = forest.close(&[], &mut graph, &mut context).err().unwrap();
    let (error, forest) = failure.into_parts();
    assert!(matches!(
        error,
        PreparedValueFailure::Constraint(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::SchemeState
            ))
        ))
    ));
    assert!(Arc::ptr_eq(&members, &forest.products[0].members));
    assert_eq!(forest.products[0].members.as_ref(), &[member]);
    assert_eq!(graph.retained_terms(), retained);
    graph
        .prepare_body(
            &named,
            PreparedEffectRow::Canonical(EffectRow::closed(
                EffectSet::from_labels(["body.actual"]).unwrap(),
            )),
            vec![],
            &mut context,
        )
        .unwrap();
    let closed = forest.close(&[], &mut graph, &mut context).unwrap();
    assert!(Arc::ptr_eq(&members, &closed.products[0].members));
    assert_eq!(closed.products[0].result.as_ref().unwrap().len(), 1);
    assert_eq!(
        closed.products[0].result.as_ref().unwrap()[0]
            .effects
            .predicate()
            .value(),
        &EffectPredicate::unconstrained()
    );
    assert!(closed.original.original_path(member).is_ok());
    assert!(
        graph
            .prepare_body(
                &named,
                PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new(),)),
                vec![],
                &mut context
            )
            .is_err()
    );
}

#[test]
fn cancellation_does_not_consume_or_publish_a_prepared_forest() {
    use crate::types::constraints::TypeConstraintAbort;
    use std::sync::atomic::Ordering;
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let mut component = Component::new(&mut context, &graph).unwrap();
    root(
        &mut component,
        &mut context,
        &mut graph,
        EffectPredicate::unconstrained(),
        false,
    );
    let forest = component
        .infer_original_products(&mut context, &mut graph)
        .unwrap();
    let members = Arc::clone(&forest.products[0].members);
    let retained = graph.retained_terms();
    let frames = context.lexical_frames().clone();
    cancellation.store(true, Ordering::Relaxed);
    let (error, forest) = forest
        .close(&[], &mut graph, &mut context)
        .err()
        .unwrap()
        .into_parts();
    assert!(matches!(
        error,
        PreparedValueFailure::Constraint(TypeConstraintError::Abort(
            TypeConstraintAbort::Cancelled
        ))
    ));
    assert!(Arc::ptr_eq(&members, &forest.products[0].members));
    assert_eq!(graph.retained_terms(), retained);
    assert_eq!(context.lexical_frames(), &frames);
    cancellation.store(false, Ordering::Relaxed);
    let closed = forest.close(&[], &mut graph, &mut context).unwrap();
    assert!(Arc::ptr_eq(&members, &closed.products[0].members));
    assert!(closed.products[0].result.is_ok());
}

#[test]
fn actual_closed_joint_products_seal_each_application_and_canonical_projection() {
    use crate::types::constraints::{
        ConstraintClosurePolicy, TypeConstraintSolution,
        normalization::project_closed_operand,
        transaction::{
            ActiveConstraintProbe, ConstraintSourceId, PreparedConstraintSourceProjection,
            PreparedSourceOrdinal, SourceProbeObservation, StoredSourceSelection,
        },
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let mut component = Component::new(&mut context, &graph).unwrap();
    let members = [
        root(
            &mut component,
            &mut context,
            &mut graph,
            EffectPredicate::unconstrained(),
            false,
        ),
        root(
            &mut component,
            &mut context,
            &mut graph,
            EffectPredicate::unconstrained(),
            false,
        ),
    ];
    let forest = component
        .infer_original_products(&mut context, &mut graph)
        .unwrap();
    let closed = forest.close(&[], &mut graph, &mut context).unwrap();
    let product = &closed.products[0];
    let joint = &product.result.as_ref().unwrap()[0];
    assert!(Arc::ptr_eq(&joint.branch.lineage, &product.members));
    assert_eq!(joint.completed.len(), members.len());
    for (completed, member) in joint.completed.iter().zip(members) {
        assert_eq!(completed.member, member);
    }
    let frames = context.lexical_frames().clone();
    for member in members {
        let source = closed.original.original_path(member).unwrap();
        let application = source.root_id();
        let request = &source.lineage.projections[0];
        context
            .with_original_scope(product.scope.clone(), |context| {
                let source = ActiveConstraintProbe {
                    source: ConstraintSourceId::new(application, ()),
                    source_ordinal: PreparedSourceOrdinal {
                        application,
                        ordinal: 0,
                    },
                    branch: SourceProbeObservation::closed_for_test((), ()),
                    selection: StoredSourceSelection::Unchecked,
                    prepared_source_projection: PreparedConstraintSourceProjection::Scalar,
                    value_expected: None,
                    actual: context.fixture_operand(TypeKind::<PreparedEffects>::I64),
                }
                .close(&joint.branch, &joint.effects, graph.read(), context)?;
                assert_eq!(source.actual(), &TypeKind::I64);
                let solution = TypeConstraintSolution::complete_application_with_effects(
                    &joint.branch,
                    application,
                    &joint.effects,
                    graph.read(),
                    context,
                )?;
                assert!(solution.bindings().next().is_none());
                assert!(solution.effect_predicate().value().is_unconstrained());
                context.enter_node()?;
                let value = project_closed_operand(
                    request.value.view(),
                    &joint.branch,
                    ConstraintClosurePolicy::ProjectionClosed,
                    &joint.effects,
                    graph.read(),
                    context,
                )?;
                let projection = solution.reify_projection(
                    Arc::clone(&request.key),
                    &value,
                    &joint.branch,
                    application,
                    ConstraintClosurePolicy::ProjectionClosed,
                    &joint.effects,
                    graph.read(),
                    context,
                )?;
                let canonical: TypeKind = projection.to_quantified_type().unwrap();
                assert_eq!(canonical, TypeKind::I64);
                Ok::<_, TypeConstraintError>(())
            })
            .unwrap();
    }
    assert_eq!(context.lexical_frames(), &frames);
    assert!(Arc::ptr_eq(&joint.branch.lineage, &product.members));
    let joint = closed
        .products
        .into_vec()
        .remove(0)
        .result
        .unwrap()
        .into_vec()
        .remove(0);
    for completed in joint.completed.into_vec() {
        let solved = completed
            .fixture_materialization(&graph, &mut context)
            .unwrap()
            .finish(&mut context, |_, _, _| Ok(true))
            .unwrap()
            .into_unique()
            .unwrap();
        assert_eq!(solved.component.selected().projections().len(), 1);
        assert_eq!(
            solved.component.selected().projections()[0]
                .to_quantified_type()
                .unwrap(),
            TypeKind::I64
        );
    }
}
