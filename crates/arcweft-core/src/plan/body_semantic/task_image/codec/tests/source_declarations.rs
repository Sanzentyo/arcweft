use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeControlEffectCancellation, RuntimeControlEffectCardinality,
    RuntimeControlEffectContractDefinition, RuntimeControlEffectKind, RuntimeControlEffectOrdering,
    RuntimeControlEffectRow, RuntimeControlEffectTerminalBehavior, RuntimeEffectSet,
    RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionInputBindingSeed, RuntimeFunctionInputOrigin, RuntimeFunctionInputSource,
    RuntimeFunctionInputTransfer, RuntimeFunctionParameterIdentity,
    RuntimeFunctionParameterPassing, RuntimeFunctionSemanticRole, RuntimeFunctionSiteBodyKind,
    RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionTypeContract,
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind, RuntimeLocalBindingStorage,
    RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource, RuntimeLocalReadSeed,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlanBuilder, RuntimePlanTypeProjection,
    RuntimePlanTypeSeed, RuntimeTaskControlMode, RuntimeTypeBinder, RuntimeTypeScope,
    RuntimeValueOwnershipRequirement,
};
use crate::value::{RuntimeLocalReadMode, RuntimeValue};

pub(super) fn nominal_candidate() -> UnsealedRuntimePlanImage {
    let mut builder = crate::plan::type_table::source_codec::domains::tests::builder(
        crate::entry::RuntimeNominalRecordShape::Record,
        2,
        true,
    );
    let boolean = RuntimeSemanticTypeId::from_bytes([3; 32]);
    builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([125; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [],
            RuntimeExprSeed::new(
                boolean,
                RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
            ),
        )
        .unwrap();
    builder
        .push_control_effect_contract(RuntimeControlEffectContractDefinition {
            mode: RuntimeTaskControlMode::MustSuspend,
            effects: Box::new([RuntimeControlEffectRow {
                kind: RuntimeControlEffectKind::HostOperation,
                identity: None,
                inputs: Box::new([]),
                output: Some(boolean),
                cardinality: RuntimeControlEffectCardinality::ExactlyOnce,
                ordering: RuntimeControlEffectOrdering::SingleTerminal,
                cancellation: RuntimeControlEffectCancellation::NotObserved,
                terminal: RuntimeControlEffectTerminalBehavior::Value,
            }]),
            children: Box::new([]),
        })
        .unwrap();
    builder
        .prepare_task_image(RuntimeTaskPlanSealLimits::default())
        .unwrap()
}

fn assert_body_literal_limit(error: RuntimeTaskSourceCodecError, expected_kind: &'static str) {
    use crate::plan::construction::source::{expression, function_bodies};
    let RuntimeTaskSourceCodecError::Program(error) = error else {
        panic!("expected a program admission failure: {error:?}");
    };
    let RuntimeProgramSourceError::Bodies(
        function_bodies::RuntimeFunctionBodySourceError::Expression(error),
    ) = *error
    else {
        panic!("expected a body admission failure: {error:?}");
    };
    assert!(
        matches!(*error,
        expression::RuntimeExpressionSourceError::Literal(crate::value::RuntimeLiteralSourceError::Limit {
            kind, actual, maximum,
        }) if kind == expected_kind && actual == maximum + 1),
        "{error:?}"
    );
}

#[test]
fn nominal_declarations_share_the_composed_limits_and_original_executable_seal() {
    let original = nominal_candidate()
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    let encoded = bytes(&original);
    let program = original
        .encode_program_source(
            limits().program,
            &resources(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
    let elements = program.elements();
    let storage = program.storage_bytes();
    let exact = RuntimeTaskSourceCodecLimits {
        program: RuntimeProgramSourceLimits {
            elements: elements as u32,
            ..limits().program
        },
        decoded_bytes: storage,
        ..limits()
    };
    let restored = decode_image(
        &encoded,
        exact,
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    )
    .unwrap()
    .seal(RuntimeTaskPlanSealLimits::default(), None)
    .unwrap()
    .into_plan();
    assert_eq!(
        restored.nominal_record_domains(),
        original.nominal_record_domains()
    );
    assert_eq!(restored.variant_domains(), original.variant_domains());
    assert_eq!(
        restored.executable_semantic_digest(),
        original.executable_semantic_digest()
    );
    assert_eq!(bytes(&restored), encoded);
    for (short, kind) in [
        (
            RuntimeTaskSourceCodecLimits {
                program: RuntimeProgramSourceLimits {
                    elements: exact.program.elements - 1,
                    ..exact.program
                },
                ..exact
            },
            "source elements",
        ),
        (
            RuntimeTaskSourceCodecLimits {
                decoded_bytes: storage - 1,
                ..exact
            },
            "decoded bytes",
        ),
    ] {
        let error = decode_image(
            &encoded,
            short,
            TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .err()
        .unwrap();
        assert_body_literal_limit(error, kind);
    }
}

#[test]
fn nominal_source_distinguishes_codec_policy_from_diagnostic_labels_in_original_semantics() {
    let original = nominal_candidate()
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    let encoded = bytes(&original);
    let [_, nominals, _, _] = declaration_ranges(&encoded);
    for (old, replacement, semantic) in [
        (b"wire0".as_slice(), b"other".as_slice(), true),
        (b"Missing".as_slice(), b"Absent_".as_slice(), false),
    ] {
        let offsets = encoded[nominals.clone()]
            .windows(old.len())
            .enumerate()
            .filter_map(|(offset, bytes)| (bytes == old).then_some(offset))
            .collect::<Vec<_>>();
        assert_eq!(offsets.len(), 1);
        let start = nominals.start + offsets[0];
        let mut changed = encoded.clone();
        changed[start..start + old.len()].copy_from_slice(replacement);
        let restored = decode_image(
            &changed,
            limits(),
            TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap()
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
        // Domain labels are diagnostic; the accepted owner/layout and actual
        // occurrence codec remain the nominal semantic authority.
        if semantic {
            assert_ne!(
                restored.executable_semantic_digest(),
                original.executable_semantic_digest()
            );
        } else {
            assert_eq!(
                restored.executable_semantic_digest(),
                original.executable_semantic_digest()
            );
        }
        assert!(
            restored.nominal_record_domains() != original.nominal_record_domains()
                || restored.variant_domains() != original.variant_domains()
        );
        assert_eq!(bytes(&restored), changed);
        assert_eq!(bytes(&original), encoded);
    }
}

pub(super) fn declaration_candidate() -> UnsealedRuntimePlanImage {
    declaration_candidate_with(false, RuntimeTaskControlMode::MustSuspend)
}

fn declaration_candidate_with(
    mutable: bool,
    mode: RuntimeTaskControlMode,
) -> UnsealedRuntimePlanImage {
    let mut builder = RuntimePlanBuilder::new();
    let unit = RuntimeSemanticTypeId::from_bytes([101; 32]);
    let boolean = RuntimeSemanticTypeId::from_bytes([102; 32]);
    let function_type = RuntimeSemanticTypeId::from_bytes([103; 32]);
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    function_type,
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            RuntimeTypeBinder::new(0, 0, 0),
                            crate::effect_row::EffectPredicate::unconstrained(),
                            crate::effect_row::EffectFormula::empty(),
                        ),
                        parameters: Box::new([unit]),
                        result: unit,
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalDeclarationSource::Binding {
                    identity: [104; 32],
                    declaration: RuntimeLocalBindingDeclaration::new(
                        RuntimeLocalBindingKind::LetBinding,
                        mutable,
                        RuntimeLocalBindingStorage::Derived,
                    ),
                },
                unit,
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    define_declaration_body(&mut builder, local);
    builder
        .push_control_effect_contract(RuntimeControlEffectContractDefinition {
            mode,
            effects: Box::new([RuntimeControlEffectRow {
                kind: RuntimeControlEffectKind::HostOperation,
                identity: None,
                inputs: Box::new([]),
                output: Some(unit),
                cardinality: RuntimeControlEffectCardinality::ExactlyOnce,
                ordering: RuntimeControlEffectOrdering::SingleTerminal,
                cancellation: RuntimeControlEffectCancellation::NotObserved,
                terminal: RuntimeControlEffectTerminalBehavior::Value,
            }]),
            children: Box::new([]),
        })
        .unwrap();
    builder
        .prepare_task_image(RuntimeTaskPlanSealLimits::default())
        .unwrap()
}

pub(super) fn scoped_declaration_candidate() -> UnsealedRuntimePlanImage {
    let mut builder = RuntimePlanBuilder::new();
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let parameter_type = RuntimeSemanticTypeId::from_bytes([111; 32]);
    let result_type = RuntimeSemanticTypeId::from_bytes([112; 32]);
    let function_type = RuntimeSemanticTypeId::from_bytes([113; 32]);
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([114; 32]);
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    parameter_type,
                    RuntimePlanTypeProjection::BoundType(scope.bound_type(0, 0).unwrap()),
                )
                .with_scope(scope),
                RuntimePlanTypeSeed::new(result_type, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    function_type,
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            binder,
                            crate::effect_row::EffectPredicate::unconstrained(),
                            crate::effect_row::EffectFormula::empty(),
                        ),
                        parameters: Box::new([parameter_type]),
                        result: result_type,
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::in_function(
                RuntimeLocalDeclarationSource::Parameter(parameter),
                parameter_type,
                function_type,
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    define_scoped_declaration_body(&mut builder, local);
    builder
        .prepare_task_image(RuntimeTaskPlanSealLimits::default())
        .unwrap()
}

#[test]
fn declaration_frames_rebuild_the_original_executable_local_placement() {
    for candidate in [declaration_candidate, scoped_declaration_candidate] {
        let original = candidate()
            .seal(RuntimeTaskPlanSealLimits::default(), None)
            .unwrap()
            .into_plan();
        assert_eq!(original.local_declarations().len(), 1);
        let encoded = bytes(&original);
        let decoded = decode_image(
            &encoded,
            limits(),
            TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
        let restored = decoded
            .seal(RuntimeTaskPlanSealLimits::default(), None)
            .unwrap()
            .into_plan();
        assert_eq!(restored.type_table(), original.type_table());
        assert_eq!(restored.local_declarations(), original.local_declarations());
        assert_eq!(
            restored.control_effect_contracts(),
            original.control_effect_contracts()
        );
        assert_eq!(
            restored.executable_semantic_digest(),
            original.executable_semantic_digest()
        );
        assert_eq!(bytes(&restored), encoded);
    }
}

#[test]
fn declaration_element_limit_covers_types_and_controls_together() {
    let original = declaration_candidate()
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    let encoded = bytes(&original);
    let program = original
        .encode_program_source(
            limits().program,
            &resources(),
            &mut TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
    let elements = program.elements();
    for (limit_value, accepted) in [(elements, true), (elements - 1, false)] {
        let result = decode_image(
            &encoded,
            RuntimeTaskSourceCodecLimits {
                program: RuntimeProgramSourceLimits {
                    elements: limit_value as u32,
                    ..limits().program
                },
                ..limits()
            },
            TaskSemanticMeter::new(1_000_000, 1_000_000),
        );
        if accepted {
            assert!(result.is_ok());
        } else {
            let error = result.err().unwrap();
            assert_body_literal_limit(error, "source elements");
        }
    }
}

fn declaration_ranges(encoded: &[u8]) -> [std::ops::Range<usize>; 4] {
    let mut offset =
        DOMAIN.len() + 1 + 4 + b"arcweft.runtime.program-source.v1\0".len() + 1 + 15 * 4;
    std::array::from_fn(|_| {
        let length = u32::from_le_bytes(encoded[offset..offset + 4].try_into().unwrap()) as usize;
        let start = offset + 4;
        offset = start + length;
        start..offset
    })
}

#[test]
fn declaration_frames_cannot_retype_admitted_original_expressions() {
    let original = declaration_candidate()
        .seal(RuntimeTaskPlanSealLimits::default(), None)
        .unwrap()
        .into_plan();
    let mut encoded = bytes(&original);
    let [types, _, _, _] = declaration_ranges(&encoded);
    let type_bytes = &encoded[types.clone()];
    let projection = |identity: u8| {
        let matches = type_bytes
            .windows(32)
            .enumerate()
            .filter_map(|(offset, bytes)| (bytes == [identity; 32]).then_some(offset))
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        let identity = types.start + matches[0];
        assert_eq!(encoded[identity + 32], 0);
        let scope_length =
            u32::from_le_bytes(encoded[identity + 33..identity + 37].try_into().unwrap()) as usize;
        identity + 37 + scope_length
    };
    let unit = projection(101);
    let boolean = projection(102);
    assert_ne!(encoded[unit], encoded[boolean]);
    encoded[unit] = encoded[boolean];
    let decoded = decode_image(
        &encoded,
        limits(),
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    );
    let completed = decoded.and_then(|image| {
        image
            .seal(RuntimeTaskPlanSealLimits::default(), None)
            .map_err(RuntimeTaskSourceCodecError::Image)
    });
    assert!(
        matches!(
            completed.err().unwrap(),
            RuntimeTaskSourceCodecError::Program(error) if matches!(*error, RuntimeProgramSourceError::Bodies(_))
        ),
        "a declaration frame must not make an admitted Unit literal inhabit Bool"
    );
}

#[test]
fn program_source_owns_local_and_control_definitions_without_an_external_inventory() {
    for (mutable, mode) in [
        (true, RuntimeTaskControlMode::MustSuspend),
        (false, RuntimeTaskControlMode::MaySuspend),
    ] {
        let other = declaration_candidate_with(mutable, mode)
            .seal(RuntimeTaskPlanSealLimits::default(), None)
            .unwrap()
            .into_plan();
        let encoded = bytes(&other);
        let result = decode_image(
            &encoded,
            limits(),
            TaskSemanticMeter::new(1_000_000, 1_000_000),
        );
        let restored = result
            .unwrap()
            .seal(RuntimeTaskPlanSealLimits::default(), None)
            .unwrap()
            .into_plan();
        assert_eq!(restored.inventory, other.inventory);
        assert_eq!(bytes(&restored), encoded);
    }
}

#[test]
fn every_frame_preflights_before_type_admission_or_key_publication() {
    let (original, _) = published();
    assert_eq!(original.type_table().len(), 2);
    let mut encoded = bytes(&original);
    let [types, nominals, _, controls] = declaration_ranges(&encoded);
    let type_header = b"arcweft.runtime.type-sources.v1\0".len() + 1 + 4;
    let first = types.start + type_header;
    assert_eq!(&encoded[first..first + 4], &0_u32.to_le_bytes());
    assert_eq!(encoded[first + 36], 0); // No nominal declaration.
    let scope_length =
        u32::from_le_bytes(encoded[first + 37..first + 41].try_into().unwrap()) as usize;
    let second = first + 41 + scope_length + 2; // Primitive projection, no data codec.
    assert_eq!(&encoded[second..second + 4], &1_u32.to_le_bytes());
    let identity: [u8; 32] = encoded[first + 4..first + 36].try_into().unwrap();
    encoded[second + 4..second + 36].copy_from_slice(&identity);
    let (bad_type, coordinate) = decode(
        &encoded,
        1,
        limits(),
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    );
    assert!(matches!(
        bad_type.err().unwrap(),
        RuntimeTaskSourceCodecError::Program(error) if matches!(*error,
            RuntimeProgramSourceError::Declarations(crate::plan::declaration_sources::RuntimePlanDeclarationSourceError::Types(
                crate::plan::type_table::source_codec::RuntimeTypeSourceError::Admission(_))))
    ));
    assert!(coordinate.is_none());
    encoded[nominals.start] ^= 1;
    let (bad_nominal, coordinate) = decode(
        &encoded,
        1,
        limits(),
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    );
    assert!(matches!(
        bad_nominal.err().unwrap(),
        RuntimeTaskSourceCodecError::Program(error) if matches!(*error,
            RuntimeProgramSourceError::Declarations(crate::plan::declaration_sources::RuntimePlanDeclarationSourceError::Nominals(
                crate::plan::type_table::source_codec::RuntimeTypeSourceError::Domain)))
    ));
    assert!(coordinate.is_none());
    encoded[nominals.start] ^= 1;
    // The conflicting type is structurally bounded but fails admission. A
    // malformed later control must be found before that admission is attempted.
    encoded[controls.start] ^= 1;
    let (bad_control, coordinate) = decode(
        &encoded,
        1,
        limits(),
        TaskSemanticMeter::new(1_000_000, 1_000_000),
    );
    assert!(matches!(
        bad_control.err().unwrap(),
        RuntimeTaskSourceCodecError::Program(error) if matches!(*error,
            RuntimeProgramSourceError::Declarations(crate::plan::declaration_sources::RuntimePlanDeclarationSourceError::Controls(
                crate::plan::control_effect::source_codec::RuntimeControlSourceError::Domain)))
    ));
    assert!(coordinate.is_none());
}

pub(super) fn define_declaration_body(
    builder: &mut RuntimePlanBuilder,
    local: crate::plan::RuntimeLocalSeedId,
) {
    let unit = RuntimeSemanticTypeId::from_bytes([101; 32]);
    let body = RuntimeExprSeed::new(
        unit,
        RuntimeExprSeedKind::Let {
            binding: local.clone(),
            expr: Box::new(RuntimeExprSeed::new(
                unit,
                RuntimeExprSeedKind::Value(RuntimeValue::Unit),
            )),
            body: Box::new(RuntimeExprSeed::new(
                unit,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    local,
                    RuntimeLocalReadMode::Copy,
                )),
            )),
        },
    );
    builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([105; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [],
            body,
        )
        .unwrap();
}

pub(super) fn define_scoped_declaration_body(
    builder: &mut RuntimePlanBuilder,
    local: crate::plan::RuntimeLocalSeedId,
) {
    let parameter_type = RuntimeSemanticTypeId::from_bytes([111; 32]);
    let result_type = RuntimeSemanticTypeId::from_bytes([112; 32]);
    let function_type = RuntimeSemanticTypeId::from_bytes([113; 32]);
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([114; 32]);
    let function = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([115; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: Some(function_type),
            inputs: Box::new([RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Formal,
                origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: RuntimeFunctionParameterPassing::Affine,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    parameter_type,
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local,
                    },
                ),
                ownership: RuntimeValueOwnershipRequirement::default(),
                unrestricted_bindings: Box::new([]),
            }]),
            result: result_type,
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    builder
        .define_function_site_seed(
            &function,
            RuntimeExprSeed::new(
                result_type,
                RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
            ),
        )
        .unwrap();
}
