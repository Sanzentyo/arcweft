# ZIP Gap Unimplemented Items 2026-06-22

This note lists the concrete unfinished items for
`D:/sanze/Downloads/arcweft-zip-gap-audit-2026-06-21.zip`.
It is the readable companion to
`docs/implementation/zip-gap-open-items-2026-06-21.md`, which remains the
strict requirement ledger.

Implementation baseline used for this inventory:
`c8f5f10e Preflight CSV bytes budgets` plus the TOML source preflight cut
documented below.

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
| Data | ZG-D-001-YAML strict pre-scalar-event allocation | Partial implementation | public `Yaml` loader tree 前の event budget gate はあるが、event receiver が見る前に scalar `String` が parser 内部で確保される |
| Data | ZG-D-001-Arrow IPC reader materialization budget | Partial implementation | `RecordBatch` から Arcweft `Value` を作る前の row/field/string/bytes budget はあるが、`FileReader` 内部の column buffer materialization 前 cap はない |
| Data | ZG-D-001-Parquet reader materialization budget | Partial implementation | metadata row count と batch conversion budget はあるが、row group/page decode が column buffers を materialize する前の string/binary cap はない |
| Data | ZG-D-001-Avro datum materialization budget | Partial implementation | top-level datum stream と `AvroValue -> Arcweft Value` 変換時の budget はあるが、`apache_avro::Reader` が nested `AvroValue` を materialize する前の visitor/reader policy がない |

## ZG-A-004: Linux/macOS Validation

Status: **Verification debt**.

Existing implementation/evidence:

- Windows では、stdio MCP transport、remote REPL parsing、data codec focused
  gates、workspace clippy などの証跡が記録されている。
- `zip-gap-open-items-2026-06-21.md` と
  `zip-gap-audit-2026-06-21.md` は Windows 実行結果を中心に記録している。

具体的に未記録な証跡:

- Linux での remote REPL / stdio MCP focused tests。
- macOS での remote REPL / stdio MCP focused tests。
- Linux/macOS/Windows の workspace gates または CI matrix evidence。
- 各 platform の command、revision、result、失敗時の理由。

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

Status: **Open implementation**.

Existing implementation:

- `arcweft-data::DecodeBudget` exists.
- Arcweft Binary decoding uses parse-time input/node/depth/collection/string/byte
  checks before allocating the full decoded value.
- YAML/Arrow/Parquet/Avro already apply some caps or shape validation
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
- YAML now runs a low-level `yaml-rust2` event parser budget gate before the
  public `Yaml` loader tree is built.
- Arrow IPC and Parquet now create `DecodeBudget` at decode entry, consume row
  sequence budget, record field map budget, decoded value node budget, and
  string/bytes limits before copying Arrow scalar buffers into Arcweft `Value`.
  Parquet also rejects total row count from metadata before building the record
  batch reader and caps reader batch size by the sequence limit.
- Avro now consumes top-level datum stream row budget while iterating
  `apache_avro::Reader`, avoids collecting all scalar datums before enforcing
  the single-datum top-level scalar policy, and consumes record/map/array,
  node, string, and bytes budgets before copying `AvroValue` contents into
  Arcweft `Value`.
- CSV now runs a `csv-core` byte-level preflight before constructing
  `csv::StringRecord` values. The preflight consumes input length, top-level
  row sequence budget, per-record field-count budget, and unescaped field
  string length budget with a fixed output buffer. It also validates headers
  against `TypeShape` during preflight and checks hex/base64 decoded byte
  upper bounds for bytes cells before `StringRecord` materialization.

具体的に未実装または部分実装に残っている動作:

- **YAML strict pre-scalar-event allocation**:
  YAML は public `YamlLoader` tree の前に event parser budget gate を通すため、
  巨大 document tree の構築は抑止できる。一方で `MarkedEventReceiver` が
  `Scalar` event を受け取る時点では parser 内部で scalar `String` がすでに
  確保されている。ZIP の「unbounded intermediate allocation 前に失敗する」
  条件を厳密に満たすには、scalar token payload を確保する前の cap、または
  それと同等の scanner-level limit が必要である。
- **Arrow IPC reader materialization budget**:
  Arrow IPC は `max_input_len` と `Seq<Record>` schema validation はあるが、
  `arrow::ipc::reader::FileReader` が `RecordBatch` と column buffers を返した
  後に batch boundary budget を行う。row sequence、record map、value node、
  scalar string/bytes length は Arcweft `Value` allocation 前に structured
  error で止まる。未実装なのは、`FileReader` 内部が column buffers を
  materialize する前に同じ string/binary/row cap を強制する lower-level reader
  または crate-supported equivalent である。
- **Parquet reader materialization budget**:
  Parquet も `max_input_len` と scalar row schema validation に加え、metadata
  の total row count を reader build 前に確認し、reader batch size を sequence
  limit に合わせ、`RecordBatch` から Arcweft `Value` を作る前に row/map/node/
  string/bytes budget を消費する。未実装なのは、row group/page decode が
  string/binary column buffers を materialize する前の budget gate と、metadata
  だけでは分からない per-cell payload size を reader 内部 allocation 前に止める
  経路である。
- **Avro datum materialization budget**:
  Avro は `max_input_len`、schema validation、top-level datum stream の
  `sequence_item` 消費、top-level scalar の single-datum streaming check、
  `AvroValue` から Arcweft `Value` への変換時 budget 消費を持つ。record/map/
  array length、node count、string length、bytes length は Arcweft `Value`
  allocation 前に structured error として返る。未実装なのは
  `apache_avro::Reader` が nested `AvroValue::Array` / `Map` / `Record` /
  `String` / `Bytes` / payload enum branch を materialize する前に同じ budget
  で止める reader/visitor policy である。特に単一 datum 内の巨大 array/map/
  record/payload enum は、`AvroValue` 後の validation では reader 内部 allocation
  を防げない。

必要な実装:

- Format ごとに parser-integrated visitor、bounded reader、streaming reader、
  metadata preflight、または crate-supported equivalent を導入する。
- 少なくとも input length、node count、nesting depth、collection length、
  string length、byte length のうち format が表現できるものを parse 中に数える。
- Budget 超過は Arcweft data error として structured に返し、panic や allocator
  failure に依存しない。

必要なテスト・証跡:

- 各 codec に adversarial input tests を追加する。YAML は巨大 scalar
  や巨大 collection、Arrow/Parquet は巨大 row/batch/column buffer、Avro は巨大
  datum array/map/string/bytes を対象にする。
- 深い nesting、巨大 array/map/record、巨大 string/bytes、巨大 row/column などが
  unbounded intermediate allocation 前に失敗することを示す。
- Arcweft Binary で済んでいる budget tests と同じ意味の matrix を、
  対象 codec の表現能力に合わせて持つ。

なぜ完了扱いにできないか:

`DecodeLimits::validate` のような post-parse validation は、悪意ある入力が
巨大な native document を先に確保するケースには遅すぎる。ZIP の対象は
「Arcweft value へ変換した後に弾く」ではなく「parse/materialize 中に弾く」
ことである。

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
- YAML decoding now runs a low-level `yaml-rust2` event parser budget gate
  before constructing the public `Yaml` loader tree. Focused tests cover input
  length, scalar string length, sequence length, and node budget exhaustion.
  The event parser still allocates scalar event strings before the receiver
  sees them, but unbounded `Yaml` document tree construction is budget-gated.
- Raw shape conversion plus JSON, TOML, YAML, MsgPack, CBOR, CSV, Arrow,
  Parquet, and Avro reject non-finite floats, float-to-integer recovery, and
  signed/unsigned bounds violations through focused numeric edge tests.
- Arrow IPC and Parquet require `Seq<Record>` shapes, derive scalar schemas from
  `FieldShape`, reject malformed rows and unsupported nested/enum shapes, and
  carry the same numeric edge matrix for supported scalar rows. They now also
  consume decode budget at batch conversion time for rows, record fields, value
  nodes, strings, and bytes before copying Arrow scalar buffers into Arcweft
  `Value`; Parquet rejects metadata row-count overflow before building the
  record batch reader. Strict pre-`RecordBatch` / row-group page buffer
  materialization remains tracked under ZG-D-001.
- Avro validates supplied schemas against `TypeShape`, maps scalar, record,
  option, array, map, native unit enum, and payload enum values
  bidirectionally, enforces top-level scalar versus datum-stream policy, and
  carries the numeric edge matrix for supported scalar values. Payload enum
  variants use an Avro union of variant records in `VariantShape` order, with a
  single typed `payload` field for payload variants. It now consumes top-level
  row budget during reader iteration, avoids collecting all scalar datums before
  enforcing single-datum scalar decode, and checks row/record/string/bytes
  budgets before copying materialized `AvroValue` contents into Arcweft values.
  Strict pre-`AvroValue` nested datum materialization remains tracked under
  ZG-D-001.
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
