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
    source_context: Arc<super::ProjectCompilationContext>,
    registered_world: Arc<RegisteredSemanticWorld>,
    assertion_build_profile: AssertionBuildProfile,
    final_analysis: Arc<FinalSemanticAnalysis>,
    semantic_index: Arc<ProjectSemanticIndex>,
}

impl ProjectAnalysisLease {
    pub(super) fn new(
        tooling: Arc<ProjectToolingLease>,
        source_context: Arc<super::ProjectCompilationContext>,
        registered_world: Arc<RegisteredSemanticWorld>,
        assertion_build_profile: AssertionBuildProfile,
        final_analysis: Arc<FinalSemanticAnalysis>,
        semantic_index: Arc<ProjectSemanticIndex>,
    ) -> Self {
        Self {
            tooling,
            source_context,
            registered_world,
            assertion_build_profile,
            final_analysis,
            semantic_index,
        }
    }

    /// Original complete source publication context used for this generation.
    /// A consumer starting a new project republishes its typed inputs instead
    /// of treating an accepted catalog projection as a fresh source base.
    pub const fn source_compilation_context(&self) -> &Arc<super::ProjectCompilationContext> {
        &self.source_context
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

    /// Compiles an admitted value or body root using this exact semantic
    /// generation and its complete free-input/whole-formal execution ABI.
    pub fn compile_deterministic_program(
        &self,
        source: arcweft_lang_sema::final_analysis::CheckedExecutionSource,
        instance: Option<arcweft_lang_sema::final_analysis::CheckedLocalUseInstantiation<'_>>,
        control: &crate::lower::ProjectInstantiationControl,
    ) -> Result<
        crate::lower::CompiledDeterministicProgram,
        crate::lower::DeterministicProgramCompileError,
    > {
        crate::lower::programs::compile_deterministic_program(self, source, instance, control)
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

impl ProjectAnalysisLease {
    /// Issues target entities only from this complete immutable source
    /// analysis/index/HIR lease. Metadata index builders cannot substitute for
    /// the final generation accepted here.
    pub fn try_project_entity_catalog(
        &self,
    ) -> Result<
        Arc<arcweft_lang_sema::project_index::AcceptedProjectEntityCatalog>,
        arcweft_lang_sema::project_index::ProjectEntityPublicationError,
    > {
        let project = self.hir_project().analysis_view().map_err(|_| {
            arcweft_lang_sema::project_index::ProjectEntityPublicationError::Generation(Box::new(
                arcweft_lang_sema::final_analysis::FinalSemanticAnalysisError::InvalidOwner,
            ))
        })?;
        arcweft_lang_sema::project_index::AcceptedProjectEntityCatalog::try_from_final_project(
            Arc::clone(self.semantic_index()),
            project,
            self.project_symbols(),
            self.final_analysis(),
        )
        .map(Arc::new)
    }
}
