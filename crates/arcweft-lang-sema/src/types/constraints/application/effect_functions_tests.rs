use super::*;
use crate::{
    effect_row::{
        EffectConstraintVariable,
        prepared::{PreparedEffectGraph, PreparedEffectInvariant},
    },
    types::{
        DetachedGenericOwnerId, GenericBinder, GenericEffectParameterId, GenericParameterOwnerId,
        constraints::{
            NoConstraintClient, TypeConstraintAbort, TypeConstraintError,
            context::{LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits},
        },
    },
};
use std::sync::atomic::AtomicBool;

type Context<'a> = TypeConstraintContext<'a, LocalConstraintAccounting<'a>, NoConstraintClient>;

#[test]
fn conditional_output_agreement_uses_the_declared_captured_function_signature() {
    use crate::effect_row::prepared::EffectResidualPredicateView;
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
            let parent_variable = parameters.opened_effect_variables()[0].variable().clone();
            let original = context.fixture_original_context(None)?;
            let parent = context.application_scope((), parameters, original.clone(), &mut graph)?;
            let path = context.start_path_with_imported(parent, None, original, graph.read())?;
            let lease = path
                .applications
                .import_parameters(&[parent_variable.clone().into()], context)?;
            let original = context.fixture_original_context(Some(&lease))?;
            let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
            let child_variable = parameters.opened_effect_variables()[0].variable().clone();
            let child = context.application_scope((), parameters, original, &mut graph)?;
            let dependencies = graph
                .read()
                .unknown_dependencies(child.effect_function(&child_variable).unwrap())?
                .to_vec();
            assert!(dependencies.contains(&parent_variable));
            assert!(matches!(
                parent_variable,
                GenericEffectReference::Inference(_)
            ));
            let original = graph.read().freeze_scoped(&[], &[], &[], context)?;
            let bound = graph
                .bind_original_scoped(&original, context)?
                .freeze(graph.read(), context)?
                .compile_residual_for_test(context)?;
            let mut pending = vec![bound.satisfiable()];
            let mut found = false;
            while let Some(predicate) = pending.pop() {
                match predicate.view()? {
                    EffectResidualPredicateView::ForAllFiniteRows { parameters, .. } => {
                        found |= parameters == dependencies;
                    }
                    EffectResidualPredicateView::And(children) => pending.extend(children),
                    EffectResidualPredicateView::Not(body)
                    | EffectResidualPredicateView::ExistsLocalFunctions { body, .. }
                    | EffectResidualPredicateView::ExistsUniformChoices { body, .. }
                    | EffectResidualPredicateView::SubstituteLower { body, .. } => {
                        pending.push(body)
                    }
                    _ => {}
                }
            }
            assert!(
                found,
                "agreement quantifies the whole declared function signature"
            );
            // The same inference key remains illegal as an ambient source Γ row.
            assert!(matches!(
                graph.read().scoped_constraint(
                    &dependencies,
                    &crate::effect_row::prepared::PreparedEffectPredicate::Canonical(
                        crate::effect_row::EffectPredicate::unconstrained()
                    ),
                    &crate::effect_row::prepared::PreparedEffectPredicate::Canonical(
                        crate::effect_row::EffectPredicate::unconstrained()
                    ),
                    context
                ),
                Err(
                    crate::effect_row::prepared::PreparedEffectFailure::Invariant(
                        PreparedEffectInvariant::ParameterContract
                    )
                )
            ));
            Ok::<_, TypeConstraintError>(())
        })
        .unwrap();
}

fn context(cancellation: &AtomicBool) -> Context<'_> {
    Context::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(65_536, 32_768, 1024, 1024),
        cancellation,
    ))
}

fn parameter_scope(eligibility: EffectConstraintEligibility) -> TypeConstraintParameterScope {
    let variable = GenericEffectParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_401)),
        0,
    )
    .into();
    TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [],
        [],
        TypeConstraintEffectScope::seal_call_scope(
            [EffectConstraintVariable::new(variable, eligibility)],
            [],
        )
        .unwrap(),
        [],
        [],
    )
    .unwrap()
}

#[test]
fn branches_and_imports_keep_the_issued_function_with_the_complete_intro_gamma() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let variable = parameters.opened_effect_variables()[0].variable().clone();
    context
        .with_binder(GenericBinder::new(0, 0, 2), |context| {
            let original = context.fixture_original_context(None)?;
            let first =
                context.application_scope((), parameters.clone(), original.clone(), &mut graph)?;
            let function = first.effect_function(&variable).unwrap().clone();
            let expected = [
                context.lexical_scope().bound_effect(0, 0)?,
                context.lexical_scope().bound_effect(0, 1)?,
            ];
            assert_eq!(graph.read().unknown_dependencies(&function)?, expected);
            let repeated =
                context.application_scope((), parameters, original.clone(), &mut graph)?;
            assert!(first.same_owner(&repeated));
            assert_eq!(repeated.effect_function(&variable), Some(&function));
            let parent = context.start_path_with_imported(first, None, original, graph.read())?;
            let branch = context.fork_path(&parent)?;
            assert_eq!(
                branch.applications.effect_function(&variable),
                Some(&function)
            );
            let lease = parent
                .applications
                .import_parameters(&[variable.clone().into()], context)?;
            assert_eq!(lease.effect_function(&variable), Some(&function));
            let original = context.fixture_original_context(Some(&lease))?;
            let child_parameters = parameter_scope(EffectConstraintEligibility::FutureEligible);
            let child_variable = child_parameters.opened_effect_variables()[0]
                .variable()
                .clone();
            let child =
                context.application_scope((), child_parameters, original.clone(), &mut graph)?;
            let child_function = child.effect_function(&child_variable).unwrap().clone();
            let mut expected = expected.to_vec();
            expected.push(variable.clone());
            expected.sort();
            assert_eq!(
                graph.read().unknown_dependencies(&child_function)?,
                expected
            );
            let child =
                context.start_path_with_imported(child, Some(lease), original, graph.read())?;
            assert_eq!(
                child.applications.effect_function(&variable),
                Some(&function)
            );
            assert_eq!(
                child.applications.effect_function(&child_variable),
                Some(&child_function)
            );
            Ok::<_, TypeConstraintError>(())
        })
        .unwrap();
}

#[test]
fn equally_shaped_new_frames_cannot_redeclare_the_same_opened_variable() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let variable = parameters.opened_effect_variables()[0].variable().clone();
    let first = context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let original = context.fixture_original_context(None)?;
            context.application_scope((), parameters.clone(), original, &mut graph)
        })
        .unwrap();
    let function = first.effect_function(&variable).unwrap().clone();
    let result = context.with_binder(GenericBinder::new(0, 0, 1), |context| {
        let original = context.fixture_original_context(None)?;
        context.application_scope((), parameters, original, &mut graph)
    });
    assert!(matches!(
        result,
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(
                crate::effect_row::prepared::PreparedEffectFailure::Invariant(
                    PreparedEffectInvariant::ParameterContract,
                )
            ),
        ))
    ));
    graph.read().validate_unknown(&function).unwrap();
}

#[test]
fn rolled_back_application_receipts_stay_stale_after_redeclaration() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let variable = parameters.opened_effect_variables()[0].variable().clone();
    let original = context.fixture_original_context(None).unwrap();
    let mut stale = None;
    let mut stale_application = None;
    let failed = context.with_application_scope(
        &mut graph,
        None,
        (),
        parameters.clone(),
        original.clone(),
        |context, graph, application| {
            let function = application.effect_function(&variable).unwrap();
            graph.read().validate_unknown(function)?;
            stale = Some(function.clone());
            stale_application = Some(application);
            cancellation.store(true, std::sync::atomic::Ordering::Release);
            context.check_cancelled()
        },
    );
    assert!(matches!(
        failed,
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
    cancellation.store(false, std::sync::atomic::Ordering::Release);
    assert_eq!(
        graph.read().validate_unknown(stale.as_ref().unwrap()),
        Err(PreparedEffectInvariant::StaleReference)
    );
    let application = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    let function = application.effect_function(&variable).unwrap();
    assert_ne!(Some(function), stale.as_ref());
    graph.read().validate_unknown(function).unwrap();
    assert_eq!(
        graph.read().validate_unknown(stale.as_ref().unwrap()),
        Err(PreparedEffectInvariant::StaleReference)
    );
    let stale_path =
        context.start_path_with_imported(stale_application.unwrap(), None, original, graph.read());
    assert!(matches!(
        stale_path,
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(
                crate::effect_row::prepared::PreparedEffectFailure::Invariant(
                    PreparedEffectInvariant::StaleReference,
                )
            ),
        ))
    ));
}

#[test]
fn an_unused_source_gamma_parameter_is_part_of_the_function_signature() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let variable = parameters.opened_effect_variables()[0].variable().clone();
    let captured: GenericEffectReference = GenericEffectParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_402)),
        0,
    )
    .into();
    let original = context
        .fixture_original_context_with_rigid(super::super::original_context::OriginalRigidScope {
            types: &[],
            consts: &[],
            effects: std::slice::from_ref(&captured),
            imported: None,
        })
        .unwrap();
    let application = context
        .application_scope((), parameters.clone(), original, &mut graph)
        .unwrap();
    let function = application.effect_function(&variable).unwrap();
    assert_eq!(
        graph.read().unknown_dependencies(function).unwrap(),
        [captured]
    );
    let smaller_gamma = context.fixture_original_context(None).unwrap();
    let conflict = context.application_scope((), parameters, smaller_gamma, &mut graph);
    assert!(matches!(
        conflict,
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(
                crate::effect_row::prepared::PreparedEffectFailure::Invariant(
                    PreparedEffectInvariant::ParameterContract,
                )
            ),
        ))
    ));
    graph.read().validate_unknown(function).unwrap();
}

#[test]
fn a_registered_application_cannot_be_admitted_with_another_graph() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let original = context.fixture_original_context(None).unwrap();
    let application = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    let other_graph = PreparedEffectGraph::default();
    let result = context.start_path_with_imported(application, None, original, other_graph.read());
    assert!(matches!(
        result,
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(
                crate::effect_row::prepared::PreparedEffectFailure::Invariant(
                    PreparedEffectInvariant::ForeignIssuer,
                )
            ),
        ))
    ));
}

#[test]
fn a_closed_root_preserves_its_predicate_and_authenticates_imported_effects() {
    use crate::effect_row::{EffectPredicate, prepared::PreparedEffectPredicate};
    use crate::types::constraints::{
        TypeConstraintInitializationFailure, transaction::TypeConstraintTransaction,
    };

    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let parent_variable = parameters.opened_effect_variables()[0].variable().clone();
    let original = context.fixture_original_context(None).unwrap();
    let parent = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    let function = parent.effect_function(&parent_variable).unwrap().clone();
    let parent = context
        .start_path_with_imported(parent, None, original, graph.read())
        .unwrap();
    let imported = parent
        .applications
        .import_parameters(&[parent_variable.clone().into()], &mut context)
        .unwrap();
    let original = context.fixture_original_context(Some(&imported)).unwrap();
    let effects = TypeConstraintEffectScope::seal_call_scope_with_predicate(
        [],
        [],
        PreparedEffectPredicate::Canonical(EffectPredicate::impossible()),
    )
    .unwrap();
    let parameters = TypeConstraintParameterScope::seal_call_scope(
        GenericBinder::EMPTY,
        [],
        [],
        effects,
        [],
        [],
    )
    .unwrap();

    let mut foreign = PreparedEffectGraph::default();
    let mut no_branches = Context::with_accounting(LocalConstraintAccounting::new(
        TypeConstraintLimits::new(1024, 1024, 0, 1024),
        &cancellation,
    ));
    let rejected = TypeConstraintTransaction::initialize_with_imported(
        &mut no_branches,
        &mut foreign,
        original.clone(),
        (),
        parameters.clone(),
        None,
        Some(imported.clone()),
    );
    assert!(matches!(
        rejected,
        Err(TypeConstraintInitializationFailure::Abort(
            TypeConstraintAbort::BranchLimit {
                actual: 1,
                limit: 0
            }
        ))
    ));

    let result = TypeConstraintTransaction::initialize_with_imported(
        &mut context,
        &mut foreign,
        original.clone(),
        (),
        parameters.clone(),
        None,
        Some(imported.clone()),
    );
    assert!(matches!(
        result,
        Err(TypeConstraintInitializationFailure::Invariant(
            TypeConstraintInvariant::PreparedEffect(
                crate::effect_row::prepared::PreparedEffectFailure::Invariant(
                    PreparedEffectInvariant::ForeignIssuer,
                )
            )
        ))
    ));

    let retained = graph.retained_terms();
    let child = TypeConstraintTransaction::initialize_with_imported(
        &mut context,
        &mut graph,
        original,
        (),
        parameters,
        None,
        Some(imported),
    )
    .unwrap();
    let path = child.test_single_path();
    assert_eq!(
        path.effects.ambient.original_predicate().constant_truth(),
        Some(false)
    );
    assert_eq!(
        path.applications.effect_function(&parent_variable),
        Some(&function)
    );
    assert_eq!(graph.retained_terms(), retained);
}

#[test]
fn original_r_instantiates_the_registered_function_under_its_actual_gamma() {
    use crate::effect_row::prepared::{
        PreparedEffectPredicate, PreparedEffectRelation, PreparedEffectRow,
    };
    use crate::{
        effect_row::{EffectPredicate, EffectRow},
        effects::EffectSet,
    };
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
            let variable = parameters.opened_effect_variables()[0].variable().clone();
            let input = context.lexical_scope().bound_effect(0, 0)?;
            let original = context.fixture_original_context(None)?;
            let owner = context.application_scope((), parameters, original.clone(), &mut graph)?;
            let function = owner.effect_function(&variable).unwrap().clone();
            let mut path = context.start_path_with_imported(owner, None, original, graph.read())?;
            path.effects.ambient.constrain_relation(
                &mut graph,
                PreparedEffectRelation::Equal,
                &PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), variable.clone())),
                &PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), input.clone())),
                context,
            )?;
            let prepared = path.applications.prepare_effect_predicate(
                &path.effects.ambient,
                path.effects.ambient.original_predicate(),
                &path.scope,
                &mut graph,
                context,
            )?;
            let mut remaining = BTreeSet::new();
            graph
                .read()
                .visit_predicate_parameters(&prepared, context, &mut |reference, _| {
                    remaining.insert(reference.clone());
                    Ok(())
                })?;
            assert_eq!(remaining, BTreeSet::from([input.clone()]));
            graph.add_scoped_obligation(
                vec![input.clone()],
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                prepared,
                context,
            )?;
            let principal = graph.solve_principal(context)?;
            assert_eq!(
                principal.unknown(&function)?,
                &EffectRow::open(EffectSet::new(), input)
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn transitive_function_captures_extend_gamma_from_the_source_owners() {
    use crate::effect_row::prepared::{PreparedEffectRelation, PreparedEffectRow};
    use crate::{effect_row::EffectRow, effects::EffectSet};
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let captures: Vec<GenericEffectReference> = (0..2)
        .map(|slot| {
            GenericEffectParameterId::new(
                GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_403)),
                slot,
            )
            .into()
        })
        .collect();
    let original = context
        .fixture_original_context_with_rigid(super::super::original_context::OriginalRigidScope {
            types: &[],
            consts: &[],
            effects: &captures,
            imported: None,
        })
        .unwrap();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let parent_variable = parameters.opened_effect_variables()[0].variable().clone();
    let parent = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    let parent = context
        .start_path_with_imported(parent, None, original, graph.read())
        .unwrap();
    let imported = parent
        .applications
        .import_parameters(&[parent_variable.clone().into()], &mut context)
        .unwrap();
    let original = context.fixture_original_context(Some(&imported)).unwrap();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let child_variable = parameters.opened_effect_variables()[0].variable().clone();
    let child = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    let mut child = context
        .start_path_with_imported(child, Some(imported), original, graph.read())
        .unwrap();
    for capture in &captures {
        assert!(
            child
                .effects
                .ambient
                .original_variables()
                .any(|variable| variable == capture)
        );
    }
    child
        .effects
        .ambient
        .constrain_relation(
            &mut graph,
            PreparedEffectRelation::Equal,
            &PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), child_variable)),
            &PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), parent_variable)),
            &mut context,
        )
        .unwrap();
    let prepared = child
        .prepare_effect_constraint(&mut graph, &mut context)
        .unwrap();
    let mut references = BTreeSet::new();
    graph
        .read()
        .visit_predicate_parameters(prepared.predicate(), &mut context, &mut |reference, _| {
            references.insert(reference.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(references, captures.into_iter().collect());
}

#[test]
fn an_application_row_can_be_stale_while_its_registered_function_is_still_live() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let variable = parameters.opened_effect_variables()[0].variable().clone();
    let original = context.fixture_original_context(None).unwrap();
    let first = context
        .application_scope((), parameters.clone(), original.clone(), &mut graph)
        .unwrap();
    let function = first.effect_function(&variable).unwrap().clone();
    let mut checkpoint = graph.checkpoint(&mut context).unwrap();
    let stale = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    assert!(first.same_owner(&stale));
    graph.rollback(&mut checkpoint).unwrap();
    graph.read().validate_unknown(&function).unwrap();
    first
        .validate_effect_functions(graph.read(), &mut context)
        .unwrap();
    let result = context.start_path_with_imported(stale, None, original, graph.read());
    assert!(matches!(
        result,
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedEffect(
                crate::effect_row::prepared::PreparedEffectFailure::Invariant(
                    PreparedEffectInvariant::StaleReference,
                )
            ),
        ))
    ));
}

#[test]
fn failed_r_projection_preserves_the_original_r_and_releases_new_terms() {
    use crate::effect_row::prepared::{PreparedEffectRelation, PreparedEffectRow};
    use crate::{effect_row::EffectRow, effects::EffectSet};
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let variable = parameters.opened_effect_variables()[0].variable().clone();
    let original = context.fixture_original_context(None).unwrap();
    let owner = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    let mut path = context
        .start_path_with_imported(owner, None, original, graph.read())
        .unwrap();
    path.effects
        .ambient
        .constrain_relation(
            &mut graph,
            PreparedEffectRelation::Equal,
            &PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), variable.clone())),
            &PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
            &mut context,
        )
        .unwrap();
    let foreign: GenericEffectReference = GenericEffectParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_404)),
        0,
    )
    .into();
    let source = path.effects.ambient.original_predicate().clone();
    let retained = graph.retained_terms();
    let result = path.effects.ambient.project_registered_predicate(
        &source,
        &BTreeMap::from([(
            variable,
            PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), foreign)),
        )]),
        &mut graph,
        &mut context,
    );
    assert!(result.is_err());
    assert_eq!(path.effects.ambient.original_predicate(), &source);
    assert_eq!(graph.retained_terms(), retained);
    graph.read().validate_retained_predicate(&source).unwrap();
}

#[test]
fn application_transport_uses_actual_frames_and_fails_closed_on_an_equal_shape() {
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let variable = parameters.opened_effect_variables()[0].variable().clone();
    let owner = context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let original = context.fixture_original_context(None)?;
            let owner = context.application_scope((), parameters, original, &mut graph)?;
            context.with_binder(GenericBinder::new(0, 0, 1), |context| {
                let target = context.capture_lexical_scope()?;
                let mut edit = graph.edit_terms();
                let row = owner
                    .effect_application(&variable)
                    .unwrap()
                    .transport(&target, &mut edit, context)?;
                edit.commit()?;
                let mut references = BTreeSet::new();
                graph
                    .read()
                    .visit_row_parameters(&row, context, &mut |reference, _| {
                        references.insert(reference.clone());
                        Ok(())
                    })?;
                assert_eq!(
                    references,
                    BTreeSet::from([target.shape().bound_effect(1, 0)?])
                );
                Ok(())
            })?;
            Ok(owner)
        })
        .unwrap();
    let retained = graph.retained_terms();
    context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let target = context.capture_lexical_scope()?;
            let mut edit = graph.edit_terms();
            assert!(
                owner
                    .effect_application(&variable)
                    .unwrap()
                    .transport(&target, &mut edit, context)
                    .is_err()
            );
            assert_eq!(edit.commit(), Err(PreparedEffectInvariant::TermEditState));
            Ok(())
        })
        .unwrap();
    assert_eq!(graph.retained_terms(), retained);
}

#[test]
fn imported_introduction_frames_survive_outside_the_parent_lexical_traversal() {
    use crate::effect_row::prepared::{PreparedEffectRelation, PreparedEffectRow};
    use crate::{effect_row::EffectRow, effects::EffectSet};
    let cancellation = AtomicBool::new(false);
    let mut context = context(&cancellation);
    let mut graph = PreparedEffectGraph::default();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let parent_variable = parameters.opened_effect_variables()[0].variable().clone();
    let parent = context
        .with_binder(GenericBinder::new(0, 0, 1), |context| {
            let original = context.fixture_original_context(None)?;
            let owner = context.application_scope((), parameters, original.clone(), &mut graph)?;
            context.start_path_with_imported(owner, None, original, graph.read())
        })
        .unwrap();
    assert!(context.lexical_scope().binders().is_empty());
    let imported = parent
        .applications
        .import_parameters(&[parent_variable.clone().into()], &mut context)
        .unwrap();
    let original = context.fixture_original_context(Some(&imported)).unwrap();
    let parameters = parameter_scope(EffectConstraintEligibility::Bindable);
    let child_variable = parameters.opened_effect_variables()[0].variable().clone();
    let owner = context
        .application_scope((), parameters, original.clone(), &mut graph)
        .unwrap();
    assert_eq!(owner.introduction_scope(), original.gamma());
    assert!(context.lexical_scope().binders().is_empty());
    let mut child = context
        .start_path_with_imported(owner, Some(imported), original, graph.read())
        .unwrap();
    child
        .effects
        .ambient
        .constrain_relation(
            &mut graph,
            PreparedEffectRelation::Equal,
            &PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), child_variable)),
            &PreparedEffectRow::Canonical(EffectRow::open(EffectSet::new(), parent_variable)),
            &mut context,
        )
        .unwrap();
    let mut joint = ConstraintLexicalScope::default();
    child
        .applications
        .merge_lexical_scopes(&mut joint, &mut context)
        .unwrap();
    let prepared = child
        .applications
        .prepare_effect_predicate(
            &child.effects.ambient,
            child.effects.ambient.original_predicate(),
            &joint,
            &mut graph,
            &mut context,
        )
        .unwrap();
    let mut references = BTreeSet::new();
    graph
        .read()
        .visit_predicate_parameters(&prepared, &mut context, &mut |reference, _| {
            references.insert(reference.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(
        references,
        BTreeSet::from([joint.shape().bound_effect(0, 0).unwrap()])
    );
}
