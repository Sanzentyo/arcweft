use std::{fs, path::Path};

#[test]
fn takumi_adapter_source_does_not_call_cpu_full_surface_raster_fallback() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/arcweft-takumi-adapter");
    let source_roots = [
        manifest_dir.join("src"),
        repo_root.join("crates/arcweft-render-wgpu/src"),
    ];

    let banned_needles = [
        "takumi_raster::",
        "takumi-raster",
        "render_to_image",
        "render_to_pixmap",
        "render_rgba",
        "read_pixels",
        "full_surface_fallback",
        "CpuFullSurfaceFallback",
    ];

    for root in source_roots {
        for file in rust_files(&root) {
            let content = fs::read_to_string(&file).expect("Rust source can be read");
            for needle in banned_needles {
                assert!(
                    !content.contains(needle),
                    "CPU full-surface fallback marker `{needle}` found in {}",
                    file.display()
                );
            }
        }
    }
}

fn rust_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files.sort();
    files
}

fn collect_rust_files(root: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == "rs")
        {
            files.push(path);
        }
    }
}
