//! Pure type rules shared by final analysis and publication validation.

use arcweft_lang_hir::leaf::{HirBigUint, HirIntegerSuffix};

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

/// Admission of a non-negative compact-sequence element to its selected
/// integer primitive. Unary negation is not an element of this compact form,
/// so signed minima do not need their extra positive magnitude here.
pub(super) fn nonnegative_integer_magnitude_fits(magnitude: &HirBigUint, ty: &TypeKind) -> bool {
    let maximum = match ty {
        TypeKind::I8 => i8::MAX as u128,
        TypeKind::I16 => i16::MAX as u128,
        TypeKind::I32 => i32::MAX as u128,
        TypeKind::I64 => i64::MAX as u128,
        TypeKind::I128 => i128::MAX as u128,
        TypeKind::ISize => isize::MAX as u128,
        TypeKind::U8 => u8::MAX as u128,
        TypeKind::U16 => u16::MAX as u128,
        TypeKind::U32 => u32::MAX as u128,
        TypeKind::U64 => u64::MAX as u128,
        TypeKind::U128 => u128::MAX,
        TypeKind::USize => usize::MAX as u128,
        _ => return false,
    };
    let Some(value) = magnitude
        .limbs_le()
        .iter()
        .rev()
        .try_fold(0_u128, |value, limb| {
            value
                .checked_mul(1_u128 << 32)?
                .checked_add(u128::from(*limb))
        })
    else {
        return false;
    };
    value <= maximum
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
