//! A task executes either a host request or an admitted original program row.

use super::descriptor::TaskDescriptorVisitor;
use super::{HostTaskRequest, TaskPlanSemanticDigest};
use crate::entry::RuntimeSchemaError;
use crate::plan::{RuntimeTaskSource, RuntimeTaskSourceAssertion};
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TaskExecutionKind {
    Host,
    Program,
}

/// Retains the exact accepted Program launch. Its private constructor excludes
/// Host request payloads, so cloning this immutable receipt cannot clone a live
/// runtime value. The original Source remains its only executable authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub(crate) struct ProgramTaskLaunch {
    receipt: std::sync::Arc<super::TaskSubmission>,
}

impl ProgramTaskLaunch {
    pub(super) fn retain(receipt: &super::TaskSubmission) -> Result<Self, super::TaskEnsureError> {
        if !matches!(&receipt.spec().request, TaskRequest::Program(_)) {
            return Err(super::TaskEnsureError::ProgramSourceMismatch);
        }
        receipt.spec().validate_task_descriptor()?;
        receipt.spec().validate_handle(receipt.handle())?;
        Ok(Self {
            receipt: std::sync::Arc::new(receipt.clone()),
        })
    }

    pub(crate) fn handle(&self) -> super::TaskHandle {
        self.receipt.handle()
    }

    pub(crate) fn spec(&self) -> &super::TaskSpec {
        self.receipt.spec()
    }

    pub(crate) fn source(&self) -> &RuntimeTaskSource {
        let TaskRequest::Program(source) = &self.receipt.spec().request else {
            unreachable!("the retained launch has no Host request payload")
        };
        source
    }

    pub(crate) fn matches_spec<P>(&self, spec: &super::TaskSpec<P>) -> bool {
        self.receipt.spec().same_program_contract(spec)
    }

    pub(crate) fn admit_result<V: crate::value::RuntimeStoredValue>(
        &self,
        value: &V,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.receipt.admit_retained(admission)?;
        admission.admit_composite(V::admission_origin(), |owner, resources| {
            let view = value.borrowed_view();
            resources.validate_view(view, 0, None)?;
            crate::value::AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                view, owner, resources,
            )?;
            self.receipt
                .spec()
                .validate_admitted_value_view(view, owner, resources)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    #[test]
    fn host_launch_rejection_preserves_the_original_journal_and_ordinal() {
        let need = crate::tests::reusable_need("program-launch-host-rejection");
        let mut spec = need.reusable_spec().unwrap().clone();
        spec.policy = super::super::TaskPolicy::AlwaysStart;
        let mut registry = super::super::NeedProducerRegistry::default();
        let frontiers = registry.launch_frontiers();
        assert_eq!(
            registry
                .ensure_program_task_with_binding::<(), super::super::TaskEnsureError>(
                    spec.clone(),
                    |_| { panic!("a Host packet must never publish a Program launch") }
                )
                .unwrap_err(),
            super::super::TaskEnsureError::ProgramSourceMismatch
        );
        assert_eq!(registry.launch_frontiers(), frontiers);
        assert!(registry.task_admissions().next().is_none());

        let accepted = registry.ensure_task(spec).unwrap();
        assert_eq!(accepted.handle().correlation.launch_ordinal.get(), 1);
        assert!(matches!(
            ProgramTaskLaunch::retain(&accepted),
            Err(super::super::TaskEnsureError::ProgramSourceMismatch)
        ));
    }
}

/// A terminal report issued only when Core retires the corresponding program
/// task body. Dispatch coordinates and publication order remain driver-owned.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct ProgramTaskCompletion {
    launch: ProgramTaskLaunch,
    terminal: ProgramTaskTerminal,
}

/// Status metadata for a program whose result remains owned by Core's task
/// state. This status alone does not grant original-body execution authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ProgramTaskTerminal {
    Completed,
    Failed(super::RuntimeTaskFailure),
    Cancelled,
}

/// Publication delivery is separate from the local result's ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ProgramTaskRetirementPhase {
    Executing,
    Unacknowledged,
    ReplayPending,
    Acknowledged,
}

pub(crate) enum ProgramTaskOutcome {
    Returned(crate::value::RuntimeValue),
    Failed(super::RuntimeTaskFailure),
    Cancelled,
}

pub(crate) struct ProgramTaskRetirementRejection {
    pub reason: super::NeedProducerAdmissionError,
    pub outcome: ProgramTaskOutcome,
}

impl ProgramTaskCompletion {
    pub fn handle(&self) -> super::TaskHandle {
        self.launch.handle()
    }

    pub const fn terminal(&self) -> &ProgramTaskTerminal {
        &self.terminal
    }

    /// The actual issuing executable, rather than an equal serialized locator.
    pub fn source(&self) -> &RuntimeTaskSource {
        self.launch.source()
    }

    pub(super) fn retired(launch: ProgramTaskLaunch, terminal: ProgramTaskTerminal) -> Self {
        Self { launch, terminal }
    }

    /// Binds Core's retirement proof to the complete accepted launch and its
    /// driver-owned publication coordinates. Rejection returns the same proof.
    pub fn bind_publication(
        self,
        accepted: &super::TaskSubmission,
        dispatch: super::TaskDispatchIdentity,
        revision: super::TaskPublicationRevision,
    ) -> Result<ProgramTaskPublication, ProgramTaskCompletionRejection> {
        if let Err(reason) = self.validate_publication_binding(accepted, &dispatch) {
            return Err(ProgramTaskCompletionRejection {
                reason,
                completion: self,
            });
        }
        Ok(self.into_publication(dispatch, revision))
    }

    fn validate_publication_binding(
        &self,
        accepted: &super::TaskSubmission,
        dispatch: &super::TaskDispatchIdentity,
    ) -> Result<(), super::TaskCompletionError> {
        if self.handle() != accepted.handle()
            || dispatch.correlation != self.handle().correlation
            || !self.launch.matches_spec(accepted.spec())
        {
            return Err(super::TaskCompletionError::DispatchMismatch {
                task_id: self.handle().correlation.task_id,
            });
        }
        Ok(())
    }

    fn into_publication(
        self,
        dispatch: super::TaskDispatchIdentity,
        revision: super::TaskPublicationRevision,
    ) -> ProgramTaskPublication {
        let kind = super::TaskEventKind::ProgramTerminal(self.terminal);
        ProgramTaskPublication {
            launch: self.launch,
            event: super::TaskEvent::from_dispatch(dispatch, revision, kind),
        }
    }
}

/// A failed publication binding retains its original retirement proof.
#[derive(Debug, thiserror::Error)]
#[error("{reason}")]
pub struct ProgramTaskCompletionRejection {
    #[source]
    pub reason: super::TaskCompletionError,
    pub completion: ProgramTaskCompletion,
}

/// A terminal packet retaining Core's proof and original Source. Its actual
/// result remains in Core while the scheduler retires the matching dispatch.
#[derive(Debug, PartialEq)]
pub struct ProgramTaskPublication {
    pub(super) launch: ProgramTaskLaunch,
    pub(super) event: super::TaskEvent,
}

/// Owns one prepared batch while its caller's report vector is borrowed. Every
/// unsuccessful exit restores the reports in their original order. Preparation
/// exposes only borrowed retirement reports to the driver.
pub struct ProgramTaskPublicationBatch<'a> {
    destination: &'a mut Vec<ProgramTaskCompletion>,
    packets: Vec<ProgramTaskPublication>,
    original_order: std::collections::BTreeMap<super::TaskId, usize>,
}

impl<'a> ProgramTaskPublicationBatch<'a> {
    pub fn bind<'dispatch>(
        completions: &'a mut Vec<ProgramTaskCompletion>,
        mut dispatch: impl FnMut(
            &ProgramTaskCompletion,
        ) -> Result<
            (
                &'dispatch super::TaskSubmission,
                super::TaskDispatchIdentity,
                super::TaskPublicationRevision,
            ),
            super::TaskCompletionError,
        >,
    ) -> Result<Self, super::TaskCompletionError> {
        let mut original_order = std::collections::BTreeMap::new();
        let mut bindings = Vec::with_capacity(completions.len());
        for (ordinal, completion) in completions.iter().enumerate() {
            let task_id = completion.handle().correlation.task_id;
            if original_order.insert(task_id, ordinal).is_some() {
                return Err(super::TaskCompletionError::DuplicateTerminalEvent { task_id });
            }
            let (accepted, identity, revision) = dispatch(completion)?;
            completion.validate_publication_binding(accepted, &identity)?;
            bindings.push((identity, revision));
        }
        let packets = std::mem::take(completions)
            .into_iter()
            .zip(bindings)
            .map(|(completion, (identity, revision))| {
                completion.into_publication(identity, revision)
            })
            .collect();
        Ok(Self {
            destination: completions,
            packets,
            original_order,
        })
    }

    pub fn packets(&self) -> &[ProgramTaskPublication] {
        &self.packets
    }

    pub fn packets_mut(&mut self) -> &mut [ProgramTaskPublication] {
        &mut self.packets
    }

    /// Orders the actual owning records once using the common replay law.
    pub fn sort_for_publication(&mut self) {
        self.packets
            .sort_by(|left, right| super::compare_task_events(&left.event, &right.event));
    }

    /// Commits the transfer. Dropping an uncommitted batch restores its reports.
    pub fn into_events(mut self) -> Vec<super::TaskEvent> {
        std::mem::take(&mut self.packets)
            .into_iter()
            .map(ProgramTaskPublication::into_event)
            .collect()
    }
}

impl Drop for ProgramTaskPublicationBatch<'_> {
    fn drop(&mut self) {
        self.destination.extend(
            self.packets
                .drain(..)
                .map(ProgramTaskPublication::into_completion),
        );
        self.destination.sort_by_key(|completion| {
            self.original_order
                .get(&completion.handle().correlation.task_id)
                .copied()
        });
    }
}

impl ProgramTaskPublication {
    /// Returns the same Core report after a rejected batch. No public operation
    /// on this packet can replace Core's local result with a Host value.
    pub fn into_completion(self) -> ProgramTaskCompletion {
        let super::TaskEventKind::ProgramTerminal(terminal) = self.event.kind else {
            unreachable!("an original-body terminal packet retains its retirement status")
        };
        ProgramTaskCompletion {
            launch: self.launch,
            terminal,
        }
    }

    /// Moves retirement metadata into the runtime's temporal event stream.
    pub fn into_event(self) -> super::TaskEvent {
        self.event
    }
}

/// Program work retains its actual Source. Its inert specialization contains
/// only a locator assertion and cannot be submitted to a live scheduler.
/// Host operations retain the existing closed host request grammar.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub enum TaskRequest<P = crate::value::RuntimePayload, S = RuntimeTaskSource> {
    Host(HostTaskRequest<P>),
    Program(S),
}

impl<P, S> From<HostTaskRequest<P>> for TaskRequest<P, S> {
    fn from(request: HostTaskRequest<P>) -> Self {
        Self::Host(request)
    }
}

impl<P, S> TaskRequest<P, S> {
    pub const fn kind(&self) -> TaskExecutionKind {
        match self {
            Self::Host(_) => TaskExecutionKind::Host,
            Self::Program(_) => TaskExecutionKind::Program,
        }
    }
    /// Borrows the host branch; program execution has no host-call identity.
    pub const fn as_host(&self) -> Option<&HostTaskRequest<P>> {
        match self {
            Self::Host(request) => Some(request),
            Self::Program(_) => None,
        }
    }

    pub fn as_host_mut(&mut self) -> Option<&mut HostTaskRequest<P>> {
        match self {
            Self::Host(request) => Some(request),
            Self::Program(_) => None,
        }
    }

    pub(crate) fn payloads(&self) -> impl Iterator<Item = &P> {
        let host = match self {
            Self::Host(request) => Some(request),
            Self::Program(_) => None,
        };
        host.into_iter().flat_map(HostTaskRequest::payloads)
    }

    pub(super) fn map_admitted<Q, T, E>(
        &self,
        payload: impl FnMut(&P) -> Result<Q, E>,
        source: impl FnOnce(&S) -> Result<T, E>,
    ) -> Result<TaskRequest<Q, T>, E> {
        match self {
            Self::Host(request) => request.map_admitted(payload).map(TaskRequest::Host),
            Self::Program(program) => source(program).map(TaskRequest::Program),
        }
    }

    pub(super) fn into_mapped_admitted<Q, T, E>(
        self,
        payload: impl FnMut(P) -> Result<Q, E>,
        source: impl FnOnce(S) -> Result<T, E>,
    ) -> Result<TaskRequest<Q, T>, E> {
        match self {
            Self::Host(request) => request.into_mapped_admitted(payload).map(TaskRequest::Host),
            Self::Program(program) => source(program).map(TaskRequest::Program),
        }
    }
}

impl<S> TaskRequest<crate::value::RuntimePayload, S> {
    pub fn runtime_values(&self) -> impl Iterator<Item = &crate::value::RuntimeValue> {
        self.payloads().map(crate::value::RuntimePayload::value)
    }
}

/// Only these two Source roles participate in the common task descriptor.
/// An assertion admits metadata; only the live role can retain an execution
/// lease. Restore must resolve the assertion through its enclosing executable.
pub(crate) trait TaskExecutionSource {
    fn expected_key_bytes(&self) -> [u8; 32];
    fn with_owner<T>(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        visit: impl FnOnce(
            &mut RuntimeRetainedValueAdmission,
        ) -> Result<T, AwbcRuntimeValueSnapshotError>,
    ) -> Result<T, AwbcRuntimeValueSnapshotError>;
    fn visit_descriptor(
        &self,
        visitor: &mut impl TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError>;
    fn admit_source(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError>;
}

impl TaskExecutionSource for RuntimeTaskSource {
    fn with_owner<T>(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        visit: impl FnOnce(
            &mut RuntimeRetainedValueAdmission,
        ) -> Result<T, AwbcRuntimeValueSnapshotError>,
    ) -> Result<T, AwbcRuntimeValueSnapshotError> {
        admission.with_program_owner(self.program_owner(), visit)
    }
    fn expected_key_bytes(&self) -> [u8; 32] {
        *self.task_plan().key().as_bytes()
    }

    fn visit_descriptor(
        &self,
        visitor: &mut impl TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        self.assertion().visit_descriptor(visitor, depth)
    }

    fn admit_source(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if admission
            .program_owner()
            .is_none_or(|owner| !owner.same_program(&self.program_owner()))
        {
            return Err(AwbcRuntimeValueSnapshotError::Message {
                message: "program task Source belongs to another executable".into(),
            });
        }
        let limits = crate::plan::RuntimeTaskPlanSealLimits::default();
        let mut meter = admission.task_source_meter(limits)?;
        self.borrow(limits, &mut meter).map_err(|error| {
            admission
                .status()
                .err()
                .unwrap_or_else(|| AwbcRuntimeValueSnapshotError::Message {
                    message: error.to_string(),
                })
        })?;
        Ok(())
    }
}

impl TaskExecutionSource for RuntimeTaskSourceAssertion {
    fn with_owner<T>(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        visit: impl FnOnce(
            &mut RuntimeRetainedValueAdmission,
        ) -> Result<T, AwbcRuntimeValueSnapshotError>,
    ) -> Result<T, AwbcRuntimeValueSnapshotError> {
        // An inert assertion grants no executable lease. Its enclosing saved
        // program supplies the authority before any value can become live.
        visit(admission)
    }
    fn expected_key_bytes(&self) -> [u8; 32] {
        self.expected_key_bytes()
    }

    fn visit_descriptor(
        &self,
        visitor: &mut impl TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        self.visit_descriptor(visitor, depth)
    }

    fn admit_source(
        &self,
        _admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        // The common descriptor meters the complete inert locator. This grants
        // no live Source; original-program lookup is required before restore.
        Ok(())
    }
}

impl<P, S: TaskExecutionSource> TaskRequest<P, S> {
    pub(super) fn matches_plan(&self, plan: TaskPlanSemanticDigest) -> bool {
        match self {
            Self::Host(_) => true,
            Self::Program(source) => source.expected_key_bytes() == *plan.as_bytes(),
        }
    }

    pub(super) fn admit_source(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        match self {
            Self::Host(_) => Ok(()),
            Self::Program(source) => source.admit_source(admission),
        }
    }

    pub(super) fn visit_descriptor(
        &self,
        visitor: &mut impl TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        match self {
            Self::Host(request) => request.visit_descriptor(visitor, depth),
            Self::Program(source) => {
                visitor.node(depth)?;
                // This is a task execution discriminator, outside the closed
                // HostTaskRequestKind tag space.
                visitor.fixed(b"program", depth + 1)?;
                source.visit_descriptor(visitor, depth + 1)
            }
        }
    }
}
