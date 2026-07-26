use super::{
    project::source_document_for_path,
    shared::{is_arcw_path, print_json},
};
use arcweft_source::SourceDocument;
use arcweft_tooling::{
    format::format_document,
    model::{FormatOptions, ToolingDiagnostic, ToolingEditReport, ToolingError},
};
use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

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

fn is_awfagent_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "awfagent")
}

fn is_tooling_source_path(path: &Path) -> bool {
    is_arcw_path(path) || is_awfagent_path(path)
}

fn collect_tooling_paths(path: &Path) -> Result<Vec<PathBuf>, ExitCode> {
    if path.is_file() {
        if !is_tooling_source_path(path) {
            eprintln!(
                "error: {} is not a supported .arcw or .awfagent source file",
                path.display(),
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
            } else if is_tooling_source_path(&entry_path) {
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
    run_tooling_command(options, |document| {
        format_document(
            document,
            FormatOptions {
                canonical_rich_text: options.canonical_rich_text,
            },
        )
    })
}

fn run_tooling_command(
    options: &ToolingCommandOptions,
    mut run_one: impl FnMut(Arc<SourceDocument>) -> Result<ToolingEditReport, ToolingError>,
) -> Result<(), ExitCode> {
    let paths = collect_tooling_paths(&options.path)?;
    let mut reports = Vec::new();
    for path in paths {
        let source = fs::read_to_string(&path).map_err(|error| {
            eprintln!("error: failed to read {}: {error}", path.display());
            ExitCode::FAILURE
        })?;
        let document = Arc::new(source_document_for_path(&path, source)?);
        let report = run_one(document).map_err(|error| {
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
