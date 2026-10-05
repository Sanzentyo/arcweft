//! Sans-I/O launch admission. The complete bound start specification is the
//! input; one journal alone issues ordinals and exact task/Need correlations.

use super::{
    GenerationId, NeedProducerInstanceKey, TaskEnsureError, TaskHandle, TaskId, TaskKey,
    TaskLaunchOrdinal, TaskPolicy, TaskSpec,
};
use std::collections::BTreeMap;

/// Accepted work transported to an executor. The receipt is validated against
/// the exact start specification before either is exposed to the backend.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskSubmission {
    spec: TaskSpec,
    handle: TaskHandle,
}

impl TaskSubmission {
    pub(crate) fn try_from_accepted(
        spec: TaskSpec,
        handle: TaskHandle,
    ) -> Result<Self, TaskEnsureError> {
        spec.validate_outcome()?;
        let expected = spec.correlation(handle.correlation.launch_ordinal)?;
        if expected != handle.correlation {
            return Err(TaskEnsureError::TaskIdSpecificationConflict {
                task_id: handle.correlation.task_id,
            });
        }
        Ok(Self { spec, handle })
    }

    pub const fn spec(&self) -> &TaskSpec {
        &self.spec
    }
    pub const fn handle(&self) -> TaskHandle {
        self.handle
    }
    pub const fn task_id(&self) -> TaskId {
        self.handle.correlation.task_id
    }
    pub const fn task_key(&self) -> TaskKey {
        self.handle.correlation.task_key
    }
    pub fn into_parts(self) -> (TaskSpec, TaskHandle) {
        (self.spec, self.handle)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TaskAdmission {
    spec: TaskSpec,
    handle: TaskHandle,
}

pub(super) struct PreparedTaskAdmission {
    spec: TaskSpec,
    handle: TaskHandle,
    following: Option<u64>,
    existing: bool,
}

struct TaskAdmissionTransaction<'a> {
    journal: &'a mut TaskAdmissionJournal,
    handle: TaskHandle,
    inserted: bool,
    joined: bool,
    previous_ordinal: Option<u64>,
    committed: bool,
}

impl Drop for TaskAdmissionTransaction<'_> {
    fn drop(&mut self) {
        if self.committed || !self.inserted {
            return;
        }
        let correlation = self.handle.correlation;
        self.journal.accepted.remove(&correlation.task_id);
        if self.joined {
            self.journal.joined.remove(&correlation.task_key);
        } else {
            let key = (correlation.generation, correlation.producer);
            match self.previous_ordinal {
                Some(previous) => {
                    self.journal.next_ordinal.insert(key, previous);
                }
                None => {
                    self.journal.next_ordinal.remove(&key);
                }
            }
        }
    }
}

impl PreparedTaskAdmission {
    pub(super) const fn correlation(&self) -> super::TaskCorrelation {
        self.handle.correlation
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskAdmissionJournal {
    accepted: BTreeMap<TaskId, TaskAdmission>,
    joined: BTreeMap<TaskKey, TaskId>,
    next_ordinal: BTreeMap<(GenerationId, NeedProducerInstanceKey), u64>,
}

impl TaskAdmissionJournal {
    pub(super) fn submissions(&self) -> impl Iterator<Item = TaskSubmission> + '_ {
        self.accepted.values().map(|row| TaskSubmission {
            spec: row.spec.clone(),
            handle: row.handle,
        })
    }

    pub(super) fn validate_submissions(
        submissions: &[TaskSubmission],
    ) -> Result<BTreeMap<(GenerationId, NeedProducerInstanceKey), u64>, TaskEnsureError> {
        let mut tasks = std::collections::BTreeSet::new();
        let mut joins = std::collections::BTreeSet::new();
        let mut ordinals: BTreeMap<_, std::collections::BTreeSet<u64>> = BTreeMap::new();
        for submission in submissions {
            let spec = submission.spec();
            let correlation = submission.handle().correlation;
            spec.validate_outcome()?;
            if spec.correlation(correlation.launch_ordinal)? != correlation
                || !tasks.insert(correlation.task_id)
            {
                return Err(TaskEnsureError::TaskIdSpecificationConflict {
                    task_id: correlation.task_id,
                });
            }
            if spec.policy == TaskPolicy::JoinSameKey {
                if !joins.insert(correlation.task_key) {
                    return Err(TaskEnsureError::TaskIdSpecificationConflict {
                        task_id: correlation.task_id,
                    });
                }
            } else {
                ordinals
                    .entry((spec.generation, spec.producer.key()))
                    .or_default()
                    .insert(correlation.launch_ordinal.get());
            }
        }
        ordinals
            .into_iter()
            .map(|(key, values)| {
                let mut next = 1_u64;
                for ordinal in values {
                    if ordinal != next {
                        return Err(TaskEnsureError::LaunchOrdinalExhausted);
                    }
                    next = next
                        .checked_add(1)
                        .ok_or(TaskEnsureError::LaunchOrdinalExhausted)?;
                }
                Ok((key, next))
            })
            .collect()
    }
    /// Makes the receipt available only inside an accepted journal transaction.
    /// A failed owner publication, including unwinding, restores its frontier.
    pub(super) fn with_admission<T, E>(
        &mut self,
        prepared: PreparedTaskAdmission,
        publish: impl FnOnce(TaskHandle) -> Result<T, E>,
    ) -> Result<T, E> {
        let inserted = !prepared.existing;
        let joined = prepared.spec.policy == TaskPolicy::JoinSameKey;
        let correlation = prepared.handle.correlation;
        let previous_ordinal = self
            .next_ordinal
            .get(&(correlation.generation, correlation.producer))
            .copied();
        let handle = self.commit_task(prepared);
        let mut transaction = TaskAdmissionTransaction {
            journal: self,
            handle,
            inserted,
            joined,
            previous_ordinal,
            committed: false,
        };
        let result = publish(handle)?;
        transaction.committed = true;
        Ok(result)
    }
    /// Every rejection precedes journal mutation and ordinal advancement.
    /// Join is ordinal zero; AlwaysStart takes the next journal-owned ordinal.
    pub fn ensure_task(&mut self, spec: TaskSpec) -> Result<TaskHandle, TaskEnsureError> {
        let prepared = self.inspect_task(spec)?;
        Ok(self.commit_task(prepared))
    }

    pub(super) fn inspect_task(
        &self,
        spec: TaskSpec,
    ) -> Result<PreparedTaskAdmission, TaskEnsureError> {
        spec.validate_outcome()?;
        let input = &spec;
        let key = TaskKey::try_for(input.generation, input.producer.key(), input.policy)?;
        if input.policy == TaskPolicy::JoinSameKey
            && let Some(owner) = self.joined.get(&key)
        {
            let admission = self
                .accepted
                .get(owner)
                .expect("joined journal row has an owner");
            if !admission.spec.same_join_contract(&spec) {
                return Err(TaskEnsureError::JoinSpecificationConflict {
                    task_id: admission.handle.correlation.task_id,
                    owner_id: *owner,
                    key,
                });
            }
            return Ok(PreparedTaskAdmission {
                spec,
                handle: admission.handle,
                following: None,
                existing: true,
            });
        }
        let counter = (input.generation, input.producer.key());
        let (ordinal, following) = match input.policy {
            TaskPolicy::JoinSameKey => (TaskLaunchOrdinal::JOIN, None),
            TaskPolicy::AlwaysStart => {
                let value = self.next_ordinal.get(&counter).copied().unwrap_or(1);
                let following = value
                    .checked_add(1)
                    .ok_or(TaskEnsureError::LaunchOrdinalExhausted)?;
                (
                    TaskLaunchOrdinal::try_for_policy(input.policy, value)?,
                    Some(following),
                )
            }
        };
        let correlation = input.correlation(ordinal)?;
        let handle = TaskHandle { correlation };
        if self.accepted.contains_key(&correlation.task_id) {
            return Err(TaskEnsureError::TaskIdSpecificationConflict {
                task_id: correlation.task_id,
            });
        }
        Ok(PreparedTaskAdmission {
            spec,
            handle,
            following,
            existing: false,
        })
    }

    pub(super) fn commit_task(&mut self, prepared: PreparedTaskAdmission) -> TaskHandle {
        let PreparedTaskAdmission {
            spec,
            handle,
            following,
            existing,
        } = prepared;
        let correlation = handle.correlation;
        if existing {
            assert!(
                self.accepted_spec(handle)
                    .is_some_and(|accepted| accepted.same_join_contract(&spec))
            );
            return handle;
        }
        if let Some(following) = following {
            let counter = (spec.generation, spec.producer.key());
            assert_eq!(
                self.next_ordinal(counter.0, counter.1),
                correlation.launch_ordinal.get()
            );
            self.next_ordinal.insert(counter, following);
        }
        if spec.policy == TaskPolicy::JoinSameKey {
            assert!(
                self.joined
                    .insert(correlation.task_key, correlation.task_id)
                    .is_none()
            );
        }
        assert!(
            self.accepted
                .insert(correlation.task_id, TaskAdmission { spec, handle })
                .is_none()
        );
        handle
    }

    pub(super) fn restore_accepted(
        &mut self,
        spec: TaskSpec,
        handle: TaskHandle,
    ) -> Result<(), TaskEnsureError> {
        let submission = TaskSubmission::try_from_accepted(spec, handle)?;
        let (spec, handle) = submission.into_parts();
        let correlation = handle.correlation;
        if self.accepted.contains_key(&correlation.task_id)
            || (spec.policy == TaskPolicy::JoinSameKey
                && self.joined.contains_key(&correlation.task_key))
        {
            return Err(TaskEnsureError::TaskIdSpecificationConflict {
                task_id: correlation.task_id,
            });
        }
        let following = if spec.policy == TaskPolicy::AlwaysStart {
            Some(
                correlation
                    .launch_ordinal
                    .get()
                    .checked_add(1)
                    .ok_or(TaskEnsureError::LaunchOrdinalExhausted)?,
            )
        } else {
            None
        };
        if let Some(following) = following {
            let entry = self
                .next_ordinal
                .entry((spec.generation, spec.producer.key()))
                .or_insert(1);
            *entry = (*entry).max(following);
        }
        if spec.policy == TaskPolicy::JoinSameKey {
            self.joined
                .insert(correlation.task_key, correlation.task_id);
        }
        self.accepted
            .insert(correlation.task_id, TaskAdmission { spec, handle });
        Ok(())
    }

    pub(super) fn next_ordinal(
        &self,
        generation: GenerationId,
        producer: NeedProducerInstanceKey,
    ) -> u64 {
        self.next_ordinal
            .get(&(generation, producer))
            .copied()
            .unwrap_or(1)
    }

    pub(super) fn frontiers(
        &self,
    ) -> impl Iterator<Item = ((GenerationId, NeedProducerInstanceKey), u64)> + '_ {
        self.next_ordinal.iter().map(|(key, value)| (*key, *value))
    }

    pub(super) fn is_empty(&self) -> bool {
        self.accepted.is_empty() && self.joined.is_empty() && self.next_ordinal.is_empty()
    }

    pub fn accepted_spec(&self, handle: TaskHandle) -> Option<&TaskSpec> {
        self.accepted
            .get(&handle.correlation.task_id)
            .filter(|entry| entry.handle == handle)
            .map(|entry| &entry.spec)
    }

    pub fn submission(&self, handle: TaskHandle) -> Option<TaskSubmission> {
        self.accepted_spec(handle).map(|entry| TaskSubmission {
            spec: entry.clone(),
            handle,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::RuntimeCheckedType;
    use crate::task::{
        CancelScopeId, HostTaskRequest, NeedProducerContractDigest, NeedProducerFamily,
        NeedProducerInstance, NeedProducerSiteDigest, NeedProducerSpec, RuntimeTypeSemanticDigest,
        TaskClass, TaskOutcomeContract, TaskPlanSemanticDigest, TaskPriority,
    };
    use crate::value::RuntimeValue;

    fn specification(policy: TaskPolicy) -> TaskSpec {
        let input = NeedProducerSpec::new(
            NeedProducerFamily::HostAdapterTask,
            NeedProducerContractDigest::from_bytes([1; 32]),
            TaskPlanSemanticDigest::from_bytes([2; 32]),
            NeedProducerSiteDigest::from_bytes([3; 32]),
            RuntimeTypeSemanticDigest::from_bytes(
                *RuntimeCheckedType::Bool
                    .semantic_identity_digest()
                    .as_bytes(),
            ),
            RuntimeValue::Tuple(Vec::new()).try_digest(1024).unwrap(),
        );
        TaskSpec {
            generation: GenerationId::new(0),
            producer: NeedProducerInstance::try_from(&input).unwrap(),
            class: TaskClass::Cpu,
            priority: TaskPriority(0),
            cancel_scope: CancelScopeId("fixture".into()),
            policy,
            outcome: TaskOutcomeContract::new(RuntimeCheckedType::Bool),
            request: HostTaskRequest::custom("fixture", "run", []),
            debug_label: "diagnostic".into(),
        }
    }

    #[test]
    fn rejected_publication_preserves_ordinal_and_accepted_rows() {
        let mut journal = TaskAdmissionJournal::default();
        let spec = specification(TaskPolicy::AlwaysStart);
        let before = journal.clone();
        let prepared = journal.inspect_task(spec.clone()).unwrap();
        assert_eq!(prepared.correlation().launch_ordinal.get(), 1);
        assert_eq!(
            journal.with_admission(prepared, |_| Err::<(), _>("binding rejected")),
            Err("binding rejected")
        );
        assert_eq!(journal, before);
        let first = journal.ensure_task(spec.clone()).unwrap();
        let second = journal.ensure_task(spec).unwrap();
        assert_eq!(first.correlation.launch_ordinal.get(), 1);
        assert_eq!(second.correlation.launch_ordinal.get(), 2);
        assert_ne!(first.correlation.task_id, second.correlation.task_id);
    }

    #[test]
    fn join_compares_full_contract_before_mutation_and_ignores_diagnostic_text() {
        let mut journal = TaskAdmissionJournal::default();
        let spec = specification(TaskPolicy::JoinSameKey);
        let first = journal.ensure_task(spec.clone()).unwrap();
        let mut diagnostic = spec.clone();
        diagnostic.debug_label = "other diagnostic".into();
        assert_eq!(journal.ensure_task(diagnostic).unwrap(), first);
        let before = journal.clone();
        let mut conflicting = spec;
        conflicting.priority = TaskPriority(7);
        assert!(matches!(
            journal.ensure_task(conflicting),
            Err(TaskEnsureError::JoinSpecificationConflict { .. })
        ));
        assert_eq!(journal, before);
        assert_eq!(first.correlation.launch_ordinal, TaskLaunchOrdinal::JOIN);
    }

    #[test]
    fn unwinding_publication_restores_journal_frontier() {
        let mut journal = TaskAdmissionJournal::default();
        let spec = specification(TaskPolicy::AlwaysStart);
        let prepared = journal.inspect_task(spec.clone()).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            journal.with_admission(prepared, |_| -> Result<(), ()> {
                panic!("publication panic")
            })
        }));
        assert!(result.is_err());
        assert!(journal.is_empty());
        assert_eq!(
            journal
                .ensure_task(spec)
                .unwrap()
                .correlation
                .launch_ordinal
                .get(),
            1
        );
    }
}
