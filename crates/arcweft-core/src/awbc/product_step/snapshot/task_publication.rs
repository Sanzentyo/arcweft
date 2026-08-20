use crate::task::{LogicalEpoch, TaskEvent, TaskEventKind, TaskId, TaskSequence};
use crate::value::{AwbcRuntimeValueSnapshot, RuntimePayload};

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductTaskEventSaveSnapshot {
    pub logical_epoch: LogicalEpoch,
    pub task_id: TaskId,
    pub sequence: TaskSequence,
    pub kind: AwbcProductTaskEventKindSaveSnapshot,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
pub enum AwbcProductTaskEventKindSaveSnapshot {
    Ready(AwbcRuntimeValueSnapshot),
    Failed(String),
    Cancelled,
    Progress(arcweft_need::Progress),
}

impl AwbcProductTaskEventSaveSnapshot {
    pub(super) fn from_live(event: &TaskEvent) -> Result<Self, String> {
        Ok(Self {
            logical_epoch: event.logical_epoch,
            task_id: event.task_id.clone(),
            sequence: event.sequence,
            kind: match &event.kind {
                TaskEventKind::Ready(value) => AwbcProductTaskEventKindSaveSnapshot::Ready(
                    AwbcRuntimeValueSnapshot::from_runtime_value(value.value())
                        .map_err(|error| error.to_string())?,
                ),
                TaskEventKind::Failed(error) => {
                    AwbcProductTaskEventKindSaveSnapshot::Failed(error.clone())
                }
                TaskEventKind::Cancelled => AwbcProductTaskEventKindSaveSnapshot::Cancelled,
                TaskEventKind::Progress(progress) => {
                    AwbcProductTaskEventKindSaveSnapshot::Progress(progress.clone())
                }
            },
        })
    }

    pub(super) fn into_live(self) -> Result<TaskEvent, String> {
        Ok(TaskEvent {
            logical_epoch: self.logical_epoch,
            task_id: self.task_id,
            sequence: self.sequence,
            kind: match self.kind {
                AwbcProductTaskEventKindSaveSnapshot::Ready(value) => {
                    TaskEventKind::Ready(RuntimePayload::from(
                        value
                            .into_runtime_value()
                            .map_err(|error| error.to_string())?,
                    ))
                }
                AwbcProductTaskEventKindSaveSnapshot::Failed(error) => TaskEventKind::Failed(error),
                AwbcProductTaskEventKindSaveSnapshot::Cancelled => TaskEventKind::Cancelled,
                AwbcProductTaskEventKindSaveSnapshot::Progress(progress) => {
                    TaskEventKind::Progress(progress)
                }
            },
        })
    }
}
