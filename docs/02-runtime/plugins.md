# Rust / WASM plugin

## 実行形態

| 形態 | 用途 | 長所 | 短所 |
|---|---|---|---|
| static Rust Activity | 公式ゲーム、Web対応 | 最速、型安全 | hot reloadしにくい |
| native dylib | 開発中hot reload、信頼済みmod | 高速 | ABI管理が必要 |
| out-of-process | 重いActivity、クラッシュ隔離 | 安全 | IPC設計が重い |
| WASM component | mod、sandbox | 配布しやすい | zero-copy制約 |

## Rust export

```awft
extern rust mod mini_games::truck from crate "truck_game" {
    pub type TruckInput
    pub type TruckResult

    pub fn score_to_rank(score: i32) -> Rank

    pub activity truck_game: Activity<TruckInput, TruckResult>
    requires input.seed != 0
    ensures result.score >= 0
}
```

## WASM plugin

```rust
wasm plugin affection_ai from "plugins/affection_ai.wasm" {
    abi = "wit:arcweft:plugin/affection@0.1.0"
    sandbox {
        memory = 8MiB
        fuel_per_call = 2_000_000
        wasi = false
        network = false
        filesystem = false
    }
    import fn score(state: GameState, event: ChoiceEvent) -> i32
}
```

## Security

- filesystem/network は deny by default。
- host import whitelist。
- fuel / memory / call time limit。
- WASI preopen なしがデフォルト。
- Activity から直接 engine state を mutate しない。

