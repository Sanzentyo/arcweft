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

    pub(crate) fn source(&self) -> &RuntimeTaskSource {
        let TaskRequest::Program(source) = &self.receipt.spec().request else {
            unreachable!("the retained launch has no Host request payload")
        };
        source
    }

    pub(crate) fn matches_spec<P>(&self, spec: &super::TaskSpec<P>) -> bool {
        self.receipt.spec().same_program_contract(spec)
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
            registry.ensure_program_task_with_publication(spec.clone(), |_| {
                panic!("a Host packet must never publish a Program launch")
            }),
            Err(super::super::TaskEnsureError::ProgramSourceMismatch)
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
    handle: super::TaskHandle,
    source: RuntimeTaskSource,
    terminal: ProgramTaskTerminal,
}

/// Program task bodies currently have Unit results. Dialogue result custody
/// remains with the activation; it is not copied into this task report.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub enum ProgramTaskTerminal {
    Completed,
    Failed(super::RuntimeTaskFailure),
    Cancelled,
}

impl ProgramTaskCompletion {
    pub const fn handle(&self) -> super::TaskHandle {
        self.handle
    }

    pub const fn terminal(&self) -> &ProgramTaskTerminal {
        &self.terminal
    }

    /// The actual issuing executable, rather than an equal serialized locator.
    pub const fn source(&self) -> &RuntimeTaskSource {
        &self.source
    }

    pub(crate) fn completed(launch: &ProgramTaskLaunch, cancelled: bool) -> Self {
        Self {
            handle: launch.handle(),
            source: launch.source().clone(),
            terminal: if cancelled {
                ProgramTaskTerminal::Cancelled
            } else {
                ProgramTaskTerminal::Completed
            },
        }
    }

    pub(crate) fn failed(launch: &ProgramTaskLaunch, message: &str) -> Self {
        Self {
            handle: launch.handle(),
            source: launch.source().clone(),
            terminal: ProgramTaskTerminal::Failed(super::RuntimeTaskFailure {
                kind: super::RuntimeTaskFailureKind::WorkerFailure,
                diagnostic: super::BoundedRuntimeDiagnostic::from_borrowed_message(message),
            }),
        }
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
