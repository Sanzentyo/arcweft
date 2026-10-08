use super::*;
use crate::pattern::{RuntimePatternBindingStep as Step, RuntimeSemanticTypeId};
use crate::plan::{
    RuntimeLocalDeclarationSeed, RuntimeLocalOrigin, RuntimeLocalSeedId, RuntimePatternRestSeed,
    RuntimePatternSeed, RuntimePatternSeedKind as Kind, RuntimePlanBuildError, RuntimePlanBuilder,
    RuntimePlanRecordField, RuntimePlanSequenceKind, RuntimePlanTypeProjection as Type,
    RuntimePlanTypeSeed, RuntimeRecordFieldSeedId, RuntimeRecordPatternFieldSeed,
};

fn semantic(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}

fn fixture(local_types: &[u8]) -> (RuntimePlanBuilder, Vec<RuntimeLocalSeedId>) {
    let mut builder = RuntimePlanBuilder::new();
    let types = [
        (1, Type::Bool),
        (2, Type::Tuple(Box::new([semantic(1), semantic(1)]))),
        (
            3,
            Type::Sequence {
                kind: RuntimePlanSequenceKind::Vec,
                item: semantic(1),
            },
        ),
        (
            4,
            Type::Record(Box::new([
                RuntimePlanRecordField::new("first", semantic(1)),
                RuntimePlanRecordField::new("second", semantic(1)),
            ])),
        ),
        (5, Type::Tuple(Box::new([semantic(1)]))),
        (
            6,
            Type::Option {
                item: semantic(1),
                some_payload: semantic(5),
            },
        ),
    ];
    let admission = builder
        .admit_type_batch(
            types
                .into_iter()
                .map(|(tag, ty)| RuntimePlanTypeSeed::new(semantic(tag), ty)),
            local_types.iter().enumerate().map(|(ordinal, ty)| {
                let name = format!("arcweft.pattern-declarations.fixture.local.{ordinal}");
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalOrigin::Binding(*blake3::hash(name.as_bytes()).as_bytes()),
                    semantic(*ty),
                )
            }),
        )
        .expect("admitted complete pattern fixture");
    (builder, admission.local_ids().to_vec())
}

fn bind(tag: u8, local: &RuntimeLocalSeedId, mutable: bool) -> RuntimePatternSeed {
    RuntimePatternSeed::new(
        semantic(tag),
        Kind::Bind {
            mutable,
            local: local.clone(),
        },
    )
}

#[test]
fn whole_and_tuple_declarations_retain_borrowed_coordinates_types_and_mutability() {
    let (builder, locals) = fixture(&[2, 1, 1]);
    let pattern = builder
        .lower_pattern_seed_for_test(RuntimePatternSeed::new(
            semantic(2),
            Kind::Whole {
                local: locals[0].clone(),
                pattern: Box::new(RuntimePatternSeed::new(
                    semantic(2),
                    Kind::Tuple(Box::new([
                        bind(1, &locals[1], true),
                        RuntimePatternSeed::new(
                            semantic(1),
                            Kind::Typed {
                                local: locals[2].clone(),
                            },
                        ),
                    ])),
                )),
            },
        ))
        .unwrap();
    let declarations = pattern.binding_declarations().collect::<Vec<_>>();
    assert_eq!(declarations.len(), 3);
    assert_eq!(
        declarations
            .iter()
            .map(|d| d.is_mutable())
            .collect::<Vec<_>>(),
        [false, true, false]
    );
    assert_eq!(declarations[0].ty(), pattern.ty());
    assert_ne!(declarations[0].ty(), declarations[1].ty());
    assert_eq!(declarations[1].ty(), declarations[2].ty());
    assert_eq!(declarations[0].coordinate().path().steps(), [Step::Whole]);
    assert_eq!(
        declarations[1].coordinate().path().steps(),
        [Step::TupleElement(0)]
    );
    assert_eq!(
        declarations[2].coordinate().path().steps(),
        [Step::TupleElement(1)]
    );
    let RuntimePatternKind::Whole { binding, .. } = pattern.kind() else {
        panic!("whole pattern")
    };
    assert!(std::ptr::eq(binding, declarations[0].coordinate()));
    assert_eq!(binding.local(), declarations[0].local());
}

#[test]
fn sequence_and_record_rest_declarations_follow_items_and_keep_the_whole_type() {
    for (owner, step) in [(3, Step::SequenceRest), (4, Step::RecordRest)] {
        let (builder, locals) = fixture(&[1, owner]);
        let kind = if owner == 3 {
            Kind::Sequence {
                items: Box::new([bind(1, &locals[0], true)]),
                rest: RuntimePatternRestSeed::Bind(locals[1].clone()),
            }
        } else {
            Kind::Record {
                fields: Box::new([RuntimeRecordPatternFieldSeed::new(
                    RuntimeRecordFieldSeedId::from_zero_based(1),
                    bind(1, &locals[0], true),
                )]),
                rest: RuntimePatternRestSeed::Bind(locals[1].clone()),
            }
        };
        let pattern = builder
            .lower_pattern_seed_for_test(RuntimePatternSeed::new(semantic(owner), kind))
            .unwrap();
        let declarations = pattern.binding_declarations().collect::<Vec<_>>();
        assert_eq!(declarations.len(), 2);
        assert!(declarations[0].is_mutable());
        assert!(!declarations[1].is_mutable());
        assert_eq!(declarations[1].ty(), pattern.ty());
        assert_ne!(declarations[0].ty(), declarations[1].ty());
        assert_eq!(declarations[1].coordinate().path().steps(), [step]);
        assert_eq!(
            declarations[0].coordinate().path().steps(),
            [if owner == 3 {
                Step::SequenceElement(0)
            } else {
                Step::RecordField(0)
            }]
        );
    }
}

#[test]
fn variant_payload_declarations_preserve_the_full_binding_path() {
    let (builder, locals) = fixture(&[1]);
    let pattern = builder
        .lower_pattern_seed_for_test(RuntimePatternSeed::new(
            semantic(6),
            Kind::Variant {
                ordinal: 0,
                payload: Some(Box::new(RuntimePatternSeed::new(
                    semantic(5),
                    Kind::Tuple(Box::new([bind(1, &locals[0], true)])),
                ))),
            },
        ))
        .unwrap();
    let declarations = pattern.binding_declarations().collect::<Vec<_>>();
    assert_eq!(declarations.len(), 1);
    assert!(declarations[0].is_mutable());
    assert_eq!(
        declarations[0].coordinate().path().steps(),
        [Step::VariantPayload, Step::TupleElement(0)]
    );
    assert_ne!(declarations[0].ty(), pattern.ty());
}

#[test]
fn or_alternatives_reject_mutability_mismatch_and_do_not_poison_later_admission() {
    let (builder, locals) = fixture(&[1]);
    for (expected, actual) in [(true, false), (false, true)] {
        assert!(
            matches!(builder.lower_pattern_seed_for_test(RuntimePatternSeed::new(
            semantic(1), Kind::Or(Box::new([
                bind(1, &locals[0], expected), bind(1, &locals[0], actual),
            ])),
        )), Err(RuntimePlanBuildError::OrBindingMutabilityMismatch {
            expected: found_expected, actual: found_actual, ..
        }) if found_expected == expected && found_actual == actual)
        );
    }
    let accepted = builder
        .lower_pattern_seed_for_test(RuntimePatternSeed::new(
            semantic(1),
            Kind::Or(Box::new([
                bind(1, &locals[0], true),
                bind(1, &locals[0], true),
            ])),
        ))
        .unwrap();
    let declarations = accepted.binding_declarations().collect::<Vec<_>>();
    assert_eq!(declarations.len(), 1);
    assert!(declarations[0].is_mutable());
}

#[test]
fn or_inventory_equivalence_is_by_local_declaration_not_binding_position() {
    let (builder, locals) = fixture(&[1, 1]);
    let arm = |reversed| {
        RuntimePatternSeed::new(
            semantic(2),
            Kind::Tuple(if reversed {
                Box::new([bind(1, &locals[1], false), bind(1, &locals[0], true)])
            } else {
                Box::new([bind(1, &locals[0], true), bind(1, &locals[1], false)])
            }),
        )
    };
    let pattern = builder
        .lower_pattern_seed_for_test(RuntimePatternSeed::new(
            semantic(2),
            Kind::Or(Box::new([arm(false), arm(true)])),
        ))
        .unwrap();
    let declarations = pattern.binding_declarations().collect::<Vec<_>>();
    assert_eq!(declarations.len(), 2);
    assert!(declarations[0].is_mutable());
    assert!(!declarations[1].is_mutable());
    assert_eq!(
        declarations[0].coordinate().path().steps(),
        [Step::TupleElement(0)]
    );
    assert_eq!(
        declarations[1].coordinate().path().steps(),
        [Step::TupleElement(1)]
    );
}

#[test]
fn deep_or_and_whole_patterns_keep_declaration_semantics_without_native_recursion() {
    let (builder, locals) = fixture(&[1]);
    let mut seed = bind(1, &locals[0], true);
    for _ in 0..256 {
        seed = RuntimePatternSeed::new(
            semantic(1),
            Kind::Or(Box::new([bind(1, &locals[0], true), seed])),
        );
    }
    let pattern = builder.lower_pattern_seed_for_test(seed).unwrap();
    let declarations = pattern.binding_declarations().collect::<Vec<_>>();
    assert_eq!(declarations.len(), 1);
    assert!(declarations[0].is_mutable());
    assert_eq!(declarations[0].coordinate().path().steps(), [Step::Whole]);

    let (builder, locals) = fixture(&[1; 65]);
    let mut seed = bind(1, &locals[64], true);
    for local in locals[..64].iter().rev() {
        seed = RuntimePatternSeed::new(
            semantic(1),
            Kind::Whole {
                local: local.clone(),
                pattern: Box::new(seed),
            },
        );
    }
    let pattern = builder.lower_pattern_seed_for_test(seed).unwrap();
    let declarations = pattern.binding_declarations().collect::<Vec<_>>();
    assert_eq!(declarations.len(), 65);
    assert!(declarations[..64].iter().all(|d| !d.is_mutable()));
    assert!(declarations[64].is_mutable());
    assert!(declarations.iter().all(|d| d.ty() == pattern.ty()));
    assert!(
        declarations
            .iter()
            .all(|d| d.coordinate().path().steps() == [Step::Whole])
    );
    assert_ne!(declarations[0].local(), declarations[64].local());
}
