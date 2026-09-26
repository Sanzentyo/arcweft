//! Program-bound admission and producer-owned tuple encoding of `CharacterDialogue`.

mod policies;
mod role_payload;
mod roles;

use super::{
    CharacterDialogue, CharacterDialogueCleanupValue, CharacterDialogueConfig,
    CharacterDialogueContractIdentity, CharacterDialogueCustomFieldId,
    CharacterDialogueCustomValue, CharacterDialogueFocusValue, CharacterDialogueHookValue,
    CharacterDialoguePortraitValue, CharacterDialogueRichTextValue,
    CharacterDialogueRuntimeRole as Role, CharacterDialogueStageValue, CharacterDialogueStyleValue,
    CharacterDialogueTypedValue, CharacterDialogueValueError,
    CharacterDialogueVisualManifestEvidence, CharacterDialogueVoice, CharacterDialogueVoiceId,
    DialogueLocaleId, PRODUCTION_CHARACTER_DIALOGUE_LIMITS,
};
use crate::{FallbackStylePolicy, InlineFailurePolicy, InlineFallback};
use arcweft_character::{
    catalog::CharacterVisualManifestEvidence,
    id::{CharacterId, CharacterLookId},
};
use arcweft_core::{
    character_nominal::{RuntimeCharacterLookSourceAuthority, RuntimeCharacterLookSourceError},
    entry::{RuntimeSchemaLimits, RuntimeValueDigest},
    pattern::{
        RuntimeBuiltinVariantCaseIdentity, RuntimeCheckedType, RuntimeOpaqueTypeOwner,
        RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId, RuntimeVariantIdentity,
    },
    program_types::RuntimeProgramTypes,
    task::RuntimeProgramOwner,
    value::{
        RuntimeEntityReference, RuntimeOpaquePersistence, RuntimeOpaqueValue,
        RuntimeOpaqueValueClass, RuntimeSeq, RuntimeValue, runtime_sequence_dense_bytes,
    },
};
use arcweft_id::DeclarationIdentityFamily;
use arcweft_interaction_model::dialogue::{
    CharacterDialogueFieldCoordinate, CharacterDialogueOperation, CharacterDialoguePatchField,
    CharacterDialoguePatchOperation,
};
use arcweft_view::{ViewId, ViewRegistry};
use policies::CharacterDialoguePolicyCase;
pub use policies::{
    CharacterDialoguePolicyCaseSpec, CharacterDialoguePolicyTypeGraph,
    CharacterDialoguePolicyTypeSchema, CharacterDialoguePolicyVariantOwner,
};
pub use role_payload::{
    CharacterDialogueRichTextColor, CharacterDialogueRichTextProperties,
    CharacterDialogueRichTextProperty, CharacterDialogueRichTextPropertyValue,
    CharacterDialogueRolePayloadCodec, CharacterDialogueRolePayloadSchema,
};
pub use roles::{
    CharacterDialogueRuntimeRoleBody, CharacterDialogueRuntimeRoleType,
    CharacterDialogueRuntimeRoleTypes,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use super::generation::{CharacterDialogueGenerationDeclaration, CharacterDialogueTypeReference};

const CHARACTER_DIALOGUE_FIELD_COUNT: usize = 18;
const CUSTOM_FIELD_CATALOG_DIGEST_DOMAIN: &[u8] =
    b"arcweft.character-dialogue.custom-field-catalog.v1\0";

/// A field's source type reference and policy in one accepted bundle generation.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueRuntimeCustomFieldDescriptor<T = RuntimeSemanticTypeId> {
    id: CharacterDialogueCustomFieldId,
    semantic_type: T,
    clearable: bool,
    accepted_views: BTreeSet<ViewId>,
}

/// Source catalog digest and its runtime field references. This is not a type table.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueRuntimeCustomFieldCatalog<T = RuntimeSemanticTypeId> {
    digest: RuntimeValueDigest,
    fields:
        BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueRuntimeCustomFieldDescriptor<T>>,
}

/// Effective default configuration and its accepted generation digest for one
/// logical Character declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeDefault {
    character: CharacterId,
    config: CharacterDialogueConfig,
    expected_digest: Option<RuntimeValueDigest>,
}

impl CharacterDialogueRuntimeDefault {
    #[must_use]
    pub const fn new(character: CharacterId, config: CharacterDialogueConfig) -> Self {
        Self {
            character,
            config,
            expected_digest: None,
        }
    }

    #[must_use]
    pub const fn with_expected_digest(
        character: CharacterId,
        config: CharacterDialogueConfig,
        digest: RuntimeValueDigest,
    ) -> Self {
        Self {
            character,
            config,
            expected_digest: Some(digest),
        }
    }

    #[must_use]
    pub const fn character(&self) -> &CharacterId {
        &self.character
    }

    #[must_use]
    pub const fn expected_digest(&self) -> Option<RuntimeValueDigest> {
        self.expected_digest
    }

    #[must_use]
    pub const fn config(&self) -> &CharacterDialogueConfig {
        &self.config
    }
}

/// Complete accepted default configuration rows owned by one runtime generation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeDefaultCatalog {
    defaults: BTreeMap<CharacterId, CharacterDialogueRuntimeDefault>,
}

impl CharacterDialogueRuntimeDefaultCatalog {
    pub fn try_new(
        defaults: impl IntoIterator<Item = CharacterDialogueRuntimeDefault>,
    ) -> Result<Self, CharacterDialogueValueError> {
        let mut rows = BTreeMap::new();
        for default in defaults {
            let character = default.character.clone();
            if rows.insert(character.clone(), default).is_some() {
                return Err(CharacterDialogueValueError::DuplicateDefaults(character));
            }
        }
        Ok(Self { defaults: rows })
    }

    #[must_use]
    pub fn get(&self, character: &CharacterId) -> Option<&CharacterDialogueRuntimeDefault> {
        self.defaults.get(character)
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = &CharacterDialogueRuntimeDefault> {
        self.defaults.values()
    }
}

/// Generation-owned CharacterDialogue producer and admission context.
///
/// It owns the exact executable lease and shared immutable catalogs/defaults;
/// all type lookup and value validation derive from that lease.
pub struct CharacterDialogueRuntimeSchema {
    generation_digest: RuntimeValueDigest,
    view_catalog: Arc<ViewRegistry>,
    custom_fields: Arc<CharacterDialogueRuntimeCustomFieldCatalog>,
    defaults: Arc<CharacterDialogueRuntimeDefaultCatalog>,
    roles: CharacterDialogueRuntimeRoleTypes,
    voice_source_type: RuntimeSemanticTypeId,
    look_source_authority: Arc<RuntimeCharacterLookSourceAuthority>,
    program_owner: RuntimeProgramOwner,
    view_contracts: RuntimeValueDigest,
    policies: CharacterDialoguePolicyTypeGraph,
}

impl std::fmt::Debug for CharacterDialogueRuntimeSchema {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CharacterDialogueRuntimeSchema")
            .field("generation_digest", &self.generation_digest)
            .finish_non_exhaustive()
    }
}

/// Fully admitted domain value and its exact opaque runtime representation.
#[derive(Clone, Debug)]
pub struct CharacterDialogueValue {
    opaque: RuntimeOpaqueValue,
    dialogue: CharacterDialogue,
}

impl<T> CharacterDialogueRuntimeCustomFieldDescriptor<T> {
    pub fn new(
        id: CharacterDialogueCustomFieldId,
        semantic_type: T,
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
    pub const fn semantic_type_ref(&self) -> &T {
        &self.semantic_type
    }
    pub const fn clearable(&self) -> bool {
        self.clearable
    }
    pub const fn accepted_views(&self) -> &BTreeSet<ViewId> {
        &self.accepted_views
    }
}

impl<T: Copy> CharacterDialogueRuntimeCustomFieldDescriptor<T> {
    pub const fn semantic_type(&self) -> T {
        self.semantic_type
    }
}

impl<T: CharacterDialogueTypeReference> CharacterDialogueRuntimeCustomFieldCatalog<T> {
    pub fn try_new(
        descriptors: impl IntoIterator<Item = CharacterDialogueRuntimeCustomFieldDescriptor<T>>,
    ) -> Result<Self, CharacterDialogueValueError> {
        let mut fields = BTreeMap::new();
        for descriptor in descriptors {
            let id = descriptor.id.clone();
            if fields.contains_key(&id) {
                return Err(CharacterDialogueValueError::DuplicateCustomField(id));
            }
            Self::validate_descriptor_limits(&descriptor)?;
            if fields.len() >= usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_custom_fields) {
                return Err(CharacterDialogueValueError::Limit {
                    limit: "custom_fields",
                    maximum: usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_custom_fields),
                });
            }
            fields.insert(id, descriptor);
        }
        let digest = Self::digest_fields(&fields)?;
        Ok(Self { digest, fields })
    }

    /// Constructs the catalog only when its computed descriptor digest matches
    /// the digest advertised by an enclosing generation or bundle.
    pub fn try_new_with_expected_digest(
        expected: RuntimeValueDigest,
        descriptors: impl IntoIterator<Item = CharacterDialogueRuntimeCustomFieldDescriptor<T>>,
    ) -> Result<Self, CharacterDialogueValueError> {
        let catalog = Self::try_new(descriptors)?;
        if catalog.digest != expected {
            return Err(CharacterDialogueValueError::CustomSchemaMismatch);
        }
        Ok(catalog)
    }

    pub const fn digest(&self) -> RuntimeValueDigest {
        self.digest
    }
    pub const fn fields(
        &self,
    ) -> &BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueRuntimeCustomFieldDescriptor<T>>
    {
        &self.fields
    }
    pub fn get(
        &self,
        id: &CharacterDialogueCustomFieldId,
    ) -> Option<&CharacterDialogueRuntimeCustomFieldDescriptor<T>> {
        self.fields.get(id)
    }

    pub fn visit_type_refs<'a>(&'a self, visit: &mut impl FnMut(&'a T)) {
        self.fields
            .values()
            .for_each(|descriptor| visit(descriptor.semantic_type_ref()));
    }

    fn validate_descriptor_limits(
        descriptor: &CharacterDialogueRuntimeCustomFieldDescriptor<T>,
    ) -> Result<(), CharacterDialogueValueError> {
        let limits = PRODUCTION_CHARACTER_DIALOGUE_LIMITS;
        if descriptor.id.as_str().len() > usize::from(limits.max_custom_field_id_bytes) {
            return Err(CharacterDialogueValueError::Limit {
                limit: "custom_field_id_bytes",
                maximum: usize::from(limits.max_custom_field_id_bytes),
            });
        }
        if descriptor.accepted_views.len() > limits.max_values_per_sequence as usize {
            return Err(CharacterDialogueValueError::Limit {
                limit: "custom_field_accepted_views",
                maximum: limits.max_values_per_sequence as usize,
            });
        }
        if descriptor
            .accepted_views
            .iter()
            .any(|view| view.as_str().len() > usize::from(limits.max_public_id_bytes))
        {
            return Err(CharacterDialogueValueError::Limit {
                limit: "custom_field_view_id_bytes",
                maximum: usize::from(limits.max_public_id_bytes),
            });
        }
        Ok(())
    }

    fn digest_fields(
        fields: &BTreeMap<
            CharacterDialogueCustomFieldId,
            CharacterDialogueRuntimeCustomFieldDescriptor<T>,
        >,
    ) -> Result<RuntimeValueDigest, CharacterDialogueValueError> {
        let mut encoder = CustomFieldCatalogDigestEncoder::new();
        encoder.write_len(fields.len(), "custom_fields")?;
        for descriptor in fields.values() {
            encoder.write_str(descriptor.id.as_str(), "custom_field_id_bytes")?;
            encoder.write_bytes(
                descriptor.semantic_type.semantic_identity().as_bytes(),
                "custom_catalog_bytes",
            )?;
            encoder.write_u8(u8::from(descriptor.clearable), "custom_catalog_bytes")?;
            encoder.write_len(
                descriptor.accepted_views.len(),
                "custom_field_accepted_views",
            )?;
            for view in &descriptor.accepted_views {
                encoder.write_str(view.as_str(), "custom_field_view_id_bytes")?;
            }
        }
        Ok(encoder.finish())
    }
}

/// Computes the same typed default digest used by runtime admission, without
/// requiring an executable program lease. A caller's expected digest is
/// verified against this result and never supplies it.
pub(super) fn effective_default_config_digest<T: CharacterDialogueTypeReference>(
    config: &CharacterDialogueConfig,
    roles: &CharacterDialogueRuntimeRoleTypes<T>,
) -> Result<RuntimeValueDigest, CharacterDialogueValueError> {
    config.validate()?;
    let rich_text_index = Role::AUTHORED_BASE
        .iter()
        .position(|role| *role == Role::RichText)
        .expect("RichText has a fixed authored role slot");
    let rich_text_identity = roles.authored_refs()[rich_text_index]
        .value_ref()
        .semantic_identity();
    let rich_text_owner = RuntimeOpaqueTypeOwner::exact_with(
        CharacterDialogueRuntimeSchema::opaque_type_producer(),
        rich_text_identity,
        RuntimeOpaqueValueClass::Plain,
        RuntimeOpaquePersistence::ConstantAndSnapshot,
    );
    let policies = CharacterDialoguePolicyTypeGraph::try_new(
        rich_text_owner,
        PRODUCTION_CHARACTER_DIALOGUE_LIMITS.runtime_schema_limits(),
    )?;
    RuntimeValue::Tuple(CharacterDialogueRuntimeSchema::encode_config_fields(
        config, &policies,
    ))
    .try_digest_with_limits(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.runtime_schema_limits())
    .map_err(CharacterDialogueValueError::from)
}

struct CustomFieldCatalogDigestEncoder {
    hasher: blake3::Hasher,
    encoded_len: usize,
}

impl CustomFieldCatalogDigestEncoder {
    fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CUSTOM_FIELD_CATALOG_DIGEST_DOMAIN);
        Self {
            hasher,
            encoded_len: CUSTOM_FIELD_CATALOG_DIGEST_DOMAIN.len(),
        }
    }

    fn write_len(
        &mut self,
        value: usize,
        limit: &'static str,
    ) -> Result<(), CharacterDialogueValueError> {
        let value = u32::try_from(value).map_err(|_| CharacterDialogueValueError::Limit {
            limit,
            maximum: u32::MAX as usize,
        })?;
        self.write_bytes(&value.to_le_bytes(), "custom_catalog_bytes")
    }

    fn write_str(
        &mut self,
        value: &str,
        limit: &'static str,
    ) -> Result<(), CharacterDialogueValueError> {
        self.write_len(value.len(), limit)?;
        self.write_bytes(value.as_bytes(), limit)
    }

    fn write_u8(
        &mut self,
        value: u8,
        limit: &'static str,
    ) -> Result<(), CharacterDialogueValueError> {
        self.write_bytes(&[value], limit)
    }

    fn write_bytes(
        &mut self,
        bytes: &[u8],
        limit: &'static str,
    ) -> Result<(), CharacterDialogueValueError> {
        let maximum = PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_config_encoded_bytes as usize;
        let Some(next_len) = self.encoded_len.checked_add(bytes.len()) else {
            return Err(CharacterDialogueValueError::Limit { limit, maximum });
        };
        if next_len > maximum {
            return Err(CharacterDialogueValueError::Limit { limit, maximum });
        }
        self.hasher.update(bytes);
        self.encoded_len = next_len;
        Ok(())
    }

    fn finish(self) -> RuntimeValueDigest {
        RuntimeValueDigest::from_bytes(*self.hasher.finalize().as_bytes())
    }
}

impl CharacterDialogueRuntimeSchema {
    /// Sole producer for configuration roles and exact `CharacterDialogue` values.
    pub fn opaque_type_producer() -> RuntimeOpaqueTypeProducerId {
        super::runtime_type::character_dialogue_opaque_type_producer()
    }

    /// Completes schema admission after the declaration has checked actual
    /// resources and executable roots. No caller supplies parallel role or
    /// default inventories.
    pub(super) fn try_bind_generation(
        generation: &CharacterDialogueGenerationDeclaration<RuntimeSemanticTypeId>,
        view_catalog: Arc<ViewRegistry>,
        look_source_authority: Arc<RuntimeCharacterLookSourceAuthority>,
        program_owner: RuntimeProgramOwner,
    ) -> Result<Self, CharacterDialogueValueError> {
        if !look_source_authority
            .program_owner()
            .same_program(&program_owner)
        {
            return Err(CharacterDialogueValueError::ForeignProgramOwner);
        }
        let custom_fields = Arc::new(generation.custom_fields().clone());
        let defaults = Arc::new(CharacterDialogueRuntimeDefaultCatalog::try_new(
            generation
                .characters()
                .values()
                .map(|row| row.defaults().clone()),
        )?);
        let roles = generation.roles().clone();
        let voice_source_type = *generation.voice();
        let program = program_owner.types();
        let rich_text = roles.validate(program)?;
        Self::validate_voice_source_type(voice_source_type, program)?;
        for descriptor in custom_fields.fields.values() {
            program.require_type(descriptor.semantic_type)?;
        }
        let view_contracts = generation.presentation().view_registry_digest();
        let policies = CharacterDialoguePolicyTypeGraph::try_new(
            rich_text,
            PRODUCTION_CHARACTER_DIALOGUE_LIMITS.runtime_schema_limits(),
        )?;
        policies.validate_program_types(program)?;
        let schema = Self {
            generation_digest: generation.digest(),
            view_catalog,
            custom_fields,
            defaults,
            roles,
            voice_source_type,
            look_source_authority,
            program_owner,
            view_contracts,
            policies,
        };
        for character in schema
            .look_source_authority
            .character_catalog()
            .characters()
        {
            if schema.defaults.get(character).is_none() {
                return Err(CharacterDialogueValueError::MissingDefaults(
                    character.clone(),
                ));
            }
        }
        for default in schema.defaults.rows() {
            let digest = schema.config_digest(default.config())?;
            if default
                .expected_digest()
                .is_some_and(|expected| expected != digest)
            {
                return Err(CharacterDialogueValueError::DefaultDigestMismatch(
                    default.character().clone(),
                ));
            }
            let contract = schema.contract_for(default.character(), digest)?;
            let dialogue = CharacterDialogue::try_new(
                default.character().clone(),
                contract,
                default.config().clone(),
            )?;
            schema.validate_dialogue(&dialogue)?;
        }
        Ok(schema)
    }

    fn program_types(&self) -> arcweft_core::program_types::RuntimeProgramTypes<'_> {
        self.program_owner.types()
    }

    /// Static input contract shared by dynamic dialogue-line evidence. The
    /// executable lease is retained separately by `Self::program_owner`.
    #[must_use]
    pub const fn generation_digest(&self) -> RuntimeValueDigest {
        self.generation_digest
    }

    /// Returns the exact executable lease that owns this producer generation.
    #[must_use]
    pub const fn program_owner(&self) -> &RuntimeProgramOwner {
        &self.program_owner
    }

    /// Returns the accepted source Voice identity retained from this program.
    #[must_use]
    pub const fn voice_source_type(&self) -> RuntimeSemanticTypeId {
        self.voice_source_type
    }

    /// Returns the Core authority that owns the exact Character catalog and
    /// proves every present `Look<C>` row against this executable.
    #[must_use]
    pub const fn look_source_authority(&self) -> &Arc<RuntimeCharacterLookSourceAuthority> {
        &self.look_source_authority
    }

    fn validate_voice_source_type(
        semantic_type: RuntimeSemanticTypeId,
        program: RuntimeProgramTypes<'_>,
    ) -> Result<(), CharacterDialogueValueError> {
        let invalid = || CharacterDialogueValueError::VoiceSourceType {
            reason: "expected the accepted one-case DialogueVoice enum containing only `auto`",
        };
        let RuntimeCheckedType::Variant {
            owner:
                RuntimeVariantIdentity::Nominal {
                    nominal,
                    semantic_identity,
                    ..
                },
            arguments,
            cases,
        } = program.checked_type(semantic_type)?
        else {
            return Err(invalid());
        };
        if nominal.as_str() != "DialogueVoice"
            || semantic_identity != semantic_type
            || !arguments.is_empty()
            || cases.len() != 1
            || cases[0].name != "auto"
            || cases[0].payload.is_some()
        {
            return Err(invalid());
        }
        Ok(())
    }

    fn contract_for(
        &self,
        character: &CharacterId,
        defaults: RuntimeValueDigest,
    ) -> Result<CharacterDialogueContractIdentity, CharacterDialogueValueError> {
        Ok(CharacterDialogueContractIdentity::with_visual_manifest(
            self.visual_manifest_contract(character)?,
            defaults,
            self.custom_fields.digest,
            self.view_contracts,
        ))
    }

    fn visual_manifest_contract(
        &self,
        character: &CharacterId,
    ) -> Result<CharacterDialogueVisualManifestEvidence, CharacterDialogueValueError> {
        match self
            .look_source_authority
            .character_catalog()
            .visual_evidence(character)
        {
            None => Err(CharacterDialogueValueError::MissingCharacter(
                character.clone(),
            )),
            Some(CharacterVisualManifestEvidence::Absent) => {
                Ok(CharacterDialogueVisualManifestEvidence::Absent)
            }
            Some(CharacterVisualManifestEvidence::Present(manifest)) => {
                Ok(CharacterDialogueVisualManifestEvidence::Present(
                    RuntimeValueDigest::from_bytes(*manifest.semantic_fingerprint_v1().as_bytes()),
                ))
            }
        }
    }

    fn config_digest(
        &self,
        config: &CharacterDialogueConfig,
    ) -> Result<RuntimeValueDigest, CharacterDialogueValueError> {
        RuntimeValue::Tuple(Self::encode_config_fields(config, &self.policies))
            .try_digest_with_limits(Self::limits())
            .map_err(CharacterDialogueValueError::from)
    }

    /// Computes the canonical digest for the current effective configuration
    /// after admitting it through this exact generation schema.
    pub fn effective_config_digest(
        &self,
        dialogue: &CharacterDialogue,
    ) -> Result<RuntimeValueDigest, CharacterDialogueValueError> {
        self.validate_dialogue(dialogue)?;
        self.config_digest(&dialogue.config)
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
        if !self
            .look_source_authority
            .character_catalog()
            .contains_character(&character)
        {
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

    /// Constructs a fresh value from the accepted Character defaults and the
    /// already evaluated, source-ordered field rows.
    pub fn construct(
        &self,
        owner: &RuntimeProgramOwner,
        target: &RuntimeValue,
        fields: &[CharacterDialoguePatchField<RuntimeValue>],
        result_type: RuntimeSemanticTypeId,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError> {
        self.ensure_program_owner(owner)?;
        let character = Self::character_target(target)?;
        if !self
            .look_source_authority
            .character_catalog()
            .contains_character(&character)
        {
            return Err(CharacterDialogueValueError::MissingCharacter(character));
        }
        let defaults = self
            .defaults
            .get(&character)
            .ok_or_else(|| CharacterDialogueValueError::MissingDefaults(character.clone()))?;
        let contract = self.contract_for(&character, self.config_digest(defaults.config())?)?;
        let base = CharacterDialogue::try_new(character, contract, defaults.config.clone())?;
        let dialogue = self.apply_runtime_fields(base, fields)?;
        self.admit_result(dialogue, result_type)
    }

    /// Reconfigures the exact live CharacterDialogue value using evaluated
    /// source-ordered field rows. Its Character identity and generation
    /// contract remain those of the admitted target.
    pub fn reconfigure(
        &self,
        owner: &RuntimeProgramOwner,
        target: &RuntimeValue,
        fields: &[CharacterDialoguePatchField<RuntimeValue>],
        result_type: RuntimeSemanticTypeId,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError> {
        let target = self.admit(owner, target, result_type)?;
        let dialogue = self.apply_runtime_fields(target.dialogue, fields)?;
        self.admit_result(dialogue, result_type)
    }

    /// Validates a value against this generation and its exact executable
    /// before returning the domain value used by dynamic application/display.
    pub fn admit(
        &self,
        owner: &RuntimeProgramOwner,
        value: &RuntimeValue,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError> {
        self.ensure_program_owner(owner)?;
        self.program_types()
            .accepts_value(semantic_type, value, Self::limits())?;
        let RuntimeValue::Opaque(value) = value else {
            return Err(field_shape(
                "runtime_value",
                "expected an opaque CharacterDialogue value",
            ));
        };
        self.try_decode_opaque(value)
    }

    /// Closed operation entry used by the Core typed producer boundary.
    pub fn apply(
        &self,
        owner: &RuntimeProgramOwner,
        operation: CharacterDialogueOperation,
        target: RuntimeValue,
        fields: &[CharacterDialoguePatchField<RuntimeValue>],
        result_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeValue, CharacterDialogueValueError> {
        self.ensure_program_owner(owner)?;
        let value = match operation {
            CharacterDialogueOperation::Factory => {
                self.construct(owner, &target, fields, result_type)?
            }
            CharacterDialogueOperation::Reconfigure => {
                self.reconfigure(owner, &target, fields, result_type)?
            }
        };
        Ok(value.into_runtime_value())
    }

    fn ensure_program_owner(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<(), CharacterDialogueValueError> {
        if self.program_owner.same_program(owner) {
            Ok(())
        } else {
            Err(CharacterDialogueValueError::ForeignProgramOwner)
        }
    }

    fn admit_result(
        &self,
        dialogue: CharacterDialogue,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError> {
        let value = self.encode(&dialogue)?;
        self.program_types().accepts_value(
            semantic_type,
            &RuntimeValue::Opaque(value.opaque.clone()),
            Self::limits(),
        )?;
        Ok(value)
    }

    fn character_target(value: &RuntimeValue) -> Result<CharacterId, CharacterDialogueValueError> {
        let RuntimeValue::EntityRef(RuntimeEntityReference::Project {
            family: DeclarationIdentityFamily::Character,
            public_id,
        }) = value
        else {
            return Err(field_shape(
                "target",
                "factory target must be a Character entity reference",
            ));
        };
        CharacterId::try_new(public_id.as_str())
            .map_err(|error| field_shape("target", error.to_string()))
    }

    fn apply_runtime_fields(
        &self,
        mut dialogue: CharacterDialogue,
        fields: &[CharacterDialoguePatchField<RuntimeValue>],
    ) -> Result<CharacterDialogue, CharacterDialogueValueError> {
        let maximum = usize::from(PRODUCTION_CHARACTER_DIALOGUE_LIMITS.max_patch_fields);
        if fields.len() > maximum {
            return Err(CharacterDialogueValueError::Limit {
                limit: "patch_fields",
                maximum,
            });
        }
        for field in fields {
            self.apply_runtime_field(&dialogue.character, &mut dialogue.config, field)?;
        }
        CharacterDialogue::try_new(
            dialogue.character.clone(),
            dialogue.contract,
            dialogue.config,
        )
    }

    fn apply_runtime_field(
        &self,
        character: &CharacterId,
        config: &mut CharacterDialogueConfig,
        field: &CharacterDialoguePatchField<RuntimeValue>,
    ) -> Result<(), CharacterDialogueValueError> {
        match &field.operation {
            CharacterDialoguePatchOperation::Set(value) => {
                self.apply_runtime_field_value(character, config, &field.coordinate, value)
            }
            CharacterDialoguePatchOperation::Clear => {
                self.clear_runtime_field(config, &field.coordinate)
            }
        }
    }

    fn apply_runtime_field_value(
        &self,
        character: &CharacterId,
        config: &mut CharacterDialogueConfig,
        coordinate: &CharacterDialogueFieldCoordinate,
        value: &RuntimeValue,
    ) -> Result<(), CharacterDialogueValueError> {
        match coordinate {
            CharacterDialogueFieldCoordinate::Voice => {
                config.voice = Some(self.decode_source_voice(value)?);
            }
            CharacterDialogueFieldCoordinate::Look => {
                config.look = Some(self.decode_source_look(character, value)?);
            }
            CharacterDialogueFieldCoordinate::Stage => {
                config.stage = Some(CharacterDialogueStageValue::try_new(
                    Self::decode_role_value(value)?,
                )?);
            }
            CharacterDialogueFieldCoordinate::Portrait => {
                config.portrait = Some(CharacterDialoguePortraitValue::try_new(
                    Self::decode_role_value(value)?,
                )?);
            }
            CharacterDialogueFieldCoordinate::Focus => {
                config.focus = Some(CharacterDialogueFocusValue::try_new(
                    Self::decode_role_value(value)?,
                )?);
            }
            CharacterDialogueFieldCoordinate::Cleanup => {
                config.cleanup = Some(CharacterDialogueCleanupValue::try_new(
                    Self::decode_role_value(value)?,
                )?);
            }
            CharacterDialogueFieldCoordinate::View => config.view = Self::decode_view(value)?,
            CharacterDialogueFieldCoordinate::SourceLocale => {
                config.source_locale = Some(Self::decode_source_locale(value)?);
            }
            CharacterDialogueFieldCoordinate::Hooks => config.hooks = self.decode_hooks(value)?,
            CharacterDialogueFieldCoordinate::Style => {
                config.style = super::patch::merge_style_set(&config.style, value.clone())?;
            }
            CharacterDialogueFieldCoordinate::RichText => {
                config.rich_text =
                    super::patch::merge_rich_text_set(&config.rich_text, value.clone())?;
            }
            CharacterDialogueFieldCoordinate::InlineFailure => {
                config.inline_failure = self.decode_inline_failure(value)?;
            }
            CharacterDialogueFieldCoordinate::Custom(id) => {
                let typed = CharacterDialogueTypedValue::try_new(value.clone())?;
                config
                    .custom
                    .insert(id.clone(), CharacterDialogueCustomValue::try_new(typed)?);
            }
        }
        Ok(())
    }

    fn clear_runtime_field(
        &self,
        config: &mut CharacterDialogueConfig,
        coordinate: &CharacterDialogueFieldCoordinate,
    ) -> Result<(), CharacterDialogueValueError> {
        match coordinate {
            CharacterDialogueFieldCoordinate::Voice => config.voice = None,
            CharacterDialogueFieldCoordinate::Look => config.look = None,
            CharacterDialogueFieldCoordinate::Stage => config.stage = None,
            CharacterDialogueFieldCoordinate::Portrait => config.portrait = None,
            CharacterDialogueFieldCoordinate::Focus => config.focus = None,
            CharacterDialogueFieldCoordinate::Cleanup => config.cleanup = None,
            CharacterDialogueFieldCoordinate::View => {
                config.view = super::patch::standard_dialogue_view();
            }
            CharacterDialogueFieldCoordinate::SourceLocale => config.source_locale = None,
            CharacterDialogueFieldCoordinate::Hooks => config.hooks.clear(),
            CharacterDialogueFieldCoordinate::Style => {
                let empty = self.canonical_rich_text_no_overrides()?;
                config.style = CharacterDialogueStyleValue::try_new(empty.typed().clone())?;
            }
            CharacterDialogueFieldCoordinate::RichText => {
                config.rich_text = self.canonical_rich_text_no_overrides()?;
            }
            CharacterDialogueFieldCoordinate::InlineFailure => {
                config.inline_failure = InlineFailurePolicy::FailLine;
            }
            CharacterDialogueFieldCoordinate::Custom(id) => {
                let descriptor = self
                    .custom_fields
                    .get(id)
                    .ok_or_else(|| CharacterDialogueValueError::UnknownCustomField(id.clone()))?;
                if !descriptor.clearable {
                    return Err(field_shape(
                        "custom",
                        format!("custom field `{id}` is not clearable"),
                    ));
                }
                config.custom.remove(id);
            }
        }
        Ok(())
    }

    fn canonical_rich_text_no_overrides(
        &self,
    ) -> Result<CharacterDialogueRichTextValue, CharacterDialogueValueError> {
        let binding = self
            .roles
            .authored(Role::RichText)
            .expect("RichText has a fixed authored role slot");
        let CharacterDialogueRuntimeRoleBody::Bound { codec, .. } = binding.body() else {
            return Err(CharacterDialogueValueError::RoleType {
                role: Role::RichText,
                reason: "RichText has no accepted payload codec",
            });
        };
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            Self::opaque_type_producer(),
            binding.value(),
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::ConstantAndSnapshot,
        );
        let value = owner.try_wrap(codec.no_overrides_payload()?)?;
        CharacterDialogueRichTextValue::try_new(CharacterDialogueTypedValue::try_new(value)?)
    }

    fn decode_role_value(
        value: &RuntimeValue,
    ) -> Result<CharacterDialogueTypedValue, CharacterDialogueValueError> {
        CharacterDialogueTypedValue::try_new(value.clone())
    }

    fn decode_view(value: &RuntimeValue) -> Result<ViewId, CharacterDialogueValueError> {
        let RuntimeValue::EntityRef(RuntimeEntityReference::Project { family, public_id }) = value
        else {
            return Err(field_shape("view", "expected View entity reference"));
        };
        if *family != DeclarationIdentityFamily::View {
            return Err(field_shape("view", "expected View entity reference"));
        }
        ViewId::parse_public(public_id.as_str())
            .map_err(|error| field_shape("view", error.to_string()))
    }

    fn decode_source_locale(
        value: &RuntimeValue,
    ) -> Result<DialogueLocaleId, CharacterDialogueValueError> {
        let RuntimeValue::String(value) = value else {
            return Err(field_shape("source_locale", "expected String"));
        };
        DialogueLocaleId::try_new(value.clone())
    }

    fn decode_hooks(
        &self,
        value: &RuntimeValue,
    ) -> Result<Vec<CharacterDialogueHookValue>, CharacterDialogueValueError> {
        let RuntimeValue::Seq(values) = value else {
            return Err(field_shape("hooks", "expected Seq"));
        };
        values
            .clone()
            .into_values()
            .into_iter()
            .map(|value| {
                CharacterDialogueHookValue::try_new(CharacterDialogueTypedValue::try_new(value)?)
            })
            .collect()
    }

    fn decode_source_voice(
        &self,
        value: &RuntimeValue,
    ) -> Result<CharacterDialogueVoice, CharacterDialogueValueError> {
        let semantic_type = self.voice_source_type;
        self.program_types()
            .accepts_value(semantic_type, value, Self::limits())?;
        let RuntimeCheckedType::Variant { owner, cases, .. } =
            self.program_types().checked_type(semantic_type)?
        else {
            return Err(CharacterDialogueValueError::VoiceSourceType {
                reason: "accepted Voice type is not a nominal enum",
            });
        };
        let RuntimeValue::Variant {
            owner: value_owner,
            ordinal,
            name,
            payload,
        } = value
        else {
            return Err(field_shape(
                "voice",
                "expected accepted DialogueVoice value",
            ));
        };
        if value_owner != &owner || *ordinal != 0 || name != &cases[0].name || payload.is_some() {
            return Err(field_shape(
                "voice",
                "value does not match the accepted DialogueVoice case row",
            ));
        }
        Ok(CharacterDialogueVoice::Auto)
    }

    fn decode_source_look(
        &self,
        character: &CharacterId,
        value: &RuntimeValue,
    ) -> Result<CharacterLookId, CharacterDialogueValueError> {
        self.look_source_authority
            .decode(&self.program_owner, character, value)
            .map_err(|error| match error {
                RuntimeCharacterLookSourceError::ForeignProgram => {
                    CharacterDialogueValueError::ForeignProgramOwner
                }
                RuntimeCharacterLookSourceError::MissingVisualManifest(character) => {
                    CharacterDialogueValueError::MissingVisualManifest(character)
                }
                RuntimeCharacterLookSourceError::InvalidLookType(character) => {
                    CharacterDialogueValueError::LookSourceType {
                        character,
                        reason: "accepted Core Look authority rejected the executable row",
                    }
                }
                RuntimeCharacterLookSourceError::InvalidLookValue(character) => {
                    field_shape("look", format!("invalid Look value for `{character}`"))
                }
                RuntimeCharacterLookSourceError::NominalGraph(error) => {
                    CharacterDialogueValueError::NominalSchema(error)
                }
                RuntimeCharacterLookSourceError::ProgramType(error) => {
                    CharacterDialogueValueError::ProgramType(error)
                }
            })
    }

    fn validate_dialogue(
        &self,
        dialogue: &CharacterDialogue,
    ) -> Result<(), CharacterDialogueValueError> {
        dialogue.config.validate()?;
        let visual_manifest = self.visual_manifest_contract(&dialogue.character)?;
        if visual_manifest != dialogue.contract.visual_manifest() {
            return Err(CharacterDialogueValueError::VisualManifestEvidenceMismatch(
                dialogue.character.clone(),
            ));
        }
        let defaults = self.defaults.get(&dialogue.character).ok_or_else(|| {
            CharacterDialogueValueError::MissingDefaults(dialogue.character.clone())
        })?;
        if self.config_digest(defaults.config())? != dialogue.contract.defaults() {
            return Err(CharacterDialogueValueError::DefaultsMismatch(
                dialogue.character.clone(),
            ));
        }
        if dialogue.contract.view_contracts() != self.view_contracts {
            return Err(CharacterDialogueValueError::ViewContractsMismatch);
        }
        if let Some(look) = &dialogue.config.look {
            let manifest = self
                .look_source_authority
                .character_catalog()
                .visual_manifest(&dialogue.character)
                .ok_or_else(|| {
                    CharacterDialogueValueError::MissingVisualManifest(dialogue.character.clone())
                })?;
            if manifest.look(look).is_none() {
                return Err(CharacterDialogueValueError::MissingLook {
                    character: dialogue.character.clone(),
                    look: look.clone(),
                });
            }
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
            self.program_types().accepts_value(
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

impl CharacterDialogueRuntimeSchema {
    fn encode_payload(&self, dialogue: &CharacterDialogue) -> RuntimeValue {
        let contract = dialogue.contract;
        let config = &dialogue.config;
        let mut fields = vec![
            RuntimeValue::EntityRef(RuntimeEntityReference::Project {
                family: DeclarationIdentityFamily::Character,
                public_id: dialogue.character.as_public_id(),
            }),
            Self::encode_visual_manifest(contract.visual_manifest()),
            Self::digest_value(contract.defaults()),
            Self::digest_value(contract.custom_schema()),
            Self::digest_value(contract.view_contracts()),
        ];
        fields.extend(Self::encode_config_fields(config, &self.policies));
        RuntimeValue::Tuple(fields)
    }

    fn encode_config_fields(
        config: &CharacterDialogueConfig,
        policies: &CharacterDialoguePolicyTypeGraph,
    ) -> Vec<RuntimeValue> {
        vec![
            Self::encode_option(
                config
                    .voice
                    .as_ref()
                    .map(|voice| Self::encode_voice(voice, policies)),
            ),
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
            Self::encode_inline_failure(&config.inline_failure, policies),
            Self::encode_custom(&config.custom),
        ]
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
        let contract = CharacterDialogueContractIdentity::with_visual_manifest(
            Self::decode_visual_manifest(&fields[1])?,
            Self::decode_digest(&fields[2], "defaults_digest")?,
            Self::decode_digest(&fields[3], "custom_schema_digest")?,
            Self::decode_digest(&fields[4], "view_contracts_digest")?,
        );
        let voice = Self::decode_option(&fields[5], "voice")?
            .map(|voice| self.decode_stored_voice(voice))
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

    fn encode_voice(
        voice: &CharacterDialogueVoice,
        policies: &CharacterDialoguePolicyTypeGraph,
    ) -> RuntimeValue {
        match voice {
            CharacterDialogueVoice::Auto => {
                Self::dialogue_variant(policies, CharacterDialoguePolicyCase::VoiceAuto, None)
            }
            CharacterDialogueVoice::Id(id) => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::VoiceId,
                Some(RuntimeValue::String(id.as_str().to_owned())),
            ),
        }
    }

    fn decode_stored_voice(
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
        self.expect_dialogue_variant_owner(
            owner,
            CharacterDialoguePolicyVariantOwner::Voice,
            "voice",
        )?;
        match (
            CharacterDialoguePolicyTypeGraph::case_for_value(
                CharacterDialoguePolicyVariantOwner::Voice,
                *ordinal,
                name,
            ),
            payload.as_deref(),
        ) {
            (Some(CharacterDialoguePolicyCase::VoiceAuto), None) => {
                Ok(CharacterDialogueVoice::Auto)
            }
            (Some(CharacterDialoguePolicyCase::VoiceId), Some(RuntimeValue::String(id))) => {
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
    fn encode_inline_failure(
        policy: &InlineFailurePolicy,
        policies: &CharacterDialoguePolicyTypeGraph,
    ) -> RuntimeValue {
        match policy {
            InlineFailurePolicy::FailLine => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::InlineFailureFailLine,
                None,
            ),
            InlineFailurePolicy::Discard => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::InlineFailureDiscard,
                None,
            ),
            InlineFailurePolicy::Fallback { fallback } => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::InlineFailureFallback,
                Some(Self::encode_fallback(fallback, policies)),
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
            CharacterDialoguePolicyVariantOwner::InlineFailure,
            "inline_failure",
        )?;
        match (
            CharacterDialoguePolicyTypeGraph::case_for_value(
                CharacterDialoguePolicyVariantOwner::InlineFailure,
                *ordinal,
                name,
            ),
            payload.as_deref(),
        ) {
            (Some(CharacterDialoguePolicyCase::InlineFailureFailLine), None) => {
                Ok(InlineFailurePolicy::FailLine)
            }
            (Some(CharacterDialoguePolicyCase::InlineFailureDiscard), None) => {
                Ok(InlineFailurePolicy::Discard)
            }
            (Some(CharacterDialoguePolicyCase::InlineFailureFallback), Some(value)) => {
                Ok(InlineFailurePolicy::Fallback {
                    fallback: self.decode_fallback(value)?,
                })
            }
            _ => Err(field_shape("inline_failure", "invalid policy variant")),
        }
    }

    fn encode_fallback(
        fallback: &InlineFallback,
        policies: &CharacterDialoguePolicyTypeGraph,
    ) -> RuntimeValue {
        match fallback {
            InlineFallback::Text { text, style } => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::InlineFallbackText,
                Some(RuntimeValue::Tuple(vec![
                    RuntimeValue::String(text.clone()),
                    Self::encode_fallback_style(style, policies),
                ])),
            ),
            InlineFallback::ExprSource { style } => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::InlineFallbackExprSource,
                Some(Self::encode_fallback_style(style, policies)),
            ),
            InlineFallback::CallSource { style } => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::InlineFallbackCallSource,
                Some(Self::encode_fallback_style(style, policies)),
            ),
            InlineFallback::ValuePlain => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::InlineFallbackValuePlain,
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
            CharacterDialoguePolicyVariantOwner::InlineFallback,
            "inline_failure",
        )?;
        match (
            CharacterDialoguePolicyTypeGraph::case_for_value(
                CharacterDialoguePolicyVariantOwner::InlineFallback,
                *ordinal,
                name,
            ),
            payload.as_deref(),
        ) {
            (
                Some(CharacterDialoguePolicyCase::InlineFallbackText),
                Some(RuntimeValue::Tuple(values)),
            ) if values.len() == 2 => {
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
            (Some(CharacterDialoguePolicyCase::InlineFallbackExprSource), Some(style)) => {
                Ok(InlineFallback::ExprSource {
                    style: self.decode_fallback_style(style)?,
                })
            }
            (Some(CharacterDialoguePolicyCase::InlineFallbackCallSource), Some(style)) => {
                Ok(InlineFallback::CallSource {
                    style: self.decode_fallback_style(style)?,
                })
            }
            (Some(CharacterDialoguePolicyCase::InlineFallbackValuePlain), None) => {
                Ok(InlineFallback::ValuePlain)
            }
            _ => Err(field_shape("inline_failure", "invalid fallback variant")),
        }
    }

    fn encode_fallback_style(
        style: &FallbackStylePolicy,
        policies: &CharacterDialoguePolicyTypeGraph,
    ) -> RuntimeValue {
        match style {
            FallbackStylePolicy::Plain => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::FallbackStylePlain,
                None,
            ),
            FallbackStylePolicy::InheritSurrounding => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::FallbackStyleInheritSurrounding,
                None,
            ),
            FallbackStylePolicy::Apply { styles } => Self::dialogue_variant(
                policies,
                CharacterDialoguePolicyCase::FallbackStyleApply,
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
            CharacterDialoguePolicyVariantOwner::FallbackStyle,
            "inline_failure",
        )?;
        match (
            CharacterDialoguePolicyTypeGraph::case_for_value(
                CharacterDialoguePolicyVariantOwner::FallbackStyle,
                *ordinal,
                name,
            ),
            payload.as_deref(),
        ) {
            (Some(CharacterDialoguePolicyCase::FallbackStylePlain), None) => {
                Ok(FallbackStylePolicy::Plain)
            }
            (Some(CharacterDialoguePolicyCase::FallbackStyleInheritSurrounding), None) => {
                Ok(FallbackStylePolicy::InheritSurrounding)
            }
            (
                Some(CharacterDialoguePolicyCase::FallbackStyleApply),
                Some(RuntimeValue::Seq(styles)),
            ) => Ok(FallbackStylePolicy::Apply {
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
        policies: &CharacterDialoguePolicyTypeGraph,
        case: CharacterDialoguePolicyCase,
        payload: Option<RuntimeValue>,
    ) -> RuntimeValue {
        let (owner, ordinal, name) = CharacterDialoguePolicyTypeGraph::case_spec(case);
        RuntimeValue::Variant {
            owner: policies.identity(owner).clone(),
            ordinal,
            name: name.to_owned(),
            payload: payload.map(Box::new),
        }
    }

    fn expect_dialogue_variant_owner(
        &self,
        actual: &RuntimeVariantIdentity,
        expected: CharacterDialoguePolicyVariantOwner,
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

    fn encode_visual_manifest(evidence: CharacterDialogueVisualManifestEvidence) -> RuntimeValue {
        match evidence {
            CharacterDialogueVisualManifestEvidence::Absent => Self::encode_option(None),
            CharacterDialogueVisualManifestEvidence::Present(digest) => {
                Self::encode_option(Some(Self::digest_value(digest)))
            }
        }
    }

    fn decode_visual_manifest(
        value: &RuntimeValue,
    ) -> Result<CharacterDialogueVisualManifestEvidence, CharacterDialogueValueError> {
        match Self::decode_option(value, "visual_manifest")? {
            None => Ok(CharacterDialogueVisualManifestEvidence::Absent),
            Some(value) => Ok(CharacterDialogueVisualManifestEvidence::Present(
                Self::decode_digest(value, "visual_manifest_digest")?,
            )),
        }
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
