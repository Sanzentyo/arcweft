use crate::task::{TaskEvent, TaskEventKind};
use crate::value::AwbcRuntimeValueSnapshot;

pub type AwbcProductTaskEventSaveSnapshot = TaskEvent<AwbcRuntimeValueSnapshot>;
pub type AwbcProductTaskEventKindSaveSnapshot = TaskEventKind<AwbcRuntimeValueSnapshot>;

impl AwbcProductTaskEventSaveSnapshot {
    pub(super) fn from_admitted_live(
        event: &TaskEvent,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(Self {
            correlation: event.correlation,
            cursor: event.cursor,
            kind: match &event.kind {
                TaskEventKind::Ready(value) => AwbcProductTaskEventKindSaveSnapshot::Ready(
                    AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value.value(), owner)
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

    pub(super) fn into_admitted_live(
        self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<TaskEvent, String> {
        Ok(TaskEvent {
            correlation: self.correlation,
            cursor: self.cursor,
            kind: match self.kind {
                AwbcProductTaskEventKindSaveSnapshot::Ready(value) => {
                    let value = value
                        .into_admitted_runtime_value(owner)
                        .map_err(|error| error.to_string())?;
                    TaskEventKind::Ready(value.into())
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

/// Fixed `TaskId` atoms cannot be JSON object keys. Version-one persistence uses
/// defining task order and rejects duplicate or reordered rows at decode.
pub(super) mod rows {
    use crate::task::{TaskId, TaskPublicationCursor};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub(in crate::awbc::product_step::snapshot) fn serialize<S: Serializer>(
        values: &BTreeMap<TaskId, TaskPublicationCursor>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        values.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub(in crate::awbc::product_step::snapshot) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<TaskId, TaskPublicationCursor>, D::Error> {
        let rows = Vec::<(TaskId, TaskPublicationCursor)>::deserialize(deserializer)?;
        let mut values = BTreeMap::new();
        let mut previous = None;
        for (task, cursor) in rows {
            if previous.is_some_and(|previous| previous >= task) {
                return Err(serde::de::Error::custom(
                    "task publication rows must have unique increasing task identities",
                ));
            }
            previous = Some(task);
            values.insert(task, cursor);
        }
        Ok(values)
    }
}
