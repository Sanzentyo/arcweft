use super::{
    RuntimeEvalError, RuntimeInt, RuntimeSignedIntWidth, RuntimeUInt, RuntimeUnsignedIntWidth,
    RuntimeValue, runtime_value_label,
};
use crate::plan::{RuntimeBuiltinIteratorEvidence, RuntimeIteratorEvidence, RuntimeTraitMethodId};
use serde::{Deserialize, Serialize};

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
                if runtime_int_same_kind(start, end) =>
            {
                Ok(Self::Int {
                    start: Some(start),
                    end: Some(end),
                    inclusive,
                })
            }
            (Some(RuntimeValue::UInt(start)), Some(RuntimeValue::UInt(end)))
                if runtime_uint_same_kind(start, end) =>
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

    pub fn into_iterator(self) -> Result<RuntimeIterator, Self> {
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeIterator {
    Values {
        items: Vec<RuntimeValue>,
        index: usize,
    },
    Range(RuntimeRangeIterator),
    Witness {
        state: Box<RuntimeValue>,
        next: RuntimeTraitMethodId,
    },
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
            RuntimeIteratorEvidence::Builtin(RuntimeBuiltinIteratorEvidence::Range) => {
                match value {
                    RuntimeValue::Range(range) => {
                        range.into_iterator().map_err(RuntimeValue::Range)
                    }
                    value => Err(value),
                }
            }
            RuntimeIteratorEvidence::Builtin(RuntimeBuiltinIteratorEvidence::Seq) => match value {
                RuntimeValue::Seq(seq) => Ok(Self::values(seq.into_values())),
                value => Err(value),
            },
            RuntimeIteratorEvidence::Builtin(RuntimeBuiltinIteratorEvidence::Stream)
            | RuntimeIteratorEvidence::Witness(_) => Err(value),
            RuntimeIteratorEvidence::Builtin(
                RuntimeBuiltinIteratorEvidence::Vec
                | RuntimeBuiltinIteratorEvidence::Array
                | RuntimeBuiltinIteratorEvidence::Slice
                | RuntimeBuiltinIteratorEvidence::TupleHomogeneous,
            ) => match value {
                RuntimeValue::Seq(seq) => Ok(Self::values(seq.into_values())),
                RuntimeValue::Tuple(values) => Ok(Self::values(values)),
                value => Err(value),
            },
        }
    }

    pub fn values(items: Vec<RuntimeValue>) -> Self {
        Self::Values { items, index: 0 }
    }

    pub fn witness(state: RuntimeValue, next: RuntimeTraitMethodId) -> Self {
        Self::Witness {
            state: Box::new(state),
            next,
        }
    }
}

impl Iterator for RuntimeIterator {
    type Item = RuntimeValue;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Values { items, index } => {
                let value = items.get(*index).cloned()?;
                *index += 1;
                Some(value)
            }
            Self::Range(range) => range.next(),
            Self::Witness { .. } => None,
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

fn runtime_int_same_kind(lhs: RuntimeInt, rhs: RuntimeInt) -> bool {
    lhs.width() == rhs.width()
}

fn runtime_uint_same_kind(lhs: RuntimeUInt, rhs: RuntimeUInt) -> bool {
    lhs.width() == rhs.width()
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
