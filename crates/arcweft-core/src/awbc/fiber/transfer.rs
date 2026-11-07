//! Original Flow transfer custody while invocation cleanup is still running.

use super::{
    AwbcEffectPlanId, AwbcFiberFrameSnapshot, AwbcFiberReturnPointSnapshot,
    AwbcRuntimeValueSnapshot, AwbcSaveResult, FiberCursor, FiberFrame, FiberReturnPoint,
    InertRuntimeValueStorage, RuntimeProgramOwner, RuntimeValue,
};
use super::{AwbcProgram, FiberState, FiberStateError, RuntimeStoredValue};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::value::{RuntimeScalarView, RuntimeValueView};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;

impl AwbcProgram {
    pub(in crate::awbc) fn flow_transfer_effect_target(
        &self,
        effect: super::AwbcEffectPlanId,
    ) -> Result<super::AwbcFunctionId, crate::awbc::vm::VmError> {
        use crate::awbc::schema::{AwbcConstant, AwbcEffectPayload};
        let Some(AwbcEffectPayload::Goto(args)) = self
            .effect_plans
            .get(effect.index())
            .map(|row| &row.payload)
        else {
            return Err(FiberStateError::InvalidFrame.into());
        };
        let Some(AwbcConstant::String(id)) =
            args.first().and_then(|id| self.constants.get(id.index()))
        else {
            return Err(FiberStateError::InvalidFrame.into());
        };
        self.resolve_flow_target_value(
            self.strings
                .get(id.index())
                .ok_or(FiberStateError::InvalidFrame)?,
        )
        .map(|(_, function)| function)
        .map_err(crate::awbc::vm::VmError::DynamicTarget)
    }
}

impl<V, B> FiberFlowTransferOrigin<V, B> {
    pub(super) fn source(&self) -> FiberCursor {
        match self {
            Self::Static { source }
            | Self::Dynamic { source, .. }
            | Self::Choice { source, .. }
            | Self::Effect { source, .. }
            | Self::ChoiceEffect { source, .. } => *source,
        }
    }
}

impl<V: RuntimeStoredValue, B: Borrow<RuntimeFunctionEffectInstantiation>, R, G>
    FiberPendingFlowTransfer<V, B, R, G>
{
    pub(super) fn validate_retained(
        &self,
        program: &AwbcProgram,
        resources: &mut ValueResourceAdmission,
    ) -> AwbcSaveResult<()> {
        use crate::awbc::schema::{
            AwbcFrameSlotRole, AwbcFunctionKind, AwbcInstruction, AwbcTerminator,
        };
        let invalid = || FiberStateError::InvalidFrame;
        let frame = &self.destination;
        let function = program
            .functions
            .get(frame.function.index())
            .ok_or_else(invalid)?;
        if function.kind != AwbcFunctionKind::Flow
            || program.flow_identity(frame.function).is_none()
            || frame.return_to.is_some()
            || frame.format.is_some()
            || !frame.format_attempts.is_empty()
            || !frame.root_cleanups.is_empty()
            || !frame.root_deferred.is_empty()
            || !frame.scopes.is_empty()
            || !frame.flow_registry.is_empty()
        {
            return Err(invalid().into());
        }
        let layout = super::validate_frame_layout(
            program,
            frame.function,
            frame.layout,
            frame.type_instantiation.as_ref().map(Borrow::borrow),
            frame.registers.len(),
            &frame.input_order,
            &resources.work(),
        )?;
        resources.work().collection(frame.registers.len())?;
        let mut arguments = Vec::new();
        for (slot, storage) in layout.slots.iter().zip(&frame.registers) {
            if slot.role == AwbcFrameSlotRole::Parameter {
                arguments.push(storage.as_ref().ok_or_else(invalid)?.borrowed_view());
            } else if !storage.is_vacant() {
                return Err(invalid().into());
            }
        }
        frame.validate_retained_values(
            program,
            FiberCursor {
                function: frame.function,
                block: function.entry_block,
                instruction_offset: 0,
            },
            resources,
            V::borrowed_view,
            "pending_flow_transfer.destination",
        )?;
        crate::awbc::vm::validate_function_input_ownership_views(
            program,
            frame.function,
            &arguments,
            &resources.work(),
        )
        .map_err(
            |error| crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            },
        )?;
        let source = self.origin.source();
        super::validate_cursor(program, source)?;
        let block = program
            .blocks
            .get(source.block.index())
            .ok_or_else(invalid)?;
        let at_terminator = source.instruction_offset == block.instructions.len;
        let choice_option = |option: u32| -> Result<_, FiberStateError> {
            let AwbcTerminator::Choice { choice, .. } = &block.terminator else {
                return Err(invalid());
            };
            let choice = program.choices.get(choice.index()).ok_or_else(invalid)?;
            if !at_terminator
                || option < choice.options.start
                || option >= choice.options.start.saturating_add(choice.options.len)
            {
                return Err(invalid());
            }
            program
                .choice_options
                .get(option as usize)
                .ok_or_else(invalid)
        };
        let target = match &self.origin {
            FiberFlowTransferOrigin::Static { .. } => {
                let AwbcTerminator::GotoStatic { function, .. } = &block.terminator else {
                    return Err(invalid().into());
                };
                if !at_terminator {
                    return Err(invalid().into());
                }
                *function
            }
            FiberFlowTransferOrigin::Dynamic {
                selector, binding, ..
            } => {
                let AwbcTerminator::GotoDynamic { target, .. } = &block.terminator else {
                    return Err(invalid().into());
                };
                if !at_terminator {
                    return Err(invalid().into());
                }
                let source_function = program
                    .functions
                    .get(source.function.index())
                    .ok_or_else(invalid)?;
                let binding = binding.as_ref().map(Borrow::borrow);
                match (source_function.type_context, binding) {
                    (None, None) => {}
                    (Some(context), Some(binding))
                        if program
                            .runtime_types
                            .get(context.owner_type().index())
                            .is_some_and(|row| row.semantic_identity() == binding.context())
                            && binding.is_valid_with_work(program, &resources.work()) => {}
                    _ => return Err(invalid().into()),
                }
                if let Some(binding) = binding {
                    resources.function_instantiation(
                        binding,
                        crate::entry::schema::value_budget::ResourceByteKind::Metadata,
                    )?;
                }
                let slot = program
                    .frame_layouts
                    .get(source_function.frame_layout.index())
                    .and_then(|layout| layout.slots.get(target.index()))
                    .ok_or_else(invalid)?;
                crate::program_types::RuntimeProgramValueType::Awbc {
                    program,
                    ty: slot.ty,
                }
                .validate_view(selector.borrowed_view(), binding, resources)?;
                match selector.borrowed_view() {
                    RuntimeValueView::Scalar(RuntimeScalarView::String(label)) => program
                        .resolve_flow_target_value(label)
                        .map(|(_, function)| function)
                        .map_err(|_| invalid())?,
                    RuntimeValueView::Scalar(RuntimeScalarView::EntityRef(entity)) => entity
                        .exact_flow_identity()
                        .and_then(|flow| program.flow_function(flow))
                        .ok_or_else(invalid)?,
                    _ => return Err(invalid().into()),
                }
            }
            FiberFlowTransferOrigin::Choice { option, .. } => {
                choice_option(*option)?.target.ok_or_else(invalid)?
            }
            FiberFlowTransferOrigin::Effect { effect, .. } => {
                let instruction = source
                    .instruction_offset
                    .checked_add(block.instructions.start)
                    .and_then(|index| program.instructions.get(index as usize));
                if !matches!(instruction, Some(AwbcInstruction::EmitEffect { effect: reached, .. }) if reached == effect)
                    || at_terminator
                {
                    return Err(invalid().into());
                }
                program
                    .flow_transfer_effect_target(*effect)
                    .map_err(|_| invalid())?
            }
            FiberFlowTransferOrigin::ChoiceEffect { option, effect, .. } => {
                let option = choice_option(*option)?;
                if !option.effects.contains(effect) && option.out_effect != Some(*effect) {
                    return Err(invalid().into());
                }
                program
                    .flow_transfer_effect_target(*effect)
                    .map_err(|_| invalid())?
            }
        };
        if target != frame.function {
            return Err(invalid().into());
        }
        Ok(())
    }
}

impl FiberState {
    pub(in crate::awbc) fn stage_flow_transfer(
        &mut self,
        mut prepared: super::PreparedFiberRootReplacement,
        args: Vec<RuntimeValue>,
        origin: FiberFlowTransferOrigin<
            RuntimeValue,
            std::sync::Arc<RuntimeFunctionEffectInstantiation>,
        >,
    ) {
        assert!(self.pending_flow_transfer.is_none());
        assert_eq!(self.instance, prepared.owner);
        assert_eq!(args.len(), prepared.parameters.len());
        for (register, value) in prepared.parameters.iter().zip(args) {
            prepared.frame.registers[register.index()] = value.into();
        }
        self.next_frame_instance = prepared.next_frame_instance;
        self.discard_format_attempts();
        self.pending_flow_transfer = Some(FiberPendingFlowTransfer {
            origin,
            destination: prepared.frame,
        });
    }

    pub(in crate::awbc) fn complete_flow_transfer(
        &mut self,
        program: &AwbcProgram,
    ) -> Result<super::AwbcFunctionId, FiberStateError> {
        if self.frames.len() != 1
            || !self.active_frame()?.scopes.is_empty()
            || !self.active_frame()?.root_deferred.is_empty()
            || !self.active_frame()?.root_cleanups.is_empty()
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let pending = self
            .pending_flow_transfer
            .take()
            .ok_or(FiberStateError::InvalidFrame)?;
        let function = pending.destination.function;
        self.cursor = FiberCursor {
            function,
            block: program.functions[function.index()].entry_block,
            instruction_offset: 0,
        };
        self.frames = vec![pending.destination];
        Ok(function)
    }
}

/// The checked operation that selected the actual next Flow. Dynamic targets
/// retain their original value and generic context after their source frame
/// leaves the active stack; static targets remain owned by program metadata.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: InertRuntimeValueStorage, B: Serialize",
        deserialize = "V: InertRuntimeValueStorage, B: Deserialize<'de>"
    )
)]
pub(in crate::awbc) enum FiberFlowTransferOrigin<V, B> {
    Static {
        source: FiberCursor,
    },
    Dynamic {
        source: FiberCursor,
        selector: V,
        binding: Option<B>,
    },
    Choice {
        source: FiberCursor,
        option: u32,
    },
    Effect {
        source: FiberCursor,
        effect: AwbcEffectPlanId,
    },
    ChoiceEffect {
        source: FiberCursor,
        option: u32,
        effect: AwbcEffectPlanId,
    },
}

impl<V: InertRuntimeValueStorage + Clone, B: Clone> Clone for FiberFlowTransferOrigin<V, B> {
    fn clone(&self) -> Self {
        match self {
            Self::Static { source } => Self::Static { source: *source },
            Self::Dynamic {
                source,
                selector,
                binding,
            } => Self::Dynamic {
                source: *source,
                selector: selector.clone(),
                binding: binding.clone(),
            },
            Self::Choice { source, option } => Self::Choice {
                source: *source,
                option: *option,
            },
            Self::Effect { source, effect } => Self::Effect {
                source: *source,
                effect: *effect,
            },
            Self::ChoiceEffect {
                source,
                option,
                effect,
            } => Self::ChoiceEffect {
                source: *source,
                option: *option,
                effect: *effect,
            },
        }
    }
}

/// The real destination invocation owns transferred arguments while the old
/// stack drains. It has not executed instructions or registered any cleanup.
/// The same grammar stores live values and the public inert save projection.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: InertRuntimeValueStorage, B: Serialize, FiberFrame<V, B, R, G>: Serialize",
        deserialize = "V: InertRuntimeValueStorage, B: Deserialize<'de>, FiberFrame<V, B, R, G>: Deserialize<'de>"
    )
)]
pub(in crate::awbc) struct FiberPendingFlowTransfer<
    V = RuntimeValue,
    B = std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>,
    R = FiberReturnPoint,
    G = crate::presentation::RuntimePresentationScopeBinding,
> {
    pub(in crate::awbc) origin: FiberFlowTransferOrigin<V, B>,
    pub(in crate::awbc) destination: FiberFrame<V, B, R, G>,
}

impl<V: InertRuntimeValueStorage + Clone, B: Clone, R: Clone, G: Clone> Clone
    for FiberPendingFlowTransfer<V, B, R, G>
{
    fn clone(&self) -> Self {
        Self {
            origin: self.origin.clone(),
            destination: self.destination.clone(),
        }
    }
}

pub(super) type AwbcPendingFlowTransferSnapshot = FiberPendingFlowTransfer<
    AwbcRuntimeValueSnapshot,
    crate::program_types::RuntimeFunctionEffectInstantiation,
    AwbcFiberReturnPointSnapshot,
    crate::presentation::RuntimePresentationScopeSnapshot,
>;

impl FiberPendingFlowTransfer {
    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<AwbcPendingFlowTransferSnapshot> {
        use FiberFlowTransferOrigin as Origin;
        let origin = match &self.origin {
            Origin::Static { source } => Origin::Static { source: *source },
            Origin::Dynamic {
                source,
                selector,
                binding,
            } => Origin::Dynamic {
                source: *source,
                selector: AwbcRuntimeValueSnapshot::from_admitted_runtime_value(selector, owner)?,
                binding: binding.as_deref().cloned(),
            },
            Origin::Choice { source, option } => Origin::Choice {
                source: *source,
                option: *option,
            },
            Origin::Effect { source, effect } => Origin::Effect {
                source: *source,
                effect: *effect,
            },
            Origin::ChoiceEffect {
                source,
                option,
                effect,
            } => Origin::ChoiceEffect {
                source: *source,
                option: *option,
                effect: *effect,
            },
        };
        Ok(AwbcPendingFlowTransferSnapshot {
            origin,
            destination: AwbcFiberFrameSnapshot::from_live(&self.destination, owner)?,
        })
    }
}

impl AwbcPendingFlowTransferSnapshot {
    pub(super) fn into_live_admitted<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> AwbcSaveResult<FiberPendingFlowTransfer> {
        use FiberFlowTransferOrigin as Origin;
        let owner = owner.into();
        let origin = match self.origin {
            Origin::Static { source } => Origin::Static { source },
            Origin::Dynamic {
                source,
                selector,
                binding,
            } => Origin::Dynamic {
                source,
                selector: selector.into_admitted_runtime_value(owner)?,
                binding: binding.map(std::sync::Arc::new),
            },
            Origin::Choice { source, option } => Origin::Choice { source, option },
            Origin::Effect { source, effect } => Origin::Effect { source, effect },
            Origin::ChoiceEffect {
                source,
                option,
                effect,
            } => Origin::ChoiceEffect {
                source,
                option,
                effect,
            },
        };
        Ok(FiberPendingFlowTransfer {
            origin,
            destination: self.destination.into_live(owner)?,
        })
    }
}
