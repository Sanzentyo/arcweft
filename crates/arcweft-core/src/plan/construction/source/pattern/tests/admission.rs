use super::*;

fn raw_rows(rows: &[(Tag, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = DOMAIN.to_vec();
    bytes.push(1);
    bytes.extend_from_slice(&u32::try_from(rows.len()).unwrap().to_le_bytes());
    for (tag, fields) in rows {
        bytes.push(*tag as u8);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(fields);
    }
    bytes
}

#[test]
fn pattern_source_preflight_counts_derived_paths_and_has_no_discard_depth_restriction() {
    for depth in [64, 65, 200] {
        let mut rows = (0..depth)
            .map(|_| (Tag::Tuple, 1u32.to_le_bytes().to_vec()))
            .collect::<Vec<_>>();
        rows.push((Tag::Discard, Vec::new()));
        RuntimePattern::preflight_sources(&raw_rows(&rows), limits(), &resources(), &mut meter())
            .unwrap();
        *rows.last_mut().unwrap() = (Tag::Bind, vec![0, 1, 0, 0, 0]);
        let bytes = raw_rows(&rows);
        let result =
            RuntimePattern::preflight_sources(&bytes, limits(), &resources(), &mut meter());
        if depth == 64 {
            result.unwrap();
        } else {
            assert!(matches!(result, Err(RuntimePatternSourceError::Limit {
                kind: "binding depth", actual, maximum: 64,
            }) if actual == depth));
        }
        *rows.last_mut().unwrap() = (Tag::Sequence, vec![0, 0, 0, 0, 2, 1, 0, 0, 0]);
        assert!(matches!(
            RuntimePattern::preflight_sources(
                &raw_rows(&rows),
                limits(),
                &resources(),
                &mut meter()
            ),
            Err(RuntimePatternSourceError::Limit {
                kind: "binding depth",
                ..
            })
        ));
    }
}

#[test]
fn pattern_source_framing_rejects_bad_arity_roots_missing_children_flags_and_references() {
    let rows = [
        vec![
            (Tag::Or, 1u32.to_le_bytes().to_vec()),
            (Tag::Discard, vec![]),
        ],
        vec![(Tag::Discard, vec![]), (Tag::Discard, vec![])],
        vec![(Tag::Tuple, 1u32.to_le_bytes().to_vec())],
        vec![(Tag::Bind, vec![2, 1, 0, 0, 0])],
        vec![(Tag::Bind, vec![0, 0, 0, 0, 0])],
        vec![(Tag::Typed, 5u32.to_le_bytes().to_vec())],
        vec![(Tag::Variant, vec![0, 0, 0, 0, 2])],
        vec![
            (Tag::Record, vec![1, 0, 0, 0, 0, 0, 0, 0, 0]),
            (Tag::Discard, vec![]),
        ],
        vec![(Tag::Sequence, vec![0, 0, 0, 0, 3])],
    ];
    for rows in rows {
        let mut bank = meter();
        assert!(
            RuntimePattern::preflight_sources(&raw_rows(&rows), limits(), &resources(), &mut bank)
                .is_err()
        );
        assert_eq!(bank.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
}

#[test]
fn source_pattern_namespace_and_literal_type_failures_cannot_publish_an_executable() {
    let (builder, _) = fixture();
    let pattern = builder.lower_pattern_seed_for_test(boolean(true)).unwrap();
    let bytes = encode(&pattern);
    let plan = builder.finish().unwrap();
    let declaration_bytes = frames(&plan);
    for namespace in [true, false] {
        let mut bytes = bytes.clone();
        let mut bank = meter();
        let declarations = declarations(&declaration_bytes, &mut bank);
        let bounded = if namespace {
            RuntimePatternSourceLimits {
                type_count: 8,
                ..limits()
            }
        } else {
            let entity = plan.type_table().id_for_semantic(ENTITY).unwrap();
            bytes[DOMAIN.len() + 6..DOMAIN.len() + 10]
                .copy_from_slice(&entity.get().get().to_le_bytes());
            limits()
        };
        let mut values = resources();
        let prepared =
            RuntimePattern::preflight_sources(&bytes, bounded, &values, &mut bank).unwrap();
        let mut source = declarations.into_construction(bank).unwrap();
        let error = prepared
            .materialize(&mut source, &mut values)
            .err()
            .unwrap();
        if namespace {
            assert!(matches!(
                error,
                RuntimePatternSourceError::Namespace { kind: "type", .. }
            ));
        } else {
            assert!(matches!(
                error,
                RuntimePatternSourceError::Literal(RuntimeLiteralSourceError::Value(_))
            ));
        }
        assert_eq!(
            source.meter.status(),
            Err(TaskSemanticEncodingError::OwnerRejected)
        );
        assert!(
            source
                .prepare_image(RuntimeTaskPlanSealLimits::default())
                .is_err()
        );
    }
}

#[test]
fn source_pattern_enters_original_generic_function_binder_and_changes_its_executable_seal() {
    use crate::plan::{
        RuntimeEffectSet, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFunctionDefinitionIdentity,
        RuntimeFunctionInputBindingSeed, RuntimeFunctionInputOrigin, RuntimeFunctionInputSource,
        RuntimeFunctionInputTransfer, RuntimeFunctionParameterIdentity,
        RuntimeFunctionParameterPassing, RuntimeFunctionSemanticRole, RuntimeFunctionSiteBodyKind,
        RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionTypeContract, RuntimeTypeBinder,
        RuntimeTypeScope, RuntimeValueOwnershipRequirement,
    };
    const PARAM: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([81; 32]);
    const FUNCTION: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([82; 32]);
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([83; 32]);
    let define = |builder: &mut RuntimePlanBuilder,
                  local: crate::plan::RuntimeLocalSeedId,
                  pattern| {
        let function = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([84; 32]),
                role: RuntimeFunctionSemanticRole::Ordinary,
                function_type: Some(FUNCTION),
                inputs: Box::new([RuntimeFunctionInputBindingSeed {
                    transfer: RuntimeFunctionInputTransfer::Formal,
                    origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                    source: RuntimeFunctionInputSource::Parameter {
                        position: 0,
                        passing: RuntimeFunctionParameterPassing::Affine,
                    },
                    input_local: local,
                    pattern,
                    ownership: RuntimeValueOwnershipRequirement::default(),
                    unrestricted_bindings: Box::new([]),
                }]),
                result: BOOL,
                body_kind: RuntimeFunctionSiteBodyKind::Expression,
                effects: RuntimeEffectSet::empty(),
            })
            .unwrap();
        builder
            .define_function_site_seed(
                &function,
                RuntimeExprSeed::new(BOOL, RuntimeExprSeedKind::Value(RuntimeValue::Bool(true))),
            )
            .unwrap();
    };
    let mut builder = RuntimePlanBuilder::new();
    let binder = RuntimeTypeBinder::new(1, 0, 0);
    let scope = RuntimeTypeScope::root().enter(binder).unwrap();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(PARAM, P::BoundType(scope.bound_type(0, 0).unwrap()))
                    .with_scope(scope),
                RuntimePlanTypeSeed::new(BOOL, P::Bool),
                RuntimePlanTypeSeed::new(
                    FUNCTION,
                    P::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            binder,
                            crate::effect_row::EffectPredicate::unconstrained(),
                            crate::effect_row::EffectFormula::empty(),
                        ),
                        parameters: Box::new([PARAM]),
                        result: BOOL,
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::in_function(
                RuntimeLocalDeclarationSource::Parameter(parameter),
                PARAM,
                FUNCTION,
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    define(
        &mut builder,
        local.clone(),
        seed(
            PARAM,
            K::Bind {
                mutable: false,
                local,
            },
        ),
    );
    let plan = builder.finish().unwrap();
    let original = plan.function_sites().iter().next().unwrap().inputs()[0].pattern();
    let bounded = RuntimePatternSourceLimits {
        type_count: plan.type_table().len(),
        local_count: 1,
        ..limits()
    };
    let encoded = original
        .prepare_sources_encoding(bounded, &resources(), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
    encoded.append_to(&mut bytes).unwrap();
    let declaration_bytes = frames(&plan);
    let restore = |bytes: &[u8]| {
        let mut bank = meter();
        let declarations = declarations(&declaration_bytes, &mut bank);
        let mut values = resources();
        let pattern =
            RuntimePattern::preflight_sources(bytes, bounded, &values, &mut bank).unwrap();
        let mut source = declarations.into_construction(bank).unwrap();
        let pattern = pattern.materialize(&mut source, &mut values).unwrap();
        let local = source.locals[0].clone();
        define(&mut source.builder, local, pattern);
        let seal = RuntimeTaskPlanSealLimits::default();
        source
            .prepare_image(seal)
            .unwrap()
            .seal(seal, None)
            .unwrap()
            .into_plan()
    };
    let restored = restore(&bytes);
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(restored.function_sites(), plan.function_sites());
    bytes[DOMAIN.len() + 10] = 1;
    let changed = restore(&bytes);
    assert_ne!(
        changed.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
}
