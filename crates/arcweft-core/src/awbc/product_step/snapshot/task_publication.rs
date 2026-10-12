use crate::task::{
    RuntimeTaskFailure, TaskCorrelation, TaskEvent, TaskEventKind, TaskOutcomeContract,
    TaskPublicationCursor,
};
use crate::value::AwbcRuntimeValueSnapshot;

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
    pub(super) fn from_live(
        event: &TaskEvent,
        owner: &crate::task::RuntimeProgramOwner,
        outcome: Option<&TaskOutcomeContract>,
    ) -> Result<Self, String> {
        if let TaskEventKind::Ready(value) = &event.kind {
            require_outcome(outcome, owner)?
                .validate_value(value.value())
                .map_err(|error| error.to_string())?;
        }
        Ok(Self {
            correlation: event.correlation,
            cursor: event.cursor,
            kind: match &event.kind {
                TaskEventKind::Ready(value) => AwbcProductTaskEventKindSaveSnapshot::Ready(
                    AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value.value(), owner)
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
        outcome: Option<&TaskOutcomeContract>,
    ) -> Result<TaskEvent, String> {
        Ok(TaskEvent {
            correlation: self.correlation,
            cursor: self.cursor,
            kind: match self.kind {
                AwbcProductTaskEventKindSaveSnapshot::Ready(value) => {
                    let outcome = require_outcome(outcome, owner)?;
                    let value = value
                        .into_runtime_value_for_program(owner)
                        .map_err(|error| error.to_string())?;
                    TaskEventKind::Ready(
                        outcome
                            .try_payload(value)
                            .map_err(|error| error.to_string())?,
                    )
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

fn require_outcome(
    outcome: Option<&TaskOutcomeContract>,
    owner: &crate::task::RuntimeProgramOwner,
) -> Result<crate::task::BoundTaskOutcome, String> {
    outcome
        .ok_or_else(|| "queued Ready has no exact admitted task correlation".to_owned())?
        .bind_for_value(owner)
        .map_err(|error| error.to_string())
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
