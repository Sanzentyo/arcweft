//! Compiler-owned executable Fx definition catalog.
//!
//! Final sema owns definition identity, parameter schema, defaults, and body
//! roots. This phase materializes each referenced graph exactly once before
//! View or runtime Content lowering; downstream consumers only bind checked
//! applications against this immutable catalog.

use arcweft_bundle::fx_definitions::{FxDefinitions, FxDefinitionsError};
use arcweft_lang_sema::final_analysis::{CheckedFxDefinition, FinalSemanticAnalysis};
use arcweft_presentation::fx::{FxDefinition, FxId, build_builtin_fx_definition};
use thiserror::Error;

#[derive(Clone, Debug)]
pub(crate) struct CompiledFxCatalog {
    definitions: FxDefinitions,
}

#[derive(Debug, Error)]
pub(crate) enum CompiledFxCatalogError {
    #[error("checked builtin Fx definition `{definition}` could not be materialized: {reason}")]
    Builtin { definition: FxId, reason: String },
    #[error("checked project Fx definition `{definition}` could not be lowered: {reason}")]
    Project { definition: FxId, reason: String },
    #[error(transparent)]
    Inventory(#[from] FxDefinitionsError),
}

impl CompiledFxCatalog {
    pub(crate) fn lower(analysis: &FinalSemanticAnalysis) -> Result<Self, CompiledFxCatalogError> {
        let definitions = analysis
            .checked_fx_definitions()
            .definitions()
            .map(|(_, checked)| match checked {
                CheckedFxDefinition::Builtin {
                    specialization,
                    definition,
                    ..
                } => build_builtin_fx_definition(*specialization)
                    .map(|template| template.definition().clone())
                    .map_err(|error| CompiledFxCatalogError::Builtin {
                        definition: definition.clone(),
                        reason: error.to_string(),
                    }),
                CheckedFxDefinition::Project(project) => {
                    crate::lower::fx::lower_checked_definition(
                        project,
                        analysis.checked_fx_definitions(),
                    )
                    .map_err(|reason| CompiledFxCatalogError::Project {
                        definition: project.definition().clone(),
                        reason,
                    })
                }
            })
            .collect::<Result<Vec<FxDefinition>, _>>()?;
        Ok(Self {
            definitions: FxDefinitions::try_new(definitions)?,
        })
    }

    pub(crate) fn get(&self, id: &FxId) -> Option<&FxDefinition> {
        self.definitions.get(id)
    }

    pub(crate) const fn definitions(&self) -> &FxDefinitions {
        &self.definitions
    }

    pub(crate) fn into_definitions(self) -> FxDefinitions {
        self.definitions
    }
}
