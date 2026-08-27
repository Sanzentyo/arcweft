use super::{
    AwbcRuntimeDialogueActivationSnapshot, AwbcRuntimePublishedDialogueHandlesSnapshot,
    LineRuntimeError, RuntimeDialogueActivationState, RuntimeDialogueCommitReceipt,
    RuntimeDialogueTerminalKind, RuntimeHandleDropReceipt, RuntimePublishedDialogueHandles,
};
use crate::effect::RuntimeDropPolicy;
use crate::runtime_id::{DialogueActivationId, ExecutionInstanceId};
use crate::value::ownership::RuntimeOwnedSlotId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, btree_map::Entry};
use thiserror::Error;

/// Executor-neutral sole owner of active dialogue transactions and surviving
/// post-publication handle ledgers.
///
/// `F` is the executor-specific frame payload. Revision, line-runtime state,
/// Active/PublishedHandles phase, publication replacement, parent drop, and
/// command-outcome correlation are shared here by structured and Product
/// executors.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimeDialogueActivationRegistry<F, T> {
    entries: BTreeMap<DialogueActivationId, RuntimeDialogueRegistryEntry<F, T>>,
}

impl<F, T> Default for RuntimeDialogueActivationRegistry<F, T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum RuntimeDialogueRegistryEntry<F, T> {
    Active {
        revision: u64,
        frame: F,
        line: RuntimeDialogueActivationState<T>,
    },
    PublishedHandles {
        revision: u64,
        handles: RuntimePublishedDialogueHandles,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimeDialogueActivationTransaction<F, T> {
    activation: DialogueActivationId,
    revision: u64,
    frame: F,
    line: RuntimeDialogueActivationState<T>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimeDialogueRegistryCommitReceipt {
    line: RuntimeDialogueCommitReceipt,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeDialogueRegistrySaveSnapshot<F, T> {
    entries: Vec<RuntimeDialogueRegistrySaveEntry<F, T>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
enum RuntimeDialogueRegistrySaveEntry<F, T> {
    Active {
        activation: DialogueActivationId,
        revision: u64,
        frame: F,
        line: AwbcRuntimeDialogueActivationSnapshot<T>,
    },
    PublishedHandles {
        activation: DialogueActivationId,
        revision: u64,
        handles: AwbcRuntimePublishedDialogueHandlesSnapshot,
    },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum RuntimeDialogueRegistrySnapshotError {
    #[error(transparent)]
    Value(#[from] crate::value::AwbcRuntimeValueSnapshotError),
    #[error(transparent)]
    Line(#[from] LineRuntimeError),
    #[error("dialogue registry snapshot repeats activation {activation:?}")]
    DuplicateActivation { activation: DialogueActivationId },
    #[error("dialogue registry snapshot retains a terminal published-handle entry")]
    TerminalPublishedHandles,
    #[error("dialogue registry frame snapshot is invalid: {message}")]
    Frame { message: String },
}

impl RuntimeDialogueRegistryCommitReceipt {
    pub(crate) fn into_line(self) -> RuntimeDialogueCommitReceipt {
        self.line
    }
}

impl<F, T> RuntimeDialogueActivationTransaction<F, T> {
    pub(crate) const fn activation(&self) -> &DialogueActivationId {
        &self.activation
    }

    pub(crate) const fn frame(&self) -> &F {
        &self.frame
    }

    pub(crate) const fn frame_mut(&mut self) -> &mut F {
        &mut self.frame
    }

    pub(crate) const fn line(&self) -> &RuntimeDialogueActivationState<T> {
        &self.line
    }

    pub(crate) const fn line_mut(&mut self) -> &mut RuntimeDialogueActivationState<T> {
        &mut self.line
    }

    pub(crate) fn parts_mut(&mut self) -> (&mut F, &mut RuntimeDialogueActivationState<T>) {
        (&mut self.frame, &mut self.line)
    }
}

impl<F: Clone, T: Clone> RuntimeDialogueActivationRegistry<F, T> {
    pub(crate) fn to_save_snapshot<S>(
        &self,
        mut snapshot_frame: impl FnMut(&F) -> Result<S, RuntimeDialogueRegistrySnapshotError>,
    ) -> Result<RuntimeDialogueRegistrySaveSnapshot<S, T>, RuntimeDialogueRegistrySnapshotError>
    {
        let entries = self
            .entries
            .iter()
            .map(|(activation, entry)| {
                Ok(match entry {
                    RuntimeDialogueRegistryEntry::Active {
                        revision,
                        frame,
                        line,
                    } => RuntimeDialogueRegistrySaveEntry::Active {
                        activation: activation.clone(),
                        revision: *revision,
                        frame: snapshot_frame(frame)?,
                        line: AwbcRuntimeDialogueActivationSnapshot::from_live(line)?,
                    },
                    RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles } => {
                        RuntimeDialogueRegistrySaveEntry::PublishedHandles {
                            activation: activation.clone(),
                            revision: *revision,
                            handles: AwbcRuntimePublishedDialogueHandlesSnapshot::from_live(
                                handles,
                            ),
                        }
                    }
                })
            })
            .collect::<Result<_, RuntimeDialogueRegistrySnapshotError>>()?;
        Ok(RuntimeDialogueRegistrySaveSnapshot { entries })
    }

    pub(crate) fn from_save_snapshot<S>(
        snapshot: RuntimeDialogueRegistrySaveSnapshot<S, T>,
        mut restore_frame: impl FnMut(
            &DialogueActivationId,
            S,
            &RuntimeDialogueActivationState<T>,
        ) -> Result<F, RuntimeDialogueRegistrySnapshotError>,
    ) -> Result<Self, RuntimeDialogueRegistrySnapshotError> {
        let mut entries = BTreeMap::new();
        for entry in snapshot.entries {
            let (activation, entry) = match entry {
                RuntimeDialogueRegistrySaveEntry::Active {
                    activation,
                    revision,
                    frame,
                    line,
                } => {
                    let line = line.into_live()?;
                    line.restore_admit(&activation)?;
                    let frame = restore_frame(&activation, frame, &line)?;
                    (
                        activation,
                        RuntimeDialogueRegistryEntry::Active {
                            revision,
                            frame,
                            line,
                        },
                    )
                }
                RuntimeDialogueRegistrySaveEntry::PublishedHandles {
                    activation,
                    revision,
                    handles,
                } => {
                    let handles = handles.into_live();
                    handles.restore_admit(&activation)?;
                    if handles.is_terminal() {
                        return Err(RuntimeDialogueRegistrySnapshotError::TerminalPublishedHandles);
                    }
                    (
                        activation,
                        RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles },
                    )
                }
            };
            match entries.entry(activation.clone()) {
                Entry::Vacant(slot) => {
                    slot.insert(entry);
                }
                Entry::Occupied(_) => {
                    return Err(RuntimeDialogueRegistrySnapshotError::DuplicateActivation {
                        activation,
                    });
                }
            }
        }
        Ok(Self { entries })
    }

    pub(crate) fn begin(
        &mut self,
        activation: DialogueActivationId,
        frame: F,
    ) -> Result<(), LineRuntimeError> {
        match self.entries.entry(activation) {
            Entry::Vacant(entry) => {
                entry.insert(RuntimeDialogueRegistryEntry::Active {
                    revision: 0,
                    frame,
                    line: RuntimeDialogueActivationState::new(),
                });
                Ok(())
            }
            Entry::Occupied(_) => Err(LineRuntimeError::DuplicateActivationLedger),
        }
    }

    pub(crate) fn active_ids(&self) -> Vec<DialogueActivationId> {
        self.entries
            .iter()
            .filter_map(|(activation, entry)| {
                matches!(entry, RuntimeDialogueRegistryEntry::Active { .. })
                    .then_some(activation.clone())
            })
            .collect()
    }

    pub(crate) fn active_frame(&self, activation: &DialogueActivationId) -> Option<&F> {
        match self.entries.get(activation) {
            Some(RuntimeDialogueRegistryEntry::Active { frame, .. }) => Some(frame),
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. }) | None => None,
        }
    }

    pub(crate) fn active_line(
        &self,
        activation: &DialogueActivationId,
    ) -> Option<&RuntimeDialogueActivationState<T>> {
        match self.entries.get(activation) {
            Some(RuntimeDialogueRegistryEntry::Active { line, .. }) => Some(line),
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. }) | None => None,
        }
    }

    pub(crate) fn is_published(&self, activation: &DialogueActivationId) -> bool {
        matches!(
            self.entries.get(activation),
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. })
        )
    }

    pub(crate) fn begin_transaction(
        &self,
        activation: &DialogueActivationId,
    ) -> Result<RuntimeDialogueActivationTransaction<F, T>, LineRuntimeError> {
        match self.entries.get(activation) {
            Some(RuntimeDialogueRegistryEntry::Active {
                revision,
                frame,
                line,
            }) => Ok(RuntimeDialogueActivationTransaction {
                activation: activation.clone(),
                revision: *revision,
                frame: frame.clone(),
                line: line.clone(),
            }),
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. }) => {
                Err(LineRuntimeError::ActivationFrameReleased)
            }
            None => Err(LineRuntimeError::UnknownActivationLedger),
        }
    }

    pub(crate) fn commit(
        &mut self,
        mut transaction: RuntimeDialogueActivationTransaction<F, T>,
    ) -> Result<RuntimeDialogueRegistryCommitReceipt, LineRuntimeError> {
        if transaction.line.terminal_kind().is_some() {
            return Err(LineRuntimeError::UnexpectedTerminalDisposition);
        }
        let (revision, frame, line) = match self.entries.get_mut(&transaction.activation) {
            Some(RuntimeDialogueRegistryEntry::Active {
                revision,
                frame,
                line,
            }) => (revision, frame, line),
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. }) => {
                return Err(LineRuntimeError::ActivationFrameReleased);
            }
            None => return Err(LineRuntimeError::UnknownActivationLedger),
        };
        if *revision != transaction.revision {
            return Err(LineRuntimeError::StaleActivationTransaction);
        }
        let next_revision = revision
            .checked_add(1)
            .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
        let receipt = RuntimeDialogueRegistryCommitReceipt {
            line: transaction.line.take_commit_receipt(),
        };
        *revision = next_revision;
        *frame = transaction.frame;
        *line = transaction.line;
        Ok(receipt)
    }

    pub(crate) fn commit_published(
        &mut self,
        mut transaction: RuntimeDialogueActivationTransaction<F, T>,
    ) -> Result<RuntimeDialogueRegistryCommitReceipt, LineRuntimeError> {
        if transaction.line.terminal_kind() != Some(RuntimeDialogueTerminalKind::Published) {
            return Err(LineRuntimeError::TerminalDispositionMismatch);
        }
        let Entry::Occupied(mut entry) = self.entries.entry(transaction.activation.clone()) else {
            return Err(LineRuntimeError::UnknownActivationLedger);
        };
        let RuntimeDialogueRegistryEntry::Active { revision, .. } = entry.get() else {
            return Err(LineRuntimeError::ActivationFrameReleased);
        };
        if *revision != transaction.revision {
            return Err(LineRuntimeError::StaleActivationTransaction);
        }
        let next_revision = revision
            .checked_add(1)
            .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
        let receipt = RuntimeDialogueRegistryCommitReceipt {
            line: transaction.line.take_commit_receipt(),
        };
        let handles = transaction.line.into_published_handles()?;
        if handles.has_live_leases() {
            entry.insert(RuntimeDialogueRegistryEntry::PublishedHandles {
                revision: next_revision,
                handles,
            });
        } else {
            entry.remove();
        }
        Ok(receipt)
    }

    pub(crate) fn commit_abandoned(
        &mut self,
        mut transaction: RuntimeDialogueActivationTransaction<F, T>,
    ) -> Result<RuntimeDialogueRegistryCommitReceipt, LineRuntimeError> {
        if transaction.line.terminal_kind() != Some(RuntimeDialogueTerminalKind::Abandoned)
            || !transaction.line.is_terminal()
        {
            return Err(LineRuntimeError::TerminalDispositionMismatch);
        }
        let Entry::Occupied(entry) = self.entries.entry(transaction.activation.clone()) else {
            return Err(LineRuntimeError::UnknownActivationLedger);
        };
        let RuntimeDialogueRegistryEntry::Active { revision, .. } = entry.get() else {
            return Err(LineRuntimeError::ActivationFrameReleased);
        };
        if *revision != transaction.revision {
            return Err(LineRuntimeError::StaleActivationTransaction);
        }
        let receipt = RuntimeDialogueRegistryCommitReceipt {
            line: transaction.line.take_commit_receipt(),
        };
        entry.remove();
        Ok(receipt)
    }

    pub(crate) fn accept_published_outcome(
        &mut self,
        outcome: &crate::presentation::RuntimeLineHostOutcome,
    ) -> Result<Option<LineRuntimeError>, LineRuntimeError> {
        let activation = outcome.command().activation().clone();
        let mut next = self.clone();
        let Some(RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles }) =
            next.entries.get_mut(&activation)
        else {
            return Err(LineRuntimeError::StaleCommandOutcome);
        };
        let next_revision = revision
            .checked_add(1)
            .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
        let diagnostic = handles.accept_outcome(outcome)?;
        *revision = next_revision;
        if handles.is_terminal() {
            next.entries.remove(&activation);
        }
        *self = next;
        Ok(diagnostic)
    }

    /// Commits the complete before/after parent-fiber affine graph as one
    /// registry transaction. Exact parent register slots, ledger owners,
    /// command journals, and entry revisions advance together or remain
    /// unchanged.
    pub(crate) fn reconcile_parent_fiber(
        &mut self,
        execution: ExecutionInstanceId,
        before: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        after: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        drop_policy: Option<RuntimeDropPolicy>,
    ) -> Result<RuntimeHandleDropReceipt, LineRuntimeError> {
        if after.keys().any(|token| !before.contains_key(token)) {
            return Err(LineRuntimeError::UnexpectedParentHandleOccurrence);
        }
        let mut grouped_before = BTreeMap::<DialogueActivationId, BTreeMap<_, _>>::new();
        let mut grouped_after = BTreeMap::<DialogueActivationId, BTreeMap<_, _>>::new();
        for (token, owner) in before {
            grouped_before
                .entry(token.activation().clone())
                .or_default()
                .insert(token.clone(), *owner);
        }
        for (token, owner) in after {
            grouped_after
                .entry(token.activation().clone())
                .or_default()
                .insert(token.clone(), *owner);
        }

        let mut next = self.clone();
        let mut commands = Vec::new();
        for (activation, source) in grouped_before {
            let destination = grouped_after.remove(&activation).unwrap_or_default();
            if source == destination {
                continue;
            }
            let Some(RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles }) =
                next.entries.get_mut(&activation)
            else {
                return Err(LineRuntimeError::ParentHandleBeforePublication);
            };
            let next_revision = revision
                .checked_add(1)
                .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
            let receipt = handles.reconcile_parent_owned(
                &activation,
                execution,
                &source,
                &destination,
                drop_policy,
            )?;
            *revision = next_revision;
            commands.extend(receipt.into_commands());
        }
        if !grouped_after.is_empty() {
            return Err(LineRuntimeError::UnexpectedParentHandleOccurrence);
        }
        next.entries.retain(|_, entry| {
            !matches!(
                entry,
                RuntimeDialogueRegistryEntry::PublishedHandles { handles, .. }
                    if handles.is_terminal()
            )
        });
        *self = next;
        Ok(RuntimeHandleDropReceipt::from_commands(commands))
    }
}
