use super::*;

fn limits() -> RuntimeTaskRequestCodecLimits {
    RuntimeTaskRequestCodecLimits {
        roles: 13,
        path_steps: 12,
        encoded_bytes: 10_000,
        decoded_bytes: 16_384,
        type_count: 1,
    }
}
fn fixture_template() -> RuntimeTaskRequestTemplate {
    let ty = RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
    let identity = RuntimeRequestRoleIdentity::from_accepted_identity([7; 32]);
    let path = || {
        Box::new([
            RuntimeRequestPathStep::Operand(1),
            RuntimeRequestPathStep::Tuple(2),
            RuntimeRequestPathStep::Record(identity),
            RuntimeRequestPathStep::Variant(identity),
            RuntimeRequestPathStep::CallArgument(3),
            RuntimeRequestPathStep::NamedArgument(identity),
            RuntimeRequestPathStep::SpreadArgument(4),
            RuntimeRequestPathStep::Capture(5),
            RuntimeRequestPathStep::AwaitManyItem,
            RuntimeRequestPathStep::TimeoutSource,
            RuntimeRequestPathStep::TimeoutLimit,
            RuntimeRequestPathStep::LineChild(6),
        ])
    };
    let roles = [
        RuntimeRequestArgumentRole::Positional,
        RuntimeRequestArgumentRole::Named,
        RuntimeRequestArgumentRole::Spread,
        RuntimeRequestArgumentRole::Capture,
        RuntimeRequestArgumentRole::AwaitManyItem,
        RuntimeRequestArgumentRole::TimeoutSource,
        RuntimeRequestArgumentRole::TimeoutLimit,
        RuntimeRequestArgumentRole::LineInput,
    ];
    let sources = [
        RuntimeRequestValueSource::Literal,
        RuntimeRequestValueSource::Local,
        RuntimeRequestValueSource::Capture,
        RuntimeRequestValueSource::Projection,
        RuntimeRequestValueSource::CallResult,
        RuntimeRequestValueSource::AggregateItem,
        RuntimeRequestValueSource::NeedHandle,
        RuntimeRequestValueSource::Literal,
    ];
    let arguments = roles
        .into_iter()
        .zip(sources)
        .enumerate()
        .map(|(ordinal, (role, source))| RuntimeRequestArgument {
            role,
            source,
            identity: (ordinal % 2 == 1).then_some(identity),
            ty,
            path: path(),
        })
        .collect();
    let roles = [
        RuntimeRequestFieldRole::Required,
        RuntimeRequestFieldRole::Optional,
        RuntimeRequestFieldRole::Repeated,
        RuntimeRequestFieldRole::NamedOnly,
        RuntimeRequestFieldRole::PositionalOnly,
    ];
    let fields = roles
        .into_iter()
        .map(|role| RuntimeRequestField {
            role,
            identity,
            ty,
            path: path(),
        })
        .collect();
    RuntimeTaskRequestTemplate::at_operation(3, arguments, fields)
}

#[test]
fn complete_request_grammar_round_trips_and_preserves_source_order() {
    let template = fixture_template();
    let bytes = template.encode(limits()).unwrap();
    let decoded = RuntimeTaskRequestTemplate::decode(&bytes, limits()).unwrap();
    assert_eq!(bytes, decoded.encode(limits()).unwrap());
    assert_eq!(&bytes[..10], &[1, 1, 3, 0, 0, 0, 8, 0, 0, 0]);
    let mut reversed = fixture_template();
    reversed.arguments.reverse();
    assert_ne!(bytes, reversed.encode(limits()).unwrap());
}

#[test]
fn request_codec_rejects_all_truncations_and_noncanonical_tags_and_coordinates() {
    let bytes = fixture_template().encode(limits()).unwrap();
    for end in 0..bytes.len() {
        assert!(RuntimeTaskRequestTemplate::decode(&bytes[..end], limits()).is_err());
    }
    let mut changed = bytes.clone();
    changed.push(0);
    assert!(matches!(
        RuntimeTaskRequestTemplate::decode(&changed, limits()),
        Err(RuntimeTaskRequestCodecError::Trailing)
    ));
    for (index, value) in [(0, 2), (1, 255), (14, 255), (15, 2), (20, 255), (25, 255)] {
        let mut changed = bytes.clone();
        changed[index] = value;
        assert!(
            RuntimeTaskRequestTemplate::decode(&changed, limits()).is_err(),
            "tamper at {index}"
        );
    }
    let mut changed = bytes.clone();
    changed[10] = 1;
    assert!(matches!(
        RuntimeTaskRequestTemplate::decode(&changed, limits()),
        Err(RuntimeTaskRequestCodecError::Ordinal {
            expected: 0,
            actual: 1
        })
    ));
    let mut changed = bytes;
    changed[16..20].fill(0);
    assert!(matches!(
        RuntimeTaskRequestTemplate::decode(&changed, limits()),
        Err(RuntimeTaskRequestCodecError::ZeroType)
    ));
}

#[test]
fn request_codec_limits_apply_before_allocation_and_accept_exact_boundaries() {
    let template = fixture_template();
    let bytes = template.encode(limits()).unwrap();
    let exact = RuntimeTaskRequestCodecLimits {
        encoded_bytes: bytes.len(),
        decoded_bytes: RuntimeTaskRequestTemplate::preflight_source(&bytes, limits())
            .unwrap()
            .storage_bytes(),
        ..limits()
    };
    assert_eq!(bytes, template.encode(exact).unwrap());
    RuntimeTaskRequestTemplate::decode(&bytes, exact).unwrap();
    for short in [
        RuntimeTaskRequestCodecLimits { roles: 12, ..exact },
        RuntimeTaskRequestCodecLimits {
            path_steps: 11,
            ..exact
        },
        RuntimeTaskRequestCodecLimits {
            encoded_bytes: bytes.len() - 1,
            ..exact
        },
        RuntimeTaskRequestCodecLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
        RuntimeTaskRequestCodecLimits {
            type_count: 0,
            ..exact
        },
    ] {
        assert!(matches!(
            template.encode(short),
            Err(RuntimeTaskRequestCodecError::Limit { .. })
        ));
        assert!(matches!(
            RuntimeTaskRequestTemplate::decode(&bytes, short),
            Err(RuntimeTaskRequestCodecError::Limit { .. })
        ));
    }
    let mut changed = bytes;
    changed[6..10].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        RuntimeTaskRequestTemplate::decode(&changed, exact),
        Err(RuntimeTaskRequestCodecError::Limit { kind: "roles", .. })
    ));
}

#[test]
fn combined_role_limit_precedes_a_truncated_field_list() {
    let bytes = fixture_template().encode(limits()).unwrap();
    let mut arguments_only = fixture_template();
    arguments_only.fields = Box::new([]);
    let field_count_end = arguments_only.encode(limits()).unwrap().len();
    let short = RuntimeTaskRequestCodecLimits {
        roles: 12,
        ..limits()
    };
    assert!(matches!(
        RuntimeTaskRequestTemplate::decode(&bytes[..field_count_end], short),
        Err(RuntimeTaskRequestCodecError::Limit {
            kind: "roles",
            actual: 13,
            maximum: 12
        })
    ));
}

#[test]
fn request_source_preflight_counts_all_owned_rows_and_paths_before_materialization() {
    let source = fixture_template();
    let bytes = source.encode(limits()).unwrap();
    let checked = RuntimeTaskRequestTemplate::preflight_source(&bytes, limits()).unwrap();
    let expected = 8 * std::mem::size_of::<RuntimeRequestArgument>()
        + 5 * std::mem::size_of::<RuntimeRequestField>()
        + 13 * 12 * std::mem::size_of::<RuntimeRequestPathStep>();
    assert_eq!(checked.storage_bytes(), expected);
    let exact = RuntimeTaskRequestCodecLimits {
        decoded_bytes: expected,
        ..limits()
    };
    let retained = RuntimeTaskRequestTemplate::preflight_source(&bytes, exact)
        .unwrap()
        .materialize()
        .unwrap();
    assert_eq!(retained, source);
    assert_eq!(retained.encode(exact).unwrap(), bytes);
    assert!(matches!(
        RuntimeTaskRequestTemplate::preflight_source(
            &bytes,
            RuntimeTaskRequestCodecLimits {
                decoded_bytes: expected - 1,
                ..exact
            },
        ),
        Err(RuntimeTaskRequestCodecError::Limit {
            kind: "decoded bytes",
            actual,
            maximum,
        }) if actual == expected && maximum == expected - 1
    ));
}

#[test]
fn request_source_preflight_rejects_a_late_type_outside_the_original_inventory() {
    let mut source = fixture_template();
    source.fields.last_mut().unwrap().ty =
        RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::new(2).unwrap());
    let bytes = source
        .encode(RuntimeTaskRequestCodecLimits {
            type_count: 2,
            ..limits()
        })
        .unwrap();
    for error in [
        RuntimeTaskRequestTemplate::preflight_source(&bytes, limits())
            .err()
            .unwrap(),
        RuntimeTaskRequestTemplate::decode(&bytes, limits()).unwrap_err(),
        source.encode(limits()).unwrap_err(),
    ] {
        assert_eq!(
            error,
            RuntimeTaskRequestCodecError::Limit {
                kind: "type coordinate",
                actual: 2,
                maximum: 1,
            }
        );
    }
}
