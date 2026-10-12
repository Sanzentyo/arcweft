# save / replay / hot reload

## SaveSnapshot

```rust
pub struct SaveSnapshot {
    pub bundle_hash: BundleHash,
    pub program_version: Version,
    pub state: Value,
    pub logical_time: LogicalTime,
    pub active_scene: SceneId,
    pub flow_fibers: Vec<FlowFiberSnapshot>,
    pub active_activities: Vec<ActivitySnapshot>,
    pub audio_state: AudioSnapshot,
    pub awaited_tasks: Vec<AwaitedTaskSnapshot>,
}
```

## 未完了 task

生 Future は保存しない。保存するのは task key と continuation。

```rust
pub struct AwaitedTaskSnapshot {
    pub task_id: TaskId,
    pub key: TaskKey,
    pub class: TaskClass,
    pub request: HostTaskRequest,
}
```

load 時は `ensure_task` で再登録。

Restartable dispatch の復元では、保存された完全な task correlation、logical epoch、
dispatch sequence と publication frontier を保持する。復元先には新しい native host
dispatch owner を用意し、再登録時に新しい `HostTaskAttempt` を発行する。
adapter と worker はその寿命証拠を返す。
保存前の worker packet は、公開座標と次の publication revision が一致していても
復元後の attempt と一致しないため、scheduler・frontier・統計を更新する前に拒否する。
稼働中の同一 dispatch の Join は既存 attempt を共有する。この host 内の証拠は保存・
digest・公開イベント順序には含めず、成功した publication は従来の決定的な座標を使う。

## ReplayTrace

```rust
pub struct ReplayTrace {
    pub engine_version: String,
    pub bundle_hash: BundleHash,
    pub initial_state: StateSnapshot,
    pub frames: Vec<RecordedFrameInput>,
    pub task_responses: Vec<RecordedTaskResponse>,
    pub audio_events: Vec<RecordedAudioEvent>,
    pub agent_actions: Vec<RecordedAgentAction>,
}
```

## Hot reload

```text
incoming patch
  → parse
  → typecheck
  → contract check
  → shader validate
  → wasm/rust ABI check
  → state compatibility check
  → dry-run current continuation
  → commit at frame boundary
```

## 保存形式の契約

Arcweft が所有する save・snapshot・codec の contract version は `1`。
未公開の内部形式は同じ version のまま更新し、旧形式の reader や migration 経路を
並置しない。host 内の worker 寿命証拠は保存形式に含めない。

retained View の root binding・mount parameter・local state は、選択された
runtime program の既存 Core value snapshot に投影して保存する。JSON decode
では live value を構築せず、artifact/generation の照合後に同じ program authority
で復元する。callable state と capture は復元先の immutable executable に束縛し、
nominal header はその型グラフで検証する。View の in-memory snapshot に独立の
serde reader を残さず、mount identity・型付き slot・axis seed の検証を既存 View
restore で実行してから session を公開する。

Product の queued `Ready` は、保持された task admission の完全な correlation から
outcome を選び、既存の `BoundTaskOutcome` で検証してから保存・復元結果を公開する。
program outcome は選択中の immutable executable に束縛し、standalone outcome は
既存の有限 predicate を使う。欠落・不一致・重複 admission は復元を拒否する。
この検証は inert save と直接の live snapshot restore の両方に適用する。

固定 byte-array atom の `TaskId` を JSON object key にしない。
`task_publications` の version `1` JSON は、task identity の昇順の
`[TaskId, TaskPublicationCursor]` row 配列として保存する。
decode は重複・順序逆転・旧 object grammar を拒否し、別の reader を残さない。

## Agent replay

Agent actions are replayed as semantic actions when possible.

```bash
arcw agent script replay traces/bug.arcwx --json
```
