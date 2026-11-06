//! Actual execution authority borrowed for one VM slice, never a saved owner.

use super::{
    AwbcProgram, AwbcRegisterId, FiberState, FiberStateError, VmError, VmExecutionContext,
};
use crate::effect::{
    RuntimeDropPolicyKind, RuntimePreparedRegistryMutation, RuntimeRegistryDropTarget,
    RuntimeRegistryOwner, RuntimeRegistryReadKind, RuntimeRegistryWriteSource,
    RuntimeScopeRegistry,
};
use crate::presentation::resources::{PresentationBanks, PresentationBody};
use crate::value::ownership::RuntimeValueOwnershipRequirement;
use crate::value::{RuntimePayload, RuntimeRetainedValueAdmission, RuntimeStoredValue};

#[derive(Default)]
pub(crate) struct VmExecutionResources<'execution> {
    pub(crate) context: Option<&'execution VmExecutionContext>,
    pub(crate) presentation:
        Option<&'execution mut Option<PresentationBanks<PresentationBody<RuntimePayload>>>>,
    pub(crate) actions: Option<&'execution mut crate::action::RuntimeActionMailbox>,
    pub(crate) dialogue_registry: Option<VmDialogueRegistryAccess<'execution>>,
}

/// Borrows the scope reached by a Product dialogue step. The enclosing
/// transaction publishes any write after the instruction and custody checks.
pub(crate) struct VmDialogueRegistryAccess<'a> {
    pub(crate) owner: RuntimeRegistryOwner,
    pub(crate) rows: &'a RuntimeScopeRegistry,
    pub(crate) admission: &'a mut RuntimeRetainedValueAdmission,
    pub(crate) pending: &'a mut Option<(RuntimeRegistryOwner, RuntimePreparedRegistryMutation)>,
}

impl VmDialogueRegistryAccess<'_> {
    fn prepare_write(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        effect: crate::awbc::schema::AwbcEffectPlanId,
        key: &arcweft_id::LifetimeRegistryKey,
        value_type: crate::awbc::schema::AwbcTypeId,
        args: &[AwbcRegisterId],
    ) -> Result<(), VmError> {
        if self.pending.is_some() {
            return Err(FiberStateError::InvalidFrame.into());
        }
        let [source_register] = args else {
            return Err(FiberStateError::InvalidFrame.into());
        };
        let ty = program
            .runtime_types
            .get(value_type.index())
            .ok_or(FiberStateError::InvalidFrame)?
            .semantic_identity();
        let source = match self.owner {
            RuntimeRegistryOwner::Dialogue(activation) => {
                RuntimeRegistryWriteSource::AwbcLine { effect, activation }
            }
            RuntimeRegistryOwner::AwbcFlow { fiber, frame } => RuntimeRegistryWriteSource::Awbc {
                effect,
                fiber,
                frame,
            },
            RuntimeRegistryOwner::NativeFlow => return Err(FiberStateError::InvalidFrame.into()),
        };
        let frame = fiber.active_frame()?;
        let prepared = self
            .rows
            .prepare_write(
                self.owner,
                key,
                ty,
                frame.type_instantiation.as_deref(),
                source,
                self.admission,
            )
            .map_err(FiberStateError::Snapshot)?;
        let transfer = frame.prepare_register_values(program, args)?;
        let update = prepared
            .capture(frame.register(*source_register)?)
            .map_err(FiberStateError::Snapshot)?;
        drop(transfer.commit(fiber.active_frame_mut()?));
        *self.pending = Some((self.owner, RuntimePreparedRegistryMutation::Write(update)));
        Ok(())
    }
}

impl VmExecutionResources<'_> {
    pub(super) fn registry_read(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        dst: AwbcRegisterId,
        key: &arcweft_id::LifetimeRegistryKey,
        kind: RuntimeRegistryReadKind,
    ) -> Result<(), VmError> {
        use arcweft_id::LifetimeScopeKind;
        let admitted = self
            .context
            .ok_or(VmError::MissingExecutionContext)?
            .admitted_program(program)
            .map_err(|_| VmError::MissingExecutionContext)?;
        let frame = fiber.active_frame()?;
        if !frame
            .registers
            .get(dst.index())
            .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
        {
            return Err(FiberStateError::InvalidFrame.into());
        }
        let expected = program
            .frame_layouts
            .get(frame.layout.index())
            .and_then(|layout| layout.slots.get(dst.index()))
            .and_then(|slot| program.runtime_types.get(slot.ty.index()))
            .ok_or(FiberStateError::InvalidFrame)?
            .semantic_identity();
        let binding = frame.type_instantiation.as_deref();
        let destination = match key.scope() {
            LifetimeScopeKind::Flow => fiber.reached_flow_frame_index(program),
            LifetimeScopeKind::Line => None,
            _ => return Err(VmError::MissingExecutionContext),
        };
        let value = if let Some(index) = destination {
            let mut admission = admitted
                .retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
            fiber
                .admit_retained(&mut admission)
                .map_err(FiberStateError::Snapshot)?;
            let reached = &fiber.frames[index];
            reached.flow_registry.read(
                RuntimeRegistryOwner::AwbcFlow {
                    fiber: fiber.instance,
                    frame: reached.instance,
                },
                key,
                kind,
                expected,
                binding,
                &mut admission,
            )
        } else {
            let reached = self
                .dialogue_registry
                .as_mut()
                .ok_or(VmError::MissingExecutionContext)?;
            reached.rows.read(
                reached.owner,
                key,
                kind,
                expected,
                binding,
                reached.admission,
            )
        }
        .map_err(FiberStateError::Snapshot)?;
        fiber.active_frame_mut()?.set_register(dst, value)?;
        Ok(())
    }

    pub(super) fn registry_write(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        effect: crate::awbc::schema::AwbcEffectPlanId,
        key: &arcweft_id::LifetimeRegistryKey,
        value_type: crate::awbc::schema::AwbcTypeId,
        args: &[AwbcRegisterId],
    ) -> Result<(), VmError> {
        use crate::effect::{
            RuntimeEffectMaterializeError, RuntimeRegistryOwner, RuntimeRegistryStateUpdate,
            RuntimeRegistryWriteSource,
        };

        let missing = RuntimeEffectMaterializeError::RegistryContextRequired;
        let context = self.context.ok_or(missing.clone())?;
        let admitted = context
            .admitted_program(program)
            .map_err(|_| missing.clone())?;
        if key.scope() == &arcweft_id::LifetimeScopeKind::Line {
            let line = self.dialogue_registry.as_mut().ok_or(missing.clone())?;
            return line.prepare_write(program, fiber, effect, key, value_type, args);
        }
        if key.scope() != &arcweft_id::LifetimeScopeKind::Flow {
            return Err(missing.into());
        }
        let Some(destination) = fiber.reached_flow_frame_index(program) else {
            let parent = self.dialogue_registry.as_mut().ok_or(missing.clone())?;
            return parent.prepare_write(program, fiber, effect, key, value_type, args);
        };
        let [source] = args else {
            return Err(FiberStateError::InvalidFrame.into());
        };
        let ty = program
            .runtime_types
            .get(value_type.index())
            .ok_or(missing)?
            .semantic_identity();
        let mut admission =
            admitted.retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        fiber
            .admit_retained(&mut admission)
            .map_err(FiberStateError::Snapshot)?;
        let result = FiberStateError::Snapshot;
        let write_source = RuntimeRegistryWriteSource::Awbc {
            effect,
            fiber: fiber.instance,
            frame: fiber.frames[destination].instance,
        };
        let frame = fiber.active_frame()?;
        let prepared = fiber.frames[destination]
            .flow_registry
            .prepare_write(
                RuntimeRegistryOwner::AwbcFlow {
                    fiber: fiber.instance,
                    frame: fiber.frames[destination].instance,
                },
                key,
                ty,
                frame.type_instantiation.as_deref(),
                write_source,
                &mut admission,
            )
            .map_err(result)?;
        let transfer = frame.prepare_register_values(program, args)?;
        let update = prepared.capture(frame.register(*source)?).map_err(result)?;
        update
            .commit(&mut fiber.frames[destination].flow_registry)
            .map_err(result)?;
        drop(
            transfer.commit(
                fiber
                    .active_frame_mut()
                    .expect("the validated frame is still active"),
            ),
        );
        Ok(())
    }

    pub(super) fn registry_drop(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        target: &RuntimeRegistryDropTarget<crate::awbc::schema::AwbcTypeId>,
        policy: RuntimeDropPolicyKind,
        args: &[AwbcRegisterId],
    ) -> Result<(), VmError> {
        use arcweft_id::LifetimeScopeKind;
        let missing = crate::effect::RuntimeEffectMaterializeError::RegistryContextRequired;
        let admitted = self
            .context
            .ok_or(missing.clone())?
            .admitted_program(program)
            .map_err(|_| missing.clone())?;
        let destination = match target.key().scope() {
            LifetimeScopeKind::Flow => fiber.reached_flow_frame_index(program),
            LifetimeScopeKind::Line => None,
            _ => return Err(missing.into()),
        };
        let ty = program
            .runtime_types
            .get(target.item_type().index())
            .ok_or(FiberStateError::InvalidFrame)?
            .semantic_identity();
        let frame = fiber.active_frame()?;
        let fade = match (policy, args) {
            (RuntimeDropPolicyKind::Stop, [fade]) => {
                let crate::value::RuntimeValue::Duration(fade) = frame.register(*fade)? else {
                    return Err(FiberStateError::InvalidFrame.into());
                };
                Some(*fade)
            }
            (RuntimeDropPolicyKind::Stop, _) | (_, [_, ..]) => {
                return Err(FiberStateError::InvalidFrame.into());
            }
            (_, []) => None,
        };
        let policy = policy
            .materialize(fade)
            .ok_or(FiberStateError::InvalidFrame)?;
        let transfer = frame.prepare_register_values(program, args)?;
        let binding = frame.type_instantiation.as_deref();
        if let Some(index) = destination {
            let mut admission = admitted
                .retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
            fiber
                .admit_retained(&mut admission)
                .map_err(FiberStateError::Snapshot)?;
            let reached = &fiber.frames[index];
            let removal = reached
                .flow_registry
                .prepare_drop(
                    RuntimeRegistryOwner::AwbcFlow {
                        fiber: fiber.instance,
                        frame: reached.instance,
                    },
                    target.key(),
                    target.presence(),
                    ty,
                    binding,
                    &mut admission,
                )
                .map_err(FiberStateError::Snapshot)?;
            removal
                .commit(&mut fiber.frames[index].flow_registry)
                .map_err(FiberStateError::Snapshot)?;
        } else {
            let reached = self.dialogue_registry.as_mut().ok_or(missing)?;
            if reached.pending.is_some() {
                return Err(FiberStateError::InvalidFrame.into());
            }
            let removal = reached
                .rows
                .prepare_drop(
                    reached.owner,
                    target.key(),
                    target.presence(),
                    ty,
                    binding,
                    reached.admission,
                )
                .map_err(FiberStateError::Snapshot)?;
            *reached.pending = Some((
                reached.owner,
                RuntimePreparedRegistryMutation::Drop(removal, policy),
            ));
        }
        drop(
            transfer.commit(
                fiber
                    .active_frame_mut()
                    .expect("the validated frame is still active"),
            ),
        );
        Ok(())
    }

    pub(super) fn receive_action(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        channel: AwbcRegisterId,
        dst: AwbcRegisterId,
    ) -> Result<Option<crate::pattern::RuntimeSemanticTypeId>, VmError> {
        let admitted = self
            .context
            .ok_or(VmError::ActionContextRequired)?
            .admitted_program(program)
            .map_err(|_| VmError::ActionContextRequired)?;
        let mailbox = self
            .actions
            .as_deref_mut()
            .ok_or(VmError::ActionContextRequired)?;
        let frame = fiber.active_frame()?;
        let layout = program
            .frame_layouts
            .get(frame.layout.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let semantic = |register: AwbcRegisterId| {
            layout
                .slots
                .get(register.index())
                .and_then(|slot| program.runtime_types.get(slot.ty.index()))
                .map(|ty| ty.semantic_identity())
                .ok_or(FiberStateError::InvalidFrame)
        };
        let channel_type = semantic(channel)?;
        let destination = semantic(dst)?;
        if !frame
            .registers
            .get(dst.index())
            .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
        {
            return Err(FiberStateError::InvalidFrame.into());
        }
        let mut admission =
            admitted.retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        let channel_value = frame.register(channel)?.view();
        let payload = admission
            .admit_composite(
                crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
                |owner, resources| {
                    owner
                        .expect("admitted execution owns its original program")
                        .types()
                        .action_receive_payload(
                            channel_value,
                            channel_type,
                            destination,
                            frame.type_instantiation.as_deref(),
                            resources,
                        )
                        .map_err(Into::into)
                },
            )
            .map_err(|cause| VmError::ActionAdmission(Box::new(cause)))?;
        fiber
            .admit_retained(&mut admission)
            .map_err(|cause| VmError::ActionAdmission(Box::new(cause)))?;
        if let Some(value) = mailbox.peek(payload) {
            admission
                .validate_owned_typed_borrowed_in_invocation(
                    value.borrowed_value(),
                    destination,
                    frame.type_instantiation.as_deref(),
                )
                .map_err(|cause| VmError::ActionAdmission(Box::new(cause)))?;
        }
        let value = mailbox
            .receive(payload, &mut admission)
            .map_err(|cause| VmError::ActionAdmission(Box::new(cause)))?;
        match value {
            Some(value) => {
                // The vacant destination and exclusive frame were checked before dequeue.
                fiber
                    .active_frame_mut()?
                    .set_register(dst, value.into_value())?;
                Ok(None)
            }
            None => Ok(Some(payload)),
        }
    }

    pub(super) fn emit_action(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        payload_type: crate::awbc::schema::AwbcTypeId,
        args: &[AwbcRegisterId],
    ) -> Result<(), VmError> {
        use crate::value::RuntimeStoredValue;
        let context = self.context.ok_or(VmError::ActionContextRequired)?;
        let admitted = context
            .admitted_program(program)
            .map_err(|_| VmError::ActionContextRequired)?;
        let mailbox = self
            .actions
            .as_deref_mut()
            .ok_or(VmError::ActionContextRequired)?;
        let [source] = args else {
            return Err(VmError::Runtime(
                "Action emission needs one original operand".into(),
            ));
        };
        let ty = program
            .runtime_types
            .get(payload_type.index())
            .ok_or(VmError::MissingExecutionContext)?
            .semantic_identity();
        let mut admission =
            admitted.retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        // Admission and capacity allocation finish before the owning register
        // can be moved. The VM retains this frame exclusively through commit.
        let prepared = mailbox
            .prepare_emit(
                ty,
                fiber.active_frame()?.register(*source)?.borrowed_value(),
                &mut admission,
            )
            .map_err(|error| VmError::ActionAdmission(Box::new(error)))?;
        let mut values = fiber
            .active_frame_mut()?
            .take_register_values(program, args)?;
        prepared.commit(RuntimePayload::new(
            values.pop().expect("the admitted unary operand"),
        ));
        Ok(())
    }

    pub(super) fn remove_presentation_slot(
        &mut self,
        program: &AwbcProgram,
        fiber: &FiberState,
        call: &crate::presentation::RuntimePresentationSlotCall,
        args: &[AwbcRegisterId],
        result: crate::pattern::RuntimeSemanticTypeId,
    ) -> Result<crate::value::RuntimeValue, VmError> {
        use crate::{
            presentation::resources::PresentationSlotRemovalAdmission,
            presentation::{RuntimePresentationError, RuntimePresentationSlotSelectors},
            value::RuntimeValue,
        };
        call.validate_execution(args.len())?;
        let context = self.context.ok_or(VmError::MissingExecutionContext)?;
        let admitted = context.admitted_program(program)?;
        if admitted.original_task_source().is_none() {
            return Err(VmError::MissingExecutionContext);
        }
        let owner = context.program_owner(program)?;
        let bank = fiber.presentation_bank();
        if !bank.matches_artifact(context.artifact()) {
            return Err(RuntimePresentationError::Program.into());
        }
        let storage = self
            .presentation
            .as_deref_mut()
            .ok_or(VmError::MissingExecutionContext)?;
        let frame = fiber.active_frame()?;
        let mut values = Vec::with_capacity(args.len());
        for register in args {
            values.push(frame.register(*register)?);
        }
        let mut admission =
            admitted.retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        let address = (|| {
            let address = RuntimePresentationSlotSelectors::from_values(
                call.inputs().iter().copied().zip(values.iter().copied()),
            )?
            .construct_address(call.resource(), &mut admission)?;
            let removal = PresentationSlotRemovalAdmission {
                bank,
                address: &address,
                result,
                binding: frame.type_instantiation.as_deref(),
            };
            if let Some(banks) = storage.as_ref() {
                banks.admit_retained_for_slot_removal(
                    &owner,
                    context.artifact(),
                    &mut admission,
                    Some(removal),
                )?;
            } else {
                admission.validate_option_preview(
                    None,
                    result,
                    removal.binding,
                    RuntimeValueOwnershipRequirement::Owned,
                )?;
            }
            fiber.admit_retained(&mut admission)?;
            admission.finish_presentation_custody()?;
            Ok::<_, crate::value::AwbcRuntimeValueSnapshotError>(address)
        })()
        .map_err(|cause| VmError::PresentationAdmission(Box::new(cause)))?;
        let removed = storage
            .as_mut()
            .map(|banks| banks.clear_slot(&owner, bank, &address))
            .transpose()?
            .flatten();
        Ok(removed.map_or_else(RuntimeValue::option_none, |body| {
            RuntimeValue::option_some(body.value.into_value())
        }))
    }

    pub(super) fn apply_presentation_handle_method(
        &mut self,
        program: &AwbcProgram,
        fiber: &FiberState,
        target: &crate::awbc::schema::AwbcValueTarget,
        method: arcweft_interaction_model::presentation::PresentationHandleMethodId,
    ) -> Result<(), VmError> {
        use crate::value::{RuntimeOpaqueValueClass, RuntimeValue};
        let error = |cause: &dyn std::fmt::Display| VmError::Runtime(cause.to_string());
        let context = self.context.ok_or(VmError::MissingExecutionContext)?;
        let admitted = context.admitted_program(program)?;
        let owner = context.program_owner(program)?;
        let banks = self
            .presentation
            .as_deref_mut()
            .and_then(Option::as_mut)
            .ok_or(VmError::MissingExecutionContext)?;
        let mut admission =
            admitted.retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        banks
            .admit_retained(&owner, context.artifact(), &mut admission)
            .map_err(|cause| error(&cause))?;
        fiber
            .admit_retained(&mut admission)
            .map_err(|cause| error(&cause))?;
        admission
            .finish_presentation_custody()
            .map_err(|cause| error(&cause))?;
        let frame = fiber.active_frame()?;
        let value = match target {
            crate::awbc::schema::AwbcValueTarget::Value(register) => frame.register(*register)?,
            crate::awbc::schema::AwbcValueTarget::Inspect(place)
            | crate::awbc::schema::AwbcValueTarget::Take(place) => frame
                .registers
                .get(place.base().index())
                .and_then(|storage| storage.field(place.fields()))
                .ok_or(FiberStateError::InvalidFrame)?,
        };
        let RuntimeValue::Opaque(handle) = value else {
            return Err(VmError::Runtime(
                "presentation method requires a handle".into(),
            ));
        };
        if handle.value_class() != RuntimeOpaqueValueClass::PresentationHandle {
            return Err(VmError::Runtime(
                "presentation method requires a handle".into(),
            ));
        }
        let token =
            crate::runtime_id::RuntimePresentationHandleToken::try_decode_payload(handle.payload())
                .map_err(|cause| error(&cause))?;
        let scope = banks.handle_scope(&owner, &token, fiber.presentation_scope_bindings())?;
        banks.apply_handle_method(&owner, &token, scope, method)?;
        Ok(())
    }

    /// Check the original execution authority before any operand is moved or
    /// copied out of its register. A context for equal bytes is not its lease.
    pub(super) fn validate_presentation_factory(
        &self,
        program: &AwbcProgram,
        call: &crate::presentation::RuntimePresentationFactoryCall,
        count: usize,
        result: crate::pattern::RuntimeSemanticTypeId,
    ) -> Result<(), VmError> {
        let error = |cause: &dyn std::fmt::Display| VmError::Runtime(cause.to_string());
        call.validate_argument_count(count)
            .map_err(|cause| error(&cause))?;
        call.result_owner(
            crate::program_types::RuntimeProgramTypes::Awbc(program),
            result,
        )
        .map_err(|cause| error(&cause))?;
        let context = self.context.ok_or(VmError::MissingExecutionContext)?;
        let admitted = context.admitted_program(program)?;
        if admitted.original_task_source().is_none() || self.presentation.is_none() {
            return Err(VmError::MissingExecutionContext);
        }
        Ok(())
    }

    pub(super) fn banks_mut(
        &mut self,
    ) -> Option<&mut PresentationBanks<PresentationBody<RuntimePayload>>> {
        self.presentation.as_deref_mut().and_then(Option::as_mut)
    }

    pub(super) fn validate_presentation_view(
        &self,
        program: &AwbcProgram,
        call: &crate::presentation::RuntimePresentationViewCall,
        count: usize,
        result: crate::pattern::RuntimeSemanticTypeId,
    ) -> Result<(), VmError> {
        call.validate_argument_count(count)
            .and_then(|()| {
                call.validate_result(
                    crate::program_types::RuntimeProgramTypes::Awbc(program),
                    result,
                )
            })
            .map_err(|cause| VmError::Runtime(cause.to_string()))?;
        let admitted = self
            .context
            .ok_or(VmError::MissingExecutionContext)?
            .admitted_program(program)?;
        if admitted.original_task_source().is_none() || self.presentation.is_none() {
            return Err(VmError::MissingExecutionContext);
        }
        Ok(())
    }

    pub(super) fn construct_presentation_view(
        &mut self,
        program: &AwbcProgram,
        fiber: &FiberState,
        call: &std::sync::Arc<crate::presentation::RuntimePresentationViewCall>,
        values: Vec<crate::value::RuntimeValue>,
        result: crate::pattern::RuntimeSemanticTypeId,
    ) -> Result<crate::value::RuntimeValue, VmError> {
        self.validate_presentation_view(program, call, values.len(), result)?;
        let context = self.context.ok_or(VmError::MissingExecutionContext)?;
        let admitted = context.admitted_program(program)?;
        let owner = context.program_owner(program)?;
        let source = admitted
            .original_task_source()
            .ok_or(VmError::MissingExecutionContext)?;
        let mut admission =
            admitted.retained_value_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        let error = |cause| VmError::PresentationAdmission(Box::new(cause));
        if let Some(banks) = self.presentation.as_deref().and_then(Option::as_ref) {
            banks
                .admit_retained(&owner, context.artifact(), &mut admission)
                .map_err(error)?;
        }
        fiber.admit_retained(&mut admission).map_err(error)?;
        let binding = fiber.active_frame()?.type_instantiation.clone();
        let value = admission
            .construct_presentation_resource(
                source.clone(),
                crate::presentation::RuntimePresentationResourceLayout::View(
                    std::sync::Arc::clone(call),
                ),
                binding,
                RuntimePayload::new(crate::value::RuntimeValue::Tuple(values)),
            )
            .map_err(error)?;
        admission.finish_presentation_custody().map_err(error)?;
        Ok(value)
    }

    pub(super) fn construct_presentation_factory(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        call: &std::sync::Arc<crate::presentation::RuntimePresentationFactoryCall>,
        values: Vec<crate::value::RuntimeValue>,
        result: crate::pattern::RuntimeSemanticTypeId,
    ) -> Result<crate::value::RuntimeValue, VmError> {
        use crate::{
            entry::RuntimeSchemaLimits, presentation::resources::PresentationLedgerLimits,
            value::RuntimeValue,
        };
        let error = |cause: &dyn std::fmt::Display| VmError::Runtime(cause.to_string());
        self.validate_presentation_factory(program, call, values.len(), result)?;
        let context = self.context.ok_or(VmError::MissingExecutionContext)?;
        let admitted = context.admitted_program(program)?;
        let owner = context.program_owner(program)?;
        let handle_owner = call
            .result_owner(owner.types(), result)
            .map_err(|cause| error(&cause))?;
        let source = admitted
            .original_task_source()
            .ok_or(VmError::MissingExecutionContext)?;
        let limits = RuntimeSchemaLimits::engine_default();
        let storage = self
            .presentation
            .as_deref_mut()
            .ok_or(VmError::MissingExecutionContext)?;
        let mut admission = admitted.retained_value_admission(limits);
        if let Some(banks) = storage.as_ref() {
            banks
                .admit_retained(&owner, context.artifact(), &mut admission)
                .map_err(|cause| error(&cause))?;
        }
        fiber
            .admit_retained(&mut admission)
            .map_err(|cause| error(&cause))?;
        let binding = fiber.active_frame()?.type_instantiation.clone();
        let body = RuntimeValue::Tuple(values);
        let site = call
            .registration_site(&mut admission, &body)
            .map_err(|cause| error(&cause))?;
        let resource = admission
            .construct_presentation_resource(
                source.clone(),
                crate::presentation::RuntimePresentationResourceLayout::Factory(
                    std::sync::Arc::clone(call),
                ),
                binding.clone(),
                RuntimePayload::new(body),
            )
            .map_err(|cause| error(&cause))?;
        let banks = storage.get_or_insert_with(|| {
            PresentationBanks::new(
                owner.clone(),
                context.artifact(),
                PresentationLedgerLimits::for_schema(limits),
            )
        });
        banks
            .preflight_registration_site(&owner, fiber.presentation_bank(), &site)
            .map_err(|cause| error(&cause))?;
        let ledger = banks
            .ledger_mut(fiber.presentation_bank())
            .map_err(|cause| error(&cause))?;
        ledger
            .preflight_register(&owner)
            .map_err(|cause| error(&cause))?;
        let scope = fiber
            .bind_presentation_scope(&owner, ledger)
            .map_err(|cause| error(&cause))?;
        let token = banks
            .register_and_retire(
                &owner,
                scope,
                site,
                PresentationBody {
                    expected: call.resource_type(),
                    binding,
                    value: RuntimePayload::new(resource),
                },
            )
            .map_err(|cause| error(&cause.cause))?;
        handle_owner
            .try_wrap(token.encode_payload())
            .map_err(|cause| error(&cause))
    }

    /// Admit every actual result resource while it still lives in its source
    /// register. A nested return receives it in the caller's current scope;
    /// a terminal result owns its detached original-bank lease.
    pub(super) fn exit_return_scope(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        registers: impl IntoIterator<Item = AwbcRegisterId>,
    ) -> Result<(), VmError> {
        fiber.validate_cleanup_inputs(program)?;
        let mut escaping = Self::escaping_tokens(fiber, registers)?;
        if let Some(return_to) = &fiber.active_frame()?.return_to {
            return_to.continuation.visit_runtime_values(|value| {
                if let crate::value::RuntimeValue::Opaque(opaque) = value
                    && opaque.value_class()
                        == crate::value::RuntimeOpaqueValueClass::PresentationHandle
                {
                    escaping.push(
                        crate::runtime_id::RuntimePresentationHandleToken::try_decode_payload(
                            opaque.payload(),
                        )
                        .map_err(|error| VmError::Runtime(error.to_string()))?,
                    );
                }
                Ok::<_, VmError>(())
            })?;
        }
        let (targets, destination) = fiber
            .presentation_return_scopes_mut()
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if targets.is_empty() && escaping.is_empty() {
            return Ok(());
        }
        let context = self.context;
        let banks = self.banks_mut().ok_or_else(|| {
            FiberStateError::Snapshot(
                crate::presentation::RuntimePresentationError::MissingLedger.into(),
            )
        })?;
        let owner = context
            .ok_or(VmError::MissingExecutionContext)?
            .program_owner(program)?;
        banks
            .transition_scope_contour(&owner, targets, destination, &escaping, &[])
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        Ok(())
    }

    pub(super) fn exit_goto_scope(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        replacement: &mut crate::awbc::fiber::PreparedFiberRootReplacement,
        registers: &[AwbcRegisterId],
    ) -> Result<(), VmError> {
        fiber.validate_cleanup_inputs(program)?;
        let escaping = Self::escaping_tokens(fiber, registers.iter().copied())?;
        let instance = fiber.instance;
        let targets = fiber
            .presentation_unwind_scopes_mut()
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if targets.is_empty() && escaping.is_empty() {
            return Ok(());
        }
        let context = self.context;
        let banks = self.banks_mut().ok_or_else(|| {
            FiberStateError::Snapshot(
                crate::presentation::RuntimePresentationError::MissingLedger.into(),
            )
        })?;
        let owner = context
            .ok_or(VmError::MissingExecutionContext)?
            .program_owner(program)?;
        banks
            .transition_scope_contour(
                &owner,
                targets,
                Some(crate::presentation::resources::PresentationScopeExit {
                    execution: crate::runtime_id::ExecutionInstanceId::from_allocated(
                        instance.get(),
                    ),
                    fiber: crate::runtime_id::RuntimePersistentFiberId::from_allocated(
                        instance.get().get(),
                    ),
                    scope: replacement.presentation_scope_target(),
                }),
                &escaping,
                &[],
            )
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        Ok(())
    }

    fn escaping_tokens(
        fiber: &FiberState,
        registers: impl IntoIterator<Item = AwbcRegisterId>,
    ) -> Result<Vec<crate::runtime_id::RuntimePresentationHandleToken>, VmError> {
        let mut escaping = Vec::new();
        for register in registers {
            for handle in fiber
                .active_frame()?
                .register(register)?
                .view()
                .affine_handles()
                .map_err(|error| VmError::Runtime(error.to_string()))?
            {
                if let crate::value::ownership::RuntimeAffineHandle::Presentation {
                    token, ..
                } = handle
                {
                    escaping.push(token);
                }
            }
        }
        Ok(escaping)
    }

    /// All cleanup declarations and actual scope grants precede body movement.
    /// Without a bank, the standalone VM can unwind resource-free frames only.
    pub(super) fn retire_unwind(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
    ) -> Result<(), VmError> {
        fiber.validate_cleanup_inputs(program)?;
        let targets = fiber
            .presentation_unwind_scopes_mut()
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if targets.is_empty() {
            return Ok(());
        }
        let context = self.context;
        let banks = self.banks_mut().ok_or_else(|| {
            FiberStateError::Snapshot(
                crate::presentation::RuntimePresentationError::MissingLedger.into(),
            )
        })?;
        let owner = context
            .ok_or(VmError::MissingExecutionContext)?
            .program_owner(program)?;
        banks
            .retire_scope_contour(&owner, targets)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
