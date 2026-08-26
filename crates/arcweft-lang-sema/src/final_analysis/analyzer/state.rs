//! Candidate-transaction state and mutation rollback.

use std::sync::Arc;

use super::calls::{
    AnalyzerPreparedCallGraph, AnalyzerPreparedCallPrefix, AnalyzerPreparedUnselectedCall,
};
use super::expression_error::PhysicalCallAttemptId;
use super::{
    BTreeMap, BTreeSet, CallTargetFacts, CandidateFactTransactionViolation, CheckedIteration,
    ExprId, LocalId, PatternId, PhysicalCandidateArgumentEvaluation, TypeKind,
};
use crate::callable::{
    CheckedCallSite, PreparedCallContinuationRef, PreparedCallGraph, PreparedCallGraphCheckpoint,
    PreparedCallGraphDelta, PreparedCallGraphReplayMismatch, PreparedCallGraphSiteState,
};
use crate::final_analysis::PreparedExpressionFact;

#[derive(Debug)]
struct CandidateFactIssuer;

/// A move-only token naming one candidate transaction.
///
/// The journal offset is deliberately carried by the token rather than being
/// exposed as an untyped `usize`.  Only [`SemanticFactState`] can mint a
/// token, and every consuming operation validates that the token is the
/// active LIFO transaction before changing state.
pub(super) struct CandidateFactCheckpoint {
    issuer: Arc<CandidateFactIssuer>,
    id: CandidateFactCheckpointId,
    journal_start: CandidateFactJournalCursor,
    epoch: u64,
    graph: PreparedCallGraphCheckpoint,
}

impl std::fmt::Debug for CandidateFactCheckpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CandidateFactCheckpoint")
            .field("id", &self.id)
            .field("journal_start", &self.journal_start)
            .field("epoch", &self.epoch)
            .finish()
    }
}

/// Exact-top authority for checkpoint close and projection application.
#[derive(Debug)]
pub(super) struct CandidateFactTransactionAuthority<'a> {
    checkpoint: &'a CandidateFactCheckpoint,
}

/// Purpose-specific fact scope owned by one analyzer callback.  The raw
/// checkpoint never crosses the callback client boundary; only this move-only
/// scope can be closed by the analyzer-owned purpose API.
pub(super) struct ActiveCallbackFactScope {
    checkpoint: CandidateFactCheckpoint,
}

/// Probe callback identity.  It carries no semantic facts and cannot be used
/// to close a materialization scope.
pub(crate) struct ProbeFactCheckpoint {
    issuer: Arc<CandidateFactIssuer>,
    epoch: u64,
    id: CandidateFactCheckpointId,
}

/// Materialization callback identity.  It carries no semantic facts and is a
/// distinct affine type from [`ProbeFactCheckpoint`].
pub(crate) struct MaterializationFactCheckpoint {
    issuer: Arc<CandidateFactIssuer>,
    epoch: u64,
    id: CandidateFactCheckpointId,
}

impl ActiveCallbackFactScope {
    fn id(&self) -> CandidateFactCheckpointId {
        self.checkpoint.id
    }

    pub(super) fn probe_checkpoint(&self) -> ProbeFactCheckpoint {
        ProbeFactCheckpoint {
            issuer: Arc::clone(&self.checkpoint.issuer),
            epoch: self.checkpoint.epoch,
            id: self.id(),
        }
    }

    pub(super) fn materialization_checkpoint(&self) -> MaterializationFactCheckpoint {
        MaterializationFactCheckpoint {
            issuer: Arc::clone(&self.checkpoint.issuer),
            epoch: self.checkpoint.epoch,
            id: self.id(),
        }
    }

    pub(super) fn matches_probe_checkpoint(&self, checkpoint: &ProbeFactCheckpoint) -> bool {
        Arc::ptr_eq(&self.checkpoint.issuer, &checkpoint.issuer)
            && self.checkpoint.epoch == checkpoint.epoch
            && self.id() == checkpoint.id
    }

    pub(super) fn matches_materialization_checkpoint(
        &self,
        checkpoint: &MaterializationFactCheckpoint,
    ) -> bool {
        Arc::ptr_eq(&self.checkpoint.issuer, &checkpoint.issuer)
            && self.checkpoint.epoch == checkpoint.epoch
            && self.id() == checkpoint.id
    }

    fn into_checkpoint(self) -> CandidateFactCheckpoint {
        self.checkpoint
    }
}

/// Borrowed authority for candidate-sensitive expression facts. It may name an
/// active ancestor, with a visibility cursor that prevents baseline reads.
#[derive(Debug)]
pub(super) struct CandidateExpressionFactAuthority<'a> {
    checkpoint: &'a CandidateFactCheckpoint,
    visibility: CandidateFactJournalCursor,
}

#[derive(Clone)]
pub(super) struct CandidatePhysicalAttemptContext {
    issuer: Arc<CandidateFactIssuer>,
    epoch: u64,
    id: CandidateFactCheckpointId,
}

impl PartialEq for CandidatePhysicalAttemptContext {
    fn eq(&self, other: &Self) -> bool {
        self.epoch == other.epoch && self.id == other.id
    }
}

impl Eq for CandidatePhysicalAttemptContext {}

impl CandidatePhysicalAttemptContext {
    pub(super) fn same_issued_context(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer) && self == other
    }
}

impl<'a> CandidateExpressionFactAuthority<'a> {
    pub(super) fn reborrow<'b>(&'b self) -> CandidateExpressionFactAuthority<'b>
    where
        'a: 'b,
    {
        CandidateExpressionFactAuthority {
            checkpoint: self.checkpoint,
            visibility: self.visibility,
        }
    }

    pub(super) fn physical_attempt_context(&self) -> CandidatePhysicalAttemptContext {
        CandidatePhysicalAttemptContext {
            issuer: Arc::clone(&self.checkpoint.issuer),
            epoch: self.checkpoint.epoch,
            id: self.checkpoint.id,
        }
    }
}

#[derive(Debug)]
pub(super) struct CandidateFactCloseFailure {
    violation: CandidateFactTransactionViolation,
    checkpoint: CandidateFactCheckpoint,
}

impl CandidateFactCloseFailure {
    pub(super) fn into_parts(self) -> (CandidateFactTransactionViolation, CandidateFactCheckpoint) {
        (self.violation, self.checkpoint)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CandidateFactCheckpointId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CandidateFactJournalCursor(usize);

#[derive(Clone, Copy)]
struct ActiveCandidateCheckpoint {
    id: CandidateFactCheckpointId,
    journal_start: CandidateFactJournalCursor,
}

/// One use of an implicit callable's body expression that resolved to a
/// captured local.  The key is deliberately private: callers may only create
/// rows through [`SemanticFactState::record_implicit_capture_use`], so a
/// candidate cannot manufacture an unjournaled fact key.
pub(super) type ImplicitCaptureUseKey = (ExprId, ExprId);

#[derive(Debug)]
pub(super) struct CandidateProjectionApplyFailure {
    violation: CandidateFactTransactionViolation,
    projection: CandidateSemanticProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ExpressionFactWriteViolation {
    AlreadyPublished,
    MissingPublishedFact,
    Candidate(CandidateFactTransactionViolation),
}

impl CandidateProjectionApplyFailure {
    pub(super) fn into_parts(
        self,
    ) -> (
        CandidateFactTransactionViolation,
        CandidateSemanticProjection,
    ) {
        (self.violation, self.projection)
    }
}

/// Sole mutable owner for candidate-sensitive semantic facts.
///
/// Every write flows through this owner so candidate probes cannot bypass the
/// rollback journal by mutating one fact map directly.
pub(super) struct SemanticFactState {
    issuer: Arc<CandidateFactIssuer>,
    locals: BTreeMap<LocalId, TypeKind>,
    patterns: BTreeMap<PatternId, TypeKind>,
    expressions: BTreeMap<ExprId, PreparedExpressionFact>,
    expression_stack: BTreeSet<ExprId>,
    iteration_facts: BTreeMap<ExprId, CheckedIteration>,
    implicit_capture_uses: BTreeMap<ImplicitCaptureUseKey, LocalId>,
    prepared_calls: Option<AnalyzerPreparedCallGraph>,
    final_calls: BTreeMap<ExprId, CallTargetFacts>,
    physical_candidate_argument_evaluations: PhysicalCandidateEvaluationTranscript,
    active_physical_candidate_argument_evaluations: Option<PhysicalCandidateEvaluationTranscript>,
    active_physical_call_attempts: Vec<ActivePhysicalCallAttempt>,
    next_checkpoint_id: u64,
    candidate_checkpoints: Vec<ActiveCandidateCheckpoint>,
    candidate_journal: Vec<SemanticFactMutation>,
    poison: Option<CandidateFactTransactionViolation>,
    epoch: u64,
}

#[derive(Debug)]
struct CandidateProjectionAuthority {
    issuer: Arc<CandidateFactIssuer>,
    epoch: u64,
}

/// Move-only semantic facts produced by one completed candidate attempt.
///
/// Accepted selection and deterministic recovery both consume this carrier:
/// values are removed from the candidate owner when it is extracted, so
/// applying a primary projection transfers ownership rather than cloning a
/// second copy of the candidate state. Non-primary attempts are explicitly
/// validated and discarded.
pub(super) struct CandidateSemanticProjection {
    authority: CandidateProjectionAuthority,
    graph_delta: PreparedCallGraphDelta<AnalyzerPreparedCallPrefix, AnalyzerPreparedUnselectedCall>,
    locals: BTreeMap<LocalId, Option<TypeKind>>,
    patterns: BTreeMap<PatternId, Option<TypeKind>>,
    expressions: BTreeMap<ExprId, Option<PreparedExpressionFact>>,
    iterations: BTreeMap<ExprId, Option<CheckedIteration>>,
    implicit_capture_uses: BTreeMap<ImplicitCaptureUseKey, Option<LocalId>>,
    physical_candidate_argument_evaluations: PhysicalCandidateEvaluationTranscript,
}

/// One root candidate transaction's operational evaluation transcript.
///
/// Candidate probes nested under the root may commit, roll back, or extract
/// semantic projections without erasing physical work that already happened.
/// The root transaction alone publishes, rolls back, or transfers the batch.
#[derive(Default, Eq, PartialEq)]
struct PhysicalCandidateEvaluationTranscript {
    rows: BTreeMap<ExprId, Vec<PhysicalCandidateArgumentEvaluation>>,
}

struct ActivePhysicalCallAttempt {
    attempt: PhysicalCallAttemptId,
    transcript: PhysicalCandidateEvaluationTranscript,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PhysicalCallAttemptClose {
    Completed,
    Failed,
    Cancelled,
}

pub(super) enum PhysicalCandidateEvaluationAdmission {
    Recorded,
    Duplicate,
    LimitReached,
}

impl PhysicalCandidateEvaluationTranscript {
    fn row_count(&self) -> usize {
        self.rows.values().map(Vec::len).sum()
    }

    fn root_row_count(&self, root: ExprId) -> usize {
        self.rows.get(&root).map_or(0, Vec::len)
    }

    fn matching_row(
        &self,
        root: ExprId,
        proposed: &PhysicalCandidateArgumentEvaluation,
    ) -> Option<&PhysicalCandidateArgumentEvaluation> {
        self.rows
            .get(&root)?
            .iter()
            .find(|existing| existing.same_candidate_slot(proposed))
    }

    fn merge(&mut self, other: Self) {
        for (root, rows) in other.rows {
            let target = self.rows.entry(root).or_default();
            for proposed in rows {
                if !target
                    .iter()
                    .any(|existing| existing.same_candidate_slot(&proposed))
                {
                    target.push(proposed);
                }
            }
        }
    }
}

/// The operation requested after a successful candidate evaluation.  The
/// runner consumes this value, so callers cannot close a checkpoint through a
/// second side channel.
pub(super) enum CandidateFactTransactionAction<T> {
    Commit(T),
    Rollback(T),
    Extract(T),
}

pub(super) enum CandidateFactOperationFailure {
    Expression(super::expression_error::AnalyzerExpressionError),
    Projection(CandidateProjectionApplyFailure),
}

impl From<super::expression_error::AnalyzerExpressionError> for CandidateFactOperationFailure {
    fn from(error: super::expression_error::AnalyzerExpressionError) -> Self {
        Self::Expression(error)
    }
}

impl CandidateFactOperationFailure {
    fn into_expression_error(self) -> super::expression_error::AnalyzerExpressionError {
        match self {
            Self::Expression(error) => error,
            Self::Projection(failure) => {
                let (violation, _projection) = failure.into_parts();
                super::expression_error::AnalyzerExpressionError::fact(violation)
            }
        }
    }
}

/// Result of the one analyzer-owned candidate transaction runner.
pub(super) enum CandidateFactTransactionOutcome<T> {
    Committed(T),
    RolledBack(T),
    Extracted {
        value: T,
        projection: CandidateSemanticProjection,
    },
}

impl<T> CandidateFactTransactionOutcome<T> {
    pub(super) fn into_committed(self) -> Result<T, CandidateFactTransactionViolation> {
        match self {
            Self::Committed(value) => Ok(value),
            Self::RolledBack(_) | Self::Extracted { .. } => {
                Err(CandidateFactTransactionViolation::UnrecoverableLedger)
            }
        }
    }

    pub(super) fn into_extracted(
        self,
    ) -> Result<(T, CandidateSemanticProjection), CandidateFactTransactionViolation> {
        match self {
            Self::Extracted { value, projection } => Ok((value, projection)),
            Self::Committed(_) | Self::RolledBack(_) => {
                Err(CandidateFactTransactionViolation::UnrecoverableLedger)
            }
        }
    }
}

impl std::fmt::Debug for CandidateSemanticProjection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CandidateSemanticProjection")
            .field("epoch", &self.authority.epoch)
            .field("locals", &self.locals.len())
            .field("patterns", &self.patterns.len())
            .field("expressions", &self.expressions.len())
            .field("iterations", &self.iterations.len())
            .field("implicit_capture_uses", &self.implicit_capture_uses.len())
            .field(
                "physical_candidate_argument_evaluations",
                &self.physical_candidate_argument_evaluations.row_count(),
            )
            .field("graph_delta", &"sealed")
            .finish()
    }
}

impl PartialEq for CandidateSemanticProjection {
    fn eq(&self, other: &Self) -> bool {
        self.replay_eq(other)
    }
}

impl Eq for CandidateSemanticProjection {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CandidateSemanticReplayMismatch {
    Authority,
    PreparedGraph(PreparedCallGraphReplayMismatch),
    Locals,
    Patterns,
    Expressions,
    Iterations,
    ImplicitCaptureUses,
    PhysicalCandidateEvaluations,
}

impl CandidateSemanticProjection {
    pub(super) fn replay_eq(&self, other: &Self) -> bool {
        self.replay_mismatch(other).is_none()
    }

    pub(super) fn replay_mismatch(&self, other: &Self) -> Option<CandidateSemanticReplayMismatch> {
        if let Some(mismatch) = self.semantic_replay_mismatch(other) {
            return Some(mismatch);
        }
        if self.physical_candidate_argument_evaluations
            != other.physical_candidate_argument_evaluations
        {
            return Some(CandidateSemanticReplayMismatch::PhysicalCandidateEvaluations);
        }
        None
    }

    pub(super) fn semantic_replay_mismatch(
        &self,
        other: &Self,
    ) -> Option<CandidateSemanticReplayMismatch> {
        if !Arc::ptr_eq(&self.authority.issuer, &other.authority.issuer)
            || self.authority.epoch != other.authority.epoch
        {
            return Some(CandidateSemanticReplayMismatch::Authority);
        }
        if let Some(mismatch) = self.graph_delta.replay_mismatch(&other.graph_delta) {
            return Some(CandidateSemanticReplayMismatch::PreparedGraph(mismatch));
        }
        if self.locals != other.locals {
            return Some(CandidateSemanticReplayMismatch::Locals);
        }
        if self.patterns != other.patterns {
            return Some(CandidateSemanticReplayMismatch::Patterns);
        }
        if self.expressions != other.expressions {
            return Some(CandidateSemanticReplayMismatch::Expressions);
        }
        if self.iterations != other.iterations {
            return Some(CandidateSemanticReplayMismatch::Iterations);
        }
        if self.implicit_capture_uses != other.implicit_capture_uses {
            return Some(CandidateSemanticReplayMismatch::ImplicitCaptureUses);
        }
        None
    }
}

#[derive(Default)]
struct CandidateProjectionOwners {
    locals: BTreeSet<LocalId>,
    patterns: BTreeSet<PatternId>,
    expressions: BTreeSet<ExprId>,
    iterations: BTreeSet<ExprId>,
    implicit_capture_uses: BTreeSet<ImplicitCaptureUseKey>,
}

enum SemanticFactMutation {
    Local {
        owner: LocalId,
        previous: Option<Box<TypeKind>>,
    },
    Pattern {
        owner: PatternId,
        previous: Option<Box<TypeKind>>,
    },
    Expression {
        owner: ExprId,
        previous: Option<Box<PreparedExpressionFact>>,
    },
    Iteration {
        owner: ExprId,
        previous: Option<Box<CheckedIteration>>,
    },
    ImplicitCaptureUse {
        key: ImplicitCaptureUseKey,
        previous: Option<LocalId>,
    },
}

impl SemanticFactState {
    pub(super) fn new() -> Self {
        Self {
            issuer: Arc::new(CandidateFactIssuer),
            locals: BTreeMap::new(),
            patterns: BTreeMap::new(),
            expressions: BTreeMap::new(),
            expression_stack: BTreeSet::new(),
            iteration_facts: BTreeMap::new(),
            implicit_capture_uses: BTreeMap::new(),
            prepared_calls: Some(PreparedCallGraph::new()),
            final_calls: BTreeMap::new(),
            physical_candidate_argument_evaluations: PhysicalCandidateEvaluationTranscript::default(
            ),
            active_physical_candidate_argument_evaluations: None,
            active_physical_call_attempts: Vec::new(),
            next_checkpoint_id: 0,
            candidate_checkpoints: Vec::new(),
            candidate_journal: Vec::new(),
            poison: None,
            epoch: 0,
        }
    }

    pub(super) fn candidate_expression_authority<'a>(
        &self,
        checkpoint: &'a CandidateFactCheckpoint,
    ) -> Result<CandidateExpressionFactAuthority<'a>, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if !Arc::ptr_eq(&self.issuer, &checkpoint.issuer) {
            return Err(CandidateFactTransactionViolation::ForeignCheckpoint);
        }
        if checkpoint.epoch != self.epoch {
            return Err(CandidateFactTransactionViolation::StaleCheckpoint);
        }
        let active = self
            .candidate_checkpoints
            .iter()
            .find(|active| active.id == checkpoint.id)
            .ok_or(CandidateFactTransactionViolation::StaleCheckpoint)?;
        if checkpoint.journal_start != active.journal_start
            || checkpoint.journal_start.0 > self.candidate_journal.len()
        {
            return Err(CandidateFactTransactionViolation::JournalCursorMismatch);
        }
        self.prepared_calls()?
            .validate_ancestor_checkpoint(&checkpoint.graph)
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })?;
        Ok(CandidateExpressionFactAuthority {
            checkpoint,
            visibility: checkpoint.journal_start,
        })
    }

    /// Opens a purpose-specific callback fact scope.  Callers never receive
    /// the raw checkpoint; they receive only a phase-typed identity derived
    /// from the returned scope.
    pub(super) fn open_callback_fact_scope(
        &mut self,
    ) -> Result<ActiveCallbackFactScope, CandidateFactTransactionViolation> {
        let checkpoint = self.begin_candidate_transaction()?;
        Ok(ActiveCallbackFactScope { checkpoint })
    }

    pub(super) fn callback_fact_authority<'a>(
        &self,
        scope: &'a ActiveCallbackFactScope,
    ) -> Result<CandidateExpressionFactAuthority<'a>, CandidateFactTransactionViolation> {
        self.candidate_expression_authority(&scope.checkpoint)
    }

    pub(super) fn rollback_callback_fact_scope(
        &mut self,
        scope: ActiveCallbackFactScope,
    ) -> Result<(), CandidateFactCloseFailure> {
        let checkpoint = scope.into_checkpoint();
        self.rollback_candidate_transaction(checkpoint)
    }

    pub(super) fn extract_callback_fact_scope(
        &mut self,
        scope: ActiveCallbackFactScope,
    ) -> Result<CandidateSemanticProjection, CandidateFactCloseFailure> {
        let checkpoint = scope.into_checkpoint();
        self.extract_and_rollback(checkpoint)
    }

    pub(super) fn abort_callback_scope_close_failure(
        &mut self,
        failure: CandidateFactCloseFailure,
    ) -> CandidateFactTransactionViolation {
        self.abort_after_close_failure(failure)
    }

    pub(super) fn transaction_authority<'a>(
        &self,
        checkpoint: &'a CandidateFactCheckpoint,
    ) -> Result<CandidateFactTransactionAuthority<'a>, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if !Arc::ptr_eq(&self.issuer, &checkpoint.issuer) {
            return Err(CandidateFactTransactionViolation::ForeignCheckpoint);
        }
        if checkpoint.epoch != self.epoch {
            return Err(CandidateFactTransactionViolation::StaleCheckpoint);
        }
        if checkpoint.journal_start.0 > self.candidate_journal.len() {
            return Err(CandidateFactTransactionViolation::JournalCursorMismatch);
        }
        if !self.candidate_checkpoints.last().is_some_and(|active| {
            active.id == checkpoint.id && active.journal_start == checkpoint.journal_start
        }) {
            return Err(CandidateFactTransactionViolation::NonLifoCheckpoint);
        }
        self.prepared_calls()?
            .validate_checkpoint(&checkpoint.graph)
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })?;
        Ok(CandidateFactTransactionAuthority { checkpoint })
    }

    pub(super) fn ensure_healthy(&self) -> Result<(), CandidateFactTransactionViolation> {
        if self.poison.is_some() {
            Err(CandidateFactTransactionViolation::Poisoned)
        } else {
            Ok(())
        }
    }

    /// Returns an expression only when it was written by the active candidate
    /// checkpoint.  A candidate evaluator must never use the published map as
    /// a baseline: an expression that happens to exist there belongs to a
    /// different semantic authority.
    pub(super) fn candidate_expression(
        &self,
        authority: &CandidateExpressionFactAuthority<'_>,
        owner: ExprId,
    ) -> Result<Option<PreparedExpressionFact>, CandidateFactTransactionViolation> {
        self.validate_candidate_authority(authority)?;
        let start = authority.visibility.0;
        let touched = self.candidate_journal[start..].iter().any(|mutation| {
            matches!(mutation, SemanticFactMutation::Expression { owner: candidate, .. } if *candidate == owner)
        });
        Ok(touched
            .then(|| self.expressions.get(&owner).cloned())
            .flatten())
    }

    pub(super) fn validate_candidate_authority(
        &self,
        authority: &CandidateExpressionFactAuthority<'_>,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if !Arc::ptr_eq(&self.issuer, &authority.checkpoint.issuer) {
            return Err(CandidateFactTransactionViolation::ForeignCheckpoint);
        }
        if authority.checkpoint.epoch != self.epoch {
            return Err(CandidateFactTransactionViolation::StaleCheckpoint);
        }
        let active = self
            .candidate_checkpoints
            .iter()
            .find(|active| {
                active.id == authority.checkpoint.id
                    && active.journal_start == authority.checkpoint.journal_start
            })
            .ok_or(CandidateFactTransactionViolation::StaleCheckpoint)?;
        if authority.checkpoint.journal_start.0 > self.candidate_journal.len()
            || authority.visibility.0 < active.journal_start.0
            || authority.visibility.0 > self.candidate_journal.len()
        {
            return Err(CandidateFactTransactionViolation::JournalCursorMismatch);
        }
        self.prepared_calls()?
            .validate_ancestor_checkpoint(&authority.checkpoint.graph)
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })?;
        Ok(())
    }

    pub(super) const fn locals(&self) -> &BTreeMap<LocalId, TypeKind> {
        &self.locals
    }

    pub(super) fn prepared_calls(
        &self,
    ) -> Result<&AnalyzerPreparedCallGraph, CandidateFactTransactionViolation> {
        self.prepared_calls.as_ref().ok_or_else(|| {
            CandidateFactTransactionViolation::PreparedCallGraph(
                crate::callable::CallConstraintInvariant::PreparedGraphConsumed.into(),
            )
        })
    }

    pub(super) fn take_prepared_calls(
        &mut self,
    ) -> Result<AnalyzerPreparedCallGraph, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if !self.candidate_checkpoints.is_empty()
            || !self.candidate_journal.is_empty()
            || !self.expression_stack.is_empty()
            || !self.active_physical_call_attempts.is_empty()
        {
            return Err(CandidateFactTransactionViolation::UnrecoverableLedger);
        }
        self.validate_call_expression_graph()?;
        self.prepared_calls()?
            .validate_seal_ready()
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })?;
        self.prepared_calls.take().ok_or_else(|| {
            CandidateFactTransactionViolation::PreparedCallGraph(
                crate::callable::CallConstraintInvariant::PreparedGraphConsumed.into(),
            )
        })
    }

    fn prepared_calls_mut(
        &mut self,
    ) -> Result<&mut AnalyzerPreparedCallGraph, CandidateFactTransactionViolation> {
        self.prepared_calls.as_mut().ok_or_else(|| {
            CandidateFactTransactionViolation::PreparedCallGraph(
                crate::callable::CallConstraintInvariant::PreparedGraphConsumed.into(),
            )
        })
    }

    pub(super) fn seal_selected_application(
        &mut self,
        target: &CandidateFactTransactionAuthority<'_>,
        site: crate::callable::CheckedCallSite,
        prefix: AnalyzerPreparedCallPrefix,
    ) -> Result<(TypeKind, Option<PreparedCallContinuationRef>), CandidateFactTransactionViolation>
    {
        self.ensure_healthy()?;
        if !Arc::ptr_eq(&self.issuer, &target.checkpoint.issuer) {
            return Err(CandidateFactTransactionViolation::ForeignCheckpoint);
        }
        if target.checkpoint.epoch != self.epoch {
            return Err(CandidateFactTransactionViolation::StaleCheckpoint);
        }
        if !self.candidate_checkpoints.last().is_some_and(|active| {
            active.id == target.checkpoint.id
                && active.journal_start == target.checkpoint.journal_start
        }) {
            return Err(CandidateFactTransactionViolation::NonLifoCheckpoint);
        }
        self.prepared_calls()?
            .validate_checkpoint(&target.checkpoint.graph)
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })?;
        self.prepared_calls_mut()?
            .seal_selected_application(site, prefix)
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })
    }

    pub(super) fn insert_unselected_call(
        &mut self,
        target: &CandidateFactTransactionAuthority<'_>,
        site: crate::callable::CheckedCallSite,
        value: AnalyzerPreparedUnselectedCall,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if !Arc::ptr_eq(&self.issuer, &target.checkpoint.issuer) {
            return Err(CandidateFactTransactionViolation::ForeignCheckpoint);
        }
        if target.checkpoint.epoch != self.epoch {
            return Err(CandidateFactTransactionViolation::StaleCheckpoint);
        }
        if !self.candidate_checkpoints.last().is_some_and(|active| {
            active.id == target.checkpoint.id
                && active.journal_start == target.checkpoint.journal_start
        }) {
            return Err(CandidateFactTransactionViolation::NonLifoCheckpoint);
        }
        self.prepared_calls()?
            .validate_checkpoint(&target.checkpoint.graph)
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })?;
        let dependencies = value.dependencies();
        self.prepared_calls_mut()?
            .seal_unselected(site, dependencies, value)
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })
    }

    pub(super) const fn patterns(&self) -> &BTreeMap<PatternId, TypeKind> {
        &self.patterns
    }

    pub(super) const fn expressions(&self) -> &BTreeMap<ExprId, PreparedExpressionFact> {
        &self.expressions
    }

    pub(super) const fn iteration_facts(&self) -> &BTreeMap<ExprId, CheckedIteration> {
        &self.iteration_facts
    }

    /// Returns the currently pending use rows without creating a second
    /// capture inventory. Each implicit callable seal consumes its own rows;
    /// publication requires this map to be empty.
    pub(super) const fn pending_implicit_capture_uses(
        &self,
    ) -> &BTreeMap<ImplicitCaptureUseKey, LocalId> {
        &self.implicit_capture_uses
    }

    #[cfg(test)]
    pub(super) fn implicit_capture_use(
        &self,
        callable: ExprId,
        expression: ExprId,
    ) -> Option<LocalId> {
        self.implicit_capture_uses
            .get(&(callable, expression))
            .copied()
    }

    /// Journal-removes and returns every pending use owned by one implicit
    /// callable. The checked callable fact immediately seals these rows into
    /// topology-authenticated evidence.
    pub(super) fn take_implicit_capture_uses(
        &mut self,
        callable: ExprId,
    ) -> Result<Box<[(ExprId, LocalId)]>, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        let rows = self
            .implicit_capture_uses
            .iter()
            .filter_map(|((owner, expression), local)| {
                (*owner == callable).then_some((*expression, *local))
            })
            .collect::<Vec<_>>();
        for (expression, _) in &rows {
            self.remove_implicit_capture_use((callable, *expression));
        }
        Ok(rows.into_boxed_slice())
    }

    pub(super) const fn calls(&self) -> &BTreeMap<ExprId, CallTargetFacts> {
        &self.final_calls
    }

    pub(super) const fn physical_candidate_argument_evaluations(
        &self,
    ) -> &BTreeMap<ExprId, Vec<PhysicalCandidateArgumentEvaluation>> {
        &self.physical_candidate_argument_evaluations.rows
    }

    pub(super) fn begin_physical_call_attempt(
        &mut self,
        attempt: PhysicalCallAttemptId,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        match self.active_physical_call_attempts.last() {
            Some(parent) if attempt.is_direct_child_of(&parent.attempt) => {}
            Some(_) => return Err(CandidateFactTransactionViolation::PhysicalCallAttemptOrder),
            None if attempt.is_root() => {}
            None => {
                return Err(CandidateFactTransactionViolation::PhysicalCallAttemptRootMismatch);
            }
        }
        self.active_physical_call_attempts
            .push(ActivePhysicalCallAttempt {
                attempt,
                transcript: PhysicalCandidateEvaluationTranscript::default(),
            });
        Ok(())
    }

    pub(super) fn close_physical_call_attempt(
        &mut self,
        attempt: &PhysicalCallAttemptId,
        close: PhysicalCallAttemptClose,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if self
            .active_physical_call_attempts
            .last()
            .is_none_or(|active| !active.attempt.same_issued_attempt(attempt))
        {
            return Err(CandidateFactTransactionViolation::PhysicalCallAttemptOrder);
        }
        let ActivePhysicalCallAttempt {
            attempt: _,
            transcript,
        } = self
            .active_physical_call_attempts
            .pop()
            .expect("the active attempt was validated above");
        match close {
            PhysicalCallAttemptClose::Failed => {}
            PhysicalCallAttemptClose::Completed => {
                if let Some(parent) = self.active_physical_call_attempts.last_mut() {
                    parent.transcript.merge(transcript);
                } else if let Some(candidate) =
                    self.active_physical_candidate_argument_evaluations.as_mut()
                {
                    candidate.merge(transcript);
                } else {
                    self.physical_candidate_argument_evaluations
                        .merge(transcript);
                }
            }
            PhysicalCallAttemptClose::Cancelled => {
                if let Some(parent) = self.active_physical_call_attempts.last_mut() {
                    parent.transcript.merge(transcript);
                } else {
                    // Cancellation exposes a typed operational prefix even
                    // though no semantic candidate transaction can commit.
                    self.physical_candidate_argument_evaluations
                        .merge(transcript);
                }
            }
        }
        Ok(())
    }

    pub(super) fn record_physical_candidate_argument_evaluation(
        &mut self,
        evaluation: PhysicalCandidateArgumentEvaluation,
        limit: u64,
    ) -> Result<PhysicalCandidateEvaluationAdmission, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        let active = self
            .active_physical_call_attempts
            .last()
            .ok_or(CandidateFactTransactionViolation::PhysicalCallAttemptRootMismatch)?;
        if !active.attempt.same_issued_attempt(evaluation.attempt()) {
            return Err(CandidateFactTransactionViolation::PhysicalCallAttemptMismatch);
        }
        let root = evaluation.attempt().root();
        if self
            .physical_candidate_argument_evaluations
            .matching_row(root, &evaluation)
            .is_some()
            || self
                .active_physical_candidate_argument_evaluations
                .as_ref()
                .and_then(|candidate| candidate.matching_row(root, &evaluation))
                .is_some()
            || self
                .active_physical_call_attempts
                .iter()
                .any(|attempt| attempt.transcript.matching_row(root, &evaluation).is_some())
        {
            return Ok(PhysicalCandidateEvaluationAdmission::Duplicate);
        }
        let observed = self
            .physical_candidate_argument_evaluations
            .root_row_count(root)
            .checked_add(
                self.active_physical_candidate_argument_evaluations
                    .as_ref()
                    .map_or(0, |candidate| candidate.root_row_count(root)),
            )
            .and_then(|count| {
                self.active_physical_call_attempts
                    .iter()
                    .try_fold(count, |count, attempt| {
                        count.checked_add(attempt.transcript.root_row_count(root))
                    })
            })
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(CandidateFactTransactionViolation::UnrecoverableLedger)?;
        if observed >= limit {
            return Ok(PhysicalCandidateEvaluationAdmission::LimitReached);
        }
        self.active_physical_call_attempts
            .last_mut()
            .expect("the active call attempt was validated above")
            .transcript
            .rows
            .entry(root)
            .or_default()
            .push(evaluation);
        Ok(PhysicalCandidateEvaluationAdmission::Recorded)
    }

    pub(super) fn begin_expression(
        &mut self,
        owner: ExprId,
    ) -> Result<bool, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        Ok(self.expression_stack.insert(owner))
    }

    pub(super) fn end_expression(&mut self, owner: ExprId) {
        self.expression_stack.remove(&owner);
    }

    /// Starts one candidate transaction and mints its move-only checkpoint.
    fn begin_candidate_transaction(
        &mut self,
    ) -> Result<CandidateFactCheckpoint, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        let opens_root = self.candidate_checkpoints.is_empty();
        if opens_root
            != self
                .active_physical_candidate_argument_evaluations
                .is_none()
        {
            return Err(CandidateFactTransactionViolation::UnrecoverableLedger);
        }
        let id = CandidateFactCheckpointId(self.next_checkpoint_id);
        self.next_checkpoint_id = self
            .next_checkpoint_id
            .checked_add(1)
            .ok_or(CandidateFactTransactionViolation::SequenceExhausted)?;
        let graph = self
            .prepared_calls_mut()?
            .begin_delta()
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })?;
        let checkpoint = CandidateFactCheckpoint {
            issuer: Arc::clone(&self.issuer),
            id,
            journal_start: CandidateFactJournalCursor(self.candidate_journal.len()),
            epoch: self.epoch,
            graph,
        };
        if opens_root {
            self.active_physical_candidate_argument_evaluations =
                Some(PhysicalCandidateEvaluationTranscript::default());
        }
        self.candidate_checkpoints.push(ActiveCandidateCheckpoint {
            id,
            journal_start: checkpoint.journal_start,
        });
        Ok(checkpoint)
    }

    fn commit_candidate_transaction(
        &mut self,
        checkpoint: CandidateFactCheckpoint,
    ) -> Result<(), CandidateFactCloseFailure> {
        if let Err(violation) = self.validate_checkpoint(&checkpoint) {
            return Err(CandidateFactCloseFailure {
                violation,
                checkpoint,
            });
        }
        let CandidateFactCheckpoint {
            issuer,
            id,
            journal_start,
            epoch,
            graph,
        } = checkpoint;
        if let Err(failure) = self
            .prepared_calls
            .as_mut()
            .expect("checkpoint validation proves the graph is live")
            .commit_delta(graph)
        {
            let (violation, graph) = failure.into_parts();
            return Err(CandidateFactCloseFailure {
                violation: CandidateFactTransactionViolation::PreparedCallGraph(violation.into()),
                checkpoint: CandidateFactCheckpoint {
                    issuer,
                    id,
                    journal_start,
                    epoch,
                    graph,
                },
            });
        }
        self.candidate_checkpoints.pop();
        if self.candidate_checkpoints.is_empty() {
            let physical = self
                .active_physical_candidate_argument_evaluations
                .take()
                .expect("checkpoint validation proves the root physical transcript is live");
            if let Some(attempt) = self.active_physical_call_attempts.last_mut() {
                attempt.transcript.merge(physical);
            } else {
                self.physical_candidate_argument_evaluations.merge(physical);
            }
            self.candidate_journal.clear();
        }
        Ok(())
    }

    fn rollback_candidate_transaction(
        &mut self,
        checkpoint: CandidateFactCheckpoint,
    ) -> Result<(), CandidateFactCloseFailure> {
        if let Err(violation) = self.validate_checkpoint(&checkpoint) {
            return Err(CandidateFactCloseFailure {
                violation,
                checkpoint,
            });
        }
        let CandidateFactCheckpoint {
            issuer,
            id,
            journal_start,
            epoch,
            graph,
        } = checkpoint;
        if let Err(failure) = self
            .prepared_calls
            .as_mut()
            .expect("checkpoint validation proves the graph is live")
            .rollback_delta(graph)
        {
            let (violation, graph) = failure.into_parts();
            return Err(CandidateFactCloseFailure {
                violation: CandidateFactTransactionViolation::PreparedCallGraph(violation.into()),
                checkpoint: CandidateFactCheckpoint {
                    issuer,
                    id,
                    journal_start,
                    epoch,
                    graph,
                },
            });
        }
        self.candidate_checkpoints.pop();
        self.rollback_journal(journal_start);
        if self.candidate_checkpoints.is_empty() {
            self.active_physical_candidate_argument_evaluations = None;
            self.candidate_journal.clear();
        }
        Ok(())
    }

    /// Extracts the final values touched by a candidate and rolls that
    /// candidate back in one move-only operation.
    fn extract_and_rollback(
        &mut self,
        checkpoint: CandidateFactCheckpoint,
    ) -> Result<CandidateSemanticProjection, CandidateFactCloseFailure> {
        if let Err(violation) = self.validate_checkpoint(&checkpoint) {
            return Err(CandidateFactCloseFailure {
                violation,
                checkpoint,
            });
        }
        let CandidateFactCheckpoint {
            issuer,
            id,
            journal_start,
            epoch,
            graph,
        } = checkpoint;
        let graph_delta = match self
            .prepared_calls
            .as_mut()
            .expect("checkpoint validation proves the graph is live")
            .extract_delta(graph)
        {
            Ok(delta) => delta,
            Err(failure) => {
                let (violation, graph) = failure.into_parts();
                return Err(CandidateFactCloseFailure {
                    violation: CandidateFactTransactionViolation::PreparedCallGraph(
                        violation.into(),
                    ),
                    checkpoint: CandidateFactCheckpoint {
                        issuer,
                        id,
                        journal_start,
                        epoch,
                        graph,
                    },
                });
            }
        };
        self.candidate_checkpoints.pop();
        let physical_candidate_argument_evaluations = if self.candidate_checkpoints.is_empty() {
            self.active_physical_candidate_argument_evaluations
                .take()
                .expect("checkpoint validation proves the root physical transcript is live")
        } else {
            PhysicalCandidateEvaluationTranscript::default()
        };
        let owners = self.projection_owners(journal_start);
        let projection = CandidateSemanticProjection {
            authority: CandidateProjectionAuthority {
                issuer: Arc::clone(&self.issuer),
                epoch: self.epoch,
            },
            graph_delta,
            locals: owners
                .locals
                .into_iter()
                .map(|owner| (owner, self.locals.remove(&owner)))
                .collect(),
            patterns: owners
                .patterns
                .into_iter()
                .map(|owner| (owner, self.patterns.remove(&owner)))
                .collect(),
            expressions: owners
                .expressions
                .into_iter()
                .map(|owner| (owner, self.expressions.remove(&owner)))
                .collect(),
            iterations: owners
                .iterations
                .into_iter()
                .map(|owner| (owner, self.iteration_facts.remove(&owner)))
                .collect(),
            implicit_capture_uses: owners
                .implicit_capture_uses
                .into_iter()
                .map(|key| (key, self.implicit_capture_uses.remove(&key)))
                .collect(),
            physical_candidate_argument_evaluations,
        };
        self.rollback_journal(journal_start);
        if self.candidate_checkpoints.is_empty() {
            self.candidate_journal.clear();
        }
        Ok(projection)
    }

    /// Applies a probe projection while another candidate transaction is
    /// active.  Each transferred value is itself journaled, so an enclosing
    /// rollback also undoes the application.
    pub(super) fn apply_candidate_projection(
        &mut self,
        target: &CandidateFactTransactionAuthority<'_>,
        projection: CandidateSemanticProjection,
    ) -> Result<(), CandidateProjectionApplyFailure> {
        let violation = if let Err(error) = self.ensure_healthy() {
            Some(error)
        } else if !Arc::ptr_eq(&self.issuer, &target.checkpoint.issuer) {
            Some(CandidateFactTransactionViolation::ForeignCheckpoint)
        } else if target.checkpoint.epoch != self.epoch {
            Some(CandidateFactTransactionViolation::StaleCheckpoint)
        } else if target.checkpoint.journal_start.0 > self.candidate_journal.len() {
            Some(CandidateFactTransactionViolation::JournalCursorMismatch)
        } else if !self.candidate_checkpoints.last().is_some_and(|active| {
            active.id == target.checkpoint.id
                && active.journal_start == target.checkpoint.journal_start
        }) {
            Some(CandidateFactTransactionViolation::NonLifoCheckpoint)
        } else if !Arc::ptr_eq(&self.issuer, &projection.authority.issuer) {
            Some(CandidateFactTransactionViolation::ForeignProjection)
        } else if projection.authority.epoch != target.checkpoint.epoch {
            Some(CandidateFactTransactionViolation::ProjectionAuthorityMismatch)
        } else if self.prepared_calls.is_none() {
            Some(CandidateFactTransactionViolation::PreparedCallGraph(
                crate::callable::CallConstraintInvariant::PreparedGraphConsumed.into(),
            ))
        } else {
            None
        };
        if let Some(violation) = violation {
            return Err(CandidateProjectionApplyFailure {
                violation,
                projection,
            });
        }
        if let Err(violation) =
            self.preflight_projection_call_graph(&projection.graph_delta, &projection.expressions)
        {
            return Err(CandidateProjectionApplyFailure {
                violation,
                projection,
            });
        }
        let CandidateSemanticProjection {
            authority,
            graph_delta,
            locals,
            patterns,
            expressions,
            iterations,
            implicit_capture_uses,
            physical_candidate_argument_evaluations,
        } = projection;
        if let Some((key, existing, proposed)) =
            implicit_capture_uses.iter().find_map(|(key, value)| {
                let local = (*value)?;
                let existing = self
                    .implicit_capture_uses
                    .get(key)
                    .copied()
                    .filter(|existing| *existing != local)?;
                Some((*key, existing, local))
            })
        {
            return Err(CandidateProjectionApplyFailure {
                violation: CandidateFactTransactionViolation::ImplicitCaptureUseConflict {
                    callable: key.0,
                    expression: key.1,
                    existing,
                    proposed,
                },
                projection: CandidateSemanticProjection {
                    authority,
                    graph_delta,
                    locals,
                    patterns,
                    expressions,
                    iterations,
                    implicit_capture_uses,
                    physical_candidate_argument_evaluations,
                },
            });
        }
        match self
            .prepared_calls
            .as_mut()
            .expect("prepared graph presence was validated above")
            .restore_delta(&target.checkpoint.graph, graph_delta)
        {
            Ok(()) => {}
            Err(failure) => {
                let (violation, graph_delta) = failure.into_parts();
                return Err(CandidateProjectionApplyFailure {
                    violation: CandidateFactTransactionViolation::PreparedCallGraph(
                        violation.into(),
                    ),
                    projection: CandidateSemanticProjection {
                        authority,
                        graph_delta,
                        locals,
                        patterns,
                        expressions,
                        iterations,
                        implicit_capture_uses,
                        physical_candidate_argument_evaluations,
                    },
                });
            }
        }
        self.active_physical_candidate_argument_evaluations
            .as_mut()
            .expect("projection target validation proves the physical transcript is live")
            .merge(physical_candidate_argument_evaluations);
        self.apply_projection_entries(
            locals,
            patterns,
            expressions,
            iterations,
            implicit_capture_uses,
        );
        Ok(())
    }

    /// Explicitly consumes a projection that cannot be published (for
    /// example, both postfix candidates succeeded).  The issuer and epoch
    /// are still checked so a foreign or stale projection cannot be silently
    /// discarded as if it belonged to this fact ledger.
    pub(super) fn discard_candidate_projection(
        &self,
        projection: CandidateSemanticProjection,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if !Arc::ptr_eq(&self.issuer, &projection.authority.issuer) {
            return Err(CandidateFactTransactionViolation::ForeignProjection);
        }
        if projection.authority.epoch != self.epoch {
            return Err(CandidateFactTransactionViolation::ProjectionAuthorityMismatch);
        }
        self.prepared_calls()?
            .validate_delta(&projection.graph_delta)
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })?;
        drop(projection);
        Ok(())
    }

    pub(super) fn set_local_type(
        &mut self,
        owner: LocalId,
        value: TypeKind,
    ) -> Result<bool, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        Ok(self.set_local_type_unchecked(owner, value))
    }

    fn set_local_type_unchecked(&mut self, owner: LocalId, value: TypeKind) -> bool {
        let previous = self.locals.insert(owner, value);
        let replaced = previous.is_some();
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal.push(SemanticFactMutation::Local {
                owner,
                previous: previous.map(Box::new),
            });
        }
        replaced
    }

    pub(super) fn set_pattern_type(
        &mut self,
        owner: PatternId,
        value: TypeKind,
    ) -> Result<bool, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        Ok(self.set_pattern_type_unchecked(owner, value))
    }

    fn set_pattern_type_unchecked(&mut self, owner: PatternId, value: TypeKind) -> bool {
        let previous = self.patterns.insert(owner, value);
        let replaced = previous.is_some();
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal.push(SemanticFactMutation::Pattern {
                owner,
                previous: previous.map(Box::new),
            });
        }
        replaced
    }

    pub(super) fn set_iteration_fact(
        &mut self,
        owner: ExprId,
        value: CheckedIteration,
    ) -> Result<bool, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        Ok(self.set_iteration_fact_unchecked(owner, value))
    }

    fn set_iteration_fact_unchecked(&mut self, owner: ExprId, value: CheckedIteration) -> bool {
        let previous = self.iteration_facts.insert(owner, value);
        let replaced = previous.is_some();
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::Iteration {
                    owner,
                    previous: previous.map(Box::new),
                });
        }
        replaced
    }

    /// Records one resolved implicit-capture use in the active fact ledger.
    /// Repeating the exact row is idempotent.  A key resolving to a different
    /// local is an invariant failure, not a candidate mismatch: allowing the
    /// probe to continue would make capture ownership depend on retry order.
    pub(super) fn record_implicit_capture_use(
        &mut self,
        callable: ExprId,
        expression: ExprId,
        local: LocalId,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        let key = (callable, expression);
        if let Some(existing) = self.implicit_capture_uses.get(&key) {
            if *existing == local {
                return Ok(());
            }
            return Err(
                CandidateFactTransactionViolation::ImplicitCaptureUseConflict {
                    callable,
                    expression,
                    existing: *existing,
                    proposed: local,
                },
            );
        }
        let previous = self.implicit_capture_uses.insert(key, local);
        debug_assert!(previous.is_none());
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::ImplicitCaptureUse { key, previous });
        }
        Ok(())
    }
}

impl SemanticFactState {
    fn call_graph_mismatch() -> CandidateFactTransactionViolation {
        CandidateFactTransactionViolation::PreparedCallGraph(
            crate::callable::CallConstraintInvariant::PreparedCallSiteMismatch.into(),
        )
    }

    fn validate_call_expression_write(
        &self,
        owner: ExprId,
        value: &PreparedExpressionFact,
    ) -> Result<(), CandidateFactTransactionViolation> {
        let Some(site) = value.checked_call_site(owner) else {
            return Ok(());
        };
        if let Some(graph) = self.prepared_calls.as_ref() {
            return match graph.site_state(site) {
                Some(
                    PreparedCallGraphSiteState::Selected | PreparedCallGraphSiteState::Unselected,
                ) => Ok(()),
                None => Err(Self::call_graph_mismatch()),
            };
        }
        // The consuming C sealer may refine the type/effects/resolution of an
        // already correlated call expression after taking the prepared graph.
        // It cannot create a new call-backed expression or change its site
        // family after that affine boundary.
        self.expressions
            .get(&owner)
            .and_then(|existing| existing.checked_call_site(owner))
            .filter(|existing| *existing == site)
            .map(|_| ())
            .ok_or_else(Self::call_graph_mismatch)
    }

    fn validate_call_expression_graph(&self) -> Result<(), CandidateFactTransactionViolation> {
        let graph = self.prepared_calls()?;
        if self.expressions.iter().any(|(owner, expression)| {
            expression
                .checked_call_site(*owner)
                .is_some_and(|site| graph.site_state(site).is_none())
        }) || graph.sites().any(|site| {
            let owner = match site {
                CheckedCallSite::HirCall(owner) | CheckedCallSite::DialogueApplication(owner) => {
                    owner
                }
            };
            self.expressions
                .get(&owner)
                .and_then(|expression| expression.checked_call_site(owner))
                != Some(site)
        }) {
            return Err(Self::call_graph_mismatch());
        }
        Ok(())
    }

    fn preflight_projection_call_graph(
        &self,
        delta: &PreparedCallGraphDelta<AnalyzerPreparedCallPrefix, AnalyzerPreparedUnselectedCall>,
        projected_expressions: &BTreeMap<ExprId, Option<PreparedExpressionFact>>,
    ) -> Result<(), CandidateFactTransactionViolation> {
        let graph = self.prepared_calls()?;
        let delta_sites = delta.site_states().map_err(|violation| {
            CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
        })?;
        let graph_sites = graph.sites().collect::<BTreeSet<_>>();
        let expression_sites = self
            .expressions
            .iter()
            .filter_map(|(owner, expression)| {
                expression
                    .checked_call_site(*owner)
                    .map(|site| (*owner, site))
            })
            .collect::<BTreeMap<_, _>>();
        let expression_updates = projected_expressions
            .iter()
            .map(|(owner, expression)| {
                (
                    *owner,
                    expression
                        .as_ref()
                        .and_then(|expression| expression.checked_call_site(*owner)),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if projected_call_site_inventories_match(
            graph_sites,
            delta_sites.iter().map(|(site, _state)| *site),
            expression_sites,
            expression_updates,
        ) {
            Ok(())
        } else {
            Err(Self::call_graph_mismatch())
        }
    }

    pub(super) fn publish_new_expression(
        &mut self,
        owner: ExprId,
        value: impl Into<PreparedExpressionFact>,
    ) -> Result<(), ExpressionFactWriteViolation> {
        let value = value.into();
        self.ensure_healthy()
            .map_err(ExpressionFactWriteViolation::Candidate)?;
        if self.expressions.contains_key(&owner) {
            return Err(ExpressionFactWriteViolation::AlreadyPublished);
        }
        self.validate_call_expression_write(owner, &value)
            .map_err(ExpressionFactWriteViolation::Candidate)?;
        self.write_expression(owner, value);
        Ok(())
    }

    pub(super) fn replace_contextual_expression(
        &mut self,
        owner: ExprId,
        value: impl Into<PreparedExpressionFact>,
    ) -> Result<(), ExpressionFactWriteViolation> {
        self.ensure_healthy()
            .map_err(ExpressionFactWriteViolation::Candidate)?;
        self.replace_existing_expression(owner, value)
    }

    pub(super) fn replace_existing_expression(
        &mut self,
        owner: ExprId,
        value: impl Into<PreparedExpressionFact>,
    ) -> Result<(), ExpressionFactWriteViolation> {
        let value = value.into();
        self.ensure_healthy()
            .map_err(ExpressionFactWriteViolation::Candidate)?;
        if !self.expressions.contains_key(&owner) {
            return Err(ExpressionFactWriteViolation::MissingPublishedFact);
        }
        self.validate_call_expression_write(owner, &value)
            .map_err(ExpressionFactWriteViolation::Candidate)?;
        self.write_expression(owner, value);
        Ok(())
    }

    fn write_expression(&mut self, owner: ExprId, value: PreparedExpressionFact) {
        let previous = self.expressions.insert(owner, value);
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::Expression {
                    owner,
                    previous: previous.map(Box::new),
                });
        }
    }

    fn remove_expression(&mut self, owner: ExprId) {
        let previous = self.expressions.remove(&owner);
        if previous.is_some() && !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::Expression {
                    owner,
                    previous: previous.map(Box::new),
                });
        }
    }

    pub(super) fn publish_final_call_fact(
        &mut self,
        owner: ExprId,
        value: CallTargetFacts,
    ) -> Result<bool, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if self.prepared_calls.is_some()
            || !self.candidate_checkpoints.is_empty()
            || !self.candidate_journal.is_empty()
            || !self.active_physical_call_attempts.is_empty()
            || value.expression() != owner
        {
            return Err(CandidateFactTransactionViolation::UnrecoverableLedger);
        }
        Ok(self.final_calls.insert(owner, value).is_some())
    }

    fn remove_implicit_capture_use(&mut self, key: ImplicitCaptureUseKey) {
        let previous = self.implicit_capture_uses.remove(&key);
        if previous.is_some() && !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::ImplicitCaptureUse { key, previous });
        }
    }

    fn validate_checkpoint(
        &self,
        checkpoint: &CandidateFactCheckpoint,
    ) -> Result<(), CandidateFactTransactionViolation> {
        if let Some(cause) = self.poison.clone() {
            return Err(cause);
        }
        if !Arc::ptr_eq(&self.issuer, &checkpoint.issuer) {
            return Err(CandidateFactTransactionViolation::ForeignCheckpoint);
        }
        match self.candidate_checkpoints.last().copied() {
            None => Err(CandidateFactTransactionViolation::StaleCheckpoint),
            Some(active) if active.id == checkpoint.id => {
                if active.journal_start != checkpoint.journal_start
                    || checkpoint.epoch != self.epoch
                    || checkpoint.journal_start.0 > self.candidate_journal.len()
                {
                    Err(CandidateFactTransactionViolation::JournalCursorMismatch)
                } else {
                    Ok(())
                }
            }
            Some(_)
                if self
                    .candidate_checkpoints
                    .iter()
                    .any(|active| active.id == checkpoint.id) =>
            {
                Err(CandidateFactTransactionViolation::NonLifoCheckpoint)
            }
            Some(_) => Err(CandidateFactTransactionViolation::StaleCheckpoint),
        }?;
        if self
            .active_physical_candidate_argument_evaluations
            .is_none()
        {
            return Err(CandidateFactTransactionViolation::UnrecoverableLedger);
        }
        self.prepared_calls()?
            .validate_checkpoint(&checkpoint.graph)
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })
    }

    /// A close failure is converted to a poisoned ledger only by the analyzer
    /// transaction runner, after it has retained the move-only checkpoint.
    fn abort_after_close_failure(
        &mut self,
        failure: CandidateFactCloseFailure,
    ) -> CandidateFactTransactionViolation {
        if let Some(cause) = self.poison.clone() {
            let (_, checkpoint) = failure.into_parts();
            let CandidateFactCheckpoint { graph, .. } = checkpoint;
            if let Some(prepared_calls) = self.prepared_calls.as_mut() {
                let _ = prepared_calls.abort_after_close_failure(graph);
            }
            return cause;
        }
        let (violation, checkpoint) = failure.into_parts();
        let CandidateFactCheckpoint { graph, .. } = checkpoint;
        let violation = match self.prepared_calls.as_mut() {
            Some(prepared_calls) => match prepared_calls.abort_after_close_failure(graph) {
                Ok(()) => violation,
                Err(graph_violation) => {
                    CandidateFactTransactionViolation::PreparedCallGraph(graph_violation.into())
                }
            },
            None => CandidateFactTransactionViolation::PreparedCallGraph(
                crate::callable::CallConstraintInvariant::PreparedGraphConsumed.into(),
            ),
        };
        let oldest = self
            .candidate_checkpoints
            .first()
            .map(|active| active.journal_start);
        let recoverable = oldest.is_none_or(|cursor| cursor.0 <= self.candidate_journal.len());
        if let Some(cursor) = oldest.filter(|_| recoverable) {
            self.rollback_journal(cursor);
        }
        self.candidate_checkpoints.clear();
        self.candidate_journal.clear();
        self.active_physical_candidate_argument_evaluations = None;
        let cause = if recoverable {
            violation
        } else {
            CandidateFactTransactionViolation::UnrecoverableLedger
        };
        let Some(next_epoch) = self.epoch.checked_add(1) else {
            self.poison = Some(CandidateFactTransactionViolation::UnrecoverableLedger);
            return CandidateFactTransactionViolation::UnrecoverableLedger;
        };
        self.epoch = next_epoch;
        self.poison = Some(cause.clone());
        cause
    }

    fn projection_owners(
        &self,
        journal_start: CandidateFactJournalCursor,
    ) -> CandidateProjectionOwners {
        let mut owners = CandidateProjectionOwners::default();
        for mutation in &self.candidate_journal[journal_start.0..] {
            match mutation {
                SemanticFactMutation::Local { owner, .. } => {
                    owners.locals.insert(*owner);
                }
                SemanticFactMutation::Pattern { owner, .. } => {
                    owners.patterns.insert(*owner);
                }
                SemanticFactMutation::Expression { owner, .. } => {
                    owners.expressions.insert(*owner);
                }
                SemanticFactMutation::Iteration { owner, .. } => {
                    owners.iterations.insert(*owner);
                }
                SemanticFactMutation::ImplicitCaptureUse { key, .. } => {
                    owners.implicit_capture_uses.insert(*key);
                }
            }
        }
        owners
    }

    fn rollback_journal(&mut self, journal_start: CandidateFactJournalCursor) {
        let mut mutations = self.candidate_journal.split_off(journal_start.0);
        while let Some(mutation) = mutations.pop() {
            match mutation {
                SemanticFactMutation::Local { owner, previous } => {
                    restore_map_entry(&mut self.locals, owner, previous.map(|previous| *previous));
                }
                SemanticFactMutation::Pattern { owner, previous } => {
                    restore_map_entry(
                        &mut self.patterns,
                        owner,
                        previous.map(|previous| *previous),
                    );
                }
                SemanticFactMutation::Expression { owner, previous } => {
                    restore_map_entry(
                        &mut self.expressions,
                        owner,
                        previous.map(|previous| *previous),
                    );
                }
                SemanticFactMutation::Iteration { owner, previous } => {
                    restore_map_entry(
                        &mut self.iteration_facts,
                        owner,
                        previous.map(|previous| *previous),
                    );
                }
                SemanticFactMutation::ImplicitCaptureUse { key, previous } => {
                    restore_map_entry(&mut self.implicit_capture_uses, key, previous);
                }
            }
        }
    }

    fn apply_projection_entries(
        &mut self,
        locals: BTreeMap<LocalId, Option<TypeKind>>,
        patterns: BTreeMap<PatternId, Option<TypeKind>>,
        expressions: BTreeMap<ExprId, Option<PreparedExpressionFact>>,
        iterations: BTreeMap<ExprId, Option<CheckedIteration>>,
        implicit_capture_uses: BTreeMap<ImplicitCaptureUseKey, Option<LocalId>>,
    ) {
        for (owner, value) in locals {
            self.apply_local(owner, value);
        }
        for (owner, value) in patterns {
            self.apply_pattern(owner, value);
        }
        for (owner, value) in expressions {
            self.apply_expression(owner, value);
        }
        for (owner, value) in iterations {
            self.apply_iteration(owner, value);
        }
        for (key, value) in implicit_capture_uses {
            self.apply_implicit_capture_use(key, value);
        }
    }

    fn apply_local(&mut self, owner: LocalId, value: Option<TypeKind>) {
        if let Some(value) = value {
            self.set_local_type_unchecked(owner, value);
        } else {
            self.remove_local_type(owner);
        }
    }

    fn apply_pattern(&mut self, owner: PatternId, value: Option<TypeKind>) {
        if let Some(value) = value {
            self.set_pattern_type_unchecked(owner, value);
        } else {
            self.remove_pattern_type(owner);
        }
    }

    fn apply_expression(&mut self, owner: ExprId, value: Option<PreparedExpressionFact>) {
        if let Some(value) = value {
            self.write_expression(owner, value);
        } else {
            self.remove_expression(owner);
        }
    }

    fn apply_iteration(&mut self, owner: ExprId, value: Option<CheckedIteration>) {
        if let Some(value) = value {
            self.set_iteration_fact_unchecked(owner, value);
        } else {
            self.remove_iteration_fact(owner);
        }
    }

    fn apply_implicit_capture_use(&mut self, key: ImplicitCaptureUseKey, value: Option<LocalId>) {
        if let Some(value) = value {
            let already_present = self
                .implicit_capture_uses
                .get(&key)
                .is_some_and(|existing| *existing == value);
            if !already_present {
                let previous = self.implicit_capture_uses.insert(key, value);
                if !self.candidate_checkpoints.is_empty() {
                    self.candidate_journal
                        .push(SemanticFactMutation::ImplicitCaptureUse { key, previous });
                }
            }
        } else {
            self.remove_implicit_capture_use(key);
        }
    }

    fn remove_local_type(&mut self, owner: LocalId) {
        let previous = self.locals.remove(&owner);
        if previous.is_some() && !self.candidate_checkpoints.is_empty() {
            self.candidate_journal.push(SemanticFactMutation::Local {
                owner,
                previous: previous.map(Box::new),
            });
        }
    }

    fn remove_pattern_type(&mut self, owner: PatternId) {
        let previous = self.patterns.remove(&owner);
        if previous.is_some() && !self.candidate_checkpoints.is_empty() {
            self.candidate_journal.push(SemanticFactMutation::Pattern {
                owner,
                previous: previous.map(Box::new),
            });
        }
    }

    fn remove_iteration_fact(&mut self, owner: ExprId) {
        let previous = self.iteration_facts.remove(&owner);
        if previous.is_some() && !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::Iteration {
                    owner,
                    previous: previous.map(Box::new),
                });
        }
    }
}

impl<'project, 'catalog, 'control> super::Analyzer<'project, 'catalog, 'control> {
    /// Sole owner of candidate checkpoint lifecycle.  Raw state operations stay
    /// private to this module so a callback cannot close a checkpoint without
    /// the trusted abort/recovery path.
    pub(super) fn run_candidate_fact_transaction<T, E>(
        &mut self,
        operation: impl for<'a> FnOnce(
            &'a mut Self,
            CandidateExpressionFactAuthority<'a>,
            CandidateFactTransactionAuthority<'a>,
        ) -> Result<CandidateFactTransactionAction<T>, E>,
    ) -> Result<CandidateFactTransactionOutcome<T>, super::expression_error::AnalyzerExpressionError>
    where
        E: Into<CandidateFactOperationFailure>,
    {
        use super::expression_error::AnalyzerExpressionError;

        let checkpoint = self
            .facts
            .begin_candidate_transaction()
            .map_err(AnalyzerExpressionError::fact)?;
        let expression_authority = match self.facts.candidate_expression_authority(&checkpoint) {
            Ok(authority) => authority,
            Err(error) => {
                return match self.facts.rollback_candidate_transaction(checkpoint) {
                    Ok(()) => Err(AnalyzerExpressionError::fact(error)),
                    Err(failure) => Err(AnalyzerExpressionError::fact(
                        self.facts.abort_after_close_failure(failure),
                    )),
                };
            }
        };
        let transaction_authority = match self.facts.transaction_authority(&checkpoint) {
            Ok(authority) => authority,
            Err(error) => {
                return match self.facts.rollback_candidate_transaction(checkpoint) {
                    Ok(()) => Err(AnalyzerExpressionError::fact(error)),
                    Err(failure) => Err(AnalyzerExpressionError::fact(
                        self.facts.abort_after_close_failure(failure),
                    )),
                };
            }
        };
        let action = match operation(self, expression_authority, transaction_authority) {
            Ok(action) => action,
            Err(error) => {
                return match self.facts.rollback_candidate_transaction(checkpoint) {
                    Ok(()) => Err(error.into().into_expression_error()),
                    Err(failure) => Err(AnalyzerExpressionError::fact(
                        self.facts.abort_after_close_failure(failure),
                    )),
                };
            }
        };
        match action {
            CandidateFactTransactionAction::Commit(value) => {
                match self.facts.commit_candidate_transaction(checkpoint) {
                    Ok(()) => Ok(CandidateFactTransactionOutcome::Committed(value)),
                    Err(failure) => Err(AnalyzerExpressionError::fact(
                        self.facts.abort_after_close_failure(failure),
                    )),
                }
            }
            CandidateFactTransactionAction::Rollback(value) => {
                match self.facts.rollback_candidate_transaction(checkpoint) {
                    Ok(()) => Ok(CandidateFactTransactionOutcome::RolledBack(value)),
                    Err(failure) => Err(AnalyzerExpressionError::fact(
                        self.facts.abort_after_close_failure(failure),
                    )),
                }
            }
            CandidateFactTransactionAction::Extract(value) => {
                match self.facts.extract_and_rollback(checkpoint) {
                    Ok(projection) => {
                        Ok(CandidateFactTransactionOutcome::Extracted { value, projection })
                    }
                    Err(failure) => Err(AnalyzerExpressionError::fact(
                        self.facts.abort_after_close_failure(failure),
                    )),
                }
            }
        }
    }
}

fn projected_call_site_inventories_match(
    mut graph_sites: BTreeSet<CheckedCallSite>,
    delta_sites: impl IntoIterator<Item = CheckedCallSite>,
    mut expression_sites: BTreeMap<ExprId, CheckedCallSite>,
    expression_updates: BTreeMap<ExprId, Option<CheckedCallSite>>,
) -> bool {
    for site in delta_sites {
        if !graph_sites.insert(site) {
            return false;
        }
    }
    for (owner, site) in expression_updates {
        match site {
            Some(site) if site.expression() == owner => {
                expression_sites.insert(owner, site);
            }
            Some(_) => return false,
            None => {
                expression_sites.remove(&owner);
            }
        }
    }
    expression_sites.values().copied().collect::<BTreeSet<_>>() == graph_sites
}

fn restore_map_entry<K: Ord, V>(map: &mut BTreeMap<K, V>, key: K, value: Option<V>) {
    if let Some(value) = value {
        map.insert(key, value);
    } else {
        map.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callable::{
        CallConstraintInvariant, CheckedCallSite, PreparedCallContinuationRef, PreparedCallGraph,
    };

    fn call_identity() -> (ExprId, LocalId, PatternId) {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn consume(value: i64) -> i64 { value }\n",
                "fn caller(value: i64) { consume(value); }\n",
            ),
            None,
        );
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root module");
        let local = module
            .locals()
            .next()
            .map(|(owner, _)| owner)
            .expect("local");
        let pattern = module
            .patterns()
            .next()
            .map(|(owner, _)| owner)
            .expect("pattern");
        let call = crate::final_analysis::tests::analyze(&fixture)
            .expect("call fixture")
            .calls()
            .next()
            .map(|(_, facts)| facts.expression())
            .expect("call fact");
        (call, local, pattern)
    }

    fn distinct_test_locals() -> (LocalId, LocalId) {
        let fixture = crate::final_analysis::tests::fixture(
            "fn pair(first: i64, second: i64) { first; second; }\n",
            None,
        );
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root module");
        let mut locals = module.locals().map(|(owner, _)| owner);
        let first = locals.next().expect("first local");
        let second = locals.next().expect("second local");
        (first, second)
    }

    fn graph_sites() -> Vec<CheckedCallSite> {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller(value: i64) { consume(value); }\nfn consume(value: i64) -> i64 { value }\n",
            None,
        );
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root HIR module");
        module
            .expressions()
            .take(4)
            .map(|(owner, _)| CheckedCallSite::HirCall(owner))
            .collect()
    }

    fn no_graph_dependencies() -> Box<[PreparedCallContinuationRef]> {
        Vec::new().into_boxed_slice()
    }

    #[test]
    fn projection_call_correlation_rejects_missing_expression_site_before_apply() {
        let site = graph_sites()[0];
        assert!(!projected_call_site_inventories_match(
            BTreeSet::new(),
            [site],
            BTreeMap::new(),
            BTreeMap::new(),
        ));
    }

    #[test]
    fn projection_call_correlation_rejects_extra_expression_site_before_apply() {
        let site = graph_sites()[0];
        assert!(!projected_call_site_inventories_match(
            BTreeSet::new(),
            [],
            BTreeMap::new(),
            BTreeMap::from([(site.expression(), Some(site))]),
        ));
    }

    #[test]
    fn projection_call_correlation_rejects_wrong_site_family_before_apply() {
        let site = graph_sites()[0];
        let CheckedCallSite::HirCall(owner) = site else {
            panic!("fixture site must be a HIR call");
        };
        assert!(!projected_call_site_inventories_match(
            BTreeSet::new(),
            [site],
            BTreeMap::new(),
            BTreeMap::from([(owner, Some(CheckedCallSite::DialogueApplication(owner)),)]),
        ));
    }

    #[test]
    fn prepared_call_graph_site_seal_is_exactly_idempotent() {
        let site = graph_sites()[0];
        let mut graph = PreparedCallGraph::<(), u8>::new();
        let checkpoint = graph.begin_delta().expect("graph checkpoint");
        graph
            .seal_unselected(site, no_graph_dependencies(), 1)
            .expect("first site seal");
        graph
            .seal_unselected(site, no_graph_dependencies(), 1)
            .expect("identical site evidence reuses its one graph node");
        assert_eq!(
            graph
                .seal_unselected(site, no_graph_dependencies(), 2)
                .expect_err("different evidence cannot reuse a sealed site"),
            CallConstraintInvariant::PreparedGraphReplayMismatch
        );
        graph
            .rollback_delta(checkpoint)
            .expect("one idempotent node rolls back exactly");
    }

    #[test]
    fn prepared_call_graph_nested_lifo_and_exact_restore() {
        let sites = graph_sites();
        assert!(sites.len() >= 3);
        let mut graph = PreparedCallGraph::<()>::new();
        let outer = graph.begin_delta().expect("outer graph checkpoint");
        graph
            .seal_unselected(sites[0], no_graph_dependencies(), ())
            .expect("outer node");
        let inner = graph.begin_delta().expect("inner graph checkpoint");
        graph
            .seal_unselected(sites[1], no_graph_dependencies(), ())
            .expect("inner node");
        graph.commit_delta(inner).expect("nested commit");
        graph.rollback_delta(outer).expect("outer rollback");

        let source = graph.begin_delta().expect("source checkpoint");
        graph
            .seal_unselected(sites[0], no_graph_dependencies(), ())
            .expect("source node");
        let delta = graph.extract_delta(source).expect("extract source");

        let outer = graph.begin_delta().expect("restore outer checkpoint");
        graph
            .seal_unselected(sites[1], no_graph_dependencies(), ())
            .expect("outer baseline node");
        graph
            .restore_delta(&outer, delta)
            .expect("restore into outer");
        graph
            .rollback_delta(outer)
            .expect("rollback restored nodes");

        let source = graph.begin_delta().expect("collision source checkpoint");
        graph
            .seal_unselected(sites[2], no_graph_dependencies(), ())
            .expect("collision source node");
        let delta = graph
            .extract_delta(source)
            .expect("extract collision source");
        let outer = graph.begin_delta().expect("collision outer checkpoint");
        graph
            .seal_unselected(sites[2], no_graph_dependencies(), ())
            .expect("collision node");
        let failure = graph
            .restore_delta(&outer, delta)
            .expect_err("restore preflight must reject duplicate site");
        let (_, delta) = failure.into_parts();
        graph.rollback_delta(outer).expect("rollback collision");
        let target = graph.begin_delta().expect("retry target checkpoint");
        graph
            .restore_delta(&target, delta)
            .expect("retry exact delta");
        graph.rollback_delta(target).expect("rollback retry target");
    }

    #[test]
    fn prepared_call_graph_restore_requires_exact_active_target() {
        let sites = graph_sites();
        let mut graph = PreparedCallGraph::<()>::new();
        let source = graph.begin_delta().expect("source checkpoint");
        graph
            .seal_unselected(sites[0], no_graph_dependencies(), ())
            .expect("source node");
        let delta = graph.extract_delta(source).expect("extract source");

        let outer = graph.begin_delta().expect("outer target");
        let inner = graph.begin_delta().expect("inner target");
        let failure = graph
            .restore_delta(&outer, delta)
            .expect_err("non-LIFO target must reject restore");
        let (violation, delta) = failure.into_parts();
        assert_eq!(violation, CallConstraintInvariant::PreparedGraphDeltaOrder);
        graph.rollback_delta(inner).expect("close inner target");
        graph
            .restore_delta(&outer, delta)
            .expect("outer target is now exact top");
        graph.rollback_delta(outer).expect("rollback restored node");
    }

    #[test]
    fn non_lifo_checkpoint_failure_returns_the_token_and_preserves_ledger() {
        let mut state = SemanticFactState::new();
        let outer = state
            .begin_candidate_transaction()
            .expect("outer checkpoint");
        let inner = state
            .begin_candidate_transaction()
            .expect("inner checkpoint");

        let failure = state
            .commit_candidate_transaction(outer)
            .expect_err("outer checkpoint is not LIFO");
        let (violation, outer) = failure.into_parts();
        assert_eq!(
            violation,
            CandidateFactTransactionViolation::NonLifoCheckpoint
        );
        assert!(state.rollback_candidate_transaction(inner).is_ok());
        assert!(state.rollback_candidate_transaction(outer).is_ok());
    }

    #[test]
    fn foreign_checkpoint_failure_returns_the_token_to_its_issuer() {
        let mut origin = SemanticFactState::new();
        let checkpoint = origin.begin_candidate_transaction().expect("checkpoint");
        let mut foreign = SemanticFactState::new();
        let failure = foreign
            .rollback_candidate_transaction(checkpoint)
            .expect_err("foreign checkpoint is rejected");
        let (violation, checkpoint) = failure.into_parts();
        assert_eq!(
            violation,
            CandidateFactTransactionViolation::ForeignCheckpoint
        );
        assert!(origin.rollback_candidate_transaction(checkpoint).is_ok());
    }

    #[test]
    fn stale_checkpoint_is_distinct_from_foreign_and_non_lifo() {
        let mut origin = SemanticFactState::new();
        let checkpoint = origin.begin_candidate_transaction().expect("checkpoint");
        let mut same_issuer_empty_ledger = SemanticFactState::new();
        same_issuer_empty_ledger.issuer = Arc::clone(&origin.issuer);
        let failure = same_issuer_empty_ledger
            .rollback_candidate_transaction(checkpoint)
            .expect_err("checkpoint is absent from this issuer ledger");
        let (violation, checkpoint) = failure.into_parts();
        assert_eq!(
            violation,
            CandidateFactTransactionViolation::StaleCheckpoint
        );
        assert!(origin.rollback_candidate_transaction(checkpoint).is_ok());
    }

    #[test]
    fn foreign_projection_failure_returns_the_projection_to_its_issuer() {
        let mut origin = SemanticFactState::new();
        let probe = origin.begin_candidate_transaction().expect("probe");
        let projection = origin
            .extract_and_rollback(probe)
            .unwrap_or_else(|_| panic!("origin projection extraction"));
        let mut foreign = SemanticFactState::new();
        let foreign_transaction = foreign
            .begin_candidate_transaction()
            .expect("foreign transaction");
        let foreign_authority = foreign
            .transaction_authority(&foreign_transaction)
            .expect("foreign authority");
        let failure = foreign
            .apply_candidate_projection(&foreign_authority, projection)
            .expect_err("foreign projection is rejected");
        let (violation, projection) = failure.into_parts();
        assert_eq!(
            violation,
            CandidateFactTransactionViolation::ForeignProjection
        );
        drop(foreign_authority);
        assert!(
            foreign
                .rollback_candidate_transaction(foreign_transaction)
                .is_ok()
        );
        let origin_transaction = origin
            .begin_candidate_transaction()
            .expect("origin transaction");
        let origin_authority = origin
            .transaction_authority(&origin_transaction)
            .expect("origin authority");
        assert!(
            origin
                .apply_candidate_projection(&origin_authority, projection)
                .is_ok()
        );
        drop(origin_authority);
        assert!(
            origin
                .rollback_candidate_transaction(origin_transaction)
                .is_ok()
        );
    }

    #[test]
    fn projection_requires_a_target_authority() {
        let mut state = SemanticFactState::new();
        let probe = state.begin_candidate_transaction().expect("probe");
        let projection = state
            .extract_and_rollback(probe)
            .unwrap_or_else(|_| panic!("projection extraction"));
        let transaction = state.begin_candidate_transaction().expect("transaction");
        let authority = state
            .transaction_authority(&transaction)
            .expect("target authority");
        assert!(
            state
                .apply_candidate_projection(&authority, projection)
                .is_ok()
        );
        drop(authority);
        assert!(state.rollback_candidate_transaction(transaction).is_ok());
    }

    #[test]
    fn checkpoint_sequence_exhaustion_is_typed() {
        let mut state = SemanticFactState::new();
        state.next_checkpoint_id = u64::MAX;
        assert!(matches!(
            state.begin_candidate_transaction(),
            Err(CandidateFactTransactionViolation::SequenceExhausted)
        ));
    }

    #[test]
    fn implicit_capture_use_is_discarded_by_candidate_rollback() {
        let (call, local, _) = call_identity();
        let callable = call;
        let expression = callable;
        let mut state = SemanticFactState::new();
        let checkpoint = state.begin_candidate_transaction().expect("checkpoint");

        state
            .record_implicit_capture_use(callable, expression, local)
            .expect("capture-use row");
        assert_eq!(
            state.implicit_capture_use(callable, expression),
            Some(local)
        );

        state
            .rollback_candidate_transaction(checkpoint)
            .expect("rollback candidate");
        assert!(state.pending_implicit_capture_uses().is_empty());
        assert_eq!(state.implicit_capture_use(callable, expression), None);
    }

    #[test]
    fn implicit_capture_use_projection_survives_extract_apply_until_outer_rollback() {
        let (call, local, _) = call_identity();
        let callable = call;
        let expression = callable;
        let mut state = SemanticFactState::new();
        let probe = state.begin_candidate_transaction().expect("probe");

        state
            .record_implicit_capture_use(callable, expression, local)
            .expect("capture-use row");
        let projection = state
            .extract_and_rollback(probe)
            .expect("extract projection");
        assert!(state.pending_implicit_capture_uses().is_empty());

        let outer = state.begin_candidate_transaction().expect("outer");
        let authority = state
            .transaction_authority(&outer)
            .expect("outer authority");
        state
            .apply_candidate_projection(&authority, projection)
            .expect("apply projection");
        drop(authority);
        assert_eq!(
            state.implicit_capture_use(callable, expression),
            Some(local)
        );

        state
            .rollback_candidate_transaction(outer)
            .expect("rollback outer");
        assert!(state.pending_implicit_capture_uses().is_empty());
    }

    #[test]
    fn implicit_capture_use_conflict_is_terminal_and_exact_repeat_is_idempotent() {
        let (call, local, _) = call_identity();
        let callable = call;
        let expression = callable;
        let mut state = SemanticFactState::new();
        let checkpoint = state.begin_candidate_transaction().expect("checkpoint");

        state
            .record_implicit_capture_use(callable, expression, local)
            .expect("first row");
        state
            .record_implicit_capture_use(callable, expression, local)
            .expect("exact repeat is idempotent");
        let (_, conflicting_local) = distinct_test_locals();
        assert_ne!(local, conflicting_local);
        let conflict = state
            .record_implicit_capture_use(callable, expression, conflicting_local)
            .expect_err("different local for one use is an invariant failure");
        assert_eq!(
            conflict,
            CandidateFactTransactionViolation::ImplicitCaptureUseConflict {
                callable,
                expression,
                existing: local,
                proposed: conflicting_local,
            }
        );
        assert_eq!(
            state.implicit_capture_use(callable, expression),
            Some(local)
        );
        state
            .rollback_candidate_transaction(checkpoint)
            .expect("rollback after conflict");
    }
}
