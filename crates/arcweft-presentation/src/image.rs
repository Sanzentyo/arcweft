use arcweft_interaction_model::presentation::{
    ImageObjectAlignment, ImageObjectFit, ImageObjectPlayback, ImageObjectTransform,
};

use crate::hit::HitRect;
use crate::input::InteractionTarget;
use crate::layer::LayerId;
use crate::semantic::{SemanticNode, SemanticRole};
use arcweft_id::PublicId;
use std::collections::BTreeMap;

/// Stable presentation object id for an image-like visual object.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImageObjectId {
    id: PublicId,
}

/// Encoded image asset reference before adapter-side byte lookup and decode.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ImageAssetRef {
    id: PublicId,
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

/// Typed proxy metadata attached to an image presentation object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageObjectProxy {
    id: PublicId,
    type_name: Option<String>,
    role: Option<String>,
    layer: Option<PublicId>,
    depth_milli: Option<i32>,
    hit_test: bool,
    params: BTreeMap<PublicId, ImageObjectParam>,
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
    proxies: Vec<ImageObjectProxy>,
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

impl ImageObjectProxy {
    pub fn new(id: PublicId) -> Self {
        Self {
            id,
            type_name: None,
            role: None,
            layer: None,
            depth_milli: None,
            hit_test: false,
            params: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_type_name(mut self, type_name: impl Into<String>) -> Self {
        self.type_name = Some(type_name.into());
        self
    }

    #[must_use]
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    #[must_use]
    pub fn with_layer(mut self, layer: PublicId) -> Self {
        self.layer = Some(layer);
        self
    }

    #[must_use]
    pub const fn with_depth_milli(mut self, depth_milli: i32) -> Self {
        self.depth_milli = Some(depth_milli);
        self
    }

    #[must_use]
    pub const fn with_hit_test(mut self, hit_test: bool) -> Self {
        self.hit_test = hit_test;
        self
    }

    #[must_use]
    pub fn with_param(mut self, key: PublicId, value: ImageObjectParam) -> Self {
        self.params.insert(key, value);
        self
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }

    pub fn type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }

    pub fn role(&self) -> Option<&str> {
        self.role.as_deref()
    }

    pub const fn layer(&self) -> Option<&PublicId> {
        self.layer.as_ref()
    }

    pub const fn depth_milli(&self) -> Option<i32> {
        self.depth_milli
    }

    pub const fn hit_test(&self) -> bool {
        self.hit_test
    }

    pub fn params(&self) -> &BTreeMap<PublicId, ImageObjectParam> {
        &self.params
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
            proxies: Vec::new(),
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
    pub fn with_proxy(mut self, proxy: ImageObjectProxy) -> Self {
        self.proxies.push(proxy);
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

    pub fn proxies(&self) -> &[ImageObjectProxy] {
        &self.proxies
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
