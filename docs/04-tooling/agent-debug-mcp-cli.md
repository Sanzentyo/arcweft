# Agent Debug Bus / MCP / CLI

## Agent Debug Bus

Verifier diagnostics and actions use a stable JSON schema consumed by CLI and
LSP. `arcweft-agent-protocol` owns the typed observation JSON exposed by the
CLI and MCP adapters. The normative observation/capture shape, scope, URI, raw
RGBA, and coordinate-space contract is in [Agent Observe and Capture Contract](agent-observe-capture-contract.md).
Command syntax and current CLI behavior are documented in [CLI](cli.md).
`arcweft-agent-mcp` maps the shared protocol to MCP resource and tool-result JSON.

`arcw agent mcp` is the current minimal stdio transport. It accepts line-delimited
JSON-RPC requests for `initialize`, `tools/list`, `tools/call`, `resources/list`,
`resources/templates/list`, and `resources/read`. `resources/templates/list`
advertises the stable `arcweft://` URI families for presentation-tree filters,
viewport, layer, object, and rich-text child captures before a source has been
observed. `tools/call`
supports `arcweft.observe`, `arcweft.capture`, `arcweft.resource.read`,
`arcweft.session.info`, `arcweft.action`, `arcweft.hit_test`, and
`arcweft.debug.search`; the server keeps the latest
one-shot observation in memory so clients can observe once, inspect the current
session/frame/resource state, list resources, then either call
`arcweft.capture` for a viewport/layer/object PNG or raw RGBA image content,
dispatch one enabled semantic `arcweft.action`,
call `arcweft.hit_test` for a depth-sorted object hit list at a viewport
coordinate, or read a specific object/layer/rich-text child image URI.
Hit-test hits include each selected object's `capture_refs`, so clients can
follow a hit directly to color, object-id, or mask captures without separately
walking the full observation object list.
`arcweft.capture` can also take a listed image `uri` directly,
using that URI to infer format, capture kind, and viewport/layer/object scope.
Passing `source`, or passing `manifest` plus `profile`, to `arcweft.capture` or
`arcweft.hit_test` runs a bounded observation first and then returns the
requested result, which gives LLM debuggers a single-call path for "show me this
layer/object" and "what did I click here" workflows while still updating
`resources/list`.
`arcweft.observe` uses the same direct-source versus launch-profile selection
model as `arcw agent observe`, so the profile-owned dialogue View, Style, and
inline-failure policy, together with adapter context, are available through MCP
without also supplying a source path. Capture resource shape, URI families,
retention, and animation sampling are defined by the
[Agent Observe and Capture Contract](agent-observe-capture-contract.md).
`arcweft.session.info` returns the latest frame identifiers, resources, images,
observed layers, observed objects, resource templates, and latest capture
metadata so a debugger can recover current capture options, object ids, and
rich-text child capture refs without rereading the whole observation. It also
includes `latest_capture_uri` and a `latest_capture_resource` descriptor, so a
client can immediately read or recapture the most recent PNG/raw image without
matching it against `resources/list`. `capture_resource_count` records how many
tool-produced capture resources are currently cached in the session.
`arcweft.action` runs against the same persistent native runtime state as
`arcweft.observe`. It accepts either an observed `action_id` or direct semantic
`kind` (`advance_text`, `select_choice`, `invoke`, or `scroll`) with required
target fields, accepts optional JSON object `args` for direct invoke calls, validates
that the requested semantic action is currently enabled, dispatches the matching
runtime input, then returns accepted before/after ticks, state hashes, and the
post-action frame summary. Pointer-click synthesis remains outside the current
MCP action surface. Scroll accepts an observed `region` plus signed i32
`delta_x_milli` / `delta_y_milli`, converts them to logical pixels, and routes
through the same live prepared-frame input controller used by native scrolling.

Agent execution traces use the same resource conversion path. The protocol
surface exposes `AgentResourceKind::Trace`, and `arcweft-agent-mcp` maps typed
`AgentTraceRecord` arrays to `arcweft://run/{run_id}/trace.arcwx` with
`application/vnd.arcweft.agent-trace+json`. The stdio MCP server exposes
`arcweft.trace.read`, which validates a `.arcwx` file, caches the trace resource
in the current MCP session, and serves it through `resources/list`,
`resources/read`, and `arcweft.resource.read` without requiring a prior native
observation. Capture trace records carry `blob_refs` copied from the capture
content hash, and `.arcwx` validation rejects capture records whose blob refs do
not include that captured content hash.

`arcweft.debug.search` is the MCP surface for the rebuildable Agent debug
SQLite store. It accepts exactly one selector: `query` for literal FTS5 chunk
search, `query_vector` plus `model_id` and `model_revision` for stored-vector
search, `graph_query` with optional `graph_depth` for symbol/edge graph search,
or `history_query` for indexed history search. Optional `path`, `limit`, and
`max_privacy` apply to every channel; `path` defaults to
`.arcweft/cache/agent-debug.sqlite3`. Privacy filtering happens before limiting
and ranking for the relevant channel. The tool returns chunk ids, title/body
text, source kind/key, privacy, search channel, rank, and score. It does not
require an observation or trace to be cached in the MCP session.
`arcweft.rag.query` can also opt into the same local stored-vector channel with
`local_embedding = true`, `path`, and optional
`local_embedding_model_id` / `local_embedding_model_revision` /
`local_embedding_dimensions`. The query text is hashed through the deterministic
local embedding adapter, matching rows are fused into the returned
`RagContextPack`, and missing embeddings fall back to the existing
lexical/graph/history/diagnostic/test-result channels. That local fallback is
recorded as an `AGENT_RAG_EMBEDDING_FALLBACK` warning diagnostic in the same
debug store for later `arcweft.debug.search` or `debug db search` readback.
`arcweft.debug.script.runs` reads persisted Agent Script lifecycle rows from
the same SQLite store. It accepts optional `path`, `session_id`, and `limit`,
orders recent runs by their session-local start sequence, and returns run id,
session id, Agent id, source/artifact hashes, project binding mode, sequence
range, trace URI, outcome, effectfulness, error, and deterministic metadata.
It is the MCP counterpart to `arcw debug db runs`.
`arcweft.debug.repl.cells` reads persisted Agent REPL cells from the same store
for a required `session_id`, returning cell source, stable source hash, status,
display JSON, diagnostics, and effectfulness. It mirrors
`arcw debug db repl-cells`.


## Tool catalog

Tools:

```text
arcweft.observe
arcweft.action
arcweft.act
arcweft.wait
arcweft.session.step_frames
arcweft.script.run
arcweft.capture
arcweft.resource.read
arcweft.hit_test
arcweft.session.info
arcweft.get_state
arcweft.signal_get
arcweft.log_query
arcweft.rag.query
arcweft.rag.explain
arcweft.rag.context.read
arcweft.debug.search
arcweft.debug.script.runs
arcweft.debug.session.timeline
arcweft.debug.repl.cells
arcweft.trace.read
```

`arcweft.act` is the typed action alias for clients that use the shorter Agent
verb. It shares the same input schema, capability checks, and result contract as
`arcweft.action`. `arcweft.session.step_frames` advances the persistent native
runtime by an explicit frame count and returns the resulting frame summary;
clients should use it instead of emulating deterministic stepping through wait
or observe polling.

Observation-derived state, signal, and log readback accepts `max_privacy`
(`public`, `project`, `sensitive`, or `secret`). The default is `project`;
`max_privacy=public` blocks cached observation debug data rather than returning
project-private payloads.

## Product security

Product and HTTP Agent access, local-token requirements, auditing, capability
tiers, and developer-tool restrictions are owned by
[Security and sandbox](../05-build-and-security/security.md).
