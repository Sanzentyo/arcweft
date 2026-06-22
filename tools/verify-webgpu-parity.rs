#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
png = "0.18.1"
---

use png::{BitDepth, ColorType, Decoder};
use std::env;
use std::error::Error;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args().skip(1).collect())?;
    let native = RgbaImage::read(&args.native)?;
    let web = RgbaImage::read(&args.web)?;
    let report = VisualParityReport::compare(&native, &web, args.changed_channel_threshold)?;
    let passed = args.thresholds.accepts(&report);
    if let Some(path) = &args.report {
        report.write_json(path, &args.thresholds, passed)?;
    }
    println!(
        "native/web parity: psnr={:.4} dB, ssim={:.4}, mse={:.6}, mae={:.6}, maxae={:.6}, changed_pixel_ratio={:.6}",
        report.psnr, report.ssim, report.mse, report.mae, report.maxae, report.changed_pixel_ratio
    );
    if passed {
        return Ok(());
    }
    Err(format!(
        "native/web parity failed approved thresholds: {}",
        args.thresholds.describe()
    )
    .into())
}

#[derive(Clone, Debug)]
struct Args {
    native: PathBuf,
    web: PathBuf,
    report: Option<PathBuf>,
    thresholds: VisualParityThresholds,
    changed_channel_threshold: u8,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut parsed = Self {
            native: PathBuf::from("target/webgpu-parity/native.png"),
            web: PathBuf::from("target/webgpu-parity/web.png"),
            report: Some(PathBuf::from("target/webgpu-parity/parity-report.json")),
            thresholds: VisualParityThresholds::default(),
            changed_channel_threshold: 3,
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--native" => {
                    index += 1;
                    parsed.native = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--native requires a path".to_owned())?,
                    );
                }
                "--web" => {
                    index += 1;
                    parsed.web = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--web requires a path".to_owned())?,
                    );
                }
                "--report" => {
                    index += 1;
                    parsed.report = Some(PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--report requires a path".to_owned())?,
                    ));
                }
                "--no-report" => parsed.report = None,
                "--min-psnr" => {
                    index += 1;
                    parsed.thresholds.min_psnr = parse_f64(&args, index, "--min-psnr")?;
                }
                "--min-ssim" => {
                    index += 1;
                    parsed.thresholds.min_ssim = parse_f64(&args, index, "--min-ssim")?;
                }
                "--max-mse" => {
                    index += 1;
                    parsed.thresholds.max_mse = parse_f64(&args, index, "--max-mse")?;
                }
                "--max-mae" => {
                    index += 1;
                    parsed.thresholds.max_mae = parse_f64(&args, index, "--max-mae")?;
                }
                "--max-maxae" => {
                    index += 1;
                    parsed.thresholds.max_maxae = parse_f64(&args, index, "--max-maxae")?;
                }
                "--max-changed-pixel-ratio" => {
                    index += 1;
                    parsed.thresholds.max_changed_pixel_ratio =
                        parse_f64(&args, index, "--max-changed-pixel-ratio")?;
                }
                "--changed-channel-threshold" => {
                    index += 1;
                    parsed.changed_channel_threshold = args
                        .get(index)
                        .ok_or_else(|| {
                            "--changed-channel-threshold requires an integer".to_owned()
                        })?
                        .parse()
                        .map_err(|error| {
                            format!("--changed-channel-threshold must be an integer: {error}")
                        })?;
                }
                "--help" | "-h" => return Err(Self::usage()),
                unknown => return Err(format!("unknown argument `{unknown}`\n{}", Self::usage())),
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn usage() -> String {
        "usage: cargo +nightly -Zscript tools/verify-webgpu-parity.rs \
         [--native target/webgpu-parity/native.png] [--web target/webgpu-parity/web.png] \
         [--report target/webgpu-parity/parity-report.json] \
         [--min-psnr 24.0] [--min-ssim 0.68] [--max-mse 0.004] \
         [--max-mae 0.006] [--max-maxae 0.90] [--max-changed-pixel-ratio 0.02]"
            .to_owned()
    }
}

fn parse_f64(args: &[String], index: usize, name: &str) -> Result<f64, String> {
    args.get(index)
        .ok_or_else(|| format!("{name} requires a number"))?
        .parse()
        .map_err(|error| format!("{name} must be a number: {error}"))
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
}

#[derive(Clone, Debug)]
struct VisualParityThresholds {
    min_psnr: f64,
    min_ssim: f64,
    max_mse: f64,
    max_mae: f64,
    max_maxae: f64,
    max_changed_pixel_ratio: f64,
}

impl Default for VisualParityThresholds {
    fn default() -> Self {
        Self {
            min_psnr: 24.0,
            min_ssim: 0.68,
            max_mse: 0.004,
            max_mae: 0.006,
            max_maxae: 0.90,
            max_changed_pixel_ratio: 0.02,
        }
    }
}

impl VisualParityThresholds {
    fn accepts(&self, report: &VisualParityReport) -> bool {
        report.psnr >= self.min_psnr
            && report.ssim >= self.min_ssim
            && report.mse <= self.max_mse
            && report.mae <= self.max_mae
            && report.maxae <= self.max_maxae
            && report.changed_pixel_ratio <= self.max_changed_pixel_ratio
    }

    fn describe(&self) -> String {
        format!(
            "psnr>={:.4}, ssim>={:.4}, mse<={:.6}, mae<={:.6}, maxae<={:.6}, changed_pixel_ratio<={:.6}",
            self.min_psnr,
            self.min_ssim,
            self.max_mse,
            self.max_mae,
            self.max_maxae,
            self.max_changed_pixel_ratio
        )
    }
}

#[derive(Clone, Debug)]
struct VisualParityReport {
    width: u32,
    height: u32,
    psnr: f64,
    ssim: f64,
    mse: f64,
    mae: f64,
    maxae: f64,
    changed_pixel_ratio: f64,
}

impl VisualParityReport {
    fn compare(
        native: &RgbaImage,
        web: &RgbaImage,
        changed_channel_threshold: u8,
    ) -> Result<Self, Box<dyn Error>> {
        if native.width != web.width || native.height != web.height {
            return Err(format!(
                "capture dimensions differ: native={}x{}, web={}x{}",
                native.width, native.height, web.width, web.height
            )
            .into());
        }
        if native.rgba.len() != web.rgba.len() {
            return Err("capture byte lengths differ".into());
        }
        let pixel_count = native.rgba.len() / 4;
        let mut native_luma = Vec::with_capacity(pixel_count);
        let mut web_luma = Vec::with_capacity(pixel_count);
        let mut sum_square_error = 0.0;
        let mut sum_absolute_error = 0.0;
        let mut max_absolute_error = 0.0;
        let mut changed_pixels = 0usize;

        for (left, right) in native.rgba.chunks_exact(4).zip(web.rgba.chunks_exact(4)) {
            let left_luma = luma(left);
            let right_luma = luma(right);
            let absolute_error = (left_luma - right_luma).abs();
            native_luma.push(left_luma);
            web_luma.push(right_luma);
            sum_square_error += absolute_error * absolute_error;
            sum_absolute_error += absolute_error;
            if absolute_error > max_absolute_error {
                max_absolute_error = absolute_error;
            }
            if left
                .iter()
                .zip(right.iter())
                .take(3)
                .any(|(left, right)| left.abs_diff(*right) > changed_channel_threshold)
            {
                changed_pixels += 1;
            }
        }

        let samples = pixel_count as f64;
        let mse = sum_square_error / samples.max(1.0);
        let psnr = if mse == 0.0 {
            f64::INFINITY
        } else {
            10.0 * (1.0 / mse).log10()
        };
        Ok(Self {
            width: native.width,
            height: native.height,
            psnr,
            ssim: global_ssim(&native_luma, &web_luma),
            mse,
            mae: sum_absolute_error / samples.max(1.0),
            maxae: max_absolute_error,
            changed_pixel_ratio: changed_pixels as f64 / samples.max(1.0),
        })
    }

    fn write_json(
        &self,
        path: &Path,
        thresholds: &VisualParityThresholds,
        passed: bool,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            path,
            format!(
                concat!(
                    "{{\n",
                    "  \"passed\": {},\n",
                    "  \"dimensions\": {{ \"width\": {}, \"height\": {} }},\n",
                    "  \"metrics\": {{\n",
                    "    \"psnr\": {:.12},\n",
                    "    \"ssim\": {:.12},\n",
                    "    \"mse\": {:.12},\n",
                    "    \"mae\": {:.12},\n",
                    "    \"maxae\": {:.12},\n",
                    "    \"changed_pixel_ratio\": {:.12}\n",
                    "  }},\n",
                    "  \"thresholds\": {{\n",
                    "    \"min_psnr\": {:.12},\n",
                    "    \"min_ssim\": {:.12},\n",
                    "    \"max_mse\": {:.12},\n",
                    "    \"max_mae\": {:.12},\n",
                    "    \"max_maxae\": {:.12},\n",
                    "    \"max_changed_pixel_ratio\": {:.12}\n",
                    "  }}\n",
                    "}}\n"
                ),
                passed,
                self.width,
                self.height,
                self.psnr,
                self.ssim,
                self.mse,
                self.mae,
                self.maxae,
                self.changed_pixel_ratio,
                thresholds.min_psnr,
                thresholds.min_ssim,
                thresholds.max_mse,
                thresholds.max_mae,
                thresholds.max_maxae,
                thresholds.max_changed_pixel_ratio
            ),
        )?;
        Ok(())
    }
}

fn luma(pixel: &[u8]) -> f64 {
    let red = f64::from(pixel[0]) / 255.0;
    let green = f64::from(pixel[1]) / 255.0;
    let blue = f64::from(pixel[2]) / 255.0;
    red.mul_add(0.2126, green.mul_add(0.7152, blue * 0.0722))
}

fn global_ssim(reference: &[f64], distorted: &[f64]) -> f64 {
    let samples = reference.len().max(1) as f64;
    let mean_reference = reference.iter().sum::<f64>() / samples;
    let mean_distorted = distorted.iter().sum::<f64>() / samples;
    let variance_reference = reference
        .iter()
        .map(|value| {
            let delta = value - mean_reference;
            delta * delta
        })
        .sum::<f64>()
        / samples;
    let variance_distorted = distorted
        .iter()
        .map(|value| {
            let delta = value - mean_distorted;
            delta * delta
        })
        .sum::<f64>()
        / samples;
    let covariance = reference
        .iter()
        .zip(distorted.iter())
        .map(|(left, right)| (left - mean_reference) * (right - mean_distorted))
        .sum::<f64>()
        / samples;
    let c1 = 0.01_f64.powi(2);
    let c2 = 0.03_f64.powi(2);
    ((2.0 * mean_reference * mean_distorted + c1) * (2.0 * covariance + c2))
        / ((mean_reference.powi(2) + mean_distorted.powi(2) + c1)
            * (variance_reference + variance_distorted + c2))
}
