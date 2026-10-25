# Graph / RAG / Git

## Source of truth

- Arcweft source and project inputs are versioned in Git.
- Git commit object IDs identify repository revisions.
- Project hashes, source-content hashes, semantic hashes, and graph-row IDs
  identify indexed Arcweft data; none replaces the Git revision.
- Graph, history, and RAG records in the Agent debug store are rebuildable
  indexes derived from accepted project inputs.

The CLI owns source/project indexing and debug-store queries. See
[CLI](cli.md) and [Agent Debug Bus / MCP / CLI](agent-debug-mcp-cli.md) for
current commands and MCP access.

## Graph nodes

The Agent debug store indexes project symbols as rebuildable graph rows. The
current row types and their field ownership are defined by
[`arcweft-debug-model::graph`](../../crates/arcweft-debug-model/src/graph.rs).
Source indexing derives symbols from the accepted project semantic index and
records their source path, source-content hash, byte span, semantic hash, and
summary when available. `kind` is an index value, not a closed node enum in
this store contract.

## Edges

Directed graph rows refer to indexed symbol IDs and carry an edge kind, weight,
and metadata. The row contract is owned by
[`arcweft-debug-model::graph`](../../crates/arcweft-debug-model/src/graph.rs).
The source index derives current relationships from accepted project entities
and actions. Privacy filtering applies before graph results are limited and
ranked.

## GraphPatch

The language contract includes `GraphPatch` as a structured
`PrecompileOutput` variant. Its owner is
[Precompile output](../01-language/macros-precompile.md#出力); this chapter does
not duplicate its operation schema. A patch operation ID is scoped to the patch
and is not a repository revision.

## Git and entity history

Git remains the authority for repository revision history. For example,
`70e24164373e7898ff9ef83f56f4c48523ce108e` is a full 40-character Git commit
object ID.

The debug store's current history row is owned by
[`arcweft-debug-model::history`](../../crates/arcweft-debug-model/src/history.rs).
It indexes a change ID, optional operation ID, before/after semantic hashes,
summary, and metadata. These rebuildable rows support history search; they are
not a replacement for `git log`.

## RAG

RAG combines exact-entity, lexical, vector, graph, history, diagnostic, trace,
and summary candidates. Privacy filtering occurs before rank limiting and
context-byte trimming. Source/project indexing and query options are documented
in [CLI](cli.md); MCP tools are documented in
[Agent Debug Bus / MCP / CLI](agent-debug-mcp-cli.md).

The current query and result types are owned by
[`arcweft-debug-model::rag`](../../crates/arcweft-debug-model/src/rag.rs).
A version-1 `RagContextPack` contains the query, selected context items, and a
truncation flag. Each item carries its source kind, title and body, fused score,
contributing channels, entity IDs, and optional source anchor.
