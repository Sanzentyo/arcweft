//! Callable documentation, source evidence, and shared signature schemas.

use std::{collections::HashSet, sync::Arc};

use arcweft_lang_hir::symbol::CallableDeclarationId;
use arcweft_source::SourceSpan;

use crate::{
    effect_row::EffectRow,
    env::{FunctionParam, FunctionSignature},
    types::TypeKind,
};

use super::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallableDocumentationError,
    CallableGroupIndex, CallableLimits, CallableName, CallableParameterIndex, CallableSchemaError,
    CallableSourceError, CapacityMethodId, CollectionMethodId, DataLastCallableId,
    DialogueCallableId, DomainMethodId, EnumVariantSignatureId, FxCallableSignatureId,
    IntegerMethodId, LanguageDocumentationFamily, OptionConstructorKind, PresentationCallableId,
    PresentationHandleMethodId, PromotionCallableId, ReductionConstructorKind,
    ResultConstructorKind, RustItemPath, RustProvenanceError, RustProvenanceField, TraitCallableId,
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
        declaration: CallableDeclarationId,
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
    declaration: Option<CallableDeclarationId>,
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
        declaration: Option<CallableDeclarationId>,
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
    pub const fn declaration(&self) -> Option<&CallableDeclarationId> {
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableEffectSchema {
    Fixed(EffectRow),
    Project {
        declaration: CallableDeclarationId,
        declared: EffectRow,
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
    Trait(TraitCallableId),
    DataLast(DataLastCallableId),
    Capacity(CapacityMethodId),
    Drop,
    Promotion(PromotionCallableId),
    Speaker,
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
        })
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
            && self.groups.len() == other.groups.len()
            && self
                .groups
                .iter()
                .zip(other.groups.iter())
                .all(|(left, right)| left.semantic_eq(right))
    }

    /// Whether this catalog schema exactly represents one source-level semantic signature.
    pub(crate) fn matches_function_signature(&self, signature: &FunctionSignature) -> bool {
        self.result == *signature.body_return_type()
            && self.groups.len() == signature.remaining_call_groups().saturating_add(1)
            && self.groups.iter().enumerate().all(|(index, group)| {
                let parameters = if index == 0 {
                    signature.params()
                } else {
                    signature
                        .remaining_param_group(index - 1)
                        .unwrap_or_default()
                };
                group.parameters().len() == parameters.len()
                    && group
                        .parameters()
                        .iter()
                        .zip(parameters)
                        .all(|(catalog, source)| parameter_matches(catalog, source))
            })
    }
}

fn parameter_matches(catalog: &CallableParameter, source: &FunctionParam) -> bool {
    let type_matches = match catalog.ty() {
        CallableParameterType::Exact(ty) => ty == source.ty(),
        CallableParameterType::Unchecked => false,
    };
    let passing_matches = match catalog.passing() {
        CallableParameterPassing::PositionalOnly => source.name().is_none() && !source.is_rest(),
        CallableParameterPassing::PositionalOrNamed => source.name().is_some() && !source.is_rest(),
        CallableParameterPassing::RestPositional => source.is_rest(),
        CallableParameterPassing::NamedOnly | CallableParameterPassing::RestNamed => false,
    };
    let presence_matches = match catalog.presence() {
        CallableParameterPresence::Required => !source.has_default(),
        CallableParameterPresence::Defaulted => source.has_default(),
        CallableParameterPresence::Optional => false,
    };
    catalog.name().map(CallableName::as_str) == source.name()
        && type_matches
        && passing_matches
        && presence_matches
}

impl CallableEffectSchema {
    pub fn fixed(row: EffectRow) -> Self {
        Self::Fixed(row)
    }
    pub fn project(declaration: CallableDeclarationId, declared: EffectRow) -> Self {
        Self::Project {
            declaration,
            declared,
        }
    }
    pub const fn declared(&self) -> &EffectRow {
        match self {
            Self::Fixed(row) | Self::Project { declared: row, .. } => row,
        }
    }
    pub const fn project_declaration(&self) -> Option<&CallableDeclarationId> {
        match self {
            Self::Project { declaration, .. } => Some(declaration),
            Self::Fixed(_) => None,
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
