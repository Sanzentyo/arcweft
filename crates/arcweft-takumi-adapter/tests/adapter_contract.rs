use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_scene::{ViewAffine2D, ViewPrimitiveRange};
use arcweft_takumi_adapter::{
    ArcweftNodeMetadata, CssInvalidationClass, CssPropertyClass, TakumiCaptureFrame,
    TakumiCaptureRecord, TakumiDiagnosticCode, TakumiPath,
};
use arcweft_view::{ContainerKind, FragmentKind, HandlerId, NodeId, NodeKey, StyleId};

#[test]
fn capture_record_keeps_same_range_and_bounds_as_rendered_primitive() {
    let metadata = ArcweftNodeMetadata::new(
        NodeId(1),
        NodeKey(2),
        FragmentKind::Container(ContainerKind::Block),
        StyleId(3),
        [HandlerId(4)],
        None,
    );
    let primitive_range = ViewPrimitiveRange { start: 5, end: 6 };
    let bounds = HitRect::new(1.0, 2.0, 3.0, 4.0);
    let record = TakumiCaptureRecord::new(
        metadata,
        primitive_range,
        bounds,
        ViewAffine2D::IDENTITY,
        None,
    );
    let mut frame = TakumiCaptureFrame::default();
    frame.push(record);

    assert_eq!(frame.records()[0].primitive_range(), primitive_range);
    assert_eq!(frame.records()[0].local_bounds(), bounds);
}

#[test]
fn transform_and_opacity_are_paint_only_invalidations() {
    assert_eq!(
        CssPropertyClass::classify("transform").invalidation(),
        CssInvalidationClass::PaintOnly,
    );
    assert_eq!(
        CssPropertyClass::classify("opacity").invalidation(),
        CssInvalidationClass::PaintOnly,
    );
    assert_eq!(
        CssPropertyClass::classify("grid-template-columns").invalidation(),
        CssInvalidationClass::LayoutScene,
    );
}

#[test]
fn takumi_path_attributes_are_stable() {
    let path = TakumiPath::root().child(2).child(0).child(5);
    assert_eq!(path.to_attribute(), "2.0.5");
    assert_eq!(TakumiPath::from_attribute("2.0.5"), Some(path));
}

#[test]
fn diagnostic_code_names_are_contractual() {
    assert_eq!(
        arcweft_takumi_adapter::DirectCssSupport::diagnose_css(".x { filter: url(#goo); }")
            .diagnostics()[0]
            .code(),
        TakumiDiagnosticCode::UnsupportedDirectCss,
    );
}
