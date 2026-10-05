use super::*;
use crate::entry::RuntimeValueDigest;
use crate::task::{
    NeedProducerContractDigest, NeedProducerFamily, NeedProducerSiteDigest, NeedProducerSpec,
    RuntimeTypeSemanticDigest, TaskPlanSemanticDigest,
};

fn producer_spec(seed: u8) -> NeedProducerSpec {
    NeedProducerSpec::new(
        NeedProducerFamily::StructuredTaskPlan,
        NeedProducerContractDigest::from_bytes([seed; 32]),
        TaskPlanSemanticDigest::from_bytes([2; 32]),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        RuntimeTypeSemanticDigest::from_bytes([4; 32]),
        RuntimeValueDigest::from_bytes([5; 32]),
    )
}

fn producer(seed: u8) -> NeedProducerInstanceKey {
    producer_spec(seed)
        .instance_key()
        .expect("complete producer instance")
}

#[test]
fn fixed_task_identity_policy_and_ordinal_admission_is_exact() {
    for byte in 0..=u8::MAX {
        match byte {
            0 | 1 => {
                let policy = TaskPolicy::from_semantic_tag(byte).unwrap();
                assert_eq!(policy.semantic_tag(), byte);
            }
            _ => assert_eq!(
                TaskPolicy::from_semantic_tag(byte),
                Err(TaskIdentityError::UnknownPolicy(byte))
            ),
        }
    }
    for ordinal in [0, 1, u64::MAX] {
        for policy in [TaskPolicy::JoinSameKey, TaskPolicy::AlwaysStart] {
            let result = TaskLaunchOrdinal::try_for_policy(policy, ordinal);
            match (policy, ordinal) {
                (TaskPolicy::JoinSameKey, 0) | (TaskPolicy::AlwaysStart, 1..) => {
                    assert_eq!(result.unwrap().get(), ordinal);
                }
                (TaskPolicy::JoinSameKey, _) => {
                    assert_eq!(result, Err(TaskIdentityError::NonZeroJoinOrdinal));
                }
                (TaskPolicy::AlwaysStart, 0) => {
                    assert_eq!(result, Err(TaskIdentityError::ZeroAlwaysStartOrdinal));
                }
            }
        }
    }
    assert_eq!(
        NeedId::try_for(
            producer(1),
            TaskPolicy::AlwaysStart,
            TaskLaunchOrdinal::JOIN
        ),
        Err(TaskIdentityError::ZeroAlwaysStartOrdinal)
    );
    let restored_ordinal = serde_json::from_str::<TaskLaunchOrdinal>("1").unwrap();
    assert_eq!(
        NeedId::try_for(producer(1), TaskPolicy::JoinSameKey, restored_ordinal),
        Err(TaskIdentityError::NonZeroJoinOrdinal)
    );
}

#[test]
fn fixed_task_identity_generation_and_launch_truth_table() {
    let producer = producer(1);
    let join = TaskPolicy::JoinSameKey;
    let start = TaskPolicy::AlwaysStart;
    let first = TaskLaunchOrdinal::try_for_policy(start, 1).unwrap();
    let second = TaskLaunchOrdinal::try_for_policy(start, 2).unwrap();
    let need = NeedId::try_for(producer, start, first).unwrap();
    let key0 = TaskKey::try_for(GenerationId::new(0), producer, start).unwrap();
    let key1 = TaskKey::try_for(GenerationId::new(1), producer, start).unwrap();
    assert_eq!(need, NeedId::try_for(producer, start, first).unwrap());
    assert_ne!(need, NeedId::try_for(producer, start, second).unwrap());
    assert_ne!(
        need,
        NeedId::try_for(producer, join, TaskLaunchOrdinal::JOIN).unwrap()
    );
    assert_ne!(key0, key1);
    assert_ne!(
        key0,
        TaskKey::try_for(GenerationId::new(0), producer, join).unwrap()
    );
    assert_ne!(
        TaskId::try_for(key0, first).unwrap(),
        TaskId::try_for(key1, first).unwrap()
    );
    assert_ne!(
        TaskId::try_for(key0, first).unwrap(),
        TaskId::try_for(key0, second).unwrap()
    );
    assert_ne!(
        need.as_bytes(),
        TaskId::try_for(key0, first).unwrap().as_bytes()
    );
    for generation in [GenerationId::new(0), GenerationId::new(1)] {
        let key = TaskKey::try_for(generation, producer, join).unwrap();
        assert_eq!(key, TaskKey::try_for(generation, producer, join).unwrap());
        assert_eq!(
            TaskId::try_for(key, TaskLaunchOrdinal::JOIN).unwrap(),
            TaskId::try_for(key, TaskLaunchOrdinal::JOIN).unwrap()
        );
    }
}

#[test]
fn fixed_task_identity_uses_exact_version_one_transcripts() {
    let producer = producer(1);
    let generation = GenerationId::new(0x0102_0304_0506_0708);
    let ordinal =
        TaskLaunchOrdinal::try_for_policy(TaskPolicy::AlwaysStart, 0x090a_0b0c_0d0e_0f10).unwrap();
    // Independent transcript assembly catches domain/field-order/endianness
    // mistakes and accidental generation or launch fields in the wrong owner.
    let need_bytes = [
        b"arcweft.need.id.v1\0".as_slice(),
        producer.as_bytes(),
        &[1],
        &ordinal.get().to_le_bytes(),
    ]
    .concat();
    let key_bytes = [
        b"arcweft.task.key.v1\0".as_slice(),
        &generation.get().to_le_bytes(),
        producer.as_bytes(),
        &[1],
    ]
    .concat();
    let key = TaskKey::try_for(generation, producer, TaskPolicy::AlwaysStart).unwrap();
    let task_bytes = [
        b"arcweft.task.id.v1\0".as_slice(),
        key.as_bytes(),
        &ordinal.get().to_le_bytes(),
    ]
    .concat();
    assert_eq!(
        NeedId::try_for(producer, TaskPolicy::AlwaysStart, ordinal)
            .unwrap()
            .as_bytes(),
        blake3::hash(&need_bytes).as_bytes()
    );
    assert_eq!(key.as_bytes(), blake3::hash(&key_bytes).as_bytes());
    assert_eq!(
        TaskId::try_for(key, ordinal).unwrap().as_bytes(),
        blake3::hash(&task_bytes).as_bytes()
    );
    assert_ne!(
        NeedId::try_for(producer, TaskPolicy::AlwaysStart, ordinal).unwrap(),
        NeedId::try_for(self::producer(2), TaskPolicy::AlwaysStart, ordinal).unwrap()
    );
}

#[test]
fn fixed_task_identity_wire_rejects_zero_and_wrong_shape() {
    let zero = serde_json::to_string(&[0u8; 32]).unwrap();
    assert!(serde_json::from_str::<NeedId>(&zero).is_err());
    assert!(serde_json::from_str::<TaskKey>(&zero).is_err());
    assert!(serde_json::from_str::<TaskId>(&zero).is_err());
    assert!(serde_json::from_str::<NeedProducerInstanceKey>(&zero).is_err());
    assert_eq!(
        NeedProducerInstanceKey::try_from_bytes([0; 32]),
        Err(TaskIdentityError::Zero {
            kind: TaskIdentityKind::NeedProducerInstance,
        })
    );
    for invalid in ["[]", "[1]", "\"need.v1.legacy\"", "null"] {
        assert!(serde_json::from_str::<NeedId>(invalid).is_err());
        assert!(serde_json::from_str::<TaskKey>(invalid).is_err());
        assert!(serde_json::from_str::<TaskId>(invalid).is_err());
        assert!(serde_json::from_str::<NeedProducerInstanceKey>(invalid).is_err());
    }
    assert_eq!(
        NeedId::try_from_bytes([0; 32]),
        Err(TaskIdentityError::Zero {
            kind: TaskIdentityKind::Need
        })
    );
    assert_eq!(
        TaskKey::try_from_bytes([0; 32]),
        Err(TaskIdentityError::Zero {
            kind: TaskIdentityKind::TaskKey
        })
    );
    assert_eq!(
        TaskId::try_from_bytes([0; 32]),
        Err(TaskIdentityError::Zero {
            kind: TaskIdentityKind::Task
        })
    );
    let need = NeedId::try_from_bytes([1; 32]).unwrap();
    let key = TaskKey::try_from_bytes([2; 32]).unwrap();
    let task = TaskId::try_from_bytes([3; 32]).unwrap();
    let instance = producer(1);
    assert_eq!(
        serde_json::from_str::<NeedProducerInstanceKey>(&serde_json::to_string(&instance).unwrap())
            .unwrap(),
        instance
    );
    assert_eq!(
        serde_json::from_str::<NeedId>(&serde_json::to_string(&need).unwrap()).unwrap(),
        need
    );
    assert_eq!(
        serde_json::from_str::<TaskKey>(&serde_json::to_string(&key).unwrap()).unwrap(),
        key
    );
    assert_eq!(
        serde_json::from_str::<TaskId>(&serde_json::to_string(&task).unwrap()).unwrap(),
        task
    );
}

#[test]
fn fixed_task_identity_correlation_revalidates_every_field() {
    let spec = NeedProducerInstance::try_from(&producer_spec(1)).unwrap();
    let generation = GenerationId::new(0);
    let policy = TaskPolicy::AlwaysStart;
    let ordinal = TaskLaunchOrdinal::try_for_policy(policy, 1).unwrap();
    let next_ordinal = TaskLaunchOrdinal::try_for_policy(policy, 2).unwrap();
    let accepted = TaskCorrelation::try_for(generation, &spec, policy, ordinal).unwrap();
    assert_eq!(
        accepted.validate(generation, &spec, policy, ordinal),
        Ok(())
    );
    let rebased = TaskCorrelation::try_for(GenerationId::new(1), &spec, policy, ordinal).unwrap();
    assert_eq!(accepted.need, rebased.need);
    assert_ne!(accepted.task_key, rebased.task_key);
    assert_ne!(accepted.task_id, rebased.task_id);
    for forged in [
        TaskCorrelation {
            generation: rebased.generation,
            ..accepted
        },
        TaskCorrelation {
            producer: producer(2),
            ..accepted
        },
        TaskCorrelation {
            producer_contract: NeedProducerContractDigest::from_bytes([9; 32]),
            ..accepted
        },
        TaskCorrelation {
            need: NeedId::try_from_bytes([9; 32]).unwrap(),
            ..accepted
        },
        TaskCorrelation {
            task_key: rebased.task_key,
            ..accepted
        },
        TaskCorrelation {
            task_id: rebased.task_id,
            ..accepted
        },
        TaskCorrelation {
            launch_ordinal: next_ordinal,
            ..accepted
        },
    ] {
        assert_eq!(
            forged.validate(generation, &spec, policy, ordinal),
            Err(TaskCorrelationError::Mismatch)
        );
    }
    assert_eq!(
        accepted.validate(
            generation,
            &NeedProducerInstance::try_from(&producer_spec(2)).unwrap(),
            policy,
            ordinal
        ),
        Err(TaskCorrelationError::Mismatch)
    );
    assert_eq!(
        accepted.validate(generation, &spec, TaskPolicy::JoinSameKey, ordinal),
        Err(TaskCorrelationError::Identity(
            TaskIdentityError::NonZeroJoinOrdinal
        ))
    );
}
