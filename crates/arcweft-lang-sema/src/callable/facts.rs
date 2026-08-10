//! Checker-owned callable facts and public semantic signature results.

use std::{collections::HashSet, sync::Arc};

use arcweft_lang_hir::{
    expr::{HirAssociatedSeparator, HirCallArgumentOrdinal},
    identity::{ExprId, TypeId},
    source_index::{HirCallArgumentSourcePart, HirExprSourceRole, HirSourceQuery},
    symbol::CallableDeclarationKey,
};
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::{effect_row::EffectRow, types::TypeKind};

use super::{
    CallResolverAccountingReport, CallableArgumentSlotIndex, CallableCandidateId,
    CallableDiagnosticCode, CallableDocumentation, CallableGroupIndex, CallableGroupKind,
    CallableLimits, CallableName, CallableParameterCoordinate, CallableParameterPassing,
    CallableParameterPresence, CallableParameterSource, CallableParameterType,
    CallableQueryLimitError, CallableSource, NonCallableSource, ResolvedCallable,
    SemanticSignatureError, SignatureOrigin, SignatureQueryWorkReport, SignatureWorkReport,
    UnknownCallKind,
};

/// Immutable semantic facts committed for one checked call expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallTargetFacts {
    expression: ExprId,
    enclosing_callable: Option<CallableDeclarationKey>,
    callee: Option<CallCalleeClassificationFact>,
    target: CallTargetFact,
    arguments: Arc<[CheckedCallArgumentFact]>,
    result: Option<TypeKind>,
    effects: EffectRow,
    current_group: CallableGroupIndex,
    next_group: Option<CallableGroupIndex>,
    function_value_type: Option<TypeKind>,
    poison: CallPoison,
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

/// Typed outcome of resolving a checked call target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallTargetFact {
    /// One callable was selected after considering the retained candidates.
    Selected {
        /// Callable whose checked transaction was committed.
        selected: Box<ResolvedCallable>,
        /// Ordered candidates considered for this call.
        considered: Arc<[ResolvedCallable]>,
    },
    /// Multiple equally viable callable candidates remain.
    Ambiguous {
        /// Deterministically ordered viable candidates.
        candidates: Arc<[ResolvedCallable]>,
        /// Complete ordered candidates considered before the viable tie was selected.
        considered: Arc<[ResolvedCallable]>,
    },
    /// Resolution found bounded candidates, but none accepted the authored call.
    Rejected {
        /// Deterministically ordered candidates retained for diagnostics and tooling.
        candidates: Arc<[ResolvedCallable]>,
    },
    /// Target resolution succeeded to a value that is not callable.
    NonCallable {
        /// Typed source that established the non-callable target.
        source: NonCallableSource,
        /// Type of the resolved target value.
        ty: TypeKind,
    },
    /// No callable target could be resolved.
    Missing {
        /// Typed classification of the missing target.
        kind: UnknownCallKind,
    },
}

/// Checked mapping retained for one authored call argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallArgumentFact {
    argument: HirCallArgumentOrdinal,
    slots: Arc<[CheckedCallArgumentSlotFact]>,
    poison: CallPoison,
}

/// Checked mapping retained for one typed slot produced by an argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallArgumentSlotFact {
    slot: CallableArgumentSlotIndex,
    source: CheckedCallArgumentSlotSource,
    mapped: Option<CallableParameterCoordinate>,
    inferred: Option<TypeKind>,
    expected: Option<TypeKind>,
    poison: CallPoison,
}

pub(crate) struct CheckedCallArgumentSlotInput {
    pub(crate) slot: CallableArgumentSlotIndex,
    pub(crate) source: CheckedCallArgumentSlotSource,
    pub(crate) mapped: Option<CallableParameterCoordinate>,
    pub(crate) inferred: Option<TypeKind>,
    pub(crate) expected: Option<TypeKind>,
    pub(crate) poison: CallPoison,
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
    pub(crate) expression: ExprId,
    pub(crate) enclosing_callable: Option<CallableDeclarationKey>,
    pub(crate) callee: Option<CallCalleeClassificationFact>,
    pub(crate) checked: CheckedCallTarget,
    pub(crate) diagnostics: Vec<CallableDiagnostic>,
    pub(crate) accounting: CallResolverAccountingReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedCallTarget {
    target: CallTargetFact,
    result: Option<TypeKind>,
    arguments: Arc<[CheckedCallArgumentFact]>,
    effects: EffectRow,
    current_group: CallableGroupIndex,
    next_group: Option<CallableGroupIndex>,
    function_value_type: Option<TypeKind>,
    poison: CallPoison,
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
            expression,
            enclosing_callable,
            callee,
            checked,
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
        if !callee_is_valid_for_expression(callee, expression) {
            return Err(SemanticSignatureError::InvalidCalleeClassification);
        }
        for (argument_index, argument) in checked.arguments.iter().enumerate() {
            let expected = HirCallArgumentOrdinal::try_from_usize(argument_index)
                .map_err(|_| SemanticSignatureError::ActiveParameterOutOfBounds)?;
            if argument.argument != expected {
                return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
            }
            for (slot_index, slot) in argument.slots.iter().enumerate() {
                let expected = CallableArgumentSlotIndex::try_from_usize(slot_index)
                    .map_err(|_| SemanticSignatureError::ActiveParameterOutOfBounds)?;
                if slot.slot != expected {
                    return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
                }
                if slot.source.owner().module() != expression.module() {
                    return Err(SemanticSignatureError::SourceIdentityMismatch);
                }
                if let (Some(candidate), Some(coordinate)) =
                    (checked.active_candidate(), slot.mapped)
                    && candidate
                        .schema()
                        .group(coordinate.group())
                        .and_then(|group| group.parameters().get(coordinate.parameter().get()))
                        .is_none()
                {
                    return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
                }
            }
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
        validate_call_target_candidates(&checked.target, limits)?;
        if !call_accounting_matches(&checked, accounting) {
            return Err(SemanticSignatureError::InvalidCallAccounting);
        }
        Ok(Self {
            expression,
            enclosing_callable,
            callee,
            target: checked.target,
            arguments: checked.arguments,
            result: checked.result,
            effects: checked.effects,
            current_group: checked.current_group,
            next_group: checked.next_group,
            function_value_type: checked.function_value_type,
            poison: checked.poison,
            diagnostics: diagnostics.into(),
            accounting,
        })
    }

    /// Returns the checker expression identity for this call.
    pub const fn expression(&self) -> ExprId {
        self.expression
    }
    /// Returns the typed final-HIR query for the complete Call source.
    pub fn source_query(&self) -> HirSourceQuery {
        HirSourceQuery::Expr {
            owner: self.expression,
            role: HirExprSourceRole::Whole,
        }
    }
    /// Returns the exact ordinary project function that lexically owns this call.
    pub(crate) const fn enclosing_callable(&self) -> Option<&CallableDeclarationKey> {
        self.enclosing_callable.as_ref()
    }
    /// Returns the project-aware semantic callee classification when structural
    /// recovery did not prevent one from being established.
    pub const fn callee(&self) -> Option<CallCalleeClassificationFact> {
        self.callee
    }
    /// Returns the typed target-resolution outcome.
    pub const fn target(&self) -> &CallTargetFact {
        &self.target
    }
    /// Returns authored arguments in source order with their checked mappings.
    pub fn arguments(&self) -> &[CheckedCallArgumentFact] {
        &self.arguments
    }
    /// Final committed or deterministic-recovery slot projection.
    ///
    /// This is semantic retained state, not a count of physical candidate
    /// evaluation. Speculative facts from non-primary probes are absent.
    pub fn retained_argument_inference_facts(
        &self,
    ) -> impl Iterator<Item = &CheckedCallArgumentSlotFact> {
        self.arguments
            .iter()
            .flat_map(|argument| argument.slots().iter())
    }
    /// Returns the checked result type when one was established.
    pub const fn result(&self) -> Option<&TypeKind> {
        self.result.as_ref()
    }
    /// Returns the effect row committed for the selected call.
    pub const fn effects(&self) -> &EffectRow {
        &self.effects
    }
    /// Returns the parameter group consumed by this call expression.
    pub const fn current_group(&self) -> CallableGroupIndex {
        self.current_group
    }
    /// Returns the next curried parameter group, if this call is partial.
    pub const fn next_group(&self) -> Option<CallableGroupIndex> {
        self.next_group
    }
    /// Returns the exact callable value type when the target was a function value.
    pub const fn function_value_type(&self) -> Option<&TypeKind> {
        self.function_value_type.as_ref()
    }
    /// Returns the aggregate recovery state for the checked call.
    pub const fn poison(&self) -> CallPoison {
        self.poison
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
}

fn validate_call_target_candidates(
    target: &CallTargetFact,
    limits: &CallableLimits,
) -> Result<(), SemanticSignatureError> {
    let validate_complete = |candidates: &[ResolvedCallable]| {
        if candidates.len() > limits.max_candidates_per_call() {
            return Err(CallableQueryLimitError::Candidates {
                actual: candidates.len(),
                limit: limits.max_candidates_per_call(),
            }
            .into());
        }
        let mut ids = HashSet::with_capacity(candidates.len());
        if candidates
            .iter()
            .any(|candidate| !ids.insert(candidate.id().clone()))
        {
            return Err(SemanticSignatureError::DuplicateCandidate);
        }
        Ok(ids)
    };

    match target {
        CallTargetFact::Selected {
            selected,
            considered,
        } => {
            let considered_ids = validate_complete(considered)?;
            if considered.is_empty() || !considered_ids.contains(selected.id()) {
                return Err(SemanticSignatureError::DuplicateCandidate);
            }
        }
        CallTargetFact::Ambiguous {
            candidates,
            considered,
        } => {
            let considered_ids = validate_complete(considered)?;
            validate_complete(candidates)?;
            if candidates.len() < 2
                || candidates
                    .iter()
                    .any(|candidate| !considered_ids.contains(candidate.id()))
            {
                return Err(SemanticSignatureError::DuplicateCandidate);
            }
        }
        CallTargetFact::Rejected { candidates } => {
            validate_complete(candidates)?;
            if candidates.is_empty() {
                return Err(SemanticSignatureError::DuplicateCandidate);
            }
        }
        CallTargetFact::NonCallable { .. } | CallTargetFact::Missing { .. } => {}
    }
    Ok(())
}

fn callee_is_valid_for_expression(
    callee: Option<CallCalleeClassificationFact>,
    expression: ExprId,
) -> bool {
    match callee {
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
    }
}

fn call_accounting_matches(
    checked: &CheckedCallTarget,
    accounting: CallResolverAccountingReport,
) -> bool {
    let Ok(arguments) = u64::try_from(checked.arguments.len()) else {
        return false;
    };
    if accounting.logical_argument_checks() != arguments
        || accounting.retained_argument_fact_publications() != arguments
        || accounting.resolver_invocations() > 1
    {
        return false;
    }
    let (candidate_count, replay) = match &checked.target {
        CallTargetFact::Selected { considered, .. } => {
            let Ok(candidate_count) = u64::try_from(considered.len()) else {
                return false;
            };
            (candidate_count, candidate_count > 1)
        }
        CallTargetFact::Ambiguous { considered, .. } => {
            let Ok(candidate_count) = u64::try_from(considered.len()) else {
                return false;
            };
            (candidate_count, false)
        }
        CallTargetFact::Rejected { candidates } => {
            let Ok(candidate_count) = u64::try_from(candidates.len()) else {
                return false;
            };
            (candidate_count, false)
        }
        CallTargetFact::NonCallable { .. } | CallTargetFact::Missing { .. } => (0, false),
    };
    let Some(expected_probes) = candidate_count.checked_mul(arguments) else {
        return false;
    };
    let expected_replay = if replay { arguments } else { 0 };
    accounting.candidate_argument_probes() == expected_probes
        && accounting.selected_replay_argument_visits() == expected_replay
}

impl CheckedCallArgumentFact {
    pub(crate) fn new(
        argument: HirCallArgumentOrdinal,
        slots: Vec<CheckedCallArgumentSlotFact>,
        poison: CallPoison,
    ) -> Self {
        Self {
            argument,
            slots: slots.into(),
            poison,
        }
    }

    /// Returns the final-HIR argument coordinate in source order.
    pub const fn argument(&self) -> HirCallArgumentOrdinal {
        self.argument
    }

    /// Returns one typed source role owned by the final Call expression.
    pub const fn source_role(&self, part: HirCallArgumentSourcePart) -> HirExprSourceRole {
        HirExprSourceRole::CallArgument {
            argument: self.argument,
            part,
        }
    }
    /// Returns typed slots produced by this argument in mapping order.
    pub fn slots(&self) -> &[CheckedCallArgumentSlotFact] {
        &self.slots
    }
    /// Returns the aggregate recovery state for this argument.
    pub const fn poison(&self) -> CallPoison {
        self.poison
    }
}

impl CheckedCallArgumentSlotFact {
    pub(crate) fn new(input: CheckedCallArgumentSlotInput) -> Self {
        Self {
            slot: input.slot,
            source: input.source,
            mapped: input.mapped,
            inferred: input.inferred,
            expected: input.expected,
            poison: input.poison,
        }
    }

    /// Returns the zero-based slot index within its authored argument.
    pub const fn slot(&self) -> CallableArgumentSlotIndex {
        self.slot
    }
    /// Returns the typed final-HIR query for this checked slot expression.
    pub fn source_query(&self) -> HirSourceQuery {
        self.source.source_query()
    }
    /// Returns the checked parameter coordinate mapped to this slot.
    pub const fn mapped(&self) -> Option<CallableParameterCoordinate> {
        self.mapped
    }
    /// Returns the type inferred for the checked slot expression.
    pub const fn inferred(&self) -> Option<&TypeKind> {
        self.inferred.as_ref()
    }
    /// Returns the mapped parameter's expected type, when checked.
    pub const fn expected(&self) -> Option<&TypeKind> {
        self.expected.as_ref()
    }
    /// Returns the recovery state for this checked slot.
    pub const fn poison(&self) -> CallPoison {
        self.poison
    }

    /// Returns the typed source of this checked slot.
    pub const fn source(&self) -> CheckedCallArgumentSlotSource {
        self.source
    }

    /// Returns the expression identity when this slot is expression-backed.
    pub const fn expression(&self) -> Option<ExprId> {
        match self.source {
            CheckedCallArgumentSlotSource::Expression(expression) => Some(expression),
            CheckedCallArgumentSlotSource::CompactNumericElement { .. } => None,
        }
    }
}

impl CheckedCallTarget {
    fn active_candidate(&self) -> Option<&ResolvedCallable> {
        match &self.target {
            CallTargetFact::Selected { selected, .. } => Some(selected),
            CallTargetFact::Ambiguous { candidates, .. }
            | CallTargetFact::Rejected { candidates } => candidates.first(),
            CallTargetFact::NonCallable { .. } | CallTargetFact::Missing { .. } => None,
        }
    }

    pub(crate) fn selected(
        selected: &ResolvedCallable,
        considered: &[ResolvedCallable],
        arguments: Vec<CheckedCallArgumentFact>,
        result: TypeKind,
        effects: EffectRow,
        current_group: CallableGroupIndex,
        poison: CallPoison,
    ) -> Self {
        let next_group = CallableGroupIndex::try_from_usize(current_group.get() + 1)
            .ok()
            .filter(|next| selected.schema().group(*next).is_some());
        Self {
            target: CallTargetFact::Selected {
                selected: Box::new(selected.clone()),
                considered: considered.to_vec().into(),
            },
            result: Some(result),
            arguments: arguments.into(),
            effects,
            current_group,
            next_group,
            function_value_type: None,
            poison,
        }
    }

    #[must_use]
    pub(crate) fn with_function_value_type(mut self, function_value_type: TypeKind) -> Self {
        self.function_value_type = Some(function_value_type);
        self
    }

    pub(crate) fn ambiguous(
        candidates: &[ResolvedCallable],
        considered: &[ResolvedCallable],
        arguments: Vec<CheckedCallArgumentFact>,
        recovery_result: TypeKind,
        current_group: CallableGroupIndex,
    ) -> Self {
        Self {
            target: CallTargetFact::Ambiguous {
                candidates: candidates.to_vec().into(),
                considered: considered.to_vec().into(),
            },
            result: Some(recovery_result),
            arguments: arguments.into(),
            effects: EffectRow::closed(crate::effects::EffectSet::new()),
            current_group,
            next_group: None,
            function_value_type: None,
            poison: CallPoison::Rejected,
        }
    }

    pub(crate) fn rejected(
        candidates: &[ResolvedCallable],
        arguments: Vec<CheckedCallArgumentFact>,
        recovery_result: TypeKind,
        current_group: CallableGroupIndex,
    ) -> Self {
        Self {
            target: CallTargetFact::Rejected {
                candidates: candidates.to_vec().into(),
            },
            result: Some(recovery_result),
            arguments: arguments.into(),
            effects: EffectRow::closed(crate::effects::EffectSet::new()),
            current_group,
            next_group: None,
            function_value_type: None,
            poison: CallPoison::Rejected,
        }
    }

    /// Retains the typed, candidate-neutral result of an associated receiver
    /// whose generic arity failed before shared-resolver entry.
    pub(crate) fn associated_receiver_recovery(
        arguments: Vec<CheckedCallArgumentFact>,
        recovery_result: TypeKind,
    ) -> Self {
        Self {
            target: CallTargetFact::Missing {
                kind: UnknownCallKind::AssociatedType,
            },
            result: Some(recovery_result),
            arguments: arguments.into(),
            effects: EffectRow::closed(crate::effects::EffectSet::new()),
            current_group: CallableGroupIndex::ZERO,
            next_group: None,
            function_value_type: None,
            poison: CallPoison::Recovered,
        }
    }
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
    ty: CallableParameterType,
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
        ty: CallableParameterType,
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
            ty,
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
    pub const fn ty(&self) -> &CallableParameterType {
        &self.ty
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
