use super::*;
use crate::{
    entry::{
        RuntimeNominalRecordShape as Shape, RuntimeNominalRecordShapeError,
        RuntimeNominalSchemaBody, RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField,
        RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeSchemaLimits,
        RuntimeTypeSchema,
    },
    plan::RuntimeNominalRecordDomainFieldSeed,
    value::RuntimeRecordFieldId,
};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

fn schema(shape: Shape, names: &[Option<&str>]) -> RuntimeNominalSchemaGraph {
    RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalSchemaIdentity::new(
                RuntimeNominalTypeId::try_new("fixture.RecordDomain").unwrap(),
                semantic(1),
            ),
            vec![],
            RuntimeNominalSchemaBody::Record {
                shape,
                fields: names
                    .iter()
                    .enumerate()
                    .map(|(ordinal, name)| {
                        RuntimeNominalSchemaField::new(
                            RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                            name.map(str::to_owned),
                            RuntimeTypeSchema::Bool,
                        )
                    })
                    .collect(),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap()
}

fn types(schema: &RuntimeNominalSchemaGraph) -> [RuntimePlanTypeSeed; 2] {
    [
        RuntimePlanTypeSeed::new(
            semantic(1),
            RuntimePlanTypeProjection::Nominal {
                nominal: RuntimeNominalTypeId::try_new("fixture.RecordDomain").unwrap(),
                layout: schema.try_layout_hash(semantic(1)).unwrap(),
                arguments: Box::new([]),
            },
        ),
        RuntimePlanTypeSeed::new(semantic(2), RuntimePlanTypeProjection::Bool),
    ]
}

fn fields(names: &[Option<&str>]) -> Vec<RuntimeNominalRecordDomainFieldSeed> {
    names
        .iter()
        .enumerate()
        .map(|(ordinal, name)| {
            RuntimeNominalRecordDomainFieldSeed::new(
                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                name.map(str::to_owned),
                semantic(2),
            )
        })
        .collect()
}

#[test]
fn every_record_shape_retains_explicit_field_ids_and_source_order() {
    for (shape, names) in [
        (Shape::Unit, vec![]),
        (Shape::Tuple, vec![]),
        (Shape::Tuple, vec![None]),
        (Shape::Tuple, vec![None, None]),
        (Shape::Record, vec![]),
        (Shape::Record, vec![Some("z"), Some("a")]),
        (Shape::Newtype, vec![None]),
    ] {
        let schema = schema(shape, &names);
        let mut builder = RuntimePlanBuilder::new();
        let domain = RuntimeNominalRecordDomainSeed::new(semantic(1), shape, fields(&names));
        for _ in 0..2 {
            builder
                .admit_semantic_batch(types(&schema), [], [domain.clone()], [], &schema)
                .unwrap();
        }
        let plan = builder.finish().unwrap();
        let owner = plan.type_table().id_for_semantic(semantic(1)).unwrap();
        let field_type = plan.type_table().id_for_semantic(semantic(2)).unwrap();
        let admitted = plan.nominal_record_domains().get(owner).unwrap();
        assert_eq!(admitted.shape(), shape);
        assert_eq!(plan.nominal_record_domains().len(), 1);
        assert_eq!(admitted.fields().len(), names.len());
        for (ordinal, (field, expected_name)) in admitted.fields().iter().zip(names).enumerate() {
            assert_eq!(
                field.field(),
                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap()
            );
            assert_eq!(field.name(), expected_name);
            assert_eq!(field.ty(), field_type);
        }
    }
}

#[test]
fn invalid_source_shapes_leave_types_locals_and_domains_unpublished() {
    for (shape, names, expected) in [
        (
            Shape::Unit,
            vec![None],
            RuntimeNominalRecordShapeError::FieldCount {
                shape: Shape::Unit,
                expected: 0,
                actual: 1,
            },
        ),
        (
            Shape::Tuple,
            vec![Some("named")],
            RuntimeNominalRecordShapeError::UnexpectedFieldName {
                shape: Shape::Tuple,
                ordinal: 0,
            },
        ),
        (
            Shape::Record,
            vec![None],
            RuntimeNominalRecordShapeError::MissingFieldName { ordinal: 0 },
        ),
        (
            Shape::Record,
            vec![Some("")],
            RuntimeNominalRecordShapeError::MissingFieldName { ordinal: 0 },
        ),
        (
            Shape::Record,
            vec![Some("repeat"), Some("repeat")],
            RuntimeNominalRecordShapeError::DuplicateFieldName {
                ordinal: 1,
                name: "repeat".to_owned(),
            },
        ),
        (
            Shape::Newtype,
            vec![],
            RuntimeNominalRecordShapeError::FieldCount {
                shape: Shape::Newtype,
                expected: 1,
                actual: 0,
            },
        ),
        (
            Shape::Newtype,
            vec![None, None],
            RuntimeNominalRecordShapeError::FieldCount {
                shape: Shape::Newtype,
                expected: 1,
                actual: 2,
            },
        ),
    ] {
        let schema = schema(Shape::Newtype, &[None]);
        let mut builder = RuntimePlanBuilder::new();
        let invalid = RuntimeNominalRecordDomainSeed::new(semantic(1), shape, fields(&names));
        assert!(
            matches!(builder.admit_semantic_batch(types(&schema), [RuntimeLocalDeclarationSeed::new(semantic(2))], [invalid], [], &schema),
            Err(RuntimePlanBuildError::NominalRecordDomain(RuntimeNominalRecordDomainError::Shape { source, .. })) if source == expected)
        );
        let valid =
            RuntimeNominalRecordDomainSeed::new(semantic(1), Shape::Newtype, fields(&[None]));
        let admission = builder
            .admit_semantic_batch(
                types(&schema),
                [RuntimeLocalDeclarationSeed::new(semantic(2))],
                [valid],
                [],
                &schema,
            )
            .unwrap();
        assert_eq!(admission.local_ids().len(), 1);
        let plan = builder.finish().unwrap();
        assert_eq!(plan.type_table().len(), 2);
        assert_eq!(plan.local_declarations().len(), 1);
        assert_eq!(plan.nominal_record_domains().len(), 1);
        assert!(plan.variant_domains().is_empty());
    }
}

#[test]
fn reordered_and_gapped_field_ids_cannot_be_reinterpreted_as_positions() {
    for ordinals in [[1, 0], [0, 2], [0, 0]] {
        let schema = schema(Shape::Tuple, &[None, None]);
        let mut builder = RuntimePlanBuilder::new();
        let fields = ordinals.into_iter().map(|ordinal| {
            RuntimeNominalRecordDomainFieldSeed::new(
                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                None,
                semantic(2),
            )
        });
        let domain = RuntimeNominalRecordDomainSeed::new(semantic(1), Shape::Tuple, fields);
        assert!(matches!(
            builder.admit_semantic_batch(types(&schema), [], [domain], [], &schema),
            Err(RuntimePlanBuildError::NominalRecordDomain(
                RuntimeNominalRecordDomainError::FieldIdentity { .. }
            ))
        ));
        let plan = builder.finish().unwrap();
        assert!(plan.type_table().is_empty());
        assert!(plan.nominal_record_domains().is_empty());
    }
}

#[test]
fn conflicting_shapes_do_not_replace_an_already_admitted_domain() {
    let schema = schema(Shape::Tuple, &[None]);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            types(&schema),
            [],
            [RuntimeNominalRecordDomainSeed::new(
                semantic(1),
                Shape::Tuple,
                fields(&[None]),
            )],
            [],
            &schema,
        )
        .unwrap();
    assert!(matches!(
        builder.admit_semantic_batch(
            types(&schema),
            [],
            [RuntimeNominalRecordDomainSeed::new(
                semantic(1),
                Shape::Newtype,
                fields(&[None])
            )],
            [],
            &schema,
        ),
        Err(RuntimePlanBuildError::NominalRecordDomain(
            RuntimeNominalRecordDomainError::ConflictingDomain { .. }
        ))
    ));
    let plan = builder.finish().unwrap();
    let owner = plan.type_table().id_for_semantic(semantic(1)).unwrap();
    assert_eq!(
        plan.nominal_record_domains().get(owner).unwrap().shape(),
        Shape::Tuple
    );
}
