//! Closed graph and sampler programs for Arcweft-owned rich-text builtins.

mod attrs;
mod value_expr;

use std::collections::BTreeMap;

use arcweft_presentation::fx::{
    FxContextSlot, FxGraph, FxId, FxNode, FxPhase, FxProperty, FxResourceId, FxRuntimeType,
    FxRuntimeValue, FxSamplerProgram, FxStaticValue, FxTarget,
};
use arcweft_presentation::rich_text::{
    BuiltinRichTextFx, BuiltinRichTextFxPhase, BuiltinRichTextFxProperty,
};

use crate::errors::RuntimePlanLowerError;

use super::{fx_error, presentation_phase};
use attrs::{
    alias_number, alias_seconds, authored_seed, bool_attr, direction, non_negative, number,
    optional_number, parse_color, positive_number, static_f32, static_length,
    validate_symbolic_origin,
};
use value_expr::{
    ProgramExpr, TransformFields, add, angle_expr, context, cos, div, f32_expr, floor_to_i32,
    hash_noise, i32_expr, length_expr, less_equal, make_color, max, mul, sampler, select,
    signed_noise, sin, sub,
};

pub(super) fn effect_graph(
    id: &FxId,
    effect: BuiltinRichTextFx,
    phase: BuiltinRichTextFxPhase,
    target: FxTarget,
    attrs: &BTreeMap<String, String>,
) -> Result<FxGraph, RuntimePlanLowerError> {
    ensure_effect_properties(effect, attrs)?;
    if !effect.supported_phases().contains(&phase) {
        return Err(fx_error(format!(
            "effect `{}` does not support phase {phase:?}",
            effect.selector()
        )));
    }
    let presentation_phase = presentation_phase(phase);
    let node = match phase {
        BuiltinRichTextFxPhase::GlyphTransform => {
            ensure_text_paint_target(effect, target)?;
            FxNode::Transform {
                fx: id.clone(),
                properties: vec![
                    FxProperty::new("target", FxStaticValue::Target(target)),
                    FxProperty::new("phase", FxStaticValue::Phase(presentation_phase)),
                    FxProperty::new(
                        "sampler",
                        FxStaticValue::Sampler(transform_sampler(effect, attrs)?),
                    ),
                ],
            }
        }
        BuiltinRichTextFxPhase::GlyphColor if effect == BuiltinRichTextFx::Sparkle => {
            ensure_text_paint_target(effect, target)?;
            FxNode::Color {
                properties: vec![
                    FxProperty::new("target", FxStaticValue::Target(target)),
                    FxProperty::new("phase", FxStaticValue::Phase(presentation_phase)),
                    FxProperty::new(
                        "tint",
                        FxStaticValue::Sampler(sparkle_color_sampler(attrs)?),
                    ),
                ],
            }
        }
        BuiltinRichTextFxPhase::GlyphMask if effect == BuiltinRichTextFx::Typewriter => {
            ensure_text_paint_target(effect, target)?;
            FxNode::Mask {
                fx: id.clone(),
                properties: vec![
                    FxProperty::new("target", FxStaticValue::Target(target)),
                    FxProperty::new("phase", FxStaticValue::Phase(presentation_phase)),
                    FxProperty::new(
                        "coverage",
                        FxStaticValue::Sampler(typewriter_coverage_sampler(attrs)?),
                    ),
                ],
            }
        }
        BuiltinRichTextFxPhase::PostProcess => post_process_node(id, effect, attrs)?,
        _ => {
            return Err(fx_error(format!(
                "effect `{}` does not implement phase {phase:?}",
                effect.selector()
            )));
        }
    };
    FxGraph::try_new(vec![node])
        .map_err(|error| fx_error(format!("invalid `{}` graph: {error}", effect.selector())))
}

pub(super) fn shader_graph(
    id: &FxId,
    effect: BuiltinRichTextFx,
    phase: BuiltinRichTextFxPhase,
    target: FxTarget,
    attrs: &BTreeMap<String, String>,
) -> Result<FxGraph, RuntimePlanLowerError> {
    debug_assert_eq!(effect, BuiltinRichTextFx::Shader);
    ensure_effect_properties(effect, attrs)?;
    if !effect.supported_phases().contains(&phase) {
        return Err(fx_error(format!(
            "shader shorthand does not implement phase {phase:?}"
        )));
    }
    if phase == BuiltinRichTextFxPhase::PostProcess {
        if target != FxTarget::Viewport {
            return Err(fx_error("post-process shader must target viewport"));
        }
    } else if !matches!(target, FxTarget::Content | FxTarget::Line | FxTarget::Glyph) {
        return Err(fx_error(format!(
            "shader target {target:?} has no RichText paint owner"
        )));
    }
    let resource = attrs
        .get("id")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| fx_error("shader shorthand requires a non-empty `id`"))?;
    let resource = FxResourceId::try_new(resource.clone())
        .map_err(|error| fx_error(format!("invalid shader resource: {error}")))?;
    let mut uniforms = Vec::new();
    if let Some(value) = optional_number(attrs, "amount")? {
        uniforms.push(FxProperty::new("amount", static_f32(value)?));
    }
    if attrs.contains_key("dir") {
        uniforms.push(FxProperty::new(
            "dir",
            FxStaticValue::Runtime(FxRuntimeValue::Vec2(direction(attrs, [0.0, 1.0])?)),
        ));
    }
    if let Some(value) = attrs.get("color") {
        uniforms.push(FxProperty::new(
            "color",
            FxStaticValue::Runtime(FxRuntimeValue::Color(parse_color(value)?)),
        ));
    }
    let node = FxNode::Shader {
        fx: id.clone(),
        properties: vec![
            FxProperty::new("target", FxStaticValue::Target(target)),
            FxProperty::new("phase", FxStaticValue::Phase(presentation_phase(phase))),
            FxProperty::new("resource", FxStaticValue::Resource(resource)),
            FxProperty::new("uniforms", FxStaticValue::Record(uniforms)),
        ],
    };
    FxGraph::try_new(vec![node]).map_err(|error| fx_error(format!("invalid shader graph: {error}")))
}

fn ensure_text_paint_target(
    effect: BuiltinRichTextFx,
    target: FxTarget,
) -> Result<(), RuntimePlanLowerError> {
    if matches!(target, FxTarget::Content | FxTarget::Line | FxTarget::Glyph) {
        Ok(())
    } else {
        Err(fx_error(format!(
            "effect `{}` target {target:?} has no inline RichText paint owner",
            effect.selector()
        )))
    }
}

fn ensure_effect_properties(
    effect: BuiltinRichTextFx,
    attrs: &BTreeMap<String, String>,
) -> Result<(), RuntimePlanLowerError> {
    let schema = effect.property_schema();
    for name in attrs.keys() {
        let Some(property) = BuiltinRichTextFxProperty::from_source_name(name) else {
            return Err(fx_error(format!(
                "effect `{}` has no property named `{name}`",
                effect.selector()
            )));
        };
        if !schema.accepts(property) {
            return Err(fx_error(format!(
                "effect `{}` has no property named `{name}`",
                effect.selector()
            )));
        }
    }
    Ok(())
}

fn transform_sampler(
    effect: BuiltinRichTextFx,
    attrs: &BTreeMap<String, String>,
) -> Result<FxSamplerProgram, RuntimePlanLowerError> {
    let fields = match effect {
        BuiltinRichTextFx::Wave => wave_fields(attrs)?,
        BuiltinRichTextFx::Shake => shake_fields(attrs, false)?,
        BuiltinRichTextFx::Jitter => shake_fields(attrs, true)?,
        BuiltinRichTextFx::Arc => arc_fields(attrs)?,
        BuiltinRichTextFx::Spin => spin_fields(attrs)?,
        BuiltinRichTextFx::Pulse => pulse_fields(attrs)?,
        BuiltinRichTextFx::Sparkle => sparkle_fields(attrs)?,
        BuiltinRichTextFx::Motion => motion_fields(attrs)?,
        BuiltinRichTextFx::Typewriter | BuiltinRichTextFx::Shader => {
            return Err(fx_error(format!(
                "effect `{}` has no transform sampler",
                effect.selector()
            )));
        }
    };
    fields.finish()
}

fn wave_fields(attrs: &BTreeMap<String, String>) -> Result<TransformFields, RuntimePlanLowerError> {
    let amplitude = length_expr(number(attrs, "amp", 4.0)?)?;
    let period = positive_number(attrs, "period", 12.0)?;
    let speed = alias_number(attrs, "speed", "freq", 1.0)?;
    let direction = direction(attrs, [0.0, 1.0])?;
    let phase = mul(
        add(
            div(context(FxContextSlot::Ordinal), f32_expr(period)),
            mul(context(FxContextSlot::Time), f32_expr(speed)),
        ),
        f32_expr(std::f32::consts::TAU),
    );
    let displacement = mul(amplitude, sin(phase));
    Ok(TransformFields {
        translate_x: mul(displacement.clone(), f32_expr(direction.x.get())),
        translate_y: mul(displacement, f32_expr(direction.y.get())),
        ..TransformFields::identity()
    })
}

fn shake_fields(
    attrs: &BTreeMap<String, String>,
    fixed: bool,
) -> Result<TransformFields, RuntimePlanLowerError> {
    let amplitude = length_expr(number(attrs, "amp", 2.0)?)?;
    let bucket = if fixed {
        i32_expr(0)
    } else {
        floor_to_i32(mul(
            context(FxContextSlot::Time),
            f32_expr(number(attrs, "speed", 16.0)?),
        ))
    };
    let noise_x = signed_noise(bucket.clone());
    let noise_y = signed_noise(add(bucket, i32_expr(0x51f1_5e5d)));
    Ok(TransformFields {
        translate_x: mul(amplitude.clone(), noise_x),
        translate_y: mul(amplitude, noise_y),
        ..TransformFields::identity()
    })
}

fn arc_fields(attrs: &BTreeMap<String, String>) -> Result<TransformFields, RuntimePlanLowerError> {
    let radius = length_expr(number(attrs, "radius", 120.0)?)?;
    let angle = add(
        angle_expr(number(attrs, "start", 0.0)?)?,
        mul(
            angle_expr(number(attrs, "step", 8.0)?)?,
            context(FxContextSlot::Ordinal),
        ),
    );
    Ok(TransformFields {
        translate_x: mul(radius.clone(), cos(angle.clone())),
        translate_y: mul(radius, sin(angle.clone())),
        rotation: add(angle, angle_expr(90.0)?),
        ..TransformFields::identity()
    })
}

fn spin_fields(attrs: &BTreeMap<String, String>) -> Result<TransformFields, RuntimePlanLowerError> {
    validate_symbolic_origin(attrs)?;
    let phase = temporal_phase(attrs, 1.0)?;
    let amplitude = angle_expr(alias_number(attrs, "angle", "amp", 6.0)?)?;
    Ok(TransformFields {
        rotation: mul(amplitude, sin(phase)),
        ..TransformFields::identity()
    })
}

fn pulse_fields(
    attrs: &BTreeMap<String, String>,
) -> Result<TransformFields, RuntimePlanLowerError> {
    validate_symbolic_origin(attrs)?;
    let amplitude = non_negative(alias_number(attrs, "amp", "amount", 0.08)?, "amp")?;
    let sample = add(
        mul(sin(temporal_phase(attrs, 1.0)?), f32_expr(0.5)),
        f32_expr(0.5),
    );
    let scale = add(f32_expr(1.0), mul(f32_expr(amplitude), sample));
    Ok(TransformFields {
        scale_x: scale.clone(),
        scale_y: scale,
        ..TransformFields::identity()
    })
}

fn sparkle_phase(attrs: &BTreeMap<String, String>) -> Result<ProgramExpr, RuntimePlanLowerError> {
    let speed = positive_number(attrs, "speed", 2.2)?;
    Ok(mul(
        add(
            add(
                mul(context(FxContextSlot::Time), f32_expr(speed)),
                hash_noise(i32_expr(0)),
            ),
            mul(context(FxContextSlot::Ordinal), f32_expr(0.071)),
        ),
        f32_expr(std::f32::consts::TAU),
    ))
}

fn sparkle_shimmer(attrs: &BTreeMap<String, String>) -> Result<ProgramExpr, RuntimePlanLowerError> {
    Ok(add(
        mul(sin(sparkle_phase(attrs)?), f32_expr(0.5)),
        f32_expr(0.5),
    ))
}

fn sparkle_fields(
    attrs: &BTreeMap<String, String>,
) -> Result<TransformFields, RuntimePlanLowerError> {
    let amplitude = non_negative(number(attrs, "amp", 1.6)?, "amp")?;
    let phase = sparkle_phase(attrs)?;
    let shimmer = sparkle_shimmer(attrs)?;
    let drift = cos(add(
        mul(phase, f32_expr(0.73)),
        mul(hash_noise(i32_expr(1)), f32_expr(std::f32::consts::TAU)),
    ));
    let scale = add(f32_expr(1.0), mul(shimmer.clone(), f32_expr(0.035)));
    Ok(TransformFields {
        translate_x: mul(length_expr(amplitude * 0.18)?, drift),
        translate_y: mul(length_expr(amplitude * -0.35)?, shimmer.clone()),
        scale_x: scale.clone(),
        scale_y: scale,
        opacity: add(f32_expr(0.82), mul(shimmer, f32_expr(0.18))),
        ..TransformFields::identity()
    })
}

fn motion_fields(
    attrs: &BTreeMap<String, String>,
) -> Result<TransformFields, RuntimePlanLowerError> {
    let function = attrs
        .get("fn")
        .or_else(|| attrs.get("curve"))
        .map_or("breath_orbit", String::as_str)
        .trim()
        .trim_start_matches('@')
        .trim_start_matches('.');
    if !matches!(
        function,
        "breath_orbit" | "fx.breath_orbit" | "elastic_bloom" | "fx.elastic_bloom"
    ) {
        return Err(fx_error(format!(
            "shared motion provider `{function}` is not available"
        )));
    }
    let speed = number(attrs, "speed", 1.0)?;
    let amplitude = alias_number(attrs, "amp", "radius", 4.0)?;
    let angle = angle_expr(number(attrs, "angle", 6.0)?)?;
    let scale_amplitude = non_negative(alias_number(attrs, "scale", "scale_amp", 0.08)?, "scale")?;
    let sample_time = add(
        add(
            mul(context(FxContextSlot::Time), f32_expr(speed)),
            mul(context(FxContextSlot::Ordinal), f32_expr(0.037)),
        ),
        mul(hash_noise(i32_expr(0)), f32_expr(0.11)),
    );
    let tau_time = mul(sample_time.clone(), f32_expr(std::f32::consts::TAU));
    let primary = sin(tau_time.clone());
    if function.ends_with("elastic_bloom") {
        let snap = max(
            sin(add(
                mul(tau_time, f32_expr(3.0)),
                mul(hash_noise(i32_expr(1)), f32_expr(0.25)),
            )),
            f32_expr(0.0),
        );
        let snap = mul(snap.clone(), snap);
        let scale = add(f32_expr(1.0), mul(f32_expr(scale_amplitude), snap.clone()));
        return Ok(TransformFields {
            translate_x: mul(length_expr(amplitude * 0.25)?, primary.clone()),
            translate_y: mul(length_expr(amplitude * -0.55)?, snap.clone()),
            rotation: mul(
                angle,
                add(
                    mul(primary, f32_expr(0.35)),
                    mul(snap.clone(), f32_expr(0.65)),
                ),
            ),
            scale_x: scale.clone(),
            scale_y: scale,
            ..TransformFields::identity()
        });
    }
    let secondary = sin(add(
        mul(sample_time, f32_expr(2.0 * std::f32::consts::TAU)),
        hash_noise(i32_expr(0)),
    ));
    let orbit = add(tau_time, mul(secondary.clone(), f32_expr(0.32)));
    let bloom = add(
        mul(
            add(mul(primary.clone(), f32_expr(0.5)), f32_expr(0.5)),
            f32_expr(0.72),
        ),
        mul(
            add(mul(secondary.clone(), f32_expr(0.5)), f32_expr(0.5)),
            f32_expr(0.28),
        ),
    );
    let scale = add(
        f32_expr(1.0),
        mul(f32_expr(scale_amplitude), max(bloom.clone(), f32_expr(0.0))),
    );
    Ok(TransformFields {
        translate_x: mul(
            length_expr(amplitude)?,
            mul(
                cos(orbit.clone()),
                add(f32_expr(0.65), mul(bloom, f32_expr(0.35))),
            ),
        ),
        translate_y: mul(
            length_expr(amplitude)?,
            add(
                mul(sin(orbit), f32_expr(0.48)),
                mul(secondary.clone(), f32_expr(0.18)),
            ),
        ),
        rotation: mul(
            angle,
            add(mul(primary, f32_expr(0.72)), mul(secondary, f32_expr(0.28))),
        ),
        scale_x: scale.clone(),
        scale_y: scale,
        ..TransformFields::identity()
    })
}

fn sparkle_color_sampler(
    attrs: &BTreeMap<String, String>,
) -> Result<FxSamplerProgram, RuntimePlanLowerError> {
    let shimmer = sparkle_shimmer(attrs)?;
    let green = div(
        add(f32_expr(150.0), mul(shimmer.clone(), f32_expr(80.0))),
        f32_expr(255.0),
    );
    let blue = div(
        add(f32_expr(190.0), mul(shimmer, f32_expr(65.0))),
        f32_expr(255.0),
    );
    sampler(
        FxRuntimeType::Color,
        make_color(f32_expr(1.0), green, blue, f32_expr(1.0)),
    )
}

fn typewriter_coverage_sampler(
    attrs: &BTreeMap<String, String>,
) -> Result<FxSamplerProgram, RuntimePlanLowerError> {
    let cps = non_negative(number(attrs, "cps", 28.0)?, "cps")?;
    let delay = non_negative(alias_seconds(attrs, "delay", "start", 0.0)?, "delay")?;
    let progress = mul(
        max(
            sub(context(FxContextSlot::Time), f32_expr(delay)),
            f32_expr(0.0),
        ),
        f32_expr(cps),
    );
    let ordinal = context(FxContextSlot::Ordinal);
    let visible = less_equal(add(ordinal.clone(), f32_expr(1.0)), progress.clone());
    let cursor_enabled = bool_attr(attrs, "cursor")?.unwrap_or(false);
    let hidden_coverage = if cursor_enabled {
        let opacity = alias_number(attrs, "cursor_alpha", "cursor_opacity", 0.35)?;
        if !(0.0..=1.0).contains(&opacity) {
            return Err(fx_error("typewriter cursor opacity must be in [0, 1]"));
        }
        select(
            less_equal(ordinal, progress),
            f32_expr(opacity),
            f32_expr(0.0),
        )
    } else {
        f32_expr(0.0)
    };
    sampler(
        FxRuntimeType::F32,
        select(visible, f32_expr(1.0), hidden_coverage),
    )
}

fn post_process_node(
    id: &FxId,
    effect: BuiltinRichTextFx,
    attrs: &BTreeMap<String, String>,
) -> Result<FxNode, RuntimePlanLowerError> {
    let mut uniforms = Vec::new();
    let resource = match effect {
        BuiltinRichTextFx::Wave | BuiltinRichTextFx::Shake | BuiltinRichTextFx::Jitter => {
            let amplitude = alias_number(attrs, "amp", "amount", 3.0)?;
            let period = positive_number(attrs, "period", 64.0)?;
            let speed = number(attrs, "speed", 1.0)?;
            uniforms.push(FxProperty::new("amplitude", static_length(amplitude)?));
            uniforms.push(FxProperty::new("period", static_length(period)?));
            let phase = if effect == BuiltinRichTextFx::Jitter {
                f32_expr(0.0)
            } else {
                mul(
                    mul(context(FxContextSlot::Time), f32_expr(speed)),
                    f32_expr(std::f32::consts::TAU),
                )
            };
            uniforms.push(FxProperty::new(
                "phase",
                FxStaticValue::Sampler(sampler(FxRuntimeType::F32, phase)?),
            ));
            uniforms.push(FxProperty::new(
                "direction",
                FxStaticValue::Runtime(FxRuntimeValue::Vec2(direction(attrs, [1.0, 0.0])?)),
            ));
            uniforms.push(FxProperty::new(
                "seed",
                FxStaticValue::Runtime(FxRuntimeValue::I32(authored_seed(attrs))),
            ));
            format!("arcweft.post.{}", effect.selector())
        }
        BuiltinRichTextFx::Sparkle => {
            let amount = alias_number(attrs, "amount", "amp", 0.35)?;
            if !(0.0..=1.0).contains(&amount) {
                return Err(fx_error("sparkle post-process amount must be in [0, 1]"));
            }
            uniforms.push(FxProperty::new("amount", static_f32(amount)?));
            uniforms.push(FxProperty::new(
                "phase",
                FxStaticValue::Sampler(sampler(
                    FxRuntimeType::F32,
                    mul(
                        context(FxContextSlot::Time),
                        f32_expr(2.2 * std::f32::consts::TAU),
                    ),
                )?),
            ));
            uniforms.push(FxProperty::new(
                "seed",
                FxStaticValue::Runtime(FxRuntimeValue::I32(authored_seed(attrs))),
            ));
            "arcweft.post.sparkle".to_owned()
        }
        BuiltinRichTextFx::Arc
        | BuiltinRichTextFx::Spin
        | BuiltinRichTextFx::Pulse
        | BuiltinRichTextFx::Motion => {
            let amount = alias_number(attrs, "amount", "amp", 0.18)?;
            if !(0.0..=1.0).contains(&amount) {
                return Err(fx_error(format!(
                    "{} post-process amount must be in [0, 1]",
                    effect.selector()
                )));
            }
            uniforms.push(FxProperty::new("amount", static_f32(amount)?));
            format!("arcweft.post.tint.{}", effect.selector())
        }
        BuiltinRichTextFx::Typewriter | BuiltinRichTextFx::Shader => {
            return Err(fx_error(format!(
                "effect `{}` has no post-process program",
                effect.selector()
            )));
        }
    };
    Ok(FxNode::Shader {
        fx: id.clone(),
        properties: vec![
            FxProperty::new("target", FxStaticValue::Target(FxTarget::Viewport)),
            FxProperty::new("phase", FxStaticValue::Phase(FxPhase::PostProcess)),
            FxProperty::new(
                "resource",
                FxStaticValue::Resource(
                    FxResourceId::try_new(resource)
                        .map_err(|error| fx_error(format!("invalid post resource: {error}")))?,
                ),
            ),
            FxProperty::new("uniforms", FxStaticValue::Record(uniforms)),
        ],
    })
}

fn temporal_phase(
    attrs: &BTreeMap<String, String>,
    default_speed: f32,
) -> Result<ProgramExpr, RuntimePlanLowerError> {
    Ok(mul(
        mul(
            context(FxContextSlot::Time),
            f32_expr(number(attrs, "speed", default_speed)?),
        ),
        f32_expr(std::f32::consts::TAU),
    ))
}
