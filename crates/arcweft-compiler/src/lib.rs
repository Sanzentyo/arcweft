//! Source-to-runtime-plan compiler driver for Arcweft.

use arcweft_core::plan::{RuntimePlan, RuntimePureHelperOrigin};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_hir::model::{HirFunction, HirModule};
use arcweft_lang_sema::check::{TypeCheckReport, analyze_types};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::project_index::ProjectSemanticIndex;
use arcweft_lang_sema::resolve::{
    registry_from_hir, registry_from_hir_and_project, validate_hir_references,
};
use arcweft_lang_syntax::ast::items::TypedSyntaxTree;
use arcweft_lang_syntax::lint::{SyntaxLint, SyntaxLintSeverity, lint_id_policy};
use arcweft_lang_syntax::parser::{ParseOptions, SourceDialect, parse_document, parse_source};
use arcweft_lang_syntax::source::ParsedSource;
use arcweft_render_text::LineDisplayCatalog;
pub use arcweft_runtime_plan::flow::{
    RuntimePlanLowerOptions, RuntimePlanLowerReport, RuntimePlanLowerStats,
};
use arcweft_runtime_plan::flow::{
    lower_runtime_plan_with_options, lower_runtime_plan_with_stats,
    lower_runtime_plan_with_stats_and_options,
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
    use arcweft_id::PublicId;
    use arcweft_lang_sema::project_index::{
        EntitySymbol, ProgramHash, ProjectSemanticIndex, SemanticHash,
    };
    use arcweft_lang_sema::types::{EntityKind, EntityType, TypeKind};
    use arcweft_render_text::{RichTextColor, RichTextStyle};
    use arcweft_source::SourceAnchor;

    fn public_id(value: &str) -> PublicId {
        PublicId::try_new(value).expect("valid public id")
    }

    fn project_with_entity(id: &str, kind: EntityKind) -> ProjectSemanticIndex {
        ProjectSemanticIndex::new(ProgramHash::new("program-test")).with_entity(EntitySymbol::new(
            public_id(id),
            EntityType::new(kind, None),
            SourceAnchor::generated(),
            SemanticHash::new(format!("shape.{id}.v1")),
        ))
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
