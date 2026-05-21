前提資料どおり、まず arcweft の理念・構造を見たうえで、現行 `main` のコードと設計文書をレビューしました。 直近では 2026-05-21 に `.arcw` 拡張子へ寄せるコミットがあり、現在の前提は `arcw` CLI / `.arcw` source と見てよさそうです。

## レビュー結論

**計測基盤を先に作る判断は、arcweft では特に正しいです。** 理由は、すでに `syntax -> hir -> sema -> runtime-plan/verify -> tooling/cli` という層があり、CLI が parse / lower / resolve / typecheck / verify / runtime lowering を一本のパイプラインで通しているからです。  ここにフェーズ別・ステップ別の計測を入れるだけで、言語開発のボトルネックがかなり見える構造になっています。

一方で、**型システムの健全性を自己検証するには、今の `typecheck_hir` が “OK/Err を返す checker” に留まっていて、型判断の証跡を残していない点が最大の不足です。** `typecheck_hir` は `TypeChecker` を作って module を検査し、エラーがなければ `Ok(())` を返しますが、各 HIR node がどの規則でどの型になったかは保持していません。 まずここを `TypeCheckReport` / `TypeJudgment` へ拡張するのが本筋です。

---

## 良い点

arcweft は、自己検証とプロファイルを入れやすい構造にかなり近いです。

まず、core runtime は Sans I/O 境界を持っており、host が pure data を入れて pure output batch と stop reason を受け取る設計になっています。 `RuntimeExecutor` も `step` と `fiber` だけの小さい境界で、VM を意味論の source of truth にする設計です。 これはプロファイラにも健全性テストにも非常に相性が良いです。

次に、verification 側もすでに Sans I/O です。`arcweft-verify` は proof obligation / diagnostics / solver-neutral proof problem を作る crate で、具体的な solver や I/O は adapter 側に置く方針です。 設計文書上も、semantic pass の `SemanticReport` を source of truth とし、lifetime promotion、unsafe lifetime、upper-lifetime write、thread capture、effect capability、proof body、Raw syntax、runtime conflict、MustDrop などを obligation 化する方針が明文化されています。

さらに、テストもかなり良いです。typecheck tests は route binding、try、return mismatch、raw expression、presentation handles、lifetime registry、borrow escape、line lifetime、thread capture、array mismatch などをすでに押さえています。   semantic tests も MustDrop、branch/cancel path、proof reference、axiom、thread join typing、runtime conflict、effect capability、unsafe audit を検査しています。 

---

## いま直すべきギャップ

### 1. `arcw bench` はまだ “計測” ではなく “bench 宣言の検証” に近い

設計文書にも、現在の `arcw bench` は `measure` section を要求して `validated / skipped / failed` を返す段階で、renderer/audio/wall-clock/allocation counters/benchmark timing は adapter work とされています。 実装でも `script_bench_selection` は `collect_script_tests` から bench を集め、`validate_script_bench` に通すだけで、headless runtime execution や timing をしていません。

`Engine::step` には `executed_ops` がローカル変数として存在し、`max_ops` budget と stop reason の判定に使われています。 しかし `RuntimeStepResult` は `output / fiber_status / stop_reason` だけで、`executed_ops` や queue 長、source/stream 処理数などの統計を返していません。 ここを public stats にするのが最初の改善です。

### 2. 数値型が仕様より粗い

これはかなり重要です。仕様では、Arcweft は `i8/i16/i32/i64/i128/u8/.../usize/f32/f64` の明示幅 numeric primitive を使い、`int` / `uint` / `float` / `Number` のような concrete fallback は持たず、unsuffixed numeric literal は expected type が必要だと定めています。

しかし実装の `TypeKind` はまだ `Int` / `Float` を持っています。 さらに `Literal::Int(_) -> TypeKind::Int`、`Literal::Float(_) -> TypeKind::Float` になっており、`named_type_label` でも `"i32" | "i64" | "usize" | "Int" => TypeKind::Int`、`"f32" | "f64" | "Float" => TypeKind::Float` に畳まれています。 

型健全性の自己検証をやるなら、最初の固定点はここです。**型が粗いまま preservation/progress を検査しても、`i32` と `usize` の混同を検出できません。**

### 3. solver 結果が verification report に反映されていない

設計上は Z3 / OxiZ / SMT-LIB external / Kani / Creusot / Verus / runtime contract checks / property test generator まで見据えています。 ただし現在の `ProofExpr` は `Bool / Var / Not / And / Or / Eq / App` の最小構成で、設計文書にある `Int / Le / Forall / Exists` まではまだありません。 OxiZ adapter も、非空 assertion は ProofExpr-to-OxiZ lowering 未実装のため `Unknown` を返します。

また CLI の `solve_report` は solver outcome を stderr に出すだけで、`VerificationReport` の diagnostics/severity/exit code に戻していません。 つまり現状の solver は「参考出力」であって、「自己検証に失敗したら CI を落とす」段階にはまだなっていません。

### 4. `verify_module` が adapter/profile の `TypeCheckEnv` を受け取っていない

`arcw check --profile` や adapter context の設計では、generic check は strict、profile 選択時だけ adapter context を適用する方針になっています。 しかし `verify_module` は内部で `TypeCheckEnv::new()` を作って `analyze_semantics` に渡しています。

今後、profile-selected adapter context が semantic obligation の discharge に関わるなら、`verify_module_with_env(module, env, policy)` を追加して、CLI 側では `load_and_check_selection` と同じ env を verifier に渡すべきです。

---

## 改善計画

### Phase 0: 計測 spine を core/CLI に通す

最初の PR はこれが良いです。

`arcweft-core::step` に `RuntimeStepStats` を追加します。

```rust
pub struct RuntimeStepStats {
    pub executed_ops: usize,
    pub pending_ops_before: usize,
    pub pending_ops_after: usize,
    pub task_events_in: usize,
    pub source_events_in: usize,
    pub source_events_emitted: usize,
    pub stream_events_emitted: usize,
    pub line_effects: usize,
    pub diagnostics: usize,
}

pub struct RuntimeStepResult {
    pub output: RuntimeStepOutput,
    pub fiber_status: FlowFiberStatus,
    pub stop_reason: RuntimeStepStopReason,
    pub stats: RuntimeStepStats,
}
```

壁時計時間は core に入れません。core は Sans I/O を守り、CLI/tooling 側で `Instant` を使って `parse / lint / lower / resolve / readiness / typecheck / line_task_lower / verify / runtime_lower / run` の elapsed を測ります。core runtime 側は deterministic counter、CLI 側は elapsed time、という分担にします。

追加コマンドは `arcw profile` がよいです。

```bash
arcw profile game/routes/opening.arcw --mode drain --steps 64 --json
arcw profile --manifest arcw.toml --profile bench.opening --json
```

出力は最低限これで十分です。

```json
{
  "source": "game/routes/opening.arcw",
  "phases": [
    { "name": "parse", "elapsed_ns": 1200000 },
    { "name": "typecheck", "elapsed_ns": 900000 },
    { "name": "verify", "elapsed_ns": 700000 }
  ],
  "runtime": {
    "steps": [
      { "index": 0, "executed_ops": 32, "stop_reason": "budget_exhausted" }
    ]
  }
}
```

この段階で CI には wall-clock gate を入れず、`executed_ops` や node count のような deterministic budget だけを gate にします。wall-clock はローカル最適化と trend 観察用です。

### Phase 1: `arcw bench` を headless 実行できる範囲だけ実測化する

今の `arcw bench` は validate 中心なので、headless で実行可能な `measure` だけを実行します。adapter-only section は今まで通り skipped でよいです。

追加 option:

```bash
arcw bench game/routes/opening.arcw --steps 64 --iterations 50 --warmup 5 --json
```

report には以下を入れます。

```json
{
  "id": "bench.opening",
  "status": "measured",
  "warmup": 5,
  "iterations": 50,
  "steps": 64,
  "elapsed_ns": {
    "min": 1000000,
    "median": 1200000,
    "max": 1600000
  },
  "deterministic": {
    "executed_ops_median": 128,
    "effects_median": 8,
    "diagnostics": 0
  }
}
```

これで「高速化できたか」を見るだけでなく、「言語仕様変更で実行 op 数が増えていないか」も見られるようになります。

### Phase 2: 型検査を `TypeCheckReport` 化する

ここから型健全性の自己検証に入ります。

今の API は残しつつ、内部的には新しい API を作ります。

```rust
pub fn analyze_types(module: &HirModule, env: &TypeCheckEnv) -> TypeCheckReport;

pub fn typecheck_hir(module: &HirModule, env: &TypeCheckEnv) -> Result<(), Vec<TypeCheckError>> {
    analyze_types(module, env).into_result()
}
```

`TypeCheckReport` は少なくともこれを持ちます。

```rust
pub struct TypeCheckReport {
    pub diagnostics: Vec<TypeCheckError>,
    pub judgments: Vec<TypeJudgment>,
    pub symbols: TypeSymbolTable,
}

pub struct TypeJudgment {
    pub node: HirNodeRef,
    pub ty: TypeKind,
    pub rule: TypeRule,
    pub premises: Vec<JudgmentId>,
}
```

この `judgments` が「型システムの自己検証」の入力になります。つまり checker が「通した/落とした」だけではなく、**なぜその型になったか**を machine-readable に残します。

### Phase 3: 数値型を仕様どおり precise にする

`TypeKind::Int` / `TypeKind::Float` を段階的に置き換えます。

```rust
pub enum TypeKind {
    Primitive(PrimitiveType),
    // ...
}

pub enum PrimitiveType {
    Bool,
    Char,
    String,
    Unit,
    Int(IntType),
    Float(FloatType),
    Duration,
    Color,
    Ratio,
    Length,
    Angle,
}

pub struct IntType {
    pub signed: bool,
    pub bits: IntBits,
}

pub enum IntBits {
    B8,
    B16,
    B32,
    B64,
    B128,
    Size,
}

pub enum FloatType {
    F32,
    F64,
}
```

unsuffixed literal は即 `i32` にせず、expected type で解決します。

```rust
pub enum LiteralType {
    UnsuffixedInt,
    UnsuffixedFloat,
    SuffixedInt(IntType),
    SuffixedFloat(FloatType),
}
```

このテストを先に足します。

```arcw
flow @flow.ok ok {
    let a: i32 = 1
    let b = 1i32
    let c: usize = 0usize
}

flow @flow.bad_no_expected bad_no_expected {
    let a = 1
}

flow @flow.bad_width bad_width {
    let a: i32 = 1u64
}
```

ここが通ると、型健全性検証の足場が一気に信頼できます。

### Phase 4: RuntimePlan type validator を追加する

`typecheck_hir -> lower_runtime_plan` の後に、独立した validator を走らせます。

```rust
pub fn validate_runtime_plan_types(
    plan: &RuntimePlan,
    types: &TypeCheckReport,
) -> RuntimeTypeValidationReport;
```

これは checker の再実装ではなく、**lowering 後の runtime IR が type witness と矛盾していないか**を見る pass です。

検査対象:

* `FlowOp::Let` の RHS type と binding type が一致する。
* `Choice` option value / route target / out value が期待型に合う。
* `Await` target が `Need<T, E>` 由来である。
* `RuntimeExpr` の binary/unary/call が `TypeJudgment` と対応する。
* `Never` は後続の通常値として使われない。
* `line` lifetime の値が runtime plan の line boundary を越えない。
* effect capability がない call-shaped effect が runtime lowering に混入しない。

これは形式証明ではありませんが、実装バグを非常に高確率で捕まえます。

### Phase 5: preservation / progress の executable self-check を作る

最初から論文風の完全証明を狙うより、arcweft では executable self-check が向いています。

追加する性質:

```text
Preservation:
  well-typed HIR を runtime plan に lower し、1 step 実行しても、
  runtime env / observations / pending ops は型 witness と矛盾しない。

Progress:
  well-typed headless subset は、
  Done / Blocked / Output / BudgetExhausted のいずれかに分類され、
  static type mismatch 由来の runtime diagnostic を出さない。
```

CLI は例えばこれです。

```bash
arcw verify-types game/routes/opening.arcw --mode test --json
arcw verify-types tests/fixtures/arcw/soundness --generated 1000 --json
```

生成テストは二系統にします。

1つ目は **well-typed generator**。型規則から HIR/AST を生成し、`parse -> lower -> typecheck -> runtime lower -> validate -> run` を通します。

2つ目は **negative mutator**。通る fixture に対して、型幅を変える、capability を消す、`'line` を thread に持ち込む、MustDrop の片枝 drop を消す、route parameter を欠落させる、といった破壊を入れます。これは必ず typecheck/semantic/verify のどこかで落ちるべきです。

失敗した generated case は shrink して、

```text
tests/fixtures/arcw/soundness/should_pass/generated_XXXX.arcw
tests/fixtures/arcw/soundness/should_fail/generated_XXXX.arcw
```

へ保存できるようにします。

### Phase 6: solver を “参考出力” から “検証結果” に昇格する

ここは後半でよいですが、必ず必要です。

まず `verify_module_with_env` を追加します。

```rust
pub fn verify_module_with_env(
    module: &HirModule,
    env: &TypeCheckEnv,
    policy: VerificationPolicy,
) -> VerificationReport;
```

既存の `verify_module` は `TypeCheckEnv::new()` を使う convenience wrapper にします。

次に `solve_report` を `VerificationReport` に反映します。

```rust
pub struct SolverCheck {
    pub obligation: String,
    pub backend: BackendKind,
    pub outcome: SmtOutcome,
    pub required: bool,
}
```

`test` / `release` mode では、required obligation が `Sat` または `Unknown` なら exit failure にします。今のように stderr に出すだけでは、CI 上の自己検証になりません。

最後に `ProofExpr` を設計文書に寄せます。少なくとも `Int`, `Le`, `Forall`, `Exists` は入れるべきです。設計文書側の Proof IR にはそれらがすでに書かれています。

---

## 最初に切る PR

私なら、次の順番で切ります。

1. **`RuntimeStepStats` + `arcw profile --json`**

   * `Engine::step` の `executed_ops` を `RuntimeStepResult.stats` に出す。
   * CLI の compile/check phases に elapsed timer を付ける。
   * core には wall-clock を入れない。

2. **`arcw bench` の headless measure 実行**

   * `--iterations / --warmup / --steps / --json`。
   * deterministic stats と elapsed summary を出す。
   * adapter-only section は skipped のまま。

3. **`TypeCheckReport` 導入**

   * `typecheck_hir` は互換 wrapper として残す。
   * `TypeJudgment` を出す。
   * まずは主要 expression / stmt / line-plan だけでよい。

4. **numeric primitive 精密化**

   * `TypeKind::Int/Float` を置き換える。
   * unsuffixed literal の expected-type 解決を入れる。
   * 仕様と一致させる。

5. **RuntimePlan type validator**

   * `lower_runtime_plan` 後に validator。
   * `arcw check` と `arcw verify-types` に接続。

6. **solver outcome を report/exit code に反映**

   * `verify_module_with_env`。
   * `SolverCheck`。
   * `test/release` mode で required unknown/sat を failure。

---

## 完了条件

この改善のゴールは、次を CI で回せる状態です。

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets --all-features

arcw check tests/fixtures/arcw/spec_should_pass/check
arcw check tests/fixtures/arcw/spec_should_fail

arcw profile tests/fixtures/arcw/spec_should_pass/run/opening.arcw --json
arcw bench --manifest arcw.toml --profile bench.opening --json

arcw verify-types tests/fixtures/arcw/soundness --mode test --json
arcw verify tests/fixtures/arcw/soundness --mode test --backend z3 --json
```

既存の implementation snapshot でも `cargo fmt/test/clippy` と pass/fail fixtures は重視されています。 そこに **profile/bench/soundness** を足す形が自然です。

最重要の方針はこれです。

**プロファイラは “時間を測る道具” ではなく、compiler/runtime/typechecker/verifier の各フェーズに stable な観測点を作ること。型健全性の自己検証は “solver を呼ぶこと” ではなく、型判断の証跡、runtime lowering の検査、実行保存性テスト、solver-backed obligations を同じ report に合流させること。**

この順で進めると、arcweft は「速くなる」だけでなく、「速く変えても壊れていないことを自分で説明できる」言語基盤になります。
