//! Context-owned `CharacterDialogue` runtime record encoding and decoding.

use super::{
    CharacterDialogue, CharacterDialogueCleanupValue, CharacterDialogueConfig,
    CharacterDialogueContractIdentity, CharacterDialogueCustomFieldId,
    CharacterDialogueCustomValue, CharacterDialogueFocusValue, CharacterDialogueHookValue,
    CharacterDialoguePortraitValue, CharacterDialogueRichTextValue, CharacterDialogueStageValue,
    CharacterDialogueStyleValue, CharacterDialogueTypedValue, CharacterDialogueValueError,
    CharacterDialogueVoice, CharacterDialogueVoiceId, DialogueLocaleId,
};
use crate::{FallbackStylePolicy, InlineFailurePolicy, InlineFallback};
use arcweft_character::{
    catalog::CharacterCatalog,
    id::{CharacterId, CharacterLookId},
};
use arcweft_core::{
    entry::{
        RuntimeBytesFormat, RuntimeNominalTypeId, RuntimeSchemaField, RuntimeTypeSchema,
        RuntimeValueDigest, TypeLayoutHash,
    },
    pattern::{RuntimeSemanticTypeId, RuntimeVariantIdentity},
    value::{RuntimeNominalRecordValue, RuntimeSeq, RuntimeValue, runtime_sequence_dense_bytes},
};
use arcweft_view::{ViewId, ViewRegistry};
use std::collections::{BTreeMap, BTreeSet};

const CHARACTER_DIALOGUE_FIELD_COUNT: usize = 18;
const CUSTOM_ENTRY_FIELD_COUNT: usize = 4;

#[derive(Clone, Copy)]
enum DialogueRuntimeVariantOwner {
    Voice,
    InlineFailure,
    InlineFallback,
    FallbackStyle,
}

impl DialogueRuntimeVariantOwner {
    const fn public_id(self) -> &'static str {
        match self {
            Self::Voice => "arcweft.dialogue.CharacterDialogueVoice",
            Self::InlineFailure => "arcweft.dialogue.InlineFailurePolicy",
            Self::InlineFallback => "arcweft.dialogue.InlineFallback",
            Self::FallbackStyle => "arcweft.dialogue.FallbackStylePolicy",
        }
    }

    const fn semantic_digest(self) -> [u8; 32] {
        match self {
            // SHA-256 of the versioned canonical owner labels. The bytes are
            // frozen schema identity, not source or display spellings.
            Self::Voice => [
                0x76, 0x53, 0x13, 0x17, 0x90, 0x11, 0xc8, 0xe7, 0x34, 0x93, 0xbe, 0xbe, 0x4e, 0xc0,
                0x4d, 0x05, 0x6a, 0xd3, 0xd5, 0xcd, 0x6a, 0xbd, 0xd3, 0x94, 0x9b, 0x0f, 0x8a, 0x36,
                0x9e, 0x3c, 0x6a, 0x4d,
            ],
            Self::InlineFailure => [
                0x5c, 0xfa, 0x09, 0xb9, 0xb5, 0x88, 0x19, 0x62, 0xe9, 0xdd, 0xe3, 0x22, 0xfb, 0xe5,
                0x50, 0xa8, 0x7a, 0x2b, 0xcc, 0x6f, 0xe1, 0x98, 0xc0, 0xe4, 0xd8, 0x51, 0x51, 0xb2,
                0x24, 0x8d, 0xed, 0x77,
            ],
            Self::InlineFallback => [
                0xb0, 0x2c, 0xfa, 0x28, 0x38, 0xd6, 0xf8, 0x30, 0x9e, 0x47, 0xad, 0xab, 0x77, 0xf1,
                0x24, 0xea, 0x90, 0x2a, 0xb6, 0xea, 0xa4, 0xa6, 0xf9, 0x88, 0xfa, 0xec, 0x56, 0x58,
                0x28, 0x50, 0x69, 0xc5,
            ],
            Self::FallbackStyle => [
                0x89, 0xa6, 0x0b, 0xba, 0xba, 0x9b, 0x88, 0xe0, 0x84, 0x03, 0x27, 0x37, 0xd8, 0x0e,
                0x27, 0xa3, 0xf4, 0xdd, 0x5a, 0x63, 0xeb, 0x3b, 0xec, 0x51, 0xcc, 0x7d, 0x5b, 0xd9,
                0x09, 0x82, 0xc1, 0xe2,
            ],
        }
    }

    fn identity(self) -> RuntimeVariantIdentity {
        RuntimeVariantIdentity::Nominal {
            nominal: RuntimeNominalTypeId::try_new(self.public_id())
                .expect("dialogue runtime variant owner IDs are valid"),
            semantic_identity: RuntimeSemanticTypeId::from_bytes(self.semantic_digest()),
        }
    }
}

/// Runtime-only custom-field descriptor accepted with one bundle generation.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueRuntimeCustomFieldDescriptor {
    id: CharacterDialogueCustomFieldId,
    nominal_type: Option<RuntimeNominalTypeId>,
    layout: TypeLayoutHash,
    clearable: bool,
    accepted_views: BTreeSet<ViewId>,
}

/// Immutable runtime custom-field catalog and semantic digest.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueRuntimeCustomFieldCatalog {
    digest: RuntimeValueDigest,
    fields: BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueRuntimeCustomFieldDescriptor>,
}

/// Context required to validate the Cut 1 `CharacterDialogue` domain carrier.
///
/// Exact role nominal identities and layouts are validated by the accepted
/// AWBC type table introduced in Cut 4.
pub struct CharacterDialogueRuntimeSchema<'a> {
    character_catalog: &'a CharacterCatalog,
    view_catalog: &'a ViewRegistry,
    custom_fields: &'a CharacterDialogueRuntimeCustomFieldCatalog,
    expected_layout: TypeLayoutHash,
}

/// Canonical Cut 1 nominal carrier paired with its validated domain value.
#[derive(Clone, Debug)]
pub struct CharacterDialogueValue {
    record: RuntimeNominalRecordValue,
    dialogue: CharacterDialogue,
}

impl CharacterDialogueRuntimeCustomFieldDescriptor {
    #[must_use]
    pub fn new(
        id: CharacterDialogueCustomFieldId,
        nominal_type: Option<RuntimeNominalTypeId>,
        layout: TypeLayoutHash,
        clearable: bool,
        accepted_views: BTreeSet<ViewId>,
    ) -> Self {
        Self {
            id,
            nominal_type,
            layout,
            clearable,
            accepted_views,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &CharacterDialogueCustomFieldId {
        &self.id
    }

    #[must_use]
    pub const fn nominal_type(&self) -> Option<&RuntimeNominalTypeId> {
        self.nominal_type.as_ref()
    }

    #[must_use]
    pub const fn layout(&self) -> TypeLayoutHash {
        self.layout
    }

    #[must_use]
    pub const fn clearable(&self) -> bool {
        self.clearable
    }

    #[must_use]
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

    #[must_use]
    pub const fn digest(&self) -> RuntimeValueDigest {
        self.digest
    }

    #[must_use]
    pub const fn fields(
        &self,
    ) -> &BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueRuntimeCustomFieldDescriptor>
    {
        &self.fields
    }

    #[must_use]
    pub fn get(
        &self,
        id: &CharacterDialogueCustomFieldId,
    ) -> Option<&CharacterDialogueRuntimeCustomFieldDescriptor> {
        self.fields.get(id)
    }
}

impl<'a> CharacterDialogueRuntimeSchema<'a> {
    #[must_use]
    pub const fn new(
        character_catalog: &'a CharacterCatalog,
        view_catalog: &'a ViewRegistry,
        custom_fields: &'a CharacterDialogueRuntimeCustomFieldCatalog,
        expected_layout: TypeLayoutHash,
    ) -> Self {
        Self {
            character_catalog,
            view_catalog,
            custom_fields,
            expected_layout,
        }
    }

    pub fn decode(
        &self,
        value: &RuntimeNominalRecordValue,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError> {
        value.validate_shape(
            &character_dialogue_type_id(),
            self.expected_layout,
            CHARACTER_DIALOGUE_FIELD_COUNT,
        )?;
        let dialogue = decode_record(value)?;
        self.validate_dialogue(&dialogue)?;
        let canonical = encode_record(&dialogue)?;
        if canonical != *value {
            return Err(CharacterDialogueValueError::Field {
                field: "runtime_record",
                reason: "record is not in canonical runtime form".to_owned(),
            });
        }
        Ok(CharacterDialogueValue {
            // `RuntimeValue` equality intentionally treats `-0.0` and `0.0`
            // as equal. Retain the re-encoded value so the accepted carrier
            // cannot diverge from its normalized domain representation.
            record: canonical,
            dialogue,
        })
    }

    pub fn encode(
        &self,
        value: &CharacterDialogue,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError> {
        self.validate_dialogue(value)?;
        let record = encode_record(value)?;
        record.validate_shape(
            &character_dialogue_type_id(),
            self.expected_layout,
            CHARACTER_DIALOGUE_FIELD_COUNT,
        )?;
        Ok(CharacterDialogueValue {
            record,
            dialogue: value.clone(),
        })
    }

    fn validate_dialogue(
        &self,
        dialogue: &CharacterDialogue,
    ) -> Result<(), CharacterDialogueValueError> {
        if dialogue.layout != self.expected_layout {
            return Err(arcweft_core::value::RuntimeNominalRecordError::Layout {
                expected: self.expected_layout,
                actual: dialogue.layout,
            }
            .into());
        }
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
        for (id, value) in &dialogue.config.custom {
            let descriptor = self
                .custom_fields
                .get(id)
                .ok_or_else(|| CharacterDialogueValueError::UnknownCustomField(id.clone()))?;
            if descriptor.nominal_type.as_ref() != value.typed().nominal_type()
                || descriptor.layout != value.typed().layout()
            {
                return Err(CharacterDialogueValueError::CustomFieldType(id.clone()));
            }
            if !descriptor.accepted_views.contains(&dialogue.config.view) {
                return Err(CharacterDialogueValueError::CustomFieldView {
                    field: id.clone(),
                    view: dialogue.config.view.clone(),
                });
            }
        }
        dialogue.config.validate()
    }
}

impl CharacterDialogueValue {
    #[must_use]
    pub const fn dialogue(&self) -> &CharacterDialogue {
        &self.dialogue
    }

    #[must_use]
    pub const fn record(&self) -> &RuntimeNominalRecordValue {
        &self.record
    }

    #[must_use]
    pub fn into_runtime_value(self) -> RuntimeValue {
        RuntimeValue::NominalRecord(self.record)
    }
}

pub(super) fn encode_record(
    dialogue: &CharacterDialogue,
) -> Result<RuntimeNominalRecordValue, CharacterDialogueValueError> {
    let contract = dialogue.contract;
    let config = &dialogue.config;
    let fields = vec![
        RuntimeValue::EntityRef(dialogue.character.as_str().to_owned()),
        digest_value(contract.character_manifest()),
        digest_value(contract.defaults()),
        digest_value(contract.custom_schema()),
        digest_value(contract.view_contracts()),
        encode_option(config.voice.as_ref().map(encode_voice)),
        encode_option(
            config
                .look
                .as_ref()
                .map(|look| RuntimeValue::String(look.as_str().to_owned())),
        ),
        encode_typed_option(
            config
                .stage
                .as_ref()
                .map(CharacterDialogueStageValue::typed),
        ),
        encode_typed_option(
            config
                .portrait
                .as_ref()
                .map(CharacterDialoguePortraitValue::typed),
        ),
        encode_typed_option(
            config
                .focus
                .as_ref()
                .map(CharacterDialogueFocusValue::typed),
        ),
        encode_typed_option(
            config
                .cleanup
                .as_ref()
                .map(CharacterDialogueCleanupValue::typed),
        ),
        RuntimeValue::EntityRef(config.view.as_str().to_owned()),
        encode_option(
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
        encode_inline_failure(&config.inline_failure),
        encode_custom(&config.custom),
    ];
    let record =
        RuntimeNominalRecordValue::new(character_dialogue_type_id(), dialogue.layout, fields);
    RuntimeValue::NominalRecord(record.clone()).try_canonical_bytes(
        super::PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize,
    )?;
    Ok(record)
}

fn decode_record(
    record: &RuntimeNominalRecordValue,
) -> Result<CharacterDialogue, CharacterDialogueValueError> {
    let fields = record.fields();
    let character = fields
        .first()
        .and_then(RuntimeValue::as_identifier)
        .ok_or_else(|| field_shape("character_id", "expected EntityRef"))
        .and_then(|value| {
            CharacterId::try_new(value.to_owned())
                .map_err(|error| field_shape("character_id", error.to_string()))
        })?;
    let contract = CharacterDialogueContractIdentity::new(
        decode_digest(&fields[1], "character_manifest_digest")?,
        decode_digest(&fields[2], "defaults_digest")?,
        decode_digest(&fields[3], "custom_schema_digest")?,
        decode_digest(&fields[4], "view_contracts_digest")?,
    );
    let voice = decode_option(&fields[5], "voice")?
        .map(decode_voice)
        .transpose()?;
    let look = decode_option(&fields[6], "look")?
        .map(|value| {
            let RuntimeValue::String(value) = value else {
                return Err(field_shape("look", "expected String"));
            };
            CharacterLookId::try_new(value.clone())
                .map_err(|error| field_shape("look", error.to_string()))
        })
        .transpose()?;
    let stage = decode_typed_option(&fields[7], "stage")?
        .map(CharacterDialogueStageValue::try_new)
        .transpose()?;
    let portrait = decode_typed_option(&fields[8], "portrait")?
        .map(CharacterDialoguePortraitValue::try_new)
        .transpose()?;
    let focus = decode_typed_option(&fields[9], "focus")?
        .map(CharacterDialogueFocusValue::try_new)
        .transpose()?;
    let cleanup = decode_typed_option(&fields[10], "cleanup")?
        .map(CharacterDialogueCleanupValue::try_new)
        .transpose()?;
    let view = fields[11]
        .as_identifier()
        .ok_or_else(|| field_shape("view", "expected EntityRef"))
        .and_then(|value| {
            ViewId::parse_public(value.to_owned())
                .map_err(|error| field_shape("view", error.to_string()))
        })?;
    let source_locale = decode_option(&fields[12], "source_locale")?
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
            typed_from_nominal(value, "hooks").and_then(CharacterDialogueHookValue::try_new)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let style =
        CharacterDialogueStyleValue::try_new(typed_from_nominal(fields[14].clone(), "style")?)?;
    let rich_text = CharacterDialogueRichTextValue::try_new(typed_from_nominal(
        fields[15].clone(),
        "rich_text",
    )?)?;
    let inline_failure = decode_inline_failure(&fields[16])?;
    let custom = decode_custom(&fields[17])?;
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
    CharacterDialogue::try_new(character, record.layout(), contract, config)
}

fn encode_voice(voice: &CharacterDialogueVoice) -> RuntimeValue {
    match voice {
        CharacterDialogueVoice::Auto => {
            dialogue_variant(DialogueRuntimeVariantOwner::Voice, 0, "Auto", None)
        }
        CharacterDialogueVoice::Id(id) => dialogue_variant(
            DialogueRuntimeVariantOwner::Voice,
            1,
            "Id",
            Some(RuntimeValue::EntityRef(id.as_str().to_owned())),
        ),
    }
}

fn decode_voice(
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
    expect_dialogue_variant_owner(owner, DialogueRuntimeVariantOwner::Voice, "voice")?;
    match (*ordinal, name.as_str(), payload.as_deref()) {
        (0, "Auto", None) => Ok(CharacterDialogueVoice::Auto),
        (1, "Id", Some(RuntimeValue::EntityRef(id))) => {
            CharacterDialogueVoiceId::try_new(id.clone()).map(CharacterDialogueVoice::Id)
        }
        _ => Err(field_shape("voice", "invalid DialogueVoice variant")),
    }
}

fn encode_typed_option(value: Option<&CharacterDialogueTypedValue>) -> RuntimeValue {
    encode_option(value.map(|value| value.value().clone()))
}

fn decode_typed_option(
    value: &RuntimeValue,
    field: &'static str,
) -> Result<Option<CharacterDialogueTypedValue>, CharacterDialogueValueError> {
    decode_option(value, field)?
        .cloned()
        .map(|value| typed_from_nominal(value, field))
        .transpose()
}

fn typed_from_nominal(
    value: RuntimeValue,
    field: &'static str,
) -> Result<CharacterDialogueTypedValue, CharacterDialogueValueError> {
    let RuntimeValue::NominalRecord(record) = &value else {
        return Err(field_shape(field, "expected nominal record"));
    };
    CharacterDialogueTypedValue::try_new(Some(record.type_id().clone()), record.layout(), value)
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
    let RuntimeValue::Variant {
        owner,
        ordinal,
        name,
        payload,
    } = value
    else {
        return Err(field_shape(field, "expected Option variant"));
    };
    match (owner, *ordinal, name.as_str(), payload.as_deref()) {
        (RuntimeVariantIdentity::Option, 1, "None", None) => Ok(None),
        (RuntimeVariantIdentity::Option, 0, "Some", Some(value)) => Ok(Some(value)),
        _ => Err(field_shape(field, "invalid Option payload")),
    }
}

fn encode_custom(
    custom: &BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueCustomValue>,
) -> RuntimeValue {
    let mut entries = Vec::with_capacity(custom.len());
    for (id, value) in custom {
        let typed = value.typed();
        entries.push(RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            custom_entry_type_id(),
            custom_entry_layout(),
            vec![
                RuntimeValue::String(id.as_str().to_owned()),
                encode_option(
                    typed
                        .nominal_type()
                        .map(|id| RuntimeValue::String(id.as_str().to_owned())),
                ),
                layout_value(typed.layout()),
                typed.value().clone(),
            ],
        )));
    }
    RuntimeValue::Seq(RuntimeSeq::values(entries))
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
    let mut previous: Option<CharacterDialogueCustomFieldId> = None;
    for entry in entries.clone().into_values() {
        let RuntimeValue::NominalRecord(entry) = entry else {
            return Err(field_shape("custom", "expected nominal custom entry"));
        };
        entry.validate_shape(
            &custom_entry_type_id(),
            custom_entry_layout(),
            CUSTOM_ENTRY_FIELD_COUNT,
        )?;
        let fields = entry.fields();
        let RuntimeValue::String(id) = &fields[0] else {
            return Err(field_shape("custom.field_id", "expected String"));
        };
        let id = CharacterDialogueCustomFieldId::try_new(id.clone())?;
        if previous.as_ref().is_some_and(|previous| previous >= &id) {
            return Err(CharacterDialogueValueError::NonCanonicalCustomOrder);
        }
        previous = Some(id.clone());
        let nominal_type = decode_option(&fields[1], "custom.declared_nominal_type")?
            .map(|value| {
                let RuntimeValue::String(value) = value else {
                    return Err(field_shape(
                        "custom.declared_nominal_type",
                        "expected String",
                    ));
                };
                RuntimeNominalTypeId::try_new(value.clone())
                    .map_err(|error| field_shape("custom.declared_nominal_type", error.to_string()))
            })
            .transpose()?;
        let layout = decode_layout(&fields[2], "custom.declared_layout")?;
        let typed = CharacterDialogueTypedValue::try_new(nominal_type, layout, fields[3].clone())?;
        if custom
            .insert(id.clone(), CharacterDialogueCustomValue::try_new(typed)?)
            .is_some()
        {
            return Err(CharacterDialogueValueError::DuplicateCustomField(id));
        }
    }
    Ok(custom)
}

fn encode_inline_failure(policy: &InlineFailurePolicy) -> RuntimeValue {
    let value = match policy {
        InlineFailurePolicy::FailLine => dialogue_variant(
            DialogueRuntimeVariantOwner::InlineFailure,
            0,
            "FailLine",
            None,
        ),
        InlineFailurePolicy::Discard => dialogue_variant(
            DialogueRuntimeVariantOwner::InlineFailure,
            1,
            "Discard",
            None,
        ),
        InlineFailurePolicy::Fallback { fallback } => dialogue_variant(
            DialogueRuntimeVariantOwner::InlineFailure,
            2,
            "Fallback",
            Some(encode_fallback(fallback)),
        ),
    };
    RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
        inline_failure_type_id(),
        inline_failure_layout(),
        vec![value],
    ))
}

fn decode_inline_failure(
    value: &RuntimeValue,
) -> Result<InlineFailurePolicy, CharacterDialogueValueError> {
    let RuntimeValue::NominalRecord(record) = value else {
        return Err(field_shape("inline_failure", "expected nominal record"));
    };
    record.validate_shape(&inline_failure_type_id(), inline_failure_layout(), 1)?;
    let RuntimeValue::Variant {
        owner,
        ordinal,
        name,
        payload,
    } = &record.fields()[0]
    else {
        return Err(field_shape("inline_failure", "expected policy variant"));
    };
    expect_dialogue_variant_owner(
        owner,
        DialogueRuntimeVariantOwner::InlineFailure,
        "inline_failure",
    )?;
    match (*ordinal, name.as_str(), payload.as_deref()) {
        (0, "FailLine", None) => Ok(InlineFailurePolicy::FailLine),
        (1, "Discard", None) => Ok(InlineFailurePolicy::Discard),
        (2, "Fallback", Some(value)) => Ok(InlineFailurePolicy::Fallback {
            fallback: decode_fallback(value)?,
        }),
        _ => Err(field_shape("inline_failure", "invalid policy variant")),
    }
}

fn encode_fallback(fallback: &InlineFallback) -> RuntimeValue {
    match fallback {
        InlineFallback::Text { text, style } => dialogue_variant(
            DialogueRuntimeVariantOwner::InlineFallback,
            0,
            "Text",
            Some(RuntimeValue::Tuple(vec![
                RuntimeValue::String(text.clone()),
                encode_fallback_style(style),
            ])),
        ),
        InlineFallback::ExprSource { style } => dialogue_variant(
            DialogueRuntimeVariantOwner::InlineFallback,
            1,
            "ExprSource",
            Some(encode_fallback_style(style)),
        ),
        InlineFallback::CallSource { style } => dialogue_variant(
            DialogueRuntimeVariantOwner::InlineFallback,
            2,
            "CallSource",
            Some(encode_fallback_style(style)),
        ),
        InlineFallback::ValuePlain => dialogue_variant(
            DialogueRuntimeVariantOwner::InlineFallback,
            3,
            "ValuePlain",
            None,
        ),
    }
}

fn decode_fallback(value: &RuntimeValue) -> Result<InlineFallback, CharacterDialogueValueError> {
    let RuntimeValue::Variant {
        owner,
        ordinal,
        name,
        payload,
    } = value
    else {
        return Err(field_shape("inline_failure", "expected fallback variant"));
    };
    expect_dialogue_variant_owner(
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
                style: decode_fallback_style(&values[1])?,
            })
        }
        (1, "ExprSource", Some(style)) => Ok(InlineFallback::ExprSource {
            style: decode_fallback_style(style)?,
        }),
        (2, "CallSource", Some(style)) => Ok(InlineFallback::CallSource {
            style: decode_fallback_style(style)?,
        }),
        (3, "ValuePlain", None) => Ok(InlineFallback::ValuePlain),
        _ => Err(field_shape("inline_failure", "invalid fallback variant")),
    }
}

fn encode_fallback_style(style: &FallbackStylePolicy) -> RuntimeValue {
    match style {
        FallbackStylePolicy::Plain => {
            dialogue_variant(DialogueRuntimeVariantOwner::FallbackStyle, 0, "Plain", None)
        }
        FallbackStylePolicy::InheritSurrounding => dialogue_variant(
            DialogueRuntimeVariantOwner::FallbackStyle,
            1,
            "InheritSurrounding",
            None,
        ),
        FallbackStylePolicy::Apply { styles } => dialogue_variant(
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
    expect_dialogue_variant_owner(
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
                    typed_from_nominal(value, "inline_failure.style")
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
    owner: DialogueRuntimeVariantOwner,
    ordinal: u32,
    name: &str,
    payload: Option<RuntimeValue>,
) -> RuntimeValue {
    RuntimeValue::Variant {
        owner: owner.identity(),
        ordinal,
        name: name.to_owned(),
        payload: payload.map(Box::new),
    }
}

fn expect_dialogue_variant_owner(
    actual: &RuntimeVariantIdentity,
    expected: DialogueRuntimeVariantOwner,
    field: &'static str,
) -> Result<(), CharacterDialogueValueError> {
    if actual == &expected.identity() {
        Ok(())
    } else {
        Err(field_shape(field, "variant has the wrong typed owner"))
    }
}

fn digest_value(value: RuntimeValueDigest) -> RuntimeValue {
    runtime_sequence_dense_bytes(value.as_bytes().to_vec())
}

fn layout_value(value: TypeLayoutHash) -> RuntimeValue {
    runtime_sequence_dense_bytes(value.as_bytes().to_vec())
}

fn decode_digest(
    value: &RuntimeValue,
    field: &'static str,
) -> Result<RuntimeValueDigest, CharacterDialogueValueError> {
    decode_fixed_bytes(value, field).map(RuntimeValueDigest::from_bytes)
}

fn decode_layout(
    value: &RuntimeValue,
    field: &'static str,
) -> Result<TypeLayoutHash, CharacterDialogueValueError> {
    decode_fixed_bytes(value, field).map(TypeLayoutHash::from_bytes)
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

fn character_dialogue_type_id() -> RuntimeNominalTypeId {
    RuntimeNominalTypeId::try_new("std.character_dialogue")
        .expect("reserved CharacterDialogue nominal identity is valid")
}

fn custom_entry_type_id() -> RuntimeNominalTypeId {
    RuntimeNominalTypeId::try_new("std.character_dialogue_custom_entry")
        .expect("reserved custom-entry nominal identity is valid")
}

fn inline_failure_type_id() -> RuntimeNominalTypeId {
    RuntimeNominalTypeId::try_new("std.inline_failure_policy")
        .expect("reserved inline-failure nominal identity is valid")
}

fn custom_entry_layout() -> TypeLayoutHash {
    RuntimeTypeSchema::Record {
        name: "std.character_dialogue_custom_entry".to_owned(),
        fields: vec![
            schema_field("field_id", RuntimeTypeSchema::String),
            schema_field(
                "declared_nominal_type",
                RuntimeTypeSchema::Option(Box::new(RuntimeTypeSchema::String)),
            ),
            schema_field(
                "declared_layout",
                RuntimeTypeSchema::Bytes {
                    format: RuntimeBytesFormat::Array,
                },
            ),
            schema_field("value", RuntimeTypeSchema::Named("Dynamic".to_owned())),
        ],
        deny_unknown_fields: true,
    }
    .try_layout_hash()
    .expect("fixed custom-entry schema has a canonical layout")
}

fn inline_failure_layout() -> TypeLayoutHash {
    RuntimeTypeSchema::Record {
        name: "std.inline_failure_policy".to_owned(),
        fields: vec![schema_field(
            "policy",
            RuntimeTypeSchema::Named("InlineFailurePolicy".to_owned()),
        )],
        deny_unknown_fields: true,
    }
    .try_layout_hash()
    .expect("fixed inline-failure schema has a canonical layout")
}

fn schema_field(name: &str, schema: RuntimeTypeSchema) -> RuntimeSchemaField {
    RuntimeSchemaField {
        rust_name: name.to_owned(),
        wire_name: name.to_owned(),
        schema,
        has_default: false,
        skip: false,
        bytes_format: None,
    }
}

fn field_shape(field: &'static str, reason: impl Into<String>) -> CharacterDialogueValueError {
    CharacterDialogueValueError::Field {
        field,
        reason: reason.into(),
    }
}
