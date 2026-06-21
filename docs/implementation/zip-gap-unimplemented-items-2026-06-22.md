# ZIP Gap Unimplemented Items 2026-06-22

This note lists the concrete unfinished items for
`D:/sanze/Downloads/arcweft-zip-gap-audit-2026-06-21.zip` as of the current
checkout. It is a readable companion to
`zip-gap-open-items-2026-06-21.md`, which remains the strict requirement
ledger.

## Status Model

未実装として扱う範囲を次の 3 種類に分ける。

- **Open implementation**: 現在のソースが ZIP の対象仕様にまだ達していない。
  入口や型はあっても、動作が不足している。
- **Partial implementation**: 初期実装はあるが、ZIP の受け入れ条件を満たす
  ほどの動作・エラー・テストがそろっていない。
- **Verification debt**: 実装はある可能性が高いが、要求された検証証跡がない
  ので完了扱いにできない。

## Summary

| Area | Item | Status | Current blocker |
| --- | --- | --- | --- |
| Agent | ZG-A-001 REPL project-bound binding policy | Partial implementation | Project hash change時の binding preserve/drop diagnostics とテストが不足 |
| Agent | ZG-A-002 stdio MCP transport hardening | Partial implementation | timeout、bounded stderr、graceful shutdown-before-kill が不足 |
| Agent | ZG-A-003 `.awfagent` formatter proof | Partial implementation | comments/trivia と Agent item 全体の lossless/canonical golden が不足 |
| Agent | ZG-A-004 Linux/macOS validation | Verification debt | Windows 以外の現在証跡が未記録 |
| Data | ZG-D-001 parse-time budgets outside Arcweft Binary | Open implementation | 多くの codec が format-native value を先に materialize している |
| Data | ZG-D-002 derive shape generation policy gaps | Partial implementation | generics、tuple/unit policy、repr range の trybuild 証跡が不足 |
| Data | ZG-D-004 Arrow IPC / Parquet shape guidance | Open implementation | value inference から schema を作っており `TypeShape` 主導ではない |
| Data | ZG-D-005 Avro shape fidelity | Open implementation | Avro schema と Arcweft `TypeShape` の対応検証が不足 |
| Data | ZG-D-009 numeric edge-case policy matrix | Partial implementation | codec 横断の bounds / NaN / infinity policy test が不足 |

## Agent Items

### ZG-A-001: REPL project-bound binding policy

The REPL has remote session support and serializable binding snapshots, but it
does not yet make the project boundary explicit enough. When a remote
`:connect` targets a different `program_hash`, the implementation must
distinguish between self-contained bindings and session-bound bindings.

What remains:

- Preserve primitive, string, and collection bindings that do not depend on the
  old remote session.
- Drop observation, resource, RAG, and other session-bound bindings when the
  project hash changes.
- Report structured diagnostics or report fields that explain every preserve
  or drop decision.
- Add tests that connect to two different remote project hashes and assert the
  preservation, drop, and diagnostic behavior.

Why it is not complete:

The current behavior can make remote REPL state look portable across
incompatible projects without telling the user which bindings are still valid.

### ZG-A-002: stdio MCP transport hardening

The stdio MCP adapter can spawn a child process and pass a fake-child
roundtrip, but it is not hardened as a production transport.

What remains:

- Add request timeout enforcement instead of blocking indefinitely on a line
  read.
- Retain stderr in a bounded buffer so failures have context without unbounded
  memory growth.
- Attempt protocol/process shutdown before falling back to killing the child.
- Add fake-child tests for timeout, stderr retention, and shutdown-before-kill.

Why it is not complete:

A hung or noisy child process can still stall the REPL or lose the most useful
failure context.

### ZG-A-003: `.awfagent` formatter proof

The formatting entrypoint accepts `.awfagent`, and the route is dialect-aware.
That is only the entrypoint slice, not proof that the Agent formatter is
lossless and canonical.

What remains:

- Add golden formatting fixtures for comments and trivia.
- Add golden fixtures for Agent declarations, effects, waits, actions,
  captures, resources, and RAG calls.
- Add idempotence tests showing that formatting already-formatted `.awfagent`
  input produces stable output.
- Keep `.arcw` regressions so Agent-specific formatting does not break game
  syntax formatting.

Why it is not complete:

The ZIP target is formatter behavior, not merely extension recognition.

### ZG-A-004: Linux/macOS validation

The focused cuts have Windows validation, but there is no current recorded
Linux/macOS evidence for the remote REPL and data-codec work.

What remains:

- Run the focused remote REPL gates on Linux, macOS, and Windows.
- Run the relevant workspace gates on all three platforms or record CI evidence.
- Record the command, platform, and result in the implementation notes.

Why it is not complete:

Process behavior, line endings, stdio buffering, and shell invocation differ
enough across platforms that Windows-only evidence cannot close this item.

## Data Items

### ZG-D-001: parse-time budgets outside Arcweft Binary

`arcweft-data::DecodeBudget` exists and Arcweft Binary uses parse-time checks,
but most other codecs still parse into format-native values before Arcweft's
limits can reject hostile input.

What remains:

- Add parser-integrated visitors, bounded readers, or equivalent early budget
  checks for JSON, TOML, YAML, MsgPack, CBOR, CSV, Arrow, Parquet, and Avro.
- Cover input length, node count, nesting depth, collection length, string
  length, and byte length where the format can express them.
- Add adversarial tests that fail before unbounded intermediate allocation.

Why it is not complete:

Post-parse validation is too late for hostile inputs that allocate huge native
documents first.

### ZG-D-002: derive shape generation policy gaps

`arcweft-data-derive` now has a typed attribute parser and trybuild coverage for
many malformed attributes, but several derive surfaces are still not nailed
down.

What remains:

- Generate precise generic bounds or reject unsupported generic derive surfaces
  with explicit compile errors and tests.
- Define tuple struct and unit struct support policy, then cover it with
  trybuild pass/fail fixtures.
- Define multi-field tuple enum variant support policy and test it.
- Validate repr enum discriminants against the selected repr range.

Why it is not complete:

Unsupported derive surfaces must either produce correct shapes or fail with
clear compile errors. Silent partial generation would recreate the original
package gap.

### ZG-D-004: Arrow IPC and Parquet shape guidance

The Arrow/Parquet codec currently derives schema information from observed
values. It ignores the supplied Arcweft shape in the places that matter for
schema fidelity.

What remains:

- Map `TypeShape` to Arrow schema fields.
- Encode rows strictly against that schema rather than inferring from observed
  keys or first values.
- Decode back through `TypeShape` validation.
- Define and test option/null policy.
- Add record, enum, and malformed row tests.
- Add decode limit tests.

Why it is not complete:

Value inference can widen, null, or reshape data based on the sample values
instead of the declared Arcweft contract.

### ZG-D-005: Avro shape fidelity

The Avro codec uses an Avro schema for the reader/writer, but it does not yet
prove that the schema corresponds to Arcweft `TypeShape` semantics.

What remains:

- Generate or validate Avro schemas from `TypeShape`.
- Map Arcweft records, enums, options/unions, maps, and scalar rows
  bidirectionally.
- Check enum variant indices and payload shapes against `VariantShape`.
- Define top-level scalar versus row-set behavior.
- Add strict error-case tests and decode limit tests.

Why it is not complete:

Avro schema compatibility alone does not guarantee Arcweft shape compatibility.

### ZG-D-009: numeric edge-case policy matrix

Central raw numeric conversion now performs checked integer bounds, but numeric
behavior is not yet proven consistently across every relevant codec.

What remains:

- Add shared policy tests for signed/unsigned crossings.
- Reject out-of-range integer values consistently.
- Reject float-to-integer recovery unless a codec policy explicitly allows it.
- Define and test NaN and infinity encode/decode behavior.
- Apply the matrix to JSON, TOML, YAML, MsgPack, CBOR, CSV, Arrow, Parquet, and
  Avro as applicable.

Why it is not complete:

Without a cross-codec matrix, individual adapters can still silently cast,
null, or preserve non-finite values differently.

## Already Covered Slices

The following slices should not be counted as currently unimplemented, though
some of them leave related items open:

- CSV is schema-driven for scalar `Seq<Record>` rows.
- HTTP codec negotiation rejects ambiguous content and enforces body caps at
  the adapter boundary.
- `CodecRegistry` rejects duplicate ids, media types, extensions, and aliases.
- Config merge is shape-aware and provenance-producing.
- Save decoding supports explicit multi-step migration chains.
- JSON, TOML, YAML, MsgPack, and CBOR have moved away from the earlier broad
  JSON bridge for the covered raw-shape paths.

## Completion Rule

Do not close the ZIP goal until every item above is either implemented and
tested or intentionally moved out of scope in a repository-visible note. A
workspace `cargo clippy --workspace --all-targets --all-features` pass is
necessary at reviewable cut points, but it is not sufficient for items that
require malformed input tests, cross-format roundtrips, process behavior tests,
or platform matrix validation.
