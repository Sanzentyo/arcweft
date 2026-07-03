use crate::app::image_declarations::{DeclaredImageObject, declaration_arg_value};
use arcweft_bundle::BundleImageObjectBounds;
use arcweft_layout::{
    LayoutSize,
    stage_placement::{
        StageAnchor, StageInsets, StagePlacement, StagePlacementContext, StageRect,
        StageScalePolicy, StageSize,
    },
};
use num_traits::ToPrimitive;
use std::process::ExitCode;

use super::{parse_bool_arg, parse_px_milli, unquote_arg};

pub(super) fn image_stage_placement(
    declaration: &DeclaredImageObject,
) -> Result<StagePlacement, ExitCode> {
    if declaration_arg_value(declaration.args(), "position").is_none() {
        return Ok(StagePlacement::absolute(StageRect::new(
            image_px_milli_arg(declaration, "x")?,
            image_px_milli_arg(declaration, "y")?,
            image_px_milli_arg(declaration, "width").and_then(width_height_milli)?,
            image_px_milli_arg(declaration, "height").and_then(width_height_milli)?,
        )));
    }

    if ["x", "y", "width", "height"]
        .iter()
        .any(|name| declaration_arg_value(declaration.args(), name).is_some())
    {
        eprintln!(
            "error[stage_placement.mixed_absolute_and_anchor]: image object `{}` mixes `position` with absolute x/y/width/height",
            declaration.id()
        );
        return Err(ExitCode::from(2));
    }
    if declaration_arg_value(declaration.args(), "scale.x").is_some()
        || declaration_arg_value(declaration.args(), "scale.y").is_some()
    {
        eprintln!(
            "error[stage_placement.independent_axis_scale_rejected]: image object `{}` uses independent stage scale axes",
            declaration.id()
        );
        return Err(ExitCode::from(2));
    }

    let anchor = declaration_arg_value(declaration.args(), "position")
        .and_then(parse_anchor_position)
        .ok_or_else(|| {
            eprintln!(
                "error[stage_placement.conflicting_fit_and_scale]: image object `{}` has invalid `position`",
                declaration.id()
            );
            ExitCode::from(2)
        })?;
    let object_anchor = declaration_arg_value(declaration.args(), "object_anchor")
        .and_then(|value| StageAnchor::from_keyword(unquote_arg(value)))
        .unwrap_or(anchor);
    let width = image_px_milli_arg_named(declaration, "size.width").and_then(width_height_milli)?;
    let height =
        image_px_milli_arg_named(declaration, "size.height").and_then(width_height_milli)?;
    let margins = StageInsets::new(
        optional_px_milli_arg(declaration, "margin.top"),
        optional_px_milli_arg(declaration, "margin.right"),
        optional_px_milli_arg(declaration, "margin.bottom"),
        optional_px_milli_arg(declaration, "margin.left"),
    );
    let scale = declaration_arg_value(declaration.args(), "scale")
        .and_then(|value| StageScalePolicy::from_keyword(unquote_arg(value)))
        .unwrap_or(StageScalePolicy::Design);

    Ok(
        StagePlacement::anchor(anchor, object_anchor, StageSize::new(width, height))
            .with_margins(margins)
            .with_scale_policy(scale)
            .with_safe_area(
                declaration_arg_value(declaration.args(), "safe_area")
                    .and_then(parse_bool_arg)
                    .unwrap_or(false),
            ),
    )
}

fn parse_anchor_position(value: &str) -> Option<StageAnchor> {
    let value = unquote_arg(value).trim();
    let inner = value.strip_prefix("anchor(")?.strip_suffix(')')?.trim();
    StageAnchor::from_keyword(inner)
}

pub(super) fn image_design_bounds(
    placement: &StagePlacement,
) -> Result<BundleImageObjectBounds, ExitCode> {
    let resolved = placement
        .resolve(StagePlacementContext::new(
            LayoutSize::new(1280.0, 720.0),
            LayoutSize::new(1280.0, 720.0),
        ))
        .map_err(|error| {
            eprintln!("error: image placement failed: {error}");
            ExitCode::from(2)
        })?;
    Ok(BundleImageObjectBounds {
        x_milli: f32_to_i32_milli(resolved.design_bbox.origin.x),
        y_milli: f32_to_i32_milli(resolved.design_bbox.origin.y),
        width_milli: f32_to_u32_milli(resolved.design_bbox.size.width),
        height_milli: f32_to_u32_milli(resolved.design_bbox.size.height),
    })
}

fn image_px_milli_arg_named(
    declaration: &DeclaredImageObject,
    name: &str,
) -> Result<i32, ExitCode> {
    declaration_arg_value(declaration.args(), name)
        .and_then(parse_px_milli)
        .ok_or_else(|| {
            eprintln!(
                "error[stage_placement.missing_size]: image object `{}` is missing or has invalid `{name}`",
                declaration.id()
            );
            ExitCode::from(2)
        })
}

fn optional_px_milli_arg(declaration: &DeclaredImageObject, name: &str) -> i32 {
    declaration_arg_value(declaration.args(), name)
        .and_then(parse_px_milli)
        .unwrap_or_default()
}

fn image_px_milli_arg(declaration: &DeclaredImageObject, name: &str) -> Result<i32, ExitCode> {
    let Some(value) = declaration_arg_value(declaration.args(), name) else {
        eprintln!(
            "error: image object `{}` is missing `{name}`",
            declaration.id()
        );
        return Err(ExitCode::from(2));
    };
    parse_px_milli(value).ok_or_else(|| {
        eprintln!(
            "error: image object `{}` has invalid `{name}` value `{value}`",
            declaration.id()
        );
        ExitCode::from(2)
    })
}

fn width_height_milli(value: i32) -> Result<u32, ExitCode> {
    u32::try_from(value).map_err(|_| {
        eprintln!("error: image width/height must be non-negative");
        ExitCode::from(2)
    })
}

fn f32_to_i32_milli(value: f32) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    (f64::from(value) * 1_000.0)
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX))
        .to_i32()
        .unwrap_or(0)
}

fn f32_to_u32_milli(value: f32) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    (f64::from(value.max(0.0)) * 1_000.0)
        .round()
        .clamp(0.0, f64::from(u32::MAX))
        .to_u32()
        .unwrap_or(0)
}
