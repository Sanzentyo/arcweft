//! Checker-owned callable facts and public semantic signature results.

use std::{collections::HashSet, sync::Arc};

use arcweft_lang_hir::symbol::CallableDeclarationId;
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::{checker::TypeExpressionId, effect_row::EffectRow, types::TypeKind};

use super::{
    CallableArgumentIndex, CallableArgumentSlotIndex, CallableCandidateId, CallableDiagnosticCode,
    CallableDocumentation, CallableGroupIndex, CallableGroupKind, CallableLimits, CallableName,
    CallableParameterCoordinate, CallableParameterPassing, CallableParameterPresence,
    CallableParameterSource, CallableParameterType, CallableQueryLimitError, CallableSource,
    NonCallableSource, ResolvedCallable, SemanticSignatureError, SignatureOrigin,
    SignatureQueryWorkReport, SignatureWorkReport, UnknownCallKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CallTargetFactMode {
    Disabled,
    All,
    Focused {
        call: SourceSpan,
        active_argument: Option<usize>,
        byte_offset: Option<usize>,
    },
}

/// Immutable semantic facts committed for one checked call expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallTargetFacts {
    expression: TypeExpressionId,
    document: SourceDocumentIdentity,
    call_span: SourceSpan,
    enclosing_callable: Option<CallableDeclarationId>,
    target: CallTargetFact,
    arguments: Arc<[CheckedCallArgumentFact]>,
    result: Option<TypeKind>,
    effects: EffectRow,
    current_group: CallableGroupIndex,
    next_group: Option<CallableGroupIndex>,
    function_value_type: Option<TypeKind>,
    poison: CallPoison,
    diagnostics: Arc<[CallableDiagnostic]>,
    active_parameter: Option<CallableParameterCoordinate>,
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
    index: CallableArgumentIndex,
    source: Option<SourceSpan>,
    authored_name: Option<CallableName>,
    authored_name_source: Option<SourceSpan>,
    spread: bool,
    slots: Arc<[CheckedCallArgumentSlotFact]>,
    poison: CallPoison,
}

/// Checked mapping retained for one typed slot produced by an argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallArgumentSlotFact {
    slot: CallableArgumentSlotIndex,
    expression: TypeExpressionId,
    source: Option<SourceSpan>,
    mapped: Option<CallableParameterCoordinate>,
    inferred: Option<TypeKind>,
    expected: Option<TypeKind>,
    poison: CallPoison,
}

pub(crate) struct CheckedCallArgumentSlotInput {
    pub(crate) slot: CallableArgumentSlotIndex,
    pub(crate) expression: TypeExpressionId,
    pub(crate) source: Option<SourceSpan>,
    pub(crate) mapped: Option<CallableParameterCoordinate>,
    pub(crate) inferred: Option<TypeKind>,
    pub(crate) expected: Option<TypeKind>,
    pub(crate) poison: CallPoison,
}

pub(crate) struct CallTargetFactsInput {
    pub(crate) expression: TypeExpressionId,
    pub(crate) document: SourceDocumentIdentity,
    pub(crate) call_span: SourceSpan,
    pub(crate) enclosing_callable: Option<CallableDeclarationId>,
    pub(crate) checked: CheckedCallTarget,
    pub(crate) active_parameter: Option<CallableParameterCoordinate>,
    pub(crate) diagnostics: Vec<CallableDiagnostic>,
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
            document,
            call_span,
            enclosing_callable,
            checked,
            active_parameter,
            diagnostics,
        } = input;
        validate_span(&document, &call_span)?;
        if diagnostics.len() > limits.max_diagnostics() {
            return Err(CallableQueryLimitError::Diagnostics {
                actual: diagnostics.len(),
                limit: limits.max_diagnostics(),
            }
            .into());
        }
        for (argument_index, argument) in checked.arguments.iter().enumerate() {
            let expected = CallableArgumentIndex::try_from_usize(argument_index)
                .map_err(|_| SemanticSignatureError::ActiveParameterOutOfBounds)?;
            if argument.index != expected || (!argument.spread && argument.slots.is_empty()) {
                return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
            }
            if let Some(source) = &argument.source {
                validate_span(&document, source)?;
            }
            if let Some(source) = &argument.authored_name_source {
                validate_span(&document, source)?;
                if argument.authored_name.is_none()
                    || argument.source.as_ref().is_some_and(|argument_source| {
                        source.range().start() < argument_source.range().start()
                            || source.range().end() > argument_source.range().end()
                    })
                {
                    return Err(SemanticSignatureError::InvalidSpan);
                }
            }
            for (slot_index, slot) in argument.slots.iter().enumerate() {
                let expected = CallableArgumentSlotIndex::try_from_usize(slot_index)
                    .map_err(|_| SemanticSignatureError::ActiveParameterOutOfBounds)?;
                if slot.slot != expected {
                    return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
                }
                if let Some(source) = &slot.source {
                    validate_span(&document, source)?;
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
                validate_span(&document, span)?;
            }
            for related in diagnostic.related() {
                if let Some(span) = related.span() {
                    validate_span(&document, span)?;
                }
            }
        }
        if let Some(active_parameter) = active_parameter
            && checked
                .active_candidate()
                .and_then(|candidate| {
                    candidate
                        .schema()
                        .group(active_parameter.group())
                        .and_then(|group| {
                            group.parameters().get(active_parameter.parameter().get())
                        })
                })
                .is_none()
        {
            return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
        }
        Ok(Self {
            expression,
            document,
            call_span,
            enclosing_callable,
            target: checked.target,
            arguments: checked.arguments,
            result: checked.result,
            effects: checked.effects,
            current_group: checked.current_group,
            next_group: checked.next_group,
            function_value_type: checked.function_value_type,
            poison: checked.poison,
            diagnostics: diagnostics.into(),
            active_parameter,
        })
    }

    /// Returns the checker expression identity for this call.
    pub const fn expression(&self) -> TypeExpressionId {
        self.expression
    }
    /// Returns the exact accepted source-document identity.
    pub const fn document(&self) -> &SourceDocumentIdentity {
        &self.document
    }
    /// Returns the exact authored call span in the accepted document.
    pub const fn call_span(&self) -> &SourceSpan {
        &self.call_span
    }
    /// Returns the exact ordinary project function that lexically owns this call.
    pub(crate) const fn enclosing_callable(&self) -> Option<&CallableDeclarationId> {
        self.enclosing_callable.as_ref()
    }
    /// Returns the typed target-resolution outcome.
    pub const fn target(&self) -> &CallTargetFact {
        &self.target
    }
    /// Returns authored arguments in source order with their checked mappings.
    pub fn arguments(&self) -> &[CheckedCallArgumentFact] {
        &self.arguments
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
    /// Returns the parameter selected by focused cursor analysis, if available.
    pub const fn active_parameter(&self) -> Option<CallableParameterCoordinate> {
        self.active_parameter
    }
}

impl CheckedCallArgumentFact {
    pub(crate) fn new(
        index: CallableArgumentIndex,
        source: Option<SourceSpan>,
        authored_name: Option<CallableName>,
        authored_name_source: Option<SourceSpan>,
        spread: bool,
        slots: Vec<CheckedCallArgumentSlotFact>,
        poison: CallPoison,
    ) -> Self {
        Self {
            index,
            source,
            authored_name,
            authored_name_source,
            spread,
            slots: slots.into(),
            poison,
        }
    }

    /// Returns the zero-based authored argument index.
    pub const fn index(&self) -> CallableArgumentIndex {
        self.index
    }
    /// Returns the authored argument name for a named argument.
    pub const fn authored_name(&self) -> Option<&CallableName> {
        self.authored_name.as_ref()
    }
    /// Returns the exact authored name-token span for a named argument.
    pub const fn authored_name_source(&self) -> Option<&SourceSpan> {
        self.authored_name_source.as_ref()
    }
    /// Returns whether the authored argument used spread syntax.
    pub const fn spread(&self) -> bool {
        self.spread
    }
    /// Returns typed slots produced by this argument in mapping order.
    pub fn slots(&self) -> &[CheckedCallArgumentSlotFact] {
        &self.slots
    }
    /// Returns the aggregate recovery state for this argument.
    pub const fn poison(&self) -> CallPoison {
        self.poison
    }

    /// Returns the complete authored argument span when source-backed.
    pub const fn source(&self) -> Option<&SourceSpan> {
        self.source.as_ref()
    }
}

impl CheckedCallArgumentSlotFact {
    pub(crate) fn new(input: CheckedCallArgumentSlotInput) -> Self {
        Self {
            slot: input.slot,
            expression: input.expression,
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
    /// Returns the exact slot source span when source-backed.
    pub const fn source(&self) -> Option<&SourceSpan> {
        self.source.as_ref()
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

    /// Returns the checker expression identity for this slot.
    pub const fn expression(&self) -> TypeExpressionId {
        self.expression
    }
}

impl CheckedCallTarget {
    fn active_candidate(&self) -> Option<&ResolvedCallable> {
        match &self.target {
            CallTargetFact::Selected { selected, .. } => Some(selected),
            CallTargetFact::Ambiguous { candidates } | CallTargetFact::Rejected { candidates } => {
                candidates.first()
            }
            CallTargetFact::NonCallable { .. } | CallTargetFact::Missing { .. } => None,
        }
    }

    pub(crate) fn active_parameter(
        &self,
        active_argument: Option<usize>,
        byte_offset: Option<usize>,
    ) -> Option<CallableParameterCoordinate> {
        let active_argument = active_argument?;
        let candidate = self.active_candidate()?;
        let group = candidate.schema().group(self.current_group)?;
        if let Some(argument) = self.arguments.get(active_argument) {
            if let Some(byte_offset) = byte_offset {
                let mut exact = argument
                    .slots
                    .iter()
                    .filter(|slot| {
                        slot.source.as_ref().is_some_and(|source| {
                            source.range().start() <= byte_offset
                                && byte_offset <= source.range().end()
                        })
                    })
                    .filter_map(|slot| slot.mapped);
                let first = exact.next();
                if first.is_some() && exact.all(|candidate| Some(candidate) == first) {
                    return first;
                }
            }
            let mut mapped = argument.slots.iter().filter_map(|slot| slot.mapped);
            let first = mapped.next();
            return (first.is_some() && mapped.all(|candidate| Some(candidate) == first))
                .then_some(first)
                .flatten();
        }
        if active_argument != self.arguments.len() {
            return None;
        }

        let mut provided = vec![false; group.parameters().len()];
        if let super::CallableInstantiation::DataLast {
            group: implicit_group,
            parameter,
            ..
        } = candidate.instantiation()
            && *implicit_group == self.current_group
            && let Some(provided) = provided.get_mut(parameter.get())
        {
            *provided = true;
        }
        for coordinate in self
            .arguments
            .iter()
            .flat_map(|argument| argument.slots.iter())
            .filter_map(|slot| slot.mapped)
            .filter(|coordinate| coordinate.group() == self.current_group)
        {
            let Some(parameter) = group.parameters().get(coordinate.parameter().get()) else {
                continue;
            };
            if !matches!(
                parameter.passing(),
                CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
            ) {
                provided[parameter.index().get()] = true;
            }
        }
        group
            .parameters()
            .iter()
            .find(|parameter| {
                !provided[parameter.index().get()]
                    && matches!(
                        parameter.passing(),
                        CallableParameterPassing::PositionalOrNamed
                            | CallableParameterPassing::PositionalOnly
                            | CallableParameterPassing::RestPositional
                    )
            })
            .map(|parameter| {
                CallableParameterCoordinate::new(self.current_group, parameter.index())
            })
    }

    pub(crate) fn selected(
        selected: &ResolvedCallable,
        considered: &[ResolvedCallable],
        arguments: Vec<CheckedCallArgumentFact>,
        result: TypeKind,
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
            effects: selected.schema().effects().declared().clone(),
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
        arguments: Vec<CheckedCallArgumentFact>,
        current_group: CallableGroupIndex,
    ) -> Self {
        Self {
            target: CallTargetFact::Ambiguous {
                candidates: candidates.to_vec().into(),
            },
            result: None,
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
        current_group: CallableGroupIndex,
    ) -> Self {
        Self {
            target: CallTargetFact::Rejected {
                candidates: candidates.to_vec().into(),
            },
            result: None,
            arguments: arguments.into(),
            effects: EffectRow::closed(crate::effects::EffectSet::new()),
            current_group,
            next_group: None,
            function_value_type: None,
            poison: CallPoison::Rejected,
        }
    }

    pub(crate) fn non_callable(
        source: NonCallableSource,
        ty: TypeKind,
        arguments: Vec<CheckedCallArgumentFact>,
        current_group: CallableGroupIndex,
    ) -> Self {
        Self {
            target: CallTargetFact::NonCallable { source, ty },
            result: None,
            arguments: arguments.into(),
            effects: EffectRow::closed(crate::effects::EffectSet::new()),
            current_group,
            next_group: None,
            function_value_type: None,
            poison: CallPoison::Rejected,
        }
    }

    pub(crate) fn missing(
        kind: UnknownCallKind,
        arguments: Vec<CheckedCallArgumentFact>,
        current_group: CallableGroupIndex,
    ) -> Self {
        Self {
            target: CallTargetFact::Missing { kind },
            result: None,
            arguments: arguments.into(),
            effects: EffectRow::closed(crate::effects::EffectSet::new()),
            current_group,
            next_group: None,
            function_value_type: None,
            poison: CallPoison::Rejected,
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
    expression: TypeExpressionId,
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
        expression: TypeExpressionId,
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
    pub const fn expression(&self) -> TypeExpressionId {
        self.expression
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
    Argument(TypeExpressionId),
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
