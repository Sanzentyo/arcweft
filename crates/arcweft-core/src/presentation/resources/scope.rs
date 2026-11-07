//! Resource grants of an actual execution scope, across issuing banks.

use super::*;

/// Dynamic owner, never a scope name or a reusable static instruction coordinate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PresentationScopeOwner {
    pub(super) bank: RuntimePresentationBankId,
    pub(super) execution: ExecutionInstanceId,
    pub(super) fiber: RuntimePersistentFiberId,
    pub(super) scope: RuntimeScopeInstanceId,
}

/// A native grant remains private with its original lexical storage. Public
/// AWBC scope images additionally bind the original dynamic frame and its
/// verified lexical declaration, so exchanging two headers cannot move custody.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all = "snake_case")]
pub(crate) enum PresentationScopeAnchor {
    Native,
    Awbc {
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    },
}

pub(crate) trait PresentationScopeStorage {
    fn admit_awbc_scope(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        fiber: crate::runtime_id::RuntimeFiberInstanceId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>;
}

impl PresentationScopeOwner {
    pub(super) fn admit_metadata(
        &self,
        anchor: &PresentationScopeAnchor,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&(self, anchor))
    }

    fn validate_awbc_anchor(
        self,
        anchor: PresentationScopeAnchor,
        fiber: crate::runtime_id::RuntimeFiberInstanceId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Result<(), RuntimePresentationError> {
        if self.execution.get() != fiber.get()
            || self.fiber.get() != fiber.get().get()
            || anchor != (PresentationScopeAnchor::Awbc { frame, scope })
        {
            return Err(RuntimePresentationError::Owner);
        }
        Ok(())
    }
}

/// An inert reference to the resource grant of one reached lexical frame.
/// The ledger, rather than this retained reference, owns its active lifetime.
/// Its program lease is retained only in memory; wire input cannot issue it.
///
/// ```compile_fail
/// use arcweft_core::presentation::RuntimePresentationScopeBinding;
/// let _: RuntimePresentationScopeBinding = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_core::presentation::RuntimePresentationScopeBinding;
/// fn encode(binding: &RuntimePresentationScopeBinding) { let _ = serde_json::to_vec(binding); }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePresentationScopeBinding {
    pub(super) program: RuntimeProgramOwner,
    pub(super) grants: BTreeMap<RuntimePresentationBankId, PresentationScopeGrant>,
}

/// One issuing bank's grant held at the enclosing physical scope. A scope may
/// receive resources from several banks without renaming their original tokens.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PresentationScopeGrant {
    pub(super) owner: PresentationScopeOwner,
    pub(super) anchor: PresentationScopeAnchor,
}

/// Inert coordinates of one reached scope. Decode does not issue a grant;
/// restoration requires its original program and the enclosing custody bank.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePresentationScopeSnapshot {
    #[serde(with = "super::bank_rows")]
    pub(super) grants: BTreeMap<RuntimePresentationBankId, PresentationScopeGrant>,
}

impl RuntimePresentationScopeBinding {
    pub(crate) fn owns(&self, owner: PresentationScopeOwner) -> bool {
        self.grants
            .get(&owner.bank)
            .is_some_and(|grant| grant.owner == owner)
    }

    pub(crate) fn banks(&self) -> impl Iterator<Item = RuntimePresentationBankId> + '_ {
        self.grants.keys().copied()
    }

    pub(crate) fn snapshot_admitted(
        &self,
        program: &RuntimeProgramOwner,
    ) -> Result<RuntimePresentationScopeSnapshot, RuntimePresentationError> {
        if !self.program.same_program(program) {
            return Err(RuntimePresentationError::Program);
        }
        Ok(RuntimePresentationScopeSnapshot {
            grants: self.grants.clone(),
        })
    }
}

impl RuntimePresentationScopeSnapshot {
    pub(crate) fn into_admitted_live(
        self,
        program: &RuntimeProgramOwner,
    ) -> RuntimePresentationScopeBinding {
        RuntimePresentationScopeBinding {
            program: program.clone(),
            grants: self.grants,
        }
    }

    pub(crate) fn scope_keys(
        &self,
    ) -> impl Iterator<
        Item = (
            ExecutionInstanceId,
            RuntimePersistentFiberId,
            RuntimeScopeInstanceId,
        ),
    > + '_ {
        self.grants
            .values()
            .map(|grant| (grant.owner.execution, grant.owner.fiber, grant.owner.scope))
    }

    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admit_grants(&self.grants, admission)?;
        admission.validate_presentation_scope_snapshot(self)
    }
}

impl PresentationScopeStorage for RuntimePresentationScopeBinding {
    fn admit_awbc_scope(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        fiber: crate::runtime_id::RuntimeFiberInstanceId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        for grant in self.grants.values() {
            grant
                .owner
                .validate_awbc_anchor(grant.anchor, fiber, frame, scope)?;
        }
        self.admit_retained(admission)
    }
}

impl PresentationScopeStorage for RuntimePresentationScopeSnapshot {
    fn admit_awbc_scope(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        fiber: crate::runtime_id::RuntimeFiberInstanceId,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        for grant in self.grants.values() {
            grant
                .owner
                .validate_awbc_anchor(grant.anchor, fiber, frame, scope)?;
        }
        self.admit_retained(admission)
    }
}

/// Exclusive access to the grant slot of one reached execution scope. Resolving
/// the real frame may fail before this capability exists; installation after
/// ledger admission is infallible and cannot expose an unregistered grant.
pub(crate) struct PresentationScopeTarget<'scope> {
    binding: &'scope mut Option<RuntimePresentationScopeBinding>,
    pub(super) anchor: PresentationScopeAnchor,
}

/// Borrowed grant at its actual storage anchor; it cannot issue a live grant.
pub(crate) struct PresentationScopeRef<'scope> {
    pub(super) binding: Option<&'scope RuntimePresentationScopeBinding>,
    pub(super) anchor: PresentationScopeAnchor,
}

impl<'scope> PresentationScopeRef<'scope> {
    pub(crate) fn awbc(
        binding: Option<&'scope RuntimePresentationScopeBinding>,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Self {
        Self {
            binding,
            anchor: PresentationScopeAnchor::Awbc { frame, scope },
        }
    }
}

/// One real execution's scope in a possibly multi-fiber retirement contour.
pub(crate) struct PresentationScopeExit<S> {
    pub(crate) execution: ExecutionInstanceId,
    pub(crate) fiber: RuntimePersistentFiberId,
    pub(crate) scope: S,
}

impl<'scope> PresentationScopeTarget<'scope> {
    pub(super) fn reborrow(&mut self) -> PresentationScopeTarget<'_> {
        PresentationScopeTarget {
            binding: &mut *self.binding,
            anchor: self.anchor,
        }
    }

    pub(crate) fn as_ref(&self) -> PresentationScopeRef<'_> {
        PresentationScopeRef {
            binding: self.binding(),
            anchor: self.anchor,
        }
    }
    pub(crate) const fn native(
        binding: &'scope mut Option<RuntimePresentationScopeBinding>,
    ) -> Self {
        Self {
            binding,
            anchor: PresentationScopeAnchor::Native,
        }
    }

    pub(crate) const fn awbc(
        binding: &'scope mut Option<RuntimePresentationScopeBinding>,
        frame: crate::runtime_id::RuntimeFrameInstanceId,
        scope: Option<crate::awbc::schema::AwbcScopeId>,
    ) -> Self {
        Self {
            binding,
            anchor: PresentationScopeAnchor::Awbc { frame, scope },
        }
    }

    pub(super) fn binding(&self) -> Option<&RuntimePresentationScopeBinding> {
        self.binding.as_ref()
    }

    pub(super) fn insert(&mut self, program: &RuntimeProgramOwner, grant: PresentationScopeGrant) {
        let binding = self
            .binding
            .get_or_insert_with(|| RuntimePresentationScopeBinding {
                program: program.clone(),
                grants: BTreeMap::new(),
            });
        assert!(
            binding.program.same_program(program),
            "scope grant program was admitted"
        );
        binding.grants.insert(grant.owner.bank, grant);
    }

    pub(super) fn remove(&mut self, bank: RuntimePresentationBankId) {
        if let Some(binding) = self.binding {
            binding.grants.remove(&bank);
            if binding.grants.is_empty() {
                *self.binding = None;
            }
        }
    }
}

impl RuntimePresentationScopeBinding {
    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        if admission
            .program_owner()
            .is_none_or(|owner| !owner.same_program(&self.program))
        {
            return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: "presentation scope requires its original executable lease".into(),
            });
        }
        admit_grants(&self.grants, admission)?;
        admission.validate_presentation_scope(self)
    }
}

impl RuntimePresentationScopeBinding {
    pub(crate) fn matches_context(
        &self,
        program: &RuntimeProgramOwner,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
    ) -> bool {
        self.program.same_program(program)
            && !self.grants.is_empty()
            && self.grants.values().all(|grant| {
                grant.owner.execution == execution
                    && grant.owner.fiber == fiber
                    && grant.anchor == PresentationScopeAnchor::Native
            })
    }
    pub(crate) fn scope_keys(
        &self,
    ) -> impl Iterator<
        Item = (
            ExecutionInstanceId,
            RuntimePersistentFiberId,
            RuntimeScopeInstanceId,
        ),
    > + '_ {
        self.grants
            .values()
            .map(|grant| (grant.owner.execution, grant.owner.fiber, grant.owner.scope))
    }
}

fn admit_grants(
    grants: &BTreeMap<RuntimePresentationBankId, PresentationScopeGrant>,
    admission: &mut crate::value::RuntimeRetainedValueAdmission,
) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
    let result = (|| {
        admission.validate_metadata_collection(grants.len())?;
        if grants.is_empty() {
            return Err(RuntimePresentationError::InvalidState.into());
        }
        for (bank, grant) in grants {
            admission.validate_metadata(bank)?;
            grant.owner.admit_metadata(&grant.anchor, admission)?;
            if *bank != grant.owner.bank {
                return Err(RuntimePresentationError::Owner.into());
            }
        }
        Ok(())
    })();
    result.inspect_err(|error: &crate::value::AwbcRuntimeValueSnapshotError| {
        admission.retain_failure(error.clone())
    })
}
