//! Source-bound runtime literal admission, including nominal scalar values.

use super::*;

pub(super) struct RuntimeLiteralContext<'a> {
    world: &'a RegisteredSemanticWorld,
    analysis: &'a FinalSemanticAnalysis,
}

impl<'a> RuntimeLiteralContext<'a> {
    pub(super) const fn new(
        world: &'a RegisteredSemanticWorld,
        analysis: &'a FinalSemanticAnalysis,
    ) -> Self {
        Self { world, analysis }
    }

    pub(super) fn font_family(
        &self,
        family: &RichTextFontFamily,
        ty: &TypeKind,
    ) -> Result<RuntimeValue, String> {
        if !matches!(ty, TypeKind::CompileTimeScalar(scalar)
            if scalar.kind() == arcweft_lang_sema::types::CompileTimeScalarKind::FontFamily)
        {
            return Err("font family has a different accepted scalar type".to_owned());
        }
        let graph = self
            .analysis
            .project_runtime_nominal_graph(self.world, ty, Default::default())
            .map_err(|error| error.to_string())?;
        let identity = RuntimeSemanticTypeId::from(
            ty.semantic_identity_digest()
                .map_err(|error| error.to_string())?,
        );
        let payload = match family {
            RichTextFontFamily::Named { name } => {
                Some(RuntimeValue::Tuple(vec![RuntimeValue::String(
                    name.clone(),
                )]))
            }
            _ => None,
        };
        graph
            .try_variant_value(
                identity,
                family.variant_ordinal(),
                payload,
                arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
            )
            .map_err(|error| error.to_string())
    }

    pub(super) fn literal(
        &self,
        literal: &HirLiteral,
        ty: &TypeKind,
    ) -> Result<RuntimeValue, String> {
        if matches!(ty, TypeKind::CompileTimeScalar(scalar)
        if scalar.kind() == arcweft_lang_sema::types::CompileTimeScalarKind::FontFamily)
        {
            let value = arcweft_lang_sema::checked_compile_time::reduce_scalar_literal(
                literal,
                &arcweft_lang_sema::checked_compile_time::CheckedCompileTimeScalarKind::FontFamily,
            )
            .map_err(|error| error.to_string())?;
            let CheckedCompileTimeScalar::FontFamily(family) = value else {
                return Err("font-family literal has a different scalar value".to_owned());
            };
            return self.font_family(&family, ty);
        }
        if let TypeKind::CompileTimeScalar(scalar) = ty
            && scalar.kind().runtime_quantity_kind().is_some()
        {
            use arcweft_lang_sema::checked_compile_time::{
                CheckedCompileTimeScalarKind, reduce_scalar_literal,
            };
            use arcweft_lang_sema::types::CompileTimeScalarKind;
            let kind = match scalar.kind() {
                CompileTimeScalarKind::Milli => CheckedCompileTimeScalarKind::Milli,
                CompileTimeScalarKind::Ratio => CheckedCompileTimeScalarKind::Ratio,
                CompileTimeScalarKind::Length => CheckedCompileTimeScalarKind::Length,
                CompileTimeScalarKind::Angle => CheckedCompileTimeScalarKind::Angle,
                _ => return Err("scalar has no quantity representation".to_owned()),
            };
            let value = reduce_scalar_literal(literal, &kind)
                .map_err(|error| error.to_string())?
                .runtime_quantity()
                .ok_or_else(|| "literal reducer returned a different quantity kind".to_owned())?;
            return value
                .into_runtime_value(RuntimeSemanticTypeId::from(
                    ty.semantic_identity_digest()
                        .map_err(|error| error.to_string())?,
                ))
                .map_err(|error| error.to_string());
        }
        match literal {
            HirLiteral::String(HirStringLiteral::Value(value)) => {
                Ok(RuntimeValue::String(value.to_string()))
            }
            HirLiteral::Character(HirCharacterLiteral::Value(value)) => {
                Ok(RuntimeValue::Char(*value))
            }
            HirLiteral::Integer(HirIntegerLiteral::Value { magnitude, .. }) => {
                runtime_integer_magnitude(magnitude, ty)
            }
            HirLiteral::Float(HirFloatLiteral::Value { decimal, .. }) => {
                runtime_decimal(decimal, ty)
            }
            HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { decimal, .. }) => {
                runtime_decimal(decimal, ty)
            }
            HirLiteral::Boolean(value) => Ok(RuntimeValue::Bool(*value)),
            HirLiteral::Duration(HirDurationLiteral::Value(value)) => {
                let nanos = big_uint_to_u128(value.semantic_value().nanoseconds())
                    .and_then(|value| u64::try_from(value).ok())
                    .ok_or_else(|| {
                        "Duration literal exceeds the runtime u64 nanosecond domain".to_owned()
                    })?;
                Ok(RuntimeValue::Duration(LogicalDuration::from_nanos(nanos)))
            }
            HirLiteral::String(HirStringLiteral::Invalid(issue)) => Err(issue.to_string()),
            HirLiteral::Character(HirCharacterLiteral::Invalid(issue)) => Err(issue.to_string()),
            HirLiteral::Integer(HirIntegerLiteral::Invalid(issue)) => Err(issue.to_string()),
            HirLiteral::Float(HirFloatLiteral::Invalid(issue)) => Err(issue.to_string()),
            HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(issue)) => Err(issue.to_string()),
            HirLiteral::Duration(HirDurationLiteral::Invalid(issue)) => Err(issue.to_string()),
        }
    }
}
