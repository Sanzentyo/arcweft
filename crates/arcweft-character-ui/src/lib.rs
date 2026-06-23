//! Retained-UI lowering for typed Arcweft character compositions.
//!
//! This crate is Sans I/O. A host supplies decoded package images, and the
//! lowering builds the same `ViewFragment`, layout data, image source table, and
//! `UiLayerOutput` used by ordinary Arcweft UI components.

use arcweft_character::{
    id::{CharacterPartId, CharacterVariantId},
    manifest::{CharacterAssetPath, CharacterBlendMode, CharacterRect},
};
use arcweft_image::{
    DecodedImage, ImageDecodeOptions, ImageError, ImageFormat, decode_image_bytes,
};
use arcweft_presentation::character::{CharacterRenderLayer, CharacterRenderSpec};
use arcweft_ui::{
    ContainerKind, FragmentKind, ImageAlignment, ImageFit, ImageId, LayoutBox, LayoutLength,
    LayoutPoint, LayoutResults, LayoutSize, LayoutTree, NodeId, NodeKey, StyleId, UiError,
    UiImageSource, UiImageSourceTable, UiLayerOutput, UiResolvedImageFrame, UiSemanticFragment,
    ViewFragment, ViewFragmentBuilder,
};
use std::collections::BTreeMap;
use thiserror::Error;

/// Policy for metadata the retained image path cannot reproduce exactly yet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CharacterUiCompatibility {
    /// Reject unsupported blend modes and clipping before producing a view.
    Strict,
    /// Keep metadata in the layer table while rendering through the closest
    /// retained-image behavior.
    #[default]
    PreserveMetadata,
}

/// One decoded image keyed by its package-relative manifest path.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CharacterImageSet {
    images: BTreeMap<CharacterAssetPath, DecodedImage>,
}

/// One retained image node corresponding to a resolved character layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterUiLayer {
    part: CharacterPartId,
    variant: CharacterVariantId,
    asset: CharacterAssetPath,
    node: NodeId,
    image: ImageId,
    rect: CharacterRect,
    z: i32,
    blend: CharacterBlendMode,
    clipping: bool,
}

/// Retained component data for one character look.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterUiView {
    fragment: ViewFragment,
    layouts: LayoutResults,
    output: UiLayerOutput,
    image_sources: UiImageSourceTable,
    root: NodeId,
    layers: Vec<CharacterUiLayer>,
}

/// Failure while decoding package images or lowering a character view.
#[derive(Debug, Error)]
pub enum CharacterUiError {
    #[error("missing decoded image `{0}`")]
    MissingImage(String),
    #[error(
        "image `{asset}` dimensions {actual_width}x{actual_height} do not match manifest rectangle {expected_width}x{expected_height}"
    )]
    ImageDimensionMismatch {
        asset: String,
        expected_width: u32,
        expected_height: u32,
        actual_width: u32,
        actual_height: u32,
    },
    #[error("image `{asset}` could not be decoded: {source}")]
    ImageDecode {
        asset: String,
        #[source]
        source: ImageError,
    },
    #[error("character layer `{part}.{variant}` uses unsupported retained-UI blend mode {blend:?}")]
    UnsupportedBlend {
        part: String,
        variant: String,
        blend: CharacterBlendMode,
    },
    #[error("character layer `{part}.{variant}` uses clipping, which retained UI cannot reproduce")]
    UnsupportedClipping { part: String, variant: String },
    #[error("character coordinate or dimension cannot be represented by retained UI")]
    CoordinateOverflow,
    #[error("character layer index {0} is out of range")]
    LayerOutOfRange(usize),
    #[error(transparent)]
    Ui(#[from] UiError),
}

impl CharacterImageSet {
    /// Creates an empty decoded image set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a decoded package image, returning the previous value if present.
    pub fn insert(
        &mut self,
        path: CharacterAssetPath,
        image: DecodedImage,
    ) -> Option<DecodedImage> {
        self.images.insert(path, image)
    }

    /// Decodes PNG payloads without performing filesystem I/O.
    pub fn from_png_files(
        files: impl IntoIterator<Item = (CharacterAssetPath, Vec<u8>)>,
    ) -> Result<Self, CharacterUiError> {
        let mut set = Self::new();
        for (path, bytes) in files {
            let image = decode_image_bytes(ImageFormat::Png, &bytes, ImageDecodeOptions::default())
                .map_err(|source| CharacterUiError::ImageDecode {
                    asset: path.as_str().to_owned(),
                    source,
                })?;
            set.insert(path, image);
        }
        Ok(set)
    }

    /// Returns a decoded image by package-relative path.
    pub fn get(&self, path: &CharacterAssetPath) -> Option<&DecodedImage> {
        self.images.get(path)
    }

    /// Number of decoded package images.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

impl CharacterUiView {
    /// Lowers a resolved character render spec to retained UI image nodes.
    pub fn build(
        render: &CharacterRenderSpec,
        images: &CharacterImageSet,
        compatibility: CharacterUiCompatibility,
    ) -> Result<Self, CharacterUiError> {
        let mut fragment_builder = ViewFragmentBuilder::default();
        let mut image_sources = UiImageSourceTable::default();
        let mut image_nodes = Vec::with_capacity(render.layers().len());
        let mut layers = Vec::with_capacity(render.layers().len());

        for layer in render.layers() {
            check_compatibility(layer, compatibility)?;
            let image = images.get(layer.asset_path()).cloned().ok_or_else(|| {
                CharacterUiError::MissingImage(layer.asset_path().as_str().to_owned())
            })?;
            check_dimensions(layer, &image)?;

            let image_id = image_sources.insert(
                UiImageSource::new(image)
                    .with_fit(ImageFit::Stretch)
                    .with_alignment(ImageAlignment::top_left())
                    .with_opacity_milli(opacity_milli(layer.opacity())),
            )?;
            let node = fragment_builder.push_node(
                stable_layer_key(render, layer),
                FragmentKind::Image(image_id),
                StyleId(0),
                &[],
                &[],
                None,
            )?;
            image_nodes.push(node);
            layers.push(CharacterUiLayer {
                part: layer.part().clone(),
                variant: layer.variant().clone(),
                asset: layer.asset_path().clone(),
                node,
                image: image_id,
                rect: layer.rect(),
                z: layer.z(),
                blend: layer.blend(),
                clipping: layer.clipping(),
            });
        }

        let root = fragment_builder.push_node(
            stable_root_key(render),
            FragmentKind::Container(ContainerKind::Stack),
            StyleId(0),
            &image_nodes,
            &[],
            None,
        )?;
        let fragment = fragment_builder.finish();
        let tree = LayoutTree::from_fragment(&fragment)?;
        let mut layouts = LayoutResults::new(&tree);
        for layer in &layers {
            layouts.set(layer.node, rect_layout(layer.rect)?)?;
        }
        let canvas = render.canvas();
        layouts.set(
            root,
            LayoutBox::new(
                LayoutPoint::new(LayoutLength::px(0), LayoutLength::px(0)),
                LayoutSize::new(
                    LayoutLength::px(
                        i32::try_from(canvas.width())
                            .map_err(|_| CharacterUiError::CoordinateOverflow)?,
                    ),
                    LayoutLength::px(
                        i32::try_from(canvas.height())
                            .map_err(|_| CharacterUiError::CoordinateOverflow)?,
                    ),
                ),
            ),
        )?;
        let output =
            UiLayerOutput::from_fragment(&fragment, &layouts, UiSemanticFragment::default())?;

        Ok(Self {
            fragment,
            layouts,
            output,
            image_sources,
            root,
            layers,
        })
    }

    /// Retained fragment mounted below the character layer.
    pub const fn fragment(&self) -> &ViewFragment {
        &self.fragment
    }

    /// Exact fixed-point layouts for each cropped image and the canvas root.
    pub const fn layouts(&self) -> &LayoutResults {
        &self.layouts
    }

    /// Display/semantic output consumed by the UI frame commit path.
    pub const fn output(&self) -> &UiLayerOutput {
        &self.output
    }

    /// Decoded image sources referenced by `FragmentKind::Image` nodes.
    pub const fn image_sources(&self) -> &UiImageSourceTable {
        &self.image_sources
    }

    /// Root stack node for the character component.
    pub const fn root(&self) -> NodeId {
        self.root
    }

    /// Bottom-to-top layer metadata retained for diagnostics and future blend passes.
    pub fn layers(&self) -> &[CharacterUiLayer] {
        &self.layers
    }

    /// Resolves one retained character image to the frame submitted to the renderer.
    pub fn resolve_layer_frame(
        &self,
        index: usize,
        visual_time_millis: u64,
    ) -> Result<UiResolvedImageFrame<'_>, CharacterUiError> {
        let layer = self
            .layers
            .get(index)
            .ok_or(CharacterUiError::LayerOutOfRange(index))?;
        let layout = self.layouts.require(layer.node)?;
        self.image_sources
            .resolve_frame(layer.image, layout, visual_time_millis)
            .map_err(CharacterUiError::from)
    }
}

impl CharacterUiLayer {
    pub const fn part(&self) -> &CharacterPartId {
        &self.part
    }

    pub const fn variant(&self) -> &CharacterVariantId {
        &self.variant
    }

    pub const fn asset(&self) -> &CharacterAssetPath {
        &self.asset
    }

    pub const fn node(&self) -> NodeId {
        self.node
    }

    pub const fn image(&self) -> ImageId {
        self.image
    }

    pub const fn rect(&self) -> CharacterRect {
        self.rect
    }

    pub const fn z(&self) -> i32 {
        self.z
    }

    pub const fn blend(&self) -> CharacterBlendMode {
        self.blend
    }

    pub const fn clipping(&self) -> bool {
        self.clipping
    }
}

fn check_compatibility(
    layer: &CharacterRenderLayer,
    compatibility: CharacterUiCompatibility,
) -> Result<(), CharacterUiError> {
    if compatibility == CharacterUiCompatibility::PreserveMetadata {
        return Ok(());
    }
    if !layer.blend().is_retained_ui_supported() {
        return Err(CharacterUiError::UnsupportedBlend {
            part: layer.part().to_string(),
            variant: layer.variant().to_string(),
            blend: layer.blend(),
        });
    }
    if layer.clipping() {
        return Err(CharacterUiError::UnsupportedClipping {
            part: layer.part().to_string(),
            variant: layer.variant().to_string(),
        });
    }
    Ok(())
}

fn check_dimensions(
    layer: &CharacterRenderLayer,
    image: &DecodedImage,
) -> Result<(), CharacterUiError> {
    let actual = image.dimensions();
    let expected = layer.rect();
    if actual.width() == expected.width() && actual.height() == expected.height() {
        Ok(())
    } else {
        Err(CharacterUiError::ImageDimensionMismatch {
            asset: layer.asset_path().as_str().to_owned(),
            expected_width: expected.width(),
            expected_height: expected.height(),
            actual_width: actual.width(),
            actual_height: actual.height(),
        })
    }
}

fn rect_layout(rect: CharacterRect) -> Result<LayoutBox, CharacterUiError> {
    let width = i32::try_from(rect.width()).map_err(|_| CharacterUiError::CoordinateOverflow)?;
    let height = i32::try_from(rect.height()).map_err(|_| CharacterUiError::CoordinateOverflow)?;
    Ok(LayoutBox::new(
        LayoutPoint::new(LayoutLength::px(rect.x()), LayoutLength::px(rect.y())),
        LayoutSize::new(LayoutLength::px(width), LayoutLength::px(height)),
    ))
}

const fn opacity_milli(opacity: u8) -> u16 {
    (opacity as u16).saturating_mul(1_000) / 255
}

fn stable_root_key(render: &CharacterRenderSpec) -> NodeKey {
    stable_key(&[render.character().as_str(), render.look().as_str(), "root"])
}

fn stable_layer_key(render: &CharacterRenderSpec, layer: &CharacterRenderLayer) -> NodeKey {
    stable_key(&[
        render.character().as_str(),
        render.look().as_str(),
        layer.part().as_str(),
        layer.variant().as_str(),
    ])
}

fn stable_key(parts: &[&str]) -> NodeKey {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    NodeKey(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_character::{
        id::{CharacterId, CharacterLookId},
        manifest::{
            CharacterCanvas, CharacterLook, CharacterManifest, CharacterPart,
            CharacterPartSelection, CharacterPoint, CharacterVariant,
        },
    };
    use arcweft_image::{DecodedImageFrame, ImageDimensions, ImageRepetition};
    use arcweft_presentation::character::CharacterRenderSpec;

    fn sample_manifest() -> CharacterManifest {
        let part = CharacterPartId::try_new("body").expect("part");
        let variant = CharacterVariantId::try_new("uniform").expect("variant");
        let look = CharacterLookId::try_new("normal").expect("look");
        CharacterManifest::new(
            CharacterId::try_new("character.akane").expect("character"),
            CharacterCanvas::new(4, 8),
            CharacterPoint::new(2, 8),
            look.clone(),
            vec![CharacterPart::new(
                part.clone(),
                0,
                vec![CharacterVariant::new(
                    variant.clone(),
                    CharacterAssetPath::try_new("layers/body.png").expect("path"),
                    CharacterRect::new(0, 0, 4, 8),
                    u8::MAX,
                    CharacterBlendMode::Normal,
                    false,
                )],
            )],
            vec![CharacterLook::new(
                look,
                vec![CharacterPartSelection::new(part, variant)],
            )],
            None,
        )
        .expect("manifest")
    }

    fn image(width: u32, height: u32) -> DecodedImage {
        let dimensions = ImageDimensions::new(width, height).expect("dimensions");
        let frame = DecodedImageFrame::new(
            0,
            dimensions,
            0,
            vec![255; dimensions.rgba_len().expect("length")],
        )
        .expect("frame");
        DecodedImage::new(
            ImageFormat::Png,
            dimensions,
            ImageRepetition::Once,
            vec![frame],
        )
        .expect("image")
    }

    #[test]
    fn builds_retained_image_stack_from_render_spec() {
        let manifest = sample_manifest();
        let render = CharacterRenderSpec::from_manifest_default(&manifest).expect("render");
        let mut images = CharacterImageSet::new();
        images.insert(
            CharacterAssetPath::try_new("layers/body.png").expect("path"),
            image(4, 8),
        );

        let view = CharacterUiView::build(&render, &images, CharacterUiCompatibility::Strict)
            .expect("view");

        assert_eq!(view.layers().len(), 1);
        assert_eq!(view.output().display().as_slice().len(), 1);
        assert_eq!(
            view.resolve_layer_frame(0, 0)
                .expect("frame")
                .frame()
                .dimensions()
                .width(),
            4
        );
    }

    #[test]
    fn strict_mode_rejects_unimplemented_blend_behavior() {
        let mut manifest = sample_manifest();
        let json = manifest
            .to_json_pretty()
            .expect("json")
            .replace("\"blend\": \"normal\"", "\"blend\": \"overlay\"");
        manifest = CharacterManifest::from_json(&json).expect("manifest");
        let render = CharacterRenderSpec::from_manifest_default(&manifest).expect("render");
        let mut images = CharacterImageSet::new();
        images.insert(
            CharacterAssetPath::try_new("layers/body.png").expect("path"),
            image(4, 8),
        );

        assert!(matches!(
            CharacterUiView::build(&render, &images, CharacterUiCompatibility::Strict),
            Err(CharacterUiError::UnsupportedBlend { .. })
        ));
    }
}
