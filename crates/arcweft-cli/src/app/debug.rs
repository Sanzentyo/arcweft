use super::shared::print_json;
use arcweft_debug_sqlite::store::{
    DebugStore, DebugStoreForeignKeyViolation, DebugStoreStats, DebugStoreValidationReport,
};
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
    Validate(DebugDbOptions),
    Reindex(DebugDbOptions),
    Delete(DebugDbDeleteOptions),
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbOptions {
    #[arg(long, default_value = DEFAULT_DEBUG_DB_PATH)]
    path: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(super) struct DebugDbDeleteOptions {
    #[command(flatten)]
    db: DebugDbOptions,
    #[arg(long)]
    unreferenced_blobs: bool,
    #[arg(long)]
    validate: bool,
}

#[derive(serde::Serialize)]
struct DebugDbReport {
    path: String,
    user_version: u32,
    stats: DebugDbStatsReport,
}

#[derive(serde::Serialize)]
struct DebugDbStatsReport {
    programs: u64,
    sessions: u64,
    script_runs: u64,
    debug_events: u64,
    frames: u64,
    actions: u64,
    captures: u64,
    blobs: u64,
    chunks: u64,
    embeddings: u64,
    rag_queries: u64,
}

#[derive(serde::Serialize)]
struct DebugDbValidationCliReport {
    path: String,
    user_version: u32,
    valid: bool,
    integrity_messages: Vec<String>,
    foreign_key_violations: Vec<DebugDbForeignKeyViolationReport>,
    missing_capture_blob_refs: u64,
    invalid_embedding_blobs: u64,
    stats: DebugDbStatsReport,
}

#[derive(serde::Serialize)]
struct DebugDbForeignKeyViolationReport {
    table: String,
    rowid: i64,
    parent: String,
    fkid: i64,
}

#[derive(serde::Serialize)]
struct DebugDbReindexReport {
    path: String,
    user_version: u32,
    chunks_indexed: u64,
}

#[derive(serde::Serialize)]
struct DebugDbDeleteReport {
    path: String,
    user_version: u32,
    deleted_unreferenced_blobs: u64,
    validation: Option<DebugDbValidationCliReport>,
}

pub(super) fn debug_command(command: DebugCommand) -> Result<(), ExitCode> {
    match command {
        DebugCommand::Db { command } => debug_db_command(command),
    }
}

fn debug_db_command(command: DebugDbCommand) -> Result<(), ExitCode> {
    match command {
        DebugDbCommand::Status(options) | DebugDbCommand::Migrate(options) => {
            debug_db_status_or_migrate_command(&options)
        }
        DebugDbCommand::Validate(options) => debug_db_validate_command(&options),
        DebugDbCommand::Reindex(options) => debug_db_reindex_command(&options),
        DebugDbCommand::Delete(options) => debug_db_delete_command(&options),
    }
}

fn debug_db_status_or_migrate_command(options: &DebugDbOptions) -> Result<(), ExitCode> {
    let report = open_debug_db(options)?;
    if options.json {
        return print_json(&report);
    }
    println!(
        "{}: schema version {}, chunks {}, blobs {}, captures {}",
        report.path,
        report.user_version,
        report.stats.chunks,
        report.stats.blobs,
        report.stats.captures
    );
    Ok(())
}

fn debug_db_validate_command(options: &DebugDbOptions) -> Result<(), ExitCode> {
    let (store, path, user_version, stats) = open_debug_store(options)?;
    let validation = store.validate().map_err(|error| {
        eprintln!(
            "error: failed to validate debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let report = validation_report(path, user_version, stats, validation);
    if options.json {
        return print_json(&report);
    }
    println!(
        "{}: {}",
        report.path,
        if report.valid { "valid" } else { "invalid" }
    );
    println!(
        "integrity_messages={}, foreign_key_violations={}, missing_capture_blob_refs={}, invalid_embedding_blobs={}",
        report.integrity_messages.len(),
        report.foreign_key_violations.len(),
        report.missing_capture_blob_refs,
        report.invalid_embedding_blobs
    );
    Ok(())
}

fn debug_db_reindex_command(options: &DebugDbOptions) -> Result<(), ExitCode> {
    let (store, path, user_version, _) = open_debug_store(options)?;
    let report = store.reindex().map_err(|error| {
        eprintln!(
            "error: failed to reindex debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let report = DebugDbReindexReport {
        path,
        user_version,
        chunks_indexed: report.chunks_indexed,
    };
    if options.json {
        return print_json(&report);
    }
    println!(
        "{}: rebuilt chunk FTS index for {} chunks",
        report.path, report.chunks_indexed
    );
    Ok(())
}

fn debug_db_delete_command(options: &DebugDbDeleteOptions) -> Result<(), ExitCode> {
    if !options.unreferenced_blobs {
        eprintln!("error: debug db delete requires --unreferenced-blobs");
        return Err(ExitCode::from(2));
    }
    let (store, path, user_version, _) = open_debug_store(&options.db)?;
    let deleted_unreferenced_blobs = store.delete_unreferenced_blobs().map_err(|error| {
        eprintln!(
            "error: failed to delete unreferenced blob records from debug database {}: {error}",
            options.db.path.display()
        );
        ExitCode::FAILURE
    })?;
    let validation = options
        .validate
        .then(|| post_delete_validation(&store, path.clone(), user_version, &options.db))
        .transpose()?;
    let report = DebugDbDeleteReport {
        path,
        user_version,
        deleted_unreferenced_blobs,
        validation,
    };
    if options.db.json {
        return print_json(&report);
    }
    println!(
        "{}: deleted {} unreferenced blob records",
        report.path, report.deleted_unreferenced_blobs
    );
    if let Some(validation) = &report.validation {
        println!(
            "validation: {}",
            if validation.valid { "valid" } else { "invalid" }
        );
    }
    Ok(())
}

fn post_delete_validation(
    store: &DebugStore,
    path: String,
    user_version: u32,
    options: &DebugDbOptions,
) -> Result<DebugDbValidationCliReport, ExitCode> {
    let stats = store.stats().map_err(|error| {
        eprintln!(
            "error: failed to read debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    let validation = store.validate().map_err(|error| {
        eprintln!(
            "error: failed to validate debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    Ok(validation_report(
        path,
        user_version,
        stats_report(stats),
        validation,
    ))
}

fn open_debug_db(options: &DebugDbOptions) -> Result<DebugDbReport, ExitCode> {
    let (_store, path, user_version, stats) = open_debug_store(options)?;
    Ok(DebugDbReport {
        path,
        user_version,
        stats,
    })
}

fn open_debug_store(
    options: &DebugDbOptions,
) -> Result<(DebugStore, String, u32, DebugDbStatsReport), ExitCode> {
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
    let stats = store.stats().map_err(|error| {
        eprintln!(
            "error: failed to read debug database {}: {error}",
            options.path.display()
        );
        ExitCode::FAILURE
    })?;
    Ok((
        store,
        options.path.display().to_string(),
        user_version,
        stats_report(stats),
    ))
}

fn validation_report(
    path: String,
    user_version: u32,
    stats: DebugDbStatsReport,
    validation: DebugStoreValidationReport,
) -> DebugDbValidationCliReport {
    let valid = validation.integrity_messages.is_empty()
        && validation.foreign_key_violations.is_empty()
        && validation.missing_capture_blob_refs == 0
        && validation.invalid_embedding_blobs == 0;
    DebugDbValidationCliReport {
        path,
        user_version,
        valid,
        integrity_messages: validation.integrity_messages,
        foreign_key_violations: validation
            .foreign_key_violations
            .into_iter()
            .map(foreign_key_violation_report)
            .collect(),
        missing_capture_blob_refs: validation.missing_capture_blob_refs,
        invalid_embedding_blobs: validation.invalid_embedding_blobs,
        stats,
    }
}

fn foreign_key_violation_report(
    violation: DebugStoreForeignKeyViolation,
) -> DebugDbForeignKeyViolationReport {
    DebugDbForeignKeyViolationReport {
        table: violation.table,
        rowid: violation.rowid,
        parent: violation.parent,
        fkid: violation.fkid,
    }
}

fn stats_report(stats: DebugStoreStats) -> DebugDbStatsReport {
    DebugDbStatsReport {
        programs: stats.programs,
        sessions: stats.sessions,
        script_runs: stats.script_runs,
        debug_events: stats.debug_events,
        frames: stats.frames,
        actions: stats.actions,
        captures: stats.captures,
        blobs: stats.blobs,
        chunks: stats.chunks,
        embeddings: stats.embeddings,
        rag_queries: stats.rag_queries,
    }
}
