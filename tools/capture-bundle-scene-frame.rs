#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "capture-bundle-scene-frame"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false

[dependencies]
arcweft-bundle = { path = "../crates/arcweft-bundle" }
arcweft-player-scene = { path = "../crates/arcweft-player-scene" }
arcweft-render-wgpu = { path = "../crates/arcweft-render-wgpu" }
arcweft-runtime-driver = { path = "../crates/arcweft-runtime-driver" }
png = "0.18.1"
pollster = "0.4.0"
wgpu = { version = "29.0.3", default-features = false, features = ["std", "wgsl", "dx12", "metal", "vulkan"] }
---

use arcweft_bundle::ArcweftBundle;
use arcweft_player_scene::images::BundleImageCatalog;
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, RenderChoiceItem, RenderDialogue, RenderPreferences,
    RenderScene, RenderViewport, SharedFramePlanner,
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
    let bundle = ArcweftBundle::from_json_slice(&fs::read(&args.bundle)?)?;
    let frame = prepare_frame(&bundle, &args)?;
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(args.target_format.wgpu()))?;
    if let Some(font) = &args.font {
        capture.register_font_bytes(fs::read(font)?)?;
    }
    let image = capture.capture_frame(&frame)?;
    write_png(&args.output, image.width, image.height, &image.rgba)?;
    println!(
        "wrote {} ({}x{}, images={}, choices={}, visual_time_millis={})",
        args.output.display(),
        image.width,
        image.height,
        frame.images.len(),
        frame.choices.len(),
        args.visual_time_millis
    );
    Ok(())
}

#[derive(Clone, Debug)]
struct Args {
    bundle: PathBuf,
    output: PathBuf,
    font: Option<PathBuf>,
    width: u32,
    height: u32,
    max_ticks: u64,
    visual_time_millis: u64,
    target_format: CaptureTargetFormat,
    verbose: bool,
    select_choice: Option<String>,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut bundle = None;
        let mut output = None;
        let mut result = Self {
            bundle: PathBuf::new(),
            output: PathBuf::new(),
            font: Some(PathBuf::from("web/assets/arcweft-demo.ttf")),
            width: 1280,
            height: 720,
            max_ticks: 16,
            visual_time_millis: 160,
            target_format: CaptureTargetFormat::Rgba8UnormSrgb,
            verbose: false,
            select_choice: None,
        };
        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--output" => output = Some(PathBuf::from(required_value(&mut iter, "--output")?)),
                "--font" => result.font = Some(PathBuf::from(required_value(&mut iter, "--font")?)),
                "--no-font" => result.font = None,
                "--width" => result.width = parse_u32(&required_value(&mut iter, "--width")?)?,
                "--height" => result.height = parse_u32(&required_value(&mut iter, "--height")?)?,
                "--max-ticks" => {
                    result.max_ticks = parse_u64(&required_value(&mut iter, "--max-ticks")?)?;
                }
                "--visual-time-millis" => {
                    result.visual_time_millis =
                        parse_u64(&required_value(&mut iter, "--visual-time-millis")?)?;
                }
                "--target-format" => {
                    result.target_format =
                        CaptureTargetFormat::parse(&required_value(&mut iter, "--target-format")?)?;
                }
                "--verbose" => result.verbose = true,
                "--select-choice" => {
                    result.select_choice = Some(required_value(&mut iter, "--select-choice")?);
                }
                "--help" | "-h" => return Err(help()),
                value if value.starts_with('-') => {
                    return Err(format!("unknown option `{value}`\n\n{}", help()));
                }
                value => bundle = Some(PathBuf::from(value)),
            }
        }
        result.bundle = bundle.ok_or_else(help)?;
        result.output = output.ok_or_else(|| "--output is required".to_owned())?;
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug)]
enum CaptureTargetFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
}

impl CaptureTargetFormat {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "rgba8unorm" => Ok(Self::Rgba8Unorm),
            "rgba8unorm-srgb" | "rgba8unorm_srgb" => Ok(Self::Rgba8UnormSrgb),
            _ => Err(format!("unsupported target format `{value}`")),
        }
    }

    const fn wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            Self::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

fn prepare_frame(
    bundle: &ArcweftBundle,
    args: &Args,
) -> Result<arcweft_render_wgpu::geometry::PreparedFrame, Box<dyn Error>> {
    let mut session = BundleSession::new(bundle, BundleSessionOptions::default())?;
    let images = BundleImageCatalog::from_bundle(bundle)?;
    let mut presentation = None;
    let mut selected = args.select_choice.is_none();
    for tick in 1..=args.max_ticks {
        let step = session.step_with_clock(
            RuntimeClockStep::from_millis(tick, 16)?,
            BundleStepInput::default(),
        );
        if args.verbose {
            eprintln!(
                "tick={tick} line_effects={:?} presentation_images={}",
                step.line_effects,
                step.presentation.images.len()
            );
        }
        if !selected && !step.presentation.choices.is_empty() {
            if let Some(choice) = &args.select_choice {
                session.queue_choice_selection(choice.clone());
                selected = true;
            }
            presentation = Some(step.presentation);
            continue;
        }
        presentation = Some(step.presentation);
        if selected
            && presentation
            .as_ref()
            .is_some_and(|presentation| {
                !presentation.images.is_empty()
                    && (presentation.dialogue.is_some() || !presentation.choices.is_empty())
            })
        {
            break;
        }
    }
    let presentation = presentation.ok_or("bundle did not produce a presentation frame")?;
    let viewport = RenderViewport {
        logical_width: args.width as f32,
        logical_height: args.height as f32,
        physical_width: args.width,
        physical_height: args.height,
        scale_factor: 1.0,
    };
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
        images: images.render_images(&presentation.images, args.visual_time_millis)?,
        viewport,
        visual_time_millis: args.visual_time_millis,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
    };
    SharedFramePlanner::prepare(&scene).map_err(|error| error.to_string().into())
}

fn required_value(iter: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    iter.next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_u32(value: &str) -> Result<u32, String> {
    value
        .parse()
        .map_err(|error| format!("invalid integer `{value}`: {error}"))
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|error| format!("invalid integer `{value}`: {error}"))
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()?;
    fs::write(path, bytes)?;
    Ok(())
}

fn help() -> String {
    "usage: cargo +nightly -Zscript tools/capture-bundle-scene-frame.rs BUNDLE --output PATH [--width 1280] [--height 720] [--select-choice ID]"
        .to_owned()
}
