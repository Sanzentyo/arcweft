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
serde_json = "1.0.150"
wgpu = { version = "29.0.3", default-features = false, features = ["std", "wgsl", "dx12", "metal", "vulkan"] }
---

use arcweft_bundle::{ArcweftBundle, BundleFormat};
use arcweft_player_scene::images::BundleImageCatalog;
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, PreparedFrame, RenderChoiceItem, RenderDialogue,
    RenderFontFamily, RenderGlyphTransformKind, RenderPreferences, RenderScene, RenderTextBlock,
    RenderTextSlant, RenderTextWeight, RenderViewport, SharedFramePlanner,
};
use arcweft_render_wgpu::offscreen::SharedOffscreenCapture;
use arcweft_render_wgpu::renderer::{
    StyledParagraphEvidenceFontContext, StyledParagraphGlyphBounds,
    StyledParagraphGlyphTransformEvidence, StyledParagraphLayoutEvidence, StyledParagraphLineBox,
    StyledParagraphRevealState, StyledParagraphStyleEvidence, StyledParagraphTransformSupport,
};
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{BundleSession, BundleSessionOptions, BundleStepInput};
use png::{BitDepth, ColorType, Encoder};
use serde_json::{Value, json};
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
    let mut evidence_context = StyledParagraphEvidenceFontContext::new();
    evidence_context.register_font_bytes(font_bytes.to_vec())?;
    let paragraph_evidence = evidence_context.frame_styled_paragraph_layout_evidence(frame);
    let report = json!({
        "schema_version": "arcweft.css_style_native_frame_observation.v3",
        "checkpoint": checkpoint,
        "visual_time_millis": visual_time_millis,
        "font": {
            "path": font_path.display().to_string(),
            "byte_len": font_bytes.len(),
            "fnv1a64": fnv1a64_hex(font_bytes),
        },
        "viewport": {
            "logical_width_milli": f32_milli(frame.viewport.logical_width),
            "logical_height_milli": f32_milli(frame.viewport.logical_height),
            "physical_width": frame.viewport.physical_width,
            "physical_height": frame.viewport.physical_height,
            "scale_factor_milli": f64_milli(frame.viewport.scale_factor),
        },
        "rectangle_count": frame.rectangles.len(),
        "image_count": frame.images.len(),
        "text_count": frame.text.len() + frame.styled_paragraphs.len(),
        "styled_paragraph_count": frame.styled_paragraphs.len(),
        "choice_count": frame.choices.len(),
        "text": frame.text.iter().map(text_json).collect::<Vec<_>>(),
        "styled_paragraphs": frame.styled_paragraphs.iter().zip(&paragraph_evidence).map(
            |(paragraph, evidence)| styled_paragraph_json(paragraph, evidence)
        ).collect::<Vec<_>>(),
    });
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    Ok(())
}

fn text_json(text: &RenderTextBlock) -> Value {
    json!({
        "text": text.text,
        "bounds": bounds_json_values(text.bounds.x, text.bounds.y, text.bounds.width, text.bounds.height),
        "font_size_milli": f32_milli(text.font_size),
        "line_height_milli": f32_milli(text.line_height),
        "rgba": text.rgba,
    })
}

fn styled_paragraph_json(
    paragraph: &arcweft_render_wgpu::geometry::RenderStyledParagraph,
    evidence: &StyledParagraphLayoutEvidence,
) -> Value {
    let line_boxes = evidence
        .line_boxes
        .iter()
        .map(line_box_json)
        .collect::<Vec<_>>();
    let glyph_bounds = evidence
        .glyph_bounds
        .iter()
        .map(glyph_bounds_json)
        .collect::<Vec<_>>();
    let glyph_transforms = evidence
        .glyph_transforms
        .iter()
        .map(glyph_transform_json)
        .collect::<Vec<_>>();
    json!({
        "text": paragraph.text,
        "bounds": bounds_json_values(evidence.bounds.x, evidence.bounds.y, evidence.bounds.width, evidence.bounds.height),
        "text_len": evidence.text_len,
        "visible_end": evidence.visible_end,
        "default_style": style_evidence_json(&evidence.default_style),
        "span_count": evidence.spans.len(),
        "line_box_count": line_boxes.len(),
        "glyph_count": glyph_bounds.len(),
        "glyph_transform_count": glyph_transforms.len(),
        "transform_support": transform_support_label(evidence.transform_support),
        "spans": evidence.spans.iter().map(|span| {
            let style = style_evidence_json(&span.style);
            json!({
                "start": span.range.start,
                "end": span.range.end,
                "node_index": span.node_index,
                "font_size_milli": f32_milli(span.style.font_size),
                "line_height_milli": f32_milli(span.style.line_height),
                "rgba": span.style.rgba,
                "style": style,
            })
        }).collect::<Vec<_>>(),
        "line_boxes": line_boxes,
        "glyph_bounds": glyph_bounds,
        "glyph_transforms": glyph_transforms,
    })
}

fn line_box_json(line: &StyledParagraphLineBox) -> Value {
    json!({
        "line_index": line.line_index,
        "bounds": bounds_json_values(line.bounds.x, line.bounds.y, line.bounds.width, line.bounds.height),
    })
}

fn glyph_bounds_json(glyph: &StyledParagraphGlyphBounds) -> Value {
    json!({
        "source_start": glyph.source_range.start,
        "source_end": glyph.source_range.end,
        "line_index": glyph.line_index,
        "bounds": bounds_json_values(glyph.bounds.x, glyph.bounds.y, glyph.bounds.width, glyph.bounds.height),
        "visible": glyph.visible,
        "reveal_state": reveal_state_label(glyph.reveal_state),
        "style": style_evidence_json(&glyph.style),
        "glyph_transform": glyph.glyph_transform.as_ref().map(glyph_transform_json),
    })
}

fn glyph_transform_json(transform: &StyledParagraphGlyphTransformEvidence) -> Value {
    json!({
        "source_start": transform.range.start,
        "source_end": transform.range.end,
        "node_index": transform.node_index,
        "kind": glyph_transform_kind_label(transform.motion.kind),
        "amplitude_milli": f32_milli(transform.motion.amplitude),
        "frequency_milli": f32_milli(transform.motion.frequency),
        "sampled_offset_y_milli": f32_milli(transform.sampled_offset_y),
        "rendered": transform.rendered,
        "support": "metadata_only_unsupported",
    })
}

fn style_evidence_json(style: &StyledParagraphStyleEvidence) -> Value {
    json!({
        "font_size_milli": f32_milli(style.font_size),
        "line_height_milli": f32_milli(style.line_height),
        "rgba": style.rgba,
        "font_family": font_family_label(&style.font_family),
        "weight": text_weight_label(style.weight),
        "slant": text_slant_label(style.slant),
    })
}

fn bounds_json_values(x: f32, y: f32, width: f32, height: f32) -> Value {
    json!({
        "x_milli": f32_milli(x),
        "y_milli": f32_milli(y),
        "width_milli": f32_milli(width),
        "height_milli": f32_milli(height),
    })
}

fn font_family_label(family: &RenderFontFamily) -> &str {
    match family {
        RenderFontFamily::Serif => "serif",
        RenderFontFamily::SansSerif => "sans_serif",
        RenderFontFamily::Monospace => "monospace",
        RenderFontFamily::Cursive => "cursive",
        RenderFontFamily::Fantasy => "fantasy",
        RenderFontFamily::Named(name) => name.as_str(),
    }
}

fn text_weight_label(weight: RenderTextWeight) -> &'static str {
    match weight {
        RenderTextWeight::Regular => "regular",
        RenderTextWeight::Bold => "bold",
    }
}

fn text_slant_label(slant: RenderTextSlant) -> &'static str {
    match slant {
        RenderTextSlant::Upright => "upright",
        RenderTextSlant::Italic => "italic",
    }
}

fn reveal_state_label(state: StyledParagraphRevealState) -> &'static str {
    match state {
        StyledParagraphRevealState::Visible => "visible",
        StyledParagraphRevealState::PartiallyVisible => "partially_visible",
        StyledParagraphRevealState::Hidden => "hidden",
    }
}

fn transform_support_label(support: StyledParagraphTransformSupport) -> &'static str {
    match support {
        StyledParagraphTransformSupport::NoTransforms => "no_transforms",
        StyledParagraphTransformSupport::MetadataOnlyUnsupported => "metadata_only_unsupported",
    }
}

fn glyph_transform_kind_label(kind: RenderGlyphTransformKind) -> &'static str {
    match kind {
        RenderGlyphTransformKind::Wave => "wave",
        RenderGlyphTransformKind::Shake => "shake",
        RenderGlyphTransformKind::Jitter => "jitter",
    }
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
