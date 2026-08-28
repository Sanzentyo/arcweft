//! Canonical literal grammar and identities for checked Match coverage.

use std::sync::Arc;

use arcweft_lang_hir::leaf::{
    HirCharacterLiteral, HirDurationLiteral, HirFloatLiteral, HirIntegerLiteral, HirLiteral,
    HirStringLiteral, HirUnitNumberLiteral,
};

use super::match_transaction::{
    CheckedMatchBudget, CheckedMatchBuildError, CheckedMatchLimitKind, checked_len,
};
use crate::{
    semantic_coordinate::StableSemanticCoordinate,
    types::{SemanticTypeDigest, TypeKind},
};

/// Canonical checked literal meaning used by open-domain witnesses.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CanonicalCoverageLiteral {
    pub(super) semantic_type: SemanticTypeDigest,
    pub(super) bytes: Arc<[u8]>,
}

impl CanonicalCoverageLiteral {
    pub(super) fn from_checked(
        literal: &HirLiteral,
        ty: &TypeKind,
        budget: &mut CheckedMatchBudget,
        retained_before: u64,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<(Self, u64), CheckedMatchBuildError> {
        let mut retained = retained_before;
        let mut literal_bytes = 0_u64;
        encode_canonical_literal(literal, ty, |chunk| {
            let length = checked_len(chunk.len(), CheckedMatchLimitKind::TranscriptBytes)?;
            literal_bytes = literal_bytes.checked_add(length).ok_or(
                CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::TranscriptBytes,
                },
            )?;
            retained =
                retained
                    .checked_add(length)
                    .ok_or(CheckedMatchBuildError::ArithmeticOverflow {
                        kind: CheckedMatchLimitKind::TranscriptBytes,
                    })?;
            budget.admit_transcript_allocation(retained)?;
            Ok(())
        })
        .map_err(|error| map_canonical_literal_encoding_error(error, coordinate))?;
        let capacity = usize::try_from(literal_bytes).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::TranscriptBytes,
            }
        })?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(capacity).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::TranscriptBytes,
            }
        })?;
        encode_canonical_literal(literal, ty, |chunk| {
            bytes.extend_from_slice(chunk);
            Ok::<_, CheckedMatchBuildError>(())
        })
        .map_err(|error| map_canonical_literal_encoding_error(error, coordinate))?;
        if bytes.len() != capacity {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: coordinate.clone(),
            });
        }
        Ok((
            Self {
                semantic_type: ty.semantic_identity_digest(),
                bytes: bytes.into(),
            },
            retained,
        ))
    }
}

fn map_canonical_literal_encoding_error(
    error: CanonicalLiteralEncodingError<CheckedMatchBuildError>,
    coordinate: &StableSemanticCoordinate,
) -> CheckedMatchBuildError {
    match error {
        CanonicalLiteralEncodingError::Invalid => CheckedMatchBuildError::PoisonedSemanticNode {
            coordinate: coordinate.clone(),
        },
        CanonicalLiteralEncodingError::ArithmeticOverflow => {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::TranscriptBytes,
            }
        }
        CanonicalLiteralEncodingError::Sink(error) => error,
    }
}

pub(crate) enum CanonicalLiteralEncodingError<E> {
    Invalid,
    ArithmeticOverflow,
    Sink(E),
}

pub(crate) fn encode_canonical_literal<E>(
    literal: &HirLiteral,
    ty: &TypeKind,
    mut write: impl FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), CanonicalLiteralEncodingError<E>> {
    match literal {
        HirLiteral::String(HirStringLiteral::Value(value)) => {
            write(&[0]).map_err(CanonicalLiteralEncodingError::Sink)?;
            write_canonical_len(&mut write, value.len())?;
            write(value.as_bytes()).map_err(CanonicalLiteralEncodingError::Sink)?;
        }
        HirLiteral::Character(HirCharacterLiteral::Value(value)) => {
            write(&[1]).map_err(CanonicalLiteralEncodingError::Sink)?;
            write(&u32::from(*value).to_le_bytes()).map_err(CanonicalLiteralEncodingError::Sink)?;
        }
        HirLiteral::Integer(HirIntegerLiteral::Value { magnitude, .. }) => {
            write(&[2]).map_err(CanonicalLiteralEncodingError::Sink)?;
            write_canonical_len(&mut write, magnitude.limbs_le().len())?;
            for limb in magnitude.limbs_le() {
                write(&limb.to_le_bytes()).map_err(CanonicalLiteralEncodingError::Sink)?;
            }
        }
        HirLiteral::Float(HirFloatLiteral::Value { decimal, .. }) => {
            write(&[3]).map_err(CanonicalLiteralEncodingError::Sink)?;
            let canonical = decimal.to_decimal_string();
            match ty {
                TypeKind::F32 => {
                    write(&[0]).map_err(CanonicalLiteralEncodingError::Sink)?;
                    let value = canonical
                        .parse::<f32>()
                        .map_err(|_| CanonicalLiteralEncodingError::Invalid)?;
                    write(&value.to_bits().to_le_bytes())
                        .map_err(CanonicalLiteralEncodingError::Sink)?;
                }
                TypeKind::F64 => {
                    write(&[1]).map_err(CanonicalLiteralEncodingError::Sink)?;
                    let value = canonical
                        .parse::<f64>()
                        .map_err(|_| CanonicalLiteralEncodingError::Invalid)?;
                    write(&value.to_bits().to_le_bytes())
                        .map_err(CanonicalLiteralEncodingError::Sink)?;
                }
                _ => return Err(CanonicalLiteralEncodingError::Invalid),
            }
        }
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { decimal, unit }) => {
            write(&[4, unit_number_tag(*unit)]).map_err(CanonicalLiteralEncodingError::Sink)?;
            write_canonical_len(&mut write, decimal.coefficient().digits().len())?;
            write(decimal.coefficient().digits()).map_err(CanonicalLiteralEncodingError::Sink)?;
            write(&decimal.scale().to_le_bytes()).map_err(CanonicalLiteralEncodingError::Sink)?;
            write(&decimal.exponent10().to_le_bytes())
                .map_err(CanonicalLiteralEncodingError::Sink)?;
        }
        HirLiteral::Boolean(value) => {
            write(&[5, u8::from(*value)]).map_err(CanonicalLiteralEncodingError::Sink)?;
        }
        HirLiteral::Duration(HirDurationLiteral::Value(value)) => {
            write(&[6]).map_err(CanonicalLiteralEncodingError::Sink)?;
            let limbs = value.semantic_value().nanoseconds().limbs_le();
            write_canonical_len(&mut write, limbs.len())?;
            for limb in limbs {
                write(&limb.to_le_bytes()).map_err(CanonicalLiteralEncodingError::Sink)?;
            }
        }
        HirLiteral::String(HirStringLiteral::Invalid(_))
        | HirLiteral::Character(HirCharacterLiteral::Invalid(_))
        | HirLiteral::Integer(HirIntegerLiteral::Invalid(_))
        | HirLiteral::Float(HirFloatLiteral::Invalid(_))
        | HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(_))
        | HirLiteral::Duration(HirDurationLiteral::Invalid(_)) => {
            return Err(CanonicalLiteralEncodingError::Invalid);
        }
    }
    Ok(())
}

fn write_canonical_len<E>(
    write: &mut impl FnMut(&[u8]) -> Result<(), E>,
    value: usize,
) -> Result<(), CanonicalLiteralEncodingError<E>> {
    let value =
        u64::try_from(value).map_err(|_| CanonicalLiteralEncodingError::ArithmeticOverflow)?;
    write(&value.to_le_bytes()).map_err(CanonicalLiteralEncodingError::Sink)
}

fn unit_number_tag(unit: arcweft_lang_hir::leaf::HirUnitNumberUnit) -> u8 {
    use arcweft_lang_hir::leaf::HirUnitNumberUnit;
    match unit {
        HirUnitNumberUnit::Percent => 0,
        HirUnitNumberUnit::Px => 1,
        HirUnitNumberUnit::Pt => 2,
        HirUnitNumberUnit::Em => 3,
        HirUnitNumberUnit::Rem => 4,
        HirUnitNumberUnit::Vw => 5,
        HirUnitNumberUnit::Vh => 6,
        HirUnitNumberUnit::Deg => 7,
        HirUnitNumberUnit::Rad => 8,
        HirUnitNumberUnit::Turn => 9,
        HirUnitNumberUnit::Db => 10,
        HirUnitNumberUnit::Lufs => 11,
        HirUnitNumberUnit::Bpm => 12,
        HirUnitNumberUnit::Bars => 13,
    }
}
