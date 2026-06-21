use arcweft_agent_protocol::artifact::{
    RequiredEntity, RequiredEntitySourceAnchor, RequiredEntitySourcePosition,
};
use arcweft_agent_protocol::ids::{PublicId as AgentPublicId, StableHash};
use arcweft_agent_protocol::protocol::{
    AgentProjectFlowControlSummary, AgentProjectGraph, AgentProjectGraphEdge,
    AgentProjectGraphSummary, AgentProjectGraphSymbol,
};
use arcweft_lang_sema::project_index::ProjectSemanticIndex;
use arcweft_source::{SourceAnchor, SourceName, SourcePosition};

use crate::agent_effects::entity_kind_label;

/// Builds the Agent artifact entity compatibility snapshot for a project index.
pub fn agent_required_entities_from_project(
    project: &ProjectSemanticIndex,
) -> Result<Vec<RequiredEntity>, arcweft_agent_protocol::ids::IdentifierError> {
    project
        .entities()
        .values()
        .map(required_agent_entity)
        .collect()
}

/// Builds the Agent runtime project graph snapshot for debug/readback calls.
pub fn agent_project_graph_from_project(
    project: &ProjectSemanticIndex,
) -> Result<AgentProjectGraph, arcweft_agent_protocol::ids::IdentifierError> {
    let symbols = agent_project_graph_symbols(project)?;
    let edges = agent_project_graph_edges(project);
    Ok(AgentProjectGraph { symbols, edges })
}

fn required_agent_entity(
    entity: &arcweft_lang_sema::project_index::EntitySymbol,
) -> Result<RequiredEntity, arcweft_agent_protocol::ids::IdentifierError> {
    Ok(RequiredEntity {
        public_id: AgentPublicId::new(entity.id().as_str().to_owned())?,
        kind: entity_kind_label(entity.ty().kind()).to_owned(),
        semantic_hash: StableHash::new(entity.semantic_hash().as_str().to_owned())?,
        source_anchor: required_entity_source_anchor(entity.source()),
    })
}

fn agent_project_graph_symbols(
    project: &ProjectSemanticIndex,
) -> Result<Vec<AgentProjectGraphSymbol>, arcweft_agent_protocol::ids::IdentifierError> {
    let mut symbols = vec![AgentProjectGraphSymbol {
        symbol_id: agent_project_summary_symbol_id(),
        public_id: None,
        qualified_name: Some("project".to_owned()),
        kind: "project_summary".to_owned(),
        semantic_hash: None,
        flow_control: None,
        project_summary: Some(agent_project_graph_summary(project)),
        summary: format!(
            "Project with {} entities, {} Agent actions, {} project callables, {} relations, {} dependency edges, {} dynamic-control flows, and {} debug queries",
            project.entities().len(),
            agent_project_action_count(project),
            project.project_callables().len(),
            project.relations().len(),
            project.dependency_relations().len(),
            agent_dynamic_control_flow_count(project),
            project.debug_queries().len()
        ),
    }];
    for entity in project.entities().values() {
        let control_summary = agent_flow_control_summary_text(project, entity.id().as_str());
        let flow_control = agent_project_flow_control_summary(project, entity.id().as_str());
        symbols.push(AgentProjectGraphSymbol {
            symbol_id: agent_project_entity_symbol_id(entity.id().as_str()),
            public_id: Some(AgentPublicId::new(entity.id().as_str().to_owned())?),
            qualified_name: None,
            kind: entity_kind_label(entity.ty().kind()).to_owned(),
            semantic_hash: Some(entity.semantic_hash().as_str().to_owned()),
            flow_control,
            project_summary: None,
            summary: format!(
                "{} entity `{}`{}",
                entity_kind_label(entity.ty().kind()),
                entity.id().as_str(),
                control_summary
            ),
        });
        for action in entity.agent_actions() {
            symbols.push(AgentProjectGraphSymbol {
                symbol_id: agent_project_action_symbol_id(
                    entity.id().as_str(),
                    action.action().as_str(),
                ),
                public_id: None,
                qualified_name: Some(action.action().as_str().to_owned()),
                kind: "agent_action".to_owned(),
                semantic_hash: None,
                flow_control: None,
                project_summary: None,
                summary: format!(
                    "Agent action `{}` on `{}`",
                    action.action().as_str(),
                    entity.id().as_str()
                ),
            });
        }
    }
    for (name, callable) in project.project_callables() {
        symbols.push(AgentProjectGraphSymbol {
            symbol_id: agent_project_callable_symbol_id(name.as_str()),
            public_id: None,
            qualified_name: Some(name.as_str().to_owned()),
            kind: format!("project_{}", callable.kind().as_str()),
            semantic_hash: Some(callable.semantic_hash().as_str().to_owned()),
            flow_control: None,
            project_summary: None,
            summary: format!(
                "Project {} callable `{}`",
                callable.kind().as_str(),
                name.as_str()
            ),
        });
    }
    for name in project.debug_queries().keys() {
        symbols.push(AgentProjectGraphSymbol {
            symbol_id: agent_project_debug_query_symbol_id(name.as_str()),
            public_id: None,
            qualified_name: Some(name.as_str().to_owned()),
            kind: "debug_query".to_owned(),
            semantic_hash: None,
            flow_control: None,
            project_summary: None,
            summary: format!("Debug query `{}`", name.as_str()),
        });
    }
    Ok(symbols)
}

fn agent_project_graph_edges(project: &ProjectSemanticIndex) -> Vec<AgentProjectGraphEdge> {
    let mut edges = project
        .entities()
        .values()
        .map(|entity| AgentProjectGraphEdge {
            from_symbol_id: agent_project_summary_symbol_id(),
            to_symbol_id: agent_project_entity_symbol_id(entity.id().as_str()),
            edge_kind: "contains_entity".to_owned(),
        })
        .collect::<Vec<_>>();
    for entity in project.entities().values() {
        edges.extend(
            entity
                .agent_actions()
                .iter()
                .map(|action| AgentProjectGraphEdge {
                    from_symbol_id: agent_project_entity_symbol_id(entity.id().as_str()),
                    to_symbol_id: agent_project_action_symbol_id(
                        entity.id().as_str(),
                        action.action().as_str(),
                    ),
                    edge_kind: "exposes_agent_action".to_owned(),
                }),
        );
    }
    edges.extend(
        project
            .project_callables()
            .keys()
            .map(|name| AgentProjectGraphEdge {
                from_symbol_id: agent_project_summary_symbol_id(),
                to_symbol_id: agent_project_callable_symbol_id(name.as_str()),
                edge_kind: "contains_callable".to_owned(),
            }),
    );
    edges.extend(
        project
            .debug_queries()
            .keys()
            .map(|name| AgentProjectGraphEdge {
                from_symbol_id: agent_project_summary_symbol_id(),
                to_symbol_id: agent_project_debug_query_symbol_id(name.as_str()),
                edge_kind: "contains_debug_query".to_owned(),
            }),
    );
    edges.extend(
        project
            .relations()
            .iter()
            .map(|relation| AgentProjectGraphEdge {
                from_symbol_id: agent_project_entity_symbol_id(relation.from().as_str()),
                to_symbol_id: agent_project_entity_symbol_id(relation.to().as_str()),
                edge_kind: relation.edge_kind().as_str().to_owned(),
            }),
    );
    edges.extend(
        project
            .dependency_relations()
            .iter()
            .map(|relation| AgentProjectGraphEdge {
                from_symbol_id: agent_project_symbol_ref_id(relation.from()),
                to_symbol_id: agent_project_symbol_ref_id(relation.to()),
                edge_kind: relation.edge_kind().as_str().to_owned(),
            }),
    );
    edges
}

fn agent_project_summary_symbol_id() -> String {
    "project:summary".to_owned()
}

fn agent_project_entity_symbol_id(id: &str) -> String {
    format!("project:entity:{id}")
}

fn agent_project_action_symbol_id(entity_id: &str, action: &str) -> String {
    format!("project:action:{entity_id}:{action}")
}

fn agent_project_callable_symbol_id(name: &str) -> String {
    format!("project:callable:{name}")
}

fn agent_project_debug_query_symbol_id(name: &str) -> String {
    format!("project:debug_query:{name}")
}

fn agent_project_symbol_ref_id(
    symbol_ref: &arcweft_lang_sema::project_index::ProjectGraphSymbolRef,
) -> String {
    match symbol_ref {
        arcweft_lang_sema::project_index::ProjectGraphSymbolRef::Entity(id) => {
            agent_project_entity_symbol_id(id.as_str())
        }
        arcweft_lang_sema::project_index::ProjectGraphSymbolRef::Callable(name) => {
            agent_project_callable_symbol_id(name.as_str())
        }
    }
}

fn agent_dynamic_control_flow_count(project: &ProjectSemanticIndex) -> usize {
    project
        .flow_control_summaries()
        .values()
        .filter(|summary| summary.has_dynamic_control())
        .count()
}

fn agent_project_action_count(project: &ProjectSemanticIndex) -> usize {
    project
        .entities()
        .values()
        .map(|entity| entity.agent_actions().len())
        .sum()
}

fn agent_project_graph_summary(project: &ProjectSemanticIndex) -> AgentProjectGraphSummary {
    AgentProjectGraphSummary {
        entity_count: usize_to_u32_saturating(project.entities().len()),
        agent_action_count: usize_to_u32_saturating(agent_project_action_count(project)),
        project_callable_count: usize_to_u32_saturating(project.project_callables().len()),
        relation_count: usize_to_u32_saturating(project.relations().len()),
        dependency_edge_count: usize_to_u32_saturating(project.dependency_relations().len()),
        dynamic_control_flow_count: usize_to_u32_saturating(agent_dynamic_control_flow_count(
            project,
        )),
        debug_query_count: usize_to_u32_saturating(project.debug_queries().len()),
    }
}

fn agent_project_flow_control_summary(
    project: &ProjectSemanticIndex,
    flow_id: &str,
) -> Option<AgentProjectFlowControlSummary> {
    let summary = project
        .flow_control_summaries()
        .iter()
        .find_map(|(id, summary)| (id.as_str() == flow_id).then_some(summary))?;
    Some(AgentProjectFlowControlSummary {
        has_dynamic_control: summary.has_dynamic_control(),
        static_goto_count: usize_to_u32_saturating(summary.static_goto_count()),
        dynamic_goto_count: usize_to_u32_saturating(summary.dynamic_goto_count()),
        branch_count: usize_to_u32_saturating(summary.branch_count()),
        loop_count: usize_to_u32_saturating(summary.loop_count()),
        await_count: usize_to_u32_saturating(summary.await_count()),
        thread_count: usize_to_u32_saturating(summary.thread_count()),
        select_branch_count: usize_to_u32_saturating(summary.select_branch_count()),
    })
}

fn agent_flow_control_summary_text(project: &ProjectSemanticIndex, flow_id: &str) -> String {
    let Some(summary) = project
        .flow_control_summaries()
        .iter()
        .find_map(|(id, summary)| (id.as_str() == flow_id).then_some(summary))
    else {
        return String::new();
    };
    if !summary.has_dynamic_control() && summary.static_goto_count() == 0 {
        return String::new();
    }
    format!(
        " control(static_goto={}, dynamic_goto={}, branches={}, loops={}, awaits={}, threads={}, select_branches={})",
        summary.static_goto_count(),
        summary.dynamic_goto_count(),
        summary.branch_count(),
        summary.loop_count(),
        summary.await_count(),
        summary.thread_count(),
        summary.select_branch_count()
    )
}

fn usize_to_u32_saturating(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn required_entity_source_anchor(source: &SourceAnchor) -> Option<RequiredEntitySourceAnchor> {
    let SourceName::Path(path) = source.source() else {
        return None;
    };
    let range = source.byte_range();
    Some(RequiredEntitySourceAnchor {
        path: path.clone(),
        start_byte: u64::try_from(range.start).ok()?,
        end_byte: u64::try_from(range.end).ok()?,
        start: source.start().map(required_entity_source_position),
        end: source.end().map(required_entity_source_position),
    })
}

fn required_entity_source_position(position: SourcePosition) -> RequiredEntitySourcePosition {
    RequiredEntitySourcePosition {
        line: position.line,
        column: position.column,
    }
}
