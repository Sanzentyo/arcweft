const CAPTURE_SOURCE: &str = include_str!("../src/capture.rs");
const EVIDENCE_SOURCE: &str = include_str!("../src/evidence.rs");
const SCHEMA: &str = include_str!("../../../docs/schemas/compositing-capture-evidence.schema.json");
const EXPECTED_EVIDENCE: &str = include_str!("fixtures/compositing-capture/expected-evidence.json");

#[test]
fn capture_schema_sources_do_not_leak_native_or_platform_identity() {
    let combined = [CAPTURE_SOURCE, EVIDENCE_SOURCE, SCHEMA, EXPECTED_EVIDENCE].join("\n");
    for forbidden in [
        "HWND",
        "NSView",
        "winit::window",
        "web_sys",
        "swapchain",
        "surface_id",
        "native_window",
        "platform_handle",
    ] {
        assert!(
            !combined.contains(forbidden),
            "forbidden platform identity leaked: {forbidden}"
        );
    }
}

#[test]
fn compositing_evidence_does_not_use_cpu_raster_expected_images() {
    let combined = [CAPTURE_SOURCE, EVIDENCE_SOURCE, EXPECTED_EVIDENCE]
        .join("\n")
        .to_ascii_lowercase();
    for forbidden in [
        "takumi_raster",
        "render_rgba",
        "cpu_rgba",
        "full ui surface upload",
    ] {
        assert!(
            !combined.contains(forbidden),
            "CPU fallback evidence hook leaked: {forbidden}"
        );
    }
}
