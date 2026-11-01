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
        self.admit_presentation_state_for_slot_removal(admission, None)
    }

    pub(super) fn admit_presentation_state_for_slot_removal(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        removal: Option<crate::presentation::resources::PresentationSlotRemovalAdmission<'_>>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        let owner = crate::task::RuntimeProgramOwner::Plan(Arc::clone(&self.plan));
        if let Some(banks) = &self.presentation {
            let artifact = self
                .plan
                .artifact()
                .ok_or(RuntimePresentationError::MissingArtifact)?;
            banks.admit_retained_for_slot_removal(&owner, artifact, admission, removal)?;
        } else if let Some(removal) = removal {
            admission.validate_owned_option_preview(None, removal.result, removal.binding)?;
        }
        Ok(())
    }
}

impl Engine {
    pub(super) fn evaluate_presentation_slot_removal(
        &mut self,
        call: &crate::presentation::RuntimePresentationSlotCall,
        args: &[crate::value::RuntimeCallArgument],
        result: crate::runtime_id::RuntimePlanTypeId,
        backend: &mut impl crate::pure::RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        use crate::{
            plan::RuntimePlanTypeProjection,
            presentation::resources::PresentationSlotRemovalAdmission,
            value::{RuntimeCallArgumentMode, RuntimeRetainedValueAdmission},
        };
        call.validate_execution(args.len())?;
        let artifact = self.presentation_context_artifact()?;
        if !self
            .fiber
            .frame
            .presentation_bank
            .matches_artifact(artifact)
        {
            return Err(RuntimePresentationError::Program.into());
        }
        let row = self
            .plan
            .type_table()
            .get(result)
            .ok_or(RuntimeEvalError::InvalidExpressionType(result))?;
        if call.is_reference()
            || !call.accepts_plan_result(row.projection(), |item| {
                self.plan
                    .type_table()
                    .get(*item)
                    .and_then(|row| match row.projection() {
                        RuntimePlanTypeProjection::PresentationResource(kind) => Some(*kind),
                        _ => None,
                    })
            })
        {
            return Err(RuntimePresentationError::SlotReferenceArguments.into());
        }
        let result = row.semantic_identity();
        let mut positions = 0u8;
        for argument in args {
            let position = usize::try_from(argument.abi_position())
                .map_err(|_| RuntimePresentationError::SlotReferenceArguments)?;
            if position >= args.len()
                || positions & (1 << position) != 0
                || argument.mode() != RuntimeCallArgumentMode::Value
                || !self
                    .plan
                    .type_table()
                    .get(argument.value().ty())
                    .is_some_and(|row| {
                        matches!(row.projection(), RuntimePlanTypeProjection::EntityReference)
                    })
            {
                return Err(RuntimePresentationError::SlotReferenceArguments.into());
            }
            positions |= 1 << position;
        }
        let mut values = Vec::with_capacity(args.len());
        // Keep authored evaluation order; select roles only after evaluation.
        for argument in args {
            values.push((
                argument.abi_position(),
                self.evaluate_expr_with_backend(argument.value(), backend)?,
            ));
        }
        values.sort_unstable_by_key(|(position, _)| *position);
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            self.fiber.frame.program.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let admitted = (|| {
            for (_, value) in &values {
                admission.validate_borrowed(value.into())?;
            }
            let address = crate::presentation::RuntimePresentationSlotSelectors::from_values(
                call.inputs()
                    .iter()
                    .copied()
                    .zip(values.iter().map(|(_, value)| value)),
            )?
            .construct_address(call.resource(), &mut admission)?;
            self.admit_live_retained_values_for_slot_removal(
                &mut admission,
                Some(PresentationSlotRemovalAdmission {
                    bank: self.fiber.frame.presentation_bank,
                    address: &address,
                    result,
                    binding: self.fiber.frame.env.function_instantiation(),
                }),
            )?;
            admission.finish_presentation_custody()?;
            Ok::<_, crate::value::AwbcRuntimeValueSnapshotError>(address)
        })()
        .map_err(|source| RuntimeEvalError::NativeWorkAdmission {
            source: Box::new(source),
        })?;
        let removed = self
            .presentation
            .as_mut()
            .map(|banks| {
                banks.clear_slot(
                    &self.fiber.frame.program,
                    self.fiber.frame.presentation_bank,
                    &admitted,
                )
            })
            .transpose()?
            .flatten();
        Ok(removed.map_or_else(RuntimeValue::option_none, |body| {
            RuntimeValue::option_some(body.value.into_value())
        }))
    }

    pub(crate) fn register_presentation_body(
        &mut self,
        site: PresentationRegistrationSite,
        expected: crate::pattern::RuntimeSemanticTypeId,
        value: RuntimePayload,
    ) -> Result<
        (
            crate::runtime_id::RuntimePresentationHandleToken,
            Option<
                crate::presentation::resources::PresentationRetirement<
                    PresentationBody<RuntimePayload>,
                >,
            >,
        ),
        crate::presentation::resources::PresentationRegisterError<
            RuntimePayload,
            crate::value::AwbcRuntimeValueSnapshotError,
        >,
    > {
        let (owner, scope) = match self.prepare_presentation_registration(expected, &value) {
            Ok(prepared) => prepared,
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
        Ok((token, previous))
    }

    /// The actual frame owns the execution origin. Check it before capture
    /// evaluation and again before registration; this issues no grant or bank.
    pub(super) fn presentation_context_artifact(
        &self,
    ) -> Result<crate::effect::RuntimeArtifactFingerprint, RuntimePresentationError> {
        if !matches!(&self.fiber.frame.program, crate::task::RuntimeProgramOwner::Plan(program)
            if Arc::ptr_eq(program, &self.plan))
        {
            return Err(RuntimePresentationError::Program);
        }
        if self.fiber.execution.get().get() != self.fiber.persistent_id.get() {
            return Err(RuntimePresentationError::Execution);
        }
        self.fiber.frame.env.validate_presentation_context(
            &self.fiber.frame.program,
            self.fiber.execution,
            self.fiber.persistent_id,
        )?;
        if self.presentation.is_none()
            && self
                .fiber
                .frame
                .env
                .presentation_scope_bindings()
                .next()
                .is_some()
        {
            return Err(RuntimePresentationError::Owner);
        }
        self.plan
            .artifact()
            .ok_or(RuntimePresentationError::MissingArtifact)
    }

    fn prepare_presentation_registration(
        &mut self,
        expected: crate::pattern::RuntimeSemanticTypeId,
        value: &RuntimePayload,
    ) -> Result<
        (
            crate::task::RuntimeProgramOwner,
            crate::presentation::resources::PresentationScopeOwner,
        ),
        crate::value::AwbcRuntimeValueSnapshotError,
    > {
        let artifact = self.presentation_context_artifact()?;
        let owner = self.fiber.frame.program.clone();
        let limits = crate::entry::RuntimeSchemaLimits::engine_default();
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
        let scope = self.fiber.bind_presentation_scope(ledger)?;
        Ok((owner, scope))
    }

    pub(super) fn evaluate_presentation_factory(
        &mut self,
        call: &Arc<crate::presentation::RuntimePresentationFactoryCall>,
        values: Vec<RuntimeValue>,
        result: crate::runtime_id::RuntimePlanTypeId,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        let error = |cause: String| RuntimeEvalError::UnsupportedPure {
            name: "core.presentation.factory".into(),
            reason: cause,
        };
        call.validate_argument_count(values.len())
            .map_err(|cause| error(cause.to_string()))?;
        let row = self
            .plan
            .type_table()
            .get(result)
            .ok_or(RuntimePresentationError::InvalidState)?;
        let handle_owner = call
            .result_owner(
                crate::program_types::RuntimeProgramTypes::Plan(&self.plan),
                row.semantic_identity(),
            )
            .map_err(|cause| error(cause.to_string()))?;
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            self.fiber.frame.program.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.admit_live_retained_values(&mut admission)
            .map_err(|cause| error(cause.to_string()))?;
        let binding = self.fiber.frame.env.function_instantiation_lease().cloned();
        let (site, resource) = admission
            .construct_native_presentation_resource_with_metadata(
                Arc::clone(&self.plan),
                Arc::clone(call),
                binding.clone(),
                RuntimePayload::new(RuntimeValue::Tuple(values)),
                |admission, call, body| call.registration_site(admission, body),
            )
            .map_err(|cause| error(cause.to_string()))?;
        admission
            .finish_presentation_custody()
            .map_err(|cause| error(cause.to_string()))?;
        let value = RuntimePayload::new(resource);
        if let Some(banks) = &self.presentation {
            banks.preflight_registration_site(
                &self.fiber.frame.program,
                self.fiber.frame.presentation_bank,
                &site,
            )?;
        }
        let (program, scope) = self
            .prepare_presentation_registration(call.resource_type(), &value)
            .map_err(|cause| error(cause.to_string()))?;
        let token = self
            .presentation
            .as_mut()
            .expect("prepared bank")
            .register_and_retire(
                &program,
                scope,
                site,
                PresentationBody {
                    expected: call.resource_type(),
                    binding,
                    value,
                },
            )
            .map_err(|cause| RuntimeEvalError::from(cause.cause))?;
        handle_owner
            .try_wrap(token.encode_payload())
            .map_err(|cause| error(cause.to_string()))
    }

    /// Reconciles resources before removing the physical lexical frame. Bodies
    /// move to the engine's retirement owner, including across rollback.
    pub(super) fn pop_runtime_scope(&mut self) -> Result<(), RuntimeEvalError> {
        self.retire_current_presentation_scope()?;
        self.fiber.frame.env.pop_scope();
        Ok(())
    }

    pub(super) fn evaluate_presentation_handle_method(
        &mut self,
        method: arcweft_interaction_model::presentation::PresentationHandleMethodId,
        target: &crate::value::RuntimeValueTarget,
        backend: &mut impl crate::pure::RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        use crate::value::{
            RuntimeOpaqueValueClass, RuntimeRetainedValueAdmission, RuntimeValueTarget,
        };
        self.presentation_context_artifact()?;
        let owned = match target {
            RuntimeValueTarget::Value(value) => {
                Some(self.evaluate_expr_with_backend(value, backend)?)
            }
            RuntimeValueTarget::Inspect { .. } | RuntimeValueTarget::Take { .. } => None,
        };
        let value = match target {
            RuntimeValueTarget::Value(_) => owned.as_ref().expect("evaluated owned receiver"),
            RuntimeValueTarget::Inspect { place, .. } | RuntimeValueTarget::Take { place, .. } => {
                self.fiber.frame.env.inspect_place(place)?
            }
        };
        let semantic = self
            .plan
            .type_table()
            .get(target.ty())
            .ok_or(RuntimeEvalError::InvalidExpressionType(target.ty()))?
            .semantic_identity();
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            self.fiber.frame.program.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.admit_live_retained_values(&mut admission)
            .map_err(|source| RuntimeEvalError::NativeWorkAdmission {
                source: Box::new(source),
            })?;
        let binding = self.fiber.frame.env.function_instantiation();
        let admitted = if owned.is_some() {
            admission.validate_owned_typed_borrowed_in_invocation(value.into(), semantic, binding)
        } else {
            admission.validate_admitted_typed_borrowed_in_invocation(
                value.into(),
                semantic,
                binding,
            )
        };
        admitted.map_err(|source| RuntimeEvalError::NativeWorkAdmission {
            source: Box::new(source),
        })?;
        admission.finish_presentation_custody().map_err(|source| {
            RuntimeEvalError::NativeWorkAdmission {
                source: Box::new(source),
            }
        })?;
        let RuntimeValue::Opaque(handle) = value else {
            return Err(RuntimeEvalError::InvalidExpressionType(target.ty()));
        };
        if handle.value_class() != RuntimeOpaqueValueClass::PresentationHandle {
            return Err(RuntimeEvalError::InvalidExpressionType(target.ty()));
        }
        let token =
            crate::runtime_id::RuntimePresentationHandleToken::try_decode_payload(handle.payload())
                .map_err(|_| RuntimePresentationError::InvalidState)?;
        let banks = self
            .presentation
            .as_mut()
            .ok_or(RuntimePresentationError::MissingLedger)?;
        let scope = banks.handle_scope(
            &self.fiber.frame.program,
            &token,
            self.fiber.frame.env.presentation_scope_bindings(),
        )?;
        banks.apply_handle_method(&self.fiber.frame.program, &token, scope, method)?;
        if let RuntimeValueTarget::Take { place, .. } = target {
            self.fiber
                .frame
                .env
                .take_place(place)
                .expect("exclusive method preflight retains the initialized receiver");
        }
        Ok(RuntimeValue::Unit)
    }

    pub(super) fn transition_presentation_return<'value>(
        &mut self,
        scopes: usize,
        detached: bool,
        values: impl IntoIterator<Item = &'value RuntimeValue>,
    ) -> Result<(), RuntimeEvalError> {
        use crate::presentation::resources::PresentationScopeExit;
        let escaping =
            values
                .into_iter()
                .try_fold(Vec::new(), |mut escaping, value| {
                    escaping.extend(value.view().affine_handles()?.into_iter().filter_map(
                        |handle| match handle {
                            crate::value::ownership::RuntimeAffineHandle::Presentation {
                                token,
                                ..
                            } => Some(token),
                            crate::value::ownership::RuntimeAffineHandle::Line(_) => None,
                        },
                    ));
                    Ok::<_, crate::value::ownership::RuntimeAffineHandleError>(escaping)
                })
                .map_err(|cause| RuntimeEvalError::UnsupportedPure {
                    name: "presentation return".into(),
                    reason: cause.to_string(),
                })?;
        if self.presentation.is_none() && escaping.is_empty() {
            return Ok(());
        }
        let banks = self
            .presentation
            .as_mut()
            .ok_or(RuntimePresentationError::MissingLedger)?;
        let execution = self.fiber.execution;
        let fiber = self.fiber.persistent_id;
        let (exiting, destination) = self
            .fiber
            .frame
            .env
            .presentation_scope_exit_targets(scopes, detached)?;
        banks.transition_scope_contour(
            &self.fiber.frame.program,
            exiting
                .into_iter()
                .map(|scope| PresentationScopeExit {
                    execution,
                    fiber,
                    scope,
                })
                .collect(),
            destination.map(|scope| PresentationScopeExit {
                execution,
                fiber,
                scope,
            }),
            &escaping,
            &[],
        )?;
        Ok(())
    }

    pub(super) fn finish_runtime_value_scope(
        &mut self,
        result: Result<RuntimeValue, RuntimeEvalError>,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        if let Ok(value) = &result {
            self.transition_presentation_return(1, false, std::iter::once(value))?;
        }
        self.pop_runtime_scope()?;
        result
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
