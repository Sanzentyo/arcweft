#[test]
fn frame_report_schema_mentions_focus_navigation() {
    let source = include_str!("../src/report.rs");
    assert!(source.contains("focus: arcweft_render_wgpu::geometry::FocusNavigationDebug"));
    assert!(source.contains("frame.focus_debug()"));
}
