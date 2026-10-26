use super::super::RuntimePlanSourceConstruction;
use super::super::functions::RuntimeFunctionSourceLimits;
use super::super::pattern::tests::{declarations, frames, meter, resources};
use super::*;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::*;
use crate::value::*;

fn ty(tag: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([tag; 32])
}
fn seed(tag: u8, kind: RuntimeExprSeedKind) -> RuntimeExprSeed {
    RuntimeExprSeed::new(ty(tag), kind)
}
fn boolean(value: bool) -> RuntimeExprSeed {
    seed(1, RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)))
}
fn integer(value: i64) -> RuntimeExprSeed {
    seed(2, RuntimeExprSeedKind::Value(RuntimeValue::i64(value)))
}
fn empty_handles() -> RuntimeExpressionSourceHandles<'static> {
    RuntimeExpressionSourceHandles {
        states: &[],
        specializations: &[],
        helpers: &[],
        methods: &[],
        attempts: &[],
    }
}

#[test]
fn array_from_vec_source_round_trip_preserves_exact_type_and_runtime_length_check() {
    for length in [0, 2] {
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(ty(1), RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(
                        ty(2),
                        RuntimePlanTypeProjection::Sequence {
                            kind: RuntimePlanSequenceKind::Vec,
                            item: ty(1),
                        },
                    ),
                    RuntimePlanTypeSeed::new(
                        ty(3),
                        RuntimePlanTypeProjection::Array {
                            item: ty(1),
                            length: length.into(),
                        },
                    ),
                ],
                [],
            )
            .unwrap();
        let expression = seed(
            3,
            RuntimeExprSeedKind::ArrayFromVec {
                value: Box::new(seed(
                    2,
                    RuntimeExprSeedKind::BracketSeq(vec![boolean(true); length as usize].into()),
                )),
            },
        );
        add_function(&mut builder, 1, expression);
        let plan = builder.finish().unwrap();
        let (mut source, functions) = source(&plan);
        replay_functions(&plan, &mut source, &functions, &empty_handles());
        let restored = finish(source);
        assert_eq!(restored.function_sites(), plan.function_sites());
        assert_eq!(
            restored.executable_semantic_digest(),
            plan.executable_semantic_digest()
        );

        let array = restored.type_table().id_for_semantic(ty(3)).unwrap();
        for actual in 0..=3 {
            let value =
                RuntimeValue::Seq(RuntimeSeq::values(vec![RuntimeValue::Bool(true); actual]));
            let result = RuntimeEnv::default().construct_array_from_vec(&restored, array, value);
            assert_eq!(result.is_ok(), actual as u64 == length);
        }
        if length > 0 {
            assert!(
                RuntimeEnv::default()
                    .construct_array_from_vec(
                        &restored,
                        array,
                        RuntimeValue::Seq(RuntimeSeq::values(vec![
                            RuntimeValue::Unit;
                            length as usize
                        ])),
                    )
                    .is_err()
            );
        }
        assert!(
            RuntimeEnv::default()
                .construct_array_from_vec(
                    &restored,
                    array,
                    RuntimeValue::Seq(RuntimeSeq::values(vec![
                        RuntimeValue::Unit;
                        length as usize + 1
                    ])),
                )
                .is_err()
        );
    }
}

#[test]
fn array_from_vec_builder_rejects_non_vec_or_different_item_identity() {
    for source in [
        RuntimePlanTypeProjection::Sequence {
            kind: RuntimePlanSequenceKind::Seq,
            item: ty(1),
        },
        RuntimePlanTypeProjection::Sequence {
            kind: RuntimePlanSequenceKind::Vec,
            item: ty(4),
        },
    ] {
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(ty(1), RuntimePlanTypeProjection::Bool),
                    RuntimePlanTypeSeed::new(ty(2), source),
                    RuntimePlanTypeSeed::new(
                        ty(3),
                        RuntimePlanTypeProjection::Array {
                            item: ty(1),
                            length: 0.into(),
                        },
                    ),
                    RuntimePlanTypeSeed::new(ty(4), RuntimePlanTypeProjection::Unit),
                ],
                [],
            )
            .unwrap();
        assert!(
            builder
                .lower_expression(seed(
                    3,
                    RuntimeExprSeedKind::ArrayFromVec {
                        value: Box::new(seed(2, RuntimeExprSeedKind::BracketSeq(Box::new([])))),
                    }
                ))
                .is_err()
        );
    }
}
pub(in crate::plan::construction::source) fn limits(
    plan: &RuntimePlan,
) -> RuntimeExpressionSourceLimits {
    RuntimeExpressionSourceLimits {
        nodes: 100_000,
        depth: 128,
        elements: 1_000_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        type_count: plan.type_table().len(),
        local_count: plan.local_declarations().len(),
        state_count: plan.callable_states().len(),
        specialization_count: plan.callable_specializations().len(),
        helper_count: plan.pure_helpers().len(),
        method_count: plan.trait_methods().len(),
        template_count: plan.dialogue_content_templates().rows().len(),
        attempt_count: plan.format_attempts().len(),
    }
}
pub(in crate::plan::construction::source) fn decode(
    expr: &RuntimeExpr,
    plan: &RuntimePlan,
    source: &mut RuntimePlanSourceConstruction,
    handles: &RuntimeExpressionSourceHandles<'_>,
) -> RuntimeExprSeed {
    let encoded = expr
        .encode_sources(limits(plan), &resources(), &mut source.meter)
        .unwrap();
    let prepared = RuntimeExpr::preflight_sources(
        encoded.bytes(),
        limits(plan),
        &resources(),
        &mut source.meter,
    )
    .unwrap();
    assert_eq!(prepared.storage_bytes(), encoded.storage_bytes());
    assert_eq!(prepared.elements(), encoded.elements());
    assert_eq!(prepared.node_count(), encoded.node_count());
    prepared
        .materialize(source, handles, &mut resources())
        .unwrap()
}
pub(in crate::plan::construction::source) fn replay_functions(
    plan: &RuntimePlan,
    source: &mut RuntimePlanSourceConstruction,
    functions: &[RuntimeFunctionSiteSeedId],
    handles: &RuntimeExpressionSourceHandles<'_>,
) {
    let handles = super::super::flow::RuntimeFlowSourceHandles {
        expression: handles,
        contents: &[],
        defers: &[],
        tasks: &[],
    };
    super::super::function_bodies::tests::restore(plan, source, functions, &handles);
}
pub(in crate::plan::construction::source) fn source(
    plan: &RuntimePlan,
) -> (
    RuntimePlanSourceConstruction,
    Box<[RuntimeFunctionSiteSeedId]>,
) {
    let mut bank = meter();
    let frames = frames(plan);
    let declarations = declarations(&frames, &mut bank);
    let limits = RuntimeFunctionSourceLimits {
        rows: 1000,
        elements: 1_000_000,
        pattern_nodes: 100_000,
        encoded_bytes: 1 << 24,
        decoded_bytes: 1 << 28,
        type_count: plan.type_table().len(),
        local_count: plan.local_declarations().len(),
    };
    let encoded = plan
        .function_sites()
        .prepare_declaration_sources_encoding(limits, &resources(), &mut bank)
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.encoded_bytes());
    encoded.append_to(&mut bytes).unwrap();
    let functions = RuntimeFunctionSiteTable::preflight_declaration_sources(
        &bytes,
        limits,
        &resources(),
        &mut bank,
    )
    .unwrap();
    let mut source = declarations.into_construction(bank).unwrap();
    let handles = functions.reserve(&mut source, &mut resources()).unwrap();
    (source, handles)
}
fn finish(source: RuntimePlanSourceConstruction) -> RuntimePlan {
    let seal = RuntimeTaskPlanSealLimits::default();
    source
        .prepare_image(seal)
        .unwrap()
        .seal(seal, None)
        .unwrap()
        .into_plan()
}
pub(in crate::plan::construction::source) fn add_function(
    builder: &mut RuntimePlanBuilder,
    ordinal: u8,
    expression: RuntimeExprSeed,
) {
    let handle = builder
        .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            definition: RuntimeFunctionDefinitionIdentity::from_accepted_identity([ordinal; 32]),
            role: RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
            inputs: Box::new([]),
            result: expression.ty(),
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        })
        .unwrap();
    builder
        .define_function_site_seed(&handle, expression)
        .unwrap();
}
fn fixture() -> RuntimePlan {
    use RuntimeExprSeedKind as K;
    use RuntimePlanTypeProjection as P;
    let mut builder = RuntimePlanBuilder::new();
    use crate::entry::{
        RuntimeNominalDeclarationId, RuntimeNominalRecordShape, RuntimeNominalSchemaBody,
        RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
        RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeTypeSchema,
    };
    let nominal = RuntimeNominalTypeId::try_new("fixture.ExpressionRecord").unwrap();
    let field = RuntimeRecordFieldId::try_from_zero_based_ordinal(0).unwrap();
    let schemas = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalDeclarationId::from_bytes(*ty(9).as_bytes()),
            RuntimeNominalSchemaIdentity::new(nominal.clone(), ty(9)),
            vec![],
            RuntimeNominalSchemaBody::Record {
                shape: RuntimeNominalRecordShape::Record,
                fields: Box::new([RuntimeNominalSchemaField::new(
                    field,
                    Some("flag".to_owned()),
                    RuntimeTypeSchema::Bool,
                )]),
            },
        )],
        crate::entry::RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let admitted = builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(ty(1), P::Bool),
                RuntimePlanTypeSeed::new(ty(2), P::Signed(RuntimeSignedIntWidth::I64)),
                RuntimePlanTypeSeed::new(ty(3), P::Unit),
                RuntimePlanTypeSeed::new(ty(4), P::Tuple(Box::new([ty(1), ty(2)]))),
                RuntimePlanTypeSeed::new(
                    ty(5),
                    P::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: ty(2),
                    },
                ),
                RuntimePlanTypeSeed::new(ty(6), P::Range(ty(2))),
                RuntimePlanTypeSeed::new(ty(7), P::Tuple(Box::new([ty(2)]))),
                RuntimePlanTypeSeed::new(
                    ty(8),
                    P::Option {
                        item: ty(2),
                        some_payload: ty(7),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    ty(9),
                    P::Nominal {
                        nominal,
                        layout: schemas.try_layout_hash(ty(9)).unwrap(),
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(ty(10), P::String),
                RuntimePlanTypeSeed::new(ty(11), P::F64),
                RuntimePlanTypeSeed::new(ty(12), P::EntityReference),
                RuntimePlanTypeSeed::new(
                    ty(13),
                    P::Opaque {
                        producer: RuntimeCharacterDialogueProducerId::get(),
                        admission: crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: RuntimeOpaqueValueClass::Plain,
                        persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                        arguments: Box::new([]),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    ty(14),
                    P::Opaque {
                        producer: crate::pattern::RuntimeOpaqueTypeProducerId::try_new(
                            "std.reduction",
                        )
                        .unwrap(),
                        admission: crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: RuntimeOpaqueValueClass::Plain,
                        persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                        arguments: Box::new([ty(1)]),
                    },
                ),
                RuntimePlanTypeSeed::new(
                    ty(16),
                    P::Opaque {
                        producer: crate::pattern::RuntimeOpaqueTypeProducerId::try_new(
                            "fixture.OriginalRecord",
                        )
                        .unwrap(),
                        admission: crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: RuntimeOpaqueValueClass::Plain,
                        persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                        arguments: Box::new([]),
                    },
                ),
            ],
            [
                (2, true),
                (5, true),
                (9, true),
                (1, false),
                (5, true),
                (5, true),
                (1, false),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (tag, mutable))| {
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Binding {
                        identity: [170 + index as u8; 32],
                        declaration: RuntimeLocalBindingDeclaration::new(
                            RuntimeLocalBindingKind::PatternBinding,
                            mutable,
                            RuntimeLocalBindingStorage::Derived,
                        ),
                    },
                    ty(tag),
                )
            }),
            [RuntimeNominalRecordDomainSeed::new(
                ty(9),
                RuntimeNominalRecordShape::Record,
                [RuntimeNominalRecordDomainFieldSeed::new(
                    field,
                    Some("flag".to_owned()),
                    ty(1),
                )],
            )],
            [],
            &schemas,
        )
        .unwrap();
    let locals = admitted.local_ids();
    let read = |index: usize, tag| {
        seed(
            tag,
            K::Local(RuntimeLocalReadSeed::new(
                locals[index].clone(),
                RuntimeLocalReadMode::Copy,
            )),
        )
    };
    let mut expressions = vec![
        boolean(true),
        seed(4, K::Tuple(Box::new([boolean(false), integer(7)]))),
        seed(
            1,
            K::Unary {
                op: RuntimeUnaryOp::Not,
                expr: Box::new(boolean(false)),
            },
        ),
        seed(
            2,
            K::Binary {
                lhs: Box::new(integer(11)),
                op: RuntimeBinaryOp::Sub,
                rhs: Box::new(integer(4)),
            },
        ),
        seed(
            2,
            K::If {
                condition: Box::new(boolean(true)),
                then_expr: Box::new(integer(3)),
                else_expr: Box::new(integer(9)),
            },
        ),
        seed(
            1,
            K::Scope {
                identity: crate::scope::RuntimeScopeIdentity::Named(
                    arcweft_id::DeclarationName::try_new("original_scope").unwrap(),
                ),
                body: Box::new(boolean(true)),
            },
        ),
        seed(5, K::BracketSeq(Box::new([integer(2), integer(1)]))),
        seed(
            5,
            K::RepeatSeq {
                value: Box::new(integer(3)),
                len: 4,
            },
        ),
        seed(
            6,
            K::Range {
                start: Some(Box::new(integer(2))),
                end: Some(Box::new(integer(9))),
                inclusive: true,
            },
        ),
        seed(
            8,
            K::Variant {
                ordinal: 0,
                payload: Some(Box::new(seed(7, K::Tuple(Box::new([integer(4)]))))),
            },
        ),
        seed(
            2,
            K::ProjectTuple {
                target: Box::new(seed(4, K::Tuple(Box::new([boolean(true), integer(8)])))),
                ordinal: 1,
            },
        ),
        seed(
            1,
            K::ProjectRecord {
                target: Box::new(seed(
                    9,
                    K::NominalRecord(Box::new([RuntimeNominalRecordFieldSeed::new(
                        RuntimeRecordFieldSeedId::from_zero_based(0),
                        boolean(true),
                    )])),
                )),
                field: RuntimeRecordFieldSeedId::from_zero_based(0),
            },
        ),
        seed(
            2,
            K::Call {
                callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                args: Box::new([
                    RuntimeCallArgumentSeed::new(integer(3), RuntimeCallArgumentMode::Value, 0),
                    RuntimeCallArgumentSeed::new(integer(8), RuntimeCallArgumentMode::Value, 1),
                ]),
            },
        ),
        seed(
            2,
            K::Sum {
                source: Box::new(seed(5, K::BracketSeq(Box::new([integer(2), integer(3)])))),
            },
        ),
        seed(11, K::Value(RuntimeValue::F64(-0.0))),
        seed(
            10,
            K::Value(RuntimeValue::String("元の文字列\0とUTF-8".to_owned())),
        ),
    ];
    let assigned = seed(
        2,
        K::Assign {
            place: RuntimeAssignmentSeed {
                place: RuntimeMutablePlaceSeed::Local(locals[0].clone()),
                displacement: RuntimePlaceDisplacement::Reachable {
                    initialization: RuntimePlaceInitialization::Initialized,
                    fields: Box::new([]),
                },
            },
            expr: Box::new(integer(8)),
            body: Box::new(read(0, 2)),
        },
    );
    expressions.push(seed(
        2,
        K::Let {
            binding: locals[0].clone(),
            expr: Box::new(integer(1)),
            body: Box::new(assigned),
        },
    ));
    for pop_front in [true, false] {
        let index = if pop_front { 1 } else { 4 };
        let place = RuntimeMutablePlaceSeed::Local(locals[index].clone());
        let pop = if pop_front {
            K::SequencePopFront { place }
        } else {
            K::SequencePopBack { place }
        };
        expressions.push(seed(
            8,
            K::Let {
                binding: locals[index].clone(),
                expr: Box::new(seed(5, K::BracketSeq(Box::new([integer(4)])))),
                body: Box::new(seed(8, pop)),
            },
        ));
    }
    expressions.push(seed(
        3,
        K::Let {
            binding: locals[5].clone(),
            expr: Box::new(seed(5, K::BracketSeq(Box::new([])))),
            body: Box::new(seed(
                3,
                K::SequencePush {
                    place: RuntimeMutablePlaceSeed::Local(locals[5].clone()),
                    value: Box::new(integer(6)),
                },
            )),
        },
    ));
    let record = seed(
        9,
        K::NominalRecord(Box::new([RuntimeNominalRecordFieldSeed::new(
            RuntimeRecordFieldSeedId::from_zero_based(0),
            boolean(true),
        )])),
    );
    expressions.push(seed(
        1,
        K::Let {
            binding: locals[2].clone(),
            expr: Box::new(record),
            body: Box::new(seed(
                1,
                K::Field {
                    target: RuntimeFieldTargetSeed::Inspect(RuntimeMutablePlaceSeed::Local(
                        locals[2].clone(),
                    )),
                    field: RuntimeFieldProjectionSeed::Nominal {
                        owner: ty(9),
                        field: RuntimeRecordFieldSeedId::from_zero_based(0),
                    },
                },
            )),
        },
    ));
    let pattern = |index: usize| {
        RuntimePatternSeed::new(
            ty(1),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local: locals[index].clone(),
            },
        )
    };
    let guard = |index: usize| boolean(true).with_guard_copy_locals([locals[index].clone()]);
    expressions.push(seed(
        2,
        K::IfLet {
            pattern: pattern(3),
            expr: Box::new(boolean(true)),
            guard: Some(Box::new(guard(3))),
            then_expr: Box::new(integer(2)),
            else_expr: Box::new(integer(3)),
        },
    ));
    expressions.push(seed(
        2,
        K::Match {
            scrutinee: Box::new(boolean(true)),
            arms: Box::new([
                RuntimeExprMatchArmSeed::new(pattern(6), Some(guard(6)), integer(4)),
                RuntimeExprMatchArmSeed::new(
                    RuntimePatternSeed::new(ty(1), RuntimePatternSeedKind::Discard),
                    None,
                    integer(5),
                ),
            ]),
        },
    ));
    // Exercise all linkage families without interpreting an authored label as
    // an intrinsic or losing provider/export/overload distinctions.
    let identity = crate::entry::RuntimeCallableId::from_checked_digest([155; 32]);
    expressions.push(seed(
        1,
        K::Call {
            callee: RuntimeCallTarget::Callable(identity.clone()),
            args: Box::new([]),
        },
    ));
    for provider in [
        RuntimeExternalCallableProvider::Adapter("fixture.adapter".to_owned()),
        RuntimeExternalCallableProvider::Standard(StandardEnvironmentId::InferenceTensor),
    ] {
        for export in [
            RuntimeExternalCallableExport::Function(Box::new([
                "first".to_owned(),
                "second".to_owned(),
            ])),
            RuntimeExternalCallableExport::RustFunction(Box::new(["first.second".to_owned()])),
            RuntimeExternalCallableExport::Method {
                receiver: [156; 32],
                name: "selected".to_owned(),
            },
        ] {
            expressions.push(seed(
                1,
                K::Call {
                    callee: RuntimeCallTarget::External(RuntimeExternalCallable::new(
                        identity.clone(),
                        RuntimeExternalCallableDeclaration::try_new(
                            provider.clone(),
                            export,
                            u16::MAX,
                        )
                        .unwrap(),
                    )),
                    args: Box::new([]),
                },
            ));
        }
    }
    let entity = || {
        seed(
            12,
            K::EntityRef(RuntimeEntityReference::StructuralFlow(
                FlowRuntimeId::from_runtime_contract("original.child", "公開名").unwrap(),
            )),
        )
    };
    expressions.push(entity());
    expressions.push(seed(
        10,
        K::Field {
            target: Box::new(entity()).into(),
            field: RuntimeFieldProjectionSeed::EntityReference(RuntimeEntityReferenceField::Name),
        },
    ));
    let producer =
        crate::pattern::RuntimeOpaqueTypeProducerId::try_new("fixture.OriginalRecord").unwrap();
    let owner = crate::pattern::RuntimeOpaqueTypeOwner::exact(producer.clone(), ty(16));
    expressions.push(seed(
        1,
        K::Field {
            target: Box::new(seed(
                16,
                K::Value(RuntimeValue::Opaque(RuntimeOpaqueValue::new_exact(
                    &owner,
                    RuntimeValue::Tuple(vec![RuntimeValue::Bool(true)]),
                ))),
            ))
            .into(),
            field: RuntimeFieldProjectionSeed::OpaqueRecord {
                owner: ty(16),
                producer,
                field: RuntimeRecordFieldSeedId::from_zero_based(0),
                field_type: ty(1),
            },
        },
    ));
    expressions.push(seed(
        14,
        K::ReductionUnchanged {
            state: Box::new(boolean(true)),
        },
    ));
    use arcweft_interaction_model::dialogue::{
        CharacterDialogueCustomFieldId, CharacterDialogueFieldCoordinate as Coordinate,
        CharacterDialogueOperation as Operation, CharacterDialoguePatchField as Field,
        CharacterDialoguePatchOperation as Patch,
    };
    let character = || {
        seed(
            12,
            K::EntityRef(
                RuntimeEntityReference::try_project(
                    arcweft_id::DeclarationIdentityFamily::Character,
                    arcweft_id::PublicId::try_new("character.original").unwrap(),
                )
                .unwrap(),
            ),
        )
    };
    let factory = || {
        seed(
            13,
            K::CharacterDialogue {
                operation: Operation::Factory,
                target: Box::new(character()),
                fields: Box::new([
                    Field {
                        coordinate: Coordinate::RichText,
                        operation: Patch::Set(boolean(true)),
                    },
                    Field {
                        coordinate: Coordinate::Look,
                        operation: Patch::Clear,
                    },
                    Field {
                        coordinate: Coordinate::Custom(
                            CharacterDialogueCustomFieldId::try_new(
                                "character_dialogue_field.original",
                            )
                            .unwrap(),
                        ),
                        operation: Patch::Set(integer(4)),
                    },
                    Field {
                        coordinate: Coordinate::RichText,
                        operation: Patch::Set(boolean(false)),
                    },
                ]),
            },
        )
    };
    expressions.push(factory());
    expressions.push(seed(
        13,
        K::CharacterDialogue {
            operation: Operation::Reconfigure,
            target: Box::new(factory()),
            fields: Box::new([Field {
                coordinate: Coordinate::SourceLocale,
                operation: Patch::Set(seed(10, K::Value(RuntimeValue::String("ja-JP".to_owned())))),
            }]),
        },
    ));
    for (index, expression) in expressions.into_iter().enumerate() {
        add_function(&mut builder, 50 + index as u8, expression);
    }
    builder.finish().unwrap()
}

#[test]
fn expression_sources_preserve_every_agent_constructor_and_comparison_coordinate() {
    use RuntimeAgentExprSeed as A;
    use RuntimeAgentTypeProjection as T;
    use RuntimeExprSeedKind as K;
    use RuntimePlanTypeProjection as P;
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                (1, P::Bool),
                (10, P::String),
                (20, P::Agent(T::ActionTarget)),
                (21, P::Agent(T::CaptureTarget)),
                (22, P::Agent(T::DebugStatePath)),
                (23, P::Agent(T::ObservationFieldPath)),
                (24, P::Agent(T::Probe(ty(1)))),
                (25, P::Agent(T::Diagnostics)),
                (26, P::Agent(T::Predicate)),
                (27, P::Agent(T::ViewportPoint)),
                (28, P::Unsigned(RuntimeUnsignedIntWidth::U32)),
                (
                    29,
                    P::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: ty(26),
                    },
                ),
            ]
            .map(|(tag, projection)| RuntimePlanTypeSeed::new(ty(tag), projection)),
            [],
        )
        .unwrap();
    let text = || {
        Box::new(seed(
            10,
            K::Value(RuntimeValue::String("original.target".to_owned())),
        ))
    };
    let probe = || Box::new(seed(24, K::Agent(A::ProbeSignal { target: text() })));
    let predicate = || seed(26, K::Agent(A::PredicateExists { probe: probe() }));
    let collection = || {
        Box::new([seed(
            29,
            K::BracketSeq(Box::new([predicate(), predicate()])),
        )]) as Box<[RuntimeExprSeed]>
    };
    let mut expressions = vec![
        seed(
            20,
            K::Agent(A::ChoiceAction {
                choice: crate::entry::RuntimeCommandTargetId::try_new("choice.original").unwrap(),
            }),
        ),
        seed(21, K::Agent(A::CaptureViewport)),
        seed(21, K::Agent(A::CaptureLayer { target: text() })),
        seed(21, K::Agent(A::CaptureObject { target: text() })),
        seed(22, K::Agent(A::StatePath { path: text() })),
        seed(23, K::Agent(A::ObservationPath { path: text() })),
        seed(24, K::Agent(A::ProbeSignal { target: text() })),
        seed(24, K::Agent(A::ProbeMetric { target: text() })),
        seed(
            24,
            K::Agent(A::ProbeState {
                path: Box::new(seed(22, K::Agent(A::StatePath { path: text() }))),
            }),
        ),
        seed(
            24,
            K::Agent(A::ProbeObservation {
                path: Box::new(seed(23, K::Agent(A::ObservationPath { path: text() }))),
            }),
        ),
        seed(25, K::Agent(A::Diagnostics)),
        predicate(),
        seed(
            26,
            K::Agent(A::PredicateActionEnabled {
                target: Box::new(seed(
                    20,
                    K::Agent(A::ChoiceAction {
                        choice: crate::entry::RuntimeCommandTargetId::try_new("choice.original")
                            .unwrap(),
                    }),
                )),
            }),
        ),
        seed(
            26,
            K::Agent(A::PredicateDiagnosticsHasError {
                diagnostics: Box::new(seed(25, K::Agent(A::Diagnostics))),
            }),
        ),
        seed(
            26,
            K::Agent(A::PredicateAll {
                predicates: collection(),
            }),
        ),
        seed(
            26,
            K::Agent(A::PredicateAny {
                predicates: collection(),
            }),
        ),
        seed(
            26,
            K::Agent(A::PredicateNot {
                predicate: Box::new(predicate()),
            }),
        ),
        seed(
            27,
            K::Agent(A::ViewportPoint {
                x: Box::new(seed(28, K::Value(RuntimeValue::u32(3)))),
                y: Box::new(seed(28, K::Value(RuntimeValue::u32(9)))),
            }),
        ),
    ];
    for op in [
        RuntimeAgentCompareOp::Eq,
        RuntimeAgentCompareOp::NotEq,
        RuntimeAgentCompareOp::Greater,
        RuntimeAgentCompareOp::GreaterOrEqual,
        RuntimeAgentCompareOp::Less,
        RuntimeAgentCompareOp::LessOrEqual,
    ] {
        expressions.push(seed(
            26,
            K::Agent(A::PredicateCompare {
                probe: probe(),
                op,
                value: Box::new(boolean(true)),
            }),
        ));
    }
    for (index, expr) in expressions.into_iter().enumerate() {
        add_function(&mut builder, 80 + index as u8, expr);
    }
    let plan = builder.finish().unwrap();
    let (mut source, functions) = source(&plan);
    replay_functions(&plan, &mut source, &functions, &empty_handles());
    let restored = finish(source);
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    let mut constructors = std::collections::BTreeSet::new();
    for function in restored.function_sites().iter() {
        let RuntimeFunctionSiteBody::Expression(expr) = function.body() else {
            unreachable!()
        };
        if let RuntimeExprKind::Agent(agent) = expr.kind() {
            constructors.insert(agent.constructor().semantic_tag());
        }
    }
    assert_eq!(constructors.len(), RuntimeAgentConstructor::ALL.len());
}

#[test]
fn expression_sources_restore_original_order_scope_places_patterns_literals_and_external_linkage_then_seal()
 {
    let plan = fixture();
    let (mut source, functions) = source(&plan);
    replay_functions(&plan, &mut source, &functions, &empty_handles());
    let restored = finish(source);
    assert_eq!(restored.function_sites(), plan.function_sites());
    assert_eq!(
        restored.executable_semantic_digest(),
        plan.executable_semantic_digest()
    );
    for (left, right) in plan
        .function_sites()
        .iter()
        .zip(restored.function_sites().iter())
    {
        let (RuntimeFunctionSiteBody::Expression(left), RuntimeFunctionSiteBody::Expression(right)) =
            (left.body(), right.body())
        else {
            unreachable!()
        };
        assert_eq!(
            left.encode_sources(limits(&plan), &resources(), &mut meter())
                .unwrap()
                .bytes(),
            right
                .encode_sources(limits(&restored), &resources(), &mut meter())
                .unwrap()
                .bytes()
        );
    }
}
fn first(plan: &RuntimePlan) -> &RuntimeExpr {
    let RuntimeFunctionSiteBody::Expression(expr) =
        plan.function_sites().iter().next().unwrap().body()
    else {
        unreachable!()
    };
    expr
}
#[test]
fn expression_sources_admit_exact_counts_and_reject_every_smaller_cap() {
    let plan = fixture();
    for function in plan.function_sites().iter() {
        let RuntimeFunctionSiteBody::Expression(expr) = function.body() else {
            unreachable!()
        };
        let encoded = expr
            .encode_sources(limits(&plan), &resources(), &mut meter())
            .unwrap();
        let bytes = encoded.bytes();
        let exact = RuntimeExpressionSourceLimits {
            nodes: encoded.node_count() as u32,
            elements: encoded.elements() as u32,
            encoded_bytes: bytes.len(),
            decoded_bytes: encoded.storage_bytes(),
            ..limits(&plan)
        };
        RuntimeExpr::preflight_sources(bytes, exact, &resources(), &mut meter()).unwrap();
        for short in [
            RuntimeExpressionSourceLimits {
                nodes: exact.nodes - 1,
                ..exact
            },
            RuntimeExpressionSourceLimits {
                elements: exact.elements - 1,
                ..exact
            },
            RuntimeExpressionSourceLimits {
                encoded_bytes: exact.encoded_bytes - 1,
                ..exact
            },
            RuntimeExpressionSourceLimits {
                decoded_bytes: exact.decoded_bytes - 1,
                ..exact
            },
        ] {
            let mut bank = meter();
            assert!(RuntimeExpr::preflight_sources(bytes, short, &resources(), &mut bank).is_err());
            assert!(bank.status().is_err());
        }
    }
}
#[test]
fn expression_sources_reject_all_truncated_prefixes_extra_roots_and_invalid_refs_tags_and_flags() {
    let plan = fixture();
    let bytes = first(&plan)
        .encode_sources(limits(&plan), &resources(), &mut meter())
        .unwrap()
        .into_bytes();
    for end in 0..bytes.len() {
        assert!(
            RuntimeExpr::preflight_sources(
                &bytes[..end],
                limits(&plan),
                &resources(),
                &mut meter()
            )
            .is_err(),
            "prefix {end}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        RuntimeExpr::preflight_sources(&trailing, limits(&plan), &resources(), &mut meter()),
        Err(Error::Trailing { .. })
    ));
    for (offset, value) in [
        (0, 0),
        (DOMAIN.len(), 2),
        (DOMAIN.len() + 5, 255),
        (DOMAIN.len() + 6, 0),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(
            RuntimeExpr::preflight_sources(&bad, limits(&plan), &resources(), &mut meter())
                .is_err(),
            "offset {offset}"
        );
    }
}
#[test]
fn expression_sources_keep_the_original_bank_and_capacity_failure_does_not_write_a_prefix() {
    let plan = fixture();
    let expr = first(&plan);
    let encoded = expr
        .encode_sources(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let bytes = encoded.bytes().to_vec();
    let mut output = Vec::with_capacity(bytes.len());
    output.push(99);
    assert!(matches!(
        encoded.append_to(&mut output),
        Err(Error::OutputCapacity { .. })
    ));
    assert_eq!(output, [99]);
    let mut bank = crate::task::semantic::TaskSemanticMeter::new(1, u64::MAX);
    assert!(matches!(
        RuntimeExpr::preflight_sources(&bytes, limits(&plan), &resources(), &mut bank),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
    assert!(matches!(
        RuntimeExpr::preflight_sources(&bytes, limits(&plan), &resources(), &mut bank),
        Err(Error::Resource(TaskSemanticEncodingError::SemanticWork))
    ));
}
#[test]
fn expression_sources_authenticate_the_actual_namespace_before_materializing_a_literal() {
    let plan = fixture();
    let bytes = first(&plan)
        .encode_sources(limits(&plan), &resources(), &mut meter())
        .unwrap()
        .into_bytes();
    let mut wrong = limits(&plan);
    wrong.local_count += 1;
    let prepared =
        RuntimeExpr::preflight_sources(&bytes, wrong, &resources(), &mut meter()).unwrap();
    let (mut source, _) = source(&plan);
    assert!(matches!(
        prepared.materialize(&mut source, &empty_handles(), &mut resources()),
        Err(Error::Namespace { kind: "local", .. })
    ));
    assert!(source.meter.status().is_err());
}

#[test]
fn expression_sources_bound_depth_and_reject_missing_children_before_scope_growth() {
    use RuntimeExprSeedKind as K;
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                ty(1),
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .unwrap();
    let mut expression = boolean(true);
    for _ in 0..32 {
        expression = seed(
            1,
            K::Unary {
                op: RuntimeUnaryOp::Not,
                expr: Box::new(expression),
            },
        );
    }
    add_function(&mut builder, 199, expression);
    let plan = builder.finish().unwrap();
    let encoded = first(&plan)
        .encode_sources(limits(&plan), &resources(), &mut meter())
        .unwrap();
    let exact = RuntimeExpressionSourceLimits {
        depth: 32,
        ..limits(&plan)
    };
    RuntimeExpr::preflight_sources(encoded.bytes(), exact, &resources(), &mut meter()).unwrap();
    let short = RuntimeExpressionSourceLimits { depth: 31, ..exact };
    assert!(matches!(
        RuntimeExpr::preflight_sources(encoded.bytes(), short, &resources(), &mut meter()),
        Err(Error::Limit { kind: "depth", .. })
    ));
    assert!(matches!(
        first(&plan).encode_sources(short, &resources(), &mut meter()),
        Err(Error::Limit { kind: "depth", .. })
    ));
    let mut impossible = encoded.bytes().to_vec();
    impossible[DOMAIN.len() + 1..DOMAIN.len() + 5].copy_from_slice(&1u32.to_le_bytes());
    // The single unary row is completely framed, but cannot fill its one
    // child within the header. Its stack growth must be rejected first.
    impossible.truncate(DOMAIN.len() + 5 + 1 + 4 + 4 + 1);
    let mut bank = meter();
    assert!(matches!(
        RuntimeExpr::preflight_sources(&impossible, exact, &resources(), &mut bank),
        Err(Error::Root)
    ));
    assert!(bank.status().is_err());
}
#[test]
fn expression_intrinsic_source_tags_are_closed_and_preserve_all_original_semantic_coordinates() {
    let mut count = 0;
    for tag in 0..=u8::MAX {
        if let Some(intrinsic) = RuntimeIntrinsic::from_source_tag(tag) {
            assert_eq!(intrinsic.semantic_tag(), tag);
            count += 1;
        }
    }
    assert_eq!(count, 94);
    assert_eq!(
        RuntimeIntrinsic::from_source_tag(92),
        Some(RuntimeIntrinsic::CoreRequireUnrestricted)
    );
    assert_eq!(
        RuntimeIntrinsic::from_source_tag(93),
        Some(RuntimeIntrinsic::CoreFanoutInputs)
    );
}
