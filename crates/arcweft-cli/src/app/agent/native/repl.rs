use super::repl_project_binding::agent_repl_reconcile_project_bound_bindings;
use super::repl_snapshot::agent_repl_serialized_bindings;
use super::*;

#[derive(Default)]
pub(super) struct AgentReplState {
    pub(super) report: Option<AgentObservationReport>,
    pub(super) history: Vec<AgentReplHistoryEntry>,
    pub(super) bindings: BTreeMap<String, AgentReplBinding>,
    pub(super) connection: Option<AgentReplConnection>,
    pub(super) remote_session: Option<McpAgentSession<StdioMcpTransport>>,
    pub(super) remote_program_hash: Option<String>,
    pub(super) debug_store: Option<DebugStore>,
    pub(super) trace_path: Option<String>,
    pub(super) trace_records: usize,
    pub(super) trace_resources: Vec<AgentResource>,
    pub(super) read_only: bool,
    pub(super) persisted_cells: u64,
    pub(super) debug_db_path: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum AgentReplConnection {
    Source { path: String },
    Profile { id: String, manifest: String },
    StdioMcp { program: String, args: Vec<String> },
}

#[derive(Debug, serde::Serialize)]
pub(super) struct AgentReplHistoryEntry {
    pub(super) index: usize,
    pub(super) input: String,
    pub(super) kind: String,
    pub(super) status: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub(super) struct AgentReplBinding {
    pub(super) name: String,
    pub(super) binding_kind: String,
    pub(super) source: String,
    pub(super) status: String,
    pub(super) final_status: Option<String>,
    pub(super) host_calls: usize,
    pub(super) responses: usize,
    pub(super) serializable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) serialized_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) snapshot_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) non_serializable_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AgentReplSerializedBinding {
    pub(super) source: String,
    pub(super) snapshot_kind: String,
}

#[derive(Debug)]
struct AgentReplRemoteConnection {
    session: Option<McpAgentSession<StdioMcpTransport>>,
    program_hash: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct AgentReplRunReport {
    pub(super) ok: bool,
    pub(super) cells: Vec<AgentReplCellReport>,
    pub(super) final_tick: Option<usize>,
    pub(super) connection: Option<AgentReplConnection>,
    pub(super) remote_program_hash: Option<String>,
    pub(super) debug_db: Option<String>,
    pub(super) trace_path: Option<String>,
    pub(super) trace_records: usize,
    pub(super) read_only: bool,
    pub(super) persisted_cells: u64,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct AgentReplCellReport {
    pub(super) index: usize,
    pub(super) input: String,
    pub(super) kind: String,
    pub(super) status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) value: Option<serde_json::Value>,
    #[serde(default)]
    pub(super) quit: bool,
}

impl AgentReplCellReport {
    pub(super) fn with_persistence(mut self, result: Result<bool, String>) -> Self {
        match result {
            Ok(false) => self,
            Ok(true) => {
                if let Some(serde_json::Value::Object(value)) = &mut self.value {
                    value.insert("persisted".to_owned(), serde_json::Value::Bool(true));
                }
                self
            }
            Err(error) => Self {
                index: self.index,
                input: self.input,
                kind: self.kind,
                status: "error".to_owned(),
                message: Some(error),
                value: self.value,
                quit: false,
            },
        }
    }
}

pub(super) fn agent_repl_command(
    options: &AgentReplOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let mut state = agent_repl_initial_state(options)?;
    #[cfg(feature = "agent-repl")]
    {
        if agent_repl_uses_interactive_editor(options) {
            return agent_repl_interactive_command(options, state, adapter_registrars);
        }
    }
    let source = agent_repl_input(options)?;
    agent_repl_scripted_command(options, &mut state, adapter_registrars, &source)
}

pub(super) fn agent_repl_initial_state(
    options: &AgentReplOptions,
) -> Result<AgentReplState, ExitCode> {
    let (debug_store, debug_db_path) = agent_repl_debug_store(options)?;
    let trace = agent_repl_trace_resources(options)?;
    let connection = agent_repl_initial_connection(options)?;
    let remote_connection =
        agent_repl_connect_remote_session(connection.as_ref()).map_err(|message| {
            eprintln!("error: {message}");
            ExitCode::from(2)
        })?;
    Ok(AgentReplState {
        connection,
        remote_session: remote_connection.session,
        remote_program_hash: remote_connection.program_hash,
        debug_store,
        debug_db_path,
        trace_path: trace.path,
        trace_records: trace.record_count,
        trace_resources: trace.resources,
        read_only: options.read_only,
        ..AgentReplState::default()
    })
}

pub(super) fn agent_repl_scripted_command(
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
    source: &str,
) -> Result<(), ExitCode> {
    let mut cells = Vec::new();
    let mut ok = true;

    for (index, line) in source.lines().enumerate() {
        let input = line.trim();
        if input.is_empty() || input.starts_with('#') {
            continue;
        }
        let report = agent_repl_eval_line(index, input, options, &mut *state, adapter_registrars);
        let quit = report.quit;
        ok &= report.status != "error";
        state.history.push(AgentReplHistoryEntry {
            index: report.index,
            input: report.input.clone(),
            kind: report.kind.clone(),
            status: report.status.clone(),
        });
        if options.json {
            cells.push(report);
        } else {
            agent_repl_print_cell(&report);
        }
        if quit {
            break;
        }
    }

    agent_repl_finish_debug_session(state, ok)?;

    if options.json {
        print_json(&AgentReplRunReport {
            ok,
            cells,
            final_tick: state.report.as_ref().map(|report| report.tick),
            connection: state.connection.clone(),
            remote_program_hash: state.remote_program_hash.clone(),
            debug_db: state.debug_db_path.clone(),
            trace_path: state.trace_path.clone(),
            trace_records: state.trace_records,
            read_only: state.read_only,
            persisted_cells: state.persisted_cells,
        })?;
    }
    if ok { Ok(()) } else { Err(ExitCode::from(2)) }
}

#[cfg(feature = "agent-repl")]
pub(super) fn agent_repl_uses_interactive_editor(options: &AgentReplOptions) -> bool {
    options.input.is_none() && !options.json && std::io::stdin().is_terminal()
}

#[cfg(feature = "agent-repl")]
pub(super) fn agent_repl_interactive_command(
    options: &AgentReplOptions,
    mut state: AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let mut editor = agent_repl_line_editor(&state)?;
    let prompt = reedline::DefaultPrompt::new(
        reedline::DefaultPromptSegment::Basic("arcw".to_owned()),
        reedline::DefaultPromptSegment::Basic("agent".to_owned()),
    );
    let mut ok = true;
    let mut index = 0usize;
    loop {
        match editor.read_line(&prompt) {
            Ok(reedline::Signal::Success(buffer)) => {
                let input = buffer.trim();
                if input.is_empty() || input.starts_with('#') {
                    continue;
                }
                let report =
                    agent_repl_eval_line(index, input, options, &mut state, adapter_registrars);
                let quit = report.quit;
                ok &= report.status != "error";
                state.history.push(AgentReplHistoryEntry {
                    index: report.index,
                    input: report.input.clone(),
                    kind: report.kind.clone(),
                    status: report.status.clone(),
                });
                agent_repl_print_cell(&report);
                index = index.saturating_add(1);
                editor = agent_repl_line_editor(&state)?;
                if quit {
                    break;
                }
            }
            Ok(reedline::Signal::CtrlD) => break,
            Ok(reedline::Signal::CtrlC) => {
                eprintln!("^C");
                break;
            }
            Ok(_) => {}
            Err(error) => {
                eprintln!("error: Agent REPL editor failed: {error}");
                ok = false;
                break;
            }
        }
    }
    agent_repl_finish_debug_session(&mut state, ok)?;
    if ok { Ok(()) } else { Err(ExitCode::from(2)) }
}

#[cfg(feature = "agent-repl")]
pub(super) fn agent_repl_line_editor(
    state: &AgentReplState,
) -> Result<reedline::Reedline, ExitCode> {
    let mut editor = reedline::Reedline::create()
        .with_validator(Box::new(AgentReplReedlineValidator))
        .with_completer(Box::new(AgentReplReedlineCompleter {
            context: agent_repl_completion_context(state),
        }));
    let history_path = agent_repl_history_path().map_err(|error| {
        eprintln!("error: failed to create Agent REPL history directory: {error}");
        ExitCode::FAILURE
    })?;
    if let Some(path) = history_path {
        let history = reedline::FileBackedHistory::with_file(512, path).map_err(|error| {
            eprintln!("error: failed to open Agent REPL history: {error}");
            ExitCode::FAILURE
        })?;
        editor = editor.with_history(Box::new(history));
    }
    Ok(editor)
}

#[cfg(feature = "agent-repl")]
pub(super) fn agent_repl_history_path() -> std::io::Result<Option<PathBuf>> {
    let Some(home) = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
    else {
        return Ok(None);
    };
    let dir = home.join(".arcweft");
    std::fs::create_dir_all(&dir)?;
    Ok(Some(dir.join("agent-repl-history.txt")))
}

#[cfg(feature = "agent-repl")]
pub(super) struct AgentReplReedlineValidator;

#[cfg(feature = "agent-repl")]
impl reedline::Validator for AgentReplReedlineValidator {
    fn validate(&self, line: &str) -> reedline::ValidationResult {
        match agent_repl_classify_cell(line).completion.kind {
            AgentReplCellCompletionKind::Incomplete => reedline::ValidationResult::Incomplete,
            _ => reedline::ValidationResult::Complete,
        }
    }
}

#[cfg(feature = "agent-repl")]
pub(super) struct AgentReplReedlineCompleter {
    pub(super) context: AgentReplCompletionContext,
}

#[cfg(feature = "agent-repl")]
impl reedline::Completer for AgentReplReedlineCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<reedline::Suggestion> {
        let prefix = &line[..pos.min(line.len())];
        let replacement_start = agent_repl_completion_replacement_start(prefix);
        agent_repl_completions(prefix, &self.context)
            .into_iter()
            .map(|item| reedline::Suggestion {
                value: item.insert_text.unwrap_or(item.label.clone()),
                display_override: Some(item.label),
                description: item.detail,
                span: reedline::Span::new(replacement_start, pos.min(line.len())),
                ..reedline::Suggestion::default()
            })
            .collect()
    }
}

#[cfg(feature = "agent-repl")]
pub(super) fn agent_repl_completion_replacement_start(source_before_cursor: &str) -> usize {
    source_before_cursor
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            (!(character.is_alphanumeric()
                || matches!(character, '_' | '.' | ':' | '@' | '-' | '/')))
            .then_some(index + character.len_utf8())
        })
        .unwrap_or(0)
}

pub(super) fn agent_repl_finish_debug_session(
    state: &mut AgentReplState,
    ok: bool,
) -> Result<(), ExitCode> {
    let Some(store) = &state.debug_store else {
        return Ok(());
    };
    let mut metadata = BTreeMap::new();
    metadata.insert("cells".to_owned(), serde_json::json!(state.history.len()));
    metadata.insert(
        "persisted_cells".to_owned(),
        serde_json::json!(state.persisted_cells),
    );
    metadata.insert("read_only".to_owned(), serde_json::json!(state.read_only));
    if let Some(report) = &state.report {
        metadata.insert("final_tick".to_owned(), serde_json::json!(report.tick));
    }
    if let Some(connection) = &state.connection {
        metadata.insert(
            "connection".to_owned(),
            serde_json::to_value(connection).map_err(|error| {
                eprintln!("error: failed to serialize REPL session metadata: {error}");
                ExitCode::FAILURE
            })?,
        );
    }
    agent_debug_finish_runtime_session(
        store,
        &agent_cli_session_id(),
        if ok {
            DebugSessionStatus::Finished
        } else {
            DebugSessionStatus::Failed
        },
        &metadata,
        "REPL debug database session",
    )
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })
}

pub(super) fn agent_repl_start_debug_session(store: &DebugStore) -> Result<(), ExitCode> {
    agent_debug_start_runtime_session(
        store,
        agent_cli_session_id(),
        None,
        "repl",
        "cli",
        BTreeMap::new(),
        "REPL debug database session",
    )
    .map(|_| ())
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })
}

pub(super) fn agent_repl_initial_connection(
    options: &AgentReplOptions,
) -> Result<Option<AgentReplConnection>, ExitCode> {
    let Some(target) = options.connect.as_deref() else {
        return Ok(None);
    };
    if options.read_only {
        eprintln!("error: agent repl --connect is not available with --read-only");
        return Err(ExitCode::from(2));
    }
    agent_repl_parse_connection(target, options).map_err(|message| {
        eprintln!("error: {message}");
        ExitCode::from(2)
    })
}

pub(super) fn agent_repl_debug_store(
    options: &AgentReplOptions,
) -> Result<(Option<DebugStore>, Option<String>), ExitCode> {
    let Some(path) = &options.debug_db else {
        return Ok((None, None));
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            eprintln!("error: failed to create {}: {error}", parent.display());
            ExitCode::FAILURE
        })?;
    }
    let store = DebugStore::open(path).map_err(|error| {
        eprintln!(
            "error: failed to open REPL debug database {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })?;
    agent_repl_start_debug_session(&store)?;
    Ok((Some(store), Some(path.display().to_string())))
}

pub(super) struct AgentReplTraceResources {
    pub(super) path: Option<String>,
    pub(super) record_count: usize,
    pub(super) resources: Vec<AgentResource>,
}

pub(super) fn agent_repl_trace_resources(
    options: &AgentReplOptions,
) -> Result<AgentReplTraceResources, ExitCode> {
    match (&options.trace, options.read_only) {
        (None, true) => {
            eprintln!("error: agent repl --read-only requires --trace <file.arcwx>");
            Err(ExitCode::FAILURE)
        }
        (None, false) => Ok(AgentReplTraceResources {
            path: None,
            record_count: 0,
            resources: Vec::new(),
        }),
        (Some(path), _) => {
            let records = super::read_and_validate_agent_trace_records(path).map_err(|error| {
                eprintln!("{}: {error}", path.display());
                ExitCode::FAILURE
            })?;
            let resource = agent_repl_trace_resource(&records).map_err(|error| {
                eprintln!("{}: {error}", path.display());
                ExitCode::FAILURE
            })?;
            Ok(AgentReplTraceResources {
                path: Some(path.display().to_string()),
                record_count: records.len(),
                resources: vec![resource],
            })
        }
    }
}

pub(super) fn agent_repl_trace_resource(
    records: &[AgentTraceRecord],
) -> Result<AgentResource, String> {
    trace_resource(records).map_err(|error| format!("failed to serialize trace: {error}"))
}

pub(super) fn agent_repl_input(options: &AgentReplOptions) -> Result<String, ExitCode> {
    if let Some(path) = &options.input {
        return fs::read_to_string(path).map_err(|error| {
            eprintln!("error: failed to read {}: {error}", path.display());
            ExitCode::FAILURE
        });
    }
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| {
            eprintln!("error: failed to read REPL stdin: {error}");
            ExitCode::FAILURE
        })?;
    Ok(input)
}

pub(super) fn agent_repl_eval_line(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    if input.starts_with(':') {
        return agent_repl_eval_meta(index, input, options, state, adapter_registrars);
    }
    if state.read_only {
        return agent_repl_error(
            index,
            input,
            "cell",
            "read-only Agent REPL does not execute Agent cells".to_owned(),
        );
    }
    agent_repl_eval_compiled_cell(index, input, state)
}

pub(super) fn agent_repl_eval_meta(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    let mut parts = input.splitn(2, char::is_whitespace);
    let command = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or_default().trim();
    if state.read_only && agent_repl_read_only_rejects(command) {
        return agent_repl_error(
            index,
            input,
            "meta",
            format!("read-only Agent REPL does not execute `{command}`"),
        );
    }
    match command {
        ":help" => agent_repl_help(index, input),
        ":trace" => agent_repl_trace(index, input, state),
        ":observe" => agent_repl_observe(index, input, options, state, adapter_registrars),
        ":actions" => agent_repl_actions(index, input, state),
        ":type" | ":ast" | ":hir" | ":bytecode" | ":capture" | ":complete" | ":highlight" => {
            agent_repl_eval_inspection_meta(
                index,
                input,
                command,
                rest,
                options,
                state,
                adapter_registrars,
            )
        }
        ":query" => agent_repl_eval_query_meta(index, input, rest, state),
        ":history" => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({ "cells": &state.history }),
        ),
        ":bindings" => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "bindings": state.bindings.values().collect::<Vec<_>>(),
            }),
        ),
        ":connect" => agent_repl_connect(index, input, rest, options, state),
        ":drop" | ":load" | ":save" | ":parse" | ":classify" => {
            agent_repl_eval_binding_meta(index, input, command, rest, state)
        }
        ":reset" => {
            state.report = None;
            state.remote_session = None;
            state.history.clear();
            state.bindings.clear();
            state.connection = None;
            state.persisted_cells = 0;
            agent_repl_ok(index, input, "meta", serde_json::json!({ "reset": true }))
        }
        ":quit" => AgentReplCellReport {
            index,
            input: input.to_owned(),
            kind: "meta".to_owned(),
            status: "ok".to_owned(),
            message: None,
            value: Some(serde_json::json!({ "quit": true })),
            quit: true,
        },
        _ => agent_repl_error(
            index,
            input,
            "meta",
            format!("unknown Agent REPL command `{command}`"),
        ),
    }
}

pub(super) fn agent_repl_read_only_rejects(command: &str) -> bool {
    matches!(
        command,
        ":observe" | ":capture" | ":connect" | ":drop" | ":load" | ":save" | ":reset"
    )
}

pub(super) fn agent_repl_eval_inspection_meta(
    index: usize,
    input: &str,
    command: &str,
    rest: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    match command {
        ":type" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":type requires an expression".to_owned(),
        ),
        ":type" => agent_repl_type(index, input, rest),
        ":ast" if rest.is_empty() => {
            agent_repl_error(index, input, "meta", ":ast requires a fragment".to_owned())
        }
        ":ast" => agent_repl_ast(index, input, rest),
        ":hir" if rest.is_empty() => {
            agent_repl_error(index, input, "meta", ":hir requires a fragment".to_owned())
        }
        ":hir" => agent_repl_hir(index, input, rest),
        ":bytecode" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":bytecode requires a fragment".to_owned(),
        ),
        ":bytecode" => agent_repl_bytecode(index, input, rest),
        ":capture" => agent_repl_capture(index, input, rest, options, state, adapter_registrars),
        ":complete" => agent_repl_complete(index, input, rest, state),
        ":highlight" => agent_repl_highlight(index, input, rest),
        _ => agent_repl_error(
            index,
            input,
            "meta",
            format!("unknown Agent REPL command `{command}`"),
        ),
    }
}

pub(super) fn agent_repl_eval_query_meta(
    index: usize,
    input: &str,
    rest: &str,
    state: &AgentReplState,
) -> AgentReplCellReport {
    if rest.is_empty() {
        agent_repl_error(index, input, "meta", ":query requires text".to_owned())
    } else {
        agent_repl_query(index, input, rest, state)
    }
}

pub(super) fn agent_repl_eval_binding_meta(
    index: usize,
    input: &str,
    command: &str,
    rest: &str,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    match command {
        ":drop" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":drop requires a binding name".to_owned(),
        ),
        ":drop" => agent_repl_drop(index, input, rest, state),
        ":load" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":load requires a .awfagent path".to_owned(),
        ),
        ":load" => agent_repl_load(index, input, rest, state),
        ":save" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":save requires a .awfagent path".to_owned(),
        ),
        ":save" => agent_repl_save(index, input, rest, state),
        ":parse" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":parse requires a fragment".to_owned(),
        ),
        ":classify" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":classify requires a fragment".to_owned(),
        ),
        ":parse" | ":classify" => agent_repl_ok(index, input, "meta", agent_repl_parse_cell(rest)),
        _ => agent_repl_error(
            index,
            input,
            "meta",
            format!("unknown Agent REPL command `{command}`"),
        ),
    }
}

pub(super) fn agent_repl_help(index: usize, input: &str) -> AgentReplCellReport {
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "commands": [
                ":help",
                ":type EXPR",
                ":ast EXPR_OR_STMT",
                ":hir EXPR_OR_STMT",
                ":bytecode EXPR_OR_STMT",
                ":observe",
                ":actions",
                ":trace",
                ":capture [viewport|layer ID|object ID]",
                ":query TEXT",
                ":history",
                ":bindings",
                ":drop NAME",
                ":load FILE.awfagent",
                ":save FILE.awfagent",
                ":parse EXPR_OR_STMT",
                ":classify EXPR_OR_STMT",
                ":reset",
                ":connect current|source PATH|profile ID [--manifest PATH]",
                ":complete SOURCE_BEFORE_CURSOR",
                ":highlight SOURCE",
                ":quit"
            ],
            "startup_options": [
                "--connect current|source PATH|profile ID [--manifest PATH]"
            ]
        }),
    )
}

pub(super) fn agent_repl_trace(
    index: usize,
    input: &str,
    state: &AgentReplState,
) -> AgentReplCellReport {
    let descriptors = agent_publish_resources(state.trace_resources.clone())
        .map(|resources| list_resources_result(&resources).resources)
        .unwrap_or_default();
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "loaded": !state.trace_resources.is_empty(),
            "path": state.trace_path.clone(),
            "record_count": state.trace_records,
            "resource_count": descriptors.len(),
            "resources": descriptors,
            "read_only": state.read_only,
        }),
    )
}

pub(super) fn agent_repl_observe(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    if state.remote_session.is_some() {
        return agent_repl_remote_observe(index, input, state);
    }
    match agent_observation_report_for_options(
        &agent_repl_observe_options(options, state),
        adapter_registrars,
    ) {
        Ok(report) => {
            let value = serde_json::json!({
                "tick": report.tick,
                "frame_id": report.frame_id,
                "state_hash": report.state_hash,
                "render_hash": report.render_hash,
                "actions": report.actions.len(),
                "objects": report.objects.len(),
                "diagnostics": report.diagnostics.len(),
            });
            state.report = Some(report);
            agent_repl_ok(index, input, "meta", value)
        }
        Err(code) => agent_repl_error(
            index,
            input,
            "meta",
            format!("observe failed with exit code {code:?}"),
        ),
    }
}

pub(super) fn agent_repl_actions(
    index: usize,
    input: &str,
    state: &AgentReplState,
) -> AgentReplCellReport {
    match &state.report {
        Some(report) => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "tick": report.tick,
                "actions": report.actions,
            }),
        ),
        None => agent_repl_error(
            index,
            input,
            "meta",
            ":actions requires :observe first".to_owned(),
        ),
    }
}

pub(super) fn agent_repl_complete(
    index: usize,
    input: &str,
    source_before_cursor: &str,
    state: &AgentReplState,
) -> AgentReplCellReport {
    if source_before_cursor.is_empty() {
        return agent_repl_error(
            index,
            input,
            "meta",
            ":complete requires source text before the cursor".to_owned(),
        );
    }
    let context = agent_repl_completion_context(state);
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "source": source_before_cursor,
            "items": agent_repl_completions(source_before_cursor, &context),
        }),
    )
}

pub(super) fn agent_repl_highlight(index: usize, input: &str, source: &str) -> AgentReplCellReport {
    if source.is_empty() {
        return agent_repl_error(
            index,
            input,
            "meta",
            ":highlight requires source text".to_owned(),
        );
    }
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "source": source,
            "tokens": agent_repl_highlight_tokens(source),
        }),
    )
}

pub(super) fn agent_repl_completion_context(state: &AgentReplState) -> AgentReplCompletionContext {
    let mut context = AgentReplCompletionContext {
        live_bindings: state.bindings.keys().cloned().collect(),
        ..AgentReplCompletionContext::default()
    };
    let Some(report) = &state.report else {
        return context;
    };
    context.entities = agent_repl_completion_entities(report);
    context.action_targets = report
        .actions
        .iter()
        .map(|action| action.target.clone())
        .collect();
    context.layer_ids = report.layers.iter().map(|layer| layer.id.clone()).collect();
    context.object_ids = report
        .objects
        .iter()
        .map(|object| object.id.clone())
        .collect();
    context.effect_capabilities = report
        .presentation_tree
        .nodes
        .iter()
        .flat_map(|node| node.effects.iter().map(|effect| effect.id.clone()))
        .collect();
    context
}

pub(super) fn agent_repl_completion_entities(
    report: &AgentObservationReport,
) -> Vec<AgentReplCompletionEntity> {
    report
        .actions
        .iter()
        .filter_map(|action| match action.action {
            AgentActionKind::SelectChoice => Some(AgentReplCompletionEntity {
                id: action.target.clone(),
                kind: "choice_option".to_owned(),
            }),
            AgentActionKind::AdvanceText
            | AgentActionKind::Invoke
            | AgentActionKind::PointerClick => None,
        })
        .collect()
}

pub(super) fn agent_repl_connect(
    index: usize,
    input: &str,
    target: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    let target = target.trim();
    if target.is_empty() {
        return agent_repl_error(
            index,
            input,
            "meta",
            ":connect requires a target".to_owned(),
        );
    }

    let connection = match agent_repl_parse_connection(target, options) {
        Ok(connection) => connection,
        Err(message) => return agent_repl_error(index, input, "meta", message),
    };
    agent_repl_apply_connection(index, input, connection, state)
}

pub(super) fn agent_repl_apply_connection(
    index: usize,
    input: &str,
    connection: Option<AgentReplConnection>,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    let remote_connection = match agent_repl_connect_remote_session(connection.as_ref()) {
        Ok(session) => session,
        Err(message) => return agent_repl_error(index, input, "meta", message),
    };
    let old_program_hash = state.remote_program_hash.clone();
    let new_program_hash = remote_connection.program_hash.clone();
    let binding_policy = agent_repl_reconcile_project_bound_bindings(
        state,
        old_program_hash.as_deref(),
        new_program_hash.as_deref(),
    );
    state.connection = connection;
    state.remote_session = remote_connection.session;
    state.remote_program_hash.clone_from(&new_program_hash);
    state.report = None;

    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "connected": true,
            "connection": state.connection,
            "program_hash": state.remote_program_hash,
            "binding_policy": {
                "old_program_hash": old_program_hash,
                "new_program_hash": new_program_hash,
                "program_hash_changed": matches!(
                    (
                        old_program_hash.as_deref(),
                        state.remote_program_hash.as_deref()
                    ),
                    (Some(old), Some(new)) if old != new
                ),
                "decisions": binding_policy,
            },
        }),
    )
}

pub(super) fn agent_repl_parse_connection(
    target: &str,
    options: &AgentReplOptions,
) -> Result<Option<AgentReplConnection>, String> {
    if target == "current" {
        if options.path.is_none() && options.profile.profile.is_none() {
            return Err(
                ":connect current requires the REPL to start with a source path or --profile"
                    .to_owned(),
            );
        }
        return Ok(None);
    }
    if let Some(endpoint) = target.strip_prefix("stdio:") {
        return agent_repl_parse_stdio_mcp_connection(endpoint);
    }
    if let Some(endpoint) = target.strip_prefix("mcp:") {
        return agent_repl_parse_stdio_mcp_connection(endpoint);
    }
    if let Some(endpoint) = target.strip_prefix("stdio ") {
        return agent_repl_parse_stdio_mcp_connection(endpoint);
    }
    if let Some(path) = target.strip_prefix("source ") {
        let path = path.trim();
        if path.is_empty() {
            return Err(":connect source requires a path".to_owned());
        }
        return Ok(Some(AgentReplConnection::Source {
            path: path.to_owned(),
        }));
    }
    if let Some(rest) = target.strip_prefix("profile ") {
        return agent_repl_parse_profile_connection(rest, options);
    }
    if Path::new(target)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("arcw"))
    {
        return Ok(Some(AgentReplConnection::Source {
            path: target.to_owned(),
        }));
    }
    Ok(Some(AgentReplConnection::Profile {
        id: target.to_owned(),
        manifest: options.profile.manifest.display().to_string(),
    }))
}

fn agent_repl_connect_remote_session(
    connection: Option<&AgentReplConnection>,
) -> Result<AgentReplRemoteConnection, String> {
    let Some(AgentReplConnection::StdioMcp { program, args }) = connection else {
        return Ok(AgentReplRemoteConnection {
            session: None,
            program_hash: None,
        });
    };
    let endpoint = StdioMcpEndpoint {
        program: program.clone(),
        args: args.clone(),
    };
    let transport = StdioMcpTransport::spawn(&endpoint)
        .map_err(|error| format!("failed to connect MCP stdio endpoint `{endpoint}`: {error}"))?;
    let mut session = McpAgentSession::connect(transport, ConnectOptions::default())
        .map_err(|error| format!("failed MCP stdio handshake for `{endpoint}`: {error}"))?;
    let info = session
        .info()
        .map_err(|error| format!("failed MCP stdio session info for `{endpoint}`: {error}"))?;
    Ok(AgentReplRemoteConnection {
        session: Some(session),
        program_hash: Some(info.program_hash),
    })
}

fn agent_repl_parse_stdio_mcp_connection(
    endpoint: &str,
) -> Result<Option<AgentReplConnection>, String> {
    let mut parts = endpoint.split_whitespace();
    let program = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| ":connect stdio requires an executable name".to_owned())?;
    if program.contains(['|', '>', '<', '&', ';']) {
        return Err(
            ":connect stdio accepts an executable plus whitespace-separated args, not shell syntax"
                .to_owned(),
        );
    }
    let args = parts
        .map(|part| {
            if part.contains(['|', '>', '<', '&', ';']) {
                Err(format!(
                    "unsupported shell metacharacter in stdio arg `{part}`"
                ))
            } else {
                Ok(part.to_owned())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(AgentReplConnection::StdioMcp {
        program: program.to_owned(),
        args,
    }))
}

pub(super) fn agent_repl_parse_profile_connection(
    rest: &str,
    options: &AgentReplOptions,
) -> Result<Option<AgentReplConnection>, String> {
    let mut parts = rest.split_whitespace();
    let id = parts
        .next()
        .ok_or_else(|| ":connect profile requires an id".to_owned())?;
    let mut manifest = options.profile.manifest.display().to_string();
    while let Some(flag) = parts.next() {
        match flag {
            "--manifest" => {
                parts
                    .next()
                    .ok_or_else(|| ":connect profile --manifest requires a path".to_owned())?
                    .clone_into(&mut manifest);
            }
            _ => return Err(format!("unsupported :connect profile option `{flag}`")),
        }
    }
    Ok(Some(AgentReplConnection::Profile {
        id: id.to_owned(),
        manifest,
    }))
}

pub(super) fn agent_repl_type(
    index: usize,
    input: &str,
    fragment_source: &str,
) -> AgentReplCellReport {
    let fragment = agent_repl_parse_fragment(fragment_source);
    let parse = agent_repl_fragment_report(&fragment);
    let compiled = match agent_repl_compile_fragment(index, fragment_source, &fragment) {
        Ok(compiled) => compiled,
        Err(message) => return agent_repl_error(index, input, "meta", message),
    };
    let Some(ty) = agent_repl_display_type(&compiled.typecheck_report) else {
        return agent_repl_error(
            index,
            input,
            "meta",
            "type-check succeeded, but no displayable expression type judgment was produced"
                .to_owned(),
        );
    };
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "type": ty,
            "parse": parse,
        }),
    )
}

pub(super) fn agent_repl_ast(
    index: usize,
    input: &str,
    fragment_source: &str,
) -> AgentReplCellReport {
    let fragment = agent_repl_parse_fragment(fragment_source);
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "parse": agent_repl_fragment_report(&fragment),
            "ast": fragment.kind().map(|kind| format!("{kind:#?}")),
        }),
    )
}

pub(super) fn agent_repl_hir(
    index: usize,
    input: &str,
    fragment_source: &str,
) -> AgentReplCellReport {
    let fragment = agent_repl_parse_fragment(fragment_source);
    let Some(source) = agent_repl_inspection_source(index, fragment_source, &fragment) else {
        return agent_repl_error(
            index,
            input,
            "meta",
            "fragment is not complete enough to lower to HIR".to_owned(),
        );
    };
    match arcweft_compiler::agent::compile_agent_source(source) {
        Ok(compiled) => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "parse": agent_repl_fragment_report(&fragment),
                "hir": format!("{:#?}", compiled.hir),
            }),
        ),
        Err(error) => agent_repl_error(index, input, "meta", error.to_string()),
    }
}

pub(super) fn agent_repl_bytecode(
    index: usize,
    input: &str,
    fragment_source: &str,
) -> AgentReplCellReport {
    let fragment = agent_repl_parse_fragment(fragment_source);
    let compiled = match agent_repl_compile_fragment(index, fragment_source, &fragment) {
        Ok(compiled) => compiled,
        Err(message) => return agent_repl_error(index, input, "meta", message),
    };
    let stats = compiled.bundle.bytecode.program.stats();
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "parse": agent_repl_fragment_report(&fragment),
            "agent_id": compiled.manifest.agent_id.as_str(),
            "entry_flow": compiled.bundle.bytecode.program.entry_flow.as_ref().map(|flow| flow.0.as_str()),
            "stats": {
                "flows": stats.flows,
                "instructions": stats.instructions,
                "line_task_groups": stats.line_task_groups,
                "stream_plans": stats.stream_plans,
                "source_plans": stats.source_plans,
            },
            "program": format!("{:#?}", compiled.bundle.bytecode.program),
        }),
    )
}

pub(super) fn agent_repl_capture(
    index: usize,
    input: &str,
    target: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    if state.remote_session.is_some() {
        return agent_repl_remote_capture(index, input, target, state);
    }
    let mut observe_options = agent_repl_observe_options(options, state);
    observe_options.image = Some(AgentObserveImageKind::Png);
    observe_options.capture = Some(AgentObserveCaptureKind::Color);
    if let Err(message) = apply_agent_repl_capture_target(target, &mut observe_options) {
        return agent_repl_error(index, input, "meta", message);
    }
    match agent_observation_for_options(&observe_options, adapter_registrars).and_then(
        |mut observed| {
            let image_output = agent_observe_image_output(
                &mut observed.report,
                &observe_options,
                Some(&mut observed.native_session),
                &observed.image_frames,
            )?;
            let resource = agent_observe_image_resource(&observed.report, image_output.as_ref());
            Ok((observed.report, resource))
        },
    ) {
        Ok((report, resource)) => {
            let resource_summary = resource.as_ref().map(|resource| {
                serde_json::json!({
                    "uri": resource.uri,
                    "kind": resource.kind,
                    "mime_type": resource.mime_type,
                    "hash": resource.hash,
                })
            });
            agent_repl_ok(
                index,
                input,
                "meta",
                serde_json::json!({
                    "tick": report.tick,
                    "frame_id": report.frame_id,
                    "images": report.images,
                    "resource": resource_summary,
                }),
            )
        }
        Err(code) => agent_repl_error(
            index,
            input,
            "meta",
            format!("capture failed with exit code {code:?}"),
        ),
    }
}

fn agent_repl_remote_observe(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    let Some(session) = state.remote_session.as_mut() else {
        return agent_repl_error(
            index,
            input,
            "meta",
            "remote MCP session is not connected".to_owned(),
        );
    };
    match session.observe(ObserveRequest::default()) {
        Ok(observation) => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "remote": true,
                "tick": observation.tick,
                "frame_id": observation.frame_id,
                "state_hash": observation.state_hash,
                "render_hash": observation.render_hash,
                "actions": observation.actions.len(),
                "signals": observation.signals.len(),
            }),
        ),
        Err(error) => agent_repl_error(index, input, "meta", error.to_string()),
    }
}

fn agent_repl_remote_capture(
    index: usize,
    input: &str,
    target: &str,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    let request = match agent_repl_remote_capture_request(index, target) {
        Ok(request) => request,
        Err(message) => return agent_repl_error(index, input, "meta", message),
    };
    let Some(session) = state.remote_session.as_mut() else {
        return agent_repl_error(
            index,
            input,
            "meta",
            "remote MCP session is not connected".to_owned(),
        );
    };
    match session.capture(request) {
        Ok(capture) => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "remote": true,
                "uri": capture.uri,
                "content_hash": capture.content_hash,
                "media_type": capture.media_type,
                "byte_len": capture.byte_len,
            }),
        ),
        Err(error) => agent_repl_error(index, input, "meta", error.to_string()),
    }
}

fn agent_repl_remote_capture_request(index: usize, target: &str) -> Result<CaptureRequest, String> {
    let target = target.trim();
    let target = if target.is_empty() || target == "viewport" {
        CaptureTarget::Viewport
    } else {
        let mut parts = target.split_whitespace();
        let kind = parts.next().unwrap_or_default();
        let id = parts
            .next()
            .ok_or_else(|| format!(":capture {kind} requires an id"))?;
        if parts.next().is_some() {
            return Err(":capture accepts only viewport, layer ID, or object ID".to_owned());
        }
        match kind {
            "layer" => CaptureTarget::Layer {
                id: AgentPublicId::new(id.to_owned())
                    .map_err(|error| format!("invalid layer id `{id}`: {error}"))?,
            },
            "object" => CaptureTarget::Object { id: id.to_owned() },
            _ => return Err(":capture accepts only viewport, layer ID, or object ID".to_owned()),
        }
    };
    Ok(CaptureRequest {
        target,
        format: CaptureFormat::Png,
        capture_kind: "color".to_owned(),
        name: format!("repl.capture.{index}"),
    })
}

pub(super) fn agent_repl_query(
    index: usize,
    input: &str,
    query: &str,
    state: &AgentReplState,
) -> AgentReplCellReport {
    if state.report.is_none() && state.trace_resources.is_empty() {
        return agent_repl_error(
            index,
            input,
            "meta",
            ":query requires :observe or --trace first".to_owned(),
        );
    }
    let mcp_state = AgentMcpState {
        report: state.report.clone(),
        image_output: None,
        image_frames: AgentImageFrameStore::default(),
        capture_resources: Vec::new(),
        trace_resources: state.trace_resources.clone(),
        rag_context_packs: Vec::new(),
        project_context: None,
        native_capture_session: None,
        runtime: None,
        observe_options: None,
    };
    match agent_mcp_rag_context_pack(
        &mcp_state,
        query,
        Vec::new(),
        1,
        8,
        32 * 1024,
        PrivacyClass::Project,
    ) {
        Ok(pack) => {
            let value = serde_json::to_value(pack)
                .unwrap_or_else(|error| serde_json::json!({ "error": error.to_string() }));
            agent_repl_ok(index, input, "meta", value)
        }
        Err(error) => agent_repl_error(index, input, "meta", error),
    }
}

pub(super) fn agent_repl_drop(
    index: usize,
    input: &str,
    name: &str,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    match state.bindings.remove(name) {
        Some(binding) => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "dropped": binding.name,
                "binding_kind": binding.binding_kind,
            }),
        ),
        None => agent_repl_error(
            index,
            input,
            "meta",
            format!("REPL binding `{name}` does not exist"),
        ),
    }
}

pub(super) fn agent_repl_load(
    index: usize,
    input: &str,
    raw_path: &str,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    let path = PathBuf::from(raw_path);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            return agent_repl_error(
                index,
                input,
                "meta",
                format!("failed to read {}: {error}", path.display()),
            );
        }
    };
    if let Err(error) = arcweft_compiler::agent::compile_agent_source(source.clone()) {
        return agent_repl_error(
            index,
            input,
            "meta",
            format!("failed to load {}: {error}", path.display()),
        );
    }

    let name = agent_repl_loaded_binding_name(&path, index);
    state.bindings.insert(
        name.clone(),
        AgentReplBinding {
            name: name.clone(),
            binding_kind: "loaded_agent".to_owned(),
            source,
            status: "ok".to_owned(),
            final_status: None,
            host_calls: 0,
            responses: 0,
            serializable: false,
            serialized_source: None,
            snapshot_kind: None,
            non_serializable_reason: Some("loaded Agent source is not a REPL value".to_owned()),
        },
    );
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "loaded": path.display().to_string(),
            "binding": name,
        }),
    )
}

pub(super) fn agent_repl_save(
    index: usize,
    input: &str,
    raw_path: &str,
    state: &AgentReplState,
) -> AgentReplCellReport {
    let path = PathBuf::from(raw_path);
    let source = agent_repl_saved_source(state);
    if let Err(error) = arcweft_compiler::agent::compile_agent_source(source.clone()) {
        return agent_repl_error(
            index,
            input,
            "meta",
            format!("refusing to save invalid Agent source: {error}"),
        );
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return agent_repl_error(
            index,
            input,
            "meta",
            format!("failed to create {}: {error}", parent.display()),
        );
    }
    match fs::write(&path, source.as_bytes()) {
        Ok(()) => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "saved": path.display().to_string(),
                "bytes": source.len(),
            }),
        ),
        Err(error) => agent_repl_error(
            index,
            input,
            "meta",
            format!("failed to write {}: {error}", path.display()),
        ),
    }
}

pub(super) fn agent_repl_loaded_binding_name(path: &Path, fallback_index: usize) -> String {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("agent");
    let sanitized = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("loaded.{fallback_index}.{sanitized}")
}

pub(super) fn agent_repl_saved_source(state: &AgentReplState) -> String {
    let mut body = state
        .bindings
        .values()
        .filter(|binding| binding.status == "ok")
        .filter(|binding| binding.binding_kind == "cell")
        .map(|binding| indent_agent_repl_body(&binding.source))
        .collect::<Vec<_>>()
        .join("\n");
    if body.trim().is_empty() {
        "    return \"empty\"".clone_into(&mut body);
    } else if !body.contains("\n    return ") && !body.starts_with("    return ") {
        body.push_str("\n    return \"saved\"");
    }
    format!(
        "#[agent(version = 1)]\nagent @agent.repl.saved repl_saved()\neffects {{ agent.observe, agent.act.semantic, agent.act.physical, agent.wait, agent.capture, agent.resource.read, debug.read, debug.record, rag.query }}\n{{\n{body}\n}}\n"
    )
}

pub(super) fn agent_repl_observe_options(
    options: &AgentReplOptions,
    state: &AgentReplState,
) -> AgentObserveOptions {
    let mut observe_options = AgentObserveOptions {
        path: options.path.clone(),
        profile: options.profile.clone(),
        entry: options.entry.clone(),
        flow: options.flow.clone(),
        executor: options.executor,
        pure_backend: options.pure_backend,
        pure_workers: options.pure_workers,
        pure_batch_min_len: options.pure_batch_min_len,
        pure_object_artifacts: options.pure_object_artifacts,
        math_backend: options.math_backend,
        math_wgpu_min_elements: options.math_wgpu_min_elements,
        steps: options.steps,
        capture_step: options.capture_step,
        mode: options.mode,
        max_ops: options.max_ops,
        values: options.values.clone(),
        viewport_width: options.viewport_width,
        viewport_height: options.viewport_height,
        textbox_height: options.textbox_height,
        image: None,
        capture: None,
        layer: None,
        object: None,
        page: None,
        capture_time_seconds: options.capture_time_seconds,
        resource: None,
        read_uri: None,
        mcp: false,
        mcp_format: AgentObserveMcpFormat::Read,
        out: None,
        json: true,
    };
    match &state.connection {
        Some(AgentReplConnection::Source { path }) => {
            observe_options.path = Some(PathBuf::from(path));
            observe_options.profile.profile = None;
        }
        Some(AgentReplConnection::Profile { id, manifest }) => {
            observe_options.path = None;
            observe_options.profile.profile = Some(id.clone());
            observe_options.profile.manifest = PathBuf::from(manifest);
        }
        Some(AgentReplConnection::StdioMcp { .. }) | None => {}
    }
    observe_options
}

pub(super) fn agent_repl_parse_cell(input: &str) -> serde_json::Value {
    serde_json::to_value(agent_repl_classify_cell(input))
        .unwrap_or_else(|error| serde_json::json!({ "error": error.to_string() }))
}

pub(super) fn agent_repl_fragment_report(fragment: &ParsedFragment) -> serde_json::Value {
    serde_json::to_value(agent_repl_classification_from_fragment(fragment))
        .unwrap_or_else(|error| serde_json::json!({ "error": error.to_string() }))
}

pub(super) fn agent_repl_compile_fragment(
    index: usize,
    input: &str,
    fragment: &ParsedFragment,
) -> Result<arcweft_compiler::types::CompiledAgentBundle, String> {
    let source = agent_repl_inspection_source(index, input, fragment)
        .ok_or_else(|| "fragment is not complete enough to compile".to_owned())?;
    let project = agent_script_project_index(&[])?;
    arcweft_compiler::agent::compile_agent_bundle_with_project(source, &project)
        .map_err(|error| error.to_string())
}

pub(super) fn agent_repl_inspection_source(
    index: usize,
    input: &str,
    fragment: &ParsedFragment,
) -> Option<String> {
    (matches!(fragment.completion(), ParseCompletion::Complete) && fragment.errors().is_empty())
        .then(|| agent_repl_cell_source(index, input, fragment, ""))
}

pub(super) fn agent_repl_display_type(report: &TypeCheckReport) -> Option<String> {
    report
        .judgments
        .iter()
        .rev()
        .find(|judgment| judgment.rule == TypeJudgmentRule::Return)
        .or_else(|| report.judgments.iter().next_back())
        .map(|judgment| format!("{:?}", judgment.ty))
}

pub(super) fn apply_agent_repl_capture_target(
    target: &str,
    options: &mut AgentObserveOptions,
) -> Result<(), String> {
    let target = target.trim();
    if target.is_empty() || target == "viewport" {
        return Ok(());
    }
    let mut parts = target.split_whitespace();
    let kind = parts.next().unwrap_or_default();
    let id = parts
        .next()
        .ok_or_else(|| format!(":capture {kind} requires an id"))?;
    if parts.next().is_some() {
        return Err(":capture accepts only viewport, layer ID, or object ID".to_owned());
    }
    match kind {
        "layer" => {
            options.layer = Some(id.to_owned());
            Ok(())
        }
        "object" => {
            options.object = Some(id.to_owned());
            Ok(())
        }
        _ => Err(":capture accepts only viewport, layer ID, or object ID".to_owned()),
    }
}

pub(super) fn agent_repl_eval_compiled_cell(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    let fragment = agent_repl_parse_fragment(input);
    let parse_report = agent_repl_fragment_report(&fragment);
    if !matches!(fragment.completion(), ParseCompletion::Complete) || !fragment.errors().is_empty()
    {
        return agent_repl_invalid_fragment_report(index, input, state, &fragment, parse_report);
    }

    let source = agent_repl_cell_source(
        index,
        input,
        &fragment,
        &agent_repl_live_binding_prelude(state),
    );
    let binding_names = agent_repl_fragment_binding_names(&fragment);
    let serialized_bindings = agent_repl_serialized_bindings(&fragment);
    let project = match agent_script_project_index(&[]) {
        Ok(project) => project,
        Err(error) => return agent_repl_error(index, input, "cell", error),
    };
    let compiled =
        match arcweft_compiler::agent::compile_agent_bundle_with_project(source, &project) {
            Ok(compiled) => compiled,
            Err(error) => {
                let message = error.to_string();
                return agent_repl_compile_error_report(
                    index,
                    input,
                    state,
                    parse_report,
                    &message,
                );
            }
        };
    let project_entities =
        match arcweft_compiler::agent_project::agent_required_entities_from_project(&project) {
            Ok(entities) => entities,
            Err(error) => return agent_repl_error(index, input, "cell", error.to_string()),
        };
    let project_graph =
        match arcweft_compiler::agent_project::agent_project_graph_from_project(&project) {
            Ok(graph) => graph,
            Err(error) => return agent_repl_error(index, input, "cell", error.to_string()),
        };
    if state.remote_session.is_some() {
        return agent_repl_eval_remote_compiled_cell(
            index,
            input,
            state,
            &parse_report,
            &binding_names,
            &serialized_bindings,
            &compiled.bundle,
        );
    }
    let mut runner = AgentRunner::new(
        CliAgentSession::new(
            Vec::new(),
            Vec::new(),
            project.program_hash().as_str().to_owned(),
            project_entities,
            project_graph,
        ),
        CollectingDebugSink::default(),
        NoopRagService,
        agent_script_runtime_policy_for_bundle(&compiled.bundle),
        AgentRunnerConfig::new(agent_cli_session_id()),
    );
    let run_result = runner.run_controller_bundle(
        &compiled.bundle,
        AgentControllerRunConfig {
            max_steps: 16,
            max_ops_per_step: 128,
        },
    );
    match run_result {
        Ok(report) => agent_repl_run_success_report(
            index,
            input,
            state,
            &parse_report,
            &report,
            &binding_names,
            &serialized_bindings,
        ),
        Err(error) => {
            let message = error.to_string();
            let effect_summary = agent_repl_run_effect_summary(&runner.debug_mut().events);
            agent_repl_run_error_report(
                index,
                input,
                state,
                &parse_report,
                &message,
                effect_summary,
            )
        }
    }
}

fn agent_repl_eval_remote_compiled_cell(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    parse_report: &serde_json::Value,
    binding_names: &[String],
    serialized_bindings: &BTreeMap<String, AgentReplSerializedBinding>,
    bundle: &ArcweftBundle,
) -> AgentReplCellReport {
    let Some(session) = state.remote_session.take() else {
        return agent_repl_error(
            index,
            input,
            "cell",
            "remote MCP session is not connected".to_owned(),
        );
    };
    let mut runner = AgentRunner::new(
        session,
        CollectingDebugSink::default(),
        NoopRagService,
        agent_script_runtime_policy_for_bundle(bundle),
        AgentRunnerConfig::new(agent_cli_session_id()),
    );
    let run_result = runner.run_controller_bundle(
        bundle,
        AgentControllerRunConfig {
            max_steps: 16,
            max_ops_per_step: 128,
        },
    );
    let effect_summary = run_result
        .as_ref()
        .err()
        .map(|_| agent_repl_run_effect_summary(&runner.debug_mut().events));
    state.remote_session = Some(runner.into_session());
    match run_result {
        Ok(report) => agent_repl_run_success_report(
            index,
            input,
            state,
            parse_report,
            &report,
            binding_names,
            serialized_bindings,
        ),
        Err(error) => agent_repl_run_error_report(
            index,
            input,
            state,
            parse_report,
            &error.to_string(),
            effect_summary.unwrap_or(AgentReplRunEffectSummary {
                host_calls: 0,
                events_emitted: 0,
                partially_effectful: false,
            }),
        ),
    }
}

pub(super) fn agent_repl_invalid_fragment_report(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    fragment: &ParsedFragment,
    parse_report: serde_json::Value,
) -> AgentReplCellReport {
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "cell".to_owned(),
        status: "error".to_owned(),
        message: Some("REPL cell is not a complete Agent fragment".to_owned()),
        value: Some(parse_report),
        quit: false,
    }
    .with_persistence(agent_repl_persist_cell(
        state,
        index,
        input,
        "error",
        serde_json::json!({
            "parse": fragment
                .errors()
                .iter()
                .map(|error| error.message().to_owned())
                .collect::<Vec<_>>()
        }),
        false,
    ))
}

pub(super) fn agent_repl_compile_error_report(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    parse_report: serde_json::Value,
    message: &str,
) -> AgentReplCellReport {
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "cell".to_owned(),
        status: "error".to_owned(),
        message: Some(message.to_owned()),
        value: Some(parse_report),
        quit: false,
    }
    .with_persistence(agent_repl_persist_cell(
        state,
        index,
        input,
        "error",
        serde_json::json!({ "compile_error": message }),
        false,
    ))
}

pub(super) fn agent_repl_run_success_report(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    parse_report: &serde_json::Value,
    report: &AgentControllerRunReport,
    binding_names: &[String],
    serialized_bindings: &BTreeMap<String, AgentReplSerializedBinding>,
) -> AgentReplCellReport {
    let binding_name = format!("cell.{index}");
    let final_status = report
        .final_status
        .as_ref()
        .map(|status| format!("{status:?}"));
    let responses = report.responses.len();
    let value = serde_json::json!({
        "binding": binding_name,
        "compiled": true,
        "steps": report.steps,
        "host_calls": report.host_calls,
        "responses": report.responses,
        "events_emitted": report.events_emitted,
        "final_status": final_status,
        "bindings": binding_names,
        "parse": parse_report,
    });
    if let Some(error) = agent_repl_unsupported_snapshot_error(binding_names, serialized_bindings) {
        return agent_repl_snapshot_escape_error_report(
            index,
            input,
            state,
            parse_report,
            &error,
            report.host_calls,
            report.events_emitted,
        );
    }
    let persisted = agent_repl_persist_cell(
        state,
        index,
        input,
        "ok",
        value.clone(),
        report.host_calls > 0,
    );
    state.bindings.insert(
        binding_name.clone(),
        AgentReplBinding {
            name: binding_name.clone(),
            binding_kind: "cell".to_owned(),
            source: input.to_owned(),
            status: "ok".to_owned(),
            final_status,
            host_calls: report.host_calls,
            responses,
            serializable: false,
            serialized_source: None,
            snapshot_kind: None,
            non_serializable_reason: Some("cell artifact is not a REPL value".to_owned()),
        },
    );
    for local_name in binding_names {
        let serialized = serialized_bindings.get(local_name);
        state.bindings.insert(
            local_name.clone(),
            AgentReplBinding {
                name: local_name.clone(),
                binding_kind: "local".to_owned(),
                source: input.to_owned(),
                status: "ok".to_owned(),
                final_status: state
                    .bindings
                    .get(&binding_name)
                    .and_then(|binding| binding.final_status.clone()),
                host_calls: report.host_calls,
                responses,
                serializable: serialized.is_some(),
                serialized_source: serialized.map(|binding| binding.source.clone()),
                snapshot_kind: serialized.map(|binding| binding.snapshot_kind.clone()),
                non_serializable_reason: serialized
                    .is_none()
                    .then(|| "binding value is not a supported REPL snapshot".to_owned()),
            },
        );
    }
    agent_repl_ok(index, input, "cell", value).with_persistence(persisted)
}

pub(super) fn agent_repl_unsupported_snapshot_error(
    binding_names: &[String],
    serialized_bindings: &BTreeMap<String, AgentReplSerializedBinding>,
) -> Option<String> {
    let unsupported = binding_names
        .iter()
        .filter(|name| !serialized_bindings.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();
    (!unsupported.is_empty()).then(|| {
        format!(
            "REPL local binding(s) cannot cross cells without a supported snapshot: {}",
            unsupported.join(", ")
        )
    })
}

pub(super) fn agent_repl_snapshot_escape_error_report(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    parse_report: &serde_json::Value,
    message: &str,
    host_calls: usize,
    events_emitted: u64,
) -> AgentReplCellReport {
    let value = serde_json::json!({
        "parse": parse_report,
        "snapshot_error": message,
        "host_calls": host_calls,
        "events_emitted": events_emitted,
        "partially_effectful": host_calls > 0,
    });
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "cell".to_owned(),
        status: "error".to_owned(),
        message: Some(message.to_owned()),
        value: Some(value.clone()),
        quit: false,
    }
    .with_persistence(agent_repl_persist_cell(
        state,
        index,
        input,
        "error",
        value,
        host_calls > 0,
    ))
}

pub(super) fn agent_repl_run_error_report(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    parse_report: &serde_json::Value,
    message: &str,
    effect_summary: AgentReplRunEffectSummary,
) -> AgentReplCellReport {
    let value = serde_json::json!({
        "parse": parse_report,
        "run_error": message,
        "host_calls": effect_summary.host_calls,
        "events_emitted": effect_summary.events_emitted,
        "partially_effectful": effect_summary.partially_effectful,
    });
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "cell".to_owned(),
        status: "error".to_owned(),
        message: Some(message.to_owned()),
        value: Some(value.clone()),
        quit: false,
    }
    .with_persistence(agent_repl_persist_cell(
        state,
        index,
        input,
        "error",
        value,
        effect_summary.partially_effectful,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AgentReplRunEffectSummary {
    pub(super) host_calls: usize,
    pub(super) events_emitted: u64,
    pub(super) partially_effectful: bool,
}

pub(super) fn agent_repl_run_effect_summary(events: &[DebugEvent]) -> AgentReplRunEffectSummary {
    let host_calls = events
        .iter()
        .filter(|event| agent_repl_effectful_debug_event(event.kind))
        .count();
    AgentReplRunEffectSummary {
        host_calls,
        events_emitted: events.last().map_or(0, |event| event.sequence),
        partially_effectful: host_calls > 0,
    }
}

pub(super) fn agent_repl_effectful_debug_event(kind: DebugEventKind) -> bool {
    matches!(
        kind,
        DebugEventKind::Observation
            | DebugEventKind::Action
            | DebugEventKind::Capture
            | DebugEventKind::Assertion
            | DebugEventKind::RagQuery
    )
}

pub(super) fn agent_repl_persist_cell(
    state: &mut AgentReplState,
    index: usize,
    input: &str,
    status: &str,
    display: serde_json::Value,
    partially_effectful: bool,
) -> Result<bool, String> {
    let Some(store) = &state.debug_store else {
        return Ok(false);
    };
    let ordinal = i64::try_from(index).map_err(|_| "REPL cell index is too large".to_owned())?;
    let source_hash = StableHash::new(format!(
        "blake3:{}",
        blake3::hash(input.as_bytes()).to_hex()
    ))
    .expect("generated REPL cell hash is nonempty");
    store
        .upsert_repl_cell(&DebugReplCell {
            cell_id: format!("repl:session.cli:{ordinal}"),
            session_id: agent_cli_session_id(),
            run_id: None,
            ordinal,
            source: input.to_owned(),
            source_hash,
            status: status.to_owned(),
            inferred_type: None,
            display: Some(display),
            partially_effectful,
            diagnostic_ids: Vec::new(),
            created_unix_ms: 0,
        })
        .map_err(|error| format!("failed to persist REPL cell {index}: {error}"))?;
    state.persisted_cells = state.persisted_cells.saturating_add(1);
    Ok(true)
}

pub(super) fn agent_repl_cell_source(
    index: usize,
    input: &str,
    fragment: &ParsedFragment,
    live_binding_prelude: &str,
) -> String {
    if matches!(fragment.kind(), Some(ParsedFragmentKind::Items(_))) {
        return input.to_owned();
    }
    let cell_body = if matches!(fragment.kind(), Some(ParsedFragmentKind::Expression(_))) {
        format!("    return {input}")
    } else if input.starts_with("return ") || input.contains("\nreturn ") {
        indent_agent_repl_body(input)
    } else {
        format!("{}\n    return \"ok\"", indent_agent_repl_body(input))
    };
    let body = if live_binding_prelude.trim().is_empty() {
        cell_body
    } else {
        format!(
            "{}\n{}",
            indent_agent_repl_body(live_binding_prelude),
            cell_body
        )
    };
    format!(
        "#[agent(version = 1)]\nagent @agent.repl.cell_{index} repl_cell_{index}()\neffects {{ agent.observe, agent.act.semantic, agent.act.physical, agent.wait, agent.capture, agent.resource.read, debug.read, debug.record, rag.query }}\n{{\n{body}\n}}\n"
    )
}

pub(super) fn agent_repl_live_binding_prelude(state: &AgentReplState) -> String {
    state
        .bindings
        .values()
        .filter(|binding| binding.status == "ok")
        .filter(|binding| binding.binding_kind == "local")
        .filter_map(|binding| {
            binding
                .serialized_source
                .as_ref()
                .map(|source| format!("let {} = {source}", binding.name))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn agent_repl_fragment_binding_names(fragment: &ParsedFragment) -> Vec<String> {
    let Some(ParsedFragmentKind::Statements(statements)) = fragment.kind() else {
        return Vec::new();
    };
    let mut names = statements
        .iter()
        .flat_map(agent_repl_stmt_binding_names)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

pub(super) fn agent_repl_stmt_binding_names(statement: &Stmt) -> Vec<String> {
    match statement {
        Stmt::Let { pattern, .. }
        | Stmt::LetElse { pattern, .. }
        | Stmt::LetChoice { pattern, .. }
        | Stmt::LetScope { pattern, .. }
        | Stmt::LetLoop { pattern, .. }
        | Stmt::LetAwait { pattern, .. } => agent_repl_pattern_binding_names(pattern),
        Stmt::WhileLet { pattern, .. } | Stmt::For { pattern, .. } => {
            agent_repl_pattern_binding_names(pattern)
        }
        Stmt::Return(_)
        | Stmt::Out { .. }
        | Stmt::Goto(_)
        | Stmt::Thread(_)
        | Stmt::DeferBlock { .. }
        | Stmt::Defer { .. }
        | Stmt::Yield(_)
        | Stmt::Signal { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Expr(_)
        | Stmt::Wait(_)
        | Stmt::On { .. }
        | Stmt::UnsafeLifetime { .. }
        | Stmt::If { .. }
        | Stmt::Match { .. }
        | Stmt::Loop { .. }
        | Stmt::While { .. }
        | Stmt::Close(_)
        | Stmt::Select(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => Vec::new(),
    }
}

pub(super) fn agent_repl_pattern_binding_names(pattern: &Pattern) -> Vec<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            vec![name.clone()]
        }
        Pattern::Whole { name, pattern } => {
            let mut names = vec![name.clone()];
            names.extend(agent_repl_pattern_binding_names(pattern));
            names
        }
        Pattern::Tuple(items) => items
            .iter()
            .flat_map(agent_repl_pattern_binding_names)
            .collect(),
        Pattern::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| agent_repl_pattern_binding_names(field.pattern()))
            .collect(),
        Pattern::BracketSeq { items, rest } => {
            let mut names = items
                .iter()
                .flat_map(agent_repl_pattern_binding_names)
                .collect::<Vec<_>>();
            names.extend(rest.clone());
            names
        }
        Pattern::Variant { payload, .. } => payload
            .as_ref()
            .map(agent_repl_variant_payload_binding_names)
            .unwrap_or_default(),
        Pattern::Literal(_) | Pattern::Entity(_) | Pattern::Discard | Pattern::Raw(_) => Vec::new(),
    }
}

pub(super) fn agent_repl_variant_payload_binding_names(
    payload: &VariantPatternPayload,
) -> Vec<String> {
    match payload {
        VariantPatternPayload::Tuple(items) => items
            .iter()
            .flat_map(agent_repl_pattern_binding_names)
            .collect(),
        VariantPatternPayload::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| agent_repl_pattern_binding_names(field.pattern()))
            .collect(),
    }
}

pub(super) fn indent_agent_repl_body(input: &str) -> String {
    input
        .lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn agent_repl_ok(
    index: usize,
    input: &str,
    kind: &str,
    value: serde_json::Value,
) -> AgentReplCellReport {
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: kind.to_owned(),
        status: "ok".to_owned(),
        message: None,
        value: Some(value),
        quit: false,
    }
}

pub(super) fn agent_repl_error(
    index: usize,
    input: &str,
    kind: &str,
    message: String,
) -> AgentReplCellReport {
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: kind.to_owned(),
        status: "error".to_owned(),
        message: Some(message),
        value: None,
        quit: false,
    }
}

pub(super) fn agent_repl_print_cell(report: &AgentReplCellReport) {
    match report.status.as_str() {
        "ok" | "parsed" => {
            if let Some(value) = &report.value {
                println!("{}: {}", report.status, value);
            } else {
                println!("{}", report.status);
            }
        }
        _ => {
            eprintln!(
                "error: {}",
                report.message.as_deref().unwrap_or("REPL cell failed")
            );
        }
    }
}
