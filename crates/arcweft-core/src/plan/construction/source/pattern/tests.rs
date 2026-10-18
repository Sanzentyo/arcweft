use super::*;

mod admission;
use crate::entry::schema::RuntimeSchemaLimits;
use crate::pattern::{RuntimeSemanticTypeId, match_runtime_pattern};
use crate::plan::declaration_sources::{
    RuntimePlanDeclarationSourceBytes, RuntimePlanDeclarationSourceLimits,
};
use crate::plan::type_table::source_codec::RuntimeTypeSourceLimits;
use crate::plan::{
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind, RuntimeLocalBindingStorage,
    RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource, RuntimePatternRestSeed,
    RuntimePatternSeed, RuntimePatternSeedKind as K, RuntimePlan, RuntimePlanBuilder,
    RuntimePlanRecordField, RuntimePlanSequenceKind, RuntimePlanTypeProjection as P,
    RuntimePlanTypeSeed, RuntimeRecordFieldSeedId, RuntimeRecordPatternFieldSeed,
    RuntimeTaskPlanSealLimits,
};
use crate::value::{RuntimeEntityReference, RuntimeValue};

const BOOL: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([31; 32]);
const PAIR: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([32; 32]);
const RECORD: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([33; 32]);
const SEQUENCE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([34; 32]);
const PAYLOAD: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([35; 32]);
const OPTION: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([36; 32]);
const ENTITY: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([37; 32]);

pub(in crate::plan::construction::source) fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(100_000_000, 100_000_000)
}
pub(in crate::plan::construction::source) fn resources() -> ValueResourceAdmission {
    ValueResourceAdmission::live(RuntimeSchemaLimits::engine_default())
}
fn limits() -> RuntimePatternSourceLimits {
    RuntimePatternSourceLimits {
        nodes: 10_000,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: 7,
        local_count: 4,
    }
}
fn seed(ty: RuntimeSemanticTypeId, kind: K) -> RuntimePatternSeed {
    RuntimePatternSeed::new(ty, kind)
}
fn boolean(value: bool) -> RuntimePatternSeed {
    seed(BOOL, K::Literal(RuntimeValue::Bool(value)))
}

fn fixture() -> (RuntimePlanBuilder, Vec<crate::plan::RuntimeLocalSeedId>) {
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(BOOL, P::Bool),
                RuntimePlanTypeSeed::new(PAIR, P::Tuple(Box::new([BOOL, BOOL]))),
                RuntimePlanTypeSeed::new(
                    RECORD,
                    P::Record(Box::new([
                        RuntimePlanRecordField::new("a", BOOL),
                        RuntimePlanRecordField::new("b", BOOL),
                    ])),
                ),
                RuntimePlanTypeSeed::new(
                    SEQUENCE,
                    P::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: BOOL,
                    },
                ),
                RuntimePlanTypeSeed::new(PAYLOAD, P::Tuple(Box::new([BOOL]))),
                RuntimePlanTypeSeed::new(
                    OPTION,
                    P::Option {
                        item: BOOL,
                        some_payload: PAYLOAD,
                    },
                ),
                RuntimePlanTypeSeed::new(ENTITY, P::EntityReference),
            ],
            [BOOL, BOOL, RECORD, SEQUENCE]
                .into_iter()
                .enumerate()
                .map(|(index, ty)| {
                    RuntimeLocalDeclarationSeed::new(
                        RuntimeLocalDeclarationSource::Binding {
                            identity: [u8::try_from(index + 50).unwrap(); 32],
                            declaration: RuntimeLocalBindingDeclaration::new(
                                RuntimeLocalBindingKind::PatternBinding,
                                false,
                                RuntimeLocalBindingStorage::Derived,
                            ),
                        },
                        ty,
                    )
                }),
        )
        .unwrap();
    let locals = admitted.local_ids().to_vec();
    builder.declare_test_input_locals(&locals).unwrap();
    (builder, locals)
}

pub(in crate::plan::construction::source) fn frames(plan: &RuntimePlan) -> [Vec<u8>; 4] {
    let mut bank = meter();
    let type_limits = RuntimeTypeSourceLimits {
        rows: 100,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        policy_depth: 128,
    };
    let types = plan
        .type_table()
        .prepare_sources_encoding(type_limits, &mut bank)
        .unwrap();
    let nominals = plan
        .inventory
        .prepare_nominal_sources_encoding(type_limits, &mut bank)
        .unwrap();
    let locals = plan
        .local_declarations()
        .prepare_sources_encoding(
            crate::plan::local_declarations::source_codec::RuntimeLocalSourceLimits {
                rows: 100,
                encoded_bytes: 1 << 22,
                decoded_bytes: 1 << 24,
                type_count: plan.type_table().len(),
            },
            &mut bank,
        )
        .unwrap();
    let controls = plan
        .control_effect_contracts()
        .prepare_sources_encoding(
            crate::plan::control_effect::source_codec::RuntimeControlSourceLimits {
                rows: 100,
                elements: 100_000,
                encoded_bytes: 1 << 22,
                decoded_bytes: 1 << 24,
                type_count: plan.type_table().len(),
            },
            &mut bank,
        )
        .unwrap();
    let mut result = [
        Vec::with_capacity(types.encoded_bytes()),
        Vec::with_capacity(nominals.encoded_bytes()),
        Vec::with_capacity(locals.encoded_bytes()),
        Vec::with_capacity(controls.encoded_bytes()),
    ];
    types.append_to(&mut result[0]).unwrap();
    nominals.append_to(&mut result[1]).unwrap();
    locals.append_to(&mut result[2]).unwrap();
    controls.append_to(&mut result[3]).unwrap();
    result
}

pub(in crate::plan::construction::source) fn declarations<'a>(
    frames: &'a [Vec<u8>; 4],
    bank: &mut TaskSemanticMeter,
) -> crate::plan::declaration_sources::PreparedRuntimePlanDeclarationSources<'a> {
    RuntimePlanDeclarationSourceBytes {
        types: &frames[0],
        nominals: &frames[1],
        locals: &frames[2],
        controls: &frames[3],
    }
    .preflight(
        RuntimePlanDeclarationSourceLimits {
            types: 100,
            nominals: 100,
            locals: 100,
            controls: 100,
            elements: 100_000,
            encoded_bytes: 1 << 22,
            decoded_bytes: 1 << 24,
            policy_depth: 128,
        },
        bank,
    )
    .unwrap()
}

fn encode(pattern: &RuntimePattern) -> Vec<u8> {
    let prepared = pattern
        .prepare_sources_encoding(limits(), &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut output).unwrap();
    output
}

fn roundtrip(build: impl FnOnce(&[crate::plan::RuntimeLocalSeedId]) -> RuntimePatternSeed) {
    let (builder, locals) = fixture();
    let original = builder.lower_pattern_seed_for_test(build(&locals)).unwrap();
    let plan = builder.finish().unwrap();
    let bytes = encode(&original);
    let declarations_bytes = frames(&plan);
    let mut bank = meter();
    let declarations = declarations(&declarations_bytes, &mut bank);
    let mut values = resources();
    let prepared = RuntimePattern::preflight_sources(&bytes, limits(), &values, &mut bank).unwrap();
    let encoded = original
        .prepare_sources_encoding(limits(), &values, &mut meter())
        .unwrap();
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    assert_eq!(prepared.elements(), encoded.elements());
    assert!(prepared.node_count() > 0);
    let mut source = declarations.into_construction(bank).unwrap();
    let decoded = prepared.materialize(&mut source, &mut values).unwrap();
    let locals = source.locals.to_vec();
    source.builder.declare_test_input_locals(&locals).unwrap();
    let restored = source.builder.lower_pattern_seed_for_test(decoded).unwrap();
    assert_eq!(restored, original);
    assert_eq!(encode(&restored), bytes);
    // Both paths publish through the real executable seal, after the same ingress.
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored_plan = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(
        restored_plan.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
}

#[test]
fn original_pattern_sources_cover_every_kind_and_preserve_coordinates_order_and_seal() {
    roundtrip(|locals| {
        seed(
            BOOL,
            K::Bind {
                mutable: true,
                local: locals[0].clone(),
            },
        )
    });
    roundtrip(|_| seed(BOOL, K::Discard));
    roundtrip(|_| boolean(true));
    roundtrip(|_| {
        seed(
            ENTITY,
            K::Entity(RuntimeEntityReference::StructuralFlow(
                crate::plan::FlowRuntimeId::from_runtime_contract("main.child", "公開名").unwrap(),
            )),
        )
    });
    roundtrip(|locals| {
        seed(
            PAIR,
            K::Tuple(Box::new([
                seed(
                    BOOL,
                    K::Typed {
                        local: locals[1].clone(),
                    },
                ),
                seed(
                    BOOL,
                    K::Whole {
                        local: locals[0].clone(),
                        pattern: Box::new(boolean(false)),
                    },
                ),
            ])),
        )
    });
    roundtrip(|locals| {
        seed(
            BOOL,
            K::Or(Box::new([
                seed(
                    BOOL,
                    K::Whole {
                        local: locals[0].clone(),
                        pattern: Box::new(boolean(false)),
                    },
                ),
                seed(
                    BOOL,
                    K::Whole {
                        local: locals[0].clone(),
                        pattern: Box::new(boolean(true)),
                    },
                ),
            ])),
        )
    });
    for mode in 0..3 {
        roundtrip(|locals| {
            seed(
                RECORD,
                K::Record {
                    // Retain authored order, which differs from field identity order.
                    fields: vec![1, 0]
                        .into_iter()
                        .take(if mode == 0 { 2 } else { 1 })
                        .map(|index| {
                            RuntimeRecordPatternFieldSeed::new(
                                RuntimeRecordFieldSeedId::from_zero_based(index),
                                boolean(index == 0),
                            )
                        })
                        .collect(),
                    rest: match mode {
                        0 => RuntimePatternRestSeed::Exact,
                        1 => RuntimePatternRestSeed::Ignore,
                        _ => RuntimePatternRestSeed::Bind(locals[2].clone()),
                    },
                },
            )
        });
        roundtrip(|locals| {
            seed(
                SEQUENCE,
                K::Sequence {
                    items: Box::new([boolean(true), boolean(false)]),
                    rest: match mode {
                        0 => RuntimePatternRestSeed::Exact,
                        1 => RuntimePatternRestSeed::Ignore,
                        _ => RuntimePatternRestSeed::Bind(locals[3].clone()),
                    },
                },
            )
        });
    }
    roundtrip(|_| {
        seed(
            OPTION,
            K::Variant {
                ordinal: crate::pattern::RuntimeBuiltinVariantIdentity::Option
                    .resolve_case(crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionNone)
                    .unwrap()
                    .0,
                payload: None,
            },
        )
    });
    roundtrip(|_| {
        seed(
            OPTION,
            K::Variant {
                ordinal: crate::pattern::RuntimeBuiltinVariantIdentity::Option
                    .resolve_case(crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome)
                    .unwrap()
                    .0,
                payload: Some(Box::new(seed(PAYLOAD, K::Tuple(Box::new([boolean(true)]))))),
            },
        )
    });
}

#[test]
fn source_patterns_keep_deep_or_without_consuming_structural_binding_depth() {
    roundtrip(|locals| {
        let binding = || {
            seed(
                BOOL,
                K::Bind {
                    mutable: false,
                    local: locals[0].clone(),
                },
            )
        };
        (0..128).fold(binding(), |nested, _| {
            seed(BOOL, K::Or(Box::new([binding(), nested])))
        })
    });
    let (builder, _) = fixture();
    let pattern = builder
        .lower_pattern_seed_for_test(seed(BOOL, K::Or(Box::new([boolean(false), boolean(true)]))))
        .unwrap();
    let bytes = encode(&pattern);
    let plan = builder.finish().unwrap();
    let declaration_bytes = frames(&plan);
    let mut bank = meter();
    let declarations = declarations(&declaration_bytes, &mut bank);
    let mut values = resources();
    let prepared = RuntimePattern::preflight_sources(&bytes, limits(), &values, &mut bank).unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let decoded = prepared.materialize(&mut source, &mut values).unwrap();
    source
        .builder
        .declare_test_input_locals(&source.locals)
        .unwrap();
    let decoded = source.builder.lower_pattern_seed_for_test(decoded).unwrap();
    let plan = source.builder.finish().unwrap();
    for value in [false, true] {
        assert!(
            match_runtime_pattern(&plan, &decoded, &RuntimeValue::Bool(value))
                .unwrap()
                .is_some()
        );
    }
}

#[test]
fn pattern_source_exact_limits_and_reserved_output_are_enforced_before_retention() {
    let (builder, locals) = fixture();
    let pattern = builder
        .lower_pattern_seed_for_test(seed(
            PAIR,
            K::Tuple(Box::new([
                seed(
                    BOOL,
                    K::Bind {
                        mutable: false,
                        local: locals[0].clone(),
                    },
                ),
                boolean(true),
            ])),
        ))
        .unwrap();
    let bytes = encode(&pattern);
    let prepared =
        RuntimePattern::preflight_sources(&bytes, limits(), &resources(), &mut meter()).unwrap();
    let exact = RuntimePatternSourceLimits {
        nodes: prepared.node_count() as u32,
        elements: prepared.elements() as u32,
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        ..limits()
    };
    RuntimePattern::preflight_sources(&bytes, exact, &resources(), &mut meter()).unwrap();
    for bounded in [
        RuntimePatternSourceLimits {
            nodes: exact.nodes - 1,
            ..exact
        },
        RuntimePatternSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimePatternSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimePatternSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = meter();
        assert!(
            RuntimePattern::preflight_sources(&bytes, bounded, &resources(), &mut bank).is_err()
        );
        assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
    let encoded = pattern
        .prepare_sources_encoding(exact, &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(encoded.encoded_bytes() - 1);
    output.push(81);
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(RuntimePatternSourceError::OutputCapacity { .. })
    ));
    assert_eq!(output, [81]);
}

#[test]
fn pattern_source_rejects_malformed_frames_and_keeps_original_work_failure() {
    let (builder, _) = fixture();
    let pattern = builder.lower_pattern_seed_for_test(boolean(true)).unwrap();
    let bytes = encode(&pattern);
    let mut bad_frames = Vec::new();
    for (offset, value) in [(0, 0), (DOMAIN.len(), 2), (DOMAIN.len() + 5, 255)] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        bad_frames.push(bad);
    }
    let mut zero_type = bytes.clone();
    zero_type[DOMAIN.len() + 6..DOMAIN.len() + 10].fill(0);
    bad_frames.push(zero_type);
    let mut huge = bytes.clone();
    huge[DOMAIN.len() + 1..DOMAIN.len() + 5].copy_from_slice(&u32::MAX.to_le_bytes());
    bad_frames.push(huge);
    let mut trailing = bytes.clone();
    trailing.push(0);
    bad_frames.push(trailing);
    for end in 0..bytes.len() {
        bad_frames.push(bytes[..end].to_vec());
    }
    for bad in bad_frames {
        let mut bank = meter();
        assert!(
            RuntimePattern::preflight_sources(&bad, limits(), &resources(), &mut bank).is_err()
        );
        assert!(bank.status().is_err());
        assert!(matches!(
            RuntimePattern::preflight_sources(&bytes, limits(), &resources(), &mut bank),
            Err(RuntimePatternSourceError::Resource(_))
        ));
    }
    let mut exhausted = TaskSemanticMeter::new(0, 0);
    let first = RuntimePattern::preflight_sources(&bytes, limits(), &resources(), &mut exhausted)
        .err()
        .unwrap();
    let second = RuntimePattern::preflight_sources(&bytes, limits(), &resources(), &mut exhausted)
        .err()
        .unwrap();
    assert_eq!(first, second);
}

#[test]
fn pattern_source_literals_share_original_value_budget_across_roots_and_poison_source_owner() {
    let (builder, _) = fixture();
    let pattern = builder.lower_pattern_seed_for_test(boolean(true)).unwrap();
    let bytes = encode(&pattern);
    let plan = builder.finish().unwrap();
    let declaration_bytes = frames(&plan);
    let mut bank = meter();
    let declarations = declarations(&declaration_bytes, &mut bank);
    let mut values = ValueResourceAdmission::live(RuntimeSchemaLimits {
        max_nodes: 1,
        ..RuntimeSchemaLimits::engine_default()
    });
    let first = RuntimePattern::preflight_sources(&bytes, limits(), &values, &mut bank).unwrap();
    let second = RuntimePattern::preflight_sources(&bytes, limits(), &values, &mut bank).unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    first.materialize(&mut source, &mut values).unwrap();
    assert!(second.materialize(&mut source, &mut values).is_err());
    assert!(source.meter.status().is_err());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}
