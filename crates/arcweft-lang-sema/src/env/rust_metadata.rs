//! Source-backed Rust nominal metadata and its accepted immutable catalog.

use std::collections::BTreeMap;

use arcweft_rust_abi::ArcweftRustTypeParameterIndex;
use arcweft_source::SourceSpan;

use crate::{
    callable::{RustItemPath, RustPackageProvenance},
    registration::{EnvironmentPublicationItemId, EnvironmentTypeProjectionNode},
    types::{AcceptedNominalType, GenericTypeParameterId, TypeKind},
};

use super::{
    EnumVariantPayload, EnvironmentEnumRecordField,
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
    Record(Box<[(String, EnvironmentTypeProjectionNode)]>),
}

/// Source-backed Rust enum variant awaiting type projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustVariantMetadataInput {
    name: String,
    payload: RustVariantPayloadInput,
    source: SourceSpan,
}

/// Source-backed Rust enum payload awaiting type projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustVariantPayloadInput {
    Unit,
    Tuple(Box<[EnvironmentTypeProjectionNode]>),
    Record(Box<[(String, EnvironmentTypeProjectionNode)]>),
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
    id: AcceptedNominalId,
    package: RustPackageId,
    package_provenance: RustPackageProvenance,
    rust_item: RustItemPath,
    parameters: Box<[GenericTypeParameterId]>,
    kind: AcceptedRustTypeMetadataKind,
    source: SourceSpan,
}

/// One accepted Rust nominal shape after substituting an exact instantiation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstantiatedRustTypeMetadata {
    id: AcceptedNominalId,
    package: RustPackageId,
    package_provenance: RustPackageProvenance,
    rust_item: RustItemPath,
    kind: AcceptedRustTypeMetadataKind,
    source: SourceSpan,
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
        variants: Box<[(String, EnumVariantPayload)]>,
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
    Record(Box<[(String, TypeKind)]>),
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
        }
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
        Self {
            item: identity.item,
            id: identity.id,
            package: identity.package,
            package_provenance: identity.package_provenance,
            rust_item: identity.rust_item,
            parameters: parameters.into(),
            kind,
            source,
        }
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
        let digest = metadata_catalog_digest(&by_id);
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
            .cloned()
            .zip(nominal.arguments().iter().cloned())
            .collect::<BTreeMap<_, _>>();
        Ok(InstantiatedRustTypeMetadata {
            id: metadata.id.clone(),
            package: metadata.package.clone(),
            package_provenance: metadata.package_provenance.clone(),
            rust_item: metadata.rust_item.clone(),
            kind: metadata.kind.substitute(&substitutions),
            source: metadata.source.clone(),
        })
    }
}

impl AcceptedRustTypeMetadata {
    pub(crate) fn new(
        id: AcceptedNominalId,
        package: RustPackageId,
        package_provenance: RustPackageProvenance,
        rust_item: RustItemPath,
        parameters: impl Into<Box<[GenericTypeParameterId]>>,
        kind: AcceptedRustTypeMetadataKind,
        source: SourceSpan,
    ) -> Self {
        Self {
            id,
            package,
            package_provenance,
            rust_item,
            parameters: parameters.into(),
            kind,
            source,
        }
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
    fn substitute(&self, substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>) -> Self {
        match self {
            Self::Struct { shape } => Self::Struct {
                shape: shape.substitute(substitutions),
            },
            Self::Enum { variants } => Self::Enum {
                variants: variants
                    .iter()
                    .map(|(name, payload)| {
                        (name.clone(), substitute_variant(payload, substitutions))
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
            Self::Newtype { inner } => Self::Newtype {
                inner: inner.substitute_type_parameters(substitutions),
            },
        }
    }
}

impl AcceptedRustStructShape {
    fn substitute(&self, substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>) -> Self {
        match self {
            Self::Unit => Self::Unit,
            Self::Tuple(items) => Self::Tuple(
                items
                    .iter()
                    .map(|item| item.substitute_type_parameters(substitutions))
                    .collect(),
            ),
            Self::Record(fields) => Self::Record(
                fields
                    .iter()
                    .map(|(name, ty)| (name.clone(), ty.substitute_type_parameters(substitutions)))
                    .collect(),
            ),
        }
    }
}

impl AcceptedRustTypeMetadataDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Failure to instantiate metadata for one exact accepted Rust nominal.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum RustMetadataInstantiationError {
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
    #[error("accepted Rust metadata contains duplicate nominal `{id:?}`")]
    DuplicateNominal { id: AcceptedNominalId },
    #[error("accepted Rust metadata `{id:?}` contains duplicate variant `{variant}`")]
    DuplicateVariant {
        id: AcceptedNominalId,
        variant: String,
    },
    #[error(
        "accepted Rust metadata `{id:?}` variant `{variant}` contains duplicate record field `{field}`"
    )]
    DuplicateVariantRecordField {
        id: AcceptedNominalId,
        variant: String,
        field: String,
    },
    #[error("Rust package `{package}` has conflicting version or metadata-hash claims")]
    PackageProvenanceConflict {
        package: RustPackageId,
        first: RustPackageProvenance,
        conflicting: RustPackageProvenance,
    },
}

fn substitute_variant(
    payload: &EnumVariantPayload,
    substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
) -> EnumVariantPayload {
    match payload {
        EnumVariantPayload::Unit => EnumVariantPayload::Unit,
        EnumVariantPayload::Tuple(items) => EnumVariantPayload::Tuple(
            items
                .iter()
                .map(|item| item.substitute_type_parameters(substitutions))
                .collect(),
        ),
        EnumVariantPayload::Record(fields) => EnumVariantPayload::Record(
            fields
                .iter()
                .map(|field| {
                    EnvironmentEnumRecordField::new(
                        field.name(),
                        field.ty().substitute_type_parameters(substitutions),
                    )
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
    }
}

fn metadata_catalog_digest(
    records: &BTreeMap<AcceptedNominalId, AcceptedRustTypeMetadata>,
) -> AcceptedRustTypeMetadataDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.accepted-rust-metadata.v1\0");
    hash_len(&mut hasher, records.len());
    for record in records.values() {
        hasher.update(
            crate::types::accepted_nominal_semantic_identity_digest(&record.id, &[]).as_bytes(),
        );
        hash_str(&mut hasher, record.package.as_str());
        hash_str(&mut hasher, record.package_provenance.version());
        hash_optional_str(&mut hasher, record.package_provenance.metadata_hash());
        hash_str(&mut hasher, record.rust_item.as_str());
        hash_len(&mut hasher, record.parameters.len());
        for parameter in &record.parameters {
            hash_type(&mut hasher, &TypeKind::GenericParam(parameter.clone()));
        }
        hash_metadata_kind(&mut hasher, &record.kind);
        hash_source(&mut hasher, &record.source);
    }
    AcceptedRustTypeMetadataDigest(*hasher.finalize().as_bytes())
}

fn hash_metadata_kind(hasher: &mut blake3::Hasher, kind: &AcceptedRustTypeMetadataKind) {
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
                        hash_type(hasher, item);
                    }
                }
                AcceptedRustStructShape::Record(fields) => {
                    hasher.update(&[2]);
                    hash_len(hasher, fields.len());
                    for (name, ty) in fields {
                        hash_str(hasher, name);
                        hash_type(hasher, ty);
                    }
                }
            }
        }
        AcceptedRustTypeMetadataKind::Enum { variants } => {
            hasher.update(&[1]);
            hash_len(hasher, variants.len());
            for (name, payload) in variants {
                hash_str(hasher, name);
                match payload {
                    EnumVariantPayload::Unit => {
                        hasher.update(&[0]);
                    }
                    EnumVariantPayload::Tuple(items) => {
                        hasher.update(&[1]);
                        hash_len(hasher, items.len());
                        for item in items {
                            hash_type(hasher, item);
                        }
                    }
                    EnumVariantPayload::Record(fields) => {
                        hasher.update(&[2]);
                        hash_len(hasher, fields.len());
                        for field in fields {
                            hash_str(hasher, field.name());
                            hash_type(hasher, field.ty());
                        }
                    }
                }
            }
        }
        AcceptedRustTypeMetadataKind::Newtype { inner } => {
            hasher.update(&[2]);
            hash_type(hasher, inner);
        }
    }
}

fn hash_type(hasher: &mut blake3::Hasher, ty: &TypeKind) {
    hasher.update(ty.semantic_identity_digest().as_bytes());
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
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["Bronze", "Custom"]
        );
        assert_eq!(
            variants.iter().position(|(name, _)| name == "Custom"),
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
            id.clone(),
            RustPackageId::try_new("tooling").expect("package"),
            RustPackageProvenance::try_new("tooling", "1.0.0", None).expect("provenance"),
            RustItemPath::try_new("tooling::Envelope").expect("Rust item"),
            [parameter.clone()],
            AcceptedRustTypeMetadataKind::Struct {
                shape: AcceptedRustStructShape::Record(
                    [("value".to_owned(), TypeKind::GenericParam(parameter))]
                        .into_iter()
                        .collect(),
                ),
            },
            source("metadata://tooling/envelope", "Envelope<T>"),
        );
        let catalog =
            AcceptedRustTypeMetadataCatalog::try_new([record]).expect("generic metadata catalog");
        let before = catalog.digest();
        let instantiated = catalog
            .instantiate(&AcceptedNominalType::new(id, [TypeKind::I32]))
            .expect("generic metadata instantiation");
        assert!(matches!(
            instantiated.kind(),
            AcceptedRustTypeMetadataKind::Struct {
                shape: AcceptedRustStructShape::Record(fields)
            } if fields.as_ref() == [("value".to_owned(), TypeKind::I32)]
        ));
        assert_eq!(catalog.digest(), before);
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
            accepted_id(package, name),
            RustPackageId::try_new(package).expect("package"),
            RustPackageProvenance::try_new(package, version, None).expect("provenance"),
            RustItemPath::try_new(rust_item).expect("Rust item"),
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
            id.clone(),
            package.clone(),
            RustPackageProvenance::try_new(package.as_str(), "1.0.0", None).expect("provenance"),
            RustItemPath::try_new(format!("{}::{}", package, id.canonical_path()))
                .expect("Rust item"),
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
            .map(|name| (name.to_owned(), EnumVariantPayload::Unit))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        metadata_with_kind(id, AcceptedRustTypeMetadataKind::Enum { variants }, [])
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
