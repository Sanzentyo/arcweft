//! Inert Native session transport. The destination's Plan admits the complete
//! candidate before any of its values become live.

use super::{
    Engine, FlowFiber, NativeEngineRollbackImage, NativeFlowExecutionFrameRollbackImage,
    NativeMainLaunch, dialogue::DialogueActivationStoreRollbackImage, pure_helper_i64_call_shapes,
};
use crate::effect::RuntimeArtifactFingerprint;
use crate::entry::{ActiveEntrySnapshotV1, RuntimeNominalDeclarationId, RuntimeSchemaLimits};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::task_source::RuntimeTaskSourceProgram;
use crate::presentation::resources::{
    PresentationBanks, PresentationBanksSaveSnapshot, PresentationLedgerLimits,
};
use crate::root::{RootStateSaveSnapshotError, RootStateSaveSnapshotV1};
use crate::task::{
    NeedId, NeedProducerRestorePolicy, RuntimeNeedProducerRegistrySaveSnapshot, RuntimeProgramOwner,
};
use crate::value::{AwbcRuntimeValueSnapshot, RuntimeRetainedValueAdmission};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

type NativeEngineSaveState = Engine<
    RuntimeNeedProducerRegistrySaveSnapshot,
    AwbcRuntimeValueSnapshot,
    RootStateSaveSnapshotV1,
    NativeFlowExecutionFrameRollbackImage<()>,
    DialogueActivationStoreRollbackImage,
    Vec<(
        RuntimeSemanticTypeId,
        RuntimeNominalDeclarationId,
        AwbcRuntimeValueSnapshot,
    )>,
    PresentationBanksSaveSnapshot,
    (),
>;

/// Untrusted, serializable Native Engine state. A matching Engine must admit
/// its execution and ownership coordinates before it can be resumed.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeEngineSaveSnapshot {
    executable: [u8; 32],
    artifact: Option<RuntimeArtifactFingerprint>,
    state: NativeEngineSaveState,
}

#[derive(Debug, Error)]
pub enum NativeEngineSaveError {
    #[error("Native save belongs to a different original Plan")]
    Program,
    #[error("Native save belongs to a different runtime generation")]
    Generation,
    #[error("Native save belongs to a different original invocation or entry")]
    Entry,
    #[error("Native save requires quiescence for Needs {needs:?}")]
    NeedsQuiescence { needs: Vec<NeedId> },
    #[error(transparent)]
    Root(#[from] Box<RootStateSaveSnapshotError>),
    #[error("invalid Native save state: {0}")]
    State(String),
}

// Only the program-bound owners change representation between the wire and
// rollback states. Keep the transfer of shared fields identical both ways.
macro_rules! transfer_engine_save_state {
    ($source:ident, $($owner:ident: $value:expr),+ $(,)?) => {
        Engine {
            $($owner: $value,)+
            evaluation_stats: Default::default(),
            format_context: $source.format_context,
            generation: $source.generation,
            task_request_quota_remaining: 0,
            scalar_step_charge: None,
            need_publications: $source.need_publications,
            latest_need_publications: $source.latest_need_publications,
            need_publication_frontiers: $source.need_publication_frontiers,
            main_launch: $source.main_launch,
            next_fiber_id: $source.next_fiber_id,
            next_scheduled_scope_sequence: $source.next_scheduled_scope_sequence,
            flow_defer_frontier: $source.flow_defer_frontier,
            dialogue_occurrences: $source.dialogue_occurrences,
            dialogue_activations: $source.dialogue_activations,
            dialogue_effect_callback_activations: $source.dialogue_effect_callback_activations,
            run_child_next: $source.run_child_next,
            pure_i64_batch_inputs: Vec::new(),
            pure_i64_batch_outputs: Vec::new(),
            audio_epoch: $source.audio_epoch,
            next_audio_sequence: $source.next_audio_sequence,
            next_host_call_sequence: $source.next_host_call_sequence,
        }
    };
}

impl Engine {
    /// Captures a quiescent, inert snapshot without changing the running engine.
    pub fn snapshot_for_save(&self) -> Result<NativeEngineSaveSnapshot, NativeEngineSaveError> {
        let needs = self.quiescence_blocking_needs();
        if !needs.is_empty() {
            return Err(NativeEngineSaveError::NeedsQuiescence { needs });
        }
        if self.scalar_step_charge.is_some() {
            return Err(NativeEngineSaveError::State(
                "an operation is still being dispatched".into(),
            ));
        }
        if let Some(root) = &self.root
            && self.selected_entry().is_none_or(|entry| {
                entry.id != root.active().entry || entry.binding != root.active().binding
            })
        {
            return Err(NativeEngineSaveError::Entry);
        }
        let image = self
            .inert_rollback_image()
            .map_err(NativeEngineSaveError::State)?;
        let detach = |fiber: FlowFiber<NativeFlowExecutionFrameRollbackImage>| {
            fiber.map_frame(|frame| frame.with_program(()))
        };
        let state = transfer_engine_save_state!(
            image,
            plan: (),
            need_producers: RuntimeNeedProducerRegistrySaveSnapshot::from_admitted(
                &image.need_producers,
            ),
            actions: image.actions.snapshot_rows_admitted(),
            presentation: image.presentation.as_ref().map(PresentationBanks::inert_state_admitted),
            root: image
                .root
                .as_ref()
                .map(|root| RootStateSaveSnapshotV1::from_runtime_image(root).map_err(Box::new))
                .transpose()?,
            fiber: detach(image.fiber),
            parked_main: image.parked_main.map(detach),
            child_fibers: image.child_fibers.into_iter().map(detach).collect(),
            pure_helper_i64_call_shapes: Vec::new(),
        );
        Ok(NativeEngineSaveSnapshot {
            executable: *self.plan.executable_semantic_digest().as_bytes(),
            artifact: self.plan.artifact(),
            state,
        })
    }

    /// Resumes a saved session for the same Plan, generation and original
    /// launch. Admission failure leaves the destination intact.
    pub fn restore_save_snapshot(
        &mut self,
        snapshot: NativeEngineSaveSnapshot,
    ) -> Result<(), NativeEngineSaveError> {
        *self = self.prepare_save_restore(snapshot, None)?;
        Ok(())
    }

    pub(crate) fn prepare_save_restore(
        &self,
        snapshot: NativeEngineSaveSnapshot,
        validator: Option<crate::value::RetainedValueValidator>,
    ) -> Result<Self, NativeEngineSaveError> {
        if snapshot.executable != *self.plan.executable_semantic_digest().as_bytes()
            || snapshot.artifact != self.plan.artifact()
        {
            return Err(NativeEngineSaveError::Program);
        }
        let state = snapshot.state;
        if state.generation != self.generation {
            return Err(NativeEngineSaveError::Generation);
        }
        if self.main_launch != state.main_launch || self.root.is_some() != state.root.is_some() {
            return Err(NativeEngineSaveError::Entry);
        }

        let limits = RuntimeSchemaLimits::engine_default();
        let owner = RuntimeProgramOwner::Plan(Arc::clone(&self.plan));
        let source = RuntimeTaskSourceProgram::native(Arc::clone(&self.plan));
        let mut admission = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        if let Some(validator) = validator {
            admission.set_retained_validator(validator);
        }
        let need_producers = state
            .need_producers
            .bind_assertions(Some(&source), &mut admission)
            .map_err(|error| NativeEngineSaveError::State(error.to_string()))?;
        let needs = need_producers
            .quiescence_blocking_needs()
            .collect::<Vec<_>>();
        if !needs.is_empty() {
            return Err(NativeEngineSaveError::NeedsQuiescence { needs });
        }
        let root = if let Some(root) = state.root {
            let entry = self.selected_entry().ok_or(NativeEngineSaveError::Entry)?;
            Some(
                root.into_runtime_image(entry, &mut admission)
                    .map_err(Box::new)?,
            )
        } else {
            None
        };
        let actions = crate::action::RuntimeActionMailbox::from_rows(state.actions, &mut admission)
            .map_err(|error| NativeEngineSaveError::State(error.to_string()))?;
        let presentation = state.presentation.map(|banks| {
            banks.retain_program(&owner, PresentationLedgerLimits::for_schema(limits))
        });
        let bind = |fiber: FlowFiber<NativeFlowExecutionFrameRollbackImage<()>>| {
            fiber.map_frame(|frame| frame.bind_program(owner.clone()))
        };
        let image: NativeEngineRollbackImage = transfer_engine_save_state!(
            state,
            plan: Arc::clone(&self.plan),
            need_producers: need_producers,
            actions: actions,
            presentation: presentation,
            root: root,
            fiber: bind(state.fiber),
            parked_main: state.parked_main.map(bind),
            child_fibers: state.child_fibers.into_iter().map(bind).collect(),
            pure_helper_i64_call_shapes: pure_helper_i64_call_shapes(&self.plan),
        );
        image
            .dialogue_activations
            .validate_parent_allocations(image.next_fiber_id)
            .map_err(NativeEngineSaveError::State)?;
        image
            .admit_shared_rollback_state(&self.plan, limits, &mut admission)
            .map_err(NativeEngineSaveError::State)?;
        Self::from_admitted_rollback_image(image, &owner, NeedProducerRestorePolicy::Resume)
            .map_err(NativeEngineSaveError::State)
    }

    pub(crate) fn selected_entry(&self) -> Option<&ActiveEntrySnapshotV1> {
        match &self.main_launch {
            NativeMainLaunch::Entry(entry) => Some(entry),
            _ => None,
        }
    }
}

// Structured identifiers cannot be JSON object keys. Ordered rows preserve the
// original map order and admit each key at most once.
pub(super) mod ordered_rows {
    use crate::entry::RuntimeSchemaLimits;
    use serde::de::{Error, SeqAccess, Visitor};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::{collections::BTreeMap, fmt, marker::PhantomData};

    pub fn serialize<K: Serialize, V: Serialize, S: Serializer>(
        rows: &BTreeMap<K, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(rows.iter())
    }

    pub fn deserialize<'de, K, V, D>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        struct OrderedRows<K, V>(PhantomData<(K, V)>);

        impl<'de, K: Deserialize<'de> + Ord, V: Deserialize<'de>> Visitor<'de> for OrderedRows<K, V> {
            type Value = BTreeMap<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("unique map rows in strictly ascending order")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut rows = BTreeMap::new();
                let limits = RuntimeSchemaLimits::engine_default();
                while let Some((key, value)) = seq.next_element::<(K, V)>()? {
                    if !limits.permits_sequence_items(rows.len() + 1) {
                        return Err(A::Error::custom("too many saved map rows"));
                    }
                    if rows.last_key_value().is_some_and(|(last, _)| last >= &key) {
                        return Err(A::Error::custom("saved map rows duplicate or out of order"));
                    }
                    rows.insert(key, value);
                }
                Ok(rows)
            }
        }

        deserializer.deserialize_seq(OrderedRows(PhantomData))
    }
}
