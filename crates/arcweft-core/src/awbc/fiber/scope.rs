//! Lexical cleanup custody, with one grammar for live and inert values.

use super::input::FrameValueContext;
use super::{
    AwbcEffectPlanId, AwbcFunctionKind, AwbcInstruction, AwbcRegisterId, AwbcRuntimeType,
    AwbcRuntimeTypeShape, AwbcSaveResult, AwbcScopeId, FiberCursor, FiberDeferredInFlight,
    FiberDeferredRelease, FiberStateError, FiberTrap, RuntimeProgramOwner,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::presentation::resources::{
    PresentationScopeExit, PresentationScopeRef, PresentationScopeTarget,
};
use crate::program_types::RuntimeProgramValueType;
use crate::runtime_id::{RuntimeDeferRegistrationId, RuntimeDeferSiteId};
use crate::value::{AwbcRuntimeValueSnapshot, RuntimeValue, RuntimeValueView};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

impl<V, O, N, B, R, G> super::FiberState<V, O, N, B, R, G> {
    /// Every real frame that retains values or grants, including a Flow that
    /// owns its inputs while the previous invocation stack is leaving.
    pub(in crate::awbc) fn retained_frames(
        &self,
    ) -> impl Iterator<Item = &super::FiberFrame<V, B, R, G>> {
        self.frames.iter().chain(
            self.pending_flow_transfer
                .iter()
                .map(|pending| &pending.destination),
        )
    }

    pub(in crate::awbc) fn has_exit_work(&self) -> bool {
        self.pending_flow_transfer.is_some() || self.has_deferred_work()
    }
    pub(in crate::awbc) fn has_inflight_defer(&self) -> bool {
        self.frames
            .iter()
            .flat_map(|frame| frame.deferred_scopes())
            .any(|(_, scope)| scope.defer_inflight.is_some())
    }

    pub(in crate::awbc) fn has_pending_deferred_cleanup(&self) -> bool {
        self.frames
            .iter()
            .flat_map(|frame| frame.deferred_scopes())
            .any(|(_, scope)| scope.defer_inflight.is_some() || !scope.defer_releasing.is_empty())
    }

    pub(in crate::awbc) fn has_deferred_work(&self) -> bool {
        self.frames
            .iter()
            .flat_map(|frame| frame.deferred_scopes())
            .any(|(_, scope)| !scope.is_empty())
    }
}

impl<V, O, N, B, R, G: crate::presentation::resources::PresentationScopeStorage>
    super::FiberState<V, O, N, B, R, G>
{
    pub(super) fn admit_presentation_scopes(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> AwbcSaveResult<()> {
        let result = (|| {
            admission.validate_metadata(&self.presentation_bank)?;
            self.root
                .validate_presentation_bank(self.presentation_bank)?;
            for frame in self.retained_frames() {
                if let Some(binding) = &frame.root_presentation_scope {
                    binding.admit_awbc_scope(admission, self.instance, frame.instance, None)?;
                }
                for scope in &frame.scopes {
                    if let Some(binding) = &scope.presentation_scope {
                        binding.admit_awbc_scope(
                            admission,
                            self.instance,
                            frame.instance,
                            Some(scope.id),
                        )?;
                    }
                }
            }
            Ok(())
        })();
        result.inspect_err(|error: &crate::value::AwbcRuntimeValueSnapshotError| {
            admission.retain_failure(error.clone())
        })
    }
}

impl super::FiberFrame {
    pub(crate) fn presentation_scope_ref(&self) -> PresentationScopeRef<'_> {
        match self.scopes.last() {
            Some(scope) => PresentationScopeRef::awbc(
                scope.presentation_scope.as_ref(),
                self.instance,
                Some(scope.id),
            ),
            None => PresentationScopeRef::awbc(
                self.root_presentation_scope.as_ref(),
                self.instance,
                None,
            ),
        }
    }

    pub(crate) fn presentation_scope_binding(
        &self,
    ) -> Option<&crate::presentation::RuntimePresentationScopeBinding> {
        match self.scopes.last() {
            Some(scope) => scope.presentation_scope.as_ref(),
            None => self.root_presentation_scope.as_ref(),
        }
    }

    pub(crate) fn presentation_scope_target(
        &mut self,
    ) -> crate::presentation::resources::PresentationScopeTarget<'_> {
        match self.scopes.last_mut() {
            Some(scope) => crate::presentation::resources::PresentationScopeTarget::awbc(
                &mut scope.presentation_scope,
                self.instance,
                Some(scope.id),
            ),
            None => crate::presentation::resources::PresentationScopeTarget::awbc(
                &mut self.root_presentation_scope,
                self.instance,
                None,
            ),
        }
    }
}

impl super::FiberState {
    pub(crate) fn presentation_scope_bindings(
        &self,
    ) -> impl Iterator<Item = &crate::presentation::RuntimePresentationScopeBinding> {
        self.retained_frames().flat_map(|frame| {
            frame.root_presentation_scope.iter().chain(
                frame
                    .scopes
                    .iter()
                    .filter_map(|scope| scope.presentation_scope.as_ref()),
            )
        })
    }

    /// The exiting callee and the caller's actual current lexical destination.
    pub(crate) fn presentation_return_scopes_mut(
        &mut self,
    ) -> Result<
        (
            Vec<PresentationScopeExit<PresentationScopeTarget<'_>>>,
            Option<PresentationScopeExit<PresentationScopeTarget<'_>>>,
        ),
        crate::presentation::RuntimePresentationError,
    > {
        self.root
            .validate_presentation_bank(self.presentation_bank)?;
        let execution = crate::runtime_id::ExecutionInstanceId::from_allocated(self.instance.get());
        let fiber =
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(self.instance.get().get());
        let (callee, callers) = self
            .frames
            .split_last_mut()
            .ok_or(crate::presentation::RuntimePresentationError::MissingScope)?;
        let mut targets = Vec::new();
        for (scope, binding) in callee
            .scopes
            .iter_mut()
            .rev()
            .map(|scope| (Some(scope.id), &mut scope.presentation_scope))
            .chain(std::iter::once((None, &mut callee.root_presentation_scope)))
        {
            if binding.is_some() {
                targets.push(PresentationScopeExit {
                    execution,
                    fiber,
                    scope: PresentationScopeTarget::awbc(binding, callee.instance, scope),
                });
            }
        }
        let destination = callers.last_mut().map(|caller| PresentationScopeExit {
            execution,
            fiber,
            scope: caller.presentation_scope_target(),
        });
        Ok((targets, destination))
    }

    pub(crate) fn presentation_unwind_scopes(
        &self,
    ) -> Result<
        Vec<PresentationScopeExit<PresentationScopeRef<'_>>>,
        crate::presentation::RuntimePresentationError,
    > {
        self.root
            .validate_presentation_bank(self.presentation_bank)?;
        let execution = crate::runtime_id::ExecutionInstanceId::from_allocated(self.instance.get());
        let fiber =
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(self.instance.get().get());
        let mut targets = Vec::new();
        for frame in self.frames.iter().rev() {
            for (scope, binding) in frame
                .scopes
                .iter()
                .rev()
                .map(|scope| (Some(scope.id), scope.presentation_scope.as_ref()))
                .chain(std::iter::once((
                    None,
                    frame.root_presentation_scope.as_ref(),
                )))
            {
                if let Some(binding) = binding {
                    targets.push(PresentationScopeExit {
                        execution,
                        fiber,
                        scope: PresentationScopeRef::awbc(Some(binding), frame.instance, scope),
                    });
                }
            }
        }
        Ok(targets)
    }

    pub(crate) fn presentation_unwind_scopes_mut(
        &mut self,
    ) -> Result<
        Vec<PresentationScopeExit<PresentationScopeTarget<'_>>>,
        crate::presentation::RuntimePresentationError,
    > {
        self.root
            .validate_presentation_bank(self.presentation_bank)?;
        let execution = crate::runtime_id::ExecutionInstanceId::from_allocated(self.instance.get());
        let fiber =
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(self.instance.get().get());
        let mut targets = Vec::new();
        for frame in self.frames.iter_mut().rev() {
            for (scope, binding) in frame
                .scopes
                .iter_mut()
                .rev()
                .map(|scope| (Some(scope.id), &mut scope.presentation_scope))
                .chain(std::iter::once((None, &mut frame.root_presentation_scope)))
            {
                if binding.is_some() {
                    targets.push(PresentationScopeExit {
                        execution,
                        fiber,
                        scope: PresentationScopeTarget::awbc(binding, frame.instance, scope),
                    });
                }
            }
        }
        Ok(targets)
    }

    /// Moves the current real scope's bodies into its execution owner's queue.
    /// A VM may borrow the owner, but cannot create a replacement ledger.
    pub(crate) fn retire_presentation_scope<V>(
        &mut self,
        program: &RuntimeProgramOwner,
        banks: &mut crate::presentation::resources::PresentationBanks<V>,
    ) -> Result<(), crate::presentation::RuntimePresentationError> {
        if !matches!(program, RuntimeProgramOwner::Awbc(_)) {
            return Err(crate::presentation::RuntimePresentationError::Program);
        }
        let execution = crate::runtime_id::ExecutionInstanceId::from_allocated(self.instance.get());
        let fiber =
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(self.instance.get().get());
        let mut escaping = Vec::new();
        let final_root = self.frames.len() == 1
            && self
                .active_frame()
                .is_ok_and(|frame| frame.scopes.is_empty());
        if let Some(pending) = &self.pending_flow_transfer {
            for value in pending
                .destination
                .registers
                .iter()
                .flat_map(|slot| slot.values())
            {
                for handle in value
                    .view()
                    .affine_handles()
                    .map_err(|_| crate::presentation::RuntimePresentationError::InvalidState)?
                {
                    if let crate::value::ownership::RuntimeAffineHandle::Presentation {
                        token,
                        ..
                    } = handle
                    {
                        escaping.push(token);
                    }
                }
            }
        }
        let frame = self
            .frames
            .last_mut()
            .ok_or(super::FiberStateError::MissingFrame)
            .map_err(|_| crate::presentation::RuntimePresentationError::MissingScope)?;
        let destination =
            self.pending_flow_transfer
                .as_mut()
                .map(|pending| PresentationScopeExit {
                    execution,
                    fiber,
                    scope: pending.destination.presentation_scope_target(),
                });
        let targets = vec![PresentationScopeExit {
            execution,
            fiber,
            scope: frame.presentation_scope_target(),
        }];
        if destination.is_some() && !final_root {
            banks.transition_exiting_arguments(program, targets, destination, &escaping)
        } else {
            banks.transition_scope_contour(program, targets, destination, &escaping, &[])
        }
    }

    pub(crate) fn bind_presentation_scope<V>(
        &mut self,
        program: &RuntimeProgramOwner,
        ledger: &mut crate::presentation::resources::PresentationLedger<V>,
    ) -> Result<
        crate::presentation::resources::PresentationScopeOwner,
        crate::presentation::RuntimePresentationError,
    > {
        if !matches!(program, RuntimeProgramOwner::Awbc(_)) {
            return Err(crate::presentation::RuntimePresentationError::Program);
        }
        if ledger.bank() != self.presentation_bank {
            return Err(crate::presentation::RuntimePresentationError::Owner);
        }
        let execution = crate::runtime_id::ExecutionInstanceId::from_allocated(self.instance.get());
        let fiber =
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(self.instance.get().get());
        let frame = self
            .active_frame_mut()
            .map_err(|_| crate::presentation::RuntimePresentationError::MissingScope)?;
        ledger.bind_scope(program, execution, fiber, frame.presentation_scope_target())
    }

    pub(crate) fn release_presentation_scope<V>(
        &mut self,
        program: &RuntimeProgramOwner,
        ledger: &mut crate::presentation::resources::PresentationLedger<V>,
    ) -> Result<
        Vec<crate::presentation::resources::PresentationRetirement<V>>,
        crate::presentation::RuntimePresentationError,
    > {
        if !matches!(program, RuntimeProgramOwner::Awbc(_)) {
            return Err(crate::presentation::RuntimePresentationError::Program);
        }
        if ledger.bank() != self.presentation_bank {
            return Err(crate::presentation::RuntimePresentationError::Owner);
        }
        let execution = crate::runtime_id::ExecutionInstanceId::from_allocated(self.instance.get());
        let fiber =
            crate::runtime_id::RuntimePersistentFiberId::from_allocated(self.instance.get().get());
        let frame = self
            .active_frame_mut()
            .map_err(|_| crate::presentation::RuntimePresentationError::MissingScope)?;
        ledger.exit_bound_scope(program, execution, fiber, frame.presentation_scope_target())
    }
}

#[cfg(test)]
mod tests;

// A private codec capability restricts all three owning grammars to the inert
// Core carrier. Derive traverses these fields directly: no parallel wire record,
// live reader, or externally implementable serialization capability exists.

/// One reached lexical scope and its outstanding cleanup custody.
/// Only the inert projection implements serde.
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberScope;
/// let _: FiberScope = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberScope;
/// fn encode_live(scope: &FiberScope) { let _ = serde_json::to_vec(scope); }
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: super::InertRuntimeValueStorage, G: Serialize",
        deserialize = "V: super::InertRuntimeValueStorage, G: Deserialize<'de>"
    )
)]
pub struct FiberScope<V = RuntimeValue, G = crate::presentation::RuntimePresentationScopeBinding> {
    pub id: AwbcScopeId,
    pub depth: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation_scope: Option<G>,
    pub cleanups: Vec<FiberScopeCleanup<V>>,
    pub deferred: FiberDeferredScope<V>,
}

/// Deferred custody shared by an invocation body and an explicit lexical scope.
/// Only inert value storage implements the version 1 save grammar.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: super::InertRuntimeValueStorage",
        deserialize = "V: super::InertRuntimeValueStorage"
    )
)]
pub struct FiberDeferredScope<V = RuntimeValue> {
    pub defers: Vec<FiberDeferredRegistration<V>>,
    /// Only token identities remain after an outcome-filtered packet drops.
    pub defer_releasing: Vec<FiberDeferredRelease>,
    /// The lexical exit is fixed before the first deferred body starts.
    pub defer_exit: Option<crate::line_task::ScopeExit>,
    pub defer_inflight: Option<FiberDeferredInFlight>,
    /// The first failure does not change the exit used by later registrations.
    pub defer_failure: Option<FiberTrap>,
}

impl<V> FiberDeferredScope<V> {
    pub const fn new() -> Self {
        Self {
            defers: Vec::new(),
            defer_releasing: Vec::new(),
            defer_exit: None,
            defer_inflight: None,
            defer_failure: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.defers.is_empty()
            && self.defer_releasing.is_empty()
            && self.defer_exit.is_none()
            && self.defer_inflight.is_none()
            && self.defer_failure.is_none()
    }
}

impl<V> Default for FiberDeferredScope<V> {
    fn default() -> Self {
        Self::new()
    }
}

/// Captures at the exact reached executable defer instruction.
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberDeferredRegistration;
/// let _: FiberDeferredRegistration = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberDeferredRegistration;
/// fn encode_live(packet: &FiberDeferredRegistration) { let _ = serde_json::to_vec(packet); }
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: super::InertRuntimeValueStorage",
        deserialize = "V: super::InertRuntimeValueStorage"
    )
)]
pub struct FiberDeferredRegistration<V = RuntimeValue> {
    pub id: RuntimeDeferRegistrationId,
    pub site: RuntimeDeferSiteId,
    pub source: FiberCursor,
    pub outcome: crate::line_task::RuntimeDeferOutcomeFilter,
    pub capture_registers: Vec<AwbcRegisterId>,
    pub captures: Vec<V>,
}

/// An outstanding effect cleanup under its original effect-plan signature.
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberScopeCleanup;
/// let _: FiberScopeCleanup = serde_json::from_str("{}").unwrap();
/// ```
///
/// ```compile_fail
/// use arcweft_core::awbc::fiber::FiberScopeCleanup;
/// fn encode_live(cleanup: &FiberScopeCleanup) { let _ = serde_json::to_vec(cleanup); }
/// ```
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: super::InertRuntimeValueStorage",
        deserialize = "V: super::InertRuntimeValueStorage"
    )
)]
pub struct FiberScopeCleanup<V = RuntimeValue> {
    pub source: FiberCursor,
    pub args: Vec<V>,
}

/// A cleanup already admitted against its original registration instruction,
/// ready to dispatch after its custody leaves the frame. It has no save codec.
#[derive(Debug)]
pub(in crate::awbc) struct FiberCleanupDispatch {
    pub(in crate::awbc) effect: AwbcEffectPlanId,
    pub(in crate::awbc) args: Vec<RuntimeValue>,
}

struct CleanupDeclaration<'a> {
    key: &'a str,
    effect: AwbcEffectPlanId,
    registers: &'a [AwbcRegisterId],
    signature: &'a crate::awbc::schema::AwbcSignature,
}

impl<V> FiberScopeCleanup<V> {
    fn declaration(
        program: &super::AwbcProgram,
        source: FiberCursor,
    ) -> Result<CleanupDeclaration<'_>, FiberStateError> {
        let AwbcInstruction::RegisterCleanup { key, effect, args } =
            super::instruction_at_site(program, source)?
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        let key = program
            .strings
            .get(key.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let plan = program
            .effect_plans
            .get(effect.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        plan.payload.require_context_free()?;
        let signature = program
            .signatures
            .get(plan.signature.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        if key.is_empty() || signature.params.len() != args.len() {
            return Err(FiberStateError::InvalidFrame);
        }
        Ok(CleanupDeclaration {
            key,
            effect: *effect,
            registers: args,
            signature,
        })
    }

    /// Reads the cancellation key from the sole original instruction authority.
    ///
    /// # Errors
    /// Returns an error if the source does not name a complete cleanup declaration.
    pub fn key<'program>(
        &self,
        program: &'program super::AwbcProgram,
    ) -> Result<&'program str, FiberStateError> {
        Self::declaration(program, self.source).map(|declaration| declaration.key)
    }

    pub(super) fn validate_inputs(
        &self,
        frame: &FrameValueContext<'_, V>,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_>,
    ) -> Result<(), FiberStateError> {
        Self::validate_argument_sequence(
            frame,
            self.source,
            self.args.iter().map(Ok),
            resources,
            view,
        )
    }

    fn validate_argument_sequence<'value>(
        frame: &FrameValueContext<'_, V>,
        source: FiberCursor,
        args: impl ExactSizeIterator<Item = Result<&'value V, FiberStateError>>,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&'value V) -> RuntimeValueView<'value>,
    ) -> Result<(), FiberStateError>
    where
        V: 'value,
    {
        let mut work = resources.work();
        // Header, function/block/instruction and key/effect/signature lookups.
        for _ in 0..7 {
            work.charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        }
        work.collection(args.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let declaration = Self::declaration(frame.program, source)?;
        resources
            .metadata_string(declaration.key)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if source.function != frame.function || args.len() != declaration.registers.len() {
            return Err(FiberStateError::InvalidFrame);
        }
        for ((value, register), expected) in args
            .zip(declaration.registers)
            .zip(&declaration.signature.params)
        {
            work.charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            let slot = frame
                .layout
                .slots
                .get(register.index())
                .ok_or(FiberStateError::InvalidFrame)?;
            let value = view(value?);
            resources
                .validate_view(value, 0, None)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            for ty in [slot.ty, *expected] {
                RuntimeProgramValueType::Awbc {
                    program: frame.program,
                    ty,
                }
                .validate_admitted_view(value, frame.binding, resources)
                .map_err(|error| {
                    FiberStateError::Snapshot(
                        crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(error)),
                    )
                })?;
            }
        }
        Ok(())
    }
}
impl<V> FiberDeferredRegistration<V> {
    pub(super) fn validate_inputs(
        &self,
        frame: &FrameValueContext<'_, V>,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_>,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .collection(self.capture_registers.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if self.capture_registers.len() != self.captures.len() {
            return Err(FiberStateError::InvalidFrame);
        }
        Self::validate_capture_sequence(
            frame,
            self.source,
            self.site,
            self.outcome,
            self.capture_registers.iter().copied().zip(&self.captures),
            resources,
            view,
        )
    }

    fn validate_capture_sequence<'value>(
        frame: &FrameValueContext<'_, V>,
        source: FiberCursor,
        site: RuntimeDeferSiteId,
        outcome: crate::line_task::RuntimeDeferOutcomeFilter,
        captures: impl ExactSizeIterator<Item = (AwbcRegisterId, &'value V)>,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&'value V) -> RuntimeValueView<'value>,
    ) -> Result<(), FiberStateError>
    where
        V: 'value,
    {
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        resources
            .work()
            .collection(captures.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let signature = Self::signature(frame, site, resources)?;
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let AwbcInstruction::RegisterDefer {
            site: declared_site,
            outcome: declared_outcome,
            owner,
            captures: declared_captures,
        } = super::instruction_at_site(frame.program, source)?
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        if source.function != frame.function
            || *declared_site != site
            || *declared_outcome != outcome
            || *owner != crate::awbc::schema::AwbcDeferOwner::CurrentScope
            || declared_captures.len() != captures.len()
            || signature.params.len() != captures.len()
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let mut seen_registers = BTreeSet::new();
        for (((register, capture), expected), declared_register) in
            captures.zip(&signature.params).zip(declared_captures)
        {
            if register != *declared_register {
                return Err(FiberStateError::InvalidFrame);
            }
            let slot = frame
                .layout
                .slots
                .get(register.index())
                .ok_or(FiberStateError::InvalidFrame)?;
            let value = view(capture);
            let mut affine = false;
            let mut observe = |child: RuntimeValueView<'_>| {
                affine |= matches!(child, RuntimeValueView::Need(_))
                    || matches!(child, RuntimeValueView::Opaque(value) if !value.value_class().intrinsic_ownership().permits_copy());
                Ok(())
            };
            // Admit this actual capture once. Both declaration obligations and
            // Copy custody inspect that same borrowed graph and resource bank.
            resources
                .validate_view(value, 0, Some(&mut observe))
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            if !seen_registers.insert(register) && affine {
                return Err(FiberStateError::InvalidFrame);
            }
            for ty in [slot.ty, *expected] {
                RuntimeProgramValueType::Awbc {
                    program: frame.program,
                    ty,
                }
                .validate_admitted_view(value, frame.binding, resources)
                .map_err(|error| {
                    FiberStateError::Snapshot(
                        crate::value::AwbcRuntimeValueSnapshotError::ProgramType(Box::new(error)),
                    )
                })?;
            }
        }
        Ok(())
    }

    fn signature<'a>(
        frame: &FrameValueContext<'a, V>,
        site: RuntimeDeferSiteId,
        resources: &ValueResourceAdmission,
    ) -> Result<&'a crate::awbc::schema::AwbcSignature, FiberStateError> {
        let mut work = resources.work();
        for _ in 0..3 {
            work.charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        }
        let function_id = frame
            .program
            .defer_sites
            .get(site.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let function = frame
            .program
            .functions
            .get(function_id.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let signature = frame
            .program
            .signatures
            .get(function.signature.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        if function.kind != AwbcFunctionKind::Ordinary
            || !signature.result.is_some_and(|result| {
                matches!(
                    frame
                        .program
                        .runtime_types
                        .get(result.index())
                        .map(AwbcRuntimeType::shape),
                    Some(AwbcRuntimeTypeShape::Unit)
                )
            })
        {
            return Err(FiberStateError::InvalidFrame);
        }
        Ok(signature)
    }
}

impl<V, G> FiberScope<V, G> {
    pub(super) fn validate_inputs(
        &self,
        frame: &FrameValueContext<'_, V>,
        ordinal: usize,
        parent: Option<AwbcScopeId>,
        ids: &mut BTreeSet<RuntimeDeferRegistrationId>,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_> + Copy,
    ) -> Result<(), FiberStateError> {
        resources
            .work()
            .charge(0)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        resources
            .work()
            .collection(self.cleanups.len())
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        let definition = frame
            .layout
            .scopes
            .get(self.id.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        if self.depth as usize != ordinal
            || ordinal >= frame.layout.max_scope_depth as usize
            || definition.parent != parent
        {
            return Err(FiberStateError::InvalidFrame);
        }
        for cleanup in &self.cleanups {
            cleanup.validate_inputs(frame, resources, view)?;
        }
        self.deferred.validate_inputs(frame, ids, resources, view)
    }
}

impl<V> FiberDeferredScope<V> {
    pub(super) fn validate_inputs(
        &self,
        frame: &FrameValueContext<'_, V>,
        ids: &mut BTreeSet<RuntimeDeferRegistrationId>,
        resources: &mut ValueResourceAdmission,
        view: impl Fn(&V) -> RuntimeValueView<'_> + Copy,
    ) -> Result<(), FiberStateError> {
        // The containing frame or lexical scope admits its identity once.
        // This inline storage shares that owner's resource bank.
        for count in [self.defers.len(), self.defer_releasing.len()] {
            resources
                .work()
                .collection(count)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        }
        let started = match (self.defer_inflight, self.defer_releasing.as_slice()) {
            (running, []) => running.map(|row| row.registration),
            (None, [releasing]) => Some(releasing.registration),
            _ => return Err(FiberStateError::InvalidFrame),
        };
        if (self.defer_exit.is_none()
            && (self.defer_inflight.is_some()
                || self.defer_failure.is_some()
                || !self.defer_releasing.is_empty()))
            || !crate::scope::defer_registration_stack_is_ordered(
                self.defers.iter().map(|row| row.id),
                started,
            )
        {
            return Err(FiberStateError::InvalidFrame);
        }
        for deferred in &self.defers {
            if !ids.insert(deferred.id) {
                return Err(FiberStateError::InvalidFrame);
            }
            deferred.validate_inputs(frame, resources, view)?;
        }
        if let Some(inflight) = self.defer_inflight {
            if !ids.insert(inflight.registration) {
                return Err(FiberStateError::InvalidFrame);
            }
            FiberDeferredRegistration::<V>::signature(frame, inflight.site, resources)?;
        }
        for release in &self.defer_releasing {
            resources
                .work()
                .collection(release.tokens.len())
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            if !ids.insert(release.registration) || release.tokens.is_empty() {
                return Err(FiberStateError::InvalidFrame);
            }
            FiberDeferredRegistration::<V>::signature(frame, release.site, resources)?;
            let mut tokens = BTreeSet::new();
            for token in &release.tokens {
                resources
                    .work()
                    .charge(0)
                    .map_err(|error| FiberStateError::Snapshot(error.into()))?;
                if !tokens.insert(token) {
                    return Err(FiberStateError::InvalidFrame);
                }
            }
        }
        if let Some(failure) = &self.defer_failure {
            resources
                .work()
                .charge(0)
                .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            if let Some(message) = &failure.message {
                resources
                    .metadata_string(message)
                    .map_err(|error| FiberStateError::Snapshot(error.into()))?;
            }
            if failure
                .source_map
                .is_some_and(|id| id.index() >= frame.program.source_map.len())
            {
                return Err(FiberStateError::InvalidFrame);
            }
        }
        Ok(())
    }
}

impl FiberDeferredRegistration {
    /// Admits the reached instruction and borrowed capture packet before the
    /// line owner copies registers or commits its registration transaction.
    pub(in crate::awbc) fn validate_registration_inputs(
        program: &super::AwbcProgram,
        frame: &super::FiberFrame,
        source: FiberCursor,
        site: RuntimeDeferSiteId,
        outcome: crate::line_task::RuntimeDeferOutcomeFilter,
        captures: &[(AwbcRegisterId, RuntimeValue)],
    ) -> Result<(), FiberStateError> {
        let context = FrameValueContext {
            program,
            function: frame.function,
            layout: program
                .frame_layouts
                .get(frame.layout.index())
                .ok_or(FiberStateError::InvalidFrame)?,
            binding: frame.type_instantiation.as_deref(),
            registers: &frame.registers,
            scope_count: frame.scopes.len(),
        };
        Self::validate_capture_sequence(
            &context,
            source,
            site,
            outcome,
            captures.iter().map(|(register, value)| (*register, value)),
            &mut ValueResourceAdmission::live(crate::entry::RuntimeSchemaLimits::engine_default()),
            RuntimeValue::view,
        )
    }
}
impl FiberScopeCleanup {
    /// Checks the reached declaration and borrowed argument registers before
    /// any register moves into cleanup custody.
    pub(in crate::awbc) fn validate_registration_inputs(
        program: &super::AwbcProgram,
        frame: &super::FiberFrame,
        source: FiberCursor,
    ) -> Result<(), FiberStateError> {
        let context = FrameValueContext {
            program,
            function: frame.function,
            layout: program
                .frame_layouts
                .get(frame.layout.index())
                .ok_or(FiberStateError::InvalidFrame)?,
            binding: frame.type_instantiation.as_deref(),
            registers: &frame.registers,
            scope_count: frame.scopes.len(),
        };
        let declaration = Self::declaration(program, source)?;
        Self::validate_argument_sequence(
            &context,
            source,
            declaration
                .registers
                .iter()
                .map(|register| frame.register(*register)),
            &mut ValueResourceAdmission::live(crate::entry::RuntimeSchemaLimits::engine_default()),
            RuntimeValue::view,
        )
    }

    pub(in crate::awbc) fn into_dispatch_admitted(
        self,
        program: &super::AwbcProgram,
    ) -> FiberCleanupDispatch {
        let declaration = Self::declaration(program, self.source)
            .expect("admitted cleanup retains its original instruction");
        FiberCleanupDispatch {
            effect: declaration.effect,
            args: self.args,
        }
    }
    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberScopeCleanup<AwbcRuntimeValueSnapshot>> {
        Ok(FiberScopeCleanup {
            source: self.source,
            args: self
                .args
                .iter()
                .map(|value| AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner))
                .collect::<Result<_, _>>()?,
        })
    }
}
impl FiberScopeCleanup<AwbcRuntimeValueSnapshot> {
    pub(super) fn into_live_admitted<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> AwbcSaveResult<FiberScopeCleanup> {
        let owner = owner.into();
        Ok(FiberScopeCleanup {
            source: self.source,
            args: self
                .args
                .into_iter()
                .map(|value| value.into_admitted_runtime_value(owner))
                .collect::<Result<_, _>>()?,
        })
    }
}
impl FiberDeferredRegistration {
    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberDeferredRegistration<AwbcRuntimeValueSnapshot>> {
        Ok(FiberDeferredRegistration {
            id: self.id,
            site: self.site,
            source: self.source,
            outcome: self.outcome,
            capture_registers: self.capture_registers.clone(),
            captures: self
                .captures
                .iter()
                .map(|value| AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner))
                .collect::<Result<_, _>>()?,
        })
    }
}
impl FiberDeferredRegistration<AwbcRuntimeValueSnapshot> {
    pub(super) fn into_live_admitted<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> AwbcSaveResult<FiberDeferredRegistration> {
        let owner = owner.into();
        Ok(FiberDeferredRegistration {
            id: self.id,
            site: self.site,
            source: self.source,
            outcome: self.outcome,
            capture_registers: self.capture_registers,
            captures: self
                .captures
                .into_iter()
                .map(|value| value.into_admitted_runtime_value(owner))
                .collect::<Result<_, _>>()?,
        })
    }
}
impl FiberScope {
    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<super::AwbcFiberScopeSnapshot> {
        Ok(FiberScope {
            id: self.id,
            depth: self.depth,
            presentation_scope: self
                .presentation_scope
                .as_ref()
                .map(|binding| binding.snapshot_admitted(owner))
                .transpose()?,
            cleanups: self
                .cleanups
                .iter()
                .map(|value| value.snapshot_admitted(owner))
                .collect::<Result<_, _>>()?,
            deferred: self.deferred.snapshot_admitted(owner)?,
        })
    }
}
impl super::AwbcFiberScopeSnapshot {
    pub(super) fn into_live_admitted<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> AwbcSaveResult<FiberScope> {
        let owner = owner.into();
        Ok(FiberScope {
            id: self.id,
            depth: self.depth,
            presentation_scope: self
                .presentation_scope
                .map(|snapshot| snapshot.into_admitted_live(owner.owner())),
            cleanups: self
                .cleanups
                .into_iter()
                .map(|value| value.into_live_admitted(owner))
                .collect::<Result<_, _>>()?,
            deferred: self.deferred.into_live_admitted(owner)?,
        })
    }
}

impl FiberDeferredScope {
    pub(super) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> AwbcSaveResult<FiberDeferredScope<AwbcRuntimeValueSnapshot>> {
        Ok(FiberDeferredScope {
            defers: self
                .defers
                .iter()
                .map(|value| value.snapshot_admitted(owner))
                .collect::<Result<_, _>>()?,
            defer_releasing: self.defer_releasing.clone(),
            defer_exit: self.defer_exit,
            defer_inflight: self.defer_inflight,
            defer_failure: self.defer_failure.clone(),
        })
    }
}

impl FiberDeferredScope<AwbcRuntimeValueSnapshot> {
    pub(super) fn into_live_admitted<'a>(
        self,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
    ) -> AwbcSaveResult<FiberDeferredScope> {
        let owner = owner.into();
        Ok(FiberDeferredScope {
            defers: self
                .defers
                .into_iter()
                .map(|value| value.into_live_admitted(owner))
                .collect::<Result<_, _>>()?,
            defer_releasing: self.defer_releasing,
            defer_exit: self.defer_exit,
            defer_inflight: self.defer_inflight,
            defer_failure: self.defer_failure,
        })
    }
}
impl super::FiberState {
    pub(in crate::awbc) fn pop_exiting_scope(
        &mut self,
        program: &super::AwbcProgram,
        scope_id: Option<super::AwbcScopeId>,
    ) -> Result<(Vec<super::FiberCleanupDispatch>, Option<super::FiberTrap>), FiberStateError> {
        self.validate_cleanup_inputs(program)?;
        let frame = self.active_frame_mut()?;
        if scope_id.is_none() {
            if !frame.scopes.is_empty()
                || !frame.root_deferred.defers.is_empty()
                || !frame.root_deferred.defer_releasing.is_empty()
                || frame.root_deferred.defer_inflight.is_some()
            {
                return Err(FiberStateError::InvalidFrame);
            }
            let deferred = std::mem::take(&mut frame.root_deferred);
            return Ok((Vec::new(), deferred.defer_failure));
        }
        let Some(scope) = frame.scopes.last() else {
            return Err(FiberStateError::InvalidFrame);
        };
        if Some(scope.id) != scope_id
            || !scope.deferred.defers.is_empty()
            || !scope.deferred.defer_releasing.is_empty()
            || scope.deferred.defer_inflight.is_some()
        {
            return Err(FiberStateError::InvalidFrame);
        }
        let scope = frame
            .scopes
            .pop()
            .expect("active lexical scope was checked above");
        let layout = program
            .frame_layouts
            .get(frame.layout.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let active_scope_depth =
            u32::try_from(frame.scopes.len()).map_err(|_| FiberStateError::InvalidFrame)?;
        for (register, slot) in frame.registers.iter_mut().zip(&layout.slots) {
            if slot.scope_depth > active_scope_depth
                && !matches!(
                    slot.role,
                    crate::awbc::schema::AwbcFrameSlotRole::Parameter
                        | crate::awbc::schema::AwbcFrameSlotRole::RuntimeState
                )
            {
                *register = Default::default();
            }
        }
        Ok((
            scope
                .cleanups
                .into_iter()
                .map(|cleanup| cleanup.into_dispatch_admitted(program))
                .collect(),
            scope.deferred.defer_failure,
        ))
    }
}
