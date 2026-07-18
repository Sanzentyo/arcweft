use super::repl::{AgentReplBinding, AgentReplCellReport, AgentReplState, agent_repl_ok};
use super::repl_cli_command::{
    AgentReplParsedInput, CliReplCommand, CliReplCommandContext, CliReplCommandFormattedOutput,
    CliReplCommandResult, CliReplLocalCommandFormatter, dispatch_cli_repl_command,
    parse_agent_repl_input,
};
use super::repl_command_format::{
    CliReplCommandFormatter, ReplCommandFormatMode, ReplCommandFormatOptions,
    ReplCommandFormattedOutput, ReplCommandResultFormatter,
};
use super::{
    AgentControllerRunConfig, AgentReplOptions, AgentRunnerConfig, CollectingDebugSink,
    NativeAdapterRegistrar, NoopRagService, PathBuf, agent_cli_session_id,
    agent_script_project_index,
};
#[cfg(feature = "native-player")]
use arcweft_agent_repl::command::RuntimeTaskReplCommandHost;
use arcweft_agent_repl::command::{
    AgentSessionReplCommandHost, LoadCommand, ReloadCommand, ReplCommand, ReplCommandContext,
    ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplCommandEvidence, ReplCommandHost,
    ReplCommandHostError, ReplCommandId, ReplCommandResult, ReplCommandStatus, ReplInput,
    ReplProjectLoader, ReplTracePolicy, repl_transaction_error_json,
};
use arcweft_agent_repl::{
    ReplBaseSnapshot, ReplCellExecutionStatus, ReplCellInput, ReplEvaluateOutcome,
    ReplEvaluationRuntime, ReplSession,
};

pub(super) fn agent_repl_eval_line(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    match parse_agent_repl_input(input) {
        Ok(AgentReplParsedInput::Shared(ReplInput::Empty)) => {
            agent_repl_ok(index, input, "empty", serde_json::json!({ "empty": true }))
        }
        Ok(AgentReplParsedInput::Shared(ReplInput::Cell(cell))) => {
            agent_repl_eval_typed_cell(index, input, options, state, &cell)
        }
        Ok(AgentReplParsedInput::Shared(ReplInput::Command(command))) => {
            agent_repl_eval_typed_meta(index, input, options, state, command)
        }
        Ok(AgentReplParsedInput::Cli(command)) => {
            agent_repl_eval_cli_meta(index, input, options, state, adapter_registrars, command)
        }
        Err(error) => agent_repl_typed_cell_report(
            index,
            input,
            options,
            &ReplCommandResult::error(
                agent_repl_command_id(index),
                ReplCommandEvidence::Empty,
                error.into_diagnostic(),
            ),
        ),
    }
}

fn agent_repl_eval_typed_cell(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    cell: &ReplCellInput,
) -> AgentReplCellReport {
    let trace_policy = agent_repl_trace_policy(state);
    let Some(command_agent_session) = state.command_agent_session.as_mut() else {
        return agent_repl_command_surface_unavailable(index, input, options);
    };

    let mut debug = CollectingDebugSink::default();
    let mut rag = NoopRagService;
    let runtime = ReplEvaluationRuntime::new(
        command_agent_session,
        &mut debug,
        &mut rag,
        AgentRunnerConfig::new(agent_cli_session_id()),
    )
    .with_run_config(AgentControllerRunConfig {
        max_steps: 16,
        max_ops_per_step: 128,
    });

    let evaluation = {
        let Some(command_session) = state.command_session.as_mut() else {
            return agent_repl_command_surface_unavailable(index, input, options);
        };
        let mut context = ReplCommandContext::new(command_session)
            .with_next_command_id(agent_repl_command_id(index))
            .with_trace_policy(trace_policy);
        if let Some(result) = context.reject_cell_submission_if_read_only(cell) {
            return agent_repl_typed_cell_report(index, input, options, &result);
        }
        context.session_mut().evaluate_cell(cell, runtime)
    };
    match evaluation {
        Ok(outcome) => agent_repl_typed_cell_outcome(index, input, state, outcome),
        Err(error) => agent_repl_transaction_error_report(index, input, &error),
    }
}

fn agent_repl_transaction_error_report(
    index: usize,
    input: &str,
    error: &arcweft_agent_repl::ReplTransactionError,
) -> AgentReplCellReport {
    let message = error.parse_diagnostics().map_or_else(
        || error.to_string(),
        |diagnostics| {
            diagnostics
                .iter()
                .map(|diagnostic| {
                    format!(
                        "[{}] {} at {}..{}",
                        diagnostic.code(),
                        diagnostic.message(),
                        diagnostic.range().start(),
                        diagnostic.range().end(),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        },
    );
    let value = serde_json::json!({
        "transaction_error": repl_transaction_error_json(error),
    });
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "cell".to_owned(),
        status: "error".to_owned(),
        message: Some(message),
        value: Some(value),
        quit: false,
    }
}

fn agent_repl_eval_typed_meta(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    command: ReplCommand,
) -> AgentReplCellReport {
    let trace_policy = agent_repl_trace_policy(state);
    let mut loader = CliReplProjectLoader {
        project_path: &mut state.command_project_path,
    };

    let result = if let Some(remote_session) = state.remote_session.as_mut() {
        let mut host = AgentSessionReplCommandHost::new(remote_session);
        #[cfg(feature = "native-player")]
        if let Some(runtime_session) = state.runtime_session.as_mut() {
            let mut task_host = RuntimeTaskReplCommandHost::new(&mut host, runtime_session);
            return agent_repl_typed_cell_report(
                index,
                input,
                options,
                &agent_repl_handle_typed_command(
                    ReplCommandDispatch {
                        index,
                        command_session: &mut state.command_session,
                        tier_handler: &mut state.command_tier_handler,
                        loader: &mut loader,
                        trace_policy,
                        command,
                    },
                    Some(&mut task_host),
                ),
            );
        }
        agent_repl_handle_typed_command(
            ReplCommandDispatch {
                index,
                command_session: &mut state.command_session,
                tier_handler: &mut state.command_tier_handler,
                loader: &mut loader,
                trace_policy,
                command,
            },
            Some(&mut host),
        )
    } else {
        let Some(command_agent_session) = state.command_agent_session.as_mut() else {
            return agent_repl_command_surface_unavailable(index, input, options);
        };
        let mut host = AgentSessionReplCommandHost::new(command_agent_session);
        agent_repl_handle_typed_command_with_runtime_tasks(
            ReplCommandDispatch {
                index,
                command_session: &mut state.command_session,
                tier_handler: &mut state.command_tier_handler,
                loader: &mut loader,
                trace_policy,
                command,
            },
            &mut host,
            #[cfg(feature = "native-player")]
            state.runtime_session.as_mut(),
        )
    };

    agent_repl_typed_cell_report(index, input, options, &result)
}

fn agent_repl_eval_cli_meta(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
    command: CliReplCommand,
) -> AgentReplCellReport {
    let trace_policy = agent_repl_trace_policy(state);
    let result = dispatch_cli_repl_command(
        CliReplCommandContext {
            command_id: agent_repl_command_id(index),
            index,
            input,
            options,
            state,
            adapter_registrars,
            trace_policy,
        },
        command,
    );
    agent_repl_cli_typed_cell_report(index, input, options, &result)
}

fn agent_repl_cli_typed_cell_report(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    result: &CliReplCommandResult,
) -> AgentReplCellReport {
    let output = CliReplLocalCommandFormatter::format_result(result);
    let status = match result.status {
        ReplCommandStatus::Ok | ReplCommandStatus::ExitRequested => "ok",
        ReplCommandStatus::Queued => "queued",
        ReplCommandStatus::Rejected | ReplCommandStatus::Error => "error",
    };
    let text = output.text.clone();
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "meta".to_owned(),
        status: status.to_owned(),
        message: matches!(
            result.status,
            ReplCommandStatus::Rejected | ReplCommandStatus::Error
        )
        .then_some(text.clone()),
        value: Some(if options.json {
            output.json
        } else {
            agent_repl_cli_formatted_output_value(output)
        }),
        quit: false,
    }
}

fn agent_repl_cli_formatted_output_value(
    output: CliReplCommandFormattedOutput,
) -> serde_json::Value {
    match output.json {
        serde_json::Value::Object(mut value) => {
            value.insert(
                "formatted_text".to_owned(),
                serde_json::Value::String(output.text),
            );
            serde_json::Value::Object(value)
        }
        other => serde_json::json!({ "formatted_text": output.text, "value": other }),
    }
}

struct ReplCommandDispatch<'a> {
    index: usize,
    command_session: &'a mut Option<ReplSession>,
    tier_handler: &'a mut arcweft_agent_repl::ReplTierCommandHandler,
    loader: &'a mut dyn ReplProjectLoader,
    trace_policy: ReplTracePolicy,
    command: ReplCommand,
}

fn agent_repl_handle_typed_command_with_runtime_tasks(
    dispatch: ReplCommandDispatch<'_>,
    host: &mut dyn ReplCommandHost,
    #[cfg(feature = "native-player")] runtime_session: Option<
        &mut arcweft_runtime_driver::session::BundleSession,
    >,
) -> ReplCommandResult {
    #[cfg(feature = "native-player")]
    if let Some(runtime_session) = runtime_session {
        let mut task_host = RuntimeTaskReplCommandHost::new(host, runtime_session);
        return agent_repl_handle_typed_command(dispatch, Some(&mut task_host));
    }

    agent_repl_handle_typed_command(dispatch, Some(host))
}

fn agent_repl_handle_typed_command(
    dispatch: ReplCommandDispatch<'_>,
    host: Option<&mut dyn ReplCommandHost>,
) -> ReplCommandResult {
    let Some(command_session) = dispatch.command_session.as_mut() else {
        return ReplCommandResult::error(
            agent_repl_command_id(dispatch.index),
            ReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::HostUnavailable,
                "typed Agent REPL command session is not initialized",
            ),
        );
    };
    let mut context = ReplCommandContext::new(command_session)
        .with_next_command_id(agent_repl_command_id(dispatch.index))
        .with_loader(dispatch.loader)
        .with_trace_policy(dispatch.trace_policy);
    if let Some(host) = host {
        context = context.with_host(host);
    }
    arcweft_agent_repl::command::ReplCommandHandler::handle(
        dispatch.tier_handler,
        &mut context,
        dispatch.command,
    )
}

fn agent_repl_typed_cell_report(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    result: &ReplCommandResult,
) -> AgentReplCellReport {
    let format_options = ReplCommandFormatOptions {
        mode: if options.json {
            ReplCommandFormatMode::Json
        } else {
            ReplCommandFormatMode::Human
        },
        ..ReplCommandFormatOptions::default()
    };
    let output = CliReplCommandFormatter.format_result(result, &format_options);
    let status = match result.status {
        ReplCommandStatus::Ok | ReplCommandStatus::ExitRequested => "ok",
        ReplCommandStatus::Queued => "queued",
        ReplCommandStatus::Rejected | ReplCommandStatus::Error => "error",
    };
    let text = output.text.clone();
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "meta".to_owned(),
        status: status.to_owned(),
        message: matches!(
            result.status,
            ReplCommandStatus::Rejected | ReplCommandStatus::Error
        )
        .then_some(text.clone()),
        value: Some(if options.json {
            output.json
        } else {
            agent_repl_formatted_output_value(output)
        }),
        quit: matches!(result.status, ReplCommandStatus::ExitRequested),
    }
}

fn agent_repl_typed_cell_outcome(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    outcome: ReplEvaluateOutcome,
) -> AgentReplCellReport {
    let record = outcome.record;
    let execution = &record.execution;
    let binding_name = record.id.label();
    let status = if execution.status == ReplCellExecutionStatus::Executed {
        "ok"
    } else {
        "error"
    };
    state.bindings.insert(
        binding_name.clone(),
        AgentReplBinding {
            name: binding_name.clone(),
            binding_kind: "cell".to_owned(),
            source: record.source.clone(),
            status: status.to_owned(),
            final_status: execution.final_status.clone(),
            host_calls: execution.host_calls,
            responses: execution.responses,
            serializable: false,
            serialized_source: None,
            snapshot_kind: None,
            non_serializable_reason: Some(
                "cell artifact is represented by typed REPL evidence".to_owned(),
            ),
        },
    );
    for binding in &record.bindings {
        state.bindings.insert(
            binding.name.clone(),
            AgentReplBinding {
                name: binding.name.clone(),
                binding_kind: "local".to_owned(),
                source: binding.source.clone(),
                status: status.to_owned(),
                final_status: execution.final_status.clone(),
                host_calls: execution.host_calls,
                responses: execution.responses,
                serializable: true,
                serialized_source: Some(binding.source.clone()),
                snapshot_kind: Some(binding.snapshot_kind.as_str().to_owned()),
                non_serializable_reason: None,
            },
        );
    }

    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "cell".to_owned(),
        status: status.to_owned(),
        message: execution.error.clone(),
        value: Some(serde_json::json!({
            "binding": binding_name,
            "compiled": true,
            "committed": outcome.committed,
            "cell": {
                "id": record.id.as_u64(),
                "kind": record.kind.as_str(),
                "generation": record.generation.as_u64(),
                "overlay_hash": record.overlay_hash,
                "commit_hash": record.commit_hash,
                "entry": record.entry,
                "bytecode_stats": {
                    "flows": record.bytecode_stats.flows,
                    "instructions": record.bytecode_stats.instructions,
                    "line_task_groups": record.bytecode_stats.line_task_groups,
                    "stream_plans": record.bytecode_stats.stream_plans,
                    "source_plans": record.bytecode_stats.source_plans,
                },
                "verified_effects": record.verified_effects,
                "bindings": record.bindings.iter().map(|binding| serde_json::json!({
                    "name": binding.name,
                    "snapshot_kind": binding.snapshot_kind.as_str(),
                    "project_bound": binding.project_bound,
                })).collect::<Vec<_>>(),
            },
            "execution": {
                "status": format!("{:?}", execution.status),
                "steps": execution.steps,
                "host_calls": execution.host_calls,
                "responses": execution.responses,
                "events_emitted": execution.events_emitted,
                "final_status": execution.final_status,
                "error": execution.error,
                "partially_effectful": execution.host_effects.partially_effectful,
            }
        })),
        quit: false,
    }
}

fn agent_repl_formatted_output_value(output: ReplCommandFormattedOutput) -> serde_json::Value {
    match output.json {
        serde_json::Value::Object(mut value) => {
            value.insert(
                "formatted_text".to_owned(),
                serde_json::Value::String(output.text),
            );
            serde_json::Value::Object(value)
        }
        other => serde_json::json!({ "formatted_text": output.text, "value": other }),
    }
}

fn agent_repl_command_surface_unavailable(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
) -> AgentReplCellReport {
    agent_repl_typed_cell_report(
        index,
        input,
        options,
        &ReplCommandResult::error(
            agent_repl_command_id(index),
            ReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::HostUnavailable,
                "typed Agent REPL command surface is not initialized",
            ),
        ),
    )
}

fn agent_repl_trace_policy(state: &AgentReplState) -> ReplTracePolicy {
    if state.read_only {
        ReplTracePolicy::ReadOnlyTrace
    } else {
        ReplTracePolicy::ReadWrite
    }
}

fn agent_repl_command_id(index: usize) -> ReplCommandId {
    ReplCommandId::new(u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1))
}

struct CliReplProjectLoader<'a> {
    project_path: &'a mut Option<String>,
}

impl ReplProjectLoader for CliReplProjectLoader<'_> {
    fn load(&mut self, command: &LoadCommand) -> Result<ReplBaseSnapshot, ReplCommandHostError> {
        let snapshot = cli_repl_base_snapshot(Some(command.path.as_str()))?;
        *self.project_path = Some(command.path.clone());
        Ok(snapshot)
    }

    fn reload(
        &mut self,
        command: &ReloadCommand,
    ) -> Result<ReplBaseSnapshot, ReplCommandHostError> {
        let selected = command.path.as_deref().or(self.project_path.as_deref());
        let snapshot = cli_repl_base_snapshot(selected)?;
        if let Some(path) = &command.path {
            *self.project_path = Some(path.clone());
        }
        Ok(snapshot)
    }
}

fn cli_repl_base_snapshot(path: Option<&str>) -> Result<ReplBaseSnapshot, ReplCommandHostError> {
    if let Some(path) = path {
        let candidate = PathBuf::from(path);
        if !candidate.exists() {
            return Err(ReplCommandHostError::new(
                ReplCommandDiagnosticCode::ProjectLoaderError,
                format!(
                    "CLI REPL project path {} does not exist",
                    candidate.display()
                ),
            ));
        }
    }
    let project = agent_script_project_index(&[]).map_err(|message| {
        ReplCommandHostError::new(ReplCommandDiagnosticCode::ProjectLoaderError, message)
    })?;
    let label = path.map_or_else(
        || "cli-agent-repl:current".to_owned(),
        |path| format!("cli-agent-repl:{path}"),
    );
    Ok(ReplBaseSnapshot::from_project(label, project))
}

#[cfg(test)]
mod parse_diagnostic_tests {
    use arcweft_agent_repl::ReplTransactionError;
    use arcweft_lang_syntax::parser::parse_source;

    use super::agent_repl_transaction_error_report;

    #[test]
    fn agent_repl_parse_failure_json_preserves_typed_diagnostics() {
        let parsed =
            parse_source("pub view Card() {\n    export part as heading\n    Panel()\n}\n");
        let error = ReplTransactionError::Parse {
            diagnostics: parsed.errors().to_vec(),
        };

        let report = agent_repl_transaction_error_report(3, "invalid cell", &error);
        let message = report.message.as_deref().expect("human parse diagnostics");
        assert!(message.contains(
            "[view::export_part_missing_local] View part export needs a private local target before `as` at 34..36"
        ));
        assert!(!message.contains("Missing local View part name"));
        let value = report.value.expect("typed parse error JSON");
        let diagnostic = &value["transaction_error"]["diagnostics"][0];
        assert_eq!(diagnostic["code"], "view::export_part_missing_local");
        assert_eq!(diagnostic["label"], "Missing local View part name");
        assert_eq!(diagnostic["coordinate_space"], "synthetic_source");
        assert!(diagnostic["range"]["end"].as_u64().is_some());
        assert_eq!(diagnostic["expected"][0], "local part name");
        assert_eq!(diagnostic["found"], serde_json::Value::Null);
        assert!(
            diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("private local target"))
        );
        assert_eq!(diagnostic["recovery"][0]["applicability"], "unspecified");
    }
}

#[cfg(all(test, feature = "native-capture"))]
mod tests {
    use super::super::{
        AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT, AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH,
        CliRuntimeExecutorTier, CliRuntimeStepMode, ProfileOptions,
    };
    use super::{AgentReplOptions, agent_repl_eval_line};
    use crate::app::agent::native::repl::AgentReplState;

    fn test_options() -> AgentReplOptions {
        AgentReplOptions {
            path: None,
            profile: ProfileOptions::default(),
            entry: None,
            executor: CliRuntimeExecutorTier::BytecodeVm,
            pure_backend: None,
            pure_workers: None,
            pure_batch_min_len: None,
            pure_object_artifacts: false,
            math_backend: None,
            math_wgpu_min_elements: None,
            steps: 1,
            capture_step: None,
            mode: CliRuntimeStepMode::Drain,
            max_ops: 64,
            values: Vec::new(),
            viewport_width: AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH,
            viewport_height: AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT,
            capture_time_seconds: None,
            debug_db: None,
            trace: None,
            read_only: false,
            connect: None,
            input: None,
            json: true,
        }
    }

    #[test]
    fn repl_command_bridge_routes_cli_command_to_typed_json() {
        let options = test_options();
        let mut state = AgentReplState::default();
        let report = agent_repl_eval_line(0, ":trace", &options, &mut state, &[]);

        assert_eq!(report.status, "ok");
        let value = report.value.expect("typed CLI result JSON");
        assert_eq!(value["command"], ":trace");
        assert_eq!(value["evidence"]["kind"], "trace");
        assert!(value.get("formatted_text").is_none());
    }

    #[test]
    fn repl_command_bridge_reports_two_stage_unknown_command() {
        let options = test_options();
        let mut state = AgentReplState::default();
        let report =
            agent_repl_eval_line(0, ":definitely-not-a-command", &options, &mut state, &[]);

        assert_eq!(report.status, "error");
        let value = report.value.expect("typed parse error JSON");
        assert_eq!(value["diagnostics"][0]["code"], "unknown_command");
        assert!(
            value["diagnostics"][0]["message"]
                .as_str()
                .is_some_and(|message| message.contains("shared REPL and CLI inspection/debug"))
        );
    }
}
