use std::{path::PathBuf, sync::Arc};

use arcweft_agent_protocol::{
    ids::{SessionId, StableHash},
    protocol::{AgentProjectGraph, AgentSessionInfo},
};
use arcweft_agent_runner::{
    config::{AgentControllerRunConfig, AgentRunnerConfig},
    error::AgentRunError,
    policy::RuntimeAgentPolicy,
    runner::AgentRunner,
    session::{NoopRagService, ReplayAgentSession},
};
use arcweft_bundle::ArcweftBundle;
use arcweft_core::{
    entry::{EntryBindingIdentity, RootExecutionLimits, RuntimeCommandPolicy, RuntimeEntryRoles},
    plan::{FlowOp, FlowRuntimeId, RuntimeEntryTarget},
    value::{RuntimeExpr, RuntimeUInt, RuntimeValue},
};
use arcweft_debug_model::sink::NullDebugEventSink;
use arcweft_id::PublicId;
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    project_index::{
        ProgramHash, ProjectSemanticIndex, project_semantic_index_from_checked_project,
    },
    registration::ProjectRegistrationFacts,
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_project::{
    manifest::ProjectManifest,
    sources::{ProjectSourceFile, ProjectSources},
};
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use super::{
    CompiledProject, ProjectCompilationContext, ProjectCompileError, ProjectCompileStage,
    ProjectEntrySelection, ProjectEntrySelectionKind, compile_project,
};
use crate::{agent::compile_agent_project_bundle, error::CompileAgentError};

const ENTRY_SOURCE: &str = r"
struct GameState {
    score: i32
}

enum GameEvent {
    Start
}

fn initial_game_state() -> GameState
effects {}
{
    initial_game_state()
}

fn reduce_game(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
effects {}
{
    reduce_game(state, event)
}

flow @flow.opening opening(state: GameState) {
}

entry game @entry.game.main {
    state = GameState
    initializer = initial_game_state
    event = GameEvent
    reducer = reduce_game
    goto @flow.opening
}
";

fn entry_project(
    source_text: &str,
    selected_id: &str,
    selected_kind: ProjectEntrySelectionKind,
) -> (ProjectSources, ProjectCompilationContext) {
    let source_path = PathBuf::from("src/main.arcw");
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://compiler-entry/src/main.arcw")
                .expect("document id"),
            SourceName::path(source_path.display().to_string()),
            source_text,
        )
        .expect("source document"),
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        ProjectManifest::parse_toml("[package]\nname = \"compiler-entry\"\n").expect("manifest"),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            source_path,
            Arc::clone(&document),
            [],
        )],
    )
    .expect("project");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new("compiler-entry").expect("package"),
        document.identity().id().clone(),
        "entry-test",
    )
    .expect("world");
    let facts = ProjectRegistrationFacts::try_new(world, vec![document], Vec::new(), Vec::new())
        .expect("facts");
    let selection = ProjectEntrySelection::new(
        PublicId::try_new(selected_id).expect("entry ID"),
        selected_kind,
    );
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        None,
        Some(selection),
        Vec::new(),
    );
    (project, context)
}

fn compile_entry_project(source_text: &str) -> Result<CompiledProject, ProjectCompileError> {
    let (project, context) = entry_project(
        source_text,
        "entry.game.main",
        ProjectEntrySelectionKind::Game,
    );
    compile_project(&project, &context, &entry_runtime_options())
}

fn entry_runtime_options() -> RuntimePlanLowerOptions {
    RuntimePlanLowerOptions::default().with_command_policy(RuntimeCommandPolicy::deny_all(
        RootExecutionLimits::engine_default(),
    ))
}

#[test]
fn sel_005_checks_selected_entry_identity_and_kind_before_runtime_lowering() {
    let compiled = compile_entry_project(ENTRY_SOURCE).expect("matching selection compiles");
    assert_eq!(compiled.checked_entries().len(), 1);
    let entry = &compiled.runtime_plan().plan.entries[0];
    let RuntimeEntryRoles::Stateful(roles) = &entry.roles else {
        panic!("checked game entry must project exact stateful roles");
    };
    let expected_binding = EntryBindingIdentity::from_bytes(
        *compiled
            .checked_entries()
            .entries()
            .next()
            .expect("checked entry exists")
            .binding_digest()
            .as_bytes(),
    );
    assert_eq!(entry.binding, expected_binding);
    assert_eq!(entry.binding, roles.binding);
    assert_eq!(compiled.runtime_plan().plan.callable_executables.len(), 2);
    assert_eq!(compiled.runtime_plan().plan.flow_executables.len(), 1);
    assert_eq!(
        compiled.runtime_plan().plan.flow_executables[0].parameters[0].name,
        "state"
    );
    assert_eq!(
        entry.target,
        RuntimeEntryTarget::Flow(roles.initial_flow.flow.clone())
    );

    let (missing_project, missing_context) = entry_project(
        ENTRY_SOURCE,
        "entry.game.missing",
        ProjectEntrySelectionKind::Game,
    );
    let missing = compile_project(&missing_project, &missing_context, &entry_runtime_options())
        .expect_err("missing selected entry is rejected");
    assert_eq!(
        missing.stage(),
        ProjectCompileStage::EntrySelection.as_str()
    );
    assert!(missing.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .diagnostic()
            .code()
            .is_some_and(|code| code.as_str() == "compiler.entry_selection.missing")
    }));

    let (mismatch_project, mismatch_context) = entry_project(
        ENTRY_SOURCE,
        "entry.game.main",
        ProjectEntrySelectionKind::Agent,
    );
    let mismatch = compile_project(
        &mismatch_project,
        &mismatch_context,
        &entry_runtime_options(),
    )
    .expect_err("entry kind mismatch is rejected");
    assert_eq!(
        mismatch.stage(),
        ProjectCompileStage::EntrySelection.as_str()
    );
    assert!(mismatch.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .diagnostic()
            .code()
            .is_some_and(|code| code.as_str() == "compiler.entry_selection.kind_mismatch")
    }));
}

#[test]
fn checked_non_stateful_entry_keeps_its_exact_common_binding() {
    let source = r#"
flow @flow.main main {
    return "done"
}

entry cli @entry.cli.main {
    goto @flow.main
}
"#;
    let (project, context) =
        entry_project(source, "entry.cli.main", ProjectEntrySelectionKind::Cli);
    let compiled =
        compile_project(&project, &context, &entry_runtime_options()).expect("CLI entry compiles");
    let entry = &compiled.runtime_plan().plan.entries[0];
    let checked = compiled
        .checked_entries()
        .entries()
        .next()
        .expect("checked entry exists");

    assert!(matches!(entry.roles, RuntimeEntryRoles::None));
    assert_eq!(
        entry.binding,
        EntryBindingIdentity::from_bytes(*checked.binding_digest().as_bytes())
    );
}

#[test]
fn stateful_project_lowering_requires_explicit_adapter_command_policy() {
    let (project, context) = entry_project(
        ENTRY_SOURCE,
        "entry.game.main",
        ProjectEntrySelectionKind::Game,
    );
    let error = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect_err("an absent adapter policy must not become an implicit deny-all policy");

    assert_eq!(
        error.stage(),
        ProjectCompileStage::RuntimePlanLower.as_str()
    );
    assert!(error.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .diagnostic()
            .message()
            .contains("requires an explicit selected-adapter command constructor policy")
    }));
}

#[test]
fn generated_agent_controller_namespace_cannot_alias_an_authored_flow() {
    let source = r"
fn smoke() -> Result<Unit, AgentError>
effects { agent.observe }
{
    Ok(())
}

flow @flow.agent.smoke smoke {
}

entry agent @entry.agent.smoke {
    controller = smoke
}
";
    let (project, context) = entry_project(
        source,
        "entry.agent.smoke",
        ProjectEntrySelectionKind::Agent,
    );
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect("checked ordinary-function Agent controller compiles");
    let entry = &compiled.runtime_plan().plan.entries[0];
    let RuntimeEntryTarget::Controller(generated) = &entry.target else {
        panic!("Agent entry must target its generated controller flow");
    };
    let authored = FlowRuntimeId::from_source_entity_body("flow.agent.smoke").unwrap();

    assert_ne!(generated, &authored);
    assert!(
        FlowRuntimeId::from_source_entity_body("flow.__agent_controller.agent.smoke").is_err(),
        "the generated owner segment must be unavailable to authored flow IDs"
    );
    assert!(
        compiled
            .runtime_plan()
            .plan
            .flows
            .iter()
            .any(|flow| { flow.id == authored })
    );
    assert!(
        compiled
            .runtime_plan()
            .plan
            .flows
            .iter()
            .any(|flow| { flow.id == *generated })
    );
}

#[test]
fn body_only_change_preserves_binding_and_changes_compile_artifact_identity() {
    let changed = ENTRY_SOURCE.replace(
        "{\n    reduce_game(state, event)\n}\n\nflow",
        "{\n    let marker = 1\n    reduce_game(state, event)\n}\n\nflow",
    );
    let baseline = compile_entry_project(ENTRY_SOURCE).expect("baseline compiles");
    let changed = compile_entry_project(&changed).expect("body-only variant compiles");
    let baseline_entry = baseline.checked_entries().entries().next().unwrap();
    let changed_entry = changed.checked_entries().entries().next().unwrap();

    assert_eq!(
        baseline_entry.binding_digest(),
        changed_entry.binding_digest()
    );
    assert_ne!(
        baseline.compile_units()[0].fingerprint(),
        changed.compile_units()[0].fingerprint()
    );
}

fn checked_project_index(compiled: &CompiledProject) -> ProjectSemanticIndex {
    project_semantic_index_from_checked_project(
        compiled.hir_project(),
        ProgramHash::new("program-agent-entry-test"),
        compiled.checked_entries(),
    )
    .expect("checked project index builds")
}

#[test]
fn selected_agent_entry_lowers_only_its_exact_ordinary_controller() {
    let source = r"
fn first() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

fn second() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

entry agent @entry.agent.first {
    controller = first
}

entry agent @entry.agent.second {
    controller = second
}
";
    let (project, context) = entry_project(
        source,
        "entry.agent.second",
        ProjectEntrySelectionKind::Agent,
    );
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect("ordinary Agent controllers compile");
    let index = checked_project_index(&compiled);
    let selected = PublicId::try_new("entry.agent.second").unwrap();
    let artifact = compile_agent_project_bundle(&compiled, &selected, &index)
        .expect("selected Agent entry compiles");

    assert_eq!(artifact.manifest.entry_id.as_str(), "entry.agent.second");
    assert_eq!(
        artifact.manifest.controller_id.as_str(),
        "compiler-entry::second"
    );
    assert_eq!(artifact.bundle.bytecode.program.entries.len(), 1);
    assert_eq!(artifact.bundle.bytecode.program.flows.len(), 1);
    assert_eq!(
        artifact.bundle.bytecode.program.entries[0]
            .id
            .public_label()
            .into_string(),
        "entry.agent.second"
    );
}

#[test]
fn shared_controller_keeps_one_callable_identity_and_distinct_entry_artifacts() {
    let source = r"
#[budget(steps = 96usize)]
fn shared() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

entry agent @entry.agent.alpha {
    controller = shared
}

entry agent @entry.agent.beta {
    controller = shared
}
";
    let (project, context) = entry_project(
        source,
        "entry.agent.alpha",
        ProjectEntrySelectionKind::Agent,
    );
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect("shared ordinary Agent controller compiles");
    let index = checked_project_index(&compiled);
    let alpha = compile_agent_project_bundle(
        &compiled,
        &PublicId::try_new("entry.agent.alpha").unwrap(),
        &index,
    )
    .expect("alpha entry compiles");
    let beta = compile_agent_project_bundle(
        &compiled,
        &PublicId::try_new("entry.agent.beta").unwrap(),
        &index,
    )
    .expect("beta entry compiles");

    assert_eq!(alpha.manifest.controller_id, beta.manifest.controller_id);
    assert_ne!(alpha.manifest.entry_id, beta.manifest.entry_id);
    assert_ne!(
        alpha.manifest.entry_binding_hash,
        beta.manifest.entry_binding_hash
    );
    assert_eq!(alpha.manifest.budget.max_vm_steps, 96);
    assert_eq!(beta.manifest.budget.max_vm_steps, 96);
}

#[test]
fn agent_artifact_requires_an_exact_agent_entry_and_matching_project_index() {
    let source = r"
fn smoke() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

flow @flow.cli cli {
}

entry agent @entry.agent.smoke {
    controller = smoke
}

entry cli @entry.cli.main {
    goto @flow.cli
}
";
    let (project, context) = entry_project(
        source,
        "entry.agent.smoke",
        ProjectEntrySelectionKind::Agent,
    );
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect("mixed entry project compiles");
    let index = checked_project_index(&compiled);

    assert!(matches!(
        compile_agent_project_bundle(
            &compiled,
            &PublicId::try_new("entry.agent.missing").unwrap(),
            &index,
        ),
        Err(CompileAgentError::MissingSelectedEntry { .. })
    ));
    assert!(matches!(
        compile_agent_project_bundle(
            &compiled,
            &PublicId::try_new("entry.cli.main").unwrap(),
            &index,
        ),
        Err(CompileAgentError::SelectedEntryNotAgent { .. })
    ));
    assert!(matches!(
        compile_agent_project_bundle(
            &compiled,
            &PublicId::try_new("entry.agent.smoke").unwrap(),
            &ProjectSemanticIndex::new(ProgramHash::new("other-project")),
        ),
        Err(CompileAgentError::ProjectIndexEntryMismatch { .. })
    ));
}

#[test]
fn agent_controller_uses_callable_local_type_evidence_after_unbound_function() {
    let source = r"
fn unrelated() -> i32
effects {}
{
    1
}

fn controller() -> Result<Unit, AgentError>
effects {}
{
    let count: u32 = 1
    Ok(())
}

entry agent @entry.agent.controller {
    controller = controller
}
";
    let (project, context) = entry_project(
        source,
        "entry.agent.controller",
        ProjectEntrySelectionKind::Agent,
    );
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect("controller after unrelated ordinary function compiles");
    let entry = compiled
        .runtime_plan()
        .plan
        .entries
        .iter()
        .find(|entry| entry.id.public_label().as_str() == "entry.agent.controller")
        .expect("checked Agent entry exists");
    let RuntimeEntryTarget::Controller(controller) = &entry.target else {
        panic!("Agent entry must target a controller flow");
    };
    let flow = compiled
        .runtime_plan()
        .plan
        .flows
        .iter()
        .find(|flow| &flow.id == controller)
        .expect("controller flow exists");

    assert!(matches!(
        flow.ops.first(),
        Some(FlowOp::Let {
            expr: RuntimeExpr::Value(RuntimeValue::UInt(RuntimeUInt::U32(1))),
            ..
        })
    ));
}

#[test]
fn unbound_agent_effect_function_is_not_discovered_as_a_controller() {
    let source = r"
fn stray() -> Result<Unit, AgentError>
effects { agent.observe }
{
    observe()
    Ok(())
}

fn controller() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

entry agent @entry.agent.controller {
    controller = controller
}
";
    let (project, context) = entry_project(
        source,
        "entry.agent.controller",
        ProjectEntrySelectionKind::Agent,
    );
    let error = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect_err("Agent operations outside a bound controller require ordinary policy");

    assert!(
        matches!(
            error
                .diagnostics()
                .first()
                .map(super::ProjectCompileDiagnostic::stage),
            Some(ProjectCompileStage::Resolve | ProjectCompileStage::TypeCheck)
        ),
        "the unbound function is rejected by ordinary name/effect policy, not selected by body"
    );
}

#[test]
fn each_agent_controller_restarts_evidence_at_its_exact_callable_body() {
    let source = r"
fn first() -> Result<Unit, AgentError>
effects {}
{
    let count: u16 = 1
    Ok(())
}

fn unrelated() -> i64
effects {}
{
    1
}

fn second() -> Result<Unit, AgentError>
effects {}
{
    let count: u64 = 1
    Ok(())
}

entry agent @entry.agent.first {
    controller = first
}

entry agent @entry.agent.second {
    controller = second
}
";
    let (project, context) = entry_project(
        source,
        "entry.agent.second",
        ProjectEntrySelectionKind::Agent,
    );
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect("multiple exact ordinary Agent controllers compile");

    for (entry_id, expected) in [
        ("entry.agent.first", RuntimeUInt::U16(1)),
        ("entry.agent.second", RuntimeUInt::U64(1)),
    ] {
        let entry = compiled
            .runtime_plan()
            .plan
            .entries
            .iter()
            .find(|entry| entry.id.public_label().as_str() == entry_id)
            .expect("checked Agent entry exists");
        let RuntimeEntryTarget::Controller(controller) = &entry.target else {
            panic!("Agent entry must target a controller flow");
        };
        let flow = compiled
            .runtime_plan()
            .plan
            .flows
            .iter()
            .find(|flow| &flow.id == controller)
            .expect("controller flow exists");
        assert!(matches!(
            flow.ops.first(),
            Some(FlowOp::Let {
                expr: RuntimeExpr::Value(RuntimeValue::UInt(actual)),
                ..
            }) if actual == &expected
        ));
    }
}

#[test]
fn ordinary_agent_entry_round_trips_and_runs_with_exact_artifact_binding() {
    let source = r"
fn controller() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

entry agent @entry.agent.controller {
    controller = controller
}
";
    let (project, context) = entry_project(
        source,
        "entry.agent.controller",
        ProjectEntrySelectionKind::Agent,
    );
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .expect("ordinary Agent entry project compiles");
    let index = checked_project_index(&compiled);
    let artifact = compile_agent_project_bundle(
        &compiled,
        &PublicId::try_new("entry.agent.controller").unwrap(),
        &index,
    )
    .expect("entry-bound Agent artifact compiles");
    let bytes = artifact.bundle.to_json_bytes().expect("bundle encodes");
    let decoded = ArcweftBundle::from_json_slice(&bytes).expect("bundle decodes");
    let manifest = decoded.agent.as_ref().expect("final Agent manifest exists");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.entry_id.as_str(), "entry.agent.controller");
    assert_eq!(
        manifest.controller_id.as_str(),
        "compiler-entry::controller"
    );

    let session = ReplayAgentSession::new(
        AgentSessionInfo {
            session_id: "session.agent-entry-e2e".to_owned(),
            program_hash: manifest.project_binding.program_hash.as_str().to_owned(),
            project_entities: manifest.project_binding.required_entities.clone(),
            project_graph: AgentProjectGraph::default(),
            profile: None,
            capabilities: Vec::new(),
        },
        Vec::new(),
    );
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::default(),
        AgentRunnerConfig::new(
            SessionId::new("session.agent-entry-e2e").expect("valid session ID"),
        ),
    );
    let report = runner
        .run_controller_bundle(&decoded, AgentControllerRunConfig::default())
        .expect("decoded exact Agent artifact runs");
    assert!(report.final_status.is_some());

    let mut tampered = decoded;
    tampered.agent.as_mut().unwrap().entry_binding_hash = StableHash::from_blake3_bytes([9; 32]);
    let tampered_manifest = tampered.agent.as_ref().unwrap();
    let session = ReplayAgentSession::new(
        AgentSessionInfo {
            session_id: "session.agent-entry-e2e".to_owned(),
            program_hash: tampered_manifest
                .project_binding
                .program_hash
                .as_str()
                .to_owned(),
            project_entities: tampered_manifest.project_binding.required_entities.clone(),
            project_graph: AgentProjectGraph::default(),
            profile: None,
            capabilities: Vec::new(),
        },
        Vec::new(),
    );
    let mut runner = AgentRunner::new(
        session,
        NullDebugEventSink,
        NoopRagService,
        RuntimeAgentPolicy::default(),
        AgentRunnerConfig::new(
            SessionId::new("session.agent-entry-e2e").expect("valid session ID"),
        ),
    );
    assert!(matches!(
        runner.run_controller_bundle(&tampered, AgentControllerRunConfig::default()),
        Err(AgentRunError::AgentArtifactMismatch { .. })
    ));
}
