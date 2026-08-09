use arcweft_agent_protocol::artifact::{RequiredEntity, RequiredEntitySourceAnchor};
use arcweft_agent_protocol::ids::{
    AgentProjectGraphSymbolId, PublicId as AgentPublicId, StableHash,
};
use arcweft_agent_protocol::protocol::{
    AgentProjectFlowControlSummary, AgentProjectGraph, AgentProjectGraphEdge,
    AgentProjectGraphSummary, AgentProjectGraphSymbol,
};
use arcweft_lang_hir::symbol::CallableDeclarationKey;
use arcweft_lang_sema::callable::{
    CallableCandidateId, CallableLookupKey, CheckedCallableDeclaration, CheckedCallableFacts,
    CheckedCallableId, CheckedCallableLookupError, EnvironmentCallableId,
};
use arcweft_lang_sema::project_index::{
    EntitySymbol, EnvironmentCallableLowering, ProjectCallableSymbol, ProjectEntityId,
    ProjectGraphSymbolRef, ProjectSemanticIndex,
};
use arcweft_source::SourceAnchor;
use thiserror::Error;

/// Failure while converting exact semantic graph references into durable
/// Agent protocol identities.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AgentProjectGraphError {
    #[error(transparent)]
    Identifier(Box<arcweft_agent_protocol::ids::IdentifierError>),
    #[error("checked callable {checked:?} lookup failed: {reason:?}")]
    CheckedCallableLookup {
        checked: Box<CheckedCallableId>,
        reason: Box<CheckedCallableLookupError>,
    },
    #[error("project callable {declaration:?} is absent from the semantic index")]
    MissingProjectCallable {
        declaration: Box<CallableDeclarationKey>,
    },
    #[error("project callable {declaration:?} does not match checked callable {checked:?}")]
    InvalidProjectCallable {
        declaration: Box<CallableDeclarationKey>,
        checked: Box<CheckedCallableId>,
    },
    #[error("environment callable {declaration:?} is absent from the semantic index")]
    MissingEnvironmentCallable {
        declaration: Box<EnvironmentCallableId>,
    },
    #[error("environment callable {declaration:?} does not match checked callable {checked:?}")]
    InvalidEnvironmentCallable {
        declaration: Box<EnvironmentCallableId>,
        checked: Box<CheckedCallableId>,
    },
    #[error("checked callable {checked:?} has no durable Agent project-graph identity")]
    UnsupportedCallable { checked: Box<CheckedCallableId> },
}

impl From<arcweft_agent_protocol::ids::IdentifierError> for AgentProjectGraphError {
    fn from(error: arcweft_agent_protocol::ids::IdentifierError) -> Self {
        Self::Identifier(Box::new(error))
    }
}

/// Builds the Agent artifact entity compatibility snapshot for a project index.
pub fn agent_required_entities_from_project(
    project: &ProjectSemanticIndex,
) -> Result<Vec<RequiredEntity>, arcweft_agent_protocol::ids::IdentifierError> {
    agent_required_entities_from_symbols(project.entities().values())
}

/// Projects an explicit accepted entity inventory without manufacturing a
/// semantic index or checked callable catalog. The compatibility schema can
/// encode only project-global public identities, so structural Flow entities
/// remain available through the typed project graph instead.
pub fn agent_required_entities_from_symbols<'a>(
    symbols: impl IntoIterator<Item = &'a EntitySymbol>,
) -> Result<Vec<RequiredEntity>, arcweft_agent_protocol::ids::IdentifierError> {
    symbols
        .into_iter()
        .filter_map(required_agent_entity)
        .collect()
}

/// Builds the Agent runtime project graph snapshot for debug/readback calls.
pub fn agent_project_graph_from_project(
    project: &ProjectSemanticIndex,
) -> Result<AgentProjectGraph, AgentProjectGraphError> {
    let symbols = agent_project_graph_symbols(project)?;
    let edges = agent_project_graph_edges(project)?;
    Ok(AgentProjectGraph { symbols, edges })
}

fn required_agent_entity(
    entity: &EntitySymbol,
) -> Option<Result<RequiredEntity, arcweft_agent_protocol::ids::IdentifierError>> {
    let ProjectEntityId::Public(public_id) = entity.identity() else {
        return None;
    };
    Some(
        AgentPublicId::new(public_id.as_str().to_owned()).and_then(|public_id| {
            StableHash::new(entity.semantic_hash().as_str().to_owned()).map(|semantic_hash| {
                RequiredEntity {
                    public_id,
                    kind: entity.ty().kind().as_str().to_owned(),
                    semantic_hash,
                    source_anchor: required_entity_source_anchor(entity.source()),
                }
            })
        }),
    )
}

fn agent_project_graph_symbols(
    project: &ProjectSemanticIndex,
) -> Result<Vec<AgentProjectGraphSymbol>, AgentProjectGraphError> {
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
    for (identity, entity) in project.entities() {
        let control_summary = agent_flow_control_summary_text(project, identity);
        let flow_control = agent_project_flow_control_summary(project, identity);
        let public_id = entity.public_id().as_str();
        symbols.push(AgentProjectGraphSymbol {
            symbol_id: agent_project_entity_symbol_id(identity),
            public_id: Some(AgentPublicId::new(public_id.to_owned())?),
            qualified_name: agent_project_entity_qualified_name(identity),
            kind: entity.ty().kind().as_str().to_owned(),
            semantic_hash: Some(entity.semantic_hash().as_str().to_owned()),
            flow_control,
            project_summary: None,
            summary: format!(
                "{} entity `{}`{}",
                entity.ty().kind().as_str(),
                public_id,
                control_summary
            ),
        });
        for action in entity.agent_actions() {
            symbols.push(AgentProjectGraphSymbol {
                symbol_id: agent_project_action_symbol_id(identity, action.action().as_str()),
                public_id: None,
                qualified_name: Some(action.action().as_str().to_owned()),
                kind: "agent_action".to_owned(),
                semantic_hash: None,
                flow_control: None,
                project_summary: None,
                summary: format!(
                    "Agent action `{}` on `{}`",
                    action.action().as_str(),
                    public_id
                ),
            });
        }
    }
    for (declaration, callable) in project.project_callables() {
        let facts = validated_project_callable(project, declaration, callable)?;
        let name = declaration.qualified_name();
        symbols.push(AgentProjectGraphSymbol {
            symbol_id: agent_project_callable_symbol_id(declaration),
            public_id: None,
            qualified_name: Some(name.clone()),
            kind: format!("project_{}", callable.kind().as_str()),
            semantic_hash: Some(digest_hex(facts.interface_digest().as_bytes())),
            flow_control: None,
            project_summary: None,
            summary: format!("Project {} callable `{}`", callable.kind().as_str(), name),
        });
    }
    for (declaration, lowering) in project.environment_lowerings() {
        let facts = validated_environment_callable(project, declaration, lowering)?;
        let name = environment_callable_display_name(declaration.key());
        symbols.push(AgentProjectGraphSymbol {
            symbol_id: agent_environment_callable_symbol_id(declaration),
            public_id: None,
            qualified_name: Some(name.clone()),
            kind: format!("environment_{}", environment_callable_kind(declaration)),
            semantic_hash: Some(digest_hex(facts.interface_digest().as_bytes())),
            flow_control: None,
            project_summary: None,
            summary: format!("Environment callable `{name}`"),
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

fn agent_project_graph_edges(
    project: &ProjectSemanticIndex,
) -> Result<Vec<AgentProjectGraphEdge>, AgentProjectGraphError> {
    let mut edges = project
        .entities()
        .keys()
        .map(|identity| AgentProjectGraphEdge {
            from_symbol_id: agent_project_summary_symbol_id(),
            to_symbol_id: agent_project_entity_symbol_id(identity),
            edge_kind: "contains_entity".to_owned(),
        })
        .collect::<Vec<_>>();
    for (identity, entity) in project.entities() {
        edges.extend(
            entity
                .agent_actions()
                .iter()
                .map(|action| AgentProjectGraphEdge {
                    from_symbol_id: agent_project_entity_symbol_id(identity),
                    to_symbol_id: agent_project_action_symbol_id(
                        identity,
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
            .map(|declaration| AgentProjectGraphEdge {
                from_symbol_id: agent_project_summary_symbol_id(),
                to_symbol_id: agent_project_callable_symbol_id(declaration),
                edge_kind: "contains_callable".to_owned(),
            }),
    );
    edges.extend(
        project
            .environment_lowerings()
            .keys()
            .map(|declaration| AgentProjectGraphEdge {
                from_symbol_id: agent_project_summary_symbol_id(),
                to_symbol_id: agent_environment_callable_symbol_id(declaration),
                edge_kind: "contains_environment_callable".to_owned(),
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
                from_symbol_id: agent_project_entity_symbol_id(relation.from()),
                to_symbol_id: agent_project_entity_symbol_id(relation.to()),
                edge_kind: relation.edge_kind().as_str().to_owned(),
            }),
    );
    for relation in project.dependency_relations() {
        edges.push(AgentProjectGraphEdge {
            from_symbol_id: agent_project_symbol_ref_id(project, relation.from())?,
            to_symbol_id: agent_project_symbol_ref_id(project, relation.to())?,
            edge_kind: relation.edge_kind().as_str().to_owned(),
        });
    }
    Ok(edges)
}

fn agent_project_summary_symbol_id() -> AgentProjectGraphSymbolId {
    AgentProjectGraphSymbolId::new("project:summary")
        .expect("canonical project summary symbol ID is non-empty")
}

fn agent_project_entity_symbol_id(id: &ProjectEntityId) -> AgentProjectGraphSymbolId {
    AgentProjectGraphSymbolId::new(match id {
        ProjectEntityId::Public(public_id) => {
            format!("project:entity:public:{}", public_id.as_str())
        }
        ProjectEntityId::StructuralFlow(declaration) => format!(
            "project:entity:flow:v1:{}",
            digest_hex(declaration.semantic_digest().as_bytes())
        ),
    })
    .expect("canonical project entity symbol ID is non-empty")
}

fn agent_project_entity_qualified_name(id: &ProjectEntityId) -> Option<String> {
    match id {
        ProjectEntityId::Public(_) => None,
        ProjectEntityId::StructuralFlow(declaration) => {
            Some(CallableDeclarationKey::Flow(declaration.clone()).qualified_name())
        }
    }
}

fn agent_project_action_symbol_id(
    entity_id: &ProjectEntityId,
    action: &str,
) -> AgentProjectGraphSymbolId {
    AgentProjectGraphSymbolId::new(format!(
        "{}:action:{action}",
        agent_project_entity_symbol_id(entity_id)
    ))
    .expect("canonical project action symbol ID is non-empty")
}

fn agent_project_callable_symbol_id(
    declaration: &CallableDeclarationKey,
) -> AgentProjectGraphSymbolId {
    AgentProjectGraphSymbolId::new(format!(
        "project:callable:v1:{}",
        digest_hex(declaration.semantic_digest().as_bytes())
    ))
    .expect("canonical project callable symbol ID is non-empty")
}

fn agent_environment_callable_symbol_id(
    declaration: &EnvironmentCallableId,
) -> AgentProjectGraphSymbolId {
    AgentProjectGraphSymbolId::new(format!(
        "project:environment-callable:v1:{}",
        digest_hex(declaration.semantic_digest().as_bytes())
    ))
    .expect("canonical environment callable symbol ID is non-empty")
}

fn agent_project_debug_query_symbol_id(name: &str) -> AgentProjectGraphSymbolId {
    AgentProjectGraphSymbolId::new(format!("project:debug_query:{name}"))
        .expect("canonical debug-query symbol ID is non-empty")
}

fn agent_project_symbol_ref_id(
    project: &ProjectSemanticIndex,
    symbol_ref: &ProjectGraphSymbolRef,
) -> Result<AgentProjectGraphSymbolId, AgentProjectGraphError> {
    match symbol_ref {
        ProjectGraphSymbolRef::Entity(id) => Ok(agent_project_entity_symbol_id(id)),
        ProjectGraphSymbolRef::Callable(checked) => {
            let facts = checked_callable(project, checked)?;
            match checked.declaration() {
                CheckedCallableDeclaration::Project(declaration) => {
                    let symbol = project
                        .project_callable_by_declaration(declaration)
                        .ok_or_else(|| AgentProjectGraphError::MissingProjectCallable {
                            declaration: Box::new(declaration.clone()),
                        })?;
                    validated_project_callable(project, declaration, symbol)?;
                    Ok(agent_project_callable_symbol_id(declaration))
                }
                CheckedCallableDeclaration::Environment(declaration) => {
                    let lowering = project.environment_lowering(declaration).ok_or_else(|| {
                        AgentProjectGraphError::MissingEnvironmentCallable {
                            declaration: Box::new(declaration.clone()),
                        }
                    })?;
                    validated_environment_callable(project, declaration, lowering)?;
                    Ok(agent_environment_callable_symbol_id(declaration))
                }
                CheckedCallableDeclaration::Detached(_)
                | CheckedCallableDeclaration::Standard(_) => {
                    Err(AgentProjectGraphError::UnsupportedCallable {
                        checked: Box::new(facts.id().clone()),
                    })
                }
            }
        }
    }
}

fn checked_callable<'a>(
    project: &'a ProjectSemanticIndex,
    checked: &CheckedCallableId,
) -> Result<&'a CheckedCallableFacts, AgentProjectGraphError> {
    project.checked_callable(checked).map_err(|reason| {
        AgentProjectGraphError::CheckedCallableLookup {
            checked: Box::new(checked.clone()),
            reason: Box::new(reason),
        }
    })
}

fn validated_project_callable<'a>(
    project: &'a ProjectSemanticIndex,
    declaration: &CallableDeclarationKey,
    symbol: &ProjectCallableSymbol,
) -> Result<&'a CheckedCallableFacts, AgentProjectGraphError> {
    let facts = checked_callable(project, symbol.checked())?;
    if symbol.declaration() != declaration
        || facts.id() != symbol.checked()
        || facts.interface_digest() != symbol.interface_digest()
        || !matches!(
            facts.record().id(),
            CallableCandidateId::Project(candidate) if candidate == declaration
        )
    {
        return Err(AgentProjectGraphError::InvalidProjectCallable {
            declaration: Box::new(declaration.clone()),
            checked: Box::new(symbol.checked().clone()),
        });
    }
    Ok(facts)
}

fn validated_environment_callable<'a>(
    project: &'a ProjectSemanticIndex,
    declaration: &EnvironmentCallableId,
    lowering: &EnvironmentCallableLowering,
) -> Result<&'a CheckedCallableFacts, AgentProjectGraphError> {
    let facts = checked_callable(project, lowering.checked())?;
    if facts.id() != lowering.checked()
        || !matches!(
            facts.record().id(),
            CallableCandidateId::Environment(candidate) if candidate == declaration
        )
    {
        return Err(AgentProjectGraphError::InvalidEnvironmentCallable {
            declaration: Box::new(declaration.clone()),
            checked: Box::new(lowering.checked().clone()),
        });
    }
    Ok(facts)
}

fn environment_callable_display_name(key: &CallableLookupKey) -> String {
    match key {
        CallableLookupKey::Free(path) => path.dotted_name(),
        CallableLookupKey::Method(method) => method.method().as_str().to_owned(),
    }
}

fn environment_callable_kind(declaration: &EnvironmentCallableId) -> &'static str {
    use arcweft_lang_sema::callable::EnvironmentCallableKind;

    match declaration.kind() {
        EnvironmentCallableKind::Function => "function",
        EnvironmentCallableKind::Method => "method",
        EnvironmentCallableKind::UntypedMethodFallback => "untyped_method_fallback",
        EnvironmentCallableKind::RustFunction => "rust_function",
    }
}

fn digest_hex(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
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
    flow_id: &ProjectEntityId,
) -> Option<AgentProjectFlowControlSummary> {
    let summary = project.flow_control_summary(flow_id)?;
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

fn agent_flow_control_summary_text(
    project: &ProjectSemanticIndex,
    flow_id: &ProjectEntityId,
) -> String {
    let Some(summary) = project.flow_control_summary(flow_id) else {
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
    let range = source.byte_range();
    Some(RequiredEntitySourceAnchor {
        path: source.source().id().as_str().to_owned(),
        start_byte: u64::try_from(range.start).ok()?,
        end_byte: u64::try_from(range.end).ok()?,
        start: None,
        end: None,
    })
}
