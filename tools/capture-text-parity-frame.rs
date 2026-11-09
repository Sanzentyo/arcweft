#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
arcweft-layout = { path = "../crates/arcweft-layout" }
arcweft-player-scene = { path = "../crates/arcweft-player-scene" }
arcweft-player-web = { path = "../crates/arcweft-player-web" }
arcweft-presentation = { path = "../crates/arcweft-presentation" }
arcweft-render-wgpu = { path = "../crates/arcweft-render-wgpu" }
arcweft-runtime-driver = { path = "../crates/arcweft-runtime-driver" }
arcweft-text-model = { path = "../crates/arcweft-text-model" }
png = "0.18.1"
pollster = "0.4.0"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.150"
wgpu = { version = "29.0.3", default-features = false, features = ["std", "wgsl", "dx12", "metal", "vulkan"] }

[patch.crates-io]
glyphon = { path = "../vendor/glyphon" }
---

// Captures the shared headless-prepared text frame through native WGPU.

use arcweft_layout::ScalePolicy;
use arcweft_player_scene::{
    frame::{PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest, PlayerFrameTime},
    images::BundleImageCatalog,
    input::{DialogueProgress, InputController},
};
use arcweft_player_web::report::WebFrameObservationReport;
use arcweft_presentation::input::KeyPhase;
use arcweft_render_wgpu::geometry::{PreparedFrame, RenderPreferences, RenderViewport};
use arcweft_render_wgpu::offscreen::{
    CaptureAttachment, CaptureCropPolicy, CaptureRegion, CaptureRequest, CaptureScope,
    SharedOffscreenCapture,
};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions, BundleStepInput};
use png::{BitDepth, ColorType, Encoder};
use serde::Serialize;
use serde_json::json;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const LOGICAL_STEP_MILLIS: u32 = 16;

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args().skip(1).collect())?;
    let bundle_bytes = fs::read(&args.bundle)?;
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
        &bundle_bytes,
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
    bundle_bytes: &[u8],
    viewport: RenderViewport,
    max_ticks: u64,
    visual_time_millis: u64,
    mut advance_count: usize,
    font_resources: &[Vec<u8>],
) -> Result<PreparedTextParityFrame, Box<dyn Error>> {
    let (mut session, artifact) =
        BundleSession::from_awfb_bytes_with_bundle(bundle_bytes, BundleSessionOptions::default())?;
    let images = BundleImageCatalog::from_bundle(artifact.bundle())?;
    let mut planner = PlayerFramePlannerState::new();
    for font_bytes in font_resources {
        planner.register_font_bytes(font_bytes.clone())?;
    }
    let mut input = InputController::default();
    let mut previous_page = None;
    let mut activation_tick = None;
    let quantum = u64::from(LOGICAL_STEP_MILLIS);
    for tick in 1..=max_ticks {
        let clock = RuntimeClockStep::from_millis(tick, LOGICAL_STEP_MILLIS)?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let Some((dialogue, entry)) = step.presentation.dialogue.latest_active() else {
            continue;
        };
        let page = (
            dialogue.id(),
            entry.id(),
            entry.stage_index(),
            entry.page_index(),
        );
        if previous_page == Some(page) || dialogue.advance_target().is_none() {
            continue;
        }
        if advance_count == 0 {
            activation_tick = Some(tick);
            break;
        }
        let frame = prepare_frame(
            &session,
            &images,
            &mut planner,
            &mut input,
            viewport,
            tick.checked_mul(quantum).ok_or("clock time overflow")?,
        )?;
        match input
            .keyboard(&frame, "Enter", KeyPhase::Down)
            .dialogue_progress
        {
            DialogueProgress::Reveal { target } => session.queue_dialogue_reveal_completion(target),
            DialogueProgress::Advance { target } => {
                session.queue_dialogue_advance(target);
                previous_page = Some(page);
                advance_count -= 1;
            }
            DialogueProgress::None => {
                return Err(format!("dialogue page at tick {tick} did not handle Enter").into());
            }
        }
    }
    let Some(activation_tick) = activation_tick else {
        return Err(format!(
            "text parity sample did not reach the requested dialogue page within {max_ticks} ticks (remaining advances: {advance_count})"
        )
        .into());
    };
    let elapsed_steps = visual_time_millis.div_ceil(quantum);
    let capture_tick = activation_tick
        .checked_add(elapsed_steps)
        .ok_or("capture tick overflow")?;
    let capture_millis = capture_tick
        .checked_mul(quantum)
        .ok_or("capture time overflow")?;
    for offset in 1..=elapsed_steps {
        let tick = activation_tick + offset;
        let clock = RuntimeClockStep::from_millis(tick, LOGICAL_STEP_MILLIS)?;
        session.step_with_clock(clock, BundleStepInput::default());
    }
    let logical_clock = LogicalCaptureClock {
        quantum_millis: LOGICAL_STEP_MILLIS,
        activation_tick,
        capture_tick,
        elapsed_steps,
        elapsed_millis: elapsed_steps * quantum,
    };
    let frame = prepare_frame(
        &session,
        &images,
        &mut planner,
        &mut input,
        viewport,
        capture_millis,
    )?;
    Ok(PreparedTextParityFrame {
        frame,
        logical_clock,
    })
}

fn prepare_frame(
    session: &BundleSession,
    images: &BundleImageCatalog,
    planner: &mut PlayerFramePlannerState,
    input: &mut InputController,
    viewport: RenderViewport,
    clock_millis: u64,
) -> Result<PreparedFrame, Box<dyn Error>> {
    let style_environment = session.presentation_environment();
    let candidate = planner.prepare_candidate(
        input,
        PlayerFrameRequest {
            presentation: session.presentation(),
            style_program: session.view_style_program(),
            style_environment: &style_environment,
            style_palettes: session.view_style_palettes(),
            fx_definitions: session.fx_definitions(),
            images,
            viewport,
            fit: PlayerFrameFit::design_1280x720(ScalePolicy::Contain),
            time: PlayerFrameTime::runtime(clock_millis),
            preferences: RenderPreferences::default(),
        },
    )?;
    let (prepared, ()) = planner
        .publication_guard()
        .publish_with(candidate, input, |_| ())?;
    Ok(prepared.frame)
}

struct PreparedTextParityFrame {
    frame: PreparedFrame,
    logical_clock: LogicalCaptureClock,
}

#[derive(Clone, Copy, Serialize)]
struct LogicalCaptureClock {
    quantum_millis: u32,
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
            bundle: PathBuf::from("web/local/native-style-parity.awfb"),
            checkpoint: "default".to_owned(),
            font: PathBuf::from("web/assets/arcweft-demo.ttf"),
            additional_fonts: vec![PathBuf::from(
                "web/assets/noto-sans-jp-native-style-parity.ttf",
            )],
            output: PathBuf::from("target/native-style-parity/native-default.png"),
            frame_report: Some(PathBuf::from(
                "target/native-style-parity/native-default.frame.json",
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
         [--bundle web/local/native-style-parity.awfb] [--font web/assets/arcweft-demo.ttf] \
         [--additional-font PATH] [--no-additional-fonts] \
         [--checkpoint default] [--advance-count 0] \
         [--output target/native-style-parity/native-default.png] \
         [--frame-report target/native-style-parity/native-default.frame.json] \
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

#[derive(Serialize)]
struct NativeFrameReport<'a> {
    #[serde(flatten)]
    observation: WebFrameObservationReport,
    checkpoint: &'a str,
    visual_time_millis: u64,
    logical_clock: LogicalCaptureClock,
    execution_path: NativeFrameExecutionPath,
    fonts: Vec<NativeFrameFont>,
}

#[derive(Serialize)]
struct NativeFrameExecutionPath {
    layout: &'static str,
    raster: &'static str,
}

#[derive(Serialize)]
struct NativeFrameFont {
    path: String,
    byte_len: usize,
    fnv1a64: String,
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
    let report = NativeFrameReport {
        observation: WebFrameObservationReport::from_prepared_frame(frame)?,
        checkpoint,
        visual_time_millis,
        logical_clock,
        execution_path: NativeFrameExecutionPath {
            layout: "headless-player-scene",
            raster: "native-shared-wgpu-offscreen",
        },
        fonts: font_paths
            .iter()
            .zip(font_resources)
            .map(|(path, bytes)| NativeFrameFont {
                path: path.display().to_string(),
                byte_len: bytes.len(),
                fnv1a64: fnv1a64_hex(bytes),
            })
            .collect(),
    };
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
                arcweft_text_model::PreparedTextOwnerKind::DialogueView {
                    role: arcweft_text_model::DialoguePreparedTextRole::Content,
                    ..
                }
            )
        })
        .ok_or("prepared frame has no dialogue View content capture owner")?;
    let item = frame
        .text
        .get(owner.text)
        .ok_or("prepared dialogue View content owner has no text item")?;
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
