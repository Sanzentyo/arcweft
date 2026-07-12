//! Canonical dialogue-stage preparation using the shared shaped text engine.

use std::collections::BTreeMap;

use arcweft_glyphon::{
    GlyphonTextEngine, PreparedTextItem, TextGlyphPaint, TextGlyphTransform, TextInteractionPlan,
    TextPaintPlan,
};
use arcweft_presentation::fx::{
    FxApplication, FxApplicationResolver, FxCapabilitySet, FxDiagnostic, FxDiagnosticCode,
    FxDiagnosticContext, FxEvaluationBudget, FxGraphEvaluator, FxInstanceId, FxNamedValue,
    FxRenderResourceError, FxRenderResourceTable, FxRendererInterface, FxResolvedValue,
    FxRuntimeValue, FxTarget, ResolvedFxMask, ResolvedFxOffscreenPass, ResolvedFxOperation,
    ResolvedFxPlan, ResolvedFxPostProcess,
};
use arcweft_render_text::{
    LineDisplayStage, Milli, ResolvedTextDocument, ResolvedTextRuby, ResolvedTextRun,
    ResolvedTextStyle, RichTextAngle, RichTextPresentation, RichTextRange, TextColor,
    TextFontFamily, TextSlant, TextStyleCascade, TextWeight,
};
use arcweft_text_layout::{LayoutPoint, LayoutSize, TextLayoutRequest, layout_document};
use num_traits::ToPrimitive;

use super::{
    FramePlanError, RenderStyledParagraph, RenderViewport,
    dialogue_legacy_fx::{
        apply_glyph_paint as apply_legacy_glyph_paint, collect_frame_passes, presentation_transform,
    },
    dialogue_timeline::{DialogueRevealPolicy, evaluate_dialogue_reveal},
    prepared_text::{hit_rect_to_layout_rect, resolved_style},
};

struct DialogueFxEvaluator<'a> {
    resolver: &'a dyn FxApplicationResolver,
    capabilities: FxCapabilitySet,
    budgets: BTreeMap<FxInstanceId, FxEvaluationBudget>,
    diagnostics: Vec<FxDiagnostic>,
    resources: FxRenderResourceTable,
    offscreen_passes: Vec<ResolvedFxOffscreenPass>,
    post_processes: Vec<ResolvedFxPostProcess>,
    reduce_motion: bool,
}

impl<'a> DialogueFxEvaluator<'a> {
    fn new(resolver: &'a dyn FxApplicationResolver, reduce_motion: bool) -> Self {
        Self {
            resolver,
            capabilities: FxCapabilitySet::canonical(),
            budgets: BTreeMap::new(),
            diagnostics: Vec::new(),
            resources: FxRenderResourceTable::arcweft_builtins(),
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

    fn retain_frame_operations(
        &mut self,
        application: &FxApplication,
        plan: &ResolvedFxPlan,
    ) -> bool {
        let mut offscreen_passes = Vec::new();
        let mut post_processes = Vec::new();
        for operation in plan.offscreen() {
            let ResolvedFxOperation::Values(operation) = operation else {
                self.unsupported(
                    application,
                    "offscreen transform operation is not executable",
                );
                return false;
            };
            let result = match operation.interface {
                FxRendererInterface::Filter => ResolvedFxOffscreenPass::from_operation(operation)
                    .map(|pass| {
                        if !pass.is_identity() {
                            push_unique(&mut offscreen_passes, pass);
                        }
                    }),
                FxRendererInterface::ShaderUniform => {
                    self.resources.resolve_shader(operation).map(|_| ())
                }
                _ => Err(FxRenderResourceError::WrongInterface {
                    actual: operation.interface,
                }),
            };
            if let Err(error) = result {
                self.unsupported(application, error.to_string());
                return false;
            }
        }
        for operation in plan.post_process() {
            let ResolvedFxOperation::Values(operation) = operation else {
                self.unsupported(
                    application,
                    "post-process transform operation is not executable",
                );
                return false;
            };
            let output = match self.resources.resolve_shader(operation) {
                Ok(output) => output,
                Err(error) => {
                    self.unsupported(application, error.to_string());
                    return false;
                }
            };
            for pass in output.post_processes {
                push_unique(&mut post_processes, pass);
            }
        }
        self.offscreen_passes.extend(offscreen_passes);
        self.post_processes.extend(post_processes);
        true
    }
}

fn push_unique<T: PartialEq>(target: &mut Vec<T>, operation: T) {
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
        if !fx.retain_frame_operations(&application, &plan) {
            continue;
        }
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
) -> Result<(PreparedTextItem, bool, Vec<FxDiagnostic>, usize), FramePlanError> {
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
    let source_origin = document.source_origin();
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
    let (legacy_post_processes, legacy_diagnostics) = collect_frame_passes(
        layout
            .runs
            .iter()
            .map(|run| &run.presentation)
            .chain(layout.ruby.iter().map(|ruby| &ruby.presentation)),
        effect_seconds,
        &fx.resources,
    )?;
    for diagnostic in legacy_diagnostics {
        fx.record(diagnostic);
    }
    for pass in legacy_post_processes {
        push_unique(&mut fx.post_processes, pass);
    }
    apply_body_paint(
        &layout,
        &mut paint,
        visible_end,
        effect_seconds,
        reduce_motion,
        &mut fx,
    )?;
    paint.offscreen_passes.append(&mut fx.offscreen_passes);
    paint.post_processes.append(&mut fx.post_processes);
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
    Ok((item, reveal.complete, fx.diagnostics, source_origin))
}

fn apply_body_paint(
    layout: &arcweft_text_layout::TextLayout,
    paint: &mut TextPaintPlan,
    visible_end: usize,
    effect_seconds: f32,
    reduce_motion: bool,
    fx: &mut DialogueFxEvaluator<'_>,
) -> Result<(), FramePlanError> {
    let mut run_ordinals = BTreeMap::<u32, usize>::new();
    let run_counts = layout
        .glyphs
        .iter()
        .fold(BTreeMap::new(), |mut counts, glyph| {
            *counts.entry(glyph.run_index).or_insert(0usize) += 1;
            counts
        });
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
        let run_ordinal = run_ordinals.entry(glyph.run_index).or_default();
        let diagnostics = apply_legacy_glyph_paint(
            &run.presentation,
            glyph.logical_ordinal,
            *run_ordinal,
            run_counts
                .get(&glyph.run_index)
                .copied()
                .unwrap_or_default(),
            effect_seconds,
            glyph_paint,
            &fx.resources,
        )?;
        *run_ordinal = run_ordinal.saturating_add(1);
        for diagnostic in diagnostics {
            fx.record(diagnostic);
        }
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
            let diagnostics = apply_legacy_glyph_paint(
                &annotation.presentation,
                logical_ordinal,
                glyph_ordinal,
                annotation.glyphs.len(),
                effect_seconds,
                glyph_paint,
                &fx.resources,
            )?;
            for diagnostic in diagnostics {
                fx.record(diagnostic);
            }
            paint_index += 1;
        }
    }
    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "Transactional glyph Fx dispatch validates every closed phase before committing paint."
)]
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
                    match fx.resources.resolve_shader(operation) {
                        Ok(output) if output.post_processes.is_empty() => {
                            candidate.effects.extend(output.glyph_passes);
                        }
                        Ok(_) => {
                            unsupported =
                                Some("glyph shader resolved a non-glyph post-process".to_owned());
                            break;
                        }
                        Err(error) => {
                            unsupported = Some(error.to_string());
                            break;
                        }
                    }
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
            for operation in plan.offscreen() {
                let ResolvedFxOperation::Values(operation) = operation else {
                    continue;
                };
                if operation.interface != FxRendererInterface::ShaderUniform {
                    continue;
                }
                match fx.resources.resolve_shader(operation) {
                    Ok(output) if output.post_processes.is_empty() => {
                        candidate.effects.extend(output.glyph_passes);
                    }
                    Ok(_) => {
                        unsupported =
                            Some("offscreen glyph shader resolved a post-process pass".to_owned());
                        break;
                    }
                    Err(error) => {
                        unsupported = Some(error.to_string());
                        break;
                    }
                }
            }
        }
        if unsupported.is_none() {
            for operation in plan.mask() {
                if matches!(
                    operation.target(),
                    FxTarget::Node | FxTarget::Content | FxTarget::Line | FxTarget::Glyph
                ) {
                    let ResolvedFxOperation::Values(operation) = operation else {
                        unsupported = Some(
                            "RichText glyph mask cannot execute a transform operation".to_owned(),
                        );
                        break;
                    };
                    match ResolvedFxMask::from_operation(operation) {
                        Ok(mask) => candidate.masks.push(mask),
                        Err(error) => {
                            unsupported = Some(error.to_string());
                            break;
                        }
                    }
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

#[cfg(test)]
mod tests;
