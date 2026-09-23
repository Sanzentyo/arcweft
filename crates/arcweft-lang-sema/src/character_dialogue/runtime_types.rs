//! Exact authored role declarations and their accepted-world projections.

use arcweft_core::{
    pattern::{RuntimeCheckedType, RuntimeOpaqueTypeOwner},
    value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass},
};
use arcweft_interaction_model::dialogue::CharacterDialogueRuntimeRole as Role;
use thiserror::Error;

use crate::{
    env::{
        TypeCheckEnv,
        nominal::{
            AcceptedNominalId, AcceptedNominalInstantiationError, AcceptedNominalOrigin,
            AcceptedNominalRecord, AcceptedNominalSemantics, standard_nominal_id,
        },
    },
    registration::{AcceptedNominalWorld, AcceptedNominalWorldStamp},
    types::{EntityKind, GenericScopeError, TypeKind},
};

/// An authored role's semantic and executable projections from one declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeRoleDeclaration {
    role: Role,
    semantic_type: TypeKind,
    checked_type: RuntimeCheckedType,
}

impl CharacterDialogueRuntimeRoleDeclaration {
    pub const fn role(&self) -> Role {
        self.role
    }
    pub const fn semantic_type(&self) -> &TypeKind {
        &self.semantic_type
    }
    pub const fn checked_type(&self) -> &RuntimeCheckedType {
        &self.checked_type
    }
}

/// Complete role inventory issued atomically from the accepted nominal world.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeRoleRegistry {
    world: AcceptedNominalWorldStamp,
    declarations: [CharacterDialogueRuntimeRoleDeclaration; 6],
    style_semantic: TypeKind,
    style_checked: RuntimeCheckedType,
    semantic_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CharacterDialogueRoleDeclarationMismatch {
    Arity,
    Origin,
    Semantics,
    Producer,
    ValueClass,
    Persistence,
}

#[derive(Clone, Debug, Eq, Error, Ord, PartialEq, PartialOrd)]
pub enum CharacterDialogueRuntimeRoleError {
    #[error("CharacterDialogue role {role:?} has no exact standard declaration {id:?}")]
    Missing { role: Role, id: AcceptedNominalId },
    #[error("CharacterDialogue role {role:?} has an invalid standard declaration: {mismatch:?}")]
    Invalid {
        role: Role,
        mismatch: CharacterDialogueRoleDeclarationMismatch,
    },
    #[error("CharacterDialogue role {role:?} cannot be instantiated: {source}")]
    Instantiation {
        role: Role,
        source: AcceptedNominalInstantiationError,
    },
    #[error(transparent)]
    GenericScope(#[from] GenericScopeError),
}

impl CharacterDialogueRuntimeRoleRegistry {
    pub const fn world(&self) -> &AcceptedNominalWorldStamp {
        &self.world
    }
    pub const fn semantic_digest(&self) -> &[u8; 32] {
        &self.semantic_digest
    }
    pub const fn declarations(&self) -> &[CharacterDialogueRuntimeRoleDeclaration; 6] {
        &self.declarations
    }

    pub fn declaration(&self, role: Role) -> Option<&CharacterDialogueRuntimeRoleDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.role == role)
    }

    pub fn semantic_type(&self, role: Role) -> &TypeKind {
        if role == Role::Style {
            &self.style_semantic
        } else {
            &self
                .declaration(role)
                .expect("all authored roles were published atomically")
                .semantic_type
        }
    }

    pub fn checked_type(&self, role: Role) -> &RuntimeCheckedType {
        if role == Role::Style {
            &self.style_checked
        } else {
            &self
                .declaration(role)
                .expect("all authored roles were published atomically")
                .checked_type
        }
    }

    /// Forward declaration metadata; there is no reverse source-name recognizer.
    fn declaration_id(role: Role) -> Option<AcceptedNominalId> {
        let name = match role {
            Role::Stage => "DialogueStage",
            Role::Portrait => "DialoguePortrait",
            Role::Focus => "DialogueFocus",
            Role::Cleanup => "DialogueCleanup",
            Role::Hook => "DialogueHook",
            Role::RichText => "RichTextStyle",
            Role::Style => return None,
        };
        Some(standard_nominal_id(name))
    }

    pub(crate) fn try_project(
        world: &AcceptedNominalWorld,
    ) -> Result<Self, CharacterDialogueRuntimeRoleError> {
        use CharacterDialogueRoleDeclarationMismatch as Mismatch;
        use CharacterDialogueRuntimeRoleError as RoleError;
        let producer = arcweft_dialogue::CharacterDialogueRuntimeSchema::opaque_type_producer();
        let declarations = Role::AUTHORED_BASE
            .into_iter()
            .map(|role| {
                let id = Self::declaration_id(role).expect("authored roles have a declaration");
                let row = world
                    .nominal_catalog()
                    .exact(id.canonical_path())
                    .filter(|row| row.id() == &id)
                    .ok_or_else(|| RoleError::Missing { role, id })?;
                let mismatch = if row.arity() != 0 {
                    Some(Mismatch::Arity)
                } else if row.origin() != AcceptedNominalOrigin::Domain {
                    Some(Mismatch::Origin)
                } else {
                    None
                };
                if let Some(mismatch) = mismatch {
                    return Err(RoleError::Invalid { role, mismatch });
                }
                let AcceptedNominalSemantics::Opaque(carrier) = row.semantics() else {
                    return Err(RoleError::Invalid {
                        role,
                        mismatch: Mismatch::Semantics,
                    });
                };
                let mismatch = if carrier.producer() != &producer {
                    Some(Mismatch::Producer)
                } else if carrier.value_class() != RuntimeOpaqueValueClass::Plain {
                    Some(Mismatch::ValueClass)
                } else if carrier.persistence() != RuntimeOpaquePersistence::ConstantAndSnapshot {
                    Some(Mismatch::Persistence)
                } else {
                    None
                };
                if let Some(mismatch) = mismatch {
                    return Err(RoleError::Invalid { role, mismatch });
                }
                let semantic_type = row
                    .try_instantiate(Box::<[TypeKind]>::default())
                    .map_err(|source| RoleError::Instantiation { role, source })?;
                let identity = semantic_type.semantic_identity_digest()?.into();
                let checked_type = RuntimeCheckedType::Opaque {
                    owner: RuntimeOpaqueTypeOwner::exact_with(
                        carrier.producer().clone(),
                        identity,
                        carrier.value_class(),
                        carrier.persistence(),
                    ),
                };
                Ok(CharacterDialogueRuntimeRoleDeclaration {
                    role,
                    semantic_type,
                    checked_type,
                })
            })
            .collect::<Result<Vec<_>, RoleError>>()?;
        let declarations: [CharacterDialogueRuntimeRoleDeclaration; 6] = declarations
            .try_into()
            .expect("six authored roles were projected");
        let rich_text = declarations
            .iter()
            .find(|row| row.role == Role::RichText)
            .expect("RichText is an authored role");
        let style_semantic = TypeKind::Choice(vec![
            TypeKind::entity_ref(EntityKind::Style),
            rich_text.semantic_type.clone(),
        ]);
        let style_checked = RuntimeCheckedType::Choice(vec![
            RuntimeCheckedType::EntityReference,
            rich_text.checked_type.clone(),
        ]);
        let mut digest = blake3::Hasher::new();
        digest.update(b"arcweft.character-dialogue-role-registry.v1\0");
        for row in &declarations {
            digest.update(&[row.role.canonical_tag()]);
            digest.update(row.semantic_type.semantic_identity_digest()?.as_bytes());
            digest.update(row.checked_type.semantic_identity_digest().as_bytes());
        }
        digest.update(&[Role::Style.canonical_tag()]);
        digest.update(style_semantic.semantic_identity_digest()?.as_bytes());
        digest.update(style_checked.semantic_identity_digest().as_bytes());
        Ok(Self {
            world: world.stamp(),
            declarations,
            style_semantic,
            style_checked,
            semantic_digest: digest.finalize().into(),
        })
    }
}

impl TypeCheckEnv {
    pub(crate) fn with_standard_character_dialogue_roles(self) -> Self {
        let producer = arcweft_dialogue::CharacterDialogueRuntimeSchema::opaque_type_producer();
        Role::AUTHORED_BASE
            .into_iter()
            .fold(self, |environment, role| {
                let row = AcceptedNominalRecord::try_new_opaque(
                    CharacterDialogueRuntimeRoleRegistry::declaration_id(role)
                        .expect("authored role has a declaration"),
                    0,
                    producer.clone(),
                    RuntimeOpaqueValueClass::Plain,
                    RuntimeOpaquePersistence::ConstantAndSnapshot,
                    AcceptedNominalOrigin::Domain,
                    None,
                )
                .expect("standard role declarations have canonical metadata");
                environment
                    .try_with_nominal_record(row)
                    .expect("standard authored roles have distinct paths")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_authored_roles_and_live_occurrence_stage_have_distinct_authorities() {
        let environment = TypeCheckEnv::standard();
        for role in Role::AUTHORED_BASE {
            let id = CharacterDialogueRuntimeRoleRegistry::declaration_id(role).unwrap();
            let row = environment
                .nominal_catalog()
                .exact(id.canonical_path())
                .unwrap();
            assert_eq!(row.id(), &id);
            assert_eq!(row.origin(), AcceptedNominalOrigin::Domain);
            let ty = row.try_instantiate(Box::<[TypeKind]>::default()).unwrap();
            assert!(matches!(ty, TypeKind::AcceptedNominal(_)));
            let AcceptedNominalSemantics::Opaque(carrier) = row.semantics() else {
                panic!("authored role is an exact opaque type")
            };
            assert_eq!(
                carrier.producer(),
                &arcweft_dialogue::CharacterDialogueRuntimeSchema::opaque_type_producer()
            );
            assert_eq!(
                carrier.persistence(),
                RuntimeOpaquePersistence::ConstantAndSnapshot
            );
        }
        let authored = CharacterDialogueRuntimeRoleRegistry::declaration_id(Role::Stage).unwrap();
        let occurrence = standard_nominal_id(
            arcweft_core::value::RuntimeDialogueOpaqueRole::Stage.standard_type_name(),
        );
        let authored = environment
            .nominal_catalog()
            .exact(authored.canonical_path())
            .unwrap();
        let occurrence = environment
            .nominal_catalog()
            .exact(occurrence.canonical_path())
            .unwrap();
        assert_ne!(authored.id(), occurrence.id());
        assert_eq!(
            occurrence.runtime_carrier().unwrap().persistence(),
            RuntimeOpaquePersistence::SnapshotOnly
        );
        assert_ne!(
            authored
                .try_instantiate(Box::<[TypeKind]>::default())
                .unwrap()
                .semantic_identity_digest()
                .unwrap(),
            occurrence
                .try_instantiate(Box::<[TypeKind]>::default())
                .unwrap()
                .semantic_identity_digest()
                .unwrap()
        );
    }
}
