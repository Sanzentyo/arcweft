use arcweft_agent_protocol::artifact::{
    AgentArtifactManifest, AgentBudget, AgentBundleKind, ProjectBinding, ProjectBindingMode,
};
use arcweft_agent_protocol::ids::{PublicId as AgentPublicId, StableHash};
use arcweft_bundle::{ArcweftBundle, BundleManifest, BundleRuntimeSummary, BundleSource};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_lang_hir::model::{HirAgent, HirModule};
use arcweft_lang_sema::check::{TypeCheckReport, analyze_types};
use arcweft_lang_sema::project_index::ProjectSemanticIndex;
use arcweft_lang_sema::resolve::{registry_from_hir_and_project, validate_hir_references};
use arcweft_lang_syntax::ast::items::Attribute;
use arcweft_runtime_plan::flow::{
    RuntimePlanLowerOptions, lower_agent_controller_plan_with_stats_and_options,
};

use crate::agent_project::agent_required_entities_from_project;
use crate::effect_manifest;
use crate::error::CompileAgentError;
use crate::hir::{lower_source_tree, validate_hir_typecheck_ready};
use crate::lower::runtime_plan_options_with_typecheck_evidence;
use crate::parse::parse_agent_source_text;
use crate::types::{CompiledAgent, CompiledAgentBundle, TypecheckedAgent};

/// Compiles `.awfagent` source through the shared parser and HIR lowering path.
pub fn compile_agent_source(source: impl Into<String>) -> Result<CompiledAgent, CompileAgentError> {
    let parsed = parse_agent_source_text(source);
    if !parsed.errors().is_empty() {
        return Err(CompileAgentError::Parse(parsed.errors().to_vec()));
    }
    let hir = lower_source_tree(parsed.typed_tree()).map_err(CompileAgentError::Hir)?;
    if hir.agents().is_empty() {
        return Err(CompileAgentError::MissingAgent);
    }
    Ok(CompiledAgent { hir })
}

/// Compiles `.awfagent` source and checks it against a project semantic index.
pub fn compile_agent_source_with_project(
    source: impl Into<String>,
    project: &ProjectSemanticIndex,
) -> Result<TypecheckedAgent, CompileAgentError> {
    let compiled = compile_agent_source(source)?;
    let registry = registry_from_hir_and_project(&compiled.hir, project);
    validate_hir_references(&compiled.hir, &registry).map_err(CompileAgentError::Resolve)?;
    validate_hir_typecheck_ready(&compiled.hir).map_err(CompileAgentError::Readiness)?;
    let typecheck_report = analyze_types(&compiled.hir, &project.typecheck_env());
    typecheck_report
        .clone()
        .into_result()
        .map_err(CompileAgentError::Type)?;
    Ok(TypecheckedAgent {
        hir: compiled.hir,
        typecheck_report,
    })
}

/// Compiles one `.awfagent` controller into an Agent controller `.awfb` bundle
/// data object using the shared runtime-plan and bytecode shapes.
pub fn compile_agent_bundle_with_project(
    source: impl Into<String>,
    project: &ProjectSemanticIndex,
) -> Result<CompiledAgentBundle, CompileAgentError> {
    let source = source.into();
    let parsed = parse_agent_source_text(source.clone());
    if !parsed.errors().is_empty() {
        return Err(CompileAgentError::Parse(parsed.errors().to_vec()));
    }
    let source_hash = parsed.source_hash();
    let hir = lower_source_tree(parsed.typed_tree()).map_err(CompileAgentError::Hir)?;
    let agent = single_agent(&hir)?;
    let registry = registry_from_hir_and_project(&hir, project);
    validate_hir_references(&hir, &registry).map_err(CompileAgentError::Resolve)?;
    validate_hir_typecheck_ready(&hir).map_err(CompileAgentError::Readiness)?;
    let typecheck_report = analyze_types(&hir, &project.typecheck_env());
    typecheck_report
        .clone()
        .into_result()
        .map_err(CompileAgentError::Type)?;
    let runtime_options = runtime_plan_options_with_typecheck_evidence(
        &RuntimePlanLowerOptions::default(),
        &typecheck_report,
    )
    .map_err(CompileAgentError::RuntimePlan)?;
    let runtime_report =
        lower_agent_controller_plan_with_stats_and_options(&hir, agent, &runtime_options)
            .map_err(CompileAgentError::RuntimePlan)?;
    let bytecode = BytecodeProgram::from_runtime_plan(runtime_report.plan);
    let bytecode_stats = bytecode.stats();
    let manifest = agent_artifact_manifest(agent, source_hash, project, &typecheck_report)?;
    let source_label = format!("{}.awfagent", manifest.agent_id.as_str());
    let bundle = ArcweftBundle::new(
        BundleManifest {
            source_label: source_label.clone(),
            profile_id: None,
            profile_kind: None,
            entry: Some(format!("entry.{}", manifest.agent_id.as_str())),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: bytecode.entry_flow.as_ref().map(|flow| flow.0.clone()),
                flows: bytecode_stats.flows,
                bytecode_instructions: bytecode_stats.instructions,
                line_task_groups: bytecode_stats.line_task_groups,
                stream_plans: bytecode_stats.stream_plans,
                source_plans: bytecode_stats.source_plans,
            },
        },
        BundleSource {
            label: source_label,
            text: source,
        },
        bytecode,
        runtime_report.line_display_catalog,
    )
    .with_agent_manifest(manifest.clone());
    Ok(CompiledAgentBundle {
        bundle,
        manifest,
        hir,
        typecheck_report,
        runtime_plan_stats: runtime_report.stats,
    })
}
fn single_agent(hir: &HirModule) -> Result<&HirAgent, CompileAgentError> {
    match hir.agents() {
        [] => Err(CompileAgentError::MissingAgent),
        [agent] => Ok(agent),
        agents => Err(CompileAgentError::MultipleAgents(agents.len())),
    }
}

fn agent_artifact_manifest(
    agent: &HirAgent,
    source_hash: arcweft_lang_syntax::source::SourceHash,
    project: &ProjectSemanticIndex,
    typecheck_report: &TypeCheckReport,
) -> Result<AgentArtifactManifest, CompileAgentError> {
    let agent_id = agent_public_id(agent)?;
    let agent_effect_id =
        arcweft_lang_sema::effect_model::CallableId::new(format!("agent.{}", agent.item().name()));
    let verified_effects = effect_manifest::build_verified_effect_summary(
        &agent_effect_id,
        &typecheck_report.effects,
    )?;
    let declared_effects = verified_effects.inferred.clone();
    Ok(AgentArtifactManifest {
        schema_version: 1,
        bundle_kind: AgentBundleKind::AgentController,
        agent_id,
        source_hash: StableHash::new(format!("blake3:{}", source_hash.to_hex()))?,
        compiler_version: format!("arcweft-compiler/{}", env!("CARGO_PKG_VERSION")),
        project_binding: ProjectBinding {
            program_hash: StableHash::new(project.program_hash().as_str().to_owned())?,
            mode: ProjectBindingMode::Compatible,
            required_entities: agent_required_entities_from_project(project)?,
        },
        declared_effects,
        verified_effects,
        budget: agent_budget(agent)?,
        debug_map_hash: None,
    })
}

fn agent_budget(agent: &HirAgent) -> Result<AgentBudget, CompileAgentError> {
    agent
        .attributes()
        .iter()
        .filter(|attribute| attribute.name() == "budget")
        .try_fold(AgentBudget::default(), apply_agent_budget_attribute)
}

fn apply_agent_budget_attribute(
    mut budget: AgentBudget,
    attribute: &Attribute,
) -> Result<AgentBudget, CompileAgentError> {
    let args = attribute.args().ok_or_else(|| {
        CompileAgentError::Budget("budget attribute requires key/value arguments".to_owned())
    })?;
    for item in split_agent_budget_args(args) {
        let (key, value) = item.split_once('=').ok_or_else(|| {
            CompileAgentError::Budget(format!("budget item `{item}` must use key = value"))
        })?;
        let key = key.trim();
        let value = value.trim();
        match key {
            "timeout" => budget.logical_timeout_millis = parse_agent_budget_duration(value)?,
            "steps" => budget.max_vm_steps = parse_agent_budget_u64(value)?,
            "host_calls" => {
                budget.max_host_calls = parse_agent_budget_u32(value)?;
            }
            "observations" => {
                budget.max_observations = parse_agent_budget_u32(value)?;
            }
            "captures" => {
                budget.max_captures = parse_agent_budget_u32(value)?;
            }
            "stored_bytes" => {
                budget.max_capture_bytes = parse_agent_budget_u64(value)?;
            }
            "rag_queries" => {
                budget.max_rag_queries = parse_agent_budget_u32(value)?;
            }
            "context_bytes" => {
                budget.max_context_bytes = parse_agent_budget_u64(value)?;
            }
            other => {
                return Err(CompileAgentError::Budget(format!(
                    "unsupported budget key `{other}`"
                )));
            }
        }
    }
    Ok(budget)
}

fn split_agent_budget_args(args: &str) -> impl Iterator<Item = &str> {
    args.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
}

fn parse_agent_budget_duration(value: &str) -> Result<u64, CompileAgentError> {
    let (number, multiplier) = value
        .trim()
        .strip_suffix("ms")
        .map(|number| (number, 1))
        .or_else(|| value.trim().strip_suffix('s').map(|number| (number, 1_000)))
        .ok_or_else(|| {
            CompileAgentError::Budget(format!(
                "budget timeout `{value}` must use an `ms` or `s` suffix"
            ))
        })?;
    parse_agent_budget_u64(number)?
        .checked_mul(multiplier)
        .ok_or_else(|| CompileAgentError::Budget(format!("budget timeout `{value}` overflows")))
}

fn parse_agent_budget_u32(value: &str) -> Result<u32, CompileAgentError> {
    u32::try_from(parse_agent_budget_u64(value)?).map_err(|_| {
        CompileAgentError::Budget(format!("budget value `{value}` does not fit in u32"))
    })
}

fn parse_agent_budget_u64(value: &str) -> Result<u64, CompileAgentError> {
    let trimmed = value.trim();
    let number = ["usize", "u64", "u32"]
        .into_iter()
        .find_map(|suffix| trimmed.strip_suffix(suffix))
        .unwrap_or(trimmed);
    let digits = number.replace('_', "");
    if digits.is_empty() || !digits.chars().all(|char| char.is_ascii_digit()) {
        return Err(CompileAgentError::Budget(format!(
            "budget value `{value}` must be a non-negative integer"
        )));
    }
    digits.parse().map_err(|error| {
        CompileAgentError::Budget(format!("invalid budget value `{value}`: {error}"))
    })
}

fn agent_public_id(agent: &HirAgent) -> Result<AgentPublicId, CompileAgentError> {
    AgentPublicId::new(agent.item().id().map_or_else(
        || format!("agent.{}", agent.item().name()),
        |id| id.body().to_owned(),
    ))
    .map_err(CompileAgentError::ArtifactIdentifier)
}
