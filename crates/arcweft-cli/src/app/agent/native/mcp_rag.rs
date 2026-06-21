use super::*;

pub(super) fn agent_mcp_call_rag_query(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let query_text = arguments
        .get("query")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .ok_or_else(|| "arcweft.rag.query requires non-empty arguments.query".to_owned())?;
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(8);
    if limit == 0 {
        return Err("arcweft.rag.query argument limit must be at least 1".to_owned());
    }
    let max_context_bytes =
        agent_mcp_usize_argument(arguments, "max_context_bytes").unwrap_or(32 * 1024);
    if max_context_bytes == 0 {
        return Err("arcweft.rag.query argument max_context_bytes must be at least 1".to_owned());
    }
    let graph_depth =
        agent_mcp_u32_argument(arguments, "graph_depth", "arcweft.rag.query")?.unwrap_or(1);
    let roots = agent_mcp_rag_roots(arguments)?;
    let max_privacy = agent_mcp_privacy_class_argument(arguments, "max_privacy")?
        .unwrap_or(PrivacyClass::Project);
    let local_embedding =
        agent_mcp_bool_argument(arguments, "local_embedding", "arcweft.rag.query")?
            .unwrap_or(false);
    if local_embedding && agent_mcp_optional_debug_store_path(arguments).is_none() {
        return Err("arcweft.rag.query local_embedding requires arguments.path".to_owned());
    }
    let mut source_context = agent_mcp_rag_source_context(arguments)?;
    let config = AgentMcpRagQueryConfig {
        roots,
        graph_depth,
        limit,
        max_context_bytes,
        max_privacy,
        local_embedding,
        local_embedding_model: agent_mcp_rag_local_embedding_model(arguments)?,
    };
    if let Some(path) = agent_mcp_optional_debug_store_path(arguments) {
        source_context
            .candidates
            .extend(agent_mcp_rag_debug_store_candidates(
                path, query_text, &config,
            )?);
    }
    let result = agent_mcp_rag_query_result(state, source_context, query_text, config)?;
    if let Some(path) = agent_mcp_optional_debug_store_path(arguments) {
        persist_agent_mcp_rag_query_result(path, &result)?;
    }
    let pack = result.pack;
    let value = serde_json::to_value(&pack)
        .map_err(|error| format!("failed to serialize Agent RAG context pack: {error}"))?;
    state.rag_context_packs.push(pack);
    agent_mcp_json_tool_result(&value, "RAG context pack")
}

pub(super) fn agent_mcp_call_rag_explain(
    arguments: &serde_json::Value,
    state: &AgentMcpState,
) -> Result<McpCallToolResult, String> {
    let value = match agent_mcp_cached_rag_context_pack(arguments, state, "arcweft.rag.explain") {
        Ok(pack) => agent_mcp_rag_context_explanation_json(pack),
        Err(cache_error) => {
            let query_id =
                agent_mcp_non_empty_string_argument(arguments, "query_id").ok_or(cache_error)?;
            let audit =
                agent_mcp_rag_query_audit_from_store(arguments, query_id, "arcweft.rag.explain")?;
            agent_mcp_rag_query_audit_explanation_json(&audit)
        }
    };
    agent_mcp_json_tool_result(&value, "RAG explanation")
}

pub(super) fn agent_mcp_call_rag_context_read(
    arguments: &serde_json::Value,
    state: &AgentMcpState,
) -> Result<McpCallToolResult, String> {
    let stored_audit;
    let pack = match agent_mcp_cached_rag_context_pack(arguments, state, "arcweft.rag.context.read")
    {
        Ok(pack) => pack,
        Err(cache_error) => {
            let query_id =
                agent_mcp_non_empty_string_argument(arguments, "query_id").ok_or(cache_error)?;
            stored_audit = agent_mcp_rag_query_audit_from_store(
                arguments,
                query_id,
                "arcweft.rag.context.read",
            )?;
            &stored_audit.pack
        }
    };
    let chunk_id = agent_mcp_non_empty_string_argument(arguments, "chunk_id")
        .ok_or_else(|| "arcweft.rag.context.read requires arguments.chunk_id".to_owned())?;
    let max_bytes = agent_mcp_usize_argument(arguments, "max_bytes").unwrap_or(8192);
    if max_bytes == 0 {
        return Err("arcweft.rag.context.read argument max_bytes must be at least 1".to_owned());
    }
    let item = pack
        .items
        .iter()
        .find(|item| item.chunk_id.as_str() == chunk_id)
        .ok_or_else(|| {
            format!(
                "arcweft.rag.context.read could not find chunk_id `{chunk_id}` in cached query {}",
                pack.query.query_id
            )
        })?;
    let (body, body_truncated) = agent_mcp_truncate_utf8(&item.body, max_bytes);
    let value = serde_json::json!({
        "query_id": &pack.query.query_id,
        "chunk_id": item.chunk_id.as_str(),
        "kind": &item.kind,
        "title": &item.title,
        "body": body,
        "truncated": body_truncated,
        "body_bytes": item.body.len(),
        "returned_bytes": body.len(),
        "fused_score": item.fused_score,
        "channels": &item.channels,
        "entity_ids": &item.entity_ids,
        "source_anchor": &item.source_anchor,
    });
    agent_mcp_json_tool_result(&value, "RAG context item")
}

pub(super) fn agent_mcp_rag_query_audit_from_store(
    arguments: &serde_json::Value,
    query_id: &str,
    tool: &str,
) -> Result<DebugRagQueryAudit, String> {
    let path = agent_mcp_debug_store_path(arguments);
    let max_privacy = agent_mcp_max_privacy_argument(arguments, tool)?;
    let store = DebugStore::open(path)
        .map_err(|error| format!("{tool} failed to open `{path}`: {error}"))?;
    store
        .rag_query_audit_with_max_privacy(query_id, max_privacy)
        .map_err(|error| {
            format!("{tool} failed to read RAG query `{query_id}` from `{path}`: {error}")
        })
}

pub(super) fn agent_mcp_rag_context_explanation_json(pack: &RagContextPack) -> serde_json::Value {
    serde_json::json!({
        "schema_version": pack.schema_version,
        "query": &pack.query,
        "item_count": pack.items.len(),
        "truncated": pack.truncated,
        "items": pack
            .items
            .iter()
            .map(agent_mcp_rag_context_item_explanation)
            .collect::<Vec<_>>(),
    })
}

pub(super) fn agent_mcp_rag_query_audit_explanation_json(
    audit: &DebugRagQueryAudit,
) -> serde_json::Value {
    let mut value = agent_mcp_rag_context_explanation_json(&audit.pack);
    if let serde_json::Value::Object(fields) = &mut value {
        fields.insert(
            "session_id".to_owned(),
            audit
                .session_id
                .as_ref()
                .map_or(serde_json::Value::Null, |id| {
                    serde_json::Value::String(id.as_str().to_owned())
                }),
        );
        fields.insert(
            "run_id".to_owned(),
            audit.run_id.as_ref().map_or(serde_json::Value::Null, |id| {
                serde_json::Value::String(id.as_str().to_owned())
            }),
        );
        fields.insert(
            "status".to_owned(),
            serde_json::Value::String(audit.status.clone()),
        );
        fields.insert(
            "created_unix_ms".to_owned(),
            serde_json::Value::from(audit.created_unix_ms),
        );
        fields.insert(
            "source".to_owned(),
            serde_json::Value::String("debug_store".to_owned()),
        );
    }
    value
}

pub(super) fn agent_mcp_cached_rag_context_pack<'a>(
    arguments: &serde_json::Value,
    state: &'a AgentMcpState,
    tool: &str,
) -> Result<&'a RagContextPack, String> {
    let query_id = agent_mcp_non_empty_string_argument(arguments, "query_id");
    if let Some(query_id) = query_id {
        return state
            .rag_context_packs
            .iter()
            .rev()
            .find(|pack| pack.query.query_id == query_id)
            .ok_or_else(|| format!("{tool} could not find cached query_id `{query_id}`"));
    }
    state
        .rag_context_packs
        .last()
        .ok_or_else(|| format!("{tool} requires a prior arcweft.rag.query call"))
}

pub(super) fn agent_mcp_rag_context_item_explanation(item: &RagContextItem) -> serde_json::Value {
    serde_json::json!({
        "chunk_id": item.chunk_id.as_str(),
        "kind": &item.kind,
        "title": &item.title,
        "body_bytes": item.body.len(),
        "fused_score": item.fused_score,
        "channels": &item.channels,
        "entity_ids": &item.entity_ids,
        "source_anchor": &item.source_anchor,
    })
}

#[derive(Clone)]
pub(super) struct AgentMcpRagCandidate {
    pub(super) chunk: DebugChunk,
    pub(super) preferred_channel: SearchChannel,
}

pub(super) struct AgentMcpRagQueryResult {
    pub(super) pack: RagContextPack,
    pub(super) candidates: Vec<AgentMcpRagCandidate>,
    pub(super) source_indexes: Vec<AgentSourceRagIndex>,
}

pub(super) struct AgentMcpRagSourceContext {
    pub(super) candidates: Vec<AgentMcpRagCandidate>,
    pub(super) source_indexes: Vec<AgentSourceRagIndex>,
}

pub(super) struct AgentMcpRagQueryConfig {
    pub(super) roots: Vec<PublicId>,
    pub(super) graph_depth: u32,
    pub(super) limit: usize,
    pub(super) max_context_bytes: usize,
    pub(super) max_privacy: PrivacyClass,
    pub(super) local_embedding: bool,
    pub(super) local_embedding_model: EmbeddingModelDescriptor,
}

pub(super) fn agent_mcp_rag_context_pack(
    state: &AgentMcpState,
    query_text: &str,
    roots: Vec<PublicId>,
    graph_depth: u32,
    limit: usize,
    max_context_bytes: usize,
    max_privacy: PrivacyClass,
) -> Result<RagContextPack, String> {
    let config = AgentMcpRagQueryConfig {
        roots,
        graph_depth,
        limit,
        max_context_bytes,
        max_privacy,
        local_embedding: false,
        local_embedding_model: EmbeddingModelDescriptor {
            model_id: DEFAULT_LOCAL_EMBEDDING_MODEL_ID.to_owned(),
            model_revision: DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION.to_owned(),
            dimensions: DEFAULT_LOCAL_EMBEDDING_DIMENSIONS,
        },
    };
    agent_mcp_rag_query_result(
        state,
        AgentMcpRagSourceContext {
            candidates: Vec::new(),
            source_indexes: Vec::new(),
        },
        query_text,
        config,
    )
    .map(|result| result.pack)
}

pub(super) fn agent_mcp_rag_query_result(
    state: &AgentMcpState,
    source_context: AgentMcpRagSourceContext,
    query_text: &str,
    config: AgentMcpRagQueryConfig,
) -> Result<AgentMcpRagQueryResult, String> {
    let mut candidates = agent_mcp_rag_candidates(state)?;
    candidates.extend(source_context.candidates);
    let candidates = agent_mcp_rag_deduplicate_candidates(candidates);
    let query_candidates = candidates
        .iter()
        .filter(|candidate| candidate.chunk.privacy.is_allowed_by(config.max_privacy))
        .cloned()
        .collect::<Vec<_>>();
    if query_candidates.is_empty() {
        return Err(
            "arcweft.rag.query found no context allowed by max_privacy; observe a source/profile, read a trace, or raise arguments.max_privacy"
                .to_owned(),
        );
    }
    let program_hash = agent_mcp_rag_program_hash(state, &query_candidates)?;
    let query_id_seed = format!(
        "{}:{}:{graph_depth}:{limit}:{max_context_bytes}:{}",
        query_text,
        program_hash.as_str(),
        config.max_privacy.as_str(),
        graph_depth = config.graph_depth,
        limit = config.limit,
        max_context_bytes = config.max_context_bytes,
    );
    let query = RagQuery {
        query_id: agent_mcp_content_hash(query_id_seed),
        text: query_text.to_owned(),
        program_hash,
        roots: config.roots,
        graph_depth: config.graph_depth,
        limit: config.limit,
        max_context_bytes: config.max_context_bytes,
    };
    let pack = agent_mcp_rag_context_pack_from_candidates(
        query,
        &query_candidates,
        config.limit,
        config.max_context_bytes,
    );
    Ok(AgentMcpRagQueryResult {
        pack,
        candidates,
        source_indexes: source_context.source_indexes,
    })
}

pub(super) fn agent_mcp_rag_debug_store_candidates(
    path: &str,
    query_text: &str,
    config: &AgentMcpRagQueryConfig,
) -> Result<Vec<AgentMcpRagCandidate>, String> {
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.rag.query failed to open `{path}`: {error}"))?;
    let search_limit = config.limit.saturating_mul(8).max(32);
    let terms = std::iter::once(query_text.trim().to_owned())
        .chain(config.roots.iter().map(|root| root.as_str().to_owned()))
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    if config.local_embedding {
        let vector_results = agent_mcp_rag_debug_store_vector_results(
            &store,
            path,
            query_text,
            config,
            search_limit,
        )?;
        if vector_results.is_empty() {
            agent_mcp_rag_record_local_embedding_fallback(&store, path, query_text, config)?;
        }
        for result in vector_results {
            if seen.insert(result.chunk.id.clone()) {
                candidates.push(AgentMcpRagCandidate {
                    chunk: result.chunk,
                    preferred_channel: result.hit.channel,
                });
            }
        }
    }
    for term in terms {
        let mut results = store
            .lexical_chunk_search_with_max_privacy(&term, search_limit, config.max_privacy)
            .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?;
        results.extend(
            store
                .graph_search_with_depth_and_max_privacy(
                    &term,
                    config.graph_depth,
                    search_limit,
                    config.max_privacy,
                )
                .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_mcp_rag_candidate_from_search_result(result, "graph"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        results.extend(
            store
                .history_search_with_max_privacy(&term, search_limit, config.max_privacy)
                .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_mcp_rag_candidate_from_search_result(result, "history"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        results.extend(
            store
                .diagnostic_search_with_max_privacy(&term, search_limit, config.max_privacy)
                .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_mcp_rag_candidate_from_search_result(result, "diagnostic"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        results.extend(
            store
                .test_result_search_with_max_privacy(&term, search_limit, config.max_privacy)
                .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_mcp_rag_candidate_from_search_result(result, "test"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        for result in results {
            if seen.insert(result.chunk.id.clone()) {
                candidates.push(AgentMcpRagCandidate {
                    chunk: result.chunk,
                    preferred_channel: result.hit.channel,
                });
            }
        }
    }
    Ok(candidates)
}

pub(super) fn agent_mcp_rag_record_local_embedding_fallback(
    store: &DebugStore,
    path: &str,
    query_text: &str,
    config: &AgentMcpRagQueryConfig,
) -> Result<(), String> {
    let model = &config.local_embedding_model;
    let query = query_text.trim();
    let diagnostic_id = format!(
        "agent-mcp-rag-local-embedding-fallback:{}",
        agent_mcp_content_hash(format!(
            "{}:{}:{}:{}",
            path, query, model.model_id, model.model_revision
        ))
    );
    store
        .upsert_diagnostic(&DebugDiagnostic {
            diagnostic_id,
            program_hash: None,
            session_id: None,
            run_id: None,
            sequence: None,
            code: Some("AGENT_RAG_EMBEDDING_FALLBACK".to_owned()),
            severity: "warning".to_owned(),
            phase: "agent_rag".to_owned(),
            message: format!(
                "local embedding channel produced no hits for model {}@{}:{}; using lexical fallback channels",
                model.model_id, model.model_revision, model.dimensions
            ),
            source_path: Some(path.to_owned()),
            start_byte: None,
            end_byte: None,
            related_ids: Vec::new(),
            payload: serde_json::json!({
                "provider": "local_hash",
                "model": {
                    "model_id": model.model_id,
                    "model_revision": model.model_revision,
                    "dimensions": model.dimensions,
                },
                "query": query,
                "fallback_channels": ["lexical", "graph", "history", "diagnostics", "test_result"],
                "reason": "no_vector_hits",
            }),
            created_unix_ms: 0,
        })
        .map_err(|error| {
            format!(
                "arcweft.rag.query failed to record local embedding fallback diagnostic in `{path}`: {error}"
            )
        })
}

pub(super) fn agent_mcp_rag_debug_store_vector_results(
    store: &DebugStore,
    path: &str,
    query_text: &str,
    config: &AgentMcpRagQueryConfig,
    search_limit: usize,
) -> Result<Vec<DebugChunkSearchResult>, String> {
    let query_vector =
        local_hash_query_embedding(query_text.trim(), config.local_embedding_model.dimensions);
    store
        .vector_search_with_max_privacy(
            &config.local_embedding_model,
            &query_vector,
            search_limit,
            config.max_privacy,
        )
        .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?
        .into_iter()
        .map(|result| agent_mcp_rag_candidate_from_search_result(result, "vector"))
        .collect()
}

pub(super) fn agent_mcp_rag_local_embedding_model(
    arguments: &serde_json::Value,
) -> Result<EmbeddingModelDescriptor, String> {
    let model_id = agent_mcp_non_empty_string_argument(arguments, "local_embedding_model_id")
        .unwrap_or(DEFAULT_LOCAL_EMBEDDING_MODEL_ID);
    let model_revision =
        agent_mcp_non_empty_string_argument(arguments, "local_embedding_model_revision")
            .unwrap_or(DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION);
    let dimensions =
        agent_mcp_u32_argument(arguments, "local_embedding_dimensions", "arcweft.rag.query")?
            .unwrap_or(DEFAULT_LOCAL_EMBEDDING_DIMENSIONS);
    if dimensions == 0 {
        return Err(
            "arcweft.rag.query argument local_embedding_dimensions must be at least 1".to_owned(),
        );
    }
    if dimensions > MAX_LOCAL_EMBEDDING_DIMENSIONS {
        return Err(format!(
            "arcweft.rag.query argument local_embedding_dimensions must be at most {MAX_LOCAL_EMBEDDING_DIMENSIONS}"
        ));
    }
    Ok(EmbeddingModelDescriptor {
        model_id: model_id.to_owned(),
        model_revision: model_revision.to_owned(),
        dimensions,
    })
}

pub(super) fn agent_mcp_rag_debug_store_search_error(
    path: &str,
    error: &arcweft_debug_sqlite::store::DebugStoreError,
) -> String {
    format!("arcweft.rag.query failed to search debug store `{path}`: {error}")
}

pub(super) fn agent_mcp_rag_candidate_from_search_result(
    result: ChunkSearchResult,
    source_prefix: &str,
) -> Result<DebugChunkSearchResult, String> {
    let source_kind = match result.source_kind.as_str() {
        "source" => ChunkSourceKind::Source,
        "symbol" => ChunkSourceKind::Symbol,
        "graph_summary" | "graph_edge" | "graph_symbol" => ChunkSourceKind::GraphSummary,
        "diagnostic" => ChunkSourceKind::Diagnostic,
        "test_result" => ChunkSourceKind::TestResult,
        "agent_trace" => ChunkSourceKind::AgentTrace,
        "history" => ChunkSourceKind::History,
        "documentation" => ChunkSourceKind::Documentation,
        other => {
            return Err(format!(
                "arcweft.rag.query debug store result has unsupported source_kind `{other}`"
            ));
        }
    };
    let content_hash = agent_mcp_content_hash(&result.body);
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "search_channel".to_owned(),
        serde_json::json!(agent_mcp_search_channel_label(result.hit.channel)),
    );
    metadata.insert(
        "search_score".to_owned(),
        serde_json::to_value(result.hit.score).map_err(|error| {
            format!("arcweft.rag.query failed to serialize debug store search score: {error}")
        })?,
    );
    Ok(DebugChunkSearchResult {
        hit: result.hit.clone(),
        chunk: DebugChunk {
            id: result.hit.chunk_id,
            program_hash: None,
            source_kind,
            source_key: format!("{source_prefix}:{}", result.source_key),
            title: result.title,
            body: result.body,
            content_hash: StableHash::new(content_hash)
                .expect("generated content hash is non-empty"),
            semantic_hash: None,
            source_anchor: None,
            entity_ids: Vec::new(),
            privacy: result.privacy,
            metadata,
            created_unix_ms: 0,
        },
    })
}

pub(super) fn agent_mcp_rag_deduplicate_candidates(
    candidates: Vec<AgentMcpRagCandidate>,
) -> Vec<AgentMcpRagCandidate> {
    let mut seen = BTreeSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.chunk.id.clone()))
        .collect()
}

pub(super) fn agent_mcp_rag_context_pack_from_candidates(
    query: RagQuery,
    candidates: &[AgentMcpRagCandidate],
    limit: usize,
    max_context_bytes: usize,
) -> RagContextPack {
    let fused = reciprocal_rank_fusion(
        &agent_mcp_rag_ranked_lists(candidates, &query),
        &FusionConfig::default(),
        candidates.len(),
    );
    let by_id = candidates
        .iter()
        .map(|candidate| (candidate.chunk.id.clone(), &candidate.chunk))
        .collect::<BTreeMap<_, _>>();
    let mut used_bytes = 0usize;
    let mut truncated = false;
    let mut items = Vec::new();
    let mut selected_semantic_hashes = BTreeSet::new();
    let mut selected_source_anchors = Vec::new();
    for hit in fused {
        if items.len() >= limit {
            break;
        }
        let Some(chunk) = by_id.get(&hit.chunk_id).copied() else {
            continue;
        };
        if !agent_rag_select_context_chunk(
            chunk,
            &mut selected_semantic_hashes,
            &mut selected_source_anchors,
        ) {
            continue;
        }
        let remaining = max_context_bytes.saturating_sub(used_bytes);
        if remaining == 0 {
            truncated = true;
            break;
        }
        let (body, body_truncated) = agent_mcp_truncate_utf8(&chunk.body, remaining);
        truncated |= body_truncated;
        used_bytes = used_bytes.saturating_add(body.len());
        items.push(RagContextItem {
            chunk_id: chunk.id.clone(),
            kind: chunk.source_kind,
            title: chunk.title.clone(),
            body,
            fused_score: hit.fused_score,
            channels: hit.channels,
            entity_ids: chunk.entity_ids.clone(),
            source_anchor: chunk.source_anchor.clone(),
        });
        if body_truncated {
            break;
        }
    }
    RagContextPack {
        schema_version: 1,
        query,
        items,
        truncated,
    }
}

pub(super) fn persist_agent_mcp_rag_query_result(
    path: &str,
    result: &AgentMcpRagQueryResult,
) -> Result<(), String> {
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.rag.query failed to open `{path}`: {error}"))?;
    store
        .upsert_program(&result.pack.query.program_hash, None, None, 0)
        .map_err(|error| {
            format!("arcweft.rag.query failed to index program in `{path}`: {error}")
        })?;
    let session_id = agent_mcp_rag_query_session_id(&result.pack.query.query_id)?;
    store
        .upsert_session(&DebugSession {
            session_id: session_id.clone(),
            program_hash: Some(result.pack.query.program_hash.clone()),
            profile: "rag".to_owned(),
            transport: "mcp".to_owned(),
            started_unix_ms: 0,
            ended_unix_ms: Some(0),
            status: DebugSessionStatus::Finished,
            metadata: agent_mcp_rag_query_session_metadata(result),
        })
        .map_err(|error| {
            format!("arcweft.rag.query failed to record RAG session in `{path}`: {error}")
        })?;
    for candidate in &result.candidates {
        let mut chunk = candidate.chunk.clone();
        chunk.program_hash = Some(result.pack.query.program_hash.clone());
        store.upsert_chunk(&chunk).map_err(|error| {
            format!("arcweft.rag.query failed to index context chunk in `{path}`: {error}")
        })?;
    }
    if !result.source_indexes.is_empty() {
        for source_index in &result.source_indexes {
            let mut source_file = source_index.source_file.clone();
            source_file.program_hash = result.pack.query.program_hash.clone();
            store.upsert_source_file(&source_file).map_err(|error| {
                format!("arcweft.rag.query failed to index source file in `{path}`: {error}")
            })?;
            agent_rag_index_graph(
                &store,
                &result.pack.query.program_hash,
                source_index,
                result.source_indexes.len() > 1,
            )?;
        }
        let source_index_refs = result.source_indexes.iter().collect::<Vec<_>>();
        agent_rag_index_program_graph(&store, &result.pack.query.program_hash, &source_index_refs)?;
    }
    store
        .record_rag_context_pack(&result.pack, Some(&session_id), None, None, "selected", 0)
        .map_err(|error| {
            format!("arcweft.rag.query failed to record RAG audit in `{path}`: {error}")
        })
}

pub(super) fn agent_mcp_rag_query_session_id(query_id: &str) -> Result<SessionId, String> {
    let suffix = agent_mcp_content_hash(format!("mcp:{query_id}")).replace(':', ".");
    SessionId::new(format!("session.rag.mcp.{suffix}"))
        .map_err(|error| format!("failed to build MCP RAG session id: {error}"))
}

pub(super) fn agent_mcp_rag_query_session_metadata(
    result: &AgentMcpRagQueryResult,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        (
            "query_id".to_owned(),
            serde_json::json!(result.pack.query.query_id),
        ),
        (
            "query_text".to_owned(),
            serde_json::json!(result.pack.query.text),
        ),
        (
            "item_count".to_owned(),
            serde_json::json!(result.pack.items.len()),
        ),
        (
            "truncated".to_owned(),
            serde_json::json!(result.pack.truncated),
        ),
        (
            "roots".to_owned(),
            serde_json::json!(
                result
                    .pack
                    .query
                    .roots
                    .iter()
                    .map(PublicId::as_str)
                    .collect::<Vec<_>>()
            ),
        ),
        (
            "graph_depth".to_owned(),
            serde_json::json!(result.pack.query.graph_depth),
        ),
        (
            "max_context_bytes".to_owned(),
            serde_json::json!(result.pack.query.max_context_bytes),
        ),
    ])
}

pub(super) fn agent_mcp_optional_debug_store_path(arguments: &serde_json::Value) -> Option<&str> {
    arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
}

pub(super) fn agent_mcp_rag_source_context(
    arguments: &serde_json::Value,
) -> Result<AgentMcpRagSourceContext, String> {
    let inputs = agent_mcp_rag_source_inputs(arguments)?;
    let paths = agent_rag_source_paths(&inputs)?;
    let source_indexes = paths
        .iter()
        .map(|path| agent_source_rag_index(path))
        .collect::<Result<Vec<_>, _>>()?;
    let mut candidates = source_indexes
        .iter()
        .flat_map(|index| index.candidates.iter().cloned())
        .map(agent_mcp_rag_candidate_from_cli_source)
        .collect::<Vec<_>>();
    if !source_indexes.is_empty() {
        let seed_parts = source_indexes
            .iter()
            .map(|index| index.seed.clone())
            .collect::<Vec<_>>();
        let program_hash = agent_rag_program_hash(&seed_parts)?;
        let source_index_refs = source_indexes.iter().collect::<Vec<_>>();
        candidates.push(agent_mcp_rag_candidate_from_cli_source(
            agent_program_summary_rag_candidate(&program_hash, &source_index_refs)?,
        ));
    }
    Ok(AgentMcpRagSourceContext {
        candidates,
        source_indexes,
    })
}

pub(super) fn agent_mcp_rag_candidate_from_cli_source(
    candidate: super::AgentRagCandidate,
) -> AgentMcpRagCandidate {
    let mut chunk = candidate.chunk;
    chunk.id = ChunkId::new(format!(
        "mcp:{}:{}",
        chunk.source_key,
        chunk.content_hash.as_str()
    ));
    AgentMcpRagCandidate {
        chunk,
        preferred_channel: candidate.preferred_channel,
    }
}

pub(super) fn agent_mcp_rag_source_inputs(
    arguments: &serde_json::Value,
) -> Result<Vec<PathBuf>, String> {
    let mut inputs = Vec::new();
    if let Some(source) = agent_mcp_non_empty_string_argument(arguments, "source") {
        inputs.push(PathBuf::from(source));
    }
    let Some(sources) = arguments.get("sources") else {
        return Ok(inputs);
    };
    match sources {
        serde_json::Value::String(source) => {
            let source = source.trim();
            if !source.is_empty() {
                inputs.push(PathBuf::from(source));
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                let source = item
                    .as_str()
                    .map(str::trim)
                    .filter(|source| !source.is_empty())
                    .ok_or_else(|| {
                        "arcweft.rag.query argument sources must contain non-empty strings"
                            .to_owned()
                    })?;
                inputs.push(PathBuf::from(source));
            }
        }
        _ => {
            return Err(
                "arcweft.rag.query argument sources must be a string or an array of strings"
                    .to_owned(),
            );
        }
    }
    Ok(inputs)
}

pub(super) fn agent_mcp_rag_candidates(
    state: &AgentMcpState,
) -> Result<Vec<AgentMcpRagCandidate>, String> {
    let mut candidates = Vec::new();
    if let Some(report) = &state.report {
        let summary = agent_mcp_observation_state_summary(report);
        candidates.push(agent_mcp_rag_json_candidate(
            "observation.summary",
            "Observation state summary",
            ChunkSourceKind::GraphSummary,
            SearchChannel::Summary,
            &summary,
            Vec::new(),
            PrivacyClass::Project,
        )?);
        candidates.push(agent_mcp_rag_json_candidate(
            "observation.actions",
            "Agent action targets",
            ChunkSourceKind::GraphSummary,
            SearchChannel::Graph,
            &serde_json::to_value(&report.actions)
                .map_err(|error| format!("failed to serialize Agent actions: {error}"))?,
            Vec::new(),
            PrivacyClass::Project,
        )?);
        for object in &report.objects {
            candidates.push(agent_mcp_rag_json_candidate(
                &format!("object.{}", object.id),
                &format!("Observed object {} role {}", object.id, object.role),
                ChunkSourceKind::GraphSummary,
                SearchChannel::Graph,
                &serde_json::to_value(object)
                    .map_err(|error| format!("failed to serialize observed object: {error}"))?,
                agent_mcp_object_entity_ids(object),
                PrivacyClass::Project,
            )?);
        }
        for signal in &report.signals {
            candidates.push(agent_mcp_rag_json_candidate(
                &format!("signal.{}", signal.name),
                &format!("Signal {}", signal.name),
                ChunkSourceKind::AgentTrace,
                SearchChannel::Trace,
                &serde_json::to_value(signal)
                    .map_err(|error| format!("failed to serialize Agent signal: {error}"))?,
                agent_mcp_public_ids([signal.name.as_str()]),
                PrivacyClass::Project,
            )?);
        }
        for metric in &report.metrics {
            candidates.push(agent_mcp_rag_json_candidate(
                &format!("metric.{}", metric.name),
                &format!("Metric {}", metric.name),
                ChunkSourceKind::AgentTrace,
                SearchChannel::Trace,
                &serde_json::to_value(metric)
                    .map_err(|error| format!("failed to serialize Agent metric: {error}"))?,
                agent_mcp_public_ids([metric.name.as_str()]),
                PrivacyClass::Project,
            )?);
        }
        for (index, log) in report.logs.iter().enumerate() {
            candidates.push(agent_mcp_rag_json_candidate(
                &format!("log.{index}"),
                &format!("Runtime log {} {}", log.level, index),
                ChunkSourceKind::AgentTrace,
                SearchChannel::Trace,
                &serde_json::to_value(log)
                    .map_err(|error| format!("failed to serialize runtime log: {error}"))?,
                Vec::new(),
                PrivacyClass::Project,
            )?);
        }
        for diagnostic in &report.diagnostics {
            candidates.push(agent_mcp_rag_json_candidate(
                &format!("diagnostic.{}.{}", diagnostic.step, candidates.len()),
                &format!(
                    "Diagnostic {:?} step {}",
                    diagnostic.severity, diagnostic.step
                ),
                ChunkSourceKind::Diagnostic,
                SearchChannel::Diagnostics,
                &serde_json::to_value(diagnostic)
                    .map_err(|error| format!("failed to serialize Agent diagnostic: {error}"))?,
                agent_mcp_public_ids(
                    [
                        diagnostic.source.as_deref(),
                        diagnostic.effect_id.as_deref(),
                    ]
                    .into_iter()
                    .flatten(),
                ),
                PrivacyClass::Project,
            )?);
        }
    }
    for resource in &state.trace_resources {
        candidates.extend(agent_mcp_trace_resource_rag_candidates(resource)?);
    }
    Ok(candidates)
}

pub(super) fn agent_mcp_rag_json_candidate(
    source_key: &str,
    title: &str,
    source_kind: ChunkSourceKind,
    preferred_channel: SearchChannel,
    value: &serde_json::Value,
    entity_ids: Vec<PublicId>,
    privacy: PrivacyClass,
) -> Result<AgentMcpRagCandidate, String> {
    let body = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize RAG candidate body: {error}"))?;
    Ok(agent_mcp_rag_text_candidate(
        source_key,
        title,
        source_kind,
        preferred_channel,
        body,
        entity_ids,
        privacy,
    ))
}

pub(super) fn agent_mcp_rag_text_candidate(
    source_key: &str,
    title: &str,
    source_kind: ChunkSourceKind,
    preferred_channel: SearchChannel,
    body: String,
    entity_ids: Vec<PublicId>,
    privacy: PrivacyClass,
) -> AgentMcpRagCandidate {
    let content_hash = agent_mcp_content_hash(&body);
    AgentMcpRagCandidate {
        chunk: DebugChunk {
            id: ChunkId::new(format!("mcp:{source_key}:{content_hash}")),
            program_hash: None,
            source_kind,
            source_key: source_key.to_owned(),
            title: title.to_owned(),
            body,
            content_hash: StableHash::new(content_hash)
                .expect("generated content hash is non-empty"),
            semantic_hash: None,
            source_anchor: None,
            entity_ids,
            privacy,
            metadata: BTreeMap::new(),
            created_unix_ms: 0,
        },
        preferred_channel,
    }
}

pub(super) fn agent_mcp_trace_resource_rag_candidates(
    resource: &AgentResource,
) -> Result<Vec<AgentMcpRagCandidate>, String> {
    let AgentResourceBody::Json(value) = &resource.body else {
        return Ok(vec![agent_mcp_rag_text_candidate(
            &resource.uri,
            &format!("Trace resource {}", resource.uri),
            ChunkSourceKind::AgentTrace,
            SearchChannel::Trace,
            agent_mcp_resource_body_text(resource)?,
            Vec::new(),
            PrivacyClass::Project,
        )]);
    };
    let Some(records) = value.as_array() else {
        return Ok(vec![agent_mcp_rag_json_candidate(
            &resource.uri,
            &format!("Trace resource {}", resource.uri),
            ChunkSourceKind::AgentTrace,
            SearchChannel::Trace,
            value,
            Vec::new(),
            PrivacyClass::Project,
        )?]);
    };
    records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let title = record
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .map_or_else(
                    || format!("Trace record {index}"),
                    |kind| format!("Trace record {index} {kind}"),
                );
            agent_mcp_rag_json_candidate(
                &format!("{}.record.{index}", resource.uri),
                &title,
                ChunkSourceKind::AgentTrace,
                SearchChannel::Trace,
                record,
                Vec::new(),
                agent_mcp_json_privacy(record),
            )
        })
        .collect()
}

pub(super) fn agent_mcp_json_privacy(value: &serde_json::Value) -> PrivacyClass {
    value
        .get("privacy_class")
        .or_else(|| value.get("privacy"))
        .or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get("privacy_class"))
        })
        .or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get("privacy"))
        })
        .and_then(serde_json::Value::as_str)
        .and_then(PrivacyClass::parse)
        .unwrap_or(PrivacyClass::Project)
}

pub(super) fn agent_mcp_resource_body_text(resource: &AgentResource) -> Result<String, String> {
    match &resource.body {
        AgentResourceBody::Json(value) => serde_json::to_string_pretty(value)
            .map_err(|error| format!("failed to serialize Agent resource body: {error}")),
        AgentResourceBody::Text(text) => Ok(text.clone()),
        AgentResourceBody::BytesBase64(_) => Ok(format!(
            "binary resource {} mime_type={} hash={}",
            resource.uri, resource.mime_type, resource.hash
        )),
    }
}

pub(super) fn agent_mcp_rag_ranked_lists(
    candidates: &[AgentMcpRagCandidate],
    query: &RagQuery,
) -> Vec<Vec<SearchHit>> {
    [
        SearchChannel::ExactEntity,
        SearchChannel::Lexical,
        SearchChannel::Vector,
        SearchChannel::Graph,
        SearchChannel::History,
        SearchChannel::Diagnostics,
        SearchChannel::Trace,
        SearchChannel::Summary,
    ]
    .into_iter()
    .filter_map(|channel| {
        let mut scored = candidates
            .iter()
            .filter_map(|candidate| {
                agent_mcp_rag_score(candidate, query, channel).map(|score| (candidate, score))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.chunk.id.cmp(&right.0.chunk.id))
        });
        if scored.is_empty() {
            return None;
        }
        Some(
            scored
                .into_iter()
                .enumerate()
                .map(|(index, (candidate, score))| SearchHit {
                    chunk_id: candidate.chunk.id.clone(),
                    channel,
                    rank: index + 1,
                    score: Some(score),
                })
                .collect(),
        )
    })
    .collect()
}

pub(super) fn agent_mcp_rag_score(
    candidate: &AgentMcpRagCandidate,
    query: &RagQuery,
    channel: SearchChannel,
) -> Option<f64> {
    let haystack = agent_mcp_rag_haystack(candidate);
    match channel {
        SearchChannel::ExactEntity => {
            let root_match = query.roots.iter().any(|root| {
                candidate
                    .chunk
                    .entity_ids
                    .iter()
                    .any(|entity| entity == root)
                    || candidate.chunk.source_key == root.as_str()
                    || candidate.chunk.title.contains(root.as_str())
            });
            let query_match = candidate
                .chunk
                .entity_ids
                .iter()
                .any(|entity| entity.as_str() == query.text)
                || candidate.chunk.source_key == query.text
                || candidate.chunk.title == query.text;
            (root_match || query_match).then_some(1.0)
        }
        SearchChannel::Lexical => {
            let query_lower = query.text.to_lowercase();
            let phrase = f64::from(u8::from(haystack.contains(&query_lower)));
            let token_score = agent_mcp_rag_tokens(&query.text)
                .into_iter()
                .filter(|token| haystack.contains(token))
                .count();
            let token_score = agent_mcp_count_as_f64(token_score);
            (phrase + token_score > 0.0).then_some(phrase.mul_add(4.0, token_score))
        }
        SearchChannel::Graph => {
            let root_score = if query.graph_depth > 0 {
                let count = query
                    .roots
                    .iter()
                    .filter(|root| haystack.contains(&root.as_str().to_lowercase()))
                    .count();
                agent_mcp_count_as_f64(count)
            } else {
                0.0
            };
            let channel_score = f64::from(u8::from(
                candidate.preferred_channel == SearchChannel::Graph,
            ));
            (root_score + channel_score > 0.0).then_some(root_score + channel_score)
        }
        SearchChannel::History
        | SearchChannel::Diagnostics
        | SearchChannel::Trace
        | SearchChannel::Summary => {
            if candidate.preferred_channel != channel {
                return None;
            }
            let token_score = agent_mcp_rag_tokens(&query.text)
                .into_iter()
                .filter(|token| haystack.contains(token))
                .count();
            let token_score = agent_mcp_count_as_f64(token_score);
            (token_score > 0.0).then_some(token_score)
        }
        SearchChannel::Vector => {
            if candidate.preferred_channel != SearchChannel::Vector {
                return None;
            }
            candidate
                .chunk
                .metadata
                .get("search_score")
                .and_then(serde_json::Value::as_f64)
                .filter(|score| score.is_finite())
        }
    }
}

pub(super) fn agent_mcp_count_as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

pub(super) fn agent_mcp_rag_haystack(candidate: &AgentMcpRagCandidate) -> String {
    let mut haystack = format!(
        "{}\n{}\n{}",
        candidate.chunk.source_key, candidate.chunk.title, candidate.chunk.body
    )
    .to_lowercase();
    for entity in &candidate.chunk.entity_ids {
        haystack.push('\n');
        haystack.push_str(&entity.as_str().to_lowercase());
    }
    haystack
}

pub(super) fn agent_mcp_rag_tokens(text: &str) -> BTreeSet<String> {
    text.split(|character: char| {
        !(character.is_alphanumeric() || character == '.' || character == '_' || character == '-')
    })
    .map(str::trim)
    .filter(|token| !token.is_empty())
    .map(str::to_lowercase)
    .collect()
}

pub(super) fn agent_mcp_rag_roots(arguments: &serde_json::Value) -> Result<Vec<PublicId>, String> {
    let Some(roots) = arguments.get("roots") else {
        return Ok(Vec::new());
    };
    let roots = roots
        .as_array()
        .ok_or_else(|| "arcweft.rag.query argument roots must be an array".to_owned())?;
    roots
        .iter()
        .map(|root| {
            let value = root
                .as_str()
                .ok_or_else(|| "arcweft.rag.query roots must contain only strings".to_owned())?;
            PublicId::new(value.to_owned())
                .map_err(|_| "arcweft.rag.query roots must not be empty".to_owned())
        })
        .collect()
}

pub(super) fn agent_mcp_privacy_class_argument(
    arguments: &serde_json::Value,
    name: &str,
) -> Result<Option<PrivacyClass>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("arcweft.rag.query argument {name} must be a string"))?;
    PrivacyClass::parse(value).map(Some).ok_or_else(|| {
        format!(
            "arcweft.rag.query argument {name} must be one of public, project, sensitive, or secret"
        )
    })
}

pub(super) fn agent_mcp_max_privacy_argument(
    arguments: &serde_json::Value,
    tool: &str,
) -> Result<PrivacyClass, String> {
    let Some(value) = arguments.get("max_privacy") else {
        return Ok(PrivacyClass::Project);
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("{tool} argument max_privacy must be a string"))?;
    PrivacyClass::parse(value).ok_or_else(|| {
        format!("{tool} argument max_privacy must be one of public, project, sensitive, or secret")
    })
}

pub(super) fn agent_mcp_observation_debug_read_privacy_error(
    resource: &str,
    privacy: PrivacyClass,
    max_privacy: PrivacyClass,
) -> Option<serde_json::Value> {
    (!privacy.is_allowed_by(max_privacy)).then(|| {
        serde_json::json!({
            "status": "blocked",
            "error": format!(
                "arcweft.{resource} is {privacy} and exceeds max_privacy {max_privacy}",
                privacy = privacy.as_str(),
                max_privacy = max_privacy.as_str(),
            ),
            "resource": resource,
            "privacy": privacy.as_str(),
            "max_privacy": max_privacy.as_str(),
        })
    })
}

pub(super) fn agent_mcp_object_entity_ids(object: &AgentObservedObject) -> Vec<PublicId> {
    agent_mcp_public_ids(
        [Some(object.id.as_str()), object.entity.as_deref()]
            .into_iter()
            .flatten(),
    )
}

pub(super) fn agent_mcp_public_ids<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<PublicId> {
    values
        .into_iter()
        .filter_map(|value| PublicId::new(value.to_owned()).ok())
        .collect()
}

pub(super) fn agent_mcp_rag_program_hash(
    state: &AgentMcpState,
    candidates: &[AgentMcpRagCandidate],
) -> Result<StableHash, String> {
    let candidate_hashes = candidates
        .iter()
        .map(|candidate| candidate.chunk.content_hash.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let seed = state.report.as_ref().map_or_else(
        || candidate_hashes.clone(),
        |report| {
            format!(
                "{}:{}:{}:{}\n{}",
                report.source, report.state_hash, report.render_hash, report.tick, candidate_hashes
            )
        },
    );
    StableHash::new(agent_mcp_content_hash(seed))
        .map_err(|_| "failed to build Agent RAG program hash".to_owned())
}

pub(super) fn agent_mcp_content_hash(bytes: impl AsRef<[u8]>) -> String {
    format!("blake3:{}", blake3::hash(bytes.as_ref()).to_hex())
}

pub(super) fn agent_mcp_truncate_utf8(text: &str, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_owned(), false);
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}
