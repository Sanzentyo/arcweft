use arcweft_debug_model::{
    chunk::{ChunkSourceKind, PrivacyClass},
    rag::SearchChannel,
};

use super::DebugStoreError;

pub(crate) fn quote_fts_literal(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

pub(crate) fn debug_event_payload_privacy(payload: &serde_json::Value) -> PrivacyClass {
    payload
        .get("privacy_class")
        .or_else(|| payload.get("privacy"))
        .or_else(|| {
            payload
                .get("payload")
                .and_then(|value| value.get("privacy_class"))
        })
        .or_else(|| {
            payload
                .get("payload")
                .and_then(|value| value.get("privacy"))
        })
        .and_then(serde_json::Value::as_str)
        .and_then(PrivacyClass::parse)
        .unwrap_or(PrivacyClass::Project)
}

pub(crate) const fn search_channel_label(channel: SearchChannel) -> &'static str {
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

pub(crate) fn parse_search_channel(value: &str) -> Option<SearchChannel> {
    match value {
        "exact_entity" => Some(SearchChannel::ExactEntity),
        "lexical" => Some(SearchChannel::Lexical),
        "vector" => Some(SearchChannel::Vector),
        "graph" => Some(SearchChannel::Graph),
        "history" => Some(SearchChannel::History),
        "diagnostics" => Some(SearchChannel::Diagnostics),
        "trace" => Some(SearchChannel::Trace),
        "summary" => Some(SearchChannel::Summary),
        _ => None,
    }
}

pub(crate) fn parse_chunk_source_kind(value: &str) -> Option<ChunkSourceKind> {
    match value {
        "source" => Some(ChunkSourceKind::Source),
        "symbol" => Some(ChunkSourceKind::Symbol),
        "graph_summary" => Some(ChunkSourceKind::GraphSummary),
        "diagnostic" => Some(ChunkSourceKind::Diagnostic),
        "test_result" => Some(ChunkSourceKind::TestResult),
        "agent_trace" => Some(ChunkSourceKind::AgentTrace),
        "history" => Some(ChunkSourceKind::History),
        "documentation" => Some(ChunkSourceKind::Documentation),
        _ => None,
    }
}

pub(crate) fn delete_count(count: usize, column: &'static str) -> Result<u64, DebugStoreError> {
    u64::try_from(count).map_err(|_| DebugStoreError::IntegerOverflow(column))
}
