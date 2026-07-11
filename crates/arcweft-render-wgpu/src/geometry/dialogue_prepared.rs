//! Canonical dialogue-stage preparation using the shared shaped text engine.

use std::collections::BTreeMap;

use arcweft_glyphon::{
    GlyphonTextEngine, PreparedTextItem, TextGlyphPaint, TextGlyphTransform, TextInteractionPlan,
    TextPaintPlan,
};
use arcweft_presentation::fx::{
    Angle, FiniteF32, FxApplication, FxApplicationResolver, FxCapabilitySet, FxDiagnostic,
    FxDiagnosticCode, FxDiagnosticContext, FxEvaluationBudget, FxGraphEvaluator, FxInstanceId,
    FxNamedValue, FxRendererInterface, FxResolvedValue, FxRuntimeValue, FxSampleContext, FxTarget,
    Length, ResolvedFxOperation, ResolvedFxPlan, ResolvedTransform2D, Seconds, Transform2D,
};
use arcweft_render_text::{
    LineDisplayStage, Milli, ResolvedTextDocument, ResolvedTextRuby, ResolvedTextRun,
    ResolvedTextStyle, RichTextAngle, RichTextEffectDescriptor, RichTextEffectPhase,
    RichTextEffectTarget, RichTextParam, RichTextPresentation, RichTextRange, RichTextTransform,
    RichTextTransformOrigin, TextColor, TextFontFamily, TextSlant, TextStyleCascade, TextWeight,
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

struct DialogueFxEvaluator<'a> {
    resolver: &'a dyn FxApplicationResolver,
    capabilities: FxCapabilitySet,
    budgets: BTreeMap<FxInstanceId, FxEvaluationBudget>,
    diagnostics: Vec<FxDiagnostic>,
    offscreen_passes: Vec<ResolvedFxOperation>,
    post_processes: Vec<ResolvedFxOperation>,
    reduce_motion: bool,
}

impl<'a> DialogueFxEvaluator<'a> {
    fn new(resolver: &'a dyn FxApplicationResolver, reduce_motion: bool) -> Self {
        Self {
            resolver,
            capabilities: FxCapabilitySet::canonical(),
            budgets: BTreeMap::new(),
            diagnostics: Vec::new(),
            offscreen_passes: Vec::new(),
            post_processes: Vec::new(),
            reduce_motion,
        }
    }

    fn evaluate(&mut self, application: &FxApplication, ordinal: u32) -> ResolvedFxPlan {
        let binding = match self.resolver.resolve(application) {
            Ok(binding) => binding,
            Err(diagnostic) => {
                let diagnostic = *diagnostic;
                self.record(diagnostic.clone());
                return ResolvedFxPlan::from_diagnostic(diagnostic);
            }
        };
        let budget = self.budgets.entry(binding.instance.instance).or_default();
        let plan = FxGraphEvaluator::evaluate(
            application,
            binding,
            ordinal,
            self.reduce_motion,
            false,
            &self.capabilities,
            budget,
        );
        for diagnostic in plan.diagnostics() {
            self.record(diagnostic.clone());
        }
        plan
    }

    fn unsupported(&mut self, application: &FxApplication, message: impl Into<String>) {
        let instance = self
            .resolver
            .resolve(application)
            .ok()
            .map(|binding| binding.instance.instance);
        self.record(FxDiagnostic::error(
            FxDiagnosticCode::UnsupportedCapability,
            FxDiagnosticContext {
                definition: Some(application.definition().clone()),
                instance,
                source_range: application.source_range(),
                ..FxDiagnosticContext::default()
            },
            message,
        ));
    }

    fn record(&mut self, diagnostic: FxDiagnostic) {
        if !self.diagnostics.contains(&diagnostic) {
            self.diagnostics.push(diagnostic);
        }
    }

    fn retain_frame_operations(&mut self, plan: &ResolvedFxPlan) {
        for operation in plan.offscreen() {
            push_unique(&mut self.offscreen_passes, operation.clone());
        }
        for operation in plan.post_process() {
            push_unique(&mut self.post_processes, operation.clone());
        }
    }
}

fn push_unique(target: &mut Vec<ResolvedFxOperation>, operation: ResolvedFxOperation) {
    if !target.contains(&operation) {
        target.push(operation);
    }
}

fn apply_document_fx<'a>(
    document: &ResolvedTextDocument<'a>,
    fx: &mut DialogueFxEvaluator<'_>,
) -> Result<ResolvedTextDocument<'a>, FramePlanError> {
    let runs = document
        .runs()
        .iter()
        .enumerate()
        .map(|(run_index, run)| {
            let ordinal = u32::try_from(run_index)
                .map_err(|_| FramePlanError::FxOrdinalOverflow { actual: run_index })?;
            let (style, presentation) = apply_before_layout_fx(
                run.style().clone(),
                run.presentation().clone(),
                ordinal,
                fx,
            );
            Ok(ResolvedTextRun::new(
                run.range(),
                run.source_range(),
                style,
                presentation,
                run.source(),
            )?)
        })
        .collect::<Result<Vec<_>, FramePlanError>>()?;
    let ruby = document
        .ruby()
        .iter()
        .enumerate()
        .map(|(ruby_index, annotation)| {
            let ordinal = u32::try_from(ruby_index)
                .map_err(|_| FramePlanError::FxOrdinalOverflow { actual: ruby_index })?;
            let (style, presentation) = apply_before_layout_fx(
                annotation.style().clone(),
                annotation.presentation().clone(),
                ordinal,
                fx,
            );
            Ok(ResolvedTextRuby::new(
                annotation.base_range(),
                annotation.source_base_range(),
                annotation.text(),
                style,
                presentation,
            )?)
        })
        .collect::<Result<Vec<_>, FramePlanError>>()?;
    Ok(ResolvedTextDocument::new(
        document.text(),
        document.source_origin(),
        runs,
        ruby,
        document.revision(),
    )?)
}

fn apply_before_layout_fx(
    mut style: ResolvedTextStyle,
    mut presentation: RichTextPresentation,
    ordinal: u32,
    fx: &mut DialogueFxEvaluator<'_>,
) -> (ResolvedTextStyle, RichTextPresentation) {
    let applications = presentation.fx.clone();
    for application in applications {
        let plan = fx.evaluate(&application, ordinal);
        if !plan.is_conformant() {
            continue;
        }
        if !plan.transition().is_empty() {
            fx.unsupported(
                &application,
                "RichText has no transition-state owner for this Fx application",
            );
            continue;
        }
        let mut candidate_style = style.clone();
        let mut candidate_presentation = presentation.clone();
        let mut unsupported = None;
        for operation in plan.layout() {
            match operation {
                ResolvedFxOperation::Values(operation)
                    if operation.interface == FxRendererInterface::TextStyle
                        && matches!(
                            operation.target,
                            FxTarget::Node | FxTarget::Content | FxTarget::Line
                        ) =>
                {
                    if let Err(message) = apply_text_style_values(
                        &mut candidate_style,
                        &mut candidate_presentation,
                        &operation.values,
                    ) {
                        unsupported = Some(message);
                        break;
                    }
                }
                _ => {
                    unsupported = Some(format!(
                        "RichText cannot apply {:?} at {:?} to {:?} before layout",
                        operation.interface(),
                        operation.phase(),
                        operation.target()
                    ));
                    break;
                }
            }
        }
        if let Some(message) = unsupported {
            fx.unsupported(&application, message);
            continue;
        }
        fx.retain_frame_operations(&plan);
        style = candidate_style;
        presentation = candidate_presentation;
    }
    (style, presentation)
}

fn apply_text_style_values(
    style: &mut ResolvedTextStyle,
    presentation: &mut RichTextPresentation,
    values: &[FxNamedValue],
) -> Result<(), String> {
    for value in values {
        match (value.name.as_str(), &value.value) {
            ("weight", FxResolvedValue::Runtime(FxRuntimeValue::I32(weight))) => {
                *style = style.clone().with_weight(text_weight(*weight)?);
            }
            ("color", FxResolvedValue::Runtime(FxRuntimeValue::Color(color))) => {
                *style = style.clone().with_color(TextColor::from(*color));
            }
            ("font_family", FxResolvedValue::String(family)) => {
                *style = style
                    .clone()
                    .with_font_families(vec![text_font_family(family)])
                    .map_err(|error| error.to_string())?;
            }
            ("size", FxResolvedValue::Runtime(FxRuntimeValue::Length(size))) => {
                let font_size = positive_milli(size.pixels(), "font size")?;
                let line_height = positive_milli(size.pixels() * 1.35, "line height")?;
                *style = style
                    .clone()
                    .with_font_metrics(font_size, line_height)
                    .map_err(|error| error.to_string())?;
            }
            ("spacing", FxResolvedValue::Runtime(FxRuntimeValue::Length(spacing))) => {
                let spacing = signed_milli(spacing.pixels(), "text spacing")?;
                *style = style
                    .clone()
                    .with_spacing(spacing, style.word_spacing_milli());
            }
            ("slant", FxResolvedValue::Runtime(FxRuntimeValue::Angle(angle))) => {
                let degrees = signed_milli(angle.radians().to_degrees(), "text slant")?;
                *style = style.clone().with_slant(TextSlant::Oblique {
                    angle: RichTextAngle {
                        degrees: Milli(degrees),
                    },
                });
            }
            ("opacity", FxResolvedValue::Runtime(FxRuntimeValue::F32(opacity))) => {
                if !(0.0..=1.0).contains(&opacity.get()) {
                    return Err(format!(
                        "RichText opacity {} is outside [0, 1]",
                        opacity.get()
                    ));
                }
                presentation.opacity = Some(Milli(signed_milli(opacity.get(), "opacity")?));
            }
            (name, value) => {
                return Err(format!(
                    "RichText text-style property `{name}` has unsupported value {value:?}"
                ));
            }
        }
    }
    Ok(())
}

fn text_weight(value: i32) -> Result<TextWeight, String> {
    Ok(match value {
        1..=149 => TextWeight::Thin,
        150..=249 => TextWeight::ExtraLight,
        250..=349 => TextWeight::Light,
        350..=449 => TextWeight::Normal,
        450..=549 => TextWeight::Medium,
        550..=649 => TextWeight::SemiBold,
        650..=749 => TextWeight::Bold,
        750..=849 => TextWeight::ExtraBold,
        850..=1_000 => TextWeight::Black,
        _ => return Err(format!("text weight {value} is outside 1..=1000")),
    })
}

fn text_font_family(value: &str) -> TextFontFamily {
    match value.to_ascii_lowercase().as_str() {
        "serif" => TextFontFamily::Serif,
        "sans" | "sans-serif" | "sans_serif" => TextFontFamily::SansSerif,
        "mono" | "monospace" => TextFontFamily::Monospace,
        "cursive" => TextFontFamily::Cursive,
        "fantasy" => TextFontFamily::Fantasy,
        _ => TextFontFamily::Named(value.to_owned()),
    }
}

fn positive_milli(value: f32, label: &'static str) -> Result<u32, String> {
    let milli = f64::from(value) * 1_000.0;
    if !value.is_finite() || value <= 0.0 || milli > f64::from(u32::MAX) {
        return Err(format!(
            "{label} {value} cannot be represented in milli-pixels"
        ));
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "range and sign are validated immediately before deterministic rounding"
    )]
    Ok(milli.round() as u32)
}

fn signed_milli(value: f32, label: &'static str) -> Result<i32, String> {
    let milli = f64::from(value) * 1_000.0;
    if !value.is_finite() || milli < f64::from(i32::MIN) || milli > f64::from(i32::MAX) {
        return Err(format!(
            "{label} {value} cannot be represented in milli-units"
        ));
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "range is validated immediately before deterministic rounding"
    )]
    Ok(milli.round() as i32)
}

pub(super) fn prepare_stage(
    engine: &mut GlyphonTextEngine,
    stage: LineDisplayStage<'_>,
    paragraph: &RenderStyledParagraph,
    viewport: RenderViewport,
    reduce_motion: bool,
    reveal_complete: bool,
    fx_resolver: &dyn FxApplicationResolver,
) -> Result<(PreparedTextItem, bool, Vec<FxDiagnostic>), FramePlanError> {
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
    let mut fx = DialogueFxEvaluator::new(fx_resolver, reduce_motion);
    let document = apply_document_fx(&document, &mut fx)?;
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
    paint.offscreen_passes.append(&mut fx.offscreen_passes);
    paint.post_processes.append(&mut fx.post_processes);
    apply_body_paint(
        &layout,
        &mut paint,
        visible_end,
        effect_seconds,
        reduce_motion,
        &mut fx,
    )?;
    apply_ruby_paint(
        &layout,
        &mut paint,
        visible_end,
        effect_seconds,
        reduce_motion,
        &mut fx,
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
    Ok((item, reveal.complete, fx.diagnostics))
}

fn apply_body_paint(
    layout: &arcweft_text_layout::TextLayout,
    paint: &mut TextPaintPlan,
    visible_end: usize,
    effect_seconds: f32,
    reduce_motion: bool,
    fx: &mut DialogueFxEvaluator<'_>,
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
        apply_glyph_fx(&run.presentation, glyph.logical_ordinal, glyph_paint, fx)?;
    }
    Ok(())
}

fn apply_ruby_paint(
    layout: &arcweft_text_layout::TextLayout,
    paint: &mut TextPaintPlan,
    visible_end: usize,
    effect_seconds: f32,
    reduce_motion: bool,
    fx: &mut DialogueFxEvaluator<'_>,
) -> Result<(), FramePlanError> {
    let mut paint_index = layout.glyphs.len();
    for annotation in &layout.ruby {
        let visible = annotation.base_range.end <= visible_end;
        let opacity = presentation_opacity(&annotation.presentation)?;
        for (glyph_ordinal, glyph) in annotation.glyphs.iter().enumerate() {
            let logical_ordinal =
                u32::try_from(glyph_ordinal).map_err(|_| FramePlanError::FxOrdinalOverflow {
                    actual: glyph_ordinal,
                })?;
            let glyph_paint = &mut paint.glyphs[paint_index];
            glyph_paint.visible &= visible;
            glyph_paint.opacity_milli = opacity;
            glyph_paint.transform = TextGlyphTransform::new(presentation_transform(
                &annotation.presentation,
                glyph.ink_bounds,
                glyph.advance,
                annotation.ruby_bounds,
                logical_ordinal,
                effect_seconds,
                reduce_motion,
            )?);
            apply_glyph_fx(&annotation.presentation, logical_ordinal, glyph_paint, fx)?;
            paint_index += 1;
        }
    }
    Ok(())
}

fn apply_glyph_fx(
    presentation: &RichTextPresentation,
    logical_ordinal: u32,
    glyph_paint: &mut TextGlyphPaint,
    fx: &mut DialogueFxEvaluator<'_>,
) -> Result<(), FramePlanError> {
    for application in &presentation.fx {
        let plan = fx.evaluate(application, logical_ordinal);
        if !plan.is_conformant() {
            continue;
        }
        let mut candidate = glyph_paint.clone();
        let mut transform = candidate.transform.resolved();
        let mut unsupported = None;
        for operation in plan.glyph() {
            if !matches!(
                operation.target(),
                FxTarget::Node | FxTarget::Content | FxTarget::Line | FxTarget::Glyph
            ) {
                unsupported = Some(format!(
                    "RichText glyph paint cannot target {:?}",
                    operation.target()
                ));
                break;
            }
            match operation {
                ResolvedFxOperation::Transform(operation) => {
                    transform = transform.then(operation.transform)?;
                }
                ResolvedFxOperation::Values(operation)
                    if operation.interface == FxRendererInterface::Color =>
                {
                    if let Err(message) = apply_color_values(&mut candidate, &operation.values) {
                        unsupported = Some(message);
                        break;
                    }
                }
                ResolvedFxOperation::Values(operation)
                    if operation.interface == FxRendererInterface::ShaderUniform =>
                {
                    candidate
                        .effects
                        .push(ResolvedFxOperation::Values(operation.clone()));
                }
                ResolvedFxOperation::Values(_) => {
                    unsupported = Some(format!(
                        "RichText glyph paint does not implement interface {:?}",
                        operation.interface()
                    ));
                    break;
                }
            }
        }
        if unsupported.is_none() {
            for operation in plan.mask() {
                if matches!(
                    operation.target(),
                    FxTarget::Node | FxTarget::Content | FxTarget::Line | FxTarget::Glyph
                ) {
                    candidate.masks.push(operation.clone());
                } else {
                    unsupported = Some(format!(
                        "RichText glyph mask cannot target {:?}",
                        operation.target()
                    ));
                    break;
                }
            }
        }
        if let Some(message) = unsupported {
            fx.unsupported(application, message);
            continue;
        }
        candidate.transform = TextGlyphTransform::new(transform);
        *glyph_paint = candidate;
    }
    Ok(())
}

fn apply_color_values(paint: &mut TextGlyphPaint, values: &[FxNamedValue]) -> Result<(), String> {
    for value in values {
        match (value.name.as_str(), &value.value) {
            (
                "tint" | "multiply" | "color",
                FxResolvedValue::Runtime(FxRuntimeValue::Color(color)),
            ) => paint.color = multiply_color(paint.color, TextColor::from(*color)),
            ("opacity", FxResolvedValue::Runtime(FxRuntimeValue::F32(opacity)))
                if (0.0..=1.0).contains(&opacity.get()) =>
            {
                let value = f32::from(paint.opacity_milli) * opacity.get();
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "both operands are validated in [0, 1000] before rounding"
                )]
                {
                    paint.opacity_milli = value.round() as u16;
                }
            }
            (name, value) => {
                return Err(format!(
                    "RichText color property `{name}` has unsupported value {value:?}"
                ));
            }
        }
    }
    Ok(())
}

fn multiply_color(left: TextColor, right: TextColor) -> TextColor {
    let left = left.channels();
    let right = right.channels();
    let channel = |index: usize| {
        let product = u16::from(left[index]) * u16::from(right[index]);
        u8::try_from((product + 127) / 255).expect("normalized u8 color product remains in range")
    };
    TextColor::rgba(channel(0), channel(1), channel(2), channel(3))
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
mod tests;
