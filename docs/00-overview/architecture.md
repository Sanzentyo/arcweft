# 全体アーキテクチャ

## 目的

wgpu を基盤にした、native と WebGPU/WebGL 経路の両方に対応するノベルゲームエンジンを作る。ノベルゲーム本編だけでなく、トラックゲームや FPS ミニゲーム、HTML/CSS UI、リアクティブ native UI、カスタム WGSL、WASM plugin、Rust plugin、Cranelift JIT、形式検証、LLM デバッグを一体として扱う。

## 全体図

```text
Game Source / Bundle
  .awft DSL
  .wgsl / shader blocks
  .svg / vector
  .typ / rich text / typst blocks
  .wasm plugin
  Rust Activity crate
  assets / audio / ui html
        │
        ▼
Compiler / Tooling
  CST → HIR → Typed Graph → IR → Bytecode/JIT/Bundle
  contracts / parser / shader / audio / UI / graph / RAG
        │
        ▼
Sans I/O Core
  Engine::step(FrameInput) -> FrameOutput
  deterministic state machine
  no GPU / no FS / no wall-clock / no raw network
        │
        ├── Task Scheduler / Need runtime
        ├── Activity host
        ├── WASM / Rust plugin host
        ├── Cranelift JIT backend
        ├── wgpu renderer
        ├── Render / Input LayerStack
        ├── Game Native UI renderer
        ├── Servo / DOM HTML UI backend
        ├── Audio mixer / spatial / TTS / BGM graph
        └── Agent Debug Bus / MCP / CLI
```

## 主要な境界

### Core は Sans I/O

`arcweft-core` は GPU、Audio、Servo、DOM、filesystem、network、WASM runtime、Cranelift runtime に直接依存しない。

```rust
Engine::step(input: FrameInput) -> FrameOutput
```

`FrameOutput` は命令を実行しない。実行すべきことを `Command` / `EffectRequest` / `TaskSpec` として返す。

### 時間がかかるものは `Need<T, E>`

asset load、shader compile、lazy use realization、Activity instantiate、TTS 生成、BGM pre-render、Typeset block の組版などは `Need<T, E>` として扱う。

`Need<T, E>` は `T` に暗黙変換できない。`flow` や UI では `await ... with { pending ... }` または `AwaitView` で待機時の挙動を明示する。

### 実行可能単位は Activity

ノベルゲーム本編、トラックゲーム、FPS ミニゲーム、外部プロセス、WASM plugin、Rust plugin は `Activity` として統一する。

```rust
pub trait Activity {
    fn mount(&mut self, ctx: MountContext<'_>) -> Result<MountResult, ActivityError>;
    fn step(&mut self, input: FrameInputView<'_>, out: FrameOutputWriter<'_>) -> StepStatus;
    fn save(&self, out: SaveWriter<'_>) -> Result<()>;
    fn load(&mut self, input: SaveReader<'_>) -> Result<()>;
}
```

### Layer は描画と入力の共通境界

描画 system は `LayerTree` を持ち、world、character、effect、Activity、native UI、HTML UI、modal、debug overlay を明示的な layer として扱う。入力も同じ layer stack で上位から hit-test し、`Consumed` / `PassThrough` / `Blocked` を返す。詳細は [Layer System](../03-presentation/layers.md)。


### LayerStack は描画と入力の共通単位

`RenderSpec` は `LayerStackSpec` を持ち、背景、立ち絵、dialogue、choice、native UI、HTML UI、modal、debug overlay、Activity viewport を同じ stack 上で扱う。入力は top-most layer から routing され、modal、focus、pointer capture、semantic action、Agent hit-test は layer state として管理する。詳細は [Render / Input Layer System](../03-presentation/render-input-layers.md)。

### UI は二系統

- Game Native UI: SwiftUI 風、リアクティブ、wgpu/vector/text、Agent 観測に最適。
- HTML/CSS UI: native は Servo、web は browser DOM。

どちらも layer に載り、最終的に `UiEvent` と `ActionTarget` を返す。

### 音声も構造化

BGM、SE、Voice、TTS、spatial source、mixer bus、ducking、loudness、loop、stem はすべて audio graph で扱う。詳細は [Audio System](../03-presentation/audio.md)。

### LLM は構造化 API で操作

Agent Debug Bus は画像だけでなく以下を返す。

- screenshot
- UI tree
- scene graph
- bbox / polygon / segmentation mask
- action target
- logs / signals / metrics / assertions
- state diff
- replay trace

MCP と CLI は同じ `arcweft-agent-protocol` を使う。



## Capture Device Layer

Microphone and camera input are handled by a dedicated permissioned capture layer. Native audio uses CPAL, native camera prefers `shiguredo_video_device`, optional camera compatibility uses `nokhwa`, and Web capture uses `web-sys` MediaDevices. Capture enters the runtime through `Need`, signals, and Activity ports rather than direct device APIs. See [Microphone / Camera Capture Devices](../03-presentation/capture-devices.md).
