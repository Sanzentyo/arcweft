//! Lowering from presentation image objects into retained UI image fragments.

use crate::{
    DisplayList, FragmentKind, ImageAlignment, ImageFit, ImagePlayback, LayoutBox, LayoutLength,
    LayoutPoint, LayoutResults, LayoutSize, LayoutTree, NodeKey, SemanticSpecId, StyleId, UiError,
    UiImagePresentationMetadata, UiImageSource, UiImageSourceTable, UiLayerOutput,
    UiSemanticFragmentBuilder, UiSemanticNode, ViewFragmentBuilder,
};
use arcweft_image::DecodedImage;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::image::{
    ImageObjectAlignment, ImageObjectFit, ImageObjectPlayback, ImagePresentationObject,
};
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::SemanticRole;
use std::collections::BTreeMap;

/// A decoded image bound to a first-class presentation image object.
#[derive(Clone, Debug, PartialEq)]
pub struct UiImagePresentationInput {
    object: ImagePresentationObject,
    image: DecodedImage,
}

/// UI outputs for presentation image objects, sharing one image source table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiImagePresentationFrame {
    layers: Vec<(LayerId, UiLayerOutput)>,
    images: UiImageSourceTable,
}

#[derive(Default)]
struct LayerAssembly {
    fragment: ViewFragmentBuilder,
    layouts: Vec<(crate::NodeId, LayoutBox)>,
    semantics: UiSemanticFragmentBuilder,
}

impl UiImagePresentationInput {
    pub const fn new(object: ImagePresentationObject, image: DecodedImage) -> Self {
        Self { object, image }
    }

    pub const fn object(&self) -> &ImagePresentationObject {
        &self.object
    }

    pub const fn image(&self) -> &DecodedImage {
        &self.image
    }

    pub fn into_parts(self) -> (ImagePresentationObject, DecodedImage) {
        (self.object, self.image)
    }
}

impl UiImagePresentationFrame {
    pub fn from_inputs(
        inputs: impl IntoIterator<Item = UiImagePresentationInput>,
    ) -> Result<Self, UiError> {
        let mut images = UiImageSourceTable::default();
        let mut layers = BTreeMap::<LayerId, LayerAssembly>::new();
        for (index, input) in inputs.into_iter().enumerate() {
            let (object, image) = input.into_parts();
            if !object.visible() {
                continue;
            }
            let image_id = images.insert(
                UiImageSource::new(image)
                    .with_fit(ui_image_fit(object.fit()))
                    .with_alignment(ui_image_alignment(object.alignment()))
                    .with_playback(ui_image_playback(object.playback()))
                    .with_presentation(ui_image_presentation_metadata(&object)),
            )?;
            let layer = object.layer().clone();
            let assembly = layers.entry(layer).or_default();
            let key = image_object_node_key(&object, index);
            let semantic_id = SemanticSpecId(
                u32::try_from(assembly.layouts.len()).map_err(|_| UiError::CapacityExceeded)?,
            );
            let node = assembly.fragment.push_node(
                key,
                FragmentKind::Image(image_id),
                StyleId(0),
                &[],
                &[],
                Some(semantic_id),
            )?;
            assembly.layouts.push((node, layout_box(object.bounds())));
            let mut semantic = UiSemanticNode::new(
                key,
                object.layer().clone(),
                object.target().clone(),
                SemanticRole::Image,
                object.bounds(),
            )
            .with_enabled(object.enabled())
            .with_visible(object.visible());
            for action in object.actions() {
                semantic = semantic.with_action(action.clone());
            }
            assembly.semantics.push(semantic)?;
        }
        let layers = layers
            .into_iter()
            .map(|(layer, assembly)| {
                let fragment = assembly.fragment.finish();
                let tree = LayoutTree::from_fragment(&fragment)?;
                let mut layouts = LayoutResults::new(&tree);
                for (node, layout) in assembly.layouts {
                    layouts.set(node, layout)?;
                }
                let display = DisplayList::from_fragment(&fragment, &layouts)?;
                Ok((
                    layer,
                    UiLayerOutput::new(display, assembly.semantics.finish()),
                ))
            })
            .collect::<Result<Vec<_>, UiError>>()?;
        Ok(Self { layers, images })
    }

    pub fn layers(&self) -> &[(LayerId, UiLayerOutput)] {
        &self.layers
    }

    pub const fn images(&self) -> &UiImageSourceTable {
        &self.images
    }

    pub fn into_parts(self) -> (Vec<(LayerId, UiLayerOutput)>, UiImageSourceTable) {
        (self.layers, self.images)
    }
}

fn ui_image_fit(fit: ImageObjectFit) -> ImageFit {
    match fit {
        ImageObjectFit::Contain => ImageFit::Contain,
        ImageObjectFit::Cover => ImageFit::Cover,
        ImageObjectFit::Stretch => ImageFit::Stretch,
        ImageObjectFit::Intrinsic => ImageFit::Intrinsic,
    }
}

fn ui_image_alignment(alignment: ImageObjectAlignment) -> ImageAlignment {
    ImageAlignment::new(alignment.x_milli(), alignment.y_milli())
}

fn ui_image_playback(playback: ImageObjectPlayback) -> ImagePlayback {
    let mut result =
        ImagePlayback::new(playback.start_time_millis()).with_rate_milli(playback.rate_milli());
    if let Some(paused_at) = playback.paused_at_millis() {
        result = result.paused_at(paused_at);
    }
    if let Some(pinned) = playback.pinned_local_time_millis() {
        result = result.pinned_local_time(pinned);
    }
    result
}

fn layout_box(bounds: HitRect) -> LayoutBox {
    LayoutBox::new(
        LayoutPoint::new(layout_length(bounds.x), layout_length(bounds.y)),
        LayoutSize::new(layout_length(bounds.width), layout_length(bounds.height)),
    )
}

fn layout_length(value: f32) -> LayoutLength {
    if !value.is_finite() {
        return LayoutLength(0);
    }
    let milli = f64::from(value) * 1_000.0;
    let milli = milli.clamp(f64::from(i32::MIN), f64::from(i32::MAX));
    LayoutLength(milli.round().to_string().parse().unwrap_or(0))
}

fn image_object_node_key(object: &ImagePresentationObject, fallback: usize) -> NodeKey {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in object.id().public_id().as_str().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    if hash == 0 {
        NodeKey(u64::try_from(fallback).unwrap_or(u64::MAX))
    } else {
        NodeKey(hash)
    }
}

fn ui_image_presentation_metadata(object: &ImagePresentationObject) -> UiImagePresentationMetadata {
    UiImagePresentationMetadata::new(
        object.id().public_id().clone(),
        object.asset().public_id().clone(),
        object.target().id().clone(),
        object.layer().public_id().clone(),
        object.depth_milli(),
    )
    .with_params(object.params().clone())
    .with_actions(object.actions().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_id::PublicId;
    use arcweft_image::{
        DecodedImage, DecodedImageFrame, ImageDimensions, ImageFormat, ImageRepetition,
    };
    use arcweft_presentation::image::{
        ImageAssetRef, ImageObjectId, ImageObjectPlayback, ImagePresentationObject,
    };
    use arcweft_presentation::input::InteractionTarget;

    fn public_id(value: &str) -> PublicId {
        PublicId::try_new(value).unwrap()
    }

    fn two_frame_image() -> DecodedImage {
        let dimensions = ImageDimensions::new(1, 1).unwrap();
        DecodedImage::new(
            ImageFormat::Gif,
            dimensions,
            ImageRepetition::Infinite,
            vec![
                DecodedImageFrame::new(0, dimensions, 100, vec![255, 0, 0, 255]).unwrap(),
                DecodedImageFrame::new(1, dimensions, 100, vec![0, 255, 0, 255]).unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn presentation_image_objects_lower_to_ui_sources_display_and_semantics() {
        let layer = LayerId::new(public_id("layer.hud"));
        let target = InteractionTarget::new(public_id("target.logo"));
        let action = public_id("action.inspect.logo");
        let object = ImagePresentationObject::new(
            ImageObjectId::new(public_id("image.logo")),
            ImageAssetRef::new(public_id("asset.logo")),
            layer.clone(),
            target.clone(),
            HitRect::new(10.25, 20.5, 30.0, 40.75),
        )
        .with_fit(ImageObjectFit::Cover)
        .with_alignment(ImageObjectAlignment::top_left())
        .with_playback(ImageObjectPlayback::new(0).pinned_local_time(150))
        .with_action(action.clone());

        let frame = UiImagePresentationFrame::from_inputs([UiImagePresentationInput::new(
            object,
            two_frame_image(),
        )])
        .unwrap();

        assert_eq!(frame.images().len(), 1);
        let (layers, images) = frame.into_parts();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].0, layer);
        let output = &layers[0].1;
        let display = output.display().as_slice();
        assert_eq!(display.len(), 1);
        assert_eq!(
            display[0].kind(),
            crate::DisplayItemKind::Image(crate::ImageId(0))
        );
        assert_eq!(display[0].layout().origin.x, LayoutLength(10_250));
        assert_eq!(display[0].layout().origin.y, LayoutLength(20_500));
        assert_eq!(display[0].layout().size.width, LayoutLength(30_000));
        assert_eq!(display[0].layout().size.height, LayoutLength(40_750));

        let resolved = images
            .resolve_frame(crate::ImageId(0), display[0].layout(), 0)
            .unwrap();
        assert_eq!(resolved.frame().index(), 1);
        assert_eq!(resolved.fit(), ImageFit::Cover);
        assert_eq!(resolved.alignment(), ImageAlignment::top_left());
        let metadata = images
            .get(crate::ImageId(0))
            .and_then(UiImageSource::presentation)
            .expect("presentation image metadata is preserved with source");
        assert_eq!(metadata.object().as_str(), "image.logo");
        assert_eq!(metadata.asset().as_str(), "asset.logo");
        assert_eq!(metadata.target().as_str(), "target.logo");
        assert_eq!(metadata.layer().as_str(), "layer.hud");
        assert_eq!(metadata.actions(), &[action.clone()]);

        let semantics = output.semantics().as_slice();
        assert_eq!(semantics.len(), 1);
        assert_eq!(semantics[0].layer(), &layer);
        assert_eq!(semantics[0].target(), &target);
        assert_eq!(semantics[0].role(), SemanticRole::Image);
        assert_eq!(semantics[0].actions(), &[action]);
    }
}
