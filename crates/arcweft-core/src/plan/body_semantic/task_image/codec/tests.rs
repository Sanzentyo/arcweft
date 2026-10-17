use super::*;
use crate::plan::{
    RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanSealError, RuntimeTaskPlanSealLimits,
};

fn limits() -> RuntimeTaskSourceCodecLimits {
    RuntimeTaskSourceCodecLimits {
        rows: 4,
        encoded_bytes: 100_000,
        decoded_bytes: 100_000,
        request_roles: 32,
        path_steps: 32,
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
    plan.encode_task_sources(limits(), &mut TaskSemanticMeter::new(1_000_000, 1_000_000))
        .unwrap()
}

fn decode(
    input: &[u8],
    rows: u32,
    limits: RuntimeTaskSourceCodecLimits,
    meter: TaskSemanticMeter,
) -> (
    Result<UnsealedRuntimePlanImage, RuntimeTaskSourceCodecError>,
    RuntimeTaskPlanBuildCoordinate,
) {
    let image = candidate(rows);
    let coordinate = image.coordinate_owner.resolve(0).unwrap();
    let decoded = UnsealedRuntimePlanImage::decode_task_sources(
        image.inventory,
        image.coordinate_owner,
        input,
        limits,
        meter,
    );
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
    assert_eq!(sealed.task_plan(&coordinate).unwrap().key(), expected);
    assert!(sealed.task_plan(&original_coordinate).is_none());
    assert!(original.task_plan(&coordinate).is_none());
    assert_eq!(bytes(&sealed), encoded);
}

#[test]
fn task_source_frame_has_one_canonical_version_one_wire_shape() {
    let (original, coordinate) = published();
    let mut expected = b"arcweft.runtime.task-sources.v1\0".to_vec();
    expected.extend_from_slice(&[
        1, // version
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
    let row = DOMAIN.len() + 1 + 4;
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
            12 => changed[DOMAIN.len() + 1..row].fill(0),
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
        assert!(coordinate.sealed_plan().is_none(), "case {case}");
    }
}

#[test]
fn task_source_codec_checks_aggregate_row_storage_before_materialization() {
    let (original, _) = published();
    let encoded = bytes(&original);
    let row = DOMAIN.len() + 1 + 4;
    let mut doubled = encoded.clone();
    doubled[DOMAIN.len() + 1..row].copy_from_slice(&2_u32.to_le_bytes());
    let mut second = encoded[row..].to_vec();
    second[..4].copy_from_slice(&1_u32.to_le_bytes());
    doubled.extend(second);
    let required =
        2 * (std::mem::size_of::<RuntimeTaskPlan>() + std::mem::size_of::<ExpectedTaskPlanKey>());
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
    assert!(coordinate.sealed_plan().is_none());
    let (short, coordinate) = decode(
        &doubled,
        2,
        RuntimeTaskSourceCodecLimits {
            decoded_bytes: required - 1,
            ..exact
        },
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    );
    assert!(matches!(
        short.err().unwrap(),
        RuntimeTaskSourceCodecError::Limit {
            kind: "decoded bytes",
            ..
        }
    ));
    assert!(coordinate.sealed_plan().is_none());
}

#[test]
fn task_source_codec_shares_the_exact_work_bank_with_the_common_seal() {
    let (original, _) = published();
    let encoded = bytes(&original);
    let charged = 3 * encoded.len() as u64;
    let mut meter = TaskSemanticMeter::new(charged + 1, 1_000_000);
    meter.charge_work(1).unwrap();
    let (decoded, coordinate) = decode(&encoded, 1, limits(), meter);
    let decoded = decoded.unwrap();
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
    let (short, coordinate) = decode(
        &encoded,
        1,
        limits(),
        TaskSemanticMeter::new(charged - 1, 1_000_000),
    );
    assert_eq!(
        short.err().unwrap(),
        RuntimeTaskSourceCodecError::Resource(TaskSemanticEncodingError::SemanticWork)
    );
    assert!(coordinate.sealed_plan().is_none());
}

#[test]
fn task_source_encoder_has_exact_shared_work_and_reserved_output_bounds() {
    let (original, _) = published();
    let encoded = bytes(&original);
    let charged = 3 * encoded.len() as u64;
    let exact = RuntimeTaskSourceCodecLimits {
        encoded_bytes: encoded.len(),
        ..limits()
    };
    let mut meter = TaskSemanticMeter::new(charged, 1_000_000);
    assert_eq!(
        original.encode_task_sources(exact, &mut meter).unwrap(),
        encoded
    );
    assert_eq!(meter.totals().0, charged);
    let mut short = TaskSemanticMeter::new(charged - 1, 1_000_000);
    assert_eq!(
        original.encode_task_sources(exact, &mut short).unwrap_err(),
        RuntimeTaskSourceCodecError::Resource(TaskSemanticEncodingError::SemanticWork)
    );
    assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
    assert_eq!(
        original
            .encode_task_sources(
                RuntimeTaskSourceCodecLimits { rows: 0, ..exact },
                &mut short
            )
            .unwrap_err(),
        RuntimeTaskSourceCodecError::Resource(TaskSemanticEncodingError::SemanticWork)
    );
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
