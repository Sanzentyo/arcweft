use super::super::functions::RuntimeFunctionSourceLimits;
use super::super::pattern::tests::{declarations, frames, meter, resources};
use super::*;

#[test]
fn original_expression_sources_replay_helper_method_and_function_bodies_with_original_signatures() {
    use super::super::expression::{
        RuntimeExpressionSourceHandles,
        tests::{decode, replay_functions},
    };
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut source, functions) = source(&plan);
    let callables =
        PreparedRuntimePureCallableSources::preflight(&bytes, limits(&plan), &mut source.meter)
            .unwrap()
            .reserve(&mut source, &functions)
            .unwrap();
    let handles = RuntimeExpressionSourceHandles {
        states: &[],
        specializations: &[],
        helpers: &callables.helpers,
        methods: &callables.methods,
        attempts: &[],
    };
    for (helper, handle) in plan.pure_helpers().iter().zip(&callables.helpers) {
        let body = decode(&helper.expr, &plan, &mut source, &handles);
        source
            .builder
            .define_pure_helper_seed(handle, body)
            .unwrap();
    }
    for (method, handle) in plan.trait_methods().iter().zip(&callables.methods) {
        let body = decode(&method.body, &plan, &mut source, &handles);
        source
            .builder
            .define_trait_method_seed(handle, body)
            .unwrap();
    }
    replay_functions(&plan, &mut source, &functions, &handles);
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(restored.pure_helpers(), plan.pure_helpers());
    assert_eq!(restored.trait_methods(), plan.trait_methods());
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
}
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::*;
use crate::value::{RuntimeCallArgumentMode, RuntimeLocalReadMode};

const BOOL: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([201; 32]);
fn limits(plan: &RuntimePlan) -> RuntimePureCallableSourceLimits {
    RuntimePureCallableSourceLimits {
        helpers: 100,
        methods: 100,
        bindings: 100,
        elements: 100_000,
        encoded_bytes: 1 << 22,
        decoded_bytes: 1 << 24,
        type_count: plan.type_table().len(),
        local_count: plan.local_declarations().len(),
        function_count: plan.function_sites().len(),
    }
}
fn function_limits(plan: &RuntimePlan) -> RuntimeFunctionSourceLimits {
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
fn local(local: &RuntimeLocalSeedId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        BOOL,
        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
            local.clone(),
            RuntimeLocalReadMode::Copy,
        )),
    )
}
fn pure(helper: &RuntimePureHelperSeedId, local_id: &RuntimeLocalSeedId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        BOOL,
        RuntimeExprSeedKind::PureCall {
            helper: helper.clone(),
            args: Box::new([RuntimeCallArgumentSeed::new(
                local(local_id),
                RuntimeCallArgumentMode::Value,
                0,
            )]),
        },
    )
}
fn define(
    builder: &mut RuntimePlanBuilder,
    helpers: &[RuntimePureHelperSeedId],
    method: &RuntimeTraitMethodSeedId,
    function: &RuntimeFunctionSiteSeedId,
    locals: &[RuntimeLocalSeedId],
) {
    builder
        .define_pure_helper_seed(&helpers[0], pure(&helpers[1], &locals[0]))
        .unwrap();
    builder
        .define_pure_helper_seed(&helpers[1], local(&locals[1]))
        .unwrap();
    builder
        .define_trait_method_seed(method, pure(&helpers[0], &locals[2]))
        .unwrap();
    builder
        .define_function_site_seed(
            function,
            RuntimeExprSeed::new(
                BOOL,
                RuntimeExprSeedKind::TraitCall {
                    callable: method.clone(),
                    receiver: Box::new(local(&locals[3])),
                    args: Box::new([]),
                },
            ),
        )
        .unwrap();
}
fn fixture() -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    let parameters = (0..4)
        .map(|i| RuntimeFunctionParameterIdentity::from_accepted_identity([202 + i; 32]))
        .collect::<Vec<_>>();
    let admitted = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                BOOL,
                RuntimePlanTypeProjection::Bool,
            )],
            parameters.iter().map(|p| {
                RuntimeLocalDeclarationSeed::new(RuntimeLocalDeclarationSource::Parameter(*p), BOOL)
            }),
        )
        .unwrap();
    let locals = admitted.local_ids().to_vec();
    let function = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([206; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs: Box::new([RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Formal,
                origin: RuntimeFunctionInputOrigin::Parameter(parameters[3]),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: RuntimeFunctionParameterPassing::Value,
                },
                input_local: locals[3].clone(),
                pattern: RuntimePatternSeed::new(
                    BOOL,
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local: locals[3].clone(),
                    },
                ),
                ownership: RuntimeValueOwnershipRequirement::Owned,
                unrestricted_bindings: Box::new([]),
            }]),
            result: BOOL,
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    let mut helpers = Vec::new();
    for i in 0..2 {
        helpers.push(
            builder
                .reserve_pure_helper_seed(RuntimePureHelperDeclarationSeed {
                    definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [207 + i as u8; 32],
                    ),
                    name: format!("補助{i}"),
                    inputs: Box::new([RuntimeCallableParameterSeed {
                        identity: parameters[i],
                        local: locals[i].clone(),
                        passing: RuntimeFunctionParameterPassing::Value,
                        abi: RuntimePureInputType::Value,
                    }]),
                    result: BOOL,
                    output_abi: RuntimePureOutputType::Bool,
                    scalar_eval_supported: i == 1,
                    origin: if i == 0 {
                        RuntimePureHelperOrigin::Annotated
                    } else {
                        RuntimePureHelperOrigin::Inferred
                    },
                })
                .unwrap(),
        );
    }
    let method = builder
        .reserve_trait_method_seed(RuntimeTraitMethodDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([209; 32]),
            identity: RuntimeTraitMethodIdentity {
                impl_id: 0,
                trait_id: Some(3),
                witness: Some(0),
                trait_name: Some("検証".to_owned()),
                self_type: "Bool".to_owned(),
                method_name: "accept".to_owned(),
                monomorph_label: "Bool.accept".to_owned(),
            },
            receiver: RuntimeReceiverMode::Owned,
            inputs: Box::new([RuntimeCallableParameterSeed {
                identity: parameters[2],
                local: locals[2].clone(),
                passing: RuntimeFunctionParameterPassing::Value,
                abi: RuntimePureInputType::Value,
            }]),
            result: BOOL,
            output_abi: RuntimePureOutputType::Bool,
        })
        .unwrap();
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program: arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                [210; 32],
            ),
            site: function.clone(),
        })
        .unwrap();
    define(&mut builder, &helpers, &method, &function, &locals);
    builder.finish().unwrap()
}
fn encode(plan: &RuntimePlan) -> Vec<u8> {
    let prepared = plan
        .prepare_pure_callable_declaration_sources_encoding(limits(plan), &mut meter())
        .unwrap();
    let mut bytes = Vec::with_capacity(prepared.encoded_bytes());
    prepared.append_to(&mut bytes).unwrap();
    bytes
}
fn source(
    plan: &RuntimePlan,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
) {
    source_with_bank(plan, meter())
}
fn source_with_bank(
    plan: &RuntimePlan,
    mut bank: TaskSemanticMeter,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
) {
    let frames = frames(plan);
    let declarations = declarations(&frames, &mut bank);
    let encoded = plan
        .function_sites()
        .prepare_declaration_sources_encoding(function_limits(plan), &resources(), &mut bank)
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
    encoded.append_to(&mut bytes).unwrap();
    let functions = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        function_limits(plan),
        &resources(),
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let handles = functions.reserve(&mut source, &mut resources()).unwrap();
    (source, handles)
}

#[test]
fn pure_callable_sources_reserve_original_forward_targets_and_program_binding_before_bodies_then_seal()
 {
    let plan = fixture();
    let bytes = encode(&plan);
    let encoded = plan
        .prepare_pure_callable_declaration_sources_encoding(limits(&plan), &mut meter())
        .unwrap();
    let (mut source, functions) = source(&plan);
    let prepared =
        PreparedRuntimePureCallableSources::preflight(&bytes, limits(&plan), &mut source.meter)
            .unwrap();
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    assert_eq!(prepared.elements(), encoded.elements());
    let handles = prepared.reserve(&mut source, &functions).unwrap();
    let locals = [
        plan.pure_helpers()[0].inputs[0].local(),
        plan.pure_helpers()[1].inputs[0].local(),
        plan.trait_methods()[0].inputs[0].local(),
        plan.function_sites().iter().next().unwrap().inputs()[0].input_local(),
    ]
    .map(|id| source.local(id).unwrap().clone());
    define(
        &mut source.builder,
        &handles.helpers,
        &handles.methods[0],
        &functions[0],
        &locals,
    );
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(restored.pure_helpers(), plan.pure_helpers());
    assert_eq!(restored.trait_methods(), plan.trait_methods());
    assert_eq!(restored.pure_programs(), plan.pure_programs());
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    assert_eq!(encode(&restored), bytes);
}

#[test]
fn pure_callable_sources_admit_exact_combined_counts_and_reject_one_less_before_reservation() {
    let plan = fixture();
    let bytes = encode(&plan);
    let prepared =
        PreparedRuntimePureCallableSources::preflight(&bytes, limits(&plan), &mut meter()).unwrap();
    let exact = RuntimePureCallableSourceLimits {
        helpers: 2,
        methods: 1,
        bindings: 1,
        elements: prepared.elements() as u32,
        encoded_bytes: bytes.len(),
        decoded_bytes: prepared.storage_bytes(),
        ..limits(&plan)
    };
    PreparedRuntimePureCallableSources::preflight(&bytes, exact, &mut meter()).unwrap();
    for low in [
        RuntimePureCallableSourceLimits {
            helpers: 1,
            ..exact
        },
        RuntimePureCallableSourceLimits {
            methods: 0,
            ..exact
        },
        RuntimePureCallableSourceLimits {
            bindings: 0,
            ..exact
        },
        RuntimePureCallableSourceLimits {
            elements: exact.elements - 1,
            ..exact
        },
        RuntimePureCallableSourceLimits {
            encoded_bytes: exact.encoded_bytes - 1,
            ..exact
        },
        RuntimePureCallableSourceLimits {
            decoded_bytes: exact.decoded_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = meter();
        assert!(PreparedRuntimePureCallableSources::preflight(&bytes, low, &mut bank).is_err());
        assert!(bank.status().is_err());
    }
}

#[test]
fn pure_callable_sources_reject_truncation_tags_refs_and_trailing_data_without_retaining_a_graph() {
    let plan = fixture();
    let bytes = encode(&plan);
    for end in 0..bytes.len() {
        assert!(
            PreparedRuntimePureCallableSources::preflight(
                &bytes[..end],
                limits(&plan),
                &mut meter()
            )
            .is_err(),
            "prefix {end}"
        );
    }
    let first = DOMAIN.len() + 1 + 4;
    for (offset, value) in [
        (0, b'?'),
        (DOMAIN.len(), 2),
        (first, 0),
        (first + 4 + 32, 0),
        (first + 4 + 32 + 4, 255),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(
            PreparedRuntimePureCallableSources::preflight(&bad, limits(&plan), &mut meter())
                .is_err(),
            "offset {offset}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(
        PreparedRuntimePureCallableSources::preflight(&trailing, limits(&plan), &mut meter())
            .is_err()
    );
}

#[test]
fn pure_callable_sources_reject_foreign_function_namespace_before_allocating_callable_rows() {
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut own, _) = source(&plan);
    let (_, foreign) = source(&plan);
    let prepared =
        PreparedRuntimePureCallableSources::preflight(&bytes, limits(&plan), &mut meter()).unwrap();
    assert!(matches!(
        prepared.reserve(&mut own, &foreign),
        Err(Error::ForeignFunction)
    ));
    assert!(own.builder.pure_helpers.is_empty());
    assert!(own.builder.trait_methods.is_empty());
    assert!(own.builder.pure_programs.is_empty());
    assert!(
        own.prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn pure_callable_source_late_abi_failure_cannot_publish_the_earlier_reserved_helper() {
    let plan = fixture();
    let mut bytes = encode(&plan);
    let second = {
        let mut c = Cursor::new(&bytes, limits(&plan));
        c.header().unwrap();
        let row = c.helper(0).unwrap();
        let end = row.inputs.as_ptr() as usize + row.inputs.len();
        end - bytes.as_ptr() as usize
    };
    bytes[second + 4 + 32 + 4] = RuntimePureOutputType::I8.semantic_tag();
    let prepared =
        PreparedRuntimePureCallableSources::preflight(&bytes, limits(&plan), &mut meter()).unwrap();
    let (mut source, functions) = source(&plan);
    assert!(matches!(
        prepared.reserve(&mut source, &functions),
        Err(Error::Construction(_))
    ));
    assert_eq!(source.builder.pure_helpers.len(), 1);
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn pure_callable_source_encoding_rejects_capacity_without_touching_the_existing_prefix() {
    let plan = fixture();
    let prepared = plan
        .prepare_pure_callable_declaration_sources_encoding(limits(&plan), &mut meter())
        .unwrap();
    let mut output = Vec::with_capacity(prepared.encoded_bytes());
    output.push(93);
    let before = output.clone();
    assert!(matches!(
        prepared.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, before);
}

#[test]
fn pure_callable_source_work_exhaustion_stays_on_the_original_bank() {
    let plan = fixture();
    let bytes = encode(&plan);
    let mut bank = TaskSemanticMeter::new(bytes.len() as u64 * 3 - 1, 1_000_000);
    assert!(matches!(
        PreparedRuntimePureCallableSources::preflight(&bytes, limits(&plan), &mut bank),
        Err(Error::Resource(
            TaskSemanticEncodingError::SemanticWork { .. }
        ))
    ));
    assert!(PreparedRuntimePureCallableSources::preflight(&[], limits(&plan), &mut bank).is_err());
}

#[test]
fn pure_callable_handle_aliases_share_the_original_parameter_storage_and_keep_the_issuer() {
    let plan = fixture();
    let bytes = encode(&plan);
    let (mut source, functions) = source(&plan);
    let handles =
        PreparedRuntimePureCallableSources::preflight(&bytes, limits(&plan), &mut meter())
            .unwrap()
            .reserve(&mut source, &functions)
            .unwrap();
    let helpers = (0..128)
        .map(|_| handles.helpers[0].clone())
        .collect::<Vec<_>>();
    let methods = (0..128)
        .map(|_| handles.methods[0].clone())
        .collect::<Vec<_>>();
    let hp = handles.helpers[0]
        .resolve(&source.builder.issuer)
        .unwrap()
        .1
        .as_ptr();
    let mp = handles.methods[0]
        .resolve(&source.builder.issuer)
        .unwrap()
        .2
        .as_ptr();
    drop(handles);
    for alias in &helpers {
        assert_eq!(
            alias.resolve(&source.builder.issuer).unwrap().1.as_ptr(),
            hp
        );
    }
    for alias in &methods {
        assert_eq!(
            alias.resolve(&source.builder.issuer).unwrap().2.as_ptr(),
            mp
        );
    }
    let foreign = RuntimePlanBuilder::new();
    assert!(helpers[0].resolve(&foreign.issuer).is_none());
    assert!(methods[0].resolve(&foreign.issuer).is_none());
}

#[test]
fn pure_callable_sources_round_trip_every_physical_abi_receiver_and_optional_identity_shape() {
    use crate::value::{RuntimeSignedIntWidth as S, RuntimeUnsignedIntWidth as U};
    use RuntimePlanTypeProjection as P;
    use RuntimePureInputType as I;
    use RuntimePureOutputType as O;
    let cases = [
        (P::Signed(S::I8), I::I8, O::I8),
        (P::Signed(S::I16), I::I16, O::I16),
        (P::Signed(S::I32), I::I32, O::I32),
        (P::Signed(S::I64), I::I64, O::I64),
        (P::Signed(S::I128), I::I128, O::I128),
        (P::Signed(S::ISize), I::ISize, O::ISize),
        (P::Unsigned(U::U8), I::U8, O::U8),
        (P::Unsigned(U::U16), I::U16, O::U16),
        (P::Unsigned(U::U32), I::U32, O::U32),
        (P::Unsigned(U::U64), I::U64, O::U64),
        (P::Unsigned(U::U128), I::U128, O::U128),
        (P::Unsigned(U::USize), I::USize, O::USize),
        (P::F32, I::F32, O::F32),
        (P::F64, I::F64, O::F64),
        (P::Bool, I::Value, O::Bool),
        (P::Bool, I::Value, O::Value),
    ];
    for (projection, input, output) in cases {
        for receiver in [
            RuntimeReceiverMode::Owned,
            RuntimeReceiverMode::SharedRef,
            RuntimeReceiverMode::MutRef,
        ] {
            let mut builder = RuntimePlanBuilder::new();
            let ids = [
                RuntimeFunctionParameterIdentity::from_accepted_identity([220; 32]),
                RuntimeFunctionParameterIdentity::from_accepted_identity([221; 32]),
            ];
            let locals = builder
                .admit_type_batch(
                    [RuntimePlanTypeSeed::new(BOOL, projection.clone())],
                    ids.map(|id| {
                        RuntimeLocalDeclarationSeed::new(
                            RuntimeLocalDeclarationSource::Parameter(id),
                            BOOL,
                        )
                    }),
                )
                .unwrap()
                .local_ids()
                .to_vec();
            let helper = builder
                .reserve_pure_helper_seed(RuntimePureHelperDeclarationSeed {
                    definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [222; 32],
                    ),
                    name: String::new(),
                    inputs: Box::new([RuntimeCallableParameterSeed {
                        identity: ids[0],
                        local: locals[0].clone(),
                        passing: RuntimeFunctionParameterPassing::Value,
                        abi: input,
                    }]),
                    result: BOOL,
                    output_abi: output,
                    scalar_eval_supported: false,
                    origin: RuntimePureHelperOrigin::Inferred,
                })
                .unwrap();
            let method = builder
                .reserve_trait_method_seed(RuntimeTraitMethodDeclarationSeed {
                    definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [223; 32],
                    ),
                    identity: RuntimeTraitMethodIdentity {
                        impl_id: usize::MAX,
                        trait_id: None,
                        witness: None,
                        trait_name: None,
                        self_type: String::new(),
                        method_name: String::new(),
                        monomorph_label: String::new(),
                    },
                    receiver,
                    inputs: Box::new([RuntimeCallableParameterSeed {
                        identity: ids[1],
                        local: locals[1].clone(),
                        passing: match receiver {
                            RuntimeReceiverMode::Owned => RuntimeFunctionParameterPassing::Value,
                            RuntimeReceiverMode::SharedRef => {
                                RuntimeFunctionParameterPassing::Shared
                            }
                            RuntimeReceiverMode::MutRef => RuntimeFunctionParameterPassing::Affine,
                        },
                        abi: input,
                    }]),
                    result: BOOL,
                    output_abi: output,
                })
                .unwrap();
            builder
                .define_pure_helper_seed(&helper, local(&locals[0]))
                .unwrap();
            builder
                .define_trait_method_seed(&method, local(&locals[1]))
                .unwrap();
            let plan = builder.finish().unwrap();
            let bytes = encode(&plan);
            let (mut source, functions) = source(&plan);
            let handles = PreparedRuntimePureCallableSources::preflight(
                &bytes,
                limits(&plan),
                &mut source.meter,
            )
            .unwrap()
            .reserve(&mut source, &functions)
            .unwrap();
            let hl = source
                .local(plan.pure_helpers()[0].inputs[0].local())
                .unwrap()
                .clone();
            let ml = source
                .local(plan.trait_methods()[0].inputs[0].local())
                .unwrap()
                .clone();
            source
                .builder
                .define_pure_helper_seed(&handles.helpers[0], local(&hl))
                .unwrap();
            source
                .builder
                .define_trait_method_seed(&handles.methods[0], local(&ml))
                .unwrap();
            let seal = RuntimeTaskPlanSealLimits::default();
            let restored = source
                .prepare_image(seal)
                .unwrap()
                .seal(seal, None)
                .unwrap()
                .into_plan();
            assert_eq!(restored.pure_helpers(), plan.pure_helpers());
            assert_eq!(restored.trait_methods(), plan.trait_methods());
            assert_eq!(
                restored.executable_semantic_digest(),
                plan.executable_semantic_digest()
            );
            assert_eq!(encode(&restored), bytes);
        }
    }
}

#[test]
fn pure_callable_empty_namespaces_need_no_retained_row_storage_and_still_finish_through_the_seal() {
    let plan = RuntimePlanBuilder::new().finish().unwrap();
    let bytes = encode(&plan);
    let limits = RuntimePureCallableSourceLimits {
        helpers: 0,
        methods: 0,
        bindings: 0,
        elements: 0,
        decoded_bytes: 0,
        encoded_bytes: bytes.len(),
        ..limits(&plan)
    };
    let prepared =
        PreparedRuntimePureCallableSources::preflight(&bytes, limits, &mut meter()).unwrap();
    assert_eq!(prepared.storage_bytes(), 0);
    assert_eq!(prepared.elements(), 0);
    let (mut source, functions) = source(&plan);
    let handles = prepared.reserve(&mut source, &functions).unwrap();
    assert!(handles.helpers.is_empty());
    assert!(handles.methods.is_empty());
    let seal = RuntimeTaskPlanSealLimits::default();
    let restored = source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan();
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
}

#[test]
fn pure_program_binding_count_is_only_an_assertion_against_the_original_function_handle() {
    let plan = fixture();
    let mut bytes = encode(&plan);
    let end = bytes.len();
    bytes[end - 4..].copy_from_slice(&0u32.to_le_bytes());
    let prepared =
        PreparedRuntimePureCallableSources::preflight(&bytes, limits(&plan), &mut meter()).unwrap();
    let (mut source, functions) = source(&plan);
    assert!(matches!(
        prepared.reserve(&mut source, &functions),
        Err(Error::Namespace {
            kind: "program binding inputs",
            ..
        })
    ));
    assert!(source.builder.pure_programs.is_empty());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn pure_callable_preflight_keeps_the_original_finite_bank_after_declaration_and_function_sources() {
    let plan = fixture();
    let bytes = encode(&plan);
    let (measured, _) = source(&plan);
    let prelude = measured.meter.totals().0;
    let limit = prelude + bytes.len() as u64 * 3 - 1;
    let (mut source, _) = source_with_bank(&plan, TaskSemanticMeter::new(limit, 100_000_000));
    assert_eq!(source.meter.totals().0, prelude);
    assert!(matches!(
        PreparedRuntimePureCallableSources::preflight(&bytes, limits(&plan), &mut source.meter),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert_eq!(source.meter.totals().0, prelude);
    assert!(source.builder.pure_helpers.is_empty());
    assert!(source.builder.trait_methods.is_empty());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}

#[test]
fn pure_callable_local_resolution_preserves_the_earliest_duplicate_before_a_foreign_input() {
    let plan = fixture();
    let (mut source, _) = source(&plan);
    let mut foreign = RuntimePlanBuilder::new();
    let foreign_local = foreign
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                BOOL,
                RuntimePlanTypeProjection::Bool,
            )],
            [RuntimeLocalDeclarationSeed::new(
                RuntimeLocalDeclarationSource::Parameter(
                    RuntimeFunctionParameterIdentity::from_accepted_identity([242; 32]),
                ),
                BOOL,
            )],
        )
        .unwrap()
        .local_ids()[0]
        .clone();
    let own = source
        .local(plan.pure_helpers()[0].inputs[0].local())
        .unwrap()
        .clone();
    let inputs = [own.clone(), own, foreign_local]
        .into_iter()
        .enumerate()
        .map(|(i, local)| RuntimeCallableParameterSeed {
            identity: RuntimeFunctionParameterIdentity::from_accepted_identity([240 + i as u8; 32]),
            local,
            passing: RuntimeFunctionParameterPassing::Value,
            abi: RuntimePureInputType::Value,
        })
        .collect();
    assert!(
        matches!(source.builder.reserve_pure_helper_seed(RuntimePureHelperDeclarationSeed {
        definition:RuntimeFunctionDefinitionIdentity::from_accepted_identity([243;32]),name:String::new(),inputs,result:BOOL,output_abi:RuntimePureOutputType::Bool,scalar_eval_supported:false,origin:RuntimePureHelperOrigin::Inferred,
    }),Err(RuntimePlanBuildError::DuplicateFunctionLocal {local}) if local==plan.pure_helpers()[0].inputs[0].local())
    );
    assert!(source.builder.pure_helpers.is_empty());
    assert!(
        source
            .prepare_image(RuntimeTaskPlanSealLimits::default())
            .is_err()
    );
}
