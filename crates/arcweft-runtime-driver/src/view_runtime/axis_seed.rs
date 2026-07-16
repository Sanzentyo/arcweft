//! Host-owned logical-axis seed lifecycle for root View mounts.

use super::BundleViewRuntimeError;
use crate::presentation_handles::{
    PresentationHandleId, PresentationHandleKind, PresentationHandleRecord,
};
use arcweft_view::{
    ViewBoxAxisHostSeed, ViewBoxAxisRevision, ViewBoxAxisSeedGeneration, ViewBoxAxisSeedSource,
    ViewInheritedBoxAxes, ViewMountId,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// One compare-and-swap request for a live top-level View mount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BundleViewAxisSeedUpdate {
    pub mount: ViewMountId,
    pub expected_revision: ViewBoxAxisRevision,
    pub seed: ViewBoxAxisHostSeed,
}

/// Result of applying a live host seed update.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleViewAxisSeedUpdateOutcome {
    Unchanged {
        seed: ViewInheritedBoxAxes,
    },
    Updated {
        previous: ViewInheritedBoxAxes,
        current: ViewInheritedBoxAxes,
    },
}

/// One pending single-use next-root-mount reservation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleViewPendingAxisSeedSnapshot {
    pub handle: PresentationHandleId,
    pub seed: ViewBoxAxisHostSeed,
}

/// Exact host seed state retained by one mounted root occurrence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleViewMountedAxisSeedSnapshot {
    pub handle: PresentationHandleId,
    pub mount: ViewMountId,
    pub seed: ViewBoxAxisHostSeed,
    pub generation: ViewBoxAxisSeedGeneration,
    pub derived: ViewInheritedBoxAxes,
}

/// Persisted root seed registry. Vec wire shapes preserve duplicate evidence.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleViewAxisSeedRegistrySnapshot {
    pub pending: Vec<BundleViewPendingAxisSeedSnapshot>,
    pub mounted: Vec<BundleViewMountedAxisSeedSnapshot>,
}

/// Typed host-seed lifecycle and snapshot validation failures.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BundleViewAxisSeedError {
    #[error("presentation handle `{handle}` already owns root View mount {mount:?}")]
    HandleAlreadyMounted {
        handle: PresentationHandleId,
        mount: ViewMountId,
    },
    #[error("presentation handle `{handle}` is terminal")]
    TerminalHandle { handle: PresentationHandleId },
    #[error("presentation handle `{handle}` is not a View handle")]
    NonViewHandle { handle: PresentationHandleId },
    #[error("View mount {mount:?} is stale")]
    StaleMount { mount: ViewMountId },
    #[error("View mount {mount:?} is nested and has no host seed")]
    NestedMount { mount: ViewMountId },
    #[error(
        "View mount {mount:?} axis revision mismatch: expected {expected:?}, actual {actual:?}"
    )]
    RevisionMismatch {
        mount: ViewMountId,
        expected: ViewBoxAxisRevision,
        actual: ViewBoxAxisRevision,
    },
    #[error("View mount {mount:?} axis seed generation is exhausted")]
    RevisionExhausted { mount: ViewMountId },
    #[error("View axis seed snapshot repeats mount {mount:?}")]
    DuplicateMount { mount: ViewMountId },
    #[error("View axis seed snapshot repeats pending handle `{handle}`")]
    DuplicatePendingHandle { handle: PresentationHandleId },
    #[error("View axis seed snapshot repeats mounted handle `{handle}`")]
    DuplicateMountedHandle { handle: PresentationHandleId },
    #[error("View axis seed snapshot has pending and mounted state for `{handle}` at {mount:?}")]
    PendingForMountedHandle {
        handle: PresentationHandleId,
        mount: ViewMountId,
    },
    #[error("View axis seed snapshot references unknown or nested mount {mount:?}")]
    UnknownSnapshotMount { mount: ViewMountId },
    #[error("View axis seed snapshot mount {mount:?} belongs to `{actual}`, expected `{expected}`")]
    SnapshotHandleMismatch {
        mount: ViewMountId,
        expected: PresentationHandleId,
        actual: PresentationHandleId,
    },
    #[error("View axis seed snapshot references terminal handle `{handle}`")]
    SnapshotTerminalHandle { handle: PresentationHandleId },
    #[error("View axis seed snapshot references non-View handle `{handle}`")]
    SnapshotNonViewHandle { handle: PresentationHandleId },
    #[error("ordinary and dialogue root handles collide at `{handle}`")]
    SnapshotRootHandleCollision { handle: PresentationHandleId },
    #[error("root View mount {mount:?} is missing its axis seed snapshot")]
    MissingSnapshotSeed { mount: ViewMountId },
    #[error("root View mount {mount:?} has invalid axis seed source {seed_source:?}")]
    SnapshotSeedSource {
        mount: ViewMountId,
        seed_source: ViewBoxAxisSeedSource,
    },
    #[error("root View mount {mount:?} has a mismatched derived axis seed")]
    SnapshotSeedMismatch { mount: ViewMountId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MountedAxisSeed {
    handle: PresentationHandleId,
    seed: ViewBoxAxisHostSeed,
    generation: ViewBoxAxisSeedGeneration,
    derived: ViewInheritedBoxAxes,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct BundleViewAxisSeedRegistry {
    pending: BTreeMap<PresentationHandleId, ViewBoxAxisHostSeed>,
    mounted: BTreeMap<ViewMountId, MountedAxisSeed>,
    mounted_by_handle: BTreeMap<PresentationHandleId, ViewMountId>,
}

pub(super) struct PendingRootAxisSeed {
    mount: ViewMountId,
    record: MountedAxisSeed,
}

impl BundleViewAxisSeedRegistry {
    pub(super) fn configure_next(
        &mut self,
        handle: PresentationHandleId,
        seed: ViewBoxAxisHostSeed,
        handles: &[PresentationHandleRecord],
    ) -> Result<(), BundleViewAxisSeedError> {
        if let Some(mount) = self.mounted_by_handle.get(&handle).copied() {
            return Err(BundleViewAxisSeedError::HandleAlreadyMounted { handle, mount });
        }
        if let Some(record) = handles.iter().find(|record| record.id == handle) {
            if record.is_terminal() {
                return Err(BundleViewAxisSeedError::TerminalHandle { handle });
            }
            if record.kind != PresentationHandleKind::View {
                return Err(BundleViewAxisSeedError::NonViewHandle { handle });
            }
        }
        self.pending.insert(handle, seed);
        Ok(())
    }

    pub(super) fn cancel_next(
        &mut self,
        handle: &PresentationHandleId,
    ) -> Option<ViewBoxAxisHostSeed> {
        self.pending.remove(handle)
    }

    pub(super) fn prepare_root_mount(
        &self,
        handle: &PresentationHandleId,
        mount: ViewMountId,
    ) -> Result<PendingRootAxisSeed, BundleViewAxisSeedError> {
        if self.mounted.contains_key(&mount) {
            return Err(BundleViewAxisSeedError::DuplicateMount { mount });
        }
        if let Some(existing) = self.mounted_by_handle.get(handle).copied() {
            return Err(BundleViewAxisSeedError::HandleAlreadyMounted {
                handle: handle.clone(),
                mount: existing,
            });
        }
        let seed = self.pending.get(handle).copied().unwrap_or_default();
        let generation = ViewBoxAxisSeedGeneration::INITIAL;
        Ok(PendingRootAxisSeed {
            mount,
            record: MountedAxisSeed {
                handle: handle.clone(),
                seed,
                generation,
                derived: ViewInheritedBoxAxes::for_host_seed(mount, generation, seed),
            },
        })
    }

    pub(super) fn commit_root_mount(
        &mut self,
        plan: PendingRootAxisSeed,
    ) -> Result<(), BundleViewAxisSeedError> {
        if self.mounted.contains_key(&plan.mount) {
            return Err(BundleViewAxisSeedError::DuplicateMount { mount: plan.mount });
        }
        if let Some(mount) = self.mounted_by_handle.get(&plan.record.handle).copied() {
            return Err(BundleViewAxisSeedError::HandleAlreadyMounted {
                handle: plan.record.handle,
                mount,
            });
        }
        self.pending.remove(&plan.record.handle);
        self.mounted_by_handle
            .insert(plan.record.handle.clone(), plan.mount);
        self.mounted.insert(plan.mount, plan.record);
        Ok(())
    }

    pub(super) fn mounted_seed(&self, mount: ViewMountId) -> Option<ViewInheritedBoxAxes> {
        self.mounted.get(&mount).map(|record| record.derived)
    }

    pub(super) fn update(
        &mut self,
        update: BundleViewAxisSeedUpdate,
    ) -> Result<BundleViewAxisSeedUpdateOutcome, BundleViewAxisSeedError> {
        let record =
            self.mounted
                .get_mut(&update.mount)
                .ok_or(BundleViewAxisSeedError::StaleMount {
                    mount: update.mount,
                })?;
        if record.derived.revision() != update.expected_revision {
            return Err(BundleViewAxisSeedError::RevisionMismatch {
                mount: update.mount,
                expected: update.expected_revision,
                actual: record.derived.revision(),
            });
        }
        if record.seed == update.seed {
            return Ok(BundleViewAxisSeedUpdateOutcome::Unchanged {
                seed: record.derived,
            });
        }
        let generation = record.generation.checked_next().map_err(|_| {
            BundleViewAxisSeedError::RevisionExhausted {
                mount: update.mount,
            }
        })?;
        let previous = record.derived;
        let current = ViewInheritedBoxAxes::for_host_seed(update.mount, generation, update.seed);
        record.seed = update.seed;
        record.generation = generation;
        record.derived = current;
        Ok(BundleViewAxisSeedUpdateOutcome::Updated { previous, current })
    }

    pub(super) fn cleanup_known_handles(
        &mut self,
        handles: &[PresentationHandleRecord],
    ) -> Vec<PresentationHandleId> {
        let terminal = handles
            .iter()
            .filter(|record| record.is_terminal())
            .map(|record| record.id.clone())
            .collect::<BTreeSet<_>>();
        let non_view_pending = handles
            .iter()
            .filter(|record| !record.is_terminal() && record.kind != PresentationHandleKind::View)
            .filter(|record| self.pending.contains_key(&record.id))
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        for handle in terminal.iter().chain(non_view_pending.iter()) {
            self.pending.remove(handle);
        }
        let removed_mounts = self
            .mounted_by_handle
            .iter()
            .filter(|(handle, _)| terminal.contains(*handle))
            .map(|(_, mount)| *mount)
            .collect::<Vec<_>>();
        for mount in removed_mounts {
            self.remove_mount(mount);
        }
        non_view_pending
    }

    pub(super) fn retain_mounts(&mut self, mounts: &BTreeSet<ViewMountId>) {
        let removed = self
            .mounted
            .keys()
            .filter(|mount| !mounts.contains(mount))
            .copied()
            .collect::<Vec<_>>();
        for mount in removed {
            self.remove_mount(mount);
        }
    }

    pub(super) fn snapshot(&self) -> BundleViewAxisSeedRegistrySnapshot {
        BundleViewAxisSeedRegistrySnapshot {
            pending: self
                .pending
                .iter()
                .map(|(handle, seed)| BundleViewPendingAxisSeedSnapshot {
                    handle: handle.clone(),
                    seed: *seed,
                })
                .collect(),
            mounted: self
                .mounted
                .iter()
                .map(|(mount, record)| BundleViewMountedAxisSeedSnapshot {
                    handle: record.handle.clone(),
                    mount: *mount,
                    seed: record.seed,
                    generation: record.generation,
                    derived: record.derived,
                })
                .collect(),
        }
    }

    pub(super) fn restore(
        snapshot: &BundleViewAxisSeedRegistrySnapshot,
        roots: &BTreeMap<ViewMountId, PresentationHandleId>,
        handles: &[PresentationHandleRecord],
    ) -> Result<Self, BundleViewAxisSeedError> {
        let inventory = checked_handle_inventory(handles)?;
        let mut pending = BTreeMap::new();
        for saved in &snapshot.pending {
            validate_snapshot_handle(&saved.handle, &inventory)?;
            if pending.insert(saved.handle.clone(), saved.seed).is_some() {
                return Err(BundleViewAxisSeedError::DuplicatePendingHandle {
                    handle: saved.handle.clone(),
                });
            }
        }

        let mut mounted = BTreeMap::new();
        let mut mounted_by_handle = BTreeMap::new();
        for saved in &snapshot.mounted {
            let Some(expected_handle) = roots.get(&saved.mount) else {
                return Err(BundleViewAxisSeedError::UnknownSnapshotMount { mount: saved.mount });
            };
            if expected_handle != &saved.handle {
                return Err(BundleViewAxisSeedError::SnapshotHandleMismatch {
                    mount: saved.mount,
                    expected: expected_handle.clone(),
                    actual: saved.handle.clone(),
                });
            }
            validate_mounted_snapshot_handle(saved.mount, &saved.handle, &inventory)?;
            if mounted.contains_key(&saved.mount) {
                return Err(BundleViewAxisSeedError::DuplicateMount { mount: saved.mount });
            }
            if mounted_by_handle
                .insert(saved.handle.clone(), saved.mount)
                .is_some()
            {
                return Err(BundleViewAxisSeedError::DuplicateMountedHandle {
                    handle: saved.handle.clone(),
                });
            }
            if saved.derived.source() != saved.seed.source() {
                return Err(BundleViewAxisSeedError::SnapshotSeedSource {
                    mount: saved.mount,
                    seed_source: saved.derived.source(),
                });
            }
            let derived =
                ViewInheritedBoxAxes::for_host_seed(saved.mount, saved.generation, saved.seed);
            if saved.derived != derived {
                return Err(BundleViewAxisSeedError::SnapshotSeedMismatch { mount: saved.mount });
            }
            mounted.insert(
                saved.mount,
                MountedAxisSeed {
                    handle: saved.handle.clone(),
                    seed: saved.seed,
                    generation: saved.generation,
                    derived,
                },
            );
        }
        for (mount, handle) in roots {
            if !mounted.contains_key(mount) {
                return Err(BundleViewAxisSeedError::MissingSnapshotSeed { mount: *mount });
            }
            if pending.contains_key(handle) {
                return Err(BundleViewAxisSeedError::PendingForMountedHandle {
                    handle: handle.clone(),
                    mount: *mount,
                });
            }
        }
        Ok(Self {
            pending,
            mounted,
            mounted_by_handle,
        })
    }

    fn remove_mount(&mut self, mount: ViewMountId) {
        if let Some(record) = self.mounted.remove(&mount) {
            self.mounted_by_handle.remove(&record.handle);
        }
    }
}

fn checked_handle_inventory(
    handles: &[PresentationHandleRecord],
) -> Result<BTreeMap<PresentationHandleId, &PresentationHandleRecord>, BundleViewAxisSeedError> {
    let mut inventory = BTreeMap::new();
    for handle in handles {
        if inventory.insert(handle.id.clone(), handle).is_some() {
            return Err(BundleViewAxisSeedError::SnapshotRootHandleCollision {
                handle: handle.id.clone(),
            });
        }
    }
    Ok(inventory)
}

fn validate_snapshot_handle(
    handle: &PresentationHandleId,
    inventory: &BTreeMap<PresentationHandleId, &PresentationHandleRecord>,
) -> Result<(), BundleViewAxisSeedError> {
    let Some(record) = inventory.get(handle) else {
        // Prospective pending handles are allowed. Mounted entries are also
        // checked against the exact root map before this function is called.
        return Ok(());
    };
    if record.is_terminal() {
        return Err(BundleViewAxisSeedError::SnapshotTerminalHandle {
            handle: handle.clone(),
        });
    }
    if record.kind != PresentationHandleKind::View {
        return Err(BundleViewAxisSeedError::SnapshotNonViewHandle {
            handle: handle.clone(),
        });
    }
    Ok(())
}

fn validate_mounted_snapshot_handle(
    mount: ViewMountId,
    handle: &PresentationHandleId,
    inventory: &BTreeMap<PresentationHandleId, &PresentationHandleRecord>,
) -> Result<(), BundleViewAxisSeedError> {
    if !inventory.contains_key(handle) {
        return Err(BundleViewAxisSeedError::UnknownSnapshotMount { mount });
    }
    validate_snapshot_handle(handle, inventory)
}

impl From<BundleViewAxisSeedError> for BundleViewRuntimeError {
    fn from(error: BundleViewAxisSeedError) -> Self {
        Self::AxisSeed(error)
    }
}

#[cfg(test)]
#[path = "axis_seed_tests.rs"]
mod tests;
