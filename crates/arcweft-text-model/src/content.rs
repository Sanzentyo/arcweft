//! Immutable typed dialogue-content fragments and their pure materializer.

use std::collections::BTreeMap;

use arcweft_core::effect::RuntimeArtifactFingerprint;
use arcweft_core::entry::{RuntimeDialogueContentTemplateDigest, RuntimeSchemaLimits};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::RuntimeDialogueValueRole;
use arcweft_core::runtime_id::{
    RuntimeDialogueContentTemplateId, RuntimeDialogueEffectSiteId, RuntimeDialogueMarkId,
    RuntimeDialogueValueSlotId,
};
use arcweft_core::value::{
    MAX_RUNTIME_VALUE_NESTING_DEPTH, RuntimeDialogueContentBinding, RuntimeDialogueContentValue,
    RuntimeDialogueContentValueError, RuntimeDialogueOpaqueRole,
};
use arcweft_dialogue::{InlineFailurePolicy, InlineFailureSelection, InlineFallback};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::{RichTextControl, RichTextDocument, RichTextNode};

/// One slot declaration in an immutable fragment's exact value schema.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueContentTemplateSlot {
    slot: RuntimeDialogueValueSlotId,
    role: RuntimeDialogueValueRole,
    semantic_type: RuntimeSemanticTypeId,
}

impl DialogueContentTemplateSlot {
    #[must_use]
    pub const fn new(
        slot: RuntimeDialogueValueSlotId,
        role: RuntimeDialogueValueRole,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Self {
        Self {
            slot,
            role,
            semantic_type,
        }
    }

    #[must_use]
    pub const fn slot(&self) -> RuntimeDialogueValueSlotId {
        self.slot
    }

    #[must_use]
    pub const fn role(&self) -> RuntimeDialogueValueRole {
        self.role
    }

    #[must_use]
    pub const fn semantic_type(&self) -> RuntimeSemanticTypeId {
        self.semantic_type
    }
}

/// One mark declared by an immutable fragment template.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueContentTemplateMark {
    id: RuntimeDialogueMarkId,
    diagnostic_name: String,
}

impl DialogueContentTemplateMark {
    #[must_use]
    pub fn new(id: RuntimeDialogueMarkId, diagnostic_name: impl Into<String>) -> Self {
        Self {
            id,
            diagnostic_name: diagnostic_name.into(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> RuntimeDialogueMarkId {
        self.id
    }

    #[must_use]
    pub fn diagnostic_name(&self) -> &str {
        &self.diagnostic_name
    }
}

/// One inline effect site declared by an immutable fragment template.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueContentTemplateEffect {
    id: RuntimeDialogueEffectSiteId,
}

impl DialogueContentTemplateEffect {
    #[must_use]
    pub const fn new(id: RuntimeDialogueEffectSiteId) -> Self {
        Self { id }
    }

    #[must_use]
    pub const fn id(&self) -> RuntimeDialogueEffectSiteId {
        self.id
    }
}

/// Structural failure in one immutable fragment template.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialogueContentFragmentTemplateError {
    #[error("fragment template digest does not match its canonical template transcript")]
    DigestMismatch {
        expected: RuntimeDialogueContentTemplateDigest,
        actual: RuntimeDialogueContentTemplateDigest,
    },
    #[error("fragment template slots are not canonical and contiguous")]
    NonCanonicalSlots,
    #[error("fragment template marks are not canonical and contiguous")]
    NonCanonicalMarks,
    #[error("fragment template effects are not canonical and contiguous")]
    NonCanonicalEffects,
    #[error("fragment template Content slot {slot} does not use the exact Content semantic type")]
    InvalidContentSemanticType { slot: RuntimeDialogueValueSlotId },
    #[error("fragment template node {node} references an undeclared slot {slot}")]
    UndeclaredSlot {
        node: usize,
        slot: RuntimeDialogueValueSlotId,
    },
    #[error(
        "fragment template node {node} uses slot {slot} with role {actual:?}, expected {expected:?}"
    )]
    SlotRoleMismatch {
        node: usize,
        slot: RuntimeDialogueValueSlotId,
        actual: RuntimeDialogueValueRole,
        expected: RuntimeDialogueValueRole,
    },
    #[error("fragment template node {node} references undeclared mark {mark}")]
    UndeclaredMark {
        node: usize,
        mark: RuntimeDialogueMarkId,
    },
    #[error("fragment template node {node} references undeclared effect {effect}")]
    UndeclaredEffect {
        node: usize,
        effect: RuntimeDialogueEffectSiteId,
    },
}

/// One immutable rich-text fragment addressed by a dedicated template
/// identity and checked digest.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogueContentFragmentTemplate {
    id: RuntimeDialogueContentTemplateId,
    digest: RuntimeDialogueContentTemplateDigest,
    slots: Vec<DialogueContentTemplateSlot>,
    marks: Vec<DialogueContentTemplateMark>,
    effects: Vec<DialogueContentTemplateEffect>,
    content: RichTextDocument,
}

impl DialogueContentFragmentTemplate {
    /// Creates and validates a static template with no dynamic slots, marks,
    /// or effects.
    pub fn new(
        id: RuntimeDialogueContentTemplateId,
        digest: RuntimeDialogueContentTemplateDigest,
        content: RichTextDocument,
    ) -> Result<Self, DialogueContentFragmentTemplateError> {
        Self::try_new(id, digest, Vec::new(), Vec::new(), Vec::new(), content)
    }

    /// Creates a template whose digest is owned by this text-model
    /// authority. The digest covers the versioned schema, exact slot/mark/
    /// effect manifests, and the canonical rich-text node transcript; the
    /// dense template identity is deliberately not part of the digest.
    pub fn try_new_canonical(
        id: RuntimeDialogueContentTemplateId,
        slots: Vec<DialogueContentTemplateSlot>,
        marks: Vec<DialogueContentTemplateMark>,
        effects: Vec<DialogueContentTemplateEffect>,
        content: RichTextDocument,
    ) -> Result<Self, DialogueContentFragmentTemplateError> {
        let digest = Self::canonical_digest_for(&slots, &marks, &effects, &content);
        Self::try_new(id, digest, slots, marks, effects, content)
    }

    /// Creates and validates a template's complete slot/mark/effect schema.
    pub fn try_new(
        id: RuntimeDialogueContentTemplateId,
        digest: RuntimeDialogueContentTemplateDigest,
        slots: Vec<DialogueContentTemplateSlot>,
        marks: Vec<DialogueContentTemplateMark>,
        effects: Vec<DialogueContentTemplateEffect>,
        content: RichTextDocument,
    ) -> Result<Self, DialogueContentFragmentTemplateError> {
        let expected = Self::canonical_digest_for(&slots, &marks, &effects, &content);
        if digest != expected {
            return Err(DialogueContentFragmentTemplateError::DigestMismatch {
                expected,
                actual: digest,
            });
        }
        let template = Self {
            id,
            digest,
            slots,
            marks,
            effects,
            content,
        };
        template.validate()?;
        Ok(template)
    }

    /// Computes the canonical digest for a template manifest and node tree.
    #[must_use]
    pub fn canonical_digest_for(
        slots: &[DialogueContentTemplateSlot],
        marks: &[DialogueContentTemplateMark],
        effects: &[DialogueContentTemplateEffect],
        content: &RichTextDocument,
    ) -> RuntimeDialogueContentTemplateDigest {
        #[derive(Serialize)]
        struct Transcript<'a> {
            version: u8,
            slots: &'a [DialogueContentTemplateSlot],
            marks: &'a [DialogueContentTemplateMark],
            effects: &'a [DialogueContentTemplateEffect],
            content: &'a RichTextDocument,
        }

        let bytes = serde_json::to_vec(&Transcript {
            version: 1,
            slots,
            marks,
            effects,
            content,
        })
        .expect("dialogue content template transcript is serializable");
        RuntimeDialogueContentTemplateDigest::from_bytes(*blake3::hash(&bytes).as_bytes())
    }

    #[must_use]
    pub const fn id(&self) -> RuntimeDialogueContentTemplateId {
        self.id
    }

    #[must_use]
    pub const fn digest(&self) -> RuntimeDialogueContentTemplateDigest {
        self.digest
    }

    #[must_use]
    pub const fn template_digest(&self) -> RuntimeDialogueContentTemplateDigest {
        self.digest
    }

    #[must_use]
    pub fn slots(&self) -> &[DialogueContentTemplateSlot] {
        &self.slots
    }

    #[must_use]
    pub fn marks(&self) -> &[DialogueContentTemplateMark] {
        &self.marks
    }

    #[must_use]
    pub fn effects(&self) -> &[DialogueContentTemplateEffect] {
        &self.effects
    }

    #[must_use]
    pub const fn content(&self) -> &RichTextDocument {
        &self.content
    }

    fn validate(&self) -> Result<(), DialogueContentFragmentTemplateError> {
        for (index, slot) in self.slots.iter().enumerate() {
            let expected = RuntimeDialogueValueSlotId::from_zero_based(index)
                .ok_or(DialogueContentFragmentTemplateError::NonCanonicalSlots)?;
            if slot.slot() != expected {
                return Err(DialogueContentFragmentTemplateError::NonCanonicalSlots);
            }
            if slot.role() == RuntimeDialogueValueRole::Content
                && slot.semantic_type() != RuntimeDialogueOpaqueRole::Content.semantic_identity()
            {
                return Err(
                    DialogueContentFragmentTemplateError::InvalidContentSemanticType {
                        slot: slot.slot(),
                    },
                );
            }
        }
        for (index, mark) in self.marks.iter().enumerate() {
            let expected = RuntimeDialogueMarkId::from_zero_based(index)
                .ok_or(DialogueContentFragmentTemplateError::NonCanonicalMarks)?;
            if mark.id() != expected {
                return Err(DialogueContentFragmentTemplateError::NonCanonicalMarks);
            }
        }
        for (index, effect) in self.effects.iter().enumerate() {
            let expected = RuntimeDialogueEffectSiteId::from_zero_based(index)
                .ok_or(DialogueContentFragmentTemplateError::NonCanonicalEffects)?;
            if effect.id() != expected {
                return Err(DialogueContentFragmentTemplateError::NonCanonicalEffects);
            }
        }

        self.validate_nodes(&self.content.nodes)?;
        Ok(())
    }

    fn validate_nodes(
        &self,
        nodes: &[RichTextNode],
    ) -> Result<(), DialogueContentFragmentTemplateError> {
        for (node, value) in nodes.iter().enumerate() {
            match value {
                RichTextNode::Interpolation { slot, .. } => {
                    self.require_slot(node, *slot, RuntimeDialogueValueRole::Interpolation)?;
                }
                RichTextNode::ContentInsert { slot, .. } => {
                    self.require_slot(node, *slot, RuntimeDialogueValueRole::Content)?;
                }
                RichTextNode::Scope { body, .. } | RichTextNode::Ruby { body, .. } => {
                    self.validate_nodes(body)?;
                }
                RichTextNode::Raw { .. } => {}
                RichTextNode::Control { control } => match control {
                    RichTextControl::Mark { mark, .. } => {
                        if !self.marks.iter().any(|declared| declared.id() == *mark) {
                            return Err(DialogueContentFragmentTemplateError::UndeclaredMark {
                                node,
                                mark: *mark,
                            });
                        }
                    }
                    RichTextControl::Effect { site } => {
                        if !self.effects.iter().any(|declared| declared.id() == *site) {
                            return Err(DialogueContentFragmentTemplateError::UndeclaredEffect {
                                node,
                                effect: *site,
                            });
                        }
                    }
                    _ => {}
                },
                RichTextNode::Text { .. } | RichTextNode::HostEvent { .. } => {}
            }
        }
        Ok(())
    }

    pub(crate) fn validate_for_catalog(&self) -> Result<(), DialogueContentFragmentTemplateError> {
        let expected =
            Self::canonical_digest_for(&self.slots, &self.marks, &self.effects, &self.content);
        if self.digest != expected {
            return Err(DialogueContentFragmentTemplateError::DigestMismatch {
                expected,
                actual: self.digest,
            });
        }
        self.validate()
    }

    fn require_slot(
        &self,
        node: usize,
        slot: RuntimeDialogueValueSlotId,
        expected: RuntimeDialogueValueRole,
    ) -> Result<(), DialogueContentFragmentTemplateError> {
        let Some(declared) = self.slots.iter().find(|declared| declared.slot() == slot) else {
            return Err(DialogueContentFragmentTemplateError::UndeclaredSlot { node, slot });
        };
        if declared.role() != expected {
            return Err(DialogueContentFragmentTemplateError::SlotRoleMismatch {
                node,
                slot,
                actual: declared.role(),
                expected,
            });
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DialogueContentFragmentTemplateWire {
    id: RuntimeDialogueContentTemplateId,
    digest: RuntimeDialogueContentTemplateDigest,
    slots: Vec<DialogueContentTemplateSlot>,
    marks: Vec<DialogueContentTemplateMark>,
    effects: Vec<DialogueContentTemplateEffect>,
    content: RichTextDocument,
}

impl Serialize for DialogueContentFragmentTemplate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DialogueContentFragmentTemplateWire {
            id: self.id,
            digest: self.digest,
            slots: self.slots.clone(),
            marks: self.marks.clone(),
            effects: self.effects.clone(),
            content: self.content.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DialogueContentFragmentTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DialogueContentFragmentTemplateWire::deserialize(deserializer)?;
        Self::try_new(
            wire.id,
            wire.digest,
            wire.slots,
            wire.marks,
            wire.effects,
            wire.content,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// Invalid immutable fragment catalog transcript.
#[derive(Debug, Error, PartialEq)]
pub enum DialogueContentFragmentCatalogError {
    #[error("dialogue content fragment catalog is not in canonical template-id order")]
    NonCanonicalOrder,
    #[error("dialogue content fragment catalog repeats template `{template}`")]
    DuplicateTemplate {
        template: RuntimeDialogueContentTemplateId,
    },
    #[error("fragment template `{template}` is invalid: {source}")]
    InvalidTemplate {
        template: RuntimeDialogueContentTemplateId,
        #[source]
        source: Box<DialogueContentFragmentTemplateError>,
    },
}

/// Artifact-pinned immutable catalog of rich-text content fragments.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogueContentFragmentCatalog {
    artifact: RuntimeArtifactFingerprint,
    templates: Vec<DialogueContentFragmentTemplate>,
    by_template: BTreeMap<RuntimeDialogueContentTemplateId, usize>,
}

impl DialogueContentFragmentCatalog {
    /// Builds a canonical catalog from templates sorted by their dedicated
    /// immutable template identity. The artifact identity is shared by every
    /// catalog entry.
    pub fn try_from_templates(
        artifact: RuntimeArtifactFingerprint,
        templates: Vec<DialogueContentFragmentTemplate>,
    ) -> Result<Self, DialogueContentFragmentCatalogError> {
        for template in &templates {
            template.validate_for_catalog().map_err(|source| {
                DialogueContentFragmentCatalogError::InvalidTemplate {
                    template: template.id(),
                    source: Box::new(source),
                }
            })?;
        }
        if templates
            .windows(2)
            .any(|pair| pair[0].id() >= pair[1].id())
        {
            if let Some(pair) = templates
                .windows(2)
                .find(|pair| pair[0].id() == pair[1].id())
            {
                return Err(DialogueContentFragmentCatalogError::DuplicateTemplate {
                    template: pair[0].id(),
                });
            }
            return Err(DialogueContentFragmentCatalogError::NonCanonicalOrder);
        }
        let mut by_template = BTreeMap::new();
        for (index, template) in templates.iter().enumerate() {
            if by_template.insert(template.id(), index).is_some() {
                return Err(DialogueContentFragmentCatalogError::DuplicateTemplate {
                    template: template.id(),
                });
            }
        }
        Ok(Self {
            artifact,
            templates,
            by_template,
        })
    }

    pub fn new(
        artifact: RuntimeArtifactFingerprint,
        templates: Vec<DialogueContentFragmentTemplate>,
    ) -> Result<Self, DialogueContentFragmentCatalogError> {
        Self::try_from_templates(artifact, templates)
    }

    #[must_use]
    pub const fn artifact(&self) -> RuntimeArtifactFingerprint {
        self.artifact
    }

    #[must_use]
    pub fn templates(&self) -> &[DialogueContentFragmentTemplate] {
        &self.templates
    }

    #[must_use]
    pub fn find(
        &self,
        id: RuntimeDialogueContentTemplateId,
    ) -> Option<&DialogueContentFragmentTemplate> {
        self.by_template
            .get(&id)
            .and_then(|index| self.templates.get(*index))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DialogueContentFragmentCatalogWire {
    artifact: RuntimeArtifactFingerprint,
    templates: Vec<DialogueContentFragmentTemplate>,
}

impl Serialize for DialogueContentFragmentCatalog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DialogueContentFragmentCatalogWire {
            artifact: self.artifact,
            templates: self.templates.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DialogueContentFragmentCatalog {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DialogueContentFragmentCatalogWire::deserialize(deserializer)?;
        Self::try_from_templates(wire.artifact, wire.templates).map_err(serde::de::Error::custom)
    }
}

/// Origin of one materialized node in its immutable source template.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueContentNodeOrigin {
    template: RuntimeDialogueContentTemplateId,
    occurrence: Box<[u32]>,
    node: usize,
}

impl DialogueContentNodeOrigin {
    #[must_use]
    pub const fn template(&self) -> RuntimeDialogueContentTemplateId {
        self.template
    }

    #[must_use]
    pub fn occurrence(&self) -> &[u32] {
        &self.occurrence
    }

    #[must_use]
    pub const fn node(&self) -> usize {
        self.node
    }
}

/// One rebased mark in materialized dialogue content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedDialogueMark {
    id: RuntimeDialogueMarkId,
    diagnostic_name: String,
    origin: DialogueContentNodeOrigin,
}

impl MaterializedDialogueMark {
    #[must_use]
    pub const fn id(&self) -> RuntimeDialogueMarkId {
        self.id
    }

    #[must_use]
    pub fn diagnostic_name(&self) -> &str {
        &self.diagnostic_name
    }

    #[must_use]
    pub const fn origin(&self) -> &DialogueContentNodeOrigin {
        &self.origin
    }
}

/// One rebased inline effect site in materialized dialogue content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedDialogueEffect {
    id: RuntimeDialogueEffectSiteId,
    origin: DialogueContentNodeOrigin,
}

impl MaterializedDialogueEffect {
    #[must_use]
    pub const fn id(&self) -> RuntimeDialogueEffectSiteId {
        self.id
    }

    #[must_use]
    pub const fn origin(&self) -> &DialogueContentNodeOrigin {
        &self.origin
    }
}

/// Fully expanded content tree and its rebased typed runtime metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterializedDialogueContent {
    document: RichTextDocument,
    bindings: Box<[RuntimeDialogueContentBinding]>,
    marks: Box<[MaterializedDialogueMark]>,
    effects: Box<[MaterializedDialogueEffect]>,
    origins: Box<[DialogueContentNodeOrigin]>,
}

impl MaterializedDialogueContent {
    #[must_use]
    pub const fn document(&self) -> &RichTextDocument {
        &self.document
    }

    #[must_use]
    pub fn into_document(self) -> RichTextDocument {
        self.document
    }

    #[must_use]
    pub fn bindings(&self) -> &[RuntimeDialogueContentBinding] {
        &self.bindings
    }

    #[must_use]
    pub fn marks(&self) -> &[MaterializedDialogueMark] {
        &self.marks
    }

    #[must_use]
    pub fn effects(&self) -> &[MaterializedDialogueEffect] {
        &self.effects
    }

    /// Origins follow the materialized tree in pre-order. Every structural
    /// scope and every leaf owns exactly one origin row.
    #[must_use]
    pub fn origins(&self) -> &[DialogueContentNodeOrigin] {
        &self.origins
    }
}

/// Failure to materialize one typed content envelope into a rich-text tree.
#[derive(Debug, Error, PartialEq)]
pub enum DialogueContentMaterializationError {
    #[error("content value belongs to artifact {actual:?}, expected {expected:?}")]
    ArtifactMismatch {
        expected: RuntimeArtifactFingerprint,
        actual: RuntimeArtifactFingerprint,
    },
    #[error("content template `{template}` is absent from the immutable catalog")]
    MissingTemplate {
        template: RuntimeDialogueContentTemplateId,
    },
    #[error("content template `{template}` has digest {actual:?}, expected {expected:?}")]
    TemplateDigestMismatch {
        template: RuntimeDialogueContentTemplateId,
        expected: RuntimeDialogueContentTemplateDigest,
        actual: RuntimeDialogueContentTemplateDigest,
    },
    #[error("content insertion slot {slot} has no runtime binding")]
    MissingBinding { slot: RuntimeDialogueValueSlotId },
    #[error("content insertion slot {slot} has role {actual:?}, expected Content")]
    InvalidBindingRole {
        slot: RuntimeDialogueValueSlotId,
        actual: RuntimeDialogueValueRole,
    },
    #[error("content value does not match template slot schema at slot {slot}")]
    BindingSchemaMismatch { slot: RuntimeDialogueValueSlotId },
    #[error("content node {node} references an unrebased slot {slot}")]
    UnrebasedSlot {
        node: usize,
        slot: RuntimeDialogueValueSlotId,
    },
    #[error("content node {node} references an unrebased mark {mark}")]
    UnrebasedMark {
        node: usize,
        mark: RuntimeDialogueMarkId,
    },
    #[error("content node {node} references an unrebased effect {effect}")]
    UnrebasedEffect {
        node: usize,
        effect: RuntimeDialogueEffectSiteId,
    },
    #[error(transparent)]
    InvalidContentValue(#[from] RuntimeDialogueContentValueError),
    #[error("content insertion slot {slot} failed: {source}")]
    FailedInsertion {
        slot: RuntimeDialogueValueSlotId,
        source: Box<Self>,
    },
    #[error("materialized content exceeds the shared nesting limit of {maximum}")]
    NestingLimit { maximum: usize },
    #[error("materialized content exceeds the shared node limit of {maximum}")]
    NodeLimit { maximum: usize },
    #[error("materialized content exceeds the shared sequence limit of {maximum}")]
    SequenceLimit { maximum: usize },
    #[error("materialized content occurrence path is too deep")]
    OccurrencePathOverflow,
}

/// Pure expansion boundary for an artifact-pinned fragment catalog.
#[derive(Clone, Copy, Debug)]
pub struct DialogueContentMaterializer<'a> {
    catalog: &'a DialogueContentFragmentCatalog,
    limits: RuntimeSchemaLimits,
}

impl<'a> DialogueContentMaterializer<'a> {
    #[must_use]
    pub const fn new(catalog: &'a DialogueContentFragmentCatalog) -> Self {
        Self {
            catalog,
            limits: RuntimeSchemaLimits::engine_default(),
        }
    }

    #[must_use]
    pub const fn with_limits(
        catalog: &'a DialogueContentFragmentCatalog,
        limits: RuntimeSchemaLimits,
    ) -> Self {
        Self { catalog, limits }
    }

    #[must_use]
    pub const fn catalog(&self) -> &'a DialogueContentFragmentCatalog {
        self.catalog
    }

    #[must_use]
    pub const fn limits(&self) -> RuntimeSchemaLimits {
        self.limits
    }

    /// Expands every ContentInsert into the immutable fragment tree. Dynamic
    /// interpolation and condition nodes remain typed nodes for their later
    /// resolver; this boundary never decodes arbitrary RuntimeValue values.
    pub fn materialize(
        &self,
        value: &RuntimeDialogueContentValue,
    ) -> Result<MaterializedDialogueContent, DialogueContentMaterializationError> {
        self.materialize_with_policy(value, &InlineFailurePolicy::FailLine)
    }

    /// Expands a Content value using the effective CharacterDialogue inline
    /// failure policy for inherited insertion sites.
    pub fn materialize_with_policy(
        &self,
        value: &RuntimeDialogueContentValue,
        inherited_policy: &InlineFailurePolicy,
    ) -> Result<MaterializedDialogueContent, DialogueContentMaterializationError> {
        if value.artifact() != self.catalog.artifact() {
            return Err(DialogueContentMaterializationError::ArtifactMismatch {
                expected: self.catalog.artifact(),
                actual: value.artifact(),
            });
        }
        let mut state = MaterializationState {
            catalog: self.catalog,
            limits: self.limits,
            inherited_policy,
            node_count: 0,
            bindings: Vec::new(),
            marks: Vec::new(),
            effects: Vec::new(),
            origins: Vec::new(),
        };
        let nodes = state.expand(value, 0, Vec::new())?;
        Ok(MaterializedDialogueContent {
            document: RichTextDocument::new(nodes),
            bindings: state.bindings.into_boxed_slice(),
            marks: state.marks.into_boxed_slice(),
            effects: state.effects.into_boxed_slice(),
            origins: state.origins.into_boxed_slice(),
        })
    }
}

struct MaterializationState<'a> {
    catalog: &'a DialogueContentFragmentCatalog,
    limits: RuntimeSchemaLimits,
    inherited_policy: &'a InlineFailurePolicy,
    node_count: usize,
    bindings: Vec<RuntimeDialogueContentBinding>,
    marks: Vec<MaterializedDialogueMark>,
    effects: Vec<MaterializedDialogueEffect>,
    origins: Vec<DialogueContentNodeOrigin>,
}

impl MaterializationState<'_> {
    fn expand(
        &mut self,
        value: &RuntimeDialogueContentValue,
        depth: usize,
        occurrence: Vec<u32>,
    ) -> Result<Vec<RichTextNode>, DialogueContentMaterializationError> {
        let maximum_depth = usize::try_from(self.limits.max_depth)
            .unwrap_or(usize::MAX)
            .min(MAX_RUNTIME_VALUE_NESTING_DEPTH);
        if depth > maximum_depth {
            return Err(DialogueContentMaterializationError::NestingLimit {
                maximum: maximum_depth,
            });
        }
        let template = self.catalog.find(value.template()).ok_or(
            DialogueContentMaterializationError::MissingTemplate {
                template: value.template(),
            },
        )?;
        if template.digest() != value.template_digest() {
            return Err(
                DialogueContentMaterializationError::TemplateDigestMismatch {
                    template: value.template(),
                    expected: template.digest(),
                    actual: value.template_digest(),
                },
            );
        }
        if !self.limits.permits_sequence_items(template.slots().len())
            || !self.limits.permits_sequence_items(template.marks().len())
            || !self.limits.permits_sequence_items(template.effects().len())
        {
            return Err(DialogueContentMaterializationError::SequenceLimit {
                maximum: usize::try_from(self.limits.max_sequence_items).unwrap_or(usize::MAX),
            });
        }
        let mut slots = BTreeMap::new();
        for declaration in template.slots() {
            let Some(binding) = value.binding(declaration.slot()) else {
                return Err(DialogueContentMaterializationError::BindingSchemaMismatch {
                    slot: declaration.slot(),
                });
            };
            if binding.role() != declaration.role()
                || binding.semantic_type() != declaration.semantic_type()
            {
                return Err(DialogueContentMaterializationError::BindingSchemaMismatch {
                    slot: declaration.slot(),
                });
            }
            if declaration.role() == RuntimeDialogueValueRole::Content {
                slots.insert(declaration.slot(), None);
            } else {
                let rebased = RuntimeDialogueValueSlotId::from_zero_based(self.bindings.len())
                    .ok_or(DialogueContentMaterializationError::SequenceLimit {
                        maximum: usize::try_from(self.limits.max_sequence_items)
                            .unwrap_or(usize::MAX),
                    })?;
                self.bindings.push(binding.clone().with_slot(rebased));
                slots.insert(declaration.slot(), Some(rebased));
            }
        }
        if value.bindings().len() != template.slots().len() {
            return Err(DialogueContentMaterializationError::BindingSchemaMismatch {
                slot: value
                    .bindings()
                    .get(template.slots().len())
                    .map_or_else(
                        || RuntimeDialogueValueSlotId::from_zero_based(template.slots().len()),
                        |binding| Some(binding.slot()),
                    )
                    .ok_or(DialogueContentMaterializationError::SequenceLimit {
                        maximum: usize::try_from(self.limits.max_sequence_items)
                            .unwrap_or(usize::MAX),
                    })?,
            });
        }

        let mut marks = BTreeMap::new();
        for mark in template.marks() {
            let id = RuntimeDialogueMarkId::from_zero_based(self.marks.len()).ok_or(
                DialogueContentMaterializationError::SequenceLimit {
                    maximum: usize::try_from(self.limits.max_sequence_items).unwrap_or(usize::MAX),
                },
            )?;
            let origin = DialogueContentNodeOrigin {
                template: template.id(),
                occurrence: occurrence.clone().into_boxed_slice(),
                node: 0,
            };
            self.marks.push(MaterializedDialogueMark {
                id,
                diagnostic_name: mark.diagnostic_name().to_owned(),
                origin,
            });
            marks.insert(mark.id(), id);
        }
        let mut effects = BTreeMap::new();
        for effect in template.effects() {
            let id = RuntimeDialogueEffectSiteId::from_zero_based(self.effects.len()).ok_or(
                DialogueContentMaterializationError::SequenceLimit {
                    maximum: usize::try_from(self.limits.max_sequence_items).unwrap_or(usize::MAX),
                },
            )?;
            let origin = DialogueContentNodeOrigin {
                template: template.id(),
                occurrence: occurrence.clone().into_boxed_slice(),
                node: 0,
            };
            self.effects.push(MaterializedDialogueEffect { id, origin });
            effects.insert(effect.id(), id);
        }

        let nodes = self.expand_nodes(
            template.id(),
            value,
            depth,
            &occurrence,
            &slots,
            &marks,
            &effects,
            &template.content().nodes,
        )?;
        Ok(nodes)
    }

    fn expand_nodes(
        &mut self,
        template: RuntimeDialogueContentTemplateId,
        value: &RuntimeDialogueContentValue,
        depth: usize,
        occurrence: &[u32],
        slots: &BTreeMap<RuntimeDialogueValueSlotId, Option<RuntimeDialogueValueSlotId>>,
        marks: &BTreeMap<RuntimeDialogueMarkId, RuntimeDialogueMarkId>,
        effects: &BTreeMap<RuntimeDialogueEffectSiteId, RuntimeDialogueEffectSiteId>,
        nodes: &[RichTextNode],
    ) -> Result<Vec<RichTextNode>, DialogueContentMaterializationError> {
        let mut expanded = Vec::new();
        for (node_index, node) in nodes.iter().enumerate() {
            self.reserve_node()?;
            match node {
                RichTextNode::ContentInsert { slot, on_error } => {
                    let checkpoint = self.checkpoint();
                    let mut child_occurrence = occurrence.to_vec();
                    child_occurrence.push(u32::try_from(node_index).map_err(|_| {
                        DialogueContentMaterializationError::OccurrencePathOverflow
                    })?);
                    let result = self.expand_content_insert(*slot, value, depth, child_occurrence);
                    match result {
                        Ok(nodes) => expanded.extend(nodes),
                        Err(error) => {
                            self.rollback(checkpoint);
                            if let Some(node) = self.apply_failure(*slot, on_error, error)? {
                                self.record_origin(template, occurrence, node_index)?;
                                expanded.push(node);
                            }
                        }
                    }
                }
                RichTextNode::Scope { style, body } => {
                    self.record_origin(template, occurrence, node_index)?;
                    expanded.push(RichTextNode::Scope {
                        style: style.clone(),
                        body: self.expand_nodes(
                            template, value, depth, occurrence, slots, marks, effects, body,
                        )?,
                    });
                }
                RichTextNode::Ruby { body, ruby } => {
                    self.record_origin(template, occurrence, node_index)?;
                    expanded.push(RichTextNode::Ruby {
                        body: self.expand_nodes(
                            template, value, depth, occurrence, slots, marks, effects, body,
                        )?,
                        ruby: ruby.clone(),
                    });
                }
                node => {
                    self.record_origin(template, occurrence, node_index)?;
                    expanded.push(self.rebase_leaf_node(node, slots, marks, effects, node_index)?);
                }
            }
        }
        Ok(expanded)
    }

    fn expand_content_insert(
        &mut self,
        slot: RuntimeDialogueValueSlotId,
        value: &RuntimeDialogueContentValue,
        depth: usize,
        occurrence: Vec<u32>,
    ) -> Result<Vec<RichTextNode>, DialogueContentMaterializationError> {
        let Some(binding) = value.binding(slot) else {
            return Err(DialogueContentMaterializationError::MissingBinding { slot });
        };
        if binding.role() != RuntimeDialogueValueRole::Content {
            return Err(DialogueContentMaterializationError::InvalidBindingRole {
                slot,
                actual: binding.role(),
            });
        }
        let Some(nested) = binding.content() else {
            return Err(DialogueContentMaterializationError::InvalidBindingRole {
                slot,
                actual: binding.role(),
            });
        };
        if nested.artifact() != value.artifact() {
            return Err(DialogueContentMaterializationError::ArtifactMismatch {
                expected: value.artifact(),
                actual: nested.artifact(),
            });
        }
        let child_depth =
            depth
                .checked_add(1)
                .ok_or(DialogueContentMaterializationError::NestingLimit {
                    maximum: usize::try_from(self.limits.max_depth).unwrap_or(usize::MAX),
                })?;
        self.expand(nested, child_depth, occurrence)
    }

    fn apply_failure(
        &mut self,
        slot: RuntimeDialogueValueSlotId,
        selection: &InlineFailureSelection,
        error: DialogueContentMaterializationError,
    ) -> Result<Option<RichTextNode>, DialogueContentMaterializationError> {
        if matches!(
            error,
            DialogueContentMaterializationError::FailedInsertion { .. }
        ) {
            return Err(error);
        }
        match selection.resolve(self.inherited_policy) {
            InlineFailurePolicy::FailLine => {
                return Err(DialogueContentMaterializationError::FailedInsertion {
                    slot,
                    source: Box::new(error),
                });
            }
            InlineFailurePolicy::Discard => Ok(None),
            InlineFailurePolicy::Fallback { fallback } => {
                if let Some(text) = content_fallback_text(slot, &fallback) {
                    return Ok(Some(RichTextNode::Text { text }));
                }
                Ok(None)
            }
        }
    }

    fn rebase_leaf_node(
        &self,
        node: &RichTextNode,
        slots: &BTreeMap<RuntimeDialogueValueSlotId, Option<RuntimeDialogueValueSlotId>>,
        marks: &BTreeMap<RuntimeDialogueMarkId, RuntimeDialogueMarkId>,
        effects: &BTreeMap<RuntimeDialogueEffectSiteId, RuntimeDialogueEffectSiteId>,
        node_index: usize,
    ) -> Result<RichTextNode, DialogueContentMaterializationError> {
        Ok(match node {
            RichTextNode::Interpolation {
                slot,
                label,
                on_error,
            } => RichTextNode::Interpolation {
                slot: self.rebase_slot(*slot, slots, node_index)?,
                label: label.clone(),
                on_error: on_error.clone(),
            },
            RichTextNode::Control { control } => RichTextNode::Control {
                control: match control {
                    RichTextControl::Mark {
                        mark,
                        diagnostic_name,
                    } => RichTextControl::Mark {
                        mark: *marks.get(mark).ok_or(
                            DialogueContentMaterializationError::UnrebasedMark {
                                node: node_index,
                                mark: *mark,
                            },
                        )?,
                        diagnostic_name: diagnostic_name.clone(),
                    },
                    RichTextControl::Effect { site } => RichTextControl::Effect {
                        site: *effects.get(site).ok_or(
                            DialogueContentMaterializationError::UnrebasedEffect {
                                node: node_index,
                                effect: *site,
                            },
                        )?,
                    },
                    control => control.clone(),
                },
            },
            RichTextNode::Raw { text } => RichTextNode::Raw { text: text.clone() },
            RichTextNode::Text { text } => RichTextNode::Text { text: text.clone() },
            RichTextNode::HostEvent { event } => RichTextNode::HostEvent {
                event: event.clone(),
            },
            RichTextNode::ContentInsert { .. }
            | RichTextNode::Scope { .. }
            | RichTextNode::Ruby { .. } => {
                unreachable!("structured nodes are handled by expand_nodes before leaf rebasing")
            }
        })
    }

    fn rebase_slot(
        &self,
        slot: RuntimeDialogueValueSlotId,
        slots: &BTreeMap<RuntimeDialogueValueSlotId, Option<RuntimeDialogueValueSlotId>>,
        node: usize,
    ) -> Result<RuntimeDialogueValueSlotId, DialogueContentMaterializationError> {
        slots
            .get(&slot)
            .copied()
            .flatten()
            .ok_or(DialogueContentMaterializationError::UnrebasedSlot { node, slot })
    }

    fn reserve_node(&mut self) -> Result<(), DialogueContentMaterializationError> {
        self.node_count = self.node_count.checked_add(1).ok_or(
            DialogueContentMaterializationError::NodeLimit {
                maximum: usize::try_from(self.limits.max_nodes).unwrap_or(usize::MAX),
            },
        )?;
        if !self.limits.permits_nodes(self.node_count) {
            return Err(DialogueContentMaterializationError::NodeLimit {
                maximum: usize::try_from(self.limits.max_nodes).unwrap_or(usize::MAX),
            });
        }
        Ok(())
    }

    fn record_origin(
        &mut self,
        template: RuntimeDialogueContentTemplateId,
        occurrence: &[u32],
        node: usize,
    ) -> Result<(), DialogueContentMaterializationError> {
        let origin_count = self.origins.len().checked_add(1).ok_or(
            DialogueContentMaterializationError::SequenceLimit {
                maximum: usize::try_from(self.limits.max_sequence_items).unwrap_or(usize::MAX),
            },
        )?;
        if !self.limits.permits_sequence_items(origin_count) {
            return Err(DialogueContentMaterializationError::SequenceLimit {
                maximum: usize::try_from(self.limits.max_sequence_items).unwrap_or(usize::MAX),
            });
        }
        self.origins.push(DialogueContentNodeOrigin {
            template,
            occurrence: occurrence.to_vec().into_boxed_slice(),
            node,
        });
        Ok(())
    }

    fn checkpoint(&self) -> MaterializationCheckpoint {
        MaterializationCheckpoint {
            origins: self.origins.len(),
            node_count: self.node_count,
            bindings: self.bindings.len(),
            marks: self.marks.len(),
            effects: self.effects.len(),
        }
    }

    fn rollback(&mut self, checkpoint: MaterializationCheckpoint) {
        self.origins.truncate(checkpoint.origins);
        self.node_count = checkpoint.node_count;
        self.bindings.truncate(checkpoint.bindings);
        self.marks.truncate(checkpoint.marks);
        self.effects.truncate(checkpoint.effects);
    }
}

struct MaterializationCheckpoint {
    origins: usize,
    node_count: usize,
    bindings: usize,
    marks: usize,
    effects: usize,
}

fn content_fallback_text(
    slot: RuntimeDialogueValueSlotId,
    fallback: &InlineFallback,
) -> Option<String> {
    match fallback {
        InlineFallback::Text { text, .. } => Some(text.clone()),
        InlineFallback::ExprSource { .. } | InlineFallback::CallSource { .. } => {
            Some(format!("content slot {slot}"))
        }
        InlineFallback::ValuePlain => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::value::{RuntimeDialogueOpaqueRole, RuntimeInlineTextValue};

    fn artifact(marker: u8) -> RuntimeArtifactFingerprint {
        RuntimeArtifactFingerprint::try_from_bytes([marker; 32]).expect("artifact")
    }

    fn template_id(index: usize) -> RuntimeDialogueContentTemplateId {
        RuntimeDialogueContentTemplateId::from_zero_based(index).expect("template id")
    }

    fn interpolation_binding(
        index: usize,
        semantic_type: RuntimeSemanticTypeId,
        text: &str,
    ) -> RuntimeDialogueContentBinding {
        RuntimeDialogueContentBinding::Interpolation {
            slot: RuntimeDialogueValueSlotId::from_zero_based(index).expect("slot"),
            semantic_type,
            value: RuntimeInlineTextValue::try_new(semantic_type, text).expect("inline text"),
        }
    }

    #[test]
    fn materializer_rebases_child_slots_marks_effects_and_origins_per_occurrence() {
        let artifact = artifact(0x31);
        let interpolation_type = RuntimeSemanticTypeId::from_bytes([0xa1; 32]);
        let child_slot = RuntimeDialogueValueSlotId::from_zero_based(0).expect("child slot");
        let child = DialogueContentFragmentTemplate::try_new_canonical(
            template_id(1),
            vec![DialogueContentTemplateSlot::new(
                child_slot,
                RuntimeDialogueValueRole::Interpolation,
                interpolation_type,
            )],
            vec![DialogueContentTemplateMark::new(
                RuntimeDialogueMarkId::from_zero_based(0).expect("mark"),
                "child_mark",
            )],
            vec![DialogueContentTemplateEffect::new(
                RuntimeDialogueEffectSiteId::from_zero_based(0).expect("effect"),
            )],
            RichTextDocument::new(vec![
                RichTextNode::Text {
                    text: "child".to_owned(),
                },
                RichTextNode::Interpolation {
                    slot: child_slot,
                    label: "value".to_owned(),
                    on_error: InlineFailureSelection::Explicit {
                        policy: InlineFailurePolicy::FailLine,
                    },
                },
                RichTextNode::Control {
                    control: RichTextControl::Mark {
                        mark: RuntimeDialogueMarkId::from_zero_based(0).expect("mark"),
                        diagnostic_name: "child_mark".to_owned(),
                    },
                },
                RichTextNode::Control {
                    control: RichTextControl::Effect {
                        site: RuntimeDialogueEffectSiteId::from_zero_based(0).expect("effect"),
                    },
                },
            ]),
        )
        .expect("child template");
        let parent_slot = RuntimeDialogueValueSlotId::from_zero_based(0).expect("parent slot");
        let parent = DialogueContentFragmentTemplate::try_new_canonical(
            template_id(0),
            vec![DialogueContentTemplateSlot::new(
                parent_slot,
                RuntimeDialogueValueRole::Content,
                RuntimeDialogueOpaqueRole::Content.semantic_identity(),
            )],
            Vec::new(),
            Vec::new(),
            RichTextDocument::new(vec![
                RichTextNode::ContentInsert {
                    slot: parent_slot,
                    on_error: InlineFailureSelection::Explicit {
                        policy: InlineFailurePolicy::FailLine,
                    },
                },
                RichTextNode::ContentInsert {
                    slot: parent_slot,
                    on_error: InlineFailureSelection::Explicit {
                        policy: InlineFailurePolicy::FailLine,
                    },
                },
            ]),
        )
        .expect("parent template");
        let child_digest = child.digest();
        let parent_digest = parent.digest();
        let catalog =
            DialogueContentFragmentCatalog::try_from_templates(artifact, vec![parent, child])
                .expect("catalog");
        let child_value = RuntimeDialogueContentValue::try_new(
            artifact,
            template_id(1),
            child_digest,
            [interpolation_binding(0, interpolation_type, "x")],
        )
        .expect("child value");
        let root_value = RuntimeDialogueContentValue::try_new(
            artifact,
            template_id(0),
            parent_digest,
            [RuntimeDialogueContentBinding::Content {
                slot: parent_slot,
                semantic_type: RuntimeDialogueOpaqueRole::Content.semantic_identity(),
                value: child_value,
            }],
        )
        .expect("root value");
        let materialized = DialogueContentMaterializer::new(&catalog)
            .materialize(&root_value)
            .expect("materialized content");
        assert_eq!(materialized.document().nodes.len(), 8);
        assert_eq!(materialized.bindings().len(), 2);
        assert_eq!(materialized.marks().len(), 2);
        assert_eq!(materialized.effects().len(), 2);
        assert_eq!(
            materialized.origins().len(),
            materialized.document().nodes.len()
        );
        assert_eq!(
            materialized.marks()[0].id(),
            RuntimeDialogueMarkId::from_zero_based(0).unwrap()
        );
        assert_eq!(
            materialized.marks()[1].id(),
            RuntimeDialogueMarkId::from_zero_based(1).unwrap()
        );
        assert_eq!(
            materialized.effects()[0].id(),
            RuntimeDialogueEffectSiteId::from_zero_based(0).unwrap()
        );
        assert_eq!(
            materialized.effects()[1].id(),
            RuntimeDialogueEffectSiteId::from_zero_based(1).unwrap()
        );
        assert_eq!(materialized.origins()[0].occurrence(), &[0]);
        assert_eq!(materialized.origins()[4].occurrence(), &[1]);
    }

    #[test]
    fn materializer_applies_content_failure_policy_once() {
        let artifact = artifact(0x61);
        let slot = RuntimeDialogueValueSlotId::from_zero_based(0).expect("slot");
        let parent = DialogueContentFragmentTemplate::try_new_canonical(
            template_id(0),
            vec![DialogueContentTemplateSlot::new(
                slot,
                RuntimeDialogueValueRole::Content,
                RuntimeDialogueOpaqueRole::Content.semantic_identity(),
            )],
            Vec::new(),
            Vec::new(),
            RichTextDocument::new(vec![RichTextNode::ContentInsert {
                slot,
                on_error: InlineFailureSelection::Explicit {
                    policy: InlineFailurePolicy::Fallback {
                        fallback: InlineFallback::Text {
                            text: "fallback".to_owned(),
                            style: arcweft_dialogue::FallbackStylePolicy::Plain,
                        },
                    },
                },
            }]),
        )
        .expect("parent template");
        let parent_digest = parent.digest();
        let catalog = DialogueContentFragmentCatalog::try_from_templates(artifact, vec![parent])
            .expect("catalog");
        let unknown_child = RuntimeDialogueContentValue::try_new(
            artifact,
            template_id(1),
            RuntimeDialogueContentTemplateDigest::from_bytes([2; 32]),
            [],
        )
        .expect("unknown child value");
        let root = RuntimeDialogueContentValue::try_new(
            artifact,
            template_id(0),
            parent_digest,
            [RuntimeDialogueContentBinding::Content {
                slot,
                semantic_type: RuntimeDialogueOpaqueRole::Content.semantic_identity(),
                value: unknown_child,
            }],
        )
        .expect("root value");
        let materialized = DialogueContentMaterializer::new(&catalog)
            .materialize(&root)
            .expect("fallback policy");
        assert_eq!(
            materialized.document().nodes,
            vec![RichTextNode::Text {
                text: "fallback".to_owned()
            }]
        );
    }
}
