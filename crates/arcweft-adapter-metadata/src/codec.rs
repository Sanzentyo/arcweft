use crate::{
    AdapterMetadata,
    strict_json::{
        AdapterMetadataDecodeLimits, AdapterMetadataSourceMap, StrictJsonError, parse_strict_json,
    },
};
use arcweft_manifest_model::{CanonicalJsonError, SemanticDigest, canonical_json_bytes};
use serde::Serialize;
use std::collections::BTreeSet;
use thiserror::Error;

const PAYLOAD_CONTEXT: &str = "arcweft-adapter-metadata-payload-v1";
const ABI_CONTEXT: &str = "arcweft-adapter-abi-v1";

/// Accepted typed metadata plus ranges from the sole strict JSON parse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedAdapterMetadata {
    metadata: AdapterMetadata,
    source_map: AdapterMetadataSourceMap,
}

/// Generated metadata decode or identity failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterMetadataCodecError {
    #[error(transparent)]
    Json(#[from] StrictJsonError),
    #[error("generated metadata does not match the final typed schema: {message}")]
    Typed { message: String },
    #[error("duplicate generated export identity `{identity}`")]
    DuplicateExport { identity: String },
    #[error("duplicate generated requirement identity `{identity}`")]
    DuplicateRequirement { identity: String },
    #[error("stored adapter ABI hash does not match the canonical typed ABI projection")]
    AbiHashMismatch,
    #[error("stored adapter payload hash does not match the canonical typed payload projection")]
    PayloadHashMismatch,
    #[error(transparent)]
    Canonical(#[from] CanonicalJsonError),
}

impl SourceBackedAdapterMetadata {
    /// Decodes and verifies one complete UTF-8 metadata document without I/O.
    pub fn decode(source: &str) -> Result<Self, AdapterMetadataCodecError> {
        Self::decode_with_limits(source, AdapterMetadataDecodeLimits::PRODUCTION)
    }

    /// Decodes with explicit deterministic resource limits.
    pub fn decode_with_limits(
        source: &str,
        limits: AdapterMetadataDecodeLimits,
    ) -> Result<Self, AdapterMetadataCodecError> {
        let (value, source_map) = parse_strict_json(source, limits)?;
        let metadata = serde_json::from_value::<AdapterMetadata>(value).map_err(|error| {
            AdapterMetadataCodecError::Typed {
                message: error.to_string(),
            }
        })?;
        metadata.validate_collections()?;
        if metadata.computed_abi_hash()? != metadata.abi_hash {
            return Err(AdapterMetadataCodecError::AbiHashMismatch);
        }
        if metadata.computed_payload_hash()? != metadata.payload_hash {
            return Err(AdapterMetadataCodecError::PayloadHashMismatch);
        }
        Ok(Self {
            metadata,
            source_map,
        })
    }

    pub const fn metadata(&self) -> &AdapterMetadata {
        &self.metadata
    }

    pub const fn source_map(&self) -> &AdapterMetadataSourceMap {
        &self.source_map
    }
}

impl AdapterMetadata {
    /// Canonical ABI bytes excluding artifact and generator provenance.
    pub fn canonical_abi_bytes(&self) -> Result<Vec<u8>, CanonicalJsonError> {
        let normalized = NormalizedMetadata::new(self)?;
        canonical_json_bytes(&AbiProjection {
            target: &self.target,
            package: &self.package,
            module: &self.module,
            requirements: &normalized.requirements,
            exports: &normalized.exports,
        })
    }

    /// Canonical payload bytes covering every field except `payload_hash`.
    pub fn canonical_payload_bytes(&self) -> Result<Vec<u8>, CanonicalJsonError> {
        let normalized = NormalizedMetadata::new(self)?;
        canonical_json_bytes(&PayloadProjection {
            format: &self.format,
            schema: &self.schema,
            generator: &self.generator,
            target: &self.target,
            package: &self.package,
            module: &self.module,
            artifact: &self.artifact,
            requirements: &normalized.requirements,
            exports: &normalized.exports,
            abi_hash: &self.abi_hash,
        })
    }

    pub fn computed_abi_hash(&self) -> Result<SemanticDigest, CanonicalJsonError> {
        Ok(SemanticDigest::derive(
            ABI_CONTEXT,
            &self.canonical_abi_bytes()?,
        ))
    }

    pub fn computed_payload_hash(&self) -> Result<SemanticDigest, CanonicalJsonError> {
        Ok(SemanticDigest::derive(
            PAYLOAD_CONTEXT,
            &self.canonical_payload_bytes()?,
        ))
    }

    /// Emits the complete envelope using canonical JSON key ordering and no whitespace.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, CanonicalJsonError> {
        let normalized = NormalizedMetadata::new(self)?;
        canonical_json_bytes(&CompleteProjection {
            format: &self.format,
            schema: &self.schema,
            generator: &self.generator,
            target: &self.target,
            package: &self.package,
            module: &self.module,
            artifact: &self.artifact,
            requirements: &normalized.requirements,
            exports: &normalized.exports,
            abi_hash: &self.abi_hash,
            payload_hash: &self.payload_hash,
        })
    }

    fn validate_collections(&self) -> Result<(), AdapterMetadataCodecError> {
        let mut requirements = BTreeSet::new();
        for requirement in &self.requirements {
            let identity = requirement_identity(requirement);
            if !requirements.insert(identity.clone()) {
                return Err(AdapterMetadataCodecError::DuplicateRequirement { identity });
            }
        }

        let mut exports = BTreeSet::new();
        for identity in self
            .exports
            .types
            .iter()
            .map(|export| export.name.as_str())
            .chain(
                self.exports
                    .functions
                    .iter()
                    .map(|export| export.name.as_str()),
            )
            .chain(
                self.exports
                    .activities
                    .iter()
                    .map(|export| export.export.as_str()),
            )
        {
            let normalized = identity.to_ascii_lowercase();
            if !exports.insert(normalized) {
                return Err(AdapterMetadataCodecError::DuplicateExport {
                    identity: identity.to_owned(),
                });
            }
        }
        Ok(())
    }
}

fn requirement_identity(requirement: &crate::AdapterRequirement) -> String {
    match requirement {
        crate::AdapterRequirement::Capability { id, .. } => format!("capability:{id}"),
        crate::AdapterRequirement::Module {
            package,
            version,
            module,
            ..
        } => format!("module:{package}@{version}:{module}"),
    }
}

struct NormalizedMetadata {
    requirements: Vec<crate::AdapterRequirement>,
    exports: crate::AdapterExports,
}

impl NormalizedMetadata {
    fn new(metadata: &AdapterMetadata) -> Result<Self, CanonicalJsonError> {
        let mut keyed_requirements = metadata
            .requirements
            .iter()
            .map(|requirement| canonical_json_bytes(&requirement).map(|key| (key, requirement)))
            .collect::<Result<Vec<_>, _>>()?;
        keyed_requirements.sort_by(|left, right| left.0.cmp(&right.0));
        let requirements = keyed_requirements
            .into_iter()
            .map(|(_, requirement)| requirement.clone())
            .collect();

        let mut exports = metadata.exports.clone();
        exports.types.sort_by(|left, right| {
            left.name
                .as_str()
                .as_bytes()
                .cmp(right.name.as_str().as_bytes())
        });
        exports.functions.sort_by(|left, right| {
            left.name
                .as_str()
                .as_bytes()
                .cmp(right.name.as_str().as_bytes())
        });
        exports.activities.sort_by(|left, right| {
            left.export
                .as_str()
                .as_bytes()
                .cmp(right.export.as_str().as_bytes())
        });
        for function in &mut exports.functions {
            function.effects.sort();
            function.effects.dedup();
        }
        Ok(Self {
            requirements,
            exports,
        })
    }
}

#[derive(Serialize)]
struct AbiProjection<'a> {
    target: &'a crate::AdapterTarget,
    package: &'a crate::AdapterPackage,
    module: &'a crate::AdapterModule,
    requirements: &'a [crate::AdapterRequirement],
    exports: &'a crate::AdapterExports,
}

#[derive(Serialize)]
struct PayloadProjection<'a> {
    format: &'a crate::AdapterMetadataFormat,
    schema: &'a crate::AdapterMetadataSchema,
    generator: &'a crate::GeneratorProvenance,
    target: &'a crate::AdapterTarget,
    package: &'a crate::AdapterPackage,
    module: &'a crate::AdapterModule,
    artifact: &'a crate::AdapterArtifact,
    requirements: &'a [crate::AdapterRequirement],
    exports: &'a crate::AdapterExports,
    abi_hash: &'a SemanticDigest,
}

#[derive(Serialize)]
struct CompleteProjection<'a> {
    format: &'a crate::AdapterMetadataFormat,
    schema: &'a crate::AdapterMetadataSchema,
    generator: &'a crate::GeneratorProvenance,
    target: &'a crate::AdapterTarget,
    package: &'a crate::AdapterPackage,
    module: &'a crate::AdapterModule,
    artifact: &'a crate::AdapterArtifact,
    requirements: &'a [crate::AdapterRequirement],
    exports: &'a crate::AdapterExports,
    abi_hash: &'a SemanticDigest,
    payload_hash: &'a SemanticDigest,
}
