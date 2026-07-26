//! Multi-module project compilation driver.
//!
//! Source loading stays in `arcweft-project-loader`. This module consumes a
//! Sans I/O `ProjectSources` value, compiles SCC-based units independently
//! through syntax/HIR, and retains one shared module-preserving
//! `Arc<HirProject>` alongside the transitional crate-global semantic-pass view.

mod cache_batch;
mod dialogue_profile;
mod entry_runtime;
#[cfg(test)]
mod entry_tests;
mod registration;

pub(crate) use cache_batch::PendingProjectCompileStores;
#[cfg(test)]
use cache_batch::PendingStoreTransitionError;
pub use cache_batch::{InMemoryProjectCompileCache, NoProjectCompileCache, ProjectCompileCache};
pub use dialogue_profile::{
    CheckedDialogueProfile, DialogueProfileAdmissionError, DialogueProfileOwner,
};
pub(crate) use entry_runtime::EntryRuntimeProjection;
pub use registration::{
    AcceptedLaunchProfileInput, ProjectCompilationContext, ProjectEntrySelection,
    ProjectEntrySelectionKind,
};

use crate::{hir, image, lower, parse, style, view};
use crate::{image::CompiledImageCatalog, view::CompiledViewProduct};
use arcweft_lang_hir::{
    model::HirModule,
    project::{HirProject, HirProjectModule},
    symbol::ProjectSymbolTable,
};
#[cfg(test)]
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::{
    check::TypeCheckReport,
    entry::{CheckedEntryCatalog, CheckedEntryDiagnostic, CheckedEntryKind, check_project_entries},
    registration::{ProjectRegistrationFacts, RegisteredSemanticWorld, RegisteredTypeCheckEnv},
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath,
    cst::SyntaxParseStats,
    lint::{SyntaxLint, SyntaxLintSeverity},
    parser::recovery::ParseError,
};
use arcweft_presentation::fx::FxDefinition;
use arcweft_project::{
    graph::CompileUnitId,
    sources::{ModuleSourceHash, ProjectSourceFile, ProjectSources},
};
use arcweft_runtime_plan::{
    flow::{RuntimePlanLowerOptions, RuntimePlanLowerReport},
    fx::lower_fx_definitions_for_package,
    line_task::LoweredLineTaskGroup,
};
#[cfg(test)]
use arcweft_source::SourceDocumentId;
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceDocument, SourceDocumentIdentity,
    SourceName, SourceSetRevision,
};
use std::{collections::BTreeMap, fmt::Write as _, sync::Arc};
use thiserror::Error;

/// Stable project compilation phase used by diagnostics and profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectCompileStage {
    Parse,
    Lint,
    HirLower,
    HirProject,
    Registration,
    Resolve,
    Readiness,
    TypeCheck,
    EntryBinding,
    EntrySelection,
    StyleLower,
    ImageLower,
    FxLower,
    ViewLower,
    DialogueProfileAdmission,
    LineTaskLower,
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
    parse_error: Option<ParseError>,
    diagnostic: Diagnostic,
}

/// Source snapshot attached to diagnostics produced from one loaded project file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDiagnosticSource {
    document: SourceDocument,
}

/// Independently parsed and lowered source module.
#[derive(Clone, Debug)]
pub struct CompiledProjectModule {
    module: CanonicalModulePath,
    compile_unit: CompileUnitId,
    source_hash: ModuleSourceHash,
    source: SourceDocumentIdentity,
    syntax_lints: Vec<SyntaxLint>,
    syntax_warnings: usize,
    syntax_stats: SyntaxParseStats,
    hir: HirModule,
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

/// Fully compiled project and both module-preserving and linked HIR views.
#[derive(Debug)]
pub struct CompiledProject {
    modules: Vec<CompiledProjectModule>,
    units: Vec<ProjectCompileUnitSummary>,
    hir_project: Arc<HirProject>,
    registered_world: Arc<RegisteredSemanticWorld>,
    linked_hir: HirModule,
    typecheck_report: TypeCheckReport,
    checked_entries: CheckedEntryCatalog,
    style: style::CompiledViewStyleArtifact,
    image_catalog: CompiledImageCatalog,
    fx_definitions: Arc<[FxDefinition]>,
    view_product: CompiledViewProduct,
    dialogue_profile: CheckedDialogueProfile,
    line_task_groups: Vec<LoweredLineTaskGroup>,
    runtime_plan: RuntimePlanLowerReport,
}

/// Project compilation failure.
#[derive(Debug, Error)]
#[error("Arcweft project compilation failed during {stage}")]
pub struct ProjectCompileError {
    stage: &'static str,
    diagnostics: Vec<ProjectCompileDiagnostic>,
}

impl ProjectCompileStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Lint => "lint",
            Self::HirLower => "hir-lower",
            Self::HirProject => "hir-project",
            Self::Registration => "registration",
            Self::Resolve => "resolve",
            Self::Readiness => "readiness",
            Self::TypeCheck => "type-check",
            Self::EntryBinding => "entry-binding",
            Self::EntrySelection => "entry-selection",
            Self::StyleLower => "style-lower",
            Self::ImageLower => "image-lower",
            Self::FxLower => "fx-lower",
            Self::ViewLower => "view-lower",
            Self::DialogueProfileAdmission => "dialogue-profile-admission",
            Self::LineTaskLower => "line-task-lower",
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

    /// Original typed parser payload when this diagnostic came from syntax
    /// parsing. Other compiler stages do not manufacture or reverse-decode
    /// parser kinds from transport code strings.
    pub const fn parse_error(&self) -> Option<&ParseError> {
        self.parse_error.as_ref()
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

    pub const fn source_hash(&self) -> ModuleSourceHash {
        self.source_hash
    }

    pub const fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }

    /// Non-blocking syntax lints produced from this accepted module source.
    pub fn syntax_lints(&self) -> &[SyntaxLint] {
        &self.syntax_lints
    }

    pub const fn syntax_warnings(&self) -> usize {
        self.syntax_warnings
    }

    pub const fn syntax_stats(&self) -> &SyntaxParseStats {
        &self.syntax_stats
    }

    pub const fn hir(&self) -> &HirModule {
        &self.hir
    }
}

impl CompiledProject {
    pub fn modules(&self) -> &[CompiledProjectModule] {
        &self.modules
    }

    pub fn compile_units(&self) -> &[ProjectCompileUnitSummary] {
        &self.units
    }

    /// Returns the exact shared module-preserving HIR produced by this build.
    pub const fn hir_project(&self) -> &Arc<HirProject> {
        &self.hir_project
    }

    pub fn project_symbols(&self) -> &ProjectSymbolTable {
        self.registered_world.symbols()
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

    pub const fn linked_hir(&self) -> &HirModule {
        &self.linked_hir
    }

    pub const fn typecheck_report(&self) -> &TypeCheckReport {
        &self.typecheck_report
    }

    pub const fn checked_entries(&self) -> &CheckedEntryCatalog {
        &self.checked_entries
    }

    pub const fn style(&self) -> &style::CompiledViewStyleArtifact {
        &self.style
    }

    pub const fn image_catalog(&self) -> &CompiledImageCatalog {
        &self.image_catalog
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

    pub fn line_task_groups(&self) -> &[LoweredLineTaskGroup] {
        &self.line_task_groups
    }

    pub const fn runtime_plan(&self) -> &RuntimePlanLowerReport {
        &self.runtime_plan
    }

    pub fn syntax_warnings(&self) -> usize {
        self.modules
            .iter()
            .map(CompiledProjectModule::syntax_warnings)
            .sum()
    }
}

/// Compiles a project without retaining reusable unit artifacts.
pub fn compile_project(
    project: &ProjectSources,
    context: &ProjectCompilationContext,
    runtime_options: &RuntimePlanLowerOptions,
) -> Result<CompiledProject, ProjectCompileError> {
    compile_project_with_cache(
        project,
        context,
        runtime_options,
        &mut NoProjectCompileCache,
    )
}

/// Compiles all project modules in deterministic compile-unit order.
///
/// Parsing, linting, and HIR lowering are split and cacheable per SCC unit.
/// Current name resolution, type checking, and runtime-plan lowering remain
/// crate-global and therefore run on `HirProject::linked_module`. The retained
/// `Arc<HirProject>` is the migration boundary for making those passes
/// module-aware and is shared unchanged with downstream accepted snapshots.
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
    project: &ProjectSources,
    context: &ProjectCompilationContext,
    runtime_options: &RuntimePlanLowerOptions,
    cache: &mut C,
) -> Result<CompiledProject, ProjectCompileError>
where
    C: ProjectCompileCache,
{
    let source_documents = project_source_documents(project, context.facts())?;
    let (modules, summaries, mut pending_stores) =
        compile_project_units(project, &source_documents, cache)?;

    let result = (|| {
        let mut project_modules = Vec::with_capacity(modules.len());
        for module in &modules {
            let bound = HirProjectModule::try_new(
                module.module.clone(),
                module.source.clone(),
                module.hir.clone(),
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
        let hir_project = Arc::new(
            HirProject::new(project.package().id.as_str(), project_modules).map_err(|error| {
                linked_error(
                    ProjectCompileStage::HirProject,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("hir.project"),
                    ],
                )
            })?,
        );
        let registered_world = registration::register(hir_project.as_ref(), context)?;
        let linked_hir = hir_project.linked_module();
        hir::resolve_registered_hir_references(&linked_hir, &registered_world).map_err(
            |errors| {
                linked_error(
                    ProjectCompileStage::Resolve,
                    errors.into_iter().map(|error| error.diagnostic()),
                )
            },
        )?;
        hir::validate_hir_typecheck_ready(&linked_hir).map_err(|errors| {
            linked_error(
                ProjectCompileStage::Readiness,
                errors.into_iter().map(|error| error.diagnostic()),
            )
        })?;
        let typecheck_report = hir::typecheck_registered_project(&linked_hir, &registered_world)
            .map_err(|errors| {
                linked_error(
                    ProjectCompileStage::TypeCheck,
                    errors.into_iter().map(|error| error.diagnostic()),
                )
            })?;
        let checked_entries = check_project_entries(
            &hir_project,
            registered_world.symbols(),
            registered_world.environment().callable_catalog(),
            &typecheck_report,
        )
        .map_err(|diagnostics| {
            linked_error_with_registration_sources(
                ProjectCompileStage::EntryBinding,
                context.facts(),
                diagnostics.iter().map(entry_binding_diagnostic),
            )
        })?;
        validate_entry_selection(&checked_entries, context.entry_selection())?;
        let style = style::lower_project_view_styles(
            &hir_project,
            &linked_hir,
            &typecheck_report.style_catalog,
            project,
        )
        .map_err(|error| {
            linked_error(
                ProjectCompileStage::StyleLower,
                [
                    Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                        .with_code("style.lower"),
                ],
            )
        })?;
        let image_catalog =
            image::lower_project_images(&hir_project, project).map_err(|error| {
                linked_error_with_registration_sources(
                    ProjectCompileStage::ImageLower,
                    context.facts(),
                    [error.diagnostic()],
                )
            })?;
        let fx_definitions = Arc::<[FxDefinition]>::from(
            lower_fx_definitions_for_package(&linked_hir, project.package().id.as_str()).map_err(
                |error| {
                    linked_error(
                        ProjectCompileStage::FxLower,
                        [
                            Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                                .with_code("compiler.fx.lower"),
                        ],
                    )
                },
            )?,
        );
        let view_product = view::ViewProjectLowerer::for_project(
            &hir_project,
            &typecheck_report,
            &style,
            project,
            &image_catalog,
            &fx_definitions,
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
        let line_task_groups = lower::lower_source_line_tasks(&linked_hir).map_err(|errors| {
            linked_error(
                ProjectCompileStage::LineTaskLower,
                errors.into_iter().map(|error| error.diagnostic()),
            )
        })?;
        let agent_controllers = entry_runtime::agent_controller_requests(&checked_entries)
            .map_err(|error| {
                linked_error(
                    ProjectCompileStage::RuntimePlanLower,
                    [
                        Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                            .with_code("compiler.entry_runtime_projection"),
                    ],
                )
            })?;
        let entry_callables = entry_runtime::stateful_callable_requests(&checked_entries);
        let runtime_options = runtime_options
            .clone()
            .with_package_identity(project.package().id.as_str())
            .with_agent_controllers(agent_controllers)
            .with_entry_callables(entry_callables)
            .with_dialogue_profile(
                dialogue_profile.presentation().clone(),
                dialogue_profile.revision().clone(),
            );
        let mut runtime_plan = lower::lower_source_runtime_plan_with_typecheck_stats_and_options(
            &linked_hir,
            &typecheck_report,
            &runtime_options,
        )
        .map_err(|errors| {
            linked_error(
                ProjectCompileStage::RuntimePlanLower,
                errors.into_iter().map(|error| error.diagnostic()),
            )
        })?;
        entry_runtime::attach_checked_entries(
            &mut runtime_plan,
            &checked_entries,
            runtime_options.command_policy(),
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
            modules,
            units: summaries,
            hir_project,
            registered_world,
            linked_hir,
            typecheck_report,
            checked_entries,
            style,
            image_catalog,
            fx_definitions,
            view_product,
            dialogue_profile,
            line_task_groups,
            runtime_plan,
        })
    })();
    match result {
        Ok(compiled) => {
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

fn compile_project_units<C>(
    project: &ProjectSources,
    source_documents: &BTreeMap<CanonicalModulePath, Arc<SourceDocument>>,
    cache: &mut C,
) -> Result<
    (
        Vec<CompiledProjectModule>,
        Vec<ProjectCompileUnitSummary>,
        PendingProjectCompileStores,
    ),
    ProjectCompileError,
>
where
    C: ProjectCompileCache,
{
    let fingerprints = build_unit_fingerprints(project);
    let incremental = project.build().incremental;
    let mut modules = Vec::with_capacity(project.modules().len());
    let mut summaries = Vec::with_capacity(project.graph().compile_units().len());
    let mut pending_stores = PendingProjectCompileStores::new();

    for &unit_id in project.graph().compile_order() {
        let unit = project.graph().compile_unit(unit_id);
        let fingerprint = fingerprints[&unit_id];
        let cached = incremental
            .then(|| cache.load(fingerprint))
            .flatten()
            .filter(|cached| cached_unit_matches(project, source_documents, unit_id, cached));
        let (compiled, cache_status) = if let Some(cached) = cached {
            (cached, ProjectCompileCacheStatus::Hit)
        } else {
            let compiled = unit
                .modules()
                .iter()
                .map(|module| {
                    let source = project
                        .module(module)
                        .expect("module graph only references loaded project sources");
                    compile_module(source, &source_documents[module], unit_id)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if incremental {
                pending_stores
                    .push(fingerprint, compiled.clone())
                    .expect("pending stores remain collecting during unit lowering");
            }
            let status = if incremental {
                ProjectCompileCacheStatus::Miss
            } else {
                ProjectCompileCacheStatus::Disabled
            };
            (compiled, status)
        };
        summaries.push(ProjectCompileUnitSummary {
            id: unit_id,
            modules: unit.modules().to_vec(),
            fingerprint,
            cache_status,
        });
        modules.extend(compiled);
    }
    Ok((modules, summaries, pending_stores))
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

fn compile_module(
    source: &ProjectSourceFile,
    document: &Arc<SourceDocument>,
    compile_unit: CompileUnitId,
) -> Result<CompiledProjectModule, ProjectCompileError> {
    let parsed = parse::parse_source_document(Arc::clone(document));
    if !parsed.errors().is_empty() {
        return Err(module_parse_error(source, document, parsed.errors()));
    }
    let syntax_stats = parsed.syntax_stats();
    let tree = parsed.into_typed_tree();
    let lints = parse::lint_source_tree(&tree);
    if parse::has_error_lints(&lints) {
        return Err(module_error(
            source,
            document,
            ProjectCompileStage::Lint,
            lints
                .iter()
                .filter(|lint| lint.severity() == SyntaxLintSeverity::Error)
                .map(|lint| lint.diagnostic(document)),
        ));
    }
    let syntax_warnings = parse::count_warning_lints(&lints);
    let hir = hir::lower_source_document(document, &tree).map_err(|errors| {
        module_error(
            source,
            document,
            ProjectCompileStage::HirLower,
            errors.into_iter().map(|error| error.diagnostic(document)),
        )
    })?;
    Ok(CompiledProjectModule {
        module: source.module().clone(),
        compile_unit,
        source_hash: source.source_hash(),
        source: document.identity().clone(),
        syntax_lints: lints,
        syntax_warnings,
        syntax_stats,
        hir,
    })
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
            hasher.update(&project.module(module).unwrap().source_hash().as_bytes());
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
    project: &ProjectSources,
    source_documents: &BTreeMap<CanonicalModulePath, Arc<SourceDocument>>,
    unit_id: CompileUnitId,
    cached: &[CompiledProjectModule],
) -> bool {
    let unit = project.graph().compile_unit(unit_id);
    cached.len() == unit.modules().len()
        && cached.iter().zip(unit.modules()).all(|(cached, expected)| {
            cached.module() == expected
                && cached.compile_unit() == unit_id
                && source_documents
                    .get(expected)
                    .is_some_and(|document| document.identity() == cached.source())
                && project
                    .module(expected)
                    .is_some_and(|source| source.source_hash() == cached.source_hash())
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
                parse_error: None,
                diagnostic,
            })
            .collect(),
    }
}

fn module_parse_error(
    module_source: &ProjectSourceFile,
    document: &SourceDocument,
    errors: &[ParseError],
) -> ProjectCompileError {
    let module = module_source.module().clone();
    let source = ProjectDiagnosticSource::new(document.clone());
    ProjectCompileError {
        stage: ProjectCompileStage::Parse.as_str(),
        diagnostics: errors
            .iter()
            .cloned()
            .map(|parse_error| ProjectCompileDiagnostic {
                module: Some(module.clone()),
                stage: ProjectCompileStage::Parse,
                source: Some(source.clone()),
                diagnostic: parse_error.diagnostic(document),
                parse_error: Some(parse_error),
            })
            .collect(),
    }
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
                parse_error: None,
                diagnostic,
            })
            .collect(),
    }
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
                    parse_error: None,
                    diagnostic,
                }
            })
            .collect(),
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
                    parse_error: None,
                    diagnostic,
                }
            })
            .collect(),
    }
}

impl ProjectCompileError {
    pub const fn stage(&self) -> &'static str {
        self.stage
    }

    pub fn diagnostics(&self) -> &[ProjectCompileDiagnostic] {
        &self.diagnostics
    }
}

#[cfg(test)]
mod tests;
