use super::*;
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{
    BundleLaunchKind, BundleManifest, BundleRuntimeSummary, BundleVirtualFileSpace,
};
use arcweft_core::awbc::schema::{
    AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
    AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
    AwbcFunctionKind, AwbcProgram, AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcStringId,
    AwbcTableRange, AwbcTerminator, AwbcTrapCode,
};
use arcweft_core::bytecode::{
    BYTECODE_ABI_VERSION, BytecodeEntry, BytecodeFlow, BytecodeInstruction,
};
use arcweft_core::entry::{
    CallableContractHash, EntryBindingIdentity, FlowContractHash, RootExecutionLimits,
    RuntimeCallableId, RuntimeCallableRole, RuntimeCommandPolicy, RuntimeFlowRole,
    RuntimeNominalRole, RuntimeNominalTypeId, RuntimeStatefulEntryRoles, RuntimeTypeSchema,
    TypeLayoutHash as CoreTypeLayoutHash,
};
use arcweft_core::plan::{
    EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntryTarget,
};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};

fn test_dialogue_revision() -> DialogueProfileRevision {
    test_dialogue_revision_with_view_byte(0x5a)
}

fn test_dialogue_revision_with_view_byte(view_byte: u8) -> DialogueProfileRevision {
    let manifest = SourceDocument::try_new(
        SourceDocumentId::try_new("runtime-driver-swap-test").expect("document ID"),
        SourceName::Memory,
        "test manifest",
    )
    .expect("test document");
    let sources =
        SourceSetRevision::try_for_identities([manifest.identity()]).expect("test source revision");
    DialogueProfileRevision::from_admitted_parts(
        manifest.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.runtime-driver-swap-test").expect("View program ID"),
        AcceptedViewProgramRevision::try_from_bytes([view_byte; 32])
            .expect("View program revision"),
        ResourceTypeRegistry::empty().digest(),
    )
}

fn digest(value: &[u8]) -> BundleDigest {
    BundleDigest::of(value)
}

fn test_entry_id(name: &str) -> EntryRuntimeId {
    EntryRuntimeId::from_source_entity_body(name).expect("test entry ID")
}

fn stateful_compatibility(
    entry: &EntryRuntimeId,
    state_layout: CoreTypeLayoutHash,
) -> EntryCompatibility {
    EntryCompatibility::Stateful(StatefulEntryCompatibility {
        kind: RuntimeEntryKind::Game,
        binding: EntryBindingIdentity::from_bytes([4; 32]),
        state_identity: RuntimeNominalTypeId::try_new(format!("{}.State", entry.canonical_label()))
            .expect("state identity"),
        state_layout,
        event_identity: RuntimeNominalTypeId::try_new(format!("{}.Event", entry.canonical_label()))
            .expect("event identity"),
        event_layout: CoreTypeLayoutHash::from_bytes([6; 32]),
        initializer: RuntimeCallableRole {
            callable: RuntimeCallableId::try_new(format!("{}.initial", entry.canonical_label()))
                .expect("initializer identity"),
            contract: CallableContractHash::from_bytes([7; 32]),
        },
        reducer: RuntimeCallableRole {
            callable: RuntimeCallableId::try_new(format!("{}.reduce", entry.canonical_label()))
                .expect("reducer identity"),
            contract: CallableContractHash::from_bytes([8; 32]),
        },
        initial_flow: RuntimeFlowRole {
            flow: FlowRuntimeId::from_runtime_target_value("flow.main").expect("initial flow ID"),
            contract: FlowContractHash::from_bytes([9; 32]),
        },
    })
}

fn agent_compatibility(entry: &EntryRuntimeId) -> EntryCompatibility {
    EntryCompatibility::Agent(AgentEntryCompatibility {
        kind: RuntimeEntryKind::Agent,
        binding: EntryBindingIdentity::from_bytes([21; 32]),
        controller: RuntimeCallableRole {
            callable: RuntimeCallableId::try_new(format!("{}.controller", entry.canonical_label()))
                .expect("controller identity"),
            contract: CallableContractHash::from_bytes([22; 32]),
        },
        policy: AgentPolicyHash::from_bytes([23; 32]),
        budget: AgentBudget {
            logical_timeout_millis: 1_000,
            max_vm_steps: 10_000,
            max_host_calls: 10,
            max_observations: 20,
            max_captures: 2,
            max_capture_bytes: 4_096,
            max_rag_queries: 3,
            max_context_bytes: 8_192,
        },
    })
}

fn generation(id: u64, code: &'static [u8], content: &'static [u8]) -> Arc<ProgramGeneration> {
    let entry = test_entry_id("entry.main");
    let state_layout = CoreTypeLayoutHash::from_bytes(digest(b"state-layout").as_bytes());
    Arc::new(ProgramGeneration {
        id: GenerationId(id),
        content_root: digest(content),
        dialogue_revision: test_dialogue_revision(),
        bytecode_abi: BYTECODE_ABI_VERSION,
        code_slots: BTreeMap::from([(
            CodeSlotId("main".to_owned()),
            CodeSlot {
                signature: RuntimeSignature {
                    params: digest(b"params"),
                    result: digest(b"result"),
                    effects: digest(b"effects"),
                },
                code_digest: digest(code),
            },
        )]),
        state_layouts: BTreeMap::from([(
            StateId::for_entry_root(&entry),
            TypeLayoutHash(BundleDigest::from_bytes(*state_layout.as_bytes())),
        )]),
        entry_compatibility: BTreeMap::from([(
            entry.clone(),
            stateful_compatibility(&entry, state_layout),
        )]),
        adapter_requirements: digest(b"adapter"),
    })
}

#[test]
fn content_only_swap_does_not_require_quiescence_semantically() {
    let active = generation(1, b"code", b"old-content");
    let next = generation(2, b"code", b"new-content");

    let compatibility = classify_swap(&active, &next);

    assert_eq!(compatibility, SwapCompatibility::ContentOnly);
    assert!(compatibility.can_apply_live());
    assert!(!compatibility.requires_quiescence());
    assert_eq!(compatibility.label(), "content-only");
}

#[test]
fn dialogue_profile_revision_change_requires_presentation_reset_boundary() {
    let active = generation(1, b"code", b"content");
    let mut next = (*generation(2, b"code", b"content")).clone();
    next.dialogue_revision = test_dialogue_revision_with_view_byte(0x6b);

    let compatibility = classify_swap(&active, &next);

    assert_eq!(compatibility, SwapCompatibility::CodeCompatible);
    assert!(compatibility.can_apply_live());
    assert!(compatibility.requires_quiescence());
}

#[test]
fn compatibility_max_preserves_the_stricter_policy() {
    assert_eq!(
        SwapCompatibility::ContentOnly.max(SwapCompatibility::CodeCompatible),
        SwapCompatibility::CodeCompatible
    );
    assert_eq!(
        SwapCompatibility::CodeGenerational.max(SwapCompatibility::CodeCompatible),
        SwapCompatibility::CodeGenerational
    );
    assert_eq!(
        SwapCompatibility::RestartRequired.max(SwapCompatibility::ContentOnly),
        SwapCompatibility::RestartRequired
    );
}

#[test]
fn compatible_code_swap_commits_between_steps_and_retires_after_pins_drop() {
    let active = generation(1, b"old-code", b"content");
    let mut session = SwapSession::new(active);
    let fiber_pin = session.pin_active_generation();
    let next = generation(2, b"new-code", b"content");

    assert_eq!(
        session.prepare(next).expect("prepare"),
        SwapCompatibility::CodeCompatible
    );
    session.begin_quiescence().expect("quiesce");
    session.enter_runtime_step();
    assert_eq!(session.commit(), Err(SwapError::RuntimeNotQuiescent));
    session.finish_runtime_step();
    assert_eq!(
        session.commit().expect("commit"),
        SwapCompatibility::CodeCompatible
    );

    session.retire_unused();
    assert_eq!(session.phase(), SwapPhase::Retiring);
    assert_eq!(session.retired().len(), 1);
    drop(fiber_pin);
    session.retire_unused();
    assert_eq!(session.phase(), SwapPhase::Idle);
    assert!(session.retired().is_empty());
}

#[test]
fn state_layout_change_requires_restart() {
    let active = generation(1, b"code", b"content");
    let mut next = (*generation(2, b"new-code", b"content")).clone();
    let entry = test_entry_id("entry.main");
    next.state_layouts.insert(
        StateId::for_entry_root(&entry),
        TypeLayoutHash(digest(b"changed-layout")),
    );

    assert_eq!(
        classify_swap_for_entry(&active, &next, &entry),
        SwapCompatibility::RestartRequired
    );
}

#[test]
fn hot_007_verified_executable_generation_populates_the_selected_root_layout() {
    let mut bytecode = test_bytecode(vec![BytecodeInstruction::Flow(FlowOp::Noop)]);
    let entry = bytecode.entries[0].id.clone();
    let flow = flow_id("flow.main");
    let state_schema = RuntimeTypeSchema::I64;
    let state_layout = state_schema.try_layout_hash().expect("state layout");
    let event_schema = RuntimeTypeSchema::String;
    let event_layout = event_schema.try_layout_hash().expect("event layout");
    let binding = EntryBindingIdentity::from_bytes([4; 32]);
    bytecode.entries[0].kind = RuntimeEntryKind::Game;
    bytecode.entries[0].binding = binding;
    bytecode.entries[0].roles = RuntimeEntryRoles::Stateful(Box::new(RuntimeStatefulEntryRoles {
        binding,
        state: RuntimeNominalRole {
            identity: RuntimeNominalTypeId::try_new("GameState").expect("state identity"),
            layout: state_layout,
            schema: state_schema,
        },
        initializer: RuntimeCallableRole {
            callable: RuntimeCallableId::try_new("game.initial").expect("initializer"),
            contract: CallableContractHash::from_bytes([7; 32]),
        },
        event: RuntimeNominalRole {
            identity: RuntimeNominalTypeId::try_new("GameEvent").expect("event identity"),
            layout: event_layout,
            schema: event_schema,
        },
        reducer: RuntimeCallableRole {
            callable: RuntimeCallableId::try_new("game.reduce").expect("reducer"),
            contract: CallableContractHash::from_bytes([8; 32]),
        },
        initial_flow: RuntimeFlowRole {
            flow,
            contract: FlowContractHash::from_bytes([9; 32]),
        },
        command_policy: RuntimeCommandPolicy::deny_all(RootExecutionLimits::engine_default()),
    }));

    let generation = ProgramGeneration::from_verified_bytecode(
        GenerationId(1),
        &bytecode,
        digest(b"content"),
        digest(b"adapter"),
        test_dialogue_revision(),
    )
    .expect("verified executable generation");

    assert_eq!(
        generation
            .state_layouts
            .get(&StateId::for_entry_root(&entry)),
        Some(&TypeLayoutHash(BundleDigest::from_bytes(
            *state_layout.as_bytes()
        )))
    );
    assert!(matches!(
        generation.entry_compatibility.get(&entry),
        Some(EntryCompatibility::Stateful(compatibility))
            if compatibility.state_layout == state_layout
    ));
}

#[test]
fn unselected_entry_contract_changes_do_not_restart_the_active_entry() {
    let active = generation(1, b"code", b"content");
    let mut next = (*generation(2, b"new-code", b"content")).clone();
    let active_entry = test_entry_id("entry.main");
    let unselected = test_entry_id("entry.unselected");
    let changed_layout = CoreTypeLayoutHash::from_bytes([33; 32]);
    next.entry_compatibility.insert(
        unselected.clone(),
        stateful_compatibility(&unselected, changed_layout),
    );
    next.state_layouts.insert(
        StateId::for_entry_root(&unselected),
        TypeLayoutHash(BundleDigest::from_bytes(*changed_layout.as_bytes())),
    );

    assert_eq!(
        classify_swap_for_entry(&active, &next, &active_entry),
        SwapCompatibility::CodeCompatible
    );
}

#[test]
fn every_active_stateful_entry_contract_field_is_hot_swap_critical() {
    let active = generation(1, b"code", b"content");
    let entry = test_entry_id("entry.main");

    macro_rules! assert_restart_after {
        ($mutation:expr) => {{
            let mut next = (*generation(2, b"new-code", b"content")).clone();
            let EntryCompatibility::Stateful(compatibility) = next
                .entry_compatibility
                .get_mut(&entry)
                .expect("active entry compatibility")
            else {
                panic!("fixture is stateful");
            };
            $mutation(compatibility);
            assert_eq!(
                classify_swap_for_entry(&active, &next, &entry),
                SwapCompatibility::RestartRequired
            );
        }};
    }

    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.kind = RuntimeEntryKind::Editor;
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.binding = EntryBindingIdentity::from_bytes([10; 32]);
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.state_identity =
            RuntimeNominalTypeId::try_new("ChangedState").expect("identity");
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.state_layout = CoreTypeLayoutHash::from_bytes([11; 32]);
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.event_identity =
            RuntimeNominalTypeId::try_new("ChangedEvent").expect("identity");
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.event_layout = CoreTypeLayoutHash::from_bytes([12; 32]);
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.initializer.callable =
            RuntimeCallableId::try_new("changed.initial").expect("identity");
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.initializer.contract = CallableContractHash::from_bytes([13; 32]);
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.reducer.callable =
            RuntimeCallableId::try_new("changed.reduce").expect("identity");
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.reducer.contract = CallableContractHash::from_bytes([14; 32]);
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.initial_flow.flow =
            FlowRuntimeId::from_runtime_target_value("flow.changed").expect("flow ID");
    });
    assert_restart_after!(|compatibility: &mut StatefulEntryCompatibility| {
        compatibility.initial_flow.contract = FlowContractHash::from_bytes([15; 32]);
    });
}

#[test]
fn every_active_agent_execution_contract_field_is_hot_swap_critical() {
    let entry = test_entry_id("entry.main");
    let mut active = (*generation(1, b"code", b"content")).clone();
    active
        .entry_compatibility
        .insert(entry.clone(), agent_compatibility(&entry));

    macro_rules! assert_agent_restart_after {
        ($mutation:expr) => {{
            let mut next = (*generation(2, b"new-code", b"content")).clone();
            next.entry_compatibility
                .insert(entry.clone(), agent_compatibility(&entry));
            let EntryCompatibility::Agent(compatibility) = next
                .entry_compatibility
                .get_mut(&entry)
                .expect("active entry compatibility")
            else {
                panic!("fixture is an Agent entry");
            };
            $mutation(compatibility);
            assert_eq!(
                classify_swap_for_entry(&active, &next, &entry),
                SwapCompatibility::RestartRequired
            );
        }};
    }

    assert_agent_restart_after!(|compatibility: &mut AgentEntryCompatibility| {
        compatibility.controller.callable =
            RuntimeCallableId::try_new("changed.controller").expect("identity");
    });
    assert_agent_restart_after!(|compatibility: &mut AgentEntryCompatibility| {
        compatibility.controller.contract = CallableContractHash::from_bytes([24; 32]);
    });
    assert_agent_restart_after!(|compatibility: &mut AgentEntryCompatibility| {
        compatibility.policy = AgentPolicyHash::from_bytes([25; 32]);
    });
    assert_agent_restart_after!(|compatibility: &mut AgentEntryCompatibility| {
        compatibility.budget.max_vm_steps += 1;
    });
}

#[test]
fn active_entry_role_family_change_requires_restart() {
    let active = generation(1, b"code", b"content");
    let entry = test_entry_id("entry.main");
    let mut next = (*generation(2, b"new-code", b"content")).clone();
    next.entry_compatibility
        .insert(entry.clone(), agent_compatibility(&entry));

    assert_eq!(
        classify_swap_for_entry(&active, &next, &entry),
        SwapCompatibility::RestartRequired
    );
}

#[test]
fn missing_active_entry_or_selected_root_layout_requires_restart() {
    let active = generation(1, b"code", b"content");
    let entry = test_entry_id("entry.main");

    let mut missing_entry = (*generation(2, b"new-code", b"content")).clone();
    missing_entry.entry_compatibility.remove(&entry);
    assert_eq!(
        classify_swap_for_entry(&active, &missing_entry, &entry),
        SwapCompatibility::RestartRequired
    );

    let mut missing_layout = (*generation(2, b"new-code", b"content")).clone();
    missing_layout
        .state_layouts
        .remove(&StateId::for_entry_root(&entry));
    assert_eq!(
        classify_swap_for_entry(&active, &missing_layout, &entry),
        SwapCompatibility::RestartRequired
    );
}

#[test]
fn missing_active_code_signature_is_generational() {
    let active = generation(1, b"code", b"content");
    let mut next = (*generation(2, b"new-code", b"content")).clone();
    next.code_slots.clear();

    assert_eq!(
        classify_swap(&active, &next),
        SwapCompatibility::CodeGenerational
    );
}

#[test]
fn generation_from_bundle_classifies_content_only_when_only_content_changes() {
    let active = ProgramGeneration::from_bundle(
        GenerationId(1),
        &test_bundle(
            test_bytecode(vec![BytecodeInstruction::Flow(FlowOp::Noop)]),
            b"old",
        ),
    )
    .expect("active generation");
    let next = ProgramGeneration::from_bundle(
        GenerationId(2),
        &test_bundle(
            test_bytecode(vec![BytecodeInstruction::Flow(FlowOp::Noop)]),
            b"new",
        ),
    )
    .expect("next generation");

    assert_ne!(active.content_root, next.content_root);
    assert_eq!(active.code_slots, next.code_slots);
    assert_eq!(
        classify_swap(&active, &next),
        SwapCompatibility::ContentOnly
    );
}

#[test]
fn generation_from_bundle_treats_structured_bytecode_change_as_generational() {
    let active = ProgramGeneration::from_bundle(
        GenerationId(1),
        &test_bundle(
            test_bytecode(vec![BytecodeInstruction::Flow(FlowOp::Noop)]),
            b"asset",
        ),
    )
    .expect("active generation");
    let next = ProgramGeneration::from_bundle(
        GenerationId(2),
        &test_bundle(
            test_bytecode(vec![BytecodeInstruction::Flow(FlowOp::Return(
                "done".to_owned(),
            ))]),
            b"asset",
        ),
    )
    .expect("next generation");

    assert_eq!(
        classify_swap(&active, &next),
        SwapCompatibility::CodeGenerational
    );
}

#[test]
fn generation_from_bundle_uses_product_awbc_function_identity() {
    let active = ProgramGeneration::from_bundle(
        GenerationId(1),
        &test_bundle(BytecodeProgram::default(), b"asset")
            .with_product_awbc(test_awbc_program("revision-a")),
    )
    .expect("active AWBC generation");
    let next = ProgramGeneration::from_bundle(
        GenerationId(2),
        &test_bundle(BytecodeProgram::default(), b"asset")
            .with_product_awbc(test_awbc_program("revision-b")),
    )
    .expect("next AWBC generation");

    assert_eq!(active.content_root, next.content_root);
    assert_ne!(active.code_slots, next.code_slots);
    assert_eq!(
        classify_swap(&active, &next),
        SwapCompatibility::CodeCompatible
    );
}

#[test]
fn generation_from_bundle_rejects_unverified_bytecode() {
    let mut bytecode = test_bytecode(Vec::new());
    bytecode.abi_version = BYTECODE_ABI_VERSION + 1;

    let error = ProgramGeneration::from_bundle(GenerationId(1), &test_bundle(bytecode, b"asset"))
        .expect_err("unsupported ABI should reject generation");

    assert!(matches!(
        error,
        GenerationBuildError::VerifyBytecode(BytecodeVerificationError::UnsupportedAbi { .. })
    ));
}

fn test_bundle(bytecode: BytecodeProgram, asset_bytes: &[u8]) -> ArcweftBundle {
    let stats = bytecode.stats();
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: Some(BundleLaunchKind::Cli),
            entry: Some("entry.main".to_owned()),
            adapter: Some("test".to_owned()),
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: bytecode
                    .entries
                    .first()
                    .and_then(|entry| match &entry.target {
                        RuntimeEntryTarget::Flow(flow) => Some(flow.public_label().into_string()),
                        RuntimeEntryTarget::Routes(_) | RuntimeEntryTarget::Controller(_) => None,
                    }),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        source_map("test.arcw", "flow main { return \"ok\" }"),
        bytecode,
        LineDisplayCatalog::new(test_dialogue_revision()),
    )
    .expect("test bundle source map accepts the generated standard Style source")
    .with_virtual_files([BundleVirtualFile {
        space: BundleVirtualFileSpace::Asset,
        path: "asset.bin".to_owned(),
        bytes: asset_bytes.to_vec(),
    }])
}

fn source_map(label: &str, text: &str) -> SourceMapSection {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new(label).expect("source ID"),
        SourceName::path(label),
        text,
    )
    .expect("source document");
    SourceMapSection::try_from_documents(&[&document]).expect("source map")
}

fn test_awbc_program(revision: &str) -> AwbcProgram {
    let trap_code = if revision == "revision-a" {
        AwbcTrapCode::ExplicitPanic
    } else {
        AwbcTrapCode::InternalInvariant
    };
    AwbcProgram {
        strings: vec!["entry.main".to_owned(), revision.to_owned()],
        signatures: vec![AwbcSignature {
            params: Vec::new(),
            result: None,
            effects: AwbcEffectSetId(0),
        }],
        frame_layouts: vec![AwbcFrameLayout {
            slots: Vec::new(),
            max_scope_depth: 0,
        }],
        functions: vec![AwbcFunction {
            public_id: Some(AwbcStringId(0)),
            kind: AwbcFunctionKind::Flow,
            signature: AwbcSignatureId(0),
            frame_layout: AwbcFrameLayoutId(0),
            blocks: AwbcTableRange::new(0, 1),
            entry_block: AwbcBlockId(0),
            flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
        }],
        blocks: vec![AwbcBlock {
            owner: AwbcFunctionId(0),
            instructions: AwbcTableRange::new(0, 0),
            terminator: AwbcTerminator::Trap {
                code: trap_code,
                message: None,
            },
            safe_point: AwbcSafePointKind::FlowEntry,
            source_map: None,
        }],
        entries: vec![AwbcEntry {
            runtime_id: entry_id("entry.main"),
            binding: EntryBindingIdentity::from_bytes([1; 32]),
            public_id: AwbcStringId(0),
            kind: AwbcEntryKind::Cli,
            signature: AwbcSignatureId(0),
            target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
            roles: RuntimeEntryRoles::None,
        }],
        ..AwbcProgram::default()
    }
}

fn test_bytecode(instructions: Vec<BytecodeInstruction>) -> BytecodeProgram {
    BytecodeProgram {
        abi_version: BYTECODE_ABI_VERSION,
        runtime_layout: arcweft_core::bytecode::BytecodeRuntimeLayout::current(),
        entries: vec![BytecodeEntry {
            id: entry_id("entry.main"),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([1; 32]),
            target: RuntimeEntryTarget::Flow(flow_id("flow.main")),
            roles: RuntimeEntryRoles::None,
        }],
        callable_executables: Vec::new(),
        flow_executables: Vec::new(),
        flows: vec![BytecodeFlow {
            id: flow_id("flow.main"),
            instructions,
        }],
        pure_helpers: Vec::new(),
        line_task_groups: Vec::new(),
        stream_plans: Vec::new(),
        source_plans: Vec::new(),
    }
}

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
}

fn entry_id(value: &str) -> EntryRuntimeId {
    EntryRuntimeId::from_source_entity_body(value).expect("test entry ID is valid")
}
