//! Runtime-plan dialogue display catalog.

use crate::{RichTextDocument, RichTextStyle};
use arcweft_core::plan::RuntimeLineId;
use arcweft_dialogue::{DialogueProfileRevision, InlineFailurePolicy};
use arcweft_view::{ViewId, ViewStyleSheetId};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// Rich-text display sidecar generated while lowering a runtime plan.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineDisplayCatalog {
    dialogue_revision: DialogueProfileRevision,
    lines: Vec<LineDisplaySpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LineDisplayCatalogWire {
    dialogue_revision: DialogueProfileRevision,
    lines: Vec<LineDisplaySpec>,
}

/// Invalid pairing between a display catalog and one of its line records.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum LineDisplayCatalogError {
    #[error("line `{line:?}` belongs to a different dialogue profile revision")]
    RevisionMismatch { line: RuntimeLineId },
}

/// One dialogue line's renderable text and host-observable tag events.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineDisplaySpec {
    pub line: RuntimeLineId,
    pub callee: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_label: Option<String>,
    pub text_key: Option<String>,
    /// Stable public owner of the authored View used for this dialogue line.
    pub view: ViewId,
    /// Launch-profile native Style applied at the final mounted View root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_style: Option<ViewStyleSheetId>,
    /// Exact accepted profile/product generation that admitted this line.
    pub dialogue_revision: DialogueProfileRevision,
    pub voice: Option<String>,
    pub look: Option<String>,
    pub style: Option<String>,
    #[serde(default)]
    pub base_styles: Vec<RichTextStyle>,
    pub inline_failure: InlineFailurePolicy,
    #[serde(default)]
    pub style_contributions: Vec<RichTextStyleContribution>,
    pub args: Vec<LineDisplayArg>,
    pub content: RichTextDocument,
}

/// Provenance for one style contribution in the effective dialogue cascade.
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

/// Effective style cascade layer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextCascadeLayer {
    InlineSpan,
    LineOptions,
    SpeakerPreset,
    CharacterDialogueStyle,
    DialogueViewStyle,
    EngineDefaults,
}

/// Source of a style/default contribution.
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

/// Assignment operator used by a style/default contribution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RichTextAssignOp {
    Replace,
    Append,
}

/// Half-open byte range in source text.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RichTextSourceRange {
    pub start: usize,
    pub end: usize,
}

/// Non-reserved line argument preserved for player adapters.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineDisplayArg {
    pub name: String,
    pub value: String,
}

impl LineDisplayCatalog {
    /// Creates an empty catalog bound to one compiler-admitted profile.
    #[must_use]
    pub const fn new(dialogue_revision: DialogueProfileRevision) -> Self {
        Self {
            dialogue_revision,
            lines: Vec::new(),
        }
    }

    /// Creates a revision-checked catalog from display specs in runtime order.
    pub fn try_from_lines(
        dialogue_revision: DialogueProfileRevision,
        lines: Vec<LineDisplaySpec>,
    ) -> Result<Self, LineDisplayCatalogError> {
        let mut catalog = Self::new(dialogue_revision);
        for spec in lines {
            catalog.push(spec)?;
        }
        Ok(catalog)
    }

    /// Appends one display spec.
    pub fn push(&mut self, spec: LineDisplaySpec) -> Result<(), LineDisplayCatalogError> {
        if spec.dialogue_revision != self.dialogue_revision {
            return Err(LineDisplayCatalogError::RevisionMismatch {
                line: spec.line.clone(),
            });
        }
        self.lines.push(spec);
        Ok(())
    }

    /// Exact compiler-admitted profile generation for this entire catalog.
    #[must_use]
    pub const fn dialogue_revision(&self) -> &DialogueProfileRevision {
        &self.dialogue_revision
    }

    /// Display specs in runtime order.
    #[must_use]
    pub fn lines(&self) -> &[LineDisplaySpec] {
        &self.lines
    }

    /// Finds a line display spec by runtime line id.
    #[must_use]
    pub fn find(&self, line: &RuntimeLineId) -> Option<&LineDisplaySpec> {
        self.lines.iter().find(|spec| &spec.line == line)
    }
}

impl<'de> Deserialize<'de> for LineDisplayCatalog {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = LineDisplayCatalogWire::deserialize(deserializer)?;
        Self::try_from_lines(wire.dialogue_revision, wire.lines).map_err(serde::de::Error::custom)
    }
}
