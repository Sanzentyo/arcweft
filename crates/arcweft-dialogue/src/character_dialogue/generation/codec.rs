//! Bounded version-one wire codec for complete generation declarations.

use super::{
    CharacterDialogueCharacterDeclaration, CharacterDialogueGenerationDeclaration,
    CharacterDialogueGenerationDeclarationError, CharacterDialoguePresentationContract,
    CharacterDialogueTypeReference, CharacterDialogueVisualType,
};
use crate::{
    CharacterDialogueCleanupValue, CharacterDialogueConfig, CharacterDialogueCustomFieldId,
    CharacterDialogueCustomValue, CharacterDialogueFocusValue, CharacterDialogueHookValue,
    CharacterDialoguePortraitValue, CharacterDialogueRichTextValue,
    CharacterDialogueRolePayloadCodec, CharacterDialogueRuntimeCustomFieldCatalog,
    CharacterDialogueRuntimeCustomFieldDescriptor, CharacterDialogueRuntimeDefault,
    CharacterDialogueRuntimeRole as Role, CharacterDialogueRuntimeRoleBody,
    CharacterDialogueRuntimeRoleType, CharacterDialogueRuntimeRoleTypes,
    CharacterDialogueStageValue, CharacterDialogueStyleValue, CharacterDialogueTypedValue,
    CharacterDialogueValueError, CharacterDialogueVoice,
};
use arcweft_character::id::{CharacterId, CharacterLookId};
use arcweft_core::{
    entry::RuntimeValueDigest,
    pattern::RuntimeSemanticTypeId,
    task::RuntimeProgramOwner,
    value::{AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError},
};
use arcweft_view::ViewId;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
};
use thiserror::Error;

const GENERATION_DECLARATION_CODEC_VERSION: u8 = 1;
const MAX_GENERATION_DECLARATION_CODEC_BYTES: usize = 64 * 1024 * 1024;

/// Failure while encoding or admitting a canonical generation declaration.
#[derive(Debug, Error)]
pub enum CharacterDialogueGenerationDeclarationCodecError {
    #[error("CharacterDialogue generation declaration codec exceeds `{limit}` limit {maximum}")]
    Limit { limit: &'static str, maximum: usize },
    #[error("unsupported CharacterDialogue generation declaration codec version {0}")]
    UnsupportedVersion(u8),
    #[error("CharacterDialogue generation declaration role rows are not canonical")]
    NonCanonicalRoleOrder,
    #[error("unsupported CharacterDialogue generation role codec tag {0}")]
    UnsupportedRoleCodec(u8),
    #[error(
        "CharacterDialogue generation declaration contains a duplicate or unordered row in `{0}`"
    )]
    NonCanonicalRows(&'static str),
    #[error("CharacterDialogue generation declaration bytes are not canonical")]
    NonCanonicalEncoding,
    #[error(
        "advertised CharacterDialogue generation digest {expected:?} differs from recomputed digest {actual:?}"
    )]
    AdvertisedDigestMismatch {
        expected: RuntimeValueDigest,
        actual: RuntimeValueDigest,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Snapshot(#[from] AwbcRuntimeValueSnapshotError),
    #[error(transparent)]
    Value(Box<CharacterDialogueValueError>),
    #[error(transparent)]
    Declaration(Box<CharacterDialogueGenerationDeclarationError>),
}

impl From<CharacterDialogueValueError> for CharacterDialogueGenerationDeclarationCodecError {
    fn from(error: CharacterDialogueValueError) -> Self {
        Self::Value(Box::new(error))
    }
}

impl From<CharacterDialogueGenerationDeclarationError>
    for CharacterDialogueGenerationDeclarationCodecError
{
    fn from(error: CharacterDialogueGenerationDeclarationError) -> Self {
        Self::Declaration(Box::new(error))
    }
}

impl CharacterDialogueGenerationDeclaration<RuntimeSemanticTypeId> {
    /// Encodes this immutable declaration as bounded canonical JSON bytes.
    ///
    /// The embedded digest is recomputed before serialization and identifies
    /// the declaration content; it is never used as an input to hashing.
    pub fn encode_canonical_bytes(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<Vec<u8>, CharacterDialogueGenerationDeclarationCodecError> {
        let recomputed = CharacterDialogueGenerationDeclaration::try_new(
            self.characters
                .iter()
                .map(|(character, row)| (character.clone(), row.clone())),
            self.any_dialogue,
            self.voice,
            self.roles.clone(),
            self.custom_fields.clone(),
            self.presentation.clone(),
        )?;
        if recomputed.digest != self.digest {
            return Err(
                CharacterDialogueGenerationDeclarationCodecError::AdvertisedDigestMismatch {
                    expected: self.digest,
                    actual: recomputed.digest,
                },
            );
        }

        let wire = WireGenerationRef::from_declaration(&recomputed, owner)?;
        let mut output = BoundedOutput::new(MAX_GENERATION_DECLARATION_CODEC_BYTES);
        match serde_json::to_writer(&mut output, &wire) {
            Ok(()) => Ok(output.bytes),
            Err(_) if output.exceeded_limit => {
                Err(CharacterDialogueGenerationDeclarationCodecError::Limit {
                    limit: "generation_declaration_codec_bytes",
                    maximum: MAX_GENERATION_DECLARATION_CODEC_BYTES,
                })
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Decodes one bounded canonical declaration and verifies every digest.
    ///
    /// The supplied generation digest is compared only after all rows and
    /// effective defaults have been reconstructed through `try_new`.
    pub fn decode_canonical_bytes(
        bytes: &[u8],
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, CharacterDialogueGenerationDeclarationCodecError> {
        if bytes.len() > MAX_GENERATION_DECLARATION_CODEC_BYTES {
            return Err(CharacterDialogueGenerationDeclarationCodecError::Limit {
                limit: "generation_declaration_codec_bytes",
                maximum: MAX_GENERATION_DECLARATION_CODEC_BYTES,
            });
        }
        let wire: WireGenerationOwned = serde_json::from_slice(bytes)?;
        if wire.version != GENERATION_DECLARATION_CODEC_VERSION {
            return Err(
                CharacterDialogueGenerationDeclarationCodecError::UnsupportedVersion(wire.version),
            );
        }
        let expected_digest = wire.digest;
        let declaration = wire.into_declaration(owner)?;
        if declaration.digest != expected_digest {
            return Err(
                CharacterDialogueGenerationDeclarationCodecError::AdvertisedDigestMismatch {
                    expected: expected_digest,
                    actual: declaration.digest,
                },
            );
        }
        if declaration.encode_canonical_bytes(owner)? != bytes {
            return Err(CharacterDialogueGenerationDeclarationCodecError::NonCanonicalEncoding);
        }
        Ok(declaration)
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded_limit: bool,
}

impl BoundedOutput {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            exceeded_limit: false,
        }
    }
}

impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(next_len) = self.bytes.len().checked_add(bytes.len()) else {
            self.exceeded_limit = true;
            return Err(io::Error::other("bounded generation codec overflow"));
        };
        if next_len > self.maximum {
            self.exceeded_limit = true;
            return Err(io::Error::other("bounded generation codec limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WireGenerationRef<'a> {
    version: u8,
    digest: RuntimeValueDigest,
    characters: Vec<WireCharacterRef<'a>>,
    any_dialogue: RuntimeSemanticTypeId,
    voice: RuntimeSemanticTypeId,
    roles: Vec<WireRoleRef>,
    style: RuntimeSemanticTypeId,
    custom_fields: Vec<WireCustomDescriptorRef<'a>>,
    presentation: WirePresentationRef<'a>,
}

impl<'a> WireGenerationRef<'a> {
    fn from_declaration(
        declaration: &'a CharacterDialogueGenerationDeclaration<RuntimeSemanticTypeId>,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, CharacterDialogueGenerationDeclarationCodecError> {
        let characters = declaration
            .characters
            .iter()
            .map(|(character, row)| {
                Ok(WireCharacterRef {
                    character: character.as_str(),
                    dialogue_type: row.dialogue_type,
                    visual: WireVisualRef::from_visual(&row.visual),
                    defaults: WireDefaultRef {
                        character: row.defaults.character(),
                        config: WireConfigRef::from_config(row.defaults.config(), owner)?,
                        expected_digest: row.defaults.expected_digest(),
                    },
                })
            })
            .collect::<Result<_, CharacterDialogueGenerationDeclarationCodecError>>()?;
        let roles = Role::AUTHORED_BASE
            .into_iter()
            .zip(declaration.roles.authored_refs())
            .map(|(role, binding)| {
                let body = match binding.body_ref() {
                    CharacterDialogueRuntimeRoleBody::Unbound => WireRoleBodyRef::Unbound,
                    CharacterDialogueRuntimeRoleBody::Bound { payload, codec } => {
                        WireRoleBodyRef::Bound {
                            payload: *payload,
                            codec: codec.canonical_tag(),
                        }
                    }
                };
                WireRoleRef {
                    role: role.canonical_tag(),
                    value: *binding.value_ref(),
                    body,
                }
            })
            .collect();
        let custom_fields = declaration
            .custom_fields
            .fields()
            .values()
            .map(|field| WireCustomDescriptorRef {
                id: field.id(),
                semantic_type: *field.semantic_type_ref(),
                clearable: field.clearable(),
                accepted_views: field.accepted_views(),
            })
            .collect();

        Ok(Self {
            version: GENERATION_DECLARATION_CODEC_VERSION,
            digest: declaration.digest,
            characters,
            any_dialogue: declaration.any_dialogue,
            voice: declaration.voice,
            roles,
            style: *declaration.roles.style_ref(),
            custom_fields,
            presentation: WirePresentationRef {
                profile: declaration.presentation.profile(),
                revision: declaration.presentation.revision(),
                view_registry: declaration.presentation.view_registry_digest(),
                style_resource: declaration.presentation.style_resource_digest(),
            },
        })
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WireCharacterRef<'a> {
    character: &'a str,
    dialogue_type: RuntimeSemanticTypeId,
    visual: WireVisualRef,
    defaults: WireDefaultRef<'a>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WireVisualRef {
    Absent,
    Present {
        manifest: RuntimeValueDigest,
        look_type: RuntimeSemanticTypeId,
    },
}

impl WireVisualRef {
    fn from_visual<T>(visual: &CharacterDialogueVisualType<T>) -> Self
    where
        T: CharacterDialogueTypeReference,
    {
        match visual {
            CharacterDialogueVisualType::Absent => Self::Absent,
            CharacterDialogueVisualType::Present {
                manifest,
                look_type,
            } => Self::Present {
                manifest: *manifest,
                look_type: look_type.semantic_identity(),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WireDefaultRef<'a> {
    character: &'a CharacterId,
    config: WireConfigRef<'a>,
    expected_digest: Option<RuntimeValueDigest>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WireConfigRef<'a> {
    voice: Option<&'a CharacterDialogueVoice>,
    look: Option<&'a CharacterLookId>,
    stage: Option<WireTypedValue>,
    portrait: Option<WireTypedValue>,
    focus: Option<WireTypedValue>,
    cleanup: Option<WireTypedValue>,
    view: &'a ViewId,
    source_locale: Option<&'a crate::DialogueLocaleId>,
    hooks: Vec<WireTypedValue>,
    style: WireTypedValue,
    rich_text: WireTypedValue,
    inline_failure: crate::InlineFailurePolicy<WireTypedValue>,
    custom: Vec<WireConfigCustomRef<'a>>,
}

impl<'a> WireConfigRef<'a> {
    fn from_config(
        config: &'a CharacterDialogueConfig,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, CharacterDialogueGenerationDeclarationCodecError> {
        let custom = config
            .custom()
            .iter()
            .map(|(id, value)| {
                Ok(WireConfigCustomRef {
                    id,
                    value: WireTypedValue::capture(value.typed(), owner)?,
                })
            })
            .collect::<Result<_, CharacterDialogueGenerationDeclarationCodecError>>()?;
        Ok(Self {
            voice: config.voice(),
            look: config.look(),
            stage: config
                .stage()
                .map(|v| WireTypedValue::capture(v.typed(), owner))
                .transpose()?,
            portrait: config
                .portrait()
                .map(|v| WireTypedValue::capture(v.typed(), owner))
                .transpose()?,
            focus: config
                .focus()
                .map(|v| WireTypedValue::capture(v.typed(), owner))
                .transpose()?,
            cleanup: config
                .cleanup()
                .map(|v| WireTypedValue::capture(v.typed(), owner))
                .transpose()?,
            view: config.view(),
            source_locale: config.source_locale(),
            hooks: config
                .hooks()
                .iter()
                .map(|v| WireTypedValue::capture(v.typed(), owner))
                .collect::<Result<_, _>>()?,
            style: WireTypedValue::capture(config.style().typed(), owner)?,
            rich_text: WireTypedValue::capture(config.rich_text().typed(), owner)?,
            inline_failure: config
                .inline_failure()
                .borrow_styles()
                .try_map_styles(&mut |style| WireTypedValue::capture(style.typed(), owner))?,
            custom,
        })
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WireConfigCustomRef<'a> {
    id: &'a CharacterDialogueCustomFieldId,
    value: WireTypedValue,
}

/// The common Core DTO is the only recursive value grammar in this codec.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTypedValue {
    value: AwbcRuntimeValueSnapshot,
}

impl WireTypedValue {
    fn capture(
        value: &CharacterDialogueTypedValue,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, CharacterDialogueGenerationDeclarationCodecError> {
        Ok(Self {
            value: AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value.value(), owner)?,
        })
    }

    fn restore<T>(
        self,
        owner: &RuntimeProgramOwner,
        admit: impl FnOnce(CharacterDialogueTypedValue) -> Result<T, CharacterDialogueValueError>,
    ) -> Result<T, CharacterDialogueGenerationDeclarationCodecError> {
        let value = self.value.into_runtime_value_for_program(owner)?;
        Ok(admit(CharacterDialogueTypedValue::try_new(value)?)?)
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WireRoleRef {
    role: u8,
    value: RuntimeSemanticTypeId,
    body: WireRoleBodyRef,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WireRoleBodyRef {
    Unbound,
    Bound {
        payload: RuntimeSemanticTypeId,
        codec: u8,
    },
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WireCustomDescriptorRef<'a> {
    id: &'a CharacterDialogueCustomFieldId,
    semantic_type: RuntimeSemanticTypeId,
    clearable: bool,
    accepted_views: &'a BTreeSet<ViewId>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WirePresentationRef<'a> {
    profile: &'a crate::DialoguePresentationProfile,
    revision: &'a crate::DialogueProfileRevision,
    view_registry: RuntimeValueDigest,
    style_resource: Option<RuntimeValueDigest>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireGenerationOwned {
    version: u8,
    digest: RuntimeValueDigest,
    characters: Vec<WireCharacterOwned>,
    any_dialogue: RuntimeSemanticTypeId,
    voice: RuntimeSemanticTypeId,
    roles: Vec<WireRoleOwned>,
    style: RuntimeSemanticTypeId,
    custom_fields: Vec<WireCustomDescriptorOwned>,
    presentation: WirePresentationOwned,
}

impl WireGenerationOwned {
    fn into_declaration(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<
        CharacterDialogueGenerationDeclaration<RuntimeSemanticTypeId>,
        CharacterDialogueGenerationDeclarationCodecError,
    > {
        let mut characters = Vec::with_capacity(self.characters.len());
        for row in self.characters {
            let character = CharacterId::try_new(row.character).map_err(|error| {
                CharacterDialogueGenerationDeclarationCodecError::from(
                    CharacterDialogueValueError::Field {
                        field: "character_id",
                        reason: error.to_string(),
                    },
                )
            })?;
            let visual = match row.visual {
                WireVisualOwned::Absent => CharacterDialogueVisualType::Absent,
                WireVisualOwned::Present {
                    manifest,
                    look_type,
                } => CharacterDialogueVisualType::Present {
                    manifest,
                    look_type,
                },
            };
            let default_character =
                CharacterId::try_new(row.defaults.character).map_err(|error| {
                    CharacterDialogueGenerationDeclarationCodecError::from(
                        CharacterDialogueValueError::Field {
                            field: "defaults.character_id",
                            reason: error.to_string(),
                        },
                    )
                })?;
            let config = row.defaults.config.into_config(owner)?;
            let defaults = match row.defaults.expected_digest {
                Some(digest) => CharacterDialogueRuntimeDefault::with_expected_digest(
                    default_character,
                    config,
                    digest,
                ),
                None => CharacterDialogueRuntimeDefault::new(default_character, config),
            };
            characters.push((
                character,
                CharacterDialogueCharacterDeclaration::new(row.dialogue_type, visual, defaults),
            ));
        }

        if self.roles.len() != Role::AUTHORED_BASE.len() {
            return Err(CharacterDialogueGenerationDeclarationCodecError::NonCanonicalRoleOrder);
        }
        let mut roles = Vec::with_capacity(self.roles.len());
        for (expected_role, row) in Role::AUTHORED_BASE.into_iter().zip(self.roles) {
            if row.role != expected_role.canonical_tag() {
                return Err(
                    CharacterDialogueGenerationDeclarationCodecError::NonCanonicalRoleOrder,
                );
            }
            let body = match row.body {
                WireRoleBodyOwned::Unbound => CharacterDialogueRuntimeRoleBody::Unbound,
                WireRoleBodyOwned::Bound { payload, codec } => {
                    let codec = match codec {
                        0 => CharacterDialogueRolePayloadCodec::RichTextProperties,
                        _ => {
                            return Err(CharacterDialogueGenerationDeclarationCodecError::UnsupportedRoleCodec(codec));
                        }
                    };
                    CharacterDialogueRuntimeRoleBody::Bound { payload, codec }
                }
            };
            roles.push(CharacterDialogueRuntimeRoleType::new(row.value, body));
        }
        let authored_roles: [CharacterDialogueRuntimeRoleType; 6] = roles
            .try_into()
            .map_err(|_| CharacterDialogueGenerationDeclarationCodecError::NonCanonicalRoleOrder)?;
        let roles = CharacterDialogueRuntimeRoleTypes::new(authored_roles, self.style);

        let custom_fields = self
            .custom_fields
            .into_iter()
            .map(|field| {
                CharacterDialogueRuntimeCustomFieldDescriptor::new(
                    field.id,
                    field.semantic_type,
                    field.clearable,
                    field.accepted_views.into_iter().collect(),
                )
            })
            .collect::<Vec<_>>();
        let custom_fields = CharacterDialogueRuntimeCustomFieldCatalog::try_new(custom_fields)?;
        let presentation = CharacterDialoguePresentationContract::try_new(
            self.presentation.profile,
            self.presentation.revision,
            self.presentation.view_registry,
            self.presentation.style_resource,
        )?;

        Ok(CharacterDialogueGenerationDeclaration::try_new(
            characters,
            self.any_dialogue,
            self.voice,
            roles,
            custom_fields,
            presentation,
        )?)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCharacterOwned {
    character: String,
    dialogue_type: RuntimeSemanticTypeId,
    visual: WireVisualOwned,
    defaults: WireDefaultOwned,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireVisualOwned {
    Absent,
    Present {
        manifest: RuntimeValueDigest,
        look_type: RuntimeSemanticTypeId,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDefaultOwned {
    character: String,
    config: WireConfigOwned,
    expected_digest: Option<RuntimeValueDigest>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireConfigOwned {
    voice: Option<CharacterDialogueVoice>,
    look: Option<CharacterLookId>,
    stage: Option<WireTypedValue>,
    portrait: Option<WireTypedValue>,
    focus: Option<WireTypedValue>,
    cleanup: Option<WireTypedValue>,
    view: ViewId,
    source_locale: Option<crate::DialogueLocaleId>,
    hooks: Vec<WireTypedValue>,
    style: WireTypedValue,
    rich_text: WireTypedValue,
    inline_failure: crate::InlineFailurePolicy<WireTypedValue>,
    custom: Vec<WireConfigCustomOwned>,
}

impl WireConfigOwned {
    fn into_config(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<CharacterDialogueConfig, CharacterDialogueGenerationDeclarationCodecError> {
        let mut previous = None;
        let mut custom = BTreeMap::new();
        for row in self.custom {
            if previous
                .as_ref()
                .is_some_and(|previous| previous >= &row.id)
            {
                return Err(
                    CharacterDialogueGenerationDeclarationCodecError::NonCanonicalRows(
                        "default custom fields",
                    ),
                );
            }
            previous = Some(row.id.clone());
            custom.insert(
                row.id,
                row.value
                    .restore(owner, CharacterDialogueCustomValue::try_new)?,
            );
        }
        let mut config = CharacterDialogueConfig::try_new(
            self.view,
            self.style
                .restore(owner, CharacterDialogueStyleValue::try_new)?,
            self.rich_text
                .restore(owner, CharacterDialogueRichTextValue::try_new)?,
        )?;
        config.voice = self.voice;
        config.look = self.look;
        config.stage = self
            .stage
            .map(|v| v.restore(owner, CharacterDialogueStageValue::try_new))
            .transpose()?;
        config.portrait = self
            .portrait
            .map(|v| v.restore(owner, CharacterDialoguePortraitValue::try_new))
            .transpose()?;
        config.focus = self
            .focus
            .map(|v| v.restore(owner, CharacterDialogueFocusValue::try_new))
            .transpose()?;
        config.cleanup = self
            .cleanup
            .map(|v| v.restore(owner, CharacterDialogueCleanupValue::try_new))
            .transpose()?;
        config.source_locale = self.source_locale;
        config.hooks = self
            .hooks
            .into_iter()
            .map(|v| v.restore(owner, CharacterDialogueHookValue::try_new))
            .collect::<Result<_, _>>()?;
        config.inline_failure = self.inline_failure.try_map_styles(&mut |style| {
            style.restore(owner, CharacterDialogueStyleValue::try_new)
        })?;
        config.custom = custom;
        config.validate()?;
        Ok(config)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireConfigCustomOwned {
    id: CharacterDialogueCustomFieldId,
    value: WireTypedValue,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRoleOwned {
    role: u8,
    value: RuntimeSemanticTypeId,
    body: WireRoleBodyOwned,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireRoleBodyOwned {
    Unbound,
    Bound {
        payload: RuntimeSemanticTypeId,
        codec: u8,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCustomDescriptorOwned {
    id: CharacterDialogueCustomFieldId,
    semantic_type: RuntimeSemanticTypeId,
    clearable: bool,
    accepted_views: Vec<ViewId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePresentationOwned {
    profile: crate::DialoguePresentationProfile,
    revision: crate::DialogueProfileRevision,
    view_registry: RuntimeValueDigest,
    style_resource: Option<RuntimeValueDigest>,
}
