# Runtime Hooks and Memoization

この章は [Hooks and Memoization](../01-language/hooks-and-memoization.md) の runtime 実装方針である。

関連:

- [Sans I/O core](core.md)
- [Layered Input](layered-input.md)
- [Need / scheduler](async-scheduler.md)
- [save / replay](save-replay.md)
- [Agent Debug Bus](../04-tooling/agent-debug-mcp-cli.md)

## Hook runtime overview

```text
Compile:
  DSL hook item
    → Hook HIR
    → type/effect/contract check
    → HookTable
    → RuntimeHookPlan

Runtime:
  RuntimeStepInput + LayerTree + Signals + TaskEvents
    → HookScheduler.collect(phase)
    → stable order
    → execute hook bodies as small fibers
    → emit Events/Commands/Signals/Logs/InputDisposition
```

`arcweft-core` は OS callback を直接受け取らない。hook は `Engine::step` 内の phase point、または host 側の `InputRouter` / `RenderOwner` / `SignalBus` から明示的に起動される。

Dialogue line-local markers are not runtime hooks. `[mark .name]` creates a
line timeline marker, and `with: on mark(.name):` lowers to a handler inside the
line task group. Top-level `hook @hook...` remains the mechanism for engine
phase hooks described in this document.

---

## HookTable

```rust
pub struct HookTable {
    pub by_phase: BTreeMap<HookPhase, Vec<HookId>>,
    pub hooks: IndexMap<HookId, HookRecord>,
    pub subscriptions: HookSubscriptions,
    pub dependency_index: HookDependencyIndex,
}

pub struct HookRecord {
    pub id: HookId,
    pub entity: EntityId,
    pub target: HookTarget,
    pub phase: HookPhase,
    pub check_policy: CheckPolicy,
    pub priority: i32,
    pub once: Option<OncePolicy>,
    pub debounce: Option<LogicalDuration>,
    pub throttle: Option<LogicalDuration>,
    pub allowed_effects: EffectSet,
    pub body: HookBodyId,
    pub source: SourceAnchor,
}
```

---

## HookContext and outputs

```rust
pub struct HookContext<'a> {
    pub tick: TickId,
    pub logical_time: LogicalTime,
    pub state: &'a GameStateValue,
    pub layer_tree: Option<&'a LayerTree>,
    pub input_event: Option<&'a RoutedInputEvent>,
    pub signal_event: Option<&'a SignalEvent>,
    pub task_event: Option<&'a TaskEvent>,
    pub target: Option<EntityId>,
    pub env: HookEnvironment,
}

pub struct HookPhaseOutput {
    pub events: Vec<GameEvent>,
    pub commands: Vec<Command>,
    pub signals: Vec<SignalWrite>,
    pub logs: Vec<LogFrame>,
    pub input_disposition: Option<InputDisposition>,
    pub memo_ops: Vec<MemoOp>,
    pub diagnostics: Vec<Diagnostic>,
}
```

hook は `state` を直接変更しない。変更は Event / Command / Update 経由。

---

## Determinism

```text
- phase は固定
- hook order は stable sort
- when は pure
- logical time のみ使用
- wall-clock を直接読まない
- random は seeded capability 経由
- hook が発行した event/command は stable order で Engine に入る
```

Replay trace:

```rust
pub struct RecordedHookFire {
    pub tick: TickId,
    pub phase: HookPhase,
    pub hook: HookId,
    pub target: Option<EntityId>,
    pub condition_result: bool,
    pub output_hash: Hash,
}
```

---

## Reentrancy and cycle guards

```rust
pub struct HookCycleGuard {
    pub max_depth_per_tick: u32,
    pub max_fires_per_hook_per_tick: u32,
}
```

デフォルトは `max_depth_per_tick = 8`、`max_fires_per_hook_per_tick = 1`。repeatable hook だけ例外を許す。

---

## MemoRuntime

```rust
pub struct MemoRuntime {
    pub stores: EnumMap<MemoScopeKind, Box<dyn MemoStore>>,
    pub dependency_graph: MemoDependencyGraph,
    pub stats: MemoStats,
}

pub struct MemoKey {
    pub function: EntityId,
    pub function_semantic_hash: Hash,
    pub type_layout_hash: Hash,
    pub args_hash: Hash,
    pub env_hash: Hash,
    pub dependency_hash: Hash,
}
```

`MemoValue` は schema/version を持つ type-erased bytes。

---

## Scope mapping

```text
Frame:      frame end で破棄
Tick:       tick 単位
Scene:      scene scope 終了で破棄
Flow:       flow fiber 終了で破棄
Session:    game session 中保持
Bundle:     bundle hash が変わるまで保持
Persistent: disk cache
Lease:      memory lease lifetime に従う
```

---

## Invalidation pipeline

```text
StatePath changed
  → MemoDependencyGraph lookup
  → invalidate affected keys
  → emit MemoInvalidated signal/log

Signal changed
  → subscribed memo deps invalidated

Asset/Shader/Typeset source changed
  → content hash changed
  → cache key miss or explicit invalidation

Hot reload patch committed
  → function semantic hash changed
  → all keys for function invalidated
```

---

## Memo + Need

```rust
pub enum NeedCacheState<T, E> {
    Missing,
    Running(TaskId),
    Ready(T),
    Failed(E),
    Cancelled,
}
```

`TaskKey` は実行中の合流用、`MemoKey` は完了結果の再利用用。

---

## Agent integration

```text
arcweft.hook_list
arcweft.hook_trace
arcweft.hook_enable
arcweft.hook_disable
arcweft.memo_stats
arcweft.memo_inspect
arcweft.memo_invalidate
```

製品版では hook disable / memo invalidate は debug capability が必要。

---

## Runtime diagnostics

```text
HOOK_EFFECT_FORBIDDEN
HOOK_REENTRANCY_LIMIT
HOOK_CONDITION_IMPURE
MEMO_IMPURE_FUNCTION
MEMO_SCOPE_LIFETIME_ESCAPE
MEMO_MISSING_DEPENDENCY
```
