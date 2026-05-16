# 契約プログラミング

契約は関数、flow、reducer、Activity、parser、shader、UI component、Rust extern に付けられる。

## requires / ensures

```awft
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

```awft
pub invariant @inv.affection_bounds(state: GameState) {
    forall c in CharacterId {
        0 <= state.affection[c] && state.affection[c] <= 100
    }
}
```

## reducer contract

```awft
pub reducer update(state: GameState, event: GameEvent) -> Result<Update<GameState>, GameError>
requires invariant @inv.affection_bounds(state)
ensures result.is_ok() => invariant @inv.affection_bounds(result.unwrap().state)
{
    ...
}
```

## modifies / effects

```awft
pub fn add_affection(character: Ref<Character>, delta: i32)(state: GameState) -> GameState
reads state.affection[character]
modifies state.affection[character]
{
    ...
}
```

Effect contract:

```awft
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError>
effects { asset.read, audio.play, ui.show }
ensures no_effect network.request
{
    ...
}
```

## decreases

```awft
fn count_reachable(flow: Ref<Flow>, visited: Set<Ref<Flow>>) -> usize
decreases graph.remaining_nodes(flow, visited)
{
    ...
}
```

## 契約モード

```awft
pub enum ContractMode {
    CheckRuntime,
    DebugCheck,
    Prove,
    Assume,
    GenerateTest,
    DocumentOnly,
}
```

```awft
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


