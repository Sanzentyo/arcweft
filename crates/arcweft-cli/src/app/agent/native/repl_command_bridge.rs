use super::repl::{
    AgentReplBinding, AgentReplCellReport, AgentReplState, agent_repl_eval_meta, agent_repl_ok,
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
    ReplProjectLoader, ReplTracePolicy, parse_repl_input,
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
    match parse_repl_input(input) {
        Ok(ReplInput::Empty) => {
            agent_repl_ok(index, input, "empty", serde_json::json!({ "empty": true }))
        }
        Ok(ReplInput::Cell(cell)) => {
            agent_repl_eval_typed_cell(index, input, options, state, &cell)
        }
        Ok(ReplInput::Command(command)) => {
            agent_repl_eval_typed_meta(index, input, options, state, command)
        }
        Err(_) if agent_repl_cli_meta_command(input).is_some() => {
            agent_repl_eval_meta(index, input, options, state, adapter_registrars)
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
        Err(error) => AgentReplCellReport {
            index,
            input: input.to_owned(),
            kind: "cell".to_owned(),
            status: "error".to_owned(),
            message: Some(error.to_string()),
            value: Some(serde_json::json!({ "transaction_error": error.to_string() })),
            quit: false,
        },
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
                "entry_flow": record.entry_flow,
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

fn agent_repl_cli_meta_command(input: &str) -> Option<&str> {
    input.split_whitespace().next().filter(|command| {
        matches!(
            *command,
            ":trace"
                | ":actions"
                | ":type"
                | ":ast"
                | ":hir"
                | ":bytecode"
                | ":capture"
                | ":query"
                | ":drop"
                | ":save"
                | ":connect"
        )
    })
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
