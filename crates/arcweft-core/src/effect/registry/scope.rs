//! Retained, replay-safe values on one reached execution scope.

use super::{
    PreparedRegistryValue, RuntimeRegistryReadKind, RuntimeRegistryStateUpdate,
    RuntimeRegistryValue,
};
use crate::awbc::schema::{AwbcEffectPayload, AwbcEffectPlanId};
use crate::pattern::RuntimeSemanticTypeId;
use crate::runtime_id::{DialogueActivationId, RuntimeFiberInstanceId, RuntimeFrameInstanceId};
use crate::task::RuntimeProgramOwner;
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeValue};
use arcweft_id::{LifetimeRegistryKey, LifetimeScopeKind};
use std::sync::Arc;

/// The actual reached owner against which an inert registry row is admitted.
/// Execution obtains this from a live frame or an activation transaction.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum RuntimeRegistryOwner {
    NativeFlow,
    AwbcFlow {
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
    },
    Dialogue(DialogueActivationId),
}

impl RuntimeRegistryOwner {
    const fn scope(self) -> LifetimeScopeKind {
        match self {
            Self::NativeFlow | Self::AwbcFlow { .. } => LifetimeScopeKind::Flow,
            Self::Dialogue(_) => LifetimeScopeKind::Line,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) enum RuntimeRegistryWriteSource {
    Native,
    Awbc {
        effect: AwbcEffectPlanId,
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
    },
    NativeLine {
        activation: DialogueActivationId,
    },
    AwbcLine {
        effect: AwbcEffectPlanId,
        activation: DialogueActivationId,
    },
}

impl RuntimeRegistryWriteSource {
    fn admit(
        self,
        owner: RuntimeRegistryOwner,
        key: &LifetimeRegistryKey,
        value_type: RuntimeSemanticTypeId,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        use RuntimeRegistryOwner::{AwbcFlow, Dialogue, NativeFlow};

        let original = match (admission.program_owner(), self, owner) {
            (Some(RuntimeProgramOwner::Plan(_)), Self::Native, NativeFlow) => None,
            (
                Some(RuntimeProgramOwner::Plan(_)),
                Self::NativeLine { activation },
                Dialogue(expected),
            ) if activation == expected => None,
            (
                Some(RuntimeProgramOwner::Awbc(program)),
                Self::Awbc {
                    effect,
                    fiber,
                    frame,
                },
                AwbcFlow {
                    fiber: original_fiber,
                    frame: original_frame,
                },
            ) if (fiber, frame) == (original_fiber, original_frame) => Some((program, effect)),
            (
                Some(RuntimeProgramOwner::Awbc(program)),
                Self::AwbcLine { effect, activation },
                Dialogue(expected),
            ) if activation == expected => Some((program, effect)),
            _ => {
                return Err(invalid_registry(
                    "registry row requires its original executable and activation",
                ));
            }
        };
        if let Some((program, effect)) = original {
            let payload = program
                .effect_plans
                .get(effect.index())
                .map(|row| &row.payload);
            if !matches!(payload, Some(AwbcEffectPayload::RegistryWrite { key: original, value_type: original_type })
                if original == key && program.runtime_types.get(original_type.index())
                    .is_some_and(|ty| ty.semantic_identity() == value_type))
            {
                return Err(invalid_registry(
                    "registry row differs from its original write",
                ));
            }
        }
        match self {
            Self::Native => admission.validate_metadata(&0_u8),
            Self::Awbc {
                effect,
                fiber,
                frame,
            } => {
                admission.validate_metadata(&(1_u8, effect.0, fiber.get().get(), frame.get().get()))
            }
            Self::NativeLine { activation } => admission.validate_metadata(&(2_u8, activation)),
            Self::AwbcLine { effect, activation } => {
                admission.validate_metadata(&(3_u8, effect.0, activation))
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
pub(crate) struct RuntimeScopeRegistry(Vec<RegistryEntry>);

impl RuntimeScopeRegistry {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }
    /// Copies a reached row after admitting the full future value under the
    /// reader's result type and original invocation. Optional absence still
    /// requires the reached scope and admits a canonical Option::None.
    pub(crate) fn read(
        &self,
        owner: RuntimeRegistryOwner,
        key: &LifetimeRegistryKey,
        kind: RuntimeRegistryReadKind,
        expected: RuntimeSemanticTypeId,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        let row = self
            .admit_reached_entry(owner, key, admission)?
            .map(|entry| entry.row.as_ref());
        match (kind, row) {
            (RuntimeRegistryReadKind::Required, None) => {
                Err(invalid_registry("required registry value is absent"))
            }
            (RuntimeRegistryReadKind::Required, Some(row)) => {
                admission.validate_typed_snapshot_in_invocation(&row.value, expected, binding)?;
                row.copy_admitted(admission)
            }
            (RuntimeRegistryReadKind::Optional, row) => {
                admission.validate_owned_option_preview(
                    row.map(|row| (&row.value).into()),
                    expected,
                    binding,
                )?;
                match row {
                    Some(row) => row.copy_admitted(admission).map(RuntimeValue::option_some),
                    None => Ok(RuntimeValue::option_none()),
                }
            }
        }
    }

    fn admit_reached_entry(
        &self,
        owner: RuntimeRegistryOwner,
        key: &LifetimeRegistryKey,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<Option<&RegistryEntry>, AwbcRuntimeValueSnapshotError> {
        admission.status()?;
        if key.scope() != &owner.scope() {
            return Err(invalid_registry(
                "registry address does not select this activation",
            ));
        }
        let Ok(index) = self.0.binary_search_by(|entry| entry.row.key.cmp(key)) else {
            return Ok(None);
        };
        let entry = &self.0[index];
        entry
            .source
            .admit(owner, key, entry.row.expected, admission)?;
        Ok(Some(entry))
    }

    /// Preflights removal of the exact admitted row. The expected type is
    /// the item type even when absence is allowed.
    pub(crate) fn prepare_drop(
        &self,
        owner: RuntimeRegistryOwner,
        key: &LifetimeRegistryKey,
        kind: RuntimeRegistryReadKind,
        expected: RuntimeSemanticTypeId,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<RuntimePreparedRegistryDrop, AwbcRuntimeValueSnapshotError> {
        let row = match self.admit_reached_entry(owner, key, admission)? {
            Some(entry) => {
                admission.validate_typed_snapshot_in_invocation(
                    &entry.row.value,
                    expected,
                    binding,
                )?;
                Some(Arc::clone(&entry.row))
            }
            None if kind == RuntimeRegistryReadKind::Optional => None,
            None => return Err(invalid_registry("required registry value is absent")),
        };
        admission.validate_metadata(&(key, expected, binding))?;
        Ok(RuntimePreparedRegistryDrop {
            key: key.clone(),
            row,
        })
    }

    pub(crate) fn prepare_write<'key, 'budget>(
        &self,
        owner: RuntimeRegistryOwner,
        key: &'key LifetimeRegistryKey,
        value_type: RuntimeSemanticTypeId,
        binding: Option<&'key crate::program_types::RuntimeFunctionEffectInstantiation>,
        source: RuntimeRegistryWriteSource,
        admission: &'budget mut RuntimeRetainedValueAdmission,
    ) -> Result<RuntimeRegistryWritePreparation<'key, 'budget>, AwbcRuntimeValueSnapshotError> {
        if key.scope() != &owner.scope() {
            return Err(invalid_registry(
                "registry address does not select this activation",
            ));
        }
        let position = self.0.binary_search_by(|entry| entry.row.key.cmp(key));
        admission.validate_metadata_collection(self.0.len() + usize::from(position.is_err()))?;
        source.admit(owner, key, value_type, admission)?;
        let value = RuntimeRegistryStateUpdate::prepare(key, value_type, binding, admission)?;
        Ok(RuntimeRegistryWritePreparation {
            len: self.0.len(),
            previous: position.ok().map(|index| Arc::clone(&self.0[index].row)),
            source,
            value,
        })
    }

    fn revalidate_row(
        &self,
        key: &LifetimeRegistryKey,
        previous: Option<&Arc<RuntimeRegistryValue>>,
    ) -> Result<Result<usize, usize>, AwbcRuntimeValueSnapshotError> {
        let position = self.0.binary_search_by(|entry| entry.row.key.cmp(key));
        match (position, previous) {
            (Ok(index), Some(row)) if Arc::ptr_eq(&self.0[index].row, row) => Ok(position),
            (Err(_), None) => Ok(position),
            _ => Err(invalid_registry("registry row changed before commit")),
        }
    }

    fn reserve_write(
        &mut self,
        position: Result<usize, usize>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if position.is_err() {
            self.0
                .try_reserve(1)
                .map_err(|_| invalid_registry("registry allocation failed"))?;
        }
        Ok(())
    }

    fn commit_write(
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
        owner: RuntimeRegistryOwner,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata_collection(self.0.len())?;
        let scope = owner.scope();
        let mut previous = None;
        for entry in &self.0 {
            let key = &entry.row.key;
            if key.scope() != &scope || previous.is_some_and(|prior| prior >= key) {
                return Err(invalid_registry("invalid registry key order or selector"));
            }
            entry
                .source
                .admit(owner, key, entry.row.expected, admission)?;
            entry.row.admit_retained(admission)?;
            previous = Some(key);
        }
        Ok(())
    }
}

/// The reached registry and value admission stay paired while the input is evaluated.
pub(crate) struct RuntimeRegistryWritePreparation<'key, 'budget> {
    len: usize,
    previous: Option<Arc<RuntimeRegistryValue>>,
    source: RuntimeRegistryWriteSource,
    value: PreparedRegistryValue<'key, 'budget>,
}

impl RuntimeRegistryWritePreparation<'_, '_> {
    pub(crate) fn capture(
        self,
        value: &RuntimeValue,
    ) -> Result<RuntimePreparedRegistryWrite, AwbcRuntimeValueSnapshotError> {
        Ok(RuntimePreparedRegistryWrite {
            len: self.len,
            previous: self.previous,
            source: self.source,
            update: self.value.capture(value)?,
        })
    }
}

/// A checked write held outside its scope until its owning step commits.
pub(crate) struct RuntimePreparedRegistryWrite {
    len: usize,
    previous: Option<Arc<RuntimeRegistryValue>>,
    source: RuntimeRegistryWriteSource,
    update: RuntimeRegistryStateUpdate,
}

impl RuntimePreparedRegistryWrite {
    fn position(
        &self,
        rows: &RuntimeScopeRegistry,
    ) -> Result<Result<usize, usize>, AwbcRuntimeValueSnapshotError> {
        if rows.0.len() != self.len {
            return Err(invalid_registry("registry size changed before write"));
        }
        rows.revalidate_row(self.update.key(), self.previous.as_ref())
    }

    pub(crate) fn reserve(
        &self,
        rows: &mut RuntimeScopeRegistry,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        rows.reserve_write(self.position(rows)?)
    }

    pub(crate) fn commit(
        self,
        rows: &mut RuntimeScopeRegistry,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let position = self.position(rows)?;
        rows.reserve_write(position)?;
        rows.commit_write(position, self.source, self.update);
        Ok(())
    }
}

/// A registry removal can consume only the row observed during preflight.
pub(crate) struct RuntimePreparedRegistryDrop {
    key: LifetimeRegistryKey,
    row: Option<Arc<RuntimeRegistryValue>>,
}

impl RuntimePreparedRegistryDrop {
    fn position(
        &self,
        rows: &RuntimeScopeRegistry,
    ) -> Result<Option<usize>, AwbcRuntimeValueSnapshotError> {
        Ok(rows.revalidate_row(&self.key, self.row.as_ref())?.ok())
    }

    pub(crate) fn commit(
        self,
        rows: &mut RuntimeScopeRegistry,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if let Some(index) = self.position(rows)? {
            rows.0.remove(index);
        }
        Ok(())
    }
}

/// A scoped mutation staged outside its reached registry until the owning
/// execution has admitted the rest of the instruction's effects.
pub(crate) enum RuntimePreparedRegistryMutation {
    Write(RuntimePreparedRegistryWrite),
    Drop(RuntimePreparedRegistryDrop),
}

impl RuntimePreparedRegistryMutation {
    pub(crate) fn reserve(
        &self,
        rows: &mut RuntimeScopeRegistry,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        match self {
            Self::Write(write) => write.reserve(rows),
            Self::Drop(drop) => drop.position(rows).map(|_| ()),
        }
    }

    pub(crate) fn commit(
        self,
        rows: &mut RuntimeScopeRegistry,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        match self {
            Self::Write(write) => write.commit(rows),
            Self::Drop(drop) => drop.commit(rows),
        }
    }
}

fn invalid_registry(message: &str) -> AwbcRuntimeValueSnapshotError {
    AwbcRuntimeValueSnapshotError::Message {
        message: message.to_owned(),
    }
}
