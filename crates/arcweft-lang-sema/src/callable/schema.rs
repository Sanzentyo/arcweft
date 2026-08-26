//! Callable documentation, source evidence, and shared signature schemas.

use std::{
    collections::{BTreeSet, HashSet},
    sync::Arc,
};

use arcweft_lang_hir::symbol::{CallableDeclarationKey, CallableDeclarationOwner};
use arcweft_source::SourceSpan;

use crate::{
    character_dialogue::CharacterDialogueFieldCoordinate,
    effect_row::EffectRow,
    env::{FunctionParam, FunctionSignature, nominal::AcceptedNominalId},
    types::{
        GenericConstParameterId, GenericParameterOwnerId, GenericTypeParameterId,
        LanguageIntrinsicGenericOwner, SemanticTypeDigest, TypeGenericUseCollector, TypeKind,
    },
};

use super::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallableDocumentationError,
    CallableGroupIndex, CallableLimits, CallableName, CallableParameterIndex, CallableSchemaError,
    CallableSourceError, CapacityMethodId, CollectionMethodId, DetachedCallableDeclarationId,
    DialogueCallableId, DomainMethodId, EnumVariantSignatureId, FxCallableSignatureId,
    IntegerMethodId, LanguageDocumentationFamily, LineContextMethodId, OptionConstructorKind,
    PresentationCallableId, PresentationHandleMethodId, PromotionCallableId,
    ReductionConstructorKind, ResultConstructorKind, RustItemPath, RustProvenanceError,
    RustProvenanceField, StageMethodId,
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
    generic_inventory: CallableGenericParameterInventory,
    effects: CallableEffectSchema,
    argument_policy: CallableArgumentPolicy,
    reserved_open_names: Arc<[CallableName]>,
    validator: CallableValidator,
    evaluated_effect: Option<CallableEvaluatedEffect>,
    extension_receiver: Option<CallableExtensionReceiver>,
}

/// Typed owner selected by the declaration or intrinsic schema issuer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CallableGenericIssuerOwner {
    Callable(CallableDeclarationKey),
    AcceptedNominal(AcceptedNominalId),
    LanguageIntrinsic(LanguageIntrinsicGenericOwner),
}

/// Authenticated declaration-owned generic inventory issuer.
///
/// The issuer creates the complete contiguous type/const identities for one
/// exact owner. No arbitrary ID list can be supplied by a schema caller. The
/// schema constructor consumes this token only as input evidence and derives
/// all role and first-use rows from the checked type graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableGenericParameterIssuer {
    owner: Option<CallableGenericIssuerOwner>,
    type_count: u16,
    const_count: u16,
}

impl CallableGenericParameterIssuer {
    pub fn empty() -> Self {
        Self {
            owner: None,
            type_count: 0,
            const_count: 0,
        }
    }

    pub(crate) fn callable(
        declaration: CallableDeclarationKey,
        type_count: u16,
        const_count: u16,
    ) -> Result<Self, CallableSchemaError> {
        Self::new(
            CallableGenericIssuerOwner::Callable(declaration),
            type_count,
            const_count,
        )
    }

    pub(crate) fn accepted_nominal(
        declaration: AcceptedNominalId,
        type_count: u16,
        const_count: u16,
    ) -> Result<Self, CallableSchemaError> {
        Self::new(
            CallableGenericIssuerOwner::AcceptedNominal(declaration),
            type_count,
            const_count,
        )
    }

    pub(crate) fn language_intrinsic(
        owner: LanguageIntrinsicGenericOwner,
        type_count: u16,
        const_count: u16,
    ) -> Result<Self, CallableSchemaError> {
        Self::new(
            CallableGenericIssuerOwner::LanguageIntrinsic(owner),
            type_count,
            const_count,
        )
    }

    fn new(
        owner: CallableGenericIssuerOwner,
        type_count: u16,
        const_count: u16,
    ) -> Result<Self, CallableSchemaError> {
        if let CallableGenericIssuerOwner::LanguageIntrinsic(owner) = &owner {
            let expected_type_count = match owner {
                LanguageIntrinsicGenericOwner::OptionConstructor
                | LanguageIntrinsicGenericOwner::CollectionMap
                | LanguageIntrinsicGenericOwner::FxExists
                | LanguageIntrinsicGenericOwner::AgentSignal
                | LanguageIntrinsicGenericOwner::AgentMetric => 1,
                LanguageIntrinsicGenericOwner::ResultConstructor => 2,
            };
            if const_count != 0 || type_count != expected_type_count {
                return Err(CallableSchemaError::InvalidCandidateIssuer);
            }
            return Ok(Self {
                owner: Some(CallableGenericIssuerOwner::LanguageIntrinsic(*owner)),
                type_count,
                const_count,
            });
        }
        Ok(Self {
            owner: Some(owner),
            type_count,
            const_count,
        })
    }

    pub(crate) fn type_parameters(&self) -> Vec<GenericTypeParameterId> {
        let Some(owner) = self.generic_owner() else {
            return Vec::new();
        };
        (0..self.type_count)
            .map(|ordinal| GenericTypeParameterId::new(owner.clone(), ordinal))
            .collect()
    }

    pub(crate) fn const_parameters(&self) -> Vec<GenericConstParameterId> {
        let Some(owner) = self.generic_owner() else {
            return Vec::new();
        };
        (0..self.const_count)
            .map(|ordinal| GenericConstParameterId::new(owner.clone(), ordinal))
            .collect()
    }

    pub(crate) fn owns_type(&self, parameter: &GenericTypeParameterId) -> bool {
        self.generic_owner()
            .is_some_and(|owner| parameter.owner() == &owner)
    }

    pub(crate) fn owns_const(&self, parameter: &GenericConstParameterId) -> bool {
        self.generic_owner()
            .is_some_and(|owner| parameter.owner() == &owner)
    }

    fn generic_owner(&self) -> Option<GenericParameterOwnerId> {
        self.owner.as_ref().map(|owner| match owner {
            CallableGenericIssuerOwner::Callable(declaration) => {
                GenericParameterOwnerId::Callable(declaration.clone())
            }
            CallableGenericIssuerOwner::AcceptedNominal(declaration) => {
                GenericParameterOwnerId::AcceptedNominal(declaration.clone())
            }
            CallableGenericIssuerOwner::LanguageIntrinsic(owner) => {
                GenericParameterOwnerId::LanguageIntrinsic(*owner)
            }
        })
    }
}

/// The one schema-sealed inventory used by callable constraint preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallableGenericParameterInventory {
    types: Arc<[CallableGenericTypeUse]>,
    rigid_consts: Arc<[CallableRigidConstUse]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallableGenericTypeUse {
    parameter: GenericTypeParameterId,
    role: CallableSchemaGenericRole,
    first_use: CallableGenericFirstUse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallableRigidConstUse {
    parameter: GenericConstParameterId,
    first_use: CallableGenericFirstUse,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CallableSchemaGenericRole {
    Candidate,
    RigidReference,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CallableGenericFirstUse {
    Group(CallableGroupIndex),
    Result,
}

impl CallableGenericParameterInventory {
    pub(crate) fn types(&self) -> &[CallableGenericTypeUse] {
        &self.types
    }

    pub(crate) fn rigid_consts(&self) -> &[CallableRigidConstUse] {
        &self.rigid_consts
    }
}

impl CallableGenericTypeUse {
    pub(crate) const fn parameter(&self) -> &GenericTypeParameterId {
        &self.parameter
    }

    pub(crate) const fn role(&self) -> CallableSchemaGenericRole {
        self.role
    }

    pub(crate) const fn first_use(&self) -> CallableGenericFirstUse {
        self.first_use
    }
}

impl CallableRigidConstUse {
    pub(crate) const fn parameter(&self) -> &GenericConstParameterId {
        &self.parameter
    }

    pub(crate) const fn first_use(&self) -> CallableGenericFirstUse {
        self.first_use
    }
}

/// Call-site identity for one deliberately open named argument. The schema
/// digest prevents two open slots from different signatures from colliding;
/// the authored name is canonicalized through `CallableName` before issuance.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpenArgumentId {
    schema: super::CallableSignatureSchemaDigest,
    binding: CallableName,
}

impl OpenArgumentId {
    pub(crate) fn new(schema: super::CallableSignatureSchemaDigest, binding: CallableName) -> Self {
        Self { schema, binding }
    }

    pub const fn schema(&self) -> super::CallableSignatureSchemaDigest {
        self.schema
    }

    pub const fn binding(&self) -> &CallableName {
        &self.binding
    }
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
    admission: CallableParameterAdmission,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    consumer: CallableParameterConsumer,
    documentation: Option<Arc<str>>,
    source: Option<CallableParameterSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableParameterAdmission {
    Checked {
        declared: TypeKind,
        rule: CallableParameterValueRule,
    },
    UncheckedSupply,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterValueRule {
    guarded: Arc<[CallableParameterGuardedValueAlternative]>,
    otherwise: CallableParameterOtherwiseValueAlternative,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterGuardedValueAlternative {
    guard: CallableSemanticValueGuard,
    expected: ParameterExpectedTypeProjection,
    action: CallableArgumentSemanticAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterOtherwiseValueAlternative {
    expected: ParameterExpectedTypeProjection,
    action: CallableArgumentSemanticAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableParameterValueAlternative<'a> {
    Guarded(&'a CallableParameterGuardedValueAlternative),
    Otherwise(&'a CallableParameterOtherwiseValueAlternative),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableSemanticValueGuard {
    VariantCase {
        owner: ParameterExpectedTypeProjection,
        ordinal: u32,
        payload: VariantPayloadRequirement,
    },
}

/// Exact semantic discriminator observed on one checked source value.
///
/// This is independent of the selected schema alternative: an `otherwise`
/// row still retains a variant case when the checked value is a variant that
/// did not match any guarded row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedSemanticValueEvidence {
    VariantCase {
        owner: SemanticTypeDigest,
        ordinal: u32,
        payload: VariantPayloadRequirement,
    },
    NoVariantCase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VariantPayloadRequirement {
    Unit,
    Present,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParameterExpectedTypeProjection {
    Identity,
    ApplyUnary(CallableUnaryTypeConstructor),
}

impl ParameterExpectedTypeProjection {
    pub(crate) fn apply_to(&self, declared: &TypeKind) -> TypeKind {
        match self {
            Self::Identity => declared.clone(),
            Self::ApplyUnary(CallableUnaryTypeConstructor::Option) => {
                TypeKind::Option(Box::new(declared.clone()))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableUnaryTypeConstructor {
    Option,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableArgumentSemanticAction {
    Supply,
    Clear,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableParameterConsumer {
    Value,
    DialoguePatch(CharacterDialogueFieldCoordinate),
    DialogueApplicationMetadata(DialogueApplicationMetadataCoordinate),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueApplicationMetadataCoordinate {
    Id,
    TextKey,
}

impl CallableParameterValueRule {
    pub fn supply() -> Self {
        Self {
            guarded: Arc::from([]),
            otherwise: CallableParameterOtherwiseValueAlternative {
                expected: ParameterExpectedTypeProjection::Identity,
                action: CallableArgumentSemanticAction::Supply,
            },
        }
    }

    pub(in crate::callable) fn clearable_option() -> Self {
        Self {
            guarded: Arc::from([CallableParameterGuardedValueAlternative {
                guard: CallableSemanticValueGuard::VariantCase {
                    owner: ParameterExpectedTypeProjection::ApplyUnary(
                        CallableUnaryTypeConstructor::Option,
                    ),
                    ordinal: 1,
                    payload: VariantPayloadRequirement::Unit,
                },
                expected: ParameterExpectedTypeProjection::ApplyUnary(
                    CallableUnaryTypeConstructor::Option,
                ),
                action: CallableArgumentSemanticAction::Clear,
            }]),
            otherwise: CallableParameterOtherwiseValueAlternative {
                expected: ParameterExpectedTypeProjection::Identity,
                action: CallableArgumentSemanticAction::Supply,
            },
        }
    }

    pub fn guarded(&self) -> &[CallableParameterGuardedValueAlternative] {
        &self.guarded
    }

    pub const fn otherwise(&self) -> &CallableParameterOtherwiseValueAlternative {
        &self.otherwise
    }

    pub fn len(&self) -> usize {
        self.guarded.len() + 1
    }

    pub fn alternatives(
        &self,
    ) -> impl Clone + DoubleEndedIterator<Item = CallableParameterValueAlternative<'_>> {
        self.guarded
            .iter()
            .map(CallableParameterValueAlternative::Guarded)
            .chain(std::iter::once(
                CallableParameterValueAlternative::Otherwise(&self.otherwise),
            ))
    }

    pub fn alternative(&self, index: usize) -> Option<CallableParameterValueAlternative<'_>> {
        self.guarded
            .get(index)
            .map(CallableParameterValueAlternative::Guarded)
            .or_else(|| {
                (index == self.guarded.len()).then_some(
                    CallableParameterValueAlternative::Otherwise(&self.otherwise),
                )
            })
    }

    /// Validate the exact first-match selection owned by this schema rule.
    /// Guarded rows have priority in declaration order; `otherwise` is legal
    /// only when no guarded row accepts the observed evidence.
    pub(crate) fn selects(
        &self,
        index: usize,
        declared: &TypeKind,
        checked: &CheckedSemanticValueEvidence,
    ) -> bool {
        let mut matching = self
            .guarded
            .iter()
            .enumerate()
            .filter_map(|(index, row)| row.guard().accepts(declared, checked).then_some(index));
        match (matching.next(), matching.next()) {
            (Some(selected), None) => selected == index,
            (None, None) => index == self.guarded.len(),
            (Some(_), Some(_)) | (None, Some(_)) => false,
        }
    }
}

impl CallableParameterGuardedValueAlternative {
    pub const fn guard(&self) -> &CallableSemanticValueGuard {
        &self.guard
    }

    pub const fn expected(&self) -> &ParameterExpectedTypeProjection {
        &self.expected
    }

    pub const fn action(&self) -> CallableArgumentSemanticAction {
        self.action
    }
}

impl CallableParameterOtherwiseValueAlternative {
    pub const fn expected(&self) -> &ParameterExpectedTypeProjection {
        &self.expected
    }

    pub const fn action(&self) -> CallableArgumentSemanticAction {
        self.action
    }
}

impl<'a> CallableParameterValueAlternative<'a> {
    pub const fn guard(self) -> Option<&'a CallableSemanticValueGuard> {
        match self {
            Self::Guarded(alternative) => Some(alternative.guard()),
            Self::Otherwise(_) => None,
        }
    }

    pub const fn expected(self) -> &'a ParameterExpectedTypeProjection {
        match self {
            Self::Guarded(alternative) => alternative.expected(),
            Self::Otherwise(alternative) => alternative.expected(),
        }
    }

    pub const fn action(self) -> CallableArgumentSemanticAction {
        match self {
            Self::Guarded(alternative) => alternative.action(),
            Self::Otherwise(alternative) => alternative.action(),
        }
    }

    pub const fn is_otherwise(self) -> bool {
        matches!(self, Self::Otherwise(_))
    }
}

impl CallableSemanticValueGuard {
    pub(crate) fn accepts(
        &self,
        declared: &TypeKind,
        checked: &CheckedSemanticValueEvidence,
    ) -> bool {
        match (self, checked) {
            (
                Self::VariantCase {
                    owner,
                    ordinal,
                    payload,
                },
                CheckedSemanticValueEvidence::VariantCase {
                    owner: checked_owner,
                    ordinal: checked_ordinal,
                    payload: checked_payload,
                },
            ) => {
                owner.apply_to(declared).semantic_identity_digest() == *checked_owner
                    && ordinal == checked_ordinal
                    && payload == checked_payload
            }
            (Self::VariantCase { .. }, CheckedSemanticValueEvidence::NoVariantCase) => false,
        }
    }
}

impl CallableParameterAdmission {
    pub fn checked(declared: TypeKind) -> Self {
        Self::Checked {
            declared,
            rule: CallableParameterValueRule::supply(),
        }
    }

    pub fn checked_with_rule(declared: TypeKind, rule: CallableParameterValueRule) -> Self {
        Self::Checked { declared, rule }
    }

    pub const fn unchecked_supply() -> Self {
        Self::UncheckedSupply
    }

    pub const fn declared(&self) -> Option<&TypeKind> {
        match self {
            Self::Checked { declared, .. } => Some(declared),
            Self::UncheckedSupply => None,
        }
    }

    pub const fn rule(&self) -> Option<&CallableParameterValueRule> {
        match self {
            Self::Checked { rule, .. } => Some(rule),
            Self::UncheckedSupply => None,
        }
    }

    pub const fn is_unchecked(&self) -> bool {
        matches!(self, Self::UncheckedSupply)
    }
}

fn is_single_supply_identity_rule(rule: &CallableParameterValueRule) -> bool {
    rule.guarded().is_empty()
        && matches!(
            (rule.otherwise().expected(), rule.otherwise().action()),
            (
                ParameterExpectedTypeProjection::Identity,
                CallableArgumentSemanticAction::Supply
            )
        )
}

impl From<TypeKind> for CallableParameterAdmission {
    fn from(value: TypeKind) -> Self {
        Self::checked(value)
    }
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
    OpenSupply,
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
    LineContext(LineContextMethodId),
    Drop,
    Promotion(PromotionCallableId),
    ViewModifier(super::ViewModifierId),
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
    pub(crate) fn try_new(
        groups: Vec<CallableParameterGroup>,
        result: TypeKind,
        effects: CallableEffectSchema,
        argument_policy: CallableArgumentPolicy,
        validator: CallableValidator,
        generic_issuer: CallableGenericParameterIssuer,
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
            for parameter in group.parameters() {
                match parameter.admission() {
                    CallableParameterAdmission::UncheckedSupply => {
                        if !matches!(parameter.consumer(), CallableParameterConsumer::Value) {
                            return Err(CallableSchemaError::InvalidParameterConsumer {
                                group: group.index,
                                parameter: parameter.index,
                            });
                        }
                    }
                    CallableParameterAdmission::Checked { rule, .. } => {
                        if rule.guarded().iter().enumerate().any(|(index, row)| {
                            rule.guarded()[..index]
                                .iter()
                                .any(|previous| previous.guard() == row.guard())
                        }) {
                            return Err(CallableSchemaError::InvalidParameterAdmission {
                                group: group.index,
                                parameter: parameter.index,
                            });
                        }
                        if rule.alternatives().any(|alternative| {
                            alternative.action() == CallableArgumentSemanticAction::Clear
                                && !matches!(
                                    parameter.consumer(),
                                    CallableParameterConsumer::DialoguePatch(_)
                                )
                        }) {
                            return Err(CallableSchemaError::InvalidParameterConsumer {
                                group: group.index,
                                parameter: parameter.index,
                            });
                        }
                    }
                }
                if matches!(
                    parameter.passing(),
                    CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
                ) {
                    let typed_rest = matches!(
                        parameter.admission(),
                        CallableParameterAdmission::Checked { rule, .. }
                            if is_single_supply_identity_rule(rule)
                    );
                    let unchecked_rest = matches!(
                        parameter.admission(),
                        CallableParameterAdmission::UncheckedSupply
                    ) && parameter.passing()
                        == CallableParameterPassing::RestPositional
                        && argument_policy.spread() == SpreadArgumentPolicy::Unchecked;
                    if !typed_rest && !unchecked_rest {
                        return Err(CallableSchemaError::InvalidParameterAdmission {
                            group: group.index,
                            parameter: parameter.index,
                        });
                    }
                }
            }
        }
        if total_parameters > limits.max_parameters_per_callable() {
            return Err(CallableSchemaError::ParameterLimit {
                actual: total_parameters,
                limit: limits.max_parameters_per_callable(),
            });
        }
        let generic_inventory = seal_generic_inventory(&groups, &result, &generic_issuer)?;
        Ok(Self {
            groups: groups.into(),
            result,
            generic_inventory,
            effects,
            argument_policy,
            reserved_open_names: Arc::new([]),
            validator,
            evaluated_effect: None,
            extension_receiver: None,
        })
    }

    pub(crate) fn try_with_reserved_open_names(
        mut self,
        mut names: Vec<CallableName>,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        if !names.is_empty()
            && self.argument_policy.unknown_named() != UnknownNamedArgumentPolicy::OpenSupply
        {
            return Err(CallableSchemaError::ReservedOpenNamesRequireOpenPolicy);
        }
        if names.len() > limits.max_parameters_per_callable() {
            return Err(CallableSchemaError::ReservedOpenNameLimit {
                actual: names.len(),
                limit: limits.max_parameters_per_callable(),
            });
        }
        names.sort_unstable();
        for pair in names.windows(2) {
            if pair[0] == pair[1] {
                return Err(CallableSchemaError::DuplicateReservedOpenName {
                    name: pair[0].clone(),
                });
            }
        }
        if names.iter().any(|reserved| {
            self.groups
                .iter()
                .flat_map(|group| group.parameters())
                .any(|parameter| parameter.name() == Some(reserved))
        }) {
            let name = names
                .iter()
                .find(|reserved| {
                    self.groups
                        .iter()
                        .flat_map(|group| group.parameters())
                        .any(|parameter| parameter.name() == Some(reserved))
                })
                .expect("reserved parameter collision was observed")
                .clone();
            return Err(CallableSchemaError::ReservedOpenNameParameterCollision { name });
        }
        self.reserved_open_names = names.into();
        Ok(self)
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
            || parameter.admission().is_unchecked()
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
    pub(crate) const fn generic_inventory(&self) -> &CallableGenericParameterInventory {
        &self.generic_inventory
    }
    pub const fn effects(&self) -> &CallableEffectSchema {
        &self.effects
    }
    pub const fn argument_policy(&self) -> CallableArgumentPolicy {
        self.argument_policy
    }
    pub fn reserved_open_names(&self) -> &[CallableName] {
        &self.reserved_open_names
    }
    pub(crate) fn allows_open_name(&self, name: &CallableName) -> bool {
        self.argument_policy.unknown_named() == UnknownNamedArgumentPolicy::OpenSupply
            && self.reserved_open_names.binary_search(name).is_err()
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
            .and_then(|parameter| parameter.declared_type())
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(self.result())?;
        for group in self.groups() {
            for parameter in group.parameters() {
                if let Some(ty) = parameter.declared_type() {
                    visitor(ty)?;
                }
            }
        }
        if let Some(receiver) = self.extension_receiver_type() {
            visitor(receiver)?;
        }
        Ok(())
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
        self.generic_inventory == other.generic_inventory
            && self.result == other.result
            && self.effects == other.effects
            && self.argument_policy == other.argument_policy
            && self.reserved_open_names == other.reserved_open_names
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
                    CallableParameterAdmission::checked(parameter.clone()),
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
            CallableGenericParameterIssuer::empty(),
            limits,
        )
    }

    /// Builds the exact constructor schema retained by one accepted project
    /// enum case. The checked case row has already instantiated declaration
    /// generics, so no generic issuer or path lookup is admitted here.
    pub(crate) fn for_accepted_enum_case(
        id: EnumVariantSignatureId,
        payload: Option<&TypeKind>,
        result: TypeKind,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        let payloads: &[TypeKind] = match payload {
            Some(TypeKind::Tuple(items)) => items,
            Some(payload) => std::slice::from_ref(payload),
            None => &[],
        };
        let parameters = payloads
            .iter()
            .enumerate()
            .map(|(index, payload)| {
                CallableParameter::try_new(
                    CallableParameterIndex::try_from_usize(index).map_err(|_| {
                        CallableSchemaError::ParameterLimit {
                            actual: payloads.len(),
                            limit: limits.max_parameters_per_callable(),
                        }
                    })?,
                    Some(
                        CallableName::try_new(format!("payload{}", index + 1)).map_err(|_| {
                            CallableSchemaError::FamilyInvariant {
                                family: super::CallableFamily::EnumConstructor,
                                code: super::CallableFamilyInvariantCode::InvalidParameterType,
                            }
                        })?,
                    ),
                    CallableParameterAdmission::checked(payload.clone()),
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
            result,
            CallableEffectSchema::fixed(crate::effect_row::EffectRow::closed(
                crate::effects::EffectSet::new(),
            )),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::EnumConstructor(id),
            CallableGenericParameterIssuer::empty(),
            limits,
        )
    }
}

fn seal_generic_inventory(
    groups: &[CallableParameterGroup],
    result: &TypeKind,
    issuer: &CallableGenericParameterIssuer,
) -> Result<CallableGenericParameterInventory, CallableSchemaError> {
    let mut occurrences = Vec::new();
    for group in groups {
        let position = u32::try_from(group.index().get())
            .expect("schema group positions fit the generic-use coordinate");
        for parameter in group.parameters() {
            if let Some(ty) = parameter.declared_type() {
                occurrences.push((ty, position));
            }
        }
    }
    let result_position =
        u32::try_from(groups.len()).map_err(|_| CallableSchemaError::GroupLimit {
            actual: groups.len(),
            limit: groups.len(),
        })?;
    occurrences.push((result, result_position));
    let collected = TypeGenericUseCollector::collect_many(occurrences)?;
    let candidate_types = issuer.type_parameters();
    for parameter in &candidate_types {
        if !collected.types().contains(parameter) {
            return Err(CallableSchemaError::MissingCandidateType {
                parameter: parameter.clone(),
            });
        }
    }
    let candidate_consts = issuer.const_parameters();
    for parameter in &candidate_consts {
        if !collected.consts().contains(parameter) {
            return Err(CallableSchemaError::MissingCandidateConst {
                parameter: parameter.clone(),
            });
        }
    }
    for parameter in collected.types() {
        if issuer.owns_type(parameter) && !candidate_types.contains(parameter) {
            return Err(CallableSchemaError::MissingCandidateType {
                parameter: parameter.clone(),
            });
        }
    }
    for parameter in collected.consts() {
        if issuer.owns_const(parameter) {
            if !candidate_consts.contains(parameter) {
                return Err(CallableSchemaError::MissingCandidateConst {
                    parameter: parameter.clone(),
                });
            }
            return Err(CallableSchemaError::InferableConstGeneric {
                parameter: parameter.clone(),
            });
        }
    }

    let candidate_types = candidate_types.iter().collect::<BTreeSet<_>>();
    let types = collected
        .types()
        .iter()
        .map(|parameter| {
            let first_use = first_use_for(
                collected
                    .first_type_use(parameter)
                    .expect("collector stores a first use for every type"),
                groups.len(),
            );
            CallableGenericTypeUse {
                parameter: parameter.clone(),
                role: if candidate_types.contains(parameter) {
                    CallableSchemaGenericRole::Candidate
                } else {
                    CallableSchemaGenericRole::RigidReference
                },
                first_use,
            }
        })
        .collect::<Vec<_>>()
        .into();
    let rigid_consts = collected
        .consts()
        .iter()
        .map(|parameter| CallableRigidConstUse {
            parameter: parameter.clone(),
            first_use: first_use_for(
                collected
                    .first_const_use(parameter)
                    .expect("collector stores a first use for every const"),
                groups.len(),
            ),
        })
        .collect::<Vec<_>>()
        .into();
    Ok(CallableGenericParameterInventory {
        types,
        rigid_consts,
    })
}

fn first_use_for(position: u32, group_count: usize) -> CallableGenericFirstUse {
    if usize::try_from(position).ok() == Some(group_count) {
        CallableGenericFirstUse::Result
    } else {
        CallableGenericFirstUse::Group(
            CallableGroupIndex::try_from_usize(position as usize)
                .expect("schema group positions are representable"),
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
        generic_issuer: CallableGenericParameterIssuer,
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
                    UnknownNamedArgumentPolicy::OpenSupply,
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
            generic_issuer,
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
                CallableParameterAdmission::checked(parameter.ty().clone()),
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
        CallableParameterAdmission::unchecked_supply(),
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
                CallableParameterPassing::RestPositional => {
                    if rest_positional.is_some() || rest_named.is_some() {
                        return Err(CallableSchemaError::InvalidRestParameter {
                            group: index,
                            parameter: expected,
                        });
                    }
                    rest_positional = Some(expected);
                }
                CallableParameterPassing::RestNamed => {
                    if rest_positional.is_some() || rest_named.is_some() {
                        return Err(CallableSchemaError::InvalidRestParameter {
                            group: index,
                            parameter: expected,
                        });
                    }
                    rest_named = Some(expected);
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
        admission: impl Into<CallableParameterAdmission>,
        passing: CallableParameterPassing,
        presence: CallableParameterPresence,
        documentation: Option<Arc<str>>,
        source: Option<CallableParameterSource>,
    ) -> Result<Self, CallableSchemaError> {
        let admission = admission.into();
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
            admission,
            passing,
            presence,
            consumer: CallableParameterConsumer::Value,
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
    pub const fn admission(&self) -> &CallableParameterAdmission {
        &self.admission
    }
    pub const fn declared_type(&self) -> Option<&TypeKind> {
        self.admission.declared()
    }
    pub const fn value_rule(&self) -> Option<&CallableParameterValueRule> {
        self.admission.rule()
    }
    pub const fn consumer(&self) -> &CallableParameterConsumer {
        &self.consumer
    }
    pub(crate) fn with_consumer(mut self, consumer: CallableParameterConsumer) -> Self {
        self.consumer = consumer;
        self
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
            && self.admission == other.admission
            && self.passing == other.passing
            && self.presence == other.presence
            && self.consumer == other.consumer
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

#[cfg(test)]
mod generic_inventory_tests {
    use super::*;
    use crate::{
        callable::PRODUCTION_CALLABLE_LIMITS,
        effect_row::EffectRow,
        effects::EffectSet,
        env::nominal::{AcceptedNominalId, AcceptedNominalOwnerId},
        types::{
            ArrayLength, GenericConstParameterId, GenericParameterOwnerId, GenericTypeParameterId,
            MapKind,
        },
    };

    fn accepted_owner(owner: u64) -> AcceptedNominalId {
        let path = arcweft_lang_syntax::types::TypePath::from(
            arcweft_lang_syntax::ast::symbol_path::ProjectSymbolPath::new(
                arcweft_lang_syntax::ast::module_path::ModulePathRoot::ImplicitCrate,
                [
                    arcweft_lang_syntax::ast::symbol_path::ProjectSymbolSegment::try_new(format!(
                        "GenericOwner{owner}"
                    ))
                    .expect("generic owner path segment"),
                ],
            )
            .expect("generic owner path"),
        );
        AcceptedNominalId::new(AcceptedNominalOwnerId::Standard, path)
    }

    fn accepted_type(owner: u64, ordinal: u16) -> GenericTypeParameterId {
        GenericTypeParameterId::new(
            GenericParameterOwnerId::AcceptedNominal(accepted_owner(owner)),
            ordinal,
        )
    }

    fn parameter(index: usize, ty: TypeKind) -> CallableParameter {
        CallableParameter::try_new(
            CallableParameterIndex::try_from_usize(index).expect("test parameter index"),
            Some(CallableName::try_new(format!("arg{index}")).expect("test parameter name")),
            CallableParameterAdmission::checked(ty),
            CallableParameterPassing::PositionalOnly,
            CallableParameterPresence::Required,
            None,
            None,
        )
        .expect("test parameter is valid")
    }

    fn group(index: usize, parameters: Vec<CallableParameter>) -> CallableParameterGroup {
        let index = CallableGroupIndex::try_from_usize(index).expect("test group index");
        CallableParameterGroup::try_new(
            index,
            if index == CallableGroupIndex::ZERO {
                CallableGroupKind::Initial
            } else {
                CallableGroupKind::Curried
            },
            parameters,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("test group is valid")
    }

    fn effects() -> CallableEffectSchema {
        CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new()))
    }

    #[test]
    fn semantic_value_rule_selects_clear_guard_before_mandatory_otherwise() {
        let declared = TypeKind::I32;
        let rule = CallableParameterValueRule::clearable_option();
        let clear = CheckedSemanticValueEvidence::VariantCase {
            owner: TypeKind::Option(Box::new(declared.clone())).semantic_identity_digest(),
            ordinal: 1,
            payload: VariantPayloadRequirement::Unit,
        };
        assert!(rule.selects(0, &declared, &clear));
        assert!(!rule.selects(1, &declared, &clear));
    }

    #[test]
    fn semantic_value_rule_routes_other_variant_and_nonvariant_values_to_otherwise() {
        let declared = TypeKind::I32;
        let rule = CallableParameterValueRule::clearable_option();
        let other_variant = CheckedSemanticValueEvidence::VariantCase {
            owner: TypeKind::Option(Box::new(declared.clone())).semantic_identity_digest(),
            ordinal: 0,
            payload: VariantPayloadRequirement::Present,
        };
        assert!(rule.selects(1, &declared, &other_variant));
        assert!(rule.selects(1, &declared, &CheckedSemanticValueEvidence::NoVariantCase,));

        let supply = CallableParameterValueRule::supply();
        assert!(supply.selects(0, &declared, &other_variant));
        assert!(supply.selects(0, &declared, &CheckedSemanticValueEvidence::NoVariantCase,));
    }

    #[test]
    fn semantic_value_rule_rejects_tampered_clear_evidence_at_the_guarded_coordinate() {
        let declared = TypeKind::I32;
        let rule = CallableParameterValueRule::clearable_option();
        for tampered in [
            CheckedSemanticValueEvidence::VariantCase {
                owner: TypeKind::Option(Box::new(TypeKind::U32)).semantic_identity_digest(),
                ordinal: 1,
                payload: VariantPayloadRequirement::Unit,
            },
            CheckedSemanticValueEvidence::VariantCase {
                owner: TypeKind::Option(Box::new(declared.clone())).semantic_identity_digest(),
                ordinal: 0,
                payload: VariantPayloadRequirement::Unit,
            },
            CheckedSemanticValueEvidence::VariantCase {
                owner: TypeKind::Option(Box::new(declared.clone())).semantic_identity_digest(),
                ordinal: 1,
                payload: VariantPayloadRequirement::Present,
            },
        ] {
            assert!(!rule.selects(0, &declared, &tampered));
        }
    }

    #[test]
    fn intrinsic_schema_seals_candidate_and_result_first_use_rows() {
        let option = crate::callable::OptionConstructorKind::Some.signature_schema();
        let option_item = option
            .generic_inventory()
            .types()
            .iter()
            .find(|entry| entry.role() == CallableSchemaGenericRole::Candidate)
            .expect("Option candidate row");
        assert_eq!(
            option_item.first_use(),
            CallableGenericFirstUse::Group(CallableGroupIndex::ZERO)
        );

        let result = crate::callable::ResultConstructorKind::Ok.signature_schema();
        let result_only = result
            .generic_inventory()
            .types()
            .iter()
            .filter(|entry| entry.first_use() == CallableGenericFirstUse::Result)
            .count();
        assert_eq!(
            result_only, 1,
            "the unused Result side first occurs in result"
        );
        assert!(
            result
                .generic_inventory()
                .types()
                .iter()
                .all(|entry| entry.role() == CallableSchemaGenericRole::Candidate)
        );
    }

    #[test]
    fn explicit_issuer_classifies_foreign_types_rigid_and_retains_const_references() {
        let candidate = accepted_type(10, 0);
        let foreign = accepted_type(11, 0);
        let enclosing = GenericTypeParameterId::new(
            GenericParameterOwnerId::AcceptedNominal(AcceptedNominalId::new(
                AcceptedNominalOwnerId::Standard,
                arcweft_lang_syntax::types::TypePath::from(
                    arcweft_lang_syntax::ast::symbol_path::ProjectSymbolPath::new(
                        arcweft_lang_syntax::ast::module_path::ModulePathRoot::ImplicitCrate,
                        [
                            arcweft_lang_syntax::ast::symbol_path::ProjectSymbolSegment::try_new(
                                "Enclosing",
                            )
                            .expect("enclosing nominal segment"),
                        ],
                    )
                    .expect("enclosing nominal path"),
                ),
            )),
            2,
        );
        let constant = GenericConstParameterId::new(
            GenericParameterOwnerId::AcceptedNominal(accepted_owner(12)),
            4,
        );
        let declared = TypeKind::Tuple(vec![
            TypeKind::GenericParam(candidate.clone()),
            TypeKind::GenericParam(foreign.clone()),
            TypeKind::GenericParam(enclosing.clone()),
            TypeKind::Array {
                item: Box::new(TypeKind::I32),
                len: ArrayLength::Generic(constant.clone()),
            },
        ]);
        let schema = CallableSignatureSchema::try_new(
            vec![group(0, vec![parameter(0, declared)])],
            TypeKind::GenericParam(candidate.clone()),
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::accepted_nominal(accepted_owner(10), 1, 0)
                .expect("typed candidate issuer"),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("schema inventory");

        let types = schema.generic_inventory().types();
        assert_eq!(
            types
                .iter()
                .find(|entry| entry.parameter() == &candidate)
                .expect("candidate row")
                .role(),
            CallableSchemaGenericRole::Candidate
        );
        assert_eq!(
            types
                .iter()
                .find(|entry| entry.parameter() == &foreign)
                .expect("foreign row")
                .role(),
            CallableSchemaGenericRole::RigidReference
        );
        assert_eq!(
            types
                .iter()
                .find(|entry| entry.parameter() == &enclosing)
                .expect("enclosing row")
                .role(),
            CallableSchemaGenericRole::RigidReference
        );
        assert_eq!(
            schema
                .generic_inventory()
                .rigid_consts()
                .first()
                .expect("rigid const row")
                .parameter(),
            &constant
        );

        let later = accepted_type(13, 0);
        let later_schema = CallableSignatureSchema::try_new(
            vec![
                group(0, vec![parameter(0, TypeKind::I32)]),
                group(1, vec![parameter(0, TypeKind::GenericParam(later.clone()))]),
            ],
            TypeKind::GenericParam(later.clone()),
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::accepted_nominal(accepted_owner(13), 1, 0)
                .expect("later-group issuer"),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("later-group schema");
        assert_eq!(
            later_schema
                .generic_inventory()
                .types()
                .first()
                .expect("later candidate row")
                .first_use(),
            CallableGenericFirstUse::Group(
                CallableGroupIndex::try_from_usize(1).expect("group one")
            )
        );
    }

    #[test]
    fn issuer_tampering_rejects_invalid_arity_missing_candidates_and_inferable_consts() {
        assert!(matches!(
            CallableGenericParameterIssuer::language_intrinsic(
                LanguageIntrinsicGenericOwner::OptionConstructor,
                2,
                0,
            ),
            Err(CallableSchemaError::InvalidCandidateIssuer)
        ));
        assert!(matches!(
            CallableGenericParameterIssuer::language_intrinsic(
                LanguageIntrinsicGenericOwner::OptionConstructor,
                0,
                0,
            ),
            Err(CallableSchemaError::InvalidCandidateIssuer)
        ));

        let missing = CallableGenericParameterIssuer::accepted_nominal(accepted_owner(20), 1, 0)
            .expect("candidate issuer");
        let error = CallableSignatureSchema::try_new(
            vec![group(0, vec![parameter(0, TypeKind::I32)])],
            TypeKind::Unit,
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            missing,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect_err("omitted candidate must not disappear");
        assert!(matches!(
            error,
            CallableSchemaError::MissingCandidateType { .. }
        ));

        let omitted_const =
            CallableGenericParameterIssuer::accepted_nominal(accepted_owner(22), 0, 1)
                .expect("const candidate issuer");
        let error = CallableSignatureSchema::try_new(
            vec![group(0, vec![parameter(0, TypeKind::I32)])],
            TypeKind::Unit,
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            omitted_const,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect_err("omitted const candidate must not disappear");
        assert!(matches!(
            error,
            CallableSchemaError::MissingCandidateConst { .. }
        ));

        let constant = GenericConstParameterId::new(
            GenericParameterOwnerId::AcceptedNominal(accepted_owner(21)),
            0,
        );
        let inferable = CallableGenericParameterIssuer::accepted_nominal(accepted_owner(21), 0, 1)
            .expect("const candidate issuer");
        let error = CallableSignatureSchema::try_new(
            vec![group(
                0,
                vec![parameter(
                    0,
                    TypeKind::Array {
                        item: Box::new(TypeKind::I32),
                        len: ArrayLength::Generic(constant.clone()),
                    },
                )],
            )],
            TypeKind::Unit,
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            inferable,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect_err("inferable const candidate must be rejected");
        assert!(matches!(
            error,
            CallableSchemaError::InferableConstGeneric { parameter } if parameter == constant
        ));
    }

    #[test]
    fn inventory_role_and_first_use_are_digest_committed_deterministically() {
        let candidate = accepted_type(30, 0);
        let ty = TypeKind::Map {
            kind: MapKind::Sorted,
            key: Box::new(TypeKind::GenericParam(candidate.clone())),
            value: Box::new(TypeKind::I32),
        };
        let policy = CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::FixedLiteralOnly,
        );
        let issuer = CallableGenericParameterIssuer::accepted_nominal(accepted_owner(30), 1, 0)
            .expect("issuer");
        let make = || {
            CallableSignatureSchema::try_new(
                vec![group(0, vec![parameter(0, ty.clone())])],
                TypeKind::Unit,
                effects(),
                policy,
                CallableValidator::Ordinary,
                issuer.clone(),
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("schema")
        };
        let first = make();
        let second = make();
        assert_eq!(first.semantic_digest(), second.semantic_digest());

        let rigid = CallableSignatureSchema::try_new(
            vec![group(0, vec![parameter(0, ty)])],
            TypeKind::Unit,
            effects(),
            policy,
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("rigid schema");
        assert_ne!(first.semantic_digest(), rigid.semantic_digest());
    }
}
