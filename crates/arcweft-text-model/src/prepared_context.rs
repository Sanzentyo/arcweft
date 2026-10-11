//! One shared semantic owner and admitted prepared-text context.
//! Shared by renderer preparation and transport projections.

use crate::{
    LineDisplayFrameValidationError, LineDisplayStageProjection, ProjectedTextRangeDomain,
    ProjectedTextRangeError,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// Semantic ownership shared by prepared text and its observation projection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreparedTextOwnerKind {
    DialogueView {
        dialogue: u64,
        entry: u64,
        mount: u64,
        role: DialoguePreparedTextRole,
    },
    View {
        mount: u64,
    },
    Control,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DialoguePreparedTextRole {
    CharacterDisplayName,
    Content,
}

/// Document context always owns the whole prepared string. A clipped stage
/// must use DisplayStage, whose domain retains both source extents.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PreparedTextDocumentContext {
    ranges: ProjectedTextRangeDomain,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PreparedDisplayStageContext {
    ranges: ProjectedTextRangeDomain,
    frame: LineDisplayStageProjection,
}

/// The factory-issued context retained by the actual PreparedTextOwner.
/// Protocol stage rows share this same Arc allocation, not a rebuilt frame.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreparedTextContext {
    Document {
        context: PreparedTextDocumentContext,
    },
    DisplayStage {
        context: Arc<PreparedDisplayStageContext>,
    },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PreparedTextContextError {
    #[error(transparent)]
    Range(#[from] ProjectedTextRangeError),
    #[error(transparent)]
    Display(#[from] LineDisplayFrameValidationError),
    #[error("a document context must own the complete prepared extent")]
    DocumentExtentMismatch,
    #[error("dialogue content requires its actual display-stage context")]
    MissingDisplayStage,
}

impl PreparedTextDocumentContext {
    pub fn try_new(origin: usize, text_length: usize) -> Result<Self, PreparedTextContextError> {
        Ok(Self {
            ranges: ProjectedTextRangeDomain::document(origin, text_length)?,
        })
    }

    #[must_use]
    pub const fn ranges(self) -> ProjectedTextRangeDomain {
        self.ranges
    }

    pub fn validate_text(self, text: &str) -> Result<(), PreparedTextContextError> {
        self.ranges.validate_texts(text, text)?;
        Ok(())
    }
}

impl PreparedDisplayStageContext {
    pub fn try_new(
        frame: LineDisplayStageProjection,
        ranges: ProjectedTextRangeDomain,
        actual_prepared_text: &str,
    ) -> Result<Self, PreparedTextContextError> {
        frame.validate()?;
        ranges.validate_texts(&frame.text, actual_prepared_text)?;
        Ok(Self { ranges, frame })
    }

    #[must_use]
    pub const fn ranges(&self) -> ProjectedTextRangeDomain {
        self.ranges
    }

    #[must_use]
    pub const fn frame(&self) -> &LineDisplayStageProjection {
        &self.frame
    }

    pub fn validate_prepared_text(&self, actual: &str) -> Result<(), PreparedTextContextError> {
        self.ranges.validate_texts(&self.frame.text, actual)?;
        Ok(())
    }

    /// Exact retained display slice; validation has checked its UTF-8 extent.
    #[must_use]
    pub fn prepared_text(&self) -> &str {
        let retained = self
            .ranges
            .source_to_display(self.ranges.prepared_source_range())
            .expect("admitted prepared extent is inside the display extent");
        self.frame
            .text
            .get(retained.start..retained.end)
            .expect("admitted prepared extent preserves UTF-8 boundaries")
    }
}

impl PreparedTextContext {
    pub fn document(origin: usize, text_length: usize) -> Result<Self, PreparedTextContextError> {
        Ok(Self::Document {
            context: PreparedTextDocumentContext::try_new(origin, text_length)?,
        })
    }

    #[must_use]
    pub fn display_stage(context: Arc<PreparedDisplayStageContext>) -> Self {
        Self::DisplayStage { context }
    }

    #[must_use]
    pub fn ranges(&self) -> ProjectedTextRangeDomain {
        match self {
            Self::Document { context } => context.ranges(),
            Self::DisplayStage { context } => context.ranges(),
        }
    }

    #[must_use]
    pub fn stage(&self) -> Option<&Arc<PreparedDisplayStageContext>> {
        match self {
            Self::Document { .. } => None,
            Self::DisplayStage { context } => Some(context),
        }
    }

    /// PreparedFrame admission validates the role and actual item together.
    /// A View may legitimately render either a document or a display stage.
    pub fn validate_owner(
        &self,
        kind: PreparedTextOwnerKind,
        actual_prepared_text: &str,
    ) -> Result<(), PreparedTextContextError> {
        if matches!(
            kind,
            PreparedTextOwnerKind::DialogueView {
                role: DialoguePreparedTextRole::Content,
                ..
            }
        ) && !matches!(self, Self::DisplayStage { .. })
        {
            return Err(PreparedTextContextError::MissingDisplayStage);
        }
        match self {
            Self::Document { context } => context.validate_text(actual_prepared_text),
            Self::DisplayStage { context } => context.validate_prepared_text(actual_prepared_text),
        }
    }
}

impl<'de> Deserialize<'de> for PreparedTextDocumentContext {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            ranges: ProjectedTextRangeDomain,
        }
        let wire = Wire::deserialize(deserializer)?;
        if wire.ranges.display_source_range() != wire.ranges.prepared_source_range() {
            return Err(serde::de::Error::custom(
                PreparedTextContextError::DocumentExtentMismatch,
            ));
        }
        Ok(Self {
            ranges: wire.ranges,
        })
    }
}

impl<'de> Deserialize<'de> for PreparedDisplayStageContext {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            ranges: ProjectedTextRangeDomain,
            frame: LineDisplayStageProjection,
        }
        let wire = Wire::deserialize(deserializer)?;
        wire.frame.validate().map_err(serde::de::Error::custom)?;
        wire.ranges
            .validate_display_range(wire.ranges.display_range(), &wire.frame.text)
            .map_err(serde::de::Error::custom)?;
        let retained = wire
            .ranges
            .source_to_display(wire.ranges.prepared_source_range())
            .map_err(serde::de::Error::custom)?;
        wire.ranges
            .validate_display_range(retained, &wire.frame.text)
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            ranges: wire.ranges,
            frame: wire.frame,
        })
    }
}
