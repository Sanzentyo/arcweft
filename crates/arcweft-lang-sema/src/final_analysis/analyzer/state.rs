//! Candidate-transaction state and mutation rollback.

use std::sync::Arc;

use arcweft_lang_hir::project::HirSemanticPathOwnerId;

use super::calls::{
    AnalyzerCallConstraintDomain, AnalyzerPreparedCallGraph, AnalyzerPreparedCallPrefix,
    AnalyzerPreparedUnselectedCall, CompletedPhysicalCallSource,
};
use super::expression_error::PhysicalCallAttemptId;
use super::statements::PreparedIteration;
use super::{
    BTreeMap, BTreeSet, CallTargetFacts, CandidateFactTransactionViolation, ExprId, LocalId,
    PatternId, PhysicalCandidateArgumentEvaluation, TypeKind,
};
use crate::callable::{
    CheckedCallSite, EnclosingGenericParameterScope, PreparedCallContinuationRef,
    PreparedCallGraph, PreparedCallGraphCheckpoint, PreparedCallGraphDelta,
    PreparedCallGraphReplayMismatch, PreparedCallGraphSiteState,
};
use crate::{
    checked_rich_text::{PreparedCheckedContentCatalog, PreparedCheckedRichTextCheck},
    effect_row::{DecisionControl, DecisionWork},
    final_analysis::{
        PreparedContentApplication, PreparedDialogueApplication, PreparedExpressionFact,
        accounting::PhysicalSourceType, prepared::PreparedExpressionTypeUpdates,
    },
    types::{
        PreparedEffects, TypeProjectionControl, TypeProjectionNodeKind,
        constraints::{RetainedSourceCompletion, TypeConstraintError},
    },
};

#[derive(Debug)]
struct CandidateFactIssuer;

/// Affine authority to replace one dialogue application after its marker
/// catalog and prepared shell have been moved into the late coordinate seal.
pub(super) struct TakenDialogueApplication {
    owner: ExprId,
    site: CheckedCallSite,
    application: Box<PreparedDialogueApplication>,
    content: PreparedCheckedRichTextCheck,
}

impl TakenDialogueApplication {
    pub(super) fn into_parts(
        self,
    ) -> (
        Box<PreparedDialogueApplication>,
        PreparedCheckedRichTextCheck,
        DialogueApplicationReplacementProof,
    ) {
        (
            self.application,
            self.content,
            DialogueApplicationReplacementProof {
                owner: self.owner,
                site: self.site,
            },
        )
    }
}

/// Affine authority to replace one generic content application after its
/// checked body row has been consumed by the late seal.
pub(super) struct TakenContentApplication {
    owner: ExprId,
    site: CheckedCallSite,
    application: Box<PreparedContentApplication>,
    content: Option<PreparedCheckedRichTextCheck>,
}

impl TakenContentApplication {
    pub(super) fn into_parts(
        self,
    ) -> (
        Box<PreparedContentApplication>,
        Option<PreparedCheckedRichTextCheck>,
        ContentApplicationReplacementProof,
    ) {
        (
            self.application,
            self.content,
            ContentApplicationReplacementProof {
                owner: self.owner,
                site: self.site,
            },
        )
    }
}

/// Move-only proof that the late seal took the exact prior call-correlated
/// dialogue row and may publish only its same-site final replacement.
pub(super) struct DialogueApplicationReplacementProof {
    owner: ExprId,
    site: CheckedCallSite,
}

/// Move-only proof that a generic content application took the exact
/// prior call-correlated row and may publish only its same-site replacement.
pub(super) struct ContentApplicationReplacementProof {
    owner: ExprId,
    site: CheckedCallSite,
}

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

/// An ordinary child checkpoint within one probe callback's effect scope.
pub(super) struct ActiveProbeFactAttempt {
    checkpoint: CandidateFactCheckpoint,
    physical_checkpoint: CandidateFactCheckpointId,
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
    physical_checkpoint: CandidateFactCheckpointId,
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
            physical_checkpoint: self.physical_checkpoint,
        }
    }

    pub(super) fn physical_attempt_context(&self) -> CandidatePhysicalAttemptContext {
        CandidatePhysicalAttemptContext {
            issuer: Arc::clone(&self.checkpoint.issuer),
            epoch: self.checkpoint.epoch,
            id: self.physical_checkpoint,
        }
    }
}

#[derive(Debug)]
pub(super) enum CandidateFactMutationFailure<E> {
    Control(E),
    Invariant(CandidateFactTransactionViolation),
}

impl<E> From<CandidateFactTransactionViolation> for CandidateFactMutationFailure<E> {
    fn from(error: CandidateFactTransactionViolation) -> Self {
        Self::Invariant(error)
    }
}

impl<E> From<crate::callable::PreparedCallGraphMutationFailure<E>>
    for CandidateFactMutationFailure<E>
{
    fn from(error: crate::callable::PreparedCallGraphMutationFailure<E>) -> Self {
        match error {
            crate::callable::PreparedCallGraphMutationFailure::Control(error) => {
                Self::Control(error)
            }
            crate::callable::PreparedCallGraphMutationFailure::Invariant(error) => Self::Invariant(
                CandidateFactTransactionViolation::PreparedCallGraph(error.into()),
            ),
        }
    }
}

impl From<CandidateFactMutationFailure<std::convert::Infallible>>
    for CandidateFactTransactionViolation
{
    fn from(error: CandidateFactMutationFailure<std::convert::Infallible>) -> Self {
        match error {
            CandidateFactMutationFailure::Control(never) => match never {},
            CandidateFactMutationFailure::Invariant(error) => error,
        }
    }
}

#[derive(Debug)]
pub(super) struct CandidateFactCloseFailure<E = std::convert::Infallible> {
    violation: CandidateFactMutationFailure<E>,
    checkpoint: Box<CandidateFactCheckpoint>,
}

impl<E> CandidateFactCloseFailure<E> {
    pub(super) fn into_parts(self) -> (CandidateFactMutationFailure<E>, CandidateFactCheckpoint) {
        (self.violation, *self.checkpoint)
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
pub(super) type ImplicitCaptureUseKey = (ExprId, super::super::CheckedLocalUseSite);

#[derive(Debug)]
pub(super) struct CandidateProjectionApplyFailure<
    E = std::convert::Infallible,
    P = CandidateSemanticProjection,
> {
    violation: CandidateFactMutationFailure<E>,
    projection: P,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ExpressionFactWriteViolation {
    AlreadyPublished,
    MissingPublishedFact,
    Candidate(CandidateFactTransactionViolation),
}

impl<E, P> CandidateProjectionApplyFailure<E, P> {
    pub(super) fn into_parts(self) -> (CandidateFactMutationFailure<E>, P) {
        (self.violation, self.projection)
    }
}

impl<P> CandidateProjectionApplyFailure<crate::final_analysis::FinalSemanticAnalysisError, P> {
    pub(super) fn into_expression_error(self) -> super::expression_error::AnalyzerExpressionError {
        match self.violation {
            CandidateFactMutationFailure::Control(error) => {
                super::expression_error::AnalyzerExpressionError::fatal(error)
            }
            CandidateFactMutationFailure::Invariant(error) => {
                super::expression_error::AnalyzerExpressionError::fact(error)
            }
        }
    }
}

/// Sole mutable owner for candidate-sensitive semantic facts.
///
/// Every write flows through this owner so candidate probes cannot bypass the
/// rollback journal by mutating one fact map directly.
pub(super) struct SemanticFactState {
    issuer: Arc<CandidateFactIssuer>,
    locals: BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>>,
    patterns: BTreeMap<PatternId, TypeKind<crate::types::PreparedEffects>>,
    expressions: BTreeMap<ExprId, PreparedExpressionFact>,
    checked_content: PreparedCheckedContentCatalog,
    expression_stack: BTreeSet<ExprId>,
    iteration_facts: BTreeMap<ExprId, PreparedIteration>,
    implicit_capture_uses: BTreeMap<ImplicitCaptureUseKey, LocalId>,
    /// Rollback/extraction bookkeeping only. Selected input coordinates issue
    /// semantic source order after inference; probe event order never does.
    implicit_capture_journal_order: Vec<ImplicitCaptureUseKey>,
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

/// Borrowed observations while the original effect graph is being prepared.
/// These tables remain the sole owners of local and pattern result types.
pub(super) struct PatternTypeObservations<'a> {
    locals: &'a BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>>,
    patterns: &'a BTreeMap<PatternId, TypeKind<crate::types::PreparedEffects>>,
}

impl PatternTypeObservations<'_> {
    pub(super) fn locals(&self) -> &BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>> {
        self.locals
    }

    pub(super) fn local(&self, owner: LocalId) -> Option<&TypeKind<crate::types::PreparedEffects>> {
        self.locals.get(&owner)
    }
    pub(super) fn pattern(
        &self,
        owner: PatternId,
    ) -> Option<&TypeKind<crate::types::PreparedEffects>> {
        self.patterns.get(&owner)
    }
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
/// The complete payload stays heap-owned during callback return, replay,
/// application failure and final transfer; those paths carry the same owner.
pub(super) struct CandidateSemanticProjection {
    data: Box<CandidateSemanticProjectionData>,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct RetainedProbeTypes {
    bindings: BTreeMap<HirSemanticPathOwnerId, TypeKind<PreparedEffects>>,
    expressions: BTreeMap<ExprId, PreparedExpressionTypeUpdates>,
}

#[cfg(test)]
impl RetainedProbeTypes {
    pub(super) fn unchanged_for_test() -> Self {
        Self::default()
    }
}

impl CandidateSemanticProjection {
    pub(super) fn probe_effect_reads(
        &self,
    ) -> Option<&crate::effect_row::prepared::PreparedEffectReadScope> {
        self.data.probe_effect_reads.as_ref()
    }

    pub(super) fn visit_effect_types<'p, E>(
        &'p self,
        visit: &mut impl FnMut(&'p TypeKind<crate::types::PreparedEffects>) -> Result<(), E>,
    ) -> Result<(), E> {
        let facts = &self.data.facts;
        for ty in facts
            .locals
            .values()
            .chain(facts.patterns.values())
            .flatten()
        {
            visit(ty)?;
        }
        for fact in facts.expressions.values().flatten() {
            fact.visit_types(visit)?;
        }
        for iteration in facts.iterations.values().flatten() {
            iteration.visit_types(visit)?;
        }
        Ok(())
    }

    /// Absence, a projected deletion, and a retained expression are distinct.
    /// The borrowed fact is original preparation, not a closed type witness.
    pub(super) fn expression_update(
        &self,
        owner: ExprId,
    ) -> Option<Option<&PreparedExpressionFact>> {
        self.data.facts.expressions.get(&owner).map(Option::as_ref)
    }

    pub(super) fn retained_expression_value(
        &self,
        owner: ExprId,
    ) -> Option<&PreparedExpressionFact> {
        let facts = &self.data.facts;
        let fact = self.expression_update(owner).flatten()?;
        (facts.checked_content.is_empty()
            && facts.iterations.is_empty()
            && facts.implicit_capture_uses.is_empty()
            && facts.implicit_capture_journal_order.is_empty()
            && facts
                .physical_candidate_argument_evaluations
                .rows
                .is_empty()
            && self.data.graph_delta.is_isolated_expression_delta()
            && fact.checked_call_site(owner).is_none()
            && fact.supports_primary_type_update())
        .then_some(fact)
    }

    pub(super) fn prepare_retained_types<C: TypeProjectionControl<Error = TypeConstraintError>>(
        &self,
        owner: ExprId,
        completion: RetainedSourceCompletion<'_, AnalyzerCallConstraintDomain>,
        control: &mut C,
    ) -> Result<Option<RetainedProbeTypes>, TypeConstraintError> {
        let Some(source) =
            EnclosingGenericParameterScope::original_source_owner(completion.original())
        else {
            return Ok(None);
        };
        let mut updates = RetainedProbeTypes::default();
        let facts = &self.data.facts;
        for (fact_owner, ty) in facts
            .locals
            .iter()
            .map(|(&key, ty)| (HirSemanticPathOwnerId::Local(key), ty.as_ref()))
            .chain(
                facts
                    .patterns
                    .iter()
                    .map(|(&key, ty)| (HirSemanticPathOwnerId::Pattern(key), ty.as_ref())),
            )
        {
            control.visit_node(TypeProjectionNodeKind::Type, 1)?;
            if !source.admits_owner(fact_owner) {
                return Ok(None);
            }
            if let Some(ty) = ty {
                let Some(projected) = completion.project_declaration_type(ty, control)? else {
                    return Ok(None);
                };
                if let Some(replacement) = projected.into_replacement() {
                    control.visit_binding()?;
                    updates.bindings.insert(fact_owner, replacement);
                }
            }
        }
        for (&expression, fact) in &facts.expressions {
            control.visit_node(TypeProjectionNodeKind::Type, 1)?;
            let fact_owner = HirSemanticPathOwnerId::Expression(expression);
            if !source.admits_owner(fact_owner) {
                return Ok(None);
            }
            let Some(fact) = fact else {
                continue;
            };
            if expression == owner && matches!(fact, PreparedExpressionFact::Resolved(_)) {
                continue;
            }
            if fact.checked_call_site(expression).is_some() {
                return Ok(None);
            }
            let Some(update) = fact.prepare_type_updates(&completion, control)? else {
                return Ok(None);
            };
            if expression == owner || !update.is_empty() {
                control.visit_binding()?;
                updates.expressions.insert(expression, update);
            }
        }
        let result = completion.into_value().into_replacement();
        if let Some(update) = updates.expressions.get(&owner) {
            if !update.matches_result(result.as_ref()) {
                return Ok(None);
            }
        } else if let Some(replacement) = result {
            control.visit_binding()?;
            updates.expressions.insert(
                owner,
                PreparedExpressionTypeUpdates::resolved_type(replacement),
            );
        }
        Ok(Some(updates))
    }

    pub(super) fn effect_view<'a>(
        &'a self,
        graph: &'a AnalyzerPreparedCallGraph,
    ) -> Result<crate::callable::PreparedCallableEffectView<'a>, CandidateFactTransactionViolation>
    {
        graph
            .extracted_effect_view(&self.data.graph_delta)
            .map_err(|error| CandidateFactTransactionViolation::PreparedCallGraph(error.into()))
    }
}

struct CandidateSemanticProjectionData {
    authority: CandidateProjectionAuthority,
    graph_delta: PreparedCallGraphDelta<AnalyzerPreparedCallPrefix, AnalyzerPreparedUnselectedCall>,
    probe_effect_reads: Option<crate::effect_row::prepared::PreparedEffectReadScope>,
    facts: CandidateProjectionFacts,
}

struct CandidateProjectionFacts {
    locals: BTreeMap<LocalId, Option<TypeKind<crate::types::PreparedEffects>>>,
    patterns: BTreeMap<PatternId, Option<TypeKind<crate::types::PreparedEffects>>>,
    expressions: BTreeMap<ExprId, Option<PreparedExpressionFact>>,
    checked_content: BTreeMap<
        arcweft_lang_hir::dialogue_application::HirDialogueContentId,
        Option<PreparedCheckedRichTextCheck>,
    >,
    iterations: BTreeMap<ExprId, Option<PreparedIteration>>,
    implicit_capture_uses: BTreeMap<ImplicitCaptureUseKey, Option<LocalId>>,
    implicit_capture_journal_order: Box<[ImplicitCaptureUseKey]>,
    physical_candidate_argument_evaluations: PhysicalCandidateEvaluationTranscript,
}

impl From<CandidateSemanticProjectionData> for CandidateSemanticProjection {
    fn from(data: CandidateSemanticProjectionData) -> Self {
        Self {
            data: Box::new(data),
        }
    }
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

    fn matching_row_mut(
        &mut self,
        root: ExprId,
        proposed: &PhysicalCandidateArgumentEvaluation,
    ) -> Option<&mut PhysicalCandidateArgumentEvaluation> {
        self.rows
            .get_mut(&root)?
            .iter_mut()
            .find(|existing| existing.same_candidate_slot(proposed))
    }

    fn merge(&mut self, other: Self) {
        for (root, rows) in other.rows {
            let target = self.rows.entry(root).or_default();
            for proposed in rows {
                if let Some(existing) = target
                    .iter_mut()
                    .find(|existing| existing.same_candidate_slot(&proposed))
                {
                    existing.merge_observations(proposed);
                } else {
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
    Projection(
        Box<CandidateProjectionApplyFailure<crate::final_analysis::FinalSemanticAnalysisError>>,
    ),
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
            Self::Projection(failure) => failure.into_expression_error(),
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
            .field("epoch", &self.data.authority.epoch)
            .field("locals", &self.data.facts.locals.len())
            .field("patterns", &self.data.facts.patterns.len())
            .field("expressions", &self.data.facts.expressions.len())
            .field("checked_content", &self.data.facts.checked_content.len())
            .field("iterations", &self.data.facts.iterations.len())
            .field(
                "implicit_capture_uses",
                &self.data.facts.implicit_capture_uses.len(),
            )
            .field(
                "physical_candidate_argument_evaluations",
                &self
                    .data
                    .facts
                    .physical_candidate_argument_evaluations
                    .row_count(),
            )
            .field("graph_delta", &"sealed")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CandidateFactReplayMode {
    Semantic,
    Exact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CandidateSemanticReplayMismatch {
    Authority,
    PreparedGraph(PreparedCallGraphReplayMismatch),
    Locals,
    Patterns,
    Expressions,
    CheckedContent,
    Iterations,
    ImplicitCaptureUses,
    PhysicalCandidateEvaluations,
}

impl SemanticFactState {
    /// Replay is an observation by the live fact owner, not context-free value
    /// equality of two detached capabilities. Even two matching projections
    /// must belong to this healthy generation and its actual prepared graph.
    pub(super) fn projection_replay_mismatch(
        &self,
        left: &CandidateSemanticProjection,
        right: &CandidateSemanticProjection,
        mode: CandidateFactReplayMode,
    ) -> Option<CandidateSemanticReplayMismatch> {
        if self.ensure_healthy().is_err()
            || !Arc::ptr_eq(&self.issuer, &left.data.authority.issuer)
            || !Arc::ptr_eq(&self.issuer, &right.data.authority.issuer)
            || self.epoch != left.data.authority.epoch
            || self.epoch != right.data.authority.epoch
        {
            return Some(CandidateSemanticReplayMismatch::Authority);
        }
        let Some(graph) = self.prepared_calls.as_ref() else {
            return Some(CandidateSemanticReplayMismatch::Authority);
        };
        if let Some(mismatch) =
            graph.replay_delta_mismatch(&left.data.graph_delta, &right.data.graph_delta)
        {
            return Some(CandidateSemanticReplayMismatch::PreparedGraph(mismatch));
        }
        if left.data.facts.locals != right.data.facts.locals {
            return Some(CandidateSemanticReplayMismatch::Locals);
        }
        if left.data.facts.patterns != right.data.facts.patterns {
            return Some(CandidateSemanticReplayMismatch::Patterns);
        }
        if left.data.facts.expressions != right.data.facts.expressions {
            return Some(CandidateSemanticReplayMismatch::Expressions);
        }
        if left.data.facts.checked_content != right.data.facts.checked_content {
            return Some(CandidateSemanticReplayMismatch::CheckedContent);
        }
        if left.data.facts.iterations != right.data.facts.iterations {
            return Some(CandidateSemanticReplayMismatch::Iterations);
        }
        if left.data.facts.implicit_capture_uses != right.data.facts.implicit_capture_uses {
            return Some(CandidateSemanticReplayMismatch::ImplicitCaptureUses);
        }
        if mode == CandidateFactReplayMode::Exact
            && left.data.facts.physical_candidate_argument_evaluations
                != right.data.facts.physical_candidate_argument_evaluations
        {
            return Some(CandidateSemanticReplayMismatch::PhysicalCandidateEvaluations);
        }
        None
    }
}

#[derive(Default)]
struct CandidateProjectionOwners {
    locals: BTreeSet<LocalId>,
    patterns: BTreeSet<PatternId>,
    expressions: BTreeSet<ExprId>,
    checked_content: BTreeSet<arcweft_lang_hir::dialogue_application::HirDialogueContentId>,
    iterations: BTreeSet<ExprId>,
    implicit_capture_uses: BTreeSet<ImplicitCaptureUseKey>,
}

enum SemanticFactMutation {
    Local {
        owner: LocalId,
        previous: Option<Box<TypeKind<crate::types::PreparedEffects>>>,
    },
    Pattern {
        owner: PatternId,
        previous: Option<Box<TypeKind<crate::types::PreparedEffects>>>,
    },
    Expression {
        owner: ExprId,
        previous: Option<PreparedExpressionFact>,
    },
    CheckedContent {
        content: arcweft_lang_hir::dialogue_application::HirDialogueContentId,
        previous: Option<Box<PreparedCheckedRichTextCheck>>,
    },
    Iteration {
        owner: ExprId,
        previous: Option<Box<PreparedIteration>>,
    },
    ImplicitCaptureUse {
        key: ImplicitCaptureUseKey,
        previous: Option<LocalId>,
        order_index: usize,
    },
}

impl SemanticFactState {
    pub(super) fn new() -> Self {
        Self {
            issuer: Arc::new(CandidateFactIssuer),
            locals: BTreeMap::new(),
            patterns: BTreeMap::new(),
            expressions: BTreeMap::new(),
            checked_content: PreparedCheckedContentCatalog::default(),
            expression_stack: BTreeSet::new(),
            iteration_facts: BTreeMap::new(),
            implicit_capture_uses: BTreeMap::new(),
            implicit_capture_journal_order: Vec::new(),
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
            physical_checkpoint: checkpoint.id,
        })
    }

    /// Opens a purpose-specific callback fact scope.  Callers never receive
    /// the raw checkpoint; they receive only a phase-typed identity derived
    /// from the returned scope.
    pub(super) fn open_callback_fact_scope<C: crate::effect_row::DecisionControl>(
        &mut self,
        reads: Option<&crate::effect_row::prepared::PreparedEffectReadScope>,
        control: &mut C,
    ) -> Result<ActiveCallbackFactScope, CandidateFactMutationFailure<C::Error>> {
        let checkpoint = self.begin_candidate_transaction_with_effect_reads(reads, control)?;
        Ok(ActiveCallbackFactScope { checkpoint })
    }

    pub(super) fn callback_fact_authority<'a>(
        &self,
        scope: &'a ActiveCallbackFactScope,
    ) -> Result<CandidateExpressionFactAuthority<'a>, CandidateFactTransactionViolation> {
        self.candidate_expression_authority(&scope.checkpoint)
    }

    pub(super) fn open_probe_fact_attempt<C: DecisionControl>(
        &mut self,
        probe: &ActiveCallbackFactScope,
        control: &mut C,
    ) -> Result<ActiveProbeFactAttempt, CandidateFactMutationFailure<C::Error>> {
        self.transaction_authority(&probe.checkpoint)?;
        // Each alternative gets its own fact visibility cursor. Physical
        // accounting still belongs to the enclosing source probe.
        let physical_checkpoint = probe.checkpoint.id;
        let checkpoint = self.begin_candidate_transaction(control)?;
        Ok(ActiveProbeFactAttempt {
            checkpoint,
            physical_checkpoint,
        })
    }

    pub(super) fn probe_fact_attempt_authority<'a>(
        &self,
        attempt: &'a ActiveProbeFactAttempt,
    ) -> Result<CandidateExpressionFactAuthority<'a>, CandidateFactTransactionViolation> {
        let mut authority = self.candidate_expression_authority(&attempt.checkpoint)?;
        authority.physical_checkpoint = attempt.physical_checkpoint;
        Ok(authority)
    }

    pub(super) fn close_probe_fact_attempt(
        &mut self,
        attempt: ActiveProbeFactAttempt,
        disposition: crate::callable::ProbeCheckpointDisposition,
    ) -> Result<(), CandidateFactTransactionViolation> {
        match disposition {
            crate::callable::ProbeCheckpointDisposition::Retain => {
                self.commit_candidate_transaction(attempt.checkpoint)
            }
            crate::callable::ProbeCheckpointDisposition::Discard => {
                self.rollback_candidate_transaction(attempt.checkpoint)
            }
        }
        .map_err(|failure| self.abort_after_close_failure(failure))
    }

    pub(super) fn rollback_callback_fact_scope(
        &mut self,
        scope: ActiveCallbackFactScope,
    ) -> Result<(), CandidateFactCloseFailure> {
        let checkpoint = scope.into_checkpoint();
        self.rollback_candidate_transaction(checkpoint)
    }

    pub(super) fn extract_callback_fact_scope<C: crate::effect_row::DecisionControl>(
        &mut self,
        scope: ActiveCallbackFactScope,
        control: &mut C,
    ) -> Result<CandidateSemanticProjection, CandidateFactCloseFailure<C::Error>> {
        let checkpoint = scope.into_checkpoint();
        self.extract_and_rollback(checkpoint, control)
    }

    pub(super) fn abort_callback_scope_close_failure(
        &mut self,
        failure: CandidateFactCloseFailure,
    ) -> CandidateFactTransactionViolation {
        self.abort_after_close_failure(failure)
    }

    /// Operational extraction failure leaves the original checkpoint live.
    /// Roll it back before returning the exact incoming abort to its owner.
    /// Only an invariant/recovery failure poisons the semantic ledger.
    pub(super) fn recover_callback_scope_close_failure<E>(
        &mut self,
        failure: CandidateFactCloseFailure<E>,
    ) -> Result<E, CandidateFactTransactionViolation> {
        let (cause, checkpoint) = failure.into_parts();
        match cause {
            CandidateFactMutationFailure::Control(error) => {
                match self.rollback_candidate_transaction(checkpoint) {
                    Ok(()) => Ok(error),
                    Err(failure) => Err(self.abort_after_close_failure(failure)),
                }
            }
            CandidateFactMutationFailure::Invariant(violation) => Err(self
                .abort_after_close_failure(CandidateFactCloseFailure {
                    violation: violation.into(),
                    checkpoint: Box::new(checkpoint),
                })),
        }
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

    pub(super) const fn locals(
        &self,
    ) -> &BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>> {
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

    pub(super) fn prepared_calls_mut(
        &mut self,
    ) -> Result<&mut AnalyzerPreparedCallGraph, CandidateFactTransactionViolation> {
        self.prepared_calls.as_mut().ok_or_else(|| {
            CandidateFactTransactionViolation::PreparedCallGraph(
                crate::callable::CallConstraintInvariant::PreparedGraphConsumed.into(),
            )
        })
    }

    pub(super) fn initialize_effect_declarations(
        &mut self,
        declarations: crate::callable::PreparedCallableEffectDeclarations,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        self.prepared_calls_mut()?
            .initialize_effect_declarations(declarations)
            .map_err(|error| CandidateFactTransactionViolation::PreparedCallGraph(error.into()))
    }

    pub(super) fn effect_declarations_mut(
        &mut self,
    ) -> Result<
        &mut crate::callable::PreparedCallableEffectDeclarations,
        CandidateFactTransactionViolation,
    > {
        self.ensure_healthy()?;
        self.prepared_calls_mut()?
            .effect_declarations_mut()
            .map_err(|error| CandidateFactTransactionViolation::PreparedCallGraph(error.into()))
    }

    pub(super) fn pattern_preparation_parts(
        &mut self,
    ) -> Result<
        (
            &mut crate::callable::PreparedCallableEffectDeclarations,
            PatternTypeObservations<'_>,
        ),
        CandidateFactTransactionViolation,
    > {
        self.ensure_healthy()?;
        let calls = self.prepared_calls.as_mut().ok_or_else(|| {
            CandidateFactTransactionViolation::PreparedCallGraph(
                crate::callable::CallConstraintInvariant::PreparedGraphConsumed.into(),
            )
        })?;
        let declarations = calls
            .effect_declarations_mut()
            .map_err(|error| CandidateFactTransactionViolation::PreparedCallGraph(error.into()))?;
        Ok((
            declarations,
            PatternTypeObservations {
                locals: &self.locals,
                patterns: &self.patterns,
            },
        ))
    }

    pub(super) fn effect_declarations(
        &self,
    ) -> Result<
        &crate::callable::PreparedCallableEffectDeclarations,
        CandidateFactTransactionViolation,
    > {
        self.ensure_healthy()?;
        self.prepared_calls()?
            .effect_declarations()
            .map_err(|error| CandidateFactTransactionViolation::PreparedCallGraph(error.into()))
    }

    pub(super) fn request_effect_projection(
        &mut self,
        site: crate::callable::PreparedCallableEffectProjectionSite,
        candidate: &crate::callable::PreparedResolvedCallable,
    ) -> Result<crate::callable::PreparedCallResultRef, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        self.prepared_calls_mut()?
            .request_effect_projection(site, candidate)
            .map_err(|error| CandidateFactTransactionViolation::PreparedCallGraph(error.into()))
    }

    pub(super) fn complete_effect_projection(
        &mut self,
        reference: &crate::callable::PreparedCallResultRef,
        checked: &crate::callable::CheckedCallableId,
        row: crate::effect_row::EffectRow,
        result_schema: Option<crate::callable::CallableResultSchema>,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        self.prepared_calls_mut()?
            .complete_effect_projection(reference, checked, row, result_schema)
            .map_err(|error| CandidateFactTransactionViolation::PreparedCallGraph(error.into()))
    }

    pub(super) fn seal_selected_application(
        &mut self,
        target: &CandidateFactTransactionAuthority<'_>,
        site: crate::callable::CheckedCallSite,
        prefix: AnalyzerPreparedCallPrefix,
    ) -> Result<
        (
            crate::callable::CallableResultSchema,
            Option<PreparedCallContinuationRef>,
        ),
        CandidateFactTransactionViolation,
    > {
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

    pub(super) const fn patterns(
        &self,
    ) -> &BTreeMap<PatternId, TypeKind<crate::types::PreparedEffects>> {
        &self.patterns
    }

    pub(super) const fn expressions(&self) -> &BTreeMap<ExprId, PreparedExpressionFact> {
        &self.expressions
    }

    pub(super) fn take_dialogue_application(
        &mut self,
        owner: ExprId,
    ) -> Result<TakenDialogueApplication, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if self.prepared_calls.is_some()
            || !self.candidate_checkpoints.is_empty()
            || !self.candidate_journal.is_empty()
        {
            return Err(CandidateFactTransactionViolation::UnrecoverableLedger);
        }
        if !matches!(
            self.expressions.get(&owner),
            Some(PreparedExpressionFact::DialogueApplication(_))
        ) {
            return Err(CandidateFactTransactionViolation::ProjectionUnavailable);
        }
        let site =
            self.expressions
                .get(&owner)
                .and_then(|fact| fact.checked_call_site(owner))
                .filter(|site| {
                    *site == (CheckedCallSite::AttachedContentApplication {
                        expression: owner,
                        family:
                            crate::callable::CheckedAttachedContentApplicationFamily::DialogueLine,
                    })
                })
                .ok_or(CandidateFactTransactionViolation::ProjectionUnavailable)?;
        let PreparedExpressionFact::DialogueApplication(application) = self
            .expressions
            .remove(&owner)
            .expect("the checked variant test proves the row exists")
        else {
            unreachable!("the checked variant test proves the row family")
        };
        let content_id = application.content();
        let content = self
            .checked_content
            .take(content_id)
            .ok_or(CandidateFactTransactionViolation::ProjectionUnavailable)?;
        if content.content_id() != content_id || content_id.owner() != owner {
            return Err(CandidateFactTransactionViolation::ProjectionUnavailable);
        }
        Ok(TakenDialogueApplication {
            owner,
            site,
            application,
            content,
        })
    }

    pub(super) fn take_content_application(
        &mut self,
        owner: ExprId,
    ) -> Result<TakenContentApplication, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if self.prepared_calls.is_some()
            || !self.candidate_checkpoints.is_empty()
            || !self.candidate_journal.is_empty()
        {
            return Err(CandidateFactTransactionViolation::UnrecoverableLedger);
        }
        let site =
            self.expressions
                .get(&owner)
                .and_then(|fact| fact.checked_call_site(owner))
                .filter(|site| {
                    *site == (CheckedCallSite::AttachedContentApplication {
                        expression: owner,
                        family:
                            crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                    })
                })
                .ok_or(CandidateFactTransactionViolation::ProjectionUnavailable)?;
        let PreparedExpressionFact::ContentApplication(application) = self
            .expressions
            .remove(&owner)
            .ok_or(CandidateFactTransactionViolation::ProjectionUnavailable)?
        else {
            return Err(CandidateFactTransactionViolation::ProjectionUnavailable);
        };
        let content = match application.content() {
            Some(content_id) => {
                let content = self
                    .checked_content
                    .take(content_id)
                    .ok_or(CandidateFactTransactionViolation::ProjectionUnavailable)?;
                if content.content_id() != content_id || content_id.owner() != owner {
                    return Err(CandidateFactTransactionViolation::ProjectionUnavailable);
                }
                Some(content)
            }
            None => None,
        };
        Ok(TakenContentApplication {
            owner,
            site,
            application,
            content,
        })
    }

    pub(super) fn publish_sealed_dialogue_application(
        &mut self,
        proof: DialogueApplicationReplacementProof,
        value: PreparedExpressionFact,
    ) -> Result<(), ExpressionFactWriteViolation> {
        self.ensure_healthy()
            .map_err(ExpressionFactWriteViolation::Candidate)?;
        if self.prepared_calls.is_some()
            || !self.candidate_checkpoints.is_empty()
            || !self.candidate_journal.is_empty()
            || self.expressions.contains_key(&proof.owner)
            || proof.site
                != (CheckedCallSite::AttachedContentApplication {
                    expression: proof.owner,
                    family: crate::callable::CheckedAttachedContentApplicationFamily::DialogueLine,
                })
            || value.checked_call_site(proof.owner) != Some(proof.site)
        {
            return Err(ExpressionFactWriteViolation::Candidate(
                Self::call_graph_mismatch(),
            ));
        }
        self.write_expression(proof.owner, value);
        Ok(())
    }

    pub(super) fn publish_sealed_content_application(
        &mut self,
        proof: ContentApplicationReplacementProof,
        value: PreparedExpressionFact,
    ) -> Result<(), ExpressionFactWriteViolation> {
        self.ensure_healthy()
            .map_err(ExpressionFactWriteViolation::Candidate)?;
        if self.prepared_calls.is_some()
            || !self.candidate_checkpoints.is_empty()
            || !self.candidate_journal.is_empty()
            || self.expressions.contains_key(&proof.owner)
            || proof.site
                != (CheckedCallSite::AttachedContentApplication {
                    expression: proof.owner,
                    family: crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
                })
            || value.checked_call_site(proof.owner) != Some(proof.site)
        {
            return Err(ExpressionFactWriteViolation::Candidate(
                Self::call_graph_mismatch(),
            ));
        }
        self.write_expression(proof.owner, value);
        Ok(())
    }

    pub(super) fn checked_content_is_empty(&self) -> bool {
        self.checked_content.is_empty()
    }

    pub(super) const fn iteration_facts(&self) -> &BTreeMap<ExprId, PreparedIteration> {
        &self.iteration_facts
    }

    /// Moves the completed iterator-selection inventory into statement
    /// preparation after candidate evaluation has closed. No checked
    /// iteration row is copied into the affine statement payload.
    pub(super) fn take_iteration_facts_for_statements(
        &mut self,
    ) -> Result<BTreeMap<ExprId, PreparedIteration>, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        Ok(std::mem::take(&mut self.iteration_facts))
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
            .get(&(callable, expression.into()))
            .copied()
    }

    /// Affinely drains the complete implicit-capture ledger after candidate
    /// evaluation has closed. Every observed owner must belong to the exact
    /// accepted owner set; no per-owner drain can leave a second mutable view
    /// of the ledger or silently discard an unowned row.
    pub(super) fn take_implicit_capture_uses_for_owners(
        &mut self,
        owners: &BTreeSet<ExprId>,
    ) -> Result<
        BTreeMap<ExprId, Box<[(super::super::CheckedLocalUseSite, LocalId)]>>,
        CandidateFactTransactionViolation,
    > {
        self.ensure_healthy()?;
        if !self.candidate_checkpoints.is_empty() || !self.candidate_journal.is_empty() {
            return Err(CandidateFactTransactionViolation::UnrecoverableLedger);
        }
        if self.implicit_capture_uses.len() != self.implicit_capture_journal_order.len()
            || self
                .implicit_capture_uses
                .keys()
                .any(|key| !owners.contains(&key.0))
            || self
                .implicit_capture_journal_order
                .iter()
                .enumerate()
                .any(|(index, key)| {
                    self.implicit_capture_uses.get(key).is_none()
                        || self.implicit_capture_journal_order[..index].contains(key)
                        || !owners.contains(&key.0)
                })
        {
            return Err(CandidateFactTransactionViolation::UnrecoverableLedger);
        }
        let ordered = std::mem::take(&mut self.implicit_capture_journal_order);
        let mut values = std::mem::take(&mut self.implicit_capture_uses);
        let mut rows = owners
            .iter()
            .copied()
            .map(|owner| (owner, Vec::new()))
            .collect::<BTreeMap<_, Vec<_>>>();
        for key @ (owner, expression) in ordered {
            let local = values
                .remove(&key)
                .ok_or(CandidateFactTransactionViolation::UnrecoverableLedger)?;
            rows.get_mut(&owner)
                .ok_or(CandidateFactTransactionViolation::UnrecoverableLedger)?
                .push((expression, local));
        }
        if !values.is_empty() {
            return Err(CandidateFactTransactionViolation::UnrecoverableLedger);
        }
        Ok(rows
            .into_iter()
            .map(|(owner, rows)| (owner, rows.into_boxed_slice()))
            .collect())
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

    pub(super) fn retain_physical_candidate_observed_type(
        &mut self,
        identity: PhysicalCandidateArgumentEvaluation,
        actual: PhysicalSourceType,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        let active = self
            .active_physical_call_attempts
            .last()
            .ok_or(CandidateFactTransactionViolation::PhysicalCallAttemptRootMismatch)?;
        if !active.attempt.same_issued_attempt(identity.attempt())
            || actual.root() != identity.attempt().root()
        {
            return Err(CandidateFactTransactionViolation::PhysicalCallAttemptMismatch);
        }
        let root = identity.attempt().root();
        if let Some(row) = self
            .physical_candidate_argument_evaluations
            .matching_row_mut(root, &identity)
        {
            row.retain_observed_type(actual);
            return Ok(());
        }
        if let Some(row) = self
            .active_physical_candidate_argument_evaluations
            .as_mut()
            .and_then(|candidate| candidate.matching_row_mut(root, &identity))
        {
            row.retain_observed_type(actual);
            return Ok(());
        }
        for attempt in &mut self.active_physical_call_attempts {
            if let Some(row) = attempt.transcript.matching_row_mut(root, &identity) {
                row.retain_observed_type(actual);
                return Ok(());
            }
        }
        Err(
            CandidateFactTransactionViolation::PhysicalCandidateEvaluationMissing {
                owner: identity.call_expression(),
            },
        )
    }

    /// Attach the selected lower type to its exact original physical
    /// observation. A different attempt at the same expression or a probe
    /// with the same prepared type cannot supply this observation.
    pub(super) fn complete_physical_observations<'a>(
        &mut self,
        selected: impl IntoIterator<Item = CompletedPhysicalCallSource<'a>>,
        control: super::FinalSemanticAnalysisControl<'_>,
    ) -> Result<(), super::FinalSemanticAnalysisError> {
        self.ensure_healthy()?;
        let mut remaining = BTreeMap::new();
        for (ordinal, completed) in selected.into_iter().enumerate() {
            control.check()?;
            let (owner, source, actual) = completed.into_parts();
            if remaining
                .insert(Arc::as_ptr(source), (ordinal, owner, actual))
                .is_some()
            {
                return Err(
                    CandidateFactTransactionViolation::PhysicalCandidateTypeObservationConflict {
                        owner,
                    }
                    .into(),
                );
            }
        }
        if remaining.is_empty() {
            return Ok(());
        }
        let transcript = &mut self.physical_candidate_argument_evaluations.rows;
        let mut matched = Vec::with_capacity(remaining.len());
        for (&root, rows) in transcript.iter() {
            for (row_index, row) in rows.iter().enumerate() {
                control.check()?;
                for (observation_index, source) in row.observation_sources().enumerate() {
                    control.check()?;
                    if let Some((_, owner, actual)) =
                        remaining.remove(&Arc::as_ptr(source.shared_prepared()))
                    {
                        if root != source.root()
                            || root != row.attempt().root()
                            || row.call_expression() != owner
                        {
                            return Err(CandidateFactTransactionViolation::PhysicalCandidateTypeObservationConflict { owner }.into());
                        }
                        matched.push((root, row_index, observation_index, actual));
                    }
                }
            }
        }
        if let Some((_, owner, _)) = remaining
            .into_values()
            .min_by_key(|(ordinal, _, _)| *ordinal)
        {
            return Err(
                CandidateFactTransactionViolation::PhysicalCandidateEvaluationMissing { owner }
                    .into(),
            );
        }
        control.check()?;
        for (root, row_index, observation_index, actual) in matched {
            transcript.get_mut(&root).expect("checked physical root")[row_index]
                .complete_observed_type(observation_index, Arc::clone(actual));
        }
        Ok(())
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
    fn begin_candidate_transaction<C: crate::effect_row::DecisionControl>(
        &mut self,
        control: &mut C,
    ) -> Result<CandidateFactCheckpoint, CandidateFactMutationFailure<C::Error>> {
        self.begin_candidate_transaction_with_effect_reads(None, control)
    }

    fn begin_candidate_transaction_with_effect_reads<C: crate::effect_row::DecisionControl>(
        &mut self,
        reads: Option<&crate::effect_row::prepared::PreparedEffectReadScope>,
        control: &mut C,
    ) -> Result<CandidateFactCheckpoint, CandidateFactMutationFailure<C::Error>> {
        self.ensure_healthy()?;
        let opens_root = self.candidate_checkpoints.is_empty();
        if opens_root
            != self
                .active_physical_candidate_argument_evaluations
                .is_none()
        {
            return Err(CandidateFactTransactionViolation::UnrecoverableLedger.into());
        }
        let id = CandidateFactCheckpointId(self.next_checkpoint_id);
        self.next_checkpoint_id = self
            .next_checkpoint_id
            .checked_add(1)
            .ok_or(CandidateFactTransactionViolation::SequenceExhausted)?;
        let graph = self
            .prepared_calls_mut()?
            .begin_delta_with_effect_reads(reads, control)
            .map_err(CandidateFactMutationFailure::from)?;
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
                violation: violation.into(),
                checkpoint: Box::new(checkpoint),
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
                violation: violation.into(),
                checkpoint: Box::new(CandidateFactCheckpoint {
                    issuer,
                    id,
                    journal_start,
                    epoch,
                    graph,
                }),
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
                violation: violation.into(),
                checkpoint: Box::new(checkpoint),
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
                violation: violation.into(),
                checkpoint: Box::new(CandidateFactCheckpoint {
                    issuer,
                    id,
                    journal_start,
                    epoch,
                    graph,
                }),
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
    fn extract_and_rollback<C: crate::effect_row::DecisionControl>(
        &mut self,
        checkpoint: CandidateFactCheckpoint,
        control: &mut C,
    ) -> Result<CandidateSemanticProjection, CandidateFactCloseFailure<C::Error>> {
        if let Err(violation) = self.validate_checkpoint(&checkpoint) {
            return Err(CandidateFactCloseFailure {
                violation: violation.into(),
                checkpoint: Box::new(checkpoint),
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
            .extract_delta(graph, control)
        {
            Ok(delta) => delta,
            Err(failure) => {
                let (violation, graph) = failure.into_parts();
                return Err(CandidateFactCloseFailure {
                    violation: violation.into(),
                    checkpoint: Box::new(CandidateFactCheckpoint {
                        issuer,
                        id,
                        journal_start,
                        epoch,
                        graph,
                    }),
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
        let implicit_capture_journal_order = self
            .implicit_capture_journal_order
            .iter()
            .copied()
            .filter(|key| owners.implicit_capture_uses.contains(key))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.implicit_capture_journal_order
            .retain(|key| !implicit_capture_journal_order.contains(key));
        let projection = CandidateSemanticProjection::from(CandidateSemanticProjectionData {
            authority: CandidateProjectionAuthority {
                issuer: Arc::clone(&self.issuer),
                epoch: self.epoch,
            },
            graph_delta,
            probe_effect_reads: None,
            facts: CandidateProjectionFacts {
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
                checked_content: owners
                    .checked_content
                    .into_iter()
                    .map(|content| (content, self.checked_content.take(content)))
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
                implicit_capture_journal_order,
                physical_candidate_argument_evaluations,
            },
        });
        self.rollback_journal(journal_start);
        if self.candidate_checkpoints.is_empty() {
            self.candidate_journal.clear();
        }
        Ok(projection)
    }

    pub(super) fn apply_candidate_projection<C: DecisionControl>(
        &mut self,
        target: &CandidateFactTransactionAuthority<'_>,
        projection: CandidateSemanticProjection,
        control: &mut C,
    ) -> Result<(), CandidateProjectionApplyFailure<C::Error>> {
        self.apply_candidate_projections(target, vec![projection], control)
            .map_err(|failure| {
                let (violation, mut projections) = failure.into_parts();
                CandidateProjectionApplyFailure {
                    violation,
                    projection: projections.pop().expect("original singleton projection"),
                }
            })
    }

    /// Applies separately extracted projections at one graph base. No fact
    /// is written until every original graph delta is admitted. The active
    /// checkpoint can roll back the resulting graph and fact updates together.
    pub(super) fn apply_candidate_projections<C: DecisionControl>(
        &mut self,
        target: &CandidateFactTransactionAuthority<'_>,
        projections: Vec<CandidateSemanticProjection>,
        control: &mut C,
    ) -> Result<(), CandidateProjectionApplyFailure<C::Error, Vec<CandidateSemanticProjection>>>
    {
        if let Err(violation) = self.preflight_candidate_projections(target, &projections, control)
        {
            return Err(CandidateProjectionApplyFailure {
                violation,
                projection: projections,
            });
        }
        let (graph_deltas, updates): (Vec<_>, Vec<_>) = projections
            .into_iter()
            .map(|projection| {
                let CandidateSemanticProjectionData {
                    authority,
                    graph_delta,
                    probe_effect_reads,
                    facts,
                } = *projection.data;
                (graph_delta, (authority, probe_effect_reads, facts))
            })
            .unzip();
        if let Err(failure) = self
            .prepared_calls
            .as_mut()
            .expect("prepared graph preflight")
            .restore_deltas(&target.checkpoint.graph, graph_deltas, control)
        {
            let (violation, graph_deltas) = failure.into_parts();
            let projections = graph_deltas
                .into_iter()
                .zip(updates)
                .map(|(graph_delta, (authority, probe_effect_reads, facts))| {
                    CandidateSemanticProjectionData {
                        authority,
                        graph_delta,
                        probe_effect_reads,
                        facts,
                    }
                    .into()
                })
                .collect();
            return Err(CandidateProjectionApplyFailure {
                violation: violation.into(),
                projection: projections,
            });
        }
        for (_, _, facts) in updates {
            self.apply_projection_entries(facts);
        }
        Ok(())
    }

    fn preflight_candidate_projections<C: DecisionControl>(
        &self,
        target: &CandidateFactTransactionAuthority<'_>,
        projections: &[CandidateSemanticProjection],
        control: &mut C,
    ) -> Result<(), CandidateFactMutationFailure<C::Error>> {
        self.transaction_authority(target.checkpoint)?;
        if projections.is_empty()
            || self
                .active_physical_candidate_argument_evaluations
                .is_none()
        {
            return Err(CandidateFactTransactionViolation::ProjectionOutsideTransaction.into());
        }
        let mut writes = CandidateProjectionOwners::default();
        let mut captures = BTreeMap::new();
        for projection in projections {
            self.require_candidate_projection_owner(projection)?;
            // Reserve the two vectors transferring graph deltas and fact
            // payloads separately. The original values are still in place.
            control
                .charge(DecisionWork::Node)
                .map_err(CandidateFactMutationFailure::Control)?;
            control
                .charge(DecisionWork::Node)
                .map_err(CandidateFactMutationFailure::Control)?;
            let facts = &projection.data.facts;
            if projections.len() > 1 {
                Self::admit_projection_writes(&mut writes.locals, facts.locals.keys(), control)?;
                Self::admit_projection_writes(
                    &mut writes.patterns,
                    facts.patterns.keys(),
                    control,
                )?;
                Self::admit_projection_writes(
                    &mut writes.expressions,
                    facts.expressions.keys(),
                    control,
                )?;
                Self::admit_projection_writes(
                    &mut writes.checked_content,
                    facts.checked_content.keys(),
                    control,
                )?;
                Self::admit_projection_writes(
                    &mut writes.iterations,
                    facts.iterations.keys(),
                    control,
                )?;
            }
            for (content, value) in &facts.checked_content {
                control
                    .charge(DecisionWork::Visit)
                    .map_err(CandidateFactMutationFailure::Control)?;
                if value
                    .as_ref()
                    .is_some_and(|value| !value.belongs_to(*content))
                {
                    return Err(CandidateFactTransactionViolation::ProjectionUnavailable.into());
                }
            }
            for (&key, &proposed) in &facts.implicit_capture_uses {
                control
                    .charge(DecisionWork::Visit)
                    .map_err(CandidateFactMutationFailure::Control)?;
                let live = self.implicit_capture_uses.get(&key).copied();
                if projections.len() > 1 && !captures.contains_key(&key) {
                    control
                        .charge(DecisionWork::Node)
                        .map_err(CandidateFactMutationFailure::Control)?;
                    captures.insert(key, proposed);
                }
                if let Some(existing) = captures.get(&key).copied()
                    && existing != proposed
                {
                    if let (Some(existing), Some(proposed)) = (existing, proposed) {
                        return Err(
                            CandidateFactTransactionViolation::ImplicitCaptureUseConflict {
                                callable: key.0,
                                site: key.1,
                                existing,
                                proposed,
                            }
                            .into(),
                        );
                    }
                    return Err(CandidateFactTransactionViolation::ProjectionWriteConflict.into());
                }
                if let (Some(existing), Some(proposed)) = (live, proposed)
                    && existing != proposed
                {
                    return Err(
                        CandidateFactTransactionViolation::ImplicitCaptureUseConflict {
                            callable: key.0,
                            site: key.1,
                            existing,
                            proposed,
                        }
                        .into(),
                    );
                }
                if proposed.is_none()
                    && live.is_some()
                    && !self.implicit_capture_journal_order.contains(&key)
                {
                    return Err(CandidateFactTransactionViolation::UnrecoverableLedger.into());
                }
            }
        }
        self.preflight_projection_call_graph(projections)?;
        Ok(())
    }

    fn admit_projection_writes<'a, K: Copy + Ord + 'a, C: DecisionControl>(
        seen: &mut BTreeSet<K>,
        keys: impl Iterator<Item = &'a K>,
        control: &mut C,
    ) -> Result<(), CandidateFactMutationFailure<C::Error>> {
        for &key in keys {
            control
                .charge(DecisionWork::Node)
                .map_err(CandidateFactMutationFailure::Control)?;
            if !seen.insert(key) {
                return Err(CandidateFactTransactionViolation::ProjectionWriteConflict.into());
            }
        }
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
        self.require_candidate_projection_owner(&projection)?;
        self.prepared_calls()?
            .validate_delta(&projection.data.graph_delta)
            .map_err(|violation| {
                CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
            })?;
        drop(projection);
        Ok(())
    }

    pub(super) fn discard_candidate_projections(
        &self,
        projections: Vec<CandidateSemanticProjection>,
    ) -> Result<(), CandidateFactTransactionViolation> {
        projections
            .into_iter()
            .try_for_each(|projection| self.discard_candidate_projection(projection))
    }

    /// Retain actual graph writes as a separate projection in the same
    /// publication. A fact-only transfer needs no detached graph read permit.
    pub(super) fn stage_original_probe(
        &self,
        materialized: &mut CandidateSemanticProjection,
        mut original: CandidateSemanticProjection,
        owner: ExprId,
        completed: RetainedProbeTypes,
    ) -> Result<Option<CandidateSemanticProjection>, CandidateFactTransactionViolation> {
        self.require_candidate_projection_owner(materialized)?;
        self.require_candidate_projection_owner(&original)?;
        self.prepared_calls()?
            .validate_delta(&original.data.graph_delta)
            .map_err(|error| CandidateFactTransactionViolation::PreparedCallGraph(error.into()))?;
        if original.retained_expression_value(owner).is_none() {
            return Err(CandidateFactTransactionViolation::ProjectionUnavailable);
        }
        let source = &mut original.data.facts;
        for (owner, ty) in completed.bindings {
            match owner {
                HirSemanticPathOwnerId::Local(owner) => {
                    let Some(Some(value)) = source.locals.get_mut(&owner) else {
                        return Err(CandidateFactTransactionViolation::ProjectionUnavailable);
                    };
                    *value = ty;
                }
                HirSemanticPathOwnerId::Pattern(owner) => {
                    let Some(Some(value)) = source.patterns.get_mut(&owner) else {
                        return Err(CandidateFactTransactionViolation::ProjectionUnavailable);
                    };
                    *value = ty;
                }
                _ => return Err(CandidateFactTransactionViolation::ProjectionUnavailable),
            }
        }
        for (owner, update) in completed.expressions {
            let Some(Some(value)) = source.expressions.get_mut(&owner) else {
                return Err(CandidateFactTransactionViolation::ProjectionUnavailable);
            };
            if !value.apply_type_updates(update) {
                return Err(CandidateFactTransactionViolation::ProjectionUnavailable);
            }
        }
        if original.data.graph_delta.has_writes() {
            return Ok(Some(original));
        }
        let target = &mut materialized.data.facts;
        if source
            .locals
            .keys()
            .any(|owner| target.locals.contains_key(owner))
            || source
                .patterns
                .keys()
                .any(|owner| target.patterns.contains_key(owner))
            || source
                .expressions
                .keys()
                .any(|owner| target.expressions.contains_key(owner))
        {
            return Err(CandidateFactTransactionViolation::ProjectionWriteConflict);
        }
        target.locals.append(&mut source.locals);
        target.patterns.append(&mut source.patterns);
        target.expressions.append(&mut source.expressions);
        Ok(None)
    }

    /// A materialization callback borrows the original probe's facts before
    /// using them. The same ledger and epoch must own that preparation.
    pub(super) fn require_candidate_projection_owner(
        &self,
        projection: &CandidateSemanticProjection,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if !Arc::ptr_eq(&self.issuer, &projection.data.authority.issuer) {
            return Err(CandidateFactTransactionViolation::ForeignProjection);
        }
        if projection.data.authority.epoch != self.epoch {
            return Err(CandidateFactTransactionViolation::ProjectionAuthorityMismatch);
        }
        Ok(())
    }

    pub(super) fn retain_probe_effect_reads<C: DecisionControl>(
        &self,
        projection: &mut CandidateSemanticProjection,
        control: &mut C,
    ) -> Result<(), CandidateFactMutationFailure<C::Error>> {
        self.require_candidate_projection_owner(projection)?;
        if projection.data.probe_effect_reads.is_some() {
            return Err(CandidateFactTransactionViolation::ProjectionAuthorityMismatch.into());
        }
        let effects = self
            .prepared_calls()?
            .retain_probe_effect_reads(&projection.data.graph_delta, control)?;
        projection.data.probe_effect_reads = Some(effects);
        Ok(())
    }

    pub(super) fn set_local_type(
        &mut self,
        owner: LocalId,
        value: TypeKind<crate::types::PreparedEffects>,
    ) -> Result<bool, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        Ok(self.set_local_type_unchecked(owner, value))
    }

    fn set_local_type_unchecked(
        &mut self,
        owner: LocalId,
        value: TypeKind<crate::types::PreparedEffects>,
    ) -> bool {
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
        value: TypeKind<crate::types::PreparedEffects>,
    ) -> Result<bool, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        Ok(self.set_pattern_type_unchecked(owner, value))
    }

    fn set_pattern_type_unchecked(
        &mut self,
        owner: PatternId,
        value: TypeKind<crate::types::PreparedEffects>,
    ) -> bool {
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
        value: PreparedIteration,
    ) -> Result<bool, CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        Ok(self.set_iteration_fact_unchecked(owner, value))
    }

    fn set_iteration_fact_unchecked(&mut self, owner: ExprId, value: PreparedIteration) -> bool {
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
        site: impl Into<super::super::CheckedLocalUseSite>,
        local: LocalId,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        let site = site.into();
        let key = (callable, site);
        if let Some(existing) = self.implicit_capture_uses.get(&key) {
            if *existing == local {
                return Ok(());
            }
            return Err(
                CandidateFactTransactionViolation::ImplicitCaptureUseConflict {
                    callable,
                    site,
                    existing: *existing,
                    proposed: local,
                },
            );
        }
        let previous = self.implicit_capture_uses.insert(key, local);
        debug_assert!(previous.is_none());
        let order_index = self.implicit_capture_journal_order.len();
        self.implicit_capture_journal_order.push(key);
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::ImplicitCaptureUse {
                    key,
                    previous,
                    order_index,
                });
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
        let expression_mismatch = self.expressions.iter().find(|(owner, expression)| {
            expression
                .checked_call_site(**owner)
                .is_some_and(|site| graph.site_state(site).is_none())
        });
        let graph_mismatch = graph.sites().find(|site| {
            let site = *site;
            let owner = match site {
                CheckedCallSite::HirCall(owner)
                | CheckedCallSite::AttachedContentApplication {
                    expression: owner, ..
                } => owner,
            };
            self.expressions
                .get(&owner)
                .and_then(|expression| expression.checked_call_site(owner))
                != Some(site)
        });
        if expression_mismatch.is_some() || graph_mismatch.is_some() {
            return Err(Self::call_graph_mismatch());
        }
        Ok(())
    }

    fn preflight_projection_call_graph(
        &self,
        projections: &[CandidateSemanticProjection],
    ) -> Result<(), CandidateFactTransactionViolation> {
        let graph = self.prepared_calls()?;
        let mut delta_sites = Vec::new();
        let mut expression_updates = BTreeMap::new();
        for projection in projections {
            let expressions = &projection.data.facts.expressions;
            for &(site, _) in &*projection
                .data
                .graph_delta
                .site_states()
                .map_err(|violation| {
                    CandidateFactTransactionViolation::PreparedCallGraph(violation.into())
                })?
            {
                let owner = site.expression();
                if expressions
                    .get(&owner)
                    .and_then(Option::as_ref)
                    .and_then(|fact| fact.checked_call_site(owner))
                    != Some(site)
                {
                    return Err(Self::call_graph_mismatch());
                }
                delta_sites.push(site);
            }
            expression_updates.extend(expressions.iter().map(|(&owner, expression)| {
                let site = expression
                    .as_ref()
                    .and_then(|fact| fact.checked_call_site(owner));
                (owner, site)
            }));
        }
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
        if projected_call_site_inventories_match(
            graph_sites,
            delta_sites,
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

    /// Discards a candidate-resolution-only expression after the selected
    /// semantic channel has established that the source is not an owned
    /// expression in the final graph.
    ///
    /// Call-bearing expressions cannot be discarded here because their fact
    /// and prepared-call graph node form one transaction. Static ContentCall
    /// callees are the motivating owner: the selected call target/schema is
    /// final authority, while the authored callee spelling is not a content
    /// child and therefore must not escape as a parallel expression fact.
    pub(super) fn discard_provisional_expression(
        &mut self,
        owner: ExprId,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        if self
            .expressions
            .get(&owner)
            .is_some_and(|expression| expression.checked_call_site(owner).is_some())
        {
            return Err(Self::call_graph_mismatch());
        }
        self.remove_expression(owner);
        Ok(())
    }

    pub(super) fn publish_checked_content(
        &mut self,
        owner: ExprId,
        checked: PreparedCheckedRichTextCheck,
    ) -> Result<(), CandidateFactTransactionViolation> {
        self.ensure_healthy()?;
        let content = checked.content_id();
        if content.owner() != owner || self.checked_content.contains(content) {
            return Err(CandidateFactTransactionViolation::ProjectionUnavailable);
        }
        self.checked_content
            .insert(checked)
            .map_err(|_| CandidateFactTransactionViolation::ProjectionUnavailable)?;
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::CheckedContent {
                    content,
                    previous: None,
                });
        }
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

    /// Atomically installs a complete owner-bound replacement batch. All
    /// existence, health, and call-site checks happen before the first map
    /// mutation, so a malformed replacement cannot publish a partial seal.
    pub(super) fn replace_existing_expressions(
        &mut self,
        replacements: BTreeMap<ExprId, PreparedExpressionFact>,
    ) -> Result<(), ExpressionFactWriteViolation> {
        self.ensure_healthy()
            .map_err(ExpressionFactWriteViolation::Candidate)?;
        if replacements
            .keys()
            .any(|owner| !self.expressions.contains_key(owner))
        {
            return Err(ExpressionFactWriteViolation::MissingPublishedFact);
        }
        for (owner, value) in &replacements {
            self.validate_call_expression_write(*owner, value)
                .map_err(ExpressionFactWriteViolation::Candidate)?;
        }
        for (owner, value) in replacements {
            self.write_expression(owner, value);
        }
        Ok(())
    }

    fn write_expression(&mut self, owner: ExprId, value: PreparedExpressionFact) {
        let previous = self.expressions.insert(owner, value);
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::Expression { owner, previous });
        }
    }

    fn remove_expression(&mut self, owner: ExprId) {
        let previous = self.expressions.remove(&owner);
        if previous.is_some() && !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::Expression { owner, previous });
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
        let order_index = if previous.is_some() {
            let Some(index) = self
                .implicit_capture_journal_order
                .iter()
                .position(|existing| *existing == key)
            else {
                self.poison = Some(CandidateFactTransactionViolation::UnrecoverableLedger);
                return;
            };
            self.implicit_capture_journal_order.remove(index);
            Some(index)
        } else {
            None
        };
        if previous.is_some() && !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::ImplicitCaptureUse {
                    key,
                    previous,
                    order_index: order_index.expect("present capture has an order index"),
                });
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
        let violation = CandidateFactTransactionViolation::from(violation);
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
                SemanticFactMutation::CheckedContent { content, .. } => {
                    owners.checked_content.insert(*content);
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
                    restore_map_entry(&mut self.expressions, owner, previous);
                }
                SemanticFactMutation::CheckedContent { content, previous } => {
                    if self
                        .checked_content
                        .replace(content, previous.map(|previous| *previous))
                        .is_err()
                    {
                        self.poison =
                            Some(CandidateFactTransactionViolation::ProjectionUnavailable);
                    }
                }
                SemanticFactMutation::Iteration { owner, previous } => {
                    restore_map_entry(
                        &mut self.iteration_facts,
                        owner,
                        previous.map(|previous| *previous),
                    );
                }
                SemanticFactMutation::ImplicitCaptureUse {
                    key,
                    previous,
                    order_index,
                } => {
                    restore_map_entry(&mut self.implicit_capture_uses, key, previous);
                    if previous.is_some() {
                        self.implicit_capture_journal_order.insert(
                            order_index.min(self.implicit_capture_journal_order.len()),
                            key,
                        );
                    } else if self
                        .implicit_capture_journal_order
                        .get(order_index)
                        .is_some_and(|existing| *existing == key)
                    {
                        self.implicit_capture_journal_order.remove(order_index);
                    }
                }
            }
        }
    }

    fn apply_projection_entries(&mut self, facts: CandidateProjectionFacts) {
        let CandidateProjectionFacts {
            locals,
            patterns,
            expressions,
            checked_content,
            iterations,
            mut implicit_capture_uses,
            implicit_capture_journal_order,
            physical_candidate_argument_evaluations,
        } = facts;
        self.active_physical_candidate_argument_evaluations
            .as_mut()
            .expect("active transcript preflight")
            .merge(physical_candidate_argument_evaluations);
        for (owner, value) in locals {
            self.apply_local(owner, value);
        }
        for (owner, value) in patterns {
            self.apply_pattern(owner, value);
        }
        for (owner, value) in expressions {
            self.apply_expression(owner, value);
        }
        for (content, value) in checked_content {
            self.apply_checked_content(content, value);
        }
        for (owner, value) in iterations {
            self.apply_iteration(owner, value);
        }
        for key in implicit_capture_journal_order {
            if let Some(value) = implicit_capture_uses.remove(&key) {
                self.apply_implicit_capture_use(key, value);
            }
        }
        for (key, value) in implicit_capture_uses {
            self.apply_implicit_capture_use(key, value);
        }
    }

    fn apply_local(
        &mut self,
        owner: LocalId,
        value: Option<TypeKind<crate::types::PreparedEffects>>,
    ) {
        if let Some(value) = value {
            self.set_local_type_unchecked(owner, value);
        } else {
            self.remove_local_type(owner);
        }
    }

    fn apply_pattern(
        &mut self,
        owner: PatternId,
        value: Option<TypeKind<crate::types::PreparedEffects>>,
    ) {
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

    fn apply_checked_content(
        &mut self,
        content: arcweft_lang_hir::dialogue_application::HirDialogueContentId,
        value: Option<PreparedCheckedRichTextCheck>,
    ) {
        let previous = self
            .checked_content
            .replace(content, value)
            .expect("checked-content projection preflight");
        if !self.candidate_checkpoints.is_empty() {
            self.candidate_journal
                .push(SemanticFactMutation::CheckedContent {
                    content,
                    previous: previous.map(Box::new),
                });
        }
    }

    fn apply_iteration(&mut self, owner: ExprId, value: Option<PreparedIteration>) {
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
                let order_index = self.implicit_capture_journal_order.len();
                self.implicit_capture_journal_order.push(key);
                if !self.candidate_checkpoints.is_empty() {
                    self.candidate_journal
                        .push(SemanticFactMutation::ImplicitCaptureUse {
                            key,
                            previous,
                            order_index,
                        });
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
        mut control: super::preparation_work::AnalyzerPreparationWork<'_>,
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
            .begin_candidate_transaction(&mut control)
            .map_err(|failure| match failure {
                CandidateFactMutationFailure::Control(error) => {
                    AnalyzerExpressionError::fatal(error)
                }
                CandidateFactMutationFailure::Invariant(error) => {
                    AnalyzerExpressionError::fact(error)
                }
            })?;
        let expression_authority = match self.facts.candidate_expression_authority(&checkpoint) {
            Ok(authority) => authority,
            Err(error) => return Err(self.rollback_candidate_authority_error(checkpoint, error)),
        };
        let transaction_authority = match self.facts.transaction_authority(&checkpoint) {
            Ok(authority) => authority,
            Err(error) => return Err(self.rollback_candidate_authority_error(checkpoint, error)),
        };
        let result = operation(self, expression_authority, transaction_authority);
        self.finish_candidate_fact_transaction(checkpoint, result, &mut control)
    }

    fn rollback_candidate_authority_error(
        &mut self,
        checkpoint: CandidateFactCheckpoint,
        error: CandidateFactTransactionViolation,
    ) -> super::expression_error::AnalyzerExpressionError {
        use super::expression_error::AnalyzerExpressionError;
        match self.facts.rollback_candidate_transaction(checkpoint) {
            Ok(()) => AnalyzerExpressionError::fact(error),
            Err(failure) => {
                AnalyzerExpressionError::fact(self.facts.abort_after_close_failure(failure))
            }
        }
    }

    fn finish_candidate_fact_transaction<C, T, E>(
        &mut self,
        checkpoint: CandidateFactCheckpoint,
        result: Result<CandidateFactTransactionAction<T>, E>,
        control: &mut C,
    ) -> Result<CandidateFactTransactionOutcome<T>, super::expression_error::AnalyzerExpressionError>
    where
        E: Into<CandidateFactOperationFailure>,
        C: crate::effect_row::DecisionControl<
                Error = crate::final_analysis::FinalSemanticAnalysisError,
            >,
    {
        use super::expression_error::AnalyzerExpressionError;
        let action = match result {
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
                match self.facts.extract_and_rollback(checkpoint, control) {
                    Ok(projection) => {
                        Ok(CandidateFactTransactionOutcome::Extracted { value, projection })
                    }
                    Err(failure) => Err(
                        match self.facts.recover_callback_scope_close_failure(failure) {
                            Ok(error) => AnalyzerExpressionError::fatal(error),
                            Err(violation) => AnalyzerExpressionError::fact(violation),
                        },
                    ),
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

    struct ProjectionWork(usize);

    impl DecisionControl for ProjectionWork {
        type Error = &'static str;

        fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
            self.0 = self.0.checked_sub(1).ok_or("projection budget exhausted")?;
            Ok(())
        }
    }

    fn work() -> ProjectionWork {
        ProjectionWork(100_000)
    }
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
            .analysis_view()
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
            .analysis_view()
            .expect("executable HIR")
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .expect("root module");
        let mut locals = module.locals().map(|(owner, _)| owner);
        let first = locals.next().expect("first local");
        let second = locals.next().expect("second local");
        (first, second)
    }

    fn local_projection(
        state: &mut SemanticFactState,
        local: LocalId,
        value: TypeKind<crate::types::PreparedEffects>,
    ) -> CandidateSemanticProjection {
        let checkpoint = state.begin_candidate_transaction(&mut work()).unwrap();
        state.set_local_type(local, value).unwrap();
        state.extract_and_rollback(checkpoint, &mut work()).unwrap()
    }

    #[test]
    fn graph_budget_failure_returns_original_fact_batch_for_apply_and_rollback() {
        let (first, second) = distinct_test_locals();
        let mut state = SemanticFactState::new();
        let projections = vec![
            local_projection(&mut state, first, TypeKind::I64),
            local_projection(&mut state, second, TypeKind::Unit),
        ];
        let target = state.begin_candidate_transaction(&mut work()).unwrap();
        let authority = state.transaction_authority(&target).unwrap();
        let mut fact_budget = work();
        state
            .preflight_candidate_projections(&authority, &projections, &mut fact_budget)
            .unwrap();
        let (error, projections) = state
            .apply_candidate_projections(
                &authority,
                projections,
                &mut ProjectionWork(work().0 - fact_budget.0),
            )
            .unwrap_err()
            .into_parts();
        assert!(matches!(error, CandidateFactMutationFailure::Control(_)));
        assert!(state.locals().is_empty());
        assert!(state.candidate_journal.is_empty());
        assert_eq!(
            projections[0].data.facts.locals.get(&first),
            Some(&Some(TypeKind::I64))
        );
        assert_eq!(
            projections[1].data.facts.locals.get(&second),
            Some(&Some(TypeKind::Unit))
        );
        state
            .apply_candidate_projections(&authority, projections, &mut work())
            .unwrap();
        assert_eq!(state.locals().get(&first), Some(&TypeKind::I64));
        assert_eq!(state.locals().get(&second), Some(&TypeKind::Unit));
        state.rollback_candidate_transaction(target).unwrap();
        assert!(state.locals().is_empty());
    }

    #[test]
    fn a_child_fact_projection_applies_with_the_retained_expression_and_its_effects() {
        use crate::effect_row::{
            EffectPredicate, EffectRow,
            prepared::{PreparedEffectInvariant, PreparedEffectRow},
        };

        let fixture = crate::final_analysis::tests::fixture(
            "fn caller(value: i64, other: i64) { value; }\n",
            None,
        );
        let module = fixture
            .project
            .analysis_view()
            .unwrap()
            .module(&arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root())
            .unwrap();
        let parent_expression = module.expressions().next().unwrap().0;
        let mut locals = module.locals().map(|(owner, _)| owner);
        let parent_local = locals.next().unwrap();
        let child_local = locals.next().unwrap();
        let parent_pattern = module.patterns().next().unwrap().0;
        let mut state = SemanticFactState::new();
        let mut work = work();
        let declarations =
            crate::callable::PreparedCallableEffectDeclarations::declare([], &mut work).unwrap();
        state.initialize_effect_declarations(declarations).unwrap();
        let parent_checkpoint = state.begin_candidate_transaction(&mut work).unwrap();
        let (scheme, row) = {
            let (graph, _) = state.effect_declarations_mut().unwrap().relation();
            let scheme = graph
                .declare(vec![], EffectPredicate::unconstrained(), &mut work)
                .unwrap();
            let row = graph.application(&scheme, vec![], &mut work).unwrap();
            (scheme, row)
        };
        state
            .publish_new_expression(
                parent_expression,
                super::super::super::prepared::PreparedResolvedExpression::value(
                    TypeKind::I64,
                    super::super::super::CheckedTypeSelection::Inferred,
                    crate::effects::EffectSet::new(),
                    super::super::super::CheckedExpressionResolution::Structural,
                ),
            )
            .unwrap();
        state.set_local_type(parent_local, TypeKind::I64).unwrap();
        state
            .set_pattern_type(parent_pattern, TypeKind::I64)
            .unwrap();
        let child_checkpoint = state.begin_candidate_transaction(&mut work).unwrap();
        state
            .effect_declarations_mut()
            .unwrap()
            .relation()
            .0
            .prepare_body(
                &scheme,
                PreparedEffectRow::Canonical(EffectRow::closed(crate::effects::EffectSet::new())),
                vec![],
                &mut work,
            )
            .unwrap();
        state.set_local_type(child_local, TypeKind::Unit).unwrap();
        let child = state
            .extract_and_rollback(child_checkpoint, &mut work)
            .unwrap();
        state
            .effect_declarations_mut()
            .unwrap()
            .relation()
            .0
            .choice(&mut work)
            .unwrap();
        let parent = state
            .extract_and_rollback(parent_checkpoint, &mut work)
            .unwrap();

        let target = state.begin_candidate_transaction(&mut work).unwrap();
        let authority = state.transaction_authority(&target).unwrap();
        let (error, mut child) = state
            .apply_candidate_projection(&authority, child, &mut work)
            .unwrap_err()
            .into_parts();
        assert!(matches!(
            error,
            CandidateFactMutationFailure::Invariant(
                CandidateFactTransactionViolation::PreparedCallGraph(_)
            )
        ));
        assert!(state.locals().is_empty());
        assert!(state.expressions().is_empty());
        let row_status = |state: &SemanticFactState| {
            state
                .prepared_calls()
                .unwrap()
                .effect_declarations()
                .unwrap()
                .read()
                .validate_retained_row(&row)
        };
        assert_eq!(
            row_status(&state),
            Err(PreparedEffectInvariant::StaleReference)
        );

        let retained = state
            .stage_original_probe(
                &mut child,
                parent,
                parent_expression,
                RetainedProbeTypes::unchanged_for_test(),
            )
            .unwrap()
            .expect("the effect writes keep their original projection");
        assert!(child.expression_update(parent_expression).is_none());
        state
            .apply_candidate_projections(&authority, vec![child, retained], &mut work)
            .unwrap();
        assert_eq!(
            state.expressions()[&parent_expression].value_type(),
            Some(&TypeKind::I64)
        );
        assert_eq!(state.locals().get(&child_local), Some(&TypeKind::Unit));
        assert_eq!(state.locals().get(&parent_local), Some(&TypeKind::I64));
        assert_eq!(state.patterns().get(&parent_pattern), Some(&TypeKind::I64));
        row_status(&state).unwrap();
        state.rollback_candidate_transaction(target).unwrap();
        assert!(state.locals().is_empty());
        assert!(state.expressions().is_empty());
        assert!(state.patterns().is_empty());
        assert_eq!(
            row_status(&state),
            Err(PreparedEffectInvariant::StaleReference)
        );
    }

    #[test]
    fn competing_fact_writes_leave_the_ledger_and_both_projections_intact() {
        let (local, _) = distinct_test_locals();
        let mut state = SemanticFactState::new();
        let projections = vec![
            local_projection(&mut state, local, TypeKind::I64),
            local_projection(&mut state, local, TypeKind::Unit),
        ];
        let target = state.begin_candidate_transaction(&mut work()).unwrap();
        let authority = state.transaction_authority(&target).unwrap();
        let (error, mut projections) = state
            .apply_candidate_projections(&authority, projections, &mut work())
            .unwrap_err()
            .into_parts();
        assert!(matches!(
            error,
            CandidateFactMutationFailure::Invariant(
                CandidateFactTransactionViolation::ProjectionWriteConflict
            )
        ));
        assert!(state.locals().is_empty());
        let second = projections.pop().unwrap();
        assert_eq!(
            projections[0].data.facts.locals.get(&local),
            Some(&Some(TypeKind::I64))
        );
        state
            .apply_candidate_projection(&authority, second, &mut work())
            .unwrap();
        assert_eq!(state.locals().get(&local), Some(&TypeKind::Unit));
        state.rollback_candidate_transaction(target).unwrap();
        assert!(state.locals().is_empty());
    }

    fn graph_sites() -> Vec<CheckedCallSite> {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller(value: i64) { consume(value); }\nfn consume(value: i64) -> i64 { value }\n",
            None,
        );
        let module = fixture
            .project
            .analysis_view()
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
            BTreeMap::from([(
                owner,
                Some(CheckedCallSite::AttachedContentApplication {
                    expression: owner,
                    family: crate::callable::CheckedAttachedContentApplicationFamily::DialogueLine,
                }),
            )]),
        ));
    }

    #[test]
    fn prepared_call_graph_site_seal_is_exactly_idempotent() {
        let site = graph_sites()[0];
        let mut graph = PreparedCallGraph::<(), u8>::new();
        let checkpoint = graph.begin_delta(&mut work()).expect("graph checkpoint");
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
        let outer = graph
            .begin_delta(&mut work())
            .expect("outer graph checkpoint");
        graph
            .seal_unselected(sites[0], no_graph_dependencies(), ())
            .expect("outer node");
        let inner = graph
            .begin_delta(&mut work())
            .expect("inner graph checkpoint");
        graph
            .seal_unselected(sites[1], no_graph_dependencies(), ())
            .expect("inner node");
        graph.commit_delta(inner).expect("nested commit");
        graph.rollback_delta(outer).expect("outer rollback");

        let source = graph.begin_delta(&mut work()).expect("source checkpoint");
        graph
            .seal_unselected(sites[0], no_graph_dependencies(), ())
            .expect("source node");
        let delta = graph
            .extract_delta(source, &mut work())
            .expect("extract source");

        let outer = graph
            .begin_delta(&mut work())
            .expect("restore outer checkpoint");
        graph
            .seal_unselected(sites[1], no_graph_dependencies(), ())
            .expect("outer baseline node");
        graph
            .restore_deltas(&outer, vec![delta], &mut work())
            .expect("restore into outer");
        graph
            .rollback_delta(outer)
            .expect("rollback restored nodes");

        let source = graph
            .begin_delta(&mut work())
            .expect("collision source checkpoint");
        graph
            .seal_unselected(sites[2], no_graph_dependencies(), ())
            .expect("collision source node");
        let delta = graph
            .extract_delta(source, &mut work())
            .expect("extract collision source");
        let outer = graph
            .begin_delta(&mut work())
            .expect("collision outer checkpoint");
        graph
            .seal_unselected(sites[2], no_graph_dependencies(), ())
            .expect("collision node");
        let failure = graph
            .restore_deltas(&outer, vec![delta], &mut work())
            .expect_err("restore preflight must reject duplicate site");
        let (_, deltas) = failure.into_parts();
        graph.rollback_delta(outer).expect("rollback collision");
        let target = graph
            .begin_delta(&mut work())
            .expect("retry target checkpoint");
        graph
            .restore_deltas(&target, deltas, &mut work())
            .expect("retry exact delta");
        graph.rollback_delta(target).expect("rollback retry target");
    }

    #[test]
    fn prepared_call_graph_restore_requires_exact_active_target() {
        let sites = graph_sites();
        let mut graph = PreparedCallGraph::<()>::new();
        let source = graph.begin_delta(&mut work()).expect("source checkpoint");
        graph
            .seal_unselected(sites[0], no_graph_dependencies(), ())
            .expect("source node");
        let delta = graph
            .extract_delta(source, &mut work())
            .expect("extract source");

        let outer = graph.begin_delta(&mut work()).expect("outer target");
        let inner = graph.begin_delta(&mut work()).expect("inner target");
        let failure = graph
            .restore_deltas(&outer, vec![delta], &mut work())
            .expect_err("non-LIFO target must reject restore");
        let (violation, deltas) = failure.into_parts();
        assert!(matches!(
            violation,
            crate::callable::PreparedCallGraphMutationFailure::Invariant(
                CallConstraintInvariant::PreparedGraphDeltaOrder
            )
        ));
        graph.rollback_delta(inner).expect("close inner target");
        graph
            .restore_deltas(&outer, deltas, &mut work())
            .expect("outer target is now exact top");
        graph.rollback_delta(outer).expect("rollback restored node");
    }

    #[test]
    fn non_lifo_checkpoint_failure_returns_the_token_and_preserves_ledger() {
        let mut state = SemanticFactState::new();
        let outer = state
            .begin_candidate_transaction(&mut work())
            .expect("outer checkpoint");
        let inner = state
            .begin_candidate_transaction(&mut work())
            .expect("inner checkpoint");

        let failure = state
            .commit_candidate_transaction(outer)
            .expect_err("outer checkpoint is not LIFO");
        let (violation, outer) = failure.into_parts();
        assert!(matches!(
            violation,
            CandidateFactMutationFailure::Invariant(
                CandidateFactTransactionViolation::NonLifoCheckpoint
            )
        ));
        assert!(state.rollback_candidate_transaction(inner).is_ok());
        assert!(state.rollback_candidate_transaction(outer).is_ok());
    }

    #[test]
    fn foreign_checkpoint_failure_returns_the_token_to_its_issuer() {
        let mut origin = SemanticFactState::new();
        let checkpoint = origin
            .begin_candidate_transaction(&mut work())
            .expect("checkpoint");
        let mut foreign = SemanticFactState::new();
        let failure = foreign
            .rollback_candidate_transaction(checkpoint)
            .expect_err("foreign checkpoint is rejected");
        let (violation, checkpoint) = failure.into_parts();
        assert!(matches!(
            violation,
            CandidateFactMutationFailure::Invariant(
                CandidateFactTransactionViolation::ForeignCheckpoint
            )
        ));
        assert!(origin.rollback_candidate_transaction(checkpoint).is_ok());
    }

    #[test]
    fn stale_checkpoint_is_distinct_from_foreign_and_non_lifo() {
        let mut origin = SemanticFactState::new();
        let checkpoint = origin
            .begin_candidate_transaction(&mut work())
            .expect("checkpoint");
        let mut same_issuer_empty_ledger = SemanticFactState::new();
        same_issuer_empty_ledger.issuer = Arc::clone(&origin.issuer);
        let failure = same_issuer_empty_ledger
            .rollback_candidate_transaction(checkpoint)
            .expect_err("checkpoint is absent from this issuer ledger");
        let (violation, checkpoint) = failure.into_parts();
        assert!(matches!(
            violation,
            CandidateFactMutationFailure::Invariant(
                CandidateFactTransactionViolation::StaleCheckpoint
            )
        ));
        assert!(origin.rollback_candidate_transaction(checkpoint).is_ok());
    }

    #[test]
    fn foreign_projection_failure_returns_the_projection_to_its_issuer() {
        let mut origin = SemanticFactState::new();
        let probe = origin
            .begin_candidate_transaction(&mut work())
            .expect("probe");
        let projection = origin
            .extract_and_rollback(probe, &mut work())
            .unwrap_or_else(|_| panic!("origin projection extraction"));
        let mut foreign = SemanticFactState::new();
        let foreign_transaction = foreign
            .begin_candidate_transaction(&mut work())
            .expect("foreign transaction");
        let foreign_authority = foreign
            .transaction_authority(&foreign_transaction)
            .expect("foreign authority");
        let failure = foreign
            .apply_candidate_projection(&foreign_authority, projection, &mut work())
            .expect_err("foreign projection is rejected");
        let (violation, projection) = failure.into_parts();
        assert!(matches!(
            violation,
            CandidateFactMutationFailure::Invariant(
                CandidateFactTransactionViolation::ForeignProjection
            )
        ));
        drop(foreign_authority);
        assert!(
            foreign
                .rollback_candidate_transaction(foreign_transaction)
                .is_ok()
        );
        let origin_transaction = origin
            .begin_candidate_transaction(&mut work())
            .expect("origin transaction");
        let origin_authority = origin
            .transaction_authority(&origin_transaction)
            .expect("origin authority");
        assert!(
            origin
                .apply_candidate_projection(&origin_authority, projection, &mut work())
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
        let probe = state
            .begin_candidate_transaction(&mut work())
            .expect("probe");
        let projection = state
            .extract_and_rollback(probe, &mut work())
            .unwrap_or_else(|_| panic!("projection extraction"));
        let transaction = state
            .begin_candidate_transaction(&mut work())
            .expect("transaction");
        let authority = state
            .transaction_authority(&transaction)
            .expect("target authority");
        assert!(
            state
                .apply_candidate_projection(&authority, projection, &mut work())
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
            state.begin_candidate_transaction(&mut work()),
            Err(CandidateFactMutationFailure::Invariant(
                CandidateFactTransactionViolation::SequenceExhausted
            ))
        ));
    }

    #[test]
    fn implicit_capture_use_is_discarded_by_candidate_rollback() {
        let (call, local, _) = call_identity();
        let callable = call;
        let expression = callable;
        let mut state = SemanticFactState::new();
        let checkpoint = state
            .begin_candidate_transaction(&mut work())
            .expect("checkpoint");

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
        let probe = state
            .begin_candidate_transaction(&mut work())
            .expect("probe");

        state
            .record_implicit_capture_use(callable, expression, local)
            .expect("capture-use row");
        let projection = state
            .extract_and_rollback(probe, &mut work())
            .expect("extract projection");
        assert!(state.pending_implicit_capture_uses().is_empty());

        let outer = state
            .begin_candidate_transaction(&mut work())
            .expect("outer");
        let authority = state
            .transaction_authority(&outer)
            .expect("outer authority");
        state
            .apply_candidate_projection(&authority, projection, &mut work())
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
        let checkpoint = state
            .begin_candidate_transaction(&mut work())
            .expect("checkpoint");

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
                site: expression.into(),
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

    #[test]
    fn implicit_capture_ledger_drains_affinely_in_event_order_for_exact_owners() {
        let (callable, first_local, _) = call_identity();
        let (_, second_local) = distinct_test_locals();
        let sites = graph_sites();
        let first_expression = sites[0].expression();
        let second_expression = sites[1].expression();
        assert_ne!(first_expression, second_expression);

        let mut state = SemanticFactState::new();
        state
            .record_implicit_capture_use(callable, second_expression, second_local)
            .expect("later event");
        state
            .record_implicit_capture_use(callable, first_expression, first_local)
            .expect("earlier source event");

        let drained = state
            .take_implicit_capture_uses_for_owners(&BTreeSet::from([callable]))
            .expect("exact owner ledger drain");
        assert_eq!(
            drained.get(&callable).map(Box::as_ref),
            Some(
                &[
                    (second_expression.into(), second_local),
                    (first_expression.into(), first_local),
                ][..]
            )
        );
        assert!(state.pending_implicit_capture_uses().is_empty());
    }

    #[test]
    fn implicit_capture_ledger_rejects_an_unowned_row_without_draining() {
        let (callable, local, _) = call_identity();
        let expression = graph_sites()[0].expression();
        let foreign_callable = graph_sites()[2].expression();
        let mut state = SemanticFactState::new();
        state
            .record_implicit_capture_use(foreign_callable, expression, local)
            .expect("foreign owner row");
        assert_eq!(
            state
                .take_implicit_capture_uses_for_owners(&BTreeSet::from([callable]))
                .expect_err("unowned row must reject the whole drain"),
            CandidateFactTransactionViolation::UnrecoverableLedger
        );
        assert_eq!(
            state.implicit_capture_use(foreign_callable, expression),
            Some(local)
        );
    }

    #[test]
    fn record_input_sites_survive_candidate_extract_apply_and_rollback_together() {
        use super::super::super::CheckedLocalUseSite;
        let (callable, first_local, _) = call_identity();
        let (_, second_local) = distinct_test_locals();
        let owner = graph_sites()[0].expression();
        let first = CheckedLocalUseSite::RecordField {
            owner,
            source_ordinal: 0,
        };
        let second = CheckedLocalUseSite::RecordField {
            owner,
            source_ordinal: 1,
        };
        let mut state = SemanticFactState::new();
        let outer = state
            .begin_candidate_transaction(&mut work())
            .expect("outer");
        let candidate = state
            .begin_candidate_transaction(&mut work())
            .expect("candidate");
        state
            .record_implicit_capture_use(callable, second, second_local)
            .unwrap();
        state
            .record_implicit_capture_use(callable, first, first_local)
            .unwrap();
        let projection = state
            .extract_and_rollback(candidate, &mut work())
            .unwrap_or_else(|_| panic!("exact candidate extraction"));
        assert!(state.pending_implicit_capture_uses().is_empty());
        let authority = state
            .transaction_authority(&outer)
            .expect("outer authority");
        state
            .apply_candidate_projection(&authority, projection, &mut work())
            .unwrap_or_else(|_| panic!("selected projection application"));
        assert_eq!(
            state.pending_implicit_capture_uses(),
            &BTreeMap::from([
                ((callable, first), first_local),
                ((callable, second), second_local)
            ])
        );
        drop(authority);
        state
            .rollback_candidate_transaction(outer)
            .expect("complete outer rollback");
        assert!(state.pending_implicit_capture_uses().is_empty());
        assert!(state.implicit_capture_journal_order.is_empty());
    }

    #[test]
    fn replay_rejects_matching_projections_from_another_fact_owner() {
        let mut origin = SemanticFactState::new();
        let [left, right] = std::array::from_fn(|_| {
            let checkpoint = origin.begin_candidate_transaction(&mut work()).unwrap();
            origin
                .extract_and_rollback(checkpoint, &mut work())
                .unwrap()
        });
        assert_eq!(
            origin.projection_replay_mismatch(&left, &right, CandidateFactReplayMode::Semantic),
            None
        );
        let foreign = SemanticFactState::new();
        assert_eq!(
            foreign.projection_replay_mismatch(&left, &right, CandidateFactReplayMode::Semantic),
            Some(CandidateSemanticReplayMismatch::Authority)
        );
        // A comparison does not consume or republish either affine payload.
        let target = origin.begin_candidate_transaction(&mut work()).unwrap();
        let authority = origin.transaction_authority(&target).unwrap();
        origin
            .apply_candidate_projection(&authority, left, &mut work())
            .unwrap();
        drop(authority);
        origin.rollback_candidate_transaction(target).unwrap();
        assert_eq!(
            origin.projection_replay_mismatch(&right, &right, CandidateFactReplayMode::Semantic),
            None
        );
    }

    #[test]
    fn replay_requires_the_actual_live_prepared_graph_owner() {
        let mut origin = SemanticFactState::new();
        let [left, right] = std::array::from_fn(|_| {
            let checkpoint = origin.begin_candidate_transaction(&mut work()).unwrap();
            origin
                .extract_and_rollback(checkpoint, &mut work())
                .unwrap()
        });
        let mut different_graph = SemanticFactState::new();
        different_graph.issuer = Arc::clone(&origin.issuer);
        assert_eq!(
            different_graph.projection_replay_mismatch(
                &left,
                &right,
                CandidateFactReplayMode::Semantic
            ),
            Some(CandidateSemanticReplayMismatch::PreparedGraph(
                PreparedCallGraphReplayMismatch::Issuer
            ))
        );
        let actual_graph = origin.prepared_calls.take().unwrap();
        assert_eq!(
            origin.projection_replay_mismatch(&left, &right, CandidateFactReplayMode::Semantic),
            Some(CandidateSemanticReplayMismatch::Authority)
        );
        origin.prepared_calls = Some(actual_graph);
        assert_eq!(
            origin.projection_replay_mismatch(&left, &right, CandidateFactReplayMode::Semantic),
            None
        );
    }

    #[test]
    fn replay_rejects_expired_and_poisoned_generations() {
        let mut expired = SemanticFactState::new();
        let [left, right] = std::array::from_fn(|_| {
            let checkpoint = expired.begin_candidate_transaction(&mut work()).unwrap();
            expired
                .extract_and_rollback(checkpoint, &mut work())
                .unwrap()
        });
        expired.epoch += 1;
        assert_eq!(
            expired.projection_replay_mismatch(&left, &right, CandidateFactReplayMode::Semantic),
            Some(CandidateSemanticReplayMismatch::Authority)
        );
        let mut poisoned = SemanticFactState::new();
        let [left, right] = std::array::from_fn(|_| {
            let checkpoint = poisoned.begin_candidate_transaction(&mut work()).unwrap();
            poisoned
                .extract_and_rollback(checkpoint, &mut work())
                .unwrap()
        });
        poisoned.poison = Some(CandidateFactTransactionViolation::UnrecoverableLedger);
        assert_eq!(
            poisoned.projection_replay_mismatch(&left, &right, CandidateFactReplayMode::Semantic),
            Some(CandidateSemanticReplayMismatch::Authority)
        );
    }

    #[test]
    fn capture_replay_compares_selected_sites_instead_of_probe_event_order() {
        use super::super::super::CheckedLocalUseSite;
        let (callable, first_local, _) = call_identity();
        let (_, second_local) = distinct_test_locals();
        let owner = graph_sites()[0].expression();
        let first = (
            CheckedLocalUseSite::RecordField {
                owner,
                source_ordinal: 0,
            },
            first_local,
        );
        let second = (
            CheckedLocalUseSite::RecordField {
                owner,
                source_ordinal: 1,
            },
            second_local,
        );
        let mut state = SemanticFactState::new();
        let outer = state
            .begin_candidate_transaction(&mut work())
            .expect("outer");
        let mut projections = Vec::new();
        for rows in [[first, second], [second, first]] {
            let candidate = state
                .begin_candidate_transaction(&mut work())
                .expect("probe");
            for (site, local) in rows {
                state
                    .record_implicit_capture_use(callable, site, local)
                    .unwrap();
            }
            projections.push(
                state
                    .extract_and_rollback(candidate, &mut work())
                    .unwrap_or_else(|_| panic!("probe extraction")),
            );
        }
        assert_ne!(
            projections[0].data.facts.implicit_capture_journal_order,
            projections[1].data.facts.implicit_capture_journal_order
        );
        assert_eq!(
            state.projection_replay_mismatch(
                &projections[0],
                &projections[1],
                CandidateFactReplayMode::Semantic
            ),
            None
        );
        state
            .rollback_candidate_transaction(outer)
            .expect("outer rollback");
    }
}
