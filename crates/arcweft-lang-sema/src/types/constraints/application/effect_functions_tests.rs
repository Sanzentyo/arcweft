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
            assert_eq!(repeated.effect_function(&variable), Some(&function));
            let parent = context.start_path_with_imported(first, None, original, graph.read())?;
            let branch = context.fork_path(&parent)?;
            assert_eq!(
                branch.applications.effect_function(&variable),
                Some(&function)
            );
            let lease = parent
                .applications
                .import_parameters(&[variable.clone().into()])?;
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
    let failed: Result<(), TypeConstraintError> =
        context.with_effect_transaction(&mut graph, |context, graph| {
            let application =
                context.application_scope((), parameters.clone(), original.clone(), graph)?;
            stale = application.effect_function(&variable).cloned();
            stale_application = Some(application);
            Err(TypeConstraintAbort::Cancelled.into())
        });
    assert!(failed.is_err());
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
