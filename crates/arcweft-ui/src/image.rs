//! UI image source table and deterministic animated-frame selection.

use crate::{ImageId, LayoutBox, UiError};
use arcweft_image::{DecodedImage, DecodedImageFrame};
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
}

/// Image source registered outside the retained fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiImageSource {
    image: DecodedImage,
    fit: ImageFit,
    alignment: ImageAlignment,
    playback: ImagePlayback,
}

/// Resolved image frame ready for renderer submission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiResolvedImageFrame<'a> {
    source: ImageId,
    frame: &'a DecodedImageFrame,
    fit: ImageFit,
    alignment: ImageAlignment,
    layout: LayoutBox,
}

/// Dense UI image source registry keyed by `ImageId`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiImageSourceTable {
    sources: BTreeMap<ImageId, UiImageSource>,
    next: u32,
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

    pub const fn start_time_millis(self) -> u64 {
        self.start_time_millis
    }

    pub const fn paused_at_millis(self) -> Option<u64> {
        self.paused_at_millis
    }

    pub const fn rate_milli(self) -> u32 {
        self.rate_milli
    }

    pub fn local_time_millis(self, visual_time_millis: u64) -> u64 {
        let sample_time = self.paused_at_millis.unwrap_or(visual_time_millis);
        let elapsed = sample_time.saturating_sub(self.start_time_millis);
        if self.rate_milli == 0 {
            return 0;
        }
        elapsed.saturating_mul(u64::from(self.rate_milli)) / 1000
    }
}

impl UiImageSource {
    pub fn new(image: DecodedImage) -> Self {
        Self {
            image,
            fit: ImageFit::default(),
            alignment: ImageAlignment::default(),
            playback: ImagePlayback::new(0),
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
    pub const fn with_playback(mut self, playback: ImagePlayback) -> Self {
        self.playback = playback;
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

    pub const fn playback(&self) -> ImagePlayback {
        self.playback
    }

    pub fn frame_at_time(&self, visual_time_millis: u64) -> Option<&DecodedImageFrame> {
        self.image
            .frame_at_time_millis(self.playback.local_time_millis(visual_time_millis))
    }
}

impl<'a> UiResolvedImageFrame<'a> {
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

    pub const fn layout(self) -> LayoutBox {
        self.layout
    }
}

impl UiImageSourceTable {
    pub fn insert(&mut self, source: UiImageSource) -> Result<ImageId, UiError> {
        let id = ImageId(self.next);
        self.next = self.next.checked_add(1).ok_or(UiError::CapacityExceeded)?;
        self.insert_with_id(id, source)?;
        Ok(id)
    }

    pub fn insert_with_id(&mut self, id: ImageId, source: UiImageSource) -> Result<(), UiError> {
        if self.sources.insert(id, source).is_some() {
            return Err(UiError::DuplicateImageSource(id));
        }
        self.next = self.next.max(id.0.saturating_add(1));
        Ok(())
    }

    pub fn get(&self, id: ImageId) -> Option<&UiImageSource> {
        self.sources.get(&id)
    }

    pub fn resolve_frame(
        &self,
        id: ImageId,
        layout: LayoutBox,
        visual_time_millis: u64,
    ) -> Result<UiResolvedImageFrame<'_>, UiError> {
        let source = self.get(id).ok_or(UiError::UnknownImageSource(id))?;
        let frame = source
            .frame_at_time(visual_time_millis)
            .ok_or(UiError::UnknownImageSource(id))?;
        Ok(UiResolvedImageFrame {
            source: id,
            frame,
            fit: source.fit,
            alignment: source.alignment,
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
