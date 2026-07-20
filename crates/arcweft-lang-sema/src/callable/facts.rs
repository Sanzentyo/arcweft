//! Checker-owned callable facts and public semantic signature results.

use std::{collections::HashSet, sync::Arc};

use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::{checker::TypeExpressionId, effect_row::EffectRow, types::TypeKind};

use super::{
    CallableArgumentIndex, CallableArgumentSlotIndex, CallableCandidateId, CallableDiagnosticCode,
    CallableDocumentation, CallableGroupIndex, CallableGroupKind, CallableLimits, CallableName,
    CallableParameterCoordinate, CallableParameterPassing, CallableParameterPresence,
    CallableParameterSource, CallableParameterType, CallableQueryLimitError, CallableSource,
    NonCallableSource, ResolvedCallable, SemanticSignatureError, SignatureOrigin,
    SignatureWorkReport,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CallTargetFactMode {
    Disabled,
    #[cfg(test)]
    Focused {
        call: SourceSpan,
    },
    Cursor {
        document: SourceDocumentIdentity,
        byte_offset: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallTargetFacts {
    expression: TypeExpressionId,
    document: SourceDocumentIdentity,
    call_span: SourceSpan,
    target: CallTargetFact,
    arguments: Arc<[CheckedCallArgumentFact]>,
    result: Option<TypeKind>,
    effects: EffectRow,
    current_group: CallableGroupIndex,
    next_group: Option<CallableGroupIndex>,
    function_value_type: Option<TypeKind>,
    poison: CallPoison,
    diagnostics: Arc<[CallableDiagnostic]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CallTargetFact {
    Selected {
        selected: Box<ResolvedCallable>,
        considered: Arc<[ResolvedCallable]>,
    },
    Ambiguous {
        candidates: Arc<[ResolvedCallable]>,
    },
    NonCallable {
        source: NonCallableSource,
        ty: TypeKind,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedCallArgumentFact {
    index: CallableArgumentIndex,
    source: Option<SourceSpan>,
    authored_name: Option<CallableName>,
    spread: bool,
    slots: Arc<[CheckedCallArgumentSlotFact]>,
    poison: CallPoison,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedCallArgumentSlotFact {
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
    Clean,
    Recovered,
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
        expression: TypeExpressionId,
        document: SourceDocumentIdentity,
        call_span: SourceSpan,
        checked: CheckedCallTarget,
        diagnostics: Vec<CallableDiagnostic>,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError> {
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
            for (slot_index, slot) in argument.slots.iter().enumerate() {
                let expected = CallableArgumentSlotIndex::try_from_usize(slot_index)
                    .map_err(|_| SemanticSignatureError::ActiveParameterOutOfBounds)?;
                if slot.slot != expected {
                    return Err(SemanticSignatureError::ActiveParameterOutOfBounds);
                }
                if let Some(source) = &slot.source {
                    validate_span(&document, source)?;
                }
                if let (CallTargetFact::Selected { selected, .. }, Some(coordinate)) =
                    (&checked.target, slot.mapped)
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
            expression,
            document,
            call_span,
            target: checked.target,
            arguments: checked.arguments,
            result: checked.result,
            effects: checked.effects,
            current_group: checked.current_group,
            next_group: checked.next_group,
            function_value_type: checked.function_value_type,
            poison: checked.poison,
            diagnostics: diagnostics.into(),
        })
    }

    pub(crate) const fn expression(&self) -> TypeExpressionId {
        self.expression
    }
    pub(crate) const fn document(&self) -> &SourceDocumentIdentity {
        &self.document
    }
    pub(crate) const fn call_span(&self) -> &SourceSpan {
        &self.call_span
    }
    pub(crate) const fn target(&self) -> &CallTargetFact {
        &self.target
    }
    pub(crate) fn arguments(&self) -> &[CheckedCallArgumentFact] {
        &self.arguments
    }
    pub(crate) const fn result(&self) -> Option<&TypeKind> {
        self.result.as_ref()
    }
    pub(crate) const fn effects(&self) -> &EffectRow {
        &self.effects
    }
    pub(crate) const fn current_group(&self) -> CallableGroupIndex {
        self.current_group
    }
    pub(crate) const fn next_group(&self) -> Option<CallableGroupIndex> {
        self.next_group
    }
    #[cfg(test)]
    pub(crate) const fn function_value_type(&self) -> Option<&TypeKind> {
        self.function_value_type.as_ref()
    }
    pub(crate) const fn poison(&self) -> CallPoison {
        self.poison
    }
    pub(crate) fn diagnostics(&self) -> &[CallableDiagnostic] {
        &self.diagnostics
    }
}

impl CheckedCallArgumentFact {
    pub(crate) fn new(
        index: CallableArgumentIndex,
        source: Option<SourceSpan>,
        authored_name: Option<CallableName>,
        spread: bool,
        slots: Vec<CheckedCallArgumentSlotFact>,
        poison: CallPoison,
    ) -> Self {
        Self {
            index,
            source,
            authored_name,
            spread,
            slots: slots.into(),
            poison,
        }
    }

    pub(crate) const fn index(&self) -> CallableArgumentIndex {
        self.index
    }
    pub(crate) const fn authored_name(&self) -> Option<&CallableName> {
        self.authored_name.as_ref()
    }
    pub(crate) const fn spread(&self) -> bool {
        self.spread
    }
    pub(crate) fn slots(&self) -> &[CheckedCallArgumentSlotFact] {
        &self.slots
    }
    #[cfg(test)]
    pub(crate) const fn poison(&self) -> CallPoison {
        self.poison
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

    #[cfg(test)]
    pub(crate) const fn slot(&self) -> CallableArgumentSlotIndex {
        self.slot
    }
    #[cfg(test)]
    pub(crate) const fn source(&self) -> Option<&SourceSpan> {
        self.source.as_ref()
    }
    pub(crate) const fn mapped(&self) -> Option<CallableParameterCoordinate> {
        self.mapped
    }
    #[cfg(test)]
    pub(crate) const fn inferred(&self) -> Option<&TypeKind> {
        self.inferred.as_ref()
    }
    #[cfg(test)]
    pub(crate) const fn expected(&self) -> Option<&TypeKind> {
        self.expected.as_ref()
    }
    #[cfg(test)]
    pub(crate) const fn poison(&self) -> CallPoison {
        self.poison
    }
}

impl CheckedCallTarget {
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
    label: Arc<str>,
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
        label: Arc<str>,
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
            label,
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
    pub fn label(&self) -> &str {
        &self.label
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
    work: SignatureWorkReport,
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
        work: SignatureWorkReport,
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
            let parameter_exists = signatures
                .get(active_signature.get())
                .and_then(|signature| signature.groups.get(signature.current_group.get()))
                .is_some_and(|group| {
                    group
                        .parameters
                        .iter()
                        .any(|parameter| parameter.coordinate == active)
                });
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
            work,
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
    pub const fn work(&self) -> SignatureWorkReport {
        self.work
    }
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
