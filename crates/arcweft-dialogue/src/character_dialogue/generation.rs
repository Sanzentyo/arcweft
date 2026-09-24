//! Compiler-neutral, generation-owned CharacterDialogue declaration.

mod codec;

pub use codec::CharacterDialogueGenerationDeclarationCodecError;

use super::{
    CharacterDialogueConfig, CharacterDialogueRichTextValue, CharacterDialogueRolePayloadCodec,
    CharacterDialogueRuntimeCustomFieldCatalog, CharacterDialogueRuntimeCustomFieldDescriptor,
    CharacterDialogueRuntimeDefault, CharacterDialogueRuntimeRole as Role,
    CharacterDialogueRuntimeRoleBody, CharacterDialogueRuntimeRoleType,
    CharacterDialogueRuntimeRoleTypes, CharacterDialogueRuntimeSchema, CharacterDialogueStyleValue,
    CharacterDialogueType, CharacterDialogueTypedValue, CharacterDialogueValueError,
    InlineFailurePolicy, PRODUCTION_CHARACTER_DIALOGUE_LIMITS,
};
use crate::{
    DialoguePresentationProfile, DialogueProfileRevision, FallbackStylePolicy, InlineFallback,
};
use arcweft_character::id::CharacterId;
use arcweft_core::{
    character_nominal::CharacterNominalType,
    entry::RuntimeValueDigest,
    pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId},
    value::{
        RuntimeEntityReference, RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeSeq,
        RuntimeValue,
    },
};
use arcweft_id::DeclarationIdentityFamily;
use std::{collections::BTreeMap, error::Error};
use thiserror::Error as ThisError;

const GENERATION_DECLARATION_DIGEST_DOMAIN: &[u8] =
    b"arcweft.character-dialogue.generation-declaration.v1\0";
const MAX_GENERATION_DECLARATION_DIGEST_BYTES: usize =
    PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize * 8;

/// A declaration-time type reference that exposes only its stable semantic
/// identity. A compiler or runtime-plan type can implement this without
/// copying its type graph into Dialogue.
pub trait CharacterDialogueTypeReference {
    fn semantic_identity(&self) -> RuntimeSemanticTypeId;
}

impl CharacterDialogueTypeReference for RuntimeSemanticTypeId {
    fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        *self
    }
}

/// Accepted visual contract for one logical Character row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDialogueVisualType<T> {
    Absent,
    Present {
        manifest: RuntimeValueDigest,
        look_type: T,
    },
}

/// One logical Character's exact dialogue type, visual facts, and effective
/// default configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueCharacterDeclaration<T> {
    dialogue_type: T,
    visual: CharacterDialogueVisualType<T>,
    defaults: CharacterDialogueRuntimeDefault,
}

impl<T> CharacterDialogueCharacterDeclaration<T> {
    #[must_use]
    pub const fn new(
        dialogue_type: T,
        visual: CharacterDialogueVisualType<T>,
        defaults: CharacterDialogueRuntimeDefault,
    ) -> Self {
        Self {
            dialogue_type,
            visual,
            defaults,
        }
    }

    #[must_use]
    pub const fn dialogue_type(&self) -> &T {
        &self.dialogue_type
    }

    #[must_use]
    pub const fn visual(&self) -> &CharacterDialogueVisualType<T> {
        &self.visual
    }

    #[must_use]
    pub const fn defaults(&self) -> &CharacterDialogueRuntimeDefault {
        &self.defaults
    }
}

/// Accepted presentation inputs and the exact resource fingerprints checked
/// when this generation is used at runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialoguePresentationContract {
    profile: DialoguePresentationProfile,
    revision: DialogueProfileRevision,
    view_registry: RuntimeValueDigest,
    style_resource: Option<RuntimeValueDigest>,
}

/// Invalid generation declaration, identity binding, or accepted resource
/// fingerprint.
#[derive(Clone, Debug, Eq, ThisError, PartialEq)]
pub enum CharacterDialogueGenerationDeclarationError {
    #[error("CharacterDialogue generation contains duplicate Character `{0}`")]
    DuplicateCharacter(CharacterId),
    #[error("CharacterDialogue default row `{actual}` does not match map key `{expected}`")]
    DefaultCharacterMismatch {
        expected: CharacterId,
        actual: CharacterId,
    },
    #[error("CharacterDialogue Any type identity is {actual:?}, expected {expected:?}")]
    AnyDialogueTypeIdentity {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
    #[error(
        "CharacterDialogue exact type identity for `{character}` is {actual:?}, expected {expected:?}"
    )]
    CharacterDialogueTypeIdentity {
        character: CharacterId,
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
    #[error("CharacterLook type identity for `{character}` is {actual:?}, expected {expected:?}")]
    CharacterLookTypeIdentity {
        character: CharacterId,
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
    #[error("selected presentation Style has no accepted Style resource fingerprint")]
    MissingAcceptedStyleResource,
    #[error("View registry fingerprint differs from the accepted presentation contract")]
    ViewRegistryFingerprintMismatch {
        expected: RuntimeValueDigest,
        actual: RuntimeValueDigest,
    },
    #[error("Style resource fingerprint differs from the accepted presentation contract")]
    StyleResourceFingerprintMismatch {
        expected: Option<RuntimeValueDigest>,
        actual: Option<RuntimeValueDigest>,
    },
    #[error("CharacterDialogue generation exceeds `{limit}` limit {maximum}")]
    Limit { limit: &'static str, maximum: usize },
    #[error(transparent)]
    Value(#[from] CharacterDialogueValueError),
}

/// Fallible type-reference mapping error. Semantic identity changes are
/// rejected even when the mapper itself succeeds.
#[derive(Debug, ThisError)]
pub enum CharacterDialogueTypeReferenceMapError<E: Error + 'static> {
    #[error("CharacterDialogue type-reference mapper failed: {0}")]
    Mapper(#[source] E),
    #[error("type-reference mapping changed semantic identity from {expected:?} to {actual:?}")]
    IdentityChanged {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
    #[error(transparent)]
    Declaration(#[from] CharacterDialogueGenerationDeclarationError),
}

/// Complete accepted source declaration for one immutable runtime generation.
/// Every retained type reference resolves through `T`; this value owns neither
/// an executable type table nor a second schema graph.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueGenerationDeclaration<
    T: CharacterDialogueTypeReference = RuntimeSemanticTypeId,
> {
    characters: BTreeMap<CharacterId, CharacterDialogueCharacterDeclaration<T>>,
    any_dialogue: T,
    voice: T,
    roles: CharacterDialogueRuntimeRoleTypes<T>,
    custom_fields: CharacterDialogueRuntimeCustomFieldCatalog<T>,
    presentation: CharacterDialoguePresentationContract,
    digest: RuntimeValueDigest,
}

impl CharacterDialoguePresentationContract {
    pub fn try_new(
        profile: DialoguePresentationProfile,
        revision: DialogueProfileRevision,
        view_registry: RuntimeValueDigest,
        style_resource: Option<RuntimeValueDigest>,
    ) -> Result<Self, CharacterDialogueGenerationDeclarationError> {
        if profile.style().is_some() && style_resource.is_none() {
            return Err(CharacterDialogueGenerationDeclarationError::MissingAcceptedStyleResource);
        }
        Ok(Self {
            profile,
            revision,
            view_registry,
            style_resource,
        })
    }

    #[must_use]
    pub const fn profile(&self) -> &DialoguePresentationProfile {
        &self.profile
    }

    #[must_use]
    pub const fn revision(&self) -> &DialogueProfileRevision {
        &self.revision
    }

    #[must_use]
    pub const fn view_registry_digest(&self) -> RuntimeValueDigest {
        self.view_registry
    }

    #[must_use]
    pub const fn style_resource_digest(&self) -> Option<RuntimeValueDigest> {
        self.style_resource
    }

    /// Rechecks the fingerprints against the actual resources selected by the
    /// runtime generation.
    pub fn verify_resource_fingerprints(
        &self,
        view_registry: RuntimeValueDigest,
        style_resource: Option<RuntimeValueDigest>,
    ) -> Result<(), CharacterDialogueGenerationDeclarationError> {
        if self.view_registry != view_registry {
            return Err(
                CharacterDialogueGenerationDeclarationError::ViewRegistryFingerprintMismatch {
                    expected: self.view_registry,
                    actual: view_registry,
                },
            );
        }
        if self.style_resource != style_resource {
            return Err(
                CharacterDialogueGenerationDeclarationError::StyleResourceFingerprintMismatch {
                    expected: self.style_resource,
                    actual: style_resource,
                },
            );
        }
        Ok(())
    }
}

impl CharacterDialogueConfig {
    /// Builds the engine default from the accepted presentation profile and
    /// exact role identities. Optional roles remain absent and hooks empty.
    pub fn try_from_presentation_profile<T: CharacterDialogueTypeReference>(
        profile: &DialoguePresentationProfile,
        roles: &CharacterDialogueRuntimeRoleTypes<T>,
    ) -> Result<Self, CharacterDialogueValueError> {
        validate_role_payload_bindings(roles)?;
        let rich_text_index = Role::AUTHORED_BASE
            .iter()
            .position(|role| *role == Role::RichText)
            .expect("RichText has a fixed authored role slot");
        let binding = &roles.authored_refs()[rich_text_index];
        let CharacterDialogueRuntimeRoleBody::Bound { payload, codec } = binding.body_ref() else {
            return Err(CharacterDialogueValueError::RoleType {
                role: Role::RichText,
                reason: "RichText has no accepted payload codec",
            });
        };
        let schema = codec.payload_schema()?;
        if payload.semantic_identity() != schema.root() {
            return Err(CharacterDialogueValueError::RoleType {
                role: Role::RichText,
                reason: "RichText payload identity differs from its codec root",
            });
        }
        let rich_text_owner = RuntimeOpaqueTypeOwner::exact_with(
            CharacterDialogueRuntimeSchema::opaque_type_producer(),
            binding.value_ref().semantic_identity(),
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::ConstantAndSnapshot,
        );
        let rich_text_value = rich_text_owner.try_wrap(codec.no_overrides_payload()?)?;
        let rich_text = CharacterDialogueRichTextValue::try_new(
            CharacterDialogueTypedValue::try_new(rich_text_value)?,
        )?;
        let style_value = profile.style().map_or_else(
            || rich_text.typed().value().clone(),
            |style| {
                RuntimeValue::EntityRef(RuntimeEntityReference::Project {
                    family: DeclarationIdentityFamily::Style,
                    public_id: style.public_id().clone(),
                })
            },
        );
        let style = CharacterDialogueStyleValue::try_new(CharacterDialogueTypedValue::try_new(
            style_value,
        )?)?;
        let mut config = Self::try_new(profile.view().clone(), style, rich_text)?;
        config.inline_failure = profile.inline_failure().clone();
        config.validate()?;
        Ok(config)
    }
}

impl<T: CharacterDialogueTypeReference> CharacterDialogueGenerationDeclaration<T> {
    pub fn try_new(
        characters: impl IntoIterator<Item = (CharacterId, CharacterDialogueCharacterDeclaration<T>)>,
        any_dialogue: T,
        voice: T,
        roles: CharacterDialogueRuntimeRoleTypes<T>,
        custom_fields: CharacterDialogueRuntimeCustomFieldCatalog<T>,
        presentation: CharacterDialoguePresentationContract,
    ) -> Result<Self, CharacterDialogueGenerationDeclarationError> {
        let expected_any = CharacterDialogueType::any().runtime_semantic_identity();
        let actual_any = any_dialogue.semantic_identity();
        if expected_any != actual_any {
            return Err(
                CharacterDialogueGenerationDeclarationError::AnyDialogueTypeIdentity {
                    expected: expected_any,
                    actual: actual_any,
                },
            );
        }

        validate_role_payload_bindings(&roles)?;

        let mut rows = BTreeMap::new();
        let mut default_digests = BTreeMap::new();
        for (character, row) in characters {
            if row.defaults.character() != &character {
                return Err(
                    CharacterDialogueGenerationDeclarationError::DefaultCharacterMismatch {
                        expected: character,
                        actual: row.defaults.character().clone(),
                    },
                );
            }
            if rows.contains_key(&character) {
                return Err(
                    CharacterDialogueGenerationDeclarationError::DuplicateCharacter(character),
                );
            }
            if rows.len() >= PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_defaults_entries as usize {
                return Err(CharacterDialogueGenerationDeclarationError::Limit {
                    limit: "defaults_entries",
                    maximum: PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_defaults_entries as usize,
                });
            }
            let expected_dialogue =
                CharacterDialogueType::exact(character.clone()).runtime_semantic_identity();
            let actual_dialogue = row.dialogue_type.semantic_identity();
            if expected_dialogue != actual_dialogue {
                return Err(
                    CharacterDialogueGenerationDeclarationError::CharacterDialogueTypeIdentity {
                        character,
                        expected: expected_dialogue,
                        actual: actual_dialogue,
                    },
                );
            }
            if let CharacterDialogueVisualType::Present { look_type, .. } = &row.visual {
                let expected_look = CharacterNominalType::Look {
                    character: character.clone(),
                }
                .runtime_semantic_identity();
                let actual_look = look_type.semantic_identity();
                if expected_look != actual_look {
                    return Err(
                        CharacterDialogueGenerationDeclarationError::CharacterLookTypeIdentity {
                            character,
                            expected: expected_look,
                            actual: actual_look,
                        },
                    );
                }
            }
            row.defaults.config().validate()?;
            validate_default_config_role_values(row.defaults.config(), &roles)?;
            validate_default_config_custom_values(row.defaults.config(), &custom_fields)?;
            let default_digest =
                super::schema::effective_default_config_digest(row.defaults.config(), &roles)?;
            if row
                .defaults
                .expected_digest()
                .is_some_and(|expected| expected != default_digest)
            {
                return Err(
                    CharacterDialogueValueError::DefaultDigestMismatch(character.clone()).into(),
                );
            }
            default_digests.insert(character.clone(), default_digest);
            rows.insert(character, row);
        }

        let digest = Self::digest_parts(
            &rows,
            &any_dialogue,
            &voice,
            &roles,
            &custom_fields,
            &presentation,
            &default_digests,
        )?;
        Ok(Self {
            characters: rows,
            any_dialogue,
            voice,
            roles,
            custom_fields,
            presentation,
            digest,
        })
    }

    #[must_use]
    pub const fn characters(
        &self,
    ) -> &BTreeMap<CharacterId, CharacterDialogueCharacterDeclaration<T>> {
        &self.characters
    }

    #[must_use]
    pub const fn any_dialogue(&self) -> &T {
        &self.any_dialogue
    }

    #[must_use]
    pub const fn voice(&self) -> &T {
        &self.voice
    }

    #[must_use]
    pub const fn roles(&self) -> &CharacterDialogueRuntimeRoleTypes<T> {
        &self.roles
    }

    #[must_use]
    pub const fn custom_fields(&self) -> &CharacterDialogueRuntimeCustomFieldCatalog<T> {
        &self.custom_fields
    }

    #[must_use]
    pub const fn presentation(&self) -> &CharacterDialoguePresentationContract {
        &self.presentation
    }

    #[must_use]
    pub const fn digest(&self) -> RuntimeValueDigest {
        self.digest
    }

    /// Visits references in a stable declaration order: Any, Voice, sorted
    /// Character rows, authored roles plus Style, then sorted custom fields.
    pub fn visit_type_refs<'a>(&'a self, visit: &mut impl FnMut(&'a T)) {
        visit(&self.any_dialogue);
        visit(&self.voice);
        for row in self.characters.values() {
            visit(&row.dialogue_type);
            if let CharacterDialogueVisualType::Present { look_type, .. } = &row.visual {
                visit(look_type);
            }
        }
        self.roles.visit_type_refs(visit);
        self.custom_fields.visit_type_refs(visit);
    }

    /// Maps every declaration reference while preserving the stable semantic
    /// identity of each source reference.
    pub fn try_map_type_refs<U, E>(
        &self,
        mut map: impl FnMut(&T) -> Result<U, E>,
    ) -> Result<CharacterDialogueGenerationDeclaration<U>, CharacterDialogueTypeReferenceMapError<E>>
    where
        U: CharacterDialogueTypeReference,
        E: Error + 'static,
    {
        let mut map_ref = |source: &T| try_map_type_ref(source, &mut map);
        let characters = self
            .characters
            .iter()
            .map(|(character, row)| {
                let dialogue_type = map_ref(&row.dialogue_type)?;
                let visual = match &row.visual {
                    CharacterDialogueVisualType::Absent => CharacterDialogueVisualType::Absent,
                    CharacterDialogueVisualType::Present {
                        manifest,
                        look_type,
                    } => CharacterDialogueVisualType::Present {
                        manifest: *manifest,
                        look_type: map_ref(look_type)?,
                    },
                };
                Ok((
                    character.clone(),
                    CharacterDialogueCharacterDeclaration::new(
                        dialogue_type,
                        visual,
                        row.defaults.clone(),
                    ),
                ))
            })
            .collect::<Result<Vec<_>, CharacterDialogueTypeReferenceMapError<E>>>()?;
        let any_dialogue = map_ref(&self.any_dialogue)?;
        let voice = map_ref(&self.voice)?;
        let [first, second, third, fourth, fifth, sixth] = self.roles.authored_refs();
        let mapped_authored = [
            CharacterDialogueRuntimeRoleType::new(
                map_ref(first.value_ref())?,
                map_role_body(first.body_ref(), &mut map_ref)?,
            ),
            CharacterDialogueRuntimeRoleType::new(
                map_ref(second.value_ref())?,
                map_role_body(second.body_ref(), &mut map_ref)?,
            ),
            CharacterDialogueRuntimeRoleType::new(
                map_ref(third.value_ref())?,
                map_role_body(third.body_ref(), &mut map_ref)?,
            ),
            CharacterDialogueRuntimeRoleType::new(
                map_ref(fourth.value_ref())?,
                map_role_body(fourth.body_ref(), &mut map_ref)?,
            ),
            CharacterDialogueRuntimeRoleType::new(
                map_ref(fifth.value_ref())?,
                map_role_body(fifth.body_ref(), &mut map_ref)?,
            ),
            CharacterDialogueRuntimeRoleType::new(
                map_ref(sixth.value_ref())?,
                map_role_body(sixth.body_ref(), &mut map_ref)?,
            ),
        ];
        let roles = CharacterDialogueRuntimeRoleTypes::new(
            mapped_authored,
            map_ref(self.roles.style_ref())?,
        );
        let custom_fields = self
            .custom_fields
            .fields()
            .values()
            .map(|descriptor| {
                Ok(CharacterDialogueRuntimeCustomFieldDescriptor::new(
                    descriptor.id().clone(),
                    map_ref(descriptor.semantic_type_ref())?,
                    descriptor.clearable(),
                    descriptor.accepted_views().clone(),
                ))
            })
            .collect::<Result<Vec<_>, CharacterDialogueTypeReferenceMapError<E>>>()?;
        let custom_fields = CharacterDialogueRuntimeCustomFieldCatalog::try_new(custom_fields)
            .map_err(|error| CharacterDialogueTypeReferenceMapError::Declaration(error.into()))?;
        CharacterDialogueGenerationDeclaration::try_new(
            characters,
            any_dialogue,
            voice,
            roles,
            custom_fields,
            self.presentation.clone(),
        )
        .map_err(Into::into)
    }

    fn digest_parts(
        characters: &BTreeMap<CharacterId, CharacterDialogueCharacterDeclaration<T>>,
        any_dialogue: &T,
        voice: &T,
        roles: &CharacterDialogueRuntimeRoleTypes<T>,
        custom_fields: &CharacterDialogueRuntimeCustomFieldCatalog<T>,
        presentation: &CharacterDialoguePresentationContract,
        default_digests: &BTreeMap<CharacterId, RuntimeValueDigest>,
    ) -> Result<RuntimeValueDigest, CharacterDialogueGenerationDeclarationError> {
        let mut encoder = GenerationDeclarationDigestEncoder::new();
        encoder.write_len(characters.len(), "defaults_entries")?;
        for (character, row) in characters {
            encoder.write_str(character.as_str(), "character_id_bytes")?;
            encoder.write_type_ref(&row.dialogue_type)?;
            match &row.visual {
                CharacterDialogueVisualType::Absent => encoder.write_u8(0)?,
                CharacterDialogueVisualType::Present {
                    manifest,
                    look_type,
                } => {
                    encoder.write_u8(1)?;
                    encoder.write_bytes(manifest.as_bytes())?;
                    encoder.write_type_ref(look_type)?;
                }
            }
            let config_digest = default_digests
                .get(character)
                .expect("each declaration row has a validated effective-default digest");
            encoder.write_bytes(config_digest.as_bytes())?;
        }
        encoder.write_type_ref(any_dialogue)?;
        encoder.write_type_ref(voice)?;
        for (role, binding) in Role::AUTHORED_BASE
            .into_iter()
            .zip(roles.authored_refs().iter())
        {
            encoder.write_u8(role.canonical_tag())?;
            encoder.write_type_ref(binding.value_ref())?;
            match binding.body_ref() {
                CharacterDialogueRuntimeRoleBody::Unbound => encoder.write_u8(0)?,
                CharacterDialogueRuntimeRoleBody::Bound { payload, codec } => {
                    encoder.write_u8(1)?;
                    encoder.write_type_ref(payload)?;
                    encoder.write_u8(codec.canonical_tag())?;
                    let schema_digest = codec.payload_schema()?.schema_digest();
                    encoder.write_bytes(schema_digest.as_bytes())?;
                }
            }
        }
        encoder.write_u8(Role::Style.canonical_tag())?;
        encoder.write_type_ref(roles.style_ref())?;
        encoder.write_bytes(custom_fields.digest().as_bytes())?;
        write_presentation_contract(&mut encoder, presentation)?;
        Ok(encoder.finish())
    }
}

fn map_role_body<T, U, E>(
    body: &CharacterDialogueRuntimeRoleBody<T>,
    map_ref: &mut impl FnMut(&T) -> Result<U, CharacterDialogueTypeReferenceMapError<E>>,
) -> Result<CharacterDialogueRuntimeRoleBody<U>, CharacterDialogueTypeReferenceMapError<E>>
where
    E: Error + 'static,
{
    match body {
        CharacterDialogueRuntimeRoleBody::Unbound => Ok(CharacterDialogueRuntimeRoleBody::Unbound),
        CharacterDialogueRuntimeRoleBody::Bound { payload, codec } => {
            Ok(CharacterDialogueRuntimeRoleBody::Bound {
                payload: map_ref(payload)?,
                codec: *codec,
            })
        }
    }
}

fn validate_role_payload_bindings<T: CharacterDialogueTypeReference>(
    roles: &CharacterDialogueRuntimeRoleTypes<T>,
) -> Result<(), CharacterDialogueValueError> {
    for (role, binding) in Role::AUTHORED_BASE
        .into_iter()
        .zip(roles.authored_refs().iter())
    {
        if role == Role::RichText {
            let CharacterDialogueRuntimeRoleBody::Bound { payload, codec } = binding.body_ref()
            else {
                return Err(CharacterDialogueValueError::RoleType {
                    role,
                    reason: "RichText has no accepted payload codec",
                });
            };
            if payload.semantic_identity() != codec.payload_schema()?.root() {
                return Err(CharacterDialogueValueError::RoleType {
                    role,
                    reason: "RichText payload identity differs from its codec root",
                });
            }
        } else if !matches!(
            binding.body_ref(),
            CharacterDialogueRuntimeRoleBody::Unbound
        ) {
            return Err(CharacterDialogueValueError::RoleType {
                role,
                reason: "role body has no Dialogue-owned payload codec for this role",
            });
        }
    }
    Ok(())
}

fn validate_default_config_role_values<T: CharacterDialogueTypeReference>(
    config: &CharacterDialogueConfig,
    roles: &CharacterDialogueRuntimeRoleTypes<T>,
) -> Result<(), CharacterDialogueValueError> {
    for (role, value) in [
        (
            Role::Stage,
            config.stage.as_ref().map(|value| value.typed().value()),
        ),
        (
            Role::Portrait,
            config.portrait.as_ref().map(|value| value.typed().value()),
        ),
        (
            Role::Focus,
            config.focus.as_ref().map(|value| value.typed().value()),
        ),
        (
            Role::Cleanup,
            config.cleanup.as_ref().map(|value| value.typed().value()),
        ),
    ] {
        if let Some(value) = value {
            validate_bound_role_value(role, value, roles)?;
        }
    }
    if !config.hooks.is_empty() {
        let role = Role::Hook;
        for hook in &config.hooks {
            validate_bound_role_value(role, hook.typed().value(), roles)?;
        }
    }

    validate_bound_role_value(Role::RichText, config.rich_text.typed().value(), roles)?;
    match config.style.typed().value() {
        RuntimeValue::EntityRef(RuntimeEntityReference::Project {
            family: DeclarationIdentityFamily::Style,
            ..
        }) => {}
        value => validate_bound_role_value(Role::RichText, value, roles)?,
    }
    let rich_text_index = Role::AUTHORED_BASE
        .iter()
        .position(|role| *role == Role::RichText)
        .expect("RichText has a fixed authored role slot");
    let CharacterDialogueRuntimeRoleBody::Bound { codec, .. } =
        roles.authored_refs()[rich_text_index].body_ref()
    else {
        return Err(CharacterDialogueValueError::RoleType {
            role: Role::RichText,
            reason: "RichText has no accepted payload codec",
        });
    };
    if *codec != CharacterDialogueRolePayloadCodec::RichTextProperties {
        return Err(CharacterDialogueValueError::RoleType {
            role: Role::RichText,
            reason: "RichText payload codec is unsupported",
        });
    }
    Ok(())
}

fn validate_bound_role_value<T: CharacterDialogueTypeReference>(
    role: Role,
    value: &RuntimeValue,
    roles: &CharacterDialogueRuntimeRoleTypes<T>,
) -> Result<(), CharacterDialogueValueError> {
    let index = Role::AUTHORED_BASE
        .iter()
        .position(|candidate| *candidate == role)
        .expect("authored role has a fixed role slot");
    let binding = &roles.authored_refs()[index];
    let CharacterDialogueRuntimeRoleBody::Bound { payload, codec } = binding.body_ref() else {
        return Err(CharacterDialogueValueError::RoleType {
            role,
            reason: "role has no accepted payload codec",
        });
    };
    let RuntimeValue::Opaque(value) = value else {
        return Err(CharacterDialogueValueError::RoleType {
            role,
            reason: "authored role value is not opaque",
        });
    };
    if value.producer() != &CharacterDialogueRuntimeSchema::opaque_type_producer()
        || value.semantic_identity() != binding.value_ref().semantic_identity()
        || value.value_class() != RuntimeOpaqueValueClass::Plain
        || value.persistence() != RuntimeOpaquePersistence::ConstantAndSnapshot
    {
        return Err(CharacterDialogueValueError::RoleType {
            role,
            reason: "authored role has a different exact opaque contract",
        });
    }
    if payload.semantic_identity() != codec.payload_schema()?.root() {
        return Err(CharacterDialogueValueError::RoleType {
            role,
            reason: "role payload identity differs from its codec root",
        });
    }
    codec.decode_properties(value.payload())?;
    Ok(())
}

fn validate_default_config_custom_values<T: CharacterDialogueTypeReference>(
    config: &CharacterDialogueConfig,
    custom_fields: &CharacterDialogueRuntimeCustomFieldCatalog<T>,
) -> Result<(), CharacterDialogueValueError> {
    for id in config.custom.keys() {
        let descriptor = custom_fields
            .get(id)
            .ok_or_else(|| CharacterDialogueValueError::UnknownCustomField(id.clone()))?;
        if !descriptor.accepted_views().is_empty()
            && !descriptor.accepted_views().contains(&config.view)
        {
            return Err(CharacterDialogueValueError::CustomFieldView {
                field: id.clone(),
                view: config.view.clone(),
            });
        }
    }
    Ok(())
}

fn try_map_type_ref<T, U, E>(
    source: &T,
    map: &mut impl FnMut(&T) -> Result<U, E>,
) -> Result<U, CharacterDialogueTypeReferenceMapError<E>>
where
    T: CharacterDialogueTypeReference,
    U: CharacterDialogueTypeReference,
    E: Error + 'static,
{
    let mapped = map(source).map_err(CharacterDialogueTypeReferenceMapError::Mapper)?;
    let expected = source.semantic_identity();
    let actual = mapped.semantic_identity();
    if expected != actual {
        return Err(CharacterDialogueTypeReferenceMapError::IdentityChanged { expected, actual });
    }
    Ok(mapped)
}

fn encode_profile_inline_failure(policy: &InlineFailurePolicy) -> RuntimeValue {
    match policy {
        InlineFailurePolicy::FailLine => {
            RuntimeValue::Tuple(vec![RuntimeValue::String("fail_line".to_owned())])
        }
        InlineFailurePolicy::Discard => {
            RuntimeValue::Tuple(vec![RuntimeValue::String("discard".to_owned())])
        }
        InlineFailurePolicy::Fallback { fallback } => RuntimeValue::Tuple(vec![
            RuntimeValue::String("fallback".to_owned()),
            encode_profile_fallback(fallback),
        ]),
    }
}

fn encode_profile_fallback(fallback: &InlineFallback) -> RuntimeValue {
    match fallback {
        InlineFallback::Text { text, style } => RuntimeValue::Tuple(vec![
            RuntimeValue::String("text".to_owned()),
            RuntimeValue::String(text.clone()),
            encode_profile_fallback_style(style),
        ]),
        InlineFallback::ExprSource { style } => RuntimeValue::Tuple(vec![
            RuntimeValue::String("expr_source".to_owned()),
            encode_profile_fallback_style(style),
        ]),
        InlineFallback::CallSource { style } => RuntimeValue::Tuple(vec![
            RuntimeValue::String("call_source".to_owned()),
            encode_profile_fallback_style(style),
        ]),
        InlineFallback::ValuePlain => {
            RuntimeValue::Tuple(vec![RuntimeValue::String("value_plain".to_owned())])
        }
    }
}

fn encode_profile_fallback_style(style: &FallbackStylePolicy) -> RuntimeValue {
    match style {
        FallbackStylePolicy::Plain => {
            RuntimeValue::Tuple(vec![RuntimeValue::String("plain".to_owned())])
        }
        FallbackStylePolicy::InheritSurrounding => {
            RuntimeValue::Tuple(vec![RuntimeValue::String("inherit_surrounding".to_owned())])
        }
        FallbackStylePolicy::Apply { styles } => RuntimeValue::Tuple(vec![
            RuntimeValue::String("apply".to_owned()),
            RuntimeValue::Seq(RuntimeSeq::values(
                styles
                    .iter()
                    .map(|style| style.typed().value().clone())
                    .collect(),
            )),
        ]),
    }
}

fn write_presentation_contract(
    encoder: &mut GenerationDeclarationDigestEncoder,
    contract: &CharacterDialoguePresentationContract,
) -> Result<(), CharacterDialogueGenerationDeclarationError> {
    let profile = contract.profile();
    encoder.write_str(profile.view().as_str(), "presentation_view_id_bytes")?;
    match profile.style() {
        Some(style) => {
            encoder.write_u8(1)?;
            encoder.write_str(style.as_str(), "presentation_style_id_bytes")?;
        }
        None => encoder.write_u8(0)?,
    }
    let inline_failure = RuntimeValue::Tuple(vec![encode_profile_inline_failure(
        profile.inline_failure(),
    )])
    .try_canonical_bytes(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize)
    .map_err(CharacterDialogueValueError::from)?;
    encoder.write_sized_bytes(&inline_failure)?;

    let revision = contract.revision();
    let manifest = revision.manifest_document();
    encoder.write_str(manifest.id().as_str(), "manifest_document_id_bytes")?;
    encoder.write_bytes(manifest.revision().as_bytes())?;
    encoder.write_u64(manifest.source_len())?;
    encoder.write_bytes(revision.topology_sources().as_bytes())?;
    encoder.write_bytes(revision.compiled_sources().as_bytes())?;
    encoder.write_str(revision.view_program_id().as_str(), "view_program_id_bytes")?;
    encoder.write_bytes(revision.view_program_revision().as_bytes())?;
    encoder.write_bytes(revision.resource_types().semantic_digest().as_bytes())?;
    encoder.write_bytes(contract.view_registry_digest().as_bytes())?;
    match contract.style_resource_digest() {
        Some(digest) => {
            encoder.write_u8(1)?;
            encoder.write_bytes(digest.as_bytes())?;
        }
        None => encoder.write_u8(0)?,
    }
    Ok(())
}

struct GenerationDeclarationDigestEncoder {
    hasher: blake3::Hasher,
    encoded_len: usize,
}

impl GenerationDeclarationDigestEncoder {
    fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(GENERATION_DECLARATION_DIGEST_DOMAIN);
        Self {
            hasher,
            encoded_len: GENERATION_DECLARATION_DIGEST_DOMAIN.len(),
        }
    }

    fn write_type_ref<T: CharacterDialogueTypeReference>(
        &mut self,
        value: &T,
    ) -> Result<(), CharacterDialogueGenerationDeclarationError> {
        self.write_bytes(value.semantic_identity().as_bytes())
    }

    fn write_str(
        &mut self,
        value: &str,
        limit: &'static str,
    ) -> Result<(), CharacterDialogueGenerationDeclarationError> {
        self.write_len(value.len(), limit)?;
        self.write_bytes(value.as_bytes())
    }

    fn write_len(
        &mut self,
        value: usize,
        limit: &'static str,
    ) -> Result<(), CharacterDialogueGenerationDeclarationError> {
        let value = u32::try_from(value).map_err(|_| {
            CharacterDialogueGenerationDeclarationError::Limit {
                limit,
                maximum: u32::MAX as usize,
            }
        })?;
        self.write_bytes(&value.to_le_bytes())
    }

    fn write_u8(&mut self, value: u8) -> Result<(), CharacterDialogueGenerationDeclarationError> {
        self.write_bytes(&[value])
    }

    fn write_u64(&mut self, value: u64) -> Result<(), CharacterDialogueGenerationDeclarationError> {
        self.write_bytes(&value.to_le_bytes())
    }

    fn write_sized_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), CharacterDialogueGenerationDeclarationError> {
        self.write_len(bytes.len(), "generation_digest_bytes")?;
        self.write_bytes(bytes)
    }

    fn write_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), CharacterDialogueGenerationDeclarationError> {
        let Some(next_len) = self.encoded_len.checked_add(bytes.len()) else {
            return Err(CharacterDialogueGenerationDeclarationError::Limit {
                limit: "generation_digest_bytes",
                maximum: MAX_GENERATION_DECLARATION_DIGEST_BYTES,
            });
        };
        if next_len > MAX_GENERATION_DECLARATION_DIGEST_BYTES {
            return Err(CharacterDialogueGenerationDeclarationError::Limit {
                limit: "generation_digest_bytes",
                maximum: MAX_GENERATION_DECLARATION_DIGEST_BYTES,
            });
        }
        self.hasher.update(bytes);
        self.encoded_len = next_len;
        Ok(())
    }

    fn finish(self) -> RuntimeValueDigest {
        RuntimeValueDigest::from_bytes(*self.hasher.finalize().as_bytes())
    }
}
