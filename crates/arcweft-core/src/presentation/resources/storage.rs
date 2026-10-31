//! Admission and inert projection of the actual resource owner.

use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimePayload,
    RuntimeRetainedValueAdmission, RuntimeStoredValue,
};

#[derive(Debug, PartialEq)]
pub(crate) struct PresentationBody<V> {
    pub expected: RuntimeSemanticTypeId,
    pub binding: Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
    pub value: V,
}

impl<V: RuntimeStoredValue> PresentationBody<V> {
    pub(crate) fn admit_retained(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&(self.expected, &self.binding))?;
        admission.validate_owned_typed_borrowed_in_invocation(
            self.value.borrowed_value(),
            self.expected,
            self.binding.as_deref(),
        )
    }
}

impl Clone for PresentationBody<AwbcRuntimeValueSnapshot> {
    fn clone(&self) -> Self {
        Self {
            expected: self.expected,
            binding: self.binding.clone(),
            value: self.value.clone(),
        }
    }
}

impl PresentationBody<RuntimePayload> {
    pub(crate) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<PresentationBody<AwbcRuntimeValueSnapshot>, AwbcRuntimeValueSnapshotError> {
        Ok::<_, AwbcRuntimeValueSnapshotError>(PresentationBody {
            expected: self.expected,
            binding: self.binding.clone(),
            value: AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                self.value.value(),
                owner,
            )?,
        })
    }
}

impl PresentationBody<AwbcRuntimeValueSnapshot> {
    pub(crate) fn into_admitted_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<PresentationBody<RuntimePayload>, AwbcRuntimeValueSnapshotError> {
        Ok::<_, AwbcRuntimeValueSnapshotError>(PresentationBody {
            expected: self.expected,
            binding: self.binding,
            value: RuntimePayload(self.value.into_admitted_runtime_value(owner)?),
        })
    }
}

impl Clone for PresentationRetirement<PresentationBody<AwbcRuntimeValueSnapshot>> {
    fn clone(&self) -> Self {
        Self {
            token: self.token.clone(),
            body: self.body.clone(),
        }
    }
}

/// One admission contour created after all ledger headers pass the original
/// bank, before their bodies are traversed. Observing each owning value consumes
/// its occurrence here; the contour cannot issue grants or live handles.
pub(crate) struct PresentationAdmission {
    program: RuntimeProgramOwner,
    scopes: BTreeMap<
        RuntimeScopeInstanceId,
        (
            ExecutionInstanceId,
            RuntimePersistentFiberId,
            PresentationScopeAnchor,
        ),
    >,
    unseen: std::collections::BTreeSet<RuntimeScopeInstanceId>,
    handles: std::collections::BTreeSet<RuntimePresentationHandleToken>,
    seen_handles: std::collections::BTreeSet<RuntimePresentationHandleToken>,
    detached: std::collections::BTreeSet<RuntimePresentationHandleToken>,
}

impl PresentationAdmission {
    pub(crate) fn admit_handle(
        &mut self,
        token: &RuntimePresentationHandleToken,
    ) -> Result<(), crate::entry::RuntimeSchemaError> {
        use crate::entry::RuntimeSchemaError;
        if !self.handles.contains(token) {
            return Err(RuntimeSchemaError::PresentationHandleCustody {
                path: String::new(),
                token: token.clone(),
                duplicate: false,
            });
        }
        if !self.seen_handles.insert(token.clone()) {
            return Err(RuntimeSchemaError::PresentationHandleCustody {
                path: String::new(),
                token: token.clone(),
                duplicate: true,
            });
        }
        Ok(())
    }

    pub(crate) fn include_retirement(&mut self, token: &RuntimePresentationHandleToken) {
        self.handles.insert(token.clone());
    }

    pub(crate) fn accepts(&mut self, binding: &RuntimePresentationScopeBinding) -> bool {
        self.accepts_scope(&binding.program, binding.owner, binding.anchor)
    }

    pub(crate) fn accepts_snapshot(
        &mut self,
        program: &RuntimeProgramOwner,
        snapshot: &super::RuntimePresentationScopeSnapshot,
    ) -> bool {
        self.accepts_scope(program, snapshot.owner, snapshot.anchor)
    }

    fn accepts_scope(
        &mut self,
        program: &RuntimeProgramOwner,
        owner: PresentationScopeOwner,
        anchor: PresentationScopeAnchor,
    ) -> bool {
        self.program.same_program(program)
            && self.scopes.get(&owner.scope) == Some(&(owner.execution, owner.fiber, anchor))
            && self.unseen.remove(&owner.scope)
    }

    pub(crate) fn complete(&self) -> bool {
        self.unseen.is_empty() && self.detached.is_subset(&self.seen_handles)
    }
}

impl<V: RuntimeStoredValue> PresentationLedger<PresentationBody<V>> {
    pub(crate) fn admit_ownership(
        &self,
        owner: &RuntimeProgramOwner,
        artifact: RuntimeArtifactFingerprint,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<PresentationAdmission, AwbcRuntimeValueSnapshotError> {
        let invalid =
            || AwbcRuntimeValueSnapshotError::from(RuntimePresentationError::InvalidState);
        if !self.program.same_program(owner)
            || self.artifact != artifact
            || self.leases.len() > self.limits.handles
            || self.scopes.len() > self.limits.scopes
        {
            return Err(invalid());
        }
        admission.validate_metadata(&(
            self.artifact,
            self.next,
            self.next_scope,
            self.limits.handles,
            self.limits.scopes,
        ))?;
        admission.validate_metadata_collection(self.scopes.len())?;
        let issued_scope = self.next_scope.last_issued();
        for (scope, (execution, fiber, anchor)) in &self.scopes {
            admission.validate_metadata(&(scope, execution, fiber, anchor))?;
            if issued_scope.is_none_or(|last| scope.get() > last) {
                return Err(invalid());
            }
        }
        admission.validate_metadata_collection(self.leases.len())?;
        let mut occupied = std::collections::BTreeSet::new();
        for (issuance, lease) in &self.leases {
            admission.validate_metadata(&(issuance, &lease.token))?;
            if let Some(owner) = lease.owner {
                admission.validate_metadata(&(owner.execution, owner.fiber, owner.scope))?;
            }
            if lease.token.issuance() != *issuance
                || lease.token.artifact() != &self.artifact
                || self.next.last_issued().is_none_or(|last| *issuance > last)
                || lease.token.resource() != lease.site.resource()
                || lease
                    .owner
                    .is_some_and(|owner| self.validate_scope(owner).is_err())
            {
                return Err(invalid());
            }
            if lease.body.is_some() {
                if let PresentationRegistrationSite::Slot(address) = &lease.site {
                    admission
                        .validate_metadata(&(address.target.as_str(), address.slot.as_str()))?;
                    if !occupied.insert(address) {
                        return Err(invalid());
                    }
                }
            }
        }
        Ok(PresentationAdmission {
            program: self.program.clone(),
            unseen: self.scopes.keys().copied().collect(),
            scopes: self.scopes.clone(),
            handles: self
                .leases
                .values()
                .map(|lease| lease.token.clone())
                .collect(),
            seen_handles: Default::default(),
            detached: self
                .leases
                .values()
                .filter(|lease| lease.owner.is_none())
                .map(|lease| lease.token.clone())
                .collect(),
        })
    }

    pub(crate) fn admit_bodies(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        for lease in self.leases.values() {
            if let Some(body) = &lease.body {
                body.admit_retained(admission)?;
            }
        }
        Ok(())
    }
}

impl Clone for PresentationLedger<PresentationBody<AwbcRuntimeValueSnapshot>> {
    fn clone(&self) -> Self {
        Self {
            program: self.program.clone(),
            artifact: self.artifact,
            next: self.next,
            next_scope: self.next_scope,
            scopes: self.scopes.clone(),
            limits: self.limits,
            leases: self
                .leases
                .iter()
                .map(|(id, lease)| {
                    (
                        *id,
                        PresentationLease {
                            token: lease.token.clone(),
                            owner: lease.owner,
                            site: lease.site.clone(),
                            body: lease.body.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
}

impl PresentationLedger<PresentationBody<RuntimePayload>> {
    pub(crate) fn snapshot_admitted(
        &self,
    ) -> Result<
        PresentationLedger<PresentationBody<AwbcRuntimeValueSnapshot>>,
        AwbcRuntimeValueSnapshotError,
    > {
        let leases = self
            .leases
            .iter()
            .map(|(id, lease)| {
                Ok((
                    *id,
                    PresentationLease {
                        token: lease.token.clone(),
                        owner: lease.owner,
                        site: lease.site.clone(),
                        body: lease
                            .body
                            .as_ref()
                            .map(|body| {
                                Ok::<_, AwbcRuntimeValueSnapshotError>(PresentationBody {
                                    expected: body.expected,
                                    binding: body.binding.clone(),
                                    value: AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                                        body.value.value(),
                                        &self.program,
                                    )?,
                                })
                            })
                            .transpose()?,
                    },
                ))
            })
            .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?;
        Ok(PresentationLedger {
            program: self.program.clone(),
            artifact: self.artifact,
            next: self.next,
            next_scope: self.next_scope,
            scopes: self.scopes.clone(),
            limits: self.limits,
            leases,
        })
    }
}

impl PresentationLedger<PresentationBody<AwbcRuntimeValueSnapshot>> {
    pub(crate) fn into_admitted_live(
        self,
    ) -> Result<PresentationLedger<PresentationBody<RuntimePayload>>, AwbcRuntimeValueSnapshotError>
    {
        let leases = self
            .leases
            .into_iter()
            .map(|(id, lease)| {
                Ok((
                    id,
                    PresentationLease {
                        token: lease.token,
                        owner: lease.owner,
                        site: lease.site,
                        body: lease
                            .body
                            .map(|body| {
                                Ok::<_, AwbcRuntimeValueSnapshotError>(PresentationBody {
                                    expected: body.expected,
                                    binding: body.binding,
                                    value: RuntimePayload(
                                        body.value.into_admitted_runtime_value(&self.program)?,
                                    ),
                                })
                            })
                            .transpose()?,
                    },
                ))
            })
            .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?;
        Ok(PresentationLedger {
            program: self.program,
            artifact: self.artifact,
            next: self.next,
            next_scope: self.next_scope,
            scopes: self.scopes,
            limits: self.limits,
            leases,
        })
    }
}
