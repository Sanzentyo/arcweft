//! Publication control and shared-resolver work accounting.

#[cfg(test)]
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    AssertionBuildProfile, CallableArgumentSlotIndex, CallableCandidateId,
    CheckedCallArgumentSlotSource, ExprId, FinalSemanticAnalysisError, HirCallArgumentOrdinal,
    TypeKind,
};

/// Caller-owned cancellation observed while a semantic generation is staged.
///
/// Cancellation is terminal for the publication attempt. The staged input is
/// consumed, but no [`FinalSemanticAnalysis`] can be observed.
#[derive(Clone, Copy, Debug)]
pub struct FinalSemanticAnalysisControl<'a> {
    cancellation: &'a AtomicBool,
    assertion_build_profile: AssertionBuildProfile,
    #[cfg(test)]
    physical_slots_before_cancellation: Option<&'a Cell<usize>>,
}

impl<'a> FinalSemanticAnalysisControl<'a> {
    pub const fn new(cancellation: &'a AtomicBool) -> Self {
        Self {
            cancellation,
            assertion_build_profile: AssertionBuildProfile::Debug,
            #[cfg(test)]
            physical_slots_before_cancellation: None,
        }
    }

    /// Selects whether Debug assertion work enters executable semantic facts.
    #[must_use]
    pub const fn with_assertion_build_profile(mut self, profile: AssertionBuildProfile) -> Self {
        self.assertion_build_profile = profile;
        self
    }

    pub const fn assertion_build_profile(self) -> AssertionBuildProfile {
        self.assertion_build_profile
    }

    pub(super) const fn cancellation(self) -> &'a AtomicBool {
        self.cancellation
    }

    #[cfg(test)]
    pub(super) fn with_cancellation_after_completed_physical_slots(
        mut self,
        remaining: &'a Cell<usize>,
    ) -> Self {
        self.physical_slots_before_cancellation = Some(remaining);
        self
    }

    pub(super) fn check(self) -> Result<(), FinalSemanticAnalysisError> {
        if self.cancellation.load(Ordering::Acquire) {
            Err(FinalSemanticAnalysisError::Cancelled)
        } else {
            Ok(())
        }
    }

    pub(super) fn check_physical_slot_boundary(self) -> Result<(), FinalSemanticAnalysisError> {
        #[cfg(test)]
        if let Some(remaining) = self.physical_slots_before_cancellation {
            let current = remaining.get();
            if current == 0 {
                self.cancellation.store(true, Ordering::Release);
                return Err(FinalSemanticAnalysisError::Cancelled);
            }
            remaining.set(current - 1);
        }
        self.check()
    }
}

/// Exact publication and shared-resolver work retained with one accepted
/// semantic generation.
///
/// There is deliberately no legacy-dispatch counter. Obsolete dispatch code is
/// deleted; the only call work admitted here is the accounting already sealed
/// into [`CallTargetFacts`] by the shared resolver.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FinalSemanticAnalysisWork {
    pub(super) type_facts: u64,
    pub(super) local_facts: u64,
    pub(super) capture_facts: u64,
    pub(super) expression_facts: u64,
    pub(super) pattern_facts: u64,
    pub(super) statement_facts: u64,
    pub(super) item_facts: u64,
    pub(super) call_facts: u64,
    pub(super) call_diagnostics: u64,
    pub(super) logical_argument_checks: u64,
    pub(super) resolver_invocations: u64,
    pub(super) candidate_argument_probes: u64,
    pub(super) selected_replay_argument_visits: u64,
    pub(super) retained_argument_fact_publications: u64,
}

/// Candidate transaction phase that physically evaluated one argument slot.
///
/// This is crate-owned operational evidence. It is deliberately not part of
/// language semantics or the public signature-help contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateEvaluationPass {
    Probe,
    SelectedReplay,
    RejectedRecoveryReplay,
}

/// Candidate-specific expectation in force at one physical evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CandidateExpectedType {
    Exact(TypeKind),
    Unchecked,
    Unmapped,
}

/// Structural source family physically evaluated for one candidate slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicalArgumentEvaluationKind {
    Authored,
    Recovered,
    FixedLiteralSpread,
    TypedRestSpread,
    Unmapped,
}

/// Argument-owned half of one physical candidate evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PhysicalCandidateArgument {
    argument: HirCallArgumentOrdinal,
    slot: CallableArgumentSlotIndex,
    source: CheckedCallArgumentSlotSource,
    kind: PhysicalArgumentEvaluationKind,
    expected: CandidateExpectedType,
}

impl PhysicalCandidateArgument {
    pub(crate) const fn new(
        argument: HirCallArgumentOrdinal,
        slot: CallableArgumentSlotIndex,
        source: CheckedCallArgumentSlotSource,
        kind: PhysicalArgumentEvaluationKind,
        expected: CandidateExpectedType,
    ) -> Self {
        Self {
            argument,
            slot,
            source,
            kind,
            expected,
        }
    }
}

/// One bounded, typed observation emitted immediately before a real slot
/// evaluation. Rolled-back candidate semantics never erase this record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PhysicalCandidateArgumentEvaluation {
    call_expression: ExprId,
    candidate: CallableCandidateId,
    pass: CandidateEvaluationPass,
    argument: HirCallArgumentOrdinal,
    slot: CallableArgumentSlotIndex,
    source: CheckedCallArgumentSlotSource,
    kind: PhysicalArgumentEvaluationKind,
    expected: CandidateExpectedType,
}

impl PhysicalCandidateArgumentEvaluation {
    pub(crate) fn new(
        call_expression: ExprId,
        candidate: CallableCandidateId,
        pass: CandidateEvaluationPass,
        argument: PhysicalCandidateArgument,
    ) -> Self {
        Self {
            call_expression,
            candidate,
            pass,
            argument: argument.argument,
            slot: argument.slot,
            source: argument.source,
            kind: argument.kind,
            expected: argument.expected,
        }
    }

    pub(crate) const fn call_expression(&self) -> ExprId {
        self.call_expression
    }

    #[cfg(test)]
    pub(crate) const fn candidate(&self) -> &CallableCandidateId {
        &self.candidate
    }

    #[cfg(test)]
    pub(crate) const fn pass(&self) -> CandidateEvaluationPass {
        self.pass
    }

    pub(crate) const fn argument(&self) -> HirCallArgumentOrdinal {
        self.argument
    }

    #[cfg(test)]
    pub(crate) const fn slot(&self) -> CallableArgumentSlotIndex {
        self.slot
    }

    pub(crate) const fn source(&self) -> CheckedCallArgumentSlotSource {
        self.source
    }

    #[cfg(test)]
    pub(crate) const fn kind(&self) -> PhysicalArgumentEvaluationKind {
        self.kind
    }

    #[cfg(test)]
    pub(crate) const fn expected(&self) -> &CandidateExpectedType {
        &self.expected
    }
}

impl FinalSemanticAnalysisWork {
    pub const fn type_facts(self) -> u64 {
        self.type_facts
    }

    pub const fn local_facts(self) -> u64 {
        self.local_facts
    }

    pub const fn capture_facts(self) -> u64 {
        self.capture_facts
    }

    pub const fn expression_facts(self) -> u64 {
        self.expression_facts
    }

    pub const fn pattern_facts(self) -> u64 {
        self.pattern_facts
    }

    pub const fn statement_facts(self) -> u64 {
        self.statement_facts
    }

    pub const fn item_facts(self) -> u64 {
        self.item_facts
    }

    pub const fn call_facts(self) -> u64 {
        self.call_facts
    }

    pub const fn call_diagnostics(self) -> u64 {
        self.call_diagnostics
    }

    pub const fn logical_argument_checks(self) -> u64 {
        self.logical_argument_checks
    }

    pub const fn resolver_invocations(self) -> u64 {
        self.resolver_invocations
    }

    pub const fn candidate_argument_probes(self) -> u64 {
        self.candidate_argument_probes
    }

    pub const fn selected_replay_argument_visits(self) -> u64 {
        self.selected_replay_argument_visits
    }

    pub const fn retained_argument_fact_publications(self) -> u64 {
        self.retained_argument_fact_publications
    }
}
