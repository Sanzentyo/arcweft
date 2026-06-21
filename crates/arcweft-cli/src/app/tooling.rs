use super::commands::IdsCommand;
use super::shared::{is_arcw_path, print_json};
use arcweft_lang_syntax::parser::SourceDialect;
use arcweft_tooling::{
    format::format_source_with_dialect,
    id_context::materialize_ids,
    model::{FormatOptions, ToolingEditReport, ToolingError},
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
    expand_sugar: bool,
    #[arg(long)]
    canonical_rich_text: bool,
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

fn source_dialect_for_path(path: &Path) -> Option<SourceDialect> {
    if is_arcw_path(path) {
        Some(SourceDialect::Game)
    } else if is_awfagent_path(path) {
        Some(SourceDialect::Agent)
    } else {
        None
    }
}

fn collect_tooling_paths(path: &Path) -> Result<Vec<PathBuf>, ExitCode> {
    if path.is_file() {
        if source_dialect_for_path(path).is_none() {
            eprintln!(
                "error: {} is not an .arcw or .awfagent source file",
                path.display()
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
            } else if source_dialect_for_path(&entry_path).is_some() {
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
    output: Option<String>,
}

pub(super) fn format_command(options: &ToolingCommandOptions) -> Result<(), ExitCode> {
    run_tooling_command(options, |path, source| {
        let dialect = source_dialect_for_path(path).unwrap_or(SourceDialect::Game);
        format_source_with_dialect(
            source,
            dialect,
            FormatOptions {
                expand_sugar: options.expand_sugar,
                canonical_rich_text: options.canonical_rich_text,
            },
        )
    })
}

pub(super) fn ids_command(command: IdsCommand) -> Result<(), ExitCode> {
    match command {
        IdsCommand::Materialize(options) => {
            run_tooling_command(&options, |_path, source| materialize_ids(source))
        }
    }
}

fn run_tooling_command(
    options: &ToolingCommandOptions,
    mut run_one: impl FnMut(&Path, &str) -> Result<ToolingEditReport, ToolingError>,
) -> Result<(), ExitCode> {
    let paths = collect_tooling_paths(&options.path)?;
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
