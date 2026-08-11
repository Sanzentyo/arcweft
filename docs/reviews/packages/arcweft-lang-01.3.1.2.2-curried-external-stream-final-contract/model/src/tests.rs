use super::*;

fn digest(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn ty(byte: u8) -> TypeLayoutHash {
    TypeLayoutHash(digest(byte))
}

fn value(type_byte: u8, digest_byte: u8) -> CheckedValue {
    CheckedValue {
        ty: ty(type_byte),
        digest: ValueDigest(digest(digest_byte)),
        ownership: Ownership::Unrestricted,
    }
}

fn parameter(group: u16, parameter: u16, presence: Presence) -> Parameter {
    Parameter {
        coordinate: Coordinate::new(group, parameter),
        name: Some(format!("p{group}_{parameter}")),
        passing: Passing::PositionalOrNamed,
        presence,
        ty: ty(10 + group as u8 + parameter as u8),
    }
}

fn signature(group_sizes: &[usize]) -> Signature {
    Signature {
        definition: DefinitionId(digest(1)),
        declaration: DeclarationDigest(digest(2)),
        fingerprint: SignatureFingerprint(digest(3)),
        groups: group_sizes
            .iter()
            .enumerate()
            .map(|(group_index, size)| Group {
                index: GroupIndex(u16::try_from(group_index).unwrap()),
                kind: if group_index == 0 {
                    GroupKind::Initial
                } else {
                    GroupKind::Curried
                },
                parameters: (0..*size)
                    .map(|parameter_index| {
                        parameter(
                            u16::try_from(group_index).unwrap(),
                            u16::try_from(parameter_index).unwrap(),
                            Presence::Required,
                        )
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn group_arguments(signature: &Signature, group: u16) -> EvaluatedGroup {
    let declared = &signature.groups[usize::from(group)];
    EvaluatedGroup {
        group: GroupIndex(group),
        coordinates: declared
            .parameters
            .iter()
            .map(|parameter| parameter.coordinate)
            .collect(),
        values: declared
            .parameters
            .iter()
            .enumerate()
            .map(|(index, _parameter)| {
                ArgumentValue::Explicit(value(
                    10 + group as u8 + index as u8,
                    30 + group as u8 + index as u8,
                ))
            })
            .collect(),
    }
}

#[test]
fn non_final_group_captures_without_opening() {
    let signature = signature(&[1, 1]);
    let live = LiveGenerations::with([GenerationId(7)]);
    let partial = ExternalStreamPartial::initial(&signature, GenerationId(7), &live).unwrap();
    let mut state = RuntimeState::default();

    let outcome = partial
        .apply(
            &signature,
            &group_arguments(&signature, 0),
            &live,
            &mut state,
        )
        .unwrap();
    let ApplyOutcome::Partial(next) = outcome else {
        panic!("expected partial");
    };
    assert_eq!(next.next_group, GroupIndex(1));
    assert_eq!(next.captured.completed_groups, 1);
    assert!(state.open_requests.is_empty());
    assert_eq!(state.next_instance, 0);
}

#[test]
fn final_group_opens_once_with_all_coordinates() {
    let signature = signature(&[1, 1, 1]);
    let live = LiveGenerations::with([GenerationId(7)]);
    let mut state = RuntimeState::default();
    let initial = ExternalStreamPartial::initial(&signature, GenerationId(7), &live).unwrap();
    let ApplyOutcome::Partial(first) = initial
        .apply(
            &signature,
            &group_arguments(&signature, 0),
            &live,
            &mut state,
        )
        .unwrap()
    else {
        panic!("expected first partial");
    };
    let ApplyOutcome::Partial(second) = first
        .apply(
            &signature,
            &group_arguments(&signature, 1),
            &live,
            &mut state,
        )
        .unwrap()
    else {
        panic!("expected second partial");
    };
    let ApplyOutcome::Open(request) = second
        .apply(
            &signature,
            &group_arguments(&signature, 2),
            &live,
            &mut state,
        )
        .unwrap()
    else {
        panic!("expected open");
    };

    assert_eq!(request.arguments.completed_groups, 3);
    assert_eq!(
        request.arguments.coordinates,
        vec![
            Coordinate::new(0, 0),
            Coordinate::new(1, 0),
            Coordinate::new(2, 0),
        ]
    );
    assert_eq!(state.open_requests, vec![request]);
    assert_eq!(state.next_instance, 1);
}

#[test]
fn empty_group_progress_is_not_inferred_from_cells() {
    let signature = signature(&[1, 0, 1]);
    let live = LiveGenerations::with([GenerationId(7)]);
    let mut state = RuntimeState::default();
    let initial = ExternalStreamPartial::initial(&signature, GenerationId(7), &live).unwrap();
    let ApplyOutcome::Partial(first) = initial
        .apply(
            &signature,
            &group_arguments(&signature, 0),
            &live,
            &mut state,
        )
        .unwrap()
    else {
        panic!("expected first partial");
    };
    let ApplyOutcome::Partial(second) = first
        .apply(
            &signature,
            &group_arguments(&signature, 1),
            &live,
            &mut state,
        )
        .unwrap()
    else {
        panic!("expected second partial");
    };
    assert_eq!(second.captured.completed_groups, 2);
    assert_eq!(second.captured.coordinates, vec![Coordinate::new(0, 0)]);
    assert!(state.open_requests.is_empty());
}

#[test]
fn reordered_coordinates_are_rejected_without_state_mutation() {
    let signature = signature(&[2]);
    let live = LiveGenerations::with([GenerationId(7)]);
    let partial = ExternalStreamPartial::initial(&signature, GenerationId(7), &live).unwrap();
    let mut state = RuntimeState::default();
    let mut arguments = group_arguments(&signature, 0);
    arguments.coordinates.swap(0, 1);
    arguments.values.swap(0, 1);
    let before = state.clone();

    let error = partial
        .apply(&signature, &arguments, &live, &mut state)
        .unwrap_err();
    assert!(matches!(
        error,
        ApplyError::Argument(ArgumentError::OutOfOrderCoordinate { .. })
    ));
    assert_eq!(state, before);
}

#[test]
fn foreign_declaration_and_stale_generation_are_rejected() {
    let signature = signature(&[1, 1]);
    let live = LiveGenerations::with([GenerationId(7)]);
    let partial = ExternalStreamPartial::initial(&signature, GenerationId(7), &live).unwrap();

    let mut foreign_signature = signature.clone();
    foreign_signature.declaration = DeclarationDigest(digest(99));
    assert_eq!(
        partial
            .captured
            .validate_prefix(&foreign_signature, &live)
            .unwrap_err(),
        ArgumentError::ForeignDeclaration
    );

    let no_live_generations = LiveGenerations::default();
    assert_eq!(
        partial
            .captured
            .validate_prefix(&signature, &no_live_generations)
            .unwrap_err(),
        ArgumentError::StaleGeneration
    );
}

#[test]
fn wrong_type_and_duplicate_affine_token_are_rejected() {
    let mut signature = signature(&[2]);
    signature.groups[0].parameters[0].ty = ty(50);
    signature.groups[0].parameters[1].ty = ty(50);
    let live = LiveGenerations::with([GenerationId(7)]);
    let partial = ExternalStreamPartial::initial(&signature, GenerationId(7), &live).unwrap();
    let mut state = RuntimeState::default();
    let arguments = EvaluatedGroup {
        group: GroupIndex(0),
        coordinates: vec![Coordinate::new(0, 0), Coordinate::new(0, 1)],
        values: vec![
            ArgumentValue::Explicit(CheckedValue {
                ty: ty(50),
                digest: ValueDigest(digest(10)),
                ownership: Ownership::Affine(9),
            }),
            ArgumentValue::Explicit(CheckedValue {
                ty: ty(50),
                digest: ValueDigest(digest(11)),
                ownership: Ownership::Affine(9),
            }),
        ],
    };

    assert_eq!(
        partial
            .apply(&signature, &arguments, &live, &mut state)
            .unwrap_err(),
        ApplyError::Argument(ArgumentError::DuplicateAffineToken(9))
    );
    assert!(state.open_requests.is_empty());
}

#[test]
fn named_rest_requires_unique_canonical_names() {
    let mut signature = signature(&[1]);
    signature.groups[0].parameters[0].passing = Passing::RestNamed;
    signature.groups[0].parameters[0].ty = ty(70);
    let live = LiveGenerations::with([GenerationId(7)]);
    let partial = ExternalStreamPartial::initial(&signature, GenerationId(7), &live).unwrap();
    let mut state = RuntimeState::default();
    let arguments = EvaluatedGroup {
        group: GroupIndex(0),
        coordinates: vec![Coordinate::new(0, 0)],
        values: vec![ArgumentValue::RestNamed {
            value_ty: ty(70),
            entries: vec![
                NamedRestEntry {
                    name: "z".to_owned(),
                    value: value(70, 1),
                },
                NamedRestEntry {
                    name: "a".to_owned(),
                    value: value(70, 2),
                },
            ],
        }],
    };

    assert_eq!(
        partial
            .apply(&signature, &arguments, &live, &mut state)
            .unwrap_err(),
        ApplyError::Argument(ArgumentError::OutOfOrderNamedRestEntry(
            Coordinate::new(0, 0)
        ))
    );
}

#[test]
fn instance_overflow_is_atomic() {
    let signature = signature(&[1]);
    let live = LiveGenerations::with([GenerationId(7)]);
    let partial = ExternalStreamPartial::initial(&signature, GenerationId(7), &live).unwrap();
    let mut state = RuntimeState {
        next_instance: u64::MAX,
        open_requests: Vec::new(),
    };
    let before = state.clone();

    assert_eq!(
        partial
            .apply(
                &signature,
                &group_arguments(&signature, 0),
                &live,
                &mut state,
            )
            .unwrap_err(),
        ApplyError::InstanceIdOverflow
    );
    assert_eq!(state, before);
}

#[test]
fn signature_changes_are_generational() {
    let old = signature(&[1, 1]);
    let mut changed = old.clone();
    changed.groups[1].parameters[0].presence = Presence::Optional;
    assert_eq!(
        classify_swap(&old, &old, true, true),
        SwapCompatibility::CodeCompatible
    );
    assert_eq!(
        classify_swap(&old, &changed, true, true),
        SwapCompatibility::CodeGenerational
    );
    assert_eq!(
        classify_swap(&old, &old, false, true),
        SwapCompatibility::RestartRequired
    );
}

#[test]
fn numeric_allocations_are_frozen() {
    assert_eq!(AWBC_ABI_VERSION, 2);
    assert_eq!(AWBC_CODEC_VERSION, 8);
    assert_eq!(SAVE_SCHEMA_VERSION, 2);
    assert_eq!(AWBC_RUNTIME_TYPE_STREAM_HANDLE, 21);
    assert_eq!(AWBC_RUNTIME_TYPE_EXTERNAL_STREAM_CALLABLE, 22);
    assert_eq!(AWBC_CONSTANT_EXTERNAL_STREAM_CALLABLE, 18);
    assert_eq!(AWBC_OPCODE_APPLY_EXTERNAL_STREAM_GROUP, 0x27);
    assert_eq!(AWBC_OPCODE_OPEN_STREAM, 0x28);
}
