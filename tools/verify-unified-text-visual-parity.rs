#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
serde_json = "1.0.150"
---

//! Verifies the generated unified Text/View/Fx visual evidence packet.

use serde_json::{Value, json};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const CHECKPOINTS: [&str; 8] = [
    "vertical-rl",
    "vertical-lr",
    "jlreq-loose",
    "jlreq-strict",
    "fx-4000",
    "fx-4500",
    "reveal-20000",
    "reveal-20500",
];

fn main() -> Result<(), Box<dyn Error>> {
    let root = parse_root()?;
    let reports = CHECKPOINTS
        .iter()
        .map(|checkpoint| {
            Ok((
                *checkpoint,
                read_json(&root.join(format!("native-{checkpoint}.frame.json")))?,
                read_json(&root.join(format!("web-{checkpoint}.frame.json")))?,
            ))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

    for (checkpoint, native, web) in &reports {
        verify_backend_pair(&root, checkpoint, native, web)?;
    }

    let native = |checkpoint: &str| -> Result<&Value, Box<dyn Error>> {
        reports
            .iter()
            .find(|(candidate, _, _)| *candidate == checkpoint)
            .map(|(_, native, _)| native)
            .ok_or_else(|| failure(format!("missing native report for {checkpoint}")))
    };

    verify_vertical_rl(body(native("vertical-rl")?)?)?;
    verify_vertical_lr(body(native("vertical-lr")?)?)?;
    verify_jlreq(
        body(native("jlreq-loose")?)?,
        body(native("jlreq-strict")?)?,
    )?;
    verify_fx(
        body(native("fx-4000")?)?,
        body(native("fx-4500")?)?,
        native("fx-4000")?,
        native("fx-4500")?,
    )?;
    verify_reveal(
        body(native("reveal-20000")?)?,
        body(native("reveal-20500")?)?,
    )?;
    verify_scoped_packets(&root, &reports)?;

    let temporal_mse = ["jlreq", "fx", "reveal"]
        .into_iter()
        .map(|name| {
            let native = imq_mse(&root.join(format!("imq-native-{name}.json")))?;
            let web = imq_mse(&root.join(format!("imq-web-{name}.json")))?;
            require(
                native > 0.0 && web > 0.0,
                format!("{name} temporal/semantic images must differ on both backends"),
            )?;
            Ok((name, native, web))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

    let summary = json!({
        "schema_version": "arcweft.unified_text_visual_parity.v1",
        "passed": true,
        "checkpoints": CHECKPOINTS,
        "native_web_pixel_exact": true,
        "logical_clock_quantum_millis": 16,
        "temporal_mse": temporal_mse.iter().map(|(name, native, web)| json!({
            "name": name,
            "native": native,
            "web": web,
        })).collect::<Vec<_>>(),
        "scoped_packets": ["vertical-rl", "vertical-lr", "fx-4500"],
    });
    let summary_path = root.join("verification-summary.json");
    fs::write(
        &summary_path,
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;
    println!(
        "verified unified text visual parity packet: {}",
        summary_path.display()
    );
    Ok(())
}

fn parse_root() -> Result<PathBuf, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => root = Some(PathBuf::from(args.next().ok_or("--dir requires a path")?)),
            unknown => return Err(failure(format!("unknown argument `{unknown}`"))),
        }
    }
    root.ok_or_else(|| failure("expected --dir <evidence-directory>"))
}

fn verify_backend_pair(
    root: &Path,
    checkpoint: &str,
    native: &Value,
    web: &Value,
) -> Result<(), Box<dyn Error>> {
    require(
        native
            .pointer("/execution_path/layout")
            .and_then(Value::as_str)
            == Some("headless-player-scene"),
        format!("{checkpoint}: native report did not use headless-player-scene"),
    )?;
    require(
        native
            .pointer("/execution_path/raster")
            .and_then(Value::as_str)
            == Some("native-shared-wgpu-offscreen"),
        format!("{checkpoint}: native report did not use shared WGPU offscreen raster"),
    )?;
    require(
        web.pointer("/execution_path/layout")
            .and_then(Value::as_str)
            == Some("web-player-scene"),
        format!("{checkpoint}: Web report did not use web-player-scene"),
    )?;
    require(
        web.pointer("/execution_path/raster")
            .and_then(Value::as_str)
            == Some("web-shared-wgpu-canvas"),
        format!("{checkpoint}: Web report did not use shared WGPU canvas raster"),
    )?;
    require(
        native
            .get("fx_diagnostics")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
            && web
                .get("fx_diagnostics")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
        format!("{checkpoint}: Fx diagnostics are not empty"),
    )?;
    require(
        font_fingerprints(native)? == font_fingerprints(web)?,
        format!("{checkpoint}: Native/Web project-font fingerprints differ"),
    )?;
    require(
        native.get("logical_clock") == web.get("logical_clock"),
        format!("{checkpoint}: Native/Web logical clock evidence differs"),
    )?;
    for field in ["viewport", "text", "choices", "images", "focus"] {
        require(
            native.get(field) == web.get(field),
            format!("{checkpoint}: Native/Web prepared `{field}` evidence differs"),
        )?;
    }
    let native_body = body(native)?;
    require(
        native_body
            .get("runs")
            .and_then(Value::as_array)
            .is_some_and(|runs| {
                runs.iter().any(|run| {
                    run.pointer("/style/font_families")
                        .and_then(Value::as_array)
                        .is_some_and(|families| {
                            families.iter().any(|family| family == "Noto Sans JP")
                        })
                })
            }),
        format!("{checkpoint}: body did not resolve the project font"),
    )?;

    require_png(&root.join(format!("native-{checkpoint}.png")))?;
    require_png(&root.join(format!("web-{checkpoint}.png")))?;
    let parity = fs::read_to_string(root.join(format!("parity-{checkpoint}.json")))?;
    require(
        parity.contains("\"passed\": true"),
        format!("{checkpoint}: generic full-frame parity gate did not pass"),
    )?;
    require(
        imq_mse(&root.join(format!("imq-{checkpoint}.json")))? == 0.0,
        format!("{checkpoint}: Native/Web images are not pixel-exact"),
    )?;
    Ok(())
}

fn verify_vertical_rl(body: &Value) -> Result<(), Box<dyn Error>> {
    require(
        all_lines_are(body, "vertical_rl")?,
        "vertical-rl: wrong writing mode",
    )?;
    verify_text_combine(body, "2026")?;
    verify_sideways(body, "ARC")?;
    require(
        glyphs_for_literal(body, "。").iter().any(|glyph| {
            glyph.get("vertical_form").and_then(Value::as_str) == Some("upright_alternate")
        }),
        "vertical-rl: UAX punctuation alternate is absent",
    )?;
    require(
        glyphs_for_literal(body, "「").iter().any(|glyph| {
            glyph.get("vertical_form").and_then(Value::as_str) == Some("rotated_alternate")
        }),
        "vertical-rl: rotated opening punctuation alternate is absent",
    )?;
    Ok(())
}

fn verify_vertical_lr(body: &Value) -> Result<(), Box<dyn Error>> {
    require(
        all_lines_are(body, "vertical_lr")?,
        "vertical-lr: wrong writing mode",
    )?;
    verify_text_combine(body, "2026")?;
    verify_sideways(body, "XYZ")?;
    let ruby = array(body, "ruby")?;
    require(
        ruby.len() == 2,
        "vertical-lr: expected two ruby annotations",
    )?;
    let under_base = rect_center_x(&ruby[0]["base_bounds"])?;
    let under_ruby = rect_center_x(&ruby[0]["ruby_bounds"])?;
    require(
        under_ruby > under_base,
        "vertical-lr: ruby-under must lie on the physical right side of its base",
    )?;
    let inter_base = rect_center_x(&ruby[1]["base_bounds"])?;
    let inter_ruby = rect_center_x(&ruby[1]["ruby_bounds"])?;
    require(
        inter_ruby < inter_base,
        "vertical-lr: inter-character ruby must use the physical left over-track",
    )?;
    Ok(())
}

fn verify_jlreq(loose: &Value, strict: &Value) -> Result<(), Box<dyn Error>> {
    require(
        loose["text"] == strict["text"],
        "JLREQ fixtures do not use identical text",
    )?;
    require(
        loose["layout_hash"] != strict["layout_hash"],
        "JLREQ loose/strict layouts did not diverge",
    )?;
    let loose_period = glyph_line(loose, "。")?;
    let loose_open = glyph_line(loose, "「")?;
    let strict_period = glyph_line(strict, "。")?;
    let strict_open = glyph_line(strict, "「")?;
    require(
        loose_period != loose_open,
        "JLREQ loose mode did not split the closing/opening pair",
    )?;
    require(
        strict_period == strict_open,
        "JLREQ strict mode did not keep the closing/opening pair together",
    )?;
    Ok(())
}

fn verify_fx(
    earlier: &Value,
    later: &Value,
    earlier_report: &Value,
    later_report: &Value,
) -> Result<(), Box<dyn Error>> {
    require(
        earlier["layout_hash"] == later["layout_hash"],
        "Fx sampling changed the stable text layout hash",
    )?;
    let earlier_transforms = glyphs_for_literal(earlier, "波動光")
        .into_iter()
        .map(|glyph| glyph["transform"].clone())
        .collect::<Vec<_>>();
    let later_transforms = glyphs_for_literal(later, "波動光")
        .into_iter()
        .map(|glyph| glyph["transform"].clone())
        .collect::<Vec<_>>();
    require(
        earlier_transforms.len() == 3,
        "Fx fixture did not retain three target glyphs",
    )?;
    require(
        earlier_transforms != later_transforms,
        "Fx transforms did not advance with activation-relative logical time",
    )?;
    require(
        earlier_transforms
            .iter()
            .any(|transform| transform["translate_milli"] != json!([0, 0])),
        "Fx transforms remained identity",
    )?;
    verify_quantized_elapsed(earlier_report, 4_000)?;
    verify_quantized_elapsed(later_report, 4_500)?;
    Ok(())
}

fn verify_reveal(earlier: &Value, later: &Value) -> Result<(), Box<dyn Error>> {
    require(
        earlier["layout_hash"] == later["layout_hash"],
        "typewriter reveal changed the stable text layout hash",
    )?;
    let earlier_count = u64_field(earlier, "visible_glyph_count")?;
    let later_count = u64_field(later, "visible_glyph_count")?;
    require(
        later_count == earlier_count + 1,
        "500ms at 2 cps must reveal exactly one logical glyph",
    )?;
    let earlier_text = str_field(earlier, "visible_text")?;
    let later_text = str_field(later, "visible_text")?;
    require(
        later_text.starts_with(earlier_text) && later_text.len() > earlier_text.len(),
        "typewriter visible text did not progress by prefix",
    )?;
    Ok(())
}

fn verify_scoped_packets(
    root: &Path,
    reports: &[(&str, Value, Value)],
) -> Result<(), Box<dyn Error>> {
    for checkpoint in ["vertical-rl", "vertical-lr", "fx-4500"] {
        let report = reports
            .iter()
            .find(|(candidate, _, _)| *candidate == checkpoint)
            .map(|(_, native, _)| native)
            .ok_or_else(|| failure(format!("missing report for scope {checkpoint}")))?;
        let metadata = read_json(&root.join(format!("native-{checkpoint}.body.json")))?;
        require(
            metadata["schema_version"] == "arcweft.text_scope_capture.v1",
            format!("{checkpoint}: bad scoped capture schema"),
        )?;
        require(
            metadata["checkpoint"] == checkpoint,
            format!("{checkpoint}: scoped checkpoint identity differs"),
        )?;
        require(
            metadata["semantic_id"] == body(report)?["owner"]["semantic_id"],
            format!("{checkpoint}: scoped capture is not the dialogue View content"),
        )?;
        require(
            metadata["layout_hash"] == body(report)?["layout_hash"],
            format!("{checkpoint}: scoped capture layout hash differs"),
        )?;
        for suffix in ["color", "mask", "object-id"] {
            require_png(&root.join(format!("native-{checkpoint}.body.{suffix}.png")))?;
        }
    }
    Ok(())
}

fn verify_text_combine(body: &Value, literal: &str) -> Result<(), Box<dyn Error>> {
    let glyphs = glyphs_for_literal(body, literal);
    require(
        glyphs.len() == literal.chars().count(),
        format!("{literal}: text-combine glyph count differs"),
    )?;
    let cluster = glyphs[0]["cluster_index"].clone();
    let line = glyphs[0]["line_index"].clone();
    require(
        glyphs.iter().all(|glyph| {
            glyph["orientation"] == "text_combine_upright"
                && glyph["cluster_index"] == cluster
                && glyph["line_index"] == line
                && glyph["inline_scale_milli"]
                    .as_u64()
                    .is_some_and(|scale| scale < 1_000)
        }),
        format!("{literal}: text-combine cluster contract differs"),
    )
}

fn verify_sideways(body: &Value, literal: &str) -> Result<(), Box<dyn Error>> {
    let glyphs = glyphs_for_literal(body, literal);
    require(
        glyphs.len() == literal.chars().count(),
        format!("{literal}: sideways glyph count differs"),
    )?;
    let cluster = glyphs[0]["cluster_index"].clone();
    require(
        glyphs.iter().all(|glyph| {
            glyph["orientation"] == "sideways_cw" && glyph["cluster_index"] == cluster
        }),
        format!("{literal}: sideways cluster contract differs"),
    )
}

fn verify_quantized_elapsed(report: &Value, requested: u64) -> Result<(), Box<dyn Error>> {
    let elapsed = report
        .pointer("/logical_clock/elapsed_millis")
        .and_then(Value::as_u64)
        .ok_or_else(|| failure("logical clock has no elapsed_millis"))?;
    require(
        elapsed >= requested && elapsed < requested + 16,
        format!("logical elapsed {elapsed} is not the 16ms ceiling of {requested}"),
    )
}

fn all_lines_are(body: &Value, writing_mode: &str) -> Result<bool, Box<dyn Error>> {
    Ok(array(body, "lines")?
        .iter()
        .all(|line| line.get("writing_mode").and_then(Value::as_str) == Some(writing_mode)))
}

fn glyph_line(body: &Value, literal: &str) -> Result<u64, Box<dyn Error>> {
    let glyphs = glyphs_for_literal(body, literal);
    require(glyphs.len() == 1, format!("expected one `{literal}` glyph"))?;
    glyphs[0]
        .get("line_index")
        .and_then(Value::as_u64)
        .ok_or_else(|| failure(format!("`{literal}` glyph has no line_index")))
}

fn glyphs_for_literal<'a>(body: &'a Value, literal: &str) -> Vec<&'a Value> {
    let Some(text) = body.get("text").and_then(Value::as_str) else {
        return Vec::new();
    };
    let Some(start) = text.find(literal) else {
        return Vec::new();
    };
    let end = start + literal.len();
    body.get("glyphs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|glyph| {
            let glyph_start = glyph.get("source_start").and_then(Value::as_u64);
            let glyph_end = glyph.get("source_end").and_then(Value::as_u64);
            matches!((glyph_start, glyph_end), (Some(a), Some(b)) if a >= start as u64 && b <= end as u64)
        })
        .collect()
}

fn body(report: &Value) -> Result<&Value, Box<dyn Error>> {
    report
        .get("text")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item.pointer("/owner/kind")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind.starts_with("dialogue:"))
            })
        })
        .ok_or_else(|| failure("frame report has no dialogue View content"))
}

fn font_fingerprints(report: &Value) -> Result<Vec<String>, Box<dyn Error>> {
    let mut fingerprints = array(report, "fonts")?
        .iter()
        .map(|font| {
            Ok(format!(
                "{}:{}",
                u64_field(font, "byte_len")?,
                str_field(font, "fnv1a64")?
            ))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    fingerprints.sort();
    require(
        fingerprints.len() == 2,
        "expected the project font and deterministic fallback font",
    )?;
    Ok(fingerprints)
}

fn rect_center_x(rect: &Value) -> Result<i64, Box<dyn Error>> {
    let x = rect
        .get("x_milli")
        .and_then(Value::as_i64)
        .ok_or_else(|| failure("rectangle has no x_milli"))?;
    let width = rect
        .get("width_milli")
        .and_then(Value::as_i64)
        .ok_or_else(|| failure("rectangle has no width_milli"))?;
    Ok(x.saturating_mul(2).saturating_add(width))
}

fn imq_mse(path: &Path) -> Result<f64, Box<dyn Error>> {
    let report = read_json(path)?;
    report
        .get("metrics")
        .and_then(Value::as_array)
        .and_then(|metrics| metrics.iter().find(|metric| metric["name"] == "mse"))
        .and_then(|metric| metric.get("score"))
        .and_then(Value::as_f64)
        .ok_or_else(|| failure(format!("{} has no numeric MSE score", path.display())))
}

fn require_png(path: &Path) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    require(
        bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]),
        format!("{} is not a PNG", path.display()),
    )
}

fn read_json(path: &Path) -> Result<Value, Box<dyn Error>> {
    serde_json::from_slice(&fs::read(path)?).map_err(Into::into)
}

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, Box<dyn Error>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| failure(format!("missing array `{field}`")))
}

fn str_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, Box<dyn Error>> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| failure(format!("missing string `{field}`")))
}

fn u64_field(value: &Value, field: &str) -> Result<u64, Box<dyn Error>> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| failure(format!("missing unsigned integer `{field}`")))
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), Box<dyn Error>> {
    if condition {
        Ok(())
    } else {
        Err(failure(message))
    }
}

fn failure(message: impl Into<String>) -> Box<dyn Error> {
    std::io::Error::other(message.into()).into()
}
