#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "prepare-zundamon-sample"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false

[dependencies]
arcweft-character = { path = "../crates/arcweft-character" }
arcweft-character-psd = { path = "../crates/arcweft-character-psd" }
image = { version = "0.25.10", default-features = false, features = ["png"] }
---

use arcweft_character::id::CharacterId;
use arcweft_character::manifest::{
    CharacterManifest, CharacterPart, CharacterVariant, ResolvedCharacterLayer,
};
use arcweft_character_psd::{ImportedCharacterFile, PsdCharacterImportOptions, import_psd_character};
use image::{GenericImageView, ImageEncoder, RgbaImage, codecs::png::PngEncoder};
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_SOURCE: &str =
    ".arcweft-local/character-psd/zundamon-v3.2/ずんだもん立ち絵素材V3.2_全部詰め版.psd";
const DEFAULT_OUTPUT: &str = "samples/zundamon-stand-switch/.arcweft/asset/zundamon";

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse(env::args().skip(1).collect())?;
    let bytes = fs::read(&args.source)?;
    let character = CharacterId::try_new("character.zundamon")?;
    let imported = import_psd_character(
        &bytes,
        &PsdCharacterImportOptions::new(
            character,
            args.source
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("zundamon.psd"),
        ),
    )?;
    let (manifest, files, warnings) = imported.into_parts();
    let files = files_by_path(&files);
    let resolved_layers = manifest.resolve_look(manifest.default_look())?;
    if args.list_variants {
        list_variants(&manifest);
        return Ok(());
    }
    let base_layers = front_facing_layers(&manifest).unwrap_or(resolved_layers);
    let normal = compose_layers(&manifest, &files, &base_layers)?;
    let alternate_layers = alternate_layers(&base_layers);
    let smile = compose_layers(&manifest, &files, &alternate_layers)?;

    println!(
        "source={} canvas={}x{} default_look={} layers={} output={}",
        args.source.display(),
        manifest.canvas().width(),
        manifest.canvas().height(),
        manifest.default_look().as_str(),
        base_layers.len(),
        args.output.display()
    );
    for warning in warnings {
        println!("warning: {warning}");
    }
    if layer_keys(&alternate_layers) == layer_keys(&base_layers) {
        println!("warning: no alternate variants matched; smile.png will match normal.png");
    }
    if args.apply {
        fs::create_dir_all(&args.output)?;
        write_png(&args.output.join("normal.png"), &normal)?;
        write_png(&args.output.join("smile.png"), &smile)?;
        println!("wrote {}", args.output.join("normal.png").display());
        println!("wrote {}", args.output.join("smile.png").display());
    } else {
        println!("dry-run: pass --apply to write normal.png and smile.png");
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Args {
    source: PathBuf,
    output: PathBuf,
    apply: bool,
    list_variants: bool,
}

impl Args {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut result = Self {
            source: env_path("ARW_ZUNDAMON_PSD").unwrap_or_else(|| PathBuf::from(DEFAULT_SOURCE)),
            output: PathBuf::from(DEFAULT_OUTPUT),
            apply: false,
            list_variants: false,
        };
        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--source" => {
                    result.source = PathBuf::from(
                        iter.next()
                            .ok_or_else(|| "--source requires a path".to_owned())?,
                    );
                }
                "--output" => {
                    result.output = PathBuf::from(
                        iter.next()
                            .ok_or_else(|| "--output requires a path".to_owned())?,
                    );
                }
                "--apply" | "-a" => result.apply = true,
                "--list-variants" => result.list_variants = true,
                "--help" | "-h" => return Err(help()),
                other => return Err(format!("unknown argument `{other}`\n\n{}", help())),
            }
        }
        Ok(result)
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from)
}

fn help() -> String {
    format!(
        "usage: cargo +nightly -Zscript tools/prepare-zundamon-sample.rs [--apply] [--list-variants] [--source PATH] [--output PATH]\n\
         default source: {DEFAULT_SOURCE}\n\
         default output: {DEFAULT_OUTPUT}"
    )
}

fn list_variants(manifest: &CharacterManifest) {
    for part in manifest.parts() {
        println!("part={} z={}", part.id().as_str(), part.z());
        for variant in part.variants() {
            let source = variant.source_layer().map_or_else(
                || "<none>".to_owned(),
                |source| format!("{} / {}", source.group(), source.layer()),
            );
            println!("  {} {}", variant.id().as_str(), source);
        }
    }
}

fn files_by_path(files: &[ImportedCharacterFile]) -> BTreeMap<String, &[u8]> {
    files
        .iter()
        .map(|file| (file.path().as_str().to_owned(), file.bytes()))
        .collect()
}

fn alternate_layers<'a>(
    base_layers: &[ResolvedCharacterLayer<'a>],
) -> Vec<ResolvedCharacterLayer<'a>> {
    base_layers
        .iter()
        .copied()
        .map(|layer| {
            preferred_variant(layer.part(), layer.variant())
                .map(|variant| ResolvedCharacterLayer::new(layer.part(), variant))
                .unwrap_or(layer)
        })
        .collect()
}

fn front_facing_layers(manifest: &CharacterManifest) -> Option<Vec<ResolvedCharacterLayer<'_>>> {
    let mut layers = manifest
        .parts()
        .iter()
        .filter(|part| !skip_front_facing_part(part))
        .filter_map(|part| base_variant(part).map(|variant| ResolvedCharacterLayer::new(part, variant)))
        .collect::<Vec<_>>();
    layers.sort_by_key(|layer| {
        (
            front_layer_order(part_group(layer.part())),
            layer.part().z(),
            layer.part().id().clone(),
        )
    });
    (!layers.is_empty()).then_some(layers)
}

fn front_layer_order(group: &str) -> i32 {
    if group == "<top-level>" {
        0
    } else if contains_any(group, &["左腕", "右腕"]) {
        1
    } else if group.contains("頭_正面向き") {
        2
    } else if group.contains("枝豆") {
        3
    } else if group.contains("顔色") {
        4
    } else if group.contains("眉") {
        5
    } else if group.contains("目") {
        6
    } else if group.contains("口") {
        7
    } else {
        8
    }
}

fn skip_front_facing_part(part: &CharacterPart) -> bool {
    let group = part_group(part);
    group.contains("上向き")
        || (part.z() > 5
            && contains_any(group, &["!眉", "!目", "!口", "!顔色", "!枝豆"]))
}

fn base_variant(part: &CharacterPart) -> Option<&CharacterVariant> {
    let group = part_group(part);
    let keywords: &[&str] = if group.contains("頭_正面向き") {
        &["!頭"]
    } else if group.contains("眉") {
        &["*基本眉"]
    } else if group.contains("目") {
        &["*基本目"]
    } else if group.contains("口") {
        &["*むふ", "*ほほえみ"]
    } else if group.contains("顔色") {
        &["*ほっぺ基本"]
    } else if group.contains("枝豆") {
        &["*枝豆通常"]
    } else if contains_any(group, &["左腕", "右腕"]) {
        &["*基本"]
    } else if group == "<top-level>" {
        &["!体"]
    } else {
        &[]
    };
    part.variants()
        .iter()
        .find(|variant| {
            let text = source_key(variant);
            keywords.iter().any(|keyword| text.contains(keyword))
        })
        .or_else(|| part.variants().first())
}

fn part_group(part: &CharacterPart) -> &str {
    part.variants()
        .iter()
        .find_map(|variant| variant.source_layer().map(|source| source.group()))
        .unwrap_or("")
}

fn preferred_variant<'a>(
    part: &'a CharacterPart,
    current: &'a CharacterVariant,
) -> Option<&'a CharacterVariant> {
    let current_key = source_key(current);
    part.variants()
        .iter()
        .filter(|variant| source_key(variant) != current_key)
        .find(|variant| variant_matches(variant))
}

fn source_key(variant: &CharacterVariant) -> String {
    variant.source_layer().map_or_else(
        || variant.id().as_str().to_owned(),
        |source| format!("{}:{}", source.group(), source.layer()),
    )
}

fn variant_matches(variant: &CharacterVariant) -> bool {
    let Some(source) = variant.source_layer() else {
        return false;
    };
    let text = format!("{} {}", source.group(), source.layer());
    let group = source.group();
    let eye = group.contains('目') && contains_any(&text, &["笑", "にっこり", "閉じ"]);
    let mouth = group.contains('口') && contains_any(&text, &["笑", "あ", "にっこり", "開"]);
    let arm = contains_any(group, &["腕", "手"]) && contains_any(&text, &["上", "あげ", "挙"]);
    eye || mouth || arm
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn layer_keys(layers: &[ResolvedCharacterLayer<'_>]) -> Vec<String> {
    layers
        .iter()
        .map(|layer| source_key(layer.variant()))
        .collect()
}

fn compose_layers(
    manifest: &CharacterManifest,
    files: &BTreeMap<String, &[u8]>,
    layers: &[ResolvedCharacterLayer<'_>],
) -> Result<RgbaImage, Box<dyn Error>> {
    let canvas = manifest.canvas();
    let mut image = RgbaImage::new(canvas.width(), canvas.height());
    for layer in layers {
        let variant = layer.variant();
        let bytes = files
            .get(variant.asset().as_str())
            .ok_or_else(|| format!("missing layer asset `{}`", variant.asset().as_str()))?;
        let layer_image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?;
        let layer_image = layer_image.to_rgba8();
        let rect = variant.rect();
        overlay_normal(&mut image, &layer_image, rect.x(), rect.y(), variant.opacity());
    }
    Ok(crop_to_alpha(image))
}

fn overlay_normal(base: &mut RgbaImage, layer: &RgbaImage, x: i32, y: i32, opacity: u8) {
    for (lx, ly, pixel) in layer.enumerate_pixels() {
        let Some(dx) = i32::try_from(lx).ok().and_then(|lx| x.checked_add(lx)) else {
            continue;
        };
        let Some(dy) = i32::try_from(ly).ok().and_then(|ly| y.checked_add(ly)) else {
            continue;
        };
        if dx < 0 || dy < 0 {
            continue;
        }
        let (dx, dy) = (dx as u32, dy as u32);
        if dx >= base.width() || dy >= base.height() {
            continue;
        }
        let mut src = pixel.0;
        src[3] = ((u16::from(src[3]) * u16::from(opacity)) / u16::from(u8::MAX)) as u8;
        if src[3] == 0 {
            continue;
        }
        let dst = base.get_pixel_mut(dx, dy);
        blend_pixel(&mut dst.0, src);
    }
}

fn blend_pixel(dst: &mut [u8; 4], src: [u8; 4]) {
    let src_a = f32::from(src[3]) / 255.0;
    let dst_a = f32::from(dst[3]) / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= f32::EPSILON {
        *dst = [0, 0, 0, 0];
        return;
    }
    for channel in 0..3 {
        let src_c = f32::from(src[channel]) / 255.0;
        let dst_c = f32::from(dst[channel]) / 255.0;
        let out_c = (src_c * src_a + dst_c * dst_a * (1.0 - src_a)) / out_a;
        dst[channel] = (out_c * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn crop_to_alpha(image: RgbaImage) -> RgbaImage {
    let Some((min_x, min_y, max_x, max_y)) = alpha_bounds(&image) else {
        return image;
    };
    image.view(min_x, min_y, max_x - min_x + 1, max_y - min_y + 1).to_image()
}

fn alpha_bounds(image: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel.0[3] == 0 {
            continue;
        }
        found = true;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    found.then_some((min_x, min_y, max_x, max_y))
}

fn write_png(path: &Path, image: &RgbaImage) -> Result<(), Box<dyn Error>> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    fs::write(path, bytes)?;
    Ok(())
}
