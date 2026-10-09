use super::*;
use arcweft_core::awbc::codec::AwbcDecodeBudget;
use arcweft_core::awbc::fiber::FiberState;
use arcweft_core::awbc::schema::{
    AwbcEntryId, AwbcFieldProjection, AwbcFieldTarget, AwbcInstruction, AwbcMutablePlace,
    AwbcPlaceReadMode, AwbcProgram, AwbcStringId,
};
use arcweft_core::awbc::vm::{self, VmExit, VmStepOptions};
use arcweft_core::entry::{
    EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
    RuntimeNominalDeclarationId, RuntimeNominalRecordShape, RuntimeNominalSchemaBody,
    RuntimeNominalSchemaDefinition, RuntimeNominalSchemaField, RuntimeNominalSchemaGraph,
    RuntimeNominalSchemaIdentity, RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema,
};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeEffectSet, RuntimeEntryKind, RuntimeEntrySpec,
    RuntimeEntryTarget, RuntimeExecutableBodySeed, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFieldProjectionSeed, RuntimeFieldTargetSeed, RuntimeFlowOpSeed, RuntimeFlowSchema,
    RuntimeFlowSeed, RuntimeFunctionDefinitionIdentity, RuntimeFunctionSiteDeclarationSeed,
    RuntimeLocalDeclarationSeed, RuntimeLocalDeclarationSource, RuntimeLocalReadSeed,
    RuntimeMutablePlaceSeed, RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed,
    RuntimeNominalRecordFieldSeed, RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan,
    RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimeRecordFieldSeedId,
};
use arcweft_core::value::{
    RuntimeLocalReadMode, RuntimeNominalRecordValue, RuntimeRecordFieldId, RuntimeValue,
};

fn identity(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn local_source(name: &str) -> RuntimeLocalDeclarationSource {
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(format!("arcweft-runtime-plan.fixture.field_inspection.{name}").as_bytes());
    RuntimeLocalDeclarationSource::Binding {
        identity: *identity.finalize().as_bytes(),
        declaration: arcweft_core::plan::RuntimeLocalBindingDeclaration::new(
            arcweft_core::plan::RuntimeLocalBindingKind::PatternBinding,
            false,
            arcweft_core::plan::RuntimeLocalBindingStorage::Derived,
        ),
    }
}

fn nominal_inspection_plan() -> (RuntimePlan, RuntimeValue) {
    let nominal = RuntimeNominalTypeId::try_new("fixture.InspectedRecord").unwrap();
    let nominal_identity = RuntimeNominalSchemaIdentity::new(nominal.clone(), identity(3));
    let field = |ordinal| RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap();
    let schemas = RuntimeNominalSchemaGraph::try_new(
        vec![RuntimeNominalSchemaDefinition::new(
            RuntimeNominalDeclarationId::from_bytes(
                *nominal_identity.semantic_identity().as_bytes(),
            ),
            nominal_identity,
            Vec::new(),
            RuntimeNominalSchemaBody::Record {
                shape: RuntimeNominalRecordShape::Record,
                fields: Box::new([
                    RuntimeNominalSchemaField::new(
                        field(0),
                        Some("uri".into()),
                        RuntimeTypeSchema::String,
                    ),
                    RuntimeNominalSchemaField::new(
                        field(1),
                        Some("enabled".into()),
                        RuntimeTypeSchema::Bool,
                    ),
                ]),
            },
        )],
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let layout = schemas.try_layout_hash(identity(3)).unwrap();
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    identity(3),
                    RuntimePlanTypeProjection::Nominal {
                        nominal: nominal.clone(),
                        layout,
                        arguments: Box::new([]),
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(local_source("owner"), identity(3)),
                RuntimeLocalDeclarationSeed::new(local_source("first_uri"), identity(1)),
                RuntimeLocalDeclarationSeed::new(local_source("second_uri"), identity(1)),
            ],
            [RuntimeNominalRecordDomainSeed::new(
                identity(3),
                RuntimeNominalRecordShape::Record,
                [
                    RuntimeNominalRecordDomainFieldSeed::new(
                        field(0),
                        Some("uri".into()),
                        identity(1),
                    ),
                    RuntimeNominalRecordDomainFieldSeed::new(
                        field(1),
                        Some("enabled".into()),
                        identity(2),
                    ),
                ],
            )],
            [],
            &schemas,
        )
        .unwrap();
    let owner = admitted.local_ids()[0].clone();
    let inspection = || {
        RuntimeExprSeed::new(
            identity(1),
            RuntimeExprSeedKind::Field {
                target: RuntimeFieldTargetSeed::Inspect(RuntimeMutablePlaceSeed::Local(
                    owner.clone(),
                )),
                field: RuntimeFieldProjectionSeed::Nominal {
                    owner: identity(3),
                    field: RuntimeRecordFieldSeedId::from_zero_based(0),
                },
            },
        )
    };
    let literal = |ty, value| RuntimeExprSeed::new(identity(ty), RuntimeExprSeedKind::Value(value));
    let bind = |ty, local| {
        RuntimePatternSeed::new(
            identity(ty),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        )
    };
    let flow = FlowRuntimeId::canonical("field.nominal_inspection").unwrap();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .unwrap();
    builder
        .push_flow_executable(RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([0xf0; 32]),
            controller: None,
        })
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            RuntimeFunctionSiteDeclarationSeed::flow(
                RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]),
                None,
                Box::new([]),
                identity(3),
                RuntimeEffectSet::empty(),
            ),
            RuntimeExecutableBodySeed {
                effects: RuntimeEffectSet::empty(),
                ops: Box::new([
                    RuntimeFlowOpSeed::Let {
                        pattern: bind(3, owner.clone()),
                        expr: RuntimeExprSeed::new(
                            identity(3),
                            RuntimeExprSeedKind::NominalRecord(Box::new([
                                RuntimeNominalRecordFieldSeed::new(
                                    RuntimeRecordFieldSeedId::from_zero_based(0),
                                    literal(1, RuntimeValue::String("resource://one".into())),
                                ),
                                RuntimeNominalRecordFieldSeed::new(
                                    RuntimeRecordFieldSeedId::from_zero_based(1),
                                    literal(2, RuntimeValue::Bool(true)),
                                ),
                            ])),
                        ),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: bind(1, admitted.local_ids()[1].clone()),
                        expr: inspection(),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: bind(1, admitted.local_ids()[2].clone()),
                        expr: inspection(),
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        identity(3),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            owner,
                            RuntimeLocalReadMode::Move,
                        )),
                    )),
                ]),
            },
        ))
        .unwrap();
    builder
        .push_entry(RuntimeEntrySpec {
            id: EntryRuntimeId::canonical("field.nominal_inspection").unwrap(),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([1; 32]),
            target: RuntimeEntryTarget::Flow(flow),
            roles: RuntimeEntryRoles::None,
        })
        .unwrap();
    let expected = RuntimeValue::NominalRecord(RuntimeNominalRecordValue::new(
        nominal,
        identity(3),
        layout,
        vec![
            RuntimeValue::String("resource://one".into()),
            RuntimeValue::Bool(true),
        ],
    ));
    (builder.finish().unwrap(), expected)
}

fn lower_nominal_inspection() -> (AwbcProgram, RuntimeValue) {
    let (plan, expected) = nominal_inspection_plan();
    let report = AwbcLowerer::new(&plan, &DialogueContentCatalog::new(), "field.arcw")
        .lower()
        .unwrap();
    assert!(report.diagnostics.is_empty());
    (report.program, expected)
}

#[test]
fn nominal_field_inspection_lowers_roundtrips_and_retains_the_receiver_until_move() {
    let (program, expected) = lower_nominal_inspection();
    let reads = program
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            AwbcInstruction::ReadPlace {
                dst,
                root,
                fields,
                mode: AwbcPlaceReadMode::Copy,
            } if fields.len() == 1 && fields[0].zero_based() == 0 => Some((*dst, *root)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reads.len(),
        2,
        "each nominal inspection has its admitted ordinal Copy read"
    );
    assert_eq!(reads[0].1, reads[1].1);
    assert_ne!(reads[0].0, reads[0].1);
    assert_ne!(reads[1].0, reads[1].1);
    program
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let encoded = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded.encode_canonical().unwrap(), encoded);
    decoded
        .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
        .unwrap();
    let mut fiber = FiberState::for_entry(&decoded, AwbcEntryId(0), 0, 256).unwrap();
    let mut seen = [false; 2];
    let mut moved = false;
    let mut returned = None;
    for _ in 0..128 {
        let step = vm::step(
            &decoded,
            &mut fiber,
            VmStepOptions {
                max_instructions: 1,
            },
        )
        .unwrap();
        if let Ok(frame) = fiber.active_frame() {
            for (index, (dst, base)) in reads.iter().enumerate() {
                if !seen[index]
                    && frame.register(*dst).ok()
                        == Some(&RuntimeValue::String("resource://one".into()))
                {
                    assert_eq!(frame.register(*base).unwrap(), &expected);
                    seen[index] = true;
                }
            }
            if seen == [true; 2] && frame.register(reads[0].1).is_err() {
                moved = true;
            }
        }
        if let VmExit::Returned(value) = step.exit {
            returned = value;
            break;
        }
    }
    assert_eq!(seen, [true; 2]);
    assert!(moved, "the receiver is consumed by its later explicit Move");
    assert_eq!(returned, Some(expected));
}

#[test]
fn nominal_field_inspection_rejects_a_forged_named_awbc_coordinate() {
    let (mut program, _) = lower_nominal_inspection();
    let label = AwbcStringId(
        u32::try_from(
            program
                .strings
                .iter()
                .position(|value| value == "uri")
                .unwrap(),
        )
        .unwrap(),
    );
    let read = program
        .instructions
        .iter_mut()
        .find(|instruction| {
            matches!(instruction,
                AwbcInstruction::ReadPlace { fields, mode: AwbcPlaceReadMode::Copy, .. }
                    if fields.len() == 1 && fields[0].zero_based() == 0
            )
        })
        .unwrap();
    let (dst, root) = match read {
        AwbcInstruction::ReadPlace { dst, root, .. } => (*dst, *root),
        _ => unreachable!(),
    };
    *read = AwbcInstruction::ProjectField {
        dst,
        target: AwbcFieldTarget::Inspect(AwbcMutablePlace::Local(root)),
        field: AwbcFieldProjection::Named(label),
    };
    assert!(
        matches!(program.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
            Err(AwbcVerifyError::InvalidInvariant { message, .. }) if message == "record projection target"
        )
    );
}

fn reference_field_program(
    inspect: bool,
) -> (
    RuntimePlan,
    arcweft_id::runtime_program::RuntimePureProgramId,
) {
    use arcweft_core::plan::{
        RuntimeFunctionInputBindingSeed, RuntimeFunctionInputOrigin,
        RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionInputSource,
        RuntimeFunctionInputTransfer, RuntimeFunctionParameterIdentity,
        RuntimeFunctionParameterPassing, RuntimeFunctionSemanticRole,
        RuntimePureProgramBindingSeed,
    };
    use arcweft_core::value::RuntimeEntityReferenceField;
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(identity(1), RuntimePlanTypeProjection::EntityReference),
                RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    identity(4),
                    RuntimePlanTypeProjection::Tuple(Box::new([
                        identity(2),
                        identity(2),
                        identity(2),
                        identity(1),
                    ])),
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(
                local_source("reference"),
                identity(1),
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    let local_read = |mode| {
        RuntimeExprSeed::new(
            identity(1),
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(local.clone(), mode)),
        )
    };
    let field = |field: RuntimeEntityReferenceField| {
        RuntimeExprSeed::new(
            identity(2),
            RuntimeExprSeedKind::Field {
                target: if inspect {
                    RuntimeFieldTargetSeed::Inspect(RuntimeMutablePlaceSeed::Local(local.clone()))
                } else {
                    RuntimeFieldTargetSeed::Value(Box::new(local_read(RuntimeLocalReadMode::Copy)))
                },
                field: RuntimeFieldProjectionSeed::Agent(field.into()),
            },
        )
    };
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([97; 32]);
    let site = builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([63; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Formal,
                origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: RuntimeFunctionParameterPassing::Value,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    identity(1),
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local: local.clone(),
                    },
                ),
                ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
                unrestricted_bindings: Box::new([]),
            }],
            RuntimeExprSeed::new(
                identity(4),
                RuntimeExprSeedKind::Tuple(Box::new([
                    field(RuntimeEntityReferenceField::Id),
                    field(RuntimeEntityReferenceField::Family),
                    field(RuntimeEntityReferenceField::Name),
                    local_read(RuntimeLocalReadMode::Move),
                ])),
            ),
        )
        .unwrap();
    let program = arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([98; 32]);
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, site })
        .unwrap();
    (builder.finish().unwrap(), program)
}

fn reference_field_awbc(
    inspect: bool,
) -> (
    AwbcProgram,
    arcweft_id::runtime_program::RuntimePureProgramId,
) {
    let (plan, program) = reference_field_program(inspect);
    let lowered = AwbcLowerer::new(
        &plan,
        &DialogueContentCatalog::new(),
        "reference-field.arcw",
    )
    .lower()
    .unwrap();
    assert!(lowered.diagnostics.is_empty());
    (lowered.program, program)
}

/// Starts from a valid, builder-admitted Bool formal and String result.
/// Only the field instruction is malformed; all original type/schema rows stay owned.
fn wrong_owner_reference_field_awbc(inspect: bool) -> AwbcProgram {
    use arcweft_core::plan::{
        RuntimeFunctionInputBindingSeed, RuntimeFunctionInputOrigin,
        RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionInputSource,
        RuntimeFunctionInputTransfer, RuntimeFunctionParameterIdentity,
        RuntimeFunctionParameterPassing, RuntimeFunctionSemanticRole,
        RuntimePureProgramBindingSeed,
    };
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(identity(11), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::String),
            ],
            [RuntimeLocalDeclarationSeed::new(
                local_source("wrong_reference_owner"),
                identity(11),
            )],
        )
        .unwrap();
    let local = admitted.local_ids()[0].clone();
    let parameter = RuntimeFunctionParameterIdentity::from_accepted_identity([100; 32]);
    let site = builder
        .push_function_site_seed(
            RuntimeFunctionDefinitionIdentity::from_accepted_identity([101; 32]),
            RuntimeFunctionSemanticRole::Ordinary,
            [RuntimeFunctionInputBindingSeed {
                transfer: RuntimeFunctionInputTransfer::Formal,
                origin: RuntimeFunctionInputOrigin::Parameter(parameter),
                source: RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: RuntimeFunctionParameterPassing::Value,
                },
                input_local: local.clone(),
                pattern: RuntimePatternSeed::new(
                    identity(11),
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local,
                    },
                ),
                ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
                unrestricted_bindings: Box::new([]),
            }],
            RuntimeExprSeed::new(
                identity(2),
                RuntimeExprSeedKind::Value(RuntimeValue::String("ok".into())),
            ),
        )
        .unwrap();
    let pure_program =
        arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest([102; 32]);
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed {
            program: pure_program,
            site,
        })
        .unwrap();
    let plan = builder.finish().unwrap();
    let lowered = AwbcLowerer::new(
        &plan,
        &DialogueContentCatalog::new(),
        "wrong-reference-owner.arcw",
    )
    .lower()
    .unwrap();
    assert!(lowered.diagnostics.is_empty());
    let mut program = lowered.program;
    program
        .verify(
            AwbcVerifyBudget::default(),
            AwbcVerifyContext {
                require_entrypoint: false,
                ..Default::default()
            },
        )
        .expect("Bool input, initialized formal, and String result are otherwise admitted");
    let encoded = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded.encode_canonical().unwrap(), encoded);
    assert_eq!(
        arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
            &std::sync::Arc::new(decoded),
            pure_program,
            &[RuntimeValue::Bool(true)],
            &mut arcweft_core::pure::VmRuntimePureCallBackend::default()
        )
        .unwrap(),
        RuntimeValue::String("ok".into()),
        "typed Bool argument admission reaches the actual body"
    );
    let function = program
        .pure_programs
        .iter()
        .find(|binding| binding.program == pure_program)
        .expect("the admitted program retains its actual function binding")
        .function;
    let frame = &program.frame_layouts[program.functions[function.index()].frame_layout.index()];
    let parameters = frame
        .slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.role == arcweft_core::awbc::schema::AwbcFrameSlotRole::Parameter)
        .collect::<Vec<_>>();
    assert_eq!(
        parameters.len(),
        1,
        "the actual positional binder initializes one formal"
    );
    let (receiver, slot) = parameters[0];
    assert!(matches!(
        program.runtime_types[slot.ty.index()].shape(),
        arcweft_core::awbc::schema::AwbcRuntimeTypeShape::Bool
    ));
    assert_eq!(
        program.runtime_types[slot.ty.index()].semantic_identity(),
        identity(11)
    );
    assert_eq!(
        program.signatures[program.functions[function.index()].signature.index()].params,
        [slot.ty],
        "the receiver comes from the actual admitted Bool ABI"
    );
    let parameter = arcweft_core::awbc::schema::AwbcRegisterId(u32::try_from(receiver).unwrap());
    let pattern = program
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            AwbcInstruction::BindPattern { pattern, value, .. } if *value == parameter => {
                Some(*pattern)
            }
            _ => None,
        })
        .expect("the actual formal prologue transfers its input into the checked local");
    let arcweft_core::awbc::schema::AwbcPattern::Bind {
        target: receiver,
        expected,
        ..
    } = program.patterns[pattern.index()]
    else {
        panic!("the original direct formal retains its admitted binding pattern")
    };
    assert_eq!(expected, Some(slot.ty));
    assert_eq!(frame.slots[receiver.index()].ty, slot.ty);
    let instruction = program
        .instructions
        .iter_mut()
        .find(|instruction| matches!(instruction, AwbcInstruction::LoadConst { .. }))
        .unwrap();
    let AwbcInstruction::LoadConst { dst, .. } = instruction else {
        unreachable!()
    };
    let destination = *dst;
    let label = AwbcStringId(u32::try_from(program.strings.len()).unwrap());
    program.strings.push("id".into());
    *instruction = AwbcInstruction::ProjectField {
        dst: destination,
        target: if inspect {
            AwbcFieldTarget::Inspect(AwbcMutablePlace::Local(receiver))
        } else {
            AwbcFieldTarget::Value(receiver)
        },
        field: AwbcFieldProjection::Named(label),
    };
    program.canonicalize_string_table();
    program
}

#[test]
fn entity_reference_fields_value_and_inspect_preserve_native_awbc_roundtrip_parity() {
    use arcweft_core::engine::{Engine, FlowFiberStatus};
    use arcweft_core::pure::VmRuntimePureCallBackend;
    use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions};
    use arcweft_core::value::RuntimeEntityReference;
    use std::sync::Arc;
    let values = [
        RuntimeEntityReference::Project {
            family: arcweft_id::DeclarationIdentityFamily::Character,
            public_id: arcweft_id::PublicId::try_new("character.alice").unwrap(),
        },
        RuntimeEntityReference::DialogueLine(
            arcweft_core::plan::RuntimeLineId::from_runtime_line_value("line.story.hello").unwrap(),
        ),
        RuntimeEntityReference::CharacterLook {
            character: arcweft_character::id::CharacterId::try_new("character.alice").unwrap(),
            look: arcweft_character::id::CharacterLookId::try_new("normal").unwrap(),
        },
    ];
    for inspect in [false, true] {
        let (plan, program) = reference_field_program(inspect);
        let lowered = AwbcLowerer::new(
            &plan,
            &DialogueContentCatalog::new(),
            "reference-field.arcw",
        )
        .lower()
        .unwrap();
        assert!(lowered.diagnostics.is_empty());
        let projects = lowered
            .program
            .instructions
            .iter()
            .filter_map(|instruction| match instruction {
                AwbcInstruction::ProjectField {
                    target,
                    field: AwbcFieldProjection::Named(_),
                    ..
                } => Some(target),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(projects.len(), 3);
        assert!(
            projects
                .iter()
                .all(|target| matches!(target, AwbcFieldTarget::Inspect(_)) == inspect)
        );
        let encoded = lowered.program.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default()).unwrap();
        assert_eq!(decoded.encode_canonical().unwrap(), encoded);
        decoded
            .verify(
                AwbcVerifyBudget::default(),
                AwbcVerifyContext {
                    require_entrypoint: false,
                    ..Default::default()
                },
            )
            .unwrap();
        let plan = Arc::new(plan);
        let awbc = Arc::new(decoded);
        for value in &values {
            let input = RuntimeValue::EntityRef(value.clone());
            let expected = RuntimeValue::Tuple(vec![
                RuntimeValue::String(
                    value.field_value(arcweft_core::value::RuntimeEntityReferenceField::Id),
                ),
                RuntimeValue::String(
                    value.field_value(arcweft_core::value::RuntimeEntityReferenceField::Family),
                ),
                RuntimeValue::String(
                    value.field_value(arcweft_core::value::RuntimeEntityReferenceField::Name),
                ),
                input.clone(),
            ]);
            let mut native =
                Engine::for_program_invocation(Arc::clone(&plan), program, vec![input.clone()])
                    .unwrap();
            let mut options = RuntimeStepOptions::default();
            options.budget.max_ops = 1;
            for _ in 0..16 {
                let step = native.step(RuntimeStepInput::default(), options);
                assert!(
                    step.output.diagnostics.is_empty(),
                    "{:?}",
                    step.output.diagnostics
                );
                if matches!(native.fiber().status, FlowFiberStatus::Done(_)) {
                    break;
                }
            }
            assert_eq!(
                native.take_program_result().unwrap(),
                Some((program, expected.clone())),
                "inspect={inspect}, {value:?}"
            );
            assert_eq!(
                arcweft_core::awbc::product_step::evaluate_pure_program_with_backend(
                    &awbc,
                    program,
                    &[input],
                    &mut VmRuntimePureCallBackend::default(),
                )
                .unwrap(),
                expected,
                "inspect={inspect}, {value:?}"
            );
        }
    }
}

#[test]
fn entity_reference_named_awbc_fields_reject_unknown_names_wrong_targets_and_results() {
    for inspect in [false, true] {
        let (program, _) = reference_field_awbc(inspect);
        let mut unknown = program.clone();
        let label = AwbcStringId(u32::try_from(unknown.strings.len()).unwrap());
        unknown.strings.push("parent_id".into());
        let projection = unknown
            .instructions
            .iter_mut()
            .find_map(|instruction| match instruction {
                AwbcInstruction::ProjectField { field, .. } => Some(field),
                _ => None,
            })
            .unwrap();
        *projection = AwbcFieldProjection::Named(label);
        unknown.canonicalize_string_table();
        assert!(
            matches!(unknown.verify(AwbcVerifyBudget::default(), AwbcVerifyContext { require_entrypoint: false, ..Default::default() }),
            Err(AwbcVerifyError::InvalidInvariant { message, .. }) if message == "projected entity-reference field does not exist")
        );

        let mut wrong_result = program.clone();
        let boolean = arcweft_core::awbc::schema::AwbcTypeId(
            u32::try_from(wrong_result.runtime_types.len()).unwrap(),
        );
        wrong_result
            .runtime_types
            .push(arcweft_core::awbc::schema::AwbcRuntimeType::new(
                identity(11),
                arcweft_core::awbc::schema::AwbcRuntimeTypeShape::Bool,
            ));
        let destination = wrong_result
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                AwbcInstruction::ProjectField { dst, .. } => Some(*dst),
                _ => None,
            })
            .unwrap();
        wrong_result.frame_layouts[0].slots[destination.index()].ty = boolean;
        assert!(
            matches!(wrong_result.verify(AwbcVerifyBudget::default(), AwbcVerifyContext { require_entrypoint: false, ..Default::default() }),
            Err(AwbcVerifyError::InvalidInvariant { message, .. }) if message == "entity-reference field projection destination")
        );

        let wrong_owner = wrong_owner_reference_field_awbc(inspect);
        let outcome = wrong_owner.verify(
            AwbcVerifyBudget::default(),
            AwbcVerifyContext {
                require_entrypoint: false,
                ..Default::default()
            },
        );
        assert!(
            matches!(&outcome, Err(AwbcVerifyError::InvalidInvariant { message, .. })
                if message == "record projection target"),
            "the actual Bool receiver must reject at the field owner, inspect={inspect}: {outcome:?}"
        );
    }
}
