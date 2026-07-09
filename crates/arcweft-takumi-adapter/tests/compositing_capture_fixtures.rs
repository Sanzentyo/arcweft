use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_scene::{ViewBlendMode, ViewIsolation};
use arcweft_takumi_adapter::{
    ArcweftNodeMetadata, TakumiCaptureFrame, TakumiCompositingCaptureRecord,
    TakumiCompositingGroupId, TakumiEffectOutsets, TakumiPaintNodeId,
};
use arcweft_view::{ContainerKind, FragmentKind, HandlerId, NodeId, NodeKey, StyleId};

const EXPECTED_EVIDENCE: &str = include_str!("fixtures/compositing-capture/expected-evidence.json");

fn metadata() -> ArcweftNodeMetadata {
    ArcweftNodeMetadata::new(
        NodeId(100),
        NodeKey(9001),
        FragmentKind::Container(ContainerKind::Stack),
        StyleId(22),
        [HandlerId(77)],
        None,
    )
}

#[test]
fn fixture_generation_keeps_stable_ids_and_effect_metadata() {
    let mut frame = TakumiCaptureFrame::default();
    frame.push_compositing_group(
        TakumiCompositingCaptureRecord::new(
            metadata(),
            TakumiCompositingGroupId::new(1),
            TakumiPaintNodeId::new(1),
            HitRect::new(64.0, 48.0, 256.0, 144.0),
            HitRect::new(64.0, 48.0, 256.0, 144.0),
        )
        .with_effect_outsets(TakumiEffectOutsets::new(18.0, 9.0, 0.0))
        .with_clip_bounds(Some(HitRect::new(76.0, 60.0, 232.0, 120.0)))
        .with_mask_bounds([HitRect::new(64.0, 48.0, 256.0, 144.0)])
        .with_isolation(ViewIsolation::Isolate)
        .with_blend_mode(ViewBlendMode::Multiply),
    );

    assert_eq!(frame.evidence_json(), EXPECTED_EVIDENCE);
}
