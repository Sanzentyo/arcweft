use arcweft_agent_protocol::artifact::AgentArtifactManifest;
use arcweft_bundle::ArcweftBundle;
use arcweft_lang_sema::check::TypeCheckReport;
use arcweft_presentation::fx::FxDefinition;
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::flow::RuntimePlanLowerStats;
use std::sync::Arc;

/// Source compilation result shared by developer tooling and player hosts.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledSource {
    pub plan: arcweft_core::plan::RuntimePlan,
    pub display: LineDisplayCatalog,
    pub hir: arcweft_lang_hir::model::HirModule,
    pub typecheck_report: TypeCheckReport,
    pub style: crate::style::CompiledViewStyleArtifact,
    pub fx_definitions: Arc<[FxDefinition]>,
    pub runtime_plan_stats: RuntimePlanLowerStats,
}

/// Agent controller bundle compilation result.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledAgentBundle {
    pub bundle: ArcweftBundle,
    pub manifest: AgentArtifactManifest,
    pub hir: arcweft_lang_hir::model::HirModule,
    pub typecheck_report: TypeCheckReport,
    pub runtime_plan_stats: RuntimePlanLowerStats,
}
