# Agent trace publication identity hardening

Date: 2026-07-19

## Outcome

Agent trace publication now has one checked identity boundary in
`arcweft-agent-protocol`.

- `AgentRunId` accepts only canonical `run.<segment>` identifiers. Segments use
  lowercase ASCII letters, digits, `_`, or `-`, begin and end with an
  alphanumeric character, and cannot contain URI delimiters or empty segments.
  Deserialization enforces the same grammar.
- `AgentResourceUri::new` and URI deserialization construct transport
  addresses only. Canonical-looking text never recreates trace publication
  authority.
- The protocol-owned `trace_resource` is the only constructor that seals a
  canonical address. It derives that seal from same-run typed
  `AgentTraceRecord` values and binds it to the complete serialized trace body
  digest. Copying a sealed address and changing the body, resource kind, MIME
  type, or image contract therefore cannot reuse the canonical address.
- `trace_resource` rejects a record slice containing multiple run IDs before
  JSON serialization or publication, returning a structured
  `TraceResourceError::MixedRun` with the expected run, actual run, and
  offending record index.
- The content-policy gate preserves a source URI only when the resource is a
  trace, its private seal still matches the complete resource contract, and
  publication remains an unsanitized allow. Sanitized, reviewed, blocked,
  deserialized, string-constructed, or mutated output receives a moderated
  URI.

This is a direct replacement of the provisional string boundary. No alias,
dual reader, or compatibility constructor was added.

## MCP publication lifecycle

`arcweft.session.info` publishes the current resource set once. Its
`latest_capture`, `latest_capture_uri`, and `latest_capture_resource` fields are
derived from the already-published resource cache through the source URI. It no
longer decodes, classifies, publishes, and caches the latest capture a second
time.

The regression contract verifies that the latest-capture descriptor is a
member of the session resource list, resolves to the cached publication, and
reads back through the advertised public URI.

## Privacy and audit evidence

Focused tests cover:

- exact canonical trace URI construction;
- URI and resource serialization round trips retaining the wire address while
  dropping canonical publication authority;
- rejection of malformed run identifiers and near-miss trace URIs;
- successful same-run and empty-trace construction, plus mixed-run rejection
  without producing a resource;
- canonical-looking string construction and forged-body mutation failing to
  acquire or reuse the canonical publication address;
- allowed trace publication retaining its canonical URI across MCP
  `resources/list`, cache lookup, and `resources/read`;
- sanitized trace publication receiving a moderated URI;
- `arcweft.resource.read` recording an allowed project trace read;
- a sensitive trace being denied at project privacy and recording the blocked
  audit event;
- session-info latest-capture cache reuse, descriptor membership, and
  readback.

Tier 2 MCP/Agent validation is required at broad integration cut points that
change this public resource contract. The Tier 2 harness must follow the
current canonical resource URI, accepted semantic identity, content-policy,
and authored View geometry contracts rather than preserving stale
expectations.
