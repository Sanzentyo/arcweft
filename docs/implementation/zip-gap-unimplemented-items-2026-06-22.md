# ZIP Gap Unimplemented Items 2026-06-22

This note lists the concrete unfinished items for
`D:/sanze/Downloads/arcweft-zip-gap-audit-2026-06-21.zip`.
It is the readable companion to
`docs/implementation/zip-gap-open-items-2026-06-21.md`, which remains the
strict requirement ledger.

Implementation baseline used for this inventory:
`b65fd3c3 Map Avro payload enums through typed unions`.

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
| Agent | ZG-A-003 `.awfagent` formatter proof | Partial implementation | `.awfagent` の comments/trivia と Agent item 全体に対する lossless/canonical golden と idempotence 証跡 |
| Agent | ZG-A-004 Linux/macOS validation | Verification debt | Windows 以外での remote REPL / data codec focused gates と workspace gates の記録 |
| Data | ZG-D-001 parse-time budgets outside Arcweft Binary | Open implementation | JSON/TOML/YAML/MsgPack/CBOR/CSV/Arrow/Parquet/Avro が format-native value を作る前に Arcweft decode budget で止める reader/visitor 実装 |

## ZG-A-003: `.awfagent` Formatter Proof

Status: **Partial implementation**.

Existing implementation:

- `crates/arcweft-cli/src/app/tooling.rs` は `.awfagent` を formatting target
  として受け付ける。
- formatter route は `SourceDialect::Agent` を通る。
- Agent source では game-only sugar rewrite を避ける入口はある。

具体的に未実装な動作・証跡:

- `.awfagent` 専用の comments/trivia preserving golden が不足している。
- Agent declarations, effects, waits, actions, captures, resources, RAG calls
  をまとめて canonical form に整える golden が不足している。
- formatter が `format(format(input)) == format(input)` を満たすことを
  Agent dialect 全体で示す idempotence test が不足している。
- `.arcw` と `.awfagent` の dialect 差分が regression として固定されていない。
- 現在ある `.awfagent` 入口だけでは、lossless/canonical formatter である
  ことの証明にならない。

必要なテスト・証跡:

- Agent syntax family ごとの before/after golden。
- comments/trivia を含む roundtrip or lossless formatting golden。
- `.awfagent` idempotence test。
- `.arcw` 側の formatting regression。

なぜ完了扱いにできないか:

ZIP の要求は「拡張子を受け付けること」ではなく、
Agent dialect の formatter として安全に使えること。入口があるだけでは、
コメント欠落、trivia 破壊、Agent-only item の誤整形を防げない。

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
- JSON/TOML/YAML/MsgPack/CBOR/CSV/Arrow/Parquet/Avro already apply some caps or
  shape validation after parse, and several codecs check `max_input_len` before
  invoking their parser.

具体的に未実装な動作:

- JSON/TOML/YAML は format-native document/value を作った後に Arcweft raw shape
  validation へ進むため、深い nesting や巨大 node count を parse 中に止めない。
- MsgPack/CBOR は native value bridge へ移行済みだが、native value を作る前の
  node/depth/collection/string/byte budget enforcement がない。
- CSV は input cap と shape-driven row policy はあるが、reader iteration 中の
  row/field/string/byte budget を Arcweft budget として統合していない。
- Arrow IPC / Parquet は input cap と shape-driven schema validation はあるが、
  reader が column/row data を materialize する前の Arcweft budget 連携がない。
- Avro は input cap と schema/value validation はあるが、Avro datum stream を
  materialize する前の budget visitor/reader policy がない。

必要な実装:

- Format ごとに parser-integrated visitor、bounded reader、streaming reader、
  または crate-supported equivalent を導入する。
- 少なくとも input length、node count、nesting depth、collection length、
  string length、byte length のうち format が表現できるものを parse 中に数える。
- Budget 超過は Arcweft data error として structured に返し、panic や allocator
  failure に依存しない。

必要なテスト・証跡:

- 各 codec に adversarial input tests を追加する。
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

- CSV is schema-driven for scalar `Seq<Record>` rows.
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
- Raw shape conversion plus JSON, TOML, YAML, MsgPack, CBOR, CSV, Arrow,
  Parquet, and Avro reject non-finite floats, float-to-integer recovery, and
  signed/unsigned bounds violations through focused numeric edge tests.
- Arrow IPC and Parquet require `Seq<Record>` shapes, derive scalar schemas from
  `FieldShape`, reject malformed rows and unsupported nested/enum shapes, and
  carry the same numeric edge matrix for supported scalar rows.
- Avro validates supplied schemas against `TypeShape`, maps scalar, record,
  option, array, map, native unit enum, and payload enum values
  bidirectionally, enforces top-level scalar versus datum-stream policy, and
  carries the numeric edge matrix for supported scalar values. Payload enum
  variants use an Avro union of variant records in `VariantShape` order, with a
  single typed `payload` field for payload variants.
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
