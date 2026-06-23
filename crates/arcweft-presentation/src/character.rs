//! Typed character-stage render data resolved from a character manifest.

use arcweft_character::{
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterManifest,
        CharacterManifestError, CharacterPoint, CharacterRect,
    },
};
use arcweft_id::PublicId;

/// One resolved image layer in a character-stage object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterRenderLayer {
    part: CharacterPartId,
    variant: CharacterVariantId,
    asset_id: PublicId,
    asset_path: CharacterAssetPath,
    rect: CharacterRect,
    z: i32,
    opacity: u8,
    blend: CharacterBlendMode,
    clipping: bool,
}

/// Renderer-independent character composition for one selected look.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterRenderSpec {
    character: CharacterId,
    look: CharacterLookId,
    canvas: CharacterCanvas,
    anchor: CharacterPoint,
    layers: Vec<CharacterRenderLayer>,
}

impl CharacterRenderSpec {
    /// Resolves a manifest look into bottom-to-top render layers.
    pub fn from_manifest(
        manifest: &CharacterManifest,
        look: &CharacterLookId,
    ) -> Result<Self, CharacterManifestError> {
        let layers = manifest
            .resolve_look(look)?
            .into_iter()
            .map(|resolved| CharacterRenderLayer {
                part: resolved.part().id().clone(),
                variant: resolved.variant().id().clone(),
                asset_id: resolved
                    .variant()
                    .asset_public_id(manifest.character(), resolved.part().id()),
                asset_path: resolved.variant().asset().clone(),
                rect: resolved.variant().rect(),
                z: resolved.part().z(),
                opacity: resolved.variant().opacity(),
                blend: resolved.variant().blend(),
                clipping: resolved.variant().clipping(),
            })
            .collect();
        Ok(Self {
            character: manifest.character().clone(),
            look: look.clone(),
            canvas: manifest.canvas(),
            anchor: manifest.anchor(),
            layers,
        })
    }

    /// Resolves the manifest's default look.
    pub fn from_manifest_default(
        manifest: &CharacterManifest,
    ) -> Result<Self, CharacterManifestError> {
        Self::from_manifest(manifest, manifest.default_look())
    }

    pub const fn character(&self) -> &CharacterId {
        &self.character
    }

    pub const fn look(&self) -> &CharacterLookId {
        &self.look
    }

    pub const fn canvas(&self) -> CharacterCanvas {
        self.canvas
    }

    pub const fn anchor(&self) -> CharacterPoint {
        self.anchor
    }

    pub fn layers(&self) -> &[CharacterRenderLayer] {
        &self.layers
    }
}

impl CharacterRenderLayer {
    pub const fn part(&self) -> &CharacterPartId {
        &self.part
    }

    pub const fn variant(&self) -> &CharacterVariantId {
        &self.variant
    }

    pub const fn asset_id(&self) -> &PublicId {
        &self.asset_id
    }

    pub const fn asset_path(&self) -> &CharacterAssetPath {
        &self.asset_path
    }

    pub const fn rect(&self) -> CharacterRect {
        self.rect
    }

    pub const fn z(&self) -> i32 {
        self.z
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_character::manifest::{
        CharacterLook, CharacterPart, CharacterPartSelection, CharacterVariant,
    };

    #[test]
    fn render_spec_keeps_manifest_z_order() {
        let look_id = CharacterLookId::try_new("normal").expect("look");
        let part_id = CharacterPartId::try_new("body").expect("part");
        let variant_id = CharacterVariantId::try_new("default").expect("variant");
        let manifest = CharacterManifest::new(
            CharacterId::try_new("character.akane").expect("character"),
            CharacterCanvas::new(32, 64),
            CharacterPoint::new(16, 64),
            look_id.clone(),
            vec![CharacterPart::new(
                part_id.clone(),
                4,
                vec![CharacterVariant::new(
                    variant_id.clone(),
                    CharacterAssetPath::try_new("layers/body.png").expect("path"),
                    CharacterRect::new(0, 0, 32, 64),
                    u8::MAX,
                    CharacterBlendMode::Normal,
                    false,
                )],
            )],
            vec![CharacterLook::new(
                look_id.clone(),
                vec![CharacterPartSelection::new(part_id, variant_id)],
            )],
            None,
        )
        .expect("manifest");
        let spec = CharacterRenderSpec::from_manifest(&manifest, &look_id).expect("spec");
        assert_eq!(spec.layers()[0].z(), 4);
    }
}
