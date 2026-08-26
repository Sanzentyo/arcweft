//! Checker-owned callable facts and public semantic signature results.

use std::{collections::HashSet, sync::Arc};

use arcweft_lang_hir::{
    expr::{HirAssociatedSeparator, HirCallArgumentOrdinal},
    identity::{ExprId, TypeId},
    source_index::{HirExprSourceRole, HirSourceQuery},
    symbol::CallableDeclarationKey,
};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::{effect_row::EffectRow, types::TypeKind};

use super::{
    CallResolverAccountingReport, CallableArgumentSlotIndex, CallableCandidateId,
    CallableDiagnosticCode, CallableDocumentation, CallableGroupIndex, CallableGroupKind,
    CallableLimits, CallableName, CallableParameterAdmission, CallableParameterCoordinate,
    CallableParameterPassing, CallableParameterPresence, CallableParameterSource,
    CallableQueryLimitError, CallableSource, CheckedCallApplication, CheckedCallOperandDestination,
    CheckedCallSite, NonCallableSource, ResolvedCallable, SemanticSignatureError, SignatureOrigin,
    SignatureQueryWorkReport, SignatureWorkReport, UnknownCallKind,
};

/// Immutable semantic facts committed for one checked call expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallTargetFacts {
    enclosing_callable: Option<CallableDeclarationKey>,
    outcome: CallAnalysisOutcome,
    diagnostics: Arc<[CallableDiagnostic]>,
    accounting: CallResolverAccountingReport,
}

/// Project-aware semantic classification committed for one final-HIR Call.
///
/// Structural and source evidence remains owned by the immutable HIR Call.
/// This fact retains only the qualified semantic receiver identity selected by
/// value-first/nominal-second classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallCalleeClassificationFact {
    Value {
        expression: ExprId,
    },
    AssociatedType {
        receiver: TypeId,
        separator: HirAssociatedSeparator,
    },
}

/// One complete checked call outcome.
///
/// A selected result owns the sole final application authority.  Unselected
/// variants own only tooling evidence; they cannot expose execution, a result
/// type, effects, or a continuation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallAnalysisOutcome {
    Selected(CheckedCallApplication),
    Ambiguous(CheckedAmbiguousCallEvidence),
    Rejected(CheckedRejectedCallEvidence),
    NonCallable(CheckedNonCallableEvidence),
    Missing(CheckedMissingCallEvidence),
}

impl CallAnalysisOutcome {
    pub fn site(&self) -> CheckedCallSite {
        match self {
            Self::Selected(application) => application.core().site(),
            Self::Ambiguous(evidence) => evidence.site(),
            Self::Rejected(evidence) => evidence.site(),
            Self::NonCallable(evidence) => evidence.site(),
            Self::Missing(evidence) => evidence.site(),
        }
    }

    pub fn primary_candidate_id(&self) -> Option<&CallableCandidateId> {
        match self {
            Self::Selected(application) => Some(application.core().candidates().selected().id()),
            Self::Ambiguous(evidence) => evidence
                .candidates()
                .first()
                .map(|candidate| candidate.id()),
            Self::Rejected(evidence) => evidence
                .candidates()
                .first()
                .map(|candidate| candidate.id()),
            Self::NonCallable(_) | Self::Missing(_) => None,
        }
    }

    pub fn candidate_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = &CallableCandidateId> + DoubleEndedIterator {
        let candidates: &[Arc<ResolvedCallable>] = match self {
            Self::Selected(application) => application.core().candidates().candidates(),
            Self::Ambiguous(evidence) => evidence.candidates(),
            Self::Rejected(evidence) => evidence.candidates(),
            Self::NonCallable(_) | Self::Missing(_) => &[],
        };
        candidates.iter().map(|candidate| candidate.id())
    }

    pub fn considered_candidate_ids(
        &self,
    ) -> impl ExactSizeIterator<Item = &CallableCandidateId> + DoubleEndedIterator {
        let candidates: &[Arc<ResolvedCallable>] = match self {
            Self::Selected(application) => application.core().candidates().candidates(),
            Self::Ambiguous(evidence) => evidence.considered(),
            Self::Rejected(evidence) => evidence.candidates(),
            Self::NonCallable(_) | Self::Missing(_) => &[],
        };
        candidates.iter().map(|candidate| candidate.id())
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Selected(application) => application.visit_types(visitor),
            Self::Ambiguous(evidence) => evidence.visit_types(visitor),
            Self::Rejected(evidence) => evidence.visit_types(visitor),
            Self::NonCallable(evidence) => visitor(evidence.ty()),
            Self::Missing(_) => Ok(()),
        }
    }
}

/// Final tooling evidence for an unresolved overload tie.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedAmbiguousCallEvidence {
    site: CheckedCallSite,
    callee: Option<CallCalleeClassificationFact>,
    candidates: Arc<[Arc<ResolvedCallable>]>,
    considered: Arc<[Arc<ResolvedCallable>]>,
}

impl CheckedAmbiguousCallEvidence {
    pub(crate) fn seal(
        site: CheckedCallSite,
        callee: Option<CallCalleeClassificationFact>,
        candidates: Vec<Arc<ResolvedCallable>>,
        considered: Vec<Arc<ResolvedCallable>>,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError> {
        validate_callee_for_site(callee, site)?;
        let candidates = seal_unselected_candidates(candidates, limits)?;
        let considered = seal_unselected_candidates(considered, limits)?;
        if candidates.len() < 2
            || candidates.iter().any(|candidate| {
                !considered
                    .iter()
                    .any(|row| row.digest() == candidate.digest())
            })
        {
            return Err(SemanticSignatureError::DuplicateCandidate);
        }
        Ok(Self {
            site,
            callee,
            candidates,
            considered,
        })
    }

    pub const fn site(&self) -> CheckedCallSite {
        self.site
    }

    pub const fn callee(&self) -> Option<CallCalleeClassificationFact> {
        self.callee
    }

    pub fn candidates(&self) -> &[Arc<ResolvedCallable>] {
        &self.candidates
    }

    pub fn considered(&self) -> &[Arc<ResolvedCallable>] {
        &self.considered
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        for candidate in self.candidates() {
            candidate.visit_types(visitor)?;
        }
        for candidate in self.considered() {
            candidate.visit_types(visitor)?;
        }
        Ok(())
    }
}

/// Final tooling evidence for a bounded candidate set with no accepted row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRejectedCallEvidence {
    site: CheckedCallSite,
    callee: Option<CallCalleeClassificationFact>,
    candidates: Arc<[Arc<ResolvedCallable>]>,
}

impl CheckedRejectedCallEvidence {
    pub(crate) fn seal(
        site: CheckedCallSite,
        callee: Option<CallCalleeClassificationFact>,
        candidates: Vec<Arc<ResolvedCallable>>,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError> {
        validate_callee_for_site(callee, site)?;
        let candidates = seal_unselected_candidates(candidates, limits)?;
        if candidates.is_empty() {
            return Err(SemanticSignatureError::DuplicateCandidate);
        }
        Ok(Self {
            site,
            callee,
            candidates,
        })
    }

    pub const fn site(&self) -> CheckedCallSite {
        self.site
    }

    pub const fn callee(&self) -> Option<CallCalleeClassificationFact> {
        self.callee
    }

    pub fn candidates(&self) -> &[Arc<ResolvedCallable>] {
        &self.candidates
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        for candidate in self.candidates() {
            candidate.visit_types(visitor)?;
        }
        Ok(())
    }
}

/// Final tooling evidence for a resolved value that is not callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedNonCallableEvidence {
    site: CheckedCallSite,
    callee: Option<CallCalleeClassificationFact>,
    source: NonCallableSource,
    ty: TypeKind,
}

impl CheckedNonCallableEvidence {
    pub(crate) fn seal(
        site: CheckedCallSite,
        callee: Option<CallCalleeClassificationFact>,
        source: NonCallableSource,
        ty: TypeKind,
    ) -> Result<Self, SemanticSignatureError> {
        validate_callee_for_site(callee, site)?;
        Ok(Self {
            site,
            callee,
            source,
            ty,
        })
    }

    pub const fn site(&self) -> CheckedCallSite {
        self.site
    }

    pub const fn callee(&self) -> Option<CallCalleeClassificationFact> {
        self.callee
    }

    pub const fn source(&self) -> &NonCallableSource {
        &self.source
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}

/// Final tooling evidence for an unresolved call target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMissingCallEvidence {
    site: CheckedCallSite,
    callee: Option<CallCalleeClassificationFact>,
    kind: UnknownCallKind,
}

impl CheckedMissingCallEvidence {
    pub(crate) fn seal(
        site: CheckedCallSite,
        callee: Option<CallCalleeClassificationFact>,
        kind: UnknownCallKind,
    ) -> Result<Self, SemanticSignatureError> {
        validate_callee_for_site(callee, site)?;
        Ok(Self { site, callee, kind })
    }

    pub const fn site(&self) -> CheckedCallSite {
        self.site
    }

    pub const fn callee(&self) -> Option<CallCalleeClassificationFact> {
        self.callee
    }

    pub const fn kind(&self) -> UnknownCallKind {
        self.kind
    }
}

/// Typed final-HIR source owned by one expanded call-argument slot.
///
/// Ordinary slots own an expression identity. Compact numeric-sequence
/// elements deliberately remain ID-less and are addressed through their
/// sequence expression plus authored element ordinal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCallArgumentSlotSource {
    Expression(ExprId),
    CompactNumericElement { sequence: ExprId, ordinal: u32 },
}

impl CheckedCallArgumentSlotSource {
    /// Returns the expression that owns this slot source.
    pub const fn owner(self) -> ExprId {
        match self {
            Self::Expression(expression) => expression,
            Self::CompactNumericElement { sequence, .. } => sequence,
        }
    }

    /// Returns the exact final-HIR source query without fabricating a child ID.
    pub const fn source_query(self) -> HirSourceQuery {
        match self {
            Self::Expression(owner) => HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::Whole,
            },
            Self::CompactNumericElement { sequence, ordinal } => HirSourceQuery::Expr {
                owner: sequence,
                role: HirExprSourceRole::NumericElement { ordinal },
            },
        }
    }
}

pub(crate) struct CallTargetFactsInput {
    pub(crate) enclosing_callable: Option<CallableDeclarationKey>,
    pub(crate) outcome: CallAnalysisOutcome,
    pub(crate) diagnostics: Vec<CallableDiagnostic>,
    pub(crate) accounting: CallResolverAccountingReport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallPoison {
    /// The call or mapping was accepted without recovery.
    Clean,
    /// The call or mapping was retained through a recoverable issue.
    Recovered,
    /// The call or mapping was rejected.
    Rejected,
}

impl CallPoison {
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Rejected, _) | (_, Self::Rejected) => Self::Rejected,
            (Self::Recovered, _) | (_, Self::Recovered) => Self::Recovered,
            (Self::Clean, Self::Clean) => Self::Clean,
        }
    }
}

impl CallTargetFacts {
    pub(crate) fn try_new(
        input: CallTargetFactsInput,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError> {
        let CallTargetFactsInput {
            enclosing_callable,
            outcome,
            diagnostics,
            accounting,
        } = input;
        if diagnostics.len() > limits.max_diagnostics() {
            return Err(CallableQueryLimitError::Diagnostics {
                actual: diagnostics.len(),
                limit: limits.max_diagnostics(),
            }
            .into());
        }
        if let CallAnalysisOutcome::Selected(application) = &outcome {
            validate_selected_application(application, limits)?;
        }
        for diagnostic in &diagnostics {
            if let Some(span) = diagnostic.span() {
                validate_self_span(span)?;
            }
            for related in diagnostic.related() {
                if let Some(span) = related.span() {
                    validate_self_span(span)?;
                }
            }
        }
        if accounting.resolver_invocations() > 1 {
            return Err(SemanticSignatureError::InvalidCallAccounting);
        }
        Ok(Self {
            enclosing_callable,
            outcome,
            diagnostics: diagnostics.into(),
            accounting,
        })
    }

    /// Returns the checker expression identity for this call.
    pub fn expression(&self) -> ExprId {
        self.outcome.site().expression()
    }
    /// Returns the typed final-HIR query for the complete Call source.
    pub fn source_query(&self) -> HirSourceQuery {
        HirSourceQuery::Expr {
            owner: self.expression(),
            role: HirExprSourceRole::Whole,
        }
    }
    /// Returns the exact ordinary project function that lexically owns this call.
    pub(crate) const fn enclosing_callable(&self) -> Option<&CallableDeclarationKey> {
        self.enclosing_callable.as_ref()
    }

    /// Returns the one typed outcome owned by this final call fact.
    pub const fn outcome(&self) -> &CallAnalysisOutcome {
        &self.outcome
    }

    /// Returns the sole selected application authority, when selection
    /// succeeded.  No selected projection is rebuilt from wrapper fields.
    pub const fn selected_application(&self) -> Option<&CheckedCallApplication> {
        match &self.outcome {
            CallAnalysisOutcome::Selected(application) => Some(application),
            CallAnalysisOutcome::Ambiguous(_)
            | CallAnalysisOutcome::Rejected(_)
            | CallAnalysisOutcome::NonCallable(_)
            | CallAnalysisOutcome::Missing(_) => None,
        }
    }
    /// Returns callable diagnostics committed for this call.
    pub fn diagnostics(&self) -> &[CallableDiagnostic] {
        &self.diagnostics
    }
    /// Returns exact logical/probe/replay/publication counts committed with
    /// this call fact.
    pub const fn accounting(&self) -> CallResolverAccountingReport {
        self.accounting
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.outcome.visit_types(visitor)?;
        for diagnostic in self.diagnostics() {
            diagnostic.visit_types(visitor)?;
        }
        Ok(())
    }
}

fn validate_selected_application(
    application: &CheckedCallApplication,
    limits: &CallableLimits,
) -> Result<(), SemanticSignatureError> {
    let candidates = application.core().candidates();
    if candidates.candidates().len() > limits.max_candidates_per_call() {
        return Err(CallableQueryLimitError::Candidates {
            actual: candidates.candidates().len(),
            limit: limits.max_candidates_per_call(),
        }
        .into());
    }
    let expression = application.core().site().expression();
    let selected = candidates.selected();
    for (argument_index, argument) in application
        .core()
        .execution()
        .arguments()
        .iter()
        .enumerate()
    {
        let expected = HirCallArgumentOrdinal::try_from_usize(argument_index)
            .map_err(|_| SemanticSignatureError::ActiveParameterOutOfBounds)?;
        if argument.argument() != expected {
            return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
        }
        for (slot_index, slot) in argument.slots().iter().enumerate() {
            let expected = CallableArgumentSlotIndex::try_from_usize(slot_index)
                .map_err(|_| SemanticSignatureError::ActiveParameterOutOfBounds)?;
            if slot.slot() != expected {
                return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
            }
            if slot.source().owner().module() != expression.module() {
                return Err(SemanticSignatureError::SourceIdentityMismatch);
            }
            if let CheckedCallOperandDestination::Parameter(coordinate) = slot.destination()
                && selected
                    .schema()
                    .group(coordinate.group())
                    .and_then(|group| group.parameters().get(coordinate.parameter().get()))
                    .is_none()
            {
                return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
            }
        }
    }
    Ok(())
}

fn seal_unselected_candidates(
    mut candidates: Vec<Arc<ResolvedCallable>>,
    limits: &CallableLimits,
) -> Result<Arc<[Arc<ResolvedCallable>]>, SemanticSignatureError> {
    if candidates.len() > limits.max_candidates_per_call() {
        return Err(CallableQueryLimitError::Candidates {
            actual: candidates.len(),
            limit: limits.max_candidates_per_call(),
        }
        .into());
    }
    candidates.sort_by_key(|candidate| candidate.digest());
    let mut canonical: Vec<Arc<ResolvedCallable>> = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if let Some(previous) = canonical.last()
            && previous.digest() == candidate.digest()
        {
            if previous.as_ref() != candidate.as_ref() {
                return Err(SemanticSignatureError::DuplicateCandidate);
            }
            continue;
        }
        canonical.push(candidate);
    }
    Ok(canonical.into())
}

fn validate_callee_for_site(
    callee: Option<CallCalleeClassificationFact>,
    site: CheckedCallSite,
) -> Result<(), SemanticSignatureError> {
    let expression = site.expression();
    let valid = match callee {
        None => true,
        Some(CallCalleeClassificationFact::Value {
            expression: receiver,
        }) => receiver.module() == expression.module(),
        Some(CallCalleeClassificationFact::AssociatedType {
            receiver,
            separator,
        }) => {
            receiver.module() == expression.module()
                && matches!(separator, HirAssociatedSeparator::Present(_))
        }
    };
    valid
        .then_some(())
        .ok_or(SemanticSignatureError::InvalidCalleeClassification)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticSignatureIndex(u16);
impl SemanticSignatureIndex {
    pub fn try_from_usize(value: usize) -> Result<Self, SemanticSignatureError> {
        u16::try_from(value)
            .map(Self)
            .map_err(|_| SemanticSignatureError::ActiveSignatureOutOfBounds)
    }
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticParameter {
    coordinate: CallableParameterCoordinate,
    label: Arc<str>,
    name: Option<CallableName>,
    admission: CallableParameterAdmission,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    documentation: Option<Arc<str>>,
    source: Option<CallableParameterSource>,
}

impl SemanticParameter {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        coordinate: CallableParameterCoordinate,
        label: impl Into<Arc<str>>,
        name: Option<CallableName>,
        admission: CallableParameterAdmission,
        passing: CallableParameterPassing,
        presence: CallableParameterPresence,
        documentation: Option<Arc<str>>,
        source: Option<CallableParameterSource>,
    ) -> Result<Self, SemanticSignatureError> {
        if source.as_ref().is_some_and(|source| {
            source.group() != coordinate.group() || source.parameter() != coordinate.parameter()
        }) {
            return Err(SemanticSignatureError::InvalidSpan);
        }
        Ok(Self {
            coordinate,
            label: label.into(),
            name,
            admission,
            passing,
            presence,
            documentation,
            source,
        })
    }
    pub const fn coordinate(&self) -> CallableParameterCoordinate {
        self.coordinate
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub const fn name(&self) -> Option<&CallableName> {
        self.name.as_ref()
    }
    pub const fn admission(&self) -> &CallableParameterAdmission {
        &self.admission
    }
    pub const fn declared_type(&self) -> Option<&TypeKind> {
        self.admission.declared()
    }
    pub const fn passing(&self) -> CallableParameterPassing {
        self.passing
    }
    pub const fn presence(&self) -> CallableParameterPresence {
        self.presence
    }
    pub fn documentation(&self) -> Option<&str> {
        self.documentation.as_deref()
    }
    pub const fn source(&self) -> Option<&CallableParameterSource> {
        self.source.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticParameterGroup {
    index: CallableGroupIndex,
    kind: CallableGroupKind,
    parameters: Arc<[SemanticParameter]>,
}
impl SemanticParameterGroup {
    pub fn try_new(
        index: CallableGroupIndex,
        kind: CallableGroupKind,
        parameters: Vec<SemanticParameter>,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError> {
        if parameters.len() > limits.max_parameters_per_callable() {
            return Err(CallableQueryLimitError::Parameters {
                actual: parameters.len(),
                limit: limits.max_parameters_per_callable(),
            }
            .into());
        }
        for (expected, parameter) in parameters.iter().enumerate() {
            let expected = super::CallableParameterIndex::try_from_usize(expected)
                .map_err(|_| SemanticSignatureError::ActiveParameterOutOfBounds)?;
            if parameter.coordinate.group() != index || parameter.coordinate.parameter() != expected
            {
                return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
            }
        }
        Ok(Self {
            index,
            kind,
            parameters: parameters.into(),
        })
    }
    pub const fn index(&self) -> CallableGroupIndex {
        self.index
    }
    pub const fn kind(&self) -> CallableGroupKind {
        self.kind
    }
    pub fn parameters(&self) -> &[SemanticParameter] {
        &self.parameters
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSignature {
    candidate: CallableCandidateId,
    equivalent: Arc<[CallableCandidateId]>,
    origin: SignatureOrigin,
    authored_callee: Arc<str>,
    canonical_callee: Arc<str>,
    groups: Arc<[SemanticParameterGroup]>,
    result: TypeKind,
    effects: EffectRow,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    current_group: CallableGroupIndex,
    poison: CallPoison,
}

impl SemanticSignature {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        candidate: CallableCandidateId,
        equivalent: Vec<CallableCandidateId>,
        origin: SignatureOrigin,
        authored_callee: Arc<str>,
        canonical_callee: Arc<str>,
        groups: Vec<SemanticParameterGroup>,
        result: TypeKind,
        effects: EffectRow,
        documentation: CallableDocumentation,
        source: Option<CallableSource>,
        current_group: CallableGroupIndex,
        poison: CallPoison,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError> {
        if groups.is_empty()
            || groups.len() > limits.max_groups_per_callable()
            || groups.get(current_group.get()).is_none()
        {
            return Err(SemanticSignatureError::CurrentGroupMissing);
        }
        for (expected, group) in groups.iter().enumerate() {
            let expected = CallableGroupIndex::try_from_usize(expected)
                .map_err(|_| SemanticSignatureError::CurrentGroupMissing)?;
            if group.index != expected {
                return Err(SemanticSignatureError::CurrentGroupMissing);
            }
        }
        let mut ids = HashSet::new();
        ids.insert(candidate.clone());
        if equivalent.iter().any(|id| !ids.insert(id.clone())) {
            return Err(SemanticSignatureError::DuplicateEquivalentCandidate);
        }
        Ok(Self {
            candidate,
            equivalent: equivalent.into(),
            origin,
            authored_callee,
            canonical_callee,
            groups: groups.into(),
            result,
            effects,
            documentation,
            source,
            current_group,
            poison,
        })
    }
    pub const fn candidate(&self) -> &CallableCandidateId {
        &self.candidate
    }
    pub fn equivalent(&self) -> &[CallableCandidateId] {
        &self.equivalent
    }
    pub const fn origin(&self) -> &SignatureOrigin {
        &self.origin
    }
    pub fn authored_callee(&self) -> &str {
        &self.authored_callee
    }
    pub fn canonical_callee(&self) -> &str {
        &self.canonical_callee
    }
    pub fn groups(&self) -> &[SemanticParameterGroup] {
        &self.groups
    }
    pub const fn result(&self) -> &TypeKind {
        &self.result
    }
    pub const fn effects(&self) -> &EffectRow {
        &self.effects
    }
    pub const fn documentation(&self) -> &CallableDocumentation {
        &self.documentation
    }
    pub const fn source(&self) -> Option<&CallableSource> {
        self.source.as_ref()
    }
    pub const fn current_group(&self) -> CallableGroupIndex {
        self.current_group
    }
    pub const fn poison(&self) -> CallPoison {
        self.poison
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSignatureHelp {
    document: SourceDocumentIdentity,
    call_span: SourceSpan,
    argument_span: SourceSpan,
    expression: ExprId,
    surface: SemanticSignatureSurface,
    signatures: Arc<[SemanticSignature]>,
    active_signature: SemanticSignatureIndex,
    active_parameter: Option<CallableParameterCoordinate>,
    current_group: CallableGroupIndex,
    next_group: Option<CallableGroupIndex>,
    recovery: SemanticSignatureRecovery,
    diagnostics: Arc<[CallableDiagnostic]>,
    omitted_diagnostics: u64,
    work: SignatureWorkReport,
    query_work: SignatureQueryWorkReport,
}

/// Closed source presentation selected for one semantic signature-help query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticSignatureSurface {
    Parenthesized,
    DialogueContent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticSignatureRecovery {
    Complete,
    Recovered {
        missing_close_delimiter: bool,
        nodes: usize,
    },
}

impl SemanticSignatureHelp {
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "the constructor validates the complete public signature-help invariant atomically"
    )]
    pub fn try_new(
        document: SourceDocumentIdentity,
        call_span: SourceSpan,
        argument_span: SourceSpan,
        expression: ExprId,
        surface: SemanticSignatureSurface,
        signatures: Vec<SemanticSignature>,
        active_signature: SemanticSignatureIndex,
        active_parameter: Option<CallableParameterCoordinate>,
        current_group: CallableGroupIndex,
        next_group: Option<CallableGroupIndex>,
        recovery: SemanticSignatureRecovery,
        diagnostics: Vec<CallableDiagnostic>,
        omitted_diagnostics: u64,
        work: SignatureWorkReport,
        query_work: SignatureQueryWorkReport,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError> {
        if signatures.is_empty() {
            return Err(SemanticSignatureError::EmptySignatures);
        }
        if signatures.len() > limits.max_candidates_per_call() {
            return Err(CallableQueryLimitError::Candidates {
                actual: signatures.len(),
                limit: limits.max_candidates_per_call(),
            }
            .into());
        }
        if active_signature.get() >= signatures.len() {
            return Err(SemanticSignatureError::ActiveSignatureOutOfBounds);
        }
        let mut candidates = HashSet::new();
        if signatures
            .iter()
            .any(|signature| !candidates.insert(signature.candidate.clone()))
        {
            return Err(SemanticSignatureError::DuplicateCandidate);
        }
        if signatures
            .iter()
            .any(|signature| signature.current_group() != current_group)
        {
            return Err(SemanticSignatureError::CurrentGroupMissing);
        }
        if let Some(next_group) = next_group
            && signatures
                .iter()
                .all(|signature| signature.groups().get(next_group.get()).is_none())
        {
            return Err(SemanticSignatureError::CurrentGroupMissing);
        }
        if let SemanticSignatureRecovery::Recovered { nodes, .. } = recovery {
            if nodes == 0 {
                return Err(SemanticSignatureError::InvalidSpan);
            }
            if nodes > limits.max_recovery_nodes() {
                return Err(CallableQueryLimitError::RecoveryNodes {
                    actual: nodes,
                    limit: limits.max_recovery_nodes(),
                }
                .into());
            }
        }
        validate_span(&document, &call_span)?;
        validate_span(&document, &argument_span)?;
        if argument_span.range().start() < call_span.range().start()
            || argument_span.range().end() > call_span.range().end()
        {
            return Err(SemanticSignatureError::InvalidSpan);
        }
        for signature in &signatures {
            if let Some(source) = signature.source() {
                validate_callable_source(source)?;
            }
            for group in signature.groups() {
                for parameter in group.parameters() {
                    if let Some(source) = parameter.source() {
                        validate_parameter_source(source)?;
                    }
                }
            }
        }
        if let Some(active) = active_parameter {
            let parameter_exists = active.group() == current_group
                && signatures
                    .get(active_signature.get())
                    .is_some_and(|signature| signature_has_parameter(signature, active));
            if !parameter_exists {
                return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
            }
        }
        if diagnostics.len() > limits.max_diagnostics() {
            return Err(CallableQueryLimitError::Diagnostics {
                actual: diagnostics.len(),
                limit: limits.max_diagnostics(),
            }
            .into());
        }
        for diagnostic in &diagnostics {
            if let Some(span) = diagnostic.span() {
                validate_span(&document, span)?;
            }
            for related in diagnostic.related() {
                if let Some(span) = related.span() {
                    validate_span(&document, span)?;
                }
            }
        }
        Ok(Self {
            document,
            call_span,
            argument_span,
            expression,
            surface,
            signatures: signatures.into(),
            active_signature,
            active_parameter,
            current_group,
            next_group,
            recovery,
            diagnostics: diagnostics.into(),
            omitted_diagnostics,
            work,
            query_work,
        })
    }
    pub const fn document(&self) -> &SourceDocumentIdentity {
        &self.document
    }
    pub const fn call_span(&self) -> &SourceSpan {
        &self.call_span
    }
    pub const fn argument_span(&self) -> &SourceSpan {
        &self.argument_span
    }
    pub const fn expression(&self) -> ExprId {
        self.expression
    }
    pub const fn surface(&self) -> SemanticSignatureSurface {
        self.surface
    }
    pub fn signatures(&self) -> &[SemanticSignature] {
        &self.signatures
    }
    pub const fn active_signature(&self) -> SemanticSignatureIndex {
        self.active_signature
    }
    pub const fn active_parameter(&self) -> Option<CallableParameterCoordinate> {
        self.active_parameter
    }
    pub const fn current_group(&self) -> CallableGroupIndex {
        self.current_group
    }
    pub const fn next_group(&self) -> Option<CallableGroupIndex> {
        self.next_group
    }
    pub const fn recovery(&self) -> SemanticSignatureRecovery {
        self.recovery
    }
    pub fn diagnostics(&self) -> &[CallableDiagnostic] {
        &self.diagnostics
    }
    pub const fn omitted_diagnostics(&self) -> u64 {
        self.omitted_diagnostics
    }
    pub const fn work(&self) -> SignatureWorkReport {
        self.work
    }
    pub const fn query_work(&self) -> SignatureQueryWorkReport {
        self.query_work
    }
}

fn signature_has_parameter(
    signature: &SemanticSignature,
    coordinate: CallableParameterCoordinate,
) -> bool {
    signature
        .groups
        .get(coordinate.group().get())
        .is_some_and(|group| {
            group
                .parameters
                .iter()
                .any(|parameter| parameter.coordinate == coordinate)
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDiagnostic {
    code: CallableDiagnosticCode,
    severity: CallableDiagnosticSeverity,
    span: Option<SourceSpan>,
    subject: CallableDiagnosticSubject,
    related: Arc<[CallableDiagnosticRelated]>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableDiagnosticSeverity {
    Error,
    Warning,
    Information,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableDiagnosticSubject {
    Candidate(CallableCandidateId),
    Parameter(CallableParameterCoordinate),
    Argument(ExprId),
    Path(super::CallablePath),
    Method {
        receiver: TypeKind,
        name: CallableName,
    },
    Character(arcweft_character::id::CharacterId),
    None,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDiagnosticRelated {
    subject: CallableDiagnosticSubject,
    span: Option<SourceSpan>,
}
impl CallableDiagnosticRelated {
    pub fn new(subject: CallableDiagnosticSubject, span: Option<SourceSpan>) -> Self {
        Self { subject, span }
    }
    pub const fn subject(&self) -> &CallableDiagnosticSubject {
        &self.subject
    }
    pub const fn span(&self) -> Option<&SourceSpan> {
        self.span.as_ref()
    }
}
impl CallableDiagnostic {
    pub fn try_new(
        code: CallableDiagnosticCode,
        severity: CallableDiagnosticSeverity,
        span: Option<SourceSpan>,
        subject: CallableDiagnosticSubject,
        related: Vec<CallableDiagnosticRelated>,
        document: Option<&SourceDocumentIdentity>,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError> {
        if related.len() > limits.max_diagnostics() {
            return Err(CallableQueryLimitError::Diagnostics {
                actual: related.len(),
                limit: limits.max_diagnostics(),
            }
            .into());
        }
        if let Some(document) = document {
            if let Some(span) = &span {
                validate_span(document, span)?;
            }
            for item in &related {
                if let Some(span) = item.span() {
                    validate_span(document, span)?;
                }
            }
        }
        Ok(Self {
            code,
            severity,
            span,
            subject,
            related: related.into(),
        })
    }
    pub const fn code(&self) -> CallableDiagnosticCode {
        self.code
    }
    pub const fn severity(&self) -> CallableDiagnosticSeverity {
        self.severity
    }
    pub const fn span(&self) -> Option<&SourceSpan> {
        self.span.as_ref()
    }
    pub const fn subject(&self) -> &CallableDiagnosticSubject {
        &self.subject
    }
    pub fn related(&self) -> &[CallableDiagnosticRelated] {
        &self.related
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.subject.visit_types(visitor)?;
        for related in self.related() {
            related.visit_types(visitor)?;
        }
        Ok(())
    }
}

impl CallableDiagnosticSubject {
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Method { receiver, .. } => visitor(receiver),
            Self::Candidate(_)
            | Self::Parameter(_)
            | Self::Argument(_)
            | Self::Path(_)
            | Self::Character(_)
            | Self::None => Ok(()),
        }
    }
}

impl CallableDiagnosticRelated {
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.subject.visit_types(visitor)
    }
}

fn validate_span(
    document: &SourceDocumentIdentity,
    span: &SourceSpan,
) -> Result<(), SemanticSignatureError> {
    if span.source() != document {
        return Err(SemanticSignatureError::SourceIdentityMismatch);
    }
    if u64::try_from(span.range().end()).map_or(true, |end| end > document.source_len()) {
        return Err(SemanticSignatureError::InvalidSpan);
    }
    Ok(())
}

fn validate_callable_source(source: &CallableSource) -> Result<(), SemanticSignatureError> {
    for span in source
        .signature()
        .into_iter()
        .chain(source.name())
        .chain(source.result())
    {
        validate_self_span(span)?;
    }
    for parameter in source.parameters() {
        validate_parameter_source(parameter)?;
    }
    Ok(())
}

fn validate_parameter_source(
    source: &CallableParameterSource,
) -> Result<(), SemanticSignatureError> {
    validate_self_span(source.whole())?;
    for span in source
        .name()
        .into_iter()
        .chain(source.ty())
        .chain(source.default())
    {
        validate_self_span(span)?;
    }
    Ok(())
}

fn validate_self_span(span: &SourceSpan) -> Result<(), SemanticSignatureError> {
    if u64::try_from(span.range().end()).map_or(true, |end| end > span.source().source_len()) {
        return Err(SemanticSignatureError::InvalidSpan);
    }
    Ok(())
}
