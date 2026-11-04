//! Construction uses the same logical root, quotas and original types as owned data.

use super::*;
use crate::{
    awbc::schema::{
        AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape as Shape, AwbcStringId, AwbcTypeId,
        AwbcVariantCase, AwbcVariantIdentity,
    },
    entry::{RuntimeSchemaError, RuntimeSchemaLimits},
    pattern::{RuntimeBuiltinVariantCaseIdentity as Case, RuntimeOpaqueTypeProducerId},
    plan::{RuntimePlanBuilder, RuntimePlanTypeProjection as Type, RuntimePlanTypeSeed},
    task::RuntimeProgramOwner,
    value::{
        AwbcRuntimeValueSnapshot, RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeScalarView,
        RuntimeValue,
    },
};
use std::sync::Arc;

fn semantic(n: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([n; 32])
}

fn owners() -> ([RuntimeProgramOwner; 2], RuntimeOpaqueTypeOwner) {
    let opaque = RuntimeOpaqueTypeOwner::exact(
        RuntimeOpaqueTypeProducerId::try_new("fixture.plain-construction").unwrap(),
        semantic(5),
    );
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(semantic(1), Type::String),
                RuntimePlanTypeSeed::new(semantic(2), Type::Tuple(Box::new([semantic(1)]))),
                RuntimePlanTypeSeed::new(
                    semantic(3),
                    Type::Option {
                        item: semantic(1),
                        some_payload: semantic(2),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    semantic(4),
                    Type::Tuple(Box::new([semantic(3), semantic(3)])),
                ),
                RuntimePlanTypeSeed::new(
                    semantic(5),
                    Type::Opaque {
                        producer: opaque.producer().clone(),
                        admission: opaque.admission(),
                        value_class: opaque.value_class(),
                        persistence: opaque.persistence(),
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(semantic(6), Type::Progress),
            ],
            std::iter::empty::<crate::plan::RuntimeLocalDeclarationSeed>(),
        )
        .unwrap();
    let mut strings = vec![opaque.producer().as_str().to_owned()];
    let mut cases = [Case::OptionNone, Case::OptionSome].map(|case| {
        let (ordinal, schema) = case.owner().resolve_case(case).unwrap();
        let name = AwbcStringId(u32::try_from(strings.len()).unwrap());
        strings.push(schema.name().to_owned());
        (
            ordinal,
            AwbcVariantCase {
                name,
                payload: schema.payload_arity().map(|_| AwbcTypeId(1)),
            },
        )
    });
    cases.sort_by_key(|(ordinal, _)| *ordinal);
    let awbc = AwbcProgram {
        strings,
        runtime_types: vec![
            AwbcRuntimeType::new(semantic(1), Shape::String),
            AwbcRuntimeType::new(semantic(2), Shape::Tuple(vec![AwbcTypeId(0)])),
            AwbcRuntimeType::new(
                semantic(3),
                Shape::Variant {
                    owner: AwbcVariantIdentity::Builtin(Case::OptionNone.owner()),
                    arguments: vec![],
                    cases: cases.into_iter().map(|(_, case)| case).collect(),
                },
            ),
            AwbcRuntimeType::new(
                semantic(4),
                Shape::Tuple(vec![AwbcTypeId(2), AwbcTypeId(2)]),
            ),
            AwbcRuntimeType::new(
                semantic(5),
                Shape::Opaque {
                    producer: AwbcStringId(0),
                    admission: opaque.admission(),
                    value_class: opaque.value_class(),
                    persistence: opaque.persistence(),
                    arguments: vec![],
                },
            ),
            AwbcRuntimeType::new(semantic(6), Shape::Progress),
        ],
        ..AwbcProgram::default()
    };
    (
        [
            RuntimeProgramOwner::Plan(Arc::new(builder.finish().unwrap())),
            RuntimeProgramOwner::Awbc(Arc::new(awbc)),
        ],
        opaque,
    )
}

fn reference(owner: &RuntimeOpaqueTypeOwner) -> RuntimeValue {
    owner
        .try_wrap(RuntimeValue::Tuple(vec![
            RuntimeValue::option_some(RuntimeValue::String("あβ".to_owned())),
            RuntimeValue::option_none(),
        ]))
        .unwrap()
}

#[test]
fn prospective_options_and_opaque_round_trip_with_native_and_awbc_original_types() {
    let (programs, owner) = owners();
    let text = RuntimePlainValueRecipe::from("あβ");
    let fields = [
        RuntimePlainValueRecipe::some(&text),
        RuntimePlainValueRecipe::none(),
    ];
    let recipe = RuntimePlainValueRecipe::tuple(&fields);
    for program in programs {
        let value = RuntimeRetainedValueAdmission::for_program(
            program.clone(),
            RuntimeSchemaLimits::engine_default(),
        )
        .construct_plain_opaque(&owner, semantic(4), &recipe)
        .unwrap();
        assert_eq!(value, reference(&owner));
        let image =
            AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &program).unwrap();
        let encoded = serde_json::to_vec(&image).unwrap();
        let decoded: AwbcRuntimeValueSnapshot = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(
            decoded.into_runtime_value_for_program(&program).unwrap(),
            value
        );
    }
}

fn equivalent_boundary(
    program: &RuntimeProgramOwner,
    owner: &RuntimeOpaqueTypeOwner,
    recipe: &RuntimePlainValueRecipe<'_>,
    configure: impl Fn(&mut RuntimeSchemaLimits, u32),
    budget: &'static str,
) {
    let expected = reference(owner);
    let exact = (0..1024)
        .find(|n| {
            let mut limits = RuntimeSchemaLimits::engine_default();
            configure(&mut limits, *n);
            RuntimeRetainedValueAdmission::for_program(program.clone(), limits)
                .validate(&expected)
                .is_ok()
        })
        .expect("bounded reference fixture has an exact resource threshold");
    let mut limits = RuntimeSchemaLimits::engine_default();
    configure(&mut limits, exact);
    assert_eq!(
        RuntimeRetainedValueAdmission::for_program(program.clone(), limits)
            .construct_plain_opaque(owner, semantic(4), recipe)
            .unwrap(),
        expected
    );
    configure(&mut limits, exact.checked_sub(1).unwrap());
    let mut admission = RuntimeRetainedValueAdmission::for_program(program.clone(), limits);
    let error =
        AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded { budget });
    assert_eq!(
        admission
            .construct_plain_opaque(owner, semantic(4), recipe)
            .map_err(|error| *error),
        Err(error.clone())
    );
    assert_eq!(admission.validate_metadata(&0_u8), Err(error));
}

#[test]
fn prospective_envelope_and_option_overhead_match_owned_node_and_byte_limits() {
    let (programs, owner) = owners();
    let text = RuntimePlainValueRecipe::from("あβ");
    let fields = [
        RuntimePlainValueRecipe::some(&text),
        RuntimePlainValueRecipe::none(),
    ];
    let recipe = RuntimePlainValueRecipe::tuple(&fields);
    for program in programs {
        equivalent_boundary(
            &program,
            &owner,
            &recipe,
            |limits, n| limits.max_nodes = n,
            "nodes",
        );
        equivalent_boundary(
            &program,
            &owner,
            &recipe,
            |limits, n| limits.max_encoded_bytes = u64::from(n),
            "encoded_bytes",
        );
        equivalent_boundary(
            &program,
            &owner,
            &recipe,
            |limits, n| limits.max_string_bytes = u64::from(n),
            "string_bytes",
        );
    }
}

#[test]
fn prospective_type_and_opaque_class_rejection_keep_the_original_bank_failed() {
    let (programs, owner) = owners();
    let wrong = RuntimePlainValueRecipe::from("wrong shape");
    let text = RuntimePlainValueRecipe::from("あβ");
    let fields = [
        RuntimePlainValueRecipe::some(&text),
        RuntimePlainValueRecipe::none(),
    ];
    let valid = RuntimePlainValueRecipe::tuple(&fields);
    for program in programs {
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            program.clone(),
            RuntimeSchemaLimits::engine_default(),
        );
        let error = admission
            .construct_plain_opaque(&owner, semantic(4), &wrong)
            .unwrap_err();
        assert!(matches!(
            *error,
            AwbcRuntimeValueSnapshotError::ProgramType(_)
        ));
        assert_eq!(
            admission.construct_plain_opaque(&owner, semantic(4), &valid),
            Err(error)
        );
        let affine = RuntimeOpaqueTypeOwner::exact_with(
            owner.producer().clone(),
            owner.semantic_identity(),
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::Cue),
            RuntimeOpaquePersistence::SnapshotOnly,
        );
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            program,
            RuntimeSchemaLimits::engine_default(),
        );
        assert_eq!(
            admission.construct_plain_opaque(&affine, semantic(4), &valid),
            Err(Box::new(AwbcRuntimeValueSnapshotError::AffineValue))
        );
        assert_eq!(
            admission.construct_plain_opaque(&owner, semantic(4), &valid),
            Err(Box::new(AwbcRuntimeValueSnapshotError::AffineValue))
        );
    }
}

#[test]
fn prospective_cycle_is_bounded_before_type_lookup_or_materialization() {
    static CYCLE: RuntimePlainValueRecipe<'static> = RuntimePlainValueRecipe::some(&CYCLE);
    let mut admission = RuntimeRetainedValueAdmission::standalone(RuntimeSchemaLimits {
        max_depth: 5,
        ..RuntimeSchemaLimits::engine_default()
    });
    let error = admission
        .construct_plain_value(&CYCLE, semantic(3), None)
        .unwrap_err();
    assert_eq!(
        *error,
        AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
            budget: "depth"
        })
    );
    assert_eq!(admission.validate_metadata(&0_u8), Err(*error));
}

#[test]
fn prospective_progress_reuses_its_owning_ratio_validation() {
    for ratio in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(matches!(
            RuntimePlainValueRecipe::try_from(RuntimeScalarView::Progress { ratio, label: None }),
            Err(arcweft_need::ProgressError)
        ));
    }
    let (programs, _) = owners();
    for program in programs {
        for ratio in [0.0, 0.5, 1.0] {
            let recipe = RuntimePlainValueRecipe::try_from(RuntimeScalarView::Progress {
                ratio,
                label: Some("あβ"),
            })
            .unwrap();
            let value = RuntimeRetainedValueAdmission::for_program(
                program.clone(),
                RuntimeSchemaLimits::engine_default(),
            )
            .construct_plain_value(&recipe, semantic(6), None)
            .unwrap();
            assert_eq!(
                value,
                RuntimeValue::Progress(
                    crate::value::Progress::new(ratio)
                        .unwrap()
                        .with_label("あβ")
                )
            );
        }
    }
}
