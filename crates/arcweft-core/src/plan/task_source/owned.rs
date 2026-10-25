//! An owning lease for an original program's actual completed Task slot.

use super::program::{RuntimeTaskExecutionRow, RuntimeTaskSourceBorrowError};
use super::{RuntimeTaskSourceProgram, RuntimeTaskSourceRef};
use crate::plan::{
    RuntimePlan, RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanRef, RuntimeTaskPlanSealLimits,
};
use crate::task::RuntimeProgramOwner;
use crate::task::semantic::TaskSemanticMeter;

/// Retains the genuine source namespace. Neither key assertions nor a metadata
/// launch coordinate can construct this handle. Cloning retains the same Arc.
#[derive(Clone)]
pub struct RuntimeTaskSource {
    program: RuntimeTaskSourceProgram,
    coordinate: RuntimeTaskPlanBuildCoordinate,
    execution: Option<RuntimeTaskExecutionRow>,
    // Attested with this immutable original Source; never supplied by an inert locator.
    handle_type: crate::task::TaskHandleType,
    execution_kind: crate::task::TaskExecutionKind,
}

/// Inert locator for an already completed Source and its exact execution row.
/// Decoding this value grants no Source lease or producer authority.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTaskSourceAssertion {
    pub(in crate::plan::task_source) expected: crate::task::ExpectedTaskPlanKey,
    pub(in crate::plan::task_source) execution: RuntimeTaskSourceExecutionAssertion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(in crate::plan::task_source) enum RuntimeTaskSourceExecutionAssertion {
    Native,
    Awbc(RuntimeTaskExecutionRow),
}

impl serde::Serialize for RuntimeTaskSource {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.assertion(), serializer)
    }
}

impl RuntimeTaskSourceAssertion {
    pub(crate) const fn expected_key_bytes(&self) -> [u8; 32] {
        *self.expected.bytes()
    }

    pub(crate) fn visit_descriptor(
        &self,
        visitor: &mut impl crate::task::descriptor::TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), crate::entry::RuntimeSchemaError> {
        visitor.node(depth)?;
        visitor.fixed(self.expected.bytes(), depth + 1)?;
        let (kind, row) = match self.execution {
            RuntimeTaskSourceExecutionAssertion::Native => (0, None),
            RuntimeTaskSourceExecutionAssertion::Awbc(RuntimeTaskExecutionRow::HostCall(row)) => {
                (1, Some(row.0))
            }
            RuntimeTaskSourceExecutionAssertion::Awbc(RuntimeTaskExecutionRow::Selected(row)) => {
                (2, Some(row.0))
            }
            RuntimeTaskSourceExecutionAssertion::Awbc(RuntimeTaskExecutionRow::Template(row)) => {
                (3, Some(row.0))
            }
            RuntimeTaskSourceExecutionAssertion::Awbc(RuntimeTaskExecutionRow::Program(row)) => {
                (4, Some(row.0))
            }
        };
        visitor.fixed(&[kind], depth + 1)?;
        if let Some(row) = row {
            visitor.fixed(&row.to_le_bytes(), depth + 1)?;
        }
        Ok(())
    }
}

impl RuntimeTaskSource {
    pub(crate) fn assertion(&self) -> RuntimeTaskSourceAssertion {
        RuntimeTaskSourceAssertion {
            expected: self.task_plan().key().into(),
            execution: match self.execution {
                Some(row) => RuntimeTaskSourceExecutionAssertion::Awbc(row),
                None => RuntimeTaskSourceExecutionAssertion::Native,
            },
        }
    }
    pub(crate) fn from_borrowed(
        source: &RuntimeTaskSourceRef<'_>,
        program: &RuntimeTaskSourceProgram,
        owner: &RuntimeProgramOwner,
    ) -> Option<Self> {
        if !program.owns_source_for(source, owner) {
            return None;
        }
        let coordinate = source.owner.resolve(source.task.index().0)?;
        use crate::plan::body_semantic::flow::RuntimeBodyTaskSource;
        use crate::task::TaskExecutionKind;
        let (execution_kind, handle_type) = match source.request_endpoint().task_source()? {
            RuntimeBodyTaskSource::Producer(template) => {
                (TaskExecutionKind::Host, template.definition.handle_type)
            }
            RuntimeBodyTaskSource::ProgramProducer(producer) => {
                (TaskExecutionKind::Program, producer.handle_type)
            }
            RuntimeBodyTaskSource::Start(target) => (
                TaskExecutionKind::Host,
                crate::task::TaskHandleType::Need {
                    payload: crate::task::RuntimeTypeSemanticDigest::from_bytes(
                        *target.plan().payload_type().as_bytes(),
                    ),
                },
            ),
            RuntimeBodyTaskSource::Host(target) => (
                TaskExecutionKind::Host,
                crate::task::TaskHandleType::Need {
                    payload: crate::task::RuntimeTypeSemanticDigest::from_bytes(
                        *program
                            .original()
                            .type_table()
                            .get(target.result)?
                            .semantic_identity()
                            .as_bytes(),
                    ),
                },
            ),
        };
        Some(Self {
            program: program.clone(),
            coordinate,
            handle_type,
            execution_kind,
            execution: source
                .execution
                .as_ref()
                .map(super::program::RuntimeTaskExecutionRef::row),
        })
    }

    pub(crate) const fn handle_type(&self) -> crate::task::TaskHandleType {
        self.handle_type
    }

    pub(crate) const fn execution_kind(&self) -> crate::task::TaskExecutionKind {
        self.execution_kind
    }

    pub fn task_plan(&self) -> RuntimeTaskPlanRef<'_> {
        self.program
            .original()
            .task_plan(&self.coordinate)
            .expect("the original immutable program retains its issued slot")
    }

    pub(crate) fn borrow(
        &self,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<RuntimeTaskSourceRef<'_>, RuntimeTaskSourceBorrowError> {
        self.program
            .reborrow_source(&self.coordinate, self.execution, limits, meter)
    }

    pub(crate) fn belongs_to(&self, program: &RuntimePlan) -> bool {
        std::ptr::eq(self.program.original(), program)
    }

    pub(crate) fn program_owner(&self) -> RuntimeProgramOwner {
        self.program.program_owner()
    }

    pub(crate) fn program(&self) -> &RuntimeTaskSourceProgram {
        &self.program
    }
}

impl PartialEq for RuntimeTaskSource {
    fn eq(&self, other: &Self) -> bool {
        self.program == other.program
            && self.coordinate == other.coordinate
            && self.execution == other.execution
            && self.handle_type == other.handle_type
            && self.execution_kind == other.execution_kind
    }
}
impl Eq for RuntimeTaskSource {}

impl std::fmt::Debug for RuntimeTaskSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeTaskSource")
            .field("key", &self.task_plan().key())
            .finish_non_exhaustive()
    }
}
