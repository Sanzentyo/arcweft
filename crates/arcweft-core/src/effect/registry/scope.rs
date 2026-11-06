//! Retained, replay-safe values on one reached execution scope.

use super::{
    PreparedRegistryValue, RuntimeRegistryReadKind, RuntimeRegistryStateUpdate,
    RuntimeRegistryValue,
};
use crate::awbc::schema::{AwbcEffectPayload, AwbcEffectPlanId};
use crate::effect::RuntimeDropPolicy;
use crate::line_task::{LineRuntimeError, RuntimeHandleDropAuthorization, RuntimeHandleOwnerSlot};
use crate::pattern::RuntimeSemanticTypeId;
use crate::runtime_id::{
    DialogueActivationId, RuntimeFiberInstanceId, RuntimeFrameInstanceId, RuntimeLineHandleToken,
};
use crate::task::RuntimeProgramOwner;
use crate::value::ownership::{RuntimeAffineLineHandle, RuntimeValueOwnershipRequirement};
use crate::value::{AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission, RuntimeValue};
use arcweft_id::{LifetimeRegistryKey, LifetimeScopeKind};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

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

    const fn ownership(self) -> RuntimeValueOwnershipRequirement {
        match self {
            Self::Dialogue(_) => RuntimeValueOwnershipRequirement::Owned,
            _ => RuntimeValueOwnershipRequirement::Unrestricted,
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
#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub(crate) struct RuntimeScopeRegistry {
    rows: Vec<RegistryEntry>,
    // A staged mutation is local to this live table, including when its key
    // is absent. Inert copies and restores have no pending mutations.
    #[serde(skip)]
    identity: OnceLock<Arc<()>>,
}

impl Clone for RuntimeScopeRegistry {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
            identity: OnceLock::new(),
        }
    }
}

impl PartialEq for RuntimeScopeRegistry {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
    }
}

impl RuntimeScopeRegistry {
    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.rows.clear();
        self.identity = OnceLock::new();
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
                admission.validate_option_preview(
                    row.map(|row| (&row.value).into()),
                    expected,
                    binding,
                    RuntimeValueOwnershipRequirement::Unrestricted,
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
        let Ok(index) = self.rows.binary_search_by(|entry| entry.row.key.cmp(key)) else {
            return Ok(None);
        };
        let entry = &self.rows[index];
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
                entry
                    .row
                    .validate_value(admission, expected, binding, owner.ownership())?;
                Some(Arc::clone(&entry.row))
            }
            None if kind == RuntimeRegistryReadKind::Optional => None,
            None => return Err(invalid_registry("required registry value is absent")),
        };
        admission.validate_metadata(&(key, expected, binding))?;
        Ok(RuntimePreparedRegistryDrop {
            key: key.clone(),
            observed: self.observe(row),
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
        let position = self.rows.binary_search_by(|entry| entry.row.key.cmp(key));
        admission.validate_metadata_collection(self.rows.len() + usize::from(position.is_err()))?;
        source.admit(owner, key, value_type, admission)?;
        let value = RuntimeRegistryStateUpdate::prepare(key, value_type, binding, admission)?;
        Ok(RuntimeRegistryWritePreparation {
            len: self.rows.len(),
            observed: self.observe(position.ok().map(|index| Arc::clone(&self.rows[index].row))),
            source,
            value,
            ownership: owner.ownership(),
        })
    }

    fn observe(&self, row: Option<Arc<RuntimeRegistryValue>>) -> RuntimeRegistryObservation {
        RuntimeRegistryObservation {
            registry: Arc::clone(self.identity.get_or_init(|| Arc::new(()))),
            row,
        }
    }

    fn revalidate_row(
        &self,
        key: &LifetimeRegistryKey,
        observed: &RuntimeRegistryObservation,
    ) -> Result<Result<usize, usize>, AwbcRuntimeValueSnapshotError> {
        if !self
            .identity
            .get()
            .is_some_and(|identity| Arc::ptr_eq(identity, &observed.registry))
        {
            return Err(invalid_registry(
                "registry mutation selected a different table",
            ));
        }
        let position = self.rows.binary_search_by(|entry| entry.row.key.cmp(key));
        match (position, observed.row.as_ref()) {
            (Ok(index), Some(row)) if Arc::ptr_eq(&self.rows[index].row, row) => Ok(position),
            (Err(_), None) => Ok(position),
            _ => Err(invalid_registry("registry row changed before commit")),
        }
    }

    fn reserve_write(
        &mut self,
        position: Result<usize, usize>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if position.is_err() {
            self.rows
                .try_reserve(1)
                .map_err(|_| invalid_registry("registry allocation failed"))?;
        }
        Ok(())
    }

    fn commit_write(
        &mut self,
        position: Result<usize, usize>,
        source: RuntimeRegistryWriteSource,
        row: RuntimeRegistryValue,
    ) {
        let entry = RegistryEntry {
            source,
            row: Arc::new(row),
        };
        match position {
            Ok(index) => self.rows[index] = entry,
            Err(index) => self.rows.insert(index, entry),
        }
    }

    pub(crate) fn admit_retained(
        &self,
        owner: RuntimeRegistryOwner,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata_collection(self.rows.len())?;
        let scope = owner.scope();
        let mut previous = None;
        for entry in &self.rows {
            let key = &entry.row.key;
            if key.scope() != &scope || previous.is_some_and(|prior| prior >= key) {
                return Err(invalid_registry("invalid registry key order or selector"));
            }
            entry
                .source
                .admit(owner, key, entry.row.expected, admission)?;
            entry.row.admit_retained(admission, owner.ownership())?;
            previous = Some(key);
        }
        Ok(())
    }

    pub(crate) fn line_handle_rows(
        &self,
    ) -> Result<Vec<(&LifetimeRegistryKey, RuntimeAffineLineHandle)>, AwbcRuntimeValueSnapshotError>
    {
        let mut handles = Vec::new();
        for entry in &self.rows {
            handles.extend(
                entry
                    .row
                    .value
                    .view()
                    .line_registry_handles()?
                    .into_iter()
                    .map(|handle| (&entry.row.key, handle)),
            );
        }
        Ok(handles)
    }
}

struct RuntimeRegistryObservation {
    registry: Arc<()>,
    row: Option<Arc<RuntimeRegistryValue>>,
}

/// The reached registry and value admission stay paired while the input is evaluated.
pub(crate) struct RuntimeRegistryWritePreparation<'key, 'budget> {
    len: usize,
    observed: RuntimeRegistryObservation,
    source: RuntimeRegistryWriteSource,
    value: PreparedRegistryValue<'key, 'budget>,
    ownership: RuntimeValueOwnershipRequirement,
}

impl RuntimeRegistryWritePreparation<'_, '_> {
    pub(crate) fn capture(
        mut self,
        value: &RuntimeValue,
    ) -> Result<RuntimePreparedRegistryWrite, AwbcRuntimeValueSnapshotError> {
        Ok(RuntimePreparedRegistryWrite {
            len: self.len,
            observed: self.observed,
            source: self.source,
            row: self.value.capture_row(value, self.ownership)?,
        })
    }
}

/// A checked write held outside its scope until its owning step commits.
pub(crate) struct RuntimePreparedRegistryWrite {
    len: usize,
    observed: RuntimeRegistryObservation,
    source: RuntimeRegistryWriteSource,
    row: RuntimeRegistryValue,
}

impl RuntimePreparedRegistryWrite {
    fn position(
        &self,
        rows: &RuntimeScopeRegistry,
    ) -> Result<Result<usize, usize>, AwbcRuntimeValueSnapshotError> {
        if rows.rows.len() != self.len {
            return Err(invalid_registry("registry size changed before write"));
        }
        rows.revalidate_row(&self.row.key, &self.observed)
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
        rows.commit_write(position, self.source, self.row);
        Ok(())
    }
}

/// A registry removal can consume only the row observed during preflight.
pub(crate) struct RuntimePreparedRegistryDrop {
    key: LifetimeRegistryKey,
    observed: RuntimeRegistryObservation,
}

impl RuntimePreparedRegistryDrop {
    fn position(
        &self,
        rows: &RuntimeScopeRegistry,
    ) -> Result<Option<usize>, AwbcRuntimeValueSnapshotError> {
        Ok(rows.revalidate_row(&self.key, &self.observed)?.ok())
    }

    pub(crate) fn commit(
        self,
        rows: &mut RuntimeScopeRegistry,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if let Some(index) = self.position(rows)? {
            rows.rows.remove(index);
        }
        Ok(())
    }
}

/// A scoped mutation staged outside its reached registry until the owning
/// execution has admitted the rest of the instruction's effects.
pub(crate) enum RuntimePreparedRegistryMutation {
    Write(RuntimePreparedRegistryWrite),
    Drop(RuntimePreparedRegistryDrop, RuntimeDropPolicy),
}

impl RuntimePreparedRegistryMutation {
    pub(crate) fn line_change(
        &self,
    ) -> Result<Option<RuntimeRegistryLineChange<'_>>, AwbcRuntimeValueSnapshotError> {
        let (key, observed, incoming, policy) = match self {
            Self::Write(write) => (
                &write.row.key,
                &write.observed,
                Some(&write.row),
                RuntimeDropPolicy::Default,
            ),
            Self::Drop(drop, policy) => (&drop.key, &drop.observed, None, *policy),
        };
        if key.scope() != &LifetimeScopeKind::Line {
            return Ok(None);
        }
        let handles = |row: Option<&RuntimeRegistryValue>| match row {
            Some(row) => row.value.view().line_registry_handles(),
            None => Ok(Vec::new()),
        };
        let previous = handles(observed.row.as_deref())?;
        let incoming = handles(incoming)?;
        Ok(
            (!previous.is_empty() || !incoming.is_empty()).then_some(RuntimeRegistryLineChange {
                key,
                previous,
                incoming,
                policy,
            }),
        )
    }

    pub(crate) fn reserve(
        &self,
        rows: &mut RuntimeScopeRegistry,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        match self {
            Self::Write(write) => write.reserve(rows),
            Self::Drop(drop, _) => drop.position(rows).map(|_| ()),
        }
    }

    pub(crate) fn commit(
        self,
        rows: &mut RuntimeScopeRegistry,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        match self {
            Self::Write(write) => write.commit(rows),
            Self::Drop(drop, _) => drop.commit(rows),
        }
    }
}

pub(crate) struct RuntimeRegistryLineChange<'a> {
    key: &'a LifetimeRegistryKey,
    previous: Vec<RuntimeAffineLineHandle>,
    incoming: Vec<RuntimeAffineLineHandle>,
    policy: RuntimeDropPolicy,
}

impl RuntimeRegistryLineChange<'_> {
    pub(crate) fn merge_receiver_custody(
        &self,
        receiver: DialogueActivationId,
        before: &mut BTreeMap<RuntimeLineHandleToken, RuntimeHandleOwnerSlot>,
        after: &mut BTreeMap<RuntimeLineHandleToken, RuntimeHandleOwnerSlot>,
        drops: &mut RuntimeHandleDropAuthorization,
        mut check_issuer: impl FnMut(&RuntimeAffineLineHandle) -> Result<(), LineRuntimeError>,
    ) -> Result<(), LineRuntimeError> {
        let registry_slot =
            |handle: &RuntimeAffineLineHandle| RuntimeHandleOwnerSlot::LineRegistry {
                receiver,
                key: self.key.clone(),
                path: handle.path().clone(),
            };
        for handle in &self.previous {
            check_issuer(handle)?;
            let token = handle.token();
            if after.contains_key(token)
                || before
                    .insert(token.clone(), registry_slot(handle))
                    .is_some()
            {
                return Err(LineRuntimeError::DuplicateHandleOccurrence);
            }
        }
        drops.authorize_tokens_with_policy(
            self.previous.iter().map(|handle| handle.token().clone()),
            self.policy,
        )?;
        for handle in &self.incoming {
            check_issuer(handle)?;
            let token = handle.token();
            if !before.contains_key(token) {
                return Err(LineRuntimeError::WrongOwner);
            }
            if after.insert(token.clone(), registry_slot(handle)).is_some() {
                return Err(LineRuntimeError::DuplicateHandleOccurrence);
            }
        }
        Ok(())
    }
}

fn invalid_registry(message: &str) -> AwbcRuntimeValueSnapshotError {
    AwbcRuntimeValueSnapshotError::Message {
        message: message.to_owned(),
    }
}
