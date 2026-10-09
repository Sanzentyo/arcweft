use super::*;

use arcweft_core::awbc::fiber::{AwbcFiberStateSnapshot, FiberState};
use arcweft_core::awbc::schema::{
    AwbcBlockId, AwbcEntryId, AwbcEntryTarget, AwbcInstruction, AwbcMutablePlace, AwbcPattern,
    AwbcProgram, AwbcRuntimeTypeShape, AwbcSafePointKind, AwbcTerminator,
};
use arcweft_core::awbc::vm::{self, VmExit, VmStepOptions};
use arcweft_core::entry::{
    EntryBindingIdentity, FlowContractHash, RuntimeDialogueContentTemplateDigest,
    RuntimeEntryRoles, RuntimeFlowExecutable,
};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeAwaitPendingObserverSeed, RuntimeAwaitTargetSeed,
    RuntimeDialogueContentPlanSeed, RuntimeDialogueContentTemplateManifestSeed, RuntimeEntryKind,
    RuntimeEntrySpec, RuntimeEntryTarget, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed,
    RuntimeFlowSchema, RuntimeFlowSeed, RuntimeHostCallTargetSeed, RuntimeHttpMethod,
    RuntimeLineTaskCancelRuleSeed, RuntimeLineTaskGroupSeed, RuntimeLineTaskNodeSeed,
    RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed, RuntimeLocalSeedId, RuntimePatternSeed,
    RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuildError, RuntimePlanBuilder,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimePureHelperOrigin, RuntimePureHelperSeed,
    RuntimePureInputType, RuntimePureOutputType, RuntimeReceiverMode, RuntimeRoutePath,
    RuntimeRoutePathSegment, RuntimeRouteSpec, RuntimeTraitMethodIdentity, RuntimeTraitMethodSeed,
};
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::value::{RuntimeLocalReadMode, RuntimeValue};
use std::sync::Arc;

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::canonical(value).expect("test flow ID is valid")
}

fn entry_id(value: &str) -> EntryRuntimeId {
    EntryRuntimeId::canonical(value).expect("test entry ID is valid")
}

fn type_id(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn string_expr(value: &str) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        type_id(1),
        RuntimeExprSeedKind::Value(RuntimeValue::String(value.to_owned())),
    )
}

fn unit_expr() -> RuntimeExprSeed {
    RuntimeExprSeed::new(type_id(2), RuntimeExprSeedKind::Value(RuntimeValue::Unit))
}

fn bool_expr(value: bool) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        type_id(3),
        RuntimeExprSeedKind::Value(RuntimeValue::Bool(value)),
    )
}

fn flow_schema(flow: &FlowRuntimeId) -> RuntimeFlowSchema {
    RuntimeFlowSchema {
        flow: flow.clone(),
        parameters: Vec::new(),
    }
}

fn flow_executable(flow: &FlowRuntimeId) -> RuntimeFlowExecutable {
    RuntimeFlowExecutable {
        flow: flow.clone(),
        contract: FlowContractHash::from_bytes([0xf0; 32]),
        controller: None,
    }
}

fn build_plan(
    result: RuntimeSemanticTypeId,
    flows: impl IntoIterator<Item = (FlowRuntimeId, Vec<RuntimeFlowOpSeed>)>,
    entries: impl IntoIterator<Item = RuntimeEntrySpec>,
) -> RuntimePlan {
    let flows = flows.into_iter().collect::<Vec<_>>();
    let entries = entries.into_iter().collect::<Vec<_>>();
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(type_id(2), RuntimePlanTypeProjection::Unit),
            ],
            [],
        )
        .expect("test semantic facts admit");
    for (id, ops) in flows {
        builder
            .push_flow_schema(flow_schema(&id))
            .expect("test flow schema admits");
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                id,
                arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                    arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                        [61; 32],
                    ),
                    None,
                    Box::new([]),
                    result,
                    arcweft_core::plan::RuntimeEffectSet::empty(),
                ),
                arcweft_core::plan::RuntimeExecutableBodySeed {
                    effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                    ops: (ops).into_boxed_slice(),
                },
            ))
            .expect("test flow admits");
    }
    let mut executable_flows = Vec::new();
    for entry in &entries {
        match &entry.target {
            RuntimeEntryTarget::Flow(flow) | RuntimeEntryTarget::Controller(flow) => {
                if !executable_flows.contains(flow) {
                    builder
                        .push_flow_executable(flow_executable(flow))
                        .expect("test flow executable admits");
                    executable_flows.push(flow.clone());
                }
            }
            RuntimeEntryTarget::Routes(routes) => {
                for route in routes {
                    if !executable_flows.contains(&route.target) {
                        builder
                            .push_flow_executable(flow_executable(&route.target))
                            .expect("test route flow executable admits");
                        executable_flows.push(route.target.clone());
                    }
                }
            }
        }
    }
    for entry in entries {
        builder.push_entry(entry).expect("test entry admits");
    }
    builder.finish().expect("test runtime plan seals")
}

fn build_bool_flow_plan(
    result: RuntimeSemanticTypeId,
    flow: FlowRuntimeId,
    ops: Vec<RuntimeFlowOpSeed>,
) -> RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(type_id(2), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(type_id(3), RuntimePlanTypeProjection::Bool),
            ],
            [],
        )
        .expect("boolean loop test types admit");
    builder
        .push_flow_executable(flow_executable(&flow))
        .expect("boolean loop test flow executable admits");
    builder
        .push_flow_schema(flow_schema(&flow))
        .expect("boolean loop test flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                result,
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (ops).into_boxed_slice(),
            },
        ))
        .expect("boolean loop test flow admits");
    builder
        .push_entry(flow_entry("loop", flow))
        .expect("boolean loop test entry admits");
    builder.finish().expect("boolean loop test plan seals")
}

fn option_bool_pattern(
    option_type: RuntimeSemanticTypeId,
    payload_type: RuntimeSemanticTypeId,
    bool_type: RuntimeSemanticTypeId,
    kind: RuntimePatternSeedKind,
) -> RuntimePatternSeed {
    RuntimePatternSeed::new(
        option_type,
        RuntimePatternSeedKind::Variant {
            ordinal: 0,
            payload: Some(Box::new(RuntimePatternSeed::new(
                payload_type,
                RuntimePatternSeedKind::Tuple(Box::new([RuntimePatternSeed::new(bool_type, kind)])),
            ))),
        },
    )
}

fn build_while_let_plan(
    value: RuntimeValue,
    include_guard: bool,
    body: Vec<RuntimeFlowOpSeed>,
) -> RuntimePlan {
    let flow = flow_id("while_let");
    let bool_type = type_id(3);
    let payload_type = type_id(4);
    let option_type = type_id(5);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(type_id(2), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(bool_type, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    payload_type,
                    RuntimePlanTypeProjection::Tuple(Box::new([bool_type])),
                ),
                RuntimePlanTypeSeed::new(
                    option_type,
                    RuntimePlanTypeProjection::Option {
                        item: bool_type,
                        some_payload: payload_type,
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_source(
                    "arcweft-runtime-plan.fixture.awbc_lower.tests.build_while_let_plan.binding_a",
                    false,
                ),
                bool_type,
            )],
        )
        .expect("while-let test types and binding admit");
    let binding = admission.local_ids()[0].clone();
    let pattern = option_bool_pattern(
        option_type,
        payload_type,
        bool_type,
        RuntimePatternSeedKind::Bind {
            mutable: false,
            local: binding.clone(),
        },
    );
    let guard = include_guard.then(|| {
        RuntimeExprSeed::new(
            bool_type,
            RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                binding.clone(),
                RuntimeLocalReadMode::Copy,
            )),
        )
        .with_guard_copy_locals([binding])
    });
    builder
        .push_flow_executable(flow_executable(&flow))
        .expect("while-let test flow executable admits");
    builder
        .push_flow_schema(flow_schema(&flow))
        .expect("while-let test flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                type_id(1),
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![
                    RuntimeFlowOpSeed::WhileLet {
                        pattern,
                        expr: RuntimeExprSeed::new(option_type, RuntimeExprSeedKind::Value(value)),
                        guard,
                        body,
                    },
                    RuntimeFlowOpSeed::ReturnExpr(string_expr("after")),
                ])
                .into_boxed_slice(),
            },
        ))
        .expect("while-let test flow admits");
    builder
        .push_entry(flow_entry("while_let", flow))
        .expect("while-let test entry admits");
    builder.finish().expect("while-let test plan seals")
}

fn two_bool_vec_expr(sequence_type: RuntimeSemanticTypeId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        sequence_type,
        RuntimeExprSeedKind::Value(arcweft_core::value::runtime_sequence_values(vec![
            RuntimeValue::Bool(true),
            RuntimeValue::Bool(false),
        ])),
    )
}

fn sequence_pop_front_expr(
    option_type: RuntimeSemanticTypeId,
    receiver: &RuntimeLocalSeedId,
) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        option_type,
        RuntimeExprSeedKind::SequencePopFront {
            place: arcweft_core::plan::RuntimeMutablePlaceSeed::Local(receiver.clone()),
        },
    )
}

fn build_while_let_pop_front_plan() -> RuntimePlan {
    let flow = flow_id("while_let.pop_front");
    let bool_type = type_id(3);
    let sequence_type = type_id(4);
    let payload_type = type_id(5);
    let option_type = type_id(6);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(type_id(2), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(bool_type, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    sequence_type,
                    RuntimePlanTypeProjection::Sequence {
                        kind: arcweft_core::plan::RuntimePlanSequenceKind::Vec,
                        item: bool_type,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    payload_type,
                    RuntimePlanTypeProjection::Tuple(Box::new([bool_type])),
                ),
                RuntimePlanTypeSeed::new(
                    option_type,
                    RuntimePlanTypeProjection::Option {
                        item: bool_type,
                        some_payload: payload_type,
                    },
                ),
            ],
            [
                RuntimeLocalDeclarationSeed::new(
                    manual_local_source("arcweft-runtime-plan.fixture.awbc_lower.tests.build_while_let_pop_front_plan.binding_a", true),
                    sequence_type,
                ),
                RuntimeLocalDeclarationSeed::new(
                    manual_local_source("arcweft-runtime-plan.fixture.awbc_lower.tests.build_while_let_pop_front_plan.binding_b", false),
                    bool_type,
                ),
            ],
        )
        .expect("while-let pop_front types and locals admit");
    let sequence = admission.local_ids()[0].clone();
    let item = admission.local_ids()[1].clone();
    builder
        .push_flow_executable(flow_executable(&flow))
        .expect("while-let pop_front flow executable admits");
    builder
        .push_flow_schema(flow_schema(&flow))
        .expect("while-let pop_front flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                type_id(1),
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![
                    RuntimeFlowOpSeed::Let {
                        pattern: RuntimePatternSeed::new(
                            sequence_type,
                            RuntimePatternSeedKind::Bind {
                                mutable: true,
                                local: sequence.clone(),
                            },
                        ),
                        expr: two_bool_vec_expr(sequence_type),
                    },
                    RuntimeFlowOpSeed::WhileLet {
                        pattern: option_bool_pattern(
                            option_type,
                            payload_type,
                            bool_type,
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: item,
                            },
                        ),
                        expr: sequence_pop_front_expr(option_type, &sequence),
                        guard: None,
                        body: vec![RuntimeFlowOpSeed::Noop],
                    },
                    RuntimeFlowOpSeed::IfLet {
                        pattern: option_bool_pattern(
                            option_type,
                            payload_type,
                            bool_type,
                            RuntimePatternSeedKind::Discard,
                        ),
                        expr: sequence_pop_front_expr(option_type, &sequence),
                        guard: None,
                        then_ops: vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("leftover"))],
                        else_ops: Vec::new(),
                    },
                    RuntimeFlowOpSeed::ReturnExpr(string_expr("after")),
                ])
                .into_boxed_slice(),
            },
        ))
        .expect("while-let pop_front flow admits");
    builder
        .push_entry(flow_entry("while_let.pop_front", flow))
        .expect("while-let pop_front entry admits");
    builder.finish().expect("while-let pop_front plan seals")
}

fn build_vec_push_pop_plan() -> RuntimePlan {
    let flow = flow_id("vec.push_pop");
    let bool_type = type_id(3);
    let sequence_type = type_id(4);
    let payload_type = type_id(5);
    let option_type = type_id(6);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(type_id(2), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(bool_type, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(
                    sequence_type,
                    RuntimePlanTypeProjection::Sequence {
                        kind: arcweft_core::plan::RuntimePlanSequenceKind::Vec,
                        item: bool_type,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    payload_type,
                    RuntimePlanTypeProjection::Tuple(Box::new([bool_type])),
                ),
                RuntimePlanTypeSeed::new(
                    option_type,
                    RuntimePlanTypeProjection::Option {
                        item: bool_type,
                        some_payload: payload_type,
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_source("arcweft-runtime-plan.fixture.awbc_lower.tests.build_vec_push_pop_plan.binding_a", true),
                sequence_type,
            )],
        )
        .expect("Vec push/pop test types and local admit");
    let sequence = admission.local_ids()[0].clone();
    builder
        .push_flow_executable(flow_executable(&flow))
        .expect("Vec push/pop flow executable admits");
    builder
        .push_flow_schema(flow_schema(&flow))
        .expect("Vec push/pop flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                option_type,
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![
                    RuntimeFlowOpSeed::Let {
                        pattern: RuntimePatternSeed::new(
                            sequence_type,
                            RuntimePatternSeedKind::Bind {
                                mutable: true,
                                local: sequence.clone(),
                            },
                        ),
                        expr: two_bool_vec_expr(sequence_type),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: RuntimePatternSeed::new(
                            type_id(2),
                            RuntimePatternSeedKind::Discard,
                        ),
                        expr: RuntimeExprSeed::new(
                            type_id(2),
                            RuntimeExprSeedKind::SequencePush {
                                place: arcweft_core::plan::RuntimeMutablePlaceSeed::Local(
                                    sequence.clone(),
                                ),
                                value: Box::new(bool_expr(true)),
                            },
                        ),
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        option_type,
                        RuntimeExprSeedKind::SequencePopBack {
                            place: arcweft_core::plan::RuntimeMutablePlaceSeed::Local(sequence),
                        },
                    )),
                ])
                .into_boxed_slice(),
            },
        ))
        .expect("Vec push/pop flow admits");
    builder
        .push_entry(flow_entry("vec.push_pop", flow))
        .expect("Vec push/pop entry admits");
    builder.finish().expect("Vec push/pop plan seals")
}

fn flow_entry(id: &str, flow: FlowRuntimeId) -> RuntimeEntrySpec {
    RuntimeEntrySpec {
        id: entry_id(id),
        kind: RuntimeEntryKind::Cli,
        binding: EntryBindingIdentity::from_bytes([1; 32]),
        target: RuntimeEntryTarget::Flow(flow),
        roles: RuntimeEntryRoles::None,
    }
}

fn lower_plan(plan: &RuntimePlan) -> AwbcLowerReport {
    AwbcLowerer::new(
        plan,
        &arcweft_text_model::DialogueContentCatalog::new(),
        "test.arcw",
    )
    .lower()
    .expect("AWBC lowers builder-sealed runtime plan")
}

fn run_entry(program: &AwbcProgram) -> VmExit {
    let mut fiber =
        FiberState::for_entry(program, AwbcEntryId(0), 0, 256).expect("AWBC fiber initializes");
    vm::step(
        program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 128,
        },
    )
    .expect("AWBC VM executes entry")
    .exit
}

#[test]
fn awbc_cancellation_result_selection_uses_typed_terminal_and_fallthrough_keeps_pending() {
    let flow = flow_id("cancel.result");
    let selected_action =
        arcweft_interaction_model::input::InputActionId::new("dialogue.cancel.out")
            .expect("valid cancellation action");
    let pending_action =
        arcweft_interaction_model::input::InputActionId::new("dialogue.cancel.keep")
            .expect("valid cancellation action");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::String,
            )],
            [],
        )
        .expect("selected dialogue result type admits");
    let content = builder
        .push_dialogue_content_seed(RuntimeDialogueContentPlanSeed {
            line: arcweft_core::plan::RuntimeLineId::from_runtime_line_value("line.cancel.result")
                .expect("valid line identity"),
            template: RuntimeDialogueContentTemplateManifestSeed {
                id: arcweft_core::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                    .expect("first content template identity"),
                digest: RuntimeDialogueContentTemplateDigest::ZERO,
                slots: Box::default(),
                effects: Box::default(),
            },
            values: Box::default(),
            effect_sites: Box::default(),
            marks: Box::default(),
            effect_site_count: Default::default(),
        })
        .expect("dialogue content seed admits");
    let group = builder
        .push_line_task_group_seed(RuntimeLineTaskGroupSeed {
            definition:
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [71; 32],
                ),
            activation_ops: vec![RuntimeFlowOpSeed::CommitDialogueResult {
                value: string_expr("normal"),
            }],
            result_type: type_id(1),
            handle_sites: Box::default(),
            root: RuntimeLineTaskNodeSeed::Action(Vec::new()),
            cancel_rules: vec![
                RuntimeLineTaskCancelRuleSeed {
                    trigger: selected_action.clone(),
                    action: vec![RuntimeFlowOpSeed::SelectDialogueResult {
                        value: string_expr("selected"),
                    }],
                },
                RuntimeLineTaskCancelRuleSeed {
                    trigger: pending_action.clone(),
                    action: vec![RuntimeFlowOpSeed::Noop],
                },
            ]
            .into_boxed_slice(),
            cleanup_completed: Vec::new(),
            cleanup_cancelled: Vec::new(),
            cleanup_failed: Vec::new(),
            cleanup_policy: arcweft_core::line_task::LineCleanupPolicy::default(),
        })
        .expect("line-task cancellation result selection admits");
    builder
        .attach_line_task_group_seed(&content, &group)
        .expect("line-task group attaches to its content");
    builder
        .push_flow_schema(flow_schema(&flow))
        .expect("flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                type_id(1),
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![RuntimeFlowOpSeed::Return("done".to_owned())]).into_boxed_slice(),
            },
        ))
        .expect("entry flow admits");
    builder
        .push_flow_executable(flow_executable(&flow))
        .expect("flow executable admits");
    builder
        .push_entry(flow_entry("cancel.result", flow))
        .expect("entry admits");

    let plan = builder.finish().expect("runtime plan seals");
    let mut inventory = AwbcInventory::new(AwbcLowerOptions::default());
    inventory.intern_runtime_primitives();
    super::pattern::preflight_plan_types(&mut inventory, &plan)
        .expect("line-task result type admits in the AWBC type table");
    let diagnostics = {
        let mut lowerer = AwbcFlowLowerer::new(&mut inventory, &plan);
        lowerer.lower_plan();
        lowerer.into_diagnostics()
    };
    assert!(
        diagnostics.is_empty(),
        "unexpected lowering diagnostics: {diagnostics:?}"
    );
    let program = inventory.finish();
    let group = &program.line_task_groups[0];
    let function = |trigger: &arcweft_interaction_model::input::InputActionId| {
        let handler = group
            .cancel_handlers
            .iter()
            .find(|handler| &handler.trigger == trigger)
            .expect("line cancellation handler is present");
        &program.functions[handler.function.index()]
    };
    let function_terminators = |function: &arcweft_core::awbc::schema::AwbcFunction| {
        let start = function.blocks.start as usize;
        let end = function
            .blocks
            .checked_end()
            .expect("checked function block range") as usize;
        program.blocks[start..end]
            .iter()
            .map(|block| block.terminator.clone())
            .collect::<Vec<_>>()
    };

    let definition = plan.line_task_groups()[0].definition();
    for (source_rule, handler) in group.cancel_handlers.iter().enumerate() {
        assert_eq!(
            program.functions[handler.function.index()].definition,
            definition.generated_child(
                arcweft_core::plan::RuntimeGeneratedFunctionRole::LineCancellation {
                    source_rule: u32::try_from(source_rule).unwrap(),
                }
            )
        );
    }
    let bytes = program.encode_canonical().unwrap();
    let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &bytes,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .unwrap();
    assert_eq!(decoded.functions, program.functions);

    let selected = function(&selected_action);
    assert_eq!(
        selected.kind,
        arcweft_core::awbc::schema::AwbcFunctionKind::LineCancellationHandler
    );
    assert_eq!(program.signatures[selected.signature.index()].result, None);
    let selected_value = function_terminators(selected)
        .into_iter()
        .find_map(|terminator| match terminator {
            AwbcTerminator::SelectDialogueResult { value } => Some(value),
            _ => None,
        })
        .expect("selected cancellation result has a distinct terminator");
    let layout = &program.frame_layouts[selected.frame_layout.index()];
    assert_eq!(layout.slots[selected_value.index()].ty, group.result_type);
    assert!(
        function_terminators(selected)
            .iter()
            .all(|terminator| !matches!(terminator, AwbcTerminator::Return { .. }))
    );

    let keep_pending = function(&pending_action);
    assert_eq!(
        keep_pending.kind,
        arcweft_core::awbc::schema::AwbcFunctionKind::LineCancellationHandler
    );
    assert_eq!(
        program.signatures[keep_pending.signature.index()].result,
        None
    );
    let keep_terminators = function_terminators(keep_pending);
    assert!(
        keep_terminators
            .iter()
            .any(|terminator| matches!(terminator, AwbcTerminator::Return { value: None }))
    );
    assert!(
        keep_terminators
            .iter()
            .all(|terminator| !matches!(terminator, AwbcTerminator::SelectDialogueResult { .. }))
    );
}

#[test]
fn line_activation_local_is_exported_only_to_post_reveal_work() {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::String,
            )],
            [RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-runtime-plan.fixture.awbc_lower.tests.line_activation_local_is_exported_only_to_post_reveal_work.binding_a", false), type_id(1))],
        )
        .expect("line result and activation local admit");
    let local = admission.local_ids()[0].clone();
    let content = builder
        .push_dialogue_content_seed(RuntimeDialogueContentPlanSeed {
            line: arcweft_core::plan::RuntimeLineId::from_runtime_line_value("line.export")
                .expect("line identity"),
            template: RuntimeDialogueContentTemplateManifestSeed {
                id: arcweft_core::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(0)
                    .expect("template identity"),
                digest: RuntimeDialogueContentTemplateDigest::ZERO,
                slots: Box::default(),
                effects: Box::default(),
            },
            values: Box::default(),
            effect_sites: Box::default(),
            marks: Box::default(),
            effect_site_count: Default::default(),
        })
        .expect("content admits");
    let group = builder
        .push_line_task_group_seed(RuntimeLineTaskGroupSeed {
            definition:
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [71; 32],
                ),
            activation_ops: vec![
                RuntimeFlowOpSeed::Let {
                    pattern: RuntimePatternSeed::new(
                        type_id(1),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: local.clone(),
                        },
                    ),
                    expr: string_expr("retained"),
                },
                RuntimeFlowOpSeed::CommitDialogueResult {
                    value: string_expr("normal"),
                },
            ],
            result_type: type_id(1),
            handle_sites: Box::default(),
            root: RuntimeLineTaskNodeSeed::Action(Vec::new()),
            cancel_rules: vec![RuntimeLineTaskCancelRuleSeed {
                trigger: arcweft_interaction_model::input::InputActionId::new("dialogue.cancel")
                    .expect("cancel action"),
                action: vec![RuntimeFlowOpSeed::SelectDialogueResult {
                    value: RuntimeExprSeed::new(
                        type_id(1),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            local,
                            RuntimeLocalReadMode::Copy,
                        )),
                    ),
                }],
            }]
            .into_boxed_slice(),
            cleanup_completed: Vec::new(),
            cleanup_cancelled: Vec::new(),
            cleanup_failed: Vec::new(),
            cleanup_policy: arcweft_core::line_task::LineCleanupPolicy::default(),
        })
        .expect("activation export admits");
    builder
        .attach_line_task_group_seed(&content, &group)
        .expect("content owns line task");
    let plan = builder.finish().expect("line task plan seals");
    let mut inventory = AwbcInventory::new(AwbcLowerOptions::default());
    inventory.intern_runtime_primitives();
    super::pattern::preflight_plan_types(&mut inventory, &plan).expect("line type admits to AWBC");
    let diagnostics = {
        let mut lowerer = AwbcFlowLowerer::new(&mut inventory, &plan);
        lowerer.lower_plan();
        lowerer.into_diagnostics()
    };
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let program = inventory.finish();
    let group = &program.line_task_groups[0];
    assert!(
        group.captures.is_empty(),
        "activation local is not a caller input"
    );
    let [export] = group.activation_exports.as_slice() else {
        panic!("one retained activation local must reach line-task work")
    };
    let activation = &program.functions[group.activation.index()];
    assert!(
        program.signatures[activation.signature.index()]
            .params
            .is_empty()
    );
    let slot =
        &program.frame_layouts[activation.frame_layout.index()].slots[export.register.index()];
    assert_eq!(slot.ty, export.ty);
    assert_eq!(slot.scope_depth, 0);
    let handler = &program.functions[group.cancel_handlers[0].function.index()];
    assert_eq!(
        program.signatures[handler.signature.index()].params,
        [export.ty]
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one regression fixture covers every builtin payload edge and pattern"
)]
fn option_and_result_awbc_patterns_use_exact_tuple_payload_edges() {
    let flow = flow_id("builtin.payload_edges");
    let item = type_id(10);
    let error = type_id(11);
    let item_payload = type_id(12);
    let error_payload = type_id(13);
    let option = type_id(14);
    let result = type_id(15);
    let unit = type_id(16);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(
                    item,
                    RuntimePlanTypeProjection::Signed(
                        arcweft_core::value::RuntimeSignedIntWidth::I64,
                    ),
                ),
                RuntimePlanTypeSeed::new(error, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(
                    item_payload,
                    RuntimePlanTypeProjection::Tuple(Box::new([item])),
                ),
                RuntimePlanTypeSeed::new(
                    error_payload,
                    RuntimePlanTypeProjection::Tuple(Box::new([error])),
                ),
                RuntimePlanTypeSeed::new(
                    option,
                    RuntimePlanTypeProjection::Option {
                        item,
                        some_payload: item_payload,
                    },
                ),
                RuntimePlanTypeSeed::new(
                    result,
                    RuntimePlanTypeProjection::Result {
                        value: item,
                        error,
                        value_payload: item_payload,
                        error_payload,
                    },
                ),
                RuntimePlanTypeSeed::new(unit, RuntimePlanTypeProjection::Unit),
            ],
            [],
        )
        .expect("builtin payload type graph");
    let option_pattern = RuntimePatternSeed::new(
        option,
        RuntimePatternSeedKind::Variant {
            ordinal: 0,
            payload: Some(Box::new(RuntimePatternSeed::new(
                item_payload,
                RuntimePatternSeedKind::Tuple(Box::new([RuntimePatternSeed::new(
                    item,
                    RuntimePatternSeedKind::Discard,
                )])),
            ))),
        },
    );
    let result_ok_pattern = RuntimePatternSeed::new(
        result,
        RuntimePatternSeedKind::Variant {
            ordinal: 0,
            payload: Some(Box::new(RuntimePatternSeed::new(
                item_payload,
                RuntimePatternSeedKind::Tuple(Box::new([RuntimePatternSeed::new(
                    item,
                    RuntimePatternSeedKind::Discard,
                )])),
            ))),
        },
    );
    let result_err_pattern = RuntimePatternSeed::new(
        result,
        RuntimePatternSeedKind::Variant {
            ordinal: 1,
            payload: Some(Box::new(RuntimePatternSeed::new(
                error_payload,
                RuntimePatternSeedKind::Tuple(Box::new([RuntimePatternSeed::new(
                    error,
                    RuntimePatternSeedKind::Discard,
                )])),
            ))),
        },
    );
    builder
        .push_flow_executable(flow_executable(&flow))
        .expect("payload flow executable admits");
    builder
        .push_flow_schema(flow_schema(&flow))
        .expect("payload flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                unit,
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![
                    RuntimeFlowOpSeed::Let {
                        pattern: option_pattern,
                        expr: RuntimeExprSeed::new(
                            option,
                            RuntimeExprSeedKind::Value(RuntimeValue::option_some(
                                RuntimeValue::i64(7),
                            )),
                        ),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: result_ok_pattern,
                        expr: RuntimeExprSeed::new(
                            result,
                            RuntimeExprSeedKind::Value(RuntimeValue::result_ok(RuntimeValue::i64(
                                8,
                            ))),
                        ),
                    },
                    RuntimeFlowOpSeed::Let {
                        pattern: result_err_pattern,
                        expr: RuntimeExprSeed::new(
                            result,
                            RuntimeExprSeedKind::Value(RuntimeValue::result_err(
                                RuntimeValue::String("no".to_owned()),
                            )),
                        ),
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        unit,
                        RuntimeExprSeedKind::Value(RuntimeValue::Unit),
                    )),
                ])
                .into_boxed_slice(),
            },
        ))
        .expect("payload flow admits");
    builder
        .push_entry(flow_entry("builtin.payload_edges", flow))
        .expect("payload entry admits");

    let report = lower_plan(&builder.finish().expect("payload plan seals"));
    let program = &report.program;
    let runtime_type = |identity: RuntimeSemanticTypeId| {
        program
            .runtime_types
            .iter()
            .enumerate()
            .find(|(_, row)| row.semantic_identity() == identity)
            .map(|(index, _)| {
                arcweft_core::awbc::schema::AwbcTypeId(
                    u32::try_from(index).expect("test AWBC type index fits u32"),
                )
            })
            .expect("semantic type is retained in AWBC runtime type table")
    };
    let option_type = runtime_type(option);
    let result_type = runtime_type(result);
    let item_type = runtime_type(item);
    let error_type = runtime_type(error);
    let item_payload_type = runtime_type(item_payload);
    let error_payload_type = runtime_type(error_payload);

    let assert_payload_edge =
        |owner: arcweft_core::awbc::schema::AwbcTypeId,
         ordinal: u32,
         expected_payload: arcweft_core::awbc::schema::AwbcTypeId| {
            let AwbcRuntimeTypeShape::Variant { cases, .. } =
                program.runtime_types[owner.index()].shape()
            else {
                panic!("builtin type lowers to an AWBC variant shape");
            };
            let payload = cases
                .get(ordinal as usize)
                .and_then(|case| case.payload)
                .expect("payload-bearing builtin case has a payload edge");
            assert_eq!(payload, expected_payload);
            let expected_item = match (owner, ordinal) {
                (owner, 0) if owner == option_type || owner == result_type => item_type,
                (owner, 1) if owner == result_type => error_type,
                _ => panic!("unexpected builtin payload edge"),
            };
            assert_eq!(
                program.runtime_types[payload.index()].shape(),
                &AwbcRuntimeTypeShape::Tuple(vec![expected_item])
            );
        };
    assert_payload_edge(option_type, 0, item_payload_type);
    assert_payload_edge(result_type, 0, item_payload_type);
    assert_payload_edge(result_type, 1, error_payload_type);

    let variant_patterns = program
        .patterns
        .iter()
        .filter_map(|pattern| match pattern {
            AwbcPattern::Variant {
                ty,
                case,
                payload: Some(payload),
                ..
            } => Some((*ty, *case, *payload)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(variant_patterns.len(), 3);
    for (owner, ordinal, expected_payload) in [
        (option_type, 0, item_type),
        (result_type, 0, item_type),
        (result_type, 1, error_type),
    ] {
        let (_, _, payload) = variant_patterns
            .iter()
            .find(|(actual_owner, actual_ordinal, _)| {
                *actual_owner == owner && *actual_ordinal == ordinal
            })
            .copied()
            .expect("lowered variant pattern retains every builtin case");
        let AwbcPattern::Tuple(items) = &program.patterns[payload.index()] else {
            panic!("builtin variant pattern payload is a tuple pattern");
        };
        assert_eq!(items.len(), 1);
        let child = program
            .patterns
            .get(items[0].index())
            .expect("tuple payload pattern child");
        assert!(matches!(child, AwbcPattern::Discard));
        let expected_payload_type = match (owner, ordinal) {
            (owner, 0) if owner == option_type || owner == result_type => item_payload_type,
            (owner, 1) if owner == result_type => error_payload_type,
            _ => panic!("unexpected builtin pattern case"),
        };
        let AwbcRuntimeTypeShape::Tuple(types) =
            program.runtime_types[expected_payload_type.index()].shape()
        else {
            panic!("builtin payload type is a tuple");
        };
        assert_eq!(types, &vec![expected_payload]);
    }
}

fn foreign_local_seed() -> RuntimeLocalSeedId {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::Bool,
            )],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_source(
                    "arcweft-runtime-plan.fixture.awbc_lower.tests.foreign_local_seed.binding_a",
                    false,
                ),
                type_id(1),
            )],
        )
        .expect("foreign local admission");
    admission.local_ids()[0].clone()
}

fn builder_with_local() -> (RuntimePlanBuilder, RuntimeLocalSeedId) {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::Bool,
            )],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_source(
                    "arcweft-runtime-plan.fixture.awbc_lower.tests.builder_with_local.binding_a",
                    false,
                ),
                type_id(1),
            )],
        )
        .expect("local admission");
    (builder, admission.local_ids()[0].clone())
}

fn invalid_let_expression(binding: RuntimeLocalSeedId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        type_id(1),
        RuntimeExprSeedKind::Let {
            binding,
            expr: Box::new(bool_expr(true)),
            body: Box::new(bool_expr(true)),
        },
    )
}

fn plan_with_local() -> (
    RuntimePlan,
    arcweft_core::runtime_id::RuntimeLocalDeclarationId,
) {
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::String,
            )],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_source(
                    "arcweft-runtime-plan.fixture.awbc_lower.tests.plan_with_local.binding_a",
                    false,
                ),
                type_id(1),
            )],
        )
        .expect("local plan admission");
    builder
        .push_pure_helper_seed(RuntimePureHelperSeed {
            definition:
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [51; 32],
                ),
            name: "local".to_owned(),
            inputs: admission
                .local_ids()
                .to_vec()
                .into_boxed_slice()
                .into_iter()
                .zip(vec![RuntimePureInputType::Value])
                .map(
                    |(local, abi)| arcweft_core::plan::RuntimeCallableParameterSeed {
                        identity: arcweft_core::plan::RuntimeFunctionParameterIdentity::from_accepted_identity([91; 32]),
                        local,
                        passing: arcweft_core::plan::RuntimeFunctionParameterPassing::Value,
                        abi,
                    },
                )
                .collect(),
            output_abi: RuntimePureOutputType::Value,
            body: string_expr("ok"),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        })
        .expect("local helper admission");
    let plan = builder.finish().expect("local plan seals");
    let local = plan.pure_helpers()[0].inputs[0].local();
    (plan, local)
}

#[test]
fn missing_local_type_is_reported_instead_of_becoming_a_success_type() {
    let (_, missing) = plan_with_local();
    let plan = build_plan(type_id(2), [], []);
    let mut inventory = AwbcInventory::new(AwbcLowerOptions::default());
    let dynamic = inventory.dynamic_ty();

    assert_eq!(
        crate::awbc_lower::pattern::admitted_local_type(&mut inventory, &plan, missing),
        dynamic
    );
    assert!(inventory.take_diagnostics().iter().any(|diagnostic| {
        diagnostic.path == format!("local.{missing}") && diagnostic.is_error()
    }));
}

#[test]
fn invalid_local_seeds_cannot_produce_an_awbc_plan() {
    let foreign = foreign_local_seed();

    let mut flow_builder = RuntimePlanBuilder::new();
    assert_eq!(
        flow_builder.push_flow_seed(RuntimeFlowSeed::new(flow_id("invalid_plan"), arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]), None, Box::new([arcweft_core::plan::RuntimeFunctionInputBindingSeed { transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Formal, origin: arcweft_core::plan::RuntimeFunctionInputOrigin::Parameter(arcweft_core::plan::RuntimeFunctionParameterIdentity::from_accepted_identity([92;32])), source: arcweft_core::plan::RuntimeFunctionInputSource::Parameter { position:0, passing:arcweft_core::plan::RuntimeFunctionParameterPassing::Value }, input_local: foreign.clone(), pattern: arcweft_core::plan::RuntimePatternSeed::new(type_id(1), arcweft_core::plan::RuntimePatternSeedKind::Bind { mutable:false, local: foreign.clone() }), ownership: arcweft_core::plan::RuntimeFunctionInputOwnershipRequirement::Owned, unrestricted_bindings: Box::new([]) }]), type_id(1), arcweft_core::plan::RuntimeEffectSet::empty()), arcweft_core::plan::RuntimeExecutableBodySeed { effects: arcweft_core::plan::RuntimeEffectSet::empty(), ops: (vec![RuntimeFlowOpSeed::Noop]).into_boxed_slice() })),
        Err(RuntimePlanBuildError::ForeignLocalSeed)
    );
    assert_eq!(flow_builder.finish(), Err(RuntimePlanBuildError::Poisoned));

    let mut pure_builder = RuntimePlanBuilder::new();
    pure_builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::Bool,
            )],
            [],
        )
        .expect("pure helper type admission");
    assert_eq!(
        pure_builder.push_pure_helper_seed(RuntimePureHelperSeed {
            definition:
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [51; 32]
                ),
            name: "invalid.local".to_owned(),
            inputs: Box::new([]),
            output_abi: RuntimePureOutputType::Value,
            body: invalid_let_expression(foreign.clone()),
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Annotated,
        }),
        Err(RuntimePlanBuildError::ForeignLocalSeed)
    );
    assert_eq!(pure_builder.finish(), Err(RuntimePlanBuildError::Poisoned));

    let (mut trait_builder, receiver) = builder_with_local();
    assert_eq!(
        trait_builder.push_trait_method_seed(RuntimeTraitMethodSeed {
            definition:
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [51; 32]
                ),
            identity: RuntimeTraitMethodIdentity {
                impl_id: 0,
                trait_id: None,
                witness: None,
                trait_name: None,
                self_type: "bool".to_owned(),
                method_name: "invalid_local".to_owned(),
                monomorph_label: "invalid_local".to_owned(),
            },
            receiver: RuntimeReceiverMode::Owned,
            inputs: Box::new([arcweft_core::plan::RuntimeCallableParameterSeed {
                identity:
                    arcweft_core::plan::RuntimeFunctionParameterIdentity::from_accepted_identity(
                        [91; 32]
                    ),
                local: receiver,
                passing: arcweft_core::plan::RuntimeFunctionParameterPassing::Value,
                abi: RuntimePureInputType::Value
            }]),
            output_abi: RuntimePureOutputType::Value,
            body: invalid_let_expression(foreign),
        }),
        Err(RuntimePlanBuildError::ForeignLocalSeed)
    );
    assert_eq!(trait_builder.finish(), Err(RuntimePlanBuildError::Poisoned));
}

#[test]
fn builder_sealed_constant_return_lowers_and_executes() {
    let main = flow_id("main");
    let plan = build_plan(
        type_id(1),
        [(
            main.clone(),
            vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("ok"))],
        )],
        [flow_entry("main", main)],
    );
    let report = lower_plan(&plan);
    assert!(!report.program.functions.is_empty());
    assert_eq!(
        run_entry(&report.program),
        VmExit::Returned(Some(RuntimeValue::String("ok".to_owned())))
    );
}

#[test]
fn selected_entry_lowering_keeps_its_static_flow_closure() {
    let selected = entry_id("selected");
    let selected_flow = flow_id("selected");
    let shared = flow_id("shared");
    let unselected = flow_id("unselected");
    let plan = build_plan(
        type_id(1),
        [
            (
                selected_flow.clone(),
                vec![RuntimeFlowOpSeed::Goto(shared.clone())],
            ),
            (
                shared.clone(),
                vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("done"))],
            ),
            (
                unselected.clone(),
                vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("other"))],
            ),
        ],
        [
            RuntimeEntrySpec {
                id: selected.clone(),
                kind: RuntimeEntryKind::Cli,
                binding: EntryBindingIdentity::from_bytes([1; 32]),
                target: RuntimeEntryTarget::Flow(selected_flow.clone()),
                roles: RuntimeEntryRoles::None,
            },
            flow_entry("unselected", unselected.clone()),
        ],
    );
    let report = AwbcLowerer::for_entry(
        &plan,
        &arcweft_text_model::DialogueContentCatalog::new(),
        "test.arcw",
        &selected,
    )
    .lower()
    .expect("selected entry lowers");
    assert_eq!(report.program.entries.len(), 1);
    assert!(report.program.flow_function(&selected_flow).is_some());
    assert!(report.program.flow_function(&shared).is_some());
    assert!(report.program.flow_function(&unselected).is_none());
}

#[test]
fn dynamic_goto_keeps_all_accepted_flow_targets() {
    let selected = entry_id("selected");
    let selected_flow = flow_id("selected");
    let other = flow_id("other");
    let plan = build_plan(
        type_id(1),
        [
            (
                selected_flow.clone(),
                vec![RuntimeFlowOpSeed::GotoExpr(string_expr("other"))],
            ),
            (
                other.clone(),
                vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("done"))],
            ),
        ],
        [RuntimeEntrySpec {
            id: selected.clone(),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([1; 32]),
            target: RuntimeEntryTarget::Flow(selected_flow),
            roles: RuntimeEntryRoles::None,
        }],
    );
    let report = AwbcLowerer::for_entry(
        &plan,
        &arcweft_text_model::DialogueContentCatalog::new(),
        "test.arcw",
        &selected,
    )
    .lower()
    .expect("dynamic selected entry lowers");
    assert_eq!(report.program.flow_bindings.len(), 2);
    assert!(report.program.flow_function(&other).is_some());
}

#[test]
fn builder_sealed_plan_roundtrips_through_canonical_awbc() {
    let main = flow_id("main");
    let report = lower_plan(&build_plan(
        type_id(2),
        [(
            main.clone(),
            vec![RuntimeFlowOpSeed::ReturnExpr(unit_expr())],
        )],
        [flow_entry("main", main)],
    ));
    let encoded = report
        .program
        .encode_canonical()
        .expect("AWBC encoder accepts lowered plan");
    let decoded = AwbcProgram::decode_canonical(
        &encoded,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .expect("canonical AWBC decodes");
    assert_eq!(
        run_entry(&decoded),
        VmExit::Returned(Some(RuntimeValue::Unit))
    );
}

#[test]
fn discarded_host_call_result_still_has_an_awbc_destination() {
    let main = flow_id("main");
    let report = lower_plan(&build_plan(
        type_id(2),
        [(
            main.clone(),
            vec![
                RuntimeFlowOpSeed::HostCall {
                    binding: None,
                    target: RuntimeHostCallTargetSeed {
                        producer: arcweft_core::task::HostCallProducerDefinition {
                            contract: arcweft_core::task::NeedProducerContractDigest::from_bytes(
                                [1; 32],
                            ),
                            plan: arcweft_core::task::TaskPlanSemanticDigest::from_bytes([2; 32]),
                            site: arcweft_core::task::NeedProducerSiteDigest::from_bytes([3; 32]),
                        },
                        public_id: "test.notify".to_owned(),
                        capability: "test".to_owned(),
                        operation: "notify".to_owned(),
                        contract: None,
                        args: Vec::new(),
                        result: type_id(2),
                        mode: RuntimeHostCallMode::Suspend,
                        deterministic: false,
                    },
                },
                RuntimeFlowOpSeed::ReturnExpr(unit_expr()),
            ],
        )],
        [flow_entry("main", main)],
    ));
    let destination = report.program.blocks.iter().find_map(|block| {
        let AwbcTerminator::HostCall { dst, .. } = block.terminator else {
            return None;
        };
        dst
    });
    assert!(destination.is_some());
}

#[test]
fn host_signature_preserves_every_admitted_operand_and_result_identity() {
    for mode in [RuntimeHostCallMode::Immediate, RuntimeHostCallMode::Suspend] {
        let main = flow_id("main");
        let plan = build_plan(type_id(2),
            [(
                main.clone(),
                vec![
                    RuntimeFlowOpSeed::HostCall {
                        binding: None,
                        target: RuntimeHostCallTargetSeed {
                            producer: arcweft_core::task::HostCallProducerDefinition {
                                contract:
                                    arcweft_core::task::NeedProducerContractDigest::from_bytes(
                                        [1; 32],
                                    ),
                                plan: arcweft_core::task::TaskPlanSemanticDigest::from_bytes(
                                    [2; 32],
                                ),
                                site: arcweft_core::task::NeedProducerSiteDigest::from_bytes(
                                    [3; 32],
                                ),
                            },
                            public_id: "test.notify".to_owned(),
                            capability: "test".to_owned(),
                            operation: "notify".to_owned(),
                            contract: None,
                            args: vec![arcweft_core::plan::RuntimeHostArgumentSeed::Positional(arcweft_core::task::RuntimeRequestRoleIdentity::from_accepted_identity([42;32]),
                                string_expr("message"),
                            )],
                            result: type_id(2),
                            mode,
                            deterministic: false,
                        },
                    },
                    RuntimeFlowOpSeed::ReturnExpr(unit_expr()),
                ],
            )],
            [flow_entry("main", main)],
        );
        let report = lower_plan(&plan);
        let [host] = report.program.host_calls.as_slice() else {
            panic!("one exact host call");
        };
        let signature = &report.program.signatures[host.signature.index()];
        let [parameter] = signature.params.as_slice() else {
            panic!("one materialized host operand");
        };
        let result = signature.result.expect("host result type");
        assert_eq!(
            report.program.runtime_types[parameter.index()].semantic_identity(),
            type_id(1)
        );
        assert_eq!(
            report.program.runtime_types[result.index()].semantic_identity(),
            type_id(2)
        );
        let encoded = report.program.encode_canonical().expect("host ABI encodes");
        let decoded = AwbcProgram::decode_canonical(
            &encoded,
            arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
        )
        .expect("host ABI decodes");
        assert_eq!(decoded, report.program);
        decoded
            .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
            .expect("decoded host ABI verifies");
    }
}

#[test]
fn host_descriptor_interning_preserves_distinct_call_site_values() {
    let main = flow_id("main");
    let mut ops = ["first", "second"]
        .into_iter()
        .map(|message| RuntimeFlowOpSeed::HostCall {
            binding: None,
            target: RuntimeHostCallTargetSeed {
                producer: arcweft_core::task::HostCallProducerDefinition {
                    contract: arcweft_core::task::NeedProducerContractDigest::from_bytes([1; 32]),
                    plan: arcweft_core::task::TaskPlanSemanticDigest::from_bytes([2; 32]),
                    site: arcweft_core::task::NeedProducerSiteDigest::from_bytes([3; 32]),
                },
                public_id: "test.notify".to_owned(),
                capability: "test".to_owned(),
                operation: "notify".to_owned(),
                contract: None,
                args: vec![arcweft_core::plan::RuntimeHostArgumentSeed::Positional(
                    arcweft_core::task::RuntimeRequestRoleIdentity::from_accepted_identity(
                        [42; 32],
                    ),
                    string_expr(message),
                )],
                result: type_id(2),
                mode: RuntimeHostCallMode::Suspend,
                deterministic: false,
            },
        })
        .collect::<Vec<_>>();
    ops.push(RuntimeFlowOpSeed::ReturnExpr(unit_expr()));
    let report = lower_plan(&build_plan(
        type_id(2),
        [(main.clone(), ops)],
        [flow_entry("main", main)],
    ));
    assert_eq!(
        report.program.host_calls.len(),
        1,
        "equal host ABIs share one descriptor"
    );
    let mut fiber = FiberState::for_entry(&report.program, AwbcEntryId(0), 1, 1024).expect("entry");
    for message in ["first", "second"] {
        let output =
            vm::step(&report.program, &mut fiber, VmStepOptions::default()).expect("host step");
        let VmExit::Suspended(arcweft_core::awbc::fiber::FiberSuspensionReason::HostCall {
            call,
            args,
            destination,
        }) = output.exit
        else {
            panic!("one suspended host call")
        };
        assert_eq!(call.index(), 0);
        assert_eq!(args, [RuntimeValue::String(message.to_owned())]);
        let resume = fiber
            .suspension
            .as_ref()
            .and_then(arcweft_core::awbc::fiber::FiberSuspension::declared_resume)
            .expect("host resume");
        fiber
            .active_frame_mut()
            .expect("active host frame")
            .set_register(
                destination.expect("host result register"),
                RuntimeValue::Unit,
            )
            .expect("host response");
        fiber
            .resume_at(&report.program, resume)
            .expect("resume exact host site");
    }
    assert_eq!(
        vm::step(&report.program, &mut fiber, VmStepOptions::default())
            .expect("return step")
            .exit,
        VmExit::Returned(Some(RuntimeValue::Unit))
    );
}

#[test]
fn loop_break_paths_initialize_one_typed_result_before_binding() {
    let main = flow_id("main");
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(type_id(3), RuntimePlanTypeProjection::Bool),
            ],
            [RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-runtime-plan.fixture.awbc_lower.tests.loop_break_paths_initialize_one_typed_result_before_binding.binding_a", false), type_id(1))],
        )
        .expect("loop result facts admit");
    let result = admission.local_ids()[0].clone();
    builder
        .push_flow_executable(flow_executable(&main))
        .expect("loop flow executable admits");
    builder
        .push_flow_schema(flow_schema(&main))
        .expect("loop flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            main.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                type_id(1),
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![
                    RuntimeFlowOpSeed::Loop {
                        result: Some(RuntimePatternSeed::new(
                            type_id(1),
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: result.clone(),
                            },
                        )),
                        body: vec![RuntimeFlowOpSeed::If {
                            condition: bool_expr(true),
                            then_ops: vec![RuntimeFlowOpSeed::Break(Some(string_expr("then")))],
                            else_ops: vec![RuntimeFlowOpSeed::Break(Some(string_expr("else")))],
                        }],
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        type_id(1),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            result,
                            RuntimeLocalReadMode::Copy,
                        )),
                    )),
                ])
                .into_boxed_slice(),
            },
        ))
        .expect("loop flow admits");
    builder
        .push_entry(flow_entry("main", main))
        .expect("loop entry admits");

    let report = lower_plan(&builder.finish().expect("loop plan seals"));
    assert_eq!(
        run_entry(&report.program),
        VmExit::Returned(Some(RuntimeValue::String("then".to_owned())))
    );
    assert!(
        report
            .program
            .blocks
            .iter()
            .any(|block| block.safe_point == AwbcSafePointKind::LoopBackedge)
    );
    assert!(!report.program.intrinsics.iter().any(|intrinsic| {
        matches!(
            intrinsic.identity.as_label(),
            "flow.break" | "flow.continue"
        )
    }));
}

#[test]
fn nested_loops_bind_the_nearest_break_result() {
    let main = flow_id("main");
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                type_id(1),
                RuntimePlanTypeProjection::String,
            )],
            [
                RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-runtime-plan.fixture.awbc_lower.tests.nested_loops_bind_the_nearest_break_result.binding_a", false), type_id(1)),
                RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-runtime-plan.fixture.awbc_lower.tests.nested_loops_bind_the_nearest_break_result.binding_b", false), type_id(1)),
            ],
        )
        .expect("nested loop result facts admit");
    let inner_result = admission.local_ids()[0].clone();
    let outer_result = admission.local_ids()[1].clone();
    let binding = |local| {
        RuntimePatternSeed::new(
            type_id(1),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        )
    };
    builder
        .push_flow_executable(flow_executable(&main))
        .expect("nested loop flow executable admits");
    builder
        .push_flow_schema(flow_schema(&main))
        .expect("nested loop flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            main.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                type_id(1),
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![
                    RuntimeFlowOpSeed::Loop {
                        result: Some(binding(outer_result.clone())),
                        body: vec![
                            RuntimeFlowOpSeed::Loop {
                                result: Some(binding(inner_result.clone())),
                                body: vec![RuntimeFlowOpSeed::Break(Some(string_expr("nested")))],
                            },
                            RuntimeFlowOpSeed::Break(Some(RuntimeExprSeed::new(
                                type_id(1),
                                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                    inner_result,
                                    RuntimeLocalReadMode::Copy,
                                )),
                            ))),
                        ],
                    },
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        type_id(1),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            outer_result,
                            RuntimeLocalReadMode::Copy,
                        )),
                    )),
                ])
                .into_boxed_slice(),
            },
        ))
        .expect("nested loop flow admits");
    builder
        .push_entry(flow_entry("main", main))
        .expect("nested loop entry admits");

    let report = lower_plan(&builder.finish().expect("nested loop plan seals"));
    assert_eq!(
        run_entry(&report.program),
        VmExit::Returned(Some(RuntimeValue::String("nested".to_owned())))
    );
}

#[test]
fn loop_continue_targets_the_verified_backedge_header() {
    let main = flow_id("main");
    let plan = build_plan(
        type_id(2),
        [(
            main.clone(),
            vec![RuntimeFlowOpSeed::Loop {
                result: None,
                body: vec![RuntimeFlowOpSeed::Continue],
            }],
        )],
        [flow_entry("main", main)],
    );
    let report = lower_plan(&plan);

    assert!(
        !report
            .program
            .intrinsics
            .iter()
            .any(|intrinsic| { intrinsic.identity.as_label() == "flow.continue" })
    );
    assert!(
        report
            .program
            .blocks
            .iter()
            .enumerate()
            .any(|(index, block)| {
                matches!(
                    block.terminator,
                    AwbcTerminator::Jump { target }
                        if target.index() <= index
                            && report.program.blocks[target.index()].safe_point
                                == AwbcSafePointKind::LoopBackedge
                )
            })
    );
}

#[test]
fn while_false_condition_skips_body_and_canonical_roundtrip_preserves_execution() {
    let main = flow_id("while.zero");
    let plan = build_bool_flow_plan(
        type_id(1),
        main,
        vec![
            RuntimeFlowOpSeed::While {
                condition: bool_expr(false),
                body: vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("entered"))],
            },
            RuntimeFlowOpSeed::ReturnExpr(string_expr("after")),
        ],
    );
    let report = lower_plan(&plan);
    assert_eq!(
        run_entry(&report.program),
        VmExit::Returned(Some(RuntimeValue::String("after".to_owned())))
    );

    let encoded = report
        .program
        .encode_canonical()
        .expect("while AWBC encodes canonically");
    let decoded = AwbcProgram::decode_canonical(
        &encoded,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .expect("while AWBC decodes canonically");
    assert_eq!(
        run_entry(&decoded),
        VmExit::Returned(Some(RuntimeValue::String("after".to_owned())))
    );
}

#[test]
fn while_break_exits_after_one_positive_iteration() {
    let main = flow_id("while.break");
    let plan = build_bool_flow_plan(
        type_id(1),
        main,
        vec![
            RuntimeFlowOpSeed::While {
                condition: bool_expr(true),
                body: vec![RuntimeFlowOpSeed::Break(None)],
            },
            RuntimeFlowOpSeed::ReturnExpr(string_expr("after")),
        ],
    );
    let report = lower_plan(&plan);
    assert_eq!(
        run_entry(&report.program),
        VmExit::Returned(Some(RuntimeValue::String("after".to_owned())))
    );
    assert!(report.program.blocks.iter().any(|block| {
        block.safe_point == AwbcSafePointKind::LoopBackedge
            && matches!(block.terminator, AwbcTerminator::Jump { .. })
    }));
    assert!(
        !report
            .program
            .intrinsics
            .iter()
            .any(|intrinsic| { intrinsic.identity.as_label() == "flow.break" })
    );
}

#[test]
fn while_continue_targets_its_condition_header() {
    let main = flow_id("while.continue");
    let plan = build_bool_flow_plan(
        type_id(2),
        main,
        vec![
            RuntimeFlowOpSeed::While {
                condition: bool_expr(true),
                body: vec![RuntimeFlowOpSeed::Continue],
            },
            RuntimeFlowOpSeed::ReturnExpr(unit_expr()),
        ],
    );
    let report = lower_plan(&plan);
    assert!(
        report
            .program
            .blocks
            .iter()
            .enumerate()
            .any(|(index, block)| {
                matches!(
                    block.terminator,
                    AwbcTerminator::Jump { target }
                        if target.index() < index
                            && report.program.blocks[target.index()].safe_point
                                == AwbcSafePointKind::LoopBackedge
                )
            })
    );
    assert!(
        !report
            .program
            .intrinsics
            .iter()
            .any(|intrinsic| { intrinsic.identity.as_label() == "flow.continue" })
    );
}

#[test]
fn while_let_tests_pattern_and_guard_before_entering_its_body() {
    let false_match = build_while_let_plan(
        RuntimeValue::option_none(),
        false,
        vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("entered"))],
    );
    let false_match_report = lower_plan(&false_match);
    assert_eq!(
        run_entry(&false_match_report.program),
        VmExit::Returned(Some(RuntimeValue::String("after".to_owned())))
    );

    let false_guard = build_while_let_plan(
        RuntimeValue::option_some(RuntimeValue::Bool(false)),
        true,
        vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("entered"))],
    );
    let false_guard_report = lower_plan(&false_guard);
    assert_eq!(
        run_entry(&false_guard_report.program),
        VmExit::Returned(Some(RuntimeValue::String("after".to_owned())))
    );

    let matched = build_while_let_plan(
        RuntimeValue::option_some(RuntimeValue::Bool(true)),
        true,
        vec![RuntimeFlowOpSeed::Break(None)],
    );
    let matched_report = lower_plan(&matched);
    assert_eq!(
        run_entry(&matched_report.program),
        VmExit::Returned(Some(RuntimeValue::String("after".to_owned())))
    );

    let guard_true = build_while_let_plan(
        RuntimeValue::option_some(RuntimeValue::Bool(true)),
        true,
        vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("entered"))],
    );
    let guard_true_report = lower_plan(&guard_true);
    assert_eq!(
        run_entry(&guard_true_report.program),
        VmExit::Returned(Some(RuntimeValue::String("entered".to_owned())))
    );
}

#[test]
fn while_let_reevaluates_its_mutating_scrutinee_until_exhausted() {
    let report = lower_plan(&build_while_let_pop_front_plan());
    assert_eq!(
        run_entry(&report.program),
        VmExit::Returned(Some(RuntimeValue::String("after".to_owned())))
    );

    let loop_header = report
        .program
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, block)| {
            (block.safe_point == AwbcSafePointKind::LoopBackedge
                && matches!(
                    block.terminator,
                    AwbcTerminator::Jump { target }
                        if target.index() == index.saturating_add(1)
                ))
            .then_some(AwbcBlockId(
                u32::try_from(index).expect("test block index fits u32"),
            ))
        })
        .expect("while-let owns a dedicated loop header safe point");
    let mut fiber = FiberState::for_entry(&report.program, AwbcEntryId(0), 256, 256)
        .expect("fiber initializes");
    let mut at_between_iterations = false;
    for _ in 0..64 {
        let sequence_has_one_item = fiber
            .active_frame()
            .expect("active loop frame")
            .registers
            .iter()
            .flat_map(arcweft_core::value::RuntimePlaceStorage::values)
            .any(|value| matches!(value, RuntimeValue::Seq(sequence) if sequence.len() == 1));
        if fiber.cursor.block == loop_header && sequence_has_one_item {
            at_between_iterations = true;
            break;
        }
        assert_eq!(
            vm::step(
                &report.program,
                &mut fiber,
                VmStepOptions {
                    max_instructions: 1,
                },
            )
            .expect("single-instruction VM slice executes")
            .exit,
            VmExit::Running,
            "loop remains running before its second condition check"
        );
    }
    assert!(
        at_between_iterations,
        "the first condition pop and body complete before snapshot"
    );
    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).expect("mid-loop snapshot saves");
    let owner = arcweft_core::task::RuntimeProgramOwner::Awbc(Arc::new(report.program.clone()));
    let mut restored = snapshot
        .into_live_for_program(&owner)
        .expect("mid-loop snapshot restores to the same program");
    restored
        .validate_for_program(&report.program)
        .expect("restored loop state validates");
    assert_eq!(
        vm::step(
            &report.program,
            &mut restored,
            VmStepOptions {
                max_instructions: 128,
            },
        )
        .expect("restored while-let completes")
        .exit,
        VmExit::Returned(Some(RuntimeValue::String("after".to_owned())))
    );

    let encoded = report
        .program
        .encode_canonical()
        .expect("mutating while-let AWBC encodes canonically");
    let decoded = AwbcProgram::decode_canonical(
        &encoded,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .expect("mutating while-let AWBC decodes canonically");
    assert_eq!(
        run_entry(&decoded),
        VmExit::Returned(Some(RuntimeValue::String("after".to_owned())))
    );
}

#[test]
fn vec_push_and_pop_back_lower_to_place_mutation_instructions() {
    let report = lower_plan(&build_vec_push_pop_plan());
    assert!(
        report
            .program
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction,
                AwbcInstruction::VecPush {
                    place: AwbcMutablePlace::Local(_),
                    ..
                }
            ))
    );
    assert!(
        report
            .program
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction,
                AwbcInstruction::VecPop {
                    place: AwbcMutablePlace::Local(_),
                    ..
                }
            ))
    );
    assert_eq!(
        run_entry(&report.program),
        VmExit::Returned(Some(RuntimeValue::option_some(RuntimeValue::Bool(true))))
    );
}

#[test]
fn typed_runtime_ids_drive_static_goto_and_server_route_targets() {
    let main = flow_id("chapter.main");
    let next = flow_id("chapter.next");
    let plan = build_plan(
        type_id(1),
        [
            (main, vec![RuntimeFlowOpSeed::Goto(next.clone())]),
            (
                next.clone(),
                vec![RuntimeFlowOpSeed::ReturnExpr(string_expr("ok"))],
            ),
        ],
        [RuntimeEntrySpec {
            id: entry_id("server"),
            kind: RuntimeEntryKind::Server,
            binding: EntryBindingIdentity::from_bytes([1; 32]),
            target: RuntimeEntryTarget::Routes(vec![RuntimeRouteSpec {
                method: RuntimeHttpMethod::Get,
                path: RuntimeRoutePath::try_new([RuntimeRoutePathSegment::Literal(
                    "next".to_owned(),
                )])
                .expect("route path"),
                target: next,
                bindings: Vec::new(),
            }]),
            roles: RuntimeEntryRoles::None,
        }],
    );
    let report = lower_plan(&plan);
    let goto_target = report
        .program
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            AwbcTerminator::GotoStatic { function, .. } => Some(function),
            _ => None,
        })
        .expect("static goto lowers to a function target");
    let route_target = match &report.program.entries[0].target {
        AwbcEntryTarget::Routes(routes) => routes[0].target,
        AwbcEntryTarget::Function { .. } => panic!("test entry must lower as routes"),
    };
    assert_eq!(goto_target, route_target);
}

#[test]
fn await_observers_lower_to_progress_dispatch_and_rewait_backedge() {
    let main = flow_id("await.observer");
    let progress_type = type_id(4);
    let need_type = type_id(5);
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(type_id(1), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(type_id(2), RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(progress_type, RuntimePlanTypeProjection::Progress),
                RuntimePlanTypeSeed::new(need_type, RuntimePlanTypeProjection::Need(type_id(1))),
            ],
            [RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-runtime-plan.fixture.awbc_lower.tests.await_observers_lower_to_progress_dispatch_and_rewait_backedge.binding_a", false), need_type)],
        )
        .expect("Await observer types admit");
    let need_local = admission.local_ids()[0].clone();
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: main.clone(),
            parameters: vec![arcweft_core::entry::RuntimeFlowExecutableParameter {
                identity:
                    arcweft_core::plan::RuntimeFunctionParameterIdentity::from_accepted_identity(
                        [92; 32],
                    ),
                coordinate: arcweft_core::entry::FlowParameterCoordinate::from_position(0),
                name: "pending".to_owned(),
                mode: arcweft_core::entry::RuntimeFlowParameterMode::Owned,
                passing: arcweft_core::plan::RuntimeFunctionParameterPassing::Affine,
                semantic_identity: need_type,
            }],
        })
        .expect("Await observer flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(main.clone(), arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([61; 32]), None, Box::new([arcweft_core::plan::RuntimeFunctionInputBindingSeed { transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Formal, origin: arcweft_core::plan::RuntimeFunctionInputOrigin::Parameter(arcweft_core::plan::RuntimeFunctionParameterIdentity::from_accepted_identity([92;32])), source: arcweft_core::plan::RuntimeFunctionInputSource::Parameter { position:0, passing:arcweft_core::plan::RuntimeFunctionParameterPassing::Affine }, input_local: need_local.clone(), pattern: arcweft_core::plan::RuntimePatternSeed::new(need_type, arcweft_core::plan::RuntimePatternSeedKind::Bind { mutable:false, local: need_local.clone() }), ownership: arcweft_core::plan::RuntimeFunctionInputOwnershipRequirement::Owned, unrestricted_bindings: Box::new([]) }]), type_id(2), arcweft_core::plan::RuntimeEffectSet::empty()), arcweft_core::plan::RuntimeExecutableBodySeed { effects: arcweft_core::plan::RuntimeEffectSet::empty(), ops: (vec![RuntimeFlowOpSeed::Await {
                binding: None,
                target: RuntimeAwaitTargetSeed {
                    source: RuntimeExprSeed::new(
                        need_type,
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            need_local,
                            RuntimeLocalReadMode::Move,
                        )),
                    ),
                },
                observers: vec![RuntimeAwaitPendingObserverSeed {
                    pattern: RuntimePatternSeed::new(
                        progress_type,
                        RuntimePatternSeedKind::Discard,
                    ),
                    ops: vec![RuntimeFlowOpSeed::Noop],
                }],
            }]).into_boxed_slice() }))
        .expect("Await observer flow admits");
    let launch = flow_id("await.launch");
    builder
        .push_flow_executable(flow_executable(&launch))
        .expect("zero-argument launch flow executable admits");
    builder
        .push_flow_schema(flow_schema(&launch))
        .expect("zero-argument launch flow schema admits");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            launch.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                type_id(2),
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![RuntimeFlowOpSeed::Noop]).into_boxed_slice(),
            },
        ))
        .expect("zero-argument launch flow admits");
    builder
        .push_entry(flow_entry("await.observer", launch))
        .expect("Await observer entry admits");
    let report = lower_plan(&builder.finish().expect("Await observer plan seals"));
    let (await_index, observer) = report
        .program
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, block)| match block.terminator {
            AwbcTerminator::Await {
                observer: Some(observer),
                ..
            } => Some((index, observer)),
            _ => None,
        })
        .expect("Await retains a Progress observer resume");

    assert!(report.program.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            AwbcInstruction::TestPattern { value, .. } if *value == observer.destination
        )
    }));
    assert!(report.program.blocks.iter().any(|block| {
        matches!(
            block.terminator,
            AwbcTerminator::Jump { target } if target.index() == await_index
        )
    }));
}

#[test]
fn function_semantic_roles_survive_expression_and_executable_awbc_lowering() {
    use arcweft_core::awbc::schema::{AwbcFunctionKind, AwbcProgram};
    use arcweft_core::plan::{
        RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeFunctionSemanticRole,
        RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteBodySeed,
        RuntimeFunctionSiteDeclarationSeed, RuntimePureProgramBindingSeed,
    };
    for executable in [false, true] {
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    type_id(2),
                    RuntimePlanTypeProjection::Unit,
                )],
                [],
            )
            .unwrap();
        for &role in RuntimeFunctionSemanticRole::ALL {
            if role == RuntimeFunctionSemanticRole::Flow {
                continue;
            }
            let site = builder
                .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                    definition: arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
                    role,
                    function_type: None,
                    inputs: Box::new([]),
                    result: type_id(2),
                    body_kind: if executable {
                        RuntimeFunctionSiteBodyKind::Executable
                    } else {
                        RuntimeFunctionSiteBodyKind::Expression
                    },
                    effects: RuntimeEffectSet::empty(),
                })
                .unwrap();
            let body = if executable {
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: vec![RuntimeFlowOpSeed::ReturnExpr(unit_expr())].into_boxed_slice(),
                })
            } else {
                RuntimeFunctionSiteBodySeed::Expression(unit_expr())
            };
            builder.define_function_site_seed(&site, body).unwrap();
            let program = arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                [role.semantic_tag(); 32],
            );
            builder
                .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, site })
                .unwrap();
        }
        if executable {
            let flow = flow_id("role.root");
            builder.push_flow_schema(flow_schema(&flow)).unwrap();
            builder.push_flow_seed(RuntimeFlowSeed::new(flow, RuntimeFunctionSiteDeclarationSeed::flow(arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([42;32]), None, Box::new([]), type_id(2), RuntimeEffectSet::empty()), RuntimeExecutableBodySeed { effects: RuntimeEffectSet::empty(), ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(unit_expr())]) })).unwrap();
        }
        let plan = builder.finish().unwrap();
        let program = AwbcLowerer::new(
            &plan,
            &arcweft_text_model::DialogueContentCatalog::new(),
            "semantic-role",
        )
        .lower()
        .unwrap()
        .program;
        let encoded = program.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&encoded, Default::default()).unwrap();
        assert_eq!(decoded, program);
        let roots = decoded
            .functions
            .iter()
            .filter(|function| function.semantic_role == RuntimeFunctionSemanticRole::Flow)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), usize::from(executable));
        if let Some(root) = roots.first() {
            assert_eq!(root.kind, AwbcFunctionKind::Flow);
        }
        for binding in &decoded.pure_programs {
            let function = &decoded.functions[binding.function.index()];
            assert_eq!(function.kind, AwbcFunctionKind::Ordinary);
            assert_eq!(
                binding.program,
                arcweft_id::runtime_program::RuntimePureProgramId::from_checked_digest(
                    [function.semantic_role.semantic_tag(); 32]
                )
            );
        }
        assert_eq!(
            decoded.pure_programs.len(),
            RuntimeFunctionSemanticRole::ALL.len() - 1
        );
    }
}

fn manual_local_source(
    declaration: &str,
    mutable: bool,
) -> arcweft_core::plan::RuntimeLocalDeclarationSource {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    arcweft_core::plan::RuntimeLocalDeclarationSource::Binding {
        identity: *identity.finalize().as_bytes(),
        declaration: arcweft_core::plan::RuntimeLocalBindingDeclaration::new(
            arcweft_core::plan::RuntimeLocalBindingKind::PatternBinding,
            mutable,
            arcweft_core::plan::RuntimeLocalBindingStorage::Derived,
        ),
    }
}

#[test]
fn typed_unit_flow_completion_is_distinct_from_explicit_return_and_round_trips() {
    use arcweft_core::awbc::schema::AwbcFunctionKind;
    let main = flow_id("completion.root");
    for (ops, expected, natural) in [
        (vec![RuntimeFlowOpSeed::Noop], None, true),
        (
            vec![RuntimeFlowOpSeed::ReturnExpr(unit_expr())],
            Some(RuntimeValue::Unit),
            false,
        ),
    ] {
        let plan = build_plan(
            type_id(2),
            [(main.clone(), ops)],
            [flow_entry("completion", main.clone())],
        );
        let program = lower_plan(&plan).program;
        let root = program.flow_function(&main).unwrap();
        let row = &program.functions[root.index()];
        assert!(program.signatures[row.signature.index()].result.is_some());
        assert_eq!(
            program
                .blocks
                .iter()
                .any(|block| matches!(block.terminator, AwbcTerminator::Complete)),
            natural
        );
        assert_eq!(run_entry(&program), VmExit::Returned(expected.clone()));
        let decoded = AwbcProgram::decode_canonical(
            &program.encode_canonical().unwrap(),
            arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
        )
        .unwrap();
        assert_eq!(decoded, program);
        assert_eq!(run_entry(&decoded), VmExit::Returned(expected));
        if natural {
            let mut invalid = program.clone();
            invalid.functions[root.index()].kind = AwbcFunctionKind::Ordinary;
            assert!(
                invalid
                    .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                    .is_err()
            );
            let mut invalid = program.clone();
            let string_type = invalid
                .runtime_types
                .iter()
                .position(|ty| ty.semantic_identity() == type_id(1))
                .unwrap();
            let signature = invalid.functions[root.index()].signature;
            invalid.signatures[signature.index()].result = Some(
                arcweft_core::awbc::schema::AwbcTypeId(u32::try_from(string_type).unwrap()),
            );
            assert!(
                invalid
                    .verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default())
                    .is_err()
            );
        }
    }
}

#[test]
fn code_free_library_has_no_fabricated_source_map_location() {
    let plan = RuntimePlanBuilder::new().finish().unwrap();
    let report = AwbcLowerer::new(&plan, &DialogueContentCatalog::default(), "empty.arcw")
        .lower()
        .expect("code-free library verifies without a code location");
    assert!(report.program.blocks.is_empty());
    assert!(report.program.instructions.is_empty());
    assert!(report.program.source_map.is_empty());
    let relabeled = AwbcLowerer::new(
        &plan,
        &DialogueContentCatalog::default(),
        "relabeled-empty.arcw",
    )
    .lower()
    .unwrap();
    assert_eq!(report.program, relabeled.program);
}

#[test]
fn emitted_source_map_uses_actual_block_and_interned_source_file() {
    let flow = flow_id("source_map");
    let plan = build_plan(
        type_id(2),
        [(
            flow.clone(),
            vec![RuntimeFlowOpSeed::ReturnExpr(unit_expr())],
        )],
        [flow_entry("source_map", flow)],
    );
    let catalog = DialogueContentCatalog::default();
    let report = AwbcLowerer::new(&plan, &catalog, "source-map.arcw")
        .lower()
        .expect("emitted code and source map verify");
    assert!(!report.program.blocks.is_empty());
    let [entry] = report.program.source_map.as_slice() else {
        panic!("expected one coarse source-map entry");
    };
    assert_eq!(
        entry.location,
        arcweft_core::awbc::schema::AwbcCodeLocation::Block(AwbcBlockId(0))
    );
    assert_eq!(
        report.program.strings[entry.source_file.0 as usize],
        "source-map.arcw"
    );
    let disabled = AwbcLowerer::new(&plan, &catalog, "source-map.arcw")
        .with_options(AwbcLowerOptions {
            emit_source_map: false,
            ..AwbcLowerOptions::default()
        })
        .lower()
        .expect("source-map emission can be disabled");
    assert!(!disabled.program.blocks.is_empty());
    assert!(disabled.program.source_map.is_empty());
    let relabeled = AwbcLowerer::new(&plan, &catalog, "relabeled-source-map.arcw")
        .with_options(AwbcLowerOptions {
            emit_source_map: false,
            ..AwbcLowerOptions::default()
        })
        .lower()
        .unwrap();
    assert_eq!(disabled.program, relabeled.program);
}
