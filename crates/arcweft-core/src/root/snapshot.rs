//! Program-bound durable root snapshot wire and exact selected-entry admission.

use super::{
    RootRuntimeError, RootStateSnapshotV1, RuntimePayload, RuntimeStatefulEntryRoles,
    TypeLayoutHash,
};
use thiserror::Error;

/// Inert version-one wire projection of a durable root state.
/// Values are admitted through the selected program before live publication.
pub type RootStateSaveSnapshotV1 = super::RootStateSnapshot<crate::value::AwbcRuntimeValueSnapshot>;

impl Clone for RootStateSaveSnapshotV1 {
    fn clone(&self) -> Self {
        Self {
            state_identity: self.state_identity.clone(),
            state_layout: self.state_layout,
            event_identity: self.event_identity.clone(),
            event_layout: self.event_layout,
            phase: self.phase.clone(),
            next_sequence: self.next_sequence,
            failure: self.failure.clone(),
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RootStateSaveSnapshotError {
    #[error("Root save requires quiescence: {blockers:?}")]
    NotQuiescent { blockers: super::RootSaveBlockers },
    #[error(transparent)]
    Root(#[from] RootRuntimeError),
    #[error(transparent)]
    Value(#[from] crate::value::AwbcRuntimeValueSnapshotError),
}

// The shared owning grammar validates both storage roles with the original
// selected entry and the enclosing resource bank. Entry-local policy is an
// additional allowance for the complete root, never a replacement for it.
impl<P> super::RootStateSnapshot<P> {
    fn admit_root(
        &self,
        active: &crate::entry::ActiveEntrySnapshotV1,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), RootStateSaveSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        admission.status()?;
        admission.validate_metadata(&(
            &self.state_identity,
            self.state_layout,
            &self.event_identity,
            self.event_layout,
            self.next_sequence,
            self.phase.header(),
            &self.failure,
        ))?;
        if let Some(failure) = &self.failure {
            failure.validate_state(self.phase.header(), self.next_sequence)?;
        }
        let owner =
            admission
                .program_owner()
                .cloned()
                .ok_or(RootRuntimeError::SnapshotRoleMismatch(
                    "original program owner",
                ))?;
        let roles = RootStateSaveSnapshotV1::selected_roles(active, &owner)?;
        roles
            .event
            .validate_for_program(owner.types())
            .map_err(RootRuntimeError::InvalidSnapshotValue)?;
        RootStateSaveSnapshotV1::validate_header(
            &self.state_identity,
            self.state_layout,
            &self.event_identity,
            self.event_layout,
            roles,
        )?;
        roles
            .state
            .validate_for_program(owner.types())
            .map_err(RootRuntimeError::InvalidSnapshotValue)?;
        let value = match &self.phase {
            super::RootPhase::Initializing { .. } => {
                let contract = super::RootStartupContract::from_program(owner.types(), &active.id)?;
                self.phase
                    .validate_initialization(&contract, self.next_sequence)?;
                return Ok(());
            }
            super::RootPhase::Initialized {
                value,
                reducer: super::RootReducerPhase::Ready,
            } => value.borrowed_value(),
            super::RootPhase::Initialized { .. } => {
                return Err(RootRuntimeError::SnapshotRoleMismatch("active reducer").into());
            }
        };
        let expected = roles.state.semantic_identity;
        let entry_limits = roles.command_policy.root_limits.schema;
        admission.validate_typed_borrowed(value, expected)?;
        let mut entry_policy =
            crate::value::RuntimeRetainedValueAdmission::for_program(owner, entry_limits);
        entry_policy
            .validate_typed_borrowed(value, expected)
            .inspect_err(|error| admission.retain_failure(error.clone()))?;
        Ok(())
    }

    fn admit_optional_root(
        snapshot: Option<&Self>,
        active: &crate::entry::ActiveEntrySnapshotV1,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), RootStateSaveSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        admission.validate_metadata_collection(usize::from(snapshot.is_some()))?;
        let owner = admission
            .program_owner()
            .ok_or(RootRuntimeError::SnapshotRoleMismatch(
                "original program owner",
            ))?;
        let roles = RootStateSaveSnapshotV1::selected_entry_roles(active, owner)?;
        if snapshot.is_some() != roles.stateful().is_some() {
            return Err(RootRuntimeError::SnapshotRoleMismatch("root presence").into());
        }
        if let Some(snapshot) = snapshot {
            snapshot.admit_root(active, admission)?;
        }
        Ok(())
    }
}

// The public names select concrete storage roles; no private storage bound
// leaks through the API and a missing optional slot remains unambiguous.
macro_rules! root_snapshot_admission_api {
    ($snapshot:ty) => {
        impl $snapshot {
            /// Admits the complete root before projection or session restore.
            ///
            /// # Errors
            /// Rejects original entry/nominal/layout/type and resource failures.
            pub fn admit_retained(
                &self,
                active: &crate::entry::ActiveEntrySnapshotV1,
                admission: &mut crate::value::RuntimeRetainedValueAdmission,
            ) -> Result<(), RootStateSaveSnapshotError> {
                self.admit_root(active, admission)
            }

            /// Admits the selected entry's optional root, including presence.
            ///
            /// # Errors
            /// Also rejects a missing required root or a stateless entry's root.
            pub fn admit_optional_retained(
                snapshot: Option<&Self>,
                active: &crate::entry::ActiveEntrySnapshotV1,
                admission: &mut crate::value::RuntimeRetainedValueAdmission,
            ) -> Result<(), RootStateSaveSnapshotError> {
                Self::admit_optional_root(snapshot, active, admission)
            }
        }
    };
}
root_snapshot_admission_api!(RootStateSnapshotV1);
root_snapshot_admission_api!(RootStateSaveSnapshotV1);

impl RootStateSaveSnapshotV1 {
    pub(crate) fn from_runtime_image(
        root: &super::RootRuntimeRollbackImage,
    ) -> Result<Self, RootStateSaveSnapshotError> {
        let blockers = root.save_blockers();
        if blockers.reducer_active || blockers.pending_events != 0 || blockers.pending_commands != 0
        {
            return Err(RootStateSaveSnapshotError::NotQuiescent { blockers });
        }
        Ok(Self {
            state_identity: root.active.state_identity.clone(),
            state_layout: root.active.state_layout,
            event_identity: root.active.event_identity.clone(),
            event_layout: root.active.event_layout,
            phase: root.active.phase.clone(),
            next_sequence: root.active.next_sequence,
            failure: root.failure.clone(),
        })
    }

    pub(crate) fn into_runtime_image(
        self,
        active: &crate::entry::ActiveEntrySnapshotV1,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<super::RootRuntimeRollbackImage, RootStateSaveSnapshotError> {
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.admit_retained(active, &mut admission)?;
        let roles = Self::selected_roles(active, owner)?.clone();
        Ok(super::RootRuntime {
            active: super::ActiveRootState {
                entry: active.id.clone(),
                binding: active.binding,
                state_identity: self.state_identity,
                state_layout: self.state_layout,
                event_identity: self.event_identity,
                event_layout: self.event_layout,
                phase: self.phase,
                next_sequence: self.next_sequence,
                queued_events: Default::default(),
                committed_commands: Default::default(),
            },
            reducer: roles.reducer.clone(),
            limits: roles.command_policy.root_limits,
            roles,
            failure: self.failure,
        })
    }

    /// Borrows the existing inert state carrier for pre-construction catalog
    /// inspection. Selected-entry admission remains the original authority.
    #[must_use]
    pub fn retained_value(&self) -> Option<crate::value::RuntimeBorrowedValue<'_>> {
        self.phase.state_value().map(Into::into)
    }

    pub fn from_live(
        snapshot: &RootStateSnapshotV1,
        active: &crate::entry::ActiveEntrySnapshotV1,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, RootStateSaveSnapshotError> {
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        snapshot.admit_retained(active, &mut admission)?;
        Ok(Self {
            state_identity: snapshot.state_identity.clone(),
            state_layout: snapshot.state_layout,
            event_identity: snapshot.event_identity.clone(),
            event_layout: snapshot.event_layout,
            phase: snapshot.phase.try_map_ref(|payload| {
                crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                    payload.value(),
                    owner,
                )
            })?,
            next_sequence: snapshot.next_sequence,
            failure: snapshot.failure.clone(),
        })
    }

    pub fn into_live(
        self,
        active: &crate::entry::ActiveEntrySnapshotV1,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<RootStateSnapshotV1, RootStateSaveSnapshotError> {
        let roles = Self::selected_roles(active, owner)?;
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            roles.command_policy.root_limits.schema,
        );
        self.admit_retained(active, &mut admission)?;
        let phase = self
            .phase
            .try_map(|value| value.into_admitted_runtime_value(owner).map(RuntimePayload))?;
        Ok(RootStateSnapshotV1 {
            state_identity: self.state_identity,
            state_layout: self.state_layout,
            event_identity: self.event_identity,
            event_layout: self.event_layout,
            phase,
            next_sequence: self.next_sequence,
            failure: self.failure,
        })
    }

    pub(super) fn validate_header(
        state_identity: &crate::entry::RuntimeNominalTypeId,
        state_layout: TypeLayoutHash,
        event_identity: &crate::entry::RuntimeNominalTypeId,
        event_layout: TypeLayoutHash,
        roles: &RuntimeStatefulEntryRoles,
    ) -> Result<(), RootRuntimeError> {
        if !roles
            .state
            .matches_snapshot_header(state_identity, state_layout)
        {
            return Err(RootRuntimeError::SnapshotRoleMismatch("state"));
        }
        if !roles
            .event
            .matches_snapshot_header(event_identity, event_layout)
        {
            return Err(RootRuntimeError::SnapshotRoleMismatch("event"));
        }
        Ok(())
    }

    fn selected_roles<'a>(
        active: &crate::entry::ActiveEntrySnapshotV1,
        owner: &'a crate::task::RuntimeProgramOwner,
    ) -> Result<&'a RuntimeStatefulEntryRoles, RootRuntimeError> {
        Self::selected_entry_roles(active, owner)?
            .stateful()
            .ok_or(RootRuntimeError::SnapshotRoleMismatch("root presence"))
    }

    fn selected_entry_roles<'a>(
        active: &crate::entry::ActiveEntrySnapshotV1,
        owner: &'a crate::task::RuntimeProgramOwner,
    ) -> Result<&'a crate::entry::RuntimeEntryRoles, RootRuntimeError> {
        let entry = owner
            .types()
            .entry(&active.id)
            .ok_or_else(|| RootRuntimeError::MissingEntry(active.id.canonical_label()))?;
        if entry.binding() != active.binding || !entry.matches_kind(&active.kind) {
            return Err(RootRuntimeError::SnapshotRoleMismatch("active entry"));
        }
        Ok(entry.roles())
    }
}

pub(super) fn deserialize_required_failure<'de, D>(
    deserializer: D,
) -> Result<Option<super::RootRuntimeFailure>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(deserializer)
}
