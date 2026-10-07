use super::*;
use crate::effect_row::{EffectFormula, EffectPredicate, EffectSet};
use crate::task::semantic::TaskSemanticMeter;

fn contract(labels: &[&str], slot: u32) -> RuntimeFunctionTypeContract {
    let binder = RuntimeTypeBinder::new(1, 1, 2);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    RuntimeFunctionTypeContract::new(
        binder,
        EffectPredicate::unconstrained(),
        EffectFormula::literal(
            EffectSet::from_labels(labels.iter().copied()).unwrap(),
            Some(scope.bound_effect(0, slot).unwrap()),
        ),
    )
}

fn digest(
    contract: &RuntimeFunctionTypeContract,
    work: u64,
    bytes: u64,
) -> (Result<blake3::Hash, TaskSemanticEncodingError>, (u64, u64)) {
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let mut encoder = TaskSemanticEncoder::new(b"type-contract-test.v1\0", &mut meter);
    let result = contract.encode_semantic_contract(&mut encoder);
    let digest = encoder.finish();
    assert_eq!(result.is_ok(), digest.is_ok());
    (digest, meter.totals())
}

#[test]
fn effect_contract_uses_canonical_membership_and_preserves_lexical_role() {
    let first = contract(&["fs.read", "fs.write"], 0);
    let reordered = contract(&["fs.write", "fs.read"], 0);
    assert_eq!(
        digest(&first, 1_000, 10_000),
        digest(&reordered, 1_000, 10_000)
    );
    assert_ne!(
        digest(&first, 1_000, 10_000).0,
        digest(&contract(&["fs.read", "fs.write"], 1), 1_000, 10_000).0
    );
    assert_ne!(
        digest(&first, 1_000, 10_000).0,
        digest(&contract(&["fs.read"], 0), 1_000, 10_000).0
    );
    let wider = RuntimeFunctionTypeContract::new(
        RuntimeTypeBinder::new(2, 1, 2),
        first.predicate().clone(),
        first.invocation().clone(),
    );
    assert_ne!(
        digest(&first, 1_000, 10_000).0,
        digest(&wider, 1_000, 10_000).0
    );
}

#[test]
fn contract_uses_the_surrounding_exact_work_and_byte_budget() {
    let contract = contract(&["fs.read", "fs.write"], 1);
    let (expected, (work, bytes)) = digest(&contract, 1_000, 10_000);
    assert!(expected.is_ok());
    assert_eq!(digest(&contract, work, bytes).0, expected);
    assert_eq!(
        digest(&contract, work - 1, bytes).0,
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    assert_eq!(
        digest(&contract, work, bytes - 1).0,
        Err(TaskSemanticEncodingError::TranscriptBytes)
    );
    let mut meter = TaskSemanticMeter::new(work, bytes);
    let mut encoder = TaskSemanticEncoder::new(b"type-contract-test.v1\0", &mut meter);
    encoder.tag(9);
    assert_eq!(
        contract.encode_semantic_contract(&mut encoder),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    assert_eq!(
        encoder.finish(),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
}

#[test]
fn previous_failure_prevents_any_contract_transcript() {
    let contract = contract(&["fs.read"], 0);
    let mut meter = TaskSemanticMeter::new(0, 10_000);
    assert_eq!(
        meter.charge_work(1),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    let before = meter.totals();
    let mut encoder = TaskSemanticEncoder::new(b"type-contract-test.v1\0", &mut meter);
    assert_eq!(
        contract.encode_semantic_contract(&mut encoder),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    assert_eq!(
        encoder.finish(),
        Err(TaskSemanticEncodingError::SemanticWork)
    );
    assert_eq!(meter.totals(), before);
}

#[test]
fn scope_order_and_array_length_roles_have_exact_distinct_transcripts() {
    let a = RuntimeTypeBinder::new(1, 2, 3);
    let b = RuntimeTypeBinder::new(2, 1, 3);
    let hash = |binders: &[RuntimeTypeBinder], length: RuntimeArrayLength| {
        let scope = RuntimeTypeScope::try_from_binders(binders).unwrap();
        let mut meter = TaskSemanticMeter::new(1_000, 10_000);
        let mut encoder = TaskSemanticEncoder::new(b"type-scope-test.v1\0", &mut meter);
        scope.encode_semantic_scope(&mut encoder).unwrap();
        length.encode_semantic_length(&mut encoder);
        encoder.finish().unwrap()
    };
    assert_ne!(
        hash(&[a, b], RuntimeArrayLength::Constant(1)),
        hash(&[b, a], RuntimeArrayLength::Constant(1))
    );
    assert_ne!(
        hash(&[a], RuntimeArrayLength::Constant(1)),
        hash(&[a], RuntimeArrayLength::Constant(2))
    );
    let scope = RuntimeTypeScope::root().enter(a).unwrap();
    assert_ne!(
        hash(
            &[a],
            RuntimeArrayLength::Bound(scope.bound_const(0, 0).unwrap())
        ),
        hash(
            &[a],
            RuntimeArrayLength::Bound(scope.bound_const(0, 1).unwrap())
        )
    );
    assert_ne!(
        hash(&[a], RuntimeArrayLength::Constant(0)),
        hash(
            &[a],
            RuntimeArrayLength::Bound(scope.bound_const(0, 0).unwrap())
        )
    );
}
