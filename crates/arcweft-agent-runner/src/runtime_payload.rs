use std::collections::{BTreeMap, BTreeSet};

use arcweft_agent_protocol::{
    action::{AgentActionDispatch, AgentActionKind, AgentActionTarget},
    artifact::RequiredEntity,
    ids::PublicId,
    protocol::{
        AgentHostResponse, AgentProjectGraph, AgentProjectGraphEdge, AgentProjectGraphNeighborhood,
        AgentProjectGraphSymbol,
    },
    value::AgentValue,
};
use arcweft_core::value::{RuntimePayload, RuntimeValue};

use crate::runtime_value::runtime_field;

pub(crate) fn runtime_payload_from_response(response: &AgentHostResponse) -> RuntimePayload {
    RuntimePayload::new(match response {
        AgentHostResponse::Observation(observation) => RuntimeValue::Record(vec![
            runtime_field("tick", RuntimeValue::u64(observation.tick)),
            runtime_field(
                "frame_id",
                RuntimeValue::String(observation.frame_id.clone()),
            ),
            runtime_field(
                "state_hash",
                RuntimeValue::String(observation.state_hash.clone()),
            ),
            runtime_field(
                "render_hash",
                RuntimeValue::String(observation.render_hash.clone()),
            ),
            runtime_field("actions", runtime_action_targets(&observation.actions)),
            runtime_field("objects", runtime_observed_objects(&observation.payload)),
            runtime_field("signals", runtime_agent_value_fields(&observation.signals)),
        ]),
        AgentHostResponse::Action(result) => RuntimeValue::Record(vec![
            runtime_field("accepted", RuntimeValue::Bool(result.accepted)),
            runtime_field("before_tick", RuntimeValue::u64(result.before_tick)),
            runtime_field("after_tick", RuntimeValue::u64(result.after_tick)),
            runtime_field(
                "before_state_hash",
                RuntimeValue::String(result.before_state_hash.clone()),
            ),
            runtime_field(
                "after_state_hash",
                RuntimeValue::String(result.after_state_hash.clone()),
            ),
        ]),
        AgentHostResponse::Capture(result) => RuntimeValue::Record(vec![
            runtime_field("uri", RuntimeValue::String(result.uri.as_str().to_owned())),
            runtime_field(
                "content_hash",
                RuntimeValue::String(result.content_hash.clone()),
            ),
            runtime_field(
                "media_type",
                RuntimeValue::String(result.media_type.clone()),
            ),
            runtime_field("byte_len", RuntimeValue::u64(result.byte_len)),
        ]),
        AgentHostResponse::Resource(value) => runtime_resource_payload(value),
        AgentHostResponse::EntityMetadata(metadata) => runtime_entity_metadata_payload(metadata),
        AgentHostResponse::ProjectGraphNeighborhood(neighborhood) => {
            runtime_project_graph_neighborhood_payload(neighborhood)
        }
        AgentHostResponse::RagContext(value) => runtime_rag_context_payload(value),
        AgentHostResponse::Unit => RuntimeValue::Unit,
    })
}

pub(crate) fn project_graph_neighborhood(
    graph: &AgentProjectGraph,
    root: &PublicId,
    depth: u32,
) -> Option<AgentProjectGraphNeighborhood> {
    let root_symbol = graph
        .symbols
        .iter()
        .find(|symbol| symbol.public_id.as_ref() == Some(root))?;
    let mut selected_symbols = BTreeSet::from([root_symbol.symbol_id.clone()]);
    let mut frontier = BTreeSet::from([root_symbol.symbol_id.clone()]);
    let mut selected_edges = BTreeSet::new();
    for _ in 0..depth {
        let mut next_frontier = BTreeSet::new();
        for (index, edge) in graph.edges.iter().enumerate() {
            let touches_frontier =
                frontier.contains(&edge.from_symbol_id) || frontier.contains(&edge.to_symbol_id);
            if !touches_frontier {
                continue;
            }
            selected_edges.insert(index);
            if selected_symbols.insert(edge.from_symbol_id.clone()) {
                next_frontier.insert(edge.from_symbol_id.clone());
            }
            if selected_symbols.insert(edge.to_symbol_id.clone()) {
                next_frontier.insert(edge.to_symbol_id.clone());
            }
        }
        frontier = next_frontier;
        if frontier.is_empty() {
            break;
        }
    }
    Some(AgentProjectGraphNeighborhood {
        root: root.clone(),
        symbols: graph
            .symbols
            .iter()
            .filter(|symbol| selected_symbols.contains(&symbol.symbol_id))
            .cloned()
            .collect(),
        edges: graph
            .edges
            .iter()
            .enumerate()
            .filter(|(index, _)| selected_edges.contains(index))
            .map(|(_, edge)| edge.clone())
            .collect(),
    })
}

fn runtime_entity_metadata_payload(metadata: &RequiredEntity) -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field(
            "id",
            RuntimeValue::String(metadata.public_id.as_str().to_owned()),
        ),
        runtime_field("kind", RuntimeValue::String(metadata.kind.clone())),
        runtime_field(
            "semantic_hash",
            RuntimeValue::String(metadata.semantic_hash.as_str().to_owned()),
        ),
        runtime_field(
            "source",
            runtime_entity_source_anchor_payload(metadata.source_anchor.as_ref()),
        ),
    ])
}

fn runtime_entity_source_anchor_payload(
    source: Option<&arcweft_agent_protocol::artifact::RequiredEntitySourceAnchor>,
) -> RuntimeValue {
    let Some(source) = source else {
        return RuntimeValue::Record(vec![
            runtime_field("has_source", RuntimeValue::Bool(false)),
            runtime_field("path", RuntimeValue::String(String::new())),
            runtime_field("start_byte", RuntimeValue::u64(0)),
            runtime_field("end_byte", RuntimeValue::u64(0)),
            runtime_field("start_line", RuntimeValue::u32(0)),
            runtime_field("start_column", RuntimeValue::u32(0)),
            runtime_field("end_line", RuntimeValue::u32(0)),
            runtime_field("end_column", RuntimeValue::u32(0)),
        ]);
    };
    RuntimeValue::Record(vec![
        runtime_field("has_source", RuntimeValue::Bool(true)),
        runtime_field("path", RuntimeValue::String(source.path.clone())),
        runtime_field("start_byte", RuntimeValue::u64(source.start_byte)),
        runtime_field("end_byte", RuntimeValue::u64(source.end_byte)),
        runtime_field(
            "start_line",
            RuntimeValue::u32(source.start.map_or(0, |position| position.line)),
        ),
        runtime_field(
            "start_column",
            RuntimeValue::u32(source.start.map_or(0, |position| position.column)),
        ),
        runtime_field(
            "end_line",
            RuntimeValue::u32(source.end.map_or(0, |position| position.line)),
        ),
        runtime_field(
            "end_column",
            RuntimeValue::u32(source.end.map_or(0, |position| position.column)),
        ),
    ])
}

fn runtime_project_graph_neighborhood_payload(
    neighborhood: &AgentProjectGraphNeighborhood,
) -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field(
            "root",
            RuntimeValue::String(neighborhood.root.as_str().to_owned()),
        ),
        runtime_field(
            "node_count",
            RuntimeValue::u32(u32::try_from(neighborhood.symbols.len()).unwrap_or(u32::MAX)),
        ),
        runtime_field(
            "edge_count",
            RuntimeValue::u32(u32::try_from(neighborhood.edges.len()).unwrap_or(u32::MAX)),
        ),
        runtime_field(
            "symbols",
            arcweft_core::value::runtime_sequence_values(
                neighborhood
                    .symbols
                    .iter()
                    .map(runtime_project_graph_symbol_payload)
                    .collect(),
            ),
        ),
        runtime_field(
            "edges",
            arcweft_core::value::runtime_sequence_values(
                neighborhood
                    .edges
                    .iter()
                    .map(runtime_project_graph_edge_payload)
                    .collect(),
            ),
        ),
    ])
}

pub(crate) fn runtime_project_graph_symbol_payload(
    symbol: &AgentProjectGraphSymbol,
) -> RuntimeValue {
    let flow_control = symbol.flow_control;
    let project_summary = symbol.project_summary;
    RuntimeValue::Record(vec![
        runtime_field("symbol_id", RuntimeValue::String(symbol.symbol_id.clone())),
        runtime_field(
            "has_entity",
            RuntimeValue::Bool(symbol.public_id.as_ref().is_some()),
        ),
        runtime_field(
            "id",
            RuntimeValue::String(
                symbol
                    .public_id
                    .as_ref()
                    .map_or("", PublicId::as_str)
                    .to_owned(),
            ),
        ),
        runtime_field("kind", RuntimeValue::String(symbol.kind.clone())),
        runtime_field(
            "has_flow_control",
            RuntimeValue::Bool(flow_control.is_some()),
        ),
        runtime_field(
            "has_project_summary",
            RuntimeValue::Bool(project_summary.is_some()),
        ),
        runtime_field(
            "entity_count",
            RuntimeValue::u32(project_summary.map_or(0, |summary| summary.entity_count)),
        ),
        runtime_field(
            "agent_action_count",
            RuntimeValue::u32(project_summary.map_or(0, |summary| summary.agent_action_count)),
        ),
        runtime_field(
            "project_callable_count",
            RuntimeValue::u32(project_summary.map_or(0, |summary| summary.project_callable_count)),
        ),
        runtime_field(
            "relation_count",
            RuntimeValue::u32(project_summary.map_or(0, |summary| summary.relation_count)),
        ),
        runtime_field(
            "dependency_edge_count",
            RuntimeValue::u32(project_summary.map_or(0, |summary| summary.dependency_edge_count)),
        ),
        runtime_field(
            "dynamic_control_flow_count",
            RuntimeValue::u32(
                project_summary.map_or(0, |summary| summary.dynamic_control_flow_count),
            ),
        ),
        runtime_field(
            "debug_query_count",
            RuntimeValue::u32(project_summary.map_or(0, |summary| summary.debug_query_count)),
        ),
        runtime_field(
            "has_dynamic_control",
            RuntimeValue::Bool(flow_control.is_some_and(|summary| summary.has_dynamic_control)),
        ),
        runtime_field(
            "static_goto_count",
            RuntimeValue::u32(flow_control.map_or(0, |summary| summary.static_goto_count)),
        ),
        runtime_field(
            "dynamic_goto_count",
            RuntimeValue::u32(flow_control.map_or(0, |summary| summary.dynamic_goto_count)),
        ),
        runtime_field(
            "branch_count",
            RuntimeValue::u32(flow_control.map_or(0, |summary| summary.branch_count)),
        ),
        runtime_field(
            "loop_count",
            RuntimeValue::u32(flow_control.map_or(0, |summary| summary.loop_count)),
        ),
        runtime_field(
            "await_count",
            RuntimeValue::u32(flow_control.map_or(0, |summary| summary.await_count)),
        ),
        runtime_field(
            "thread_count",
            RuntimeValue::u32(flow_control.map_or(0, |summary| summary.thread_count)),
        ),
        runtime_field(
            "select_branch_count",
            RuntimeValue::u32(flow_control.map_or(0, |summary| summary.select_branch_count)),
        ),
        runtime_field(
            "has_semantic_hash",
            RuntimeValue::Bool(symbol.semantic_hash.as_ref().is_some()),
        ),
        runtime_field(
            "semantic_hash",
            RuntimeValue::String(symbol.semantic_hash.clone().unwrap_or_default()),
        ),
        runtime_field("summary", RuntimeValue::String(symbol.summary.clone())),
    ])
}

fn runtime_project_graph_edge_payload(edge: &AgentProjectGraphEdge) -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field(
            "from_symbol_id",
            RuntimeValue::String(edge.from_symbol_id.clone()),
        ),
        runtime_field(
            "to_symbol_id",
            RuntimeValue::String(edge.to_symbol_id.clone()),
        ),
        runtime_field("kind", RuntimeValue::String(edge.edge_kind.clone())),
    ])
}
pub(crate) fn runtime_rag_context_payload(value: &serde_json::Value) -> RuntimeValue {
    let item_count = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    RuntimeValue::Record(vec![
        runtime_field("summary", RuntimeValue::String(rag_context_summary(value))),
        runtime_field(
            "item_count",
            RuntimeValue::usize(u64::try_from(item_count).unwrap_or(u64::MAX)),
        ),
        runtime_field(
            "truncated",
            RuntimeValue::Bool(
                value
                    .get("truncated")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            ),
        ),
        runtime_field("json", RuntimeValue::String(value.to_string())),
    ])
}

fn rag_context_summary(value: &serde_json::Value) -> String {
    let query = value
        .get("query")
        .and_then(|query| query.get("text"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let item_count = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if query.is_empty() {
        format!("{item_count} RAG context item(s)")
    } else {
        format!("{item_count} RAG context item(s) for `{query}`")
    }
}

pub(crate) fn runtime_resource_payload(value: &serde_json::Value) -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field("uri", runtime_json_string_field(value, "uri")),
        runtime_field("kind", runtime_json_string_field(value, "kind")),
        runtime_field("mime_type", runtime_json_string_field(value, "mime_type")),
        runtime_field("hash", runtime_json_string_field(value, "hash")),
        runtime_field("body", runtime_resource_body_payload(value.get("body"))),
    ])
}

fn runtime_action_targets(actions: &[AgentActionTarget]) -> RuntimeValue {
    RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(
        actions
            .iter()
            .map(|action| {
                RuntimeValue::Record(vec![
                    runtime_field("id", RuntimeValue::String(action.id.clone())),
                    runtime_field("target", RuntimeValue::String(action.target.clone())),
                    runtime_field(
                        "action",
                        RuntimeValue::String(agent_action_kind_label(action.action).to_owned()),
                    ),
                    runtime_field(
                        "kind",
                        RuntimeValue::String(agent_action_dispatch_label(action.kind).to_owned()),
                    ),
                    runtime_field("enabled", RuntimeValue::Bool(action.enabled)),
                ])
            })
            .collect(),
    ))
}

fn runtime_observed_objects(payload: &serde_json::Value) -> RuntimeValue {
    RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(
        payload
            .get("objects")
            .and_then(serde_json::Value::as_array)
            .map_or_else(Vec::new, |objects| {
                objects.iter().map(runtime_observed_object).collect()
            }),
    ))
}

fn runtime_observed_object(object: &serde_json::Value) -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field("id", runtime_json_string_field(object, "id")),
        runtime_field("parent_id", runtime_json_string_field(object, "parent_id")),
        runtime_field("entity", runtime_json_string_field(object, "entity")),
        runtime_field("layer", runtime_json_string_field(object, "layer")),
        runtime_field("role", runtime_json_string_field(object, "role")),
        runtime_field(
            "visible",
            RuntimeValue::Bool(
                object
                    .get("visible")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            ),
        ),
        runtime_field(
            "enabled",
            RuntimeValue::Bool(
                object
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true),
            ),
        ),
        runtime_field("bbox", runtime_bbox(object.get("bbox"))),
        runtime_field("text", runtime_json_string_field(object, "text")),
    ])
}

fn runtime_bbox(value: Option<&serde_json::Value>) -> RuntimeValue {
    let Some(value) = value else {
        return RuntimeValue::Record(vec![
            runtime_field("space", RuntimeValue::String(String::new())),
            runtime_field("x", RuntimeValue::u32(0)),
            runtime_field("y", RuntimeValue::u32(0)),
            runtime_field("width", RuntimeValue::u32(0)),
            runtime_field("height", RuntimeValue::u32(0)),
        ]);
    };
    RuntimeValue::Record(vec![
        runtime_field("space", runtime_json_string_field(value, "space")),
        runtime_field("x", runtime_json_u32_field(value, "x")),
        runtime_field("y", runtime_json_u32_field(value, "y")),
        runtime_field("width", runtime_json_u32_field(value, "width")),
        runtime_field("height", runtime_json_u32_field(value, "height")),
    ])
}

fn runtime_agent_value_fields(values: &BTreeMap<String, AgentValue>) -> RuntimeValue {
    RuntimeValue::Record(
        values
            .iter()
            .map(|(name, value)| runtime_field(name, runtime_agent_value_payload(value)))
            .collect(),
    )
}

fn runtime_agent_value_payload(value: &AgentValue) -> RuntimeValue {
    match value {
        AgentValue::Null => RuntimeValue::Unit,
        AgentValue::Bool(value) => RuntimeValue::Bool(*value),
        AgentValue::I64(value) => RuntimeValue::i64(*value),
        AgentValue::U64(value) => RuntimeValue::u64(*value),
        AgentValue::F64(value) => RuntimeValue::F64(*value),
        AgentValue::String(value) => RuntimeValue::String(value.clone()),
        AgentValue::Entity(value) => RuntimeValue::EntityRef(value.as_str().to_owned()),
        AgentValue::List(values) => RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(
            values.iter().map(runtime_agent_value_payload).collect(),
        )),
        AgentValue::Map(values) => RuntimeValue::Record(
            values
                .iter()
                .map(|(name, value)| runtime_field(name, runtime_agent_value_payload(value)))
                .collect(),
        ),
    }
}

fn agent_action_kind_label(kind: AgentActionKind) -> &'static str {
    match kind {
        AgentActionKind::AdvanceText => "advance_text",
        AgentActionKind::SelectChoice => "select_choice",
        AgentActionKind::Invoke => "invoke",
        AgentActionKind::PointerClick => "pointer_click",
    }
}

fn agent_action_dispatch_label(kind: AgentActionDispatch) -> &'static str {
    match kind {
        AgentActionDispatch::Semantic => "semantic",
        AgentActionDispatch::Physical => "physical",
    }
}

fn runtime_json_string_field(value: &serde_json::Value, field: &str) -> RuntimeValue {
    RuntimeValue::String(
        value
            .get(field)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    )
}

fn runtime_json_u32_field(value: &serde_json::Value, field: &str) -> RuntimeValue {
    RuntimeValue::u32(
        value
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0),
    )
}

fn runtime_resource_body_payload(value: Option<&serde_json::Value>) -> RuntimeValue {
    let Some(value) = value else {
        return runtime_empty_resource_body();
    };
    let kind = value
        .get("body_kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let body = value.get("body");
    match kind {
        "json" => RuntimeValue::Record(vec![
            runtime_field("kind", RuntimeValue::String(kind.to_owned())),
            runtime_field(
                "json",
                RuntimeValue::String(body.map_or_else(String::new, serde_json::Value::to_string)),
            ),
            runtime_field(
                "value",
                body.map_or(RuntimeValue::Unit, runtime_value_from_json),
            ),
            runtime_field("text", RuntimeValue::String(String::new())),
            runtime_field("base64", RuntimeValue::String(String::new())),
            runtime_field("encoding", RuntimeValue::String(String::new())),
        ]),
        "text" => RuntimeValue::Record(vec![
            runtime_field("kind", RuntimeValue::String(kind.to_owned())),
            runtime_field("json", RuntimeValue::String(String::new())),
            runtime_field(
                "value",
                RuntimeValue::String(
                    body.and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                ),
            ),
            runtime_field(
                "text",
                RuntimeValue::String(
                    body.and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                ),
            ),
            runtime_field("base64", RuntimeValue::String(String::new())),
            runtime_field("encoding", RuntimeValue::String(String::new())),
        ]),
        "bytes_base64" => runtime_bytes_base64_body_payload(kind, body),
        _ => runtime_empty_resource_body(),
    }
}

fn runtime_bytes_base64_body_payload(kind: &str, body: Option<&serde_json::Value>) -> RuntimeValue {
    let data = body
        .and_then(|body| body.get("data"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let encoding = body
        .and_then(|body| body.get("encoding"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    RuntimeValue::Record(vec![
        runtime_field("kind", RuntimeValue::String(kind.to_owned())),
        runtime_field("json", RuntimeValue::String(String::new())),
        runtime_field("value", runtime_bytes_base64_value(body)),
        runtime_field("text", RuntimeValue::String(String::new())),
        runtime_field("base64", RuntimeValue::String(data)),
        runtime_field("encoding", RuntimeValue::String(encoding)),
    ])
}

fn runtime_empty_resource_body() -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field("kind", RuntimeValue::String(String::new())),
        runtime_field("json", RuntimeValue::String(String::new())),
        runtime_field("value", RuntimeValue::Unit),
        runtime_field("text", RuntimeValue::String(String::new())),
        runtime_field("base64", RuntimeValue::String(String::new())),
        runtime_field("encoding", RuntimeValue::String(String::new())),
    ])
}

fn runtime_bytes_base64_value(body: Option<&serde_json::Value>) -> RuntimeValue {
    RuntimeValue::Record(vec![
        runtime_field(
            "encoding",
            RuntimeValue::String(
                body.and_then(|body| body.get("encoding"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        ),
        runtime_field(
            "data",
            RuntimeValue::String(
                body.and_then(|body| body.get("data"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        ),
    ])
}

fn runtime_value_from_json(value: &serde_json::Value) -> RuntimeValue {
    match value {
        serde_json::Value::Null => RuntimeValue::Unit,
        serde_json::Value::Bool(value) => RuntimeValue::Bool(*value),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(RuntimeValue::i64)
            .or_else(|| value.as_u64().map(RuntimeValue::u64))
            .or_else(|| value.as_f64().map(RuntimeValue::F64))
            .unwrap_or(RuntimeValue::Unit),
        serde_json::Value::String(value) => RuntimeValue::String(value.clone()),
        serde_json::Value::Array(values) => RuntimeValue::Tuple(
            values
                .iter()
                .map(runtime_value_from_json)
                .collect::<Vec<_>>(),
        ),
        serde_json::Value::Object(values) => RuntimeValue::Record(
            values
                .iter()
                .map(|(key, value)| runtime_field(key, runtime_value_from_json(value)))
                .collect(),
        ),
    }
}
