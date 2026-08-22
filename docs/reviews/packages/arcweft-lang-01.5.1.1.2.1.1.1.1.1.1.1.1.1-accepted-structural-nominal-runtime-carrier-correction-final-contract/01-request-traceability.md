# Request traceability

- Request SHA-256: `cbe6a1f1f20f2c5c11df678b8098165ce8931820ece459c7bf1cf203be7bc5a4`
- 抽出した normative rows: **3**
- 各行は request の行番号へ戻せる。1 行を複数の曖昧な設計項目へ丸投げせず、owner・解決・test gate を同じ行で固定する。

| ID | request 行／section | exact requirement | concrete resolution | owner / target | acceptance rows |
|---|---:|---|---|---|---|
| REQ-01 | L75 / Required implementation order | The design must provide a compile-clean dependency order beginning with the | `AcceptedStructuralNominalRuntimeCarrier` を canonical carrier とし、所有・構築・照合・永続化の全経路を同一契約へ収束させる。 | `04-test-and-acceptance-plan.md (ASNR-* rows)` | ASNR-POS-01, ASNR-NEG-01 |
| REQ-02 | L88 / Required tests | exact live carrier acceptance plus wrong owner, field, ordinal, name, | 唯一の振る舞い所有者を既存 enum `RuntimeValue` の inherent `impl` に固定し、別 trait／side table／ad-hoc match を禁止する；対応する ASNR-POS/NEG/WIRE/RESTORE/COMPILE 行を必須 gate とし、期待 variant と no-partial-publish を明示 assert する。 | `crates/arcweft-runtime/src/value.rs:1 `RuntimeValue` + `AcceptedStructuralNominalRuntimeCarrier`` | ASNR-POS-02, ASNR-NEG-02 |
| REQ-03 | L105 / Required returned archive | The archive must contain the complete final contract, Rust-shaped schemas, | 構造側を `AcceptedStructuralLayoutId` と宣言順 payload で表し、field count・child accepted type を checked constructor で検証する。 | `03-final-design-contract.md — `AcceptedStructuralNominalRuntimeCarrier` contract` | ASNR-POS-03, ASNR-NEG-03 |

## Coverage rule

上表の各 REQ 行は `04-test-and-acceptance-plan.md` の同番号 POS/NEG 行と結合する。実装 PR は REQ 行を削除・統合してはならず、該当 test ID が存在しない状態では admission 不可とする。
