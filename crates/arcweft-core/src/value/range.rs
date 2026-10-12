use super::{
    RuntimeEvalError, RuntimeInt, RuntimeSignedIntWidth, RuntimeUInt, RuntimeUnsignedIntWidth,
    RuntimeValue, runtime_value_label,
};
use crate::plan::{RuntimeBuiltinIteratorFamily, RuntimeIteratorEvidence, RuntimeTraitMethodId};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Width-preserving integer range value.
///
/// Runtime keeps range identity as a first-class value. Consumers that need
/// sequential access convert it to `RuntimeIterator` and call `next()`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeRange {
    Int {
        start: Option<RuntimeInt>,
        end: Option<RuntimeInt>,
        inclusive: bool,
    },
    UInt {
        start: Option<RuntimeUInt>,
        end: Option<RuntimeUInt>,
        inclusive: bool,
    },
}

impl RuntimeRange {
    pub fn new(
        start: Option<RuntimeValue>,
        end: Option<RuntimeValue>,
        inclusive: bool,
    ) -> Result<Self, RuntimeEvalError> {
        match (start, end) {
            (Some(RuntimeValue::Int(start)), Some(RuntimeValue::Int(end)))
                if Self::Int {
                    start: Some(start),
                    end: Some(end),
                    inclusive,
                }
                .signed_width()
                .is_some() =>
            {
                Ok(Self::Int {
                    start: Some(start),
                    end: Some(end),
                    inclusive,
                })
            }
            (Some(RuntimeValue::UInt(start)), Some(RuntimeValue::UInt(end)))
                if Self::UInt {
                    start: Some(start),
                    end: Some(end),
                    inclusive,
                }
                .unsigned_width()
                .is_some() =>
            {
                Ok(Self::UInt {
                    start: Some(start),
                    end: Some(end),
                    inclusive,
                })
            }
            (Some(RuntimeValue::Int(start)), None) => Ok(Self::Int {
                start: Some(start),
                end: None,
                inclusive,
            }),
            (None, Some(RuntimeValue::Int(end))) => Ok(Self::Int {
                start: None,
                end: Some(end),
                inclusive,
            }),
            (Some(RuntimeValue::UInt(start)), None) => Ok(Self::UInt {
                start: Some(start),
                end: None,
                inclusive,
            }),
            (None, Some(RuntimeValue::UInt(end))) => Ok(Self::UInt {
                start: None,
                end: Some(end),
                inclusive,
            }),
            (None, None) => Err(RuntimeEvalError::InvalidRange {
                reason: "range requires at least one typed integer bound".to_owned(),
            }),
            (start, end) => Err(RuntimeEvalError::InvalidRange {
                reason: format!(
                    "range bounds must be matching integer widths, found {} and {}",
                    range_bound_value_label(start.as_ref()),
                    range_bound_value_label(end.as_ref())
                ),
            }),
        }
    }

    pub(crate) fn signed_width(&self) -> Option<RuntimeSignedIntWidth> {
        let Self::Int { start, end, .. } = self else {
            return None;
        };
        let width = start.or(*end)?.width();
        start
            .iter()
            .chain(end)
            .all(|value| value.width() == width)
            .then_some(width)
    }

    pub(crate) fn unsigned_width(&self) -> Option<RuntimeUnsignedIntWidth> {
        let Self::UInt { start, end, .. } = self else {
            return None;
        };
        let width = start.or(*end)?.width();
        start
            .iter()
            .chain(end)
            .all(|value| value.width() == width)
            .then_some(width)
    }

    pub fn into_iterator(self) -> Result<RuntimeIterator, Self> {
        if self.signed_width().is_none() && self.unsigned_width().is_none() {
            return Err(self);
        }
        match self {
            Self::Int {
                start: Some(start),
                end: Some(end),
                inclusive,
            } => Ok(RuntimeIterator::Range(RuntimeRangeIterator::Int {
                width: start.width(),
                current: start.as_i128(),
                end: end.as_i128(),
                inclusive,
                done: false,
            })),
            Self::UInt {
                start: Some(start),
                end: Some(end),
                inclusive,
            } => Ok(RuntimeIterator::Range(RuntimeRangeIterator::UInt {
                width: start.width(),
                current: start.as_u128(),
                end: end.as_u128(),
                inclusive,
                done: false,
            })),
            range => Err(range),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Int {
                start,
                end,
                inclusive,
            } => range_label(
                "int",
                start.as_ref().map(ToString::to_string),
                end.as_ref().map(ToString::to_string),
                *inclusive,
            ),
            Self::UInt {
                start,
                end,
                inclusive,
            } => range_label(
                "uint",
                start.as_ref().map(ToString::to_string),
                end.as_ref().map(ToString::to_string),
                *inclusive,
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeIterator {
    Values { items: VecDeque<RuntimeValue> },
    Range(RuntimeRangeIterator),
    Witness(RuntimeIteratorWitness),
}

/// Mutable iterator receiver leased to its original executable method table.
///
/// ```compile_fail
/// use arcweft_core::value::{RuntimeIteratorWitness, RuntimeValue};
/// use arcweft_core::task::RuntimeProgramOwner;
/// use arcweft_core::plan::RuntimeTraitMethodId;
/// fn forge(owner: RuntimeProgramOwner) -> RuntimeIteratorWitness {
///     RuntimeIteratorWitness { owner, state: Box::new(RuntimeValue::Unit), next: RuntimeTraitMethodId(0) }
/// }
/// ```
#[derive(Clone)]
pub struct RuntimeIteratorWitness {
    owner: crate::task::RuntimeProgramOwner,
    state: Box<RuntimeValue>,
    next: RuntimeTraitMethodId,
}

#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum RuntimeIteratorWitnessError {
    #[error("iterator witness has no admitted next signature at method {next:?}")]
    Signature { next: RuntimeTraitMethodId },
    #[error("iterator witness belongs to a different executable program")]
    ForeignProgram,
    #[error(transparent)]
    State(#[from] crate::program_types::RuntimeProgramTypeError),
}

impl RuntimeIteratorWitness {
    pub(crate) fn try_new(
        owner: crate::task::RuntimeProgramOwner,
        state: RuntimeValue,
        next: RuntimeTraitMethodId,
    ) -> Result<Self, RuntimeIteratorWitnessError> {
        let (state_type, _) = owner
            .types()
            .iterator_witness_types(next)
            .ok_or(RuntimeIteratorWitnessError::Signature { next })?;
        owner.types().validate_live_value(
            state_type,
            &state,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )?;
        Ok(Self {
            owner,
            state: Box::new(state),
            next,
        })
    }

    pub fn validate_for_owner(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<(), RuntimeIteratorWitnessError> {
        if !self.owner.same_program(owner) {
            return Err(RuntimeIteratorWitnessError::ForeignProgram);
        }
        let (state_type, _) = owner
            .types()
            .iterator_witness_types(self.next)
            .ok_or(RuntimeIteratorWitnessError::Signature { next: self.next })?;
        owner.types().validate_live_value(
            state_type,
            &self.state,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )?;
        Ok(())
    }

    #[must_use]
    pub const fn owner(&self) -> &crate::task::RuntimeProgramOwner {
        &self.owner
    }
    #[must_use]
    pub fn state(&self) -> &RuntimeValue {
        &self.state
    }
    #[must_use]
    pub const fn next_method(&self) -> RuntimeTraitMethodId {
        self.next
    }
    pub(crate) fn state_mut(&mut self) -> &mut RuntimeValue {
        &mut self.state
    }
}

impl std::fmt::Debug for RuntimeIteratorWitness {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeIteratorWitness")
            .field("next", &self.next)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RuntimeIteratorWitness {
    fn eq(&self, other: &Self) -> bool {
        self.owner.same_program(&other.owner)
            && self.next == other.next
            && self.state == other.state
    }
}

impl Serialize for RuntimeIterator {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let owner = match self {
            Self::Witness(witness) => Some(witness.owner()),
            _ => None,
        };
        super::AwbcRuntimeValueSnapshot::iterator_from_live(self, owner)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimeIterator {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let snapshot = super::awbc_save::AwbcRuntimeIteratorSnapshot::deserialize(deserializer)?;
        let value = super::AwbcRuntimeValueSnapshot::Iterator(snapshot)
            .into_unrestricted_standalone_value()
            .map_err(serde::de::Error::custom)?;
        let RuntimeValue::Iterator(iterator) = value else {
            unreachable!("iterator snapshot owns its value kind")
        };
        Ok(iterator)
    }
}

impl RuntimeIterator {
    pub fn from_value(value: RuntimeValue) -> Result<Self, RuntimeValue> {
        match value {
            RuntimeValue::Seq(seq) => Ok(Self::values(seq.into_values())),
            RuntimeValue::Tuple(values) => Ok(Self::values(values)),
            RuntimeValue::Range(range) => range.into_iterator().map_err(RuntimeValue::Range),
            value => Err(value),
        }
    }

    pub fn from_value_with_evidence(
        value: RuntimeValue,
        evidence: &RuntimeIteratorEvidence,
    ) -> Result<Self, RuntimeValue> {
        match evidence {
            RuntimeIteratorEvidence::Builtin(evidence) => {
                Self::from_value_with_builtin_family(value, evidence.family)
            }
            RuntimeIteratorEvidence::Witness(_) => Err(value),
        }
    }

    pub fn from_value_with_builtin_family(
        value: RuntimeValue,
        family: RuntimeBuiltinIteratorFamily,
    ) -> Result<Self, RuntimeValue> {
        match family {
            RuntimeBuiltinIteratorFamily::Range => match value {
                RuntimeValue::Range(range) => range.into_iterator().map_err(RuntimeValue::Range),
                value => Err(value),
            },
            RuntimeBuiltinIteratorFamily::Seq => match value {
                RuntimeValue::Seq(seq) => Ok(Self::values(seq.into_values())),
                value => Err(value),
            },
            RuntimeBuiltinIteratorFamily::Stream => Err(value),
            RuntimeBuiltinIteratorFamily::Vec
            | RuntimeBuiltinIteratorFamily::Array
            | RuntimeBuiltinIteratorFamily::Slice
            | RuntimeBuiltinIteratorFamily::TupleHomogeneous => match value {
                RuntimeValue::Seq(seq) => Ok(Self::values(seq.into_values())),
                RuntimeValue::Tuple(values) => Ok(Self::values(values)),
                value => Err(value),
            },
        }
    }

    pub fn values(items: Vec<RuntimeValue>) -> Self {
        Self::Values {
            items: items.into(),
        }
    }

    pub fn try_witness(
        owner: crate::task::RuntimeProgramOwner,
        state: RuntimeValue,
        next: RuntimeTraitMethodId,
    ) -> Result<Self, RuntimeIteratorWitnessError> {
        RuntimeIteratorWitness::try_new(owner, state, next).map(Self::Witness)
    }
}

impl Iterator for RuntimeIterator {
    type Item = RuntimeValue;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Values { items } => items.pop_front(),
            Self::Range(range) => range.next(),
            Self::Witness(_) => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeRangeIterator {
    Int {
        width: RuntimeSignedIntWidth,
        current: i128,
        end: i128,
        inclusive: bool,
        done: bool,
    },
    UInt {
        width: RuntimeUnsignedIntWidth,
        current: u128,
        end: u128,
        inclusive: bool,
        done: bool,
    },
}

impl RuntimeRangeIterator {
    /// Validates the retained cursor without narrowing an exhausted cursor.
    /// Inclusive iteration may retain `end + 1` beyond the element width.
    pub(crate) fn validated_end(&self) -> Option<super::RuntimeScalarView<'static>> {
        match self {
            Self::Int {
                width,
                current,
                end,
                inclusive,
                done,
            } => {
                let bound = RuntimeInt::from_i128(*width, *end)?;
                let exhausted = if *inclusive {
                    current > end
                } else {
                    current >= end
                };
                let cursor_valid = RuntimeInt::from_i128(*width, *current).is_some()
                    || (*inclusive && end.checked_add(1) == Some(*current));
                (cursor_valid
                    && (!done
                        || exhausted
                        || (*inclusive && current == end && end.checked_add(1).is_none())))
                .then_some(super::RuntimeScalarView::Int(bound))
            }
            Self::UInt {
                width,
                current,
                end,
                inclusive,
                done,
            } => {
                let bound = RuntimeUInt::from_u128(*width, *end)?;
                let exhausted = if *inclusive {
                    current > end
                } else {
                    current >= end
                };
                let cursor_valid = RuntimeUInt::from_u128(*width, *current).is_some()
                    || (*inclusive && end.checked_add(1) == Some(*current));
                (cursor_valid
                    && (!done
                        || exhausted
                        || (*inclusive && current == end && end.checked_add(1).is_none())))
                .then_some(super::RuntimeScalarView::UInt(bound))
            }
        }
    }
}

impl Iterator for RuntimeRangeIterator {
    type Item = RuntimeValue;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Int {
                width,
                current,
                end,
                inclusive,
                done,
            } => next_signed_range_value(*width, current, *end, *inclusive, done),
            Self::UInt {
                width,
                current,
                end,
                inclusive,
                done,
            } => next_unsigned_range_value(*width, current, *end, *inclusive, done),
        }
    }
}

fn next_signed_range_value(
    width: RuntimeSignedIntWidth,
    current: &mut i128,
    end: i128,
    inclusive: bool,
    done: &mut bool,
) -> Option<RuntimeValue> {
    if *done
        || !(if inclusive {
            *current <= end
        } else {
            *current < end
        })
    {
        *done = true;
        return None;
    }
    let value = RuntimeInt::from_i128(width, *current).map(RuntimeValue::Int);
    if let Some(next) = current.checked_add(1) {
        *current = next;
    } else {
        *done = true;
    }
    value
}

fn next_unsigned_range_value(
    width: RuntimeUnsignedIntWidth,
    current: &mut u128,
    end: u128,
    inclusive: bool,
    done: &mut bool,
) -> Option<RuntimeValue> {
    if *done
        || !(if inclusive {
            *current <= end
        } else {
            *current < end
        })
    {
        *done = true;
        return None;
    }
    let value = RuntimeUInt::from_u128(width, *current).map(RuntimeValue::UInt);
    if let Some(next) = current.checked_add(1) {
        *current = next;
    } else {
        *done = true;
    }
    value
}

fn range_label(
    kind: &'static str,
    start: Option<String>,
    end: Option<String>,
    inclusive: bool,
) -> String {
    let separator = if inclusive { "..=" } else { ".." };
    format!(
        "range/{kind}/{}{}{}",
        start.unwrap_or_default(),
        separator,
        end.unwrap_or_default()
    )
}

fn range_bound_value_label(value: Option<&RuntimeValue>) -> String {
    value.map_or_else(|| "unbounded".to_owned(), runtime_value_label)
}
