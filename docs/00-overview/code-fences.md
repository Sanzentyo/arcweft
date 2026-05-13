# Code Fence Policy

Arcweft の Markdown 仕様書では、Arcweft script の例示に Rust の code fence を使わない。

## 基本ルール

```text
Arcweft source / DSL:      ```awft
Arcweft agent script:      ```awfagent
WGSL shader body only:     ```wgsl
Rust implementation code:  ```rust
TOML config:               ```toml
JSON schema/example:       ```json
Shell command:             ```bash
Plain pipeline/diagram:    ```text
```

Arcweft の source extension は `.awft` なので、DSL 例は原則 `awft` fence を使う。

```awft
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    alice(id=#say.opening.001): おはよう。[p]
    Ok(FlowExit::Done)
}
```

Rust host / crate 実装例は `rust` のままにする。

```rust
pub struct FrameInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
}
```

WGSL 単体の shader body は `wgsl` を使う。ただし、`shader #shader... { wgsl { ... } }` のような Arcweft DSL 全体の例は `awft` を使う。

## 理由

- `.awft` source と Rust 実装を明確に分ける。
- LLM / RAG が Arcweft DSL と Rust API を誤認しにくくする。
- 将来の syntax highlighter / tree-sitter grammar / docs renderer が `awft` fence を手がかりにできるようにする。
