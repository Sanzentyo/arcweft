use arcweft_agent_protocol::artifact::AgentArtifactManifest;
use arcweft_bundle::ArcweftBundle;
use arcweft_lang_hir::project::HirProject;
use arcweft_lang_sema::final_analysis::FinalSemanticAnalysis;
use arcweft_presentation::fx::FxDefinition;
use arcweft_runtime_plan::flow::RuntimePlanLowerStats;
use arcweft_text_model::DialogueContentCatalog;
use std::sync::Arc;

/// Source compilation result shared by developer tooling and player hosts.
#[derive(Clone)]
pub struct CompiledSource {
    pub plan: arcweft_core::plan::RuntimePlan,
    pub dialogue_content: DialogueContentCatalog,
    pub hir_project: Arc<HirProject>,
    pub semantic_analysis: Arc<FinalSemanticAnalysis>,
    pub style: crate::style::CompiledViewStyleArtifact,
    pub fx_definitions: Arc<[FxDefinition]>,
    pub runtime_plan_stats: RuntimePlanLowerStats,
}

/// Agent controller bundle compilation result.
#[derive(Clone)]
pub struct CompiledAgentBundle {
    pub bundle: ArcweftBundle,
    pub manifest: AgentArtifactManifest,
    pub execution_diagnostics: Arc<crate::runtime_diagnostics::ExecutionDiagnosticContext>,
    pub hir_project: Arc<HirProject>,
    pub semantic_analysis: Arc<FinalSemanticAnalysis>,
    pub runtime_plan_stats: RuntimePlanLowerStats,
}

impl std::fmt::Debug for CompiledSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledSource")
            .field("plan", &self.plan)
            .field("dialogue_content", &self.dialogue_content)
            .field("hir_database", &self.hir_project.database_id())
            .field("semantic_analysis", &self.semantic_analysis)
            .field("style", &self.style)
            .field("fx_definitions", &self.fx_definitions)
            .field("runtime_plan_stats", &self.runtime_plan_stats)
            .finish()
    }
}

impl std::fmt::Debug for CompiledAgentBundle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledAgentBundle")
            .field("bundle", &self.bundle)
            .field("manifest", &self.manifest)
            .field("runtime_artifact", &self.execution_diagnostics.artifact())
            .field("hir_database", &self.hir_project.database_id())
            .field("semantic_analysis", &self.semantic_analysis)
            .field("runtime_plan_stats", &self.runtime_plan_stats)
            .finish()
    }
}
