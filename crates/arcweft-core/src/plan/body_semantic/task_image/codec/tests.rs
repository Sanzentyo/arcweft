use super::*;
use crate::plan::{
    RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealError, RuntimeTaskPlanSealLimits,
};

mod source_construction;
mod source_declarations;

fn declaration_encoding(
    plan: &RuntimePlan,
) -> (
    crate::plan::type_table::source_codec::PreparedRuntimeTypeTableEncoding<'_>,
    crate::plan::type_table::source_codec::domains::PreparedRuntimeNominalEncoding<'_>,
    crate::plan::local_declarations::source_codec::PreparedRuntimeLocalEncoding<'_>,
    crate::plan::control_effect::source_codec::PreparedRuntimeControlEncoding<'_>,
) {
    let limits = limits();
    let mut meter = TaskSemanticMeter::new(1_000_000, 1_000_000);
    let types = plan
        .type_table()
        .prepare_sources_encoding(
            crate::plan::type_table::source_codec::RuntimeTypeSourceLimits {
                rows: u32::try_from(plan.type_table().len()).unwrap(),
                elements: limits.program.elements,
                encoded_bytes: limits.encoded_bytes,
                decoded_bytes: limits.decoded_bytes,
                policy_depth: limits.program.policy_depth,
            },
            &mut meter,
        )
        .unwrap();
    let nominals = plan
        .inventory
        .prepare_nominal_sources_encoding(
            crate::plan::type_table::source_codec::RuntimeTypeSourceLimits {
                rows: u32::try_from(
                    plan.nominal_record_domains().len() + plan.variant_domains().len(),
                )
                .unwrap(),
                elements: limits.program.elements,
                encoded_bytes: limits.encoded_bytes,
                decoded_bytes: limits.decoded_bytes,
                policy_depth: limits.program.policy_depth,
            },
            &mut meter,
        )
        .unwrap();
    let locals = plan
        .local_declarations()
        .prepare_sources_encoding(
            crate::plan::local_declarations::source_codec::RuntimeLocalSourceLimits {
                rows: u32::try_from(plan.local_declarations().len()).unwrap(),
                encoded_bytes: limits.encoded_bytes,
                decoded_bytes: limits.decoded_bytes,
                type_count: plan.type_table().len(),
            },
            &mut meter,
        )
        .unwrap();
    let controls = plan
        .control_effect_contracts()
        .prepare_sources_encoding(
            crate::plan::control_effect::source_codec::RuntimeControlSourceLimits {
                rows: u32::try_from(plan.control_effect_contracts().len()).unwrap(),
                elements: limits.program.elements as usize,
                encoded_bytes: limits.encoded_bytes,
                decoded_bytes: limits.decoded_bytes,
                type_count: plan.type_table().len(),
            },
            &mut meter,
        )
        .unwrap();
    (types, nominals, locals, controls)
}

fn declaration_frames(plan: &RuntimePlan) -> [Vec<u8>; 4] {
    let (types, nominals, locals, controls) = declaration_encoding(plan);
    let mut type_bytes = Vec::with_capacity(types.encoded_bytes());
    types.append_to(&mut type_bytes).unwrap();
    let mut nominal_bytes = Vec::with_capacity(nominals.encoded_bytes());
    nominals.append_to(&mut nominal_bytes).unwrap();
    let mut local_bytes = Vec::with_capacity(locals.encoded_bytes());
    locals.append_to(&mut local_bytes).unwrap();
    let mut control_bytes = Vec::with_capacity(controls.encoded_bytes());
    controls.append_to(&mut control_bytes).unwrap();
    [type_bytes, nominal_bytes, local_bytes, control_bytes]
}

fn task_count_offset(input: &[u8]) -> usize {
    let length_offset = DOMAIN.len() + 1;
    let length =
        u32::from_le_bytes(input[length_offset..length_offset + 4].try_into().unwrap()) as usize;
    length_offset + 4 + length
}

fn resources() -> ValueResourceAdmission {
    ValueResourceAdmission::live(crate::entry::schema::RuntimeSchemaLimits::engine_default())
}

fn decode_image(
    input: &[u8],
    limits: RuntimeTaskSourceCodecLimits,
    meter: TaskSemanticMeter,
) -> Result<UnsealedRuntimePlanImage, RuntimeTaskSourceCodecError> {
    UnsealedRuntimePlanImage::decode_task_sources(
        input,
        limits,
        RuntimeTaskPlanSealLimits::default(),
        meter,
        &mut resources(),
    )
}

fn limits() -> RuntimeTaskSourceCodecLimits {
    RuntimeTaskSourceCodecLimits {
        rows: 4,
        encoded_bytes: 100_000,
        decoded_bytes: 100_000,
        request_roles: 32,
        path_steps: 32,
        program: RuntimeProgramSourceLimits {
            rows: 100_000,
            nodes: 100_000,
            elements: 100_000,
            depth: 128,
            policy_depth: 32,
            encoded_bytes: 100_000,
            decoded_bytes: 100_000,
            opaque_producer_bytes: 65_536,
        },
    }
}

fn candidate(rows: u32) -> UnsealedRuntimePlanImage {
    super::super::preparation::tests::image_with_rows(rows, None)
}

fn published() -> (RuntimePlan, RuntimeTaskPlanBuildCoordinate) {
    let image = candidate(1);
    let coordinate = image.coordinate_owner.resolve(0).unwrap();
    let plan = image
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    (plan, coordinate)
}

fn bytes(plan: &RuntimePlan) -> Vec<u8> {
    plan.encode_task_sources(
        limits(),
        &resources(),
        &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
    )
    .unwrap()
}

fn decode(
    input: &[u8],
    rows: u32,
    limits: RuntimeTaskSourceCodecLimits,
    meter: TaskSemanticMeter,
) -> (
    Result<UnsealedRuntimePlanImage, RuntimeTaskSourceCodecError>,
    Option<RuntimeTaskPlanBuildCoordinate>,
) {
    let decoded = UnsealedRuntimePlanImage::decode_task_sources(
        input,
        limits,
        RuntimeTaskPlanSealLimits::default(),
        meter,
        &mut resources(),
    );
    let coordinate = decoded.as_ref().ok().and_then(|image| {
        assert_eq!(image.task_plans.len(), rows as usize);
        image.coordinate_owner.resolve(0)
    });
    (decoded, coordinate)
}

#[test]
fn task_source_codec_recomputes_the_original_key_in_a_fresh_namespace() {
    let (original, original_coordinate) = published();
    let encoded = bytes(&original);
    let (decoded, coordinate) = decode(
        &encoded,
        1,
        limits(),
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    );
    let decoded = decoded.unwrap();
    let coordinate = coordinate.unwrap();
    assert!(coordinate.sealed_plan().is_none());
    let expected = original.task_plan(&original_coordinate).unwrap().key();
    assert_eq!(
        decoded.expected_key(0).unwrap().bytes(),
        expected.as_bytes()
    );
    let sealed = decoded
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    assert_eq!(sealed.type_table(), original.type_table());
    assert_eq!(sealed.local_declarations(), original.local_declarations());
    assert_eq!(
        sealed.control_effect_contracts(),
        original.control_effect_contracts()
    );
    assert_eq!(sealed.task_plan(&coordinate).unwrap().key(), expected);
    assert!(sealed.task_plan(&original_coordinate).is_none());
    assert!(original.task_plan(&coordinate).is_none());
    assert_eq!(bytes(&sealed), encoded);
}

#[test]
fn prepared_task_source_borrows_wire_and_defers_identity_publication() {
    let (original, original_coordinate) = published();
    let expected = original.task_plan(&original_coordinate).unwrap().key();
    for valid_assertion in [true, false] {
        let mut encoded = bytes(&original);
        if !valid_assertion {
            let key_start = encoded.len() - 32;
            encoded[key_start..].fill(0);
        }
        let mut resources = resources();
        let prepared = PreparedRuntimeTaskSource::preflight(
            &encoded,
            limits(),
            RuntimeTaskPlanSealLimits::default(),
            TaskSemanticMeter::new(1_000_000, 1_000_000),
            &mut resources,
        )
        .unwrap();
        assert_eq!(prepared.bytes(), encoded);
        assert_eq!(prepared.bytes().as_ptr(), encoded.as_ptr());
        assert_eq!(prepared.task_count, 1);
        let image = prepared.materialize().unwrap();
        let coordinate = image.coordinate_owner.resolve(0).unwrap();
        assert!(coordinate.sealed_plan().is_none());
        assert!(original.task_plan(&coordinate).is_none());
        let sealed = image.seal(RuntimeTaskPlanSealLimits::default(), None);
        if valid_assertion {
            let restored = sealed.unwrap().into_plan();
            assert_eq!(restored.task_plan(&coordinate).unwrap().key(), expected);
            assert!(restored.task_plan(&original_coordinate).is_none());
        } else {
            assert!(matches!(
                sealed.err().unwrap(),
                RuntimeTaskPlanSealError::ExpectedKeyMismatch { ordinal: 0 }
            ));
            assert!(coordinate.sealed_plan().is_none());
        }
        assert_eq!(
            original.task_plan(&original_coordinate).unwrap().key(),
            expected
        );
    }
}

#[test]
fn prepared_task_source_carries_original_work_through_materialization() {
    let (original, _) = published();
    let encoded = bytes(&original);
    let mut value_resources = resources();
    let measured = PreparedRuntimeTaskSource::preflight(
        &encoded,
        limits(),
        RuntimeTaskPlanSealLimits::default(),
        TaskSemanticMeter::new(1_000_000, 1_000_000),
        &mut value_resources,
    )
    .unwrap();
    let preflight_work = measured.meter.totals().0;
    let measured = measured.materialize().unwrap();
    let complete_work = measured.preparation_meter.as_ref().unwrap().totals().0;
    assert!(complete_work > preflight_work);
    assert!(preflight_work >= 3 * encoded.len() as u64);

    for (maximum, prefix) in [(complete_work, 0), (complete_work + 1, 1)] {
        let mut meter = TaskSemanticMeter::new(maximum, 1_000_000);
        meter.charge_work(prefix).unwrap();
        let mut value_resources = resources();
        let prepared = PreparedRuntimeTaskSource::preflight(
            &encoded,
            limits(),
            RuntimeTaskPlanSealLimits::default(),
            meter,
            &mut value_resources,
        )
        .unwrap();
        assert_eq!(prepared.meter.totals().0, preflight_work + prefix);
        let image = prepared.materialize().unwrap();
        assert_eq!(
            image.preparation_meter.as_ref().unwrap().totals().0,
            complete_work + prefix
        );
        assert!(
            image
                .coordinate_owner
                .resolve(0)
                .unwrap()
                .sealed_plan()
                .is_none()
        );
    }

    for (maximum, prefix) in [(preflight_work, 0), (complete_work, 1)] {
        let mut meter = TaskSemanticMeter::new(maximum, 1_000_000);
        meter.charge_work(prefix).unwrap();
        let mut value_resources = resources();
        let prepared = PreparedRuntimeTaskSource::preflight(
            &encoded,
            limits(),
            RuntimeTaskPlanSealLimits::default(),
            meter,
            &mut value_resources,
        )
        .unwrap();
        assert_eq!(prepared.meter.totals().0, preflight_work + prefix);
        let error = prepared.materialize().err().unwrap();
        let cause = if prefix == 0 {
            let RuntimeTaskSourceCodecError::Program(program) = error else {
                panic!("{error:?}")
            };
            let RuntimeProgramSourceError::Declarations(
                crate::plan::declaration_sources::RuntimePlanDeclarationSourceError::Resource(
                    cause,
                ),
            ) = *program
            else {
                panic!("{program:?}")
            };
            cause
        } else {
            let RuntimeTaskSourceCodecError::Resource(cause) = error else {
                panic!("{error:?}")
            };
            cause
        };
        assert_eq!(cause, TaskSemanticEncodingError::SemanticWork);
    }
}

#[test]
fn task_source_frame_has_one_canonical_version_one_wire_shape() {
    let (original, coordinate) = published();
    let mut expected = b"arcweft.runtime.task-sources.v1\0".to_vec();
    expected.push(1);
    let program = original
        .encode_program_source(
            limits().program,
            &resources(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
    expected.extend_from_slice(&u32::try_from(program.bytes().len()).unwrap().to_le_bytes());
    expected.extend_from_slice(program.bytes());
    expected.extend_from_slice(&[
        1, 0, 0, 0, // source row count
        0, 0, 0, 0, // dense source ordinal
        1, 0, 0, 0, // original function coordinate
        7, 1, // HostAdapterTask, Io
        14, 0, 0, 0, // request frame length
        1, 1, // request version, operation source
        0, 0, 0, 0, // original operation ordinal
        0, 0, 0, 0, // argument count
        0, 0, 0, 0, // field count
        1, 0, 0, 0, // original control coordinate
        0, // Ordinary binding
    ]);
    expected.extend_from_slice(original.task_plan(&coordinate).unwrap().key().as_bytes());
    assert_eq!(bytes(&original), expected);
}

#[test]
fn task_source_codec_retains_zero_expected_bytes_without_issuing_a_key() {
    let (original, _) = published();
    let mut encoded = bytes(&original);
    let start = encoded.len() - 32;
    encoded[start..].fill(0);
    let (decoded, coordinate) = decode(
        &encoded,
        1,
        limits(),
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    );
    let decoded = decoded.unwrap();
    let coordinate = coordinate.unwrap();
    assert_eq!(decoded.expected_key(0).unwrap().bytes(), &[0; 32]);
    assert!(coordinate.sealed_plan().is_none());
    assert!(matches!(
        decoded
            .seal(RuntimeTaskPlanSealLimits::default(), None)
            .err()
            .unwrap(),
        RuntimeTaskPlanSealError::ExpectedKeyMismatch { ordinal: 0 }
    ));
    assert!(coordinate.sealed_plan().is_none());
}

#[test]
fn task_source_codec_rejects_noncanonical_frames_before_publication() {
    let (original, _) = published();
    let encoded = bytes(&original);
    let count = task_count_offset(&encoded);
    let row = count + 4;
    let request_length =
        u32::from_le_bytes(encoded[row + 10..row + 14].try_into().unwrap()) as usize;
    let control = row + 14 + request_length;
    for case in 0..15 {
        let mut changed = encoded.clone();
        match case {
            0 => changed[0] ^= 1,
            1 => changed[DOMAIN.len()] = 2,
            2 => changed[row..row + 4].copy_from_slice(&1_u32.to_le_bytes()),
            3 => changed[row + 4..row + 8].fill(0),
            4 => changed[row + 8] = u8::MAX,
            5 => changed[row + 9] = u8::MAX,
            6 => changed[row + 10..row + 14].copy_from_slice(&u32::MAX.to_le_bytes()),
            7 => changed[row + 14] = 2,
            8 => changed[control..control + 4].fill(0),
            9 => changed[control + 4] = 1,
            10 => {
                changed.pop();
            }
            11 => changed.push(0),
            12 => changed[count..row].fill(0),
            13 => changed[row + 4..row + 8].copy_from_slice(&u32::MAX.to_le_bytes()),
            14 => changed[control..control + 4].copy_from_slice(&u32::MAX.to_le_bytes()),
            _ => unreachable!(),
        }
        let (decoded, coordinate) = decode(
            &changed,
            1,
            limits(),
            TaskSemanticMeter::new(1_000_000, 1_000_000),
        );
        assert!(decoded.is_err(), "case {case}");
        assert!(coordinate.is_none(), "case {case}");
    }
}

#[test]
fn task_source_codec_checks_aggregate_row_storage_before_materialization() {
    let (original, _) = published();
    let encoded = bytes(&original);
    let count = task_count_offset(&encoded);
    let row = count + 4;
    let mut doubled = encoded.clone();
    doubled[count..row].copy_from_slice(&2_u32.to_le_bytes());
    let mut second = encoded[row..].to_vec();
    second[..4].copy_from_slice(&1_u32.to_le_bytes());
    doubled.extend(second);
    let program_start = DOMAIN.len() + 1 + 4;
    let task_namespace = program_start + b"arcweft.runtime.program-source.v1\0".len() + 1 + 12 * 4;
    doubled[task_namespace..task_namespace + 4].copy_from_slice(&2_u32.to_le_bytes());
    let mut meter = TaskSemanticMeter::new(1_000_000, 1_000_000);
    let program = PreparedRuntimeProgramSource::preflight(
        &doubled[program_start..count],
        limits().program,
        &resources(),
        &mut meter,
    )
    .unwrap()
    .materialize(meter, &mut resources())
    .unwrap();
    let required = program.footprint.storage
        + 2 * (size_of::<RuntimeTaskPlan>()
            + size_of::<RuntimeTaskPlanSeed>()
            + size_of::<ExpectedTaskPlanKey>());
    let exact = RuntimeTaskSourceCodecLimits {
        decoded_bytes: required,
        ..limits()
    };
    let (decoded, coordinate) = decode(
        &doubled,
        2,
        exact,
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    );
    assert_eq!(decoded.unwrap().task_plans.len(), 2);
    assert!(coordinate.unwrap().sealed_plan().is_none());
    let (short, coordinate) = decode(
        &doubled,
        2,
        RuntimeTaskSourceCodecLimits {
            decoded_bytes: required - 1,
            ..exact
        },
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    );
    let error = short.err().unwrap();
    use crate::plan::construction::source::{expression, flow, function_bodies};
    let RuntimeTaskSourceCodecError::Program(error) = error else {
        panic!("expected a program storage limit: {error:?}");
    };
    let RuntimeProgramSourceError::Bodies(function_bodies::RuntimeFunctionBodySourceError::Flow(
        error,
    )) = *error
    else {
        panic!("expected body storage admission: {error:?}");
    };
    let flow::RuntimeFlowSourceError::Expression(error) = *error else {
        panic!("expected expression storage admission: {error:?}");
    };
    assert!(
        matches!(*error,
        expression::RuntimeExpressionSourceError::Literal(crate::value::RuntimeLiteralSourceError::Limit {
            kind: "decoded bytes", actual, maximum,
        }) if actual == maximum + 1),
        "{error:?}"
    );
    assert!(coordinate.is_none());
}

#[test]
fn task_source_codec_shares_the_exact_work_bank_with_the_common_seal() {
    let (original, _) = published();
    let encoded = bytes(&original);
    let (baseline, _) = decode(
        &encoded,
        1,
        limits(),
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    );
    let charged = baseline
        .unwrap()
        .preparation_meter
        .as_ref()
        .unwrap()
        .totals()
        .0;
    assert!(charged > 3 * encoded.len() as u64);
    let mut meter = TaskSemanticMeter::new(charged + 1, 1_000_000);
    meter.charge_work(1).unwrap();
    let (decoded, coordinate) = decode(&encoded, 1, limits(), meter);
    let decoded = decoded.unwrap();
    let coordinate = coordinate.unwrap();
    assert_eq!(
        decoded.preparation_meter.as_ref().unwrap().totals().0,
        charged + 1
    );
    let error = decoded
        .seal(
            RuntimeTaskPlanSealLimits {
                max_semantic_work: charged + 1,
                ..Default::default()
            },
            None,
        )
        .err()
        .unwrap();
    assert!(matches!(
        error,
        RuntimeTaskPlanSealError::Control(
            crate::plan::RuntimeControlEffectContractError::WorkLimit
        ) | RuntimeTaskPlanSealError::Encoding(TaskSemanticEncodingError::SemanticWork)
            | RuntimeTaskPlanSealError::Body(crate::plan::RuntimeBodySemanticError::Encoding(
                TaskSemanticEncodingError::SemanticWork
            ))
    ));
    assert!(coordinate.sealed_plan().is_none());
    let mut short_meter = TaskSemanticMeter::new(charged, 1_000_000);
    short_meter.charge_work(1).unwrap();
    let (short, coordinate) = decode(&encoded, 1, limits(), short_meter);
    assert!(matches!(
        short.err().unwrap(),
        RuntimeTaskSourceCodecError::Resource(TaskSemanticEncodingError::SemanticWork)
    ));
    assert!(coordinate.is_none());
}

#[test]
fn task_source_encoder_has_exact_shared_work_and_reserved_output_bounds() {
    let (original, _) = published();
    let encoded = bytes(&original);
    let mut measured = TaskSemanticMeter::new(1_000_000, 1_000_000);
    assert_eq!(
        original
            .encode_task_sources(limits(), &resources(), &mut measured)
            .unwrap(),
        encoded
    );
    let charged = measured.totals().0;
    assert!(charged > 3 * encoded.len() as u64);
    let exact = RuntimeTaskSourceCodecLimits {
        encoded_bytes: encoded.len(),
        ..limits()
    };
    let mut meter = TaskSemanticMeter::new(charged, 1_000_000);
    assert_eq!(
        original
            .encode_task_sources(exact, &resources(), &mut meter)
            .unwrap(),
        encoded
    );
    assert_eq!(meter.totals().0, charged);
    let mut short = TaskSemanticMeter::new(charged - 1, 1_000_000);
    assert!(matches!(
        original
            .encode_task_sources(exact, &resources(), &mut short)
            .unwrap_err(),
        RuntimeTaskSourceCodecError::Resource(TaskSemanticEncodingError::SemanticWork)
    ));
    assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
    assert!(matches!(
        original
            .encode_task_sources(
                RuntimeTaskSourceCodecLimits { rows: 0, ..exact },
                &resources(),
                &mut short
            )
            .unwrap_err(),
        RuntimeTaskSourceCodecError::Resource(TaskSemanticEncodingError::SemanticWork)
    ));
}

#[test]
fn source_class_tags_are_the_same_closed_owner_grammar() {
    for tag in 0..13 {
        assert_eq!(
            TaskClass::from_semantic_tag(tag).unwrap().semantic_tag(),
            tag
        );
    }
    assert!(TaskClass::from_semantic_tag(13).is_none());
    assert!(TaskClass::from_semantic_tag(u8::MAX).is_none());
}

#[test]
fn complete_task_source_restores_original_host_request_roles_and_emits_native_completed_keys() {
    use crate::plan::construction::source::host_calls::tests::{fixture, launch};
    use crate::step::RuntimeHostCallMode;
    for mode in [RuntimeHostCallMode::Immediate, RuntimeHostCallMode::Suspend] {
        for contract in [false, true] {
            for binding in [false, true] {
                for empty in [false, true] {
                    let original = fixture(mode, contract, binding, empty);
                    let encoded = bytes(&original);
                    let decoded = decode_image(
                        &encoded,
                        limits(),
                        TaskSemanticMeter::new(1_000_000, 1_000_000),
                    )
                    .unwrap();
                    let coordinate = decoded.coordinate_owner.resolve(0).unwrap();
                    assert!(coordinate.sealed_plan().is_none());
                    let expected = original
                        .task_plans()
                        .by_index(RuntimeTaskPlanIndex(0))
                        .unwrap();
                    let restored = decoded
                        .seal(RuntimeTaskPlanSealLimits::default(), None)
                        .unwrap()
                        .into_plan();
                    let actual = restored.task_plan(&coordinate).unwrap();
                    assert_eq!(actual.key(), expected.key());
                    assert_eq!(
                        actual.definition().request_template(),
                        expected.definition().request_template()
                    );
                    assert_eq!(
                        restored.executable_semantic_digest(),
                        original.executable_semantic_digest()
                    );
                    assert_eq!(bytes(&restored), encoded);
                    if !empty {
                        let rejected = UnsealedRuntimePlanImage::decode_task_sources(
                            &encoded,
                            limits(),
                            RuntimeTaskPlanSealLimits {
                                max_request_roles: 0,
                                ..Default::default()
                            },
                            TaskSemanticMeter::new(1_000_000, 1_000_000),
                            &mut resources(),
                        );
                        assert!(
                            matches!(rejected.err().unwrap(), RuntimeTaskSourceCodecError::Request {
                            source: RuntimeTaskRequestCodecError::Limit { kind: "roles", actual, maximum: 0 }, ..
                        } if actual > 0)
                        );
                    }
                    let (_engine, call) = launch(restored);
                    assert_eq!(call.submission().spec().producer.plan(), expected.key());
                }
            }
        }
    }
}

#[test]
fn task_source_applies_common_seal_budget_and_shape_caps_before_reconstruction() {
    let (original, _) = published();
    let encoded = bytes(&original);
    for malformed in [false, true] {
        let mut input = encoded.clone();
        if malformed {
            input[0] ^= 1;
        }
        let error = UnsealedRuntimePlanImage::decode_task_sources(
            &input,
            limits(),
            RuntimeTaskPlanSealLimits {
                max_semantic_work: 0,
                ..Default::default()
            },
            TaskSemanticMeter::new(u64::MAX, u64::MAX),
            &mut resources(),
        )
        .err()
        .unwrap();
        assert!(matches!(
            error,
            RuntimeTaskSourceCodecError::Resource(TaskSemanticEncodingError::SemanticWork)
        ));
    }
    for (seal, kind) in [
        (
            RuntimeTaskPlanSealLimits {
                max_task_plan_rows: 0,
                ..Default::default()
            },
            "rows",
        ),
        (
            RuntimeTaskPlanSealLimits {
                max_control_effect_rows: 0,
                ..Default::default()
            },
            "control rows",
        ),
    ] {
        let error = UnsealedRuntimePlanImage::decode_task_sources(
            &encoded,
            limits(),
            seal,
            TaskSemanticMeter::new(u64::MAX, u64::MAX),
            &mut resources(),
        )
        .err()
        .unwrap();
        assert!(
            matches!(error, RuntimeTaskSourceCodecError::Limit { kind: actual_kind, actual: 1, maximum: 0 }
            if actual_kind == kind)
        );
    }
    let mut view = encoded.clone();
    let row = task_count_offset(&view) + 4;
    let request = u32::from_le_bytes(view[row + 10..row + 14].try_into().unwrap()) as usize;
    view[row + 8] = NeedProducerFamily::ViewMatchSubscription.semantic_tag();
    view[row + 14 + request + 4] = 1;
    let error = UnsealedRuntimePlanImage::decode_task_sources(
        &view,
        limits(),
        RuntimeTaskPlanSealLimits {
            max_view_bindings: 0,
            ..Default::default()
        },
        TaskSemanticMeter::new(u64::MAX, u64::MAX),
        &mut resources(),
    )
    .err()
    .unwrap();
    assert!(matches!(
        error,
        RuntimeTaskSourceCodecError::Limit {
            kind: "View rows",
            actual: 1,
            maximum: 0
        }
    ));
}
