
use super::*;

pub(super) fn agent_mcp_call_debug_search(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let query = agent_mcp_non_empty_string_argument(arguments, "query");
    let query_vector = agent_mcp_query_vector_argument(arguments, "query_vector")?;
    let graph_query = agent_mcp_non_empty_string_argument(arguments, "graph_query");
    let history_query = agent_mcp_non_empty_string_argument(arguments, "history_query");
    let diagnostic_query = agent_mcp_non_empty_string_argument(arguments, "diagnostic_query");
    let test_query = agent_mcp_non_empty_string_argument(arguments, "test_query");
    let selector_count = usize::from(query.is_some())
        + usize::from(query_vector.is_some())
        + usize::from(graph_query.is_some())
        + usize::from(history_query.is_some())
        + usize::from(diagnostic_query.is_some())
        + usize::from(test_query.is_some());
    if selector_count == 0 {
        return Err(
            "arcweft.debug.search requires one of query, query_vector, graph_query, history_query, diagnostic_query, or test_query"
                .to_owned(),
        );
    }
    if selector_count > 1 {
        return Err(
            "arcweft.debug.search accepts only one of query, query_vector, graph_query, history_query, diagnostic_query, or test_query"
                .to_owned(),
        );
    }
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(10);
    if limit == 0 {
        return Err("arcweft.debug.search argument limit must be at least 1".to_owned());
    }
    let graph_depth =
        agent_mcp_u32_argument(arguments, "graph_depth", "arcweft.debug.search")?.unwrap_or(1);
    let max_privacy = agent_mcp_privacy_class_argument(arguments, "max_privacy")?
        .unwrap_or(PrivacyClass::Project);
    let path = agent_mcp_debug_store_path(arguments);
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.debug.search failed to open `{path}`: {error}"))?;
    let request = AgentMcpDebugSearchRequest {
        query,
        query_vector: query_vector.as_deref(),
        graph_query,
        history_query,
        diagnostic_query,
        test_query,
        graph_depth,
        limit,
        max_privacy,
    };
    let hits = agent_mcp_debug_search_hits(&store, &request, arguments)
        .map_err(|error| format!("arcweft.debug.search failed to search `{path}`: {error}"))?
        .iter()
        .map(agent_mcp_debug_search_hit_json)
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "path": path,
        "query": query,
        "query_vector_dimensions": query_vector.as_ref().map(Vec::len),
        "graph_query": graph_query,
        "graph_depth": graph_query.map(|_| graph_depth),
        "history_query": history_query,
        "diagnostic_query": diagnostic_query,
        "test_query": test_query,
        "limit": limit,
        "max_privacy": max_privacy.as_str(),
        "hits": hits,
    });
    agent_mcp_json_tool_result(&value, "debug search")
}

pub(super) struct AgentMcpDebugSearchRequest<'a> {
    pub(super) query: Option<&'a str>,
    pub(super) query_vector: Option<&'a [f32]>,
    pub(super) graph_query: Option<&'a str>,
    pub(super) history_query: Option<&'a str>,
    pub(super) diagnostic_query: Option<&'a str>,
    pub(super) test_query: Option<&'a str>,
    pub(super) graph_depth: u32,
    pub(super) limit: usize,
    pub(super) max_privacy: PrivacyClass,
}

pub(super) fn agent_mcp_debug_search_hits(
    store: &DebugStore,
    request: &AgentMcpDebugSearchRequest<'_>,
    arguments: &serde_json::Value,
) -> Result<Vec<ChunkSearchResult>, String> {
    if let Some(query) = request.query {
        return store
            .lexical_search_with_max_privacy(query, request.limit, request.max_privacy)
            .map_err(|error| error.to_string());
    }
    if let Some(query) = request.graph_query {
        return store
            .graph_search_with_depth_and_max_privacy(
                query,
                request.graph_depth,
                request.limit,
                request.max_privacy,
            )
            .map_err(|error| error.to_string());
    }
    if let Some(query) = request.history_query {
        return store
            .history_search_with_max_privacy(query, request.limit, request.max_privacy)
            .map_err(|error| error.to_string());
    }
    if let Some(query) = request.diagnostic_query {
        return store
            .diagnostic_search_with_max_privacy(query, request.limit, request.max_privacy)
            .map_err(|error| error.to_string());
    }
    if let Some(query) = request.test_query {
        return store
            .test_result_search_with_max_privacy(query, request.limit, request.max_privacy)
            .map_err(|error| error.to_string());
    }
    let vector = request
        .query_vector
        .expect("debug search selector validation requires a vector");
    let model = agent_mcp_debug_search_model(arguments, vector.len())?;
    store
        .vector_search_with_max_privacy(&model, vector, request.limit, request.max_privacy)
        .map_err(|error| error.to_string())
}

pub(super) fn agent_mcp_debug_search_hit_json(result: &ChunkSearchResult) -> serde_json::Value {
    serde_json::json!({
        "chunk_id": result.hit.chunk_id.as_str(),
        "title": result.title,
        "body": result.body,
        "source_kind": result.source_kind,
        "source_key": result.source_key,
        "privacy": result.privacy.as_str(),
        "channel": agent_mcp_search_channel_label(result.hit.channel),
        "rank": result.hit.rank,
        "score": result.hit.score,
    })
}

pub(super) fn agent_mcp_call_debug_script_runs(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(20);
    if limit == 0 {
        return Err("arcweft.debug.script.runs argument limit must be at least 1".to_owned());
    }
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.debug.script.runs")?;
    let session_id = agent_mcp_non_empty_string_argument(arguments, "session_id")
        .map(SessionId::new)
        .transpose()
        .map_err(|error| format!("arcweft.debug.script.runs invalid session_id: {error}"))?;
    let path = agent_mcp_debug_store_path(arguments);
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.debug.script.runs failed to open `{path}`: {error}"))?;
    let runs = store
        .script_runs(session_id.as_ref(), limit)
        .map_err(|error| format!("arcweft.debug.script.runs failed to read `{path}`: {error}"))?;
    let value = serde_json::json!({
        "path": path,
        "session_id": session_id.as_ref().map(SessionId::as_str),
        "limit": limit,
        "max_privacy": max_privacy.as_str(),
        "runs": runs.iter().map(|run| agent_mcp_debug_script_run_json(run, max_privacy)).collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug script runs")
}

pub(super) fn agent_mcp_debug_script_run_json(
    run: &DebugScriptRun,
    max_privacy: PrivacyClass,
) -> serde_json::Value {
    let include_project_metadata = PrivacyClass::Project.is_allowed_by(max_privacy);
    serde_json::json!({
        "run_id": run.run_id.as_str(),
        "session_id": run.session_id.as_str(),
        "agent_id": run.agent_id.as_ref().map(PublicId::as_str),
        "artifact_hash": run.artifact_hash.as_ref().map(StableHash::as_str),
        "source_hash": run.source_hash.as_ref().map(StableHash::as_str),
        "project_binding_mode": &run.project_binding_mode,
        "started_sequence": run.started_sequence,
        "finished_sequence": run.finished_sequence,
        "outcome": run.outcome.as_str(),
        "partially_effectful": run.partially_effectful,
        "trace_uri": &run.trace_uri,
        "error": &run.error,
        "project": if include_project_metadata { debug_project_readback_json(&run.metadata) } else { None },
        "metadata": if include_project_metadata { serde_json::json!(&run.metadata) } else { serde_json::json!({}) },
    })
}

pub(super) fn agent_mcp_call_debug_close_stale_sessions(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let stale_after_millis = agent_mcp_u64_argument(
        arguments,
        "stale_after_millis",
        "arcweft.debug.sessions.close_stale",
    )?
    .ok_or_else(|| {
        "arcweft.debug.sessions.close_stale requires arguments.stale_after_millis".to_owned()
    })?;
    if stale_after_millis == 0 {
        return Err(
            "arcweft.debug.sessions.close_stale argument stale_after_millis must be at least 1"
                .to_owned(),
        );
    }
    let stale_after_millis = i64::try_from(stale_after_millis).map_err(|_| {
        "arcweft.debug.sessions.close_stale argument stale_after_millis is too large".to_owned()
    })?;
    let reason =
        agent_mcp_non_empty_string_argument(arguments, "reason").unwrap_or("stale_running_session");
    let dry_run = arguments
        .get("dry_run")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let now_unix_ms = agent_mcp_current_unix_millis();
    let cutoff_unix_ms = now_unix_ms.saturating_sub(stale_after_millis);
    let path = agent_mcp_debug_store_path(arguments);
    let store = DebugStore::open(path).map_err(|error| {
        format!("arcweft.debug.sessions.close_stale failed to open `{path}`: {error}")
    })?;
    let matched_sessions = store
        .stale_running_sessions(cutoff_unix_ms)
        .map_err(|error| {
            format!("arcweft.debug.sessions.close_stale failed to read `{path}`: {error}")
        })?;
    let closed_sessions = if dry_run {
        Vec::new()
    } else {
        store
            .abandon_stale_running_sessions(cutoff_unix_ms, now_unix_ms, reason)
            .map_err(|error| {
                format!("arcweft.debug.sessions.close_stale failed to update `{path}`: {error}")
            })?
    };
    let value = serde_json::json!({
        "path": path,
        "stale_after_millis": stale_after_millis,
        "cutoff_unix_ms": cutoff_unix_ms,
        "closed_unix_ms": now_unix_ms,
        "reason": reason,
        "dry_run": dry_run,
        "matched_sessions": matched_sessions
            .iter()
            .map(agent_mcp_debug_session_json)
            .collect::<Vec<_>>(),
        "closed_sessions": closed_sessions
            .iter()
            .map(agent_mcp_debug_session_json)
            .collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug close stale sessions")
}

pub(super) fn agent_mcp_debug_session_json(session: &DebugSession) -> serde_json::Value {
    serde_json::json!({
        "session_id": session.session_id.as_str(),
        "program_hash": session.program_hash.as_ref().map(StableHash::as_str),
        "profile": &session.profile,
        "transport": &session.transport,
        "started_unix_ms": session.started_unix_ms,
        "ended_unix_ms": session.ended_unix_ms,
        "status": session.status.as_str(),
        "metadata": &session.metadata,
    })
}

pub(super) fn agent_mcp_current_unix_millis() -> i64 {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

pub(super) fn agent_mcp_call_debug_session_timeline(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(50);
    if limit == 0 {
        return Err("arcweft.debug.session.timeline argument limit must be at least 1".to_owned());
    }
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.debug.session.timeline")?;
    let session_id = agent_mcp_non_empty_string_argument(arguments, "session_id");
    let run_id = agent_mcp_non_empty_string_argument(arguments, "run_id");
    let path = agent_mcp_debug_store_path(arguments);
    let store = DebugStore::open(path).map_err(|error| {
        format!("arcweft.debug.session.timeline failed to open `{path}`: {error}")
    })?;
    let events = store
        .session_timeline_with_max_privacy(session_id, run_id, limit, max_privacy)
        .map_err(|error| {
            format!("arcweft.debug.session.timeline failed to read `{path}`: {error}")
        })?;
    let value = serde_json::json!({
        "path": path,
        "session_id": session_id,
        "run_id": run_id,
        "limit": limit,
        "max_privacy": max_privacy.as_str(),
        "events": events.iter().map(agent_mcp_debug_timeline_event_json).collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug session timeline")
}

pub(super) fn agent_mcp_debug_timeline_event_json(event: &DebugTimelineEvent) -> serde_json::Value {
    serde_json::json!({
        "session_id": &event.session_id,
        "run_id": &event.run_id,
        "sequence": event.sequence,
        "tick": event.tick,
        "kind": &event.event_kind,
        "privacy": event.privacy.as_str(),
        "payload": &event.payload,
        "created_unix_ms": event.created_unix_ms,
    })
}

pub(super) fn agent_mcp_call_debug_repl_cells(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(50);
    if limit == 0 {
        return Err("arcweft.debug.repl.cells argument limit must be at least 1".to_owned());
    }
    let session_id = agent_mcp_non_empty_string_argument(arguments, "session_id")
        .ok_or_else(|| "arcweft.debug.repl.cells requires arguments.session_id".to_owned())
        .and_then(|value| {
            SessionId::new(value)
                .map_err(|error| format!("arcweft.debug.repl.cells invalid session_id: {error}"))
        })?;
    let path = agent_mcp_debug_store_path(arguments);
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.debug.repl.cells failed to open `{path}`: {error}"))?;
    let cells = store
        .repl_cells_for_session(&session_id)
        .map_err(|error| format!("arcweft.debug.repl.cells failed to read `{path}`: {error}"))?;
    let value = serde_json::json!({
        "path": path,
        "session_id": session_id.as_str(),
        "limit": limit,
        "cells": cells
            .iter()
            .take(limit)
            .map(agent_mcp_debug_repl_cell_json)
            .collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug REPL cells")
}

pub(super) fn agent_mcp_debug_repl_cell_json(cell: &DebugReplCell) -> serde_json::Value {
    serde_json::json!({
        "cell_id": &cell.cell_id,
        "session_id": cell.session_id.as_str(),
        "run_id": cell.run_id.as_ref().map(AgentRunId::as_str),
        "ordinal": cell.ordinal,
        "source": &cell.source,
        "source_hash": cell.source_hash.as_str(),
        "status": &cell.status,
        "inferred_type": &cell.inferred_type,
        "display": &cell.display,
        "partially_effectful": cell.partially_effectful,
        "diagnostic_ids": &cell.diagnostic_ids,
        "created_unix_ms": cell.created_unix_ms,
    })
}

pub(super) fn agent_mcp_call_debug_source_files(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let program_hash = agent_mcp_non_empty_string_argument(arguments, "program_hash")
        .ok_or_else(|| "arcweft.debug.source.files requires arguments.program_hash".to_owned())
        .and_then(|value| {
            StableHash::new(value).map_err(|error| {
                format!("arcweft.debug.source.files invalid program_hash: {error}")
            })
        })?;
    let path = agent_mcp_debug_store_path(arguments);
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.debug.source.files")?;
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.debug.source.files failed to open `{path}`: {error}"))?;
    let sources = if PrivacyClass::Project.is_allowed_by(max_privacy) {
        store
            .source_files_for_program(&program_hash)
            .map_err(|error| {
                format!("arcweft.debug.source.files failed to read `{path}`: {error}")
            })?
    } else {
        Vec::new()
    };
    let value = serde_json::json!({
        "path": path,
        "program_hash": program_hash.as_str(),
        "max_privacy": max_privacy.as_str(),
        "sources": sources
            .iter()
            .map(agent_mcp_debug_source_file_json)
            .collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug source files")
}

pub(super) fn agent_mcp_debug_source_file_json(source: &DebugSourceFile) -> serde_json::Value {
    serde_json::json!({
        "program_hash": source.program_hash.as_str(),
        "path": &source.path,
        "language": &source.language,
        "content_hash": source.content_hash.as_str(),
        "byte_len": source.byte_len,
        "metadata": &source.metadata,
    })
}

pub(super) fn agent_mcp_call_debug_graph_inventory(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let program_hash = agent_mcp_non_empty_string_argument(arguments, "program_hash")
        .ok_or_else(|| "arcweft.debug.graph.inventory requires arguments.program_hash".to_owned())
        .and_then(|value| {
            StableHash::new(value).map_err(|error| {
                format!("arcweft.debug.graph.inventory invalid program_hash: {error}")
            })
        })?;
    let path = agent_mcp_debug_store_path(arguments);
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.debug.graph.inventory")?;
    let store = DebugStore::open(path).map_err(|error| {
        format!("arcweft.debug.graph.inventory failed to open `{path}`: {error}")
    })?;
    let (symbols, edges) =
        if PrivacyClass::Project.is_allowed_by(max_privacy) {
            let symbols = store.graph_symbols_for_program(&program_hash).map_err(|error| {
            format!("arcweft.debug.graph.inventory failed to read symbols from `{path}`: {error}")
        })?;
            let edges = store
                .graph_edges_for_program(&program_hash)
                .map_err(|error| {
                    format!(
                        "arcweft.debug.graph.inventory failed to read edges from `{path}`: {error}"
                    )
                })?;
            (symbols, edges)
        } else {
            (Vec::new(), Vec::new())
        };
    let value = serde_json::json!({
        "path": path,
        "program_hash": program_hash.as_str(),
        "max_privacy": max_privacy.as_str(),
        "symbol_count": symbols.len(),
        "edge_count": edges.len(),
        "symbols": symbols
            .iter()
            .map(agent_mcp_debug_graph_symbol_json)
            .collect::<Vec<_>>(),
        "edges": edges
            .iter()
            .map(agent_mcp_debug_graph_edge_json)
            .collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug graph inventory")
}

pub(super) fn agent_mcp_debug_graph_symbol_json(symbol: &DebugGraphSymbol) -> serde_json::Value {
    serde_json::json!({
        "program_hash": symbol.program_hash.as_str(),
        "symbol_id": &symbol.symbol_id,
        "public_id": symbol.public_id.as_ref().map(PublicId::as_str),
        "qualified_name": &symbol.qualified_name,
        "kind": &symbol.kind,
        "type_json": &symbol.type_json,
        "source_path": &symbol.source_path,
        "source_content_hash": symbol.source_content_hash.as_ref().map(StableHash::as_str),
        "start_byte": symbol.start_byte,
        "end_byte": symbol.end_byte,
        "semantic_hash": symbol.semantic_hash.as_ref().map(StableHash::as_str),
        "summary": &symbol.summary,
        "metadata": &symbol.metadata,
    })
}

pub(super) fn agent_mcp_debug_graph_edge_json(edge: &DebugGraphEdge) -> serde_json::Value {
    serde_json::json!({
        "program_hash": edge.program_hash.as_str(),
        "from_symbol_id": &edge.from_symbol_id,
        "to_symbol_id": &edge.to_symbol_id,
        "edge_kind": &edge.edge_kind,
        "weight": edge.weight,
        "metadata": &edge.metadata,
    })
}

pub(super) fn agent_mcp_debug_store_path(arguments: &serde_json::Value) -> &str {
    arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .unwrap_or(".arcweft/cache/agent-debug.sqlite3")
}

pub(super) fn agent_mcp_debug_search_model(
    arguments: &serde_json::Value,
    dimensions: usize,
) -> Result<EmbeddingModelDescriptor, String> {
    let model_id = agent_mcp_non_empty_string_argument(arguments, "model_id")
        .ok_or_else(|| "arcweft.debug.search query_vector requires model_id".to_owned())?;
    let model_revision = agent_mcp_non_empty_string_argument(arguments, "model_revision")
        .ok_or_else(|| "arcweft.debug.search query_vector requires model_revision".to_owned())?;
    let dimensions = u32::try_from(dimensions)
        .map_err(|_| "arcweft.debug.search query_vector has too many dimensions".to_owned())?;
    Ok(EmbeddingModelDescriptor {
        model_id: model_id.to_owned(),
        model_revision: model_revision.to_owned(),
        dimensions,
    })
}

pub(super) fn agent_mcp_non_empty_string_argument<'a>(
    arguments: &'a serde_json::Value,
    name: &str,
) -> Option<&'a str> {
    arguments
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn agent_mcp_query_vector_argument(
    arguments: &serde_json::Value,
    name: &str,
) -> Result<Option<Vec<f32>>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| parse_agent_mcp_query_vector_json_number(item, name))
            .collect::<Result<Vec<_>, _>>()
            .and_then(non_empty_agent_mcp_query_vector)
            .map(Some),
        serde_json::Value::String(text) => parse_agent_mcp_query_vector_string(text)
            .and_then(non_empty_agent_mcp_query_vector)
            .map(Some),
        _ => Err(format!(
            "arcweft.debug.search argument {name} must be an array of numbers or a comma-separated string"
        )),
    }
}

pub(super) fn parse_agent_mcp_query_vector_json_number(
    value: &serde_json::Value,
    name: &str,
) -> Result<f32, String> {
    if !value.is_number() {
        return Err(format!(
            "arcweft.debug.search argument {name} must contain finite numbers"
        ));
    }
    let parsed = value
        .to_string()
        .parse::<f32>()
        .map_err(|_| format!("arcweft.debug.search argument {name} must contain finite numbers"))?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(format!(
            "arcweft.debug.search argument {name} must contain finite numbers"
        ))
    }
}

pub(super) fn parse_agent_mcp_query_vector_string(value: &str) -> Result<Vec<f32>, String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f32>().map_err(|error| {
                format!("invalid arcweft.debug.search query_vector component `{part}`: {error}")
            })
        })
        .collect()
}

pub(super) fn non_empty_agent_mcp_query_vector(values: Vec<f32>) -> Result<Vec<f32>, String> {
    if values.is_empty() {
        return Err("arcweft.debug.search query_vector must not be empty".to_owned());
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(
            "arcweft.debug.search query_vector must contain only finite numbers".to_owned(),
        );
    }
    Ok(values)
}

pub(super) const fn agent_mcp_search_channel_label(channel: SearchChannel) -> &'static str {
    match channel {
        SearchChannel::ExactEntity => "exact_entity",
        SearchChannel::Lexical => "lexical",
        SearchChannel::Vector => "vector",
        SearchChannel::Graph => "graph",
        SearchChannel::History => "history",
        SearchChannel::Diagnostics => "diagnostics",
        SearchChannel::Trace => "trace",
        SearchChannel::Summary => "summary",
    }
}
