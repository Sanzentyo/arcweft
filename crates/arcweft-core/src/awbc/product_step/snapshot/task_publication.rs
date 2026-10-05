use crate::task::{
    RuntimeTaskFailure, TaskCorrelation, TaskEvent, TaskEventKind, TaskPublicationCursor,
};
use crate::value::{AwbcRuntimeValueSnapshot, RuntimePayload};

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcProductTaskEventSaveSnapshot {
    pub correlation: TaskCorrelation,
    pub cursor: TaskPublicationCursor,
    pub kind: AwbcProductTaskEventKindSaveSnapshot,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq, serde::Serialize)]
pub enum AwbcProductTaskEventKindSaveSnapshot {
    Ready(AwbcRuntimeValueSnapshot),
    InfrastructureFailure(RuntimeTaskFailure),
    Cancelled,
    Progress(arcweft_need::Progress),
}

impl AwbcProductTaskEventSaveSnapshot {
    pub(super) fn from_live(event: &TaskEvent) -> Result<Self, String> {
        Ok(Self {
            correlation: event.correlation,
            cursor: event.cursor,
            kind: match &event.kind {
                TaskEventKind::Ready(value) => AwbcProductTaskEventKindSaveSnapshot::Ready(
                    AwbcRuntimeValueSnapshot::from_runtime_value(value.value())
                        .map_err(|error| error.to_string())?,
                ),
                TaskEventKind::InfrastructureFailure(error) => {
                    AwbcProductTaskEventKindSaveSnapshot::InfrastructureFailure(error.clone())
                }
                TaskEventKind::Cancelled => AwbcProductTaskEventKindSaveSnapshot::Cancelled,
                TaskEventKind::Progress(progress) => {
                    AwbcProductTaskEventKindSaveSnapshot::Progress(progress.clone())
                }
            },
        })
    }

    pub(super) fn into_live(
        self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<TaskEvent, String> {
        Ok(TaskEvent {
            correlation: self.correlation,
            cursor: self.cursor,
            kind: match self.kind {
                AwbcProductTaskEventKindSaveSnapshot::Ready(value) => {
                    TaskEventKind::Ready(RuntimePayload::from(
                        value
                            .into_runtime_value_for_program(owner)
                            .map_err(|error| error.to_string())?,
                    ))
                }
                AwbcProductTaskEventKindSaveSnapshot::InfrastructureFailure(error) => {
                    TaskEventKind::InfrastructureFailure(error)
                }
                AwbcProductTaskEventKindSaveSnapshot::Cancelled => TaskEventKind::Cancelled,
                AwbcProductTaskEventKindSaveSnapshot::Progress(progress) => {
                    TaskEventKind::Progress(progress)
                }
            },
        })
    }
}
