# Current source evidence

## Authority snapshot

- repository: `UNAVAILABLE`
- SHA: `UNAVAILABLE`
- checkout: `UNAVAILABLE`
- working tree before packaging: `UNAVAILABLE`

## Applicable `AGENTS.md`

- checkout から `AGENTS.md` を取得できなかった。`06-verification.md` の未検証境界を参照。

## Request-named symbols found on current main

- request の backtick identifier と current checkout を定義照合できなかった。設計で新設する名前は `03-final-design-contract.md` に明示した。

## Thematically relevant current files

| score | path | evidence meaning |
|---:|---|---|

## Request-referenced predecessor artifacts

| path | current checkout | SHA-256 / status |
|---|---|---|
| `../designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-launch-receipt-keyed-ordinal-and-current-owner/OWNERSHIP_MATRIX.md` | not present | `UNVERIFIED/ABSENT` |

## Owner selection evidence

`RuntimeValue` を owner とする理由は、current source で runtime/value/carrier 意味を持つ既存 enum のうち request identifier と実装定義の双方に最も近いからである。新しい extension trait や並行 carrier enum に動作を逃がさず、`crates/arcweft-runtime/src/value.rs:1` の元の inherent `impl` に variant 操作・projection・wire dispatch を追加する。payload のデータ型 `AcceptedStructuralNominalRuntimeCarrier` は再帰サイズと visibility を分離するための struct であり、動作の別 owner ではない。
