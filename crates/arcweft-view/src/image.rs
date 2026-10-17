//! View image source table and deterministic animated-frame selection.

use crate::{ImageId, LayoutBox, ViewError};
use arcweft_id::PublicId;
use arcweft_image::{DecodedImage, DecodedImageFrame};
use arcweft_interaction_model::presentation::{
    ImageObjectAlignment, ImageObjectFit, ImageObjectPlayback, ImageObjectTransform,
};
use arcweft_presentation::image::{ImageObjectParam, ImageObjectProxy};
use std::collections::BTreeMap;

/// Image source registered outside the retained fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewImageSource {
    image: DecodedImage,
    fit: ImageObjectFit,
    alignment: ImageObjectAlignment,
    opacity_milli: u16,
    transform: ImageObjectTransform,
    playback: ImageObjectPlayback,
    presentation: Option<ViewImagePresentationMetadata>,
}

/// Presentation-object metadata preserved with an image source for Agent/debug output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewImagePresentationMetadata {
    object: PublicId,
    asset: PublicId,
    target: PublicId,
    layer: PublicId,
    opacity_milli: u16,
    depth_milli: i32,
    transform: ImageObjectTransform,
    params: BTreeMap<PublicId, ImageObjectParam>,
    proxies: Vec<ImageObjectProxy>,
    actions: Vec<PublicId>,
}

/// Resolved image frame ready for renderer submission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewResolvedImageFrame<'a> {
    source: ImageId,
    frame: &'a DecodedImageFrame,
    fit: ImageObjectFit,
    alignment: ImageObjectAlignment,
    opacity_milli: u16,
    transform: ImageObjectTransform,
    layout: LayoutBox,
}

/// Dense View image source registry keyed by `ImageId`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewImageSourceTable {
    sources: BTreeMap<ImageId, ViewImageSource>,
    next: u32,
}

impl ViewImageSource {
    pub fn new(image: DecodedImage) -> Self {
        Self {
            image,
            fit: ImageObjectFit::default(),
            alignment: ImageObjectAlignment::default(),
            opacity_milli: 1_000,
            transform: ImageObjectTransform::identity(),
            playback: ImageObjectPlayback::new(0),
            presentation: None,
        }
    }

    #[must_use]
    pub const fn with_fit(mut self, fit: ImageObjectFit) -> Self {
        self.fit = fit;
        self
    }

    #[must_use]
    pub const fn with_alignment(mut self, alignment: ImageObjectAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    #[must_use]
    pub const fn with_opacity_milli(mut self, opacity_milli: u16) -> Self {
        self.opacity_milli = opacity_milli;
        self
    }

    #[must_use]
    pub const fn with_transform(mut self, transform: ImageObjectTransform) -> Self {
        self.transform = transform;
        self
    }

    #[must_use]
    pub const fn with_playback(mut self, playback: ImageObjectPlayback) -> Self {
        self.playback = playback;
        self
    }

    #[must_use]
    pub fn with_presentation(mut self, presentation: ViewImagePresentationMetadata) -> Self {
        self.presentation = Some(presentation);
        self
    }

    pub const fn image(&self) -> &DecodedImage {
        &self.image
    }

    pub const fn fit(&self) -> ImageObjectFit {
        self.fit
    }

    pub const fn alignment(&self) -> ImageObjectAlignment {
        self.alignment
    }

    pub const fn opacity_milli(&self) -> u16 {
        self.opacity_milli
    }

    pub const fn transform(&self) -> ImageObjectTransform {
        self.transform
    }

    pub const fn playback(&self) -> ImageObjectPlayback {
        self.playback
    }

    pub fn presentation(&self) -> Option<&ViewImagePresentationMetadata> {
        self.presentation.as_ref()
    }

    pub fn frame_at_time(&self, visual_time_millis: u64) -> Option<&DecodedImageFrame> {
        self.image
            .frame_at_time_millis(self.playback.local_time_millis(visual_time_millis))
    }
}

impl ViewImagePresentationMetadata {
    pub fn new(
        object: PublicId,
        asset: PublicId,
        target: PublicId,
        layer: PublicId,
        opacity_milli: u16,
        depth_milli: i32,
        transform: ImageObjectTransform,
    ) -> Self {
        Self {
            object,
            asset,
            target,
            layer,
            opacity_milli,
            depth_milli,
            transform,
            params: BTreeMap::new(),
            proxies: Vec::new(),
            actions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_params(mut self, params: BTreeMap<PublicId, ImageObjectParam>) -> Self {
        self.params = params;
        self
    }

    #[must_use]
    pub fn with_proxies(mut self, proxies: Vec<ImageObjectProxy>) -> Self {
        self.proxies = proxies;
        self
    }

    #[must_use]
    pub fn with_actions(mut self, actions: Vec<PublicId>) -> Self {
        self.actions = actions;
        self
    }

    pub const fn object(&self) -> &PublicId {
        &self.object
    }

    pub const fn asset(&self) -> &PublicId {
        &self.asset
    }

    pub const fn target(&self) -> &PublicId {
        &self.target
    }

    pub const fn layer(&self) -> &PublicId {
        &self.layer
    }

    pub const fn opacity_milli(&self) -> u16 {
        self.opacity_milli
    }

    pub const fn depth_milli(&self) -> i32 {
        self.depth_milli
    }

    pub const fn transform(&self) -> ImageObjectTransform {
        self.transform
    }

    pub const fn params(&self) -> &BTreeMap<PublicId, ImageObjectParam> {
        &self.params
    }

    pub fn proxies(&self) -> &[ImageObjectProxy] {
        &self.proxies
    }

    pub fn actions(&self) -> &[PublicId] {
        &self.actions
    }
}

impl<'a> ViewResolvedImageFrame<'a> {
    pub const fn source(self) -> ImageId {
        self.source
    }

    pub const fn frame(self) -> &'a DecodedImageFrame {
        self.frame
    }

    pub const fn fit(self) -> ImageObjectFit {
        self.fit
    }

    pub const fn alignment(self) -> ImageObjectAlignment {
        self.alignment
    }

    pub const fn opacity_milli(self) -> u16 {
        self.opacity_milli
    }

    pub const fn transform(self) -> ImageObjectTransform {
        self.transform
    }

    pub const fn layout(self) -> LayoutBox {
        self.layout
    }
}

impl ViewImageSourceTable {
    pub fn insert(&mut self, source: ViewImageSource) -> Result<ImageId, ViewError> {
        let id = ImageId(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ViewError::CapacityExceeded)?;
        self.insert_with_id(id, source)?;
        Ok(id)
    }

    pub fn insert_with_id(
        &mut self,
        id: ImageId,
        source: ViewImageSource,
    ) -> Result<(), ViewError> {
        if self.sources.insert(id, source).is_some() {
            return Err(ViewError::DuplicateImageSource(id));
        }
        self.next = self.next.max(id.0.saturating_add(1));
        Ok(())
    }

    pub fn get(&self, id: ImageId) -> Option<&ViewImageSource> {
        self.sources.get(&id)
    }

    pub fn resolve_frame(
        &self,
        id: ImageId,
        layout: LayoutBox,
        visual_time_millis: u64,
    ) -> Result<ViewResolvedImageFrame<'_>, ViewError> {
        let source = self.get(id).ok_or(ViewError::UnknownImageSource(id))?;
        let frame = source
            .frame_at_time(visual_time_millis)
            .ok_or(ViewError::UnknownImageSource(id))?;
        Ok(ViewResolvedImageFrame {
            source: id,
            frame,
            fit: source.fit,
            alignment: source.alignment,
            opacity_milli: source.opacity_milli,
            transform: source.transform,
            layout,
        })
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}
