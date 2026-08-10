use crate::identity::{
    NominalTypeId, ResourceBundleSectionId, ResourceBundleSectionVersion, ResourceCodecId,
    ResourceCodecVersion, ResourceDescriptorSourceId, ResourceFamilyGroupId, ResourceFieldId,
    ResourceFieldName, ResourcePublicIdFamily, ResourceRuntimeHandleKindId, ResourceSchemaId,
    ResourceSchemaVersion, ResourceTypeId, ResourceVariantId, ResourceVariantName,
};
use crate::value::{ResourceConstValue, ResourceValueType};
use arcweft_manifest_model::PackageId;
use std::collections::BTreeSet;
use thiserror::Error;

/// Whether a resource field must be authored/materialized or may be absent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceFieldPresence {
    Required,
    Optional,
}

/// One stable field contract in a nominal record schema.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceFieldDescriptor {
    id: ResourceFieldId,
    name: ResourceFieldName,
    value_type: ResourceValueType,
    presence: ResourceFieldPresence,
    default: Option<ResourceConstValue>,
    docs: String,
}

/// One stable variant contract in a nominal enum schema.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceVariantDescriptor {
    id: ResourceVariantId,
    name: ResourceVariantName,
    payload: Option<ResourceValueType>,
    docs: String,
}

/// Nominal record schema used by a resource body or nested value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceRecordSchema {
    id: ResourceSchemaId,
    nominal_type: NominalTypeId,
    version: ResourceSchemaVersion,
    fields: Vec<ResourceFieldDescriptor>,
}

/// Nominal enum schema used by resource field values.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceEnumSchema {
    id: ResourceSchemaId,
    nominal_type: NominalTypeId,
    version: ResourceSchemaVersion,
    variants: Vec<ResourceVariantDescriptor>,
}

/// Closed nominal schema inventory published with the resource registry.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceValueSchema {
    Record(ResourceRecordSchema),
    Enum(ResourceEnumSchema),
}

/// Kind of one published nominal value schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceValueSchemaKind {
    Record,
    Enum,
}

/// Agent-visible catalog and live-state exposure policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceAgentExposure {
    Hidden,
    Catalog,
    CatalogAndRuntime,
}

/// Generic hot-reload behavior declared by a resource type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceHotReloadClass {
    RestartRequired,
    ReplaceDefinition,
    UpdateLiveHandle,
}

/// Generic runtime, Agent, save, and hot-reload capabilities.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceCapabilities {
    runtime_handle_kind: Option<ResourceRuntimeHandleKindId>,
    agent_exposure: ResourceAgentExposure,
    save_definition_reference: bool,
    hot_reload: ResourceHotReloadClass,
}

/// Stable compiler-to-product lowering selection.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceLoweringBinding {
    codec_id: ResourceCodecId,
    codec_version: ResourceCodecVersion,
    section_id: ResourceBundleSectionId,
    section_version: ResourceBundleSectionVersion,
}

/// Stable codec registration accepted by a registry publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceCodecSupport {
    codec_id: ResourceCodecId,
    versions: BTreeSet<ResourceCodecVersion>,
}

/// Human-facing descriptor documentation excluded from semantic digests.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceTypeDocs {
    summary: String,
}

/// Neutral provenance retained for deterministic duplicate evidence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceDescriptorProvenance {
    package: PackageId,
    source: ResourceDescriptorSourceId,
}

/// One extension-neutral configured resource type descriptor.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceTypeDescriptor {
    provenance: ResourceDescriptorProvenance,
    type_id: ResourceTypeId,
    public_id_family: ResourcePublicIdFamily,
    family_group: ResourceFamilyGroupId,
    body_schema: ResourceSchemaId,
    capabilities: ResourceCapabilities,
    lowering: ResourceLoweringBinding,
    docs: ResourceTypeDocs,
}

/// Invalid capability combination rejected during registry publication.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceCapabilityError {
    #[error("Agent runtime exposure requires a runtime handle kind")]
    AgentRuntimeWithoutHandle,
    #[error("live-handle hot reload requires a runtime handle kind")]
    LiveHotReloadWithoutHandle,
}

impl ResourceFieldDescriptor {
    pub fn required(
        id: ResourceFieldId,
        name: ResourceFieldName,
        value_type: ResourceValueType,
    ) -> Self {
        Self {
            id,
            name,
            value_type,
            presence: ResourceFieldPresence::Required,
            default: None,
            docs: String::new(),
        }
    }

    pub fn optional(
        id: ResourceFieldId,
        name: ResourceFieldName,
        value_type: ResourceValueType,
    ) -> Self {
        Self {
            id,
            name,
            value_type,
            presence: ResourceFieldPresence::Optional,
            default: None,
            docs: String::new(),
        }
    }

    #[must_use]
    pub fn with_default(mut self, default: ResourceConstValue) -> Self {
        self.default = Some(default);
        self
    }

    #[must_use]
    pub fn with_docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = docs.into();
        self
    }

    pub const fn id(&self) -> ResourceFieldId {
        self.id
    }

    pub const fn name(&self) -> &ResourceFieldName {
        &self.name
    }

    pub const fn value_type(&self) -> &ResourceValueType {
        &self.value_type
    }

    pub const fn presence(&self) -> ResourceFieldPresence {
        self.presence
    }

    pub const fn default(&self) -> Option<&ResourceConstValue> {
        self.default.as_ref()
    }

    pub fn docs(&self) -> &str {
        &self.docs
    }
}

impl ResourceVariantDescriptor {
    pub fn unit(id: ResourceVariantId, name: ResourceVariantName) -> Self {
        Self {
            id,
            name,
            payload: None,
            docs: String::new(),
        }
    }

    pub fn with_payload(
        id: ResourceVariantId,
        name: ResourceVariantName,
        payload: ResourceValueType,
    ) -> Self {
        Self {
            id,
            name,
            payload: Some(payload),
            docs: String::new(),
        }
    }

    #[must_use]
    pub fn with_docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = docs.into();
        self
    }

    pub const fn id(&self) -> ResourceVariantId {
        self.id
    }

    pub const fn name(&self) -> &ResourceVariantName {
        &self.name
    }

    pub const fn payload(&self) -> Option<&ResourceValueType> {
        self.payload.as_ref()
    }

    pub fn docs(&self) -> &str {
        &self.docs
    }
}

impl ResourceRecordSchema {
    pub fn new(
        id: ResourceSchemaId,
        nominal_type: NominalTypeId,
        version: ResourceSchemaVersion,
        fields: impl IntoIterator<Item = ResourceFieldDescriptor>,
    ) -> Self {
        Self {
            id,
            nominal_type,
            version,
            fields: fields.into_iter().collect(),
        }
    }

    pub const fn id(&self) -> &ResourceSchemaId {
        &self.id
    }

    pub const fn nominal_type(&self) -> &NominalTypeId {
        &self.nominal_type
    }

    pub const fn version(&self) -> ResourceSchemaVersion {
        self.version
    }

    pub fn fields(&self) -> &[ResourceFieldDescriptor] {
        &self.fields
    }

    pub(crate) fn sort_fields(&mut self) {
        self.fields.sort_by(|left, right| {
            left.id()
                .cmp(&right.id())
                .then_with(|| left.name().cmp(right.name()))
        });
    }
}

impl ResourceEnumSchema {
    pub fn new(
        id: ResourceSchemaId,
        nominal_type: NominalTypeId,
        version: ResourceSchemaVersion,
        variants: impl IntoIterator<Item = ResourceVariantDescriptor>,
    ) -> Self {
        Self {
            id,
            nominal_type,
            version,
            variants: variants.into_iter().collect(),
        }
    }

    pub const fn id(&self) -> &ResourceSchemaId {
        &self.id
    }

    pub const fn nominal_type(&self) -> &NominalTypeId {
        &self.nominal_type
    }

    pub const fn version(&self) -> ResourceSchemaVersion {
        self.version
    }

    pub fn variants(&self) -> &[ResourceVariantDescriptor] {
        &self.variants
    }

    pub(crate) fn sort_variants(&mut self) {
        self.variants.sort_by(|left, right| {
            left.id()
                .cmp(&right.id())
                .then_with(|| left.name().cmp(right.name()))
        });
    }
}

impl ResourceValueSchema {
    pub const fn id(&self) -> &ResourceSchemaId {
        match self {
            Self::Record(schema) => schema.id(),
            Self::Enum(schema) => schema.id(),
        }
    }

    pub const fn nominal_type(&self) -> &NominalTypeId {
        match self {
            Self::Record(schema) => schema.nominal_type(),
            Self::Enum(schema) => schema.nominal_type(),
        }
    }

    pub const fn version(&self) -> ResourceSchemaVersion {
        match self {
            Self::Record(schema) => schema.version(),
            Self::Enum(schema) => schema.version(),
        }
    }

    pub const fn kind(&self) -> ResourceValueSchemaKind {
        match self {
            Self::Record(_) => ResourceValueSchemaKind::Record,
            Self::Enum(_) => ResourceValueSchemaKind::Enum,
        }
    }

    pub(crate) fn canonicalize(&mut self) {
        match self {
            Self::Record(schema) => schema.sort_fields(),
            Self::Enum(schema) => schema.sort_variants(),
        }
    }
}

impl ResourceCapabilities {
    pub const fn new(
        runtime_handle_kind: Option<ResourceRuntimeHandleKindId>,
        agent_exposure: ResourceAgentExposure,
        save_definition_reference: bool,
        hot_reload: ResourceHotReloadClass,
    ) -> Self {
        Self {
            runtime_handle_kind,
            agent_exposure,
            save_definition_reference,
            hot_reload,
        }
    }

    pub const fn definition_only() -> Self {
        Self::new(
            None,
            ResourceAgentExposure::Catalog,
            true,
            ResourceHotReloadClass::ReplaceDefinition,
        )
    }

    pub const fn runtime_handle_kind(&self) -> Option<&ResourceRuntimeHandleKindId> {
        self.runtime_handle_kind.as_ref()
    }

    pub const fn agent_exposure(&self) -> ResourceAgentExposure {
        self.agent_exposure
    }

    pub const fn saves_definition_reference(&self) -> bool {
        self.save_definition_reference
    }

    pub const fn hot_reload(&self) -> ResourceHotReloadClass {
        self.hot_reload
    }

    pub fn validate(&self) -> Result<(), ResourceCapabilityError> {
        if self.agent_exposure == ResourceAgentExposure::CatalogAndRuntime
            && self.runtime_handle_kind.is_none()
        {
            return Err(ResourceCapabilityError::AgentRuntimeWithoutHandle);
        }
        if self.hot_reload == ResourceHotReloadClass::UpdateLiveHandle
            && self.runtime_handle_kind.is_none()
        {
            return Err(ResourceCapabilityError::LiveHotReloadWithoutHandle);
        }
        Ok(())
    }
}

impl ResourceLoweringBinding {
    pub const fn new(
        codec_id: ResourceCodecId,
        codec_version: ResourceCodecVersion,
        section_id: ResourceBundleSectionId,
        section_version: ResourceBundleSectionVersion,
    ) -> Self {
        Self {
            codec_id,
            codec_version,
            section_id,
            section_version,
        }
    }

    pub const fn codec_id(&self) -> &ResourceCodecId {
        &self.codec_id
    }

    pub const fn codec_version(&self) -> ResourceCodecVersion {
        self.codec_version
    }

    pub const fn section_id(&self) -> &ResourceBundleSectionId {
        &self.section_id
    }

    pub const fn section_version(&self) -> ResourceBundleSectionVersion {
        self.section_version
    }
}

impl ResourceCodecSupport {
    pub fn new(
        codec_id: ResourceCodecId,
        versions: impl IntoIterator<Item = ResourceCodecVersion>,
    ) -> Self {
        Self {
            codec_id,
            versions: versions.into_iter().collect(),
        }
    }

    pub const fn codec_id(&self) -> &ResourceCodecId {
        &self.codec_id
    }

    pub const fn versions(&self) -> &BTreeSet<ResourceCodecVersion> {
        &self.versions
    }

    pub fn supports(&self, version: ResourceCodecVersion) -> bool {
        self.versions.contains(&version)
    }
}

impl ResourceTypeDocs {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
        }
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }
}

impl ResourceDescriptorProvenance {
    pub const fn new(package: PackageId, source: ResourceDescriptorSourceId) -> Self {
        Self { package, source }
    }

    pub const fn package(&self) -> &PackageId {
        &self.package
    }

    pub const fn source(&self) -> &ResourceDescriptorSourceId {
        &self.source
    }
}

impl ResourceTypeDescriptor {
    #[allow(
        clippy::too_many_arguments,
        reason = "a resource descriptor is a closed publication record whose independent identities must remain explicit"
    )]
    pub const fn new(
        provenance: ResourceDescriptorProvenance,
        type_id: ResourceTypeId,
        public_id_family: ResourcePublicIdFamily,
        family_group: ResourceFamilyGroupId,
        body_schema: ResourceSchemaId,
        capabilities: ResourceCapabilities,
        lowering: ResourceLoweringBinding,
        docs: ResourceTypeDocs,
    ) -> Self {
        Self {
            provenance,
            type_id,
            public_id_family,
            family_group,
            body_schema,
            capabilities,
            lowering,
            docs,
        }
    }

    pub const fn provenance(&self) -> &ResourceDescriptorProvenance {
        &self.provenance
    }

    pub const fn type_id(&self) -> &ResourceTypeId {
        &self.type_id
    }

    pub const fn public_id_family(&self) -> &ResourcePublicIdFamily {
        &self.public_id_family
    }

    pub const fn family_group(&self) -> &ResourceFamilyGroupId {
        &self.family_group
    }

    pub const fn body_schema(&self) -> &ResourceSchemaId {
        &self.body_schema
    }

    pub const fn capabilities(&self) -> &ResourceCapabilities {
        &self.capabilities
    }

    pub const fn lowering(&self) -> &ResourceLoweringBinding {
        &self.lowering
    }

    pub const fn docs(&self) -> &ResourceTypeDocs {
        &self.docs
    }

    /// Returns the canonical semantic digest claimed by a resource manifest.
    ///
    /// Documentation and provenance are intentionally excluded, matching the
    /// complete registry digest contract.
    pub fn semantic_digest(&self) -> crate::registry::ResourceTypeDescriptorDigest {
        crate::registry::descriptor_digest(self)
    }

    /// Exact byte length of the canonical transcript consumed by
    /// [`Self::semantic_digest`].
    ///
    /// Format boundaries use this owner-provided measurement to precharge
    /// deterministic work without reimplementing the private transcript.
    pub fn semantic_digest_transcript_len(&self) -> usize {
        crate::registry::descriptor_digest_transcript_len(self)
    }
}
