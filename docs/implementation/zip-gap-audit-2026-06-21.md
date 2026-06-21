# ZIP Gap Audit 2026-06-21

This note records the implementation cut for
`arcweft-zip-gap-audit-2026-06-21.zip`.

For the current requirement-by-requirement open-item ledger, see
`docs/implementation/zip-gap-open-items-2026-06-21.md`.

## Implemented in this cut

- Added `arcweft-agent-mcp-client`, a Sans I/O MCP-backed `AgentSession`
  adapter with typed initialize, tool validation, action alias selection,
  `step_frames`, resource readback, and in-memory contract tests.
- Added reusable `arcweft-test::agent::FixtureAgentSession` for exact
  request/response fixture vectors without putting test fixtures in runner
  production APIs.
- Added `arcweft.act` and `arcweft.session.step_frames` to the MCP descriptor
  and CLI dispatcher surfaces. The action alias shares the canonical
  `arcweft.action` schema and handler.
- Made `arcweft.session.info` include typed `AgentSessionInfo` fields while
  preserving the existing debug/resource inventory payload.
- Changed Agent REPL endpoint parsing so `stdio:` / `mcp:` endpoints are
  represented as structured `AgentReplConnection::StdioMcp` values instead of
  being classified as a package non-goal.
- Added CLI-owned `StdioMcpTransport` for line-delimited JSON-RPC MCP child
  processes and wired retained remote `McpAgentSession` connections into
  `--connect` / `:connect`. The REPL performs the remote handshake before
  swapping session state, runs remote Agent cells through `AgentRunner`, and
  routes remote `:observe` / `:capture` through typed `AgentSession` calls.
- Hardened `StdioMcpTransport` so request reads are timeout-bounded instead of
  directly blocking on child stdout, child stderr is retained as a bounded tail
  attached to transport failures, and shutdown attempts JSON-RPC `shutdown`
  plus `exit` before kill fallback.
- Extended `arcw fmt` path handling to include `.awfagent`, dispatch through
  `SourceDialect::Agent`, and reject game-only sugar rewrites for Agent sources.
- Added `arcweft-data::raw` with shape-checked raw transcoding. Type labels now
  live on `RawValue`, `Number`, and `TypeShape`; the earlier external
  `raw_type_error`/label-helper shape was removed.
- Hardened the `arcweft-data-derive` attribute parser so `#[arcweft(...)]`
  parsing returns structured `syn::Result` errors instead of discarding parse
  failures. Unknown attributes, invalid `rename_all` / `bytes` / `repr` values,
  wrong container targets, `content` without `tag`, and duplicate final wire
  names are compile errors covered by trybuild fixtures. Bare `bytes` and
  `default = "path::factory"` are accepted as part of the typed grammar.
- Hardened `arcweft-data-derive` shape policy so generated impls add
  Encode/Decode/Reflect where predicates from concrete field types rather than
  every generic type parameter, skipped generic marker fields do not require
  Encode/Decode bounds, unsupported tuple/unit structs and tuple enum surfaces
  are compile-time errors, internally tagged newtype variants are rejected at
  macro expansion, and repr enum discriminants are literal/range checked before
  shape generation.
- Added `arcweft-data::DecodeBudget` and wired Arcweft Binary decoding through
  parse-time input, node, depth, collection length, string length, and bytes
  length checks before value allocation. Arcweft Binary now rejects duplicate
  map keys and enum payload flags other than `0` or `1`.
- Hardened `arcweft-save` envelope decoding with explicit `SaveDecodeOptions`,
  envelope/schema id/codec id/payload length caps, trailing-data rejection,
  exact expected schema id checks, future-version rejection, required migration
  for older versions, migration schema/version checks, and post-migration shape
  validation.
- Added `arcweft-save` multi-step migration chains. `SaveMigrationChain`
  validates schema id consistency, strictly advancing source/target versions,
  duplicate source-version rejection, and current-version bounds before decode
  uses the chain. Save envelope v1 keeps its checksum contract payload-only;
  schema id, codec id, and schema version remain explicit header fields checked
  by the decoder rather than folded into the payload checksum contract.
- Moved `JsonCodec` encode/decode onto the shape-driven raw transcoder path for
  primitive values, records, string-key maps, options, byte fields, and the
  current external enum raw representation. JSON decoding now checks the input
  cap before parsing and uses `TypeShape`/`FieldShape` bytes policy for
  base64/hex/array byte recovery instead of the previous dynamic `Value` bridge.
- Moved `TomlCodec` encode/decode onto the shape-driven raw transcoder path for
  primitive values, records, string-key maps, byte fields, and the current
  external enum raw representation. TOML decoding now checks the input cap
  before parsing and uses TOML document parsing rather than value-only parsing.
  Because TOML has no null value, `Option::None` is encoded as record field
  omission; top-level or sequence-contained `None` is rejected as unsupported.
- Moved `YamlCodec` encode/decode onto the shape-driven raw transcoder path for
  primitive values, records, string-key maps, options, byte fields, and the
  current external enum raw representation. YAML decoding now checks the input
  cap before parsing and rejects empty or multi-document inputs instead of
  silently decoding only the first document.
- Extended the central raw shape transcoder with enum adjacent/internal tag
  styles and numeric `EnumRepr` discriminants. JSON enum encoding/decoding now
  accepts scalar repr raw values as well as map-shaped tagged enum values, and
  signed/unsigned integer recovery performs checked cross-signedness conversion
  so JSON's single integer syntax can decode into unsigned Arcweft shapes.
- Replaced the MessagePack and CBOR `serde_json::Value` bridge with native
  value-to-`RawValue` mappings. MessagePack now preserves bin values and
  signed/unsigned integer markers through `rmpv::Value`; CBOR now preserves
  byte strings and integer values through `ciborium::Value`. Both codecs check
  the input cap before parsing, reject trailing bytes after one top-level
  value, and reject extension/tag values explicitly instead of silently
  reshaping them.
- Made `CsvCodec` require a top-level `Seq<Record>` shape, derive CSV columns
  from `FieldShape` instead of observed row data, perform strict scalar cell
  conversion for bools, integers, finite floats, strings, chars, options, and
  scalar byte encodings, and reject missing/unknown/duplicate columns according
  to the record policy instead of silently dropping or stringifying data.
- Hardened numeric edge policy across the central raw shape bridge and the
  currently shape-driven codecs. `encode_with_shape` / `decode_with_shape` now
  reject non-finite floats, TOML/YAML negative integers flow through the shared
  signed-to-unsigned bounds policy instead of early invalid-type rejection, and
  JSON, TOML, YAML, MsgPack, CBOR, CSV, Arrow IPC, and Parquet carry focused tests for
  signed/unsigned crossings, out-of-range values, float-to-integer rejection,
  and NaN/infinity rejection where the format can express them.
- Made Arrow IPC and Parquet shape-driven for supported scalar row data.
  `arcweft-codec-arrow` now requires `Seq<Record>` shapes, derives Arrow fields
  from `FieldShape`, performs strict row conversion instead of observed-value
  inference, rejects unknown/missing fields, preserves option/null policy,
  validates decoded columns against the declared shape, checks input length
  before parsing, and explicitly rejects nested or enum shapes.
- Hardened `arcweft-http-codec` negotiation. Request decoding now accepts
  parameterized `Content-Type` values only when parameters are syntactically
  valid, rejects wildcard content types, and enforces `DecodeOptions`
  `max_input_len` before codec decode. Response encoding now evaluates all
  `Accept` header values with q weights, wildcard ranges, `q=0` rejection,
  specificity, and header order, returning a concrete registered media type
  rather than the client wildcard range.
- Made `CodecRegistry` registration fallible and duplicate-aware. Registry
  insertion now rejects duplicate codec ids, normalized media types, normalized
  file extensions, and duplicate aliases within a single codec instead of
  letting later HTTP/save/config lookups depend on registration order.
- Made `arcweft-config` shape-aware. Config layer merging now requires a
  `TypeShape`, rejects unknown record fields through `FieldShape` /
  `RecordPolicy`, validates scalar values against the declared shape, applies
  list replace/append policy deterministically, and returns per-path
  provenance showing which layer/source supplied the final value.

## Remaining implementation debt

- Remote REPL cells now execute through the typed MCP session, but the
  project-bound binding policy is still coarse: primitive/string/collection
  binding preservation versus session-bound binding drop on project-hash
  changes needs explicit diagnostics and tests.
- The CLI-owned stdio MCP transport is process-backed and now covers fake-child
  roundtrip, timeout enforcement, bounded stderr retention, and graceful
  shutdown-before-kill behavior.
- The checked-in `.awfagent` formatter path is dialect-aware and diagnostic
  producing, but it is not yet a full lossless canonical formatter with golden
  coverage for comments/trivia and all Agent item families.
- Data raw transcoding covers the initial shape/value bridge. Parser-integrated
  JSON/TOML/YAML/CSV decode budgets and strict binary raw coverage remain
  separate data-format tasks.
- The derive parser now rejects malformed attributes, covers the main
  container/field/variant grammar, uses field-type generic bounds, and rejects
  unsupported tuple/unit/repr surfaces through trybuild-covered compile errors.
- Parse-time budget coverage currently protects Arcweft Binary and provides the
  shared `DecodeBudget` API. JSON/TOML/YAML/MsgPack/CBOR and tabular codecs
  still need parser-integrated visitors/readers rather than post-parse-only
  validation; MsgPack/CBOR now at least avoid the previous JSON bridge and
  preserve native bytes/integer categories.
- Save decoding now enforces the envelope identity/version gates from the ZIP
  guide and supports explicit multi-step migration chains. Envelope v1's
  checksum scope is documented as payload-only; any future header-authenticated
  checksum would require a versioned envelope contract instead of changing v1.
- JSON, TOML, and YAML shape decoding now cover the first concrete T-105 cuts,
  including central enum adjacent/internal/repr raw forms. Remaining
  JSON/TOML/YAML work includes parser-integrated budget visitors and deeper
  enum payload byte-policy coverage.
- CSV, Arrow IPC, and Parquet shape decoding now cover strict `Seq<Record>`
  scalar rows, including the scalar numeric edge matrix. Remaining tabular work
  includes Avro shape-guided schemas plus parser/reader-integrated budget
  enforcement.
- HTTP negotiation now covers the T-112 `Accept` q/wildcard/q=0/content
  parameter/body cap matrix for the codec adapter boundary. Remaining data
  adapter policy work is mostly in config provenance and non-CSV tabular
  formats.
- Codec registry uniqueness now covers the D-24 duplicate id/media/extension
  policy. Intentional format aliases remain explicit per-codec media type or
  extension entries and must be distinct after normalization.
- Config merge now covers the T-111 / D-22 shape-aware provenance slice for
  unknown fields, source precedence, list policy, required-field checking, and
  recursive redaction.

## Validation

```bash
cargo check -p arcweft-data -p arcweft-test -p arcweft-agent-mcp -p arcweft-agent-mcp-client -p arcweft-tooling -p arcweft-cli --all-targets --all-features
cargo check -p arcweft-data -p arcweft-data-derive --all-targets --all-features
cargo check -p arcweft-core -p arcweft-cli -p arcweft-agent-runner -p arcweft-agent-mcp-client --all-targets --all-features
cargo test -p arcweft-core --all-features
cargo check -p arcweft-agent-mcp-client -p arcweft-cli --all-targets --all-features
cargo test -p arcweft-data raw_shape --test raw_shape
cargo test -p arcweft-data --features derive --test derive_attrs
cargo test -p arcweft-data --features derive derive_attribute_ui
cargo clippy -p arcweft-data -p arcweft-data-derive --all-targets --all-features -- -D warnings
cargo test -p arcweft-codec-binary --test strict_decode
cargo clippy -p arcweft-data -p arcweft-codec-binary --all-targets --all-features -- -D warnings
cargo check -p arcweft-save --all-targets --all-features
cargo test -p arcweft-save --test strict_decode
cargo clippy -p arcweft-save --all-targets --all-features -- -D warnings
cargo check -p arcweft-data -p arcweft-codec-json --all-targets --all-features
cargo test -p arcweft-codec-json --test shape_codec
cargo clippy -p arcweft-data -p arcweft-codec-json --all-targets --all-features -- -D warnings
cargo check -p arcweft-data -p arcweft-codec-toml --all-targets --all-features
cargo test -p arcweft-codec-toml --test shape_codec
cargo clippy -p arcweft-data -p arcweft-codec-toml --all-targets --all-features -- -D warnings
cargo check -p arcweft-data -p arcweft-codec-yaml --all-targets --all-features
cargo test -p arcweft-codec-yaml --test shape_codec
cargo clippy -p arcweft-data -p arcweft-codec-yaml --all-targets --all-features -- -D warnings
cargo check -p arcweft-data -p arcweft-codec-json -p arcweft-codec-toml -p arcweft-codec-yaml --all-targets --all-features
cargo test -p arcweft-data raw_shape --test raw_shape
cargo test -p arcweft-data --features derive --test derive_attrs
cargo test -p arcweft-data --features derive derive_attribute_ui
cargo clippy -p arcweft-data -p arcweft-data-derive --all-targets --all-features -- -D warnings
cargo check -p arcweft-data -p arcweft-codec-msgpack -p arcweft-codec-cbor --all-targets --all-features
cargo test -p arcweft-codec-msgpack --test native_mapping
cargo test -p arcweft-codec-cbor --test native_mapping
cargo clippy -p arcweft-data -p arcweft-codec-msgpack -p arcweft-codec-cbor --all-targets --all-features -- -D warnings
cargo test -p arcweft-data raw_shape --test raw_shape
cargo test -p arcweft-codec-json --test shape_codec
cargo test -p arcweft-codec-toml --test shape_codec
cargo test -p arcweft-codec-yaml --test shape_codec
cargo test -p arcweft-codec-msgpack --test native_mapping
cargo test -p arcweft-codec-cbor --test native_mapping
cargo test -p arcweft-codec-csv --test shape_codec
cargo check -p arcweft-codec-csv --all-targets --all-features
cargo test -p arcweft-codec-csv --test shape_codec
cargo test -p arcweft-codec-arrow --all-features
cargo test -p arcweft-codec-arrow --test shape_codec -- --nocapture
cargo clippy -p arcweft-codec-arrow --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-codec-csv --all-targets --all-features -- -D warnings
cargo check -p arcweft-http-codec --all-targets --all-features
cargo test -p arcweft-http-codec --test negotiation
cargo clippy -p arcweft-http-codec --all-targets --all-features -- -D warnings
cargo check -p arcweft-data -p arcweft-http-codec -p arcweft-save --all-targets --all-features
cargo test -p arcweft-data --test codec_registry
cargo clippy -p arcweft-data -p arcweft-http-codec -p arcweft-save --all-targets --all-features -- -D warnings
cargo check -p arcweft-config --all-targets --all-features
cargo test -p arcweft-config --test shape_merge
cargo clippy -p arcweft-config --all-targets --all-features -- -D warnings
cargo test -p arcweft-agent-mcp -p arcweft-agent-mcp-client -p arcweft-test --all-features
cargo test -p arcweft-tooling agent_format --all-features
cargo test -p arcweft-cli stdio_transport_roundtrips_agent_session_calls_through_fake_child --all-features
cargo test -p arcweft-cli stdio_transport_ --all-features -- --nocapture
cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings
cargo test -p arcweft-cli agent_repl_parse_stdio_connection --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audit-2026-06-21
```

All commands above passed on Windows in this checkout.
