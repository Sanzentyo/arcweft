use super::*;
use crate::entry::schema::{RuntimeBytesFormat, RuntimeCodecUse, RuntimeFieldCodecUse};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeFunctionTypeContract, RuntimePlanBuilder, RuntimePlanInventory, RuntimePlanRecordField,
    RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed, RuntimeTypeBinder, RuntimeTypeScope,
};

fn semantic(value: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([value; 32])
}
fn inventory(seeds: impl IntoIterator<Item = RuntimePlanTypeSeed>) -> RuntimePlanInventory {
    let mut builder = RuntimePlanBuilder::new();
    builder.admit_type_batch(seeds, []).unwrap();
    let inventory = builder
        .prepare_inventory(crate::plan::RuntimeTaskPlanSealLimits::default())
        .unwrap();
    inventory.verify().unwrap();
    inventory
}
fn row_digest(
    inventory: &RuntimePlanInventory,
    tag: u8,
    work: u64,
    bytes: u64,
) -> (Result<blake3::Hash, RuntimeBodySemanticError>, (u64, u64)) {
    let ty = inventory
        .type_table()
        .id_for_semantic(semantic(tag))
        .unwrap();
    let row = inventory.type_table().get(ty).unwrap();
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let digest =
        row.executable_semantic_row_digest(&RuntimeBodySemanticContext::new(inventory), &mut meter);
    (digest, meter.totals())
}

#[test]
fn actual_bool_row_has_exact_version_one_owner_grammar() {
    let inventory = inventory([RuntimePlanTypeSeed::new(semantic(1), Type::Bool)]);
    let mut expected = b"arcweft.runtime-plan.executable-row.v1\0".to_vec();
    expected.extend_from_slice(&[0, 3]);
    expected.extend_from_slice(semantic(1).as_bytes());
    expected.push(0); // accepted nominal declaration absent
    expected.extend_from_slice(&0_u32.to_le_bytes()); // root scope
    expected.extend_from_slice(&0_u32.to_le_bytes()); // no type children
    expected.push(0); // no occurrence codec
    let (actual, (work, bytes)) = row_digest(&inventory, 1, 1_000, 10_000);
    assert_eq!(actual.unwrap(), blake3::hash(&expected));
    assert_eq!(work, 7);
    assert_eq!(bytes, u64::try_from(expected.len()).unwrap());
}

#[test]
fn source_diagnostic_names_are_excluded_but_child_roles_are_ordered() {
    let make = |names: [&str; 2], reverse: bool| {
        inventory([
            RuntimePlanTypeSeed::new(
                semantic(1),
                Type::Record(Box::new([
                    RuntimePlanRecordField::new(names[0], semantic(if reverse { 3 } else { 2 })),
                    RuntimePlanRecordField::new(names[1], semantic(if reverse { 2 } else { 3 })),
                ])),
            ),
            RuntimePlanTypeSeed::new(semantic(2), Type::Bool),
            RuntimePlanTypeSeed::new(semantic(3), Type::String),
        ])
    };
    let first = make(["first", "second"], false);
    let renamed = make(["debug_a", "debug_b"], false);
    let reversed = make(["first", "second"], true);
    assert_eq!(
        row_digest(&first, 1, 1_000, 10_000).0.unwrap(),
        row_digest(&renamed, 1, 1_000, 10_000).0.unwrap()
    );
    assert_ne!(
        row_digest(&first, 1, 1_000, 10_000).0.unwrap(),
        row_digest(&reversed, 1, 1_000, 10_000).0.unwrap()
    );
}

#[test]
fn actual_codec_and_lexical_contract_mutations_change_the_owner_row() {
    let bytes = |format| {
        inventory([RuntimePlanTypeSeed::new(semantic(1), Type::Bytes)
            .with_data_codec(RuntimeCodecUse::Bytes { format })])
    };
    assert_ne!(
        row_digest(&bytes(RuntimeBytesFormat::Binary), 1, 1_000, 10_000)
            .0
            .unwrap(),
        row_digest(&bytes(RuntimeBytesFormat::Base64), 1, 1_000, 10_000)
            .0
            .unwrap()
    );
    let bound = |arity| {
        let scope = RuntimeTypeScope::root()
            .enter(RuntimeTypeBinder::new(arity, 0, 0))
            .unwrap();
        inventory([RuntimePlanTypeSeed::new(
            semantic(1),
            Type::BoundType(scope.bound_type(0, 0).unwrap()),
        )
        .with_scope(scope)])
    };
    assert_ne!(
        row_digest(&bound(1), 1, 1_000, 10_000).0.unwrap(),
        row_digest(&bound(2), 1, 1_000, 10_000).0.unwrap()
    );
    let function = |effect| {
        inventory([
            RuntimePlanTypeSeed::new(
                semantic(1),
                Type::Function {
                    contract: RuntimeFunctionTypeContract::monomorphic(
                        crate::effect_row::EffectSet::from_labels([effect]).unwrap(),
                    ),
                    parameters: Box::new([]),
                    result: semantic(2),
                },
            ),
            RuntimePlanTypeSeed::new(semantic(2), Type::Bool),
        ])
    };
    assert_ne!(
        row_digest(&function("fs.read"), 1, 1_000, 10_000)
            .0
            .unwrap(),
        row_digest(&function("fs.write"), 1, 1_000, 10_000)
            .0
            .unwrap()
    );
}

#[test]
fn exact_owner_row_budget_is_shared_and_first_failure_is_sticky() {
    let inventory = inventory([RuntimePlanTypeSeed::new(semantic(1), Type::Bytes)
        .with_data_codec(RuntimeCodecUse::Bytes {
            format: RuntimeBytesFormat::Hex,
        })]);
    let (expected, (work, bytes)) = row_digest(&inventory, 1, 1_000, 10_000);
    assert_eq!(
        row_digest(&inventory, 1, work, bytes).0.unwrap(),
        expected.unwrap()
    );
    assert!(matches!(
        row_digest(&inventory, 1, work - 1, bytes).0,
        Err(RuntimeBodySemanticError::Encoding(
            crate::task::semantic::TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert!(matches!(
        row_digest(&inventory, 1, work, bytes - 1).0,
        Err(RuntimeBodySemanticError::Encoding(
            crate::task::semantic::TaskSemanticEncodingError::TranscriptBytes
        ))
    ));
    let mut meter = TaskSemanticMeter::new(0, 10_000);
    let _ = meter.charge_work(1);
    let before = meter.totals();
    let row = inventory.type_table().declarations().next().unwrap();
    assert!(matches!(
        row.executable_semantic_row_digest(
            &RuntimeBodySemanticContext::new(&inventory),
            &mut meter
        ),
        Err(RuntimeBodySemanticError::Encoding(
            crate::task::semantic::TaskSemanticEncodingError::SemanticWork
        ))
    ));
    assert_eq!(meter.totals(), before);
}

#[test]
fn codec_default_rejects_an_unbound_program_without_emitting_a_digest() {
    let inventory = inventory([]);
    let program = arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([21; 32]);
    let codec = RuntimeCodecUse::Record {
        name: "Wire".into(),
        deny_unknown_fields: false,
        fields: Box::new([RuntimeFieldCodecUse {
            wire_name: "value".into(),
            has_default: true,
            default_program: Some(program),
            skip: false,
            bytes_format: None,
            value: RuntimeCodecUse::Plain,
        }]),
    };
    let mut meter = TaskSemanticMeter::new(1_000, 10_000);
    let mut encoder = TaskSemanticEncoder::new(b"codec-test.v1\0", &mut meter);
    assert!(
        matches!(codec.encode_executable_policy(&RuntimeBodySemanticContext::new(&inventory), &mut encoder), Err(RuntimeBodySemanticError::UnknownPureProgram { program: actual }) if actual == program)
    );
    assert_eq!(
        encoder.finish(),
        Err(crate::task::semantic::TaskSemanticEncodingError::OwnerRejected)
    );
}

#[test]
fn accepted_nominal_identity_and_layout_are_included_without_source_spelling() {
    use crate::entry::{
        RuntimeNominalDeclarationId, RuntimeNominalRecordShape, RuntimeNominalSchemaBody,
        RuntimeNominalSchemaDefinition, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId, RuntimeSchemaLimits,
    };
    use crate::plan::RuntimeNominalRecordDomainSeed;
    let make = |name: &str, shape| {
        let nominal = RuntimeNominalTypeId::try_new(name).unwrap();
        let graph = RuntimeNominalSchemaGraph::try_new(
            vec![RuntimeNominalSchemaDefinition::new(
                RuntimeNominalDeclarationId::from_bytes([61; 32]),
                RuntimeNominalSchemaIdentity::new(nominal.clone(), semantic(1)),
                vec![],
                RuntimeNominalSchemaBody::Record {
                    shape,
                    fields: Box::new([]),
                },
            )],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_semantic_batch(
                [RuntimePlanTypeSeed::new(
                    semantic(1),
                    Type::Nominal {
                        nominal,
                        layout: graph.try_layout_hash(semantic(1)).unwrap(),
                        arguments: Box::new([]),
                    },
                )],
                [],
                [RuntimeNominalRecordDomainSeed::new(semantic(1), shape, [])],
                [],
                &graph,
            )
            .unwrap();
        let inventory = builder
            .prepare_inventory(crate::plan::RuntimeTaskPlanSealLimits::default())
            .unwrap();
        inventory.verify().unwrap();
        inventory
    };
    let a = make("fixture.First", RuntimeNominalRecordShape::Unit);
    let b = make("fixture.Second", RuntimeNominalRecordShape::Unit);
    let c = make("fixture.First", RuntimeNominalRecordShape::Tuple);
    assert_ne!(
        row_digest(&a, 1, 1_000, 10_000).0.unwrap(),
        row_digest(&b, 1, 1_000, 10_000).0.unwrap()
    );
    assert_ne!(
        row_digest(&a, 1, 1_000, 10_000).0.unwrap(),
        row_digest(&c, 1, 1_000, 10_000).0.unwrap()
    );
}

#[test]
fn codec_default_reference_resolves_the_actual_function_not_binding_address() {
    use crate::plan::{
        RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionDefinitionIdentity,
        RuntimeFunctionSemanticRole, RuntimeFunctionSiteBodyKind,
        RuntimeFunctionSiteDeclarationSeed, RuntimePureProgramBindingSeed,
    };
    let make = |padding: bool, alias: u8, definition: u8| {
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch([RuntimePlanTypeSeed::new(semantic(1), Type::Bool)], [])
            .unwrap();
        if padding {
            builder
                .push_function_site_seed(
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity([99; 32]),
                    RuntimeFunctionSemanticRole::Ordinary,
                    [],
                    RuntimeExprSeed::new(
                        semantic(1),
                        RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(false)),
                    ),
                )
                .unwrap();
        }
        let site = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [definition; 32],
                ),
                role: RuntimeFunctionSemanticRole::Ordinary,
                function_type: None,
                inputs: Box::new([]),
                result: semantic(1),
                body_kind: RuntimeFunctionSiteBodyKind::Expression,
                effects: crate::plan::RuntimeEffectSet::empty(),
            })
            .unwrap();
        builder
            .define_function_site_seed(
                &site,
                RuntimeExprSeed::new(
                    semantic(1),
                    RuntimeExprSeedKind::Value(crate::value::RuntimeValue::Bool(true)),
                ),
            )
            .unwrap();
        let program =
            arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([alias; 32]);
        builder
            .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, site })
            .unwrap();
        let inventory = builder
            .prepare_inventory(crate::plan::RuntimeTaskPlanSealLimits::default())
            .unwrap();
        inventory.verify().unwrap();
        let codec = RuntimeCodecUse::Record {
            name: "Wire".into(),
            deny_unknown_fields: false,
            fields: Box::new([RuntimeFieldCodecUse {
                wire_name: "value".into(),
                has_default: true,
                default_program: Some(program),
                skip: false,
                bytes_format: None,
                value: RuntimeCodecUse::Plain,
            }]),
        };
        let mut meter = TaskSemanticMeter::new(1_000, 10_000);
        let mut encoder = TaskSemanticEncoder::new(b"codec-default-test.v1\0", &mut meter);
        codec
            .encode_executable_policy(&RuntimeBodySemanticContext::new(&inventory), &mut encoder)
            .unwrap();
        encoder.finish().unwrap()
    };
    assert_eq!(make(false, 21, 31), make(true, 22, 31));
    assert_ne!(make(false, 21, 31), make(false, 21, 32));
}

#[test]
fn codec_diagnostic_type_names_are_excluded_and_wire_roles_remain_semantic() {
    let inventory = inventory([]);
    let digest = |codec: &RuntimeCodecUse| {
        let mut meter = TaskSemanticMeter::new(1_000, 10_000);
        let mut encoder = TaskSemanticEncoder::new(b"codec-name-test.v1\0", &mut meter);
        codec
            .encode_executable_policy(&RuntimeBodySemanticContext::new(&inventory), &mut encoder)
            .unwrap();
        encoder.finish().unwrap()
    };
    let record = |name: &str, wire: &str, deny| RuntimeCodecUse::Record {
        name: name.into(),
        deny_unknown_fields: deny,
        fields: Box::new([RuntimeFieldCodecUse {
            wire_name: wire.into(),
            has_default: false,
            default_program: None,
            skip: false,
            bytes_format: None,
            value: RuntimeCodecUse::Plain,
        }]),
    };
    assert_eq!(
        digest(&record("OriginalSourceName", "value", false)),
        digest(&record("RenamedSourceName", "value", false))
    );
    assert_ne!(
        digest(&record("OriginalSourceName", "value", false)),
        digest(&record("OriginalSourceName", "renamed_wire", false))
    );
    assert_ne!(
        digest(&record("OriginalSourceName", "value", false)),
        digest(&record("OriginalSourceName", "value", true))
    );
    let variant = |name: &str, wire: &str| RuntimeCodecUse::Enum {
        name: name.into(),
        tag: crate::entry::schema::RuntimeEnumTagStyle::External,
        repr: None,
        cases: Box::new([crate::entry::schema::RuntimeVariantCodecUse {
            wire_name: wire.into(),
            discriminant: None,
            payload: None,
        }]),
    };
    assert_eq!(
        digest(&variant("OriginalSourceEnum", "Unit")),
        digest(&variant("RenamedSourceEnum", "Unit"))
    );
    assert_ne!(
        digest(&variant("OriginalSourceEnum", "Unit")),
        digest(&variant("OriginalSourceEnum", "RenamedWireCase"))
    );
}
