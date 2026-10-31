//! Admission and inert projection of the actual resource owner.

use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimePayload,
    RuntimeRetainedValueAdmission, RuntimeStoredValue,
};

/// Reject duplicate identities before decoding their second owned body. Both
/// ledger indexes use this visitor under the enclosing bounded save decoder.
pub(super) fn deserialize_unique_map<'de, D, K, V>(
    deserializer: D,
) -> Result<BTreeMap<K, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
{
    struct UniqueMap<K, V>(std::marker::PhantomData<(K, V)>);

    impl<'de, K, V> serde::de::Visitor<'de> for UniqueMap<K, V>
    where
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
    {
        type Value = BTreeMap<K, V>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map with unique presentation identities")
        }

        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut input: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some(key) = input.next_key()? {
                if values.contains_key(&key) {
                    return Err(serde::de::Error::custom("duplicate presentation identity"));
                }
                values.insert(key, input.next_value()?);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_map(UniqueMap(std::marker::PhantomData))
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: crate::value::InertRuntimeValueStorage",
        deserialize = "V: crate::value::InertRuntimeValueStorage"
    )
)]
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
    pub(crate) fn into_admitted_live<'a>(
        self,
        context: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> Result<PresentationBody<RuntimePayload>, AwbcRuntimeValueSnapshotError> {
        Ok::<_, AwbcRuntimeValueSnapshotError>(PresentationBody {
            expected: self.expected,
            binding: self.binding,
            value: RuntimePayload(self.value.into_admitted_runtime_value(context.into())?),
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
        let result = (|| {
            let invalid =
                || AwbcRuntimeValueSnapshotError::from(RuntimePresentationError::InvalidState);
            if !self.program.same_program(owner)
                || self.state.artifact != artifact
                || self.state.leases.len() > self.state.limits.handles
                || self.state.scopes.len() > self.state.limits.scopes
            {
                return Err(invalid());
            }
            admission.validate_metadata(&(
                self.state.artifact,
                self.state.next,
                self.state.next_scope,
                self.state.limits.handles,
                self.state.limits.scopes,
            ))?;
            admission.validate_metadata_collection(self.state.scopes.len())?;
            let issued_scope = self.state.next_scope.last_issued();
            for (scope, (execution, fiber, anchor)) in &self.state.scopes {
                admission.validate_metadata(&(scope, execution, fiber, anchor))?;
                if issued_scope.is_none_or(|last| scope.get() > last) {
                    return Err(invalid());
                }
            }
            admission.validate_metadata_collection(self.state.leases.len())?;
            let mut occupied = std::collections::BTreeSet::new();
            for (issuance, lease) in &self.state.leases {
                admission.validate_metadata(&(issuance, &lease.token))?;
                admission.validate_metadata(&lease.site)?;
                if let PresentationRegistrationSite::Slot(address) = &lease.site {
                    address.validate()?;
                }
                if let Some(owner) = lease.owner {
                    admission.validate_metadata(&(owner.execution, owner.fiber, owner.scope))?;
                }
                if lease.token.issuance() != *issuance
                    || lease.token.artifact() != &self.state.artifact
                    || self
                        .state
                        .next
                        .last_issued()
                        .is_none_or(|last| *issuance > last)
                    || lease.token.resource() != lease.site.resource()
                    || lease
                        .owner
                        .is_some_and(|owner| self.validate_scope(owner).is_err())
                {
                    return Err(invalid());
                }
                if lease.body.is_some() {
                    if let PresentationRegistrationSite::Slot(address) = &lease.site {
                        if !occupied.insert(address) {
                            return Err(invalid());
                        }
                    }
                }
            }
            Ok(PresentationAdmission {
                program: self.program.clone(),
                unseen: self.state.scopes.keys().copied().collect(),
                scopes: self.state.scopes.clone(),
                handles: self
                    .state
                    .leases
                    .values()
                    .map(|lease| lease.token.clone())
                    .collect(),
                seen_handles: Default::default(),
                detached: self
                    .state
                    .leases
                    .values()
                    .filter(|lease| lease.owner.is_none())
                    .map(|lease| lease.token.clone())
                    .collect(),
            })
        })();
        result.inspect_err(|error: &AwbcRuntimeValueSnapshotError| {
            admission.retain_failure(error.clone());
        })
    }

    pub(crate) fn admit_bodies(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        for lease in self.state.leases.values() {
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
            state: self.state.clone(),
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
            .state
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
                            .map(|body| body.snapshot_admitted(&self.program))
                            .transpose()?,
                    },
                ))
            })
            .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?;
        Ok(PresentationLedger {
            program: self.program.clone(),
            state: PresentationLedgerState {
                artifact: self.state.artifact,
                next: self.state.next,
                next_scope: self.state.next_scope,
                scopes: self.state.scopes.clone(),
                limits: self.state.limits,
                leases,
            },
        })
    }
}

impl PresentationLedger<PresentationBody<AwbcRuntimeValueSnapshot>> {
    #[cfg(test)]
    pub(crate) fn inert_state(
        &self,
    ) -> &PresentationLedgerState<PresentationBody<AwbcRuntimeValueSnapshot>> {
        &self.state
    }

    pub(crate) fn into_admitted_live<'a>(
        self,
        context: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> Result<PresentationLedger<PresentationBody<RuntimePayload>>, AwbcRuntimeValueSnapshotError>
    {
        let context = context.into();
        if !self.program.same_program(context.owner()) {
            return Err(RuntimePresentationError::Program.into());
        }
        let leases = self
            .state
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
                            .map(|body| body.into_admitted_live(context))
                            .transpose()?,
                    },
                ))
            })
            .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?;
        Ok(PresentationLedger {
            program: self.program,
            state: PresentationLedgerState {
                artifact: self.state.artifact,
                next: self.state.next,
                next_scope: self.state.next_scope,
                scopes: self.state.scopes,
                limits: self.state.limits,
                leases,
            },
        })
    }
}

#[cfg(test)]
impl PresentationLedgerState<PresentationBody<AwbcRuntimeValueSnapshot>> {
    /// Retains the caller's selected lease with inert storage. Complete Product
    /// admission must still verify every header before any live materialization.
    pub(crate) fn retain_program(
        self,
        program: RuntimeProgramOwner,
    ) -> PresentationLedger<PresentationBody<AwbcRuntimeValueSnapshot>> {
        PresentationLedger {
            program,
            state: self,
        }
    }
}
