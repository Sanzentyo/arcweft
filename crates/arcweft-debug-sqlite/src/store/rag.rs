use std::collections::BTreeSet;

use std::collections::BTreeMap;

use arcweft_agent_protocol::ids::{PublicId, StableHash};
use arcweft_debug_model::{
    chunk::ChunkId,
    rag::{RagContextItem, RagQuery, SearchChannel},
};

use super::{
    DebugStoreError,
    convert::{source_anchor_from_row, truncate_utf8},
    helpers::{parse_chunk_source_kind, parse_search_channel},
};

#[derive(Debug)]
pub(crate) struct RagQueryRow {
    pub(crate) query_text: String,
    pub(crate) program_hash: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) policy_json: String,
    pub(crate) status: String,
    pub(crate) created_unix_ms: i64,
}

#[derive(Debug)]
pub(crate) struct RagHitRow {
    pub(crate) chunk_id: String,
    pub(crate) source_kind: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) fused_score: f64,
    pub(crate) entity_ids_json: String,
    pub(crate) source_path: Option<String>,
    pub(crate) start_byte: Option<i64>,
    pub(crate) end_byte: Option<i64>,
    pub(crate) channel: String,
    pub(crate) channel_rank: i64,
}

#[derive(Debug)]
pub(crate) struct RagHitAccumulator {
    pub(crate) source_kind: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) fused_score: f64,
    pub(crate) entity_ids_json: String,
    pub(crate) source_path: Option<String>,
    pub(crate) start_byte: Option<i64>,
    pub(crate) end_byte: Option<i64>,
    pub(crate) channel_rank: i64,
    pub(crate) channels: BTreeSet<SearchChannel>,
}

pub(crate) fn rag_policy_roots(
    policy: &serde_json::Value,
) -> Result<Vec<PublicId>, DebugStoreError> {
    policy
        .get("roots")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(|value| PublicId::new(value.to_owned()).map_err(DebugStoreError::from))
        .collect()
}

pub(crate) fn rag_query_from_audit_row(
    query_id: &str,
    row: &RagQueryRow,
    policy: &serde_json::Value,
) -> Result<RagQuery, DebugStoreError> {
    let program_hash = row
        .program_hash
        .clone()
        .or_else(|| {
            policy
                .get("program_hash")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .ok_or_else(|| DebugStoreError::RagQueryNotIndexed(query_id.to_owned()))?;
    Ok(RagQuery {
        query_id: query_id.to_owned(),
        text: row.query_text.clone(),
        program_hash: StableHash::new(program_hash)?,
        roots: rag_policy_roots(policy)?,
        graph_depth: rag_policy_u32(policy, "graph_depth", 0)?,
        limit: rag_policy_usize(policy, "limit", usize::MAX)?,
        max_context_bytes: rag_policy_usize(policy, "max_context_bytes", usize::MAX)?,
    })
}

pub(crate) fn rag_context_items_from_hit_rows(
    query: &RagQuery,
    policy: &serde_json::Value,
    rows: Vec<RagHitRow>,
) -> Result<(Vec<RagContextItem>, bool), DebugStoreError> {
    let (mut grouped, order) = grouped_rag_hit_rows(rows)?;
    let mut used_bytes = 0usize;
    let mut truncated = policy
        .get("truncated")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let mut items = Vec::new();
    for chunk_id in order {
        let Some(accumulator) = grouped.remove(&chunk_id) else {
            continue;
        };
        if items.len() >= query.limit {
            truncated = true;
            break;
        }
        let remaining = query.max_context_bytes.saturating_sub(used_bytes);
        if remaining == 0 {
            truncated = true;
            break;
        }
        let (item, body_truncated) =
            rag_context_item_from_accumulator(chunk_id, accumulator, remaining)?;
        truncated |= body_truncated;
        used_bytes = used_bytes.saturating_add(item.body.len());
        items.push(item);
        if body_truncated {
            break;
        }
    }
    Ok((items, truncated))
}

fn grouped_rag_hit_rows(
    rows: Vec<RagHitRow>,
) -> Result<(BTreeMap<String, RagHitAccumulator>, Vec<String>), DebugStoreError> {
    let mut grouped = BTreeMap::<String, RagHitAccumulator>::new();
    let mut order = Vec::<String>::new();
    for row in rows {
        let channel = parse_search_channel(&row.channel)
            .ok_or_else(|| DebugStoreError::InvalidSearchChannel(row.channel.clone()))?;
        let entry = grouped.entry(row.chunk_id.clone()).or_insert_with(|| {
            order.push(row.chunk_id.clone());
            RagHitAccumulator {
                source_kind: row.source_kind.clone(),
                title: row.title.clone(),
                body: row.body.clone(),
                fused_score: row.fused_score,
                entity_ids_json: row.entity_ids_json.clone(),
                source_path: row.source_path.clone(),
                start_byte: row.start_byte,
                end_byte: row.end_byte,
                channel_rank: row.channel_rank,
                channels: BTreeSet::new(),
            }
        });
        entry.channel_rank = entry.channel_rank.min(row.channel_rank);
        entry.channels.insert(channel);
    }
    Ok((grouped, order))
}

fn rag_context_item_from_accumulator(
    chunk_id: String,
    accumulator: RagHitAccumulator,
    max_body_bytes: usize,
) -> Result<(RagContextItem, bool), DebugStoreError> {
    let (body, body_truncated) = truncate_utf8(&accumulator.body, max_body_bytes);
    let kind = parse_chunk_source_kind(&accumulator.source_kind)
        .ok_or_else(|| DebugStoreError::InvalidChunkSourceKind(accumulator.source_kind.clone()))?;
    let entity_ids = serde_json::from_str::<Vec<String>>(&accumulator.entity_ids_json)?
        .into_iter()
        .map(PublicId::new)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        RagContextItem {
            chunk_id: ChunkId::new(chunk_id),
            kind,
            title: accumulator.title,
            body,
            fused_score: accumulator.fused_score,
            channels: accumulator.channels,
            entity_ids,
            source_anchor: source_anchor_from_row(
                accumulator.source_path,
                accumulator.start_byte,
                accumulator.end_byte,
            )?,
        },
        body_truncated,
    ))
}

pub(crate) fn rag_policy_u32(
    policy: &serde_json::Value,
    key: &'static str,
    default: u32,
) -> Result<u32, DebugStoreError> {
    policy
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .map(|value| u32::try_from(value).map_err(|_| DebugStoreError::IntegerOverflow(key)))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

pub(crate) fn rag_policy_usize(
    policy: &serde_json::Value,
    key: &'static str,
    default: usize,
) -> Result<usize, DebugStoreError> {
    policy
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .map(|value| usize::try_from(value).map_err(|_| DebugStoreError::IntegerOverflow(key)))
        .transpose()
        .map(|value| value.unwrap_or(default))
}
