use super::{
    AgentActionSignature, AgentPublicId, AgentRagCommand, AgentRagContextReadOptions,
    AgentRagExplainOptions, AgentRagIndexOptions, AgentRagQueryOptions, AgentScriptTraceReport,
    AgentTraceRecord, BTreeMap, BTreeSet, ChunkId, ChunkSourceKind, DebugChunk, DebugDiagnostic,
    DebugGraphEdge, DebugGraphSymbol, DebugSession, DebugSessionStatus, DebugSourceAnchor,
    DebugSourceFile, DebugStore, EmbeddingModelDescriptor, EntitySymbol, ExitCode, FusionConfig,
    MAX_LOCAL_EMBEDDING_DIMENSIONS, Path, PathBuf, PrivacyClass, ProgramHash,
    ProjectCallableSymbol, ProjectSemanticIndex, QualifiedName, RagContextItem, RagContextPack,
    RagQuery, SearchChannel, SearchHit, SemaPublicId, SessionId, SourceAnchor, StableHash,
    agent_trace_kind_name, fs, local_hash_query_embedding, print_json,
    project_semantic_index_from_hir, read_and_validate_agent_trace_records, reciprocal_rank_fusion,
    validate_agent_trace,
};

pub(in crate::app::agent) mod source_index;
use source_index::{
    AgentProgramGraphSummary, AgentRagCandidateMeta, AgentSourceRagIndex, agent_content_hash,
    agent_program_graph_symbol, agent_program_source_file_graph_edge,
    agent_program_summary_rag_candidate, agent_rag_candidate, agent_rag_program_hash,
    agent_rag_roots, agent_rag_source_paths, agent_source_rag_index, agent_trace_rag_ranked_lists,
    agent_trace_rag_seed, agent_trace_record_entity_ids, agent_trace_record_privacy,
    search_channel_label, truncate_utf8,
};
pub(super) fn agent_rag_command(command: AgentRagCommand) -> Result<(), ExitCode> {
    match command {
        AgentRagCommand::Index(options) => agent_rag_index_command(&options),
        AgentRagCommand::Query(options) => agent_rag_query_command(&options),
        AgentRagCommand::Explain(options) => agent_rag_explain_command(&options),
        AgentRagCommand::ContextRead(options) => agent_rag_context_read_command(&options),
    }
}

pub(super) fn agent_rag_index_command(options: &AgentRagIndexOptions) -> Result<(), ExitCode> {
    match agent_rag_index_report(options) {
        Ok(report) => {
            if options.json {
                return print_json(&report);
            }
            println!(
                "{}: indexed {} chunk(s), skipped {} unchanged chunk(s), sources indexed={}, skipped={}",
                report.path,
                report.indexed_chunks,
                report.skipped_unchanged_chunks,
                report.indexed_sources,
                report.skipped_unchanged_sources
            );
            Ok(())
        }
        Err(error) => {
            eprintln!("agent rag index: {error}");
            Err(ExitCode::FAILURE)
        }
    }
}

pub(super) fn agent_rag_query_command(options: &AgentRagQueryOptions) -> Result<(), ExitCode> {
    match agent_rag_query_result(options) {
        Ok(result) => {
            if let Some(path) = &options.debug_db
                && let Err(error) = persist_agent_rag_query_result(path, &result)
            {
                eprintln!("{}: {error}", path.display());
                return Err(ExitCode::FAILURE);
            }
            let pack = result.pack;
            if options.json {
                print_json(&pack)?;
            } else {
                println!(
                    "{}: {} item(s), truncated={}",
                    agent_rag_query_input_label(options),
                    pack.items.len(),
                    pack.truncated
                );
                for item in &pack.items {
                    println!(
                        "- {} [{}] score={:.6}",
                        item.title,
                        item.chunk_id.as_str(),
                        item.fused_score
                    );
                }
            }
            Ok(())
        }
        Err(error) => {
            eprintln!("{}: {error}", agent_rag_query_input_label(options));
            Err(ExitCode::FAILURE)
        }
    }
}

pub(super) fn agent_rag_explain_command(options: &AgentRagExplainOptions) -> Result<(), ExitCode> {
    let query_id = options.query_id.trim();
    if query_id.is_empty() {
        eprintln!("agent rag explain: query id must not be empty");
        return Err(ExitCode::from(2));
    }
    match agent_rag_persisted_audit(&options.debug_db, query_id, options.max_privacy) {
        Ok(audit) => {
            let report = AgentRagExplainReport {
                path: options.debug_db.display().to_string(),
                query_id: query_id.to_owned(),
                session_id: audit.session_id.as_ref().map(|id| id.as_str().to_owned()),
                run_id: audit.run_id.as_ref().map(|id| id.as_str().to_owned()),
                max_privacy: options.max_privacy,
                status: audit.status,
                created_unix_ms: audit.created_unix_ms,
                query: audit.pack.query,
                item_count: audit.pack.items.len(),
                truncated: audit.pack.truncated,
                items: audit
                    .pack
                    .items
                    .into_iter()
                    .map(agent_rag_explain_item_report)
                    .collect(),
            };
            if options.json {
                return print_json(&report);
            }
            println!(
                "{}: RAG query {} status={} item(s)={} truncated={} max_privacy={}",
                report.path,
                report.query_id,
                report.status,
                report.item_count,
                report.truncated,
                report.max_privacy.as_str()
            );
            for item in &report.items {
                println!(
                    "- {} [{}] score={:.6}",
                    item.title,
                    item.chunk_id.as_str(),
                    item.fused_score
                );
            }
            Ok(())
        }
        Err(error) => {
            eprintln!("agent rag explain: {error}");
            Err(ExitCode::FAILURE)
        }
    }
}

pub(super) fn agent_rag_context_read_command(
    options: &AgentRagContextReadOptions,
) -> Result<(), ExitCode> {
    let query_id = options.query_id.trim();
    if query_id.is_empty() {
        eprintln!("agent rag context-read: query id must not be empty");
        return Err(ExitCode::from(2));
    }
    if options.chunk_id.trim().is_empty() {
        eprintln!("agent rag context-read: chunk id must not be empty");
        return Err(ExitCode::from(2));
    }
    if options.max_bytes == 0 {
        eprintln!("agent rag context-read --max-bytes must be at least 1");
        return Err(ExitCode::from(2));
    }
    match agent_rag_context_read_report(options, query_id) {
        Ok(report) => {
            if options.json {
                return print_json(&report);
            }
            println!(
                "{}: RAG context {} from query {} bytes={} truncated={}",
                report.path,
                report.chunk_id.as_str(),
                report.query_id,
                report.item.body.len(),
                report.body_truncated
            );
            println!("{}", report.item.body);
            Ok(())
        }
        Err(error) => {
            eprintln!("agent rag context-read: {error}");
            Err(ExitCode::FAILURE)
        }
    }
}

pub(super) fn agent_rag_index_report(
    options: &AgentRagIndexOptions,
) -> Result<AgentRagIndexReport, String> {
    if options.source.is_empty() {
        return Err("agent rag index requires at least one --source file or directory".to_owned());
    }
    let source_paths = agent_rag_source_paths(&options.source)?;
    if source_paths.is_empty() {
        return Err("agent rag index found no .arcw source files".to_owned());
    }
    let source_indexes = source_paths
        .iter()
        .map(|path| agent_source_rag_index(path).map(|index| (path, index)))
        .collect::<Result<Vec<_>, _>>()?;
    let seed_parts = source_indexes
        .iter()
        .map(|(_, index)| index.seed.clone())
        .collect::<Vec<_>>();
    let program_hash = agent_rag_program_hash(&seed_parts)?;
    let store = DebugStore::open(&options.debug_db)
        .map_err(|error| format!("failed to open RAG debug DB: {error}"))?;
    store
        .upsert_program(&program_hash, None, None, 0)
        .map_err(|error| format!("failed to index RAG program: {error}"))?;

    let mut source_reports = Vec::new();
    let mut indexed_chunks = 0usize;
    let mut skipped_unchanged_chunks = 0usize;
    for (source_path, source_index) in &source_indexes {
        let (report, indexed_for_source, skipped_for_source) = agent_rag_index_source(
            &store,
            &program_hash,
            source_path,
            source_index,
            options.changed,
            source_indexes.len() > 1,
        )?;
        indexed_chunks = indexed_chunks.saturating_add(indexed_for_source);
        skipped_unchanged_chunks = skipped_unchanged_chunks.saturating_add(skipped_for_source);
        source_reports.push(report);
    }
    let source_index_refs = source_indexes
        .iter()
        .map(|(_, index)| index)
        .collect::<Vec<_>>();
    let (indexed_program_chunks, skipped_program_chunks) = agent_rag_index_program_summary_chunk(
        &store,
        &program_hash,
        &source_index_refs,
        options.changed,
    )?;
    indexed_chunks = indexed_chunks.saturating_add(indexed_program_chunks);
    skipped_unchanged_chunks = skipped_unchanged_chunks.saturating_add(skipped_program_chunks);
    agent_rag_index_program_graph(&store, &program_hash, &source_index_refs)?;
    let indexed_sources = source_reports
        .iter()
        .filter(|source| source.indexed)
        .count();
    let skipped_unchanged_sources = source_reports
        .iter()
        .filter(|source| !source.indexed)
        .count();
    let session_id = agent_rag_index_session_id(&program_hash, options.changed)?;
    store
        .upsert_session(&DebugSession {
            session_id: session_id.clone(),
            program_hash: Some(program_hash.clone()),
            profile: "rag".to_owned(),
            transport: "cli".to_owned(),
            started_unix_ms: 0,
            ended_unix_ms: Some(0),
            status: DebugSessionStatus::Finished,
            metadata: agent_rag_index_session_metadata(
                options,
                &source_reports,
                indexed_sources,
                skipped_unchanged_sources,
                indexed_chunks,
                skipped_unchanged_chunks,
            ),
        })
        .map_err(|error| format!("failed to record RAG index session: {error}"))?;
    Ok(AgentRagIndexReport {
        path: options.debug_db.display().to_string(),
        session_id: session_id.as_str().to_owned(),
        changed_only: options.changed,
        program_hash: program_hash.as_str().to_owned(),
        sources: source_reports,
        indexed_sources,
        skipped_unchanged_sources,
        indexed_chunks,
        skipped_unchanged_chunks,
    })
}

pub(super) fn agent_rag_index_source(
    store: &DebugStore,
    program_hash: &StableHash,
    source_path: &Path,
    source_index: &AgentSourceRagIndex,
    changed_only: bool,
    scope_public_ids: bool,
) -> Result<(AgentRagIndexedSourceReport, usize, usize), String> {
    let mut source_file = source_index.source_file.clone();
    source_file.program_hash = program_hash.clone();
    store
        .upsert_source_file(&source_file)
        .map_err(|error| format!("failed to index RAG source file: {error}"))?;

    let mut indexed_chunks = 0usize;
    let mut skipped_unchanged_chunks = 0usize;
    for candidate in &source_index.candidates {
        if changed_only
            && store
                .chunk_content_hash_exists(&candidate.chunk.content_hash)
                .map_err(|error| format!("failed to check existing RAG chunk: {error}"))?
        {
            skipped_unchanged_chunks = skipped_unchanged_chunks.saturating_add(1);
            continue;
        }
        let mut chunk = candidate.chunk.clone();
        chunk.program_hash = Some(program_hash.clone());
        store
            .upsert_chunk(&chunk)
            .map_err(|error| format!("failed to index RAG chunk: {error}"))?;
        indexed_chunks = indexed_chunks.saturating_add(1);
    }

    agent_rag_index_graph(store, program_hash, source_index, scope_public_ids)?;
    Ok((
        AgentRagIndexedSourceReport {
            path: source_path.display().to_string(),
            source_hash: source_index.source_hash.clone(),
            candidate_chunks: source_index.candidates.len(),
            indexed_chunks,
            skipped_unchanged_chunks,
            source_file_recorded: true,
            indexed: indexed_chunks > 0,
        },
        indexed_chunks,
        skipped_unchanged_chunks,
    ))
}

pub(super) fn agent_rag_index_program_summary_chunk(
    store: &DebugStore,
    program_hash: &StableHash,
    source_indexes: &[&AgentSourceRagIndex],
    changed_only: bool,
) -> Result<(usize, usize), String> {
    let candidate = agent_program_summary_rag_candidate(program_hash, source_indexes)?;
    if changed_only
        && store
            .chunk_content_hash_exists(&candidate.chunk.content_hash)
            .map_err(|error| format!("failed to check existing RAG program summary: {error}"))?
    {
        return Ok((0, 1));
    }
    let mut chunk = candidate.chunk;
    chunk.program_hash = Some(program_hash.clone());
    store
        .upsert_chunk(&chunk)
        .map_err(|error| format!("failed to index RAG program summary: {error}"))?;
    Ok((1, 0))
}

pub(super) fn agent_rag_index_graph(
    store: &DebugStore,
    program_hash: &StableHash,
    source_index: &AgentSourceRagIndex,
    scope_public_ids: bool,
) -> Result<(), String> {
    for symbol in &source_index.graph_symbols {
        let mut symbol = symbol.clone();
        symbol.program_hash = program_hash.clone();
        if scope_public_ids {
            agent_scope_graph_symbol_public_id(&mut symbol, source_index);
        }
        store
            .upsert_graph_symbol(&symbol)
            .map_err(|error| format!("failed to index RAG graph symbol: {error}"))?;
    }
    for edge in &source_index.graph_edges {
        let mut edge = edge.clone();
        edge.program_hash = program_hash.clone();
        store
            .upsert_graph_edge(&edge)
            .map_err(|error| format!("failed to index RAG graph edge: {error}"))?;
    }
    Ok(())
}

pub(super) fn agent_scope_graph_symbol_public_id(
    symbol: &mut DebugGraphSymbol,
    source_index: &AgentSourceRagIndex,
) {
    if symbol.kind == "source_file" {
        return;
    }
    let Some(public_id) = &symbol.public_id else {
        return;
    };
    let source_public_id = public_id.as_str().to_owned();
    symbol.metadata.insert(
        "source_public_id".to_owned(),
        serde_json::json!(source_public_id),
    );
    if symbol.qualified_name.is_none() {
        symbol.qualified_name = Some(source_public_id.clone());
    }
    symbol.public_id = Some(
        AgentPublicId::new(format!(
            "{}.{}",
            source_index.source_key_prefix, source_public_id
        ))
        .expect("source-scoped graph public id is nonempty"),
    );
}

pub(super) fn agent_rag_index_program_graph(
    store: &DebugStore,
    program_hash: &StableHash,
    source_indexes: &[&AgentSourceRagIndex],
) -> Result<(), String> {
    let summary = agent_program_graph_summary(source_indexes);
    store
        .upsert_graph_symbol(&agent_program_graph_symbol(program_hash, &summary))
        .map_err(|error| format!("failed to index RAG program graph symbol: {error}"))?;
    for source_index in source_indexes {
        store
            .upsert_graph_edge(&agent_program_source_file_graph_edge(
                program_hash,
                source_index,
            ))
            .map_err(|error| format!("failed to index RAG program source-file edge: {error}"))?;
    }
    Ok(())
}

pub(super) fn agent_program_graph_summary(
    source_indexes: &[&AgentSourceRagIndex],
) -> AgentProgramGraphSummary {
    let symbol_kinds = source_indexes
        .iter()
        .map(|source_index| agent_graph_symbol_kind_counts(&source_index.graph_symbols))
        .fold(BTreeMap::new(), agent_merge_kind_counts);
    let edge_kinds = source_indexes
        .iter()
        .map(|source_index| agent_graph_edge_kind_counts(&source_index.graph_edges))
        .fold(BTreeMap::new(), agent_merge_kind_counts);
    AgentProgramGraphSummary {
        sources: source_indexes.len(),
        source_graph_symbols: source_indexes
            .iter()
            .map(|source_index| source_index.graph_symbols.len())
            .sum(),
        source_graph_edges: source_indexes
            .iter()
            .map(|source_index| source_index.graph_edges.len())
            .sum(),
        candidate_chunks: source_indexes
            .iter()
            .map(|source_index| source_index.candidates.len())
            .sum(),
        source_bytes: source_indexes
            .iter()
            .map(|source_index| source_index.source_file.byte_len)
            .sum(),
        dynamic_control_flows: source_indexes
            .iter()
            .flat_map(|source_index| source_index.graph_symbols.iter())
            .filter(|symbol| agent_graph_symbol_has_dynamic_control(symbol))
            .count(),
        symbol_kinds,
        edge_kinds,
    }
}

pub(super) fn agent_graph_symbol_kind_counts(
    symbols: &[DebugGraphSymbol],
) -> BTreeMap<String, usize> {
    symbols.iter().fold(BTreeMap::new(), |mut counts, symbol| {
        *counts.entry(symbol.kind.clone()).or_insert(0) += 1;
        counts
    })
}

pub(super) fn agent_graph_edge_kind_counts(edges: &[DebugGraphEdge]) -> BTreeMap<String, usize> {
    edges.iter().fold(BTreeMap::new(), |mut counts, edge| {
        *counts.entry(edge.edge_kind.clone()).or_insert(0) += 1;
        counts
    })
}

pub(super) fn agent_merge_kind_counts(
    mut left: BTreeMap<String, usize>,
    right: BTreeMap<String, usize>,
) -> BTreeMap<String, usize> {
    for (kind, count) in right {
        *left.entry(kind).or_insert(0) += count;
    }
    left
}

pub(super) fn agent_graph_symbol_has_dynamic_control(symbol: &DebugGraphSymbol) -> bool {
    symbol
        .metadata
        .get("flow_control")
        .and_then(|value| value.get("has_dynamic_control"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn agent_rag_context_read_report(
    options: &AgentRagContextReadOptions,
    query_id: &str,
) -> Result<AgentRagContextReadReport, String> {
    let audit = agent_rag_persisted_audit(&options.debug_db, query_id, options.max_privacy)?;
    let mut item = audit
        .pack
        .items
        .into_iter()
        .find(|item| item.chunk_id.as_str() == options.chunk_id.trim())
        .ok_or_else(|| {
            format!(
                "could not find chunk id `{}` in persisted RAG query `{query_id}`",
                options.chunk_id.trim()
            )
        })?;
    let (body, body_truncated) = truncate_utf8(&item.body, options.max_bytes);
    item.body = body;
    Ok(AgentRagContextReadReport {
        path: options.debug_db.display().to_string(),
        query_id: query_id.to_owned(),
        chunk_id: item.chunk_id.clone(),
        max_privacy: options.max_privacy,
        max_bytes: options.max_bytes,
        body_truncated,
        item,
    })
}

pub(super) fn agent_rag_persisted_audit(
    path: &Path,
    query_id: &str,
    max_privacy: PrivacyClass,
) -> Result<arcweft_debug_sqlite::store::DebugRagQueryAudit, String> {
    let store = DebugStore::open(path).map_err(|error| {
        format!(
            "failed to open persisted RAG debug DB `{}`: {error}",
            path.display()
        )
    })?;
    store
        .rag_query_audit_with_max_privacy(query_id, max_privacy)
        .map_err(|error| {
            format!(
                "failed to read persisted RAG query `{query_id}` from `{}`: {error}",
                path.display()
            )
        })
}

pub(super) fn agent_rag_explain_item_report(item: RagContextItem) -> AgentRagExplainItemReport {
    AgentRagExplainItemReport {
        chunk_id: item.chunk_id,
        kind: item.kind,
        title: item.title,
        fused_score: item.fused_score,
        channels: item.channels,
        entity_ids: item.entity_ids,
        source_anchor: item.source_anchor,
    }
}

pub(super) fn agent_rag_query_input_label(options: &AgentRagQueryOptions) -> String {
    match (&options.trace, options.source.as_slice()) {
        (Some(trace), []) => trace.display().to_string(),
        (None, [source]) => source.display().to_string(),
        (Some(trace), [source]) => {
            format!("trace {} + source {}", trace.display(), source.display())
        }
        (Some(trace), sources) => {
            format!("trace {} + {} sources", trace.display(), sources.len())
        }
        (None, [_source, rest @ ..]) => format!("{} sources", rest.len().saturating_add(1)),
        (None, []) => "agent rag query".to_owned(),
    }
}

#[derive(Clone)]
pub(super) struct AgentRagCandidate {
    pub(super) chunk: DebugChunk,
    pub(super) preferred_channel: SearchChannel,
}

pub(super) struct AgentRagQueryResult {
    pub(super) pack: RagContextPack,
    pub(super) candidates: Vec<AgentRagCandidate>,
    pub(super) source_indexes: Vec<AgentSourceRagIndex>,
}

#[derive(serde::Serialize)]
pub(super) struct AgentRagIndexReport {
    pub(super) path: String,
    pub(super) session_id: String,
    pub(super) changed_only: bool,
    pub(super) program_hash: String,
    pub(super) sources: Vec<AgentRagIndexedSourceReport>,
    pub(super) indexed_sources: usize,
    pub(super) skipped_unchanged_sources: usize,
    pub(super) indexed_chunks: usize,
    pub(super) skipped_unchanged_chunks: usize,
}

#[derive(serde::Serialize)]
pub(super) struct AgentRagIndexedSourceReport {
    pub(super) path: String,
    pub(super) source_hash: String,
    pub(super) candidate_chunks: usize,
    pub(super) indexed_chunks: usize,
    pub(super) skipped_unchanged_chunks: usize,
    pub(super) source_file_recorded: bool,
    pub(super) indexed: bool,
}

pub(super) fn agent_rag_index_session_id(
    program_hash: &StableHash,
    changed_only: bool,
) -> Result<SessionId, String> {
    let suffix = agent_content_hash(format!(
        "cli:index:{}:{changed_only}",
        program_hash.as_str()
    ))
    .replace(':', ".");
    SessionId::new(format!("session.rag.index.cli.{suffix}"))
        .map_err(|error| format!("failed to build RAG index session id: {error}"))
}

pub(super) fn agent_rag_index_session_metadata(
    options: &AgentRagIndexOptions,
    sources: &[AgentRagIndexedSourceReport],
    indexed_sources: usize,
    skipped_unchanged_sources: usize,
    indexed_chunks: usize,
    skipped_unchanged_chunks: usize,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("operation".to_owned(), serde_json::json!("index")),
        (
            "changed_only".to_owned(),
            serde_json::json!(options.changed),
        ),
        (
            "indexed_sources".to_owned(),
            serde_json::json!(indexed_sources),
        ),
        (
            "skipped_unchanged_sources".to_owned(),
            serde_json::json!(skipped_unchanged_sources),
        ),
        (
            "indexed_chunks".to_owned(),
            serde_json::json!(indexed_chunks),
        ),
        (
            "skipped_unchanged_chunks".to_owned(),
            serde_json::json!(skipped_unchanged_chunks),
        ),
        (
            "sources".to_owned(),
            serde_json::json!(
                sources
                    .iter()
                    .map(|source| {
                        serde_json::json!({
                            "path": source.path,
                            "source_hash": source.source_hash,
                            "candidate_chunks": source.candidate_chunks,
                            "indexed_chunks": source.indexed_chunks,
                            "skipped_unchanged_chunks": source.skipped_unchanged_chunks,
                            "source_file_recorded": source.source_file_recorded,
                            "indexed": source.indexed,
                        })
                    })
                    .collect::<Vec<_>>()
            ),
        ),
    ])
}

#[derive(serde::Serialize)]
pub(super) struct AgentRagExplainReport {
    pub(super) path: String,
    pub(super) query_id: String,
    pub(super) session_id: Option<String>,
    pub(super) run_id: Option<String>,
    pub(super) max_privacy: PrivacyClass,
    pub(super) status: String,
    pub(super) created_unix_ms: i64,
    pub(super) query: RagQuery,
    pub(super) item_count: usize,
    pub(super) truncated: bool,
    pub(super) items: Vec<AgentRagExplainItemReport>,
}

#[derive(serde::Serialize)]
pub(super) struct AgentRagExplainItemReport {
    pub(super) chunk_id: ChunkId,
    pub(super) kind: ChunkSourceKind,
    pub(super) title: String,
    pub(super) fused_score: f64,
    pub(super) channels: BTreeSet<SearchChannel>,
    pub(super) entity_ids: Vec<AgentPublicId>,
    pub(super) source_anchor: Option<DebugSourceAnchor>,
}

#[derive(serde::Serialize)]
pub(super) struct AgentRagContextReadReport {
    pub(super) path: String,
    pub(super) query_id: String,
    pub(super) chunk_id: ChunkId,
    pub(super) max_privacy: PrivacyClass,
    pub(super) max_bytes: usize,
    pub(super) body_truncated: bool,
    pub(super) item: RagContextItem,
}

pub(super) fn agent_rag_query_result(
    options: &AgentRagQueryOptions,
) -> Result<AgentRagQueryResult, String> {
    let query_text = options.query.trim();
    if options.trace.is_none() && options.source.is_empty() && options.debug_db.is_none() {
        return Err(
            "agent rag query requires --trace, --source, --debug-db, or a combination of them"
                .to_owned(),
        );
    }
    if query_text.is_empty() {
        return Err("agent rag query requires a non-empty --query".to_owned());
    }
    if options.limit == 0 {
        return Err("agent rag query --limit must be at least 1".to_owned());
    }
    if options.max_context_bytes == 0 {
        return Err("agent rag query --max-context-bytes must be at least 1".to_owned());
    }
    if options.local_embedding && options.debug_db.is_none() {
        return Err("agent rag query --local-embedding requires --debug-db".to_owned());
    }
    if options.local_embedding {
        agent_rag_local_embedding_model(options)?;
    }
    let roots = agent_rag_roots(&options.roots)?;
    let mut candidates = Vec::new();
    let mut seed_parts = Vec::new();
    let mut source_indexes = Vec::new();
    if let Some(trace) = &options.trace {
        let records = read_and_validate_agent_trace_records(trace)?;
        let trace_report = validate_agent_trace(trace, &records, None)?;
        seed_parts.push(agent_trace_rag_seed(trace, &records));
        candidates.extend(agent_trace_rag_candidates(&trace_report, &records)?);
    }
    let source_paths = agent_rag_source_paths(&options.source)?;
    for source in &source_paths {
        let source_index = agent_source_rag_index(source)?;
        seed_parts.push(source_index.seed.clone());
        candidates.extend(source_index.candidates.clone());
        source_indexes.push(source_index);
    }
    if let Some(debug_db) = &options.debug_db {
        let debug_candidates = agent_rag_debug_db_candidates(debug_db, options)?;
        seed_parts.extend(agent_rag_debug_db_seed_parts(debug_db, &debug_candidates));
        candidates.extend(debug_candidates);
    }
    let program_hash = agent_rag_program_hash(&seed_parts)?;
    if !source_indexes.is_empty() {
        let source_index_refs = source_indexes.iter().collect::<Vec<_>>();
        candidates.push(agent_program_summary_rag_candidate(
            &program_hash,
            &source_index_refs,
        )?);
    }
    let candidates = agent_rag_deduplicate_candidates(candidates);
    let query_candidates = agent_rag_query_allowed_candidates(options, &candidates);
    let query = RagQuery {
        query_id: agent_content_hash(format!(
            "{}:{}:{}:{}:{}:{}",
            query_text,
            program_hash.as_str(),
            options.graph_depth,
            options.limit,
            options.max_context_bytes,
            options.max_privacy.as_str()
        )),
        text: query_text.to_owned(),
        program_hash,
        roots,
        graph_depth: options.graph_depth,
        limit: options.limit,
        max_context_bytes: options.max_context_bytes,
    };
    let pack = agent_trace_rag_pack_from_candidates(options, query, &query_candidates);
    Ok(AgentRagQueryResult {
        pack,
        candidates,
        source_indexes,
    })
}

pub(super) fn agent_rag_debug_db_candidates(
    path: &Path,
    options: &AgentRagQueryOptions,
) -> Result<Vec<AgentRagCandidate>, String> {
    let store = DebugStore::open(path)
        .map_err(|error| format!("agent rag query failed to open debug DB: {error}"))?;
    let search_limit = options.limit.saturating_mul(8).max(32);
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    if options.local_embedding {
        let vector_results =
            agent_rag_debug_db_vector_results(&store, path, options, search_limit)?;
        if vector_results.is_empty() {
            agent_rag_record_local_embedding_fallback(&store, path, options)?;
        }
        for result in vector_results {
            if seen.insert(result.chunk.id.clone()) {
                candidates.push(AgentRagCandidate {
                    chunk: result.chunk,
                    preferred_channel: result.hit.channel,
                });
            }
        }
    }
    let terms = std::iter::once(options.query.trim().to_owned())
        .chain(options.roots.iter().map(|root| root.trim().to_owned()))
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    for term in terms {
        let mut results = store
            .lexical_chunk_search_with_max_privacy(&term, search_limit, options.max_privacy)
            .map_err(|error| agent_rag_debug_db_search_error(path, &error))?;
        results.extend(
            store
                .graph_search_with_depth_and_max_privacy(
                    &term,
                    options.graph_depth,
                    search_limit,
                    options.max_privacy,
                )
                .map_err(|error| agent_rag_debug_db_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_rag_candidate_from_search_result(result, "graph"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        results.extend(
            store
                .history_search_with_max_privacy(&term, search_limit, options.max_privacy)
                .map_err(|error| agent_rag_debug_db_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_rag_candidate_from_search_result(result, "history"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        results.extend(
            store
                .diagnostic_search_with_max_privacy(&term, search_limit, options.max_privacy)
                .map_err(|error| agent_rag_debug_db_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_rag_candidate_from_search_result(result, "diagnostic"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        results.extend(
            store
                .test_result_search_with_max_privacy(&term, search_limit, options.max_privacy)
                .map_err(|error| agent_rag_debug_db_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_rag_candidate_from_search_result(result, "test"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        for result in results {
            if seen.insert(result.chunk.id.clone()) {
                candidates.push(AgentRagCandidate {
                    chunk: result.chunk,
                    preferred_channel: result.hit.channel,
                });
            }
        }
    }
    Ok(candidates)
}

pub(super) fn agent_rag_record_local_embedding_fallback(
    store: &DebugStore,
    path: &Path,
    options: &AgentRagQueryOptions,
) -> Result<(), String> {
    let model = agent_rag_local_embedding_model(options)?;
    let query = options.query.trim();
    let diagnostic_id = format!(
        "agent-rag-local-embedding-fallback:{}",
        agent_content_hash(format!(
            "{}:{}:{}:{}",
            path.display(),
            query,
            model.model_id,
            model.model_revision
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
            source_path: Some(path.display().to_string()),
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
                "agent rag query failed to record local embedding fallback diagnostic in `{}`: {error}",
                path.display()
            )
        })
}

pub(super) fn agent_rag_debug_db_vector_results(
    store: &DebugStore,
    path: &Path,
    options: &AgentRagQueryOptions,
    search_limit: usize,
) -> Result<Vec<arcweft_debug_sqlite::store::DebugChunkSearchResult>, String> {
    let model = agent_rag_local_embedding_model(options)?;
    let query_vector = local_hash_query_embedding(options.query.trim(), model.dimensions);
    store
        .vector_search_with_max_privacy(&model, &query_vector, search_limit, options.max_privacy)
        .map_err(|error| agent_rag_debug_db_search_error(path, &error))?
        .into_iter()
        .map(|result| agent_rag_candidate_from_search_result(result, "vector"))
        .collect()
}

pub(super) fn agent_rag_local_embedding_model(
    options: &AgentRagQueryOptions,
) -> Result<EmbeddingModelDescriptor, String> {
    let model_id = options.local_embedding_model_id.trim();
    if model_id.is_empty() {
        return Err("agent rag query --local-embedding-model-id must not be empty".to_owned());
    }
    let model_revision = options.local_embedding_model_revision.trim();
    if model_revision.is_empty() {
        return Err(
            "agent rag query --local-embedding-model-revision must not be empty".to_owned(),
        );
    }
    if options.local_embedding_dimensions == 0 {
        return Err("agent rag query --local-embedding-dimensions must be at least 1".to_owned());
    }
    if options.local_embedding_dimensions > MAX_LOCAL_EMBEDDING_DIMENSIONS {
        return Err(format!(
            "agent rag query --local-embedding-dimensions must be at most {MAX_LOCAL_EMBEDDING_DIMENSIONS}"
        ));
    }
    Ok(EmbeddingModelDescriptor {
        model_id: model_id.to_owned(),
        model_revision: model_revision.to_owned(),
        dimensions: options.local_embedding_dimensions,
    })
}

pub(super) fn agent_rag_debug_db_search_error(
    path: &Path,
    error: &arcweft_debug_sqlite::store::DebugStoreError,
) -> String {
    format!(
        "agent rag query failed to search debug DB `{}`: {error}",
        path.display()
    )
}

pub(super) fn agent_rag_candidate_from_search_result(
    result: arcweft_debug_sqlite::store::ChunkSearchResult,
    source_prefix: &str,
) -> Result<arcweft_debug_sqlite::store::DebugChunkSearchResult, String> {
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
                "agent rag query debug DB result has unsupported source_kind `{other}`"
            ));
        }
    };
    let content_hash = agent_content_hash(&result.body);
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "search_channel".to_owned(),
        serde_json::json!(search_channel_label(result.hit.channel)),
    );
    metadata.insert(
        "search_score".to_owned(),
        serde_json::to_value(result.hit.score).map_err(|error| {
            format!("agent rag query failed to serialize debug DB search score: {error}")
        })?,
    );
    Ok(arcweft_debug_sqlite::store::DebugChunkSearchResult {
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

pub(super) fn agent_rag_debug_db_seed_parts(
    path: &Path,
    candidates: &[AgentRagCandidate],
) -> Vec<String> {
    if candidates.is_empty() {
        return vec![format!(
            "debug-db-empty:{}",
            agent_content_hash(path.display().to_string())
        )];
    }
    let mut seed_parts = candidates
        .iter()
        .map(|candidate| {
            format!(
                "debug-db:{}:{}",
                candidate.chunk.id.as_str(),
                candidate.chunk.content_hash.as_str()
            )
        })
        .collect::<Vec<_>>();
    seed_parts.sort();
    seed_parts
}

pub(super) fn agent_rag_deduplicate_candidates(
    candidates: Vec<AgentRagCandidate>,
) -> Vec<AgentRagCandidate> {
    let mut seen = BTreeSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.chunk.id.clone()))
        .collect()
}

pub(super) fn agent_trace_rag_pack_from_candidates(
    options: &AgentRagQueryOptions,
    query: RagQuery,
    candidates: &[AgentRagCandidate],
) -> RagContextPack {
    let fused = reciprocal_rank_fusion(
        &agent_trace_rag_ranked_lists(candidates, &query),
        &FusionConfig::default(),
        candidates.len(),
    );
    let by_id = candidates
        .iter()
        .map(|candidate| (candidate.chunk.id.clone(), &candidate.chunk))
        .collect::<BTreeMap<_, _>>();
    let mut items = Vec::new();
    let mut used_bytes = 0usize;
    let mut truncated = false;
    let mut selected_semantic_hashes = BTreeSet::new();
    let mut selected_source_anchors = Vec::new();
    for hit in fused {
        if items.len() >= options.limit {
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
        let remaining = options.max_context_bytes.saturating_sub(used_bytes);
        if remaining == 0 {
            truncated = true;
            break;
        }
        let (body, body_truncated) = truncate_utf8(&chunk.body, remaining);
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

pub(in crate::app) fn agent_rag_select_context_chunk(
    chunk: &DebugChunk,
    selected_semantic_hashes: &mut BTreeSet<StableHash>,
    selected_source_anchors: &mut Vec<DebugSourceAnchor>,
) -> bool {
    if chunk
        .semantic_hash
        .as_ref()
        .is_some_and(|hash| selected_semantic_hashes.contains(hash))
    {
        return false;
    }
    if chunk.source_anchor.as_ref().is_some_and(|anchor| {
        selected_source_anchors
            .iter()
            .any(|selected| agent_rag_source_anchors_overlap(selected, anchor))
    }) {
        return false;
    }

    if let Some(hash) = &chunk.semantic_hash {
        selected_semantic_hashes.insert(hash.clone());
    }
    if let Some(anchor) = &chunk.source_anchor {
        selected_source_anchors.push(anchor.clone());
    }
    true
}

pub(super) fn agent_rag_source_anchors_overlap(
    left: &DebugSourceAnchor,
    right: &DebugSourceAnchor,
) -> bool {
    left.path == right.path
        && ((left.start_byte == right.start_byte && left.end_byte == right.end_byte)
            || (left.start_byte < right.end_byte && right.start_byte < left.end_byte))
}

pub(super) fn agent_rag_query_allowed_candidates(
    options: &AgentRagQueryOptions,
    candidates: &[AgentRagCandidate],
) -> Vec<AgentRagCandidate> {
    candidates
        .iter()
        .filter(|candidate| candidate.chunk.privacy.is_allowed_by(options.max_privacy))
        .cloned()
        .collect()
}

pub(super) fn persist_agent_rag_query_result(
    path: &Path,
    result: &AgentRagQueryResult,
) -> Result<(), String> {
    let store = DebugStore::open(path)
        .map_err(|error| format!("agent rag query failed to open debug DB: {error}"))?;
    store
        .upsert_program(&result.pack.query.program_hash, None, None, 0)
        .map_err(|error| format!("agent rag query failed to index RAG program: {error}"))?;
    let session_id = agent_rag_query_session_id("cli", &result.pack.query.query_id)?;
    store
        .upsert_session(&DebugSession {
            session_id: session_id.clone(),
            program_hash: Some(result.pack.query.program_hash.clone()),
            profile: "rag".to_owned(),
            transport: "cli".to_owned(),
            started_unix_ms: 0,
            ended_unix_ms: Some(0),
            status: DebugSessionStatus::Finished,
            metadata: agent_rag_query_session_metadata(result),
        })
        .map_err(|error| format!("agent rag query failed to record RAG session: {error}"))?;
    for candidate in &result.candidates {
        let mut chunk = candidate.chunk.clone();
        chunk.program_hash = Some(result.pack.query.program_hash.clone());
        store
            .upsert_chunk(&chunk)
            .map_err(|error| format!("agent rag query failed to index RAG chunk: {error}"))?;
    }
    if !result.source_indexes.is_empty() {
        for source_index in &result.source_indexes {
            let mut source_file = source_index.source_file.clone();
            source_file.program_hash = result.pack.query.program_hash.clone();
            store
                .upsert_source_file(&source_file)
                .map_err(|error| format!("agent rag query failed to index source file: {error}"))?;
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
        .map_err(|error| format!("agent rag query failed to record RAG audit: {error}"))
}

pub(super) fn agent_rag_query_session_id(
    transport: &str,
    query_id: &str,
) -> Result<SessionId, String> {
    let suffix = agent_content_hash(format!("{transport}:{query_id}")).replace(':', ".");
    SessionId::new(format!("session.rag.{transport}.{suffix}"))
        .map_err(|error| format!("failed to build RAG session id: {error}"))
}

pub(super) fn agent_rag_query_session_metadata(
    result: &AgentRagQueryResult,
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
                    .map(AgentPublicId::as_str)
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

pub(super) fn agent_trace_rag_candidates(
    trace_report: &AgentScriptTraceReport,
    records: &[AgentTraceRecord],
) -> Result<Vec<AgentRagCandidate>, String> {
    let mut candidates = Vec::with_capacity(records.len() + 1);
    candidates.push(agent_trace_rag_json_candidate(
        "trace.summary",
        "Agent trace summary",
        ChunkSourceKind::GraphSummary,
        SearchChannel::Summary,
        &serde_json::to_value(trace_report)
            .map_err(|error| format!("failed to serialize trace summary: {error}"))?,
        Vec::new(),
        PrivacyClass::Project,
    )?);
    candidates.extend(
        records
            .iter()
            .map(|record| {
                agent_trace_rag_json_candidate(
                    &format!("trace.record.{}", record.sequence),
                    &format!(
                        "Trace record {} {}",
                        record.sequence,
                        agent_trace_kind_name(record.kind)
                    ),
                    ChunkSourceKind::AgentTrace,
                    SearchChannel::Trace,
                    &serde_json::to_value(record)
                        .map_err(|error| format!("failed to serialize trace record: {error}"))?,
                    agent_trace_record_entity_ids(record),
                    agent_trace_record_privacy(record),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(candidates)
}

pub(super) fn agent_trace_rag_json_candidate(
    source_key: &str,
    title: &str,
    source_kind: ChunkSourceKind,
    preferred_channel: SearchChannel,
    value: &serde_json::Value,
    entity_ids: Vec<AgentPublicId>,
    privacy: PrivacyClass,
) -> Result<AgentRagCandidate, String> {
    let body = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize RAG candidate body: {error}"))?;
    Ok(agent_rag_candidate(
        source_key,
        title,
        source_kind,
        preferred_channel,
        body,
        AgentRagCandidateMeta {
            entity_ids,
            privacy,
            source_anchor: None,
            semantic_hash: None,
            metadata: BTreeMap::new(),
        },
    ))
}
