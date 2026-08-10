use super::{MapKind, TypeKind};

impl TypeKind {
    /// Returns the exact typed field published by an Agent runtime record.
    ///
    /// These shapes are semantic protocol authority shared by final analysis
    /// and runtime payload projection. Unknown fields remain rejected; this
    /// boundary never falls back to spelling-based record inference.
    pub(crate) fn agent_field_type(&self, field: &str) -> Option<Self> {
        Some(match self {
            Self::Observation => match field {
                "tick" => Self::U64,
                "frame_id" | "state_hash" | "render_hash" => Self::String,
                "actions" => Self::Vec(Box::new(Self::ActionTarget)),
                "objects" => Self::Vec(Box::new(Self::ObservedObject)),
                "signals" => Self::Map {
                    kind: MapKind::BTree,
                    key: Box::new(Self::AgentValue),
                    value: Box::new(Self::AgentValue),
                },
                _ => return None,
            },
            Self::ObservedObject => match field {
                "id" => Self::Named("ObservedObjectId".to_owned()),
                "parent_id" | "entity" | "layer" | "role" | "text" => Self::String,
                "visible" | "enabled" => Self::Bool,
                "bbox" => Self::AgentBBox,
                _ => return None,
            },
            Self::AgentBBox => match field {
                "space" => Self::String,
                "x" | "y" | "width" | "height" => Self::U32,
                _ => return None,
            },
            Self::ActionTarget => match field {
                "id" | "target" | "kind" => Self::String,
                "action" => Self::ActionName,
                "enabled" => Self::Bool,
                _ => return None,
            },
            Self::ActionResult => match field {
                "accepted" => Self::Bool,
                "before_tick" | "after_tick" => Self::U64,
                "before_state_hash" | "after_state_hash" => Self::String,
                _ => return None,
            },
            Self::CaptureRef => match field {
                "uri" | "content_hash" | "media_type" => Self::String,
                "byte_len" => Self::U64,
                _ => return None,
            },
            Self::AgentResource => match field {
                "uri" | "kind" | "mime_type" | "hash" => Self::String,
                "body" => Self::AgentResourceBody,
                _ => return None,
            },
            Self::AgentResourceBody => match field {
                "kind" | "json" | "text" | "base64" | "encoding" => Self::String,
                "value" => Self::AgentValue,
                _ => return None,
            },
            Self::AgentEntityMetadata => match field {
                "id" | "kind" | "semantic_hash" => Self::String,
                "source" => Self::AgentSourceAnchor,
                _ => return None,
            },
            Self::AgentSourceAnchor => match field {
                "has_source" => Self::Bool,
                "path" => Self::String,
                "start_byte" | "end_byte" => Self::U64,
                "start_line" | "start_column" | "end_line" | "end_column" => Self::U32,
                _ => return None,
            },
            Self::AgentProjectGraphNeighborhood
            | Self::AgentProjectGraphSymbol
            | Self::AgentProjectGraphEdge => return agent_project_graph_field(self, field),
            Self::Ref(_) => match field {
                "id" | "family" | "name" => Self::String,
                _ => return None,
            },
            _ => return None,
        })
    }
}

fn agent_project_graph_field(owner: &TypeKind, field: &str) -> Option<TypeKind> {
    Some(match owner {
        TypeKind::AgentProjectGraphNeighborhood => match field {
            "root" => TypeKind::String,
            "node_count" | "edge_count" => TypeKind::U32,
            "symbols" => TypeKind::Vec(Box::new(TypeKind::AgentProjectGraphSymbol)),
            "edges" => TypeKind::Vec(Box::new(TypeKind::AgentProjectGraphEdge)),
            _ => return None,
        },
        TypeKind::AgentProjectGraphSymbol => match field {
            "symbol_id" | "id" | "kind" | "semantic_hash" | "summary" => TypeKind::String,
            "has_entity"
            | "has_semantic_hash"
            | "has_flow_control"
            | "has_dynamic_control"
            | "has_project_summary" => TypeKind::Bool,
            "entity_count"
            | "agent_action_count"
            | "project_callable_count"
            | "relation_count"
            | "dependency_edge_count"
            | "dynamic_control_flow_count"
            | "debug_query_count"
            | "static_goto_count"
            | "dynamic_goto_count"
            | "branch_count"
            | "loop_count"
            | "await_count"
            | "thread_count"
            | "select_branch_count" => TypeKind::U32,
            _ => return None,
        },
        TypeKind::AgentProjectGraphEdge => match field {
            "from_symbol_id" | "to_symbol_id" | "kind" => TypeKind::String,
            _ => return None,
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_runtime_records_publish_exact_fields() {
        assert_eq!(
            TypeKind::ActionResult.agent_field_type("accepted"),
            Some(TypeKind::Bool)
        );
        assert_eq!(
            TypeKind::AgentResource.agent_field_type("body"),
            Some(TypeKind::AgentResourceBody)
        );
        assert_eq!(TypeKind::ActionResult.agent_field_type("missing"), None);
    }
}
