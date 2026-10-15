//! Original executable storage admission before live/inert value mapping.

use super::{AwbcRuntimeValueSnapshotError, RuntimeProgramOwner};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::{RuntimeProgramTypes, RuntimeProgramValueType};
use crate::value::{
    RuntimeAgentKind, RuntimeIteratorKind, RuntimeValueView, RuntimeVariantIdentity,
};

pub(super) struct SnapshotStorageAdmission<'a> {
    owner: Option<&'a RuntimeProgramOwner>,
    resources: &'a ValueResourceAdmission,
}

impl<'a> SnapshotStorageAdmission<'a> {
    pub(super) fn new(
        owner: Option<&'a RuntimeProgramOwner>,
        resources: &'a ValueResourceAdmission,
    ) -> Self {
        Self { owner, resources }
    }

    pub(super) fn validate(
        &self,
        value: RuntimeValueView<'_>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if let Some(error) = self.resources.work().failure() {
            return Err(error.into());
        }
        if !self.resources.has_program_storage_obligations() {
            return Ok(());
        }
        crate::entry::schema::value_encoding::visit_admitted_storage(
            value,
            &self.resources.work(),
            |node| self.validate_node(node),
        )
    }

    fn require_owner(
        &self,
        role: &str,
    ) -> Result<&RuntimeProgramOwner, AwbcRuntimeValueSnapshotError> {
        self.owner
            .ok_or_else(|| AwbcRuntimeValueSnapshotError::Message {
                message: format!("{role} values require a program-bound snapshot"),
            })
    }

    fn validate_node(
        &self,
        value: RuntimeValueView<'_>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let expected = match value {
            RuntimeValueView::NominalRecord(record) => Some(
                self.require_owner("nominal")?
                    .types()
                    .value_type(record.semantic_identity())?,
            ),
            RuntimeValueView::Variant(variant) => match variant.owner() {
                RuntimeVariantIdentity::Nominal {
                    semantic_identity, ..
                } => Some(
                    self.require_owner("nominal")?
                        .types()
                        .value_type(*semantic_identity)?,
                ),
                RuntimeVariantIdentity::Builtin(_) => None,
            },
            RuntimeValueView::Callable(callable) => Some(self.callable_type(callable)?),
            RuntimeValueView::Need(need) => {
                need.validate_admitted_storage(self.owner, self.resources)?;
                None
            }
            RuntimeValueView::Iterator(iterator) => {
                if let RuntimeIteratorKind::Witness(witness) = iterator.kind() {
                    let owner = self.require_owner("iterator witness")?;
                    if witness
                        .owner()
                        .is_some_and(|actual| !actual.same_program(owner))
                    {
                        return Err(AwbcRuntimeValueSnapshotError::Message {
                            message: "iterator witness belongs to another program".into(),
                        });
                    }
                    let (state, _) = owner
                        .types()
                        .iterator_witness_types(witness.next_method())
                        .ok_or_else(|| AwbcRuntimeValueSnapshotError::Message {
                            message: "iterator witness has no original next-method contract".into(),
                        })?;
                    owner.types().value_type(state)?.validate_snapshot_storage(
                        witness.state(),
                        &self.resources.work(),
                        self.resources.origin(),
                    )?;
                }
                None
            }
            RuntimeValueView::Agent(agent) => {
                if let RuntimeAgentKind::DataShape(shape) = agent.kind() {
                    let owner = self.require_owner("DataShape")?;
                    let matches = match owner.types() {
                        RuntimeProgramTypes::Plan(program) => {
                            shape.matches_plan(program, shape.shape_type())
                        }
                        RuntimeProgramTypes::Awbc(program) => {
                            shape.matches_awbc(program, shape.shape_type())
                        }
                    };
                    if !matches {
                        return Err(AwbcRuntimeValueSnapshotError::Message {
                            message:
                                "DataShape witness disagrees with the selected original program"
                                    .into(),
                        });
                    }
                }
                None
            }
            _ => None,
        };
        if let Some(expected) = expected {
            expected.validate_snapshot_storage(
                value,
                &self.resources.work(),
                self.resources.origin(),
            )?;
        }
        Ok(())
    }

    /// Resolves the function row from the original state while the complete
    /// retained graph still borrows its input storage.
    fn callable_type(
        &self,
        callable: crate::value::RuntimeCallableView<'_>,
    ) -> Result<RuntimeProgramValueType<'_>, AwbcRuntimeValueSnapshotError> {
        let owner = self.require_owner("callable")?;
        if callable
            .owner()
            .is_some_and(|actual| !actual.same_program(owner))
        {
            return Err(AwbcRuntimeValueSnapshotError::Message {
                message: "callable rollback image belongs to another program".into(),
            });
        }
        let missing = || AwbcRuntimeValueSnapshotError::Message {
            message: format!(
                "callable state {} is absent from the selected program",
                callable.state()
            ),
        };
        Ok(match owner.types() {
            RuntimeProgramTypes::Plan(program) => RuntimeProgramValueType::Plan {
                program,
                ty: program
                    .callable_states()
                    .get(callable.state())
                    .ok_or_else(missing)?
                    .function_type,
            },
            RuntimeProgramTypes::Awbc(program) => RuntimeProgramValueType::Awbc {
                program,
                ty: program
                    .callable_states
                    .get(callable.state().index())
                    .ok_or_else(missing)?
                    .function_type,
            },
        })
    }
}
