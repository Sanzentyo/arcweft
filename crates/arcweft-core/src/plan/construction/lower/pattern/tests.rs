use crate::pattern::{RuntimePattern, RuntimeSemanticTypeId, match_runtime_pattern};
use crate::plan::{
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind, RuntimeLocalBindingStorage,
    RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource, RuntimeLocalSeedId,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlanBuildError, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed,
};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::value::RuntimeValue;

const BOOLEAN: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([71; 32]);
const TRIPLE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([72; 32]);
const ROOT: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([73; 32]);

fn fixture(prefix: usize) -> (RuntimePlanBuilder, Box<[RuntimeLocalSeedId]>) {
    let mut builder = RuntimePlanBuilder::new();
    let mut root = vec![BOOLEAN; prefix];
    root.push(TRIPLE);
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(BOOLEAN, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    TRIPLE,
                    RuntimePlanTypeProjection::Tuple(Box::new([BOOLEAN; 3])),
                ),
                RuntimePlanTypeSeed::new(
                    ROOT,
                    RuntimePlanTypeProjection::Tuple(root.into_boxed_slice()),
                ),
            ],
            (0..prefix + 3).map(|index| {
                let mut identity = [87; 32];
                identity[..4].copy_from_slice(&u32::try_from(index).unwrap().to_le_bytes());
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Binding {
                        identity,
                        declaration: RuntimeLocalBindingDeclaration::new(
                            RuntimeLocalBindingKind::PatternBinding,
                            false,
                            RuntimeLocalBindingStorage::Derived,
                        ),
                    },
                    BOOLEAN,
                )
            }),
        )
        .unwrap();
    let locals = admitted.local_ids().to_vec().into_boxed_slice();
    // These standalone matcher inputs have real, explicit callable ingress.
    builder.declare_test_input_locals(&locals).unwrap();
    (builder, locals)
}

fn binding(local: &RuntimeLocalSeedId, mutable: bool) -> RuntimePatternSeed {
    RuntimePatternSeed::new(
        BOOLEAN,
        RuntimePatternSeedKind::Bind {
            local: local.clone(),
            mutable,
        },
    )
}

fn alternative(
    first: &RuntimeLocalSeedId,
    second: &RuntimeLocalSeedId,
    mutable: bool,
    discriminator: bool,
) -> RuntimePatternSeed {
    RuntimePatternSeed::new(
        TRIPLE,
        RuntimePatternSeedKind::Tuple(Box::new([
            binding(first, mutable),
            binding(second, mutable),
            RuntimePatternSeed::new(
                BOOLEAN,
                RuntimePatternSeedKind::Literal(RuntimeValue::Bool(discriminator)),
            ),
        ])),
    )
}

fn with_prefix(
    locals: &[RuntimeLocalSeedId],
    prefix: usize,
    tail: RuntimePatternSeed,
) -> RuntimePatternSeed {
    let mut fields = locals[..prefix]
        .iter()
        .map(|local| binding(local, false))
        .collect::<Vec<_>>();
    fields.push(tail);
    RuntimePatternSeed::new(
        ROOT,
        RuntimePatternSeedKind::Tuple(fields.into_boxed_slice()),
    )
}

fn nested_or(locals: &[RuntimeLocalSeedId], prefix: usize, depth: usize) -> RuntimePatternSeed {
    let first = &locals[prefix];
    let second = &locals[prefix + 1];
    let mut pattern = alternative(first, second, false, true);
    for _ in 0..depth {
        pattern = RuntimePatternSeed::new(
            TRIPLE,
            RuntimePatternSeedKind::Or(Box::new([
                pattern,
                alternative(second, first, false, false),
            ])),
        );
    }
    with_prefix(locals, prefix, pattern)
}

fn id(builder: &RuntimePlanBuilder, local: &RuntimeLocalSeedId) -> RuntimeLocalDeclarationId {
    local.resolve(&builder.issuer).unwrap().0
}

fn matched_values(
    builder: RuntimePlanBuilder,
    pattern: &RuntimePattern,
    prefix: usize,
) -> Vec<crate::value::RuntimeLocalBinding> {
    let plan = builder.finish().unwrap();
    let mut fields = vec![RuntimeValue::Bool(true); prefix];
    fields.push(RuntimeValue::Tuple(vec![
        RuntimeValue::Bool(true),
        RuntimeValue::Bool(false),
        RuntimeValue::Bool(false),
    ]));
    match_runtime_pattern(&plan, pattern, &RuntimeValue::Tuple(fields))
        .unwrap()
        .unwrap()
}

#[test]
fn or_admission_keeps_prefix_and_reordered_bindings_through_actual_matching() {
    let (builder, locals) = fixture(2);
    let pattern = builder
        .root_body_context()
        .lower_pattern_seed(nested_or(&locals, 2, 3))
        .unwrap();
    let first = id(&builder, &locals[2]);
    let second = id(&builder, &locals[3]);
    let prefix = [id(&builder, &locals[0]), id(&builder, &locals[1])];
    let bindings = matched_values(builder, &pattern, 2);
    assert_eq!(bindings.len(), 4);
    for (local, expected) in [
        (prefix[0], true),
        (prefix[1], true),
        (first, false),
        (second, true),
    ] {
        assert_eq!(
            bindings
                .iter()
                .find(|binding| binding.local == local)
                .unwrap()
                .value,
            RuntimeValue::Bool(expected)
        );
    }
    assert_eq!(pattern.binding_declarations().count(), 4);
}

#[test]
fn deep_or_scopes_do_not_accumulate_or_drop_surrounding_bindings() {
    const PREFIX: usize = 128;
    const DEPTH: usize = 128;
    let (builder, locals) = fixture(PREFIX);
    let pattern = builder
        .root_body_context()
        .lower_pattern_seed(nested_or(&locals, PREFIX, DEPTH))
        .unwrap();
    assert_eq!(pattern.binding_declarations().count(), PREFIX + 2);
    let bindings = matched_values(builder, &pattern, PREFIX);
    assert_eq!(bindings.len(), PREFIX + 2);
    let distinct = bindings
        .iter()
        .map(|binding| binding.local)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(distinct.len(), PREFIX + 2);
}

#[test]
fn or_checkpoint_preserves_duplicate_rejection_before_and_after_the_or() {
    let (mut builder, locals) = fixture(1);
    let before = with_prefix(
        &locals,
        1,
        RuntimePatternSeed::new(
            TRIPLE,
            RuntimePatternSeedKind::Or(Box::new([
                alternative(&locals[0], &locals[2], false, true),
                alternative(&locals[0], &locals[2], false, false),
            ])),
        ),
    );
    assert!(
        matches!(builder.root_body_context().lower_pattern_seed(before),
        Err(RuntimePlanBuildError::DuplicatePatternBinding { local }) if local == id(&builder, &locals[0]))
    );
    let reverse_root = RuntimeSemanticTypeId::from_bytes([74; 32]);
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                reverse_root,
                RuntimePlanTypeProjection::Tuple(Box::new([TRIPLE, BOOLEAN])),
            )],
            [],
        )
        .unwrap();
    let after = RuntimePatternSeed::new(
        reverse_root,
        RuntimePatternSeedKind::Tuple(Box::new([
            RuntimePatternSeed::new(
                TRIPLE,
                RuntimePatternSeedKind::Or(Box::new([
                    alternative(&locals[1], &locals[2], false, true),
                    alternative(&locals[2], &locals[1], false, false),
                ])),
            ),
            binding(&locals[1], false),
        ])),
    );
    assert!(
        matches!(builder.root_body_context().lower_pattern_seed(after),
        Err(RuntimePlanBuildError::DuplicatePatternBinding { local }) if local == id(&builder, &locals[1]))
    );
}

#[test]
fn or_alternatives_preserve_inventory_precedence_and_lowest_mutability_diagnostic() {
    let (builder, locals) = fixture(1);
    for (second, inventory) in [
        (alternative(&locals[2], &locals[3], true, false), true),
        (alternative(&locals[2], &locals[1], true, false), false),
    ] {
        // The first alternative deliberately introduces B before A. The
        // diagnostic still selects the lowest local identity after inventory.
        let pattern = with_prefix(
            &locals,
            1,
            RuntimePatternSeed::new(
                TRIPLE,
                RuntimePatternSeedKind::Or(Box::new([
                    alternative(&locals[2], &locals[1], false, true),
                    second,
                ])),
            ),
        );
        let error = builder
            .root_body_context()
            .lower_pattern_seed(pattern)
            .unwrap_err();
        if inventory {
            assert!(
                matches!(error, RuntimePlanBuildError::InvalidTypeProjection {
                context: "Or binding inventory", ty,
            } if ty == builder.types.id_for_semantic(TRIPLE).unwrap())
            );
        } else {
            assert!(
                matches!(error, RuntimePlanBuildError::OrBindingMutabilityMismatch {
                local, expected: false, actual: true,
            } if local == id(&builder, &locals[1]))
            );
        }
    }
}
