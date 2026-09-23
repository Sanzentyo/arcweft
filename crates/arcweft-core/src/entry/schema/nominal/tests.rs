use super::*;
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
};
use crate::value::{
    RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeValue,
};
use std::collections::BTreeSet;

fn identity(byte: u8) -> RuntimeNominalSchemaIdentity {
    RuntimeNominalSchemaIdentity::new(
        RuntimeNominalTypeId::try_new(format!("n{byte}")).unwrap(),
        RuntimeSemanticTypeId::from_bytes([byte; 32]),
    )
}

fn record(
    byte: u8,
    shape: RuntimeNominalRecordShape,
    arguments: Vec<RuntimeTypeSchema>,
    fields: Vec<(Option<&str>, RuntimeTypeSchema)>,
) -> RuntimeNominalSchemaDefinition {
    RuntimeNominalSchemaDefinition::new(
        identity(byte),
        arguments,
        RuntimeNominalSchemaBody::Record {
            shape,
            fields: fields
                .into_iter()
                .enumerate()
                .map(|(index, (name, schema))| {
                    RuntimeNominalSchemaField::new(
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(index).unwrap(),
                        name.map(str::to_owned),
                        schema,
                    )
                })
                .collect(),
        },
    )
}

fn unit(byte: u8, arguments: Vec<RuntimeTypeSchema>) -> RuntimeNominalSchemaDefinition {
    record(byte, RuntimeNominalRecordShape::Unit, arguments, vec![])
}

fn graph(definitions: Vec<RuntimeNominalSchemaDefinition>) -> RuntimeNominalSchemaGraph {
    RuntimeNominalSchemaGraph::try_new(definitions, RuntimeSchemaLimits::engine_default()).unwrap()
}

fn hash(graph: &RuntimeNominalSchemaGraph, byte: u8) -> TypeLayoutHash {
    graph
        .try_layout_hash(identity(byte).semantic_identity())
        .unwrap()
}

#[test]
fn merging_source_graphs_preserves_recursive_layouts_and_rejects_conflicting_bodies() {
    let original = graph(recursive_definitions());
    let mut reversed_definitions = recursive_definitions();
    reversed_definitions.reverse();
    let reordered = graph(reversed_definitions);
    let unrelated = graph(vec![unit(200, vec![])]);
    let limits = RuntimeSchemaLimits::engine_default();
    let merged = RuntimeNominalSchemaGraph::try_merge(
        [&original, &original, &reordered, &unrelated],
        limits,
    )
    .unwrap();
    for definition in original.definitions() {
        let root = definition.identity().semantic_identity();
        assert_eq!(merged.try_layout_hash(root), original.try_layout_hash(root));
    }
    assert_eq!(merged.definitions().len(), original.definitions().len() + 1);

    let bytes = |format| {
        graph(vec![record(
            201,
            RuntimeNominalRecordShape::Newtype,
            vec![],
            vec![(None, RuntimeTypeSchema::Bytes { format })],
        )])
    };
    let binary = bytes(super::super::RuntimeBytesFormat::Binary);
    let hex = bytes(super::super::RuntimeBytesFormat::Hex);
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_merge([&binary, &hex], limits),
        Err(RuntimeNominalSchemaGraphError::ConflictingDefinition { .. })
    ));
    assert!(
        RuntimeNominalSchemaGraph::try_merge(
            [&original],
            RuntimeSchemaLimits {
                max_nodes: 0,
                ..limits
            }
        )
        .is_err()
    );
}

fn nominal_value(
    graph: &RuntimeNominalSchemaGraph,
    byte: u8,
    fields: Vec<RuntimeValue>,
) -> RuntimeValue {
    RuntimeValue::NominalRecord(crate::value::RuntimeNominalRecordValue::new(
        identity(byte).nominal().clone(),
        identity(byte).semantic_identity(),
        hash(graph, byte),
        fields,
    ))
}

#[test]
fn empty_arrays_still_require_complete_nominal_item_definitions() {
    let container = record(
        2,
        RuntimeNominalRecordShape::Tuple,
        vec![],
        vec![(
            None,
            RuntimeTypeSchema::Array {
                item: Box::new(RuntimeTypeSchema::NominalRef(identity(1))),
                length: 0,
            },
        )],
    );
    assert!(
        RuntimeNominalSchemaGraph::try_new(
            vec![container.clone()],
            RuntimeSchemaLimits::engine_default()
        )
        .is_err()
    );
    let accepted = graph(vec![container.clone(), unit(1, vec![])]);
    let value = nominal_value(
        &accepted,
        2,
        vec![RuntimeValue::Seq(crate::value::RuntimeSeq::dense_units(0))],
    );
    assert!(
        accepted
            .accepts_value(
                identity(2).semantic_identity(),
                &value,
                RuntimeSchemaLimits::engine_default()
            )
            .is_ok()
    );
    let changed_item = graph(vec![
        container,
        record(
            1,
            RuntimeNominalRecordShape::Tuple,
            vec![],
            vec![(None, RuntimeTypeSchema::Bool)],
        ),
    ]);
    assert_ne!(hash(&accepted, 2), hash(&changed_item, 2));
}

#[test]
fn graph_choices_retain_nominal_edges_and_validate_exact_layouts() {
    let graph = graph(vec![
        unit(1, vec![]),
        record(
            2,
            RuntimeNominalRecordShape::Record,
            vec![],
            vec![(
                Some("value"),
                RuntimeTypeSchema::Choice(
                    vec![
                        RuntimeTypeSchema::NominalRef(identity(1)),
                        RuntimeTypeSchema::Bool,
                    ]
                    .into_boxed_slice(),
                ),
            )],
        ),
    ]);
    let limits = RuntimeSchemaLimits {
        max_nodes: 2,
        max_validation_work: 6,
        ..RuntimeSchemaLimits::engine_default()
    };
    let value = nominal_value(&graph, 2, vec![nominal_value(&graph, 1, vec![])]);
    assert_eq!(
        graph
            .accepts_value(identity(2).semantic_identity(), &value, limits)
            .unwrap(),
        value.try_digest(limits.platform_encoded_bytes()).unwrap()
    );
    let wrong = RuntimeValue::NominalRecord(crate::value::RuntimeNominalRecordValue::new(
        identity(1).nominal().clone(),
        identity(1).semantic_identity(),
        TypeLayoutHash::from_bytes([99; 32]),
        vec![],
    ));
    let value = nominal_value(&graph, 2, vec![wrong]);
    let Err(RuntimeSchemaError::ChoiceNoMatch { branches, .. }) =
        graph.accepts_value(identity(2).semantic_identity(), &value, limits)
    else {
        panic!("neither nominal nor Bool branch admits a forged layout");
    };
    assert!(matches!(
        branches[0].source(),
        RuntimeSchemaError::NominalLayout { .. }
    ));
}

#[test]
fn builtin_payload_schemas_keep_nominal_graph_edges_in_case_order() {
    let container = record(
        3,
        RuntimeNominalRecordShape::Tuple,
        vec![],
        vec![(
            None,
            RuntimeTypeSchema::result(
                RuntimeTypeSchema::NominalRef(identity(1)),
                RuntimeTypeSchema::NominalRef(identity(2)),
            ),
        )],
    );
    assert!(
        RuntimeNominalSchemaGraph::try_new(
            vec![unit(1, vec![]), container.clone()],
            RuntimeSchemaLimits::engine_default(),
        )
        .is_err()
    );
    let graph = graph(vec![unit(1, vec![]), unit(2, vec![]), container]);
    for (is_ok, byte) in [(true, 1), (false, 2)] {
        let wrap = |value| {
            if is_ok {
                RuntimeValue::result_ok(value)
            } else {
                RuntimeValue::result_err(value)
            }
        };
        let value = nominal_value(&graph, 3, vec![wrap(nominal_value(&graph, byte, vec![]))]);
        assert!(
            graph
                .accepts_value(
                    identity(3).semantic_identity(),
                    &value,
                    RuntimeSchemaLimits::engine_default(),
                )
                .is_ok()
        );
        let wrong_case = nominal_value(
            &graph,
            3,
            vec![wrap(nominal_value(&graph, 3 - byte, vec![]))],
        );
        assert!(
            graph
                .accepts_value(
                    identity(3).semantic_identity(),
                    &wrong_case,
                    RuntimeSchemaLimits::engine_default(),
                )
                .is_err()
        );
        let wrong_layout =
            RuntimeValue::NominalRecord(crate::value::RuntimeNominalRecordValue::new(
                identity(byte).nominal().clone(),
                identity(byte).semantic_identity(),
                TypeLayoutHash::from_bytes([99; 32]),
                vec![],
            ));
        let value = nominal_value(&graph, 3, vec![wrap(wrong_layout)]);
        assert!(matches!(
            graph.accepts_value(
                identity(3).semantic_identity(),
                &value,
                RuntimeSchemaLimits::engine_default()
            ),
            Err(RuntimeSchemaError::NominalLayout { .. })
        ));
    }
}

#[test]
fn graph_values_share_layout_work_and_one_value_allowance() {
    let definitions = || {
        vec![
            record(
                1,
                RuntimeNominalRecordShape::Tuple,
                vec![],
                vec![(
                    None,
                    RuntimeTypeSchema::Seq(Box::new(RuntimeTypeSchema::NominalRef(identity(2)))),
                )],
            ),
            unit(2, vec![]),
        ]
    };
    let layout_limits = RuntimeSchemaLimits {
        max_nodes: 6,
        ..RuntimeSchemaLimits::engine_default()
    };
    let graph = RuntimeNominalSchemaGraph::try_new(definitions(), layout_limits).unwrap();
    let leaf = nominal_value(&graph, 2, vec![]);
    let value = nominal_value(
        &graph,
        1,
        vec![RuntimeValue::Seq(crate::value::RuntimeSeq::values(
            vec![leaf; 100],
        ))],
    );
    let value_limits = RuntimeSchemaLimits {
        max_nodes: 102,
        max_depth: 2,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert_eq!(
        graph
            .accepts_value(identity(1).semantic_identity(), &value, value_limits)
            .unwrap(),
        value.try_digest(1_000_000).unwrap()
    );
    assert!(matches!(
        graph.accepts_value(
            identity(1).semantic_identity(),
            &value,
            RuntimeSchemaLimits {
                max_nodes: 101,
                ..value_limits
            }
        ),
        Err(RuntimeSchemaError::BudgetExceeded { budget: "nodes" })
    ));
    let limited = RuntimeNominalSchemaGraph::try_new(
        definitions(),
        RuntimeSchemaLimits {
            max_nodes: 5,
            ..layout_limits
        },
    )
    .unwrap();
    assert!(
        matches!(limited.accepts_value(identity(1).semantic_identity(), &value, value_limits), Err(RuntimeSchemaError::NominalGraph { source }) if matches!(*source, RuntimeNominalSchemaGraphError::BudgetExceeded { budget: "nodes" }))
    );
}

#[test]
fn recursive_nominal_values_follow_typed_references_and_builtin_payloads() {
    let graph = graph(vec![record(
        1,
        RuntimeNominalRecordShape::Newtype,
        vec![],
        vec![(
            None,
            RuntimeTypeSchema::option(RuntimeTypeSchema::NominalRef(identity(1))),
        )],
    )]);
    let mut value = nominal_value(&graph, 1, vec![RuntimeValue::option_none()]);
    for _ in 0..128 {
        value = nominal_value(&graph, 1, vec![RuntimeValue::option_some(value)]);
    }
    let limits = RuntimeSchemaLimits {
        max_depth: 385,
        max_nodes: 514,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert_eq!(
        graph
            .accepts_value(identity(1).semantic_identity(), &value, limits)
            .unwrap(),
        value.try_digest(1_000_000).unwrap()
    );
    assert!(matches!(
        graph.accepts_value(
            identity(1).semantic_identity(),
            &value,
            RuntimeSchemaLimits {
                max_depth: 384,
                ..limits
            }
        ),
        Err(RuntimeSchemaError::BudgetExceeded { budget: "depth" })
    ));
    assert!(matches!(
        graph.accepts_value(
            identity(1).semantic_identity(),
            &RuntimeValue::Tuple(vec![]),
            limits
        ),
        Err(RuntimeSchemaError::Type {
            expected: "nominal record",
            ..
        })
    ));
    assert!(
        matches!(graph.accepts_value(identity(9).semantic_identity(), &value, limits), Err(RuntimeSchemaError::NominalGraph { source }) if matches!(*source, RuntimeNominalSchemaGraphError::UnknownRoot { .. }))
    );
}

#[test]
fn graph_variants_require_exact_owner_layout_case_and_payload_shape() {
    use crate::pattern::RuntimeVariantIdentity;
    let graph = graph(vec![RuntimeNominalSchemaDefinition::new(
        identity(7),
        vec![],
        RuntimeNominalSchemaBody::Variant {
            cases: vec![
                RuntimeNominalSchemaCase::new(0, "Unit".to_owned(), None),
                RuntimeNominalSchemaCase::new(
                    1,
                    "EmptyTuple".to_owned(),
                    Some(RuntimeTypeSchema::Tuple(Box::new([]))),
                ),
                RuntimeNominalSchemaCase::new(
                    2,
                    "Single".to_owned(),
                    Some(RuntimeTypeSchema::Tuple(
                        vec![RuntimeTypeSchema::Bool].into_boxed_slice(),
                    )),
                ),
                RuntimeNominalSchemaCase::new(
                    3,
                    "EmptyRecord".to_owned(),
                    Some(RuntimeTypeSchema::RecordValue {
                        fields: Box::new([]),
                    }),
                ),
                RuntimeNominalSchemaCase::new(
                    4,
                    "Named".to_owned(),
                    Some(RuntimeTypeSchema::RecordValue {
                        fields: vec![super::super::RuntimeSchemaValueField::new(
                            RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap(),
                            "value".to_owned(),
                            RuntimeTypeSchema::I64,
                        )]
                        .into_boxed_slice(),
                    }),
                ),
            ]
            .into_boxed_slice(),
        },
    )]);
    let owner = RuntimeVariantIdentity::Nominal {
        nominal: identity(7).nominal().clone(),
        semantic_identity: identity(7).semantic_identity(),
        layout: hash(&graph, 7),
    };
    let value = |ordinal, name: &str, payload: Option<RuntimeValue>| RuntimeValue::Variant {
        owner: owner.clone(),
        ordinal,
        name: name.to_owned(),
        payload: payload.map(Box::new),
    };
    let limits = RuntimeSchemaLimits::engine_default();
    let validate =
        |value: &RuntimeValue| graph.accepts_value(identity(7).semantic_identity(), value, limits);
    for value in [
        value(0, "Unit", None),
        value(1, "EmptyTuple", Some(RuntimeValue::Tuple(vec![]))),
        value(
            2,
            "Single",
            Some(RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)])),
        ),
        value(
            3,
            "EmptyRecord",
            Some(RuntimeValue::try_record(vec![]).unwrap()),
        ),
        value(
            4,
            "Named",
            Some(
                RuntimeValue::try_record(vec![("value".to_owned(), RuntimeValue::i64(1))]).unwrap(),
            ),
        ),
    ] {
        assert_eq!(
            validate(&value).unwrap(),
            value.try_digest(1_000_000).unwrap()
        );
    }
    assert!(matches!(
        validate(&value(0, "Unit", Some(RuntimeValue::Unit))),
        Err(RuntimeSchemaError::VariantPayload { .. })
    ));
    assert!(matches!(
        validate(&value(1, "EmptyTuple", None)),
        Err(RuntimeSchemaError::VariantPayload { .. })
    ));
    assert!(matches!(
        validate(&value(2, "Single", Some(RuntimeValue::Bool(true)))),
        Err(RuntimeSchemaError::Type {
            expected: "tuple",
            ..
        })
    ));
    assert!(matches!(
        validate(&value(1, "Unit", None)),
        Err(RuntimeSchemaError::UnknownVariant { .. })
    ));
    assert!(matches!(
        validate(&value(
            4,
            "Named",
            Some(
                RuntimeValue::try_record(vec![("other".to_owned(), RuntimeValue::i64(1))]).unwrap()
            )
        )),
        Err(RuntimeSchemaError::RecordField { .. })
    ));
    let mut wrong_layout = value(0, "Unit", None);
    let RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal { layout, .. },
        ..
    } = &mut wrong_layout
    else {
        unreachable!()
    };
    *layout = TypeLayoutHash::from_bytes([9; 32]);
    assert!(matches!(
        validate(&wrong_layout),
        Err(RuntimeSchemaError::NominalLayout { .. })
    ));
    let mut wrong_semantic = value(0, "Unit", None);
    let RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            semantic_identity, ..
        },
        ..
    } = &mut wrong_semantic
    else {
        unreachable!()
    };
    *semantic_identity = identity(9).semantic_identity();
    assert!(matches!(
        validate(&wrong_semantic),
        Err(RuntimeSchemaError::NominalSemanticIdentity { .. })
    ));
}

#[test]
fn graph_nominals_cannot_be_declared_inline_without_semantic_identity() {
    for inline in [
        RuntimeTypeSchema::Record {
            name: "n2".to_owned(),
            fields: vec![],
            deny_unknown_fields: true,
        },
        RuntimeTypeSchema::Enum {
            name: "n2".to_owned(),
            variants: vec![],
            tag: super::super::RuntimeEnumTagStyle::External,
            repr: None,
        },
    ] {
        assert!(matches!(
            RuntimeNominalSchemaGraph::try_new(
                vec![unit(1, vec![inline])],
                RuntimeSchemaLimits::engine_default()
            ),
            Err(RuntimeNominalSchemaGraphError::InlineNominalDefinition { .. })
        ));
    }
}

#[test]
fn graph_opaque_fields_share_the_complete_value_and_byte_budget() {
    let owner = RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("p").unwrap(),
        RuntimeSemanticTypeId::from_bytes([7; 32]),
    );
    let schema = || RuntimeTypeSchema::ExactOpaque {
        owner: owner.clone(),
        arguments: vec![RuntimeTypeSchema::Bool].into_boxed_slice(),
    };
    let graph = graph(vec![record(
        1,
        RuntimeNominalRecordShape::Tuple,
        vec![],
        vec![(None, schema()), (None, schema())],
    )]);
    let payload = || {
        owner
            .try_wrap(RuntimeValue::Seq(crate::value::RuntimeSeq::dense_units(2)))
            .unwrap()
    };
    let value = nominal_value(&graph, 1, vec![payload(), payload()]);
    let bytes = value.try_canonical_bytes(1_000_000).unwrap();
    let limits = RuntimeSchemaLimits {
        max_nodes: 9,
        max_depth: 3,
        max_encoded_bytes: bytes.len() as u64,
        ..RuntimeSchemaLimits::engine_default()
    };
    assert_eq!(
        graph
            .accepts_value(identity(1).semantic_identity(), &value, limits)
            .unwrap()
            .as_bytes(),
        blake3::hash(&bytes).as_bytes()
    );
    assert!(matches!(
        graph.accepts_value(
            identity(1).semantic_identity(),
            &value,
            RuntimeSchemaLimits {
                max_nodes: 8,
                ..limits
            }
        ),
        Err(RuntimeSchemaError::BudgetExceeded { budget: "nodes" })
    ));
    assert!(matches!(
        graph.accepts_value(
            identity(1).semantic_identity(),
            &value,
            RuntimeSchemaLimits {
                max_encoded_bytes: limits.max_encoded_bytes - 1,
                ..limits
            }
        ),
        Err(RuntimeSchemaError::BudgetExceeded {
            budget: "encoded_bytes"
        })
    ));
    let wrong = RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("q").unwrap(),
        RuntimeSemanticTypeId::from_bytes([7; 32]),
    );
    let value = nominal_value(
        &graph,
        1,
        vec![wrong.try_wrap(RuntimeValue::Unit).unwrap(), payload()],
    );
    assert!(
        matches!(graph.accepts_value(identity(1).semantic_identity(), &value, limits), Err(RuntimeSchemaError::OpaqueOwner { path }) if path == "$[0]")
    );
    assert!(matches!(
        graph.accepts_value(
            identity(9).semantic_identity(),
            &value,
            RuntimeSchemaLimits {
                max_nodes: 0,
                ..limits
            }
        ),
        Err(RuntimeSchemaError::BudgetExceeded { budget: "nodes" })
    ));
}

#[test]
fn unit_graph_has_an_exact_version_one_document() {
    let graph = graph(vec![unit(1, vec![])]);
    let mut expected = b"arcweft.nominal-schema-graph\0".to_vec();
    expected.push(1);
    expected.extend_from_slice(&[1; 32]);
    expected.extend_from_slice(&[1, 2, b'n', b'1']);
    expected.extend_from_slice(&[1; 32]);
    // Version one commits the optional codec policy after the nominal body;
    // this annotation-free unit has the explicit absent-policy marker.
    expected.extend_from_slice(&[0, 0, 0, 0, 0]);
    assert_eq!(
        encoding::bytes(&graph, identity(1).semantic_identity()).unwrap(),
        expected
    );
    assert_eq!(
        hash(&graph, 1).as_bytes(),
        blake3::hash(&expected).as_bytes()
    );
    assert_eq!(
        graph.try_layouts().unwrap().as_ref(),
        &[(identity(1).semantic_identity(), hash(&graph, 1))]
    );
}

#[test]
fn empty_shapes_and_one_field_forms_have_distinct_layouts() {
    let mut layouts = BTreeSet::new();
    for shape in [
        RuntimeNominalRecordShape::Unit,
        RuntimeNominalRecordShape::Tuple,
        RuntimeNominalRecordShape::Record,
    ] {
        assert!(layouts.insert(hash(&graph(vec![record(1, shape, vec![], vec![])]), 1)));
    }
    for shape in [
        RuntimeNominalRecordShape::Tuple,
        RuntimeNominalRecordShape::Newtype,
    ] {
        assert!(layouts.insert(hash(
            &graph(vec![record(
                1,
                shape,
                vec![],
                vec![(None, RuntimeTypeSchema::Bool)]
            )]),
            1
        )));
    }
}

fn recursive_definitions() -> Vec<RuntimeNominalSchemaDefinition> {
    vec![
        record(
            1,
            RuntimeNominalRecordShape::Tuple,
            vec![],
            vec![(None, RuntimeTypeSchema::NominalRef(identity(2)))],
        ),
        RuntimeNominalSchemaDefinition::new(
            identity(2),
            vec![],
            RuntimeNominalSchemaBody::Variant {
                cases: vec![RuntimeNominalSchemaCase::new(
                    0,
                    "Again".to_owned(),
                    Some(RuntimeTypeSchema::option(RuntimeTypeSchema::NominalRef(
                        identity(1),
                    ))),
                )]
                .into_boxed_slice(),
            },
        ),
    ]
}

#[test]
fn recursive_layouts_ignore_input_order_and_unrelated_definitions() {
    let original = graph(recursive_definitions());
    let mut reversed = recursive_definitions();
    reversed.reverse();
    reversed.push(unit(3, vec![RuntimeTypeSchema::String]));
    let reversed = graph(reversed);
    assert_eq!(hash(&original, 1), hash(&reversed, 1));
    assert_eq!(hash(&original, 2), hash(&reversed, 2));
    assert_ne!(hash(&original, 1), hash(&original, 2));
    assert_eq!(
        encoding::bytes(&original, identity(1).semantic_identity()).unwrap(),
        encoding::bytes(&reversed, identity(1).semantic_identity()).unwrap()
    );
}

#[test]
fn unused_ordered_arguments_and_argument_only_descendants_are_retained() {
    let make = |arguments| graph(vec![unit(1, arguments), unit(2, vec![])]);
    let reference = RuntimeTypeSchema::NominalRef(identity(2));
    let ordered = make(vec![RuntimeTypeSchema::Bool, reference.clone()]);
    let reversed = make(vec![reference.clone(), RuntimeTypeSchema::Bool]);
    let repeated = make(vec![reference.clone(), reference.clone()]);
    let single = make(vec![reference.clone()]);
    assert_ne!(hash(&ordered, 1), hash(&reversed, 1));
    assert_ne!(hash(&repeated, 1), hash(&single, 1));
    assert_eq!(
        repeated
            .definition(identity(1).semantic_identity())
            .unwrap()
            .arguments(),
        &[reference.clone(), reference.clone()]
    );
    let changed_descendant = graph(vec![
        unit(1, vec![reference]),
        record(2, RuntimeNominalRecordShape::Tuple, vec![], vec![]),
    ]);
    assert_ne!(hash(&single, 1), hash(&changed_descendant, 1));
    let distinct_empty_instances = graph(vec![
        unit(1, vec![RuntimeTypeSchema::Bool]),
        unit(2, vec![RuntimeTypeSchema::String]),
    ]);
    assert_ne!(
        hash(&distinct_empty_instances, 1),
        hash(&distinct_empty_instances, 2)
    );
}

#[test]
fn duplicate_conflicting_and_dangling_identities_reject() {
    let limits = RuntimeSchemaLimits::engine_default();
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(vec![unit(1, vec![]), unit(1, vec![])], limits),
        Err(RuntimeNominalSchemaGraphError::DuplicateIdentity { .. })
    ));
    let mut conflicting = unit(2, vec![]);
    conflicting.identity.semantic_identity = identity(1).semantic_identity;
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(vec![unit(1, vec![]), conflicting], limits),
        Err(RuntimeNominalSchemaGraphError::ConflictingSemanticIdentity { .. })
    ));
    let mut conflicting = unit(2, vec![]);
    conflicting.identity.nominal = identity(1).nominal;
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(vec![unit(1, vec![]), conflicting], limits),
        Err(RuntimeNominalSchemaGraphError::ConflictingNominalIdentity { .. })
    ));
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(
            vec![unit(1, vec![RuntimeTypeSchema::NominalRef(identity(2))])],
            limits
        ),
        Err(RuntimeNominalSchemaGraphError::DanglingReference { .. })
    ));
    let mismatched =
        RuntimeNominalSchemaIdentity::new(identity(2).nominal, identity(1).semantic_identity);
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(
            vec![unit(1, vec![RuntimeTypeSchema::NominalRef(mismatched)])],
            limits
        ),
        Err(RuntimeNominalSchemaGraphError::DanglingReference { .. })
    ));
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(
            vec![unit(1, vec![RuntimeTypeSchema::Named("n1".to_owned())])],
            limits
        ),
        Err(RuntimeNominalSchemaGraphError::NonNominalReference { .. })
    ));
}

#[test]
fn field_names_ids_and_variant_ordinals_are_checked_before_publication() {
    let limits = RuntimeSchemaLimits::engine_default();
    for fields in [
        vec![(None, RuntimeTypeSchema::Unit)],
        vec![(Some(""), RuntimeTypeSchema::Unit)],
        vec![
            (Some("x"), RuntimeTypeSchema::Unit),
            (Some("x"), RuntimeTypeSchema::Unit),
        ],
    ] {
        assert!(matches!(
            RuntimeNominalSchemaGraph::try_new(
                vec![record(1, RuntimeNominalRecordShape::Record, vec![], fields)],
                limits
            ),
            Err(RuntimeNominalSchemaGraphError::RecordShape { .. })
        ));
    }
    let mut wrong_id = record(
        1,
        RuntimeNominalRecordShape::Tuple,
        vec![],
        vec![(None, RuntimeTypeSchema::Unit)],
    );
    let RuntimeNominalSchemaBody::Record { fields, .. } = &mut wrong_id.body else {
        unreachable!()
    };
    fields[0].field = RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap();
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(vec![wrong_id], limits),
        Err(RuntimeNominalSchemaGraphError::InvalidFieldIdentity { .. })
    ));
    let variant = |cases: Vec<_>| {
        RuntimeNominalSchemaDefinition::new(
            identity(1),
            vec![],
            RuntimeNominalSchemaBody::Variant {
                cases: cases.into_boxed_slice(),
            },
        )
    };
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(
            vec![variant(vec![RuntimeNominalSchemaCase::new(
                1,
                "Case".to_owned(),
                None
            )])],
            limits
        ),
        Err(RuntimeNominalSchemaGraphError::InvalidCaseOrdinal { .. })
    ));
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(
            vec![variant(vec![RuntimeNominalSchemaCase::new(
                0,
                String::new(),
                None
            )])],
            limits
        ),
        Err(RuntimeNominalSchemaGraphError::EmptyCaseName { .. })
    ));
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(
            vec![variant(vec![
                RuntimeNominalSchemaCase::new(0, "Case".to_owned(), None),
                RuntimeNominalSchemaCase::new(1, "Case".to_owned(), None)
            ])],
            limits
        ),
        Err(RuntimeNominalSchemaGraphError::DuplicateCaseName { .. })
    ));
}

#[test]
fn scalar_collection_node_depth_and_encoding_limits_are_inclusive() {
    let make = || unit(1, vec![RuntimeTypeSchema::option(RuntimeTypeSchema::Bool)]);
    let generous = graph(vec![make()]);
    let length = encoding::bytes(&generous, identity(1).semantic_identity())
        .unwrap()
        .len();
    let limits = RuntimeSchemaLimits {
        max_validation_work: RuntimeSchemaLimits::engine_default().max_validation_work,
        max_depth: 2,
        max_nodes: 5,
        max_sequence_items: 2,
        max_string_bytes: 4,
        max_encoded_bytes: u64::try_from(length).unwrap(),
    };
    let accepted = RuntimeNominalSchemaGraph::try_new(vec![make()], limits).unwrap();
    assert_eq!(hash(&accepted, 1), hash(&generous, 1));
    for (limit, expected) in [
        (
            RuntimeSchemaLimits {
                max_depth: 1,
                ..limits
            },
            "depth",
        ),
        (
            RuntimeSchemaLimits {
                max_nodes: 4,
                ..limits
            },
            "nodes",
        ),
        (
            RuntimeSchemaLimits {
                max_sequence_items: 1,
                ..limits
            },
            "sequence_items",
        ),
        (
            RuntimeSchemaLimits {
                max_string_bytes: 3,
                ..limits
            },
            "string_bytes",
        ),
    ] {
        assert!(
            matches!(RuntimeNominalSchemaGraph::try_new(vec![make()], limit), Err(RuntimeNominalSchemaGraphError::BudgetExceeded { budget }) if budget == expected)
        );
    }
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(
            vec![make()],
            RuntimeSchemaLimits {
                max_encoded_bytes: limits.max_encoded_bytes - 1,
                ..limits
            }
        ),
        Err(RuntimeNominalSchemaGraphError::Encoding(
            RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes"
            }
        ))
    ));
}

#[test]
fn all_layouts_share_work_and_byte_allowances() {
    let limits = RuntimeSchemaLimits {
        max_nodes: 9,
        ..RuntimeSchemaLimits::engine_default()
    };
    let recursive = RuntimeNominalSchemaGraph::try_new(recursive_definitions(), limits).unwrap();
    recursive
        .try_layout_hash(identity(1).semantic_identity())
        .unwrap();
    recursive
        .try_layout_hash(identity(2).semantic_identity())
        .unwrap();
    assert!(matches!(
        recursive.try_layouts(),
        Err(RuntimeNominalSchemaGraphError::BudgetExceeded { budget: "nodes" })
    ));
    let definitions = || vec![unit(1, vec![]), unit(2, vec![])];
    let generous = graph(definitions());
    let bytes = encoding::bytes(&generous, identity(1).semantic_identity())
        .unwrap()
        .len()
        + encoding::bytes(&generous, identity(2).semantic_identity())
            .unwrap()
            .len();
    let exact = RuntimeSchemaLimits {
        max_nodes: 2,
        max_encoded_bytes: u64::try_from(bytes).unwrap(),
        ..RuntimeSchemaLimits::engine_default()
    };
    let accepted = RuntimeNominalSchemaGraph::try_new(definitions(), exact).unwrap();
    assert_eq!(accepted.try_layouts().unwrap().len(), 2);
    let short = RuntimeNominalSchemaGraph::try_new(
        definitions(),
        RuntimeSchemaLimits {
            max_encoded_bytes: exact.max_encoded_bytes - 1,
            ..exact
        },
    )
    .unwrap();
    assert!(matches!(
        short.try_layouts(),
        Err(RuntimeNominalSchemaGraphError::Encoding(
            RuntimeSchemaError::BudgetExceeded {
                budget: "encoded_bytes"
            }
        ))
    ));
}

#[test]
fn deep_graph_admission_hashing_and_rejection_cleanup_are_iterative() {
    let depth = 20_000;
    let make = || {
        let mut schema = RuntimeTypeSchema::Unit;
        for _ in 0..depth {
            schema = RuntimeTypeSchema::option(schema);
        }
        vec![unit(1, vec![schema])]
    };
    let limits = RuntimeSchemaLimits {
        max_depth: depth + 1,
        max_nodes: depth * 3 + 2,
        ..RuntimeSchemaLimits::engine_default()
    };
    let accepted = RuntimeNominalSchemaGraph::try_new(make(), limits).unwrap();
    accepted
        .try_layout_hash(identity(1).semantic_identity())
        .unwrap();
    drop(accepted);
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(make(), RuntimeSchemaLimits::engine_default()),
        Err(RuntimeNominalSchemaGraphError::BudgetExceeded { budget: "depth" })
    ));
}

#[test]
fn opaque_identity_atoms_arguments_and_handle_kinds_change_the_layout() {
    let make = |producer, semantic, class, persistence, argument| {
        graph(vec![unit(
            1,
            vec![RuntimeTypeSchema::ExactOpaque {
                owner: RuntimeOpaqueTypeOwner::exact_with(
                    RuntimeOpaqueTypeProducerId::try_new(producer).unwrap(),
                    identity(semantic).semantic_identity,
                    class,
                    persistence,
                ),
                arguments: vec![argument].into_boxed_slice(),
            }],
        )])
    };
    let plain = RuntimeOpaqueValueClass::Plain;
    let persistent = RuntimeOpaquePersistence::ConstantAndSnapshot;
    let baseline = hash(&make("p", 2, plain, persistent, RuntimeTypeSchema::Bool), 1);
    for changed in [
        make("q", 2, plain, persistent, RuntimeTypeSchema::Bool),
        make("p", 3, plain, persistent, RuntimeTypeSchema::Bool),
        make(
            "p",
            2,
            plain,
            RuntimeOpaquePersistence::SnapshotOnly,
            RuntimeTypeSchema::Bool,
        ),
        make("p", 2, plain, persistent, RuntimeTypeSchema::String),
    ] {
        assert_ne!(baseline, hash(&changed, 1));
    }
    let cue = make(
        "p",
        2,
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
        persistent,
        RuntimeTypeSchema::Bool,
    );
    let voice = make(
        "p",
        2,
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Voice),
        persistent,
        RuntimeTypeSchema::Bool,
    );
    assert_ne!(hash(&cue, 1), hash(&voice, 1));
    let wide = RuntimeTypeSchema::ExactOpaque {
        owner: RuntimeOpaqueTypeOwner::with_admission(
            RuntimeOpaqueTypeProducerId::try_new("p").unwrap(),
            identity(2).semantic_identity,
            RuntimeOpaqueTypeAdmission::ProducerWide,
            plain,
            persistent,
        ),
        arguments: vec![].into_boxed_slice(),
    };
    assert!(matches!(
        RuntimeNominalSchemaGraph::try_new(
            vec![unit(1, vec![wide])],
            RuntimeSchemaLimits::engine_default()
        ),
        Err(RuntimeNominalSchemaGraphError::NonExactOpaque { .. })
    ));
}

#[test]
fn standalone_operations_cannot_publish_an_unresolved_nominal_reference() {
    let schema = RuntimeTypeSchema::Tuple(
        vec![RuntimeTypeSchema::NominalRef(identity(1))].into_boxed_slice(),
    );
    assert!(matches!(
        schema.try_layout_hash(),
        Err(RuntimeSchemaError::NominalGraphRequired { .. })
    ));
    assert!(matches!(
        schema.validate_value(
            &RuntimeValue::Tuple(vec![RuntimeValue::Unit]),
            RuntimeSchemaLimits::engine_default()
        ),
        Err(RuntimeSchemaError::NominalGraphRequired { .. })
    ));
}

#[test]
fn raw_serde_definitions_cannot_bypass_identity_admission() {
    let mut raw = serde_json::to_value(unit(1, vec![])).unwrap();
    raw["identity"]["nominal"] = serde_json::json!("");
    if let Ok(definition) = serde_json::from_value::<RuntimeNominalSchemaDefinition>(raw) {
        assert!(matches!(
            RuntimeNominalSchemaGraph::try_new(
                vec![definition],
                RuntimeSchemaLimits::engine_default()
            ),
            Err(RuntimeNominalSchemaGraphError::InvalidIdentity { .. })
        ));
    }
}
