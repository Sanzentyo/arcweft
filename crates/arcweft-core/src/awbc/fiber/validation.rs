//! Original executable metadata shared by live and inert fiber storage.

use super::{
    AwbcBlockId, AwbcFiberRoot, AwbcFunctionId, AwbcProgram, FiberCallablePendingStorage,
    FiberCursor, FiberReturnContext, FiberReturnContinuation, FiberReturnPoint, FiberState,
    FiberStateError, FiberStatus, FiberStreamState, FiberTerminalValue, RuntimeStoredValue,
    format_operand_count_at_site, validate_cursor, validate_frame_layout,
    validate_return_point_metadata,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::RuntimeFunctionEffectInstantiation;
use std::{borrow::Borrow, collections::BTreeSet};

impl<V, O, N, B, P> FiberState<V, O, N, B, FiberReturnPoint<FiberReturnContinuation<P, V>>> {
    /// Source admission follows the complete value grammar on its same bank;
    /// no copied aggregate or independent resource allowance is introduced.
    pub(super) fn admit_task_source_templates(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        need: impl Fn(&N) -> crate::value::RuntimeNeedView<'_> + Copy,
    ) -> super::AwbcSaveResult<()>
    where
        V: RuntimeStoredValue,
        B: Borrow<RuntimeFunctionEffectInstantiation>,
    {
        let Some(super::FiberSuspension { reason, .. }) = &self.suspension else {
            return Ok(());
        };
        if !matches!(
            reason,
            super::FiberSuspensionReason::AwaitMany(_)
                | super::FiberSuspensionReason::ProgramFanout(_)
        ) {
            return Ok(());
        }
        admission.status()?;
        let original = admission.task_source().cloned().ok_or_else(|| {
            crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: "AwaitMany admission requires its enclosing admitted original Source"
                    .into(),
            }
        })?;
        let frame = self.frames.last().ok_or(FiberStateError::MissingFrame)?;
        match reason {
            super::FiberSuspensionReason::AwaitMany(state) => state.admit_original_source(
                &original,
                frame.type_instantiation.as_ref().map(Borrow::borrow),
                need,
                admission,
            ),
            super::FiberSuspensionReason::ProgramFanout(state) => {
                state.admit_original_source(&original, need, admission)
            }
            _ => unreachable!("suspended task state was selected before Source admission"),
        }
        .inspect_err(|error| admission.retain_failure(error.clone()))
    }

    /// A stopped caller resumes at its callee's authenticated return point;
    /// only the innermost frame uses the fiber's current cursor.
    pub(super) fn frame_resume_cursor(&self, index: usize) -> Result<FiberCursor, FiberStateError> {
        let frame = self
            .frames
            .get(index)
            .ok_or(FiberStateError::MissingFrame)?;
        let cursor = match self.frames.get(index + 1) {
            Some(callee) => {
                callee
                    .return_to
                    .as_ref()
                    .ok_or(FiberStateError::InvalidFrame)?
                    .cursor
            }
            None => self.cursor,
        };
        if cursor.function != frame.function {
            return Err(FiberStateError::InvalidFrame);
        }
        Ok(cursor)
    }

    pub(super) fn validate_structure(
        &self,
        program: &AwbcProgram,
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), FiberStateError>
    where
        V: RuntimeStoredValue,
        B: Borrow<RuntimeFunctionEffectInstantiation>,
        P: FiberCallablePendingStorage,
    {
        resources
            .work()
            .collection(self.frames.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        for frame in &self.frames {
            resources
                .work()
                .collection(frame.registers.len())
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            validate_frame_layout(
                program,
                frame.function,
                frame.layout,
                frame.type_instantiation.as_ref().map(Borrow::borrow),
                frame.registers.len(),
                &resources.work(),
            )
            .map_err(|error| {
                resources
                    .work()
                    .failure()
                    .map_or(error, |resource| FiberStateError::Snapshot(resource.into()))
            })?;
        }
        self.root.validate_root_frame(
            program,
            Some(
                self.frames
                    .first()
                    .map_or(self.cursor.function, |frame| frame.function),
            ),
        )?;
        if self.root == AwbcFiberRoot::Empty {
            return self.validate_empty_origin();
        }
        if matches!(self.root, AwbcFiberRoot::Task(_))
            && matches!(
                self.terminal.as_ref(),
                Some(
                    FiberTerminalValue::ReturnedTransferred
                        | FiberTerminalValue::DialogueResultSelected(_)
                        | FiberTerminalValue::DialogueResultTransferred
                )
            )
        {
            return Err(FiberStateError::InvalidFrame);
        }
        self.status.validate_retained_shape(
            self.suspension.is_some(),
            self.terminal.as_ref(),
            self.return_summary.as_deref(),
        )?;
        validate_cursor(program, self.cursor)?;
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.function != self.cursor.function)
        {
            return Err(FiberStateError::InvalidFrame);
        }
        // Authenticate the whole frame inventory before any retained input
        // or return continuation is inspected under its value grammar.
        let mut instances = BTreeSet::new();
        for frame in &self.frames {
            resources
                .work()
                .charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            if !instances.insert(frame.instance)
                || self
                    .next_frame_instance
                    .next()
                    .is_some_and(|next| frame.instance.get() >= next)
            {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        for (index, frame) in self.frames.iter().enumerate() {
            frame.validate_continuations(
                program,
                index.checked_sub(1).map(|index| &self.frames[index]),
                self.frames.get(index + 1),
                self.cursor,
                &mut resources.work(),
            )?;
        }
        let stacks = self
            .frames
            .iter()
            .enumerate()
            .map(|(index, frame)| Ok((self.frame_resume_cursor(index)?, frame.scopes.as_slice())))
            .collect::<Result<Vec<_>, FiberStateError>>()?;
        program
            .verify_scope_stacks(stacks)
            .map_err(|_| FiberStateError::InvalidFrame)?;
        if matches!(self.status, FiberStatus::Running | FiberStatus::Suspended)
            && self.frames.is_empty()
        {
            return Err(FiberStateError::MissingFrame);
        }
        if self.terminal.is_some()
            && self
                .frames
                .iter()
                .any(|frame| !frame.format_attempts.is_empty())
        {
            return Err(FiberStateError::InvalidFrame);
        }
        Ok(())
    }

    /// An empty origin has no executable frame or cursor to authenticate.
    fn validate_empty_origin(&self) -> Result<(), FiberStateError> {
        if self.root == AwbcFiberRoot::Empty
            && self.frames.is_empty()
            && self.status == FiberStatus::Returned
            && self.suspension.is_none()
            && matches!(self.terminal, Some(FiberTerminalValue::Returned(None)))
            && self.return_summary.is_none()
            && self.streams.is_empty()
            && self.line_cursor == 0
            && self.cursor
                == (FiberCursor {
                    function: AwbcFunctionId::default(),
                    block: AwbcBlockId::default(),
                    instruction_offset: 0,
                })
        {
            Ok(())
        } else {
            Err(FiberStateError::InvalidFrame)
        }
    }
    /// Admits every original header and retained input under one allowance,
    /// before either storage projection constructs or copies descendants.
    pub(super) fn validate_retained(
        &self,
        owner: &super::RuntimeProgramOwner,
        resources: &mut ValueResourceAdmission,
        opaque: impl Fn(&O) -> crate::value::RuntimeValueView<'_> + Copy,
        need: impl Fn(&N) -> crate::value::RuntimeNeedView<'_> + Copy,
    ) -> super::AwbcSaveResult<()>
    where
        V: RuntimeStoredValue,
        B: Borrow<RuntimeFunctionEffectInstantiation>,
        P: FiberCallablePendingStorage,
    {
        let super::RuntimeProgramOwner::Awbc(program) = owner else {
            return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: "AWBC fiber admission requires its original AWBC program".to_owned(),
            });
        };
        self.status
            .validate_retained_shape(
                self.suspension.is_some(),
                self.terminal.as_ref(),
                self.return_summary.as_deref(),
            )
            .map_err(crate::value::AwbcRuntimeValueSnapshotError::from)?;
        self.validate_structure(program, resources)?;
        for (index, frame) in self.frames.iter().enumerate() {
            if let Some(point) = &frame.return_to {
                point.continuation.validate_pending_inputs(
                    owner,
                    &FiberReturnContext {
                        program,
                        caller: index
                            .checked_sub(1)
                            .map(|index| self.frames[index].function),
                        binding: index.checked_sub(1).and_then(|index| {
                            self.frames[index]
                                .type_instantiation
                                .as_ref()
                                .map(Borrow::borrow)
                        }),
                        returning_function: frame.function,
                        cursor: point.cursor,
                        destination: point.destination,
                    },
                    resources,
                    V::borrowed_view,
                )?;
            }
        }
        // Every pending invocation keeps its original input rows. Admit them
        // before replaying register Copy obligations, so a malformed input
        // remains attributed to its owning typed boundary.
        for (index, frame) in self.frames.iter().enumerate() {
            frame.validate_retained_values(
                program,
                self.frame_resume_cursor(index)?,
                resources,
                V::borrowed_view,
                &format!("frames[{index}]"),
            )?;
        }
        if let Some(suspension) = &self.suspension {
            let frame = self.frames.last().ok_or_else(|| {
                crate::value::AwbcRuntimeValueSnapshotError::from(FiberStateError::MissingFrame)
            })?;
            suspension
                .validate_inputs(
                    &frame.value_context(program)?,
                    self.cursor,
                    resources,
                    V::borrowed_view,
                    opaque,
                    need,
                )
                .map_err(crate::value::AwbcRuntimeValueSnapshotError::from)?;
        }
        FiberStreamState::validate_inventory(
            program,
            self.root,
            &self.streams,
            resources,
            V::borrowed_view,
        )
        .map_err(crate::value::AwbcRuntimeValueSnapshotError::from)?;
        if let Some(summary) = &self.return_summary {
            resources.metadata_string(summary)?;
        }
        if let Some(terminal) = &self.terminal {
            let context = self
                .frames
                .first()
                .map(|frame| frame.value_context(program))
                .transpose()?;
            terminal
                .validate_inputs(
                    program,
                    self.root,
                    context.as_ref(),
                    self.frames.len(),
                    resources,
                    V::borrowed_view,
                )
                .map_err(crate::value::AwbcRuntimeValueSnapshotError::from)?;
            terminal.validate_summary(
                Some(owner.types()),
                self.return_summary.as_deref(),
                V::borrowed_view,
            )?;
        }
        Ok(())
    }
}

impl<V, B, P> super::FiberFrame<V, B, FiberReturnPoint<FiberReturnContinuation<P, V>>> {
    /// The frame owns its parked formatter and its return to the caller.
    fn validate_continuations(
        &self,
        program: &AwbcProgram,
        parent: Option<&Self>,
        child: Option<&Self>,
        cursor: FiberCursor,
        work: &mut crate::entry::schema::value_budget::ValidationWork,
    ) -> Result<(), FiberStateError>
    where
        V: RuntimeStoredValue,
        B: Borrow<RuntimeFunctionEffectInstantiation>,
        P: FiberCallablePendingStorage,
    {
        if let Some(format) = &self.format {
            let parked_at = child
                .and_then(|child| child.return_to.as_ref())
                .map_or(cursor, |point| point.cursor);
            if parked_at != format.storage.site {
                return Err(FiberStateError::InvalidFrame);
            }
            if let Some(child) = child {
                let continuation = child.return_to.as_ref().map(|point| &point.continuation);
                let operand_return = matches!(continuation,
                    Some(FiberReturnContinuation::FormatOperand { site, ordinal })
                        if *site == format.storage.site && *ordinal == format.storage.next_operand);
                let display_return = matches!(continuation,
                    Some(FiberReturnContinuation::FormatDisplay { site })
                        if *site == format.storage.site && format.storage.next_operand
                            == format_operand_count_at_site(program, format.storage.site)?);
                if !operand_return && !display_return {
                    return Err(FiberStateError::InvalidFrame);
                }
            }
        }
        if let Some(point) = &self.return_to {
            let parent = parent.ok_or(FiberStateError::InvalidFrame)?;
            validate_return_point_metadata(program, parent, self.function, point, work)?;
        }
        Ok(())
    }
}
