//! Retained, replay-safe values on one reached Flow invocation.

use super::{RuntimeRegistryStateUpdate, RuntimeRegistryValue};
use crate::awbc::schema::{AwbcEffectPayload, AwbcEffectPlanId};
use crate::runtime_id::{RuntimeFiberInstanceId, RuntimeFrameInstanceId};
use crate::task::RuntimeProgramOwner;
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission};
use arcweft_id::{LifetimeRegistryKey, LifetimeScopeKind};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) enum RuntimeRegistryWriteSource {
    Native,
    Awbc {
        effect: AwbcEffectPlanId,
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
    },
}

impl RuntimeRegistryWriteSource {
    fn admit(
        self,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        match self {
            Self::Native => admission.validate_metadata(&0_u8),
            Self::Awbc {
                effect,
                fiber,
                frame,
            } => {
                admission.validate_metadata(&(1_u8, effect.0, fiber.get().get(), frame.get().get()))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct RegistryEntry {
    source: RuntimeRegistryWriteSource,
    row: Arc<RuntimeRegistryValue>,
}

/// The enclosing activation grants access. This inert data can survive a
/// private checkpoint or public AWBC save without becoming an active lease.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub(crate) struct RuntimeFlowRegistry(Vec<RegistryEntry>);

impl RuntimeFlowRegistry {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn prepare_write(
        &self,
        key: &LifetimeRegistryKey,
        source: RuntimeRegistryWriteSource,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<Result<usize, usize>, AwbcRuntimeValueSnapshotError> {
        if key.scope() != &LifetimeScopeKind::Flow {
            return Err(invalid_registry(
                "registry address does not select this Flow",
            ));
        }
        let position = self.0.binary_search_by(|entry| entry.row.key.cmp(key));
        admission.validate_metadata_collection(self.0.len() + usize::from(position.is_err()))?;
        source.admit(admission)?;
        Ok(position)
    }

    pub(crate) fn reserve_write(
        &mut self,
        position: Result<usize, usize>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if position.is_err() {
            self.0
                .try_reserve(1)
                .map_err(|_| invalid_registry("Flow registry allocation failed"))?;
        }
        Ok(())
    }

    pub(crate) fn commit(
        &mut self,
        position: Result<usize, usize>,
        source: RuntimeRegistryWriteSource,
        update: RuntimeRegistryStateUpdate,
    ) {
        let entry = RegistryEntry {
            source,
            row: Arc::new(update.row),
        };
        match position {
            Ok(index) => self.0[index] = entry,
            Err(index) => self.0.insert(index, entry),
        }
    }

    pub(crate) fn admit_retained(
        &self,
        awbc_frame: Option<(RuntimeFiberInstanceId, RuntimeFrameInstanceId)>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata_collection(self.0.len())?;
        let mut previous = None;
        for entry in &self.0 {
            let key = &entry.row.key;
            if key.scope() != &LifetimeScopeKind::Flow || previous.is_some_and(|prior| prior >= key)
            {
                return Err(invalid_registry(
                    "invalid Flow registry key order or selector",
                ));
            }
            entry.source.admit(admission)?;
            match (admission.program_owner(), entry.source, awbc_frame) {
                (Some(RuntimeProgramOwner::Plan(_)), RuntimeRegistryWriteSource::Native, None) => {}
                (
                    Some(RuntimeProgramOwner::Awbc(program)),
                    RuntimeRegistryWriteSource::Awbc {
                        effect,
                        fiber,
                        frame,
                    },
                    Some(expected_frame),
                ) if (fiber, frame) == expected_frame => {
                    let payload = program
                        .effect_plans
                        .get(effect.index())
                        .map(|plan| &plan.payload);
                    if !matches!(payload, Some(AwbcEffectPayload::RegistryWrite { key: declared, value_type })
                        if declared == key && program.runtime_types.get(value_type.index())
                            .is_some_and(|ty| ty.semantic_identity() == entry.row.expected))
                    {
                        return Err(invalid_registry(
                            "saved registry row differs from its original write",
                        ));
                    }
                }
                _ => {
                    return Err(invalid_registry(
                        "registry row requires its original executable and Flow invocation",
                    ));
                }
            }
            entry.row.admit_retained(admission)?;
            previous = Some(key);
        }
        Ok(())
    }
}

fn invalid_registry(message: &str) -> AwbcRuntimeValueSnapshotError {
    AwbcRuntimeValueSnapshotError::Message {
        message: message.to_owned(),
    }
}
