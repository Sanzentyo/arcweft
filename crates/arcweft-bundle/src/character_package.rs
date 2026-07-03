//! Bundle representation for typed `.awchar` character packages.

use crate::{BundleCodecError, BundleVirtualFile, BundleVirtualFileRef, BundleVirtualFileSpace};
use arcweft_character::{
    manifest::{CharacterAssetPath, CharacterBlendMode, CharacterRect, CharacterSourceLayer},
    package::{CHARACTER_PACKAGE_MANIFEST_PATH, CharacterPackage},
};
use serde::{Deserialize, Serialize};

/// One `.awchar` package carried by an Arcweft product bundle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleCharacterPackage {
    pub character: String,
    pub manifest: BundleVirtualFileRef,
    pub layers: Vec<BundleCharacterLayerResource>,
}

/// One layer payload and metadata inside a bundled character package.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleCharacterLayerResource {
    pub part: String,
    pub variant: String,
    pub asset_path: CharacterAssetPath,
    pub file: BundleVirtualFileRef,
    pub rect: CharacterRect,
    pub z: i32,
    pub opacity: u8,
    pub blend: CharacterBlendMode,
    pub clipping: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_layer: Option<CharacterSourceLayer>,
}

impl BundleCharacterPackage {
    /// Converts a validated `.awchar` package to bundle resource metadata and files.
    pub fn from_character_package(
        package: &CharacterPackage,
        root: impl AsRef<str>,
    ) -> Result<(Self, Vec<BundleVirtualFile>), BundleCodecError> {
        let root = normalize_resource_root(root.as_ref());
        let manifest_file = BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: format!("{root}/{CHARACTER_PACKAGE_MANIFEST_PATH}"),
            bytes: package.manifest_bytes().to_vec(),
        };
        let mut files = vec![manifest_file.clone()];
        let mut layers = Vec::new();
        for part in package.manifest().parts() {
            for variant in part.variants() {
                let Some(payload) = package.layer_payload(variant.asset()) else {
                    return Err(BundleCodecError::MissingCharacterLayerPayload {
                        character_id: package.manifest().character().to_string(),
                        path: variant.asset().as_str().to_owned(),
                        part: part.id().to_string(),
                        variant: variant.id().to_string(),
                    });
                };
                let file = BundleVirtualFile {
                    space: BundleVirtualFileSpace::Asset,
                    path: format!("{root}/{}", variant.asset().as_str()),
                    bytes: payload.bytes().to_vec(),
                };
                layers.push(BundleCharacterLayerResource {
                    part: part.id().to_string(),
                    variant: variant.id().to_string(),
                    asset_path: variant.asset().clone(),
                    file: BundleVirtualFileRef {
                        space: file.space,
                        path: file.path.clone(),
                    },
                    rect: variant.rect(),
                    z: part.z(),
                    opacity: variant.opacity(),
                    blend: variant.blend(),
                    clipping: variant.clipping(),
                    source_layer: variant.source_layer().cloned(),
                });
                files.push(file);
            }
        }
        layers.sort_by(|left, right| {
            left.z
                .cmp(&right.z)
                .then_with(|| left.part.cmp(&right.part))
                .then_with(|| left.variant.cmp(&right.variant))
        });
        Ok((
            Self {
                character: package.manifest().character().to_string(),
                manifest: BundleVirtualFileRef {
                    space: BundleVirtualFileSpace::Asset,
                    path: manifest_file.path,
                },
                layers,
            },
            files,
        ))
    }

    /// Verifies that every manifest/layer file reference exists in the bundle.
    pub fn validate_files(&self, files: &[BundleVirtualFile]) -> Result<(), BundleCodecError> {
        self.require_file(files, &self.manifest)?;
        for layer in &self.layers {
            self.require_file(files, &layer.file)?;
        }
        Ok(())
    }

    fn require_file(
        &self,
        files: &[BundleVirtualFile],
        file_ref: &BundleVirtualFileRef,
    ) -> Result<(), BundleCodecError> {
        files
            .iter()
            .any(|file| file.space == file_ref.space && file.path == file_ref.path)
            .then_some(())
            .ok_or_else(|| BundleCodecError::MissingCharacterPackageFile {
                character_id: self.character.clone(),
                path: file_ref.path.clone(),
            })
    }
}

fn normalize_resource_root(root: &str) -> String {
    root.trim_matches('/').to_owned()
}
