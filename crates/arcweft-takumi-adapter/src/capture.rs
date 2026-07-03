use crate::metadata::ArcweftNodeMetadata;
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_scene::{
    UiAffine2D, UiBlendMode, UiClip, UiIsolation, UiPrimitiveRange,
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TakumiPaintNodeId(pub u32);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TakumiCompositingGroupId(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TakumiEffectOutsets {
    pub filter_px: f32,
    pub backdrop_filter_px: f32,
    pub mask_px: f32,
    pub total_px: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TakumiCaptureRecord {
    metadata: ArcweftNodeMetadata,
    primitive_range: UiPrimitiveRange,
    local_bounds: HitRect,
    layout_bounds: HitRect,
    visual_bounds: HitRect,
    hit_bounds: HitRect,
    clip_bounds: Option<HitRect>,
    mask_bounds: Vec<HitRect>,
    effect_outsets: TakumiEffectOutsets,
    transform: UiAffine2D,
    clip: Option<UiClip>,
    compositing_group_id: Option<TakumiCompositingGroupId>,
    paint_node_id: Option<TakumiPaintNodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TakumiCompositingCaptureRecord {
    metadata: ArcweftNodeMetadata,
    primitive_range: Option<UiPrimitiveRange>,
    layout_bounds: HitRect,
    visual_bounds: HitRect,
    hit_bounds: HitRect,
    clip_bounds: Option<HitRect>,
    mask_bounds: Vec<HitRect>,
    effect_outsets: TakumiEffectOutsets,
    compositing_group_id: TakumiCompositingGroupId,
    paint_node_id: TakumiPaintNodeId,
    isolation: UiIsolation,
    blend_mode: UiBlendMode,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TakumiCaptureFrame {
    records: Vec<TakumiCaptureRecord>,
    compositing_records: Vec<TakumiCompositingCaptureRecord>,
}

impl TakumiPaintNodeId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TakumiCompositingGroupId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TakumiEffectOutsets {
    pub fn new(filter_px: f32, backdrop_filter_px: f32, mask_px: f32) -> Self {
        let filter_px = positive_px(filter_px);
        let backdrop_filter_px = positive_px(backdrop_filter_px);
        let mask_px = positive_px(mask_px);
        Self {
            filter_px,
            backdrop_filter_px,
            mask_px,
            total_px: filter_px.max(backdrop_filter_px).max(mask_px),
        }
    }

    pub const fn none() -> Self {
        Self {
            filter_px: 0.0,
            backdrop_filter_px: 0.0,
            mask_px: 0.0,
            total_px: 0.0,
        }
    }
}

impl TakumiCaptureRecord {
    pub fn new(
        metadata: ArcweftNodeMetadata,
        primitive_range: UiPrimitiveRange,
        local_bounds: HitRect,
        transform: UiAffine2D,
        clip: Option<UiClip>,
    ) -> Self {
        let clip_bounds = clip.as_ref().map(ui_clip_bounds);
        Self {
            metadata,
            primitive_range,
            local_bounds,
            layout_bounds: local_bounds,
            visual_bounds: local_bounds,
            hit_bounds: local_bounds,
            clip_bounds,
            mask_bounds: Vec::new(),
            effect_outsets: TakumiEffectOutsets::none(),
            transform,
            clip,
            compositing_group_id: None,
            paint_node_id: None,
        }
    }

    #[must_use]
    pub fn with_layout_bounds(mut self, bounds: HitRect) -> Self {
        self.layout_bounds = bounds;
        self
    }

    #[must_use]
    pub fn with_visual_bounds(mut self, bounds: HitRect) -> Self {
        self.visual_bounds = bounds;
        self
    }

    #[must_use]
    pub fn with_hit_bounds(mut self, bounds: HitRect) -> Self {
        self.hit_bounds = bounds;
        self
    }

    #[must_use]
    pub fn with_clip_bounds(mut self, bounds: Option<HitRect>) -> Self {
        self.clip_bounds = bounds;
        self
    }

    #[must_use]
    pub fn with_mask_bounds(mut self, bounds: impl IntoIterator<Item = HitRect>) -> Self {
        self.mask_bounds = bounds.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_effect_outsets(mut self, outsets: TakumiEffectOutsets) -> Self {
        self.effect_outsets = outsets;
        self.visual_bounds = self.layout_bounds.outset(outsets.total_px);
        self
    }

    #[must_use]
    pub const fn with_compositing_group_id(mut self, id: TakumiCompositingGroupId) -> Self {
        self.compositing_group_id = Some(id);
        self
    }

    #[must_use]
    pub const fn with_paint_node_id(mut self, id: TakumiPaintNodeId) -> Self {
        self.paint_node_id = Some(id);
        self
    }

    pub fn metadata(&self) -> &ArcweftNodeMetadata {
        &self.metadata
    }

    pub const fn primitive_range(&self) -> UiPrimitiveRange {
        self.primitive_range
    }

    pub const fn local_bounds(&self) -> HitRect {
        self.local_bounds
    }

    pub const fn layout_bounds(&self) -> HitRect {
        self.layout_bounds
    }

    pub const fn visual_bounds(&self) -> HitRect {
        self.visual_bounds
    }

    pub const fn hit_bounds(&self) -> HitRect {
        self.hit_bounds
    }

    pub const fn clip_bounds(&self) -> Option<HitRect> {
        self.clip_bounds
    }

    pub fn mask_bounds(&self) -> &[HitRect] {
        &self.mask_bounds
    }

    pub const fn effect_outsets(&self) -> TakumiEffectOutsets {
        self.effect_outsets
    }

    pub const fn transform(&self) -> UiAffine2D {
        self.transform
    }

    pub fn clip(&self) -> Option<&UiClip> {
        self.clip.as_ref()
    }

    pub const fn compositing_group_id(&self) -> Option<TakumiCompositingGroupId> {
        self.compositing_group_id
    }

    pub const fn paint_node_id(&self) -> Option<TakumiPaintNodeId> {
        self.paint_node_id
    }
}

impl TakumiCompositingCaptureRecord {
    pub fn new(
        metadata: ArcweftNodeMetadata,
        compositing_group_id: TakumiCompositingGroupId,
        paint_node_id: TakumiPaintNodeId,
        layout_bounds: HitRect,
        visual_bounds: HitRect,
    ) -> Self {
        Self {
            metadata,
            primitive_range: None,
            layout_bounds,
            visual_bounds,
            hit_bounds: layout_bounds,
            clip_bounds: None,
            mask_bounds: Vec::new(),
            effect_outsets: TakumiEffectOutsets::none(),
            compositing_group_id,
            paint_node_id,
            isolation: UiIsolation::Auto,
            blend_mode: UiBlendMode::Normal,
        }
    }

    #[must_use]
    pub const fn with_primitive_range(mut self, range: Option<UiPrimitiveRange>) -> Self {
        self.primitive_range = range;
        self
    }

    #[must_use]
    pub const fn with_hit_bounds(mut self, bounds: HitRect) -> Self {
        self.hit_bounds = bounds;
        self
    }

    #[must_use]
    pub const fn with_clip_bounds(mut self, bounds: Option<HitRect>) -> Self {
        self.clip_bounds = bounds;
        self
    }

    #[must_use]
    pub fn with_mask_bounds(mut self, bounds: impl IntoIterator<Item = HitRect>) -> Self {
        self.mask_bounds = bounds.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_effect_outsets(mut self, outsets: TakumiEffectOutsets) -> Self {
        self.effect_outsets = outsets;
        self.visual_bounds = self.layout_bounds.outset(outsets.total_px);
        self
    }

    #[must_use]
    pub const fn with_isolation(mut self, isolation: UiIsolation) -> Self {
        self.isolation = isolation;
        self
    }

    #[must_use]
    pub const fn with_blend_mode(mut self, blend_mode: UiBlendMode) -> Self {
        self.blend_mode = blend_mode;
        self
    }

    pub fn metadata(&self) -> &ArcweftNodeMetadata {
        &self.metadata
    }

    pub const fn primitive_range(&self) -> Option<UiPrimitiveRange> {
        self.primitive_range
    }

    pub const fn layout_bounds(&self) -> HitRect {
        self.layout_bounds
    }

    pub const fn visual_bounds(&self) -> HitRect {
        self.visual_bounds
    }

    pub const fn hit_bounds(&self) -> HitRect {
        self.hit_bounds
    }

    pub const fn clip_bounds(&self) -> Option<HitRect> {
        self.clip_bounds
    }

    pub fn mask_bounds(&self) -> &[HitRect] {
        &self.mask_bounds
    }

    pub const fn effect_outsets(&self) -> TakumiEffectOutsets {
        self.effect_outsets
    }

    pub const fn compositing_group_id(&self) -> TakumiCompositingGroupId {
        self.compositing_group_id
    }

    pub const fn paint_node_id(&self) -> TakumiPaintNodeId {
        self.paint_node_id
    }

    pub const fn isolation(&self) -> UiIsolation {
        self.isolation
    }

    pub const fn blend_mode(&self) -> UiBlendMode {
        self.blend_mode
    }
}

impl TakumiCaptureFrame {
    pub fn push(&mut self, record: TakumiCaptureRecord) {
        self.records.push(record);
    }

    pub fn push_compositing_group(&mut self, record: TakumiCompositingCaptureRecord) {
        self.compositing_records.push(record);
    }

    pub fn records(&self) -> &[TakumiCaptureRecord] {
        &self.records
    }

    pub fn compositing_records(&self) -> &[TakumiCompositingCaptureRecord] {
        &self.compositing_records
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.compositing_records.is_empty()
    }

    pub fn evidence_json(&self) -> String {
        crate::evidence::capture_frame_to_json(self)
    }
}

pub(crate) fn ui_clip_bounds(clip: &UiClip) -> HitRect {
    match clip {
        UiClip::Rect(bounds) | UiClip::RoundedRect { bounds, .. } => *bounds,
    }
}

fn positive_px(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_render_wgpu::ui_scene::UiColorRgba8;
    use arcweft_ui::{ContainerKind, FragmentKind, HandlerId, NodeId, NodeKey, StyleId};

    fn metadata() -> ArcweftNodeMetadata {
        ArcweftNodeMetadata::new(
            NodeId(2),
            NodeKey(7),
            FragmentKind::Container(ContainerKind::Block),
            StyleId(3),
            [HandlerId(11)],
            None,
        )
    }

    #[test]
    fn direct_record_defaults_keep_visual_and_hit_bounds_separate_from_future_effects() {
        let record = TakumiCaptureRecord::new(
            metadata(),
            UiPrimitiveRange { start: 4, end: 9 },
            HitRect::new(10.0, 20.0, 30.0, 40.0),
            UiAffine2D::IDENTITY,
            Some(UiClip::Rect(HitRect::new(12.0, 22.0, 20.0, 30.0))),
        )
        .with_paint_node_id(TakumiPaintNodeId::new(1))
        .with_compositing_group_id(TakumiCompositingGroupId::new(10));

        assert_eq!(record.layout_bounds(), HitRect::new(10.0, 20.0, 30.0, 40.0));
        assert_eq!(record.visual_bounds(), record.layout_bounds());
        assert_eq!(record.hit_bounds(), record.layout_bounds());
        assert_eq!(
            record.clip_bounds(),
            Some(HitRect::new(12.0, 22.0, 20.0, 30.0))
        );
        assert_eq!(record.paint_node_id(), Some(TakumiPaintNodeId::new(1)));
        assert_eq!(
            record.compositing_group_id(),
            Some(TakumiCompositingGroupId::new(10))
        );
    }

    #[test]
    fn group_record_expands_visual_bounds_without_expanding_hit_bounds() {
        let outsets = TakumiEffectOutsets::new(18.0, 0.0, 0.0);
        let record = TakumiCompositingCaptureRecord::new(
            metadata(),
            TakumiCompositingGroupId::new(4),
            TakumiPaintNodeId::new(3),
            HitRect::new(100.0, 50.0, 80.0, 40.0),
            HitRect::new(100.0, 50.0, 80.0, 40.0),
        )
        .with_effect_outsets(outsets)
        .with_mask_bounds([HitRect::new(100.0, 50.0, 80.0, 40.0)])
        .with_blend_mode(UiBlendMode::Multiply)
        .with_isolation(UiIsolation::Isolate);

        assert_eq!(record.hit_bounds(), HitRect::new(100.0, 50.0, 80.0, 40.0));
        assert_eq!(
            record.visual_bounds(),
            HitRect::new(82.0, 32.0, 116.0, 76.0)
        );
        assert_eq!(
            record.mask_bounds(),
            &[HitRect::new(100.0, 50.0, 80.0, 40.0)]
        );
        assert!((record.effect_outsets().filter_px - 18.0).abs() <= f32::EPSILON);
        assert_eq!(record.blend_mode(), UiBlendMode::Multiply);
        assert_eq!(record.isolation(), UiIsolation::Isolate);
    }

    #[test]
    fn finite_outset_constructor_sanitizes_non_finite_values() {
        let outsets = TakumiEffectOutsets::new(f32::NAN, f32::INFINITY, -1.0);
        assert_eq!(outsets, TakumiEffectOutsets::none());

        let shadow_color = UiColorRgba8 {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 128,
        };
        assert_eq!(shadow_color.alpha, 128);
    }
}
