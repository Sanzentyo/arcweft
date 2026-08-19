//! Callable documentation, source evidence, and shared signature schemas.

use std::{collections::HashSet, sync::Arc};

use arcweft_lang_hir::symbol::{CallableDeclarationKey, CallableDeclarationOwner};
use arcweft_source::SourceSpan;

use crate::{
    effect_row::EffectRow,
    env::{FunctionParam, FunctionSignature},
    types::TypeKind,
};

use super::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallableDocumentationError,
    CallableGroupIndex, CallableLimits, CallableName, CallableParameterIndex, CallableSchemaError,
    CallableSourceError, CapacityMethodId, CollectionMethodId, DetachedCallableDeclarationId,
    DialogueCallableId, DomainMethodId, EnumVariantSignatureId, FxCallableSignatureId,
    IntegerMethodId, LanguageDocumentationFamily, OptionConstructorKind, PresentationCallableId,
    PresentationHandleMethodId, PromotionCallableId, ReductionConstructorKind,
    ResultConstructorKind, RustItemPath, RustProvenanceError, RustProvenanceField, StageMethodId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDocumentation {
    summary: Option<Arc<str>>,
    details: Option<Arc<str>>,
    parameters: Arc<[CallableParameterDocumentation]>,
    provenance: DocumentationProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterDocumentation {
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
    text: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentationProvenance {
    Missing,
    ProjectSource {
        declaration: CallableDeclarationKey,
    },
    AdapterTooling {
        package: AdapterPackageId,
    },
    RustMetadata {
        adapter: AdapterPackageId,
        package: RustPackageProvenance,
        item: RustItemPath,
    },
    Language {
        family: LanguageDocumentationFamily,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustPackageProvenance {
    name: Arc<str>,
    version: Arc<str>,
    metadata_hash: Option<Arc<str>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustCallablePurity {
    External,
    Pure,
    Task,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustCallableProvenance {
    adapter: AdapterPackageId,
    package: RustPackageProvenance,
    rust_path: RustItemPath,
    purity: RustCallablePurity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSource {
    declaration: Option<CallableDeclarationKey>,
    signature: Option<SourceSpan>,
    name: Option<SourceSpan>,
    result: Option<SourceSpan>,
    parameters: Arc<[CallableParameterSource]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterSource {
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
    whole: SourceSpan,
    name: Option<SourceSpan>,
    ty: Option<SourceSpan>,
    default: Option<SourceSpan>,
}

impl CallableDocumentation {
    pub fn try_new(
        summary: Option<Arc<str>>,
        details: Option<Arc<str>>,
        parameters: Vec<CallableParameterDocumentation>,
        provenance: DocumentationProvenance,
    ) -> Result<Self, CallableDocumentationError> {
        let mut coordinates = HashSet::new();
        for parameter in &parameters {
            if !coordinates.insert((parameter.group, parameter.parameter)) {
                return Err(CallableDocumentationError::DuplicateParameter {
                    group: parameter.group,
                    parameter: parameter.parameter,
                });
            }
        }
        Ok(Self {
            summary,
            details,
            parameters: parameters.into(),
            provenance,
        })
    }
    pub fn missing() -> Self {
        Self {
            summary: None,
            details: None,
            parameters: Arc::from([]),
            provenance: DocumentationProvenance::Missing,
        }
    }
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }
    pub fn details(&self) -> Option<&str> {
        self.details.as_deref()
    }
    pub fn parameters(&self) -> &[CallableParameterDocumentation] {
        &self.parameters
    }
    pub const fn provenance(&self) -> &DocumentationProvenance {
        &self.provenance
    }
    pub fn parameter(
        &self,
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    ) -> Option<&str> {
        self.parameters
            .iter()
            .find(|entry| entry.group == group && entry.parameter == parameter)
            .map(CallableParameterDocumentation::text)
    }

    /// Retains the accepted documentation and identifies a canonical callable
    /// owner when the authored callee is an alias or another accepted spelling.
    #[must_use]
    pub fn with_canonical_owner_note(&self, canonical_owner: &str) -> Self {
        let note = format!("Canonical owner: `{canonical_owner}`.");
        let details = self.details.as_deref().map_or_else(
            || Arc::<str>::from(note.as_str()),
            |details| Arc::<str>::from(format!("{details}\n\n{note}")),
        );
        Self {
            summary: self.summary.clone(),
            details: Some(details),
            parameters: Arc::clone(&self.parameters),
            provenance: self.provenance.clone(),
        }
    }
}

impl CallableParameterDocumentation {
    pub fn try_new(
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
        text: impl Into<Arc<str>>,
    ) -> Result<Self, CallableDocumentationError> {
        let text = text.into();
        if text.is_empty() {
            return Err(CallableDocumentationError::EmptyText);
        }
        Ok(Self {
            group,
            parameter,
            text,
        })
    }
    pub const fn group(&self) -> CallableGroupIndex {
        self.group
    }
    pub const fn parameter(&self) -> CallableParameterIndex {
        self.parameter
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl CallableParameterSource {
    pub fn try_new(
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
        whole: SourceSpan,
        name: Option<SourceSpan>,
        ty: Option<SourceSpan>,
        default: Option<SourceSpan>,
    ) -> Result<Self, CallableSourceError> {
        for child in name.iter().chain(ty.iter()).chain(default.iter()) {
            validate_child_span(&whole, child)?;
        }
        Ok(Self {
            group,
            parameter,
            whole,
            name,
            ty,
            default,
        })
    }
    pub const fn group(&self) -> CallableGroupIndex {
        self.group
    }
    pub const fn parameter(&self) -> CallableParameterIndex {
        self.parameter
    }
    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
    }
    pub const fn name(&self) -> Option<&SourceSpan> {
        self.name.as_ref()
    }
    pub const fn ty(&self) -> Option<&SourceSpan> {
        self.ty.as_ref()
    }
    pub const fn default(&self) -> Option<&SourceSpan> {
        self.default.as_ref()
    }
}

impl CallableSource {
    pub fn try_new(
        declaration: Option<CallableDeclarationKey>,
        signature: Option<SourceSpan>,
        name: Option<SourceSpan>,
        result: Option<SourceSpan>,
        parameters: Vec<CallableParameterSource>,
    ) -> Result<Self, CallableSourceError> {
        let mut coordinates = HashSet::new();
        for parameter in &parameters {
            if !coordinates.insert((parameter.group, parameter.parameter)) {
                return Err(CallableSourceError::DuplicateParameter {
                    group: parameter.group,
                    parameter: parameter.parameter,
                });
            }
        }
        if let Some(signature) = &signature {
            for span in name
                .iter()
                .chain(result.iter())
                .chain(parameters.iter().map(CallableParameterSource::whole))
            {
                validate_child_span(signature, span)?;
            }
        } else if name.is_some() || result.is_some() || !parameters.is_empty() {
            return Err(CallableSourceError::SpanOutsideSignature);
        }
        Ok(Self {
            declaration,
            signature,
            name,
            result,
            parameters: parameters.into(),
        })
    }
    pub const fn declaration(&self) -> Option<&CallableDeclarationKey> {
        self.declaration.as_ref()
    }
    pub const fn signature(&self) -> Option<&SourceSpan> {
        self.signature.as_ref()
    }
    pub const fn name(&self) -> Option<&SourceSpan> {
        self.name.as_ref()
    }
    pub const fn result(&self) -> Option<&SourceSpan> {
        self.result.as_ref()
    }
    pub fn parameters(&self) -> &[CallableParameterSource] {
        &self.parameters
    }
    pub fn parameter(
        &self,
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    ) -> Option<&CallableParameterSource> {
        self.parameters
            .iter()
            .find(|entry| entry.group == group && entry.parameter == parameter)
    }
}

fn validate_child_span(parent: &SourceSpan, child: &SourceSpan) -> Result<(), CallableSourceError> {
    if parent.source() != child.source() {
        return Err(CallableSourceError::SourceIdentityMismatch);
    }
    let parent_range = parent.range();
    let child_range = child.range();
    if child_range.start() < parent_range.start() || child_range.end() > parent_range.end() {
        return Err(CallableSourceError::SpanOutsideSignature);
    }
    Ok(())
}

impl RustPackageProvenance {
    pub fn try_new(
        name: impl Into<Arc<str>>,
        version: impl Into<Arc<str>>,
        metadata_hash: Option<Arc<str>>,
    ) -> Result<Self, RustProvenanceError> {
        let name = validate_rust_field(name.into(), RustProvenanceField::PackageName)?;
        let version = validate_rust_field(version.into(), RustProvenanceField::PackageVersion)?;
        if let Some(hash) = &metadata_hash {
            validate_rust_field(Arc::clone(hash), RustProvenanceField::MetadataHash)?;
        }
        Ok(Self {
            name,
            version,
            metadata_hash,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn metadata_hash(&self) -> Option<&str> {
        self.metadata_hash.as_deref()
    }
}

fn validate_rust_field(
    value: Arc<str>,
    field: RustProvenanceField,
) -> Result<Arc<str>, RustProvenanceError> {
    if value.is_empty() {
        return Err(RustProvenanceError::Empty { field });
    }
    if let Some((byte, _)) = value
        .char_indices()
        .find(|(_, character)| character.is_control())
    {
        return Err(RustProvenanceError::Control { field, byte });
    }
    Ok(value)
}

impl RustCallableProvenance {
    pub fn try_new(
        adapter: AdapterPackageId,
        package: RustPackageProvenance,
        rust_path: RustItemPath,
        purity: RustCallablePurity,
    ) -> Result<Self, RustProvenanceError> {
        Ok(Self {
            adapter,
            package,
            rust_path,
            purity,
        })
    }
    pub const fn adapter(&self) -> &AdapterPackageId {
        &self.adapter
    }
    pub const fn package(&self) -> &RustPackageProvenance {
        &self.package
    }
    pub const fn rust_path(&self) -> &RustItemPath {
        &self.rust_path
    }
    pub const fn purity(&self) -> RustCallablePurity {
        self.purity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSignatureSchema {
    groups: Arc<[CallableParameterGroup]>,
    result: TypeKind,
    effects: CallableEffectSchema,
    argument_policy: CallableArgumentPolicy,
    validator: CallableValidator,
    evaluated_effect: Option<CallableEvaluatedEffect>,
    extension_receiver: Option<CallableExtensionReceiver>,
}

/// Declaration-owned receiver coordinate for one ordinary extension function.
///
/// Ownership is not duplicated here: the exact parameter type at this
/// coordinate remains the sole owned/shared/mutable authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableExtensionReceiver {
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
}

impl CallableExtensionReceiver {
    pub const fn new(group: CallableGroupIndex, parameter: CallableParameterIndex) -> Self {
        Self { group, parameter }
    }

    pub const fn group(self) -> CallableGroupIndex {
        self.group
    }

    pub const fn parameter(self) -> CallableParameterIndex {
        self.parameter
    }
}

/// Runtime-observable effect produced when a selected callable is used as an
/// expression statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableEvaluatedEffect {
    Log(CallableLogLevel),
    SignalWrite,
    MetricWrite,
    EmitEvent,
    Panic,
    Fail,
    Bail,
    Ensure,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl CallableLogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableEffectSchema {
    Fixed(EffectRow),
    Project {
        declaration: CallableDeclarationKey,
    },
    Detached {
        declaration: DetachedCallableDeclarationId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterGroup {
    index: CallableGroupIndex,
    kind: CallableGroupKind,
    parameters: Arc<[CallableParameter]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableGroupKind {
    Initial,
    Curried,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameter {
    index: CallableParameterIndex,
    name: Option<CallableName>,
    ty: CallableParameterType,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    documentation: Option<Arc<str>>,
    source: Option<CallableParameterSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableParameterType {
    Exact(TypeKind),
    Unchecked,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableParameterPassing {
    PositionalOnly,
    PositionalOrNamed,
    NamedOnly,
    RestPositional,
    RestNamed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableParameterPresence {
    Required,
    Optional,
    Defaulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallableArgumentPolicy {
    unknown_named: UnknownNamedArgumentPolicy,
    spread: SpreadArgumentPolicy,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownNamedArgumentPolicy {
    Reject,
    OpenChecked,
    OpenUnchecked,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpreadArgumentPolicy {
    Reject,
    FixedLiteralOnly,
    TypedRest,
    Unchecked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableValidator {
    Ordinary,
    Untyped,
    Fx(FxCallableSignatureId),
    UnknownFxMember { member: CallableName },
    EnumConstructor(EnumVariantSignatureId),
    ResultConstructor(ResultConstructorKind),
    OptionConstructor(OptionConstructorKind),
    ReductionConstructor(ReductionConstructorKind),
    Builtin(BuiltinCallableId),
    Agent(AgentIntrinsicSignatureId),
    Presentation(PresentationCallableId),
    Dialogue(DialogueCallableId),
    Collection(CollectionMethodId),
    PresentationHandle(PresentationHandleMethodId),
    Integer(IntegerMethodId),
    Domain(DomainMethodId),
    Method(CallableMethodRole),
    Capacity(CapacityMethodId),
    Stage(StageMethodId),
    Drop,
    Promotion(PromotionCallableId),
}

/// Pre-check behavior of one structurally identified method declaration.
///
/// This role deliberately carries no declaration, witness, source, or effect
/// identity. Those remain owned by the structural key and checked catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableMethodRole {
    TraitRequirement,
    TraitImplementation,
    Inherent,
}

impl CallableMethodRole {
    pub(crate) const fn required_owner(self) -> CallableDeclarationOwner {
        match self {
            Self::TraitRequirement => CallableDeclarationOwner::TraitRequirement,
            Self::TraitImplementation => CallableDeclarationOwner::TraitImplementation,
            Self::Inherent => CallableDeclarationOwner::InherentMethod,
        }
    }

    pub const fn is_dispatch_contract(self) -> bool {
        matches!(self, Self::TraitRequirement)
    }

    pub const fn is_runtime_callable(self) -> bool {
        !self.is_dispatch_contract()
    }
}

impl CallableSignatureSchema {
    pub fn try_new(
        groups: Vec<CallableParameterGroup>,
        result: TypeKind,
        effects: CallableEffectSchema,
        argument_policy: CallableArgumentPolicy,
        validator: CallableValidator,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        if groups.is_empty() {
            return Err(CallableSchemaError::EmptyGroups);
        }
        if groups.len() > limits.max_groups_per_callable() {
            return Err(CallableSchemaError::GroupLimit {
                actual: groups.len(),
                limit: limits.max_groups_per_callable(),
            });
        }
        let mut total_parameters = 0usize;
        for (expected, group) in groups.iter().enumerate() {
            let expected = CallableGroupIndex::try_from_usize(expected).map_err(|_| {
                CallableSchemaError::GroupLimit {
                    actual: groups.len(),
                    limit: limits.max_groups_per_callable(),
                }
            })?;
            if group.index != expected {
                return Err(CallableSchemaError::NonContiguousGroup {
                    expected,
                    actual: group.index,
                });
            }
            let expected_kind = if expected.get() == 0 {
                CallableGroupKind::Initial
            } else {
                CallableGroupKind::Curried
            };
            if group.kind != expected_kind {
                return Err(CallableSchemaError::InvalidGroupKind { group: group.index });
            }
            total_parameters = total_parameters.checked_add(group.parameters.len()).ok_or(
                CallableSchemaError::ParameterLimit {
                    actual: usize::MAX,
                    limit: limits.max_parameters_per_callable(),
                },
            )?;
        }
        if total_parameters > limits.max_parameters_per_callable() {
            return Err(CallableSchemaError::ParameterLimit {
                actual: total_parameters,
                limit: limits.max_parameters_per_callable(),
            });
        }
        Ok(Self {
            groups: groups.into(),
            result,
            effects,
            argument_policy,
            validator,
            evaluated_effect: None,
            extension_receiver: None,
        })
    }

    pub fn with_extension_receiver(
        mut self,
        receiver: CallableExtensionReceiver,
    ) -> Result<Self, CallableSchemaError> {
        if self.extension_receiver.is_some() {
            return Err(CallableSchemaError::DuplicateExtensionReceiver);
        }
        let group =
            self.group(receiver.group())
                .ok_or(CallableSchemaError::InvalidExtensionReceiver {
                    group: receiver.group(),
                    parameter: receiver.parameter(),
                })?;
        let parameter = group.parameter(receiver.parameter()).ok_or(
            CallableSchemaError::InvalidExtensionReceiver {
                group: receiver.group(),
                parameter: receiver.parameter(),
            },
        )?;
        let receiver_first =
            receiver.group() == CallableGroupIndex::ZERO && receiver.parameter().get() == 0;
        let receiver_data_last = receiver.group().get() + 1 == self.groups.len()
            && receiver.group().get() == 1
            && self.groups.len() == 2
            && group.parameters().len() == 1
            && receiver.parameter().get() == 0;
        if (!receiver_first && !receiver_data_last)
            || parameter.passing() != CallableParameterPassing::PositionalOnly
            || parameter.presence() != CallableParameterPresence::Required
            || !matches!(parameter.ty(), CallableParameterType::Exact(_))
        {
            return Err(CallableSchemaError::InvalidExtensionReceiver {
                group: receiver.group(),
                parameter: receiver.parameter(),
            });
        }
        self.extension_receiver = Some(receiver);
        Ok(self)
    }

    pub(crate) fn with_evaluated_effect(mut self, effect: CallableEvaluatedEffect) -> Self {
        self.evaluated_effect = Some(effect);
        self
    }
    pub fn groups(&self) -> &[CallableParameterGroup] {
        &self.groups
    }
    pub const fn result(&self) -> &TypeKind {
        &self.result
    }
    pub const fn effects(&self) -> &CallableEffectSchema {
        &self.effects
    }
    pub const fn argument_policy(&self) -> CallableArgumentPolicy {
        self.argument_policy
    }
    pub const fn validator(&self) -> &CallableValidator {
        &self.validator
    }
    pub const fn evaluated_effect(&self) -> Option<CallableEvaluatedEffect> {
        self.evaluated_effect
    }
    pub const fn extension_receiver(&self) -> Option<CallableExtensionReceiver> {
        self.extension_receiver
    }
    pub fn extension_receiver_type(&self) -> Option<&TypeKind> {
        let receiver = self.extension_receiver?;
        self.group(receiver.group())
            .and_then(|group| group.parameter(receiver.parameter()))
            .and_then(|parameter| match parameter.ty() {
                CallableParameterType::Exact(ty) => Some(ty),
                CallableParameterType::Unchecked => None,
            })
    }
    pub fn group(&self, index: CallableGroupIndex) -> Option<&CallableParameterGroup> {
        self.groups
            .get(index.get())
            .filter(|group| group.index == index)
    }
    pub fn total_parameters(&self) -> usize {
        self.groups.iter().map(|group| group.parameters.len()).sum()
    }

    pub fn semantic_eq(&self, other: &Self) -> bool {
        self.result == other.result
            && self.effects == other.effects
            && self.argument_policy == other.argument_policy
            && self.validator == other.validator
            && self.evaluated_effect == other.evaluated_effect
            && self.extension_receiver == other.extension_receiver
            && self.groups.len() == other.groups.len()
            && self
                .groups
                .iter()
                .zip(other.groups.iter())
                .all(|(left, right)| left.semantic_eq(right))
    }

    /// Builds the strict positional schema for an evaluated function value.
    pub(crate) fn for_function_value(
        ty: &TypeKind,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        let TypeKind::Function {
            params,
            return_type,
            effects,
        } = ty
        else {
            return Err(CallableSchemaError::FamilyInvariant {
                family: super::CallableFamily::FunctionValue,
                code: super::CallableFamilyInvariantCode::InvalidParameterType,
            });
        };
        let parameters = params
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                CallableParameter::try_new(
                    CallableParameterIndex::try_from_usize(index).map_err(|_| {
                        CallableSchemaError::ParameterLimit {
                            actual: params.len(),
                            limit: limits.max_parameters_per_callable(),
                        }
                    })?,
                    Some(
                        CallableName::try_new(format!("arg{}", index + 1)).map_err(|_| {
                            CallableSchemaError::FamilyInvariant {
                                family: super::CallableFamily::FunctionValue,
                                code: super::CallableFamilyInvariantCode::InvalidParameterType,
                            }
                        })?,
                    ),
                    CallableParameterType::Exact(parameter.clone()),
                    CallableParameterPassing::PositionalOnly,
                    CallableParameterPresence::Required,
                    None,
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let group = CallableParameterGroup::try_new(
            CallableGroupIndex::ZERO,
            CallableGroupKind::Initial,
            parameters,
            limits,
        )?;
        Self::try_new(
            vec![group],
            return_type.as_ref().clone(),
            CallableEffectSchema::fixed(effects.clone()),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            limits,
        )
    }
}

impl FunctionSignature {
    /// Projects one accepted semantic function signature into the canonical
    /// callable schema used by lexical and function-value resolution.
    pub(crate) fn callable_schema(
        &self,
        effects: EffectRow,
        validator: CallableValidator,
        limits: &CallableLimits,
    ) -> Result<CallableSignatureSchema, CallableSchemaError> {
        let (groups, argument_policy) = if self.checks_args() {
            let mut groups = Vec::with_capacity(self.remaining_call_groups().saturating_add(1));
            groups.push(function_parameter_group(0, self.params(), limits)?);
            for index in 0..self.remaining_call_groups() {
                groups.push(function_parameter_group(
                    index + 1,
                    self.remaining_param_group(index).unwrap_or_default(),
                    limits,
                )?);
            }
            let spread = if self
                .params()
                .iter()
                .chain(
                    (0..self.remaining_call_groups())
                        .flat_map(|index| self.remaining_param_group(index).unwrap_or_default()),
                )
                .any(FunctionParam::is_rest)
            {
                SpreadArgumentPolicy::TypedRest
            } else {
                SpreadArgumentPolicy::FixedLiteralOnly
            };
            (
                groups,
                CallableArgumentPolicy::new(UnknownNamedArgumentPolicy::Reject, spread),
            )
        } else {
            (
                vec![unchecked_function_parameter_group(limits)?],
                CallableArgumentPolicy::new(
                    UnknownNamedArgumentPolicy::OpenUnchecked,
                    SpreadArgumentPolicy::Unchecked,
                ),
            )
        };
        CallableSignatureSchema::try_new(
            groups,
            self.body_return_type().clone(),
            CallableEffectSchema::fixed(effects),
            argument_policy,
            validator,
            limits,
        )
    }
}

fn function_parameter_group(
    index: usize,
    params: &[FunctionParam],
    limits: &CallableLimits,
) -> Result<CallableParameterGroup, CallableSchemaError> {
    let group =
        CallableGroupIndex::try_from_usize(index).map_err(|_| CallableSchemaError::GroupLimit {
            actual: index.saturating_add(1),
            limit: limits.max_groups_per_callable(),
        })?;
    let parameters = params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            CallableParameter::try_new(
                CallableParameterIndex::try_from_usize(index).map_err(|_| {
                    CallableSchemaError::ParameterLimit {
                        actual: params.len(),
                        limit: limits.max_parameters_per_callable(),
                    }
                })?,
                parameter
                    .name()
                    .map(CallableName::try_new)
                    .transpose()
                    .map_err(|_| CallableSchemaError::MissingParameterName {
                        group,
                        parameter: CallableParameterIndex::try_from_usize(index).unwrap_or(
                            CallableParameterIndex::try_from_usize(0)
                                .expect("zero parameter index is representable"),
                        ),
                    })?,
                CallableParameterType::Exact(parameter.ty().clone()),
                if parameter.is_rest() {
                    CallableParameterPassing::RestPositional
                } else if parameter.name().is_some() {
                    CallableParameterPassing::PositionalOrNamed
                } else {
                    CallableParameterPassing::PositionalOnly
                },
                if parameter.has_default() {
                    CallableParameterPresence::Defaulted
                } else {
                    CallableParameterPresence::Required
                },
                None,
                None,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    CallableParameterGroup::try_new(
        group,
        if index == 0 {
            CallableGroupKind::Initial
        } else {
            CallableGroupKind::Curried
        },
        parameters,
        limits,
    )
}

fn unchecked_function_parameter_group(
    limits: &CallableLimits,
) -> Result<CallableParameterGroup, CallableSchemaError> {
    let parameter = CallableParameter::try_new(
        CallableParameterIndex::try_from_usize(0).expect("zero parameter index is representable"),
        Some(CallableName::try_new("args").expect("static callable name is valid")),
        CallableParameterType::Unchecked,
        CallableParameterPassing::RestPositional,
        CallableParameterPresence::Optional,
        None,
        None,
    )?;
    CallableParameterGroup::try_new(
        CallableGroupIndex::ZERO,
        CallableGroupKind::Initial,
        vec![parameter],
        limits,
    )
}

impl CallableEffectSchema {
    pub fn fixed(row: EffectRow) -> Self {
        Self::Fixed(row)
    }
    pub fn project(declaration: CallableDeclarationKey) -> Self {
        Self::Project { declaration }
    }
    pub fn detached(declaration: DetachedCallableDeclarationId) -> Self {
        Self::Detached { declaration }
    }
    pub const fn fixed_row(&self) -> Option<&EffectRow> {
        match self {
            Self::Fixed(row) => Some(row),
            Self::Project { .. } | Self::Detached { .. } => None,
        }
    }
    pub const fn project_declaration(&self) -> Option<&CallableDeclarationKey> {
        match self {
            Self::Project { declaration } => Some(declaration),
            Self::Fixed(_) | Self::Detached { .. } => None,
        }
    }
    pub const fn detached_declaration(&self) -> Option<&DetachedCallableDeclarationId> {
        match self {
            Self::Detached { declaration } => Some(declaration),
            Self::Fixed(_) | Self::Project { .. } => None,
        }
    }
}

impl CallableParameterGroup {
    pub fn try_new(
        index: CallableGroupIndex,
        kind: CallableGroupKind,
        parameters: Vec<CallableParameter>,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        if parameters.len() > limits.max_parameters_per_callable() {
            return Err(CallableSchemaError::ParameterLimit {
                actual: parameters.len(),
                limit: limits.max_parameters_per_callable(),
            });
        }
        let mut names = HashSet::new();
        let mut rest_positional = None;
        let mut rest_named = None;
        for (expected, parameter) in parameters.iter().enumerate() {
            let expected = CallableParameterIndex::try_from_usize(expected).map_err(|_| {
                CallableSchemaError::ParameterLimit {
                    actual: parameters.len(),
                    limit: limits.max_parameters_per_callable(),
                }
            })?;
            if parameter.index != expected {
                return Err(CallableSchemaError::NonContiguousParameter {
                    group: index,
                    expected,
                    actual: parameter.index,
                });
            }
            if parameter
                .source
                .as_ref()
                .is_some_and(|source| source.group != index || source.parameter != parameter.index)
            {
                return Err(CallableSchemaError::SourceCoordinateMismatch {
                    group: index,
                    parameter: parameter.index,
                });
            }
            if let Some(name) = &parameter.name
                && !names.insert(name.clone())
            {
                return Err(CallableSchemaError::DuplicateParameterName {
                    group: index,
                    name: name.clone(),
                });
            }
            match parameter.passing {
                CallableParameterPassing::RestPositional
                    if rest_positional.replace(expected).is_some() =>
                {
                    return Err(CallableSchemaError::InvalidRestParameter {
                        group: index,
                        parameter: expected,
                    });
                }
                CallableParameterPassing::RestNamed if rest_named.replace(expected).is_some() => {
                    return Err(CallableSchemaError::InvalidRestParameter {
                        group: index,
                        parameter: expected,
                    });
                }
                _ => {}
            }
        }
        if let Some(rest) = rest_positional
            && parameters.iter().skip(rest.get() + 1).any(|parameter| {
                matches!(
                    parameter.passing,
                    CallableParameterPassing::PositionalOnly
                        | CallableParameterPassing::PositionalOrNamed
                        | CallableParameterPassing::RestPositional
                )
            })
        {
            return Err(CallableSchemaError::InvalidRestParameter {
                group: index,
                parameter: rest,
            });
        }
        if let Some(rest) = rest_named
            && parameters.iter().skip(rest.get() + 1).any(|parameter| {
                matches!(
                    parameter.passing,
                    CallableParameterPassing::NamedOnly
                        | CallableParameterPassing::PositionalOrNamed
                        | CallableParameterPassing::RestNamed
                )
            })
        {
            return Err(CallableSchemaError::InvalidRestParameter {
                group: index,
                parameter: rest,
            });
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
    pub fn parameters(&self) -> &[CallableParameter] {
        &self.parameters
    }
    pub fn parameter(&self, index: CallableParameterIndex) -> Option<&CallableParameter> {
        self.parameters
            .get(index.get())
            .filter(|parameter| parameter.index == index)
    }
    fn semantic_eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.kind == other.kind
            && self.parameters.len() == other.parameters.len()
            && self
                .parameters
                .iter()
                .zip(other.parameters.iter())
                .all(|(left, right)| left.semantic_eq(right))
    }
}

impl CallableParameter {
    pub fn try_new(
        index: CallableParameterIndex,
        name: Option<CallableName>,
        ty: CallableParameterType,
        passing: CallableParameterPassing,
        presence: CallableParameterPresence,
        documentation: Option<Arc<str>>,
        source: Option<CallableParameterSource>,
    ) -> Result<Self, CallableSchemaError> {
        if matches!(
            passing,
            CallableParameterPassing::NamedOnly | CallableParameterPassing::RestNamed
        ) && name.is_none()
        {
            return Err(CallableSchemaError::MissingParameterName {
                group: CallableGroupIndex::ZERO,
                parameter: index,
            });
        }
        if matches!(
            passing,
            CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
        ) && presence == CallableParameterPresence::Defaulted
        {
            return Err(CallableSchemaError::InvalidDefaultedRest {
                group: CallableGroupIndex::ZERO,
                parameter: index,
            });
        }
        if let Some(source) = &source
            && source.parameter != index
        {
            return Err(CallableSchemaError::SourceCoordinateMismatch {
                group: source.group,
                parameter: index,
            });
        }
        Ok(Self {
            index,
            name,
            ty,
            passing,
            presence,
            documentation,
            source,
        })
    }
    pub const fn index(&self) -> CallableParameterIndex {
        self.index
    }
    pub fn name(&self) -> Option<&CallableName> {
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
    fn semantic_eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.name == other.name
            && self.ty == other.ty
            && self.passing == other.passing
            && self.presence == other.presence
    }
}

impl CallableArgumentPolicy {
    pub const fn new(
        unknown_named: UnknownNamedArgumentPolicy,
        spread: SpreadArgumentPolicy,
    ) -> Self {
        Self {
            unknown_named,
            spread,
        }
    }
    pub const fn unknown_named(self) -> UnknownNamedArgumentPolicy {
        self.unknown_named
    }
    pub const fn spread(self) -> SpreadArgumentPolicy {
        self.spread
    }
}

mod families;

pub(super) use families::{dialogue_schema, presentation_schema};
