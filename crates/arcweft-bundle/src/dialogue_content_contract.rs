//! Cross-section admission for the AWBC Content ABI and immutable text body.

use arcweft_core::value::RuntimeDialoguePlainTextContextTemplateProof;
use arcweft_text_model::DialogueContentFragmentTemplate;

use crate::{ArcweftBundle, BundleCodecError};

impl ArcweftBundle {
    /// Validates the joined AWBC slot ABI and immutable dialogue template body.
    ///
    /// This is required for decoded bundles and directly constructed runtime
    /// sessions: the Core AWBC verifier alone cannot inspect the text catalog.
    ///
    /// # Errors
    ///
    /// Returns a contract error if the AWBC pointer, verified row, or
    /// canonical text-model template disagree.
    pub fn validate_dialogue_content_contract(
        &self,
    ) -> Result<Option<RuntimeDialoguePlainTextContextTemplateProof>, BundleCodecError> {
        let program = self.product_awbc.program();
        let Some(reference) = program
            .validated_plain_text_context_template()
            .map_err(|error| BundleCodecError::InvalidDialogueContentContract {
                message: error.to_string(),
            })?
        else {
            return Ok(None);
        };
        self.product_awbc.verify_product_executable()?;
        let actual = self
            .dialogue_content
            .find_template(reference.id())
            .ok_or_else(|| BundleCodecError::InvalidDialogueContentContract {
                message: format!(
                    "plain-text context template {} is absent from the dialogue catalog",
                    reference.id()
                ),
            })?;
        let expected = DialogueContentFragmentTemplate::plain_text_context(reference.id())
            .map_err(|error| BundleCodecError::InvalidDialogueContentContract {
                message: error.to_string(),
            })?;
        if actual != &expected || actual.digest() != reference.digest() {
            return Err(BundleCodecError::InvalidDialogueContentContract {
                message: format!(
                    "plain-text context template {} does not match its canonical body and AWBC digest",
                    reference.id()
                ),
            });
        }
        RuntimeDialoguePlainTextContextTemplateProof::try_from_validated_ref(
            reference,
            expected.digest(),
        )
        .map(Some)
        .map_err(|error| BundleCodecError::InvalidDialogueContentContract {
            message: error.to_string(),
        })
    }
}
