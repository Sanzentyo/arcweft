# 実装ガイド

この章は、実際に Rust workspace を起こすときの推奨順序と、各 crate の実装粒度をまとめる。

## 1. 最小コンパイラ

### `arcweft-lang-syntax`

- `logos` 等で lexer。
- `rowan` 互換の lossless CST。
- コメント、空白、ID、`[[...]]` link、`@...` entity reference、`#[...]` attribute を保持。
- recovery parser を優先し、LSP で壊れたファイルも扱う。

実装順:

```text
TokenKind
  → GreenNodeBuilder
  → CST node accessor
  → source map
  → parser tests
```

### `arcweft-lang-hir`

- AST から semantic item を抽出。
- module summary を作る。
- `EntityId` と `PublicId` を解決。
- `use` / `pub` / `lazy use` を解決。
- 型推論と effect check の前段を作る。

### `arcweft-id` / `arcweft-ref`

- `entities.toml` registry を読み書き。
- ID 自動推論。
- rename plan。
- alias / deprecated alias。
- reference level / intent / policy。

## 2. 型システム

### 型

```text
Primitive: (), bool, String, explicit integer and float widths, Duration, Color, Ratio, Length, Angle
ADT: struct, enum, newtype
Generic: List<T>, Map<K,V>, Set<T>, Option<T>, Result<T,E>, Need<T,E>, Ref<T>
Function: A -> B, curried function groups
Borrow: &'a T, &'a mut T
```

### 推論

- unification table は `ena` 系で実装。
- region/lifetime は初期は小さな自前 solver。
- `await` / `yield` / `thread` を suspension boundary としてマーク。
- borrow が boundary を跨ぐ場合は compile error。

## 3. Core runtime

### `arcweft-core`

- `Engine::step` を完成させる。
- State / FlowFiber / Reducer / View を VM 上で動かす。
- `FrameInput` / `FrameOutput` を JSON debug dump できるようにする。

### `arcweft-lang-vm`

- Typed IR から bytecode。
- pure function、reducer、flow continuation を実行。
- fuel / recursion depth / allocation budget。

## 4. Need / Task / Scheduler

### `arcweft-need`

- `Need<T,E>` の型と lowering。
- `await with` / `poll` / `select` lowering。
- flow 内 naked await の診断。

### `arcweft-scheduler`

- single-thread cooperative scheduler から実装。
- task event ordering を deterministic にする。
- native multi-thread は後から adapter。

## 5. Rendering / UI / Agent

### wgpu renderer

- headless offscreen を先に作る。
- object-id pass を早期実装。
- sprite / text / simple UI / screenshot。

### Agent Debug Bus

- semantic action を先に作る。
- screenshot/bbox は次。
- MCP は CLI で protocol が固まってから。

## 6. Audio

### MVP

- bus/mixer。
- BGM ensure/stop。
- SE one-shot。
- voice cue。
- dummy backend + native backend。
- signal: current_bgm, bus_levels。

### Advanced

- spatial audio。
- TTS provider。
- BGM stem/adaptive music。
- authoring/precompose。

## 7. Cranelift JIT

- VM が正。
- pure numeric function に限定して開始。
- `--compare-vm` を必須 debug mode にする。
- JIT compile pending 中は VM fallback。
- CLIF dump / asm dump / perf metrics を実装。

## 8. Contracts / Parser / Verification

- runtime contract check を先に実装。
- parser contract と ParseError を整える。
- Z3/OxiZ backend は Proof IR が安定してから。
- Rust Activity には Kani harness generation を optional で追加。

## 9. 完成判定

最初の縦断シナリオ:

```text
.awft opening
  → parse / HIR / typecheck
  → graph build
  → headless run
  → render screenshot
  → choose semantic action
  → state transition
  → log/signal/assert capture
  → replay
```

これが通れば、以後の WGSL、Audio、UI、JIT、RAG は同じ土台へ載せられる。
