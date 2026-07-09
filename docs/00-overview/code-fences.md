# Code Fence Policy

Arcweft の Markdown 仕様書では、Arcweft script の例示に Rust の code fence を使わない。

## 基本ルール

```text
Arcweft source / DSL:      ```arcw
Arcweft agent script:      ```awfagent
WGSL shader body only:     ```wgsl
Rust implementation code:  ```rust
TOML config:               ```toml
JSON schema/example:       ```json
Shell command:             ```bash
Plain pipeline/diagram:    ```text
```

Arcweft の source extension は `.arcw` なので、DSL 例は原則 `arcw` fence を使う。
`arcw` / `awfagent` fence と `.arcw` / `.awfagent` source では、通常コメントは
`//`、doc comment は `///` / `//!` を使う。`#` は `#[...]` / `#![...]`
attribute と dialogue interpolation の一部であり、comment introducer として
使わない。

```arcw
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    // Arcweft comments use `//`, not `#`.
    alice(id=@say.opening.001): おはよう。[p]
    Ok(FlowExit::Done)
}
```

Rust host / crate 実装例は `rust` のままにする。

```rust
pub struct RuntimeStepInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
}
```

WGSL 単体の shader body は `wgsl` を使う。ただし、`shader @shader... { wgsl { ... } }` のような Arcweft DSL 全体の例は `arcw` を使う。

## 理由

- `.arcw` source と Rust 実装を明確に分ける。
- LLM / RAG が Arcweft DSL と Rust API を誤認しにくくする。
- 将来の syntax highlighter / tree-sitter grammar / docs renderer が `arcw` fence を手がかりにできるようにする。

## Validation

コメント構文の受理・拒否は parser の挙動テストで検証する。文書や sample の
表記はレビューと formatter で整えるものとし、リポジトリ内の文字列を走査する
source gate は正しさの根拠にしない。

