#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
arcweft-bundle = { path = "../crates/arcweft-bundle" }
arcweft-player-scene = { path = "../crates/arcweft-player-scene" }
arcweft-render-wgpu = { path = "../crates/arcweft-render-wgpu" }
arcweft-runtime-driver = { path = "../crates/arcweft-runtime-driver" }
png = "0.18.1"
pollster = "0.4.0"
wgpu = { version = "29.0.3", default-features = false, features = ["std", "wgsl", "dx12", "metal", "vulkan"] }
---

use arcweft_bundle::{ArcweftBundle, BundleFormat};
use arcweft_player_scene::images::BundleImageCatalog;
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, PreparedFrame, RenderChoiceItem, RenderDialogue,
    RenderPreferences, RenderScene, RenderViewport, SharedFramePlanner,
};
use arcweft_render_wgpu::offscreen::SharedOffscreenCapture;
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions, BundleStepInput};
use png::{BitDepth, ColorType, Encoder};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args().skip(1).collect())?;
    let bundle = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &fs::read(&args.bundle)?)?;
    let frame = prepare_css_style_frame(
        &bundle,
        args.viewport.render_viewport(),
        args.max_ticks,
        args.visual_time_millis,
    )?;
    let font_bytes = fs::read(&args.font)?;
    if let Some(path) = &args.frame_report {
        write_frame_report(
            path,
            &frame,
            args.viewport.as_str(),
            args.visual_time_millis,
            &args.font,
            &font_bytes,
        )?;
    }
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(args.target_format.wgpu()))?;
    capture.register_font_bytes(font_bytes)?;
    let image = capture.capture_frame(&frame)?;
    write_png(&args.output, image.width, image.height, &image.rgba)?;
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
) -> Result<PreparedFrame, Box<dyn Error>> {
    let mut session = BundleSession::new(bundle, BundleSessionOptions::default())?;
    let images = BundleImageCatalog::from_bundle(bundle)?;
    let mut presentation = None;
    for tick in 1..=max_ticks {
        let clock = RuntimeClockStep::from_millis(tick, 16)?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let ready = step.presentation.dialogue.is_some();
        presentation = Some(step.presentation);
        if ready {
            break;
        }
    }
    let presentation = presentation.ok_or("css-style sample produced no presentation frame")?;
    if presentation.dialogue.is_none() {
        return Err(format!(
            "css-style sample did not reach a dialogue frame within {max_ticks} ticks"
        )
        .into());
    }
    let scene = RenderScene {
        dialogue: presentation
            .dialogue
            .as_ref()
            .map(RenderDialogue::from_display_frame),
        choices: presentation
            .choices
            .iter()
            .map(|choice| RenderChoiceItem {
                id: choice.id.clone(),
                label: choice.label.clone(),
            })
            .collect(),
        text_inputs: Vec::new(),
        images: images.render_images(&presentation.images, visual_time_millis)?,
        viewport,
        visual_time_millis,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
    };
    SharedFramePlanner::prepare(&scene).map_err(|error| error.to_string().into())
}

struct Args {
    bundle: PathBuf,
    font: PathBuf,
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
            output: PathBuf::from("target/css-style-parity/native-default.png"),
            frame_report: Some(PathBuf::from("target/css-style-parity/native-default.frame.json")),
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
                "--output" => {
                    index += 1;
                    parsed.output = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--output requires a path".to_owned())?,
                    );
                }
                "--frame-report" => {
                    index += 1;
                    parsed.frame_report = Some(PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--frame-report requires a path".to_owned())?,
                    ));
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
    font_path: &Path,
    font_bytes: &[u8],
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut json = String::new();
    json.push_str("{\n");
    json.push_str("  \"schema_version\": \"arcweft.css_style_native_frame_observation.v2\",\n");
    json.push_str("  \"checkpoint\": ");
    push_json_string(&mut json, checkpoint);
    json.push_str(",\n");
    json.push_str(&format!(
        "  \"visual_time_millis\": {visual_time_millis},\n"
    ));
    json.push_str("  \"font\": { \"path\": ");
    push_json_string(&mut json, &font_path.display().to_string());
    json.push_str(&format!(
        ", \"byte_len\": {}, \"fnv1a64\": \"{}\" }},\n",
        font_bytes.len(),
        fnv1a64_hex(font_bytes)
    ));
    json.push_str(&format!(
        concat!(
            "  \"viewport\": {{ ",
            "\"logical_width_milli\": {}, ",
            "\"logical_height_milli\": {}, ",
            "\"physical_width\": {}, ",
            "\"physical_height\": {}, ",
            "\"scale_factor_milli\": {} }},\n"
        ),
        f32_milli(frame.viewport.logical_width),
        f32_milli(frame.viewport.logical_height),
        frame.viewport.physical_width,
        frame.viewport.physical_height,
        f64_milli(frame.viewport.scale_factor)
    ));
    json.push_str(&format!(
        "  \"rectangle_count\": {},\n  \"image_count\": {},\n  \"text_count\": {},\n  \"styled_paragraph_count\": {},\n  \"choice_count\": {},\n",
        frame.rectangles.len(),
        frame.images.len(),
        frame.text.len() + frame.styled_paragraphs.len(),
        frame.styled_paragraphs.len(),
        frame.choices.len()
    ));
    json.push_str("  \"text\": [\n");
    for (index, text) in frame.text.iter().enumerate() {
        json.push_str("    { \"text\": ");
        push_json_string(&mut json, &text.text);
        json.push_str(&format!(
            concat!(
                ", \"bounds\": {{ ",
                "\"x_milli\": {}, ",
                "\"y_milli\": {}, ",
                "\"width_milli\": {}, ",
                "\"height_milli\": {} }}, ",
                "\"font_size_milli\": {}, ",
                "\"line_height_milli\": {}, ",
                "\"rgba\": [{}, {}, {}, {}] }}"
            ),
            f32_milli(text.bounds.x),
            f32_milli(text.bounds.y),
            f32_milli(text.bounds.width),
            f32_milli(text.bounds.height),
            f32_milli(text.font_size),
            f32_milli(text.line_height),
            text.rgba[0],
            text.rgba[1],
            text.rgba[2],
            text.rgba[3]
        ));
        if index + 1 != frame.text.len() {
            json.push(',');
        }
        json.push('\n');
    }
    json.push_str("  ],\n");
    json.push_str("  \"styled_paragraphs\": [\n");
    for (index, paragraph) in frame.styled_paragraphs.iter().enumerate() {
        json.push_str("    { \"text\": ");
        push_json_string(&mut json, &paragraph.text);
        json.push_str(&format!(
            concat!(
                ", \"bounds\": {{ ",
                "\"x_milli\": {}, ",
                "\"y_milli\": {}, ",
                "\"width_milli\": {}, ",
                "\"height_milli\": {} }}, ",
                "\"visible_end\": {}, ",
                "\"span_count\": {}, ",
                "\"spans\": ["
            ),
            f32_milli(paragraph.bounds.x),
            f32_milli(paragraph.bounds.y),
            f32_milli(paragraph.bounds.width),
            f32_milli(paragraph.bounds.height),
            paragraph.reveal.visible_end,
            paragraph.spans.len()
        ));
        for (span_index, span) in paragraph.spans.iter().enumerate() {
            json.push_str(&format!(
                concat!(
                    "{{ \"start\": {}, \"end\": {}, ",
                    "\"font_size_milli\": {}, ",
                    "\"line_height_milli\": {}, ",
                    "\"rgba\": [{}, {}, {}, {}] }}"
                ),
                span.range.start,
                span.range.end,
                f32_milli(span.style.font_size),
                f32_milli(span.style.line_height),
                span.style.color[0],
                span.style.color[1],
                span.style.color[2],
                span.style.color[3]
            ));
            if span_index + 1 != paragraph.spans.len() {
                json.push_str(", ");
            }
        }
        json.push_str("] }");
        if index + 1 != frame.styled_paragraphs.len() {
            json.push(',');
        }
        json.push('\n');
    }
    json.push_str("  ]\n}\n");
    fs::write(path, json)?;
    Ok(())
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if ch.is_control() => output.push_str(&format!("\\u{:04x}", u32::from(ch))),
            ch => output.push(ch),
        }
    }
    output.push('"');
}

fn f32_milli(value: f32) -> i64 {
    f64_milli(f64::from(value))
}

fn f64_milli(value: f64) -> i64 {
    let scaled = (value * 1_000.0).round();
    if !scaled.is_finite() {
        return 0;
    }
    if scaled < i64::MIN as f64 {
        i64::MIN
    } else if scaled > i64::MAX as f64 {
        i64::MAX
    } else {
        scaled as i64
    }
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
