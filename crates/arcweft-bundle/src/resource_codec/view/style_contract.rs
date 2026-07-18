//! Cross-section validation for View program Style references.

use super::model::{ViewProgramResource, ViewStyleApplicationTarget, ViewStyleResource};
use crate::resource_codec::SectionCodecError;
use arcweft_view::style::{ViewStylePatchId, ViewStyleSheetId};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewStyleContractError {
    #[error("View Style resource is invalid: {0}")]
    InvalidResource(#[source] SectionCodecError),
    #[error("View program references Style sheet {sheet:?}, but no View Style resource exists")]
    MissingResourceForSheet { sheet: ViewStyleSheetId },
    #[error(
        "View program references inline Style patch {patch:?}, but no View Style resource exists"
    )]
    MissingResourceForPatch { patch: ViewStylePatchId },
    #[error("View program references unknown Style sheet {sheet:?}")]
    UnknownSheet { sheet: ViewStyleSheetId },
    #[error("View program references unknown inline Style patch {patch:?}")]
    UnknownPatch { patch: ViewStylePatchId },
    #[error(
        "View definition `{view}` carries inline Style patch {patch:?}; definition root scopes may contain named sheets only"
    )]
    InlineDefinitionPatch {
        view: String,
        patch: ViewStylePatchId,
    },
}

impl ViewProgramResource {
    /// Validates every typed Style reference against the complete resource
    /// catalog at the bundle boundary.
    pub fn validate_style_contract(
        &self,
        style: Option<&ViewStyleResource>,
    ) -> Result<(), ViewStyleContractError> {
        if let Some(style) = style {
            style
                .encode_canonical_section()
                .map_err(ViewStyleContractError::InvalidResource)?;
        }
        self.validate_style_references(style)
    }

    pub(super) fn validate_style_references(
        &self,
        style: Option<&ViewStyleResource>,
    ) -> Result<(), ViewStyleContractError> {
        for definition in &self.definitions {
            if let Some(patch) = definition
                .styles
                .iter()
                .find_map(|reference| match reference {
                    ViewStyleApplicationTarget::Inline { patch } => Some(*patch),
                    ViewStyleApplicationTarget::Named { .. } => None,
                })
            {
                return Err(ViewStyleContractError::InlineDefinitionPatch {
                    view: definition.public_id.as_str().to_owned(),
                    patch,
                });
            }
            validate_references(&definition.styles, style)?;
        }
        for instruction in &self.instructions {
            validate_references(instruction.styles(), style)?;
        }
        Ok(())
    }
}

fn validate_references(
    references: &[ViewStyleApplicationTarget],
    style: Option<&ViewStyleResource>,
) -> Result<(), ViewStyleContractError> {
    for reference in references {
        match (reference, style) {
            (ViewStyleApplicationTarget::Named { sheet }, Some(style))
                if style.sheet(sheet).is_none() =>
            {
                return Err(ViewStyleContractError::UnknownSheet {
                    sheet: sheet.clone(),
                });
            }
            (ViewStyleApplicationTarget::Inline { patch }, Some(style))
                if style.inline_patch(*patch).is_none() =>
            {
                return Err(ViewStyleContractError::UnknownPatch { patch: *patch });
            }
            (ViewStyleApplicationTarget::Named { sheet }, None) => {
                return Err(ViewStyleContractError::MissingResourceForSheet {
                    sheet: sheet.clone(),
                });
            }
            (ViewStyleApplicationTarget::Inline { patch }, None) => {
                return Err(ViewStyleContractError::MissingResourceForPatch { patch: *patch });
            }
            (
                ViewStyleApplicationTarget::Named { .. }
                | ViewStyleApplicationTarget::Inline { .. },
                Some(_),
            ) => {}
        }
    }
    Ok(())
}
