use super::*;
use crate::{
    callable::{RustItemPath, RustPackageProvenance},
    env::{
        EnvironmentRecordField,
        nominal::{
            AcceptedNominalId, AcceptedNominalOrigin, AcceptedNominalOwnerId,
            AcceptedOpaqueRuntimeCarrier, RustPackageId,
        },
        rust_metadata::{
            RustStructMetadataInput, RustTypeMetadataPublicationIdentity,
            RustTypeMetadataPublicationInput, RustTypeMetadataPublicationKind,
            RustTypeParameterPublicationInput, RustVariantMetadataInput, RustVariantPayloadInput,
        },
    },
    final_analysis::{
        RuntimeAcceptedRustNominalKind, RuntimeNominalGraphProjectionError as ProjectionError,
        RuntimeNominalGraphProjectionLimitKind as LimitKind,
        RuntimeNominalGraphProjectionLimits as Limits,
    },
    registration::{AcceptedNominalInputVisibility, AcceptedNominalInventoryInput},
    types::AcceptedNominalType,
};
use arcweft_core::entry::{RuntimeNominalRecordShape, RuntimeNominalSchemaBody, RuntimeTypeSchema};
use arcweft_rust_abi::ArcweftRustTypeParameterIndex;

#[test]
fn project_root_admits_nested_joined_rust_values_from_one_source_graph() {
    use arcweft_core::{
        entry::RuntimeSchemaLimits,
        value::{RuntimeNominalRecordValue, RuntimeSeq, RuntimeValue},
    };

    let input = MetadataFixture::new();
    let fixture = input.build_with_source(
        vec![(
            "Payload",
            1,
            RustTypeMetadataPublicationKind::Newtype {
                inner: input.parameter(),
            },
        )],
        None,
        "struct State { values: Vec<Payload<bool>> }\nfn retain(value: State) -> State { value }\n",
    );
    let analysis = analyze(&fixture).unwrap();
    let state = project_nominal_expression_type(&analysis, "State");
    let projection = analysis
        .runtime_nominal_projection(state.semantic_identity_digest().unwrap())
        .unwrap();
    let rust = analysis
        .project_accepted_rust_nominal(
            &fixture.registered,
            &input.nominal("Payload", vec![TypeKind::Bool]),
            Limits::PRODUCTION,
        )
        .unwrap();
    let graph = projection.graph();
    assert_eq!(graph.definitions().len(), 2);
    assert_eq!(graph.try_layout_hash(rust.root()).unwrap(), rust.layout());
    let value = |nested| {
        RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
            projection.nominal().clone(),
            projection.semantic_identity(),
            projection.layout(),
            vec![RuntimeValue::Seq(RuntimeSeq::values(vec![
                RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
                    rust.nominal().clone(),
                    rust.root(),
                    rust.layout(),
                    vec![nested],
                )),
            ]))],
        ))
    };
    let limits = RuntimeSchemaLimits::engine_default();
    graph
        .accepts_value(
            projection.semantic_identity(),
            &value(RuntimeValue::Bool(true)),
            limits,
        )
        .unwrap();
    assert!(
        graph
            .accepts_value(
                projection.semantic_identity(),
                &value(RuntimeValue::Unit),
                limits
            )
            .is_err()
    );
}

struct MetadataFixture {
    document: Arc<SourceDocument>,
    adapter: AdapterPackageId,
    package: RustPackageId,
}

impl MetadataFixture {
    fn new() -> Self {
        Self {
            document: source_document(
                "arcweft-test://sema/rust-graph",
                "rust.environment",
                "Rust ADT metadata",
            ),
            adapter: AdapterPackageId::try_new("rust-graph-fixture").unwrap(),
            package: RustPackageId::try_new("rust_graph").unwrap(),
        }
    }

    fn source(&self) -> arcweft_source::SourceSpan {
        self.document
            .span(SourceRange::new(0, self.document.text().len()))
            .unwrap()
    }

    fn id(&self, name: &str) -> AcceptedNominalId {
        AcceptedNominalId::new(
            AcceptedNominalOwnerId::RustPackage(self.package.clone()),
            TypePath::from(
                ProjectSymbolPath::new(
                    ModulePathRoot::ImplicitCrate,
                    [ProjectSymbolSegment::try_new(name).unwrap()],
                )
                .unwrap(),
            ),
        )
    }

    fn node(&self, kind: EnvironmentTypeProjectionKind) -> EnvironmentTypeProjectionNode {
        EnvironmentTypeProjectionNode::new(self.source(), kind)
    }

    fn parameter(&self) -> EnvironmentTypeProjectionNode {
        self.node(EnvironmentTypeProjectionKind::TypeParameter {
            index: ArcweftRustTypeParameterIndex::try_from_usize(0).unwrap(),
        })
    }

    fn reference(
        &self,
        name: &str,
        arguments: Vec<EnvironmentTypeProjectionNode>,
    ) -> EnvironmentTypeProjectionNode {
        self.node(EnvironmentTypeProjectionKind::AcceptedNominal {
            id: self.id(name),
            arguments: arguments.into(),
        })
    }

    fn build(
        &self,
        rows: Vec<(&str, u16, RustTypeMetadataPublicationKind)>,
        opaque: Option<AcceptedNominalInventoryInput>,
    ) -> Fixture {
        self.build_with_source(rows, opaque, "fn main() {}")
    }

    fn build_with_source(
        &self,
        rows: Vec<(&str, u16, RustTypeMetadataPublicationKind)>,
        opaque: Option<AcceptedNominalInventoryInput>,
        source: &str,
    ) -> Fixture {
        let mut inventory = Vec::new();
        let mut metadata = Vec::new();
        for (name, arity, kind) in rows {
            let id = self.id(name);
            let rust_item = RustItemPath::try_new(format!("rust_graph::{name}")).unwrap();
            let item = EnvironmentPublicationItemId::RustType {
                adapter: self.adapter.clone(),
                package: self.package.clone(),
                rust_item: rust_item.clone(),
                accepted_path: id.canonical_path().clone(),
            };
            inventory.push(AcceptedNominalInventoryInput::new_rust_adt(
                id.clone(),
                arity,
                AcceptedNominalInputVisibility::Visible,
                self.source(),
                item.clone(),
            ));
            metadata.push(RustTypeMetadataPublicationInput::new(
                RustTypeMetadataPublicationIdentity::new(
                    item,
                    id,
                    self.package.clone(),
                    RustPackageProvenance::try_new("rust_graph", "1.0.0", None).unwrap(),
                    rust_item,
                ),
                (0..arity)
                    .map(|index| {
                        RustTypeParameterPublicationInput::new(
                            ArcweftRustTypeParameterIndex::try_from_usize(usize::from(index))
                                .unwrap(),
                            format!("T{index}"),
                            self.source(),
                        )
                    })
                    .collect::<Vec<_>>(),
                kind,
                self.source(),
            ));
        }
        inventory.extend(opaque);
        let input = SourceBackedEnvironmentRegistrationInput::new(
            EnvironmentCallableOwner::Adapter(self.adapter.clone()),
            self.document.identity().clone(),
            EnvironmentManifestDigest::from_bytes([71; 32]),
            inventory,
            [],
            metadata,
            [],
        );
        fixture_with_environment_inputs(source, None, vec![(Arc::clone(&self.document), input)])
    }

    fn nominal(&self, name: &str, arguments: Vec<TypeKind>) -> AcceptedNominalType {
        AcceptedNominalType::new(self.id(name), arguments)
    }

    fn recursive_kind(&self) -> RustTypeMetadataPublicationKind {
        RustTypeMetadataPublicationKind::Struct {
            shape: RustStructMetadataInput::Record(
                vec![
                    EnvironmentRecordField::new("value", self.parameter()),
                    EnvironmentRecordField::new(
                        "next".to_owned(),
                        self.node(EnvironmentTypeProjectionKind::Option(Box::new(
                            self.reference("Node", vec![self.parameter()]),
                        ))),
                    ),
                ]
                .into(),
            ),
        }
    }
}

#[test]
fn accepted_rust_shapes_preserve_empty_and_single_payload_forms() {
    let input = MetadataFixture::new();
    let integer = input.node(EnvironmentTypeProjectionKind::I32);
    let fixture = input.build(
        vec![
            (
                "UnitStruct",
                0,
                RustTypeMetadataPublicationKind::Struct {
                    shape: RustStructMetadataInput::Unit,
                },
            ),
            (
                "TupleStruct",
                0,
                RustTypeMetadataPublicationKind::Struct {
                    shape: RustStructMetadataInput::Tuple(vec![integer.clone()].into()),
                },
            ),
            (
                "RecordStruct",
                0,
                RustTypeMetadataPublicationKind::Struct {
                    shape: RustStructMetadataInput::Record(
                        vec![
                            EnvironmentRecordField::new("second", integer.clone()),
                            EnvironmentRecordField::new(
                                "first".to_owned(),
                                input.node(EnvironmentTypeProjectionKind::Bool),
                            ),
                        ]
                        .into(),
                    ),
                },
            ),
            (
                "Newtype",
                0,
                RustTypeMetadataPublicationKind::Newtype {
                    inner: integer.clone(),
                },
            ),
            (
                "Cases",
                0,
                RustTypeMetadataPublicationKind::Enum {
                    variants: vec![
                        RustVariantMetadataInput::new(
                            "Unit".to_owned(),
                            RustVariantPayloadInput::Unit,
                            input.source(),
                        ),
                        RustVariantMetadataInput::new(
                            "Tuple0".to_owned(),
                            RustVariantPayloadInput::Tuple(Box::new([])),
                            input.source(),
                        ),
                        RustVariantMetadataInput::new(
                            "Tuple1".to_owned(),
                            RustVariantPayloadInput::Tuple(vec![integer].into()),
                            input.source(),
                        ),
                        RustVariantMetadataInput::new(
                            "Record0".to_owned(),
                            RustVariantPayloadInput::Record(Box::new([])),
                            input.source(),
                        ),
                        RustVariantMetadataInput::new(
                            "Record1".to_owned(),
                            RustVariantPayloadInput::Record(
                                vec![EnvironmentRecordField::new(
                                    "enabled".to_owned(),
                                    input.node(EnvironmentTypeProjectionKind::Bool),
                                )]
                                .into(),
                            ),
                            input.source(),
                        ),
                    ]
                    .into(),
                },
            ),
        ],
        None,
    );
    let analysis = analyze(&fixture).unwrap();
    for (name, shape, count) in [
        ("UnitStruct", RuntimeNominalRecordShape::Unit, 0),
        ("TupleStruct", RuntimeNominalRecordShape::Tuple, 1),
        ("RecordStruct", RuntimeNominalRecordShape::Record, 2),
        ("Newtype", RuntimeNominalRecordShape::Newtype, 1),
    ] {
        let projection = analysis
            .project_accepted_rust_nominal(
                &fixture.registered,
                &input.nominal(name, vec![]),
                Limits::PRODUCTION,
            )
            .unwrap();
        assert_eq!(
            projection.kind(),
            RuntimeAcceptedRustNominalKind::Record(shape)
        );
        let RuntimeNominalSchemaBody::Record { fields, .. } = projection
            .graph()
            .definition(projection.root())
            .unwrap()
            .body()
        else {
            panic!("record");
        };
        assert_eq!(fields.len(), count);
        assert_eq!(
            projection.layout(),
            projection
                .graph()
                .try_layout_hash(projection.root())
                .unwrap()
        );
        if name == "RecordStruct" {
            assert_eq!(
                fields
                    .iter()
                    .map(|field| (field.field().zero_based(), field.name()))
                    .collect::<Vec<_>>(),
                [(0, Some("second")), (1, Some("first"))]
            );
        }
    }
    let projection = analysis
        .project_accepted_rust_nominal(
            &fixture.registered,
            &input.nominal("Cases", vec![]),
            Limits::PRODUCTION,
        )
        .unwrap();
    let RuntimeNominalSchemaBody::Variant { cases } = projection
        .graph()
        .definition(projection.root())
        .unwrap()
        .body()
    else {
        panic!("variant");
    };
    assert!(cases[0].payload().is_none());
    assert!(
        matches!(cases[1].payload(), Some(RuntimeTypeSchema::Tuple(items)) if items.is_empty())
    );
    assert!(
        matches!(cases[2].payload(), Some(RuntimeTypeSchema::Tuple(items)) if items.as_ref() == [RuntimeTypeSchema::I32])
    );
    assert!(
        matches!(cases[3].payload(), Some(RuntimeTypeSchema::RecordValue { fields }) if fields.is_empty())
    );
    assert!(
        matches!(cases[4].payload(), Some(RuntimeTypeSchema::RecordValue { fields }) if fields.len() == 1 && fields[0].name() == "enabled")
    );
    assert_eq!(projection.nominal_type(), &input.nominal("Cases", vec![]));
    assert!(projection.case_payload_type(0).unwrap().is_none());
    let tuple = projection.case_payload_type(1).unwrap().unwrap();
    let record = projection.case_payload_type(3).unwrap().unwrap();
    assert_ne!(
        tuple.semantic_identity_digest().unwrap(),
        record.semantic_identity_digest().unwrap()
    );
    assert!(matches!(
        projection.case_payload_type(5),
        Err(crate::types::VariantPayloadSealError::MissingCase { ordinal: 5 })
    ));
}

#[test]
fn rust_generic_graph_retains_recursive_project_instances_and_case_payloads() {
    let input = MetadataFixture::new();
    let fixture = input.build_with_source(
        vec![(
            "Holder",
            1,
            RustTypeMetadataPublicationKind::Struct {
                shape: RustStructMetadataInput::Record(
                    vec![EnvironmentRecordField::new("value", input.parameter())].into(),
                ),
            },
        )],
        None,
        concat!(
            "struct Branch<T> { zeta: T, alpha: Option<Branch<T>> }\n",
            "enum Case<T> { End, More(Branch<T>), Empty(Unit) }\n",
            "fn left(value: Branch<i64>) -> Branch<i64> { value }\n",
            "fn right(value: Branch<bool>) -> Branch<bool> { value }\n",
            "fn case(value: Case<i64>) -> Case<i64> { value }\n",
        ),
    );
    let analysis = analyze(&fixture).unwrap();
    let project_type = |name: &str, argument: &TypeKind| {
        analysis
            .types()
            .find_map(|(_, ty)| match ty {
                TypeKind::ProjectNominal(nominal)
                    if nominal.declaration().name().as_str() == name
                        && nominal.arguments() == std::slice::from_ref(argument) =>
                {
                    Some(ty.clone())
                }
                _ => None,
            })
            .expect("closed project type from the accepted source")
    };
    let integer = project_type("Branch", &TypeKind::I64);
    let boolean = project_type("Branch", &TypeKind::Bool);
    let case = project_type("Case", &TypeKind::I64);
    let nominal = input.nominal(
        "Holder",
        vec![TypeKind::Tuple(vec![
            integer.clone(),
            boolean.clone(),
            case.clone(),
        ])],
    );
    let projected = analysis
        .project_accepted_rust_nominal(&fixture.registered, &nominal, Limits::PRODUCTION)
        .unwrap();
    let graph = projected.graph();
    assert_eq!(graph.definitions().len(), 4);
    for (ty, expected) in [
        (&integer, RuntimeTypeSchema::I64),
        (&boolean, RuntimeTypeSchema::Bool),
    ] {
        let identity = ty.semantic_identity_digest().unwrap().into();
        let definition = graph.definition(identity).unwrap();
        assert_eq!(
            graph.try_layout_hash(identity).unwrap(),
            analysis
                .runtime_nominal_projection(ty.semantic_identity_digest().unwrap())
                .unwrap()
                .layout(),
        );
        assert_eq!(
            definition.identity().nominal(),
            analysis
                .runtime_nominal_projection(ty.semantic_identity_digest().unwrap())
                .unwrap()
                .nominal(),
        );
        let RuntimeNominalSchemaBody::Record { shape, fields } = definition.body() else {
            panic!("a project struct retains its record body")
        };
        assert_eq!(*shape, RuntimeNominalRecordShape::Record);
        assert_eq!(definition.arguments(), std::slice::from_ref(&expected));
        assert_eq!(
            fields
                .iter()
                .map(|field| (field.field().zero_based(), field.name()))
                .collect::<Vec<_>>(),
            [(0, Some("zeta")), (1, Some("alpha"))]
        );
        assert_eq!(fields[0].schema(), &expected);
        let RuntimeTypeSchema::Builtin(next) = fields[1].schema() else {
            panic!("Option")
        };
        assert_eq!(
            next.payloads(),
            &[RuntimeTypeSchema::NominalRef(definition.identity().clone())]
        );
    }
    let integer_identity = graph
        .definition(integer.semantic_identity_digest().unwrap().into())
        .unwrap()
        .identity();
    let RuntimeNominalSchemaBody::Variant { cases } = graph
        .definition(case.semantic_identity_digest().unwrap().into())
        .unwrap()
        .body()
    else {
        panic!("project enum")
    };
    assert_eq!(
        cases
            .iter()
            .map(|case| (case.ordinal(), case.name()))
            .collect::<Vec<_>>(),
        [(0, "End"), (1, "More"), (2, "Empty")]
    );
    assert_eq!(cases[0].payload(), None);
    assert_eq!(
        cases[1].payload(),
        Some(&RuntimeTypeSchema::Tuple(Box::new([
            RuntimeTypeSchema::NominalRef(integer_identity.clone())
        ])))
    );
    assert_eq!(
        cases[2].payload(),
        Some(&RuntimeTypeSchema::Tuple(Box::new([
            RuntimeTypeSchema::Unit
        ])))
    );
    assert!(matches!(
        analysis.project_accepted_rust_nominal(
            &fixture.registered,
            &nominal,
            Limits {
                max_definitions: 3,
                ..Limits::PRODUCTION
            }
        ),
        Err(ProjectionError::Limit {
            kind: LimitKind::Definitions,
            ..
        })
    ));
    projected
        .validate_for(&analysis, &fixture.registered)
        .unwrap();
}

#[test]
fn recursive_generic_instances_share_only_exact_nodes_and_ignore_unrelated_rows() {
    let input = MetadataFixture::new();
    let root = RustTypeMetadataPublicationKind::Newtype {
        inner: input.node(EnvironmentTypeProjectionKind::Tuple(
            vec![
                input.reference("Node", vec![input.node(EnvironmentTypeProjectionKind::I32)]),
                input.reference(
                    "Node",
                    vec![input.node(EnvironmentTypeProjectionKind::Bool)],
                ),
                input.reference("Node", vec![input.node(EnvironmentTypeProjectionKind::I32)]),
            ]
            .into(),
        )),
    };
    let rows = vec![("Root", 0, root), ("Node", 1, input.recursive_kind())];
    let fixture = input.build(rows.clone(), None);
    let analysis = analyze(&fixture).unwrap();
    let projection = analysis
        .project_accepted_rust_nominal(
            &fixture.registered,
            &input.nominal("Root", vec![]),
            Limits::PRODUCTION,
        )
        .unwrap();
    assert_eq!(projection.graph().definitions().len(), 3);
    let RuntimeNominalSchemaBody::Record { fields, .. } = projection
        .graph()
        .definition(projection.root())
        .unwrap()
        .body()
    else {
        panic!("record");
    };
    let RuntimeTypeSchema::Tuple(items) = fields[0].schema() else {
        panic!("tuple");
    };
    assert_eq!(items[0], items[2]);
    assert_ne!(items[0], items[1]);
    let mut reordered = rows.into_iter().rev().collect::<Vec<_>>();
    reordered.push((
        "Unrelated",
        0,
        RustTypeMetadataPublicationKind::Struct {
            shape: RustStructMetadataInput::Unit,
        },
    ));
    let other = input.build(reordered, None);
    let other_analysis = analyze(&other).unwrap();
    let other_projection = other_analysis
        .project_accepted_rust_nominal(
            &other.registered,
            &input.nominal("Root", vec![]),
            Limits::PRODUCTION,
        )
        .unwrap();
    assert_eq!(projection.layout(), other_projection.layout());
    assert_eq!(
        projection.graph().definitions().collect::<Vec<_>>(),
        other_projection.graph().definitions().collect::<Vec<_>>()
    );
    assert_ne!(projection.stamp(), other_projection.stamp());
}

#[test]
fn projection_rejects_equal_foreign_allocations_and_preserves_the_accepted_world() {
    let input = MetadataFixture::new();
    let fixture = input.build(
        vec![(
            "UnitStruct",
            0,
            RustTypeMetadataPublicationKind::Struct {
                shape: RustStructMetadataInput::Unit,
            },
        )],
        None,
    );
    let analysis = analyze(&fixture).unwrap();
    let nominal = input.nominal("UnitStruct", vec![]);
    let stamp = fixture
        .registered
        .environment()
        .accepted_rust_projection_stamp();
    let projection = analysis
        .project_accepted_rust_nominal(&fixture.registered, &nominal, Limits::PRODUCTION)
        .unwrap();
    projection
        .validate_for(&analysis, &fixture.registered.clone())
        .unwrap();
    let mut foreign = fixture.registered.clone();
    foreign.environment = Arc::new(foreign.environment().clone());
    assert_eq!(
        foreign.environment().accepted_rust_projection_stamp(),
        stamp
    );
    assert_eq!(
        projection.validate_for(&analysis, &foreign).unwrap_err(),
        ProjectionError::StaleGeneration
    );
    assert_eq!(
        analysis
            .project_accepted_rust_nominal(&foreign, &nominal, Limits::PRODUCTION)
            .unwrap_err(),
        ProjectionError::StaleGeneration
    );
    let exhausted = Limits {
        max_type_nodes: 0,
        ..Limits::PRODUCTION
    };
    assert!(matches!(
        analysis
            .project_accepted_rust_nominal(&foreign, &nominal, exhausted)
            .unwrap_err(),
        ProjectionError::Limit {
            kind: LimitKind::TypeNodes,
            ..
        }
    ));
    assert_eq!(
        fixture
            .registered
            .environment()
            .accepted_rust_projection_stamp(),
        stamp
    );
}

#[test]
fn mutual_recursion_is_finite_and_limits_reject_without_publication() {
    let input = MetadataFixture::new();
    let fixture = input.build(
        vec![
            (
                "Left",
                0,
                RustTypeMetadataPublicationKind::Newtype {
                    inner: input.reference("Right", vec![]),
                },
            ),
            (
                "Right",
                0,
                RustTypeMetadataPublicationKind::Enum {
                    variants: vec![RustVariantMetadataInput::new(
                        "Back".to_owned(),
                        RustVariantPayloadInput::Tuple(
                            vec![input.reference("Left", vec![])].into(),
                        ),
                        input.source(),
                    )]
                    .into(),
                },
            ),
        ],
        None,
    );
    let analysis = analyze(&fixture).unwrap();
    let nominal = input.nominal("Left", vec![]);
    let accepted = analysis
        .project_accepted_rust_nominal(&fixture.registered, &nominal, Limits::PRODUCTION)
        .unwrap();
    assert_eq!(accepted.graph().definitions().len(), 2);
    for (limits, expected_kind) in [
        (
            Limits {
                max_definitions: 1,
                ..Limits::PRODUCTION
            },
            LimitKind::Definitions,
        ),
        (
            Limits {
                max_active_nominal_depth: 1,
                ..Limits::PRODUCTION
            },
            LimitKind::ActiveNominalDepth,
        ),
        (
            Limits {
                max_nominal_edges: 2,
                ..Limits::PRODUCTION
            },
            LimitKind::NominalEdges,
        ),
        (
            Limits {
                max_fields_and_cases: 2,
                ..Limits::PRODUCTION
            },
            LimitKind::FieldsAndCases,
        ),
    ] {
        assert!(
            matches!(analysis.project_accepted_rust_nominal(&fixture.registered, &nominal, limits), Err(ProjectionError::Limit { kind, .. }) if kind == expected_kind)
        );
        accepted
            .validate_for(&analysis, &fixture.registered)
            .unwrap();
    }
    let exact = Limits {
        max_definitions: 2,
        max_active_nominal_depth: 2,
        max_nominal_edges: 3,
        max_fields_and_cases: 3,
        ..Limits::PRODUCTION
    };
    let repeated = analysis
        .project_accepted_rust_nominal(&fixture.registered, &nominal, exact)
        .unwrap();
    assert_eq!(accepted.layout(), repeated.layout());
    let classifier = crate::ownership::RuntimeProducerArgumentClassifier::try_new(
        &analysis,
        &fixture.registered,
    )
    .unwrap();
    assert_eq!(
        classifier
            .classify(&TypeKind::AcceptedNominal(nominal))
            .unwrap_err()
            .rejection(),
        Some(crate::ownership::RuntimeOwnershipRejection::MissingRuntimeSnapshotOwner)
    );
}

#[test]
fn nested_opaque_schema_retains_exact_evidence_and_generic_arguments() {
    let input = MetadataFixture::new();
    let owner = EnvironmentCallableOwner::Adapter(input.adapter.clone());
    let path = input.id("OpaqueLeaf").canonical_path().clone();
    let id = AcceptedNominalId::new(
        AcceptedNominalOwnerId::Environment(
            crate::env::identity::EnvironmentBindingId::try_new(format!(
                "adapter:{}",
                input.adapter.as_str()
            ))
            .unwrap(),
        ),
        path.clone(),
    );
    let producer = RuntimeOpaqueTypeProducerId::try_new("fixture.opaque_leaf").unwrap();
    let opaque = AcceptedNominalInventoryInput::new_opaque(
        id.clone(),
        1,
        AcceptedOpaqueRuntimeCarrier::new(
            producer.clone(),
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::SnapshotOnly,
        ),
        AcceptedNominalInputVisibility::Visible,
        AcceptedNominalOrigin::Adapter,
        input.source(),
        EnvironmentPublicationItemId::AdapterNominal { owner, path },
    );
    let fixture = input.build(
        vec![(
            "Root",
            0,
            RustTypeMetadataPublicationKind::Newtype {
                inner: input.node(EnvironmentTypeProjectionKind::AcceptedNominal {
                    id: id.clone(),
                    arguments: vec![input.node(EnvironmentTypeProjectionKind::I32)].into(),
                }),
            },
        )],
        Some(opaque),
    );
    let analysis = analyze(&fixture).unwrap();
    let projection = analysis
        .project_accepted_rust_nominal(
            &fixture.registered,
            &input.nominal("Root", vec![]),
            Limits::PRODUCTION,
        )
        .unwrap();
    assert_eq!(projection.graph().definitions().len(), 1);
    let RuntimeNominalSchemaBody::Record { fields, .. } = projection
        .graph()
        .definition(projection.root())
        .unwrap()
        .body()
    else {
        panic!("record");
    };
    let RuntimeTypeSchema::ExactOpaque { owner, arguments } = fields[0].schema() else {
        panic!("exact opaque");
    };
    let expected = arcweft_core::pattern::RuntimeOpaqueTypeOwner::exact_with(
        producer,
        TypeKind::AcceptedNominal(AcceptedNominalType::new(id, [TypeKind::I32]))
            .semantic_identity_digest()
            .unwrap()
            .into(),
        RuntimeOpaqueValueClass::Plain,
        RuntimeOpaquePersistence::SnapshotOnly,
    );
    assert_eq!(owner, &expected);
    assert_eq!(arguments.as_ref(), [RuntimeTypeSchema::I32]);
}

#[test]
fn zero_one_and_multiple_fields_keep_their_declared_carrier_shapes() {
    let input = MetadataFixture::new();
    for count in 0..=2 {
        let tuple = (0..count)
            .map(|_| input.node(EnvironmentTypeProjectionKind::Bool))
            .collect::<Vec<_>>();
        let record = (0..count)
            .map(|index| {
                EnvironmentRecordField::new(
                    format!("field{index}"),
                    input.node(EnvironmentTypeProjectionKind::Bool),
                )
            })
            .collect::<Vec<_>>();
        let fixture = input.build(
            vec![
                (
                    "TupleShape",
                    0,
                    RustTypeMetadataPublicationKind::Struct {
                        shape: RustStructMetadataInput::Tuple(tuple.clone().into()),
                    },
                ),
                (
                    "RecordShape",
                    0,
                    RustTypeMetadataPublicationKind::Struct {
                        shape: RustStructMetadataInput::Record(record.clone().into()),
                    },
                ),
                (
                    "PayloadShapes",
                    0,
                    RustTypeMetadataPublicationKind::Enum {
                        variants: vec![
                            RustVariantMetadataInput::new(
                                "Tuple".to_owned(),
                                RustVariantPayloadInput::Tuple(tuple.into()),
                                input.source(),
                            ),
                            RustVariantMetadataInput::new(
                                "Record".to_owned(),
                                RustVariantPayloadInput::Record(record.into()),
                                input.source(),
                            ),
                        ]
                        .into(),
                    },
                ),
            ],
            None,
        );
        let analysis = analyze(&fixture).unwrap();
        for (name, shape) in [
            ("TupleShape", RuntimeNominalRecordShape::Tuple),
            ("RecordShape", RuntimeNominalRecordShape::Record),
        ] {
            let projection = analysis
                .project_accepted_rust_nominal(
                    &fixture.registered,
                    &input.nominal(name, vec![]),
                    Limits::PRODUCTION,
                )
                .unwrap();
            let RuntimeNominalSchemaBody::Record { fields, .. } = projection
                .graph()
                .definition(projection.root())
                .unwrap()
                .body()
            else {
                panic!("record");
            };
            assert_eq!(
                projection.kind(),
                RuntimeAcceptedRustNominalKind::Record(shape)
            );
            assert_eq!(fields.len(), count);
            assert!(
                fields
                    .iter()
                    .all(|field| field.schema() == &RuntimeTypeSchema::Bool)
            );
        }
        let projection = analysis
            .project_accepted_rust_nominal(
                &fixture.registered,
                &input.nominal("PayloadShapes", vec![]),
                Limits::PRODUCTION,
            )
            .unwrap();
        let RuntimeNominalSchemaBody::Variant { cases } = projection
            .graph()
            .definition(projection.root())
            .unwrap()
            .body()
        else {
            panic!("variant");
        };
        assert!(
            matches!(cases[0].payload(), Some(RuntimeTypeSchema::Tuple(items)) if items.len() == count)
        );
        assert!(
            matches!(cases[1].payload(), Some(RuntimeTypeSchema::RecordValue { fields }) if fields.len() == count)
        );
        assert_ne!(cases[0].payload(), cases[1].payload());
    }
}

#[test]
fn generic_argument_order_and_unused_arguments_remain_layout_inputs() {
    let input = MetadataFixture::new();
    let second = input.node(EnvironmentTypeProjectionKind::TypeParameter {
        index: ArcweftRustTypeParameterIndex::try_from_usize(1).unwrap(),
    });
    let fixture = input.build(
        vec![
            (
                "Pair",
                2,
                RustTypeMetadataPublicationKind::Struct {
                    shape: RustStructMetadataInput::Tuple(vec![input.parameter(), second].into()),
                },
            ),
            (
                "Marker",
                1,
                RustTypeMetadataPublicationKind::Struct {
                    shape: RustStructMetadataInput::Unit,
                },
            ),
        ],
        None,
    );
    let analysis = analyze(&fixture).unwrap();
    for (name, forward, reverse) in [
        (
            "Pair",
            vec![TypeKind::Bool, TypeKind::I32],
            vec![TypeKind::I32, TypeKind::Bool],
        ),
        ("Marker", vec![TypeKind::Bool], vec![TypeKind::I32]),
    ] {
        let first = analysis
            .project_accepted_rust_nominal(
                &fixture.registered,
                &input.nominal(name, forward),
                Limits::PRODUCTION,
            )
            .unwrap();
        let second = analysis
            .project_accepted_rust_nominal(
                &fixture.registered,
                &input.nominal(name, reverse),
                Limits::PRODUCTION,
            )
            .unwrap();
        assert_ne!(first.root(), second.root());
        assert_ne!(first.layout(), second.layout());
        assert_ne!(
            first.graph().definition(first.root()).unwrap().arguments(),
            second
                .graph()
                .definition(second.root())
                .unwrap()
                .arguments()
        );
    }
    assert!(matches!(
        analysis.project_accepted_rust_nominal(
            &fixture.registered,
            &input.nominal("Pair", vec![]),
            Limits::PRODUCTION
        ),
        Err(ProjectionError::Arity {
            expected: 2,
            actual: 0,
            ..
        })
    ));
}
