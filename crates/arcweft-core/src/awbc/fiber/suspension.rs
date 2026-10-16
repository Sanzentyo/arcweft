//! Original suspension authority over one live/inert grammar.

use super::{
    AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcRuntimeValueSnapshot, AwbcSaveResult,
    FiberAwaitTarget, FiberCursor, FiberDialogueContentEffectBinding, FiberResumeTarget,
    FiberState, FiberStateError, FiberSuspension, FiberSuspensionReason, FrameValueContext,
    RuntimeDialogueValueBinding, RuntimeProgramOwner, RuntimeValue,
    dialogue_target_type_matches_program, validate_cursor,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::RuntimeProgramValueType;
use crate::task::RuntimeNeedHandleSaveSnapshot;
use crate::value::{RuntimeNeedView, RuntimeValueView};

impl<V, O, N> FiberSuspension<V, O, N> {
    #[expect(
        clippy::too_many_lines,
        reason = "One closed suspension algebra authenticates original sites before retained value construction"
    )]
    pub(super) fn validate_inputs(
        &self,
        frame: &FrameValueContext<'_, V>,
        cursor: FiberCursor,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_> + Copy,
        opaque: impl Fn(&O) -> RuntimeValueView<'_> + Copy,
        need: impl Fn(&N) -> RuntimeNeedView<'_> + Copy,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .charge(0)
            .map_err(|e| FiberStateError::Snapshot(e.into()))?;
        validate_cursor(frame.program, cursor)?;
        if cursor.function != frame.function {
            return Err(FiberStateError::InvalidFrame);
        }
        let resume = match self.resume {
            FiberResumeTarget::Exact(exact) => {
                if !matches!(self.reason, FiberSuspensionReason::BudgetYield) || exact != cursor {
                    return Err(FiberStateError::InvalidFrame);
                }
                return Ok(());
            }
            FiberResumeTarget::Declared(id) => {
                let point = frame
                    .program
                    .resume_points
                    .get(id.index())
                    .ok_or(FiberStateError::UnknownResumePoint(id.0))?;
                let function = frame
                    .program
                    .functions
                    .get(frame.function.index())
                    .ok_or(FiberStateError::InvalidFrame)?;
                if point.function != frame.function {
                    return Err(FiberStateError::ResumeFunctionMismatch {
                        resume: id.0,
                        actual: point.function.0,
                        expected: frame.function.0,
                    });
                }
                if point.frame_layout != function.frame_layout {
                    return Err(FiberStateError::ResumeLayoutMismatch {
                        resume: id.0,
                        actual: point.frame_layout.0,
                        expected: function.frame_layout.0,
                    });
                }
                id
            }
        };
        let block = frame
            .program
            .blocks
            .get(cursor.block.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        if cursor.instruction_offset != block.instructions.len {
            return Err(FiberStateError::InvalidFrame);
        }
        match (&self.reason, &block.terminator) {
            (
                FiberSuspensionReason::BudgetYield,
                super::super::schema::AwbcTerminator::BudgetYield { resume: original },
            ) if *original == resume => Ok(()),
            (
                FiberSuspensionReason::Choice {
                    choice,
                    destination,
                },
                super::super::schema::AwbcTerminator::Choice {
                    choice: original,
                    dst,
                    resume: original_resume,
                },
            ) if choice == original && destination == dst && resume == *original_resume => {
                if frame.program.choices.get(choice.index()).is_none()
                    || frame.layout.slots.get(destination.index()).is_none()
                {
                    return Err(FiberStateError::InvalidFrame);
                }
                Ok(())
            }
            (
                FiberSuspensionReason::Await {
                    target:
                        FiberAwaitTarget::Need {
                            need: retained,
                            item_type,
                            handle,
                        },
                    binding,
                    observer,
                },
                super::super::schema::AwbcTerminator::Await {
                    handle: original,
                    binding: original_binding,
                    observer: original_observer,
                    resume: original_resume,
                },
            ) if handle == original
                && binding == original_binding
                && observer == original_observer
                && resume == *original_resume =>
            {
                let handle_view = need(retained);
                handle_view.validate_identity().map_err(|e| {
                    FiberStateError::Snapshot(
                        crate::value::AwbcRuntimeValueSnapshotError::Message {
                            message: e.to_string(),
                        },
                    )
                })?;
                let ty = frame
                    .layout
                    .slots
                    .get(handle.index())
                    .ok_or(FiberStateError::InvalidFrame)?
                    .ty;
                if !matches!(frame.program.runtime_types.get(ty.index()).map(AwbcRuntimeType::shape), Some(AwbcRuntimeTypeShape::Need(item)) if item == item_type)
                    || frame
                        .registers
                        .get(handle.index())
                        .is_none_or(|r| r.values().next().is_some())
                {
                    return Err(FiberStateError::InvalidRuntimeValue {
                        path: "suspension.await.target".into(),
                        reason: "Need identity or selected item type disagrees with its consumed handle register".into(),
                    });
                }
                if binding.is_some_and(|p| frame.program.patterns.get(p.index()).is_none()) {
                    return Err(FiberStateError::InvalidFrame);
                }
                if let Some(observer) = observer {
                    let point = frame
                        .program
                        .resume_points
                        .get(observer.resume.index())
                        .ok_or(FiberStateError::InvalidFrame)?;
                    if frame
                        .layout
                        .slots
                        .get(observer.destination.index())
                        .is_none()
                        || point.function != frame.function
                        || point.frame_layout
                            != frame.program.functions[frame.function.index()].frame_layout
                    {
                        return Err(FiberStateError::InvalidFrame);
                    }
                }
                RuntimeProgramValueType::Awbc {
                    program: frame.program,
                    ty,
                }
                .validate_view(
                    RuntimeValueView::Need(handle_view),
                    frame.binding,
                    resources,
                )
                .map_err(|e| {
                    FiberStateError::Snapshot(
                        crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(e)),
                    )
                })
            }
            (
                FiberSuspensionReason::AwaitMany(state),
                super::super::schema::AwbcTerminator::AwaitMany { .. },
            ) => state.validate_inputs(frame, cursor, self.resume, resources, view, need),
            (
                FiberSuspensionReason::HostCall {
                    call,
                    args,
                    destination,
                },
                super::super::schema::AwbcTerminator::HostCall {
                    call: original,
                    args: registers,
                    dst,
                    resume: original_resume,
                },
            ) if call == original && destination == dst && resume == *original_resume => {
                resources
                    .work()
                    .collection(args.len())
                    .map_err(|e| FiberStateError::Snapshot(e.into()))?;
                let call = frame
                    .program
                    .host_calls
                    .get(call.index())
                    .ok_or(FiberStateError::InvalidFrame)?;
                let signature = frame
                    .program
                    .signatures
                    .get(call.signature.index())
                    .ok_or(FiberStateError::InvalidFrame)?;
                if args.len() != registers.len()
                    || args.len() != signature.params.len()
                    || !matches!(
                        (signature.result, destination),
                        (None, None) | (Some(_), Some(_))
                    )
                {
                    return Err(FiberStateError::InvalidFrame);
                }
                if let (Some(expected), Some(destination)) = (signature.result, destination)
                    && frame
                        .layout
                        .slots
                        .get(destination.index())
                        .is_none_or(|slot| slot.ty != expected)
                {
                    return Err(FiberStateError::InvalidFrame);
                }
                for ((value, register), expected) in
                    args.iter().zip(registers).zip(&signature.params)
                {
                    let value = view(value);
                    value.admit_ownership(
                        crate::value::ownership::RuntimeValueOwnershipRequirement::Unrestricted,
                        resources,
                        0,
                    )?;
                    for ty in [
                        frame
                            .layout
                            .slots
                            .get(register.index())
                            .ok_or(FiberStateError::InvalidFrame)?
                            .ty,
                        *expected,
                    ] {
                        RuntimeProgramValueType::Awbc {
                            program: frame.program,
                            ty,
                        }
                        .validate_admitted_view(value, frame.binding, resources)
                        .map_err(|e| {
                            FiberStateError::Snapshot(
                                crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(
                                    e,
                                )),
                            )
                        })?;
                    }
                }
                Ok(())
            }
            (
                FiberSuspensionReason::Dialogue { .. },
                super::super::schema::AwbcTerminator::Dialogue {
                    resume: original, ..
                },
            ) if resume == *original => self.reason.validate_dialogue_inputs(
                frame,
                &block.terminator,
                resources,
                view,
                opaque,
            ),
            _ => Err(FiberStateError::InvalidFrame),
        }
    }
}

impl<V, O, N> FiberSuspensionReason<V, O, N> {
    #[expect(
        clippy::too_many_lines,
        reason = "Dialogue admits its complete source and consumer inventory through one shared resource budget"
    )]
    fn validate_dialogue_inputs(
        &self,
        frame: &FrameValueContext<'_, V>,
        source: &super::super::schema::AwbcTerminator,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_> + Copy,
        opaque: impl Fn(&O) -> RuntimeValueView<'_> + Copy,
    ) -> Result<(), FiberStateError> {
        let Self::Dialogue {
            target,
            target_type,
            content,
            values,
            effects,
            line_task_captures,
            result,
        } = self
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        let super::super::schema::AwbcTerminator::Dialogue {
            target: target_register,
            content: original_content,
            values: originals,
            effects: original_effects,
            line_task_captures: capture_registers,
            result: original_result,
            ..
        } = source
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        let program = frame.program;
        if content != original_content
            || result != original_result
            || values.len() != originals.len()
            || effects.len() != original_effects.len()
            || line_task_captures.len() != capture_registers.len()
            || frame
                .layout
                .slots
                .get(target_register.index())
                .is_none_or(|s| s.ty != *target_type)
            || !dialogue_target_type_matches_program(program, *target_type)
            || frame
                .layout
                .slots
                .get(result.destination.index())
                .is_none_or(|s| s.ty != result.ty)
            || program.patterns.get(result.pattern.index()).is_none()
        {
            return Err(FiberStateError::InvalidFrame);
        }
        for count in [
            values.len(),
            effects.len(),
            line_task_captures.len(),
            program.content_templates.len(),
        ] {
            resources
                .work()
                .collection(count)
                .map_err(|e| FiberStateError::Snapshot(e.into()))?;
        }
        if let Some(target) = target {
            RuntimeProgramValueType::Awbc {
                program,
                ty: *target_type,
            }
            .validate_view(opaque(target), frame.binding, resources)
            .map_err(|e| {
                FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(
                    Box::new(e),
                ))
            })?;
        }
        let content = program
            .content_units
            .get(content.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let mut template = None;
        for candidate in &program.content_templates {
            resources
                .work()
                .charge(0)
                .map_err(|e| FiberStateError::Snapshot(e.into()))?;
            if candidate.id == content.template && template.replace(candidate).is_some() {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        let template = template.ok_or(FiberStateError::InvalidFrame)?;
        if template.slots.len() != values.len() || template.effects.len() != effects.len() {
            return Err(FiberStateError::InvalidFrame);
        }
        for ((binding, original), declared) in values.iter().zip(originals).zip(&template.slots) {
            let role = match original.role {
                super::super::schema::AwbcDialogueValueRole::Interpolation => {
                    crate::plan::RuntimeDialogueValueRole::Interpolation
                }
                super::super::schema::AwbcDialogueValueRole::Content => {
                    crate::plan::RuntimeDialogueValueRole::Content
                }
                super::super::schema::AwbcDialogueValueRole::Formatted => {
                    crate::plan::RuntimeDialogueValueRole::Formatted
                }
            };
            if binding.slot != original.slot
                || binding.slot != declared.slot
                || binding.role != role
                || original.role != declared.role
            {
                return Err(FiberStateError::InvalidFrame);
            }
            let value = view(&binding.value);
            resources
                .validate_view(value, 0, None)
                .map_err(|e| FiberStateError::Snapshot(e.into()))?;
            for ty in [
                frame
                    .layout
                    .slots
                    .get(original.value.index())
                    .ok_or(FiberStateError::InvalidFrame)?
                    .ty,
                declared.semantic_type,
            ] {
                RuntimeProgramValueType::Awbc { program, ty }
                    .validate_admitted_view(value, frame.binding, resources)
                    .map_err(|e| {
                        FiberStateError::Snapshot(
                            crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(e)),
                        )
                    })?;
            }
        }
        for ((effect, original), declared) in
            effects.iter().zip(original_effects).zip(&template.effects)
        {
            let state = program
                .callable_states
                .get(effect.state.index())
                .ok_or(FiberStateError::InvalidFrame)?;
            resources
                .work()
                .collection(effect.captures.len())
                .map_err(|e| FiberStateError::Snapshot(e.into()))?;
            if effect.site != original.site
                || effect.site != declared.site
                || effect.state != original.state
                || effect.captures.len() != original.captures.len()
                || effect.captures.len() != declared.capture_types.len()
                || state.retained.len() != effect.captures.len()
            {
                return Err(FiberStateError::InvalidFrame);
            }
            for (((value, register), expected), retained) in effect
                .captures
                .iter()
                .zip(&original.captures)
                .zip(&declared.capture_types)
                .zip(&state.retained)
            {
                let value = view(value);
                resources
                    .validate_view(value, 0, None)
                    .map_err(|e| FiberStateError::Snapshot(e.into()))?;
                for ty in [
                    frame
                        .layout
                        .slots
                        .get(register.index())
                        .ok_or(FiberStateError::InvalidFrame)?
                        .ty,
                    *expected,
                    retained.ty,
                ] {
                    RuntimeProgramValueType::Awbc { program, ty }
                        .validate_admitted_view(value, frame.binding, resources)
                        .map_err(|e| {
                            FiberStateError::Snapshot(
                                crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(
                                    e,
                                )),
                            )
                        })?;
                }
            }
        }
        match content.line_task_group {
            None if line_task_captures.is_empty() => {}
            Some(id) => {
                let group = program
                    .line_task_groups
                    .get(id.index())
                    .ok_or(FiberStateError::InvalidFrame)?;
                let signature = program
                    .functions
                    .get(group.activation.index())
                    .and_then(|f| program.signatures.get(f.signature.index()))
                    .ok_or(FiberStateError::InvalidFrame)?;
                if group.captures.len() != line_task_captures.len()
                    || signature.params.len() != line_task_captures.len()
                {
                    return Err(FiberStateError::InvalidFrame);
                }
                for ((value, register), expected) in line_task_captures
                    .iter()
                    .zip(capture_registers)
                    .zip(&signature.params)
                {
                    let value = view(value);
                    value.admit_ownership(
                        crate::value::ownership::RuntimeValueOwnershipRequirement::Unrestricted,
                        resources,
                        0,
                    )?;
                    for ty in [
                        frame
                            .layout
                            .slots
                            .get(register.index())
                            .ok_or(FiberStateError::InvalidFrame)?
                            .ty,
                        *expected,
                    ] {
                        RuntimeProgramValueType::Awbc { program, ty }
                            .validate_admitted_view(value, frame.binding, resources)
                            .map_err(|e| {
                                FiberStateError::Snapshot(
                                    crate::value::AwbcRuntimeValueSnapshotError::ProgramType(
                                        Box::new(e),
                                    ),
                                )
                            })?;
                    }
                }
            }
            _ => return Err(FiberStateError::InvalidFrame),
        }
        Ok(())
    }
}
impl FiberSuspension {
    pub(super) fn validate_state(
        &self,
        program: &AwbcProgram,
        state: &FiberState,
    ) -> Result<(), FiberStateError> {
        let frame = state.active_frame()?;
        self.validate_inputs(
            &FrameValueContext {
                program,
                function: frame.function,
                layout: program
                    .frame_layouts
                    .get(frame.layout.index())
                    .ok_or(FiberStateError::InvalidFrame)?,
                binding: frame.type_instantiation.as_deref(),
                registers: &frame.registers,
                scope_count: frame.scopes.len(),
            },
            state.cursor,
            &mut ValueResourceAdmission::live(crate::entry::RuntimeSchemaLimits::engine_default()),
            RuntimeValue::view,
            crate::value::RuntimeOpaqueValue::view,
            crate::task::RuntimeNeedHandle::view,
        )
    }

    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<
        FiberSuspension<
            AwbcRuntimeValueSnapshot,
            AwbcRuntimeValueSnapshot,
            RuntimeNeedHandleSaveSnapshot,
        >,
    > {
        Ok(FiberSuspension {
            resume: self.resume,
            reason: self.reason.snapshot_admitted(owner)?,
        })
    }
}

impl FiberSuspensionReason {
    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<
        FiberSuspensionReason<
            AwbcRuntimeValueSnapshot,
            AwbcRuntimeValueSnapshot,
            RuntimeNeedHandleSaveSnapshot,
        >,
    > {
        let value =
            |v: &RuntimeValue| AwbcRuntimeValueSnapshot::from_admitted_runtime_value(v, owner);
        Ok(match self {
            Self::Dialogue {
                target,
                target_type,
                content,
                values,
                effects,
                line_task_captures,
                result,
            } => FiberSuspensionReason::Dialogue {
                target: target
                    .as_ref()
                    .map(|v| AwbcRuntimeValueSnapshot::from_admitted_opaque(v, owner))
                    .transpose()?,
                target_type: *target_type,
                content: *content,
                values: values
                    .iter()
                    .map(|b| {
                        Ok(RuntimeDialogueValueBinding {
                            slot: b.slot,
                            role: b.role,
                            value: value(&b.value)?,
                        })
                    })
                    .collect::<AwbcSaveResult<Vec<_>>>()?
                    .into_boxed_slice(),
                effects: effects
                    .iter()
                    .map(|e| {
                        Ok(FiberDialogueContentEffectBinding {
                            site: e.site,
                            state: e.state,
                            captures: e
                                .captures
                                .iter()
                                .map(value)
                                .collect::<Result<Vec<_>, _>>()?
                                .into_boxed_slice(),
                        })
                    })
                    .collect::<AwbcSaveResult<Vec<_>>>()?
                    .into_boxed_slice(),
                line_task_captures: line_task_captures
                    .iter()
                    .map(value)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
                result: result.clone(),
            },
            Self::Choice {
                choice,
                destination,
            } => FiberSuspensionReason::Choice {
                choice: *choice,
                destination: *destination,
            },
            Self::Await {
                target:
                    FiberAwaitTarget::Need {
                        need,
                        item_type,
                        handle,
                    },
                binding,
                observer,
            } => FiberSuspensionReason::Await {
                target: FiberAwaitTarget::Need {
                    need: RuntimeNeedHandleSaveSnapshot::from_live(need, Some(owner))?,
                    item_type: *item_type,
                    handle: *handle,
                },
                binding: *binding,
                observer: *observer,
            },
            Self::AwaitMany(state) => {
                FiberSuspensionReason::AwaitMany(state.snapshot_admitted(owner)?)
            }
            Self::HostCall {
                call,
                args,
                destination,
            } => FiberSuspensionReason::HostCall {
                call: *call,
                args: args.iter().map(value).collect::<Result<_, _>>()?,
                destination: *destination,
            },
            Self::BudgetYield => FiberSuspensionReason::BudgetYield,
        })
    }
}

impl
    FiberSuspension<
        AwbcRuntimeValueSnapshot,
        AwbcRuntimeValueSnapshot,
        RuntimeNeedHandleSaveSnapshot,
    >
{
    pub(super) fn into_live_admitted(
        self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberSuspension> {
        Ok(FiberSuspension {
            resume: self.resume,
            reason: self.reason.into_live_admitted(owner)?,
        })
    }
}

impl
    FiberSuspensionReason<
        AwbcRuntimeValueSnapshot,
        AwbcRuntimeValueSnapshot,
        RuntimeNeedHandleSaveSnapshot,
    >
{
    pub(super) fn into_live_admitted(
        self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberSuspensionReason> {
        let value = |v: AwbcRuntimeValueSnapshot| v.into_admitted_runtime_value(owner);
        Ok(match self {
            Self::Dialogue {
                target,
                target_type,
                content,
                values,
                effects,
                line_task_captures,
                result,
            } => FiberSuspensionReason::Dialogue {
                target: target
                    .map(|v| {
                        let RuntimeValue::Opaque(target) = value(v)? else {
                            return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                                message: "admitted dialogue target is not opaque".into(),
                            });
                        };
                        Ok(target)
                    })
                    .transpose()?,
                target_type,
                content,
                values: values
                    .into_iter()
                    .map(|b| {
                        Ok(RuntimeDialogueValueBinding {
                            slot: b.slot,
                            role: b.role,
                            value: value(b.value)?,
                        })
                    })
                    .collect::<AwbcSaveResult<Vec<_>>>()?
                    .into_boxed_slice(),
                effects: effects
                    .into_iter()
                    .map(|e| {
                        Ok(FiberDialogueContentEffectBinding {
                            site: e.site,
                            state: e.state,
                            captures: e
                                .captures
                                .into_iter()
                                .map(value)
                                .collect::<Result<Vec<_>, _>>()?
                                .into_boxed_slice(),
                        })
                    })
                    .collect::<AwbcSaveResult<Vec<_>>>()?
                    .into_boxed_slice(),
                line_task_captures: line_task_captures
                    .into_iter()
                    .map(value)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
                result,
            },
            Self::Choice {
                choice,
                destination,
            } => FiberSuspensionReason::Choice {
                choice,
                destination,
            },
            Self::Await {
                target:
                    FiberAwaitTarget::Need {
                        need,
                        item_type,
                        handle,
                    },
                binding,
                observer,
            } => FiberSuspensionReason::Await {
                target: FiberAwaitTarget::Need {
                    need: need.into_live(owner)?,
                    item_type,
                    handle,
                },
                binding,
                observer,
            },
            Self::AwaitMany(state) => {
                FiberSuspensionReason::AwaitMany(state.into_live_admitted(owner)?)
            }
            Self::HostCall {
                call,
                args,
                destination,
            } => FiberSuspensionReason::HostCall {
                call,
                args: args.into_iter().map(value).collect::<Result<_, _>>()?,
                destination,
            },
            Self::BudgetYield => FiberSuspensionReason::BudgetYield,
        })
    }
}
