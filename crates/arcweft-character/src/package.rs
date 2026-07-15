//! Sans I/O `.awchar` package representation.
//!
//! A package is a validated manifest plus the package-relative PNG payloads
//! referenced by every manifest variant.  Filesystem enumeration and writes stay
//! in adapter crates.

use crate::manifest::{
    CharacterAssetPath, CharacterManifest, CharacterManifestError, CharacterRuntimeDecodeError,
};
use std::collections::BTreeMap;
use thiserror::Error;

/// File name inside an `.awchar` package directory.
pub const CHARACTER_PACKAGE_MANIFEST_PATH: &str = "character.awchar.json";

/// One PNG payload inside a typed character package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterLayerPayload {
    path: CharacterAssetPath,
    bytes: Vec<u8>,
}

/// Complete Sans I/O representation of one `.awchar` package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterPackage {
    manifest: CharacterManifest,
    manifest_bytes: Vec<u8>,
    layer_payloads: BTreeMap<CharacterAssetPath, CharacterLayerPayload>,
}

/// Character package validation or codec failure.
#[derive(Debug, Error)]
pub enum CharacterPackageError {
    #[error("failed to encode character package manifest: {0}")]
    ManifestEncode(#[source] serde_json::Error),
    #[error("character package manifest is not UTF-8: {0}")]
    ManifestUtf8(#[from] std::str::Utf8Error),
    #[error("failed to decode character package manifest: {0}")]
    ManifestDecode(#[from] CharacterRuntimeDecodeError),
    #[error(transparent)]
    Manifest(#[from] CharacterManifestError),
    #[error("package contains duplicate layer payload `{0}`")]
    DuplicateLayerPayload(String),
    #[error("package is missing layer payload `{path}` for `{part}.{variant}`")]
    MissingLayerPayload {
        path: String,
        part: String,
        variant: String,
    },
    #[error("package contains unreferenced layer payload `{0}`")]
    UnreferencedLayerPayload(String),
}

impl CharacterLayerPayload {
    /// Creates one package-relative PNG payload.
    pub fn new(path: CharacterAssetPath, bytes: Vec<u8>) -> Self {
        Self { path, bytes }
    }

    /// Package-relative manifest path.
    pub const fn path(&self) -> &CharacterAssetPath {
        &self.path
    }

    /// Exact bytes published for this layer.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl CharacterPackage {
    /// Builds a package from a typed manifest and layer payloads.
    pub fn new(
        manifest: CharacterManifest,
        payloads: impl IntoIterator<Item = CharacterLayerPayload>,
    ) -> Result<Self, CharacterPackageError> {
        manifest.validate()?;
        let manifest_bytes = manifest
            .to_json_pretty()
            .map(String::into_bytes)
            .map_err(CharacterPackageError::ManifestEncode)?;
        Self::from_validated_parts(manifest, manifest_bytes, payloads)
    }

    /// Parses manifest bytes and validates that all layer payloads are present.
    pub fn from_manifest_bytes(
        manifest_bytes: Vec<u8>,
        payloads: impl IntoIterator<Item = CharacterLayerPayload>,
    ) -> Result<Self, CharacterPackageError> {
        let manifest_source = std::str::from_utf8(&manifest_bytes)?;
        let manifest = CharacterManifest::decode_runtime_json(manifest_source)?;
        Self::from_validated_parts(manifest, manifest_bytes, payloads)
    }

    fn from_validated_parts(
        manifest: CharacterManifest,
        manifest_bytes: Vec<u8>,
        payloads: impl IntoIterator<Item = CharacterLayerPayload>,
    ) -> Result<Self, CharacterPackageError> {
        let mut layer_payloads = BTreeMap::new();
        for payload in payloads {
            let path = payload.path.clone();
            if layer_payloads.insert(path.clone(), payload).is_some() {
                return Err(CharacterPackageError::DuplicateLayerPayload(
                    path.as_str().to_owned(),
                ));
            }
        }
        Self::validate_payloads(&manifest, &layer_payloads)?;
        Ok(Self {
            manifest,
            manifest_bytes,
            layer_payloads,
        })
    }

    /// Validates package payload coverage against the manifest.
    pub fn validate_payloads(
        manifest: &CharacterManifest,
        payloads: &BTreeMap<CharacterAssetPath, CharacterLayerPayload>,
    ) -> Result<(), CharacterPackageError> {
        let expected = expected_payloads(manifest);
        for (path, (part, variant)) in &expected {
            if !payloads.contains_key(*path) {
                return Err(CharacterPackageError::MissingLayerPayload {
                    path: path.as_str().to_owned(),
                    part: part.clone(),
                    variant: variant.clone(),
                });
            }
        }
        for path in payloads.keys() {
            if !expected.contains_key(path) {
                return Err(CharacterPackageError::UnreferencedLayerPayload(
                    path.as_str().to_owned(),
                ));
            }
        }
        Ok(())
    }

    /// Typed manifest.
    pub const fn manifest(&self) -> &CharacterManifest {
        &self.manifest
    }

    /// Deterministic manifest bytes to publish inside the package.
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }

    /// Ordered layer payloads keyed by package-relative path.
    pub fn layer_payloads(&self) -> impl ExactSizeIterator<Item = &CharacterLayerPayload> {
        self.layer_payloads.values()
    }

    /// Returns one layer payload by package-relative path.
    pub fn layer_payload(&self, path: &CharacterAssetPath) -> Option<&CharacterLayerPayload> {
        self.layer_payloads.get(path)
    }

    /// Consumes the package into manifest bytes and layer payloads.
    pub fn into_parts(self) -> (CharacterManifest, Vec<u8>, Vec<CharacterLayerPayload>) {
        (
            self.manifest,
            self.manifest_bytes,
            self.layer_payloads.into_values().collect(),
        )
    }
}

fn expected_payloads(
    manifest: &CharacterManifest,
) -> BTreeMap<&CharacterAssetPath, (String, String)> {
    manifest
        .parts()
        .iter()
        .flat_map(|part| {
            part.variants().iter().map(move |variant| {
                (
                    variant.asset(),
                    (part.id().to_string(), variant.id().to_string()),
                )
            })
        })
        .collect::<BTreeMap<_, _>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::CharacterAssetPath;

    fn payload(path: &str) -> CharacterLayerPayload {
        CharacterLayerPayload::new(
            CharacterAssetPath::try_new(path).expect("path"),
            vec![1, 2, 3],
        )
    }

    #[test]
    fn package_rejects_unreferenced_payloads() {
        let manifest = CharacterManifest::decode_runtime_json(include_str!(
            "../tests/fixtures/zundamon.awchar/character.awchar.json"
        ))
        .expect("manifest");
        let mut payloads = manifest
            .parts()
            .iter()
            .flat_map(|part| {
                part.variants()
                    .iter()
                    .map(|variant| payload(variant.asset().as_str()))
            })
            .collect::<Vec<_>>();
        payloads.push(payload("layers/ghost.png"));

        let error = CharacterPackage::new(manifest, payloads).expect_err("unreferenced payload");
        assert!(matches!(
            error,
            CharacterPackageError::UnreferencedLayerPayload(_)
        ));
    }
}
