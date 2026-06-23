//! Multi-module project compilation driver.
//!
//! Source loading stays in `arcweft-project-loader`. This module consumes a
//! Sans I/O `ProjectSources` value, compiles SCC-based units independently
//! through syntax/HIR, and retains a module-preserving `HirProject` alongside
//! the transitional crate-global semantic-pass view.

use crate::{hir, lower, parse};
use arcweft_lang_hir::{
    model::HirModule,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_sema::{check::TypeCheckReport, env::TypeCheckEnv};
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
use std::{collections::BTreeMap, fmt::Write as _};
use thiserror::Error;

/// Stable project compilation phase used by diagnostics and profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectCompileStage {
    Parse,
    Lint,
    HirLower,
    HirProject,
    Resolve,
    Readiness,
    TypeCheck,
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
    messages: Vec<String>,
}

/// Independently parsed and lowered source module.
#[derive(Clone, Debug)]
pub struct CompiledProjectModule {
    module: CanonicalModulePath,
    compile_unit: CompileUnitId,
    source_hash: ModuleSourceHash,
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
    linked_hir: HirModule,
    typecheck_report: TypeCheckReport,
    line_task_groups: Vec<LoweredLineTaskGroup>,
    runtime_plan: RuntimePlanLowerReport,
}

/// In-process cache boundary for independently lowered compile units.
///
/// A persistent cache adapter should store a stable serialized unit format, not
/// `HirModule` directly. This trait deliberately covers only the current
/// in-process vertical slice.
pub trait ProjectCompileCache {
    fn load(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
    ) -> Option<Vec<CompiledProjectModule>>;

    fn store(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
        modules: &[CompiledProjectModule],
    );
}

/// No-op cache used by the simple compiler entry point.
#[derive(Default)]
pub struct NoProjectCompileCache;

/// Deterministic in-memory unit cache for watch mode and tests.
#[derive(Default)]
pub struct InMemoryProjectCompileCache {
    units: BTreeMap<ProjectCompileUnitFingerprint, Vec<CompiledProjectModule>>,
}

/// Project compilation failure.
#[derive(Debug, Error)]
#[error("Arcweft project compilation failed during {stage}: {messages:?}")]
pub struct ProjectCompileError {
    stage: &'static str,
    diagnostics: Vec<ProjectCompileDiagnostic>,
    messages: Vec<String>,
}

impl ProjectCompileStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Lint => "lint",
            Self::HirLower => "hir-lower",
            Self::HirProject => "hir-project",
            Self::Resolve => "resolve",
            Self::Readiness => "readiness",
            Self::TypeCheck => "type-check",
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

    pub fn messages(&self) -> &[String] {
        &self.messages
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

    pub const fn linked_hir(&self) -> &HirModule {
        &self.linked_hir
    }

    pub const fn typecheck_report(&self) -> &TypeCheckReport {
        &self.typecheck_report
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

impl ProjectCompileCache for NoProjectCompileCache {
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
    }
}

impl ProjectCompileCache for InMemoryProjectCompileCache {
    fn load(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
    ) -> Option<Vec<CompiledProjectModule>> {
        self.units.get(&fingerprint).cloned()
    }

    fn store(
        &mut self,
        fingerprint: ProjectCompileUnitFingerprint,
        modules: &[CompiledProjectModule],
    ) {
        self.units.insert(fingerprint, modules.to_vec());
    }
}

/// Compiles a project without retaining reusable unit artifacts.
pub fn compile_project_with_env(
    project: &ProjectSources,
    env: &TypeCheckEnv,
    runtime_options: &RuntimePlanLowerOptions,
) -> Result<CompiledProject, ProjectCompileError> {
    compile_project_with_cache(project, env, runtime_options, &mut NoProjectCompileCache)
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
pub fn compile_project_with_cache<C>(
    project: &ProjectSources,
    env: &TypeCheckEnv,
    runtime_options: &RuntimePlanLowerOptions,
    cache: &mut C,
) -> Result<CompiledProject, ProjectCompileError>
where
    C: ProjectCompileCache,
{
    let fingerprints = build_unit_fingerprints(project);
    let incremental = project.manifest().build().incremental();
    let mut modules = Vec::with_capacity(project.modules().len());
    let mut summaries = Vec::with_capacity(project.graph().compile_units().len());

    for &unit_id in project.graph().compile_order() {
        let unit = project.graph().compile_unit(unit_id);
        let fingerprint = fingerprints[&unit_id];
        let cached = incremental
            .then(|| cache.load(fingerprint))
            .flatten()
            .filter(|cached| cached_unit_matches(project, unit_id, cached));
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
                    compile_module(source, unit_id)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if incremental {
                cache.store(fingerprint, &compiled);
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

    let hir_project = HirProject::new(
        modules
            .iter()
            .map(|module| HirProjectModule::new(module.module.clone(), module.hir.clone())),
    )
    .map_err(|error| linked_error(ProjectCompileStage::HirProject, [error.to_string()]))?;
    let linked_hir = hir_project.linked_module();
    hir::resolve_hir_references(&linked_hir).map_err(|errors| {
        linked_error(
            ProjectCompileStage::Resolve,
            errors.into_iter().map(|error| error.to_string()),
        )
    })?;
    hir::validate_hir_typecheck_ready(&linked_hir).map_err(|errors| {
        linked_error(
            ProjectCompileStage::Readiness,
            errors.into_iter().map(|error| error.message().to_owned()),
        )
    })?;
    let typecheck_report = hir::typecheck_hir_with_env(&linked_hir, env).map_err(|errors| {
        linked_error(
            ProjectCompileStage::TypeCheck,
            errors.into_iter().map(|error| error.message().to_owned()),
        )
    })?;
    let line_task_groups = lower::lower_source_line_tasks(&linked_hir).map_err(|errors| {
        linked_error(
            ProjectCompileStage::LineTaskLower,
            errors.into_iter().map(|error| error.message().to_owned()),
        )
    })?;
    let runtime_plan =
        lower::lower_source_runtime_plan_with_stats_and_options(&linked_hir, runtime_options)
            .map_err(|errors| {
                linked_error(
                    ProjectCompileStage::RuntimePlanLower,
                    errors.into_iter().map(|error| error.message().to_owned()),
                )
            })?;

    Ok(CompiledProject {
        modules,
        units: summaries,
        hir_project,
        linked_hir,
        typecheck_report,
        line_task_groups,
        runtime_plan,
    })
}

fn compile_module(
    source: &ProjectSourceFile,
    compile_unit: CompileUnitId,
) -> Result<CompiledProjectModule, ProjectCompileError> {
    let parsed = parse::parse_source_text(source.source().to_owned());
    if !parsed.errors().is_empty() {
        return Err(module_error(
            source.module().clone(),
            ProjectCompileStage::Parse,
            parsed
                .errors()
                .iter()
                .map(|error| error.message().to_owned()),
        ));
    }
    let syntax_stats = parsed.syntax_stats();
    let tree = parsed.into_typed_tree();
    let lints = parse::lint_source_tree(&tree);
    if parse::has_error_lints(&lints) {
        return Err(module_error(
            source.module().clone(),
            ProjectCompileStage::Lint,
            lints
                .iter()
                .filter(|lint| lint.severity() == SyntaxLintSeverity::Error)
                .map(|lint| lint.message().to_owned()),
        ));
    }
    let syntax_warnings = parse::count_warning_lints(&lints);
    let hir = hir::lower_source_tree(&tree).map_err(|errors| {
        module_error(
            source.module().clone(),
            ProjectCompileStage::HirLower,
            errors.into_iter().map(|error| error.message().to_owned()),
        )
    })?;
    Ok(CompiledProjectModule {
        module: source.module().clone(),
        compile_unit,
        source_hash: source.source_hash(),
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
    unit_id: CompileUnitId,
    cached: &[CompiledProjectModule],
) -> bool {
    let unit = project.graph().compile_unit(unit_id);
    cached.len() == unit.modules().len()
        && cached.iter().zip(unit.modules()).all(|(cached, expected)| {
            cached.module() == expected
                && cached.compile_unit() == unit_id
                && project
                    .module(expected)
                    .is_some_and(|source| source.source_hash() == cached.source_hash())
        })
}

fn module_error(
    module: CanonicalModulePath,
    stage: ProjectCompileStage,
    messages: impl IntoIterator<Item = String>,
) -> ProjectCompileError {
    let messages = messages.into_iter().collect::<Vec<_>>();
    ProjectCompileError {
        stage: stage.as_str(),
        diagnostics: vec![ProjectCompileDiagnostic {
            module: Some(module),
            stage,
            messages: messages.clone(),
        }],
        messages,
    }
}

fn linked_error(
    stage: ProjectCompileStage,
    messages: impl IntoIterator<Item = String>,
) -> ProjectCompileError {
    let messages = messages.into_iter().collect::<Vec<_>>();
    ProjectCompileError {
        stage: stage.as_str(),
        diagnostics: vec![ProjectCompileDiagnostic {
            module: None,
            stage,
            messages: messages.clone(),
        }],
        messages,
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
