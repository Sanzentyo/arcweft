//! Engine-owned resources share admission and rollback with their exact frames.

use super::*;
use crate::presentation::RuntimePresentationError;
use crate::presentation::resources::{
    PresentationBanks, PresentationBody, PresentationLedgerLimits, PresentationRegistrationSite,
};
use crate::value::RuntimeStoredValue;

impl<N, P: RuntimeStoredValue, R, F, D> Engine<N, P, R, F, D> {
    pub(super) fn admit_presentation_state(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan));
        if let Some(banks) = &self.presentation {
            let artifact = self
                .plan
                .artifact()
                .ok_or(RuntimePresentationError::MissingArtifact)?;
            banks.admit_retained(&owner, artifact, admission)?;
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
            self.admit_live_retained_values(&mut admission)?;
            let binding = self.fiber.frame.env.function_instantiation_lease();
            admission.validate_metadata(&(expected, binding))?;
            admission.validate_owned_typed_borrowed_in_invocation(
                value.borrowed_value(),
                expected,
                binding.map(Arc::as_ref),
            )?;
            admission.finish_presentation_custody()?;
            let banks = self.presentation.get_or_insert_with(|| {
                PresentationBanks::new(
                    owner.clone(),
                    artifact,
                    PresentationLedgerLimits::for_schema(limits),
                )
            });
            banks.preflight_register(&owner)?;
            let ledger = banks.ledger_mut(self.fiber.frame.presentation_bank)?;
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
            .expect("prepared original resource owner")
            .ledger_mut(self.fiber.frame.presentation_bank)
            .expect("prepared original bank");
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
            let banks = self
                .presentation
                .as_mut()
                .ok_or(RuntimePresentationError::Owner)?;
            banks.retire_bound_scope(
                &self.fiber.frame.program,
                self.fiber.execution,
                self.fiber.persistent_id,
                self.fiber.frame.env.presentation_scope_target()?,
            )?;
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
