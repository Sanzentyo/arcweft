use crate::hit::HitRect;
use crate::input::InteractionTarget;
use crate::layer::LayerId;
use crate::semantic::{SemanticNode, SemanticRole};
use arcweft_id::PublicId;
use std::collections::BTreeMap;

/// Stable presentation object id for an image-like visual object.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ImageObjectId {
    id: PublicId,
}

/// Encoded image asset reference before adapter-side byte lookup and decode.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ImageAssetRef {
    id: PublicId,
}

/// How an image's intrinsic pixels map into its authored presentation bounds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ImageObjectFit {
    #[default]
    Contain,
    Cover,
    Stretch,
    Intrinsic,
}

/// Alignment inside the authored presentation bounds, expressed in thousandths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageObjectAlignment {
    x_milli: i32,
    y_milli: i32,
}

/// Deterministic playback policy for static and animated image objects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageObjectPlayback {
    start_time_millis: u64,
    paused_at_millis: Option<u64>,
    rate_milli: u32,
    pinned_local_time_millis: Option<u64>,
}

/// Fixed-point object transform in layer-local coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageObjectTransform {
    pub m11_milli: i32,
    pub m12_milli: i32,
    pub m21_milli: i32,
    pub m22_milli: i32,
    pub tx_milli: i32,
    pub ty_milli: i32,
}

/// Typed custom parameter attached to an image presentation object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageObjectParam {
    Bool(bool),
    Integer(i64),
    Milli(i32),
    Text(String),
    Id(PublicId),
}

/// First-class image presentation object shared by render, hit-test, and Agent observation.
#[derive(Clone, Debug, PartialEq)]
pub struct ImagePresentationObject {
    id: ImageObjectId,
    asset: ImageAssetRef,
    layer: LayerId,
    target: InteractionTarget,
    bounds: HitRect,
    fit: ImageObjectFit,
    alignment: ImageObjectAlignment,
    opacity_milli: u16,
    depth_milli: i32,
    playback: ImageObjectPlayback,
    transform: ImageObjectTransform,
    params: BTreeMap<PublicId, ImageObjectParam>,
    actions: Vec<PublicId>,
    enabled: bool,
    visible: bool,
}

impl ImageObjectId {
    pub const fn new(id: PublicId) -> Self {
        Self { id }
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.id
    }
}

impl ImageAssetRef {
    pub const fn new(id: PublicId) -> Self {
        Self { id }
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.id
    }
}

impl ImageObjectAlignment {
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

impl Default for ImageObjectAlignment {
    fn default() -> Self {
        Self::center()
    }
}

impl ImageObjectPlayback {
    pub const fn new(start_time_millis: u64) -> Self {
        Self {
            start_time_millis,
            paused_at_millis: None,
            rate_milli: 1_000,
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
        elapsed.saturating_mul(u64::from(self.rate_milli)) / 1_000
    }
}

impl ImageObjectTransform {
    pub const fn identity() -> Self {
        Self {
            m11_milli: 1_000,
            m12_milli: 0,
            m21_milli: 0,
            m22_milli: 1_000,
            tx_milli: 0,
            ty_milli: 0,
        }
    }

    pub const fn translation_milli(
        horizontal_offset_milli: i32,
        vertical_offset_milli: i32,
    ) -> Self {
        Self {
            tx_milli: horizontal_offset_milli,
            ty_milli: vertical_offset_milli,
            ..Self::identity()
        }
    }
}

impl ImagePresentationObject {
    pub fn new(
        id: ImageObjectId,
        asset: ImageAssetRef,
        layer: LayerId,
        target: InteractionTarget,
        bounds: HitRect,
    ) -> Self {
        Self {
            id,
            asset,
            layer,
            target,
            bounds,
            fit: ImageObjectFit::default(),
            alignment: ImageObjectAlignment::default(),
            opacity_milli: 1_000,
            depth_milli: 0,
            playback: ImageObjectPlayback::new(0),
            transform: ImageObjectTransform::identity(),
            params: BTreeMap::new(),
            actions: Vec::new(),
            enabled: true,
            visible: true,
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
    pub const fn with_depth_milli(mut self, depth_milli: i32) -> Self {
        self.depth_milli = depth_milli;
        self
    }

    #[must_use]
    pub const fn with_playback(mut self, playback: ImageObjectPlayback) -> Self {
        self.playback = playback;
        self
    }

    #[must_use]
    pub const fn with_transform(mut self, transform: ImageObjectTransform) -> Self {
        self.transform = transform;
        self
    }

    #[must_use]
    pub fn with_param(mut self, key: PublicId, value: ImageObjectParam) -> Self {
        self.params.insert(key, value);
        self
    }

    #[must_use]
    pub fn with_action(mut self, action: PublicId) -> Self {
        self.actions.push(action);
        self
    }

    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub const fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub const fn id(&self) -> &ImageObjectId {
        &self.id
    }

    pub const fn asset(&self) -> &ImageAssetRef {
        &self.asset
    }

    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn bounds(&self) -> HitRect {
        self.bounds
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

    pub const fn depth_milli(&self) -> i32 {
        self.depth_milli
    }

    pub const fn playback(&self) -> ImageObjectPlayback {
        self.playback
    }

    pub const fn transform(&self) -> ImageObjectTransform {
        self.transform
    }

    pub fn params(&self) -> &BTreeMap<PublicId, ImageObjectParam> {
        &self.params
    }

    pub fn actions(&self) -> &[PublicId] {
        &self.actions
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn visible(&self) -> bool {
        self.visible
    }

    pub fn sample_time_millis(&self, visual_time_millis: u64) -> u64 {
        self.playback.local_time_millis(visual_time_millis)
    }

    pub fn semantic_node(&self) -> SemanticNode {
        self.actions.iter().cloned().fold(
            SemanticNode::new(
                self.layer.clone(),
                self.target.clone(),
                SemanticRole::Image,
                self.bounds,
            )
            .with_enabled(self.enabled)
            .with_visible(self.visible),
            SemanticNode::with_action,
        )
    }
}
