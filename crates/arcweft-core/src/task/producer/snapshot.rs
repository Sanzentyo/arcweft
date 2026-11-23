//! The persistent Need journal contains inert values and Source assertions.

use super::{
    NeedProducerInvocationFrontier, NeedProducerInvocationToken, NeedProducerLaunchFrontier,
    NeedProducerLaunchOriginRestore, NeedProducerLaunchRestore, NeedProducerRegistryImage,
    NeedProducerRuntimeArgument, ProgramTaskRetirementPhase, RuntimeNeedProducerState,
    RuntimeTaskProducerSourceError, TaskCorrelation, TaskPublicationCursor, TaskSnapshotSource,
    TaskSourceRestore, TaskSpecSaveSnapshot, TaskSubmissionSaveSnapshot,
};
use crate::plan::task_source::RuntimeTaskSourceProgram;
use crate::plan::{RuntimeTaskPlanSealLimits, RuntimeTaskSource, RuntimeTaskSourceAssertion};
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

/// The persistent journal of accepted work, launch state and allocator frontiers.
/// It can only be bound using the enclosing executable's original Source.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeNeedProducerRegistrySaveSnapshot {
    pub task_admissions: Vec<TaskSubmissionSaveSnapshot>,
    pub launches: Vec<RuntimeNeedProducerLaunchSaveSnapshot>,
    pub invocations: Vec<RuntimeNeedProducerInvocationSaveSnapshot>,
    pub invocation_frontiers: Vec<NeedProducerInvocationFrontier>,
    pub launch_frontiers: Vec<NeedProducerLaunchFrontier>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeNeedProducerLaunchSaveSnapshot {
    pub origin: RuntimeNeedProducerLaunchOriginSaveSnapshot,
    pub correlation: TaskCorrelation,
    pub state: RuntimeNeedProducerState<AwbcRuntimeValueSnapshot>,
    pub publication: Option<TaskPublicationCursor>,
    pub task_submitted: bool,
    pub task_terminal: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeNeedProducerLaunchOriginSaveSnapshot {
    Need {
        invocation: NeedProducerInvocationToken,
        source: RuntimeTaskSourceAssertion,
        arguments: Vec<NeedProducerRuntimeArgument<AwbcRuntimeValueSnapshot>>,
        task_spec: TaskSpecSaveSnapshot,
    },
    Program {
        retirement: ProgramTaskRetirementPhase,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeNeedProducerInvocationSaveSnapshot {
    pub invocation: NeedProducerInvocationToken,
    pub correlation: TaskCorrelation,
}

impl RuntimeNeedProducerRegistrySaveSnapshot {
    pub(crate) fn from_admitted(registry: &NeedProducerRegistryImage) -> Self {
        let assert_source = |source: RuntimeTaskSource| Ok::<_, Infallible>(source.assertion());
        Self {
            task_admissions: registry
                .task_admissions
                .iter()
                .cloned()
                .map(|row| {
                    row.map_source_admitted(assert_source)
                        .unwrap_or_else(|never| match never {})
                })
                .collect(),
            launches: registry
                .launches
                .iter()
                .map(|launch| RuntimeNeedProducerLaunchSaveSnapshot {
                    origin: match &launch.origin {
                        NeedProducerLaunchOriginRestore::Need { invocation, start } => {
                            RuntimeNeedProducerLaunchOriginSaveSnapshot::Need {
                                invocation: *invocation,
                                source: start.source().assertion(),
                                arguments: start.arguments.clone(),
                                task_spec: start
                                    .task_spec()
                                    .clone()
                                    .map_source_admitted(assert_source)
                                    .unwrap_or_else(|never| match never {}),
                            }
                        }
                        NeedProducerLaunchOriginRestore::Program { retirement } => {
                            RuntimeNeedProducerLaunchOriginSaveSnapshot::Program {
                                retirement: *retirement,
                            }
                        }
                    },
                    correlation: launch.correlation,
                    state: launch.state.clone(),
                    publication: launch.publication,
                    task_submitted: launch.task_submitted,
                    task_terminal: launch.task_terminal,
                })
                .collect(),
            invocations: registry
                .invocations
                .iter()
                .map(
                    |(invocation, correlation)| RuntimeNeedProducerInvocationSaveSnapshot {
                        invocation: *invocation,
                        correlation: *correlation,
                    },
                )
                .collect(),
            invocation_frontiers: registry.invocation_frontiers.clone(),
            launch_frontiers: registry.launch_frontiers.clone(),
        }
    }

    /// Binds wire assertions to the enclosing executable and admits the whole
    /// inert journal before returning it for live reconstruction.
    pub(crate) fn bind(
        self,
        source_program: Option<&RuntimeTaskSourceProgram>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<NeedProducerRegistryImage, AwbcRuntimeValueSnapshotError> {
        let bound = self.bind_assertions(source_program, admission)?;
        bound.admit_retained(admission)?;
        Ok(bound)
    }

    /// Rebinds source assertions before the enclosing session admits the
    /// journal together with its waiters, publications, and executing fibers.
    pub(crate) fn bind_assertions(
        self,
        source_program: Option<&RuntimeTaskSourceProgram>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<NeedProducerRegistryImage, AwbcRuntimeValueSnapshotError> {
        // Bound every row inventory before allocating the bound journal or
        // starting any Source lookup. The bound pass inspects each row's data.
        for count in [
            self.task_admissions.len(),
            self.launches.len(),
            self.invocations.len(),
            self.invocation_frontiers.len(),
            self.launch_frontiers.len(),
        ] {
            admission.validate_metadata_collection(count)?;
        }
        for launch in &self.launches {
            if let RuntimeNeedProducerLaunchOriginSaveSnapshot::Need { arguments, .. } =
                &launch.origin
            {
                admission.validate_metadata_collection(arguments.len())?;
            }
        }
        let owner = admission.program_owner().cloned();
        if let Some(source) = source_program
            && !owner
                .as_ref()
                .is_some_and(|owner| owner.same_program(&source.program_owner()))
        {
            return Err(RuntimeTaskProducerSourceError::OriginalProgram.into());
        }
        for row in &self.task_admissions {
            row.admit_retained(admission)?;
        }
        let task_admissions =
            self.task_admissions
                .into_iter()
                .map(|row| {
                    row.map_source_admitted(|assertion| {
                        let owner = owner.as_ref().ok_or_else(|| {
                            AwbcRuntimeValueSnapshotError::Message {
                                message: "program task has no enclosing executable".into(),
                            }
                        })?;
                        assertion.restore_source(
                            owner,
                            source_program.map(|program| TaskSourceRestore { program, admission }),
                        )
                    })
                })
                .collect::<Result<_, _>>()?;
        let mut launches = Vec::with_capacity(self.launches.len());
        for launch in self.launches {
            let origin = match launch.origin {
                RuntimeNeedProducerLaunchOriginSaveSnapshot::Program { retirement } => {
                    NeedProducerLaunchOriginRestore::Program { retirement }
                }
                RuntimeNeedProducerLaunchOriginSaveSnapshot::Need {
                    invocation,
                    source,
                    arguments,
                    task_spec,
                } => {
                    admission.validate_metadata(&source)?;
                    let program =
                        source_program.ok_or_else(|| AwbcRuntimeValueSnapshotError::Message {
                            message: "selected saved launch has no admitted original Source".into(),
                        })?;
                    let limits = RuntimeTaskPlanSealLimits::default();
                    let mut meter = admission.task_source_meter(limits)?;
                    let source = program
                        .borrow_assertion(&source, limits, &mut meter)
                        .map_err(|error| {
                            admission.status().err().unwrap_or_else(|| {
                                AwbcRuntimeValueSnapshotError::Message {
                                    message: error.to_string(),
                                }
                            })
                        })?;
                    let prepared = source.prepare_selected_producer(program, admission)?;
                    if arguments.len() != prepared.arguments().len() {
                        return Err(RuntimeTaskProducerSourceError::ArgumentCount {
                            actual: arguments.len(),
                            expected: prepared.arguments().len(),
                        }
                        .into());
                    }
                    let task_spec = task_spec.map_source_admitted(|_| {
                        Err::<RuntimeTaskSource, _>(AwbcRuntimeValueSnapshotError::Message {
                            message: "a selected host producer cannot restore program execution"
                                .into(),
                        })
                    })?;
                    let start = prepared.restore_start(arguments, task_spec)?;
                    NeedProducerLaunchOriginRestore::Need { invocation, start }
                }
            };
            launches.push(NeedProducerLaunchRestore {
                origin,
                correlation: launch.correlation,
                state: launch.state,
                publication: launch.publication,
                task_submitted: launch.task_submitted,
                task_terminal: launch.task_terminal,
            });
        }
        Ok(NeedProducerRegistryImage {
            task_admissions,
            launches,
            invocations: self
                .invocations
                .into_iter()
                .map(|row| (row.invocation, row.correlation))
                .collect(),
            invocation_frontiers: self.invocation_frontiers,
            launch_frontiers: self.launch_frontiers,
        })
    }

    pub(crate) fn visit_saved_value_roots<E>(
        &self,
        visit: &mut impl FnMut(&AwbcRuntimeValueSnapshot) -> Result<(), E>,
    ) -> Result<(), E> {
        for row in &self.task_admissions {
            for value in row.request_values() {
                visit(value)?;
            }
        }
        for launch in &self.launches {
            if let RuntimeNeedProducerLaunchOriginSaveSnapshot::Need {
                arguments,
                task_spec,
                ..
            } = &launch.origin
            {
                for argument in arguments {
                    visit(&argument.value)?;
                }
                for value in task_spec.request.payloads() {
                    visit(value)?;
                }
            }
            if let RuntimeNeedProducerState::Ready(value) = &launch.state {
                visit(value)?;
            }
        }
        Ok(())
    }
}
