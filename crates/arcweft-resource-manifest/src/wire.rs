use crate::ResourceManifestSourceMap;
use arcweft_manifest_model::{PackageId, PackageVersion, RawDigest};
use arcweft_resource_model::descriptor::{
    ResourceCodecSupport, ResourceTypeDescriptor, ResourceValueSchema,
};
use arcweft_source::SourceDocument;
use std::sync::Arc;

pub const RESOURCE_TYPE_MANIFEST_FORMAT: &str = "arcweft.resource-type-manifest";
pub const RESOURCE_TYPE_MANIFEST_SCHEMA: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceTypeManifestFormatV1;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceTypeManifestSchemaV1;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageCoordinateFile {
    id: PackageId,
    version: PackageVersion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceTypeManifestFileV1 {
    format: ResourceTypeManifestFormatV1,
    schema: ResourceTypeManifestSchemaV1,
    package: PackageCoordinateFile,
    schemas: Vec<ResourceValueSchema>,
    resource_types: Vec<ResourceTypeDescriptor>,
    codecs: Vec<ResourceCodecSupport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedResourceTypeManifestV1 {
    package: PackageCoordinateFile,
    schemas: Vec<ResourceValueSchema>,
    resource_types: Vec<ResourceTypeDescriptor>,
    codecs: Vec<ResourceCodecSupport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedResourceTypeManifestV1 {
    document: Arc<SourceDocument>,
    file: ResourceTypeManifestFileV1,
    typed: TypedResourceTypeManifestV1,
    source_map: ResourceManifestSourceMap,
    canonical_bytes: Arc<[u8]>,
    canonical_digest: RawDigest,
}

impl PackageCoordinateFile {
    pub const fn new(id: PackageId, version: PackageVersion) -> Self {
        Self { id, version }
    }
    pub const fn id(&self) -> &PackageId {
        &self.id
    }
    pub const fn version(&self) -> &PackageVersion {
        &self.version
    }
}

impl ResourceTypeManifestFileV1 {
    pub(crate) fn new(
        package: PackageCoordinateFile,
        schemas: Vec<ResourceValueSchema>,
        resource_types: Vec<ResourceTypeDescriptor>,
        codecs: Vec<ResourceCodecSupport>,
    ) -> Self {
        Self {
            format: ResourceTypeManifestFormatV1,
            schema: ResourceTypeManifestSchemaV1,
            package,
            schemas,
            resource_types,
            codecs,
        }
    }
    pub const fn format(&self) -> ResourceTypeManifestFormatV1 {
        self.format
    }
    pub const fn schema(&self) -> ResourceTypeManifestSchemaV1 {
        self.schema
    }
    pub const fn package(&self) -> &PackageCoordinateFile {
        &self.package
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

impl TypedResourceTypeManifestV1 {
    pub(crate) fn from_file(file: &ResourceTypeManifestFileV1) -> Self {
        Self {
            package: file.package.clone(),
            schemas: file.schemas.clone(),
            resource_types: file.resource_types.clone(),
            codecs: file.codecs.clone(),
        }
    }
    pub const fn package(&self) -> &PackageCoordinateFile {
        &self.package
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

impl SourceBackedResourceTypeManifestV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "accepted source-backed product retains six independently validated authorities"
    )]
    pub(crate) fn new(
        document: Arc<SourceDocument>,
        file: ResourceTypeManifestFileV1,
        typed: TypedResourceTypeManifestV1,
        source_map: ResourceManifestSourceMap,
        canonical_bytes: Arc<[u8]>,
        canonical_digest: RawDigest,
    ) -> Self {
        Self {
            document,
            file,
            typed,
            source_map,
            canonical_bytes,
            canonical_digest,
        }
    }
    pub const fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }
    pub const fn file(&self) -> &ResourceTypeManifestFileV1 {
        &self.file
    }
    pub const fn typed(&self) -> &TypedResourceTypeManifestV1 {
        &self.typed
    }
    pub const fn source_map(&self) -> &ResourceManifestSourceMap {
        &self.source_map
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub const fn canonical_digest(&self) -> RawDigest {
        self.canonical_digest
    }
}
