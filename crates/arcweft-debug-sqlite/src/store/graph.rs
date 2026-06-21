use std::collections::BTreeSet;

use arcweft_agent_protocol::ids::{PublicId, StableHash};
use arcweft_debug_model::{
    chunk::PrivacyClass,
    graph::{DebugGraphEdge, DebugGraphSymbol},
};
use rusqlite::params;

use super::DebugStore;
use super::{
    ChunkSearchResult, DebugStoreError,
    convert::{
        debug_graph_edge_from_raw, debug_graph_symbol_from_raw, raw_debug_graph_edge_from_row,
        raw_debug_graph_symbol_from_row, sqlite_i64,
    },
    search::{
        GraphSearchRow, GraphSymbolSearchRow, graph_chunk_search_result,
        graph_symbol_chunk_search_result,
    },
};

impl DebugStore {
    pub fn upsert_graph_symbol(&self, symbol: &DebugGraphSymbol) -> Result<(), DebugStoreError> {
        let program_id = self.require_program_id(&symbol.program_hash)?;
        let source_file_id = match (&symbol.source_path, &symbol.source_content_hash) {
            (Some(path), Some(content_hash)) => {
                self.source_file_id(program_id, path.as_str(), content_hash.as_str())?
            }
            _ => None,
        };
        let start_byte = symbol
            .start_byte
            .map(|value| sqlite_i64(value, "symbols.start_byte"))
            .transpose()?;
        let end_byte = symbol
            .end_byte
            .map(|value| sqlite_i64(value, "symbols.end_byte"))
            .transpose()?;
        self.connection.execute(
            "INSERT INTO symbols(
               symbol_id, program_id, public_id, qualified_name, kind, type_json,
               source_file_id, start_byte, end_byte, semantic_hash, summary, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(symbol_id) DO UPDATE SET
               program_id = excluded.program_id,
               public_id = excluded.public_id,
               qualified_name = excluded.qualified_name,
               kind = excluded.kind,
               type_json = excluded.type_json,
               source_file_id = excluded.source_file_id,
               start_byte = excluded.start_byte,
               end_byte = excluded.end_byte,
               semantic_hash = excluded.semantic_hash,
               summary = excluded.summary,
               metadata_json = excluded.metadata_json",
            params![
                &symbol.symbol_id,
                program_id,
                symbol.public_id.as_ref().map(PublicId::as_str),
                symbol.qualified_name.as_deref(),
                &symbol.kind,
                symbol
                    .type_json
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                source_file_id,
                start_byte,
                end_byte,
                symbol.semantic_hash.as_ref().map(StableHash::as_str),
                &symbol.summary,
                serde_json::to_string(&symbol.metadata)?,
            ],
        )?;
        Ok(())
    }

    pub fn upsert_graph_edge(&self, edge: &DebugGraphEdge) -> Result<(), DebugStoreError> {
        let program_id = self.require_program_id(&edge.program_hash)?;
        self.connection.execute(
            "INSERT INTO graph_edges(
               program_id, from_symbol_id, to_symbol_id, edge_kind, weight, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(program_id, from_symbol_id, to_symbol_id, edge_kind) DO UPDATE SET
               weight = excluded.weight,
               metadata_json = excluded.metadata_json",
            params![
                program_id,
                &edge.from_symbol_id,
                &edge.to_symbol_id,
                &edge.edge_kind,
                edge.weight,
                serde_json::to_string(&edge.metadata)?,
            ],
        )?;
        Ok(())
    }

    pub fn graph_symbols_for_program(
        &self,
        program_hash: &StableHash,
    ) -> Result<Vec<DebugGraphSymbol>, DebugStoreError> {
        let Some(program_id) = self.program_id(program_hash)? else {
            return Ok(Vec::new());
        };
        let mut statement = self.connection.prepare(
            "SELECT p.program_hash, s.symbol_id, s.public_id, s.qualified_name,
                    s.kind, s.type_json, sf.path, sf.content_hash,
                    s.start_byte, s.end_byte, s.semantic_hash, s.summary,
                    s.metadata_json
             FROM symbols AS s
             JOIN programs AS p ON p.program_id = s.program_id
             LEFT JOIN source_files AS sf ON sf.source_file_id = s.source_file_id
             WHERE s.program_id = ?1
             ORDER BY s.kind ASC,
                      COALESCE(s.public_id, s.qualified_name, s.symbol_id) ASC,
                      s.symbol_id ASC",
        )?;
        let rows = statement.query_map([program_id], raw_debug_graph_symbol_from_row)?;
        rows.map(|row| {
            row.map_err(DebugStoreError::from)
                .and_then(debug_graph_symbol_from_raw)
        })
        .collect()
    }

    pub fn graph_edges_for_program(
        &self,
        program_hash: &StableHash,
    ) -> Result<Vec<DebugGraphEdge>, DebugStoreError> {
        let Some(program_id) = self.program_id(program_hash)? else {
            return Ok(Vec::new());
        };
        let mut statement = self.connection.prepare(
            "SELECT p.program_hash, ge.from_symbol_id, ge.to_symbol_id,
                    ge.edge_kind, ge.weight, ge.metadata_json
             FROM graph_edges AS ge
             JOIN programs AS p ON p.program_id = ge.program_id
             WHERE ge.program_id = ?1
             ORDER BY ge.from_symbol_id ASC, ge.edge_kind ASC, ge.to_symbol_id ASC",
        )?;
        let rows = statement.query_map([program_id], raw_debug_graph_edge_from_row)?;
        rows.map(|row| {
            row.map_err(DebugStoreError::from)
                .and_then(debug_graph_edge_from_raw)
        })
        .collect()
    }

    pub fn graph_search_with_max_privacy(
        &self,
        query: &str,
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
        self.graph_search_with_depth_and_max_privacy(query, 1, limit, max_privacy)
    }

    pub fn graph_search_with_depth_and_max_privacy(
        &self,
        query: &str,
        graph_depth: u32,
        limit: usize,
        max_privacy: PrivacyClass,
    ) -> Result<Vec<ChunkSearchResult>, DebugStoreError> {
        if query.trim().is_empty()
            || limit == 0
            || !PrivacyClass::Project.is_allowed_by(max_privacy)
        {
            return Ok(Vec::new());
        }
        let edge_rows = self.graph_search_rows(query, graph_depth, limit)?;
        let mut excluded_symbol_ids = BTreeSet::new();
        for row in &edge_rows {
            excluded_symbol_ids.insert(row.from_symbol_id.clone());
            excluded_symbol_ids.insert(row.to_symbol_id.clone());
        }
        let mut results = edge_rows
            .iter()
            .enumerate()
            .map(|(index, row)| graph_chunk_search_result(query, index, row))
            .collect::<Vec<_>>();
        let remaining = limit.saturating_sub(results.len());
        if remaining > 0 {
            let base_rank = results.len();
            results.extend(
                self.graph_symbol_search_rows(query)?
                    .into_iter()
                    .filter(|row| !excluded_symbol_ids.contains(&row.symbol_id))
                    .take(remaining)
                    .enumerate()
                    .map(|(index, row)| {
                        graph_symbol_chunk_search_result(query, base_rank + index, &row)
                    }),
            );
        }
        Ok(results)
    }

    fn graph_search_rows(
        &self,
        query: &str,
        graph_depth: u32,
        limit: usize,
    ) -> Result<Vec<GraphSearchRow>, DebugStoreError> {
        let mut statement = self.connection.prepare(
            "WITH RECURSIVE frontier(symbol_id, depth) AS (
                SELECT symbol_id, 0
                FROM symbols
                WHERE instr(lower(coalesce(public_id, '')), ?1) > 0
                   OR instr(lower(coalesce(qualified_name, '')), ?1) > 0
                   OR instr(lower(kind), ?1) > 0
                   OR instr(lower(summary), ?1) > 0
                UNION
                SELECT CASE
                         WHEN ge.from_symbol_id = frontier.symbol_id THEN ge.to_symbol_id
                         ELSE ge.from_symbol_id
                       END,
                       frontier.depth + 1
                FROM frontier
                JOIN graph_edges AS ge
                  ON ge.from_symbol_id = frontier.symbol_id
                  OR ge.to_symbol_id = frontier.symbol_id
                WHERE frontier.depth < ?2
             ),
             edge_matches(edge_id, distance) AS (
                SELECT ge.edge_id, MIN(frontier.depth + 1)
                FROM graph_edges AS ge
                JOIN frontier
                  ON ge.from_symbol_id = frontier.symbol_id
                  OR ge.to_symbol_id = frontier.symbol_id
                WHERE frontier.depth < ?2
                GROUP BY ge.edge_id
                UNION ALL
                SELECT ge.edge_id, 0
                FROM graph_edges AS ge
                WHERE instr(lower(ge.edge_kind), ?1) > 0
             ),
             ranked_edges(edge_id, distance) AS (
                SELECT edge_id, MIN(distance)
                FROM edge_matches
                GROUP BY edge_id
             )
             SELECT ge.edge_id, ge.edge_kind, ge.weight, ranked_edges.distance,
                    from_symbol.symbol_id, from_symbol.public_id,
                    from_symbol.qualified_name, from_symbol.kind, from_symbol.summary,
                    to_symbol.symbol_id, to_symbol.public_id,
                    to_symbol.qualified_name, to_symbol.kind, to_symbol.summary
             FROM ranked_edges
             JOIN graph_edges AS ge ON ge.edge_id = ranked_edges.edge_id
             JOIN symbols AS from_symbol ON from_symbol.symbol_id = ge.from_symbol_id
             JOIN symbols AS to_symbol ON to_symbol.symbol_id = ge.to_symbol_id
             ORDER BY (ge.weight / (ranked_edges.distance + 1.0)) DESC,
                      ranked_edges.distance,
                      ge.edge_id
             LIMIT ?3",
        )?;
        let like_query = query.trim().to_lowercase();
        let graph_depth = i64::from(graph_depth);
        let limit = i64::try_from(limit)
            .map_err(|_| DebugStoreError::IntegerOverflow("graph_edges.limit"))?;
        let rows = statement.query_map(params![like_query, graph_depth, limit], |row| {
            Ok(GraphSearchRow {
                edge_id: row.get(0)?,
                edge_kind: row.get(1)?,
                weight: row.get(2)?,
                distance: row.get(3)?,
                from_symbol_id: row.get(4)?,
                from_public_id: row.get(5)?,
                from_qualified_name: row.get(6)?,
                from_kind: row.get(7)?,
                from_summary: row.get(8)?,
                to_symbol_id: row.get(9)?,
                to_public_id: row.get(10)?,
                to_qualified_name: row.get(11)?,
                to_kind: row.get(12)?,
                to_summary: row.get(13)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DebugStoreError::from)
    }

    fn graph_symbol_search_rows(
        &self,
        query: &str,
    ) -> Result<Vec<GraphSymbolSearchRow>, DebugStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT symbol_id, public_id, qualified_name, kind, summary,
                    semantic_hash, start_byte, end_byte
             FROM symbols
             WHERE instr(lower(coalesce(public_id, '')), ?1) > 0
                OR instr(lower(coalesce(qualified_name, '')), ?1) > 0
                OR instr(lower(kind), ?1) > 0
                OR instr(lower(summary), ?1) > 0
             ORDER BY
                CASE
                  WHEN lower(coalesce(public_id, '')) = ?1 THEN 0
                  WHEN lower(coalesce(qualified_name, '')) = ?1 THEN 1
                  ELSE 2
                END,
                symbol_id",
        )?;
        let like_query = query.trim().to_lowercase();
        let rows = statement.query_map([like_query], |row| {
            Ok(GraphSymbolSearchRow {
                symbol_id: row.get(0)?,
                public_id: row.get(1)?,
                qualified_name: row.get(2)?,
                kind: row.get(3)?,
                summary: row.get(4)?,
                semantic_hash: row.get(5)?,
                start_byte: row.get(6)?,
                end_byte: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DebugStoreError::from)
    }
}
