//! Durable source-execution state shared by the executor and native host.

use crate::native_task::{
    NativeTaskBridge, NativeTaskBridgeSaveError, NativeTaskBridgeSaveSnapshot,
};
use arcweft_core::engine::NativeEngineSaveSnapshot;
use arcweft_core::executor::{
    ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot, ArcweftRuntimeExecutorSnapshotError,
};
use arcweft_core::value::RuntimeBorrowedValue;
use arcweft_save::{SaveDecodeOptions, SaveEnvelopeLimits, SaveSchemaId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SCHEMA: &str = "arcweft.native_source_session";
const VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSourceSessionSave {
    next_step_index: u64,
    executor: Box<NativeEngineSaveSnapshot>,
    dispatches: NativeTaskBridgeSaveSnapshot,
}

#[derive(Debug, Error)]
pub enum NativeSourceSessionSaveError {
    #[error("source session save requires a Native executor, received {tier}")]
    ExecutorTier { tier: &'static str },
    #[error(
        "source session has {tasks} task events and {calls} host results to deliver before saving"
    )]
    PendingIngress { tasks: usize, calls: usize },
    #[error("saved dispatch epoch does not precede the next source session step")]
    DispatchEpoch,
    #[error(transparent)]
    Executor(#[from] ArcweftRuntimeExecutorSnapshotError),
    #[error(transparent)]
    Dispatches(#[from] NativeTaskBridgeSaveError),
    #[error("source session save codec error: {message}")]
    Codec { message: String },
}

impl NativeSourceSessionSave {
    pub fn capture(
        executor: &ArcweftRuntimeExecutor,
        host: &NativeTaskBridge,
        next_step_index: u64,
        task_events: &[arcweft_core::task::TaskEvent],
        host_results: &[arcweft_core::step::RuntimeHostCallResult],
    ) -> Result<Self, NativeSourceSessionSaveError> {
        if !task_events.is_empty() || !host_results.is_empty() {
            return Err(NativeSourceSessionSaveError::PendingIngress {
                tasks: task_events.len(),
                calls: host_results.len(),
            });
        }
        let ArcweftRuntimeExecutorSnapshot::Native(snapshot) = executor.snapshot()? else {
            return Err(NativeSourceSessionSaveError::ExecutorTier {
                tier: executor.tier().as_str(),
            });
        };
        let save = Self {
            next_step_index,
            dispatches: host.snapshot_dispatch_journal(&executor.restartable_dispatches())?,
            executor: snapshot,
        };
        save.validate_epoch()?;
        Ok(save)
    }

    pub fn max_file_bytes() -> u64 {
        SaveEnvelopeLimits::default().max_envelope_bytes
    }

    pub fn encode(&self) -> Result<Vec<u8>, NativeSourceSessionSaveError> {
        arcweft_save::encode_typed_json_save(self, SaveSchemaId::new(SCHEMA), VERSION).map_err(
            |error| NativeSourceSessionSaveError::Codec {
                message: error.to_string(),
            },
        )
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, NativeSourceSessionSaveError> {
        arcweft_save::decode_strict_typed_json_save(
            bytes,
            &SaveSchemaId::new(SCHEMA),
            VERSION,
            &SaveDecodeOptions::default(),
        )
        .map_err(|error| NativeSourceSessionSaveError::Codec {
            message: error.to_string(),
        })
    }

    pub fn restore(
        self,
        executor: &mut ArcweftRuntimeExecutor,
        host: &mut NativeTaskBridge,
        validate_value: impl for<'a> Fn(RuntimeBorrowedValue<'a>) -> Result<(), String>
        + Send
        + Sync
        + 'static,
    ) -> Result<u64, NativeSourceSessionSaveError> {
        self.validate_epoch()?;
        executor.restore_native_session(*self.executor, validate_value, |launches| {
            host.restore_dispatch_journal(self.dispatches, launches)
                .map_err(|error| error.to_string())
        })?;
        Ok(self.next_step_index)
    }

    fn validate_epoch(&self) -> Result<(), NativeSourceSessionSaveError> {
        if self.dispatches.precedes_epoch(self.next_step_index) {
            Ok(())
        } else {
            Err(NativeSourceSessionSaveError::DispatchEpoch)
        }
    }
}
