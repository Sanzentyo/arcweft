use super::commands::IdsCommand;
use super::shared::{is_arcw_path, print_json};
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_sema::{
    canonicalization::{CanonicalizationSourceSet, SemanticDataUnavailable},
    check::analyze_project_types_for_canonicalization,
    env::TypeCheckEnv,
};
use arcweft_source::{SourceDocumentId, SourceRange};
use arcweft_tooling::{
    canonicalize_source,
    format::format_source,
    id_context::materialize_ids,
    model::{
        CanonicalizationInput, FormatOptions, ToolingDiagnostic, ToolingEditReport, ToolingError,
    },
};
use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Args, Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub(in crate::app) struct ToolingCommandOptions {
    path: PathBuf,
    #[arg(long)]
    canonical_rich_text: bool,
    #[arg(long)]
    write: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct CanonicalizeCommandOptions {
    path: PathBuf,
    #[arg(long)]
    write: bool,
    #[arg(long)]
    json: bool,
}

fn is_awfagent_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "awfagent")
}

fn is_tooling_source_path(path: &Path) -> bool {
    is_arcw_path(path) || is_awfagent_path(path)
}

fn collect_tooling_paths(path: &Path, game_only: bool) -> Result<Vec<PathBuf>, ExitCode> {
    if path.is_file() {
        if !is_tooling_source_path(path) || (game_only && !is_arcw_path(path)) {
            eprintln!(
                "error: {} is not a supported {} source file",
                path.display(),
                if game_only {
                    ".arcw"
                } else {
                    ".arcw or .awfagent"
                },
            );
            return Err(ExitCode::from(2));
        }
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        eprintln!("error: {} is not a file or directory", path.display());
        return Err(ExitCode::from(2));
    }
    let mut paths = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|error| {
            eprintln!("error: failed to read {}: {error}", dir.display());
            ExitCode::FAILURE
        })? {
            let entry = entry.map_err(|error| {
                eprintln!("error: failed to read directory entry: {error}");
                ExitCode::FAILURE
            })?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else if is_tooling_source_path(&entry_path)
                && (!game_only || is_arcw_path(&entry_path))
            {
                paths.push(entry_path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

#[derive(serde::Serialize)]
struct ToolingCommandReport {
    files: Vec<ToolingFileReport>,
}

#[derive(serde::Serialize)]
struct ToolingFileReport {
    path: String,
    changed: bool,
    edits: usize,
    diagnostics: Vec<ToolingDiagnostic>,
    output: Option<String>,
}

pub(super) fn format_command(options: &ToolingCommandOptions) -> Result<(), ExitCode> {
    run_tooling_command(options, false, |_path, source| {
        format_source(
            source,
            FormatOptions {
                canonical_rich_text: options.canonical_rich_text,
            },
        )
    })
}

pub(super) fn canonicalize_command(options: &CanonicalizeCommandOptions) -> Result<(), ExitCode> {
    let tooling_options = ToolingCommandOptions {
        path: options.path.clone(),
        canonical_rich_text: false,
        write: options.write,
        json: options.json,
    };
    run_tooling_command(&tooling_options, true, canonicalize_project_source)
}

pub(super) fn ids_command(command: IdsCommand) -> Result<(), ExitCode> {
    match command {
        IdsCommand::Materialize(options) => {
            run_tooling_command(&options, false, |_path, source| materialize_ids(source))
        }
    }
}

fn run_tooling_command(
    options: &ToolingCommandOptions,
    game_only: bool,
    mut run_one: impl FnMut(&Path, &str) -> Result<ToolingEditReport, ToolingError>,
) -> Result<(), ExitCode> {
    let paths = collect_tooling_paths(&options.path, game_only)?;
    let mut reports = Vec::new();
    for path in paths {
        let source = fs::read_to_string(&path).map_err(|error| {
            eprintln!("error: failed to read {}: {error}", path.display());
            ExitCode::FAILURE
        })?;
        let report = run_one(&path, &source).map_err(|error| {
            eprintln!("error: failed to edit {}: {error}", path.display());
            ExitCode::FAILURE
        })?;
        if options.write && report.changed {
            fs::write(&path, &report.output).map_err(|error| {
                eprintln!("error: failed to write {}: {error}", path.display());
                ExitCode::FAILURE
            })?;
        }
        reports.push(ToolingFileReport {
            path: path.display().to_string(),
            changed: report.changed,
            edits: report.edits.len(),
            diagnostics: report.diagnostics,
            output: if options.write {
                None
            } else {
                Some(report.output)
            },
        });
    }
    if options.json {
        print_json(&ToolingCommandReport { files: reports })
    } else {
        for report in &reports {
            println!(
                "{}: {} edit(s){}",
                report.path,
                report.edits,
                if report.changed { "" } else { " (unchanged)" }
            );
            for diagnostic in &report.diagnostics {
                eprintln!(
                    "{} {}:{}..{}: {}",
                    diagnostic.code,
                    report.path,
                    diagnostic.start,
                    diagnostic.end,
                    diagnostic.message
                );
            }
            if !options.write
                && let Some(output) = &report.output
            {
                print!("{output}");
                if !output.ends_with('\n') {
                    println!();
                }
            }
        }
        Ok(())
    }
}

fn canonicalize_project_source(
    path: &Path,
    source: &str,
) -> Result<ToolingEditReport, ToolingError> {
    let loaded = arcweft_project_loader::project::load_discovered(path)
        .map_err(|error| semantic_data_unavailable(path, error.to_string()))?;
    let selected_path = normalized_path(path);
    let selected = loaded
        .sources()
        .modules()
        .find(|candidate| normalized_path(candidate.path()) == selected_path)
        .ok_or_else(|| {
            semantic_data_unavailable(
                path,
                "source is not a module in the discovered project".to_owned(),
            )
        })?;
    if selected.source() != source {
        return Err(semantic_data_unavailable(
            path,
            "loaded project source differs from the requested source snapshot".to_owned(),
        ));
    }

    let lowered = loaded
        .sources()
        .modules()
        .map(|project_source| {
            let document = loaded
                .module_document(project_source.module())
                .expect("loaded project retains one document per source module");
            let parsed = arcweft_lang_syntax::parser::parse_source(document.text());
            if !parsed.errors().is_empty() {
                return Err(semantic_data_unavailable(
                    project_source.path(),
                    format!("source has syntax errors: {:?}", parsed.errors()),
                ));
            }
            let hir = lower_document_to_hir(document, parsed.typed_tree()).map_err(|errors| {
                semantic_data_unavailable(
                    project_source.path(),
                    format!("HIR lowering failed: {errors:?}"),
                )
            })?;
            let identity = document.identity().clone();
            let source_span = document
                .span(SourceRange::new(0, document.text().len()))
                .expect("loaded source document owns its complete UTF-8 range");
            let module =
                HirProjectModule::try_new(project_source.module().clone(), identity.clone(), hir)
                    .map_err(|error| {
                    semantic_data_unavailable(project_source.path(), error.to_string())
                })?;
            Ok((module, (project_source.module().clone(), source_span)))
        })
        .collect::<Result<Vec<_>, ToolingError>>()?;
    let hir_project = HirProject::new(
        loaded.sources().manifest().package().name().as_str(),
        lowered.iter().map(|(module, _)| module.clone()),
    )
    .map_err(|error| semantic_data_unavailable(path, error.to_string()))?;
    let identities = lowered
        .iter()
        .map(|(_, source)| source.clone())
        .collect::<Vec<_>>();
    let sources = CanonicalizationSourceSet::try_new(hir_project.package().clone(), identities)
        .map_err(|error| semantic_data_unavailable(path, error.to_string()))?;
    let report = analyze_project_types_for_canonicalization(
        &hir_project,
        &TypeCheckEnv::standard(),
        &sources,
    )
    .map_err(|error| semantic_data_unavailable(path, error.to_string()))?;
    let selected_identity = sources.source(selected.module()).ok_or_else(|| {
        semantic_data_unavailable(path, "selected module has no semantic identity".to_owned())
    })?;
    let inventory = report
        .canonicalization_inventory(selected.module(), selected_identity)
        .ok_or_else(|| {
            semantic_data_unavailable(
                path,
                "checked report has no exact inventory for the selected module".to_owned(),
            )
        })?;
    canonicalize_source(source, CanonicalizationInput::Checked(inventory))
}

fn normalized_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn semantic_data_unavailable(path: &Path, reason: String) -> ToolingError {
    let unavailable = SemanticDataUnavailable::new(
        SourceDocumentId::try_new(normalized_path(path).display().to_string())
            .expect("normalized source path is non-empty"),
        reason,
    );
    ToolingError::SemanticDataUnavailable {
        document: unavailable.document().as_str().to_owned(),
        reason: unavailable.reason().to_owned(),
    }
}
