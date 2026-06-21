use arcweft_debug_model::{
    chunk::{ChunkId, PrivacyClass},
    rag::{SearchChannel, SearchHit},
};

use super::ChunkSearchResult;

#[derive(Debug)]
pub(crate) struct GraphSearchRow {
    pub(crate) edge_id: i64,
    pub(crate) edge_kind: String,
    pub(crate) weight: f64,
    pub(crate) distance: i32,
    pub(crate) from_symbol_id: String,
    pub(crate) from_public_id: Option<String>,
    pub(crate) from_qualified_name: Option<String>,
    pub(crate) from_kind: String,
    pub(crate) from_summary: String,
    pub(crate) to_symbol_id: String,
    pub(crate) to_public_id: Option<String>,
    pub(crate) to_qualified_name: Option<String>,
    pub(crate) to_kind: String,
    pub(crate) to_summary: String,
}

#[derive(Debug)]
pub(crate) struct GraphSymbolSearchRow {
    pub(crate) symbol_id: String,
    pub(crate) public_id: Option<String>,
    pub(crate) qualified_name: Option<String>,
    pub(crate) kind: String,
    pub(crate) summary: String,
    pub(crate) semantic_hash: Option<String>,
    pub(crate) start_byte: Option<i64>,
    pub(crate) end_byte: Option<i64>,
}

pub(crate) fn graph_chunk_search_result(
    query: &str,
    index: usize,
    row: &GraphSearchRow,
) -> ChunkSearchResult {
    let from_label = graph_symbol_label(
        &row.from_symbol_id,
        row.from_public_id.as_deref(),
        row.from_qualified_name.as_deref(),
    );
    let to_label = graph_symbol_label(
        &row.to_symbol_id,
        row.to_public_id.as_deref(),
        row.to_qualified_name.as_deref(),
    );
    let title = format!("{from_label} --{}--> {to_label}", row.edge_kind);
    let body = format!(
        "edge_kind={}\nweight={:.6}\ndistance={}\nfrom_kind={}\nfrom_summary={}\nto_kind={}\nto_summary={}",
        row.edge_kind,
        row.weight,
        row.distance,
        row.from_kind,
        row.from_summary,
        row.to_kind,
        row.to_summary
    );
    ChunkSearchResult {
        hit: SearchHit {
            chunk_id: ChunkId::new(format!("graph:{}", row.edge_id)),
            channel: SearchChannel::Graph,
            rank: index + 1,
            score: Some(graph_score(query, row)),
        },
        title,
        body,
        source_kind: "graph_edge".to_owned(),
        source_key: row.edge_id.to_string(),
        privacy: PrivacyClass::Project,
    }
}

pub(crate) fn graph_symbol_chunk_search_result(
    query: &str,
    index: usize,
    row: &GraphSymbolSearchRow,
) -> ChunkSearchResult {
    let label = graph_symbol_label(
        &row.symbol_id,
        row.public_id.as_deref(),
        row.qualified_name.as_deref(),
    );
    let body = format!(
        "symbol_id={}\nkind={}\nsummary={}\nsemantic_hash={}\nstart_byte={}\nend_byte={}",
        row.symbol_id,
        row.kind,
        row.summary,
        row.semantic_hash.as_deref().unwrap_or("-"),
        row.start_byte
            .map_or_else(|| "-".to_owned(), |value| value.to_string()),
        row.end_byte
            .map_or_else(|| "-".to_owned(), |value| value.to_string())
    );
    ChunkSearchResult {
        hit: SearchHit {
            chunk_id: ChunkId::new(format!("graph_symbol:{}", row.symbol_id)),
            channel: SearchChannel::Graph,
            rank: index + 1,
            score: Some(graph_symbol_score(query, row)),
        },
        title: format!("Graph symbol {label}"),
        body,
        source_kind: "graph_symbol".to_owned(),
        source_key: row.symbol_id.clone(),
        privacy: PrivacyClass::Project,
    }
}

fn graph_symbol_label(
    symbol_id: &str,
    public_id: Option<&str>,
    qualified_name: Option<&str>,
) -> String {
    public_id.or(qualified_name).unwrap_or(symbol_id).to_owned()
}

fn graph_score(query: &str, row: &GraphSearchRow) -> f64 {
    let query = query.trim().to_lowercase();
    let base = if row
        .from_public_id
        .as_deref()
        .is_some_and(|id| id.eq_ignore_ascii_case(&query))
        || row
            .to_public_id
            .as_deref()
            .is_some_and(|id| id.eq_ignore_ascii_case(&query))
        || row.edge_kind.eq_ignore_ascii_case(&query)
    {
        row.weight + 2.0
    } else if row.from_summary.to_lowercase().contains(&query)
        || row.to_summary.to_lowercase().contains(&query)
    {
        row.weight + 1.0
    } else {
        row.weight
    };
    base / f64::from(row.distance.max(0) + 1)
}

fn graph_symbol_score(query: &str, row: &GraphSymbolSearchRow) -> f64 {
    let query = query.trim().to_lowercase();
    if row
        .public_id
        .as_deref()
        .is_some_and(|id| id.eq_ignore_ascii_case(&query))
        || row
            .qualified_name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(&query))
    {
        2.0
    } else if row.summary.to_lowercase().contains(&query) {
        1.0
    } else {
        0.5
    }
}

#[derive(Clone, Copy)]
pub(crate) struct DiagnosticSearchBodyFields<'a> {
    pub(crate) phase: &'a str,
    pub(crate) message: &'a str,
    pub(crate) source_path: Option<&'a str>,
    pub(crate) start_byte: Option<i64>,
    pub(crate) end_byte: Option<i64>,
    pub(crate) sequence: Option<i64>,
    pub(crate) related_ids_json: &'a str,
    pub(crate) payload_json: &'a str,
}

pub(crate) fn diagnostic_search_body(fields: DiagnosticSearchBodyFields<'_>) -> String {
    let mut lines = vec![format!("phase={}", fields.phase), fields.message.to_owned()];
    if let Some(sequence) = fields.sequence {
        lines.push(format!("sequence={sequence}"));
    }
    if let Some(source_path) = fields.source_path {
        lines.push(format!("source_path={source_path}"));
    }
    if let (Some(start), Some(end)) = (fields.start_byte, fields.end_byte) {
        lines.push(format!("range={start}..{end}"));
    }
    if fields.related_ids_json != "[]" {
        lines.push(format!("related_ids={}", fields.related_ids_json));
    }
    if fields.payload_json != "{}" {
        lines.push(format!("payload={}", fields.payload_json));
    }
    lines.join("\n")
}

pub(crate) fn diagnostic_score(query: &str, code: Option<&str>, severity: &str, body: &str) -> f64 {
    let query = query.trim().to_lowercase();
    let exact = if code.is_some_and(|code| code.eq_ignore_ascii_case(&query)) {
        4.0
    } else {
        0.0
    };
    let severity_boost = match severity {
        "error" => 2.0,
        "warning" => 1.0,
        _ => 0.0,
    };
    let body_match = if body.to_lowercase().contains(&query) {
        1.0
    } else {
        0.0
    };
    exact + severity_boost + body_match
}

pub(crate) fn test_result_search_body(
    duration_millis: Option<i64>,
    diagnostic_ids_json: &str,
    artifact_refs_json: &str,
    summary: &str,
) -> String {
    let mut lines = Vec::new();
    if let Some(duration) = duration_millis {
        lines.push(format!("duration_millis={duration}"));
    }
    if diagnostic_ids_json != "[]" {
        lines.push(format!("diagnostic_ids={diagnostic_ids_json}"));
    }
    if artifact_refs_json != "[]" {
        lines.push(format!("artifact_refs={artifact_refs_json}"));
    }
    if !summary.is_empty() {
        lines.push(summary.to_owned());
    }
    lines.join("\n")
}

pub(crate) fn test_result_score(query: &str, test_id: &str, outcome: &str, body: &str) -> f64 {
    let query = query.trim().to_lowercase();
    let exact = if test_id.eq_ignore_ascii_case(&query) {
        4.0
    } else {
        0.0
    };
    let outcome_boost = match outcome {
        "failed" | "error" => 2.0,
        "flaky" => 1.0,
        _ => 0.0,
    };
    let body_match = if body.to_lowercase().contains(&query) {
        1.0
    } else {
        0.0
    };
    exact + outcome_boost + body_match
}
