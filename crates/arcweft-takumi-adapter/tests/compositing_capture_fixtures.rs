use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::ui_scene::{UiBlendMode, UiIsolation};
use arcweft_takumi_adapter::{
    ArcweftNodeMetadata, TakumiCaptureFrame, TakumiCompositingCaptureRecord,
    TakumiCompositingGroupId, TakumiEffectOutsets, TakumiPaintNodeId,
};
use arcweft_ui::{ContainerKind, FragmentKind, HandlerId, NodeId, NodeKey, StyleId};

const EXPECTED_EVIDENCE: &str = include_str!("fixtures/compositing-capture/expected-evidence.json");
const SCENE_CSS: &str = include_str!("fixtures/compositing-capture/scene.css");

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
fn fixture_css_covers_all_seq069c_compositing_families() {
    for required in [
        "filter:",
        "backdrop-filter:",
        "mask-image:",
        "clip-path:",
        "mix-blend-mode:",
    ] {
        assert!(
            SCENE_CSS.contains(required),
            "fixture CSS is missing {required}"
        );
    }
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
        .with_isolation(UiIsolation::Isolate)
        .with_blend_mode(UiBlendMode::Multiply),
    );

    let evidence = frame.evidence_json();
    assert!(evidence.contains("\"paint_node_id\": 1"));
    assert!(evidence.contains("\"compositing_group_id\": 1"));
    assert!(evidence.contains("\"filter_px\": 18.0"));
    assert!(evidence.contains("\"backdrop_filter_px\": 9.0"));
    assert!(evidence.contains("\"blend_mode\": \"Multiply\""));

    for expected_fragment in [
        "arcweft.compositing-capture.v1",
        "\"record_kind\": \"compositing_group\"",
        "\"visual_bounds\"",
        "\"mask_bounds\"",
        "\"effect_outsets\"",
    ] {
        assert!(EXPECTED_EVIDENCE.contains(expected_fragment));
    }
}
