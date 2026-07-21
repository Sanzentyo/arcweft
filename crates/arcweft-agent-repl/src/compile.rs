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
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::{
    error::{ReplTransactionError, ReplTransactionPhase},
    session::ReplBaseSnapshot,
    source::ParsedReplCell,
};

const REPL_PACKAGE_NAME: &str = "org.arcweft.tool.agent-repl";

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
    let (source_path, document) = repl_source_document(parsed, base)?;
    let project = repl_project(source_path, &document)?;
    let facts = repl_registration_facts(parsed, &document)?;
    let context = ProjectCompilationContext::new(
        Arc::new(base.project().typecheck_env()),
        Arc::new(facts),
        Arc::new(arcweft_resource_model::registry::ResourceTypeRegistry::empty()),
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

fn repl_source_document(
    parsed: &ParsedReplCell,
    base: &ReplBaseSnapshot,
) -> Result<(PathBuf, Arc<SourceDocument>), ReplTransactionError> {
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
    Ok((source_path, document))
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
    ProjectRegistrationFacts::try_new(world, vec![Arc::clone(document)], Vec::new(), Vec::new())
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
        .filter_map(|diagnostic| diagnostic.parse_error().cloned())
        .collect::<Vec<_>>();
    if !parse_diagnostics.is_empty() {
        return ReplTransactionError::Parse {
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
    use arcweft_lang_sema::project_index::{ProgramHash, ProjectSemanticIndex};
    use arcweft_lang_syntax::parser::recovery::ParseErrorKind;

    use crate::{
        cell::ReplCellKind, error::ReplTransactionError, session::ReplBaseSnapshot,
        source::ParsedReplCell,
    };

    use super::compile_repl_cell;

    #[test]
    fn project_parser_payload_reaches_the_repl_without_code_reconstruction() {
        let source = r"pub view Card() {
    export part as card.heading
    Panel().part(header)
}
";
        let parsed = ParsedReplCell {
            kind: ReplCellKind::Item,
            source: source.to_owned(),
            source_hash: "source-hash".to_owned(),
            synthetic_source: source.to_owned(),
            synthetic_source_hash: "synthetic-source-hash".to_owned(),
            synthetic_entry_id: "entry.agent.repl.cell_1".to_owned(),
            synthetic_controller_name: "repl_cell_1".to_owned(),
            bindings: Vec::new(),
        };
        let base = ReplBaseSnapshot::from_project(
            "test",
            ProjectSemanticIndex::new(ProgramHash::new("program-test")),
        );

        let Err(error) = compile_repl_cell(&parsed, &base) else {
            panic!("malformed View export must fail REPL compilation");
        };
        let ReplTransactionError::Parse {
            diagnostics,
            coordinate_space,
        } = error
        else {
            panic!("project parser diagnostics must remain typed at the REPL boundary");
        };
        assert_eq!(
            coordinate_space,
            crate::error::ReplParseCoordinateSpace::SyntheticSourceUtf8Bytes
        );
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.kind() == ParseErrorKind::ViewExportPartMissingLocal)
            .expect("typed missing-local parser diagnostic");
        assert_eq!(diagnostic.code(), "view::export_part_missing_local");
        assert_eq!(&source[diagnostic.range().as_range()], "as");
    }
}
