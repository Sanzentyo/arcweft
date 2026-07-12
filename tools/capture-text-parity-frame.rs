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

// Captures the shared headless-prepared text frame through native WGPU.

use arcweft_bundle::{ArcweftBundle, BundleFormat};
use arcweft_layout::ScalePolicy;
use arcweft_player_scene::{
    frame::{PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest},
    images::BundleImageCatalog,
    input::InputController,
};
use arcweft_player_web::report::WebFrameObservationReport;
use arcweft_render_wgpu::geometry::{PreparedFrame, RenderPreferences, RenderViewport};
use arcweft_render_wgpu::offscreen::{
    CaptureAttachment, CaptureCropPolicy, CaptureRegion, CaptureRequest, CaptureScope,
    SharedOffscreenCapture,
};
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
    let prepared = prepare_text_parity_frame(
        &bundle,
        args.viewport.render_viewport(),
        args.max_ticks,
        args.visual_time_millis,
        args.advance_count,
        &font_resources,
    )?;
    if let Some(path) = &args.frame_report {
        write_frame_report(
            path,
            &prepared.frame,
            &args.checkpoint,
            args.visual_time_millis,
            prepared.logical_clock,
            &font_paths,
            &font_resources,
        )?;
    }
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(args.target_format.wgpu()))?;
    for font_bytes in font_resources {
        capture.register_font_bytes(font_bytes)?;
    }
    let image = capture.capture(&prepared.frame, &CaptureRequest::whole_frame_color())?;
    let rgba = image
        .attachment_rgba(CaptureAttachment::Color)
        .ok_or("offscreen capture omitted the requested color attachment")?;
    write_png(&args.output, image.width, image.height, rgba)?;
    if let Some(prefix) = &args.scope_capture_prefix {
        write_text_scope_capture(
            &mut capture,
            &prepared.frame,
            prefix,
            &args.checkpoint,
            args.visual_time_millis,
        )?;
    }
    println!(
        "wrote native/headless text parity capture {} ({}x{}, checkpoint={}, viewport={}, advance_count={}, visual_time_millis={}, target_format={})",
        args.output.display(),
        image.width,
        image.height,
        args.checkpoint,
        args.viewport.as_str(),
        args.advance_count,
        args.visual_time_millis,
        args.target_format.as_str()
    );
    Ok(())
}

fn prepare_text_parity_frame(
    bundle: &ArcweftBundle,
    viewport: RenderViewport,
    max_ticks: u64,
    visual_time_millis: u64,
    mut advance_count: usize,
    font_resources: &[Vec<u8>],
) -> Result<PreparedTextParityFrame, Box<dyn Error>> {
    let mut session = BundleSession::new(bundle, BundleSessionOptions::default())?;
    let images = BundleImageCatalog::from_bundle(bundle)?;
    let mut presentation = None;
    let mut next_tick = 1_u64;
    for _ in 0..max_ticks {
        let tick = next_tick;
        next_tick = next_tick.saturating_add(1);
        let clock = RuntimeClockStep::from_millis(tick, 16)?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let advance_target = step
            .presentation
            .textboxes
            .latest_active()
            .and_then(|(textbox, _)| textbox.advance_target());
        presentation = Some(step.presentation);
        if advance_count > 0
            && let Some(target) = advance_target
        {
            session.queue_dialogue_advance(target);
            advance_count -= 1;
            continue;
        }
        if advance_count == 0 && advance_target.is_some() {
            break;
        }
    }
    let presentation = presentation.ok_or("text parity sample produced no presentation frame")?;
    if advance_count != 0 || presentation.textboxes.latest_active().is_none() {
        return Err(format!(
            "text parity sample did not reach the requested dialogue page within {max_ticks} ticks (remaining advances: {advance_count})"
        )
        .into());
    }
    let activation_tick = next_tick.saturating_sub(1);
    let elapsed_steps = visual_time_millis.div_ceil(16);
    let mut presentation = presentation;
    for _ in 0..elapsed_steps {
        let clock = RuntimeClockStep::from_millis(next_tick, 16)?;
        next_tick = next_tick.saturating_add(1);
        presentation = session
            .step_with_clock(clock, BundleStepInput::default())
            .presentation;
    }
    let logical_clock = LogicalCaptureClock {
        activation_tick,
        capture_tick: next_tick.saturating_sub(1),
        elapsed_steps,
        elapsed_millis: elapsed_steps.saturating_mul(16),
    };
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
        .map(|prepared| PreparedTextParityFrame {
            frame: prepared.frame,
            logical_clock,
        })
        .map_err(|error| error.to_string().into())
}

struct PreparedTextParityFrame {
    frame: PreparedFrame,
    logical_clock: LogicalCaptureClock,
}

#[derive(Clone, Copy)]
struct LogicalCaptureClock {
    activation_tick: u64,
    capture_tick: u64,
    elapsed_steps: u64,
    elapsed_millis: u64,
}

struct Args {
    bundle: PathBuf,
    checkpoint: String,
    font: PathBuf,
    additional_fonts: Vec<PathBuf>,
    output: PathBuf,
    frame_report: Option<PathBuf>,
    scope_capture_prefix: Option<PathBuf>,
    viewport: TextParityViewport,
    visual_time_millis: u64,
    advance_count: usize,
    max_ticks: u64,
    target_format: CaptureTargetFormat,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut parsed = Self {
            bundle: PathBuf::from("web/local/css-style-parity.awfb"),
            checkpoint: "default".to_owned(),
            font: PathBuf::from("web/assets/arcweft-demo.ttf"),
            additional_fonts: vec![PathBuf::from(
                "web/assets/noto-sans-jp-css-style-parity.ttf",
            )],
            output: PathBuf::from("target/css-style-parity/native-default.png"),
            frame_report: Some(PathBuf::from(
                "target/css-style-parity/native-default.frame.json",
            )),
            scope_capture_prefix: None,
            viewport: TextParityViewport::Default,
            visual_time_millis: 9_000,
            advance_count: 0,
            max_ticks: 64,
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
                "--checkpoint" => {
                    index += 1;
                    parsed.checkpoint = args
                        .get(index)
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| "--checkpoint requires a non-empty name".to_owned())?
                        .clone();
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
                "--scope-capture-prefix" => {
                    index += 1;
                    parsed.scope_capture_prefix =
                        Some(PathBuf::from(args.get(index).ok_or_else(|| {
                            "--scope-capture-prefix requires a path".to_owned()
                        })?));
                }
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
                "--advance-count" => {
                    index += 1;
                    parsed.advance_count = args
                        .get(index)
                        .ok_or_else(|| "--advance-count requires an integer".to_owned())?
                        .parse()
                        .map_err(|error| format!("--advance-count must be an integer: {error}"))?;
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
        "usage: cargo +nightly -Zscript tools/capture-text-parity-frame.rs \
         [--bundle web/local/css-style-parity.awfb] [--font web/assets/arcweft-demo.ttf] \
         [--additional-font PATH] [--no-additional-fonts] \
         [--checkpoint default] [--advance-count 0] \
         [--output target/css-style-parity/native-default.png] \
         [--frame-report target/css-style-parity/native-default.frame.json] \
         [--scope-capture-prefix PATH] [--viewport default|compact|hidpi] \
         [--visual-time-millis 9000] [--max-ticks 16] \
         [--target-format rgba8unorm|rgba8unorm-srgb]"
            .to_owned()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextParityViewport {
    Default,
    Compact,
    Hidpi,
}

impl TextParityViewport {
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

impl std::str::FromStr for TextParityViewport {
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
}

impl CaptureTargetFormat {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Rgba8Unorm => "rgba8unorm",
            Self::Rgba8UnormSrgb => "rgba8unorm-srgb",
        }
    }

    const fn wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            Self::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

impl std::str::FromStr for CaptureTargetFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "rgba8unorm" => Ok(Self::Rgba8Unorm),
            "rgba8unorm-srgb" => Ok(Self::Rgba8UnormSrgb),
            unknown => Err(format!("unknown target format `{unknown}`")),
        }
    }
}

fn write_frame_report(
    path: &Path,
    frame: &PreparedFrame,
    checkpoint: &str,
    visual_time_millis: u64,
    logical_clock: LogicalCaptureClock,
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
        "logical_clock".to_owned(),
        json!({
            "quantum_millis": 16,
            "activation_tick": logical_clock.activation_tick,
            "capture_tick": logical_clock.capture_tick,
            "elapsed_steps": logical_clock.elapsed_steps,
            "elapsed_millis": logical_clock.elapsed_millis,
        }),
    );
    object.insert(
        "execution_path".to_owned(),
        json!({
            "layout": "headless-player-scene",
            "raster": "native-shared-wgpu-offscreen",
        }),
    );
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

fn write_text_scope_capture(
    capture: &mut SharedOffscreenCapture,
    frame: &PreparedFrame,
    prefix: &Path,
    checkpoint: &str,
    visual_time_millis: u64,
) -> Result<(), Box<dyn Error>> {
    let owner = frame
        .prepared_text_owners()
        .iter()
        .rev()
        .find(|owner| {
            matches!(
                owner.kind,
                arcweft_render_wgpu::geometry::PreparedTextOwnerKind::TextBox {
                    part: arcweft_render_wgpu::geometry::PreparedTextBoxPart::Body,
                    ..
                }
            )
        })
        .ok_or("prepared frame has no TextBox body capture owner")?;
    let item = frame
        .text
        .get(owner.text)
        .ok_or("prepared TextBox body owner has no text item")?;
    let object_id_rgba = [53, 159, 212, u8::MAX];
    let scoped = capture.capture(
        frame,
        &CaptureRequest::new(
            [
                CaptureAttachment::Color,
                CaptureAttachment::Mask,
                CaptureAttachment::ObjectId,
            ],
            CaptureScope::Regions(vec![CaptureRegion::new(
                owner.semantic_id.clone(),
                owner.object_bounds,
                object_id_rgba,
            )]),
            CaptureCropPolicy::ScopeBounds,
        ),
    )?;
    let attachment_paths = [
        (CaptureAttachment::Color, "color"),
        (CaptureAttachment::Mask, "mask"),
        (CaptureAttachment::ObjectId, "object-id"),
    ]
    .into_iter()
    .map(|(attachment, suffix)| {
        let path = PathBuf::from(format!("{}.{}.png", prefix.display(), suffix));
        let rgba = scoped
            .attachment_rgba(attachment)
            .ok_or_else(|| format!("scoped capture omitted {suffix} attachment"))?;
        write_png(&path, scoped.width, scoped.height, rgba)?;
        Ok::<_, Box<dyn Error>>((suffix, path))
    })
    .collect::<Result<Vec<_>, _>>()?;
    let metadata_path = PathBuf::from(format!("{}.json", prefix.display()));
    if let Some(parent) = metadata_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &metadata_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "schema_version": "arcweft.text_scope_capture.v1",
                "checkpoint": checkpoint,
                "visual_time_millis": visual_time_millis,
                "semantic_id": owner.semantic_id.as_str(),
                "object_bounds": {
                    "x": owner.object_bounds.x,
                    "y": owner.object_bounds.y,
                    "width": owner.object_bounds.width,
                    "height": owner.object_bounds.height,
                },
                "physical_crop": {
                    "origin_x": scoped.origin_x,
                    "origin_y": scoped.origin_y,
                    "width": scoped.width,
                    "height": scoped.height,
                },
                "object_id_rgba": object_id_rgba,
                "layout_hash": hex_bytes(&item.layout.hash.as_bytes()),
                "attachments": attachment_paths
                    .iter()
                    .map(|(kind, path)| json!({
                        "kind": kind,
                        "path": path.display().to_string(),
                    }))
                    .collect::<Vec<_>>(),
            }))?
        ),
    )?;
    Ok(())
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
