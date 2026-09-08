//! Static dialogue-content catalog produced from accepted typed HIR.

use std::collections::BTreeMap;

use crate::{
    DialogueContentFragmentCatalog, DialogueContentFragmentCatalogError,
    DialogueContentFragmentTemplate, DialogueContentFragmentTemplateError,
};
use arcweft_core::effect::RuntimeArtifactFingerprint;
use arcweft_core::entry::RuntimeDialogueContentTemplateDigest;
use arcweft_core::plan::{RuntimeDialogueContentApplicationKey, RuntimeLineId};
use arcweft_core::runtime_id::RuntimeDialogueContentTemplateId;
use arcweft_dialogue::{
    DialoguePresentationProfile, DialogueProfileRevision,
    character_presentation::CheckedCharacterPresentationPlan,
};
use arcweft_id::TextKey;
use arcweft_source::ProductSourceRef;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Static, source-owned dialogue content for one accepted runtime line.
///
/// Dynamic `CharacterDialogue` configuration is intentionally absent. It is
/// supplied by the runtime value when a display frame is created.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogueContentSpec {
    line: RuntimeLineId,
    text_key: TextKey,
    template: RuntimeDialogueContentTemplateId,
    template_digest: RuntimeDialogueContentTemplateDigest,
    character: CheckedCharacterPresentationPlan,
    presentation: DialoguePresentationSnapshot,
    inline_styles: Vec<RichTextStyleContribution>,
    source: ProductSourceRef,
}

/// Exact accepted presentation profile and revision used by one dialogue line.
#[derive(Clone, Debug, PartialEq)]
pub struct DialoguePresentationSnapshot {
    profile: DialoguePresentationProfile,
    revision: DialogueProfileRevision,
}

impl DialoguePresentationSnapshot {
    pub const fn new(
        profile: DialoguePresentationProfile,
        revision: DialogueProfileRevision,
    ) -> Self {
        Self { profile, revision }
    }

    pub const fn profile(&self) -> &DialoguePresentationProfile {
        &self.profile
    }

    pub const fn revision(&self) -> &DialogueProfileRevision {
        &self.revision
    }
}

impl DialogueContentSpec {
    /// Creates a line record around the immutable template authority.
    pub fn try_new(
        line: RuntimeLineId,
        text_key: TextKey,
        template: &DialogueContentFragmentTemplate,
        character: CheckedCharacterPresentationPlan,
        presentation: DialoguePresentationSnapshot,
        inline_styles: Vec<RichTextStyleContribution>,
        source: ProductSourceRef,
    ) -> Result<Self, DialogueContentSpecError> {
        template.validate_for_catalog().map_err(|source| {
            DialogueContentSpecError::InvalidTemplate {
                template: template.id(),
                source: Box::new(source),
            }
        })?;
        Ok(Self {
            line,
            text_key,
            template: template.id(),
            template_digest: template.digest(),
            character,
            presentation,
            inline_styles,
            source,
        })
    }

    fn from_identity(
        line: RuntimeLineId,
        text_key: TextKey,
        template: RuntimeDialogueContentTemplateId,
        template_digest: RuntimeDialogueContentTemplateDigest,
        character: CheckedCharacterPresentationPlan,
        presentation: DialoguePresentationSnapshot,
        inline_styles: Vec<RichTextStyleContribution>,
        source: ProductSourceRef,
    ) -> Self {
        Self {
            line,
            text_key,
            template,
            template_digest,
            character,
            presentation,
            inline_styles,
            source,
        }
    }

    pub const fn line(&self) -> &RuntimeLineId {
        &self.line
    }

    pub const fn text_key(&self) -> &TextKey {
        &self.text_key
    }

    #[must_use]
    pub const fn template_id(&self) -> RuntimeDialogueContentTemplateId {
        self.template
    }

    #[must_use]
    pub const fn template_digest(&self) -> RuntimeDialogueContentTemplateDigest {
        self.template_digest
    }

    pub const fn character(&self) -> &CheckedCharacterPresentationPlan {
        &self.character
    }

    pub const fn presentation(&self) -> &DialoguePresentationProfile {
        self.presentation.profile()
    }

    pub const fn presentation_revision(&self) -> &DialogueProfileRevision {
        self.presentation.revision()
    }

    pub fn inline_styles(&self) -> &[RichTextStyleContribution] {
        &self.inline_styles
    }

    pub const fn source(&self) -> &ProductSourceRef {
        &self.source
    }

    #[must_use]
    pub fn key(&self) -> RuntimeDialogueContentApplicationKey {
        RuntimeDialogueContentApplicationKey::new(self.line.clone(), self.template)
    }
}

/// Invalid line record assembled around a checked content template.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialogueContentSpecError {
    #[error("dialogue content template `{template}` is invalid: {source}")]
    InvalidTemplate {
        template: RuntimeDialogueContentTemplateId,
        #[source]
        source: Box<DialogueContentFragmentTemplateError>,
    },
}

/// Immutable static dialogue catalog with exact keyed lookup.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogueContentCatalog {
    records: Vec<DialogueContentSpec>,
    by_application: BTreeMap<RuntimeDialogueContentApplicationKey, usize>,
    templates: Vec<DialogueContentFragmentTemplate>,
    by_template: BTreeMap<RuntimeDialogueContentTemplateId, usize>,
}

/// Invalid static dialogue catalog transcript.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialogueContentCatalogError {
    #[error("dialogue content catalog repeats application `{key:?}`")]
    DuplicateApplication {
        key: RuntimeDialogueContentApplicationKey,
    },
    #[error("dialogue content catalog is not in canonical (line, template, text_key) order")]
    NonCanonicalOrder,
    #[error("dialogue content catalog repeats template `{template}`")]
    DuplicateTemplate {
        template: RuntimeDialogueContentTemplateId,
    },
    #[error("dialogue content catalog templates are not in canonical identity order")]
    NonCanonicalTemplateOrder,
    #[error("dialogue content catalog application `{key:?}` references a missing template")]
    MissingTemplate {
        key: RuntimeDialogueContentApplicationKey,
    },
    #[error("dialogue content template `{template}` is invalid: {source}")]
    InvalidTemplate {
        template: RuntimeDialogueContentTemplateId,
        #[source]
        source: Box<DialogueContentFragmentTemplateError>,
    },
}

impl DialogueContentCatalog {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
            by_application: BTreeMap::new(),
            templates: Vec::new(),
            by_template: BTreeMap::new(),
        }
    }

    /// Builds the complete catalog from top-level line records and every
    /// nested immutable template emitted by the compiler.
    pub fn try_from_records_and_templates(
        records: Vec<DialogueContentSpec>,
        templates: Vec<DialogueContentFragmentTemplate>,
    ) -> Result<Self, DialogueContentCatalogError> {
        if records
            .windows(2)
            .any(|pair| (pair[0].key(), pair[0].text_key()) > (pair[1].key(), pair[1].text_key()))
        {
            return Err(DialogueContentCatalogError::NonCanonicalOrder);
        }
        let mut by_application = BTreeMap::new();
        for (index, record) in records.iter().enumerate() {
            let key = record.key();
            if by_application.insert(key.clone(), index).is_some() {
                return Err(DialogueContentCatalogError::DuplicateApplication { key });
            }
        }
        for template in &templates {
            template.validate_for_catalog().map_err(|source| {
                DialogueContentCatalogError::InvalidTemplate {
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
                return Err(DialogueContentCatalogError::DuplicateTemplate {
                    template: pair[0].id(),
                });
            }
            return Err(DialogueContentCatalogError::NonCanonicalTemplateOrder);
        }
        let mut by_template = BTreeMap::new();
        for (index, template) in templates.iter().enumerate() {
            if by_template.insert(template.id(), index).is_some() {
                return Err(DialogueContentCatalogError::DuplicateTemplate {
                    template: template.id(),
                });
            }
        }
        for record in &records {
            let Some(template) = by_template
                .get(&record.template_id())
                .and_then(|index| templates.get(*index))
            else {
                return Err(DialogueContentCatalogError::MissingTemplate { key: record.key() });
            };
            if template.id() != record.template_id()
                || template.digest() != record.template_digest()
            {
                return Err(DialogueContentCatalogError::InvalidTemplate {
                    template: record.template_id(),
                    source: Box::new(DialogueContentFragmentTemplateError::DigestMismatch {
                        expected: template.digest(),
                        actual: record.template_digest(),
                    }),
                });
            }
        }
        Ok(Self {
            records,
            by_application,
            templates,
            by_template,
        })
    }

    pub fn records(&self) -> &[DialogueContentSpec] {
        &self.records
    }

    pub fn find(&self, key: &RuntimeDialogueContentApplicationKey) -> Option<&DialogueContentSpec> {
        self.by_application
            .get(key)
            .and_then(|index| self.records.get(*index))
    }

    #[must_use]
    pub fn templates(&self) -> &[DialogueContentFragmentTemplate] {
        &self.templates
    }

    #[must_use]
    pub fn find_template(
        &self,
        template: RuntimeDialogueContentTemplateId,
    ) -> Option<&DialogueContentFragmentTemplate> {
        self.by_template
            .get(&template)
            .and_then(|index| self.templates.get(*index))
    }

    pub fn fragment_catalog(
        &self,
        artifact: RuntimeArtifactFingerprint,
    ) -> Result<DialogueContentFragmentCatalog, DialogueContentFragmentCatalogError> {
        DialogueContentFragmentCatalog::try_from_templates(artifact, self.templates.clone())
    }
}

impl Default for DialogueContentCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DialogueContentSpecWire {
    line: RuntimeLineId,
    text_key: String,
    template: RuntimeDialogueContentTemplateId,
    template_digest: RuntimeDialogueContentTemplateDigest,
    character: CheckedCharacterPresentationPlan,
    presentation: DialoguePresentationProfile,
    presentation_revision: DialogueProfileRevision,
    inline_styles: Vec<RichTextStyleContribution>,
    source: ProductSourceRef,
}

impl Serialize for DialogueContentSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DialogueContentSpecWire {
            line: self.line.clone(),
            text_key: self.text_key.as_str().to_owned(),
            template: self.template,
            template_digest: self.template_digest,
            character: self.character.clone(),
            presentation: self.presentation.profile().clone(),
            presentation_revision: self.presentation.revision().clone(),
            inline_styles: self.inline_styles.clone(),
            source: self.source.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DialogueContentSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DialogueContentSpecWire::deserialize(deserializer)?;
        Ok(Self::from_identity(
            wire.line,
            TextKey::try_new(wire.text_key).map_err(serde::de::Error::custom)?,
            wire.template,
            wire.template_digest,
            wire.character,
            DialoguePresentationSnapshot::new(wire.presentation, wire.presentation_revision),
            wire.inline_styles,
            wire.source,
        ))
    }
}

impl Serialize for DialogueContentCatalog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        #[serde(deny_unknown_fields)]
        struct Wire<'a> {
            records: &'a [DialogueContentSpec],
            templates: &'a [DialogueContentFragmentTemplate],
        }
        Wire {
            records: &self.records,
            templates: &self.templates,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DialogueContentCatalog {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            records: Vec<DialogueContentSpec>,
            templates: Vec<DialogueContentFragmentTemplate>,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::try_from_records_and_templates(wire.records, wire.templates)
            .map_err(serde::de::Error::custom)
    }
}

/// Provenance for one inline style contribution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextStyleContribution {
    pub path: String,
    pub layer: RichTextCascadeLayer,
    pub source: RichTextSettingSource,
    pub op: RichTextAssignOp,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_index: Option<usize>,
    #[serde(default)]
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowed_by: Option<usize>,
}

/// Source-owned style layers retained in static dialogue content.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextCascadeLayer {
    InlineSpan,
    DialogueViewStyle,
    EngineDefaults,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RichTextSettingSource {
    SourceFile {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        public_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        range: Option<RichTextSourceRange>,
    },
    EngineDefault {
        key: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextAssignOp {
    Replace,
    Append,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextSourceRange {
    pub start: usize,
    pub end: usize,
}
