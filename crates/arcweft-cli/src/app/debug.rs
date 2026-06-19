use super::shared::print_json;
use arcweft_debug_sqlite::store::DebugStore;
use clap::{Args, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

const DEFAULT_DEBUG_DB_PATH: &str = ".arcweft/cache/agent-debug.sqlite3";

#[derive(Debug, Subcommand)]
pub(super) enum DebugCommand {
    Db {
        #[command(subcommand)]
        command: DebugDbCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum DebugDbCommand {
    Status(DebugDbOptions),
    Migrate(DebugDbOptions),
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbOptions {
    #[arg(long, default_value = DEFAULT_DEBUG_DB_PATH)]
    path: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(serde::Serialize)]
struct DebugDbReport {
    path: String,
    user_version: u32,
}

pub(super) fn debug_command(command: DebugCommand) -> Result<(), ExitCode> {
    match command {
        DebugCommand::Db { command } => debug_db_command(command),
    }
}

fn debug_db_command(command: DebugDbCommand) -> Result<(), ExitCode> {
    match command {
        DebugDbCommand::Status(options) | DebugDbCommand::Migrate(options) => {
            let report = open_debug_db(&options)?;
            if options.json {
                print_json(&report)
            } else {
                println!("{}: schema version {}", report.path, report.user_version);
                Ok(())
            }
        }
    }
}

fn open_debug_db(options: &DebugDbOptions) -> Result<DebugDbReport, ExitCode> {
    if let Some(parent) = options.path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            eprintln!("error: failed to create {}: {error}", parent.display());
            ExitCode::FAILURE
        })?;
    }
    let store = DebugStore::open(&options.path).map_err(|error| {
        eprintln!(
            "error: failed to open debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let user_version = store.user_version().map_err(|error| {
        eprintln!(
            "error: failed to read debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    Ok(DebugDbReport {
        path: options.path.display().to_string(),
        user_version,
    })
}
