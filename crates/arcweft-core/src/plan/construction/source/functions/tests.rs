use super::super::pattern::tests::{declarations, frames, meter, resources};
use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFlowOpSeed, RuntimeFunctionCaptureMode as Capture, RuntimeFunctionDefinitionIdentity,
    RuntimeFunctionInputOrigin as Origin, RuntimeFunctionInputSource as Source,
    RuntimeFunctionInputTransfer as Transfer, RuntimeFunctionParameterIdentity,
    RuntimeFunctionParameterPassing as Passing, RuntimeFunctionSemanticRole as Role,
    RuntimeFunctionSiteBodyKind as Body, RuntimeFunctionTypeContract,
    RuntimeLocalBindingDeclaration, RuntimeLocalBindingKind, RuntimeLocalBindingStorage,
    RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource as LocalSource, RuntimePatternSeed,
    RuntimePatternSeedKind as Pattern, RuntimePlan, RuntimePlanTypeProjection as P,
    RuntimePlanTypeSeed, RuntimeTaskPlanSealLimits, RuntimeTypeBinder, RuntimeTypeScope,
    RuntimeValueOwnershipRequirement as Ownership,
};
use crate::value::RuntimeValue;

const BOOL: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([91; 32]);
const PARAM: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([92; 32]);
const FUNCTION: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([93; 32]);
fn limits(plan: &RuntimePlan) -> RuntimeFunctionSourceLimits {
    RuntimeFunctionSourceLimits {
        rows: 100,
        elements: 100_000,
        pattern_nodes: 10_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
        local_count: plan.local_declarations().len(),
    }
}
fn body(builder: &mut RuntimePlanBuilder, handle: &RuntimeFunctionSiteSeedId, kind: Body) {
    let value = RuntimeExprSeed::new(BOOL, RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)));
    match kind {
        Body::Expression => builder.define_function_site_seed(handle, value).unwrap(),
        Body::Executable => builder
            .define_function_site_seed(
                handle,
                crate::plan::RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(value)]),
                }),
            )
            .unwrap(),
    }
}

fn candidate_result(
    role: Role,
    kind: Body,
    transfer: Transfer,
    source: Source,
    origin: Origin,
    generic: bool,
) -> Result<RuntimePlan, RuntimePlanBuildError> {
    let mut builder = RuntimePlanBuilder::new();
    let local_source = match origin {
        Origin::Binding(identity) => LocalSource::Binding {
            identity,
            declaration: RuntimeLocalBindingDeclaration::new(
                RuntimeLocalBindingKind::PatternBinding,
                false,
                RuntimeLocalBindingStorage::Derived,
            ),
        },
        Origin::Parameter(parameter) => LocalSource::Parameter(parameter),
        Origin::EvaluatedResult(definition) => LocalSource::EvaluatedResult(definition),
    };
    let mut types = vec![RuntimePlanTypeSeed::new(BOOL, P::Bool)];
    let ty = if generic { PARAM } else { BOOL };
    if generic {
        let binder = RuntimeTypeBinder::new(1, 0, 0);
        let scope = RuntimeTypeScope::root().enter(binder).unwrap();
        types.push(
            RuntimePlanTypeSeed::new(PARAM, P::BoundType(scope.bound_type(0, 0).unwrap()))
                .with_scope(scope),
        );
        types.push(RuntimePlanTypeSeed::new(
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
        ));
    }
    let local = if generic {
        RuntimeLocalDeclarationSeed::in_function(local_source, ty, FUNCTION)
    } else {
        RuntimeLocalDeclarationSeed::new(local_source, ty)
    };
    let admitted = builder.admit_type_batch(types, [local]).unwrap();
    let local = admitted.local_ids()[0].clone();
    let pattern = if generic {
        Pattern::Bind {
            mutable: false,
            local: local.clone(),
        }
    } else {
        Pattern::Whole {
            local: local.clone(),
            pattern: Box::new(RuntimePatternSeed::new(
                BOOL,
                Pattern::Literal(RuntimeValue::Bool(true)),
            )),
        }
    };
    let effects = if kind == Body::Executable {
        RuntimeEffectSet::try_from_effects([
            arcweft_id::EffectId::log_write(),
            arcweft_id::EffectId::parse("io.read(resource)").unwrap(),
        ])
        .unwrap()
    } else {
        RuntimeEffectSet::empty()
    };
    let handle = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([94; 32]),
            role,
            function_type: generic.then_some(FUNCTION),
            inputs: Box::new([RuntimeFunctionInputBindingSeed {
                transfer,
                origin,
                source,
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(ty, pattern),
                ownership: if generic {
                    Ownership::Owned
                } else {
                    Ownership::Unrestricted
                },
                unrestricted_bindings: if generic {
                    Box::new([])
                } else {
                    Box::new([local])
                },
            }]),
            result: BOOL,
            body_kind: kind,
            effects,
        })
        .unwrap();
    body(&mut builder, &handle, kind);
    builder.finish()
}
fn candidate(
    role: Role,
    kind: Body,
    transfer: Transfer,
    source: Source,
    origin: Origin,
    generic: bool,
) -> RuntimePlan {
    candidate_result(role, kind, transfer, source, origin, generic).unwrap()
}
fn simple() -> RuntimePlan {
    candidate(
        Role::Ordinary,
        Body::Expression,
        Transfer::ExternalBinding,
        Source::Capture { position: 0 },
        Origin::Binding([95; 32]),
        false,
    )
}
fn encode(plan: &RuntimePlan) -> Vec<u8> {
    let encoded = plan
        .function_sites()
        .prepare_declaration_sources_encoding(limits(plan), &resources(), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
    encoded.append_to(&mut bytes).unwrap();
    bytes
}
fn restore(plan: &RuntimePlan, bytes: &[u8]) -> RuntimePlan {
    let declarations_bytes = frames(plan);
    let mut bank = meter();
    let declarations = declarations(&declarations_bytes, &mut bank);
    let mut values = resources();
    let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
        bytes,
        limits(plan),
        &values,
        &mut bank,
    )
    .unwrap();
    let encoded = plan
        .function_sites()
        .prepare_declaration_sources_encoding(limits(plan), &values, &mut meter())
        .unwrap();
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    assert_eq!(prepared.elements(), encoded.elements());
    assert_eq!(prepared.pattern_nodes(), encoded.pattern_nodes());
    assert_eq!(prepared.row_count(), plan.function_sites().len());
    let mut source = declarations.into_construction(bank).unwrap();
    let functions = prepared.reserve(&mut source, &mut values).unwrap();
    for (handle, row) in functions.iter().zip(plan.function_sites().iter()) {
        body(&mut source.builder, handle, row.body().kind());
    }
    let seal = RuntimeTaskPlanSealLimits::default();
    source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan()
}

#[test]
fn function_declaration_sources_preserve_all_roles_capture_operations_passing_ownership_effects_and_seal()
 {
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([96; 32]);
    let mut cases = vec![(
        Transfer::ExternalBinding,
        Source::Capture { position: 0 },
        Origin::Binding([95; 32]),
    )];
    for mode in [Capture::Copy, Capture::SnapshotClone, Capture::Move] {
        cases.push((
            Transfer::Transferred(mode),
            Source::Capture { position: 0 },
            Origin::EvaluatedResult(RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                [97; 32],
            )),
        ));
    }
    for passing in [Passing::Value, Passing::Shared, Passing::Affine] {
        cases.push((
            Transfer::Formal,
            Source::Parameter {
                position: 0,
                passing,
            },
            Origin::Parameter(parameter),
        ));
        cases.push((
            Transfer::Formal,
            Source::CapturedParameter {
                position: 0,
                passing,
            },
            Origin::Parameter(parameter),
        ));
    }
    for role in Role::ALL {
        for kind in [Body::Expression, Body::Executable] {
            for &(transfer, source, origin) in &cases {
                let result = candidate_result(*role, kind, transfer, source, origin, false);
                if *role == Role::Flow {
                    assert!(matches!(result, Err(RuntimePlanBuildError::Plan(
                        crate::plan::entry_inventory::RuntimePlanError::FlowFunctionOwnerCount { actual: 1, expected: 0 }
                    ))));
                    continue;
                }
                let plan = result.unwrap();
                let bytes = encode(&plan);
                let restored = restore(&plan, &bytes);
                assert_eq!(restored.function_sites(), plan.function_sites());
                assert_eq!(restored.local_declarations(), plan.local_declarations());
                assert_eq!(
                    restored.executable_semantic_digest(),
                    plan.executable_semantic_digest()
                );
                assert_eq!(encode(&restored), bytes);
            }
        }
    }
}

#[test]
fn function_declaration_sources_reserve_original_generic_binder_before_body_definition() {
    let plan = candidate(
        Role::Ordinary,
        Body::Expression,
        Transfer::Formal,
        Source::Parameter {
            position: 0,
            passing: Passing::Affine,
        },
        Origin::Parameter(RuntimeFunctionParameterIdentity::from_accepted_identity(
            [96; 32],
        )),
        true,
    );
    let bytes = encode(&plan);
    let restored = restore(&plan, &bytes);
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    let declarations_bytes = frames(&plan);
    let mut bank = meter();
    let declarations = declarations(&declarations_bytes, &mut bank);
    let mut values = resources();
    let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        limits(&plan),
        &values,
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let _handles = prepared.reserve(&mut source, &mut values).unwrap();
    // Signature transport is not permission to publish an absent body.
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn function_declaration_sources_enforce_combined_limits_and_reserved_output() {
    let plan = paired_candidate();
    let bytes = encode(&plan);
    assert_eq!(
        restore(&plan, &bytes).executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        limits(&plan),
        &resources(),
        &mut meter(),
    )
    .unwrap();
    let exact = RuntimeFunctionSourceLimits {
        rows: 1,
        elements: prepared.elements() as u32,
        pattern_nodes: prepared.pattern_nodes() as u32,
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        ..limits(&plan)
    };
    RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        exact,
        &resources(),
        &mut meter(),
    )
    .unwrap();
    plan.function_sites()
        .prepare_declaration_sources_encoding(exact, &resources(), &mut meter())
        .unwrap();
    for bounded in [
        RuntimeFunctionSourceLimits { rows: 0, ..exact },
        RuntimeFunctionSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimeFunctionSourceLimits {
            pattern_nodes: exact.pattern_nodes - 1,
            ..exact
        },
        RuntimeFunctionSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimeFunctionSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = meter();
        assert!(
            RuntimeFunctionSiteTable::preflight_declaration_sources(
                &bytes,
                bounded,
                &resources(),
                &mut bank
            )
            .is_err()
        );
        assert!(bank.status().is_err());
    }
    let prepared = plan
        .function_sites()
        .prepare_declaration_sources_encoding(exact, &resources(), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(prepared.encoded_bytes() - 1);
    output.push(61);
    assert!(matches!(
        prepared.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [61]);
}

#[test]
fn function_declaration_sources_reject_every_truncation_bad_tags_ordinals_and_trailing_bytes() {
    let plan = simple();
    let bytes = encode(&plan);
    let mut invalid = (0..bytes.len())
        .map(|end| bytes[..end].to_vec())
        .collect::<Vec<_>>();
    for (offset, value) in [
        (0, 0),
        (DOMAIN.len(), 9),
        (DOMAIN.len() + 5, 0),
        (DOMAIN.len() + 5 + 4 + 32, 255),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        invalid.push(bad);
    }
    let mut bad = bytes.clone();
    bad.push(0);
    invalid.push(bad);
    for bad in invalid {
        let mut bank = meter();
        assert!(
            RuntimeFunctionSiteTable::preflight_declaration_sources(
                &bad,
                limits(&plan),
                &resources(),
                &mut bank
            )
            .is_err()
        );
        assert!(bank.status().is_err());
        assert!(matches!(
            RuntimeFunctionSiteTable::preflight_declaration_sources(
                &bytes,
                limits(&plan),
                &resources(),
                &mut bank
            ),
            Err(Error::Resource(_))
        ));
    }
    // Invalid effect evidence must be bounded before its owning parser may
    // copy the rejected label into a structured diagnostic.
    let plan = forward_candidate();
    let mut bytes = encode(&plan);
    let effects = DOMAIN.len() + 1 + 4 + 4 + 32 + 1 + 4 + 4 + 1;
    bytes[effects..effects + 4].copy_from_slice(&1u32.to_le_bytes());
    let mut payload = 4096u32.to_le_bytes().to_vec();
    payload.extend([b' '; 4096]);
    bytes.splice(effects + 4..effects + 4, payload);
    let bounded = RuntimeFunctionSourceLimits {
        decoded_bytes: RuntimePlanBuilder::function_source_storage_bytes(2, 0).unwrap(),
        ..limits(&plan)
    };
    assert!(matches!(
        RuntimeFunctionSiteTable::preflight_declaration_sources(
            &bytes,
            bounded,
            &resources(),
            &mut meter()
        ),
        Err(Error::Limit {
            kind: "decoded bytes",
            ..
        })
    ));
}

#[test]
fn function_declaration_sources_reject_semantic_input_errors_in_the_actual_owner() {
    let plan = simple();
    let mut bytes = encode(&plan);
    // A well-framed formal transfer cannot reinterpret a captured binding.
    let input = DOMAIN.len() + 1 + 4 + 4 + 32 + 1 + 4 + 4 + 1 + 4 + 4;
    assert_eq!(bytes[input], 1);
    bytes[input] = 2;
    let declarations_bytes = frames(&plan);
    let mut bank = meter();
    let declarations = declarations(&declarations_bytes, &mut bank);
    let mut values = resources();
    let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        limits(&plan),
        &values,
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    assert!(matches!(
        prepared.reserve(&mut source, &mut values),
        Err(Error::Construction(
            RuntimePlanBuildError::InvalidFunctionInputSource { .. }
        ))
    ));
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

#[test]
fn empty_function_sources_still_enforce_header_limit_and_preserve_the_original_work_bank() {
    let plan = RuntimePlanBuilder::new().finish().unwrap();
    let bytes = encode(&plan);
    let exact = RuntimeFunctionSourceLimits {
        rows: 0,
        elements: 0,
        pattern_nodes: 0,
        encoded_bytes: bytes.len(),
        decoded_bytes: 0,
        ..limits(&plan)
    };
    let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        exact,
        &resources(),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(prepared.storage_bytes(), 0);
    plan.function_sites()
        .prepare_declaration_sources_encoding(exact, &resources(), &mut meter())
        .unwrap();
    let short = RuntimeFunctionSourceLimits {
        encoded_bytes: bytes.len() - 1,
        ..exact
    };
    assert!(
        plan.function_sites()
            .prepare_declaration_sources_encoding(short, &resources(), &mut meter())
            .is_err()
    );
    let mut bank = TaskSemanticMeter::new(0, 0);
    let first = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        exact,
        &resources(),
        &mut bank,
    )
    .err()
    .unwrap();
    let second = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        exact,
        &resources(),
        &mut bank,
    )
    .err()
    .unwrap();
    assert_eq!(first, second);
}

#[test]
fn function_source_reservation_checks_actual_namespace_and_shares_parent_value_budget() {
    let plan = paired_candidate();
    let bytes = encode(&plan);
    let declaration_bytes = frames(&plan);
    for wrong_namespace in [true, false] {
        let mut bank = meter();
        let declarations = declarations(&declaration_bytes, &mut bank);
        let bounded = RuntimeFunctionSourceLimits {
            local_count: plan.local_declarations().len() + usize::from(wrong_namespace),
            ..limits(&plan)
        };
        let mut values = ValueResourceAdmission::live(crate::entry::RuntimeSchemaLimits {
            max_nodes: 1,
            ..crate::entry::RuntimeSchemaLimits::engine_default()
        });
        let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
            &bytes, bounded, &values, &mut bank,
        )
        .unwrap();
        let mut source = declarations.into_construction(bank).unwrap();
        let error = prepared.reserve(&mut source, &mut values).err().unwrap();
        if wrong_namespace {
            assert!(matches!(error, Error::Namespace { kind: "local", .. }));
        } else {
            assert!(matches!(error, Error::Pattern(_)));
        }
        assert!(source.builder.function_sites.is_empty());
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

fn paired_candidate() -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    let parameters =
        [110, 111].map(|id| RuntimeFunctionParameterIdentity::from_accepted_identity([id; 32]));
    let admitted = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(BOOL, P::Bool)],
            parameters.map(|parameter| {
                RuntimeLocalDeclarationSeed::new(LocalSource::Parameter(parameter), BOOL)
            }),
        )
        .unwrap();
    let inputs = admitted
        .local_ids()
        .iter()
        .zip(parameters)
        .enumerate()
        .map(
            |(position, (local, parameter))| RuntimeFunctionInputBindingSeed {
                transfer: Transfer::Formal,
                origin: Origin::Parameter(parameter),
                source: Source::Parameter {
                    position: position as u32,
                    passing: Passing::Value,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    BOOL,
                    Pattern::Whole {
                        local: local.clone(),
                        pattern: Box::new(RuntimePatternSeed::new(
                            BOOL,
                            Pattern::Literal(RuntimeValue::Bool(true)),
                        )),
                    },
                ),
                ownership: Ownership::Unrestricted,
                unrestricted_bindings: Box::new([local.clone()]),
            },
        )
        .collect();
    let handle = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([112; 32]),
            role: Role::Ordinary,
            function_type: None,
            inputs,
            result: BOOL,
            body_kind: Body::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    body(&mut builder, &handle, Body::Expression);
    builder.finish().unwrap()
}

const CALLABLE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([103; 32]);
fn define_forward(builder: &mut RuntimePlanBuilder, functions: &[RuntimeFunctionSiteSeedId]) {
    builder
        .define_function_site_seed(
            &functions[0],
            RuntimeExprSeed::new(
                BOOL,
                RuntimeExprSeedKind::Apply {
                    callee: Box::new(RuntimeExprSeed::new(
                        CALLABLE,
                        RuntimeExprSeedKind::Function {
                            site: functions[1].clone(),
                            captures: Box::new([]),
                        },
                    )),
                    args: Box::new([]),
                },
            ),
        )
        .unwrap();
    body(builder, &functions[1], Body::Expression);
}
fn forward_candidate() -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(BOOL, P::Bool),
                RuntimePlanTypeSeed::new(
                    CALLABLE,
                    P::Function {
                        contract: RuntimeFunctionTypeContract::new(
                            RuntimeTypeBinder::new(0, 0, 0),
                            crate::effect_row::EffectPredicate::unconstrained(),
                            crate::effect_row::EffectFormula::empty(),
                        ),
                        parameters: Box::new([]),
                        result: BOOL,
                    },
                ),
            ],
            [],
        )
        .unwrap();
    let functions = (0..2)
        .map(|index| {
            builder
                .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                    definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [104 + index; 32],
                    ),
                    role: Role::Ordinary,
                    function_type: None,
                    inputs: Box::new([]),
                    result: BOOL,
                    body_kind: Body::Expression,
                    effects: RuntimeEffectSet::empty(),
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    define_forward(&mut builder, &functions);
    builder.finish().unwrap()
}

#[test]
fn original_function_sources_reserve_forward_references_before_any_body_and_preserve_the_complete_seal()
 {
    let plan = forward_candidate();
    let bytes = encode(&plan);
    let declaration_bytes = frames(&plan);
    let mut bank = meter();
    let declarations = declarations(&declaration_bytes, &mut bank);
    let mut values = resources();
    let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        limits(&plan),
        &values,
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let functions = prepared.reserve(&mut source, &mut values).unwrap();
    assert_eq!(functions.len(), 2);
    define_forward(&mut source.builder, &functions);
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(encode(&restored), bytes);
}

#[test]
fn a_late_function_source_reservation_failure_cannot_publish_an_earlier_valid_row() {
    let plan = forward_candidate();
    let mut bytes = encode(&plan);
    // Both zero-input rows have the same fixed header. Type Bool is not a frame contract.
    let header = DOMAIN.len() + 1 + 4;
    let row = 4 + 32 + 1 + 4 + 4 + 1 + 4 + 4;
    bytes[header + row + 4 + 32 + 1..header + row + 4 + 32 + 1 + 4]
        .copy_from_slice(&1u32.to_le_bytes());
    let declaration_bytes = frames(&plan);
    let mut bank = meter();
    let declarations = declarations(&declaration_bytes, &mut bank);
    let mut values = resources();
    let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        limits(&plan),
        &values,
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    assert!(matches!(
        prepared.reserve(&mut source, &mut values),
        Err(Error::Construction(_))
    ));
    assert_eq!(source.builder.function_sites.len(), 1);
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

#[test]
fn flow_function_sources_require_the_real_flow_root_and_schema_at_the_common_seal() {
    use crate::plan::{FlowRuntimeId, RuntimeFlowSchema, RuntimeFlowSeed};
    let unit = crate::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let id = FlowRuntimeId::canonical("source_flow").unwrap();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch([RuntimePlanTypeSeed::new(unit, P::Unit)], [])
        .unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: id.clone(),
            parameters: vec![],
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            id.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([106; 32]),
                None,
                Box::new([]),
                unit,
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([]),
            },
        ))
        .unwrap();
    let plan = builder.finish().unwrap();
    let bytes = encode(&plan);
    let declaration_bytes = frames(&plan);
    for with_root in [true, false] {
        let mut bank = meter();
        let declarations = declarations(&declaration_bytes, &mut bank);
        let mut values = resources();
        let prepared = RuntimeFunctionSiteTable::preflight_declaration_sources(
            &bytes,
            limits(&plan),
            &values,
            &mut bank,
        )
        .unwrap();
        let mut source = declarations.into_construction(bank).unwrap();
        let functions = prepared.reserve(&mut source, &mut values).unwrap();
        source
            .builder
            .define_function_site_seed(
                &functions[0],
                crate::plan::RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([]),
                }),
            )
            .unwrap();
        if with_root {
            source
                .builder
                .bind_flow_function_seed(id.clone(), &functions[0])
                .unwrap();
            source
                .builder
                .push_flow_schema(RuntimeFlowSchema {
                    flow: id.clone(),
                    parameters: vec![],
                })
                .unwrap();
        }
        let seal = RuntimeTaskPlanSealLimits::default();
        let result = source
            .prepare_image(seal)
            .and_then(|image| image.seal(seal, None));
        if with_root {
            let restored = result.unwrap().into_plan();
            assert_eq!(restored.function_sites(), plan.function_sites());
            assert_eq!(
                restored.executable_semantic_digest(),
                plan.executable_semantic_digest()
            );
            assert_eq!(encode(&restored), bytes);
        } else {
            assert!(
                matches!(result,Err(crate::plan::RuntimeTaskPlanSealError::Verification(error))
                if matches!(*error,crate::plan::entry_inventory::RuntimePlanError::FlowFunctionOwnerCount {actual:1,expected:0}))
            );
        }
    }
}

#[test]
fn source_function_unrestricted_binding_checks_accept_deep_or_through_the_single_iterative_inventory()
 {
    let mut builder = RuntimePlanBuilder::new();
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([107; 32]);
    let admitted = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(BOOL, P::Bool)],
            [RuntimeLocalDeclarationSeed::new(
                LocalSource::Parameter(parameter),
                BOOL,
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    let binding = || {
        RuntimePatternSeed::new(
            BOOL,
            Pattern::Bind {
                mutable: false,
                local: local.clone(),
            },
        )
    };
    let pattern = (0..256).fold(binding(), |nested, _| {
        RuntimePatternSeed::new(BOOL, Pattern::Or(Box::new([nested, binding()])))
    });
    let handle = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([108; 32]),
            role: Role::Ordinary,
            function_type: None,
            inputs: Box::new([RuntimeFunctionInputBindingSeed {
                transfer: Transfer::Formal,
                origin: Origin::Parameter(parameter),
                source: Source::Parameter {
                    position: 0,
                    passing: Passing::Value,
                },
                input_local: local.clone(),
                pattern,
                ownership: Ownership::Unrestricted,
                unrestricted_bindings: Box::new([local]),
            }]),
            result: BOOL,
            body_kind: Body::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    body(&mut builder, &handle, Body::Expression);
    let plan = builder.finish().unwrap();
    let bytes = encode(&plan);
    let restored = restore(&plan, &bytes);
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
}
