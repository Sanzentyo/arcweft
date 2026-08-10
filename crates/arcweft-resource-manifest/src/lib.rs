//! Strict Sans-I/O wire boundary for package-defined resource types.
//!
//! Filesystem discovery and bundle framing deliberately live in their adapter
//! owners. This crate performs one source-backed JSON decode, typed conversion,
//! canonical encoding, and atomic immutable-registry publication.

mod budget;
mod decode;
mod diagnostic;
mod encode;
mod limits;
mod publication;
mod shape;
mod source_map;
mod strict_json;
mod wire;

pub use decode::{ResourceManifestPackageExpectation, decode_resource_type_manifest};
pub use diagnostic::{
    ResourceManifestDiagnostic, ResourceManifestDiagnosticCode, ResourceManifestRelatedSpan,
    ResourceManifestReport,
};
pub use encode::{ResourceManifestEncodeError, encode_resource_type_manifest_v1};
pub use limits::{ResourceManifestDecodeLimits, ResourceManifestPublicationLimits};
pub use publication::{PublishedResourceTypeManifestSetV1, publish_resource_type_manifests_v1};
pub use source_map::{
    JsonPath, JsonPathSegment, JsonTokenRange, ResourceCodecSource, ResourceConstSourcePath,
    ResourceFieldSource, ResourceManifestSourceMap, ResourceSchemaSource, ResourceTypeSource,
    ResourceVariantSource,
};
pub use wire::{
    PackageCoordinateFile, ResourceTypeManifestFileV1, ResourceTypeManifestFormatV1,
    ResourceTypeManifestSchemaV1, SourceBackedResourceTypeManifestV1, TypedResourceTypeManifestV1,
};
