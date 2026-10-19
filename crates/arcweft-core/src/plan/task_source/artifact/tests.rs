use super::*;
use crate::awbc::codec::{AwbcCodecError, AwbcDecodeBudget};
use crate::awbc::schema::AwbcProgram;
use crate::plan::RuntimeTaskPlanIndex;
use crate::plan::construction::source::selected_producers;
use crate::task::semantic::TaskSemanticEncodingError;
use crate::task::{HostRestartPolicy, TaskClass, TaskPolicy};

fn original(request: u8) -> RuntimePlan {
    selected_producers::tests::fixture(
        request,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::MustBeQuiescent,
        TaskClass::Io,
    )
}

fn candidate(source: RuntimeTaskSourceArtifact) -> AwbcProgram {
    AwbcProgram {
        original_source: Some(source),
        ..AwbcProgram::default()
    }
}

#[test]
fn canonical_awbc_carries_and_reseals_the_complete_original_source() {
    let limits = RuntimeTaskSourceArtifactLimits::default();
    for request in 0..3 {
        let original = original(request);
        let source = original.task_source_artifact(limits).unwrap();
        let retained = source.clone();
        assert_eq!(retained.bytes().as_ptr(), source.bytes().as_ptr());
        assert_eq!(
            source.expected_executable(),
            original.executable_semantic_digest().as_bytes()
        );
        let program = candidate(source);
        let encoded = program.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
        assert_eq!(decoded, program);
        assert_eq!(decoded.encode_canonical().unwrap(), encoded);
        let source = decoded.original_source.unwrap();
        assert_eq!(source.bytes(), retained.bytes());
        let admitted = source.admit(limits, None).unwrap();
        assert_eq!(admitted.type_table(), original.type_table());
        assert_eq!(admitted.task_source_artifact(limits).unwrap(), source);
        assert_eq!(
            admitted.executable_semantic_digest(),
            original.executable_semantic_digest()
        );
        let first = original
            .task_plans()
            .by_index(RuntimeTaskPlanIndex(0))
            .unwrap();
        let last = admitted
            .task_plans()
            .by_index(RuntimeTaskPlanIndex(0))
            .unwrap();
        assert_eq!(first.key(), last.key());
        assert!(!first.belongs_to(&admitted));
        assert!(!last.belongs_to(&original));
    }
}

#[test]
fn canonical_awbc_source_assertions_cannot_publish_forged_executable_or_task_keys() {
    let limits = RuntimeTaskSourceArtifactLimits::default();
    let original = original(2);
    let source = original.task_source_artifact(limits).unwrap();
    for forged_executable in [false, true] {
        let mut source = source.clone();
        if forged_executable {
            source =
                RuntimeTaskSourceArtifact::from_encoded_source([0; 32], source.bytes().to_vec());
        } else {
            let mut bytes = source.bytes().to_vec();
            let key_start = bytes.len() - 32;
            bytes[key_start..].fill(0);
            source = RuntimeTaskSourceArtifact::from_encoded_source(
                *source.expected_executable(),
                bytes,
            );
        }
        let program = candidate(source);
        let bytes = program.encode_canonical().unwrap();
        // Wire assertions are inert, so structural decoding does not mint them.
        let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
        let error = decoded
            .original_source
            .unwrap()
            .admit(limits, None)
            .unwrap_err();
        if forged_executable {
            assert!(matches!(
                error.seal_error(),
                Some(RuntimeTaskPlanSealError::ExpectedExecutableKeyMismatch)
            ));
        } else {
            assert!(matches!(
                error.seal_error(),
                Some(RuntimeTaskPlanSealError::ExpectedKeyMismatch { ordinal: 0 })
            ));
        }
    }
}

#[test]
fn canonical_awbc_source_enforces_wire_storage_and_work_before_retaining_bytes() {
    let source = original(2)
        .task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
        .unwrap();
    let bytes = candidate(source.clone()).encode_canonical().unwrap();
    let mut budget = AwbcDecodeBudget::default();
    budget.original_source.codec.encoded_bytes = source.bytes().len() - 1;
    let error = AwbcProgram::decode_canonical(&bytes, budget).unwrap_err();
    assert!(matches!(
        error,
        AwbcCodecError::BudgetExceeded { budget: "original_source_bytes", actual, limit }
            if actual == source.bytes().len() && limit == actual - 1
    ));
    let mut budget = AwbcDecodeBudget::default();
    budget.original_source.codec.decoded_bytes = 0;
    let AwbcCodecError::OriginalSource { source, .. } =
        AwbcProgram::decode_canonical(&bytes, budget).unwrap_err()
    else {
        panic!("the Source construction footprint must be rejected")
    };
    assert!(matches!(
        source.0.as_ref(),
        RuntimeTaskSourceCodecError::Limit { kind: "decoded bytes", maximum: 0, actual } if *actual > 0
    ));
    let mut budget = AwbcDecodeBudget::default();
    budget.original_source.seal.max_semantic_work = 0;
    let AwbcCodecError::OriginalSource { source, .. } =
        AwbcProgram::decode_canonical(&bytes, budget).unwrap_err()
    else {
        panic!("the original shared work allowance must be enforced")
    };
    assert!(matches!(
        source.0.as_ref(),
        RuntimeTaskSourceCodecError::Resource(TaskSemanticEncodingError::SemanticWork)
    ));
}

#[test]
fn awbc_execution_identity_uses_the_checked_e_claim_instead_of_source_wire_spelling() {
    let limits = RuntimeTaskSourceArtifactLimits::default();
    let source = original(2).task_source_artifact(limits).unwrap();
    let program = candidate(source.clone());
    let expected = program.executable_identity().unwrap();
    let mut bytes = source.bytes().to_vec();
    bytes[0] ^= 1;
    let changed_wire =
        RuntimeTaskSourceArtifact::from_encoded_source(*source.expected_executable(), bytes);
    assert_eq!(
        candidate(changed_wire.clone())
            .executable_identity()
            .unwrap(),
        expected
    );
    // An identity over assertion data does not admit the malformed Source.
    assert!(changed_wire.admit(limits, None).is_err());
    let mut claim = *source.expected_executable();
    claim[0] ^= 1;
    let changed_claim =
        RuntimeTaskSourceArtifact::from_encoded_source(claim, source.bytes().to_vec());
    assert_ne!(
        candidate(changed_claim).executable_identity().unwrap(),
        expected
    );
}
