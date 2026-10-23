use super::*;
use crate::awbc::schema::*;
use crate::awbc::verify::{AwbcVerifyBudget, AwbcVerifyContext};
use crate::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId};
use crate::plan::construction::source::{host_calls, selected_producers};
use crate::plan::task_source::RuntimeTaskSourceArtifactLimits;
use crate::plan::*;
use crate::task::*;
use crate::value::{RuntimeLocalReadMode, RuntimeValue};
use std::sync::Arc;

fn boolean() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Bool.semantic_identity_digest()
}
fn unit() -> RuntimeSemanticTypeId {
    RuntimeCheckedType::Unit.semantic_identity_digest()
}
fn sequence() -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([0x91; 32])
}
fn definition(marker: u8) -> RuntimeFunctionDefinitionIdentity {
    RuntimeFunctionDefinitionIdentity::from_accepted_identity([marker; 32])
}

pub(in crate::plan::task_source) fn original() -> RuntimePlan {
    original_with_other_child(false)
}

fn original_with_other_child(other: bool) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(unit(), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(boolean(), RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    sequence(),
                    RuntimePlanTypeProjection::Sequence {
                        kind: RuntimePlanSequenceKind::Vec,
                        item: boolean(),
                    },
                ),
            ],
            (1..=if other { 6 } else { 3 }).map(|marker| {
                RuntimeLocalDeclarationSeed::new(
                    RuntimeLocalDeclarationSource::Binding {
                        identity: [marker; 32],
                        declaration: RuntimeLocalBindingDeclaration::new(
                            RuntimeLocalBindingKind::PatternBinding,
                            false,
                            RuntimeLocalBindingStorage::Derived,
                        ),
                    },
                    boolean(),
                )
            }),
        )
        .unwrap();
    let mut functions = Vec::new();
    for marker in 1..=if other { 6 } else { 3 } {
        let result = match marker % 3 {
            1 => unit(),
            2 => sequence(),
            _ => boolean(),
        };
        functions.push(
            builder
                .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                    definition: definition(marker),
                    role: RuntimeFunctionSemanticRole::Ordinary,
                    function_type: None,
                    inputs: Box::new([]),
                    result,
                    body_kind: RuntimeFunctionSiteBodyKind::Executable,
                    effects: RuntimeEffectSet::empty(),
                })
                .unwrap(),
        );
    }
    let scalar = RuntimeExprSeed::new(
        boolean(),
        RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
    );
    let source = RuntimeExprSeed::new(
        sequence(),
        RuntimeExprSeedKind::BracketSeq(Box::new([scalar.clone()])),
    );
    let returned = RuntimeExprSeed::new(unit(), RuntimeExprSeedKind::Value(RuntimeValue::Unit));
    let mut returns = vec![(1, source.clone()), (2, scalar.clone())];
    if other {
        returns.extend([(4, source.clone()), (5, scalar.clone())]);
    }
    for (function, value) in returns {
        builder
            .define_function_site_seed(
                &functions[function],
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(value)]),
                }),
            )
            .unwrap();
    }
    let locals = admitted.local_ids();
    let argument = |marker, local: &RuntimeLocalSeedId, named: bool| {
        let value = RuntimeExprSeed::new(
            boolean(),
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                local.clone(),
                RuntimeLocalReadMode::Copy,
            )),
        );
        let role = RuntimeRequestRoleIdentity::from_accepted_identity([marker; 32]);
        if named {
            RuntimeHostArgumentTemplate::named(role, "条件", value)
        } else {
            RuntimeHostArgumentTemplate::positional(role, value)
        }
    };
    for group in 0..=usize::from(other) {
        let locals = &locals[group * 3..group * 3 + 3];
        let base_coordinate = builder
            .task_plan_construction()
            .reserve_original_source()
            .unwrap();
        let child_coordinate = builder
            .task_plan_construction()
            .reserve_original_source()
            .unwrap();
        let producer = |marker, family, plan, payload: RuntimeSemanticTypeId, args| {
            RuntimeNeedProducerTemplateSeed {
                definition: crate::task::NeedProducerDefinition {
                    function: definition(marker),
                    family: family,
                    contract: NeedProducerContractDigest::from_bytes([marker; 32]),
                    plan: plan,
                    producer_site: NeedProducerSiteDigest::from_bytes([marker; 32]),
                    payload_type: RuntimeTypeSemanticDigest::from_bytes(*payload.as_bytes()),
                    class: TaskClass::Cpu,
                    priority: TaskPriority(-3),
                    cancel_scope: CancelScopeId("元scope".into()),
                    policy: TaskPolicy::AlwaysStart,
                    debug_label: "template source fixture".into(),
                },
                request: RuntimeHostTaskRequestTemplateSeed {
                    capability: HostCapabilityId("source".into()),
                    operation: "request".into(),
                    args,
                },
            }
        };
        let mut ops = Vec::new();
        for local in &locals[..2] {
            ops.push(RuntimeFlowOpSeed::Let {
                pattern: RuntimePatternSeed::new(
                    boolean(),
                    RuntimePatternSeedKind::Bind {
                        local: local.clone(),
                        mutable: false,
                    },
                ),
                expr: scalar.clone(),
            });
        }
        ops.push(RuntimeFlowOpSeed::AwaitMany {
            binding: None,
            target: RuntimeAwaitManyTargetSeed {
                source: source.clone(),
                item_binding: locals[2].clone(),
                limit: 2,
                base: producer(
                    u8::try_from(group * 3 + 2).unwrap(),
                    NeedProducerFamily::AwaitManyBase,
                    base_coordinate,
                    sequence(),
                    vec![
                        argument(10, &locals[0], false),
                        argument(11, &locals[1], true),
                    ],
                ),
                child: producer(
                    u8::try_from(group * 3 + 3).unwrap(),
                    NeedProducerFamily::AwaitManyChild,
                    child_coordinate,
                    boolean(),
                    vec![
                        argument(12, &locals[2], false),
                        argument(13, &locals[0], true),
                    ],
                ),
            },
            pending: vec![],
        });
        ops.push(RuntimeFlowOpSeed::ReturnExpr(returned.clone()));
        builder
            .define_function_site_seed(
                &functions[group * 3],
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: ops.into_boxed_slice(),
                }),
            )
            .unwrap();
    }
    builder.finish().unwrap()
}

fn target(plan: &RuntimePlan) -> &AwaitManyTarget {
    let FlowOp::AwaitMany { target, .. } = &plan
        .function_sites()
        .iter()
        .next()
        .unwrap()
        .body()
        .executable()
        .unwrap()
        .ops()[2]
    else {
        unreachable!()
    };
    target
}

pub(in crate::plan::task_source) fn executable() -> AwbcProgram {
    let original = original();
    let target = target(&original);
    let mut strings = vec![
        "source".into(),
        "request".into(),
        "元scope".into(),
        "条件".into(),
        "別".into(),
    ];
    strings.sort();
    let string = |value: &str| {
        AwbcStringId(
            u32::try_from(strings.iter().position(|actual| actual == value).unwrap()).unwrap(),
        )
    };
    let mut code = AwbcProgram {
        original_source: Some(
            original
                .task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
                .unwrap(),
        ),
        runtime_types: vec![
            AwbcRuntimeType::new(boolean(), AwbcRuntimeTypeShape::Bool),
            AwbcRuntimeType::new(
                sequence(),
                AwbcRuntimeTypeShape::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item: AwbcTypeId(0),
                },
            ),
            AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x92; 32]),
                AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0); 2]),
            ),
        ],
        effect_sets: vec![AwbcEffectSet::default()],
        signatures: vec![AwbcSignature {
            params: vec![AwbcTypeId(0); 2],
            result: None,
            effects: AwbcEffectSetId(0),
        }],
        ..AwbcProgram::default()
    };
    let captures = target.captures().collect::<Vec<_>>();
    for (index, producer) in [&target.base, &target.child].into_iter().enumerate() {
        let mut inputs = captures.clone();
        if index == 1 {
            inputs.push(target.item_binding);
        }
        let count = u32::try_from(inputs.len()).unwrap();
        let result = AwbcTypeId(2);
        let signature = AwbcSignatureId(u32::try_from(code.signatures.len()).unwrap());
        code.signatures.push(AwbcSignature {
            params: vec![AwbcTypeId(0); inputs.len()],
            result: Some(result),
            effects: AwbcEffectSetId(0),
        });
        let mut slots = vec![
            AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0
            };
            inputs.len()
        ];
        slots.push(AwbcFrameSlot {
            name: None,
            ty: result,
            role: AwbcFrameSlotRole::Temporary,
            scope_depth: 0,
        });
        let function = AwbcFunctionId(u32::try_from(index).unwrap());
        let block = AwbcBlockId(u32::try_from(index).unwrap());
        code.frame_layouts.push(AwbcFrameLayout {
            slots,
            scopes: vec![],
            max_scope_depth: 0,
        });
        code.functions.push(AwbcFunction {
            definition: producer
                .definition
                .function
                .generated_child(RuntimeGeneratedFunctionRole::TaskRequest),
            semantic_role: RuntimeFunctionSemanticRole::Ordinary,
            public_id: None,
            kind: AwbcFunctionKind::Ordinary,
            signature,
            type_context: None,
            input_ownership: inputs
                .iter()
                .enumerate()
                .map(|(position, local)| {
                    AwbcFunctionInputOwnership::capture(
                        original.local_declarations().get(*local).unwrap().origin(),
                        u32::try_from(position).unwrap(),
                        RuntimeFunctionCaptureMode::Copy,
                    )
                })
                .collect(),
            frame_layout: AwbcFrameLayoutId(u32::try_from(index).unwrap()),
            blocks: AwbcTableRange::new(block.0, 1),
            entry_block: block,
            flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
        });
        code.instructions.push(AwbcInstruction::MakeTuple {
            dst: AwbcRegisterId(count),
            items: if index == 0 {
                vec![AwbcRegisterId(0), AwbcRegisterId(1)]
            } else {
                vec![AwbcRegisterId(2), AwbcRegisterId(0)]
            },
        });
        code.blocks.push(AwbcBlock {
            owner: function,
            instructions: AwbcTableRange::new(u32::try_from(index).unwrap(), 1),
            terminator: AwbcTerminator::Return {
                value: Some(AwbcRegisterId(count)),
            },
            safe_point: AwbcSafePointKind::CallableBoundary,
            source_map: None,
        });
        code.task_plans.push(AwbcTaskPlan {
            class: AwbcTaskClass::Cpu,
            priority: -3,
            cancel_scope: string("元scope"),
            policy: AwbcTaskPolicy::AlwaysStart,
            payload_type: AwbcTypeId(if index == 0 { 1 } else { 0 }),
            execution: crate::awbc::schema::AwbcTaskPlanExecution::Host(
                crate::awbc::schema::AwbcHostTaskPlan {
                    signature: AwbcSignatureId(0),
                    request: AwbcTaskRequestProjection::CustomCapability {
                        capability: string("source"),
                        operation: string("request"),
                    },
                    arguments: vec![
                        AwbcHostArgument {
                            name: None,
                            spread: false,
                        },
                        AwbcHostArgument {
                            name: Some(string("条件")),
                            spread: false,
                        },
                    ],
                    kind: if index == 0 {
                        AwbcHostTaskPlanKind::AwaitMany {
                            contract: producer.definition.contract,
                            site: producer.definition.producer_site,
                            plan: original.task_plan(&producer.definition.plan).unwrap().key(),
                            child: AwbcTaskPlanId(1),
                            captures: vec![AwbcRegisterId(0), AwbcRegisterId(1)],
                            request_function: function,
                            item_binding: AwbcRegisterId(2),
                            limit: target.limit,
                        }
                    } else {
                        AwbcHostTaskPlanKind::Template {
                            family: producer.definition.family,
                            contract: producer.definition.contract,
                            site: producer.definition.producer_site,
                            plan: original.task_plan(&producer.definition.plan).unwrap().key(),
                            request_function: function,
                        }
                    },
                },
            ),
        });
    }
    code.strings = strings;
    code
}

fn admit(code: AwbcProgram) -> RuntimeTaskSourceProgram {
    RuntimeTaskSourceProgram::admit_awbc(
        Arc::new(code),
        AwbcVerifyBudget::default(),
        AwbcVerifyContext {
            require_entrypoint: false,
            ..AwbcVerifyContext::default()
        },
        RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap()
}
fn meter() -> TaskSemanticMeter {
    TaskSemanticMeter::new(1_000_000, 1_000_000)
}

#[test]
fn awbc_templates_resolve_both_original_rows_and_capture_abis_after_decode() {
    let bytes = executable().encode_canonical().unwrap();
    let program = admit(
        AwbcProgram::decode_canonical(&bytes, crate::awbc::codec::AwbcDecodeBudget::default())
            .unwrap(),
    );
    for task in [AwbcTaskPlanId(0), AwbcTaskPlanId(1)] {
        let mut work = meter();
        let source = program
            .awbc_template_source(task, RuntimeTaskPlanSealLimits::default(), &mut work)
            .unwrap();
        assert!(source.task_plan().belongs_to(program.original()));
        let expected = if task.0 == 0 {
            &target(program.original()).base
        } else {
            &target(program.original()).child
        };
        assert_eq!(
            source.task_plan().key(),
            program
                .original()
                .task_plan(&expected.definition.plan)
                .unwrap()
                .key()
        );
        assert_eq!(work.totals().1, 0);
    }
}

#[test]
fn awbc_template_owning_source_preserves_checked_rows_and_rechecks_the_new_allowance() {
    let program = admit(executable());
    for task in [AwbcTaskPlanId(0), AwbcTaskPlanId(1)] {
        let bound = program
            .awbc_template_source(task, RuntimeTaskPlanSealLimits::default(), &mut meter())
            .unwrap();
        let owned = crate::plan::task_source::RuntimeTaskSource::from_borrowed(
            &bound,
            &program,
            &program.program_owner(),
        )
        .unwrap();
        let borrowed = owned
            .borrow(RuntimeTaskPlanSealLimits::default(), &mut meter())
            .unwrap();
        assert!(program.owns_source_for(&borrowed, &program.program_owner()));
        assert_eq!(borrowed.task_plan().key(), bound.task_plan().key());
        assert!(matches!(
            owned.borrow(
                RuntimeTaskPlanSealLimits {
                    max_request_roles: 1,
                    ..RuntimeTaskPlanSealLimits::default()
                },
                &mut meter()
            ),
            Err(super::super::RuntimeTaskSourceBorrowError::Template(
                RuntimeAwbcTemplateSourceError::Roles {
                    field: RuntimeAwbcTemplateSourceField::ArgumentCount,
                    actual: 2,
                    maximum: 1,
                    ..
                }
            ))
        ));
    }
}

#[test]
fn awbc_templates_reject_foreign_keys_without_metadata_fallback() {
    for task in [AwbcTaskPlanId(0), AwbcTaskPlanId(1)] {
        let mut code = executable();
        match &mut code.task_plans[task.index()]
            .host_mut()
            .expect("Host fixture")
            .kind
        {
            AwbcHostTaskPlanKind::AwaitMany { plan, .. }
            | AwbcHostTaskPlanKind::Template { plan, .. } => {
                *plan = TaskPlanSemanticDigest::from_bytes([0; 32])
            }
            _ => unreachable!(),
        }
        let program = admit(code);
        let mut work = meter();
        assert!(matches!(
            program.awbc_template_source(task, RuntimeTaskPlanSealLimits::default(), &mut work),
            Err(RuntimeAwbcTemplateSourceError::Source {
                source: RuntimeTaskSourceError::MissingExpectedKey,
                ..
            })
        ));
        assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
}

#[test]
fn awbc_templates_reject_same_id_different_executable_payload_and_capture_shapes() {
    let mut code = executable();
    code.runtime_types[0] = AwbcRuntimeType::new(boolean(), AwbcRuntimeTypeShape::String);
    // Every code-local parameter, tuple and aggregate stays internally valid;
    // only its asserted correspondence with the original Bool is false.
    let program = admit(code);
    let mut work = meter();
    assert!(matches!(
        program.awbc_template_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut work
        ),
        Err(RuntimeAwbcTemplateSourceError::Type {
            field: RuntimeAwbcTemplateSourceField::Payload,
            ..
        })
    ));
    assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
}

#[test]
fn awbc_templates_check_argument_and_capture_types_after_valid_payloads() {
    for argument in [false, true] {
        let mut code = executable();
        let string = AwbcTypeId(u32::try_from(code.runtime_types.len()).unwrap());
        code.runtime_types.push(AwbcRuntimeType::new(
            RuntimeCheckedType::String.semantic_identity_digest(),
            AwbcRuntimeTypeShape::String,
        ));
        for index in [0, 1] {
            code.signatures[index + 1].params[1] = string;
            code.frame_layouts[index].slots[1].ty = string;
        }
        if argument {
            code.signatures[0].params[1] = string;
            code.runtime_types[2] = AwbcRuntimeType::new(
                RuntimeSemanticTypeId::from_bytes([0x92; 32]),
                AwbcRuntimeTypeShape::Tuple(vec![AwbcTypeId(0), string]),
            );
            let AwbcInstruction::MakeTuple { items, .. } = &mut code.instructions[1] else {
                unreachable!()
            };
            *items = vec![AwbcRegisterId(2), AwbcRegisterId(1)];
        } else {
            code.frame_layouts[0].slots.push(AwbcFrameSlot {
                name: None,
                ty: AwbcTypeId(0),
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            });
            let AwbcInstruction::MakeTuple { items, .. } = &mut code.instructions[0] else {
                unreachable!()
            };
            *items = vec![AwbcRegisterId(0), AwbcRegisterId(3)];
            code.instructions.insert(
                0,
                AwbcInstruction::CopyValue {
                    dst: AwbcRegisterId(3),
                    src: AwbcRegisterId(0),
                },
            );
            code.blocks[0].instructions = AwbcTableRange::new(0, 2);
            code.blocks[1].instructions = AwbcTableRange::new(2, 1);
        }
        let program = admit(code);
        let mut work = meter();
        let expected = if argument {
            RuntimeAwbcTemplateSourceField::ArgumentType { index: 1 }
        } else {
            RuntimeAwbcTemplateSourceField::CaptureType { index: 1 }
        };
        assert!(
            matches!(program.awbc_template_source(AwbcTaskPlanId(0), RuntimeTaskPlanSealLimits::default(), &mut work), Err(RuntimeAwbcTemplateSourceError::Type { field, .. }) if field == expected)
        );
        assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
}

#[test]
fn awbc_await_many_rejects_another_genuine_admitted_child_from_the_same_source() {
    let original = original_with_other_child(true);
    let other = original
        .function_sites()
        .iter()
        .find(|function| function.definition() == definition(4))
        .unwrap();
    let FlowOp::AwaitMany { target: other, .. } = &other.body().executable().unwrap().ops()[2]
    else {
        unreachable!()
    };
    let mut code = executable();
    code.original_source = Some(
        original
            .task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
            .unwrap(),
    );
    let AwbcHostTaskPlanKind::AwaitMany { plan, .. } =
        &mut code.task_plans[0].host_mut().expect("Host fixture").kind
    else {
        unreachable!()
    };
    *plan = original
        .task_plan(&target(&original).base.definition.plan)
        .unwrap()
        .key();
    let AwbcHostTaskPlanKind::Template {
        plan,
        contract,
        site,
        ..
    } = &mut code.task_plans[1].host_mut().expect("Host fixture").kind
    else {
        unreachable!()
    };
    *plan = original
        .task_plan(&other.child.definition.plan)
        .unwrap()
        .key();
    *contract = other.child.definition.contract;
    *site = other.child.definition.producer_site;
    code.functions[1].definition = other
        .child
        .definition
        .function
        .generated_child(RuntimeGeneratedFunctionRole::TaskRequest);
    code.functions[1].input_ownership = other
        .captures()
        .chain(std::iter::once(other.item_binding))
        .enumerate()
        .map(|(position, local)| {
            AwbcFunctionInputOwnership::capture(
                original.local_declarations().get(local).unwrap().origin(),
                u32::try_from(position).unwrap(),
                RuntimeFunctionCaptureMode::Copy,
            )
        })
        .collect();
    let program = admit(code);
    // The replacement is a genuine independently admitted child in this same
    // namespace, but it is not the child of the asserted original parent.
    program
        .awbc_template_source(
            AwbcTaskPlanId(1),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter(),
        )
        .unwrap();
    let mut work = meter();
    assert!(matches!(
        program.awbc_template_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut work
        ),
        Err(RuntimeAwbcTemplateSourceError::Mismatch {
            field: RuntimeAwbcTemplateSourceField::ChildKey,
            ..
        })
    ));
    assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
}

#[test]
fn awbc_templates_reject_changed_original_family_and_base_row_kind() {
    for child in [false, true] {
        let mut code = executable();
        let AwbcHostTaskPlanKind::AwaitMany {
            contract,
            site,
            plan,
            request_function,
            ..
        } = code.task_plans[0].host_mut().expect("Host fixture").kind
        else {
            unreachable!()
        };
        code.task_plans[0].host_mut().expect("Host fixture").kind =
            AwbcHostTaskPlanKind::Template {
                family: NeedProducerFamily::AwaitManyBase,
                contract,
                site,
                plan,
                request_function,
            };
        let task = if child {
            let AwbcHostTaskPlanKind::Template { family, .. } =
                &mut code.task_plans[1].host_mut().expect("Host fixture").kind
            else {
                unreachable!()
            };
            *family = NeedProducerFamily::StructuredTaskPlan;
            AwbcTaskPlanId(1)
        } else {
            AwbcTaskPlanId(0)
        };
        let program = admit(code);
        assert!(
            matches!(program.awbc_template_source(task, RuntimeTaskPlanSealLimits::default(), &mut meter()), Err(RuntimeAwbcTemplateSourceError::Mismatch { field, .. }) if field == if child { RuntimeAwbcTemplateSourceField::Family } else { RuntimeAwbcTemplateSourceField::RequestKind })
        );
    }
}

#[test]
fn awbc_templates_reject_independently_valid_metadata_and_capture_changes() {
    use RuntimeAwbcTemplateSourceField as Field;
    for field in [
        Field::Contract,
        Field::Site,
        Field::Capability,
        Field::Operation,
        Field::Class,
        Field::Priority,
        Field::CancelScope,
        Field::Policy,
        Field::ArgumentName { index: 1 },
        Field::ArgumentSpread { index: 0 },
        Field::RequestDefinition,
        Field::Capture { index: 0 },
        Field::Limit,
    ] {
        let mut code = executable();
        let wrong = AwbcStringId(
            u32::try_from(code.strings.iter().position(|text| text == "別").unwrap()).unwrap(),
        );
        let row = &mut code.task_plans[0];
        let crate::awbc::schema::AwbcTaskPlanExecution::Host(host) = &mut row.execution else {
            panic!("fixture owns a Host execution contract")
        };
        match field {
            Field::Contract => {
                let AwbcHostTaskPlanKind::AwaitMany { contract, .. } = &mut host.kind else {
                    unreachable!()
                };
                *contract = NeedProducerContractDigest::from_bytes([0; 32]);
            }
            Field::Site => {
                let AwbcHostTaskPlanKind::AwaitMany { site, .. } = &mut host.kind else {
                    unreachable!()
                };
                *site = NeedProducerSiteDigest::from_bytes([0; 32]);
            }
            Field::Capability => {
                let AwbcTaskRequestProjection::CustomCapability { capability, .. } =
                    &mut host.request
                else {
                    unreachable!()
                };
                *capability = wrong;
            }
            Field::Operation => {
                let AwbcTaskRequestProjection::CustomCapability { operation, .. } =
                    &mut host.request
                else {
                    unreachable!()
                };
                *operation = wrong;
            }
            Field::Class => row.class = AwbcTaskClass::Io,
            Field::Priority => row.priority += 1,
            Field::CancelScope => row.cancel_scope = wrong,
            Field::Policy => row.policy = AwbcTaskPolicy::JoinSameKey,
            Field::ArgumentName { index } => host.arguments[index].name = None,
            Field::ArgumentSpread { index } => host.arguments[index].spread = true,
            Field::RequestDefinition => code.functions[0].definition = definition(99),
            Field::Capture { .. } => {
                code.functions[0].input_ownership[0].origin =
                    code.functions[0].input_ownership[1].origin
            }
            Field::Limit => {
                let AwbcHostTaskPlanKind::AwaitMany { limit, .. } = &mut host.kind else {
                    unreachable!()
                };
                *limit += 1;
            }
            _ => unreachable!(),
        }
        let program = admit(code);
        let mut work = meter();
        assert!(
            matches!(program.awbc_template_source(AwbcTaskPlanId(0), RuntimeTaskPlanSealLimits::default(), &mut work), Err(RuntimeAwbcTemplateSourceError::Mismatch { field: actual, .. }) if actual == field),
            "{field:?}"
        );
        assert_eq!(work.status(), Err(TaskSemanticEncodingError::OwnerRejected));
    }
}

#[test]
fn awbc_templates_share_exact_prior_and_first_failed_work() {
    let program = admit(executable());
    let limits = RuntimeTaskPlanSealLimits::default();
    let mut measured = meter();
    program
        .awbc_template_source(AwbcTaskPlanId(0), limits, &mut measured)
        .unwrap();
    let (work, bytes) = measured.totals();
    assert!(work > 1);
    assert_eq!(bytes, 0);
    let mut exact = TaskSemanticMeter::new(work, 0);
    program
        .awbc_template_source(AwbcTaskPlanId(0), limits, &mut exact)
        .unwrap();
    assert_eq!(exact.totals(), (work, 0));
    let mut prefixed = TaskSemanticMeter::new(work + 1, 0);
    prefixed.charge_work(1).unwrap();
    program
        .awbc_template_source(AwbcTaskPlanId(0), limits, &mut prefixed)
        .unwrap();
    assert_eq!(prefixed.totals(), (work + 1, 0));
    let mut short = TaskSemanticMeter::new(work - 1, 0);
    for _ in 0..2 {
        assert!(matches!(
            program.awbc_template_source(AwbcTaskPlanId(0), limits, &mut short),
            Err(RuntimeAwbcTemplateSourceError::Encoding(
                TaskSemanticEncodingError::SemanticWork
            ))
        ));
        assert_eq!(short.status(), Err(TaskSemanticEncodingError::SemanticWork));
    }
    let native = RuntimeTaskSourceProgram::native(Arc::new(original()));
    assert!(matches!(
        native.awbc_template_source(AwbcTaskPlanId(99), limits, &mut short),
        Err(RuntimeAwbcTemplateSourceError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
    let mut tightened = meter();
    tightened.charge_work(1).unwrap();
    assert!(matches!(
        program.awbc_template_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits {
                max_semantic_work: 0,
                ..limits
            },
            &mut tightened
        ),
        Err(RuntimeAwbcTemplateSourceError::Encoding(
            TaskSemanticEncodingError::SemanticWork
        ))
    ));
}

#[test]
fn awbc_templates_bound_roles_before_a_forged_key_and_admit_shared_capture_prefixes() {
    let program = admit(executable());
    let exact = RuntimeTaskPlanSealLimits {
        max_function_roles: 3,
        ..RuntimeTaskPlanSealLimits::default()
    };
    program
        .awbc_template_source(AwbcTaskPlanId(0), exact, &mut meter())
        .unwrap();
    let mut code = executable();
    let AwbcHostTaskPlanKind::AwaitMany { plan, .. } =
        &mut code.task_plans[0].host_mut().expect("Host fixture").kind
    else {
        unreachable!()
    };
    *plan = TaskPlanSemanticDigest::from_bytes([0; 32]);
    let program = admit(code);
    assert!(matches!(
        program.awbc_template_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits {
                max_request_roles: 1,
                ..exact
            },
            &mut meter()
        ),
        Err(RuntimeAwbcTemplateSourceError::Roles {
            field: RuntimeAwbcTemplateSourceField::ArgumentCount,
            actual: 2,
            maximum: 1,
            ..
        })
    ));
}

#[test]
fn awbc_templates_reject_real_other_endpoint_kinds_and_absent_or_native_rows() {
    let native = RuntimeTaskSourceProgram::native(Arc::new(original()));
    assert!(matches!(
        native.awbc_template_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(RuntimeAwbcTemplateSourceError::Backend)
    ));
    let program = admit(executable());
    assert!(matches!(
        program.awbc_template_source(
            AwbcTaskPlanId(2),
            RuntimeTaskPlanSealLimits::default(),
            &mut meter()
        ),
        Err(RuntimeAwbcTemplateSourceError::MissingTask { .. })
    ));
    for original in [
        host_calls::tests::fixture(crate::step::RuntimeHostCallMode::Suspend, true, true, false),
        selected_producers::tests::fixture(
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::MustBeQuiescent,
            TaskClass::Cpu,
        ),
    ] {
        let mut code = executable();
        code.original_source = Some(
            original
                .task_source_artifact(RuntimeTaskSourceArtifactLimits::default())
                .unwrap(),
        );
        let AwbcHostTaskPlanKind::AwaitMany { plan, .. } =
            &mut code.task_plans[0].host_mut().expect("Host fixture").kind
        else {
            unreachable!()
        };
        *plan = original
            .task_plans()
            .by_index(RuntimeTaskPlanIndex(0))
            .unwrap()
            .key();
        let program = admit(code);
        assert!(matches!(
            program.awbc_template_source(
                AwbcTaskPlanId(0),
                RuntimeTaskPlanSealLimits::default(),
                &mut meter()
            ),
            Err(RuntimeAwbcTemplateSourceError::Mismatch {
                field: RuntimeAwbcTemplateSourceField::RequestKind,
                ..
            })
        ));
    }
}
