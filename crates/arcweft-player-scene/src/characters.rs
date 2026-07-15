//! Host-neutral prepared-frame path for typed `.awchar` characters.

use arcweft_character::{
    id::{CharacterId, CharacterLookId},
    manifest::{CharacterManifest, CharacterManifestError, CharacterRuntimeDecodeError},
    package::CharacterPackage,
};
use arcweft_character_view::{
    CharacterImageSet, CharacterView, CharacterViewCompatibility, CharacterViewError,
};
use arcweft_presentation::character::{CharacterRenderSpec, CharacterStageBounds};
use std::collections::BTreeMap;
use thiserror::Error;

/// Decoded package data available to the player.
#[derive(Clone, Debug, Default)]
pub struct BundleCharacterCatalog {
    packages: BTreeMap<CharacterId, DecodedCharacterPackage>,
}

#[derive(Clone, Debug)]
struct DecodedCharacterPackage {
    manifest: CharacterManifest,
    images: CharacterImageSet,
}

/// Prepared character frame shared by native and web renderers.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedCharacterStageFrame {
    render: CharacterRenderSpec,
    view: CharacterView,
}

/// Agent-observable character object emitted from a prepared frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterObservedObject {
    pub character: String,
    pub look: String,
    pub bbox: CharacterStageBounds,
    pub capture_ref: String,
    pub layers: Vec<CharacterObservedLayer>,
}

/// Agent-observable metadata for one resolved character layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterObservedLayer {
    pub part: String,
    pub variant: String,
    pub asset_path: String,
    pub rect: arcweft_character::manifest::CharacterRect,
    pub z: i32,
    pub source_layer: Option<String>,
    pub capture_ref: String,
}

/// Failure while decoding or preparing character-stage data.
#[derive(Debug, Error)]
pub enum BundleCharacterCatalogError {
    #[error("character `{0}` is not present in the decoded catalog")]
    MissingCharacter(String),
    #[error("failed to decode character manifest: {0}")]
    ManifestDecode(#[from] CharacterRuntimeDecodeError),
    #[error(transparent)]
    Manifest(#[from] CharacterManifestError),
    #[error(transparent)]
    View(#[from] CharacterViewError),
}

impl BundleCharacterCatalog {
    /// Creates an empty decoded character catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a validated Sans I/O package.  The PNG bytes are decoded once and
    /// reused for every look preparation.
    pub fn insert_package(
        &mut self,
        package: &CharacterPackage,
    ) -> Result<(), BundleCharacterCatalogError> {
        let images = CharacterImageSet::from_png_files(
            package
                .layer_payloads()
                .map(|payload| (payload.path().clone(), payload.bytes().to_vec())),
        )?;
        self.packages.insert(
            package.manifest().character().clone(),
            DecodedCharacterPackage {
                manifest: package.manifest().clone(),
                images,
            },
        );
        Ok(())
    }

    /// Builds a catalog from one package.  Tests and small host adapters can use
    /// this path without constructing a whole product bundle.
    pub fn from_character_package(
        package: &CharacterPackage,
    ) -> Result<Self, BundleCharacterCatalogError> {
        let mut catalog = Self::new();
        catalog.insert_package(package)?;
        Ok(catalog)
    }

    /// Prepares one selected character look for shared native/web rendering.
    pub fn prepare(
        &self,
        character: &CharacterId,
        look: Option<&CharacterLookId>,
    ) -> Result<PreparedCharacterStageFrame, BundleCharacterCatalogError> {
        let decoded = self
            .packages
            .get(character)
            .ok_or_else(|| BundleCharacterCatalogError::MissingCharacter(character.to_string()))?;
        let look = look.unwrap_or_else(|| decoded.manifest.default_look());
        let render = CharacterRenderSpec::from_manifest(&decoded.manifest, look)?;
        let view = CharacterView::build(
            &render,
            &decoded.images,
            CharacterViewCompatibility::PreserveMetadata,
        )?;
        Ok(PreparedCharacterStageFrame { render, view })
    }
}

impl PreparedCharacterStageFrame {
    /// Renderer-independent selected look and layer stack.
    pub const fn render_spec(&self) -> &CharacterRenderSpec {
        &self.render
    }

    /// Retained View lowering consumed by the shared renderer path.
    pub const fn view(&self) -> &CharacterView {
        &self.view
    }

    /// Stable bbox derived from source canvas and anchor.
    pub fn stable_bbox(&self) -> CharacterStageBounds {
        self.render.source_canvas_bounds()
    }

    /// Builds Agent observe metadata without flattening the character.
    pub fn observe_object(&self) -> CharacterObservedObject {
        let character = self.render.character().to_string();
        let look = self.render.look().to_string();
        let capture_root = format!("capture.{character}");
        let layers = self
            .render
            .layers()
            .iter()
            .map(|layer| {
                let source_layer = layer.source_layer().map(|source| {
                    format!("{}/{}#{}", source.group(), source.layer(), source.index())
                });
                CharacterObservedLayer {
                    part: layer.part().to_string(),
                    variant: layer.variant().to_string(),
                    asset_path: layer.asset_path().as_str().to_owned(),
                    rect: layer.rect(),
                    z: layer.z(),
                    source_layer,
                    capture_ref: format!(
                        "{}.{}.{}",
                        capture_root,
                        layer.part().as_str(),
                        layer.variant().as_str()
                    ),
                }
            })
            .collect();
        CharacterObservedObject {
            character,
            look,
            bbox: self.stable_bbox(),
            capture_ref: capture_root,
            layers,
        }
    }
}
