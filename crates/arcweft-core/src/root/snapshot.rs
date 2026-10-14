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

// Live and inert roots have one metadata/role grammar. Both the enclosing
// group's aggregate limit and the selected entry's own limit use the existing
// Core value admission authority, before either representation is copied.
macro_rules! admit_root_snapshot {
    ($snapshot:ty, $validate:ident, $origin:ident, $($value:tt)+) => {
        impl $snapshot {
            /// Admits this borrowed root under the enclosing group's original
            /// selected entry before projection, activation or session restore.
            ///
            /// # Errors
            /// Rejects incompatible entry/nominal/layout metadata, noncopyable
            /// or mistyped values, and enclosing or entry-policy limit failures.
            pub fn admit_retained(
                &self,
                active: &crate::entry::ActiveEntrySnapshotV1,
                admission: &mut crate::value::RuntimeRetainedValueAdmission,
            ) -> Result<(), RootStateSaveSnapshotError> {
                admission.validate_metadata_string(self.state_identity.as_str())?;
                admission.validate_metadata_string(self.event_identity.as_str())?;
                let owner = admission.program_owner().cloned().ok_or(
                    RootRuntimeError::SnapshotRoleMismatch("original program owner"),
                )?;
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
                let value = &self.$($value)+;
                admission.$validate(value, roles.state.semantic_identity)?;
                let expected = roles.state.semantic_identity;
                let entry_limits = roles.command_policy.root_limits.schema;
                let mut entry_policy = crate::value::RuntimeRetainedValueAdmission::for_program(
                    owner.clone(),
                    entry_limits,
                );
                entry_policy
                    .$validate(value, expected)
                    .inspect_err(|error| admission.retain_failure(error.clone()))?;
                owner.types()
                    .accepts_value_view(expected, value.view(), entry_limits,
                        crate::entry::schema::value_encoding::ValueAdmissionOrigin::$origin)
                    .map_err(crate::value::AwbcRuntimeValueSnapshotError::from)
                    .inspect_err(|error| admission.retain_failure(error.clone()))?;
                Ok(())
            }

            /// Validates the selected entry's complete optional root slot.
            ///
            /// # Errors
            /// Also rejects a missing required root or a root on a stateless
            /// entry before the enclosing session copies or activates a prefix.
            pub fn admit_optional_retained(
                snapshot: Option<&Self>,
                active: &crate::entry::ActiveEntrySnapshotV1,
                admission: &mut crate::value::RuntimeRetainedValueAdmission,
            ) -> Result<(), RootStateSaveSnapshotError> {
                admission.validate_metadata_collection(usize::from(snapshot.is_some()))?;
                let owner = admission.program_owner().ok_or(
                    RootRuntimeError::SnapshotRoleMismatch("original program owner"),
                )?;
                let roles = RootStateSaveSnapshotV1::selected_entry_roles(active, owner)?;
                if snapshot.is_some() != roles.stateful().is_some() {
                    return Err(RootRuntimeError::SnapshotRoleMismatch("root presence").into());
                }
                if let Some(snapshot) = snapshot {
                    snapshot.admit_retained(active, admission)?;
                }
                Ok(())
            }
        }
    };
}

admit_root_snapshot!(RootStateSnapshotV1, validate_typed, Live, value.0);
admit_root_snapshot!(
    RootStateSaveSnapshotV1,
    validate_typed_snapshot,
    InertSnapshot,
    value
);

impl RootStateSaveSnapshotV1 {
    /// Borrows the existing inert state carrier for pre-construction catalog
    /// inspection. Selected-entry admission remains the original authority.
    #[must_use]
    pub fn retained_value(&self) -> crate::value::RuntimeBorrowedValue<'_> {
        (&self.value).into()
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
            value: crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
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
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            roles.command_policy.root_limits.schema,
        );
        self.admit_retained(active, &mut admission)?;
        let value = RuntimePayload(self.value.into_admitted_runtime_value(owner)?);
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
        Self::selected_entry_roles(active, owner)?
            .stateful()
            .ok_or(RootRuntimeError::SnapshotRoleMismatch("root presence"))
    }

    fn selected_entry_roles<'a>(
        active: &crate::entry::ActiveEntrySnapshotV1,
        owner: &'a crate::task::RuntimeProgramOwner,
    ) -> Result<&'a crate::entry::RuntimeEntryRoles, RootRuntimeError> {
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
        Ok(roles)
    }
}
