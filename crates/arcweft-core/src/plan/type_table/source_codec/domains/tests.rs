use super::*;
use crate::entry::schema::{RuntimeCodecUse, RuntimeFieldCodecUse};
use crate::entry::{
    RuntimeNominalDeclarationId, RuntimeNominalSchemaBody, RuntimeNominalSchemaCase,
    RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
    RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema,
};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::body_semantic::task_image::UnsealedRuntimePlanImage;
use crate::plan::{
    RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed, RuntimePlanBuilder,
    RuntimePlanTypeSeed, RuntimeTaskPlanSealLimits, RuntimeVariantCaseSeed,
    RuntimeVariantDomainSeed,
};
use crate::task::semantic::TaskSemanticEncodingError;

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

pub(in crate::plan) fn builder(
    shape: RuntimeNominalRecordShape,
    count: usize,
    policy: bool,
) -> RuntimePlanBuilder {
    let record = RuntimeNominalTypeId::try_new("fixture.SourceRecord").unwrap();
    let variant = RuntimeNominalTypeId::try_new("fixture.SourceVariant").unwrap();
    let name =
        |ordinal| (shape == RuntimeNominalRecordShape::Record).then(|| format!("field{ordinal}"));
    let definition = RuntimeNominalSchemaDefinition::new(
        RuntimeNominalDeclarationId::from_bytes([121; 32]),
        RuntimeNominalSchemaIdentity::new(record.clone(), semantic(1)),
        vec![],
        RuntimeNominalSchemaBody::Record {
            shape,
            fields: (0..count)
                .map(|ordinal| {
                    RuntimeNominalSchemaField::new(
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                        name(ordinal),
                        RuntimeTypeSchema::Bool,
                    )
                })
                .collect(),
        },
    );
    let definition = if policy {
        definition.with_data_codec(RuntimeCodecUse::Record {
            name: "WireRecord".into(),
            deny_unknown_fields: true,
            fields: (0..count)
                .map(|ordinal| RuntimeFieldCodecUse {
                    wire_name: format!("wire{ordinal}"),
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
    let graph = RuntimeNominalSchemaGraph::try_new(
        vec![
            definition,
            RuntimeNominalSchemaDefinition::new(
                RuntimeNominalDeclarationId::from_bytes([122; 32]),
                RuntimeNominalSchemaIdentity::new(variant.clone(), semantic(2)),
                vec![],
                RuntimeNominalSchemaBody::Variant {
                    cases: Box::new([
                        RuntimeNominalSchemaCase::new(0, "Missing".into(), None),
                        RuntimeNominalSchemaCase::new(
                            1,
                            "Present".into(),
                            Some(RuntimeTypeSchema::Bool),
                        ),
                    ]),
                },
            ),
        ],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let variant_layout = graph.try_layout_hash(semantic(2)).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(
                    semantic(1),
                    RuntimePlanTypeProjection::Nominal {
                        nominal: record,
                        layout: graph.try_layout_hash(semantic(1)).unwrap(),
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    semantic(2),
                    RuntimePlanTypeProjection::Nominal {
                        nominal: variant.clone(),
                        layout: variant_layout,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(semantic(3), RuntimePlanTypeProjection::Bool),
            ],
            [],
            [RuntimeNominalRecordDomainSeed::new(
                semantic(1),
                shape,
                (0..count).map(|ordinal| {
                    RuntimeNominalRecordDomainFieldSeed::new(
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                        name(ordinal),
                        semantic(3),
                    )
                }),
            )],
            [RuntimeVariantDomainSeed::new(
                semantic(2),
                variant,
                variant_layout,
                [
                    RuntimeVariantCaseSeed::new("Missing", None),
                    RuntimeVariantCaseSeed::new("Present", Some(semantic(3))),
                ],
            )],
            &graph,
        )
        .unwrap();
    builder
}

fn limits() -> RuntimeTypeSourceLimits {
    RuntimeTypeSourceLimits {
        rows: 8,
        elements: 100_000,
        encoded_bytes: 100_000,
        decoded_bytes: 100_000,
        policy_depth: 32,
    }
}

fn encode(inventory: &RuntimePlanInventory) -> Vec<u8> {
    let prepared = inventory
        .prepare_nominal_sources_encoding(
            limits(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
    let mut output = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut output).unwrap();
    output
}

fn decode(
    bytes: &[u8],
    inventory: &RuntimePlanInventory,
    limits: RuntimeTypeSourceLimits,
    meter: &mut TaskSemanticMeter,
) -> Result<(RuntimeNominalRecordDomainTable, RuntimeVariantDomainTable), Error> {
    RuntimePlanInventory::preflight_nominal_sources(
        bytes,
        inventory.type_table().len(),
        limits,
        meter,
    )?
    .materialize(inventory.type_table(), meter)
}

#[test]
fn nominal_sources_preserve_shapes_cases_policies_and_the_actual_executable_digest() {
    use RuntimeNominalRecordShape as S;
    for (shape, count, policy) in [
        (S::Unit, 0, false),
        (S::Tuple, 0, false),
        (S::Tuple, 2, false),
        (S::Newtype, 1, false),
        (S::Record, 0, true),
        (S::Record, 2, true),
    ] {
        let original = builder(shape, count, policy).finish().unwrap();
        let fresh_builder = builder(shape, count, policy);
        let coordinate_owner = fresh_builder.task_coordinate_owner(0);
        let mut fresh = fresh_builder
            .prepare_inventory(RuntimeTaskPlanSealLimits::default(), 0)
            .unwrap();
        let encoded = encode(&fresh);
        let prepared = RuntimePlanInventory::preflight_nominal_sources(
            &encoded,
            fresh.type_table.len(),
            limits(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
        assert_eq!((prepared.record_count(), prepared.variant_count()), (1, 1));
        let (records, variants) = decode(
            &encoded,
            &fresh,
            limits(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
        assert_eq!(&records, original.nominal_record_domains());
        assert_eq!(&variants, original.variant_domains());
        fresh.nominal_record_domains = records;
        fresh.variant_domains = variants;
        assert_eq!(encode(&fresh), encoded);
        let restored =
            UnsealedRuntimePlanImage::new_for_test(fresh, coordinate_owner, Box::new([]), None)
                .unwrap()
                .seal(RuntimeTaskPlanSealLimits::default(), None)
                .unwrap()
                .into_plan();
        assert_eq!(
            restored.executable_semantic_digest(),
            original.executable_semantic_digest()
        );
    }
}

#[test]
fn nominal_sources_enforce_combined_rows_storage_elements_and_encoded_bounds() {
    let original = candidate(RuntimeNominalRecordShape::Record, 2, true);
    let mut meter = TaskSemanticMeter::new(1_000_000, 1_000_000);
    let measured = original
        .prepare_nominal_sources_encoding(limits(), &mut meter)
        .unwrap();
    let exact = RuntimeTypeSourceLimits {
        rows: 2,
        elements: u32::try_from(measured.elements()).unwrap(),
        encoded_bytes: measured.encoded_bytes(),
        decoded_bytes: measured.storage_bytes(),
        ..limits()
    };
    let encoded = encode(&original);
    decode(
        &encoded,
        &original,
        exact,
        &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
    )
    .unwrap();
    for short in [
        RuntimeTypeSourceLimits { rows: 1, ..exact },
        RuntimeTypeSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeTypeSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeTypeSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            RuntimePlanInventory::preflight_nominal_sources(
                &encoded,
                3,
                short,
                &mut TaskSemanticMeter::new(1_000_000, 1_000_000)
            )
            .err()
            .unwrap(),
            Error::Limit { .. }
        ));
    }
}

#[test]
fn nominal_sources_reject_noncanonical_frames_before_materialization() {
    let original = candidate(RuntimeNominalRecordShape::Record, 2, true);
    let encoded = encode(&original);
    let owner = DOMAIN.len() + 1 + 4;
    for case in 0..8 {
        let mut changed = encoded.clone();
        match case {
            0 => changed[0] ^= 1,
            1 => changed[DOMAIN.len()] = 2,
            2 => {
                changed.pop();
            }
            3 => changed.push(0),
            4 => changed[owner..owner + 4].fill(0),
            5 => changed[owner + 4] = 255,
            6 => changed[owner + 9..owner + 13].copy_from_slice(&1_u32.to_le_bytes()),
            7 => changed[DOMAIN.len() + 1..owner].copy_from_slice(&u32::MAX.to_le_bytes()),
            _ => unreachable!(),
        }
        assert!(
            RuntimePlanInventory::preflight_nominal_sources(
                &changed,
                3,
                limits(),
                &mut TaskSemanticMeter::new(1_000_000, 1_000_000)
            )
            .is_err(),
            "mutation {case}"
        );
    }
    let mut record_only = candidate(RuntimeNominalRecordShape::Unit, 0, false);
    record_only.variant_domains = RuntimeVariantDomainTable::from_source_domains(vec![]).unwrap();
    let single = encode(&record_only);
    let mut duplicate = single[..owner].to_vec();
    duplicate[DOMAIN.len() + 1..owner].copy_from_slice(&2_u32.to_le_bytes());
    let record_row = &single[owner..single.len() - 4];
    duplicate.extend_from_slice(record_row);
    duplicate.extend_from_slice(record_row);
    duplicate.extend_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        RuntimePlanInventory::preflight_nominal_sources(
            &duplicate,
            3,
            limits(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000)
        )
        .err()
        .unwrap(),
        Error::NominalOrder
    );
}

#[test]
fn nominal_sources_bind_actual_type_owners_and_validate_case_names() {
    let original = candidate(RuntimeNominalRecordShape::Record, 2, true);
    let mut encoded = encode(&original);
    let owner = DOMAIN.len() + 1 + 4;
    encoded[owner..owner + 4].copy_from_slice(&3_u32.to_le_bytes());
    assert!(matches!(
        decode(
            &encoded,
            &original,
            limits(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000)
        ),
        Err(Error::NominalOwner { .. })
    ));
    let variant = original.variant_domains().domains().next().unwrap();
    let name = variant.nominal().as_str().as_bytes();
    let source = encode(&original);
    let start = source
        .windows(name.len())
        .position(|bytes| bytes == name)
        .unwrap();
    for offset in [start + name.len() - 1, start + name.len()] {
        let mut changed = source.clone();
        changed[offset] ^= 1;
        assert!(matches!(
            decode(
                &changed,
                &original,
                limits(),
                &mut TaskSemanticMeter::new(1_000_000, 1_000_000)
            ),
            Err(Error::NominalOwner { .. })
        ));
    }
    let mut encoded = encode(&original);
    let missing = encoded
        .windows(7)
        .position(|bytes| bytes == b"Missing")
        .unwrap();
    encoded[missing..missing + 7].copy_from_slice(b"Present");
    assert!(matches!(
        decode(
            &encoded,
            &original,
            limits(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000)
        ),
        Err(Error::VariantDomain(
            crate::plan::RuntimeVariantDomainError::DuplicateCaseName { .. }
        ))
    ));
}

#[test]
fn nominal_sources_reject_an_owner_shared_by_record_and_variant_tables() {
    let mut variants = candidate(RuntimeNominalRecordShape::Record, 2, true);
    let owner = variants
        .nominal_record_domains()
        .domains()
        .next()
        .unwrap()
        .owner();
    let (nominal, layout) = nominal_owner(variants.type_table(), owner).unwrap();
    variants.variant_domains = RuntimeVariantDomainTable::from_source_domains(vec![
        RuntimeVariantDomain::from_admitted_parts(
            owner,
            nominal.clone(),
            layout,
            [("Only".to_owned(), None)],
        ),
    ])
    .unwrap();
    assert!(matches!(
        variants.prepare_nominal_sources_encoding(
            limits(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000)
        ),
        Err(Error::NominalKind { .. })
    ));
    variants.nominal_record_domains =
        RuntimeNominalRecordDomainTable::from_source_domains(vec![]).unwrap();
    let variant_only = encode(&variants);
    let mut records = candidate(RuntimeNominalRecordShape::Record, 2, true);
    records.variant_domains = RuntimeVariantDomainTable::from_source_domains(vec![]).unwrap();
    let record_only = encode(&records);
    let mut joined = record_only[..record_only.len() - 4].to_vec();
    joined.extend_from_slice(&variant_only[DOMAIN.len() + 1 + 4..]);
    assert!(matches!(
        decode(
            &joined,
            &records,
            limits(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000)
        ),
        Err(Error::NominalKind { .. })
    ));
}

#[test]
fn nominal_source_work_stays_on_the_precharged_sticky_bank_and_output_is_reserved() {
    let original = candidate(RuntimeNominalRecordShape::Record, 2, true);
    let encoded = encode(&original);
    let mut measured = TaskSemanticMeter::new(1_000_000, 1_000_000);
    decode(&encoded, &original, limits(), &mut measured).unwrap();
    let work = measured.totals().0;
    let mut exact = TaskSemanticMeter::new(work + 1, 1_000_000);
    exact.charge_work(1).unwrap();
    decode(&encoded, &original, limits(), &mut exact).unwrap();
    assert_eq!(exact.totals().0, work + 1);
    let mut short = TaskSemanticMeter::new(work, 1_000_000);
    short.charge_work(1).unwrap();
    assert_eq!(
        decode(&encoded, &original, limits(), &mut short)
            .err()
            .unwrap(),
        Error::Resource(TaskSemanticEncodingError::SemanticWork)
    );
    assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
    assert!(matches!(
        original
            .prepare_nominal_sources_encoding(limits(), &mut short)
            .err()
            .unwrap(),
        Error::Resource(TaskSemanticEncodingError::SemanticWork)
    ));
    let mut output = Vec::with_capacity(encoded.len() + 1);
    output.push(77);
    original
        .prepare_nominal_sources_encoding(
            limits(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap()
        .append_to(&mut output)
        .unwrap();
    assert_eq!(output[0], 77);
    assert_eq!(&output[1..], encoded);
    let mut short = Vec::with_capacity(encoded.len() - 1);
    let capacity = short.capacity();
    assert!(matches!(
        original
            .prepare_nominal_sources_encoding(
                limits(),
                &mut TaskSemanticMeter::new(1_000_000, 1_000_000)
            )
            .unwrap()
            .append_to(&mut short),
        Err(Error::OutputCapacity { .. })
    ));
    assert!(short.is_empty());
    assert_eq!(short.capacity(), capacity);
}

fn candidate(shape: RuntimeNominalRecordShape, count: usize, policy: bool) -> RuntimePlanInventory {
    builder(shape, count, policy)
        .prepare_inventory(RuntimeTaskPlanSealLimits::default(), 0)
        .unwrap()
}
