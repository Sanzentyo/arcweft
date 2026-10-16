use super::*;

fn limits() -> RuntimeTypeScopeCodecLimits {
    RuntimeTypeScopeCodecLimits {
        encoded_bytes: 1024,
        decoded_bytes: 1024,
    }
}

fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(u64::MAX, u64::MAX)
}

fn encode(scope: &RuntimeTypeScope) -> Vec<u8> {
    let prepared = scope
        .prepare_source_encoding(limits(), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn version_one_scope_wire_preserves_each_original_namespace() {
    let scope = RuntimeTypeScope::try_from_binders(vec![
        RuntimeTypeBinder::new(1, 2, 3),
        RuntimeTypeBinder::new(4, 5, 6),
    ])
    .unwrap();
    let bytes = encode(&scope);
    assert_eq!(
        bytes,
        [
            1, 2, 0, 0, 0, 1, 0, 2, 0, 3, 0, 0, 0, 4, 0, 5, 0, 6, 0, 0, 0
        ]
    );
    let retained = RuntimeTypeScope::preflight_source(&bytes, limits(), &mut meter())
        .unwrap()
        .materialize();
    assert_eq!(retained, scope);
    // References continue to use the incoming outermost-to-innermost scope;
    // neither counts nor a digest stand in for the original lexical rows.
    assert_eq!(retained.bound_type(0, 3).unwrap().slot(), 3);
    assert_eq!(retained.bound_type(1, 0).unwrap().depth(), 1);
    assert!(retained.bound_type(1, 1).is_err());
    assert_eq!(retained.bound_const(0, 4).unwrap().slot(), 4);
    assert!(retained.bound_const(1, 2).is_err());
    assert_eq!(retained.bound_effect(0, 5).unwrap().slot(), 5);
    assert!(retained.bound_effect(1, 3).is_err());
    assert!(retained.require_root().is_err());
}

#[test]
fn root_scope_has_no_retained_allocation_and_remains_an_executable_root() {
    let bytes = encode(&RuntimeTypeScope::root());
    assert_eq!(bytes, [1, 0, 0, 0, 0]);
    let checked = RuntimeTypeScope::preflight_source(
        &bytes,
        RuntimeTypeScopeCodecLimits {
            encoded_bytes: bytes.len(),
            decoded_bytes: 0,
        },
        &mut meter(),
    )
    .unwrap();
    assert_eq!(checked.storage_bytes(), 0);
    let scope = checked.materialize();
    assert_eq!(scope, RuntimeTypeScope::root());
    scope.require_root().unwrap();
    assert!(scope.bound_type(0, 0).is_err());
}

#[test]
fn maximum_scope_depth_and_arities_round_trip_without_narrowing() {
    let scope = RuntimeTypeScope::try_from_binders(vec![
        RuntimeTypeBinder::new(
            u16::MAX,
            u16::MAX,
            u32::MAX
        );
        MAX_RUNTIME_PLAN_TYPE_DEPTH
    ])
    .unwrap();
    let bytes = encode(&scope);
    let retained = RuntimeTypeScope::preflight_source(&bytes, limits(), &mut meter())
        .unwrap()
        .materialize();
    assert_eq!(retained, scope);
    let outermost = u32::try_from(MAX_RUNTIME_PLAN_TYPE_DEPTH - 1).unwrap();
    assert!(retained.bound_type(outermost, u16::MAX - 1).is_ok());
    assert!(retained.bound_const(outermost, u16::MAX - 1).is_ok());
    assert!(retained.bound_effect(outermost, u32::MAX - 1).is_ok());
    assert!(retained.bound_effect(outermost, u32::MAX).is_err());
}

#[test]
fn all_truncations_and_late_invalid_binders_fail_whole_frame_admission() {
    let scope = RuntimeTypeScope::try_from_binders(vec![
        RuntimeTypeBinder::new(0, 1, 0),
        RuntimeTypeBinder::new(0, 0, 1),
    ])
    .unwrap();
    let bytes = encode(&scope);
    for cut in 0..bytes.len() {
        assert_eq!(
            RuntimeTypeScope::preflight_source(&bytes[..cut], limits(), &mut meter())
                .err()
                .unwrap(),
            RuntimeTypeScopeCodecError::Truncated,
            "cut {cut}"
        );
    }
    let mut late_empty = bytes.clone();
    late_empty[HEADER_BYTES + BINDER_BYTES..].fill(0);
    assert_eq!(
        RuntimeTypeScope::preflight_source(&late_empty, limits(), &mut meter())
            .err()
            .unwrap(),
        RuntimeTypeScopeCodecError::Scope(RuntimeTypeScopeError::EmptyBinder)
    );
    let mut wrong_version = bytes.clone();
    wrong_version[0] = 2;
    assert_eq!(
        RuntimeTypeScope::preflight_source(&wrong_version, limits(), &mut meter())
            .err()
            .unwrap(),
        RuntimeTypeScopeCodecError::Version { actual: 2 }
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        RuntimeTypeScope::preflight_source(&trailing, limits(), &mut meter())
            .err()
            .unwrap(),
        RuntimeTypeScopeCodecError::Trailing
    );
    let mut overflow_count = bytes;
    overflow_count[1..HEADER_BYTES].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        RuntimeTypeScope::preflight_source(&overflow_count, limits(), &mut meter())
            .err()
            .unwrap(),
        RuntimeTypeScopeCodecError::Scope(RuntimeTypeScopeError::DepthLimit)
    );
}

#[test]
fn exact_encoded_and_retained_storage_limits_are_shared_by_both_directions() {
    let scope = RuntimeTypeScope::try_from_binders(vec![RuntimeTypeBinder::new(1, 0, 0)]).unwrap();
    let bytes = encode(&scope);
    let storage = size_of::<RuntimeTypeBinder>();
    let exact = RuntimeTypeScopeCodecLimits {
        encoded_bytes: bytes.len(),
        decoded_bytes: storage,
    };
    let prepared = scope.prepare_source_encoding(exact, &mut meter()).unwrap();
    assert_eq!(prepared.encoded_bytes(), bytes.len());
    assert_eq!(prepared.storage_bytes(), storage);
    assert_eq!(
        RuntimeTypeScope::preflight_source(&bytes, exact, &mut meter())
            .unwrap()
            .storage_bytes(),
        storage
    );
    for short in [
        RuntimeTypeScopeCodecLimits {
            encoded_bytes: bytes.len() - 1,
            ..exact
        },
        RuntimeTypeScopeCodecLimits {
            decoded_bytes: storage - 1,
            ..exact
        },
    ] {
        let encode_error = scope
            .prepare_source_encoding(short, &mut meter())
            .err()
            .unwrap();
        let decode_error = RuntimeTypeScope::preflight_source(&bytes, short, &mut meter())
            .err()
            .unwrap();
        assert_eq!(encode_error, decode_error);
        assert!(matches!(
            encode_error,
            RuntimeTypeScopeCodecError::Limit { .. }
        ));
    }
}

#[test]
fn parent_append_keeps_prefix_and_pointer_and_never_reserves_capacity() {
    let scope = RuntimeTypeScope::try_from_binders(vec![RuntimeTypeBinder::new(1, 0, 0)]).unwrap();
    let source = encode(&scope);
    let mut output = Vec::with_capacity(3 + source.len());
    output.extend_from_slice(&[9, 8, 7]);
    let pointer = output.as_ptr();
    let capacity = output.capacity();
    scope
        .prepare_source_encoding(limits(), &mut meter())
        .unwrap()
        .append_to(&mut output)
        .unwrap();
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(output.capacity(), capacity);
    assert_eq!(&output[..3], [9, 8, 7]);
    assert_eq!(&output[3..], source);
    let mut no_capacity = Vec::new();
    assert_eq!(
        scope
            .prepare_source_encoding(limits(), &mut meter())
            .unwrap()
            .append_to(&mut no_capacity),
        Err(RuntimeTypeScopeCodecError::OutputCapacity {
            required: source.len(),
            remaining: 0,
        })
    );
    assert_eq!(no_capacity.len(), 0);
    assert_eq!(no_capacity.capacity(), 0);
}

#[test]
fn encoding_and_decoding_consume_the_original_bank_without_clearing_its_error() {
    let scope = RuntimeTypeScope::root();
    let bytes = encode(&scope);
    let work = 3 * u64::try_from(bytes.len()).unwrap();
    for decoding in [false, true] {
        let mut exact = TaskSemanticMeter::new(1 + work, u64::MAX);
        exact.charge_work(1).unwrap();
        if decoding {
            RuntimeTypeScope::preflight_source(&bytes, limits(), &mut exact).unwrap();
        } else {
            scope.prepare_source_encoding(limits(), &mut exact).unwrap();
        }
        assert_eq!(exact.status(), Ok(()));
        assert_eq!(
            exact.charge_work(1),
            Err(TaskSemanticEncodingError::SemanticWork)
        );
        let mut short = TaskSemanticMeter::new(work - 1, u64::MAX);
        let error = if decoding {
            RuntimeTypeScope::preflight_source(&bytes, limits(), &mut short)
                .err()
                .unwrap()
        } else {
            scope
                .prepare_source_encoding(limits(), &mut short)
                .err()
                .unwrap()
        };
        assert_eq!(
            error,
            RuntimeTypeScopeCodecError::Resource(TaskSemanticEncodingError::SemanticWork)
        );
        // Even a later invalid scope/limit cannot replace the first error.
        assert_eq!(
            RuntimeTypeScope::preflight_source(&[], limits(), &mut short)
                .err()
                .unwrap(),
            error
        );
    }
}

#[test]
fn invalid_scope_poisoning_is_sticky_for_later_valid_source() {
    let bytes = [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut bank = meter();
    assert!(matches!(
        RuntimeTypeScope::preflight_source(&bytes, limits(), &mut bank),
        Err(RuntimeTypeScopeCodecError::Scope(
            RuntimeTypeScopeError::EmptyBinder
        ))
    ));
    assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    assert_eq!(
        RuntimeTypeScope::root()
            .prepare_source_encoding(limits(), &mut bank)
            .err()
            .unwrap(),
        RuntimeTypeScopeCodecError::Resource(TaskSemanticEncodingError::OwnerRejected)
    );
}
