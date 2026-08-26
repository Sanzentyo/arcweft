//! Typed expression-evaluation errors and the generation-local call-frame owner.
//!
//! Expression checking is deliberately narrower than the public final
//! analysis error.  A candidate mismatch is not an authored fatal error, and
//! a nested call constraint failure must retain its lower payload until the
//! outer analyzer chooses the public boundary.

use std::{cell::Cell, rc::Rc, sync::Arc};

use arcweft_lang_hir::identity::ExprId;

use super::{
    calls::CallAnalysisFailure,
    state::{CandidateExpressionFactAuthority, CandidatePhysicalAttemptContext},
};
use crate::{
    final_analysis::{
        CandidateFactTransactionViolation, FinalCallConstraintFailure, FinalSemanticAnalysisError,
    },
    types::constraints::TypeConstraintAbort,
};

#[derive(Debug)]
pub(super) enum AnalyzerExpressionFactAuthority<'a> {
    Published,
    Candidate(CandidateExpressionFactAuthority<'a>),
}

#[derive(Debug)]
struct PhysicalCallAttemptIssuer;

/// Call-frame-issued operational coordinate for one exact call execution.
/// The nonce lineage distinguishes repeated evaluation of the same HIR call
/// under different outer candidate passes without exposing a global counter
/// as an independent identity authority.
#[derive(Clone)]
pub(in crate::final_analysis) struct PhysicalCallAttemptId {
    issuer: Arc<PhysicalCallAttemptIssuer>,
    root: ExprId,
    owner: ExprId,
    nonce_lineage: Box<[u64]>,
    candidate_context: Option<CandidatePhysicalAttemptContext>,
}

impl PhysicalCallAttemptId {
    pub(in crate::final_analysis) const fn root(&self) -> ExprId {
        self.root
    }

    pub(in crate::final_analysis) const fn owner(&self) -> ExprId {
        self.owner
    }

    pub(in crate::final_analysis) fn is_direct_child_of(&self, parent: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &parent.issuer)
            && self.root == parent.root
            && self.nonce_lineage.len() == parent.nonce_lineage.len() + 1
            && self.nonce_lineage.starts_with(&parent.nonce_lineage)
    }

    pub(in crate::final_analysis) fn is_root(&self) -> bool {
        self.nonce_lineage.len() == 1
    }

    pub(in crate::final_analysis) fn same_operational_attempt(&self, other: &Self) -> bool {
        if !Arc::ptr_eq(&self.issuer, &other.issuer)
            || self.root != other.root
            || self.owner != other.owner
        {
            return false;
        }
        match (&self.candidate_context, &other.candidate_context) {
            (Some(left), Some(right)) => left.same_issued_context(right),
            (None, None) => self.nonce_lineage == other.nonce_lineage,
            (Some(_), None) | (None, Some(_)) => false,
        }
    }

    pub(in crate::final_analysis) fn same_issued_attempt(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer)
            && self.root == other.root
            && self.owner == other.owner
            && self.nonce_lineage == other.nonce_lineage
            && match (&self.candidate_context, &other.candidate_context) {
                (Some(left), Some(right)) => left.same_issued_context(right),
                (None, None) => true,
                (Some(_), None) | (None, Some(_)) => false,
            }
    }
}

impl std::fmt::Debug for PhysicalCallAttemptId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PhysicalCallAttemptId")
            .field("root", &self.root)
            .field("owner", &self.owner)
            .field("depth", &self.nonce_lineage.len())
            .finish()
    }
}

impl PartialEq for PhysicalCallAttemptId {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && self.owner == other.owner
            && self.nonce_lineage == other.nonce_lineage
            && self.candidate_context == other.candidate_context
    }
}

impl Eq for PhysicalCallAttemptId {}

/// Immutable context threaded through one expression evaluator invocation.
/// The frame owner is shared with the RAII stack so nested callbacks observe
/// the same depth authority without manually balancing a counter.
pub(crate) struct AnalyzerExpressionContext<'a> {
    authority: AnalyzerExpressionFactAuthority<'a>,
    frames: Rc<CallFrameStack>,
}

impl<'a> AnalyzerExpressionContext<'a> {
    pub(super) fn published(frames: Rc<CallFrameStack>) -> Self {
        Self {
            authority: AnalyzerExpressionFactAuthority::Published,
            frames,
        }
    }

    pub(super) fn candidate(
        authority: CandidateExpressionFactAuthority<'a>,
        frames: Rc<CallFrameStack>,
    ) -> Self {
        Self {
            authority: AnalyzerExpressionFactAuthority::Candidate(authority),
            frames,
        }
    }

    pub(super) fn authority(&self) -> &AnalyzerExpressionFactAuthority<'a> {
        &self.authority
    }

    pub(super) fn is_candidate(&self) -> bool {
        matches!(
            self.authority,
            AnalyzerExpressionFactAuthority::Candidate(_)
        )
    }

    pub(super) fn child_candidate<'b>(
        &'b self,
        authority: CandidateExpressionFactAuthority<'b>,
    ) -> AnalyzerExpressionContext<'b>
    where
        'a: 'b,
    {
        let authority = match &self.authority {
            AnalyzerExpressionFactAuthority::Published => authority,
            AnalyzerExpressionFactAuthority::Candidate(existing) => existing.reborrow(),
        };
        AnalyzerExpressionContext::candidate(authority, Rc::clone(&self.frames))
    }

    pub(super) fn enter_call(
        &self,
        owner: ExprId,
    ) -> Result<ActiveCallFrame, CallFrameEnterFailure> {
        self.frames.enter(owner)
    }

    fn physical_attempt_context(&self) -> Option<CandidatePhysicalAttemptContext> {
        match &self.authority {
            AnalyzerExpressionFactAuthority::Published => None,
            AnalyzerExpressionFactAuthority::Candidate(authority) => {
                Some(authority.physical_attempt_context())
            }
        }
    }
}

/// One shared stack for ordinary, constraint, and immediate dialogue calls.
/// It owns no semantic facts; it only provides a depth/ownership lease.
pub(super) struct CallFrameStack {
    physical_attempt_issuer: Arc<PhysicalCallAttemptIssuer>,
    depth: Cell<u64>,
    root: Cell<Option<ExprId>>,
    active_nonce: Cell<Option<u64>>,
    next_nonce: Cell<u64>,
    poison: Cell<Option<CallFrameInvariant>>,
    nonce_lineage: Box<[Cell<Option<CallFrameNonceLineage>>]>,
    limit: u64,
}

/// The owner-held relation for one live frame.
///
/// A frame lease carries only its own nonce. Its parent is retained in this
/// fixed, owner-owned LIFO storage so a lease cannot supply a forged nonce to
/// restore after it is popped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CallFrameNonceLineage {
    nonce: u64,
    parent: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CallFrameInvariant {
    Unclosed {
        owner: ExprId,
        entered_depth: u64,
        nonce: u64,
    },
    StaleClose {
        owner: ExprId,
        entered_depth: u64,
        actual_depth: u64,
        nonce: u64,
    },
    OutOfOrderClose {
        owner: ExprId,
        entered_depth: u64,
        actual_depth: u64,
        nonce: u64,
        active_nonce: Option<u64>,
    },
    DepthMismatch {
        owner: ExprId,
        entered_depth: u64,
        actual_depth: u64,
    },
    RootMismatch {
        owner: ExprId,
        expected: ExprId,
        actual: Option<ExprId>,
    },
    NonceMismatch {
        owner: ExprId,
        expected: Option<u64>,
        actual: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CallFrameEnterFailure {
    Abort(TypeConstraintAbort),
    Invariant(CallFrameInvariant),
}

impl CallFrameStack {
    pub(super) fn new(limit: usize) -> Result<Rc<Self>, TypeConstraintAbort> {
        let limit = u64::try_from(limit).map_err(|_| TypeConstraintAbort::ArithmeticOverflow)?;
        Ok(Rc::new(Self {
            physical_attempt_issuer: Arc::new(PhysicalCallAttemptIssuer),
            depth: Cell::new(0),
            root: Cell::new(None),
            active_nonce: Cell::new(None),
            next_nonce: Cell::new(0),
            poison: Cell::new(None),
            nonce_lineage: std::iter::repeat_with(|| Cell::new(None))
                .take(usize::try_from(limit).map_err(|_| TypeConstraintAbort::ArithmeticOverflow)?)
                .collect(),
            limit,
        }))
    }

    fn poison_cause(&self) -> Option<CallFrameInvariant> {
        self.poison.get()
    }

    fn mark_poison(&self, violation: CallFrameInvariant) -> CallFrameInvariant {
        match self.poison.get() {
            Some(existing) => existing,
            None => {
                self.poison.set(Some(violation));
                violation
            }
        }
    }

    pub(super) fn enter(
        self: &Rc<Self>,
        owner: ExprId,
    ) -> Result<ActiveCallFrame, CallFrameEnterFailure> {
        if let Some(cause) = self.poison_cause() {
            return Err(CallFrameEnterFailure::Invariant(cause));
        }
        let current_depth = self.depth.get();
        let actual = current_depth
            .checked_add(1)
            .ok_or(CallFrameEnterFailure::Abort(
                TypeConstraintAbort::ArithmeticOverflow,
            ))?;
        if actual > self.limit {
            return Err(CallFrameEnterFailure::Abort(
                TypeConstraintAbort::CallDepth {
                    actual,
                    limit: self.limit,
                },
            ));
        }
        if current_depth == 0 {
            if let Some(actual_root) = self.root.get() {
                return Err(CallFrameEnterFailure::Invariant(self.mark_poison(
                    CallFrameInvariant::RootMismatch {
                        owner,
                        expected: owner,
                        actual: Some(actual_root),
                    },
                )));
            }
            if let Some(actual_nonce) = self.active_nonce.get() {
                return Err(CallFrameEnterFailure::Invariant(self.mark_poison(
                    CallFrameInvariant::NonceMismatch {
                        owner,
                        expected: None,
                        actual: Some(actual_nonce),
                    },
                )));
            }
            if let Some(actual_lineage) = self.nonce_lineage.iter().find_map(Cell::get) {
                return Err(CallFrameEnterFailure::Invariant(self.mark_poison(
                    CallFrameInvariant::NonceMismatch {
                        owner,
                        expected: None,
                        actual: Some(actual_lineage.nonce),
                    },
                )));
            }
        }
        let nonce = self.next_nonce.get();
        let next_nonce = nonce.checked_add(1).ok_or(CallFrameEnterFailure::Abort(
            TypeConstraintAbort::ArithmeticOverflow,
        ))?;
        let root_owner = if current_depth == 0 {
            owner
        } else {
            self.root.get().ok_or_else(|| {
                CallFrameEnterFailure::Invariant(self.mark_poison(
                    CallFrameInvariant::RootMismatch {
                        owner,
                        expected: owner,
                        actual: None,
                    },
                ))
            })?
        };
        let parent = if current_depth == 0 {
            None
        } else {
            let parent_index = usize::try_from(current_depth - 1).map_err(|_| {
                CallFrameEnterFailure::Abort(TypeConstraintAbort::ArithmeticOverflow)
            })?;
            let parent = self
                .nonce_lineage
                .get(parent_index)
                .and_then(Cell::get)
                .ok_or_else(|| {
                    CallFrameEnterFailure::Invariant(self.mark_poison(
                        CallFrameInvariant::NonceMismatch {
                            owner,
                            expected: self.active_nonce.get(),
                            actual: None,
                        },
                    ))
                })?;
            if self.active_nonce.get() != Some(parent.nonce) {
                return Err(CallFrameEnterFailure::Invariant(self.mark_poison(
                    CallFrameInvariant::NonceMismatch {
                        owner,
                        expected: self.active_nonce.get(),
                        actual: Some(parent.nonce),
                    },
                )));
            }
            Some(parent.nonce)
        };
        let slot_index = usize::try_from(current_depth)
            .map_err(|_| CallFrameEnterFailure::Abort(TypeConstraintAbort::ArithmeticOverflow))?;
        let slot = self.nonce_lineage.get(slot_index).ok_or_else(|| {
            CallFrameEnterFailure::Invariant(self.mark_poison(CallFrameInvariant::DepthMismatch {
                owner,
                entered_depth: actual,
                actual_depth: current_depth,
            }))
        })?;
        if let Some(actual_lineage) = slot.get() {
            return Err(CallFrameEnterFailure::Invariant(self.mark_poison(
                CallFrameInvariant::NonceMismatch {
                    owner,
                    expected: None,
                    actual: Some(actual_lineage.nonce),
                },
            )));
        }
        self.depth.set(actual);
        if current_depth == 0 {
            self.root.set(Some(owner));
        }
        slot.set(Some(CallFrameNonceLineage { nonce, parent }));
        self.active_nonce.set(Some(nonce));
        self.next_nonce.set(next_nonce);
        let nonce_lineage = self
            .nonce_lineage
            .iter()
            .take(slot_index + 1)
            .map(|slot| {
                slot.get().map(|lineage| lineage.nonce).ok_or_else(|| {
                    CallFrameEnterFailure::Invariant(self.mark_poison(
                        CallFrameInvariant::NonceMismatch {
                            owner,
                            expected: Some(nonce),
                            actual: None,
                        },
                    ))
                })
            })
            .collect::<Result<Box<[_]>, _>>()?;
        Ok(ActiveCallFrame {
            stack: Rc::clone(self),
            owner,
            entered_depth: actual,
            nonce,
            root_owner,
            physical_attempt: PhysicalCallAttemptId {
                issuer: Arc::clone(&self.physical_attempt_issuer),
                root: root_owner,
                owner,
                nonce_lineage,
                candidate_context: None,
            },
            closed: false,
        })
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.depth.get() == 0
    }

    #[cfg(test)]
    pub(super) fn root(&self) -> Option<ExprId> {
        self.root.get()
    }

    #[cfg(test)]
    pub(super) fn is_poisoned(&self) -> bool {
        self.poison.get().is_some()
    }

    fn close_frame(
        &self,
        owner: ExprId,
        entered_depth: u64,
        nonce: u64,
        root_owner: ExprId,
    ) -> Result<(), CallFrameInvariant> {
        if let Some(cause) = self.poison_cause() {
            return Err(cause);
        }
        let actual_depth = self.depth.get();
        let active_nonce = self.active_nonce.get();
        if actual_depth != entered_depth {
            let violation = if actual_depth < entered_depth {
                CallFrameInvariant::StaleClose {
                    owner,
                    entered_depth,
                    actual_depth,
                    nonce,
                }
            } else {
                CallFrameInvariant::OutOfOrderClose {
                    owner,
                    entered_depth,
                    actual_depth,
                    nonce,
                    active_nonce,
                }
            };
            return Err(self.mark_poison(violation));
        }
        if active_nonce != Some(nonce) {
            return Err(self.mark_poison(CallFrameInvariant::OutOfOrderClose {
                owner,
                entered_depth,
                actual_depth,
                nonce,
                active_nonce,
            }));
        }
        if self.root.get() != Some(root_owner) {
            return Err(self.mark_poison(CallFrameInvariant::RootMismatch {
                owner,
                expected: root_owner,
                actual: self.root.get(),
            }));
        }
        if entered_depth == 0 {
            return Err(self.mark_poison(CallFrameInvariant::DepthMismatch {
                owner,
                entered_depth,
                actual_depth,
            }));
        }
        let top_index = usize::try_from(entered_depth - 1).map_err(|_| {
            self.mark_poison(CallFrameInvariant::DepthMismatch {
                owner,
                entered_depth,
                actual_depth,
            })
        })?;
        let Some(top_slot) = self.nonce_lineage.get(top_index) else {
            return Err(self.mark_poison(CallFrameInvariant::DepthMismatch {
                owner,
                entered_depth,
                actual_depth,
            }));
        };
        let Some(top) = top_slot.get() else {
            return Err(self.mark_poison(CallFrameInvariant::NonceMismatch {
                owner,
                expected: Some(nonce),
                actual: self.active_nonce.get(),
            }));
        };
        if top.nonce != nonce || self.active_nonce.get() != Some(top.nonce) {
            return Err(self.mark_poison(CallFrameInvariant::OutOfOrderClose {
                owner,
                entered_depth,
                actual_depth,
                nonce,
                active_nonce,
            }));
        }
        let remaining = entered_depth - 1;
        let restored_nonce = if remaining == 0 {
            if top.parent.is_some() {
                return Err(self.mark_poison(CallFrameInvariant::NonceMismatch {
                    owner,
                    expected: None,
                    actual: top.parent,
                }));
            }
            None
        } else {
            let parent_index = usize::try_from(remaining - 1).map_err(|_| {
                self.mark_poison(CallFrameInvariant::DepthMismatch {
                    owner,
                    entered_depth,
                    actual_depth,
                })
            })?;
            let Some(parent) = self.nonce_lineage.get(parent_index).and_then(Cell::get) else {
                return Err(self.mark_poison(CallFrameInvariant::NonceMismatch {
                    owner,
                    expected: top.parent,
                    actual: None,
                }));
            };
            if top.parent != Some(parent.nonce) {
                return Err(self.mark_poison(CallFrameInvariant::NonceMismatch {
                    owner,
                    expected: Some(parent.nonce),
                    actual: top.parent,
                }));
            }
            Some(parent.nonce)
        };

        top_slot.set(None);
        self.depth.set(remaining);
        self.active_nonce.set(restored_nonce);
        if remaining == 0 {
            self.root.set(None);
            self.validate_empty_root(owner, root_owner)?;
        }
        Ok(())
    }

    fn validate_empty_root(
        &self,
        owner: ExprId,
        root_owner: ExprId,
    ) -> Result<(), CallFrameInvariant> {
        if self.depth.get() != 0 {
            return Err(self.mark_poison(CallFrameInvariant::DepthMismatch {
                owner,
                entered_depth: 0,
                actual_depth: self.depth.get(),
            }));
        }
        if let Some(actual) = self.root.get() {
            return Err(self.mark_poison(CallFrameInvariant::RootMismatch {
                owner,
                expected: root_owner,
                actual: Some(actual),
            }));
        }
        if let Some(actual) = self.active_nonce.get() {
            return Err(self.mark_poison(CallFrameInvariant::NonceMismatch {
                owner,
                expected: None,
                actual: Some(actual),
            }));
        }
        if let Some(actual) = self.nonce_lineage.iter().find_map(Cell::get) {
            return Err(self.mark_poison(CallFrameInvariant::NonceMismatch {
                owner,
                expected: None,
                actual: Some(actual.nonce),
            }));
        }
        if let Some(cause) = self.poison_cause() {
            return Err(cause);
        }
        Ok(())
    }
}

pub(super) struct ActiveCallFrame {
    stack: Rc<CallFrameStack>,
    owner: ExprId,
    entered_depth: u64,
    nonce: u64,
    root_owner: ExprId,
    physical_attempt: PhysicalCallAttemptId,
    closed: bool,
}

impl ActiveCallFrame {
    pub(super) fn physical_attempt(
        &self,
        context: &AnalyzerExpressionContext<'_>,
    ) -> PhysicalCallAttemptId {
        let mut attempt = self.physical_attempt.clone();
        attempt.candidate_context = context.physical_attempt_context();
        attempt
    }

    pub(super) fn close(mut self) -> Result<(), CallFrameInvariant> {
        self.closed = true;
        self.stack
            .close_frame(self.owner, self.entered_depth, self.nonce, self.root_owner)
    }
}

impl Drop for ActiveCallFrame {
    fn drop(&mut self) {
        if !self.closed {
            let actual_depth = self.stack.depth.get();
            let active_nonce = self.stack.active_nonce.get();
            let violation = if actual_depth < self.entered_depth {
                CallFrameInvariant::StaleClose {
                    owner: self.owner,
                    entered_depth: self.entered_depth,
                    actual_depth,
                    nonce: self.nonce,
                }
            } else if actual_depth > self.entered_depth {
                CallFrameInvariant::OutOfOrderClose {
                    owner: self.owner,
                    entered_depth: self.entered_depth,
                    actual_depth,
                    nonce: self.nonce,
                    active_nonce,
                }
            } else if active_nonce != Some(self.nonce) {
                CallFrameInvariant::OutOfOrderClose {
                    owner: self.owner,
                    entered_depth: self.entered_depth,
                    actual_depth,
                    nonce: self.nonce,
                    active_nonce,
                }
            } else {
                CallFrameInvariant::Unclosed {
                    owner: self.owner,
                    entered_depth: self.entered_depth,
                    nonce: self.nonce,
                }
            };
            self.stack.mark_poison(violation);
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum AnalyzerExpressionRejection {
    Unavailable { owner: ExprId },
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum AnalyzerExpressionInvariant {
    Fact(Box<CandidateFactTransactionViolation>),
    Semantic(Box<FinalSemanticAnalysisError>),
    Cycle {
        owner: ExprId,
    },
    CallFrame {
        owner: ExprId,
        violation: Box<CallFrameInvariant>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum AnalyzerExpressionError {
    Rejected(AnalyzerExpressionRejection),
    Fatal(Box<FinalSemanticAnalysisError>),
    Abort(TypeConstraintAbort),
    Invariant(AnalyzerExpressionInvariant),
    Call {
        owner: ExprId,
        failure: CallAnalysisFailure,
    },
}

impl AnalyzerExpressionError {
    pub(super) fn fatal(error: FinalSemanticAnalysisError) -> Self {
        Self::Fatal(Box::new(error))
    }

    pub(super) fn invariant(error: FinalSemanticAnalysisError) -> Self {
        Self::Invariant(AnalyzerExpressionInvariant::Semantic(Box::new(error)))
    }

    pub(super) fn fact(violation: CandidateFactTransactionViolation) -> Self {
        Self::Invariant(AnalyzerExpressionInvariant::Fact(Box::new(violation)))
    }

    pub(super) fn rejected(owner: ExprId) -> Self {
        Self::Rejected(AnalyzerExpressionRejection::Unavailable { owner })
    }

    pub(super) fn is_cancellation(&self) -> bool {
        match self {
            Self::Abort(TypeConstraintAbort::Cancelled) => true,
            Self::Fatal(error) => matches!(error.as_ref(), FinalSemanticAnalysisError::Cancelled),
            Self::Invariant(invariant) => matches!(
                invariant,
                AnalyzerExpressionInvariant::Semantic(error)
                    if matches!(error.as_ref(), FinalSemanticAnalysisError::Cancelled)
            ),
            Self::Call { failure, .. } => matches!(
                failure,
                CallAnalysisFailure::Abort(TypeConstraintAbort::Cancelled)
            ),
            Self::Rejected(_)
            | Self::Abort(
                TypeConstraintAbort::ArithmeticOverflow
                | TypeConstraintAbort::WorkLimit { .. }
                | TypeConstraintAbort::NodeLimit { .. }
                | TypeConstraintAbort::BranchLimit { .. }
                | TypeConstraintAbort::BindingLimit { .. }
                | TypeConstraintAbort::SourceProbeLimit { .. }
                | TypeConstraintAbort::MaterializationLimit { .. }
                | TypeConstraintAbort::CallDepth { .. },
            ) => false,
        }
    }

    pub(super) fn into_public(self, owner: ExprId) -> FinalSemanticAnalysisError {
        match self {
            Self::Rejected(_) => FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
            Self::Fatal(error) => *error,
            Self::Abort(TypeConstraintAbort::Cancelled) => FinalSemanticAnalysisError::Cancelled,
            Self::Abort(TypeConstraintAbort::ArithmeticOverflow) => {
                FinalSemanticAnalysisError::AccountingOverflow
            }
            Self::Abort(TypeConstraintAbort::CallDepth { .. })
            | Self::Abort(TypeConstraintAbort::WorkLimit { .. })
            | Self::Abort(TypeConstraintAbort::NodeLimit { .. })
            | Self::Abort(TypeConstraintAbort::BranchLimit { .. })
            | Self::Abort(TypeConstraintAbort::BindingLimit { .. })
            | Self::Abort(TypeConstraintAbort::SourceProbeLimit { .. })
            | Self::Abort(TypeConstraintAbort::MaterializationLimit { .. }) => {
                FinalSemanticAnalysisError::CallResolutionFailed { owner }
            }
            Self::Invariant(invariant) => match invariant {
                AnalyzerExpressionInvariant::Fact(violation) => (*violation).into(),
                AnalyzerExpressionInvariant::Semantic(error) => *error,
                AnalyzerExpressionInvariant::Cycle { owner } => {
                    FinalSemanticAnalysisError::ExpressionCycle { owner }
                }
                AnalyzerExpressionInvariant::CallFrame { owner, violation } => {
                    FinalSemanticAnalysisError::CallFrameInvariant(
                        crate::final_analysis::FinalCallFrameInvariant::new(owner, *violation),
                    )
                }
            },
            Self::Call { owner, failure } => FinalSemanticAnalysisError::CallConstraintFailure(
                FinalCallConstraintFailure::new(owner, failure),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_owner() -> ExprId {
        let fixture = crate::final_analysis::tests::fixture("fn caller() { 1; }\n", None);
        fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root module")
            .expressions()
            .next()
            .map(|(owner, _)| owner)
            .expect("expression owner")
    }

    #[test]
    fn nested_frames_close_affinely_and_clear_root() {
        let owner = test_owner();
        let stack = CallFrameStack::new(2).expect("frame stack");
        let outer = stack.enter(owner).expect("outer frame");
        let inner = stack.enter(owner).expect("inner frame");
        assert_eq!(stack.depth.get(), 2);
        assert_eq!(stack.root(), Some(owner));
        inner.close().expect("inner close");
        assert_eq!(stack.depth.get(), 1);
        assert_eq!(stack.active_nonce.get(), Some(0));
        outer.close().expect("outer close");
        assert!(stack.is_empty());
        assert_eq!(stack.root(), None);
        assert_eq!(stack.active_nonce.get(), None);
        assert!(stack.nonce_lineage.iter().all(|slot| slot.get().is_none()));
        assert!(!stack.is_poisoned());
    }

    #[test]
    fn stale_or_out_of_order_close_poison_preserves_live_frame_state() {
        let owner = test_owner();
        let stack = CallFrameStack::new(2).expect("frame stack");
        let outer = stack.enter(owner).expect("outer frame");
        let inner = stack.enter(owner).expect("inner frame");
        let close = outer.close();
        let cause = close.expect_err("out-of-order close cause");
        assert!(matches!(&cause, CallFrameInvariant::OutOfOrderClose { .. }));
        assert!(stack.is_poisoned());
        assert_eq!(stack.depth.get(), 2);
        assert_eq!(stack.root(), Some(owner));
        assert_eq!(inner.close(), Err(cause));
        assert_eq!(stack.depth.get(), 2);

        let poisoned = stack.enter(owner);
        assert!(matches!(
            poisoned,
            Err(CallFrameEnterFailure::Invariant(
                actual
            )) if actual == cause
        ));
        assert_eq!(stack.depth.get(), 2);
    }

    #[test]
    fn parent_nonce_tamper_is_rejected_before_restore() {
        let owner = test_owner();
        let stack = CallFrameStack::new(2).expect("frame stack");
        let outer = stack.enter(owner).expect("outer frame");
        let inner = stack.enter(owner).expect("inner frame");
        let top = stack.nonce_lineage[1].get().expect("inner lineage");
        stack.nonce_lineage[1].set(Some(CallFrameNonceLineage {
            parent: Some(99),
            ..top
        }));

        let close = inner.close();
        let cause = close.expect_err("parent nonce tamper cause");
        assert!(matches!(
            &cause,
            CallFrameInvariant::NonceMismatch {
                expected: Some(0),
                actual: Some(99),
                ..
            }
        ));
        assert_eq!(stack.depth.get(), 2);
        assert_eq!(stack.active_nonce.get(), Some(top.nonce));
        assert_eq!(outer.close(), Err(cause));
    }

    #[test]
    fn dropping_an_unclosed_frame_poison_marks_without_mutating_state() {
        let owner = test_owner();
        let stack = CallFrameStack::new(2).expect("frame stack");
        let outer = stack.enter(owner).expect("outer frame");
        let inner = stack.enter(owner).expect("inner frame");
        drop(outer);
        assert!(stack.is_poisoned());
        assert_eq!(stack.depth.get(), 2);
        assert_eq!(stack.active_nonce.get(), Some(1));
        let cause = stack.poison_cause().expect("drop poison cause");
        assert_eq!(inner.close(), Err(cause));
        assert_eq!(stack.depth.get(), 2);

        let poisoned = stack.enter(owner);
        assert!(matches!(
            poisoned,
            Err(CallFrameEnterFailure::Invariant(actual)) if actual == cause
        ));
    }

    #[test]
    fn nonce_overflow_aborts_before_any_frame_mutation() {
        let owner = test_owner();
        let stack = CallFrameStack::new(2).expect("frame stack");
        stack.next_nonce.set(u64::MAX);
        let result = stack.enter(owner);
        assert!(matches!(
            result,
            Err(CallFrameEnterFailure::Abort(
                TypeConstraintAbort::ArithmeticOverflow
            ))
        ));
        assert_eq!(stack.depth.get(), 0);
        assert_eq!(stack.root(), None);
        assert_eq!(stack.active_nonce.get(), None);
        assert!(!stack.is_poisoned());
    }

    #[test]
    fn call_frame_invariant_reaches_exact_public_error_carrier() {
        let owner = test_owner();
        let error = AnalyzerExpressionError::Invariant(AnalyzerExpressionInvariant::CallFrame {
            owner,
            violation: Box::new(CallFrameInvariant::DepthMismatch {
                owner,
                entered_depth: 1,
                actual_depth: 0,
            }),
        })
        .into_public(owner);
        assert!(matches!(
            error,
            FinalSemanticAnalysisError::CallFrameInvariant(_)
        ));
    }
}
