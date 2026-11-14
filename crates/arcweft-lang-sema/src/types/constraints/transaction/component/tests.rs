use super::*;
use crate::types::constraints::{
    ConstraintAcceptance, NoConstraintClient, TypeConstraintParameterScope,
    TypeConstraintProjectionClosure,
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
    let mut lower = TypeConstraintTransaction::initialize_fixture(
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
    let original = context.fixture_original_context(None).unwrap();
    PreparedValueOutput {
        ty: ty.prepare_effects(original.scope(), context).unwrap(),
        original,
        preparation: "use",
    }
}

#[test]
fn original_source_context_is_retained_and_current_registration_scope_cannot_replace_it() {
    let effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let binder = GenericBinder::new(1, 0, 0);
    let (original, first, second) = context
        .with_binder(binder, |context| {
            let original = context.fixture_original_context(None)?;
            let ty = TypeKind::GenericParam(context.lexical_scope().bound_type(0, 0).unwrap());
            let parameter = crate::types::GenericTypeParameterId::new(
                crate::types::GenericParameterOwnerId::Detached(
                    crate::types::DetachedGenericOwnerId::new(921),
                ),
                0,
            );
            let template = TypeKind::generic_parameter(parameter.clone());
            let make = |context: &mut Context<'_>| {
                let mut transaction = TypeConstraintTransaction::initialize(
                    context,
                    original.clone(),
                    (),
                    TypeConstraintParameterScope::new([(
                        parameter.clone(),
                        crate::types::constraints::TypeConstraintParameterEligibility::Bindable,
                    )])
                    .unwrap(),
                    None,
                )
                .unwrap();
                transaction.constrain(
                    context,
                    &template,
                    &ty,
                    ConstraintAcceptance::PatternAcceptsActual,
                );
                transaction.request_projection(
                    context,
                    (),
                    &template,
                    TypeConstraintProjectionClosure::Closed,
                );
                let suspended = transaction.suspend_result(context, ()).unwrap();
                assert_eq!(suspended.original_context(), &original);
                let resumed = suspended.resume();
                let suspended = resumed.suspend_result(context, ()).unwrap();
                assert_eq!(suspended.original_context(), &original);
                suspended
            };
            Ok((original.clone(), make(context), make(context)))
        })
        .unwrap();
    context
        .with_binder(binder, |context| {
            let unrelated = context.fixture_original_context(None)?;
            assert_eq!(unrelated.scope(), original.scope());
            assert_ne!(unrelated, original);
            assert!(matches!(
                component.admit_roots(context, unrelated, vec![first], "foreign"),
                Err(PreparedValueFailure::ProducerContext)
            ));
            assert!(component.values.is_empty());
            assert_eq!(component.next_value, 0);
            let value = component
                .admit_roots(context, original.clone(), vec![second], "original")
                .unwrap();
            let retained = component.value(&value).unwrap();
            assert_eq!(retained.original, original);
            let ValueContent::Available { ty } = &retained.alternatives[0].value else {
                panic!("the original scoped type remains available")
            };
            assert_eq!(
                ty,
                &TypeKind::GenericParam(original.scope().bound_type(0, 0).unwrap())
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn original_source_context_is_required_even_when_no_candidate_frontier_exists() {
    let effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let original = context.fixture_original_context(None).unwrap();
    let mut component = Component::new(&mut context, &effects).unwrap();
    let value = component
        .admit_roots(&mut context, original.clone(), Vec::new(), "diagnostic")
        .unwrap();
    let retained = component.value(&value).unwrap();
    let ValueDefinition::Constraint { frontiers } = &retained.definition else {
        panic!("retained original diagnostic")
    };
    assert!(frontiers.is_empty());
    assert_eq!(retained.original, original);
    assert_eq!(retained.alternatives.len(), 2);
    assert!(
        retained
            .alternatives
            .iter()
            .all(|alternative| matches!(alternative.value, ValueContent::Unavailable(_)))
    );
}

#[test]
fn repeated_reads_are_correlated_without_copying_or_filtering_the_producer() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let roots = vec![
        root(&mut context, &TypeKind::I32),
        root(&mut context, &TypeKind::String),
    ];
    let value = component
        .admit_fixture_roots(&mut context, roots, "initializer")
        .unwrap();
    let mut use_ = component
        .issue_fixture_use(&mut context, &[value.clone(), value.clone()])
        .unwrap();
    assert!(component.has_live_uses());
    let mut observed = Vec::new();
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
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
    let derived = component
        .retain_use(&mut context, &mut effects, use_)
        .unwrap();
    assert!(!component.has_live_uses());
    assert_eq!(component.value(&value).unwrap().alternatives.len(), 4);
    assert_eq!(component.value(&derived).unwrap().alternatives.len(), 4);
    let ValueDefinition::Constraint {
        frontiers: originals,
        ..
    } = &component.value(&value).unwrap().definition
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
fn intrinsic_values_retain_their_actual_type_const_and_effect_frames() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let binder = GenericBinder::new(1, 1, 1);
    let (original, output) = context
        .with_binder(binder, |context| {
            let original = context.fixture_original_context(None)?;
            let ty = TypeKind::function_with_effects(
                [TypeKind::Array {
                    item: Box::new(TypeKind::GenericParam(
                        original.scope().bound_type(0, 0).unwrap(),
                    )),
                    len: ArrayLength::Generic(original.scope().bound_const(0, 0).unwrap()),
                }],
                TypeKind::Bool,
                EffectRow::open(
                    EffectSet::new(),
                    original.scope().bound_effect(0, 0).unwrap(),
                ),
            );
            let output = PreparedValueOutput {
                ty: ty.prepare_effects(original.scope(), context).unwrap(),
                original: original.clone(),
                preparation: "intrinsic",
            };
            Ok((original, output))
        })
        .unwrap();
    let mut component = Component::new(&mut context, &effects).unwrap();
    context
        .with_binder(binder, |context| {
            let unrelated = context.fixture_original_context(None)?;
            assert_eq!(unrelated.scope(), original.scope());
            assert_ne!(unrelated, original);
            let value = component
                .admit_value(context, effects.read(), output)
                .unwrap();
            assert_eq!(component.value(&value).unwrap().original, original);
            let mut use_ = component.issue_use(context, unrelated, &[value]).unwrap();
            let alternative = component
                .next_use_alternative(context, &mut effects, &mut use_)
                .unwrap()
                .unwrap();
            let input = component.input(&use_, &alternative, 0).unwrap();
            assert_eq!(input.original_context(), &original);
            let PreparedValueInput::Available {
                ty:
                    TypeKind::Function {
                        params,
                        effects: retained_effects,
                        ..
                    },
                ..
            } = input
            else {
                panic!("the original intrinsic function remains available")
            };
            assert_eq!(
                params,
                &[TypeKind::Array {
                    item: Box::new(TypeKind::GenericParam(
                        original.scope().bound_type(0, 0).unwrap()
                    )),
                    len: ArrayLength::Generic(original.scope().bound_const(0, 0).unwrap()),
                }]
            );
            assert_eq!(
                retained_effects,
                &PreparedEffectRow::Canonical(EffectRow::open(
                    EffectSet::new(),
                    original.scope().bound_effect(0, 0).unwrap(),
                ))
            );
            component
                .submit_use(
                    context,
                    &mut use_,
                    alternative,
                    PreparedValueOutcome::Rejected("use outcome"),
                )
                .unwrap();
            assert!(
                component
                    .next_use_alternative(context, &mut effects, &mut use_)
                    .unwrap()
                    .is_none()
            );
            let observed = component.finish_use(use_).unwrap();
            drop(observed);
            Ok(())
        })
        .unwrap();
}

#[test]
fn accepted_and_conditional_use_outputs_cannot_substitute_an_equal_shaped_foreign_context() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let binder = GenericBinder::new(1, 0, 0);
    let original = context
        .with_binder(binder, |context| context.fixture_original_context(None))
        .unwrap();
    let foreign = context
        .with_binder(binder, |context| context.fixture_original_context(None))
        .unwrap();
    assert_eq!(original.scope(), foreign.scope());
    assert_ne!(original, foreign);
    let mut component = Component::new(&mut context, &effects).unwrap();
    for conditional in [false, true] {
        let mut use_ = component
            .issue_use(&mut context, original.clone(), &[])
            .unwrap();
        let alternative = component
            .next_use_alternative(&mut context, &mut effects, &mut use_)
            .unwrap()
            .unwrap();
        let accepted = PreparedValueOutput {
            ty: TypeKind::I32,
            original: foreign.clone(),
            preparation: "foreign",
        };
        let outcome = if conditional {
            PreparedValueOutcome::Conditional {
                fits: Component::always(),
                accepted,
                rejected: "rejected",
            }
        } else {
            PreparedValueOutcome::Accepted(accepted)
        };
        assert!(matches!(
            component.submit_use(&mut context, &mut use_, alternative, outcome),
            Err(PreparedValueFailure::UseContext)
        ));
        assert!(use_.outputs.is_empty());
        assert_eq!(component.next_value, 0);
        assert!(component.values.is_empty());
        assert!(matches!(
            component.finish_use(use_),
            Err(PreparedValueFailure::UseState)
        ));
    }
}

#[test]
fn derived_values_keep_the_use_context_for_both_available_and_diagnostic_outcomes() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let original = context
        .with_binder(GenericBinder::new(1, 0, 0), |context| {
            context.fixture_original_context(None)
        })
        .unwrap();
    let mut component = Component::new(&mut context, &effects).unwrap();
    for accepted in [true, false] {
        let mut use_ = component
            .issue_use(&mut context, original.clone(), &[])
            .unwrap();
        let alternative = component
            .next_use_alternative(&mut context, &mut effects, &mut use_)
            .unwrap()
            .unwrap();
        let outcome = if accepted {
            PreparedValueOutcome::Accepted(PreparedValueOutput {
                ty: TypeKind::Bool,
                original: original.clone(),
                preparation: "accepted",
            })
        } else {
            PreparedValueOutcome::Rejected("diagnostic")
        };
        component
            .submit_use(&mut context, &mut use_, alternative, outcome)
            .unwrap();
        assert!(
            component
                .next_use_alternative(&mut context, &mut effects, &mut use_)
                .unwrap()
                .is_none()
        );
        let observed = component.finish_use(use_).unwrap();
        let value = component
            .retain_use(&mut context, &mut effects, observed)
            .unwrap();
        assert_eq!(component.value(&value).unwrap().original, original);
        let reader = context.fixture_original_context(None).unwrap();
        let mut use_ = component.issue_use(&mut context, reader, &[value]).unwrap();
        let alternative = component
            .next_use_alternative(&mut context, &mut effects, &mut use_)
            .unwrap()
            .unwrap();
        let input = component.input(&use_, &alternative, 0).unwrap();
        assert_eq!(input.original_context(), &original);
        assert_eq!(
            matches!(input, PreparedValueInput::Available { .. }),
            accepted
        );
        drop(use_);
    }
}

#[test]
fn independent_origins_form_a_product_but_derived_values_keep_shared_origins() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let mut values = Vec::new();
    for _ in 0..2 {
        let roots = vec![root(&mut context, &TypeKind::I32)];
        values.push(
            component
                .admit_fixture_roots(&mut context, roots, "root")
                .unwrap(),
        );
    }
    let mut use_ = component.issue_fixture_use(&mut context, &values).unwrap();
    let mut count = 0;
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
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
    let derived = component
        .retain_use(&mut context, &mut effects, observed)
        .unwrap();
    let mut use_ = component
        .issue_fixture_use(&mut context, &[values[0].clone(), derived])
        .unwrap();
    let mut correlated = 0;
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
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
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut first = Component::new(&mut context, &effects).unwrap();
    let mut second = Component::new(&mut context, &effects).unwrap();
    let value_output = output(&mut context, &TypeKind::I32);
    let value = first
        .admit_value(&mut context, effects.read(), value_output)
        .unwrap();
    assert!(matches!(
        second.issue_fixture_use(&mut context, &[value.clone()]),
        Err(PreparedValueFailure::ForeignIssuer)
    ));
    let mut use_ = first.issue_fixture_use(&mut context, &[value]).unwrap();
    let alternative = first
        .next_use_alternative(&mut context, &mut effects, &mut use_)
        .unwrap()
        .unwrap();
    assert!(matches!(
        first.next_use_alternative(&mut context, &mut effects, &mut use_),
        Err(PreparedValueFailure::UseState)
    ));
    let accepted = PreparedValueOutcome::Accepted(output(&mut context, &TypeKind::I32));
    first
        .submit_use(&mut context, &mut use_, alternative, accepted)
        .unwrap();
    assert!(
        first
            .next_use_alternative(&mut context, &mut effects, &mut use_)
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
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let producer_output = output(&mut context, &TypeKind::I32);
    let producer = component
        .admit_value(&mut context, effects.read(), producer_output)
        .unwrap();
    let graph = &mut effects;
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
        .issue_fixture_use(&mut context, &[producer.clone()])
        .unwrap();
    let alternative = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
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
            .next_use_alternative(&mut context, &mut effects, &mut use_)
            .unwrap()
            .is_none()
    );
    let observed = component.finish_use(use_).unwrap();
    let derived = component
        .retain_use(&mut context, &mut effects, observed)
        .unwrap();
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
    let closed = effects.solve_principal(&mut context).unwrap();
    assert!(closed.scheme(&scheme).unwrap().is_empty());
}

#[test]
fn failed_retention_rolls_back_effect_terms_and_publishes_no_value() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let roots = vec![
        root(&mut context, &TypeKind::I32),
        root(&mut context, &TypeKind::String),
    ];
    let producer = component
        .admit_fixture_roots(&mut context, roots, "producer")
        .unwrap();
    let local_fits = effects.not(Component::always(), &mut context).unwrap();
    let foreign_fits = PreparedEffectGraph::default()
        .not(Component::always(), &mut context)
        .unwrap();
    let mut use_ = component
        .issue_fixture_use(&mut context, &[producer])
        .unwrap();
    let mut fits = [local_fits, foreign_fits].into_iter();
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
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
    let before_terms = effects.retained_terms();
    let before_values = component.values.len();
    let before_ordinal = component.next_value;
    assert!(matches!(
        component.retain_use(&mut context, &mut effects, observed),
        Err(PreparedValueFailure::Effect(
            PreparedEffectFailure::Invariant(PreparedEffectInvariant::ForeignIssuer)
        ))
    ));
    assert_eq!(effects.retained_terms(), before_terms);
    assert_eq!(component.values.len(), before_values);
    assert_eq!(component.next_value, before_ordinal);
    assert!(!component.has_live_uses());
}

fn arrow(
    original: OriginalProducerContext,
    row: PreparedEffectRow,
) -> PreparedValueOutput<&'static str> {
    PreparedValueOutput {
        ty: TypeKind::Function {
            binder: GenericBinder::EMPTY,
            predicate: Component::always(),
            params: vec![],
            return_type: Box::new(TypeKind::I32),
            effects: row,
        },
        original,
        preparation: "function value",
    }
}

#[test]
fn retained_function_values_preserve_unknown_annotations_and_reject_foreign_or_stale_terms() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let original = context.fixture_original_context(None).unwrap();
    let value = component
        .admit_value(
            &mut context,
            effects.read(),
            arrow(
                original.clone(),
                PreparedEffectRow::Canonical(EffectRow::unknown()),
            ),
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
        component.admit_value(
            &mut context,
            effects.read(),
            arrow(original.clone(), foreign)
        ),
        Err(PreparedValueFailure::Effect(
            PreparedEffectFailure::Invariant(PreparedEffectInvariant::ForeignIssuer)
        ))
    ));
    let mut checkpoint = effects.checkpoint(&mut context).unwrap();
    let stale = effects.union(vec![], &mut context).unwrap();
    effects.rollback(&mut checkpoint).unwrap();
    assert!(matches!(
        component.admit_value(&mut context, effects.read(), arrow(original, stale)),
        Err(PreparedValueFailure::Effect(
            PreparedEffectFailure::Invariant(PreparedEffectInvariant::StaleReference)
        ))
    ));
    assert_eq!(component.values.len(), 1);
}

#[test]
fn separate_value_components_share_the_actual_declaration_graph_without_sharing_value_issuers() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut effects = PreparedEffectGraph::default();
    let scheme = effects
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let row = effects.application(&scheme, vec![], &mut context).unwrap();
    let original = context.fixture_original_context(None).unwrap();
    let mut first = Component::new(&mut context, &effects).unwrap();
    let mut second = Component::new(&mut context, &effects).unwrap();
    let first_value = first
        .admit_value(
            &mut context,
            effects.read(),
            arrow(original.clone(), row.clone()),
        )
        .unwrap();
    let second_value = second
        .admit_value(&mut context, effects.read(), arrow(original.clone(), row))
        .unwrap();
    assert!(matches!(
        second.issue_fixture_use(&mut context, &[first_value.clone()]),
        Err(PreparedValueFailure::ForeignIssuer)
    ));
    let foreign = PreparedEffectGraph::default();
    assert!(matches!(
        first.admit_value(&mut context, foreign.read(), arrow(original, literal(&[]))),
        Err(PreparedValueFailure::Effect(
            PreparedEffectFailure::Invariant(PreparedEffectInvariant::ForeignIssuer)
        ))
    ));
    assert_eq!(first.values.len(), 1);
    effects
        .prepare_body(&scheme, literal(&["fs.read"]), vec![], &mut context)
        .unwrap();
    let closed = effects.solve_principal(&mut context).unwrap();
    for (component, value) in [(&first, &first_value), (&second, &second_value)] {
        let ValueContent::Available {
            ty: TypeKind::Function { effects, .. },
        } = &component.value(value).unwrap().alternatives[0].value
        else {
            panic!("expected a retained function value")
        };
        assert_eq!(
            closed.resolve_row(effects, &mut context).unwrap(),
            EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
        );
    }
}

#[test]
fn extracted_function_value_admission_retains_its_original_affine_preparation_until_real_closure() {
    use crate::effect_row::prepared::PreparedEffectDelta;

    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut effects = PreparedEffectGraph::default();
    let mut checkpoint = effects.checkpoint(&mut context).unwrap();
    let scheme = effects
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let row = effects.application(&scheme, vec![], &mut context).unwrap();
    let predicate = effects
        .relate(
            PreparedEffectRelation::Equal,
            row.clone(),
            literal(&["fs.read"]),
            &mut context,
        )
        .unwrap();
    effects
        .add_scoped_obligation(
            vec![],
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            predicate.clone(),
            &mut context,
        )
        .unwrap();
    let preparation = Arc::new(
        effects
            .extract_delta(&mut checkpoint, &mut context)
            .unwrap(),
    );
    let mut component =
        OpenConstraintComponent::<NoConstraintClient, Arc<PreparedEffectDelta>>::new(
            &mut context,
            &effects,
        )
        .unwrap();
    let original = context.fixture_original_context(None).unwrap();
    let output = PreparedValueOutput {
        ty: TypeKind::Function {
            binder: GenericBinder::EMPTY,
            predicate,
            params: vec![],
            return_type: Box::new(TypeKind::I32),
            effects: row.clone(),
        },
        original: original.clone(),
        preparation: Arc::clone(&preparation),
    };
    let view = effects.read_extracted(&preparation, &mut context).unwrap();
    let value = component.admit_value(&mut context, view, output).unwrap();
    assert_eq!(Arc::strong_count(&preparation), 2);
    assert_eq!(effects.retained_terms(), (0, 0));
    assert_eq!(
        effects.validate_retained_row(&row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    let ValueContent::Available { ty } = &component.value(&value).unwrap().alternatives[0].value
    else {
        panic!("expected prepared function value")
    };
    let retained = ty.clone();
    assert_eq!(component.value(&value).unwrap().original, original);
    drop(component);
    assert_eq!(Arc::strong_count(&preparation), 1);
    let delta =
        Arc::try_unwrap(preparation).expect("only the original publication payload remains");
    effects.restore_delta(delta, &mut context).unwrap();
    effects
        .prepare_body(&scheme, literal(&["fs.read"]), vec![], &mut context)
        .unwrap();
    let principal = effects.solve_principal(&mut context).unwrap();
    let canonical = retained
        .project_canonical_effects(original.scope(), &principal, &mut context)
        .unwrap();
    assert_eq!(
        canonical,
        TypeKind::Function {
            binder: GenericBinder::EMPTY,
            predicate: EffectPredicate::unconstrained(),
            params: vec![],
            return_type: Box::new(TypeKind::I32),
            effects: EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap()),
        }
    );
}

#[test]
fn empty_input_use_runs_once_and_a_failed_submission_cannot_be_resumed() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let mut use_ = component.issue_fixture_use(&mut context, &[]).unwrap();
    let alternative = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
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
        component.next_use_alternative(&mut context, &mut effects, &mut use_),
        Err(PreparedValueFailure::UseState)
    ));
    assert!(matches!(
        component.finish_use(use_),
        Err(PreparedValueFailure::UseState)
    ));
    assert!(!component.has_live_uses());
    let mut use_ = component.issue_fixture_use(&mut context, &[]).unwrap();
    let alternative = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
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
            .next_use_alternative(&mut context, &mut effects, &mut use_)
            .unwrap()
            .is_none()
    );
    let observed = component.finish_use(use_).unwrap();
    let value = component
        .retain_use(&mut context, &mut effects, observed)
        .unwrap();
    assert_eq!(component.value(&value).unwrap().alternatives.len(), 1);
}

#[test]
fn fixed_component_allocations_and_cancelled_enumeration_obey_the_original_budget() {
    let mut effects = PreparedEffectGraph::default();
    use crate::types::constraints::TypeConstraintError;
    use std::sync::atomic::Ordering;

    let cancellation = AtomicBool::new(false);
    for (nodes, succeeds) in [(2, false), (3, true)] {
        let mut bounded =
            ConstraintTestSetup::new(TypeConstraintLimits::new(100, nodes, 16, 16), &cancellation)
                .into_parts()
                .0;
        let result = Component::new(&mut bounded, &effects);
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
    let mut component = Component::new(&mut context, &effects).unwrap();
    let producer_output = output(&mut context, &TypeKind::I32);
    let producer = component
        .admit_value(&mut context, effects.read(), producer_output)
        .unwrap();
    let mut use_ = component
        .issue_fixture_use(&mut context, &[producer])
        .unwrap();
    cancellation.store(true, Ordering::Relaxed);
    assert!(matches!(
        component.next_use_alternative(&mut context, &mut effects, &mut use_),
        Err(PreparedValueFailure::Constraint(
            TypeConstraintError::Abort(_)
        ))
    ));
    cancellation.store(false, Ordering::Relaxed);
    assert!(matches!(
        component.next_use_alternative(&mut context, &mut effects, &mut use_),
        Err(PreparedValueFailure::UseState)
    ));
    drop(use_);
    assert!(!component.has_live_uses());
    assert_eq!(component.values.len(), 1);
}

#[test]
fn aliases_do_not_copy_affine_preparation_facts() {
    let mut effects = PreparedEffectGraph::default();
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
        OpenConstraintComponent::<NoConstraintClient, Preparation>::new(&mut context, &effects)
            .unwrap();
    let dropped = Rc::new(Cell::new(0));
    let original = context.fixture_original_context(None).unwrap();
    let producer = component
        .admit_value(
            &mut context,
            effects.read(),
            PreparedValueOutput {
                ty: TypeKind::I32,
                original: original.clone(),
                preparation: Preparation(Rc::clone(&dropped)),
            },
        )
        .unwrap();
    let mut use_ = component
        .issue_fixture_use(&mut context, &[producer.clone(), producer])
        .unwrap();
    let alternative = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
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
                original,
                preparation: Preparation(Rc::clone(&dropped)),
            }),
        )
        .unwrap();
    assert!(
        component
            .next_use_alternative(&mut context, &mut effects, &mut use_)
            .unwrap()
            .is_none()
    );
    let observed = component.finish_use(use_).unwrap();
    let _derived = component
        .retain_use(&mut context, &mut effects, observed)
        .unwrap();
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
    type ProbePreparation = ();
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
    let effects = PreparedEffectGraph::default();
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
    let mut component =
        OpenConstraintComponent::<Applications, ()>::new(&mut context, &effects).unwrap();
    let mut roots = Vec::new();
    for application in [0, 1] {
        let mut root = TypeConstraintTransaction::initialize_fixture(
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
        component.admit_fixture_roots(&mut context, roots, ()),
        Err(PreparedValueFailure::ProducerApplication)
    ));
    let mut parent = TypeConstraintTransaction::initialize_fixture(
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
        nested.original_context().unwrap().clone(),
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
        component.admit_fixture_roots(&mut context, vec![child], ()),
        Err(PreparedValueFailure::DependentRoot)
    ));
    assert!(component.values.is_empty());
    assert_eq!(component.next_value, 0);
}

#[test]
fn descendant_without_parent_receipt_cannot_be_registered_as_an_independent_root() {
    let effects = PreparedEffectGraph::default();
    use crate::types::constraints::application::ConstraintApplicationScope;

    let cancellation = AtomicBool::new(false);
    let mut context = ConstraintTestSetup::<_, Applications>::new(
        TypeConstraintLimits::new(65536, 65536, 1024, 1024),
        &cancellation,
    )
    .into_parts()
    .0;
    let mut component =
        OpenConstraintComponent::<Applications, ()>::new(&mut context, &effects).unwrap();
    let parent = ConstraintApplicationScope::test_root(0, TypeConstraintParameterScope::empty());
    let parent_id = parent.id();
    let path = context.start_path(parent).unwrap();
    let child = ConstraintApplicationScope::test_root(1, TypeConstraintParameterScope::empty());
    let child_id = child.id();
    let path = context.admit_application(path, child).unwrap();
    assert_eq!(path.root_id(), parent_id);
    let mut child =
        TypeConstraintTransaction::from_path(&mut context, child_id, path, None).unwrap();
    assert!(child.parent_receipt.is_none());
    child.request_projection(
        &mut context,
        (),
        &TypeKind::I32,
        TypeConstraintProjectionClosure::Closed,
    );
    let child = child.suspend_result(&mut context, ()).unwrap();
    assert!(matches!(
        component.admit_fixture_roots(&mut context, vec![child], ()),
        Err(PreparedValueFailure::DependentRoot)
    ));
    assert!(component.values.is_empty());
    assert_eq!(component.next_value, 0);
    let mut root = TypeConstraintTransaction::initialize_fixture(
        &mut context,
        2,
        TypeConstraintParameterScope::empty(),
        None,
    )
    .unwrap();
    root.request_projection(
        &mut context,
        (),
        &TypeKind::Bool,
        TypeConstraintProjectionClosure::Closed,
    );
    let root = root.suspend_result(&mut context, ()).unwrap();
    let registered = component
        .admit_fixture_roots(&mut context, vec![root], ())
        .unwrap();
    assert_eq!(registered.ordinal, 0);
    assert_eq!(component.values.len(), 1);
}

#[test]
fn imported_opening_cannot_change_semantic_application_when_original_roots_join() {
    let effects = PreparedEffectGraph::default();
    use crate::types::{
        DetachedGenericOwnerId, GenericParameterOwnerId, GenericTypeParameterId,
        constraints::{
            ConstraintAcceptance, ConstraintGenericParameterId, TypeConstraintEffectScope,
            TypeConstraintInvariant, TypeConstraintParameterEligibility,
            TypeConstraintParameterScopeInvariant, application::ConstraintApplicationScope,
            context::TypeConstraintTypeParameterScopeRow,
        },
    };
    for reversed in [false, true] {
        let cancellation = AtomicBool::new(false);
        let mut context = ConstraintTestSetup::<_, Applications>::new(
            TypeConstraintLimits::new(65536, 65536, 1024, 1024),
            &cancellation,
        )
        .into_parts()
        .0;
        let mut component =
            OpenConstraintComponent::<Applications, ()>::new(&mut context, &effects).unwrap();
        let key = GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(9492)),
            0,
        );
        let parameters = TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [TypeConstraintTypeParameterScopeRow::new(
                key.clone(),
                TypeConstraintParameterEligibility::Bindable,
            )],
            [],
            TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
            [],
            [],
        )
        .unwrap();
        let reference = parameters.type_reference(&key.into()).unwrap();
        let parent = context
            .start_path(ConstraintApplicationScope::test_root(1, parameters.clone()))
            .unwrap();
        let lease = parent
            .applications
            .import_parameters(&[ConstraintGenericParameterId::Type(reference.clone())])
            .unwrap();
        let mut owner =
            TypeConstraintTransaction::initialize_fixture(&mut context, 0, parameters, None)
                .unwrap();
        owner.constrain(
            &mut context,
            &TypeKind::GenericParam(reference),
            &TypeKind::Bool,
            ConstraintAcceptance::PatternAcceptsActual,
        );
        owner.request_projection(
            &mut context,
            (),
            &TypeKind::I64,
            TypeConstraintProjectionClosure::Closed,
        );
        let owner = owner.suspend_result(&mut context, ()).unwrap();
        let mut imported = TypeConstraintTransaction::initialize_with_imported_fixture(
            &mut context,
            2,
            TypeConstraintParameterScope::empty(),
            None,
            Some(lease),
        )
        .unwrap();
        imported.request_projection(
            &mut context,
            (),
            &TypeKind::I64,
            TypeConstraintProjectionClosure::Closed,
        );
        let imported = imported.suspend_result(&mut context, ()).unwrap();
        let roots = if reversed {
            [imported, owner]
        } else {
            [owner, imported]
        };
        for root in roots {
            component
                .admit_fixture_roots(&mut context, vec![root], ())
                .unwrap();
        }
        assert!(matches!(
            component.infer_original_products(&mut context),
            Err(PreparedValueFailure::Constraint(
                TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
                    TypeConstraintParameterScopeInvariant::ApplicationContractConflict
                ))
            ))
        ));
    }
}

#[test]
fn original_binding_inputs_retain_hidden_type_const_absence_and_lexical_scope() {
    let mut effects = PreparedEffectGraph::default();
    use crate::types::{
        DetachedGenericOwnerId, GenericConstParameterId, GenericParameterOwnerId,
        GenericTypeParameterId,
        constraints::{
            ConstraintAcceptance, TypeConstraintConstEligibility,
            TypeConstraintParameterEligibility,
            context::{TypeConstraintConstParameterScopeRow, TypeConstraintTypeParameterScopeRow},
        },
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(941));
    let bound = GenericTypeParameterId::new(owner.clone(), 0);
    let future = GenericTypeParameterId::new(owner.clone(), 1);
    let length = GenericConstParameterId::new(owner, 0);
    let parameters = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [
            TypeConstraintTypeParameterScopeRow::new(
                bound.clone(),
                TypeConstraintParameterEligibility::Bindable,
            ),
            TypeConstraintTypeParameterScopeRow::new(
                future.clone(),
                TypeConstraintParameterEligibility::FutureEligible,
            ),
        ],
        [TypeConstraintConstParameterScopeRow::new(
            length.clone(),
            TypeConstraintConstEligibility::Bindable,
        )],
        crate::types::constraints::TypeConstraintEffectScope::seal_call_scope([], []).unwrap(),
        [],
        [],
    )
    .unwrap();
    let bound = parameters.type_reference(&bound.into()).unwrap();
    let future = parameters.type_reference(&future.into()).unwrap();
    let length = parameters.const_reference(&length.into()).unwrap();
    let binder = GenericBinder::new(1, 0, 0);
    let enclosing = context.enter_binder_scope(binder);
    let mut lower =
        TypeConstraintTransaction::initialize_fixture(&mut context, (), parameters, None).unwrap();
    lower.constrain(
        &mut context,
        &TypeKind::Tuple(vec![
            TypeKind::GenericParam(bound.clone()),
            TypeKind::Array {
                item: Box::new(TypeKind::I32),
                len: ArrayLength::Generic(length.clone()),
            },
        ]),
        &TypeKind::Tuple(vec![
            TypeKind::String,
            TypeKind::Array {
                item: Box::new(TypeKind::I32),
                len: ArrayLength::Const(3),
            },
        ]),
        ConstraintAcceptance::PatternAcceptsActual,
    );
    lower.request_projection(
        &mut context,
        (),
        &TypeKind::I64,
        TypeConstraintProjectionClosure::Closed,
    );
    let root = lower.suspend_result(&mut context, ()).unwrap();
    let value = component
        .admit_fixture_roots(&mut context, vec![root], "original initializer")
        .unwrap();
    context.restore_binder_scope(enclosing);
    assert!(context.lexical_scope().binders().is_empty());
    let mut use_ = component
        .issue_fixture_use(&mut context, &[value.clone(), value])
        .unwrap();
    let alternative = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
        .unwrap()
        .unwrap();
    assert!(matches!(
        component.input(&use_, &alternative, 0).unwrap(),
        PreparedValueInput::Available {
            ty: TypeKind::I64,
            ..
        }
    ));
    let mut visits = 0;
    component
        .visit_original_constraints(&mut context, &use_, &alternative, |original, context| {
            visits += 1;
            assert_eq!(original.application(), ());
            assert_eq!(original.scope().binders(), &[binder]);
            assert_eq!(
                original.parameters().eligibility(&future),
                Some(TypeConstraintParameterEligibility::FutureEligible)
            );
            assert_eq!(
                original
                    .type_bindings()
                    .map(|(parameter, binding)| {
                        assert_eq!(binding.scope().binders(), &[binder]);
                        (parameter, binding.value().clone())
                    })
                    .collect::<Vec<_>>(),
                vec![(&bound, TypeKind::String)]
            );
            assert_eq!(
                original
                    .const_bindings()
                    .map(|(parameter, binding)| {
                        assert_eq!(binding.scope().binders(), &[binder]);
                        (parameter, binding.value().clone())
                    })
                    .collect::<Vec<_>>(),
                vec![(&length, ArrayLength::Const(3))]
            );
            assert!(
                original
                    .type_bindings()
                    .all(|(parameter, _)| parameter != &future)
            );
            let projected = project_type(
                &TypeKind::GenericParam(bound.clone()),
                original.projection_view(),
                ConstraintClosurePolicy::Hint,
                context,
            )?;
            assert_eq!(projected.value, TypeKind::String);
            Ok(())
        })
        .unwrap();
    assert_eq!(visits, 1);
}

#[test]
fn derived_value_reads_visit_one_original_path_and_do_not_invent_diagnostic_paths() {
    let mut effects = PreparedEffectGraph::default();
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let root = root(&mut context, &TypeKind::I32);
    let producer = component
        .admit_fixture_roots(&mut context, vec![root], "original")
        .unwrap();
    let mut first = component
        .issue_fixture_use(&mut context, &[producer.clone()])
        .unwrap();
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut effects, &mut first)
        .unwrap()
    {
        let outcome = if alternative.blocked {
            PreparedValueOutcome::Blocked("blocked")
        } else {
            PreparedValueOutcome::Accepted(output(&mut context, &TypeKind::I32))
        };
        component
            .submit_use(&mut context, &mut first, alternative, outcome)
            .unwrap();
    }
    let first = component.finish_use(first).unwrap();
    let derived = component
        .retain_use(&mut context, &mut effects, first)
        .unwrap();
    let mut use_ = component
        .issue_fixture_use(&mut context, &[producer, derived.clone(), derived])
        .unwrap();
    let mut available = 0;
    let mut diagnostic = 0;
    while let Some(alternative) = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
        .unwrap()
    {
        let mut visits = 0;
        component
            .visit_original_constraints(&mut context, &use_, &alternative, |original, _| {
                visits += 1;
                assert_eq!(original.type_bindings().len(), 0);
                assert_eq!(original.const_bindings().len(), 0);
                Ok(())
            })
            .unwrap();
        let outcome = if alternative.blocked {
            assert_eq!(visits, 0);
            diagnostic += 1;
            PreparedValueOutcome::Blocked("blocked")
        } else {
            assert_eq!(visits, 1);
            available += 1;
            PreparedValueOutcome::Accepted(output(&mut context, &TypeKind::I32))
        };
        component
            .submit_use(&mut context, &mut use_, alternative, outcome)
            .unwrap();
    }
    assert_eq!((available, diagnostic), (1, 2));
}

#[test]
fn original_constraint_observation_validates_authority_and_cancellation_before_visiting() {
    let mut effects = PreparedEffectGraph::default();
    use std::sync::atomic::Ordering;
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut component = Component::new(&mut context, &effects).unwrap();
    let root = root(&mut context, &TypeKind::I32);
    let producer = component
        .admit_fixture_roots(&mut context, vec![root], "original")
        .unwrap();
    let mut use_ = component
        .issue_fixture_use(&mut context, &[producer])
        .unwrap();
    let alternative = component
        .next_use_alternative(&mut context, &mut effects, &mut use_)
        .unwrap()
        .unwrap();
    let mut calls = 0;
    cancellation.store(true, Ordering::Relaxed);
    assert!(matches!(
        component.visit_original_constraints(&mut context, &use_, &alternative, |_, _| {
            calls += 1;
            Ok(())
        }),
        Err(PreparedValueFailure::Constraint(
            TypeConstraintError::Abort(_)
        ))
    ));
    assert_eq!(calls, 0);
    cancellation.store(false, Ordering::Relaxed);
    let other = Component::new(&mut context, &effects).unwrap();
    assert!(matches!(
        other.visit_original_constraints(&mut context, &use_, &alternative, |_, _| {
            calls += 1;
            Ok(())
        }),
        Err(PreparedValueFailure::ForeignIssuer)
    ));
    assert_eq!(calls, 0);
    component
        .visit_original_constraints(&mut context, &use_, &alternative, |_, _| {
            calls += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(calls, 1);
}
