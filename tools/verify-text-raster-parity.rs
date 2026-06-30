#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
png = "0.18.1"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.150"
---

use png::{BitDepth, ColorType, Decoder};
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--self-test") {
        run_self_test()?;
        println!("verify-text-raster-parity self-test passed");
        return Ok(());
    }

    let args = Args::parse(raw_args)?;
    let native = RgbaImage::read(&args.native)?;
    let web = RgbaImage::read(&args.web)?;
    let native_frame = FrameObservation::read(&args.native_frame)?;
    let web_frame = FrameObservation::read(&args.web_frame)?;
    let font = args
        .font
        .as_ref()
        .map(|path| FontFingerprint::read(path))
        .transpose()?;

    let report = compare_text_raster(
        &native,
        &web,
        &native_frame,
        &web_frame,
        &args,
        font,
    );
    report.write_json(&args.report)?;
    println!(
        "text raster parity: checkpoint={}, passed={}, runs={}, max_mask_xor_ratio={:.6}, max_bbox_delta_px={:.3}, max_centroid_delta_px={:.3}, max_coverage_delta_ratio={:.6}",
        report.checkpoint,
        report.passed,
        report.runs.len(),
        report.aggregate.max_mask_xor_ratio,
        report.aggregate.max_bbox_delta_px,
        report.aggregate.max_centroid_delta_px,
        report.aggregate.max_coverage_delta_ratio
    );

    if report.passed {
        Ok(())
    } else {
        Err(format!(
            "text raster parity failed for {}: {}",
            report.checkpoint,
            report.failure_reasons.join("; ")
        )
        .into())
    }
}

#[derive(Clone, Debug)]
struct Args {
    checkpoint: String,
    native: PathBuf,
    web: PathBuf,
    native_frame: PathBuf,
    web_frame: PathBuf,
    report: PathBuf,
    font: Option<PathBuf>,
    ink_affinity_threshold: f64,
    thresholds: TextRasterThresholds,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut parsed = Self {
            checkpoint: "default".to_owned(),
            native: PathBuf::from("target/css-style-parity/native-default.png"),
            web: PathBuf::from("target/css-style-parity/web-default.png"),
            native_frame: PathBuf::from("target/css-style-parity/native-default.frame.json"),
            web_frame: PathBuf::from("target/css-style-parity/web-default.frame.json"),
            report: PathBuf::from("target/css-style-parity/text-raster-default.json"),
            font: Some(PathBuf::from("web/assets/arcweft-demo.ttf")),
            ink_affinity_threshold: 0.35,
            thresholds: TextRasterThresholds::default(),
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--checkpoint" => {
                    index += 1;
                    parsed.checkpoint = args
                        .get(index)
                        .ok_or_else(|| "--checkpoint requires a name".to_owned())?
                        .to_owned();
                }
                "--native" => {
                    index += 1;
                    parsed.native = path_arg(&args, index, "--native")?;
                }
                "--web" => {
                    index += 1;
                    parsed.web = path_arg(&args, index, "--web")?;
                }
                "--native-frame" => {
                    index += 1;
                    parsed.native_frame = path_arg(&args, index, "--native-frame")?;
                }
                "--web-frame" => {
                    index += 1;
                    parsed.web_frame = path_arg(&args, index, "--web-frame")?;
                }
                "--report" => {
                    index += 1;
                    parsed.report = path_arg(&args, index, "--report")?;
                }
                "--font" => {
                    index += 1;
                    parsed.font = Some(path_arg(&args, index, "--font")?);
                }
                "--no-font-fingerprint" => parsed.font = None,
                "--ink-affinity-threshold" => {
                    index += 1;
                    parsed.ink_affinity_threshold = parse_f64(&args, index, "--ink-affinity-threshold")?;
                }
                "--layout-milli-tolerance" => {
                    index += 1;
                    parsed.thresholds.layout_milli_tolerance = parse_i64(&args, index, "--layout-milli-tolerance")?;
                }
                "--max-bbox-delta-px" => {
                    index += 1;
                    parsed.thresholds.max_bbox_delta_px = parse_f64(&args, index, "--max-bbox-delta-px")?;
                }
                "--max-centroid-delta-px" => {
                    index += 1;
                    parsed.thresholds.max_centroid_delta_px = parse_f64(&args, index, "--max-centroid-delta-px")?;
                }
                "--max-coverage-delta-ratio" => {
                    index += 1;
                    parsed.thresholds.max_coverage_delta_ratio =
                        parse_f64(&args, index, "--max-coverage-delta-ratio")?;
                }
                "--max-mask-xor-ratio" => {
                    index += 1;
                    parsed.thresholds.max_mask_xor_ratio = parse_f64(&args, index, "--max-mask-xor-ratio")?;
                }
                "--min-ink-pixels" => {
                    index += 1;
                    parsed.thresholds.min_ink_pixels = parse_usize(&args, index, "--min-ink-pixels")?;
                }
                "--help" | "-h" => return Err(Self::usage()),
                unknown => return Err(format!("unknown argument `{unknown}`\n{}", Self::usage())),
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn usage() -> String {
        "usage: cargo +nightly -Zscript tools/verify-text-raster-parity.rs \
         [--checkpoint default] \
         [--native target/css-style-parity/native-default.png] \
         [--web target/css-style-parity/web-default.png] \
         [--native-frame target/css-style-parity/native-default.frame.json] \
         [--web-frame target/css-style-parity/web-default.frame.json] \
         [--report target/css-style-parity/text-raster-default.json] \
         [--font web/assets/arcweft-demo.ttf] \
         [--ink-affinity-threshold 0.35] \
         [--max-bbox-delta-px 2.0] [--max-centroid-delta-px 1.25] \
         [--max-coverage-delta-ratio 0.15] [--max-mask-xor-ratio 0.45]"
            .to_owned()
    }
}

fn path_arg(args: &[String], index: usize, name: &str) -> Result<PathBuf, String> {
    args.get(index)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{name} requires a path"))
}

fn parse_f64(args: &[String], index: usize, name: &str) -> Result<f64, String> {
    let value = args
        .get(index)
        .ok_or_else(|| format!("{name} requires a number"))?
        .parse::<f64>()
        .map_err(|error| format!("{name} must be a number: {error}"))?;
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| format!("{name} must be finite"))
}

fn parse_i64(args: &[String], index: usize, name: &str) -> Result<i64, String> {
    args.get(index)
        .ok_or_else(|| format!("{name} requires an integer"))?
        .parse()
        .map_err(|error| format!("{name} must be an integer: {error}"))
}

fn parse_usize(args: &[String], index: usize, name: &str) -> Result<usize, String> {
    args.get(index)
        .ok_or_else(|| format!("{name} requires an integer"))?
        .parse()
        .map_err(|error| format!("{name} must be an integer: {error}"))
}

#[derive(Clone, Debug)]
struct RgbaImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl RgbaImage {
    fn read(path: &Path) -> Result<Self, Box<dyn Error>> {
        let file = fs::File::open(path)?;
        let decoder = Decoder::new(BufReader::new(file));
        let mut reader = decoder.read_info()?;
        let buffer_size = reader
            .output_buffer_size()
            .ok_or("PNG output buffer size overflowed")?;
        let mut buffer = vec![0; buffer_size];
        let info = reader.next_frame(&mut buffer)?;
        if !matches!(info.color_type, ColorType::Rgb | ColorType::Rgba)
            || info.bit_depth != BitDepth::Eight
        {
            return Err(format!(
                "{} must be an 8-bit RGB or RGBA PNG, got {:?} {:?}",
                path.display(),
                info.color_type,
                info.bit_depth
            )
            .into());
        }
        buffer.truncate(info.buffer_size());
        let rgba = match info.color_type {
            ColorType::Rgba => buffer,
            ColorType::Rgb => buffer
                .chunks_exact(3)
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
                .collect(),
            _ => unreachable!("color type was checked above"),
        };
        Ok(Self {
            width: info.width,
            height: info.height,
            rgba,
        })
    }

    fn blank(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let pixel_count = usize::try_from(width)
            .unwrap_or(0)
            .saturating_mul(usize::try_from(height).unwrap_or(0));
        Self {
            width,
            height,
            rgba: std::iter::repeat_n(rgba, pixel_count)
                .flatten()
                .collect(),
        }
    }

    fn fill_rect(&mut self, rect: PixelRect, rgba: [u8; 4]) {
        for y in rect.top..rect.bottom.min(self.height) {
            for x in rect.left..rect.right.min(self.width) {
                let Some(index) = pixel_index(self.width, x, y) else {
                    continue;
                };
                self.rgba[index..index + 4].copy_from_slice(&rgba);
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct FrameObservation {
    #[serde(default)]
    schema_version: String,
    viewport: FrameViewport,
    #[serde(default)]
    text_count: usize,
    #[serde(default)]
    styled_paragraph_count: usize,
    #[serde(default)]
    text: Vec<FrameText>,
    #[serde(default)]
    styled_paragraphs: Vec<FrameStyledParagraph>,
}

impl FrameObservation {
    fn read(path: &Path) -> Result<Self, Box<dyn Error>> {
        let bytes = fs::read(path)?;
        serde_json::from_slice(&bytes).map_err(Into::into)
    }

    fn text_count_for_report(&self) -> usize {
        self.text_count
            .max(self.text.len() + self.styled_paragraph_count)
            .max(self.text_runs_for_report().len())
    }

    fn text_runs_for_report(&self) -> Vec<FrameText> {
        self.text
            .iter()
            .cloned()
            .chain(
                self.styled_paragraphs
                    .iter()
                    .flat_map(FrameStyledParagraph::text_runs_for_report),
            )
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct FrameViewport {
    logical_width_milli: i64,
    logical_height_milli: i64,
    physical_width: u32,
    physical_height: u32,
    scale_factor_milli: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct FrameText {
    text: String,
    bounds: FrameBounds,
    font_size_milli: i64,
    line_height_milli: i64,
    rgba: [u8; 4],
}

#[derive(Clone, Debug, Deserialize)]
struct FrameStyledParagraph {
    text: String,
    bounds: FrameBounds,
    #[serde(default)]
    visible_end: usize,
    #[serde(default)]
    span_count: usize,
    #[serde(default)]
    spans: Vec<FrameStyledSpan>,
}

impl FrameStyledParagraph {
    fn text_runs_for_report(&self) -> Vec<FrameText> {
        self.spans
            .iter()
            .take(self.span_count.max(self.spans.len()))
            .filter_map(|span| {
                let start = span.start.min(self.text.len());
                let end = span.end.min(self.text.len());
                if start >= end {
                    return None;
                }
                let visible_end = self.visible_end.min(self.text.len());
                let text = self
                    .text
                    .get(start..end.min(visible_end))
                    .unwrap_or("")
                    .to_owned();
                Some(FrameText {
                    text,
                    bounds: self.bounds,
                    font_size_milli: span.font_size_milli,
                    line_height_milli: span.line_height_milli,
                    rgba: span.rgba,
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug, Deserialize)]
struct FrameStyledSpan {
    start: usize,
    end: usize,
    font_size_milli: i64,
    line_height_milli: i64,
    rgba: [u8; 4],
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct FrameBounds {
    x_milli: i64,
    y_milli: i64,
    width_milli: i64,
    height_milli: i64,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct TextRasterThresholds {
    layout_milli_tolerance: i64,
    max_bbox_delta_px: f64,
    max_centroid_delta_px: f64,
    max_coverage_delta_ratio: f64,
    max_mask_xor_ratio: f64,
    min_ink_pixels: usize,
}

impl Default for TextRasterThresholds {
    fn default() -> Self {
        Self {
            layout_milli_tolerance: 0,
            max_bbox_delta_px: 2.0,
            max_centroid_delta_px: 1.25,
            max_coverage_delta_ratio: 0.15,
            max_mask_xor_ratio: 0.45,
            min_ink_pixels: 4,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct TextRasterReport {
    schema_version: &'static str,
    checkpoint: String,
    contract: &'static str,
    passed: bool,
    failure_reasons: Vec<String>,
    native: CaptureSummary,
    web: CaptureSummary,
    font: Option<FontFingerprint>,
    ink_affinity_threshold: f64,
    thresholds: TextRasterThresholds,
    aggregate: AggregateReport,
    runs: Vec<TextRunReport>,
}

impl TextRasterReport {
    fn write_json(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, format!("{}\n", serde_json::to_string_pretty(self)?))?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
struct CaptureSummary {
    png_path: String,
    frame_path: String,
    frame_schema_version: String,
    png_width: u32,
    png_height: u32,
    logical_width_milli: i64,
    logical_height_milli: i64,
    physical_width: u32,
    physical_height: u32,
    scale_factor_milli: i64,
    text_count: usize,
}

#[derive(Clone, Debug, Serialize)]
struct FontFingerprint {
    path: String,
    byte_len: usize,
    fnv1a64: String,
}

impl FontFingerprint {
    fn read(path: &Path) -> Result<Self, Box<dyn Error>> {
        let bytes = fs::read(path)?;
        Ok(Self {
            path: path.display().to_string(),
            byte_len: bytes.len(),
            fnv1a64: fnv1a64_hex(&bytes),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
struct AggregateReport {
    max_mask_xor_ratio: f64,
    max_bbox_delta_px: f64,
    max_centroid_delta_px: f64,
    max_coverage_delta_ratio: f64,
    layout_mismatch_count: usize,
    failed_run_count: usize,
}

#[derive(Clone, Debug, Serialize)]
struct TextRunReport {
    index: usize,
    text: String,
    passed: bool,
    failure_reasons: Vec<String>,
    layout: LayoutReport,
    physical_region: RegionReport,
    native_mask: MaskReport,
    web_mask: MaskReport,
    mask_xor_ratio: f64,
    bbox_delta_px: Option<f64>,
    centroid_delta_px: Option<f64>,
    coverage_delta_ratio: f64,
}

#[derive(Clone, Debug, Serialize)]
struct LayoutReport {
    matched: bool,
    max_delta_milli: i64,
    native_bounds: FrameBoundsReport,
    web_bounds: FrameBoundsReport,
    native_font_size_milli: i64,
    web_font_size_milli: i64,
    native_line_height_milli: i64,
    web_line_height_milli: i64,
    native_rgba: [u8; 4],
    web_rgba: [u8; 4],
}

#[derive(Clone, Copy, Debug, Serialize)]
struct FrameBoundsReport {
    x_milli: i64,
    y_milli: i64,
    width_milli: i64,
    height_milli: i64,
}

impl From<FrameBounds> for FrameBoundsReport {
    fn from(bounds: FrameBounds) -> Self {
        Self {
            x_milli: bounds.x_milli,
            y_milli: bounds.y_milli,
            width_milli: bounds.width_milli,
            height_milli: bounds.height_milli,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
struct RegionReport {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    width: u32,
    height: u32,
}

impl From<PixelRect> for RegionReport {
    fn from(rect: PixelRect) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
            width: rect.width(),
            height: rect.height(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct MaskReport {
    ink_pixels: usize,
    area_pixels: usize,
    coverage_ratio: f64,
    bbox: Option<MaskBBox>,
    centroid: Option<Centroid>,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct MaskBBox {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct Centroid {
    x: f64,
    y: f64,
}

#[derive(Clone, Debug)]
struct TextMask {
    pixels: Vec<bool>,
    report: MaskReport,
}

fn compare_text_raster(
    native: &RgbaImage,
    web: &RgbaImage,
    native_frame: &FrameObservation,
    web_frame: &FrameObservation,
    args: &Args,
    font: Option<FontFingerprint>,
) -> TextRasterReport {
    let mut failure_reasons = Vec::new();
    if (native.width, native.height) != (web.width, web.height) {
        failure_reasons.push(format!(
            "png dimensions differ: native={}x{}, web={}x{}",
            native.width, native.height, web.width, web.height
        ));
    }
    if native_frame.viewport.physical_width != native.width
        || native_frame.viewport.physical_height != native.height
    {
        failure_reasons.push("native frame viewport physical size does not match native PNG".to_owned());
    }
    if web_frame.viewport.physical_width != web.width || web_frame.viewport.physical_height != web.height {
        failure_reasons.push("web frame viewport physical size does not match web PNG".to_owned());
    }
    if native_frame.text_count_for_report() != web_frame.text_count_for_report() {
        failure_reasons.push(format!(
            "text counts differ: native={}, web={}",
            native_frame.text_count_for_report(),
            web_frame.text_count_for_report()
        ));
    }
    if native_frame.viewport.scale_factor_milli != web_frame.viewport.scale_factor_milli
        || native_frame.viewport.logical_width_milli != web_frame.viewport.logical_width_milli
        || native_frame.viewport.logical_height_milli != web_frame.viewport.logical_height_milli
    {
        failure_reasons.push("typed viewport evidence differs between native and web".to_owned());
    }

    let native_runs = native_frame.text_runs_for_report();
    let web_runs = web_frame.text_runs_for_report();
    let run_count = native_runs.len().max(web_runs.len());
    let mut runs = Vec::with_capacity(run_count);
    for index in 0..run_count {
        match (native_runs.get(index), web_runs.get(index)) {
            (Some(native_text), Some(web_text)) => runs.push(compare_text_run(
                index,
                native,
                web,
                native_frame,
                web_frame,
                native_text,
                web_text,
                args,
            )),
            (Some(native_text), None) => runs.push(missing_run_report(
                index,
                native_text,
                "missing web text run".to_owned(),
            )),
            (None, Some(web_text)) => runs.push(missing_run_report(
                index,
                web_text,
                "missing native text run".to_owned(),
            )),
            (None, None) => {}
        }
    }

    failure_reasons.extend(
        runs.iter()
            .filter(|run| !run.passed)
            .map(|run| format!("text run {} failed: {}", run.index, run.failure_reasons.join(", "))),
    );
    let aggregate = aggregate_report(&runs);
    let passed = failure_reasons.is_empty();
    TextRasterReport {
        schema_version: "arcweft.text_raster_parity.v1",
        checkpoint: args.checkpoint.clone(),
        contract: "text-mask/layout identity with backend-specific antialias allowance",
        passed,
        failure_reasons,
        native: capture_summary(&args.native, &args.native_frame, native, native_frame),
        web: capture_summary(&args.web, &args.web_frame, web, web_frame),
        font,
        ink_affinity_threshold: args.ink_affinity_threshold,
        thresholds: args.thresholds,
        aggregate,
        runs,
    }
}

fn capture_summary(
    png_path: &Path,
    frame_path: &Path,
    image: &RgbaImage,
    frame: &FrameObservation,
) -> CaptureSummary {
    CaptureSummary {
        png_path: png_path.display().to_string(),
        frame_path: frame_path.display().to_string(),
        frame_schema_version: frame.schema_version.clone(),
        png_width: image.width,
        png_height: image.height,
        logical_width_milli: frame.viewport.logical_width_milli,
        logical_height_milli: frame.viewport.logical_height_milli,
        physical_width: frame.viewport.physical_width,
        physical_height: frame.viewport.physical_height,
        scale_factor_milli: frame.viewport.scale_factor_milli,
        text_count: frame.text_count_for_report(),
    }
}

fn compare_text_run(
    index: usize,
    native_image: &RgbaImage,
    web_image: &RgbaImage,
    native_frame: &FrameObservation,
    web_frame: &FrameObservation,
    native_text: &FrameText,
    web_text: &FrameText,
    args: &Args,
) -> TextRunReport {
    let layout = compare_layout(native_text, web_text, args.thresholds.layout_milli_tolerance);
    let native_rect = physical_text_rect(
        native_text.bounds,
        native_frame.viewport.scale_factor_milli,
        native_image.width,
        native_image.height,
        1,
    );
    let web_rect = physical_text_rect(
        web_text.bounds,
        web_frame.viewport.scale_factor_milli,
        web_image.width,
        web_image.height,
        1,
    );
    let region = native_rect.union(web_rect);
    let native_mask = text_mask(
        native_image,
        region,
        native_text.rgba,
        args.ink_affinity_threshold,
    );
    let web_mask = text_mask(web_image, region, web_text.rgba, args.ink_affinity_threshold);
    let comparison = compare_masks(&native_mask, &web_mask);

    let requires_ink = !native_text.text.trim().is_empty() || !web_text.text.trim().is_empty();
    let mut failure_reasons = Vec::new();
    if !layout.matched {
        failure_reasons.push(format!(
            "typed layout/style differs, max_delta_milli={}",
            layout.max_delta_milli
        ));
    }
    if requires_ink && native_mask.report.ink_pixels < args.thresholds.min_ink_pixels {
        failure_reasons.push(format!(
            "native ink pixels {} below minimum {}",
            native_mask.report.ink_pixels, args.thresholds.min_ink_pixels
        ));
    }
    if requires_ink && web_mask.report.ink_pixels < args.thresholds.min_ink_pixels {
        failure_reasons.push(format!(
            "web ink pixels {} below minimum {}",
            web_mask.report.ink_pixels, args.thresholds.min_ink_pixels
        ));
    }
    if comparison.mask_xor_ratio > args.thresholds.max_mask_xor_ratio {
        failure_reasons.push(format!(
            "mask_xor_ratio {:.6} exceeds {:.6}",
            comparison.mask_xor_ratio, args.thresholds.max_mask_xor_ratio
        ));
    }
    match comparison.bbox_delta_px {
        Some(delta) if delta > args.thresholds.max_bbox_delta_px => failure_reasons.push(format!(
            "bbox_delta_px {:.3} exceeds {:.3}",
            delta, args.thresholds.max_bbox_delta_px
        )),
        None if requires_ink => failure_reasons.push("cannot compare text mask bounding boxes".to_owned()),
        _ => {}
    }
    match comparison.centroid_delta_px {
        Some(delta) if delta > args.thresholds.max_centroid_delta_px => failure_reasons.push(format!(
            "centroid_delta_px {:.3} exceeds {:.3}",
            delta, args.thresholds.max_centroid_delta_px
        )),
        None if requires_ink => failure_reasons.push("cannot compare text mask centroids".to_owned()),
        _ => {}
    }
    if comparison.coverage_delta_ratio > args.thresholds.max_coverage_delta_ratio {
        failure_reasons.push(format!(
            "coverage_delta_ratio {:.6} exceeds {:.6}",
            comparison.coverage_delta_ratio, args.thresholds.max_coverage_delta_ratio
        ));
    }

    TextRunReport {
        index,
        text: native_text.text.clone(),
        passed: failure_reasons.is_empty(),
        failure_reasons,
        layout,
        physical_region: region.into(),
        native_mask: native_mask.report,
        web_mask: web_mask.report,
        mask_xor_ratio: comparison.mask_xor_ratio,
        bbox_delta_px: comparison.bbox_delta_px,
        centroid_delta_px: comparison.centroid_delta_px,
        coverage_delta_ratio: comparison.coverage_delta_ratio,
    }
}

fn compare_layout(
    native_text: &FrameText,
    web_text: &FrameText,
    tolerance_milli: i64,
) -> LayoutReport {
    let deltas = [
        (native_text.bounds.x_milli - web_text.bounds.x_milli).abs(),
        (native_text.bounds.y_milli - web_text.bounds.y_milli).abs(),
        (native_text.bounds.width_milli - web_text.bounds.width_milli).abs(),
        (native_text.bounds.height_milli - web_text.bounds.height_milli).abs(),
        (native_text.font_size_milli - web_text.font_size_milli).abs(),
        (native_text.line_height_milli - web_text.line_height_milli).abs(),
    ];
    let max_delta_milli = deltas.into_iter().max().unwrap_or(0);
    let matched = native_text.text == web_text.text
        && native_text.rgba == web_text.rgba
        && max_delta_milli <= tolerance_milli;
    LayoutReport {
        matched,
        max_delta_milli,
        native_bounds: native_text.bounds.into(),
        web_bounds: web_text.bounds.into(),
        native_font_size_milli: native_text.font_size_milli,
        web_font_size_milli: web_text.font_size_milli,
        native_line_height_milli: native_text.line_height_milli,
        web_line_height_milli: web_text.line_height_milli,
        native_rgba: native_text.rgba,
        web_rgba: web_text.rgba,
    }
}

fn missing_run_report(index: usize, text: &FrameText, reason: String) -> TextRunReport {
    let layout = LayoutReport {
        matched: false,
        max_delta_milli: i64::MAX,
        native_bounds: text.bounds.into(),
        web_bounds: text.bounds.into(),
        native_font_size_milli: text.font_size_milli,
        web_font_size_milli: text.font_size_milli,
        native_line_height_milli: text.line_height_milli,
        web_line_height_milli: text.line_height_milli,
        native_rgba: text.rgba,
        web_rgba: text.rgba,
    };
    TextRunReport {
        index,
        text: text.text.clone(),
        passed: false,
        failure_reasons: vec![reason],
        layout,
        physical_region: PixelRect::default().into(),
        native_mask: empty_mask_report(),
        web_mask: empty_mask_report(),
        mask_xor_ratio: 1.0,
        bbox_delta_px: None,
        centroid_delta_px: None,
        coverage_delta_ratio: 1.0,
    }
}

fn aggregate_report(runs: &[TextRunReport]) -> AggregateReport {
    runs.iter().fold(AggregateReport::default(), |mut aggregate, run| {
        aggregate.max_mask_xor_ratio = aggregate.max_mask_xor_ratio.max(run.mask_xor_ratio);
        aggregate.max_bbox_delta_px = aggregate.max_bbox_delta_px.max(run.bbox_delta_px.unwrap_or(0.0));
        aggregate.max_centroid_delta_px = aggregate
            .max_centroid_delta_px
            .max(run.centroid_delta_px.unwrap_or(0.0));
        aggregate.max_coverage_delta_ratio = aggregate
            .max_coverage_delta_ratio
            .max(run.coverage_delta_ratio);
        if !run.layout.matched {
            aggregate.layout_mismatch_count += 1;
        }
        if !run.passed {
            aggregate.failed_run_count += 1;
        }
        aggregate
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PixelRect {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl PixelRect {
    fn width(self) -> u32 {
        self.right.saturating_sub(self.left)
    }

    fn height(self) -> u32 {
        self.bottom.saturating_sub(self.top)
    }

    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }
}

fn physical_text_rect(
    bounds: FrameBounds,
    scale_factor_milli: i64,
    image_width: u32,
    image_height: u32,
    padding_px: i32,
) -> PixelRect {
    let scale = (scale_factor_milli as f64 / 1_000.0).max(f64::EPSILON);
    let x = bounds.x_milli as f64 / 1_000.0 * scale;
    let y = bounds.y_milli as f64 / 1_000.0 * scale;
    let width = bounds.width_milli.max(0) as f64 / 1_000.0 * scale;
    let height = bounds.height_milli.max(0) as f64 / 1_000.0 * scale;
    let left = clamp_i32(x.floor() as i32 - padding_px, 0, image_width as i32);
    let top = clamp_i32(y.floor() as i32 - padding_px, 0, image_height as i32);
    let right = clamp_i32((x + width).ceil() as i32 + padding_px, 0, image_width as i32);
    let bottom = clamp_i32((y + height).ceil() as i32 + padding_px, 0, image_height as i32);
    PixelRect {
        left: u32::try_from(left).unwrap_or(0),
        top: u32::try_from(top).unwrap_or(0),
        right: u32::try_from(right.max(left)).unwrap_or(0),
        bottom: u32::try_from(bottom.max(top)).unwrap_or(0),
    }
}

fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    value.clamp(min, max)
}

fn text_mask(
    image: &RgbaImage,
    region: PixelRect,
    text_rgba: [u8; 4],
    ink_affinity_threshold: f64,
) -> TextMask {
    let width = usize::try_from(region.width()).unwrap_or(0);
    let height = usize::try_from(region.height()).unwrap_or(0);
    let area_pixels = width.saturating_mul(height);
    let mut pixels = Vec::with_capacity(area_pixels);
    let mut ink_pixels = 0usize;
    let mut left = u32::MAX;
    let mut top = u32::MAX;
    let mut right = 0u32;
    let mut bottom = 0u32;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;

    for y in region.top..region.bottom {
        for x in region.left..region.right {
            let ink = (x < image.width && y < image.height)
                .then(|| pixel_index(image.width, x, y))
                .flatten()
                .and_then(|index| image.rgba.get(index..index + 4))
                .is_some_and(|pixel| {
                    pixel[3] > 0 && ink_affinity(pixel, text_rgba) >= ink_affinity_threshold
                });
            pixels.push(ink);
            if ink {
                ink_pixels += 1;
                left = left.min(x);
                top = top.min(y);
                right = right.max(x.saturating_add(1));
                bottom = bottom.max(y.saturating_add(1));
                sum_x += f64::from(x) + 0.5;
                sum_y += f64::from(y) + 0.5;
            }
        }
    }

    let (bbox, centroid) = if ink_pixels == 0 {
        (None, None)
    } else {
        (
            Some(MaskBBox {
                left,
                top,
                right,
                bottom,
            }),
            Some(Centroid {
                x: sum_x / ink_pixels as f64,
                y: sum_y / ink_pixels as f64,
            }),
        )
    };
    TextMask {
        pixels,
        report: MaskReport {
            ink_pixels,
            area_pixels,
            coverage_ratio: ratio(ink_pixels, area_pixels),
            bbox,
            centroid,
        },
    }
}

fn pixel_index(width: u32, x: u32, y: u32) -> Option<usize> {
    let width = usize::try_from(width).ok()?;
    let x = usize::try_from(x).ok()?;
    let y = usize::try_from(y).ok()?;
    y.checked_mul(width)?.checked_add(x)?.checked_mul(4)
}

fn ink_affinity(pixel: &[u8], text_rgba: [u8; 4]) -> f64 {
    let channels = [
        (pixel[0], text_rgba[0]),
        (pixel[1], text_rgba[1]),
        (pixel[2], text_rgba[2]),
    ];
    let distance = channels
        .into_iter()
        .map(|(actual, target)| {
            let delta = (f64::from(actual) - f64::from(target)) / 255.0;
            delta * delta
        })
        .sum::<f64>()
        .sqrt();
    (1.0 - distance / 3.0_f64.sqrt()).clamp(0.0, 1.0)
}

struct MaskComparison {
    mask_xor_ratio: f64,
    bbox_delta_px: Option<f64>,
    centroid_delta_px: Option<f64>,
    coverage_delta_ratio: f64,
}

fn compare_masks(native: &TextMask, web: &TextMask) -> MaskComparison {
    let (xor_pixels, union_pixels) = native.pixels.iter().zip(&web.pixels).fold(
        (0usize, 0usize),
        |(xor_pixels, union_pixels), (native, web)| {
            (
                xor_pixels + usize::from(*native ^ *web),
                union_pixels + usize::from(*native || *web),
            )
        },
    );
    let bbox_delta_px = match (native.report.bbox, web.report.bbox) {
        (Some(native), Some(web)) => Some(max_bbox_delta(native, web)),
        (None, None) => Some(0.0),
        _ => None,
    };
    let centroid_delta_px = match (native.report.centroid, web.report.centroid) {
        (Some(native), Some(web)) => Some(
            ((native.x - web.x) * (native.x - web.x) + (native.y - web.y) * (native.y - web.y))
                .sqrt(),
        ),
        (None, None) => Some(0.0),
        _ => None,
    };
    MaskComparison {
        mask_xor_ratio: ratio(xor_pixels, union_pixels.max(1)),
        bbox_delta_px,
        centroid_delta_px,
        coverage_delta_ratio: coverage_delta_ratio(native.report.ink_pixels, web.report.ink_pixels),
    }
}

fn max_bbox_delta(native: MaskBBox, web: MaskBBox) -> f64 {
    [
        native.left.abs_diff(web.left),
        native.top.abs_diff(web.top),
        native.right.abs_diff(web.right),
        native.bottom.abs_diff(web.bottom),
    ]
    .into_iter()
    .max()
    .map_or(0.0, f64::from)
}

fn coverage_delta_ratio(native_pixels: usize, web_pixels: usize) -> f64 {
    let max_pixels = native_pixels.max(web_pixels);
    ratio(native_pixels.abs_diff(web_pixels), max_pixels.max(1))
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    numerator as f64 / denominator.max(1) as f64
}

fn empty_mask_report() -> MaskReport {
    MaskReport {
        ink_pixels: 0,
        area_pixels: 0,
        coverage_ratio: 0.0,
        bbox: None,
        centroid: None,
    }
}

fn fnv1a64_hex(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{hash:016x}")
}

fn run_self_test() -> Result<(), Box<dyn Error>> {
    let mut native = RgbaImage::blank(16, 8, [0, 0, 0, 255]);
    let mut web = RgbaImage::blank(16, 8, [0, 0, 0, 255]);
    let ink = PixelRect {
        left: 3,
        top: 2,
        right: 9,
        bottom: 6,
    };
    native.fill_rect(ink, [255, 255, 255, 255]);
    web.fill_rect(ink, [250, 250, 250, 255]);
    let frame = FrameObservation {
        schema_version: "arcweft.text_raster_parity.self_test".to_owned(),
        viewport: FrameViewport {
            logical_width_milli: 16_000,
            logical_height_milli: 8_000,
            physical_width: 16,
            physical_height: 8,
            scale_factor_milli: 1_000,
        },
        text_count: 1,
        styled_paragraph_count: 0,
        text: vec![FrameText {
            text: "self-test".to_owned(),
            bounds: FrameBounds {
                x_milli: 2_000,
                y_milli: 1_000,
                width_milli: 8_000,
                height_milli: 6_000,
            },
            font_size_milli: 12_000,
            line_height_milli: 16_000,
            rgba: [255, 255, 255, 255],
        }],
        styled_paragraphs: Vec::new(),
    };
    let args = Args {
        checkpoint: "self-test".to_owned(),
        native: PathBuf::from("native.png"),
        web: PathBuf::from("web.png"),
        native_frame: PathBuf::from("native.frame.json"),
        web_frame: PathBuf::from("web.frame.json"),
        report: PathBuf::from("text-raster.json"),
        font: None,
        ink_affinity_threshold: 0.35,
        thresholds: TextRasterThresholds::default(),
    };
    let report = compare_text_raster(&native, &web, &frame, &frame, &args, None);
    if !report.passed {
        return Err(format!("self-test should pass: {:?}", report.failure_reasons).into());
    }
    Ok(())
}
