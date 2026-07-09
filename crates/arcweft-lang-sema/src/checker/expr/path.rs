//! Literal, path, and short-variant expression typing.

use super::super::helpers::numeric_literal_suffix_type;
use super::support::{
    has_multiple_numeric_choice_alternatives, is_unit_number_type,
    unique_numeric_choice_alternative,
};
use super::{
    BorrowLocalState, TypeCheckError, TypeChecker, TypeExpressionId, TypeKind,
    TypedLoweringEvidence, TypedLoweringEvidenceKind,
};
use arcweft_lang_syntax::expr::Literal;

impl TypeChecker<'_> {
    pub(super) fn check_literal_expr(
        &mut self,
        literal: &Literal,
        expected: Option<&TypeKind>,
    ) -> TypeKind {
        match literal {
            Literal::String(_) => TypeKind::String,
            Literal::Char { .. } => TypeKind::Char,
            Literal::Bool(_) => TypeKind::Bool,
            Literal::Duration { .. } => TypeKind::Duration,
            Literal::Int { suffix, .. } => {
                if let Some(suffix) = suffix {
                    let Some(ty) = numeric_literal_suffix_type(Some(suffix.as_str())) else {
                        self.errors.push(TypeCheckError::new(format!(
                            "unknown integer literal suffix `{suffix}`"
                        )));
                        return TypeKind::Named("_".to_owned());
                    };
                    if ty.is_integer() || is_unit_number_type(&ty) {
                        ty
                    } else {
                        self.errors.push(TypeCheckError::new(format!(
                            "integer literal suffix must be an integer type, found {ty:?}"
                        )));
                        TypeKind::Named("_".to_owned())
                    }
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
                    self.record_numeric_fallback_in_inferred_closure("integer", TypeKind::I32);
                    TypeKind::I32
                }
            }
            Literal::Float { suffix, .. } => {
                if let Some(suffix) = suffix {
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
                    self.record_numeric_fallback_in_inferred_closure("float", TypeKind::F64);
                    TypeKind::F64
                }
            }
            Literal::UnitNumber { suffix, .. } => {
                numeric_literal_suffix_type(Some(suffix.as_str()))
                    .unwrap_or_else(|| TypeKind::Named("_".to_owned()))
            }
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
            self.record_typed_lowering_evidence(TypedLoweringEvidence {
                expression_id,
                kind: TypedLoweringEvidenceKind::FunctionValueReference {
                    callee: path.to_owned(),
                    ty: ty.clone(),
                },
            });
            return Some(ty);
        }
        if let Some(ty) = self.check_dotted_path_target(path) {
            return Some(ty);
        }
        if path == "None" {
            return Some(TypeKind::Option(Box::new(TypeKind::Named("_".to_owned()))));
        }
        if path == "asset" {
            return Some(TypeKind::Named("AssetApi".to_owned()));
        }
        if path == "voice" {
            return Some(TypeKind::Named("VoiceApi".to_owned()));
        }
        if path == "state" {
            return Some(TypeKind::Named("GameState".to_owned()));
        }
        if path == "line" {
            return Some(TypeKind::Named("LineContext".to_owned()));
        }
        if path == "auto" {
            return Some(TypeKind::Named("Auto".to_owned()));
        }
        if matches!(path, "InlineFailure" | "InlineFallback" | "FallbackStyle") {
            return Some(TypeKind::Named(format!("{path}Namespace")));
        }
        // Short enum-variant expressions such as `.Instant` rely on expected
        // type resolution in the full checker. The Phase 1 checker preserves
        // unknown short variants as variant values after registered symbols and
        // patch names had a chance to resolve.
        if path.starts_with('.') {
            return Some(TypeKind::Named("Variant".to_owned()));
        }
        self.errors
            .push(TypeCheckError::new(format!("unknown symbol `{path}`")));
        None
    }
}
