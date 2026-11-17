//! Short work loans share an existing preparation owner across expression phases.

use std::{cell::RefCell, rc::Rc};

use super::{
    callable_effect_graph::CallableEffectClosureWork,
    calls::{CallAnalysisFailure, CallAnalysisInvariant},
};
use crate::{
    callable::{CallConstraintInvariant, CandidateConstraintWorkLoan},
    effect_row::{DecisionControl, DecisionWork},
    final_analysis::{
        FinalCallConstraintFailure, FinalSemanticAnalysisControl, FinalSemanticAnalysisError,
    },
    types::constraints::{
        TypeConstraintAbort, TypeConstraintError, TypeConstraintInvariant,
        TypeConstraintProjectionInvariant,
    },
    types::{TypeProjectionControl, TypeProjectionNodeKind},
};

trait DeclarationWork {
    fn charge(&self, work: DecisionWork) -> Result<(), FinalSemanticAnalysisError>;
    fn check(&self) -> Result<(), FinalSemanticAnalysisError>;
}

impl DeclarationWork for RefCell<CallableEffectClosureWork<'_>> {
    fn charge(&self, work: DecisionWork) -> Result<(), FinalSemanticAnalysisError> {
        // This private borrow performs accounting only and invokes no caller
        // code. It is released before any evaluator or driver callback.
        DecisionControl::charge(&mut *self.borrow_mut(), work)
    }

    fn check(&self) -> Result<(), FinalSemanticAnalysisError> {
        TypeProjectionControl::check(&mut *self.borrow_mut())
    }
}

/// Clones retain the same work authority. They do not reset a budget or retain
/// an exclusive reservation borrow while an expression or child driver runs.
#[derive(Clone)]
pub(super) struct AnalyzerPreparationWork<'a> {
    owner: WorkOwner<'a>,
}

#[derive(Clone)]
enum WorkOwner<'a> {
    Declaration(Rc<dyn DeclarationWork + 'a>),
    Candidate {
        work: CandidateConstraintWorkLoan<'a>,
        owner: arcweft_lang_hir::identity::ExprId,
    },
}

impl<'a> AnalyzerPreparationWork<'a> {
    pub(super) fn root(limit: u64, control: FinalSemanticAnalysisControl<'a>) -> Self {
        Self {
            owner: WorkOwner::Declaration(Rc::new(RefCell::new(CallableEffectClosureWork::new(
                limit, control,
            )))),
        }
    }

    pub(super) fn candidate(
        work: CandidateConstraintWorkLoan<'a>,
        owner: arcweft_lang_hir::identity::ExprId,
    ) -> Self {
        Self {
            owner: WorkOwner::Candidate { work, owner },
        }
    }

    fn constraint_error(
        owner: arcweft_lang_hir::identity::ExprId,
        error: TypeConstraintError,
    ) -> FinalSemanticAnalysisError {
        let failure = match error {
            TypeConstraintError::Abort(TypeConstraintAbort::Cancelled) => {
                return FinalSemanticAnalysisError::Cancelled;
            }
            TypeConstraintError::Abort(error) => CallAnalysisFailure::Abort(error),
            TypeConstraintError::Invariant(error) => CallAnalysisFailure::Invariant(
                CallAnalysisInvariant::Constraint(CallConstraintInvariant::Lower(error)),
            ),
            // Accounting cannot reject an authored candidate. Preserve an
            // unexpected rejection as a violation of this operational port.
            TypeConstraintError::Rejected(error) => {
                CallAnalysisFailure::Invariant(CallAnalysisInvariant::Constraint(
                    CallConstraintInvariant::Lower(TypeConstraintInvariant::Projection(
                        TypeConstraintProjectionInvariant::Mismatch(error),
                    )),
                ))
            }
        };
        FinalSemanticAnalysisError::CallConstraintFailure(FinalCallConstraintFailure::new(
            owner, failure,
        ))
    }
}

impl DecisionControl for AnalyzerPreparationWork<'_> {
    type Error = FinalSemanticAnalysisError;

    fn charge(&mut self, work: DecisionWork) -> Result<(), Self::Error> {
        match &mut self.owner {
            WorkOwner::Declaration(owner) => owner.charge(work),
            WorkOwner::Candidate { work: loan, owner } => loan
                .charge(work)
                .map_err(|error| Self::constraint_error(*owner, error)),
        }
    }
}

impl TypeProjectionControl for AnalyzerPreparationWork<'_> {
    type Error = FinalSemanticAnalysisError;

    fn check(&mut self) -> Result<(), Self::Error> {
        match &mut self.owner {
            WorkOwner::Declaration(owner) => owner.check(),
            WorkOwner::Candidate { work, owner } => work
                .check()
                .map_err(|error| Self::constraint_error(*owner, error)),
        }
    }

    fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Self::Error> {
        self.charge(DecisionWork::Visit)
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.charge(DecisionWork::Node)
    }
}
