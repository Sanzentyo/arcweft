//! Runtime character-manifest values and their shared semantic validation.
//!
//! These data types remain together so the runtime and registration decoders
//! validate exactly the same model without a second projection layer.

use crate::id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId};
use crate::{CHARACTER_MANIFEST_FORMAT, CHARACTER_MANIFEST_VERSION};
use arcweft_id::PublicId;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Component, Path},
};
use thiserror::Error;

/// Relative path to one extracted character image.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CharacterAssetPath(String);

/// Character source canvas in logical pixels.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterCanvas {
    pub(super) width: u32,
    pub(super) height: u32,
}

/// Integer point in source-canvas coordinates.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterPoint {
    pub(super) x: i32,
    pub(super) y: i32,
}

/// Rectangle in source-canvas coordinates.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterRect {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: u32,
    pub(super) height: u32,
}

/// Blend operation preserved from the source artwork.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterBlendMode {
    PassThrough,
    Normal,
    Dissolve,
    Darken,
    Multiply,
    ColorBurn,
    LinearBurn,
    DarkerColor,
    Lighten,
    Screen,
    ColorDodge,
    LinearDodge,
    LighterColor,
    Overlay,
    SoftLight,
    HardLight,
    VividLight,
    LinearLight,
    PinLight,
    HardMix,
    Difference,
    Exclusion,
    Subtract,
    Divide,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

/// One selectable pixel layer inside a character part.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterVariant {
    pub(super) id: CharacterVariantId,
    pub(super) asset: CharacterAssetPath,
    pub(super) rect: CharacterRect,
    pub(super) opacity: u8,
    pub(super) blend: CharacterBlendMode,
    pub(super) clipping: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) source_layer: Option<CharacterSourceLayer>,
}

/// One independently selected character part such as body, eyes, or mouth.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterPart {
    pub(super) id: CharacterPartId,
    pub(super) z: i32,
    pub(super) variants: Vec<CharacterVariant>,
}

/// Selection of one variant for one part.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterPartSelection {
    pub(super) part: CharacterPartId,
    pub(super) variant: CharacterVariantId,
}

/// Complete named character appearance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterLook {
    pub(super) id: CharacterLookId,
    pub(super) select: Vec<CharacterPartSelection>,
}

/// Source-kind recorded in import provenance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSourceKind {
    Psd,
}

/// Source provenance without embedding an absolute host path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterSource {
    pub(super) kind: CharacterSourceKind,
    pub(super) file_name: String,
    pub(super) blake3: String,
    pub(super) importer: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) warnings: Vec<String>,
}

/// Trace back to the PSD layer that produced one variant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterSourceLayer {
    pub(super) index: usize,
    pub(super) group: String,
    pub(super) layer: String,
}

/// Versioned Arcweft character-composition manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CharacterManifest {
    pub(super) format: String,
    pub(super) version: u32,
    pub(super) character: CharacterId,
    pub(super) canvas: CharacterCanvas,
    pub(super) anchor: CharacterPoint,
    pub(super) default_look: CharacterLookId,
    pub(super) parts: Vec<CharacterPart>,
    pub(super) looks: Vec<CharacterLook>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<CharacterSource>,
}

/// A resolved part/variant pair in deterministic render order.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedCharacterLayer<'a> {
    pub(super) part: &'a CharacterPart,
    pub(super) variant: &'a CharacterVariant,
}

/// Deterministic character manifest validation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterManifestError {
    #[error("unsupported character manifest format `{0}`")]
    UnsupportedFormat(String),
    #[error("unsupported character manifest version {0}")]
    UnsupportedVersion(u32),
    #[error("character canvas dimensions must both be non-zero")]
    EmptyCanvas,
    #[error("character manifest must declare at least one part")]
    MissingParts,
    #[error("character manifest must declare at least one look")]
    MissingLooks,
    #[error("character `{character}` contains duplicate part `{part}`")]
    DuplicatePart {
        character: CharacterId,
        part: CharacterPartId,
    },
    #[error("character `{character}` part `{part}` contains duplicate variant `{variant}`")]
    DuplicateVariant {
        character: CharacterId,
        part: CharacterPartId,
        variant: CharacterVariantId,
    },
    #[error("character `{character}` contains duplicate look `{look}`")]
    DuplicateLook {
        character: CharacterId,
        look: CharacterLookId,
    },
    #[error("character `{character}` contains duplicate extracted asset path `{asset}`")]
    DuplicateAssetPath {
        character: CharacterId,
        asset: CharacterAssetPath,
    },
    #[error("character `{character}` part `{part}` must contain at least one variant")]
    EmptyPart {
        character: CharacterId,
        part: CharacterPartId,
    },
    #[error("character `{character}` variant `{part}.{variant}` has an empty rectangle")]
    EmptyVariantRect {
        character: CharacterId,
        part: CharacterPartId,
        variant: CharacterVariantId,
    },
    #[error("character `{character}` default look `{look}` is not declared")]
    MissingDefaultLook {
        character: CharacterId,
        look: CharacterLookId,
    },
    #[error("character `{character}` look `{look}` selects part `{part}` more than once")]
    DuplicateLookPart {
        character: CharacterId,
        look: CharacterLookId,
        part: CharacterPartId,
    },
    #[error("character `{character}` look `{look}` does not select part `{part}`")]
    MissingLookPart {
        character: CharacterId,
        look: CharacterLookId,
        part: CharacterPartId,
    },
    #[error("character `{character}` look `{look}` selects unknown part `{part}`")]
    UnknownLookPart {
        character: CharacterId,
        look: CharacterLookId,
        part: CharacterPartId,
    },
    #[error("character `{character}` look `{look}` selects unknown variant `{part}.{variant}`")]
    UnknownLookVariant {
        character: CharacterId,
        look: CharacterLookId,
        part: CharacterPartId,
        variant: CharacterVariantId,
    },
    #[error("character `{character}` look `{look}` is not declared")]
    UnknownLook {
        character: CharacterId,
        look: CharacterLookId,
    },
}

/// Relative asset path validation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterAssetPathError {
    #[error("character asset path must not be empty")]
    Empty,
    #[error("character asset path must be relative and must not contain `..`")]
    Unsafe,
    #[error("character asset path must point to a `.png` file")]
    NotPng,
}

impl CharacterAssetPath {
    /// Validates a portable, package-relative PNG path.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CharacterAssetPathError> {
        let value = value.into().replace('\\', "/");
        if value.is_empty() {
            return Err(CharacterAssetPathError::Empty);
        }
        let path = Path::new(&value);
        let safe = !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
        if !safe {
            return Err(CharacterAssetPathError::Unsafe);
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("png") {
            return Err(CharacterAssetPathError::NotPng);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CharacterAssetPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CharacterAssetPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(de::Error::custom)
    }
}

impl CharacterCanvas {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }
}

impl CharacterPoint {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn y(self) -> i32 {
        self.y
    }
}

impl CharacterRect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn y(self) -> i32 {
        self.y
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }
}

impl CharacterBlendMode {
    /// Converts the upstream `psd` crate's public debug spelling into Arcweft's
    /// stable blend-mode enum.
    ///
    /// `psd::PsdLayer::blend_mode` returns a public type from a private module,
    /// so consumers cannot name that type in a function signature. Keeping this
    /// mapping on the Arcweft enum avoids an importer-local string switch and
    /// gives every importer one canonical conversion table.
    pub fn from_photoshop_debug_name(name: &str) -> Option<Self> {
        Some(match name {
            "PassThrough" => Self::PassThrough,
            "Normal" => Self::Normal,
            "Dissolve" => Self::Dissolve,
            "Darken" => Self::Darken,
            "Multiply" => Self::Multiply,
            "ColorBurn" => Self::ColorBurn,
            "LinearBurn" => Self::LinearBurn,
            "DarkerColor" => Self::DarkerColor,
            "Lighten" => Self::Lighten,
            "Screen" => Self::Screen,
            "ColorDodge" => Self::ColorDodge,
            "LinearDodge" => Self::LinearDodge,
            "LighterColor" => Self::LighterColor,
            "Overlay" => Self::Overlay,
            "SoftLight" => Self::SoftLight,
            "HardLight" => Self::HardLight,
            "VividLight" => Self::VividLight,
            "LinearLight" => Self::LinearLight,
            "PinLight" => Self::PinLight,
            "HardMix" => Self::HardMix,
            "Difference" => Self::Difference,
            "Exclusion" => Self::Exclusion,
            "Subtract" => Self::Subtract,
            "Divide" => Self::Divide,
            "Hue" => Self::Hue,
            "Saturation" => Self::Saturation,
            "Color" => Self::Color,
            "Luminosity" => Self::Luminosity,
            _ => return None,
        })
    }

    /// Whether the first native character renderer can reproduce this mode.
    ///
    /// Unsupported modes are still preserved by the manifest for future
    /// renderer implementations and tooling diagnostics.
    pub const fn is_baseline_renderer_supported(self) -> bool {
        matches!(
            self,
            Self::PassThrough | Self::Normal | Self::Multiply | Self::Screen
        )
    }

    /// Whether the retained View image node path reproduces this mode exactly.
    ///
    /// The behavior belongs to the owned enum so importers and View adapters do
    /// not grow separate blend-mode switches or extension traits.
    pub const fn is_retained_view_supported(self) -> bool {
        matches!(self, Self::PassThrough | Self::Normal)
    }

    /// Stable replay/serialization code independent of Rust enum layout.
    pub const fn stable_code(self) -> u32 {
        match self {
            Self::PassThrough => 0,
            Self::Normal => 1,
            Self::Dissolve => 2,
            Self::Darken => 3,
            Self::Multiply => 4,
            Self::ColorBurn => 5,
            Self::LinearBurn => 6,
            Self::DarkerColor => 7,
            Self::Lighten => 8,
            Self::Screen => 9,
            Self::ColorDodge => 10,
            Self::LinearDodge => 11,
            Self::LighterColor => 12,
            Self::Overlay => 13,
            Self::SoftLight => 14,
            Self::HardLight => 15,
            Self::VividLight => 16,
            Self::LinearLight => 17,
            Self::PinLight => 18,
            Self::HardMix => 19,
            Self::Difference => 20,
            Self::Exclusion => 21,
            Self::Subtract => 22,
            Self::Divide => 23,
            Self::Hue => 24,
            Self::Saturation => 25,
            Self::Color => 26,
            Self::Luminosity => 27,
        }
    }
}

impl CharacterVariant {
    pub const fn new(
        id: CharacterVariantId,
        asset: CharacterAssetPath,
        rect: CharacterRect,
        opacity: u8,
        blend: CharacterBlendMode,
        clipping: bool,
    ) -> Self {
        Self {
            id,
            asset,
            rect,
            opacity,
            blend,
            clipping,
            source_layer: None,
        }
    }

    #[must_use]
    pub fn with_source_layer(mut self, source_layer: CharacterSourceLayer) -> Self {
        self.source_layer = Some(source_layer);
        self
    }

    pub const fn id(&self) -> &CharacterVariantId {
        &self.id
    }

    pub const fn asset(&self) -> &CharacterAssetPath {
        &self.asset
    }

    /// Deterministic resource id used by presentation/View resource nodes.
    ///
    /// # Panics
    ///
    /// Panics only if validated character, part, or variant ids are bypassed and
    /// an invalid public asset id is formed.
    pub fn asset_public_id(&self, character: &CharacterId, part: &CharacterPartId) -> PublicId {
        let character_name = character
            .as_str()
            .strip_prefix("character.")
            .unwrap_or_else(|| character.as_str());
        PublicId::try_new(format!(
            "asset.character.{character_name}.{}.{}",
            part.as_str(),
            self.id.as_str()
        ))
        .expect("validated character, part, and variant ids form a public asset id")
    }

    pub const fn rect(&self) -> CharacterRect {
        self.rect
    }

    pub const fn opacity(&self) -> u8 {
        self.opacity
    }

    pub const fn blend(&self) -> CharacterBlendMode {
        self.blend
    }

    pub const fn clipping(&self) -> bool {
        self.clipping
    }

    pub const fn source_layer(&self) -> Option<&CharacterSourceLayer> {
        self.source_layer.as_ref()
    }
}

impl CharacterPart {
    pub fn new(id: CharacterPartId, z: i32, variants: Vec<CharacterVariant>) -> Self {
        Self { id, z, variants }
    }

    pub const fn id(&self) -> &CharacterPartId {
        &self.id
    }

    pub const fn z(&self) -> i32 {
        self.z
    }

    pub fn variants(&self) -> &[CharacterVariant] {
        &self.variants
    }

    pub fn variant(&self, id: &CharacterVariantId) -> Option<&CharacterVariant> {
        self.variants.iter().find(|variant| variant.id() == id)
    }
}

impl CharacterPartSelection {
    pub const fn new(part: CharacterPartId, variant: CharacterVariantId) -> Self {
        Self { part, variant }
    }

    pub const fn part(&self) -> &CharacterPartId {
        &self.part
    }

    pub const fn variant(&self) -> &CharacterVariantId {
        &self.variant
    }
}

impl CharacterLook {
    pub fn new(id: CharacterLookId, select: Vec<CharacterPartSelection>) -> Self {
        Self { id, select }
    }

    pub const fn id(&self) -> &CharacterLookId {
        &self.id
    }

    pub fn selections(&self) -> &[CharacterPartSelection] {
        &self.select
    }
}

impl CharacterSource {
    pub fn psd(
        file_name: impl Into<String>,
        blake3: impl Into<String>,
        importer: impl Into<String>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            kind: CharacterSourceKind::Psd,
            file_name: file_name.into(),
            blake3: blake3.into(),
            importer: importer.into(),
            warnings,
        }
    }

    pub const fn kind(&self) -> CharacterSourceKind {
        self.kind
    }

    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    pub fn blake3(&self) -> &str {
        &self.blake3
    }

    pub fn importer(&self) -> &str {
        &self.importer
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

impl CharacterSourceLayer {
    pub fn new(index: usize, group: impl Into<String>, layer: impl Into<String>) -> Self {
        Self {
            index,
            group: group.into(),
            layer: layer.into(),
        }
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn layer(&self) -> &str {
        &self.layer
    }
}

impl CharacterManifest {
    pub fn new(
        character: CharacterId,
        canvas: CharacterCanvas,
        anchor: CharacterPoint,
        default_look: CharacterLookId,
        parts: Vec<CharacterPart>,
        looks: Vec<CharacterLook>,
        source: Option<CharacterSource>,
    ) -> Result<Self, CharacterManifestError> {
        let manifest = Self {
            format: CHARACTER_MANIFEST_FORMAT.to_owned(),
            version: CHARACTER_MANIFEST_VERSION,
            character,
            canvas,
            anchor,
            default_look,
            parts,
            looks,
            source,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Revalidates format invariants after deserialization or construction.
    pub fn validate(&self) -> Result<(), CharacterManifestError> {
        if self.format != CHARACTER_MANIFEST_FORMAT {
            return Err(CharacterManifestError::UnsupportedFormat(
                self.format.clone(),
            ));
        }
        if self.version != CHARACTER_MANIFEST_VERSION {
            return Err(CharacterManifestError::UnsupportedVersion(self.version));
        }
        if self.canvas.width == 0 || self.canvas.height == 0 {
            return Err(CharacterManifestError::EmptyCanvas);
        }
        if self.parts.is_empty() {
            return Err(CharacterManifestError::MissingParts);
        }
        if self.looks.is_empty() {
            return Err(CharacterManifestError::MissingLooks);
        }

        self.validate_composition()
    }

    fn validate_composition(&self) -> Result<(), CharacterManifestError> {
        let mut part_ids = BTreeSet::new();
        let mut asset_paths = BTreeSet::new();
        let mut variants_by_part =
            BTreeMap::<&CharacterPartId, BTreeSet<&CharacterVariantId>>::new();
        for part in &self.parts {
            if !part_ids.insert(&part.id) {
                return Err(CharacterManifestError::DuplicatePart {
                    character: self.character.clone(),
                    part: part.id.clone(),
                });
            }
            if part.variants.is_empty() {
                return Err(CharacterManifestError::EmptyPart {
                    character: self.character.clone(),
                    part: part.id.clone(),
                });
            }
            let mut variant_ids = BTreeSet::new();
            for variant in &part.variants {
                if !variant_ids.insert(&variant.id) {
                    return Err(CharacterManifestError::DuplicateVariant {
                        character: self.character.clone(),
                        part: part.id.clone(),
                        variant: variant.id.clone(),
                    });
                }
                if !asset_paths.insert(&variant.asset) {
                    return Err(CharacterManifestError::DuplicateAssetPath {
                        character: self.character.clone(),
                        asset: variant.asset.clone(),
                    });
                }
                if variant.rect.width == 0 || variant.rect.height == 0 {
                    return Err(CharacterManifestError::EmptyVariantRect {
                        character: self.character.clone(),
                        part: part.id.clone(),
                        variant: variant.id.clone(),
                    });
                }
            }
            variants_by_part.insert(&part.id, variant_ids);
        }

        let mut look_ids = BTreeSet::new();
        for look in &self.looks {
            if !look_ids.insert(&look.id) {
                return Err(CharacterManifestError::DuplicateLook {
                    character: self.character.clone(),
                    look: look.id.clone(),
                });
            }
            let mut selected = BTreeSet::new();
            for selection in &look.select {
                if !selected.insert(&selection.part) {
                    return Err(CharacterManifestError::DuplicateLookPart {
                        character: self.character.clone(),
                        look: look.id.clone(),
                        part: selection.part.clone(),
                    });
                }
                let Some(variants) = variants_by_part.get(&selection.part) else {
                    return Err(CharacterManifestError::UnknownLookPart {
                        character: self.character.clone(),
                        look: look.id.clone(),
                        part: selection.part.clone(),
                    });
                };
                if !variants.contains(&selection.variant) {
                    return Err(CharacterManifestError::UnknownLookVariant {
                        character: self.character.clone(),
                        look: look.id.clone(),
                        part: selection.part.clone(),
                        variant: selection.variant.clone(),
                    });
                }
            }
            if let Some(missing) = part_ids.iter().find(|part| !selected.contains(*part)) {
                return Err(CharacterManifestError::MissingLookPart {
                    character: self.character.clone(),
                    look: look.id.clone(),
                    part: (*missing).clone(),
                });
            }
        }
        if !look_ids.contains(&self.default_look) {
            return Err(CharacterManifestError::MissingDefaultLook {
                character: self.character.clone(),
                look: self.default_look.clone(),
            });
        }
        Ok(())
    }

    pub fn format(&self) -> &str {
        &self.format
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn character(&self) -> &CharacterId {
        &self.character
    }

    pub const fn canvas(&self) -> CharacterCanvas {
        self.canvas
    }

    pub const fn anchor(&self) -> CharacterPoint {
        self.anchor
    }

    pub const fn default_look(&self) -> &CharacterLookId {
        &self.default_look
    }

    pub fn parts(&self) -> &[CharacterPart] {
        &self.parts
    }

    /// Returns one manifest part by id.
    pub fn part(&self, id: &CharacterPartId) -> Option<&CharacterPart> {
        self.parts.iter().find(|part| part.id() == id)
    }

    pub fn looks(&self) -> &[CharacterLook] {
        &self.looks
    }

    pub const fn source(&self) -> Option<&CharacterSource> {
        self.source.as_ref()
    }

    pub fn look(&self, id: &CharacterLookId) -> Option<&CharacterLook> {
        self.looks.iter().find(|look| look.id() == id)
    }

    /// Resolves one look into bottom-to-top render layers.
    pub fn resolve_look(
        &self,
        id: &CharacterLookId,
    ) -> Result<Vec<ResolvedCharacterLayer<'_>>, CharacterManifestError> {
        let look = self
            .look(id)
            .ok_or_else(|| CharacterManifestError::UnknownLook {
                character: self.character.clone(),
                look: id.clone(),
            })?;
        let selected = look
            .selections()
            .iter()
            .map(|selection| (selection.part(), selection.variant()))
            .collect::<BTreeMap<_, _>>();
        let mut layers = self
            .parts
            .iter()
            .map(|part| {
                let variant_id = selected.get(part.id()).ok_or_else(|| {
                    CharacterManifestError::MissingLookPart {
                        character: self.character.clone(),
                        look: id.clone(),
                        part: part.id().clone(),
                    }
                })?;
                let variant = part.variant(variant_id).ok_or_else(|| {
                    CharacterManifestError::UnknownLookVariant {
                        character: self.character.clone(),
                        look: id.clone(),
                        part: part.id().clone(),
                        variant: (*variant_id).clone(),
                    }
                })?;
                Ok(ResolvedCharacterLayer { part, variant })
            })
            .collect::<Result<Vec<_>, _>>()?;
        layers.sort_by_key(|layer| (layer.part.z(), layer.part.id().clone()));
        Ok(layers)
    }
}

impl<'a> ResolvedCharacterLayer<'a> {
    pub const fn new(part: &'a CharacterPart, variant: &'a CharacterVariant) -> Self {
        Self { part, variant }
    }

    pub const fn part(self) -> &'a CharacterPart {
        self.part
    }

    pub const fn variant(self) -> &'a CharacterVariant {
        self.variant
    }
}
