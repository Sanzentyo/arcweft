use super::{
    AgentPublicId, AgentRagCandidate, AgentTraceRecord, BTreeMap, BTreeSet, ChunkId,
    ChunkSourceKind, DebugChunk, DebugGraphEdge, DebugGraphSymbol, DebugSourceAnchor,
    DebugSourceFile, EntitySymbol, Path, PathBuf, PrivacyClass, ProjectCallableSymbol,
    ProjectSemanticIndex, RagQuery, SearchChannel, SearchHit, SemaPublicId, SessionId,
    SourceAnchor, StableHash, agent_graph_edge_kind_counts, agent_graph_symbol_has_dynamic_control,
    agent_graph_symbol_kind_counts, agent_program_graph_summary, agent_trace_kind_name, fs,
};
use arcweft_lang_hir::symbol::CallableDeclarationKey;
use arcweft_lang_sema::callable::{
    CallableCandidateId, CheckedCallableFacts, CheckedCallableLookupError,
};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::project_index::ProjectEntityId;
use arcweft_source::SourceSpan;

pub(in crate::app::agent) struct AgentSourceRagIndex {
    pub(in crate::app::agent) seed: String,
    pub(in crate::app::agent) source_hash: String,
    pub(in crate::app::agent) source_key_prefix: String,
    pub(in crate::app::agent) source_file: DebugSourceFile,
    pub(in crate::app::agent) candidates: Vec<AgentRagCandidate>,
    pub(in crate::app::agent) graph_symbols: Vec<DebugGraphSymbol>,
    pub(in crate::app::agent) graph_edges: Vec<DebugGraphEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app::agent) struct AgentProgramGraphSummary {
    pub(in crate::app::agent) sources: usize,
    pub(in crate::app::agent) source_graph_symbols: usize,
    pub(in crate::app::agent) source_graph_edges: usize,
    pub(in crate::app::agent) candidate_chunks: usize,
    pub(in crate::app::agent) source_bytes: u64,
    pub(in crate::app::agent) dynamic_control_flows: usize,
    pub(in crate::app::agent) symbol_kinds: BTreeMap<String, usize>,
    pub(in crate::app::agent) edge_kinds: BTreeMap<String, usize>,
}

pub(in crate::app::agent) fn agent_rag_source_paths(
    inputs: &[PathBuf],
) -> Result<Vec<PathBuf>, String> {
    let mut files = BTreeSet::new();
    for input in inputs {
        if input.is_dir() {
            for path in agent_rag_arcw_files_in_dir(input)? {
                files.insert(path);
            }
        } else {
            files.insert(input.clone());
        }
    }
    Ok(files.into_iter().collect())
}

pub(in crate::app::agent) fn agent_rag_arcw_files_in_dir(
    root: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir).map_err(|error| {
            format!("agent rag query failed to read {}: {error}", dir.display())
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "agent rag query failed to read entry under {}: {error}",
                    dir.display()
                )
            })?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if is_arcw_path(&path) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(in crate::app::agent) fn is_arcw_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("arcw"))
}

pub(in crate::app::agent) fn agent_source_rag_index(
    path: &Path,
) -> Result<AgentSourceRagIndex, String> {
    let checked =
        crate::app::project::load_and_check_with_env(path, &TypeCheckEnv::standard(), Vec::new())
            .map_err(|code| format!("agent rag query failed to compile source: {code:?}"))?;
    let source = checked.source_document.text();
    let source_hash = agent_content_hash(source);
    let project = checked.compiled.semantic_index();
    let source_file = DebugSourceFile {
        program_hash: StableHash::new(project.program_hash().as_str())
            .map_err(|error| format!("invalid source file program hash: {error}"))?,
        path: path.display().to_string(),
        language: "arcw".to_owned(),
        content_hash: StableHash::new(source_hash.clone())
            .map_err(|error| format!("invalid source file content hash: {error}"))?,
        byte_len: u64::try_from(source.len())
            .map_err(|_| "source file byte length overflowed u64".to_owned())?,
        metadata: BTreeMap::from([("extension".to_owned(), serde_json::json!("arcw"))]),
    };
    let source_key_prefix = agent_source_rag_key_prefix(path);
    let mut candidates = agent_source_text_rag_candidates(path, source, &source_key_prefix)?;
    candidates.extend(agent_project_semantic_rag_candidates(
        path,
        project,
        &source_key_prefix,
    )?);
    let mut graph_symbols = agent_project_graph_symbols(project, &source_key_prefix)?;
    graph_symbols.push(agent_source_file_graph_symbol(
        &source_file,
        &source_key_prefix,
    ));
    agent_attach_source_file_to_graph_symbols(&mut graph_symbols, &source_file);
    let mut graph_edges = agent_project_graph_edges(project, &source_key_prefix)?;
    graph_edges.push(agent_source_file_project_graph_edge(
        &source_file,
        &source_key_prefix,
    ));
    Ok(AgentSourceRagIndex {
        seed: format!("source:{}:{source_hash}", path.display()),
        source_hash,
        source_key_prefix,
        source_file,
        candidates,
        graph_symbols,
        graph_edges,
    })
}

pub(in crate::app::agent) fn agent_source_file_graph_symbol(
    source_file: &DebugSourceFile,
    source_key_prefix: &str,
) -> DebugGraphSymbol {
    DebugGraphSymbol {
        symbol_id: agent_source_file_graph_symbol_id(source_key_prefix),
        program_hash: source_file.program_hash.clone(),
        public_id: Some(
            AgentPublicId::new(format!("source_file.{}", source_file.content_hash.as_str()))
                .expect("source-file graph public id is nonempty"),
        ),
        qualified_name: Some(source_file.path.clone()),
        kind: "source_file".to_owned(),
        type_json: Some(serde_json::json!({
            "language": source_file.language,
            "byte_len": source_file.byte_len,
        })),
        source_path: Some(source_file.path.clone()),
        source_content_hash: Some(source_file.content_hash.clone()),
        start_byte: Some(0),
        end_byte: Some(source_file.byte_len),
        semantic_hash: Some(source_file.content_hash.clone()),
        summary: format!(
            "Source file `{}` language={} bytes={} hash={}",
            source_file.path,
            source_file.language,
            source_file.byte_len,
            source_file.content_hash.as_str()
        ),
        metadata: BTreeMap::from([
            (
                "language".to_owned(),
                serde_json::json!(source_file.language),
            ),
            (
                "content_hash".to_owned(),
                serde_json::json!(source_file.content_hash.as_str()),
            ),
            (
                "byte_len".to_owned(),
                serde_json::json!(source_file.byte_len),
            ),
        ]),
    }
}

pub(in crate::app::agent) fn agent_source_file_project_graph_edge(
    source_file: &DebugSourceFile,
    source_key_prefix: &str,
) -> DebugGraphEdge {
    DebugGraphEdge {
        program_hash: source_file.program_hash.clone(),
        from_symbol_id: agent_source_file_graph_symbol_id(source_key_prefix),
        to_symbol_id: agent_project_summary_graph_symbol_id(source_key_prefix),
        edge_kind: "contains_project_graph".to_owned(),
        weight: 1.0,
        metadata: BTreeMap::from([
            (
                "source_path".to_owned(),
                serde_json::json!(source_file.path),
            ),
            (
                "source_content_hash".to_owned(),
                serde_json::json!(source_file.content_hash.as_str()),
            ),
        ]),
    }
}

pub(in crate::app::agent) fn agent_program_graph_symbol(
    program_hash: &StableHash,
    summary: &AgentProgramGraphSummary,
) -> DebugGraphSymbol {
    DebugGraphSymbol {
        symbol_id: agent_program_graph_symbol_id(program_hash),
        program_hash: program_hash.clone(),
        public_id: Some(
            AgentPublicId::new(format!("program.{}", program_hash.as_str()))
                .expect("program graph public id is nonempty"),
        ),
        qualified_name: Some(format!("program.{}", program_hash.as_str())),
        kind: "program".to_owned(),
        type_json: Some(serde_json::json!({
            "source_count": summary.sources,
            "source_graph_symbol_count": summary.source_graph_symbols,
            "source_graph_edge_count": summary.source_graph_edges,
            "candidate_chunk_count": summary.candidate_chunks,
            "source_byte_count": summary.source_bytes,
            "dynamic_control_flow_count": summary.dynamic_control_flows,
            "source_graph_symbol_kinds": summary.symbol_kinds,
            "source_graph_edge_kinds": summary.edge_kinds,
        })),
        source_path: None,
        source_content_hash: None,
        start_byte: None,
        end_byte: None,
        semantic_hash: Some(program_hash.clone()),
        summary: format!(
            "Program `{}` with {} indexed source files, {} candidate chunks, {} graph symbols, and {} graph edges",
            program_hash.as_str(),
            summary.sources,
            summary.candidate_chunks,
            summary.source_graph_symbols,
            summary.source_graph_edges
        ),
        metadata: BTreeMap::from([
            (
                "source_count".to_owned(),
                serde_json::json!(summary.sources),
            ),
            (
                "source_graph_symbol_count".to_owned(),
                serde_json::json!(summary.source_graph_symbols),
            ),
            (
                "source_graph_edge_count".to_owned(),
                serde_json::json!(summary.source_graph_edges),
            ),
            (
                "candidate_chunk_count".to_owned(),
                serde_json::json!(summary.candidate_chunks),
            ),
            (
                "source_byte_count".to_owned(),
                serde_json::json!(summary.source_bytes),
            ),
            (
                "dynamic_control_flow_count".to_owned(),
                serde_json::json!(summary.dynamic_control_flows),
            ),
            (
                "source_graph_symbol_kinds".to_owned(),
                serde_json::json!(summary.symbol_kinds),
            ),
            (
                "source_graph_edge_kinds".to_owned(),
                serde_json::json!(summary.edge_kinds),
            ),
        ]),
    }
}

pub(in crate::app::agent) fn agent_program_source_file_graph_edge(
    program_hash: &StableHash,
    source_index: &AgentSourceRagIndex,
) -> DebugGraphEdge {
    DebugGraphEdge {
        program_hash: program_hash.clone(),
        from_symbol_id: agent_program_graph_symbol_id(program_hash),
        to_symbol_id: agent_source_file_graph_symbol_id(&source_index.source_key_prefix),
        edge_kind: "contains_source_file".to_owned(),
        weight: 1.0,
        metadata: BTreeMap::from([
            (
                "source_path".to_owned(),
                serde_json::json!(source_index.source_file.path),
            ),
            (
                "source_content_hash".to_owned(),
                serde_json::json!(source_index.source_file.content_hash.as_str()),
            ),
        ]),
    }
}

pub(in crate::app::agent) fn agent_attach_source_file_to_graph_symbols(
    symbols: &mut [DebugGraphSymbol],
    source_file: &DebugSourceFile,
) {
    for symbol in symbols {
        symbol.source_path = Some(source_file.path.clone());
        symbol.source_content_hash = Some(source_file.content_hash.clone());
    }
}

pub(in crate::app::agent) fn agent_source_rag_key_prefix(path: &Path) -> String {
    format!("source.{}", agent_content_hash(path.display().to_string()))
}

pub(in crate::app::agent) fn agent_source_text_rag_candidates(
    path: &Path,
    source: &str,
    source_key_prefix: &str,
) -> Result<Vec<AgentRagCandidate>, String> {
    agent_source_text_ranges(source)
        .into_iter()
        .enumerate()
        .map(|(index, range)| {
            let body = source[range.clone()].trim().to_owned();
            let source_key = format!("{source_key_prefix}.text.{index}");
            let mut metadata = BTreeMap::new();
            metadata.insert(
                "path".to_owned(),
                serde_json::Value::String(path.display().to_string()),
            );
            Ok(agent_rag_candidate(
                &source_key,
                &format!("Source text {}", path.display()),
                ChunkSourceKind::Source,
                SearchChannel::Lexical,
                body,
                AgentRagCandidateMeta {
                    entity_ids: Vec::new(),
                    privacy: PrivacyClass::Project,
                    source_anchor: Some(debug_source_anchor(path, range)?),
                    semantic_hash: None,
                    metadata,
                },
            ))
        })
        .collect()
}

pub(in crate::app::agent) fn agent_source_text_ranges(source: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        let line_start = offset;
        let line_end = offset.saturating_add(line.len());
        if line.trim().is_empty() {
            if let Some(start) = start.take()
                && start < line_start
            {
                ranges.push(agent_trim_source_range(source, start..line_start));
            }
        } else if start.is_none() {
            start = Some(line_start);
        }
        offset = line_end;
    }
    if let Some(start) = start
        && start < source.len()
    {
        ranges.push(agent_trim_source_range(source, start..source.len()));
    }
    if ranges.is_empty() && !source.is_empty() {
        ranges.push(agent_trim_source_range(source, 0..source.len()));
    }
    ranges
        .into_iter()
        .filter(|range| range.start < range.end)
        .collect()
}

pub(in crate::app::agent) fn agent_trim_source_range(
    source: &str,
    range: std::ops::Range<usize>,
) -> std::ops::Range<usize> {
    let mut start = range.start;
    let mut end = range.end;
    while start < end {
        let Some(character) = source[start..end].chars().next() else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        start = start.saturating_add(character.len_utf8());
    }
    while start < end {
        let Some(character) = source[start..end].chars().next_back() else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        end = end.saturating_sub(character.len_utf8());
    }
    start..end
}

pub(in crate::app::agent) fn agent_project_semantic_rag_candidates(
    path: &Path,
    project: &ProjectSemanticIndex,
    source_key_prefix: &str,
) -> Result<Vec<AgentRagCandidate>, String> {
    let mut candidates = Vec::new();
    candidates.push(agent_project_summary_rag_candidate(
        path,
        project,
        source_key_prefix,
    )?);
    for entity in project.entities().values() {
        candidates.push(agent_project_entity_rag_candidate(
            entity,
            source_key_prefix,
            project,
        )?);
    }
    for (declaration, callable) in project.project_callables() {
        candidates.push(agent_project_callable_rag_candidate(
            project,
            declaration,
            callable,
            source_key_prefix,
        )?);
    }
    for (name, query) in project.debug_queries() {
        candidates.push(agent_project_debug_query_rag_candidate(
            name,
            query,
            source_key_prefix,
        )?);
    }
    Ok(candidates)
}

pub(in crate::app::agent) fn agent_project_graph_symbols(
    project: &ProjectSemanticIndex,
    _source_key_prefix: &str,
) -> Result<Vec<DebugGraphSymbol>, String> {
    let program_hash = StableHash::new(project.program_hash().as_str())
        .map_err(|error| format!("invalid project graph program hash: {error}"))?;
    arcweft_compiler::agent_project::agent_project_graph_from_project(project)
        .map_err(|error| error.to_string())?
        .symbols
        .into_iter()
        .map(|symbol| {
            let semantic_hash = symbol
                .semantic_hash
                .map(StableHash::new)
                .transpose()
                .map_err(|error| format!("invalid project graph semantic hash: {error}"))?;
            let mut metadata = BTreeMap::new();
            if let Some(flow_control) = symbol.flow_control {
                metadata.insert(
                    "flow_control".to_owned(),
                    serde_json::to_value(flow_control)
                        .map_err(|error| format!("failed to project flow control: {error}"))?,
                );
            }
            if let Some(project_summary) = symbol.project_summary {
                metadata.insert(
                    "project_summary".to_owned(),
                    serde_json::to_value(project_summary)
                        .map_err(|error| format!("failed to project project summary: {error}"))?,
                );
            }
            Ok(DebugGraphSymbol {
                symbol_id: symbol.symbol_id.to_string(),
                program_hash: program_hash.clone(),
                public_id: symbol.public_id,
                qualified_name: symbol.qualified_name,
                kind: symbol.kind,
                type_json: None,
                source_path: None,
                source_content_hash: None,
                start_byte: None,
                end_byte: None,
                semantic_hash,
                summary: symbol.summary,
                metadata,
            })
        })
        .collect()
}

pub(in crate::app::agent) fn agent_project_graph_edges(
    project: &ProjectSemanticIndex,
    _source_key_prefix: &str,
) -> Result<Vec<DebugGraphEdge>, String> {
    let program_hash = StableHash::new(project.program_hash().as_str())
        .map_err(|error| format!("invalid project graph program hash: {error}"))?;
    Ok(
        arcweft_compiler::agent_project::agent_project_graph_from_project(project)
            .map_err(|error| error.to_string())?
            .edges
            .into_iter()
            .map(|edge| DebugGraphEdge {
                program_hash: program_hash.clone(),
                from_symbol_id: edge.from_symbol_id.to_string(),
                to_symbol_id: edge.to_symbol_id.to_string(),
                edge_kind: edge.edge_kind,
                weight: 0.9,
                metadata: BTreeMap::new(),
            })
            .collect(),
    )
}

pub(in crate::app::agent) fn agent_project_summary_graph_symbol_id(
    _source_key_prefix: &str,
) -> String {
    "project:summary".to_owned()
}

pub(in crate::app::agent) fn agent_program_graph_symbol_id(program_hash: &StableHash) -> String {
    format!("program.{}.summary", program_hash.as_str())
}

pub(in crate::app::agent) fn agent_source_file_graph_symbol_id(source_key_prefix: &str) -> String {
    format!("{source_key_prefix}.source_file")
}

pub(in crate::app::agent) fn agent_dynamic_control_flow_count(
    project: &ProjectSemanticIndex,
) -> usize {
    project
        .flow_control_summaries()
        .values()
        .filter(|summary| summary.has_dynamic_control())
        .count()
}

pub(in crate::app::agent) fn agent_project_action_count(project: &ProjectSemanticIndex) -> usize {
    project
        .entities()
        .values()
        .map(|entity| entity.agent_actions().len())
        .sum()
}

pub(in crate::app::agent) fn agent_project_entity_kind_counts(
    project: &ProjectSemanticIndex,
) -> BTreeMap<String, usize> {
    project
        .entities()
        .values()
        .fold(BTreeMap::new(), |mut counts, entity| {
            *counts
                .entry(format!("{:?}", entity.ty().kind()))
                .or_insert(0) += 1;
            counts
        })
}

pub(in crate::app::agent) fn agent_project_relation_kind_counts(
    project: &ProjectSemanticIndex,
) -> BTreeMap<String, usize> {
    project
        .relations()
        .iter()
        .fold(BTreeMap::new(), |mut counts, relation| {
            *counts
                .entry(relation.edge_kind().as_str().to_owned())
                .or_insert(0) += 1;
            counts
        })
}

pub(in crate::app::agent) fn agent_project_dependency_edge_kind_counts(
    project: &ProjectSemanticIndex,
) -> BTreeMap<String, usize> {
    project
        .dependency_relations()
        .iter()
        .fold(BTreeMap::new(), |mut counts, relation| {
            *counts
                .entry(relation.edge_kind().as_str().to_owned())
                .or_insert(0) += 1;
            counts
        })
}

pub(in crate::app::agent) fn agent_project_flow_control_counts_json(
    project: &ProjectSemanticIndex,
) -> serde_json::Value {
    let summaries = project
        .flow_control_summaries()
        .values()
        .collect::<Vec<_>>();
    serde_json::json!({
        "symbol_count": summaries
            .iter()
            .filter(|summary| {
                summary.has_dynamic_control()
                    || summary.static_goto_count() > 0
                    || summary.dynamic_goto_count() > 0
                    || summary.branch_count() > 0
                    || summary.loop_count() > 0
                    || summary.await_count() > 0
                    || summary.thread_count() > 0
                    || summary.select_branch_count() > 0
            })
            .count(),
        "has_dynamic_control": summaries.iter().any(|summary| summary.has_dynamic_control()),
        "static_goto_count": summaries.iter().map(|summary| summary.static_goto_count()).sum::<usize>(),
        "dynamic_goto_count": summaries.iter().map(|summary| summary.dynamic_goto_count()).sum::<usize>(),
        "branch_count": summaries.iter().map(|summary| summary.branch_count()).sum::<usize>(),
        "loop_count": summaries.iter().map(|summary| summary.loop_count()).sum::<usize>(),
        "await_count": summaries.iter().map(|summary| summary.await_count()).sum::<usize>(),
        "thread_count": summaries.iter().map(|summary| summary.thread_count()).sum::<usize>(),
        "select_branch_count": summaries.iter().map(|summary| summary.select_branch_count()).sum::<usize>(),
    })
}

pub(in crate::app::agent) fn agent_flow_control_summary_text(
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

pub(in crate::app::agent) fn project_flow_control_summary_json(
    summary: Option<&arcweft_lang_sema::project_index::ProjectFlowControlSummary>,
) -> serde_json::Value {
    let Some(summary) = summary else {
        return serde_json::json!({
            "has_dynamic_control": false,
            "static_goto_count": 0,
            "dynamic_goto_count": 0,
            "branch_count": 0,
            "loop_count": 0,
            "await_count": 0,
            "thread_count": 0,
            "select_branch_count": 0,
        });
    };
    serde_json::json!({
        "has_dynamic_control": summary.has_dynamic_control(),
        "static_goto_count": summary.static_goto_count(),
        "dynamic_goto_count": summary.dynamic_goto_count(),
        "branch_count": summary.branch_count(),
        "loop_count": summary.loop_count(),
        "await_count": summary.await_count(),
        "thread_count": summary.thread_count(),
        "select_branch_count": summary.select_branch_count(),
    })
}

pub(in crate::app::agent) fn agent_program_summary_rag_candidate(
    program_hash: &StableHash,
    source_indexes: &[&AgentSourceRagIndex],
) -> Result<AgentRagCandidate, String> {
    let summary = agent_program_graph_summary(source_indexes);
    let sources = source_indexes
        .iter()
        .map(|source_index| {
            serde_json::json!({
                "path": source_index.source_file.path,
                "content_hash": source_index.source_file.content_hash.as_str(),
                "byte_len": source_index.source_file.byte_len,
                "candidate_chunks": source_index.candidates.len(),
                "graph_symbols": source_index.graph_symbols.len(),
                "graph_edges": source_index.graph_edges.len(),
                "dynamic_control_flows": agent_source_dynamic_control_flow_count(source_index),
                "graph_symbol_kinds": agent_graph_symbol_kind_counts(&source_index.graph_symbols),
                "graph_edge_kinds": agent_graph_edge_kind_counts(&source_index.graph_edges),
                "flow_control_counts": agent_source_flow_control_counts_json(source_index),
                "flow_control_symbols": agent_source_flow_control_symbols_json(source_index),
                "project_summary": agent_source_project_summary_json(source_index),
            })
        })
        .collect::<Vec<_>>();
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": 1,
        "kind": "program_rag_index",
        "program_hash": program_hash.as_str(),
        "counts": {
            "sources": summary.sources,
            "candidate_chunks": summary.candidate_chunks,
            "source_bytes": summary.source_bytes,
            "source_graph_symbols": summary.source_graph_symbols,
            "source_graph_edges": summary.source_graph_edges,
            "dynamic_control_flows": summary.dynamic_control_flows,
            "source_graph_symbol_kinds": summary.symbol_kinds,
            "source_graph_edge_kinds": summary.edge_kinds,
        },
        "sources": sources,
    }))
    .map_err(|error| format!("failed to serialize program RAG summary: {error}"))?;
    Ok(agent_rag_candidate(
        &format!("program.{}.summary", program_hash.as_str()),
        "Program RAG index summary",
        ChunkSourceKind::GraphSummary,
        SearchChannel::Summary,
        body,
        AgentRagCandidateMeta {
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Project,
            source_anchor: None,
            semantic_hash: Some(program_hash.clone()),
            metadata: BTreeMap::from([
                (
                    "program_hash".to_owned(),
                    serde_json::json!(program_hash.as_str()),
                ),
                (
                    "source_count".to_owned(),
                    serde_json::json!(summary.sources),
                ),
                (
                    "candidate_chunk_count".to_owned(),
                    serde_json::json!(summary.candidate_chunks),
                ),
                (
                    "source_graph_symbol_count".to_owned(),
                    serde_json::json!(summary.source_graph_symbols),
                ),
                (
                    "source_graph_edge_count".to_owned(),
                    serde_json::json!(summary.source_graph_edges),
                ),
                (
                    "source_graph_symbol_kinds".to_owned(),
                    serde_json::json!(summary.symbol_kinds),
                ),
                (
                    "source_graph_edge_kinds".to_owned(),
                    serde_json::json!(summary.edge_kinds),
                ),
            ]),
        },
    ))
}

pub(in crate::app::agent) fn agent_source_dynamic_control_flow_count(
    source_index: &AgentSourceRagIndex,
) -> usize {
    source_index
        .graph_symbols
        .iter()
        .filter(|symbol| agent_graph_symbol_has_dynamic_control(symbol))
        .count()
}

pub(in crate::app::agent) fn agent_source_flow_control_counts_json(
    source_index: &AgentSourceRagIndex,
) -> serde_json::Value {
    let flow_controls = source_index
        .graph_symbols
        .iter()
        .filter_map(|symbol| symbol.metadata.get("flow_control"))
        .collect::<Vec<_>>();
    serde_json::json!({
        "symbol_count": flow_controls
            .iter()
            .filter(|flow_control| agent_flow_control_json_has_control(flow_control))
            .count(),
        "has_dynamic_control": flow_controls.iter().any(|flow_control| {
            flow_control
                .get("has_dynamic_control")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        }),
        "static_goto_count": agent_flow_control_json_sum(&flow_controls, "static_goto_count"),
        "dynamic_goto_count": agent_flow_control_json_sum(&flow_controls, "dynamic_goto_count"),
        "branch_count": agent_flow_control_json_sum(&flow_controls, "branch_count"),
        "loop_count": agent_flow_control_json_sum(&flow_controls, "loop_count"),
        "await_count": agent_flow_control_json_sum(&flow_controls, "await_count"),
        "thread_count": agent_flow_control_json_sum(&flow_controls, "thread_count"),
        "select_branch_count": agent_flow_control_json_sum(&flow_controls, "select_branch_count"),
    })
}

pub(in crate::app::agent) fn agent_flow_control_json_sum(
    flow_controls: &[&serde_json::Value],
    field: &str,
) -> u64 {
    flow_controls
        .iter()
        .map(|flow_control| {
            flow_control
                .get(field)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
        })
        .sum()
}

pub(in crate::app::agent) fn agent_source_flow_control_symbols_json(
    source_index: &AgentSourceRagIndex,
) -> Vec<serde_json::Value> {
    source_index
        .graph_symbols
        .iter()
        .filter_map(|symbol| {
            let flow_control = symbol.metadata.get("flow_control")?;
            if !agent_flow_control_json_has_control(flow_control) {
                return None;
            }
            Some(serde_json::json!({
                "symbol_id": symbol.symbol_id.as_str(),
                "public_id": symbol.public_id.as_ref().map(AgentPublicId::as_str),
                "qualified_name": symbol.qualified_name.as_deref(),
                "kind": symbol.kind.as_str(),
                "summary": symbol.summary.as_str(),
                "flow_control": flow_control,
            }))
        })
        .collect()
}

pub(in crate::app::agent) fn agent_flow_control_json_has_control(
    flow_control: &serde_json::Value,
) -> bool {
    flow_control
        .get("has_dynamic_control")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
        || [
            "static_goto_count",
            "dynamic_goto_count",
            "branch_count",
            "loop_count",
            "await_count",
            "thread_count",
            "select_branch_count",
        ]
        .into_iter()
        .any(|field| {
            flow_control
                .get(field)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                > 0
        })
}

pub(in crate::app::agent) fn agent_source_project_summary_json(
    source_index: &AgentSourceRagIndex,
) -> Option<serde_json::Value> {
    source_index
        .graph_symbols
        .iter()
        .find(|symbol| symbol.kind == "project_summary")
        .map(|symbol| {
            serde_json::json!({
                "symbol_id": symbol.symbol_id.as_str(),
                "qualified_name": symbol.qualified_name.as_deref(),
                "summary": symbol.summary.as_str(),
                "metadata": &symbol.metadata,
            })
        })
}

pub(in crate::app::agent) fn agent_project_summary_rag_candidate(
    path: &Path,
    project: &ProjectSemanticIndex,
    source_key_prefix: &str,
) -> Result<AgentRagCandidate, String> {
    let entity_kind_counts = agent_project_entity_kind_counts(project);
    let relation_kind_counts = agent_project_relation_kind_counts(project);
    let dependency_edge_kind_counts = agent_project_dependency_edge_kind_counts(project);
    let flow_control_counts = agent_project_flow_control_counts_json(project);
    let agent_action_count = agent_project_action_count(project);
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": project.schema_version(),
        "kind": "project_semantic_index",
        "program_hash": project.program_hash().as_str(),
        "bundle_hash": project.bundle_hash().map(arcweft_lang_sema::project_index::BundleHash::as_str),
        "counts": {
            "entities": project.entities().len(),
            "callables": project.checked_callables().records().len(),
            "project_callables": project.project_callables().len(),
            "agent_actions": agent_action_count,
            "relations": project.relations().len(),
            "dependency_edges": project.dependency_relations().len(),
            "dynamic_control_flows": agent_dynamic_control_flow_count(project),
            "types": project.types().len(),
            "debug_queries": project.debug_queries().len(),
            "entity_kinds": entity_kind_counts,
            "relation_kinds": relation_kind_counts,
            "dependency_edge_kinds": dependency_edge_kind_counts,
            "flow_control": flow_control_counts,
        },
    }))
    .map_err(|error| format!("failed to serialize project RAG summary: {error}"))?;
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "path".to_owned(),
        serde_json::Value::String(path.display().to_string()),
    );
    metadata.insert(
        "entity_kind_counts".to_owned(),
        serde_json::json!(agent_project_entity_kind_counts(project)),
    );
    metadata.insert(
        "relation_kind_counts".to_owned(),
        serde_json::json!(agent_project_relation_kind_counts(project)),
    );
    metadata.insert(
        "dependency_edge_kind_counts".to_owned(),
        serde_json::json!(agent_project_dependency_edge_kind_counts(project)),
    );
    metadata.insert(
        "flow_control_counts".to_owned(),
        agent_project_flow_control_counts_json(project),
    );
    metadata.insert(
        "agent_action_count".to_owned(),
        serde_json::json!(agent_action_count),
    );
    Ok(agent_rag_candidate(
        &format!("{source_key_prefix}.project.summary"),
        "Project semantic index summary",
        ChunkSourceKind::GraphSummary,
        SearchChannel::Summary,
        body,
        AgentRagCandidateMeta {
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Project,
            source_anchor: None,
            semantic_hash: Some(
                StableHash::new(project.program_hash().as_str())
                    .map_err(|error| format!("invalid project semantic hash: {error}"))?,
            ),
            metadata,
        },
    ))
}

pub(in crate::app::agent) fn agent_project_entity_rag_candidate(
    entity: &EntitySymbol,
    source_key_prefix: &str,
    project: &ProjectSemanticIndex,
) -> Result<AgentRagCandidate, String> {
    let identity = entity.identity();
    let identity_key = identity.canonical_key();
    let entity_id = agent_public_id_from_sema(entity.public_id())?;
    let flow_control = project_flow_control_summary_json(project.flow_control_summary(identity));
    let actions = entity
        .agent_actions()
        .iter()
        .map(|action| {
            serde_json::json!({
                "action": action.action().as_str(),
                "params": action.params().iter().map(|param| {
                    serde_json::json!({
                        "name": param.name(),
                        "type": format!("{:?}", param.ty()),
                        "has_default": param.has_default(),
                    })
                }).collect::<Vec<_>>(),
                "return_type": format!("{:?}", action.return_type()),
            })
        })
        .collect::<Vec<_>>();
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "kind": "project_entity",
        "identity": identity_key,
        "public_id": entity.public_id().as_str(),
        "entity_kind": format!("{:?}", entity.ty().kind()),
        "value_type": entity.ty().value().map(|ty| format!("{ty:?}")),
        "source": source_anchor_json(entity.source()),
        "semantic_hash": entity.semantic_hash().as_str(),
        "agent_actions": actions,
        "flow_control": flow_control,
    }))
    .map_err(|error| format!("failed to serialize project entity RAG chunk: {error}"))?;
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "entity_kind".to_owned(),
        serde_json::Value::String(format!("{:?}", entity.ty().kind())),
    );
    metadata.insert("flow_control".to_owned(), flow_control);
    Ok(agent_rag_candidate(
        &format!(
            "{source_key_prefix}.project.entity.{}",
            identity.canonical_key()
        ),
        &format!(
            "Project entity {} {:?}{}",
            entity.public_id().as_str(),
            entity.ty().kind(),
            agent_flow_control_summary_text(project, identity)
        ),
        ChunkSourceKind::Symbol,
        SearchChannel::Graph,
        body,
        AgentRagCandidateMeta {
            entity_ids: vec![entity_id],
            privacy: PrivacyClass::Project,
            source_anchor: debug_anchor_from_source_anchor(entity.source())?,
            semantic_hash: Some(
                StableHash::new(entity.semantic_hash().as_str())
                    .map_err(|error| format!("invalid entity semantic hash: {error}"))?,
            ),
            metadata,
        },
    ))
}

pub(in crate::app::agent) fn agent_project_callable_rag_candidate(
    project: &ProjectSemanticIndex,
    declaration: &CallableDeclarationKey,
    callable: &ProjectCallableSymbol,
    source_key_prefix: &str,
) -> Result<AgentRagCandidate, String> {
    let facts = checked_project_callable(project, declaration, callable)?;
    let name = declaration.qualified_name();
    let source = facts.source().and_then(|source| source.signature());
    let interface_digest = digest_hex(callable.interface_digest().as_bytes());
    let declaration_digest = digest_hex(declaration.semantic_digest().as_bytes());
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "kind": "project_callable",
        "name": name.as_str(),
        "callable_kind": callable.kind().as_str(),
        "signature": format!("{:?}", facts.signature()),
        "source": source_span_json(source),
        "semantic_hash": interface_digest.as_str(),
    }))
    .map_err(|error| format!("failed to serialize project callable RAG chunk: {error}"))?;
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "callable_kind".to_owned(),
        serde_json::Value::String(callable.kind().as_str().to_owned()),
    );
    Ok(agent_rag_candidate(
        &format!("{source_key_prefix}.project.callable.v1.{declaration_digest}"),
        &format!("Project {} callable {}", callable.kind().as_str(), name),
        ChunkSourceKind::Symbol,
        SearchChannel::Graph,
        body,
        AgentRagCandidateMeta {
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Project,
            source_anchor: debug_anchor_from_source_span(source)?,
            semantic_hash: Some(
                StableHash::new(interface_digest)
                    .map_err(|error| format!("invalid callable semantic hash: {error}"))?,
            ),
            metadata,
        },
    ))
}

fn checked_project_callable<'a>(
    project: &'a ProjectSemanticIndex,
    declaration: &CallableDeclarationKey,
    callable: &ProjectCallableSymbol,
) -> Result<&'a CheckedCallableFacts, String> {
    let facts = project
        .checked_callable(callable.checked())
        .map_err(|reason| checked_callable_lookup_message(callable.checked(), &reason))?;
    if callable.declaration() != declaration
        || facts.id() != callable.checked()
        || facts.interface_digest() != callable.interface_digest()
        || !matches!(
            facts.record().id(),
            CallableCandidateId::Project(candidate) if candidate == declaration
        )
    {
        return Err(format!(
            "project callable {declaration:?} does not match checked callable {:?}",
            callable.checked()
        ));
    }
    Ok(facts)
}

fn checked_callable_lookup_message(
    checked: &arcweft_lang_sema::callable::CheckedCallableId,
    reason: &CheckedCallableLookupError,
) -> String {
    format!("checked callable {checked:?} lookup failed: {reason:?}")
}

fn debug_anchor_from_source_span(
    span: Option<&SourceSpan>,
) -> Result<Option<DebugSourceAnchor>, String> {
    span.map(|span| {
        Ok(DebugSourceAnchor {
            path: span.source().id().as_str().to_owned(),
            start_byte: u64::try_from(span.range().start())
                .map_err(|_| "callable source start byte overflowed u64".to_owned())?,
            end_byte: u64::try_from(span.range().end())
                .map_err(|_| "callable source end byte overflowed u64".to_owned())?,
        })
    })
    .transpose()
}

fn source_span_json(span: Option<&SourceSpan>) -> serde_json::Value {
    span.map_or(serde_json::Value::Null, |span| {
        serde_json::json!({
            "document": span.source().id().as_str(),
            "start_byte": span.range().start(),
            "end_byte": span.range().end(),
        })
    })
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

pub(in crate::app::agent) fn agent_project_debug_query_rag_candidate(
    name: &arcweft_lang_sema::project_index::QualifiedName,
    query: &arcweft_lang_sema::project_index::DebugQuerySymbol,
    source_key_prefix: &str,
) -> Result<AgentRagCandidate, String> {
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "kind": "project_debug_query",
        "name": name.as_str(),
        "signature": format!("{:?}", query.signature()),
    }))
    .map_err(|error| format!("failed to serialize project debug query RAG chunk: {error}"))?;
    Ok(agent_rag_candidate(
        &format!("{source_key_prefix}.project.debug_query.{}", name.as_str()),
        &format!("Project debug query {}", name.as_str()),
        ChunkSourceKind::Symbol,
        SearchChannel::Graph,
        body,
        AgentRagCandidateMeta {
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Project,
            source_anchor: None,
            semantic_hash: Some(
                StableHash::new(agent_content_hash(name.as_str()))
                    .map_err(|error| format!("invalid debug query semantic hash: {error}"))?,
            ),
            metadata: BTreeMap::new(),
        },
    ))
}

pub(in crate::app::agent) struct AgentRagCandidateMeta {
    pub(in crate::app::agent) entity_ids: Vec<AgentPublicId>,
    pub(in crate::app::agent) privacy: PrivacyClass,
    pub(in crate::app::agent) source_anchor: Option<DebugSourceAnchor>,
    pub(in crate::app::agent) semantic_hash: Option<StableHash>,
    pub(in crate::app::agent) metadata: BTreeMap<String, serde_json::Value>,
}

pub(in crate::app::agent) fn agent_rag_candidate(
    source_key: &str,
    title: &str,
    source_kind: ChunkSourceKind,
    preferred_channel: SearchChannel,
    body: String,
    meta: AgentRagCandidateMeta,
) -> AgentRagCandidate {
    let content_hash = agent_content_hash(&body);
    AgentRagCandidate {
        chunk: DebugChunk {
            id: ChunkId::new(format!("cli:{source_key}:{content_hash}")),
            program_hash: None,
            source_kind,
            source_key: source_key.to_owned(),
            title: title.to_owned(),
            body,
            content_hash: StableHash::new(content_hash)
                .expect("generated content hash is non-empty"),
            semantic_hash: meta.semantic_hash,
            source_anchor: meta.source_anchor,
            entity_ids: meta.entity_ids,
            privacy: meta.privacy,
            metadata: meta.metadata,
            created_unix_ms: 0,
        },
        preferred_channel,
    }
}

pub(in crate::app::agent) fn agent_public_id_from_sema(
    id: &SemaPublicId,
) -> Result<AgentPublicId, String> {
    AgentPublicId::new(id.as_str().to_owned())
        .map_err(|error| format!("failed to convert project id `{}`: {error}", id.as_str()))
}

pub(in crate::app::agent) fn debug_anchor_from_source_anchor(
    anchor: &SourceAnchor,
) -> Result<Option<DebugSourceAnchor>, String> {
    Ok(Some(DebugSourceAnchor {
        path: anchor.source().id().as_str().to_owned(),
        start_byte: u64::try_from(anchor.byte_range().start)
            .map_err(|_| "source anchor start byte overflowed u64".to_owned())?,
        end_byte: u64::try_from(anchor.byte_range().end)
            .map_err(|_| "source anchor end byte overflowed u64".to_owned())?,
    }))
}

pub(in crate::app::agent) fn debug_source_anchor(
    path: &Path,
    range: std::ops::Range<usize>,
) -> Result<DebugSourceAnchor, String> {
    Ok(DebugSourceAnchor {
        path: path.display().to_string(),
        start_byte: u64::try_from(range.start)
            .map_err(|_| "source chunk start byte overflowed u64".to_owned())?,
        end_byte: u64::try_from(range.end)
            .map_err(|_| "source chunk end byte overflowed u64".to_owned())?,
    })
}

pub(in crate::app::agent) fn source_anchor_json(anchor: &SourceAnchor) -> serde_json::Value {
    serde_json::json!({
        "document": anchor.source().id().as_str(),
        "start_byte": anchor.byte_range().start,
        "end_byte": anchor.byte_range().end,
    })
}

pub(in crate::app::agent) fn agent_trace_record_privacy(record: &AgentTraceRecord) -> PrivacyClass {
    record
        .payload
        .get("privacy_class")
        .or_else(|| record.payload.get("privacy"))
        .or_else(|| {
            record
                .payload
                .get("payload")
                .and_then(|payload| payload.get("privacy_class"))
        })
        .or_else(|| {
            record
                .payload
                .get("payload")
                .and_then(|payload| payload.get("privacy"))
        })
        .and_then(serde_json::Value::as_str)
        .and_then(PrivacyClass::parse)
        .unwrap_or(PrivacyClass::Project)
}

pub(in crate::app::agent) fn agent_trace_record_entity_ids(
    record: &AgentTraceRecord,
) -> Vec<AgentPublicId> {
    [
        Some(record.run_id.as_str()),
        record.session_id.as_ref().map(SessionId::as_str),
        Some(agent_trace_kind_name(record.kind)),
    ]
    .into_iter()
    .flatten()
    .filter_map(|value| AgentPublicId::new(value.to_owned()).ok())
    .collect()
}

pub(in crate::app::agent) fn agent_trace_rag_ranked_lists(
    candidates: &[AgentRagCandidate],
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
                agent_trace_rag_score(candidate, query, channel).map(|score| (candidate, score))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.chunk.id.cmp(&right.0.chunk.id))
        });
        (!scored.is_empty()).then(|| {
            scored
                .into_iter()
                .enumerate()
                .map(|(index, (candidate, score))| SearchHit {
                    chunk_id: candidate.chunk.id.clone(),
                    channel,
                    rank: index + 1,
                    score: Some(score),
                })
                .collect()
        })
    })
    .collect()
}

pub(in crate::app::agent) fn agent_trace_rag_score(
    candidate: &AgentRagCandidate,
    query: &RagQuery,
    channel: SearchChannel,
) -> Option<f64> {
    let haystack = agent_trace_rag_haystack(candidate);
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
            let token_score = agent_count_as_f64(
                agent_rag_tokens(&query.text)
                    .into_iter()
                    .filter(|token| haystack.contains(token))
                    .count(),
            );
            (phrase + token_score > 0.0).then_some(phrase.mul_add(4.0, token_score))
        }
        SearchChannel::Graph => {
            let root_score = if query.graph_depth > 0 {
                agent_count_as_f64(
                    query
                        .roots
                        .iter()
                        .filter(|root| haystack.contains(&root.as_str().to_lowercase()))
                        .count(),
                )
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
            let token_score = agent_count_as_f64(
                agent_rag_tokens(&query.text)
                    .into_iter()
                    .filter(|token| haystack.contains(token))
                    .count(),
            );
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

pub(in crate::app::agent) const fn search_channel_label(channel: SearchChannel) -> &'static str {
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

pub(in crate::app::agent) fn agent_trace_rag_haystack(candidate: &AgentRagCandidate) -> String {
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

pub(in crate::app::agent) fn agent_rag_roots(
    values: &[String],
) -> Result<Vec<AgentPublicId>, String> {
    values
        .iter()
        .map(|root| {
            let root = root.trim();
            AgentPublicId::new(root.to_owned())
                .map_err(|_| "agent rag query --root values must not be empty".to_owned())
        })
        .collect()
}

pub(in crate::app::agent) fn parse_agent_privacy_class(
    value: &str,
) -> Result<PrivacyClass, String> {
    PrivacyClass::parse(value).ok_or_else(|| {
        format!("privacy class must be one of public, project, sensitive, or secret: `{value}`")
    })
}

pub(in crate::app::agent) fn agent_rag_tokens(text: &str) -> BTreeSet<String> {
    text.split(|character: char| {
        !(character.is_alphanumeric() || character == '.' || character == '_' || character == '-')
    })
    .map(str::trim)
    .filter(|token| !token.is_empty())
    .map(str::to_lowercase)
    .collect()
}

pub(in crate::app::agent) fn agent_trace_rag_seed(
    path: &Path,
    records: &[AgentTraceRecord],
) -> String {
    let seed = records
        .iter()
        .map(|record| record.payload_hash.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    format!("trace:{}:{seed}", path.display())
}

pub(in crate::app::agent) fn agent_rag_program_hash(
    seed_parts: &[String],
) -> Result<StableHash, String> {
    StableHash::new(agent_content_hash(seed_parts.join("\n")))
        .map_err(|_| "failed to build Agent RAG program hash".to_owned())
}

pub(in crate::app::agent) fn agent_content_hash(bytes: impl AsRef<[u8]>) -> String {
    format!("blake3:{}", blake3::hash(bytes.as_ref()).to_hex())
}

pub(in crate::app::agent) fn agent_count_as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

pub(in crate::app::agent) fn truncate_utf8(text: &str, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_owned(), false);
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}
