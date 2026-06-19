//! Source-to-runtime-plan compiler driver for Arcweft.

use arcweft_agent_protocol::{
    artifact::{
        AgentArtifactManifest, AgentBudget, AgentBundleKind,
        EffectCapability as AgentEffectCapability, ProjectBinding, ProjectBindingMode,
        RequiredEntity,
    },
    ids::{PublicId as AgentPublicId, StableHash},
};
use arcweft_bundle::{ArcweftBundle, BundleManifest, BundleRuntimeSummary, BundleSource};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::plan::{RuntimePlan, RuntimePureHelperOrigin};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_hir::model::{HirAgent, HirFunction, HirModule};
use arcweft_lang_sema::check::{TypeCheckReport, analyze_types};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::project_index::ProjectSemanticIndex;
use arcweft_lang_sema::resolve::{
    registry_from_hir, registry_from_hir_and_project, validate_hir_references,
};
use arcweft_lang_sema::types::EntityKind;
use arcweft_lang_syntax::ast::flow::ContractClause;
use arcweft_lang_syntax::ast::items::TypedSyntaxTree;
use arcweft_lang_syntax::expr::{CallArg, Expr};
use arcweft_lang_syntax::lint::{SyntaxLint, SyntaxLintSeverity, lint_id_policy};
use arcweft_lang_syntax::parser::{ParseOptions, SourceDialect, parse_document, parse_source};
use arcweft_lang_syntax::source::ParsedSource;
use arcweft_render_text::LineDisplayCatalog;
pub use arcweft_runtime_plan::flow::{
    RuntimePlanLowerOptions, RuntimePlanLowerReport, RuntimePlanLowerStats,
};
use arcweft_runtime_plan::flow::{
    lower_agent_controller_plan_with_stats, lower_runtime_plan_with_options,
    lower_runtime_plan_with_stats, lower_runtime_plan_with_stats_and_options,
};
pub use arcweft_runtime_plan::line_task::LoweredLineTaskGroup;
use arcweft_runtime_plan::line_task::lower_line_task_groups;
pub use arcweft_runtime_plan::pure::{
    PureHelperCandidate, PureHelperCandidateReport, PureHelperLowerError,
};
use arcweft_runtime_plan::pure::{lower_pure_helper_candidate, lower_pure_helper_candidates};
use std::fmt;
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

/// Source compiler diagnostics for the shared driver.
#[derive(Debug, Error)]
pub enum CompileSourceError {
    #[error("parse errors: {0:?}")]
    Parse(Vec<arcweft_lang_syntax::parser::recovery::ParseError>),
    #[error("HIR lowering errors: {0:?}")]
    Hir(Vec<arcweft_lang_hir::model::HirLowerError>),
    #[error("reference resolution errors: {0:?}")]
    Resolve(Vec<arcweft_lang_sema::resolve::NameResolutionError>),
    #[error("type-check readiness errors: {0:?}")]
    Readiness(Vec<arcweft_lang_sema::diagnostics::TypeCheckReadinessError>),
    #[error("type errors: {0:?}")]
    Type(Vec<arcweft_lang_sema::diagnostics::TypeCheckError>),
    #[error("runtime-plan lowering errors: {0:?}")]
    RuntimePlan(Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>),
}

/// Agent controller compiler diagnostics.
#[derive(Debug, Error)]
pub enum CompileAgentError {
    #[error("parse errors: {0:?}")]
    Parse(Vec<arcweft_lang_syntax::parser::recovery::ParseError>),
    #[error("HIR lowering errors: {0:?}")]
    Hir(Vec<arcweft_lang_hir::model::HirLowerError>),
    #[error("reference resolution errors: {0:?}")]
    Resolve(Vec<arcweft_lang_sema::resolve::NameResolutionError>),
    #[error("type-check readiness errors: {0:?}")]
    Readiness(Vec<arcweft_lang_sema::diagnostics::TypeCheckReadinessError>),
    #[error("type errors: {0:?}")]
    Type(Vec<arcweft_lang_sema::diagnostics::TypeCheckError>),
    #[error("agent source did not declare a top-level `agent` item")]
    MissingAgent,
    #[error("agent bundle compilation requires exactly one top-level `agent` item, found {0}")]
    MultipleAgents(usize),
    #[error("agent runtime-plan lowering errors: {0:?}")]
    RuntimePlan(Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>),
    #[error("agent artifact identifier error: {0}")]
    ArtifactIdentifier(#[from] arcweft_agent_protocol::ids::IdentifierError),
}

/// HIR semantic validation diagnostics for the shared compiler driver.
#[derive(Debug, Error)]
pub enum ValidateHirError {
    #[error("reference resolution errors: {0:?}")]
    Resolve(Vec<arcweft_lang_sema::resolve::NameResolutionError>),
    #[error("type-check readiness errors: {0:?}")]
    Readiness(Vec<arcweft_lang_sema::diagnostics::TypeCheckReadinessError>),
    #[error("type errors: {0:?}")]
    Type(Vec<arcweft_lang_sema::diagnostics::TypeCheckError>),
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

/// Parses source text into the shared syntax parser output.
pub fn parse_source_text(source: impl Into<String>) -> ParsedSource {
    parse_source(source)
}

/// Parses Agent dialect source text into the shared syntax parser output.
pub fn parse_agent_source_text(source: impl Into<String>) -> ParsedSource {
    parse_document(
        source,
        ParseOptions {
            source_dialect: SourceDialect::Agent,
        },
    )
}

/// Runs syntax-level source lints on a typed syntax tree.
pub fn lint_source_tree(tree: &TypedSyntaxTree) -> Vec<SyntaxLint> {
    lint_id_policy(tree)
}

/// Counts source lints that should be reported as warnings.
pub fn count_warning_lints(lints: &[SyntaxLint]) -> usize {
    lints
        .iter()
        .filter(|lint| matches!(lint.severity(), SyntaxLintSeverity::Warning))
        .count()
}

/// Returns whether any source lint should stop compilation.
pub fn has_error_lints(lints: &[SyntaxLint]) -> bool {
    lints
        .iter()
        .any(|lint| matches!(lint.severity(), SyntaxLintSeverity::Error))
}

/// Lowers a typed syntax tree into HIR.
pub fn lower_source_tree(
    tree: &TypedSyntaxTree,
) -> Result<HirModule, Vec<arcweft_lang_hir::model::HirLowerError>> {
    lower_to_hir(tree)
}

/// Compiles `.awfagent` source through the shared parser and HIR lowering path.
pub fn compile_agent_source(source: impl Into<String>) -> Result<CompiledAgent, CompileAgentError> {
    let parsed = parse_agent_source_text(source);
    if !parsed.errors().is_empty() {
        return Err(CompileAgentError::Parse(parsed.errors().to_vec()));
    }
    let hir = lower_source_tree(parsed.typed_tree()).map_err(CompileAgentError::Hir)?;
    if hir.agents().is_empty() {
        return Err(CompileAgentError::MissingAgent);
    }
    Ok(CompiledAgent { hir })
}

/// Compiles `.awfagent` source and checks it against a project semantic index.
pub fn compile_agent_source_with_project(
    source: impl Into<String>,
    project: &ProjectSemanticIndex,
) -> Result<TypecheckedAgent, CompileAgentError> {
    let compiled = compile_agent_source(source)?;
    let registry = registry_from_hir_and_project(&compiled.hir, project);
    validate_hir_references(&compiled.hir, &registry).map_err(CompileAgentError::Resolve)?;
    validate_hir_typecheck_ready(&compiled.hir).map_err(CompileAgentError::Readiness)?;
    let typecheck_report = analyze_types(&compiled.hir, &project.typecheck_env());
    typecheck_report
        .clone()
        .into_result()
        .map_err(CompileAgentError::Type)?;
    Ok(TypecheckedAgent {
        hir: compiled.hir,
        typecheck_report,
    })
}

/// Compiles one `.awfagent` controller into an Agent controller `.awfb` bundle
/// data object using the shared runtime-plan and bytecode shapes.
pub fn compile_agent_bundle_with_project(
    source: impl Into<String>,
    project: &ProjectSemanticIndex,
) -> Result<CompiledAgentBundle, CompileAgentError> {
    let source = source.into();
    let parsed = parse_agent_source_text(source.clone());
    if !parsed.errors().is_empty() {
        return Err(CompileAgentError::Parse(parsed.errors().to_vec()));
    }
    let source_hash = parsed.source_hash();
    let hir = lower_source_tree(parsed.typed_tree()).map_err(CompileAgentError::Hir)?;
    let agent = single_agent(&hir)?;
    let registry = registry_from_hir_and_project(&hir, project);
    validate_hir_references(&hir, &registry).map_err(CompileAgentError::Resolve)?;
    validate_hir_typecheck_ready(&hir).map_err(CompileAgentError::Readiness)?;
    let typecheck_report = analyze_types(&hir, &project.typecheck_env());
    typecheck_report
        .clone()
        .into_result()
        .map_err(CompileAgentError::Type)?;
    let runtime_report = lower_agent_controller_plan_with_stats(&hir, agent)
        .map_err(CompileAgentError::RuntimePlan)?;
    let bytecode = BytecodeProgram::from_runtime_plan(runtime_report.plan);
    let bytecode_stats = bytecode.stats();
    let manifest = agent_artifact_manifest(agent, source_hash, project)?;
    let source_label = format!("{}.awfagent", manifest.agent_id.as_str());
    let bundle = ArcweftBundle::new(
        BundleManifest {
            source_label: source_label.clone(),
            profile_id: None,
            profile_kind: None,
            entry: Some(format!("entry.{}", manifest.agent_id.as_str())),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: bytecode.entry_flow.as_ref().map(|flow| flow.0.clone()),
                flows: bytecode_stats.flows,
                bytecode_instructions: bytecode_stats.instructions,
                line_task_groups: bytecode_stats.line_task_groups,
                stream_plans: bytecode_stats.stream_plans,
                source_plans: bytecode_stats.source_plans,
            },
        },
        BundleSource {
            label: source_label,
            text: source,
        },
        bytecode,
        runtime_report.line_display_catalog,
    )
    .with_agent_manifest(manifest.clone());
    Ok(CompiledAgentBundle {
        bundle,
        manifest,
        hir,
        typecheck_report,
        runtime_plan_stats: runtime_report.stats,
    })
}

/// Validates and type-checks HIR with a supplied environment.
pub fn validate_hir_with_env(
    hir: &HirModule,
    env: &TypeCheckEnv,
) -> Result<TypeCheckReport, ValidateHirError> {
    resolve_hir_references(hir).map_err(ValidateHirError::Resolve)?;
    validate_hir_typecheck_ready(hir).map_err(ValidateHirError::Readiness)?;
    typecheck_hir_with_env(hir, env).map_err(ValidateHirError::Type)
}

/// Validates HIR entity references against declarations in the same module.
pub fn resolve_hir_references(
    hir: &HirModule,
) -> Result<(), Vec<arcweft_lang_sema::resolve::NameResolutionError>> {
    let registry = registry_from_hir(hir);
    validate_hir_references(hir, &registry)
}

/// Validates that HIR no longer contains raw syntax fragments.
pub fn validate_hir_typecheck_ready(
    hir: &HirModule,
) -> Result<(), Vec<arcweft_lang_sema::diagnostics::TypeCheckReadinessError>> {
    arcweft_lang_sema::check::validate_typecheck_ready(hir)
}

/// Type-checks HIR and returns the full type-check report.
pub fn typecheck_hir_with_env(
    hir: &HirModule,
    env: &TypeCheckEnv,
) -> Result<TypeCheckReport, Vec<arcweft_lang_sema::diagnostics::TypeCheckError>> {
    let report = analyze_types(hir, env);
    report.clone().into_result()?;
    Ok(report)
}

fn single_agent(hir: &HirModule) -> Result<&HirAgent, CompileAgentError> {
    match hir.agents() {
        [] => Err(CompileAgentError::MissingAgent),
        [agent] => Ok(agent),
        agents => Err(CompileAgentError::MultipleAgents(agents.len())),
    }
}

fn agent_artifact_manifest(
    agent: &HirAgent,
    source_hash: arcweft_lang_syntax::source::SourceHash,
    project: &ProjectSemanticIndex,
) -> Result<AgentArtifactManifest, CompileAgentError> {
    let agent_id = agent_public_id(agent)?;
    Ok(AgentArtifactManifest {
        schema_version: 1,
        bundle_kind: AgentBundleKind::AgentController,
        agent_id,
        source_hash: StableHash::new(format!("blake3:{}", source_hash.to_hex()))?,
        compiler_version: format!("arcweft-compiler/{}", env!("CARGO_PKG_VERSION")),
        project_binding: ProjectBinding {
            program_hash: StableHash::new(project.program_hash().as_str().to_owned())?,
            mode: ProjectBindingMode::Compatible,
            required_entities: project
                .entities()
                .values()
                .map(required_agent_entity)
                .collect::<Result<Vec<_>, _>>()?,
        },
        declared_effects: declared_agent_effects(agent),
        budget: AgentBudget::default(),
        debug_map_hash: None,
    })
}

fn agent_public_id(agent: &HirAgent) -> Result<AgentPublicId, CompileAgentError> {
    AgentPublicId::new(agent.item().id().map_or_else(
        || format!("agent.{}", agent.item().name()),
        |id| id.body().to_owned(),
    ))
    .map_err(CompileAgentError::ArtifactIdentifier)
}

fn required_agent_entity(
    entity: &arcweft_lang_sema::project_index::EntitySymbol,
) -> Result<RequiredEntity, arcweft_agent_protocol::ids::IdentifierError> {
    Ok(RequiredEntity {
        public_id: AgentPublicId::new(entity.id().as_str().to_owned())?,
        kind: entity_kind_label(entity.ty().kind()).to_owned(),
        type_fingerprint: StableHash::new(entity.semantic_hash().as_str().to_owned())?,
    })
}

fn declared_agent_effects(agent: &HirAgent) -> Vec<AgentEffectCapability> {
    let mut effects = agent
        .item()
        .contracts()
        .iter()
        .filter_map(|contract| match contract {
            ContractClause::Effects(effects) => Some(effects),
            _ => None,
        })
        .flat_map(|effects| effects.iter().filter_map(effect_label))
        .map(AgentEffectCapability::new)
        .collect::<Vec<_>>();
    effects.sort();
    effects.dedup();
    effects
}

fn effect_label(expr: &Expr) -> Option<String> {
    if let Expr::Call { callee, args } = expr
        && effect_path_label(callee).as_deref() == Some("state.write")
    {
        return state_write_effect_label(args);
    }
    if let Expr::MethodCall {
        receiver,
        method,
        args,
    } = expr
        && method == "write"
        && effect_path_label(receiver).as_deref() == Some("state")
    {
        return state_write_effect_label(args);
    }
    match expr {
        Expr::MethodCall {
            receiver, method, ..
        } => effect_path_label(receiver).map(|receiver| format!("{receiver}.{method}")),
        Expr::Call { callee, .. } => effect_label(callee),
        _ => effect_path_label(expr),
    }
}

fn effect_path_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => Some(path.clone()),
        Expr::Field { target, field } => {
            effect_path_label(target).map(|target| format!("{target}.{field}"))
        }
        _ => None,
    }
}

fn state_write_effect_label(args: &[CallArg]) -> Option<String> {
    args.first().and_then(|arg| match arg.value() {
        Expr::LifetimePath { key, .. } => Some(format!("state.write({})", key.scope().as_str())),
        Expr::Path(path) => path
            .strip_prefix('\'')
            .map(|scope| format!("state.write({scope})")),
        _ => None,
    })
}

fn entity_kind_label(kind: &EntityKind) -> &str {
    match kind {
        EntityKind::Agent => "agent",
        EntityKind::Entry => "entry",
        EntityKind::Flow => "flow",
        EntityKind::Fragment => "fragment",
        EntityKind::Choice => "choice",
        EntityKind::ChoiceOption => "choice_option",
        EntityKind::Character => "character",
        EntityKind::Component => "component",
        EntityKind::Activity => "activity",
        EntityKind::Textbox => "textbox",
        EntityKind::DialogueLine => "dialogue_line",
        EntityKind::Text => "text",
        EntityKind::Asset => "asset",
        EntityKind::Image => "image",
        EntityKind::Animation => "animation",
        EntityKind::Capture => "capture",
        EntityKind::Hook => "hook",
        EntityKind::Signal => "signal",
        EntityKind::Metric => "metric",
        EntityKind::Scene => "scene",
        EntityKind::Source => "source",
        EntityKind::Test => "test",
        EntityKind::Bench => "bench",
        EntityKind::Layer => "layer",
        EntityKind::Voice => "voice",
        EntityKind::Se => "se",
        EntityKind::Bgm => "bgm",
        EntityKind::AudioBus => "audio_bus",
        EntityKind::MixerSnapshot => "mixer_snapshot",
        EntityKind::Ducking => "ducking",
        EntityKind::Motion => "motion",
        EntityKind::Rig => "rig",
        EntityKind::Slot => "slot",
        EntityKind::Target => "target",
        EntityKind::Other(value) => value.as_str(),
    }
}

/// Lowers dialogue line plans from HIR into runtime task groups.
pub fn lower_source_line_tasks(
    hir: &HirModule,
) -> Result<Vec<LoweredLineTaskGroup>, Vec<arcweft_runtime_plan::errors::LinePlanLowerError>> {
    lower_line_task_groups(hir)
}

/// Lowers checked HIR into a runtime plan with explicit profile/build-context options.
pub fn lower_source_runtime_plan_with_options(
    hir: &HirModule,
    options: &RuntimePlanLowerOptions,
) -> Result<RuntimePlan, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>> {
    lower_runtime_plan_with_options(hir, options)
}

/// Lowers checked HIR into a runtime plan and display catalog with compiler counters.
pub fn lower_source_runtime_plan_with_stats_and_options(
    hir: &HirModule,
    options: &RuntimePlanLowerOptions,
) -> Result<RuntimePlanLowerReport, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>> {
    lower_runtime_plan_with_stats_and_options(hir, options)
}

/// Lowers pure helper candidates from checked HIR.
pub fn lower_source_pure_helper_candidates(
    hir: &HirModule,
) -> Result<PureHelperCandidateReport, Vec<PureHelperLowerError>> {
    lower_pure_helper_candidates(hir)
}

/// Lowers one checked pure function into a runtime helper candidate.
pub fn lower_source_pure_helper_candidate(
    function: &HirFunction,
    origin: RuntimePureHelperOrigin,
) -> Result<PureHelperCandidate, PureHelperLowerError> {
    lower_pure_helper_candidate(function, origin)
}

/// Lowers checked HIR functions annotated for native text shader/effect/motion registries.
pub fn lower_source_text_pure_helper_candidates(
    hir: &HirModule,
) -> Result<TextPureHelperCandidateReport, Vec<TextPureHelperCandidateError>> {
    let mut report = TextPureHelperCandidateReport::default();
    let mut errors = Vec::new();
    for function in hir.functions() {
        for kind in TextPureHelperKind::from_function(function) {
            if !function.has_attribute("pure") {
                errors.push(TextPureHelperCandidateError::MissingPureAttribute {
                    kind,
                    name: function.name().to_owned(),
                });
                continue;
            }
            match lower_source_pure_helper_candidate(function, RuntimePureHelperOrigin::Annotated) {
                Ok(candidate) => report.push(kind, candidate),
                Err(source) => {
                    errors.push(TextPureHelperCandidateError::PureLower { kind, source });
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(report)
    } else {
        Err(errors)
    }
}

/// Compiles an Arcweft source string with the standard type-checking environment.
pub fn compile_source(source: &str) -> Result<CompiledSource, CompileSourceError> {
    compile_source_with_env(source, &TypeCheckEnv::standard())
}

/// Compiles an Arcweft source string with a supplied type-checking environment.
pub fn compile_source_with_env(
    source: &str,
    env: &TypeCheckEnv,
) -> Result<CompiledSource, CompileSourceError> {
    let parsed = parse_source(source.to_owned());
    if !parsed.errors().is_empty() {
        return Err(CompileSourceError::Parse(parsed.errors().to_vec()));
    }
    let hir = lower_to_hir(parsed.typed_tree()).map_err(CompileSourceError::Hir)?;
    let typecheck_report = validate_hir_with_env(&hir, env).map_err(|error| match error {
        ValidateHirError::Resolve(errors) => CompileSourceError::Resolve(errors),
        ValidateHirError::Readiness(errors) => CompileSourceError::Readiness(errors),
        ValidateHirError::Type(errors) => CompileSourceError::Type(errors),
    })?;
    let report = lower_runtime_plan_with_stats(&hir).map_err(CompileSourceError::RuntimePlan)?;
    Ok(CompiledSource {
        plan: report.plan,
        display: report.line_display_catalog,
        hir,
        typecheck_report,
        runtime_plan_stats: report.stats,
    })
}

impl TextPureHelperKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Shader => "shader",
            Self::Effect => "effect",
            Self::Motion => "motion",
        }
    }

    fn from_function(function: &HirFunction) -> impl Iterator<Item = Self> + '_ {
        [
            (
                Self::Shader,
                function.has_attribute("text_shader") || function.has_attribute("rich_text_shader"),
            ),
            (
                Self::Effect,
                function.has_attribute("text_effect") || function.has_attribute("rich_text_effect"),
            ),
            (
                Self::Motion,
                function.has_attribute("text_motion") || function.has_attribute("rich_text_motion"),
            ),
        ]
        .into_iter()
        .filter_map(|(kind, selected)| selected.then_some(kind))
    }
}

impl fmt::Display for TextPureHelperKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TextPureHelperCandidateReport {
    fn push(&mut self, kind: TextPureHelperKind, candidate: PureHelperCandidate) {
        match kind {
            TextPureHelperKind::Shader => self.shaders.push(candidate),
            TextPureHelperKind::Effect => self.effects.push(candidate),
            TextPureHelperKind::Motion => self.motions.push(candidate),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::BundleKind;
    use arcweft_id::PublicId;
    use arcweft_lang_sema::project_index::{
        AgentActionParam, AgentActionSignature, EntitySymbol, ProgramHash, ProjectSemanticIndex,
        QualifiedName, SemanticHash,
    };
    use arcweft_lang_sema::types::{EntityKind, EntityType, TypeKind};
    use arcweft_render_text::{RichTextColor, RichTextStyle};
    use arcweft_source::SourceAnchor;

    fn public_id(value: &str) -> PublicId {
        PublicId::try_new(value).expect("valid public id")
    }

    fn project_with_entity(id: &str, kind: EntityKind) -> ProjectSemanticIndex {
        project_with_typed_entity(id, kind, None)
    }

    fn project_with_typed_entity(
        id: &str,
        kind: EntityKind,
        value: Option<TypeKind>,
    ) -> ProjectSemanticIndex {
        ProjectSemanticIndex::new(ProgramHash::new("program-test")).with_entity(EntitySymbol::new(
            public_id(id),
            EntityType::new(kind, value),
            SourceAnchor::generated(),
            SemanticHash::new(format!("shape.{id}.v1")),
        ))
    }

    fn project_with_agent_action(
        id: &str,
        kind: EntityKind,
        action: &str,
        params: impl IntoIterator<Item = AgentActionParam>,
    ) -> ProjectSemanticIndex {
        ProjectSemanticIndex::new(ProgramHash::new("program-test")).with_entity(
            EntitySymbol::new(
                public_id(id),
                EntityType::new(kind, None),
                SourceAnchor::generated(),
                SemanticHash::new(format!("shape.{id}.v1")),
            )
            .with_agent_action(AgentActionSignature::new(
                QualifiedName::new(action),
                params,
                TypeKind::ActionResult,
            )),
        )
    }

    #[test]
    fn compiles_dialogue_source_to_plan_and_display_catalog() {
        let source = r"
character @character.alice Alice as alice {}

entry game @entry.main {
    start(@flow.main)
}

flow @flow.main main {
    alice: Hello
}
";

        let compiled = compile_source(source).expect("source compiles");

        assert!(!compiled.plan.entries.is_empty());
        assert!(!compiled.display.lines().is_empty());
    }

    #[test]
    fn compiles_agent_source_through_agent_dialect() {
        let compiled = compile_agent_source(
            r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe }
{
    observe()
}
",
        )
        .expect("agent source compiles");

        assert_eq!(compiled.hir.agents().len(), 1);
        assert_eq!(compiled.hir.agents()[0].item().name(), "opening_smoke");
    }

    #[test]
    fn compile_agent_source_rejects_legacy_line_commands() {
        let error = compile_agent_source("observe\n").expect_err("legacy command fails");

        assert!(matches!(error, CompileAgentError::Parse(_)));
    }

    #[test]
    fn compile_agent_source_with_project_checks_choose_intrinsic() {
        let project = project_with_entity("choice.opening.listen", EntityKind::ChoiceOption);
        let compiled = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.act.semantic }
{
    choose(@choice.opening.listen)
}
",
            &project,
        )
        .expect("agent source typechecks against project index");

        assert_eq!(compiled.hir.agents().len(), 1);
        assert!(compiled.typecheck_report.diagnostics.is_empty());
        assert!(
            compiled
                .typecheck_report
                .judgments
                .iter()
                .any(|judgment| judgment.ty == TypeKind::ActionResult)
        );
    }

    #[test]
    fn compile_agent_source_with_project_rejects_choose_family_mismatch() {
        let project = project_with_entity("flow.main", EntityKind::Flow);
        let error = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.act.semantic }
{
    choose(@flow.main)
}
",
            &project,
        )
        .expect_err("flow is not a choice option");

        assert!(matches!(error, CompileAgentError::Type(_)));
    }

    #[test]
    fn compile_agent_source_with_project_rejects_unresolved_project_entity() {
        let project = ProjectSemanticIndex::new(ProgramHash::new("program-test"));
        let error = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.act.semantic }
{
    choose(@choice.opening.listen)
}
",
            &project,
        )
        .expect_err("missing project entity");

        assert!(matches!(error, CompileAgentError::Resolve(_)));
    }

    #[test]
    fn compile_agent_source_with_project_checks_signal_probe_wait() {
        let project =
            project_with_typed_entity("signal.ready", EntityKind::Signal, Some(TypeKind::Bool));
        let compiled = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe, agent.wait }
{
    wait(signal(@signal.ready).eq(true), timeout = 5s)
}
",
            &project,
        )
        .expect("signal probe and wait typecheck");

        assert!(compiled.typecheck_report.diagnostics.is_empty());
        assert!(
            compiled
                .typecheck_report
                .judgments
                .iter()
                .any(|judgment| judgment.ty == TypeKind::Predicate)
        );
    }

    #[test]
    fn compile_agent_source_with_project_checks_statement_wait_entity_probe() {
        let project = project_with_typed_entity(
            "signal.current_flow",
            EntityKind::Signal,
            Some(TypeKind::entity_ref(EntityKind::Flow)),
        )
        .with_entity(EntitySymbol::new(
            public_id("flow.opening"),
            EntityType::new(EntityKind::Flow, None),
            SourceAnchor::generated(),
            SemanticHash::new("shape.flow.opening.v1"),
        ));
        let compiled = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe, agent.wait }
{
    wait(signal(@signal.current_flow).eq(@flow.opening), timeout = 5s, stable_frames = 1u32, poll_frames = 1u32)
}
",
            &project,
        )
        .expect("statement-form wait lowers to typed Agent intrinsic");

        assert!(compiled.typecheck_report.diagnostics.is_empty());
        assert!(
            compiled
                .typecheck_report
                .judgments
                .iter()
                .any(|judgment| judgment.ty == TypeKind::Predicate)
        );
    }

    #[test]
    fn compile_agent_source_with_project_rejects_signal_payload_mismatch() {
        let project =
            project_with_typed_entity("signal.ready", EntityKind::Signal, Some(TypeKind::Bool));
        let error = compile_agent_source_with_project(
            r#"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe }
{
    signal(@signal.ready).eq("yes")
}
"#,
            &project,
        )
        .expect_err("signal bool payload rejects string comparison");

        assert!(matches!(error, CompileAgentError::Type(_)));
    }

    #[test]
    fn compile_agent_source_with_project_rejects_wait_without_timeout() {
        let project =
            project_with_typed_entity("signal.ready", EntityKind::Signal, Some(TypeKind::Bool));
        let error = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe, agent.wait }
{
    wait(signal(@signal.ready).eq(true))
}
",
            &project,
        )
        .expect_err("wait requires timeout");

        assert!(matches!(error, CompileAgentError::Type(_)));
    }

    #[test]
    fn compile_agent_source_with_project_checks_metric_probe() {
        let project =
            project_with_typed_entity("metric.fps", EntityKind::Metric, Some(TypeKind::F32));
        let compiled = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.perf_watch perf_watch()
effects { agent.observe }
{
    metric(@metric.fps).eq(60.0f32)
}
",
            &project,
        )
        .expect("metric probe typechecks");

        assert!(compiled.typecheck_report.diagnostics.is_empty());
    }

    #[test]
    fn compile_agent_source_with_project_checks_composite_predicates() {
        let project =
            project_with_typed_entity("signal.ready", EntityKind::Signal, Some(TypeKind::Bool))
                .with_entity(EntitySymbol::new(
                    public_id("metric.fps"),
                    EntityType::new(EntityKind::Metric, Some(TypeKind::F32)),
                    SourceAnchor::generated(),
                    SemanticHash::new("shape.metric.fps.v1"),
                ));
        let compiled = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.composite_wait composite_wait()
effects { agent.observe, agent.wait }
{
    wait(all(signal(@signal.ready).eq(true), not(metric(@metric.fps).lt(30.0f32))), timeout = 5s)
}
",
            &project,
        )
        .expect("composite predicate typechecks");

        assert!(compiled.typecheck_report.diagnostics.is_empty());
        assert!(
            compiled
                .typecheck_report
                .judgments
                .iter()
                .filter(|judgment| judgment.ty == TypeKind::Predicate)
                .count()
                >= 3
        );
    }

    #[test]
    fn compile_agent_source_with_project_checks_state_and_observation_probes() {
        let project = ProjectSemanticIndex::new(ProgramHash::new("program-a"));
        let compiled = compile_agent_source_with_project(
            r#"
#[agent(version = 1)]
agent @agent.debug_state debug_state()
effects { agent.observe, agent.wait, debug.read }
{
    wait(
        all(state("route.phase").eq("opening"), observation("tick").ge(1i64)),
        timeout = 5s,
    )
}
"#,
            &project,
        )
        .expect("state and observation probes typecheck");

        assert!(compiled.typecheck_report.diagnostics.is_empty());
        assert!(
            compiled
                .typecheck_report
                .judgments
                .iter()
                .any(|judgment| judgment.ty == TypeKind::Probe(Box::new(TypeKind::AgentValue)))
        );
    }

    #[test]
    fn compile_agent_source_with_project_rejects_wait_zero_stable() {
        let project =
            project_with_typed_entity("signal.ready", EntityKind::Signal, Some(TypeKind::Bool));
        let error = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe, agent.wait }
{
    wait(signal(@signal.ready).eq(true), timeout = 5s, stable_frames = 0u32)
}
",
            &project,
        )
        .expect_err("wait stable must be positive");

        assert!(matches!(error, CompileAgentError::Type(_)));
    }

    #[test]
    fn compile_agent_source_with_project_checks_invoke_intrinsic() {
        let project = project_with_agent_action(
            "activity.inventory",
            EntityKind::Activity,
            "open",
            [AgentActionParam::required("label", TypeKind::String)],
        );
        let compiled = compile_agent_source_with_project(
            r#"
#[agent(version = 1)]
agent @agent.open_inventory open_inventory()
effects { agent.act.semantic }
{
    invoke(@activity.inventory, .open, { label = "main" })
}
"#,
            &project,
        )
        .expect("invoke intrinsic typechecks against project action signature");

        assert!(compiled.typecheck_report.diagnostics.is_empty());
        assert!(
            compiled
                .typecheck_report
                .judgments
                .iter()
                .any(|judgment| judgment.ty == TypeKind::ActionResult)
        );
    }

    #[test]
    fn compile_agent_source_with_project_rejects_unknown_invoke_action() {
        let project =
            project_with_agent_action("activity.inventory", EntityKind::Activity, "open", []);
        let error = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.open_inventory open_inventory()
effects { agent.act.semantic }
{
    invoke(@activity.inventory, .close)
}
",
            &project,
        )
        .expect_err("unknown action rejects");

        assert!(matches!(error, CompileAgentError::Type(_)));
    }

    #[test]
    fn compile_agent_source_with_project_rejects_unknown_invoke_arg() {
        let project = project_with_agent_action(
            "activity.inventory",
            EntityKind::Activity,
            "open",
            [AgentActionParam::required("label", TypeKind::String)],
        );
        let error = compile_agent_source_with_project(
            r#"
#[agent(version = 1)]
agent @agent.open_inventory open_inventory()
effects { agent.act.semantic }
{
    invoke(@activity.inventory, .open, { title = "main" })
}
"#,
            &project,
        )
        .expect_err("unknown invoke arg rejects");

        assert!(matches!(error, CompileAgentError::Type(_)));
    }

    #[test]
    fn compile_agent_source_with_project_rejects_missing_invoke_arg() {
        let project = project_with_agent_action(
            "activity.inventory",
            EntityKind::Activity,
            "open",
            [AgentActionParam::required("label", TypeKind::String)],
        );
        let error = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.open_inventory open_inventory()
effects { agent.act.semantic }
{
    invoke(@activity.inventory, .open)
}
",
            &project,
        )
        .expect_err("missing required invoke arg rejects");

        assert!(matches!(error, CompileAgentError::Type(_)));
    }

    #[test]
    fn compile_agent_source_with_project_rejects_invoke_arg_type_mismatch() {
        let project = project_with_agent_action(
            "activity.inventory",
            EntityKind::Activity,
            "open",
            [AgentActionParam::required("index", TypeKind::U32)],
        );
        let error = compile_agent_source_with_project(
            r#"
#[agent(version = 1)]
agent @agent.open_inventory open_inventory()
effects { agent.act.semantic }
{
    invoke(@activity.inventory, .open, { index = "main" })
}
"#,
            &project,
        )
        .expect_err("invoke arg type mismatch rejects");

        assert!(matches!(error, CompileAgentError::Type(_)));
    }

    #[test]
    fn compile_agent_source_with_project_checks_capture_and_debug_record() {
        let project = project_with_entity("layer.hud", EntityKind::Layer);
        let compiled = compile_agent_source_with_project(
            r#"
#[agent(version = 1)]
agent @agent.capture_hud capture_hud()
effects { agent.capture, debug.record }
{
    let shot = capture(layer(@layer.hud), format = .png, name = "hud")
    attach(shot)
    checkpoint("after-capture")
    note(fmt("captured"))
}
"#,
            &project,
        )
        .expect("capture and debug record intrinsics typecheck");

        assert!(compiled.typecheck_report.diagnostics.is_empty());
    }

    #[test]
    fn compile_agent_source_with_project_rejects_capture_without_effect() {
        let error = compile_agent_source_with_project(
            r"
#[agent(version = 1)]
agent @agent.capture_view capture_view()
{
    capture(viewport())
}
",
            &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
        )
        .expect_err("capture requires declared effect");

        assert!(matches!(error, CompileAgentError::Type(_)));
    }

    #[test]
    fn compile_agent_source_with_project_checks_rag_query() {
        let project = project_with_entity("choice.opening.listen", EntityKind::ChoiceOption);
        let compiled = compile_agent_source_with_project(
            r#"
#[agent(version = 1)]
agent @agent.debug_context debug_context()
effects { rag.query }
{
    rag.query(
        "opening choice recent failures",
        roots = [@choice.opening.listen],
        graph_depth = 2u32,
        limit = 8usize,
    )
}
"#,
            &project,
        )
        .expect("rag.query intrinsic typechecks");

        assert!(compiled.typecheck_report.diagnostics.is_empty());
        assert!(
            compiled
                .typecheck_report
                .judgments
                .iter()
                .any(|judgment| judgment.ty == TypeKind::RagContextPack)
        );
    }

    #[test]
    fn compile_agent_source_with_project_rejects_rag_query_without_effect() {
        let error = compile_agent_source_with_project(
            r#"
#[agent(version = 1)]
agent @agent.debug_context debug_context()
{
    rag.query("opening choice recent failures")
}
"#,
            &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
        )
        .expect_err("rag.query requires declared effect");

        assert!(matches!(error, CompileAgentError::Type(_)));
    }

    #[test]
    fn compile_agent_bundle_with_project_builds_agent_controller_bundle() {
        let compiled = compile_agent_bundle_with_project(
            r"
#[agent(version = 1)]
agent @agent.observe_smoke observe_smoke()
effects { agent.observe }
{
    observe()
}
",
            &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
        )
        .expect("agent bundle compiles");

        assert_eq!(compiled.bundle.bundle_kind, BundleKind::AgentController);
        assert_eq!(compiled.manifest.agent_id.as_str(), "agent.observe_smoke");
        assert_eq!(
            compiled
                .manifest
                .declared_effects
                .iter()
                .map(AgentEffectCapability::as_str)
                .collect::<Vec<_>>(),
            vec!["agent.observe"]
        );
        assert_eq!(compiled.bundle.bytecode.program.flows.len(), 1);
        assert!(
            compiled.bundle.manifest.runtime.bytecode_instructions > 0,
            "Agent body should lower into bytecode operations"
        );

        let bytes = compiled.bundle.to_json_bytes().expect("bundle encodes");
        let decoded =
            arcweft_bundle::ArcweftBundle::from_json_slice(&bytes).expect("bundle decodes");

        assert_eq!(decoded.bundle_kind, BundleKind::AgentController);
        assert_eq!(
            decoded.agent.as_ref().map(|agent| agent.agent_id.as_str()),
            Some("agent.observe_smoke")
        );
    }

    #[test]
    fn lower_source_runtime_plan_with_options_applies_dialogue_defaults_profile() {
        let parsed = parse_source_text(
            r##"
pub dialogue defaults @dialogue.defaults {
    text_color = rgb("#101112")
}

pub dialogue defaults @dialogue:.defaults.mobile {
    text_color = rgb("#202122")
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Hello[p]
}
"##,
        );
        let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
        validate_hir_with_env(&hir, &TypeCheckEnv::standard()).expect("fixture typechecks");

        let report = lower_source_runtime_plan_with_stats_and_options(
            &hir,
            &RuntimePlanLowerOptions::default().with_dialogue_defaults("dialogue.defaults.mobile"),
        )
        .expect("runtime plan lowers with selected dialogue defaults");
        let spec = report
            .line_display_catalog
            .lines()
            .first()
            .expect("line display spec");

        assert_eq!(
            spec.base_styles,
            vec![RichTextStyle::Color {
                value: RichTextColor::Rgb {
                    red: 32,
                    green: 33,
                    blue: 34
                }
            }]
        );
    }

    #[test]
    fn lower_source_text_pure_helper_candidates_classifies_renderer_extensions() {
        let parsed = parse_source_text(
            r"
#[text_shader]
#[pure]
fn glow(t: f32, glyph: f32, seed: f32) -> f32 {
    return t + glyph + seed
}

#[text_effect]
#[pure]
fn jitter(t: f32, glyph: f32, seed: f32) -> f32 {
    return t - glyph + seed
}

#[text_motion]
#[pure]
fn orbit(t: f32, glyph: f32, seed: f32) -> f32 {
    return t + glyph * seed
}
",
        );
        let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");

        let report =
            lower_source_text_pure_helper_candidates(&hir).expect("text renderer helpers lower");

        assert_eq!(report.shaders[0].name(), "glow");
        assert_eq!(report.effects[0].name(), "jitter");
        assert_eq!(report.motions[0].name(), "orbit");
    }

    #[test]
    fn lower_source_text_pure_helper_candidates_rejects_unpure_exports() {
        let parsed = parse_source_text(
            r"
#[text_effect]
fn drift(t: f32, glyph: f32, seed: f32) -> f32 {
    return t + glyph + seed
}
",
        );
        let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");

        assert_eq!(
            lower_source_text_pure_helper_candidates(&hir),
            Err(vec![TextPureHelperCandidateError::MissingPureAttribute {
                kind: TextPureHelperKind::Effect,
                name: "drift".to_owned(),
            }])
        );
    }
}
