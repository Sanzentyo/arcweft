//! Producer-owned fixed-point scalars with the exact registered declaration identity.

use arcweft_primitives::{CheckedAngle, CheckedLength, LengthUnit, Milli, RatioMilli};

use super::{
    RuntimeBorrowedValue, RuntimeInt, RuntimeOpaquePersistence, RuntimeOpaqueValueClass,
    RuntimeOpaqueValueError, RuntimeScalarView, RuntimeUInt, RuntimeValue, RuntimeValueView,
};
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
    RuntimeSemanticTypeId,
};

/// Scalar representation selected by an accepted semantic declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeQuantityKind {
    Milli,
    Ratio,
    Length,
    Angle,
}

impl RuntimeQuantityKind {
    const ALL: [Self; 4] = [Self::Milli, Self::Ratio, Self::Length, Self::Angle];

    fn producer_name(self) -> &'static str {
        match self {
            Self::Milli => "std.scalar.milli",
            Self::Ratio => "std.scalar.ratio",
            Self::Length => "std.scalar.length",
            Self::Angle => "std.scalar.angle",
        }
    }

    fn for_producer(producer: &RuntimeOpaqueTypeProducerId) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.producer_name() == producer.as_str())
    }

    /// Classifies only a concrete, unrestricted scalar owner.
    #[must_use]
    pub fn from_owner(owner: &RuntimeOpaqueTypeOwner) -> Option<Self> {
        if owner.admission() != RuntimeOpaqueTypeAdmission::ExactIdentity
            || owner.value_class() != RuntimeOpaqueValueClass::Plain
            || owner.persistence() != RuntimeOpaquePersistence::ConstantAndSnapshot
        {
            return None;
        }
        Self::for_producer(owner.producer())
    }

    #[must_use]
    pub const fn supports_negation(self) -> bool {
        !matches!(self, Self::Ratio)
    }

    /// The caller supplies the accepted declaration's semantic identity;
    /// a diagnostic spelling never creates a nominal identity here.
    #[must_use]
    pub fn owner(self, identity: RuntimeSemanticTypeId) -> RuntimeOpaqueTypeOwner {
        RuntimeOpaqueTypeOwner::exact(
            RuntimeOpaqueTypeProducerId::try_new(self.producer_name())
                .expect("quantity producers have valid identities"),
            identity,
        )
    }

    fn decode_payload(self, payload: RuntimeValueView<'_>) -> Option<RuntimeQuantity> {
        Some(match (self, payload) {
            (
                Self::Milli,
                RuntimeValueView::Scalar(RuntimeScalarView::Int(RuntimeInt::I32(value))),
            ) => RuntimeQuantity::Milli(Milli(value)),
            (
                Self::Ratio,
                RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U16(value))),
            ) => RuntimeQuantity::Ratio(RatioMilli::try_new(value)?),
            (
                Self::Angle,
                RuntimeValueView::Scalar(RuntimeScalarView::Int(RuntimeInt::I32(value))),
            ) => RuntimeQuantity::Angle(CheckedAngle {
                milli_degrees: value,
            }),
            (Self::Length, RuntimeValueView::Tuple(fields)) if fields.len() == 2 => {
                let RuntimeValueView::Scalar(RuntimeScalarView::Int(RuntimeInt::I32(milli))) =
                    fields.get(0)?
                else {
                    return None;
                };
                let RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U8(unit))) =
                    fields.get(1)?
                else {
                    return None;
                };
                RuntimeQuantity::Length(CheckedLength {
                    milli,
                    unit: LengthUnit::from_encoded(unit)?,
                })
            }
            _ => return None,
        })
    }
}

/// Typed inspection/construction of the canonical opaque scalar carrier.
/// Values keep their unit until the consuming layout context resolves it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeQuantity {
    Milli(Milli),
    Ratio(RatioMilli),
    Length(CheckedLength),
    Angle(CheckedAngle),
}

impl RuntimeQuantity {
    /// Negates a signed scalar without dropping its unit or wrapping overflow.
    #[must_use]
    pub fn checked_neg(self) -> Option<Self> {
        Some(match self {
            Self::Milli(Milli(value)) => Self::Milli(Milli(value.checked_neg()?)),
            Self::Length(value) => Self::Length(CheckedLength {
                milli: value.milli.checked_neg()?,
                unit: value.unit,
            }),
            Self::Angle(value) => Self::Angle(CheckedAngle {
                milli_degrees: value.milli_degrees.checked_neg()?,
            }),
            Self::Ratio(_) => return None,
        })
    }
    #[must_use]
    pub const fn kind(self) -> RuntimeQuantityKind {
        match self {
            Self::Milli(_) => RuntimeQuantityKind::Milli,
            Self::Ratio(_) => RuntimeQuantityKind::Ratio,
            Self::Length(_) => RuntimeQuantityKind::Length,
            Self::Angle(_) => RuntimeQuantityKind::Angle,
        }
    }

    pub fn into_runtime_value(
        self,
        identity: RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, RuntimeOpaqueValueError> {
        let payload = match self {
            Self::Milli(Milli(value)) => RuntimeValue::Int(RuntimeInt::I32(value)),
            Self::Ratio(value) => RuntimeValue::UInt(RuntimeUInt::U16(value.get())),
            Self::Length(value) => RuntimeValue::Tuple(vec![
                RuntimeValue::Int(RuntimeInt::I32(value.milli)),
                RuntimeValue::UInt(RuntimeUInt::U8(value.unit.encoded())),
            ]),
            Self::Angle(value) => RuntimeValue::Int(RuntimeInt::I32(value.milli_degrees)),
        };
        self.kind().owner(identity).try_wrap(payload)
    }

    /// Borrows live values or inert snapshots without activating or copying them.
    #[must_use]
    pub fn from_value<'a>(value: impl Into<RuntimeBorrowedValue<'a>>) -> Option<Self> {
        let opaque = value.into().opaque()?;
        let kind = RuntimeQuantityKind::for_producer(opaque.producer())?;
        let owner = kind.owner(opaque.semantic_identity());
        if !owner.accepts_opaque_view(opaque.value) {
            return None;
        }
        kind.decode_payload(opaque.value.payload())
    }
}

pub(crate) fn quantity_payload_is_valid(
    owner: &RuntimeOpaqueTypeOwner,
    payload: RuntimeValueView<'_>,
) -> bool {
    RuntimeQuantityKind::for_producer(owner.producer()).is_none_or(|kind| {
        owner.admission() == RuntimeOpaqueTypeAdmission::ExactIdentity
            && owner.value_class() == RuntimeOpaqueValueClass::Plain
            && owner.persistence() == RuntimeOpaquePersistence::ConstantAndSnapshot
            && kind.decode_payload(payload).is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::RuntimeCheckedType;

    #[test]
    fn quantity_carriers_preserve_unit_and_exact_declaration() {
        let identity = RuntimeSemanticTypeId::from_bytes([7; 32]);
        let other = RuntimeSemanticTypeId::from_bytes([8; 32]);
        for unit in [
            LengthUnit::Px,
            LengthUnit::Pt,
            LengthUnit::Ch,
            LengthUnit::Em,
            LengthUnit::Rem,
            LengthUnit::Vw,
            LengthUnit::Vh,
            LengthUnit::Percent,
        ] {
            let quantity = RuntimeQuantity::Length(CheckedLength { milli: -1250, unit });
            let value = quantity.into_runtime_value(identity).unwrap();
            assert_eq!(RuntimeQuantity::from_value(&value), Some(quantity));
            assert!(
                RuntimeCheckedType::Opaque {
                    owner: RuntimeQuantityKind::Length.owner(identity)
                }
                .accepts_value(&value)
            );
            assert!(
                !RuntimeCheckedType::Opaque {
                    owner: RuntimeQuantityKind::Length.owner(other)
                }
                .accepts_value(&value)
            );
            let decoded: RuntimeValue =
                serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
            assert_eq!(RuntimeQuantity::from_value(&decoded), Some(quantity));
        }
    }

    #[test]
    fn producer_rejects_wrong_payload_width_unit_and_ratio_range() {
        let identity = RuntimeSemanticTypeId::from_bytes([7; 32]);
        let owner = RuntimeQuantityKind::Length.owner(identity);
        for payload in [
            RuntimeValue::String("12px".to_owned()),
            RuntimeValue::Tuple(vec![
                RuntimeValue::Int(RuntimeInt::I64(12)),
                RuntimeValue::UInt(RuntimeUInt::U8(0)),
            ]),
            RuntimeValue::Tuple(vec![
                RuntimeValue::Int(RuntimeInt::I32(12)),
                RuntimeValue::UInt(RuntimeUInt::U8(255)),
            ]),
        ] {
            assert!(owner.try_wrap(payload).is_err());
        }
        assert!(
            RuntimeQuantityKind::Ratio
                .owner(identity)
                .try_wrap(RuntimeValue::UInt(RuntimeUInt::U16(1001)))
                .is_err()
        );
    }

    #[test]
    fn signed_negation_preserves_units_and_rejects_overflow_and_ratios() {
        assert_eq!(
            RuntimeQuantity::Length(CheckedLength {
                milli: 1250,
                unit: LengthUnit::Rem
            })
            .checked_neg(),
            Some(RuntimeQuantity::Length(CheckedLength {
                milli: -1250,
                unit: LengthUnit::Rem
            }))
        );
        assert_eq!(
            RuntimeQuantity::Milli(Milli(-2)).checked_neg(),
            Some(RuntimeQuantity::Milli(Milli(2)))
        );
        assert_eq!(
            RuntimeQuantity::Angle(CheckedAngle {
                milli_degrees: 90000
            })
            .checked_neg(),
            Some(RuntimeQuantity::Angle(CheckedAngle {
                milli_degrees: -90000
            }))
        );
        assert_eq!(RuntimeQuantity::Milli(Milli(i32::MIN)).checked_neg(), None);
        assert_eq!(
            RuntimeQuantity::Ratio(RatioMilli::try_new(1).unwrap()).checked_neg(),
            None
        );
    }

    #[test]
    fn deserialized_header_is_not_evidence_for_an_invalid_quantity() {
        let identity = RuntimeSemanticTypeId::from_bytes([7; 32]);
        let value = RuntimeQuantity::Length(CheckedLength {
            milli: 1250,
            unit: LengthUnit::Px,
        })
        .into_runtime_value(identity)
        .unwrap();
        let mut wire = serde_json::to_value(&value).unwrap();
        wire["Opaque"]["payload"] =
            serde_json::to_value(RuntimeValue::String("1.25px".to_owned())).unwrap();
        let forged: RuntimeValue = serde_json::from_value(wire).unwrap();
        assert_eq!(RuntimeQuantity::from_value(&forged), None);
        assert!(
            !RuntimeCheckedType::Opaque {
                owner: RuntimeQuantityKind::Length.owner(identity)
            }
            .accepts_value(&forged)
        );
    }
}
