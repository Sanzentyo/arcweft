//! Deterministic construction and immutable publication of resource type
//! registries.

use crate::descriptor::{
    ResourceCapabilityError, ResourceCodecSupport, ResourceDescriptorProvenance,
    ResourceTypeDescriptor, ResourceValueSchema, ResourceValueSchemaKind,
};
use crate::identity::{
    NominalTypeId, ResourceCodecId, ResourceCodecVersion, ResourceFamilyGroupId, ResourceFieldId,
    ResourceFieldName, ResourcePublicIdFamily, ResourceSchemaId, ResourceTypeId, ResourceVariantId,
    ResourceVariantName,
};
use crate::value::{
    ResourceValidationPathSegment, ResourceValueTypePath, ResourceValueValidationError,
};
use arcweft_manifest_model::SemanticDigest;
use core::fmt;
use std::collections::BTreeMap;
use thiserror::Error;

mod digest;
mod validation;

/// Sole resource descriptor publication schema accepted by this cut.
pub const RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Candidate schemas, types, and lowering codecs submitted atomically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceRegistryPublication {
    manifest_schema_version: u32,
    schemas: Vec<ResourceValueSchema>,
    resource_types: Vec<ResourceTypeDescriptor>,
    codecs: Vec<ResourceCodecSupport>,
}

/// Semantic digest of one canonical nominal resource value schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceSchemaDigest(SemanticDigest);

/// Semantic digest of one complete immutable resource type registry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceTypeRegistryDigest(SemanticDigest);

/// Immutable, canonically ordered configured-resource registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceTypeRegistry {
    manifest_schema_version: u32,
    schemas: BTreeMap<ResourceSchemaId, ResourceValueSchema>,
    schema_digests: BTreeMap<ResourceSchemaId, ResourceSchemaDigest>,
    resource_types: BTreeMap<ResourceTypeId, ResourceTypeDescriptor>,
    codecs: BTreeMap<ResourceCodecId, ResourceCodecSupport>,
    digest: ResourceTypeRegistryDigest,
}

/// All deterministic issues found before registry publication.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error(
    "resource registry publication rejected with {count} issue(s)",
    count = .issues.len()
)]
pub struct ResourceRegistryPublicationError {
    issues: Vec<ResourceRegistryIssue>,
}

/// One typed immutable-registry publication issue.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceRegistryIssue {
    #[error("resource manifest schema version {actual} is unsupported; expected {expected}")]
    UnsupportedManifestSchemaVersion { expected: u32, actual: u32 },
    #[error("resource codec `{codec}` was registered more than once")]
    DuplicateCodec { codec: ResourceCodecId },
    #[error("resource codec `{codec}` supports no schema versions")]
    CodecWithoutVersions { codec: ResourceCodecId },
    #[error("resource type `{type_id}` is duplicated by `{first:?}` and `{second:?}`")]
    DuplicateType {
        type_id: ResourceTypeId,
        first: ResourceDescriptorProvenance,
        second: ResourceDescriptorProvenance,
    },
    #[error("resource schema `{schema}` was registered more than once")]
    DuplicateSchema { schema: ResourceSchemaId },
    #[error("nominal value type `{nominal_type}` is bound to both `{first}` and `{second}`")]
    DuplicateNominalSchema {
        nominal_type: NominalTypeId,
        first: ResourceSchemaId,
        second: ResourceSchemaId,
    },
    #[error("resource schema `{schema}` duplicates field ID {field}")]
    DuplicateFieldId {
        schema: ResourceSchemaId,
        field: ResourceFieldId,
    },
    #[error("resource schema `{schema}` duplicates field name `{field}`")]
    DuplicateFieldName {
        schema: ResourceSchemaId,
        field: ResourceFieldName,
    },
    #[error("resource schema `{schema}` duplicates variant ID {variant}")]
    DuplicateVariantId {
        schema: ResourceSchemaId,
        variant: ResourceVariantId,
    },
    #[error("resource schema `{schema}` duplicates variant name `{variant}`")]
    DuplicateVariantName {
        schema: ResourceSchemaId,
        variant: ResourceVariantName,
    },
    #[error("required field {field} in resource schema `{schema}` declares a default")]
    RequiredFieldHasDefault {
        schema: ResourceSchemaId,
        field: ResourceFieldId,
    },
    #[error("default for field {field} in resource schema `{schema}` is invalid: {source}")]
    InvalidFieldDefault {
        schema: ResourceSchemaId,
        field: ResourceFieldId,
        source: ResourceDefaultValidationError,
    },
    #[error("{owner} references unknown nominal resource value schema `{target}` at {path:?}")]
    UnknownValueSchema {
        owner: ResourceSchemaId,
        target: ResourceSchemaId,
        path: ResourceValueTypePath,
    },
    #[error("{owner} expects `{target}` at {path:?} to be a {expected:?} schema, found {actual:?}")]
    ValueSchemaKindMismatch {
        owner: ResourceSchemaId,
        target: ResourceSchemaId,
        path: ResourceValueTypePath,
        expected: ResourceValueSchemaKind,
        actual: ResourceValueSchemaKind,
    },
    #[error("{owner} references unknown configured resource type `{target}` at {path:?}")]
    UnknownResourceReferenceType {
        owner: ResourceSchemaId,
        target: ResourceTypeId,
        path: ResourceValueTypePath,
    },
    #[error("resource type `{type_id}` references unknown body schema `{schema}`")]
    UnknownBodySchema {
        type_id: ResourceTypeId,
        schema: ResourceSchemaId,
    },
    #[error("resource type `{type_id}` uses non-record body schema `{schema}`")]
    BodySchemaNotRecord {
        type_id: ResourceTypeId,
        schema: ResourceSchemaId,
    },
    #[error(
        "resource type `{type_id}` does not match body schema `{schema}` nominal type `{actual}`"
    )]
    BodySchemaNominalTypeMismatch {
        type_id: ResourceTypeId,
        schema: ResourceSchemaId,
        actual: NominalTypeId,
    },
    #[error(
        "resource type `{type_id}` belongs to package `{actual}`, but provenance says `{expected}`"
    )]
    ProvenancePackageMismatch {
        type_id: ResourceTypeId,
        expected: arcweft_manifest_model::PackageId,
        actual: arcweft_manifest_model::PackageId,
    },
    #[error(
        "public-ID family `{family}` is shared by incompatible groups `{first_group}` and `{second_group}`"
    )]
    FamilyCollision {
        family: ResourcePublicIdFamily,
        first_group: ResourceFamilyGroupId,
        first_type: ResourceTypeId,
        second_group: ResourceFamilyGroupId,
        second_type: ResourceTypeId,
    },
    #[error("resource type `{type_id}` selects absent lowering codec `{codec}`")]
    MissingCodec {
        type_id: ResourceTypeId,
        codec: ResourceCodecId,
    },
    #[error("resource type `{type_id}` selects unsupported codec version {version} for `{codec}`")]
    UnsupportedCodecVersion {
        type_id: ResourceTypeId,
        codec: ResourceCodecId,
        version: ResourceCodecVersion,
    },
    #[error("resource type `{type_id}` has invalid capabilities: {source}")]
    InvalidCapabilities {
        type_id: ResourceTypeId,
        source: ResourceCapabilityError,
    },
    #[error("resource value type nesting exceeds the supported publication depth")]
    ValueTypeNestingTooDeep { owner: ResourceSchemaId },
}

/// Invalid nested structure in a typed descriptor default.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceDefaultValidationError {
    #[error(transparent)]
    Structural(#[from] ResourceValueValidationError),
    #[error("record constant contains unknown field {field}")]
    UnknownRecordField { field: ResourceFieldId },
    #[error("record constant omits required field {field}")]
    MissingRecordField { field: ResourceFieldId },
    #[error("enum constant selects unknown variant {variant}")]
    UnknownEnumVariant { variant: ResourceVariantId },
    #[error("enum variant payload presence does not match its schema")]
    EnumPayloadPresence,
    #[error("nested default is invalid at {segment:?}: {source}")]
    Nested {
        segment: ResourceValidationPathSegment,
        source: Box<Self>,
    },
    #[error("resource default nesting exceeds the supported publication depth")]
    NestingTooDeep,
}

/// Stored registry digest or schema digest no longer matches canonical data.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceRegistryIntegrityError {
    #[error("resource schema `{schema}` has no stored digest")]
    MissingSchemaDigest { schema: ResourceSchemaId },
    #[error("stored schema digest `{schema}` has no corresponding schema")]
    UnexpectedSchemaDigest { schema: ResourceSchemaId },
    #[error("resource schema `{schema}` digest does not match canonical schema data")]
    SchemaDigestMismatch { schema: ResourceSchemaId },
    #[error("resource registry digest does not match canonical registry data")]
    RegistryDigestMismatch,
}

impl ResourceRegistryPublication {
    pub fn new(
        manifest_schema_version: u32,
        schemas: impl IntoIterator<Item = ResourceValueSchema>,
        resource_types: impl IntoIterator<Item = ResourceTypeDescriptor>,
        codecs: impl IntoIterator<Item = ResourceCodecSupport>,
    ) -> Self {
        Self {
            manifest_schema_version,
            schemas: schemas.into_iter().collect(),
            resource_types: resource_types.into_iter().collect(),
            codecs: codecs.into_iter().collect(),
        }
    }

    pub const fn manifest_schema_version(&self) -> u32 {
        self.manifest_schema_version
    }

    pub fn schemas(&self) -> &[ResourceValueSchema] {
        &self.schemas
    }

    pub fn resource_types(&self) -> &[ResourceTypeDescriptor] {
        &self.resource_types
    }

    pub fn codecs(&self) -> &[ResourceCodecSupport] {
        &self.codecs
    }
}

impl ResourceSchemaDigest {
    pub const fn semantic_digest(self) -> SemanticDigest {
        self.0
    }
}

impl fmt::Display for ResourceSchemaDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl ResourceTypeRegistryDigest {
    pub const fn semantic_digest(self) -> SemanticDigest {
        self.0
    }
}

impl fmt::Display for ResourceTypeRegistryDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl ResourceTypeRegistry {
    /// Publishes the canonical empty registry used when a compilation has no
    /// configured resource types.
    ///
    /// # Panics
    ///
    /// Panics only if the current manifest schema rejects an empty canonical
    /// publication, which would be an internal contract regression.
    #[must_use]
    pub fn empty() -> Self {
        Self::publish(ResourceRegistryPublication::new(
            RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            [],
            [],
            [],
        ))
        .expect("the current empty resource registry is canonical")
    }

    /// Validates and atomically publishes one immutable candidate registry.
    pub fn publish(
        publication: ResourceRegistryPublication,
    ) -> Result<Self, ResourceRegistryPublicationError> {
        let validation::ValidatedRegistryParts {
            schemas,
            resource_types,
            codecs,
        } = validation::validate_and_normalize(publication)?;
        let schema_digests = schemas
            .iter()
            .map(|(id, schema)| (id.clone(), digest::schema_digest(schema)))
            .collect::<BTreeMap<_, _>>();
        let digest = digest::registry_digest(
            RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            &schemas,
            &resource_types,
            &codecs,
        );
        Ok(Self {
            manifest_schema_version: RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            schemas,
            schema_digests,
            resource_types,
            codecs,
            digest,
        })
    }

    pub const fn manifest_schema_version(&self) -> u32 {
        self.manifest_schema_version
    }

    pub fn len(&self) -> usize {
        self.resource_types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.resource_types.is_empty()
    }

    pub fn get(&self, type_id: &ResourceTypeId) -> Option<&ResourceTypeDescriptor> {
        self.resource_types.get(type_id)
    }

    pub fn types(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ResourceTypeId, &ResourceTypeDescriptor)> {
        self.resource_types.iter()
    }

    pub fn schema(&self, schema_id: &ResourceSchemaId) -> Option<&ResourceValueSchema> {
        self.schemas.get(schema_id)
    }

    pub fn schemas(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ResourceSchemaId, &ResourceValueSchema)> {
        self.schemas.iter()
    }

    pub fn schema_digest(&self, schema_id: &ResourceSchemaId) -> Option<ResourceSchemaDigest> {
        self.schema_digests.get(schema_id).copied()
    }

    pub fn codec(&self, codec_id: &ResourceCodecId) -> Option<&ResourceCodecSupport> {
        self.codecs.get(codec_id)
    }

    pub const fn digest(&self) -> ResourceTypeRegistryDigest {
        self.digest
    }

    pub fn verify_integrity(&self) -> Result<(), ResourceRegistryIntegrityError> {
        for (schema_id, schema) in &self.schemas {
            match self.schema_digests.get(schema_id).copied() {
                None => {
                    return Err(ResourceRegistryIntegrityError::MissingSchemaDigest {
                        schema: schema_id.clone(),
                    });
                }
                Some(stored) if stored != digest::schema_digest(schema) => {
                    return Err(ResourceRegistryIntegrityError::SchemaDigestMismatch {
                        schema: schema_id.clone(),
                    });
                }
                Some(_) => {}
            }
        }
        if let Some(schema) = self
            .schema_digests
            .keys()
            .find(|schema| !self.schemas.contains_key(*schema))
        {
            return Err(ResourceRegistryIntegrityError::UnexpectedSchemaDigest {
                schema: schema.clone(),
            });
        }
        let actual = digest::registry_digest(
            self.manifest_schema_version,
            &self.schemas,
            &self.resource_types,
            &self.codecs,
        );
        if actual != self.digest {
            return Err(ResourceRegistryIntegrityError::RegistryDigestMismatch);
        }
        Ok(())
    }
}

impl ResourceRegistryPublicationError {
    pub fn issues(&self) -> &[ResourceRegistryIssue] {
        &self.issues
    }

    pub(crate) fn new(issues: Vec<ResourceRegistryIssue>) -> Self {
        Self { issues }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION, ResourceRegistryIntegrityError,
        ResourceRegistryPublication, ResourceSchemaDigest, ResourceTypeRegistry,
        ResourceTypeRegistryDigest,
    };
    use crate::descriptor::{ResourceRecordSchema, ResourceValueSchema};
    use crate::identity::{
        NominalTypeId, ResourceModulePath, ResourceSchemaId, ResourceSchemaVersion,
        ResourceTypeName,
    };
    use arcweft_manifest_model::{PackageId, SemanticDigest};

    #[test]
    fn integrity_check_rejects_tampered_registry_digest() {
        let mut registry = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
            RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            [],
            [],
            [],
        ))
        .unwrap();
        registry.digest = ResourceTypeRegistryDigest(SemanticDigest::from_bytes([0xa5; 32]));

        assert_eq!(
            registry.verify_integrity(),
            Err(ResourceRegistryIntegrityError::RegistryDigestMismatch)
        );
    }

    #[test]
    fn integrity_check_rejects_tampered_schema_digest() {
        let schema_id = ResourceSchemaId::try_new("example.integrity").unwrap();
        let schema = ResourceValueSchema::Record(ResourceRecordSchema::new(
            schema_id.clone(),
            NominalTypeId::new(
                PackageId::new("com.example.resources").unwrap(),
                ResourceModulePath::try_new("integrity").unwrap(),
                ResourceTypeName::try_new("Integrity").unwrap(),
            ),
            ResourceSchemaVersion::try_new(1).unwrap(),
            [],
        ));
        let mut registry = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
            RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            [schema],
            [],
            [],
        ))
        .unwrap();
        registry.schema_digests.insert(
            schema_id.clone(),
            ResourceSchemaDigest(SemanticDigest::from_bytes([0x5a; 32])),
        );

        assert_eq!(
            registry.verify_integrity(),
            Err(ResourceRegistryIntegrityError::SchemaDigestMismatch { schema: schema_id })
        );
    }
}
