use crate::{runtime_id::RuntimeCaptureSlotId, value::RuntimeRecordFieldId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use thiserror::Error;

/// Hard maximum for a canonical runtime-value path.
pub const MAX_RUNTIME_VALUE_PATH_SEGMENTS: u32 = 64;

/// Canonical path from a runtime-value graph root to one nested value.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeValuePath(Box<[RuntimeValuePathSegment]>);

/// One canonical edge in a runtime-value graph.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeValuePathSegment {
    TupleElement(u32),
    SequenceElement(u64),
    TupleColumn(u32),
    RecordField(RuntimeRecordFieldId),
    RecordColumn(RuntimeRecordFieldId),
    NominalRecordField(RuntimeRecordFieldId),
    FunctionCapture(RuntimeCaptureSlotId),
    VariantPayload,
    IteratorRemainder(u64),
    IteratorWitnessState,
    OpaquePayload,
    ReductionState,
    ReductionCommandPayload(u32),
    AgentEmbeddedValue(u32),
}

/// Failure to construct or resolve a canonical runtime-value path.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeValuePathError {
    #[error("runtime value path has {actual} segments; maximum is {maximum}")]
    TooDeep { maximum: u32, actual: usize },
    #[error("runtime value path does not exist in the selected value graph")]
    Missing { path: RuntimeValuePath },
    #[error("runtime value path segment {segment} has the wrong aggregate kind")]
    WrongAggregateKind { segment: usize },
    #[error("runtime record field identities are not contiguous and unique")]
    InvalidRecordFieldIdentity,
}

impl RuntimeValuePath {
    #[must_use]
    pub fn root() -> Self {
        Self(Box::new([]))
    }

    pub fn try_from_segments(
        segments: impl IntoIterator<Item = RuntimeValuePathSegment>,
    ) -> Result<Self, RuntimeValuePathError> {
        let segments = segments.into_iter().collect::<Vec<_>>();
        if segments.len() > MAX_RUNTIME_VALUE_PATH_SEGMENTS as usize {
            return Err(RuntimeValuePathError::TooDeep {
                maximum: MAX_RUNTIME_VALUE_PATH_SEGMENTS,
                actual: segments.len(),
            });
        }
        Ok(Self(segments.into_boxed_slice()))
    }

    #[must_use]
    pub fn segments(&self) -> &[RuntimeValuePathSegment] {
        &self.0
    }

    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn child(&self, segment: RuntimeValuePathSegment) -> Result<Self, RuntimeValuePathError> {
        let actual = self.0.len() + 1;
        if actual > MAX_RUNTIME_VALUE_PATH_SEGMENTS as usize {
            return Err(RuntimeValuePathError::TooDeep {
                maximum: MAX_RUNTIME_VALUE_PATH_SEGMENTS,
                actual,
            });
        }
        let mut segments = Vec::with_capacity(actual);
        segments.extend_from_slice(&self.0);
        segments.push(segment);
        Ok(Self(segments.into_boxed_slice()))
    }
}

impl RuntimeValuePathSegment {
    #[must_use]
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::TupleElement(_) => 0,
            Self::SequenceElement(_) => 1,
            Self::TupleColumn(_) => 2,
            Self::RecordField(_) => 3,
            Self::RecordColumn(_) => 4,
            Self::NominalRecordField(_) => 5,
            Self::FunctionCapture(_) => 6,
            Self::VariantPayload => 7,
            Self::IteratorRemainder(_) => 8,
            Self::IteratorWitnessState => 9,
            Self::OpaquePayload => 10,
            Self::ReductionState => 11,
            Self::ReductionCommandPayload(_) => 12,
            Self::AgentEmbeddedValue(_) => 13,
        }
    }
}

impl Ord for RuntimeValuePathSegment {
    fn cmp(&self, other: &Self) -> Ordering {
        self.canonical_tag()
            .cmp(&other.canonical_tag())
            .then_with(|| match (self, other) {
                (Self::TupleElement(left), Self::TupleElement(right))
                | (Self::TupleColumn(left), Self::TupleColumn(right)) => left.cmp(right),
                (Self::SequenceElement(left), Self::SequenceElement(right))
                | (Self::IteratorRemainder(left), Self::IteratorRemainder(right)) => {
                    left.cmp(right)
                }
                (Self::RecordField(left), Self::RecordField(right))
                | (Self::RecordColumn(left), Self::RecordColumn(right))
                | (Self::NominalRecordField(left), Self::NominalRecordField(right)) => {
                    left.cmp(right)
                }
                (Self::FunctionCapture(left), Self::FunctionCapture(right)) => left.cmp(right),
                (
                    Self::ReductionCommandPayload(left) | Self::AgentEmbeddedValue(left),
                    Self::ReductionCommandPayload(right) | Self::AgentEmbeddedValue(right),
                ) => left.cmp(right),
                _ => Ordering::Equal,
            })
    }
}

impl PartialOrd for RuntimeValuePathSegment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RuntimeValuePath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for RuntimeValuePath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HumanPathSegmentRef {
    TupleElement { index: u32 },
    SequenceElement { index: String },
    TupleColumn { index: u32 },
    RecordField { field: RuntimeRecordFieldId },
    RecordColumn { field: RuntimeRecordFieldId },
    NominalRecordField { field: RuntimeRecordFieldId },
    FunctionCapture { capture: RuntimeCaptureSlotId },
    VariantPayload,
    IteratorRemainder { index: String },
    IteratorWitnessState,
    OpaquePayload {},
    ReductionState,
    ReductionCommandPayload { index: u32 },
    AgentEmbeddedValue { index: u32 },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum HumanPathSegment {
    TupleElement { index: u32 },
    SequenceElement { index: String },
    TupleColumn { index: u32 },
    RecordField { field: RuntimeRecordFieldId },
    RecordColumn { field: RuntimeRecordFieldId },
    NominalRecordField { field: RuntimeRecordFieldId },
    FunctionCapture { capture: RuntimeCaptureSlotId },
    VariantPayload,
    IteratorRemainder { index: String },
    IteratorWitnessState,
    OpaquePayload {},
    ReductionState,
    ReductionCommandPayload { index: u32 },
    AgentEmbeddedValue { index: u32 },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum NonHumanPathSegmentRef {
    TupleElement { index: u32 },
    SequenceElement { index: u64 },
    TupleColumn { index: u32 },
    RecordField { field: RuntimeRecordFieldId },
    RecordColumn { field: RuntimeRecordFieldId },
    NominalRecordField { field: RuntimeRecordFieldId },
    FunctionCapture { capture: RuntimeCaptureSlotId },
    VariantPayload,
    IteratorRemainder { index: u64 },
    IteratorWitnessState,
    OpaquePayload {},
    ReductionState,
    ReductionCommandPayload { index: u32 },
    AgentEmbeddedValue { index: u32 },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum NonHumanPathSegment {
    TupleElement { index: u32 },
    SequenceElement { index: u64 },
    TupleColumn { index: u32 },
    RecordField { field: RuntimeRecordFieldId },
    RecordColumn { field: RuntimeRecordFieldId },
    NominalRecordField { field: RuntimeRecordFieldId },
    FunctionCapture { capture: RuntimeCaptureSlotId },
    VariantPayload,
    IteratorRemainder { index: u64 },
    IteratorWitnessState,
    OpaquePayload {},
    ReductionState,
    ReductionCommandPayload { index: u32 },
    AgentEmbeddedValue { index: u32 },
}

fn parse_canonical_u64<E: serde::de::Error>(value: &str) -> Result<u64, E> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(E::custom("expected canonical unsigned decimal string"));
    }
    value.parse().map_err(E::custom)
}

impl Serialize for RuntimeValuePathSegment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if !serializer.is_human_readable() {
            let segment = match *self {
                Self::TupleElement(index) => NonHumanPathSegmentRef::TupleElement { index },
                Self::SequenceElement(index) => NonHumanPathSegmentRef::SequenceElement { index },
                Self::TupleColumn(index) => NonHumanPathSegmentRef::TupleColumn { index },
                Self::RecordField(field) => NonHumanPathSegmentRef::RecordField { field },
                Self::RecordColumn(field) => NonHumanPathSegmentRef::RecordColumn { field },
                Self::NominalRecordField(field) => {
                    NonHumanPathSegmentRef::NominalRecordField { field }
                }
                Self::FunctionCapture(capture) => {
                    NonHumanPathSegmentRef::FunctionCapture { capture }
                }
                Self::VariantPayload => NonHumanPathSegmentRef::VariantPayload,
                Self::IteratorRemainder(index) => {
                    NonHumanPathSegmentRef::IteratorRemainder { index }
                }
                Self::IteratorWitnessState => NonHumanPathSegmentRef::IteratorWitnessState,
                Self::OpaquePayload => NonHumanPathSegmentRef::OpaquePayload {},
                Self::ReductionState => NonHumanPathSegmentRef::ReductionState,
                Self::ReductionCommandPayload(index) => {
                    NonHumanPathSegmentRef::ReductionCommandPayload { index }
                }
                Self::AgentEmbeddedValue(index) => {
                    NonHumanPathSegmentRef::AgentEmbeddedValue { index }
                }
            };
            return segment.serialize(serializer);
        }
        let human = match *self {
            Self::TupleElement(index) => HumanPathSegmentRef::TupleElement { index },
            Self::SequenceElement(index) => HumanPathSegmentRef::SequenceElement {
                index: index.to_string(),
            },
            Self::TupleColumn(index) => HumanPathSegmentRef::TupleColumn { index },
            Self::RecordField(field) => HumanPathSegmentRef::RecordField { field },
            Self::RecordColumn(field) => HumanPathSegmentRef::RecordColumn { field },
            Self::NominalRecordField(field) => HumanPathSegmentRef::NominalRecordField { field },
            Self::FunctionCapture(capture) => HumanPathSegmentRef::FunctionCapture { capture },
            Self::VariantPayload => HumanPathSegmentRef::VariantPayload,
            Self::IteratorRemainder(index) => HumanPathSegmentRef::IteratorRemainder {
                index: index.to_string(),
            },
            Self::IteratorWitnessState => HumanPathSegmentRef::IteratorWitnessState,
            Self::OpaquePayload => HumanPathSegmentRef::OpaquePayload {},
            Self::ReductionState => HumanPathSegmentRef::ReductionState,
            Self::ReductionCommandPayload(index) => {
                HumanPathSegmentRef::ReductionCommandPayload { index }
            }
            Self::AgentEmbeddedValue(index) => HumanPathSegmentRef::AgentEmbeddedValue { index },
        };
        human.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimeValuePathSegment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return Ok(match NonHumanPathSegment::deserialize(deserializer)? {
                NonHumanPathSegment::TupleElement { index } => Self::TupleElement(index),
                NonHumanPathSegment::SequenceElement { index } => Self::SequenceElement(index),
                NonHumanPathSegment::TupleColumn { index } => Self::TupleColumn(index),
                NonHumanPathSegment::RecordField { field } => Self::RecordField(field),
                NonHumanPathSegment::RecordColumn { field } => Self::RecordColumn(field),
                NonHumanPathSegment::NominalRecordField { field } => {
                    Self::NominalRecordField(field)
                }
                NonHumanPathSegment::FunctionCapture { capture } => Self::FunctionCapture(capture),
                NonHumanPathSegment::VariantPayload => Self::VariantPayload,
                NonHumanPathSegment::IteratorRemainder { index } => Self::IteratorRemainder(index),
                NonHumanPathSegment::IteratorWitnessState => Self::IteratorWitnessState,
                NonHumanPathSegment::OpaquePayload {} => Self::OpaquePayload,
                NonHumanPathSegment::ReductionState => Self::ReductionState,
                NonHumanPathSegment::ReductionCommandPayload { index } => {
                    Self::ReductionCommandPayload(index)
                }
                NonHumanPathSegment::AgentEmbeddedValue { index } => {
                    Self::AgentEmbeddedValue(index)
                }
            });
        }
        Ok(match HumanPathSegment::deserialize(deserializer)? {
            HumanPathSegment::TupleElement { index } => Self::TupleElement(index),
            HumanPathSegment::SequenceElement { index } => {
                Self::SequenceElement(parse_canonical_u64(&index)?)
            }
            HumanPathSegment::TupleColumn { index } => Self::TupleColumn(index),
            HumanPathSegment::RecordField { field } => Self::RecordField(field),
            HumanPathSegment::RecordColumn { field } => Self::RecordColumn(field),
            HumanPathSegment::NominalRecordField { field } => Self::NominalRecordField(field),
            HumanPathSegment::FunctionCapture { capture } => Self::FunctionCapture(capture),
            HumanPathSegment::VariantPayload => Self::VariantPayload,
            HumanPathSegment::IteratorRemainder { index } => {
                Self::IteratorRemainder(parse_canonical_u64(&index)?)
            }
            HumanPathSegment::IteratorWitnessState => Self::IteratorWitnessState,
            HumanPathSegment::OpaquePayload {} => Self::OpaquePayload,
            HumanPathSegment::ReductionState => Self::ReductionState,
            HumanPathSegment::ReductionCommandPayload { index } => {
                Self::ReductionCommandPayload(index)
            }
            HumanPathSegment::AgentEmbeddedValue { index } => Self::AgentEmbeddedValue(index),
        })
    }
}

impl Serialize for RuntimeValuePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimeValuePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let segments = Vec::<RuntimeValuePathSegment>::deserialize(deserializer)?;
        Self::try_from_segments(segments).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_field(value: u32) -> RuntimeRecordFieldId {
        serde_json::from_str(&value.to_string()).unwrap()
    }

    #[test]
    fn path_order_is_manual_lexicographic_and_prefix_first() {
        let paths = [
            RuntimeValuePath::root(),
            RuntimeValuePath::try_from_segments([RuntimeValuePathSegment::TupleElement(0)])
                .unwrap(),
            RuntimeValuePath::try_from_segments([
                RuntimeValuePathSegment::TupleElement(0),
                RuntimeValuePathSegment::VariantPayload,
            ])
            .unwrap(),
            RuntimeValuePath::try_from_segments([RuntimeValuePathSegment::SequenceElement(0)])
                .unwrap(),
            RuntimeValuePath::try_from_segments([RuntimeValuePathSegment::RecordField(
                record_field(1),
            )])
            .unwrap(),
            RuntimeValuePath::try_from_segments([RuntimeValuePathSegment::OpaquePayload]).unwrap(),
        ];
        assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn path_depth_accepts_64_and_rejects_65() {
        let exact = RuntimeValuePath::try_from_segments(std::iter::repeat_n(
            RuntimeValuePathSegment::VariantPayload,
            64,
        ))
        .unwrap();
        assert_eq!(exact.segments().len(), 64);
        assert!(matches!(
            exact.child(RuntimeValuePathSegment::VariantPayload),
            Err(RuntimeValuePathError::TooDeep { actual: 65, .. })
        ));
    }

    #[test]
    fn path_json_uses_typed_fields_and_decimal_u64_strings() {
        let path = RuntimeValuePath::try_from_segments([
            RuntimeValuePathSegment::RecordField(record_field(2)),
            RuntimeValuePathSegment::SequenceElement(4),
            RuntimeValuePathSegment::VariantPayload,
            RuntimeValuePathSegment::OpaquePayload,
        ])
        .unwrap();
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(
            json,
            r#"[{"kind":"record_field","field":2},{"kind":"sequence_element","index":"4"},{"kind":"variant_payload"},{"kind":"opaque_payload"}]"#
        );
        assert_eq!(
            serde_json::from_str::<RuntimeValuePath>(&json).unwrap(),
            path
        );
        assert!(
            serde_json::from_str::<RuntimeValuePath>(r#"[{"kind":"sequence_element","index":4}]"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<RuntimeValuePath>(
                r#"[{"kind":"sequence_element","index":"04"}]"#
            )
            .is_err()
        );
        for invalid in [
            r#"[{"kind":"record_field","field":0}]"#,
            r#"[{"kind":"sequence_element","index":"4","extra":true}]"#,
            r#"[{"kind":"variant_payload","kind":"variant_payload"}]"#,
            r#"[{"kind":"opaque_payload","extra":true}]"#,
            r#"[{"kind":"unknown"}]"#,
        ] {
            assert!(serde_json::from_str::<RuntimeValuePath>(invalid).is_err());
        }
    }
}
