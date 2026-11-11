use super::*;
use crate::types::constraints::{
    NoConstraintClient, TypeConstraintParameterScope, TypeConstraintProjectionClosure,
    context::{LocalConstraintAccounting, TypeConstraintLimits},
    test_support::ConstraintTestSetup,
    transaction::TypeConstraintTransaction,
};
use crate::{
    effect_row::{
        EffectRow,
        prepared::{PreparedEffectInvariant, PreparedEffectRelation, PreparedEffectRow},
    },
    effects::EffectSet,
    types::GenericBinder,
};
use std::sync::atomic::AtomicBool;

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

fn root(
    context: &mut Context<'_>,
    ty: &TypeKind,
) -> SuspendedConstraintFrontier<NoConstraintClient> {
    let mut lower = TypeConstraintTransaction::initialize(
        context,
        (),
        TypeConstraintParameterScope::empty(),
        None,
    )
    .unwrap();
    lower.request_projection(context, (), ty, TypeConstraintProjectionClosure::Closed);
    lower.suspend_result(context, ()).unwrap()
}

fn output(context: &mut Context<'_>, ty: &TypeKind) -> PreparedValueOutput<&'static str> {
    PreparedValueOutput {
        ty: ty
            .prepare_effects(&GenericScope::default(), context)
            .unwrap(),
        scope: GenericScope::default(),
        preparation: "use",
    }
}

#[test]
fn repeated_reads_are_correlated_without_copying_or_filtering_the_producer() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let roots = vec![
        root(&mut context, &TypeKind::I32),
        root(&mut context, &TypeKind::String),
    ];
    let value = component
        .admit_roots(&mut context, roots, "initializer")
        .unwrap();
    let mut use_ = component
        .issue_use(&mut context, &[value.clone(), value.clone()])
        .unwrap();
    assert!(component.has_live_uses());
    let mut observed = Vec::new();
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut use_)
        .unwrap()
    {
        let outcome = match (
            component.input(&use_, &alternative, 0).unwrap(),
            component.input(&use_, &alternative, 1).unwrap(),
        ) {
            (
                PreparedValueInput::Available { ty: left, .. },
                PreparedValueInput::Available { ty: right, .. },
            ) => {
                assert_eq!(left, right);
                observed.push(matches!(left, TypeKind::String));
                if matches!(left, TypeKind::String) {
                    PreparedValueOutcome::Accepted(output(&mut context, &TypeKind::String))
                } else {
                    PreparedValueOutcome::Rejected("member mismatch")
                }
            }
            (
                PreparedValueInput::Unavailable { reason: left, .. },
                PreparedValueInput::Unavailable { reason: right, .. },
            ) => {
                assert_eq!(left, right);
                PreparedValueOutcome::Blocked("producer diagnostic")
            }
            _ => panic!("one initializer cannot choose a different alternative for each read"),
        };
        component
            .submit_use(&mut context, &mut use_, alternative, outcome)
            .unwrap();
    }
    assert_eq!(observed, [false, true]);
    let use_ = component.finish_use(use_).unwrap();
    assert!(component.has_live_uses());
    let derived = component.retain_use(&mut context, use_).unwrap();
    assert!(!component.has_live_uses());
    assert_eq!(component.value(&value).unwrap().alternatives.len(), 4);
    assert_eq!(component.value(&derived).unwrap().alternatives.len(), 4);
    let ValueDefinition::Constraint(originals) = &component.value(&value).unwrap().definition
    else {
        panic!("the producer retains its original frontiers");
    };
    assert_eq!(originals.len(), 2);
    assert!(
        originals
            .iter()
            .all(|root| root.transaction.materialization.is_empty())
    );
}

#[test]
fn independent_origins_form_a_product_but_derived_values_keep_shared_origins() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let mut values = Vec::new();
    for _ in 0..2 {
        let roots = vec![root(&mut context, &TypeKind::I32)];
        values.push(component.admit_roots(&mut context, roots, "root").unwrap());
    }
    let mut use_ = component.issue_use(&mut context, &values).unwrap();
    let mut count = 0;
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut use_)
        .unwrap()
    {
        count += 1;
        let outcome = if alternative.blocked {
            PreparedValueOutcome::Blocked("blocked")
        } else {
            PreparedValueOutcome::Accepted(output(&mut context, &TypeKind::I32))
        };
        component
            .submit_use(&mut context, &mut use_, alternative, outcome)
            .unwrap();
    }
    assert_eq!(count, 9);
    let observed = component.finish_use(use_).unwrap();
    let derived = component.retain_use(&mut context, observed).unwrap();
    let mut use_ = component
        .issue_use(&mut context, &[values[0].clone(), derived])
        .unwrap();
    let mut correlated = 0;
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut use_)
        .unwrap()
    {
        correlated += 1;
        let outcome = if alternative.blocked {
            PreparedValueOutcome::Blocked("blocked")
        } else {
            PreparedValueOutcome::Accepted(output(&mut context, &TypeKind::I32))
        };
        component
            .submit_use(&mut context, &mut use_, alternative, outcome)
            .unwrap();
    }
    assert_eq!(correlated, 9);
}

#[test]
fn foreign_references_and_ticket_misuse_do_not_create_checked_results() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut first = Component::new(&mut context).unwrap();
    let mut second = Component::new(&mut context).unwrap();
    let value_output = output(&mut context, &TypeKind::I32);
    let value = first.admit_value(&mut context, value_output).unwrap();
    assert!(matches!(
        second.issue_use(&mut context, &[value.clone()]),
        Err(PreparedValueFailure::ForeignIssuer)
    ));
    let mut use_ = first.issue_use(&mut context, &[value]).unwrap();
    let alternative = first
        .next_use_alternative(&mut context, &mut use_)
        .unwrap()
        .unwrap();
    assert!(matches!(
        first.next_use_alternative(&mut context, &mut use_),
        Err(PreparedValueFailure::UseState)
    ));
    let accepted = PreparedValueOutcome::Accepted(output(&mut context, &TypeKind::I32));
    first
        .submit_use(&mut context, &mut use_, alternative, accepted)
        .unwrap();
    assert!(
        first
            .next_use_alternative(&mut context, &mut use_)
            .unwrap()
            .is_none()
    );
    drop(first.finish_use(use_).unwrap());
    assert!(!first.has_live_uses());
}

fn literal(labels: &[&str]) -> PreparedEffectRow {
    PreparedEffectRow::Canonical(EffectRow::closed(
        EffectSet::from_labels(labels.iter().copied()).unwrap(),
    ))
}

#[test]
fn conditional_use_keeps_rejection_without_constraining_the_producer_scheme() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let producer_output = output(&mut context, &TypeKind::I32);
    let producer = component
        .admit_value(&mut context, producer_output)
        .unwrap();
    let graph = component.effects_mut();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let recursive = graph.application(&scheme, vec![], &mut context).unwrap();
    graph
        .prepare_body(&scheme, recursive.clone(), vec![], &mut context)
        .unwrap();
    let fits = graph
        .relate(
            PreparedEffectRelation::Equal,
            recursive,
            literal(&["fs.read"]),
            &mut context,
        )
        .unwrap();
    let mut use_ = component
        .issue_use(&mut context, &[producer.clone()])
        .unwrap();
    let alternative = component
        .next_use_alternative(&mut context, &mut use_)
        .unwrap()
        .unwrap();
    let accepted = output(&mut context, &TypeKind::String);
    component
        .submit_use(
            &mut context,
            &mut use_,
            alternative,
            PreparedValueOutcome::Conditional {
                fits,
                accepted,
                rejected: "member rejected",
            },
        )
        .unwrap();
    assert!(
        component
            .next_use_alternative(&mut context, &mut use_)
            .unwrap()
            .is_none()
    );
    let observed = component.finish_use(use_).unwrap();
    let derived = component.retain_use(&mut context, observed).unwrap();
    let alternatives = &component.value(&derived).unwrap().alternatives;
    assert_eq!(alternatives.len(), 2);
    assert_eq!(alternatives[0].origins, alternatives[1].origins);
    assert!(matches!(
        alternatives[0].value,
        ValueContent::Available { .. }
    ));
    assert!(matches!(
        alternatives[1].value,
        ValueContent::Unavailable(PreparedValueUnavailable::RejectedUse)
    ));
    assert!(matches!(
        component.value(&producer).unwrap().alternatives[0].value,
        ValueContent::Available {
            ty: TypeKind::I32,
            ..
        }
    ));
    // Requiring the successful member use would instead force fs.read into
    // S = S. Its least solution must remain empty, with the use rejected.
    let closed = component.effects.close(&mut context).unwrap();
    assert!(closed.scheme(&scheme).unwrap().is_empty());
}

#[test]
fn failed_retention_rolls_back_effect_terms_and_publishes_no_value() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let roots = vec![
        root(&mut context, &TypeKind::I32),
        root(&mut context, &TypeKind::String),
    ];
    let producer = component
        .admit_roots(&mut context, roots, "producer")
        .unwrap();
    let local_fits = component
        .effects
        .not(Component::always(), &mut context)
        .unwrap();
    let foreign_fits = PreparedEffectGraph::default()
        .not(Component::always(), &mut context)
        .unwrap();
    let mut use_ = component.issue_use(&mut context, &[producer]).unwrap();
    let mut fits = [local_fits, foreign_fits].into_iter();
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut use_)
        .unwrap()
    {
        let outcome = if alternative.blocked {
            PreparedValueOutcome::Blocked("blocked")
        } else {
            PreparedValueOutcome::Conditional {
                fits: fits.next().unwrap(),
                accepted: output(&mut context, &TypeKind::I32),
                rejected: "rejected",
            }
        };
        component
            .submit_use(&mut context, &mut use_, alternative, outcome)
            .unwrap();
    }
    let observed = component.finish_use(use_).unwrap();
    let before_terms = component.effects.retained_terms();
    let before_values = component.values.len();
    let before_ordinal = component.next_value;
    assert!(matches!(
        component.retain_use(&mut context, observed),
        Err(PreparedValueFailure::Effect(
            PreparedEffectFailure::Invariant(PreparedEffectInvariant::ForeignIssuer)
        ))
    ));
    assert_eq!(component.effects.retained_terms(), before_terms);
    assert_eq!(component.values.len(), before_values);
    assert_eq!(component.next_value, before_ordinal);
    assert!(!component.has_live_uses());
}

fn arrow(row: PreparedEffectRow) -> PreparedValueOutput<&'static str> {
    PreparedValueOutput {
        ty: TypeKind::Function {
            binder: GenericBinder::EMPTY,
            predicate: Component::always(),
            params: vec![],
            return_type: Box::new(TypeKind::I32),
            effects: row,
        },
        scope: GenericScope::default(),
        preparation: "function value",
    }
}

#[test]
fn retained_function_values_preserve_unknown_annotations_and_reject_foreign_or_stale_terms() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let value = component
        .admit_value(
            &mut context,
            arrow(PreparedEffectRow::Canonical(EffectRow::unknown())),
        )
        .unwrap();
    assert!(matches!(
        &component.value(&value).unwrap().alternatives[0].value,
        ValueContent::Available {
            ty: TypeKind::Function { effects: PreparedEffectRow::Canonical(row), .. }, ..
        } if row == &EffectRow::unknown()
    ));
    let foreign = PreparedEffectGraph::default()
        .union(vec![], &mut context)
        .unwrap();
    assert!(matches!(
        component.admit_value(&mut context, arrow(foreign)),
        Err(PreparedValueFailure::Effect(
            PreparedEffectFailure::Invariant(PreparedEffectInvariant::ForeignIssuer)
        ))
    ));
    let mut checkpoint = component.effects.checkpoint(&mut context).unwrap();
    let stale = component.effects.union(vec![], &mut context).unwrap();
    component.effects.rollback(&mut checkpoint).unwrap();
    assert!(matches!(
        component.admit_value(&mut context, arrow(stale)),
        Err(PreparedValueFailure::Effect(
            PreparedEffectFailure::Invariant(PreparedEffectInvariant::StaleReference)
        ))
    ));
    assert_eq!(component.values.len(), 1);
}

#[test]
fn empty_input_use_runs_once_and_a_failed_submission_cannot_be_resumed() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let mut use_ = component.issue_use(&mut context, &[]).unwrap();
    let alternative = component
        .next_use_alternative(&mut context, &mut use_)
        .unwrap()
        .unwrap();
    assert!(matches!(
        component.submit_use(
            &mut context,
            &mut use_,
            alternative,
            PreparedValueOutcome::Blocked("invalid")
        ),
        Err(PreparedValueFailure::InputState)
    ));
    assert!(matches!(
        component.next_use_alternative(&mut context, &mut use_),
        Err(PreparedValueFailure::UseState)
    ));
    assert!(matches!(
        component.finish_use(use_),
        Err(PreparedValueFailure::UseState)
    ));
    assert!(!component.has_live_uses());
    let mut use_ = component.issue_use(&mut context, &[]).unwrap();
    let alternative = component
        .next_use_alternative(&mut context, &mut use_)
        .unwrap()
        .unwrap();
    let accepted = output(&mut context, &TypeKind::I32);
    component
        .submit_use(
            &mut context,
            &mut use_,
            alternative,
            PreparedValueOutcome::Accepted(accepted),
        )
        .unwrap();
    assert!(
        component
            .next_use_alternative(&mut context, &mut use_)
            .unwrap()
            .is_none()
    );
    let observed = component.finish_use(use_).unwrap();
    let value = component.retain_use(&mut context, observed).unwrap();
    assert_eq!(component.value(&value).unwrap().alternatives.len(), 1);
}

#[test]
fn fixed_component_allocations_and_cancelled_enumeration_obey_the_original_budget() {
    use crate::types::constraints::TypeConstraintError;
    use std::sync::atomic::Ordering;

    let cancellation = AtomicBool::new(false);
    for (nodes, succeeds) in [(2, false), (3, true)] {
        let mut bounded =
            ConstraintTestSetup::new(TypeConstraintLimits::new(100, nodes, 16, 16), &cancellation)
                .into_parts()
                .0;
        let result = Component::new(&mut bounded);
        if succeeds {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(PreparedValueFailure::Constraint(
                    TypeConstraintError::Abort(_)
                ))
            ));
        }
    }
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context).unwrap();
    let producer_output = output(&mut context, &TypeKind::I32);
    let producer = component
        .admit_value(&mut context, producer_output)
        .unwrap();
    let mut use_ = component.issue_use(&mut context, &[producer]).unwrap();
    cancellation.store(true, Ordering::Relaxed);
    assert!(matches!(
        component.next_use_alternative(&mut context, &mut use_),
        Err(PreparedValueFailure::Constraint(
            TypeConstraintError::Abort(_)
        ))
    ));
    cancellation.store(false, Ordering::Relaxed);
    assert!(matches!(
        component.next_use_alternative(&mut context, &mut use_),
        Err(PreparedValueFailure::UseState)
    ));
    drop(use_);
    assert!(!component.has_live_uses());
    assert_eq!(component.values.len(), 1);
}

#[test]
fn aliases_do_not_copy_affine_preparation_facts() {
    use std::cell::Cell;
    struct Preparation(Rc<Cell<usize>>);
    impl Drop for Preparation {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component =
        OpenConstraintComponent::<NoConstraintClient, Preparation>::new(&mut context).unwrap();
    let dropped = Rc::new(Cell::new(0));
    let producer = component
        .admit_value(
            &mut context,
            PreparedValueOutput {
                ty: TypeKind::I32,
                scope: GenericScope::default(),
                preparation: Preparation(Rc::clone(&dropped)),
            },
        )
        .unwrap();
    let mut use_ = component
        .issue_use(&mut context, &[producer.clone(), producer])
        .unwrap();
    let alternative = component
        .next_use_alternative(&mut context, &mut use_)
        .unwrap()
        .unwrap();
    let (
        PreparedValueInput::Available {
            preparation: left, ..
        },
        PreparedValueInput::Available {
            preparation: right, ..
        },
    ) = (
        component.input(&use_, &alternative, 0).unwrap(),
        component.input(&use_, &alternative, 1).unwrap(),
    )
    else {
        panic!("both aliases read the original prepared facts");
    };
    assert!(std::ptr::eq(left, right));
    component
        .submit_use(
            &mut context,
            &mut use_,
            alternative,
            PreparedValueOutcome::Accepted(PreparedValueOutput {
                ty: TypeKind::I32,
                scope: GenericScope::default(),
                preparation: Preparation(Rc::clone(&dropped)),
            }),
        )
        .unwrap();
    assert!(
        component
            .next_use_alternative(&mut context, &mut use_)
            .unwrap()
            .is_none()
    );
    let observed = component.finish_use(use_).unwrap();
    let _derived = component.retain_use(&mut context, observed).unwrap();
    assert_eq!(dropped.get(), 0);
    drop(component);
    assert_eq!(dropped.get(), 2);
}

struct Applications;
impl ConstraintDomain for Applications {
    type Application = u32;
    type Source = ();
    type AlternativeIndex = ();
    type EvidenceRule = ();
    type ObservedEvidence = ();
    type CheckedEvidence = ();
    type ProbeSemanticBranch = ();
    type SealedBranchValue = ();
    type Projection = ();
    type SourceErrorCause = ();
    type ClientInvariant = ();
    fn evidence_accepts((): &(), (): &()) -> bool {
        true
    }
    fn project_checked_evidence((): &(), _: &TypeKind) -> Option<()> {
        Some(())
    }
    fn alternative_ordinal((): &()) -> u32 {
        0
    }
    fn client_invariant_source((): &()) {}
    fn empty_sealed_branch() {}
}

#[test]
fn producer_registration_preserves_parent_correlation_and_one_semantic_application() {
    use crate::types::constraints::{
        ConstraintAcceptance, PreparedConstraintSourceProjection, PreparedSourceConstraint,
    };

    let cancellation = AtomicBool::new(false);
    let mut context = ConstraintTestSetup::<_, Applications>::new(
        TypeConstraintLimits::new(65536, 65536, 1024, 1024),
        &cancellation,
    )
    .into_parts()
    .0;
    let mut component = OpenConstraintComponent::<Applications, ()>::new(&mut context).unwrap();
    let mut roots = Vec::new();
    for application in [0, 1] {
        let mut root = TypeConstraintTransaction::initialize(
            &mut context,
            application,
            TypeConstraintParameterScope::empty(),
            None,
        )
        .unwrap();
        root.request_projection(
            &mut context,
            (),
            &TypeKind::I32,
            TypeConstraintProjectionClosure::Closed,
        );
        roots.push(root.suspend_result(&mut context, ()).unwrap());
    }
    assert!(matches!(
        component.admit_roots(&mut context, roots, ()),
        Err(PreparedValueFailure::ProducerApplication)
    ));
    let mut parent = TypeConstraintTransaction::initialize(
        &mut context,
        2,
        TypeConstraintParameterScope::empty(),
        None,
    )
    .unwrap();
    parent
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::unchecked((), PreparedConstraintSourceProjection::Scalar),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .unwrap();
    let ticket = parent.next_probe(&mut context).unwrap().unwrap();
    let nested = ticket.fork_for_child(&mut context).unwrap();
    let mut child = TypeConstraintTransaction::initialize_from_nested_path(
        &mut context,
        3,
        TypeConstraintParameterScope::empty(),
        None,
        nested,
    )
    .unwrap();
    child.request_projection(
        &mut context,
        (),
        &TypeKind::I32,
        TypeConstraintProjectionClosure::Closed,
    );
    let child = child.suspend_result(&mut context, ()).unwrap();
    assert!(matches!(
        component.admit_roots(&mut context, vec![child], ()),
        Err(PreparedValueFailure::DependentRoot)
    ));
    assert!(component.values.is_empty());
    assert_eq!(component.next_value, 0);
}
