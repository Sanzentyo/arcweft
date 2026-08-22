//! Producer-validated opaque runtime values.

use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId};
use crate::value::RuntimeValue;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeHandleKind {
    StageActor,
    Cue,
    Voice,
}

impl RuntimeHandleKind {
    pub fn try_producer(
        self,
    ) -> Result<RuntimeOpaqueTypeProducerId, crate::entry::RuntimeIdentityError> {
        RuntimeOpaqueTypeProducerId::try_new(match self {
            Self::StageActor => "std.line.stage_actor_handle",
            Self::Cue => "std.line.cue_handle",
            Self::Voice => "std.line.voice_handle",
        })
    }

    pub(crate) const fn canonical_tag(self) -> u8 {
        match self {
            Self::StageActor => 1,
            Self::Cue => 2,
            Self::Voice => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RuntimeOpaqueValueClass {
    Plain,
    AffineHandle(RuntimeHandleKind),
}

impl RuntimeOpaqueValueClass {
    /// Stable semantic transcript tag used by catalog and ownership digests.
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Plain => 0,
            Self::AffineHandle(kind) => kind.canonical_tag(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOpaquePersistence {
    ConstantAndSnapshot,
    SnapshotOnly,
}

impl RuntimeOpaquePersistence {
    /// Stable semantic tag used by catalog and ownership digests.
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::ConstantAndSnapshot => 0,
            Self::SnapshotOnly => 1,
        }
    }
}

/// Exact producer evidence and payload for one opaque runtime value.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeOpaqueValue {
    producer: RuntimeOpaqueTypeProducerId,
    semantic_identity: RuntimeSemanticTypeId,
    value_class: RuntimeOpaqueValueClass,
    persistence: RuntimeOpaquePersistence,
    payload: Box<RuntimeValue>,
}

impl RuntimeOpaqueValue {
    pub(crate) fn new_exact(owner: &RuntimeOpaqueTypeOwner, payload: RuntimeValue) -> Self {
        Self {
            producer: owner.producer().clone(),
            semantic_identity: owner.semantic_identity(),
            value_class: owner.value_class(),
            persistence: owner.persistence(),
            payload: Box::new(payload),
        }
    }

    #[must_use]
    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId {
        &self.producer
    }

    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }

    #[must_use]
    pub const fn value_class(&self) -> RuntimeOpaqueValueClass {
        self.value_class
    }

    #[must_use]
    pub const fn persistence(&self) -> RuntimeOpaquePersistence {
        self.persistence
    }

    #[must_use]
    pub const fn payload(&self) -> &RuntimeValue {
        &self.payload
    }

    #[must_use]
    pub fn into_payload(self) -> RuntimeValue {
        *self.payload
    }
}

/// Failure to construct a concrete opaque runtime value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeOpaqueValueError {
    #[error("producer-wide opaque type is not a concrete runtime value owner")]
    NonConcreteOwner {
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::awbc::schema::AwbcFunctionId;
    use crate::entry::RuntimeSchemaError;
    use crate::pattern::{RuntimeCheckedType, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner};
    use crate::value::{
        AwbcRuntimeValueSnapshot, RuntimeFunctionValue, RuntimeSeq, RuntimeValueNestingError,
    };

    fn producer(value: &str) -> RuntimeOpaqueTypeProducerId {
        RuntimeOpaqueTypeProducerId::try_new(value).expect("valid producer")
    }

    fn exact(producer: &str, identity: u8) -> RuntimeOpaqueTypeOwner {
        RuntimeOpaqueTypeOwner::exact(
            self::producer(producer),
            RuntimeSemanticTypeId::from_bytes([identity; 32]),
        )
    }

    #[test]
    fn owner_assignability_is_exact_or_expected_producer_wide() {
        let exact_a = exact("std.test", 1);
        let exact_b = exact("std.test", 2);
        let other = exact("std.other", 1);
        let wide = RuntimeOpaqueTypeOwner::producer_wide(
            producer("std.test"),
            RuntimeSemanticTypeId::from_bytes([9; 32]),
        );
        let other_wide = RuntimeOpaqueTypeOwner::producer_wide(
            producer("std.test"),
            RuntimeSemanticTypeId::from_bytes([8; 32]),
        );

        assert!(exact_a.accepts_owner(&exact_a));
        assert!(!exact_a.accepts_owner(&exact_b));
        assert!(!exact_a.accepts_owner(&other));
        assert!(wide.accepts_owner(&exact_a));
        assert!(!wide.accepts_owner(&other));
        assert!(wide.accepts_owner(&wide));
        assert!(!wide.accepts_owner(&other_wide));
    }

    #[test]
    fn only_exact_owner_wraps_and_checked_acceptance_is_fail_closed() {
        let exact_a = exact("std.test", 1);
        let exact_b = exact("std.test", 2);
        let other = exact("std.other", 1);
        let wide = RuntimeOpaqueTypeOwner::producer_wide(
            producer("std.test"),
            RuntimeSemanticTypeId::from_bytes([9; 32]),
        );

        assert_eq!(
            wide.try_wrap(RuntimeValue::Unit),
            Err(RuntimeOpaqueValueError::NonConcreteOwner {
                producer: producer("std.test"),
                semantic_identity: RuntimeSemanticTypeId::from_bytes([9; 32]),
            })
        );

        let value = exact_a
            .try_wrap(RuntimeValue::String("payload".to_owned()))
            .expect("exact owner wraps");
        assert!(RuntimeCheckedType::Opaque { owner: exact_a }.accepts_value(&value));
        assert!(RuntimeCheckedType::Opaque { owner: wide }.accepts_value(&value));
        assert!(!RuntimeCheckedType::Opaque { owner: exact_b }.accepts_value(&value));
        assert!(!RuntimeCheckedType::Opaque { owner: other }.accepts_value(&value));
        assert!(
            !RuntimeCheckedType::Opaque {
                owner: exact("std.test", 1),
            }
            .accepts_value(&RuntimeValue::String("payload".to_owned()))
        );
    }

    #[test]
    fn complete_result_owner_accepts_both_exact_opaque_branches() {
        let ok = exact("std.ok", 1);
        let error = exact("std.error", 2);
        let checked = RuntimeCheckedType::Result {
            ok: Box::new(RuntimeCheckedType::Opaque { owner: ok.clone() }),
            error: Box::new(RuntimeCheckedType::Opaque {
                owner: error.clone(),
            }),
        };

        assert!(checked.accepts_value(&RuntimeValue::result_ok(
            ok.try_wrap(RuntimeValue::Unit).expect("ok payload")
        )));
        assert!(
            checked.accepts_value(&RuntimeValue::result_err(
                error
                    .try_wrap(RuntimeValue::String("error".to_owned()))
                    .expect("error payload")
            ))
        );
    }

    #[test]
    fn recursive_composites_accept_only_matching_opaque_evidence() {
        let first = exact("std.first", 1);
        let second = exact("std.second", 2);
        let foreign = exact("std.foreign", 1);
        let first_value = first
            .try_wrap(RuntimeValue::Unit)
            .expect("first exact owner wraps");
        let second_value = second
            .try_wrap(RuntimeValue::String("second".to_owned()))
            .expect("second exact owner wraps");
        let foreign_value = foreign
            .try_wrap(RuntimeValue::Unit)
            .expect("foreign exact owner wraps");

        let option = RuntimeCheckedType::Option(Box::new(RuntimeCheckedType::Opaque {
            owner: first.clone(),
        }));
        assert!(option.accepts_value(&RuntimeValue::option_none()));
        assert!(option.accepts_value(&RuntimeValue::option_some(first_value.clone())));
        assert!(!option.accepts_value(&RuntimeValue::option_some(foreign_value.clone())));
        assert!(
            RuntimeCheckedType::Option(Box::new(RuntimeCheckedType::Never))
                .accepts_value(&RuntimeValue::option_none())
        );
        assert!(
            !RuntimeCheckedType::Option(Box::new(RuntimeCheckedType::Never))
                .accepts_value(&RuntimeValue::option_some(RuntimeValue::Unit))
        );

        let tuple = RuntimeCheckedType::Tuple(vec![
            RuntimeCheckedType::Opaque {
                owner: first.clone(),
            },
            RuntimeCheckedType::Opaque {
                owner: second.clone(),
            },
        ]);
        assert!(tuple.accepts_value(&RuntimeValue::Tuple(vec![
            first_value.clone(),
            second_value.clone(),
        ])));
        assert!(!tuple.accepts_value(&RuntimeValue::Tuple(vec![
            foreign_value.clone(),
            second_value,
        ])));
        assert!(
            RuntimeCheckedType::Tuple(Vec::new()).accepts_value(&RuntimeValue::Tuple(Vec::new()))
        );

        let choice = RuntimeCheckedType::Choice(vec![
            RuntimeCheckedType::Opaque {
                owner: first.clone(),
            },
            RuntimeCheckedType::Opaque {
                owner: second.clone(),
            },
        ]);
        assert!(choice.accepts_value(&first_value));
        assert!(
            choice.accepts_value(
                &second
                    .try_wrap(RuntimeValue::Unit)
                    .expect("second exact owner wraps")
            )
        );
        assert!(!choice.accepts_value(&foreign_value));
        assert!(!RuntimeCheckedType::Choice(Vec::new()).accepts_value(&RuntimeValue::Unit));

        let sequence =
            RuntimeCheckedType::Sequence(Box::new(RuntimeCheckedType::Opaque { owner: first }));
        assert!(sequence.accepts_value(&RuntimeValue::Seq(RuntimeSeq::values(Vec::new()))));
        assert!(
            sequence.accepts_value(&RuntimeValue::Seq(RuntimeSeq::values(vec![
                first_value.clone(),
                first_value,
            ])))
        );
        assert!(
            !sequence.accepts_value(&RuntimeValue::Seq(RuntimeSeq::values(vec![foreign_value,])))
        );
        assert!(!RuntimeCheckedType::Never.accepts_value(&RuntimeValue::Unit));
    }

    #[test]
    fn opaque_value_has_tag_16_and_payload_participates_in_nesting() {
        let owner = exact("std.agent_error", 7);
        let value = owner
            .try_wrap(RuntimeValue::Unit)
            .expect("exact owner wraps");
        let mut expected = vec![16];
        expected.extend_from_slice(&15_u32.to_le_bytes());
        expected.extend_from_slice(b"std.agent_error");
        expected.extend_from_slice(&[7; 32]);
        expected.extend_from_slice(&[0, 0]);
        expected.push(1);

        assert_eq!(
            value.try_canonical_bytes(128).expect("canonical bytes"),
            expected
        );
        assert_eq!(
            value.validate_nesting_depth(0),
            Err(RuntimeValueNestingError::Exceeded { maximum: 0 })
        );
        assert_eq!(value.validate_nesting_depth(1), Ok(()));
        assert_eq!(
            value.ownership(),
            crate::value::ownership::RuntimeValueOwnership::Unrestricted
        );
    }

    #[test]
    fn opaque_carriers_round_trip_without_an_ownerless_form() {
        let owner = exact("std.round_trip", 3);
        let checked = RuntimeCheckedType::Opaque {
            owner: owner.clone(),
        };
        let value = owner
            .try_wrap(RuntimeValue::String("payload".to_owned()))
            .expect("exact owner wraps");

        assert_eq!(
            serde_json::from_str::<RuntimeCheckedType>(
                &serde_json::to_string(&checked).expect("checked type serializes")
            )
            .expect("checked type deserializes"),
            checked
        );
        assert_eq!(
            serde_json::from_str::<RuntimeValue>(
                &serde_json::to_string(&value).expect("opaque value serializes")
            )
            .expect("opaque value deserializes"),
            value
        );
        assert!(
            serde_json::from_value::<RuntimeCheckedType>(serde_json::json!({
                "Opaque": {}
            }))
            .is_err()
        );
        assert!(serde_json::from_str::<RuntimeOpaqueTypeProducerId>(r#""""#).is_err());
        assert!(
            serde_json::from_str::<RuntimeOpaqueTypeProducerId>(r#""bad\u0001producer""#).is_err()
        );
    }

    #[test]
    fn opaque_wrapper_does_not_encode_runtime_only_payloads() {
        let function = RuntimeFunctionValue::new_awbc(Vec::new(), AwbcFunctionId(0), Vec::new());
        let value = exact("std.runtime_only", 4)
            .try_wrap(RuntimeValue::Function(function))
            .expect("exact owner wraps after producer validation");

        assert_eq!(
            value.try_canonical_bytes(1024),
            Err(RuntimeSchemaError::Encoding {
                message: "runtime-only value has no replay/save encoding".to_owned(),
            })
        );
    }

    #[test]
    fn admission_discriminants_are_stable() {
        assert_eq!(RuntimeOpaqueTypeAdmission::ExactIdentity as u8, 0);
        assert_eq!(RuntimeOpaqueTypeAdmission::ProducerWide as u8, 1);
    }

    #[test]
    fn affine_snapshot_only_handle_is_not_a_constant_and_round_trips_in_snapshot() {
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            producer("std.line.cue_handle"),
            RuntimeSemanticTypeId::from_bytes([11; 32]),
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        let value = owner
            .try_wrap(RuntimeValue::UInt(crate::value::RuntimeUInt::U32(9)))
            .expect("exact handle owner wraps");
        let wide = RuntimeOpaqueTypeOwner::producer_wide_with(
            producer("std.line.cue_handle"),
            RuntimeSemanticTypeId::from_bytes([12; 32]),
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        assert!(wide.accepts_owner(&owner));
        assert!(!exact("std.line.cue_handle", 11).accepts_owner(&owner));

        assert_eq!(
            value.ownership(),
            crate::value::ownership::RuntimeValueOwnership::Affine
        );
        assert!(value.contains_nonconstant_opaque());
        assert!(
            RuntimeValue::Tuple(vec![RuntimeValue::Unit, value.clone()])
                .contains_nonconstant_opaque()
        );
        assert_eq!(
            value.try_canonical_bytes(128),
            Err(RuntimeSchemaError::Encoding {
                message: "opaque value class/persistence is not constant-admissible".to_owned(),
            })
        );
        let snapshot = AwbcRuntimeValueSnapshot::from_runtime_value(&value)
            .expect("live handle snapshots explicitly");
        assert_eq!(
            snapshot
                .into_runtime_value()
                .expect("snapshot handle restores"),
            value
        );
        assert!(
            !exact("std.line.cue_handle", 11).accepts_opaque_value(match &value {
                RuntimeValue::Opaque(value) => value,
                _ => unreachable!("owner wrapped opaque value"),
            })
        );
    }
}
