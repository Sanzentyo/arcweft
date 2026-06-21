use arcweft_agent_protocol::ids::{AgentRunId, PublicId, SessionId, StableHash};
use arcweft_debug_model::{
    chunk::{ChunkId, DebugChunk, PrivacyClass, SourceAnchor},
    graph::{DebugGraphEdge, DebugGraphSymbol},
    script::{DebugScriptRun, DebugScriptRunOutcome},
    session::{DebugSession, DebugSessionStatus},
    source::DebugSourceFile,
};

use super::{
    DebugStoreError,
    helpers::parse_chunk_source_kind,
    raw::{
        RawDebugChunk, RawDebugGraphEdge, RawDebugGraphSymbol, RawDebugScriptRun, RawDebugSession,
        RawDebugSourceFile,
    },
};

pub(crate) fn source_anchor_from_row(
    path: Option<String>,
    start_byte: Option<i64>,
    end_byte: Option<i64>,
) -> Result<Option<SourceAnchor>, DebugStoreError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let Some(start_byte) = start_byte else {
        return Ok(None);
    };
    let Some(end_byte) = end_byte else {
        return Ok(None);
    };
    Ok(Some(SourceAnchor {
        path,
        start_byte: u64::try_from(start_byte)
            .map_err(|_| DebugStoreError::IntegerOverflow("chunks.start_byte"))?,
        end_byte: u64::try_from(end_byte)
            .map_err(|_| DebugStoreError::IntegerOverflow("chunks.end_byte"))?,
    }))
}

pub(crate) fn raw_debug_chunk_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawDebugChunk> {
    Ok(RawDebugChunk {
        chunk_id: row.get(0)?,
        program_hash: row.get(1)?,
        source_kind: row.get(2)?,
        source_key: row.get(3)?,
        title: row.get(4)?,
        body: row.get(5)?,
        content_hash: row.get(6)?,
        semantic_hash: row.get(7)?,
        source_path: row.get(8)?,
        entity_ids_json: row.get(9)?,
        start_byte: row.get(10)?,
        end_byte: row.get(11)?,
        privacy_class: row.get(12)?,
        metadata_json: row.get(13)?,
        created_unix_ms: row.get(14)?,
    })
}

pub(crate) fn raw_debug_source_file_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RawDebugSourceFile> {
    Ok(RawDebugSourceFile {
        program_hash: row.get(0)?,
        path: row.get(1)?,
        language: row.get(2)?,
        content_hash: row.get(3)?,
        byte_len: row.get(4)?,
        metadata_json: row.get(5)?,
    })
}

pub(crate) fn raw_debug_graph_symbol_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RawDebugGraphSymbol> {
    Ok(RawDebugGraphSymbol {
        program_hash: row.get(0)?,
        symbol_id: row.get(1)?,
        public_id: row.get(2)?,
        qualified_name: row.get(3)?,
        kind: row.get(4)?,
        type_json: row.get(5)?,
        source_path: row.get(6)?,
        source_content_hash: row.get(7)?,
        start_byte: row.get(8)?,
        end_byte: row.get(9)?,
        semantic_hash: row.get(10)?,
        summary: row.get(11)?,
        metadata_json: row.get(12)?,
    })
}

pub(crate) fn raw_debug_graph_edge_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RawDebugGraphEdge> {
    Ok(RawDebugGraphEdge {
        program_hash: row.get(0)?,
        from_symbol_id: row.get(1)?,
        to_symbol_id: row.get(2)?,
        edge_kind: row.get(3)?,
        weight: row.get(4)?,
        metadata_json: row.get(5)?,
    })
}

pub(crate) fn raw_debug_session_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RawDebugSession> {
    Ok(RawDebugSession {
        session_id: row.get(0)?,
        program_hash: row.get(1)?,
        profile: row.get(2)?,
        transport: row.get(3)?,
        started_unix_ms: row.get(4)?,
        ended_unix_ms: row.get(5)?,
        status: row.get(6)?,
        metadata_json: row.get(7)?,
    })
}

pub(crate) fn raw_debug_script_run_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RawDebugScriptRun> {
    Ok(RawDebugScriptRun {
        run_id: row.get(0)?,
        session_id: row.get(1)?,
        agent_id: row.get(2)?,
        artifact_hash: row.get(3)?,
        source_hash: row.get(4)?,
        project_binding_mode: row.get(5)?,
        started_sequence: row.get(6)?,
        finished_sequence: row.get(7)?,
        outcome: row.get(8)?,
        partially_effectful: row.get(9)?,
        trace_uri: row.get(10)?,
        error_json: row.get(11)?,
        metadata_json: row.get(12)?,
    })
}

pub(crate) fn debug_chunk_from_raw(raw: RawDebugChunk) -> Result<DebugChunk, DebugStoreError> {
    let source_kind = parse_chunk_source_kind(&raw.source_kind)
        .ok_or_else(|| DebugStoreError::InvalidChunkSourceKind(raw.source_kind.clone()))?;
    let privacy = PrivacyClass::parse(&raw.privacy_class)
        .ok_or_else(|| DebugStoreError::InvalidPrivacyClass(raw.privacy_class.clone()))?;
    let entity_ids = serde_json::from_str::<Vec<String>>(&raw.entity_ids_json)?
        .into_iter()
        .map(PublicId::new)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DebugChunk {
        id: ChunkId::new(raw.chunk_id),
        program_hash: raw.program_hash.map(StableHash::new).transpose()?,
        source_kind,
        source_key: raw.source_key,
        title: raw.title,
        body: raw.body,
        content_hash: StableHash::new(raw.content_hash)?,
        semantic_hash: raw.semantic_hash.map(StableHash::new).transpose()?,
        source_anchor: source_anchor_from_row(raw.source_path, raw.start_byte, raw.end_byte)?,
        entity_ids,
        privacy,
        metadata: serde_json::from_str(&raw.metadata_json)?,
        created_unix_ms: raw.created_unix_ms,
    })
}

pub(crate) fn debug_source_file_from_raw(
    raw: RawDebugSourceFile,
) -> Result<DebugSourceFile, DebugStoreError> {
    Ok(DebugSourceFile {
        program_hash: StableHash::new(raw.program_hash)?,
        path: raw.path,
        language: raw.language,
        content_hash: StableHash::new(raw.content_hash)?,
        byte_len: u64::try_from(raw.byte_len)
            .map_err(|_| DebugStoreError::IntegerOverflow("source_files.byte_len"))?,
        metadata: serde_json::from_str(&raw.metadata_json)?,
    })
}

pub(crate) fn debug_graph_symbol_from_raw(
    raw: RawDebugGraphSymbol,
) -> Result<DebugGraphSymbol, DebugStoreError> {
    Ok(DebugGraphSymbol {
        symbol_id: raw.symbol_id,
        program_hash: StableHash::new(raw.program_hash)?,
        public_id: raw.public_id.map(PublicId::new).transpose()?,
        qualified_name: raw.qualified_name,
        kind: raw.kind,
        type_json: raw
            .type_json
            .map(|json| serde_json::from_str(&json))
            .transpose()?,
        source_path: raw.source_path,
        source_content_hash: raw.source_content_hash.map(StableHash::new).transpose()?,
        start_byte: raw
            .start_byte
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| DebugStoreError::IntegerOverflow("symbols.start_byte"))
            })
            .transpose()?,
        end_byte: raw
            .end_byte
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| DebugStoreError::IntegerOverflow("symbols.end_byte"))
            })
            .transpose()?,
        semantic_hash: raw.semantic_hash.map(StableHash::new).transpose()?,
        summary: raw.summary,
        metadata: serde_json::from_str(&raw.metadata_json)?,
    })
}

pub(crate) fn debug_graph_edge_from_raw(
    raw: RawDebugGraphEdge,
) -> Result<DebugGraphEdge, DebugStoreError> {
    Ok(DebugGraphEdge {
        program_hash: StableHash::new(raw.program_hash)?,
        from_symbol_id: raw.from_symbol_id,
        to_symbol_id: raw.to_symbol_id,
        edge_kind: raw.edge_kind,
        weight: raw.weight,
        metadata: serde_json::from_str(&raw.metadata_json)?,
    })
}

pub(crate) fn debug_session_from_raw(
    raw: RawDebugSession,
) -> Result<DebugSession, DebugStoreError> {
    let status = DebugSessionStatus::parse(&raw.status)
        .ok_or_else(|| DebugStoreError::InvalidSessionStatus(raw.status.clone()))?;
    let metadata = serde_json::from_str(&raw.metadata_json)?;
    Ok(DebugSession {
        session_id: SessionId::new(raw.session_id)?,
        program_hash: raw.program_hash.map(StableHash::new).transpose()?,
        profile: raw.profile,
        transport: raw.transport,
        started_unix_ms: raw.started_unix_ms,
        ended_unix_ms: raw.ended_unix_ms,
        status,
        metadata,
    })
}

pub(crate) fn debug_script_run_from_raw(
    raw: RawDebugScriptRun,
) -> Result<DebugScriptRun, DebugStoreError> {
    let outcome = DebugScriptRunOutcome::parse(&raw.outcome)
        .ok_or_else(|| DebugStoreError::InvalidScriptRunOutcome(raw.outcome.clone()))?;
    Ok(DebugScriptRun {
        run_id: AgentRunId::new(raw.run_id)?,
        session_id: SessionId::new(raw.session_id)?,
        agent_id: raw.agent_id.map(PublicId::new).transpose()?,
        artifact_hash: raw.artifact_hash.map(StableHash::new).transpose()?,
        source_hash: raw.source_hash.map(StableHash::new).transpose()?,
        project_binding_mode: raw.project_binding_mode,
        started_sequence: u64::try_from(raw.started_sequence)
            .map_err(|_| DebugStoreError::IntegerOverflow("script_runs.started_sequence"))?,
        finished_sequence: raw
            .finished_sequence
            .map(|sequence| {
                u64::try_from(sequence)
                    .map_err(|_| DebugStoreError::IntegerOverflow("script_runs.finished_sequence"))
            })
            .transpose()?,
        outcome,
        partially_effectful: raw.partially_effectful != 0,
        trace_uri: raw.trace_uri,
        error: raw
            .error_json
            .map(|json| serde_json::from_str(&json))
            .transpose()?,
        metadata: serde_json::from_str(&raw.metadata_json)?,
    })
}

pub(crate) fn truncate_utf8(value: &str, max_bytes: usize) -> (String, bool) {
    if value.len() <= max_bytes {
        return (value.to_owned(), false);
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_owned(), true)
}

pub(crate) fn sqlite_i64(value: u64, column: &'static str) -> Result<i64, DebugStoreError> {
    i64::try_from(value).map_err(|_| DebugStoreError::IntegerOverflow(column))
}

pub(crate) const fn sqlite_bool(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

pub(crate) fn history_score(query: &str, change_id: &str, body: &str) -> f64 {
    let query = query.trim().to_lowercase();
    if change_id.eq_ignore_ascii_case(&query) {
        2.0
    } else if body.to_lowercase().contains(&query) {
        1.0
    } else {
        0.5
    }
}
