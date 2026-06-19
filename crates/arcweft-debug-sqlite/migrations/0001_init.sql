PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    applied_unix_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS programs (
    program_id INTEGER PRIMARY KEY,
    program_hash TEXT NOT NULL UNIQUE,
    bundle_hash TEXT,
    jj_change_id TEXT,
    profile_hash TEXT,
    source_root TEXT,
    created_unix_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS source_files (
    source_file_id INTEGER PRIMARY KEY,
    program_id INTEGER NOT NULL REFERENCES programs(program_id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    language TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    metadata_json TEXT NOT NULL DEFAULT '{}',
    UNIQUE (program_id, path, content_hash)
);

CREATE INDEX IF NOT EXISTS source_files_program_path
    ON source_files(program_id, path);

CREATE TABLE IF NOT EXISTS symbols (
    symbol_id TEXT PRIMARY KEY,
    program_id INTEGER NOT NULL REFERENCES programs(program_id) ON DELETE CASCADE,
    public_id TEXT,
    qualified_name TEXT,
    kind TEXT NOT NULL,
    type_json TEXT,
    source_file_id INTEGER REFERENCES source_files(source_file_id) ON DELETE SET NULL,
    start_byte INTEGER,
    end_byte INTEGER,
    semantic_hash TEXT,
    summary TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL DEFAULT '{}',
    CHECK (start_byte IS NULL OR start_byte >= 0),
    CHECK (end_byte IS NULL OR end_byte >= start_byte)
);

CREATE UNIQUE INDEX IF NOT EXISTS symbols_program_public_kind
    ON symbols(program_id, public_id, kind)
    WHERE public_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS symbols_program_qualified
    ON symbols(program_id, qualified_name)
    WHERE qualified_name IS NOT NULL;
CREATE INDEX IF NOT EXISTS symbols_semantic_hash
    ON symbols(program_id, semantic_hash)
    WHERE semantic_hash IS NOT NULL;

CREATE TABLE IF NOT EXISTS graph_edges (
    edge_id INTEGER PRIMARY KEY,
    program_id INTEGER NOT NULL REFERENCES programs(program_id) ON DELETE CASCADE,
    from_symbol_id TEXT NOT NULL REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    to_symbol_id TEXT NOT NULL REFERENCES symbols(symbol_id) ON DELETE CASCADE,
    edge_kind TEXT NOT NULL,
    weight REAL NOT NULL DEFAULT 1.0,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    UNIQUE (program_id, from_symbol_id, to_symbol_id, edge_kind)
);

CREATE INDEX IF NOT EXISTS graph_edges_from
    ON graph_edges(program_id, from_symbol_id, edge_kind);
CREATE INDEX IF NOT EXISTS graph_edges_to
    ON graph_edges(program_id, to_symbol_id, edge_kind);

CREATE TABLE IF NOT EXISTS sessions (
    session_id TEXT PRIMARY KEY,
    program_id INTEGER REFERENCES programs(program_id) ON DELETE SET NULL,
    profile TEXT NOT NULL,
    transport TEXT NOT NULL,
    started_unix_ms INTEGER NOT NULL,
    ended_unix_ms INTEGER,
    status TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS script_runs (
    run_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
    agent_id TEXT,
    artifact_hash TEXT,
    source_hash TEXT,
    project_binding_mode TEXT NOT NULL,
    started_sequence INTEGER NOT NULL,
    finished_sequence INTEGER,
    outcome TEXT NOT NULL,
    partially_effectful INTEGER NOT NULL DEFAULT 0 CHECK (partially_effectful IN (0, 1)),
    trace_uri TEXT,
    error_json TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS script_runs_session
    ON script_runs(session_id, started_sequence);
CREATE INDEX IF NOT EXISTS script_runs_agent
    ON script_runs(agent_id, outcome);

CREATE TABLE IF NOT EXISTS debug_events (
    event_id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
    run_id TEXT REFERENCES script_runs(run_id) ON DELETE SET NULL,
    sequence INTEGER NOT NULL,
    tick INTEGER,
    event_kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_unix_ms INTEGER NOT NULL,
    UNIQUE (session_id, sequence, event_kind)
);

CREATE INDEX IF NOT EXISTS debug_events_session_sequence
    ON debug_events(session_id, sequence);

CREATE TABLE IF NOT EXISTS script_steps (
    run_id TEXT NOT NULL REFERENCES script_runs(run_id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    parent_sequence INTEGER,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    before_tick INTEGER,
    after_tick INTEGER,
    status TEXT NOT NULL,
    started_unix_ms INTEGER,
    finished_unix_ms INTEGER,
    source_path TEXT,
    start_byte INTEGER,
    end_byte INTEGER,
    payload_json TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (run_id, sequence),
    FOREIGN KEY (run_id, parent_sequence)
        REFERENCES script_steps(run_id, sequence) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS frames (
    frame_pk INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
    run_id TEXT REFERENCES script_runs(run_id) ON DELETE SET NULL,
    sequence INTEGER NOT NULL,
    tick INTEGER NOT NULL,
    frame_id TEXT NOT NULL,
    state_hash TEXT NOT NULL,
    render_hash TEXT NOT NULL,
    source TEXT NOT NULL,
    capture_time_millis INTEGER,
    observation_json TEXT NOT NULL,
    created_unix_ms INTEGER NOT NULL,
    UNIQUE (session_id, tick, frame_id)
);

CREATE INDEX IF NOT EXISTS frames_run_sequence
    ON frames(run_id, sequence);
CREATE INDEX IF NOT EXISTS frames_state_hash
    ON frames(state_hash);

CREATE TABLE IF NOT EXISTS actions (
    action_pk INTEGER PRIMARY KEY,
    run_id TEXT REFERENCES script_runs(run_id) ON DELETE SET NULL,
    session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    before_tick INTEGER,
    after_tick INTEGER,
    dispatch TEXT NOT NULL,
    action_kind TEXT NOT NULL,
    target TEXT,
    accepted INTEGER CHECK (accepted IN (0, 1)),
    request_json TEXT NOT NULL,
    result_json TEXT,
    created_unix_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS actions_run_sequence
    ON actions(run_id, sequence);
CREATE INDEX IF NOT EXISTS actions_target
    ON actions(target, action_kind);

CREATE TABLE IF NOT EXISTS diagnostics (
    diagnostic_id TEXT PRIMARY KEY,
    program_id INTEGER REFERENCES programs(program_id) ON DELETE CASCADE,
    session_id TEXT REFERENCES sessions(session_id) ON DELETE CASCADE,
    run_id TEXT REFERENCES script_runs(run_id) ON DELETE CASCADE,
    sequence INTEGER,
    code TEXT,
    severity TEXT NOT NULL,
    phase TEXT NOT NULL,
    message TEXT NOT NULL,
    source_path TEXT,
    start_byte INTEGER,
    end_byte INTEGER,
    related_ids_json TEXT NOT NULL DEFAULT '[]',
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_unix_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS diagnostics_run_severity
    ON diagnostics(run_id, severity, sequence);
CREATE INDEX IF NOT EXISTS diagnostics_code
    ON diagnostics(code, severity);

CREATE TABLE IF NOT EXISTS blobs (
    blob_hash TEXT PRIMARY KEY,
    media_type TEXT NOT NULL,
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    relative_path TEXT NOT NULL UNIQUE,
    privacy_class TEXT NOT NULL,
    created_unix_ms INTEGER NOT NULL,
    last_access_unix_ms INTEGER NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS captures (
    capture_id TEXT PRIMARY KEY,
    run_id TEXT REFERENCES script_runs(run_id) ON DELETE SET NULL,
    session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    tick INTEGER NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_id TEXT,
    capture_kind TEXT NOT NULL,
    renderer TEXT NOT NULL,
    composition TEXT NOT NULL,
    blob_hash TEXT REFERENCES blobs(blob_hash) ON DELETE SET NULL,
    resource_uri TEXT NOT NULL,
    width INTEGER NOT NULL CHECK (width >= 0),
    height INTEGER NOT NULL CHECK (height >= 0),
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_unix_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS captures_run_sequence
    ON captures(run_id, sequence);
CREATE INDEX IF NOT EXISTS captures_scope
    ON captures(session_id, tick, scope_kind, scope_id);

CREATE TABLE IF NOT EXISTS chunks (
    chunk_id TEXT NOT NULL UNIQUE,
    program_id INTEGER REFERENCES programs(program_id) ON DELETE CASCADE,
    session_id TEXT REFERENCES sessions(session_id) ON DELETE CASCADE,
    run_id TEXT REFERENCES script_runs(run_id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL,
    source_key TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    semantic_hash TEXT,
    source_path TEXT,
    entity_ids_json TEXT NOT NULL DEFAULT '[]',
    start_byte INTEGER,
    end_byte INTEGER,
    privacy_class TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_unix_ms INTEGER NOT NULL,
    CHECK (start_byte IS NULL OR start_byte >= 0),
    CHECK (end_byte IS NULL OR end_byte >= start_byte)
);

CREATE INDEX IF NOT EXISTS chunks_program_source
    ON chunks(program_id, source_kind, source_key);
CREATE INDEX IF NOT EXISTS chunks_content_hash
    ON chunks(content_hash);
CREATE INDEX IF NOT EXISTS chunks_semantic_hash
    ON chunks(program_id, semantic_hash)
    WHERE semantic_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS chunks_privacy
    ON chunks(privacy_class);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    title,
    body,
    content = 'chunks',
    content_rowid = 'rowid',
    tokenize = 'trigram'
);

CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, title, body)
    VALUES (new.rowid, new.title, new.body);
END;

CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, title, body)
    VALUES ('delete', old.rowid, old.title, old.body);
END;

CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, title, body)
    VALUES ('delete', old.rowid, old.title, old.body);
    INSERT INTO chunks_fts(rowid, title, body)
    VALUES (new.rowid, new.title, new.body);
END;

CREATE TABLE IF NOT EXISTS embeddings (
    chunk_id TEXT NOT NULL REFERENCES chunks(chunk_id) ON DELETE CASCADE,
    model_id TEXT NOT NULL,
    model_revision TEXT NOT NULL,
    dimensions INTEGER NOT NULL CHECK (dimensions > 0),
    original_norm REAL NOT NULL CHECK (original_norm > 0.0),
    vector_le_f32 BLOB NOT NULL,
    content_hash TEXT NOT NULL,
    created_unix_ms INTEGER NOT NULL,
    PRIMARY KEY (chunk_id, model_id, model_revision, dimensions)
);

CREATE INDEX IF NOT EXISTS embeddings_model
    ON embeddings(model_id, model_revision, dimensions);

CREATE TABLE IF NOT EXISTS history_entries (
    history_id TEXT PRIMARY KEY,
    program_id INTEGER REFERENCES programs(program_id) ON DELETE CASCADE,
    symbol_id TEXT REFERENCES symbols(symbol_id) ON DELETE SET NULL,
    change_id TEXT NOT NULL,
    operation_id TEXT,
    ordinal INTEGER NOT NULL,
    semantic_hash_before TEXT,
    semantic_hash_after TEXT,
    summary TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_unix_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS history_symbol_ordinal
    ON history_entries(symbol_id, ordinal DESC);
CREATE INDEX IF NOT EXISTS history_change
    ON history_entries(change_id, ordinal);

CREATE TABLE IF NOT EXISTS test_results (
    test_result_id TEXT PRIMARY KEY,
    program_id INTEGER REFERENCES programs(program_id) ON DELETE CASCADE,
    run_id TEXT REFERENCES script_runs(run_id) ON DELETE SET NULL,
    test_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    outcome TEXT NOT NULL,
    duration_millis INTEGER,
    diagnostic_ids_json TEXT NOT NULL DEFAULT '[]',
    artifact_refs_json TEXT NOT NULL DEFAULT '[]',
    summary TEXT NOT NULL DEFAULT '',
    created_unix_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS test_results_test_outcome
    ON test_results(test_id, outcome, created_unix_ms DESC);

CREATE TABLE IF NOT EXISTS rag_queries (
    query_id TEXT PRIMARY KEY,
    program_id INTEGER REFERENCES programs(program_id) ON DELETE CASCADE,
    session_id TEXT REFERENCES sessions(session_id) ON DELETE SET NULL,
    run_id TEXT REFERENCES script_runs(run_id) ON DELETE SET NULL,
    query_text TEXT NOT NULL,
    query_hash TEXT NOT NULL,
    model_id TEXT,
    model_revision TEXT,
    policy_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL,
    created_unix_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS rag_query_hits (
    query_id TEXT NOT NULL REFERENCES rag_queries(query_id) ON DELETE CASCADE,
    chunk_id TEXT NOT NULL REFERENCES chunks(chunk_id) ON DELETE CASCADE,
    channel TEXT NOT NULL,
    channel_rank INTEGER NOT NULL CHECK (channel_rank > 0),
    channel_score REAL,
    fused_score REAL NOT NULL,
    selected INTEGER NOT NULL DEFAULT 0 CHECK (selected IN (0, 1)),
    explanation_json TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (query_id, chunk_id, channel)
);

CREATE INDEX IF NOT EXISTS rag_query_hits_fused
    ON rag_query_hits(query_id, fused_score DESC, chunk_id);

CREATE TABLE IF NOT EXISTS repl_cells (
    cell_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE,
    run_id TEXT REFERENCES script_runs(run_id) ON DELETE SET NULL,
    ordinal INTEGER NOT NULL,
    source TEXT NOT NULL,
    source_hash TEXT NOT NULL,
    status TEXT NOT NULL,
    inferred_type_json TEXT,
    display_json TEXT,
    partially_effectful INTEGER NOT NULL DEFAULT 0 CHECK (partially_effectful IN (0, 1)),
    diagnostic_ids_json TEXT NOT NULL DEFAULT '[]',
    created_unix_ms INTEGER NOT NULL,
    UNIQUE (session_id, ordinal)
);

INSERT OR IGNORE INTO schema_migrations(version, name, applied_unix_ms)
VALUES (1, 'debug_store_v1', 0);

PRAGMA user_version = 1;
