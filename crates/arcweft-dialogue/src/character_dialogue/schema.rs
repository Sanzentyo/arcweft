//! Program-bound admission and producer-owned tuple encoding of `CharacterDialogue`.

mod policies;
mod roles;

use super::{
    CharacterDialogue, CharacterDialogueCleanupValue, CharacterDialogueConfig,
    CharacterDialogueContractIdentity, CharacterDialogueCustomFieldId,
    CharacterDialogueCustomValue, CharacterDialogueFocusValue, CharacterDialogueHookValue,
    CharacterDialoguePortraitValue, CharacterDialogueRichTextValue,
    CharacterDialogueRuntimeRole as Role, CharacterDialogueStageValue, CharacterDialogueStyleValue,
    CharacterDialogueTypedValue, CharacterDialogueValueError, CharacterDialogueVoice,
    CharacterDialogueVoiceId, DialogueLocaleId, PRODUCTION_CHARACTER_DIALOGUE_LIMITS,
};
use crate::{FallbackStylePolicy, InlineFailurePolicy, InlineFallback};
use arcweft_character::{
    catalog::CharacterCatalog,
    id::{CharacterId, CharacterLookId},
};
use arcweft_core::{
    entry::{RuntimeSchemaLimits, RuntimeValueDigest},
    pattern::{
        RuntimeBuiltinVariantCaseIdentity, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId,
        RuntimeVariantIdentity,
    },
    program_types::RuntimeProgramTypes,
    value::{
        RuntimeEntityReference, RuntimeOpaqueValue, RuntimeSeq, RuntimeValue,
        runtime_sequence_dense_bytes,
    },
};
use arcweft_id::DeclarationIdentityFamily;
use arcweft_view::{ViewId, ViewRegistry};
use policies::{DialoguePolicyTypes, DialogueRuntimeVariantOwner};
pub use roles::{CharacterDialogueRuntimeRoleType, CharacterDialogueRuntimeRoleTypes};
use std::collections::{BTreeMap, BTreeSet};

const CHARACTER_DIALOGUE_FIELD_COUNT: usize = 18;

/// A field's source type reference and policy in one accepted bundle generation.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueRuntimeCustomFieldDescriptor {
    id: CharacterDialogueCustomFieldId,
    semantic_type: RuntimeSemanticTypeId,
    clearable: bool,
    accepted_views: BTreeSet<ViewId>,
}

/// Source catalog digest and its runtime field references. This is not a type table.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueRuntimeCustomFieldCatalog {
    digest: RuntimeValueDigest,
    fields: BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueRuntimeCustomFieldDescriptor>,
}

/// Producer context borrowing the active program and accepted generation inputs.
/// Type and payload references resolve only through that program. Defaults and
/// the source custom-catalog digest must come from the generation being loaded.
/// Construction checks structural consistency; it does not grant publication
/// authority to a compiler or bundle integrator.
pub struct CharacterDialogueRuntimeSchema<'a> {
    character_catalog: &'a CharacterCatalog,
    view_catalog: &'a ViewRegistry,
    custom_fields: &'a CharacterDialogueRuntimeCustomFieldCatalog,
    defaults: &'a BTreeMap<CharacterId, RuntimeValueDigest>,
    roles: &'a CharacterDialogueRuntimeRoleTypes,
    program: RuntimeProgramTypes<'a>,
    view_contracts: RuntimeValueDigest,
    policies: DialoguePolicyTypes,
}

/// Fully admitted domain value and its exact opaque runtime representation.
#[derive(Clone, Debug)]
pub struct CharacterDialogueValue {
    opaque: RuntimeOpaqueValue,
    dialogue: CharacterDialogue,
}

impl CharacterDialogueRuntimeCustomFieldDescriptor {
    pub fn new(
        id: CharacterDialogueCustomFieldId,
        semantic_type: RuntimeSemanticTypeId,
        clearable: bool,
        accepted_views: BTreeSet<ViewId>,
    ) -> Self {
        Self {
            id,
            semantic_type,
            clearable,
            accepted_views,
        }
    }
    pub const fn id(&self) -> &CharacterDialogueCustomFieldId {
        &self.id
    }
    pub const fn semantic_type(&self) -> RuntimeSemanticTypeId {
        self.semantic_type
    }
    pub const fn clearable(&self) -> bool {
        self.clearable
    }
    pub const fn accepted_views(&self) -> &BTreeSet<ViewId> {
        &self.accepted_views
    }
}

impl CharacterDialogueRuntimeCustomFieldCatalog {
    pub fn try_new(
        digest: RuntimeValueDigest,
        descriptors: impl IntoIterator<Item = CharacterDialogueRuntimeCustomFieldDescriptor>,
    ) -> Result<Self, CharacterDialogueValueError> {
        let mut fields = BTreeMap::new();
        for descriptor in descriptors {
            let id = descriptor.id.clone();
            if fields.insert(id.clone(), descriptor).is_some() {
                return Err(CharacterDialogueValueError::DuplicateCustomField(id));
            }
        }
        Ok(Self { digest, fields })
    }
    pub const fn digest(&self) -> RuntimeValueDigest {
        self.digest
    }
    pub const fn fields(
        &self,
    ) -> &BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueRuntimeCustomFieldDescriptor>
    {
        &self.fields
    }
    pub fn get(
        &self,
        id: &CharacterDialogueCustomFieldId,
    ) -> Option<&CharacterDialogueRuntimeCustomFieldDescriptor> {
        self.fields.get(id)
    }
}

impl<'a> CharacterDialogueRuntimeSchema<'a> {
    /// Sole producer for configuration roles and exact `CharacterDialogue` values.
    pub fn opaque_type_producer() -> RuntimeOpaqueTypeProducerId {
        super::runtime_type::character_dialogue_opaque_type_producer()
    }

    /// Resolves the complete role and custom type inventory before publishing a
    /// producer context. Recursive payloads remain references to program rows.
    pub fn try_new(
        character_catalog: &'a CharacterCatalog,
        view_catalog: &'a ViewRegistry,
        custom_fields: &'a CharacterDialogueRuntimeCustomFieldCatalog,
        defaults: &'a BTreeMap<CharacterId, RuntimeValueDigest>,
        roles: &'a CharacterDialogueRuntimeRoleTypes,
        program: RuntimeProgramTypes<'a>,
    ) -> Result<Self, CharacterDialogueValueError> {
        let rich_text = roles.validate(program)?;
        for descriptor in custom_fields.fields.values() {
            program.require_type(descriptor.semantic_type)?;
        }
        let view_contracts =
            RuntimeValueDigest::from_bytes(*view_catalog.runtime_digest_v1()?.as_bytes());
        let policies = DialoguePolicyTypes::try_new(
            rich_text,
            PRODUCTION_CHARACTER_DIALOGUE_LIMITS.runtime_schema_limits(),
        )?;
        Ok(Self {
            character_catalog,
            view_catalog,
            custom_fields,
            defaults,
            roles,
            program,
            view_contracts,
            policies,
        })
    }

    fn limits() -> RuntimeSchemaLimits {
        PRODUCTION_CHARACTER_DIALOGUE_LIMITS.runtime_schema_limits()
    }

    pub fn encode(
        &self,
        value: &CharacterDialogue,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError> {
        self.validate_dialogue(value)?;
        let payload = self.encode_payload(value);
        let owner =
            super::CharacterDialogueType::exact(value.character.clone()).runtime_opaque_owner();
        let runtime = owner.try_wrap(payload)?;
        runtime.try_digest_with_limits(Self::limits())?;
        let RuntimeValue::Opaque(opaque) = runtime else {
            unreachable!("exact owner wraps as opaque")
        };
        Ok(CharacterDialogueValue {
            opaque,
            dialogue: value.clone(),
        })
    }

    /// Decodes only the final exact opaque representation. Removed nominal
    /// wrappers have no reader. Character correlation precedes nested decoding.
    pub fn try_decode_opaque(
        &self,
        value: &RuntimeOpaqueValue,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError> {
        let expected_producer = Self::opaque_type_producer();
        if value.producer() != &expected_producer {
            return Err(CharacterDialogueValueError::OpaqueProducer {
                expected: expected_producer,
                actual: value.producer().clone(),
            });
        }
        let RuntimeValue::Tuple(fields) = value.payload() else {
            return Err(CharacterDialogueValueError::OpaquePayload);
        };
        if fields.len() != CHARACTER_DIALOGUE_FIELD_COUNT {
            return Err(CharacterDialogueValueError::OpaquePayload);
        }
        let RuntimeValue::EntityRef(RuntimeEntityReference::Project {
            family: DeclarationIdentityFamily::Character,
            public_id,
        }) = &fields[0]
        else {
            return Err(field_shape(
                "character_id",
                "expected Character entity reference",
            ));
        };
        let character = CharacterId::try_new(public_id.as_str())
            .map_err(|error| field_shape("character_id", error.to_string()))?;
        if self.character_catalog.get(&character).is_none() {
            return Err(CharacterDialogueValueError::MissingCharacter(character));
        }
        let expected = super::CharacterDialogueType::exact(character).runtime_opaque_owner();
        if expected.semantic_identity() != value.semantic_identity() {
            return Err(CharacterDialogueValueError::OpaqueSemanticIdentity {
                expected: expected.semantic_identity(),
                actual: value.semantic_identity(),
            });
        }
        if !expected.accepts_opaque_value(value) {
            return Err(CharacterDialogueValueError::OpaqueContract);
        }
        // Bound the complete input before any recursive domain normalization or clone.
        value.payload().try_digest_with_limits(Self::limits())?;
        let dialogue = self.decode_payload(fields)?;
        let canonical = self.encode(&dialogue)?;
        let input = RuntimeValue::Opaque(value.clone()).try_canonical_bytes(
            PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize,
        )?;
        let output = RuntimeValue::Opaque(canonical.opaque.clone()).try_canonical_bytes(
            PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize,
        )?;
        if input != output {
            return Err(field_shape(
                "runtime_payload",
                "value is not in canonical runtime form",
            ));
        }
        Ok(canonical)
    }

    pub fn canonical_bytes(
        &self,
        value: &CharacterDialogue,
    ) -> Result<Vec<u8>, CharacterDialogueValueError> {
        Ok(self
            .encode(value)?
            .into_runtime_value()
            .try_canonical_bytes(
                PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize,
            )?)
    }

    pub fn digest(
        &self,
        value: &CharacterDialogue,
    ) -> Result<RuntimeValueDigest, CharacterDialogueValueError> {
        Ok(self
            .encode(value)?
            .into_runtime_value()
            .try_digest(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize)?)
    }

    fn validate_dialogue(
        &self,
        dialogue: &CharacterDialogue,
    ) -> Result<(), CharacterDialogueValueError> {
        dialogue.config.validate()?;
        let manifest = self
            .character_catalog
            .get(&dialogue.character)
            .ok_or_else(|| {
                CharacterDialogueValueError::MissingCharacter(dialogue.character.clone())
            })?;
        if manifest.semantic_fingerprint_v1().as_bytes()
            != dialogue.contract.character_manifest().as_bytes()
        {
            return Err(CharacterDialogueValueError::CharacterManifestMismatch(
                dialogue.character.clone(),
            ));
        }
        let defaults = self.defaults.get(&dialogue.character).ok_or_else(|| {
            CharacterDialogueValueError::MissingDefaults(dialogue.character.clone())
        })?;
        if *defaults != dialogue.contract.defaults() {
            return Err(CharacterDialogueValueError::DefaultsMismatch(
                dialogue.character.clone(),
            ));
        }
        if dialogue.contract.view_contracts() != self.view_contracts {
            return Err(CharacterDialogueValueError::ViewContractsMismatch);
        }
        if let Some(look) = &dialogue.config.look
            && manifest.look(look).is_none()
        {
            return Err(CharacterDialogueValueError::MissingLook {
                character: dialogue.character.clone(),
                look: look.clone(),
            });
        }
        if self.view_catalog.resolve(&dialogue.config.view).is_none() {
            return Err(CharacterDialogueValueError::MissingView(
                dialogue.config.view.clone(),
            ));
        }
        if dialogue.contract.custom_schema() != self.custom_fields.digest {
            return Err(CharacterDialogueValueError::CustomSchemaMismatch);
        }
        let config = &dialogue.config;
        for (role, value) in [
            (
                Role::Stage,
                config
                    .stage
                    .as_ref()
                    .map(CharacterDialogueStageValue::typed),
            ),
            (
                Role::Portrait,
                config
                    .portrait
                    .as_ref()
                    .map(CharacterDialoguePortraitValue::typed),
            ),
            (
                Role::Focus,
                config
                    .focus
                    .as_ref()
                    .map(CharacterDialogueFocusValue::typed),
            ),
            (
                Role::Cleanup,
                config
                    .cleanup
                    .as_ref()
                    .map(CharacterDialogueCleanupValue::typed),
            ),
        ] {
            if let Some(value) = value {
                self.validate_role(role, value.value())?;
            }
        }
        for value in &config.hooks {
            self.validate_role(Role::Hook, value.typed().value())?;
        }
        self.validate_role(Role::Style, config.style.typed().value())?;
        self.validate_role(Role::RichText, config.rich_text.typed().value())?;
        if let InlineFailurePolicy::Fallback { fallback } = &config.inline_failure {
            let style = match fallback {
                InlineFallback::Text { style, .. }
                | InlineFallback::ExprSource { style }
                | InlineFallback::CallSource { style } => Some(style),
                InlineFallback::ValuePlain => None,
            };
            if let Some(FallbackStylePolicy::Apply { styles }) = style {
                for value in styles {
                    self.validate_role(Role::Style, value.typed().value())?;
                }
            }
        }
        for (id, value) in &config.custom {
            let descriptor = self
                .custom_fields
                .get(id)
                .ok_or_else(|| CharacterDialogueValueError::UnknownCustomField(id.clone()))?;
            if !descriptor.accepted_views.contains(&config.view) {
                return Err(CharacterDialogueValueError::CustomFieldView {
                    field: id.clone(),
                    view: config.view.clone(),
                });
            }
            self.program.accepts_value(
                descriptor.semantic_type,
                value.typed().value(),
                Self::limits(),
            )?;
        }
        Ok(())
    }
}

impl CharacterDialogueValue {
    pub const fn dialogue(&self) -> &CharacterDialogue {
        &self.dialogue
    }
    pub const fn opaque(&self) -> &RuntimeOpaqueValue {
        &self.opaque
    }
    pub fn into_runtime_value(self) -> RuntimeValue {
        RuntimeValue::Opaque(self.opaque)
    }
}

impl CharacterDialogueRuntimeSchema<'_> {
    fn encode_payload(&self, dialogue: &CharacterDialogue) -> RuntimeValue {
        let contract = dialogue.contract;
        let config = &dialogue.config;
        let fields = vec![
            RuntimeValue::EntityRef(RuntimeEntityReference::Project {
                family: DeclarationIdentityFamily::Character,
                public_id: dialogue.character.as_public_id(),
            }),
            Self::digest_value(contract.character_manifest()),
            Self::digest_value(contract.defaults()),
            Self::digest_value(contract.custom_schema()),
            Self::digest_value(contract.view_contracts()),
            Self::encode_option(config.voice.as_ref().map(|voice| self.encode_voice(voice))),
            Self::encode_option(
                config
                    .look
                    .as_ref()
                    .map(|look| RuntimeValue::String(look.as_str().to_owned())),
            ),
            Self::encode_typed_option(
                config
                    .stage
                    .as_ref()
                    .map(CharacterDialogueStageValue::typed),
            ),
            Self::encode_typed_option(
                config
                    .portrait
                    .as_ref()
                    .map(CharacterDialoguePortraitValue::typed),
            ),
            Self::encode_typed_option(
                config
                    .focus
                    .as_ref()
                    .map(CharacterDialogueFocusValue::typed),
            ),
            Self::encode_typed_option(
                config
                    .cleanup
                    .as_ref()
                    .map(CharacterDialogueCleanupValue::typed),
            ),
            RuntimeValue::EntityRef(RuntimeEntityReference::Project {
                family: DeclarationIdentityFamily::View,
                public_id: config.view.public_id().clone(),
            }),
            Self::encode_option(
                config
                    .source_locale
                    .as_ref()
                    .map(|locale| RuntimeValue::String(locale.as_str().to_owned())),
            ),
            RuntimeValue::Seq(RuntimeSeq::values(
                config
                    .hooks
                    .iter()
                    .map(|hook| hook.typed().value().clone())
                    .collect(),
            )),
            config.style.typed().value().clone(),
            config.rich_text.typed().value().clone(),
            self.encode_inline_failure(&config.inline_failure),
            Self::encode_custom(&config.custom),
        ];
        RuntimeValue::Tuple(fields)
    }

    fn decode_payload(
        &self,
        fields: &[RuntimeValue],
    ) -> Result<CharacterDialogue, CharacterDialogueValueError> {
        let RuntimeValue::EntityRef(RuntimeEntityReference::Project { family, public_id }) = fields
            .first()
            .ok_or_else(|| field_shape("character_id", "expected EntityRef"))?
        else {
            return Err(field_shape(
                "character_id",
                "expected Character entity reference",
            ));
        };
        if *family != DeclarationIdentityFamily::Character {
            return Err(field_shape(
                "character_id",
                "expected Character entity reference",
            ));
        }
        let character = CharacterId::try_new(public_id.as_str())
            .map_err(|error| field_shape("character_id", error.to_string()))?;
        let contract = CharacterDialogueContractIdentity::new(
            Self::decode_digest(&fields[1], "character_manifest_digest")?,
            Self::decode_digest(&fields[2], "defaults_digest")?,
            Self::decode_digest(&fields[3], "custom_schema_digest")?,
            Self::decode_digest(&fields[4], "view_contracts_digest")?,
        );
        let voice = Self::decode_option(&fields[5], "voice")?
            .map(|voice| self.decode_voice(voice))
            .transpose()?;
        let look = Self::decode_option(&fields[6], "look")?
            .map(|value| {
                let RuntimeValue::String(value) = value else {
                    return Err(field_shape("look", "expected String"));
                };
                CharacterLookId::try_new(value.clone())
                    .map_err(|error| field_shape("look", error.to_string()))
            })
            .transpose()?;
        let stage = Self::decode_typed_option(&fields[7], "stage")?
            .map(CharacterDialogueStageValue::try_new)
            .transpose()?;
        let portrait = Self::decode_typed_option(&fields[8], "portrait")?
            .map(CharacterDialoguePortraitValue::try_new)
            .transpose()?;
        let focus = Self::decode_typed_option(&fields[9], "focus")?
            .map(CharacterDialogueFocusValue::try_new)
            .transpose()?;
        let cleanup = Self::decode_typed_option(&fields[10], "cleanup")?
            .map(CharacterDialogueCleanupValue::try_new)
            .transpose()?;
        let RuntimeValue::EntityRef(RuntimeEntityReference::Project { family, public_id }) =
            &fields[11]
        else {
            return Err(field_shape("view", "expected View entity reference"));
        };
        if *family != DeclarationIdentityFamily::View {
            return Err(field_shape("view", "expected View entity reference"));
        }
        let view = ViewId::parse_public(public_id.as_str())
            .map_err(|error| field_shape("view", error.to_string()))?;
        let source_locale = Self::decode_option(&fields[12], "source_locale")?
            .map(|value| {
                let RuntimeValue::String(value) = value else {
                    return Err(field_shape("source_locale", "expected String"));
                };
                DialogueLocaleId::try_new(value.clone())
            })
            .transpose()?;
        let RuntimeValue::Seq(hooks) = &fields[13] else {
            return Err(field_shape("hooks", "expected Seq"));
        };
        let hooks = hooks
            .clone()
            .into_values()
            .into_iter()
            .map(|value| {
                CharacterDialogueTypedValue::try_new(value)
                    .and_then(CharacterDialogueHookValue::try_new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let style = CharacterDialogueStyleValue::try_new(CharacterDialogueTypedValue::try_new(
            fields[14].clone(),
        )?)?;
        let rich_text = CharacterDialogueRichTextValue::try_new(
            CharacterDialogueTypedValue::try_new(fields[15].clone())?,
        )?;
        let inline_failure = self.decode_inline_failure(&fields[16])?;
        let custom = Self::decode_custom(&fields[17])?;
        let config = CharacterDialogueConfig {
            voice,
            look,
            stage,
            portrait,
            focus,
            cleanup,
            view,
            source_locale,
            hooks,
            style,
            rich_text,
            inline_failure,
            custom,
        };
        CharacterDialogue::try_new(character, contract, config)
    }

    fn encode_voice(&self, voice: &CharacterDialogueVoice) -> RuntimeValue {
        match voice {
            CharacterDialogueVoice::Auto => {
                self.dialogue_variant(DialogueRuntimeVariantOwner::Voice, 0, "Auto", None)
            }
            CharacterDialogueVoice::Id(id) => self.dialogue_variant(
                DialogueRuntimeVariantOwner::Voice,
                1,
                "Id",
                Some(RuntimeValue::String(id.as_str().to_owned())),
            ),
        }
    }

    fn decode_voice(
        &self,
        value: &RuntimeValue,
    ) -> Result<CharacterDialogueVoice, CharacterDialogueValueError> {
        let RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            payload,
        } = value
        else {
            return Err(field_shape("voice", "expected DialogueVoice variant"));
        };
        self.expect_dialogue_variant_owner(owner, DialogueRuntimeVariantOwner::Voice, "voice")?;
        match (*ordinal, name.as_str(), payload.as_deref()) {
            (0, "Auto", None) => Ok(CharacterDialogueVoice::Auto),
            (1, "Id", Some(RuntimeValue::String(id))) => {
                CharacterDialogueVoiceId::try_new(id.clone()).map(CharacterDialogueVoice::Id)
            }
            _ => Err(field_shape("voice", "invalid DialogueVoice variant")),
        }
    }

    fn encode_typed_option(value: Option<&CharacterDialogueTypedValue>) -> RuntimeValue {
        Self::encode_option(value.map(|value| value.value().clone()))
    }

    fn decode_typed_option(
        value: &RuntimeValue,
        field: &'static str,
    ) -> Result<Option<CharacterDialogueTypedValue>, CharacterDialogueValueError> {
        Self::decode_option(value, field)?
            .cloned()
            .map(CharacterDialogueTypedValue::try_new)
            .transpose()
    }

    fn encode_option(value: Option<RuntimeValue>) -> RuntimeValue {
        match value {
            Some(value) => RuntimeValue::option_some(value),
            None => RuntimeValue::option_none(),
        }
    }

    fn decode_option<'a>(
        value: &'a RuntimeValue,
        field: &'static str,
    ) -> Result<Option<&'a RuntimeValue>, CharacterDialogueValueError> {
        match value.builtin_variant_case() {
            Some((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => Ok(None),
            Some((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => Ok(Some(value)),
            _ => Err(field_shape(field, "invalid Option payload")),
        }
    }

    fn encode_custom(
        custom: &BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueCustomValue>,
    ) -> RuntimeValue {
        RuntimeValue::Seq(RuntimeSeq::values(
            custom
                .iter()
                .map(|(id, value)| {
                    RuntimeValue::Tuple(vec![
                        RuntimeValue::String(id.as_str().to_owned()),
                        value.typed().value().clone(),
                    ])
                })
                .collect(),
        ))
    }

    fn decode_custom(
        value: &RuntimeValue,
    ) -> Result<
        BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueCustomValue>,
        CharacterDialogueValueError,
    > {
        let RuntimeValue::Seq(entries) = value else {
            return Err(field_shape("custom", "expected Seq"));
        };
        let mut custom = BTreeMap::new();
        let mut previous = None;
        for entry in entries.clone().into_values() {
            let RuntimeValue::Tuple(fields) = entry else {
                return Err(field_shape("custom", "expected two-element tuple"));
            };
            let [RuntimeValue::String(id), value] = fields.as_slice() else {
                return Err(field_shape("custom", "expected field ID and value"));
            };
            let id = CharacterDialogueCustomFieldId::try_new(id.clone())?;
            if previous.as_ref().is_some_and(|previous| previous >= &id) {
                return Err(CharacterDialogueValueError::NonCanonicalCustomOrder);
            }
            previous = Some(id.clone());
            let typed = CharacterDialogueTypedValue::try_new(value.clone())?;
            custom.insert(id, CharacterDialogueCustomValue::try_new(typed)?);
        }
        Ok(custom)
    }
    fn encode_inline_failure(&self, policy: &InlineFailurePolicy) -> RuntimeValue {
        match policy {
            InlineFailurePolicy::FailLine => self.dialogue_variant(
                DialogueRuntimeVariantOwner::InlineFailure,
                0,
                "FailLine",
                None,
            ),
            InlineFailurePolicy::Discard => self.dialogue_variant(
                DialogueRuntimeVariantOwner::InlineFailure,
                1,
                "Discard",
                None,
            ),
            InlineFailurePolicy::Fallback { fallback } => self.dialogue_variant(
                DialogueRuntimeVariantOwner::InlineFailure,
                2,
                "Fallback",
                Some(self.encode_fallback(fallback)),
            ),
        }
    }

    fn decode_inline_failure(
        &self,
        value: &RuntimeValue,
    ) -> Result<InlineFailurePolicy, CharacterDialogueValueError> {
        let RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            payload,
        } = value
        else {
            return Err(field_shape("inline_failure", "expected policy variant"));
        };
        self.expect_dialogue_variant_owner(
            owner,
            DialogueRuntimeVariantOwner::InlineFailure,
            "inline_failure",
        )?;
        match (*ordinal, name.as_str(), payload.as_deref()) {
            (0, "FailLine", None) => Ok(InlineFailurePolicy::FailLine),
            (1, "Discard", None) => Ok(InlineFailurePolicy::Discard),
            (2, "Fallback", Some(value)) => Ok(InlineFailurePolicy::Fallback {
                fallback: self.decode_fallback(value)?,
            }),
            _ => Err(field_shape("inline_failure", "invalid policy variant")),
        }
    }

    fn encode_fallback(&self, fallback: &InlineFallback) -> RuntimeValue {
        match fallback {
            InlineFallback::Text { text, style } => self.dialogue_variant(
                DialogueRuntimeVariantOwner::InlineFallback,
                0,
                "Text",
                Some(RuntimeValue::Tuple(vec![
                    RuntimeValue::String(text.clone()),
                    self.encode_fallback_style(style),
                ])),
            ),
            InlineFallback::ExprSource { style } => self.dialogue_variant(
                DialogueRuntimeVariantOwner::InlineFallback,
                1,
                "ExprSource",
                Some(self.encode_fallback_style(style)),
            ),
            InlineFallback::CallSource { style } => self.dialogue_variant(
                DialogueRuntimeVariantOwner::InlineFallback,
                2,
                "CallSource",
                Some(self.encode_fallback_style(style)),
            ),
            InlineFallback::ValuePlain => self.dialogue_variant(
                DialogueRuntimeVariantOwner::InlineFallback,
                3,
                "ValuePlain",
                None,
            ),
        }
    }

    fn decode_fallback(
        &self,
        value: &RuntimeValue,
    ) -> Result<InlineFallback, CharacterDialogueValueError> {
        let RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            payload,
        } = value
        else {
            return Err(field_shape("inline_failure", "expected fallback variant"));
        };
        self.expect_dialogue_variant_owner(
            owner,
            DialogueRuntimeVariantOwner::InlineFallback,
            "inline_failure",
        )?;
        match (*ordinal, name.as_str(), payload.as_deref()) {
            (0, "Text", Some(RuntimeValue::Tuple(values))) if values.len() == 2 => {
                let RuntimeValue::String(text) = &values[0] else {
                    return Err(field_shape(
                        "inline_failure",
                        "fallback text must be String",
                    ));
                };
                Ok(InlineFallback::Text {
                    text: text.clone(),
                    style: self.decode_fallback_style(&values[1])?,
                })
            }
            (1, "ExprSource", Some(style)) => Ok(InlineFallback::ExprSource {
                style: self.decode_fallback_style(style)?,
            }),
            (2, "CallSource", Some(style)) => Ok(InlineFallback::CallSource {
                style: self.decode_fallback_style(style)?,
            }),
            (3, "ValuePlain", None) => Ok(InlineFallback::ValuePlain),
            _ => Err(field_shape("inline_failure", "invalid fallback variant")),
        }
    }

    fn encode_fallback_style(&self, style: &FallbackStylePolicy) -> RuntimeValue {
        match style {
            FallbackStylePolicy::Plain => {
                self.dialogue_variant(DialogueRuntimeVariantOwner::FallbackStyle, 0, "Plain", None)
            }
            FallbackStylePolicy::InheritSurrounding => self.dialogue_variant(
                DialogueRuntimeVariantOwner::FallbackStyle,
                1,
                "InheritSurrounding",
                None,
            ),
            FallbackStylePolicy::Apply { styles } => self.dialogue_variant(
                DialogueRuntimeVariantOwner::FallbackStyle,
                2,
                "Apply",
                Some(RuntimeValue::Seq(RuntimeSeq::values(
                    styles
                        .iter()
                        .map(|style| style.typed().value().clone())
                        .collect(),
                ))),
            ),
        }
    }

    fn decode_fallback_style(
        &self,
        value: &RuntimeValue,
    ) -> Result<FallbackStylePolicy, CharacterDialogueValueError> {
        let RuntimeValue::Variant {
            owner,
            ordinal,
            name,
            payload,
        } = value
        else {
            return Err(field_shape(
                "inline_failure",
                "expected fallback style variant",
            ));
        };
        self.expect_dialogue_variant_owner(
            owner,
            DialogueRuntimeVariantOwner::FallbackStyle,
            "inline_failure",
        )?;
        match (*ordinal, name.as_str(), payload.as_deref()) {
            (0, "Plain", None) => Ok(FallbackStylePolicy::Plain),
            (1, "InheritSurrounding", None) => Ok(FallbackStylePolicy::InheritSurrounding),
            (2, "Apply", Some(RuntimeValue::Seq(styles))) => Ok(FallbackStylePolicy::Apply {
                styles: styles
                    .clone()
                    .into_values()
                    .into_iter()
                    .map(|value| {
                        CharacterDialogueTypedValue::try_new(value)
                            .and_then(CharacterDialogueStyleValue::try_new)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            _ => Err(field_shape(
                "inline_failure",
                "invalid fallback style variant",
            )),
        }
    }

    fn dialogue_variant(
        &self,
        owner: DialogueRuntimeVariantOwner,
        ordinal: u32,
        name: &str,
        payload: Option<RuntimeValue>,
    ) -> RuntimeValue {
        RuntimeValue::Variant {
            owner: self.policies.identity(owner).clone(),
            ordinal,
            name: name.to_owned(),
            payload: payload.map(Box::new),
        }
    }

    fn expect_dialogue_variant_owner(
        &self,
        actual: &RuntimeVariantIdentity,
        expected: DialogueRuntimeVariantOwner,
        field: &'static str,
    ) -> Result<(), CharacterDialogueValueError> {
        if actual == self.policies.identity(expected) {
            Ok(())
        } else {
            Err(field_shape(field, "variant has the wrong typed owner"))
        }
    }

    fn digest_value(value: RuntimeValueDigest) -> RuntimeValue {
        runtime_sequence_dense_bytes(value.as_bytes().to_vec())
    }

    fn decode_digest(
        value: &RuntimeValue,
        field: &'static str,
    ) -> Result<RuntimeValueDigest, CharacterDialogueValueError> {
        Self::decode_fixed_bytes(value, field).map(RuntimeValueDigest::from_bytes)
    }

    fn decode_fixed_bytes(
        value: &RuntimeValue,
        field: &'static str,
    ) -> Result<[u8; 32], CharacterDialogueValueError> {
        let RuntimeValue::Seq(sequence) = value else {
            return Err(field_shape(field, "expected dense u8[32]"));
        };
        let values = sequence.clone().into_values();
        if values.len() != 32 {
            return Err(field_shape(field, "expected exactly 32 bytes"));
        }
        let mut bytes = [0; 32];
        for (target, value) in bytes.iter_mut().zip(values) {
            let RuntimeValue::UInt(value) = value else {
                return Err(field_shape(field, "expected u8 values"));
            };
            *target = value
                .try_into_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| field_shape(field, "expected u8 values"))?;
        }
        Ok(bytes)
    }
}

fn field_shape(field: &'static str, reason: impl Into<String>) -> CharacterDialogueValueError {
    CharacterDialogueValueError::Field {
        field,
        reason: reason.into(),
    }
}
