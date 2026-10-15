use super::{
    AwbcRuntimeDialogueActivationSnapshot, AwbcRuntimePublishedDialogueHandlesSnapshot,
    LineRuntimeError, RuntimeDialogueActivationState, RuntimeDialogueCommitReceipt,
    RuntimeDialogueTerminalKind, RuntimeHandleDropReceipt, RuntimePublishedDialogueHandles,
};
use crate::runtime_id::{
    DialogueActivationId, ExecutionInstanceId, RuntimeDeferRegistrationId, RuntimeDeferSiteId,
};
use crate::task::RuntimeProgramOwner;
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
#[derive(Debug, PartialEq)]
pub(crate) struct RuntimeDialogueActivationRegistry<F, T> {
    entries: BTreeMap<DialogueActivationId, RuntimeDialogueRegistryEntry<F, T>>,
}

/// Sole owner of published activation metadata during an executor transfer.
/// Active frames and in-flight transactions cannot enter this phase.
#[derive(Debug, PartialEq)]
pub(crate) struct RuntimePublishedDialogueRegistry {
    entries: BTreeMap<DialogueActivationId, (u64, RuntimePublishedDialogueHandles)>,
}

impl RuntimePublishedDialogueRegistry {
    #[cfg(test)]
    pub(crate) fn with_revision_for_test(mut self, revision: u64) -> Self {
        for (current, _) in self.entries.values_mut() {
            *current = revision;
        }
        self
    }

    pub(crate) fn into_registry<F, T>(self) -> RuntimeDialogueActivationRegistry<F, T> {
        RuntimeDialogueActivationRegistry {
            entries: self
                .entries
                .into_iter()
                .map(|(activation, (revision, handles))| {
                    (
                        activation,
                        RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles },
                    )
                })
                .collect(),
        }
    }
}

impl<F, T> RuntimeDialogueActivationRegistry<F, T> {
    pub(crate) fn try_visit_active<'a, E>(
        &'a self,
        mut visit: impl FnMut(
            &'a DialogueActivationId,
            &'a F,
            &'a RuntimeDialogueActivationState<T>,
        ) -> Result<(), E>,
    ) -> Result<(), E> {
        for (activation, entry) in &self.entries {
            if let RuntimeDialogueRegistryEntry::Active { frame, line, .. } = entry {
                visit(activation, frame, line)?;
            }
        }
        Ok(())
    }

    pub(crate) fn into_published(
        self,
    ) -> Result<RuntimePublishedDialogueRegistry, (Self, LineRuntimeError)> {
        if self
            .entries
            .values()
            .any(|entry| !matches!(entry, RuntimeDialogueRegistryEntry::PublishedHandles { .. }))
        {
            return Err((self, LineRuntimeError::ParentHandleBeforePublication));
        }
        let entries = self
            .entries
            .into_iter()
            .map(|(activation, entry)| {
                let RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles } = entry
                else {
                    unreachable!("the complete borrowed phase check precedes the registry move")
                };
                (activation, (revision, handles))
            })
            .collect();
        Ok(RuntimePublishedDialogueRegistry { entries })
    }
}

impl<F, T> Default for RuntimeDialogueActivationRegistry<F, T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Debug, PartialEq)]
enum RuntimeDialogueRegistryEntry<F, T> {
    Active {
        revision: u64,
        frame: F,
        line: RuntimeDialogueActivationState<T>,
    },
    /// The frame and line have moved into one transaction. No second live
    /// transaction can be opened until that owner commits or is restored.
    InFlight { revision: u64 },
    PublishedHandles {
        revision: u64,
        handles: RuntimePublishedDialogueHandles,
    },
}

#[derive(Debug, PartialEq)]
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

pub(crate) struct RuntimeDialogueCommitProof {
    activation: DialogueActivationId,
    revision: u64,
    next_revision: u64,
}

/// Borrowed proof for the final publication transfer. It is obtained while
/// the result still belongs to the line, before the parent frame receives it.
pub(crate) struct RuntimeDialoguePublishedCommitProof {
    activation: DialogueActivationId,
    revision: u64,
    next_revision: u64,
}

/// Borrowed proof for the final abandoned-frame release.
pub(crate) struct RuntimeDialogueAbandonedCommitProof {
    activation: DialogueActivationId,
    revision: u64,
}

pub(crate) struct RuntimeDialoguePublishedOutcomeStage {
    updates: BTreeMap<DialogueActivationId, (u64, RuntimePublishedDialogueHandles)>,
    diagnostics: Vec<LineRuntimeError>,
}

/// Borrowed, fully checked parent-handle transition. It contains only cloned
/// ledger metadata and host commands, never a live RuntimeValue owner.
pub(crate) struct PreparedRuntimeParentFiberReconciliation {
    updates: Vec<(
        DialogueActivationId,
        u64,
        u64,
        RuntimePublishedDialogueHandles,
    )>,
    receipt: RuntimeHandleDropReceipt,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeDialogueRegistrySaveSnapshot<F, T> {
    entries: Vec<RuntimeDialogueRegistrySaveEntry<F, T>>,
}

impl<F, T> RuntimeDialogueRegistrySaveSnapshot<F, T> {
    pub(crate) fn active_line(
        &self,
        activation: &DialogueActivationId,
    ) -> Option<&AwbcRuntimeDialogueActivationSnapshot<T>> {
        self.entries.iter().find_map(|entry| match entry {
            RuntimeDialogueRegistrySaveEntry::Active {
                activation: candidate,
                line,
                ..
            } if candidate == activation => Some(line),
            _ => None,
        })
    }

    pub(crate) fn try_visit_active<'a, E>(
        &'a self,
        mut visit: impl FnMut(
            &'a DialogueActivationId,
            &'a F,
            &'a AwbcRuntimeDialogueActivationSnapshot<T>,
        ) -> Result<(), E>,
    ) -> Result<(), E> {
        for entry in &self.entries {
            match entry {
                RuntimeDialogueRegistrySaveEntry::Active {
                    activation,
                    frame,
                    line,
                    ..
                } => visit(activation, frame, line)?,
                RuntimeDialogueRegistrySaveEntry::PublishedHandles { .. } => {}
            }
        }
        Ok(())
    }
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

impl From<super::LineTaskSnapshotError> for RuntimeDialogueRegistrySnapshotError {
    fn from(error: super::LineTaskSnapshotError) -> Self {
        match error {
            super::LineTaskSnapshotError::Value(error) => Self::Value(error),
            error => Self::Frame {
                message: format!("invalid line-task reducer snapshot: {error}"),
            },
        }
    }
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

    /// Replaces the frame representation while moving the sole line owner
    /// through the same registry transaction.
    pub(crate) fn map_frame<G>(
        self,
        map: impl FnOnce(F) -> G,
    ) -> RuntimeDialogueActivationTransaction<G, T> {
        RuntimeDialogueActivationTransaction {
            activation: self.activation,
            revision: self.revision,
            frame: map(self.frame),
            line: self.line,
        }
    }
}

impl<F, T: Clone> RuntimeDialogueActivationRegistry<F, T> {
    #[cfg(test)]
    pub(crate) fn to_save_snapshot<S>(
        &self,
        snapshot_frame: impl FnMut(&F) -> Result<S, RuntimeDialogueRegistrySnapshotError>,
    ) -> Result<RuntimeDialogueRegistrySaveSnapshot<S, T>, RuntimeDialogueRegistrySnapshotError>
    where
        T: super::handle::RuntimeDialogueTypeCoordinate,
    {
        self.to_snapshot_with_owner(None, snapshot_frame)
    }

    #[cfg(test)]
    pub(crate) fn to_rollback_snapshot<S>(
        &self,
        owner: &RuntimeProgramOwner,
        snapshot_frame: impl FnMut(&F) -> Result<S, RuntimeDialogueRegistrySnapshotError>,
    ) -> Result<RuntimeDialogueRegistrySaveSnapshot<S, T>, RuntimeDialogueRegistrySnapshotError>
    where
        T: super::handle::RuntimeDialogueTypeCoordinate,
    {
        self.to_snapshot_with_owner(Some(owner), snapshot_frame)
    }

    #[cfg(test)]
    fn to_snapshot_with_owner<S>(
        &self,
        owner: Option<&RuntimeProgramOwner>,
        snapshot_frame: impl FnMut(&F) -> Result<S, RuntimeDialogueRegistrySnapshotError>,
    ) -> Result<RuntimeDialogueRegistrySaveSnapshot<S, T>, RuntimeDialogueRegistrySnapshotError>
    where
        T: super::handle::RuntimeDialogueTypeCoordinate,
    {
        let mut admission = match owner {
            Some(owner) => crate::value::RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            ),
            None => crate::value::RuntimeRetainedValueAdmission::standalone(
                crate::entry::RuntimeSchemaLimits::engine_default(),
            ),
        };
        self.admit_retained(&mut admission)?;
        self.snapshot_admitted(owner, snapshot_frame)
    }

    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), RuntimeDialogueRegistrySnapshotError>
    where
        T: super::handle::RuntimeDialogueTypeCoordinate,
    {
        admission.validate_metadata_collection(self.entries.len())?;
        for (activation, entry) in &self.entries {
            match entry {
                RuntimeDialogueRegistryEntry::Active { revision, line, .. } => {
                    admission.validate_metadata(&(activation, revision))?;
                    line.admit_retained_resources(admission)?;
                }
                RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles } => {
                    admission.validate_metadata(&(activation, revision, handles))?;
                    handles.restore_admit(activation)?;
                }
                RuntimeDialogueRegistryEntry::InFlight { .. } => {
                    return Err(RuntimeDialogueRegistrySnapshotError::Frame {
                        message: "dialogue transaction is in flight".to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    pub(crate) fn snapshot_admitted<S>(
        &self,
        owner: Option<&RuntimeProgramOwner>,
        mut snapshot_frame: impl FnMut(&F) -> Result<S, RuntimeDialogueRegistrySnapshotError>,
    ) -> Result<RuntimeDialogueRegistrySaveSnapshot<S, T>, RuntimeDialogueRegistrySnapshotError>
    where
        T: super::handle::RuntimeDialogueTypeCoordinate,
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
                        line: AwbcRuntimeDialogueActivationSnapshot::from_admitted_live(
                            line, owner,
                        )?,
                    },
                    RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles } => {
                        RuntimeDialogueRegistrySaveEntry::PublishedHandles {
                            activation: activation.clone(),
                            revision: *revision,
                            handles: handles.clone(),
                        }
                    }
                    RuntimeDialogueRegistryEntry::InFlight { .. } => {
                        return Err(RuntimeDialogueRegistrySnapshotError::Frame {
                            message: "dialogue transaction is in flight".to_owned(),
                        });
                    }
                })
            })
            .collect::<Result<_, RuntimeDialogueRegistrySnapshotError>>()?;
        Ok(RuntimeDialogueRegistrySaveSnapshot { entries })
    }

    #[cfg(test)]
    pub(crate) fn from_save_snapshot_with_deferred_children<S>(
        snapshot: RuntimeDialogueRegistrySaveSnapshot<S, T>,
        owner: &RuntimeProgramOwner,
        expected_children: &BTreeMap<
            DialogueActivationId,
            (RuntimeDeferRegistrationId, RuntimeDeferSiteId),
        >,
        restore_frame: impl FnMut(
            &DialogueActivationId,
            S,
            &RuntimeDialogueActivationState<T>,
        ) -> Result<F, RuntimeDialogueRegistrySnapshotError>,
    ) -> Result<Self, RuntimeDialogueRegistrySnapshotError>
    where
        T: super::handle::RuntimeDialogueTypeCoordinate,
    {
        // Borrow the complete journal before constructing the first line owner.
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        snapshot.admit_retained(&mut admission, expected_children)?;
        Self::from_admitted_save_snapshot(snapshot, owner, restore_frame)
    }

    pub(crate) fn from_admitted_save_snapshot<S>(
        snapshot: RuntimeDialogueRegistrySaveSnapshot<S, T>,
        owner: &RuntimeProgramOwner,
        mut restore_frame: impl FnMut(
            &DialogueActivationId,
            S,
            &RuntimeDialogueActivationState<T>,
        ) -> Result<F, RuntimeDialogueRegistrySnapshotError>,
    ) -> Result<Self, RuntimeDialogueRegistrySnapshotError>
    where
        T: super::handle::RuntimeDialogueTypeCoordinate,
    {
        let mut entries = BTreeMap::new();
        for entry in snapshot.entries {
            let (activation, entry) = match entry {
                RuntimeDialogueRegistrySaveEntry::Active {
                    activation,
                    revision,
                    frame,
                    line,
                } => {
                    let line = line.into_admitted_live(owner)?;
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
                } => (
                    activation,
                    RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles },
                ),
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
}

impl<F, T: Clone + super::handle::RuntimeDialogueTypeCoordinate>
    RuntimeDialogueRegistrySaveSnapshot<F, T>
{
    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        expected_children: &BTreeMap<
            DialogueActivationId,
            (RuntimeDeferRegistrationId, RuntimeDeferSiteId),
        >,
    ) -> Result<(), RuntimeDialogueRegistrySnapshotError> {
        admission.validate_metadata_collection(self.entries.len())?;
        admission.validate_metadata(expected_children)?;
        let mut identities = std::collections::BTreeSet::new();
        let mut active_children = std::collections::BTreeSet::new();
        for entry in &self.entries {
            let (activation, revision) = match entry {
                RuntimeDialogueRegistrySaveEntry::Active {
                    activation,
                    revision,
                    ..
                }
                | RuntimeDialogueRegistrySaveEntry::PublishedHandles {
                    activation,
                    revision,
                    ..
                } => (activation, revision),
            };
            admission.validate_metadata(&(activation, revision))?;
            if !identities.insert(activation) {
                return Err(RuntimeDialogueRegistrySnapshotError::DuplicateActivation {
                    activation: activation.clone(),
                });
            }
            match entry {
                RuntimeDialogueRegistrySaveEntry::Active { line, .. } => {
                    line.admit_retained_resources(admission)?;
                    line.restore_admit_with_deferred_child(
                        activation,
                        expected_children.get(activation).copied(),
                    )?;
                    active_children.insert(activation);
                }
                RuntimeDialogueRegistrySaveEntry::PublishedHandles { handles, .. } => {
                    admission.validate_metadata(handles)?;
                    handles.restore_admit(activation)?;
                    if handles.is_terminal() {
                        return Err(RuntimeDialogueRegistrySnapshotError::TerminalPublishedHandles);
                    }
                }
            }
        }
        if expected_children
            .keys()
            .any(|activation| !active_children.contains(activation))
        {
            return Err(LineRuntimeError::InvalidRestoredDeferredState.into());
        }
        Ok(())
    }
}

impl<F, T: Clone> RuntimeDialogueActivationRegistry<F, T> {
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

    #[must_use]
    pub(crate) fn has_active_or_inflight(&self) -> bool {
        self.entries.values().any(|entry| {
            matches!(
                entry,
                RuntimeDialogueRegistryEntry::Active { .. }
                    | RuntimeDialogueRegistryEntry::InFlight { .. }
            )
        })
    }

    pub(crate) fn active_frame(&self, activation: &DialogueActivationId) -> Option<&F> {
        match self.entries.get(activation) {
            Some(RuntimeDialogueRegistryEntry::Active { frame, .. }) => Some(frame),
            Some(
                RuntimeDialogueRegistryEntry::InFlight { .. }
                | RuntimeDialogueRegistryEntry::PublishedHandles { .. },
            )
            | None => None,
        }
    }

    pub(crate) fn active_line(
        &self,
        activation: &DialogueActivationId,
    ) -> Option<&RuntimeDialogueActivationState<T>> {
        match self.entries.get(activation) {
            Some(RuntimeDialogueRegistryEntry::Active { line, .. }) => Some(line),
            Some(
                RuntimeDialogueRegistryEntry::InFlight { .. }
                | RuntimeDialogueRegistryEntry::PublishedHandles { .. },
            )
            | None => None,
        }
    }

    pub(crate) fn is_published(&self, activation: &DialogueActivationId) -> bool {
        matches!(
            self.entries.get(activation),
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. })
        )
    }

    pub(crate) fn active_revision(&self, activation: &DialogueActivationId) -> Option<u64> {
        match self.entries.get(activation) {
            Some(RuntimeDialogueRegistryEntry::Active { revision, .. }) => Some(*revision),
            _ => None,
        }
    }

    /// Stages host outcomes against published ledgers only. Active frame and
    /// line owners are neither cloned nor mutated during ingress preflight.
    pub(crate) fn stage_published_outcomes(
        &self,
        outcomes: &[crate::presentation::RuntimeLineHostOutcome],
    ) -> Result<RuntimeDialoguePublishedOutcomeStage, LineRuntimeError> {
        let mut updates = BTreeMap::new();
        let mut diagnostics = Vec::new();
        for outcome in outcomes {
            let activation = outcome.command().activation();
            if !self.is_published(activation) {
                continue;
            }
            if !updates.contains_key(activation) {
                let Some(RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles }) =
                    self.entries.get(activation)
                else {
                    unreachable!()
                };
                updates.insert(activation.clone(), (*revision, handles.clone()));
            }
            let (revision, handles) = updates
                .get_mut(activation)
                .expect("staged published ledger");
            *revision = revision
                .checked_add(1)
                .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
            if let Some(diagnostic) = handles.accept_outcome(outcome)? {
                diagnostics.push(diagnostic);
            }
        }
        Ok(RuntimeDialoguePublishedOutcomeStage {
            updates,
            diagnostics,
        })
    }

    pub(crate) fn commit_published_outcomes(
        &mut self,
        stage: RuntimeDialoguePublishedOutcomeStage,
    ) -> Vec<LineRuntimeError> {
        for (activation, (revision, handles)) in stage.updates {
            if handles.is_terminal() {
                self.entries.remove(&activation);
            } else {
                *self
                    .entries
                    .get_mut(&activation)
                    .expect("staged published slot remains present") =
                    RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles };
            }
        }
        stage.diagnostics
    }

    pub(crate) fn begin_transaction(
        &mut self,
        activation: &DialogueActivationId,
    ) -> Result<RuntimeDialogueActivationTransaction<F, T>, LineRuntimeError> {
        match self.entries.get_mut(activation) {
            Some(entry @ RuntimeDialogueRegistryEntry::Active { .. }) => {
                let revision = match entry {
                    RuntimeDialogueRegistryEntry::Active { revision, .. } => *revision,
                    _ => unreachable!(),
                };
                let RuntimeDialogueRegistryEntry::Active { frame, line, .. } =
                    std::mem::replace(entry, RuntimeDialogueRegistryEntry::InFlight { revision })
                else {
                    unreachable!("only an active payload can be taken")
                };
                Ok(RuntimeDialogueActivationTransaction {
                    activation: activation.clone(),
                    revision,
                    frame,
                    line,
                })
            }
            Some(RuntimeDialogueRegistryEntry::InFlight { .. }) => {
                Err(LineRuntimeError::StaleActivationTransaction)
            }
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. }) => {
                Err(LineRuntimeError::ActivationFrameReleased)
            }
            None => Err(LineRuntimeError::UnknownActivationLedger),
        }
    }

    /// Returns a sole owned payload to its original active slot after a
    /// rejected staged operation. No value is copied.
    pub(crate) fn restore_transaction(
        &mut self,
        transaction: RuntimeDialogueActivationTransaction<F, T>,
    ) -> Result<(), LineRuntimeError> {
        let Some(entry @ RuntimeDialogueRegistryEntry::InFlight { .. }) =
            self.entries.get_mut(&transaction.activation)
        else {
            return Err(LineRuntimeError::StaleActivationTransaction);
        };
        if !matches!(entry, RuntimeDialogueRegistryEntry::InFlight { revision } if *revision == transaction.revision)
        {
            return Err(LineRuntimeError::StaleActivationTransaction);
        }
        *entry = RuntimeDialogueRegistryEntry::Active {
            revision: transaction.revision,
            frame: transaction.frame,
            line: transaction.line,
        };
        Ok(())
    }

    pub(crate) fn commit(
        &mut self,
        transaction: RuntimeDialogueActivationTransaction<F, T>,
    ) -> Result<RuntimeDialogueRegistryCommitReceipt, LineRuntimeError> {
        let proof = match self.inspect_commit(&transaction) {
            Ok(proof) => proof,
            Err(error) => {
                self.restore_transaction(transaction)?;
                return Err(error);
            }
        };
        Ok(self.commit_prepared(transaction, proof))
    }

    pub(crate) fn inspect_commit(
        &self,
        transaction: &RuntimeDialogueActivationTransaction<F, T>,
    ) -> Result<RuntimeDialogueCommitProof, LineRuntimeError> {
        if transaction.line.terminal_kind().is_some() {
            return Err(LineRuntimeError::UnexpectedTerminalDisposition);
        }
        let next_revision = match self.entries.get(&transaction.activation) {
            Some(RuntimeDialogueRegistryEntry::InFlight { revision })
                if *revision == transaction.revision =>
            {
                revision
                    .checked_add(1)
                    .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)
            }
            Some(
                RuntimeDialogueRegistryEntry::InFlight { .. }
                | RuntimeDialogueRegistryEntry::Active { .. },
            ) => Err(LineRuntimeError::StaleActivationTransaction),
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. }) => {
                Err(LineRuntimeError::ActivationFrameReleased)
            }
            None => Err(LineRuntimeError::UnknownActivationLedger),
        }?;
        Ok(RuntimeDialogueCommitProof {
            activation: transaction.activation.clone(),
            revision: transaction.revision,
            next_revision,
        })
    }

    pub(crate) fn commit_prepared(
        &mut self,
        mut transaction: RuntimeDialogueActivationTransaction<F, T>,
        proof: RuntimeDialogueCommitProof,
    ) -> RuntimeDialogueRegistryCommitReceipt {
        assert_eq!(transaction.activation, proof.activation);
        assert_eq!(transaction.revision, proof.revision);
        assert!(transaction.line.terminal_kind().is_none());
        assert!(matches!(
            self.entries.get(&proof.activation),
            Some(RuntimeDialogueRegistryEntry::InFlight { revision })
                if *revision == proof.revision
        ));
        let receipt = RuntimeDialogueRegistryCommitReceipt {
            line: transaction.line.take_commit_receipt(),
        };
        *self
            .entries
            .get_mut(&transaction.activation)
            .expect("checked in-flight slot") = RuntimeDialogueRegistryEntry::Active {
            revision: proof.next_revision,
            frame: transaction.frame,
            line: transaction.line,
        };
        receipt
    }

    /// Preflight a publishing line before its result value leaves line
    /// custody. A successful caller may release the frame, finish publication,
    /// bind the moved result, and then commit with this proof.
    pub(crate) fn inspect_published(
        &self,
        transaction: &RuntimeDialogueActivationTransaction<F, T>,
    ) -> Result<RuntimeDialoguePublishedCommitProof, LineRuntimeError> {
        transaction.line.inspect_publish_completion()?;
        let next_revision = match self.entries.get(&transaction.activation) {
            Some(RuntimeDialogueRegistryEntry::InFlight { revision })
                if *revision == transaction.revision =>
            {
                revision
                    .checked_add(1)
                    .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?
            }
            Some(
                RuntimeDialogueRegistryEntry::InFlight { .. }
                | RuntimeDialogueRegistryEntry::Active { .. },
            ) => return Err(LineRuntimeError::StaleActivationTransaction),
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. }) => {
                return Err(LineRuntimeError::ActivationFrameReleased);
            }
            None => return Err(LineRuntimeError::UnknownActivationLedger),
        };
        Ok(RuntimeDialoguePublishedCommitProof {
            activation: transaction.activation.clone(),
            revision: transaction.revision,
            next_revision,
        })
    }

    pub(crate) fn commit_published_prepared(
        &mut self,
        mut transaction: RuntimeDialogueActivationTransaction<F, T>,
        proof: RuntimeDialoguePublishedCommitProof,
    ) -> RuntimeDialogueRegistryCommitReceipt {
        assert_eq!(transaction.activation, proof.activation);
        assert_eq!(transaction.revision, proof.revision);
        assert!(matches!(
            self.entries.get(&proof.activation),
            Some(RuntimeDialogueRegistryEntry::InFlight { revision })
                if *revision == proof.revision
        ));
        assert!(transaction.line.can_into_published_handles().is_ok());
        let receipt = RuntimeDialogueRegistryCommitReceipt {
            line: transaction.line.take_commit_receipt(),
        };
        let handles = transaction
            .line
            .into_published_handles()
            .expect("publication shape passed borrowed preflight");
        if handles.has_live_leases() {
            *self
                .entries
                .get_mut(&transaction.activation)
                .expect("checked in-flight slot") =
                RuntimeDialogueRegistryEntry::PublishedHandles {
                    revision: proof.next_revision,
                    handles,
                };
        } else {
            self.entries.remove(&transaction.activation);
        }
        receipt
    }

    /// Preflight a failure-close line before its final frame release.
    pub(crate) fn inspect_abandoned(
        &self,
        transaction: &RuntimeDialogueActivationTransaction<F, T>,
    ) -> Result<RuntimeDialogueAbandonedCommitProof, LineRuntimeError> {
        transaction.line.inspect_abandon_completion()?;
        match self.entries.get(&transaction.activation) {
            Some(RuntimeDialogueRegistryEntry::InFlight { revision })
                if *revision == transaction.revision =>
            {
                Ok(RuntimeDialogueAbandonedCommitProof {
                    activation: transaction.activation.clone(),
                    revision: transaction.revision,
                })
            }
            Some(
                RuntimeDialogueRegistryEntry::InFlight { .. }
                | RuntimeDialogueRegistryEntry::Active { .. },
            ) => Err(LineRuntimeError::StaleActivationTransaction),
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. }) => {
                Err(LineRuntimeError::ActivationFrameReleased)
            }
            None => Err(LineRuntimeError::UnknownActivationLedger),
        }
    }

    pub(crate) fn commit_abandoned_prepared(
        &mut self,
        mut transaction: RuntimeDialogueActivationTransaction<F, T>,
        proof: RuntimeDialogueAbandonedCommitProof,
    ) -> RuntimeDialogueRegistryCommitReceipt {
        assert_eq!(transaction.activation, proof.activation);
        assert_eq!(transaction.revision, proof.revision);
        assert!(matches!(
            self.entries.get(&proof.activation),
            Some(RuntimeDialogueRegistryEntry::InFlight { revision })
                if *revision == proof.revision
        ));
        assert_eq!(
            transaction.line.terminal_kind(),
            Some(RuntimeDialogueTerminalKind::Abandoned)
        );
        assert!(transaction.line.is_terminal());
        let receipt = RuntimeDialogueRegistryCommitReceipt {
            line: transaction.line.take_commit_receipt(),
        };
        self.entries.remove(&transaction.activation);
        receipt
    }

    pub(crate) fn commit_published(
        &mut self,
        mut transaction: RuntimeDialogueActivationTransaction<F, T>,
    ) -> Result<RuntimeDialogueRegistryCommitReceipt, LineRuntimeError> {
        if transaction.line.terminal_kind() != Some(RuntimeDialogueTerminalKind::Published) {
            self.restore_transaction(transaction)?;
            return Err(LineRuntimeError::TerminalDispositionMismatch);
        }
        let next_revision = match self.entries.get(&transaction.activation) {
            Some(RuntimeDialogueRegistryEntry::InFlight { revision })
                if *revision == transaction.revision =>
            {
                revision
                    .checked_add(1)
                    .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)
            }
            Some(
                RuntimeDialogueRegistryEntry::InFlight { .. }
                | RuntimeDialogueRegistryEntry::Active { .. },
            ) => Err(LineRuntimeError::StaleActivationTransaction),
            Some(RuntimeDialogueRegistryEntry::PublishedHandles { .. }) => {
                Err(LineRuntimeError::ActivationFrameReleased)
            }
            None => Err(LineRuntimeError::UnknownActivationLedger),
        };
        let next_revision = match next_revision {
            Ok(revision) => revision,
            Err(error) => {
                self.restore_transaction(transaction)?;
                return Err(error);
            }
        };
        if let Err(error) = transaction.line.can_into_published_handles() {
            self.restore_transaction(transaction)?;
            return Err(error);
        }
        let receipt = RuntimeDialogueRegistryCommitReceipt {
            line: transaction.line.take_commit_receipt(),
        };
        let handles = transaction
            .line
            .into_published_handles()
            .expect("published handle extraction passed borrowed preflight");
        if handles.has_live_leases() {
            *self
                .entries
                .get_mut(&transaction.activation)
                .expect("checked in-flight slot") =
                RuntimeDialogueRegistryEntry::PublishedHandles {
                    revision: next_revision,
                    handles,
                };
        } else {
            self.entries.remove(&transaction.activation);
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
            self.restore_transaction(transaction)?;
            return Err(LineRuntimeError::TerminalDispositionMismatch);
        }
        if !matches!(self.entries.get(&transaction.activation),
            Some(RuntimeDialogueRegistryEntry::InFlight { revision }) if *revision == transaction.revision
        ) {
            self.restore_transaction(transaction)?;
            return Err(LineRuntimeError::StaleActivationTransaction);
        }
        let receipt = RuntimeDialogueRegistryCommitReceipt {
            line: transaction.line.take_commit_receipt(),
        };
        self.entries.remove(&transaction.activation);
        Ok(receipt)
    }

    pub(crate) fn accept_published_outcome(
        &mut self,
        outcome: &crate::presentation::RuntimeLineHostOutcome,
    ) -> Result<Option<LineRuntimeError>, LineRuntimeError> {
        let activation = outcome.command().activation().clone();
        let Some(RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles }) =
            self.entries.get(&activation)
        else {
            return Err(LineRuntimeError::StaleCommandOutcome);
        };
        let next_revision = revision
            .checked_add(1)
            .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
        let mut candidate = handles.clone();
        let diagnostic = candidate.accept_outcome(outcome)?;
        if candidate.is_terminal() {
            self.entries.remove(&activation);
        } else {
            *self
                .entries
                .get_mut(&activation)
                .expect("checked published slot") =
                RuntimeDialogueRegistryEntry::PublishedHandles {
                    revision: next_revision,
                    handles: candidate,
                };
        }
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
        drops: &super::RuntimeHandleDropAuthorization,
    ) -> Result<RuntimeHandleDropReceipt, LineRuntimeError> {
        let prepared = self.inspect_parent_fiber_reconciliation(execution, before, after, drops)?;
        Ok(self.commit_parent_fiber_reconciliation(prepared))
    }

    pub(crate) fn inspect_parent_fiber_reconciliation(
        &self,
        execution: ExecutionInstanceId,
        before: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        after: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        drops: &super::RuntimeHandleDropAuthorization,
    ) -> Result<PreparedRuntimeParentFiberReconciliation, LineRuntimeError> {
        if after.keys().any(|token| !before.contains_key(token)) {
            return Err(LineRuntimeError::UnexpectedParentHandleOccurrence);
        }
        drops.validate_removed(
            |token| before.contains_key(token),
            |token| after.contains_key(token),
        )?;
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

        let mut updates = Vec::new();
        let mut commands = Vec::new();
        for (activation, source) in grouped_before {
            let destination = grouped_after.remove(&activation).unwrap_or_default();
            let Some(RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles }) =
                self.entries.get(&activation)
            else {
                return Err(LineRuntimeError::ParentHandleBeforePublication);
            };
            let mut candidate = handles.clone();
            let receipt = candidate.reconcile_parent_owned(
                &activation,
                execution,
                &source,
                &destination,
                drops,
            )?;
            if source == destination {
                continue;
            }
            let next_revision = revision
                .checked_add(1)
                .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
            updates.push((activation, *revision, next_revision, candidate));
            commands.extend(receipt.into_commands());
        }
        if !grouped_after.is_empty() {
            return Err(LineRuntimeError::UnexpectedParentHandleOccurrence);
        }
        Ok(PreparedRuntimeParentFiberReconciliation {
            updates,
            receipt: RuntimeHandleDropReceipt::from_commands(commands),
        })
    }

    pub(crate) fn commit_parent_fiber_reconciliation(
        &mut self,
        prepared: PreparedRuntimeParentFiberReconciliation,
    ) -> RuntimeHandleDropReceipt {
        for (activation, expected_revision, revision, handles) in prepared.updates {
            assert!(
                matches!(
                    self.entries.get(&activation),
                    Some(RuntimeDialogueRegistryEntry::PublishedHandles { revision, .. })
                        if *revision == expected_revision
                ),
                "prepared parent-fiber reconciliation requires unchanged registry revision"
            );
            if handles.is_terminal() {
                self.entries.remove(&activation);
            } else {
                *self
                    .entries
                    .get_mut(&activation)
                    .expect("checked published slot") =
                    RuntimeDialogueRegistryEntry::PublishedHandles { revision, handles };
            }
        }
        prepared.receipt
    }
}

#[cfg(test)]
mod snapshot_admission_tests {
    use super::*;
    use crate::awbc::schema::{AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcTypeId};
    use crate::value::RuntimeValue;

    #[test]
    fn all_original_result_rows_precede_first_frame_copy_or_construction() {
        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(AwbcProgram {
            runtime_types: vec![AwbcRuntimeType::new(
                crate::pattern::RuntimeSemanticTypeId::from_bytes([0xb3; 32]),
                AwbcRuntimeTypeShape::Bool,
            )],
            ..AwbcProgram::default()
        }));
        let activation = |ordinal| {
            DialogueActivationId::new(
                crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0xb4; 32]).unwrap(),
                crate::runtime_id::RuntimePersistentFiberId::from_allocated(1),
                crate::runtime_id::RuntimeDialogueContentPlanId::from_accepted_ordinal(
                    std::num::NonZeroU32::MIN,
                ),
                ordinal,
            )
        };
        let mut first = RuntimeDialogueActivationState::new();
        first
            .commit_result(AwbcTypeId(0), RuntimeValue::Bool(true))
            .unwrap();
        let mut last = RuntimeDialogueActivationState::new();
        last.commit_result(
            AwbcTypeId(0),
            RuntimeValue::String("wrong original row".into()),
        )
        .unwrap();
        let registry = RuntimeDialogueActivationRegistry {
            entries: BTreeMap::from([
                (
                    activation(0),
                    RuntimeDialogueRegistryEntry::Active {
                        revision: 0,
                        frame: 0u32,
                        line: first,
                    },
                ),
                (
                    activation(1),
                    RuntimeDialogueRegistryEntry::Active {
                        revision: 0,
                        frame: 1u32,
                        line: last,
                    },
                ),
            ]),
        };
        let mut copied = 0;
        assert!(
            registry
                .to_rollback_snapshot(&owner, |frame| {
                    copied += 1;
                    Ok(*frame)
                })
                .is_err()
        );
        assert_eq!(copied, 0);

        // Standalone inert capture carries no executable type proof. Restore
        // must inspect both original rows before invoking its first projector.
        let inert = registry.to_save_snapshot(|frame| Ok(*frame)).unwrap();
        let bytes = serde_json::to_vec(&inert).unwrap();
        let mut constructed = 0;
        assert!(RuntimeDialogueActivationRegistry::<u32, AwbcTypeId>::from_save_snapshot_with_deferred_children(
            inert.clone(), &owner, &BTreeMap::new(), |_, frame, _| {
                constructed += 1;
                Ok(frame)
            },
        ).is_err());
        assert_eq!(constructed, 0);
        assert_eq!(serde_json::to_vec(&inert).unwrap(), bytes);
        assert_eq!(registry.entries.len(), 2);
    }
}
