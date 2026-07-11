//! Canonical dialogue-stage preparation using the shared shaped text engine.

use arcweft_glyphon::{
    GlyphonTextEngine, PreparedTextItem, TextGlyphTransform, TextInteractionPlan, TextPaintPlan,
};
use arcweft_presentation::fx::{
    Angle, FiniteF32, FxSampleContext, Length, ResolvedTransform2D, Seconds, Transform2D,
};
use arcweft_render_text::{
    LineDisplayStage, RichTextEffectDescriptor, RichTextEffectPhase, RichTextEffectTarget,
    RichTextParam, RichTextPresentation, RichTextRange, RichTextTransform, RichTextTransformOrigin,
    TextStyleCascade,
};
use arcweft_text_layout::{
    LayoutPoint, LayoutRect, LayoutSize, TextLayoutRequest, layout_document,
};
use num_traits::ToPrimitive;

use super::{
    FramePlanError, RenderStyledParagraph, RenderViewport,
    dialogue_timeline::{DialogueRevealPolicy, evaluate_dialogue_reveal},
    prepared_text::{hit_rect_to_layout_rect, resolved_style},
};

pub(super) fn prepare_stage(
    engine: &mut GlyphonTextEngine,
    stage: LineDisplayStage<'_>,
    paragraph: &RenderStyledParagraph,
    viewport: RenderViewport,
    reduce_motion: bool,
    reveal_complete: bool,
) -> Result<(PreparedTextItem, bool), FramePlanError> {
    let runs = stage.text_runs();
    let controls = stage.controls();
    let reveal = evaluate_dialogue_reveal(
        stage.text(),
        &runs,
        &controls,
        stage.reveal_start(),
        DialogueRevealPolicy {
            complete_stage: reveal_complete,
            instant_characters: reduce_motion,
        },
        paragraph.visual_time_millis,
    );
    let cascade = TextStyleCascade::new(resolved_style(&paragraph.default_style)?);
    let document = stage.frame().resolve_stage_document(stage, &cascade)?;
    let document = document.project(RichTextRange::new(
        reveal.display_start,
        document.text().len(),
    ))?;
    let bounds = hit_rect_to_layout_rect(paragraph.bounds);
    let layout = layout_document(
        &document,
        TextLayoutRequest {
            origin: LayoutPoint::new(bounds.x, bounds.y),
            size: LayoutSize::new(bounds.width, bounds.height),
            ..TextLayoutRequest::default()
        },
        engine,
    )?;
    let visible_end = reveal
        .visible_end
        .saturating_sub(reveal.display_start)
        .min(document.text().len());
    let effect_seconds = if reduce_motion {
        0.0
    } else {
        paragraph.visual_time_millis.to_f32().unwrap_or(f32::MAX) / 1_000.0
    };
    let mut paint = TextPaintPlan::from_layout(&layout);
    apply_body_paint(
        &layout,
        &mut paint,
        visible_end,
        effect_seconds,
        reduce_motion,
    )?;
    apply_ruby_paint(
        &layout,
        &mut paint,
        visible_end,
        effect_seconds,
        reduce_motion,
    )?;
    let interaction = TextInteractionPlan::from_layout(&layout, None)
        .with_text_and_selection_color(document.text(), [0.0; 4])
        .with_container_bounds(bounds);
    let item = engine.prepare_text_item(
        layout,
        paint,
        interaction,
        Some(bounds),
        viewport.physical_scale_factor_f32(),
    )?;
    Ok((item, reveal.complete))
}

fn apply_body_paint(
    layout: &arcweft_text_layout::TextLayout,
    paint: &mut TextPaintPlan,
    visible_end: usize,
    effect_seconds: f32,
    reduce_motion: bool,
) -> Result<(), FramePlanError> {
    for (glyph_index, glyph) in layout.glyphs.iter().enumerate() {
        let run = usize::try_from(glyph.run_index)
            .ok()
            .and_then(|index| layout.runs.get(index));
        let Some(run) = run else {
            continue;
        };
        let glyph_paint = &mut paint.glyphs[glyph_index];
        glyph_paint.visible &= glyph.source_range.end <= visible_end;
        glyph_paint.opacity_milli = presentation_opacity(&run.presentation)?;
        glyph_paint.transform = TextGlyphTransform::new(presentation_transform(
            &run.presentation,
            glyph.ink_bounds,
            glyph.advance,
            run.bounds,
            glyph.logical_ordinal,
            effect_seconds,
            reduce_motion,
        )?);
    }
    Ok(())
}

fn apply_ruby_paint(
    layout: &arcweft_text_layout::TextLayout,
    paint: &mut TextPaintPlan,
    visible_end: usize,
    effect_seconds: f32,
    reduce_motion: bool,
) -> Result<(), FramePlanError> {
    let mut paint_index = layout.glyphs.len();
    for annotation in &layout.ruby {
        let visible = annotation.base_range.end <= visible_end;
        let opacity = presentation_opacity(&annotation.presentation)?;
        for (glyph_ordinal, glyph) in annotation.glyphs.iter().enumerate() {
            let glyph_paint = &mut paint.glyphs[paint_index];
            glyph_paint.visible &= visible;
            glyph_paint.opacity_milli = opacity;
            glyph_paint.transform = TextGlyphTransform::new(presentation_transform(
                &annotation.presentation,
                glyph.ink_bounds,
                glyph.advance,
                annotation.ruby_bounds,
                u32::try_from(glyph_ordinal).unwrap_or(u32::MAX),
                effect_seconds,
                reduce_motion,
            )?);
            paint_index += 1;
        }
    }
    Ok(())
}

fn presentation_opacity(presentation: &RichTextPresentation) -> Result<u16, FramePlanError> {
    let value = presentation.opacity.map_or(1_000, |opacity| opacity.0);
    u16::try_from(value)
        .ok()
        .filter(|value| *value <= 1_000)
        .ok_or(FramePlanError::InvalidRichTextOpacity { value })
}

fn presentation_transform(
    presentation: &RichTextPresentation,
    glyph_bounds: LayoutRect,
    glyph_advance: LayoutSize,
    run_bounds: LayoutRect,
    logical_ordinal: u32,
    effect_seconds: f32,
    reduce_motion: bool,
) -> Result<ResolvedTransform2D, FramePlanError> {
    let mut resolved = ResolvedTransform2D::identity();
    if let Some(transform) = &presentation.transform {
        resolved = resolved.then(resolve_authored_transform(
            transform,
            glyph_bounds,
            glyph_advance,
            run_bounds,
        )?)?;
    }
    for effect in &presentation.effects {
        if let Some(transform) = builtin_effect_transform(
            effect,
            glyph_bounds,
            glyph_advance,
            run_bounds,
            logical_ordinal,
            effect_seconds,
            reduce_motion,
        )? {
            resolved = resolved.then(transform.resolve()?)?;
        }
    }
    Ok(resolved)
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
    );
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

#[allow(clippy::too_many_arguments)]
fn builtin_effect_transform(
    effect: &RichTextEffectDescriptor,
    glyph_bounds: LayoutRect,
    glyph_advance: LayoutSize,
    run_bounds: LayoutRect,
    logical_ordinal: u32,
    effect_seconds: f32,
    reduce_motion: bool,
) -> Result<Option<Transform2D>, FramePlanError> {
    if effect.phase != RichTextEffectPhase::GlyphTransform {
        return Ok(None);
    }
    let mut transform = Transform2D::default();
    let mut origin = RichTextTransformOrigin::GlyphCenter;
    match effect.id.as_str() {
        "wave" => {
            let amplitude = effect_value(effect, "amp", 4.0)?;
            let period = effect_value(effect, "period", 12.0)?;
            if period <= 0.0 {
                return Err(invalid_effect_parameter(effect, "period"));
            }
            let speed = effect_value_alias(effect, "speed", "freq", 1.0)?;
            let authored_phase = effect_value(effect, "phase", 0.0)?;
            let direction = effect_direction(effect, [0.0, 1.0])?;
            let phase = (logical_ordinal.to_f32().unwrap_or(f32::MAX) / period
                + effect_seconds * speed
                + authored_phase)
                * std::f32::consts::TAU;
            let delta = amplitude * phase.sin();
            transform.translate_x = Length::try_pixels(direction[0] * delta)?;
            transform.translate_y = Length::try_pixels(direction[1] * delta)?;
        }
        "shake" | "jitter" => {
            let amplitude = effect_value(effect, "amp", 2.0)?;
            let speed = effect_value(effect, "speed", 16.0)?;
            let bucket = if effect.id == "jitter" || reduce_motion {
                0
            } else {
                (effect_seconds * speed)
                    .floor()
                    .to_i32()
                    .ok_or_else(|| invalid_effect_parameter(effect, "speed"))?
            };
            let context = FxSampleContext::from_elapsed(
                Seconds::try_seconds(effect_seconds)?,
                logical_ordinal,
                effect_seed(effect),
                reduce_motion,
            );
            let x = context.deterministic_noise(bucket)?.get() * 2.0 - 1.0;
            let y = context
                .deterministic_noise(bucket.wrapping_add(0x51f1_5e5d))?
                .get()
                * 2.0
                - 1.0;
            transform.translate_x = Length::try_pixels(x * amplitude)?;
            transform.translate_y = Length::try_pixels(y * amplitude)?;
        }
        "arc" => {
            let radius = effect_value(effect, "radius", 120.0)?;
            let start = effect_value(effect, "start", 0.0)?;
            let step = effect_value(effect, "step", 8.0)?;
            let angle = (start + step * logical_ordinal.to_f32().unwrap_or(f32::MAX)).to_radians();
            transform.translate_x = Length::try_pixels(radius * angle.cos())?;
            transform.translate_y = Length::try_pixels(radius * angle.sin())?;
            transform.rotation = Angle::try_radians(angle + std::f32::consts::FRAC_PI_2)?;
        }
        "spin" => {
            let angle = effect_value_alias(effect, "angle", "amp", 6.0)?;
            let speed = effect_value(effect, "speed", 1.0)?;
            let phase = effect_value(effect, "phase", 0.0)?;
            let sample = (effect_seconds * speed + phase) * std::f32::consts::TAU;
            transform.rotation = Angle::try_degrees(f64::from(angle * sample.sin()))?;
            origin = effect_origin(effect)?.unwrap_or(RichTextTransformOrigin::Center);
        }
        "pulse" => {
            let amplitude = effect_value_alias(effect, "amp", "amount", 0.08)?;
            if amplitude < 0.0 {
                return Err(invalid_effect_parameter(effect, "amp"));
            }
            let speed = effect_value(effect, "speed", 1.0)?;
            let phase = effect_value(effect, "phase", 0.0)?;
            let sample = (effect_seconds * speed + phase) * std::f32::consts::TAU;
            let scale = 1.0 + amplitude * (sample.sin() * 0.5 + 0.5);
            transform.scale_x = FiniteF32::try_new(scale)?;
            transform.scale_y = FiniteF32::try_new(scale)?;
            origin = effect_origin(effect)?.unwrap_or(RichTextTransformOrigin::Center);
        }
        _ => return Ok(None),
    }
    let [origin_x, origin_y] = transform_origin(
        origin,
        effect.target,
        glyph_bounds,
        glyph_advance,
        run_bounds,
    );
    transform.origin_x = Length::try_pixels(origin_x)?;
    transform.origin_y = Length::try_pixels(origin_y)?;
    Ok(Some(transform))
}

fn transform_origin(
    origin: RichTextTransformOrigin,
    target: RichTextEffectTarget,
    glyph_bounds: LayoutRect,
    glyph_advance: LayoutSize,
    run_bounds: LayoutRect,
) -> [f32; 2] {
    let target_bounds = match target {
        RichTextEffectTarget::Glyph => glyph_bounds,
        RichTextEffectTarget::Document
        | RichTextEffectTarget::Line
        | RichTextEffectTarget::Sentence
        | RichTextEffectTarget::Run
        | RichTextEffectTarget::TextBox
        | RichTextEffectTarget::Screen => run_bounds,
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
    [global[0] - glyph_bounds.x, global[1] - glyph_bounds.y]
}

fn effect_value(
    effect: &RichTextEffectDescriptor,
    name: &'static str,
    default: f32,
) -> Result<f32, FramePlanError> {
    effect
        .params
        .get(name)
        .map(|value| effect_param_value(effect, name, value))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn effect_value_alias(
    effect: &RichTextEffectDescriptor,
    name: &'static str,
    alias: &'static str,
    default: f32,
) -> Result<f32, FramePlanError> {
    if effect.params.contains_key(name) {
        effect_value(effect, name, default)
    } else {
        effect_value(effect, alias, default)
    }
}

fn effect_param_value(
    effect: &RichTextEffectDescriptor,
    name: &'static str,
    value: &RichTextParam,
) -> Result<f32, FramePlanError> {
    let parsed = match value {
        RichTextParam::Int { value } => value.to_f32(),
        RichTextParam::Milli { value } => Some(value.as_f32()),
        RichTextParam::Raw { value } | RichTextParam::Text { value } => {
            let value = value.trim();
            let numeric = ["px", "deg", "ms", "s", "ch"]
                .iter()
                .find_map(|suffix| value.strip_suffix(suffix))
                .unwrap_or(value)
                .trim();
            numeric
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
        }
        RichTextParam::Bool { .. }
        | RichTextParam::Vec2 { .. }
        | RichTextParam::Selector { .. }
        | RichTextParam::Expr { .. } => None,
    };
    parsed.ok_or_else(|| invalid_effect_parameter(effect, name))
}

fn effect_direction(
    effect: &RichTextEffectDescriptor,
    default: [f32; 2],
) -> Result<[f32; 2], FramePlanError> {
    if let Some(value) = effect.params.get("dir") {
        return match value {
            RichTextParam::Vec2 { value } => Ok([value.x.as_f32(), value.y.as_f32()]),
            RichTextParam::Raw { value } | RichTextParam::Text { value } => {
                let (x, y) = value
                    .split_once(',')
                    .ok_or_else(|| invalid_effect_parameter(effect, "dir"))?;
                Ok([
                    x.trim()
                        .parse()
                        .map_err(|_| invalid_effect_parameter(effect, "dir"))?,
                    y.trim()
                        .parse()
                        .map_err(|_| invalid_effect_parameter(effect, "dir"))?,
                ])
            }
            _ => Err(invalid_effect_parameter(effect, "dir")),
        };
    }
    if let Some(value) = effect.params.get("axis") {
        let axis = match value {
            RichTextParam::Raw { value }
            | RichTextParam::Text { value }
            | RichTextParam::Selector { value } => value.trim().trim_start_matches('.'),
            _ => return Err(invalid_effect_parameter(effect, "axis")),
        };
        return match axis {
            "x" => Ok([1.0, 0.0]),
            "y" => Ok([0.0, 1.0]),
            _ => Err(invalid_effect_parameter(effect, "axis")),
        };
    }
    Ok(default)
}

fn effect_origin(
    effect: &RichTextEffectDescriptor,
) -> Result<Option<RichTextTransformOrigin>, FramePlanError> {
    let Some(value) = effect.params.get("origin") else {
        return Ok(None);
    };
    let value = match value {
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value } => value.trim().trim_start_matches('.'),
        _ => return Err(invalid_effect_parameter(effect, "origin")),
    };
    match value {
        "baseline_start" | "start" => Ok(Some(RichTextTransformOrigin::BaselineStart)),
        "baseline_center" => Ok(Some(RichTextTransformOrigin::BaselineCenter)),
        "center" => Ok(Some(RichTextTransformOrigin::Center)),
        "glyph_center" | "glyph" => Ok(Some(RichTextTransformOrigin::GlyphCenter)),
        _ => Err(invalid_effect_parameter(effect, "origin")),
    }
}

fn effect_seed(effect: &RichTextEffectDescriptor) -> u64 {
    let Some(seed) = effect.params.get("seed") else {
        return 0;
    };
    match seed {
        RichTextParam::Bool { value } => u64::from(*value),
        RichTextParam::Int { value } => u64::from_ne_bytes(value.to_ne_bytes()),
        RichTextParam::Milli { value } => u64::from_ne_bytes(i64::from(value.0).to_ne_bytes()),
        RichTextParam::Vec2 { value } => {
            u64::from_ne_bytes(i64::from(value.x.0).to_ne_bytes())
                ^ u64::from_ne_bytes(i64::from(value.y.0).to_ne_bytes()).rotate_left(17)
        }
        RichTextParam::Raw { value }
        | RichTextParam::Text { value }
        | RichTextParam::Selector { value }
        | RichTextParam::Expr { source: value } => value
            .as_bytes()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
            }),
    }
}

fn invalid_effect_parameter(
    effect: &RichTextEffectDescriptor,
    parameter: &'static str,
) -> FramePlanError {
    FramePlanError::InvalidRichTextEffectParameter {
        effect: effect.id.clone(),
        parameter,
    }
}

#[cfg(test)]
mod tests {
    use arcweft_core::plan::RuntimeLineId;
    use arcweft_presentation::hit::HitRect;
    use arcweft_render_text::{
        InlineFailurePolicy, LineDisplaySpec, Milli, RichTextControl, RichTextDocument,
        RichTextEffectDescriptor, RichTextEffectPhase, RichTextEffectTarget, RichTextLayout,
        RichTextNode, RichTextParam, RichTextStateScope, RichTextStyle, RichTextWritingMode,
        RuntimeLineContext,
    };
    use std::collections::BTreeMap;

    use super::*;
    use crate::geometry::{
        RenderFontFamily, RenderTextReveal, RenderTextSlant, RenderTextStyle, RenderTextWeight,
    };

    const TEST_FONT: &[u8] = include_bytes!("../../../../web/assets/noto-sans-jp-vf.ttf");

    #[test]
    fn vertical_ruby_stage_uses_canonical_layout_and_prepared_glyphs() {
        let frame = frame(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Layout {
                    layout: RichTextLayout {
                        writing_mode: RichTextWritingMode::VerticalRl,
                        ..RichTextLayout::default()
                    },
                },
            },
            RichTextNode::Ruby {
                base: "漢字".to_owned(),
                ruby: "かんじ".to_owned(),
            },
            RichTextNode::Text {
                text: "ABC2026".to_owned(),
            },
        ]);
        let stage = frame.stage(0).expect("stage");
        let mut engine = GlyphonTextEngine::from_project_fonts("ja", vec![TEST_FONT.to_vec()])
            .expect("font engine");

        let (item, complete) =
            prepare_stage(&mut engine, stage, &paragraph(0), viewport(), false, true)
                .expect("stage prepares");

        assert!(complete);
        assert!(!item.layout.ruby.is_empty());
        assert!(
            item.layout
                .runs
                .iter()
                .all(|run| run.writing_mode == RichTextWritingMode::VerticalRl)
        );
        assert_eq!(item.glyphs.len(), item.paint.glyphs.len());
        assert!(item.paint.glyphs.iter().all(|glyph| glyph.visible));
    }

    #[test]
    fn reveal_changes_only_paint() {
        let frame = frame(vec![RichTextNode::Text {
            text: "after".to_owned(),
        }]);
        let stage = frame.stage(0).expect("stage");
        let mut engine = GlyphonTextEngine::from_project_fonts("en", vec![TEST_FONT.to_vec()])
            .expect("font engine");
        let (hidden, _) =
            prepare_stage(&mut engine, stage, &paragraph(0), viewport(), false, false)
                .expect("timed stage prepares");
        let (complete, _) =
            prepare_stage(&mut engine, stage, &paragraph(0), viewport(), false, true)
                .expect("complete stage prepares");

        assert_eq!(hidden.layout.hash, complete.layout.hash);
        assert_eq!(complete.interaction.text, "after");
        assert!(hidden.paint.glyphs.iter().all(|glyph| !glyph.visible));
        assert!(complete.paint.glyphs.iter().all(|glyph| glyph.visible));
    }

    #[test]
    fn clear_projects_the_remaining_stage_to_the_textbox_origin() {
        let frame = frame(vec![
            RichTextNode::Text {
                text: "before".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Clear,
            },
            RichTextNode::Text {
                text: "after".to_owned(),
            },
        ]);
        let stage = frame.stage(0).expect("stage");
        let mut engine = GlyphonTextEngine::from_project_fonts("en", vec![TEST_FONT.to_vec()])
            .expect("font engine");

        let (item, complete) =
            prepare_stage(&mut engine, stage, &paragraph(0), viewport(), false, true)
                .expect("cleared stage prepares");

        assert!(complete);
        assert_eq!(item.interaction.text, "after");
        assert!(
            item.layout
                .glyphs
                .iter()
                .all(|glyph| glyph.layout_bounds.x >= 20.0)
        );
    }

    #[test]
    fn wave_uses_logical_glyph_ordinal_and_time_only_changes_paint() {
        let frame = frame(vec![
            RichTextNode::StyleStart {
                style: RichTextStyle::Effect {
                    effect: RichTextEffectDescriptor {
                        id: "wave".to_owned(),
                        params: BTreeMap::from([
                            (
                                "amp".to_owned(),
                                RichTextParam::Milli {
                                    value: Milli(4_000),
                                },
                            ),
                            (
                                "period".to_owned(),
                                RichTextParam::Milli {
                                    value: Milli(8_000),
                                },
                            ),
                        ]),
                        target: RichTextEffectTarget::Glyph,
                        phase: RichTextEffectPhase::GlyphTransform,
                        state_scope: RichTextStateScope::Glyph,
                    },
                },
            },
            RichTextNode::Text {
                text: "漢字".to_owned(),
            },
        ]);
        let stage = frame.stage(0).expect("stage");
        let mut engine = GlyphonTextEngine::from_project_fonts("ja", vec![TEST_FONT.to_vec()])
            .expect("font engine");
        let (at_zero, _) =
            prepare_stage(&mut engine, stage, &paragraph(0), viewport(), false, true)
                .expect("zero-time stage prepares");
        let (later, _) =
            prepare_stage(&mut engine, stage, &paragraph(500), viewport(), false, true)
                .expect("later stage prepares");

        assert_eq!(at_zero.layout.hash, later.layout.hash);
        assert_ne!(at_zero.paint, later.paint);
        let first_y = at_zero.paint.glyphs[0].transform.resolved().translation()[1].pixels();
        let second_y = at_zero.paint.glyphs[1].transform.resolved().translation()[1].pixels();
        assert!(first_y.abs() <= 0.001);
        assert!((second_y - std::f32::consts::FRAC_1_SQRT_2 * 4.0).abs() <= 0.001);
    }

    fn frame(nodes: Vec<RichTextNode>) -> arcweft_render_text::LineDisplayFrame {
        LineDisplaySpec {
            line: RuntimeLineId::canonical("prepared.dialogue.test").expect("line id"),
            callee: "narrator".to_owned(),
            speaker_label: None,
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: Some(InlineFailurePolicy::FailLine),
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(nodes),
        }
        .resolve_frame(&RuntimeLineContext::default())
        .expect("frame resolves")
    }

    fn paragraph(visual_time_millis: u64) -> RenderStyledParagraph {
        RenderStyledParagraph {
            text: String::new(),
            bounds: HitRect::new(20.0, 30.0, 360.0, 180.0),
            default_style: RenderTextStyle {
                font_size: 24.0,
                line_height: 32.0,
                color: [245, 245, 245, 255],
                font_family: RenderFontFamily::SansSerif,
                weight: RenderTextWeight::Regular,
                slant: RenderTextSlant::Upright,
            },
            spans: Vec::new(),
            reveal: RenderTextReveal {
                visible_end: 0,
                complete: false,
            },
            glyph_transforms: Vec::new(),
            visual_time_millis,
        }
    }

    fn viewport() -> RenderViewport {
        RenderViewport {
            logical_width: 640.0,
            logical_height: 360.0,
            physical_width: 1_280,
            physical_height: 720,
            scale_factor: 2.0,
        }
    }
}
