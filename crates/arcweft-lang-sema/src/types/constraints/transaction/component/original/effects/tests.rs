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
    transaction.request_projection(
        context,
        (),
        &TypeKind::<PreparedEffects>::I64,
        TypeConstraintProjectionClosure::Closed,
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
            &EffectPredicate::unconstrained(),
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
            .prepare_original_effects(&members, &mut graph, &mut context)
            .unwrap();
        let mut branch = branch(&component, &members, &mut context);
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
        .prepare_original_effects(&members, &mut graph, &mut context)
        .unwrap();
    let branch = branch(&component, &members, &mut context);
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
