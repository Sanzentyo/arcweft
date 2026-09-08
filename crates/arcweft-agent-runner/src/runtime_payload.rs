use std::collections::{BTreeMap, BTreeSet};

use arcweft_agent_protocol::{
    action::{AgentActionDispatch, AgentActionKind, AgentActionTarget},
    artifact::{RequiredEntity, RequiredEntitySourceAnchor, RequiredEntitySourcePosition},
    geometry::{AgentBBox, AgentCoordinateSpace},
    ids::AgentProjectGraphSymbolId,
    object::AgentObservedObject,
    protocol::{
        AgentHostResponse, AgentProjectFlowControlSummary, AgentProjectGraph,
        AgentProjectGraphEdge, AgentProjectGraphNeighborhood, AgentProjectGraphSummary,
        AgentProjectGraphSymbol,
    },
    resource::{AgentResource, AgentResourceBody},
    value::AgentValue,
};
use arcweft_core::{
    entry::RuntimeCommandTargetId,
    pattern::RuntimeBuiltinVariantCaseIdentity,
    value::{
        RuntimeAgentAction, RuntimeAgentActionDispatch, RuntimeAgentActionTarget,
        RuntimeAgentField, RuntimeAgentValue, RuntimePayload, RuntimeValue,
    },
};
use arcweft_debug_model::rag::RagContextPack;

use crate::error::{AgentHostResponseAdmissionError, AgentHostResponseKind};
use crate::runtime_value::{runtime_field, runtime_record};

fn agent_field(field: RuntimeAgentField, value: RuntimeValue) -> (String, RuntimeValue) {
    runtime_field(field.as_label(), value)
}

fn runtime_optional<T>(value: Option<T>, some: impl FnOnce(T) -> RuntimeValue) -> RuntimeValue {
    value.map_or_else(RuntimeValue::option_none, |value| {
        RuntimeValue::option_some(some(value))
    })
}

fn runtime_optional_string(value: Option<&str>) -> RuntimeValue {
    runtime_optional(value, |value| RuntimeValue::String(value.to_owned()))
}

pub(crate) fn runtime_payload_from_response(
    response: &AgentHostResponse,
) -> Result<RuntimePayload, AgentHostResponseAdmissionError> {
    let value = match response {
        AgentHostResponse::Observation(observation) => runtime_record(vec![
            agent_field(
                RuntimeAgentField::ObservationTick,
                RuntimeValue::u64(observation.tick),
            ),
            agent_field(
                RuntimeAgentField::ObservationFrameId,
                RuntimeValue::String(observation.frame_id.clone()),
            ),
            agent_field(
                RuntimeAgentField::ObservationStateHash,
                RuntimeValue::String(observation.state_hash.clone()),
            ),
            agent_field(
                RuntimeAgentField::ObservationRenderHash,
                RuntimeValue::String(observation.render_hash.clone()),
            ),
            agent_field(
                RuntimeAgentField::ObservationActions,
                runtime_action_targets(&observation.actions)?,
            ),
            agent_field(
                RuntimeAgentField::ObservationObjects,
                runtime_observed_objects(&observation.payload)?,
            ),
            agent_field(
                RuntimeAgentField::ObservationSignals,
                runtime_agent_value_fields(&observation.signals)?,
            ),
        ]),
        AgentHostResponse::Action(result) => runtime_record(vec![
            agent_field(
                RuntimeAgentField::ActionResultAccepted,
                RuntimeValue::Bool(result.accepted),
            ),
            agent_field(
                RuntimeAgentField::ActionResultBeforeTick,
                RuntimeValue::u64(result.before_tick),
            ),
            agent_field(
                RuntimeAgentField::ActionResultAfterTick,
                RuntimeValue::u64(result.after_tick),
            ),
            agent_field(
                RuntimeAgentField::ActionResultBeforeStateHash,
                RuntimeValue::String(result.before_state_hash.clone()),
            ),
            agent_field(
                RuntimeAgentField::ActionResultAfterStateHash,
                RuntimeValue::String(result.after_state_hash.clone()),
            ),
        ]),
        AgentHostResponse::Capture(result) => runtime_record(vec![
            agent_field(
                RuntimeAgentField::CaptureReferenceUri,
                RuntimeValue::String(result.uri.as_str().to_owned()),
            ),
            agent_field(
                RuntimeAgentField::CaptureReferenceContentHash,
                RuntimeValue::String(result.content_hash.clone()),
            ),
            agent_field(
                RuntimeAgentField::CaptureReferenceMediaType,
                RuntimeValue::String(result.media_type.clone()),
            ),
            agent_field(
                RuntimeAgentField::CaptureReferenceByteLen,
                RuntimeValue::u64(result.byte_len),
            ),
        ]),
        AgentHostResponse::Resource(value) => runtime_resource_payload(value)?,
        AgentHostResponse::EntityMetadata(metadata) => runtime_entity_metadata_payload(metadata),
        AgentHostResponse::ProjectGraphNeighborhood(neighborhood) => {
            runtime_project_graph_neighborhood_payload(neighborhood)?
        }
        AgentHostResponse::RagContext(value) => runtime_rag_context_payload(value)?,
        AgentHostResponse::Unit => RuntimeValue::Unit,
    };
    Ok(RuntimePayload::new(value))
}

pub(crate) fn project_graph_neighborhood(
    graph: &AgentProjectGraph,
    root: &AgentProjectGraphSymbolId,
    depth: u32,
) -> Option<AgentProjectGraphNeighborhood> {
    let root_symbol = graph
        .symbols
        .iter()
        .find(|symbol| &symbol.symbol_id == root)?;
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
    runtime_record(vec![
        agent_field(
            RuntimeAgentField::EntityMetadataId,
            RuntimeValue::String(metadata.public_id.as_str().to_owned()),
        ),
        agent_field(
            RuntimeAgentField::EntityMetadataKind,
            RuntimeValue::String(metadata.kind.clone()),
        ),
        agent_field(
            RuntimeAgentField::EntityMetadataSemanticHash,
            RuntimeValue::String(metadata.semantic_hash.as_str().to_owned()),
        ),
        agent_field(
            RuntimeAgentField::EntityMetadataSource,
            runtime_optional(
                metadata.source_anchor.as_ref(),
                runtime_entity_source_anchor_payload,
            ),
        ),
    ])
}

fn runtime_entity_source_anchor_payload(source: &RequiredEntitySourceAnchor) -> RuntimeValue {
    runtime_record(vec![
        agent_field(
            RuntimeAgentField::SourceAnchorPath,
            RuntimeValue::String(source.path.clone()),
        ),
        agent_field(
            RuntimeAgentField::SourceAnchorStartByte,
            RuntimeValue::u64(source.start_byte),
        ),
        agent_field(
            RuntimeAgentField::SourceAnchorEndByte,
            RuntimeValue::u64(source.end_byte),
        ),
        agent_field(
            RuntimeAgentField::SourceAnchorStart,
            runtime_optional(source.start, runtime_entity_source_position_payload),
        ),
        agent_field(
            RuntimeAgentField::SourceAnchorEnd,
            runtime_optional(source.end, runtime_entity_source_position_payload),
        ),
    ])
}

fn runtime_entity_source_position_payload(source: RequiredEntitySourcePosition) -> RuntimeValue {
    runtime_record(vec![
        agent_field(
            RuntimeAgentField::SourcePositionLine,
            RuntimeValue::u32(source.line),
        ),
        agent_field(
            RuntimeAgentField::SourcePositionColumn,
            RuntimeValue::u32(source.column),
        ),
    ])
}

fn runtime_project_graph_neighborhood_payload(
    neighborhood: &AgentProjectGraphNeighborhood,
) -> Result<RuntimeValue, AgentHostResponseAdmissionError> {
    let node_count = u32::try_from(neighborhood.symbols.len()).map_err(|_| {
        AgentHostResponseAdmissionError::CountOutOfRange {
            response: AgentHostResponseKind::ProjectGraphNeighborhood,
            path: "project_graph_neighborhood.symbols",
            actual: neighborhood.symbols.len(),
        }
    })?;
    let edge_count = u32::try_from(neighborhood.edges.len()).map_err(|_| {
        AgentHostResponseAdmissionError::CountOutOfRange {
            response: AgentHostResponseKind::ProjectGraphNeighborhood,
            path: "project_graph_neighborhood.edges",
            actual: neighborhood.edges.len(),
        }
    })?;
    Ok(runtime_record(vec![
        agent_field(
            RuntimeAgentField::ProjectGraphNeighborhoodRoot,
            RuntimeValue::String(neighborhood.root.as_str().to_owned()),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphNeighborhoodNodeCount,
            RuntimeValue::u32(node_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphNeighborhoodEdgeCount,
            RuntimeValue::u32(edge_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphNeighborhoodSymbols,
            arcweft_core::value::runtime_sequence_values(
                neighborhood
                    .symbols
                    .iter()
                    .map(runtime_project_graph_symbol_payload)
                    .collect(),
            ),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphNeighborhoodEdges,
            arcweft_core::value::runtime_sequence_values(
                neighborhood
                    .edges
                    .iter()
                    .map(runtime_project_graph_edge_payload)
                    .collect(),
            ),
        ),
    ]))
}

pub(crate) fn runtime_project_graph_symbol_payload(
    symbol: &AgentProjectGraphSymbol,
) -> RuntimeValue {
    runtime_record(vec![
        agent_field(
            RuntimeAgentField::ProjectGraphSymbolSymbolId,
            RuntimeValue::String(symbol.symbol_id.as_str().to_owned()),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSymbolId,
            runtime_optional_string(symbol.public_id.as_ref().map(|id| id.as_str())),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSymbolKind,
            RuntimeValue::String(symbol.kind.clone()),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSymbolSemanticHash,
            runtime_optional_string(symbol.semantic_hash.as_deref()),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSymbolFlowControl,
            runtime_optional(
                symbol.flow_control,
                runtime_project_flow_control_summary_payload,
            ),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSymbolProjectSummary,
            runtime_optional(
                symbol.project_summary,
                runtime_project_graph_summary_payload,
            ),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSymbolSummary,
            RuntimeValue::String(symbol.summary.clone()),
        ),
    ])
}

fn runtime_project_flow_control_summary_payload(
    summary: AgentProjectFlowControlSummary,
) -> RuntimeValue {
    runtime_record(vec![
        agent_field(
            RuntimeAgentField::ProjectFlowControlHasDynamicControl,
            RuntimeValue::Bool(summary.has_dynamic_control),
        ),
        agent_field(
            RuntimeAgentField::ProjectFlowControlStaticGotoCount,
            RuntimeValue::u32(summary.static_goto_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectFlowControlDynamicGotoCount,
            RuntimeValue::u32(summary.dynamic_goto_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectFlowControlBranchCount,
            RuntimeValue::u32(summary.branch_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectFlowControlLoopCount,
            RuntimeValue::u32(summary.loop_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectFlowControlAwaitCount,
            RuntimeValue::u32(summary.await_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectFlowControlThreadCount,
            RuntimeValue::u32(summary.thread_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectFlowControlSelectBranchCount,
            RuntimeValue::u32(summary.select_branch_count),
        ),
    ])
}

fn runtime_project_graph_summary_payload(summary: AgentProjectGraphSummary) -> RuntimeValue {
    runtime_record(vec![
        agent_field(
            RuntimeAgentField::ProjectGraphSummaryEntityCount,
            RuntimeValue::u32(summary.entity_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSummaryAgentActionCount,
            RuntimeValue::u32(summary.agent_action_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSummaryProjectCallableCount,
            RuntimeValue::u32(summary.project_callable_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSummaryRelationCount,
            RuntimeValue::u32(summary.relation_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSummaryDependencyEdgeCount,
            RuntimeValue::u32(summary.dependency_edge_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSummaryDynamicControlFlowCount,
            RuntimeValue::u32(summary.dynamic_control_flow_count),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphSummaryDebugQueryCount,
            RuntimeValue::u32(summary.debug_query_count),
        ),
    ])
}

fn runtime_project_graph_edge_payload(edge: &AgentProjectGraphEdge) -> RuntimeValue {
    runtime_record(vec![
        agent_field(
            RuntimeAgentField::ProjectGraphEdgeFromSymbolId,
            RuntimeValue::String(edge.from_symbol_id.as_str().to_owned()),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphEdgeToSymbolId,
            RuntimeValue::String(edge.to_symbol_id.as_str().to_owned()),
        ),
        agent_field(
            RuntimeAgentField::ProjectGraphEdgeKind,
            RuntimeValue::String(edge.edge_kind.clone()),
        ),
    ])
}
pub(crate) fn runtime_rag_context_payload(
    value: &serde_json::Value,
) -> Result<RuntimeValue, AgentHostResponseAdmissionError> {
    let context: RagContextPack =
        decode_response_shape(AgentHostResponseKind::RagContext, "rag_context", value)?;
    if context.schema_version != 1 {
        return Err(AgentHostResponseAdmissionError::InvalidValue {
            response: AgentHostResponseKind::RagContext,
            path: "rag_context.schema_version".to_owned(),
            value: context.schema_version.to_string(),
            expected: "schema version 1",
        });
    }
    let item_count = u64::try_from(context.items.len()).map_err(|_| {
        AgentHostResponseAdmissionError::CountOutOfRange {
            response: AgentHostResponseKind::RagContext,
            path: "rag_context.items",
            actual: context.items.len(),
        }
    })?;
    Ok(runtime_record(vec![
        runtime_field(
            "summary",
            RuntimeValue::String(rag_context_summary(&context)),
        ),
        runtime_field("item_count", RuntimeValue::usize(item_count)),
        runtime_field("truncated", RuntimeValue::Bool(context.truncated)),
        runtime_field("json", RuntimeValue::String(value.to_string())),
    ]))
}

fn rag_context_summary(value: &RagContextPack) -> String {
    let query = &value.query.text;
    let item_count = value.items.len();
    if query.is_empty() {
        format!("{item_count} RAG context item(s)")
    } else {
        format!("{item_count} RAG context item(s) for `{query}`")
    }
}

pub(crate) fn runtime_resource_payload(
    value: &serde_json::Value,
) -> Result<RuntimeValue, AgentHostResponseAdmissionError> {
    let resource: AgentResource =
        decode_response_shape(AgentHostResponseKind::Resource, "resource", value)?;
    Ok(runtime_record(vec![
        agent_field(
            RuntimeAgentField::ResourceUri,
            RuntimeValue::String(resource.uri.as_str().to_owned()),
        ),
        agent_field(
            RuntimeAgentField::ResourceKind,
            RuntimeValue::String(resource.kind.as_str().to_owned()),
        ),
        agent_field(
            RuntimeAgentField::ResourceMimeType,
            RuntimeValue::String(resource.mime_type.clone()),
        ),
        agent_field(
            RuntimeAgentField::ResourceHash,
            RuntimeValue::String(resource.hash.clone()),
        ),
        agent_field(
            RuntimeAgentField::ResourceBody,
            runtime_resource_body_payload(&resource.body)?,
        ),
    ]))
}

fn runtime_action_targets(
    actions: &[AgentActionTarget],
) -> Result<RuntimeValue, AgentHostResponseAdmissionError> {
    actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let id = RuntimeCommandTargetId::try_new(action.id.clone()).map_err(|error| {
                AgentHostResponseAdmissionError::InvalidIdentity {
                    response: AgentHostResponseKind::Observation,
                    path: format!("observation.actions[{index}].id"),
                    value: action.id.clone(),
                    detail: error.to_string(),
                }
            })?;
            let target =
                RuntimeCommandTargetId::try_new(action.target.clone()).map_err(|error| {
                    AgentHostResponseAdmissionError::InvalidIdentity {
                        response: AgentHostResponseKind::Observation,
                        path: format!("observation.actions[{index}].target"),
                        value: action.target.clone(),
                        detail: error.to_string(),
                    }
                })?;
            Ok(RuntimeValue::Agent(RuntimeAgentValue::ActionTarget(
                RuntimeAgentActionTarget::new(
                    id,
                    target,
                    runtime_agent_action(action.action),
                    runtime_agent_action_dispatch(action.kind),
                    action.enabled,
                ),
            )))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(arcweft_core::value::runtime_sequence_values)
}

fn runtime_observed_objects(
    payload: &serde_json::Value,
) -> Result<RuntimeValue, AgentHostResponseAdmissionError> {
    let objects =
        payload
            .get("objects")
            .ok_or_else(|| AgentHostResponseAdmissionError::InvalidValue {
                response: AgentHostResponseKind::Observation,
                path: "observation.payload.objects".to_owned(),
                value: "<missing>".to_owned(),
                expected: "an objects array",
            })?;
    let objects = objects.as_array().ok_or_else(|| {
        invalid_shape_from_value::<Vec<AgentObservedObject>>(
            AgentHostResponseKind::Observation,
            "observation.payload.objects",
            objects,
        )
    })?;
    objects
        .iter()
        .enumerate()
        .map(|(index, object)| {
            decode_response_shape(
                AgentHostResponseKind::Observation,
                &format!("observation.payload.objects[{index}]"),
                object,
            )
            .map(|object: AgentObservedObject| runtime_observed_object(&object))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|objects| RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(objects)))
}

fn runtime_observed_object(object: &AgentObservedObject) -> RuntimeValue {
    runtime_record(vec![
        agent_field(
            RuntimeAgentField::ObservedObjectId,
            RuntimeValue::String(object.id.clone()),
        ),
        agent_field(
            RuntimeAgentField::ObservedObjectParentId,
            runtime_optional_string(object.parent_id.as_deref()),
        ),
        agent_field(
            RuntimeAgentField::ObservedObjectEntity,
            runtime_optional_string(object.entity.as_deref()),
        ),
        agent_field(
            RuntimeAgentField::ObservedObjectLayer,
            RuntimeValue::String(object.layer.clone()),
        ),
        agent_field(
            RuntimeAgentField::ObservedObjectRole,
            RuntimeValue::String(object.role.clone()),
        ),
        agent_field(
            RuntimeAgentField::ObservedObjectVisible,
            RuntimeValue::Bool(object.visible),
        ),
        agent_field(
            RuntimeAgentField::ObservedObjectEnabled,
            RuntimeValue::Bool(object.enabled),
        ),
        agent_field(
            RuntimeAgentField::ObservedObjectBoundingBox,
            runtime_bbox(&object.bbox),
        ),
        agent_field(
            RuntimeAgentField::ObservedObjectText,
            runtime_optional_string(object.text.as_deref()),
        ),
    ])
}

fn runtime_bbox(value: &AgentBBox) -> RuntimeValue {
    let space = match value.space {
        AgentCoordinateSpace::Viewport => "viewport",
        AgentCoordinateSpace::World => "world",
        AgentCoordinateSpace::View => "view",
    };
    runtime_record(vec![
        agent_field(
            RuntimeAgentField::BoundingBoxSpace,
            RuntimeValue::String(space.to_owned()),
        ),
        agent_field(RuntimeAgentField::BoundingBoxX, RuntimeValue::u32(value.x)),
        agent_field(RuntimeAgentField::BoundingBoxY, RuntimeValue::u32(value.y)),
        agent_field(
            RuntimeAgentField::BoundingBoxWidth,
            RuntimeValue::u32(value.width),
        ),
        agent_field(
            RuntimeAgentField::BoundingBoxHeight,
            RuntimeValue::u32(value.height),
        ),
    ])
}

fn runtime_agent_value_fields(
    values: &BTreeMap<String, AgentValue>,
) -> Result<RuntimeValue, AgentHostResponseAdmissionError> {
    values
        .iter()
        .map(|(name, value)| {
            runtime_agent_value_payload(value, &format!("observation.signals.{name}"))
                .map(|value| runtime_field(name, value))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(runtime_record)
}

fn runtime_agent_value_payload(
    value: &AgentValue,
    path: &str,
) -> Result<RuntimeValue, AgentHostResponseAdmissionError> {
    match value {
        AgentValue::Null => Ok(RuntimeValue::Unit),
        AgentValue::Bool(value) => Ok(RuntimeValue::Bool(*value)),
        AgentValue::I64(value) => Ok(RuntimeValue::i64(*value)),
        AgentValue::U64(value) => Ok(RuntimeValue::u64(*value)),
        AgentValue::F64(value) if value.is_finite() => Ok(RuntimeValue::F64(*value)),
        AgentValue::F64(value) => Err(AgentHostResponseAdmissionError::InvalidValue {
            response: AgentHostResponseKind::Observation,
            path: path.to_owned(),
            value: value.to_string(),
            expected: "a finite Agent numeric value",
        }),
        AgentValue::String(value) => Ok(RuntimeValue::String(value.clone())),
        AgentValue::Entity(value) => Ok(RuntimeValue::String(value.as_str().to_owned())),
        AgentValue::List(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| runtime_agent_value_payload(value, &format!("{path}[{index}]")))
            .collect::<Result<Vec<_>, _>>()
            .map(|values| RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(values))),
        AgentValue::Map(values) => values
            .iter()
            .map(|(name, value)| {
                runtime_agent_value_payload(value, &format!("{path}.{name}"))
                    .map(|value| runtime_field(name, value))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_record),
    }
}

const fn runtime_agent_action(kind: AgentActionKind) -> RuntimeAgentAction {
    match kind {
        AgentActionKind::AdvanceText => RuntimeAgentAction::AdvanceText,
        AgentActionKind::SelectChoice => RuntimeAgentAction::SelectChoice,
        AgentActionKind::Invoke => RuntimeAgentAction::Invoke,
        AgentActionKind::Scroll => RuntimeAgentAction::Scroll,
        AgentActionKind::PointerClick => RuntimeAgentAction::PointerClick,
    }
}

const fn runtime_agent_action_dispatch(kind: AgentActionDispatch) -> RuntimeAgentActionDispatch {
    match kind {
        AgentActionDispatch::Semantic => RuntimeAgentActionDispatch::Semantic,
        AgentActionDispatch::Physical => RuntimeAgentActionDispatch::Physical,
    }
}

fn runtime_resource_body_payload(
    value: &AgentResourceBody,
) -> Result<RuntimeValue, AgentHostResponseAdmissionError> {
    match value {
        AgentResourceBody::Json(body) => runtime_builtin_variant(
            RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyJson,
            Some(runtime_value_from_json(body, "resource.body")?),
            "resource.body",
        ),
        AgentResourceBody::Text(body) => runtime_builtin_variant(
            RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyText,
            Some(RuntimeValue::String(body.clone())),
            "resource.body",
        ),
        AgentResourceBody::BytesBase64(body) => {
            let encoding = RuntimeValue::try_builtin_variant(
                RuntimeBuiltinVariantCaseIdentity::AgentBinaryEncodingBase64,
                None,
            )
            .map_err(|error| invalid_builtin_variant("resource.body.encoding", error))?;
            let binary_body = runtime_record(vec![
                agent_field(RuntimeAgentField::BinaryBodyEncoding, encoding),
                agent_field(
                    RuntimeAgentField::BinaryBodyData,
                    RuntimeValue::Agent(RuntimeAgentValue::BinaryData(body.data.clone())),
                ),
            ]);
            runtime_builtin_variant(
                RuntimeBuiltinVariantCaseIdentity::AgentResourceBodyBytesBase64,
                Some(binary_body),
                "resource.body",
            )
        }
    }
}

fn runtime_builtin_variant(
    case: RuntimeBuiltinVariantCaseIdentity,
    payload: Option<RuntimeValue>,
    path: &'static str,
) -> Result<RuntimeValue, AgentHostResponseAdmissionError> {
    RuntimeValue::try_builtin_variant(case, payload)
        .map_err(|error| invalid_builtin_variant(path, error))
}

fn invalid_builtin_variant(
    path: &'static str,
    error: impl std::fmt::Display,
) -> AgentHostResponseAdmissionError {
    AgentHostResponseAdmissionError::InvalidValue {
        response: AgentHostResponseKind::Resource,
        path: path.to_owned(),
        value: error.to_string(),
        expected: "canonical builtin variant case",
    }
}

fn runtime_value_from_json(
    value: &serde_json::Value,
    path: &str,
) -> Result<RuntimeValue, AgentHostResponseAdmissionError> {
    match value {
        serde_json::Value::Null => Ok(RuntimeValue::Unit),
        serde_json::Value::Bool(value) => Ok(RuntimeValue::Bool(*value)),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(RuntimeValue::i64)
            .or_else(|| value.as_u64().map(RuntimeValue::u64))
            .or_else(|| value.as_f64().map(RuntimeValue::F64))
            .ok_or_else(|| AgentHostResponseAdmissionError::NumberOutOfRange {
                response: AgentHostResponseKind::Resource,
                path: path.to_owned(),
                value: value.to_string(),
            }),
        serde_json::Value::String(value) => Ok(RuntimeValue::String(value.clone())),
        serde_json::Value::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| runtime_value_from_json(value, &format!("{path}[{index}]")))
            .collect::<Result<Vec<_>, _>>()
            .map(|values| RuntimeValue::Seq(arcweft_core::value::RuntimeSeq::values(values))),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| {
                runtime_value_from_json(value, &format!("{path}.{key}"))
                    .map(|value| runtime_field(key, value))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(runtime_record),
    }
}

fn decode_response_shape<T: serde::de::DeserializeOwned>(
    response: AgentHostResponseKind,
    path: &str,
    value: &serde_json::Value,
) -> Result<T, AgentHostResponseAdmissionError> {
    serde_json::from_value(value.clone()).map_err(|source| {
        AgentHostResponseAdmissionError::InvalidShape {
            response,
            path: path.to_owned(),
            source,
        }
    })
}

fn invalid_shape_from_value<T: serde::de::DeserializeOwned>(
    response: AgentHostResponseKind,
    path: &str,
    value: &serde_json::Value,
) -> AgentHostResponseAdmissionError {
    match serde_json::from_value::<T>(value.clone()) {
        Err(source) => AgentHostResponseAdmissionError::InvalidShape {
            response,
            path: path.to_owned(),
            source,
        },
        Ok(_) => unreachable!("shape precondition established by the caller"),
    }
}
