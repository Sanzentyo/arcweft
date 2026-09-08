//! Closed render-resource programs and executable paint-pass output.
//!
//! Resource IDs are resolved here, before a native, Web, or headless backend
//! sees a glyph or GPU object. Backends consume only the finite closed output
//! types below and therefore never own string registries or effect callbacks.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    FiniteF32, FxColor, FxDiagnosticCode, FxPhase, FxRendererInterface, FxResolvedValue,
    FxResourceId, FxRuntimeValue, FxShaderUniform, FxVec2, Length, Opacity,
    ResolvedFilterOperation, ResolvedMaskOperation, ResolvedShaderUniformOperation,
};

/// One extra glyph raster pass emitted before the main glyph pass.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedFxGlyphPass {
    pub offset_x: Length,
    pub offset_y: Length,
    pub color: FxColor,
}

/// Paint-only glyph mask resolved before raster submission.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedFxMask {
    pub coverage: Opacity,
    pub invert: bool,
}

/// One finite shared offscreen filter pass.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedFxOffscreenPass {
    pub blur_radius: Length,
    pub brightness: FiniteF32,
    pub contrast: FiniteF32,
    pub saturation: FiniteF32,
}

/// Closed displacement program kind used by the shared GPU compositor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedFxDisplacementKind {
    Wave,
    Shake,
    Jitter,
}

/// One shared post-process pass with all runtime values already sampled.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedFxPostProcess {
    Tint {
        color: FxColor,
        amount: Opacity,
    },
    Displacement {
        displacement: ResolvedFxDisplacementKind,
        amplitude: Length,
        period: Length,
        phase_radians: FiniteF32,
        direction: FxVec2,
        seed: u64,
    },
    Sparkle {
        amount: Opacity,
        phase_radians: FiniteF32,
        seed: u64,
    },
}

/// Backend-neutral program attached to one canonical render resource ID.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FxRenderProgram {
    Glow {
        color: FxColor,
    },
    Tint {
        color: FxColor,
        amount: Opacity,
    },
    Displacement {
        displacement: ResolvedFxDisplacementKind,
    },
    Sparkle,
}

/// Stable typed program table shared by native, Web, headless, and capture.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FxRenderResourceTable(BTreeMap<FxResourceId, FxRenderProgram>);

/// Fully resolved output of one shader-resource invocation.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedFxResourceOutput {
    pub glyph_passes: Vec<ResolvedFxGlyphPass>,
    pub post_processes: Vec<ResolvedFxPostProcess>,
}

/// Render-resource insertion or invocation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FxRenderResourceError {
    #[error("duplicate Fx render resource `{resource}`")]
    DuplicateResource { resource: String },
    #[error("Fx renderer interface {actual:?} is not a shader-resource invocation")]
    WrongInterface { actual: FxRendererInterface },
    #[error("Fx shader invocation is missing resource property")]
    MissingResource,
    #[error("Fx shader invocation contains duplicate property `{property}`")]
    DuplicateProperty { property: String },
    #[error("Fx shader property `{property}` has the wrong closed value type")]
    InvalidProperty { property: String },
    #[error("Fx render resource `{resource}` is not present in the shared table")]
    UnknownResource { resource: String },
    #[error("Fx render resource `{resource}` does not support phase {phase:?}")]
    UnsupportedPhase { resource: String, phase: FxPhase },
    #[error("Fx render value `{property}` must be in the closed interval [0, 1]")]
    InvalidOpacity { property: String },
    #[error("Fx render value `{property}` must be finite and non-negative")]
    InvalidNonNegative { property: String },
    #[error("Fx render direction must be a non-zero finite vector")]
    InvalidDirection,
}

impl FxRenderResourceError {
    /// Stable diagnostic category for this closed render-resource failure.
    pub const fn diagnostic_code(&self) -> FxDiagnosticCode {
        match self {
            Self::DuplicateResource { .. } => FxDiagnosticCode::DuplicateProvider,
            Self::UnknownResource { .. } => FxDiagnosticCode::MissingProvider,
            Self::WrongInterface { .. } | Self::UnsupportedPhase { .. } => {
                FxDiagnosticCode::UnsupportedCapability
            }
            Self::InvalidOpacity { .. } => FxDiagnosticCode::InvalidOpacity,
            Self::MissingResource
            | Self::DuplicateProperty { .. }
            | Self::InvalidProperty { .. }
            | Self::InvalidNonNegative { .. }
            | Self::InvalidDirection => FxDiagnosticCode::ProgramValidation,
        }
    }
}

impl ResolvedFxGlyphPass {
    pub const fn new(offset_x: Length, offset_y: Length, color: FxColor) -> Self {
        Self {
            offset_x,
            offset_y,
            color,
        }
    }
}

impl ResolvedFxMask {
    /// Resolves the typed values of one `Mask` interface operation.
    pub fn from_operation(
        operation: &ResolvedMaskOperation,
    ) -> Result<Self, FxRenderResourceError> {
        if let Some(resource) = &operation.resource {
            return Err(FxRenderResourceError::UnknownResource {
                resource: resource.as_str().to_owned(),
            });
        }
        let coverage = opacity(operation.coverage.unwrap_or(FiniteF32::ONE), "coverage")?;
        Ok(Self {
            coverage,
            invert: operation.invert.unwrap_or(false),
        })
    }

    /// Effective constant alpha coverage after optional inversion.
    ///
    /// # Panics
    ///
    /// Panics only if subtracting a validated opacity from one stops producing
    /// a finite value in `[0, 1]`, which would violate [`Opacity`]'s invariant.
    pub fn effective_coverage(self) -> Opacity {
        if !self.invert {
            return self.coverage;
        }
        let value = FiniteF32::try_new(1.0 - self.coverage.value().get())
            .expect("inverting a validated opacity remains finite");
        Opacity::try_new(value).expect("inverting a validated opacity remains in range")
    }
}

impl ResolvedFxOffscreenPass {
    /// Resolves the typed values of one `Filter` interface operation.
    pub fn from_operation(
        operation: &ResolvedFilterOperation,
    ) -> Result<Self, FxRenderResourceError> {
        let blur_radius = operation.blur_radius.unwrap_or_default();
        if blur_radius.pixels() < 0.0 {
            return Err(FxRenderResourceError::InvalidNonNegative {
                property: "blur_radius".to_owned(),
            });
        }
        let brightness =
            non_negative(operation.brightness.unwrap_or(FiniteF32::ONE), "brightness")?;
        let contrast = non_negative(operation.contrast.unwrap_or(FiniteF32::ONE), "contrast")?;
        let saturation =
            non_negative(operation.saturation.unwrap_or(FiniteF32::ONE), "saturation")?;
        Ok(Self {
            blur_radius,
            brightness,
            contrast,
            saturation,
        })
    }

    pub fn is_identity(self) -> bool {
        self.blur_radius.pixels() == 0.0
            && self.brightness == FiniteF32::ONE
            && self.contrast == FiniteF32::ONE
            && self.saturation == FiniteF32::ONE
    }
}

impl FxRenderResourceTable {
    /// Arcweft-owned programs available identically in every shared player.
    ///
    /// # Panics
    ///
    /// Panics if a checked-in builtin ID is invalid or duplicated. Both are
    /// static programming errors covered by this module's tests.
    pub fn arcweft_builtins() -> Self {
        let mut table = Self::default();
        for (id, program) in [
            (
                "soft_glow",
                FxRenderProgram::Glow {
                    color: rgb(155, 205, 255),
                },
            ),
            (
                "warm_glow",
                FxRenderProgram::Glow {
                    color: rgb(255, 178, 112),
                },
            ),
            (
                "shader.source_glow",
                FxRenderProgram::Glow {
                    color: rgb(96, 64, 255),
                },
            ),
            (
                "screen_tint",
                FxRenderProgram::Tint {
                    color: rgb(120, 160, 255),
                    amount: opacity_from_unit(0.25),
                },
            ),
            (
                "arcweft.post.wave",
                FxRenderProgram::Displacement {
                    displacement: ResolvedFxDisplacementKind::Wave,
                },
            ),
            (
                "arcweft.post.shake",
                FxRenderProgram::Displacement {
                    displacement: ResolvedFxDisplacementKind::Shake,
                },
            ),
            (
                "arcweft.post.jitter",
                FxRenderProgram::Displacement {
                    displacement: ResolvedFxDisplacementKind::Jitter,
                },
            ),
            ("arcweft.post.sparkle", FxRenderProgram::Sparkle),
            (
                "arcweft.post.tint.arc",
                FxRenderProgram::Tint {
                    color: rgb(210, 190, 255),
                    amount: opacity_from_unit(0.18),
                },
            ),
            (
                "arcweft.post.tint.spin",
                FxRenderProgram::Tint {
                    color: rgb(170, 220, 255),
                    amount: opacity_from_unit(0.18),
                },
            ),
            (
                "arcweft.post.tint.pulse",
                FxRenderProgram::Tint {
                    color: rgb(255, 220, 150),
                    amount: opacity_from_unit(0.18),
                },
            ),
            (
                "arcweft.post.tint.motion",
                FxRenderProgram::Tint {
                    color: rgb(255, 170, 220),
                    amount: opacity_from_unit(0.18),
                },
            ),
        ] {
            table
                .insert(
                    FxResourceId::try_new(id).expect("builtin resource ID"),
                    program,
                )
                .expect("builtin render resource IDs are unique");
        }
        table
    }

    pub fn insert(
        &mut self,
        resource: FxResourceId,
        program: FxRenderProgram,
    ) -> Result<(), FxRenderResourceError> {
        if self.0.contains_key(&resource) {
            return Err(FxRenderResourceError::DuplicateResource {
                resource: resource.as_str().to_owned(),
            });
        }
        self.0.insert(resource, program);
        Ok(())
    }

    pub fn get(&self, resource: &FxResourceId) -> Option<FxRenderProgram> {
        self.0.get(resource).copied()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&FxResourceId, &FxRenderProgram)> {
        self.0.iter()
    }

    /// Resolves a shader operation to finite backend-executable passes.
    pub fn resolve_shader(
        &self,
        operation: &ResolvedShaderUniformOperation,
    ) -> Result<ResolvedFxResourceOutput, FxRenderResourceError> {
        let invocation = ShaderInvocation::from_operation(operation)?;
        let program = self.get(&invocation.resource).ok_or_else(|| {
            FxRenderResourceError::UnknownResource {
                resource: invocation.resource.as_str().to_owned(),
            }
        })?;
        program.resolve(&invocation, operation.phase)
    }
}

struct ShaderInvocation {
    resource: FxResourceId,
    uniforms: Vec<FxShaderUniform>,
}

impl ShaderInvocation {
    fn from_operation(
        operation: &ResolvedShaderUniformOperation,
    ) -> Result<Self, FxRenderResourceError> {
        Ok(Self {
            resource: operation
                .resource
                .clone()
                .ok_or(FxRenderResourceError::MissingResource)?,
            uniforms: operation.uniforms.clone(),
        })
    }

    fn uniform(&self, name: &str) -> Result<Option<&FxResolvedValue>, FxRenderResourceError> {
        let mut values = self
            .uniforms
            .iter()
            .filter(|value| value.name.as_str() == name)
            .map(|value| &value.value);
        let value = values.next();
        if values.next().is_some() {
            return Err(FxRenderResourceError::DuplicateProperty {
                property: name.to_owned(),
            });
        }
        Ok(value)
    }

    fn require_uniform_schema(&self, allowed: &[&str]) -> Result<(), FxRenderResourceError> {
        if let Some(value) = self
            .uniforms
            .iter()
            .find(|value| !allowed.contains(&value.name.as_str()))
        {
            return Err(FxRenderResourceError::InvalidProperty {
                property: value.name.as_str().to_owned(),
            });
        }
        Ok(())
    }
}

impl FxRenderProgram {
    fn resolve(
        self,
        invocation: &ShaderInvocation,
        phase: FxPhase,
    ) -> Result<ResolvedFxResourceOutput, FxRenderResourceError> {
        match self {
            Self::Glow { color } => resolve_glow(invocation, phase, color),
            Self::Tint { color, amount } => resolve_tint(invocation, phase, color, amount),
            Self::Displacement { displacement } => {
                resolve_displacement(invocation, phase, displacement)
            }
            Self::Sparkle => resolve_sparkle(invocation, phase),
        }
    }
}

fn resolve_displacement(
    invocation: &ShaderInvocation,
    phase: FxPhase,
    displacement: ResolvedFxDisplacementKind,
) -> Result<ResolvedFxResourceOutput, FxRenderResourceError> {
    invocation.require_uniform_schema(&["amplitude", "period", "phase", "direction", "seed"])?;
    if phase != FxPhase::PostProcess {
        return Err(FxRenderResourceError::UnsupportedPhase {
            resource: invocation.resource.as_str().to_owned(),
            phase,
        });
    }
    let amplitude = shader_length(invocation, "amplitude")?.unwrap_or(Length::ZERO);
    if amplitude.pixels() < 0.0 {
        return Err(FxRenderResourceError::InvalidNonNegative {
            property: "amplitude".to_owned(),
        });
    }
    let period = shader_length(invocation, "period")?
        .unwrap_or(Length::try_pixels(64.0).expect("builtin period is finite"));
    if period.pixels() <= 0.0 {
        return Err(FxRenderResourceError::InvalidNonNegative {
            property: "period".to_owned(),
        });
    }
    let phase_radians = shader_f32(invocation, "phase")?.unwrap_or(FiniteF32::ZERO);
    let direction = normalized(shader_vec2(invocation, "direction")?.unwrap_or(FxVec2 {
        x: FiniteF32::ONE,
        y: FiniteF32::ZERO,
    }))?;
    let seed = shader_i32(invocation, "seed")?.unwrap_or(0);
    Ok(ResolvedFxResourceOutput {
        post_processes: vec![ResolvedFxPostProcess::Displacement {
            displacement,
            amplitude,
            period,
            phase_radians,
            direction,
            seed: u64::from_ne_bytes(i64::from(seed).to_ne_bytes()),
        }],
        ..ResolvedFxResourceOutput::default()
    })
}

fn resolve_sparkle(
    invocation: &ShaderInvocation,
    phase: FxPhase,
) -> Result<ResolvedFxResourceOutput, FxRenderResourceError> {
    invocation.require_uniform_schema(&["amount", "phase", "seed"])?;
    if phase != FxPhase::PostProcess {
        return Err(FxRenderResourceError::UnsupportedPhase {
            resource: invocation.resource.as_str().to_owned(),
            phase,
        });
    }
    let amount = opacity(
        shader_f32(invocation, "amount")?.unwrap_or(FiniteF32::ZERO),
        "amount",
    )?;
    let phase_radians = shader_f32(invocation, "phase")?.unwrap_or(FiniteF32::ZERO);
    let seed = shader_i32(invocation, "seed")?.unwrap_or(0);
    Ok(ResolvedFxResourceOutput {
        post_processes: vec![ResolvedFxPostProcess::Sparkle {
            amount,
            phase_radians,
            seed: u64::from_ne_bytes(i64::from(seed).to_ne_bytes()),
        }],
        ..ResolvedFxResourceOutput::default()
    })
}

fn resolve_glow(
    invocation: &ShaderInvocation,
    phase: FxPhase,
    default_color: FxColor,
) -> Result<ResolvedFxResourceOutput, FxRenderResourceError> {
    invocation.require_uniform_schema(&["amount", "dir", "color"])?;
    let amount = shader_f32(invocation, "amount")?.unwrap_or(FiniteF32::ONE);
    non_negative(amount, "amount")?;
    let direction = shader_vec2(invocation, "dir")?.unwrap_or(FxVec2 {
        x: FiniteF32::ZERO,
        y: FiniteF32::ONE,
    });
    let direction = normalized(direction)?;
    let color = shader_color(invocation, "color")?.unwrap_or(default_color);
    if phase == FxPhase::PostProcess {
        return Ok(ResolvedFxResourceOutput {
            post_processes: vec![ResolvedFxPostProcess::Tint {
                color,
                amount: opacity(amount, "amount")?,
            }],
            ..ResolvedFxResourceOutput::default()
        });
    }
    if phase == FxPhase::GlyphColor {
        return Ok(ResolvedFxResourceOutput {
            glyph_passes: vec![ResolvedFxGlyphPass::new(
                Length::default(),
                Length::default(),
                with_alpha(color, opacity(amount, "amount")?),
            )],
            ..ResolvedFxResourceOutput::default()
        });
    }
    if phase != FxPhase::OffscreenPass {
        return Err(FxRenderResourceError::UnsupportedPhase {
            resource: invocation.resource.as_str().to_owned(),
            phase,
        });
    }

    let radius = (amount.get() * 6.0).clamp(1.0, 12.0);
    let side = FxVec2 {
        x: FiniteF32::try_new(-direction.y.get()).expect("finite normalized direction"),
        y: direction.x,
    };
    let passes = [
        (direction, radius, 72.0),
        (direction, radius * -0.5, 44.0),
        (side, radius * 0.5, 32.0),
        (side, radius * -0.5, 32.0),
    ]
    .into_iter()
    .map(|(axis, distance, alpha_scale)| {
        let alpha = (amount.get() * alpha_scale).round().clamp(8.0, 96.0) / 255.0;
        Ok(ResolvedFxGlyphPass::new(
            Length::try_pixels(axis.x.get() * distance).map_err(|_| {
                FxRenderResourceError::InvalidNonNegative {
                    property: "offset".to_owned(),
                }
            })?,
            Length::try_pixels(axis.y.get() * distance).map_err(|_| {
                FxRenderResourceError::InvalidNonNegative {
                    property: "offset".to_owned(),
                }
            })?,
            with_alpha(color, opacity_from_unit(alpha)),
        ))
    })
    .collect::<Result<Vec<_>, FxRenderResourceError>>()?;
    Ok(ResolvedFxResourceOutput {
        glyph_passes: passes,
        ..ResolvedFxResourceOutput::default()
    })
}

fn resolve_tint(
    invocation: &ShaderInvocation,
    phase: FxPhase,
    default_color: FxColor,
    default_amount: Opacity,
) -> Result<ResolvedFxResourceOutput, FxRenderResourceError> {
    invocation.require_uniform_schema(&["amount", "color"])?;
    if phase != FxPhase::PostProcess {
        return Err(FxRenderResourceError::UnsupportedPhase {
            resource: invocation.resource.as_str().to_owned(),
            phase,
        });
    }
    let color = shader_color(invocation, "color")?.unwrap_or(default_color);
    let amount = shader_f32(invocation, "amount")?
        .map(|value| opacity(value, "amount"))
        .transpose()?
        .unwrap_or(default_amount);
    Ok(ResolvedFxResourceOutput {
        post_processes: vec![ResolvedFxPostProcess::Tint { color, amount }],
        ..ResolvedFxResourceOutput::default()
    })
}

fn shader_f32(
    invocation: &ShaderInvocation,
    name: &str,
) -> Result<Option<FiniteF32>, FxRenderResourceError> {
    invocation.uniform(name)?.map_or(Ok(None), |value| {
        let FxResolvedValue::Runtime(FxRuntimeValue::F32(value)) = value else {
            return Err(FxRenderResourceError::InvalidProperty {
                property: name.to_owned(),
            });
        };
        Ok(Some(*value))
    })
}

fn shader_i32(
    invocation: &ShaderInvocation,
    name: &str,
) -> Result<Option<i32>, FxRenderResourceError> {
    invocation.uniform(name)?.map_or(Ok(None), |value| {
        let FxResolvedValue::Runtime(FxRuntimeValue::I32(value)) = value else {
            return Err(FxRenderResourceError::InvalidProperty {
                property: name.to_owned(),
            });
        };
        Ok(Some(*value))
    })
}

fn shader_length(
    invocation: &ShaderInvocation,
    name: &str,
) -> Result<Option<Length>, FxRenderResourceError> {
    invocation.uniform(name)?.map_or(Ok(None), |value| {
        let FxResolvedValue::Runtime(FxRuntimeValue::Length(value)) = value else {
            return Err(FxRenderResourceError::InvalidProperty {
                property: name.to_owned(),
            });
        };
        Ok(Some(*value))
    })
}

fn shader_vec2(
    invocation: &ShaderInvocation,
    name: &str,
) -> Result<Option<FxVec2>, FxRenderResourceError> {
    invocation.uniform(name)?.map_or(Ok(None), |value| {
        let FxResolvedValue::Runtime(FxRuntimeValue::Vec2(value)) = value else {
            return Err(FxRenderResourceError::InvalidProperty {
                property: name.to_owned(),
            });
        };
        Ok(Some(*value))
    })
}

fn shader_color(
    invocation: &ShaderInvocation,
    name: &str,
) -> Result<Option<FxColor>, FxRenderResourceError> {
    invocation.uniform(name)?.map_or(Ok(None), |value| {
        let FxResolvedValue::Runtime(FxRuntimeValue::Color(value)) = value else {
            return Err(FxRenderResourceError::InvalidProperty {
                property: name.to_owned(),
            });
        };
        Ok(Some(*value))
    })
}

fn normalized(value: FxVec2) -> Result<FxVec2, FxRenderResourceError> {
    let length = value.x.get().hypot(value.y.get());
    if !length.is_finite() || length <= f32::EPSILON {
        return Err(FxRenderResourceError::InvalidDirection);
    }
    Ok(FxVec2 {
        x: FiniteF32::try_new(value.x.get() / length)
            .map_err(|_| FxRenderResourceError::InvalidDirection)?,
        y: FiniteF32::try_new(value.y.get() / length)
            .map_err(|_| FxRenderResourceError::InvalidDirection)?,
    })
}

fn non_negative(value: FiniteF32, property: &str) -> Result<FiniteF32, FxRenderResourceError> {
    if value.get() < 0.0 {
        Err(FxRenderResourceError::InvalidNonNegative {
            property: property.to_owned(),
        })
    } else {
        Ok(value)
    }
}

fn opacity(value: FiniteF32, property: &str) -> Result<Opacity, FxRenderResourceError> {
    Opacity::try_new(value).map_err(|_| FxRenderResourceError::InvalidOpacity {
        property: property.to_owned(),
    })
}

fn rgb(red: u8, green: u8, blue: u8) -> FxColor {
    FxColor::new(
        opacity_from_unit(f32::from(red) / 255.0),
        opacity_from_unit(f32::from(green) / 255.0),
        opacity_from_unit(f32::from(blue) / 255.0),
        Opacity::OPAQUE,
    )
}

fn with_alpha(color: FxColor, alpha: Opacity) -> FxColor {
    FxColor::new(color.red(), color.green(), color.blue(), alpha)
}

fn opacity_from_unit(value: f32) -> Opacity {
    Opacity::try_new(FiniteF32::try_new(value).expect("constant opacity is finite"))
        .expect("constant opacity is in range")
}

#[cfg(test)]
mod tests {
    use super::{
        FxRenderResourceError, FxRenderResourceTable, ResolvedFxMask, ResolvedFxOffscreenPass,
    };
    use crate::fx::{
        FiniteF32, FxDiagnosticCode, FxPhase, FxResourceId, FxRuntimeValue, FxShaderUniform,
        FxTarget, Length, ResolvedFilterOperation, ResolvedMaskOperation,
        ResolvedShaderUniformOperation,
    };

    #[test]
    fn builtin_glow_resolves_to_closed_glyph_passes() {
        let operation = ResolvedShaderUniformOperation {
            phase: FxPhase::OffscreenPass,
            target: FxTarget::Content,
            resource: Some(FxResourceId::try_new("soft_glow").expect("resource")),
            stage: None,
            uniforms: vec![FxShaderUniform::runtime(
                crate::fx::FxUniformName::try_new("amount").expect("checked-in name"),
                FxRuntimeValue::F32(FiniteF32::try_new(0.5).expect("finite")),
            )],
        };

        let output = FxRenderResourceTable::arcweft_builtins()
            .resolve_shader(&operation)
            .expect("shared glow program resolves");

        assert_eq!(output.glyph_passes.len(), 4);
        assert!(output.post_processes.is_empty());
        assert!((output.glyph_passes[0].offset_y.pixels() - 3.0).abs() <= f32::EPSILON);
    }

    #[test]
    fn unknown_shader_resource_is_not_a_noop() {
        let operation = ResolvedShaderUniformOperation {
            phase: FxPhase::GlyphColor,
            target: FxTarget::Glyph,
            resource: Some(FxResourceId::try_new("missing.shader").expect("resource")),
            stage: None,
            uniforms: Vec::new(),
        };

        assert_eq!(
            FxRenderResourceTable::arcweft_builtins().resolve_shader(&operation),
            Err(FxRenderResourceError::UnknownResource {
                resource: "missing.shader".to_owned(),
            })
        );
        assert_eq!(
            FxRenderResourceError::UnknownResource {
                resource: "missing.shader".to_owned(),
            }
            .diagnostic_code(),
            FxDiagnosticCode::MissingProvider
        );
    }

    #[test]
    fn typed_mask_and_filter_validate_closed_values() {
        let mask = ResolvedFxMask::from_operation(&ResolvedMaskOperation {
            phase: FxPhase::GlyphMask,
            target: FxTarget::Glyph,
            resource: None,
            coverage: Some(FiniteF32::try_new(0.25).expect("finite")),
            invert: None,
        })
        .expect("mask");
        assert!((mask.effective_coverage().value().get() - 0.25).abs() <= f32::EPSILON);

        let filter = ResolvedFxOffscreenPass::from_operation(&ResolvedFilterOperation {
            phase: FxPhase::OffscreenPass,
            target: FxTarget::Content,
            blur_radius: Some(Length::try_pixels(4.0).expect("length")),
            brightness: None,
            contrast: None,
            saturation: None,
        })
        .expect("filter");
        assert!((filter.blur_radius.pixels() - 4.0).abs() <= f32::EPSILON);
        assert!(!filter.is_identity());
    }
}
