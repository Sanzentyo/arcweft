# ZIP Gap Unimplemented Items 2026-06-22

This note lists the concrete unfinished items for
`D:/sanze/Downloads/arcweft-zip-gap-audit-2026-06-21.zip`.
It is the readable companion to
`docs/implementation/zip-gap-open-items-2026-06-21.md`, which remains the
strict requirement ledger.

Implementation baseline used for this inventory:
`5b1b6618 Preflight Arrow IPC buffers` plus the current Parquet
variable-width preflight and Avro OCF datum preflight cuts.

This document is the concrete "what is still not implemented" answer for the
ZIP gap goal as of that baseline. The current unfinished set is intentionally
small:

- **ZG-A-004 / T-009 / A-21**: Linux and macOS validation evidence is still
  absent.

Everything else from the ZIP is either recorded as resolved in
`zip-gap-open-items-2026-06-21.md`, or is a related validation follow-up that
depends on one of the items above.

## Status Model

未実装として扱う範囲を次の 3 種類に分ける。

- **Open implementation**: 現在のソースが ZIP の対象仕様にまだ達していない。
  入口や型はあっても、要求された動作がない。
- **Partial implementation**: 初期実装はあるが、ZIP の受け入れ条件を満たす
  動作・診断・テスト・証跡がそろっていない。
- **Verification debt**: 実装はある可能性が高いが、要求された検証証跡がない
  ので完了扱いにできない。

## Current Open List

| Area | Item | Status | Missing thing that blocks completion |
| --- | --- | --- | --- |
| Agent | ZG-A-004 Linux/macOS validation | Verification debt | Windows 以外での remote REPL / stdio MCP / data codec focused gates と workspace gates の記録 |

## ZG-A-004: Linux/macOS Validation

ZIP mapping: `T-009`, `A-21`.

Status: **Verification debt**.

Existing implementation/evidence:

- Windows では、stdio MCP transport、remote REPL parsing、data codec focused
  gates、workspace clippy などの証跡が記録されている。
- `zip-gap-open-items-2026-06-21.md` と
  `zip-gap-audit-2026-06-21.md` は Windows 実行結果を中心に記録している。

具体的に未実装として残っているもの:

- Linux での remote REPL / stdio MCP focused tests。
- macOS での remote REPL / stdio MCP focused tests。
- Linux/macOS/Windows の workspace gates または CI matrix evidence。
- 各 platform の command、revision、result、失敗時の理由。

ここは source behavior の未実装ではなく、検証証跡の未実装である。Windows
での実行証跡はあるが、ZIP の platform matrix は Linux/macOS の process
lifecycle、path handling、stdio framing も対象にしているため、Windows だけでは
完了扱いにできない。

必要なテスト・証跡:

- 少なくとも remote REPL / stdio MCP process behavior に関わる focused tests
  を Linux/macOS/Windows で記録する。
- data codec の open/changed slice に対する focused tests を platform matrix
  か CI で確認する。
- reviewable cut point で workspace check/clippy と structural audit の結果を
  実装文書に残す。

なぜ完了扱いにできないか:

stdio process、line endings、shell invocation、path handling は OS 差が出る。
Windows のみの成功では remote process adapter と CLI validation の ZIP 要求を
閉じられない。

## ZG-D-001: Parse-Time Budgets Outside Arcweft Binary

ZIP mapping: `T-104`, `D-13`; Parquet related surface: `D-18`; Avro related
surface: `D-19`.

Status: **Resolved implementation evidence**.

Existing implementation/evidence:

- `arcweft-data::DecodeBudget` exists.
- Arcweft Binary decoding uses parse-time input/node/depth/collection/string/byte
  checks before allocating the full decoded value.
- Parquet/Avro already apply some caps or shape validation
  after parse, and several codecs check `max_input_len` before invoking their
  parser.
- TOML now runs a source-level preflight before `toml::Deserializer::parse`
  builds its internal `DeTable`, then consumes `DecodeBudget` again through
  serde deserialization before building public `toml::Value` shape-projection
  helpers. The preflight checks input length, source string length, root/table
  map items, array items, inline-table items, value nodes, and nesting depth.
- JSON now uses a `serde_json::Deserializer` seed/visitor that consumes
  `DecodeBudget` while parsing dynamic raw values, before any
  `serde_json::Value` shape-projection helper is built.
- MsgPack and CBOR now use bounded low-level readers that consume
  `arcweft-data::DecodeBudget` while parsing raw values, before building
  `rmpv::Value`, `ciborium::Value`, or Arcweft `Value` intermediates.
- YAML now runs a source scalar preflight before invoking `yaml-rust2`, so
  plain, quoted, and block scalar string limits are checked before parser
  scalar event strings can be allocated. It then runs a low-level
  `yaml-rust2` event parser budget gate before the public `Yaml` loader tree is
  built, covering document tree nodes, sequences, mappings, and scalar strings
  again at the parser event boundary.
- Arrow IPC now preflights IPC file footer and record batch metadata before
  constructing `arrow::ipc::reader::FileReader`. The preflight rejects unknown
  columns for deny-unknown record shapes, unsupported dictionaries/compressed
  record batches, oversized record-batch row counts, and Utf8/Binary per-cell
  string/bytes lengths by reading offset buffers directly from the IPC body.
  Arrow IPC then consumes row sequence, record field map, decoded value node,
  and string/bytes budgets again while copying Arrow scalar buffers into
  Arcweft `Value`. Parquet creates `DecodeBudget` at decode entry, rejects
  total row count from metadata before building the record batch reader, caps
  reader batch size by the sequence limit, preflights declared shape
  variable-width columns before building `ParquetRecordBatchReader`, rejects
  compressed variable-width column chunks under Arcweft limits, caps
  uncompressed column chunk, unencoded byte-array data, and page buffers by the
  declared string/bytes limit, and consumes row/map/node/string/bytes budgets
  before copying Arrow scalar buffers into Arcweft `Value`.
- Avro now runs a schema-guided OCF preflight before constructing
  `apache_avro::Reader`. The preflight parses the container header without
  allocating `AvroValue`, rejects compressed Avro blocks under Arcweft limits,
  scans null-codec block datum bytes with the writer schema, consumes
  `DecodeBudget` for datum nodes, nested arrays/maps/records, map keys,
  strings, bytes, fixed values, unions, and top-level datum streams, then the
  normal `AvroValue -> Arcweft Value` conversion consumes the same budgets
  again before copying public values.
- CSV now runs a `csv-core` byte-level preflight before constructing
  `csv::StringRecord` values. The preflight consumes input length, top-level
  row sequence budget, per-record field-count budget, and unescaped field
  string length budget with a fixed output buffer. It also validates headers
  against `TypeShape` during preflight and checks hex/base64 decoded byte
  upper bounds for bytes cells before `StringRecord` materialization.

具体的に未実装または部分実装に残っている data behavior:

- なし。JSON/TOML/YAML/MsgPack/CBOR/CSV/Arrow IPC/Parquet/Avro/Arcweft
  Binary の ZIP 対象 budget/shape items は current source と focused tests で
  resolved として記録済み。

なぜ完了扱いにできないか:

data behavior は current implementation evidence で完了扱いにできる。ZIP goal
全体は、Linux/macOS validation evidence がまだないため完了扱いにしない。

## Already Covered Slices

The following slices should not be counted as currently unimplemented, though
some of them leave related items open:

- CSV is schema-driven for scalar `Seq<Record>` rows and now consumes
  `DecodeBudget` during a `csv-core` preflight before `StringRecord`
  materialization for row count, record field count, and unescaped field string
  length. The preflight validates headers against `TypeShape` and checks
  hex/base64 decoded byte upper bounds for bytes cells before `StringRecord`
  materialization. Focused tests cover huge quoted fields, field-count limits,
  and quoted hex bytes budget failure before record materialization.
- HTTP codec negotiation rejects ambiguous content and enforces body caps at
  the adapter boundary.
- `CodecRegistry` rejects duplicate ids, media types, extensions, and aliases.
- stdio MCP transport requests time out, retain bounded stderr tails, and try
  protocol shutdown plus exit before kill fallback.
- Remote REPL `:connect` now records remote `program_hash`, preserves only
  project-independent literal primitive/string/numeric collection bindings
  across remote hash changes, drops project-bound and session-derived bindings,
  and reports structured per-binding preserve/drop decisions. A focused test
  connects to two fake stdio MCP children with different program hashes and
  asserts the resulting `binding_policy`.
- `.awfagent` formatting is dialect-aware, source-preserving, idempotent, and
  covered by golden tests for comments/trivia, Agent declarations, effects,
  waits, semantic and physical actions, captures, resources, debug recording,
  and RAG calls. CLI `arcw fmt --json file.awfagent` preserves Agent source, and
  `arcw fmt --expand-sugar file.awfagent` rejects game-dialect rewrites.
- MsgPack and CBOR decoding now consume `DecodeBudget` during parse. Focused
  adversarial tests cover declared string/bytes length before payload reads,
  declared array length before item allocation, node budget exhaustion, and
  CBOR indefinite array item budget exhaustion.
- JSON decoding now consumes `DecodeBudget` through a serde visitor before
  `serde_json::Value` shape projection. Focused tests cover input length,
  string length, sequence length, and node budget exhaustion.
- TOML decoding now runs a source-level `DecodeBudget` preflight before
  `toml::Deserializer::parse` can build its internal `DeTable`, then consumes
  budget again through serde deserialization before public `toml::Value` shape
  projection. Focused tests cover input length, string length, array length,
  node budget exhaustion, and malformed oversized string, bare-key, and array
  inputs that fail with `LimitExceeded` before TOML parse errors.
- YAML decoding now runs a source scalar preflight before `yaml-rust2` parser
  entry, then runs a low-level event parser budget gate before constructing the
  public `Yaml` loader tree. Focused tests cover input length, scalar string
  length, sequence length, node budget exhaustion, and oversized quoted, plain,
  and block scalar inputs that fail with `LimitExceeded` before parser event
  string allocation.
- Raw shape conversion plus JSON, TOML, YAML, MsgPack, CBOR, CSV, Arrow,
  Parquet, and Avro reject non-finite floats, float-to-integer recovery, and
  signed/unsigned bounds violations through focused numeric edge tests.
- Arrow IPC and Parquet require `Seq<Record>` shapes, derive scalar schemas from
  `FieldShape`, reject malformed rows and unsupported nested/enum shapes, and
  carry the same numeric edge matrix for supported scalar rows. Arrow IPC now
  preflights footer/message metadata and Utf8/Binary offset buffers before
  `FileReader` can materialize `RecordBatch` column buffers; focused tests
  cover oversized IPC string and bytes cells failing before record-batch
  decoding. Parquet rejects metadata row-count overflow before building the
  record batch reader, preflights declared shape variable-width columns before
  building `ParquetRecordBatchReader`, rejects compressed variable-width column
  chunks under Arcweft limits, caps uncompressed column chunk, unencoded
  byte-array data, and page buffers by the declared string/bytes limit, then
  consumes row/record/string/bytes budgets again before copying Arrow scalar
  buffers into Arcweft `Value`.
- Avro validates supplied schemas against `TypeShape`, maps scalar, record,
  option, array, map, native unit enum, and payload enum values
  bidirectionally, enforces top-level scalar versus datum-stream policy, and
  carries the numeric edge matrix for supported scalar values. Payload enum
  variants use an Avro union of variant records in `VariantShape` order, with a
  single typed `payload` field for payload variants. It now consumes top-level
  row budget during reader iteration, avoids collecting all scalar datums before
  enforcing single-datum scalar decode, and checks row/record/string/bytes
  budgets before copying materialized `AvroValue` contents into Arcweft values.
  Avro now also scans OCF null-codec datum bytes before `apache_avro::Reader`
  can materialize nested `AvroValue` trees, including arrays, maps, records,
  strings, bytes, fixed values, unions, and top-level datum streams. Compressed
  Avro blocks are rejected rather than decompressed outside the Arcweft budget
  boundary.
- Config merge is shape-aware and provenance-producing.
- Save decoding supports explicit multi-step migration chains.
- Derive shape generation now uses field-type where predicates and compile-time
  policy errors for tuple/unit structs, unsupported tuple variants, internally
  tagged newtype variants, and invalid repr discriminants.
- JSON, TOML, YAML, MsgPack, and CBOR have moved away from the earlier broad
  JSON bridge for the covered raw-shape paths.

## Completion Rule

Do not close the ZIP goal until every item above is either implemented and
tested or intentionally moved out of scope in a repository-visible note.
A workspace `cargo clippy --workspace --all-targets --all-features` pass is
necessary at reviewable cut points, but it is not sufficient for items that
require malformed input tests, cross-format roundtrips, process behavior tests,
or platform matrix validation.
