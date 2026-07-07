use std::fs;
use std::path::Path;

#[test]
fn renderer_sources_do_not_introduce_forbidden_full_surface_fallbacks() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_dir = manifest_dir.join("src");
    let compositor_sources = [
        source_dir.join("view_blend.rs"),
        source_dir.join("view_clip_path.rs"),
        source_dir.join("view_compositor.rs"),
        source_dir.join("view_effects.rs"),
        source_dir.join("view_mask.rs"),
    ];
    let forbidden = [
        "takumi_raster",
        "takumi::raster",
        "tiny_skia::Pixmap",
        "render_to_rgba",
        "upload_full_ui_surface",
        "read_to_buffer",
        "map_async",
    ];

    for path in compositor_sources {
        let source = fs::read_to_string(&path).expect("source is readable");
        for term in forbidden {
            assert!(
                !source.contains(term),
                "{} contains forbidden fallback marker `{term}`",
                path.display()
            );
        }
    }
}
