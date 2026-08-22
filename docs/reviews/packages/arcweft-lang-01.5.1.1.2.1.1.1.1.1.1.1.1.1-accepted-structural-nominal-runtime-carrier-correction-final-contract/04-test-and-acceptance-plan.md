# Test and acceptance plan

各 row は実装 PR でそのまま test 名または fixture ID にする。期待結果は boolean だけでなく、error variant、index、bytes、publication count まで assert する。

## Core exact rows

| ID | level | setup / stimulus | exact assertions | requirement |
|---|---|---|---|---|
| ASNR-POS-00 | unit | catalog に nominal/layout/2 fields を登録し `RuntimeValue::try_accepted_structural_nominal` | variant は AcceptedStructuralNominal、accessors は exact identities、field order preserved | canonical construction |
| ASNR-NEG-00 | unit | same structural shape but different nominal | `NominalLayoutMismatch` or nominal mismatch; value is not constructed | nominal identity retained |
| ASNR-NEG-01 | unit | field count expected 2 / actual 1 and 3 | `FieldCount { expected: 2, actual }`; no allocation publication | arity invariant |
| ASNR-NEG-02 | unit | child at index 1 has wrong accepted type | `FieldType { index: 1 }`; input ownership handled without partial carrier | child typing |
| ASNR-POS-01 | matcher integration | nominal pattern exact, fields match | one match, transcript contains stable nominal+layout IDs | match integration |
| ASNR-NEG-03 | matcher integration | same fields/layout, nominal pattern differs | no match before child traversal; no nominal evidence emitted | no structural fallback |
| ASNR-POS-02 | matcher integration | explicitly structural pattern | layout/fields match; result is structural evidence only | boundary separation |
| ASNR-WIRE-00 | golden | encode minimal zero-field carrier | exact committed hex bytes: tag + `01 00` + canonical IDs + `00` field count | byte grammar |
| ASNR-WIRE-01 | golden | encode two fields twice in clean processes | byte-for-byte identical output; no pointer/capacity bytes | determinism |
| ASNR-WIRE-02 | negative | unknown tag | exact `UnknownRuntimeValueTag` | dispatch |
| ASNR-WIRE-03 | negative | known tag, version 2 | `UnsupportedWireVersion { found: 2 }` | versioning |
| ASNR-WIRE-04 | negative | overlong ULEB, length beyond limit, truncated child, trailing byte | each exact malformed-frame variant; no panic/OOM | canonical decoder |
| ASNR-RESTORE-00 | restart integration | snapshot carrier, restart with identical catalog | exact nominal/layout/fields and match transcript restored | roundtrip |
| ASNR-RESTORE-01 | negative restart | snapshot catalog nominal absent | typed unknown nominal; externally visible registry/task counts unchanged | two-phase abort |
| ASNR-RESTORE-02 | negative restart | layout digest changed under same display name | incompatibility; no name-based fallback | identity authority |
| ASNR-RESTORE-03 | negative restart | child 2 fails resolve after child 1 succeeds | zero prepared objects published, handles/tasks remain pre-restore | atomicity |
| ASNR-RESTORE-04 | restart graph | forward refs and permitted cycle fixture | resolve in pending graph, validate once, publish as one commit | graph restore |
| ASNR-COMPILE-00 | compile-fail | construct `AcceptedStructuralNominalRuntimeCarrier { ... }` outside owner module | private fields error | construction authority |
| ASNR-COMPILE-01 | compile-fail/lint | add extension trait to dispatch `RuntimeValue` accepted variant | architecture test/lint rejects or review gate catches; expected API remains inherent | owner rule |
| ASNR-COMPILE-02 | compile-fail | call unchecked `From`/`Default` constructor | API does not exist | no invalid state |
| ASNR-PROP-00 | property | arbitrary admitted carriers under configured limits | decode(encode(v)) semantic-equals v; encode is canonical | codec law |
| ASNR-PROP-01 | property | mutate every byte in golden corpus | decoder returns typed error or a different fully-valid value; never panic/partial publish | robustness |
| ASNR-POS-01 | requirement-specific | fixture satisfying request L75: The design must provide a compile-clean dependency order beginning with the | `AcceptedStructuralNominalRuntimeCarrier` を canonical carrier とし、所有・構築・照合・永続化の全経路を同一契約へ収束させる。 and all invariants remain true | REQ-01 |
| ASNR-NEG-01 | requirement-specific negative | minimally violate one precondition of REQ-01 | exact typed rejection; no fallback and no partial publication | REQ-01 |
| ASNR-POS-02 | requirement-specific | fixture satisfying request L88: exact live carrier acceptance plus wrong owner, field, ordinal, name, | 唯一の振る舞い所有者を既存 enum `RuntimeValue` の inherent `impl` に固定し、別 trait／side table／ad-hoc match を禁止する；対応する ASNR-POS/NEG/WIRE/RESTORE/COMPILE 行を必須 gate とし、期待 variant と no-partial-publish を明示 assert する。 and all invariants remain true | REQ-02 |
| ASNR-NEG-02 | requirement-specific negative | minimally violate one precondition of REQ-02 | exact typed rejection; no fallback and no partial publication | REQ-02 |
| ASNR-POS-03 | requirement-specific | fixture satisfying request L105: The archive must contain the complete final contract, Rust-shaped schemas, | 構造側を `AcceptedStructuralLayoutId` と宣言順 payload で表し、field count・child accepted type を checked constructor で検証する。 and all invariants remain true | REQ-03 |
| ASNR-NEG-03 | requirement-specific negative | minimally violate one precondition of REQ-03 | exact typed rejection; no fallback and no partial publication | REQ-03 |

## Required command gates for the implementation PR

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
# Run repository-specific compile-fail/snapshot/restart suites named by current AGENTS.md.
```

この design-only package では production code がないため上記 implementation gates を合格済みとは主張しない。実際に実行した command は `06-verification.md` だけを authority とする。
