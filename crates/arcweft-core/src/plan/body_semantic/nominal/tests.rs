use super::*;
use crate::entry::schema::{RuntimeCodecUse, RuntimeFieldCodecUse};
use crate::entry::{
    RuntimeNominalDeclarationId, RuntimeNominalRecordShape as Shape, RuntimeNominalSchemaBody,
    RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField,
    RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
    RuntimeSchemaLimits, RuntimeTypeSchema as Schema,
};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlanBuilder,
    RuntimePlanInventory, RuntimePlanTypeSeed, RuntimeTaskPlanSealLimits, RuntimeVariantCaseSeed,
    RuntimeVariantDomainSeed,
};
use crate::task::semantic::TaskSemanticEncodingError;
use crate::value::RuntimeRecordFieldId;

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

fn record_inventory(shape: Shape, types: &[bool], wire: Option<&str>) -> RuntimePlanInventory {
    let nominal = RuntimeNominalTypeId::try_new("fixture.Record").unwrap();
    let field_name = |ordinal| (shape == Shape::Record).then(|| format!("field{ordinal}"));
    let definition = RuntimeNominalSchemaDefinition::new(
        RuntimeNominalDeclarationId::from_bytes([71; 32]),
        RuntimeNominalSchemaIdentity::new(nominal.clone(), semantic(1)),
        vec![],
        RuntimeNominalSchemaBody::Record {
            shape,
            fields: types
                .iter()
                .enumerate()
                .map(|(ordinal, boolean)| {
                    RuntimeNominalSchemaField::new(
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                        field_name(ordinal),
                        if *boolean {
                            Schema::Bool
                        } else {
                            Schema::String
                        },
                    )
                })
                .collect(),
        },
    );
    let definition = if let Some(wire) = wire {
        definition.with_data_codec(RuntimeCodecUse::Record {
            name: "DiagnosticRecord".into(),
            deny_unknown_fields: false,
            fields: types
                .iter()
                .enumerate()
                .map(|(ordinal, _)| RuntimeFieldCodecUse {
                    wire_name: format!("{wire}{ordinal}"),
                    has_default: false,
                    default_program: None,
                    skip: false,
                    bytes_format: None,
                    value: RuntimeCodecUse::Plain,
                })
                .collect(),
        })
    } else {
        definition
    };
    let graph =
        RuntimeNominalSchemaGraph::try_new(vec![definition], RuntimeSchemaLimits::engine_default())
            .unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(
                    semantic(1),
                    RuntimePlanTypeProjection::Nominal {
                        nominal,
                        layout: graph.try_layout_hash(semantic(1)).unwrap(),
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(semantic(2), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(semantic(3), RuntimePlanTypeProjection::String),
            ],
            [],
            [RuntimeNominalRecordDomainSeed::new(
                semantic(1),
                shape,
                types.iter().enumerate().map(|(ordinal, boolean)| {
                    RuntimeNominalRecordDomainFieldSeed::new(
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                        field_name(ordinal),
                        semantic(if *boolean { 2 } else { 3 }),
                    )
                }),
            )],
            [],
            &graph,
        )
        .unwrap();
    let inventory = builder
        .prepare_inventory(RuntimeTaskPlanSealLimits::default())
        .unwrap();
    inventory.verify().unwrap();
    inventory
}

fn variant_inventory(payloads: &[Option<bool>]) -> RuntimePlanInventory {
    let nominal = RuntimeNominalTypeId::try_new("fixture.Variant").unwrap();
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalDeclarationId::from_bytes([72; 32]),
            RuntimeNominalSchemaIdentity::new(nominal.clone(), semantic(1)),
            vec![],
            RuntimeNominalSchemaBody::Variant {
                cases: payloads
                    .iter()
                    .enumerate()
                    .map(|(ordinal, payload)| {
                        RuntimeNominalSchemaCase::new(
                            u32::try_from(ordinal).unwrap(),
                            format!("case{ordinal}"),
                            payload.map(|boolean| {
                                if boolean {
                                    Schema::Bool
                                } else {
                                    Schema::String
                                }
                            }),
                        )
                    })
                    .collect(),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let layout = graph.try_layout_hash(semantic(1)).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(
                    semantic(1),
                    RuntimePlanTypeProjection::Nominal {
                        nominal: nominal.clone(),
                        layout,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(semantic(2), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(semantic(3), RuntimePlanTypeProjection::String),
            ],
            [],
            [],
            [RuntimeVariantDomainSeed::new(
                semantic(1),
                nominal,
                layout,
                payloads.iter().enumerate().map(|(ordinal, payload)| {
                    RuntimeVariantCaseSeed::new(
                        format!("case{ordinal}"),
                        payload.map(|boolean| semantic(if boolean { 2 } else { 3 })),
                    )
                }),
            )],
            &graph,
        )
        .unwrap();
    let inventory = builder
        .prepare_inventory(RuntimeTaskPlanSealLimits::default())
        .unwrap();
    inventory.verify().unwrap();
    inventory
}

fn digest(
    inventory: &RuntimePlanInventory,
    variant: bool,
    work: u64,
    bytes: u64,
) -> (Result<blake3::Hash, RuntimeBodySemanticError>, (u64, u64)) {
    let owner = inventory.type_table().id_for_semantic(semantic(1)).unwrap();
    let context = RuntimeBodySemanticContext::new(inventory);
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let result = if variant {
        inventory
            .variant_domains()
            .get(owner)
            .unwrap()
            .executable_semantic_row_digest(&context, &mut meter)
    } else {
        inventory
            .nominal_record_domains()
            .get(owner)
            .unwrap()
            .executable_semantic_row_digest(&context, &mut meter)
    };
    (result, meter.totals())
}

#[test]
fn unit_record_row_has_exact_owner_layout_and_empty_domain_bytes() {
    let inventory = record_inventory(Shape::Unit, &[], None);
    let owner = inventory.type_table().id_for_semantic(semantic(1)).unwrap();
    let row = inventory.type_table().get(owner).unwrap();
    let RuntimePlanTypeProjection::Nominal {
        nominal, layout, ..
    } = row.projection()
    else {
        panic!()
    };
    let mut expected = b"arcweft.runtime-plan.executable-row.v1\0".to_vec();
    expected.extend([2, 0]);
    expected.extend([1; 32]);
    expected.extend([71; 32]);
    expected.extend(u32::try_from(nominal.as_str().len()).unwrap().to_le_bytes());
    expected.extend(nominal.as_str().as_bytes());
    expected.extend(layout.as_bytes());
    expected.extend(0_u32.to_le_bytes());
    expected.push(0);
    let (actual, (_, bytes)) = digest(&inventory, false, 1_000, 10_000);
    assert_eq!(actual.unwrap(), blake3::hash(&expected));
    assert_eq!(bytes, u64::try_from(expected.len()).unwrap());
}

#[test]
fn nominal_rows_enter_the_complete_source_ordered_executable_prefix() {
    use crate::plan::body_semantic::executable_rows::fixture_prefix;
    let record = record_inventory(Shape::Record, &[true, false], Some("record"));
    let changed = record_inventory(Shape::Record, &[false, true], Some("record"));
    assert_ne!(fixture_prefix(&record), fixture_prefix(&changed));
    let variant = variant_inventory(&[None, Some(true)]);
    let changed = variant_inventory(&[None, Some(false)]);
    assert_ne!(fixture_prefix(&variant), fixture_prefix(&changed));
}

#[test]
fn variant_row_has_exact_case_identity_and_optional_payload_coordinates() {
    let inventory = variant_inventory(&[None, Some(true)]);
    let owner = inventory.type_table().id_for_semantic(semantic(1)).unwrap();
    let row = inventory.type_table().get(owner).unwrap();
    let RuntimePlanTypeProjection::Nominal {
        nominal, layout, ..
    } = row.projection()
    else {
        panic!()
    };
    let payload = inventory.type_table().id_for_semantic(semantic(2)).unwrap();
    let mut expected = b"arcweft.runtime-plan.executable-row.v1\0".to_vec();
    expected.extend([3, 0]);
    expected.extend([1; 32]);
    expected.extend([72; 32]);
    expected.extend(u32::try_from(nominal.as_str().len()).unwrap().to_le_bytes());
    expected.extend(nominal.as_str().as_bytes());
    expected.extend(layout.as_bytes());
    expected.extend(2_u32.to_le_bytes());
    expected.extend(0_u32.to_le_bytes()); // first case source role
    expected.extend(0_u32.to_le_bytes()); // first accepted case identity
    expected.push(0);
    expected.extend(1_u32.to_le_bytes());
    expected.extend(1_u32.to_le_bytes());
    expected.push(1);
    expected.extend((payload.get().get() - 1).to_le_bytes());
    expected.push(0); // no occurrence codec
    let (actual, (_, bytes)) = digest(&inventory, true, 1_000, 10_000);
    assert_eq!(actual.unwrap(), blake3::hash(&expected));
    assert_eq!(bytes, u64::try_from(expected.len()).unwrap());
}

#[test]
fn domain_label_shadow_preserves_accepted_owner_layout_and_codec_semantics() {
    // Shadow only excluded domain labels, holding accepted layout and wire
    // artifacts fixed. Recomputing a source schema is a different operation:
    // its current layout owner also commits source-schema names.
    let record = record_inventory(Shape::Record, &[true, false], Some("wire"));
    let owner = record.type_table().id_for_semantic(semantic(1)).unwrap();
    let row = record.nominal_record_domains().get(owner).unwrap();
    let shadow = RuntimeNominalRecordDomain::from_admitted_parts(
        owner,
        row.shape(),
        row.fields()
            .iter()
            .enumerate()
            .map(|(ordinal, field)| (field.field(), Some(format!("renamed{ordinal}")), field.ty())),
    )
    .with_data_codec(row.data_codec().cloned());
    let mut table =
        crate::plan::nominal_record_domains::RuntimeNominalRecordDomainTableBuilder::new();
    let prepared = table.prepare_batch([shadow]).unwrap();
    table.commit_batch(prepared);
    let mut changed = record.clone();
    changed.nominal_record_domains = table.finish();
    changed.verify().unwrap();
    assert_eq!(
        digest(&record, false, 10_000, 100_000).0.unwrap(),
        digest(&changed, false, 10_000, 100_000).0.unwrap()
    );

    let variant = variant_inventory(&[None, Some(true)]);
    let owner = variant.type_table().id_for_semantic(semantic(1)).unwrap();
    let row = variant.variant_domains().get(owner).unwrap();
    let shadow = RuntimeVariantDomain::from_admitted_parts(
        owner,
        row.nominal().clone(),
        row.layout(),
        row.cases()
            .iter()
            .enumerate()
            .map(|(ordinal, case)| (format!("renamed{ordinal}"), case.payload())),
    )
    .with_data_codec(row.data_codec().cloned());
    let mut table = crate::plan::variant_domains::RuntimeVariantDomainTableBuilder::new();
    let prepared = table.prepare_batch([shadow]).unwrap();
    table.commit_batch(prepared);
    let mut changed = variant.clone();
    changed.variant_domains = table.finish();
    changed.verify().unwrap();
    assert_eq!(
        digest(&variant, true, 10_000, 100_000).0.unwrap(),
        digest(&changed, true, 10_000, 100_000).0.unwrap()
    );
}

#[test]
fn record_shapes_field_types_and_source_order_are_semantic() {
    let inventories = [
        record_inventory(Shape::Unit, &[], None),
        record_inventory(Shape::Tuple, &[], None),
        record_inventory(Shape::Record, &[], None),
        record_inventory(Shape::Tuple, &[true], None),
        record_inventory(Shape::Newtype, &[true], None),
        record_inventory(Shape::Record, &[true, false], None),
        record_inventory(Shape::Record, &[false, true], None),
    ];
    let hashes: std::collections::BTreeSet<_> = inventories
        .iter()
        .map(|inventory| {
            *digest(inventory, false, 10_000, 100_000)
                .0
                .unwrap()
                .as_bytes()
        })
        .collect();
    assert_eq!(hashes.len(), inventories.len());
}

#[test]
fn occurrence_wire_roles_change_the_actual_admitted_record_row() {
    let first = record_inventory(Shape::Record, &[true], Some("first"));
    let second = record_inventory(Shape::Record, &[true], Some("second"));
    let plain = record_inventory(Shape::Record, &[true], None);
    let hashes: std::collections::BTreeSet<_> = [&first, &second, &plain]
        .into_iter()
        .map(|inventory| {
            *digest(inventory, false, 10_000, 100_000)
                .0
                .unwrap()
                .as_bytes()
        })
        .collect();
    assert_eq!(hashes.len(), 3);
}

#[test]
fn variant_case_presence_payload_type_and_source_order_are_semantic() {
    let inventories = [
        variant_inventory(&[None]),
        variant_inventory(&[Some(true)]),
        variant_inventory(&[Some(false)]),
        variant_inventory(&[None, Some(true)]),
        variant_inventory(&[Some(true), None]),
    ];
    let hashes: std::collections::BTreeSet<_> = inventories
        .iter()
        .map(|inventory| {
            *digest(inventory, true, 10_000, 100_000)
                .0
                .unwrap()
                .as_bytes()
        })
        .collect();
    assert_eq!(hashes.len(), inventories.len());
}

#[test]
fn nominal_rows_share_exact_work_and_byte_limits_and_sticky_failure() {
    for (inventory, variant) in [
        (
            record_inventory(Shape::Record, &[true, false], Some("wire")),
            false,
        ),
        (variant_inventory(&[None, Some(true)]), true),
    ] {
        let (expected, (work, bytes)) = digest(&inventory, variant, 10_000, 100_000);
        assert_eq!(
            digest(&inventory, variant, work, bytes).0.unwrap(),
            expected.unwrap()
        );
        assert!(matches!(
            digest(&inventory, variant, work - 1, bytes).0,
            Err(RuntimeBodySemanticError::Encoding(
                TaskSemanticEncodingError::SemanticWork
            ))
        ));
        assert!(matches!(
            digest(&inventory, variant, work, bytes - 1).0,
            Err(RuntimeBodySemanticError::Encoding(
                TaskSemanticEncodingError::TranscriptBytes
            ))
        ));
        let mut meter = TaskSemanticMeter::new(0, 100_000);
        meter.charge_work(1).unwrap_err();
        let before = meter.totals();
        let owner = inventory.type_table().id_for_semantic(semantic(1)).unwrap();
        let context = RuntimeBodySemanticContext::new(&inventory);
        let actual = if variant {
            inventory
                .variant_domains()
                .get(owner)
                .unwrap()
                .executable_semantic_row_digest(&context, &mut meter)
        } else {
            inventory
                .nominal_record_domains()
                .get(owner)
                .unwrap()
                .executable_semantic_row_digest(&context, &mut meter)
        };
        assert!(matches!(
            actual,
            Err(RuntimeBodySemanticError::Encoding(
                TaskSemanticEncodingError::SemanticWork
            ))
        ));
        assert_eq!(meter.totals(), before);
    }
}

#[test]
fn equal_rows_from_another_inventory_cannot_supply_an_owner_digest() {
    for (inventory, variant) in [
        (record_inventory(Shape::Unit, &[], None), false),
        (variant_inventory(&[None]), true),
    ] {
        let foreign = inventory.clone();
        let owner = inventory.type_table().id_for_semantic(semantic(1)).unwrap();
        let context = RuntimeBodySemanticContext::new(&foreign);
        let mut meter = TaskSemanticMeter::new(10_000, 100_000);
        let actual = if variant {
            inventory
                .variant_domains()
                .get(owner)
                .unwrap()
                .executable_semantic_row_digest(&context, &mut meter)
        } else {
            inventory
                .nominal_record_domains()
                .get(owner)
                .unwrap()
                .executable_semantic_row_digest(&context, &mut meter)
        };
        assert!(matches!(
            actual,
            Err(RuntimeBodySemanticError::ForeignNominalDomain)
        ));
        assert_eq!(meter.totals(), (0, 0));
        assert_eq!(
            meter.status(),
            Err(TaskSemanticEncodingError::OwnerRejected)
        );
    }
}
