# 契約プログラミング

契約は通常の関数、flow、Activity、shader、View、Rust extern に付けられる。
entry から reducer や Agent controller に選択される関数も、専用の宣言
family ではなく通常の関数契約を使う。

## requires / ensures

```arcw
pub fn add_affection(character: Ref<Character>, delta: i32)(state: GameState) -> GameState
requires delta >= -100 && delta <= 100
ensures result.affection[character] >= 0
ensures result.affection[character] <= 100
ensures result.affection[character] ==
    (old(state.affection[character]) + delta).clamp(0, 100)
{
    state.update(.affection[character], |v| (v + delta).clamp(0, 100))
}
```

- `result`: 戻り値。
- `old(expr)`: 呼び出し前の値。

## invariant

```arcw
pub invariant @inv.affection_bounds(state: GameState) {
    forall c in CharacterId {
        0 <= state.affection[c] && state.affection[c] <= 100
    }
}
```

## entry-bound reducer contract

```arcw
pub fn update(state: &GameState, event: GameEvent)
    -> Result<Reduction<GameState>, ReducerError>
effects {}
requires invariant @inv.affection_bounds(state)
ensures result.is_ok() => invariant @inv.affection_bounds(result.unwrap().state)
{
    ...
}
```

この関数を root reducer として使う entry は `reducer = update` を明示する。

## modifies / effects

```arcw
pub fn add_affection(character: Ref<Character>, delta: i32)(state: GameState) -> GameState
reads state.affection[character]
modifies state.affection[character]
{
    ...
}
```

Effect contract:

```arcw
pub flow opening(state: GameState) -> Result<FlowExit, FlowError>
effects { asset.read, audio.play, view.show }
ensures no_effect network.request
{
    ...
}
```

## decreases

```arcw
fn count_reachable(flow: Ref<Flow>, visited: OrderedSet<Ref<Flow>>) -> usize
decreases graph.remaining_nodes(flow, visited)
{
    ...
}
```

## 契約モード

```arcw
pub enum ContractMode {
    CheckRuntime,
    DebugCheck,
    Prove,
    Assume,
    GenerateTest,
    DocumentOnly,
}
```

```arcw
requires prove state.seed != 0
ensures check result.score >= 0
ensures debug result.debug_trace.len() < 1024
assume external_plugin_is_deterministic
```

## 検証 backend

- Runtime check
- SMT: Z3 / OxiZ
- Kani harness for Rust functions
- Creusot / Verus bridge for Rust proof-oriented code
- Property test generation
- LLM counterexample explanation


