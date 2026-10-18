//! Static observation literals use their original domain owner in both phases.

use super::*;
use crate::effect::RuntimeAssignment;
use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
use crate::entry::{RuntimeNominalRecordShape, RuntimeSchemaLimits};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::value_admission::PlanValueAuthority;
use crate::plan::*;
use crate::program_types::RuntimeProgramTypes;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::{AwbcRuntimeValueSnapshot as Snapshot, RuntimeLiteralSourceLimits};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn builder() -> RuntimePlanBuilder {
    let mut builder = crate::plan::type_table::source_codec::domains::tests::builder(
        RuntimeNominalRecordShape::Record,
        2,
        true,
    );
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                semantic(240),
                RuntimePlanTypeProjection::Unit,
            )],
            [],
        )
        .unwrap();
    builder
}
fn inventory() -> RuntimePlanInventory {
    builder()
        .prepare_inventory(RuntimeTaskPlanSealLimits::default(), 0)
        .unwrap()
}
fn record(inventory: &RuntimePlanInventory) -> RuntimeValue {
    RuntimeProgramTypes::Plan(inventory)
        .try_record_value(
            semantic(1),
            vec![RuntimeValue::Bool(true), RuntimeValue::Bool(false)],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap()
}
fn resources(limits: RuntimeSchemaLimits) -> ValueResourceAdmission {
    ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::InertSnapshot)
}
fn bank() -> ValueResourceAdmission {
    resources(RuntimeSchemaLimits::engine_default())
}
fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(u64::MAX, u64::MAX)
}
fn limits() -> RuntimeLiteralSourceLimits {
    RuntimeLiteralSourceLimits {
        encoded_bytes: 1 << 20,
        decoded_bytes: 1 << 20,
        elements: 10_000,
    }
}
fn encode(source: &Snapshot) -> Vec<u8> {
    let prepared = source
        .prepare_literal_source_encoding(limits(), &bank(), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}
fn opaque(source: Snapshot) -> Snapshot {
    let value = RuntimeOpaqueTypeOwner::exact(
        crate::pattern::RuntimeOpaqueTypeProducerId::try_new("fixture.ContextFreeBox").unwrap(),
        semantic(99),
    )
    .try_wrap(RuntimeValue::Unit)
    .unwrap();
    let mut snapshot = Snapshot::from_admitted_storage(&value, None).unwrap();
    let Snapshot::Opaque(opaque) = &mut snapshot else {
        unreachable!()
    };
    opaque.payload = Box::new(source);
    snapshot
}
fn authority(builder: &RuntimePlanBuilder) -> PlanValueAuthority<'_> {
    PlanValueAuthority::Building {
        types: &builder.types,
        records: &builder.nominal_record_domains,
        variants: &builder.variant_domains,
    }
}
fn reserve(builder: &mut RuntimePlanBuilder, tag: u8) -> RuntimeFunctionSiteSeedId {
    builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([tag; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs: Box::new([]),
            result: semantic(240),
            body_kind: RuntimeFunctionSiteBodyKind::Executable,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap()
}
fn body(effects: Vec<LineEffectRequest>) -> RuntimeFunctionSiteBodySeed {
    RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
        effects: RuntimeEffectSet::empty(),
        ops: effects
            .into_iter()
            .map(|effect| RuntimeFlowOpSeed::Effect(RuntimeLineEffectSeed::Static(effect)))
            .chain([RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                semantic(240),
                RuntimeExprSeedKind::Value(RuntimeValue::Unit),
            ))])
            .collect(),
    })
}

#[test]
fn static_literal_sources_preserve_dynamic_roots_and_original_nominal_descendants() {
    let inventory = inventory();
    // No tuple, integer, string, float, Option or opaque type is declared.
    assert_eq!(inventory.type_table().len(), 4);
    let record = record(&inventory);
    let variant = RuntimeProgramTypes::Plan(&inventory)
        .try_variant_value(
            semantic(2),
            1,
            Some(RuntimeValue::Bool(true)),
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
    let opaque = RuntimeOpaqueTypeOwner::exact(
        crate::pattern::RuntimeOpaqueTypeProducerId::try_new("fixture.ContextFreeBox").unwrap(),
        semantic(99),
    )
    .try_wrap(record.clone())
    .unwrap();
    let values = [
        RuntimeValue::Tuple(vec![
            record.clone(),
            variant,
            RuntimeValue::u64(u64::MAX),
            RuntimeValue::String("original".into()),
        ]),
        RuntimeValue::option_some(record),
        opaque,
        RuntimeValue::F32(f32::from_bits(0x7fc0_1234)),
    ];
    let building = builder();
    for value in values {
        let source = Snapshot::from_admitted_storage(&value, None).unwrap();
        let bytes = encode(&source);
        for authority in [
            PlanValueAuthority::Inventory(&inventory),
            authority(&building),
        ] {
            authority
                .validate_static_literal(&value, &mut bank())
                .unwrap();
            let restored = authority
                .decode_static_literal_source_value(&bytes, limits(), &mut bank(), &mut meter())
                .unwrap();
            assert_eq!(
                Snapshot::from_admitted_storage(&restored, None).unwrap(),
                source
            );
            assert_eq!(
                encode(&Snapshot::from_admitted_storage(&restored, None).unwrap()),
                bytes
            );
        }
    }
}

#[test]
fn static_literal_sources_check_nominal_domains_under_untyped_containers_before_mapping() {
    let inventory = inventory();
    let original = Snapshot::from_admitted_storage(&record(&inventory), None).unwrap();
    for mutation in 0..6 {
        let mut changed = original.clone();
        let Snapshot::NominalRecord(record) = &mut changed else {
            unreachable!()
        };
        match mutation {
            0 => record.semantic_identity = semantic(77),
            1 => record.layout = crate::entry::TypeLayoutHash::from_bytes([77; 32]),
            2 => {
                record.type_id =
                    crate::entry::RuntimeNominalTypeId::try_new("fixture.Foreign").unwrap()
            }
            3 => record.fields[1].field = record.fields[0].field,
            4 => record.fields[0].value = Snapshot::String("wrong bool".into()),
            5 => {
                record.fields.pop();
            }
            _ => unreachable!(),
        }
        let source = Snapshot::Tuple(vec![opaque(changed)]);
        let mut meter = meter();
        assert!(
            PlanValueAuthority::Inventory(&inventory)
                .decode_static_literal_source_value(
                    &encode(&source),
                    limits(),
                    &mut bank(),
                    &mut meter,
                )
                .is_err(),
            "nominal mutation {mutation}"
        );
        assert_eq!(
            meter.status(),
            Err(TaskSemanticEncodingError::OwnerRejected)
        );
        assert!(
            PlanValueAuthority::Inventory(&inventory)
                .decode_static_literal_source_value(
                    &encode(&original),
                    limits(),
                    &mut bank(),
                    &mut meter,
                )
                .is_err()
        );
    }
}

#[test]
fn static_literal_sources_share_precharged_work_and_aggregate_value_nodes() {
    let inventory = inventory();
    let source = Snapshot::from_admitted_storage(&record(&inventory), None).unwrap();
    let authority = PlanValueAuthority::Inventory(&inventory);
    let mut measured = bank();
    authority
        .materialize_static_literal_source_value(source.clone(), &mut measured)
        .unwrap();
    let work = measured.work().consumed();
    for (precharge, allowance, accepted) in [
        (0, work, true),
        (0, work - 1, false),
        (1, work + 1, true),
        (1, work, false),
    ] {
        let mut bank = resources(RuntimeSchemaLimits {
            max_validation_work: allowance,
            ..RuntimeSchemaLimits::engine_default()
        });
        for _ in 0..precharge {
            bank.work().charge(0).unwrap();
        }
        let result = authority.materialize_static_literal_source_value(source.clone(), &mut bank);
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(bank.work().consumed(), work + precharge);
        } else {
            let failure = bank.work().failure().unwrap();
            assert!(
                authority
                    .materialize_static_literal_source_value(source.clone(), &mut bank)
                    .is_err()
            );
            assert_eq!(bank.work().failure(), Some(failure));
        }
    }
    let mut bank = resources(RuntimeSchemaLimits {
        max_nodes: 3,
        ..RuntimeSchemaLimits::engine_default()
    });
    authority
        .materialize_static_literal_source_value(source, &mut bank)
        .unwrap();
    assert!(
        authority
            .materialize_static_literal_source_value(Snapshot::Bool(true), &mut bank)
            .is_err()
    );
}

#[test]
fn static_literal_sources_reject_affine_and_runtime_only_descendants_before_live_mapping() {
    let inventory = inventory();
    let mut affine = opaque(Snapshot::Unit);
    let Snapshot::Opaque(leaf) = &mut affine else {
        unreachable!()
    };
    leaf.producer = crate::value::RuntimeHandleKind::Cue.try_producer().unwrap();
    leaf.value_class =
        crate::value::RuntimeOpaqueValueClass::AffineHandle(crate::value::RuntimeHandleKind::Cue);
    leaf.persistence = crate::value::RuntimeOpaquePersistence::SnapshotOnly;
    let source = Snapshot::Tuple(vec![affine]);
    assert_eq!(
        PlanValueAuthority::Inventory(&inventory)
            .materialize_static_literal_source_value(source, &mut bank()),
        Err(crate::value::AwbcRuntimeValueSnapshotError::AffineValue)
    );
    let callable = Snapshot::Callable(crate::value::AwbcRuntimeCallableSnapshot {
        state: crate::runtime_id::RuntimeCallableStateId::from_accepted_ordinal(
            std::num::NonZeroU32::new(1).unwrap(),
        ),
        retained: vec![],
    });
    assert!(
        PlanValueAuthority::Inventory(&inventory)
            .materialize_static_literal_source_value(Snapshot::Tuple(vec![callable]), &mut bank())
            .is_err()
    );
}

#[test]
fn static_assignment_sources_reach_original_factories_and_common_executable_seal() {
    let inventory = inventory();
    let original = RuntimeValue::Tuple(vec![record(&inventory), RuntimeValue::u64(u64::MAX)]);
    let source = Snapshot::from_admitted_storage(&original, None).unwrap();
    let mut restored = builder();
    let value = authority(&restored)
        .decode_static_literal_source_value(&encode(&source), limits(), &mut bank(), &mut meter())
        .unwrap();
    let mut expected = builder();
    for (builder, value) in [(&mut restored, value), (&mut expected, original)] {
        let handle = reserve(builder, 241);
        builder
            .define_function_site_seed(
                &handle,
                body(vec![
                    LineEffectRequest::SignalWrite(
                        RuntimeAssignment::try_copy("signal.original".into(), &value).unwrap(),
                    ),
                    LineEffectRequest::MetricWrite(
                        RuntimeAssignment::try_new("metric.original".into(), value).unwrap(),
                    ),
                ]),
            )
            .unwrap();
    }
    let restored = restored.finish().unwrap();
    let expected = expected.finish().unwrap();
    assert_eq!(restored.function_sites(), expected.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        expected.executable_semantic_digest()
    );
}

#[test]
fn static_assignment_factories_reject_bad_nominal_descendants_and_poison_publication() {
    let inventory = inventory();
    let RuntimeValue::NominalRecord(record) = record(&inventory) else {
        unreachable!()
    };
    let bad = record
        .try_map_fields(|_| {
            Ok::<_, std::convert::Infallible>(RuntimeValue::String("wrong bool".into()))
        })
        .unwrap_or_else(|never| match never {});
    for signal in [false, true] {
        let mut builder = builder();
        let prefix = reserve(&mut builder, 242);
        builder
            .define_function_site_seed(&prefix, body(vec![]))
            .unwrap();
        let later = reserve(&mut builder, 243);
        let assignment = RuntimeAssignment::try_new(
            "observation.original".into(),
            RuntimeValue::Tuple(vec![RuntimeValue::NominalRecord(bad.clone())]),
        )
        .unwrap();
        let effect = if signal {
            LineEffectRequest::SignalWrite(assignment)
        } else {
            LineEffectRequest::MetricWrite(assignment)
        };
        assert!(matches!(
            builder.define_function_site_seed(&later, body(vec![effect])),
            Err(RuntimePlanBuildError::StaticEffectLiteral(_))
        ));
        assert!(builder.function_sites[0].body.is_some());
        assert!(builder.function_sites[1].body.is_none());
        assert!(matches!(
            builder.finish(),
            Err(RuntimePlanBuildError::Poisoned)
        ));
    }
}
