#[derive(Debug)]
pub(crate) struct RawDebugSession {
    pub(crate) session_id: String,
    pub(crate) program_hash: Option<String>,
    pub(crate) profile: String,
    pub(crate) transport: String,
    pub(crate) started_unix_ms: i64,
    pub(crate) ended_unix_ms: Option<i64>,
    pub(crate) status: String,
    pub(crate) metadata_json: String,
}

#[derive(Debug)]
pub(crate) struct RawDebugScriptRun {
    pub(crate) run_id: String,
    pub(crate) session_id: String,
    pub(crate) agent_id: Option<String>,
    pub(crate) artifact_hash: Option<String>,
    pub(crate) source_hash: Option<String>,
    pub(crate) project_binding_mode: String,
    pub(crate) started_sequence: i64,
    pub(crate) finished_sequence: Option<i64>,
    pub(crate) outcome: String,
    pub(crate) partially_effectful: i64,
    pub(crate) trace_uri: Option<String>,
    pub(crate) error_json: Option<String>,
    pub(crate) metadata_json: String,
}

#[derive(Debug)]
pub(crate) struct RawDebugChunk {
    pub(crate) chunk_id: String,
    pub(crate) program_hash: Option<String>,
    pub(crate) source_kind: String,
    pub(crate) source_key: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) content_hash: String,
    pub(crate) semantic_hash: Option<String>,
    pub(crate) source_path: Option<String>,
    pub(crate) entity_ids_json: String,
    pub(crate) start_byte: Option<i64>,
    pub(crate) end_byte: Option<i64>,
    pub(crate) privacy_class: String,
    pub(crate) metadata_json: String,
    pub(crate) created_unix_ms: i64,
}

#[derive(Debug)]
pub(crate) struct RawDebugSourceFile {
    pub(crate) program_hash: String,
    pub(crate) path: String,
    pub(crate) language: String,
    pub(crate) content_hash: String,
    pub(crate) byte_len: i64,
    pub(crate) metadata_json: String,
}

#[derive(Debug)]
pub(crate) struct RawDebugGraphSymbol {
    pub(crate) program_hash: String,
    pub(crate) symbol_id: String,
    pub(crate) public_id: Option<String>,
    pub(crate) qualified_name: Option<String>,
    pub(crate) kind: String,
    pub(crate) type_json: Option<String>,
    pub(crate) source_path: Option<String>,
    pub(crate) source_content_hash: Option<String>,
    pub(crate) start_byte: Option<i64>,
    pub(crate) end_byte: Option<i64>,
    pub(crate) semantic_hash: Option<String>,
    pub(crate) summary: String,
    pub(crate) metadata_json: String,
}

#[derive(Debug)]
pub(crate) struct RawDebugGraphEdge {
    pub(crate) program_hash: String,
    pub(crate) from_symbol_id: String,
    pub(crate) to_symbol_id: String,
    pub(crate) edge_kind: String,
    pub(crate) weight: f64,
    pub(crate) metadata_json: String,
}
