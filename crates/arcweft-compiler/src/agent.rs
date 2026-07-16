use arcweft_agent_protocol::artifact::{
    AgentArtifactManifest, AgentBundleKind, ProjectBinding, ProjectBindingMode,
};
use arcweft_agent_protocol::ids::{
    CallableId as ArtifactCallableId, PublicId as ArtifactPublicId, StableHash,
};
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{ArcweftBundle, BundleManifest, BundleRuntimeSummary};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::entry::{AgentBudget, RuntimeCallableExecutableCode};
use arcweft_core::plan::{
    EntryRuntimeId, FlowRuntimeId, RuntimeEntrySpec, RuntimeEntryTarget, RuntimePlan,
};
use arcweft_id::PublicId;
use arcweft_lang_hir::model::HirFunction;
use arcweft_lang_hir::symbol::CallableDeclarationId;
use arcweft_lang_sema::effect_model::CallableId as EffectCallableId;
use arcweft_lang_sema::entry::{CheckedAgentEntry, CheckedEntryKind};
use arcweft_lang_sema::project_index::{
    ProjectEntryRoleKind, ProjectEntryRoleTarget, ProjectSemanticIndex,
};
use arcweft_runtime_plan::flow::RuntimePlanLowerStats;

use crate::agent_project::agent_required_entities_from_project;
use crate::effect_manifest;
use crate::error::CompileAgentError;
use crate::project::{CompiledProject, EntryRuntimeProjection};
use crate::types::CompiledAgentBundle;

/// Builds one explicitly selected checked Agent entry as an entry-bound
/// controller artifact.
///
/// Project compilation has already resolved and checked every source entry.
/// This boundary accepts one canonical entry ID and lowers only its exact
/// ordinary controller declaration into the artifact program.
pub fn compile_agent_project_bundle(
    compiled: &CompiledProject,
    selected_entry: &PublicId,
    project: &ProjectSemanticIndex,
) -> Result<CompiledAgentBundle, CompileAgentError> {
    let checked = compiled
        .checked_entries()
        .get_public(selected_entry)
        .ok_or_else(|| CompileAgentError::MissingSelectedEntry {
            entry: selected_entry.as_str().to_owned(),
        })?
        .agent()
        .ok_or_else(|| CompileAgentError::SelectedEntryNotAgent {
            entry: selected_entry.as_str().to_owned(),
        })?;
    let controller = exact_controller_function(compiled, checked)?;
    compile_checked_agent_bundle(compiled, checked, controller, project)
}

/// Core Agent artifact boundary after exact entry and controller selection.
///
/// Callers must pass the accepted semantic entry and the exact ordinary HIR
/// function named by its controller role. The boundary validates both against
/// the compiled project before projecting the selected runtime artifact.
pub fn compile_checked_agent_bundle(
    compiled: &CompiledProject,
    checked: &CheckedAgentEntry,
    controller: &HirFunction,
    project: &ProjectSemanticIndex,
) -> Result<CompiledAgentBundle, CompileAgentError> {
    validate_checked_agent_inputs(compiled, checked, controller)?;
    validate_project_index_entry(project, checked)?;
    let runtime_id = EntryRuntimeId::from_source_entity_body(checked.id().public_id().as_str())
        .map_err(|_| CompileAgentError::MissingRuntimeEntry {
            entry: checked.id().to_string(),
        })?;
    let runtime_entry = compiled
        .runtime_plan()
        .plan
        .entries
        .iter()
        .find(|entry| entry.id == runtime_id)
        .cloned()
        .ok_or_else(|| CompileAgentError::MissingRuntimeEntry {
            entry: checked.id().to_string(),
        })?;
    let controller_flow = match &runtime_entry.target {
        RuntimeEntryTarget::Controller(flow) => flow.clone(),
        _ => {
            return Err(CompileAgentError::InvalidRuntimeEntry {
                entry: checked.id().to_string(),
            });
        }
    };
    let Some(runtime_roles) = runtime_entry.roles.agent() else {
        return Err(CompileAgentError::InvalidRuntimeEntry {
            entry: checked.id().to_string(),
        });
    };
    if runtime_roles.controller.callable.as_str() != checked.controller().declaration().to_string()
        || runtime_roles.binding.as_bytes() != checked.binding_digest().as_bytes()
        || runtime_roles.controller.contract.as_bytes()
            != checked.controller().contract_digest().as_bytes()
        || runtime_roles.policy.as_bytes() != checked.policy_digest().as_bytes()
        || runtime_roles.budget != EntryRuntimeProjection::agent_budget(checked.budget())
    {
        return Err(CompileAgentError::InvalidRuntimeEntry {
            entry: checked.id().to_string(),
        });
    }

    let runtime_budget = runtime_roles.budget;
    let selected_plan =
        selected_agent_runtime_plan(compiled, runtime_entry, &controller_flow, checked)?;
    selected_plan
        .verify()
        .map_err(|error| CompileAgentError::RuntimePlanVerification(error.to_string()))?;
    let pure_helpers = selected_plan.pure_helpers.len();
    let bytecode = BytecodeProgram::from_runtime_plan(selected_plan);
    let bytecode_stats = bytecode.stats();
    let manifest = agent_artifact_manifest(compiled, checked, project, runtime_budget)?;
    let documents = compiled
        .modules()
        .iter()
        .map(|module| {
            module
                .hir()
                .source_document()
                .ok_or_else(|| CompileAgentError::MissingSourceDocument {
                    module: module.module().to_string(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source_map = SourceMapSection::try_from_documents(&documents)?;
    let bundle = ArcweftBundle::new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some(manifest.entry_id.as_str().to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: Some(controller_flow.public_label().into_string()),
                flows: bytecode_stats.flows,
                bytecode_instructions: bytecode_stats.instructions,
                line_task_groups: bytecode_stats.line_task_groups,
                stream_plans: bytecode_stats.stream_plans,
                source_plans: bytecode_stats.source_plans,
            },
        },
        source_map,
        bytecode,
        arcweft_render_text::LineDisplayCatalog::default(),
    )
    .with_agent_manifest(manifest.clone());
    Ok(CompiledAgentBundle {
        bundle,
        manifest,
        hir: compiled.linked_hir().clone(),
        typecheck_report: compiled.typecheck_report().clone(),
        runtime_plan_stats: RuntimePlanLowerStats {
            pure_helpers,
            ..RuntimePlanLowerStats::default()
        },
    })
}

fn validate_checked_agent_inputs(
    compiled: &CompiledProject,
    checked: &CheckedAgentEntry,
    controller: &HirFunction,
) -> Result<(), CompileAgentError> {
    let Some(accepted) = compiled
        .checked_entries()
        .get_public(checked.id().public_id())
        .and_then(arcweft_lang_sema::entry::CheckedEntryBinding::agent)
    else {
        return Err(CompileAgentError::MissingSelectedEntry {
            entry: checked.id().to_string(),
        });
    };
    if accepted != checked {
        return Err(CompileAgentError::InvalidRuntimeEntry {
            entry: checked.id().to_string(),
        });
    }
    let declaration = CallableDeclarationId::for_function(
        checked.controller().declaration().package(),
        controller,
    )
    .map_err(|_| CompileAgentError::ControllerDeclarationCardinality {
        controller: checked.controller().declaration().to_string(),
        matches: 0,
    })?;
    if declaration != *checked.controller().declaration() {
        return Err(CompileAgentError::ControllerDeclarationCardinality {
            controller: checked.controller().declaration().to_string(),
            matches: 0,
        });
    }
    let accepted = exact_controller_function(compiled, checked)?;
    if accepted != controller {
        return Err(CompileAgentError::ControllerFunctionMismatch {
            controller: checked.controller().declaration().to_string(),
        });
    }
    Ok(())
}

fn selected_agent_runtime_plan(
    compiled: &CompiledProject,
    entry: RuntimeEntrySpec,
    controller_flow: &FlowRuntimeId,
    checked: &CheckedAgentEntry,
) -> Result<RuntimePlan, CompileAgentError> {
    let full = &compiled.runtime_plan().plan;
    let mut flows = full.flows.iter().filter(|flow| &flow.id == controller_flow);
    let selected_flow = flows.next().cloned();
    if selected_flow.is_none() || flows.next().is_some() {
        return Err(CompileAgentError::InvalidRuntimeEntry {
            entry: checked.id().to_string(),
        });
    }
    let runtime_roles =
        entry
            .roles
            .agent()
            .ok_or_else(|| CompileAgentError::InvalidRuntimeEntry {
                entry: checked.id().to_string(),
            })?;
    let mut callables = full.callable_executables.iter().filter(|executable| {
        executable.callable == runtime_roles.controller.callable
            && executable.contract == runtime_roles.controller.contract
            && matches!(
                &executable.code,
                RuntimeCallableExecutableCode::ControllerFlow(flow) if flow == controller_flow
            )
    });
    let callable = callables.next().cloned();
    if callable.is_none() || callables.next().is_some() {
        return Err(CompileAgentError::InvalidRuntimeEntry {
            entry: checked.id().to_string(),
        });
    }
    let mut flow_executables = full
        .flow_executables
        .iter()
        .filter(|executable| &executable.flow == controller_flow);
    let flow_executable = flow_executables.next().cloned();
    if flow_executable.is_none() || flow_executables.next().is_some() {
        return Err(CompileAgentError::InvalidRuntimeEntry {
            entry: checked.id().to_string(),
        });
    }
    let plan = RuntimePlan::new(vec![selected_flow.expect("checked above")], Vec::new())
        .map_err(|error| CompileAgentError::RuntimePlanVerification(error.to_string()))?
        .with_entries(vec![entry])
        .with_entry_executables(
            vec![callable.expect("checked above")],
            vec![flow_executable.expect("checked above")],
        )
        .with_pure_helpers(full.pure_helpers.clone())
        .with_trait_methods(full.trait_methods.clone());
    Ok(plan)
}

fn validate_project_index_entry(
    project: &ProjectSemanticIndex,
    checked: &CheckedAgentEntry,
) -> Result<(), CompileAgentError> {
    let valid_record = project.entry_record(checked.id()).is_some_and(|record| {
        record.kind() == &CheckedEntryKind::Agent
            && record.binding_digest() == checked.binding_digest()
            && record.agent_policy_digest() == Some(checked.policy_digest())
    });
    let mut controller_edges = project
        .entry_role_edges_for(checked.id())
        .filter(|edge| edge.role() == ProjectEntryRoleKind::Controller);
    let valid_controller = controller_edges.next().is_some_and(|edge| {
        matches!(
            edge.target(),
            ProjectEntryRoleTarget::Callable {
                declaration,
                contract_digest,
            } if declaration == checked.controller().declaration()
                && contract_digest == checked.controller().contract_digest()
        )
    }) && controller_edges.next().is_none();
    if valid_record && valid_controller {
        Ok(())
    } else {
        Err(CompileAgentError::ProjectIndexEntryMismatch {
            entry: checked.id().to_string(),
        })
    }
}

fn exact_controller_function<'a>(
    compiled: &'a CompiledProject,
    checked: &CheckedAgentEntry,
) -> Result<&'a HirFunction, CompileAgentError> {
    let declaration = checked.controller().declaration();
    if compiled.hir_project().package() != declaration.package() {
        return Err(CompileAgentError::ControllerDeclarationCardinality {
            controller: declaration.to_string(),
            matches: 0,
        });
    }
    let Some(module) = compiled.hir_project().module(declaration.module()) else {
        return Err(CompileAgentError::ControllerDeclarationCardinality {
            controller: declaration.to_string(),
            matches: 0,
        });
    };
    let mut matches = module.functions().iter().filter(|function| {
        CallableDeclarationId::for_function(declaration.package(), function)
            .is_ok_and(|candidate| candidate == *declaration)
    });
    match (matches.next(), matches.next()) {
        (Some(controller), None) => Ok(controller),
        (first, second) => Err(CompileAgentError::ControllerDeclarationCardinality {
            controller: declaration.to_string(),
            matches: usize::from(first.is_some()) + usize::from(second.is_some()),
        }),
    }
}

fn agent_artifact_manifest(
    compiled: &CompiledProject,
    checked: &CheckedAgentEntry,
    project: &ProjectSemanticIndex,
    budget: AgentBudget,
) -> Result<AgentArtifactManifest, CompileAgentError> {
    let declaration = checked.controller().declaration();
    let effect_id = EffectCallableId::project_function(declaration);
    let closed_effect_rows = compiled
        .typecheck_report()
        .effects
        .closed_effect_rows()
        .map_err(effect_manifest::VerifiedEffectBuildError::from)?;
    let verified_effects =
        effect_manifest::build_verified_effect_summary(&effect_id, &closed_effect_rows)?;
    let declared_effects = verified_effects.inferred.clone();
    Ok(AgentArtifactManifest {
        schema_version: 1,
        bundle_kind: AgentBundleKind::AgentController,
        entry_id: ArtifactPublicId::new(checked.id().public_id().as_str())?,
        controller_id: ArtifactCallableId::new(declaration.to_string())?,
        entry_binding_hash: StableHash::from_blake3_bytes(*checked.binding_digest().as_bytes()),
        controller_contract_hash: StableHash::from_blake3_bytes(
            *checked.controller().contract_digest().as_bytes(),
        ),
        policy_hash: StableHash::from_blake3_bytes(*checked.policy_digest().as_bytes()),
        source_hash: compiled_source_hash(compiled),
        compiler_version: format!("arcweft-compiler/{}", env!("CARGO_PKG_VERSION")),
        project_binding: ProjectBinding {
            program_hash: StableHash::new(project.program_hash().as_str().to_owned())?,
            mode: ProjectBindingMode::Compatible,
            required_entities: agent_required_entities_from_project(project)?,
        },
        declared_effects,
        verified_effects,
        budget,
        debug_map_hash: None,
    })
}

fn compiled_source_hash(compiled: &CompiledProject) -> StableHash {
    let mut modules = compiled.modules().iter().collect::<Vec<_>>();
    modules.sort_by(|left, right| left.module().cmp(right.module()));
    let mut hasher = blake3::Hasher::new_derive_key("arcweft.agent-artifact.source-set.v1");
    for module in modules {
        hash_source_part(&mut hasher, module.module().to_string().as_bytes());
        hash_source_part(&mut hasher, &module.source_hash().as_bytes());
    }
    StableHash::from_blake3_bytes(hasher.finalize().into())
}

fn hash_source_part(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}
