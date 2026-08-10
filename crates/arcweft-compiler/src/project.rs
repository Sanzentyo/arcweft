//! Multi-module project compilation driver.
//!
//! Source loading stays in `arcweft-project-loader`. This module consumes a
//! Sans I/O `ProjectSources` value, admits the complete revision-bound parsed
//! source set into one HIR transaction, and retains the same shared
//! module-preserving `Arc<HirProject>` for every semantic and runtime consumer.

mod cache_batch;
mod dialogue_profile;
mod entry_runtime;
#[cfg(test)]
mod entry_tests;
mod registration;

pub use arcweft_lang_sema::assertion::AssertionBuildProfile;
pub(crate) use cache_batch::PendingProjectCompileStores;
#[cfg(test)]
use cache_batch::PendingStoreTransitionError;
pub use cache_batch::{InMemoryProjectCompileCache, NoProjectCompileCache, ProjectCompileCache};
pub use dialogue_profile::{
    CheckedDialogueProfile, DialogueProfileAdmissionError, DialogueProfileOwner,
};
pub(crate) use entry_runtime::EntryRuntimeProjection;
use entry_runtime::runtime_entry_lowering_input;
pub use registration::{
    AcceptedLaunchProfileInput, ProjectCompilationContext, ProjectEntrySelection,
    ProjectEntrySelectionKind,
};

use crate::view::CompiledViewProduct;
use crate::{lower, parse, style, view};
use arcweft_lang_hir::{
    database::HirDatabase,
    identity::{HirDatabaseCreateError, HirDatabaseId, HirSnapshotId},
    lowering::{HirLoweringControl, HirModuleKey, LoweringRequest},
    module::HirModule,
    project::{
        HirPackageModuleKey, HirProject, HirProjectBuildError, HirProjectBuilder, HirProjectModule,
    },
    symbol::{CallablePackageId, ProjectSymbolTable},
};
#[cfg(test)]
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::{
    entry::{CheckedEntryCatalog, CheckedEntryDiagnostic, CheckedEntryKind, check_project_entries},
    final_analysis::{
        FinalSemanticAnalysis, FinalSemanticAnalysisControl, FinalSemanticCatalogs,
        analyze_final_project, project_callable_tail_recovery_diagnostics,
    },
    project_index::{ProgramHash, ProjectSemanticIndex},
    proof_return::classify_proof_return_project,
    registration::{ProjectRegistrationFacts, RegisteredSemanticWorld, RegisteredTypeCheckEnv},
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    incremental::{ParsedSource, SyntaxDiagnostic, SyntaxParseStats},
    lint::{SyntaxLint, SyntaxLintSeverity, lint_id_policy},
};
use arcweft_presentation::fx::FxDefinition;
use arcweft_project::{
    graph::CompileUnitId,
    sources::{ProjectSourceFile, ProjectSources},
};
use arcweft_runtime_plan::flow::{RuntimePlanLowerReport, lower_runtime_plan_with_stats};
#[cfg(test)]
use arcweft_source::SourceDocumentId;
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceDocument, SourceDocumentIdentity,
    SourceName, SourceSetRevision,
};
use arcweft_verify::{
    Severity as VerificationSeverity, VerificationPolicy, VerificationReport, verify_project,
};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    sync::{Arc, atomic::AtomicBool},
};
use thiserror::Error;

/// Stable project compilation phase used by diagnostics and profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectCompileStage {
    Parse,
    Lint,
    HirLower,
    HirProject,
    Registration,
    Readiness,
    TypeCheck,
    EntryBinding,
    EntrySelection,
    StyleLower,
    ViewLower,
    DialogueProfileAdmission,
    RuntimePlanLower,
}

/// Whether one unit reused its in-process incremental artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectCompileCacheStatus {
    Disabled,
    Hit,
    Miss,
}

/// Structured project compiler diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCompileDiagnostic {
    module: Option<CanonicalModulePath>,
    stage: ProjectCompileStage,
    source: Option<ProjectDiagnosticSource>,
    syntax_diagnostic: Option<SyntaxDiagnostic>,
    diagnostic: Diagnostic,
}

/// Source snapshot attached to diagnostics produced from one loaded project file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDiagnosticSource {
    document: SourceDocument,
}

/// Independently parsed and lowered source module.
#[derive(Clone)]
pub struct CompiledProjectModule {
    module: CanonicalModulePath,
    compile_unit: CompileUnitId,
    syntax_lints: Vec<SyntaxLint>,
    syntax_stats: SyntaxParseStats,
    parsed: ParsedSource,
    hir: Arc<HirModule>,
}

/// Compiler-owned final-HIR session reused only within one process build session.
pub struct ProjectCompilationSession {
    hir: HirDatabase,
    accepted_hir_project: Option<(HirProjectCacheKey, Arc<HirProject>)>,
}

/// Private exact-snapshot key for the session's last accepted HIR project.
#[derive(Clone, Debug, Eq, PartialEq)]
struct HirProjectCacheKey {
    root_package: CallablePackageId,
    modules: Box<[(HirPackageModuleKey, SourceDocumentIdentity, HirSnapshotId)]>,
}

/// Deterministic content/dependency key for one compile unit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectCompileUnitFingerprint([u8; 32]);

/// One compile unit's public build summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCompileUnitSummary {
    id: CompileUnitId,
    modules: Vec<CanonicalModulePath>,
    fingerprint: ProjectCompileUnitFingerprint,
    cache_status: ProjectCompileCacheStatus,
}

/// Fully compiled project bound to one exact module-preserving HIR generation.
pub struct CompiledProject {
    tooling: Arc<ProjectToolingLease>,
    registered_world: Arc<RegisteredSemanticWorld>,
    assertion_build_profile: AssertionBuildProfile,
    final_analysis: Arc<FinalSemanticAnalysis>,
    verification: Arc<VerificationReport>,
    checked_entries: CheckedEntryCatalog,
    semantic_index: Arc<ProjectSemanticIndex>,
    style: style::CompiledViewStyleArtifact,
    fx_definitions: Arc<[FxDefinition]>,
    view_product: CompiledViewProduct,
    dialogue_profile: CheckedDialogueProfile,
    runtime_plan: RuntimePlanLowerReport,
}

/// Immutable pre-executable compiler product retained for typed tooling.
///
/// This lease is assembled only after every module and the module-preserving
/// HIR project have committed. Executable compilation either retains this
/// exact allocation or returns it with a later-stage failure; no consumer may
/// rebuild the project, symbol table, or parsed sources from text.
pub struct ProjectToolingLease {
    modules: Arc<[CompiledProjectModule]>,
    units: Arc<[ProjectCompileUnitSummary]>,
    hir_project: Arc<HirProject>,
    symbols: Arc<ProjectSymbolTable>,
    diagnostics: Arc<[ProjectCompileDiagnostic]>,
}

impl std::fmt::Debug for ProjectToolingLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectToolingLease")
            .field("hir_database", &self.hir_project.database_id())
            .field("modules", &self.modules.len())
            .field("compile_units", &self.units.len())
            .field("symbol_world", self.symbols.world())
            .field("symbol_revision", self.symbols.revision())
            .field("diagnostics", &self.diagnostics.len())
            .finish()
    }
}

impl std::fmt::Debug for CompiledProjectModule {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledProjectModule")
            .field("module", &self.module)
            .field("compile_unit", &self.compile_unit)
            .field("syntax_lints", &self.syntax_lints)
            .field("syntax_stats", &self.syntax_stats)
            .field("parsed", &self.parsed)
            .field("source", &self.hir.provenance().source_identity())
            .field("hir_snapshot", &self.hir.snapshot_id())
            .finish()
    }
}

impl std::fmt::Debug for CompiledProject {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledProject")
            .field("tooling", &self.tooling)
            .field("assertion_build_profile", &self.assertion_build_profile)
            .field("final_analysis", &self.final_analysis)
            .field("verification", &self.verification)
            .field("checked_entries", &self.checked_entries)
            .field("registered_world", &self.registered_world)
            .field("semantic_index", &self.semantic_index)
            .field("style", &self.style)
            .field("fx_definitions", &self.fx_definitions)
            .field("view_product", &self.view_product)
            .field("dialogue_profile", &self.dialogue_profile)
            .field("runtime_plan", &self.runtime_plan)
            .field("program_hash", self.semantic_index.program_hash())
            .field("syntax_warnings", &self.syntax_warnings())
            .finish()
    }
}

/// Project compilation failure.
#[derive(Debug, Error)]
#[error("Arcweft project compilation failed during {stage}")]
pub struct ProjectCompileError {
    stage: &'static str,
    diagnostics: Vec<ProjectCompileDiagnostic>,
    tooling: Option<Arc<ProjectToolingLease>>,
}

impl ProjectCompileStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Lint => "lint",
            Self::HirLower => "hir-lower",
            Self::HirProject => "hir-project",
            Self::Registration => "registration",
            Self::Readiness => "readiness",
            Self::TypeCheck => "type-check",
            Self::EntryBinding => "entry-binding",
            Self::EntrySelection => "entry-selection",
            Self::StyleLower => "style-lower",
            Self::ViewLower => "view-lower",
            Self::DialogueProfileAdmission => "dialogue-profile-admission",
            Self::RuntimePlanLower => "runtime-plan-lower",
        }
    }
}

impl ProjectCompileCacheStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Hit => "hit",
            Self::Miss => "miss",
        }
    }

    pub const fn is_hit(self) -> bool {
        matches!(self, Self::Hit)
    }
}

impl ProjectCompileDiagnostic {
    pub fn module(&self) -> Option<&CanonicalModulePath> {
        self.module.as_ref()
    }

    pub const fn stage(&self) -> ProjectCompileStage {
        self.stage
    }

    pub const fn source(&self) -> Option<&ProjectDiagnosticSource> {
        self.source.as_ref()
    }

    /// Original revision-bound grammar diagnostic, when syntax recovery produced it.
    pub const fn syntax_diagnostic(&self) -> Option<&SyntaxDiagnostic> {
        self.syntax_diagnostic.as_ref()
    }

    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
}

impl ProjectDiagnosticSource {
    pub fn new(document: SourceDocument) -> Self {
        Self { document }
    }

    pub const fn name(&self) -> &SourceName {
        self.document.display_name()
    }

    pub const fn document(&self) -> &SourceDocument {
        &self.document
    }

    pub fn text(&self) -> Option<&str> {
        Some(self.document.text())
    }
}

impl ProjectCompileUnitFingerprint {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn to_hex(self) -> String {
        self.0
            .iter()
            .fold(String::with_capacity(64), |mut hex, byte| {
                write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
                hex
            })
    }
}

impl ProjectCompileUnitSummary {
    pub const fn id(&self) -> CompileUnitId {
        self.id
    }

    pub fn modules(&self) -> &[CanonicalModulePath] {
        &self.modules
    }

    pub const fn fingerprint(&self) -> ProjectCompileUnitFingerprint {
        self.fingerprint
    }

    pub const fn cache_status(&self) -> ProjectCompileCacheStatus {
        self.cache_status
    }
}

impl CompiledProjectModule {
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn compile_unit(&self) -> CompileUnitId {
        self.compile_unit
    }

    /// Exact source identity retained by this source-backed compiled module.
    ///
    /// # Panics
    ///
    /// Panics only if the compiler violates its construction invariant and
    /// stores a synthetic HIR module in a compiled project source slot.
    pub fn source(&self) -> &SourceDocumentIdentity {
        self.hir.provenance().source_identity()
    }

    /// Non-blocking syntax lints produced from this accepted module source.
    pub fn syntax_lints(&self) -> &[SyntaxLint] {
        &self.syntax_lints
    }

    pub fn syntax_warnings(&self) -> usize {
        parse::count_warning_lints(&self.syntax_lints)
    }

    pub const fn syntax_stats(&self) -> &SyntaxParseStats {
        &self.syntax_stats
    }

    pub const fn parsed(&self) -> &ParsedSource {
        &self.parsed
    }

    pub const fn hir(&self) -> &Arc<HirModule> {
        &self.hir
    }
}

impl ProjectCompilationSession {
    /// Creates the sole final-HIR database for one compiler session.
    pub fn try_new() -> Result<Self, HirDatabaseCreateError> {
        Ok(Self {
            hir: HirDatabase::try_new()?,
            accepted_hir_project: None,
        })
    }

    pub const fn hir_database_id(&self) -> HirDatabaseId {
        self.hir.database_id()
    }
}

impl HirProjectCacheKey {
    fn new(root_package: CallablePackageId, modules: &[HirProjectModule]) -> Self {
        let mut modules = modules
            .iter()
            .map(|module| {
                (
                    module.key(),
                    module.source().clone(),
                    module.module().snapshot_id(),
                )
            })
            .collect::<Vec<_>>();
        modules.sort();
        Self {
            root_package,
            modules: modules.into_boxed_slice(),
        }
    }
}

impl CompiledProject {
    pub fn modules(&self) -> &[CompiledProjectModule] {
        self.tooling.modules()
    }

    pub fn compile_units(&self) -> &[ProjectCompileUnitSummary] {
        self.tooling.compile_units()
    }

    /// Returns the exact shared module-preserving HIR produced by this build.
    pub fn hir_project(&self) -> &Arc<HirProject> {
        self.tooling.hir_project()
    }

    /// Returns the exact pre-executable compiler product retained by this build.
    pub const fn tooling_lease(&self) -> &Arc<ProjectToolingLease> {
        &self.tooling
    }

    pub fn project_symbols(&self) -> &ProjectSymbolTable {
        self.tooling.project_symbols()
    }

    pub fn registered_world(&self) -> &RegisteredSemanticWorld {
        &self.registered_world
    }

    /// Retains the exact registered world owned by this compiled project.
    pub fn registered_world_arc(&self) -> Arc<RegisteredSemanticWorld> {
        Arc::clone(&self.registered_world)
    }

    pub fn registered_environment(&self) -> &RegisteredTypeCheckEnv {
        self.registered_world.environment()
    }

    /// Returns the typed assertion profile retained by this compiled artifact.
    pub const fn assertion_build_profile(&self) -> AssertionBuildProfile {
        self.assertion_build_profile
    }

    /// Returns the exact final semantic report admitted for this HIR project generation.
    pub const fn final_analysis(&self) -> &Arc<FinalSemanticAnalysis> {
        &self.final_analysis
    }

    /// Returns verifier evidence bound to the same accepted HIR and semantic generation.
    pub const fn verification(&self) -> &Arc<VerificationReport> {
        &self.verification
    }

    pub const fn checked_entries(&self) -> &CheckedEntryCatalog {
        &self.checked_entries
    }

    /// Returns the exact Agent/LSP semantic projection accepted for this build.
    pub const fn semantic_index(&self) -> &Arc<ProjectSemanticIndex> {
        &self.semantic_index
    }

    /// Returns the compiler-owned program identity derived once from the
    /// package and compile-order unit fingerprints.
    pub fn program_hash(&self) -> &ProgramHash {
        self.semantic_index.program_hash()
    }

    pub const fn style(&self) -> &style::CompiledViewStyleArtifact {
        &self.style
    }

    pub fn fx_definitions(&self) -> &[FxDefinition] {
        &self.fx_definitions
    }

    pub const fn view_product(&self) -> &CompiledViewProduct {
        &self.view_product
    }

    /// Dialogue presentation admitted against this project's exact View/Style
    /// product. Direct source compilation receives a typed project-default
    /// owner rather than an unchecked runtime fallback.
    pub const fn dialogue_profile(&self) -> &CheckedDialogueProfile {
        &self.dialogue_profile
    }

    pub const fn runtime_plan(&self) -> &RuntimePlanLowerReport {
        &self.runtime_plan
    }

    /// Binds this exact compiled project's assertion sites to the canonical
    /// runtime-plan artifact key selected by the cache/build owner.
    ///
    /// The returned capability is session-only: it joins persisted assertion
    /// failures to this project's HIR identities without placing those
    /// identities in the artifact or deriving a second digest.
    pub fn execution_diagnostic_context(
        &self,
        artifact_key: arcweft_project::artifact::RuntimePlanArtifactKey,
    ) -> Result<
        crate::runtime_diagnostics::ExecutionDiagnosticContext,
        crate::runtime_diagnostics::ExecutionDiagnosticContextError,
    > {
        crate::runtime_diagnostics::ExecutionDiagnosticContext::try_from_runtime_plan_artifact(
            artifact_key,
            &self.runtime_plan,
        )
    }

    pub fn syntax_warnings(&self) -> usize {
        self.tooling
            .modules()
            .iter()
            .map(CompiledProjectModule::syntax_warnings)
            .sum()
    }
}

impl ProjectToolingLease {
    fn new(
        modules: Vec<CompiledProjectModule>,
        units: Vec<ProjectCompileUnitSummary>,
        hir_project: Arc<HirProject>,
        symbols: Arc<ProjectSymbolTable>,
        diagnostics: Vec<ProjectCompileDiagnostic>,
    ) -> Self {
        Self {
            modules: modules.into(),
            units: units.into(),
            hir_project,
            symbols,
            diagnostics: diagnostics.into(),
        }
    }

    pub fn modules(&self) -> &[CompiledProjectModule] {
        &self.modules
    }

    pub fn compile_units(&self) -> &[ProjectCompileUnitSummary] {
        &self.units
    }

    pub const fn hir_project(&self) -> &Arc<HirProject> {
        &self.hir_project
    }

    pub fn project_symbols(&self) -> &ProjectSymbolTable {
        &self.symbols
    }

    pub fn diagnostics(&self) -> &[ProjectCompileDiagnostic] {
        &self.diagnostics
    }
}

/// Compiles a project without retaining reusable unit artifacts.
pub fn compile_project(
    session: &mut ProjectCompilationSession,
    project: &ProjectSources,
    parsed_sources: &BTreeMap<CanonicalModulePath, ParsedSource>,
    context: &ProjectCompilationContext,
) -> Result<CompiledProject, ProjectCompileError> {
    compile_project_with_cache(
        session,
        project,
        parsed_sources,
        context,
        &mut NoProjectCompileCache,
    )
}

/// Compiles all project modules in deterministic compile-unit order.
///
/// Revision-bound parsed-source admission, linting, and HIR lowering are split
/// and cacheable per SCC unit. This boundary never reparses project text.
/// Project-wide semantic and runtime consumers read the same exact
/// `Arc<HirProject>`; no linked clone or source reconstruction is admitted.
///
/// # Panics
///
/// Panics only if the module graph inside `ProjectSources` references a module
/// that is absent from the same validated `ProjectSources` inventory.
#[allow(
    clippy::too_many_lines,
    reason = "one function owns the explicit pending-cache transaction through every project-wide stage"
)]
pub fn compile_project_with_cache<C>(
    session: &mut ProjectCompilationSession,
    project: &ProjectSources,
    parsed_sources: &BTreeMap<CanonicalModulePath, ParsedSource>,
    context: &ProjectCompilationContext,
    cache: &mut C,
) -> Result<CompiledProject, ProjectCompileError>
where
    C: ProjectCompileCache,
{
    project_source_documents(project, context.facts())?;
    let hir_lowering_control = HirLoweringControl::new();
    let (modules, summaries, mut pending_stores, registration_prelude, mut recovery_diagnostics) =
        compile_project_units(
            &mut session.hir,
            project,
            parsed_sources,
            context,
            cache,
            hir_lowering_control,
        )?;

    let mut attempted_project_cache_key = None;
    let result = (|| {
        let package =
            CallablePackageId::try_new(project.package().id.as_str()).map_err(|error| {
                linked_error(
                    ProjectCompileStage::HirProject,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("hir.project.package"),
                    ],
                )
            })?;
        let mut project_modules = Vec::with_capacity(modules.len());
        for module in &modules {
            let bound = HirProjectModule::try_new(
                &session.hir,
                &package,
                module.module(),
                module.source(),
                Arc::clone(module.hir()),
            )
            .map_err(|error| {
                linked_error(
                    ProjectCompileStage::HirProject,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("hir.project.module"),
                    ],
                )
            })?;
            project_modules.push(bound);
        }
        let project_cache_key = HirProjectCacheKey::new(package.clone(), &project_modules);
        attempted_project_cache_key = Some(project_cache_key.clone());
        let hir_project = if let Some((cached_key, cached_project)) =
            session.accepted_hir_project.as_ref()
            && cached_key == &project_cache_key
        {
            Arc::clone(cached_project)
        } else {
            let mut project_builder = HirProjectBuilder::new(&session.hir, package);
            for module in project_modules {
                project_builder.insert_module(module).map_err(|error| {
                    linked_error(
                        ProjectCompileStage::HirProject,
                        [
                            Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                                .with_code("hir.project"),
                        ],
                    )
                })?;
            }
            Arc::new(project_builder.finish().map_err(|error| match error {
                HirProjectBuildError::DialogueLines(rejection) => linked_error(
                    ProjectCompileStage::HirProject,
                    rejection
                        .diagnostics()
                        .iter()
                        .map(
                            arcweft_lang_hir::line_identity::DialogueLineDiagnostic::to_source_diagnostic,
                        ),
                ),
                error => linked_error(
                    ProjectCompileStage::HirProject,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("hir.project"),
                    ],
                ),
            })?)
        };
        let mut semantic_tail_diagnostics = Vec::new();
        for module in &modules {
            let projected = project_callable_tail_recovery_diagnostics(
                module.hir(),
                registration_prelude.symbols(),
                module.parsed().document(),
            )
            .map_err(|error| {
                linked_error(
                    ProjectCompileStage::TypeCheck,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("sema.recovery_projection"),
                    ],
                )
            })?;
            let source = ProjectDiagnosticSource::new(module.parsed().document().clone());
            semantic_tail_diagnostics.extend(projected.into_iter().map(|projected| {
                ProjectCompileDiagnostic {
                    module: Some(module.module().clone()),
                    stage: ProjectCompileStage::TypeCheck,
                    source: Some(source.clone()),
                    syntax_diagnostic: None,
                    diagnostic: projected.into_diagnostic(),
                }
            }));
        }
        let has_semantic_tail_diagnostics = !semantic_tail_diagnostics.is_empty();
        recovery_diagnostics.extend(semantic_tail_diagnostics);
        let tooling = Arc::new(ProjectToolingLease::new(
            modules,
            summaries,
            Arc::clone(&hir_project),
            Arc::clone(registration_prelude.symbol_lease()),
            recovery_diagnostics,
        ));

        (|| {
        if has_semantic_tail_diagnostics {
            return Err(linked_error(
                ProjectCompileStage::TypeCheck,
                std::iter::empty::<Diagnostic>(),
            ));
        }
        let registered_world = registration::finish_proof_return_registration(
            hir_project.as_ref(),
            registration_prelude,
            context,
        )?;
        let executable = hir_project.executable_view().map_err(|error| {
            linked_error(
                ProjectCompileStage::Readiness,
                [
                    Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                        .with_code("hir.project.execution"),
                ],
            )
        })?;
        let semantic_cancellation = AtomicBool::new(false);
        let final_analysis = Arc::new(
            analyze_final_project(
                executable,
                registered_world.symbols(),
                FinalSemanticCatalogs::production(&registered_world),
                FinalSemanticAnalysisControl::new(&semantic_cancellation)
                    .with_assertion_build_profile(context.assertion_build_profile()),
            )
            .map_err(|error| {
                let diagnostic = error.source_diagnostic().unwrap_or_else(|| {
                    Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                        .with_code(error.diagnostic_code())
                });
                linked_error(
                    ProjectCompileStage::TypeCheck,
                    [diagnostic],
                )
            })?,
        );
        let verification = Arc::new(
            verify_project(
                executable,
                registered_world.symbols(),
                final_analysis.as_ref(),
                VerificationPolicy::default(),
            )
            .map_err(|error| {
                linked_error(
                    ProjectCompileStage::RuntimePlanLower,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("verify.project.input"),
                    ],
                )
            })?,
        );
        let verification_errors = verification
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == VerificationSeverity::Error)
            .map(|diagnostic| project_verification_diagnostic(diagnostic, context))
            .collect::<Vec<_>>();
        if !verification_errors.is_empty() {
            return Err(linked_error_with_compilation_sources(
                ProjectCompileStage::RuntimePlanLower,
                context,
                verification_errors,
            ));
        }
        let checked_entries = check_project_entries(
            executable,
            registered_world.symbols(),
            final_analysis.as_ref(),
        )
        .map_err(|diagnostics| {
            linked_error_with_registration_sources(
                ProjectCompileStage::EntryBinding,
                context.facts(),
                diagnostics.iter().map(entry_binding_diagnostic),
            )
        })?;
        validate_entry_selection(&checked_entries, context.entry_selection())?;
        let semantic_index = Arc::new(
            ProjectSemanticIndex::try_from_final_project(
                project_program_hash(project.package().id.as_str(), tooling.compile_units()),
                executable,
                registered_world.symbols(),
                final_analysis.as_ref(),
                &checked_entries,
            )
            .map_err(|error| {
                linked_error(
                    ProjectCompileStage::TypeCheck,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("sema.project_index"),
                    ],
                )
            })?,
        );
        let style = style::lower_project_view_styles(&hir_project, final_analysis.as_ref())
            .map_err(|error| {
                linked_error(
                    ProjectCompileStage::StyleLower,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("style.lower"),
                    ],
                )
            })?;
        // Environment-owned FX definitions are supplied by their checked
        // registration owner. The deleted flattened-HIR reader must not be
        // recreated from source text or from an obsolete HIR clone.
        let fx_definitions = Arc::<[FxDefinition]>::from([]);
        let view_product = view::ViewProjectLowerer::for_project(
            &hir_project,
            final_analysis.as_ref(),
            registered_world.symbols(),
            &style,
            project,
            context.resource_types(),
        )
        .and_then(view::ViewProjectLowerer::lower)
        .map_err(|error| {
            linked_error_with_registration_sources(
                ProjectCompileStage::ViewLower,
                context.facts(),
                [error.diagnostic()],
            )
        })?;
        let dialogue_profile_input = if let Some(input) = context.accepted_launch_profile() {
            dialogue_profile::DialogueProfileAdmissionInput::Launch(input)
        } else {
            let topology_sources = SourceSetRevision::try_for_identities(
                std::iter::once(project.manifest_document().identity()).chain(
                    project
                        .modules()
                        .map(|source| source.document().identity()),
                ),
            )
            .map_err(|error| {
                linked_error(
                    ProjectCompileStage::DialogueProfileAdmission,
                    [Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                        .with_code("profile.dialogue.project-source-revision")
                        .with_label(DiagnosticLabel::primary(
                            project.manifest_document().start_span(),
                            Some(
                                "the project-default dialogue profile could not bind this source inventory"
                                    .to_owned(),
                            ),
                        ))],
                )
            })?;
            dialogue_profile::DialogueProfileAdmissionInput::ProjectDefault {
                manifest: project.manifest_document(),
                topology_sources,
            }
        };
        let dialogue_profile = CheckedDialogueProfile::try_admit(
            dialogue_profile_input,
            &view_product,
            context.resource_types(),
        )
        .map_err(|error| {
            linked_error_with_compilation_sources(
                ProjectCompileStage::DialogueProfileAdmission,
                context,
                [error.diagnostic()],
            )
        })?;
        let runtime_facts = lower::project_runtime_semantic_facts(
            executable,
            registered_world.symbols(),
            &final_analysis,
            Some((dialogue_profile.presentation(), dialogue_profile.revision())),
            context.accepted_launch_profile().and_then(|input| {
                input
                    .resolved_profile()
                    .localization()
                    .character_names()
            }),
        )
        .map_err(|error| {
            linked_error(
                ProjectCompileStage::RuntimePlanLower,
                [
                    Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                        .with_code("compiler.runtime_semantic_projection"),
                ],
            )
        })?;
        let entry_runtime_input = runtime_entry_lowering_input(
            executable,
            registered_world.symbols(),
            &final_analysis,
            &checked_entries,
            context.command_policy(),
        )
        .map_err(|error| {
            linked_error(
                ProjectCompileStage::RuntimePlanLower,
                [
                    Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                        .with_code("compiler.entry_runtime_projection"),
                ],
            )
        })?;
        let runtime_plan =
            lower_runtime_plan_with_stats(executable, &runtime_facts, &entry_runtime_input)
                .map_err(|errors| {
                    linked_error(
                        ProjectCompileStage::RuntimePlanLower,
                        errors.into_iter().map(|error| {
                            Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                                .with_code("compiler.runtime_plan_lower")
                        }),
                    )
                })?;
        runtime_plan.plan.verify().map_err(|error| {
            linked_error(
                ProjectCompileStage::RuntimePlanLower,
                [
                    Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                        .with_code("compiler.runtime_plan_verify"),
                ],
            )
        })?;

        Ok(CompiledProject {
            tooling: Arc::clone(&tooling),
            registered_world,
            assertion_build_profile: context.assertion_build_profile(),
            final_analysis,
            verification,
            checked_entries,
            semantic_index,
            style,
            fx_definitions,
            view_product,
            dialogue_profile,
            runtime_plan,
        })
        })()
        .map_err(|error: ProjectCompileError| error.with_tooling_lease(Arc::clone(&tooling)))
    })();
    match result {
        Ok(compiled) => {
            session.accepted_hir_project = Some((
                attempted_project_cache_key
                    .expect("successful compilation constructed one exact HIR project cache key"),
                Arc::clone(compiled.hir_project()),
            ));
            pending_stores
                .flush(cache)
                .expect("pending compiler stores are finalized exactly once after assembly");
            Ok(compiled)
        }
        Err(error) => {
            pending_stores.discard();
            Err(error)
        }
    }
}

fn project_program_hash(package: &str, units: &[ProjectCompileUnitSummary]) -> ProgramHash {
    let mut bytes = Vec::with_capacity(package.len() + units.len() * 32);
    bytes.extend_from_slice(package.as_bytes());
    for unit in units {
        bytes.extend_from_slice(&unit.fingerprint().as_bytes());
    }
    ProgramHash::new(blake3::hash(&bytes).to_hex().to_string())
}

type CompiledProjectUnits = (
    Vec<CompiledProjectModule>,
    Vec<ProjectCompileUnitSummary>,
    PendingProjectCompileStores,
    arcweft_lang_sema::registration::ProofReturnRegistrationPrelude,
    Vec<ProjectCompileDiagnostic>,
);

#[expect(
    clippy::too_many_lines,
    reason = "compile-unit admission is one atomic cache and diagnostic transaction"
)]
fn compile_project_units<C>(
    hir_database: &mut HirDatabase,
    project: &ProjectSources,
    parsed_sources: &BTreeMap<CanonicalModulePath, ParsedSource>,
    context: &ProjectCompilationContext,
    cache: &mut C,
    hir_lowering_control: HirLoweringControl,
) -> Result<CompiledProjectUnits, ProjectCompileError>
where
    C: ProjectCompileCache,
{
    let fingerprints = build_unit_fingerprints(project);
    let package = CallablePackageId::try_new(project.package().id.as_str()).map_err(|error| {
        linked_error(
            ProjectCompileStage::HirProject,
            [
                Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                    .with_code("hir.project.package"),
            ],
        )
    })?;
    let incremental = project.build().incremental;
    let mut unit_statuses = BTreeMap::new();
    let mut cached_modules = BTreeMap::new();
    for &unit_id in project.graph().compile_order() {
        let fingerprint = fingerprints[&unit_id];
        let cached = incremental
            .then(|| cache.load(fingerprint))
            .flatten()
            .filter(|modules| {
                cached_unit_matches(hir_database, project, parsed_sources, unit_id, modules)
            });
        let status = if let Some(modules) = cached {
            for module in modules {
                cached_modules.insert(module.module().clone(), module);
            }
            ProjectCompileCacheStatus::Hit
        } else if incremental {
            ProjectCompileCacheStatus::Miss
        } else {
            ProjectCompileCacheStatus::Disabled
        };
        unit_statuses.insert(unit_id, status);
    }

    let mut requests = Vec::with_capacity(project.modules().len());
    let mut pending = BTreeMap::new();
    let mut recovery_diagnostics = Vec::new();
    for &unit_id in project.graph().compile_order() {
        let unit = project.graph().compile_unit(unit_id);
        for module in unit.modules() {
            let source = project
                .module(module)
                .expect("module graph only references loaded project sources");
            let parsed = parsed_sources.get(module).ok_or_else(|| {
                linked_error(
                    ProjectCompileStage::Parse,
                    [Diagnostic::new(
                        DiagnosticSeverity::Error,
                        format!("project has no accepted parsed source for `{module}`"),
                    )
                    .with_code("syntax.project.missing_snapshot")],
                )
            })?;
            let document = parsed.document();
            if document.identity() != source.document().identity()
                || document.text() != source.source()
            {
                return Err(module_error(
                    source,
                    document,
                    ProjectCompileStage::Parse,
                    [Diagnostic::new(
                        DiagnosticSeverity::Error,
                        "parsed source does not match the accepted project source",
                    )
                    .with_code("syntax.project.source_mismatch")],
                ));
            }
            recovery_diagnostics.extend(module_parse_diagnostics(
                source,
                document,
                parsed.diagnostics(),
            ));
            let syntax_stats = parsed.syntax_stats();
            let lints = lint_id_policy(parsed).map_err(|error| {
                module_error(
                    source,
                    document,
                    ProjectCompileStage::Lint,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("syntax.lint.projection"),
                    ],
                )
            })?;
            if parse::has_error_lints(&lints) {
                recovery_diagnostics.extend(
                    module_error(
                        source,
                        document,
                        ProjectCompileStage::Lint,
                        lints
                            .iter()
                            .filter(|lint| lint.severity() == SyntaxLintSeverity::Error)
                            .map(|lint| lint.diagnostic(parsed.document())),
                    )
                    .diagnostics,
                );
            }
            let key = HirModuleKey::new(
                package.clone(),
                source.module().clone(),
                document.identity().clone(),
            );
            let request = LoweringRequest::try_new(key, parsed).map_err(|error| {
                module_error(
                    source,
                    document,
                    ProjectCompileStage::HirLower,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("hir.lower.request"),
                    ],
                )
            })?;
            pending.insert(
                module.clone(),
                (unit_id, lints, syntax_stats, parsed.clone()),
            );
            if !cached_modules.contains_key(module) {
                requests.push(request);
            }
        }
    }
    let transaction = hir_database
        .stage_proof_return_project_with_retained(
            requests,
            cached_modules
                .values()
                .map(|module| Arc::clone(module.hir())),
            context.facts().world().clone(),
            *context.facts().symbol_revision(),
            context
                .facts()
                .documents()
                .map(|document| document.identity()),
            hir_lowering_control,
        )
        .map_err(|error| {
            linked_error(
                ProjectCompileStage::HirLower,
                [
                    Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                        .with_code("hir.lower.project_transaction"),
                ],
            )
        })?;
    let headers = transaction.headers().cloned().collect::<Vec<_>>();
    let generation = Arc::clone(transaction.generation());
    let header_view = transaction.header_view();
    let registration_prelude = registration::prepare_proof_return_registration(
        Arc::clone(&generation),
        header_view,
        context,
    )?;
    let classification = classify_proof_return_project(
        generation,
        &headers,
        header_view,
        registration_prelude.symbols(),
        registration_prelude.nominal_world(),
    )
    .map_err(|error| {
        linked_error(
            ProjectCompileStage::TypeCheck,
            [
                Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                    .with_code("sema.proof_return"),
            ],
        )
    })?;
    let outputs = transaction
        .publish_modules_with_semantic_facts(hir_database, classification.into_facts())
        .map_err(|error| {
            linked_error(
                ProjectCompileStage::HirLower,
                [
                    Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                        .with_code("hir.lower.project_publish"),
                ],
            )
        })?;
    let mut modules = Vec::with_capacity(outputs.len());
    for hir in outputs {
        let module = hir.key().path().clone();
        let (compile_unit, syntax_lints, syntax_stats, parsed) = pending
            .remove(&module)
            .expect("published project transaction returns every accepted module exactly once");
        modules.push(CompiledProjectModule {
            module,
            compile_unit,
            syntax_lints,
            syntax_stats,
            parsed,
            hir,
        });
    }
    debug_assert!(pending.is_empty());
    modules.sort_by(|left, right| left.module().cmp(right.module()));

    let mut summaries = Vec::with_capacity(project.graph().compile_units().len());
    let mut pending_stores = PendingProjectCompileStores::new();
    for &unit_id in project.graph().compile_order() {
        let unit = project.graph().compile_unit(unit_id);
        let fingerprint = fingerprints[&unit_id];
        let compiled = modules
            .iter()
            .filter(|module| module.compile_unit() == unit_id)
            .cloned()
            .collect::<Vec<_>>();
        let cache_status = unit_statuses[&unit_id];
        if cache_status == ProjectCompileCacheStatus::Miss {
            pending_stores
                .push(fingerprint, compiled)
                .expect("pending stores remain collecting during project transaction");
        }
        summaries.push(ProjectCompileUnitSummary {
            id: unit_id,
            modules: unit.modules().to_vec(),
            fingerprint,
            cache_status,
        });
    }
    Ok((
        modules,
        summaries,
        pending_stores,
        registration_prelude,
        recovery_diagnostics,
    ))
}

fn project_source_documents(
    project: &ProjectSources,
    facts: &ProjectRegistrationFacts,
) -> Result<BTreeMap<CanonicalModulePath, Arc<SourceDocument>>, ProjectCompileError> {
    project
        .modules()
        .map(|source| {
            let candidate = source.document();
            let document = facts
                .documents()
                .find(|document| document.identity().id() == candidate.identity().id())
                .filter(|document| {
                    document.identity() == candidate.identity()
                        && document.text() == candidate.text()
                })
                .cloned()
                .ok_or_else(|| {
                    linked_error(
                        ProjectCompileStage::HirProject,
                        [Diagnostic::new(
                            DiagnosticSeverity::Error,
                            format!(
                                "registration facts do not contain the accepted source document for `{}`",
                                source.path().display()
                            ),
                        )
                        .with_code("source.project_document")],
                    )
                })?;
            Ok((source.module().clone(), document))
        })
        .collect()
}

fn build_unit_fingerprints(
    project: &ProjectSources,
) -> BTreeMap<CompileUnitId, ProjectCompileUnitFingerprint> {
    let mut fingerprints: BTreeMap<CompileUnitId, ProjectCompileUnitFingerprint> = BTreeMap::new();
    for &unit_id in project.graph().compile_order() {
        let unit = project.graph().compile_unit(unit_id);
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft-project-compile-unit-v2\0");
        hasher.update(project.package().id.as_str().as_bytes());
        for module in unit.modules() {
            hasher.update(module.to_string().as_bytes());
            hasher.update(project.module(module).unwrap().source_revision().as_bytes());
        }
        for dependency in unit.dependencies() {
            let fingerprint = fingerprints
                .get(dependency)
                .expect("compile order places body dependency fingerprints first");
            hasher.update(&fingerprint.0);
        }
        fingerprints.insert(
            unit_id,
            ProjectCompileUnitFingerprint(*hasher.finalize().as_bytes()),
        );
    }
    fingerprints
}

fn cached_unit_matches(
    hir_database: &HirDatabase,
    project: &ProjectSources,
    parsed_sources: &BTreeMap<CanonicalModulePath, ParsedSource>,
    unit_id: CompileUnitId,
    cached: &[CompiledProjectModule],
) -> bool {
    let unit = project.graph().compile_unit(unit_id);
    if cached.len() != unit.modules().len() {
        return false;
    }

    let mut observed = BTreeMap::new();
    for module in cached {
        if observed.insert(module.module(), module).is_some() {
            return false;
        }
    }
    unit.modules().iter().all(|expected| {
        let Some(module) = observed.get(expected).copied() else {
            return false;
        };
        let Some(parsed) = parsed_sources.get(expected) else {
            return false;
        };
        let Some(source) = project.module(expected) else {
            return false;
        };
        let Some(current) = hir_database.current(module.hir().key()) else {
            return false;
        };
        module.compile_unit() == unit_id
            && module.parsed().is_same_snapshot(parsed)
            && parsed.document().identity() == source.document().identity()
            && module.hir().key().path() == expected
            && module.hir().key().source() == parsed.document().identity()
            && module.hir().provenance().syntax_snapshot() == parsed.snapshot_id()
            && module.hir().provenance().source_identity() == parsed.document().identity()
            && module.hir().is_cache_eligible()
            && Arc::ptr_eq(&current, module.hir())
    })
}

fn module_error(
    module_source: &ProjectSourceFile,
    document: &SourceDocument,
    stage: ProjectCompileStage,
    diagnostics: impl IntoIterator<Item = Diagnostic>,
) -> ProjectCompileError {
    let module = module_source.module().clone();
    let source = ProjectDiagnosticSource::new(document.clone());
    ProjectCompileError {
        stage: stage.as_str(),
        diagnostics: diagnostics
            .into_iter()
            .map(|diagnostic| ProjectCompileDiagnostic {
                module: Some(module.clone()),
                stage,
                source: Some(source.clone()),
                syntax_diagnostic: None,
                diagnostic,
            })
            .collect(),
        tooling: None,
    }
}

fn module_parse_diagnostics(
    module_source: &ProjectSourceFile,
    document: &SourceDocument,
    diagnostics: &[SyntaxDiagnostic],
) -> Vec<ProjectCompileDiagnostic> {
    let module = module_source.module().clone();
    let source = ProjectDiagnosticSource::new(document.clone());
    diagnostics
        .iter()
        .cloned()
        .map(|syntax_diagnostic| {
            let mut diagnostic =
                Diagnostic::new(DiagnosticSeverity::Error, syntax_diagnostic.message())
                    .with_code(syntax_diagnostic.code())
                    .with_label(DiagnosticLabel::primary(
                        syntax_diagnostic.primary().clone(),
                        None,
                    ));
            if let Some(related) = syntax_diagnostic.related() {
                diagnostic = diagnostic.with_label(DiagnosticLabel::secondary(
                    related.clone(),
                    Some("related syntax recovery".to_owned()),
                ));
            }
            ProjectCompileDiagnostic {
                module: Some(module.clone()),
                stage: ProjectCompileStage::Parse,
                source: Some(source.clone()),
                diagnostic,
                syntax_diagnostic: Some(syntax_diagnostic),
            }
        })
        .collect()
}

fn entry_binding_diagnostic(diagnostic: &CheckedEntryDiagnostic) -> Diagnostic {
    let mut rendered = Diagnostic::new(DiagnosticSeverity::Error, diagnostic.message())
        .with_code(diagnostic.code())
        .with_span(diagnostic.primary().clone());
    for related in diagnostic.related() {
        rendered = rendered.with_label(DiagnosticLabel::secondary(
            related.clone(),
            Some("related entry binding declaration".to_owned()),
        ));
    }
    rendered
}

fn validate_entry_selection(
    catalog: &CheckedEntryCatalog,
    selection: Option<&ProjectEntrySelection>,
) -> Result<(), ProjectCompileError> {
    let Some(selection) = selection else {
        return Ok(());
    };
    let Some(binding) = catalog.get_public(selection.id()) else {
        return Err(linked_error(
            ProjectCompileStage::EntrySelection,
            [Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("selected source entry `{}` does not exist", selection.id()),
            )
            .with_code("compiler.entry_selection.missing")],
        ));
    };
    let actual = binding.kind();
    if entry_selection_kind_matches(selection.kind(), &actual) {
        Ok(())
    } else {
        Err(linked_error(
            ProjectCompileStage::EntrySelection,
            [Diagnostic::new(
                DiagnosticSeverity::Error,
                format!(
                    "selected source entry `{}` has kind `{}`, but the launch surface requires `{}`",
                    selection.id(),
                    actual.as_str(),
                    selection.kind().as_str(),
                ),
            )
            .with_code("compiler.entry_selection.kind_mismatch")],
        ))
    }
}

fn entry_selection_kind_matches(
    selected: ProjectEntrySelectionKind,
    actual: &CheckedEntryKind,
) -> bool {
    matches!(
        (selected, actual),
        (ProjectEntrySelectionKind::Game, CheckedEntryKind::Game)
            | (ProjectEntrySelectionKind::Editor, CheckedEntryKind::Editor)
            | (ProjectEntrySelectionKind::Cli, CheckedEntryKind::Cli)
            | (ProjectEntrySelectionKind::Server, CheckedEntryKind::Server)
            | (
                ProjectEntrySelectionKind::Activity,
                CheckedEntryKind::Activity
            )
            | (ProjectEntrySelectionKind::Test, CheckedEntryKind::Test)
            | (ProjectEntrySelectionKind::Bench, CheckedEntryKind::Bench)
            | (ProjectEntrySelectionKind::Agent, CheckedEntryKind::Agent)
    )
}

fn linked_error(
    stage: ProjectCompileStage,
    diagnostics: impl IntoIterator<Item = Diagnostic>,
) -> ProjectCompileError {
    ProjectCompileError {
        stage: stage.as_str(),
        diagnostics: diagnostics
            .into_iter()
            .map(|diagnostic| ProjectCompileDiagnostic {
                module: None,
                stage,
                source: None,
                syntax_diagnostic: None,
                diagnostic,
            })
            .collect(),
        tooling: None,
    }
}

fn project_verification_diagnostic(
    diagnostic: &arcweft_verify::VerificationDiagnostic,
    context: &ProjectCompilationContext,
) -> Diagnostic {
    diagnostic
        .source
        .as_ref()
        .and_then(|source| {
            context
                .facts()
                .documents()
                .find(|document| document.identity() == &source.source)
        })
        .map_or_else(
            || {
                Diagnostic::new(DiagnosticSeverity::Error, diagnostic.message.clone())
                    .with_code(diagnostic.id.clone())
            },
            |document| diagnostic.source_diagnostic(document),
        )
}

fn linked_error_with_registration_sources(
    stage: ProjectCompileStage,
    facts: &ProjectRegistrationFacts,
    diagnostics: impl IntoIterator<Item = Diagnostic>,
) -> ProjectCompileError {
    ProjectCompileError {
        stage: stage.as_str(),
        diagnostics: diagnostics
            .into_iter()
            .map(|diagnostic| {
                let source = diagnostic.span().and_then(|span| {
                    facts
                        .documents()
                        .find(|document| document.identity() == span.source())
                        .map(|document| ProjectDiagnosticSource::new(document.as_ref().clone()))
                });
                ProjectCompileDiagnostic {
                    module: None,
                    stage,
                    source,
                    syntax_diagnostic: None,
                    diagnostic,
                }
            })
            .collect(),
        tooling: None,
    }
}

fn linked_error_with_compilation_sources(
    stage: ProjectCompileStage,
    context: &ProjectCompilationContext,
    diagnostics: impl IntoIterator<Item = Diagnostic>,
) -> ProjectCompileError {
    ProjectCompileError {
        stage: stage.as_str(),
        diagnostics: diagnostics
            .into_iter()
            .map(|diagnostic| {
                let source = diagnostic.span().and_then(|span| {
                    context
                        .facts()
                        .documents()
                        .find(|document| document.identity() == span.source())
                        .cloned()
                        .or_else(|| {
                            context.accepted_launch_profile().and_then(|input| {
                                let document = input.manifest().document();
                                (document.identity() == span.source()).then(|| Arc::clone(document))
                            })
                        })
                        .map(|document| ProjectDiagnosticSource::new(document.as_ref().clone()))
                });
                ProjectCompileDiagnostic {
                    module: None,
                    stage,
                    source,
                    syntax_diagnostic: None,
                    diagnostic,
                }
            })
            .collect(),
        tooling: None,
    }
}

impl ProjectCompileError {
    fn with_tooling_lease(mut self, tooling: Arc<ProjectToolingLease>) -> Self {
        debug_assert!(self.tooling.is_none());
        if !tooling.diagnostics().is_empty() {
            let mut diagnostics = tooling.diagnostics().to_vec();
            diagnostics.append(&mut self.diagnostics);
            self.diagnostics = diagnostics;
        }
        self.tooling = Some(tooling);
        self
    }

    pub const fn stage(&self) -> &'static str {
        self.stage
    }

    pub fn diagnostics(&self) -> &[ProjectCompileDiagnostic] {
        &self.diagnostics
    }

    /// Returns the exact pre-executable project when failure occurred after
    /// the compiler had committed one tooling generation.
    pub fn tooling_lease(&self) -> Option<&Arc<ProjectToolingLease>> {
        self.tooling.as_ref()
    }
}

#[cfg(test)]
mod tests;
