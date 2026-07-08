//! View image source table and deterministic animated-frame selection.

use crate::{ImageId, LayoutBox, ViewError};
use arcweft_id::PublicId;
use arcweft_image::{DecodedImage, DecodedImageFrame};
use arcweft_presentation::image::{
    ImageObjectAlignment, ImageObjectFit, ImageObjectParam, ImageObjectPlayback, ImageObjectProxy,
    ImageObjectTransform,
};
use std::collections::BTreeMap;

/// How an image's intrinsic pixels map into a layout box.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ImageFit {
    /// Preserve aspect ratio and fit entirely inside the destination box.
    #[default]
    Contain,
    /// Preserve aspect ratio and cover the destination box, clipping overflow.
    Cover,
    /// Stretch to exactly fill the destination box.
    Stretch,
    /// Use the intrinsic pixel size.
    Intrinsic,
}

/// Alignment inside the destination box, expressed in thousandths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageAlignment {
    x_milli: i32,
    y_milli: i32,
}

/// Playback state for static and animated images.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImagePlayback {
    start_time_millis: u64,
    paused_at_millis: Option<u64>,
    rate_milli: u32,
    pinned_local_time_millis: Option<u64>,
}

/// Image source registered outside the retained fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewImageSource {
    image: DecodedImage,
    fit: ImageFit,
    alignment: ImageAlignment,
    opacity_milli: u16,
    transform: ImageObjectTransform,
    playback: ImagePlayback,
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
    fit: ImageFit,
    alignment: ImageAlignment,
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

impl From<ImageObjectFit> for ImageFit {
    fn from(value: ImageObjectFit) -> Self {
        match value {
            ImageObjectFit::Contain => Self::Contain,
            ImageObjectFit::Cover => Self::Cover,
            ImageObjectFit::Stretch => Self::Stretch,
            ImageObjectFit::Intrinsic => Self::Intrinsic,
        }
    }
}

impl ImageAlignment {
    pub const fn new(x_milli: i32, y_milli: i32) -> Self {
        Self { x_milli, y_milli }
    }

    pub const fn center() -> Self {
        Self::new(500, 500)
    }

    pub const fn top_left() -> Self {
        Self::new(0, 0)
    }

    pub const fn x_milli(self) -> i32 {
        self.x_milli
    }

    pub const fn y_milli(self) -> i32 {
        self.y_milli
    }
}

impl From<ImageObjectAlignment> for ImageAlignment {
    fn from(value: ImageObjectAlignment) -> Self {
        Self::new(value.x_milli(), value.y_milli())
    }
}

impl Default for ImageAlignment {
    fn default() -> Self {
        Self::center()
    }
}

impl ImagePlayback {
    pub const fn new(start_time_millis: u64) -> Self {
        Self {
            start_time_millis,
            paused_at_millis: None,
            rate_milli: 1000,
            pinned_local_time_millis: None,
        }
    }

    #[must_use]
    pub const fn paused_at(mut self, visual_time_millis: u64) -> Self {
        self.paused_at_millis = Some(visual_time_millis);
        self
    }

    #[must_use]
    pub const fn with_rate_milli(mut self, rate_milli: u32) -> Self {
        self.rate_milli = rate_milli;
        self
    }

    #[must_use]
    pub const fn pinned_local_time(mut self, local_time_millis: u64) -> Self {
        self.pinned_local_time_millis = Some(local_time_millis);
        self
    }

    pub const fn start_time_millis(self) -> u64 {
        self.start_time_millis
    }

    pub const fn paused_at_millis(self) -> Option<u64> {
        self.paused_at_millis
    }

    pub const fn rate_milli(self) -> u32 {
        self.rate_milli
    }

    pub const fn pinned_local_time_millis(self) -> Option<u64> {
        self.pinned_local_time_millis
    }

    pub fn local_time_millis(self, visual_time_millis: u64) -> u64 {
        if let Some(pinned) = self.pinned_local_time_millis {
            return pinned;
        }
        let sample_time = self.paused_at_millis.unwrap_or(visual_time_millis);
        let elapsed = sample_time.saturating_sub(self.start_time_millis);
        if self.rate_milli == 0 {
            return 0;
        }
        elapsed.saturating_mul(u64::from(self.rate_milli)) / 1000
    }
}

impl From<ImageObjectPlayback> for ImagePlayback {
    fn from(value: ImageObjectPlayback) -> Self {
        let mut playback = Self::new(value.start_time_millis()).with_rate_milli(value.rate_milli());
        if let Some(paused_at) = value.paused_at_millis() {
            playback = playback.paused_at(paused_at);
        }
        if let Some(pinned) = value.pinned_local_time_millis() {
            playback = playback.pinned_local_time(pinned);
        }
        playback
    }
}

impl ViewImageSource {
    pub fn new(image: DecodedImage) -> Self {
        Self {
            image,
            fit: ImageFit::default(),
            alignment: ImageAlignment::default(),
            opacity_milli: 1_000,
            transform: ImageObjectTransform::identity(),
            playback: ImagePlayback::new(0),
            presentation: None,
        }
    }

    #[must_use]
    pub const fn with_fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    #[must_use]
    pub const fn with_alignment(mut self, alignment: ImageAlignment) -> Self {
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
    pub const fn with_playback(mut self, playback: ImagePlayback) -> Self {
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

    pub const fn fit(&self) -> ImageFit {
        self.fit
    }

    pub const fn alignment(&self) -> ImageAlignment {
        self.alignment
    }

    pub const fn opacity_milli(&self) -> u16 {
        self.opacity_milli
    }

    pub const fn transform(&self) -> ImageObjectTransform {
        self.transform
    }

    pub const fn playback(&self) -> ImagePlayback {
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

    pub const fn fit(self) -> ImageFit {
        self.fit
    }

    pub const fn alignment(self) -> ImageAlignment {
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
