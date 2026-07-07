use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_scene::{UiAffine2D, UiBlendMode, UiIsolation, UiPrimitiveRange};
use arcweft_takumi_adapter::{
    ArcweftNodeMetadata, COMPOSITING_EVIDENCE_SCHEMA_VERSION, TakumiCaptureFrame,
    TakumiCaptureRecord, TakumiCompositingCaptureRecord, TakumiCompositingGroupId,
    TakumiEffectOutsets, TakumiPaintNodeId,
};
use arcweft_view::{ContainerKind, FragmentKind, HandlerId, NodeId, NodeKey, StyleId};

fn metadata(node: u32, key: u64) -> ArcweftNodeMetadata {
    ArcweftNodeMetadata::new(
        NodeId(node),
        NodeKey(key),
        FragmentKind::Container(ContainerKind::Block),
        StyleId(7),
        [HandlerId(9)],
        None,
    )
}

#[test]
fn schema_surface_exposes_layout_visual_hit_clip_mask_and_effect_bounds() {
    let object = TakumiCaptureRecord::new(
        metadata(1, 10),
        UiPrimitiveRange { start: 0, end: 2 },
        HitRect::new(10.0, 20.0, 30.0, 40.0),
        UiAffine2D::IDENTITY,
        None,
    )
    .with_paint_node_id(TakumiPaintNodeId::new(2))
    .with_compositing_group_id(TakumiCompositingGroupId::new(1));

    assert_eq!(object.layout_bounds(), HitRect::new(10.0, 20.0, 30.0, 40.0));
    assert_eq!(object.visual_bounds(), object.layout_bounds());
    assert_eq!(object.hit_bounds(), object.layout_bounds());
    assert_eq!(object.paint_node_id(), Some(TakumiPaintNodeId::new(2)));

    let group = TakumiCompositingCaptureRecord::new(
        metadata(2, 20),
        TakumiCompositingGroupId::new(1),
        TakumiPaintNodeId::new(1),
        HitRect::new(0.0, 0.0, 100.0, 60.0),
        HitRect::new(0.0, 0.0, 100.0, 60.0),
    )
    .with_effect_outsets(TakumiEffectOutsets::new(18.0, 6.0, 0.0))
    .with_hit_bounds(HitRect::new(0.0, 0.0, 100.0, 60.0))
    .with_clip_bounds(Some(HitRect::new(4.0, 4.0, 92.0, 52.0)))
    .with_mask_bounds([HitRect::new(0.0, 0.0, 100.0, 60.0)])
    .with_isolation(UiIsolation::Isolate)
    .with_blend_mode(UiBlendMode::Multiply);

    assert_eq!(
        group.visual_bounds(),
        HitRect::new(-18.0, -18.0, 136.0, 96.0)
    );
    assert_eq!(group.hit_bounds(), HitRect::new(0.0, 0.0, 100.0, 60.0));
    assert_eq!(
        group.clip_bounds(),
        Some(HitRect::new(4.0, 4.0, 92.0, 52.0))
    );
    assert_eq!(group.mask_bounds(), &[HitRect::new(0.0, 0.0, 100.0, 60.0)]);

    let mut frame = TakumiCaptureFrame::default();
    frame.push(object);
    frame.push_compositing_group(group);

    let evidence = frame.evidence_json();
    assert!(evidence.contains(COMPOSITING_EVIDENCE_SCHEMA_VERSION));
    assert!(evidence.contains("\"record_kind\": \"object\""));
    assert!(evidence.contains("\"record_kind\": \"compositing_group\""));
    assert!(evidence.contains("\"visual_bounds\""));
    assert!(evidence.contains("\"hit_bounds\""));
    assert!(evidence.contains("\"mask_bounds\""));
    assert!(evidence.contains("\"effect_outsets\""));
}
