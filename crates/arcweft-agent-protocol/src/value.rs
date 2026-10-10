use crate::ids::{IdentifierError, PublicId};
use arcweft_core::value::{RuntimeEntityReference, RuntimeValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Deterministic wire value used by Agent host requests and debug records.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AgentValue {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    String(String),
    Entity(PublicId),
    List(Vec<Self>),
    Map(BTreeMap<String, Self>),
}

/// Failure to project evaluated runtime data into the closed Agent value grammar.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AgentValueProjectionError {
    #[error("an affine runtime value cannot be copied into an Agent observation")]
    AffineValue,
    #[error("integer is out of the Agent i64 range")]
    SignedIntegerOutOfRange,
    #[error("integer is out of the Agent u64 range")]
    UnsignedIntegerOutOfRange,
    #[error("Agent numeric values must be finite")]
    NonFiniteNumber,
    #[error("runtime value has no Agent protocol representation")]
    UnsupportedRuntimeValue,
    #[error(transparent)]
    EntityId(#[from] IdentifierError),
}

impl TryFrom<&RuntimeValue> for AgentValue {
    type Error = AgentValueProjectionError;

    fn try_from(value: &RuntimeValue) -> Result<Self, Self::Error> {
        if !value.ownership().permits_copy() {
            return Err(AgentValueProjectionError::AffineValue);
        }
        Self::project_unrestricted(value)
    }
}

impl TryFrom<&RuntimeEntityReference> for AgentValue {
    type Error = AgentValueProjectionError;

    fn try_from(value: &RuntimeEntityReference) -> Result<Self, Self::Error> {
        // The variant and its admitted owner determine this public observation
        // identity. A label never selects a runtime declaration or capability.
        let id = match value {
            RuntimeEntityReference::Project { public_id, .. } => public_id.as_str().to_owned(),
            RuntimeEntityReference::StructuralFlow(flow) => flow.public_label().as_str().to_owned(),
            RuntimeEntityReference::ImportedProject(entity) => {
                entity.public_id().as_str().to_owned()
            }
            RuntimeEntityReference::DialogueLine(line) => line.public_label().as_str().to_owned(),
            RuntimeEntityReference::CharacterLook { character, look } => {
                format!("{character}.look.{}", look.as_str())
            }
        };
        PublicId::new(id).map(Self::Entity).map_err(Into::into)
    }
}

impl AgentValue {
    fn project_unrestricted(value: &RuntimeValue) -> Result<Self, AgentValueProjectionError> {
        match value {
            RuntimeValue::Unit => Ok(Self::Null),
            RuntimeValue::Bool(value) => Ok(Self::Bool(*value)),
            RuntimeValue::Int(value) => value
                .try_into_i64()
                .map(Self::I64)
                .ok_or(AgentValueProjectionError::SignedIntegerOutOfRange),
            RuntimeValue::UInt(value) => value
                .try_into_u64()
                .map(Self::U64)
                .ok_or(AgentValueProjectionError::UnsignedIntegerOutOfRange),
            RuntimeValue::F32(value) if value.is_finite() => Ok(Self::F64(f64::from(*value))),
            RuntimeValue::F64(value) if value.is_finite() => Ok(Self::F64(*value)),
            RuntimeValue::F32(_) | RuntimeValue::F64(_) => {
                Err(AgentValueProjectionError::NonFiniteNumber)
            }
            RuntimeValue::String(value) => Ok(Self::String(value.clone())),

            RuntimeValue::EntityRef(value) => Self::try_from(value),
            RuntimeValue::Tuple(values) => values
                .iter()
                .map(Self::project_unrestricted)
                .collect::<Result<Vec<_>, _>>()
                .map(Self::List),
            RuntimeValue::Seq(values) => {
                // value_at projects one existing logical row. The complete
                // graph's copy proof was checked before reaching this loop.
                (0..values.len())
                    .map(|index| Self::project_unrestricted(&values.value_at(index)))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Self::List)
            }
            RuntimeValue::Record(fields) => fields
                .iter()
                .map(|field| {
                    Self::project_unrestricted(field.value())
                        .map(|value| (field.name().to_owned(), value))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()
                .map(Self::Map),
            _ => Err(AgentValueProjectionError::UnsupportedRuntimeValue),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::plan::FlowRuntimeId;
    use arcweft_core::value::{RuntimeImportedProjectEntityReference, RuntimeSeq};

    #[test]
    fn runtime_projection_preserves_entities_and_literal_strings_in_nested_values() {
        let flow = FlowRuntimeId::from_checked_declaration_digest([7; 32], "flow.opening").unwrap();
        assert_ne!(flow.canonical_label(), "flow.opening");
        let imported = RuntimeImportedProjectEntityReference::try_new(
            arcweft_id::ProjectEntityReferenceFamily::Flow,
            arcweft_id::PublicId::try_new("flow.opening").unwrap(),
            [11; 32],
            [12; 32],
            [13; 32],
            Some(flow.clone()),
        )
        .unwrap();
        let values = RuntimeValue::Seq(RuntimeSeq::values(vec![
            RuntimeValue::EntityRef(RuntimeEntityReference::StructuralFlow(flow.clone())),
            RuntimeValue::EntityRef(RuntimeEntityReference::ImportedProject(imported.clone())),
            RuntimeValue::String("@flow.opening".to_owned()),
            RuntimeValue::String("flow.opening".to_owned()),
            RuntimeValue::String("true".to_owned()),
            RuntimeValue::String("42".to_owned()),
            RuntimeValue::Bool(true),
            RuntimeValue::i64(42),
            RuntimeValue::u64(u64::MAX),
            RuntimeValue::F32(0.5),
        ]));
        let projected = AgentValue::try_from(&values).unwrap();
        let wire = serde_json::to_value(&projected).unwrap();
        for index in [0, 1] {
            assert_eq!(
                wire["value"][index],
                serde_json::json!({"kind": "entity", "value": "flow.opening"})
            );
        }
        for (index, text) in [
            (2, "@flow.opening"),
            (3, "flow.opening"),
            (4, "true"),
            (5, "42"),
        ] {
            assert_eq!(
                wire["value"][index],
                serde_json::json!({"kind": "string", "value": text})
            );
        }
        assert_eq!(
            wire["value"][6],
            serde_json::json!({"kind": "bool", "value": true})
        );
        assert_eq!(
            wire["value"][7],
            serde_json::json!({"kind": "i64", "value": 42})
        );
        assert_eq!(
            wire["value"][8],
            serde_json::json!({"kind": "u64", "value": u64::MAX})
        );
        assert_eq!(
            wire["value"][9],
            serde_json::json!({"kind": "f64", "value": 0.5})
        );
        assert_eq!(
            serde_json::from_value::<AgentValue>(wire).unwrap(),
            projected
        );
        let RuntimeValue::Seq(rows) = values else {
            unreachable!()
        };
        assert_eq!(
            rows.value_at(0),
            RuntimeValue::EntityRef(RuntimeEntityReference::StructuralFlow(flow))
        );
        let RuntimeValue::EntityRef(RuntimeEntityReference::ImportedProject(retained)) =
            rows.value_at(1)
        else {
            panic!("the original imported owner remains retained");
        };
        assert_eq!(retained.target_generation(), &[11; 32]);
        assert_eq!(retained.semantic_identity(), &[12; 32]);
        assert_eq!(retained.value_type(), &[13; 32]);
        assert_eq!(retained, imported);
    }

    #[test]
    fn runtime_projection_refuses_non_finite_and_out_of_range_children() {
        assert_eq!(
            AgentValue::try_from(&RuntimeValue::Tuple(vec![
                RuntimeValue::Bool(true),
                RuntimeValue::F64(f64::NAN)
            ])),
            Err(AgentValueProjectionError::NonFiniteNumber),
        );
        assert_eq!(
            AgentValue::try_from(&RuntimeValue::i128(i128::MAX)),
            Err(AgentValueProjectionError::SignedIntegerOutOfRange),
        );
        assert_eq!(
            AgentValue::try_from(&RuntimeValue::u128(u128::from(u64::MAX))),
            Ok(AgentValue::U64(u64::MAX)),
        );
        assert_eq!(
            AgentValue::try_from(&RuntimeValue::usize(u64::MAX)),
            Ok(AgentValue::U64(u64::MAX)),
        );
        assert_eq!(
            AgentValue::try_from(&RuntimeValue::u128(u128::MAX)),
            Err(AgentValueProjectionError::UnsignedIntegerOutOfRange),
        );
    }

    #[test]
    fn runtime_projection_normalizes_dense_and_columnar_sequences_through_the_same_grammar() {
        let flow = FlowRuntimeId::from_checked_declaration_digest([7; 32], "flow.opening").unwrap();
        let references = RuntimeSeq::dense_entity_refs(vec![
            RuntimeEntityReference::StructuralFlow(flow.clone()),
            RuntimeEntityReference::StructuralFlow(flow),
        ]);
        let rows = RuntimeSeq::record_columns(
            2,
            vec![
                ("flow".to_owned(), references),
                (
                    "ready".to_owned(),
                    RuntimeSeq::dense_bool(vec![true, false]),
                ),
                ("count".to_owned(), RuntimeSeq::dense_u64(vec![1, u64::MAX])),
            ],
        )
        .unwrap();
        let ordinary = RuntimeValue::Seq(RuntimeSeq::values(vec![
            RuntimeValue::try_record(vec![
                (
                    "flow".to_owned(),
                    RuntimeValue::EntityRef(RuntimeEntityReference::StructuralFlow(
                        FlowRuntimeId::from_checked_declaration_digest([7; 32], "flow.opening")
                            .unwrap(),
                    )),
                ),
                ("ready".to_owned(), RuntimeValue::Bool(true)),
                ("count".to_owned(), RuntimeValue::u64(1)),
            ])
            .unwrap(),
            RuntimeValue::try_record(vec![
                (
                    "flow".to_owned(),
                    RuntimeValue::EntityRef(RuntimeEntityReference::StructuralFlow(
                        FlowRuntimeId::from_checked_declaration_digest([7; 32], "flow.opening")
                            .unwrap(),
                    )),
                ),
                ("ready".to_owned(), RuntimeValue::Bool(false)),
                ("count".to_owned(), RuntimeValue::u64(u64::MAX)),
            ])
            .unwrap(),
        ]));
        assert_eq!(
            AgentValue::try_from(&RuntimeValue::Seq(rows)).unwrap(),
            AgentValue::try_from(&ordinary).unwrap()
        );
        assert_eq!(
            AgentValue::try_from(&RuntimeValue::Seq(RuntimeSeq::dense_f64(vec![
                0.5,
                f64::INFINITY
            ]))),
            Err(AgentValueProjectionError::NonFiniteNumber),
        );
    }

    #[test]
    fn runtime_projection_uses_the_same_public_line_identity_for_local_and_imported_owners() {
        let local = RuntimeEntityReference::DialogueLine(
            arcweft_core::plan::RuntimeLineId::from_source_entity_body("say.opening").unwrap(),
        );
        let imported = RuntimeEntityReference::ImportedProject(
            RuntimeImportedProjectEntityReference::try_new(
                arcweft_id::ProjectEntityReferenceFamily::DialogueLine,
                arcweft_id::PublicId::try_new("say.opening").unwrap(),
                [11; 32],
                [12; 32],
                [13; 32],
                None,
            )
            .unwrap(),
        );
        assert_eq!(local, imported);
        let expected = AgentValue::Entity(PublicId::new("say.opening").unwrap());
        assert_eq!(AgentValue::try_from(&local).unwrap(), expected);
        assert_eq!(AgentValue::try_from(&imported).unwrap(), expected);
    }
}
