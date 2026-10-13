use std::sync::Arc;

use super::*;
use crate::{
    awbc::schema::{
        AwbcProgram, AwbcRecordField, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcStringId,
        AwbcTypeId, AwbcVariantCase, AwbcVariantIdentity,
    },
    entry::{
        RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
        RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
        RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema,
        TypeLayoutHash,
    },
    pattern::RuntimeVariantIdentity,
    plan::{
        RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlanBuilder,
        RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimeVariantCaseSeed,
        RuntimeVariantDomainSeed,
    },
    value::RuntimeRecordFieldId,
};

#[test]
fn standalone_snapshot_uses_the_shared_decoder_and_rejects_program_and_affine_leaves() {
    let mut deep = RuntimeValue::Bool(true);
    for _ in 0..=super::super::MAX_RUNTIME_VALUE_NESTING_DEPTH {
        deep = RuntimeValue::Tuple(vec![deep]);
    }
    assert!(matches!(
        AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(&deep),
        Err(AwbcRuntimeValueSnapshotError::Nesting(_))
    ));
    let value = RuntimeValue::Tuple(vec![
        RuntimeValue::Bool(true),
        RuntimeValue::String("retained".to_owned()),
    ]);
    let snapshot = AwbcRuntimeValueSnapshot::from_unrestricted_standalone_value(&value).unwrap();
    let encoded = serde_json::to_vec(&snapshot).unwrap();
    let decoded: AwbcRuntimeValueSnapshot = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.into_unrestricted_standalone_value().unwrap(), value);

    let callable = AwbcRuntimeValueSnapshot::Callable(AwbcRuntimeCallableSnapshot {
        state: RuntimeCallableStateId::from_zero_based(0).unwrap(),
        retained: vec![],
    });
    assert!(
        AwbcRuntimeValueSnapshot::Tuple(vec![callable])
            .into_unrestricted_standalone_value()
            .is_err()
    );
    let affine = AwbcRuntimeValueSnapshot::from_standalone_value(&RuntimeValue::NeedHandle(
        crate::tests::reusable_need("need.standalone-snapshot"),
    ))
    .unwrap();
    assert_eq!(
        AwbcRuntimeValueSnapshot::Tuple(vec![affine]).into_unrestricted_standalone_value(),
        Err(AwbcRuntimeValueSnapshotError::AffineValue)
    );
}

fn dense_string_graph_fixture() -> (AwbcRuntimeValueSnapshot, RuntimeSchemaLimits) {
    let dense = AwbcRuntimeValueSnapshot::Seq(AwbcRuntimeSeqSnapshot::Dense(DenseSeq::Strings(
        super::super::DenseSeqStorage::new(vec![
            "captured".to_owned(),
            "retained".to_owned(),
            "captured".to_owned(),
        ]),
    )));
    let exact = RuntimeSchemaLimits {
        max_depth: 1,
        max_nodes: 4,
        max_sequence_items: 3,
        max_string_bytes: 8,
        max_validation_work: 5,
        ..RuntimeSchemaLimits::engine_default()
    };
    (dense, exact)
}

#[test]
fn capture_checks_borrowed_dense_resources_before_projection_for_both_program_owners() {
    let value =
        RuntimeValue::MatrixF32(crate::math::DenseMatrixF32::new(1, 2, vec![1.25, 2.5]).unwrap());
    // Tag, numeric type, rank, two dimensions, length and two tagged f32s.
    let exact = RuntimeSchemaLimits {
        max_depth: 1,
        max_nodes: 3,
        max_sequence_items: 2,
        max_validation_work: 3,
        max_encoded_bytes: 17,
        ..RuntimeSchemaLimits::engine_default()
    };
    for owner in owners() {
        let image = AwbcRuntimeValueSnapshot::from_runtime_value_for_program_with_limits(
            &value, &owner, exact,
        )
        .unwrap();
        let encoded = serde_json::to_vec(&image).unwrap();
        let decoded: AwbcRuntimeValueSnapshot = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(
            decoded.into_runtime_value_for_program(&owner).unwrap(),
            value
        );
        for limits in [
            RuntimeSchemaLimits {
                max_depth: 0,
                ..exact
            },
            RuntimeSchemaLimits {
                max_nodes: 2,
                ..exact
            },
            RuntimeSchemaLimits {
                max_sequence_items: 1,
                ..exact
            },
            RuntimeSchemaLimits {
                max_validation_work: 2,
                ..exact
            },
            RuntimeSchemaLimits {
                max_encoded_bytes: 16,
                ..exact
            },
        ] {
            assert!(matches!(
                AwbcRuntimeValueSnapshot::from_runtime_value_for_program_with_limits(
                    &value, &owner, limits,
                ),
                Err(AwbcRuntimeValueSnapshotError::Resource(_))
            ));
        }
    }
}

#[test]
fn inert_restore_checks_encoded_byte_limits_for_both_program_owners() {
    let value =
        RuntimeValue::MatrixF32(crate::math::DenseMatrixF32::new(1, 2, vec![1.25, 2.5]).unwrap());
    let exact = RuntimeSchemaLimits {
        max_encoded_bytes: 17,
        ..RuntimeSchemaLimits::engine_default()
    };
    for owner in owners() {
        let snapshot =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
        assert_eq!(
            snapshot
                .clone()
                .into_runtime_value_for_program_with_limits(&owner, exact)
                .unwrap(),
            value
        );
        for unrestricted in [false, true] {
            let short = RuntimeSchemaLimits {
                max_encoded_bytes: 16,
                ..exact
            };
            let result = if unrestricted {
                snapshot
                    .clone()
                    .into_unrestricted_runtime_value_for_program_with_limits(&owner, short)
            } else {
                snapshot
                    .clone()
                    .into_runtime_value_for_program_with_limits(&owner, short)
            };
            assert!(matches!(
                result,
                Err(AwbcRuntimeValueSnapshotError::Resource(
                    crate::entry::RuntimeSchemaError::BudgetExceeded {
                        budget: "encoded_bytes"
                    }
                ))
            ));
        }
    }
}

#[test]
fn inert_restore_rejects_bytes_before_resolving_a_missing_callable_state() {
    for owner in owners() {
        let snapshot = AwbcRuntimeValueSnapshot::Tuple(vec![AwbcRuntimeValueSnapshot::Callable(
            AwbcRuntimeCallableSnapshot {
                state: RuntimeCallableStateId::from_zero_based(0).unwrap(),
                retained: vec![],
            },
        )]);
        assert!(matches!(
            snapshot.into_runtime_value_for_program_with_limits(
                &owner,
                RuntimeSchemaLimits {
                    max_encoded_bytes: 0,
                    ..RuntimeSchemaLimits::engine_default()
                }
            ),
            Err(AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded {
                    budget: "encoded_bytes"
                }
            ))
        ));
    }
}

#[test]
fn owned_capture_preserves_affine_opaque_custody_and_copyable_capture_rejects_it() {
    let issued = crate::tests::program_custody::issued_program_handle();
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&issued.plan));
    let image =
        AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&issued.value, &owner).unwrap();
    assert_eq!(
        image.affine_line_handle_tokens().unwrap(),
        vec![issued.token.clone()]
    );
    assert_eq!(
        AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(
            &issued.value,
            &owner,
        ),
        Err(AwbcRuntimeValueSnapshotError::AffineValue)
    );
    let restored = image.into_runtime_value_for_program(&owner).unwrap();
    assert_eq!(
        restored.affine_line_handles().unwrap()[0].token(),
        &issued.token
    );
}

#[test]
fn capture_bounds_empty_columns_and_predicates_without_materializing_rows() {
    let columns = RuntimeValue::Seq(RuntimeSeq::TupleColumns(
        TupleSeq::new(
            0,
            vec![RuntimeSeq::TupleColumns(
                TupleSeq::new(0, vec![RuntimeSeq::Values(vec![])]).unwrap(),
            )],
        )
        .unwrap(),
    ));
    let predicate = RuntimeValue::Agent(RuntimeAgentValue::Predicate(RuntimeAgentPredicate::Not {
        predicate: Box::new(RuntimeAgentPredicate::DiagnosticsHasError),
    }));
    for owner in owners() {
        for (value, depth, nodes, work) in [(&columns, 2, 1, 4), (&predicate, 1, 2, 3)] {
            let limits = RuntimeSchemaLimits {
                max_depth: depth,
                max_nodes: nodes,
                max_validation_work: work,
                ..RuntimeSchemaLimits::engine_default()
            };
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program_with_limits(
                value, &owner, limits,
            )
            .unwrap();
            for limited in [
                RuntimeSchemaLimits {
                    max_depth: depth - 1,
                    ..limits
                },
                RuntimeSchemaLimits {
                    max_validation_work: work - 1,
                    ..limits
                },
            ] {
                assert!(matches!(
                    AwbcRuntimeValueSnapshot::from_runtime_value_for_program_with_limits(
                        value, &owner, limited,
                    ),
                    Err(AwbcRuntimeValueSnapshotError::Resource(_))
                ));
            }
        }
    }
}

#[test]
fn capture_checks_retained_effect_metadata_before_program_type_matching() {
    let binding: crate::program_types::RuntimeFunctionEffectInstantiation =
        serde_json::from_value(serde_json::json!({
            "context": semantic(250),
            "effects": [["log.write", "metric.write"]]
        }))
        .unwrap();
    let candidate = RuntimeValue::NominalRecord(
        RuntimeNominalRecordValue::new(
            RuntimeNominalTypeId::try_new("n.T").unwrap(),
            semantic(250),
            TypeLayoutHash::from_bytes([0; 32]),
            vec![],
        )
        .with_type_instantiation(Some(Arc::new(binding))),
    );
    let exact = RuntimeSchemaLimits {
        max_nodes: 1,
        max_depth: 3,
        max_sequence_items: 2,
        max_string_bytes: 12,
        max_validation_work: 5,
        ..RuntimeSchemaLimits::engine_default()
    };
    crate::entry::schema::validate_runtime_snapshot_value(&candidate, exact).unwrap();
    for owner in owners() {
        for limits in [
            RuntimeSchemaLimits {
                max_depth: 2,
                ..exact
            },
            RuntimeSchemaLimits {
                max_sequence_items: 1,
                ..exact
            },
            RuntimeSchemaLimits {
                max_string_bytes: 11,
                ..exact
            },
            RuntimeSchemaLimits {
                max_validation_work: 4,
                ..exact
            },
        ] {
            assert!(matches!(
                AwbcRuntimeValueSnapshot::from_runtime_value_for_program_with_limits(
                    &candidate, &owner, limits,
                ),
                Err(AwbcRuntimeValueSnapshotError::Resource(_))
            ));
        }
        // Resource admission alone cannot publish this unknown nominal header.
        assert!(
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program_with_limits(
                &candidate, &owner, exact,
            )
            .is_err()
        );
    }
}

#[test]
fn direct_iterator_capture_rejects_oversized_descendants_and_preserves_remainder() {
    let limits = RuntimeSchemaLimits::engine_default();
    let oversized = RuntimeIterator::Values {
        items: VecDeque::from([RuntimeValue::String(
            "x".repeat(usize::try_from(limits.max_string_bytes).unwrap() + 1),
        )]),
    };
    for owner in owners() {
        assert!(matches!(
            AwbcRuntimeValueSnapshot::from_runtime_iterator_for_program(&oversized, &owner),
            Err(AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded {
                    budget: "string_bytes"
                }
            ))
        ));
        let mut iterator = RuntimeIterator::Values {
            items: VecDeque::from([RuntimeValue::Bool(false), RuntimeValue::Bool(true)]),
        };
        assert_eq!(iterator.next(), Some(RuntimeValue::Bool(false)));
        let image =
            AwbcRuntimeValueSnapshot::from_runtime_iterator_for_program(&iterator, &owner).unwrap();
        let mut restored = image.into_runtime_iterator_for_program(&owner).unwrap();
        assert_eq!(restored.next(), Some(RuntimeValue::Bool(true)));
        assert_eq!(restored.next(), None);
        assert_eq!(iterator.next(), Some(RuntimeValue::Bool(true)));
    }
}

#[test]
fn inert_graph_limits_cover_dense_columnar_predicate_and_retained_values_before_restore() {
    let defaults = RuntimeSchemaLimits::engine_default();
    let (dense, exact) = dense_string_graph_fixture();
    dense.validate_graph_limits(exact).unwrap();
    for limits in [
        RuntimeSchemaLimits {
            max_depth: 0,
            ..exact
        },
        RuntimeSchemaLimits {
            max_nodes: 3,
            ..exact
        },
        RuntimeSchemaLimits {
            max_sequence_items: 2,
            ..exact
        },
        RuntimeSchemaLimits {
            max_string_bytes: 7,
            ..exact
        },
        RuntimeSchemaLimits {
            max_validation_work: 4,
            ..exact
        },
    ] {
        assert!(matches!(
            dense.validate_graph_limits(limits),
            Err(AwbcRuntimeValueSnapshotError::Resource(_))
        ));
    }
    let columns = AwbcRuntimeValueSnapshot::Seq(AwbcRuntimeSeqSnapshot::TupleColumns {
        len: 2,
        columns: vec![
            AwbcRuntimeSeqSnapshot::Dense(DenseSeq::Units(2)),
            AwbcRuntimeSeqSnapshot::Values(vec![
                AwbcRuntimeValueSnapshot::Bool(true),
                AwbcRuntimeValueSnapshot::Bool(false),
            ]),
        ],
    });
    let column_limits = RuntimeSchemaLimits {
        max_nodes: 7,
        max_depth: 2,
        max_validation_work: 10,
        ..defaults
    };
    columns.validate_graph_limits(column_limits).unwrap();
    assert!(matches!(
        columns.validate_graph_limits(RuntimeSchemaLimits {
            max_nodes: 6,
            ..column_limits
        }),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "nodes" }
        ))
    ));
    let predicate = AwbcRuntimeValueSnapshot::Agent(AwbcRuntimeAgentSnapshot::Predicate(
        AwbcRuntimeAgentPredicateSnapshot::Not {
            predicate: Box::new(AwbcRuntimeAgentPredicateSnapshot::DiagnosticsHasError),
        },
    ));
    predicate
        .validate_graph_limits(RuntimeSchemaLimits {
            max_nodes: 2,
            ..defaults
        })
        .unwrap();
    assert!(
        predicate
            .validate_graph_limits(RuntimeSchemaLimits {
                max_nodes: 1,
                ..defaults
            })
            .is_err()
    );

    for owner in owners() {
        let image = AwbcRuntimeValueSnapshot::Tuple(vec![AwbcRuntimeValueSnapshot::Callable(
            AwbcRuntimeCallableSnapshot {
                state: RuntimeCallableStateId::from_zero_based(999).unwrap(),
                retained: vec![AwbcRuntimeValueSnapshot::String("captured".into())],
            },
        )]);
        let limits = RuntimeSchemaLimits {
            max_nodes: 2,
            ..defaults
        };
        assert!(matches!(
            image.into_runtime_value_for_program_with_limits(&owner, limits),
            Err(AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "nodes" }
            ))
        ));
        let restored = dense
            .clone()
            .into_runtime_value_for_program_with_limits(&owner, exact)
            .unwrap();
        assert!(matches!(restored, RuntimeValue::Seq(value) if value.len() == 3));
    }
}

#[test]
fn engine_iterator_restore_applies_shared_graph_limits_before_private_construction() {
    for owner in owners() {
        let mut deep = AwbcRuntimeValueSnapshot::Bool(true);
        for _ in 0..=super::super::MAX_RUNTIME_VALUE_NESTING_DEPTH {
            deep = AwbcRuntimeValueSnapshot::Tuple(vec![deep]);
        }
        let image = AwbcRuntimeValueSnapshot::Iterator(AwbcRuntimeIteratorSnapshot::Values {
            items: vec![deep],
        });
        assert!(matches!(
            image.into_runtime_iterator_for_program(&owner),
            Err(AwbcRuntimeValueSnapshotError::Nesting(_))
        ));
        let image = AwbcRuntimeValueSnapshot::Iterator(AwbcRuntimeIteratorSnapshot::Values {
            items: vec![AwbcRuntimeValueSnapshot::Bool(true)],
        });
        let bytes = serde_json::to_vec(&image).unwrap();
        let decoded: AwbcRuntimeValueSnapshot = serde_json::from_slice(&bytes).unwrap();
        let mut restored = decoded.into_runtime_iterator_for_program(&owner).unwrap();
        assert_eq!(restored.next(), Some(RuntimeValue::Bool(true)));
        assert_eq!(restored.next(), None);
    }
}

#[test]
fn empty_columnar_metadata_consumes_shared_inert_traversal_work() {
    let image = AwbcRuntimeValueSnapshot::Seq(AwbcRuntimeSeqSnapshot::TupleColumns {
        len: 0,
        columns: vec![AwbcRuntimeSeqSnapshot::TupleColumns {
            len: 0,
            columns: vec![AwbcRuntimeSeqSnapshot::Values(vec![])],
        }],
    });
    let limits = RuntimeSchemaLimits {
        max_nodes: 1,
        max_validation_work: 4,
        ..RuntimeSchemaLimits::engine_default()
    };
    image.validate_graph_limits(limits).unwrap();
    assert!(matches!(
        image.validate_graph_limits(RuntimeSchemaLimits {
            max_validation_work: 3,
            ..limits
        }),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            crate::entry::RuntimeSchemaError::ValidationWork { .. }
        ))
    ));
}

#[test]
fn exhausted_storage_work_rejects_before_reading_the_next_column_header() {
    let image = AwbcRuntimeValueSnapshot::Seq(AwbcRuntimeSeqSnapshot::TupleColumns {
        len: 0,
        columns: vec![
            AwbcRuntimeSeqSnapshot::Values(vec![]),
            AwbcRuntimeSeqSnapshot::Values(vec![AwbcRuntimeValueSnapshot::Bool(true)]),
        ],
    });
    let limits = RuntimeSchemaLimits {
        max_validation_work: 3,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert!(matches!(
        image.validate_graph_limits(limits),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            crate::entry::RuntimeSchemaError::ValidationWork { consumed: 4, .. }
        )),
    ));
    assert!(matches!(
        image.validate_graph_limits(RuntimeSchemaLimits {
            max_validation_work: 4,
            ..limits
        }),
        Err(AwbcRuntimeValueSnapshotError::Resource(
            crate::entry::RuntimeSchemaError::Encoding { .. }
        )),
    ));
}

#[test]
fn retained_effect_instantiations_obey_inert_metadata_limits_before_type_matching() {
    let binding: crate::program_types::RuntimeFunctionEffectInstantiation =
        serde_json::from_value(serde_json::json!({
            "context": semantic(250),
            "effects": [["log.write", "metric.write"]]
        }))
        .unwrap();
    let nominal = RuntimeNominalTypeId::try_new("n.T").unwrap();
    let layout = TypeLayoutHash::from_bytes([0; 32]);
    let images = [
        AwbcRuntimeValueSnapshot::NominalRecord(AwbcRuntimeNominalRecordSnapshot {
            type_id: nominal.clone(),
            semantic_identity: semantic(250),
            layout,
            type_instantiation: Some(binding.clone()),
            fields: Vec::new(),
        }),
        AwbcRuntimeValueSnapshot::Variant {
            owner: RuntimeVariantIdentity::Nominal {
                nominal,
                semantic_identity: semantic(250),
                layout,
            },
            ordinal: 0,
            name: "case".into(),
            type_instantiation: Some(binding),
            payload: None,
        },
    ];
    let exact = RuntimeSchemaLimits {
        max_nodes: 1,
        max_depth: 3,
        max_sequence_items: 2,
        max_string_bytes: 12,
        max_validation_work: 5,
        ..RuntimeSchemaLimits::engine_default()
    };
    for image in images {
        image.validate_graph_limits(exact).unwrap();
        for limits in [
            RuntimeSchemaLimits {
                max_depth: 2,
                ..exact
            },
            RuntimeSchemaLimits {
                max_sequence_items: 1,
                ..exact
            },
            RuntimeSchemaLimits {
                max_validation_work: 4,
                ..exact
            },
            RuntimeSchemaLimits {
                max_string_bytes: 11,
                ..exact
            },
        ] {
            assert!(matches!(
                image.validate_graph_limits(limits),
                Err(AwbcRuntimeValueSnapshotError::Resource(_))
            ));
        }
        for owner in owners() {
            assert!(matches!(
                image.clone().into_runtime_value_for_program_with_limits(
                    &owner,
                    RuntimeSchemaLimits {
                        max_validation_work: 4,
                        ..exact
                    }
                ),
                Err(AwbcRuntimeValueSnapshotError::Resource(
                    crate::entry::RuntimeSchemaError::ValidationWork { .. }
                ))
            ));
        }
    }
}

#[test]
fn unrestricted_restore_rejects_nested_affine_values_before_any_live_constructor() {
    let leaf = AwbcRuntimeValueSnapshot::from_standalone_value(&RuntimeValue::NeedHandle(
        crate::tests::reusable_need("need.inert-preflight"),
    ))
    .unwrap();
    let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
    let opaque = AwbcRuntimeOpaqueSnapshot {
        producer: super::super::RuntimeHandleKind::Cue.try_producer().unwrap(),
        semantic_identity: semantic(99),
        value_class: super::super::RuntimeOpaqueValueClass::AffineHandle(
            super::super::RuntimeHandleKind::Cue,
        ),
        persistence: super::super::RuntimeOpaquePersistence::SnapshotOnly,
        payload: Box::new(AwbcRuntimeValueSnapshot::Unit),
    };
    for owner in owners() {
        let values = [
            leaf.clone(),
            AwbcRuntimeValueSnapshot::Tuple(vec![leaf.clone()]),
            AwbcRuntimeValueSnapshot::Record(vec![AwbcRuntimeFieldSnapshot {
                field,
                name: "child".into(),
                value: leaf.clone(),
            }]),
            AwbcRuntimeValueSnapshot::NominalRecord(AwbcRuntimeNominalRecordSnapshot {
                type_id: RuntimeNominalTypeId::try_new("snapshot.Record").unwrap(),
                semantic_identity: semantic(1),
                layout: TypeLayoutHash::from_bytes([0; 32]),
                type_instantiation: None,
                fields: vec![AwbcRuntimeNominalFieldSnapshot {
                    field,
                    value: leaf.clone(),
                }],
            }),
            AwbcRuntimeValueSnapshot::Seq(AwbcRuntimeSeqSnapshot::Values(vec![leaf.clone()])),
            AwbcRuntimeValueSnapshot::Seq(AwbcRuntimeSeqSnapshot::TupleColumns {
                len: 1,
                columns: vec![AwbcRuntimeSeqSnapshot::Values(vec![leaf.clone()])],
            }),
            AwbcRuntimeValueSnapshot::Seq(AwbcRuntimeSeqSnapshot::RecordColumns {
                len: 1,
                fields: vec![AwbcRuntimeRecordSeqFieldSnapshot {
                    field,
                    name: "child".into(),
                    values: AwbcRuntimeSeqSnapshot::Values(vec![leaf.clone()]),
                }],
            }),
            AwbcRuntimeValueSnapshot::Iterator(AwbcRuntimeIteratorSnapshot::Values {
                items: vec![leaf.clone()],
            }),
            AwbcRuntimeValueSnapshot::Callable(AwbcRuntimeCallableSnapshot {
                state: RuntimeCallableStateId::from_zero_based(0).unwrap(),
                retained: vec![leaf.clone()],
            }),
            AwbcRuntimeValueSnapshot::Agent(AwbcRuntimeAgentSnapshot::Predicate(
                AwbcRuntimeAgentPredicateSnapshot::Compare {
                    probe: RuntimeAgentProbe::Signal {
                        target: RuntimeCommandTargetId::try_new("signal.test").unwrap(),
                    },
                    op: RuntimeAgentCompareOp::Eq,
                    value: Box::new(leaf.clone()),
                },
            )),
            AwbcRuntimeValueSnapshot::Opaque(opaque.clone()),
            AwbcRuntimeValueSnapshot::Tuple(vec![AwbcRuntimeValueSnapshot::Opaque(opaque.clone())]),
        ];
        for value in values {
            assert_eq!(
                value.into_unrestricted_runtime_value_for_program(&owner),
                Err(AwbcRuntimeValueSnapshotError::AffineValue),
                "affine admission must precede nominal/callable/opaque construction"
            );
        }
    }
}

#[test]
fn inert_restore_checks_complete_nesting_before_live_construction() {
    for owner in owners() {
        let mut valid = AwbcRuntimeValueSnapshot::Bool(true);
        for _ in 0..super::super::MAX_RUNTIME_VALUE_NESTING_DEPTH {
            valid = AwbcRuntimeValueSnapshot::Tuple(vec![valid]);
        }
        let restored = valid
            .clone()
            .into_unrestricted_runtime_value_for_program(&owner)
            .unwrap();
        restored
            .validate_nesting_depth(super::super::MAX_RUNTIME_VALUE_NESTING_DEPTH)
            .unwrap();
        let invalid = AwbcRuntimeValueSnapshot::Tuple(vec![valid]);
        assert!(matches!(
            invalid.into_runtime_value_for_program(&owner),
            Err(AwbcRuntimeValueSnapshotError::Nesting(_))
        ));
        let mut hidden = AwbcRuntimeValueSnapshot::Callable(AwbcRuntimeCallableSnapshot {
            state: RuntimeCallableStateId::from_zero_based(0).unwrap(),
            retained: vec![],
        });
        for _ in 0..=super::super::MAX_RUNTIME_VALUE_NESTING_DEPTH {
            hidden = AwbcRuntimeValueSnapshot::Tuple(vec![hidden]);
        }
        assert!(
            matches!(
                hidden.into_runtime_value_for_program(&owner),
                Err(AwbcRuntimeValueSnapshotError::Nesting(_))
            ),
            "the complete inert graph rejects before a missing callable constructor"
        );
    }
}

#[test]
fn dense_atoms_at_the_value_nesting_boundary_obey_the_selected_logical_depth() {
    let mut value =
        RuntimeValue::MatrixF32(crate::math::DenseMatrixF32::new(1, 2, vec![1.25, 2.5]).unwrap());
    for _ in 0..super::super::MAX_RUNTIME_VALUE_NESTING_DEPTH {
        value = RuntimeValue::Tuple(vec![value]);
    }
    let limits = RuntimeSchemaLimits {
        max_depth: u32::try_from(super::super::MAX_RUNTIME_VALUE_NESTING_DEPTH + 1).unwrap(),
        ..RuntimeSchemaLimits::engine_default()
    };
    for owner in owners() {
        let image = AwbcRuntimeValueSnapshot::from_runtime_value_for_program_with_limits(
            &value, &owner, limits,
        )
        .unwrap();
        assert_eq!(
            image
                .clone()
                .into_runtime_value_for_program_with_limits(&owner, limits)
                .unwrap(),
            value,
        );
        let shallower = RuntimeSchemaLimits {
            max_depth: limits.max_depth - 1,
            ..limits
        };
        assert!(matches!(
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program_with_limits(
                &value, &owner, shallower
            ),
            Err(AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "depth" }
            )),
        ));
        assert!(matches!(
            image.into_runtime_value_for_program_with_limits(&owner, shallower),
            Err(AwbcRuntimeValueSnapshotError::Resource(
                crate::entry::RuntimeSchemaError::BudgetExceeded { budget: "depth" }
            )),
        ));
    }
}

#[test]
fn inert_preflight_checks_empty_column_metadata_and_preserves_issued_handle_tokens() {
    let mut storage = AwbcRuntimeSeqSnapshot::Values(vec![]);
    for _ in 0..=super::super::MAX_RUNTIME_VALUE_NESTING_DEPTH {
        storage = AwbcRuntimeSeqSnapshot::TupleColumns {
            len: 0,
            columns: vec![storage],
        };
    }
    for owner in owners() {
        assert!(matches!(
            AwbcRuntimeValueSnapshot::Seq(storage.clone())
                .into_unrestricted_runtime_value_for_program(&owner),
            Err(AwbcRuntimeValueSnapshotError::Nesting(_))
        ));
    }
    let issued = crate::tests::program_custody::issued_program_handle();
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&issued.plan));
    let image =
        AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&issued.value, &owner).unwrap();
    assert_eq!(
        image.affine_line_handle_tokens().unwrap(),
        vec![issued.token.clone()]
    );
    assert_eq!(
        image.into_unrestricted_runtime_value_for_program(&owner),
        Err(AwbcRuntimeValueSnapshotError::AffineValue)
    );
    assert!(
        issued.ledger.lease(&issued.token).is_some(),
        "inert rejection preserves the original issued ledger"
    );
    assert_eq!(
        AwbcRuntimeValueSnapshot::from_unrestricted_runtime_value_for_program(
            &issued.value,
            &owner
        ),
        Err(AwbcRuntimeValueSnapshotError::AffineValue)
    );
}

#[test]
fn float_snapshots_preserve_scalar_bits_and_dense_storage_in_nested_values() {
    use crate::value::DenseSeqStorage;

    let bits32 = [0x8000_0000, 0x7f80_0000, 0x7fc0_1234];
    let bits64 = [
        0x8000_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0x7ff8_0000_0000_1234,
    ];
    let value = RuntimeValue::Tuple(vec![
        RuntimeValue::F32(f32::from_bits(bits32[2])),
        RuntimeValue::F64(f64::from_bits(bits64[2])),
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F32(DenseSeqStorage::new(
            bits32.map(f32::from_bits).to_vec(),
        )))),
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F64(DenseSeqStorage::new(
            bits64.map(f64::from_bits).to_vec(),
        )))),
    ]);
    let image = AwbcRuntimeValueSnapshot::from_standalone_value(&value).unwrap();
    let bytes = serde_json::to_vec(&image).unwrap();
    let decoded: AwbcRuntimeValueSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(image, decoded);
    assert_ne!(
        value, value,
        "ordinary runtime equality retains NaN semantics"
    );
    let mut altered = image.clone();
    let AwbcRuntimeValueSnapshot::Tuple(values) = &mut altered else {
        unreachable!();
    };
    let AwbcRuntimeValueSnapshot::Seq(AwbcRuntimeSeqSnapshot::Dense(DenseSeq::F32(storage))) =
        &mut values[2]
    else {
        unreachable!();
    };
    let changed = [bits32[0], bits32[1], bits32[2] ^ 1];
    *storage = DenseSeqStorage::new(changed.map(f32::from_bits).to_vec());
    assert_ne!(image, altered, "snapshot equality retains the NaN payload");
    let owner = RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram::default()));
    let restored = decoded.into_runtime_value_for_program(&owner).unwrap();
    let RuntimeValue::Tuple(values) = restored else {
        panic!("tuple storage survives");
    };
    assert_eq!(values.len(), 4);
    let RuntimeValue::F32(value) = values[0] else {
        panic!("f32 scalar survives");
    };
    assert_eq!(value.to_bits(), bits32[2]);
    let RuntimeValue::F64(value) = values[1] else {
        panic!("f64 scalar survives");
    };
    assert_eq!(value.to_bits(), bits64[2]);
    let RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F32(value))) = &values[2] else {
        panic!("f32 dense storage survives");
    };
    assert_eq!(
        value
            .as_slice()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>(),
        bits32
    );
    let RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F64(value))) = &values[3] else {
        panic!("f64 dense storage survives");
    };
    assert_eq!(
        value
            .as_slice()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>(),
        bits64
    );
}

#[test]
fn native_rollback_image_round_trips_plan_callable_and_affine_iterator_without_live_clone() {
    let plan = Arc::new(crate::tests::function_application::returning_function_plan(
        crate::plan::RuntimeFunctionSiteBodyKind::Expression,
    ));
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let callable = RuntimeCallableValue::try_new(
        owner.clone(),
        crate::tests::function_application::returning_callable_state(&plan),
        [],
    )
    .unwrap();
    let value = RuntimeValue::Tuple(vec![
        RuntimeValue::Callable(callable),
        RuntimeValue::Iterator(RuntimeIterator::values(vec![RuntimeValue::NeedHandle(
            crate::tests::reusable_need("need.once"),
        )])),
    ]);
    assert!(!value.ownership().permits_copy());
    assert!(AwbcRuntimeValueSnapshot::from_standalone_value(&value).is_err());
    let image = AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner)
        .expect("the native rollback image records a Plan callable and iterator");
    drop(value);
    let restored = image
        .into_runtime_value_for_program(&owner)
        .expect("the sole native owner restores after the old value is dropped");
    assert!(!restored.ownership().permits_copy());
    assert!(
        matches!(restored, RuntimeValue::Tuple(values) if matches!(&values[0], RuntimeValue::Callable(_)) && matches!(&values[1], RuntimeValue::Iterator(_)))
    );
}

#[test]
fn iterator_snapshot_round_trips_only_the_owned_remainder() {
    let owner = RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram::default()));
    let mut iterator = RuntimeIterator::values(vec![
        RuntimeValue::NeedHandle(crate::tests::reusable_need("need.consumed")),
        RuntimeValue::NeedHandle(crate::tests::reusable_need("need.remaining")),
    ]);
    assert_eq!(
        iterator.next(),
        Some(RuntimeValue::NeedHandle(crate::tests::reusable_need(
            "need.consumed"
        )))
    );

    let value = RuntimeValue::Iterator(iterator);
    let snapshot = AwbcRuntimeValueSnapshot::from_standalone_value(&value)
        .expect("the iterator remainder is a valid AWBC snapshot");
    let encoded = serde_json::to_vec(&snapshot).expect("iterator remainder serializes");
    let decoded: AwbcRuntimeValueSnapshot =
        serde_json::from_slice(&encoded).expect("iterator remainder decodes");
    let restored = decoded
        .into_runtime_value_for_program(&owner)
        .expect("the iterator remainder restores");

    let RuntimeValue::Iterator(mut restored_iterator) = restored else {
        panic!("snapshot restores a runtime iterator");
    };
    assert_eq!(
        restored_iterator.next(),
        Some(RuntimeValue::NeedHandle(crate::tests::reusable_need(
            "need.remaining"
        )))
    );
}

#[test]
fn need_snapshot_preserves_handle_identity_and_rejects_zero_or_forged_correlations() {
    let value = RuntimeValue::NeedHandle(crate::tests::reusable_need("need.profile"));
    let snapshot = AwbcRuntimeValueSnapshot::from_standalone_value(&value)
        .expect("typed Need handle snapshots");
    let encoded = serde_json::to_vec(&snapshot).expect("Need snapshot serializes");
    let decoded: AwbcRuntimeValueSnapshot =
        serde_json::from_slice(&encoded).expect("Need snapshot decodes");
    let owner = RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram::default()));
    assert_eq!(decoded.into_runtime_value_for_program(&owner), Ok(value));
    let mut zero: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    zero["NeedHandle"]["correlation"]["need"] = serde_json::json!(vec![0_u8; 32]);
    assert!(serde_json::from_value::<AwbcRuntimeValueSnapshot>(zero).is_err());
    let mut forged: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    forged["NeedHandle"]["correlation"]["producer_contract"] = serde_json::json!(vec![9_u8; 32]);
    let forged: AwbcRuntimeValueSnapshot = serde_json::from_value(forged).unwrap();
    assert!(forged.into_runtime_value_for_program(&owner).is_err());
}

#[test]
fn rgba8_color_snapshot_preserves_all_channels() {
    let value = RuntimeValue::Color(crate::value::RuntimeColor::new(12, 34, 56, 78));
    let snapshot = AwbcRuntimeValueSnapshot::from_standalone_value(&value)
        .expect("RGBA8 color snapshots without a program-specific payload");
    let encoded = serde_json::to_vec(&snapshot).expect("Color snapshot serializes");
    let decoded: AwbcRuntimeValueSnapshot =
        serde_json::from_slice(&encoded).expect("Color snapshot decodes");
    let owner = RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram::default()));

    assert_eq!(decoded.into_runtime_value_for_program(&owner), Ok(value));
}

#[test]
fn awbc_snapshot_deserialize_rejects_empty_all_and_any_predicates() {
    for value in [
        serde_json::json!({ "All": { "predicates": [] } }),
        serde_json::json!({ "Any": { "predicates": [] } }),
    ] {
        assert!(serde_json::from_value::<AwbcRuntimeAgentPredicateSnapshot>(value).is_err());
    }
}

#[test]
fn awbc_snapshot_deserialize_rejects_nested_empty_predicates() {
    let value = serde_json::json!({
        "All": {
            "predicates": [{
                "Any": {
                    "predicates": [],
                },
            }],
        },
    });
    assert!(serde_json::from_value::<AwbcRuntimeAgentPredicateSnapshot>(value).is_err());
}

fn semantic(index: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([index; 32])
}

pub(in crate::value) fn owners() -> [RuntimeProgramOwner; 2] {
    let limits = RuntimeSchemaLimits::engine_default();
    let record = RuntimeNominalTypeId::try_new("snapshot.Record").unwrap();
    let event = RuntimeNominalTypeId::try_new("snapshot.Event").unwrap();
    let record_identity = RuntimeNominalSchemaIdentity::new(record.clone(), semantic(1));
    let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![
            RuntimeNominalSchemaDefinition::new(
                crate::entry::RuntimeNominalDeclarationId::from_bytes(
                    *(record_identity.clone()).semantic_identity().as_bytes(),
                ),
                record_identity.clone(),
                vec![],
                RuntimeNominalSchemaBody::Record {
                    shape: RuntimeNominalRecordShape::Record,
                    fields: vec![RuntimeNominalSchemaField::new(
                        field,
                        Some("flag".to_owned()),
                        RuntimeTypeSchema::Bool,
                    )]
                    .into(),
                },
            ),
            RuntimeNominalSchemaDefinition::new(
                crate::entry::RuntimeNominalDeclarationId::from_bytes(
                    *(RuntimeNominalSchemaIdentity::new(event.clone(), semantic(3)))
                        .semantic_identity()
                        .as_bytes(),
                ),
                RuntimeNominalSchemaIdentity::new(event.clone(), semantic(3)),
                vec![],
                RuntimeNominalSchemaBody::Variant {
                    cases: vec![
                        RuntimeNominalSchemaCase::new(0, "Empty".to_owned(), None),
                        RuntimeNominalSchemaCase::new(
                            1,
                            "Set".to_owned(),
                            Some(RuntimeTypeSchema::Tuple(
                                vec![RuntimeTypeSchema::NominalRef(record_identity)].into(),
                            )),
                        ),
                    ]
                    .into(),
                },
            ),
        ],
        limits,
    )
    .unwrap();
    let record_layout = graph.try_layout_hash(semantic(1)).unwrap();
    let event_layout = graph.try_layout_hash(semantic(3)).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(
                    semantic(1),
                    RuntimePlanTypeProjection::Nominal {
                        nominal: record.clone(),
                        layout: record_layout,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(semantic(2), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    semantic(3),
                    RuntimePlanTypeProjection::Nominal {
                        nominal: event.clone(),
                        layout: event_layout,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    semantic(4),
                    RuntimePlanTypeProjection::Tuple(Box::new([semantic(1)])),
                ),
            ],
            [],
            [RuntimeNominalRecordDomainSeed::new(
                semantic(1),
                RuntimeNominalRecordShape::Record,
                [RuntimeNominalRecordDomainFieldSeed::new(
                    field,
                    Some("flag".to_owned()),
                    semantic(2),
                )],
            )],
            [RuntimeVariantDomainSeed::new(
                semantic(3),
                event,
                event_layout,
                [
                    RuntimeVariantCaseSeed::new("Empty", None),
                    RuntimeVariantCaseSeed::new("Set", Some(semantic(4))),
                ],
            )],
            &graph,
        )
        .unwrap();
    let awbc = AwbcProgram {
        strings: ["Empty", "Set", "flag", "snapshot.Event", "snapshot.Record"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        runtime_types: vec![
            AwbcRuntimeType::new(
                semantic(1),
                AwbcRuntimeTypeShape::NominalRecord {
                    public_id: AwbcStringId(4),
                    layout: *record_layout.as_bytes(),
                    arguments: vec![],
                    shape: RuntimeNominalRecordShape::Record,
                    fields: vec![AwbcRecordField {
                        field,
                        name: Some(AwbcStringId(2)),
                        ty: AwbcTypeId(1),
                    }],
                },
            ),
            AwbcRuntimeType::new(semantic(2), AwbcRuntimeTypeShape::Bool),
            AwbcRuntimeType::new(
                semantic(3),
                AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Nominal {
                        public_id: AwbcStringId(3),
                        layout: *event_layout.as_bytes(),
                    },
                    arguments: vec![],
                    cases: vec![
                        AwbcVariantCase {
                            name: AwbcStringId(0),
                            payload: None,
                        },
                        AwbcVariantCase {
                            name: AwbcStringId(1),
                            payload: Some(AwbcTypeId(3)),
                        },
                    ],
                },
            ),
            AwbcRuntimeType::new(
                semantic(4),
                AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0)]),
            ),
        ],
        ..AwbcProgram::default()
    };
    [
        RuntimeProgramOwner::Plan(Arc::new(builder.finish().unwrap())),
        RuntimeProgramOwner::Awbc(Arc::new(awbc)),
    ]
}

#[test]
fn nominal_snapshot_restore_requires_exact_program_admission() {
    for owner in owners() {
        let record = owner
            .types()
            .try_record_value(
                semantic(1),
                vec![RuntimeValue::Bool(true)],
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap();
        let event = owner
            .types()
            .try_variant_value(
                semantic(3),
                1,
                Some(RuntimeValue::Tuple(vec![record.clone()])),
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap();
        for value in [
            record.clone(),
            event.clone(),
            RuntimeValue::Tuple(vec![event]),
        ] {
            assert!(AwbcRuntimeValueSnapshot::from_standalone_value(&value).is_err());
            let absent_program = RuntimeProgramOwner::Awbc(Arc::new(AwbcProgram::default()));
            assert!(
                AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &absent_program)
                    .is_err()
            );
            let saved =
                AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
            let encoded = serde_json::to_value(saved.clone()).unwrap();
            let saved: AwbcRuntimeValueSnapshot = serde_json::from_value(encoded).unwrap();
            assert_eq!(
                saved
                    .clone()
                    .into_runtime_value_for_program(&owner)
                    .unwrap(),
                value
            );
            assert!(
                saved
                    .into_runtime_value_for_program(&absent_program)
                    .is_err()
            );
        }
        let AwbcRuntimeValueSnapshot::NominalRecord(saved) =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&record, &owner).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(
            serde_json::to_value(&saved.fields).unwrap(),
            serde_json::json!([{"field": 1, "value": {"Bool": true}}])
        );
        let mut old_fields = serde_json::to_value(&saved).unwrap();
        old_fields["fields"] = serde_json::json!([{"Bool": true}]);
        assert!(serde_json::from_value::<AwbcRuntimeNominalRecordSnapshot>(old_fields).is_err());
        for defect in 0..6 {
            let mut tampered = saved.clone();
            match defect {
                0 => tampered.semantic_identity = semantic(2),
                1 => tampered.type_id = RuntimeNominalTypeId::try_new("snapshot.Foreign").unwrap(),
                2 => tampered.layout = TypeLayoutHash::from_bytes([199; 32]),
                3 => tampered.fields.push(AwbcRuntimeNominalFieldSnapshot {
                    field: RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap(),
                    value: AwbcRuntimeValueSnapshot::Bool(false),
                }),
                4 => {
                    tampered.fields[0].value =
                        AwbcRuntimeValueSnapshot::String("wrong field type".to_owned())
                }
                5 => {
                    tampered.fields[0].field =
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap();
                    // The field coordinate must fail before this deliberately
                    // foreign descendant is reconstructed.
                    let mut foreign = saved.clone();
                    foreign.semantic_identity = semantic(99);
                    tampered.fields[0].value = AwbcRuntimeValueSnapshot::NominalRecord(foreign);
                }
                _ => unreachable!(),
            }
            let error = AwbcRuntimeValueSnapshot::NominalRecord(tampered)
                .into_runtime_value_for_program(&owner)
                .expect_err("invalid nominal snapshot");
            if defect == 5 {
                assert!(error.to_string().contains("nominal snapshot field 0"));
            }
        }
        let foreign = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            saved.type_id.clone(),
            semantic(3),
            saved.layout,
            vec![RuntimeValue::Bool(true)],
        ));
        assert_ne!(record, foreign);
        assert_ne!(
            record.try_canonical_bytes(1024).unwrap(),
            foreign.try_canonical_bytes(1024).unwrap()
        );
        assert!(
            owner
                .types()
                .validate_live_value(semantic(1), &foreign, RuntimeSchemaLimits::engine_default())
                .is_err()
        );
        assert!(
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&foreign, &owner).is_err()
        );
        let wrong_case = AwbcRuntimeValueSnapshot::Variant {
            type_instantiation: None,
            owner: super::super::RuntimeVariantIdentity::Nominal {
                nominal: RuntimeNominalTypeId::try_new("snapshot.Event").unwrap(),
                semantic_identity: semantic(3),
                layout: match owner
                    .types()
                    .try_variant_value(semantic(3), 0, None, RuntimeSchemaLimits::engine_default())
                    .unwrap()
                {
                    RuntimeValue::Variant(crate::value::RuntimeVariantValue {
                        owner: super::super::RuntimeVariantIdentity::Nominal { layout, .. },
                        ..
                    }) => layout,
                    _ => unreachable!(),
                },
            },
            ordinal: 0,
            name: "Set".to_owned(),
            payload: None,
        };
        let wrong_case_live = RuntimeValue::Variant(crate::value::RuntimeVariantValue {
            owner: match &wrong_case {
                AwbcRuntimeValueSnapshot::Variant { owner, .. } => owner.clone(),
                _ => unreachable!(),
            },
            ordinal: 0,
            name: "Set".to_owned(),
            type_instantiation: None,
            payload: None,
        });
        assert!(
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&wrong_case_live, &owner)
                .is_err()
        );
        assert!(wrong_case.into_runtime_value_for_program(&owner).is_err());

        let event = owner
            .types()
            .try_variant_value(
                semantic(3),
                1,
                Some(RuntimeValue::Tuple(vec![record.clone()])),
                RuntimeSchemaLimits::engine_default(),
            )
            .unwrap();
        assert!(serde_json::to_value(&record).is_err());
        assert!(
            serde_json::from_value::<RuntimeValue>(serde_json::json!({
                "NominalRecord": {
                    "type_id": saved.type_id,
                    "semantic_identity": saved.semantic_identity,
                    "layout": saved.layout,
                    "type_instantiation": null,
                    "fields": [{"Bool": true}]
                }
            }))
            .is_err()
        );
        let image =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&event, &owner).unwrap();
        for defect in 0..5 {
            let mut candidate = image.clone();
            let AwbcRuntimeValueSnapshot::Variant {
                owner: identity,
                ordinal,
                name,
                payload,
                ..
            } = &mut candidate
            else {
                unreachable!();
            };
            let mut foreign = saved.clone();
            foreign.semantic_identity = semantic(99);
            *payload = Some(Box::new(AwbcRuntimeValueSnapshot::NominalRecord(foreign)));
            match defect {
                0 => *name = "Unknown".to_owned(),
                1 => *ordinal = 99,
                2 => *payload = None,
                3 => {
                    let RuntimeVariantIdentity::Nominal { nominal, .. } = identity else {
                        unreachable!();
                    };
                    *nominal = RuntimeNominalTypeId::try_new("snapshot.Foreign").unwrap();
                }
                4 => {
                    let RuntimeVariantIdentity::Nominal { layout, .. } = identity else {
                        unreachable!();
                    };
                    *layout = TypeLayoutHash::from_bytes([199; 32]);
                }
                _ => unreachable!(),
            }
            let error = candidate
                .into_runtime_value_for_program(&owner)
                .expect_err("invalid variant header precedes descendant restoration");
            if defect != 1 {
                assert!(error.to_string().contains("different snapshot header"));
            }
        }
    }
}

#[test]
fn nominal_variant_construction_and_restore_reject_dangling_payload_type_without_panic() {
    let [_, RuntimeProgramOwner::Awbc(mut program)] = owners() else {
        unreachable!("fixture has one native and one AWBC owner");
    };
    let program = Arc::make_mut(&mut program);
    let ordinal = program
        .runtime_types
        .iter()
        .position(|ty| ty.semantic_identity() == semantic(3))
        .unwrap();
    let mut shape = program.runtime_types[ordinal].shape().clone();
    let AwbcRuntimeTypeShape::Variant {
        owner: AwbcVariantIdentity::Nominal { public_id, layout },
        cases,
        ..
    } = &mut shape
    else {
        unreachable!("fixture nominal variant");
    };
    let nominal =
        RuntimeNominalTypeId::try_new(program.strings[public_id.index()].clone()).unwrap();
    let layout = TypeLayoutHash::from_bytes(*layout);
    cases[1].payload = Some(AwbcTypeId(999));
    program.runtime_types[ordinal] = AwbcRuntimeType::new(semantic(3), shape);
    let owner = RuntimeProgramOwner::Awbc(Arc::new(program.clone()));
    let error = owner
        .types()
        .try_variant_value(
            semantic(3),
            1,
            Some(RuntimeValue::Unit),
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap_err();
    let crate::program_types::RuntimeProgramValueConstructionError::ProgramType(
        crate::program_types::RuntimeProgramTypeError::AwbcType(error),
    ) = error
    else {
        panic!("expected typed AWBC child-reference error");
    };
    assert_eq!(
        *error,
        crate::awbc::type_projection::AwbcTypeProjectionError::RuntimeTypeOutOfBounds {
            index: 999
        }
    );
    let saved = AwbcRuntimeValueSnapshot::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            nominal,
            semantic_identity: semantic(3),
            layout,
        },
        ordinal: 1,
        name: "Set".into(),
        payload: Some(Box::new(AwbcRuntimeValueSnapshot::Unit)),
        type_instantiation: None,
    };
    let error = saved.into_runtime_value_for_program(&owner).unwrap_err();
    assert_eq!(
        error.to_string(),
        "AWBC runtime type index 999 is out of bounds"
    );
}

#[test]
fn inert_expected_type_rejects_a_nominal_child_before_callable_construction() {
    let limits = RuntimeSchemaLimits::engine_default();
    for owner in owners() {
        let record = owner
            .types()
            .try_record_value(semantic(1), vec![RuntimeValue::Bool(true)], limits)
            .unwrap();
        let image =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&record, &owner).unwrap();
        for unrestricted in [false, true] {
            let restored = if unrestricted {
                image
                    .clone()
                    .into_unrestricted_typed_runtime_value_for_program_with_limits(
                        &owner,
                        semantic(1),
                        limits,
                    )
            } else {
                image
                    .clone()
                    .into_typed_runtime_value_for_program_with_limits(&owner, semantic(1), limits)
            }
            .unwrap();
            assert_eq!(restored, record);

            let mut candidate = image.clone();
            let AwbcRuntimeValueSnapshot::NominalRecord(record) = &mut candidate else {
                unreachable!();
            };
            record.fields[0].value =
                AwbcRuntimeValueSnapshot::Callable(AwbcRuntimeCallableSnapshot {
                    state: crate::runtime_id::RuntimeCallableStateId::from_zero_based(0).unwrap(),
                    retained: vec![],
                });
            let error = if unrestricted {
                candidate.into_unrestricted_typed_runtime_value_for_program_with_limits(
                    &owner,
                    semantic(1),
                    limits,
                )
            } else {
                candidate.into_typed_runtime_value_for_program_with_limits(
                    &owner,
                    semantic(1),
                    limits,
                )
            }
            .unwrap_err();
            let AwbcRuntimeValueSnapshotError::ProgramType(error) = error else {
                panic!("expected type rejection precedes missing callable construction: {error}");
            };
            let source = match *error {
                crate::program_types::RuntimeProgramTypeError::PlanValue(error) => match *error {
                    crate::plan::RuntimePlanValueAdmissionError::Value { source, .. } => source,
                    error => panic!("expected original plan value rejection: {error}"),
                },
                crate::program_types::RuntimeProgramTypeError::AwbcValue(error) => match *error {
                    crate::awbc::value_admission::AwbcValueAdmissionError::Value {
                        source, ..
                    } => source,
                    error
                    @ crate::awbc::value_admission::AwbcValueAdmissionError::UnknownType {
                        ..
                    } => panic!("expected original AWBC value rejection: {error}"),
                },
                error => panic!("expected original program value admission: {error}"),
            };
            assert!(matches!(
                source,
                crate::entry::RuntimeSchemaError::Type { .. }
            ));
        }
    }
}

#[test]
fn inert_expected_type_checks_field_ids_and_reuses_preflight_work() {
    use crate::{entry::RuntimeSchemaError, program_types::RuntimeProgramTypeError};
    for owner in owners() {
        let limits = RuntimeSchemaLimits::engine_default();
        let image = AwbcRuntimeValueSnapshot::Bool(true);
        owner
            .types()
            .validate_inert_snapshot_value(
                semantic(2),
                &image,
                RuntimeSchemaLimits {
                    max_validation_work: 2,
                    ..limits
                },
            )
            .unwrap();
        let error = owner
            .types()
            .validate_inert_snapshot_value(
                semantic(2),
                &image,
                RuntimeSchemaLimits {
                    max_validation_work: 1,
                    ..limits
                },
            )
            .unwrap_err();
        let source = match error {
            RuntimeProgramTypeError::PlanValue(error) => match *error {
                crate::plan::RuntimePlanValueAdmissionError::Value { source, .. } => source,
                error => panic!("expected original plan work rejection: {error}"),
            },
            RuntimeProgramTypeError::AwbcValue(error) => match *error {
                crate::awbc::value_admission::AwbcValueAdmissionError::Value { source, .. } => {
                    source
                }
                error @ crate::awbc::value_admission::AwbcValueAdmissionError::UnknownType {
                    ..
                } => panic!("expected original AWBC work rejection: {error}"),
            },
            error => panic!("expected type phase to retain the resource work: {error}"),
        };
        assert!(matches!(
            source,
            RuntimeSchemaError::ValidationWork { consumed: 2, .. }
        ));

        let record = owner
            .types()
            .try_record_value(semantic(1), vec![RuntimeValue::Bool(true)], limits)
            .unwrap();
        let mut image =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&record, &owner).unwrap();
        // Two resource nodes, two expected-value nodes, and the original
        // nominal type relation share one counter for either executable kind.
        let exact = RuntimeSchemaLimits {
            max_validation_work: 5,
            ..limits
        };
        owner
            .types()
            .validate_inert_snapshot_value(semantic(1), &image, exact)
            .unwrap();
        let error = owner
            .types()
            .validate_inert_snapshot_value(
                semantic(1),
                &image,
                RuntimeSchemaLimits {
                    max_validation_work: 4,
                    ..exact
                },
            )
            .unwrap_err();
        let source = match error {
            RuntimeProgramTypeError::PlanValue(error) => match *error {
                crate::plan::RuntimePlanValueAdmissionError::Value { source, .. } => source,
                error => panic!("expected original plan work rejection: {error}"),
            },
            RuntimeProgramTypeError::AwbcValue(error) => match *error {
                crate::awbc::value_admission::AwbcValueAdmissionError::Value { source, .. } => {
                    source
                }
                error @ crate::awbc::value_admission::AwbcValueAdmissionError::UnknownType {
                    ..
                } => panic!("expected original AWBC work rejection: {error}"),
            },
            error => panic!("expected nominal type relation to reuse resource work: {error}"),
        };
        assert!(matches!(
            source,
            RuntimeSchemaError::ValidationWork {
                limit: 4,
                consumed: 5,
                ..
            }
        ));
        let AwbcRuntimeValueSnapshot::NominalRecord(record) = &mut image else {
            unreachable!();
        };
        record.fields[0].field = RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap();
        assert!(
            owner
                .types()
                .validate_inert_snapshot_value(semantic(1), &image, limits)
                .is_err()
        );

        let error = owner
            .types()
            .validate_inert_snapshot_value(
                semantic(250),
                &image,
                RuntimeSchemaLimits {
                    max_encoded_bytes: 0,
                    ..limits
                },
            )
            .unwrap_err();
        assert!(
            matches!(error, RuntimeProgramTypeError::SnapshotResource(error)
            if matches!(*error, RuntimeSchemaError::BudgetExceeded { budget: "encoded_bytes" }))
        );
    }
}
