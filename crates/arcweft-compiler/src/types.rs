use arcweft_agent_protocol::artifact::AgentArtifactManifest;
use arcweft_bundle::ArcweftBundle;
use arcweft_lang_sema::check::TypeCheckReport;
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::flow::RuntimePlanLowerStats;
use arcweft_runtime_plan::pure::{PureHelperCandidate, PureHelperLowerError};
use thiserror::Error;

/// Source compilation result shared by developer tooling and player hosts.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledSource {
    pub plan: arcweft_core::plan::RuntimePlan,
    pub display: LineDisplayCatalog,
    pub hir: arcweft_lang_hir::model::HirModule,
    pub typecheck_report: TypeCheckReport,
    pub runtime_plan_stats: RuntimePlanLowerStats,
}

/// Agent controller compilation result before bytecode/runtime artifact lowering.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledAgent {
    pub hir: arcweft_lang_hir::model::HirModule,
}

/// Agent controller compilation result checked against a project semantic index.
#[derive(Clone, Debug, PartialEq)]
pub struct TypecheckedAgent {
    pub hir: arcweft_lang_hir::model::HirModule,
    pub typecheck_report: TypeCheckReport,
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

/// Text renderer extension family selected from source attributes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextPureHelperKind {
    Shader,
    Effect,
    Motion,
}

/// Pure helper candidates exported for native rich-text renderer registries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextPureHelperCandidateReport {
    pub shaders: Vec<PureHelperCandidate>,
    pub effects: Vec<PureHelperCandidate>,
    pub motions: Vec<PureHelperCandidate>,
}

/// Error while exporting source-local text renderer pure helpers.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TextPureHelperCandidateError {
    #[error("text {kind} function `{name}` must be annotated with #[pure]")]
    MissingPureAttribute {
        kind: TextPureHelperKind,
        name: String,
    },
    #[error("text {kind} pure helper lowering failed: {source}")]
    PureLower {
        kind: TextPureHelperKind,
        #[source]
        source: PureHelperLowerError,
    },
}
