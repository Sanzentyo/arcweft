use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use arcweft_compiler::{
    agent::compile_agent_project_bundle,
    incremental::{BuildSnapshotRequest, runtime_plan_artifact_key, snapshot_compiled_project},
    project::{
        ProjectCompilationContext, ProjectCompilationSession, ProjectCompileError,
        ProjectCompileStage, ProjectEntrySelection, ProjectEntrySelectionKind, compile_project,
    },
    types::CompiledAgentBundle,
};
use arcweft_id::PublicId;
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::registration::ProjectRegistrationFacts;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::{
    binding::{ReplBindingRecord, committed_bindings},
    error::{ReplTransactionError, ReplTransactionPhase},
    session::ReplBaseSnapshot,
    source::ParsedReplCell,
};

const REPL_PACKAGE_NAME: &str = "org.arcweft.tool.agent-repl";

pub(crate) struct CompiledReplCell {
    pub(crate) artifact: CompiledAgentBundle,
    pub(crate) bindings: Vec<ReplBindingRecord>,
}

pub(crate) fn compile_repl_cell(
    parsed: &ParsedReplCell,
    base: &ReplBaseSnapshot,
) -> Result<CompiledReplCell, ReplTransactionError> {
    let selected_entry = PublicId::try_new(&parsed.synthetic_entry_id).map_err(|error| {
        ReplTransactionError::Compile {
            phase: ReplTransactionPhase::ClassifyParse,
            message: error.to_string(),
        }
    })?;
    let source_path = PathBuf::from(format!("repl/{}.arcw", parsed.synthetic_controller_name));
    let document = Arc::clone(parsed.parsed_source.document_lease());
    let project = repl_project(source_path, &document)?;
    let facts = repl_registration_facts(parsed, &document)?;
    let context = ProjectCompilationContext::new(
        Arc::clone(base.typecheck_environment()),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
        None,
        Some(ProjectEntrySelection::new(
            selected_entry.clone(),
            ProjectEntrySelectionKind::Agent,
        )),
    );
    let parsed_sources = BTreeMap::from([(
        CanonicalModulePath::crate_root(),
        parsed.parsed_source.clone(),
    )]);
    let mut compilation_session =
        ProjectCompilationSession::try_new().map_err(|error| ReplTransactionError::Compile {
            phase: ReplTransactionPhase::HirLowering,
            message: error.to_string(),
        })?;
    let compiled = compile_project(
        &mut compilation_session,
        &project,
        &parsed_sources,
        &context,
    )
    .map_err(|error| map_project_compile_error(&error))?;
    let artifact_project = base.target_entities().iter().cloned().fold(
        compiled.semantic_index().as_ref().clone(),
        arcweft_lang_sema::project_index::ProjectSemanticIndex::with_entity,
    );
    let bindings = committed_bindings(
        parsed.id,
        compiled.hir_project(),
        &document,
        &parsed.synthetic_controller_name,
        parsed.cell_source_range,
    )
    .map_err(|message| ReplTransactionError::Compile {
        phase: ReplTransactionPhase::SemanticEffectChecks,
        message,
    })?;
    let snapshot = snapshot_compiled_project(
        &project,
        &compiled,
        BuildSnapshotRequest {
            build_id: compiled.program_hash().as_str().to_owned(),
            compiler_build_id: env!("CARGO_PKG_VERSION").to_owned(),
            target_triple: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            target_features: Vec::new(),
            profile: "agent-repl".to_owned(),
            selected_entries: vec![selected_entry.as_str().to_owned()],
        },
    );
    let runtime_plan_artifact_key = runtime_plan_artifact_key(&snapshot, &compiled);
    let artifact = compile_agent_project_bundle(
        &compiled,
        &selected_entry,
        &artifact_project,
        runtime_plan_artifact_key,
    )
    .map_err(|error| ReplTransactionError::Compile {
        phase: ReplTransactionPhase::SemanticEffectChecks,
        message: error.to_string(),
    })?;
    Ok(CompiledReplCell { artifact, bindings })
}

fn repl_project(
    source_path: PathBuf,
    document: &Arc<SourceDocument>,
) -> Result<ProjectSources, ReplTransactionError> {
    ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new(REPL_PACKAGE_NAME).map_err(|error| {
                ReplTransactionError::Compile {
                    phase: ReplTransactionPhase::ClassifyParse,
                    message: error.to_string(),
                }
            })?,
            version: PackageVersion::new("0.0.0").map_err(|error| {
                ReplTransactionError::Compile {
                    phase: ReplTransactionPhase::ClassifyParse,
                    message: error.to_string(),
                }
            })?,
        },
        BuildSpec::default(),
        Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-repl://manifest").map_err(|error| {
                    ReplTransactionError::Compile {
                        phase: ReplTransactionPhase::ClassifyParse,
                        message: error.to_string(),
                    }
                })?,
                SourceName::Memory,
                "",
            )
            .map_err(|error| ReplTransactionError::Compile {
                phase: ReplTransactionPhase::ClassifyParse,
                message: error.to_string(),
            })?,
        ),
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            source_path,
            Arc::clone(document),
            [],
        )],
    )
    .map_err(|error| ReplTransactionError::Compile {
        phase: ReplTransactionPhase::ClassifyParse,
        message: error.to_string(),
    })
}

fn repl_registration_facts(
    parsed: &ParsedReplCell,
    document: &Arc<SourceDocument>,
) -> Result<ProjectRegistrationFacts, ReplTransactionError> {
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
    ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(document)],
        Vec::new(),
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
    })
}

fn map_project_compile_error(error: &ProjectCompileError) -> ReplTransactionError {
    let parse_diagnostics = error
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| diagnostic.syntax_diagnostic().cloned())
        .collect::<Vec<_>>();
    if !parse_diagnostics.is_empty() {
        return ReplTransactionError::AttachedParse {
            diagnostics: parse_diagnostics,
            coordinate_space: crate::error::ReplParseCoordinateSpace::SyntheticSourceUtf8Bytes,
        };
    }

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

#[cfg(test)]
mod tests {
    use crate::{
        cell::{ReplCellId, ReplCellInput},
        session::ReplBaseSnapshot,
        source::classify_repl_cell,
    };
    use arcweft_lang_sema::project_index::ProgramHash;
    use std::sync::Arc;

    use super::compile_repl_cell;

    #[test]
    fn compiler_reuses_the_accepted_synthetic_source_identity() {
        let parsed = classify_repl_cell(
            ReplCellId::new(1),
            &ReplCellInput::statement("let answer = 42"),
            "",
        )
        .expect("synthetic cell parses exactly once");
        let base = ReplBaseSnapshot::new(
            "test",
            &ProgramHash::new("program-test"),
            Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
            [],
        );
        let compiled = compile_repl_cell(&parsed, &base).expect("accepted cell compiles");
        assert!(
            compiled
                .artifact
                .hir_project
                .view()
                .modules()
                .any(|(_, module)| module.provenance().source_identity()
                    == parsed.parsed_source.document().identity())
        );
        assert_eq!(compiled.bindings.len(), 1);
        assert_eq!(compiled.bindings[0].name, "answer");
    }
}
