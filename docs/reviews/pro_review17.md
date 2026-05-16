前提として、arcweft関連ではまず理念・構造を確認する方針を踏まえました。

## 結論

**今すぐリファクタした方がよいのは、`arcweft-lang-syntax` の責務過多、`arcweft-lang-hir` が実体のない再エクスポート境界になっている点、`core` と `dialogue/presentation` の依存の向き、`arcweft_oxiz_z3_parity_migration/` の未整理な残置、CLIの巨大化です。**

一方で、`arcweft-id`、`arcweft-source`、`arcweft-need`、`arcweft-verify-z3`、`arcweft-verify-oxiz` は現時点では1ファイルでも許容範囲です。むしろ先に大きな境界を直す方が効果が高いです。

## 現状の構造認識

Arcweftの設計上の中心は、`.awft` DSLから lossless CST、typed AST/HIR、Typed IR、bytecode/bundle へ進み、実行時は **Sans I/O Core** が `Engine::step(FrameInput) -> FrameOutput` 的に純粋な状態遷移を行い、GPU・FS・network・Audio・WASM・Craneliftなどは外側のHost/Adapterに置く、という構造です。設計文書でも `arcweft-core` はGPU、Audio、Servo、DOM、filesystem、network、WASM runtime、Cranelift runtimeに直接依存しないと明記されています。

ワークスペースは現在、`arcweft-core`、`arcweft-dialogue`、`arcweft-id`、`arcweft-lang-hir`、`arcweft-lang-lsp`、`arcweft-lang-syntax`、`arcweft-need`、`arcweft-presentation`、`arcweft-source`、`arcweft-test`、`arcweft-verify`、`arcweft-verify-oxiz`、`arcweft-verify-z3`、`arcweft-cli` の14クレート構成です。

ただし実装ステータス文書では「implemented workspace members」として8クレートしか列挙されておらず、現行Cargo構成とズレています。ここはコードではなくドキュメント側のリファクタ対象です。

## 現在の依存関係の見取り図

現状のCargo依存は、おおむね次の形です。

```text
arcweft-id
arcweft-source
arcweft-need

arcweft-presentation -> arcweft-id
arcweft-dialogue     -> arcweft-id, arcweft-presentation, arcweft-source
arcweft-core         -> arcweft-dialogue, arcweft-id, arcweft-need, arcweft-source

arcweft-lang-syntax  -> arcweft-core, arcweft-source, rowan, blake3
arcweft-lang-hir     -> arcweft-lang-syntax
arcweft-verify       -> arcweft-lang-hir, arcweft-lang-syntax
arcweft-verify-oxiz  -> arcweft-verify, oxiz-solver
arcweft-verify-z3    -> arcweft-verify
arcweft-lang-lsp     -> arcweft-verify, lsp-types

arcweft-test         -> arcweft-lang-hir
arcweft-cli          -> core, syntax, hir, test, verify, verify-oxiz, verify-z3
```

依存の大枠は循環しておらず、`verify-z3` が外部Z3プロセスを持ち、`verify-oxiz` がOxiZ依存を持つという分離は良いです。`verify-z3` はSMT-LIBをemitして外部コマンドを呼ぶアダプタとして分かれており、`verify-oxiz` も solver-neutral な `arcweft-verify` の外側に置かれています。 

問題は、**`arcweft-lang-syntax` が syntax 以上のことを抱えすぎていること**と、**`arcweft-core` が runtime core なのに dialogue/presentation 側を引き込んでいること**です。

## 最優先で直した方がいいもの

### 1. `arcweft-lang-syntax` の責務を分ける

`arcweft-lang-syntax/src/lib.rs` はコメント上では「surface parser」「syntax-level parsing only」「type resolution or runtime semanticsを避ける」と宣言していますが、実際には `check`、`lower`、`resolve`、`runtime_plan` まで同じクレートから公開しています。

これは現時点で一番大きい構造ズレです。`check.rs` には `TypeCheckEnv`、`TypeKind`、`typecheck_hir` などの最小型検査があり、`runtime_plan.rs` は HIR から `arcweft-core` の `RuntimePlan` や `LineTaskGroup` に落としています。 

おすすめの分割はこれです。

```text
arcweft-lang-syntax
  cst.rs
  parser.rs
  ast.rs
  expr.rs
  pattern.rs
  types.rs
  text.rs
  lint.rs
  source.rs

arcweft-lang-hir
  HIR型
  lower_to_hir
  HirLowerError

arcweft-lang-sema
  NameRegistry
  registry_from_hir
  validate_hir_references
  TypeCheckEnv
  typecheck_hir
  validate_typecheck_ready

arcweft-runtime-plan  または arcweft-lang-runtime
  lower_line_task_groups
  lower_runtime_plan
  RuntimePlanLowerError
  LinePlanLowerError
```

今の `arcweft-lang-hir` は、実体としては `arcweft-lang-syntax` からHIR関連型を再エクスポートしているだけです。これは「安定境界のつもり」にはなっていますが、実際には境界を切れていません。

`arcweft-verify` も `arcweft-lang-hir` だけでなく `arcweft-lang-syntax` の `Expr`、`LifetimeKey`、`ThreadBlock`、`lower_line_task_groups` などを直接見ています。検証器が syntax crate に戻っているので、HIR境界はまだ機能していません。

ここを切ると、以後の設計がかなり楽になります。

### 2. `core -> dialogue -> presentation` の依存を再検討する

`arcweft-core` は `arcweft-dialogue`、`arcweft-id`、`arcweft-need`、`arcweft-source` に依存しています。

さらに `arcweft-core/src/lib.rs` の `prelude` は dialogue builder系、id、need、sourceをまとめて再エクスポートしています。

これは便利ですが、Coreを「runtime semantic source of truth」として薄く保つなら、`dialogue` や `presentation` のサーフェスモデルをCoreが引き込むのはやや重いです。`dialogue` 側は `arcweft-presentation` も再エクスポートしているため、Coreが間接的にpresentation surfaceの概念まで持ち込みます。

おすすめは次のどちらかです。

```text
案A: preludeだけを facade crate に逃がす

arcweft-core      -> id, need, source のみ
arcweft-dialogue  -> id, source, presentation
arcweft           -> core, dialogue, presentation, need, id, source を re-export
```

または、

```text
案B: coreを runtime と model に割る

arcweft-runtime-core
  Engine, FrameInput, FrameOutput, RuntimePlan, FlowOp

arcweft-model
  id, source anchor, dialogue/presentationにまたがる純データ

arcweft-core
  facade / compatibility layer
```

現時点では案Aで十分だと思います。`arcweft-core::prelude` をすぐ消す必要はありませんが、Core本体から dialogue/presentation convenience を外す方向に寄せた方が、Sans I/O境界が保ちやすいです。

### 3. `arcweft_oxiz_z3_parity_migration/` を整理する

リポジトリ直下に `arcweft_oxiz_z3_parity_migration/` が残っており、その中に `crates/arcweft-cli/tests/verify_oxiz_z3_parity.rs` や大量の `verify/oxiz_z3_parity` fixture が存在します。 

これは現行ワークスペースの `members` には含まれていません。つまり、通常の `cargo test --workspace` では基本的に検証されない一方で、検索やレビュー時には実装の一部のように見えます。

これはかなり優先度高めで整理した方がいいです。

選択肢は3つです。

```text
1. 本当に使うなら:
   crates/arcweft-verify-parity または verify/fixtures に移し、
   workspace member または明示的な test harness にする。

2. 資料として残すだけなら:
   docs/archive/oxiz-z3-parity-migration/ に移し、
   READMEで「実行対象外」と明記する。

3. もう不要なら:
   削除する。
```

現状のように「workspace外だがcrates/tests風の構造を持つ」のが一番危ないです。

### 4. `core/src/lib.rs` を分割する

`arcweft-core/src/lib.rs` は、prelude、時刻、FrameInput/FrameOutput、RuntimeValue、RuntimeExpr、RuntimePattern、RuntimeEnv、RuntimePlan、FlowOp、Engine、TaskHost、SourceEvent、LineTaskGroup、LineEffectRequest、ResourceAccess などが1ファイルに入っています。

これは現段階では動きますが、今後のRuntime/VM/Task/Source/LinePlanの伸び方を考えると、早めに分けた方がいいです。

おすすめのmod分割はこの程度です。

```text
crates/arcweft-core/src/
  lib.rs
  prelude.rs
  time.rs
  frame.rs
  value.rs
  expr.rs
  pattern.rs
  env.rs
  plan.rs
  engine.rs
  task.rs
  source.rs
  line_task.rs
  effect.rs
  resource.rs
  error.rs
```

最初から細かくしすぎる必要はありません。まずは `engine.rs`、`plan.rs`、`line_task.rs`、`effect.rs`、`task.rs`、`value.rs` くらいで十分です。

### 5. `cli/src/main.rs` をコマンド単位で分ける

`arcweft-cli/src/main.rs` は、コマンドdispatch、引数parse、`check`、`verify`、`unsafe`、`plan`、`run`、`test`、`bench`、`load_and_check`、JSON出力、runtime report生成まで1ファイルに入っています。

CLIはI/O境界なので多少大きくても許容できますが、すでにコマンド数が増えています。ここは今後確実に肥大化します。

おすすめはこれです。

```text
crates/arcweft-cli/src/
  main.rs
  args.rs
  check.rs
  verify.rs
  unsafe_audit.rs
  runtime_plan.rs
  runtime_run.rs
  script_test.rs
  script_bench.rs
  load.rs
  output.rs
```

特に `load_and_check` はCLI以外のツールでも使いたくなる可能性が高いので、将来的には `arcweft-tooling` 的なSans I/O寄りの crate に切り出す候補です。ただし今は `fs::read_to_string` を含むので、純粋関数版とCLI I/O版を分けるのがよいです。

## 中優先で直すとよいもの

### 6. `arcweft-presentation` の1ファイル構成

`arcweft-presentation` は `PresentationScope`、`PresentationSlot`、`PresentationTarget`、`PresentationHandle`、`SlotRef`、`SlotValue`、`ClearPresentation`、`BackgroundSurface`、`CharacterSurface`、`PresentationRegistry`、ヘルパー関数、テストまで1ファイルです。

まだ許容範囲ですが、すでに概念が増えています。

```text
presentation/
  lib.rs
  id.rs          // scope / target / slot
  handle.rs      // PresentationHandle, SlotRef, SlotValue, ClearPresentation
  registry.rs    // PresentationRegistry
  surface.rs     // BackgroundSurface, CharacterSurface
  helpers.rs     // bg, show_character, hide_character, asset...
```

特に `PresentationRegistry` はテストも含めて `registry.rs` に移すと見通しが良くなります。

### 7. `arcweft-dialogue` のpresentation再エクスポートを狭める

`arcweft-dialogue` は dialogue model と builder のほかに、`arcweft_presentation` の型や関数を広く再エクスポートしています。

DialogueがPresentationを使うのは自然ですが、`BackgroundSurface`、`PresentationRegistry`、`SlotValue` などまでDialogue crateから見えると、境界が曖昧になります。

おすすめは、Dialogueに必要な adapter だけを残すことです。

```text
arcweft-dialogue
  model.rs
  builder.rs
  content.rs
  presentation_adapter.rs  // show, hide, character_ref 程度
```

もし `bg` や `PresentationRegistry` が必要なら、`arcweft-presentation` から直接使う方が分かりやすいです。

### 8. `arcweft-verify` を report / SMT / collector に分ける

`arcweft-verify/src/lib.rs` は、公開schema、SMT IR、SMT emit、`SmtBackend` trait、`ObligationCollector`、collectorの走査ロジック、テストまで入っています。 

今すぐ壊れているわけではありませんが、検証器は今後拡張されるので分割した方がよいです。

```text
verify/
  lib.rs
  policy.rs
  report.rs
  obligation.rs
  smt.rs
  collector.rs
  diagnostics.rs
```

`verify-z3` と `verify-oxiz` の分離は良いので、そこは維持でよいです。

### 9. `arcweft-lang-lsp` の名前を見直す

`arcweft-lang-lsp` は実態として「verifier reportをLSP diagnostics/code actionsへ変換するcrate」です。ソースコメントにも verifier diagnostics helper と書かれており、依存も `arcweft-verify` と `lsp-types` が中心です。

今のままでも動きますが、名前としては少し広すぎます。

選択肢は2つです。

```text
1. 今の実態に合わせる:
   arcweft-verify-lsp にリネーム

2. 名前に合わせて拡張する:
   parse/typecheck/name-resolution diagnostics も扱う lang LSP crate にする
```

現時点では、`arcweft-verify-lsp` の方が実態に合っています。

## test関数・テスト配置について

Rust側のテスト配置は、全体としては悪くありません。

`arcweft-lang-syntax` は `src/tests/mod.rs` に内部テストを機能別に分け、さらに `tests/parser_p0.rs` と `tests/parser_p1.rs` に外部API寄りのテストを置いています。これは良い配置です。  

ただし、`parser_p0.rs` と `parser_p1.rs` の両方に `parse_ok` helper が重複しています。内部テスト側にも `src/tests/support.rs` があり、こちらにも `parse_ok` や共通helperがあります。

ここは小さく直せます。

```text
crates/arcweft-lang-syntax/tests/
  support/mod.rs
  parser_p0.rs
  parser_p1.rs
```

として、integration test側の `parse_ok` を共通化するとよいです。

`arcweft-dialogue` は `#[cfg(test)] mod tests;` から `src/tests/builder.rs` と `src/tests/model.rs` に分かれており、これは良いです。 

`arcweft-cli` は binary を起動する integration test として `tests/check.rs` に置かれており、配置方針は正しいです。ただし、ファイル名が `check.rs` なのに、実際には `check`、`verify`、`unsafe`、`plan`、`run`、`test`、`bench` まで全部見ています。

ここは次のように分けるとメンテしやすいです。

```text
crates/arcweft-cli/tests/
  support.rs
  check.rs
  verify.rs
  unsafe_audit.rs
  plan.rs
  run.rs
  script_test.rs
  bench.rs
```

DSLの `test` / `bench` 宣言については、`arcweft-test` がHIRから manifest を抽出する形になっており、配置はかなり良いです。`arcweft-test` は実行せず、CLI/LSP/runtime adapter向けの安定manifestを作るという責務に絞られています。

## 逆に、まだリファクタしなくていいもの

`arcweft-id`、`arcweft-source`、`arcweft-need` は小さな基盤crateとして今の1ファイル構成でよいです。`id` は `EntityId`、`PublicId`、`TextKey` とvalidation、`need` は `Need<T, E>` と `Progress`、`source` は `SourceAnchor` と位置情報に絞られています。  

`arcweft-verify-z3` と `arcweft-verify-oxiz` も、今は単一責務が明確なので1ファイルでも問題ありません。むしろ `arcweft-verify` 本体を先に分けるべきです。 

## 推奨リファクタ順

1. **`arcweft_oxiz_z3_parity_migration/` を整理する。** workspace外の疑似crate/test構造は混乱源です。
2. **`arcweft-lang-syntax` から HIR / sema / runtime_plan を切り出す。** これが最も設計効果が高いです。
3. **`arcweft-lang-hir` を実体のある境界にする。** 再エクスポートcrateから卒業させます。
4. **`arcweft-core` の `dialogue` 依存と prelude を見直す。** facade crateに逃がすのが安全です。
5. **`core/src/lib.rs` と `cli/src/main.rs` を分割する。** 巨大化が進む前に切ると後が楽です。
6. **`presentation` / `dialogue` / `verify` を中粒度に分ける。** 急ぎではないですが、今なら低コストです。
7. **CLI tests と syntax integration test helper を整理する。** 小さいが効果のある整頓です。

現時点の一番大事な判断は、**ファイルを細かく分ける前に、`syntax -> hir -> sema -> verify/runtime-plan -> core` の意味的な境界を先に切ること**です。ファイル分割だけ先に進めると、見た目は綺麗になりますが、依存の濁りは残ります。
