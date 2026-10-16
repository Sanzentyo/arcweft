use super::super::RuntimePlanTypeTableBuilder;
use super::*;
use crate::effect_row::{DecisionControl, DecisionWork, EffectFormula, EffectPredicate, EffectSet};
use crate::entry::schema::{
    RuntimeBytesFormat, RuntimeCodecUse, RuntimeEnumRepr, RuntimeEnumTagStyle,
    RuntimeFieldCodecUse, RuntimeVariantCodecUse,
};
use crate::entry::{RuntimeMapKind, RuntimeNominalTypeId, TypeLayoutHash};
use crate::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeProducerId,
};
use crate::plan::{
    RuntimeAgentOperationalType, RuntimeAgentTypeProjection, RuntimeArrayLength,
    RuntimeFunctionTypeContract, RuntimePlanRecordField, RuntimePlanSequenceKind,
    RuntimePlanTypeProjection as P, RuntimePlanTypeSeed, RuntimeTypeBinder,
};
use crate::value::{
    RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass,
    RuntimeSignedIntWidth as S, RuntimeUnsignedIntWidth as U,
};

fn identity(value: u32) -> RuntimeSemanticTypeId {
    let mut bytes = [0; 32];
    bytes[..4].copy_from_slice(&value.to_le_bytes());
    RuntimeSemanticTypeId::from_bytes(bytes)
}
fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(u64::MAX, u64::MAX)
}
fn limits() -> RuntimeTypeSourceLimits {
    RuntimeTypeSourceLimits {
        rows: 1024,
        elements: 65536,
        encoded_bytes: 1 << 20,
        decoded_bytes: 1 << 20,
        policy_depth: 128,
    }
}
fn table(seeds: Vec<RuntimePlanTypeSeed>) -> RuntimePlanTypeTable {
    let mut builder = RuntimePlanTypeTableBuilder::new();
    builder.intern_batch(seeds).unwrap();
    builder.finish().unwrap()
}
fn add(
    seeds: &mut Vec<RuntimePlanTypeSeed>,
    projection: P<RuntimeSemanticTypeId>,
) -> RuntimeSemanticTypeId {
    let id = identity(u32::try_from(seeds.len() + 1).unwrap());
    seeds.push(RuntimePlanTypeSeed::new(id, projection));
    id
}
fn one() -> RuntimePlanTypeTable {
    table(vec![RuntimePlanTypeSeed::new(identity(1), P::Unit)])
}
struct Algebra;
impl DecisionControl for Algebra {
    type Error = std::convert::Infallible;
    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn full_table() -> RuntimePlanTypeTable {
    let mut seeds = Vec::new();
    let unit = add(&mut seeds, P::Unit);
    let payload = add(&mut seeds, P::Tuple(vec![unit].into_boxed_slice()));
    for projection in [
        P::Never,
        P::Bool,
        P::F32,
        P::F64,
        P::String,
        P::Color,
        P::Char,
        P::Bytes,
        P::Duration,
        P::Progress,
        P::EntityReference,
        P::AgentValue,
    ] {
        add(&mut seeds, projection);
    }
    for width in [S::I8, S::I16, S::I32, S::I64, S::I128, S::ISize] {
        add(&mut seeds, P::Signed(width));
    }
    for width in [U::U8, U::U16, U::U32, U::U64, U::U128, U::USize] {
        add(&mut seeds, P::Unsigned(width));
    }
    for dense in crate::math::RuntimeDenseNumericType::ALL {
        add(&mut seeds, P::DenseNumeric(dense));
    }
    for projection in [
        P::Range(unit),
        P::Iterator(unit),
        P::Need(unit),
        P::ThreadHandle(unit),
        P::Shared(unit),
        P::Reference(unit),
    ] {
        add(&mut seeds, projection);
    }
    for kind in [
        RuntimePlanSequenceKind::Vec,
        RuntimePlanSequenceKind::Array,
        RuntimePlanSequenceKind::Slice,
        RuntimePlanSequenceKind::Seq,
    ] {
        add(&mut seeds, P::Sequence { kind, item: unit });
    }
    add(
        &mut seeds,
        P::Array {
            item: unit,
            length: RuntimeArrayLength::Constant(u64::MAX),
        },
    );
    for kind in [
        RuntimeMapKind::Ordered,
        RuntimeMapKind::Sorted,
        RuntimeMapKind::BTree,
    ] {
        add(
            &mut seeds,
            P::Map {
                kind,
                key: unit,
                value: unit,
            },
        );
    }
    add(
        &mut seeds,
        P::Stream {
            item: unit,
            error: unit,
        },
    );
    add(
        &mut seeds,
        P::Option {
            item: unit,
            some_payload: payload,
        },
    );
    add(
        &mut seeds,
        P::Result {
            value: unit,
            error: unit,
            value_payload: payload,
            error_payload: payload,
        },
    );
    for tag in 0..=6 {
        let owner = RuntimeBuiltinVariantIdentity::from_wire_tag(tag).unwrap();
        let cases = owner
            .cases()
            .iter()
            .map(|case| case.has_payload().then_some(payload))
            .collect();
        add(&mut seeds, P::BuiltinVariant { owner, cases });
    }
    let binder = RuntimeTypeBinder::new(1, 1, 1);
    let child = RuntimeTypeScope::root().enter(binder).unwrap();
    let bound = identity(u32::try_from(seeds.len() + 1).unwrap());
    seeds.push(
        RuntimePlanTypeSeed::new(bound, P::BoundType(child.bound_type(0, 0).unwrap()))
            .with_scope(child.clone()),
    );
    let array = identity(u32::try_from(seeds.len() + 1).unwrap());
    seeds.push(
        RuntimePlanTypeSeed::new(
            array,
            P::Array {
                item: unit,
                length: RuntimeArrayLength::Bound(child.bound_const(0, 0).unwrap()),
            },
        )
        .with_scope(child.clone()),
    );
    let invocation =
        EffectFormula::variable(child.bound_effect(0, 0).unwrap(), &mut Algebra).unwrap();
    add(
        &mut seeds,
        P::Function {
            contract: RuntimeFunctionTypeContract::new(
                binder,
                EffectPredicate::unconstrained(),
                invocation,
            ),
            parameters: vec![bound, array].into_boxed_slice(),
            result: bound,
        },
    );
    add(
        &mut seeds,
        P::Nominal {
            nominal: RuntimeNominalTypeId::try_new("source::Nominal").unwrap(),
            layout: TypeLayoutHash::from_bytes([42; 32]),
            arguments: vec![unit].into_boxed_slice(),
        },
    );
    add(
        &mut seeds,
        P::Record(vec![RuntimePlanRecordField::new("original\0名前", unit)].into_boxed_slice()),
    );
    add(
        &mut seeds,
        P::Choice(vec![unit, payload].into_boxed_slice()),
    );
    for class in [
        RuntimeOpaqueValueClass::Plain,
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor),
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
        RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Voice),
    ] {
        add(
            &mut seeds,
            P::Opaque {
                producer: RuntimeOpaqueTypeProducerId::try_new("source::Opaque").unwrap(),
                admission: RuntimeOpaqueTypeAdmission::ProducerWide,
                value_class: class,
                persistence: RuntimeOpaquePersistence::SnapshotOnly,
                arguments: vec![unit].into_boxed_slice(),
            },
        );
    }
    for tag in 0..=35 {
        let Some(kind) = RuntimeAgentOperationalType::from_semantic_tag(tag) else {
            continue;
        };
        let agent = match kind {
            RuntimeAgentOperationalType::Probe => RuntimeAgentTypeProjection::Probe(unit),
            RuntimeAgentOperationalType::DataShape => RuntimeAgentTypeProjection::DataShape(unit),
            kind => RuntimeAgentTypeProjection::try_leaf(kind).unwrap(),
        };
        add(&mut seeds, P::Agent(agent));
    }
    table(seeds)
}

#[test]
fn complete_original_type_algebra_round_trips_through_existing_graph_admission() {
    let original = full_table();
    let mut kinds = [false; 37];
    for declaration in original.declarations() {
        kinds[declaration.projection().executable_semantic_kind() as usize] = true;
    }
    assert!(kinds.into_iter().all(|covered| covered));
    let bytes = original.encode_sources(limits(), &mut meter()).unwrap();
    let decoded = RuntimePlanTypeTable::decode_sources(&bytes, limits(), &mut meter()).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(
        decoded.encode_sources(limits(), &mut meter()).unwrap(),
        bytes
    );
    for (id, declaration) in original.declarations_with_ids() {
        assert_eq!(decoded.get(id), Some(declaration));
    }
}

#[test]
fn canonical_version_one_single_row_wire_preserves_original_source_order() {
    let bytes = one().encode_sources(limits(), &mut meter()).unwrap();
    let mut expected = b"arcweft.runtime.type-sources.v1\0".to_vec();
    expected.extend_from_slice(&[1, 1, 0, 0, 0]);
    expected.extend_from_slice(&0_u32.to_le_bytes());
    let mut semantic = [0; 32];
    semantic[0] = 1;
    expected.extend_from_slice(&semantic);
    expected.extend_from_slice(&[0, 5, 0, 0, 0, 1, 0, 0, 0, 0, 2, 0]);
    assert_eq!(bytes, expected);
}

#[test]
fn empty_original_table_needs_no_decoded_storage_or_synthetic_type() {
    let original = RuntimePlanTypeTableBuilder::new().finish().unwrap();
    let exact = RuntimeTypeSourceLimits {
        rows: 0,
        elements: 0,
        decoded_bytes: 0,
        ..limits()
    };
    let bytes = original.encode_sources(exact, &mut meter()).unwrap();
    let mut bank = meter();
    let prepared = RuntimePlanTypeTable::preflight_sources(&bytes, exact, &mut bank).unwrap();
    assert_eq!(prepared.storage_bytes(), 0);
    assert_eq!(prepared.elements(), 0);
    let decoded = prepared.materialize(&mut bank).unwrap();
    assert!(decoded.is_empty());
    assert_eq!(decoded, original);
}

#[test]
fn invalid_dense_parts_report_both_original_source_roles() {
    let original = table(vec![RuntimePlanTypeSeed::new(
        identity(1),
        P::DenseNumeric(crate::math::RuntimeDenseNumericType::ALL[0]),
    )]);
    let bytes = original.encode_sources(limits(), &mut meter()).unwrap();
    let parts_offset = DOMAIN.len() + 5 + 46 + 1;
    for parts in [[2, 0], [0, 255], [255, 255]] {
        let mut corrupt = bytes.clone();
        corrupt[parts_offset..parts_offset + 2].copy_from_slice(&parts);
        assert_eq!(
            RuntimePlanTypeTable::decode_sources(&corrupt, limits(), &mut meter()),
            Err(RuntimeTypeSourceError::DenseNumeric { parts })
        );
    }
}

#[test]
fn prepared_table_appends_to_parent_without_a_temporary_frame_allocation() {
    let original = full_table();
    let bytes = original.encode_sources(limits(), &mut meter()).unwrap();
    let prepared = original
        .prepare_sources_encoding(limits(), &mut meter())
        .unwrap();
    let mut bank = meter();
    let source = RuntimePlanTypeTable::preflight_sources(&bytes, limits(), &mut bank).unwrap();
    assert_eq!(prepared.storage_bytes(), source.storage_bytes());
    assert_eq!(prepared.elements(), source.elements());
    let mut output = Vec::with_capacity(bytes.len() + 3);
    output.extend_from_slice(&[5, 6, 7]);
    let pointer = output.as_ptr();
    let capacity = output.capacity();
    prepared.append_to(&mut output).unwrap();
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(output.capacity(), capacity);
    assert_eq!(&output[..3], [5, 6, 7]);
    assert_eq!(&output[3..], bytes);
    let mut absent = Vec::new();
    assert_eq!(
        original
            .prepare_sources_encoding(limits(), &mut meter())
            .unwrap()
            .append_to(&mut absent),
        Err(RuntimeTypeSourceError::OutputCapacity {
            required: bytes.len(),
            remaining: 0
        })
    );
    assert_eq!(absent.capacity(), 0);
}

#[test]
fn all_table_frames_can_preflight_before_retention_and_failure_poisoning_is_atomic() {
    let original = full_table();
    let bytes = original.encode_sources(limits(), &mut meter()).unwrap();
    let leaf = one();
    let leaf_bytes = leaf.encode_sources(limits(), &mut meter()).unwrap();
    let work = 3 * u64::try_from(bytes.len() + leaf_bytes.len()).unwrap();
    let mut bank = TaskSemanticMeter::new(work, u64::MAX);
    let first = RuntimePlanTypeTable::preflight_sources(&bytes, limits(), &mut bank).unwrap();
    let second = RuntimePlanTypeTable::preflight_sources(&leaf_bytes, limits(), &mut bank).unwrap();
    assert!(
        first
            .storage_bytes()
            .checked_add(second.storage_bytes())
            .is_some()
    );
    assert_eq!(first.materialize(&mut bank).unwrap(), original);
    assert_eq!(second.materialize(&mut bank).unwrap(), leaf);
    assert_eq!(
        bank.charge_work(1),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    let mut corrupt = leaf_bytes;
    corrupt[0] ^= 1;
    let mut bank = meter();
    let first = RuntimePlanTypeTable::preflight_sources(&bytes, limits(), &mut bank).unwrap();
    assert!(matches!(
        RuntimePlanTypeTable::preflight_sources(&corrupt, limits(), &mut bank),
        Err(RuntimeTypeSourceError::Domain)
    ));
    assert_eq!(
        first.materialize(&mut bank),
        Err(RuntimeTypeSourceError::Resource(
            TaskSemanticEncodingError::OwnerRejected
        ))
    );
}

#[test]
fn all_truncations_and_noncanonical_header_order_tags_fail() {
    let bytes = one().encode_sources(limits(), &mut meter()).unwrap();
    for cut in 0..bytes.len() {
        assert!(
            RuntimePlanTypeTable::decode_sources(&bytes[..cut], limits(), &mut meter()).is_err(),
            "cut {cut}"
        );
    }
    let row = DOMAIN.len() + 5;
    let projection = row + 46;
    for offset in [0, DOMAIN.len(), row, row + 36, projection, projection + 1] {
        let mut corrupt = bytes.clone();
        corrupt[offset] = 255;
        assert!(
            RuntimePlanTypeTable::decode_sources(&corrupt, limits(), &mut meter()).is_err(),
            "offset {offset}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        RuntimePlanTypeTable::decode_sources(&trailing, limits(), &mut meter()),
        Err(RuntimeTypeSourceError::Trailing)
    );
}

#[test]
fn zero_or_upper_references_and_cycles_cannot_become_admitted_type_tables() {
    let original = table(vec![
        RuntimePlanTypeSeed::new(
            identity(1),
            P::Sequence {
                kind: RuntimePlanSequenceKind::Vec,
                item: identity(2),
            },
        ),
        RuntimePlanTypeSeed::new(identity(2), P::Unit),
    ]);
    let bytes = original.encode_sources(limits(), &mut meter()).unwrap();
    let reference = DOMAIN.len() + 5 + 46 + 2;
    for id in [0, 3, u32::MAX] {
        let mut corrupt = bytes.clone();
        corrupt[reference..reference + 4].copy_from_slice(&id.to_le_bytes());
        assert!(matches!(
            RuntimePlanTypeTable::decode_sources(&corrupt, limits(), &mut meter()),
            Err(RuntimeTypeSourceError::Reference { .. })
        ));
    }
    let mut cycle = bytes;
    cycle[reference..reference + 4].copy_from_slice(&1_u32.to_le_bytes());
    assert!(
        matches!(RuntimePlanTypeTable::decode_sources(&cycle, limits(), &mut meter()), Err(RuntimeTypeSourceError::Admission(error)) if matches!(*error, RuntimePlanTypeTableError::Cycle { .. }))
    );
}

#[test]
fn duplicate_semantic_rows_and_foreign_child_scope_fail_original_owners() {
    let leaves = table(vec![
        RuntimePlanTypeSeed::new(identity(1), P::Unit),
        RuntimePlanTypeSeed::new(identity(2), P::Bool),
    ]);
    let mut bytes = leaves.encode_sources(limits(), &mut meter()).unwrap();
    let first = DOMAIN.len() + 5 + 4;
    bytes.copy_within(first..first + 32, first + 48);
    assert!(
        matches!(RuntimePlanTypeTable::decode_sources(&bytes, limits(), &mut meter()), Err(RuntimeTypeSourceError::Admission(error)) if matches!(*error, RuntimePlanTypeTableError::ConflictingProjection { .. }))
    );
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let child = RuntimeTypeScope::root().enter(binder).unwrap();
    let original = table(vec![
        RuntimePlanTypeSeed::new(identity(1), P::BoundType(child.bound_type(0, 0).unwrap()))
            .with_scope(child),
        RuntimePlanTypeSeed::new(
            identity(2),
            P::Function {
                contract: RuntimeFunctionTypeContract::new(
                    binder,
                    EffectPredicate::unconstrained(),
                    EffectFormula::literal(EffectSet::default(), None),
                ),
                parameters: vec![identity(1)].into_boxed_slice(),
                result: identity(1),
            },
        ),
    ]);
    let mut bytes = original.encode_sources(limits(), &mut meter()).unwrap();
    let effects = DOMAIN.len() + 5 + 41 + 5 + 4;
    bytes[effects..effects + 4].copy_from_slice(&1_u32.to_le_bytes());
    assert!(
        matches!(RuntimePlanTypeTable::decode_sources(&bytes, limits(), &mut meter()), Err(RuntimeTypeSourceError::Admission(error)) if matches!(*error, RuntimePlanTypeTableError::ChildScope { .. }))
    );
}

#[test]
fn exact_aggregate_storage_elements_and_original_work_bank_cover_all_rows() {
    let original = full_table();
    let mut bank = meter();
    let mut counter = Writer::new(None, Some(&mut bank), limits(), original.len());
    counter.table(&original).unwrap();
    let exact = RuntimeTypeSourceLimits {
        rows: u32::try_from(original.len()).unwrap(),
        elements: u32::try_from(counter.counts.elements).unwrap(),
        encoded_bytes: counter.length,
        decoded_bytes: counter.counts.storage,
        ..limits()
    };
    let bytes = original.encode_sources(exact, &mut meter()).unwrap();
    assert_eq!(
        RuntimePlanTypeTable::decode_sources(&bytes, exact, &mut meter()).unwrap(),
        original
    );
    for short in [
        RuntimeTypeSourceLimits {
            rows: exact.rows - 1,
            ..exact
        },
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
        assert!(original.encode_sources(short, &mut meter()).is_err());
        assert!(RuntimePlanTypeTable::decode_sources(&bytes, short, &mut meter()).is_err());
    }
    let work = 3 * u64::try_from(bytes.len()).unwrap();
    for decoding in [false, true] {
        let mut bank = TaskSemanticMeter::new(work + 1, u64::MAX);
        bank.charge_work(1).unwrap();
        if decoding {
            RuntimePlanTypeTable::decode_sources(&bytes, exact, &mut bank).unwrap();
        } else {
            original.encode_sources(exact, &mut bank).unwrap();
        }
        assert_eq!(
            bank.charge_work(1),
            Err(TaskSemanticEncodingError::SemanticWork)
        );
        let mut short = TaskSemanticMeter::new(work - 1, u64::MAX);
        let error = if decoding {
            RuntimePlanTypeTable::decode_sources(&bytes, exact, &mut short)
                .err()
                .unwrap()
        } else {
            original.encode_sources(exact, &mut short).err().unwrap()
        };
        assert!(matches!(
            error,
            RuntimeTypeSourceError::Resource(TaskSemanticEncodingError::SemanticWork)
                | RuntimeTypeSourceError::Scope(RuntimeTypeScopeCodecError::Resource(
                    TaskSemanticEncodingError::SemanticWork
                ))
                | RuntimeTypeSourceError::Function(RuntimeFunctionTypeSourceError::Resource(
                    TaskSemanticEncodingError::SemanticWork
                ))
        ));
        assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
        assert_eq!(
            RuntimePlanTypeTable::decode_sources(&[], exact, &mut short),
            Err(RuntimeTypeSourceError::Resource(
                TaskSemanticEncodingError::SemanticWork
            ))
        );
    }
}

#[test]
fn row_and_scope_binders_share_the_whole_frame_element_budget() {
    let scope =
        RuntimeTypeScope::try_from_binders(vec![RuntimeTypeBinder::new(1, 0, 0); 64]).unwrap();
    let original = table(vec![
        RuntimePlanTypeSeed::new(identity(1), P::BoundType(scope.bound_type(63, 0).unwrap()))
            .with_scope(scope),
    ]);
    let exact = RuntimeTypeSourceLimits {
        elements: 65,
        ..limits()
    };
    let bytes = original.encode_sources(exact, &mut meter()).unwrap();
    assert_eq!(
        RuntimePlanTypeTable::decode_sources(&bytes, exact, &mut meter()).unwrap(),
        original
    );
    let short = RuntimeTypeSourceLimits {
        elements: 64,
        ..exact
    };
    assert!(matches!(
        original.encode_sources(short, &mut meter()),
        Err(RuntimeTypeSourceError::Limit {
            kind: "source elements",
            ..
        })
    ));
    assert!(matches!(
        RuntimePlanTypeTable::decode_sources(&bytes, short, &mut meter()),
        Err(RuntimeTypeSourceError::Limit {
            kind: "source elements",
            ..
        })
    ));
}

fn policy_round_trip(policy: &RuntimeCodecUse) {
    let mut bank = meter();
    let mut counter = Writer::new(None, Some(&mut bank), limits(), 0);
    counter.policy(policy, 0).unwrap();
    let length = counter.length;
    let storage = counter.counts.storage;
    let mut bytes = Vec::with_capacity(length);
    Writer::new(Some(&mut bytes), None, limits(), 0)
        .policy(policy, 0)
        .unwrap();
    let mut bank = meter();
    let mut reader = Reader::new(&bytes, limits(), &mut bank);
    reader.policy(false, 0).unwrap();
    assert_eq!(reader.counts.storage, storage);
    let mut reader = Reader::new(&bytes, limits(), &mut bank);
    assert_eq!(&reader.policy(true, 0).unwrap().unwrap(), policy);
    assert_eq!(reader.offset, bytes.len());
}

#[test]
fn every_codec_policy_shape_and_attached_default_source_round_trips() {
    use RuntimeCodecUse as C;
    for format in RuntimeBytesFormat::ALL {
        policy_round_trip(&C::Bytes { format: *format });
    }
    for policy in [
        C::Plain,
        C::Unary {
            item: Box::new(C::Plain),
        },
        C::Newtype {
            inner: Box::new(C::Plain),
        },
        C::Tuple {
            items: vec![C::Plain].into_boxed_slice(),
        },
        C::Map {
            key: Box::new(C::Plain),
            value: Box::new(C::Plain),
        },
        C::RecordFields {
            fields: vec![C::Plain].into_boxed_slice(),
        },
        C::Builtin {
            payloads: vec![C::Plain].into_boxed_slice(),
        },
        C::Choice {
            alternatives: vec![C::Plain].into_boxed_slice(),
        },
        C::Opaque {
            arguments: vec![C::Plain].into_boxed_slice(),
        },
        C::NominalRef,
    ] {
        policy_round_trip(&policy);
    }
    policy_round_trip(&C::Record {
        name: "SourceRecord".to_owned(),
        deny_unknown_fields: true,
        fields: vec![RuntimeFieldCodecUse {
            wire_name: "payload".to_owned(),
            has_default: true,
            default_program: Some(
                arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([7; 32]),
            ),
            skip: true,
            bytes_format: Some(RuntimeBytesFormat::Hex),
            value: C::Plain,
        }]
        .into_boxed_slice(),
    });
    for repr in RuntimeEnumRepr::ALL {
        for tag in [
            RuntimeEnumTagStyle::External,
            RuntimeEnumTagStyle::Internal {
                tag: "kind".to_owned(),
            },
            RuntimeEnumTagStyle::Adjacent {
                tag: "kind".to_owned(),
                content: "value".to_owned(),
            },
        ] {
            policy_round_trip(&C::Enum {
                name: "SourceEnum".to_owned(),
                tag,
                repr: Some(*repr),
                cases: vec![
                    RuntimeVariantCodecUse {
                        wire_name: "first".to_owned(),
                        discriminant: Some(i128::MIN),
                        payload: None,
                    },
                    RuntimeVariantCodecUse {
                        wire_name: "second".to_owned(),
                        discriminant: Some(i128::MAX),
                        payload: Some(C::Plain),
                    },
                ]
                .into_boxed_slice(),
            });
        }
    }
}

#[test]
fn valid_declared_policy_is_retained_and_incompatible_policy_is_rejected() {
    let original = table(vec![
        RuntimePlanTypeSeed::new(identity(1), P::Bytes).with_data_codec(RuntimeCodecUse::Bytes {
            format: RuntimeBytesFormat::Hex,
        }),
        RuntimePlanTypeSeed::new(
            identity(2),
            P::Record(
                vec![RuntimePlanRecordField::new("original", identity(1))].into_boxed_slice(),
            ),
        )
        .with_data_codec(RuntimeCodecUse::Record {
            name: "OriginalRecord".to_owned(),
            deny_unknown_fields: true,
            fields: vec![RuntimeFieldCodecUse {
                wire_name: "payload".to_owned(),
                has_default: true,
                default_program: Some(
                    arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([7; 32]),
                ),
                skip: false,
                bytes_format: Some(RuntimeBytesFormat::Hex),
                value: RuntimeCodecUse::Bytes {
                    format: RuntimeBytesFormat::Hex,
                },
            }]
            .into_boxed_slice(),
        }),
    ]);
    let bytes = original.encode_sources(limits(), &mut meter()).unwrap();
    assert_eq!(
        RuntimePlanTypeTable::decode_sources(&bytes, limits(), &mut meter()).unwrap(),
        original
    );
    let mut wrong = bytes;
    let projection = DOMAIN.len() + 5 + 46;
    wrong[projection] = 2;
    assert!(
        matches!(RuntimePlanTypeTable::decode_sources(&wrong, limits(), &mut meter()), Err(RuntimeTypeSourceError::Admission(error)) if matches!(*error, RuntimePlanTypeTableError::CodecUse { .. }))
    );
}

#[test]
fn nominal_source_identity_is_preserved_and_invalid_identity_source_is_rejected() {
    let original = table(vec![
        RuntimePlanTypeSeed::new(
            identity(1),
            P::Nominal {
                nominal: RuntimeNominalTypeId::try_new("original::Nominal").unwrap(),
                layout: TypeLayoutHash::from_bytes([9; 32]),
                arguments: Box::new([]),
            },
        )
        .with_data_codec(RuntimeCodecUse::NominalRef),
    ]);
    let bytes = original.encode_sources(limits(), &mut meter()).unwrap();
    let row = DOMAIN.len() + 5;
    let mut declared = bytes.clone();
    declared[row + 36] = 1;
    declared.splice(row + 37..row + 37, [17; 32]);
    let decoded = RuntimePlanTypeTable::decode_sources(&declared, limits(), &mut meter()).unwrap();
    assert_eq!(
        decoded.declarations().next().unwrap().nominal_declaration(),
        Some(RuntimeNominalDeclarationId::from_bytes([17; 32]))
    );
    assert_eq!(
        decoded.encode_sources(limits(), &mut meter()).unwrap(),
        declared
    );
    let string = row + 46 + 1 + 4;
    for invalid in [0, 255] {
        let mut corrupt = bytes.clone();
        corrupt[string] = invalid;
        assert!(matches!(
            RuntimePlanTypeTable::decode_sources(&corrupt, limits(), &mut meter()),
            Err(RuntimeTypeSourceError::Identity(_) | RuntimeTypeSourceError::Utf8)
        ));
    }
}

#[test]
fn nested_policy_depth_and_late_malformed_policy_fail_without_retention() {
    let mut policy = RuntimeCodecUse::Plain;
    for _ in 0..4 {
        policy = RuntimeCodecUse::Unary {
            item: Box::new(policy),
        };
    }
    let mut bank = meter();
    let mut writer = Writer::new(None, Some(&mut bank), limits(), 0);
    writer.policy(&policy, 0).unwrap();
    let length = writer.length;
    let mut bytes = Vec::with_capacity(length);
    Writer::new(Some(&mut bytes), None, limits(), 0)
        .policy(&policy, 0)
        .unwrap();
    let short = RuntimeTypeSourceLimits {
        policy_depth: 3,
        ..limits()
    };
    assert!(
        Writer::new(None, Some(&mut meter()), short, 0)
            .policy(&policy, 0)
            .is_err()
    );
    assert!(
        Reader::new(&bytes, short, &mut meter())
            .policy(false, 0)
            .is_err()
    );
    *bytes.last_mut().unwrap() = 255;
    assert!(matches!(
        Reader::new(&bytes, limits(), &mut meter()).policy(false, 0),
        Err(RuntimeTypeSourceError::Tag {
            kind: "codec policy",
            tag: 255
        })
    ));
}
