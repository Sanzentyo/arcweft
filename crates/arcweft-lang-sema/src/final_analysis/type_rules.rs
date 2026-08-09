//! Pure type rules shared by final analysis and publication validation.

use arcweft_lang_hir::leaf::HirIntegerSuffix;

use super::TypeKind;

pub(super) fn integer_suffix_type(suffix: Option<HirIntegerSuffix>) -> Option<TypeKind> {
    Some(match suffix? {
        HirIntegerSuffix::I8 => TypeKind::I8,
        HirIntegerSuffix::I16 => TypeKind::I16,
        HirIntegerSuffix::I32 => TypeKind::I32,
        HirIntegerSuffix::I64 => TypeKind::I64,
        HirIntegerSuffix::I128 => TypeKind::I128,
        HirIntegerSuffix::ISize => TypeKind::ISize,
        HirIntegerSuffix::U8 => TypeKind::U8,
        HirIntegerSuffix::U16 => TypeKind::U16,
        HirIntegerSuffix::U32 => TypeKind::U32,
        HirIntegerSuffix::U64 => TypeKind::U64,
        HirIntegerSuffix::U128 => TypeKind::U128,
        HirIntegerSuffix::USize => TypeKind::USize,
    })
}

pub(super) fn is_integer(ty: &TypeKind) -> bool {
    matches!(
        ty,
        TypeKind::I8
            | TypeKind::I16
            | TypeKind::I32
            | TypeKind::I64
            | TypeKind::I128
            | TypeKind::ISize
            | TypeKind::U8
            | TypeKind::U16
            | TypeKind::U32
            | TypeKind::U64
            | TypeKind::U128
            | TypeKind::USize
    )
}

/// Selects the semantic type of one physically-addressed compact numeric
/// element. The sequence's authored common suffix wins, followed by an exact
/// integer expectation, then the deterministic `I64` fallback.
pub(super) fn compact_numeric_element_type(
    common_suffix: Option<HirIntegerSuffix>,
    expected: Option<&TypeKind>,
) -> TypeKind {
    integer_suffix_type(common_suffix)
        .or_else(|| expected.filter(|ty| is_integer(ty)).cloned())
        .unwrap_or(TypeKind::I64)
}
