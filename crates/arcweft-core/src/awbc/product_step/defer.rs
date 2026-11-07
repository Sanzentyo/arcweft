//! Deferred callbacks owned by ordinary executable frames and lexical scopes.

use super::*;
use crate::awbc::fiber::{
    AwbcFiberLexicalScopeId, FiberDeferredInFlight, FiberDeferredRegistration,
};
use crate::scope::RuntimePreparedDeferredRegistration;
use crate::value::ownership::RuntimeOwnedSlotId;

pub(super) enum PreparedFlowDefer {
    Register {
        owner: AwbcFiberLexicalScopeId,
        cursor: FiberCursor,
        registration: RuntimePreparedDeferredRegistration,
        site: crate::runtime_id::RuntimeDeferSiteId,
        outcome: crate::scope::RuntimeDeferOutcomeFilter,
        captures: Vec<(crate::awbc::schema::AwbcRegisterId, RuntimeValue)>,
    },
    Unwind {
        owner: AwbcFiberLexicalScopeId,
        exit: ScopeExit,
        spawn: Option<line::PreparedLineChildSpawn>,
    },
}

impl PreparedFlowDefer {
    /// Projects custody onto the exact packet or already allocated child frame.
    /// The source values stay in their original owner until all issuers admit it.
    pub(super) fn project_custody(
        &self,
        execution: crate::runtime_id::ExecutionInstanceId,
        fiber: &FiberState,
        after: &mut BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        drops: &mut crate::line_task::RuntimeHandleDropAuthorization,
    ) -> Result<(), ProductStepError> {
        match self {
            Self::Register {
                owner,
                registration,
                captures,
                ..
            } => {
                for (ordinal, (_, capture)) in captures.iter().enumerate() {
                    let slot = RuntimeOwnedSlotId::AwbcDeferCapture {
                        execution,
                        fiber: owner.fiber,
                        frame: owner.frame,
                        scope: owner.scope,
                        registration: registration.id(),
                        ordinal: u32::try_from(ordinal)
                            .map_err(|_| LineRuntimeError::OwnedSlotOverflow)?,
                    };
                    for handle in line::unique_line_handles(capture)? {
                        if after.insert(handle.token().clone(), slot).is_some() {
                            return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                        }
                    }
                }
            }
            Self::Unwind { owner, spawn, .. } => {
                let packet = owner
                    .active_scope(fiber)
                    .and_then(|scope| scope.defers.last())
                    .ok_or(LineRuntimeError::InvalidDeferredTransition)?;
                let mut destinations = spawn.as_ref().map(|spawn| spawn.capture_slots(execution));
                for (ordinal, capture) in packet.captures.iter().enumerate() {
                    let source = RuntimeOwnedSlotId::AwbcDeferCapture {
                        execution,
                        fiber: owner.fiber,
                        frame: owner.frame,
                        scope: owner.scope,
                        registration: packet.id,
                        ordinal: u32::try_from(ordinal)
                            .map_err(|_| LineRuntimeError::OwnedSlotOverflow)?,
                    };
                    let destination = destinations
                        .as_mut()
                        .map(|slots| {
                            slots
                                .next()
                                .ok_or(LineRuntimeError::InvalidDeferredTransition)
                        })
                        .transpose()?;
                    let handles = line::unique_line_handles(capture)?;
                    for handle in &handles {
                        if after.remove(handle.token()) != Some(source) {
                            return Err(LineRuntimeError::WrongOwner.into());
                        }
                        if let Some(destination) = destination {
                            after.insert(handle.token().clone(), destination);
                        }
                    }
                    if destination.is_none() {
                        drops.authorize_handles(handles)?;
                    }
                }
                if destinations.is_some_and(|mut slots| slots.next().is_some()) {
                    return Err(LineRuntimeError::InvalidDeferredTransition.into());
                }
            }
        }
        Ok(())
    }
}

impl AwbcProductStepExecutor {
    pub(super) fn published_flow_handle_owners(
        &self,
        detached: Option<&FiberState>,
    ) -> Result<
        BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        ProductStepError,
    > {
        let mut owners = BTreeMap::new();
        for fiber in std::iter::once(&self.fiber)
            .chain(self.child_fibers.iter().map(|child| &child.fiber))
            .chain(detached)
        {
            for (token, slot) in line::product_fiber_handle_owners(PRODUCT_EXECUTION_ID, fiber)? {
                if matches!(fiber.presentation_bank(), crate::runtime_id::RuntimePresentationBankId::Dialogue { activation } if token.activation() == &activation)
                {
                    continue;
                }
                if owners.insert(token, slot).is_some() {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
            }
        }
        self.dialogues
            .extend_received_frame_handle_owners(PRODUCT_EXECUTION_ID, &mut owners)?;
        Ok(owners)
    }

    pub(super) fn prepare_flow_defer_step(
        &self,
        fiber: &FiberState,
        observations: &mut Vec<VmObservation>,
        batch: &ProductLineTaskExecutionBatch,
    ) -> Result<Option<PreparedFlowDefer>, ProductStepError> {
        let Some(index) = observations.iter().position(|observation| {
            matches!(
                observation,
                VmObservation::ScopedDeferRegistration { .. }
                    | VmObservation::ScopedDeferUnwind { .. }
            )
        }) else {
            return Ok(None);
        };
        let invalid =
            || ProductStepError::Internal("invalid ordinary deferred scope transition".into());
        let frame = fiber.active_frame()?;
        let (cursor, scope) = match &observations[index] {
            VmObservation::ScopedDeferRegistration { cursor, scope, .. }
            | VmObservation::ScopedDeferUnwind { cursor, scope } => (*cursor, *scope),
            _ => unreachable!(),
        };
        let owner = AwbcFiberLexicalScopeId {
            fiber: fiber.instance,
            frame: frame.instance,
            scope,
        };
        let deferred = owner.active_scope(fiber).ok_or_else(invalid)?;
        if fiber.cursor != cursor
            || deferred.defer_inflight.is_some()
            || !deferred.defer_releasing.is_empty()
        {
            return Err(invalid());
        }
        match &observations[index] {
            VmObservation::ScopedDeferRegistration {
                site,
                outcome,
                captures,
                ..
            } => {
                if deferred.defer_exit.is_some() {
                    return Err(invalid());
                }
                fiber.validate_yielded_instruction(cursor)?;
                FiberDeferredRegistration::validate_registration_inputs(
                    &self.program,
                    frame,
                    cursor,
                    *site,
                    *outcome,
                    captures,
                )?;
                for (register, _) in captures {
                    if !frame
                        .registers
                        .get(register.index())
                        .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
                    {
                        return Err(invalid());
                    }
                }
                let registration = self.flow_defer_frontier.prepare().ok_or_else(invalid)?;
                let VmObservation::ScopedDeferRegistration {
                    site,
                    outcome,
                    captures,
                    ..
                } = observations.remove(index)
                else {
                    unreachable!()
                };
                Ok(Some(PreparedFlowDefer::Register {
                    owner,
                    cursor,
                    registration,
                    site,
                    outcome,
                    captures,
                }))
            }
            VmObservation::ScopedDeferUnwind { .. } => {
                let packet = deferred.defers.last().ok_or_else(invalid)?;
                let requested = match fiber.status {
                    FiberStatus::Cancelled => ScopeExit::Cancelled,
                    FiberStatus::Trapped => ScopeExit::Failed,
                    _ => ScopeExit::Completed,
                };
                let exit = deferred.defer_exit.unwrap_or(requested);
                let spawn = if packet.outcome.matches(exit) {
                    let function = *self
                        .program
                        .defer_sites
                        .get(packet.site.index())
                        .ok_or_else(invalid)?;
                    Some(batch.prepare_spawn(
                        self,
                        function,
                        fiber.presentation_bank(),
                        &packet.captures,
                    )?)
                } else {
                    None
                };
                observations.remove(index);
                Ok(Some(PreparedFlowDefer::Unwind { owner, exit, spawn }))
            }
            _ => unreachable!(),
        }
    }

    pub(super) fn commit_flow_defer_step(
        fiber: &mut FiberState,
        prepared: PreparedFlowDefer,
        frontier: &mut crate::scope::RuntimeDeferRegistrationFrontier,
    ) -> Option<PreparedFlowDeferSpawn> {
        match prepared {
            PreparedFlowDefer::Register {
                owner,
                cursor,
                registration,
                site,
                outcome,
                captures,
            } => {
                let instruction = fiber
                    .validate_yielded_instruction(cursor)
                    .expect("registration instruction was admitted");
                let id = frontier.commit(registration);
                let (capture_registers, captures) = captures.into_iter().unzip();
                owner
                    .active_scope_mut(fiber)
                    .expect("registration owner was admitted")
                    .defers
                    .push(FiberDeferredRegistration {
                        id,
                        site,
                        source: cursor,
                        outcome,
                        capture_registers,
                        captures,
                    });
                fiber.commit_yielded_instruction_prepared(instruction);
                None
            }
            PreparedFlowDefer::Unwind { owner, exit, spawn } => {
                let scope = owner
                    .active_scope_mut(fiber)
                    .expect("unwind owner was admitted");
                scope.defer_exit = Some(exit);
                let packet = scope.defers.pop().expect("unwind packet was admitted");
                if let Some(spawn) = spawn {
                    scope.defer_inflight = Some(FiberDeferredInFlight {
                        registration: packet.id,
                        site: packet.site,
                    });
                    Some(PreparedFlowDeferSpawn {
                        owner: ProductChildFiberOwner::FlowDeferred {
                            owner,
                            registration: packet.id,
                            site: packet.site,
                        },
                        captures: packet.captures,
                        spawn,
                    })
                } else {
                    None
                }
            }
        }
    }

    pub(super) fn finish_flow_defer_child(
        &mut self,
        child: ProductChildFiber,
    ) -> Result<(), ProductStepError> {
        let ProductChildFiberOwner::FlowDeferred {
            owner,
            registration,
            site,
        } = child.owner
        else {
            unreachable!()
        };
        let fiber = if self.fiber.instance == owner.fiber {
            &mut self.fiber
        } else {
            &mut self
                .child_fibers
                .iter_mut()
                .find(|child| child.fiber.instance == owner.fiber)
                .ok_or_else(|| ProductStepError::Internal("defer parent disappeared".into()))?
                .fiber
        };
        let scope = owner
            .active_scope_mut(fiber)
            .ok_or_else(|| ProductStepError::Internal("defer scope disappeared".into()))?;
        if scope.defer_inflight != Some(FiberDeferredInFlight { registration, site }) {
            return Err(ProductStepError::Internal(
                "defer completion owner mismatch".into(),
            ));
        }
        scope.defer_inflight = None;
        if let Some(FiberTerminalValue::Trapped(trap)) = child.fiber.terminal {
            scope.defer_failure.get_or_insert(trap);
        }
        Ok(())
    }
}

pub(super) struct PreparedFlowDeferSpawn {
    owner: ProductChildFiberOwner,
    captures: Vec<RuntimeValue>,
    spawn: line::PreparedLineChildSpawn,
}

impl PreparedFlowDeferSpawn {
    pub(super) fn commit(
        self,
        executor: &AwbcProductStepExecutor,
        batch: &mut ProductLineTaskExecutionBatch,
    ) {
        batch.spawn_prepared(executor, self.owner, self.captures, self.spawn);
    }
}

/// One admission rule for live checkpoints and inert saves. Line-owned packets
/// use their activation ledger; ordinary packets use the executor frontier.
pub(super) fn validate_flow_defer_contour<'a, V: 'a, O: 'a, N: 'a, B: 'a, R: 'a, G: 'a>(
    program: &crate::awbc::schema::AwbcProgram,
    frontier: crate::scope::RuntimeDeferRegistrationFrontier,
    fibers: impl IntoIterator<
        Item = (
            &'a crate::awbc::fiber::FiberState<V, O, N, B, R, G>,
            Option<(
                AwbcFiberLexicalScopeId,
                crate::runtime_id::RuntimeDeferRegistrationId,
                crate::runtime_id::RuntimeDeferSiteId,
            )>,
        ),
    >,
) -> Result<(), String> {
    let mut registrations = BTreeSet::new();
    let mut inflight = BTreeMap::new();
    let mut children = BTreeMap::new();
    for (fiber, child) in fibers {
        for frame in &fiber.frames {
            for (scope, deferred) in frame.deferred_scopes() {
                if !deferred.defer_releasing.is_empty() {
                    return Err("ordinary defer retains Line release custody".into());
                }
                for id in deferred
                    .defers
                    .iter()
                    .map(|packet| packet.id)
                    .chain(deferred.defer_inflight.map(|running| running.registration))
                {
                    if !frontier.contains(id) || !registrations.insert(id) {
                        return Err(
                            "ordinary defer registration is duplicated or beyond its frontier"
                                .into(),
                        );
                    }
                }
                if let Some(running) = deferred.defer_inflight {
                    let owner = AwbcFiberLexicalScopeId {
                        fiber: fiber.instance,
                        frame: frame.instance,
                        scope,
                    };
                    if deferred.defer_exit.is_none()
                        || owner.active_scope(fiber).is_none()
                        || inflight.insert(owner, running).is_some()
                    {
                        return Err("ordinary defer inflight scope is not active".into());
                    }
                }
            }
        }
        if let Some((owner, registration, site)) = child {
            let function = program
                .defer_sites
                .get(site.index())
                .copied()
                .ok_or("ordinary deferred child references missing site")?;
            if owner.fiber >= fiber.instance
                || fiber.root != crate::awbc::fiber::AwbcFiberRoot::Function(function)
                || fiber.frames.first().map(|frame| frame.function) != Some(function)
                || children
                    .insert(owner, FiberDeferredInFlight { registration, site })
                    .is_some()
            {
                return Err("ordinary deferred child has a stale or duplicate parent".into());
            }
        }
    }
    if inflight != children {
        return Err("ordinary deferred children do not match scope inflight packets".into());
    }
    Ok(())
}
