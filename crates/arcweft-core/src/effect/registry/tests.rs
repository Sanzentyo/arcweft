use std::sync::Arc;

use arcweft_id::{DeclarationName, LifetimeScopeKind};

use super::*;
use crate::awbc::schema::{AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcTypeId};
use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
use crate::plan::{RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed};

mod journal;

fn semantic(index: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([index; 32])
}

fn owners() -> [RuntimeProgramOwner; 2] {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(semantic(1), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(semantic(2), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    semantic(3),
                    RuntimePlanTypeProjection::Tuple(Box::new([semantic(1), semantic(2)])),
                ),
            ],
            [],
        )
        .unwrap();
    let awbc = AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::new(semantic(1), AwbcRuntimeTypeShape::Bool),
            AwbcRuntimeType::new(semantic(2), AwbcRuntimeTypeShape::String),
            AwbcRuntimeType::new(
                semantic(3),
                AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0), AwbcTypeId(1)]),
            ),
        ],
        ..AwbcProgram::default()
    };
    [
        RuntimeProgramOwner::Plan(Arc::new(builder.finish().unwrap())),
        RuntimeProgramOwner::Awbc(Arc::new(awbc)),
    ]
}

fn key(scope: &str, segments: &[&str]) -> LifetimeRegistryKey {
    LifetimeRegistryKey::new(
        LifetimeScopeKind::parse(scope).unwrap(),
        segments
            .iter()
            .map(|segment| DeclarationName::try_new(*segment).unwrap())
            .collect(),
    )
}

fn admission(owner: &RuntimeProgramOwner) -> RuntimeRetainedValueAdmission {
    RuntimeRetainedValueAdmission::for_program(owner.clone(), RuntimeSchemaLimits::engine_default())
}

#[test]
fn registry_update_replays_typed_data_under_both_original_executables() {
    let value = RuntimeValue::Tuple(vec![
        RuntimeValue::Bool(true),
        RuntimeValue::String("既読\0🙂".to_owned()),
    ]);
    for owner in owners() {
        for scope in [
            "frame",
            "tick",
            "cue",
            "line",
            "scene",
            "flow",
            "session",
            "global",
            "persistent",
            "章",
        ] {
            for key in [key(scope, &[]), key(scope, &["旗", "既読"])] {
                let packet = RuntimeRegistryStateUpdate::try_copy(
                    &key,
                    semantic(3),
                    None,
                    &value,
                    &mut admission(&owner),
                )
                .unwrap();
                assert_eq!(packet.key(), &key);
                assert_eq!(packet.value_type(), semantic(3));
                assert_eq!(packet.required_effect(), EffectId::state_write(&key));
                assert_eq!(packet.materialize(&mut admission(&owner)).unwrap(), value);
                // Replay data neither consumes nor mutates the original packet.
                assert_eq!(packet.materialize(&mut admission(&owner)).unwrap(), value);
            }
        }
    }
}

#[test]
fn registry_update_rejects_other_programs_and_retains_the_first_failure() {
    for (owner, foreign) in owners().into_iter().zip(owners()) {
        let packet = RuntimeRegistryStateUpdate::try_copy(
            &key("flow", &["seen"]),
            semantic(1),
            None,
            &RuntimeValue::Bool(true),
            &mut admission(&owner),
        )
        .unwrap();
        let mut foreign_admission = admission(&foreign);
        let error = packet.materialize(&mut foreign_admission).unwrap_err();
        assert_eq!(
            error,
            AwbcRuntimeValueSnapshotError::ProgramOwnerMismatch {
                expected: semantic(1)
            }
        );
        assert_eq!(foreign_admission.validate(&RuntimeValue::Unit), Err(error));
        let mut standalone =
            RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits::engine_default());
        let error = packet.materialize(&mut standalone).unwrap_err();
        assert_eq!(
            error,
            AwbcRuntimeValueSnapshotError::ProgramOwnerRequired {
                expected: semantic(1)
            }
        );
        assert_eq!(standalone.validate(&RuntimeValue::Unit), Err(error));
        assert_eq!(
            packet.materialize(&mut admission(&owner)).unwrap(),
            RuntimeValue::Bool(true)
        );
    }
}

#[test]
fn registry_update_checks_original_type_before_publishing_any_packet() {
    for owner in owners() {
        for (expected, value) in [
            (semantic(1), RuntimeValue::String("wrong type".to_owned())),
            (semantic(99), RuntimeValue::Bool(true)),
        ] {
            let mut bank = admission(&owner);
            let error = RuntimeRegistryStateUpdate::try_copy(
                &key("flow", &["seen"]),
                expected,
                None,
                &value,
                &mut bank,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                AwbcRuntimeValueSnapshotError::ProgramType(_)
            ));
            assert_eq!(bank.validate(&RuntimeValue::Unit), Err(error));
        }
    }
}

#[test]
fn registry_update_cannot_copy_nested_affine_custody_into_replay_data() {
    for owner in owners() {
        let value = RuntimeValue::Tuple(vec![
            RuntimeValue::Bool(true),
            RuntimeValue::NeedHandle(crate::tests::reusable_need("registry-update")),
        ]);
        let mut bank = admission(&owner);
        let error = RuntimeRegistryStateUpdate::try_copy(
            &key("flow", &["seen"]),
            semantic(3),
            None,
            &value,
            &mut bank,
        )
        .unwrap_err();
        assert_eq!(error, AwbcRuntimeValueSnapshotError::AffineValue);
        assert_eq!(bank.validate(&RuntimeValue::Unit), Err(error));
        assert!(
            matches!(&value, RuntimeValue::Tuple(values) if matches!(values[1], RuntimeValue::NeedHandle(_)))
        );
    }
}

#[test]
fn registry_updates_share_one_value_budget_for_capture_and_replay_groups() {
    for owner in owners() {
        let limits = RuntimeSchemaLimits {
            max_nodes: 1,
            ..RuntimeSchemaLimits::engine_default()
        };
        let key = key("flow", &["seen"]);
        let mut bank = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        let packet = RuntimeRegistryStateUpdate::try_copy(
            &key,
            semantic(1),
            None,
            &RuntimeValue::Bool(true),
            &mut bank,
        )
        .unwrap();
        let error = RuntimeRegistryStateUpdate::try_copy(
            &key,
            semantic(1),
            None,
            &RuntimeValue::Bool(false),
            &mut bank,
        )
        .unwrap_err();
        assert_eq!(
            error,
            AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
                budget: "nodes"
            })
        );
        assert_eq!(packet.materialize(&mut bank), Err(error.clone()));
        let mut replay = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        assert_eq!(
            packet.materialize(&mut replay).unwrap(),
            RuntimeValue::Bool(true)
        );
        assert_eq!(packet.materialize(&mut replay), Err(error));
    }
}

#[test]
fn registry_key_metadata_is_bounded_before_value_projection() {
    for owner in owners() {
        for (limits, key, budget) in [
            (
                RuntimeSchemaLimits {
                    max_string_bytes: 4,
                    ..RuntimeSchemaLimits::engine_default()
                },
                key("flow", &["既読"]),
                "string_bytes",
            ),
            (
                RuntimeSchemaLimits {
                    max_sequence_items: 1,
                    ..RuntimeSchemaLimits::engine_default()
                },
                key("flow", &["flags", "seen"]),
                "sequence_items",
            ),
        ] {
            let mut bank = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
            let error = RuntimeRegistryStateUpdate::try_copy(
                &key,
                semantic(1),
                None,
                &RuntimeValue::Bool(true),
                &mut bank,
            )
            .unwrap_err();
            assert_eq!(
                error,
                AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
                    budget
                })
            );
            assert_eq!(bank.validate(&RuntimeValue::Unit), Err(error));
        }
    }
}

#[test]
fn registry_update_refuses_an_unrelated_retained_invocation() {
    for owner in owners() {
        // A Bool row cannot supply a function declaration's effect binding.
        let binding =
            RuntimeFunctionEffectInstantiation::from_source_rows(semantic(1), Box::new([]));
        let mut bank = admission(&owner);
        let error = RuntimeRegistryStateUpdate::try_copy(
            &key("flow", &["seen"]),
            semantic(1),
            Some(&binding),
            &RuntimeValue::Bool(true),
            &mut bank,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AwbcRuntimeValueSnapshotError::ProgramType(_)
        ));
        assert_eq!(bank.validate(&RuntimeValue::Unit), Err(error));
    }
}

#[test]
fn registry_update_key_bounds_are_inclusive_and_precede_unknown_type_resolution() {
    for owner in owners() {
        let limits = RuntimeSchemaLimits {
            max_string_bytes: 4,
            max_sequence_items: 1,
            ..RuntimeSchemaLimits::engine_default()
        };
        let mut exact = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        RuntimeRegistryStateUpdate::try_copy(
            &key("flow", &["seen"]),
            semantic(1),
            None,
            &RuntimeValue::Bool(true),
            &mut exact,
        )
        .unwrap();
        let mut bank = RuntimeRetainedValueAdmission::for_program(owner, limits);
        let error = RuntimeRegistryStateUpdate::try_copy(
            &key("flow", &["flags"]),
            semantic(99),
            None,
            &RuntimeValue::Bool(true),
            &mut bank,
        )
        .unwrap_err();
        assert_eq!(
            error,
            AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
                budget: "string_bytes"
            })
        );
        assert_eq!(bank.validate(&RuntimeValue::Unit), Err(error));
    }
}

#[test]
fn registry_update_header_and_values_share_exact_encoded_byte_limits() {
    for owner in owners() {
        // Canonical length-prefixed UTF-8 names, count, 32-byte type, optional
        // binding tag, and the two-byte Bool value. Unicode counts UTF-8 bytes.
        for (key, bytes) in [
            (key("flow", &["seen"]), 46),
            (key("persistent", &[]), 47),
            (key("章", &["旗"]), 44),
        ] {
            let limits = RuntimeSchemaLimits {
                max_encoded_bytes: bytes,
                ..RuntimeSchemaLimits::engine_default()
            };
            let packet = RuntimeRegistryStateUpdate::try_copy(
                &key,
                semantic(1),
                None,
                &RuntimeValue::Bool(true),
                &mut RuntimeRetainedValueAdmission::for_program(owner.clone(), limits),
            )
            .unwrap();
            assert_eq!(
                packet
                    .materialize(&mut RuntimeRetainedValueAdmission::for_program(
                        owner.clone(),
                        limits,
                    ))
                    .unwrap(),
                RuntimeValue::Bool(true)
            );
            let short = RuntimeSchemaLimits {
                max_encoded_bytes: bytes - 1,
                ..limits
            };
            let error =
                AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
                    budget: "encoded_bytes",
                });
            let mut capture = RuntimeRetainedValueAdmission::for_program(owner.clone(), short);
            assert_eq!(
                RuntimeRegistryStateUpdate::try_copy(
                    &key,
                    semantic(1),
                    None,
                    &RuntimeValue::Bool(false),
                    &mut capture,
                )
                .unwrap_err(),
                error
            );
            assert_eq!(capture.validate(&RuntimeValue::Unit), Err(error.clone()));
            let mut replay = RuntimeRetainedValueAdmission::for_program(owner.clone(), short);
            assert_eq!(packet.materialize(&mut replay), Err(error.clone()));
            assert_eq!(replay.validate(&RuntimeValue::Unit), Err(error.clone()));

            let pair = RuntimeSchemaLimits {
                max_encoded_bytes: 2 * bytes,
                ..limits
            };
            let mut bank = RuntimeRetainedValueAdmission::for_program(owner.clone(), pair);
            for _ in 0..2 {
                RuntimeRegistryStateUpdate::try_copy(
                    &key,
                    semantic(1),
                    None,
                    &RuntimeValue::Bool(true),
                    &mut bank,
                )
                .unwrap();
            }
            assert_eq!(
                RuntimeRegistryStateUpdate::try_copy(
                    &key,
                    semantic(1),
                    None,
                    &RuntimeValue::Bool(true),
                    &mut bank,
                )
                .unwrap_err(),
                error
            );
            let mut bank = RuntimeRetainedValueAdmission::for_program(owner.clone(), pair);
            for _ in 0..2 {
                assert_eq!(
                    packet.materialize(&mut bank).unwrap(),
                    RuntimeValue::Bool(true)
                );
            }
            assert_eq!(packet.materialize(&mut bank), Err(error));
        }
    }
}

#[test]
fn registry_update_bounds_binding_header_before_its_original_type_relation() {
    let binding = RuntimeFunctionEffectInstantiation::from_source_rows(
        semantic(1),
        Box::new([std::iter::once(EffectId::parse("log.write").unwrap()).collect()]),
    );
    for owner in owners() {
        for bytes in [87, 90] {
            let limits = RuntimeSchemaLimits {
                max_encoded_bytes: bytes,
                ..RuntimeSchemaLimits::engine_default()
            };
            let mut bank = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
            let error = RuntimeRegistryStateUpdate::try_copy(
                &key("flow", &["seen"]),
                semantic(1),
                Some(&binding),
                &RuntimeValue::Bool(true),
                &mut bank,
            )
            .unwrap_err();
            if bytes == 87 {
                assert_eq!(
                    error,
                    AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
                        budget: "encoded_bytes"
                    },)
                );
            } else {
                // All 88 header + 2 value bytes fit, but Bool is not the original
                // function declaration required by this retained invocation.
                assert!(matches!(
                    error,
                    AwbcRuntimeValueSnapshotError::ProgramType(_)
                ));
            }
            assert_eq!(bank.validate(&RuntimeValue::Unit), Err(error));
        }
    }
}
