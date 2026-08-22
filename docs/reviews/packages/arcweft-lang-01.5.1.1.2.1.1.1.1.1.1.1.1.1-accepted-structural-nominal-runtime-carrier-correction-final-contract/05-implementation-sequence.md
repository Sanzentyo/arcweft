# Implementation sequence

各 step は前 step の tests を green にしてから進める。production PR はこの順を入れ替えて invalid intermediate state を main に入れない。

## 0. Baseline and owner lock

- checkout `UNAVAILABLE` を parent にする。
- applicable `AGENTS.md` を再読し、`crates/arcweft-runtime/src/value.rs` を支配する最深ファイルの指示を PR checklist に転記する。
- current `RuntimeValue` definition、inherent impl、all exhaustive matches、wire tags、visitors、snapshot/restore entrypoints を `rg` で inventory 化する。
- baseline fmt/clippy/tests を記録する。既存失敗は変更前ログと分離する。

**Gate:** production diff 0、baseline SHA/log 固定。

## 1. Payload invariant type

Target: `crates/arcweft-runtime/src/value.rs` または owner module が既に payload structs を置く sibling file。

- `AcceptedStructuralNominalRuntimeCarrier` と `AcceptedStructuralNominalCarrierError` を追加。
- fields private、`try_new` + borrow-only accessors のみ。
- accepted catalog の既存型を使い、同義の `AcceptedNominalCatalog` を新設しない。
- affine child を clone しない validation API に合わせる。

**Gate:** ASNR-POS-00, NEG-00..02, COMPILE-00/02。

## 2. Existing enum variant and inherent behavior

Target: `crates/arcweft-runtime/src/value.rs:1` の `RuntimeValue`。

- `AcceptedStructuralNominal(Box<AcceptedStructuralNominalRuntimeCarrier>)` を元 enum に追加。
- 元の inherent `impl` に validating constructor と projection を追加。
- compiler が示す全 exhaustive matches を semantic mapping table に従って更新。
- wildcard arm を追加して compiler errors を消すことは禁止。

**Gate:** owner enum unit tests、size/layout policy、drop/visit/format tests。

## 3. Checker/admission handoff

- accepted semantic fact producer が nominal ID、accepted layout ID、declaration-order children を一度だけ materialize する。
- runtime producer は `RuntimeValue::try_accepted_structural_nominal` を呼ぶ。
- structural inference、display-name lookup、side-map insertion を削除／非採用。

**Gate:** REQ-specific POS/NEG rows、no unchecked constructor call (`rg`)。

## 4. Generic match and coverage

- matcher/transcript/coverage call sites を owner projection に移行。
- nominal first、then layout、then children の比較順を固定。
- explicit structural pattern の結果型から nominal proof を生成しない。
- all owner variants の mapping test を table-driven にする。

**Gate:** ASNR-POS-01/02、NEG-03、request の match/coverage rows。

## 5. Canonical codec

- existing wire-tag owner tableで未使用 tag を割り当て、値を golden fixture に固定。
- `RuntimeValue` inherent encode/decode dispatch と pending payload codec を追加。
- limits/canonical ULEB/trailing-byte checks を先に実装し、allocation は検査後。

**Gate:** ASNR-WIRE-00..04、PROP-00/01。

## 6. Two-phase restore

- pending carrierには raw/stable identities と pending child refs だけを保持。
- catalog/object resolution 後、同じ `try_new` で prepared value を作る。
- coordinator の既存 commit barrier に prepared value を載せ、全 graph 成功後のみ publish。
- abort path の registry/task/handle count を tests で観測可能にする。test-only hook を production authority にしない。

**Gate:** ASNR-RESTORE-00..04。

## 7. Compatibility and documentation

- explicit v1 grammar、wire tag、legacy rejection/migration policy を repository contract doc に反映。
- any snapshot schema digest/catalog manifest を更新。
- predecessor contracts の status/index がある場合はこの correction を successor として link する。

**Gate:** golden files reviewed、old fixtures の accept/reject matrix complete。

## 8. Full admission

- `04-test-and-acceptance-plan.md` の全 row を実装 tests に対応付ける。
- fmt/clippy/all tests/compile-fail/restart suite を clean checkout で実行。
- `rg` で extension trait、unchecked construction、structural nominal fallback、Debug serialization が 0 件であることを確認。
- source line citations を implementation SHA へ更新し、design SHA と実装 SHA の差分を記録。

**Rollback boundary:** codec tag を release artifact へ出す前なら variant/codec commit を一括 revert。release 後は tag を再利用せず、versioned migration/rejection を追加する。
