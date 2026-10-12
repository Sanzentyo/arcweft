//! Program-bound durable root snapshot wire and exact selected-entry admission.

use super::{
    RootRuntimeError, RootStateSnapshotV1, RuntimePayload, RuntimeStatefulEntryRoles,
    TransitionSequence, TypeLayoutHash,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Inert version-one wire projection of a durable root state.
/// Values are admitted through the selected program before live publication.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RootStateSaveSnapshotV1 {
    state_identity: crate::entry::RuntimeNominalTypeId,
    state_layout: TypeLayoutHash,
    event_identity: crate::entry::RuntimeNominalTypeId,
    event_layout: TypeLayoutHash,
    value: crate::value::AwbcRuntimeValueSnapshot,
    next_sequence: TransitionSequence,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RootStateSaveSnapshotError {
    #[error(transparent)]
    Root(#[from] RootRuntimeError),
    #[error(transparent)]
    Value(#[from] crate::value::AwbcRuntimeValueSnapshotError),
}

impl RootStateSaveSnapshotV1 {
    pub fn from_live(
        snapshot: &RootStateSnapshotV1,
        active: &crate::entry::ActiveEntrySnapshotV1,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, RootStateSaveSnapshotError> {
        let roles = Self::selected_roles(active, owner)?;
        roles
            .event
            .validate_for_program(owner.types())
            .map_err(RootRuntimeError::InvalidSnapshotValue)?;
        Self::validate_header(
            &snapshot.state_identity,
            snapshot.state_layout,
            &snapshot.event_identity,
            snapshot.event_layout,
            roles,
        )?;
        roles
            .state
            .accepts_payload(
                owner.types(),
                &snapshot.value,
                roles.command_policy.root_limits.schema,
            )
            .map_err(RootRuntimeError::InvalidSnapshotValue)?;
        Ok(Self {
            state_identity: snapshot.state_identity.clone(),
            state_layout: snapshot.state_layout,
            event_identity: snapshot.event_identity.clone(),
            event_layout: snapshot.event_layout,
            value:
                crate::value::AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(
                    snapshot.value.value(),
                    owner,
                )?,
            next_sequence: snapshot.next_sequence,
        })
    }

    pub fn into_live(
        self,
        active: &crate::entry::ActiveEntrySnapshotV1,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<RootStateSnapshotV1, RootStateSaveSnapshotError> {
        let roles = Self::selected_roles(active, owner)?;
        roles
            .event
            .validate_for_program(owner.types())
            .map_err(RootRuntimeError::InvalidSnapshotValue)?;
        Self::validate_header(
            &self.state_identity,
            self.state_layout,
            &self.event_identity,
            self.event_layout,
            roles,
        )?;
        let value = RuntimePayload(
            self.value
                .into_unrestricted_runtime_value_for_program_with_limits(
                    owner,
                    roles.command_policy.root_limits.schema,
                )?,
        );
        roles
            .state
            .accepts_payload(
                owner.types(),
                &value,
                roles.command_policy.root_limits.schema,
            )
            .map_err(RootRuntimeError::InvalidSnapshotValue)?;
        Ok(RootStateSnapshotV1 {
            state_identity: self.state_identity,
            state_layout: self.state_layout,
            event_identity: self.event_identity,
            event_layout: self.event_layout,
            value,
            next_sequence: self.next_sequence,
        })
    }

    fn validate_header(
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
        let roles = match owner {
            crate::task::RuntimeProgramOwner::Plan(plan) => {
                let entry = plan
                    .entries()
                    .iter()
                    .find(|entry| entry.id == active.id)
                    .ok_or_else(|| RootRuntimeError::MissingEntry(active.id.canonical_label()))?;
                if entry.kind != active.kind || entry.binding != active.binding {
                    return Err(RootRuntimeError::SnapshotRoleMismatch("active entry"));
                }
                &entry.roles
            }
            crate::task::RuntimeProgramOwner::Awbc(program) => {
                let entry = program
                    .entries
                    .iter()
                    .find(|entry| entry.runtime_id == active.id)
                    .ok_or_else(|| RootRuntimeError::MissingEntry(active.id.canonical_label()))?;
                if entry.kind.runtime_kind(&program.strings).as_ref() != Some(&active.kind)
                    || entry.binding != active.binding
                {
                    return Err(RootRuntimeError::SnapshotRoleMismatch("active entry"));
                }
                &entry.roles
            }
        };
        roles
            .stateful()
            .ok_or(RootRuntimeError::SnapshotRoleMismatch("root presence"))
    }
}
