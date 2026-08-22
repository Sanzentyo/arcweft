# Accepted structural nominal runtime carrier correction — final contract

- **基準 repository**: `UNAVAILABLE`
- **実際に検査した Git SHA**: `UNAVAILABLE`
- **checkout 表示**: `UNAVAILABLE`（`origin/main` の detached checkout でも可。SHA を authority とする）
- **成果物種別**: design-only。production source／patch／overlay は含めない。
- **主 request**: `REQUEST.md`
- **OPEN_QUESTIONS**: **0**

## 結論

accepted structural value と nominal identity を別経路に保存する設計を廃し、既存 runtime owner enum `RuntimeValue` が直接保持する `AcceptedStructuralNominalRuntimeCarrier` へ統合する。carrier は checker/admission を通った値だけを表し、構築・match projection・canonical encode・二相 restore の権威を `RuntimeValue` の inherent `impl` に集約する。同形異名目を同一視する fallback、restore 時だけの side table、extension trait による後付け動作は採用しない。

## 読み順

1. `01-request-traceability.md` — request の必須項目と設計／テストの 1:1 対応。
2. `02-current-source-evidence.md` — current main と親契約の実 source evidence。
3. `03-final-design-contract.md` — 実装可能な型・API・状態遷移・wire grammar。
4. `04-test-and-acceptance-plan.md` — exact test rows と assertions。
5. `05-implementation-sequence.md` — dependency 順の編集計画。
6. `06-verification.md` — 実行済み検証と未検証境界。
7. `07-package-manifest.sha256` — package integrity。

## 閉じた決定

- canonical runtime owner: `RuntimeValue` (`crates/arcweft-runtime/src/value.rs:1`)
- canonical payload carrier: `AcceptedStructuralNominalRuntimeCarrier`
- nominal identity: `AcceptedNominalTypeId`
- structural/layout identity: `AcceptedStructuralLayoutId`
- recursive payload: owner value `RuntimeValue` の declaration-order boxed slice
- construction: checked/admitted `try_new` のみ
- behavior owner: `RuntimeValue` の既存 inherent `impl`
- persistence: explicit tag/version canonical bytes
- restore: decode/resolve/validate と publish を分離した二相 commit
- compatibility: exact nominal + layout identity。structural coincidence のみでは互換にしない
