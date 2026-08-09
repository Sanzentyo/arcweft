//! Static dialogue-content catalog produced from accepted typed HIR.

use std::collections::BTreeMap;

use crate::RichTextDocument;
use arcweft_core::plan::RuntimeLineId;
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
    content: RichTextDocument,
    inline_styles: Vec<RichTextStyleContribution>,
    source: ProductSourceRef,
}

impl DialogueContentSpec {
    pub fn new(
        line: RuntimeLineId,
        text_key: TextKey,
        content: RichTextDocument,
        inline_styles: Vec<RichTextStyleContribution>,
        source: ProductSourceRef,
    ) -> Self {
        Self {
            line,
            text_key,
            content,
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

    pub const fn content(&self) -> &RichTextDocument {
        &self.content
    }

    pub fn inline_styles(&self) -> &[RichTextStyleContribution] {
        &self.inline_styles
    }

    pub const fn source(&self) -> &ProductSourceRef {
        &self.source
    }
}

/// Immutable static dialogue catalog with exact keyed lookup.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogueContentCatalog {
    records: Vec<DialogueContentSpec>,
    by_line: BTreeMap<RuntimeLineId, usize>,
}

/// Invalid static dialogue catalog transcript.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialogueContentCatalogError {
    #[error("dialogue content catalog repeats runtime line `{line}`")]
    DuplicateLine { line: RuntimeLineId },
    #[error("dialogue content catalog is not in canonical (line, text_key) order")]
    NonCanonicalOrder,
}

impl DialogueContentCatalog {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
            by_line: BTreeMap::new(),
        }
    }

    /// Accepts only the sole schema's canonical `(line, text_key)` order.
    pub fn try_from_records(
        records: Vec<DialogueContentSpec>,
    ) -> Result<Self, DialogueContentCatalogError> {
        if records
            .windows(2)
            .any(|pair| (pair[0].line(), pair[0].text_key()) > (pair[1].line(), pair[1].text_key()))
        {
            return Err(DialogueContentCatalogError::NonCanonicalOrder);
        }
        let mut by_line = BTreeMap::new();
        for (index, record) in records.iter().enumerate() {
            if by_line.insert(record.line().clone(), index).is_some() {
                return Err(DialogueContentCatalogError::DuplicateLine {
                    line: record.line().clone(),
                });
            }
        }
        Ok(Self { records, by_line })
    }

    pub fn records(&self) -> &[DialogueContentSpec] {
        &self.records
    }

    pub fn find(&self, line: &RuntimeLineId) -> Option<&DialogueContentSpec> {
        self.by_line
            .get(line)
            .and_then(|index| self.records.get(*index))
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
    content: RichTextDocument,
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
            content: self.content.clone(),
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
        Ok(Self::new(
            wire.line,
            TextKey::try_new(wire.text_key).map_err(serde::de::Error::custom)?,
            wire.content,
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
        self.records.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DialogueContentCatalog {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from_records(Vec::<DialogueContentSpec>::deserialize(deserializer)?)
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
