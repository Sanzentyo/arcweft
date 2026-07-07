use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const REQUIRED_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED";
const PINNED_ENV: &str = "ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED";
const METRICS: &[&str] = &["psnr", "ssim", "mse", "mae", "maxae"];
const EXPECTED_SIZE: (u32, u32) = (320, 180);

#[derive(Clone, Copy, Debug)]
enum Target {
    Native,
    Web,
}

impl Target {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Web => "web",
        }
    }

    fn artifact_dir(self) -> PathBuf {
        workspace_root()
            .join("target/seq06.13e.1-inset-box-shadow-golden")
            .join(self.as_str())
    }

    fn reference_png(self) -> PathBuf {
        workspace_root()
            .join("fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow")
            .join(self.as_str())
            .join("seq06_13e1_inset_box_shadow.png")
    }

    fn capture_log(self) -> PathBuf {
        self.artifact_dir().join("command-logs").join(match self {
            Self::Native => "native-exact-png-capture.log",
            Self::Web => "web-exact-png-capture.log",
        })
    }

    fn review_decision(self) -> PathBuf {
        workspace_root()
            .join("target/seq06.13e.1-inset-box-shadow-golden/review")
            .join(format!(
                "seq06_13e1_{}_promotion_decision.json",
                self.as_str()
            ))
    }
}

#[test]
fn seq06_13e1_inset_shadow_policy_pins_typed_compositor_route() {
    let root = workspace_root();
    let policy =
        fs::read_to_string(root.join(
            "fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow-exact-png-policy.json",
        ))
        .expect("read seq06.13e.1 exact PNG policy");
    for required in [
        "ViewCompositor::render_group",
        "PASS_BOX_SHADOW",
        "CPU raster fallback",
        "WebAssembly-exported renderer readback",
    ] {
        assert!(
            policy.contains(required),
            "policy should retain required seq06.13e.1 route fragment `{required}`"
        );
    }

    for (label, path) in [
        (
            "native fixture",
            root.join("docs/fixtures/native/seq06_13e1_inset_box_shadow_exact_golden.json"),
        ),
        (
            "web fixture",
            root.join("docs/fixtures/web/seq06_13e1_inset_box_shadow_exact_golden.json"),
        ),
    ] {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {label} {}: {error}", path.display()));
        for required in [
            "ViewCompositor::render_group",
            "PASS_BOX_SHADOW",
            "rounded_inset_shadow_card",
            "mixed_outer_inset_shadow_card",
            "CPU raster fallback",
        ] {
            assert!(
                text.contains(required),
                "{label} should retain required seq06.13e.1 route fragment `{required}`"
            );
        }
    }

    let smoke = fs::read_to_string(
        root.join("crates/arcweft-render-wgpu/tests/view_box_shadow_gpu_smoke.rs"),
    )
    .expect("read seq06.13e GPU smoke source");
    assert!(smoke.contains("ViewBoxShadow::inset"));
    assert!(smoke.contains("stats.box_shadow_passes, 3"));

    let web_export = fs::read_to_string(
        root.join("crates/arcweft-player-web/src/inset_shadow_exact_capture.rs"),
    )
    .expect("read Web exact wasm export source");
    let surface_lowerer =
        fs::read_to_string(root.join("crates/arcweft-player-scene/src/frame/surfaces.rs"))
            .expect("read player surface lowerer source");
    assert!(web_export.contains("capture_seq06_13e1_inset_box_shadow_exact_png"));
    assert!(web_export.contains("ViewStyleResource"));
    assert!(web_export.contains("runtime_surfaces_with_style"));
    assert!(web_export.contains("BundlePresentationSnapshot"));
    assert!(web_export.contains("PlayerFramePlanner::prepare"));
    assert!(web_export.contains("SharedRenderer::render_to_view"));
    assert!(surface_lowerer.contains("ViewRoundedRect"));
    assert!(surface_lowerer.contains("ViewCompositingEffects"));
    assert!(surface_lowerer.contains("ViewBoxShadow"));
    assert!(surface_lowerer.contains("push_view_scene"));
    assert!(web_export.contains("copy_texture_to_buffer"));
    assert!(!web_export.contains("runtime_surface_style"));
    assert!(!web_export.contains("with_view_scenes"));
    assert!(!web_export.contains("getContext"));

    let web_script =
        fs::read_to_string(root.join("web/tests/seq06-13e1-inset-shadow-exact-capture.mjs"))
            .expect("read Web exact capture script");
    assert!(web_script.contains("capture_seq06_13e1_inset_box_shadow_exact_png"));
    for forbidden in [
        ".screenshot(",
        "getContext(\"2d\")",
        "toDataURL",
        "drawImage",
    ] {
        assert!(
            !web_script.contains(forbidden),
            "Web exact capture script must not use forbidden fallback fragment `{forbidden}`"
        );
    }
}

#[test]
fn seq06_13e1_inset_shadow_web_collector_classifies_exact_failures() {
    let collector = fs::read_to_string(
        workspace_root().join("tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs"),
    )
    .expect("read seq06.13e.1 collector source");
    for required in [
        "webgpu_pin_missing",
        "missing_browser_runtime",
        "missing_web_runtime_tool",
        "missing_candidate_png",
        "transparent_candidate",
        "webgpu_validation_error",
        "candidate and reference PNG dimensions differ",
        "imq comparison failed",
        "web-exact-png-capture.log",
    ] {
        assert!(
            collector.contains(required),
            "collector should retain Web exact failure classification `{required}`"
        );
    }
}

#[test]
#[ignore = "tier 2 visual regression: requires pinned native exact PNG packet"]
fn seq06_13e1_inset_shadow_native_exact_png_packet_is_complete() {
    assert_exact_png_packet(Target::Native);
}

#[test]
#[ignore = "tier 2 visual regression: requires pinned WebGPU exact PNG packet"]
fn seq06_13e1_inset_shadow_web_exact_png_packet_is_complete() {
    assert_exact_png_packet(Target::Web);
}

fn assert_exact_png_packet(target: Target) {
    let dir = target.artifact_dir();
    let candidate = dir.join("seq06_13e1_inset_box_shadow.candidate.png");
    let observe = dir.join("seq06_13e1_inset_box_shadow.observe.json");
    let metrics = dir.join("seq06_13e1_inset_box_shadow.imq.json");
    let environment = dir.join("seq06_13e1_inset_box_shadow.environment.json");
    let reference = target.reference_png();
    let capture_log = target.capture_log();
    let review = target.review_decision();
    let required = [
        &candidate,
        &observe,
        &metrics,
        &environment,
        &capture_log,
        &review,
    ];
    let missing = required
        .iter()
        .filter(|path| !path.exists())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        let classification = exact_packet_missing_classification();
        let message = format!(
            "seq06.13e.1 {} exact PNG packet unavailable: classification={classification}; missing={missing:?}",
            target.as_str()
        );
        assert!(!exact_required(), "{message}");
        eprintln!("{message}");
        return;
    }

    assert_png_dimensions(&candidate, EXPECTED_SIZE, "candidate");
    if reference.exists() {
        assert_png_dimensions(&reference, EXPECTED_SIZE, "reference");
        assert_json_contains(
            &review,
            &["passed_existing_baseline_gate", "promoted", "false"],
        );
    } else {
        assert_json_contains(&metrics, &["baseline_missing", "max_mse", "max_mae"]);
        assert_json_contains(
            &review,
            &["ready_for_first_promotion_review", "promoted", "false"],
        );
    }
    assert_json_contains(
        &observe,
        &[
            "PASS_BOX_SHADOW",
            "box_shadow",
            "inset",
            "ViewProgramResource::runtime_surfaces_with_style",
            "BundlePresentationSnapshot::surfaces",
            "SharedRenderer::render_to_view",
            "ViewRoundedRect",
            "ViewCompositor::render_group",
            "copy_texture_to_buffer",
        ],
    );
    assert_json_contains(
        &environment,
        &["environment", "imq", "arcweft", "candidate", "reference"],
    );
    if matches!(target, Target::Web) {
        assert_json_contains(
            &environment,
            &["browser", "runtime", "webgpu", "device_pixel_ratio"],
        );
    }
    assert_json_contains(&metrics, METRICS);
}

fn exact_packet_missing_classification() -> &'static str {
    if exact_required() && env::var_os(PINNED_ENV).is_none() {
        "environment_not_pinned"
    } else if !command_available("imq") {
        "environment_blocker:missing_imq"
    } else {
        "baseline_missing"
    }
}

fn exact_required() -> bool {
    env::var_os(REQUIRED_ENV).is_some()
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn assert_png_dimensions(path: &Path, expected: (u32, u32), label: &str) {
    let bytes = fs::read(path)
        .unwrap_or_else(|error| panic!("read {label} PNG {}: {error}", path.display()));
    assert!(
        bytes.len() >= 24,
        "{label} PNG should include an IHDR chunk: {}",
        path.display()
    );
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{label} PNG signature");
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("PNG width bytes"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("PNG height bytes"));
    assert_eq!(
        (width, height),
        expected,
        "{label} PNG dimensions: {}",
        path.display()
    );
}

fn assert_json_contains(path: &Path, needles: &[&str]) {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read JSON {}: {error}", path.display()));
    for needle in needles {
        assert!(
            text.contains(needle),
            "{} should contain `{needle}`",
            path.display()
        );
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("arcweft-render-wgpu lives under crates/")
        .to_path_buf()
}
