//! First-class immutable Character dialogue runtime value.

mod identity;
mod limits;
mod patch;
mod schema;
mod typed_value;

use crate::{DialogueContent, InlineFailurePolicy, LinePlan};
use arcweft_character::id::{CharacterId, CharacterLookId};
use arcweft_core::{
    entry::{RuntimeValueDigest, TypeLayoutHash},
    plan::RuntimeLineId,
};
use arcweft_id::TextKey;
use arcweft_source::SourceAnchor;
use arcweft_view::ViewId;
use core::hash::{Hash, Hasher};
use std::collections::BTreeMap;
use thiserror::Error;

use self::limits::{MAX_LOCAL_ID_BYTES, MAX_PUBLIC_ID_BYTES};

pub use identity::{
    CharacterDialogueContractIdentity, CharacterDialogueCustomFieldId, CharacterDialogueVoice,
    CharacterDialogueVoiceId, DialogueLocaleId,
};
pub use limits::{CharacterDialogueLimits, PRODUCTION_CHARACTER_DIALOGUE_LIMITS};
pub use patch::{CharacterDialoguePatch, PatchField, RuntimeFieldPath, StructuredPatch};
pub use schema::{
    CharacterDialogueRuntimeCustomFieldCatalog, CharacterDialogueRuntimeCustomFieldDescriptor,
    CharacterDialogueRuntimeSchema, CharacterDialogueValue,
};
pub use typed_value::{
    CharacterDialogueCleanupValue, CharacterDialogueCustomValue, CharacterDialogueFocusValue,
    CharacterDialogueHookValue, CharacterDialoguePortraitValue, CharacterDialogueRichTextValue,
    CharacterDialogueStageValue, CharacterDialogueStyleValue, CharacterDialogueTypedValue,
};

/// Immutable character-owned dialogue configuration.
///
/// The provisional speaker/preset builder API is intentionally absent:
///
/// ```compile_fail
/// use arcweft_dialogue::{SayOptions, SpeakerPreset, SpeakerRef};
/// ```
///
/// ```compile_fail
/// use arcweft_dialogue::VoicePolicy;
/// ```
///
/// ```compile_fail
/// use arcweft_dialogue::DialogueLineBuilder;
/// ```
///
/// ```compile_fail
/// use arcweft_dialogue::CharacterDialogue;
///
/// fn removed_say(dialogue: CharacterDialogue) {
///     let _ = dialogue.say("removed");
/// }
/// ```
#[derive(Clone, Debug)]
pub struct CharacterDialogue {
    pub(super) character: CharacterId,
    pub(super) layout: TypeLayoutHash,
    pub(super) contract: CharacterDialogueContractIdentity,
    pub(super) config: CharacterDialogueConfig,
}

/// Effective reusable configuration stored by one [`CharacterDialogue`].
#[derive(Clone, Debug)]
pub struct CharacterDialogueConfig {
    pub(super) voice: Option<CharacterDialogueVoice>,
    pub(super) look: Option<CharacterLookId>,
    pub(super) stage: Option<CharacterDialogueStageValue>,
    pub(super) portrait: Option<CharacterDialoguePortraitValue>,
    pub(super) focus: Option<CharacterDialogueFocusValue>,
    pub(super) cleanup: Option<CharacterDialogueCleanupValue>,
    pub(super) view: ViewId,
    pub(super) source_locale: Option<DialogueLocaleId>,
    pub(super) hooks: Vec<CharacterDialogueHookValue>,
    pub(super) style: CharacterDialogueStyleValue,
    pub(super) rich_text: CharacterDialogueRichTextValue,
    pub(super) inline_failure: InlineFailurePolicy,
    pub(super) custom: BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueCustomValue>,
}

/// One source-owned content application after reusable configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueContentApplication {
    dialogue: CharacterDialogue,
    line: RuntimeLineId,
    text_key: TextKey,
    content: DialogueContent,
    plan: LinePlan,
    source: SourceAnchor,
}

/// `CharacterDialogue` construction, patch, or runtime-schema failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CharacterDialogueValueError {
    #[error("invalid {kind} identity `{value}`")]
    Identity { kind: &'static str, value: String },
    #[error("invalid dialogue locale `{value}`: {reason}")]
    Locale { value: String, reason: &'static str },
    #[error("CharacterDialogue field `{field}` has invalid runtime shape: {reason}")]
    Field { field: &'static str, reason: String },
    #[error("CharacterDialogue structured patch contains overlapping paths")]
    OverlappingStructuredPaths,
    #[error("CharacterDialogue exceeds `{limit}` limit {maximum}")]
    Limit { limit: &'static str, maximum: usize },
    #[error("character `{0}` is not present in the accepted character catalog")]
    MissingCharacter(CharacterId),
    #[error("character `{0}` manifest digest does not match the runtime value")]
    CharacterManifestMismatch(CharacterId),
    #[error("character `{character}` has no look `{look}`")]
    MissingLook {
        character: CharacterId,
        look: CharacterLookId,
    },
    #[error("View `{0}` is not present in the accepted View catalog")]
    MissingView(ViewId),
    #[error("CharacterDialogue custom-schema digest is stale")]
    CustomSchemaMismatch,
    #[error("unknown CharacterDialogue custom field `{0}`")]
    UnknownCustomField(CharacterDialogueCustomFieldId),
    #[error("duplicate CharacterDialogue custom-field descriptor `{0}`")]
    DuplicateCustomField(CharacterDialogueCustomFieldId),
    #[error("CharacterDialogue custom entries are not in canonical field-id order")]
    NonCanonicalCustomOrder,
    #[error("custom field `{0}` has the wrong declared nominal type or layout")]
    CustomFieldType(CharacterDialogueCustomFieldId),
    #[error("custom field `{field}` is not accepted by View `{view}`")]
    CustomFieldView {
        field: CharacterDialogueCustomFieldId,
        view: ViewId,
    },
    #[error(transparent)]
    Nominal(#[from] arcweft_core::value::RuntimeNominalRecordError),
    #[error(transparent)]
    RuntimeSchema(#[from] arcweft_core::entry::RuntimeSchemaError),
}

impl CharacterDialogue {
    /// Constructs a validated immutable value from one accepted defaults
    /// snapshot. Catalog-dependent checks are performed by the runtime schema.
    pub fn try_new(
        character: CharacterId,
        layout: TypeLayoutHash,
        contract: CharacterDialogueContractIdentity,
        config: CharacterDialogueConfig,
    ) -> Result<Self, CharacterDialogueValueError> {
        config.validate()?;
        let dialogue = Self {
            character,
            layout,
            contract,
            config,
        };
        dialogue.canonical_bytes()?;
        Ok(dialogue)
    }

    #[must_use]
    pub const fn character(&self) -> &CharacterId {
        &self.character
    }

    #[must_use]
    pub const fn layout(&self) -> TypeLayoutHash {
        self.layout
    }

    #[must_use]
    pub const fn contract(&self) -> CharacterDialogueContractIdentity {
        self.contract
    }

    #[must_use]
    pub const fn config(&self) -> &CharacterDialogueConfig {
        &self.config
    }

    /// Applies a complete checked patch to a cloned config.
    ///
    /// The receiver is never mutated, including when validation fails.
    pub fn patched(
        &self,
        patch: &CharacterDialoguePatch,
    ) -> Result<Self, CharacterDialogueValueError> {
        let config = patch::apply_patch(&self.config, patch)?;
        Self::try_new(self.character.clone(), self.layout, self.contract, config)
    }

    /// Canonical value digest used by equality, hashing, persistence, and
    /// stale-contract checks.
    pub fn digest(&self) -> Result<RuntimeValueDigest, CharacterDialogueValueError> {
        let record = schema::encode_record(self)?;
        Ok(arcweft_core::value::RuntimeValue::NominalRecord(record)
            .try_digest(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize)?)
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, CharacterDialogueValueError> {
        let record = schema::encode_record(self)?;
        Ok(
            arcweft_core::value::RuntimeValue::NominalRecord(record).try_canonical_bytes(
                PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize,
            )?,
        )
    }
}

impl PartialEq for CharacterDialogue {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_bytes()
            .expect("validated CharacterDialogue remains canonically encodable")
            == other
                .canonical_bytes()
                .expect("validated CharacterDialogue remains canonically encodable")
    }
}

impl Eq for CharacterDialogue {}

impl Hash for CharacterDialogue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.digest()
            .expect("validated CharacterDialogue remains canonically hashable")
            .as_bytes()
            .hash(state);
    }
}

impl CharacterDialogueConfig {
    /// Constructs the minimal required configuration. All optional roles are
    /// absent and inline failures default to failing the line.
    pub fn try_new(
        view: ViewId,
        style: CharacterDialogueStyleValue,
        rich_text: CharacterDialogueRichTextValue,
    ) -> Result<Self, CharacterDialogueValueError> {
        let config = Self {
            voice: None,
            look: None,
            stage: None,
            portrait: None,
            focus: None,
            cleanup: None,
            view,
            source_locale: None,
            hooks: Vec::new(),
            style,
            rich_text,
            inline_failure: InlineFailurePolicy::FailLine,
            custom: BTreeMap::new(),
        };
        config.validate()?;
        Ok(config)
    }

    #[must_use]
    pub const fn voice(&self) -> Option<&CharacterDialogueVoice> {
        self.voice.as_ref()
    }

    #[must_use]
    pub const fn look(&self) -> Option<&CharacterLookId> {
        self.look.as_ref()
    }

    #[must_use]
    pub const fn stage(&self) -> Option<&CharacterDialogueStageValue> {
        self.stage.as_ref()
    }

    #[must_use]
    pub const fn portrait(&self) -> Option<&CharacterDialoguePortraitValue> {
        self.portrait.as_ref()
    }

    #[must_use]
    pub const fn focus(&self) -> Option<&CharacterDialogueFocusValue> {
        self.focus.as_ref()
    }

    #[must_use]
    pub const fn cleanup(&self) -> Option<&CharacterDialogueCleanupValue> {
        self.cleanup.as_ref()
    }

    #[must_use]
    pub const fn view(&self) -> &ViewId {
        &self.view
    }

    #[must_use]
    pub const fn source_locale(&self) -> Option<&DialogueLocaleId> {
        self.source_locale.as_ref()
    }

    #[must_use]
    pub fn hooks(&self) -> &[CharacterDialogueHookValue] {
        &self.hooks
    }

    #[must_use]
    pub const fn style(&self) -> &CharacterDialogueStyleValue {
        &self.style
    }

    #[must_use]
    pub const fn rich_text(&self) -> &CharacterDialogueRichTextValue {
        &self.rich_text
    }

    #[must_use]
    pub const fn inline_failure(&self) -> &InlineFailurePolicy {
        &self.inline_failure
    }

    #[must_use]
    pub const fn custom(
        &self,
    ) -> &BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueCustomValue> {
        &self.custom
    }

    pub(super) fn validate(&self) -> Result<(), CharacterDialogueValueError> {
        if self.hooks.len() > usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_hooks) {
            return Err(CharacterDialogueValueError::Limit {
                limit: "hooks",
                maximum: usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_hooks),
            });
        }
        if self.custom.len() > usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_custom_fields) {
            return Err(CharacterDialogueValueError::Limit {
                limit: "custom_fields",
                maximum: usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_custom_fields),
            });
        }
        if self.view.as_str().len() > MAX_PUBLIC_ID_BYTES {
            return Err(CharacterDialogueValueError::Limit {
                limit: "view_id_bytes",
                maximum: MAX_PUBLIC_ID_BYTES,
            });
        }
        if let Some(CharacterDialogueVoice::Id(voice)) = &self.voice
            && voice.as_str().len() > MAX_PUBLIC_ID_BYTES
        {
            return Err(CharacterDialogueValueError::Limit {
                limit: "voice_id_bytes",
                maximum: MAX_PUBLIC_ID_BYTES,
            });
        }
        if let Some(look) = &self.look
            && look.as_str().len() > MAX_LOCAL_ID_BYTES
        {
            return Err(CharacterDialogueValueError::Limit {
                limit: "look_id_bytes",
                maximum: MAX_LOCAL_ID_BYTES,
            });
        }
        typed_value::validate_config_value_limits(self)?;
        Ok(())
    }
}

impl CharacterDialogueContentApplication {
    pub fn try_new(
        dialogue: CharacterDialogue,
        line: RuntimeLineId,
        text_key: TextKey,
        content: DialogueContent,
        plan: LinePlan,
        source: SourceAnchor,
    ) -> Result<Self, CharacterDialogueValueError> {
        let maximum = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_line_id_bytes);
        if line.public_label().as_str().len() > maximum {
            return Err(CharacterDialogueValueError::Limit {
                limit: "line_id_bytes",
                maximum,
            });
        }
        if text_key.as_str().len() > maximum {
            return Err(CharacterDialogueValueError::Limit {
                limit: "text_key_bytes",
                maximum,
            });
        }
        Ok(Self {
            dialogue,
            line,
            text_key,
            content,
            plan,
            source,
        })
    }

    #[must_use]
    pub const fn dialogue(&self) -> &CharacterDialogue {
        &self.dialogue
    }

    #[must_use]
    pub const fn line(&self) -> &RuntimeLineId {
        &self.line
    }

    #[must_use]
    pub const fn text_key(&self) -> &TextKey {
        &self.text_key
    }

    #[must_use]
    pub const fn content(&self) -> &DialogueContent {
        &self.content
    }

    #[must_use]
    pub const fn plan(&self) -> &LinePlan {
        &self.plan
    }

    #[must_use]
    pub const fn source(&self) -> &SourceAnchor {
        &self.source
    }
}
