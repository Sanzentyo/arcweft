//! Literal, path, and short-variant expression typing.

use super::super::helpers::{builtin_path_type, numeric_literal_suffix_type};
use super::support::{
    has_multiple_numeric_choice_alternatives, inline_failure_builtin_variant_type,
    is_unit_number_type, std_float_constant_type, unique_numeric_choice_alternative,
};
use super::{
    BorrowLocalState, CallableDeclarationId, TypeCheckError, TypeChecker, TypeExpressionId,
    TypeKind, TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use arcweft_lang_syntax::expr::{DottedPath, IntLiteral, Literal};
use arcweft_lang_syntax::literal::FloatSuffix;

impl TypeChecker<'_> {
    pub(super) fn check_project_callable_path_expr(
        &mut self,
        path: &str,
        declaration: &CallableDeclarationId,
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        let Some(signature) = self.project_function_signatures.get(declaration).cloned() else {
            self.errors.push(TypeCheckError::new(format!(
                "accepted project callable `{}` has no semantic signature",
                declaration.qualified_name()
            )));
            return None;
        };
        let callable = crate::effect_model::CallableId::project_function(declaration);
        let effects = self
            .effect_collector
            .inferred_effect_row(&callable)
            .unwrap_or_else(crate::effect_row::EffectRow::unknown);
        let Some(ty) = signature.function_value_type_with_effects(effects) else {
            self.errors.push(TypeCheckError::new(format!(
                "accepted project callable `{}` has no function value type",
                declaration.qualified_name()
            )));
            return None;
        };
        self.last_checked_closure_effect_callable = Some(callable);
        self.record_typed_lowering_evidence(TypedLoweringEvidence::new(
            expression_id,
            TypedLoweringEvidenceKind::FunctionValueReference {
                callee: path.to_owned(),
                ty: ty.clone(),
            },
        ));
        Some(ty)
    }

    pub(super) fn check_literal_expr(
        &mut self,
        literal: &Literal,
        expected: Option<&TypeKind>,
        expression_id: TypeExpressionId,
    ) -> TypeKind {
        match literal {
            Literal::String(_) => TypeKind::String,
            Literal::Char { .. } => TypeKind::Char,
            Literal::Bool(_) => TypeKind::Bool,
            Literal::Duration { .. } => TypeKind::Duration,
            Literal::Int(literal) => {
                let ty = if let Some(suffix) = literal.suffix() {
                    TypeKind::from(suffix)
                } else if let Some(expected) = expected.filter(|ty| ty.is_integer()) {
                    expected.clone()
                } else if let Some(expected) = expected
                    && let Some(ty) =
                        unique_numeric_choice_alternative(expected, TypeKind::is_integer)
                {
                    ty
                } else if expected.is_some_and(|expected| {
                    has_multiple_numeric_choice_alternatives(expected, TypeKind::is_integer)
                }) {
                    self.errors.push(TypeCheckError::new(
                        "unsuffixed integer literal requires an expected integer type".to_owned(),
                    ));
                    TypeKind::Named("_".to_owned())
                } else {
                    self.record_numeric_fallback(
                        expression_id,
                        super::super::NumericFallbackKind::IntegerLiteral,
                        "integer",
                        TypeKind::I32,
                    );
                    TypeKind::I32
                };
                self.validate_integer_literal(literal, &ty);
                ty
            }
            Literal::Float { raw, suffix } => {
                let ty = if let Some(suffix) = suffix {
                    let Some(ty) = numeric_literal_suffix_type(Some(suffix.as_str())) else {
                        self.errors.push(TypeCheckError::new(format!(
                            "unknown float literal suffix `{suffix}`"
                        )));
                        return TypeKind::Named("_".to_owned());
                    };
                    if ty.is_float() || is_unit_number_type(&ty) {
                        ty
                    } else {
                        self.errors.push(TypeCheckError::new(format!(
                            "float literal suffix must be a float type, found {ty:?}"
                        )));
                        TypeKind::Named("_".to_owned())
                    }
                } else if let Some(expected) = expected.filter(|ty| ty.is_float()) {
                    expected.clone()
                } else if let Some(expected) = expected
                    && let Some(ty) =
                        unique_numeric_choice_alternative(expected, TypeKind::is_float)
                {
                    ty
                } else if expected.is_some_and(|expected| {
                    has_multiple_numeric_choice_alternatives(expected, TypeKind::is_float)
                }) {
                    self.errors.push(TypeCheckError::new(
                        "unsuffixed float literal requires an expected float type".to_owned(),
                    ));
                    TypeKind::Named("_".to_owned())
                } else {
                    self.record_numeric_fallback(
                        expression_id,
                        super::super::NumericFallbackKind::FloatLiteral,
                        "float",
                        TypeKind::F64,
                    );
                    TypeKind::F64
                };
                self.validate_float_literal(raw, suffix.as_ref().copied(), &ty);
                ty
            }
            Literal::UnitNumber { suffix, .. } => {
                numeric_literal_suffix_type(Some(suffix.as_str()))
                    .unwrap_or_else(|| TypeKind::Named("_".to_owned()))
            }
        }
    }

    pub(super) fn validate_integer_literal(&mut self, literal: &IntLiteral, target: &TypeKind) {
        let magnitude = match literal.magnitude() {
            Ok(magnitude) => magnitude,
            Err(error) => {
                self.errors.push(TypeCheckError::invalid_integer_literal(
                    literal.raw(),
                    error.to_string(),
                ));
                return;
            }
        };
        if !integer_magnitude_fits(magnitude, target, self.allow_signed_min_literal) {
            self.errors
                .push(TypeCheckError::integer_literal_out_of_range(
                    literal.raw(),
                    target.clone(),
                ));
        }
    }

    fn validate_float_literal(
        &mut self,
        raw: &str,
        suffix: Option<FloatSuffix>,
        target: &TypeKind,
    ) {
        let suffix_len = suffix.map_or(0, |suffix| suffix.as_str().len());
        let number_end = raw.len().saturating_sub(suffix_len);
        let normalized = raw[..number_end]
            .chars()
            .filter(|ch| *ch != '_')
            .collect::<String>();
        let finite = match target {
            TypeKind::F32 => normalized.parse::<f32>().is_ok_and(f32::is_finite),
            TypeKind::F64 => normalized.parse::<f64>().is_ok_and(f64::is_finite),
            _ => true,
        };
        if !finite {
            self.errors.push(TypeCheckError::float_literal_out_of_range(
                raw,
                target.clone(),
            ));
        }
    }

    pub(super) fn check_path_expr_with_expected(
        &mut self,
        path: &DottedPath,
        expected: Option<&TypeKind>,
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        let label = path.as_label();
        if let Some(ty) = self.expected_short_variant_type(label, expected) {
            return Some(ty);
        }
        if path.is_single("None")
            && let Some(expected @ TypeKind::Option(_)) = expected
        {
            return Some(expected.clone());
        }
        self.check_path_expr(path, expression_id)
    }

    pub(super) fn check_short_variant_expr(
        &mut self,
        variant: &str,
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        let label = format!(".{variant}");
        if let Some(ty) = self.symbol_type(&label).cloned() {
            return ty;
        }
        if let Some(ty) = self.expected_short_variant_type(variant, expected) {
            return ty;
        }
        TypeKind::Named("Variant".to_owned())
    }

    fn expected_short_variant_type(
        &self,
        path: &str,
        expected: Option<&TypeKind>,
    ) -> Option<TypeKind> {
        let variant = path
            .strip_prefix('.')
            .or_else(|| path.strip_prefix('\''))
            .unwrap_or(path);
        match expected? {
            TypeKind::Choice(alternatives) => {
                let mut matches = alternatives
                    .iter()
                    .filter(|ty| self.enum_has_variant(ty, variant));
                let selected = matches.next()?;
                matches.next().is_none().then(|| selected.clone())
            }
            ty if self.enum_has_variant(ty, variant) => Some(ty.clone()),
            _ => None,
        }
    }

    fn check_path_expr(
        &mut self,
        path: &DottedPath,
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        let label = path.as_label();
        if let Some(state) = self.borrow_local_lifetimes.get(label) {
            match state {
                BorrowLocalState::Dropped => self.errors.push(TypeCheckError::new(format!(
                    "borrowed local `{label}` was used after it was dropped"
                ))),
                BorrowLocalState::MaybeDropped(_) => {
                    self.errors.push(TypeCheckError::new(format!(
                        "borrowed local `{label}` may have been dropped on another control-flow path"
                    )));
                }
                BorrowLocalState::Live(_) => {}
            }
        }
        if let Some(ty) = self.symbol_type_with_capture(label) {
            return Some(ty);
        }
        if let Some(ty) = self.function_value_type(label) {
            self.record_typed_lowering_evidence(TypedLoweringEvidence::new(
                expression_id,
                TypedLoweringEvidenceKind::FunctionValueReference {
                    callee: label.to_owned(),
                    ty: ty.clone(),
                },
            ));
            return Some(ty);
        }
        if let Some((prefix, ty)) = self.dotted_value_path_resolution(path) {
            if self.locals.contains_key(prefix.as_label()) {
                let _ = self.local_symbol_type_with_capture(prefix.as_label());
            }
            return Some(ty);
        }
        if let Some(ty) = builtin_path_type(label) {
            return Some(ty);
        }
        self.errors
            .push(TypeCheckError::new(format!("unknown symbol `{label}`")));
        None
    }

    pub(in crate::checker) fn dotted_value_path_resolution(
        &self,
        path: &DottedPath,
    ) -> Option<(DottedPath, TypeKind)> {
        if let Some(ty) = std_float_constant_type(path.as_label())
            .or_else(|| inline_failure_builtin_variant_type(path.as_label()))
        {
            return Some((path.clone(), ty));
        }
        for prefix_len in (1..path.segments().len()).rev() {
            let prefix = path.prefix(prefix_len)?;
            let Some(mut ty) = self
                .symbol_type(prefix.as_label())
                .cloned()
                .or_else(|| builtin_path_type(prefix.as_label()))
            else {
                continue;
            };
            for field in &path.segments()[prefix_len..] {
                ty = self.value_field_type(&ty, field.as_str())?;
            }
            return Some((prefix, ty));
        }
        None
    }
}

fn integer_magnitude_fits(magnitude: u128, target: &TypeKind, allow_signed_min: bool) -> bool {
    let signed_max = |bits: u32| {
        let max = if bits == 128 {
            i128::MAX as u128
        } else {
            (1_u128 << (bits - 1)) - 1
        };
        magnitude <= max + u128::from(allow_signed_min)
    };
    let unsigned_max = |bits: u32| bits == 128 || magnitude <= (1_u128 << bits).saturating_sub(1);
    match target {
        TypeKind::I8 => signed_max(8),
        TypeKind::I16 => signed_max(16),
        TypeKind::I32 => signed_max(32),
        TypeKind::I64 | TypeKind::ISize => signed_max(64),
        TypeKind::I128 => signed_max(128),
        TypeKind::U8 => unsigned_max(8),
        TypeKind::U16 => unsigned_max(16),
        TypeKind::U32 => unsigned_max(32),
        TypeKind::U64 | TypeKind::USize => unsigned_max(64),
        _ => true,
    }
}
