# ZIP Gap Open Items 2026-06-21

This ledger records the remaining work against
`D:/sanze/Downloads/arcweft-zip-gap-audit-2026-06-21.zip` after the current
implementation cuts. It is intentionally stricter than
`zip-gap-audit-2026-06-21.md`: items are left open unless current source and
validation evidence prove the ZIP acceptance criteria.

For a concise explanation of the concrete unfinished items, see
`zip-gap-unimplemented-items-2026-06-22.md`.

## Status Terms

- **Open implementation** means the current source contradicts the ZIP target
  or still uses the old behavior.
- **Partial implementation** means a first path exists, but the ZIP acceptance
  criteria are not fully covered by tests or behavior.
- **Verification debt** means the implementation may be present, but the
  required validation evidence is missing.

## Stopped Agent Validation Items

### ZG-A-004: Linux/macOS platform validation is stopped for this goal

- ZIP tasks: T-009, A-21.
- Status: stopped / out of current completion scope by explicit user direction
  on 2026-06-22.
- Current evidence:
  GitHub Actions run `27921332799` for
  `7e0c3145f2bf20111594c1bf9053ea60c3847657` records the latest matrix attempt
  before stopping non-Windows validation. Windows and macOS passed formatting,
  remote REPL/stdio MCP focused gates, data codec focused gates, workspace
  clippy, workspace non-CLI tests, CLI lib/bin tests, the CLI regression
  harness, and CLI fixture check/run tests. Ubuntu passed formatting, focused
  Agent gates, focused data codec gates, and workspace clippy, then failed
  non-CLI workspace tests in `arcweft-render-native` because the Linux CI
  headless graphics environment could not provide a suitable wgpu adapter for
  native renderer capture tests. Earlier Ubuntu attempts exposed missing native
  development packages and runner disk pressure; those workflow issues were
  addressed before validation was stopped.
- Why this matters: stdio process behavior, line endings, and CLI shell
  invocation differ across platforms.
- Current completion decision: not a source-behavior implementation gap and no
  longer a blocker for this ZIP goal. Non-Windows validation is recorded as
  stopped, not silently complete.
- Workflow state:
  `.github/workflows/zip-gap-platform-validation.yml` is intentionally disabled.
  It no longer runs on push, and its checked-in job is guarded with
  `if: ${{ false }}` so the validation job does not execute while this pause is
  in effect.

## Resolved Agent Items

### ZG-E-001: CLI target-effect availability is fully integrated

- Current evidence:
  `TypeCheckEnv` now separates checker capabilities from target effect
  availability. `AdapterManifest::apply_to_env` applies symbols, functions,
  function effects, checker capabilities, and Rust metadata without selecting a
  target availability set; `AdapterManifest::apply_to_target_env` additionally
  marks manifest effects as target-provided.
- CLI behavior:
  direct source checks no longer turn standard desktop helper manifests into a
  partial target environment. Entry-target flows are treated as boundary
  callables for effect analysis, so extern capability calls still require
  explicit source `effects { ... }` declarations.
- Scoped behavior:
  same-path scoped/unscoped effect coverage remains structural, while a
  different scoped capability does not cover a scoped inferred effect.
- Validation evidence:
  `cargo test -p arcweft-lang-sema entry_target_flow_requires_explicit_effects_for_extern_capability_calls -- --nocapture`,
  `cargo test -p arcweft-lang-sema target_effect_availability -- --nocapture`,
  and `cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet`
  passed on Windows in this checkout.

## Resolved Runtime Data Adapter Items

### ZG-R-001: runtime `data.decode` carries explicit shapes

- Current evidence:
  runtime external calls now accept `data.decode(bytes, format, shape)`, where
  `shape` is a runtime `DataShape` value such as the value returned by
  `data.shape(value)`. The runtime converts that record into a real
  `TypeShape` and calls the core codec `decode_value` path.
- Covered formats:
  JSON, TOML, YAML, MessagePack, CBOR, CSV, Arrow IPC, Parquet, and Arcweft
  Binary decode through the explicit shape path. The existing dynamic JSON and
  dynamic Avro envelope path remains available for 2-argument
  `data.decode(bytes, format)`.
- Non-goal:
  schema-bound Avro decode still requires an Avro schema-bearing codec surface;
  it is not modeled as a `TypeShape`-only runtime call.
- Validation evidence:
  `cargo test -p arcweft-runtime-accelerator data_external_call_ -- --nocapture`
  and
  `cargo test -p arcweft-lang-sema typechecks_data_codec_builtins_with_format_enum -- --nocapture`
  passed on Windows in this checkout.

### ZG-A-001: REPL project-bound binding policy is explicit

- ZIP tasks: T-004, A-10, A-12.
- Current evidence:
  `crates/arcweft-cli/src/app/agent/native/repl.rs` now records the active
  remote `program_hash` from `AgentSessionInfo`, classifies REPL snapshots as
  project-independent `literal` values or project-bound/session-derived values,
  and reconciles bindings when `:connect` switches between two remote sessions
  with different hashes. Primitive/string/numeric collection literal bindings
  are preserved. Entity references, observation/resource/RAG snapshots, cell
  artifacts, loaded Agent sources, and unsupported local snapshots are dropped.
- Structured report:
  The `:connect` meta cell now returns `binding_policy` with
  `old_program_hash`, `new_program_hash`, `program_hash_changed`, and one
  decision per binding. Each decision includes binding name, kind, status,
  snapshot kind, `preserved`/`dropped`, reason, and old/new program hashes.
- Validation evidence:
  `cargo test -p arcweft-cli agent_repl_project_hash --all-features -- --nocapture`,
  `cargo test -p arcweft-cli agent_repl_serialized_bindings_separate_literals_from_project_refs --all-features -- --nocapture`,
  `cargo test -p arcweft-cli agent_repl_stdio_connect_reports_project_hash_binding_policy --all-features -- --nocapture`,
  and
  `cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings`
  passed on Windows. The stdio connect test uses two fake MCP child processes
  with different `program_hash` values and asserts literal preservation,
  session-derived drop, and the structured `binding_policy` report.
- Remaining related work: Linux/macOS platform validation for the remote REPL
  path remains tracked under ZG-A-004.

### ZG-A-003: `.awfagent` formatter is lossless and idempotent for Agent dialect

- ZIP tasks: T-007, A-14.
- Current evidence:
  `crates/arcweft-cli/src/app/tooling.rs` accepts `.awfagent`, dispatches those
  files through `SourceDialect::Agent`, and rejects game-only sugar rewrites for
  Agent sources. The current Agent formatter contract is source-preserving:
  it reports Agent parser diagnostics but does not rewrite authored Agent
  source, so its canonical form is the stable authored form.
- Golden coverage:
  `crates/arcweft-tooling/src/tests.rs` now has
  `agent_format_preserves_comments_trivia_and_item_golden` for comments,
  doc comments, blank lines, attributes, Agent declarations, effects, waits,
  semantic actions, captures, resources, debug recording, and RAG calls. The
  same test asserts no diagnostics, unchanged output, and a stable second pass.
- Sample/idempotence coverage:
  `agent_format_is_idempotent_for_action_resource_and_rag_samples` covers
  physical pointer actions, resource read/attach, and the broader
  failure-investigation Agent sample. Existing `.arcw` formatting tests still
  cover game-source sugar behavior separately.
- CLI coverage:
  `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` now asserts that
  `arcw fmt --json file.awfagent` preserves Agent comments and calls without
  edits, and that `arcw fmt --expand-sugar file.awfagent` is rejected instead
  of applying game-dialect rewrites.
- Validation evidence:
  `cargo test -p arcweft-tooling agent_format --all-features -- --nocapture`,
  `cargo test -p arcweft-cli fmt_accepts_awfagent_path_and_preserves_agent_source_json --test check --all-features -- --nocapture`,
  and
  `cargo test -p arcweft-cli fmt_rejects_game_sugar_rewrites_for_awfagent_path --test check --all-features -- --nocapture`
  passed on Windows.
- Remaining related work: Linux/macOS platform validation for this cut remains
  tracked under ZG-A-004.

### ZG-A-002: stdio MCP transport is hardened for blocking children

- ZIP tasks: T-003, T-004, A-08, A-09.
- Current evidence:
  `crates/arcweft-cli/src/app/agent/mcp_stdio.rs` now reads child stdout on a
  dedicated line reader thread and uses request `recv_timeout` instead of
  blocking directly on `read_line`. Child stderr is retained as a bounded tail
  buffer and attached to write/read/timeout/closed errors. Shutdown now attempts
  a JSON-RPC `shutdown` request and `exit` notification, waits for graceful
  process exit, and only then falls back to kill.
- Validation evidence:
  `cargo test -p arcweft-cli stdio_transport_ --all-features -- --nocapture`
  and
  `cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings`
  passed on Windows. The focused tests cover the existing fake-child
  roundtrip, request timeout with bounded stderr tail retention, and
  shutdown-before-kill through a marker-writing fake child.
- Remaining related work: this closes the stdio transport hardening slice.
  Project-bound REPL binding diagnostics and formatter golden coverage are
  resolved below; Linux/macOS validation is recorded above as stopped for this
  goal.

## Open Core Data Codec Items

None currently identified. The core data-codec implementation gaps from the
ZIP audit are closed in the current source. The runtime external-call surface
for shape-required decode is tracked separately under `ZG-R-001`, because it is
an adapter/API integration gap rather than a core codec behavior gap. No core
data-codec source-behavior gap remains open in this ledger.

## Resolved Data Items

### ZG-D-001: parse-time budgets are complete for the ZIP data slice

- ZIP tasks: T-104, D-13.
- Current evidence:
  `arcweft-data::DecodeBudget` exists, and Arcweft Binary uses budget checks
  during parsing. MsgPack and CBOR now parse directly into budgeted raw values
  through low-level readers. JSON now consumes budget through a
  `serde_json::Deserializer` visitor before `serde_json::Value` shape
  projection. TOML now runs a source-level preflight before
  `toml::Deserializer::parse` builds its internal `DeTable`, covering input
  length, source string length, root/table map items, array items, inline-table
  items, value nodes, and nesting depth; it then consumes budget again through
  serde deserialization before public `toml::Value` shape projection. CSV now
  runs a `csv-core` byte-level preflight before constructing
  `csv::StringRecord` values, covering row count, record field count, unescaped
  field string length, header validation, and hex/base64 decoded byte upper
  bounds for bytes cells. YAML now runs a source scalar preflight before
  `yaml-rust2` parser entry, covering plain, quoted, and block scalar string
  limits before parser scalar event allocation; it then uses an event parser
  budget gate before constructing the public `Yaml` loader tree. Arrow IPC now
  preflights IPC footer/message metadata and Utf8/Binary offset buffers before
  `FileReader` materializes `RecordBatch` column buffers, then consumes decode
  budget again at batch conversion time. Parquet consumes decode budget at
  batch conversion time for rows, record field counts, value nodes, strings,
  and bytes before copying Arrow scalar buffers into Arcweft `Value`; Parquet
  also rejects metadata row-count overflow before building the record batch
  reader, preflights declared shape variable-width columns before building
  `ParquetRecordBatchReader`, rejects compressed variable-width column chunks
  under Arcweft limits, and caps uncompressed column chunk, unencoded
  byte-array data, and page buffers by the declared string/bytes limit. Avro
  now consumes top-level datum
  stream row budget while iterating `apache_avro::Reader`, avoids collecting all
  scalar datums before enforcing the single-datum scalar policy, and consumes
  record/map/array/node/string/bytes budgets before copying materialized
  `AvroValue` contents into Arcweft `Value`. Avro also runs an OCF null-codec
  preflight before constructing `apache_avro::Reader`, scanning writer-schema
  datum bytes for nested arrays, maps, records, map keys, strings, bytes, fixed
  values, unions, and top-level datum streams. Compressed Avro blocks are
  rejected rather than decompressed outside the Arcweft budget boundary.
- Concrete unfinished slices:
  None in the data codec implementation slice.
- Why this matters: hostile inputs can allocate large intermediate documents
  before Arcweft limits run; current covered codecs now put budget checks at
  parser, preflight, or reader boundaries before the previously problematic
  public value materialization points.
- Completion evidence needed: broader ZIP completion still needs the platform
  validation evidence tracked under ZG-A-004 and final reviewable-cut gates.

### ZG-D-005: Avro is shape-guided for supported records, options, and enums

- ZIP tasks: T-109, D-19.
- Current evidence:
  `crates/arcweft-codec-avro/src/codec.rs` validates the supplied Avro schema
  against Arcweft `TypeShape`, distinguishes top-level scalar datums from
  top-level `Seq` datum streams, maps records, options/unions, arrays, maps,
  scalar fields, native unit enums, and payload enum variants bidirectionally,
  and checks enum branch order against `VariantShape`.
- Payload enum contract:
  native Avro enum schemas are used for all-unit enums. Enums with any payload
  variant use an Avro union whose branches are variant records in
  `VariantShape` order. Unit variants use an empty record branch named by
  `wire_name`; payload variants use a record branch named by `wire_name` with
  exactly one `payload` field whose schema is validated against the variant
  payload shape.
- Validation evidence:
  `cargo test -p arcweft-codec-avro --test shape_codec -- --nocapture`,
  `cargo test -p arcweft-codec-avro --all-features`, and
  `cargo clippy -p arcweft-codec-avro --all-targets --all-features -- -D warnings`
  passed on Windows. The focused tests cover payload enum roundtrip, native
  enum symbol mismatch, payload schema mismatch, unknown variants, missing
  payloads, top-level scalar versus datum-stream policy, decode input caps,
  top-level row budget, record-field budget, string budget, bytes budget,
  nested array/string/bytes pre-`AvroValue` preflight, compressed-block
  rejection before reader materialization, and numeric edge policy. The current
  reviewable cut also passed `cargo clippy --workspace --all-targets
  --all-features` and the structural audit script with `0 error(s), 83
  warning(s)`.
- Remaining related work: top-level datum-stream and post-`AvroValue`
  conversion budgets are covered. Strict pre-`AvroValue` nested datum budget
  enforcement is now covered by the OCF null-codec preflight. Compressed Avro
  blocks are rejected under Arcweft limits instead of being decompressed before
  budgeting.

### ZG-D-004: Arrow IPC and Parquet are schema-driven for scalar rows

- ZIP tasks: T-108, T-109, D-18.
- Current evidence:
  `crates/arcweft-codec-arrow/src/lib.rs` now requires a top-level
  `Seq<Record>` shape for both Arrow IPC and Parquet, derives fields and Arrow
  data types from `FieldShape`, performs strict row conversion instead of
  inferring from observed values, rejects unknown and missing required fields,
  preserves option/null policy, validates decoded columns back through the
  declared shape, enforces input length caps before parsing, and rejects nested
  or enum shapes explicitly instead of stringifying or inferring them.
- Validation evidence:
  `cargo test -p arcweft-codec-arrow --all-features`,
  `cargo test -p arcweft-codec-arrow --test shape_codec -- --nocapture`, and
  `cargo clippy -p arcweft-codec-arrow --all-targets --all-features -- -D warnings`
  passed on Windows. The focused tests cover Arrow IPC and Parquet roundtrip,
  unknown/missing fields, option nulls, unsupported nested/enum shapes, decode
  input caps, row/record-field/string/bytes budget consumption during decode,
  Arrow IPC pre-`RecordBatch` string/bytes preflight, Parquet pre-
  `RecordBatch` string/bytes preflight, and numeric edge policy. The current
  reviewable cut also passed `cargo clippy --workspace --all-targets
  --all-features` and the structural audit script with `0 error(s), 83
  warning(s)`.
- Remaining related work: this resolves the Arrow IPC / Parquet shape-guided
  scalar-row slice, adds batch-conversion budget enforcement, and resolves
  Arrow IPC pre-`RecordBatch` string/binary buffer budget enforcement through
  IPC metadata/body preflight. Parquet now also resolves strict pre-
  `RecordBatch` variable-width column buffer enforcement through row-group/page
  preflight under Arcweft limits; compressed variable-width Parquet columns are
  rejected rather than decompressed outside that budget boundary.

### ZG-D-009: numeric edge-case policy is complete across current codecs

- ZIP tasks: D-25.
- Current evidence:
  central raw numeric conversion uses checked integer bounds and rejects
  non-finite floats. JSON, TOML, YAML, MsgPack, CBOR, CSV, Arrow IPC, Parquet,
  and Avro have focused tests for signed/unsigned crossings, out-of-range
  values, float-to-integer rejection, and NaN/infinity encode/decode rejection
  where the format can express the case.
- Validation evidence:
  Avro coverage is in
  `cargo test -p arcweft-codec-avro --test shape_codec -- --nocapture` and
  `cargo test -p arcweft-codec-avro --all-features`, which passed on Windows
  together with
  `cargo clippy -p arcweft-codec-avro --all-targets --all-features -- -D warnings`.
- Remaining related work: parser/reader-integrated budget enforcement remains
  tracked under ZG-D-001.

### ZG-D-002: derive shape generation policy gaps are closed

- ZIP tasks: T-103, D-05, D-06, D-07.
- Current evidence:
  `arcweft-data-derive` now builds Encode/Decode/Reflect where predicates from
  the concrete field types each generated impl uses instead of blindly bounding
  every generic type parameter. Unsupported tuple structs, unit structs,
  multi-field tuple enum variants, internally tagged newtype variants, repr
  enum discriminant expressions, and out-of-range repr discriminants fail at
  macro expansion with explicit compile errors instead of compiling into
  runtime `unsupported` branches or silently truncated numeric shapes.
- Validation evidence:
  `cargo test -p arcweft-data --features derive --test derive_attrs`,
  `cargo test -p arcweft-data --features derive derive_attribute_ui`, and
  `cargo clippy -p arcweft-data -p arcweft-data-derive --all-targets --all-features -- -D warnings`
  passed on Windows.
- Remaining related work: this resolves the derive policy slice.

### ZG-D-003: CSV is schema-driven for scalar row data

- ZIP tasks: T-108, D-17.
- Current evidence:
  `crates/arcweft-codec-csv/src/lib.rs` now requires a top-level
  `Seq<Record>` shape, derives headers from `FieldShape`, performs strict
  scalar conversion, rejects duplicate headers, rejects missing required
  columns, and applies `RecordPolicy::deny_unknown_fields` to unknown columns
  and encode fields. Decode now also runs a `csv-core` preflight before
  `StringRecord` materialization for row count, record field count, and
  unescaped field string length, plus shape-guided hex/base64 decoded byte
  upper bounds for bytes cells.
- Validation evidence:
  `cargo check -p arcweft-codec-csv --all-targets --all-features` and
  `cargo test -p arcweft-codec-csv --test shape_codec`,
  `cargo test -p arcweft-codec-csv --all-features`, and
  `cargo clippy -p arcweft-codec-csv --all-targets --all-features -- -D warnings`
  passed on Windows. The focused tests cover shape-driven rows, malformed
  headers, numeric edge cases, row/string/field-count budget limits, and quoted
  hex bytes budget failure before `StringRecord` materialization.
- Remaining related work: this resolves the CSV slice. Parquet, Avro, and
  parser/reader-integrated decode budgets remain open under their own items.

### ZG-D-007: HTTP codec negotiation and body limits are strict

- ZIP tasks: T-112, D-23.
- Current evidence:
  `crates/arcweft-http-codec/src/lib.rs` now exposes options-aware request and
  response helpers, validates `Content-Type` parameters, rejects wildcard
  content types, checks `DecodeOptions::limits.max_input_len` before codec
  decode, and negotiates `Accept` with q weights, wildcards, `q=0`,
  specificity, and header order.
- Validation evidence:
  `cargo check -p arcweft-http-codec --all-targets --all-features`,
  `cargo test -p arcweft-http-codec --test negotiation`,
  `cargo clippy -p arcweft-http-codec --all-targets --all-features -- -D warnings`,
  `cargo test -p arcweft-http-codec --all-features`, and
  `cargo clippy --workspace --all-targets --all-features` passed on Windows.
- Remaining related work: this resolves the HTTP adapter negotiation/body-cap
  slice. Non-CSV tabular codecs and config provenance remain open separately.

### ZG-D-008: CodecRegistry rejects ambiguous registrations

- ZIP tasks: D-24.
- Current evidence:
  `arcweft-data::CodecRegistry::with`, `register`, and `register_arc` now
  return `Result` and validate new codecs before insertion. Duplicate codec ids,
  normalized media types, normalized file extensions, and duplicate aliases
  inside a single codec are rejected with `DataErrorKind::DuplicateField`.
- Validation evidence:
  `cargo check -p arcweft-data -p arcweft-http-codec -p arcweft-save --all-targets --all-features`
  and `cargo test -p arcweft-data --test codec_registry`,
  `cargo clippy -p arcweft-data -p arcweft-http-codec -p arcweft-save --all-targets --all-features -- -D warnings`,
  `cargo test -p arcweft-data --all-features`,
  `cargo test -p arcweft-http-codec -p arcweft-save --all-features`, and
  `cargo clippy --workspace --all-targets --all-features` passed on Windows.
- Remaining related work: registry uniqueness is now enforced, but concrete
  data adapters still need their own shape/budget work where listed above.

### ZG-D-006: config merge is shape-aware and provenance-producing

- ZIP tasks: T-111, D-22.
- Current evidence:
  `crates/arcweft-config/src/lib.rs` now requires a `TypeShape` for
  `merge_config_layers`, validates incoming layer values against record fields
  and scalar shapes, rejects unknown fields through `FieldShape` /
  `RecordPolicy`, applies list replace/append policy, fills missing optional
  fields as `Unit`, checks required fields after all layers, and returns a
  `ConfigMergeReport` with per-path layer/source provenance.
- Validation evidence:
  `cargo check -p arcweft-config --all-targets --all-features`,
  `cargo test -p arcweft-config --test shape_merge`, and
  `cargo clippy -p arcweft-config --all-targets --all-features -- -D warnings`,
  `cargo test -p arcweft-config --all-features`, and
  `cargo clippy --workspace --all-targets --all-features` passed on Windows.
- Remaining related work: config merge provenance is covered. Broader
  repository completion still depends on parse-time budgets and Agent REPL/MCP
  hardening.

### ZG-D-010: save migration supports explicit multi-step chains

- ZIP tasks: T-110, D-21.
- Current evidence:
  `crates/arcweft-save/src/lib.rs` now exposes `SaveMigrationStep` and
  `SaveMigrationChain`. Chain construction validates schema id consistency,
  strictly advancing source/target versions, duplicate source-version rejection,
  and target bounds against the current schema version. `decode_save` can run a
  chain through the existing `SaveMigration` plan boundary and still validates
  the migrated value against the current shape.
- Validation evidence:
  `cargo check -p arcweft-save --all-targets --all-features`,
  `cargo test -p arcweft-save --test strict_decode`, and
  `cargo clippy -p arcweft-save --all-targets --all-features -- -D warnings`,
  `cargo test -p arcweft-save --all-features`, and
  `cargo clippy --workspace --all-targets --all-features` passed on Windows.
- Checksum decision: save envelope v1 checksum remains payload-only. Schema id,
  codec id, schema version, length caps, and trailing data are explicit header
  checks. A future header-authenticated checksum would require a new versioned
  envelope contract rather than changing v1 semantics.
- Remaining related work: save envelope migration chaining is covered. Broader
  repository completion still depends on parse-time budgets and Agent REPL/MCP
  hardening.

## Verification Debt That Blocks Goal Completion

- Re-run the full validation plan from the ZIP ledger after the open
  implementation items are closed.
- Re-run structural audit at each reviewable cut point:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audit-2026-06-21
```

- Record exact current evidence for every ZIP task before marking the goal
  complete. Passing workspace clippy alone is not sufficient for tasks whose
  acceptance criteria require malformed input tests, cross-format roundtrips,
  remote process behavior, or platform matrix validation.
