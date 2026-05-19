use crate::task::*;

#[test]
fn normalizes_task_events_by_replay_stable_keys() {
    let events = vec![
        TaskEvent {
            logical_epoch: LogicalEpoch(1),
            task_id: TaskId("b".to_owned()),
            sequence: TaskSequence(0),
            kind: TaskEventKind::Ready("b".to_owned()),
        },
        TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: TaskId("z".to_owned()),
            sequence: TaskSequence(9),
            kind: TaskEventKind::Ready("z".to_owned()),
        },
        TaskEvent {
            logical_epoch: LogicalEpoch(1),
            task_id: TaskId("a".to_owned()),
            sequence: TaskSequence(1),
            kind: TaskEventKind::Ready("a".to_owned()),
        },
    ];

    let normalized = normalize_task_events(events);
    let keys: Vec<_> = normalized
        .iter()
        .map(|event| {
            (
                event.logical_epoch,
                event.task_id.0.as_str(),
                event.sequence,
            )
        })
        .collect();
    assert_eq!(
        keys,
        vec![
            (LogicalEpoch(0), "z", TaskSequence(9)),
            (LogicalEpoch(1), "a", TaskSequence(1)),
            (LogicalEpoch(1), "b", TaskSequence(0)),
        ]
    );
}
