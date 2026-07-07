#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "seq06-13e1-inset-shadow-exact-golden-policy-gate"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false
---

/*
Validates that seq06.13e.1 exact golden policy/docs keep the typed Arcweft
compositor route and the no-fallback contract. This gate is safe outside the
pinned visual-golden environment because it does not compare pixels.
*/

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse(env::args().skip(1))?;
    if args.help {
        println!("usage: cargo +nightly -Zscript tools/source-gates/seq06_13e1_inset_shadow_exact_golden_policy.rs --root .");
        return Ok(());
    }
    let root = args
        .root
        .canonicalize()
        .map_err(|error| format!("canonicalize {}: {error}", args.root.display()))?;

    let policy = read_required(
        &root,
        "fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow-exact-png-policy.json",
    )?;
    let native = read_required(
        &root,
        "docs/fixtures/native/seq06_13e1_inset_box_shadow_exact_golden.json",
    )?;
    let web = read_required(
        &root,
        "docs/fixtures/web/seq06_13e1_inset_box_shadow_exact_golden.json",
    )?;
    let note = read_required(
        &root,
        "docs/implementation/seq-06.13e.1.1-web-exact-png-readback-harness-2026-07-04.md",
    )?;
    let native_capture = read_required(
        &root,
        "tools/capture-seq06-13e1-inset-shadow-native-frame.rs",
    )?;
    let web_capture = read_required(
        &root,
        "crates/arcweft-player-web/src/inset_shadow_exact_capture.rs",
    )?;
    let web_script = read_required(
        &root,
        "web/tests/seq06-13e1-inset-shadow-exact-capture.mjs",
    )?;
    let collector = read_required(
        &root,
        "tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs",
    )?;
    let css = read_required(&root, "docs/fixtures/css/seq06.13e-inset-box-shadow-card.css")?;
    let smoke = read_required(&root, "crates/arcweft-render-wgpu/tests/ui_box_shadow_gpu_smoke.rs")?;

    for required in [
        "UiCompositor::render_group",
        "PASS_BOX_SHADOW",
        "ViewProgramResource::runtime_element_styles_with_style",
        "ViewRuntimeControlVisualStyle fill/radius/shadows",
        "UiRoundedRect primitive from tree-aware Panel part style",
        "PlayerFramePlanner::prepare",
        "PreparedFrame::with_ui_scenes",
        "SharedRenderer::render_to_view",
        "UiBoxShadowPassPlan",
        "box_shadow_list_from_takumi",
        "browser DOM CSS box-shadow screenshots",
        "canvas 2D fallback",
        "CPU raster fallback",
        "WebAssembly-exported renderer readback",
        "environment_not_pinned",
        "baseline_missing",
        "max_mse",
        "max_mae",
    ] {
        require_contains(&policy, required, "policy")?;
    }

    for (label, text) in [("native fixture", &native), ("web fixture", &web)] {
        require_contains(text, "rounded_inset_shadow_card", label)?;
        require_contains(text, "mixed_outer_inset_shadow_card", label)?;
        require_contains(text, "PASS_BOX_SHADOW", label)?;
        require_contains(text, "CPU raster fallback", label)?;
    }

    require_contains(&note, "Web exact readback harness", "implementation note")?;
    require_contains(&note, "no-promotion", "implementation note")?;
    require_contains(
        &note,
        "ViewProgramResource::runtime_element_styles_with_style",
        "implementation note",
    )?;
    require_contains(&note, "SharedRenderer::render_to_view", "implementation note")?;
    require_contains(&note, "WebAssembly-exported renderer readback", "implementation note")?;
    require_contains(&native_capture, "UiCompositor::render_group", "native capture")?;
    require_contains(&native_capture, "PASS_BOX_SHADOW WGSL kind flag", "native capture")?;
    require_contains(&native_capture, "seq06_13e1_inset_box_shadow.candidate.png", "native capture")?;
    require_contains(&native_capture, "seq06_13e1_inset_box_shadow.observe.json", "native capture")?;
    require_contains(&web_capture, "capture_seq06_13e1_inset_box_shadow_exact_png", "web wasm capture")?;
    require_contains(&web_capture, "ViewStyleResource", "web wasm capture")?;
    require_contains(
        &web_capture,
        "runtime_element_styles_with_style",
        "web wasm capture",
    )?;
    require_contains(&web_capture, "PlayerFramePlanner::prepare", "web wasm capture")?;
    require_contains(&web_capture, "SharedRenderer::render_to_view", "web wasm capture")?;
    require_contains(&web_capture, "UiRoundedRect", "web wasm capture")?;
    require_contains(&web_capture, "UiCompositor::render_group", "web wasm capture")?;
    require_contains(&web_capture, "copy_texture_to_buffer", "web wasm capture")?;
    require_absent(&web_capture, "getContext", "web wasm capture")?;
    require_contains(&web_script, "capture_seq06_13e1_inset_box_shadow_exact_png", "web capture script")?;
    for forbidden in [".screenshot(", "getContext(\"2d\")", "toDataURL", "drawImage"] {
        require_absent(&web_script, forbidden, "web capture script")?;
    }
    require_contains(&collector, "web-exact-png-capture.log", "collector")?;
    require_contains(&collector, "ready_for_first_promotion_review", "collector")?;
    require_contains(&collector, "baseline_missing", "collector")?;
    require_contains(&collector, "missing_browser_runtime", "collector")?;
    require_contains(&collector, "missing_candidate_png", "collector")?;
    require_contains(&collector, "transparent_candidate", "collector")?;
    require_contains(&collector, "webgpu_validation_error", "collector")?;
    require_contains(&collector, "max_mse", "collector")?;
    require_contains(&collector, "max_mae", "collector")?;
    require_contains(&css, "box-shadow: inset", "CSS fixture")?;
    require_contains(&css, "filter: drop-shadow", "CSS fixture")?;
    require_contains(&smoke, "UiBoxShadow::inset", "GPU smoke")?;
    require_contains(&smoke, "stats.box_shadow_passes, 3", "GPU smoke")?;

    println!("seq06.13e.1 inset shadow exact-golden policy gate passed");
    Ok(())
}

#[derive(Debug)]
struct Args {
    root: PathBuf,
    help: bool,
}

impl Args {
    fn parse(values: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut root = None;
        let mut help = false;
        let mut values = values.peekable();
        while let Some(arg) = values.next() {
            match arg.as_str() {
                "--root" => root = Some(PathBuf::from(next_arg(&mut values, "--root")?)),
                "--help" | "-h" => help = true,
                unknown => return Err(format!("unknown argument `{unknown}`")),
            }
        }
        Ok(Self {
            root: if help {
                root.unwrap_or_else(|| PathBuf::from("."))
            } else {
                root.ok_or_else(|| String::from("missing --root"))?
            },
            help,
        })
    }
}

fn next_arg(
    values: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    name: &str,
) -> Result<String, String> {
    values.next().ok_or_else(|| format!("{name} requires a value"))
}

fn read_required(root: &Path, relative: &str) -> Result<String, String> {
    let path = root.join(relative);
    fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))
}

fn require_contains(text: &str, needle: &str, label: &str) -> Result<(), String> {
    if text.contains(needle) {
        Ok(())
    } else {
        Err(format!("{label} is missing required fragment `{needle}`"))
    }
}

fn require_absent(text: &str, needle: &str, label: &str) -> Result<(), String> {
    if text.contains(needle) {
        Err(format!("{label} contains forbidden fragment `{needle}`"))
    } else {
        Ok(())
    }
}
