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
use super::role_payload::{CharacterDialogueRolePayloadCodec, CharacterDialogueRolePayloadSchema};

/// Source identity and body-evidence binding for one authored opaque role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeRoleType<T = RuntimeSemanticTypeId> {
    value: T,
    body: CharacterDialogueRuntimeRoleBody<T>,
}

impl<T> CharacterDialogueRuntimeRoleType<T> {
    pub const fn new(value: T, body: CharacterDialogueRuntimeRoleBody<T>) -> Self {
        Self { value, body }
    }

    pub const fn value_ref(&self) -> &T {
        &self.value
    }

    pub const fn body_ref(&self) -> &CharacterDialogueRuntimeRoleBody<T> {
        &self.body
    }
}

impl<T: Copy> CharacterDialogueRuntimeRoleType<T> {
    pub const fn value(self) -> T {
        self.value
    }

    pub const fn body(self) -> CharacterDialogueRuntimeRoleBody<T> {
        self.body
    }
}

/// Evidence that an authored role has a usable payload schema.
///
/// An unbound role still has its total outer opaque identity, but accepts no
/// opaque body value until Dialogue owns a real constructor and validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacterDialogueRuntimeRoleBody<T = RuntimeSemanticTypeId> {
    /// The role is retained in the outer schema but has no accepted body.
    Unbound,
    /// The role payload is admitted by this exact Dialogue-owned codec.
    Bound {
        /// Semantic identity of the payload root.
        payload: T,
        /// Closed schema and value codec for that payload.
        codec: CharacterDialogueRolePayloadCodec,
    },
}

impl<T> CharacterDialogueRuntimeRoleBody<T> {
    #[must_use]
    pub const fn unbound() -> Self {
        Self::Unbound
    }

    #[must_use]
    pub const fn bound(payload: T, codec: CharacterDialogueRolePayloadCodec) -> Self {
        Self::Bound { payload, codec }
    }

    #[must_use]
    pub const fn payload_ref(&self) -> Option<&T> {
        match self {
            Self::Unbound => None,
            Self::Bound { payload, .. } => Some(payload),
        }
    }

    #[must_use]
    pub const fn codec(&self) -> Option<CharacterDialogueRolePayloadCodec> {
        match self {
            Self::Unbound => None,
            Self::Bound { codec, .. } => Some(*codec),
        }
    }
}

/// Complete bindings in `CharacterDialogueRuntimeRole::AUTHORED_BASE` order.
/// Style has the separately projected ordered choice identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeRoleTypes<T = RuntimeSemanticTypeId> {
    authored: [CharacterDialogueRuntimeRoleType<T>; 6],
    style: T,
}

impl<T> CharacterDialogueRuntimeRoleTypes<T> {
    pub const fn new(authored: [CharacterDialogueRuntimeRoleType<T>; 6], style: T) -> Self {
        Self { authored, style }
    }

    #[must_use]
    pub const fn authored_refs(&self) -> &[CharacterDialogueRuntimeRoleType<T>; 6] {
        &self.authored
    }

    #[must_use]
    pub const fn style_ref(&self) -> &T {
        &self.style
    }

    pub fn visit_type_refs<'a>(&'a self, visit: &mut impl FnMut(&'a T)) {
        for binding in &self.authored {
            visit(binding.value_ref());
            if let Some(payload) = binding.body_ref().payload_ref() {
                visit(payload);
            }
        }
        visit(&self.style);
    }
}

impl<T: Copy> CharacterDialogueRuntimeRoleTypes<T> {
    pub fn authored(&self, role: Role) -> Option<CharacterDialogueRuntimeRoleType<T>> {
        Role::AUTHORED_BASE
            .iter()
            .position(|candidate| *candidate == role)
            .map(|index| self.authored[index])
    }

    pub fn value_type(&self, role: Role) -> T {
        self.authored(role)
            .map_or(self.style, |binding| binding.value())
    }
}

impl CharacterDialogueRuntimeRoleTypes<RuntimeSemanticTypeId> {
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
            if role == Role::RichText {
                let CharacterDialogueRuntimeRoleBody::Bound { payload, codec } = binding.body_ref()
                else {
                    return Err(invalid("RichText has no accepted payload codec"));
                };
                let schema = codec.payload_schema()?;
                if *payload != schema.root() {
                    return Err(invalid(
                        "RichText payload identity differs from its codec root",
                    ));
                }
                Self::validate_payload_schema(role, schema, program)?;
                rich_text = Some(owner);
            } else if !matches!(
                binding.body_ref(),
                CharacterDialogueRuntimeRoleBody::Unbound
            ) {
                return Err(invalid(
                    "role body has no Dialogue-owned payload codec for this role",
                ));
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

    fn validate_payload_schema(
        role: Role,
        schema: &CharacterDialogueRolePayloadSchema,
        program: RuntimeProgramTypes<'_>,
    ) -> Result<(), CharacterDialogueValueError> {
        for seed in schema.types() {
            let expected = schema
                .checked_type(seed.semantic_identity())
                .expect("every payload seed has its codec-owned checked projection");
            let actual = program.checked_type(seed.semantic_identity())?;
            if actual != *expected {
                return Err(CharacterDialogueValueError::RoleType {
                    role,
                    reason: "payload graph differs from the bound Dialogue codec",
                });
            }
        }
        Ok(())
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
        let CharacterDialogueRuntimeRoleBody::Bound { payload, codec } = binding.body() else {
            return Err(CharacterDialogueValueError::RoleType {
                role: authored,
                reason: "role has no accepted payload codec",
            });
        };
        self.program_types()
            .accepts_value(payload, value.payload(), Self::limits())?;
        codec.decode_properties(value.payload())?;
        Ok(())
    }
}
