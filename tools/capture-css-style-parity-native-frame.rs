#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
arcweft-bundle = { path = "../crates/arcweft-bundle" }
arcweft-layout = { path = "../crates/arcweft-layout" }
arcweft-player-scene = { path = "../crates/arcweft-player-scene" }
arcweft-player-web = { path = "../crates/arcweft-player-web" }
arcweft-render-wgpu = { path = "../crates/arcweft-render-wgpu" }
arcweft-runtime-driver = { path = "../crates/arcweft-runtime-driver" }
png = "0.18.1"
pollster = "0.4.0"
serde_json = "1.0.150"
wgpu = { version = "29.0.3", default-features = false, features = ["std", "wgsl", "dx12", "metal", "vulkan"] }

[patch.crates-io]
glyphon = { path = "../vendor/glyphon" }
---

use arcweft_bundle::{ArcweftBundle, BundleFormat};
use arcweft_layout::ScalePolicy;
use arcweft_player_scene::{
    frame::{PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest},
    images::BundleImageCatalog,
    input::InputController,
};
use arcweft_player_web::report::WebFrameObservationReport;
use arcweft_render_wgpu::geometry::{PreparedFrame, RenderPreferences, RenderViewport};
use arcweft_render_wgpu::offscreen::{CaptureAttachment, CaptureRequest, SharedOffscreenCapture};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions, BundleStepInput};
use png::{BitDepth, ColorType, Encoder};
use serde_json::json;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args().skip(1).collect())?;
    let bundle = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &fs::read(&args.bundle)?)?;
    let font_paths = args
        .additional_fonts
        .iter()
        .chain(std::iter::once(&args.font))
        .collect::<Vec<_>>();
    let font_resources = font_paths
        .iter()
        .map(|path| fs::read(path))
        .collect::<Result<Vec<_>, _>>()?;
    let frame = prepare_css_style_frame(
        &bundle,
        args.viewport.render_viewport(),
        args.max_ticks,
        args.visual_time_millis,
        &font_resources,
    )?;
    if let Some(path) = &args.frame_report {
        write_frame_report(
            path,
            &frame,
            args.viewport.as_str(),
            args.visual_time_millis,
            &font_paths,
            &font_resources,
        )?;
    }
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(args.target_format.wgpu()))?;
    for font_bytes in font_resources {
        capture.register_font_bytes(font_bytes)?;
    }
    let image = capture.capture(&frame, &CaptureRequest::whole_frame_color())?;
    let rgba = image
        .attachment_rgba(CaptureAttachment::Color)
        .ok_or("offscreen capture omitted the requested color attachment")?;
    write_png(&args.output, image.width, image.height, rgba)?;
    println!(
        "wrote css-style native capture {} ({}x{}, viewport={}, visual_time_millis={}, target_format={})",
        args.output.display(),
        image.width,
        image.height,
        args.viewport.as_str(),
        args.visual_time_millis,
        args.target_format.as_str()
    );
    Ok(())
}

fn prepare_css_style_frame(
    bundle: &ArcweftBundle,
    viewport: RenderViewport,
    max_ticks: u64,
    visual_time_millis: u64,
    font_resources: &[Vec<u8>],
) -> Result<PreparedFrame, Box<dyn Error>> {
    let mut session = BundleSession::new(bundle, BundleSessionOptions::default())?;
    let images = BundleImageCatalog::from_bundle(bundle)?;
    let mut presentation = None;
    for tick in 1..=max_ticks {
        let clock = RuntimeClockStep::from_millis(tick, 16)?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let ready = step.presentation.textboxes.latest_active().is_some();
        presentation = Some(step.presentation);
        if ready {
            break;
        }
    }
    let presentation = presentation.ok_or("css-style sample produced no presentation frame")?;
    if presentation.textboxes.latest_active().is_none() {
        return Err(format!(
            "css-style sample did not reach a dialogue frame within {max_ticks} ticks"
        )
        .into());
    }
    let mut planner = PlayerFramePlannerState::new();
    for font_bytes in font_resources {
        planner.register_font_bytes(font_bytes.clone())?;
    }
    let mut input = InputController::default();
    planner
        .prepare(
            &mut input,
            PlayerFrameRequest {
                presentation: &presentation,
                fx_definitions: &bundle.fx_definitions,
                images: &images,
                viewport,
                fit: PlayerFrameFit::design_1280x720(ScalePolicy::Contain),
                image_time_millis: visual_time_millis,
                visual_time_millis,
                dialogue_reveal_complete: false,
                preferences: RenderPreferences::default(),
            },
        )
        .map(|prepared| prepared.frame)
        .map_err(|error| error.to_string().into())
}

struct Args {
    bundle: PathBuf,
    font: PathBuf,
    additional_fonts: Vec<PathBuf>,
    output: PathBuf,
    frame_report: Option<PathBuf>,
    viewport: CssStyleViewport,
    visual_time_millis: u64,
    max_ticks: u64,
    target_format: CaptureTargetFormat,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut parsed = Self {
            bundle: PathBuf::from("web/local/css-style-parity.awfb"),
            font: PathBuf::from("web/assets/arcweft-demo.ttf"),
            additional_fonts: vec![PathBuf::from(
                "web/assets/noto-sans-jp-css-style-parity.ttf",
            )],
            output: PathBuf::from("target/css-style-parity/native-default.png"),
            frame_report: Some(PathBuf::from(
                "target/css-style-parity/native-default.frame.json",
            )),
            viewport: CssStyleViewport::Default,
            visual_time_millis: 9_000,
            max_ticks: 16,
            target_format: CaptureTargetFormat::Rgba8Unorm,
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--bundle" => {
                    index += 1;
                    parsed.bundle = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--bundle requires a path".to_owned())?,
                    );
                }
                "--font" => {
                    index += 1;
                    parsed.font = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--font requires a path".to_owned())?,
                    );
                }
                "--additional-font" => {
                    index += 1;
                    parsed
                        .additional_fonts
                        .push(PathBuf::from(args.get(index).ok_or_else(|| {
                            "--additional-font requires a path".to_owned()
                        })?));
                }
                "--no-additional-fonts" => parsed.additional_fonts.clear(),
                "--output" => {
                    index += 1;
                    parsed.output = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--output requires a path".to_owned())?,
                    );
                }
                "--frame-report" => {
                    index += 1;
                    parsed.frame_report =
                        Some(PathBuf::from(args.get(index).ok_or_else(|| {
                            "--frame-report requires a path".to_owned()
                        })?));
                }
                "--no-frame-report" => parsed.frame_report = None,
                "--viewport" => {
                    index += 1;
                    parsed.viewport = args
                        .get(index)
                        .ok_or_else(|| "--viewport requires default|compact|hidpi".to_owned())?
                        .parse()?;
                }
                "--max-ticks" => {
                    index += 1;
                    parsed.max_ticks = args
                        .get(index)
                        .ok_or_else(|| "--max-ticks requires an integer".to_owned())?
                        .parse()
                        .map_err(|error| format!("--max-ticks must be an integer: {error}"))?;
                }
                "--visual-time-millis" => {
                    index += 1;
                    parsed.visual_time_millis = args
                        .get(index)
                        .ok_or_else(|| "--visual-time-millis requires an integer".to_owned())?
                        .parse()
                        .map_err(|error| {
                            format!("--visual-time-millis must be an integer: {error}")
                        })?;
                }
                "--target-format" => {
                    index += 1;
                    parsed.target_format = args
                        .get(index)
                        .ok_or_else(|| "--target-format requires a format".to_owned())?
                        .parse()?;
                }
                "--help" | "-h" => return Err(Self::usage()),
                unknown => return Err(format!("unknown argument `{unknown}`\n{}", Self::usage())),
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn usage() -> String {
        "usage: cargo +nightly -Zscript tools/capture-css-style-parity-native-frame.rs \
         [--bundle web/local/css-style-parity.awfb] [--font web/assets/arcweft-demo.ttf] \
         [--additional-font PATH] [--no-additional-fonts] \
         [--output target/css-style-parity/native-default.png] \
         [--frame-report target/css-style-parity/native-default.frame.json] \
         [--viewport default|compact|hidpi] [--visual-time-millis 9000] [--max-ticks 16] \
         [--target-format rgba8unorm|rgba8unorm-srgb|bgra8unorm|bgra8unorm-srgb]"
            .to_owned()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CssStyleViewport {
    Default,
    Compact,
    Hidpi,
}

impl CssStyleViewport {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Compact => "compact",
            Self::Hidpi => "hidpi",
        }
    }

    const fn render_viewport(self) -> RenderViewport {
        match self {
            Self::Default => RenderViewport {
                logical_width: 1280.0,
                logical_height: 720.0,
                physical_width: 1280,
                physical_height: 720,
                scale_factor: 1.0,
            },
            Self::Compact => RenderViewport {
                logical_width: 960.0,
                logical_height: 540.0,
                physical_width: 960,
                physical_height: 540,
                scale_factor: 1.0,
            },
            Self::Hidpi => RenderViewport {
                logical_width: 640.0,
                logical_height: 360.0,
                physical_width: 1280,
                physical_height: 720,
                scale_factor: 2.0,
            },
        }
    }
}

impl std::str::FromStr for CssStyleViewport {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "default" => Ok(Self::Default),
            "compact" => Ok(Self::Compact),
            "hidpi" => Ok(Self::Hidpi),
            unknown => Err(format!("unknown viewport `{unknown}`")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureTargetFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Bgra8Unorm,
    Bgra8UnormSrgb,
}

impl CaptureTargetFormat {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Rgba8Unorm => "rgba8unorm",
            Self::Rgba8UnormSrgb => "rgba8unorm-srgb",
            Self::Bgra8Unorm => "bgra8unorm",
            Self::Bgra8UnormSrgb => "bgra8unorm-srgb",
        }
    }

    const fn wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            Self::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            Self::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
            Self::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
        }
    }
}

impl std::str::FromStr for CaptureTargetFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "rgba8unorm" => Ok(Self::Rgba8Unorm),
            "rgba8unorm-srgb" => Ok(Self::Rgba8UnormSrgb),
            "bgra8unorm" => Ok(Self::Bgra8Unorm),
            "bgra8unorm-srgb" => Ok(Self::Bgra8UnormSrgb),
            unknown => Err(format!("unknown target format `{unknown}`")),
        }
    }
}

fn write_frame_report(
    path: &Path,
    frame: &PreparedFrame,
    checkpoint: &str,
    visual_time_millis: u64,
    font_paths: &[&PathBuf],
    font_resources: &[Vec<u8>],
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut report = serde_json::to_value(WebFrameObservationReport::from_prepared_frame(frame))?;
    let object = report
        .as_object_mut()
        .ok_or("canonical frame report did not serialize as an object")?;
    object.insert("checkpoint".to_owned(), json!(checkpoint));
    object.insert("visual_time_millis".to_owned(), json!(visual_time_millis));
    object.insert(
        "fonts".to_owned(),
        json!(
            font_paths
                .iter()
                .zip(font_resources)
                .map(|(path, bytes)| json!({
                    "path": path.display().to_string(),
                    "byte_len": bytes.len(),
                    "fnv1a64": fnv1a64_hex(bytes),
                }))
                .collect::<Vec<_>>()
        ),
    );
    fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(())
}

fn fnv1a64_hex(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{hash:016x}")
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(path)?;
    let mut encoder = Encoder::new(file, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}
