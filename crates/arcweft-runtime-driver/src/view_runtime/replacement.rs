//! Prepared atomic replacement of one accepted View program generation.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use arcweft_bundle::resource_codec::SourceSetRevision;
use arcweft_bundle::resource_codec::view::{
    ValidatedViewProduct, ValidatedViewStyleResource, ViewStyleResource,
};
use arcweft_view::{
    AcceptedViewProgramRevision, ViewId, ViewMountId, ViewProgramId, ViewRegistry,
    ViewRegistryError, ViewSchemaId, ViewValueProgramInventory,
};
use thiserror::Error;

use super::axis_seed::BundleViewAxisSeedRegistry;
use super::catalog::{ViewProgramCatalog, ViewProgramCatalogError};
use super::owner::AcceptedViewProgramGeneration;
use super::{BundleViewRuntime, MountedView, ViewOccurrenceKey};

mod reconcile;
pub use reconcile::ViewMountReconcileError;
use reconcile::reconcile_mounts;

/// Exact invalidation publication for one accepted semantic replacement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewProgramInvalidation {
    generation: AcceptedViewProgramGeneration,
    frame_revision: u64,
    owners: BTreeSet<ViewId>,
    export_owners: BTreeSet<ViewId>,
    direct_callers: BTreeSet<ViewId>,
    retired_mounts: BTreeSet<ViewMountId>,
}

/// Candidate state prepared without mutating the live View runtime.
pub struct PreparedViewProgramReplacement {
    expected: ExpectedViewRuntimeState,
    candidate: PreparedViewPublication,
    outcome: ViewProgramReplacementOutcome,
}

#[derive(Clone)]
struct ExpectedViewRuntimeState {
    program: ViewProgramId,
    revision: AcceptedViewProgramRevision,
    source_revision: SourceSetRevision,
    style: Option<ViewStyleResource>,
    generation: AcceptedViewProgramGeneration,
    frame_revision: u64,
    logical_time: arcweft_presentation::fx::FxLogicalTime,
    allocator: arcweft_view::ViewMountAllocator,
    root_bindings: BTreeMap<String, arcweft_core::value::RuntimeValue>,
    mounts: BTreeMap<ViewOccurrenceKey, MountedView>,
    axis_seeds: BundleViewAxisSeedRegistry,
    required_dialogue_views: BTreeSet<ViewId>,
}

enum PreparedViewPublication {
    Unchanged,
    SourceOnly(Box<PreparedSourceOnlyPublication>),
    Semantic(Box<PreparedSemanticPublication>),
}

struct PreparedSourceOnlyPublication {
    product: ValidatedViewProduct,
    catalog: ViewProgramCatalog,
}

struct PreparedSemanticPublication {
    product: ValidatedViewProduct,
    catalog: ViewProgramCatalog,
    registry: ViewRegistry,
    inventory: ViewValueProgramInventory,
    generation: AcceptedViewProgramGeneration,
    frame_revision: u64,
    mounts: BTreeMap<ViewOccurrenceKey, MountedView>,
    axis_seeds: BundleViewAxisSeedRegistry,
    invalidation: ViewProgramInvalidation,
}

/// Observable result of committing one prepared replacement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewProgramReplacementOutcome {
    Unchanged,
    SourceOnly,
    Semantic {
        previous: AcceptedViewProgramRevision,
        accepted: AcceptedViewProgramRevision,
        generation: AcceptedViewProgramGeneration,
    },
}

/// Failure to prepare or atomically publish a View-program replacement.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewProgramReplacementError {
    #[error("replacement program identity does not match the accepted program")]
    ProgramIdentityMismatch,
    #[error("replacement changes the accepted native Style program")]
    StyleProgramChanged,
    #[error("prepared replacement is stale")]
    StalePreparedState,
    #[error("accepted View-program generation is exhausted")]
    GenerationExhausted,
    #[error("frame revision is exhausted")]
    FrameRevisionExhausted,
    #[error(transparent)]
    Catalog(#[from] ViewProgramCatalogError),
    #[error(transparent)]
    Registry(#[from] ViewRegistryError),
    #[error(transparent)]
    Reconcile(#[from] ViewMountReconcileError),
    #[error("replacement removes required dialogue View definition `{definition}`")]
    MissingRequiredDialogueView { definition: ViewId },
    #[error("replacement View `{definition}` no longer owns a typed dialogue input parameter")]
    RequiredDialogueViewMissingRole { definition: ViewId },
}

impl AcceptedViewProgramGeneration {
    pub fn checked_next(self) -> Result<Self, ViewProgramReplacementError> {
        self.get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .map(Self)
            .ok_or(ViewProgramReplacementError::GenerationExhausted)
    }
}

impl ViewProgramInvalidation {
    pub const fn generation(&self) -> AcceptedViewProgramGeneration {
        self.generation
    }

    pub const fn frame_revision(&self) -> u64 {
        self.frame_revision
    }

    pub const fn owners(&self) -> &BTreeSet<ViewId> {
        &self.owners
    }

    pub const fn export_owners(&self) -> &BTreeSet<ViewId> {
        &self.export_owners
    }

    pub const fn direct_callers(&self) -> &BTreeSet<ViewId> {
        &self.direct_callers
    }

    pub const fn retired_mounts(&self) -> &BTreeSet<ViewMountId> {
        &self.retired_mounts
    }
}

impl BundleViewRuntime {
    /// Builds a complete candidate publication without mutating live state.
    pub fn prepare_view_program_replacement(
        &self,
        candidate: ValidatedViewProduct,
    ) -> Result<PreparedViewProgramReplacement, ViewProgramReplacementError> {
        if !same_optional_style_semantics(self.product.style(), candidate.style()) {
            return Err(ViewProgramReplacementError::StyleProgramChanged);
        }
        let current = self
            .catalog
            .as_ref()
            .ok_or(ViewProgramReplacementError::ProgramIdentityMismatch)?;
        let candidate_catalog = ViewProgramCatalog::try_from_validated(&candidate)?
            .ok_or(ViewProgramReplacementError::ProgramIdentityMismatch)?;
        if current.program_id() != candidate_catalog.program_id() {
            return Err(ViewProgramReplacementError::ProgramIdentityMismatch);
        }
        self.validate_replacement_dialogue_views(&candidate_catalog)?;
        candidate
            .program()
            .ok_or(ViewProgramReplacementError::ProgramIdentityMismatch)?
            .resource()
            .validate_dialogue_contract(self.text.as_ref())
            .map_err(ViewMountReconcileError::from)?;
        let expected = self.expected_replacement_state(current);
        if current.revision() == candidate_catalog.revision() {
            return Ok(self.prepare_same_revision_publication(
                current,
                candidate,
                candidate_catalog,
                expected,
            ));
        }

        let generation = self.generation.checked_next()?;
        let frame_revision = checked_next_frame_revision(self.frame_revision)?;
        let mut registry = self.registry.clone();
        for view in current.view_ids() {
            registry.retire_arcweft(view, current.program_id(), current.revision())?;
        }
        for (view, definition) in candidate_catalog.definitions() {
            registry.register_arcweft(
                view.clone(),
                ViewSchemaId(definition.state_schema_hash()),
                candidate_catalog.program_id().clone(),
                candidate_catalog.revision(),
            )?;
        }
        let inventory = ViewValueProgramInventory::from_programs(
            candidate_catalog.resource().value_programs.clone(),
        )
        .map_err(ViewMountReconcileError::from)?;
        let (mounts, retired_mounts) = reconcile_mounts(
            &self.mounts,
            &candidate_catalog,
            &registry,
            generation,
            &inventory,
        )?;
        let live_mounts = mounts
            .values()
            .map(|mounted| mounted.state.mount())
            .collect::<BTreeSet<_>>();
        let mut axis_seeds = self.axis_seeds.clone();
        axis_seeds.retain_mounts(&live_mounts);
        let diff = current.semantic_diff(&candidate_catalog);
        let invalidation = ViewProgramInvalidation {
            generation,
            frame_revision,
            owners: diff.owners,
            export_owners: diff.export_owners,
            direct_callers: diff.direct_callers,
            retired_mounts,
        };
        let outcome = ViewProgramReplacementOutcome::Semantic {
            previous: current.revision(),
            accepted: candidate_catalog.revision(),
            generation,
        };
        Ok(PreparedViewProgramReplacement {
            expected,
            candidate: PreparedViewPublication::Semantic(Box::new(PreparedSemanticPublication {
                product: candidate,
                catalog: candidate_catalog,
                registry,
                inventory,
                generation,
                frame_revision,
                mounts,
                axis_seeds,
                invalidation,
            })),
            outcome,
        })
    }

    fn prepare_same_revision_publication(
        &self,
        current: &ViewProgramCatalog,
        candidate: ValidatedViewProduct,
        candidate_catalog: ViewProgramCatalog,
        expected: ExpectedViewRuntimeState,
    ) -> PreparedViewProgramReplacement {
        let source_or_provenance_changed = current.source_revision()
            != candidate_catalog.source_revision()
            || self
                .product
                .style()
                .map(ValidatedViewStyleResource::resource)
                != candidate.style().map(ValidatedViewStyleResource::resource);
        let (candidate, outcome) = if source_or_provenance_changed {
            (
                PreparedViewPublication::SourceOnly(Box::new(PreparedSourceOnlyPublication {
                    product: candidate,
                    catalog: candidate_catalog,
                })),
                ViewProgramReplacementOutcome::SourceOnly,
            )
        } else {
            (
                PreparedViewPublication::Unchanged,
                ViewProgramReplacementOutcome::Unchanged,
            )
        };
        PreparedViewProgramReplacement {
            expected,
            candidate,
            outcome,
        }
    }

    /// Publishes a previously prepared candidate after an exact stale check.
    pub fn commit_view_program_replacement(
        &mut self,
        prepared: PreparedViewProgramReplacement,
    ) -> Result<ViewProgramReplacementOutcome, ViewProgramReplacementError> {
        if !self.matches_expected_replacement_state(&prepared.expected) {
            return Err(ViewProgramReplacementError::StalePreparedState);
        }
        match prepared.candidate {
            PreparedViewPublication::Unchanged => {}
            PreparedViewPublication::SourceOnly(publication) => {
                self.style_program = publication
                    .product
                    .style()
                    .map(|style| style.program().clone());
                self.product = publication.product;
                self.catalog = Some(publication.catalog);
            }
            PreparedViewPublication::Semantic(publication) => {
                let PreparedSemanticPublication {
                    product,
                    catalog,
                    registry,
                    inventory,
                    generation,
                    frame_revision,
                    mounts,
                    axis_seeds,
                    invalidation,
                } = *publication;
                self.style_program = product.style().map(|style| style.program().clone());
                self.product = product;
                self.catalog = Some(catalog);
                self.registry = registry;
                self.inventory = inventory;
                self.generation = generation;
                self.frame_revision = frame_revision;
                self.mounts = mounts;
                self.axis_seeds = axis_seeds;
                self.last_invalidation = Some(invalidation);
            }
        }
        Ok(prepared.outcome)
    }

    #[must_use]
    pub const fn accepted_generation(&self) -> AcceptedViewProgramGeneration {
        self.generation
    }

    #[must_use]
    pub const fn frame_revision(&self) -> u64 {
        self.frame_revision
    }

    #[must_use]
    pub const fn last_invalidation(&self) -> Option<&ViewProgramInvalidation> {
        self.last_invalidation.as_ref()
    }

    fn expected_replacement_state(&self, catalog: &ViewProgramCatalog) -> ExpectedViewRuntimeState {
        ExpectedViewRuntimeState {
            program: catalog.program_id().clone(),
            revision: catalog.revision(),
            source_revision: catalog.source_revision(),
            style: self.product.style().map(|style| style.resource().clone()),
            generation: self.generation,
            frame_revision: self.frame_revision,
            logical_time: self.logical_time,
            allocator: self.allocator,
            root_bindings: self.root_bindings.clone(),
            mounts: self.mounts.clone(),
            axis_seeds: self.axis_seeds.clone(),
            required_dialogue_views: self.required_dialogue_views.clone(),
        }
    }

    fn matches_expected_replacement_state(&self, expected: &ExpectedViewRuntimeState) -> bool {
        let Some(catalog) = &self.catalog else {
            return false;
        };
        catalog.program_id() == &expected.program
            && catalog.revision() == expected.revision
            && catalog.source_revision() == expected.source_revision
            && self
                .product
                .style()
                .map(ValidatedViewStyleResource::resource)
                == expected.style.as_ref()
            && self.generation == expected.generation
            && self.frame_revision == expected.frame_revision
            && self.logical_time == expected.logical_time
            && self.allocator == expected.allocator
            && self.root_bindings == expected.root_bindings
            && self.mounts == expected.mounts
            && self.axis_seeds == expected.axis_seeds
            && self.required_dialogue_views == expected.required_dialogue_views
    }

    fn validate_replacement_dialogue_views(
        &self,
        candidate: &ViewProgramCatalog,
    ) -> Result<(), ViewProgramReplacementError> {
        for definition in &self.required_dialogue_views {
            if candidate.definition_index(definition).is_none() {
                return Err(ViewProgramReplacementError::MissingRequiredDialogueView {
                    definition: definition.clone(),
                });
            }
            if !candidate.accepts_dialogue_input(definition) {
                return Err(
                    ViewProgramReplacementError::RequiredDialogueViewMissingRole {
                        definition: definition.clone(),
                    },
                );
            }
        }
        Ok(())
    }
}

fn same_optional_style_semantics(
    current: Option<&ValidatedViewStyleResource>,
    candidate: Option<&ValidatedViewStyleResource>,
) -> bool {
    match (current, candidate) {
        (None, None) => true,
        (Some(current), Some(candidate)) => current.has_same_runtime_semantics(candidate),
        (None, Some(_)) | (Some(_), None) => false,
    }
}

fn checked_next_frame_revision(revision: u64) -> Result<u64, ViewProgramReplacementError> {
    revision
        .checked_add(1)
        .ok_or(ViewProgramReplacementError::FrameRevisionExhausted)
}

#[cfg(test)]
mod tests {
    use super::{
        AcceptedViewProgramGeneration, ViewProgramReplacementError, checked_next_frame_revision,
    };

    #[test]
    fn replacement_counters_accept_exact_max_and_reject_one_over() {
        let generation_below_max: AcceptedViewProgramGeneration =
            serde_json::from_str(&(u64::MAX - 1).to_string()).unwrap();
        let max_generation = generation_below_max.checked_next().unwrap();
        assert_eq!(max_generation.get(), u64::MAX);
        assert_eq!(
            max_generation.checked_next(),
            Err(ViewProgramReplacementError::GenerationExhausted),
        );

        assert_eq!(checked_next_frame_revision(u64::MAX - 1), Ok(u64::MAX));
        assert_eq!(
            checked_next_frame_revision(u64::MAX),
            Err(ViewProgramReplacementError::FrameRevisionExhausted),
        );
    }
}
