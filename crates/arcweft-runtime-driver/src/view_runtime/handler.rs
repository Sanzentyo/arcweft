//! Event-time execution and atomic publication from an accepted handler seal.

use super::{
    BundleViewEventDispatchError, BundleViewRuntime, MountedViewHandlerPublication,
    RuntimeDialogueActionToken, ViewProgramRuntimeAuthority,
};
use crate::dialogue::BundlePresentationInput;
use arcweft_core::{
    awbc::product_step::evaluate_pure_program_with_backend,
    pure::{RuntimeCallBackend, VmRuntimePureCallBackend},
    task::RuntimeProgramOwner,
    value::RuntimeValue,
};
use arcweft_view::{ViewHandlerInvocation, ViewHandlerResultRole, ViewHandlerTransitionValueRole};

impl BundleViewRuntime {
    /// Dispatches an active event through its exact mount-owned seal. A state
    /// transition executes here and publishes only after every output is checked.
    pub fn dispatch_invocation(
        &mut self,
        invocation: &ViewHandlerInvocation,
    ) -> Result<Option<BundlePresentationInput>, BundleViewEventDispatchError> {
        self.dispatch_invocation_with_backend(invocation, &mut VmRuntimePureCallBackend::default())
    }

    /// Uses the caller's accepted external-call/format context for the same
    /// verified program and publication boundary as ordinary View evaluation.
    pub fn dispatch_invocation_with_backend<B: RuntimeCallBackend>(
        &mut self,
        invocation: &ViewHandlerInvocation,
        backend: &mut B,
    ) -> Result<Option<BundlePresentationInput>, BundleViewEventDispatchError> {
        let published = self
            .event_tokens
            .get(&invocation.route())
            .ok_or(BundleViewEventDispatchError::UnknownBinding)?;
        if published.event != invocation.event() || &published.target != invocation.target() {
            return Err(BundleViewEventDispatchError::InvocationMismatch);
        }
        let mounted = self
            .mounts
            .get(&published.owner)
            .ok_or(BundleViewEventDispatchError::UnknownBinding)?;
        let seal = mounted
            .handler_seals
            .get(&published.seal)
            .filter(|seal| seal.revision == published.revision)
            .ok_or(BundleViewEventDispatchError::UnknownBinding)?;
        if let MountedViewHandlerPublication::DialogueAction(token) = &seal.publication {
            return token.presentation_input();
        }
        let ViewProgramRuntimeAuthority::Awbc(program) = &self.program_runtime else {
            return Err(BundleViewEventDispatchError::UnknownBinding);
        };
        let accepted = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.program_runtime(published.seal.program))
            .ok_or(BundleViewEventDispatchError::UnknownBinding)?;
        let result_type = accepted.result().value_type();
        let invalid_result = || BundleViewEventDispatchError::InvalidStateResult {
            expected: result_type,
        };
        let ViewHandlerResultRole::StateTransition {
            value: value_role,
            writes,
        } = accepted.result().role()
        else {
            return Err(invalid_result());
        };
        if seal.captures.len() != accepted.captures().len() {
            return Err(invalid_result());
        }
        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::clone(program));
        let arguments = seal
            .captures
            .iter()
            .zip(accepted.captures())
            .enumerate()
            .map(|(input, (capture, schema))| {
                let invalid = || BundleViewEventDispatchError::InvalidStateInput {
                    input,
                    expected: schema.value_type(),
                };
                let value = if let Some(address) = &capture.retained {
                    let cell = mounted.local_state.get(address).ok_or_else(|| {
                        BundleViewEventDispatchError::MissingStateField {
                            path: address.0.clone(),
                            field: address.1,
                        }
                    })?;
                    if cell.value_type != schema.value_type()
                        || !cell.value.ownership().permits_copy()
                    {
                        return Err(invalid());
                    }
                    cell.value.clone()
                } else {
                    capture
                        .snapshot
                        .clone()
                        .into_runtime_value_for_program(&owner)
                        .map_err(|_| invalid())?
                };
                if !program
                    .semantic_type_id(schema.value_type())
                    .is_some_and(|ty| program.value_matches_type(&value, ty))
                {
                    return Err(invalid());
                }
                Ok(value)
            })
            .collect::<Result<Vec<_>, BundleViewEventDispatchError>>()?;
        let result =
            evaluate_pure_program_with_backend(program, accepted.program(), &arguments, backend)
                .map_err(|source| BundleViewEventDispatchError::ProgramExecution {
                    source: Box::new(source),
                })?;
        if !program
            .semantic_type_id(result_type)
            .is_some_and(|ty| program.value_matches_type(&result, ty))
        {
            return Err(invalid_result());
        }
        let RuntimeValue::Tuple(results) = result else {
            return Err(invalid_result());
        };
        if results.len() != writes.len() + 1 {
            return Err(invalid_result());
        }
        let mut results = results.into_iter();
        let value = results.next().ok_or_else(invalid_result)?;
        let action = match value_role {
            ViewHandlerTransitionValueRole::Unit if matches!(value, RuntimeValue::Unit) => None,
            ViewHandlerTransitionValueRole::Unit => return Err(invalid_result()),
            ViewHandlerTransitionValueRole::DialogueAction => {
                RuntimeDialogueActionToken::try_from_runtime_value(value)?.presentation_input()?
            }
        };
        let updates = writes
            .iter()
            .zip(results)
            .map(|(write, value)| {
                let input = write.input();
                let expected = accepted
                    .captures()
                    .get(input)
                    .ok_or_else(invalid_result)?
                    .value_type();
                let invalid =
                    || BundleViewEventDispatchError::InvalidStateInput { input, expected };
                let address = seal
                    .captures
                    .get(input)
                    .and_then(|capture| capture.retained.as_ref())
                    .filter(|(_, field)| *field == write.field())
                    .ok_or_else(invalid)?;
                let cell = mounted.local_state.get(address).ok_or_else(|| {
                    BundleViewEventDispatchError::MissingStateField {
                        path: address.0.clone(),
                        field: address.1,
                    }
                })?;
                if cell.value_type != expected
                    || !value.ownership().permits_copy()
                    || !program
                        .semantic_type_id(expected)
                        .is_some_and(|ty| program.value_matches_type(&value, ty))
                {
                    return Err(invalid());
                }
                Ok((address.clone(), value))
            })
            .collect::<Result<Vec<_>, BundleViewEventDispatchError>>()?;

        let occurrence = published.owner.clone();
        let invalidate = !updates.is_empty();
        let revisions = if invalidate {
            self.mounts
                .iter()
                .filter(|(key, _)| key.handle == occurrence.handle)
                .map(|(key, mount)| {
                    let count = u64::try_from(mount.handler_seals.len())
                        .map_err(|_| BundleViewEventDispatchError::HandlerRevisionExhausted)?;
                    let first = mount.next_handler_seal_revision;
                    let end = first
                        .checked_add(count)
                        .ok_or(BundleViewEventDispatchError::HandlerRevisionExhausted)?;
                    Ok((key.clone(), first, end))
                })
                .collect::<Result<Vec<_>, BundleViewEventDispatchError>>()?
        } else {
            Vec::new()
        };
        // No execution, allocation, or fallible validation remains in this
        // commit. The exclusive runtime retains every prepared address/owner.
        let mounted = self
            .mounts
            .get_mut(&occurrence)
            .expect("the exclusive runtime retains the prepared occurrence");
        for (address, value) in updates {
            mounted
                .local_state
                .get_mut(&address)
                .expect("the exclusive mount retains every prepared output address")
                .value = value;
        }
        if invalidate {
            for (key, first, end) in revisions {
                let mounted = self
                    .mounts
                    .get_mut(&key)
                    .expect("the exclusive runtime retains the prepared route owner");
                for (seal, revision) in mounted.handler_seals.values_mut().zip(first..end) {
                    seal.revision = revision;
                }
                mounted.next_handler_seal_revision = end;
            }
            // A parent update can change child parameters/controls. Retire the
            // handle's old routes; new frames cannot revive their old leases.
            // Immutable cached values are retained independently of revisions.
            self.event_tokens
                .retain(|_, route| route.owner.handle != occurrence.handle);
        }
        Ok(action)
    }
}
