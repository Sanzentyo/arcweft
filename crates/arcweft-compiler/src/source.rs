use std::{path::PathBuf, sync::Arc};

use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::error::CompileSourceError;
use crate::project::{ProjectCompilationContext, compile_project};
use crate::types::CompiledSource;

const SOURCE_PACKAGE: &str = "local.arcweft.single-source";

/// Compiles an Arcweft source string through the standard project compiler.
pub fn compile_source(source: &str) -> Result<CompiledSource, CompileSourceError> {
    compile_source_with_env(source, &TypeCheckEnv::standard())
}

/// Compiles one in-memory module through the same View/profile admission and
/// runtime-plan path as an ordinary project.
///
/// # Panics
///
/// Panics only if the compiler-owned single-source package constants or the
/// internally constructed one-root project violate their static invariants.
pub fn compile_source_with_env(
    source: &str,
    env: &TypeCheckEnv,
) -> Result<CompiledSource, CompileSourceError> {
    let document = source_document(source);
    let manifest = manifest_document();
    let package = PackageSpec {
        id: PackageId::new(SOURCE_PACKAGE).expect("single-source package ID is valid"),
        version: PackageVersion::new("0.0.0").expect("single-source package version is valid"),
    };
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        package,
        BuildSpec::default(),
        Arc::clone(&manifest),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            PathBuf::from("src/main.arcw"),
            Arc::clone(&document),
            [],
        )],
    )
    .expect("one canonical root module forms a valid project source inventory");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(SOURCE_PACKAGE)
            .expect("single-source callable package ID is valid"),
        document.identity().id().clone(),
        "single-source",
    )
    .expect("single-source symbol world is valid");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&manifest), Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
    )
    .expect("single-source documents form coherent registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(env.clone()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
        Vec::new(),
    );
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())?;
    let report = compiled.runtime_plan();
    Ok(CompiledSource {
        plan: report.plan.clone(),
        display: report.line_display_catalog.clone(),
        hir: compiled.linked_hir().clone(),
        typecheck_report: compiled.typecheck_report().clone(),
        style: compiled.style().clone(),
        fx_definitions: Arc::from(compiled.fx_definitions()),
        runtime_plan_stats: report.stats,
    })
}

fn source_document(source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-source://src/main.arcw")
                .expect("single-source document ID is valid"),
            SourceName::path("src/main.arcw"),
            source,
        )
        .expect("an in-memory Rust string is a valid source document"),
    )
}

fn manifest_document() -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-source://arcw.toml")
                .expect("single-source manifest ID is valid"),
            SourceName::path("arcw.toml"),
            "",
        )
        .expect("the synthetic project manifest is a valid source document"),
    )
}

#[cfg(test)]
mod tests {
    use arcweft_lang_sema::env::TypeCheckEnv;

    use super::{compile_source, compile_source_with_env};

    #[test]
    fn source_compiler_entrypoints_reject_removed_role_declarations_at_parse() {
        for source in [
            "state GameState {\n    value: i32\n}\n",
            "reducer update(state: GameState, event: GameEvent) -> GameState {\n    state\n}\n",
            "agent @agent.smoke smoke() {\n    Ok(())\n}\n",
        ] {
            for result in [
                compile_source(source),
                compile_source_with_env(source, &TypeCheckEnv::standard()),
            ] {
                let error = result.expect_err("removed declaration must be rejected");
                assert!(
                    error
                        .project()
                        .diagnostics()
                        .iter()
                        .any(|diagnostic| diagnostic.parse_error().is_some())
                );
            }
        }
    }
}
