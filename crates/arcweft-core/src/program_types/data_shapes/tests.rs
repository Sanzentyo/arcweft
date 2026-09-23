use super::*;
use crate::{
    awbc::schema::{AwbcProgram, AwbcRuntimeType, AwbcStringId, AwbcTypeId, AwbcVariantCase},
    entry::{RuntimeBytesFormat, RuntimeMapKind},
    plan::{RuntimePlan, RuntimePlanBuilder, RuntimePlanSequenceKind, RuntimePlanTypeSeed},
};

fn semantic(index: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([index; 32])
}

#[test]
fn data_shape_transparency_requires_a_selected_one_slot_policy() {
    let policy = RuntimeCodecUse::Newtype {
        inner: Box::new(RuntimeCodecUse::Plain),
    };
    policy
        .validate_schema(
            &crate::entry::RuntimeTypeSchema::Tuple(Box::new([
                crate::entry::RuntimeTypeSchema::Bool,
            ])),
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
    assert!(
        policy
            .validate_schema(
                &crate::entry::RuntimeTypeSchema::Tuple(Box::new([])),
                RuntimeSchemaLimits::engine_default()
            )
            .is_err()
    );
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(semantic(1), PlanType::Bool),
                RuntimePlanTypeSeed::new(semantic(2), PlanType::Tuple(Box::new([semantic(1)])))
                    .with_data_codec(policy.clone()),
                RuntimePlanTypeSeed::new(semantic(3), PlanType::Tuple(Box::new([semantic(1)]))),
            ],
            [],
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    let awbc = AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::new(semantic(1), AwbcType::Bool),
            AwbcRuntimeType::new(semantic(2), AwbcType::Tuple(vec![AwbcTypeId(0)]))
                .with_data_codec(policy.clone()),
            AwbcRuntimeType::new(semantic(3), AwbcType::Tuple(vec![AwbcTypeId(0)])),
        ],
        ..AwbcProgram::default()
    };
    for types in [
        RuntimeProgramTypes::Plan(&plan),
        RuntimeProgramTypes::Awbc(&awbc),
    ] {
        let shapes = RuntimeProgramDataShapes::new(types);
        let root = shapes
            .root(semantic(2), RuntimeSchemaLimits::engine_default())
            .unwrap()
            .referenced_id()
            .unwrap();
        let child = shapes.transparent_child(root).unwrap().unwrap();
        assert_eq!(shapes.semantic_type(child), Some(semantic(1)));
        assert_eq!(
            ShapeRef::Id(root).resolve(&shapes).unwrap().as_ref(),
            &TypeShape::Bool
        );
        let tuple = shapes
            .root(semantic(3), RuntimeSchemaLimits::engine_default())
            .unwrap()
            .referenced_id()
            .unwrap();
        assert!(shapes.transparent_child(tuple).unwrap().is_none());
    }
    let wrong = AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::new(semantic(1), AwbcType::Tuple(vec![])).with_data_codec(policy),
        ],
        ..AwbcProgram::default()
    };
    assert!(
        RuntimeProgramDataShapes::new(RuntimeProgramTypes::Awbc(&wrong))
            .validate_codec_uses(RuntimeSchemaLimits::engine_default())
            .is_err()
    );
}

fn programs() -> (RuntimePlan, AwbcProgram) {
    let mut builder = RuntimePlanBuilder::new();
    let rows = [
        PlanType::Bool,
        PlanType::Tuple(Box::new([semantic(1)])),
        PlanType::Option {
            item: semantic(1),
            some_payload: semantic(2),
        },
        PlanType::String,
        PlanType::Map {
            kind: RuntimeMapKind::BTree,
            key: semantic(4),
            value: semantic(3),
        },
        PlanType::Sequence {
            kind: RuntimePlanSequenceKind::Vec,
            item: semantic(5),
        },
        PlanType::Tuple(Box::new([semantic(6), semantic(6)])),
    ];
    builder
        .admit_type_batch(
            rows.into_iter().enumerate().map(|(index, projection)| {
                RuntimePlanTypeSeed::new(semantic(u8::try_from(index + 1).unwrap()), projection)
            }),
            [],
        )
        .unwrap();
    let awbc = AwbcProgram {
        strings: vec!["None".to_owned(), "Some".to_owned()],
        runtime_types: [
            AwbcType::Bool,
            AwbcType::Tuple(vec![AwbcTypeId(0)]),
            AwbcType::Variant {
                owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                arguments: vec![],
                cases: vec![
                    AwbcVariantCase {
                        name: AwbcStringId(1),
                        payload: Some(AwbcTypeId(1)),
                    },
                    AwbcVariantCase {
                        name: AwbcStringId(0),
                        payload: None,
                    },
                ],
            },
            AwbcType::String,
            AwbcType::Map {
                kind: RuntimeMapKind::BTree,
                key: AwbcTypeId(3),
                value: AwbcTypeId(2),
            },
            AwbcType::Sequence(AwbcTypeId(4)),
            AwbcType::Tuple(vec![AwbcTypeId(5), AwbcTypeId(5)]),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, shape)| {
            AwbcRuntimeType::new(semantic(u8::try_from(index + 1).unwrap()), shape)
        })
        .collect(),
        ..AwbcProgram::default()
    };
    (builder.finish().unwrap(), awbc)
}

#[test]
fn data_shape_access_projects_original_rows_and_shared_children_without_inference() {
    let (plan, awbc) = programs();
    for types in [
        RuntimeProgramTypes::Plan(&plan),
        RuntimeProgramTypes::Awbc(&awbc),
    ] {
        let shapes = RuntimeProgramDataShapes::new(types);
        let root = shapes
            .root(semantic(7), RuntimeSchemaLimits::engine_default())
            .unwrap();
        let root = root.resolve(&shapes).unwrap();
        let TypeShape::Tuple(children) = root.as_ref() else {
            panic!("tuple");
        };
        assert_eq!(children.len(), 2);
        let [TypeShape::Ref(left), TypeShape::Ref(right)] = children.as_slice() else {
            panic!("references");
        };
        assert_eq!(left, right);
        assert_eq!(shapes.semantic_type(*left), Some(semantic(6)));
        let sequence = shapes.get_shape(*left).unwrap();
        let TypeShape::Seq(item) = sequence.as_ref() else {
            panic!("sequence");
        };
        let TypeShape::Ref(map) = item.as_ref() else {
            panic!("map reference");
        };
        let map = shapes.get_shape(*map).unwrap();
        let TypeShape::Map { kind, value, .. } = map.as_ref() else {
            panic!("map");
        };
        assert_eq!(*kind, arcweft_data::MapKind::BTree);
        let TypeShape::Ref(option) = value.as_ref() else {
            panic!("option reference");
        };
        let option = shapes.get_shape(*option).unwrap();
        let TypeShape::Option(item) = option.as_ref() else {
            panic!("option");
        };
        let TypeShape::Ref(boolean) = item.as_ref() else {
            panic!("boolean reference");
        };
        assert_eq!(shapes.semantic_type(*boolean), Some(semantic(1)));
        assert_eq!(
            shapes.get_shape(*boolean).unwrap().as_ref(),
            &TypeShape::Bool
        );
        assert!(shapes.get_shape(ShapeId::new(usize::MAX)).is_none());
    }
}

#[test]
fn data_shape_access_uses_selected_bytes_policy_and_rejects_budget_overflow() {
    for format in [
        RuntimeBytesFormat::Binary,
        RuntimeBytesFormat::Base64,
        RuntimeBytesFormat::Hex,
        RuntimeBytesFormat::Array,
    ] {
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(semantic(1), PlanType::Bytes)
                    .with_data_codec(crate::entry::schema::RuntimeCodecUse::Bytes { format })],
                [],
            )
            .unwrap();
        let program = builder.finish().unwrap();
        let shapes = RuntimeProgramDataShapes::new(RuntimeProgramTypes::Plan(&program));
        let root = shapes
            .root(semantic(1), RuntimeSchemaLimits::engine_default())
            .unwrap();
        assert_eq!(
            root.resolve(&shapes).unwrap().as_ref(),
            &TypeShape::Bytes {
                format: format.into()
            }
        );
        let limits = RuntimeSchemaLimits {
            max_nodes: 0,
            ..RuntimeSchemaLimits::engine_default()
        };
        assert!(matches!(
            shapes.root(semantic(1), limits),
            Err(RuntimeProgramDataShapeError::Limit { limit: "max_nodes" })
        ));
    }
}

fn recursive_codec_fixture() -> (RuntimePlan, AwbcProgram) {
    use crate::entry::schema::{RuntimeCodecUse as Use, RuntimeFieldCodecUse};
    use crate::entry::{
        RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaField, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId, RuntimeTypeSchema,
    };
    use crate::plan::{RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed};
    use crate::value::RuntimeRecordFieldId;
    let limits = RuntimeSchemaLimits::engine_default();
    let nominal = RuntimeNominalTypeId::try_new("fixture.Config").unwrap();
    let identity = RuntimeNominalSchemaIdentity::new(nominal.clone(), semantic(1));
    let fields = [
        (
            "left",
            RuntimeTypeSchema::Seq(Box::new(RuntimeTypeSchema::Bytes {
                format: RuntimeBytesFormat::Base64,
            })),
        ),
        (
            "right",
            RuntimeTypeSchema::Seq(Box::new(RuntimeTypeSchema::Bytes {
                format: RuntimeBytesFormat::Hex,
            })),
        ),
        (
            "next",
            RuntimeTypeSchema::option(RuntimeTypeSchema::NominalRef(identity.clone())),
        ),
    ];
    let codec = Use::Record {
        name: "fixture.Config".to_owned(),
        deny_unknown_fields: true,
        fields: fields
            .iter()
            .map(|(name, schema)| RuntimeFieldCodecUse {
                wire_name: format!("wire_{name}"),
                has_default: false,
                default_program: None,
                skip: false,
                bytes_format: None,
                value: Use::from_schema(schema, limits).unwrap(),
            })
            .collect(),
    };
    let argument = RuntimeTypeSchema::Bytes {
        format: RuntimeBytesFormat::Array,
    };
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![
            RuntimeNominalSchemaDefinition::new(
                identity,
                vec![argument.clone()],
                RuntimeNominalSchemaBody::Record {
                    shape: RuntimeNominalRecordShape::Record,
                    fields: fields
                        .iter()
                        .enumerate()
                        .map(|(ordinal, (name, schema))| {
                            RuntimeNominalSchemaField::new(
                                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                                Some((*name).to_owned()),
                                schema.clone(),
                            )
                        })
                        .collect(),
                },
            )
            .with_data_codec(codec.clone()),
        ],
        limits,
    )
    .unwrap();
    let layout = graph.try_layout_hash(semantic(1)).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(
                    semantic(1),
                    PlanType::Nominal {
                        nominal: nominal.clone(),
                        layout,
                        arguments: Box::new([semantic(2)]),
                    },
                ),
                RuntimePlanTypeSeed::new(semantic(2), PlanType::Bytes),
                RuntimePlanTypeSeed::new(
                    semantic(3),
                    PlanType::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: semantic(2),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    semantic(4),
                    PlanType::Option {
                        item: semantic(1),
                        some_payload: semantic(5),
                    },
                ),
                RuntimePlanTypeSeed::new(semantic(5), PlanType::Tuple(Box::new([semantic(1)]))),
            ],
            [],
            [RuntimeNominalRecordDomainSeed::new(
                semantic(1),
                RuntimeNominalRecordShape::Record,
                [
                    ("left", semantic(3)),
                    ("right", semantic(3)),
                    ("next", semantic(4)),
                ]
                .into_iter()
                .enumerate()
                .map(|(ordinal, (name, ty))| {
                    RuntimeNominalRecordDomainFieldSeed::new(
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                        Some(name.to_owned()),
                        ty,
                    )
                }),
            )],
            [],
            &graph,
        )
        .unwrap();
    let awbc = AwbcProgram {
        strings: ["None", "Some", "fixture.Config", "left", "next", "right"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        runtime_types: vec![
            AwbcRuntimeType::new(
                semantic(1),
                AwbcType::NominalRecord {
                    public_id: AwbcStringId(2),
                    layout: *layout.as_bytes(),
                    arguments: vec![AwbcTypeId(1)],
                    shape: RuntimeNominalRecordShape::Record,
                    fields: [(3, 2), (5, 2), (4, 3)]
                        .into_iter()
                        .enumerate()
                        .map(
                            |(ordinal, (name, ty))| crate::awbc::schema::AwbcRecordField {
                                field: RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                                    .unwrap(),
                                name: Some(AwbcStringId(name)),
                                ty: AwbcTypeId(ty),
                            },
                        )
                        .collect(),
                },
            )
            .with_data_codec(codec)
            .with_data_codec_arguments(vec![Use::from_schema(&argument, limits).unwrap()]),
            AwbcRuntimeType::new(semantic(2), AwbcType::Bytes),
            AwbcRuntimeType::new(semantic(3), AwbcType::Sequence(AwbcTypeId(1))),
            AwbcRuntimeType::new(
                semantic(4),
                AwbcType::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                    arguments: vec![],
                    cases: vec![
                        AwbcVariantCase {
                            name: AwbcStringId(1),
                            payload: Some(AwbcTypeId(4)),
                        },
                        AwbcVariantCase {
                            name: AwbcStringId(0),
                            payload: None,
                        },
                    ],
                },
            ),
            AwbcRuntimeType::new(semantic(5), AwbcType::Tuple(vec![AwbcTypeId(0)])),
        ],
        ..AwbcProgram::default()
    };
    (builder.finish().unwrap(), awbc)
}

#[test]
fn data_shape_occurrences_preserve_recursive_nominals_and_shared_bytes_use_policies() {
    let (plan, awbc) = recursive_codec_fixture();
    for types in [
        RuntimeProgramTypes::Plan(&plan),
        RuntimeProgramTypes::Awbc(&awbc),
    ] {
        let shapes = RuntimeProgramDataShapes::new(types);
        shapes
            .validate_codec_uses(RuntimeSchemaLimits::engine_default())
            .unwrap();
        let root = shapes
            .root(semantic(1), RuntimeSchemaLimits::engine_default())
            .unwrap();
        let record = root.resolve(&shapes).unwrap();
        let TypeShape::Record { fields, .. } = record.as_ref() else {
            panic!("record");
        };
        assert_eq!(fields[0].rust_name, "left");
        assert_eq!(fields[0].wire_name, "wire_left");
        let mut byte_occurrences = Vec::new();
        for (index, expected) in [
            (0, arcweft_data::BytesFormat::Base64),
            (1, arcweft_data::BytesFormat::Hex),
        ] {
            let TypeShape::Ref(sequence) = fields[index].shape else {
                panic!("sequence occurrence");
            };
            assert_eq!(shapes.semantic_type(sequence), Some(semantic(3)));
            let sequence = shapes.get_shape(sequence).unwrap();
            let TypeShape::Seq(item) = sequence.as_ref() else {
                panic!("sequence");
            };
            let TypeShape::Ref(byte) = item.as_ref() else {
                panic!("byte occurrence");
            };
            assert_eq!(shapes.semantic_type(*byte), Some(semantic(2)));
            assert_eq!(
                shapes.get_shape(*byte).unwrap().as_ref(),
                &TypeShape::Bytes { format: expected }
            );
            byte_occurrences.push(*byte);
        }
        assert_ne!(byte_occurrences[0], byte_occurrences[1]);
        let TypeShape::Ref(next) = fields[2].shape else {
            panic!("next occurrence");
        };
        let next = shapes.get_shape(next).unwrap();
        let TypeShape::Option(item) = next.as_ref() else {
            panic!("option");
        };
        let TypeShape::Ref(recursive) = item.as_ref() else {
            panic!("nominal back edge");
        };
        assert_eq!(ShapeRef::Id(*recursive).resolve(&shapes).unwrap(), record);
    }
}

#[test]
fn data_shape_occurrence_topology_mismatch_is_rejected_without_a_value() {
    let mut builder = RuntimePlanBuilder::new();
    assert!(
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(semantic(1), PlanType::Bool).with_data_codec(
                        crate::entry::schema::RuntimeCodecUse::Bytes {
                            format: RuntimeBytesFormat::Hex
                        }
                    )
                ],
                []
            )
            .is_err()
    );
    assert!(builder.finish().unwrap().type_table().is_empty());
    let wrong_scalar = AwbcProgram {
        runtime_types: vec![
            AwbcRuntimeType::new(semantic(1), AwbcType::Bool).with_data_codec(
                crate::entry::schema::RuntimeCodecUse::Bytes {
                    format: RuntimeBytesFormat::Hex,
                },
            ),
        ],
        ..AwbcProgram::default()
    };
    assert!(matches!(
        RuntimeProgramDataShapes::new(RuntimeProgramTypes::Awbc(&wrong_scalar))
            .validate_codec_uses(RuntimeSchemaLimits::engine_default()),
        Err(RuntimeProgramDataShapeError::PolicyMismatch { .. })
    ));
    let (_, mut awbc) = recursive_codec_fixture();
    let wrong = crate::entry::schema::RuntimeCodecUse::Plain;
    awbc.runtime_types[0] =
        AwbcRuntimeType::new(semantic(1), awbc.runtime_types[0].shape().clone())
            .with_data_codec(wrong);
    let shapes = RuntimeProgramDataShapes::new(RuntimeProgramTypes::Awbc(&awbc));
    assert!(matches!(
        shapes.validate_codec_uses(RuntimeSchemaLimits::engine_default()),
        Err(RuntimeProgramDataShapeError::PolicyMismatch { .. })
    ));
}

#[test]
fn data_shape_policy_budget_preflight_does_not_publish_a_partial_index() {
    let (_, awbc) = recursive_codec_fixture();
    let shapes = RuntimeProgramDataShapes::new(RuntimeProgramTypes::Awbc(&awbc));
    let defaults = RuntimeSchemaLimits::engine_default();
    for limits in [
        RuntimeSchemaLimits {
            max_depth: 1,
            ..defaults
        },
        RuntimeSchemaLimits {
            max_string_bytes: 4,
            ..defaults
        },
        RuntimeSchemaLimits {
            max_encoded_bytes: 1,
            ..defaults
        },
        RuntimeSchemaLimits {
            max_sequence_items: 2,
            ..defaults
        },
        RuntimeSchemaLimits {
            max_validation_work: 1,
            ..defaults
        },
        RuntimeSchemaLimits {
            max_nodes: 7,
            ..defaults
        },
    ] {
        assert!(shapes.validate_codec_uses(limits).is_err());
        assert!(shapes.occurrences.get().is_none());
    }
    shapes.validate_codec_uses(defaults).unwrap();
    assert!(shapes.occurrences.get().is_some());
    assert!(
        shapes
            .validate_codec_uses(RuntimeSchemaLimits {
                max_string_bytes: 4,
                ..defaults
            })
            .is_err()
    );
}

#[test]
fn data_shape_canonical_nominal_body_cannot_be_its_own_back_edge() {
    let (_, mut awbc) = recursive_codec_fixture();
    awbc.runtime_types[0] =
        AwbcRuntimeType::new(semantic(1), awbc.runtime_types[0].shape().clone())
            .with_data_codec(RuntimeCodecUse::NominalRef)
            .with_data_codec_arguments(vec![RuntimeCodecUse::Bytes {
                format: RuntimeBytesFormat::Array,
            }]);
    let shapes = RuntimeProgramDataShapes::new(RuntimeProgramTypes::Awbc(&awbc));
    assert!(matches!(
        shapes.validate_codec_uses(RuntimeSchemaLimits::engine_default()),
        Err(RuntimeProgramDataShapeError::PolicyMismatch { .. })
    ));
    assert!(
        shapes
            .root(semantic(1), RuntimeSchemaLimits::engine_default())
            .is_err()
    );
}

fn variant_codec_fixture(with_payload: bool) -> (RuntimePlan, AwbcProgram) {
    use crate::entry::schema::RuntimeVariantCodecUse;
    use crate::entry::{
        RuntimeEnumRepr, RuntimeEnumTagStyle, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
        RuntimeNominalSchemaDefinition, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId, RuntimeTypeSchema,
    };
    use crate::plan::{RuntimeVariantCaseSeed, RuntimeVariantDomainSeed};
    let limits = RuntimeSchemaLimits::engine_default();
    let nominal = RuntimeNominalTypeId::try_new("fixture.Packet").unwrap();
    let payload = with_payload.then(|| {
        RuntimeTypeSchema::Tuple(Box::new([RuntimeTypeSchema::Bytes {
            format: RuntimeBytesFormat::Base64,
        }]))
    });
    let codec = RuntimeCodecUse::Enum {
        name: "Packet".to_owned(),
        tag: RuntimeEnumTagStyle::Adjacent {
            tag: "kind".to_owned(),
            content: "data".to_owned(),
        },
        repr: (!with_payload).then_some(RuntimeEnumRepr::U16),
        cases: vec![
            RuntimeVariantCodecUse {
                wire_name: "wire_empty".to_owned(),
                discriminant: (!with_payload).then_some(4),
                payload: None,
            },
            RuntimeVariantCodecUse {
                wire_name: "wire_blob".to_owned(),
                discriminant: (!with_payload).then_some(20),
                payload: payload
                    .as_ref()
                    .map(|schema| RuntimeCodecUse::from_schema(schema, limits).unwrap()),
            },
        ]
        .into_boxed_slice(),
    };
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![
            RuntimeNominalSchemaDefinition::new(
                RuntimeNominalSchemaIdentity::new(nominal.clone(), semantic(1)),
                vec![],
                RuntimeNominalSchemaBody::Variant {
                    cases: vec![
                        RuntimeNominalSchemaCase::new(0, "Empty".to_owned(), None),
                        RuntimeNominalSchemaCase::new(1, "Blob".to_owned(), payload),
                    ]
                    .into_boxed_slice(),
                },
            )
            .with_data_codec(codec.clone()),
        ],
        limits,
    )
    .unwrap();
    let layout = graph.try_layout_hash(semantic(1)).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(
                    semantic(1),
                    PlanType::Nominal {
                        nominal: nominal.clone(),
                        layout,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(semantic(2), PlanType::Tuple(Box::new([semantic(3)]))),
                RuntimePlanTypeSeed::new(semantic(3), PlanType::Bytes),
            ],
            [],
            [],
            [RuntimeVariantDomainSeed::new(
                semantic(1),
                nominal,
                layout,
                [
                    RuntimeVariantCaseSeed::new("Empty", None),
                    RuntimeVariantCaseSeed::new("Blob", with_payload.then_some(semantic(2))),
                ],
            )],
            &graph,
        )
        .unwrap();
    let awbc = AwbcProgram {
        strings: ["Blob", "Empty", "fixture.Packet"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        runtime_types: vec![
            AwbcRuntimeType::new(
                semantic(1),
                AwbcType::Variant {
                    owner: AwbcVariantIdentity::Nominal {
                        public_id: AwbcStringId(2),
                        layout: *layout.as_bytes(),
                    },
                    arguments: vec![],
                    cases: vec![
                        AwbcVariantCase {
                            name: AwbcStringId(1),
                            payload: None,
                        },
                        AwbcVariantCase {
                            name: AwbcStringId(0),
                            payload: with_payload.then_some(AwbcTypeId(1)),
                        },
                    ],
                },
            )
            .with_data_codec(codec)
            .with_data_codec_arguments(vec![]),
            AwbcRuntimeType::new(semantic(2), AwbcType::Tuple(vec![AwbcTypeId(2)])),
            AwbcRuntimeType::new(semantic(3), AwbcType::Bytes),
        ],
        ..AwbcProgram::default()
    };
    (builder.finish().unwrap(), awbc)
}

#[test]
fn data_shape_enum_policies_preserve_selected_cases_payloads_tags_and_repr() {
    for with_payload in [false, true] {
        let (plan, awbc) = variant_codec_fixture(with_payload);
        for types in [
            RuntimeProgramTypes::Plan(&plan),
            RuntimeProgramTypes::Awbc(&awbc),
        ] {
            let shapes = RuntimeProgramDataShapes::new(types);
            shapes
                .validate_codec_uses(RuntimeSchemaLimits::engine_default())
                .unwrap();
            let root = shapes
                .root(semantic(1), RuntimeSchemaLimits::engine_default())
                .unwrap();
            let root = root.resolve(&shapes).unwrap();
            let TypeShape::Enum {
                name,
                variants,
                tag,
                repr,
            } = root.as_ref()
            else {
                panic!("enum");
            };
            assert_eq!(name, "Packet");
            assert_eq!(
                *tag,
                arcweft_data::EnumTagStyle::Adjacent {
                    tag: "kind".to_owned(),
                    content: "data".to_owned()
                }
            );
            assert_eq!(
                *repr,
                (!with_payload).then_some(arcweft_data::EnumRepr::U16)
            );
            assert_eq!(
                (&*variants[0].rust_name, &*variants[0].wire_name),
                ("Empty", "wire_empty")
            );
            assert_eq!(
                (&*variants[1].rust_name, &*variants[1].wire_name),
                ("Blob", "wire_blob")
            );
            assert_eq!(variants[0].discriminant, (!with_payload).then_some(4));
            assert_eq!(variants[1].discriminant, (!with_payload).then_some(20));
            assert!(variants[0].payload.is_none());
            if with_payload {
                let Some(TypeShape::Ref(payload)) = &variants[1].payload else {
                    panic!("payload reference");
                };
                let payload = shapes.get_shape(*payload).unwrap();
                let TypeShape::Tuple(items) = payload.as_ref() else {
                    panic!("tuple payload");
                };
                let [TypeShape::Ref(bytes)] = items.as_slice() else {
                    panic!("bytes occurrence");
                };
                assert_eq!(
                    shapes.get_shape(*bytes).unwrap().as_ref(),
                    &TypeShape::Bytes {
                        format: arcweft_data::BytesFormat::Base64
                    }
                );
            } else {
                assert!(variants[1].payload.is_none());
            }
        }
    }
    let (_, mut awbc) = variant_codec_fixture(true);
    let mut codec = awbc.runtime_types[0].data_codec().unwrap().clone();
    let RuntimeCodecUse::Enum { cases, .. } = &mut codec else {
        unreachable!();
    };
    cases.swap(0, 1);
    awbc.runtime_types[0] =
        AwbcRuntimeType::new(semantic(1), awbc.runtime_types[0].shape().clone())
            .with_data_codec(codec)
            .with_data_codec_arguments(vec![]);
    assert!(matches!(
        RuntimeProgramDataShapes::new(RuntimeProgramTypes::Awbc(&awbc))
            .validate_codec_uses(RuntimeSchemaLimits::engine_default()),
        Err(RuntimeProgramDataShapeError::PolicyMismatch { .. })
    ));
}
