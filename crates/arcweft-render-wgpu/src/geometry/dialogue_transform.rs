//! Authored static `RichText` transform resolution against shaped geometry.

use arcweft_presentation::fx::{
    Angle, FiniteF32, FxTarget, Length, ResolvedTransform2D, Transform2D,
};
use arcweft_text_layout::{LayoutRect, LayoutSize};
use arcweft_text_model::{RichTextPresentation, RichTextTransform, RichTextTransformOrigin};

use super::FramePlanError;

pub(super) fn presentation_transform(
    presentation: &RichTextPresentation,
    glyph_bounds: LayoutRect,
    glyph_advance: LayoutSize,
    run_bounds: LayoutRect,
) -> Result<ResolvedTransform2D, FramePlanError> {
    presentation.transform.as_ref().map_or_else(
        || Ok(ResolvedTransform2D::identity()),
        |transform| resolve_authored_transform(transform, glyph_bounds, glyph_advance, run_bounds),
    )
}

fn resolve_authored_transform(
    transform: &RichTextTransform,
    glyph_bounds: LayoutRect,
    glyph_advance: LayoutSize,
    run_bounds: LayoutRect,
) -> Result<ResolvedTransform2D, FramePlanError> {
    let [origin_x, origin_y] = transform_origin(
        transform.origin,
        transform.target,
        glyph_bounds,
        glyph_advance,
        run_bounds,
    )?;
    Ok(Transform2D {
        translate_x: Length::try_pixels(transform.translate.x.as_f32())?,
        translate_y: Length::try_pixels(transform.translate.y.as_f32())?,
        scale_x: FiniteF32::try_new(transform.scale.x.as_f32())?,
        scale_y: FiniteF32::try_new(transform.scale.y.as_f32())?,
        skew_x: Angle::try_degrees(f64::from(transform.skew.x.as_f32()))?,
        skew_y: Angle::try_degrees(f64::from(transform.skew.y.as_f32()))?,
        rotation: Angle::try_degrees(f64::from(transform.rotate.as_degrees_f32()))?,
        origin_x: Length::try_pixels(origin_x)?,
        origin_y: Length::try_pixels(origin_y)?,
        opacity: FiniteF32::ONE,
    }
    .resolve()?)
}

fn transform_origin(
    origin: RichTextTransformOrigin,
    target: FxTarget,
    glyph_bounds: LayoutRect,
    glyph_advance: LayoutSize,
    run_bounds: LayoutRect,
) -> Result<[f32; 2], FramePlanError> {
    let target_bounds = match target {
        FxTarget::Glyph => glyph_bounds,
        FxTarget::Content | FxTarget::Line => run_bounds,
        FxTarget::Node | FxTarget::Background | FxTarget::Viewport => {
            return Err(FramePlanError::UnsupportedRichTextTransformTarget { target });
        }
    };
    let global = match origin {
        RichTextTransformOrigin::BaselineStart => [target_bounds.x, target_bounds.y],
        RichTextTransformOrigin::BaselineCenter => [
            glyph_bounds.x + glyph_advance.width * 0.5,
            glyph_bounds.y + glyph_advance.height * 0.5,
        ],
        RichTextTransformOrigin::Center | RichTextTransformOrigin::GlyphCenter => [
            target_bounds.x + target_bounds.width * 0.5,
            target_bounds.y + target_bounds.height * 0.5,
        ],
    };
    Ok([global[0] - glyph_bounds.x, global[1] - glyph_bounds.y])
}
