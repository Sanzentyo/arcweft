//! Multi-module project compilation driver.
//!
//! Source loading stays in `arcweft-project-loader`. This module consumes a
//! Sans I/O `ProjectSources` value, compiles SCC-based units independently
//! through syntax/HIR, and retains a module-preserving `HirProject` alongside
//! the transitional crate-global semantic-pass view.

mod cache_batch;
mod registration;

pub(crate) use cache_batch::PendingProjectCompileStores;
#[cfg(test)]
use cache_batch::PendingStoreTransitionError;
pub use cache_batch::{InMemoryProjectCompileCache, NoProjectCompileCache, ProjectCompileCache};
pub use registration::ProjectCompilationContext;

use crate::{hir, lower, parse, style};
use arcweft_lang_hir::{
    model::HirModule,
    project::{HirProject, HirProjectModule},
    symbol::ProjectSymbolTable,
};
#[cfg(test)]
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::{
    check::TypeCheckReport,
    registration::{ProjectRegistrationFacts, RegisteredSemanticWorld, RegisteredTypeCheckEnv},
};
use arcweft_lang_syntax::{
    ast::module_path::CanonicalModulePath, cst::SyntaxParseStats, lint::SyntaxLintSeverity,
};
use arcweft_project::{
    graph::CompileUnitId,
    sources::{ModuleSourceHash, ProjectSourceFile, ProjectSources},
};
use arcweft_runtime_plan::{
    flow::{RuntimePlanLowerOptions, RuntimePlanLowerReport},
    line_task::LoweredLineTaskGroup,
};
#[cfg(test)]
use arcweft_source::SourceDocumentId;
use arcweft_source::{
    Diagnostic, DiagnosticSeverity, SourceDocument, SourceDocumentIdentity, SourceName,
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
    StyleLower,
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
    hir_project: HirProject,
    registered_world: Arc<RegisteredSemanticWorld>,
    linked_hir: HirModule,
    typecheck_report: TypeCheckReport,
    style: style::CompiledViewStyleArtifact,
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
            Self::StyleLower => "style-lower",
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

    pub const fn hir_project(&self) -> &HirProject {
        &self.hir_project
    }

    pub fn project_symbols(&self) -> &ProjectSymbolTable {
        self.registered_world.symbols()
    }

    pub fn registered_world(&self) -> &RegisteredSemanticWorld {
        &self.registered_world
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

    pub const fn style(&self) -> &style::CompiledViewStyleArtifact {
        &self.style
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
/// `HirProject` is the migration boundary for making those passes module-aware.
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
        let hir_project = HirProject::new(
            project.manifest().package().name().as_str(),
            modules.iter().map(|module| {
                HirProjectModule::new(
                    module.module.clone(),
                    module.source.clone(),
                    module.hir.clone(),
                )
            }),
        )
        .map_err(|error| {
            linked_error(
                ProjectCompileStage::HirProject,
                [
                    Diagnostic::new(DiagnosticSeverity::Error, error.to_string())
                        .with_code("hir.project"),
                ],
            )
        })?;
        let registered_world = registration::register(&hir_project, context)?;
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
        let line_task_groups = lower::lower_source_line_tasks(&linked_hir).map_err(|errors| {
            linked_error(
                ProjectCompileStage::LineTaskLower,
                errors.into_iter().map(|error| error.diagnostic()),
            )
        })?;
        let runtime_options = runtime_options
            .clone()
            .with_package_identity(project.manifest().package().name().as_str());
        let runtime_plan = lower::lower_source_runtime_plan_with_typecheck_stats_and_options(
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

        Ok(CompiledProject {
            modules,
            units: summaries,
            hir_project,
            registered_world,
            linked_hir,
            typecheck_report,
            style,
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
    let incremental = project.manifest().build().incremental();
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
    document: &SourceDocument,
    compile_unit: CompileUnitId,
) -> Result<CompiledProjectModule, ProjectCompileError> {
    let parsed = parse::parse_source_text(source.source().to_owned());
    if !parsed.errors().is_empty() {
        return Err(module_error(
            source,
            document,
            ProjectCompileStage::Parse,
            parsed
                .errors()
                .iter()
                .map(|error| error.diagnostic(document)),
        ));
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
        hasher.update(project.manifest().package().name().as_str().as_bytes());
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
                diagnostic,
            })
            .collect(),
    }
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
mod tests {
    use super::*;
    use arcweft_lang_hir::symbol::{
        CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolWorldId,
    };
    use arcweft_lang_sema::registration::{
        EnvironmentBindingId, ExternalRegistrationFact, RegisteredExternalOwner,
    };
    use arcweft_lang_syntax::ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot},
        symbol_path::SymbolPath,
    };
    use arcweft_project::{manifest::ProjectManifest, sources::ProjectSourceFile};
    use arcweft_source::{DiagnosticLabel, SourceDocument, SourceRange};
    use std::path::PathBuf;

    #[test]
    fn project_compile_diagnostics_own_typed_diagnostic_and_source_snapshot() {
        let source_text = "flow @flow.opening start {\n}\n";
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("src/main.arcw").expect("document id"),
                SourceName::path("src/main.arcw"),
                source_text,
            )
            .expect("source document"),
        );
        let source = ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            PathBuf::from("src/main.arcw"),
            Arc::clone(&document),
            [],
        );
        let span = document
            .span(SourceRange::new(5, 18))
            .expect("diagnostic span");
        let error = module_error(
            &source,
            &document,
            ProjectCompileStage::Parse,
            [Diagnostic::new(DiagnosticSeverity::Error, "parse failed")
                .with_code("syntax.parse")
                .with_label(DiagnosticLabel::primary(
                    span,
                    Some("found token here".to_owned()),
                ))],
        );

        let diagnostic = error.diagnostics().first().expect("diagnostic");
        assert_eq!(
            diagnostic.module(),
            Some(&CanonicalModulePath::crate_root())
        );
        assert_eq!(diagnostic.stage(), ProjectCompileStage::Parse);
        assert_eq!(
            diagnostic.diagnostic().code().expect("code").as_str(),
            "syntax.parse"
        );
        assert_eq!(
            diagnostic.source().expect("source").text(),
            Some(source_text)
        );
        assert_eq!(
            diagnostic.source().expect("source").name().display_name(),
            "src/main.arcw"
        );
    }

    #[test]
    fn pending_store_state_is_one_way() {
        #[derive(Default)]
        struct RecordingCache {
            stores: Vec<(ProjectCompileUnitFingerprint, usize)>,
        }

        impl ProjectCompileCache for RecordingCache {
            fn load(
                &mut self,
                _fingerprint: ProjectCompileUnitFingerprint,
            ) -> Option<Vec<CompiledProjectModule>> {
                None
            }

            fn store(
                &mut self,
                fingerprint: ProjectCompileUnitFingerprint,
                modules: &[CompiledProjectModule],
            ) {
                self.stores.push((fingerprint, modules.len()));
            }
        }

        let fingerprint = ProjectCompileUnitFingerprint([7; 32]);
        let mut pending = PendingProjectCompileStores::new();
        pending
            .push(fingerprint, Vec::new())
            .expect("collecting accepts stores");
        let mut cache = RecordingCache::default();
        pending.flush(&mut cache).expect("first flush succeeds");
        assert_eq!(cache.stores, vec![(fingerprint, 0)]);
        assert_eq!(
            pending.push(fingerprint, Vec::new()),
            Err(PendingStoreTransitionError::AlreadyFinalized)
        );
        assert_eq!(
            pending.flush(&mut cache),
            Err(PendingStoreTransitionError::AlreadyFinalized)
        );
        assert_eq!(cache.stores, vec![(fingerprint, 0)]);

        let mut discarded = PendingProjectCompileStores::new();
        discarded.discard();
        discarded.discard();
        assert_eq!(
            discarded.push(fingerprint, Vec::new()),
            Err(PendingStoreTransitionError::AlreadyFinalized)
        );
        assert_eq!(
            discarded.flush(&mut cache),
            Err(PendingStoreTransitionError::AlreadyFinalized)
        );
    }

    #[test]
    fn registration_diagnostic_retains_accepted_source_document() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-project://compiler-registration/src/main.arcw")
                    .expect("document id"),
                SourceName::path("src/main.arcw"),
                "fn main() -> Unit { () }\n",
            )
            .expect("document"),
        );
        let world = ProjectSymbolWorldId::try_new(
            CallablePackageId::try_new("compiler-registration").expect("package"),
            document.identity().id().clone(),
            "test",
        )
        .expect("world");
        let facts = ProjectRegistrationFacts::try_new(
            world,
            vec![Arc::clone(&document)],
            Vec::new(),
            Vec::new(),
        )
        .expect("facts");
        let span = document.span(SourceRange::new(0, 2)).expect("span");
        let error = linked_error_with_registration_sources(
            ProjectCompileStage::Registration,
            &facts,
            [
                Diagnostic::new(DiagnosticSeverity::Error, "registration failed")
                    .with_code("aw.character.registration.unknown_owner")
                    .with_span(span),
            ],
        );

        let diagnostic = error.diagnostics().first().expect("diagnostic");
        let source = diagnostic.source().expect("accepted source document");
        assert_eq!(source.document().identity(), document.identity());
        assert_eq!(source.document().text(), document.text());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the cache-rollback test retains every stage input and the zero-store assertion in one scenario"
    )]
    fn pending_stores_discard_on_registration_error() {
        #[derive(Default)]
        struct RecordingCache {
            stores: usize,
        }

        impl ProjectCompileCache for RecordingCache {
            fn load(
                &mut self,
                _fingerprint: ProjectCompileUnitFingerprint,
            ) -> Option<Vec<CompiledProjectModule>> {
                None
            }

            fn store(
                &mut self,
                _fingerprint: ProjectCompileUnitFingerprint,
                _modules: &[CompiledProjectModule],
            ) {
                self.stores += 1;
            }
        }

        let source_text = "fn main() -> Unit { () }\n";
        let source_path = PathBuf::from("src/main.arcw");
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-project://compiler-registration/src/main.arcw")
                    .expect("document id"),
                SourceName::path(source_path.display().to_string()),
                source_text,
            )
            .expect("document"),
        );
        let project = ProjectSources::new(
            PathBuf::from("arcw.toml"),
            PathBuf::new(),
            ProjectManifest::parse_toml("[package]\nname = \"compiler-registration\"\n")
                .expect("manifest"),
            [ProjectSourceFile::new(
                CanonicalModulePath::crate_root(),
                source_path.clone(),
                Arc::clone(&document),
                [],
            )],
        )
        .expect("project");
        let declaration = document.span(SourceRange::new(0, 2)).expect("span");
        let owner = EnvironmentBindingId::try_new("environment.missing").expect("environment id");
        let direct_bindings = [owner.as_str()]
            .into_iter()
            .map(|name| {
                ProjectDirectBinding::try_new(
                    CanonicalModulePath::crate_root(),
                    name,
                    Some(Visibility::Public),
                    declaration.clone(),
                    false,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("direct bindings");
        let seed = ExternalDeclarationSeed::try_new(
            SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str())
                .expect("symbol path"),
            Some(Visibility::Public),
            declaration.clone(),
            direct_bindings,
        )
        .expect("external seed");
        let world = ProjectSymbolWorldId::try_new(
            CallablePackageId::try_new("compiler-registration").expect("package"),
            document.identity().id().clone(),
            "test",
        )
        .expect("world");
        let facts = ProjectRegistrationFacts::try_new(
            world,
            vec![document],
            vec![ExternalRegistrationFact::new(
                seed,
                RegisteredExternalOwner::Environment(owner),
                declaration,
            )],
            Vec::new(),
        )
        .expect("facts");
        let context = ProjectCompilationContext::new(
            Arc::new(TypeCheckEnv::standard()),
            Arc::new(facts),
            None,
        );
        let mut cache = RecordingCache::default();

        let error = compile_project_with_cache(
            &project,
            &context,
            &RuntimePlanLowerOptions::default(),
            &mut cache,
        )
        .expect_err("unknown character owner rejects project");
        assert_eq!(error.stage(), ProjectCompileStage::Registration.as_str());
        assert_eq!(cache.stores, 0);
        assert!(error.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .diagnostic()
                .code()
                .is_some_and(|code| code.as_str() == "aw.character.registration.unknown_owner")
        }));
    }

    #[test]
    fn registration_failure_discards_project() {
        pending_stores_discard_on_registration_error();
    }
}
