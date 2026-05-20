# Arcweft 依存関係・全体構成・`mod` 分割・ライフタイム設計レビュー

作成日: 2026-05-19  
対象リポジトリ: `Sanzentyo/arcweft` `main`  
目的: ファイル依存・crate 依存・責務集中を確認し、`mod` 分割候補と Rust/Arcweft ライフタイムで重点的に詰めるべき箇所を、実装手順つきで整理する。

---

## 1. 結論

現状の Cargo workspace は、概ね Arcweft の設計方針どおりに層が分かれている。特に `arcweft-core` が `id` / `need` / `source` 程度にしか依存していない点、`syntax -> hir -> sema -> runtime-plan/verify -> tooling/cli` という方向が大きく崩れていない点は良い。

一方で、Rust ファイル単位では責務がかなり集中している。今すぐ優先して分割すべきファイルは以下。

| 優先度 | ファイル | 問題 | 推奨対応 |
|---:|---|---|---|
| 1 | `crates/arcweft-core/src/lib.rs` | Runtime IR、Engine、式評価、pattern matching、source/stream、task、line task が単一ファイルに同居 | `time`, `frame`, `value`, `pattern`, `plan`, `engine`, `task`, `source`, `stream`, `line_task`, `effect`, `observation` に分割 |
| 2 | `crates/arcweft-lang-sema/src/check.rs` | 型、診断、環境、checker 状態、borrow/lifetime、stmt/expr/flow/line/source/choice の検査が同居 | `types`, `env`, `diagnostics`, `checker`, `borrow`, `lifetime`, `expr`, `stmt`, `flow`, `line_plan`, `choice`, `source`, `effects` に分割 |
| 3 | `crates/arcweft-lang-syntax/src/ast.rs` | すべての AST family が単一ファイル | `ast/common.rs`, `ast/ids.rs`, `ast/items.rs`, `ast/flow.rs`, `ast/dialogue.rs`, `ast/choice.rs`, `ast/line_plan.rs`, `ast/source.rs`, `ast/proof.rs` へ分割 |
| 4 | `crates/arcweft-lang-syntax/src/parser.rs` | top-level dispatch から flow/dialogue/choice/source/line-plan parser まで同居 | `parser.rs` を facade/driver にし、`parser/top_level.rs`, `parser/flow.rs`, `parser/dialogue.rs`, `parser/choice.rs`, `parser/line_plan.rs`, `parser/source.rs`, `parser/recovery.rs`, `parser/helpers.rs` へ分割 |
| 5 | `crates/arcweft-lang-hir/src/lower.rs` | HIR model 定義と lowering 実装が同居 | `model.rs`, `lower.rs`, `lower_flow.rs`, `lower_dialogue.rs`, `lower_choice.rs`, `lower_ids.rs` へ分割 |
| 6 | `crates/arcweft-runtime-plan/src/lib.rs` | runtime lowering 全般が単一ファイル | `line_task.rs`, `flow.rs`, `expr.rs`, `pattern.rs`, `source.rs`, `stream.rs`, `errors.rs` へ分割 |

ライフタイムについては、**AST/HIR/RuntimePlan に Rust の lifetime parameter を広く入れるべきではない**。Arcweft の中間表現はデータ形式・診断・hot reload・serialize/replay に使うので、`String` / `Vec` を持つ owned model のままが安全。

ただし、以下は重点的に詰めるべき。

1. `arcweft-lang-sema` の Arcweft lifetime registry / borrow escape 検査を `lifetime.rs` と `borrow.rs` に独立させる。
2. `await` / `yield` / `thread` / `defer` / line plan child task の suspension boundary で borrow が跨がないことをテストで固定する。
3. runtime adapter 境界には `RuntimeStepInputRef<'a>` / `ActivitySnapshotRef<'a>` のような view/ref 型を導入する。ただし core data model 本体は owned のままにする。

---

## 2. 現在の依存関係

### 2.1 Workspace 全体

root `Cargo.toml` の workspace member は以下の層に整理できる。

```text
foundation / data
  arcweft-source
  arcweft-id
  arcweft-need
  arcweft-adt
  arcweft-memory
  arcweft-ref

core runtime data
  arcweft-core

application facade
  arcweft

language pipeline
  arcweft-lang-syntax
  arcweft-lang-hir
  arcweft-lang-sema
  arcweft-runtime-plan

verification / tooling
  arcweft-verify
  arcweft-verify-oxiz
  arcweft-verify-z3
  arcweft-verify-lsp
  arcweft-tooling
  arcweft-test

entrypoint
  arcweft-cli
```

望ましい依存方向は次。

```text
source/id/need/ref/adt/memory
        │
        ▼
core                      syntax
        │                   │
        └──── runtime-plan ◀ hir ◀ syntax
                            │
                            ▼
                          sema
                            │
                            ▼
                         verify ── verify-oxiz / verify-z3
                            │
                         verify-lsp

cli は上位集約なので多くの crate に依存してよい。
arcweft は application-facing facade なので broad prelude としてよい。
```

### 2.2 現状で良い点

- `arcweft-core` は `arcweft-id`, `arcweft-need`, `arcweft-source`, `thiserror` にしか依存していない。設計上の Sans I/O core として妥当。
- `arcweft-lang-syntax` は parser/syntax-only に近く、`arcweft-source`, `rowan`, `blake3`, `thiserror` だけに依存している。
- `arcweft-lang-hir` は `syntax` を受けて HIR に落とす層として自然。
- `arcweft-verify-oxiz` / `arcweft-verify-z3` は `arcweft-verify` の adapter になっており、solver 依存が verify 本体に漏れていない。
- `arcweft-cli` は上位 entrypoint なので、`fs`, `path`, `process`, `serde_json`, verifier backend などを集約してよい。

### 2.3 注意したい依存

#### `arcweft-dialogue -> arcweft-presentation`

`arcweft-dialogue` が `arcweft-presentation` に依存している。Dialogue が「表示対象や TextBox の純データモデル」まで含むなら許容できるが、低レイヤーの dialogue model を presentation に引きずらせたくないなら再検討したい。

候補:

```text
arcweft-dialogue
  -> arcweft-id
  -> arcweft-source
  -> arcweft-presentation-model  # 新設する場合

arcweft-presentation
  -> arcweft-id
  -> arcweft-presentation-model
```

`TextBoxRef`, `SpeakerRef`, `VoiceRef` などが純 ID wrapper で済むなら `arcweft-id` / `arcweft-ref` に寄せてもよい。

#### `arcweft-runtime-plan -> arcweft-lang-syntax`

`arcweft-runtime-plan` は `arcweft-lang-hir` と `arcweft-lang-syntax` の両方に依存している。現在の HIR は syntax type を多く re-export しているため自然だが、将来的には runtime lowering が parser internals に触れているように見える。

短期対応:

- 直接 `arcweft_lang_syntax::...` を使っている import を一覧化する。
- `arcweft-lang-hir` が re-export している型は `arcweft_lang_hir::...` 経由に寄せる。
- `TypeRef`, `Expr`, `Pattern`, `Stmt`, `LinePlan` などが HIR の正式 surface model として必要なら、HIR crate 側に明示的に re-export し続ける。

長期対応:

- HIR 用の独自 `HirExpr`, `HirPattern`, `HirStmt` を段階的に増やし、runtime-plan は syntax crate を直接見ないようにする。

#### `arcweft-lang-sema -> arcweft-lang-syntax`

`sema` も `syntax` 型を直接多く使っている。Phase 1 の HIR が syntax に近いので現状は許容。ただし設計方針上は `syntax -> hir -> sema` を明確にしたい。

短期対応:

- `sema/check.rs` の import を `arcweft_lang_hir::{...}` に寄せる。
- `sema` が直接 syntax を参照する場合は「HIR が re-export していないため」など理由を残す。

長期対応:

- HIR が type checker に必要な shape をすべて持つ。
- `validate_typecheck_ready` の RawExpr 排除を前提に、sema 側から raw syntax fallback を減らす。

#### `arcweft-test` の重複 dependency

`arcweft-test/Cargo.toml` では `arcweft-lang-hir` が `[dependencies]` と `[dev-dependencies]` の両方にある。不要なら dev-dependency 側を削除する。

---

## 3. `arcweft-core/src/lib.rs` 分割案

### 3.1 現状の問題

`core/src/lib.rs` は、次の責務を単一ファイルで持っている。

- logical time: `TickId`, `LogicalDuration`, `LogicalTime`
- frame data: `RuntimeStepInput`, `RuntimeStepOutput`, `RuntimeDiagnostic`
- runtime value/expression/pattern: `RuntimeValue`, `RuntimeExpr`, `RuntimePattern`
- runtime plan: `RuntimePlan`, `RuntimeFlow`, `FlowOp`, `StreamPlan`, `SourcePlan`
- engine state: `Engine`, `FlowFiber`, `FlowFiberStatus`, `FlowCursor`
- source/stream queues and backpressure
- task/Need event normalization
- line task graph and cleanup
- expression evaluator
- pattern matcher
- flow opcode dispatcher
- line task runner
- helper labels and control conversion

このまま機能追加すると、`lib.rs` が実質的な architectural boundary になってしまう。Arcweft の方針である「private modules + explicit `pub use`」に寄せるべき。

### 3.2 推奨ファイル構成

```text
crates/arcweft-core/src/
  lib.rs
  time.rs
  frame.rs
  value.rs
  pattern.rs
  effect.rs
  task.rs
  source.rs
  stream.rs
  plan.rs
  observation.rs
  line_task.rs
  engine.rs
  engine/flow.rs
  engine/source.rs
  engine/stream.rs
  engine/suspend.rs
  engine/eval.rs
  tests.rs
```

`mod.rs` ではなく、既存方針どおり `engine.rs` + `engine/*.rs` を使う。

### 3.3 `lib.rs` の形

```rust
mod effect;
mod engine;
mod frame;
mod line_task;
mod observation;
mod pattern;
mod plan;
mod source;
mod stream;
mod task;
mod time;
mod value;

pub use effect::{
    LineEffectRequest, RuntimeAssignment, RuntimeCall, RuntimeCommand, RuntimeEvent,
    RuntimeField, RuntimeLog,
};
pub use engine::{Engine, FlowCursor, FlowExit, FlowFiber, FlowFiberStatus};
pub use frame::{AudioEvent, RuntimeStepInput, RuntimeStepOutput, InputEvent, RuntimeDiagnostic, UiEvent};
pub use line_task::{
    AudioCleanup, ChildCancelPolicy, ChildJoinPolicy, ChildTaskCleanup, LineAssertionRequest,
    LineBindingRequest, LineCancelRuleRequest, LineChildTask, LineCleanupPolicy,
    LineMemoRequest, LineOptionRequest, LineOutRequest, LineTaskGroup, LineTaskNode,
    LineTaskScope, LineTaskTrigger, ParallelPolicy, PresentationCleanup, ScopeExit,
    run_line_task_group,
};
pub use observation::RuntimeObservationState;
pub use pattern::{RuntimePattern, RuntimeRecordPatternField};
pub use plan::{
    ChoiceRuntimeOption, FlowEvent, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeMatchArm,
    RuntimePlan, RuntimePlanError, RuntimeLineId,
};
pub use source::{
    BackpressurePolicy, OverflowPolicy, PrivacyPolicy, ReplayPolicy, SourceEvent,
    SourceEventKind, SourceHandlerPlan, SourceId, SourceOp, SourcePlan, SourcePolicy,
    SourceRuntimeState, normalize_source_events,
};
pub use stream::{
    StreamEvent, StreamMatchArm, StreamOp, StreamPlan, StreamRuntimeId, StreamRuntimeState,
};
pub use task::{
    AwaitTarget, CancelScopeId, LogicalEpoch, NeedId, SchedulerBudget, TaskClass, TaskEvent,
    TaskEventKind, TaskHandle, TaskHost, TaskId, TaskKey, TaskPolicy, TaskPriority,
    TaskSequence, TaskSource, TaskSpec, normalize_task_events,
};
pub use time::{LogicalDuration, LogicalTime, TickId};
pub use value::{
    RuntimeBinaryOp, RuntimeEvalError, RuntimeExpr, RuntimeExprMatchArm, RuntimeFieldExpr,
    RuntimeFieldValue, RuntimeUnaryOp, RuntimeValue,
};

#[cfg(test)]
mod tests;
```

### 3.4 移動単位

#### Step 1: `time.rs`

移動対象:

- `TickId`
- `LogicalDuration`
- `LogicalTime`
- `impl LogicalDuration`
- `impl Default for LogicalDuration`
- `impl LogicalTime`

`LogicalDuration` は `RuntimeStepInput`, `LineTaskTrigger`, `RuntimeValue::Duration` で使うため public re-export する。

#### Step 2: `frame.rs`

移動対象:

- `RuntimeStepInput`
- `RuntimeStepOutput`
- `RuntimeDiagnostic`
- `RuntimeBinding` は `value.rs` でもよいが、frame input から使われるため `value.rs` に置くのが自然。
- `InputEvent`, `UiEvent`, `AudioEvent`
- `impl RuntimeStepOutput::merge`

`RuntimeStepOutput::merge` は engine/line_task から使うだけなら `pub(crate)` にする。

```rust
impl RuntimeStepOutput {
    pub(crate) fn merge(&mut self, other: Self) { ... }
}
```

#### Step 3: `value.rs`

移動対象:

- `RuntimeBinding`
- `RuntimeValue`
- `RuntimeFieldValue`
- `RuntimeExpr`
- `RuntimeExprMatchArm`
- `RuntimeFieldExpr`
- `RuntimeUnaryOp`
- `RuntimeBinaryOp`
- `RuntimeEvalError`
- `evaluate_unary`
- `evaluate_binary`
- `unsupported_binary`
- `runtime_unary_op_label`
- `runtime_binary_op_label`
- `expr_runtime_label`
- `runtime_value_label`

`evaluate_expr` は `Engine` の env に依存するので `engine/eval.rs` に置く。`runtime_value_label` は複数 module で使うので `pub(crate)` にする。

#### Step 4: `pattern.rs`

移動対象:

- `RuntimePattern`
- `RuntimeRecordPatternField`
- `match_runtime_pattern`
- `reject_duplicate_bindings`
- `collect_pattern_bindings`
- `collect_pattern_list`

`match_runtime_pattern` は `engine/eval.rs`, `engine/stream.rs`, `engine/source.rs` から使うため `pub(crate)`。

#### Step 5: `effect.rs`

移動対象:

- `LineEffectRequest`
- `RuntimeCall`
- `RuntimeLog`
- `RuntimeAssignment`
- `RuntimeEvent`
- `RuntimeCommand`
- `RuntimeField`
- `ResourceAccess`, `ResourceAccessMode`, `ConflictPolicy`, `ReduceOp`
- `FlowControl`, `control_from_effect`

`FlowControl` と `control_from_effect` は engine 内だけでもよい。より自然には `engine/flow.rs` に置く。

#### Step 6: `task.rs`

移動対象:

- `TaskId`, `TaskKey`, `NeedId`, `CancelScopeId`
- `LogicalEpoch`, `TaskSequence`, `TaskPriority`
- `AwaitTarget`
- `TaskSpec`, `TaskHandle`, `SchedulerBudget`
- `TaskClass`, `TaskPolicy`, `TaskSource`
- `TaskEvent`, `TaskEventKind`
- `TaskHost`
- `normalize_task_events`
- `await_task_spec`

`await_task_spec` は engine 内 helper なので `engine/suspend.rs` に置いてもよい。

#### Step 7: `source.rs` / `stream.rs`

`source.rs`:

- `SourceId`
- `SourcePolicy`, `BackpressurePolicy`, `OverflowPolicy`, `ReplayPolicy`, `PrivacyPolicy`
- `SourceEvent`, `SourceEventKind`
- `SourcePlan`, `SourceHandlerPlan`, `SourceOp`
- `SourceRuntimeState`
- `normalize_source_events`

`stream.rs`:

- `StreamRuntimeId`
- `StreamPlan`, `StreamOp`, `StreamMatchArm`
- `StreamEvent`
- `StreamRuntimeState`

#### Step 8: `plan.rs`

移動対象:

- `RuntimePlan`
- `RuntimeFlow`
- `FlowOp`
- `FlowRuntimeId`
- `RuntimeLineId`
- `RuntimeMatchArm`
- `RuntimeMatchSelection`
- `ChoiceRuntimeOption`
- `FlowEvent`
- `RuntimePlanError`
- `impl RuntimePlan`
- `impl From<&str> for FlowRuntimeId`
- `impl From<&str> for RuntimeLineId`

`RuntimeMatchSelection` は engine の内部だけなら `engine/flow.rs` に置く。

#### Step 9: `line_task.rs`

移動対象:

- `ScopeExit`
- `LineTaskGroup`
- `LineTaskScope`
- `LineTaskNode`
- `ParallelPolicy`
- `LineChildTask`
- `LineTaskTrigger`
- `ChildJoinPolicy`
- `ChildCancelPolicy`
- `LineOptionRequest`
- `LineBindingRequest`
- `LineOutRequest`
- `LineCancelRuleRequest`
- `LineMemoRequest`
- `LineAssertionRequest`
- `LineCleanupPolicy`
- `ChildTaskCleanup`, `PresentationCleanup`, `AudioCleanup`
- `run_line_task_group`
- `run_line_task_group_for_input`
- `input_matches_trigger`
- `run_scope`, `run_scope_cleanup`, `run_node`, `run_child_task`
- `trigger_is_ready`, `task_spec`, `outcome_defer_stack`, `flatten_defer_stack`

`run_line_task_group_for_input` は engine だけで使うなら `pub(crate)`。

#### Step 10: `engine.rs` と `engine/*.rs`

`engine.rs`:

- `Engine`
- `FlowFiber`
- `RuntimeObservationState` への参照
- `FlowControlStackEntry`, `FlowControlStackEntryKind`
- `FlowCursor`
- `FlowFiberStatus`
- `AwaitState`, `ChoiceState`, `FlowExit`
- `Engine::new`, `Engine::step`, `fiber`, `record_observations`

`engine/flow.rs`:

- `step_flow`
- `bind_value`
- `advance_if_needed`
- `push_ops`
- scope/loop helpers
- break/continue helpers
- `goto`, `return_value`, `finish`, `apply_control_effects`

`engine/eval.rs`:

- `evaluate_expr`
- `evaluate_bool`
- `evaluate_entity_target`
- `try_bind_pattern`
- `evaluate_let`
- `evaluate_if_let`
- `evaluate_match`
- `evaluate_if_let_expr`
- `evaluate_match_expr`
- `fail_eval`
- `diagnose_runtime_error`

`engine/source.rs`:

- `apply_source_events`
- `dispatch_source_event`
- `apply_unhandled_source_event`
- `record_source_event_state`
- `execute_source_ops`
- `execute_source_op`
- `push_source_item`
- `close_source`
- `source_handler_match`

`engine/stream.rs`:

- `step_stream_plans`
- `execute_stream_ops`
- `execute_stream_op`
- `bind_stream_let`
- `execute_stream_for_next`
- `yield_stream_item`
- `execute_stream_match`
- `close_stream_source`
- `evaluate_queue_target`
- `pop_queue_item`

`engine/suspend.rs`:

- `resume_suspended`
- `resume_await_state`
- `resume_choice_state`
- `input_selects_choice`
- `await_task_spec`

### 3.5 分割時の注意

- まず `pub use` で既存 public API を保つ。既存テストが `use arcweft_core::*` でも壊れないようにする。
- 移動直後は visibility を広めにし、後から `pub(crate)` に絞る。
- `RuntimeValue`, `RuntimeExpr`, `FlowOp` のような central type は cyclic import を生みやすい。先に `value.rs`, `pattern.rs`, `plan.rs` を固める。
- `engine` は他 module の型を使うだけにし、他 module が `Engine` に依存しないようにする。
- `LineTaskGroup` 実行は `Engine` から独立した pure runner として残す。

### 3.6 確認コマンド

```bash
cargo check -p arcweft-core
cargo test -p arcweft-core
cargo clippy -p arcweft-core --all-targets --all-features
```

---

## 4. `arcweft-lang-sema/src/check.rs` 分割案

### 4.1 現状の問題

`check.rs` は以下を全部持っている。

- `EntityKind`, `TypeKind`, `MapKind`, `HandleState`
- `TypeCheckEnv`, method registry
- `TypeCheckError`, readiness error
- `TypeChecker` 本体
- borrow state
- lifetime registry state
- loop/line/yield/effect state
- HIR module traversal
- flow item checker
- statement checker
- expression checker
- source checker
- choice checker
- line plan checker
- presentation call checker
- helper関数群

Arcweft の「lifetime で頑張るべき部分」はすでにここに集まり始めているため、ここは最優先で責務別に分けるべき。

### 4.2 推奨ファイル構成

```text
crates/arcweft-lang-sema/src/
  lib.rs
  check.rs
  diagnostics.rs
  env.rs
  types.rs
  checker.rs
  borrow.rs
  lifetime.rs
  flow.rs
  stmt.rs
  expr.rs
  line_plan.rs
  choice.rs
  source.rs
  effects.rs
  symbols.rs
  resolve.rs
  semantic/
    facts.rs
```

### 4.3 `check.rs` の役割

`check.rs` は public API facade にする。

```rust
mod borrow;
mod checker;
mod choice;
mod diagnostics;
mod effects;
mod env;
mod expr;
mod flow;
mod lifetime;
mod line_plan;
mod source;
mod stmt;
mod types;

pub use diagnostics::{TypeCheckError, TypeCheckReadinessError};
pub use env::TypeCheckEnv;
pub use types::{EntityKind, HandleState, MapKind, TypeKind};

pub fn validate_typecheck_ready(module: &HirModule) -> Result<(), Vec<TypeCheckReadinessError>> {
    checker::validate_typecheck_ready(module)
}

pub fn typecheck_hir(module: &HirModule, env: &TypeCheckEnv) -> Result<(), Vec<TypeCheckError>> {
    checker::typecheck_hir(module, env)
}
```

### 4.4 `checker.rs`

`TypeChecker` は crate 内部型にする。

```rust
pub(crate) struct TypeChecker<'a> {
    pub(crate) env: &'a TypeCheckEnv,
    pub(crate) errors: Vec<TypeCheckError>,
    pub(crate) borrows: BorrowState,
    pub(crate) locals: HashMap<String, TypeKind>,
    pub(crate) loop_stack: Vec<LoopContext>,
    pub(crate) line_label_stack: Vec<Option<String>>,
    pub(crate) line_cancel_depth: usize,
    pub(crate) active_presentation_defaults: HashMap<String, String>,
    pub(crate) line_mark_stack: Vec<HashSet<String>>,
    pub(crate) lifetimes: LifetimeRegistryState,
    pub(crate) effect_capabilities: HashSet<String>,
    pub(crate) yield_stack: Vec<YieldContext>,
}
```

最初の PR では field を完全に state object 化せず、既存 field のまま module 分割だけしてもよい。次の PR で `BorrowState` / `LifetimeRegistryState` に寄せる。

### 4.5 `borrow.rs`

移動対象:

- `BorrowLocalState`
- `BorrowStateSnapshot`
- `merge_borrow_local_states`
- `register_borrow_bindings`
- `release_borrow_local`
- `clear_borrow_local`
- `remove_active_borrow_lifetime`
- `snapshot_borrow_state`
- `restore_borrow_state`
- `merge_borrow_state_from_paths`
- `rebuild_active_borrows`
- `reject_active_borrows`
- `reject_borrow_escape`
- `release_direct_drop_expr`

推奨 state object:

```rust
#[derive(Clone, Debug, Default)]
pub(crate) struct BorrowState {
    active: Vec<String>,
    locals: HashMap<String, BorrowLocalState>,
}

#[derive(Clone, Debug)]
pub(crate) struct BorrowStateSnapshot {
    active: Vec<String>,
    locals: HashMap<String, BorrowLocalState>,
}

impl BorrowState {
    pub(crate) fn snapshot(&self) -> BorrowStateSnapshot { ... }
    pub(crate) fn restore(&mut self, snapshot: BorrowStateSnapshot) { ... }
    pub(crate) fn reject_active(&self, errors: &mut Vec<TypeCheckError>, boundary: &str) { ... }
    pub(crate) fn reject_escape(
        ty: Option<&TypeKind>,
        errors: &mut Vec<TypeCheckError>,
        destination: &str,
    ) { ... }
}
```

### 4.6 `lifetime.rs`

移動対象:

- `TypeCheckerScopeSnapshot`
- `snapshot_runtime_scope`
- `restore_runtime_scope`
- `with_line_runtime_scope`
- `with_child_task_scope`
- `check_lifetime_set_stmt`
- `check_lifetime_path_expr`
- `check_lifetime_pipe`
- `drop_lifetime_key`
- `check_lifetime_access`
- `lifetime_available`
- helper: `lifetime_key`, `lifetime_value_type`, `type_contains_borrow_ref`, `collect_type_kind_lifetimes`

推奨 state object:

```rust
#[derive(Clone, Debug, Default)]
pub(crate) struct LifetimeRegistryState {
    guarantees: HashSet<LifetimeKey>,
    dropped: HashSet<LifetimeKey>,
    available: Vec<LifetimeScopeKind>,
}
```

`Line` / `Cue` が line runtime 内でだけ available になるルールをここに閉じ込める。

```rust
impl LifetimeRegistryState {
    pub(crate) fn is_available(&self, scope: &LifetimeScopeKind) -> bool {
        !matches!(scope, LifetimeScopeKind::Line | LifetimeScopeKind::Cue)
            || self.available.contains(scope)
    }
}
```

### 4.7 suspension boundary の検査を強める

現状も `await`, `yield`, `thread`, `defer` で active borrow を拒否している。これは良い方向なので、module 分割後にテストを増やす。

追加テスト案:

```rust
#[test]
fn borrow_cannot_cross_await_boundary() { ... }

#[test]
fn borrow_cannot_cross_yield_boundary() { ... }

#[test]
fn borrow_cannot_cross_thread_boundary() { ... }

#[test]
fn borrow_can_be_dropped_before_await() { ... }

#[test]
fn maybe_dropped_borrow_reports_control_flow_error() { ... }
```

### 4.8 lifetime registry のテストを追加する

追加テスト案:

```rust
#[test]
fn line_lifetime_is_unavailable_outside_line_scope() { ... }

#[test]
fn line_lifetime_read_requires_guarantee_or_optional_access() { ... }

#[test]
fn writing_global_lifetime_requires_state_write_capability() { ... }

#[test]
fn borrowed_value_cannot_be_written_to_upper_lifetime_registry() { ... }

#[test]
fn dropped_lifetime_key_cannot_be_read_again() { ... }

#[test]
fn lifetime_key_double_drop_is_diagnostic() { ... }
```

### 4.9 確認コマンド

```bash
cargo check -p arcweft-lang-sema
cargo test -p arcweft-lang-sema
cargo clippy -p arcweft-lang-sema --all-targets --all-features
```

---

## 5. `arcweft-lang-syntax` 分割案

### 5.1 `lib.rs` は現状よい

`crates/arcweft-lang-syntax/src/lib.rs` は module 宣言と re-export の facade として機能している。ここは大きな問題ではない。

### 5.2 `ast.rs` 分割

推奨構成:

```text
crates/arcweft-lang-syntax/src/
  ast.rs
  ast/common.rs
  ast/ids.rs
  ast/items.rs
  ast/flow.rs
  ast/functions.rs
  ast/dialogue.rs
  ast/choice.rs
  ast/line_plan.rs
  ast/source.rs
  ast/proof.rs
  ast/tests.rs
```

`ast.rs` は re-export のみに寄せる。

```rust
mod common;
mod choice;
mod dialogue;
mod flow;
mod functions;
mod ids;
mod items;
mod line_plan;
mod proof;
mod source;
mod tests;

pub use common::{DocBlock, TextRange, Visibility};
pub use ids::{EntityRef, EntityRefSyntax, FamilyRelativeEntityRef, IdRef, RelativeId};
pub use items::{Attribute, Item, ModuleDecl, RawItem, RawSyntax, RawSyntaxFamily, UseItem, UseMode};
pub use flow::{Flow, FlowInit, FlowItem, FlowKind, ScopeBlock, ScopeExprBlock};
...
```

注意点:

- Constructor や accessor が `impl` として散らばっている場合、型と同じ file に移動する。
- `pub(crate) struct FlowInit` のような parser/lower 用 initializer は、関連 type と同じ module に置く。
- `TextRange` は全 family が使うので `ast/common.rs`。
- `EntityRef`, `IdRef`, `RelativeId` は language 全体に跨るので `ast/ids.rs`。

### 5.3 `parser.rs` 分割

推奨構成:

```text
crates/arcweft-lang-syntax/src/
  parser.rs
  parser/top_level.rs
  parser/items.rs
  parser/flow.rs
  parser/dialogue.rs
  parser/choice.rs
  parser/line_plan.rs
  parser/source.rs
  parser/proof.rs
  parser/recovery.rs
  parser/helpers.rs
```

`parser.rs` には public API と `Parser` struct を残す。

```rust
mod choice;
mod dialogue;
mod flow;
mod helpers;
mod items;
mod line_plan;
mod proof;
mod recovery;
mod source;
mod top_level;

pub fn parse_source(source: impl Into<String>) -> ParsedSource { ... }

pub(crate) struct Parser { ... }
```

`impl Parser` は複数 module に分散できる。

```rust
// parser/top_level.rs
impl Parser {
    pub(crate) fn parse_top_level_line(...) { ... }
    pub(crate) fn validate_module_path(...) -> bool { ... }
    pub(crate) fn validate_use_tree(...) -> bool { ... }
}
```

Rust では `src/parser.rs` が `parser` module の root になり、`src/parser/top_level.rs` を submodule として読めるので `mod.rs` は不要。

### 5.4 分割順序

1. `parser/recovery.rs`: `ParseError`, `RecoverySuggestion`, error push helpers。
2. `parser/helpers.rs`: string split / block extraction helper のうち parser 固有のもの。
3. `parser/top_level.rs`: `parse_top_level_line`, `parse_top_level_item`, module/use validation。
4. `parser/flow.rs`: flow-like body parser。
5. `parser/dialogue.rs`, `parser/line_plan.rs`, `parser/choice.rs`。
6. `parser/source.rs`, `parser/proof.rs`。

確認:

```bash
cargo check -p arcweft-lang-syntax
cargo test -p arcweft-lang-syntax
```

---

## 6. `arcweft-lang-hir` 分割案

### 6.1 現状の問題

`lower.rs` は HIR model type と lowering implementation が同居している。型定義を読むために lowering 処理全体を読む必要がある。

### 6.2 推奨構成

```text
crates/arcweft-lang-hir/src/
  lib.rs
  model.rs
  lower.rs
  lower_context.rs
  lower_flow.rs
  lower_dialogue.rs
  lower_choice.rs
  lower_ids.rs
  id_context.rs
```

`model.rs`:

- `HirModule`
- `HirFlow`
- `HirFunction`
- `HirTopLevelDecl`
- `HirFlowItem`
- `HirDialogue`
- `HirChoice`
- `HirChoiceOption`
- `HirSourceLocale`
- `HirScope`
- `HirScopeExpr`
- `HirIf`, `HirIfLet`, `HirMatch`, `HirLoop`, `HirFor`, `HirWhile`, `HirWhileLet`, `HirSelect`, `HirBorrow`, `HirAwait`

`lower.rs`:

- `HirLowerError`
- `lower_to_hir`
- lowering driver

`lower_context.rs`:

- `LowerContext`
- relative ID / scope stack / line counter 周り

`lower_flow.rs`:

- flow body lowering
- `FlowItem -> HirFlowItem`

`lower_dialogue.rs`:

- speaker/dialogue normalization
- relative line ID materialization

`lower_choice.rs`:

- choice option normalization

`lower_ids.rs`:

- entity/id/relative ID materialization helper

### 6.3 実装方針

- 最初は型移動だけを行い、lowering の挙動は変えない。
- `pub use model::{...};` を `lib.rs` または `lower.rs` から提供し、既存 import を壊さない。
- `LowerContext` は private のままでよい。

確認:

```bash
cargo check -p arcweft-lang-hir
cargo test -p arcweft-lang-hir
```

---

## 7. `arcweft-runtime-plan/src/lib.rs` 分割案

### 7.1 現状の問題

runtime-plan は、HIR から core runtime data へ変換する重要な境界。現状は line task lowering、flow runtime lowering、source/stream lowering、expr/pattern lowering、error type が単一ファイルにある。

### 7.2 推奨構成

```text
crates/arcweft-runtime-plan/src/
  lib.rs
  errors.rs
  line_task.rs
  flow.rs
  expr.rs
  pattern.rs
  source.rs
  stream.rs
  labels.rs
```

`lib.rs`:

```rust
mod errors;
mod expr;
mod flow;
mod labels;
mod line_task;
mod pattern;
mod source;
mod stream;

pub use errors::{LinePlanLowerError, RuntimePlanLowerError};
pub use flow::lower_runtime_plan;
pub use line_task::{LoweredLineTaskGroup, lower_line_task_groups};
```

`expr.rs`:

- `lower_runtime_expr`
- unary/binary op mapping
- literal mapping

`pattern.rs`:

- `lower_runtime_pattern`
- record/list/variant pattern mapping

`source.rs`:

- `lower_source_plan`
- `lower_source_handler`
- `lower_source_policy`
- `lower_source_stmt`

`stream.rs`:

- `lower_stream_function`
- `lower_stream_stmt_list`
- `stream_type_labels`

`line_task.rs`:

- `LoweredLineTaskGroup`
- line plan lowering
- line task group generation

`flow.rs`:

- `FlowRuntimeLowerer`
- `lower_runtime_plan`
- `HirFlowItem -> FlowOp`

### 7.3 依存縮小

`runtime-plan` は currently `arcweft_lang_syntax` を直接 import している。分割後、各 module で syntax 型が本当に必要かを明確にする。

最初の目標:

```rust
use arcweft_lang_hir::{Expr, Pattern, Stmt, TypeRef, ...};
```

のように HIR re-export 経由に寄せる。

---

## 8. CLI / facade の扱い

### 8.1 `arcweft` facade

`crates/arcweft/src/lib.rs` は application-facing prelude として機能している。これは低レイヤーではないため、広い re-export は許容できる。

ただし、facade が肥大化した場合は以下のように分けるとよい。

```rust
pub mod prelude {
    pub use crate::core::*;
    pub use crate::dialogue::*;
    pub use crate::presentation::*;
}

pub mod core {
    pub use arcweft_core::*;
    pub use arcweft_id::{...};
    pub use arcweft_need::{...};
}

pub mod dialogue {
    pub use arcweft_dialogue::{...};
}

pub mod presentation {
    pub use arcweft_presentation::{...};
}
```

### 8.2 `arcweft-cli/src/main.rs`

CLI は `check`, `verify`, `unsafe`, `plan`, `run`, `test`, `bench`, `fmt`, `ids` を単一 `main.rs` で処理している。今後 CLI コマンドが増えるなら分割する。

推奨構成:

```text
crates/arcweft-cli/src/
  main.rs
  output.rs
  args.rs
  commands/check.rs
  commands/verify.rs
  commands/unsafe.rs
  commands/plan.rs
  commands/run.rs
  commands/test.rs
  commands/bench.rs
  commands/fmt.rs
  commands/ids.rs
  load.rs
  json.rs
```

ただし、最優先は `core` と `sema`。CLI 分割は後回しでよい。

---

## 9. ライフタイム設計: 頑張るべきところ / 頑張らない方がよいところ

### 9.1 頑張らない方がよいところ

#### AST / HIR に `'src` を広く入れない

`Expr<'src>`, `HirModule<'src>` のように source string を借用する設計は、短期的には allocation を減らせるが、Arcweft には以下の用途がある。

- diagnostics
- formatter
- hot reload
- LSP
- verifier
- runtime-plan lowering
- replay/debug trace
- 将来的な serialize / cache

これらは `source` の寿命と中間表現の寿命を分離した方が扱いやすい。現状の `String` / `Vec` 所有モデルを維持する。

#### `Engine` 内部を self-referential にしない

`RuntimePlan` 内の `FlowOp` を借用して `Engine<'plan>` にする案は、clone を減らせるが、self-referential / long-lived borrow に近くなる。Phase 1 では避ける。

代替:

- `FlowRuntimeId` -> index の map を持つ。
- `FlowCursor` は `flow_index` + `op_index` にする。
- 必要なら `FlowProgramIndex` を owned data として持つ。

```rust
pub struct RuntimePlanIndex {
    flow_by_id: BTreeMap<FlowRuntimeId, usize>,
}

pub struct FlowCursor {
    pub flow_index: usize,
    pub op_index: usize,
}
```

これは Rust lifetime を増やさずに lookup と clone を減らせる。

### 9.2 頑張るべきところ

#### Arcweft DSL の lifetime registry 検査

現在の `sema/check.rs` には、以下の重要な検査がある。

- lifetime registry key の read/write/drop
- `Line` / `Cue` scope の availability
- optional access の扱い
- guarantee の有無
- drop 済み key の再読込拒否
- upper lifetime registry への borrowed value escape 拒否
- non-line lifetime write に `state.write(scope)` capability を要求

この領域は Arcweft 独自性が強く、`lifetime.rs` として独立させ、仕様とテストを集中させるべき。

#### Borrow escape / suspension boundary

`await`, `yield`, `thread`, `defer`, line child task は suspension boundary。borrowed values がここを跨ぐと、実行タイミングと所有者が分かれ、replay 不能・未定義的な仕様になりやすい。

方針:

- borrow が active なまま `await` できない。
- borrow が active なまま `yield` できない。
- borrow が active なまま `thread` に入れない。
- borrow が active なまま `defer` / cleanup boundary に入れない。
- line/cue lifetime は line runtime scope 内だけ available。
- child task には line/cue borrow を暗黙 capture させない。

#### Runtime adapter の view 型

core data は owned のままにしつつ、adapter API には lifetime view を導入してよい。

```rust
pub struct RuntimeStepInputRef<'a> {
    pub tick: TickId,
    pub dt: LogicalDuration,
    pub input_events: &'a [InputEvent],
    pub task_events: &'a [TaskEvent],
    pub ui_events: &'a [UiEvent],
    pub audio_events: &'a [AudioEvent],
}

pub struct RuntimeStepOutputSink<'a> {
    output: &'a mut RuntimeStepOutput,
}
```

設計文書上の `Activity` も `MountContext<'_>`, `RuntimeStepInputRef<'_>`, `RuntimeStepOutputSink<'_>`, `ActivitySnapshotRef<'_>` のような境界を想定しているため、ここは Rust lifetime を使う価値がある。

---

## 10. 推奨 PR 順序

### PR 1: `arcweft-core` leaf module 分割

対象:

- `time.rs`
- `frame.rs`
- `value.rs`
- `pattern.rs`
- `effect.rs`
- `task.rs`

目的:

- public API を保ったまま leaf 型を移す。
- `cargo check -p arcweft-core` を通す。

### PR 2: `arcweft-core` runtime module 分割

対象:

- `source.rs`
- `stream.rs`
- `plan.rs`
- `line_task.rs`
- `observation.rs`
- `engine.rs` / `engine/*.rs`

目的:

- `Engine` と runtime plan の依存を整理する。
- `step_flow` はいったん `engine/flow.rs` に移すだけでよい。細分化は次。

### PR 3: `arcweft-core` tests 整理

対象:

```text
crates/arcweft-core/src/tests.rs
crates/arcweft-core/src/tests/source.rs
crates/arcweft-core/src/tests/stream.rs
crates/arcweft-core/src/tests/flow.rs
crates/arcweft-core/src/tests/line_task.rs
```

目的:

- 分割後の責務ごとにテストも分ける。

### PR 4: `arcweft-lang-sema` diagnostics/types/env 分割

対象:

- `diagnostics.rs`
- `types.rs`
- `env.rs`
- `checker.rs`

目的:

- `check.rs` を public facade にする。
- `TypeChecker` を crate-private にする。

### PR 5: `arcweft-lang-sema` borrow/lifetime 分割

対象:

- `borrow.rs`
- `lifetime.rs`

目的:

- Arcweft lifetime registry と borrow escape ルールを独立させる。
- suspension boundary tests を追加する。

### PR 6: `arcweft-lang-sema` feature checker 分割

対象:

- `expr.rs`
- `stmt.rs`
- `flow.rs`
- `line_plan.rs`
- `choice.rs`
- `source.rs`
- `effects.rs`

目的:

- `check.rs` の肥大化を解消する。
- 各 language family ごとのテストを追加する。

### PR 7: `syntax` AST/parser 分割

対象:

- `ast/*`
- `parser/*`

目的:

- parser の family ごとの責務を分ける。
- grammar docs と対応しやすくする。

### PR 8: `hir` / `runtime-plan` 分割

対象:

- `arcweft-lang-hir/src/model.rs`
- `arcweft-lang-hir/src/lower_*.rs`
- `arcweft-runtime-plan/src/*.rs`

目的:

- HIR model と lowering implementation を分ける。
- runtime-plan の direct syntax dependency を縮小する。

### PR 9: dependency cleanup

対象:

- `arcweft-test/Cargo.toml` の重複 dependency
- `arcweft-runtime-plan` の direct syntax dependency 確認
- `arcweft-dialogue -> arcweft-presentation` の妥当性判断
- `arcweft-core` の空 feature flag の扱いを文書化または整理

---

## 11. 具体的な修正チェックリスト

### `arcweft-core`

- [ ] `src/time.rs` を作成し、time 型を移動。
- [ ] `src/frame.rs` を作成し、frame/event/diagnostic 型を移動。
- [ ] `src/value.rs` を作成し、runtime value/expr/operator/error を移動。
- [ ] `runtime_value_label` を `pub(crate)` にする。
- [ ] `src/pattern.rs` を作成し、runtime pattern/matcher を移動。
- [ ] `match_runtime_pattern` を `pub(crate)` にする。
- [ ] `src/effect.rs` を作成し、effect request と runtime request 型を移動。
- [ ] `src/task.rs` を作成し、task/need/cancel/scheduler 型を移動。
- [ ] `src/source.rs` を作成し、source policy/event/state を移動。
- [ ] `src/stream.rs` を作成し、stream plan/op/state を移動。
- [ ] `src/plan.rs` を作成し、runtime plan/flow/op/event を移動。
- [ ] `src/line_task.rs` を作成し、line task graph/runner を移動。
- [ ] `src/observation.rs` を作成し、runtime observation state を移動。
- [ ] `src/engine.rs` と `src/engine/*.rs` を作成し、Engine 実装を移動。
- [ ] `src/lib.rs` を module declaration + explicit `pub use` にする。
- [ ] `cargo test -p arcweft-core` を通す。

### `arcweft-lang-sema`

- [ ] `src/check.rs` を facade 化する。
- [ ] `src/types.rs` に `EntityKind`, `TypeKind`, `MapKind`, `HandleState` を移す。
- [ ] `src/env.rs` に `TypeCheckEnv`, `MethodSignature` を移す。
- [ ] `src/diagnostics.rs` に error 型を移す。
- [ ] `src/checker.rs` に `TypeChecker`, public check entrypoint を移す。
- [ ] `src/borrow.rs` に borrow state と related methods を移す。
- [ ] `src/lifetime.rs` に lifetime registry 検査を移す。
- [ ] `src/expr.rs`, `stmt.rs`, `flow.rs`, `line_plan.rs`, `choice.rs`, `source.rs`, `effects.rs` に checker impl を分散。
- [ ] lifetime/borrow tests を追加。
- [ ] `cargo test -p arcweft-lang-sema` を通す。

### `arcweft-lang-syntax`

- [ ] `ast.rs` を facade にする。
- [ ] `ast/common.rs`, `ast/ids.rs`, `ast/items.rs` を先に作る。
- [ ] `ast/flow.rs`, `ast/dialogue.rs`, `ast/choice.rs`, `ast/line_plan.rs`, `ast/source.rs` を作る。
- [ ] `parser.rs` に `Parser` と `parse_source` を残す。
- [ ] `parser/top_level.rs` に top-level dispatch を移す。
- [ ] `parser/flow.rs`, `parser/dialogue.rs`, `parser/choice.rs`, `parser/line_plan.rs`, `parser/source.rs` に family parser を移す。
- [ ] `cargo test -p arcweft-lang-syntax` を通す。

### `arcweft-lang-hir`

- [ ] `model.rs` に HIR type を移す。
- [ ] `lower_context.rs` に `LowerContext` を移す。
- [ ] `lower_flow.rs`, `lower_dialogue.rs`, `lower_choice.rs`, `lower_ids.rs` を作る。
- [ ] `lower.rs` は public lowering API と driver にする。
- [ ] `cargo test -p arcweft-lang-hir` を通す。

### `arcweft-runtime-plan`

- [ ] `errors.rs` を作る。
- [ ] `expr.rs` / `pattern.rs` を作る。
- [ ] `source.rs` / `stream.rs` を作る。
- [ ] `line_task.rs` / `flow.rs` を作る。
- [ ] direct `arcweft_lang_syntax` import を HIR re-export 経由にできるものから置換。
- [ ] `cargo test -p arcweft-runtime-plan` を通す。

---

## 12. 実行すべき検証コマンド

分割 PR ごと:

```bash
cargo fmt
cargo check -p <changed-crate>
cargo test -p <changed-crate>
cargo clippy -p <changed-crate> --all-targets --all-features
```

まとめて実行できる段階:

```bash
cargo fmt
cargo test --workspace
cargo clippy --workspace --all-targets --all-features
```

---

## 13. 残り TODO

- 実際の `cargo check` / `cargo test` は、このレビュー作成時点ではローカル checkout 上で実行していない。GitHub 上のファイル構成・Cargo manifest・主要 source の読解に基づく設計レビューである。
- `arcweft-dialogue -> arcweft-presentation` は、型の中身をさらに見て「意図した依存」か「presentation model 抽出が必要」かを判断する。
- `runtime-plan -> syntax` と `sema -> syntax` は、HIR の成熟度に合わせて段階的に縮小する。今すぐ無理に消すより、まず `core` / `sema` のファイル責務を分ける方が安全。
- `Engine` の clone 削減は、module 分割後に `RuntimePlanIndex` を追加してから検討する。Rust lifetime で参照を張り巡らせるのは後回し。

---

## 14. 最終推奨

最初に着手するなら、`arcweft-core/src/lib.rs` の分割を行う。理由は、core が Sans I/O architecture の中心であり、ここが monolith のままだと runtime-plan、test、CLI、future Activity adapter すべてが影響を受けるため。

次に `arcweft-lang-sema/src/check.rs` を分割し、`borrow.rs` と `lifetime.rs` を独立させる。Arcweft の「安全な runtime / line / cue / task lifetime」の仕様はここに集約されるべきであり、今後の verifier と unsafe lifetime audit の土台になる。
