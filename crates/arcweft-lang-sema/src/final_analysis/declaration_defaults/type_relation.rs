//! Conditional default checking uses the ordinary candidate-wide type relation.

use super::super::{FinalSemanticAnalysisControl, FinalSemanticAnalysisError};
use crate::{
    callable::{
        CallableParameterCoordinate, CallableSignatureSchema, CheckedDeclarationDefault,
        PRODUCTION_CALLABLE_LIMITS, ResolverWork,
    },
    effect_row::{EffectConstraintEligibility, EffectConstraintVariable, EffectPredicate},
    types::constraints::context::TypeConstraintContext,
    types::constraints::transaction::TypeConstraintTransaction,
    types::constraints::{
        ConstraintAcceptance, ConstraintDomain, TypeConstraintAbort, TypeConstraintEffectScope,
        TypeConstraintFailure, TypeConstraintFailureInvariant, TypeConstraintInitializationFailure,
        TypeConstraintInvariant, TypeConstraintParameterScope,
    },
    types::{GenericBinder, GenericEffectReference, TypeGenericUseCollector, TypeKind},
};
use arcweft_lang_hir::identity::ExprId;
use std::{convert::Infallible, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
enum DefaultConstraintFailure {
    Abort(TypeConstraintAbort),
    Invariant(TypeConstraintInvariant),
    UnexpectedSource,
}

/// Retains a declaration default's real constraint failure without a fake call.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("declaration default constraint failure for {owner:?}: {failure:?}")]
pub struct FinalDeclarationDefaultConstraintFailure {
    owner: ExprId,
    failure: Arc<DefaultConstraintFailure>,
}

struct DefaultTypeRelation;
impl ConstraintDomain for DefaultTypeRelation {
    type Application = CallableParameterCoordinate;
    type Source = Infallible;
    type AlternativeIndex = Infallible;
    type EvidenceRule = Infallible;
    type ObservedEvidence = Infallible;
    type CheckedEvidence = Infallible;
    type ProbeSemanticBranch = ();
    type SealedBranchValue = ();
    type Projection = Infallible;
    type SourceErrorCause = Infallible;
    type ClientInvariant = Infallible;
    fn evidence_accepts(rule: &Infallible, _: &Infallible) -> bool {
        match *rule {}
    }
    fn project_checked_evidence(observed: &Infallible, _: &TypeKind) -> Option<Infallible> {
        match *observed {}
    }
    fn alternative_ordinal(index: &Infallible) -> u32 {
        match *index {}
    }
    fn client_invariant_source(invariant: &Infallible) -> Infallible {
        match *invariant {}
    }
    fn empty_sealed_branch() {}
}

impl CheckedDeclarationDefault {
    /// Omitted parameter effects may be chosen for this default invocation.
    /// Earlier input effects remain rigid; the declared binding is not narrowed.
    pub(crate) fn check_type(
        signature: &CallableSignatureSchema,
        coordinate: CallableParameterCoordinate,
        actual: &TypeKind,
        owner: ExprId,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        control.check()?;
        let expected = signature
            .parameter_type(coordinate)
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        if expected.accepts(actual) {
            return Ok(true);
        }
        let used = TypeGenericUseCollector::collect(expected)
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let inventory = signature.generic_inventory();
        let variables = inventory.effects().iter().map(|entry| {
            let bindable = entry.role() == crate::callable::CallableSchemaGenericRole::Candidate
                && matches!(entry.parameter(), GenericEffectReference::Free(parameter) if used.effects().contains(parameter));
            EffectConstraintVariable::new(entry.parameter().clone(), if bindable {
                EffectConstraintEligibility::Bindable
            } else { EffectConstraintEligibility::Rigid })
        }).collect::<Vec<_>>();
        let effects = TypeConstraintEffectScope::seal_call_scope_with_predicate(
            variables,
            [],
            EffectPredicate::unconstrained(),
        )
        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let parameters = TypeConstraintParameterScope::seal_call_scope(
            GenericBinder::EMPTY,
            [],
            [],
            effects,
            [],
            [],
        )
        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let failure = |failure| {
            FinalSemanticAnalysisError::DeclarationDefaultConstraintFailure(
                FinalDeclarationDefaultConstraintFailure {
                    owner,
                    failure: Arc::new(failure),
                },
            )
        };
        let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
        let session = work
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, control.cancellation())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let mut context = TypeConstraintContext::with_accounting(session);
        let mut relation = TypeConstraintTransaction::<DefaultTypeRelation>::initialize(
            &mut context,
            coordinate,
            parameters,
            None,
        )
        .map_err(|error| {
            failure(match error {
                TypeConstraintInitializationFailure::Abort(error) => {
                    DefaultConstraintFailure::Abort(error)
                }
                TypeConstraintInitializationFailure::Invariant(error) => {
                    DefaultConstraintFailure::Invariant(error)
                }
            })
        })?;
        relation.constrain(
            &mut context,
            expected,
            actual,
            ConstraintAcceptance::PatternAcceptsActual,
        );
        match relation.finish(&mut context) {
            Ok(_) => Ok(true),
            Err(TypeConstraintFailure::Rejected(_)) => Ok(false),
            Err(TypeConstraintFailure::Abort(error)) => {
                Err(failure(DefaultConstraintFailure::Abort(error)))
            }
            Err(TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Constraint(
                error,
            ))) => Err(failure(DefaultConstraintFailure::Invariant(error))),
            Err(TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Client(
                error,
            ))) => match *error {},
            Err(TypeConstraintFailure::FatalSource(_)) => {
                Err(failure(DefaultConstraintFailure::UnexpectedSource))
            }
        }
    }
}
