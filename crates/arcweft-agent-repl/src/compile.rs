use std::{path::PathBuf, sync::Arc};

use arcweft_compiler::{
    agent::compile_agent_project_bundle,
    project::{
        ProjectCompilationContext, ProjectCompileError, ProjectCompileStage, ProjectEntrySelection,
        ProjectEntrySelectionKind, compile_project,
    },
    types::CompiledAgentBundle,
};
use arcweft_id::PublicId;
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::registration::ProjectRegistrationFacts;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_project::{
    manifest::ProjectManifest,
    sources::{ProjectSourceFile, ProjectSources},
};
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::{
    error::{ReplTransactionError, ReplTransactionPhase},
    session::ReplBaseSnapshot,
    source::ParsedReplCell,
};

const REPL_PACKAGE_NAME: &str = "arcweft-agent-repl";

pub(crate) fn compile_repl_cell(
    parsed: &ParsedReplCell,
    base: &ReplBaseSnapshot,
) -> Result<CompiledAgentBundle, ReplTransactionError> {
    let selected_entry = PublicId::try_new(&parsed.synthetic_entry_id).map_err(|error| {
        ReplTransactionError::Compile {
            phase: ReplTransactionPhase::ClassifyParse,
            message: error.to_string(),
        }
    })?;
    let source_path = PathBuf::from(format!("repl/{}.arcw", parsed.synthetic_controller_name));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-repl://{}/{}.arcw",
                base.generation().as_u64(),
                parsed.synthetic_controller_name
            ))
            .map_err(|error| ReplTransactionError::Compile {
                phase: ReplTransactionPhase::ClassifyParse,
                message: error.to_string(),
            })?,
            SourceName::path(source_path.display().to_string()),
            parsed.synthetic_source.clone(),
        )
        .map_err(|error| ReplTransactionError::Compile {
            phase: ReplTransactionPhase::ClassifyParse,
            message: error.to_string(),
        })?,
    );
    let project = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        ProjectManifest::parse_toml(&format!("[package]\nname = \"{REPL_PACKAGE_NAME}\"\n"))
            .map_err(|error| ReplTransactionError::Compile {
                phase: ReplTransactionPhase::ClassifyParse,
                message: error.to_string(),
            })?,
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            source_path,
            Arc::clone(&document),
            [],
        )],
    )
    .map_err(|error| ReplTransactionError::Compile {
        phase: ReplTransactionPhase::ClassifyParse,
        message: error.to_string(),
    })?;
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new(REPL_PACKAGE_NAME).map_err(|error| {
            ReplTransactionError::Compile {
                phase: ReplTransactionPhase::HirLowering,
                message: error.to_string(),
            }
        })?,
        document.identity().id().clone(),
        format!("repl-cell-{}", parsed.synthetic_controller_name),
    )
    .map_err(|error| ReplTransactionError::Compile {
        phase: ReplTransactionPhase::HirLowering,
        message: error.to_string(),
    })?;
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ReplTransactionError::Compile {
        phase: ReplTransactionPhase::SemanticEffectChecks,
        message: error
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.diagnostic().message().to_owned())
            .collect::<Vec<_>>()
            .join("; "),
    })?;
    let context = ProjectCompilationContext::new(
        Arc::new(base.project().typecheck_env()),
        Arc::new(facts),
        None,
        Some(ProjectEntrySelection::new(
            selected_entry.clone(),
            ProjectEntrySelectionKind::Agent,
        )),
        Vec::new(),
    );
    let compiled = compile_project(&project, &context, &RuntimePlanLowerOptions::default())
        .map_err(|error| map_project_compile_error(&error))?;
    let artifact_project = base
        .project()
        .clone()
        .with_checked_entry_catalog(compiled.checked_entries());
    compile_agent_project_bundle(&compiled, &selected_entry, &artifact_project).map_err(|error| {
        ReplTransactionError::Compile {
            phase: ReplTransactionPhase::SemanticEffectChecks,
            message: error.to_string(),
        }
    })
}

fn map_project_compile_error(error: &ProjectCompileError) -> ReplTransactionError {
    let phase = match error
        .diagnostics()
        .first()
        .map(arcweft_compiler::project::ProjectCompileDiagnostic::stage)
    {
        Some(ProjectCompileStage::Parse | ProjectCompileStage::Lint) => {
            ReplTransactionPhase::ClassifyParse
        }
        Some(ProjectCompileStage::HirLower | ProjectCompileStage::HirProject) => {
            ReplTransactionPhase::HirLowering
        }
        _ => ReplTransactionPhase::SemanticEffectChecks,
    };
    let message = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.diagnostic().message().to_owned())
        .collect::<Vec<_>>()
        .join("; ");
    ReplTransactionError::Compile { phase, message }
}
