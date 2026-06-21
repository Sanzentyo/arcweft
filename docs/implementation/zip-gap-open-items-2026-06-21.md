# ZIP Gap Open Items 2026-06-21

This ledger records the remaining work against
`D:/sanze/Downloads/arcweft-zip-gap-audit-2026-06-21.zip` after the current
implementation cuts. It is intentionally stricter than
`zip-gap-audit-2026-06-21.md`: items are left open unless current source and
validation evidence prove the ZIP acceptance criteria.

## Status Terms

- **Open implementation** means the current source contradicts the ZIP target
  or still uses the old behavior.
- **Partial implementation** means a first path exists, but the ZIP acceptance
  criteria are not fully covered by tests or behavior.
- **Verification debt** means the implementation may be present, but the
  required validation evidence is missing.

## Open Agent Items

### ZG-A-001: REPL project-bound binding policy is still coarse

- ZIP tasks: T-004, A-10, A-12.
- Current evidence:
  `crates/arcweft-cli/src/app/agent/native/repl.rs` has remote session support
  and serializable binding snapshots, but the connect path does not yet expose
  explicit diagnostics for preserving primitive/string/collection bindings
  versus dropping session-bound bindings when the remote `program_hash` changes.
- Why this matters: remote `:connect` can otherwise appear to carry live REPL
  state across incompatible projects without telling the user which bindings
  remain meaningful.
- Completion evidence needed: tests that connect to two different remote
  project hashes, preserve self-contained primitive/string/collection bindings,
  drop session-bound observation/resource/RAG bindings, and assert structured
  diagnostics/report fields for each decision.

### ZG-A-002: stdio MCP transport lacks production hardening

- ZIP tasks: T-003, T-004, A-08, A-09.
- Current evidence:
  `crates/arcweft-cli/src/app/agent/mcp_stdio.rs` spawns a child process and
  passes a fake-child roundtrip, but `request` blocks on `read_line` without a
  timeout, stderr is piped but not retained or bounded, and `shutdown` kills the
  child immediately instead of trying protocol/process shutdown before kill.
- Why this matters: a hung or noisy remote endpoint can stall the REPL and hide
  useful failure context.
- Completion evidence needed: timeout tests, bounded stderr retention tests,
  and shutdown-before-kill tests against a fake child.

### ZG-A-003: `.awfagent` formatter is not yet proven lossless/canonical

- ZIP tasks: T-007, A-14.
- Current evidence:
  `crates/arcweft-cli/src/app/tooling.rs` accepts `.awfagent`, and the current
  formatter route is dialect-aware. The remaining audit still records that
  comments/trivia and all Agent item families do not have full golden coverage.
- Why this matters: accepting `.awfagent` in `arcw fmt` is only the entrypoint;
  the ZIP target asks for idempotent, canonical formatting behavior for the
  dialect.
- Completion evidence needed: golden and idempotence tests for comments,
  trivia, Agent declarations, effects, waits, actions, captures, resources,
  RAG calls, and `.arcw` regression cases.

### ZG-A-004: Linux/macOS platform validation is not recorded

- ZIP tasks: T-009, A-21.
- Current evidence: the repository records Windows validation for the focused
  cuts, but no current Linux/macOS CI or local run evidence is recorded for the
  remote REPL and data-codec changes.
- Why this matters: stdio process behavior, line endings, and CLI shell
  invocation differ across platforms.
- Completion evidence needed: focused remote REPL gates and workspace gates on
  Linux, Windows, and macOS, either through CI or explicit recorded runs.

## Open Data Items

### ZG-D-001: parse-time budgets are still incomplete outside Arcweft Binary

- ZIP tasks: T-104, D-13.
- Current evidence:
  `arcweft-data::DecodeBudget` exists, and Arcweft Binary uses budget checks
  during parsing. JSON, TOML, YAML, MsgPack, CBOR, CSV, Arrow, Parquet, and
  Avro still materialize format-native values before final
  `DecodeLimits::validate` or equivalent shape validation.
- Why this matters: hostile inputs can allocate large intermediate documents
  before Arcweft limits run.
- Completion evidence needed: parser-integrated visitors/readers or equivalent
  bounded readers for each codec, plus adversarial input/depth/node/collection
  tests that fail before unbounded allocation.

### ZG-D-002: derive shape generation still has known policy gaps

- ZIP tasks: T-103, D-05, D-06, D-07.
- Current evidence:
  `arcweft-data-derive` now has a typed attribute parser and trybuild coverage,
  but the remaining implementation note still lists precise generic bounds,
  tuple/unit struct policy, multi-field tuple enum policy, and repr
  discriminant range validation as open.
- Why this matters: unsupported derive surfaces should either generate correct
  shapes or fail with explicit compile errors; silent partial generation would
  recreate the original package problem.
- Completion evidence needed: pass/fail trybuild fixtures for generic bounds,
  tuple/unit structs, multi-field tuple enum variants, and repr range errors.

### ZG-D-003: CSV is not schema-driven

- ZIP tasks: T-108, D-17.
- Current evidence:
  `crates/arcweft-codec-csv/src/lib.rs` ignores the `shape` parameter, derives
  headers from the first encoded row, decodes every cell as `Value::String`,
  and has no tests in the crate.
- Why this matters: extra columns, missing columns, nested values, null-like
  cells, and numeric/bool/bytes cells can be silently reshaped instead of being
  checked against the declared schema.
- Completion evidence needed: require `Seq<Record>` shape, derive columns from
  `FieldShape`, reject unknown/missing columns according to `RecordPolicy`,
  perform checked scalar conversion, reject unsupported nested shapes, and add
  roundtrip/error tests.

### ZG-D-004: Arrow IPC and Parquet still infer schemas from values

- ZIP tasks: T-108, T-109, D-18.
- Current evidence:
  `crates/arcweft-codec-arrow/src/lib.rs` ignores `shape`, builds Arrow fields
  from the union of observed record keys, infers column types from the first
  matching value, writes nulls when a row value has the wrong type, and has no
  crate tests.
- Why this matters: schema fidelity is not guaranteed; row data can be widened,
  nulled, or converted based on observed values rather than the declared
  Arcweft shape.
- Completion evidence needed: shape-to-Arrow schema mapping, strict row
  conversion, decode validation back to `TypeShape`, option/null policy tests,
  enum/record tests, and decode limit tests.

### ZG-D-005: Avro is not shape-guided enough for record/enum/option fidelity

- ZIP tasks: T-109, D-19.
- Current evidence:
  `crates/arcweft-codec-avro/src/lib.rs` uses the supplied Avro schema for the
  writer/reader, but ignores the Arcweft `shape` parameter. Enum payloads are
  encoded as ad hoc records with `variant`/`payload`, enum indices are not
  checked against Arcweft `VariantShape`, and decode always returns a sequence
  of rows. The crate has no tests.
- Why this matters: Avro schema compatibility does not prove Arcweft shape
  compatibility, especially for enums, options/unions, maps, and top-level
  scalar versus row-set behavior.
- Completion evidence needed: Avro schema generation or validation from
  `TypeShape`, bidirectional enum/option/record mapping, top-level shape policy,
  strict error cases, and limits tests.

### ZG-D-006: config merge lacks shape-aware provenance

- ZIP tasks: T-111, D-22.
- Current evidence:
  `crates/arcweft-config/src/lib.rs` merges dynamic `Value` trees and has a
  `deny_unknown_fields` flag in `ConfigMergePolicy`, but no `TypeShape`
  parameter, field provenance model, source-precedence report, or tests.
- Why this matters: config consumers cannot explain where a field came from,
  reject unknown fields through schema, or distinguish override precedence from
  accidental shape drift.
- Completion evidence needed: shape-aware merge API, per-field provenance,
  unknown-field enforcement, list strategy tests, redaction/provenance tests,
  and source precedence tests.

### ZG-D-007: HTTP codec negotiation and body limits are incomplete

- ZIP tasks: T-112, D-23.
- Current evidence:
  `crates/arcweft-http-codec/src/lib.rs` strips Accept parameters, chooses the
  first registered exact media type, does not implement q weights, wildcards,
  `q=0`, content-type parameter policy, or a body-size cap before codec decode.
  The crate has no tests.
- Why this matters: HTTP clients can receive a less preferred format or a
  format they explicitly rejected, and oversized request bodies are delegated
  to codec parsing before adapter-level limits run.
- Completion evidence needed: standards-aware Accept sorting, wildcard and
  `q=0` rejection, content parameter tests, request/response body cap tests,
  and structured negotiation errors.

### ZG-D-008: CodecRegistry allows ambiguous registrations

- ZIP tasks: D-24.
- Current evidence:
  `arcweft-data::CodecRegistry::register` and `register_arc` append codecs to a
  vector and return `()`. Lookup returns the first matching id or media type,
  so duplicate ids/media types/extensions are not rejected.
- Why this matters: save/config/http adapters can become order-dependent when
  two codecs claim the same format id or media type.
- Completion evidence needed: fallible registration/builder APIs with
  duplicate id, media type, and extension tests, while preserving intentional
  aliases as explicit metadata.

### ZG-D-009: numeric edge-case policy is not complete across codecs

- ZIP tasks: D-25.
- Current evidence:
  central raw numeric conversion now uses checked integer bounds, but tabular
  codecs still perform inference or optional conversions, and there is no
  repository-wide NaN/infinity policy test matrix.
- Why this matters: lossy casts, silently nulled cells, or non-finite floats
  can produce format-specific behavior instead of a stable Arcweft data
  contract.
- Completion evidence needed: shared numeric policy tests for signed/unsigned
  crossings, out-of-range values, float-to-integer rejection, and
  NaN/infinity encode/decode behavior in every relevant codec.

### ZG-D-010: save migration is hardened but not fully chained

- ZIP tasks: T-110, D-21.
- Current evidence:
  `arcweft-save` has strict envelope identity/version/limit/trailing checks
  and migration hooks. The remaining audit still records explicit multi-step
  migration chains and future checksum-header coverage decisions as open.
- Why this matters: a single migration hook does not prove behavior for
  multi-version save evolution, and checksum scope must be explicit before a
  future envelope version depends on it.
- Completion evidence needed: multi-step migration chain model/tests,
  migration schema/version checks across more than one hop, and an ADR or
  versioned contract for checksum metadata scope.

## Verification Debt That Blocks Goal Completion

- Re-run the full validation plan from the ZIP ledger after the open
  implementation items are closed.
- Re-run structural audit at each reviewable cut point:

```bash
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audit-2026-06-21
```

- Record exact current evidence for every ZIP task before marking the goal
  complete. Passing workspace clippy alone is not sufficient for tasks whose
  acceptance criteria require malformed input tests, cross-format roundtrips,
  remote process behavior, or platform matrix validation.
