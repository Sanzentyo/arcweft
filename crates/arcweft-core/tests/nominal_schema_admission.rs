use arcweft_core::entry::{
    RuntimeNominalRecordShape as Shape, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
    RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
    RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeSchemaLimits,
    RuntimeTypeSchema as Schema, TypeLayoutHash,
};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    RuntimeLocalDeclarationSeed, RuntimeNominalRecordDomainFieldSeed,
    RuntimeNominalRecordDomainSeed, RuntimePlanBuildError, RuntimePlanBuilder,
    RuntimePlanNominalSchemaError as Error, RuntimePlanSchemaComponent as Component,
    RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed, RuntimeVariantCaseSeed,
    RuntimeVariantDomainSeed,
};
use arcweft_core::value::{RuntimeRecordFieldId, RuntimeSignedIntWidth};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn nominal(tag: u8) -> RuntimeNominalTypeId {
    RuntimeNominalTypeId::try_new(format!("fixture.Schema{tag}")).unwrap()
}
fn identity(tag: u8) -> RuntimeNominalSchemaIdentity {
    RuntimeNominalSchemaIdentity::new(nominal(tag), semantic(tag))
}
fn field(ordinal: usize) -> RuntimeRecordFieldId {
    RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap()
}
fn seed(tag: u8, ty: Type<RuntimeSemanticTypeId>) -> RuntimePlanTypeSeed {
    RuntimePlanTypeSeed::new(semantic(tag), ty)
}

fn definitions() -> Vec<RuntimeNominalSchemaDefinition> {
    vec![
        RuntimeNominalSchemaDefinition::new(
            identity(1),
            vec![Schema::Bool],
            RuntimeNominalSchemaBody::Record {
                shape: Shape::Record,
                fields: vec![
                    RuntimeNominalSchemaField::new(field(0), Some("head".to_owned()), Schema::Bool),
                    RuntimeNominalSchemaField::new(
                        field(1),
                        Some("tail".to_owned()),
                        Schema::option(Schema::NominalRef(identity(5))),
                    ),
                ]
                .into_boxed_slice(),
            },
        ),
        RuntimeNominalSchemaDefinition::new(
            identity(5),
            vec![],
            RuntimeNominalSchemaBody::Variant {
                cases: vec![
                    RuntimeNominalSchemaCase::new(0, "End".to_owned(), None),
                    RuntimeNominalSchemaCase::new(
                        1,
                        "Link".to_owned(),
                        Some(Schema::Tuple(Box::new([Schema::NominalRef(identity(1))]))),
                    ),
                ]
                .into_boxed_slice(),
            },
        ),
    ]
}

fn graph() -> RuntimeNominalSchemaGraph {
    RuntimeNominalSchemaGraph::try_new(definitions(), RuntimeSchemaLimits::engine_default())
        .unwrap()
}

fn types(graph: &RuntimeNominalSchemaGraph) -> Vec<RuntimePlanTypeSeed> {
    vec![
        seed(
            1,
            Type::Nominal {
                nominal: nominal(1),
                layout: graph.try_layout_hash(semantic(1)).unwrap(),
                arguments: Box::new([semantic(2)]),
            },
        ),
        seed(2, Type::Bool),
        seed(
            3,
            Type::Option {
                item: semantic(5),
                some_payload: semantic(4),
            },
        ),
        seed(4, Type::Tuple(Box::new([semantic(5)]))),
        seed(
            5,
            Type::Nominal {
                nominal: nominal(5),
                layout: graph.try_layout_hash(semantic(5)).unwrap(),
                arguments: Box::new([]),
            },
        ),
        seed(6, Type::Tuple(Box::new([semantic(1)]))),
        seed(7, Type::Signed(RuntimeSignedIntWidth::I8)),
    ]
}

fn record(head: &str, head_type: u8) -> RuntimeNominalRecordDomainSeed {
    RuntimeNominalRecordDomainSeed::new(
        semantic(1),
        Shape::Record,
        [
            RuntimeNominalRecordDomainFieldSeed::new(
                field(0),
                Some(head.to_owned()),
                semantic(head_type),
            ),
            RuntimeNominalRecordDomainFieldSeed::new(
                field(1),
                Some("tail".to_owned()),
                semantic(3),
            ),
        ],
    )
}

fn variant(graph: &RuntimeNominalSchemaGraph, link: &str) -> RuntimeVariantDomainSeed {
    RuntimeVariantDomainSeed::new(
        semantic(5),
        nominal(5),
        graph.try_layout_hash(semantic(5)).unwrap(),
        [
            RuntimeVariantCaseSeed::new("End", None),
            RuntimeVariantCaseSeed::new(link, Some(semantic(6))),
        ],
    )
}

#[test]
fn recursive_record_variant_graph_is_correlated_and_discarded_before_plan_seal() {
    let graph = graph();
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            types(&graph),
            [RuntimeLocalDeclarationSeed::new(semantic(1))],
            [record("head", 2)],
            [variant(&graph, "Link")],
            &graph,
        )
        .unwrap();
    assert_eq!(admission.type_ids().len(), 7);
    assert_eq!(admission.local_ids().len(), 1);
    let ids = admission.type_ids().to_vec();
    drop(graph);
    // Existing nominal references remain usable when only new ordinary rows
    // and locals are added. No retained schema catalog is needed.
    builder
        .admit_type_batch(
            [seed(8, Type::Tuple(Box::new([semantic(1), semantic(5)])))],
            [RuntimeLocalDeclarationSeed::new(semantic(5))],
        )
        .unwrap();
    let plan = builder.finish().unwrap();
    assert_eq!(plan.type_table().id_for_semantic(semantic(1)), Some(ids[0]));
    assert_eq!(plan.type_table().id_for_semantic(semantic(5)), Some(ids[4]));
    assert_eq!(
        plan.nominal_record_domains().get(ids[0]).unwrap().fields()[1].ty(),
        ids[2]
    );
    assert_eq!(
        plan.variant_domains().get(ids[4]).unwrap().cases()[1].payload(),
        Some(ids[5])
    );
}

#[test]
fn name_type_case_and_layout_mismatches_publish_nothing() {
    let graph = graph();
    for problem in 0..4 {
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [seed(9, Type::Unit)],
                [RuntimeLocalDeclarationSeed::new(semantic(9))],
            )
            .unwrap();
        let mut candidate_types = types(&graph);
        if problem == 3 {
            candidate_types[0] = seed(
                1,
                Type::Nominal {
                    nominal: nominal(1),
                    layout: TypeLayoutHash::from_bytes([99; 32]),
                    arguments: Box::new([semantic(2)]),
                },
            );
        }
        let result = builder.admit_semantic_batch(
            candidate_types,
            [RuntimeLocalDeclarationSeed::new(semantic(1))],
            [record(
                if problem == 0 { "renamed" } else { "head" },
                if problem == 1 { 7 } else { 2 },
            )],
            [variant(&graph, if problem == 2 { "Wrong" } else { "Link" })],
            &graph,
        );
        match (problem, result) {
            (
                0,
                Err(RuntimePlanBuildError::NominalSchema(Error::Mismatch {
                    component: Component::RecordFields,
                    ..
                })),
            )
            | (
                1,
                Err(RuntimePlanBuildError::NominalSchema(Error::Mismatch {
                    component: Component::Projection,
                    ..
                })),
            )
            | (
                2,
                Err(RuntimePlanBuildError::NominalSchema(Error::Mismatch {
                    component: Component::VariantCases,
                    ..
                })),
            )
            | (3, Err(RuntimePlanBuildError::NominalSchema(Error::Layout { .. }))) => {}
            (_, actual) => panic!("unexpected failure for {problem}: {actual:?}"),
        }
        let plan = builder.finish().unwrap();
        assert_eq!(plan.type_table().declarations().len(), 1);
        assert_eq!(plan.local_declarations().len(), 1);
        assert!(plan.nominal_record_domains().is_empty());
        assert!(plan.variant_domains().is_empty());
    }
}

#[test]
fn new_nominal_types_cannot_enter_through_the_ordinary_type_batch() {
    let graph = graph();
    let mut builder = RuntimePlanBuilder::new();
    assert!(matches!(
        builder.admit_type_batch(types(&graph), []),
        Err(RuntimePlanBuildError::NominalSchema(
            Error::MissingProof { .. }
        ))
    ));
    assert!(
        builder
            .finish()
            .unwrap()
            .type_table()
            .declarations()
            .next()
            .is_none()
    );
}

#[test]
fn every_proof_definition_needs_a_type_and_correlation_work_is_bounded() {
    let original = graph();
    let mut extended = definitions();
    extended.push(RuntimeNominalSchemaDefinition::new(
        identity(8),
        vec![],
        RuntimeNominalSchemaBody::Record {
            shape: Shape::Unit,
            fields: Box::new([]),
        },
    ));
    let extra = RuntimeNominalSchemaGraph::try_new(extended, RuntimeSchemaLimits::engine_default())
        .unwrap();
    let limited = RuntimeNominalSchemaGraph::try_new(
        definitions(),
        RuntimeSchemaLimits {
            max_validation_work: 0,
            ..RuntimeSchemaLimits::engine_default()
        },
    )
    .unwrap();
    for (proof, budget_failure) in [(&extra, false), (&limited, true)] {
        let mut builder = RuntimePlanBuilder::new();
        let result = builder.admit_semantic_batch(
            types(&original),
            [],
            [record("head", 2)],
            [variant(&original, "Link")],
            proof,
        );
        match (budget_failure, result) {
            (false, Err(RuntimePlanBuildError::NominalSchema(Error::MissingType { identity })))
                if identity == semantic(8) => {}
            (true, Err(RuntimePlanBuildError::NominalSchema(Error::Limits { .. }))) => {}
            (_, actual) => panic!("unexpected proof failure: {actual:?}"),
        }
        assert!(
            builder
                .finish()
                .unwrap()
                .type_table()
                .declarations()
                .next()
                .is_none()
        );
    }
}
