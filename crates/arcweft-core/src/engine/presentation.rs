//! Engine-owned resources share admission and rollback with their exact frames.

use super::*;
use crate::presentation::RuntimePresentationError;
use crate::presentation::resources::{
    PresentationBody, PresentationLedger, PresentationLedgerLimits, PresentationRegistrationSite,
};
use crate::value::RuntimeStoredValue;

impl<N, P: RuntimeStoredValue, R, F, D> Engine<N, P, R, F, D> {
    pub(super) fn admit_presentation_state(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan));
        if let Some(ledger) = &self.presentation {
            let artifact = self
                .plan
                .artifact()
                .ok_or(RuntimePresentationError::MissingArtifact)?;
            let proof = ledger.admit_retained(&owner, artifact, admission)?;
            admission.bind_presentation_scopes(proof);
        }
        admission.validate_metadata_collection(self.presentation_retirements.len())?;
        let mut retired_ids = BTreeSet::new();
        for retired in &self.presentation_retirements {
            admission.validate_metadata(&retired.token)?;
            let ledger = self
                .presentation
                .as_ref()
                .ok_or(RuntimePresentationError::InvalidState)?;
            ledger.validate_retirement(&retired.token)?;
            if !retired_ids.insert(retired.token.issuance()) {
                return Err(RuntimePresentationError::InvalidState.into());
            }
            if Some(*retired.token.artifact()) != self.plan.artifact() {
                return Err(RuntimePresentationError::InvalidState.into());
            }
            retired.body.admit_retained(admission)?;
        }
        Ok(())
    }
}

impl Engine {
    pub(crate) fn register_presentation_body(
        &mut self,
        site: PresentationRegistrationSite,
        expected: crate::pattern::RuntimeSemanticTypeId,
        value: RuntimePayload,
    ) -> Result<
        (
            crate::runtime_id::RuntimePresentationHandleToken,
            Option<RuntimePayload>,
        ),
        crate::presentation::resources::PresentationRegisterError<
            RuntimePayload,
            crate::value::AwbcRuntimeValueSnapshotError,
        >,
    > {
        let owner = self.fiber.frame.program.clone();
        let limits = crate::entry::RuntimeSchemaLimits::engine_default();
        let prepared = (|| {
            if !owner.same_program(&crate::task::RuntimeProgramOwner::Plan(Arc::clone(
                &self.plan,
            ))) {
                return Err(RuntimePresentationError::Program.into());
            }
            if self.fiber.execution.get().get() != self.fiber.persistent_id.get() {
                return Err(RuntimePresentationError::Execution.into());
            }
            if self.presentation.is_none()
                && self.fiber.frame.env.presentation_scope_binding().is_some()
            {
                return Err(RuntimePresentationError::Owner.into());
            }
            let artifact = self
                .plan
                .artifact()
                .ok_or(RuntimePresentationError::MissingArtifact)?;
            let mut admission =
                crate::value::RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
            let binding = self.fiber.frame.env.function_instantiation_lease();
            admission.validate_metadata(&(expected, binding))?;
            admission.validate_owned_typed_borrowed_in_invocation(
                value.borrowed_value(),
                expected,
                binding.map(Arc::as_ref),
            )?;
            let ledger = self.presentation.get_or_insert_with(|| {
                PresentationLedger::new(
                    owner.clone(),
                    artifact,
                    PresentationLedgerLimits {
                        handles: limits.max_sequence_items as usize,
                        scopes: limits.max_sequence_items as usize,
                    },
                )
            });
            ledger.preflight_register(&owner)?;
            Ok::<_, crate::value::AwbcRuntimeValueSnapshotError>(
                self.fiber.bind_presentation_scope(ledger)?,
            )
        })();
        let scope = match prepared {
            Ok(scope) => scope,
            Err(cause) => {
                return Err(crate::presentation::resources::PresentationRegisterError {
                    cause,
                    body: value,
                });
            }
        };
        let ledger = self
            .presentation
            .as_mut()
            .expect("prepared original resource owner");
        let (token, previous) = ledger
            .register(
                &owner,
                scope,
                site,
                PresentationBody {
                    expected,
                    binding: self.fiber.frame.env.function_instantiation_lease().cloned(),
                    value,
                },
            )
            .map_err(
                |error| crate::presentation::resources::PresentationRegisterError {
                    cause: error.cause.into(),
                    body: error.body.value,
                },
            )?;
        Ok((token, previous.map(|body| body.value)))
    }

    /// Reconciles resources before removing the physical lexical frame. Bodies
    /// move to the engine's retirement owner, including across rollback.
    pub(super) fn pop_runtime_scope(&mut self) -> Result<(), RuntimeEvalError> {
        self.retire_current_presentation_scope()?;
        self.fiber.frame.env.pop_scope();
        Ok(())
    }

    pub(super) fn retire_current_presentation_scope(&mut self) -> Result<(), RuntimeEvalError> {
        if self.fiber.frame.env.presentation_scope_binding().is_some() {
            let ledger = self
                .presentation
                .as_mut()
                .ok_or(RuntimePresentationError::Owner)?;
            let maximum =
                crate::entry::RuntimeSchemaLimits::engine_default().max_sequence_items as usize;
            let count = ledger.bound_scope_body_count(&self.fiber.frame.env);
            if self
                .presentation_retirements
                .len()
                .checked_add(count)
                .is_none_or(|count| count > maximum)
            {
                return Err(RuntimePresentationError::RetirementCapacity.into());
            }
            let retired = self.fiber.release_presentation_scope(ledger)?;
            self.presentation_retirements.extend(retired);
        }
        Ok(())
    }

    pub(super) fn pop_runtime_scope_for_output(&mut self, output: &mut RuntimeStepOutput) -> bool {
        if let Err(error) = self.pop_runtime_scope() {
            self.fail_eval(error, output);
            return false;
        }
        true
    }
}
