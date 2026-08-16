use arcweft_core::{
    plan::RuntimeAgentOperationalType,
    value::{RuntimeAgentField, RuntimeAgentFieldResult},
};

use super::{AgentBuiltinType, MapKind, TypeKind};

impl TypeKind {
    /// Resolves one accepted Agent protocol field into its closed coordinate
    /// and exact semantic result. Downstream stages retain the coordinate.
    pub(crate) fn agent_field_type(&self, field: &str) -> Option<(RuntimeAgentField, Self)> {
        let coordinate = agent_field_coordinate(self, field)?;
        Some((coordinate, semantic_field_result(coordinate.result())))
    }
}

fn agent_field_coordinate(owner: &TypeKind, field: &str) -> Option<RuntimeAgentField> {
    match owner {
        TypeKind::Observation => observation_field(field),
        TypeKind::ObservedObject => observed_object_field(field),
        TypeKind::AgentBBox => bounding_box_field(field),
        TypeKind::ActionTarget => action_target_field(field),
        TypeKind::ActionResult => action_result_field(field),
        TypeKind::CaptureRef => capture_reference_field(field),
        TypeKind::AgentResource => resource_field(field),
        TypeKind::AgentResourceBody => resource_body_field(field),
        TypeKind::AgentEntityMetadata => entity_metadata_field(field),
        TypeKind::AgentSourceAnchor => source_anchor_field(field),
        TypeKind::AgentProjectGraphNeighborhood => project_graph_neighborhood_field(field),
        TypeKind::AgentProjectGraphSymbol => project_graph_symbol_field(field),
        TypeKind::AgentProjectGraphEdge => project_graph_edge_field(field),
        TypeKind::Ref(_) => reference_field(field),
        _ => None,
    }
}

fn observation_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "tick" => RuntimeAgentField::ObservationTick,
        "frame_id" => RuntimeAgentField::ObservationFrameId,
        "state_hash" => RuntimeAgentField::ObservationStateHash,
        "render_hash" => RuntimeAgentField::ObservationRenderHash,
        "actions" => RuntimeAgentField::ObservationActions,
        "objects" => RuntimeAgentField::ObservationObjects,
        "signals" => RuntimeAgentField::ObservationSignals,
        _ => return None,
    })
}

fn observed_object_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "id" => RuntimeAgentField::ObservedObjectId,
        "parent_id" => RuntimeAgentField::ObservedObjectParentId,
        "entity" => RuntimeAgentField::ObservedObjectEntity,
        "layer" => RuntimeAgentField::ObservedObjectLayer,
        "role" => RuntimeAgentField::ObservedObjectRole,
        "text" => RuntimeAgentField::ObservedObjectText,
        "visible" => RuntimeAgentField::ObservedObjectVisible,
        "enabled" => RuntimeAgentField::ObservedObjectEnabled,
        "bbox" => RuntimeAgentField::ObservedObjectBoundingBox,
        _ => return None,
    })
}

fn bounding_box_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "space" => RuntimeAgentField::BoundingBoxSpace,
        "x" => RuntimeAgentField::BoundingBoxX,
        "y" => RuntimeAgentField::BoundingBoxY,
        "width" => RuntimeAgentField::BoundingBoxWidth,
        "height" => RuntimeAgentField::BoundingBoxHeight,
        _ => return None,
    })
}

fn action_target_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "id" => RuntimeAgentField::ActionId,
        "target" => RuntimeAgentField::ActionTarget,
        "action" => RuntimeAgentField::ActionName,
        "kind" => RuntimeAgentField::ActionKind,
        "enabled" => RuntimeAgentField::ActionEnabled,
        _ => return None,
    })
}

fn action_result_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "accepted" => RuntimeAgentField::ActionResultAccepted,
        "before_tick" => RuntimeAgentField::ActionResultBeforeTick,
        "after_tick" => RuntimeAgentField::ActionResultAfterTick,
        "before_state_hash" => RuntimeAgentField::ActionResultBeforeStateHash,
        "after_state_hash" => RuntimeAgentField::ActionResultAfterStateHash,
        _ => return None,
    })
}

fn capture_reference_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "uri" => RuntimeAgentField::CaptureReferenceUri,
        "content_hash" => RuntimeAgentField::CaptureReferenceContentHash,
        "media_type" => RuntimeAgentField::CaptureReferenceMediaType,
        "byte_len" => RuntimeAgentField::CaptureReferenceByteLen,
        _ => return None,
    })
}

fn resource_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "uri" => RuntimeAgentField::ResourceUri,
        "kind" => RuntimeAgentField::ResourceKind,
        "mime_type" => RuntimeAgentField::ResourceMimeType,
        "hash" => RuntimeAgentField::ResourceHash,
        "body" => RuntimeAgentField::ResourceBody,
        _ => return None,
    })
}

fn resource_body_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "kind" => RuntimeAgentField::ResourceBodyKind,
        "json" => RuntimeAgentField::ResourceBodyJson,
        "text" => RuntimeAgentField::ResourceBodyText,
        "base64" => RuntimeAgentField::ResourceBodyBase64,
        "encoding" => RuntimeAgentField::ResourceBodyEncoding,
        "value" => RuntimeAgentField::ResourceBodyValue,
        _ => return None,
    })
}

fn entity_metadata_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "id" => RuntimeAgentField::EntityMetadataId,
        "kind" => RuntimeAgentField::EntityMetadataKind,
        "semantic_hash" => RuntimeAgentField::EntityMetadataSemanticHash,
        "source" => RuntimeAgentField::EntityMetadataSource,
        _ => return None,
    })
}

fn source_anchor_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "has_source" => RuntimeAgentField::SourceAnchorHasSource,
        "path" => RuntimeAgentField::SourceAnchorPath,
        "start_byte" => RuntimeAgentField::SourceAnchorStartByte,
        "end_byte" => RuntimeAgentField::SourceAnchorEndByte,
        "start_line" => RuntimeAgentField::SourceAnchorStartLine,
        "start_column" => RuntimeAgentField::SourceAnchorStartColumn,
        "end_line" => RuntimeAgentField::SourceAnchorEndLine,
        "end_column" => RuntimeAgentField::SourceAnchorEndColumn,
        _ => return None,
    })
}

fn project_graph_neighborhood_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "root" => RuntimeAgentField::ProjectGraphNeighborhoodRoot,
        "node_count" => RuntimeAgentField::ProjectGraphNeighborhoodNodeCount,
        "edge_count" => RuntimeAgentField::ProjectGraphNeighborhoodEdgeCount,
        "symbols" => RuntimeAgentField::ProjectGraphNeighborhoodSymbols,
        "edges" => RuntimeAgentField::ProjectGraphNeighborhoodEdges,
        _ => return None,
    })
}

fn project_graph_edge_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "from_symbol_id" => RuntimeAgentField::ProjectGraphEdgeFromSymbolId,
        "to_symbol_id" => RuntimeAgentField::ProjectGraphEdgeToSymbolId,
        "kind" => RuntimeAgentField::ProjectGraphEdgeKind,
        _ => return None,
    })
}

fn reference_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "id" => RuntimeAgentField::ReferenceId,
        "family" => RuntimeAgentField::ReferenceFamily,
        "name" => RuntimeAgentField::ReferenceName,
        _ => return None,
    })
}

fn project_graph_symbol_field(field: &str) -> Option<RuntimeAgentField> {
    Some(match field {
        "symbol_id" => RuntimeAgentField::ProjectGraphSymbolSymbolId,
        "id" => RuntimeAgentField::ProjectGraphSymbolId,
        "kind" => RuntimeAgentField::ProjectGraphSymbolKind,
        "semantic_hash" => RuntimeAgentField::ProjectGraphSymbolSemanticHash,
        "summary" => RuntimeAgentField::ProjectGraphSymbolSummary,
        "has_entity" => RuntimeAgentField::ProjectGraphSymbolHasEntity,
        "has_semantic_hash" => RuntimeAgentField::ProjectGraphSymbolHasSemanticHash,
        "has_flow_control" => RuntimeAgentField::ProjectGraphSymbolHasFlowControl,
        "has_dynamic_control" => RuntimeAgentField::ProjectGraphSymbolHasDynamicControl,
        "has_project_summary" => RuntimeAgentField::ProjectGraphSymbolHasProjectSummary,
        "entity_count" => RuntimeAgentField::ProjectGraphSymbolEntityCount,
        "agent_action_count" => RuntimeAgentField::ProjectGraphSymbolAgentActionCount,
        "project_callable_count" => RuntimeAgentField::ProjectGraphSymbolProjectCallableCount,
        "relation_count" => RuntimeAgentField::ProjectGraphSymbolRelationCount,
        "dependency_edge_count" => RuntimeAgentField::ProjectGraphSymbolDependencyEdgeCount,
        "dynamic_control_flow_count" => {
            RuntimeAgentField::ProjectGraphSymbolDynamicControlFlowCount
        }
        "debug_query_count" => RuntimeAgentField::ProjectGraphSymbolDebugQueryCount,
        "static_goto_count" => RuntimeAgentField::ProjectGraphSymbolStaticGotoCount,
        "dynamic_goto_count" => RuntimeAgentField::ProjectGraphSymbolDynamicGotoCount,
        "branch_count" => RuntimeAgentField::ProjectGraphSymbolBranchCount,
        "loop_count" => RuntimeAgentField::ProjectGraphSymbolLoopCount,
        "await_count" => RuntimeAgentField::ProjectGraphSymbolAwaitCount,
        "thread_count" => RuntimeAgentField::ProjectGraphSymbolThreadCount,
        "select_branch_count" => RuntimeAgentField::ProjectGraphSymbolSelectBranchCount,
        _ => return None,
    })
}

fn semantic_field_result(result: RuntimeAgentFieldResult) -> TypeKind {
    match result {
        RuntimeAgentFieldResult::Bool => TypeKind::Bool,
        RuntimeAgentFieldResult::String => TypeKind::String,
        RuntimeAgentFieldResult::U32 => TypeKind::U32,
        RuntimeAgentFieldResult::U64 => TypeKind::U64,
        RuntimeAgentFieldResult::Agent(operational) => agent_type(operational),
        RuntimeAgentFieldResult::VecAgent(operational) => {
            TypeKind::Vec(Box::new(agent_type(operational)))
        }
        RuntimeAgentFieldResult::AgentValueMap => TypeKind::Map {
            kind: MapKind::BTree,
            key: Box::new(TypeKind::AgentValue),
            value: Box::new(TypeKind::AgentValue),
        },
    }
}

fn agent_type(operational: RuntimeAgentOperationalType) -> TypeKind {
    match operational {
        RuntimeAgentOperationalType::ObservedObjectId => {
            TypeKind::AgentBuiltin(AgentBuiltinType::ObservedObjectId)
        }
        RuntimeAgentOperationalType::BoundingBox => TypeKind::AgentBBox,
        RuntimeAgentOperationalType::ActionName => TypeKind::ActionName,
        RuntimeAgentOperationalType::ResourceBody => TypeKind::AgentResourceBody,
        RuntimeAgentOperationalType::SourceAnchor => TypeKind::AgentSourceAnchor,
        RuntimeAgentOperationalType::AgentValue => TypeKind::AgentValue,
        RuntimeAgentOperationalType::ActionTarget => TypeKind::ActionTarget,
        RuntimeAgentOperationalType::ObservedObject => TypeKind::ObservedObject,
        RuntimeAgentOperationalType::ProjectGraphSymbol => TypeKind::AgentProjectGraphSymbol,
        RuntimeAgentOperationalType::ProjectGraphEdge => TypeKind::AgentProjectGraphEdge,
        _ => unreachable!("Agent field result only declares protocol record result types"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_runtime_records_publish_exact_coordinates_and_types() {
        assert_eq!(
            TypeKind::ActionResult.agent_field_type("accepted"),
            Some((RuntimeAgentField::ActionResultAccepted, TypeKind::Bool))
        );
        assert_eq!(
            TypeKind::AgentResource.agent_field_type("body"),
            Some((RuntimeAgentField::ResourceBody, TypeKind::AgentResourceBody))
        );
        assert_eq!(TypeKind::ActionResult.agent_field_type("missing"), None);
    }
}
