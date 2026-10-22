//! Borrowed admission of native execution ownership before rollback projection.

use super::{
    Engine, FlowControlStackEntry, FlowControlStackEntryKind, FlowCursor, FlowFiberOwner,
    FlowFiberStatus, FlowOp, FlowOpRollbackImage, FunctionCallFrame, FunctionReturnContinuation,
    NativeEngineRollbackImage, NativeFlowExecutionFrame, NativeFormatAttemptFrame,
    NativeInvocationRoot, eval,
};
use crate::runtime_id::DialogueActivationId;
use crate::task::TaskPublicationCursor;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

mod await_state;

pub(super) trait SuspendedValues {
    fn admit_suspended_values(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>;
}

fn admit_default_packet(
    state: crate::runtime_id::RuntimeCallableStateId,
    site: crate::runtime_id::RuntimeFunctionSiteId,
    admission: &mut crate::value::RuntimeRetainedValueAdmission,
    origin: crate::entry::schema::value_encoding::ValueAdmissionOrigin,
    validate: impl FnOnce(
        &crate::task::RuntimeProgramOwner,
        &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), crate::value::RuntimeCallableValueError>,
) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
    admission.admit_composite(origin, |owner, resources| {
        let owner = owner.ok_or_else(|| crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: "native default packet requires its original executable".into(),
        })?;
        crate::value::RuntimeCallableBodyReference::Plan(site)
            .validate_default_return(owner.types(), state)?;
        validate(owner, resources)?;
        Ok(())
    })
}

pub(super) trait PendingStorage {
    fn admit_default(
        &self,
        site: crate::runtime_id::RuntimeFunctionSiteId,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>;
}

impl PendingStorage for crate::value::RuntimeCallablePendingGroup {
    fn admit_default(
        &self,
        site: crate::runtime_id::RuntimeFunctionSiteId,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admit_default_packet(
            self.state(),
            site,
            admission,
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
            |owner, resources| self.validate_snapshot_inputs(owner, resources),
        )
    }
}

impl PendingStorage for crate::value::RuntimeCallablePendingSnapshot {
    fn admit_default(
        &self,
        site: crate::runtime_id::RuntimeFunctionSiteId,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admit_default_packet(
            self.state,
            site,
            admission,
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::InertSnapshot,
            |owner, resources| self.validate_inputs(owner, resources),
        )
    }
}

impl<P> FunctionReturnContinuation<P> {
    pub(super) fn admit_header_and_packet(
        &self,
        site: crate::runtime_id::RuntimeFunctionSiteId,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        P: PendingStorage,
    {
        let invalid = |message: &str| crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: message.into(),
        };
        match self {
            Self::RootInitializer { entry } | Self::RootReducer { entry } => {
                admission.validate_metadata(entry)?;
                let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner()
                else {
                    return Err(invalid("native Root continuation requires its plan owner"));
                };
                let roles = plan
                    .entries()
                    .iter()
                    .find(|candidate| candidate.id == *entry)
                    .and_then(|entry| entry.roles.stateful())
                    .ok_or_else(|| invalid("native Root entry is absent from its plan"))?;
                let role = match self {
                    Self::RootInitializer { .. } => &roles.initializer,
                    Self::RootReducer { .. } => &roles.reducer,
                    _ => unreachable!("matched Root continuation"),
                };
                if !plan.callable_executables().iter().any(|executable| {
                    executable.callable == role.callable
                        && executable.contract == role.contract
                        && executable.code
                            == crate::entry::RuntimeCallableExecutableCode::FunctionSite(site)
                }) {
                    return Err(invalid(
                        "native Root continuation does not match its entry role",
                    ));
                }
            }
            Self::Program { program } => {
                admission.validate_metadata(program)?;
                let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner()
                else {
                    return Err(invalid(
                        "native program continuation requires its plan owner",
                    ));
                };
                if !plan
                    .resolve_pure_program(*program)
                    .is_ok_and(|binding| binding.site() == site)
                {
                    return Err(invalid(
                        "native program continuation is absent from its plan",
                    ));
                }
            }
            Self::Function { site: target } => {
                admission.validate_metadata(&target.get())?;
                let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner()
                else {
                    return Err(invalid(
                        "native function continuation requires its plan owner",
                    ));
                };
                if *target != site
                    || crate::pure::RuntimePureFunctionRef::resolve(plan, *target).is_err()
                {
                    return Err(invalid(
                        "native function continuation is absent from its plan",
                    ));
                }
            }
            Self::CallableDefault { pending, .. } => pending.admit_default(site, admission)?,
            Self::Bind { .. } => {}
        }
        Ok(())
    }
}

impl FlowCursor {
    fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        let invalid = |message: &str| crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: message.into(),
        };
        admission.validate_metadata(&(self.flow_index, self.op_index))?;
        let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
            return Err(invalid(
                "native cursor requires its original Plan executable",
            ));
        };
        let flow = plan
            .flows
            .get(self.flow_index)
            .ok_or_else(|| invalid("native cursor references an unknown original flow"))?;
        if self.op_index > flow.body().ops().len() {
            return Err(invalid("native cursor exceeds its original flow body"));
        }
        Ok(())
    }
}

impl<O, P> FunctionCallFrame<O, P> {
    fn admit_code_position(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        use crate::plan::code::{RuntimeCodeBodyRef, RuntimeCodeOwner};
        let invalid = |message: &str| crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: message.into(),
        };
        admission.validate_metadata(&(self.site.get(), self.next_instruction))?;
        let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
            return Err(invalid(
                "native function position requires its original Plan executable",
            ));
        };
        let valid = match plan
            .inventory
            .code_body(RuntimeCodeOwner::Function(self.site))
        {
            Some(RuntimeCodeBodyRef::Operations(operations)) => {
                self.next_instruction <= operations.len()
            }
            Some(RuntimeCodeBodyRef::Expression(_)) => self.next_instruction == 0,
            None => false,
        };
        if !valid {
            return Err(invalid(
                "native function position exceeds its original body",
            ));
        }
        Ok(())
    }
}

impl<V, O> NativeFormatAttemptFrame<V, O> {
    fn admit_owned_values(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        V: crate::value::RuntimeStoredValue,
    {
        let invalid = |message: &str| crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: message.into(),
        };
        admission.validate_metadata(&(
            self.attempt.get(),
            &self.context,
            self.next_operand,
            &self.first_recoverable,
        ))?;
        admission.validate_metadata_collection(self.values.len())?;
        let owner = admission
            .program_owner()
            .cloned()
            .ok_or_else(|| invalid("native formatter requires its original executable"))?;
        let crate::task::RuntimeProgramOwner::Plan(plan) = owner else {
            return Err(invalid(
                "native formatter requires its original Plan executable",
            ));
        };
        let manifest = plan.format_attempt(self.attempt).ok_or_else(|| {
            invalid("native formatter attempt is absent from its original executable")
        })?;
        if self.values.len() != manifest.operands().len() || self.next_operand > self.values.len() {
            return Err(invalid(
                "native formatter operand inventory differs from its original ABI",
            ));
        }
        if let Some(active) = &self.active {
            admission.validate_metadata(&active.ordinal)?;
            admission.validate_metadata_collection(active.caller_pending_ops.len())?;
            if active.ordinal != self.next_operand || active.ordinal >= self.values.len() {
                return Err(invalid(
                    "native formatter active operand differs from its source-order frontier",
                ));
            }
            if let Some(cursor) = active.resume {
                cursor.admit_retained(admission)?;
            }
        }
        for (index, (value, operand)) in self.values.iter().zip(manifest.operands()).enumerate() {
            if (index >= self.next_operand && value.is_some())
                || (index < self.next_operand
                    && self.first_recoverable.is_none()
                    && value.is_none())
            {
                return Err(invalid(
                    "native formatter value presence differs from its completion frontier",
                ));
            }
            if let Some(value) = value {
                let expected = plan
                    .type_table()
                    .get(operand.ty())
                    .ok_or_else(|| {
                        invalid("native formatter operand has an unknown original type")
                    })?
                    .semantic_identity();
                admission.validate_owned_typed_borrowed_in_invocation(
                    value.borrowed_value(),
                    expected,
                    binding,
                )?;
            }
        }
        Ok(())
    }
}

impl<V, N, P, Q, FV, FP, O, C, A, AS, MS, D>
    Engine<
        V,
        N,
        P,
        crate::root::RootRuntime<Q>,
        NativeFlowExecutionFrame<FV, FP, O, C, A, FlowFiberStatus<AS, MS>>,
        D,
    >
{
    pub(super) fn admit_root_execution(&self) -> Result<(), String>
    where
        C: ScopeControl<O>,
    {
        if !matches!(self.fiber.owner, FlowFiberOwner::Executor) {
            return Err("native main fiber does not have its executor owner".into());
        }
        self.fiber.frame.admit_root_phase(
            self.root.as_ref(),
            &self.fiber.owner,
            matches!(self.fiber.frame.status, FlowFiberStatus::Failed(_)),
        )?;
        let mut reducer_count = 0;
        for child in &self.child_fibers {
            let root = if matches!(child.owner, FlowFiberOwner::RootReducer { .. }) {
                if !matches!(child.frame.status, FlowFiberStatus::Running) {
                    return Err("native Root reducer is not a running pure frame".into());
                }
                reducer_count += 1;
                self.root.as_ref()
            } else {
                None
            };
            child.frame.admit_root_phase(root, &child.owner, false)?;
        }
        let evaluating = self.root.as_ref().is_some_and(|root| {
            matches!(
                root.active().phase,
                crate::root::RootPhase::Initialized {
                    reducer: crate::root::RootReducerPhase::Evaluating,
                    ..
                }
            )
        });
        if reducer_count != usize::from(evaluating) {
            return Err(
                "native Root reducer phase does not have exactly its executable fiber".into(),
            );
        }
        if evaluating
            && !self
                .child_fibers
                .front()
                .is_some_and(|child| matches!(child.owner, FlowFiberOwner::RootReducer { .. }))
        {
            return Err("native Root reducer lost its executor priority".into());
        }
        Ok(())
    }
}

impl<V, P, O, C, A, S> NativeFlowExecutionFrame<V, P, O, C, A, S> {
    pub(super) fn admit_root_phase<Q>(
        &self,
        root: Option<&crate::root::RootRuntime<Q>>,
        owner: &FlowFiberOwner,
        failed: bool,
    ) -> Result<(), String>
    where
        C: ScopeControl<O>,
    {
        let mut functions = self
            .control_stack
            .iter()
            .filter_map(ScopeControl::function_context);
        let invocation = functions.next().and_then(|context| context.root);
        if functions.any(|context| context.root.is_some()) {
            return Err("native Root role is not the invocation root".into());
        }
        match (owner, root, invocation) {
            (FlowFiberOwner::Executor, Some(root), Some(RootInvocation::Initializer(entry)))
                if root.active().entry == *entry
                    && matches!(
                        root.active().phase,
                        crate::root::RootPhase::Initializing { .. }
                    )
                    && self.cursor.is_none() =>
            {
                Ok(())
            }
            (
                FlowFiberOwner::RootReducer { entry: owner_entry },
                Some(root),
                Some(RootInvocation::Reducer(entry)),
            ) if owner_entry == entry
                && root.active().entry == *entry
                && matches!(
                    root.active().phase,
                    crate::root::RootPhase::Initialized {
                        reducer: crate::root::RootReducerPhase::Evaluating,
                        ..
                    }
                )
                && self.cursor.is_none() =>
            {
                Ok(())
            }
            (FlowFiberOwner::Executor, Some(root), None)
                if !matches!(
                    root.active().phase,
                    crate::root::RootPhase::Initializing { .. }
                ) || failed =>
            {
                Ok(())
            }
            (
                FlowFiberOwner::Executor
                | FlowFiberOwner::LineTask(_)
                | FlowFiberOwner::DialogueEffectCallback { .. },
                None,
                None,
            ) => Ok(()),
            _ => Err("native Root phase does not match its executable fiber origin".into()),
        }
    }

    pub(super) fn admit_owned_values(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        V: crate::value::RuntimeStoredValue,
        P: crate::value::RuntimeStoredValue,
        C: ScopeControl<O>,
        A: SuspendedValues,
        S: SuspendedValues,
    {
        let invalid =
            |message: String| crate::value::AwbcRuntimeValueSnapshotError::Message { message };
        admission.validate_metadata(&self.line_cursor)?;
        admission.validate_metadata_collection(self.root_cleanups.len())?;
        for cleanup in &self.root_cleanups {
            admission.validate_metadata_string(&cleanup.key)?;
        }
        admission.validate_metadata_collection(self.stream_states.len())?;
        if let Some(cursor) = self.cursor {
            cursor.admit_retained(admission)?;
        }
        if let Some(state) = &self.await_observer {
            state.admit_suspended_values(admission, self.env.function_instantiation())?;
        }
        self.status
            .admit_suspended_values(admission, self.env.function_instantiation())?;
        self.env.admit_retained(admission)?;
        self.admit_function_contexts(admission)?;
        if let Err(message) =
            self.observations
                .admit_values(admission, P::borrowed_value, "retained")
        {
            return Err(admission.status().err().unwrap_or_else(|| invalid(message)));
        }
        for (id, stream) in &self.stream_states {
            admission.validate_metadata(id)?;
            if id != &stream.id {
                return Err(invalid(
                    "native stream key differs from its retained owner".into(),
                ));
            }
            stream.admit_native_retained(admission)?;
        }
        if let Some(value) = &self.selected_dialogue_result {
            admission.validate_owned_borrowed(value.borrowed_value())?;
        }
        Ok(())
    }

    fn admit_function_contexts(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        C: ScopeControl<O>,
    {
        let invalid = || crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: "native function frames disagree with their retained environment contexts"
                .into(),
        };
        admission.validate_metadata_collection(self.control_stack.len())?;
        let mut contexts = self.env.function_contexts().rev().peekable();
        for entry in self.control_stack.iter().rev() {
            if let Some(frame) = entry.function_context() {
                admission.validate_metadata(&(frame.site.get(), frame.entered))?;
                let Some((site, binding)) = contexts.next() else {
                    return Err(invalid());
                };
                if !frame.entered
                    || frame.site != site
                    || frame.instantiation != binding.map(Arc::as_ref)
                {
                    return Err(invalid());
                }
            }
            let binding = contexts
                .peek()
                .and_then(|(_, binding)| binding.map(Arc::as_ref));
            entry.admit_control_values(admission, binding)?;
        }
        // Callback and deferred child roots retain one function environment
        // without a return frame. Nested calls above it still zip exactly.
        if contexts.next().is_some() && contexts.next().is_some() {
            return Err(invalid());
        }
        Ok(())
    }
}

pub(super) enum ScopeBoundary<'a> {
    Open(
        &'a crate::scope::RuntimeScopeIdentity,
        crate::scope::RuntimeScheduledScopeToken,
    ),
    Close(crate::scope::RuntimeScheduledScopeToken),
}

pub(super) trait ScopeOperation {
    fn scope_boundary(&self) -> Option<ScopeBoundary<'_>>;
}

impl ScopeOperation for FlowOp {
    fn scope_boundary(&self) -> Option<ScopeBoundary<'_>> {
        match self {
            Self::EnterScheduledScope { identity, token } => {
                Some(ScopeBoundary::Open(identity, *token))
            }
            Self::ExitScheduledScope { token } => Some(ScopeBoundary::Close(*token)),
            _ => None,
        }
    }
}

impl ScopeOperation for FlowOpRollbackImage {
    fn scope_boundary(&self) -> Option<ScopeBoundary<'_>> {
        match self {
            Self::EnterScheduledScope { identity, token } => {
                Some(ScopeBoundary::Open(identity, *token))
            }
            Self::ExitScheduledScope { token } => Some(ScopeBoundary::Close(*token)),
            _ => None,
        }
    }
}

pub(super) enum ScopeState<'a, O> {
    Frame(crate::scope::RuntimeScheduledScopeToken),
    Pending(&'a VecDeque<O>),
}

pub(super) struct FunctionContext<'a> {
    site: crate::runtime_id::RuntimeFunctionSiteId,
    entered: bool,
    instantiation: Option<&'a crate::program_types::RuntimeFunctionEffectInstantiation>,
    root: Option<RootInvocation<'a>>,
}

enum RootInvocation<'a> {
    Initializer(&'a crate::plan::EntryRuntimeId),
    Reducer(&'a crate::plan::EntryRuntimeId),
}

pub(super) trait ScopeControl<O> {
    fn function_context(&self) -> Option<FunctionContext<'_>>;
    fn admit_control_values(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>;
    fn visit_scope_state(
        &self,
        visit: impl FnMut(ScopeState<'_, O>) -> Result<(), String>,
    ) -> Result<(), String>;
}

impl<O, P, V> ScopeControl<O>
    for FlowControlStackEntryKind<FunctionCallFrame<O, P>, NativeFormatAttemptFrame<V, O>>
where
    V: crate::value::RuntimeStoredValue,
    P: PendingStorage,
{
    fn admit_control_values(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        match self {
            Self::FormatAttempt(frame) => frame.admit_owned_values(admission, binding),
            Self::FunctionCall(frame) => {
                frame.admit_code_position(admission)?;
                if let Some(cursor) = frame.resume {
                    cursor.admit_retained(admission)?;
                }
                frame
                    .continuation
                    .admit_header_and_packet(frame.site, admission)
            }
            _ => Ok(()),
        }
    }

    fn function_context(&self) -> Option<FunctionContext<'_>> {
        match self {
            Self::FunctionCall(frame) => Some(FunctionContext {
                site: frame.site,
                entered: frame.function_scope,
                instantiation: frame.type_instantiation.as_deref(),
                root: match &frame.continuation {
                    FunctionReturnContinuation::RootInitializer { entry } => {
                        Some(RootInvocation::Initializer(entry))
                    }
                    FunctionReturnContinuation::RootReducer { entry } => {
                        Some(RootInvocation::Reducer(entry))
                    }
                    _ => None,
                },
            }),
            _ => None,
        }
    }

    fn visit_scope_state(
        &self,
        mut visit: impl FnMut(ScopeState<'_, O>) -> Result<(), String>,
    ) -> Result<(), String> {
        match self {
            Self::Scope {
                origin: crate::scope::RuntimeScopeFrameOrigin::Scheduled(token),
                ..
            } => visit(ScopeState::Frame(*token)),
            Self::FunctionCall(frame) => visit(ScopeState::Pending(&frame.caller_pending_ops)),
            Self::FormatAttempt(frame) => {
                if let Some(active) = &frame.active {
                    visit(ScopeState::Pending(&active.caller_pending_ops))?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl ScopeControl<FlowOp> for FlowControlStackEntry {
    fn admit_control_values(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        self.kind.admit_control_values(admission, binding)
    }

    fn function_context(&self) -> Option<FunctionContext<'_>> {
        self.kind.function_context()
    }

    fn visit_scope_state(
        &self,
        visit: impl FnMut(ScopeState<'_, FlowOp>) -> Result<(), String>,
    ) -> Result<(), String> {
        self.kind.visit_scope_state(visit)
    }
}

impl<V, N, P, R, FV, FP, O, C, A, S, D>
    Engine<V, N, P, R, NativeFlowExecutionFrame<FV, FP, O, C, A, S>, D>
{
    pub(super) fn validate_scheduled_scope_state(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), String>
    where
        O: ScopeOperation,
        C: ScopeControl<O>,
    {
        // Bound the complete borrowed inventory before constructing validation
        // sets. Saved rows remain inert throughout these same scope laws.
        for fiber in std::iter::once(&self.fiber).chain(&self.child_fibers) {
            admission
                .validate_metadata_collection(fiber.frame.pending_ops.len())
                .map_err(|error| error.to_string())?;
            admission
                .validate_metadata_collection(fiber.frame.control_stack.len())
                .map_err(|error| error.to_string())?;
            for entry in &fiber.frame.control_stack {
                entry.visit_scope_state(|state| match state {
                    ScopeState::Frame(token) => admission
                        .validate_metadata(&token)
                        .map_err(|error| error.to_string()),
                    ScopeState::Pending(ops) => admission
                        .validate_metadata_collection(ops.len())
                        .map_err(|error| error.to_string()),
                })?;
            }
        }
        if self.next_scheduled_scope_sequence == 0 {
            return Err("native control scope allocator is zero".into());
        }
        let mut opened = BTreeSet::new();
        let mut closed = BTreeSet::new();
        for fiber in std::iter::once(&self.fiber).chain(&self.child_fibers) {
            let valid = |token: crate::scope::RuntimeScheduledScopeToken| {
                token.belongs_to(fiber.execution, fiber.persistent_id)
                    && token.ordinal().get() < self.next_scheduled_scope_sequence
            };
            for entry in &fiber.frame.control_stack {
                entry.visit_scope_state(|state| {
                    if let ScopeState::Frame(token) = state
                        && (!valid(token) || !opened.insert(token))
                    {
                        return Err(
                            "native scope frame has a foreign, repeated, or unallocated token"
                                .into(),
                        );
                    }
                    Ok(())
                })?;
            }
            let mut pending = |ops: &VecDeque<O>| -> Result<(), String> {
                for op in ops {
                    match op.scope_boundary() {
                        Some(ScopeBoundary::Open(identity, token)) => {
                            admission
                                .validate_metadata(identity)
                                .map_err(|error| error.to_string())?;
                            if token.kind() == crate::scope::RuntimeScopeFrameKind::Control
                                && !matches!(
                                    identity,
                                    crate::scope::RuntimeScopeIdentity::Anonymous
                                )
                            {
                                return Err(
                                    "native generated scope carries an authored namespace".into()
                                );
                            }
                            if !valid(token) || !opened.insert(token) {
                                return Err("native pending scope open has a foreign, repeated, or unallocated token".into());
                            }
                        }
                        Some(ScopeBoundary::Close(token))
                            if !valid(token) || !closed.insert(token) =>
                        {
                            return Err("native pending scope close has a foreign, repeated, or unallocated token".into());
                        }
                        Some(ScopeBoundary::Close(_)) | None => {}
                    }
                }
                Ok(())
            };
            pending(&fiber.frame.pending_ops)?;
            for entry in &fiber.frame.control_stack {
                entry.visit_scope_state(|state| match state {
                    ScopeState::Pending(ops) => pending(ops),
                    ScopeState::Frame(_) => Ok(()),
                })?;
            }
        }
        if opened != closed {
            return Err(
                "native scope frames and queued exact close markers are not bijective".into(),
            );
        }
        Ok(())
    }
}

impl<V, N, P, R, F, D> Engine<V, N, P, R, F, D> {
    pub(super) fn admit_metadata(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&(
            self.evaluation_stats.evaluated_exprs,
            self.evaluation_stats.evaluated_calls,
            self.evaluation_stats.evaluated_binary_ops,
        ))?;
        admission.validate_metadata(&self.format_context)?;
        admission.validate_metadata(&(
            self.generation,
            self.task_request_quota_remaining,
            self.main_started,
            self.next_fiber_id,
            self.next_scheduled_scope_sequence,
            self.run_child_next,
            self.audio_epoch,
            self.next_audio_sequence,
            self.next_host_call_sequence,
        ))?;
        admission.validate_metadata(&self.scalar_step_charge)?;
        admission.validate_metadata(&self.need_publication_frontiers)?;
        admission.validate_metadata(&self.dialogue_occurrences)?;
        admission.validate_metadata(&self.dialogue_effect_callback_activations)?;
        admission.validate_metadata(&self.pure_i64_batch_inputs)?;
        admission.validate_metadata(&self.pure_i64_batch_outputs)?;
        admission.validate_metadata(&self.pure_helper_i64_call_shapes)?;
        if self.pure_helper_i64_call_shapes.len() != self.plan.pure_helpers.len() {
            return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: "native pure-helper cache differs from its original inventory".into(),
            });
        }
        for (shape, helper) in self
            .pure_helper_i64_call_shapes
            .iter()
            .zip(&self.plan.pure_helpers)
        {
            admission.validate_metadata_collection(helper.inputs.len())?;
            if *shape != eval::pure_helper_has_i64_call_shape(helper) {
                return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                    message: "native pure-helper cache differs from its original signature".into(),
                });
            }
        }
        admission.validate_metadata_collection(self.need_publications.len())?;
        for (correlation, queue) in &self.need_publications {
            admission.validate_metadata(correlation)?;
            admission.validate_metadata_collection(queue.len())?;
        }
        admission.validate_metadata_collection(self.latest_need_publications.len())?;
        admission.validate_metadata_collection(self.child_fibers.len())?;
        let invalid_identity = || crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: "native fiber identities do not follow their original allocator".into(),
        };
        if self.fiber.id.0 != 0 || self.fiber.parent.is_some() {
            return Err(invalid_identity());
        }
        let mut fiber_ids = BTreeSet::new();
        for fiber in std::iter::once(&self.fiber).chain(&self.child_fibers) {
            admission.validate_metadata(&(
                fiber.id,
                fiber.parent,
                fiber.persistent_id,
                fiber.execution,
                &fiber.owner,
            ))?;
            if fiber.id.0 >= self.next_fiber_id
                || !fiber_ids.insert(fiber.id.0)
                || !fiber.id.matches_persistent(fiber.persistent_id)
                || fiber.execution.get().get() != fiber.persistent_id.get()
            {
                return Err(invalid_identity());
            }
        }
        for child in &self.child_fibers {
            if let FlowFiberOwner::DialogueEffectCallback { activation, .. } = &child.owner
                && !self
                    .dialogue_effect_callback_activations
                    .contains(activation)
            {
                return Err(invalid_identity());
            }
            let Some(parent) = child.parent else {
                return Err(invalid_identity());
            };
            if parent.0 >= child.id.0
                || (child.owner.has_joined_work() && !fiber_ids.contains(&parent.0))
                || child
                    .owner
                    .dialogue_activation()
                    .is_some_and(|activation| !parent.matches_persistent(activation.owner_fiber()))
            {
                return Err(invalid_identity());
            }
        }
        Ok(())
    }

    pub(super) fn admit_invocation_result(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        V: crate::value::RuntimeStoredValue,
    {
        let Some((root, value)) = &self.invocation_result else {
            return Ok(());
        };
        let invalid =
            |message: String| crate::value::AwbcRuntimeValueSnapshotError::Message { message };
        let expected = match root {
            NativeInvocationRoot::Program(program) => {
                admission.validate_metadata(program)?;
                self.plan
                    .pure_program_binding(*program)
                    .map_err(|error| invalid(error.to_string()))?
                    .result_type()
            }
            NativeInvocationRoot::Function(site) => {
                admission.validate_metadata(&site.get())?;
                let function = crate::pure::RuntimePureFunctionRef::resolve(&self.plan, *site)
                    .map_err(|error| invalid(error.to_string()))?;
                self.plan
                    .type_table()
                    .get(function.result_type())
                    .ok_or_else(|| {
                        invalid("native function result has an unknown original type".into())
                    })?
                    .semantic_identity()
            }
        };
        admission.validate_owned_typed_borrowed(value.borrowed_value(), expected)
    }

    pub(super) fn admit_publications(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        let invalid = |message: &str| crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: message.to_owned(),
        };
        if self.latest_need_publications.len() != self.need_publication_frontiers.len() {
            return Err(invalid("native latest publications and frontiers differ"));
        }
        for (correlation, latest) in &self.latest_need_publications {
            admission.validate_metadata(correlation)?;
            if latest.correlation() != *correlation
                || self.need_publication_frontiers.get(correlation) != Some(&latest.cursor())
            {
                return Err(invalid(
                    "native latest publication differs from its correlation/frontier",
                ));
            }
            latest.admit_retained(admission)?;
        }
        for (correlation, queue) in &self.need_publications {
            let mut previous = None;
            for publication in queue {
                let cursor = publication.cursor();
                if publication.correlation() != *correlation
                    || previous.is_some_and(|previous: TaskPublicationCursor| {
                        previous.compare_same_source(cursor) != Some(std::cmp::Ordering::Less)
                    })
                {
                    return Err(invalid(
                        "native queued publications are cross-correlation or unordered",
                    ));
                }
                publication.admit_retained(admission)?;
                previous = Some(cursor);
            }
            if previous.is_some()
                && self.need_publication_frontiers.get(correlation) != previous.as_ref()
            {
                return Err(invalid(
                    "native queued publication tail differs from its frontier",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn expected_deferred_children(
        &self,
        inflight: impl Fn(
            &DialogueActivationId,
        ) -> Option<(
            crate::runtime_id::RuntimeDeferRegistrationId,
            crate::runtime_id::RuntimeDeferSiteId,
        )>,
    ) -> Result<
        BTreeMap<
            DialogueActivationId,
            (
                crate::runtime_id::RuntimeDeferRegistrationId,
                crate::runtime_id::RuntimeDeferSiteId,
            ),
        >,
        String,
    > {
        let mut expected_deferred_children = BTreeMap::new();
        for child in &self.child_fibers {
            let FlowFiberOwner::LineTask(child_owner) = &child.owner else {
                continue;
            };
            let crate::line_task::LineTaskWork::Defer(id) = child_owner.tag.work() else {
                continue;
            };
            let activation = child_owner.tag.activation_id();
            let Some((inflight_id, site)) = inflight(activation) else {
                // Scoped defers have a frame-owned in-flight marker instead.
                continue;
            };
            if inflight_id != id
                || expected_deferred_children
                    .insert(activation.clone(), (id, site))
                    .is_some()
            {
                return Err("native deferred child custody is inconsistent".to_owned());
            }
        }
        Ok(expected_deferred_children)
    }
}

impl NativeEngineRollbackImage {
    pub(super) fn admit_rollback_values(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<
        BTreeMap<
            DialogueActivationId,
            (
                crate::runtime_id::RuntimeDeferRegistrationId,
                crate::runtime_id::RuntimeDeferSiteId,
            ),
        >,
        String,
    > {
        let crate::task::RuntimeProgramOwner::Plan(expected) = owner else {
            return Err("native rollback requires its original Plan executable".into());
        };
        if !Arc::ptr_eq(expected, &self.plan) {
            return Err("native rollback belongs to a different original executable".into());
        }
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.admit_metadata(&mut admission)
            .map_err(|error| error.to_string())?;
        self.admit_invocation_result(&mut admission)
            .map_err(|error| error.to_string())?;
        self.need_producers
            .admit_retained(&mut admission)
            .map_err(|error| error.to_string())?;
        self.admit_publications(&mut admission)
            .map_err(|error| error.to_string())?;
        if let Some(root) = &self.root {
            root.admit_retained(&mut admission)
                .map_err(|error| error.to_string())?;
        }
        self.admit_root_execution()?;
        self.validate_scheduled_scope_state(&mut admission)?;
        for fiber in std::iter::once(&self.fiber).chain(&self.child_fibers) {
            fiber
                .frame
                .admit_owned_values(&mut admission)
                .map_err(|error| error.to_string())?;
        }
        let expected_deferred_children = self.expected_deferred_children(|activation| {
            self.dialogue_activations
                .active_line(activation)
                .and_then(crate::line_task::RuntimeDialogueActivationState::deferred_inflight)
        })?;
        Ok(expected_deferred_children)
    }
}
