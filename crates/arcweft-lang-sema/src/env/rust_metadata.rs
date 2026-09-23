//! Source-backed Rust nominal metadata and its accepted immutable catalog.

mod join;
mod names;
pub(crate) use join::JoinedRustMetadata;
pub use join::{RustMetadataJoinError, RustMetadataJoinErrorKind};
pub use names::{RustMetadataNameProblem, RustMetadataNameScope};

use std::collections::BTreeMap;

use arcweft_rust_abi::ArcweftRustTypeParameterIndex;
use arcweft_source::SourceSpan;

use crate::{
    callable::{RustItemPath, RustPackageProvenance},
    registration::{EnvironmentPublicationItemId, EnvironmentTypeProjectionNode},
    types::{
        AcceptedNominalType, GenericTypeParameterId, TypeInstantiationError, TypeKind,
        TypeProjectionControl, TypeProjectionError,
    },
};

use super::{
    EnumVariantPayload, EnvironmentRecordField,
    nominal::{AcceptedNominalId, RustPackageId},
};

/// One Rust generic parameter retained before accepted-world projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTypeParameterPublicationInput {
    index: ArcweftRustTypeParameterIndex,
    name: String,
    source: SourceSpan,
}

/// Source-backed Rust struct shape awaiting type projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustStructMetadataInput {
    Unit,
    Tuple(Box<[EnvironmentTypeProjectionNode]>),
    Record(Box<[EnvironmentRecordField<EnvironmentTypeProjectionNode>]>),
}

/// Source-backed Rust enum variant awaiting type projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustVariantMetadataInput {
    name: String,
    payload: RustVariantPayloadInput,
    source: SourceSpan,
    wire_name: Option<String>,
    discriminant: Option<i128>,
}

/// Source-backed Rust enum payload awaiting type projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustVariantPayloadInput {
    Unit,
    Tuple(Box<[EnvironmentTypeProjectionNode]>),
    Record(Box<[EnvironmentRecordField<EnvironmentTypeProjectionNode>]>),
}

/// Source-backed Rust nominal shape awaiting accepted-world projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustTypeMetadataPublicationKind {
    Struct {
        shape: RustStructMetadataInput,
    },
    Enum {
        variants: Box<[RustVariantMetadataInput]>,
    },
    Newtype {
        inner: EnvironmentTypeProjectionNode,
    },
}

/// One Rust nominal declaration awaiting accepted-world metadata projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTypeMetadataPublicationInput {
    item: EnvironmentPublicationItemId,
    id: AcceptedNominalId,
    package: RustPackageId,
    package_provenance: RustPackageProvenance,
    rust_item: RustItemPath,
    parameters: Box<[RustTypeParameterPublicationInput]>,
    kind: RustTypeMetadataPublicationKind,
    source: SourceSpan,
    data_policy: arcweft_rust_abi::ArcweftRustDataTypePolicy,
}

/// Stable declaration and Rust provenance identity for one metadata publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTypeMetadataPublicationIdentity {
    item: EnvironmentPublicationItemId,
    id: AcceptedNominalId,
    package: RustPackageId,
    package_provenance: RustPackageProvenance,
    rust_item: RustItemPath,
}

/// Deterministic identity of an accepted Rust metadata catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedRustTypeMetadataDigest([u8; 32]);

/// Immutable accepted Rust nominal metadata catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedRustTypeMetadataCatalog {
    by_id: BTreeMap<AcceptedNominalId, AcceptedRustTypeMetadata>,
    digest: AcceptedRustTypeMetadataDigest,
}

/// One accepted Rust nominal metadata declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedRustTypeMetadata {
    item: EnvironmentPublicationItemId,
    id: AcceptedNominalId,
    package: RustPackageId,
    package_provenance: RustPackageProvenance,
    rust_item: RustItemPath,
    parameters: Box<[GenericTypeParameterId]>,
    kind: AcceptedRustTypeMetadataKind,
    source: SourceSpan,
    data_policy: arcweft_rust_abi::ArcweftRustDataTypePolicy,
}

/// One accepted Rust nominal shape after substituting an exact instantiation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstantiatedRustTypeMetadata {
    item: EnvironmentPublicationItemId,
    id: AcceptedNominalId,
    package: RustPackageId,
    package_provenance: RustPackageProvenance,
    rust_item: RustItemPath,
    kind: AcceptedRustTypeMetadataKind,
    source: SourceSpan,
    data_policy: arcweft_rust_abi::ArcweftRustDataTypePolicy,
}

/// Accepted Rust nominal shape with semantic type templates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptedRustTypeMetadataKind {
    Struct {
        shape: AcceptedRustStructShape,
    },
    Enum {
        /// Variants retain the declaration order supplied by the Rust
        /// metadata producer.  Variant ordinals are semantic, so this must
        /// not be normalized through a key-sorting map.
        variants: Box<[AcceptedRustVariantMetadata]>,
    },
    Newtype {
        inner: TypeKind,
    },
}

/// Accepted Rust struct payload shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptedRustStructShape {
    Unit,
    Tuple(Box<[TypeKind]>),
    Record(Box<[EnvironmentRecordField]>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedRustVariantMetadata {
    name: String,
    wire_name: String,
    discriminant: Option<i128>,
    payload: EnumVariantPayload,
}

impl AcceptedRustVariantMetadata {
    pub fn new(name: impl Into<String>, payload: EnumVariantPayload) -> Self {
        let name = name.into();
        Self {
            wire_name: name.clone(),
            name,
            discriminant: None,
            payload,
        }
    }
    pub fn with_wire_policy(
        mut self,
        wire_name: impl Into<String>,
        discriminant: Option<i128>,
    ) -> Self {
        self.wire_name = wire_name.into();
        self.discriminant = discriminant;
        self
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn wire_name(&self) -> &str {
        &self.wire_name
    }
    pub const fn discriminant(&self) -> Option<i128> {
        self.discriminant
    }
    pub const fn payload(&self) -> &EnumVariantPayload {
        &self.payload
    }
}

/// Exact source default request. The accepted callable catalog supplies the
/// executable proof for the fully instantiated field type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustFieldDefault {
    Trait,
    Function(RustItemPath),
}

impl RustTypeParameterPublicationInput {
    pub fn new(index: ArcweftRustTypeParameterIndex, name: String, source: SourceSpan) -> Self {
        Self {
            index,
            name,
            source,
        }
    }

    pub const fn index(&self) -> ArcweftRustTypeParameterIndex {
        self.index
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl RustVariantMetadataInput {
    pub fn new(name: String, payload: RustVariantPayloadInput, source: SourceSpan) -> Self {
        Self {
            name,
            payload,
            source,
            wire_name: None,
            discriminant: None,
        }
    }

    pub fn with_wire_policy(
        mut self,
        wire_name: Option<String>,
        discriminant: Option<i128>,
    ) -> Self {
        self.wire_name = wire_name;
        self.discriminant = discriminant;
        self
    }
    pub fn wire_name(&self) -> &str {
        self.wire_name.as_deref().unwrap_or(&self.name)
    }
    pub const fn discriminant(&self) -> Option<i128> {
        self.discriminant
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn payload(&self) -> &RustVariantPayloadInput {
        &self.payload
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl RustTypeMetadataPublicationInput {
    pub fn new(
        identity: RustTypeMetadataPublicationIdentity,
        parameters: impl Into<Box<[RustTypeParameterPublicationInput]>>,
        kind: RustTypeMetadataPublicationKind,
        source: SourceSpan,
    ) -> Self {
        let data_policy = arcweft_rust_abi::ArcweftRustDataTypePolicy::standard(
            identity
                .rust_item
                .as_str()
                .rsplit("::")
                .next()
                .expect("non-empty Rust path"),
        );
        Self {
            item: identity.item,
            id: identity.id,
            package: identity.package,
            package_provenance: identity.package_provenance,
            rust_item: identity.rust_item,
            parameters: parameters.into(),
            kind,
            source,
            data_policy,
        }
    }

    pub fn with_data_policy(
        mut self,
        policy: Option<arcweft_rust_abi::ArcweftRustDataTypePolicy>,
    ) -> Self {
        if let Some(policy) = policy {
            self.data_policy = policy;
        }
        self
    }
    pub const fn data_policy(&self) -> &arcweft_rust_abi::ArcweftRustDataTypePolicy {
        &self.data_policy
    }

    pub const fn item(&self) -> &EnvironmentPublicationItemId {
        &self.item
    }

    pub const fn id(&self) -> &AcceptedNominalId {
        &self.id
    }

    pub const fn package(&self) -> &RustPackageId {
        &self.package
    }

    pub const fn package_provenance(&self) -> &RustPackageProvenance {
        &self.package_provenance
    }

    pub const fn rust_item(&self) -> &RustItemPath {
        &self.rust_item
    }

    pub fn parameters(&self) -> &[RustTypeParameterPublicationInput] {
        &self.parameters
    }

    pub const fn kind(&self) -> &RustTypeMetadataPublicationKind {
        &self.kind
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl RustTypeMetadataPublicationIdentity {
    pub fn new(
        item: EnvironmentPublicationItemId,
        id: AcceptedNominalId,
        package: RustPackageId,
        package_provenance: RustPackageProvenance,
        rust_item: RustItemPath,
    ) -> Self {
        Self {
            item,
            id,
            package,
            package_provenance,
            rust_item,
        }
    }
}

impl Default for AcceptedRustTypeMetadataCatalog {
    fn default() -> Self {
        Self::try_new([]).expect("an empty Rust metadata catalog is valid")
    }
}

impl AcceptedRustTypeMetadataCatalog {
    pub(crate) fn try_new(
        records: impl IntoIterator<Item = AcceptedRustTypeMetadata>,
    ) -> Result<Self, AcceptedRustTypeMetadataCatalogError> {
        let mut by_id = BTreeMap::new();
        let mut package_claims = BTreeMap::<RustPackageId, RustPackageProvenance>::new();
        for record in records {
            record.kind.validate_names(&record.id)?;
            if let Some(first) = package_claims.get(record.package()) {
                if first != record.package_provenance() {
                    return Err(
                        AcceptedRustTypeMetadataCatalogError::PackageProvenanceConflict {
                            package: record.package().clone(),
                            first: first.clone(),
                            conflicting: record.package_provenance().clone(),
                        },
                    );
                }
            } else {
                package_claims.insert(
                    record.package().clone(),
                    record.package_provenance().clone(),
                );
            }
            let id = record.id().clone();
            if by_id.insert(id.clone(), record).is_some() {
                return Err(AcceptedRustTypeMetadataCatalogError::DuplicateNominal { id });
            }
        }
        let digest = metadata_catalog_digest(&by_id)?;
        Ok(Self { by_id, digest })
    }

    pub fn get(&self, id: &AcceptedNominalId) -> Option<&AcceptedRustTypeMetadata> {
        self.by_id.get(id)
    }

    pub const fn digest(&self) -> AcceptedRustTypeMetadataDigest {
        self.digest
    }

    pub fn instantiate(
        &self,
        nominal: &AcceptedNominalType,
    ) -> Result<InstantiatedRustTypeMetadata, RustMetadataInstantiationError> {
        self.instantiate_with_control(nominal, &mut crate::types::UnmeteredTypeProjection)
    }

    /// Substitutes one exact instance using the caller's transaction budget.
    pub fn instantiate_with_control<C: TypeProjectionControl>(
        &self,
        nominal: &AcceptedNominalType,
        control: &mut C,
    ) -> Result<InstantiatedRustTypeMetadata, RustMetadataInstantiationError<C::Error>> {
        control
            .check()
            .map_err(RustMetadataInstantiationError::Control)?;
        let metadata = self.by_id.get(nominal.declaration()).ok_or_else(|| {
            RustMetadataInstantiationError::UnknownNominal {
                id: nominal.declaration().clone(),
            }
        })?;
        if metadata.parameters.len() != nominal.arguments().len() {
            return Err(RustMetadataInstantiationError::WrongArity {
                id: nominal.declaration().clone(),
                expected: metadata.parameters.len(),
                actual: nominal.arguments().len(),
            });
        }
        let substitutions = metadata
            .parameters
            .iter()
            .zip(nominal.arguments())
            .collect::<BTreeMap<_, _>>();
        let kind = metadata
            .kind
            .try_map_types(&mut |ty| {
                ty.instantiate_type_parameters_with_control(&substitutions, control)
            })
            .map_err(|error| match error {
                TypeProjectionError::Instantiation(error) => {
                    RustMetadataInstantiationError::Type(error)
                }
                TypeProjectionError::Control(error) => {
                    RustMetadataInstantiationError::Control(error)
                }
            })?;
        Ok(InstantiatedRustTypeMetadata {
            item: metadata.item.clone(),
            id: metadata.id.clone(),
            package: metadata.package.clone(),
            package_provenance: metadata.package_provenance.clone(),
            rust_item: metadata.rust_item.clone(),
            kind,
            source: metadata.source.clone(),
            data_policy: metadata.data_policy.clone(),
        })
    }
}

impl AcceptedRustTypeMetadata {
    pub(crate) fn new(
        identity: RustTypeMetadataPublicationIdentity,
        parameters: impl Into<Box<[GenericTypeParameterId]>>,
        kind: AcceptedRustTypeMetadataKind,
        source: SourceSpan,
    ) -> Self {
        let data_policy = arcweft_rust_abi::ArcweftRustDataTypePolicy::standard(
            identity
                .rust_item
                .as_str()
                .rsplit("::")
                .next()
                .expect("non-empty Rust path"),
        );
        Self {
            item: identity.item,
            id: identity.id,
            package: identity.package,
            package_provenance: identity.package_provenance,
            rust_item: identity.rust_item,
            parameters: parameters.into(),
            kind,
            source,
            data_policy,
        }
    }

    pub(crate) fn with_data_policy(
        mut self,
        policy: arcweft_rust_abi::ArcweftRustDataTypePolicy,
    ) -> Self {
        self.data_policy = policy;
        self
    }

    pub const fn item(&self) -> &EnvironmentPublicationItemId {
        &self.item
    }

    pub const fn id(&self) -> &AcceptedNominalId {
        &self.id
    }

    pub const fn package(&self) -> &RustPackageId {
        &self.package
    }

    pub const fn package_provenance(&self) -> &RustPackageProvenance {
        &self.package_provenance
    }

    pub const fn rust_item(&self) -> &RustItemPath {
        &self.rust_item
    }

    pub fn parameters(&self) -> &[GenericTypeParameterId] {
        &self.parameters
    }

    pub const fn kind(&self) -> &AcceptedRustTypeMetadataKind {
        &self.kind
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl InstantiatedRustTypeMetadata {
    pub const fn data_policy(&self) -> &arcweft_rust_abi::ArcweftRustDataTypePolicy {
        &self.data_policy
    }
    pub const fn item(&self) -> &EnvironmentPublicationItemId {
        &self.item
    }
    pub const fn id(&self) -> &AcceptedNominalId {
        &self.id
    }

    pub const fn package(&self) -> &RustPackageId {
        &self.package
    }

    pub const fn package_provenance(&self) -> &RustPackageProvenance {
        &self.package_provenance
    }

    pub const fn rust_item(&self) -> &RustItemPath {
        &self.rust_item
    }

    pub const fn kind(&self) -> &AcceptedRustTypeMetadataKind {
        &self.kind
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }
}

impl AcceptedRustTypeMetadataKind {
    fn try_map_types<E>(
        &self,
        map: &mut impl FnMut(&TypeKind) -> Result<TypeKind, E>,
    ) -> Result<Self, E> {
        Ok(match self {
            Self::Struct { shape } => Self::Struct {
                shape: shape.try_map_types(map)?,
            },
            Self::Enum { variants } => Self::Enum {
                variants: variants
                    .iter()
                    .map(|variant| {
                        Ok(AcceptedRustVariantMetadata::new(
                            variant.name.clone(),
                            variant.payload.try_map_types(map)?,
                        )
                        .with_wire_policy(variant.wire_name.clone(), variant.discriminant))
                    })
                    .collect::<Result<Vec<_>, E>>()?
                    .into_boxed_slice(),
            },
            Self::Newtype { inner } => Self::Newtype { inner: map(inner)? },
        })
    }
}

impl AcceptedRustStructShape {
    fn try_map_types<E>(
        &self,
        map: &mut impl FnMut(&TypeKind) -> Result<TypeKind, E>,
    ) -> Result<Self, E> {
        Ok(match self {
            Self::Unit => Self::Unit,
            Self::Tuple(items) => {
                Self::Tuple(items.iter().map(&mut *map).collect::<Result<_, _>>()?)
            }
            Self::Record(fields) => Self::Record(
                fields
                    .iter()
                    .map(|field| field.try_map_type(&mut *map))
                    .collect::<Result<_, E>>()?,
            ),
        })
    }
}

impl AcceptedRustTypeMetadataDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Failure to instantiate metadata for one exact accepted Rust nominal.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum RustMetadataInstantiationError<E: std::error::Error + 'static = std::convert::Infallible> {
    #[error("Rust metadata instantiation was aborted: {0}")]
    Control(#[source] E),
    #[error(transparent)]
    Type(#[from] TypeInstantiationError),
    #[error("accepted Rust metadata does not contain nominal `{id:?}`")]
    UnknownNominal { id: AcceptedNominalId },
    #[error("accepted Rust nominal expects {expected} argument(s), but received {actual}")]
    WrongArity {
        id: AcceptedNominalId,
        expected: usize,
        actual: usize,
    },
}

/// Invalid composition of one immutable accepted Rust metadata catalog.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum AcceptedRustTypeMetadataCatalogError {
    #[error(transparent)]
    GenericScope(#[from] crate::types::GenericScopeError),
    #[error("accepted Rust metadata contains duplicate nominal `{id:?}`")]
    DuplicateNominal { id: AcceptedNominalId },
    #[error(
        "accepted Rust metadata {id:?} has invalid {scope:?} member {ordinal} `{name}`: {problem:?}"
    )]
    InvalidName {
        id: AcceptedNominalId,
        scope: RustMetadataNameScope,
        ordinal: usize,
        name: String,
        problem: RustMetadataNameProblem,
    },
    #[error("Rust package `{package}` has conflicting version or metadata-hash claims")]
    PackageProvenanceConflict {
        package: RustPackageId,
        first: RustPackageProvenance,
        conflicting: RustPackageProvenance,
    },
}

impl EnumVariantPayload {
    fn try_map_types<E>(
        &self,
        map: &mut impl FnMut(&TypeKind) -> Result<TypeKind, E>,
    ) -> Result<Self, E> {
        Ok(match self {
            Self::Unit => Self::Unit,
            Self::Tuple(items) => {
                Self::Tuple(items.iter().map(&mut *map).collect::<Result<_, _>>()?)
            }
            Self::Record(fields) => Self::Record(
                fields
                    .iter()
                    .map(|field| field.try_map_type(&mut *map))
                    .collect::<Result<Vec<_>, E>>()?
                    .into_boxed_slice(),
            ),
        })
    }
}

fn metadata_catalog_digest(
    records: &BTreeMap<AcceptedNominalId, AcceptedRustTypeMetadata>,
) -> Result<AcceptedRustTypeMetadataDigest, crate::types::GenericScopeError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.accepted-rust-metadata.v1\0");
    hash_len(&mut hasher, records.len());
    for record in records.values() {
        hasher.update(&record.item.semantic_digest());
        hasher.update(record.id.semantic_digest().as_bytes());
        hash_str(&mut hasher, record.package.as_str());
        hash_str(&mut hasher, record.package_provenance.version());
        hash_optional_str(&mut hasher, record.package_provenance.metadata_hash());
        hash_str(&mut hasher, record.rust_item.as_str());
        hash_len(&mut hasher, record.parameters.len());
        for parameter in &record.parameters {
            hash_type(&mut hasher, &TypeKind::generic_parameter(parameter.clone()))?;
        }
        hash_metadata_kind(&mut hasher, &record.kind)?;
        hasher.update(&record.data_policy.canonical_bytes());
        hash_source(&mut hasher, &record.source);
    }
    Ok(AcceptedRustTypeMetadataDigest(
        *hasher.finalize().as_bytes(),
    ))
}

fn hash_metadata_kind(
    hasher: &mut blake3::Hasher,
    kind: &AcceptedRustTypeMetadataKind,
) -> Result<(), crate::types::GenericScopeError> {
    match kind {
        AcceptedRustTypeMetadataKind::Struct { shape } => {
            hasher.update(&[0]);
            match shape {
                AcceptedRustStructShape::Unit => {
                    hasher.update(&[0]);
                }
                AcceptedRustStructShape::Tuple(items) => {
                    hasher.update(&[1]);
                    hash_len(hasher, items.len());
                    for item in items {
                        hash_type(hasher, item)?;
                    }
                }
                AcceptedRustStructShape::Record(fields) => {
                    hasher.update(&[2]);
                    hash_len(hasher, fields.len());
                    for field in fields {
                        hash_str(hasher, field.name());
                        hash_type(hasher, field.ty())?;
                        hash_field_default(hasher, field);
                    }
                }
            }
        }
        AcceptedRustTypeMetadataKind::Enum { variants } => {
            hasher.update(&[1]);
            hash_len(hasher, variants.len());
            for variant in variants {
                hash_str(hasher, variant.name());
                hash_str(hasher, variant.wire_name());
                match variant.discriminant() {
                    None => {
                        hasher.update(&[0]);
                    }
                    Some(value) => {
                        hasher.update(&[1]);
                        hasher.update(&value.to_le_bytes());
                    }
                }
                match variant.payload() {
                    EnumVariantPayload::Unit => {
                        hasher.update(&[0]);
                    }
                    EnumVariantPayload::Tuple(items) => {
                        hasher.update(&[1]);
                        hash_len(hasher, items.len());
                        for item in items {
                            hash_type(hasher, item)?;
                        }
                    }
                    EnumVariantPayload::Record(fields) => {
                        hasher.update(&[2]);
                        hash_len(hasher, fields.len());
                        for field in fields {
                            hash_str(hasher, field.name());
                            hash_type(hasher, field.ty())?;
                            hash_field_default(hasher, field);
                        }
                    }
                }
            }
        }
        AcceptedRustTypeMetadataKind::Newtype { inner } => {
            hasher.update(&[2]);
            hash_type(hasher, inner)?;
        }
    }
    Ok(())
}

fn hash_type(
    hasher: &mut blake3::Hasher,
    ty: &TypeKind,
) -> Result<(), crate::types::GenericScopeError> {
    hasher.update(ty.semantic_identity_digest()?.as_bytes());
    Ok(())
}

fn hash_field_default(hasher: &mut blake3::Hasher, field: &EnvironmentRecordField) {
    hash_str(hasher, field.wire_name());
    hasher.update(&[field.bytes_format().map_or(0, |format| format as u8 + 1)]);
    hasher.update(&[u8::from(field.skip())]);
    match field.data_default() {
        None => {
            hasher.update(&[0]);
        }
        Some(RustFieldDefault::Trait) => {
            hasher.update(&[1]);
        }
        Some(RustFieldDefault::Function(path)) => {
            hasher.update(&[2]);
            hash_str(hasher, path.as_str());
        }
    }
}

fn hash_source(hasher: &mut blake3::Hasher, source: &SourceSpan) {
    hash_str(hasher, source.source().id().as_str());
    hasher.update(source.source().revision().as_bytes());
    hasher.update(&source.source().source_len().to_le_bytes());
    hash_len(hasher, source.range().start());
    hash_len(hasher, source.range().end());
}

fn hash_optional_str(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hash_str(hasher, value);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn hash_str(hasher: &mut blake3::Hasher, value: &str) {
    hash_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn hash_len(hasher: &mut blake3::Hasher, value: usize) {
    let value = u32::try_from(value)
        .expect("accepted Rust metadata sequences fit the checked u32 contract");
    hasher.update(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use arcweft_lang_syntax::ast::module_path::ModulePathRoot;
    use arcweft_lang_syntax::ast::symbol_path::{ProjectSymbolPath, ProjectSymbolSegment};
    use arcweft_lang_syntax::types::TypePath;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange};

    use super::*;
    use crate::{
        env::nominal::AcceptedNominalOwnerId,
        types::{GenericParameterOwnerId, GenericTypeParameterId},
    };

    #[test]
    fn enum_metadata_preserves_declaration_order_for_digest_and_case_ordinals() {
        let id = accepted_id("tooling", "Rank");
        let forward = enum_metadata(&id, ["Bronze", "Custom"]);
        let reverse = enum_metadata(&id, ["Custom", "Bronze"]);
        let forward_catalog = AcceptedRustTypeMetadataCatalog::try_new([forward])
            .expect("forward enum metadata catalog");
        let reverse_catalog = AcceptedRustTypeMetadataCatalog::try_new([reverse])
            .expect("reverse enum metadata catalog");

        let AcceptedRustTypeMetadataKind::Enum { variants } = forward_catalog
            .get(&id)
            .expect("forward enum metadata")
            .kind()
        else {
            panic!("metadata remains an enum");
        };
        assert_eq!(
            variants
                .iter()
                .map(|variant| variant.name())
                .collect::<Vec<_>>(),
            ["Bronze", "Custom"]
        );
        assert_eq!(
            variants
                .iter()
                .position(|variant| variant.name() == "Custom"),
            Some(1),
            "the ordinal is the source declaration ordinal"
        );
        assert_ne!(forward_catalog.digest(), reverse_catalog.digest());
    }

    #[test]
    fn metadata_catalog_digest_is_order_independent_and_identity_complete() {
        let first = metadata("alpha", "Rank", "1.0.0", "alpha::Rank", TypeKind::I32);
        let second = metadata("beta", "Rank", "1.0.0", "beta::Rank", TypeKind::String);
        let forward = AcceptedRustTypeMetadataCatalog::try_new([first.clone(), second.clone()])
            .expect("forward metadata catalog");
        let reverse = AcceptedRustTypeMetadataCatalog::try_new([second.clone(), first.clone()])
            .expect("reverse metadata catalog");
        assert_eq!(forward.digest(), reverse.digest());

        let mut changed_version = first.clone();
        changed_version.package_provenance =
            RustPackageProvenance::try_new("alpha", "2.0.0", None).expect("package provenance");
        assert_digest_changes(&first, changed_version);

        let mut changed_rust_item = first.clone();
        changed_rust_item.rust_item =
            RustItemPath::try_new("alpha::Renamed").expect("changed Rust item path");
        assert_digest_changes(&first, changed_rust_item);

        let mut changed_shape = first.clone();
        changed_shape.kind = AcceptedRustTypeMetadataKind::Newtype {
            inner: TypeKind::Bool,
        };
        assert_digest_changes(&first, changed_shape);

        let mut changed_source = first.clone();
        changed_source.source = source("metadata://alpha/changed", "changed Rank");
        assert_digest_changes(&first, changed_source);

        assert_ne!(
            AcceptedRustTypeMetadataCatalog::try_new([first])
                .expect("alpha metadata catalog")
                .digest(),
            AcceptedRustTypeMetadataCatalog::try_new([second])
                .expect("beta metadata catalog")
                .digest(),
            "equal terminal names under distinct Rust package owners remain distinct"
        );
    }

    #[test]
    fn generic_metadata_instantiation_substitutes_without_persisting_instances() {
        let id = accepted_id("tooling", "Envelope");
        let parameter =
            GenericTypeParameterId::new(GenericParameterOwnerId::AcceptedNominal(id.clone()), 0);
        let record = AcceptedRustTypeMetadata::new(
            metadata_identity(&id, "1.0.0", "tooling::Envelope"),
            [parameter.clone()],
            AcceptedRustTypeMetadataKind::Struct {
                shape: AcceptedRustStructShape::Record(
                    [EnvironmentRecordField::new(
                        "value",
                        TypeKind::generic_parameter(parameter),
                    )]
                    .into_iter()
                    .collect(),
                ),
            },
            source("metadata://tooling/envelope", "Envelope<T>"),
        );
        let catalog =
            AcceptedRustTypeMetadataCatalog::try_new([record]).expect("generic metadata catalog");
        let item = catalog.get(&id).unwrap().item().clone();
        let before = catalog.digest();
        let instantiated = catalog
            .instantiate(&AcceptedNominalType::new(id, [TypeKind::I32]))
            .expect("generic metadata instantiation");
        assert!(matches!(
            instantiated.kind(),
            AcceptedRustTypeMetadataKind::Struct {
                shape: AcceptedRustStructShape::Record(fields)
            } if fields.as_ref() == [EnvironmentRecordField::new("value", TypeKind::I32)]
        ));
        assert_eq!(catalog.digest(), before);
        assert_eq!(instantiated.item(), &item);
    }

    #[test]
    fn member_names_are_validated_in_every_metadata_namespace() {
        let id = accepted_id("tooling", "Names");
        for (names, ordinal, problem) in [
            (vec![""], 0, RustMetadataNameProblem::Empty),
            (
                vec!["member", "member"],
                1,
                RustMetadataNameProblem::Duplicate { first: 0 },
            ),
        ] {
            let cases = [
                (
                    AcceptedRustTypeMetadataKind::Struct {
                        shape: AcceptedRustStructShape::Record(
                            names
                                .iter()
                                .map(|name| EnvironmentRecordField::new(*name, TypeKind::I32))
                                .collect(),
                        ),
                    },
                    RustMetadataNameScope::StructFields,
                ),
                (
                    AcceptedRustTypeMetadataKind::Enum {
                        variants: names
                            .iter()
                            .map(|name| {
                                AcceptedRustVariantMetadata::new(*name, EnumVariantPayload::Unit)
                            })
                            .collect(),
                    },
                    RustMetadataNameScope::Variants,
                ),
                (
                    AcceptedRustTypeMetadataKind::Enum {
                        variants: Box::new([AcceptedRustVariantMetadata::new(
                            "Case".to_owned(),
                            EnumVariantPayload::Record(
                                names
                                    .iter()
                                    .map(|name| EnvironmentRecordField::new(*name, TypeKind::I32))
                                    .collect(),
                            ),
                        )]),
                    },
                    RustMetadataNameScope::VariantFields { variant: 0 },
                ),
            ];
            for (kind, scope) in cases {
                let error =
                    AcceptedRustTypeMetadataCatalog::try_new([metadata_with_kind(&id, kind, [])])
                        .unwrap_err();
                assert!(
                    matches!(error, AcceptedRustTypeMetadataCatalogError::InvalidName { scope: actual_scope, ordinal: actual_ordinal, problem: actual_problem, .. }
                    if actual_scope == scope && actual_ordinal == ordinal && actual_problem == problem)
                );
            }
        }
    }

    #[test]
    fn generic_expansion_charges_each_copy_and_keeps_the_catalog_on_abort() {
        #[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
        #[error("fixture projection budget exhausted")]
        struct Exhausted;
        struct Budget {
            maximum: u64,
            visited: u64,
        }
        impl TypeProjectionControl for Budget {
            type Error = Exhausted;
            fn check(&mut self) -> Result<(), Exhausted> {
                Ok(())
            }
            fn visit_node(
                &mut self,
                _: crate::types::TypeProjectionNodeKind,
                _: u64,
            ) -> Result<(), Exhausted> {
                self.visited += 1;
                if self.visited > self.maximum {
                    Err(Exhausted)
                } else {
                    Ok(())
                }
            }
            fn visit_binding(&mut self) -> Result<(), Exhausted> {
                self.visit_node(crate::types::TypeProjectionNodeKind::Type, 1)
            }
        }
        let id = accepted_id("tooling", "Repeated");
        let parameter =
            GenericTypeParameterId::new(GenericParameterOwnerId::AcceptedNominal(id.clone()), 0);
        let catalog = AcceptedRustTypeMetadataCatalog::try_new([AcceptedRustTypeMetadata::new(
            metadata_identity(&id, "1.0.0", "tooling::Repeated"),
            [parameter.clone()],
            AcceptedRustTypeMetadataKind::Struct {
                shape: AcceptedRustStructShape::Tuple(
                    vec![TypeKind::generic_parameter(parameter); 3].into(),
                ),
            },
            source("metadata://tooling/repeated", "Repeated<T>"),
        )])
        .unwrap();
        let nominal =
            AcceptedNominalType::new(id, [TypeKind::Tuple(vec![TypeKind::Bool, TypeKind::I32])]);
        let digest = catalog.digest();
        let mut counted = Budget {
            maximum: u64::MAX,
            visited: 0,
        };
        let expected = catalog
            .instantiate_with_control(&nominal, &mut counted)
            .unwrap();
        let mut one_less = Budget {
            maximum: counted.visited - 1,
            visited: 0,
        };
        assert!(matches!(
            catalog.instantiate_with_control(&nominal, &mut one_less),
            Err(RustMetadataInstantiationError::Control(Exhausted))
        ));
        let mut exact = Budget {
            maximum: counted.visited,
            visited: 0,
        };
        assert_eq!(
            catalog
                .instantiate_with_control(&nominal, &mut exact)
                .unwrap(),
            expected
        );
        assert_eq!(catalog.digest(), digest);
    }

    fn assert_digest_changes(
        original: &AcceptedRustTypeMetadata,
        changed: AcceptedRustTypeMetadata,
    ) {
        let original = AcceptedRustTypeMetadataCatalog::try_new([original.clone()])
            .expect("original metadata catalog");
        let changed =
            AcceptedRustTypeMetadataCatalog::try_new([changed]).expect("changed metadata catalog");
        assert_ne!(original.digest(), changed.digest());
    }

    fn metadata(
        package: &str,
        name: &str,
        version: &str,
        rust_item: &str,
        inner: TypeKind,
    ) -> AcceptedRustTypeMetadata {
        AcceptedRustTypeMetadata::new(
            metadata_identity(&accepted_id(package, name), version, rust_item),
            [],
            AcceptedRustTypeMetadataKind::Newtype { inner },
            source(
                &format!("metadata://{package}/{name}"),
                &format!("{rust_item} {version}"),
            ),
        )
    }

    fn metadata_with_kind(
        id: &AcceptedNominalId,
        kind: AcceptedRustTypeMetadataKind,
        parameters: impl Into<Box<[GenericTypeParameterId]>>,
    ) -> AcceptedRustTypeMetadata {
        let package = match id.owner() {
            AcceptedNominalOwnerId::RustPackage(package) => package.clone(),
            _ => panic!("test metadata owner is a Rust package"),
        };
        AcceptedRustTypeMetadata::new(
            metadata_identity(
                id,
                "1.0.0",
                &format!("{}::{}", package, id.canonical_path()),
            ),
            parameters,
            kind,
            source("metadata://tooling/type", "type metadata"),
        )
    }

    fn enum_metadata(
        id: &AcceptedNominalId,
        names: impl IntoIterator<Item = &'static str>,
    ) -> AcceptedRustTypeMetadata {
        let variants = names
            .into_iter()
            .map(|name| AcceptedRustVariantMetadata::new(name, EnumVariantPayload::Unit))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        metadata_with_kind(id, AcceptedRustTypeMetadataKind::Enum { variants }, [])
    }

    fn metadata_identity(
        id: &AcceptedNominalId,
        version: &str,
        rust_item: &str,
    ) -> RustTypeMetadataPublicationIdentity {
        let AcceptedNominalOwnerId::RustPackage(package) = id.owner() else {
            panic!("test metadata owner is a Rust package")
        };
        let rust_item = RustItemPath::try_new(rust_item).expect("Rust item");
        RustTypeMetadataPublicationIdentity::new(
            EnvironmentPublicationItemId::RustType {
                adapter: crate::callable::AdapterPackageId::try_new("fixture.metadata")
                    .expect("adapter"),
                package: package.clone(),
                rust_item: rust_item.clone(),
                accepted_path: id.canonical_path().clone(),
            },
            id.clone(),
            package.clone(),
            RustPackageProvenance::try_new(package.as_str(), version, None).expect("provenance"),
            rust_item,
        )
    }

    fn accepted_id(package: &str, name: &str) -> AcceptedNominalId {
        let path = TypePath::from(
            ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [ProjectSymbolSegment::try_new(name).expect("accepted nominal segment")],
            )
            .expect("accepted nominal path"),
        );
        AcceptedNominalId::new(
            AcceptedNominalOwnerId::RustPackage(
                RustPackageId::try_new(package).expect("package owner"),
            ),
            path,
        )
    }

    fn source(id: &str, text: &str) -> SourceSpan {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("source ID"),
            SourceName::Generated,
            text,
        )
        .expect("source document");
        document
            .span(SourceRange::new(0, text.len()))
            .expect("source span")
    }
}
