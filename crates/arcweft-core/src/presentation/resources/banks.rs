//! One custody owner for all execution banks and their pending retirements.

use super::*;
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimePayload,
    RuntimeRetainedValueAdmission, RuntimeStoredValue,
};

/// Bank cursors survive empty scopes and dialogue completion. Recreating a bank
/// would let an old token alias a newly issued resource with the same cursor.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "L: Serialize, V: Serialize",
        deserialize = "L: Deserialize<'de>, V: Deserialize<'de>"
    )
)]
pub(crate) struct PresentationBanks<V, L = PresentationLedger<V>> {
    #[serde(with = "bank_rows")]
    ledgers: BTreeMap<RuntimePresentationBankId, L>,
    pub(super) retirements: Vec<PresentationRetirement<V>>,
}

/// The same owning grammar with inert ledger storage and no program or policy
/// authority. Only this storage specialization has a wire reader.
pub(crate) type PresentationBanksSaveSnapshot = PresentationBanks<
    PresentationBody<AwbcRuntimeValueSnapshot>,
    PresentationLedgerState<PresentationBody<AwbcRuntimeValueSnapshot>>,
>;

impl<V> PresentationBanks<V> {
    pub(crate) fn new(
        program: RuntimeProgramOwner,
        artifact: RuntimeArtifactFingerprint,
        limits: PresentationLedgerLimits,
    ) -> Self {
        Self {
            ledgers: std::iter::once((
                RuntimePresentationBankId::Root,
                PresentationLedger::new(program, artifact, RuntimePresentationBankId::Root, limits),
            ))
            .collect(),
            retirements: Vec::new(),
        }
    }

    pub(crate) fn ledger_mut(
        &mut self,
        bank: RuntimePresentationBankId,
    ) -> Result<&mut PresentationLedger<V>, RuntimePresentationError> {
        let root = self
            .ledgers
            .get(&RuntimePresentationBankId::Root)
            .ok_or(RuntimePresentationError::InvalidState)?;
        if !bank.matches_artifact(root.state.artifact) {
            return Err(RuntimePresentationError::Program);
        }
        if !self.ledgers.contains_key(&bank) && self.ledgers.len() >= root.limits.scopes {
            return Err(RuntimePresentationError::ScopeCapacity);
        }
        let program = root.program.clone();
        let artifact = root.state.artifact;
        let limits = root.limits;
        Ok(self
            .ledgers
            .entry(bank)
            .or_insert_with(|| PresentationLedger::new(program, artifact, bank, limits)))
    }

    pub(crate) fn preflight_register(
        &self,
        program: &RuntimeProgramOwner,
    ) -> Result<(), RuntimePresentationError> {
        let root = self
            .ledgers
            .get(&RuntimePresentationBankId::Root)
            .ok_or(RuntimePresentationError::InvalidState)?;
        root.validate_program(program)?;
        if self
            .ledgers
            .values()
            .map(|ledger| ledger.state.leases.len())
            .try_fold(0usize, usize::checked_add)
            .is_none_or(|count| count >= root.limits.handles)
        {
            return Err(RuntimePresentationError::Capacity);
        }
        Ok(())
    }

    pub(crate) fn retire_bound_scope(
        &mut self,
        program: &RuntimeProgramOwner,
        execution: ExecutionInstanceId,
        fiber: RuntimePersistentFiberId,
        target: PresentationScopeTarget<'_>,
    ) -> Result<(), RuntimePresentationError> {
        let Some(binding) = target.binding() else {
            return Ok(());
        };
        let ledger = self
            .ledgers
            .get_mut(&binding.owner.bank)
            .ok_or(RuntimePresentationError::Owner)?;
        let count = ledger.bound_scope_body_count(Some(binding));
        if self
            .retirements
            .len()
            .checked_add(count)
            .is_none_or(|count| count > ledger.limits.handles)
        {
            return Err(RuntimePresentationError::RetirementCapacity);
        }
        self.retirements
            .extend(ledger.exit_bound_scope(program, execution, fiber, target)?);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn retirements(&self) -> &[PresentationRetirement<V>] {
        &self.retirements
    }

    #[cfg(test)]
    pub(crate) fn retirements_mut(&mut self) -> &mut Vec<PresentationRetirement<V>> {
        &mut self.retirements
    }
}

impl<V: RuntimeStoredValue> PresentationBanks<PresentationBody<V>> {
    /// All headers across all banks precede every retained body, under the same
    /// original-program allowance. Equal local cursors never share custody.
    pub(crate) fn admit_retained(
        &self,
        program: &RuntimeProgramOwner,
        artifact: RuntimeArtifactFingerprint,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let result = (|| {
            let root = self
                .ledgers
                .get(&RuntimePresentationBankId::Root)
                .ok_or(RuntimePresentationError::InvalidState)?;
            admission.validate_metadata_collection(self.ledgers.len())?;
            if self.ledgers.len() > root.limits.scopes {
                return Err(RuntimePresentationError::InvalidState.into());
            }
            let mut proof: Option<PresentationAdmission> = None;
            let mut handles = 0usize;
            for (bank, ledger) in &self.ledgers {
                admission.validate_metadata(bank)?;
                if *bank != ledger.bank || ledger.limits != root.limits {
                    return Err(RuntimePresentationError::InvalidState.into());
                }
                handles = handles
                    .checked_add(ledger.state.leases.len())
                    .ok_or(RuntimePresentationError::Capacity)?;
                if handles > root.limits.handles {
                    return Err(RuntimePresentationError::Capacity.into());
                }
                let next = ledger.admit_ownership(program, artifact, admission)?;
                if let Some(proof) = &mut proof {
                    proof.merge(next)?;
                } else {
                    proof = Some(next);
                }
            }
            admission.validate_metadata_collection(self.retirements.len())?;
            if self.retirements.len() > root.limits.handles {
                return Err(RuntimePresentationError::RetirementCapacity.into());
            }
            let mut retired_ids = std::collections::BTreeSet::new();
            let mut proof = proof.ok_or(RuntimePresentationError::InvalidState)?;
            for retired in &self.retirements {
                admission.validate_metadata(&retired.token)?;
                self.ledgers
                    .get(&retired.token.bank())
                    .ok_or(RuntimePresentationError::InvalidState)?
                    .validate_retirement(&retired.token)?;
                if !retired_ids.insert(retired.token.clone()) {
                    return Err(RuntimePresentationError::InvalidState.into());
                }
                proof.include_retirement(&retired.token);
            }
            admission.bind_presentation_custody(proof)?;
            for ledger in self.ledgers.values() {
                ledger.admit_bodies(admission)?;
            }
            for retired in &self.retirements {
                retired.body.admit_retained(admission)?;
            }
            Ok(())
        })();
        result.inspect_err(|error: &AwbcRuntimeValueSnapshotError| {
            admission.retain_failure(error.clone())
        })
    }
}

impl<L: Clone> Clone for PresentationBanks<PresentationBody<AwbcRuntimeValueSnapshot>, L> {
    fn clone(&self) -> Self {
        Self {
            ledgers: self.ledgers.clone(),
            retirements: self.retirements.clone(),
        }
    }
}

impl<V, L: AsRef<PresentationLedgerState<PresentationBody<V>>>>
    PresentationBanks<PresentationBody<V>, L>
{
    pub(crate) fn visit_value_roots<E>(
        &self,
        visit: &mut impl FnMut(&V) -> Result<(), E>,
    ) -> Result<(), E> {
        for ledger in self.ledgers.values() {
            for body in ledger
                .as_ref()
                .leases
                .values()
                .filter_map(|lease| lease.body.as_ref())
            {
                visit(&body.value)?;
            }
        }
        for retired in &self.retirements {
            visit(&retired.body.value)?;
        }
        Ok(())
    }
}

impl PresentationBanks<PresentationBody<RuntimePayload>> {
    pub(crate) fn snapshot_admitted(
        &self,
    ) -> Result<
        PresentationBanks<PresentationBody<AwbcRuntimeValueSnapshot>>,
        AwbcRuntimeValueSnapshotError,
    > {
        let root = self
            .ledgers
            .get(&RuntimePresentationBankId::Root)
            .ok_or(RuntimePresentationError::InvalidState)?;
        Ok(PresentationBanks {
            ledgers: self
                .ledgers
                .iter()
                .map(|(bank, ledger)| Ok((*bank, ledger.snapshot_admitted()?)))
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
            retirements: self
                .retirements
                .iter()
                .map(|retired| {
                    Ok(PresentationRetirement {
                        token: retired.token.clone(),
                        body: retired.body.snapshot_admitted(&root.program)?,
                    })
                })
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
        })
    }
}

impl PresentationBanks<PresentationBody<AwbcRuntimeValueSnapshot>> {
    pub(crate) fn inert_state_admitted(&self) -> PresentationBanksSaveSnapshot {
        PresentationBanks {
            ledgers: self
                .ledgers
                .iter()
                .map(|(bank, ledger)| (*bank, ledger.state.clone()))
                .collect(),
            retirements: self.retirements.clone(),
        }
    }

    pub(crate) fn into_admitted_live<'a>(
        self,
        context: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> Result<PresentationBanks<PresentationBody<RuntimePayload>>, AwbcRuntimeValueSnapshotError>
    {
        let context = context.into();
        Ok(PresentationBanks {
            ledgers: self
                .ledgers
                .into_iter()
                .map(|(bank, ledger)| Ok((bank, ledger.into_admitted_live(context)?)))
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
            retirements: self
                .retirements
                .into_iter()
                .map(|retired| {
                    Ok(PresentationRetirement {
                        token: retired.token,
                        body: retired.body.into_admitted_live(context)?,
                    })
                })
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
        })
    }
}

impl PresentationBanksSaveSnapshot {
    /// Attaches the selected original lease to inert storage only. Product must
    /// still admit the whole owning contour before constructing any live value.
    pub(crate) fn retain_program(
        self,
        program: &RuntimeProgramOwner,
        limits: PresentationLedgerLimits,
    ) -> PresentationBanks<PresentationBody<AwbcRuntimeValueSnapshot>> {
        PresentationBanks {
            ledgers: self
                .ledgers
                .into_iter()
                .map(|(bank, state)| (bank, state.retain_program(program.clone(), bank, limits)))
                .collect(),
            retirements: self.retirements,
        }
    }
}

mod bank_rows;

#[cfg(test)]
mod tests;
