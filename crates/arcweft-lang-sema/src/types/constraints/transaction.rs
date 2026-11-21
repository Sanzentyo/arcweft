//! Candidate-wide equations, prepared source traces, and finalization.

use crate::types::{PreparedEffects, PreparedNamedEffectPort};

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use super::super::{ArrayLength, GenericConstReference, GenericTypeReference, TypeKind};
use super::application::{
    ConstraintApplicationId, ConstraintApplicationInventory, ConstraintApplicationScope,
};
use super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::hints::{ConstraintSourceValueOwner, PreparedSourceExpectation, SourceProbeTerm};
use super::normalization::{ConstraintClosedEffects, ConstraintEffectProjection, project_type};
use super::{
    CheckedConstraintSourceProjection, ClosedMaterializationSubmission, ConstraintAcceptance,
    ConstraintClosurePolicy, ConstraintDomain, ExpectedHint, InheritedSolutionInvariant,
    InheritedSolutionInvariantKind, MaterializationImmediateFailure, MaterializedSourceRequest,
    PreparedConstraintSourceProjection, PreparedSourceConstraint, ProjectedExpectedHint,
    SolvedCandidate, SourceAlternativeHint, SourceError, SourcePhase, SourceProbeResult,
    SourceProbeSelection, TypeConstraintAbort, TypeConstraintCandidateFailure, TypeConstraintError,
    TypeConstraintFailure, TypeConstraintFailureInvariant, TypeConstraintInvariant,
    TypeConstraintProjectionClosure, TypeConstraintProjectionInvariant, TypeConstraintRejection,
    TypeConstraintSolution, TypeConstraintSourceProtocolInvariant, relate_selected_call, seal_path,
    validate_type,
};
use super::{ConstraintSourceId, RejectedConstraintSourceProjection};

mod completed_component;
mod materialization;
mod original_effects;
use materialization::MaterializationInput;
pub(crate) use materialization::{
    ConstraintMaterialization, RetainedSourcePreparations, SourcePreparationTakeError,
};
mod operand;
pub(in crate::types::constraints) use operand::ConstraintAliasStep;
pub(in crate::types::constraints) use operand::{
    AdmittedConstraintFunctionHeader, ConstraintEffectField, ConstraintEffectHeaderDerivation,
    ConstraintOperandChildren, ConstraintOperandRebuild, ConstraintProjectedOperand,
};
pub(in crate::types) use operand::{
    ConstraintConstOperandView, ConstraintFunctionHeaderView, ConstraintOperand,
    ConstraintOperandView,
};
use operand::{ResolvedConstraintResult, ResolvedConstraintSource};

/// One equation retained until candidate closure. Source ordinals connect to
/// the single source trace; selection and container evidence stay there.
pub(crate) struct PendingEquation<D: ConstraintDomain> {
    pub(crate) ordinal: ConstraintEquationId,
    pub(crate) direction: ConstraintAcceptance,
    pattern: ConstraintOperand<D>,
    actual: ConstraintOperand<D>,
    pub(crate) source_ordinal: Option<PreparedSourceOrdinal>,
    timing: ConstraintEquationTiming,
}

impl<D: ConstraintDomain> Clone for PendingEquation<D> {
    fn clone(&self) -> Self {
        Self {
            ordinal: self.ordinal,
            direction: self.direction,
            pattern: self.pattern.clone(),
            actual: self.actual.clone(),
            source_ordinal: self.source_ordinal,
            timing: self.timing,
        }
    }
}

impl<D: ConstraintDomain> PendingEquation<D> {
    pub(super) fn validate_completed<A: TypeConstraintAccounting, L>(
        &self,
        branch: &ConstraintInferenceBranch<D, L>,
        projection: &mut ConstraintEffectProjection<D>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        context.with_original_scope(self.scope().clone(), |context| {
            self.validate_in_scope(branch, projection, graph, context)
        })
    }

    pub(super) fn scope(&self) -> &super::lexical::ConstraintLexicalScope {
        self.pattern.scope()
    }

    /// Final source equations borrow the source's canonical ports and exact
    /// completion directly; no second final relation is stored in original C.
    pub(super) fn validate_completed_source<A: TypeConstraintAccounting, L>(
        &self,
        source: &ClosedConstraintProbe<D>,
        branch: &ConstraintInferenceBranch<D, L>,
        projection: &mut ConstraintEffectProjection<D>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        projection.owner().require_branch(branch)?;
        source.require_completion(projection.owner())?;
        if self.source_ordinal != Some(source.ordinal()) {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        match source.expected_operand() {
            Some(expected) => self.compare_completed(
                (expected.value(), expected.scope()),
                (
                    source.actual_operand().value(),
                    source.actual_operand().scope(),
                ),
                projection.owner(),
                context,
            ),
            None => self.validate_completed(branch, projection, graph, context),
        }
    }

    fn validate_in_scope<A: TypeConstraintAccounting, L>(
        &self,
        branch: &ConstraintInferenceBranch<D, L>,
        projection: &mut ConstraintEffectProjection<D>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        let pattern = self.pattern.value();
        let project = |projection: &mut ConstraintEffectProjection<D>,
                       value: &TypeKind<PreparedEffects>,
                       context: &mut TypeConstraintContext<'_, A, D>| {
            projection.project_type(value, branch, ConstraintClosurePolicy::Hint, graph, context)
        };
        let pattern = project(projection, pattern, context)?;
        let actual = context.with_original_scope(self.actual.scope().clone(), |context| {
            if self.timing == ConstraintEquationTiming::InferredEffectResult {
                let annotation = projection.project_type(
                    self.actual.value(),
                    branch,
                    ConstraintClosurePolicy::ResultAnnotation,
                    graph,
                    context,
                )?;
                let value = self.actual.scope().transport_canonical_type(
                    self.pattern.scope(),
                    &pattern,
                    context,
                )?;
                annotation
                    .binding_type_with_inferred_effects_with_control(&value, context)?
                    .ok_or_else(|| TypeConstraintRejection::Mismatch.into())
            } else {
                project(projection, self.actual.value(), context)
            }
        })?;
        self.compare_completed(
            (&pattern, self.pattern.scope()),
            (&actual, self.actual.scope()),
            projection.owner(),
            context,
        )
    }

    fn compare_completed<A: TypeConstraintAccounting>(
        &self,
        pattern: (&TypeKind, &super::lexical::ConstraintLexicalScope),
        actual: (&TypeKind, &super::lexical::ConstraintLexicalScope),
        effects: &ConstraintClosedEffects<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        use std::borrow::Cow;
        let target = effects.calculation_scope();
        let failure = |error| match error {
            crate::types::TypeProjectionError::Control(error) => error,
            crate::types::TypeProjectionError::Instantiation(error) => {
                TypeConstraintInvariant::Instantiation(error).into()
            }
        };
        let pattern = if pattern.1 == target {
            Cow::Borrowed(pattern.0)
        } else {
            Cow::Owned(
                target
                    .transport_canonical_type(pattern.1, pattern.0, context)
                    .map_err(failure)?,
            )
        };
        let actual = if actual.1 == target {
            Cow::Borrowed(actual.0)
        } else {
            Cow::Owned(
                target
                    .transport_canonical_type(actual.1, actual.0, context)
                    .map_err(failure)?,
            )
        };
        for _ in target.shape().binders() {
            context.enter_node()?;
        }
        context.with_original_scope(target.clone(), |context| {
            let (expected, actual) = match self.direction {
                ConstraintAcceptance::PatternAcceptsActual => (pattern.as_ref(), actual.as_ref()),
                ConstraintAcceptance::ActualAcceptsPattern => (actual.as_ref(), pattern.as_ref()),
            };
            if !expected
                .accepts_with(
                    actual,
                    super::super::compatibility::TypeCompatibilityPolicy::SelectedCall,
                    context,
                )
                .map_err(super::super::compatibility::binding_plan::map_compatibility_error)?
            {
                return Err(TypeConstraintRejection::Mismatch.into());
            }

            Ok(())
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ConstraintEquationTiming {
    Immediate,
    ResultContext,
    InferredEffectResult,
}

impl ConstraintEquationTiming {
    fn for_actual<A: TypeConstraintAccounting, D: ConstraintDomain>(
        self,
        actual: &TypeKind<PreparedEffects>,
        acceptance: ConstraintAcceptance,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        Ok(
            if self == Self::ResultContext
                && acceptance == ConstraintAcceptance::ActualAcceptsPattern
                && actual
                    .has_omitted_function_effect_rows_with_control(context)
                    .map_err(|error| match error {
                        crate::types::TypeProjectionError::Control(error) => error,
                        crate::types::TypeProjectionError::Instantiation(error) => {
                            TypeConstraintInvariant::Instantiation(error).into()
                        }
                    })?
            {
                Self::InferredEffectResult
            } else {
                self
            },
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ChoiceForkRole {
    ExpectedAlternative,
    ActualAlternative,
    ExpectedActualPair,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ChoiceDerivationStep {
    pub(crate) equation: ConstraintEquationId,
    pub(crate) direction: ConstraintAcceptance,
    pub(crate) role: ChoiceForkRole,
    pub(crate) expected: Option<u32>,
    pub(crate) actual: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DeferredCycleWitness {
    pub(crate) parameters: BTreeSet<super::ConstraintGenericParameterId>,
}

mod source;
use source::ActiveConstraintProbe;
pub(super) use source::SourceProbeObservation;
pub(crate) use source::{ClosedConstraintProbe, ClosedConstraintSourceTrace};

pub(crate) enum StoredSourceSelection<D: ConstraintDomain> {
    Unchecked,
    Checked {
        alternative: D::AlternativeIndex,
        evidence: Arc<D::ObservedEvidence>,
    },
}

impl<D: ConstraintDomain> Clone for StoredSourceSelection<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Unchecked => Self::Unchecked,
            Self::Checked {
                alternative,
                evidence,
            } => Self::Checked {
                alternative: *alternative,
                evidence: Arc::clone(evidence),
            },
        }
    }
}

impl<D: ConstraintDomain> PartialEq for StoredSourceSelection<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unchecked, Self::Unchecked) => true,
            (
                Self::Checked {
                    alternative: left_alternative,
                    evidence: left_evidence,
                },
                Self::Checked {
                    alternative: right_alternative,
                    evidence: right_evidence,
                },
            ) => left_alternative == right_alternative && left_evidence == right_evidence,
            _ => false,
        }
    }
}

impl<D: ConstraintDomain> Eq for StoredSourceSelection<D> {}

/// Source ownership is distinct from the inference algebra. A joint original
/// product uses the same algebra without fabricating a source root or probe.
pub(crate) struct ConstraintInferenceBranch<D: ConstraintDomain, L> {
    pub(super) lineage: L,
    pub(super) scope: super::lexical::ConstraintLexicalScope,
    pub(super) applications: Arc<ConstraintApplicationInventory<D>>,
    // Each value is relative to the exact introduction scope of its key's
    // admitted application. Only the scoped projection view follows an edge.
    pub(super) bindings: BTreeMap<GenericTypeReference, ConstraintOperand<D>>,
    pub(super) const_bindings: BTreeMap<GenericConstReference, ConstraintOperand<D, ArrayLength>>,
    pub(super) effects: super::normalization::BranchEffectConstraints,
    pub(crate) equations: Vec<PendingEquation<D>>,
    pub(crate) choice_key: Vec<ChoiceDerivationStep>,
    pub(crate) deferred_cycles: DeferredCycleWitness,
}

pub(crate) struct ConstraintSourceLineage<D: ConstraintDomain> {
    root: ConstraintApplicationId,
    pub(super) original: super::OriginalProducerContext,
    probe_trace: Vec<ActiveConstraintProbe<D>>,
    pub(super) projections: Vec<Arc<ProjectionRequest<D>>>,
    // Includes the detached reads of every preparation in probe_trace.
    effect_reads: Option<crate::effect_row::prepared::PreparedEffectReadScope>,
}

impl<D: ConstraintDomain> Clone for ConstraintSourceLineage<D> {
    fn clone(&self) -> Self {
        Self {
            root: self.root,
            original: self.original.clone(),
            probe_trace: self.probe_trace.clone(),
            projections: self.projections.clone(),
            effect_reads: self.effect_reads.clone(),
        }
    }
}

impl<D: ConstraintDomain> ConstraintSourceLineage<D> {
    fn effect_read_scope(&self) -> Option<&crate::effect_row::prepared::PreparedEffectReadScope> {
        self.effect_reads.as_ref()
    }
}

pub(crate) type ConstraintPath<D> = ConstraintInferenceBranch<D, ConstraintSourceLineage<D>>;

impl<D: ConstraintDomain> ConstraintPath<D> {
    #[cfg(test)]
    pub(super) fn empty(
        application: ConstraintApplicationScope<D>,
        effects: crate::effect_row::EffectConstraintEnvironment<PreparedEffects>,
    ) -> Self {
        let original = application.original_context().clone();
        Self::empty_with_imported(application, effects, None, Default::default(), original)
    }

    pub(super) fn empty_with_imported(
        application: ConstraintApplicationScope<D>,
        effects: crate::effect_row::EffectConstraintEnvironment<PreparedEffects>,
        imported: Option<super::ImportedGenericParameterScopeLease>,
        scope: super::lexical::ConstraintLexicalScope,
        original: super::OriginalProducerContext,
    ) -> Self {
        let root = application.id();
        Self {
            lineage: ConstraintSourceLineage {
                root,
                original,
                probe_trace: Vec::new(),
                projections: Vec::new(),
                effect_reads: None,
            },
            scope,
            applications: Arc::new(ConstraintApplicationInventory::single(
                application,
                imported,
            )),
            bindings: BTreeMap::new(),
            const_bindings: BTreeMap::new(),
            effects: super::normalization::BranchEffectConstraints::new(effects),
            equations: Vec::new(),
            choice_key: Vec::new(),
            deferred_cycles: DeferredCycleWitness::default(),
        }
    }

    pub(super) const fn root_id(&self) -> ConstraintApplicationId {
        self.lineage.root
    }

    #[cfg(test)]
    pub(super) fn root_scope(&self) -> &ConstraintApplicationScope<D> {
        self.applications
            .application(self.root_id())
            .expect("a source path retains its original root application")
    }
}

impl<D: ConstraintDomain, L: Clone> Clone for ConstraintInferenceBranch<D, L> {
    fn clone(&self) -> Self {
        Self {
            lineage: self.lineage.clone(),
            scope: self.scope.clone(),
            applications: Arc::clone(&self.applications),
            bindings: self.bindings.clone(),
            const_bindings: self.const_bindings.clone(),
            effects: self.effects.clone(),
            equations: self.equations.clone(),
            choice_key: self.choice_key.clone(),
            deferred_cycles: self.deferred_cycles.clone(),
        }
    }
}

/// One affine source branch. Observation consumes its path; forks share the
/// immutable input receipt and charge the surrounding context before cloning.
pub(crate) struct ProbeTicket<D: ConstraintDomain> {
    input: Arc<ProbeInput<D>>,
    path: Option<ConstraintPath<D>>,
}

/// Exact parent source receipt retained while a nested callable contributes
/// constraints to the same lower path.  The input identity binds the source,
/// prepared schema alternatives, and exact path branch together.
pub(crate) struct ConstraintSourceReceipt<D: ConstraintDomain> {
    input: Arc<ProbeInput<D>>,
}

impl<D: ConstraintDomain> Clone for ConstraintSourceReceipt<D> {
    fn clone(&self) -> Self {
        Self {
            input: Arc::clone(&self.input),
        }
    }
}

impl<D: ConstraintDomain> ConstraintSourceReceipt<D> {
    pub(crate) fn source(&self) -> ConstraintSourceId<D::Source> {
        self.input.source
    }

    pub(crate) fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.input, &other.input)
    }

    pub(super) fn matches_closed_expectation(
        &self,
        alternative: Option<D::AlternativeIndex>,
        expected: Option<&TypeKind<PreparedEffects>>,
    ) -> bool {
        if self.input.prepared.is_unchecked() {
            return alternative.is_none() && expected.is_none();
        }
        self.input.prepared.source_projection().is_scalar()
            && self.input.alternatives().any(|(row, opened)| {
                Some(row.alternative()) == alternative
                    && opened.unbound.is_empty()
                    && Some(opened.hint_type()) == expected
            })
    }
}

/// A fork of one exact source path that may admit a child callable
/// application.  Its receipt prevents a child candidate from attaching to a
/// sibling source ticket.
pub(crate) struct NestedConstraintPath<D: ConstraintDomain> {
    receipt: ConstraintSourceReceipt<D>,
    path: ConstraintPath<D>,
}

impl<D: ConstraintDomain> NestedConstraintPath<D> {
    pub(crate) fn original_context(
        &self,
    ) -> Result<&super::OriginalProducerContext, TypeConstraintError> {
        Ok(self
            .path
            .applications
            .require_application(self.receipt.input.source.application())?
            .original_context())
    }
}

/// Owner-bound handle to a result projection registered on one admitted child
/// application.  It carries no open `TypeKind<PreparedEffects>`; the lower transaction opens
/// the schema term on the matching path when it submits the parent source.
pub(crate) struct ConstraintResultProjection<D: ConstraintDomain> {
    application: ConstraintApplicationId,
    key: Arc<D::Projection>,
}

impl<D: ConstraintDomain> Clone for ConstraintResultProjection<D> {
    fn clone(&self) -> Self {
        Self {
            application: self.application,
            key: Arc::clone(&self.key),
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ConstraintResultProjection<D> {
    fn eq(&self, other: &Self) -> bool {
        self.application == other.application && self.key == other.key
    }
}

impl<D: ConstraintDomain> Eq for ConstraintResultProjection<D> {}

impl<D: ConstraintDomain> ConstraintResultProjection<D> {
    pub(crate) const fn application(&self) -> ConstraintApplicationId {
        self.application
    }

    pub(crate) fn key(&self) -> &D::Projection {
        &self.key
    }
}

/// Open child application alternatives waiting for their result to be related
/// to the exact parent source.  Completion remains owned by the parent path.
pub(crate) struct PendingChildConstraint<D: ConstraintDomain> {
    receipt: ConstraintSourceReceipt<D>,
    alternatives: Vec<PendingChildAlternative<D>>,
}

struct PendingChildAlternative<D: ConstraintDomain> {
    path: ConstraintPath<D>,
    result: ConstraintResultProjection<D>,
    branch: Option<Arc<SourceProbeObservation<D>>>,
}

impl<D: ConstraintDomain> PendingChildConstraint<D> {
    pub(crate) fn with_probe_branch(mut self, branch: D::ProbeSemanticBranch) -> Self {
        let branch = SourceProbeObservation::new(branch);
        for alternative in &mut self.alternatives {
            alternative.branch = Some(Arc::clone(&branch));
        }
        self
    }

    pub(crate) fn append(&mut self, other: Self) -> Result<(), TypeConstraintError> {
        if !self.receipt.matches(&other.receipt) {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        }
        self.alternatives.extend(other.alternatives);
        Ok(())
    }
}

/// One source's alternatives retain their own path, term and semantic evidence.
/// Construction consumes affine probe branches; no observation is broadcast.
pub(crate) struct SourceProbeContribution<D: ConstraintDomain> {
    input: Arc<ProbeInput<D>>,
    alternatives: Vec<ObservedProbeAlternative<D>>,
}

/// A callback has closed its fact scope before these alternatives can enter
/// the frontier. Forked paths alias this one move-only preparation.
pub(crate) struct ClosedSourceProbeContribution<D: ConstraintDomain> {
    input: Arc<ProbeInput<D>>,
    alternatives: Vec<ObservedProbeAlternative<D>>,
    preparation: D::ProbePreparation,
}

impl<D: ConstraintDomain> ClosedSourceProbeContribution<D> {
    fn admit_effect_reads<A: TypeConstraintAccounting>(
        &mut self,
        graph: &crate::effect_row::prepared::PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        let Some(scope) = D::probe_effect_reads(&self.preparation) else {
            return Ok(());
        };
        for alternative in &mut self.alternatives {
            let value = match alternative.result.term() {
                SourceProbeTerm::AdmittedValue(value) => value.value(),
                SourceProbeTerm::Result(result) => {
                    ResolvedConstraintResult::request(result, &alternative.path)?
                        .value
                        .value()
                }
                SourceProbeTerm::Value(_) => {
                    return Err(protocol_error(
                        TypeConstraintSourceProtocolInvariant::Outcome,
                    ));
                }
                #[cfg(test)]
                SourceProbeTerm::Type(value) => value,
            };
            let reads = graph.compose_read_scopes(
                alternative.path.lineage.effect_reads.iter().chain([scope]),
                context,
            )?;
            if !value.constraint_shape().is_closed_leaf() {
                let roots = crate::types::effect_domain::PreparedTypeEffectRoots::collect(
                    [value],
                    [],
                    context,
                )
                .map_err(|error| match error {
                    crate::types::TypeProjectionError::Control(error) => error,
                    crate::types::TypeProjectionError::Instantiation(error) => {
                        TypeConstraintInvariant::Instantiation(error).into()
                    }
                })?;
                graph.read_scoped(&reads, context)?.validate_query_roots(
                    roots.rows(),
                    roots.predicates(),
                    context,
                )?;
            }
            alternative.path.lineage.effect_reads = Some(reads);
        }
        Ok(())
    }

    fn admit_preparation<A: TypeConstraintAccounting>(
        mut self,
        graph: &crate::effect_row::prepared::PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Vec<ObservedProbeAlternative<D>>, TypeConstraintError> {
        context.check_cancelled()?;
        self.admit_effect_reads(graph, context)?;
        let first = self
            .alternatives
            .first()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Outcome))?
            .result
            .observation();
        if first.preparation().is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        let mut shared = false;
        for alternative in self.alternatives.iter().skip(1) {
            context.enter_node()?;
            let observation = alternative.result.observation();
            if observation.preparation().is_some() {
                return Err(protocol_error(
                    TypeConstraintSourceProtocolInvariant::Outcome,
                ));
            }
            shared |= !Arc::ptr_eq(first, observation);
        }
        if shared {
            // Distinct observations share facts without cloning the payload.
            // Admit this extra owner and the scan before publishing any facts.
            context.enter_node()?;
            let preparation = Arc::new(self.preparation);
            for alternative in &self.alternatives {
                let observation = alternative.result.observation();
                if observation.preparation().is_none() {
                    observation.retain_shared(Arc::clone(&preparation))?;
                }
            }
        } else {
            // The existing branch allocation is the preparation owner too.
            first.retain_owned(self.preparation)?;
        }
        Ok(self.alternatives)
    }
}

struct ObservedProbeAlternative<D: ConstraintDomain> {
    path: ConstraintPath<D>,
    result: SourceProbeResult<D>,
}

/// A child result resolved against the exact parent probe receipt. The
/// operational client shares the value already admitted to the parent path.
pub(crate) struct ResolvedResultProbe<D: ConstraintDomain> {
    source: ConstraintSourceId<D::Source>,
    actual: Arc<TypeKind<PreparedEffects>>,
}

impl<D: ConstraintDomain> ResolvedResultProbe<D> {
    pub(crate) fn into_parts(
        self,
    ) -> (
        ConstraintSourceId<D::Source>,
        Arc<TypeKind<PreparedEffects>>,
    ) {
        (self.source, self.actual)
    }
}

impl<D: ConstraintDomain> std::fmt::Debug for ResolvedResultProbe<D> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResolvedResultProbe")
            .field("actual", &self.actual)
            .finish_non_exhaustive()
    }
}

impl<D: ConstraintDomain> PartialEq for ResolvedResultProbe<D> {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && Arc::ptr_eq(&self.actual, &other.actual)
    }
}

impl<D: ConstraintDomain> Eq for ResolvedResultProbe<D> {}

impl<D: ConstraintDomain> SourceProbeContribution<D> {
    pub(crate) fn close_preparation(
        self,
        preparation: D::ProbePreparation,
    ) -> ClosedSourceProbeContribution<D> {
        ClosedSourceProbeContribution {
            input: self.input,
            alternatives: self.alternatives,
            preparation,
        }
    }

    pub(crate) fn append(&mut self, other: Self) -> Result<(), TypeConstraintError> {
        if !Arc::ptr_eq(&self.input, &other.input) {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        self.alternatives.extend(other.alternatives);
        Ok(())
    }
}

/// Immutable source/schema observation shared while a probe updates its own
/// constraint frontier. It owns no mutable bindings or application membership.
pub(crate) struct ProbeInput<D: ConstraintDomain> {
    source: ConstraintSourceId<D::Source>,
    prepared: Arc<PreparedSourceConstraint<D>>,
    opened_alternatives: Vec<OpenedSourceAlternative<D>>,
    acceptance: ConstraintAcceptance,
    equation_ordinal: Option<ConstraintEquationId>,
}

struct OpenedSourceAlternative<D: ConstraintDomain> {
    /// This is the exact opened operand from the issued input, before hint
    /// projection follows bindings on the pre-callback path.
    operand: ConstraintOperand<D>,
    projected_hint: Option<TypeKind<PreparedEffects>>,
    unbound: Box<[super::ConstraintGenericParameterId]>,
    scope_lease: Option<super::ImportedGenericParameterScopeLease>,
}

impl<D: ConstraintDomain> OpenedSourceAlternative<D> {
    fn hint_type(&self) -> &TypeKind<PreparedEffects> {
        self.projected_hint
            .as_ref()
            .unwrap_or_else(|| self.operand.value())
    }
}

impl<D: ConstraintDomain> ProbeTicket<D> {
    pub(crate) fn application(&self) -> Result<D::Application, TypeConstraintError> {
        Ok(self
            .path
            .as_ref()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::WrongPhase))?
            .applications
            .require_application(self.input.source.application())?
            .application())
    }

    pub(crate) fn original_context(
        &self,
    ) -> Result<&super::OriginalProducerContext, TypeConstraintError> {
        self.path
            .as_ref()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::WrongPhase))?
            .applications
            .require_application(self.input.source.application())
            .map(|owner| owner.original_context())
            .map_err(Into::into)
    }
    #[cfg(test)]
    pub(super) fn test_path(&self) -> &ConstraintPath<D> {
        self.path.as_ref().expect("unobserved probe branch")
    }

    pub(crate) fn source(&self) -> ConstraintSourceId<D::Source> {
        self.input.source
    }

    pub(crate) fn input(&self) -> Arc<ProbeInput<D>> {
        Arc::clone(&self.input)
    }

    pub(crate) fn effect_reads(
        &self,
    ) -> Option<&crate::effect_row::prepared::PreparedEffectReadScope> {
        self.path.as_ref()?.lineage.effect_reads.as_ref()
    }

    pub(crate) fn receipt(&self) -> ConstraintSourceReceipt<D> {
        ConstraintSourceReceipt {
            input: Arc::clone(&self.input),
        }
    }

    pub(crate) fn fork_for_child<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<NestedConstraintPath<D>, TypeConstraintError> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Ticket))?;
        Ok(NestedConstraintPath {
            receipt: self.receipt(),
            path: context.fork_path(path)?,
        })
    }

    pub(crate) fn fork<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Ticket))?;
        Ok(Self {
            input: Arc::clone(&self.input),
            path: Some(context.fork_path(path)?),
        })
    }

    /// Admit while the callback's actual relation is still live. Closing the
    /// callback may extract its terms, so admission cannot be postponed until
    /// `submit_probe`.
    pub(crate) fn observe_prepared<A: TypeConstraintAccounting>(
        &mut self,
        result: SourceProbeResult<D>,
        effects: &mut crate::effect_row::prepared::PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<SourceProbeContribution<D>, TypeConstraintError> {
        if self.path.is_none() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        let receipt = self.receipt();
        result.validate_source(&receipt)?;
        let scope = Arc::new(context.capture_lexical_scope()?);
        let result = result.admit_value(|owner| {
            let original = owner.original_context().clone();
            let (value, owner) = ResolvedConstraintSource::value(
                owner,
                &scope,
                &mut operand::ConstraintEffectPhase::new(effects),
                context,
            )?;
            Ok(ConstraintOperand::source(
                value, scope, original, receipt, owner,
            ))
        })?;
        self.observe(result)
    }

    pub(crate) fn observe(
        &mut self,
        result: SourceProbeResult<D>,
    ) -> Result<SourceProbeContribution<D>, TypeConstraintError> {
        result.validate_source(&self.receipt())?;
        let path = self
            .path
            .take()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Ticket))?;
        Ok(SourceProbeContribution {
            input: Arc::clone(&self.input),
            alternatives: vec![ObservedProbeAlternative { path, result }],
        })
    }

    pub(crate) fn observe_child(
        &mut self,
        pending: PendingChildConstraint<D>,
        branch: D::ProbeSemanticBranch,
        selection: SourceProbeSelection<D::AlternativeIndex, Arc<D::ObservedEvidence>>,
    ) -> Result<SourceProbeContribution<D>, TypeConstraintError> {
        if !self.receipt().matches(&pending.receipt) {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        }
        if self.path.is_none() || pending.alternatives.is_empty() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        let PendingChildConstraint {
            receipt: _,
            alternatives,
        } = pending;
        let branch = SourceProbeObservation::new(branch);
        let alternatives = alternatives
            .into_iter()
            .map(|alternative| ObservedProbeAlternative {
                path: alternative.path,
                result: SourceProbeResult::projection(
                    alternative.result,
                    alternative.branch.unwrap_or_else(|| Arc::clone(&branch)),
                    selection.clone(),
                ),
            })
            .collect();
        self.path.take();
        Ok(SourceProbeContribution {
            input: Arc::clone(&self.input),
            alternatives,
        })
    }
}

impl<D: ConstraintDomain> ProbeInput<D> {
    fn alternatives(
        &self,
    ) -> impl Iterator<
        Item = (
            &super::PreparedSourceAlternative<D>,
            &OpenedSourceAlternative<D>,
        ),
    > {
        self.prepared.alternatives().zip(&self.opened_alternatives)
    }

    /// Build borrowed per-alternative hints for the callback lifetime.  The
    /// temporary view cannot escape this call, so the callback never owns or
    /// rewrites a lower expected type.
    pub(crate) fn with_hint<R>(
        &self,
        callback: impl for<'h> FnOnce(ExpectedHint<'h, D>) -> R,
    ) -> R {
        if self.prepared.is_unchecked() {
            return callback(ExpectedHint::Unchecked);
        }
        let hints =
            self.alternatives()
                .map(|(alternative, opened)| {
                    let value_expected = if opened.unbound.is_empty() {
                        ProjectedExpectedHint::Complete(opened.hint_type())
                    } else {
                        ProjectedExpectedHint::Parametric {
                            expected: opened.hint_type(),
                            unbound: &opened.unbound,
                            scope_lease: opened.scope_lease.as_ref().expect(
                                "parametric expected hints retain their source scope lease",
                            ),
                        }
                    };
                    SourceAlternativeHint::new(
                        alternative.alternative(),
                        alternative.evidence(),
                        value_expected,
                        self.prepared.source_projection(),
                    )
                })
                .collect::<Vec<_>>();
        callback(ExpectedHint::Alternatives(&hints))
    }
}

pub(crate) enum ProbeSubmission<D: ConstraintDomain> {
    Accepted(ClosedSourceProbeContribution<D>),
    Rejected(D::SourceErrorCause),
}

pub(crate) struct MaterializationTicket<D: ConstraintDomain> {
    identity: MaterializationTicketIdentity,
    correlation: MaterializationCorrelationOrdinal,
    input: MaterializationInput<D>,
    phase: MaterializationTicketPhase,
}

enum MaterializationTicketPhase {
    Ready,
    CallbackBound,
    Closed,
}

pub(crate) struct MaterializationCallbackBinding<D: ConstraintDomain> {
    identity: MaterializationTicketIdentity,
    sources: Box<[ConstraintSourceId<D::Source>]>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PreparedSourceOrdinal {
    application: ConstraintApplicationId,
    ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ConstraintEquationId {
    application: ConstraintApplicationId,
    ordinal: u32,
}

/// Completion precedence follows the one ordered trace, not opening issuance
/// or an application's independently numbered prepared sources.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct MaterializationSourceOrdinal(u32);

#[derive(Clone)]
struct MaterializationTicketIdentity {
    issuer: Arc<MaterializationTicketIssuer>,
    ordinal: u64,
}

struct MaterializationTicketIssuer;

struct MaterializationCorrelationOrdinal(u32);

pub(crate) struct ClosedMaterialization<D: ConstraintDomain> {
    identity: MaterializationTicketIdentity,
    submission: ClosedMaterializationSubmission<D>,
}

impl<D: ConstraintDomain> MaterializationTicket<D> {
    pub(crate) fn requests(
        &self,
    ) -> impl ExactSizeIterator<Item = MaterializedSourceRequest<'_, D>> + '_ {
        self.input.preparations.requests(&self.input.component)
    }

    pub(crate) fn bind_callback(
        &mut self,
    ) -> Result<MaterializationCallbackBinding<D>, TypeConstraintSourceProtocolInvariant> {
        if !matches!(self.phase, MaterializationTicketPhase::Ready) {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        self.phase = MaterializationTicketPhase::CallbackBound;
        Ok(MaterializationCallbackBinding {
            identity: self.identity.clone(),
            sources: self
                .input
                .component
                .sources()
                .all()
                .iter()
                .map(ClosedConstraintProbe::source)
                .collect(),
        })
    }

    pub(crate) fn validate_callback_binding(
        &self,
        binding: &MaterializationCallbackBinding<D>,
    ) -> Result<(), TypeConstraintSourceProtocolInvariant> {
        if !matches!(self.phase, MaterializationTicketPhase::CallbackBound) {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        if !materialization_identity_matches(&self.identity, &binding.identity) {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        let expected = self
            .input
            .component
            .sources()
            .all()
            .iter()
            .map(ClosedConstraintProbe::source);
        if expected.eq(binding.sources.iter().copied()) {
            Ok(())
        } else {
            Err(TypeConstraintSourceProtocolInvariant::WrongSource)
        }
    }

    pub(crate) fn bind_closed_submission(
        &mut self,
        submission: ClosedMaterializationSubmission<D>,
    ) -> Result<ClosedMaterialization<D>, TypeConstraintSourceProtocolInvariant> {
        if !matches!(self.phase, MaterializationTicketPhase::CallbackBound) {
            return Err(TypeConstraintSourceProtocolInvariant::Ticket);
        }
        self.phase = MaterializationTicketPhase::Closed;
        Ok(ClosedMaterialization {
            identity: self.identity.clone(),
            submission,
        })
    }
}

impl<D: ConstraintDomain> MaterializationCallbackBinding<D> {
    pub(crate) fn sources(&self) -> &[ConstraintSourceId<D::Source>] {
        &self.sources
    }

    pub(crate) fn authorizes(&self, source: &ConstraintSourceId<D::Source>) -> bool {
        self.sources.iter().any(|candidate| candidate == source)
    }
}

fn materialization_identity_matches(
    left: &MaterializationTicketIdentity,
    right: &MaterializationTicketIdentity,
) -> bool {
    Arc::ptr_eq(&left.issuer, &right.issuer) && left.ordinal == right.ordinal
}

pub(crate) struct TypeConstraintProjectionOptions<'a> {
    pub(crate) closure: TypeConstraintProjectionClosure,
    pub(crate) input: Option<&'a TypeKind<PreparedEffects>>,
    pub(crate) named_effect: Option<&'a PreparedNamedEffectPort>,
}

pub(super) struct ProjectionRequest<D: ConstraintDomain> {
    application: ConstraintApplicationId,
    key: Arc<D::Projection>,
    value: ConstraintOperand<D>,
    closure: TypeConstraintProjectionClosure,
    source: Option<ConstraintResultProjection<D>>,
    input: Option<Arc<ConstraintOperand<D>>>,
    named_effect: Option<PreparedNamedEffectPort>,
}

struct ProbeOperation<D: ConstraintDomain> {
    source: ConstraintSourceId<D::Source>,
    source_ordinal: PreparedSourceOrdinal,
    prepared: Arc<PreparedSourceConstraint<D>>,
    acceptance: ConstraintAcceptance,
    equation_ordinal: Option<ConstraintEquationId>,
    rows: VecDeque<ConstraintPath<D>>,
    active_input: Option<Arc<ProbeInput<D>>>,
    advanced: Vec<ConstraintPath<D>>,
    rejections: Vec<D::SourceErrorCause>,
    relation_rejections: Vec<RejectedConstraintSourceProjection<D>>,
    deferred_rejection_tail: bool,
}

impl<D: ConstraintDomain> ProbeOperation<D> {
    fn into_ordinary_rejection(self) -> TypeConstraintFailure<D> {
        if !self.rejections.is_empty() {
            return TypeConstraintFailure::Rejected(TypeConstraintCandidateFailure::Source(
                Box::new(SourceError::new(
                    self.source,
                    SourcePhase::Probe,
                    self.rejections.into_boxed_slice(),
                )),
            ));
        }
        if let Some(rejection) = self.relation_rejections.into_iter().next() {
            return TypeConstraintFailure::Rejected(
                TypeConstraintCandidateFailure::SourceProjection(Box::new(rejection)),
            );
        }
        TypeConstraintError::Rejected(TypeConstraintRejection::Mismatch).into()
    }
}

/// One mapper-issued authored-source group. If an earlier slot eliminates
/// the semantic frontier, later slots still execute against the last live
/// correlated rows, but their results cannot resurrect the failed group.
struct ProbeGroup<D: ConstraintDomain> {
    remaining_sources: usize,
    last_live_frontier: Vec<ConstraintPath<D>>,
    deferred_failure: Option<TypeConstraintFailure<D>>,
    ordinary_rejection_pending: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeStart {
    Started,
    Skipped,
}

enum MaterializedRecord<D: ConstraintDomain> {
    Sealed {
        input: MaterializationInput<D>,
        value: D::SealedBranchValue,
    },
    Rejected {
        source_ordinal: MaterializationSourceOrdinal,
        correlation: MaterializationCorrelationOrdinal,
        source: ConstraintSourceId<D::Source>,
        cause: D::SourceErrorCause,
    },
    Fatal {
        source_ordinal: MaterializationSourceOrdinal,
        correlation: MaterializationCorrelationOrdinal,
        error: SourceError<ConstraintSourceId<D::Source>, D::SourceErrorCause>,
    },
}

enum NormalizedPath<D: ConstraintDomain> {
    Acyclic(ConstraintPath<D>),
    Cyclic(super::ConstraintGenericParameterId),
}

pub(crate) struct TypeConstraintTransaction<D: ConstraintDomain> {
    application: ConstraintApplicationId,
    original: super::OriginalProducerContext,
    parent_receipt: Option<ConstraintSourceReceipt<D>>,
    frontier: Vec<ConstraintPath<D>>,
    first_failure: Option<TypeConstraintFailure<D>>,
    next_equation: u32,
    next_source_ordinal: u32,
    first_cycle: Option<super::ConstraintGenericParameterId>,
    probe: Option<ProbeOperation<D>>,
    probe_group: Option<ProbeGroup<D>>,
    prepared_sources: BTreeSet<ConstraintSourceId<D::Source>>,
    materialization: ConstraintMaterialization<D>,
    closed: bool,
}

impl<D: ConstraintDomain> TypeConstraintTransaction<D> {
    #[cfg(test)]
    pub(crate) fn initialize_fixture<A: TypeConstraintAccounting>(
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        application: D::Application,
        parameters: super::TypeConstraintParameterScope,
        inherited: Option<Arc<TypeConstraintSolution>>,
    ) -> Result<Self, super::TypeConstraintInitializationFailure> {
        Self::initialize_with_imported_fixture(
            context,
            graph,
            application,
            parameters,
            inherited,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn initialize_with_imported_fixture<A: TypeConstraintAccounting>(
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        application: D::Application,
        parameters: super::TypeConstraintParameterScope,
        inherited: Option<Arc<TypeConstraintSolution>>,
        imported: Option<super::ImportedGenericParameterScopeLease>,
    ) -> Result<Self, super::TypeConstraintInitializationFailure> {
        let original = context
            .fixture_original_context(imported.as_ref())
            .map_err(super::TypeConstraintInitializationFailure::original_context)?;
        Self::initialize_with_imported(
            context,
            graph,
            original,
            application,
            parameters,
            inherited,
            imported,
        )
    }
    #[cfg(test)]
    pub(super) fn test_single_path(&self) -> &ConstraintPath<D> {
        assert_eq!(
            self.frontier.len(),
            1,
            "fixture has one current alternative"
        );
        &self.frontier[0]
    }

    /// A transaction always names an application already admitted on its path.
    /// Template opening must use that application, which may be a descendant
    /// of the path's root.
    pub(super) fn from_path<A: TypeConstraintAccounting>(
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        application: ConstraintApplicationId,
        path: ConstraintPath<D>,
        inherited: Option<&Arc<TypeConstraintSolution>>,
    ) -> Result<Self, TypeConstraintError> {
        Self::from_path_with_parent(context, graph, application, path, inherited, None)
    }

    fn from_path_with_parent<A: TypeConstraintAccounting>(
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        application: ConstraintApplicationId,
        path: ConstraintPath<D>,
        inherited: Option<&Arc<TypeConstraintSolution>>,
        parent_receipt: Option<ConstraintSourceReceipt<D>>,
    ) -> Result<Self, TypeConstraintError> {
        let path = Self::prepare_application(context, graph, application, path, inherited)?;
        let original = path
            .applications
            .require_application(application)?
            .original_context()
            .clone();
        Ok(Self {
            application,
            original,
            parent_receipt,
            frontier: vec![path],
            first_failure: None,
            next_equation: 0,
            next_source_ordinal: 0,
            first_cycle: None,
            probe: None,
            probe_group: None,
            prepared_sources: BTreeSet::new(),
            materialization: ConstraintMaterialization::new(graph.owner(context)?, context)?,
            closed: false,
        })
    }

    pub(crate) fn initialize_from_nested_path<A>(
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        original: super::OriginalProducerContext,
        application: D::Application,
        parameters: super::TypeConstraintParameterScope,
        inherited: Option<Arc<TypeConstraintSolution>>,
        nested: NestedConstraintPath<D>,
    ) -> Result<Self, super::TypeConstraintInitializationFailure>
    where
        A: TypeConstraintAccounting,
    {
        let result = (|| {
            let reads =
                context.compose_effect_reads(graph, nested.path.lineage.effect_read_scope())?;
            original.validate_imported_in(&nested.path.applications, context)?;
            let (mut transaction, reads) = context.with_application_scope(
                graph,
                reads.as_ref(),
                application,
                parameters,
                original,
                |context, graph, scope| {
                    let application = scope.id();
                    let path = context.admit_application(nested.path, scope, graph)?;
                    Self::from_path_with_parent(
                        context,
                        graph,
                        application,
                        path,
                        inherited.as_ref(),
                        Some(nested.receipt),
                    )
                },
            )?;
            transaction.frontier[0].lineage.effect_reads = reads;
            Ok(transaction)
        })();
        result.map_err(|error| match error {
            TypeConstraintError::Abort(error) => {
                super::TypeConstraintInitializationFailure::Abort(error)
            }
            TypeConstraintError::Invariant(error) => {
                super::TypeConstraintInitializationFailure::Invariant(error)
            }
            TypeConstraintError::Rejected(_) => {
                super::TypeConstraintInitializationFailure::Invariant(
                    super::TypeConstraintInvariant::InheritedSolution(
                        super::InheritedSolutionInvariant {
                            kind: super::InheritedSolutionInvariantKind::Forbidden,
                            parameter: None,
                        },
                    ),
                )
            }
        })
    }

    pub(crate) fn initialize<A>(
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        original: super::OriginalProducerContext,
        application: D::Application,
        parameters: super::TypeConstraintParameterScope,
        inherited: Option<Arc<TypeConstraintSolution>>,
    ) -> Result<Self, super::TypeConstraintInitializationFailure>
    where
        A: TypeConstraintAccounting,
    {
        Self::initialize_with_imported(
            context,
            graph,
            original,
            application,
            parameters,
            inherited,
            None,
        )
    }

    pub(crate) fn initialize_with_imported<A>(
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        original: super::OriginalProducerContext,
        application: D::Application,
        parameters: super::TypeConstraintParameterScope,
        inherited: Option<Arc<TypeConstraintSolution>>,
        imported: Option<super::ImportedGenericParameterScopeLease>,
    ) -> Result<Self, super::TypeConstraintInitializationFailure>
    where
        A: TypeConstraintAccounting,
    {
        match context.with_application_scope(
            graph,
            None,
            application,
            parameters,
            original.clone(),
            |context, graph, scope| {
                let application = scope.id();
                let path =
                    context.start_path_with_imported(scope, imported, original, graph.read())?;
                Self::from_path(context, graph, application, path, inherited.as_ref())
            },
        ) {
            Ok((transaction, _)) => Ok(transaction),
            Err(error) => Err(match error {
                TypeConstraintError::Abort(error) => {
                    super::TypeConstraintInitializationFailure::Abort(error)
                }
                TypeConstraintError::Invariant(error) => {
                    super::TypeConstraintInitializationFailure::Invariant(error)
                }
                TypeConstraintError::Rejected(_) => {
                    super::TypeConstraintInitializationFailure::Invariant(
                        super::TypeConstraintInvariant::InheritedSolution(
                            super::InheritedSolutionInvariant {
                                kind: super::InheritedSolutionInvariantKind::Forbidden,
                                parameter: None,
                            },
                        ),
                    )
                }
            }),
        }
    }

    pub(crate) fn constrain<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        effects: &mut crate::effect_row::prepared::PreparedEffectGraph,
        pattern: &TypeKind<PreparedEffects>,
        actual: &TypeKind<PreparedEffects>,
        acceptance: ConstraintAcceptance,
    ) {
        if let Err(error) = self.constrain_equation(
            context,
            effects,
            pattern,
            actual,
            acceptance,
            ConstraintEquationTiming::Immediate,
        ) {
            self.first_failure = Some(error.into());
        }
    }

    /// Retains one result equation for final validation while using a
    /// satisfiable relation to guide source inference. A contradictory result
    /// does not skip the candidate's authored source-checking protocol.
    pub(crate) fn constrain_result<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        effects: &mut crate::effect_row::prepared::PreparedEffectGraph,
        pattern: &TypeKind<PreparedEffects>,
        actual: &TypeKind<PreparedEffects>,
        acceptance: ConstraintAcceptance,
    ) {
        if let Err(error) = self.constrain_equation(
            context,
            effects,
            pattern,
            actual,
            acceptance,
            ConstraintEquationTiming::ResultContext,
        ) {
            self.first_failure = Some(error.into());
        }
    }

    fn constrain_equation<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        effects: &mut crate::effect_row::prepared::PreparedEffectGraph,
        pattern: &TypeKind<PreparedEffects>,
        actual: &TypeKind<PreparedEffects>,
        acceptance: ConstraintAcceptance,
        timing: ConstraintEquationTiming,
    ) -> Result<(), TypeConstraintError> {
        let original = self.original.clone();
        self.constrain_equation_from(
            context,
            effects,
            pattern,
            actual,
            acceptance,
            timing,
            |_, scope, effects, context| {
                ConstraintOperand::from_prepared_original_in(
                    actual,
                    scope,
                    original.clone(),
                    &mut operand::ConstraintEffectPhase::new(effects),
                    context,
                )
            },
        )
    }

    /// Equation processing consumes the producer's checked operand. Source
    /// admission and already-owned application terms share this consumer.
    fn constrain_equation_from<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        effects: &mut crate::effect_row::prepared::PreparedEffectGraph,
        pattern: &TypeKind<PreparedEffects>,
        actual: &TypeKind<PreparedEffects>,
        acceptance: ConstraintAcceptance,
        timing: ConstraintEquationTiming,
        mut admit: impl FnMut(
            &ConstraintPath<D>,
            Arc<super::lexical::ConstraintLexicalScope>,
            &mut crate::effect_row::prepared::PreparedEffectGraph,
            &mut TypeConstraintContext<'_, A, D>,
        ) -> Result<ConstraintOperand<D>, TypeConstraintError>,
    ) -> Result<(), TypeConstraintError> {
        if self.first_failure.is_some() || self.closed {
            return Ok(());
        }
        let timing = timing.for_actual(actual, acceptance, context)?;
        let ordinal = self.next_equation;
        self.next_equation = ordinal.checked_add(1).ok_or(TypeConstraintError::Abort(
            TypeConstraintAbort::ArithmeticOverflow,
        ))?;
        let frontier = core::mem::take(&mut self.frontier);
        let mut advanced = Vec::new();
        for mut path in frontier {
            let pattern = context.open_template_type(pattern, &path, self.application, effects)?;
            let scope = Arc::new(context.capture_lexical_scope()?);
            let pattern_operand = ConstraintOperand::template(
                pattern.clone(),
                Arc::clone(&scope),
                path.applications
                    .require_application_owner(self.application)?,
            );
            let actual_operand = admit(&path, scope, effects, context)?;
            path.equations.push(PendingEquation {
                ordinal: ConstraintEquationId {
                    application: self.application,
                    ordinal,
                },
                direction: acceptance,
                pattern: pattern_operand,
                actual: actual_operand,
                source_ordinal: None,
                timing,
            });
            if timing == ConstraintEquationTiming::InferredEffectResult {
                // Omitted effect annotations are completed from this actual
                // result at closure. They are not solver terms or evidence
                // that an unknown source row has been checked.
                advanced.push(path);
                continue;
            }
            let retained = match timing {
                ConstraintEquationTiming::Immediate => None,
                ConstraintEquationTiming::ResultContext
                | ConstraintEquationTiming::InferredEffectResult => {
                    let pattern_hint = project_type(
                        &pattern,
                        path.projection_view(),
                        ConstraintClosurePolicy::Hint,
                        effects,
                        context,
                    )?;
                    let actual_hint = project_type(
                        path.equations
                            .last()
                            .expect("the current equation is retained")
                            .actual
                            .value(),
                        path.projection_view(),
                        ConstraintClosurePolicy::Hint,
                        effects,
                        context,
                    )?;
                    if pattern_hint.remaining.is_empty() && actual_hint.remaining.is_empty() {
                        advanced.push(path);
                        continue;
                    }
                    // The same metered fork used by source alternatives
                    // protects the pre-inference row. No source authority is
                    // copied through an independent path or key constructor.
                    let inference = context.fork_path(&path)?;
                    Some(core::mem::replace(&mut path, inference))
                }
            };
            let equation = path
                .equations
                .last()
                .expect("the current equation is retained");
            let pattern = equation.pattern.clone();
            let actual = equation.actual.clone();
            match relate_selected_call(
                effects,
                &pattern.view(),
                &actual.view(),
                path,
                context,
                acceptance,
            ) {
                Ok(paths) if !paths.is_empty() => advanced.extend(paths),
                Ok(_) => {
                    if let Some(original) = retained {
                        advanced.push(original);
                    }
                }
                Err(TypeConstraintError::Rejected(_)) if retained.is_some() => {
                    advanced.push(retained.expect("a result inference row retains its obligation"));
                }
                Err(error) => return Err(error),
            }
        }
        if advanced.is_empty() {
            return Err(TypeConstraintRejection::Mismatch.into());
        }
        self.frontier = advanced;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn constrain_application_template<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        effects: &mut crate::effect_row::prepared::PreparedEffectGraph,
        pattern: &TypeKind<PreparedEffects>,
        actual: &TypeKind<PreparedEffects>,
        acceptance: ConstraintAcceptance,
    ) {
        let application = self.application;
        if let Err(error) = self.constrain_equation_from(
            context,
            effects,
            pattern,
            actual,
            acceptance,
            ConstraintEquationTiming::Immediate,
            |path, scope, graph, context| {
                let value = context.open_template_type(actual, path, application, graph)?;
                Ok(ConstraintOperand::template(
                    value,
                    scope,
                    path.applications.require_application_owner(application)?,
                ))
            },
        ) {
            self.first_failure = Some(error.into());
        }
    }

    /// Adds fixed row evidence to the same path-local effect environment used
    /// by function-type relations. Both directions are needed: a known row
    /// cannot grow to satisfy another use or collapse to the empty solution.
    pub(crate) fn constrain_effect_equality<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        left: &crate::effect_row::prepared::PreparedEffectRow,
        right: &crate::effect_row::prepared::PreparedEffectRow,
    ) {
        if self.first_failure.is_some() || self.closed {
            return;
        }
        let frontier = core::mem::take(&mut self.frontier);
        let mut advanced = Vec::with_capacity(frontier.len());
        let mut rejection = None;
        for mut path in frontier {
            let view = path.projection_view();
            let admission = (|| {
                for row in [left, right] {
                    graph.read().visit_row_parameters(
                        row,
                        context,
                        &mut |reference, context| {
                            context.validate_effect_reference(reference, view)
                        },
                    )?;
                }
                Ok::<_, TypeConstraintError>(())
            })();
            if let Err(error) = admission {
                self.first_failure = Some(error.into());
                return;
            }
            let constrained = path
                .effects
                .ambient
                .constrain_relation(
                    graph,
                    crate::effect_row::prepared::PreparedEffectRelation::Equal,
                    left,
                    right,
                    context,
                )
                .map_err(TypeConstraintError::from);
            match constrained {
                Ok(()) => advanced.push(path),
                Err(TypeConstraintError::Rejected(error)) => {
                    rejection.get_or_insert(error);
                }
                Err(error) => {
                    self.first_failure = Some(error.into());
                    return;
                }
            }
        }
        if advanced.is_empty() {
            self.first_failure = Some(
                TypeConstraintError::Rejected(
                    rejection.unwrap_or(TypeConstraintRejection::Mismatch),
                )
                .into(),
            );
        } else {
            self.frontier = advanced;
        }
    }

    pub(crate) fn request_projection<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        key: D::Projection,
        value: &TypeKind<PreparedEffects>,
        closure: TypeConstraintProjectionClosure,
    ) {
        self.request_projection_with_input(context, graph, key, value, closure, None);
    }

    /// Stage the template and its lexical input under the same graph edit.
    pub(crate) fn request_projection_with_input<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        key: D::Projection,
        value: &TypeKind<PreparedEffects>,
        closure: TypeConstraintProjectionClosure,
        input: Option<&TypeKind<PreparedEffects>>,
    ) {
        self.request_projection_with_options(
            context,
            graph,
            key,
            value,
            TypeConstraintProjectionOptions {
                closure,
                input,
                named_effect: None,
            },
        );
    }

    pub(crate) fn request_projection_with_options<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        key: D::Projection,
        value: &TypeKind<PreparedEffects>,
        options: TypeConstraintProjectionOptions<'_>,
    ) {
        let TypeConstraintProjectionOptions {
            closure,
            input,
            named_effect,
        } = options;
        if self.first_failure.is_none() && !self.closed {
            if self.probe.is_some() || self.probe_group.is_some() {
                self.record_failure(
                    protocol_error(TypeConstraintSourceProtocolInvariant::Outcome).into(),
                );
                return;
            }
            let staged = context.with_effect_transaction(graph, |context, graph| {
                context.enter_node()?;
                let key = Arc::new(key);
                let mut requests = Vec::new();
                for path in &self.frontier {
                    let request = context.with_effect_reads(
                        graph,
                        path.lineage.effect_read_scope(),
                        |context, graph| {
                            let input = input
                                .map(|input| {
                                    let scope = Arc::new(context.capture_lexical_scope()?);
                                    ConstraintOperand::from_prepared_original_in(
                                        input,
                                        scope,
                                        self.original.clone(),
                                        &mut operand::ConstraintEffectPhase::new(graph),
                                        context,
                                    )
                                    .map(Arc::new)
                                })
                                .transpose()?;
                            let value = ConstraintOperand::open_template(
                                value,
                                path,
                                self.application,
                                graph,
                                context,
                            )?;
                            context.enter_node()?;
                            Ok::<_, TypeConstraintError>(Arc::new(ProjectionRequest {
                                application: self.application,
                                key: Arc::clone(&key),
                                value,
                                closure,
                                source: None,
                                input,
                                named_effect: named_effect.cloned(),
                            }))
                        },
                    )?;
                    context.enter_node()?;
                    requests.push(request);
                }
                Ok::<_, TypeConstraintError>(requests)
            });
            match staged {
                Ok(requests) => {
                    for (path, (request, reads)) in self.frontier.iter_mut().zip(requests) {
                        path.lineage.effect_reads = reads;
                        path.lineage.projections.push(request);
                    }
                }
                Err(error) => self.record_failure(error.into()),
            }
        }
    }

    pub(crate) fn record_failure(&mut self, failure: TypeConstraintFailure<D>) {
        if self.closed {
            self.materialization.record_failure(failure);
            return;
        }
        if self.first_failure.is_none() {
            self.first_failure = Some(failure);
        }
        if self.probe.is_some() {
            self.probe = None;
            self.probe_group = None;
            self.frontier.clear();
        }
    }

    /// A producer without a remaining constraint path cannot demand its result.
    pub(crate) fn viable_result_source(
        &self,
        source: D::Source,
    ) -> Option<ConstraintSourceId<D::Source>> {
        (!self.closed && self.first_failure.is_none() && !self.frontier.is_empty())
            .then(|| ConstraintSourceId::new(self.application, source))
    }

    /// The only source entry point: a callable mapper supplies the complete
    /// prepared source constraint before any callback can run.
    pub(crate) fn begin_prepared_probe<A>(
        &mut self,
        _context: &mut TypeConstraintContext<'_, A, D>,
        prepared: PreparedSourceConstraint<D>,
        acceptance: ConstraintAcceptance,
    ) -> Result<ProbeStart, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        self.begin_probe_inner(prepared, acceptance)
    }

    pub(crate) fn begin_prepared_probe_group(
        &mut self,
        source_count: usize,
    ) -> Result<ProbeStart, TypeConstraintError> {
        if source_count == 0 || self.probe.is_some() || self.probe_group.is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        if self.first_failure.is_some() || self.closed {
            return Ok(ProbeStart::Skipped);
        }
        self.probe_group = Some(ProbeGroup {
            remaining_sources: source_count,
            last_live_frontier: self.frontier.clone(),
            deferred_failure: None,
            ordinary_rejection_pending: false,
        });
        Ok(ProbeStart::Started)
    }

    fn begin_probe_inner(
        &mut self,
        prepared: PreparedSourceConstraint<D>,
        acceptance: ConstraintAcceptance,
    ) -> Result<ProbeStart, TypeConstraintError> {
        if self.first_failure.is_some() || self.closed {
            if let Some(mut group) = self.probe_group.take() {
                group.remaining_sources =
                    group.remaining_sources.checked_sub(1).ok_or_else(|| {
                        protocol_error(TypeConstraintSourceProtocolInvariant::Outcome)
                    })?;
                if group.remaining_sources != 0 {
                    self.probe_group = Some(group);
                }
            }
            return Ok(ProbeStart::Skipped);
        }
        if self.probe.is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        prepared.validate()?;
        let source = ConstraintSourceId::new(self.application, prepared.source());
        if !self.prepared_sources.insert(source) {
            return Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::PreparedSource(
                    super::PreparedSourceConstraintInvariant::DuplicateCoordinate,
                ),
            ));
        }
        let equation_ordinal = if !prepared.is_unchecked() {
            let ordinal = self.next_equation;
            self.next_equation =
                self.next_equation
                    .checked_add(1)
                    .ok_or(TypeConstraintError::Abort(
                        TypeConstraintAbort::ArithmeticOverflow,
                    ))?;
            Some(ConstraintEquationId {
                application: self.application,
                ordinal,
            })
        } else {
            None
        };
        let source_ordinal = PreparedSourceOrdinal {
            application: self.application,
            ordinal: self.next_source_ordinal,
        };
        self.next_source_ordinal =
            self.next_source_ordinal
                .checked_add(1)
                .ok_or(TypeConstraintError::Abort(
                    TypeConstraintAbort::ArithmeticOverflow,
                ))?;
        let deferred_rejection_tail = self
            .probe_group
            .as_ref()
            .is_some_and(|group| group.ordinary_rejection_pending);
        let rows = if deferred_rejection_tail {
            self.probe_group
                .as_ref()
                .expect("a deferred rejection tail requires an active source group")
                .last_live_frontier
                .clone()
                .into()
        } else {
            if let Some(group) = self.probe_group.as_mut() {
                group.last_live_frontier = self.frontier.clone();
            }
            core::mem::take(&mut self.frontier).into()
        };
        self.probe = Some(ProbeOperation {
            source,
            source_ordinal,
            prepared: Arc::new(prepared),
            acceptance,
            equation_ordinal,
            rows,
            active_input: None,
            advanced: Vec::new(),
            rejections: Vec::new(),
            relation_rejections: Vec::new(),
            deferred_rejection_tail,
        });
        Ok(ProbeStart::Started)
    }

    pub(crate) fn next_probe<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    ) -> Result<Option<ProbeTicket<D>>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        let Some(operation) = self.probe.as_mut() else {
            return Ok(None);
        };
        if operation.active_input.is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        let Some(mut path) = operation.rows.pop_front() else {
            let operation = self.probe.take().expect("probe operation exists");
            if let Some(mut group) = self.probe_group.take() {
                group.remaining_sources =
                    group.remaining_sources.checked_sub(1).ok_or_else(|| {
                        protocol_error(TypeConstraintSourceProtocolInvariant::Outcome)
                    })?;
                // Fatal/abort/invariant callbacks set `first_failure` and
                // therefore outrank the deferred ordinary group rejection.
                // Only accepted/rejected ordinary tail results reach this
                // branch and neither may resurrect the semantic frontier.
                if self.first_failure.is_none() && !operation.deferred_rejection_tail {
                    if operation.advanced.is_empty() {
                        group.ordinary_rejection_pending = true;
                        group.deferred_failure = Some(operation.into_ordinary_rejection());
                    } else {
                        self.frontier = operation.advanced;
                        group.last_live_frontier = self.frontier.clone();
                    }
                }
                if group.remaining_sources == 0 {
                    if self.first_failure.is_none() && group.ordinary_rejection_pending {
                        self.first_failure = group.deferred_failure;
                    }
                } else {
                    self.probe_group = Some(group);
                }
            } else if operation.advanced.is_empty() && self.first_failure.is_none() {
                self.first_failure = Some(operation.into_ordinary_rejection());
            } else if self.first_failure.is_none() {
                self.frontier = operation.advanced;
            }
            return Ok(None);
        };
        let (opened_alternatives, effect_reads) = context.with_effect_reads(
            graph,
            path.lineage.effect_read_scope(),
            |context, graph| -> Result<_, TypeConstraintError> {
                let mut opened_alternatives = Vec::new();
                for alternative in operation.prepared.alternatives() {
                    let scope = Arc::new(context.capture_lexical_scope()?);
                    let expected = bind_source_expectation(
                        alternative.expectation(),
                        &self.original,
                        self.application,
                        &path,
                        scope,
                        graph,
                        context,
                    )?;
                    let (projected_hint, unbound) =
                        if expected.value().constraint_shape().is_closed_leaf() {
                            (None, Box::default())
                        } else {
                            let projected = project_type(
                                expected.value(),
                                path.projection_view(),
                                ConstraintClosurePolicy::Hint,
                                graph,
                                context,
                            )?;
                            let unbound = projected
                                .remaining
                                .iter()
                                .map(|parameter| parameter.parameter().clone())
                                .collect::<Box<[_]>>();
                            (Some(projected.value), unbound)
                        };
                    let scope_lease = if unbound.is_empty() {
                        None
                    } else {
                        Some(path.applications.import_parameters(&unbound, context)?)
                    };
                    opened_alternatives.push(OpenedSourceAlternative {
                        operand: expected,
                        projected_hint,
                        unbound,
                        scope_lease,
                    });
                }
                Ok(opened_alternatives)
            },
        )?;
        path.lineage.effect_reads = effect_reads;
        let input = Arc::new(ProbeInput {
            source: operation.source,
            prepared: Arc::clone(&operation.prepared),
            opened_alternatives,
            acceptance: operation.acceptance,
            equation_ordinal: operation.equation_ordinal,
        });
        operation.active_input = Some(Arc::clone(&input));
        Ok(Some(ProbeTicket {
            input,
            path: Some(path),
        }))
    }

    pub(crate) fn submit_probe<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        input: Arc<ProbeInput<D>>,
        submission: ProbeSubmission<D>,
    ) -> Result<Vec<ResolvedResultProbe<D>>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        let operation = self
            .probe
            .as_mut()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Ticket))?;
        if !operation
            .active_input
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, &input))
        {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        if let ProbeSubmission::Accepted(contribution) = &submission
            && !Arc::ptr_eq(&input, &contribution.input)
        {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Ticket,
            ));
        }
        let mut observed_results = Vec::new();
        match submission {
            ProbeSubmission::Rejected(cause) => {
                operation.active_input = None;
                operation.rejections.push(cause);
            }
            ProbeSubmission::Accepted(contribution) => {
                let alternatives = contribution.admit_preparation(graph, context)?;
                operation.active_input = None;
                for ObservedProbeAlternative { mut path, result } in alternatives {
                    let (actual_term, branch, callback_selection) = result.into_parts();
                    let (accepted, reads) = context.with_effect_reads(
                        graph,
                        path.lineage.effect_read_scope(),
                        |context, graph| -> Result<_, TypeConstraintError> {
                            let scope = Arc::new(context.capture_lexical_scope()?);
                            let actual_operand = resolve_probe_term(
                                actual_term,
                                &path,
                                &scope,
                                &self.original,
                                ConstraintSourceReceipt {
                                    input: Arc::clone(&input),
                                },
                                graph,
                                context,
                            )?;
                            if let Some(actual) = actual_operand.shared_result_observation() {
                                context.enter_node()?;
                                observed_results.push(ResolvedResultProbe {
                                    source: input.source,
                                    actual: Arc::clone(actual),
                                });
                            }
                            let actual = actual_operand.value().clone();
                            if path
                                .lineage
                                .probe_trace
                                .iter()
                                .any(|probe| probe.source() == input.source)
                            {
                                return Err(protocol_error(
                                    TypeConstraintSourceProtocolInvariant::Outcome,
                                ));
                            }

                            let selected = if input.prepared.is_unchecked() {
                                if !matches!(callback_selection, SourceProbeSelection::Unchecked) {
                                    return Err(protocol_error(
                                        TypeConstraintSourceProtocolInvariant::InvalidEvidence,
                                    ));
                                }
                                None
                            } else {
                                let SourceProbeSelection::Checked {
                                    alternative,
                                    evidence,
                                } = &callback_selection
                                else {
                                    return Err(protocol_error(
                                        TypeConstraintSourceProtocolInvariant::UnknownAlternative,
                                    ));
                                };
                                match validate_source_selection(
                                    &input,
                                    *alternative,
                                    Arc::clone(evidence),
                                    &actual,
                                )? {
                                    Some(selected) => Some(selected),
                                    None => return Ok(None),
                                }
                            };

                            let (pattern, stored_selection, source_projection, value_expected) =
                                match selected {
                                    None => {
                                        let Some(source_projection) =
                                            CheckedConstraintSourceProjection::derive(
                                                input.prepared.source_projection(),
                                                &actual,
                                            )
                                        else {
                                            return Ok(None);
                                        };
                                        (
                                            None,
                                            StoredSourceSelection::Unchecked,
                                            source_projection,
                                            None,
                                        )
                                    }
                                    Some((
                                        alternative,
                                        evidence,
                                        value_expected,
                                        source_projection,
                                    )) => {
                                        let pattern = source_projection.compose_operand(
                                            &actual_operand.view(),
                                            &value_expected.view(),
                                        )?;
                                        (
                                            Some(pattern),
                                            StoredSourceSelection::Checked {
                                                alternative,
                                                evidence,
                                            },
                                            source_projection,
                                            Some(value_expected.clone()),
                                        )
                                    }
                                };

                            Ok(Some((
                                actual_operand,
                                actual,
                                pattern,
                                stored_selection,
                                source_projection,
                                value_expected,
                            )))
                        },
                    )?;
                    let Some((
                        actual_operand,
                        actual,
                        pattern,
                        stored_selection,
                        source_projection,
                        value_expected,
                    )) = accepted
                    else {
                        continue;
                    };
                    path.lineage.effect_reads = reads;
                    if let Some(pattern) = &pattern {
                        path.equations.push(PendingEquation {
                            ordinal: input.equation_ordinal.ok_or_else(|| {
                                protocol_error(TypeConstraintSourceProtocolInvariant::Outcome)
                            })?,
                            direction: input.acceptance,
                            pattern: pattern.clone(),
                            actual: actual_operand.clone(),
                            source_ordinal: Some(operation.source_ordinal),
                            timing: ConstraintEquationTiming::Immediate,
                        });
                    }
                    let rejected_alternative = match &stored_selection {
                        StoredSourceSelection::Checked { alternative, .. } => Some(*alternative),
                        StoredSourceSelection::Unchecked => None,
                    };
                    let diagnostic_operands = pattern
                        .as_ref()
                        .filter(|_| D::observes_rejected_source(input.source.local()))
                        .map(|expected| (expected, actual_operand.clone()));
                    let probe = ActiveConstraintProbe {
                        source: input.source,
                        source_ordinal: operation.source_ordinal,
                        branch: Arc::clone(&branch),
                        selection: stored_selection,
                        prepared_source_projection: input.prepared.source_projection(),
                        value_expected,
                        actual: actual_operand,
                    };
                    path.lineage.probe_trace.push(probe);

                    let reads = path.lineage.effect_reads.clone();
                    let related = if pattern.is_some() {
                        let equation = path
                            .equations
                            .last()
                            .expect("a checked source retains its equation");
                        let expected = equation.pattern.clone();
                        let actual = equation.actual.clone();
                        // Closed leaves have no effect terms to look up. The
                        // path keeps its admitted reads for subsequent inputs.
                        let needs_effect_reads =
                            !expected.value().constraint_shape().is_closed_leaf()
                                || !actual.value().constraint_shape().is_closed_leaf();
                        let relate = |graph: &mut crate::effect_row::prepared::PreparedEffectGraph, context: &mut TypeConstraintContext<'_, A, D>| {
                            relate_selected_call(graph, &expected.view(), &actual.view(), path, context, input.acceptance)
                        };
                        if let Some(reads) = &reads
                            && needs_effect_reads
                        {
                            graph
                                .compute_in_read_scope(reads, context, relate)
                                .and_then(|(mut paths, reads)| {
                                    for path in &mut paths {
                                        context.enter_node()?;
                                        path.lineage.effect_reads = Some(reads.clone());
                                    }
                                    Ok(paths)
                                })
                        } else {
                            relate(graph, context)
                        }
                    } else {
                        let read = match &reads {
                            Some(reads) => graph.read_scoped(reads, context)?,
                            None => graph.read(),
                        };
                        validate_type(&actual, path.projection_view(), read, context)
                            .map(|()| vec![path])
                    };
                    match related {
                        Ok(paths) if !paths.is_empty() => operation.advanced.extend(paths),
                        Ok(_)
                        | Err(TypeConstraintError::Rejected(TypeConstraintRejection::Mismatch)) => {
                            if let Some((expected, actual)) = diagnostic_operands {
                                let read = match &reads {
                                    Some(reads) => graph.read_scoped(reads, context)?,
                                    None => graph.read(),
                                };
                                let observed = crate::types::PreparedTypeEffectProjectionFailure::optional_observation(
                                    expected.value().observe_compatibility_inputs(
                                        actual.value(), expected.scope().shape(), read, context,
                                    ),
                                )?;
                                if let Some((expected, actual)) = observed {
                                    let projection = CheckedConstraintSourceProjection::derive(
                                        input.prepared.source_projection(),
                                        &actual,
                                    )
                                    .ok_or_else(|| {
                                        protocol_error(
                                            TypeConstraintSourceProtocolInvariant::Outcome,
                                        )
                                    })?;
                                    operation.relation_rejections.push(
                                        RejectedConstraintSourceProjection::new(
                                            input.source,
                                            rejected_alternative,
                                            projection,
                                            input.acceptance,
                                            expected,
                                            actual,
                                        ),
                                    );
                                }
                            }
                        }
                        Err(error) => {
                            self.first_failure = Some(error.into());
                            operation.rows.clear();
                            break;
                        }
                    }
                }
            }
        }
        Ok(observed_results)
    }

    pub(crate) fn next_materialization_ticket<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    ) -> Result<Option<MaterializationTicket<D>>, MaterializationImmediateFailure<D>> {
        context
            .check_cancelled()
            .map_err(materialization_immediate)?;
        self.materialization
            .validate_graph(graph.read())
            .map_err(materialization_immediate)?;
        if !self.closed {
            match self.close(context, graph) {
                Ok(()) => {}
                Err(TypeConstraintError::Rejected(_)) => return Ok(None),
                Err(error) => return Err(materialization_immediate(error)),
            }
        }
        self.materialization.next_materialization_ticket(context)
    }

    pub(crate) fn validate_materialization_callback_begin(
        &self,
        ticket: &MaterializationTicket<D>,
    ) -> Result<(), TypeConstraintSourceProtocolInvariant> {
        self.materialization
            .validate_materialization_callback_begin(ticket)
    }

    pub(crate) fn submit_closed_materialization(
        &mut self,
        ticket: MaterializationTicket<D>,
        closed: ClosedMaterialization<D>,
    ) -> Result<(), MaterializationImmediateFailure<D>> {
        self.materialization
            .submit_closed_materialization(ticket, closed)
    }

    pub(crate) fn finish<A>(
        self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        compare: impl FnMut(
            &D::SealedBranchValue,
            &D::SealedBranchValue,
            &mut A,
        ) -> Result<bool, TypeConstraintFailure<D>>,
    ) -> Result<SolvedCandidate<D>, TypeConstraintFailure<D>>
    where
        A: TypeConstraintAccounting,
    {
        self.finish_alternatives(context, graph, compare)?
            .into_unique()
    }

    pub(crate) fn finish_alternatives<A>(
        mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        compare: impl FnMut(
            &D::SealedBranchValue,
            &D::SealedBranchValue,
            &mut A,
        ) -> Result<bool, TypeConstraintFailure<D>>,
    ) -> Result<super::CompletedCandidateAlternatives<D>, TypeConstraintFailure<D>>
    where
        A: TypeConstraintAccounting,
    {
        match self.close(context, graph) {
            Ok(()) => self.finish_closed_alternatives(context, compare),
            Err(error) => Err(error.into()),
        }
    }

    /// Metadata preparation borrows the graph; sealed comparison runs after
    /// that loan ends, so the source client is never borrowed twice.
    pub(crate) fn finish_closed_alternatives<A>(
        self,
        context: &mut TypeConstraintContext<'_, A, D>,
        compare: impl FnMut(
            &D::SealedBranchValue,
            &D::SealedBranchValue,
            &mut A,
        ) -> Result<bool, TypeConstraintFailure<D>>,
    ) -> Result<super::CompletedCandidateAlternatives<D>, TypeConstraintFailure<D>>
    where
        A: TypeConstraintAccounting,
    {
        self.into_materialization()?.finish(context, compare)
    }

    pub(crate) fn into_materialization(
        mut self,
    ) -> Result<ConstraintMaterialization<D>, TypeConstraintFailure<D>> {
        if !self.closed {
            if let Some(error) = self.first_failure.take() {
                return Err(error);
            }
            return Err(protocol_error(TypeConstraintSourceProtocolInvariant::Outcome).into());
        }
        if let Some(error) = self.first_failure.take() {
            self.materialization.record_failure(error);
        }
        self.materialization.retain_cycle(self.first_cycle.take());
        Ok(self.materialization)
    }

    #[cfg(test)]
    pub(crate) fn materialization_owner(&self) -> &ConstraintMaterialization<D> {
        &self.materialization
    }

    pub(crate) fn close<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    ) -> Result<(), TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        context.with_effect_transaction(graph, |context, graph| self.close_in(context, graph))
    }

    fn close_in<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    ) -> Result<(), TypeConstraintError> {
        if self.closed {
            return Ok(());
        }
        if self.probe.is_some() || self.probe_group.is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        self.closed = true;
        if self.first_failure.is_some() {
            return Ok(());
        }
        let result = self.close_frontier(context, graph);
        if let Err(error) = &result {
            self.record_failure(error.clone().into());
            self.materialization.discard_pending();
        }
        result
    }

    fn close_frontier<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    ) -> Result<(), TypeConstraintError> {
        let frontier = core::mem::take(&mut self.frontier);
        let mut acyclic = Vec::new();
        let mut completion_rejection = None;
        for path in frontier {
            match self.normalize_path(path, graph, context) {
                Ok(NormalizedPath::Acyclic(path)) => acyclic.push(path),
                Ok(NormalizedPath::Cyclic(parameter)) => {
                    if self.first_cycle.is_none() {
                        self.first_cycle = Some(parameter);
                    }
                }
                Err(TypeConstraintError::Rejected(rejection)) => {
                    completion_rejection.get_or_insert(rejection);
                }
                Err(error) => return Err(error),
            }
        }
        if acyclic.is_empty() {
            return completion_rejection.map_or(Ok(()), |rejection| Err(rejection.into()));
        }
        self.first_cycle = None;
        acyclic.sort_by(path_correlation_cmp::<D>);

        let mut groups: Vec<Vec<MaterializationInput<D>>> = Vec::new();
        for path in acyclic {
            let input = match Self::complete_component(&path, self.application, graph, context) {
                Ok(component) => component,
                Err(TypeConstraintError::Rejected(rejection)) => {
                    completion_rejection.get_or_insert(rejection);
                    continue;
                }
                Err(error) => return Err(error),
            };
            let mut group_index = None;
            for (index, group) in groups.iter().enumerate() {
                if let Some(first) = group.first()
                    && first.component.equal_with(&input.component, context)?
                {
                    group_index = Some(index);
                    break;
                }
            }
            if let Some(index) = group_index {
                groups[index].push(input);
            } else {
                groups.push(vec![input]);
            }
        }
        if groups.is_empty()
            && let Some(rejection) = completion_rejection
        {
            return Err(rejection.into());
        }
        for input in groups.into_iter().flatten() {
            self.materialization.push_component(input, context)?;
        }
        Ok(())
    }

    fn normalize_path<A>(
        &mut self,
        path: ConstraintPath<D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<NormalizedPath<D>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        let cyclic = path.deferred_cycles.parameters.iter().next().cloned();
        let reads = context.compose_effect_reads(graph, path.lineage.effect_read_scope())?;
        let (mut path, reads) =
            context.with_effect_read_scope(graph, reads.as_ref(), |context, graph| {
                seal_path(path, graph, context)
            })?;
        path.lineage.effect_reads = reads;
        if let Some(parameter) = cyclic {
            if self.first_cycle.is_none() {
                self.first_cycle = Some(parameter.clone());
            }
            return Ok(NormalizedPath::Cyclic(parameter));
        }
        Ok(NormalizedPath::Acyclic(path))
    }

    fn complete_component<A: TypeConstraintAccounting>(
        path: &ConstraintPath<D>,
        selected: ConstraintApplicationId,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<MaterializationInput<D>, TypeConstraintError> {
        context.with_original_scope(path.scope.clone(), |context| {
            context.validate_in_effect_read_scope(
                graph,
                path.lineage.effect_reads.as_ref(),
                |context, graph| {
                    let effects = path.close_original_path_effects(graph, context)?;
                    MaterializationInput::from_closed_original(
                        path,
                        selected,
                        path,
                        &mut effects.projection(),
                        graph.read(),
                        context,
                    )
                    .map_err(TypeConstraintError::from)
                },
            )
        })
    }

    fn prepare_application<A>(
        context: &mut TypeConstraintContext<'_, A, D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        application: ConstraintApplicationId,
        path: ConstraintPath<D>,
        inherited: Option<&Arc<TypeConstraintSolution>>,
    ) -> Result<ConstraintPath<D>, TypeConstraintError>
    where
        A: TypeConstraintAccounting,
    {
        context.check_cancelled()?;
        let applications = Arc::clone(&path.applications);
        let scope = applications.require_application(application)?;
        let Some(inherited) = inherited else {
            if let Some(parameter) = scope.parameters().required_inherited_keys().first() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                        kind: InheritedSolutionInvariantKind::Unclosed,
                        parameter: Some(parameter.clone().into()),
                    }),
                ));
            }
            if let Some(parameter) = scope.parameters().required_inherited_const_keys().first() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                        kind: InheritedSolutionInvariantKind::Unclosed,
                        parameter: Some(parameter.clone().into()),
                    }),
                ));
            }
            if let Some(variable) = scope.effects().required_inherited().first() {
                return Err(super::effect_invariant(
                    super::TypeConstraintEffectInvariantKind::MissingInherited,
                    Some(variable.clone()),
                ));
            }
            return Ok(path);
        };
        inherited.restore_inherited_path(application, path, graph, context)
    }
}

fn materialization_immediate<D: ConstraintDomain>(
    error: TypeConstraintError,
) -> MaterializationImmediateFailure<D> {
    match error {
        TypeConstraintError::Abort(error) => MaterializationImmediateFailure::Abort(error),
        TypeConstraintError::Invariant(error) => MaterializationImmediateFailure::Invariant(
            super::TypeConstraintFailureInvariant::Constraint(error),
        ),
        TypeConstraintError::Rejected(_) => MaterializationImmediateFailure::Invariant(
            super::TypeConstraintFailureInvariant::Constraint(
                TypeConstraintInvariant::SourceProtocol(
                    TypeConstraintSourceProtocolInvariant::Outcome,
                ),
            ),
        ),
    }
}

fn projection_error(error: TypeConstraintError) -> TypeConstraintError {
    match error {
        TypeConstraintError::Rejected(rejection) => {
            TypeConstraintError::Invariant(TypeConstraintInvariant::Projection(
                TypeConstraintProjectionInvariant::Mismatch(rejection),
            ))
        }
        TypeConstraintError::Abort(_) | TypeConstraintError::Invariant(_) => error,
    }
}

fn resolve_probe_term<A: TypeConstraintAccounting, D: ConstraintDomain>(
    term: SourceProbeTerm<D>,
    path: &ConstraintPath<D>,
    target: &Arc<super::lexical::ConstraintLexicalScope>,
    original: &super::OriginalProducerContext,
    receipt: ConstraintSourceReceipt<D>,
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintOperand<D>, TypeConstraintError> {
    match term {
        SourceProbeTerm::Value(_) => Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Outcome,
        )),
        SourceProbeTerm::AdmittedValue(actual) => {
            actual.require_source_receipt(&receipt)?;
            if actual.scope() != target.as_ref() {
                return Err(TypeConstraintInvariant::OriginalProducer.into());
            }
            Ok(actual)
        }
        #[cfg(test)]
        SourceProbeTerm::Type(value) => Ok(ConstraintOperand::source(
            value,
            Arc::clone(target),
            original.clone(),
            receipt,
            ResolvedConstraintSource::Fixture,
        )),
        SourceProbeTerm::Result(result) => {
            let (actual, result) = ResolvedConstraintResult::resolve(result, path, graph, context)?;
            Ok(ConstraintOperand::source(
                actual,
                Arc::clone(target),
                original.clone(),
                receipt,
                ResolvedConstraintSource::Result(result),
            ))
        }
    }
}

mod result_port;
pub(crate) use operand::{ConstraintResultUseSource, PreparedConstraintFunctionScheme};
mod suspension;
pub(crate) use suspension::SuspendedConstraintFrontier;
mod component;
pub(crate) use component::{
    OpenConstraintComponent, PreparedValueFailure, PreparedValueInput, PreparedValueOutcome,
    PreparedValueOutput, PreparedValueRef, PreparedValueUnavailable, PreparedValueUse,
    PreparedValueUseAlternative, PreparedValueUseTicket,
};

fn bind_source_expectation<A: TypeConstraintAccounting, D: ConstraintDomain>(
    expected: &PreparedSourceExpectation,
    original: &super::OriginalProducerContext,
    application: ConstraintApplicationId,
    path: &ConstraintPath<D>,
    scope: Arc<super::lexical::ConstraintLexicalScope>,
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintOperand<D>, TypeConstraintError> {
    match expected {
        PreparedSourceExpectation::Declaration(ty) => {
            let value = context.open_template_type(ty, path, application, graph)?;
            Ok(ConstraintOperand::template(
                value,
                scope,
                path.applications.require_application_owner(application)?,
            ))
        }
        PreparedSourceExpectation::Original(ty) => ConstraintOperand::from_prepared_original_in(
            ty,
            scope,
            original.clone(),
            &mut operand::ConstraintEffectPhase::new(graph),
            context,
        ),
    }
}

fn validate_source_selection<'a, D: ConstraintDomain>(
    input: &'a ProbeInput<D>,
    selected: D::AlternativeIndex,
    evidence: Arc<D::ObservedEvidence>,
    actual: &TypeKind<PreparedEffects>,
) -> Result<
    Option<(
        D::AlternativeIndex,
        Arc<D::ObservedEvidence>,
        &'a ConstraintOperand<D>,
        CheckedConstraintSourceProjection<PreparedEffects>,
    )>,
    TypeConstraintError,
> {
    let prepared = &input.prepared;
    let (selected_row, opened) = input
        .alternatives()
        .find(|(row, _)| row.alternative() == selected)
        .ok_or(protocol_error(
            TypeConstraintSourceProtocolInvariant::UnknownAlternative,
        ))?;
    if !D::evidence_accepts(selected_row.evidence(), &evidence) {
        return Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::InvalidEvidence,
        ));
    }
    let otherwise = prepared
        .otherwise()
        .expect("checked source retains mandatory otherwise");
    let mut first_guarded = None;
    let mut guarded_matches = 0_u32;
    for row in prepared.alternatives() {
        if !D::evidence_accepts(row.evidence(), &evidence) {
            continue;
        }
        if row.alternative() == otherwise.alternative() {
            continue;
        }
        guarded_matches = guarded_matches.saturating_add(1);
        if first_guarded.is_none() {
            first_guarded = Some(row.alternative());
        }
    }
    let selected_is_otherwise = selected == otherwise.alternative();
    if guarded_matches > 1 {
        return Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::InvalidEvidence,
        ));
    }
    if let Some(first) = first_guarded {
        if selected_is_otherwise || first != selected {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::InvalidEvidence,
            ));
        }
    } else if !selected_is_otherwise {
        return Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::InvalidEvidence,
        ));
    }
    let Some(source_projection) =
        CheckedConstraintSourceProjection::derive(prepared.source_projection(), actual)
    else {
        return Ok(None);
    };
    Ok(Some((
        selected,
        evidence,
        &opened.operand,
        source_projection,
    )))
}

fn ensure_unique_sources<D: ConstraintDomain>(
    path: &ConstraintPath<D>,
) -> Result<(), TypeConstraintError> {
    let mut sources = BTreeSet::new();
    if path
        .lineage
        .probe_trace
        .iter()
        .any(|probe| !sources.insert(probe.source()))
    {
        Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket,
        ))
    } else {
        Ok(())
    }
}

fn unique_request_ordinal<D: ConstraintDomain>(
    requests: &[ClosedConstraintProbe<D>],
    source: ConstraintSourceId<D::Source>,
) -> Result<MaterializationSourceOrdinal, TypeConstraintError> {
    let mut matches = requests
        .iter()
        .enumerate()
        .filter(|(_, request)| request.source() == source)
        .map(|(ordinal, _)| ordinal);
    let Some(ordinal) = matches.next() else {
        return Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket,
        ));
    };
    if matches.next().is_some() {
        return Err(protocol_error(
            TypeConstraintSourceProtocolInvariant::Ticket,
        ));
    }
    Ok(MaterializationSourceOrdinal(
        u32::try_from(ordinal)
            .map_err(|_| TypeConstraintError::Abort(TypeConstraintAbort::ArithmeticOverflow))?,
    ))
}
fn path_correlation_cmp<D: ConstraintDomain>(
    left: &ConstraintPath<D>,
    right: &ConstraintPath<D>,
) -> std::cmp::Ordering {
    left.choice_key
        .cmp(&right.choice_key)
        .then_with(|| {
            left.lineage
                .probe_trace
                .iter()
                .map(|probe| (probe.ordinal(), probe.source()))
                .cmp(
                    right
                        .lineage
                        .probe_trace
                        .iter()
                        .map(|probe| (probe.ordinal(), probe.source())),
                )
        })
        .then_with(|| {
            left.lineage
                .probe_trace
                .len()
                .cmp(&right.lineage.probe_trace.len())
        })
}

fn protocol_error(kind: TypeConstraintSourceProtocolInvariant) -> TypeConstraintError {
    TypeConstraintError::Invariant(TypeConstraintInvariant::SourceProtocol(kind))
}
