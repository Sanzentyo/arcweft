//! Lower-schema differentials that do not require callable/analyzer wiring.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use super::context::{
    LocalConstraintAccounting, TypeConstraintAccounting, TypeConstraintContext,
    TypeConstraintContextIssuer, TypeConstraintEffectScope, TypeConstraintLimits,
    TypeConstraintWorkReport,
};
use super::{
    ExpectedHint, SourceError, SourcePhase, TypeConstraintConstEligibility,
    TypeConstraintParameterEligibility, TypeConstraintParameterScope,
};
use crate::effect_row::{
    EffectConstraintEligibility, EffectConstraintVariable, EffectRow, EffectVar, EffectVarIssuer,
};
use crate::effects::EffectSet;
use crate::types::{
    DetachedGenericOwnerId, GenericConstParameterId, GenericParameterOwnerId,
    GenericTypeParameterId, TypeKind, TypePoisonId,
};

fn parameter(ordinal: u16) -> GenericTypeParameterId {
    GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(91)),
        ordinal,
    )
}

fn try_type_only_solution(
    bindings: impl IntoIterator<Item = (GenericTypeParameterId, TypeKind)>,
    future: &[GenericTypeParameterId],
) -> Result<TypeConstraintSolution, TypeConstraintError> {
    fn collect(ty: &TypeKind, parameters: &mut BTreeSet<GenericTypeParameterId>) {
        match ty.constraint_shape() {
            super::TypeConstraintShape::Generic(parameter) => {
                parameters.insert(parameter.clone());
            }
            super::TypeConstraintShape::Unresolved | super::TypeConstraintShape::Never => {}
            shape => {
                for child in shape.children() {
                    collect(child, parameters);
                }
            }
        }
    }

    let bindings = bindings.into_iter().collect::<Vec<_>>();
    let binding_keys = bindings
        .iter()
        .map(|(parameter, _)| parameter.clone())
        .collect::<BTreeSet<_>>();
    let future = future.iter().cloned().collect::<BTreeSet<_>>();
    let mut parameters = binding_keys.clone();
    for (_, value) in &bindings {
        collect(value, &mut parameters);
    }
    let scope = TypeConstraintParameterScope::seal_call_scope(
        parameters.into_iter().map(|parameter| {
            let eligibility = if binding_keys.contains(&parameter) {
                TypeConstraintParameterEligibility::Bindable
            } else if future.contains(&parameter) {
                TypeConstraintParameterEligibility::FutureEligible
            } else {
                TypeConstraintParameterEligibility::Rigid
            };
            super::context::TypeConstraintTypeParameterScopeRow::new(parameter, eligibility)
        }),
        std::iter::empty(),
        std::iter::empty(),
    )
    .expect("test completed-solution scope is canonical");
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(4_096, 2_048, 256, 128),
            &cancellation,
            scope,
        );
    TypeConstraintSolution::test_seal_completed(bindings, std::iter::empty(), &mut context)
}

fn type_only_solution(
    bindings: BTreeMap<GenericTypeParameterId, TypeKind>,
    future: &[GenericTypeParameterId],
) -> TypeConstraintSolution {
    try_type_only_solution(bindings, future).expect("canonical completed test solution")
}

fn bindable_scope(types: &[&TypeKind]) -> TypeConstraintParameterScope {
    fn collect(ty: &TypeKind, parameters: &mut BTreeSet<GenericTypeParameterId>) {
        match ty.constraint_shape() {
            super::TypeConstraintShape::Generic(parameter) => {
                parameters.insert(parameter.clone());
            }
            super::TypeConstraintShape::Unresolved | super::TypeConstraintShape::Never => {}
            shape => {
                for child in shape.children() {
                    collect(child, parameters);
                }
            }
        }
    }

    let mut parameters = BTreeSet::new();
    for ty in types {
        collect(ty, &mut parameters);
    }
    TypeConstraintParameterScope::new(
        parameters
            .into_iter()
            .map(|parameter| (parameter, TypeConstraintParameterEligibility::Bindable)),
    )
    .expect("unique bindable test scope")
}

#[test]
fn parameter_scope_is_sorted_and_classified_without_callable_metadata() {
    let first = parameter(1);
    let second = parameter(0);
    let scope = TypeConstraintParameterScope::new([
        (
            first.clone(),
            TypeConstraintParameterEligibility::FutureEligible,
        ),
        (second.clone(), TypeConstraintParameterEligibility::Rigid),
    ])
    .expect("unique lower scope");

    let rows = scope
        .iter()
        .map(|(parameter, eligibility)| (parameter, eligibility));
    assert_eq!(rows.count(), 2);
    assert_eq!(
        scope.eligibility(&second),
        Some(TypeConstraintParameterEligibility::Rigid)
    );
}

#[test]
fn const_scope_is_kind_separated_and_only_rigid_is_constructible() {
    let constant = GenericConstParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(92)),
        0,
    );
    let scope = TypeConstraintParameterScope::new_with_constants(
        std::iter::empty::<(GenericTypeParameterId, TypeConstraintParameterEligibility)>(),
        [(constant.clone(), TypeConstraintConstEligibility::Rigid)],
    )
    .expect("rigid const scope");
    assert_eq!(
        scope.const_eligibility(&constant),
        Some(TypeConstraintConstEligibility::Rigid)
    );
    let array = TypeKind::Array {
        item: Box::new(TypeKind::I32),
        len: super::super::ArrayLength::Generic(constant.clone()),
    };
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            scope,
        );
    assert!(
        super::normalization::project_type(
            &array,
            &BTreeMap::new(),
            super::ConstraintClosurePolicy::Hint,
            &mut context,
        )
        .is_ok()
    );
    assert!(matches!(
        TypeConstraintParameterScope::seal_call_scope(
            std::iter::empty(),
            [super::context::TypeConstraintConstParameterScopeRow::new(
                constant,
                TypeConstraintConstEligibility::Rigid,
            )],
            std::iter::empty(),
        ),
        Ok(_)
    ));
}

#[test]
fn source_error_retains_phase_and_expected_hint_is_typed() {
    let expected = TypeKind::I32;
    let hint = super::ProjectedExpectedHint::Complete(&expected);
    assert!(
        matches!(hint, super::ProjectedExpectedHint::Complete(value) if value == &TypeKind::I32)
    );

    let error = SourceError::new(4_u8, SourcePhase::Materialize, "typed");
    assert_eq!(error.source(), &4);
    assert_eq!(error.phase(), SourcePhase::Materialize);
    assert_eq!(error.cause(), &"typed");
}

#[test]
fn parameter_scope_classifies_rigid_attempt_out_of_scope_and_terminal_unbound_rows() {
    use super::ConstraintAcceptance;
    use super::context::{TypeConstraintContext, TypeConstraintLimits};

    let rigid = parameter(0);
    let bindable = parameter(1);
    let scope = TypeConstraintParameterScope::new([
        (rigid.clone(), TypeConstraintParameterEligibility::Rigid),
        (
            bindable.clone(),
            TypeConstraintParameterEligibility::Bindable,
        ),
    ])
    .expect("unique scope");
    let cancellation = AtomicBool::new(false);
    let mut rigid_transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<
            super::LocalConstraintAccounting<'_>,
            super::NoConstraintClient,
        >::with_scope(
            TypeConstraintLimits::new(64, 64, 64, 8),
            &cancellation,
            scope.clone(),
        ),
        None,
    );
    rigid_transaction.constrain(
        &TypeKind::GenericParam(rigid),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(matches!(
        rigid_transaction.finish().complete(),
        Err(super::TypeConstraintFailure::Rejected(
            super::TypeConstraintCandidateFailure::Constraint(
                super::TypeConstraintRejection::Mismatch,
            ),
        ))
    ));

    let foreign = parameter(2);
    let mut foreign_transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<
            super::LocalConstraintAccounting<'_>,
            super::NoConstraintClient,
        >::with_scope(
            TypeConstraintLimits::new(64, 64, 64, 8),
            &cancellation,
            scope.clone(),
        ),
        None,
    );
    foreign_transaction.constrain(
        &TypeKind::GenericParam(foreign),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(matches!(
        foreign_transaction.finish().complete(),
        Err(super::TypeConstraintFailure::Invariant(
            super::TypeConstraintFailureInvariant::Constraint(
                super::TypeConstraintInvariant::ParameterScope(
                    super::TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope { .. },
                ),
            ),
        ))
    ));

    let mut incomplete_transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<
            super::LocalConstraintAccounting<'_>,
            super::NoConstraintClient,
        >::with_scope(
            TypeConstraintLimits::new(64, 64, 64, 8),
            &cancellation,
            scope,
        ),
        None,
    );
    incomplete_transaction.constrain(
        &TypeKind::I32,
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(matches!(
        incomplete_transaction.finish().complete(),
        Err(super::TypeConstraintFailure::Rejected(
            super::TypeConstraintCandidateFailure::Constraint(
                super::TypeConstraintRejection::IncompleteInstantiation { .. },
            ),
        ))
    ));
    assert_eq!(bindable.ordinal(), 1);
}

use super::transaction::{
    MaterializationTicket, ProbeSubmission, ProbeTicket, TypeConstraintRun,
    TypeConstraintTransaction,
};
use super::*;

fn close_materialization_ticket<D: ConstraintDomain>(
    ticket: &mut MaterializationTicket<D>,
    submission: ClosedMaterializationSubmission<D>,
) -> super::transaction::ClosedMaterialization<D> {
    ticket
        .bind_callback()
        .expect("ready materialization ticket");
    ticket
        .bind_closed_submission(submission)
        .expect("callback-bound materialization ticket")
}

struct TestConstraintTransaction<'c, A: TypeConstraintAccounting, D: ConstraintDomain> {
    lower: TypeConstraintTransaction<D>,
    context: TypeConstraintContext<'c, A, D>,
}

impl<'c, A, D> TestConstraintTransaction<'c, A, D>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    fn begin(
        context: TypeConstraintContext<'c, A, D>,
        inherited: Option<Arc<TypeConstraintSolution>>,
    ) -> Self {
        let mut lower = TypeConstraintTransaction::new();
        let mut context = context;
        lower
            .initialize(&mut context, inherited)
            .expect("valid test initialization");
        Self { lower, context }
    }

    fn constrain(
        &mut self,
        pattern: &TypeKind,
        actual: &TypeKind,
        acceptance: ConstraintAcceptance,
    ) {
        self.lower
            .constrain(&mut self.context, pattern, actual, acceptance);
    }

    fn request_projection(
        &mut self,
        key: D::Projection,
        value: &TypeKind,
        closure: TypeConstraintProjectionClosure,
    ) {
        self.lower.request_projection(key, value, closure);
    }

    fn finish(self) -> TypeConstraintRun<'c, A, D> {
        self.lower.finish(self.context)
    }
}

fn owned_parameter(owner: u64, ordinal: u16) -> GenericTypeParameterId {
    GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(owner)),
        ordinal,
    )
}

fn solve(
    pattern: TypeKind,
    actual: TypeKind,
) -> Result<SolvedCandidate<NoConstraintClient>, TypeConstraintFailure<NoConstraintClient>> {
    solve_with(pattern, actual, ConstraintAcceptance::PatternAcceptsActual)
}

fn solve_with(
    pattern: TypeKind,
    actual: TypeKind,
    acceptance: ConstraintAcceptance,
) -> Result<SolvedCandidate<NoConstraintClient>, TypeConstraintFailure<NoConstraintClient>> {
    let cancellation = AtomicBool::new(false);
    let scope = bindable_scope(&[&pattern, &actual]);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1_024, 512, 128, 64),
            &cancellation,
            scope,
        ),
        None,
    );
    transaction.constrain(&pattern, &actual, acceptance);
    transaction.finish().complete()
}

#[test]
fn function_relation_is_contravariant_in_parameters_and_covariant_in_effects() {
    let issuer = EffectVarIssuer::fresh_prepared().expect("test issuer");
    let variable = EffectVar::issued(issuer, 0);
    let effect_scope = TypeConstraintEffectScope::seal_call_scope(
        [EffectConstraintVariable::new(
            variable,
            EffectConstraintEligibility::Bindable,
        )],
        std::iter::empty(),
    )
    .expect("effect scope");
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scopes(
            TypeConstraintLimits::new(1_024, 512, 128, 64),
            &cancellation,
            TypeConstraintParameterScope::empty(),
            effect_scope,
        ),
        None,
    );
    let expected = TypeKind::function_with_effects(
        [TypeKind::I32],
        TypeKind::I32,
        EffectRow::open(EffectSet::new(), variable),
    );
    let actual_effects = EffectSet::from_labels(["fs.read"]).expect("effect");
    let actual = TypeKind::function_with_effects(
        [TypeKind::AgentValue],
        TypeKind::I32,
        EffectRow::closed(actual_effects.clone()),
    );
    transaction.constrain(
        &expected,
        &actual,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    let solved = transaction.finish().complete().expect("function relation");

    assert_eq!(
        solved
            .solution
            .effect_bindings()
            .map(|(variable, value)| (*variable, value.clone()))
            .collect::<Vec<_>>(),
        vec![(variable, EffectRow::closed(actual_effects))]
    );
}

#[test]
fn function_effect_subset_failure_is_an_ordinary_candidate_rejection() {
    let expected = TypeKind::function_with_effects(
        [TypeKind::Never],
        TypeKind::I32,
        EffectRow::closed(EffectSet::new()),
    );
    let actual = TypeKind::function_with_effects(
        [TypeKind::I32],
        TypeKind::I32,
        EffectRow::closed(EffectSet::from_labels(["fs.read"]).expect("effect")),
    );
    assert!(matches!(
        solve(expected, actual),
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch)
        ))
    ));
}

#[test]
fn transitive_bindings_are_sealed_and_move_only() {
    let first = owned_parameter(1, 0);
    let second = owned_parameter(1, 1);
    let outcome = {
        let cancellation = AtomicBool::new(false);
        let pattern = TypeKind::Tuple(vec![
            TypeKind::GenericParam(first.clone()),
            TypeKind::GenericParam(second.clone()),
        ]);
        let actual = TypeKind::Tuple(vec![TypeKind::GenericParam(second.clone()), TypeKind::I32]);
        let mut transaction = TestConstraintTransaction::begin(
            TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
                TypeConstraintLimits::new(1_024, 512, 128, 64),
                &cancellation,
                bindable_scope(&[&pattern, &actual]),
            ),
            None,
        );
        transaction.constrain(
            &pattern,
            &actual,
            ConstraintAcceptance::PatternAcceptsActual,
        );
        transaction.finish().complete()
    }
    .expect("one sealed solution");
    let bindings = outcome
        .solution
        .bindings()
        .map(|(parameter, value)| (parameter.clone(), value.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        bindings,
        vec![(first, TypeKind::I32), (second, TypeKind::I32)]
    );
}

#[test]
fn choice_acceptance_is_branch_local_and_ambiguous_bindings_remain() {
    let first = owned_parameter(2, 0);
    let second = owned_parameter(2, 1);
    let outcome = solve(
        TypeKind::Choice(vec![
            TypeKind::GenericParam(first),
            TypeKind::GenericParam(second),
        ]),
        TypeKind::I32,
    );
    assert!(matches!(
        outcome,
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(
                TypeConstraintRejection::AmbiguousSolution { actual: 2 },
            ),
        ))
    ));
}

#[test]
fn supplied_actual_choice_requires_every_alternative_to_match() {
    let outcome = solve(
        TypeKind::I32,
        TypeKind::Choice(vec![TypeKind::I32, TypeKind::String]),
    );
    assert!(matches!(
        outcome,
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch),
        ))
    ));
}

#[test]
fn reverse_acceptance_uses_actual_choice_as_the_acceptor() {
    let accepted = solve_with(
        TypeKind::I32,
        TypeKind::Choice(vec![TypeKind::I32, TypeKind::String]),
        ConstraintAcceptance::ActualAcceptsPattern,
    );
    assert!(accepted.is_ok());

    let rejected = solve_with(
        TypeKind::Choice(vec![TypeKind::I32, TypeKind::String]),
        TypeKind::I32,
        ConstraintAcceptance::ActualAcceptsPattern,
    );
    assert!(matches!(
        rejected,
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch),
        ))
    ));
}

#[test]
fn actual_choice_keeps_generic_bindings_consistent_across_all_rows() {
    let parameter = owned_parameter(6, 0);
    let repeated = solve(
        TypeKind::Tuple(vec![
            TypeKind::GenericParam(parameter.clone()),
            TypeKind::GenericParam(parameter.clone()),
        ]),
        TypeKind::Choice(vec![
            TypeKind::Tuple(vec![TypeKind::I32, TypeKind::I32]),
            TypeKind::Tuple(vec![TypeKind::I32, TypeKind::I32]),
        ]),
    );
    assert!(repeated.is_ok());

    let divergent = solve(
        TypeKind::Tuple(vec![
            TypeKind::GenericParam(parameter.clone()),
            TypeKind::GenericParam(parameter),
        ]),
        TypeKind::Choice(vec![
            TypeKind::Tuple(vec![TypeKind::I32, TypeKind::I32]),
            TypeKind::Tuple(vec![TypeKind::String, TypeKind::String]),
        ]),
    );
    assert!(matches!(
        divergent,
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch),
        ))
    ));
}

#[test]
fn choice_to_choice_covers_each_supplied_alternative() {
    let accepted = solve(
        TypeKind::Choice(vec![TypeKind::I32, TypeKind::String]),
        TypeKind::Choice(vec![TypeKind::I32, TypeKind::String]),
    );
    assert!(accepted.is_ok());

    let rejected = solve(
        TypeKind::Choice(vec![TypeKind::I32]),
        TypeKind::Choice(vec![TypeKind::I32, TypeKind::String]),
    );
    assert!(matches!(
        rejected,
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch),
        ))
    ));
}

#[test]
fn rejected_choice_branch_does_not_leak_its_speculative_binding() {
    let parameter = owned_parameter(7, 0);
    let pattern = TypeKind::Choice(vec![
        TypeKind::Tuple(vec![TypeKind::GenericParam(parameter), TypeKind::I32]),
        TypeKind::Tuple(vec![TypeKind::String, TypeKind::String]),
    ]);
    let actual = TypeKind::Tuple(vec![TypeKind::String, TypeKind::String]);
    let cancellation = AtomicBool::new(false);
    let scope = TypeConstraintParameterScope::new([(
        owned_parameter(7, 0),
        TypeConstraintParameterEligibility::FutureEligible,
    )])
    .expect("future-eligible branch scope");
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1_024, 512, 128, 64),
            &cancellation,
            scope,
        ),
        None,
    );
    transaction.constrain(
        &pattern,
        &actual,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    let outcome = transaction
        .finish()
        .complete()
        .expect("the exact second alternative is accepted");
    assert_eq!(outcome.solution.bindings().count(), 0);
}

#[test]
fn rigid_scope_is_an_exact_atom_and_nonmatching_choice_branch_is_pruned() {
    let rigid = owned_parameter(3, 0);
    let cancellation = AtomicBool::new(false);
    let scope = TypeConstraintParameterScope::new([(
        rigid.clone(),
        TypeConstraintParameterEligibility::Rigid,
    )])
    .expect("scope");
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(128, 64, 16, 8),
            &cancellation,
            scope,
        ),
        None,
    );
    transaction.constrain(
        &TypeKind::GenericParam(rigid.clone()),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(matches!(
        transaction.finish().complete(),
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch),
        ))
    ));

    let scope = TypeConstraintParameterScope::new([(
        rigid.clone(),
        TypeConstraintParameterEligibility::Rigid,
    )])
    .expect("unique scope");
    let mut choice_transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(128, 64, 16, 8),
            &cancellation,
            scope,
        ),
        None,
    );
    choice_transaction.constrain(
        &TypeKind::Choice(vec![TypeKind::GenericParam(rigid), TypeKind::I32]),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    let outcome = choice_transaction
        .finish()
        .complete()
        .expect("the concrete alternative remains");
    assert_eq!(outcome.solution.bindings().count(), 0);
}

#[test]
fn acyclic_choice_sibling_survives_a_deferred_cycle() {
    let parameter = owned_parameter(13, 0);
    let pattern = TypeKind::Choice(vec![
        TypeKind::GenericParam(parameter.clone()),
        TypeKind::Vec(Box::new(TypeKind::GenericParam(parameter.clone()))),
    ]);
    let actual = TypeKind::Vec(Box::new(TypeKind::GenericParam(parameter.clone())));
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1_024, 512, 128, 64),
            &cancellation,
            TypeConstraintParameterScope::new([(
                parameter.clone(),
                TypeConstraintParameterEligibility::FutureEligible,
            )])
            .expect("deferred Choice scope"),
        ),
        None,
    );
    transaction.constrain(
        &pattern,
        &actual,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    let outcome = transaction
        .finish()
        .complete()
        .expect("the non-cyclic Choice sibling remains");
    assert_eq!(outcome.solution.bindings().count(), 0);
}

#[derive(Debug, Eq, PartialEq)]
enum InheritedFailureClass {
    Type(super::InheritedSolutionInvariantKind),
    Effect(super::TypeConstraintEffectInvariantKind),
}

fn inherited_failure_for(value: TypeKind) -> InheritedFailureClass {
    let parameter = owned_parameter(14, 0);
    let mut bindings = BTreeMap::new();
    bindings.insert(parameter, value);
    match try_type_only_solution(bindings, &[]) {
        Err(TypeConstraintError::Invariant(TypeConstraintInvariant::InheritedSolution(
            InheritedSolutionInvariant { kind, .. },
        ))) => InheritedFailureClass::Type(kind),
        Err(TypeConstraintError::Invariant(TypeConstraintInvariant::Effect(invariant))) => {
            InheritedFailureClass::Effect(invariant.kind)
        }
        other => panic!("expected typed completed-solution failure, got {other:?}"),
    }
}

#[test]
fn forbidden_completed_rows_are_solution_owner_invariants() {
    let unknown_effect_function = TypeKind::Function {
        params: Vec::new(),
        return_type: Box::new(TypeKind::I32),
        effects: EffectRow::unknown(),
    };
    assert_eq!(
        inherited_failure_for(unknown_effect_function),
        InheritedFailureClass::Effect(super::TypeConstraintEffectInvariantKind::UnknownRow)
    );
    assert_eq!(
        inherited_failure_for(TypeKind::Error(TypePoisonId::from_index(1))),
        InheritedFailureClass::Type(super::InheritedSolutionInvariantKind::Forbidden)
    );
    assert_eq!(
        inherited_failure_for(TypeKind::Projection {
            subject: Box::new(TypeKind::I32),
            trait_name: None,
            assoc: "Assoc".to_owned(),
        }),
        InheritedFailureClass::Type(super::InheritedSolutionInvariantKind::Forbidden)
    );
}

#[test]
fn completed_inherited_chain_is_rejected_by_the_solution_owner_without_repair() {
    let t = owned_parameter(15, 0);
    let u = owned_parameter(15, 1);
    let expected_rows = vec![
        (t.clone(), TypeKind::GenericParam(u.clone())),
        (u.clone(), TypeKind::I32),
    ];
    let failure: TypeConstraintFailure<NoConstraintClient> =
        try_type_only_solution(expected_rows, &[])
            .expect_err("a non-canonical completed carrier cannot be issued")
            .into();
    assert!(matches!(
        failure,
        TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
            TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                kind: super::InheritedSolutionInvariantKind::NonCanonical,
                parameter: Some(found),
            }),
        )) if found == t
    ));
}

#[test]
fn inherited_rigid_binding_is_an_initialization_invariant() {
    let parameter = owned_parameter(150, 0);
    let scope = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Rigid,
    )])
    .expect("rigid scope");
    let inherited = type_only_solution(BTreeMap::from([(parameter.clone(), TypeKind::I32)]), &[]);
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            scope,
        );
    let mut lower = TypeConstraintTransaction::<NoConstraintClient>::new();
    assert!(matches!(
        lower.initialize(&mut context, Some(Arc::new(inherited))),
        Err(TypeConstraintInitializationFailure::Invariant(
            TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                kind: InheritedSolutionInvariantKind::RigidBinding,
                parameter: Some(found),
            }),
        )) if found == parameter
    ));
}

#[test]
fn completed_solution_self_binding_and_occurs_cycle_are_distinct_invariants() {
    let parameter = owned_parameter(151, 0);
    assert!(matches!(
        try_type_only_solution(
            [(parameter.clone(), TypeKind::GenericParam(parameter.clone()),)],
            &[],
        ),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                kind: InheritedSolutionInvariantKind::SelfBinding,
                ..
            }),
        ))
    ));

    assert!(matches!(
        try_type_only_solution(
            [(
                parameter.clone(),
                TypeKind::Vec(Box::new(TypeKind::GenericParam(parameter))),
            )],
            &[],
        ),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                kind: InheritedSolutionInvariantKind::OccursOrCycle,
                ..
            }),
        ))
    ));
}

#[test]
fn inherited_invariant_algebra_retains_unordered_and_unclosed_kinds() {
    let kinds = [
        InheritedSolutionInvariantKind::DuplicateOrUnordered,
        InheritedSolutionInvariantKind::Unclosed,
    ];
    assert_eq!(kinds.len(), 2);
    assert_ne!(kinds[0], kinds[1]);
}

#[test]
fn sealed_call_scope_rejects_unordered_rows_and_invalid_required_keys() {
    let first = owned_parameter(160, 0);
    let second = owned_parameter(160, 1);
    assert!(matches!(
        TypeConstraintParameterScope::seal_call_scope(
            [
                super::context::TypeConstraintTypeParameterScopeRow::new(
                    second.clone(),
                    TypeConstraintParameterEligibility::Bindable,
                ),
                super::context::TypeConstraintTypeParameterScopeRow::new(
                    first.clone(),
                    TypeConstraintParameterEligibility::Bindable,
                ),
            ],
            std::iter::empty(),
            std::iter::empty(),
        ),
        Err(TypeConstraintInvariant::ParameterScope(
            TypeConstraintParameterScopeInvariant::ParameterUnordered
        ))
    ));
    assert!(matches!(
        TypeConstraintParameterScope::seal_call_scope(
            [super::context::TypeConstraintTypeParameterScopeRow::new(
                first.clone(),
                TypeConstraintParameterEligibility::FutureEligible,
            )],
            std::iter::empty(),
            [first],
        ),
        Err(TypeConstraintInvariant::ParameterScope(
            TypeConstraintParameterScopeInvariant::RequiredInheritedKeyNotBindable { .. }
        ))
    ));
}

#[test]
fn sealed_call_scope_distinguishes_duplicate_from_unordered_rows() {
    let first = owned_parameter(1600, 0);
    let second = owned_parameter(1600, 1);
    let row = |parameter| {
        super::context::TypeConstraintTypeParameterScopeRow::new(
            parameter,
            TypeConstraintParameterEligibility::Bindable,
        )
    };
    assert!(matches!(
        TypeConstraintParameterScope::seal_call_scope(
            [row(first.clone()), row(first)],
            std::iter::empty(),
            std::iter::empty(),
        ),
        Err(TypeConstraintInvariant::ParameterScope(
            TypeConstraintParameterScopeInvariant::DuplicateParameter,
        ))
    ));
    assert!(matches!(
        TypeConstraintParameterScope::seal_call_scope(
            [row(second), row(owned_parameter(1600, 0))],
            std::iter::empty(),
            std::iter::empty(),
        ),
        Err(TypeConstraintInvariant::ParameterScope(
            TypeConstraintParameterScopeInvariant::ParameterUnordered,
        ))
    ));
}

#[test]
fn required_inherited_missing_key_is_a_behavioral_unclosed_invariant() {
    let parameter = owned_parameter(161, 0);
    let scope = TypeConstraintParameterScope::seal_call_scope(
        [super::context::TypeConstraintTypeParameterScopeRow::new(
            parameter.clone(),
            TypeConstraintParameterEligibility::Bindable,
        )],
        std::iter::empty(),
        [parameter.clone()],
    )
    .expect("required inherited key scope");
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            scope,
        );
    let mut lower = TypeConstraintTransaction::<NoConstraintClient>::new();
    assert!(matches!(
        lower.initialize(&mut context, None),
        Err(TypeConstraintInitializationFailure::Invariant(
            TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                kind: InheritedSolutionInvariantKind::Unclosed,
                parameter: Some(found),
            })
        )) if found == parameter
    ));
}

#[test]
fn inherited_key_merge_classifies_canonical_extra_and_rigid_extra_exactly() {
    let required = owned_parameter(162, 0);
    let extra = owned_parameter(162, 1);
    let scope = TypeConstraintParameterScope::seal_call_scope(
        [
            super::context::TypeConstraintTypeParameterScopeRow::new(
                required.clone(),
                TypeConstraintParameterEligibility::Bindable,
            ),
            super::context::TypeConstraintTypeParameterScopeRow::new(
                extra.clone(),
                TypeConstraintParameterEligibility::Bindable,
            ),
        ],
        std::iter::empty(),
        [required.clone()],
    )
    .expect("required scope");
    let inherited = type_only_solution(
        BTreeMap::from([
            (required.clone(), TypeKind::I32),
            (extra.clone(), TypeKind::String),
        ]),
        &[],
    );
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            scope,
        );
    let mut lower = TypeConstraintTransaction::<NoConstraintClient>::new();
    assert!(matches!(
        lower.initialize(&mut context, Some(Arc::new(inherited))),
        Err(TypeConstraintInitializationFailure::Invariant(
            TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                kind: InheritedSolutionInvariantKind::UnexpectedKey,
                parameter: Some(found),
            })
        )) if found == extra
    ));

    let rigid = owned_parameter(162, 2);
    let scope = TypeConstraintParameterScope::seal_call_scope(
        [
            super::context::TypeConstraintTypeParameterScopeRow::new(
                required.clone(),
                TypeConstraintParameterEligibility::Bindable,
            ),
            super::context::TypeConstraintTypeParameterScopeRow::new(
                rigid.clone(),
                TypeConstraintParameterEligibility::Rigid,
            ),
        ],
        std::iter::empty(),
        [required.clone()],
    )
    .expect("required and rigid scope");
    let inherited = type_only_solution(
        BTreeMap::from([(required, TypeKind::I32), (rigid.clone(), TypeKind::String)]),
        &[],
    );
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            scope,
        );
    let mut lower = TypeConstraintTransaction::<NoConstraintClient>::new();
    assert!(matches!(
        lower.initialize(&mut context, Some(Arc::new(inherited))),
        Err(TypeConstraintInitializationFailure::Invariant(
            TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                kind: InheritedSolutionInvariantKind::RigidBinding,
                parameter: Some(found),
            })
        )) if found == rigid
    ));
}

#[test]
fn inherited_rigid_atom_and_rigid_projection_self_accept() {
    let bindable = owned_parameter(16, 0);
    let rigid = owned_parameter(16, 1);
    let inherited_scope = TypeConstraintParameterScope::seal_call_scope(
        [
            super::context::TypeConstraintTypeParameterScopeRow::new(
                bindable.clone(),
                TypeConstraintParameterEligibility::Bindable,
            ),
            super::context::TypeConstraintTypeParameterScopeRow::new(
                rigid.clone(),
                TypeConstraintParameterEligibility::Rigid,
            ),
        ],
        std::iter::empty(),
        [bindable.clone()],
    )
    .expect("rigid self scope");
    let mut bindings = BTreeMap::new();
    bindings.insert(bindable.clone(), TypeKind::GenericParam(rigid.clone()));
    let inherited = type_only_solution(bindings, &[]);
    let cancellation = AtomicBool::new(false);
    let transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1_024, 512, 128, 64),
            &cancellation,
            inherited_scope,
        ),
        Some(Arc::new(inherited)),
    );
    let inherited = transaction
        .finish()
        .complete()
        .expect("rigid identity is a valid sealed atom");
    assert!(
        inherited
            .solution
            .bindings()
            .any(|(parameter, value)| parameter == &bindable
                && value == &TypeKind::GenericParam(rigid.clone()))
    );

    let rigid_scope = TypeConstraintParameterScope::new([(
        rigid.clone(),
        TypeConstraintParameterEligibility::Rigid,
    )])
    .expect("rigid projection scope");
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1_024, 512, 128, 64),
            &cancellation,
            rigid_scope,
        ),
        None,
    );
    transaction.request_projection(
        (),
        &TypeKind::GenericParam(rigid.clone()),
        TypeConstraintProjectionClosure::Closed,
    );
    let projection = transaction
        .finish()
        .complete()
        .expect("rigid projection self-validates")
        .projections
        .into_iter()
        .next()
        .expect("one rigid projection");
    assert_eq!(projection.value(), &TypeKind::GenericParam(rigid));
}

#[test]
fn strict_final_projections_reject_forbidden_semantic_carriers() {
    let forbidden = [
        (
            TypeKind::Function {
                params: Vec::new(),
                return_type: Box::new(TypeKind::I32),
                effects: EffectRow::unknown(),
            },
            true,
        ),
        (TypeKind::Error(TypePoisonId::from_index(2)), false),
        (
            TypeKind::Projection {
                subject: Box::new(TypeKind::I32),
                trait_name: None,
                assoc: "Assoc".to_owned(),
            },
            false,
        ),
    ];
    for (value, effect_invariant) in forbidden {
        let cancellation = AtomicBool::new(false);
        let mut transaction = TestConstraintTransaction::begin(
            TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
                TypeConstraintLimits::new(1_024, 512, 128, 64),
                &cancellation,
                TypeConstraintParameterScope::empty(),
            ),
            None,
        );
        transaction.request_projection((), &value, TypeConstraintProjectionClosure::Closed);
        let outcome = transaction.finish().complete();
        if effect_invariant {
            assert!(matches!(
                outcome,
                Err(TypeConstraintFailure::Invariant(
                    TypeConstraintFailureInvariant::Constraint(TypeConstraintInvariant::Effect(
                        super::TypeConstraintEffectInvariant {
                            kind: super::TypeConstraintEffectInvariantKind::UnknownRow,
                            ..
                        }
                    ))
                ))
            ));
        } else {
            assert!(matches!(
                outcome,
                Err(TypeConstraintFailure::Invariant(
                    TypeConstraintFailureInvariant::Constraint(
                        TypeConstraintInvariant::Projection(
                            TypeConstraintProjectionInvariant::Mismatch
                        )
                    )
                ))
            ));
        }
    }
}

#[test]
fn foreign_binding_value_is_rejected_before_solution_publish() {
    let local = owned_parameter(200, 0);
    let foreign = owned_parameter(201, 0);
    let scope = TypeConstraintParameterScope::new([(
        local.clone(),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .expect("local scope");
    let inherited = type_only_solution(
        BTreeMap::from([(local, TypeKind::GenericParam(foreign))]),
        &[],
    );
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            scope,
        );
    let mut transaction = TypeConstraintTransaction::<NoConstraintClient>::new();
    assert!(matches!(
        transaction.initialize(&mut context, Some(Arc::new(inherited))),
        Err(TypeConstraintInitializationFailure::Invariant(
            TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                kind: super::InheritedSolutionInvariantKind::OutOfScope,
                ..
            }),
        ))
    ));
}

#[test]
fn foreign_nested_hint_is_rejected_before_source_callback_boundary() {
    let foreign = owned_parameter(202, 0);
    let expected = TypeKind::Vec(Box::new(TypeKind::GenericParam(foreign)));
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, SyntheticClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            TypeConstraintParameterScope::empty(),
        );
    let mut transaction = TypeConstraintTransaction::<SyntheticClient>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::checked(
                1,
                PreparedConstraintSourceProjection::Scalar,
                [],
                PreparedSourceAlternative::new(0, 0, expected.clone()),
            )
            .expect("prepared foreign hint"),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("begin foreign hint probe");
    assert!(matches!(
        transaction.next_probe(&mut context),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::ParameterScope(
                TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope { .. },
            )
        ))
    ));
}

#[test]
fn foreign_equal_generic_self_relation_is_fail_closed() {
    let foreign = owned_parameter(203, 0);
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            TypeConstraintParameterScope::empty(),
        ),
        None,
    );
    transaction.constrain(
        &TypeKind::GenericParam(foreign.clone()),
        &TypeKind::GenericParam(foreign),
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(matches!(
        transaction.finish().complete(),
        Err(TypeConstraintFailure::Invariant(
            TypeConstraintFailureInvariant::Constraint(TypeConstraintInvariant::ParameterScope(
                TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope { .. },
            )),
        ))
    ));
}

#[test]
fn foreign_array_length_generic_is_fail_closed() {
    let foreign = GenericConstParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(204)),
        0,
    );
    let array = TypeKind::Array {
        item: Box::new(TypeKind::I32),
        len: super::super::ArrayLength::Generic(foreign),
    };
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            TypeConstraintParameterScope::empty(),
        ),
        None,
    );
    transaction.constrain(&array, &array, ConstraintAcceptance::PatternAcceptsActual);
    assert!(matches!(
        transaction.finish().complete(),
        Err(TypeConstraintFailure::Invariant(
            TypeConstraintFailureInvariant::Constraint(TypeConstraintInvariant::ParameterScope(
                TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope { .. },
            )),
        ))
    ));
}

#[test]
fn canonical_inherited_type_extension_normalizes_then_terminal_rejects_unresolved() {
    let first = owned_parameter(205, 0);
    let second = owned_parameter(205, 1);
    let scope = TypeConstraintParameterScope::seal_call_scope(
        [
            super::context::TypeConstraintTypeParameterScopeRow::new(
                first.clone(),
                TypeConstraintParameterEligibility::Bindable,
            ),
            super::context::TypeConstraintTypeParameterScopeRow::new(
                second.clone(),
                TypeConstraintParameterEligibility::Bindable,
            ),
        ],
        std::iter::empty(),
        [first.clone()],
    )
    .expect("extension scope");
    let inherited = type_only_solution(
        BTreeMap::from([(first.clone(), TypeKind::GenericParam(second.clone()))]),
        std::slice::from_ref(&second),
    );
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(512, 256, 64, 16),
            &cancellation,
            scope,
        ),
        Some(Arc::new(inherited)),
    );
    transaction.constrain(
        &TypeKind::GenericParam(second.clone()),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    let outcome = transaction
        .finish()
        .complete()
        .expect("extended inherited solution");
    assert_eq!(
        outcome.solution.bindings().collect::<Vec<_>>(),
        vec![(&first, &TypeKind::I32), (&second, &TypeKind::I32)]
    );

    let unresolved = type_only_solution(
        BTreeMap::from([(first, TypeKind::GenericParam(second.clone()))]),
        std::slice::from_ref(&second),
    );
    let unresolved_scope = TypeConstraintParameterScope::seal_call_scope(
        [
            super::context::TypeConstraintTypeParameterScopeRow::new(
                owned_parameter(205, 0),
                TypeConstraintParameterEligibility::Bindable,
            ),
            super::context::TypeConstraintTypeParameterScopeRow::new(
                owned_parameter(205, 1),
                TypeConstraintParameterEligibility::Bindable,
            ),
        ],
        std::iter::empty(),
        [owned_parameter(205, 0)],
    )
    .expect("unresolved extension scope");
    let transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(512, 256, 64, 16),
            &cancellation,
            unresolved_scope,
        ),
        Some(Arc::new(unresolved)),
    );
    let unresolved_outcome = transaction.finish().complete();
    assert!(
        matches!(
            &unresolved_outcome,
            Err(TypeConstraintFailure::Rejected(
                TypeConstraintCandidateFailure::Constraint(
                    TypeConstraintRejection::IncompleteInstantiation { .. },
                ),
            ))
        ),
        "unexpected unresolved completion outcome: {unresolved_outcome:?}"
    );
}

#[test]
fn canonical_inherited_binding_closes_through_current_group_constraint() {
    let t = owned_parameter(208, 0);
    let u = owned_parameter(208, 1);
    let scope = TypeConstraintParameterScope::seal_call_scope(
        [
            super::context::TypeConstraintTypeParameterScopeRow::new(
                t.clone(),
                TypeConstraintParameterEligibility::Bindable,
            ),
            super::context::TypeConstraintTypeParameterScopeRow::new(
                u.clone(),
                TypeConstraintParameterEligibility::Bindable,
            ),
        ],
        std::iter::empty(),
        [t.clone()],
    )
    .expect("canonical inherited scope");
    let inherited = type_only_solution(
        BTreeMap::from([(t.clone(), TypeKind::GenericParam(u.clone()))]),
        std::slice::from_ref(&u),
    );
    assert_eq!(
        inherited.bindings().collect::<Vec<_>>(),
        vec![(&t, &TypeKind::GenericParam(u.clone()))],
        "the previous group seals a canonical edge to its future frontier"
    );
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(512, 256, 64, 16),
            &cancellation,
            scope,
        ),
        Some(Arc::new(inherited)),
    );
    transaction.constrain(
        &TypeKind::GenericParam(u.clone()),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );

    let outcome = transaction
        .finish()
        .complete()
        .expect("current group closes the inherited chain");
    assert_eq!(
        outcome.solution.bindings().collect::<Vec<_>>(),
        vec![(&t, &TypeKind::I32), (&u, &TypeKind::I32)]
    );
}

#[test]
fn inherited_key_cannot_be_replaced_by_a_later_group_constraint() {
    let parameter = owned_parameter(207, 0);
    let scope = TypeConstraintParameterScope::seal_call_scope(
        [super::context::TypeConstraintTypeParameterScopeRow::new(
            parameter.clone(),
            TypeConstraintParameterEligibility::Bindable,
        )],
        std::iter::empty(),
        [parameter.clone()],
    )
    .expect("replacement scope");
    let inherited = type_only_solution(BTreeMap::from([(parameter.clone(), TypeKind::I32)]), &[]);
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            scope,
        ),
        Some(Arc::new(inherited)),
    );
    transaction.constrain(
        &TypeKind::GenericParam(parameter),
        &TypeKind::String,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(matches!(
        transaction.finish().complete(),
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch),
        ))
    ));
}

#[test]
fn inherited_future_symbol_survives_for_the_exact_continuation_scope() {
    let bound = owned_parameter(206, 0);
    let future = owned_parameter(206, 1);
    let scope = TypeConstraintParameterScope::seal_call_scope(
        [
            super::context::TypeConstraintTypeParameterScopeRow::new(
                bound.clone(),
                TypeConstraintParameterEligibility::Bindable,
            ),
            super::context::TypeConstraintTypeParameterScopeRow::new(
                future.clone(),
                TypeConstraintParameterEligibility::FutureEligible,
            ),
        ],
        std::iter::empty(),
        [bound.clone()],
    )
    .expect("continuation scope");
    let inherited = type_only_solution(
        BTreeMap::from([(bound.clone(), TypeKind::GenericParam(future.clone()))]),
        std::slice::from_ref(&future),
    );
    let cancellation = AtomicBool::new(false);
    let transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(512, 256, 64, 16),
            &cancellation,
            scope,
        ),
        Some(Arc::new(inherited)),
    );
    let outcome = transaction
        .finish()
        .complete()
        .expect("future symbol remains owned by the exact continuation scope");
    assert_eq!(
        outcome.solution.bindings().collect::<Vec<_>>(),
        vec![(&bound, &TypeKind::GenericParam(future))]
    );
}

#[test]
fn hint_projection_cancellation_is_checked_before_descent() {
    let cancellation = AtomicBool::new(true);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            TypeConstraintParameterScope::empty(),
        );
    assert!(matches!(
        super::normalization::project_type(
            &TypeKind::Vec(Box::new(TypeKind::I32)),
            &BTreeMap::new(),
            super::ConstraintClosurePolicy::Hint,
            &mut context,
        ),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
}

#[test]
fn final_projection_cancellation_is_checked_before_descent() {
    let cancellation = AtomicBool::new(true);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 32, 16),
            &cancellation,
            TypeConstraintParameterScope::empty(),
        );
    assert!(matches!(
        super::normalization::project_type(
            &TypeKind::I32,
            &BTreeMap::new(),
            super::ConstraintClosurePolicy::ProjectionClosed,
            &mut context,
        ),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
}

#[test]
fn projected_type_node_limit_accepts_exact_and_rejects_one_over() {
    let cancellation = AtomicBool::new(false);
    let mut exact =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1, 1, 1, 1),
            &cancellation,
            TypeConstraintParameterScope::empty(),
        );
    assert!(
        super::normalization::project_type(
            &TypeKind::I32,
            &BTreeMap::new(),
            super::ConstraintClosurePolicy::Hint,
            &mut exact,
        )
        .is_ok()
    );

    let mut one_over =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1, 0, 1, 1),
            &cancellation,
            TypeConstraintParameterScope::empty(),
        );
    assert!(matches!(
        super::normalization::project_type(
            &TypeKind::I32,
            &BTreeMap::new(),
            super::ConstraintClosurePolicy::Hint,
            &mut one_over,
        ),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
            actual: 1,
            limit: 0,
        }))
    ));
}

#[test]
fn concrete_and_generic_array_projection_charge_exactly_three_nodes() {
    let cancellation = AtomicBool::new(false);
    let constant = GenericConstParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(208)),
        0,
    );
    let generic_scope = TypeConstraintParameterScope::new_with_constants(
        std::iter::empty::<(GenericTypeParameterId, TypeConstraintParameterEligibility)>(),
        [(constant.clone(), TypeConstraintConstEligibility::Rigid)],
    )
    .expect("rigid generic array scope");
    let cases = [
        (
            TypeKind::Array {
                item: Box::new(TypeKind::I32),
                len: super::super::ArrayLength::Const(3),
            },
            TypeConstraintParameterScope::empty(),
        ),
        (
            TypeKind::Array {
                item: Box::new(TypeKind::I32),
                len: super::super::ArrayLength::Generic(constant),
            },
            generic_scope,
        ),
    ];
    let policies = [
        super::ConstraintClosurePolicy::Hint,
        super::ConstraintClosurePolicy::ProjectionClosed,
        super::ConstraintClosurePolicy::ProjectionFuture,
        super::ConstraintClosurePolicy::SolutionCompletion,
    ];
    for (array, scope) in cases {
        for policy in policies {
            let mut exact = TypeConstraintContext::<
                LocalConstraintAccounting<'_>,
                NoConstraintClient,
            >::with_scope(
                TypeConstraintLimits::new(16, 3, 4, 4),
                &cancellation,
                scope.clone(),
            );
            assert!(
                super::normalization::project_type(&array, &BTreeMap::new(), policy, &mut exact,)
                    .is_ok()
            );

            let mut one_over = TypeConstraintContext::<
                LocalConstraintAccounting<'_>,
                NoConstraintClient,
            >::with_scope(
                TypeConstraintLimits::new(16, 2, 4, 4),
                &cancellation,
                scope.clone(),
            );
            assert!(matches!(
                super::normalization::project_type(&array, &BTreeMap::new(), policy, &mut one_over,),
                Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
                    actual: 3,
                    limit: 2,
                }))
            ));
        }
    }
}

#[test]
fn unresolved_array_length_charges_container_and_header_before_rejection() {
    let cancellation = AtomicBool::new(false);
    let cases = [
        super::super::ArrayLength::Error(TypePoisonId::from_index(209)),
        super::super::ArrayLength::Inferred,
    ];
    let policies = [
        super::ConstraintClosurePolicy::Hint,
        super::ConstraintClosurePolicy::ProjectionClosed,
        super::ConstraintClosurePolicy::ProjectionFuture,
        super::ConstraintClosurePolicy::SolutionCompletion,
    ];
    for length in cases {
        let array = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: length,
        };
        for policy in policies {
            let mut exact = TypeConstraintContext::<
                LocalConstraintAccounting<'_>,
                NoConstraintClient,
            >::with_scope(
                TypeConstraintLimits::new(16, 2, 4, 4),
                &cancellation,
                TypeConstraintParameterScope::empty(),
            );
            assert!(matches!(
                super::normalization::project_type(&array, &BTreeMap::new(), policy, &mut exact,),
                Err(TypeConstraintError::Rejected(
                    TypeConstraintRejection::UnresolvedType
                ))
            ));

            let mut one_over = TypeConstraintContext::<
                LocalConstraintAccounting<'_>,
                NoConstraintClient,
            >::with_scope(
                TypeConstraintLimits::new(16, 1, 4, 4),
                &cancellation,
                TypeConstraintParameterScope::empty(),
            );
            assert!(matches!(
                super::normalization::project_type(&array, &BTreeMap::new(), policy, &mut one_over,),
                Err(TypeConstraintError::Abort(TypeConstraintAbort::NodeLimit {
                    actual: 2,
                    limit: 1,
                }))
            ));
        }
    }
}

struct CancelAfterContainer<'a> {
    cancellation: &'a AtomicBool,
    limits: TypeConstraintLimits,
}

impl TypeConstraintAccounting for CancelAfterContainer<'_> {
    fn charge_constraint(
        &mut self,
        delta: &TypeConstraintWorkReport,
        _limits: TypeConstraintLimits,
    ) -> Result<(), TypeConstraintError> {
        if delta.nodes() != 0 {
            self.cancellation.store(true, Ordering::Release);
        }
        Ok(())
    }

    fn commit(&mut self) {}
}

impl<'a> TypeConstraintContextIssuer<'a> for CancelAfterContainer<'a> {
    fn context_limits(&self) -> TypeConstraintLimits {
        self.limits
    }

    fn context_cancellation(&self) -> &'a AtomicBool {
        self.cancellation
    }
}

#[test]
fn array_header_observes_cancellation_after_container_charge() {
    let cancellation = AtomicBool::new(false);
    let accounting = CancelAfterContainer {
        cancellation: &cancellation,
        limits: TypeConstraintLimits::new(16, 4, 4, 4),
    };
    let mut context =
        TypeConstraintContext::<CancelAfterContainer<'_>, NoConstraintClient>::with_accounting(
            accounting,
            TypeConstraintParameterScope::empty(),
            TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty test effect scope is canonical"),
        );
    let array = TypeKind::Array {
        item: Box::new(TypeKind::I32),
        len: super::super::ArrayLength::Const(3),
    };
    assert!(matches!(
        super::normalization::project_type(
            &array,
            &BTreeMap::new(),
            super::ConstraintClosurePolicy::Hint,
            &mut context,
        ),
        Err(TypeConstraintError::Abort(TypeConstraintAbort::Cancelled))
    ));
}

#[derive(Debug)]
struct SyntheticClient {
    probes: Arc<AtomicUsize>,
    materializations: Arc<AtomicUsize>,
    cancellation: Arc<AtomicBool>,
    reject_probe: bool,
}

impl ConstraintDomain for SyntheticClient {
    type Source = u8;
    type AlternativeIndex = u8;
    type EvidenceRule = u8;
    type CheckedEvidence = u8;
    type ProbeSemanticBranch = u8;
    type SealedBranchValue = u8;
    type Projection = u8;
    type SourceErrorCause = &'static str;
    type ClientInvariant = ();

    fn evidence_accepts(rule: &Self::EvidenceRule, checked: &Self::CheckedEvidence) -> bool {
        rule == checked
    }

    fn project_checked_evidence(
        checked: &Self::CheckedEvidence,
        _: &TypeKind,
    ) -> Option<Self::CheckedEvidence> {
        Some(*checked)
    }

    fn alternative_ordinal(index: &Self::AlternativeIndex) -> u32 {
        u32::from(*index)
    }

    fn client_invariant_source(_: &Self::ClientInvariant) -> Self::Source {
        0
    }

    fn empty_sealed_branch() -> Self::SealedBranchValue {
        0
    }
}

impl SyntheticClient {
    fn probe(&mut self, ticket: &ProbeTicket<Self>) -> ProbeSubmission<Self> {
        self.probes.fetch_add(1, Ordering::Relaxed);
        if self.reject_probe {
            return ProbeSubmission::Rejected("probe rejected");
        }
        ticket.with_hint(|hint| assert!(matches!(hint, ExpectedHint::Alternatives(_))));
        ProbeSubmission::Accepted(SourceProbeResult::checked(
            TypeKind::I32,
            self.probes.load(Ordering::Relaxed) as u8,
            0,
            0,
        ))
    }

    fn materialize(
        &mut self,
        _ticket: &MaterializationTicket<Self>,
    ) -> ClosedMaterializationSubmission<Self> {
        self.materializations.fetch_add(1, Ordering::Relaxed);
        if self.cancellation.load(Ordering::Relaxed) {
            return ClosedMaterializationSubmission::Rejected {
                source: 0,
                cause: "materialize rejected",
            };
        }
        ClosedMaterializationSubmission::Sealed(7)
    }
}

#[test]
fn source_probe_runs_once_per_frontier_row_and_materializes_projection() {
    let first = owned_parameter(4, 0);
    let second = owned_parameter(4, 1);
    let probes = Arc::new(AtomicUsize::new(0));
    let materializations = Arc::new(AtomicUsize::new(0));
    let cancellation = AtomicBool::new(false);
    let scope = TypeConstraintParameterScope::new([
        (first.clone(), TypeConstraintParameterEligibility::Bindable),
        (second.clone(), TypeConstraintParameterEligibility::Bindable),
    ])
    .expect("source test scope");
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, SyntheticClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(2_048, 512, 128, 64),
                &cancellation,
            ),
            scope,
            TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty test effect scope is canonical"),
        );
    let mut transaction = TypeConstraintTransaction::<SyntheticClient>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction.constrain(
        &mut context,
        &TypeKind::Choice(vec![
            TypeKind::GenericParam(first),
            TypeKind::GenericParam(second),
        ]),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::checked(
                9,
                PreparedConstraintSourceProjection::Scalar,
                [],
                PreparedSourceAlternative::new(0, 0, TypeKind::I32),
            )
            .expect("prepared source"),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("begin probe");
    let mut client = SyntheticClient {
        probes: probes.clone(),
        materializations: materializations.clone(),
        cancellation: Arc::new(AtomicBool::new(false)),
        reject_probe: false,
    };
    while let Some(ticket) = transaction.next_probe(&mut context).expect("probe ticket") {
        let submission = client.probe(&ticket);
        transaction
            .submit_probe(&mut context, ticket, submission)
            .expect("submit probe");
    }
    transaction.request_projection(5, &TypeKind::I32, TypeConstraintProjectionClosure::Closed);
    while let Some(mut ticket) = transaction
        .next_materialization_ticket(&mut context)
        .expect("materialization ticket")
    {
        let submission = client.materialize(&ticket);
        let closed = close_materialization_ticket(&mut ticket, submission);
        transaction
            .submit_closed_materialization(ticket, closed)
            .expect("submit materialization");
    }
    let outcome = transaction.finish(context).complete();
    assert!(matches!(
        outcome,
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(
                TypeConstraintRejection::AmbiguousSolution { .. },
            ),
        ))
    ));
    assert_eq!(probes.load(Ordering::Relaxed), 2);
    assert_eq!(materializations.load(Ordering::Relaxed), 2);
}

#[test]
fn source_rejection_is_typed_and_does_not_use_fatal_phase() {
    let probes = Arc::new(AtomicUsize::new(0));
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, SyntheticClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(256, 128, 16, 8),
                &cancellation,
            ),
            TypeConstraintParameterScope::empty(),
            TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty test effect scope is canonical"),
        );
    let mut transaction = TypeConstraintTransaction::<SyntheticClient>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::checked(
                1,
                PreparedConstraintSourceProjection::Scalar,
                [],
                PreparedSourceAlternative::new(0, 0, TypeKind::I32),
            )
            .expect("prepared source"),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("begin probe");
    let mut client = SyntheticClient {
        probes: probes.clone(),
        materializations: Arc::new(AtomicUsize::new(0)),
        cancellation: Arc::new(AtomicBool::new(false)),
        reject_probe: true,
    };
    while let Some(ticket) = transaction.next_probe(&mut context).expect("probe ticket") {
        let submission = client.probe(&ticket);
        transaction
            .submit_probe(&mut context, ticket, submission)
            .expect("submit probe");
    }
    let outcome = transaction.finish(context).complete();
    assert_eq!(probes.load(Ordering::Relaxed), 1);
    assert!(matches!(
        outcome,
        Err(TypeConstraintFailure::Rejected(TypeConstraintCandidateFailure::Source(error)))
            if error.phase() == SourcePhase::Probe
                && error.source() == &1
                && error.cause().as_ref() == &["probe rejected"]
    ));
}

#[test]
fn materialization_processes_every_trace_and_earliest_source_fatal_wins() {
    let first = owned_parameter(17, 0);
    let second = owned_parameter(17, 1);
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, SyntheticClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(4_096, 1_024, 256, 128).with_source_limits(64, 64),
                &cancellation,
            ),
            TypeConstraintParameterScope::new([
                (first.clone(), TypeConstraintParameterEligibility::Bindable),
                (second.clone(), TypeConstraintParameterEligibility::Bindable),
            ])
            .expect("materialization scope"),
            TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty test effect scope is canonical"),
        );
    let mut transaction = TypeConstraintTransaction::<SyntheticClient>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction.constrain(
        &mut context,
        &TypeKind::Choice(vec![
            TypeKind::GenericParam(first),
            TypeKind::GenericParam(second),
        ]),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    for source in [10_u8, 20_u8] {
        transaction
            .begin_prepared_probe(
                &mut context,
                PreparedSourceConstraint::checked(
                    source,
                    PreparedConstraintSourceProjection::Scalar,
                    [],
                    PreparedSourceAlternative::new(0, 0, TypeKind::I32),
                )
                .expect("prepared source"),
                ConstraintAcceptance::PatternAcceptsActual,
            )
            .expect("begin authored source");
        while let Some(ticket) = transaction.next_probe(&mut context).expect("probe ticket") {
            transaction
                .submit_probe(
                    &mut context,
                    ticket,
                    ProbeSubmission::Accepted(SourceProbeResult::checked(
                        TypeKind::I32,
                        source,
                        0,
                        0,
                    )),
                )
                .expect("accepted source");
        }
    }

    let mut traces = 0;
    while let Some(mut ticket) = transaction
        .next_materialization_ticket(&mut context)
        .expect("materialization ticket")
    {
        let error = if traces == 0 {
            SourceError::new(20, SourcePhase::Materialize, "later source")
        } else {
            SourceError::new(10, SourcePhase::Materialize, "earlier source")
        };
        traces += 1;
        let closed = close_materialization_ticket(
            &mut ticket,
            ClosedMaterializationSubmission::Fatal(error),
        );
        transaction
            .submit_closed_materialization(ticket, closed)
            .expect("fatal materialization is retained");
    }
    assert_eq!(traces, 2, "fatal must not stop later trace materialization");
    match transaction.finish(context).complete() {
        Err(TypeConstraintFailure::FatalSource(error)) => {
            assert_eq!(error.source(), &10);
            assert_eq!(error.cause(), &"earlier source");
        }
        other => panic!("expected earliest authored fatal source, got {other:?}"),
    }
}

#[test]
fn materialization_rejections_aggregate_only_the_earliest_source() {
    let parameters = [
        owned_parameter(18, 0),
        owned_parameter(18, 1),
        owned_parameter(18, 2),
    ];
    let cancellation = AtomicBool::new(false);
    let scope = TypeConstraintParameterScope::new(
        parameters
            .iter()
            .cloned()
            .map(|parameter| (parameter, TypeConstraintParameterEligibility::Bindable)),
    )
    .expect("rejection scope");
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, SyntheticClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(4_096, 1_024, 256, 128).with_source_limits(64, 64),
                &cancellation,
            ),
            scope,
            TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty test effect scope is canonical"),
        );
    let mut transaction = TypeConstraintTransaction::<SyntheticClient>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction.constrain(
        &mut context,
        &TypeKind::Choice(
            parameters
                .iter()
                .cloned()
                .map(TypeKind::GenericParam)
                .collect(),
        ),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    for source in [10_u8, 20_u8] {
        transaction
            .begin_prepared_probe(
                &mut context,
                PreparedSourceConstraint::checked(
                    source,
                    PreparedConstraintSourceProjection::Scalar,
                    [],
                    PreparedSourceAlternative::new(0, 0, TypeKind::I32),
                )
                .expect("prepared source"),
                ConstraintAcceptance::PatternAcceptsActual,
            )
            .expect("begin authored source");
        while let Some(ticket) = transaction.next_probe(&mut context).expect("probe ticket") {
            transaction
                .submit_probe(
                    &mut context,
                    ticket,
                    ProbeSubmission::Accepted(SourceProbeResult::checked(
                        TypeKind::I32,
                        source,
                        0,
                        0,
                    )),
                )
                .expect("accepted source");
        }
    }

    let mut traces = 0;
    while let Some(mut ticket) = transaction
        .next_materialization_ticket(&mut context)
        .expect("materialization ticket")
    {
        let (source, cause) = match traces {
            0 | 1 => (
                10,
                if traces == 0 {
                    "first cause"
                } else {
                    "second cause"
                },
            ),
            _ => (20, "other source"),
        };
        traces += 1;
        let closed = close_materialization_ticket(
            &mut ticket,
            ClosedMaterializationSubmission::Rejected { source, cause },
        );
        transaction
            .submit_closed_materialization(ticket, closed)
            .expect("rejection is retained");
    }
    assert_eq!(traces, 3);
    match transaction.finish(context).complete() {
        Err(TypeConstraintFailure::Rejected(TypeConstraintCandidateFailure::Source(error))) => {
            assert_eq!(error.source(), &10);
            assert_eq!(error.cause().as_ref(), &["first cause", "second cause"]);
        }
        other => panic!("expected earliest source rejection aggregate, got {other:?}"),
    }
}

#[test]
fn zero_materialization_limit_precedes_ticket_sealing_and_callback() {
    let materializations = Arc::new(AtomicUsize::new(0));
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, SyntheticClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(1_024, 256, 64, 32).with_source_limits(8, 0),
                &cancellation,
            ),
            TypeConstraintParameterScope::empty(),
            TypeConstraintEffectScope::seal_call_scope([], [])
                .expect("empty test effect scope is canonical"),
        );
    let mut transaction = TypeConstraintTransaction::<SyntheticClient>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction.constrain(
        &mut context,
        &TypeKind::I32,
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::checked(
                3,
                PreparedConstraintSourceProjection::Scalar,
                [],
                PreparedSourceAlternative::new(0, 0, TypeKind::I32),
            )
            .expect("prepared source"),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("begin probe");
    while let Some(ticket) = transaction.next_probe(&mut context).expect("probe ticket") {
        transaction
            .submit_probe(
                &mut context,
                ticket,
                ProbeSubmission::Accepted(SourceProbeResult::checked(TypeKind::I32, 3, 0, 0)),
            )
            .expect("accepted source");
    }
    let ticket = transaction
        .next_materialization_ticket(&mut context)
        .expect("lower preparation does not charge materialization")
        .expect("lower still yields the ready ticket");
    assert_eq!(ticket.requests().len(), 1);
    assert_eq!(materializations.load(Ordering::Relaxed), 0);
}

#[test]
fn keyed_projection_is_sorted_and_closed_after_unique_pair() {
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::new(TypeConstraintLimits::new(256, 128, 16, 8), &cancellation),
        None,
    );
    transaction.request_projection((), &TypeKind::I32, TypeConstraintProjectionClosure::Closed);
    let outcome = transaction.finish().complete().expect("closed projection");
    assert_eq!(outcome.projections.len(), 1);
    assert_eq!(outcome.projections[0].key(), &());
    assert_eq!(outcome.projections[0].value(), &TypeKind::I32);
}

#[test]
fn keyed_projection_duplicate_is_a_typed_invariant() {
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::new(TypeConstraintLimits::new(256, 128, 16, 8), &cancellation),
        None,
    );
    transaction.request_projection((), &TypeKind::I32, TypeConstraintProjectionClosure::Closed);
    transaction.request_projection((), &TypeKind::I64, TypeConstraintProjectionClosure::Closed);
    assert!(matches!(
        transaction.finish().complete(),
        Err(TypeConstraintFailure::Invariant(
            TypeConstraintFailureInvariant::Constraint(TypeConstraintInvariant::Projection(
                TypeConstraintProjectionInvariant::DuplicateKey,
            ))
        ))
    ));
}

#[test]
fn closed_projection_rejects_future_eligible_row_with_typed_mismatch() {
    let future = owned_parameter(50, 0);
    let cancellation = AtomicBool::new(false);
    let scope = TypeConstraintParameterScope::new([(
        future.clone(),
        TypeConstraintParameterEligibility::FutureEligible,
    )])
    .expect("projection scope");
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 16, 8),
            &cancellation,
            scope,
        ),
        None,
    );
    transaction.request_projection(
        (),
        &TypeKind::GenericParam(future),
        TypeConstraintProjectionClosure::Closed,
    );
    assert!(matches!(
        transaction.finish().complete(),
        Err(TypeConstraintFailure::Invariant(
            TypeConstraintFailureInvariant::Constraint(TypeConstraintInvariant::Projection(
                TypeConstraintProjectionInvariant::Mismatch,
            ))
        ))
    ));
}

#[test]
fn future_projection_allows_only_future_eligible_rows() {
    let future = owned_parameter(5, 0);
    let cancellation = AtomicBool::new(false);
    let scope = TypeConstraintParameterScope::new([(
        future.clone(),
        TypeConstraintParameterEligibility::FutureEligible,
    )])
    .expect("future scope");
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(256, 128, 16, 8),
            &cancellation,
            scope,
        ),
        None,
    );
    transaction.request_projection(
        (),
        &TypeKind::GenericParam(future.clone()),
        TypeConstraintProjectionClosure::AllowFutureEligible,
    );
    let outcome = transaction.finish().complete().expect("future projection");
    assert_eq!(
        outcome.projections[0].value(),
        &TypeKind::GenericParam(future)
    );
}

#[test]
fn final_selected_call_rejects_non_unique_choice_injection() {
    let outcome = solve(
        TypeKind::Choice(vec![TypeKind::ActionName, TypeKind::String]),
        TypeKind::String,
    );
    assert!(matches!(
        outcome,
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch),
        ))
    ));
}

#[test]
fn final_equation_replays_choice_after_later_binding() {
    let parameter = owned_parameter(10, 0);
    let pattern = TypeKind::Choice(vec![
        TypeKind::GenericParam(parameter.clone()),
        TypeKind::I32,
    ]);
    let actual = TypeKind::I32;
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1_024, 512, 128, 64),
            &cancellation,
            bindable_scope(&[&pattern, &actual]),
        ),
        None,
    );
    transaction.constrain(
        &pattern,
        &actual,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    transaction.constrain(
        &TypeKind::GenericParam(parameter),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(matches!(
        transaction.finish().complete(),
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch),
        ))
    ));
}

#[test]
fn deferred_cycle_is_reported_only_after_close() {
    let parameter = owned_parameter(11, 0);
    let pattern = TypeKind::GenericParam(parameter.clone());
    let actual = TypeKind::Vec(Box::new(TypeKind::GenericParam(parameter.clone())));
    let cancellation = AtomicBool::new(false);
    let mut transaction = TestConstraintTransaction::begin(
        TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
            TypeConstraintLimits::new(1_024, 512, 128, 64),
            &cancellation,
            bindable_scope(&[&pattern, &actual]),
        ),
        None,
    );
    transaction.constrain(
        &pattern,
        &actual,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(matches!(
        transaction.finish().complete(),
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(
                TypeConstraintRejection::CyclicInstantiation { .. },
            ),
        ))
    ));
}

#[derive(Eq, PartialEq)]
struct PreparedBranch(&'static str);

#[derive(Clone, Eq, PartialEq)]
enum PreparedRule {
    Tag(u8),
    Otherwise,
}

#[derive(Eq, PartialEq)]
struct PreparedSealed(u8);

#[derive(Debug)]
struct PreparedDomain;

impl ConstraintDomain for PreparedDomain {
    type Source = u8;
    type AlternativeIndex = u8;
    type EvidenceRule = PreparedRule;
    type CheckedEvidence = PreparedRule;
    type ProbeSemanticBranch = PreparedBranch;
    type SealedBranchValue = PreparedSealed;
    type Projection = u8;
    type SourceErrorCause = &'static str;
    type ClientInvariant = ();

    fn evidence_accepts(rule: &Self::EvidenceRule, checked: &Self::CheckedEvidence) -> bool {
        match (rule, checked) {
            (PreparedRule::Otherwise, _) => true,
            (PreparedRule::Tag(expected), PreparedRule::Tag(actual)) => expected == actual,
            _ => false,
        }
    }

    fn project_checked_evidence(
        checked: &Self::CheckedEvidence,
        _: &TypeKind,
    ) -> Option<Self::CheckedEvidence> {
        Some(checked.clone())
    }

    fn alternative_ordinal(index: &Self::AlternativeIndex) -> u32 {
        u32::from(*index)
    }

    fn client_invariant_source(_: &Self::ClientInvariant) -> Self::Source {
        0
    }

    fn empty_sealed_branch() -> Self::SealedBranchValue {
        PreparedSealed(0)
    }
}

fn prepared_context<'a>(
    cancellation: &'a AtomicBool,
) -> TypeConstraintContext<'a, LocalConstraintAccounting<'a>, PreparedDomain> {
    TypeConstraintContext::<LocalConstraintAccounting<'a>, PreparedDomain>::with_scope(
        TypeConstraintLimits::new(1_024, 512, 128, 64).with_source_limits(64, 64),
        cancellation,
        TypeConstraintParameterScope::empty(),
    )
}

fn checked_prepared_source(
    source: u8,
    projection: PreparedConstraintSourceProjection,
) -> PreparedSourceConstraint<PreparedDomain> {
    PreparedSourceConstraint::checked(
        source,
        projection,
        [PreparedSourceAlternative::new(
            0,
            PreparedRule::Tag(1),
            TypeKind::I32,
        )],
        PreparedSourceAlternative::new(1, PreparedRule::Otherwise, TypeKind::I32),
    )
    .expect("prepared alternatives are schema ordered")
}

#[test]
fn prepared_source_hints_and_checked_evidence_retain_one_selected_alternative() {
    let cancellation = AtomicBool::new(false);
    let mut context = prepared_context(&cancellation);
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction
        .begin_prepared_probe(
            &mut context,
            checked_prepared_source(7, PreparedConstraintSourceProjection::Scalar),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("prepared probe starts");
    let ticket = transaction
        .next_probe(&mut context)
        .expect("hint projection")
        .expect("one source row");
    ticket.with_hint(|hint| {
        let ExpectedHint::Alternatives(rows) = hint else {
            panic!("checked source must expose keyed alternatives")
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].alternative(), 0);
        assert_eq!(rows[1].alternative(), 1);
        assert_eq!(
            rows[0].source_projection(),
            PreparedConstraintSourceProjection::Scalar
        );
    });
    transaction
        .submit_probe(
            &mut context,
            ticket,
            ProbeSubmission::Accepted(SourceProbeResult::checked(
                TypeKind::I32,
                PreparedBranch("tag"),
                0,
                PreparedRule::Tag(1),
            )),
        )
        .expect("checked evidence accepted");
    assert!(
        transaction
            .next_probe(&mut context)
            .expect("finish source probe")
            .is_none()
    );
    while let Some(mut ticket) = transaction
        .next_materialization_ticket(&mut context)
        .expect("materialization ticket")
    {
        let rows = ticket.requests().collect::<Vec<_>>();
        assert!(matches!(
            rows[0],
            MaterializedSourceRequest::Checked { alternative: 0, .. }
        ));
        let closed = close_materialization_ticket(
            &mut ticket,
            ClosedMaterializationSubmission::Sealed(PreparedSealed(9)),
        );
        transaction
            .submit_closed_materialization(ticket, closed)
            .expect("materialization accepted");
    }
    let solved = transaction
        .finish(context)
        .complete()
        .expect("prepared source solves");
    assert_eq!(solved.closed_sources.len(), 1);
    let row = &solved.closed_sources[0];
    assert_eq!(row.selection().alternative(), Some(0));
    assert_eq!(row.actual(), &TypeKind::I32);
    assert_eq!(row.final_expected(), Some(&TypeKind::I32));
}

#[test]
fn rejected_source_projection_retains_exact_lower_relation_authority() {
    let cancellation = AtomicBool::new(false);
    let mut context = prepared_context(&cancellation);
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::checked(
                17,
                PreparedConstraintSourceProjection::Scalar,
                [],
                PreparedSourceAlternative::new(0, PreparedRule::Otherwise, TypeKind::String),
            )
            .expect("one checked source"),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("prepared probe starts");
    let ticket = transaction
        .next_probe(&mut context)
        .expect("probe ticket")
        .expect("one source row");
    transaction
        .submit_probe(
            &mut context,
            ticket,
            ProbeSubmission::Accepted(SourceProbeResult::checked(
                TypeKind::I32,
                PreparedBranch("mismatch"),
                0,
                PreparedRule::Otherwise,
            )),
        )
        .expect("typed mismatch is retained until lower closes the row");
    assert!(
        transaction
            .next_probe(&mut context)
            .expect("close probe")
            .is_none()
    );

    let Err(TypeConstraintFailure::Rejected(TypeConstraintCandidateFailure::SourceProjection(
        rejected,
    ))) = transaction.finish(context).complete()
    else {
        panic!("source relation mismatch must retain a typed rejected projection")
    };
    assert_eq!(rejected.source(), 17);
    assert_eq!(rejected.alternative(), Some(0));
    assert_eq!(
        rejected.source_projection(),
        &CheckedConstraintSourceProjection::Scalar
    );
    assert_eq!(
        rejected.acceptance(),
        ConstraintAcceptance::PatternAcceptsActual
    );
    assert_eq!(rejected.expected(), &TypeKind::String);
    assert_eq!(rejected.actual(), &TypeKind::I32);
}

#[test]
fn one_live_frontier_outranks_a_sibling_source_relation_rejection() {
    let cancellation = AtomicBool::new(false);
    let mut context = prepared_context(&cancellation);
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction.constrain(
        &mut context,
        &TypeKind::Choice(vec![TypeKind::String, TypeKind::Bool]),
        &TypeKind::Never,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::checked(
                23,
                PreparedConstraintSourceProjection::Scalar,
                [PreparedSourceAlternative::new(
                    0,
                    PreparedRule::Tag(1),
                    TypeKind::String,
                )],
                PreparedSourceAlternative::new(1, PreparedRule::Otherwise, TypeKind::I32),
            )
            .expect("two alternatives"),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("prepared probe starts");
    let mut row = 0_u8;
    while let Some(ticket) = transaction.next_probe(&mut context).expect("probe ticket") {
        let (alternative, evidence) = if row == 0 {
            (0, PreparedRule::Tag(1))
        } else {
            (1, PreparedRule::Otherwise)
        };
        transaction
            .submit_probe(
                &mut context,
                ticket,
                ProbeSubmission::Accepted(SourceProbeResult::checked(
                    TypeKind::I32,
                    PreparedBranch("frontier"),
                    alternative,
                    evidence,
                )),
            )
            .expect("submit frontier row");
        row += 1;
    }
    assert_eq!(row, 2);
    while let Some(mut ticket) = transaction
        .next_materialization_ticket(&mut context)
        .expect("materialization ticket")
    {
        let closed = close_materialization_ticket(
            &mut ticket,
            ClosedMaterializationSubmission::Sealed(PreparedSealed(1)),
        );
        transaction
            .submit_closed_materialization(ticket, closed)
            .expect("seal surviving sibling");
    }
    let solved = transaction
        .finish(context)
        .complete()
        .expect("one live sibling must keep the ordinary candidate viable");
    assert_eq!(solved.closed_sources.len(), 1);
    assert_eq!(solved.closed_sources[0].selection().alternative(), Some(1));
}

#[test]
fn multiple_frontier_relation_failures_choose_the_first_typed_row() {
    let cancellation = AtomicBool::new(false);
    let mut context = prepared_context(&cancellation);
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction.constrain(
        &mut context,
        &TypeKind::Choice(vec![TypeKind::String, TypeKind::Bool]),
        &TypeKind::Never,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::checked(
                29,
                PreparedConstraintSourceProjection::Scalar,
                [PreparedSourceAlternative::new(
                    0,
                    PreparedRule::Tag(1),
                    TypeKind::String,
                )],
                PreparedSourceAlternative::new(1, PreparedRule::Otherwise, TypeKind::Bool),
            )
            .expect("two alternatives"),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("prepared probe starts");
    let mut row = 0_u8;
    while let Some(ticket) = transaction.next_probe(&mut context).expect("probe ticket") {
        let (actual, alternative, evidence) = if row == 0 {
            (TypeKind::I32, 0, PreparedRule::Tag(1))
        } else {
            (TypeKind::I64, 1, PreparedRule::Otherwise)
        };
        transaction
            .submit_probe(
                &mut context,
                ticket,
                ProbeSubmission::Accepted(SourceProbeResult::checked(
                    actual,
                    PreparedBranch("frontier mismatch"),
                    alternative,
                    evidence,
                )),
            )
            .expect("submit frontier row");
        row += 1;
    }
    assert_eq!(row, 2);
    let Err(TypeConstraintFailure::Rejected(TypeConstraintCandidateFailure::SourceProjection(
        rejected,
    ))) = transaction.finish(context).complete()
    else {
        panic!("all rejected frontiers must retain the deterministic first row")
    };
    assert_eq!(rejected.source(), 29);
    assert_eq!(rejected.alternative(), Some(0));
    assert_eq!(rejected.expected(), &TypeKind::String);
    assert_eq!(rejected.actual(), &TypeKind::I32);
}

#[test]
fn unchecked_source_retains_its_closed_physical_projection_and_actual_type() {
    let cancellation = AtomicBool::new(false);
    let mut context = prepared_context(&cancellation);
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction
        .begin_prepared_probe(
            &mut context,
            PreparedSourceConstraint::unchecked(7, PreparedConstraintSourceProjection::Scalar),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("unchecked probe starts");
    let ticket = transaction
        .next_probe(&mut context)
        .expect("unchecked hint projection")
        .expect("one unchecked source row");
    ticket.with_hint(|hint| assert!(matches!(hint, ExpectedHint::Unchecked)));
    transaction
        .submit_probe(
            &mut context,
            ticket,
            ProbeSubmission::Accepted(SourceProbeResult::unchecked(
                TypeKind::I32,
                PreparedBranch("unchecked"),
            )),
        )
        .expect("unchecked source accepted");
    assert!(
        transaction
            .next_probe(&mut context)
            .expect("finish unchecked probe")
            .is_none()
    );
    while let Some(mut ticket) = transaction
        .next_materialization_ticket(&mut context)
        .expect("unchecked materialization ticket")
    {
        let rows = ticket.requests().collect::<Vec<_>>();
        let MaterializedSourceRequest::Unchecked {
            source_projection,
            actual,
            ..
        } = rows[0]
        else {
            panic!("unchecked source must remain unchecked during materialization")
        };
        assert_eq!(
            source_projection,
            &CheckedConstraintSourceProjection::Scalar
        );
        assert_eq!(actual, &TypeKind::I32);
        let closed = close_materialization_ticket(
            &mut ticket,
            ClosedMaterializationSubmission::Sealed(PreparedSealed(7)),
        );
        transaction
            .submit_closed_materialization(ticket, closed)
            .expect("unchecked materialization accepted");
    }
    let solved = transaction
        .finish(context)
        .complete()
        .expect("unchecked source solves");
    let [row] = solved.closed_sources.as_ref() else {
        panic!("one unchecked closed source")
    };
    assert!(row.selection().is_unchecked());
    assert_eq!(row.actual(), &TypeKind::I32);
    assert_eq!(
        row.source_projection(),
        &CheckedConstraintSourceProjection::Scalar
    );
    assert_eq!(row.final_expected(), None);
}

#[test]
fn dynamic_rest_projection_is_derived_from_actual_and_composes_array_length() {
    let cancellation = AtomicBool::new(false);
    let mut context = prepared_context(&cancellation);
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    let prepared = PreparedSourceConstraint::checked(
        8,
        PreparedConstraintSourceProjection::InferSpreadContainer {
            policy: ConstraintSourceContainerPolicy::Positional,
        },
        [],
        PreparedSourceAlternative::new(0, PreparedRule::Otherwise, TypeKind::I32),
    )
    .expect("typed rest has one fallback identity row");
    transaction
        .begin_prepared_probe(
            &mut context,
            prepared,
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("rest probe starts");
    let ticket = transaction
        .next_probe(&mut context)
        .expect("rest hint")
        .expect("one rest row");
    transaction
        .submit_probe(
            &mut context,
            ticket,
            ProbeSubmission::Accepted(SourceProbeResult::checked(
                TypeKind::Array {
                    item: Box::new(TypeKind::I32),
                    len: super::super::ArrayLength::Const(3),
                },
                PreparedBranch("rest"),
                0,
                PreparedRule::Otherwise,
            )),
        )
        .expect("rest actual accepted");
    assert!(
        transaction
            .next_probe(&mut context)
            .expect("finish rest probe")
            .is_none()
    );
    let mut ticket = transaction
        .next_materialization_ticket(&mut context)
        .expect("rest materialization")
        .expect("rest trace");
    let rows = ticket.requests().collect::<Vec<_>>();
    assert!(matches!(
        rows[0],
        MaterializedSourceRequest::Checked {
            source_projection: CheckedConstraintSourceProjection::SpreadContainer(
                CheckedConstraintContainerConstructor::Array {
                    len: super::super::ArrayLength::Const(3)
                }
            ),
            expected: TypeKind::Array { .. },
            ..
        }
    ));
    let closed = close_materialization_ticket(
        &mut ticket,
        ClosedMaterializationSubmission::Sealed(PreparedSealed(3)),
    );
    transaction
        .submit_closed_materialization(ticket, closed)
        .expect("rest materialization accepted");
    let solved = transaction.finish(context).complete().expect("rest solves");
    assert_eq!(
        solved.closed_sources[0].final_expected(),
        Some(&TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: super::super::ArrayLength::Const(3),
        })
    );
}

#[test]
fn closed_source_rows_normalize_generic_container_actuals_and_headers() {
    let parameter = owned_parameter(309, 0);
    let scope = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .expect("bindable actual parameter scope");
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, PreparedDomain>::with_scope(
            TypeConstraintLimits::new(1_024, 512, 128, 64).with_source_limits(64, 64),
            &cancellation,
            scope,
        );
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    let prepared = PreparedSourceConstraint::checked(
        13,
        PreparedConstraintSourceProjection::InferSpreadContainer {
            policy: ConstraintSourceContainerPolicy::Positional,
        },
        [],
        PreparedSourceAlternative::new(0, PreparedRule::Otherwise, TypeKind::I32),
    )
    .expect("typed rest has one explicit fallback");
    transaction.constrain(
        &mut context,
        &TypeKind::GenericParam(parameter.clone()),
        &TypeKind::I32,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    transaction
        .begin_prepared_probe(
            &mut context,
            prepared,
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("generic rest probe starts");
    let ticket = transaction
        .next_probe(&mut context)
        .expect("generic rest hint")
        .expect("one generic rest row");
    transaction
        .submit_probe(
            &mut context,
            ticket,
            ProbeSubmission::Accepted(SourceProbeResult::checked(
                TypeKind::Array {
                    item: Box::new(TypeKind::GenericParam(parameter.clone())),
                    len: super::super::ArrayLength::Const(5),
                },
                PreparedBranch("generic-rest"),
                0,
                PreparedRule::Otherwise,
            )),
        )
        .expect("generic actual relates to item expected");
    assert!(
        transaction
            .next_probe(&mut context)
            .expect("finish generic rest probe")
            .is_none()
    );

    let mut ticket = transaction
        .next_materialization_ticket(&mut context)
        .expect("generic rest materialization")
        .expect("generic rest trace");
    let requests = ticket.requests().collect::<Vec<_>>();
    assert!(matches!(
        requests[0],
        MaterializedSourceRequest::Checked {
            actual: TypeKind::Array {
                item,
                len: super::super::ArrayLength::Const(5)
            },
            source_projection: CheckedConstraintSourceProjection::SpreadContainer(
                CheckedConstraintContainerConstructor::Array {
                    len: super::super::ArrayLength::Const(5)
                }
            ),
            expected: TypeKind::Array {
                item: expected_item,
                len: super::super::ArrayLength::Const(5)
            },
            ..
        } if item.as_ref() == &TypeKind::I32 && expected_item.as_ref() == &TypeKind::I32
    ));
    let closed = close_materialization_ticket(
        &mut ticket,
        ClosedMaterializationSubmission::Sealed(PreparedSealed(5)),
    );
    transaction
        .submit_closed_materialization(ticket, closed)
        .expect("generic rest materialization accepted");
    let solved = transaction
        .finish(context)
        .complete()
        .expect("generic rest solves");
    let trace = &solved.closed_sources[0];
    assert_eq!(
        trace.actual(),
        &TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: super::super::ArrayLength::Const(5),
        }
    );
    assert_eq!(
        trace.source_projection(),
        &CheckedConstraintSourceProjection::SpreadContainer(
            CheckedConstraintContainerConstructor::Array {
                len: super::super::ArrayLength::Const(5)
            }
        )
    );
    assert_eq!(
        trace.final_expected(),
        Some(&TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: super::super::ArrayLength::Const(5),
        })
    );
}

#[test]
fn closed_source_rows_normalize_generic_map_actuals_and_headers() {
    let parameter = owned_parameter(310, 0);
    let scope = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .expect("bindable map-key parameter scope");
    let cancellation = AtomicBool::new(false);
    let mut context =
        TypeConstraintContext::<LocalConstraintAccounting<'_>, PreparedDomain>::with_scope(
            TypeConstraintLimits::new(1_024, 512, 128, 64).with_source_limits(64, 64),
            &cancellation,
            scope,
        );
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction.constrain(
        &mut context,
        &TypeKind::GenericParam(parameter.clone()),
        &TypeKind::String,
        ConstraintAcceptance::PatternAcceptsActual,
    );
    let prepared = PreparedSourceConstraint::checked(
        14,
        PreparedConstraintSourceProjection::InferSpreadContainer {
            policy: ConstraintSourceContainerPolicy::Named,
        },
        [],
        PreparedSourceAlternative::new(0, PreparedRule::Otherwise, TypeKind::I32),
    )
    .expect("named typed rest has one explicit fallback");
    transaction
        .begin_prepared_probe(
            &mut context,
            prepared,
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("generic map rest probe starts");
    let ticket = transaction
        .next_probe(&mut context)
        .expect("generic map rest hint")
        .expect("one generic map rest row");
    transaction
        .submit_probe(
            &mut context,
            ticket,
            ProbeSubmission::Accepted(SourceProbeResult::checked(
                TypeKind::Map {
                    kind: super::super::MapKind::BTree,
                    key: Box::new(TypeKind::GenericParam(parameter.clone())),
                    value: Box::new(TypeKind::I32),
                },
                PreparedBranch("generic-map-rest"),
                0,
                PreparedRule::Otherwise,
            )),
        )
        .expect("generic map actual relates to value expected");
    assert!(
        transaction
            .next_probe(&mut context)
            .expect("finish generic map rest probe")
            .is_none()
    );
    let mut ticket = transaction
        .next_materialization_ticket(&mut context)
        .expect("generic map rest materialization")
        .expect("generic map rest trace");
    let requests = ticket.requests().collect::<Vec<_>>();
    assert!(matches!(
        requests[0],
        MaterializedSourceRequest::Checked {
            actual: TypeKind::Map {
                kind: super::super::MapKind::BTree,
                key,
                value
            },
            source_projection: CheckedConstraintSourceProjection::SpreadContainer(
                CheckedConstraintContainerConstructor::MapValue {
                    kind: super::super::MapKind::BTree,
                    key: projection_key
                }
            ),
            expected: TypeKind::Map {
                kind: super::super::MapKind::BTree,
                key: expected_key,
                value: expected_value
            },
            ..
        } if key.as_ref() == &TypeKind::String
            && projection_key.as_ref() == &TypeKind::String
            && expected_key.as_ref() == &TypeKind::String
            && value.as_ref() == &TypeKind::I32
            && expected_value.as_ref() == &TypeKind::I32
    ));
    let closed = close_materialization_ticket(
        &mut ticket,
        ClosedMaterializationSubmission::Sealed(PreparedSealed(6)),
    );
    transaction
        .submit_closed_materialization(ticket, closed)
        .expect("generic map rest materialization accepted");
    let solved = transaction
        .finish(context)
        .complete()
        .expect("generic map rest solves");
    let trace = &solved.closed_sources[0];
    assert_eq!(
        trace.actual(),
        &TypeKind::Map {
            kind: super::super::MapKind::BTree,
            key: Box::new(TypeKind::String),
            value: Box::new(TypeKind::I32),
        }
    );
    assert_eq!(
        trace.source_projection(),
        &CheckedConstraintSourceProjection::SpreadContainer(
            CheckedConstraintContainerConstructor::MapValue {
                kind: super::super::MapKind::BTree,
                key: Box::new(TypeKind::String),
            }
        )
    );
    assert_eq!(
        trace.final_expected(),
        Some(&TypeKind::Map {
            kind: super::super::MapKind::BTree,
            key: Box::new(TypeKind::String),
            value: Box::new(TypeKind::I32),
        })
    );
}

#[test]
fn otherwise_cannot_shortcut_a_matching_guard() {
    let cancellation = AtomicBool::new(false);
    let mut context = prepared_context(&cancellation);
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction
        .begin_prepared_probe(
            &mut context,
            checked_prepared_source(9, PreparedConstraintSourceProjection::Scalar),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("first source starts");
    let ticket = transaction
        .next_probe(&mut context)
        .expect("first ticket")
        .expect("first row");
    assert!(matches!(
        transaction.submit_probe(
            &mut context,
            ticket,
            ProbeSubmission::Accepted(SourceProbeResult::checked(
                TypeKind::I32,
                PreparedBranch("bad"),
                1,
                PreparedRule::Tag(1),
            )),
        ),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::SourceProtocol(
                TypeConstraintSourceProtocolInvariant::InvalidEvidence,
            )
        ))
    ));
}

#[test]
fn otherwise_is_selected_only_when_all_guarded_evidence_is_absent() {
    let cancellation = AtomicBool::new(false);
    let mut context = prepared_context(&cancellation);
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    transaction
        .begin_prepared_probe(
            &mut context,
            checked_prepared_source(10, PreparedConstraintSourceProjection::Scalar),
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("source starts");
    let ticket = transaction
        .next_probe(&mut context)
        .expect("ticket")
        .expect("source row");
    transaction
        .submit_probe(
            &mut context,
            ticket,
            ProbeSubmission::Accepted(SourceProbeResult::checked(
                TypeKind::I32,
                PreparedBranch("otherwise"),
                1,
                PreparedRule::Tag(2),
            )),
        )
        .expect("nonmatching evidence selects otherwise");
    assert!(
        transaction
            .next_probe(&mut context)
            .expect("probe closes")
            .is_none()
    );
}

#[test]
fn duplicate_matching_guards_are_rejected_as_tampered_evidence() {
    let cancellation = AtomicBool::new(false);
    let mut context = prepared_context(&cancellation);
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    let prepared = PreparedSourceConstraint::checked(
        11,
        PreparedConstraintSourceProjection::Scalar,
        [
            PreparedSourceAlternative::new(0, PreparedRule::Tag(1), TypeKind::I32),
            PreparedSourceAlternative::new(1, PreparedRule::Tag(1), TypeKind::I32),
        ],
        PreparedSourceAlternative::new(2, PreparedRule::Otherwise, TypeKind::I32),
    )
    .expect("structural otherwise and exact coordinates");
    transaction
        .begin_prepared_probe(
            &mut context,
            prepared,
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .expect("source starts");
    let ticket = transaction
        .next_probe(&mut context)
        .expect("ticket")
        .expect("source row");
    assert!(matches!(
        transaction.submit_probe(
            &mut context,
            ticket,
            ProbeSubmission::Accepted(SourceProbeResult::checked(
                TypeKind::I32,
                PreparedBranch("tampered"),
                0,
                PreparedRule::Tag(1),
            )),
        ),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::SourceProtocol(
                TypeConstraintSourceProtocolInvariant::InvalidEvidence,
            )
        ))
    ));
}

#[test]
fn prepared_source_structurally_owns_otherwise_and_requires_exact_ordinals() {
    let gapped_ordinal = PreparedSourceConstraint::<PreparedDomain>::checked(
        12,
        PreparedConstraintSourceProjection::Scalar,
        [PreparedSourceAlternative::new(
            0,
            PreparedRule::Tag(1),
            TypeKind::I32,
        )],
        PreparedSourceAlternative::new(2, PreparedRule::Otherwise, TypeKind::I32),
    );
    assert!(matches!(
        gapped_ordinal,
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedSource(PreparedSourceConstraintInvariant::Unordered,)
        ))
    ));

    let cancellation = AtomicBool::new(false);
    let mut context = prepared_context(&cancellation);
    let mut transaction = TypeConstraintTransaction::<PreparedDomain>::new();
    transaction
        .initialize(&mut context, None)
        .expect("valid test initialization");
    let malformed = PreparedSourceConstraint::Checked {
        source: 13,
        source_projection: PreparedConstraintSourceProjection::Scalar,
        guarded: Arc::from([PreparedSourceAlternative::new(
            0,
            PreparedRule::Tag(1),
            TypeKind::I32,
        )]),
        otherwise: PreparedSourceAlternative::new(2, PreparedRule::Otherwise, TypeKind::I32),
    };
    assert!(matches!(
        transaction.begin_prepared_probe(
            &mut context,
            malformed,
            ConstraintAcceptance::PatternAcceptsActual,
        ),
        Err(TypeConstraintError::Invariant(
            TypeConstraintInvariant::PreparedSource(PreparedSourceConstraintInvariant::Unordered,)
        ))
    ));
}

#[test]
fn checked_rest_projection_retains_each_runtime_constructor() {
    let value_expected = TypeKind::I32;
    let positional = PreparedConstraintSourceProjection::InferSpreadContainer {
        policy: ConstraintSourceContainerPolicy::Positional,
    };
    for actual in [
        TypeKind::Vec(Box::new(TypeKind::I32)),
        TypeKind::Seq(Box::new(TypeKind::I32)),
        TypeKind::Slice(Box::new(TypeKind::I32)),
        TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: super::super::ArrayLength::Const(4),
        },
    ] {
        assert!(matches!(
            CheckedConstraintSourceProjection::derive(positional, &actual),
            Some(CheckedConstraintSourceProjection::SpreadContainer(_))
        ));
    }

    let actual = TypeKind::Map {
        kind: super::super::MapKind::BTree,
        key: Box::new(TypeKind::String),
        value: Box::new(TypeKind::I32),
    };
    let checked = CheckedConstraintSourceProjection::derive(
        PreparedConstraintSourceProjection::InferSpreadContainer {
            policy: ConstraintSourceContainerPolicy::Named,
        },
        &actual,
    )
    .expect("named rest map constructor");
    assert_eq!(
        checked.compose_expected(&value_expected),
        TypeKind::Map {
            kind: super::super::MapKind::BTree,
            key: Box::new(TypeKind::String),
            value: Box::new(value_expected),
        }
    );
}
