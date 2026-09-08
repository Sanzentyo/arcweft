//! Producer-validated opaque runtime values.

use crate::effect::RuntimeArtifactFingerprint;
use crate::entry::{RuntimeDialogueContentTemplateDigest, RuntimeSchemaLimits};
use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId};
use crate::plan::{RuntimeDialogueValueBinding, RuntimeDialogueValueRole};
use crate::runtime_id::{RuntimeDialogueContentTemplateId, RuntimeDialogueValueSlotId};
use crate::value::{
    DenseSeqKind, MAX_RUNTIME_VALUE_NESTING_DEPTH, RuntimeFunctionValue, RuntimeSeq, RuntimeUInt,
    RuntimeValue,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Closed producer authority for runtime-supplied dialogue values.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeDialogueOpaqueRole {
    View,
    Character,
    Content,
    Occurrence,
    Stage,
    Reveal,
    Action,
}

/// Closed field coordinates of the standard `DialogueView` runtime payload.
///
/// This owner is shared by standard pure-program lowering and runtime value
/// construction, so neither side can independently invent tuple ordinals.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeDialogueViewField {
    Character,
    Content,
    Occurrence,
    Stage,
    Reveal,
    PrimaryAction,
}

/// Typed, owner-validated payload of one standard `DialogueView` value.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueViewValue {
    character: RuntimeValue,
    content: RuntimeValue,
    occurrence: RuntimeValue,
    stage: RuntimeValue,
    reveal: RuntimeValue,
    primary_action: RuntimeValue,
}

/// Closed data carried by a `DialogueAction` token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeDialogueActionValue {
    None,
    Advance(RuntimeDialogueAdvanceAction),
}

/// Runtime-neutral coordinates of one stale-safe dialogue advance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueAdvanceAction {
    pub dialogue: u64,
    pub entry: u64,
    pub instance: u64,
    pub stage: u32,
    pub revision: u64,
}

/// Failure to admit a producer-owned dialogue carrier.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeDialogueValueError {
    #[error("runtime value is not owned by exact dialogue role {expected:?}")]
    InvalidOwner { expected: RuntimeDialogueOpaqueRole },
    #[error("DialogueView field {field:?} is not owned by exact dialogue role {expected:?}")]
    InvalidViewFieldOwner {
        field: RuntimeDialogueViewField,
        expected: RuntimeDialogueOpaqueRole,
    },
    #[error("DialogueView payload does not contain its six canonical fields")]
    InvalidViewPayload,
    #[error("DialogueAction payload violates its closed token schema")]
    InvalidActionPayload,
}

/// Fixed payload contract for one runtime dialogue content value.
///
/// The value is carried by the exact `std.dialogue.content` opaque owner. Its
/// payload is deliberately a non-empty tuple with a fixed field order:
/// contract version, artifact fingerprint, template identity, template
/// digest, canonical role-tagged value bindings, and canonical site-keyed
/// effect callbacks. The tuple is an internal runtime envelope; text-model
/// consumers receive the typed value rather than decoding this representation
/// themselves.
pub const RUNTIME_DIALOGUE_CONTENT_VALUE_VERSION: u8 = 1;

/// Failure to decode or construct a typed runtime dialogue content value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeDialogueContentValueError {
    #[error("runtime dialogue content value is not owned by the exact Content opaque owner")]
    InvalidOwner,
    #[error("runtime dialogue content payload must be the canonical non-empty envelope tuple")]
    InvalidPayload,
    #[error("runtime dialogue content payload version is {actual}, expected {expected}")]
    UnsupportedVersion { actual: u8, expected: u8 },
    #[error("runtime dialogue content payload has an invalid artifact fingerprint")]
    InvalidArtifact,
    #[error("runtime dialogue content payload has an invalid template identity")]
    InvalidTemplate,
    #[error("runtime dialogue content payload has an invalid template digest")]
    InvalidTemplateDigest,
    #[error("runtime dialogue content template manifest is invalid: {message}")]
    InvalidTemplateManifest { message: String },
    #[error("runtime dialogue content payload has too many bindings: {actual} > {maximum}")]
    BindingLimit { actual: usize, maximum: usize },
    #[error(
        "runtime dialogue content evaluated binding count {actual} does not match template count {expected}"
    )]
    BindingCountMismatch { expected: usize, actual: usize },
    #[error(
        "runtime dialogue content effect binding count {actual} does not match template count {expected}"
    )]
    EffectCountMismatch { expected: usize, actual: usize },
    #[error("runtime dialogue content binding {index} is not the canonical slot {expected}")]
    NonCanonicalSlot {
        index: usize,
        expected: RuntimeDialogueValueSlotId,
        actual: RuntimeDialogueValueSlotId,
    },
    #[error("runtime dialogue content effect binding {index} is not canonical site {expected}")]
    NonCanonicalEffectSite {
        index: usize,
        expected: crate::runtime_id::RuntimeDialogueEffectSiteId,
        actual: crate::runtime_id::RuntimeDialogueEffectSiteId,
    },
    #[error("runtime dialogue content binding {index} has an invalid shape")]
    InvalidBindingShape { index: usize },
    #[error("runtime dialogue content effect binding {index} has an invalid shape")]
    InvalidEffectShape { index: usize },
    #[error("runtime dialogue content binding {index} has an unknown role tag {tag}")]
    UnknownBindingRole { index: usize, tag: u8 },
    #[error("runtime dialogue content binding {index} has a value incompatible with role {role:?}")]
    InvalidBindingValue {
        index: usize,
        role: RuntimeDialogueValueRole,
    },
    #[error(
        "runtime dialogue content effect binding {index} is not a zero-argument callback: {message}"
    )]
    InvalidEffectCallback { index: usize, message: String },
    #[error(
        "runtime dialogue content binding {index} has an invalid inline text capture: {source}"
    )]
    InvalidInlineTextValue {
        index: usize,
        #[source]
        source: RuntimeInlineTextValueError,
    },
    #[error("nested runtime dialogue content value at binding {index} belongs to another artifact")]
    NestedArtifactMismatch { index: usize },
    #[error("runtime dialogue content value exceeds the shared nesting limit of {maximum}")]
    NestingLimit { maximum: usize },
    #[error("runtime dialogue content value exceeds the shared node limit of {maximum}")]
    NodeLimit { maximum: usize },
    #[error("runtime dialogue content value exceeds the shared string limit of {maximum} bytes")]
    StringLimit { maximum: usize },
}

/// Closed, deterministic textual capture used by a dialogue interpolation.
///
/// The semantic type identity is retained alongside the formatted text. The
/// runtime envelope therefore cannot silently reinterpret an arbitrary
/// `RuntimeValue` as display text; callers must first pass through the closed
/// formatting operation owned by this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeInlineTextValue {
    semantic_type: RuntimeSemanticTypeId,
    text: String,
}

/// Failure to construct or decode one closed inline-text capture.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeInlineTextValueError {
    #[error("inline text capture is not the canonical semantic-type/text tuple")]
    InvalidPayload,
    #[error("inline text capture cannot format this runtime value")]
    UnsupportedRuntimeValue,
    #[error("inline text capture cannot format a non-finite {kind}")]
    NonFinite { kind: &'static str },
    #[error("inline text capture exceeds the shared string limit of {maximum} bytes")]
    StringLimit { maximum: usize },
}

impl RuntimeInlineTextValue {
    /// Constructs a typed capture from already deterministic text.
    pub fn try_new(
        semantic_type: RuntimeSemanticTypeId,
        text: impl Into<String>,
    ) -> Result<Self, RuntimeInlineTextValueError> {
        Self::try_new_with_limits(semantic_type, text, RuntimeSchemaLimits::engine_default())
    }

    /// Constructs a typed capture under explicit shared runtime limits.
    pub fn try_new_with_limits(
        semantic_type: RuntimeSemanticTypeId,
        text: impl Into<String>,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeInlineTextValueError> {
        let text = text.into();
        if !limits.permits_string_bytes(text.len()) {
            return Err(RuntimeInlineTextValueError::StringLimit {
                maximum: usize::try_from(limits.max_string_bytes).unwrap_or(usize::MAX),
            });
        }
        Ok(Self {
            semantic_type,
            text,
        })
    }

    /// Formats one closed set of deterministic runtime scalar values.
    pub fn try_format_runtime_value(
        semantic_type: RuntimeSemanticTypeId,
        value: &RuntimeValue,
    ) -> Result<Self, RuntimeInlineTextValueError> {
        let text = match value {
            RuntimeValue::Unit => "()".to_owned(),
            RuntimeValue::Bool(value) => value.to_string(),
            RuntimeValue::Int(value) => value.to_string(),
            RuntimeValue::UInt(value) => value.to_string(),
            RuntimeValue::F32(value) if value.is_finite() => value.to_string(),
            RuntimeValue::F64(value) if value.is_finite() => value.to_string(),
            RuntimeValue::F32(_) => {
                return Err(RuntimeInlineTextValueError::NonFinite { kind: "f32" });
            }
            RuntimeValue::F64(_) => {
                return Err(RuntimeInlineTextValueError::NonFinite { kind: "f64" });
            }
            RuntimeValue::String(value) => value.clone(),
            RuntimeValue::Char(value) => value.to_string(),
            RuntimeValue::Duration(value) => format!("{}ns", value.as_nanos()),
            RuntimeValue::EntityRef(value) => format!("@{}", value.runtime_label()),
            RuntimeValue::Progress(value) => value
                .label()
                .map_or_else(|| value.ratio().to_string(), ToOwned::to_owned),
            _ => return Err(RuntimeInlineTextValueError::UnsupportedRuntimeValue),
        };
        Self::try_new(semantic_type, text)
    }

    /// Alias emphasizing that formatting is the only raw-value admission.
    pub fn try_from_runtime_value(
        semantic_type: RuntimeSemanticTypeId,
        value: &RuntimeValue,
    ) -> Result<Self, RuntimeInlineTextValueError> {
        Self::try_format_runtime_value(semantic_type, value)
    }

    /// Decodes the canonical semantic-type/text tuple carried in a content
    /// binding. A direct runtime String is intentionally not accepted.
    pub fn try_decode_runtime_value(
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeInlineTextValueError> {
        let RuntimeValue::Tuple(fields) = value else {
            return Err(RuntimeInlineTextValueError::InvalidPayload);
        };
        let [semantic_type, text] = fields.as_slice() else {
            return Err(RuntimeInlineTextValueError::InvalidPayload);
        };
        let RuntimeValue::Seq(sequence) = semantic_type else {
            return Err(RuntimeInlineTextValueError::InvalidPayload);
        };
        if sequence.dense_kind() != Some(DenseSeqKind::Bytes) {
            return Err(RuntimeInlineTextValueError::InvalidPayload);
        }
        let Some(bytes) = sequence.as_bytes() else {
            return Err(RuntimeInlineTextValueError::InvalidPayload);
        };
        let Ok(bytes) = <[u8; 32]>::try_from(bytes) else {
            return Err(RuntimeInlineTextValueError::InvalidPayload);
        };
        let RuntimeValue::String(text) = text else {
            return Err(RuntimeInlineTextValueError::InvalidPayload);
        };
        Self::try_new_with_limits(
            RuntimeSemanticTypeId::from_bytes(bytes),
            text.clone(),
            limits,
        )
    }

    #[must_use]
    pub const fn semantic_type(&self) -> RuntimeSemanticTypeId {
        self.semantic_type
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn into_runtime_value(self) -> RuntimeValue {
        RuntimeValue::Tuple(vec![
            RuntimeValue::Seq(RuntimeSeq::dense_bytes(
                self.semantic_type.as_bytes().to_vec(),
            )),
            RuntimeValue::String(self.text),
        ])
    }
}

/// Closed typed binding admitted into a Content envelope.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeDialogueContentBinding {
    Interpolation {
        slot: RuntimeDialogueValueSlotId,
        semantic_type: RuntimeSemanticTypeId,
        value: RuntimeInlineTextValue,
    },
    Content {
        slot: RuntimeDialogueValueSlotId,
        semantic_type: RuntimeSemanticTypeId,
        value: RuntimeDialogueContentValue,
    },
}

/// Runtime callback captured by one content-local effect site.
///
/// The callback is deliberately the existing runtime function authority.  It
/// carries no effect-expression bytecode or copied capture side table; the
/// function value owns its structured/AWBC closure representation.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueContentEffectBinding {
    site: crate::runtime_id::RuntimeDialogueEffectSiteId,
    callback: RuntimeFunctionValue,
}

impl RuntimeDialogueContentEffectBinding {
    #[must_use]
    pub const fn new(
        site: crate::runtime_id::RuntimeDialogueEffectSiteId,
        callback: RuntimeFunctionValue,
    ) -> Self {
        Self { site, callback }
    }

    #[must_use]
    pub const fn site(&self) -> crate::runtime_id::RuntimeDialogueEffectSiteId {
        self.site
    }

    #[must_use]
    pub const fn callback(&self) -> &RuntimeFunctionValue {
        &self.callback
    }
}

impl RuntimeDialogueContentBinding {
    #[must_use]
    pub const fn slot(&self) -> RuntimeDialogueValueSlotId {
        match self {
            Self::Interpolation { slot, .. } | Self::Content { slot, .. } => *slot,
        }
    }

    #[must_use]
    pub const fn role(&self) -> RuntimeDialogueValueRole {
        match self {
            Self::Interpolation { .. } => RuntimeDialogueValueRole::Interpolation,
            Self::Content { .. } => RuntimeDialogueValueRole::Content,
        }
    }

    #[must_use]
    pub const fn semantic_type(&self) -> RuntimeSemanticTypeId {
        match self {
            Self::Interpolation { semantic_type, .. } | Self::Content { semantic_type, .. } => {
                *semantic_type
            }
        }
    }

    #[must_use]
    pub const fn inline_text(&self) -> Option<&RuntimeInlineTextValue> {
        match self {
            Self::Interpolation { value, .. } => Some(value),
            Self::Content { .. } => None,
        }
    }

    #[must_use]
    pub const fn content(&self) -> Option<&RuntimeDialogueContentValue> {
        match self {
            Self::Content { value, .. } => Some(value),
            Self::Interpolation { .. } => None,
        }
    }

    #[must_use]
    pub fn with_slot(self, slot: RuntimeDialogueValueSlotId) -> Self {
        match self {
            Self::Interpolation {
                semantic_type,
                value,
                ..
            } => Self::Interpolation {
                slot,
                semantic_type,
                value,
            },
            Self::Content {
                semantic_type,
                value,
                ..
            } => Self::Content {
                slot,
                semantic_type,
                value,
            },
        }
    }

    fn into_runtime_value(self) -> RuntimeValue {
        match self {
            Self::Interpolation { value, .. } => value.into_runtime_value(),
            Self::Content { value, .. } => value.into_runtime_value(),
        }
    }
}

/// Typed runtime envelope for one dialogue content fragment.
///
/// `RuntimeDialogueContentValue` is the sole authority for the payload behind
/// the exact `std.dialogue.content` opaque owner.  The envelope deliberately
/// retains the artifact identity, plan-local template identity, and the
/// canonical role-tagged value bindings needed by text-model materialization.
/// Consumers must use this type instead of decoding the opaque payload shape.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueContentValue {
    artifact: RuntimeArtifactFingerprint,
    template: RuntimeDialogueContentTemplateId,
    template_digest: RuntimeDialogueContentTemplateDigest,
    bindings: Box<[RuntimeDialogueContentBinding]>,
    effects: Box<[RuntimeDialogueContentEffectBinding]>,
}

impl Serialize for RuntimeDialogueContentValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.clone().into_runtime_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimeDialogueContentValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        RuntimeValue::deserialize(deserializer).and_then(|value| {
            Self::try_from_runtime_value(&value).map_err(serde::de::Error::custom)
        })
    }
}

impl RuntimeDialogueContentValue {
    /// Constructs a content envelope using the named engine schema policy.
    pub fn try_new(
        artifact: RuntimeArtifactFingerprint,
        template: RuntimeDialogueContentTemplateId,
        template_digest: RuntimeDialogueContentTemplateDigest,
        bindings: impl IntoIterator<Item = RuntimeDialogueContentBinding>,
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        Self::try_new_with_effects(artifact, template, template_digest, bindings, [])
    }

    /// Constructs a content envelope with its complete callback binding list.
    pub fn try_new_with_effects(
        artifact: RuntimeArtifactFingerprint,
        template: RuntimeDialogueContentTemplateId,
        template_digest: RuntimeDialogueContentTemplateDigest,
        bindings: impl IntoIterator<Item = RuntimeDialogueContentBinding>,
        effects: impl IntoIterator<Item = RuntimeDialogueContentEffectBinding>,
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        Self::try_new_with_limits(
            artifact,
            template,
            template_digest,
            bindings,
            effects,
            RuntimeSchemaLimits::engine_default(),
        )
    }

    /// Constructs a content envelope after applying the caller-selected
    /// shared runtime schema limits.
    pub fn try_new_with_limits(
        artifact: RuntimeArtifactFingerprint,
        template: RuntimeDialogueContentTemplateId,
        template_digest: RuntimeDialogueContentTemplateDigest,
        bindings: impl IntoIterator<Item = RuntimeDialogueContentBinding>,
        effects: impl IntoIterator<Item = RuntimeDialogueContentEffectBinding>,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        let value = Self {
            artifact,
            template,
            template_digest,
            bindings: bindings.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            effects: effects.into_iter().collect::<Vec<_>>().into_boxed_slice(),
        };
        value.validate_structure(limits)?;
        value.validate_runtime_graph(limits)?;
        Ok(value)
    }

    /// Packages evaluated dialogue bindings through the single Content
    /// envelope admission algorithm. The immutable plan manifest supplies
    /// each slot's exact semantic role/type; callers cannot reinterpret a raw
    /// runtime value or omit a slot.
    pub fn try_from_evaluated_bindings(
        artifact: RuntimeArtifactFingerprint,
        template: &crate::plan::RuntimeDialogueContentTemplateManifest,
        evaluated: &[RuntimeDialogueValueBinding],
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        Self::try_from_evaluated_bindings_with_effects(artifact, template, evaluated, &[])
    }

    /// Packages evaluated value slots and their site-keyed effect callbacks
    /// against one immutable template manifest.
    pub fn try_from_evaluated_bindings_with_effects(
        artifact: RuntimeArtifactFingerprint,
        template: &crate::plan::RuntimeDialogueContentTemplateManifest,
        evaluated: &[RuntimeDialogueValueBinding],
        effects: &[RuntimeDialogueContentEffectBinding],
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        Self::try_from_evaluated_bindings_with_effects_with_limits(
            artifact,
            template,
            evaluated,
            effects,
            RuntimeSchemaLimits::engine_default(),
        )
    }

    /// Limits-aware form of [`Self::try_from_evaluated_bindings`].
    pub fn try_from_evaluated_bindings_with_limits(
        artifact: RuntimeArtifactFingerprint,
        template: &crate::plan::RuntimeDialogueContentTemplateManifest,
        evaluated: &[RuntimeDialogueValueBinding],
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        Self::try_from_evaluated_bindings_with_effects_with_limits(
            artifact,
            template,
            evaluated,
            &[],
            limits,
        )
    }

    /// Limits-aware form of [`Self::try_from_evaluated_bindings_with_effects`].
    pub fn try_from_evaluated_bindings_with_effects_with_limits(
        artifact: RuntimeArtifactFingerprint,
        template: &crate::plan::RuntimeDialogueContentTemplateManifest,
        evaluated: &[RuntimeDialogueValueBinding],
        effects: &[RuntimeDialogueContentEffectBinding],
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        if evaluated.len() != template.slots().len() {
            return Err(RuntimeDialogueContentValueError::BindingCountMismatch {
                expected: template.slots().len(),
                actual: evaluated.len(),
            });
        }
        if !limits.permits_sequence_items(evaluated.len()) {
            return Err(RuntimeDialogueContentValueError::BindingLimit {
                actual: evaluated.len(),
                maximum: usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX),
            });
        }
        let mut bindings = Vec::with_capacity(evaluated.len());
        for (index, (slot, evaluated)) in template.slots().iter().zip(evaluated).enumerate() {
            let expected_slot = RuntimeDialogueValueSlotId::from_zero_based(index).ok_or(
                RuntimeDialogueContentValueError::NodeLimit {
                    maximum: usize::try_from(limits.max_nodes).unwrap_or(usize::MAX),
                },
            )?;
            if slot.slot() != expected_slot || evaluated.slot != expected_slot {
                return Err(RuntimeDialogueContentValueError::NonCanonicalSlot {
                    index,
                    expected: expected_slot,
                    actual: evaluated.slot,
                });
            }
            if evaluated.role != slot.role() {
                return Err(RuntimeDialogueContentValueError::InvalidBindingValue {
                    index,
                    role: evaluated.role,
                });
            }
            let binding = match slot.role() {
                RuntimeDialogueValueRole::Interpolation => {
                    let value = RuntimeInlineTextValue::try_from_runtime_value(
                        slot.semantic_type(),
                        &evaluated.value,
                    )
                    .map_err(|source| {
                        RuntimeDialogueContentValueError::InvalidInlineTextValue { index, source }
                    })?;
                    RuntimeDialogueContentBinding::Interpolation {
                        slot: expected_slot,
                        semantic_type: slot.semantic_type(),
                        value,
                    }
                }
                RuntimeDialogueValueRole::Content => {
                    let value = Self::try_from_runtime_value_with_limits(&evaluated.value, limits)?;
                    RuntimeDialogueContentBinding::Content {
                        slot: expected_slot,
                        semantic_type: slot.semantic_type(),
                        value,
                    }
                }
            };
            bindings.push(binding);
        }
        if effects.len() != template.effects().len() {
            return Err(RuntimeDialogueContentValueError::EffectCountMismatch {
                expected: template.effects().len(),
                actual: effects.len(),
            });
        }
        for (index, (declared, binding)) in template.effects().iter().zip(effects).enumerate() {
            let expected = declared.site();
            if binding.site() != expected {
                return Err(RuntimeDialogueContentValueError::NonCanonicalEffectSite {
                    index,
                    expected,
                    actual: binding.site(),
                });
            }
            let callback = binding.callback();
            let remaining = callback.remaining_arity().map_err(|error| {
                RuntimeDialogueContentValueError::InvalidEffectCallback {
                    index,
                    message: error.to_string(),
                }
            })?;
            if callback.as_structured().is_some() && !callback.is_structured_executable_callback() {
                return Err(RuntimeDialogueContentValueError::InvalidEffectCallback {
                    index,
                    message: "structured callback site is not an executable Unit body".to_owned(),
                });
            }
            if remaining != 0 || callback.capture_count() != declared.capture_types().len() {
                return Err(RuntimeDialogueContentValueError::InvalidEffectCallback {
                    index,
                    message: format!(
                        "callback has {remaining} remaining parameters and {} captures, expected zero parameters and {} captures",
                        callback.capture_count(),
                        declared.capture_types().len()
                    ),
                });
            }
        }
        Self::try_new_with_limits(
            artifact,
            template.id(),
            template.digest(),
            bindings,
            effects.to_owned(),
            limits,
        )
    }

    /// Packages bindings against an already verified manifest projection. This
    /// is the bridge used by AWBC and other executors whose immutable template
    /// catalog is not the in-memory `RuntimePlan` table.
    pub fn try_from_evaluated_bindings_parts(
        artifact: RuntimeArtifactFingerprint,
        template: RuntimeDialogueContentTemplateId,
        template_digest: RuntimeDialogueContentTemplateDigest,
        slots: &[crate::plan::RuntimeDialogueContentSlot],
        evaluated: &[RuntimeDialogueValueBinding],
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        Self::try_from_evaluated_bindings_parts_with_effects(
            artifact,
            template,
            template_digest,
            slots,
            evaluated,
            &[],
            &[],
        )
    }

    /// AWBC/adapter bridge that admits a complete slot and callback manifest
    /// without requiring an in-memory [`RuntimePlan`] template table.
    pub(crate) fn try_from_evaluated_bindings_parts_with_effects(
        artifact: RuntimeArtifactFingerprint,
        template: RuntimeDialogueContentTemplateId,
        template_digest: RuntimeDialogueContentTemplateDigest,
        slots: &[crate::plan::RuntimeDialogueContentSlot],
        evaluated: &[RuntimeDialogueValueBinding],
        effect_slots: &[crate::plan::RuntimeDialogueContentEffectSlot],
        effects: &[RuntimeDialogueContentEffectBinding],
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        let manifest = crate::plan::RuntimeDialogueContentTemplateManifest::new_with_effects(
            template,
            template_digest,
            slots.to_vec().into_boxed_slice(),
            effect_slots.to_vec().into_boxed_slice(),
        );
        manifest.validate_slot_schema().map_err(|error| {
            RuntimeDialogueContentValueError::InvalidTemplateManifest {
                message: error.to_string(),
            }
        })?;
        Self::try_from_evaluated_bindings_with_effects(artifact, &manifest, evaluated, effects)
    }

    /// AWBC bridge for a verifier-admitted callback list.  AWBC owns its
    /// runtime type table, so this path deliberately consumes only the
    /// canonical callback sites after AWBC has checked each callback's
    /// capture ABI.  It does not fabricate plan-local type IDs in the core
    /// manifest domain.
    pub(crate) fn try_from_evaluated_bindings_parts_with_effect_bindings(
        artifact: RuntimeArtifactFingerprint,
        template: RuntimeDialogueContentTemplateId,
        template_digest: RuntimeDialogueContentTemplateDigest,
        slots: &[crate::plan::RuntimeDialogueContentSlot],
        evaluated: &[RuntimeDialogueValueBinding],
        effects: &[RuntimeDialogueContentEffectBinding],
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        let mut value = Self::try_from_evaluated_bindings_parts(
            artifact,
            template,
            template_digest,
            slots,
            evaluated,
        )?;
        if !RuntimeSchemaLimits::engine_default().permits_sequence_items(effects.len()) {
            return Err(RuntimeDialogueContentValueError::BindingLimit {
                actual: effects.len(),
                maximum: usize::try_from(RuntimeSchemaLimits::engine_default().max_sequence_items)
                    .unwrap_or(usize::MAX),
            });
        }
        for (index, effect) in effects.iter().enumerate() {
            let expected = crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                .ok_or(RuntimeDialogueContentValueError::NodeLimit {
                maximum: usize::try_from(RuntimeSchemaLimits::engine_default().max_nodes)
                    .unwrap_or(usize::MAX),
            })?;
            if effect.site() != expected {
                return Err(RuntimeDialogueContentValueError::NonCanonicalEffectSite {
                    index,
                    expected,
                    actual: effect.site(),
                });
            }
            let remaining = effect.callback().remaining_arity().map_err(|error| {
                RuntimeDialogueContentValueError::InvalidEffectCallback {
                    index,
                    message: error.to_string(),
                }
            })?;
            if effect.callback().as_structured().is_some()
                && !effect.callback().is_structured_executable_callback()
            {
                return Err(RuntimeDialogueContentValueError::InvalidEffectCallback {
                    index,
                    message: "structured callback site is not an executable Unit body".to_owned(),
                });
            }
            if remaining != 0 {
                return Err(RuntimeDialogueContentValueError::InvalidEffectCallback {
                    index,
                    message: format!("callback has {remaining} remaining parameters"),
                });
            }
        }
        value.effects = effects.to_owned().into_boxed_slice();
        value.validate_structure(RuntimeSchemaLimits::engine_default())?;
        value.validate_runtime_graph(RuntimeSchemaLimits::engine_default())?;
        Ok(value)
    }

    /// Decodes the exact Content-owned runtime envelope using the engine
    /// schema policy.
    pub fn try_from_runtime_value(
        value: &RuntimeValue,
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        Self::try_from_runtime_value_with_limits(value, RuntimeSchemaLimits::engine_default())
    }

    /// Decodes the exact Content-owned runtime envelope with explicit shared
    /// runtime limits.
    pub fn try_from_runtime_value_with_limits(
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        let payload = exact_dialogue_payload(value, RuntimeDialogueOpaqueRole::Content)
            .map_err(|_| RuntimeDialogueContentValueError::InvalidOwner)?;
        let value = Self::decode_payload(payload, limits, 0)?;
        value.validate_structure(limits)?;
        value.validate_runtime_graph(limits)?;
        Ok(value)
    }

    fn try_decode_unvalidated_at(
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
        depth: usize,
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        let payload = exact_dialogue_payload(value, RuntimeDialogueOpaqueRole::Content)
            .map_err(|_| RuntimeDialogueContentValueError::InvalidOwner)?;
        Self::decode_payload(payload, limits, depth)
    }

    /// Exact runtime-plan artifact fingerprint carried by this envelope.
    #[must_use]
    pub const fn artifact(&self) -> RuntimeArtifactFingerprint {
        self.artifact
    }

    /// Plan-local content template identity carried by this envelope.
    #[must_use]
    pub const fn template(&self) -> RuntimeDialogueContentTemplateId {
        self.template
    }

    /// Digest of the immutable fragment template expected by this envelope.
    #[must_use]
    pub const fn template_digest(&self) -> RuntimeDialogueContentTemplateDigest {
        self.template_digest
    }

    /// Canonical source-order role-tagged bindings.
    #[must_use]
    pub fn bindings(&self) -> &[RuntimeDialogueContentBinding] {
        &self.bindings
    }

    /// Canonical source-order effect callback bindings.
    #[must_use]
    pub fn effects(&self) -> &[RuntimeDialogueContentEffectBinding] {
        &self.effects
    }

    /// Finds a callback by its exact document-local effect site identity.
    #[must_use]
    pub fn effect(
        &self,
        site: crate::runtime_id::RuntimeDialogueEffectSiteId,
    ) -> Option<&RuntimeDialogueContentEffectBinding> {
        self.effects
            .get(site.index())
            .filter(|binding| binding.site() == site)
    }

    /// Finds a canonical binding by its document-local slot identity.
    #[must_use]
    pub fn binding(
        &self,
        slot: RuntimeDialogueValueSlotId,
    ) -> Option<&RuntimeDialogueContentBinding> {
        self.bindings
            .get(slot.index())
            .filter(|binding| binding.slot() == slot)
    }

    /// Encodes this typed envelope through the exact Content owner.
    #[must_use]
    pub fn into_runtime_value(self) -> RuntimeValue {
        let bindings = self
            .bindings
            .into_vec()
            .into_iter()
            .map(encode_content_binding)
            .collect();
        let effects = self
            .effects
            .into_vec()
            .into_iter()
            .map(encode_content_effect_binding)
            .collect();
        let payload = RuntimeValue::Tuple(vec![
            RuntimeValue::u8(RUNTIME_DIALOGUE_CONTENT_VALUE_VERSION),
            RuntimeValue::Seq(RuntimeSeq::dense_bytes(self.artifact.as_bytes().to_vec())),
            RuntimeValue::u32(self.template.get().get()),
            RuntimeValue::Seq(RuntimeSeq::dense_bytes(
                self.template_digest.as_bytes().to_vec(),
            )),
            RuntimeValue::Seq(RuntimeSeq::Values(bindings)),
            RuntimeValue::Seq(RuntimeSeq::Values(effects)),
        ]);
        wrap_dialogue_payload(RuntimeDialogueOpaqueRole::Content, payload)
    }

    fn decode_payload(
        payload: &RuntimeValue,
        limits: RuntimeSchemaLimits,
        depth: usize,
    ) -> Result<Self, RuntimeDialogueContentValueError> {
        let maximum_depth = usize::try_from(limits.max_depth)
            .unwrap_or(usize::MAX)
            .min(MAX_RUNTIME_VALUE_NESTING_DEPTH);
        if depth > maximum_depth {
            return Err(RuntimeDialogueContentValueError::NestingLimit {
                maximum: maximum_depth,
            });
        }
        let RuntimeValue::Tuple(fields) = payload else {
            return Err(RuntimeDialogueContentValueError::InvalidPayload);
        };
        let [
            version,
            artifact,
            template,
            template_digest,
            bindings,
            effects,
        ] = fields.as_slice()
        else {
            return Err(RuntimeDialogueContentValueError::InvalidPayload);
        };
        let RuntimeValue::UInt(RuntimeUInt::U8(version)) = version else {
            return Err(RuntimeDialogueContentValueError::InvalidPayload);
        };
        if *version != RUNTIME_DIALOGUE_CONTENT_VALUE_VERSION {
            return Err(RuntimeDialogueContentValueError::UnsupportedVersion {
                actual: *version,
                expected: RUNTIME_DIALOGUE_CONTENT_VALUE_VERSION,
            });
        }
        let artifact = decode_content_bytes(artifact, true)
            .ok_or(RuntimeDialogueContentValueError::InvalidArtifact)?;
        let artifact = <[u8; 32]>::try_from(artifact.as_slice())
            .ok()
            .and_then(|bytes| RuntimeArtifactFingerprint::try_from_bytes(bytes).ok())
            .ok_or(RuntimeDialogueContentValueError::InvalidArtifact)?;
        let RuntimeValue::UInt(RuntimeUInt::U32(template)) = template else {
            return Err(RuntimeDialogueContentValueError::InvalidTemplate);
        };
        let template = std::num::NonZeroU32::new(*template)
            .map(RuntimeDialogueContentTemplateId::from_nonzero)
            .ok_or(RuntimeDialogueContentValueError::InvalidTemplate)?;
        let template_digest = decode_content_bytes(template_digest, true)
            .and_then(|bytes| <[u8; 32]>::try_from(bytes.as_slice()).ok())
            .map(RuntimeDialogueContentTemplateDigest::from_bytes)
            .ok_or(RuntimeDialogueContentValueError::InvalidTemplateDigest)?;
        let RuntimeValue::Seq(sequence) = bindings else {
            return Err(RuntimeDialogueContentValueError::InvalidPayload);
        };
        let Some(bindings) = sequence.as_values() else {
            return Err(RuntimeDialogueContentValueError::InvalidPayload);
        };
        let bindings = bindings
            .iter()
            .enumerate()
            .map(|binding| decode_content_binding(binding, limits, depth))
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let RuntimeValue::Seq(sequence) = effects else {
            return Err(RuntimeDialogueContentValueError::InvalidPayload);
        };
        let Some(effects) = sequence.as_values() else {
            return Err(RuntimeDialogueContentValueError::InvalidPayload);
        };
        let effects = effects
            .iter()
            .enumerate()
            .map(decode_content_effect_binding)
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        Ok(Self {
            artifact,
            template,
            template_digest,
            bindings,
            effects,
        })
    }

    fn validate_structure(
        &self,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimeDialogueContentValueError> {
        self.validate_structure_at(limits, 0)
    }

    fn validate_structure_at(
        &self,
        limits: RuntimeSchemaLimits,
        depth: usize,
    ) -> Result<(), RuntimeDialogueContentValueError> {
        let maximum_depth = usize::try_from(limits.max_depth)
            .unwrap_or(usize::MAX)
            .min(MAX_RUNTIME_VALUE_NESTING_DEPTH);
        if depth > maximum_depth {
            return Err(RuntimeDialogueContentValueError::NestingLimit {
                maximum: maximum_depth,
            });
        }
        if !limits.permits_sequence_items(self.bindings.len()) {
            return Err(RuntimeDialogueContentValueError::BindingLimit {
                actual: self.bindings.len(),
                maximum: usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX),
            });
        }
        for (index, binding) in self.bindings.iter().enumerate() {
            let expected = RuntimeDialogueValueSlotId::from_zero_based(index).ok_or(
                RuntimeDialogueContentValueError::NodeLimit {
                    maximum: usize::try_from(limits.max_nodes).unwrap_or(usize::MAX),
                },
            )?;
            if binding.slot() != expected {
                return Err(RuntimeDialogueContentValueError::NonCanonicalSlot {
                    index,
                    expected,
                    actual: binding.slot(),
                });
            }
            match binding {
                RuntimeDialogueContentBinding::Interpolation {
                    semantic_type,
                    value,
                    ..
                } => {
                    if *semantic_type != value.semantic_type() {
                        return Err(RuntimeDialogueContentValueError::InvalidBindingValue {
                            index,
                            role: binding.role(),
                        });
                    }
                }
                RuntimeDialogueContentBinding::Content {
                    semantic_type,
                    value: nested,
                    ..
                } => {
                    if *semantic_type != RuntimeDialogueOpaqueRole::Content.semantic_identity()
                        || nested.artifact != self.artifact
                    {
                        return if nested.artifact == self.artifact {
                            Err(RuntimeDialogueContentValueError::InvalidBindingValue {
                                index,
                                role: binding.role(),
                            })
                        } else {
                            Err(RuntimeDialogueContentValueError::NestedArtifactMismatch { index })
                        };
                    }
                    let nested_depth = depth.checked_add(1).ok_or(
                        RuntimeDialogueContentValueError::NestingLimit {
                            maximum: maximum_depth,
                        },
                    )?;
                    nested.validate_structure_at(limits, nested_depth)?;
                }
            }
        }
        if !limits.permits_sequence_items(self.effects.len()) {
            return Err(RuntimeDialogueContentValueError::BindingLimit {
                actual: self.effects.len(),
                maximum: usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX),
            });
        }
        for (index, effect) in self.effects.iter().enumerate() {
            let expected = crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                .ok_or(RuntimeDialogueContentValueError::NodeLimit {
                maximum: usize::try_from(limits.max_nodes).unwrap_or(usize::MAX),
            })?;
            if effect.site() != expected {
                return Err(RuntimeDialogueContentValueError::NonCanonicalEffectSite {
                    index,
                    expected,
                    actual: effect.site(),
                });
            }
            let remaining = effect.callback().remaining_arity().map_err(|error| {
                RuntimeDialogueContentValueError::InvalidEffectCallback {
                    index,
                    message: error.to_string(),
                }
            })?;
            if effect.callback().as_structured().is_some()
                && !effect.callback().is_structured_executable_callback()
            {
                return Err(RuntimeDialogueContentValueError::InvalidEffectCallback {
                    index,
                    message: "structured callback site is not an executable Unit body".to_owned(),
                });
            }
            if remaining != 0 {
                return Err(RuntimeDialogueContentValueError::InvalidEffectCallback {
                    index,
                    message: format!("callback has {remaining} remaining parameters"),
                });
            }
        }
        Ok(())
    }

    fn validate_runtime_graph(
        &self,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimeDialogueContentValueError> {
        let runtime = self.clone().into_runtime_value();
        let maximum_depth = usize::try_from(limits.max_depth)
            .unwrap_or(usize::MAX)
            .min(MAX_RUNTIME_VALUE_NESTING_DEPTH);
        runtime.validate_nesting_depth(maximum_depth).map_err(|_| {
            RuntimeDialogueContentValueError::NestingLimit {
                maximum: maximum_depth,
            }
        })?;
        let mut budget = RuntimeContentValueBudget { limits, nodes: 0 };
        budget.visit(&runtime)
    }
}

fn encode_content_binding(binding: RuntimeDialogueContentBinding) -> RuntimeValue {
    RuntimeValue::Tuple(vec![
        RuntimeValue::u32(binding.slot().get().get()),
        RuntimeValue::u8(binding.role().encoded()),
        RuntimeValue::Seq(RuntimeSeq::dense_bytes(
            binding.semantic_type().as_bytes().to_vec(),
        )),
        binding.into_runtime_value(),
    ])
}

fn encode_content_effect_binding(binding: RuntimeDialogueContentEffectBinding) -> RuntimeValue {
    RuntimeValue::Tuple(vec![
        RuntimeValue::u32(binding.site().get().get()),
        RuntimeValue::Function(binding.callback),
    ])
}

fn decode_content_effect_binding(
    (index, value): (usize, &RuntimeValue),
) -> Result<RuntimeDialogueContentEffectBinding, RuntimeDialogueContentValueError> {
    let RuntimeValue::Tuple(fields) = value else {
        return Err(RuntimeDialogueContentValueError::InvalidEffectShape { index });
    };
    let [site, callback] = fields.as_slice() else {
        return Err(RuntimeDialogueContentValueError::InvalidEffectShape { index });
    };
    let RuntimeValue::UInt(RuntimeUInt::U32(site)) = site else {
        return Err(RuntimeDialogueContentValueError::InvalidEffectShape { index });
    };
    let Some(site) = std::num::NonZeroU32::new(*site)
        .map(crate::runtime_id::RuntimeDialogueEffectSiteId::from_accepted_ordinal)
    else {
        return Err(RuntimeDialogueContentValueError::InvalidEffectShape { index });
    };
    let RuntimeValue::Function(callback) = callback else {
        return Err(RuntimeDialogueContentValueError::InvalidEffectShape { index });
    };
    let remaining = callback.remaining_arity().map_err(|error| {
        RuntimeDialogueContentValueError::InvalidEffectCallback {
            index,
            message: error.to_string(),
        }
    })?;
    if remaining != 0 {
        return Err(RuntimeDialogueContentValueError::InvalidEffectCallback {
            index,
            message: format!("callback has {remaining} remaining parameters"),
        });
    }
    Ok(RuntimeDialogueContentEffectBinding::new(
        site,
        callback.clone(),
    ))
}

fn decode_content_binding(
    (index, value): (usize, &RuntimeValue),
    limits: RuntimeSchemaLimits,
    depth: usize,
) -> Result<RuntimeDialogueContentBinding, RuntimeDialogueContentValueError> {
    let RuntimeValue::Tuple(fields) = value else {
        return Err(RuntimeDialogueContentValueError::InvalidBindingShape { index });
    };
    let [slot, role, semantic_type, value] = fields.as_slice() else {
        return Err(RuntimeDialogueContentValueError::InvalidBindingShape { index });
    };
    let RuntimeValue::UInt(RuntimeUInt::U32(slot)) = slot else {
        return Err(RuntimeDialogueContentValueError::InvalidBindingShape { index });
    };
    let Some(slot) =
        std::num::NonZeroU32::new(*slot).map(RuntimeDialogueValueSlotId::from_accepted_ordinal)
    else {
        return Err(RuntimeDialogueContentValueError::InvalidBindingShape { index });
    };
    let RuntimeValue::UInt(RuntimeUInt::U8(role)) = role else {
        return Err(RuntimeDialogueContentValueError::InvalidBindingShape { index });
    };
    let role = RuntimeDialogueValueRole::from_encoded(*role)
        .ok_or(RuntimeDialogueContentValueError::UnknownBindingRole { index, tag: *role })?;
    let semantic_type = decode_content_semantic_type(semantic_type)
        .ok_or(RuntimeDialogueContentValueError::InvalidBindingShape { index })?;
    match role {
        RuntimeDialogueValueRole::Interpolation => {
            let value = RuntimeInlineTextValue::try_decode_runtime_value(value, limits).map_err(
                |source| RuntimeDialogueContentValueError::InvalidInlineTextValue { index, source },
            )?;
            if value.semantic_type() != semantic_type {
                return Err(RuntimeDialogueContentValueError::InvalidBindingValue { index, role });
            }
            Ok(RuntimeDialogueContentBinding::Interpolation {
                slot,
                semantic_type,
                value,
            })
        }
        RuntimeDialogueValueRole::Content => {
            let next_depth =
                depth
                    .checked_add(1)
                    .ok_or(RuntimeDialogueContentValueError::NestingLimit {
                        maximum: usize::try_from(limits.max_depth)
                            .unwrap_or(usize::MAX)
                            .min(MAX_RUNTIME_VALUE_NESTING_DEPTH),
                    })?;
            let value =
                RuntimeDialogueContentValue::try_decode_unvalidated_at(value, limits, next_depth)?;
            Ok(RuntimeDialogueContentBinding::Content {
                slot,
                semantic_type,
                value,
            })
        }
    }
}

fn decode_content_semantic_type(value: &RuntimeValue) -> Option<RuntimeSemanticTypeId> {
    let RuntimeValue::Seq(sequence) = value else {
        return None;
    };
    if sequence.dense_kind() != Some(DenseSeqKind::Bytes) {
        return None;
    }
    let bytes = sequence.as_bytes()?;
    <[u8; 32]>::try_from(bytes)
        .ok()
        .map(RuntimeSemanticTypeId::from_bytes)
}

fn decode_content_bytes(value: &RuntimeValue, exact_bytes_kind: bool) -> Option<Vec<u8>> {
    let RuntimeValue::Seq(sequence) = value else {
        return None;
    };
    if exact_bytes_kind && sequence.dense_kind() != Some(DenseSeqKind::Bytes) {
        return None;
    }
    sequence.as_bytes().map(ToOwned::to_owned)
}

struct RuntimeContentValueBudget {
    limits: RuntimeSchemaLimits,
    nodes: usize,
}

impl RuntimeContentValueBudget {
    fn visit(&mut self, value: &RuntimeValue) -> Result<(), RuntimeDialogueContentValueError> {
        self.nodes = self.nodes.saturating_add(1);
        if !self.limits.permits_nodes(self.nodes) {
            return Err(RuntimeDialogueContentValueError::NodeLimit {
                maximum: usize::try_from(self.limits.max_nodes).unwrap_or(usize::MAX),
            });
        }
        match value {
            RuntimeValue::String(value) => {
                if !self.limits.permits_string_bytes(value.len()) {
                    return Err(RuntimeDialogueContentValueError::StringLimit {
                        maximum: usize::try_from(self.limits.max_string_bytes)
                            .unwrap_or(usize::MAX),
                    });
                }
            }
            RuntimeValue::Tuple(values) => {
                self.visit_values(values)?;
            }
            RuntimeValue::Seq(sequence) => {
                if !self.limits.permits_sequence_items(sequence.len()) {
                    return Err(RuntimeDialogueContentValueError::BindingLimit {
                        actual: sequence.len(),
                        maximum: usize::try_from(self.limits.max_sequence_items)
                            .unwrap_or(usize::MAX),
                    });
                }
                for value in sequence.clone().into_values() {
                    self.visit(&value)?;
                }
            }
            RuntimeValue::Record(fields) => {
                for field in fields {
                    self.visit(field.value())?;
                }
            }
            RuntimeValue::NominalRecord(record) => self.visit_values(record.fields())?,
            RuntimeValue::Opaque(opaque) => self.visit(opaque.payload())?,
            RuntimeValue::Reduction(reduction) => {
                self.visit(reduction.state())?;
                for command in reduction.commands() {
                    self.visit(&command.payload().0)?;
                }
            }
            RuntimeValue::Agent(agent) => {
                for (_, nested) in agent.nested_runtime_values_with_depth() {
                    self.visit(nested)?;
                }
            }
            RuntimeValue::Iterator(iterator) => match iterator {
                crate::value::RuntimeIterator::Values { items, .. } => {
                    if !self.limits.permits_sequence_items(items.len()) {
                        return Err(RuntimeDialogueContentValueError::BindingLimit {
                            actual: items.len(),
                            maximum: usize::try_from(self.limits.max_sequence_items)
                                .unwrap_or(usize::MAX),
                        });
                    }
                    self.visit_values(items)?;
                }
                crate::value::RuntimeIterator::Witness { state, .. } => self.visit(state)?,
                crate::value::RuntimeIterator::Range(_) => {}
            },
            RuntimeValue::Variant { name, payload, .. } => {
                if !self.limits.permits_string_bytes(name.len()) {
                    return Err(RuntimeDialogueContentValueError::StringLimit {
                        maximum: usize::try_from(self.limits.max_string_bytes)
                            .unwrap_or(usize::MAX),
                    });
                }
                if let Some(payload) = payload {
                    self.visit(payload)?;
                }
            }
            RuntimeValue::Function(_)
            | RuntimeValue::ProjectContinuation(_)
            | RuntimeValue::Unit
            | RuntimeValue::Bool(_)
            | RuntimeValue::Int(_)
            | RuntimeValue::UInt(_)
            | RuntimeValue::F32(_)
            | RuntimeValue::F64(_)
            | RuntimeValue::MatrixF32(_)
            | RuntimeValue::MatrixF64(_)
            | RuntimeValue::TensorF32(_)
            | RuntimeValue::TensorF64(_)
            | RuntimeValue::Char(_)
            | RuntimeValue::Duration(_)
            | RuntimeValue::Progress(_)
            | RuntimeValue::Range(_)
            | RuntimeValue::EntityRef(_) => {}
        }
        Ok(())
    }

    fn visit_values(
        &mut self,
        values: &[RuntimeValue],
    ) -> Result<(), RuntimeDialogueContentValueError> {
        if !self.limits.permits_sequence_items(values.len()) {
            return Err(RuntimeDialogueContentValueError::BindingLimit {
                actual: values.len(),
                maximum: usize::try_from(self.limits.max_sequence_items).unwrap_or(usize::MAX),
            });
        }
        for value in values {
            self.visit(value)?;
        }
        Ok(())
    }
}

impl RuntimeDialogueOpaqueRole {
    #[must_use]
    pub const fn standard_type_name(self) -> &'static str {
        match self {
            Self::View => "DialogueView",
            Self::Character => "DialogueCharacter",
            Self::Content => "DialogueContent",
            Self::Occurrence => "DialogueOccurrenceId",
            Self::Stage => "DialogueStage",
            Self::Reveal => "DialogueReveal",
            Self::Action => "DialogueAction",
        }
    }

    #[must_use]
    /// # Panics
    ///
    /// Panics only if a fixed standard dialogue producer identity violates the
    /// validated runtime identity grammar.
    pub fn producer(self) -> RuntimeOpaqueTypeProducerId {
        RuntimeOpaqueTypeProducerId::try_new(match self {
            Self::View => "std.dialogue.view",
            Self::Character => "std.dialogue.character",
            Self::Content => "std.dialogue.content",
            Self::Occurrence => "std.dialogue.occurrence",
            Self::Stage => "std.dialogue.stage",
            Self::Reveal => "std.dialogue.reveal",
            Self::Action => "std.dialogue.action",
        })
        .expect("fixed dialogue runtime producer identities are valid")
    }

    #[must_use]
    pub const fn value_class(self) -> RuntimeOpaqueValueClass {
        RuntimeOpaqueValueClass::Plain
    }

    #[must_use]
    pub const fn persistence(self) -> RuntimeOpaquePersistence {
        RuntimeOpaquePersistence::SnapshotOnly
    }

    /// Exact semantic identity of the corresponding standard checked nominal.
    #[must_use]
    pub fn semantic_identity(self) -> RuntimeSemanticTypeId {
        let mut encoder = crate::pattern::RuntimeSemanticTypeIdentityEncoder::new();
        encoder.write_tag(74);
        encoder.write_str(self.standard_type_name());
        encoder.finish()
    }

    #[must_use]
    pub fn exact_owner(self) -> RuntimeOpaqueTypeOwner {
        RuntimeOpaqueTypeOwner::exact_with(
            self.producer(),
            self.semantic_identity(),
            self.value_class(),
            self.persistence(),
        )
    }

    #[must_use]
    pub fn accepts_exact_owner(self, owner: &RuntimeOpaqueTypeOwner) -> bool {
        owner == &self.exact_owner()
    }
}

impl RuntimeDialogueViewField {
    /// Canonical payload order used by both projection bytecode and values.
    pub const ALL: [Self; 6] = [
        Self::Character,
        Self::Content,
        Self::Occurrence,
        Self::Stage,
        Self::Reveal,
        Self::PrimaryAction,
    ];

    #[must_use]
    pub const fn ordinal(self) -> u32 {
        match self {
            Self::Character => 0,
            Self::Content => 1,
            Self::Occurrence => 2,
            Self::Stage => 3,
            Self::Reveal => 4,
            Self::PrimaryAction => 5,
        }
    }

    #[must_use]
    pub const fn role(self) -> RuntimeDialogueOpaqueRole {
        match self {
            Self::Character => RuntimeDialogueOpaqueRole::Character,
            Self::Content => RuntimeDialogueOpaqueRole::Content,
            Self::Occurrence => RuntimeDialogueOpaqueRole::Occurrence,
            Self::Stage => RuntimeDialogueOpaqueRole::Stage,
            Self::Reveal => RuntimeDialogueOpaqueRole::Reveal,
            Self::PrimaryAction => RuntimeDialogueOpaqueRole::Action,
        }
    }
}

impl RuntimeDialogueViewValue {
    pub fn try_new(
        character: RuntimeValue,
        content: RuntimeValue,
        occurrence: RuntimeValue,
        stage: RuntimeValue,
        reveal: RuntimeValue,
        primary_action: RuntimeValue,
    ) -> Result<Self, RuntimeDialogueValueError> {
        let value = Self {
            character,
            content,
            occurrence,
            stage,
            reveal,
            primary_action,
        };
        for field in RuntimeDialogueViewField::ALL {
            value.validate_field(field)?;
        }
        Ok(value)
    }

    pub fn try_from_runtime_value(value: &RuntimeValue) -> Result<Self, RuntimeDialogueValueError> {
        let payload = exact_dialogue_payload(value, RuntimeDialogueOpaqueRole::View)?;
        let RuntimeValue::Tuple(fields) = payload else {
            return Err(RuntimeDialogueValueError::InvalidViewPayload);
        };
        let [
            character,
            content,
            occurrence,
            stage,
            reveal,
            primary_action,
        ] = fields.as_slice()
        else {
            return Err(RuntimeDialogueValueError::InvalidViewPayload);
        };
        Self::try_new(
            character.clone(),
            content.clone(),
            occurrence.clone(),
            stage.clone(),
            reveal.clone(),
            primary_action.clone(),
        )
    }

    #[must_use]
    pub fn field(&self, field: RuntimeDialogueViewField) -> &RuntimeValue {
        match field {
            RuntimeDialogueViewField::Character => &self.character,
            RuntimeDialogueViewField::Content => &self.content,
            RuntimeDialogueViewField::Occurrence => &self.occurrence,
            RuntimeDialogueViewField::Stage => &self.stage,
            RuntimeDialogueViewField::Reveal => &self.reveal,
            RuntimeDialogueViewField::PrimaryAction => &self.primary_action,
        }
    }

    pub fn into_runtime_value(self) -> RuntimeValue {
        wrap_dialogue_payload(
            RuntimeDialogueOpaqueRole::View,
            RuntimeValue::Tuple(vec![
                self.character,
                self.content,
                self.occurrence,
                self.stage,
                self.reveal,
                self.primary_action,
            ]),
        )
    }

    fn validate_field(
        &self,
        field: RuntimeDialogueViewField,
    ) -> Result<(), RuntimeDialogueValueError> {
        let expected = field.role();
        if field == RuntimeDialogueViewField::Content {
            return RuntimeDialogueContentValue::try_from_runtime_value(self.field(field))
                .map(|_| ())
                .map_err(|_| RuntimeDialogueValueError::InvalidViewFieldOwner { field, expected });
        }
        exact_dialogue_payload(self.field(field), expected)
            .map(|_| ())
            .map_err(|_| RuntimeDialogueValueError::InvalidViewFieldOwner { field, expected })
    }
}

impl RuntimeDialogueActionValue {
    #[must_use]
    pub fn into_runtime_value(self) -> RuntimeValue {
        let payload = match self {
            Self::None => RuntimeValue::Tuple(vec![RuntimeValue::u8(0)]),
            Self::Advance(target) => RuntimeValue::Tuple(vec![
                RuntimeValue::u8(1),
                RuntimeValue::u64(target.dialogue),
                RuntimeValue::u64(target.entry),
                RuntimeValue::u64(target.instance),
                RuntimeValue::u32(target.stage),
                RuntimeValue::u64(target.revision),
            ]),
        };
        wrap_dialogue_payload(RuntimeDialogueOpaqueRole::Action, payload)
    }

    pub fn try_from_runtime_value(value: &RuntimeValue) -> Result<Self, RuntimeDialogueValueError> {
        let payload = exact_dialogue_payload(value, RuntimeDialogueOpaqueRole::Action)?;
        let RuntimeValue::Tuple(fields) = payload else {
            return Err(RuntimeDialogueValueError::InvalidActionPayload);
        };
        match fields.as_slice() {
            [RuntimeValue::UInt(RuntimeUInt::U8(0))] => Ok(Self::None),
            [
                RuntimeValue::UInt(RuntimeUInt::U8(1)),
                RuntimeValue::UInt(RuntimeUInt::U64(dialogue)),
                RuntimeValue::UInt(RuntimeUInt::U64(entry)),
                RuntimeValue::UInt(RuntimeUInt::U64(instance)),
                RuntimeValue::UInt(RuntimeUInt::U32(stage)),
                RuntimeValue::UInt(RuntimeUInt::U64(revision)),
            ] => Ok(Self::Advance(RuntimeDialogueAdvanceAction {
                dialogue: *dialogue,
                entry: *entry,
                instance: *instance,
                stage: *stage,
                revision: *revision,
            })),
            _ => Err(RuntimeDialogueValueError::InvalidActionPayload),
        }
    }
}

fn exact_dialogue_payload(
    value: &RuntimeValue,
    role: RuntimeDialogueOpaqueRole,
) -> Result<&RuntimeValue, RuntimeDialogueValueError> {
    let RuntimeValue::Opaque(opaque) = value else {
        return Err(RuntimeDialogueValueError::InvalidOwner { expected: role });
    };
    role.exact_owner()
        .accepts_opaque_value(opaque)
        .then_some(opaque.payload())
        .ok_or(RuntimeDialogueValueError::InvalidOwner { expected: role })
}

fn wrap_dialogue_payload(role: RuntimeDialogueOpaqueRole, payload: RuntimeValue) -> RuntimeValue {
    let owner = role.exact_owner();
    RuntimeValue::Opaque(RuntimeOpaqueValue::new_exact(&owner, payload))
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum RuntimeHandleKind {
    StageActor = 0,
    Cue = 1,
    Voice = 2,
}

impl RuntimeHandleKind {
    #[must_use]
    pub const fn encoded(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_encoded(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::StageActor,
            1 => Self::Cue,
            2 => Self::Voice,
            _ => return None,
        })
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::StageActor => "stage_actor",
            Self::Cue => "cue",
            Self::Voice => "voice",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Some(match label {
            "stage_actor" => Self::StageActor,
            "cue" => Self::Cue,
            "voice" => Self::Voice,
            _ => return None,
        })
    }

    pub fn try_producer(
        self,
    ) -> Result<RuntimeOpaqueTypeProducerId, crate::entry::RuntimeIdentityError> {
        RuntimeOpaqueTypeProducerId::try_new(match self {
            Self::StageActor => "std.line.stage_actor_handle",
            Self::Cue => "std.line.cue_handle",
            Self::Voice => "std.line.voice_handle",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RuntimeOpaqueValueClass {
    Plain,
    AffineHandle(RuntimeHandleKind),
}

impl RuntimeOpaqueValueClass {
    /// Stable semantic transcript tag used by catalog and ownership digests.
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Plain => 0,
            Self::AffineHandle(kind) => kind.encoded() + 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum RuntimeOpaquePersistence {
    ConstantAndSnapshot = 0,
    SnapshotOnly = 1,
}

impl RuntimeOpaquePersistence {
    /// Stable semantic tag used by catalog and ownership digests.
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::ConstantAndSnapshot,
            1 => Self::SnapshotOnly,
            _ => return None,
        })
    }
}

/// Exact producer evidence and payload for one opaque runtime value.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeOpaqueValue {
    producer: RuntimeOpaqueTypeProducerId,
    semantic_identity: RuntimeSemanticTypeId,
    value_class: RuntimeOpaqueValueClass,
    persistence: RuntimeOpaquePersistence,
    payload: Box<RuntimeValue>,
}

impl RuntimeOpaqueValue {
    pub(crate) fn new_exact(owner: &RuntimeOpaqueTypeOwner, payload: RuntimeValue) -> Self {
        Self {
            producer: owner.producer().clone(),
            semantic_identity: owner.semantic_identity(),
            value_class: owner.value_class(),
            persistence: owner.persistence(),
            payload: Box::new(payload),
        }
    }

    #[must_use]
    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId {
        &self.producer
    }

    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }

    #[must_use]
    pub const fn value_class(&self) -> RuntimeOpaqueValueClass {
        self.value_class
    }

    #[must_use]
    pub const fn persistence(&self) -> RuntimeOpaquePersistence {
        self.persistence
    }

    #[must_use]
    pub const fn payload(&self) -> &RuntimeValue {
        &self.payload
    }

    #[must_use]
    pub fn into_payload(self) -> RuntimeValue {
        *self.payload
    }
}

/// Failure to construct a concrete opaque runtime value.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeOpaqueValueError {
    #[error("producer-wide opaque type is not a concrete runtime value owner")]
    NonConcreteOwner {
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::awbc::schema::AwbcFunctionId;
    use crate::entry::RuntimeSchemaError;
    use crate::pattern::{RuntimeCheckedType, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner};
    use crate::value::{
        AwbcRuntimeValueSnapshot, RuntimeBinding, RuntimeFunctionValue, RuntimeSeq,
        RuntimeValueNestingError,
    };

    fn producer(value: &str) -> RuntimeOpaqueTypeProducerId {
        RuntimeOpaqueTypeProducerId::try_new(value).expect("valid producer")
    }

    fn exact(producer: &str, identity: u8) -> RuntimeOpaqueTypeOwner {
        RuntimeOpaqueTypeOwner::exact(
            self::producer(producer),
            RuntimeSemanticTypeId::from_bytes([identity; 32]),
        )
    }

    #[test]
    fn owner_assignability_is_exact_or_expected_producer_wide() {
        let exact_a = exact("std.test", 1);
        let exact_b = exact("std.test", 2);
        let other = exact("std.other", 1);
        let wide = RuntimeOpaqueTypeOwner::producer_wide(
            producer("std.test"),
            RuntimeSemanticTypeId::from_bytes([9; 32]),
        );
        let other_wide = RuntimeOpaqueTypeOwner::producer_wide(
            producer("std.test"),
            RuntimeSemanticTypeId::from_bytes([8; 32]),
        );

        assert!(exact_a.accepts_owner(&exact_a));
        assert!(!exact_a.accepts_owner(&exact_b));
        assert!(!exact_a.accepts_owner(&other));
        assert!(wide.accepts_owner(&exact_a));
        assert!(!wide.accepts_owner(&other));
        assert!(wide.accepts_owner(&wide));
        assert!(!wide.accepts_owner(&other_wide));
    }

    #[test]
    fn only_exact_owner_wraps_and_checked_acceptance_is_fail_closed() {
        let exact_a = exact("std.test", 1);
        let exact_b = exact("std.test", 2);
        let other = exact("std.other", 1);
        let wide = RuntimeOpaqueTypeOwner::producer_wide(
            producer("std.test"),
            RuntimeSemanticTypeId::from_bytes([9; 32]),
        );

        assert_eq!(
            wide.try_wrap(RuntimeValue::Unit),
            Err(RuntimeOpaqueValueError::NonConcreteOwner {
                producer: producer("std.test"),
                semantic_identity: RuntimeSemanticTypeId::from_bytes([9; 32]),
            })
        );

        let value = exact_a
            .try_wrap(RuntimeValue::String("payload".to_owned()))
            .expect("exact owner wraps");
        assert!(RuntimeCheckedType::Opaque { owner: exact_a }.accepts_value(&value));
        assert!(RuntimeCheckedType::Opaque { owner: wide }.accepts_value(&value));
        assert!(!RuntimeCheckedType::Opaque { owner: exact_b }.accepts_value(&value));
        assert!(!RuntimeCheckedType::Opaque { owner: other }.accepts_value(&value));
        assert!(
            !RuntimeCheckedType::Opaque {
                owner: exact("std.test", 1),
            }
            .accepts_value(&RuntimeValue::String("payload".to_owned()))
        );
    }

    #[test]
    fn complete_result_owner_accepts_both_exact_opaque_branches() {
        let ok = exact("std.ok", 1);
        let error = exact("std.error", 2);
        let checked = RuntimeCheckedType::Result {
            ok: Box::new(RuntimeCheckedType::Opaque { owner: ok.clone() }),
            error: Box::new(RuntimeCheckedType::Opaque {
                owner: error.clone(),
            }),
        };

        assert!(checked.accepts_value(&RuntimeValue::result_ok(
            ok.try_wrap(RuntimeValue::Unit).expect("ok payload")
        )));
        assert!(
            checked.accepts_value(&RuntimeValue::result_err(
                error
                    .try_wrap(RuntimeValue::String("error".to_owned()))
                    .expect("error payload")
            ))
        );
    }

    #[test]
    fn recursive_composites_accept_only_matching_opaque_evidence() {
        let first = exact("std.first", 1);
        let second = exact("std.second", 2);
        let foreign = exact("std.foreign", 1);
        let first_value = first
            .try_wrap(RuntimeValue::Unit)
            .expect("first exact owner wraps");
        let second_value = second
            .try_wrap(RuntimeValue::String("second".to_owned()))
            .expect("second exact owner wraps");
        let foreign_value = foreign
            .try_wrap(RuntimeValue::Unit)
            .expect("foreign exact owner wraps");

        let option = RuntimeCheckedType::Option(Box::new(RuntimeCheckedType::Opaque {
            owner: first.clone(),
        }));
        assert!(option.accepts_value(&RuntimeValue::option_none()));
        assert!(option.accepts_value(&RuntimeValue::option_some(first_value.clone())));
        assert!(!option.accepts_value(&RuntimeValue::option_some(foreign_value.clone())));
        assert!(
            RuntimeCheckedType::Option(Box::new(RuntimeCheckedType::Never))
                .accepts_value(&RuntimeValue::option_none())
        );
        assert!(
            !RuntimeCheckedType::Option(Box::new(RuntimeCheckedType::Never))
                .accepts_value(&RuntimeValue::option_some(RuntimeValue::Unit))
        );

        let tuple = RuntimeCheckedType::Tuple(vec![
            RuntimeCheckedType::Opaque {
                owner: first.clone(),
            },
            RuntimeCheckedType::Opaque {
                owner: second.clone(),
            },
        ]);
        assert!(tuple.accepts_value(&RuntimeValue::Tuple(vec![
            first_value.clone(),
            second_value.clone(),
        ])));
        assert!(!tuple.accepts_value(&RuntimeValue::Tuple(vec![
            foreign_value.clone(),
            second_value,
        ])));
        assert!(
            RuntimeCheckedType::Tuple(Vec::new()).accepts_value(&RuntimeValue::Tuple(Vec::new()))
        );

        let choice = RuntimeCheckedType::Choice(vec![
            RuntimeCheckedType::Opaque {
                owner: first.clone(),
            },
            RuntimeCheckedType::Opaque {
                owner: second.clone(),
            },
        ]);
        assert!(choice.accepts_value(&first_value));
        assert!(
            choice.accepts_value(
                &second
                    .try_wrap(RuntimeValue::Unit)
                    .expect("second exact owner wraps")
            )
        );
        assert!(!choice.accepts_value(&foreign_value));
        assert!(!RuntimeCheckedType::Choice(Vec::new()).accepts_value(&RuntimeValue::Unit));

        let sequence =
            RuntimeCheckedType::Sequence(Box::new(RuntimeCheckedType::Opaque { owner: first }));
        assert!(sequence.accepts_value(&RuntimeValue::Seq(RuntimeSeq::values(Vec::new()))));
        assert!(
            sequence.accepts_value(&RuntimeValue::Seq(RuntimeSeq::values(vec![
                first_value.clone(),
                first_value,
            ])))
        );
        assert!(
            !sequence.accepts_value(&RuntimeValue::Seq(RuntimeSeq::values(vec![foreign_value,])))
        );
        assert!(!RuntimeCheckedType::Never.accepts_value(&RuntimeValue::Unit));
    }

    #[test]
    fn opaque_value_has_tag_16_and_payload_participates_in_nesting() {
        let owner = exact("std.agent_error", 7);
        let value = owner
            .try_wrap(RuntimeValue::Unit)
            .expect("exact owner wraps");
        let mut expected = vec![16];
        expected.extend_from_slice(&15_u32.to_le_bytes());
        expected.extend_from_slice(b"std.agent_error");
        expected.extend_from_slice(&[7; 32]);
        expected.extend_from_slice(&[0, 0]);
        expected.push(1);

        assert_eq!(
            value.try_canonical_bytes(128).expect("canonical bytes"),
            expected
        );
        assert_eq!(
            value.validate_nesting_depth(0),
            Err(RuntimeValueNestingError::Exceeded { maximum: 0 })
        );
        assert_eq!(value.validate_nesting_depth(1), Ok(()));
        assert_eq!(
            value.ownership(),
            crate::value::ownership::RuntimeValueOwnership::Unrestricted
        );
    }

    #[test]
    fn opaque_carriers_round_trip_without_an_ownerless_form() {
        let owner = exact("std.round_trip", 3);
        let checked = RuntimeCheckedType::Opaque {
            owner: owner.clone(),
        };
        let value = owner
            .try_wrap(RuntimeValue::String("payload".to_owned()))
            .expect("exact owner wraps");

        assert_eq!(
            serde_json::from_str::<RuntimeCheckedType>(
                &serde_json::to_string(&checked).expect("checked type serializes")
            )
            .expect("checked type deserializes"),
            checked
        );
        assert_eq!(
            serde_json::from_str::<RuntimeValue>(
                &serde_json::to_string(&value).expect("opaque value serializes")
            )
            .expect("opaque value deserializes"),
            value
        );
        assert!(
            serde_json::from_value::<RuntimeCheckedType>(serde_json::json!({
                "Opaque": {}
            }))
            .is_err()
        );
        assert!(serde_json::from_str::<RuntimeOpaqueTypeProducerId>(r#""""#).is_err());
        assert!(
            serde_json::from_str::<RuntimeOpaqueTypeProducerId>(r#""bad\u0001producer""#).is_err()
        );
    }

    #[test]
    fn opaque_wrapper_does_not_encode_runtime_only_payloads() {
        let function = RuntimeFunctionValue::new_awbc(Vec::new(), AwbcFunctionId(0), Vec::new());
        let value = exact("std.runtime_only", 4)
            .try_wrap(RuntimeValue::Function(function))
            .expect("exact owner wraps after producer validation");

        assert_eq!(
            value.try_canonical_bytes(1024),
            Err(RuntimeSchemaError::Encoding {
                message: "runtime-only value has no replay/save encoding".to_owned(),
            })
        );
    }

    #[test]
    fn admission_discriminants_are_stable() {
        assert_eq!(RuntimeOpaqueTypeAdmission::ExactIdentity.encoded(), 0);
        assert_eq!(RuntimeOpaqueTypeAdmission::ProducerWide.encoded(), 1);
        assert_eq!(RuntimeOpaqueTypeAdmission::from_encoded(2), None);
    }

    #[test]
    fn opaque_value_class_and_persistence_tags_are_stable_and_closed() {
        let handles = [
            (RuntimeHandleKind::StageActor, 0),
            (RuntimeHandleKind::Cue, 1),
            (RuntimeHandleKind::Voice, 2),
        ];
        for (kind, tag) in handles {
            assert_eq!(kind.encoded(), tag);
            assert_eq!(RuntimeHandleKind::from_encoded(tag), Some(kind));
        }
        assert_eq!(RuntimeHandleKind::from_encoded(3), None);
        assert_eq!(RuntimeHandleKind::from_encoded(u8::MAX), None);

        assert_eq!(RuntimeOpaqueValueClass::Plain.semantic_tag(), 0);
        assert_eq!(
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor).semantic_tag(),
            1
        );
        assert_eq!(
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue).semantic_tag(),
            2
        );
        assert_eq!(
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Voice).semantic_tag(),
            3
        );

        let persistence = [
            (RuntimeOpaquePersistence::ConstantAndSnapshot, 0),
            (RuntimeOpaquePersistence::SnapshotOnly, 1),
        ];
        for (kind, tag) in persistence {
            assert_eq!(kind.semantic_tag(), tag);
            assert_eq!(RuntimeOpaquePersistence::from_semantic_tag(tag), Some(kind));
        }
        assert_eq!(RuntimeOpaquePersistence::from_semantic_tag(2), None);
        assert_eq!(RuntimeOpaquePersistence::from_semantic_tag(u8::MAX), None);
    }

    #[test]
    fn affine_snapshot_only_handle_is_not_a_constant_and_round_trips_in_snapshot() {
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            producer("std.line.cue_handle"),
            RuntimeSemanticTypeId::from_bytes([11; 32]),
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        let value = owner
            .try_wrap(RuntimeValue::UInt(crate::value::RuntimeUInt::U32(9)))
            .expect("exact handle owner wraps");
        let wide = RuntimeOpaqueTypeOwner::producer_wide_with(
            producer("std.line.cue_handle"),
            RuntimeSemanticTypeId::from_bytes([12; 32]),
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        assert!(wide.accepts_owner(&owner));
        assert!(!exact("std.line.cue_handle", 11).accepts_owner(&owner));

        assert_eq!(
            value.ownership(),
            crate::value::ownership::RuntimeValueOwnership::Affine
        );
        assert!(value.contains_nonconstant_opaque());
        assert!(
            RuntimeValue::Tuple(vec![RuntimeValue::Unit, value.clone()])
                .contains_nonconstant_opaque()
        );
        assert_eq!(
            value.try_canonical_bytes(128),
            Err(RuntimeSchemaError::Encoding {
                message: "opaque value class/persistence is not constant-admissible".to_owned(),
            })
        );
        let snapshot = AwbcRuntimeValueSnapshot::from_runtime_value(&value)
            .expect("live handle snapshots explicitly");
        assert_eq!(
            snapshot
                .into_runtime_value()
                .expect("snapshot handle restores"),
            value
        );
        assert!(
            !exact("std.line.cue_handle", 11).accepts_opaque_value(match &value {
                RuntimeValue::Opaque(value) => value,
                _ => unreachable!("owner wrapped opaque value"),
            })
        );
    }

    #[test]
    fn dialogue_action_carrier_round_trips_and_rejects_same_producer_forgery() {
        let action = RuntimeDialogueActionValue::Advance(RuntimeDialogueAdvanceAction {
            dialogue: 1,
            entry: 2,
            instance: 3,
            stage: 4,
            revision: 5,
        });
        let value = action.into_runtime_value();
        assert_eq!(
            RuntimeDialogueActionValue::try_from_runtime_value(&value),
            Ok(action)
        );

        let forged_owner = RuntimeOpaqueTypeOwner::exact_with(
            RuntimeDialogueOpaqueRole::Action.producer(),
            RuntimeSemanticTypeId::from_bytes([0xd1; 32]),
            RuntimeDialogueOpaqueRole::Action.value_class(),
            RuntimeDialogueOpaqueRole::Action.persistence(),
        );
        let forged = forged_owner
            .try_wrap(RuntimeValue::Tuple(vec![RuntimeValue::u8(0)]))
            .expect("forged exact owner can construct only its own value");
        assert_eq!(
            RuntimeDialogueActionValue::try_from_runtime_value(&forged),
            Err(RuntimeDialogueValueError::InvalidOwner {
                expected: RuntimeDialogueOpaqueRole::Action,
            })
        );
    }

    #[test]
    fn dialogue_view_field_owner_is_canonical_and_tamper_checked() {
        let wrap = |role: RuntimeDialogueOpaqueRole| {
            if role == RuntimeDialogueOpaqueRole::Content {
                return RuntimeDialogueContentValue::try_new(
                    RuntimeArtifactFingerprint::try_from_bytes([0xe1; 32]).expect("artifact"),
                    RuntimeDialogueContentTemplateId::from_zero_based(0).expect("template"),
                    RuntimeDialogueContentTemplateDigest::from_bytes([0xe2; 32]),
                    [],
                )
                .expect("content")
                .into_runtime_value();
            }
            role.exact_owner()
                .try_wrap(RuntimeValue::Unit)
                .expect("standard dialogue role is exact")
        };
        let view = RuntimeDialogueViewValue::try_new(
            wrap(RuntimeDialogueOpaqueRole::Character),
            wrap(RuntimeDialogueOpaqueRole::Content),
            wrap(RuntimeDialogueOpaqueRole::Occurrence),
            wrap(RuntimeDialogueOpaqueRole::Stage),
            wrap(RuntimeDialogueOpaqueRole::Reveal),
            RuntimeDialogueActionValue::None.into_runtime_value(),
        )
        .expect("canonical dialogue fields admit");
        let runtime_value = view.clone().into_runtime_value();
        assert_eq!(
            RuntimeDialogueViewValue::try_from_runtime_value(&runtime_value),
            Ok(view)
        );
        assert_eq!(RuntimeDialogueViewField::PrimaryAction.ordinal(), 5);

        let forged_action_owner = RuntimeOpaqueTypeOwner::exact_with(
            RuntimeDialogueOpaqueRole::Action.producer(),
            RuntimeSemanticTypeId::from_bytes([0xd2; 32]),
            RuntimeDialogueOpaqueRole::Action.value_class(),
            RuntimeDialogueOpaqueRole::Action.persistence(),
        );
        let forged_action = forged_action_owner
            .try_wrap(RuntimeValue::Tuple(vec![RuntimeValue::u8(0)]))
            .expect("forged exact owner wraps its own payload");
        assert_eq!(
            RuntimeDialogueViewValue::try_new(
                wrap(RuntimeDialogueOpaqueRole::Character),
                wrap(RuntimeDialogueOpaqueRole::Content),
                wrap(RuntimeDialogueOpaqueRole::Occurrence),
                wrap(RuntimeDialogueOpaqueRole::Stage),
                wrap(RuntimeDialogueOpaqueRole::Reveal),
                forged_action,
            ),
            Err(RuntimeDialogueValueError::InvalidViewFieldOwner {
                field: RuntimeDialogueViewField::PrimaryAction,
                expected: RuntimeDialogueOpaqueRole::Action,
            })
        );
    }

    fn content_template() -> RuntimeDialogueContentTemplateId {
        RuntimeDialogueContentTemplateId::from_zero_based(0).expect("template")
    }

    fn content_digest() -> RuntimeDialogueContentTemplateDigest {
        RuntimeDialogueContentTemplateDigest::from_bytes([0x42; 32])
    }

    fn content_artifact(marker: u8) -> RuntimeArtifactFingerprint {
        RuntimeArtifactFingerprint::try_from_bytes([marker; 32]).expect("artifact")
    }

    fn inline_binding(index: usize, text: &str) -> RuntimeDialogueContentBinding {
        let slot = RuntimeDialogueValueSlotId::from_zero_based(index).expect("slot");
        let semantic_type = RuntimeSemanticTypeId::from_bytes([0xa1; 32]);
        RuntimeDialogueContentBinding::Interpolation {
            slot,
            semantic_type,
            value: RuntimeInlineTextValue::try_new(semantic_type, text).expect("inline text"),
        }
    }

    #[test]
    fn content_envelope_round_trips_typed_bindings_and_canonical_wire() {
        let artifact = content_artifact(0x31);
        let value = RuntimeDialogueContentValue::try_new(
            artifact,
            content_template(),
            content_digest(),
            [inline_binding(0, "hello")],
        )
        .expect("typed content envelope");
        let runtime = value.clone().into_runtime_value();
        assert_eq!(
            RuntimeDialogueContentValue::try_from_runtime_value(&runtime),
            Ok(value.clone())
        );
        assert_eq!(
            serde_json::from_str::<RuntimeValue>(
                &serde_json::to_string(&runtime).expect("serialize content envelope")
            )
            .expect("deserialize content envelope"),
            runtime
        );
        assert_eq!(value.bindings().len(), 1);
        assert_eq!(
            value
                .binding(RuntimeDialogueValueSlotId::from_zero_based(0).unwrap())
                .and_then(RuntimeDialogueContentBinding::inline_text)
                .map(RuntimeInlineTextValue::text),
            Some("hello")
        );
    }

    #[test]
    fn content_envelope_round_trips_site_keyed_effect_callbacks_through_awbc_save() {
        let callback = RuntimeFunctionValue::new_awbc(
            Vec::new(),
            AwbcFunctionId(7),
            vec![RuntimeBinding {
                name: "captured".to_owned(),
                value: RuntimeValue::u32(9),
            }],
        );
        let site = crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(0)
            .expect("effect site");
        let value = RuntimeDialogueContentValue::try_new_with_effects(
            content_artifact(0x33),
            content_template(),
            content_digest(),
            [inline_binding(0, "hello")],
            [RuntimeDialogueContentEffectBinding::new(site, callback)],
        )
        .expect("effectful typed content envelope");
        let runtime = value.clone().into_runtime_value();
        assert_eq!(
            RuntimeDialogueContentValue::try_from_runtime_value(&runtime),
            Ok(value.clone())
        );
        assert!(serde_json::to_string(&value).is_err());
        let snapshot = AwbcRuntimeValueSnapshot::from_runtime_value(&runtime)
            .expect("AWBC snapshot preserves callback authority");
        assert_eq!(
            snapshot
                .into_runtime_value()
                .expect("AWBC snapshot restores callback"),
            runtime
        );
        assert_eq!(value.effects().len(), 1);
        assert_eq!(
            value
                .effect(site)
                .map(RuntimeDialogueContentEffectBinding::site),
            Some(site)
        );
    }

    #[test]
    fn content_envelope_rejects_empty_or_untyped_payloads() {
        let owner = RuntimeDialogueOpaqueRole::Content.exact_owner();
        for payload in [RuntimeValue::Unit, RuntimeValue::Tuple(Vec::new())] {
            let value = owner.try_wrap(payload).expect("exact owner wraps payload");
            assert!(matches!(
                RuntimeDialogueContentValue::try_from_runtime_value(&value),
                Err(RuntimeDialogueContentValueError::InvalidPayload)
            ));
        }
        let direct_string = owner
            .try_wrap(RuntimeValue::String("not a typed capture".to_owned()))
            .expect("exact owner wraps payload");
        assert!(RuntimeDialogueContentValue::try_from_runtime_value(&direct_string).is_err());
    }

    #[test]
    fn evaluated_content_bindings_report_schema_count_mismatch_separately() {
        let slot = RuntimeDialogueValueSlotId::from_zero_based(0).expect("slot");
        let semantic_type = RuntimeSemanticTypeId::from_bytes([0xa1; 32]);
        let manifest = crate::plan::RuntimeDialogueContentTemplateManifest::new_with_effects(
            content_template(),
            content_digest(),
            vec![crate::plan::RuntimeDialogueContentSlot::new(
                slot,
                RuntimeDialogueValueRole::Interpolation,
                semantic_type,
            )]
            .into_boxed_slice(),
            Box::new([]),
        );
        assert_eq!(
            RuntimeDialogueContentValue::try_from_evaluated_bindings(
                content_artifact(0x61),
                &manifest,
                &[],
            ),
            Err(RuntimeDialogueContentValueError::BindingCountMismatch {
                expected: 1,
                actual: 0,
            })
        );
    }

    #[test]
    fn content_envelope_rejects_wrong_roles_slots_and_nested_artifacts() {
        let artifact = content_artifact(0x51);
        let nested = RuntimeDialogueContentValue::try_new(
            content_artifact(0x52),
            content_template(),
            content_digest(),
            [],
        )
        .expect("nested value");
        let nested_binding = RuntimeDialogueContentBinding::Content {
            slot: RuntimeDialogueValueSlotId::from_zero_based(0).unwrap(),
            semantic_type: RuntimeDialogueOpaqueRole::Content.semantic_identity(),
            value: nested,
        };
        assert!(matches!(
            RuntimeDialogueContentValue::try_new(
                artifact,
                content_template(),
                content_digest(),
                [nested_binding],
            ),
            Err(RuntimeDialogueContentValueError::NestedArtifactMismatch { index: 0 })
        ));

        let owner = RuntimeDialogueOpaqueRole::Content.exact_owner();
        let malformed_binding = RuntimeValue::Tuple(vec![
            RuntimeValue::u32(2),
            RuntimeValue::u8(RuntimeDialogueValueRole::Interpolation.encoded()),
            RuntimeValue::Seq(RuntimeSeq::dense_bytes([0xa1; 32].to_vec())),
            RuntimeValue::String("raw string is not a typed capture".to_owned()),
        ]);
        let payload = RuntimeValue::Tuple(vec![
            RuntimeValue::u8(RUNTIME_DIALOGUE_CONTENT_VALUE_VERSION),
            RuntimeValue::Seq(RuntimeSeq::dense_bytes(artifact.as_bytes().to_vec())),
            RuntimeValue::u32(content_template().get().get()),
            RuntimeValue::Seq(RuntimeSeq::dense_bytes(
                content_digest().as_bytes().to_vec(),
            )),
            RuntimeValue::Seq(RuntimeSeq::Values(vec![malformed_binding])),
            RuntimeValue::Seq(RuntimeSeq::Values(Vec::new())),
        ]);
        let value = owner.try_wrap(payload).expect("exact owner wraps payload");
        assert!(matches!(
            RuntimeDialogueContentValue::try_from_runtime_value(&value),
            Err(
                RuntimeDialogueContentValueError::InvalidInlineTextValue { .. }
                    | RuntimeDialogueContentValueError::NonCanonicalSlot { .. },
            )
        ));
    }
}
