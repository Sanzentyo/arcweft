# Rust / WASM plugin

## 実行形態

| 形態 | 用途 | 長所 | 短所 |
|---|---|---|---|
| static Rust Activity | 公式ゲーム、Web対応 | 最速、型安全 | hot reloadしにくい |
| native dylib | 開発中hot reload、信頼済みmod | 高速 | ABI管理が必要 |
| out-of-process | 重いActivity、クラッシュ隔離 | 安全 | IPC設計が重い |
| WASM component | mod、sandbox | 配布しやすい | zero-copy制約 |

## Rust export

Rust exports are opt-in adapter metadata, not source introspection. An
Arcweft-aware Rust crate annotates exported functions and ADTs with
`arcweft-rust-abi-macros`; its build writes deterministic
`arcweft-rust-abi` JSON into Cargo output or another project-relative metadata
location. `arcweft-rust-abi-build` is the build-script helper crate for writing
that JSON and emitting Cargo rerun hints, while `arcweft-rust-abi` remains data
and codecs only and the proc macros remain the source of truth for signatures.

Arcweft source declares the imported Rust module shape, and a launch profile
selects the metadata file that makes those names visible to sema and LSP:

```arcw
extern rust mod mini_games::truck from crate "truck_game" {
    pub type TruckInput
    pub type TruckResult

    pub fn score_to_rank(score: i32) -> Rank

    pub activity truck_game: Activity<TruckInput, TruckResult>
    requires input.seed != 0
    ensures result.score >= 0
}
```

Semantic checking compares structured `extern rust mod` members against the
selected package metadata. A direct source check, or a profile that omits the
matching `rust_metadata` file, rejects the extern declaration instead of
creating dynamic fallback bindings.

Non-Arcweft-aware Rust crates are exposed through a small annotated wrapper
crate. Raw pointers, unsafe ABIs, non-static borrows, and unsupported generic
exports are rejected by the metadata macro rather than accepted as dynamic
fallbacks.

## WASM plugin

WASM is a plugin/activity sandbox format, not Arcweft's primary script runtime.
Arcweft scripts lower to Typed IR / bytecode and run on the VM. A native player may
use `arcweft-wasm-wasmtime` for sandboxed plugin calls; a browser player uses its
own Wasm player build and browser APIs, not Wasmtime.

The plugin ABI is described with WIT in `arcweft-wasm-abi`. Validation,
component generation, and inspection live in `arcweft-wasm-tools`; host execution
adapters stay outside `arcweft-core`.

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

