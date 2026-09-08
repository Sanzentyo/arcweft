//! Callable documentation, source evidence, and shared signature schemas.

use std::{
    collections::{BTreeSet, HashSet},
    sync::Arc,
};

use arcweft_lang_hir::symbol::{CallableDeclarationKey, CallableDeclarationOwner};
use arcweft_presentation::rich_text::{
    PresentationContentAttachedBodyPolicy, PresentationContentCallableDefinition,
    PresentationContentCallableDefinitionId,
};
use arcweft_rich_text_schema::RichTextCallableSchemaDigest;
use arcweft_source::SourceSpan;

use crate::{
    character_dialogue::CharacterDialogueFieldCoordinate,
    effect_row::EffectRow,
    env::{FunctionParam, FunctionSignature, nominal::AcceptedNominalId},
    record_field::AcceptedRecordFieldSemanticId,
    types::{
        AcceptedVariantPayloadFieldSemanticId, GenericConstParameterId, GenericParameterOwnerId,
        GenericTypeParameterId, LanguageIntrinsicGenericOwner, SemanticTypeDigest, TypeKind,
        VariantPayloadShape,
    },
};

use super::{
    AdapterPackageId, AgentIntrinsicSignatureId, BuiltinCallableId, CallableDocumentationError,
    CallableGroupIndex, CallableLimits, CallableName, CallableParameterCoordinate,
    CallableParameterIndex, CallableSchemaError, CallableSourceError, CapacityMethodId,
    CollectionMethodId, ContentCallableIdentity, DetachedCallableDeclarationId, DialogueCallableId,
    DomainMethodId, DropCallableId, EnumVariantSignatureId, FxSourceConstructor, IntegerMethodId,
    LanguageDocumentationFamily, LineContextMethodId, OptionConstructorKind,
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

/// Result contract owned by a callable signature.
///
/// Ordinary callables produce a typed runtime value. Content-emission
/// callables produce a typed content operation instead; they do not smuggle a
/// placeholder runtime `TypeKind` through the result contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableResultSchema {
    Value(TypeKind),
    ContentEmission(ContentCallableIdentity),
}

impl From<TypeKind> for CallableResultSchema {
    fn from(value: TypeKind) -> Self {
        Self::Value(value)
    }
}

impl CallableResultSchema {
    pub const fn value_type(&self) -> Option<&TypeKind> {
        match self {
            Self::Value(value) => Some(value),
            Self::ContentEmission(_) => None,
        }
    }

    pub const fn content_emission(&self) -> Option<ContentCallableIdentity> {
        match self {
            Self::Value(_) => None,
            Self::ContentEmission(identity) => Some(*identity),
        }
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        if let Self::Value(value) = self {
            visitor(value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSignatureSchema {
    core: CallableSignatureContents,
    digest: super::CallableSignatureSchemaDigest,
}

/// Construction data owned by the signature seal. Only a sealed schema can be
/// published; canonical encoding finishes before that outer carrier is made.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallableSignatureContents {
    pub(super) groups: Arc<[CallableParameterGroup]>,
    pub(super) result: CallableResultSchema,
    pub(super) generic_inventory: CallableGenericParameterInventory,
    pub(super) effects: CallableEffectSchema,
    pub(super) argument_policy: CallableArgumentPolicy,
    pub(super) reserved_open_names: Arc<[CallableName]>,
    pub(super) validator: CallableValidator,
    pub(super) attached_content: Option<CallableAttachedContentParameter>,
    pub(super) dependency: Option<CallableSchemaDependency>,
    pub(super) evaluated_effect: Option<CallableEvaluatedEffect>,
    pub(super) extension_receiver: Option<CallableExtensionReceiver>,
}

/// Typed owner selected by the declaration or intrinsic schema issuer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CallableGenericIssuerOwner {
    Callable(CallableDeclarationKey),
    ProjectNominal(arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationId),
    AcceptedNominal(AcceptedNominalId),
    LanguageIntrinsic(LanguageIntrinsicGenericOwner),
}

/// Authenticated formal-parameter inventory for a declaration or a function
/// scheme. Anonymous slots retain lexical references, never fabricated
/// declaration identities. The schema seal derives uses from its type graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableGenericParameterIssuer {
    authority: CallableGenericParameterAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CallableGenericParameterAuthority {
    Empty,
    Declaration {
        owner: CallableGenericIssuerOwner,
        type_count: u16,
        const_count: u16,
    },
    FunctionScheme(crate::types::GenericBinder),
}

impl CallableGenericParameterIssuer {
    /// Selects one declaration-owned constructor template from a contextual
    /// owner type. Contextual arguments are not copied into the child solver.
    pub(crate) fn for_enum_constructor_type(
        ty: &TypeKind,
        symbols: &arcweft_lang_hir::symbol::ProjectSymbolTable,
    ) -> Result<(Self, TypeKind), CallableSchemaError> {
        let (owner, type_count) = match ty {
            TypeKind::ProjectNominal(nominal) => {
                let declaration = symbols
                    .nominal(nominal.declaration())
                    .ok_or(CallableSchemaError::InvalidCandidateIssuer)?;
                if !matches!(
                    declaration.body(),
                    arcweft_lang_hir::symbol::nominal::ProjectNominalBody::Enum { .. }
                ) || declaration.type_parameters().len() != nominal.arguments().len()
                {
                    return Err(CallableSchemaError::InvalidCandidateIssuer);
                }
                let count = u16::try_from(declaration.type_parameters().len())
                    .map_err(|_| CallableSchemaError::InvalidCandidateIssuer)?;
                (
                    CallableGenericIssuerOwner::ProjectNominal(declaration.id().clone()),
                    count,
                )
            }
            TypeKind::Option(_) => (
                CallableGenericIssuerOwner::LanguageIntrinsic(
                    LanguageIntrinsicGenericOwner::OptionConstructor,
                ),
                1,
            ),
            TypeKind::Result { .. } => (
                CallableGenericIssuerOwner::LanguageIntrinsic(
                    LanguageIntrinsicGenericOwner::ResultConstructor,
                ),
                2,
            ),
            _ => return Ok((Self::empty(), ty.clone())),
        };
        let issuer = Self::new(owner, type_count, 0)?;
        let parameters = issuer
            .type_parameters()?
            .into_iter()
            .map(TypeKind::GenericParam)
            .collect::<Vec<_>>();
        let template = match ty {
            TypeKind::ProjectNominal(nominal) => TypeKind::ProjectNominal(
                crate::types::ProjectNominalType::new(nominal.declaration().clone(), parameters),
            ),
            TypeKind::Option(_) => TypeKind::Option(Box::new(parameters[0].clone())),
            TypeKind::Result { .. } => TypeKind::Result {
                ok: Box::new(parameters[0].clone()),
                error: Box::new(parameters[1].clone()),
            },
            _ => unreachable!("only parameterized constructor owners reach template construction"),
        };
        Ok((issuer, template))
    }

    pub fn empty() -> Self {
        Self {
            authority: CallableGenericParameterAuthority::Empty,
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

    fn function_scheme(binder: crate::types::GenericBinder) -> Self {
        Self {
            authority: CallableGenericParameterAuthority::FunctionScheme(binder),
        }
    }

    fn new(
        owner: CallableGenericIssuerOwner,
        type_count: u16,
        const_count: u16,
    ) -> Result<Self, CallableSchemaError> {
        if let CallableGenericIssuerOwner::LanguageIntrinsic(owner) = &owner {
            if (type_count, const_count) != owner.generic_arity() {
                return Err(CallableSchemaError::InvalidCandidateIssuer);
            }
        }
        Ok(Self {
            authority: CallableGenericParameterAuthority::Declaration {
                owner,
                type_count,
                const_count,
            },
        })
    }

    fn template_scope(&self) -> crate::types::GenericScope {
        match self.authority {
            CallableGenericParameterAuthority::FunctionScheme(binder) => {
                crate::types::GenericScope::default().with_binder(binder)
            }
            _ => crate::types::GenericScope::default(),
        }
    }

    fn type_parameters(
        &self,
    ) -> Result<Vec<crate::types::GenericTypeReference>, CallableSchemaError> {
        match &self.authority {
            CallableGenericParameterAuthority::Empty => Ok(Vec::new()),
            CallableGenericParameterAuthority::Declaration { type_count, .. } => {
                let owner = self
                    .generic_owner()
                    .expect("declaration issuer owns declaration parameters");
                Ok((0..*type_count)
                    .map(|slot| GenericTypeParameterId::new(owner.clone(), slot).into())
                    .collect())
            }
            CallableGenericParameterAuthority::FunctionScheme(binder) => {
                let scope = self.template_scope();
                (0..binder.types())
                    .map(|slot| scope.bound_type(0, slot).map_err(Into::into))
                    .collect()
            }
        }
    }

    fn const_parameters(
        &self,
    ) -> Result<Vec<crate::types::GenericConstReference>, CallableSchemaError> {
        match &self.authority {
            CallableGenericParameterAuthority::Empty => Ok(Vec::new()),
            CallableGenericParameterAuthority::Declaration { const_count, .. } => {
                let owner = self
                    .generic_owner()
                    .expect("declaration issuer owns declaration parameters");
                Ok((0..*const_count)
                    .map(|slot| GenericConstParameterId::new(owner.clone(), slot).into())
                    .collect())
            }
            CallableGenericParameterAuthority::FunctionScheme(binder) => {
                let scope = self.template_scope();
                (0..binder.const_lengths())
                    .map(|slot| scope.bound_const(0, slot).map_err(Into::into))
                    .collect()
            }
        }
    }

    fn owns_type(&self, parameter: &crate::types::GenericTypeReference) -> bool {
        match parameter {
            crate::types::GenericTypeReference::Free(parameter) => self
                .generic_owner()
                .is_some_and(|owner| parameter.owner() == &owner),
            crate::types::GenericTypeReference::Bound(parameter) => {
                parameter.depth() == 0
                    && matches!(
                        self.authority,
                        CallableGenericParameterAuthority::FunctionScheme(_)
                    )
            }
            crate::types::GenericTypeReference::Inference(_) => false,
        }
    }

    fn owns_const(&self, parameter: &crate::types::GenericConstReference) -> bool {
        match parameter {
            crate::types::GenericConstReference::Free(parameter) => self
                .generic_owner()
                .is_some_and(|owner| parameter.owner() == &owner),
            crate::types::GenericConstReference::Bound(parameter) => {
                parameter.depth() == 0
                    && matches!(
                        self.authority,
                        CallableGenericParameterAuthority::FunctionScheme(_)
                    )
            }
            crate::types::GenericConstReference::Inference(_) => false,
        }
    }

    fn generic_owner(&self) -> Option<GenericParameterOwnerId> {
        let CallableGenericParameterAuthority::Declaration { owner, .. } = &self.authority else {
            return None;
        };
        Some(match owner {
            CallableGenericIssuerOwner::Callable(declaration) => {
                GenericParameterOwnerId::Callable(declaration.clone())
            }
            CallableGenericIssuerOwner::ProjectNominal(declaration) => {
                GenericParameterOwnerId::Nominal(declaration.clone())
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
    template_scope: crate::types::GenericScope,
    types: Arc<[CallableGenericTypeUse]>,
    consts: Arc<[CallableGenericConstUse]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallableGenericTypeUse {
    parameter: crate::types::GenericTypeReference,
    role: CallableSchemaGenericRole,
    first_use: CallableGenericFirstUse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallableGenericConstUse {
    parameter: crate::types::GenericConstReference,
    role: CallableSchemaGenericRole,
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
    pub(crate) const fn template_scope(&self) -> &crate::types::GenericScope {
        &self.template_scope
    }

    pub(crate) fn template_binder(&self) -> crate::types::GenericBinder {
        self.template_scope
            .binders()
            .first()
            .copied()
            .unwrap_or(crate::types::GenericBinder::EMPTY)
    }
    pub(crate) fn types(&self) -> &[CallableGenericTypeUse] {
        &self.types
    }

    pub(crate) fn consts(&self) -> &[CallableGenericConstUse] {
        &self.consts
    }
}

impl CallableGenericTypeUse {
    pub(crate) const fn parameter(&self) -> &crate::types::GenericTypeReference {
        &self.parameter
    }

    pub(crate) const fn role(&self) -> CallableSchemaGenericRole {
        self.role
    }

    pub(crate) const fn first_use(&self) -> CallableGenericFirstUse {
        self.first_use
    }
}

impl CallableGenericConstUse {
    pub(crate) const fn parameter(&self) -> &crate::types::GenericConstReference {
        &self.parameter
    }

    pub(crate) const fn role(&self) -> CallableSchemaGenericRole {
        self.role
    }

    pub(crate) const fn first_use(&self) -> CallableGenericFirstUse {
        self.first_use
    }
}

/// Call-site identity for one deliberately open named argument. The schema
/// digest prevents two open slots from different signatures from colliding;
/// the authored name is canonicalized through `CallableName` before issuance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct OpenArgumentSemanticDigest([u8; 32]);

impl OpenArgumentSemanticDigest {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

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

    /// Issues the one opaque semantic identity for this open argument.  The
    /// schema digest and owner-issued callable binding bytes are hashed
    /// directly; no string representation or duplicate wire grammar is
    /// introduced at this boundary.
    pub(crate) fn semantic_digest(&self) -> OpenArgumentSemanticDigest {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.lang.open-argument-semantic.v1\0");
        hasher.update(self.schema.as_bytes());
        hasher.update(self.binding.canonical_identity_bytes());
        OpenArgumentSemanticDigest(*hasher.finalize().as_bytes())
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
    Drop(DropCallableId),
}

/// Callable-owned semantic role of one fixed evaluated-effect operand.
///
/// Open Log/Event fields use [`OpenArgumentId`] instead. Keeping fixed roles
/// on the evaluated-effect owner prevents statement analysis from rematching
/// parameter names or copying standard schemas.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CallableEvaluatedEffectOperandRole {
    Message,
    Target,
    Value,
    Event,
    Condition,
    Policy,
}

impl CallableEvaluatedEffect {
    /// Declaration-owned closed effect row required to execute this evaluated
    /// operation. The mapping lives on the semantic operation identity so
    /// schema producers, dialogue callbacks, and ordinary effect accounting
    /// cannot diverge or infer capabilities from runtime variants.
    pub(crate) fn declared_effect_row(self) -> EffectRow {
        let label = match self {
            Self::Log(_) => Some("log.write"),
            Self::SignalWrite => Some("signal.write"),
            Self::MetricWrite => Some("metric.write"),
            Self::EmitEvent => Some("event.emit"),
            Self::Panic | Self::Fail | Self::Bail | Self::Ensure | Self::Drop(_) => None,
        };
        EffectRow::closed(
            label
                .into_iter()
                .map(|label| {
                    crate::effects::EffectId::parse(label)
                        .expect("evaluated-effect identities are canonical static semantics")
                })
                .collect(),
        )
    }

    /// Resolves an exact schema coordinate to its closed effect operand role.
    pub(crate) const fn operand_role(
        self,
        coordinate: CallableParameterCoordinate,
    ) -> Option<CallableEvaluatedEffectOperandRole> {
        use CallableEvaluatedEffectOperandRole::{
            Condition, Event, Message, Policy, Target, Value,
        };

        let group = coordinate.group().get();
        let parameter = coordinate.parameter().get();
        match (self, group, parameter) {
            (Self::Log(_), 0, 0) | (Self::Panic | Self::Fail | Self::Bail, 0, 0) => Some(Message),
            (Self::SignalWrite | Self::MetricWrite, 0, 0) => Some(Target),
            (Self::SignalWrite | Self::MetricWrite, 0, 1) => Some(Value),
            (Self::EmitEvent, 0, 0) => Some(Event),
            (Self::Ensure, 0, 0) => Some(Condition),
            (Self::Ensure, 0, 1) => Some(Message),
            (Self::Drop(DropCallableId::Drop | DropCallableId::DropOptional), 0, 0)
            | (Self::Drop(DropCallableId::DropWithPolicy), 1, 0) => Some(Target),
            (Self::Drop(DropCallableId::DropWithPolicy), 0, 0) => Some(Policy),
            (Self::Drop(DropCallableId::OnDrop), _, _)
            | (Self::Log(_), _, _)
            | (Self::SignalWrite, _, _)
            | (Self::MetricWrite, _, _)
            | (Self::EmitEvent, _, _)
            | (Self::Panic, _, _)
            | (Self::Fail, _, _)
            | (Self::Bail, _, _)
            | (Self::Ensure, _, _)
            | (Self::Drop(_), _, _) => None,
        }
    }

    /// Whether the schema may project callable-owned open field identities.
    pub(crate) const fn accepts_open_fields(self) -> bool {
        matches!(self, Self::Log(_) | Self::EmitEvent)
    }
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
    semantic_binding: CallableParameterSemanticBinding,
    admission: CallableParameterAdmission,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    consumer: CallableParameterConsumer,
    documentation: Option<Arc<str>>,
    source: Option<CallableParameterSource>,
}

/// Stable digest payload for one schema-owned dependency definition.
///
/// The callable layer cannot depend on the text-proxy owner module, so this
/// opaque digest is deliberately issued by that owner and carried here as a
/// typed value. It is never reconstructed from a source/display name.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableSchemaDependencyDigest([u8; 32]);

impl CallableSchemaDependencyDigest {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Closed owner identity carried by a context-dependent callable schema.
///
/// The text-proxy row contains both the nominal owner identity and the exact
/// owner-issued definition digest. Consequently two declarations with equal
/// callable parameter shapes remain distinct schema dependencies.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableSchemaDependency {
    TextProxy {
        owner: SemanticTypeDigest,
        definition: CallableSchemaDependencyDigest,
    },
    PresentationContent {
        definition: PresentationContentCallableDefinitionId,
        schema: RichTextCallableSchemaDigest,
    },
}

impl CallableSchemaDependency {
    /// Constructs the callable-layer projection of an owner-issued text-proxy
    /// definition identity.
    pub(crate) const fn text_proxy(
        owner: SemanticTypeDigest,
        definition: CallableSchemaDependencyDigest,
    ) -> Self {
        Self::TextProxy { owner, definition }
    }

    pub(crate) const fn presentation_content(
        definition: PresentationContentCallableDefinitionId,
        schema: RichTextCallableSchemaDigest,
    ) -> Self {
        Self::PresentationContent { definition, schema }
    }

    pub const fn owner(self) -> Option<SemanticTypeDigest> {
        match self {
            Self::TextProxy { owner, .. } => Some(owner),
            Self::PresentationContent { .. } => None,
        }
    }

    pub const fn definition_digest(self) -> Option<CallableSchemaDependencyDigest> {
        match self {
            Self::TextProxy { definition, .. } => Some(definition),
            Self::PresentationContent { .. } => None,
        }
    }

    pub const fn presentation_content_definition(
        self,
    ) -> Option<PresentationContentCallableDefinitionId> {
        match self {
            Self::TextProxy { .. } => None,
            Self::PresentationContent { definition, .. } => Some(definition),
        }
    }

    pub const fn presentation_content_schema(self) -> Option<RichTextCallableSchemaDigest> {
        match self {
            Self::TextProxy { .. } => None,
            Self::PresentationContent { schema, .. } => Some(schema),
        }
    }

    /// Stable zero-based tag for the dependency family.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::TextProxy { .. } => 0,
            Self::PresentationContent { .. } => 1,
        }
    }

    fn valid_for(
        self,
        validator: &CallableValidator,
        attached_content: Option<CallableAttachedContentParameter>,
    ) -> bool {
        match (self, validator, attached_content) {
            (
                Self::TextProxy { .. },
                &CallableValidator::Content(ContentCallableIdentity::TextProxyObject {
                    owner,
                    definition,
                }),
                Some(parameter),
            ) => {
                parameter == CallableAttachedContentParameter::text_proxy_object()
                    && owner == self.owner().expect("text proxy dependency owner")
                    && definition
                        == self
                            .definition_digest()
                            .expect("text proxy dependency digest")
            }
            (
                Self::PresentationContent { definition, schema },
                &CallableValidator::Content(identity),
                Some(parameter),
            ) => {
                identity == ContentCallableIdentity::language(definition, schema)
                    && arcweft_presentation::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG
                        .get(definition)
                        .is_some_and(|row| {
                            identity.schema() == Some(row.schema_digest())
                                && parameter
                                    == CallableAttachedContentParameter::from_presentation_row(row)
                        })
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CallableParameterSemanticBinding {
    Coordinate,
    Named(CallableName),
    AcceptedVariantPayloadField(AcceptedVariantPayloadFieldSemanticId),
    AcceptedProjectRecordField(AcceptedRecordFieldSemanticId),
}

/// Callable-owned identity projection of the shared compile-time scalar kinds.
/// This is an admission identity only; checked scalar values and their decoding
/// remain owned by the compile-time scalar authority.
///
/// This deliberately lives with callable schema ownership instead of
/// importing the analyzer's checked-value implementation. Keeping this small
/// identity algebra here
/// avoids a reverse module dependency while retaining the exact checked scalar
/// distinction needed by the Object schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableCompileTimeScalarKind {
    Bool,
    Int,
    Milli,
    Ratio,
    Length,
    Angle,
    Duration,
    /// The digest of the accepted payload-free project-enum type schema.
    ClosedEnum(SemanticTypeDigest),
    PublicId,
    Text,
    Color,
}

impl CallableCompileTimeScalarKind {
    /// Stable zero-based tag for the closed scalar admission identity.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Bool => 0,
            Self::Int => 1,
            Self::Milli => 2,
            Self::Ratio => 3,
            Self::Length => 4,
            Self::Angle => 5,
            Self::Duration => 6,
            Self::ClosedEnum(_) => 7,
            Self::PublicId => 8,
            Self::Text => 9,
            Self::Color => 10,
        }
    }
}

/// Semantic-only admission used by callable families whose values are not
/// represented by an ordinary `TypeKind` in the call schema.
///
/// `TextProxyNominal` is a closed family marker for the exact text-proxy
/// nominal admission. Scalar fields carry their checked scalar kind; they do
/// not degrade to `Any`, `Named`, or a source spelling.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CallableCompileTimeScalarAdmission {
    kind: CallableCompileTimeScalarKind,
    value_type: TypeKind,
}

impl CallableCompileTimeScalarAdmission {
    pub fn try_new(kind: CallableCompileTimeScalarKind, value_type: TypeKind) -> Option<Self> {
        let valid = match (kind, &value_type) {
            (CallableCompileTimeScalarKind::Bool, TypeKind::Bool)
            | (CallableCompileTimeScalarKind::Int, TypeKind::I64)
            | (CallableCompileTimeScalarKind::Duration, TypeKind::Duration)
            | (CallableCompileTimeScalarKind::Text, TypeKind::String) => true,
            (CallableCompileTimeScalarKind::Milli, TypeKind::CompileTimeScalar(value)) => {
                value.kind() == crate::types::CompileTimeScalarKind::Milli
            }
            (CallableCompileTimeScalarKind::Ratio, TypeKind::CompileTimeScalar(value)) => {
                value.kind() == crate::types::CompileTimeScalarKind::Ratio
            }
            (CallableCompileTimeScalarKind::Length, TypeKind::CompileTimeScalar(value)) => {
                value.kind() == crate::types::CompileTimeScalarKind::Length
            }
            (CallableCompileTimeScalarKind::Angle, TypeKind::CompileTimeScalar(value)) => {
                value.kind() == crate::types::CompileTimeScalarKind::Angle
            }
            (CallableCompileTimeScalarKind::PublicId, TypeKind::CompileTimeScalar(value)) => {
                value.kind() == crate::types::CompileTimeScalarKind::PublicId
            }
            (CallableCompileTimeScalarKind::Color, TypeKind::CompileTimeScalar(value)) => {
                value.kind() == crate::types::CompileTimeScalarKind::Color
            }
            (CallableCompileTimeScalarKind::ClosedEnum(owner), value) => {
                value
                    .semantic_identity_digest()
                    .is_ok_and(|digest| digest == owner)
                    && matches!(value, TypeKind::ProjectNominal(nominal) if nominal.arguments().is_empty())
            }
            _ => false,
        };
        if !valid {
            return None;
        }
        Some(Self { kind, value_type })
    }

    pub const fn kind(&self) -> CallableCompileTimeScalarKind {
        self.kind
    }

    pub const fn value_type(&self) -> &TypeKind {
        &self.value_type
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CallableSemanticAdmission {
    TextProxyNominal,
    CompileTimeScalar(CallableCompileTimeScalarAdmission),
}

impl CallableSemanticAdmission {
    /// Stable zero-based tag for the closed semantic admission family.
    pub const fn semantic_tag(&self) -> u8 {
        match self {
            Self::TextProxyNominal => 0,
            Self::CompileTimeScalar(_) => 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableParameterAdmission {
    Checked {
        declared: TypeKind,
        rule: CallableParameterValueRule,
    },
    Semantic(CallableSemanticAdmission),
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

/// Variant discriminator with the owner representation supplied by its phase.
///
/// This is independent of the selected schema alternative: an `otherwise`
/// row still retains a variant case when the checked value is a variant that
/// did not match any guarded row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticValueEvidence<Owner> {
    VariantCase {
        owner: Owner,
        ordinal: u32,
        payload: VariantPayloadRequirement,
    },
    NoVariantCase,
}

/// Stable source discriminator after type projection and evidence completion.
pub type CheckedSemanticValueEvidence = SemanticValueEvidence<SemanticTypeDigest>;

/// Source observation before candidate-owned variables have been resolved.
pub(crate) type ObservedSemanticValueEvidence = SemanticValueEvidence<TypeKind>;

impl<Owner> SemanticValueEvidence<Owner> {
    pub(crate) fn try_project_owner<T, E>(
        &self,
        project: impl FnOnce(&Owner) -> Result<T, E>,
    ) -> Result<SemanticValueEvidence<T>, E> {
        Ok(match self {
            Self::VariantCase {
                owner,
                ordinal,
                payload,
            } => SemanticValueEvidence::VariantCase {
                owner: project(owner)?,
                ordinal: *ordinal,
                payload: *payload,
            },
            Self::NoVariantCase => SemanticValueEvidence::NoVariantCase,
        })
    }
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

/// Semantic destination of a content callable parameter.
///
/// Object parameters are kept as typed coordinates rather than being
/// recovered from authored names. A custom field retains the accepted field
/// identity issued by the project-record owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableContentParameterConsumer {
    ObjectId,
    ObjectType,
    ObjectRole,
    ObjectLayer,
    ObjectDepth,
    ObjectHitTest,
    ObjectCustomField(AcceptedRecordFieldSemanticId),
}

impl CallableContentParameterConsumer {
    /// Stable zero-based tag for the Object content-parameter coordinate.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::ObjectId => 0,
            Self::ObjectType => 1,
            Self::ObjectRole => 2,
            Self::ObjectLayer => 3,
            Self::ObjectDepth => 4,
            Self::ObjectHitTest => 5,
            Self::ObjectCustomField(_) => 6,
        }
    }

    /// Returns whether a scalar admission is the exact built-in Object kind
    /// for this coordinate. Custom fields retain their own checked scalar
    /// kind and therefore accept every member of the closed scalar algebra.
    pub const fn accepts_scalar_kind(self, kind: CallableCompileTimeScalarKind) -> bool {
        match self {
            Self::ObjectId | Self::ObjectRole | Self::ObjectLayer => {
                matches!(kind, CallableCompileTimeScalarKind::PublicId)
            }
            Self::ObjectDepth => matches!(kind, CallableCompileTimeScalarKind::Length),
            Self::ObjectHitTest => matches!(kind, CallableCompileTimeScalarKind::Bool),
            Self::ObjectCustomField(_) => true,
            Self::ObjectType => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableParameterConsumer {
    Value,
    DialoguePatch(CharacterDialogueFieldCoordinate),
    DialogueApplicationMetadata(DialogueApplicationMetadataCoordinate),
    Content(CallableContentParameterConsumer),
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
        self.accepts_owner(checked, |owner, checked_owner| {
            owner
                .apply_to(declared)
                .semantic_identity_digest()
                .is_ok_and(|digest| digest == *checked_owner)
        })
    }

    pub(crate) fn accepts_observation(
        &self,
        declared: &TypeKind,
        observed: &ObservedSemanticValueEvidence,
    ) -> bool {
        self.accepts_owner(observed, |owner, observed_owner| {
            owner.apply_to(declared) == *observed_owner
        })
    }

    fn accepts_owner<Owner>(
        &self,
        checked: &SemanticValueEvidence<Owner>,
        owner_accepts: impl FnOnce(&ParameterExpectedTypeProjection, &Owner) -> bool,
    ) -> bool {
        match (self, checked) {
            (
                Self::VariantCase {
                    owner,
                    ordinal,
                    payload,
                },
                SemanticValueEvidence::VariantCase {
                    owner: checked_owner,
                    ordinal: checked_ordinal,
                    payload: checked_payload,
                },
            ) => {
                owner_accepts(owner, checked_owner)
                    && ordinal == checked_ordinal
                    && payload == checked_payload
            }
            (Self::VariantCase { .. }, SemanticValueEvidence::NoVariantCase) => false,
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

    pub const fn semantic(admission: CallableSemanticAdmission) -> Self {
        Self::Semantic(admission)
    }

    pub const fn text_proxy_nominal() -> Self {
        Self::Semantic(CallableSemanticAdmission::TextProxyNominal)
    }

    pub fn compile_time_scalar(
        kind: CallableCompileTimeScalarKind,
        value_type: TypeKind,
    ) -> Option<Self> {
        CallableCompileTimeScalarAdmission::try_new(kind, value_type)
            .map(CallableSemanticAdmission::CompileTimeScalar)
            .map(Self::Semantic)
    }

    pub const fn declared(&self) -> Option<&TypeKind> {
        match self {
            Self::Checked { declared, .. } => Some(declared),
            Self::Semantic(_) | Self::UncheckedSupply => None,
        }
    }

    pub const fn rule(&self) -> Option<&CallableParameterValueRule> {
        match self {
            Self::Checked { rule, .. } => Some(rule),
            Self::Semantic(_) | Self::UncheckedSupply => None,
        }
    }

    pub const fn semantic_admission(&self) -> Option<&CallableSemanticAdmission> {
        match self {
            Self::Semantic(admission) => Some(admission),
            Self::Checked { .. } | Self::UncheckedSupply => None,
        }
    }

    pub const fn is_unchecked(&self) -> bool {
        matches!(self, Self::UncheckedSupply)
    }

    pub const fn is_semantic(&self) -> bool {
        matches!(self, Self::Semantic(_))
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

impl From<CallableSemanticAdmission> for CallableParameterAdmission {
    fn from(value: CallableSemanticAdmission) -> Self {
        Self::Semantic(value)
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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

/// Checked role admitted by an attached content body.
///
/// The role is semantic admission evidence, not a runtime `TypeKind`. In
/// particular, an attached body is never represented as a nominal
/// `DialogueContent` type in the callable schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedContentRole {
    Inline,
    Rich,
    Dialogue,
}

/// Final admission selected for one checked attached body. Literal content is
/// intentionally not assigned a fabricated Inline/Rich/Dialogue role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedAttachedContentAdmission {
    Role(CheckedContentRole),
    Literal,
}

impl CheckedAttachedContentAdmission {
    pub const fn role(self) -> Option<CheckedContentRole> {
        match self {
            Self::Role(role) => Some(role),
            Self::Literal => None,
        }
    }

    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Role(CheckedContentRole::Inline) => 0,
            Self::Role(CheckedContentRole::Rich) => 1,
            Self::Role(CheckedContentRole::Dialogue) => 2,
            Self::Literal => 3,
        }
    }
}

impl CheckedContentRole {
    /// Stable zero-based tag for the closed checked body-role identity.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Inline => 0,
            Self::Rich => 1,
            Self::Dialogue => 2,
        }
    }
}

/// Admission policy for a callable's attached content body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableAttachedContentPolicy {
    PreserveBodyRole,
    InlineOnly,
    RichOnly,
    LiteralOnly,
    Declared(CheckedContentRole),
}

/// Execution ownership of a callable's attached content body.
///
/// Presentation and text-proxy content callables consume their body while
/// sealing the checked content tree. Project/user callables instead receive a
/// runtime `DialogueContent` value through the dedicated attached-content
/// call channel. This distinction is schema authority; consumers must not
/// infer it from the selected callable family or result type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableAttachedContentExecution {
    Structural,
    RuntimeContent,
}

impl CallableAttachedContentExecution {
    /// Stable zero-based tag for the closed execution-owner identity.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Structural => 0,
            Self::RuntimeContent => 1,
        }
    }
}

/// Schema-owned description of an attached content parameter.
///
/// Presence and role are orthogonal: `Optional` means the body may be omitted;
/// it does not wrap the body role in an `Option` type. Keeping both facts in
/// one value prevents a bare policy from silently losing requiredness.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableAttachedContentParameter {
    group: CallableGroupIndex,
    presence: CallableParameterPresence,
    policy: CallableAttachedContentPolicy,
    execution: CallableAttachedContentExecution,
}

impl CallableAttachedContentParameter {
    /// Returns the complete attached-content parameter owned by a text-proxy
    /// Object schema.
    pub const fn text_proxy_object() -> Self {
        Self::new(
            CallableGroupIndex::ZERO,
            CallableParameterPresence::Required,
            CallableAttachedContentPolicy::RichOnly,
            CallableAttachedContentExecution::Structural,
        )
    }

    /// Returns the attached-content parameter for an exact Content identity.
    /// Presentation rows remain the source of language body policy; the
    /// dynamic Object identity has its closed Rich-only policy here.
    pub(crate) fn for_content_identity(identity: ContentCallableIdentity) -> Option<Self> {
        match identity {
            ContentCallableIdentity::TextProxyObject { .. } => Some(Self::text_proxy_object()),
            ContentCallableIdentity::Language { definition, .. } => {
                arcweft_presentation::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG
                    .get(definition)
                    .map(Self::from_presentation_row)
            }
        }
    }

    /// Projects the attached-body contract from one already-resolved catalog
    /// row. Callers that also validate the row digest use this entry point so
    /// one catalog lookup remains the sole source of the presentation policy.
    pub(crate) const fn from_presentation_row(row: &PresentationContentCallableDefinition) -> Self {
        Self::new(
            CallableGroupIndex::ZERO,
            CallableParameterPresence::Required,
            CallableAttachedContentPolicy::from_presentation_policy(row.attached_body_policy()),
            CallableAttachedContentExecution::Structural,
        )
    }

    pub const fn new(
        group: CallableGroupIndex,
        presence: CallableParameterPresence,
        policy: CallableAttachedContentPolicy,
        execution: CallableAttachedContentExecution,
    ) -> Self {
        Self {
            group,
            presence,
            policy,
            execution,
        }
    }

    pub const fn group(self) -> CallableGroupIndex {
        self.group
    }

    pub const fn presence(self) -> CallableParameterPresence {
        self.presence
    }

    pub const fn policy(self) -> CallableAttachedContentPolicy {
        self.policy
    }

    pub const fn execution(self) -> CallableAttachedContentExecution {
        self.execution
    }

    /// Resolves the final body admission from this callee-owned policy and
    /// the already checked surrounding role.
    pub const fn admission(
        self,
        surrounding: CheckedContentRole,
    ) -> CheckedAttachedContentAdmission {
        match self.policy {
            CallableAttachedContentPolicy::PreserveBodyRole => {
                CheckedAttachedContentAdmission::Role(surrounding)
            }
            CallableAttachedContentPolicy::InlineOnly => {
                CheckedAttachedContentAdmission::Role(CheckedContentRole::Inline)
            }
            CallableAttachedContentPolicy::RichOnly => {
                CheckedAttachedContentAdmission::Role(CheckedContentRole::Rich)
            }
            CallableAttachedContentPolicy::LiteralOnly => CheckedAttachedContentAdmission::Literal,
            CallableAttachedContentPolicy::Declared(role) => {
                CheckedAttachedContentAdmission::Role(role)
            }
        }
    }
}

impl CallableAttachedContentPolicy {
    /// Returns the body contract for one exact presentation content row.
    pub(crate) const fn from_presentation_policy(
        policy: PresentationContentAttachedBodyPolicy,
    ) -> Self {
        match policy {
            PresentationContentAttachedBodyPolicy::PreserveBodyRole => Self::PreserveBodyRole,
            PresentationContentAttachedBodyPolicy::InlineOnly => Self::InlineOnly,
            PresentationContentAttachedBodyPolicy::LiteralOnly => Self::LiteralOnly,
        }
    }

    /// Stable zero-based tag for the policy constructor. Declared roles carry
    /// their own checked role tag after this constructor tag.
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::PreserveBodyRole => 0,
            Self::InlineOnly => 1,
            Self::RichOnly => 2,
            Self::LiteralOnly => 3,
            Self::Declared(_) => 4,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableValidator {
    Ordinary,
    FxConstructor(FxSourceConstructor),
    BuiltinFx(arcweft_presentation::fx::BuiltinFxCallableRowId),
    UnknownFxMember { member: CallableName },
    EnumConstructor(EnumVariantSignatureId),
    ResultConstructor(ResultConstructorKind),
    OptionConstructor(OptionConstructorKind),
    ReductionConstructor(ReductionConstructorKind),
    Builtin(BuiltinCallableId),
    Agent(AgentIntrinsicSignatureId),
    Presentation(PresentationCallableId),
    Dialogue(DialogueCallableId),
    Content(ContentCallableIdentity),
    Collection(CollectionMethodId),
    StandardMap(super::StandardMapFamily),
    PresentationHandle(PresentationHandleMethodId),
    Integer(IntegerMethodId),
    Domain(DomainMethodId),
    Method(CallableMethodRole),
    Capacity(CapacityMethodId),
    Stage(StageMethodId),
    LineContext(LineContextMethodId),
    Drop(DropCallableId),
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
    fn seal(core: CallableSignatureContents) -> Result<Self, CallableSchemaError> {
        let digest = super::digest::schema_digest(&core)?;
        Ok(Self { core, digest })
    }

    pub const fn semantic_digest(&self) -> super::CallableSignatureSchemaDigest {
        self.digest
    }

    pub(crate) fn try_new(
        groups: Vec<CallableParameterGroup>,
        result: impl Into<CallableResultSchema>,
        effects: CallableEffectSchema,
        argument_policy: CallableArgumentPolicy,
        validator: CallableValidator,
        generic_issuer: CallableGenericParameterIssuer,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        Self::try_new_with_attached_content(
            groups,
            result,
            effects,
            argument_policy,
            validator,
            None,
            generic_issuer,
            limits,
        )
    }

    pub(crate) fn try_new_with_attached_content(
        groups: Vec<CallableParameterGroup>,
        result: impl Into<CallableResultSchema>,
        effects: CallableEffectSchema,
        argument_policy: CallableArgumentPolicy,
        validator: CallableValidator,
        attached_content: Option<CallableAttachedContentParameter>,
        generic_issuer: CallableGenericParameterIssuer,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        if matches!(validator, CallableValidator::Content(_)) {
            return Err(CallableSchemaError::FamilyInvariant {
                family: super::CallableFamily::Content,
                code: super::CallableFamilyInvariantCode::InvalidOwner,
            });
        }
        Self::try_new_with_attached_content_inner(
            groups,
            result,
            effects,
            argument_policy,
            validator,
            attached_content,
            None,
            generic_issuer,
            limits,
        )
    }

    /// Constructs a context-dependent schema only when its closed dependency
    /// has been supplied and agrees with the content validator/body contract.
    pub(crate) fn try_new_with_dependency(
        groups: Vec<CallableParameterGroup>,
        result: impl Into<CallableResultSchema>,
        effects: CallableEffectSchema,
        argument_policy: CallableArgumentPolicy,
        validator: CallableValidator,
        attached_content: Option<CallableAttachedContentParameter>,
        dependency: CallableSchemaDependency,
        generic_issuer: CallableGenericParameterIssuer,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        Self::try_new_with_attached_content_inner(
            groups,
            result,
            effects,
            argument_policy,
            validator,
            attached_content,
            Some(dependency),
            generic_issuer,
            limits,
        )
    }

    fn try_new_with_attached_content_inner(
        groups: Vec<CallableParameterGroup>,
        result: impl Into<CallableResultSchema>,
        effects: CallableEffectSchema,
        argument_policy: CallableArgumentPolicy,
        validator: CallableValidator,
        attached_content: Option<CallableAttachedContentParameter>,
        dependency: Option<CallableSchemaDependency>,
        generic_issuer: CallableGenericParameterIssuer,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        let result = result.into();
        if let CallableValidator::Content(identity) = &validator
            && attached_content != CallableAttachedContentParameter::for_content_identity(*identity)
        {
            return Err(CallableSchemaError::FamilyInvariant {
                family: super::CallableFamily::Content,
                code: super::CallableFamilyInvariantCode::InvalidValidator,
            });
        }
        if let CallableValidator::Content(identity) = &validator
            && !matches!(
                &result,
                CallableResultSchema::ContentEmission(result_identity) if result_identity == identity
            )
        {
            return Err(CallableSchemaError::FamilyInvariant {
                family: super::CallableFamily::Content,
                code: super::CallableFamilyInvariantCode::InvalidValidator,
            });
        }
        if matches!(&result, CallableResultSchema::ContentEmission(_))
            && !matches!(&validator, CallableValidator::Content(_))
        {
            return Err(CallableSchemaError::FamilyInvariant {
                family: super::CallableFamily::Content,
                code: super::CallableFamilyInvariantCode::InvalidValidator,
            });
        }
        if attached_content.is_some_and(|parameter| {
            matches!(&validator, CallableValidator::Content(_))
                != matches!(
                    parameter.execution(),
                    CallableAttachedContentExecution::Structural
                )
        }) {
            return Err(CallableSchemaError::FamilyInvariant {
                family: super::CallableFamily::Content,
                code: super::CallableFamilyInvariantCode::InvalidValidator,
            });
        }
        if attached_content.is_some_and(|parameter| {
            parameter.execution() == CallableAttachedContentExecution::RuntimeContent
                && !matches!(
                    parameter.policy(),
                    CallableAttachedContentPolicy::Declared(_)
                )
        }) {
            return Err(CallableSchemaError::FamilyInvariant {
                family: super::CallableFamily::Content,
                code: super::CallableFamilyInvariantCode::InvalidValidator,
            });
        }
        if let CallableValidator::Content(_) = &validator
            && dependency.is_none()
        {
            return Err(CallableSchemaError::FamilyInvariant {
                family: super::CallableFamily::Content,
                code: super::CallableFamilyInvariantCode::InvalidOwner,
            });
        }
        if dependency.is_some_and(|dependency| !dependency.valid_for(&validator, attached_content))
        {
            return Err(CallableSchemaError::FamilyInvariant {
                family: super::CallableFamily::Content,
                code: super::CallableFamilyInvariantCode::InvalidOwner,
            });
        }
        if groups.is_empty() {
            return Err(CallableSchemaError::EmptyGroups);
        }
        if groups.len() > limits.max_groups_per_callable() {
            return Err(CallableSchemaError::GroupLimit {
                actual: groups.len(),
                limit: limits.max_groups_per_callable(),
            });
        }
        let terminal_group =
            CallableGroupIndex::try_from_usize(groups.len() - 1).map_err(|_| {
                CallableSchemaError::GroupLimit {
                    actual: groups.len(),
                    limit: limits.max_groups_per_callable(),
                }
            })?;
        if let Some(parameter) = attached_content
            && parameter.group() != terminal_group
        {
            return Err(CallableSchemaError::InvalidAttachedContentGroup {
                expected: terminal_group,
                actual: parameter.group(),
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
                match (parameter.semantic_binding(), parameter.consumer()) {
                    (
                        CallableParameterSemanticBinding::AcceptedProjectRecordField(binding),
                        CallableParameterConsumer::Content(
                            CallableContentParameterConsumer::ObjectCustomField(field),
                        ),
                    ) if binding == field => {}
                    (CallableParameterSemanticBinding::AcceptedProjectRecordField(_), _)
                    | (
                        _,
                        CallableParameterConsumer::Content(
                            CallableContentParameterConsumer::ObjectCustomField(_),
                        ),
                    ) => {
                        return Err(CallableSchemaError::InvalidParameterConsumer {
                            group: group.index,
                            parameter: parameter.index,
                        });
                    }
                    (
                        CallableParameterSemanticBinding::Coordinate
                        | CallableParameterSemanticBinding::Named(_),
                        CallableParameterConsumer::Content(_),
                    ) => {}
                    (
                        CallableParameterSemanticBinding::AcceptedVariantPayloadField(_),
                        CallableParameterConsumer::Content(_),
                    ) => {
                        return Err(CallableSchemaError::InvalidParameterConsumer {
                            group: group.index,
                            parameter: parameter.index,
                        });
                    }
                    _ => {}
                }
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
                        if matches!(parameter.consumer(), CallableParameterConsumer::Content(_)) {
                            return Err(CallableSchemaError::InvalidParameterAdmission {
                                group: group.index,
                                parameter: parameter.index,
                            });
                        }
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
                    CallableParameterAdmission::Semantic(admission) => {
                        let valid_consumer = match (admission, parameter.consumer()) {
                            (
                                CallableSemanticAdmission::TextProxyNominal,
                                CallableParameterConsumer::Content(
                                    CallableContentParameterConsumer::ObjectType,
                                ),
                            ) => true,
                            (
                                CallableSemanticAdmission::CompileTimeScalar(admission),
                                CallableParameterConsumer::Content(consumer),
                            ) => consumer.accepts_scalar_kind(admission.kind()),
                            _ => false,
                        };
                        if !valid_consumer {
                            let error = if matches!(
                                parameter.consumer(),
                                CallableParameterConsumer::Content(_)
                            ) {
                                CallableSchemaError::InvalidParameterAdmission {
                                    group: group.index,
                                    parameter: parameter.index,
                                }
                            } else {
                                CallableSchemaError::InvalidParameterConsumer {
                                    group: group.index,
                                    parameter: parameter.index,
                                }
                            };
                            return Err(error);
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
        Self::seal(CallableSignatureContents {
            groups: groups.into(),
            result,
            generic_inventory,
            effects,
            argument_policy,
            reserved_open_names: Arc::new([]),
            validator,
            attached_content,
            dependency,
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
            && self.core.argument_policy.unknown_named() != UnknownNamedArgumentPolicy::OpenSupply
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
            self.core
                .groups
                .iter()
                .flat_map(|group| group.parameters())
                .any(|parameter| parameter.name() == Some(reserved))
        }) {
            let name = names
                .iter()
                .find(|reserved| {
                    self.core
                        .groups
                        .iter()
                        .flat_map(|group| group.parameters())
                        .any(|parameter| parameter.name() == Some(reserved))
                })
                .expect("reserved parameter collision was observed")
                .clone();
            return Err(CallableSchemaError::ReservedOpenNameParameterCollision { name });
        }
        self.core.reserved_open_names = names.into();
        Self::seal(self.core)
    }

    pub fn with_extension_receiver(
        mut self,
        receiver: CallableExtensionReceiver,
    ) -> Result<Self, CallableSchemaError> {
        if self.core.extension_receiver.is_some() {
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
        let receiver_data_last = receiver.group().get() + 1 == self.core.groups.len()
            && receiver.group().get() == 1
            && self.core.groups.len() == 2
            && group.parameters().len() == 1
            && receiver.parameter().get() == 0;
        if (!receiver_first && !receiver_data_last)
            || parameter.passing() != CallableParameterPassing::PositionalOnly
            || parameter.presence() != CallableParameterPresence::Required
            || parameter.admission().is_unchecked()
            || parameter.admission().is_semantic()
        {
            return Err(CallableSchemaError::InvalidExtensionReceiver {
                group: receiver.group(),
                parameter: receiver.parameter(),
            });
        }
        self.core.extension_receiver = Some(receiver);
        Self::seal(self.core)
    }

    pub(crate) fn with_evaluated_effect(
        mut self,
        effect: CallableEvaluatedEffect,
    ) -> Result<Self, CallableSchemaError> {
        if !matches!(self.core.effects, CallableEffectSchema::Fixed(_)) {
            return Err(CallableSchemaError::EvaluatedEffectRequiresFixedRow);
        }
        self.core.effects = CallableEffectSchema::fixed(effect.declared_effect_row());
        self.core.evaluated_effect = Some(effect);
        Self::seal(self.core)
    }
    pub fn groups(&self) -> &[CallableParameterGroup] {
        &self.core.groups
    }
    pub const fn result_schema(&self) -> &CallableResultSchema {
        &self.core.result
    }
    pub const fn value_type(&self) -> Option<&TypeKind> {
        self.core.result.value_type()
    }
    pub(crate) const fn generic_inventory(&self) -> &CallableGenericParameterInventory {
        &self.core.generic_inventory
    }
    pub const fn effects(&self) -> &CallableEffectSchema {
        &self.core.effects
    }
    pub const fn argument_policy(&self) -> CallableArgumentPolicy {
        self.core.argument_policy
    }
    pub fn reserved_open_names(&self) -> &[CallableName] {
        &self.core.reserved_open_names
    }
    pub(crate) fn allows_open_name(&self, name: &CallableName) -> bool {
        self.core.argument_policy.unknown_named() == UnknownNamedArgumentPolicy::OpenSupply
            && self.core.reserved_open_names.binary_search(name).is_err()
    }
    pub const fn validator(&self) -> &CallableValidator {
        &self.core.validator
    }
    pub const fn attached_content(&self) -> Option<CallableAttachedContentParameter> {
        self.core.attached_content
    }

    /// Returns the exact owner dependency of a context-dependent schema, when
    /// one exists. The dependency is an identity carrier rather than a
    /// source/display name and is included in the schema semantic digest.
    pub const fn dependency(&self) -> Option<CallableSchemaDependency> {
        self.core.dependency
    }

    pub const fn evaluated_effect(&self) -> Option<CallableEvaluatedEffect> {
        self.core.evaluated_effect
    }
    pub const fn extension_receiver(&self) -> Option<CallableExtensionReceiver> {
        self.core.extension_receiver
    }
    pub fn extension_receiver_type(&self) -> Option<&TypeKind> {
        let receiver = self.core.extension_receiver?;
        self.group(receiver.group())
            .and_then(|group| group.parameter(receiver.parameter()))
            .and_then(|parameter| parameter.declared_type())
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.result_schema().visit_types(visitor)?;
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
        self.core
            .groups
            .get(index.get())
            .filter(|group| group.index == index)
    }
    pub fn total_parameters(&self) -> usize {
        self.core
            .groups
            .iter()
            .map(|group| group.parameters.len())
            .sum()
    }

    pub fn semantic_eq(&self, other: &Self) -> bool {
        self.core.generic_inventory == other.core.generic_inventory
            && self.result_schema() == other.result_schema()
            && self.core.effects == other.core.effects
            && self.core.argument_policy == other.core.argument_policy
            && self.core.reserved_open_names == other.core.reserved_open_names
            && self.core.validator == other.core.validator
            && self.core.attached_content == other.core.attached_content
            && self.core.dependency == other.core.dependency
            && self.core.evaluated_effect == other.core.evaluated_effect
            && self.core.extension_receiver == other.core.extension_receiver
            && self.core.groups.len() == other.core.groups.len()
            && self
                .core
                .groups
                .iter()
                .zip(other.core.groups.iter())
                .all(|(left, right)| left.semantic_eq(right))
    }

    /// Builds the strict positional schema for an evaluated function value.
    pub(crate) fn for_function_value(
        ty: &TypeKind,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        let TypeKind::Function {
            binder,
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
            CallableGenericParameterIssuer::function_scheme(*binder),
            limits,
        )
    }

    /// Builds a constructor from one accepted declaration-template case and
    /// its authenticated formal parameters. Tuple and record payloads keep
    /// their distinct positional and named argument contracts; the application
    /// solution instantiates the result owner separately.
    pub(crate) fn for_accepted_enum_case(
        id: EnumVariantSignatureId,
        payload: &VariantPayloadShape,
        result: TypeKind,
        issuer: CallableGenericParameterIssuer,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError> {
        if id.owner() != result.semantic_identity_digest()? {
            return Err(CallableSchemaError::FamilyInvariant {
                family: super::CallableFamily::EnumConstructor,
                code: super::CallableFamilyInvariantCode::InvalidOwner,
            });
        }
        let mut parameters = Vec::with_capacity(payload.field_count());
        let mut push_parameter = |index: usize,
                                  name: Option<CallableName>,
                                  semantic_id: AcceptedVariantPayloadFieldSemanticId,
                                  ty: &TypeKind,
                                  passing: CallableParameterPassing|
         -> Result<(), CallableSchemaError> {
            let index = CallableParameterIndex::try_from_usize(index).map_err(|_| {
                CallableSchemaError::ParameterLimit {
                    actual: payload.field_count(),
                    limit: limits.max_parameters_per_callable(),
                }
            })?;
            parameters.push(CallableParameter::for_accepted_variant_payload_field(
                index,
                name,
                semantic_id,
                CallableParameterAdmission::checked(ty.clone()),
                passing,
                CallableParameterPresence::Required,
                None,
                None,
            )?);
            Ok(())
        };
        match payload {
            VariantPayloadShape::Unit => {}
            VariantPayloadShape::Tuple(fields) => {
                for (index, field) in fields.iter().enumerate() {
                    push_parameter(
                        index,
                        None,
                        field.semantic_id(),
                        field.ty(),
                        CallableParameterPassing::PositionalOnly,
                    )?;
                }
            }
            VariantPayloadShape::Record(fields) => {
                for (index, field) in fields.iter().enumerate() {
                    push_parameter(
                        index,
                        Some(CallableName::try_new(field.diagnostic_name()).map_err(|_| {
                            CallableSchemaError::FamilyInvariant {
                                family: super::CallableFamily::EnumConstructor,
                                code: super::CallableFamilyInvariantCode::InvalidParameterType,
                            }
                        })?),
                        field.semantic_id(),
                        field.ty(),
                        CallableParameterPassing::NamedOnly,
                    )?;
                }
            }
        }
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
            issuer,
            limits,
        )
    }
}

fn seal_generic_inventory(
    groups: &[CallableParameterGroup],
    result: &CallableResultSchema,
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
    if let CallableResultSchema::Value(result) = result {
        occurrences.push((result, result_position));
    }
    let template_scope = issuer.template_scope();
    let collected = crate::types::StableGenericReferenceUseCollector::collect_many_in_scope(
        occurrences,
        &template_scope,
    )?;
    let candidate_types = issuer.type_parameters()?;
    for parameter in &candidate_types {
        if !collected.types().contains(parameter) {
            return Err(CallableSchemaError::MissingCandidateType {
                parameter: parameter.clone(),
            });
        }
    }
    let candidate_consts = issuer.const_parameters()?;
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
        if issuer.owns_const(parameter) && !candidate_consts.contains(parameter) {
            return Err(CallableSchemaError::MissingCandidateConst {
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
    let candidate_consts = candidate_consts.iter().collect::<BTreeSet<_>>();
    let consts = collected
        .consts()
        .iter()
        .map(|parameter| CallableGenericConstUse {
            parameter: parameter.clone(),
            role: if candidate_consts.contains(parameter) {
                CallableSchemaGenericRole::Candidate
            } else {
                CallableSchemaGenericRole::RigidReference
            },
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
        consts,
        template_scope,
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
        let group_capacity =
            self.remaining_call_groups()
                .checked_add(1)
                .ok_or(CallableSchemaError::GroupLimit {
                    actual: usize::MAX,
                    limit: limits.max_groups_per_callable(),
                })?;
        let mut groups = Vec::with_capacity(group_capacity);
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
        CallableSignatureSchema::try_new(
            groups,
            self.body_return_type().clone(),
            CallableEffectSchema::fixed(effects),
            CallableArgumentPolicy::new(UnknownNamedArgumentPolicy::Reject, spread),
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
    let actual = index
        .checked_add(1)
        .ok_or(CallableSchemaError::GroupLimit {
            actual: usize::MAX,
            limit: limits.max_groups_per_callable(),
        })?;
    let group =
        CallableGroupIndex::try_from_usize(index).map_err(|_| CallableSchemaError::GroupLimit {
            actual,
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
        let semantic_binding = match passing {
            CallableParameterPassing::PositionalOnly | CallableParameterPassing::RestPositional => {
                CallableParameterSemanticBinding::Coordinate
            }
            CallableParameterPassing::PositionalOrNamed
            | CallableParameterPassing::NamedOnly
            | CallableParameterPassing::RestNamed => CallableParameterSemanticBinding::Named(
                name.clone()
                    .ok_or(CallableSchemaError::MissingParameterName {
                        group: CallableGroupIndex::ZERO,
                        parameter: index,
                    })?,
            ),
        };
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
            semantic_binding,
            admission,
            passing,
            presence,
            consumer: CallableParameterConsumer::Value,
            documentation,
            source,
        })
    }
    fn for_accepted_variant_payload_field(
        index: CallableParameterIndex,
        name: Option<CallableName>,
        semantic_id: AcceptedVariantPayloadFieldSemanticId,
        admission: CallableParameterAdmission,
        passing: CallableParameterPassing,
        presence: CallableParameterPresence,
        documentation: Option<Arc<str>>,
        source: Option<CallableParameterSource>,
    ) -> Result<Self, CallableSchemaError> {
        let mut parameter = Self::try_new(
            index,
            name,
            admission,
            passing,
            presence,
            documentation,
            source,
        )?;
        parameter.semantic_binding =
            CallableParameterSemanticBinding::AcceptedVariantPayloadField(semantic_id);
        Ok(parameter)
    }
    pub(crate) fn for_accepted_project_record_field(
        index: CallableParameterIndex,
        name: Option<CallableName>,
        semantic_id: AcceptedRecordFieldSemanticId,
        admission: CallableParameterAdmission,
        passing: CallableParameterPassing,
        presence: CallableParameterPresence,
        documentation: Option<Arc<str>>,
        source: Option<CallableParameterSource>,
    ) -> Result<Self, CallableSchemaError> {
        let mut parameter = Self::try_new(
            index,
            name,
            admission,
            passing,
            presence,
            documentation,
            source,
        )?;
        parameter.semantic_binding =
            CallableParameterSemanticBinding::AcceptedProjectRecordField(semantic_id);
        Ok(parameter)
    }
    pub const fn index(&self) -> CallableParameterIndex {
        self.index
    }
    pub fn name(&self) -> Option<&CallableName> {
        self.name.as_ref()
    }
    pub(super) const fn semantic_binding(&self) -> &CallableParameterSemanticBinding {
        &self.semantic_binding
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
            && self.semantic_binding == other.semantic_binding
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
pub(crate) use families::{fx_callable_schema, presentation_content_schema};

#[cfg(test)]
mod evaluated_effect_schema_tests {
    use super::*;

    #[test]
    fn evaluated_effect_owner_seals_the_exact_declared_runtime_row() {
        let cases = [
            (
                CallableEvaluatedEffect::Log(CallableLogLevel::Info),
                vec!["log.write"],
            ),
            (CallableEvaluatedEffect::SignalWrite, vec!["signal.write"]),
            (CallableEvaluatedEffect::MetricWrite, vec!["metric.write"]),
            (CallableEvaluatedEffect::EmitEvent, vec!["event.emit"]),
            (CallableEvaluatedEffect::Panic, Vec::new()),
            (CallableEvaluatedEffect::Fail, Vec::new()),
            (CallableEvaluatedEffect::Bail, Vec::new()),
            (CallableEvaluatedEffect::Ensure, Vec::new()),
            (
                CallableEvaluatedEffect::Drop(DropCallableId::Drop),
                Vec::new(),
            ),
        ];
        for (operation, expected) in cases {
            let row = operation.declared_effect_row();
            assert_eq!(row.tail(), crate::effect_row::EffectRowTail::Closed);
            assert_eq!(row.concrete().to_labels(), expected);
        }
    }

    #[test]
    fn evaluated_effect_annotation_replaces_the_generic_empty_schema_row() {
        let schema = BuiltinCallableId::Capability(super::super::CapabilityCallableId::EventEmit)
            .closed_signature_schema()
            .expect("event.emit closed schema");
        assert_eq!(
            schema
                .effects()
                .fixed_row()
                .expect("evaluated effect has one fixed row")
                .concrete()
                .to_labels(),
            ["event.emit"]
        );
    }
}

#[cfg(test)]
mod generic_inventory_tests {
    use super::*;

    #[test]
    fn function_value_schema_owns_anonymous_type_and_const_slots() {
        use crate::types::{ArrayLength, GenericBinder, GenericScope};
        let binder = GenericBinder::new(1, 1, 0);
        let scope = GenericScope::default().with_binder(binder);
        let type_key = scope.bound_type(0, 0).expect("scheme type");
        let const_key = scope.bound_const(0, 0).expect("scheme length");
        let function = TypeKind::function_with_binder(
            binder,
            [TypeKind::Array {
                item: Box::new(TypeKind::GenericParam(type_key.clone())),
                len: ArrayLength::Generic(const_key.clone()),
            }],
            TypeKind::GenericParam(type_key.clone()),
            EffectRow::closed(crate::effects::EffectSet::new()),
        );
        let schema = CallableSignatureSchema::for_function_value(
            &function,
            &super::super::PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("function scheme has a real slot inventory");
        let inventory = schema.generic_inventory();
        assert_eq!(inventory.template_scope(), &scope);
        let [ty] = inventory.types() else {
            panic!("one type slot");
        };
        let [constant] = inventory.consts() else {
            panic!("one const slot");
        };
        assert_eq!(ty.parameter(), &type_key);
        assert_eq!(constant.parameter(), &const_key);
        assert_eq!(ty.role(), CallableSchemaGenericRole::Candidate);
        assert_eq!(constant.role(), CallableSchemaGenericRole::Candidate);
        assert_eq!(
            ty.first_use(),
            CallableGenericFirstUse::Group(CallableGroupIndex::ZERO)
        );
        assert_eq!(
            constant.first_use(),
            CallableGenericFirstUse::Group(CallableGroupIndex::ZERO)
        );
        let again = CallableSignatureSchema::for_function_value(
            &function,
            &super::super::PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("same scheme");
        assert_eq!(schema.semantic_digest(), again.semantic_digest());
    }
    use crate::{
        callable::{
            CallableCandidateId, CallableFamily, CallableFamilyInvariantCode,
            PRODUCTION_CALLABLE_LIMITS,
        },
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
    fn schema_updates_publish_a_new_identity_with_canonical_reserved_names() {
        let schema = CallableSignatureSchema::try_new(
            vec![group(0, vec![])],
            TypeKind::Unit,
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::OpenSupply,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("initial schema");
        let names = |values: [&str; 2]| {
            values
                .into_iter()
                .map(|value| CallableName::try_new(value).expect("reserved name"))
                .collect()
        };
        let first = schema
            .clone()
            .try_with_reserved_open_names(names(["level", "category"]), &PRODUCTION_CALLABLE_LIMITS)
            .expect("updated schema");
        let second = schema
            .clone()
            .try_with_reserved_open_names(names(["category", "level"]), &PRODUCTION_CALLABLE_LIMITS)
            .expect("equivalent update");
        assert_ne!(schema.semantic_digest(), first.semantic_digest());
        assert_eq!(first.semantic_digest(), second.semantic_digest());
        assert_eq!(first, second);
        assert!(schema.reserved_open_names().is_empty());
    }

    #[test]
    fn environment_identity_rejects_an_active_receiver_before_ordering_or_hashing() {
        use crate::types::constraints::{
            TypeConstraintParameterEligibility, TypeConstraintParameterScope,
        };
        let parameter = accepted_type(92, 0);
        let scope = TypeConstraintParameterScope::new([(
            parameter.clone(),
            TypeConstraintParameterEligibility::Bindable,
        )])
        .expect("application scope");
        let reference = scope
            .type_reference(&(parameter).clone().into())
            .expect("opened variable");
        let receiver = TypeKind::Vec(Box::new(TypeKind::GenericParam(reference)));
        let identity = super::super::EnvironmentCallableId::try_new(
            super::super::EnvironmentCallableOwner::Standard(
                super::super::StandardEnvironmentId::Core,
            ),
            super::super::EnvironmentCallableKind::Method,
            super::super::CallableLookupKey::Method(super::super::ReceiverMethodKey::new(
                receiver,
                CallableName::try_new("len").expect("method"),
            )),
            super::super::CallableOverloadIndex::try_from_usize(0).expect("overload"),
        );
        assert!(matches!(
            identity,
            Err(crate::types::GenericScopeError::EscapedInference {
                kind: crate::types::GenericParameterKind::Type
            })
        ));
    }

    #[test]
    fn semantic_value_rule_selects_clear_guard_before_mandatory_otherwise() {
        let declared = TypeKind::I32;
        let rule = CallableParameterValueRule::clearable_option();
        let clear = CheckedSemanticValueEvidence::VariantCase {
            owner: TypeKind::Option(Box::new(declared.clone()))
                .semantic_identity_digest()
                .expect("stable fixture type"),
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
            owner: TypeKind::Option(Box::new(declared.clone()))
                .semantic_identity_digest()
                .expect("stable fixture type"),
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
    fn source_observation_keeps_an_inference_owner_until_projection() {
        use crate::types::{
            DetachedGenericOwnerId, GenericParameterOwnerId, GenericScopeError,
            GenericTypeParameterId,
            constraints::{TypeConstraintParameterEligibility, TypeConstraintParameterScope},
        };
        let parameter = GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(720)),
            0,
        );
        let scope = TypeConstraintParameterScope::new([(
            parameter.clone(),
            TypeConstraintParameterEligibility::Bindable,
        )])
        .expect("application parameter");
        let item = TypeKind::GenericParam(
            scope
                .type_reference(&(parameter).clone().into())
                .expect("active parameter"),
        );
        let observed = ObservedSemanticValueEvidence::VariantCase {
            owner: TypeKind::Option(Box::new(item.clone())),
            ordinal: 1,
            payload: VariantPayloadRequirement::Unit,
        };
        let rule = CallableParameterValueRule::clearable_option();
        let guard = rule.guarded()[0].guard();
        assert!(guard.accepts_observation(&item, &observed));
        assert!(matches!(
            observed.try_project_owner(TypeKind::semantic_identity_digest),
            Err(GenericScopeError::EscapedInference { .. }),
        ));

        let actual = TypeKind::Option(Box::new(TypeKind::I64));
        let checked = observed
            .try_project_owner(|_| actual.semantic_identity_digest())
            .expect("projected owner is stable");
        assert!(guard.accepts(&TypeKind::I64, &checked));
        assert!(!guard.accepts(&TypeKind::U64, &checked));
    }

    #[test]
    fn semantic_value_rule_rejects_tampered_clear_evidence_at_the_guarded_coordinate() {
        let declared = TypeKind::I32;
        let rule = CallableParameterValueRule::clearable_option();
        for tampered in [
            CheckedSemanticValueEvidence::VariantCase {
                owner: TypeKind::Option(Box::new(TypeKind::U32))
                    .semantic_identity_digest()
                    .expect("stable fixture type"),
                ordinal: 1,
                payload: VariantPayloadRequirement::Unit,
            },
            CheckedSemanticValueEvidence::VariantCase {
                owner: TypeKind::Option(Box::new(declared.clone()))
                    .semantic_identity_digest()
                    .expect("stable fixture type"),
                ordinal: 0,
                payload: VariantPayloadRequirement::Unit,
            },
            CheckedSemanticValueEvidence::VariantCase {
                owner: TypeKind::Option(Box::new(declared.clone()))
                    .semantic_identity_digest()
                    .expect("stable fixture type"),
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
            TypeKind::generic_parameter(candidate.clone()),
            TypeKind::generic_parameter(foreign.clone()),
            TypeKind::generic_parameter(enclosing.clone()),
            TypeKind::Array {
                item: Box::new(TypeKind::I32),
                len: ArrayLength::generic_parameter(constant.clone()),
            },
        ]);
        let schema = CallableSignatureSchema::try_new(
            vec![group(0, vec![parameter(0, declared)])],
            TypeKind::generic_parameter(candidate.clone()),
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
                .find(|entry| entry.parameter() == &candidate.clone().into())
                .expect("candidate row")
                .role(),
            CallableSchemaGenericRole::Candidate
        );
        assert_eq!(
            types
                .iter()
                .find(|entry| entry.parameter() == &foreign.clone().into())
                .expect("foreign row")
                .role(),
            CallableSchemaGenericRole::RigidReference
        );
        assert_eq!(
            types
                .iter()
                .find(|entry| entry.parameter() == &enclosing.clone().into())
                .expect("enclosing row")
                .role(),
            CallableSchemaGenericRole::RigidReference
        );
        assert_eq!(
            schema
                .generic_inventory()
                .consts()
                .first()
                .expect("rigid const row")
                .parameter(),
            &constant.clone().into()
        );

        let later = accepted_type(13, 0);
        let later_schema = CallableSignatureSchema::try_new(
            vec![
                group(0, vec![parameter(0, TypeKind::I32)]),
                group(
                    1,
                    vec![parameter(0, TypeKind::generic_parameter(later.clone()))],
                ),
            ],
            TypeKind::generic_parameter(later.clone()),
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
    fn issuer_tampering_rejects_invalid_arity_and_missing_candidates() {
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
        let schema = CallableSignatureSchema::try_new(
            vec![group(
                0,
                vec![parameter(
                    0,
                    TypeKind::Array {
                        item: Box::new(TypeKind::I32),
                        len: ArrayLength::generic_parameter(constant.clone()),
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
        .expect("equality-only const candidate is schema-owned");
        let [entry] = schema.generic_inventory().consts() else {
            panic!("one const candidate row")
        };
        assert_eq!(entry.parameter(), &constant.clone().into());
        assert_eq!(entry.role(), CallableSchemaGenericRole::Candidate);
    }

    #[test]
    fn inventory_role_and_first_use_are_digest_committed_deterministically() {
        let candidate = accepted_type(30, 0);
        let ty = TypeKind::Map {
            kind: MapKind::Sorted,
            key: Box::new(TypeKind::generic_parameter(candidate.clone())),
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

    #[test]
    fn object_content_callable_owns_rich_attached_body_policy() {
        assert_eq!(
            CallableAttachedContentPolicy::RichOnly,
            CallableAttachedContentPolicy::RichOnly,
        );
        assert_eq!(
            CallableAttachedContentParameter::text_proxy_object(),
            CallableAttachedContentParameter::new(
                CallableGroupIndex::ZERO,
                CallableParameterPresence::Required,
                CallableAttachedContentPolicy::RichOnly,
                CallableAttachedContentExecution::Structural,
            ),
        );
        let object = CallableSignatureSchema::try_new_with_dependency(
            vec![group(0, vec![parameter(0, TypeKind::Unit)])],
            CallableResultSchema::ContentEmission(ContentCallableIdentity::TextProxyObject {
                owner: SemanticTypeDigest::from_bytes([0; 32]),
                definition: CallableSchemaDependencyDigest::from_bytes([1; 32]),
            }),
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Content(ContentCallableIdentity::TextProxyObject {
                owner: SemanticTypeDigest::from_bytes([0; 32]),
                definition: CallableSchemaDependencyDigest::from_bytes([1; 32]),
            }),
            Some(CallableAttachedContentParameter::text_proxy_object()),
            CallableSchemaDependency::text_proxy(
                SemanticTypeDigest::from_bytes([0; 32]),
                CallableSchemaDependencyDigest::from_bytes([1; 32]),
            ),
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("Object schema body parameter metadata");
        assert_eq!(
            object.result_schema(),
            &CallableResultSchema::ContentEmission(ContentCallableIdentity::TextProxyObject {
                owner: SemanticTypeDigest::from_bytes([0; 32]),
                definition: CallableSchemaDependencyDigest::from_bytes([1; 32]),
            })
        );
        let ordinary_emission = CallableSignatureSchema::try_new(
            vec![group(0, vec![parameter(0, TypeKind::Unit)])],
            CallableResultSchema::ContentEmission(ContentCallableIdentity::TextProxyObject {
                owner: SemanticTypeDigest::from_bytes([0; 32]),
                definition: CallableSchemaDependencyDigest::from_bytes([1; 32]),
            }),
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect_err("content emission requires the matching content validator");
        assert!(matches!(
            ordinary_emission,
            CallableSchemaError::FamilyInvariant {
                family: CallableFamily::Content,
                code: CallableFamilyInvariantCode::InvalidValidator,
            }
        ));
        assert_eq!(
            object.attached_content(),
            Some(CallableAttachedContentParameter::text_proxy_object())
        );
        assert_eq!(CheckedContentRole::Inline.semantic_tag(), 0);
        assert_eq!(CheckedContentRole::Rich.semantic_tag(), 1);
        assert_eq!(CheckedContentRole::Dialogue.semantic_tag(), 2);
        assert_ne!(
            CallableCandidateId::Content(crate::callable::ContentCallableIdentity::language(
                PresentationContentCallableDefinitionId::Strong,
                arcweft_presentation::rich_text::PRESENTATION_CONTENT_CALLABLE_CATALOG
                    .get(PresentationContentCallableDefinitionId::Strong)
                    .expect("Strong content row")
                    .schema_digest(),
            )),
            CallableCandidateId::Dialogue(DialogueCallableId::ContentCall),
        );
    }

    #[test]
    fn object_content_parameters_retain_semantic_field_identity() {
        assert!(
            CallableParameterAdmission::compile_time_scalar(
                CallableCompileTimeScalarKind::Text,
                TypeKind::I64,
            )
            .is_none(),
            "a scalar kind cannot be paired with a different exact value type"
        );
        let owner = SemanticTypeDigest::from_bytes([31; 32]);
        let field = AcceptedRecordFieldSemanticId::issue(
            owner,
            2,
            TypeKind::String
                .semantic_identity_digest()
                .expect("stable fixture type"),
        );
        let parameter = CallableParameter::for_accepted_project_record_field(
            CallableParameterIndex::try_from_usize(0).expect("parameter index"),
            Some(CallableName::try_new("label").expect("field name")),
            field,
            CallableParameterAdmission::compile_time_scalar(
                CallableCompileTimeScalarKind::Text,
                TypeKind::String,
            )
            .expect("text scalar admission"),
            CallableParameterPassing::NamedOnly,
            CallableParameterPresence::Required,
            None,
            None,
        )
        .expect("semantic custom field parameter")
        .with_consumer(CallableParameterConsumer::Content(
            CallableContentParameterConsumer::ObjectCustomField(field),
        ));
        let schema = CallableSignatureSchema::try_new(
            vec![group(0, vec![parameter])],
            TypeKind::Unit,
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("semantic custom field schema");
        let parameter = &schema.groups()[0].parameters()[0];
        assert!(matches!(
            parameter.semantic_binding(),
            CallableParameterSemanticBinding::AcceptedProjectRecordField(id) if id == &field
        ));
        assert!(matches!(
            parameter.admission(),
            CallableParameterAdmission::Semantic(CallableSemanticAdmission::CompileTimeScalar(
                admission
            )) if admission.kind() == CallableCompileTimeScalarKind::Text
                && admission.value_type() == &TypeKind::String
        ));
        assert_eq!(
            parameter.consumer(),
            &CallableParameterConsumer::Content(
                CallableContentParameterConsumer::ObjectCustomField(field)
            )
        );
    }

    #[test]
    fn semantic_admission_is_closed_to_its_content_consumer() {
        let ordinary = parameter(0, TypeKind::String);
        let semantic = CallableParameter::try_new(
            CallableParameterIndex::try_from_usize(1).expect("parameter index"),
            Some(CallableName::try_new("type").expect("type name")),
            CallableParameterAdmission::text_proxy_nominal(),
            CallableParameterPassing::NamedOnly,
            CallableParameterPresence::Required,
            None,
            None,
        )
        .expect("semantic parameter")
        .with_consumer(CallableParameterConsumer::Value);
        let error = CallableSignatureSchema::try_new(
            vec![group(0, vec![ordinary, semantic])],
            TypeKind::Unit,
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect_err("semantic admissions require typed content consumers");
        assert!(matches!(
            error,
            CallableSchemaError::InvalidParameterConsumer { parameter, .. }
                if parameter == CallableParameterIndex::try_from_usize(1).expect("index")
        ));

        let named_fallback = parameter(0, TypeKind::Named("TextProxy".to_owned())).with_consumer(
            CallableParameterConsumer::Content(CallableContentParameterConsumer::ObjectId),
        );
        let error = CallableSignatureSchema::try_new(
            vec![group(0, vec![named_fallback])],
            TypeKind::Unit,
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect_err("named spellings must not stand in for semantic Object admission");
        assert!(matches!(
            error,
            CallableSchemaError::InvalidParameterAdmission { parameter, .. }
                if parameter == CallableParameterIndex::try_from_usize(0).expect("parameter index")
        ));

        let wrong_kind = CallableParameter::try_new(
            CallableParameterIndex::try_from_usize(0).expect("parameter index"),
            Some(CallableName::try_new("id").expect("id name")),
            CallableParameterAdmission::compile_time_scalar(
                CallableCompileTimeScalarKind::Text,
                TypeKind::String,
            )
            .expect("text scalar admission"),
            CallableParameterPassing::NamedOnly,
            CallableParameterPresence::Required,
            None,
            None,
        )
        .expect("semantic id parameter")
        .with_consumer(CallableParameterConsumer::Content(
            CallableContentParameterConsumer::ObjectId,
        ));
        let error = CallableSignatureSchema::try_new(
            vec![group(0, vec![wrong_kind])],
            TypeKind::Unit,
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect_err("Object id only admits the exact public-id scalar kind");
        assert!(matches!(
            error,
            CallableSchemaError::InvalidParameterAdmission { parameter, .. }
                if parameter == CallableParameterIndex::try_from_usize(0).expect("parameter index")
        ));
    }

    #[test]
    fn attached_content_policy_is_part_of_schema_identity() {
        let base_group = group(0, vec![parameter(0, TypeKind::String)]);
        let argument_policy = CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::FixedLiteralOnly,
        );
        let plain = CallableSignatureSchema::try_new(
            vec![base_group.clone()],
            TypeKind::String,
            effects(),
            argument_policy,
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("plain callable schema");
        let rich = CallableSignatureSchema::try_new_with_attached_content(
            vec![base_group.clone()],
            TypeKind::String,
            effects(),
            argument_policy,
            CallableValidator::Ordinary,
            Some(CallableAttachedContentParameter::new(
                CallableGroupIndex::ZERO,
                CallableParameterPresence::Optional,
                CallableAttachedContentPolicy::Declared(CheckedContentRole::Rich),
                CallableAttachedContentExecution::RuntimeContent,
            )),
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("rich content schema");
        let object_without_policy = CallableSignatureSchema::try_new(
            vec![base_group],
            TypeKind::String,
            effects(),
            argument_policy,
            CallableValidator::Content(ContentCallableIdentity::TextProxyObject {
                owner: SemanticTypeDigest::from_bytes([0; 32]),
                definition: CallableSchemaDependencyDigest::from_bytes([1; 32]),
            }),
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect_err("content validators must carry their fixed body policy");
        let object_optional = CallableSignatureSchema::try_new_with_attached_content(
            vec![group(0, vec![parameter(0, TypeKind::String)])],
            TypeKind::String,
            effects(),
            argument_policy,
            CallableValidator::Content(ContentCallableIdentity::TextProxyObject {
                owner: SemanticTypeDigest::from_bytes([0; 32]),
                definition: CallableSchemaDependencyDigest::from_bytes([1; 32]),
            }),
            Some(CallableAttachedContentParameter::new(
                CallableGroupIndex::ZERO,
                CallableParameterPresence::Optional,
                CallableAttachedContentPolicy::RichOnly,
                CallableAttachedContentExecution::Structural,
            )),
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect_err("Object content is required");
        let non_terminal = CallableSignatureSchema::try_new_with_attached_content(
            vec![
                group(0, vec![parameter(0, TypeKind::String)]),
                group(1, vec![parameter(0, TypeKind::String)]),
            ],
            TypeKind::String,
            effects(),
            argument_policy,
            CallableValidator::Ordinary,
            Some(CallableAttachedContentParameter::new(
                CallableGroupIndex::ZERO,
                CallableParameterPresence::Required,
                CallableAttachedContentPolicy::Declared(CheckedContentRole::Dialogue),
                CallableAttachedContentExecution::RuntimeContent,
            )),
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect_err("attached content belongs to the terminal group");

        assert_eq!(plain.attached_content(), None);
        assert_eq!(
            rich.attached_content(),
            Some(CallableAttachedContentParameter::new(
                CallableGroupIndex::ZERO,
                CallableParameterPresence::Optional,
                CallableAttachedContentPolicy::Declared(CheckedContentRole::Rich),
                CallableAttachedContentExecution::RuntimeContent,
            ))
        );
        assert_ne!(plain.semantic_digest(), rich.semantic_digest());
        assert!(!plain.semantic_eq(&rich));
        assert!(matches!(
            object_without_policy,
            CallableSchemaError::FamilyInvariant {
                family: CallableFamily::Content,
                code: CallableFamilyInvariantCode::InvalidOwner,
            }
        ));
        assert!(matches!(
            object_optional,
            CallableSchemaError::FamilyInvariant {
                family: CallableFamily::Content,
                code: CallableFamilyInvariantCode::InvalidOwner,
            }
        ));
        assert!(matches!(
            non_terminal,
            CallableSchemaError::InvalidAttachedContentGroup { expected, actual }
                if expected == CallableGroupIndex::try_from_usize(1).expect("terminal group")
                    && actual == CallableGroupIndex::ZERO
        ));
    }

    #[test]
    fn text_proxy_schema_dependency_commits_owner_and_definition_digest() {
        let dependency = |owner_byte, definition_byte| {
            CallableSchemaDependency::text_proxy(
                SemanticTypeDigest::from_bytes([owner_byte; 32]),
                CallableSchemaDependencyDigest::from_bytes([definition_byte; 32]),
            )
        };
        let base = |dependency| {
            CallableSignatureSchema::try_new_with_dependency(
                vec![group(0, vec![parameter(0, TypeKind::Unit)])],
                CallableResultSchema::ContentEmission({
                    let CallableSchemaDependency::TextProxy { owner, definition } = dependency
                    else {
                        panic!("test dependency is text proxy");
                    };
                    ContentCallableIdentity::TextProxyObject { owner, definition }
                }),
                effects(),
                CallableArgumentPolicy::new(
                    UnknownNamedArgumentPolicy::Reject,
                    SpreadArgumentPolicy::FixedLiteralOnly,
                ),
                CallableValidator::Content({
                    let CallableSchemaDependency::TextProxy { owner, definition } = dependency
                    else {
                        panic!("test dependency is text proxy");
                    };
                    ContentCallableIdentity::TextProxyObject { owner, definition }
                }),
                Some(CallableAttachedContentParameter::text_proxy_object()),
                dependency,
                CallableGenericParameterIssuer::empty(),
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("Object base schema")
        };
        let first = base(dependency(1, 2));
        let changed_owner = base(dependency(3, 2));
        let changed_definition = base(dependency(1, 4));
        assert_ne!(first.semantic_digest(), changed_owner.semantic_digest());
        assert_ne!(
            first.semantic_digest(),
            changed_definition.semantic_digest()
        );
        assert!(!first.semantic_eq(&changed_owner));
        assert!(!first.semantic_eq(&changed_definition));
        assert_eq!(
            first.dependency(),
            Some(dependency(1, 2)),
            "the complete closed dependency remains queryable"
        );

        let ordinary = CallableSignatureSchema::try_new_with_dependency(
            vec![group(0, vec![parameter(0, TypeKind::Unit)])],
            TypeKind::Unit,
            effects(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::FixedLiteralOnly,
            ),
            CallableValidator::Ordinary,
            None,
            dependency(1, 2),
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            ordinary,
            Err(CallableSchemaError::FamilyInvariant {
                family: CallableFamily::Content,
                code: CallableFamilyInvariantCode::InvalidOwner,
            })
        ));
    }
}

#[cfg(test)]
mod accepted_variant_schema_tests {
    use super::*;
    use crate::{callable::PRODUCTION_CALLABLE_LIMITS, types::VariantPayloadOwnerFamily};

    fn owner(ty: &TypeKind) -> SemanticTypeDigest {
        ty.semantic_identity_digest().expect("stable fixture type")
    }

    fn record_schema(
        result: &TypeKind,
        case: u32,
        label: &str,
        field_type: TypeKind,
    ) -> CallableSignatureSchema {
        let owner = owner(result);
        let payload = VariantPayloadShape::try_record(
            VariantPayloadOwnerFamily::BuiltinClosed,
            owner,
            case,
            [(label.to_owned(), field_type)],
        )
        .expect("accepted record payload");
        CallableSignatureSchema::for_accepted_enum_case(
            EnumVariantSignatureId::new(owner, case),
            &payload,
            result.clone(),
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("accepted record constructor schema")
    }

    #[test]
    fn accepted_record_labels_are_lookup_only_not_schema_identity() {
        let left = record_schema(&TypeKind::I32, 3, "fade", TypeKind::Duration);
        let right = record_schema(&TypeKind::I32, 3, "duration", TypeKind::Duration);
        let left_parameter = &left.groups()[0].parameters()[0];
        let right_parameter = &right.groups()[0].parameters()[0];

        assert_ne!(left_parameter.name(), right_parameter.name());
        assert_eq!(
            left_parameter.semantic_binding(),
            right_parameter.semantic_binding()
        );
        assert!(left.semantic_eq(&right));
        assert_eq!(left.semantic_digest(), right.semantic_digest());
    }

    #[test]
    fn accepted_record_field_owner_case_and_type_are_schema_identity() {
        let base = record_schema(&TypeKind::I32, 3, "fade", TypeKind::Duration);
        let changed_owner = record_schema(&TypeKind::I64, 3, "fade", TypeKind::Duration);
        let changed_case = record_schema(&TypeKind::I32, 4, "fade", TypeKind::Duration);
        let changed_type = record_schema(&TypeKind::I32, 3, "fade", TypeKind::I64);

        assert_ne!(base.semantic_digest(), changed_owner.semantic_digest());
        assert_ne!(base.semantic_digest(), changed_case.semantic_digest());
        assert_ne!(base.semantic_digest(), changed_type.semantic_digest());
    }

    #[test]
    fn accepted_tuple_constructor_has_no_fabricated_parameter_names() {
        let owner = owner(&TypeKind::I32);
        let payload = VariantPayloadShape::try_tuple(
            VariantPayloadOwnerFamily::BuiltinClosed,
            owner,
            2,
            [TypeKind::I64, TypeKind::Bool],
        )
        .expect("accepted tuple payload");
        let schema = CallableSignatureSchema::for_accepted_enum_case(
            EnumVariantSignatureId::new(owner, 2),
            &payload,
            TypeKind::I32,
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("accepted tuple constructor schema");

        assert!(schema.groups()[0].parameters().iter().all(|parameter| {
            parameter.name().is_none()
                && parameter.passing() == CallableParameterPassing::PositionalOnly
                && matches!(
                    parameter.semantic_binding(),
                    CallableParameterSemanticBinding::AcceptedVariantPayloadField(_)
                )
        }));
    }

    #[test]
    fn constructor_signature_owner_must_match_the_result_template() {
        let result = CallableSignatureSchema::for_accepted_enum_case(
            EnumVariantSignatureId::new(owner(&TypeKind::I32), 0),
            &VariantPayloadShape::Unit,
            TypeKind::I64,
            CallableGenericParameterIssuer::empty(),
            &PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            result,
            Err(CallableSchemaError::FamilyInvariant {
                family: super::super::CallableFamily::EnumConstructor,
                code: super::super::CallableFamilyInvariantCode::InvalidOwner,
            })
        ));
    }
}
