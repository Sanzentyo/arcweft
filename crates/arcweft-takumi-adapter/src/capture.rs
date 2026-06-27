use crate::metadata::ArcweftNodeMetadata;
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_scene::{UiAffine2, UiClip, UiPrimitiveRange};

#[derive(Clone, Debug, PartialEq)]
pub struct TakumiCaptureRecord {
    metadata: ArcweftNodeMetadata,
    primitive_range: UiPrimitiveRange,
    local_bounds: HitRect,
    transform: UiAffine2,
    clip: Option<UiClip>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TakumiCaptureFrame {
    records: Vec<TakumiCaptureRecord>,
}

impl TakumiCaptureRecord {
    pub fn new(
        metadata: ArcweftNodeMetadata,
        primitive_range: UiPrimitiveRange,
        local_bounds: HitRect,
        transform: UiAffine2,
        clip: Option<UiClip>,
    ) -> Self {
        Self {
            metadata,
            primitive_range,
            local_bounds,
            transform,
            clip,
        }
    }

    pub fn metadata(&self) -> &ArcweftNodeMetadata {
        &self.metadata
    }

    pub fn primitive_range(&self) -> UiPrimitiveRange {
        self.primitive_range
    }

    pub fn local_bounds(&self) -> HitRect {
        self.local_bounds
    }

    pub fn transform(&self) -> UiAffine2 {
        self.transform
    }

    pub fn clip(&self) -> Option<&UiClip> {
        self.clip.as_ref()
    }
}

impl TakumiCaptureFrame {
    pub fn push(&mut self, record: TakumiCaptureRecord) {
        self.records.push(record);
    }

    pub fn records(&self) -> &[TakumiCaptureRecord] {
        &self.records
    }
}
