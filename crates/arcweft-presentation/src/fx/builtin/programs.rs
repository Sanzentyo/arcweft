//! Parameterized graph and sampler construction for builtin Fx callables.

mod expression;

use crate::fx::{
    Angle, FiniteF32, FxGraph, FxNode, FxPhase, FxProperty, FxPropertyId, FxResourceId,
    FxRuntimeType, FxSamplerProgram, FxStaticValue, FxUniformField, FxUniformProgram,
    FxUniformRecord, FxUniformValue, MotionFunction,
};

use super::{
    BuiltinFxBuildError, BuiltinFxCallableId, BuiltinFxCallableRowId, BuiltinFxParameterId,
    GraphRuntimeArgument, GraphTemplateContext,
};
use expression::{
    ProgramExpr, TransformFields, add, angle, bitcast_u32_to_i32, context, cos, div, f32,
    f32_const, floor_to_i32, hash_noise, i32, less_equal, make_color, make_vec2, max, mul,
    parameter, sampler, seconds_value, select, signed_noise, sin, sub, value, vec2_x, vec2_y,
};

pub(super) fn build_graph(context: &GraphTemplateContext) -> Result<FxGraph, BuiltinFxBuildError> {
    use BuiltinFxCallableRowId as Row;
    let graph = match context.specialization().row() {
        Row::WaveGlyphTransform => transform_graph(context, wave_transform(context)?)?,
        Row::ShakeGlyphTransform => transform_graph(context, noise_transform(context, true)?)?,
        Row::JitterGlyphTransform => transform_graph(context, noise_transform(context, false)?)?,
        Row::ArcGlyphTransform => transform_graph(context, arc_transform(context)?)?,
        Row::SpinGlyphTransform => transform_graph(context, spin_transform(context)?)?,
        Row::PulseGlyphTransform => transform_graph(context, pulse_transform(context)?)?,
        Row::MotionGlyphTransform => transform_graph(context, motion_transform(context)?)?,
        Row::SparkleGlyphTransform => transform_graph(context, sparkle_transform(context)?)?,
        Row::SparkleGlyphColor => color_graph(context, sparkle_color(context)?)?,
        Row::TypewriterGlyphMask => mask_graph(context, typewriter_coverage(context)?)?,
        Row::WavePostProcess
        | Row::ShakePostProcess
        | Row::JitterPostProcess
        | Row::ArcPostProcess
        | Row::SpinPostProcess
        | Row::PulsePostProcess
        | Row::MotionPostProcess
        | Row::SparklePostProcess => post_process_graph(context)?,
        Row::ShaderGlyphColor | Row::ShaderOffscreenPass | Row::ShaderPostProcess => {
            shader_graph(context)?
        }
    };
    Ok(graph)
}

fn transform_graph(
    context: &GraphTemplateContext,
    sampler: FxSamplerProgram,
) -> Result<FxGraph, BuiltinFxBuildError> {
    Ok(FxGraph::try_new(vec![FxNode::Transform {
        fx: context.definition().clone(),
        properties: vec![
            FxProperty::new(
                FxPropertyId::Target,
                FxStaticValue::Target(context.specialization().target()),
            ),
            FxProperty::new(
                FxPropertyId::Phase,
                FxStaticValue::Phase(FxPhase::GlyphTransform),
            ),
            FxProperty::new(FxPropertyId::Sampler, FxStaticValue::Sampler(sampler)),
        ],
    }])?)
}

fn color_graph(
    context: &GraphTemplateContext,
    sampler: FxSamplerProgram,
) -> Result<FxGraph, BuiltinFxBuildError> {
    Ok(FxGraph::try_new(vec![FxNode::Color {
        properties: vec![
            FxProperty::new(
                FxPropertyId::Target,
                FxStaticValue::Target(context.specialization().target()),
            ),
            FxProperty::new(
                FxPropertyId::Phase,
                FxStaticValue::Phase(FxPhase::GlyphColor),
            ),
            FxProperty::new(FxPropertyId::Tint, FxStaticValue::Sampler(sampler)),
        ],
    }])?)
}

fn mask_graph(
    context: &GraphTemplateContext,
    sampler: FxSamplerProgram,
) -> Result<FxGraph, BuiltinFxBuildError> {
    Ok(FxGraph::try_new(vec![FxNode::Mask {
        fx: context.definition().clone(),
        properties: vec![
            FxProperty::new(
                FxPropertyId::Target,
                FxStaticValue::Target(context.specialization().target()),
            ),
            FxProperty::new(
                FxPropertyId::Phase,
                FxStaticValue::Phase(FxPhase::GlyphMask),
            ),
            FxProperty::new(FxPropertyId::Coverage, FxStaticValue::Sampler(sampler)),
        ],
    }])?)
}

fn post_process_graph(context: &GraphTemplateContext) -> Result<FxGraph, BuiltinFxBuildError> {
    let callable = context.specialization().row().callable();
    let (resource, mut uniforms) = match callable {
        BuiltinFxCallableId::Wave | BuiltinFxCallableId::Shake | BuiltinFxCallableId::Jitter => {
            let resource = match callable {
                BuiltinFxCallableId::Wave => "arcweft.post.wave",
                BuiltinFxCallableId::Shake => "arcweft.post.shake",
                BuiltinFxCallableId::Jitter => "arcweft.post.jitter",
                _ => unreachable!("matched displacement builtin"),
            };
            let speed = if context.has_parameter(BuiltinFxParameterId::Speed) {
                runtime(context, BuiltinFxParameterId::Speed)?
            } else {
                f32(FiniteF32::ZERO)
            };
            (
                resource,
                vec![
                    uniform_parameter(context, "amplitude", BuiltinFxParameterId::Amplitude)?,
                    uniform_parameter(context, "period", BuiltinFxParameterId::Period)?,
                    uniform_program(context, "phase", temporal_phase(speed))?,
                    uniform_vec2(context, "direction", BuiltinFxParameterId::Direction)?,
                    uniform_seed(context, "seed", BuiltinFxParameterId::Seed)?,
                ],
            )
        }
        BuiltinFxCallableId::Sparkle => (
            "arcweft.post.sparkle",
            vec![
                uniform_parameter(context, "amount", BuiltinFxParameterId::Amount)?,
                uniform_program(context, "phase", temporal_phase(f32_const(2.2)))?,
                uniform_seed(context, "seed", BuiltinFxParameterId::Seed)?,
            ],
        ),
        BuiltinFxCallableId::Arc
        | BuiltinFxCallableId::Spin
        | BuiltinFxCallableId::Pulse
        | BuiltinFxCallableId::Motion => {
            let resource = match callable {
                BuiltinFxCallableId::Arc => "arcweft.post.tint.arc",
                BuiltinFxCallableId::Spin => "arcweft.post.tint.spin",
                BuiltinFxCallableId::Pulse => "arcweft.post.tint.pulse",
                BuiltinFxCallableId::Motion => "arcweft.post.tint.motion",
                _ => unreachable!("matched tint builtin"),
            };
            (
                resource,
                vec![uniform_parameter(
                    context,
                    "amount",
                    BuiltinFxParameterId::Amount,
                )?],
            )
        }
        BuiltinFxCallableId::Typewriter | BuiltinFxCallableId::Shader => {
            unreachable!("post-process graph rows exclude this branch")
        }
    };
    shader_node_graph(
        context,
        FxPhase::PostProcess,
        FxStaticValue::Resource(
            FxResourceId::try_new(resource).expect("checked-in builtin resource ID is canonical"),
        ),
        std::mem::take(&mut uniforms),
    )
}

fn shader_graph(context: &GraphTemplateContext) -> Result<FxGraph, BuiltinFxBuildError> {
    let mut uniforms = Vec::new();
    if context.has_parameter(BuiltinFxParameterId::Amount) {
        uniforms.push(uniform_parameter(
            context,
            "amount",
            BuiltinFxParameterId::Amount,
        )?);
    }
    if context.has_parameter(BuiltinFxParameterId::Direction) {
        uniforms.push(uniform_vec2(
            context,
            "direction",
            BuiltinFxParameterId::Direction,
        )?);
    }
    if context.has_parameter(BuiltinFxParameterId::Color) {
        uniforms.push(uniform_parameter(
            context,
            "color",
            BuiltinFxParameterId::Color,
        )?);
    }
    shader_node_graph(
        context,
        context.specialization().row().phase(),
        context.static_value(BuiltinFxParameterId::Resource)?,
        uniforms,
    )
}

fn shader_node_graph(
    context: &GraphTemplateContext,
    phase: FxPhase,
    resource: FxStaticValue,
    uniforms: Vec<FxUniformField>,
) -> Result<FxGraph, BuiltinFxBuildError> {
    Ok(FxGraph::try_new(vec![FxNode::Shader {
        fx: context.definition().clone(),
        properties: vec![
            FxProperty::new(
                FxPropertyId::Target,
                FxStaticValue::Target(context.specialization().target()),
            ),
            FxProperty::new(FxPropertyId::Phase, FxStaticValue::Phase(phase)),
            FxProperty::new(FxPropertyId::Resource, resource),
            FxProperty::new(
                FxPropertyId::Uniforms,
                FxStaticValue::UniformRecord(FxUniformRecord::try_new(uniforms)?),
            ),
        ],
    }])?)
}

fn wave_transform(context: &GraphTemplateContext) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    let amplitude = runtime(context, BuiltinFxParameterId::Amplitude)?;
    let period = runtime(context, BuiltinFxParameterId::Period)?;
    let speed = runtime(context, BuiltinFxParameterId::Speed)?;
    let (direction_x, direction_y) = direction(context)?;
    let phase = mul(
        add(
            div(context_value(crate::fx::FxContextSlot::Ordinal), period),
            mul(context_value(crate::fx::FxContextSlot::Time), speed),
        ),
        f32_const(std::f32::consts::TAU),
    );
    let displacement = mul(amplitude, sin(phase));
    TransformFields {
        translate_x: mul(displacement.clone(), direction_x),
        translate_y: mul(displacement, direction_y),
        ..TransformFields::identity()
    }
    .finish(context.runtime_types())
}

fn noise_transform(
    context: &GraphTemplateContext,
    time_varying: bool,
) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    let amplitude = runtime(context, BuiltinFxParameterId::Amplitude)?;
    let seed = bitcast_u32_to_i32(runtime(context, BuiltinFxParameterId::Seed)?);
    let bucket = if time_varying {
        floor_to_i32(mul(
            context_value(crate::fx::FxContextSlot::Time),
            runtime(context, BuiltinFxParameterId::Speed)?,
        ))
    } else {
        i32(0)
    };
    let bucket = add(bucket, seed);
    let (direction_x, direction_y) = direction(context)?;
    TransformFields {
        translate_x: mul(
            mul(amplitude.clone(), signed_noise(bucket.clone())),
            direction_x,
        ),
        translate_y: mul(
            mul(amplitude, signed_noise(add(bucket, i32(0x51f1_5e5d)))),
            direction_y,
        ),
        ..TransformFields::identity()
    }
    .finish(context.runtime_types())
}

fn arc_transform(context: &GraphTemplateContext) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    let radius = runtime(context, BuiltinFxParameterId::Radius)?;
    let sample_angle = add(
        runtime(context, BuiltinFxParameterId::StartAngle)?,
        mul(
            runtime(context, BuiltinFxParameterId::StepAngle)?,
            context_value(crate::fx::FxContextSlot::Ordinal),
        ),
    );
    TransformFields {
        translate_x: mul(radius.clone(), cos(sample_angle.clone())),
        translate_y: mul(radius, sin(sample_angle.clone())),
        rotation: add(
            sample_angle,
            angle(Angle::try_degrees(90.0).expect("right angle is finite")),
        ),
        ..TransformFields::identity()
    }
    .finish(context.runtime_types())
}

fn spin_transform(context: &GraphTemplateContext) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    TransformFields {
        rotation: mul(
            runtime(context, BuiltinFxParameterId::RotationAmplitude)?,
            sin(temporal_phase(runtime(
                context,
                BuiltinFxParameterId::Speed,
            )?)),
        ),
        ..TransformFields::identity()
    }
    .finish(context.runtime_types())
}

fn pulse_transform(
    context: &GraphTemplateContext,
) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    let sample = add(
        mul(
            sin(temporal_phase(runtime(
                context,
                BuiltinFxParameterId::Speed,
            )?)),
            f32_const(0.5),
        ),
        f32_const(0.5),
    );
    let scale = add(
        f32(FiniteF32::ONE),
        mul(
            runtime(context, BuiltinFxParameterId::ScaleAmplitude)?,
            sample,
        ),
    );
    TransformFields {
        scale_x: scale.clone(),
        scale_y: scale,
        ..TransformFields::identity()
    }
    .finish(context.runtime_types())
}

fn motion_transform(
    context: &GraphTemplateContext,
) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    let speed = runtime(context, BuiltinFxParameterId::Speed)?;
    let amplitude = runtime(context, BuiltinFxParameterId::Amplitude)?;
    let rotation_amplitude = runtime(context, BuiltinFxParameterId::RotationAmplitude)?;
    let scale_amplitude = runtime(context, BuiltinFxParameterId::ScaleAmplitude)?;
    let sample_time = add(
        add(
            mul(context_value(crate::fx::FxContextSlot::Time), speed),
            mul(
                context_value(crate::fx::FxContextSlot::Ordinal),
                f32_const(0.037),
            ),
        ),
        mul(hash_noise(i32(0)), f32_const(0.11)),
    );
    let tau_time = mul(sample_time.clone(), f32_const(std::f32::consts::TAU));
    let primary = sin(tau_time.clone());
    let fields = match context
        .specialization()
        .motion_function()
        .expect("motion row specialization is normalized")
    {
        MotionFunction::ElasticBloom => {
            let snap = max(
                sin(add(
                    mul(tau_time, f32_const(3.0)),
                    mul(hash_noise(i32(1)), f32_const(0.25)),
                )),
                f32(FiniteF32::ZERO),
            );
            let snap = mul(snap.clone(), snap);
            let scale = add(f32(FiniteF32::ONE), mul(scale_amplitude, snap.clone()));
            TransformFields {
                translate_x: mul(mul(amplitude.clone(), f32_const(0.25)), primary.clone()),
                translate_y: mul(mul(amplitude, f32_const(-0.55)), snap.clone()),
                rotation: mul(
                    rotation_amplitude,
                    add(mul(primary, f32_const(0.35)), mul(snap, f32_const(0.65))),
                ),
                scale_x: scale.clone(),
                scale_y: scale,
                ..TransformFields::identity()
            }
        }
        MotionFunction::BreathOrbit => {
            let secondary = sin(add(
                mul(sample_time, f32_const(2.0 * std::f32::consts::TAU)),
                hash_noise(i32(0)),
            ));
            let orbit = add(tau_time, mul(secondary.clone(), f32_const(0.32)));
            let bloom = add(
                mul(
                    add(mul(primary.clone(), f32_const(0.5)), f32_const(0.5)),
                    f32_const(0.72),
                ),
                mul(
                    add(mul(secondary.clone(), f32_const(0.5)), f32_const(0.5)),
                    f32_const(0.28),
                ),
            );
            let scale = add(
                f32(FiniteF32::ONE),
                mul(scale_amplitude, max(bloom.clone(), f32(FiniteF32::ZERO))),
            );
            TransformFields {
                translate_x: mul(
                    mul(amplitude.clone(), cos(orbit.clone())),
                    add(f32_const(0.65), mul(bloom, f32_const(0.35))),
                ),
                translate_y: mul(
                    amplitude,
                    add(
                        mul(sin(orbit), f32_const(0.48)),
                        mul(secondary.clone(), f32_const(0.18)),
                    ),
                ),
                rotation: mul(
                    rotation_amplitude,
                    add(
                        mul(primary, f32_const(0.72)),
                        mul(secondary, f32_const(0.28)),
                    ),
                ),
                scale_x: scale.clone(),
                scale_y: scale,
                ..TransformFields::identity()
            }
        }
    };
    fields.finish(context.runtime_types())
}

fn sparkle_transform(
    context: &GraphTemplateContext,
) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    let amplitude = runtime(context, BuiltinFxParameterId::Amplitude)?;
    let speed = runtime(context, BuiltinFxParameterId::Speed)?;
    let phase = sparkle_phase(speed.clone());
    let shimmer = sparkle_shimmer(speed);
    let drift = cos(add(
        mul(phase, f32_const(0.73)),
        mul(hash_noise(i32(1)), f32_const(std::f32::consts::TAU)),
    ));
    let scale = add(f32(FiniteF32::ONE), mul(shimmer.clone(), f32_const(0.035)));
    TransformFields {
        translate_x: mul(mul(amplitude.clone(), f32_const(0.18)), drift),
        translate_y: mul(mul(amplitude, f32_const(-0.35)), shimmer.clone()),
        scale_x: scale.clone(),
        scale_y: scale,
        opacity: add(f32_const(0.82), mul(shimmer, f32_const(0.18))),
        ..TransformFields::identity()
    }
    .finish(context.runtime_types())
}

fn sparkle_phase(speed: ProgramExpr) -> ProgramExpr {
    mul(
        add(
            add(
                mul(context_value(crate::fx::FxContextSlot::Time), speed),
                hash_noise(i32(0)),
            ),
            mul(
                context_value(crate::fx::FxContextSlot::Ordinal),
                f32_const(0.071),
            ),
        ),
        f32_const(std::f32::consts::TAU),
    )
}

fn sparkle_shimmer(speed: ProgramExpr) -> ProgramExpr {
    add(
        mul(sin(sparkle_phase(speed)), f32_const(0.5)),
        f32_const(0.5),
    )
}

fn sparkle_color(context: &GraphTemplateContext) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    let shimmer = sparkle_shimmer(runtime(context, BuiltinFxParameterId::Speed)?);
    sampler(
        context.runtime_types(),
        FxRuntimeType::Color,
        make_color(
            f32(FiniteF32::ONE),
            div(
                add(f32_const(150.0), mul(shimmer.clone(), f32_const(80.0))),
                f32_const(255.0),
            ),
            div(
                add(f32_const(190.0), mul(shimmer, f32_const(65.0))),
                f32_const(255.0),
            ),
            f32(FiniteF32::ONE),
        ),
    )
}

fn typewriter_coverage(
    context: &GraphTemplateContext,
) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    let cps = runtime(context, BuiltinFxParameterId::CharactersPerSecond)?;
    let delay = seconds_value(runtime(context, BuiltinFxParameterId::Delay)?);
    let cursor = runtime(context, BuiltinFxParameterId::Cursor)?;
    let cursor_alpha = if context.has_parameter(BuiltinFxParameterId::CursorAlpha) {
        runtime(context, BuiltinFxParameterId::CursorAlpha)?
    } else {
        f32_const(0.35)
    };
    let progress = || {
        mul(
            max(
                sub(context_value(crate::fx::FxContextSlot::Time), delay.clone()),
                f32(FiniteF32::ZERO),
            ),
            cps.clone(),
        )
    };
    let visible = less_equal(
        add(
            context_value(crate::fx::FxContextSlot::Ordinal),
            f32(FiniteF32::ONE),
        ),
        progress(),
    );
    let cursor_coverage = select(
        less_equal(context_value(crate::fx::FxContextSlot::Ordinal), progress()),
        cursor_alpha,
        f32(FiniteF32::ZERO),
    );
    let hidden_coverage = select(cursor, cursor_coverage, f32(FiniteF32::ZERO));
    scalar_sampler(
        context,
        select(
            context_value(crate::fx::FxContextSlot::ReduceMotion),
            f32(FiniteF32::ONE),
            select(visible, f32(FiniteF32::ONE), hidden_coverage),
        ),
    )
}

fn runtime(
    context: &GraphTemplateContext,
    parameter_id: BuiltinFxParameterId,
) -> Result<ProgramExpr, BuiltinFxBuildError> {
    Ok(match context.runtime_argument(parameter_id)? {
        GraphRuntimeArgument::Parameter(reference) => parameter(reference),
        GraphRuntimeArgument::Constant(runtime_value) => value(runtime_value),
    })
}

fn direction(
    context: &GraphTemplateContext,
) -> Result<(ProgramExpr, ProgramExpr), BuiltinFxBuildError> {
    let direction = runtime(context, BuiltinFxParameterId::Direction)?;
    Ok((vec2_x(direction.clone()), vec2_y(direction)))
}

fn context_value(slot: crate::fx::FxContextSlot) -> ProgramExpr {
    context(slot)
}

fn temporal_phase(speed: ProgramExpr) -> ProgramExpr {
    mul(
        mul(context_value(crate::fx::FxContextSlot::Time), speed),
        f32_const(std::f32::consts::TAU),
    )
}

fn scalar_sampler(
    context: &GraphTemplateContext,
    expression: ProgramExpr,
) -> Result<FxSamplerProgram, BuiltinFxBuildError> {
    sampler(context.runtime_types(), FxRuntimeType::F32, expression)
}

fn uniform_parameter(
    context: &GraphTemplateContext,
    name: &str,
    parameter_id: BuiltinFxParameterId,
) -> Result<FxUniformField, BuiltinFxBuildError> {
    let value = match context.runtime_argument(parameter_id)? {
        GraphRuntimeArgument::Parameter(reference) => FxUniformValue::parameter(reference)?,
        GraphRuntimeArgument::Constant(value) => FxUniformValue::constant(value)?,
    };
    Ok(FxUniformField::try_new(name, value)?)
}

fn uniform_program(
    context: &GraphTemplateContext,
    name: &str,
    expression: ProgramExpr,
) -> Result<FxUniformField, BuiltinFxBuildError> {
    let sampler = scalar_sampler(context, expression)?;
    Ok(FxUniformField::try_new(
        name,
        FxUniformValue::Program(FxUniformProgram::try_new(sampler)?),
    )?)
}

fn uniform_vec2(
    context: &GraphTemplateContext,
    name: &str,
    parameter_id: BuiltinFxParameterId,
) -> Result<FxUniformField, BuiltinFxBuildError> {
    let direction = runtime(context, parameter_id)?;
    let sampler = sampler(
        context.runtime_types(),
        FxRuntimeType::Vec2,
        make_vec2(vec2_x(direction.clone()), vec2_y(direction)),
    )?;
    Ok(FxUniformField::try_new(
        name,
        FxUniformValue::Program(FxUniformProgram::try_new(sampler)?),
    )?)
}

fn uniform_seed(
    context: &GraphTemplateContext,
    name: &str,
    parameter_id: BuiltinFxParameterId,
) -> Result<FxUniformField, BuiltinFxBuildError> {
    let sampler = sampler(
        context.runtime_types(),
        FxRuntimeType::I32,
        bitcast_u32_to_i32(runtime(context, parameter_id)?),
    )?;
    Ok(FxUniformField::try_new(
        name,
        FxUniformValue::Program(FxUniformProgram::try_new(sampler)?),
    )?)
}
