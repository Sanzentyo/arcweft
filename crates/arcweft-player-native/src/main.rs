use arcweft_bundle::ArcweftBundle;
#[cfg(feature = "dev-capture")]
use arcweft_player_native::NativePlayerCaptureMetadata;
use arcweft_player_native::{NativePatchEndpoint, run_bundle_headless, run_bundle_windowed};
#[cfg(feature = "dev-capture")]
use arcweft_render_native as native;
use arcweft_runtime_driver::session::BundleSessionOptions;
use clap::Parser;
#[cfg(feature = "dev-capture")]
use clap::ValueEnum;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "arcweft-player-native")]
struct Args {
    /// Run without opening a native window.
    #[arg(long)]
    headless: bool,
    /// Emit JSON in headless mode.
    #[arg(long)]
    json: bool,
    /// Maximum runtime steps.
    #[arg(long, default_value_t = 64)]
    steps: usize,
    /// Capture the first resolved frame through native offscreen readback.
    #[cfg(feature = "dev-capture")]
    #[arg(long, value_enum)]
    capture: Option<NativeCaptureFormat>,
    /// Capture output path for --capture.
    #[cfg(feature = "dev-capture")]
    #[arg(long)]
    capture_out: Option<PathBuf>,
    /// Native capture width in pixels.
    #[cfg(feature = "dev-capture")]
    #[arg(long, default_value_t = 960)]
    capture_width: u32,
    /// Native capture height in pixels.
    #[cfg(feature = "dev-capture")]
    #[arg(long, default_value_t = 540)]
    capture_height: u32,
    /// Apply an `arcw run --watch` patch transport sidecar before running.
    #[arg(long)]
    patch_transport: Option<PathBuf>,
    /// Arcweft product bundle file.
    path: PathBuf,
}

#[cfg(feature = "dev-capture")]
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum NativeCaptureFormat {
    Png,
    RawRgba,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<(), String> {
    run_bundle(args)
}

fn run_bundle(args: &Args) -> Result<(), String> {
    ensure_extension(
        &args.path,
        "awfb",
        "native player expects an .awfb product bundle",
    )?;
    let mut bytes =
        fs::read(&args.path).map_err(|error| format!("failed to read bundle file: {error}"))?;
    if let Some(transport) = args.patch_transport.as_ref() {
        let mut endpoint =
            NativePatchEndpoint::from_awfb_bytes(bytes, BundleSessionOptions::default())
                .map_err(|error| error.to_string())?;
        endpoint
            .apply_patch_transport_path(transport)
            .map_err(|error| error.to_string())?;
        bytes = endpoint.active_awfb_bytes().to_vec();
    }
    let bundle = ArcweftBundle::from_product_path_slice(&args.path, &bytes)
        .map_err(|error| error.to_string())?;
    run_bundle_program(args, bundle)
}

fn run_bundle_program(args: &Args, bundle: ArcweftBundle) -> Result<(), String> {
    if args.headless {
        let report = run_bundle_headless(&bundle, args.steps).map_err(|error| error.to_string())?;
        #[cfg(feature = "dev-capture")]
        let report = attach_native_capture(args, report)?;
        write_headless_report(args, &report)?;
        return Ok(());
    }
    run_bundle_windowed(bundle, args.steps).map_err(|error| error.to_string())
}

#[cfg(feature = "dev-capture")]
fn attach_native_capture(
    args: &Args,
    mut report: arcweft_player_native::HeadlessPlayerReport,
) -> Result<arcweft_player_native::HeadlessPlayerReport, String> {
    if let Some(format) = args.capture {
        let capture_out = args
            .capture_out
            .as_ref()
            .ok_or_else(|| "--capture requires --capture-out".to_owned())?;
        let frame = report
            .frames
            .first()
            .ok_or_else(|| "no display frame was produced for native capture".to_owned())?;
        let capture = native::capture_frame_rgba(frame, args.capture_width, args.capture_height)
            .map_err(|error| error.to_string())?;
        let bytes = native_capture_bytes(format, &capture)?;
        fs::write(capture_out, bytes)
            .map_err(|error| format!("failed to write capture: {error}"))?;
        report.native_capture = Some(NativePlayerCaptureMetadata {
            renderer: "native_offscreen_wgpu_glyphon".to_owned(),
            format: format.resource_name().to_owned(),
            width: capture.width,
            height: capture.height,
            pixel_format: "rgba8_unorm".to_owned(),
            row_stride_bytes: capture.width.saturating_mul(4),
            content_bbox: capture.content_bbox,
            content_pixels: capture.content_pixels,
            written: capture_out.display().to_string(),
        });
    }
    Ok(report)
}

fn write_headless_report(
    args: &Args,
    report: &arcweft_player_native::HeadlessPlayerReport,
) -> Result<(), String> {
    if args.json {
        serde_json::to_writer_pretty(std::io::stdout(), report)
            .map_err(|error| format!("failed to write JSON: {error}"))?;
        println!();
    } else {
        for frame in &report.frames {
            println!("{} {}", frame.line.0, frame.text);
        }
    }
    Ok(())
}

fn ensure_extension(path: &Path, expected: &str, message: &str) -> Result<(), String> {
    if path.extension().and_then(|extension| extension.to_str()) == Some(expected) {
        return Ok(());
    }
    Err(format!("{message}: {}", path.display()))
}

#[cfg(feature = "dev-capture")]
fn native_capture_bytes(
    format: NativeCaptureFormat,
    capture: &native::NativeFrameCapture,
) -> Result<Vec<u8>, String> {
    match format {
        NativeCaptureFormat::RawRgba => Ok(capture.rgba.clone()),
        NativeCaptureFormat::Png => encode_png(capture),
    }
}

#[cfg(feature = "dev-capture")]
fn encode_png(capture: &native::NativeFrameCapture) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, capture.width, capture.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| format!("failed to encode PNG capture: {error}"))?;
        writer
            .write_image_data(&capture.rgba)
            .map_err(|error| format!("failed to encode PNG capture: {error}"))?;
        writer
            .finish()
            .map_err(|error| format!("failed to encode PNG capture: {error}"))?;
    }
    Ok(bytes)
}

#[cfg(feature = "dev-capture")]
impl NativeCaptureFormat {
    const fn resource_name(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::RawRgba => "raw-rgba",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::{
        ARCWEFT_BUNDLE_SCHEMA_VERSION, BundleManifest, BundleRuntimeSummary, BundleSource,
    };
    use arcweft_core::{
        bytecode::BytecodeProgram,
        plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan},
    };
    use std::{
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn args_for(path: &str) -> Args {
        Args {
            headless: true,
            json: true,
            steps: 1,
            #[cfg(feature = "dev-capture")]
            capture: None,
            #[cfg(feature = "dev-capture")]
            capture_out: None,
            #[cfg(feature = "dev-capture")]
            capture_width: 64,
            #[cfg(feature = "dev-capture")]
            capture_height: 64,
            patch_transport: None,
            path: PathBuf::from(path),
        }
    }

    #[test]
    fn default_input_requires_awfb_bundle() {
        let args = args_for("game.arcw");
        let error = run(&args).expect_err("source input must not be accepted by default");

        assert!(error.contains("native player expects an .awfb product bundle"));
    }

    #[cfg(not(feature = "dev-capture"))]
    #[test]
    fn capture_option_requires_dev_capture_feature() {
        let result = Args::try_parse_from([
            "arcweft-player-native",
            "--headless",
            "--capture",
            "png",
            "game.awfb",
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn default_bundle_mode_runs_awfb_without_source_flag() {
        let path = temp_awfb_path("bundle-mode-runs");
        let bundle = minimal_bundle();
        fs::write(
            &path,
            bundle
                .to_format_bytes(arcweft_bundle::BundleFormat::Awfb)
                .expect("bundle encodes"),
        )
        .expect("bundle fixture writes");
        let mut args = args_for(path.to_str().expect("temp path is utf8"));
        args.json = false;
        args.steps = 8;

        let result = run(&args);
        let _ = fs::remove_file(&path);

        result.expect("bundle mode runs an .awfb program");
    }

    #[test]
    fn product_awfb_input_does_not_fall_back_to_json_export() {
        let path = temp_awfb_path("legacy-json-is-not-product-awfb");
        let bundle = minimal_bundle();
        fs::write(&path, bundle.to_json_bytes().expect("legacy json encodes"))
            .expect("legacy json fixture writes");
        let args = args_for(path.to_str().expect("temp path is utf8"));

        let error = run(&args).expect_err("product .awfb input must require AWFB magic");
        let _ = fs::remove_file(&path);

        assert!(error.contains("AWFB"));
        assert!(error.contains("magic"));
    }

    #[cfg(feature = "dev-capture")]
    #[test]
    fn capture_mode_requires_capture_out() {
        let path = temp_awfb_path("capture-requires-out");
        let bundle = minimal_bundle();
        fs::write(
            &path,
            bundle
                .to_format_bytes(arcweft_bundle::BundleFormat::Awfb)
                .expect("bundle encodes"),
        )
        .expect("bundle fixture writes");
        let mut args = args_for(path.to_str().expect("temp path is utf8"));
        args.capture = Some(NativeCaptureFormat::Png);

        let error = run(&args).expect_err("capture mode requires output path");
        let _ = fs::remove_file(&path);

        assert!(error.contains("--capture requires --capture-out"));
    }

    fn minimal_bundle() -> ArcweftBundle {
        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.main".to_owned())),
            vec![RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
                ops: vec![FlowOp::Return("done".to_owned())],
            }],
            Vec::new(),
        )
        .expect("runtime plan is valid");
        let bytecode = BytecodeProgram::from_runtime_plan(plan);
        ArcweftBundle::new(
            BundleManifest {
                source_label: "bundle-mode-runs.arcw".to_owned(),
                profile_id: None,
                profile_kind: None,
                entry: None,
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 1,
                    line_task_groups: 0,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            BundleSource {
                label: "bundle-mode-runs.arcw".to_owned(),
                text: "flow @flow.main main { return \"done\" }".to_owned(),
            },
            bytecode,
            arcweft_render_text::LineDisplayCatalog::default(),
        )
    }

    fn temp_awfb_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "arcweft-player-native-{label}-{}-{nanos}-v{ARCWEFT_BUNDLE_SCHEMA_VERSION}.awfb",
            process::id()
        ))
    }
}
