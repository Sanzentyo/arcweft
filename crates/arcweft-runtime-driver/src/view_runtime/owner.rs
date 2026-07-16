//! Stable public owner evidence and private resolved mount authority.

use std::num::NonZeroU64;

use arcweft_view::{
    AcceptedViewProgramRevision, RustViewId, ViewId, ViewImplementation, ViewProgramId,
    ViewRegistry, ViewRegistryId, ViewSchemaId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::catalog::{ViewDefinitionIndex, ViewProgramCatalog};

/// Monotonic identity of one accepted semantic View-program generation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AcceptedViewProgramGeneration(NonZeroU64);

/// Public owner evidence that never exposes a process-local registry slot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewOwnerEvidence {
    Public { view: ViewId },
    AnonymousHost,
}

/// Stable owner identity persisted for one mounted View occurrence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SavedViewOwner {
    Rust {
        view: ViewId,
        schema: ViewSchemaId,
    },
    Arcweft {
        view: ViewId,
        program: ViewProgramId,
        revision: AcceptedViewProgramRevision,
    },
}

/// Failure to project or resolve a stable persisted View owner.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ViewSaveError {
    #[error("anonymous Rust Views cannot be persisted")]
    AnonymousRustViewNotPersistable,
    #[error("public View identity is no longer registered: {0}")]
    MissingPublicView(ViewId),
    #[error("saved View implementation kind does not match the registry")]
    ImplementationKindMismatch,
    #[error("saved View program/revision is not the accepted program")]
    ProgramMismatch,
}

/// Fully resolved in-memory owner authority for one live mount.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ResolvedMountedViewOwner {
    AnonymousRust {
        registry: ViewRegistryId,
        rust: RustViewId,
    },
    PublicRust {
        view: ViewId,
        registry: ViewRegistryId,
        rust: RustViewId,
    },
    Arcweft {
        view: ViewId,
        registry: ViewRegistryId,
        definition: ViewDefinitionIndex,
        program: ViewProgramId,
        revision: AcceptedViewProgramRevision,
        generation: AcceptedViewProgramGeneration,
    },
}

impl AcceptedViewProgramGeneration {
    pub const INITIAL: Self = Self(NonZeroU64::MIN);

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl ViewOwnerEvidence {
    pub const fn public_view(&self) -> Option<&ViewId> {
        match self {
            Self::Public { view } => Some(view),
            Self::AnonymousHost => None,
        }
    }
}

impl SavedViewOwner {
    pub const fn view(&self) -> &ViewId {
        match self {
            Self::Rust { view, .. } | Self::Arcweft { view, .. } => view,
        }
    }
}

impl ResolvedMountedViewOwner {
    pub(super) fn resolve_registry(
        slot: ViewRegistryId,
        registry: &ViewRegistry,
        catalog: Option<&ViewProgramCatalog>,
        generation: AcceptedViewProgramGeneration,
    ) -> Result<Self, ViewSaveError> {
        let descriptor = registry
            .get(slot)
            .ok_or(ViewSaveError::ImplementationKindMismatch)?;
        match descriptor.implementation() {
            ViewImplementation::Rust(rust) => match descriptor.id() {
                Some(view) => Ok(Self::PublicRust {
                    view: view.clone(),
                    registry: slot,
                    rust: *rust,
                }),
                None => Ok(Self::AnonymousRust {
                    registry: slot,
                    rust: *rust,
                }),
            },
            ViewImplementation::Arcweft { program } => {
                let view = descriptor
                    .id()
                    .ok_or(ViewSaveError::ImplementationKindMismatch)?;
                let catalog = catalog.ok_or(ViewSaveError::ProgramMismatch)?;
                if catalog.program_id() != program {
                    return Err(ViewSaveError::ProgramMismatch);
                }
                let definition = catalog
                    .definition_index(view)
                    .ok_or_else(|| ViewSaveError::MissingPublicView(view.clone()))?;
                Ok(Self::Arcweft {
                    view: view.clone(),
                    registry: slot,
                    definition,
                    program: program.clone(),
                    revision: catalog.revision(),
                    generation,
                })
            }
        }
    }

    pub(super) const fn view(&self) -> Option<&ViewId> {
        match self {
            Self::AnonymousRust { .. } => None,
            Self::PublicRust { view, .. } | Self::Arcweft { view, .. } => Some(view),
        }
    }

    pub(super) fn evidence(&self) -> ViewOwnerEvidence {
        self.view()
            .map_or(ViewOwnerEvidence::AnonymousHost, |view| {
                ViewOwnerEvidence::Public { view: view.clone() }
            })
    }

    pub(super) const fn definition(&self) -> Option<ViewDefinitionIndex> {
        match self {
            Self::Arcweft { definition, .. } => Some(*definition),
            Self::AnonymousRust { .. } | Self::PublicRust { .. } => None,
        }
    }

    pub(super) fn saved(&self, registry: &ViewRegistry) -> Result<SavedViewOwner, ViewSaveError> {
        match self {
            Self::AnonymousRust { .. } => Err(ViewSaveError::AnonymousRustViewNotPersistable),
            Self::PublicRust {
                view,
                registry: slot,
                rust,
            } => {
                let descriptor = registry
                    .get(*slot)
                    .filter(|descriptor| descriptor.id() == Some(view))
                    .ok_or_else(|| ViewSaveError::MissingPublicView(view.clone()))?;
                if descriptor.implementation() != &ViewImplementation::Rust(*rust) {
                    return Err(ViewSaveError::ImplementationKindMismatch);
                }
                Ok(SavedViewOwner::Rust {
                    view: view.clone(),
                    schema: descriptor.schema(),
                })
            }
            Self::Arcweft {
                view,
                registry: slot,
                program,
                revision,
                ..
            } => {
                let descriptor = registry
                    .get(*slot)
                    .filter(|descriptor| descriptor.id() == Some(view))
                    .ok_or_else(|| ViewSaveError::MissingPublicView(view.clone()))?;
                if descriptor.implementation()
                    != &(ViewImplementation::Arcweft {
                        program: program.clone(),
                    })
                {
                    return Err(ViewSaveError::ImplementationKindMismatch);
                }
                Ok(SavedViewOwner::Arcweft {
                    view: view.clone(),
                    program: program.clone(),
                    revision: *revision,
                })
            }
        }
    }

    pub(super) fn resolve_saved(
        saved: &SavedViewOwner,
        registry: &ViewRegistry,
        catalog: Option<&ViewProgramCatalog>,
        generation: AcceptedViewProgramGeneration,
    ) -> Result<Self, ViewSaveError> {
        let view = saved.view();
        let slot = registry
            .resolve(view)
            .ok_or_else(|| ViewSaveError::MissingPublicView(view.clone()))?;
        let descriptor = registry
            .get(slot)
            .ok_or_else(|| ViewSaveError::MissingPublicView(view.clone()))?;
        match saved {
            SavedViewOwner::Rust { view, schema } => {
                let resolved = Self::resolve_registry(slot, registry, catalog, generation)?;
                let Self::PublicRust {
                    view: resolved_view,
                    rust,
                    ..
                } = resolved
                else {
                    return Err(ViewSaveError::ImplementationKindMismatch);
                };
                if resolved_view != *view || descriptor.schema() != *schema {
                    return Err(ViewSaveError::ImplementationKindMismatch);
                }
                Ok(Self::PublicRust {
                    view: view.clone(),
                    registry: slot,
                    rust,
                })
            }
            SavedViewOwner::Arcweft {
                view,
                program,
                revision,
            } => {
                let catalog = catalog.ok_or(ViewSaveError::ProgramMismatch)?;
                if catalog.program_id() != program || catalog.revision() != *revision {
                    return Err(ViewSaveError::ProgramMismatch);
                }
                let definition = catalog
                    .definition_index(view)
                    .ok_or_else(|| ViewSaveError::MissingPublicView(view.clone()))?;
                if descriptor.implementation()
                    != &(ViewImplementation::Arcweft {
                        program: program.clone(),
                    })
                {
                    return Err(ViewSaveError::ImplementationKindMismatch);
                }
                Ok(Self::Arcweft {
                    view: view.clone(),
                    registry: slot,
                    definition,
                    program: program.clone(),
                    revision: *revision,
                    generation,
                })
            }
        }
    }
}
