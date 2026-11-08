//! Event-time execution and atomic publication from an accepted handler seal.

use super::{
    BundleViewEventDispatchError, BundleViewRuntime, MountedViewHandlerPublication,
    RuntimeDialogueActionToken, ViewProgramRuntimeAuthority,
};
use crate::dialogue::BundlePresentationInput;
use arcweft_core::{
    pure::{RuntimeCallBackend, RuntimeProgramExecutionContext, VmRuntimePureCallBackend},
    task::RuntimeProgramOwner,
    value::{RuntimeFormatContext, RuntimeValue},
};
use arcweft_view::{
    EventKind, ViewHandlerInvocation, ViewHandlerResultRole, ViewHandlerRouteId,
    ViewHandlerTransitionValueRole,
};

type StateAddress = (
    super::BundleViewInstancePath,
    arcweft_view::ViewStateFieldId,
);

struct PreparedTextInputUpdate {
    owner: super::ViewOccurrenceKey,
    address: StateAddress,
    value: RuntimeValue,
}

impl BundleViewRuntime {
    pub(crate) fn has_retained_text_input(&self, target: &str) -> bool {
        self.mounts.values().any(|mount| {
            mount
                .text_input_seals
                .values()
                .any(|seal| seal.target == target)
        })
    }

    /// Admits borrowed adapter text before allocation, executes against the
    /// prospective field value, then publishes the input and handler writes as
    /// one state transition. Rejection leaves state and event leases unchanged.
    pub fn dispatch_text_input_with_backend<B: RuntimeCallBackend>(
        &mut self,
        input: &arcweft_presentation::text_input::TextControlWriteBack,
        format_context: &RuntimeFormatContext,
        backend: &mut B,
    ) -> Result<Option<BundlePresentationInput>, BundleViewEventDispatchError> {
        self.dispatch_text_input_in_session(input, format_context, backend, None)
    }

    pub(crate) fn dispatch_text_input_in_session<B: RuntimeCallBackend>(
        &mut self,
        input: &arcweft_presentation::text_input::TextControlWriteBack,
        format_context: &RuntimeFormatContext,
        backend: &mut B,
        executor: Option<&mut arcweft_core::executor::ArcweftRuntimeExecutor>,
    ) -> Result<Option<BundlePresentationInput>, BundleViewEventDispatchError> {
        let invalid = || BundleViewEventDispatchError::InvalidTextInput;
        let (owner, mounted, seal_key, seal) = self
            .mounts
            .iter()
            .find_map(|(owner, mount)| {
                mount
                    .text_input_seals
                    .iter()
                    .find(|(_, seal)| seal.target == input.target().id().as_str())
                    .map(|(key, seal)| (owner, mount, key, seal))
            })
            .ok_or_else(invalid)?;
        let text = input.value().as_str();
        if seal.session != input.session().0
            || seal.secure != input.value().is_sensitive()
            || input.revision().0 < seal.revision
            || (input.is_change() && input.revision().0 == seal.revision)
            || !text.is_char_boundary(input.selection().start().get() as usize)
            || !text.is_char_boundary(input.selection().end().get() as usize)
        {
            return Err(invalid());
        }
        let ViewProgramRuntimeAuthority::Awbc(program) = &self.program_runtime else {
            return Err(invalid());
        };
        let mut admission = arcweft_core::value::RuntimeRetainedValueAdmission::for_program(
            RuntimeProgramOwner::Awbc(program.program_arc()),
            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
        );
        for (address, field) in &mounted.local_state {
            if address == &seal.address {
                admission
                    .validate_typed_borrowed(text.into(), seal.value_type)
                    .map_err(|_| invalid())?;
            } else {
                admission
                    .validate_typed(&field.value, field.value_type)
                    .map_err(|_| invalid())?;
            }
        }
        if !mounted.local_state.contains_key(&seal.address) {
            return Err(invalid());
        }
        let event = if input.is_submit() {
            EventKind::TextSubmit
        } else {
            EventKind::TextChange
        };
        let route = self.event_tokens.iter().find_map(|(route, token)| {
            (token.owner == *owner
                && token.seal.path == seal_key.0
                && token.seal.instruction == seal_key.1
                && token.event == event
                && token.target == *input.target())
            .then_some(*route)
        });
        if route.is_none()
            && mounted.handler_seals.keys().any(|key| {
                key.path == seal_key.0 && key.instruction == seal_key.1 && key.event == event
            })
        {
            return Err(BundleViewEventDispatchError::UnknownBinding);
        }
        let owner = owner.clone();
        let seal_key = seal_key.clone();
        let update = PreparedTextInputUpdate {
            owner: owner.clone(),
            address: seal.address.clone(),
            value: RuntimeValue::String(text.to_owned()),
        };
        let action = if let Some(route) = route {
            self.dispatch_handler_with_input(
                route,
                event,
                input.target(),
                format_context,
                backend,
                Some(update),
                executor,
            )?
        } else {
            self.publish_state_updates(
                &update.owner,
                vec![(update.address, update.value)],
                || Ok(()),
            )?;
            None
        };
        let seal = self
            .mounts
            .get_mut(&owner)
            .expect("prepared mount remains exclusively owned")
            .text_input_seals
            .get_mut(&seal_key)
            .expect("prepared text input remains exclusively owned");
        seal.selection = arcweft_bundle::resource_codec::view::ViewRuntimeTextSelection::new(
            input.selection().start().get(),
            input.selection().end().get(),
        );
        seal.revision = input.revision().0;
        Ok(action)
    }

    /// Dispatches an active event through its exact mount-owned seal. A state
    /// transition executes here and publishes only after every output is checked.
    pub fn dispatch_invocation(
        &mut self,
        invocation: &ViewHandlerInvocation,
    ) -> Result<Option<BundlePresentationInput>, BundleViewEventDispatchError> {
        self.dispatch_invocation_with_backend(
            invocation,
            &RuntimeFormatContext::default(),
            &mut VmRuntimePureCallBackend::default(),
        )
    }

    /// Uses the caller's accepted external-call/format context for the same
    /// verified program and publication boundary as ordinary View evaluation.
    pub fn dispatch_invocation_with_backend<B: RuntimeCallBackend>(
        &mut self,
        invocation: &ViewHandlerInvocation,
        format_context: &RuntimeFormatContext,
        backend: &mut B,
    ) -> Result<Option<BundlePresentationInput>, BundleViewEventDispatchError> {
        self.dispatch_invocation_in_session(invocation, format_context, backend, None)
    }

    pub(crate) fn dispatch_invocation_in_session<B: RuntimeCallBackend>(
        &mut self,
        invocation: &ViewHandlerInvocation,
        format_context: &RuntimeFormatContext,
        backend: &mut B,
        executor: Option<&mut arcweft_core::executor::ArcweftRuntimeExecutor>,
    ) -> Result<Option<BundlePresentationInput>, BundleViewEventDispatchError> {
        self.dispatch_handler_with_input(
            invocation.route(),
            invocation.event(),
            invocation.target(),
            format_context,
            backend,
            None,
            executor,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn dispatch_handler_with_input<B: RuntimeCallBackend>(
        &mut self,
        route: ViewHandlerRouteId,
        event: EventKind,
        target: &arcweft_presentation::input::InteractionTarget,
        format_context: &RuntimeFormatContext,
        backend: &mut B,
        input_update: Option<PreparedTextInputUpdate>,
        executor: Option<&mut arcweft_core::executor::ArcweftRuntimeExecutor>,
    ) -> Result<Option<BundlePresentationInput>, BundleViewEventDispatchError> {
        let published = self
            .event_tokens
            .get(&route)
            .ok_or(BundleViewEventDispatchError::UnknownBinding)?;
        if published.event != event
            || &published.target != target
            || input_update
                .as_ref()
                .is_some_and(|update| update.owner != published.owner)
        {
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
        if &seal.format_context != format_context {
            return Err(BundleViewEventDispatchError::InvocationMismatch);
        }
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
        let owner = RuntimeProgramOwner::Awbc(program.program_arc());
        let mut admission = arcweft_core::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
        );
        admission
            .validate_metadata_string(format_context.active_locale().as_str())
            .map_err(|_| invalid_result())?;
        for (input, (capture, schema)) in seal.captures.iter().zip(accepted.captures()).enumerate()
        {
            let invalid = || BundleViewEventDispatchError::InvalidStateInput {
                input,
                expected: schema.value_type(),
            };
            if let Some(address) = &capture.retained {
                let cell = mounted.local_state.get(address).ok_or_else(|| {
                    BundleViewEventDispatchError::MissingStateField {
                        path: address.0.clone(),
                        field: address.1,
                    }
                })?;
                if cell.value_type != schema.value_type() {
                    return Err(invalid());
                }
                let value = input_update
                    .as_ref()
                    .filter(|update| &update.address == address)
                    .map_or(&cell.value, |update| &update.value);
                admission
                    .validate_typed(value, schema.value_type())
                    .map_err(|_| invalid())?;
            } else {
                admission
                    .validate_typed_snapshot(&capture.snapshot, schema.value_type())
                    .map_err(|_| invalid())?;
            }
        }
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
                    if cell.value_type != schema.value_type() {
                        return Err(invalid());
                    }
                    let value = input_update
                        .as_ref()
                        .filter(|update| &update.address == address)
                        .map_or(&cell.value, |update| &update.value);
                    value
                        .copy_unrestricted_typed_for_program_with_limits(
                            &owner,
                            schema.value_type(),
                            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
                        )
                        .map_err(|_| invalid())?
                } else {
                    capture
                        .snapshot
                        .copy_unrestricted_typed_runtime_value_for_program_with_limits(
                            &owner,
                            schema.value_type(),
                            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
                        )
                        .map_err(|_| invalid())?
                };
                Ok(value)
            })
            .collect::<Result<Vec<_>, BundleViewEventDispatchError>>()?;
        let result = RuntimeProgramExecutionContext::awbc(program.clone(), format_context.clone())
            .evaluate_pure_program(accepted.program(), &arguments, backend)
            .map_err(|source| BundleViewEventDispatchError::ProgramExecution { source })?;
        admission
            .validate_typed(&result, result_type)
            .map_err(|_| invalid_result())?;
        let RuntimeValue::Tuple(results) = result else {
            return Err(invalid_result());
        };
        if results.len() != writes.len() + 1 {
            return Err(invalid_result());
        }
        let mut results = results.into_iter();
        let value = results.next().ok_or_else(invalid_result)?;
        let mut named_action = None;
        let action = match value_role {
            ViewHandlerTransitionValueRole::Unit if matches!(value, RuntimeValue::Unit) => None,
            ViewHandlerTransitionValueRole::Unit => return Err(invalid_result()),
            ViewHandlerTransitionValueRole::DialogueAction => {
                RuntimeDialogueActionToken::try_from_runtime_value(value)?.presentation_input()?
            }
            ViewHandlerTransitionValueRole::Action { payload_type } => {
                owner
                    .types()
                    .action_declaration(*payload_type)
                    .map_err(|_| invalid_result())?;
                named_action = Some((*payload_type, value));
                None
            }
        };
        let mut updates = writes
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
                if cell.value_type != expected {
                    return Err(invalid());
                }
                value
                    .validate_unrestricted_typed_for_program_with_limits(
                        &owner,
                        expected,
                        arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
                    )
                    .map_err(|_| invalid())?;
                Ok((address.clone(), value))
            })
            .collect::<Result<Vec<_>, BundleViewEventDispatchError>>()?;

        let occurrence = published.owner.clone();
        if let Some(update) = input_update {
            if !updates
                .iter()
                .any(|(address, _)| address == &update.address)
            {
                updates.push((update.address, update.value));
            }
        }
        self.publish_state_updates(&occurrence, updates, || {
            if let Some((payload_type, payload)) = named_action {
                executor
                    .ok_or(BundleViewEventDispatchError::ActionContextRequired)?
                    .emit_action(payload_type, payload)
                    .map_err(|source| BundleViewEventDispatchError::ActionEmission { source })?;
            }
            Ok(())
        })?;
        Ok(action)
    }

    fn publish_state_updates(
        &mut self,
        occurrence: &super::ViewOccurrenceKey,
        updates: Vec<(StateAddress, RuntimeValue)>,
        publish_action: impl FnOnce() -> Result<(), BundleViewEventDispatchError>,
    ) -> Result<(), BundleViewEventDispatchError> {
        let mounted = self
            .mounts
            .get(occurrence)
            .ok_or(BundleViewEventDispatchError::UnknownBinding)?;
        let ViewProgramRuntimeAuthority::Awbc(program) = &self.program_runtime else {
            return Err(BundleViewEventDispatchError::UnknownBinding);
        };
        let mut admission = arcweft_core::value::RuntimeRetainedValueAdmission::for_program(
            RuntimeProgramOwner::Awbc(program.program_arc()),
            arcweft_core::entry::RuntimeSchemaLimits::engine_default(),
        );
        for (index, (address, field)) in mounted.local_state.iter().enumerate() {
            let value = updates
                .iter()
                .find(|(candidate, _)| candidate == address)
                .map_or(&field.value, |(_, value)| value);
            admission
                .validate_typed(value, field.value_type)
                .map_err(|_| BundleViewEventDispatchError::InvalidStateInput {
                    input: index,
                    expected: field.value_type,
                })?;
        }
        if updates
            .iter()
            .any(|(address, _)| !mounted.local_state.contains_key(address))
        {
            return Err(BundleViewEventDispatchError::InvalidTextInput);
        }
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
        // Both owners have finished View preflight. A refused Core emission
        // leaves every View value, revision and event lease untouched.
        publish_action()?;
        // No execution, allocation, or fallible validation remains in this
        // commit. The exclusive runtime retains every prepared address/owner.
        let mounted = self
            .mounts
            .get_mut(occurrence)
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
        Ok(())
    }
}
