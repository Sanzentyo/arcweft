//! Entity-reference type metadata. Payload types describe the declaration's
//! interface; they are not values owned by the inert reference.

use super::RuntimeEntityReference;
use arcweft_id::{ProjectEntityReferenceFamily, PublicId, PublicIdFamilyError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "EntityTypeParts<R>")]
#[serde(bound(deserialize = "R: Deserialize<'de>"))]
pub struct RuntimeProjectEntityReferenceType<R> {
    family: ProjectEntityReferenceFamily,
    payload: Option<R>,
    #[serde(with = "optional_public_id")]
    declaration: Option<PublicId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntityTypeParts<R> {
    family: ProjectEntityReferenceFamily,
    payload: Option<R>,
    #[serde(with = "optional_public_id")]
    declaration: Option<PublicId>,
}

impl<R> TryFrom<EntityTypeParts<R>> for RuntimeProjectEntityReferenceType<R> {
    type Error = PublicIdFamilyError;

    fn try_from(parts: EntityTypeParts<R>) -> Result<Self, Self::Error> {
        Self::try_new(parts.family, parts.payload, parts.declaration)
    }
}

/// Complete inert reference carrier, including Character-owned look selectors.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeEntityReferenceType<R> {
    Project(RuntimeProjectEntityReferenceType<R>),
    CharacterLook(arcweft_character::id::CharacterId),
}

impl<R> RuntimeProjectEntityReferenceType<R> {
    fn try_new(
        family: ProjectEntityReferenceFamily,
        payload: Option<R>,
        declaration: Option<PublicId>,
    ) -> Result<Self, PublicIdFamilyError> {
        if let Some(id) = &declaration {
            family.validate_public_id(id)?;
        }
        Ok(Self {
            family,
            payload,
            declaration,
        })
    }
}

impl<R> RuntimeEntityReferenceType<R> {
    pub const CHARACTER_LOOK_TAG: u8 = 36;

    pub fn kind_tag(&self) -> u8 {
        match self {
            Self::Project(reference) => reference.family.semantic_tag(),
            Self::CharacterLook(_) => Self::CHARACTER_LOOK_TAG,
        }
    }
    pub fn family_only(family: ProjectEntityReferenceFamily) -> Self {
        Self::Project(RuntimeProjectEntityReferenceType {
            family,
            payload: None,
            declaration: None,
        })
    }

    pub fn try_new(
        family: ProjectEntityReferenceFamily,
        payload: Option<R>,
        declaration: Option<PublicId>,
    ) -> Result<Self, PublicIdFamilyError> {
        RuntimeProjectEntityReferenceType::try_new(family, payload, declaration).map(Self::Project)
    }

    pub const fn family(&self) -> Option<ProjectEntityReferenceFamily> {
        match self {
            Self::Project(reference) => Some(reference.family),
            Self::CharacterLook(_) => None,
        }
    }
    pub const fn payload(&self) -> Option<&R> {
        match self {
            Self::Project(reference) => reference.payload.as_ref(),
            Self::CharacterLook(_) => None,
        }
    }
    pub const fn declaration(&self) -> Option<&PublicId> {
        match self {
            Self::Project(reference) => reference.declaration.as_ref(),
            Self::CharacterLook(_) => None,
        }
    }
    pub const fn look_owner(&self) -> Option<&arcweft_character::id::CharacterId> {
        match self {
            Self::CharacterLook(character) => Some(character),
            Self::Project(_) => None,
        }
    }
    pub fn as_ref(&self) -> RuntimeEntityReferenceType<&R> {
        match self {
            Self::Project(reference) => {
                RuntimeEntityReferenceType::Project(RuntimeProjectEntityReferenceType {
                    family: reference.family,
                    payload: reference.payload.as_ref(),
                    declaration: reference.declaration.clone(),
                })
            }
            Self::CharacterLook(character) => {
                RuntimeEntityReferenceType::CharacterLook(character.clone())
            }
        }
    }
    pub fn try_map<T, E>(
        self,
        map: impl FnOnce(R) -> Result<T, E>,
    ) -> Result<RuntimeEntityReferenceType<T>, E> {
        Ok(match self {
            Self::Project(reference) => {
                RuntimeEntityReferenceType::Project(RuntimeProjectEntityReferenceType {
                    family: reference.family,
                    payload: reference.payload.map(map).transpose()?,
                    declaration: reference.declaration,
                })
            }
            Self::CharacterLook(character) => RuntimeEntityReferenceType::CharacterLook(character),
        })
    }
    pub fn map<T>(self, map: impl FnOnce(R) -> T) -> RuntimeEntityReferenceType<T> {
        match self {
            Self::Project(reference) => {
                RuntimeEntityReferenceType::Project(RuntimeProjectEntityReferenceType {
                    family: reference.family,
                    payload: reference.payload.map(map),
                    declaration: reference.declaration,
                })
            }
            Self::CharacterLook(character) => RuntimeEntityReferenceType::CharacterLook(character),
        }
    }
    /// Checks typed identity without interpreting display labels as IDs.
    /// Original-program consumers additionally admit the payload type edge.
    pub fn accepts(&self, value: &RuntimeEntityReference) -> bool {
        let Self::Project(reference) = self else {
            return value
                .character_look()
                .is_some_and(|(character, _)| Some(character) == self.look_owner());
        };
        if value.reference_family() != Some(reference.family) {
            return false;
        }
        reference.declaration.as_ref().is_none_or(|id| match value {
            RuntimeEntityReference::Project { public_id, .. } => id == public_id,
            RuntimeEntityReference::ImportedProject(entity) => id == entity.public_id(),
            RuntimeEntityReference::PresentationAddress(address) => id == address.public_id(),
            RuntimeEntityReference::StructuralFlow(flow) => {
                id.as_str() == flow.public_label_ref().as_str()
            }
            RuntimeEntityReference::DialogueLine(line) => {
                crate::runtime_id::RuntimeIdPath::preflight_source_entity_body(
                    crate::runtime_id::RuntimeIdFamily::Line,
                    id.as_str(),
                    crate::runtime_id::RuntimeIdFamily::Line.source_families(),
                )
                .is_ok_and(|expected| expected.matches_path(line.path()))
            }
            RuntimeEntityReference::CharacterLook { .. } => false,
        })
    }
}
mod optional_public_id {
    use arcweft_id::PublicId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S: Serializer>(
        id: &Option<PublicId>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        id.as_ref().map(PublicId::as_str).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<PublicId>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(PublicId::try_new_engine_owned)
            .transpose()
            .map_err(serde::de::Error::custom)
    }
}

impl RuntimeEntityReference {
    pub fn reference_family(&self) -> Option<ProjectEntityReferenceFamily> {
        Some(match self {
            Self::Project { family, public_id } => {
                if family.validate_public_id(public_id).is_err() {
                    return None;
                }
                ProjectEntityReferenceFamily::from_declaration_family(*family)?
            }
            Self::ImportedProject(entity) => entity.family(),
            Self::PresentationAddress(address) => address.kind().family(),
            Self::StructuralFlow(_) => ProjectEntityReferenceFamily::Flow,
            Self::DialogueLine(_) => ProjectEntityReferenceFamily::DialogueLine,
            Self::CharacterLook { .. } => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{RuntimeSchemaLimits, RuntimeTypeSchema};
    use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
    use crate::plan::RuntimePlanTypeProjection;
    use crate::value::RuntimeValue;
    use arcweft_id::DeclarationIdentityFamily;

    fn action(id: &str) -> RuntimeValue {
        RuntimeValue::EntityRef(
            RuntimeEntityReference::try_project(
                DeclarationIdentityFamily::Action,
                PublicId::try_new(id).unwrap(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn reference_admission_keeps_family_and_original_declaration() {
        let payload = RuntimeSemanticTypeId::from_bytes([21; 32]);
        let reference = RuntimeEntityReferenceType::try_new(
            ProjectEntityReferenceFamily::Action,
            Some(payload),
            Some(PublicId::try_new("action.feedback.submit").unwrap()),
        )
        .unwrap();
        let checked = RuntimeCheckedType::EntityReference(reference.clone());
        let schema = RuntimeTypeSchema::EntityReference(reference.clone());
        let accepted = action("action.feedback.submit");
        assert!(checked.accepts_value(&accepted));
        schema
            .validate_value(&accepted, RuntimeSchemaLimits::engine_default())
            .unwrap();
        for value in [
            action("action.feedback.cancel"),
            RuntimeValue::EntityRef(
                RuntimeEntityReference::try_project(
                    DeclarationIdentityFamily::Character,
                    PublicId::try_new("character.alice").unwrap(),
                )
                .unwrap(),
            ),
            RuntimeValue::EntityRef(RuntimeEntityReference::Project {
                family: DeclarationIdentityFamily::Action,
                public_id: PublicId::try_new("character.alice").unwrap(),
            }),
        ] {
            assert!(!checked.accepts_value(&value));
            assert!(
                schema
                    .validate_value(&value, RuntimeSchemaLimits::engine_default())
                    .is_err()
            );
        }
        let mut json = serde_json::to_value(&reference).unwrap();
        json["Project"]["family"] = serde_json::json!("character");
        assert!(
            serde_json::from_value::<RuntimeEntityReferenceType<RuntimeSemanticTypeId>>(json)
                .is_err()
        );
    }

    #[test]
    fn reference_payload_is_a_type_edge_instead_of_an_owned_value() {
        let reference = RuntimeEntityReferenceType::try_new(
            ProjectEntityReferenceFamily::Action,
            Some(7_u32),
            Some(PublicId::try_new("action.submit").unwrap()),
        )
        .unwrap();
        let projection = RuntimePlanTypeProjection::EntityReference(reference);
        assert_eq!(projection.children().as_ref(), [&7]);
        let mapped = projection.try_map(|child| Ok::<_, ()>(child + 1)).unwrap();
        let RuntimePlanTypeProjection::EntityReference(reference) = mapped else {
            panic!("reference type")
        };
        assert_eq!(reference.payload(), Some(&8));
        assert_eq!(reference.declaration().unwrap().as_str(), "action.submit");
        assert_eq!(
            reference.family(),
            Some(ProjectEntityReferenceFamily::Action)
        );
        let schema = RuntimeTypeSchema::EntityReference(
            reference.map(|_| RuntimeSemanticTypeId::from_bytes([31; 32])),
        );
        let limits = RuntimeSchemaLimits {
            max_depth: 0,
            max_nodes: 1,
            ..RuntimeSchemaLimits::engine_default()
        };
        schema
            .validate_value(&action("action.submit"), limits)
            .unwrap();
    }

    #[test]
    fn character_look_references_keep_their_character_owner() {
        let character = arcweft_character::id::CharacterId::try_new("character.alice").unwrap();
        let reference = RuntimeEntityReferenceType::<()>::CharacterLook(character.clone());
        let look = |character| RuntimeEntityReference::CharacterLook {
            character,
            look: arcweft_character::id::CharacterLookId::try_new("normal").unwrap(),
        };
        assert!(reference.accepts(&look(character)));
        assert!(!reference.accepts(&look(
            arcweft_character::id::CharacterId::try_new("character.bob").unwrap()
        )));
        assert!(
            !RuntimeEntityReferenceType::<()>::family_only(ProjectEntityReferenceFamily::Character)
                .accepts(&look(
                    arcweft_character::id::CharacterId::try_new("character.alice").unwrap()
                ))
        );
    }
}
