//! Literal, path, and short-variant expression typing.

use super::super::helpers::{builtin_path_type, numeric_literal_suffix_type};
use super::support::{
    has_multiple_numeric_choice_alternatives, is_unit_number_type,
    unique_numeric_choice_alternative,
};
use super::{
    BorrowLocalState, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use arcweft_lang_syntax::expr::{FloatSuffix, IntLiteral, Literal};

impl TypeChecker<'_> {
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
        path: &str,
        expected: Option<&TypeKind>,
        expression_id: TypeExpressionId,
    ) -> Option<TypeKind> {
        if let Some(ty) = self.expected_short_variant_type(path, expected) {
            return Some(ty);
        }
        if path == "None"
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
        let variant = path.strip_prefix('.').unwrap_or(path);
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

    fn check_path_expr(&mut self, path: &str, expression_id: TypeExpressionId) -> Option<TypeKind> {
        if let Some(state) = self.borrow_local_lifetimes.get(path) {
            match state {
                BorrowLocalState::Dropped => self.errors.push(TypeCheckError::new(format!(
                    "borrowed local `{path}` was used after it was dropped"
                ))),
                BorrowLocalState::MaybeDropped(_) => {
                    self.errors.push(TypeCheckError::new(format!(
                        "borrowed local `{path}` may have been dropped on another control-flow path"
                    )));
                }
                BorrowLocalState::Live(_) => {}
            }
        }
        if let Some(ty) = self.symbol_type_with_capture(path) {
            return Some(ty);
        }
        if let Some(ty) = self.function_value_type(path) {
            self.record_typed_lowering_evidence(TypedLoweringEvidence::new(
                expression_id,
                TypedLoweringEvidenceKind::FunctionValueReference {
                    callee: path.to_owned(),
                    ty: ty.clone(),
                },
            ));
            return Some(ty);
        }
        if let Some(ty) = self.check_dotted_path_target(path) {
            return Some(ty);
        }
        if let Some(ty) = builtin_path_type(path) {
            return Some(ty);
        }
        self.errors
            .push(TypeCheckError::new(format!("unknown symbol `{path}`")));
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
