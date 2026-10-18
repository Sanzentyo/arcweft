//! Incremental data staging under one original-program resource allowance.
//! Active-scope authorization remains the execution owner's responsibility.

use super::RuntimeRegistryStateUpdate;
use crate::entry::RuntimeSchemaLimits;
use crate::pattern::RuntimeSemanticTypeId;
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::task::RuntimeProgramOwner;
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission,
    RuntimeRetainedValueAdmissionState, RuntimeValue,
};
use arcweft_id::LifetimeRegistryKey;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum RuntimeRegistryUpdateJournalError {
    #[error(transparent)]
    Value(#[from] AwbcRuntimeValueSnapshotError),
    #[error("registry update admission still has an outstanding validator")]
    AdmissionBorrowed,
}

enum Bank {
    Ready(RuntimeRetainedValueAdmissionState),
    Failed(RuntimeRegistryUpdateJournalError),
}

/// The builder alone may append; an enclosing failure publishes no prefix.
/// Finished packets are immutable data and carry no active-scope entitlement.
pub(crate) struct RuntimeRegistryUpdateJournal {
    bank: Bank,
    updates: Vec<Arc<RuntimeRegistryStateUpdate>>,
}

impl RuntimeRegistryUpdateJournal {
    pub(crate) fn new(
        program: RuntimeProgramOwner,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeRegistryUpdateJournalError> {
        let admission = RuntimeRetainedValueAdmission::for_program(program, limits);
        let state = admission
            .try_into_state()
            .map_err(|_| RuntimeRegistryUpdateJournalError::AdmissionBorrowed)?;
        Ok(Self {
            bank: Bank::Ready(state),
            updates: Vec::new(),
        })
    }

    pub(crate) fn try_push(
        &mut self,
        key: &LifetimeRegistryKey,
        expected: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        value: &RuntimeValue,
    ) -> Result<(), RuntimeRegistryUpdateJournalError> {
        let bank = std::mem::replace(
            &mut self.bank,
            Bank::Failed(RuntimeRegistryUpdateJournalError::AdmissionBorrowed),
        );
        let state = match bank {
            Bank::Ready(state) => state,
            Bank::Failed(error) => {
                self.bank = Bank::Failed(error.clone());
                return Err(error);
            }
        };
        let mut admission = state.resume();
        let update =
            RuntimeRegistryStateUpdate::try_copy(key, expected, binding, value, &mut admission)
                .map_err(RuntimeRegistryUpdateJournalError::from);
        self.bank = match admission.try_into_state() {
            Ok(state) => Bank::Ready(state),
            Err(_) => {
                let error = update
                    .err()
                    .unwrap_or(RuntimeRegistryUpdateJournalError::AdmissionBorrowed);
                self.bank = Bank::Failed(error.clone());
                return Err(error);
            }
        };
        // No packet is exposed before the unchanged allowance is exclusively
        // retained again. A later error remains sticky in that same bank.
        self.updates.push(Arc::new(update?));
        Ok(())
    }

    pub(crate) fn finish(
        self,
    ) -> Result<Vec<Arc<RuntimeRegistryStateUpdate>>, RuntimeRegistryUpdateJournalError> {
        match self.bank {
            Bank::Ready(state) => state.resume().status()?,
            Bank::Failed(error) => return Err(error),
        }
        Ok(self.updates)
    }
}
