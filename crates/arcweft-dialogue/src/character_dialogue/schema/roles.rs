//! References from configuration roles into the active executable type table.

use arcweft_core::{
    pattern::{
        RuntimeCheckedType, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner,
        RuntimeSemanticTypeId,
    },
    program_types::RuntimeProgramTypes,
    value::{
        RuntimeEntityReference, RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeValue,
    },
};
use arcweft_id::DeclarationIdentityFamily;

use super::super::{CharacterDialogueRuntimeRole as Role, CharacterDialogueValueError};
use super::CharacterDialogueRuntimeSchema;

/// Source identities for one authored opaque role and its producer-owned body.
/// Both refer to the selected program; neither is an independent type schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeRoleType {
    value: RuntimeSemanticTypeId,
    payload: RuntimeSemanticTypeId,
}

impl CharacterDialogueRuntimeRoleType {
    pub const fn new(value: RuntimeSemanticTypeId, payload: RuntimeSemanticTypeId) -> Self {
        Self { value, payload }
    }
    pub const fn value(self) -> RuntimeSemanticTypeId {
        self.value
    }
    pub const fn payload(self) -> RuntimeSemanticTypeId {
        self.payload
    }
}

/// Complete bindings in `CharacterDialogueRuntimeRole::AUTHORED_BASE` order.
/// Style has the separately projected ordered choice identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeRoleTypes {
    authored: [CharacterDialogueRuntimeRoleType; 6],
    style: RuntimeSemanticTypeId,
}

impl CharacterDialogueRuntimeRoleTypes {
    pub const fn new(
        authored: [CharacterDialogueRuntimeRoleType; 6],
        style: RuntimeSemanticTypeId,
    ) -> Self {
        Self { authored, style }
    }

    pub fn authored(&self, role: Role) -> Option<CharacterDialogueRuntimeRoleType> {
        Role::AUTHORED_BASE
            .iter()
            .position(|candidate| *candidate == role)
            .map(|index| self.authored[index])
    }

    pub fn value_type(&self, role: Role) -> RuntimeSemanticTypeId {
        self.authored(role)
            .map_or(self.style, CharacterDialogueRuntimeRoleType::value)
    }

    pub(super) fn validate(
        &self,
        program: RuntimeProgramTypes<'_>,
    ) -> Result<RuntimeOpaqueTypeOwner, CharacterDialogueValueError> {
        let mut identities = std::collections::BTreeSet::new();
        let mut rich_text = None;
        for role in Role::AUTHORED_BASE {
            let binding = self.authored(role).expect("authored role has a fixed slot");
            let invalid = |reason| CharacterDialogueValueError::RoleType { role, reason };
            if !identities.insert(binding.value) {
                return Err(invalid("authored roles share a semantic identity"));
            }
            let RuntimeCheckedType::Opaque { owner } = program.checked_type(binding.value)? else {
                return Err(invalid("authored role is not opaque"));
            };
            if owner.admission() != RuntimeOpaqueTypeAdmission::ExactIdentity
                || owner.semantic_identity() != binding.value
                || owner.producer() != &CharacterDialogueRuntimeSchema::opaque_type_producer()
                || owner.value_class() != RuntimeOpaqueValueClass::Plain
                || owner.persistence() != RuntimeOpaquePersistence::ConstantAndSnapshot
            {
                return Err(invalid(
                    "authored role has a different exact opaque contract",
                ));
            }
            program.require_type(binding.payload)?;
            if role == Role::RichText {
                rich_text = Some(owner);
            }
        }
        let rich_text = rich_text.expect("complete authored role inventory contains RichText");
        let expected = RuntimeCheckedType::Choice(vec![
            RuntimeCheckedType::EntityReference,
            RuntimeCheckedType::Opaque {
                owner: rich_text.clone(),
            },
        ]);
        if program.checked_type(self.style)? != expected {
            return Err(CharacterDialogueValueError::RoleType {
                role: Role::Style,
                reason: "Style is not the ordered entity-reference/RichText choice",
            });
        }
        Ok(rich_text)
    }
}

impl CharacterDialogueRuntimeSchema {
    pub(super) fn validate_role(
        &self,
        role: Role,
        value: &RuntimeValue,
    ) -> Result<(), CharacterDialogueValueError> {
        self.program_types()
            .accepts_value(self.roles.value_type(role), value, Self::limits())?;
        let authored = if role == Role::Style {
            if let RuntimeValue::EntityRef(reference) = value {
                if !matches!(
                    reference,
                    RuntimeEntityReference::Project {
                        family: DeclarationIdentityFamily::Style,
                        ..
                    }
                ) {
                    return Err(CharacterDialogueValueError::RoleType {
                        role,
                        reason: "Style reference has another entity family",
                    });
                }
                return Ok(());
            }
            Role::RichText
        } else {
            role
        };
        let RuntimeValue::Opaque(value) = value else {
            return Err(CharacterDialogueValueError::RoleType {
                role,
                reason: "authored role value is not opaque",
            });
        };
        let binding = self
            .roles
            .authored(authored)
            .expect("authored role has a fixed slot");
        self.program_types()
            .accepts_value(binding.payload, value.payload(), Self::limits())?;
        Ok(())
    }
}
