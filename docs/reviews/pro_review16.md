結論としては、**現状の実装だけを見る限り、Rust のメモリ安全上の data race はほぼ起きません**。理由は、まだ OS thread / Tokio / Rayon 的な実行器がなく、`thread` も現状は VM/Runtime データ上の子タスク表現で、実スレッド生成ではないためです。`arcweft-core` も副作用を直接実行せず、`LineEffectRequest` などの要求データを返す Sans I/O 境界として設計されています。

ただし、**将来 runtime/executor を入れた瞬間に logical race は起こり得る**状態です。特に line-plan の `thread` / `on .mark` / timed cue / `together` / `await` を本当に並列・非同期実行するなら、現状の IR と checker では競合検出・順序決定・スコープ分離が足りません。

## いま安全なところ

現状の Rust 実装は、workspace dependency もかなり軽く、root では `unsafe_code = "forbid"` が入っています。 `arcweft-cli` も `arcw check` で parse → HIR → typecheck → line task group lowering を順番に実行しているだけで、並列実行はしていません。

`LineTaskGroup` も「実行」ではなく、`init`、`children`、`defer_stack`、`SignalWrite`、`MetricWrite`、`RegisterHandle` などのデータモデルです。 実装 README でも、full VM execution / `FrameInput` / `FrameOutput` / source backpressure / hook memo runtime / save replay などはまだ deferred とされています。

なので、現時点では「共有メモリを複数 thread から同時に mutate して壊れる」という種類の data race は見当たりません。

## 問題点 1: line/task スコープの checker 状態が漏れる可能性がある

これは現状の `arcw check` にも影響し得る一番具体的な問題です。

`check_line_plan_item` では `LinePlanItem::Thread` が `check_thread_body` に入り、`LinePlanItem::On` は body の `Stmt` をそのまま `check_stmt` しています。`check_thread_body` は `available_lifetimes` だけを退避・復元していて、`locals`、`lifetime_guarantees`、`dropped_lifetime_keys` は退避していません。 一方で `check_let_stmt` は `self.locals.insert(...)` でローカルを追加します。

つまり、例えば conceptual にはこういう誤判定が起き得ます。

```awft
with {
    thread bg {
        let x = 1
    }

    let y = x  // 本来は thread 内ローカルなので見えてはいけない
}
```

同様に、`on .mark { let x = ... }` の中の `x` も、親 line plan の後続 item から見えてしまう可能性があります。これは data race そのものではありませんが、**並列タスクの private state が親スコープに漏れる checker unsoundness** です。

対策は、`thread` / `on` / timed cue / `together` / child task body を読む時に、少なくとも次を snapshot/restore することです。

```rust
locals
active_presentation_defaults
lifetime_guarantees
dropped_lifetime_keys
active_borrows
available_lifetimes
```

`with_child_task_scope(|checker| ...)` のような helper を作るのが良いです。

## 問題点 2: `'line.*` の保証・drop 状態が line を跨いで漏れる可能性がある

`check_dialogue_item` では、`focus` option があると `'line.focus` を `lifetime_guarantees` に入れ、line plan 中だけ `available_lifetimes.push(Line)` しています。 しかし `lifetime_guarantees` と `dropped_lifetime_keys` は line 終了時に snapshot/restore されていません。

一方、`check_lifetime_path_expr` は `lifetime_guarantees` を見て「optional でなくても安全に読めるか」を判断し、`dropped_lifetime_keys` を見て「すでに drop 済みか」を判断します。

そのため、同じ flow 内で前の dialogue line が `'line.foo <- ...` した場合、次の dialogue line でも `'line.foo` が guaranteed と誤判定される可能性があります。逆に前の line で `'line.focus |> drop` したら、次の line の `'line.focus` が「already dropped」と誤判定される可能性もあります。

`'line` は line scope の lifetime なので、line ごとに状態を閉じるべきです。`check_dialogue_item` の plan 評価を次のような形にするのが安全です。

```rust
fn with_line_runtime_scope(&mut self, f: impl FnOnce(&mut Self)) {
    let old_guarantees = self.lifetime_guarantees.clone();
    let old_dropped = self.dropped_lifetime_keys.clone();
    let old_locals = self.locals.clone();
    let old_defaults = self.active_presentation_defaults.clone();

    self.available_lifetimes.push(LifetimeScopeKind::Line);
    f(self);
    self.available_lifetimes.pop();

    self.lifetime_guarantees = old_guarantees;
    self.dropped_lifetime_keys = old_dropped;
    self.locals = old_locals;
    self.active_presentation_defaults = old_defaults;
}
```

## 問題点 3: `together` / `start` の並列境界が lowering で消えている

AST には `LinePlanItem::StartGroup(Vec<LinePlanItem>)` と `TogetherGroup(Vec<LinePlanItem>)` があり、構文上はグループ境界を保持しています。 しかし `runtime_plan.rs` の lowering では、`StartGroup` と `TogetherGroup` を同じ扱いで中身を単に再帰 lower しています。

これは将来 executor を作る時に危険です。`together` が「同時に開始」「並列実行」「同期点あり」のどれを意味するかに関係なく、現状の `LineTaskGroup` からはその区別を復元できません。

必要なのは、`LineEffectRequest` の flat list ではなく、少なくとも次のような構造です。

```rust
enum LineTaskNode {
    Seq(Vec<LineTaskNode>),
    Parallel {
        policy: ParallelPolicy,
        children: Vec<LineTaskNode>,
    },
    Child(LineChildTask),
    Effect(LineEffectRequest),
}
```

そして `Parallel` の中では write set の衝突を検査するべきです。

## 問題点 4: 子タスクの deterministic order が足りない

設計 docs では、非同期 task event は `logical_epoch`, `task_id`, `sequence` で sort して完了順を正規化する想定になっています。 しかし現状の `LineChildTask` は `name: Option<String>`, `body`, `defer_stack` だけで、stable `TaskId`、logical epoch、sequence、priority、start condition、join/cancel policy がありません。

このままだと、例えば次のようなケースで順序が曖昧になります。

```awft
with {
    on .seen {
        'flow.flags.seen <- true
    }

    thread analytics {
        'flow.flags.seen <- false
    }
}
```

現状 checker は上位 lifetime 書き込みに `state.write(scope)` capability が必要かは見ます。 ただし、「同じ key に複数 child task が同 frame/tick で書く」ことの conflict policy はまだありません。実装 README でも deterministic concurrent write conflict checking は TODO と明記されています。

ここは P0/P1 で、`LineEffectRequest` に read/write resource key と conflict policy を持たせた方がいいです。

```rust
enum ConflictPolicy {
    Error,
    LastWriterWins { priority: i32 },
    MergePatch,
    Reduce { op: ReduceOp },
    AppendEvent,
}
```

`SignalWrite`, `MetricWrite`, `'flow.* <-`, presentation slot write, audio handle write などを全部同じ access-set で扱うと、静的検査しやすくなります。

## 問題点 5: `thread` の capture safety が浅い

checker は active borrow が `await` / `thread` / `defer` / `yield` を跨ぐのを拒否しています。`await` では `reject_active_borrows("await suspension boundary")` が呼ばれ、`Stmt::Thread` や `Expr::Thread` でも thread boundary のチェックがあります。 さらに `check_thread_body` では `'line` / `'cue` を thread 内で利用不可にするため `available_lifetimes` を filter しています。

これは良い方向ですが、まだ不足しています。

特に未実装/不足しているのは、thread への capture set です。

```text
- move capture
- shared capture
- unique handle capture
- detached thread restriction
- parent lifetime
- MustDrop handle capture
- join result type
- child task cancellation during scope exit
```

`Expr::Thread` は AST と checker にはありますが、runtime lowering 側では `lower_expr_effect` が `Expr::Thread` を effect として扱っていません。 つまり「式として thread を作って await/join する」方向は、構文/checker と runtime IR の間にまだギャップがあります。

## 問題点 6: `Need<T,E>` は値状態であって、scheduler handle ではない

`Need<T,E>` の実装は `NotStarted / Pending / Ready / Err / Cancelled` の単純な enum で、`map` / `map_err` を持つ値モデルです。 これは Sans I/O の状態表現としては良いですが、実際の async task 管理には足りません。

設計 docs 側では `TaskHost::ensure_task`, `cancel_scope`, `poll_frame`、同一 key の task join、single-thread/multi-thread の worker model が想定されています。 その実装がないので、現状の `Need` は Future ではなく「結果状態」です。

入れるなら、core は引き続き Future を直接 poll せず、こう分けるのが良いです。

```rust
// core/VM側
NeedId
TaskKey
AwaitTarget
CancelScopeId

// host adapter側
TaskHost::ensure_task(...)
TaskHost::poll_frame(...) -> Vec<TaskEvent>

// frame boundary
FrameInput { task_events, ... }
FrameOutput { effect_requests, ... }
```

こうしておけば、native multi-thread / web single-thread / replay で同じ意味論を保てます。

## 問題点 7: stream/backpressure は設計済みだが実装側 IR がない

docs では `Need` は startup/permission、`Stream` は ordered live data、`Watch` は latest value と分け、stream には `BackpressurePolicy` を持たせる設計になっています。 ただし実装 README では source/stream backpressure IR は deferred です。

camera / mic / USB / HID を入れると、ここは race というより queue overflow / stale frame / callback lifetime の問題になります。`yield` が suspension boundary であることは docs に書かれていますが、実装ではまだ full stream runtime ではありません。

## 優先度つき修正案

**P0: checker のスコープ漏れを直す。**
`thread` / `on` / timed cue / line scope / block scope で `locals` と lifetime state を snapshot/restore する。これは現在の `arcw check` の誤判定に直結します。

**P0: line lifetime state を line ごとに閉じる。**
`'line.*` の guarantee/drop state は dialogue line 終了時に必ず破棄する。`'flow` 以上だけを親へ残す場合も、明示 capability と deterministic update event 経由にする。

**P1: `LineTaskGroup` を flat effect list から task graph にする。**
`Seq`, `Parallel`, `Child`, `Wait`, `Effect`, `Cleanup` を分け、`together` の意味を IR に残す。

**P1: effect access set と conflict checker を入れる。**
`state.write('flow)`, `signal.set`, `metric.set`, presentation slot, audio handle, lifetime registry key に対して read/write/drop/append を付ける。同じ parallel region 内で同じ key に非可換 write があれば error。

**P1: task id / event ordering を入れる。**
docs の `logical_epoch, task_id, sequence` sort を実装 IR に反映する。`LineChildTask` に stable id、priority、start condition、join/cancel policy を持たせる。

**P2: async scheduler / Need / stream runtime を実装する。**
`Need` は値状態のままでもよいが、`TaskKey`, `NeedId`, `TaskEvent`, `CancelScopeId` を導入し、host adapter の completion を frame boundary で正規化する。stream は `BackpressurePolicy` を必須にする。

## まとめ

いまの実装は、実スレッドや async runtime がないので **Rust の data race は起きにくい**です。ただし、将来 line-plan child task を本当に並列・非同期実行するなら、現状のままだと **同一 state key / signal / metric / presentation slot への logical race** が起きます。

特に今すぐ直した方がいいのは、`thread` / `on` / line lifetime の checker scope leak です。ここを放置すると、runtime を実装する前から「型検査では通るが実行時には存在しない変数・line key を参照する」ケースが出ます。
