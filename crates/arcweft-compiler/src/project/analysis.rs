//! Immutable semantic publication and the compiler's completed phase leases.

use std::sync::Arc;

use arcweft_lang_hir::{project::HirProject, symbol::ProjectSymbolTable};
use arcweft_lang_sema::{
    entry::CheckedEntryCatalog,
    final_analysis::FinalSemanticAnalysis,
    project_index::{ProgramHash, ProjectSemanticIndex},
    registration::{RegisteredSemanticWorld, RegisteredTypeCheckEnv},
};

use super::{
    AssertionBuildProfile, CompiledProject, CompiledProjectModule, ProjectCompileUnitSummary,
    ProjectToolingLease,
};

/// One complete semantic generation, independent of executable admission.
///
/// The compiler alone joins these products, before checking user diagnostics
/// or entering verification/lowering. The report and index retain the same
/// checked catalog and exact HIR/world generation on both success and failure.
pub struct ProjectAnalysisLease {
    tooling: Arc<ProjectToolingLease>,
    registered_world: Arc<RegisteredSemanticWorld>,
    assertion_build_profile: AssertionBuildProfile,
    final_analysis: Arc<FinalSemanticAnalysis>,
    semantic_index: Arc<ProjectSemanticIndex>,
}

impl ProjectAnalysisLease {
    pub(super) fn new(
        tooling: Arc<ProjectToolingLease>,
        registered_world: Arc<RegisteredSemanticWorld>,
        assertion_build_profile: AssertionBuildProfile,
        final_analysis: Arc<FinalSemanticAnalysis>,
        semantic_index: Arc<ProjectSemanticIndex>,
    ) -> Self {
        Self {
            tooling,
            registered_world,
            assertion_build_profile,
            final_analysis,
            semantic_index,
        }
    }

    pub fn modules(&self) -> &[CompiledProjectModule] {
        self.tooling.modules()
    }

    pub fn compile_units(&self) -> &[ProjectCompileUnitSummary] {
        self.tooling.compile_units()
    }

    pub fn hir_project(&self) -> &Arc<HirProject> {
        self.tooling.hir_project()
    }

    /// Exact HIR/source ancestor used to construct this semantic generation.
    pub const fn tooling_lease(&self) -> &Arc<ProjectToolingLease> {
        &self.tooling
    }

    pub fn project_symbols(&self) -> &ProjectSymbolTable {
        self.tooling.project_symbols()
    }

    pub fn registered_world(&self) -> &RegisteredSemanticWorld {
        &self.registered_world
    }

    pub fn registered_world_arc(&self) -> Arc<RegisteredSemanticWorld> {
        Arc::clone(&self.registered_world)
    }

    pub fn registered_environment(&self) -> &RegisteredTypeCheckEnv {
        self.registered_world.environment()
    }

    pub const fn assertion_build_profile(&self) -> AssertionBuildProfile {
        self.assertion_build_profile
    }

    /// Complete final semantic report, including unselected tooling outcomes.
    pub const fn final_analysis(&self) -> &Arc<FinalSemanticAnalysis> {
        &self.final_analysis
    }

    pub fn checked_entries(&self) -> &CheckedEntryCatalog {
        self.final_analysis.checked_entries()
    }

    pub const fn semantic_index(&self) -> &Arc<ProjectSemanticIndex> {
        &self.semantic_index
    }

    pub fn program_hash(&self) -> &ProgramHash {
        self.semantic_index.program_hash()
    }

    pub fn syntax_warnings(&self) -> usize {
        self.modules()
            .iter()
            .map(CompiledProjectModule::syntax_warnings)
            .sum()
    }
}

impl std::fmt::Debug for ProjectAnalysisLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectAnalysisLease")
            .field("tooling", &self.tooling)
            .field("world", self.registered_world.symbols().world())
            .field(
                "symbol_revision",
                self.registered_world.symbols().revision(),
            )
            .field("assertion_build_profile", &self.assertion_build_profile)
            .field("semantic_work", &self.final_analysis.work())
            .field("program_hash", self.semantic_index.program_hash())
            .finish()
    }
}

/// Latest fully constructed compiler phase; later phases own their ancestors.
///
/// A semantic lease authorizes semantic queries, while only `Compiled`
/// exposes verification and runtime products. There is no independent tuple
/// of HIR, semantic and executable snapshots that consumers can cross-wire.
#[derive(Clone, Debug)]
pub enum ProjectCompilationLease {
    Hir(Arc<ProjectToolingLease>),
    Analyzed(Arc<ProjectAnalysisLease>),
    Compiled(Arc<CompiledProject>),
}

impl ProjectCompilationLease {
    pub fn tooling_lease(&self) -> &Arc<ProjectToolingLease> {
        match self {
            Self::Hir(tooling) => tooling,
            Self::Analyzed(analysis) => analysis.tooling_lease(),
            Self::Compiled(compiled) => compiled.analysis_lease().tooling_lease(),
        }
    }

    pub fn analysis_lease(&self) -> Option<&Arc<ProjectAnalysisLease>> {
        match self {
            Self::Hir(_) => None,
            Self::Analyzed(analysis) => Some(analysis),
            Self::Compiled(compiled) => Some(compiled.analysis_lease()),
        }
    }

    pub const fn compiled(&self) -> Option<&Arc<CompiledProject>> {
        match self {
            Self::Hir(_) | Self::Analyzed(_) => None,
            Self::Compiled(compiled) => Some(compiled),
        }
    }
}
