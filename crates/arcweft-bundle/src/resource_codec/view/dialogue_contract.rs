//! Cross-record validation for typed dialogue View projections.

use super::model::{
    DialogueTextProjection, ViewActionButtonActionResource, ViewParameterRole, ViewProgramResource,
    ViewTextResource, ViewTextSourceKind, ViewTextSurface,
};
use thiserror::Error;

/// Cross-record contract failure for typed dialogue View projections.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialogueViewContractError {
    #[error("dialogue text source `{text_source}` has no View program")]
    MissingProgram { text_source: String },
    #[error("dialogue text source `{text_source}` is not owned by a View text block")]
    MissingTextOwner { text_source: String },
    #[error("dialogue projection owner `{view}` for `{text_source}` has no View definition")]
    MissingViewDefinition { text_source: String, view: String },
    #[error("dialogue projection `{text_source}` has no owning View")]
    MissingOwningView { text_source: String },
    #[error(
        "View `{view}` projection `{text_source}` references parameter `{parameter}` without the dialogue role"
    )]
    InvalidTextParameterRole {
        text_source: String,
        view: String,
        parameter: String,
    },
    #[error(
        "View `{view}` projection `{text_source}` requires {expected:?}, but its text block uses {actual:?}"
    )]
    TextSurfaceMismatch {
        text_source: String,
        view: String,
        expected: ViewTextSurface,
        actual: ViewTextSurface,
    },
    #[error("dialogue primary action `{button}` has no owning View")]
    MissingActionOwner { button: String },
    #[error("dialogue primary action `{button}` owner `{view}` has no View definition")]
    MissingActionViewDefinition { button: String, view: String },
    #[error(
        "View `{view}` primary action `{button}` references parameter `{parameter}` without the dialogue role"
    )]
    InvalidActionParameterRole {
        button: String,
        view: String,
        parameter: String,
    },
}

impl ViewProgramResource {
    /// Validates typed dialogue text and action projections across View program
    /// and text resources.
    pub fn validate_dialogue_contract(
        &self,
        text: Option<&ViewTextResource>,
    ) -> Result<(), DialogueViewContractError> {
        self.validate_dialogue_actions()?;
        let Some(text) = text else {
            return Ok(());
        };
        for source in &text.sources {
            let ViewTextSourceKind::Dialogue {
                parameter,
                projection,
            } = &source.kind
            else {
                continue;
            };
            let blocks = self
                .text_blocks
                .iter()
                .filter(|block| block.text_source == source.public_id)
                .collect::<Vec<_>>();
            if blocks.is_empty() {
                return Err(DialogueViewContractError::MissingTextOwner {
                    text_source: source.public_id.clone(),
                });
            }
            let expected_surface = match projection {
                DialogueTextProjection::Speaker => ViewTextSurface::Text,
                DialogueTextProjection::Content => ViewTextSurface::RichText,
            };
            for block in blocks {
                let view = block.view.as_deref().ok_or_else(|| {
                    DialogueViewContractError::MissingOwningView {
                        text_source: source.public_id.clone(),
                    }
                })?;
                let definition = self
                    .definitions
                    .iter()
                    .find(|definition| definition.public_id == view)
                    .ok_or_else(|| DialogueViewContractError::MissingViewDefinition {
                        text_source: source.public_id.clone(),
                        view: view.to_owned(),
                    })?;
                let has_dialogue_role = definition.parameters.iter().any(|candidate| {
                    candidate.name == *parameter && candidate.role == ViewParameterRole::Dialogue
                });
                if !has_dialogue_role {
                    return Err(DialogueViewContractError::InvalidTextParameterRole {
                        text_source: source.public_id.clone(),
                        view: view.to_owned(),
                        parameter: parameter.clone(),
                    });
                }
                if block.surface != expected_surface {
                    return Err(DialogueViewContractError::TextSurfaceMismatch {
                        text_source: source.public_id.clone(),
                        view: view.to_owned(),
                        expected: expected_surface,
                        actual: block.surface,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_dialogue_actions(&self) -> Result<(), DialogueViewContractError> {
        for button in &self.action_buttons {
            let ViewActionButtonActionResource::DialoguePrimaryAction { parameter } =
                &button.action
            else {
                continue;
            };
            let view = button.view.as_deref().ok_or_else(|| {
                DialogueViewContractError::MissingActionOwner {
                    button: button.public_id.clone(),
                }
            })?;
            let definition = self
                .definitions
                .iter()
                .find(|definition| definition.public_id == view)
                .ok_or_else(|| DialogueViewContractError::MissingActionViewDefinition {
                    button: button.public_id.clone(),
                    view: view.to_owned(),
                })?;
            let has_dialogue_role = definition.parameters.iter().any(|candidate| {
                candidate.name == *parameter && candidate.role == ViewParameterRole::Dialogue
            });
            if !has_dialogue_role {
                return Err(DialogueViewContractError::InvalidActionParameterRole {
                    button: button.public_id.clone(),
                    view: view.to_owned(),
                    parameter: parameter.clone(),
                });
            }
        }
        Ok(())
    }
}
