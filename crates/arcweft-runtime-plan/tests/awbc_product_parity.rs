//! Product-AWBC parity coverage built exclusively through the sealed plan builder.

use std::sync::Arc;

use arcweft_core::awbc::{
    codec::AwbcDecodeBudget,
    schema::AwbcEntryId,
    verify::{AwbcVerifyBudget, AwbcVerifyContext},
};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::entry::{
    EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
    RuntimeFlowSchema,
};
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    EntryRuntimeId, FlowEvent, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec,
    RuntimeEntryTarget, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSeed,
    RuntimeFunctionInputBindingSeed, RuntimeFunctionInputSource, RuntimeLocalDeclarationSeed,
    RuntimeLocalReadSeed, RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlan,
    RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    RuntimePureProgramBindingSeed,
};
use arcweft_core::pure::VmRuntimePureCallBackend;
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use arcweft_core::task::{
    GenerationId, HostRestartPolicy, LogicalEpoch, NeedProducerContractDigest,
    NeedProducerRequestProjection, NeedProducerSiteDigest, NeedProducerTaskPlan, TaskEvent,
    TaskEventKind, TaskPolicy, TaskPriority, TaskSequence,
};
use arcweft_core::value::{Progress, RuntimeLocalReadMode, RuntimeValue};
use arcweft_id::runtime_program::RuntimePureProgramId;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_text_model::DialogueContentCatalog;

const STRING_TYPE: RuntimeSemanticTypeId = RuntimeSemanticTypeId::from_bytes([1; 32]);

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::canonical(value).expect("test flow ID is valid")
}

fn type_id(marker: u8) -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([marker; 32])
}

fn entry_id() -> EntryRuntimeId {
    EntryRuntimeId::canonical("parity.start").expect("test entry ID is valid")
}

fn string(value: &str) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        STRING_TYPE,
        RuntimeExprSeedKind::Value(RuntimeValue::String(value.to_owned())),
    )
}

fn admit_flow_authority(builder: &mut RuntimePlanBuilder, flow: &FlowRuntimeId) {
    builder
        .push_flow_executable(RuntimeFlowExecutable {
            flow: flow.clone(),
            contract: FlowContractHash::from_bytes([0xf1; 32]),
            controller: None,
        })
        .expect("flow executable admits");
    builder
        .push_flow_schema(RuntimeFlowSchema {
            flow: flow.clone(),
            parameters: Vec::new(),
        })
        .expect("flow schema admits");
}

fn plan_with_return(value: &str) -> RuntimePlan {
    plan_with_return_and_entry(value, true)
}

fn plan_with_return_and_entry(value: &str, include_entry: bool) -> RuntimePlan {
    let flow = flow_id("parity.main");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                STRING_TYPE,
                RuntimePlanTypeProjection::String,
            )],
            [],
        )
        .expect("semantic facts admit");
    if include_entry {
        admit_flow_authority(&mut builder, &flow);
    } else {
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: flow.clone(),
                parameters: Vec::new(),
            })
            .expect("direct Flow schema admits without an entry executable");
    }
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                None,
                Box::new([]),
                STRING_TYPE,
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![RuntimeFlowOpSeed::ReturnExpr(string(value))]).into_boxed_slice(),
            },
        ))
        .expect("flow admits");
    if include_entry {
        builder
            .push_entry(RuntimeEntrySpec {
                id: entry_id(),
                kind: RuntimeEntryKind::Cli,
                binding: EntryBindingIdentity::from_bytes([2; 32]),
                target: RuntimeEntryTarget::Flow(flow),
                roles: RuntimeEntryRoles::None,
            })
            .expect("entry admits");
    }
    builder.finish().expect("builder seals plan")
}

fn plan_with_await_observer() -> RuntimePlan {
    plan_with_observed_producer(None)
}

fn plan_with_observed_producer(asset: Option<arcweft_core::task::AssetLoadKind>) -> RuntimePlan {
    plan_with_observed_producer_entry(asset, false)
}

fn plan_with_observed_producer_entry(
    asset: Option<arcweft_core::task::AssetLoadKind>,
    start_with_goto: bool,
) -> RuntimePlan {
    use arcweft_core::plan::*;
    let flow = flow_id("parity.await_observer");
    let progress_type = RuntimeSemanticTypeId::from_bytes([4; 32]);
    let need_type = RuntimeSemanticTypeId::from_bytes([5; 32]);
    let function_type = type_id(6);
    let unit_type = arcweft_core::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
    let launch_type = type_id(7);
    let contract = NeedProducerContractDigest::from_bytes([0x63; 32]);
    let entity_type =
        arcweft_core::pattern::RuntimeCheckedType::EntityReference.semantic_identity_digest();
    let (request, argument_types, arguments) = match asset {
        Some(kind) => (
            NeedProducerRequestProjection::AssetLoad {
                kind,
                argument_name: "素材".into(),
            },
            vec![entity_type],
            vec![arcweft_core::task::RuntimeHostArgumentTemplate::positional(
                arcweft_core::task::RuntimeRequestRoleIdentity::from_accepted_identity([0x65; 32]),
                RuntimeExprSeed::new(
                    entity_type,
                    RuntimeExprSeedKind::Value(RuntimeValue::EntityRef(
                        arcweft_core::value::RuntimeEntityReference::try_project(
                            arcweft_id::DeclarationIdentityFamily::Asset,
                            arcweft_id::PublicId::try_new("asset.positional").unwrap(),
                        )
                        .unwrap(),
                    )),
                ),
            )],
        ),
        None => (
            NeedProducerRequestProjection::ExternCapability {
                capability: arcweft_core::task::HostCapabilityId("probe".to_owned()),
                operation: "read".to_owned(),
                contract: arcweft_core::step::HostCallContractDigest::from_bytes([0x63; 32]),
                argument_names: Box::new([]),
            },
            Vec::new(),
            Vec::new(),
        ),
    };
    let producer_plan = NeedProducerTaskPlan::try_new(
        contract,
        NeedProducerSiteDigest::from_bytes([0x64; 32]),
        request,
        argument_types.into_boxed_slice(),
        STRING_TYPE,
        TaskPolicy::JoinSameKey,
        HostRestartPolicy::Restartable,
        arcweft_core::task::TaskClass::Io,
        TaskPriority(0),
        arcweft_core::task::CancelScopeId(flow.to_string()),
    )
    .expect("Await observer producer plan is internally consistent");
    let mut builder = RuntimePlanBuilder::new();
    if asset.is_some() {
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    entity_type,
                    RuntimePlanTypeProjection::EntityReference,
                )],
                [],
            )
            .unwrap();
    }
    let admitted = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(STRING_TYPE, RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(progress_type, RuntimePlanTypeProjection::Progress),
                RuntimePlanTypeSeed::new(need_type, RuntimePlanTypeProjection::Need(STRING_TYPE)),
                RuntimePlanTypeSeed::new(
                    function_type,
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::monomorphic(
                            arcweft_core::effect_row::EffectSet::new(),
                        ),
                        parameters: Box::new([]),
                        result: STRING_TYPE,
                    },
                ),
                RuntimePlanTypeSeed::new(unit_type, RuntimePlanTypeProjection::Unit),
                RuntimePlanTypeSeed::new(
                    launch_type,
                    RuntimePlanTypeProjection::Function {
                        contract: RuntimeFunctionTypeContract::monomorphic(
                            arcweft_core::effect_row::EffectSet::new(),
                        ),
                        parameters: Box::new([]),
                        result: unit_type,
                    },
                ),
            ],
            [RuntimeLocalDeclarationSeed::new(
                manual_local_source(
                    "arcweft-runtime-plan.fixture.tests.awbc_product_parity.plan_with_await_observer.binding_a",
                ),
                need_type,
            )],
        )
        .expect("Await observer facts admit");
    let need_local = admitted.local_ids()[0].clone();
    if start_with_goto {
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: flow.clone(),
                parameters: Vec::new(),
            })
            .expect("internal Flow schema admits without an entry-only executable contract");
    } else {
        admit_flow_authority(&mut builder, &flow);
    }
    let coordinate = builder
        .task_plan_construction()
        .reserve_original_source()
        .unwrap();
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            flow.clone(),
            arcweft_core::plan::RuntimeFunctionSiteDeclarationSeed::flow(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [61; 32],
                ),
                Some(function_type),
                Box::new([]),
                STRING_TYPE,
                arcweft_core::plan::RuntimeEffectSet::empty(),
            ),
            arcweft_core::plan::RuntimeExecutableBodySeed {
                effects: arcweft_core::plan::RuntimeEffectSet::empty(),
                ops: (vec![
                    RuntimeFlowOpSeed::StartNeedProducer {
                        binding: RuntimePatternSeed::new(
                            need_type,
                            RuntimePatternSeedKind::Bind {
                                mutable: false,
                                local: need_local.clone(),
                            },
                        ),
                        target: arcweft_core::plan::RuntimeNeedProducerStartTargetSeed {
                            plan: producer_plan,
                            task_plan: coordinate.clone(),
                            arguments,
                        },
                    },
                    RuntimeFlowOpSeed::Await {
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
                        observers: vec![
                            RuntimeAwaitPendingObserverSeed {
                                pattern: RuntimePatternSeed::new(
                                    progress_type,
                                    RuntimePatternSeedKind::Discard,
                                ),
                                ops: vec![RuntimeFlowOpSeed::ReturnExpr(string("first"))],
                            },
                            RuntimeAwaitPendingObserverSeed {
                                pattern: RuntimePatternSeed::new(
                                    progress_type,
                                    RuntimePatternSeedKind::Discard,
                                ),
                                ops: vec![RuntimeFlowOpSeed::ReturnExpr(string("second"))],
                            },
                        ],
                    },
                ])
                .into_boxed_slice(),
            },
        ))
        .expect("Await observer flow admits");
    let entry_flow = if start_with_goto {
        let launch = flow_id("parity.launch");
        admit_flow_authority(&mut builder, &launch);
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                launch.clone(),
                RuntimeFunctionSiteDeclarationSeed::flow(
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity([62; 32]),
                    Some(launch_type),
                    Box::new([]),
                    unit_type,
                    RuntimeEffectSet::empty(),
                ),
                RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([RuntimeFlowOpSeed::Goto(flow)]),
                },
            ))
            .expect("launch flow admits");
        launch
    } else {
        flow
    };
    builder
        .push_entry(RuntimeEntrySpec {
            id: entry_id(),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([2; 32]),
            target: RuntimeEntryTarget::Flow(entry_flow),
            roles: RuntimeEntryRoles::None,
        })
        .expect("Await observer entry admits");
    builder.finish().expect("Await observer plan seals")
}

fn lower(plan: &RuntimePlan) -> arcweft_core::awbc::schema::AwbcProgram {
    AwbcLowerer::new(
        plan,
        &DialogueContentCatalog::new(),
        "awbc_product_parity.arcw",
    )
    .lower()
    .expect("sealed plan lowers to AWBC")
    .program
}

fn options() -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops: 32 },
        ..RuntimeStepOptions::default()
    }
}

#[test]
fn owned_program_root_retains_affine_inputs_and_result_across_save_restore() {
    use arcweft_core::awbc::fiber::AwbcFiberRoot;
    use arcweft_core::awbc::product_step::{
        AwbcProductExecutorSaveSnapshot, AwbcProductStepExecutor,
    };
    let retained = reusable_bool_need();

    let boolean = arcweft_core::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
    let need = type_id(42);
    let id = RuntimePureProgramId::from_checked_digest([43; 32]);
    let mut builder = RuntimePlanBuilder::new();
    let locals = builder
        .admit_type_batch(
            [
                RuntimePlanTypeSeed::new(boolean, RuntimePlanTypeProjection::Bool),
                RuntimePlanTypeSeed::new(need, RuntimePlanTypeProjection::Need(boolean)),
            ],
            [RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-runtime-plan.fixture.tests.awbc_product_parity.owned_program_root_retains_affine_inputs_and_result_across_save_restore.binding_a"), need)],
        )
        .unwrap();
    let local = locals.local_ids()[0].clone();
    let input = RuntimeFunctionInputBindingSeed {
        transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Formal,
        origin: arcweft_core::plan::RuntimeFunctionInputOrigin::Parameter(
            arcweft_core::plan::RuntimeFunctionParameterIdentity::from_accepted_identity([81; 32]),
        ),
        source: RuntimeFunctionInputSource::Parameter {
            position: 0,
            passing: arcweft_core::plan::RuntimeFunctionParameterPassing::Affine,
        },
        input_local: local.clone(),
        pattern: RuntimePatternSeed::new(
            need,
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local: local.clone(),
            },
        ),
        ownership: Default::default(),
        unrestricted_bindings: Box::new([]),
    };
    let site = builder
        .push_function_site_seed(
            arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
            arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary,
            [input],
            RuntimeExprSeed::new(
                need,
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    local,
                    RuntimeLocalReadMode::Move,
                )),
            ),
        )
        .unwrap();
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program: id, site })
        .unwrap();
    let program = Arc::new(lower(&builder.finish().unwrap()));
    assert!(program.entries.is_empty());
    let rejected = AwbcProductStepExecutor::for_program_invocation(
        Arc::clone(&program),
        RuntimePureProgramId::from_checked_digest([44; 32]),
        vec![RuntimeValue::NeedHandle(retained.clone())],
        GenerationId::new(7),
        64,
    )
    .unwrap_err();
    let (_, inputs) = rejected.into_parts();
    assert_eq!(inputs, [RuntimeValue::NeedHandle(retained.clone())]);

    let executor = AwbcProductStepExecutor::for_program_invocation(
        Arc::clone(&program),
        id,
        inputs,
        GenerationId::new(7),
        64,
    )
    .unwrap();
    assert_eq!(executor.compact_fiber().root, AwbcFiberRoot::Program(id));
    let saved = executor.inert_rollback_image().unwrap();
    let wire = AwbcProductExecutorSaveSnapshot::from_live(
        &saved,
        &arcweft_core::task::RuntimeProgramOwner::Awbc(executor.program_arc()),
    )
    .unwrap();
    let bytes = serde_json::to_vec(&wire).unwrap();
    let saved: AwbcProductExecutorSaveSnapshot = serde_json::from_slice(&bytes).unwrap();
    let mut forged = saved.clone();
    forged.fiber.root = AwbcFiberRoot::Entry(AwbcEntryId(0));
    let (executor, _) = executor.restore_inert_snapshot_owned(forged).unwrap_err();
    assert_eq!(executor.compact_fiber().root, AwbcFiberRoot::Program(id));
    let mut forged = saved.clone();
    forged.fiber.root = AwbcFiberRoot::Function(program.pure_program_binding(id).unwrap().function);
    let (executor, _) = executor.restore_inert_snapshot_owned(forged).unwrap_err();
    assert_eq!(executor.compact_fiber().root, AwbcFiberRoot::Program(id));
    let mut executor = executor.restore_inert_snapshot_owned(saved).unwrap();
    let result = executor.step(RuntimeStepInput::default(), options());
    assert!(
        result.output.diagnostics.is_empty(),
        "{:?}",
        result.output.diagnostics
    );
    let saved = executor.inert_rollback_image().unwrap();
    let mut forged = saved.clone();
    forged.fiber.terminal = Some(arcweft_core::awbc::fiber::FiberTerminalValue::Returned(
        Some(arcweft_core::value::AwbcRuntimeValueSnapshot::Bool(true)),
    ));
    forged.fiber.return_summary = Some("true".to_owned());
    let (mut executor, _) = executor.restore_snapshot_owned(forged).unwrap_err();
    assert_eq!(
        executor.take_program_result().unwrap(),
        Some((id, RuntimeValue::NeedHandle(retained.clone())))
    );
    assert_eq!(executor.take_program_result().unwrap(), None);
    let wire = AwbcProductExecutorSaveSnapshot::from_live(
        &saved,
        &arcweft_core::task::RuntimeProgramOwner::Awbc(executor.program_arc()),
    )
    .unwrap();
    let bytes = serde_json::to_vec(&wire).unwrap();
    let saved: AwbcProductExecutorSaveSnapshot = serde_json::from_slice(&bytes).unwrap();
    let mut executor = executor.restore_inert_snapshot_owned(saved).unwrap();
    assert_eq!(
        executor.take_program_result().unwrap(),
        Some((id, RuntimeValue::NeedHandle(retained.clone())))
    );
    assert_eq!(executor.take_program_result().unwrap(), None);
}

#[test]
fn owned_unit_program_retains_its_typed_result_across_save_restore() {
    use arcweft_core::awbc::product_step::AwbcProductStepExecutor;

    let unit = type_id(45);
    let id = RuntimePureProgramId::from_checked_digest([46; 32]);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                unit,
                RuntimePlanTypeProjection::Unit,
            )],
            [],
        )
        .unwrap();
    let site = builder
        .push_function_site_seed(
            arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity([41; 32]),
            arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary,
            [],
            RuntimeExprSeed::new(unit, RuntimeExprSeedKind::Value(RuntimeValue::Unit)),
        )
        .unwrap();
    builder
        .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program: id, site })
        .unwrap();
    let program = Arc::new(lower(&builder.finish().unwrap()));
    let mut executor = AwbcProductStepExecutor::for_program_invocation(
        program,
        id,
        Vec::new(),
        GenerationId::new(7),
        64,
    )
    .unwrap();
    assert!(
        executor
            .step(RuntimeStepInput::default(), options())
            .output
            .diagnostics
            .is_empty()
    );
    let saved = executor.inert_rollback_image().unwrap();
    let mut executor = executor.restore_snapshot_owned(saved).unwrap();
    assert_eq!(
        executor.take_program_result().unwrap(),
        Some((id, RuntimeValue::Unit))
    );
    let saved = executor.inert_rollback_image().unwrap();
    let mut executor = executor.restore_snapshot_owned(saved).unwrap();
    assert_eq!(executor.take_program_result().unwrap(), None);
}

#[test]
fn flow_library_without_public_entry_preserves_verified_executable_roots() {
    let plan = plan_with_return_and_entry("library", false);
    assert!(plan.entries().is_empty());
    assert!(plan.pure_programs().is_empty());
    let program = lower(&plan);
    assert!(program.entries.is_empty());
    assert_eq!(program.flow_bindings.len(), 1);
    let bytes = program.encode_canonical().expect("flow library encodes");
    let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &bytes,
        AwbcDecodeBudget::default(),
    )
    .expect("flow library decodes");
    decoded
        .verify(
            AwbcVerifyBudget::default(),
            AwbcVerifyContext {
                require_entrypoint: false,
                ..AwbcVerifyContext::default()
            },
        )
        .expect("flow library passes complete structural verification");
    assert_eq!(decoded.flow_bindings, program.flow_bindings);
    assert_eq!(decoded.functions, program.functions);
    assert_eq!(
        decoded.verify(AwbcVerifyBudget::default(), AwbcVerifyContext::default()),
        Err(arcweft_core::awbc::verify::AwbcVerifyError::MissingEntrypoint),
        "public-entry execution must still require an entry"
    );
    assert!(matches!(
        AwbcLowerer::for_entry(
            &plan,
            &DialogueContentCatalog::default(),
            "selected-library.arcw",
            &entry_id(),
        )
        .lower(),
        Err(arcweft_runtime_plan::awbc_lower::AwbcLowerError::Lowering(
            _
        ))
    ));
}

#[test]
fn typed_runtime_plan_and_product_awbc_return_the_same_value() {
    let plan = plan_with_return("done");
    let program = lower(&plan);
    let mut native =
        ArcweftRuntimeExecutor::from_runtime_plan(plan, ArcweftExecutionTier::RuntimePlanVm)
            .expect("runtime-plan VM builds");
    native
        .start_structured_entry(&entry_id())
        .expect("runtime-plan entry starts");
    let mut product = ArcweftRuntimeExecutor::from_awbc_product(program, AwbcEntryId(0))
        .expect("AWBC product builds");
    let mut native_backend = VmRuntimePureCallBackend::default();
    let mut product_backend = VmRuntimePureCallBackend::default();

    let native_result =
        native.step_with_pure_backend(RuntimeStepInput::default(), options(), &mut native_backend);
    let product_result = product.step_with_pure_backend(
        RuntimeStepInput::default(),
        options(),
        &mut product_backend,
    );

    assert_eq!(product_result.output, native_result.output);
    assert_eq!(product_result.stop_reason, native_result.stop_reason);
    assert_eq!(product_result.fiber_status, native_result.fiber_status);
    assert!(matches!(
        product_result.fiber_status,
        FlowFiberStatus::Done(_)
    ));
}

#[test]
fn canonical_awbc_roundtrip_preserves_product_execution() {
    let program = lower(&plan_with_return("roundtrip"));
    let bytes = program
        .encode_canonical()
        .expect("AWBC encodes canonically");
    let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &bytes,
        AwbcDecodeBudget::default(),
    )
    .expect("canonical AWBC decodes");
    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0))
        .expect("decoded AWBC product builds");
    let result = executor.step(RuntimeStepInput::default(), options());

    assert!(matches!(result.fiber_status, FlowFiberStatus::Done(_)));
}

#[test]
fn canonical_awbc_rejects_tampered_payload() {
    let mut bytes = lower(&plan_with_return("tamper"))
        .encode_canonical()
        .expect("AWBC encodes canonically");
    let last = bytes.last_mut().expect("AWBC payload is nonempty");
    *last ^= 0x80;

    assert!(
        arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
            &bytes,
            AwbcDecodeBudget::default(),
        )
        .is_err()
    );
}

#[test]
fn decoded_awbc_carries_the_complete_original_program_source() {
    let original = plan_with_await_observer();
    let expected = selected_producer_key(&original);
    let program = lower(&original);
    let source = program
        .original_source
        .as_ref()
        .expect("lowering retains Source");
    assert_eq!(
        source.expected_executable(),
        original.executable_semantic_digest().as_bytes()
    );
    let bytes = program.encode_canonical().unwrap();
    let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &bytes,
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    let source = decoded
        .original_source
        .expect("canonical bytes retain Source");
    let restored = source
        .admit(
            arcweft_core::plan::RuntimeTaskSourceArtifactLimits::default(),
            None,
        )
        .unwrap();
    assert_eq!(
        restored.executable_semantic_digest(),
        original.executable_semantic_digest()
    );
    assert_eq!(restored.task_plans().len(), original.task_plans().len());
    assert!(restored.task_plans().by_digest(expected).is_some());
    assert_eq!(
        restored.task_source_artifact(Default::default()).unwrap(),
        source
    );
    let arcweft_core::plan::FlowOp::StartNeedProducer { target, .. } =
        &original.flows()[0].body().ops()[0]
    else {
        unreachable!()
    };
    assert!(restored.task_plan(target.task_plan()).is_none());
}

#[test]
fn decoded_selected_code_asserts_the_original_completed_task_key() {
    let original = plan_with_await_observer();
    let expected = selected_producer_key(&original);
    let bytes = lower(&original).encode_canonical().unwrap();
    let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &bytes,
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    let assertions: Vec<_> = decoded
        .task_plans
        .iter()
        .filter_map(|row| match &row.kind {
            arcweft_core::awbc::schema::AwbcTaskPlanKind::NeedProducer { plan, .. } => Some(*plan),
            _ => None,
        })
        .collect();
    assert_eq!(assertions, [expected.into()]);
}

#[test]
fn native_selected_need_uses_the_completed_program_task_plan_key() {
    let plan = plan_with_await_observer();
    let expected = selected_producer_key(&plan);
    let mut executor =
        ArcweftRuntimeExecutor::from_runtime_plan(plan, ArcweftExecutionTier::RuntimePlanVm)
            .expect("runtime-plan VM builds");
    executor
        .start_structured_entry(&entry_id())
        .expect("runtime-plan entry starts");
    let result = executor.step(RuntimeStepInput::default(), options());
    assert_eq!(result.output.requests.tasks.len(), 1);
    assert_eq!(
        result.output.requests.tasks[0].spec().producer.plan(),
        expected,
        "the emitted Need must retain the original program's completed F/Q/C key"
    );
}

#[test]
fn decoded_awbc_selected_need_uses_the_completed_program_task_plan_key() {
    let plan = plan_with_await_observer();
    let expected = selected_producer_key(&plan);
    let bytes = lower(&plan).encode_canonical().expect("canonical AWBC");
    let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &bytes,
        AwbcDecodeBudget::default(),
    )
    .expect("canonical AWBC decodes");
    let mut executor = ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0))
        .expect("decoded AWBC product builds");
    let result = executor.step(RuntimeStepInput::default(), options());
    assert_eq!(result.output.requests.tasks.len(), 1);
    assert_eq!(
        result.output.requests.tasks[0].spec().producer.plan(),
        expected,
        "decoding must preserve the original program's completed F/Q/C key"
    );
}

#[test]
fn native_positional_asset_need_uses_canonical_binding_and_original_key() {
    for kind in [
        arcweft_core::task::AssetLoadKind::Image,
        arcweft_core::task::AssetLoadKind::Voice,
    ] {
        let plan = plan_with_observed_producer(Some(kind));
        let expected = selected_producer_key(&plan);
        let mut executor =
            ArcweftRuntimeExecutor::from_runtime_plan(plan, ArcweftExecutionTier::RuntimePlanVm)
                .unwrap();
        executor.start_structured_entry(&entry_id()).unwrap();
        let result = executor.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        assert_eq!(result.output.requests.tasks.len(), 1);
        let spec = result.output.requests.tasks[0].spec();
        assert_eq!(spec.producer.plan(), expected);
        assert!(
            matches!(&spec.request, arcweft_core::task::HostTaskRequest::AssetLoad(request) if request.kind == kind.as_str() && request.id == "asset.positional")
        );
    }
}

#[test]
fn decoded_awbc_positional_asset_need_uses_canonical_binding_and_original_key() {
    for kind in [
        arcweft_core::task::AssetLoadKind::Image,
        arcweft_core::task::AssetLoadKind::Voice,
    ] {
        let plan = plan_with_observed_producer(Some(kind));
        let expected = selected_producer_key(&plan);
        let bytes = lower(&plan).encode_canonical().unwrap();
        let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
            &bytes,
            AwbcDecodeBudget::default(),
        )
        .unwrap();
        let mut executor =
            ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0)).unwrap();
        let result = executor.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        assert_eq!(result.output.requests.tasks.len(), 1);
        let spec = result.output.requests.tasks[0].spec();
        assert_eq!(spec.producer.plan(), expected);
        assert!(
            matches!(&spec.request, arcweft_core::task::HostTaskRequest::AssetLoad(request) if request.kind == kind.as_str() && request.id == "asset.positional")
        );
    }
}

fn selected_producer_key(plan: &RuntimePlan) -> arcweft_core::task::TaskPlanSemanticDigest {
    let arcweft_core::plan::FlowOp::StartNeedProducer { target, .. } =
        &plan.flows()[0].body().ops()[0]
    else {
        panic!("the fixture starts its actual selected producer")
    };
    plan.task_plan(target.task_plan())
        .expect("the actual producer belongs to the completed program")
        .key()
}

#[test]
fn native_and_decoded_awbc_goto_use_the_target_function_instantiation() {
    for kind in [
        arcweft_core::task::AssetLoadKind::Image,
        arcweft_core::task::AssetLoadKind::Voice,
    ] {
        let plan = plan_with_observed_producer_entry(Some(kind), true);
        let expected = selected_producer_key(&plan);
        let bytes = lower(&plan).encode_canonical().unwrap();
        let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
            &bytes,
            AwbcDecodeBudget::default(),
        )
        .unwrap();
        let mut native =
            ArcweftRuntimeExecutor::from_runtime_plan(plan, ArcweftExecutionTier::RuntimePlanVm)
                .unwrap();
        native.start_structured_entry(&entry_id()).unwrap();
        let mut product =
            ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0)).unwrap();
        for executor in [&mut native, &mut product] {
            let result = executor.step(RuntimeStepInput::default(), options());
            assert!(
                result.output.diagnostics.is_empty(),
                "{:?}",
                result.output.diagnostics
            );
            assert_eq!(result.output.requests.tasks.len(), 1);
            let spec = result.output.requests.tasks[0].spec();
            assert_eq!(spec.producer.plan(), expected);
            assert!(
                matches!(&spec.request, arcweft_core::task::HostTaskRequest::AssetLoad(request) if request.kind == kind.as_str() && request.id == "asset.positional")
            );
        }
    }
}

#[test]
fn product_awbc_matches_first_progress_observer_and_consumes_publication_once() {
    let plan = plan_with_await_observer();
    let program = lower(&plan);
    let mut native =
        ArcweftRuntimeExecutor::from_runtime_plan(plan, ArcweftExecutionTier::RuntimePlanVm)
            .expect("runtime-plan VM builds");
    native
        .start_structured_entry(&entry_id())
        .expect("runtime-plan entry starts");
    let mut product = ArcweftRuntimeExecutor::from_awbc_product(program, AwbcEntryId(0))
        .expect("AWBC product builds");
    let native_started = native.step(RuntimeStepInput::default(), options());
    let product_started = product.step(RuntimeStepInput::default(), options());
    assert_eq!(
        native_started.output.requests.tasks,
        product_started.output.requests.tasks
    );
    let task = native_started
        .output
        .requests
        .tasks
        .first()
        .expect("Need producer emits one task")
        .clone();
    let native_dispatch = native
        .restartable_dispatches()
        .into_iter()
        .find(|dispatch| dispatch.submission.task_id() == task.task_id())
        .expect("native Need producer is registered");
    let product_dispatch = product
        .restartable_dispatches()
        .into_iter()
        .find(|dispatch| dispatch.submission.task_id() == task.task_id())
        .expect("Product Need producer is registered");
    assert_eq!(
        native_dispatch.submission.handle().correlation,
        product_dispatch.submission.handle().correlation
    );
    let publication = || TaskEvent {
        correlation: task.handle().correlation,
        cursor: arcweft_core::task::TaskPublicationCursor {
            logical_epoch: LogicalEpoch(1),
            sequence: TaskSequence(1),
        },
        kind: TaskEventKind::Progress(Progress::new(0.5).expect("fixture Progress is valid")),
    };

    let native_result = native.step(
        RuntimeStepInput {
            task_events: vec![publication()],
            ..RuntimeStepInput::default()
        },
        options(),
    );
    let product_result = product.step(
        RuntimeStepInput {
            task_events: vec![publication()],
            ..RuntimeStepInput::default()
        },
        options(),
    );

    for result in [&native_result, &product_result] {
        assert_eq!(
            result
                .output
                .flow_events
                .iter()
                .filter(|event| matches!(event, FlowEvent::AwaitProgress { .. }))
                .count(),
            1
        );
        assert!(
            matches!(
                result.fiber_status,
                FlowFiberStatus::Done(arcweft_core::engine::FlowExit::Return(ref value))
                    if value == "first"
            ),
            "unexpected Await observer status: {:?}; diagnostics: {:?}",
            result.fiber_status,
            result.output.diagnostics
        );
    }

    let duplicate_native = native.step(
        RuntimeStepInput {
            task_events: vec![publication()],
            ..RuntimeStepInput::default()
        },
        options(),
    );
    let duplicate_product = product.step(
        RuntimeStepInput {
            task_events: vec![publication()],
            ..RuntimeStepInput::default()
        },
        options(),
    );
    for result in [&duplicate_native, &duplicate_product] {
        assert!(result.output.flow_events.is_empty());
        assert!(matches!(result.fiber_status, FlowFiberStatus::Done(_)));
    }
}

fn reusable_bool_need() -> arcweft_core::task::RuntimeNeedHandle {
    use arcweft_core::{
        pattern::RuntimeCheckedType,
        task::{
            CancelScopeId, HostTaskRequest, NeedProducerFamily, NeedProducerInstance,
            NeedProducerSpec, RuntimeTypeSemanticDigest, TaskClass, TaskOutcomeContract,
            TaskPlanSemanticDigest, TaskSpec,
        },
    };
    let outcome = TaskOutcomeContract::new(RuntimeCheckedType::Bool);
    let producer = NeedProducerSpec::new(
        NeedProducerFamily::HostAdapterTask,
        NeedProducerContractDigest::from_bytes([1; 32]),
        TaskPlanSemanticDigest::from_bytes([2; 32]),
        NeedProducerSiteDigest::from_bytes([3; 32]),
        RuntimeTypeSemanticDigest::from_bytes(*outcome.payload_semantic_identity().as_bytes()),
        RuntimeValue::Tuple(vec![]).try_digest(1024).unwrap(),
    );
    arcweft_core::task::RuntimeNeedHandle::try_from(TaskSpec {
        generation: GenerationId::new(7),
        producer: NeedProducerInstance::try_from(&producer).unwrap(),
        class: TaskClass::Background,
        priority: TaskPriority(0),
        cancel_scope: CancelScopeId("fixture".into()),
        policy: TaskPolicy::JoinSameKey,
        outcome,
        request: (HostTaskRequest::custom("fixture", "bool", [])).into(),
        debug_label: "retained".into(),
    })
    .unwrap()
}

fn manual_local_source(declaration: &str) -> arcweft_core::plan::RuntimeLocalDeclarationSource {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    arcweft_core::plan::RuntimeLocalDeclarationSource::Binding {
        identity: *identity.finalize().as_bytes(),
        declaration: arcweft_core::plan::RuntimeLocalBindingDeclaration::new(
            arcweft_core::plan::RuntimeLocalBindingKind::PatternBinding,
            false,
            arcweft_core::plan::RuntimeLocalBindingStorage::Derived,
        ),
    }
}

#[test]
fn dense_constants_preserve_exact_bits_in_native_decoded_awbc_and_saved_results() {
    use arcweft_core::awbc::product_step::{
        AwbcProductExecutorSaveSnapshot, AwbcProductStepExecutor,
    };

    let f32_bits = [0x3f80_0000, 0x8000_0000, 0x7f80_0000, 0x7fc0_1234];
    let f64_bits = [
        0x3ff0_0000_0000_0000,
        0x8000_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0x7ff8_0000_0000_1234,
    ];
    let (plan, cases) = dense_constant_plan(f32_bits, f64_bits);
    let encoded = lower(&plan).encode_canonical().unwrap();
    let decoded = arcweft_core::awbc::schema::AwbcProgram::decode_canonical(
        &encoded,
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    assert!(decoded.entries.is_empty());
    assert_eq!(decoded.pure_programs.len(), 4);
    decoded
        .verify(
            AwbcVerifyBudget::default(),
            AwbcVerifyContext {
                require_entrypoint: false,
                ..AwbcVerifyContext::default()
            },
        )
        .unwrap();
    assert_eq!(decoded.encode_canonical().unwrap(), encoded);
    let program = Arc::new(decoded);
    for (id, ty) in cases {
        let mut native_backend = VmRuntimePureCallBackend::default();
        let native = arcweft_core::pure::evaluate_pure_program_with_backend(
            &plan,
            id,
            &[],
            &mut native_backend,
        )
        .unwrap();
        assert_dense_bits(&native, ty, &f32_bits, &f64_bits);
        let mut executor = AwbcProductStepExecutor::for_program_invocation(
            Arc::clone(&program),
            id,
            Vec::new(),
            GenerationId::new(7),
            64,
        )
        .unwrap();
        let result = executor.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        assert!(matches!(result.fiber_status, FlowFiberStatus::Done(_)));
        let save = AwbcProductExecutorSaveSnapshot::from_live(
            &executor.snapshot().unwrap(),
            &arcweft_core::task::RuntimeProgramOwner::Awbc(Arc::clone(&program)),
        )
        .unwrap();
        let bytes = serde_json::to_vec(&save).unwrap();
        let decoded_save: AwbcProductExecutorSaveSnapshot = serde_json::from_slice(&bytes).unwrap();
        let snapshot = decoded_save
            .into_live_for_program(executor.admitted_program())
            .unwrap();
        executor = executor.restore_snapshot_owned(snapshot).unwrap();
        let (returned_id, value) = executor.take_program_result().unwrap().unwrap();
        assert_eq!(returned_id, id);
        assert_dense_bits(&value, ty, &f32_bits, &f64_bits);
        assert!(executor.take_program_result().unwrap().is_none());
    }
}

fn dense_constant_plan(
    f32_bits: [u32; 4],
    f64_bits: [u64; 4],
) -> (
    Arc<RuntimePlan>,
    Vec<(
        RuntimePureProgramId,
        arcweft_core::math::RuntimeDenseNumericType,
    )>,
) {
    use arcweft_core::math::{
        DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64,
        RuntimeDenseNumericType as Dense, RuntimeFloatWidth as Width,
    };
    use arcweft_core::pattern::RuntimeCheckedType;

    let values = [
        RuntimeValue::MatrixF32(
            DenseMatrixF32::new(2, 2, f32_bits.map(f32::from_bits).to_vec()).unwrap(),
        ),
        RuntimeValue::MatrixF64(
            DenseMatrixF64::new(2, 2, f64_bits.map(f64::from_bits).to_vec()).unwrap(),
        ),
        RuntimeValue::TensorF32(
            DenseTensorF32::new(vec![2, 2], f32_bits.map(f32::from_bits).to_vec()).unwrap(),
        ),
        RuntimeValue::TensorF64(
            DenseTensorF64::new(vec![2, 2], f64_bits.map(f64::from_bits).to_vec()).unwrap(),
        ),
    ];
    let types = [
        Dense::Matrix(Width::F32),
        Dense::Matrix(Width::F64),
        Dense::Tensor(Width::F32),
        Dense::Tensor(Width::F64),
    ];
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            types.map(|ty| {
                RuntimePlanTypeSeed::new(
                    RuntimeCheckedType::DenseNumeric(ty).semantic_identity_digest(),
                    RuntimePlanTypeProjection::DenseNumeric(ty),
                )
            }),
            [],
        )
        .unwrap();
    let mut cases = Vec::new();
    for (index, (ty, value)) in types.into_iter().zip(values).enumerate() {
        let marker = u8::try_from(index).expect("four dense roles fit the fixture marker");
        let site = builder
            .push_function_site_seed(
                arcweft_core::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    [marker + 101; 32],
                ),
                arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary,
                [],
                RuntimeExprSeed::new(
                    RuntimeCheckedType::DenseNumeric(ty).semantic_identity_digest(),
                    RuntimeExprSeedKind::Value(value),
                ),
            )
            .unwrap();
        let program = RuntimePureProgramId::from_checked_digest([marker + 111; 32]);
        builder
            .push_pure_program_binding_seed(&RuntimePureProgramBindingSeed { program, site })
            .unwrap();
        cases.push((program, ty));
    }
    let plan = Arc::new(builder.finish().unwrap());
    (plan, cases)
}

fn assert_dense_bits(
    value: &RuntimeValue,
    ty: arcweft_core::math::RuntimeDenseNumericType,
    f32_bits: &[u32],
    f64_bits: &[u64],
) {
    use arcweft_core::math::{RuntimeDenseNumericType as Dense, RuntimeFloatWidth as Width};
    match (ty, value) {
        (Dense::Matrix(Width::F32), RuntimeValue::MatrixF32(value)) => {
            assert_eq!((value.rows(), value.cols()), (2, 2));
            assert_eq!(
                value
                    .values()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                f32_bits
            );
        }
        (Dense::Matrix(Width::F64), RuntimeValue::MatrixF64(value)) => {
            assert_eq!((value.rows(), value.cols()), (2, 2));
            assert_eq!(
                value
                    .values()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                f64_bits
            );
        }
        (Dense::Tensor(Width::F32), RuntimeValue::TensorF32(value)) => {
            assert_eq!(value.shape().dims(), &[2, 2]);
            assert_eq!(
                value
                    .values()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                f32_bits
            );
        }
        (Dense::Tensor(Width::F64), RuntimeValue::TensorF64(value)) => {
            assert_eq!(value.shape().dims(), &[2, 2]);
            assert_eq!(
                value
                    .values()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                f64_bits
            );
        }
        (ty, value) => panic!("dense kind/width changed: {ty:?} {value:?}"),
    }
}
