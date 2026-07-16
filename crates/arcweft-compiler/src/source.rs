use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::parse_source;

use crate::error::{CompileSourceError, ValidateHirError};
use crate::hir::validate_hir_with_env;
use crate::lower::lower_source_runtime_plan_with_typecheck_stats_and_options;
use crate::style::lower_source_view_styles;
use crate::types::CompiledSource;

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
    let style = lower_source_view_styles(&hir, &typecheck_report.style_catalog, parsed.document())?;
    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck_report,
        &arcweft_runtime_plan::flow::RuntimePlanLowerOptions::default(),
    )
    .map_err(CompileSourceError::RuntimePlan)?;
    Ok(CompiledSource {
        plan: report.plan,
        display: report.line_display_catalog,
        hir,
        typecheck_report,
        style,
        runtime_plan_stats: report.stats,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arcweft_lang_hir::lower::lower_to_hir;
    use arcweft_lang_sema::env::TypeCheckEnv;
    use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use super::{compile_source, compile_source_with_env};
    use crate::error::CompileSourceError;
    use crate::hir::validate_hir_with_env;

    #[test]
    fn source_compiler_entrypoints_reject_removed_role_declarations_at_parse() {
        for source in [
            "state GameState {\n    value: i32\n}\n",
            "reducer update(state: GameState, event: GameEvent) -> GameState {\n    state\n}\n",
            "agent @agent.smoke smoke() {\n    Ok(())\n}\n",
        ] {
            assert!(matches!(
                compile_source(source),
                Err(CompileSourceError::Parse(errors)) if !errors.is_empty()
            ));
            assert!(matches!(
                compile_source_with_env(source, &TypeCheckEnv::standard()),
                Err(CompileSourceError::Parse(errors)) if !errors.is_empty()
            ));
        }
    }

    #[test]
    fn arcw_and_awfagent_documents_share_ast_hir_and_sema_results() {
        let source = r"
fn smoke() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

entry agent @entry.agent.main {
    controller = smoke
}
";
        let parse = |id: &str, name: &str| {
            parse_document_with_source(
                Arc::new(
                    SourceDocument::try_new(
                        SourceDocumentId::try_new(id).expect("source ID"),
                        SourceName::path(name),
                        source,
                    )
                    .expect("source document"),
                ),
                ParseOptions::default(),
            )
        };
        let arcw = parse("parity://main.arcw", "main.arcw");
        let awfagent = parse("parity://main.awfagent", "main.awfagent");
        assert_eq!(arcw.errors(), awfagent.errors());
        assert_eq!(arcw.typed_tree(), awfagent.typed_tree());

        let arcw_hir = lower_to_hir(arcw.typed_tree()).expect(".arcw HIR");
        let agent_hir = lower_to_hir(awfagent.typed_tree()).expect(".awfagent HIR");
        assert_eq!(arcw_hir, agent_hir);
        let env = TypeCheckEnv::standard();
        let arcw_sema = validate_hir_with_env(&arcw_hir, &env).expect(".arcw sema");
        let agent_sema = validate_hir_with_env(&agent_hir, &env).expect(".awfagent sema");
        assert_eq!(arcw_sema, agent_sema);
    }
}
