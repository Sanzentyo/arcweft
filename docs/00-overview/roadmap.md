# 実装ロードマップ

## Phase 0: 基本言語と Core

- lossless CST parser
- HIR / type check
- `Option` / `Result` / `Need`
- EntityId / PublicId / Ref
- Sans I/O `Engine::step`
- simple flow / reducer / view

## Phase 1: Graph / LSP / test

- Typed Narrative Graph
- GraphPatch
- LSP diagnostics / inlay ID
- scenario test
- logging / signal / assert

## Phase 2: wgpu / UI / Agent

- headless wgpu renderer
- screenshot / object-id pass / bbox
- Game Native UI tree
- Agent Debug Bus
- CLI / MCP observe & action

## Phase 3: Asset / Shader / Audio

- asset DAG and hot reload
- typed WGSL shader pipeline
- audio mixer graph, BGM/SE/Voice basics
- spatial audio and TTS API skeleton

## Phase 4: Contracts / Parser / Verification

- contract lowering
- parser combinators
- Z3 / OxiZ backend
- Kani/Creusot harness generation for Rust extension

## Phase 5: Extension / JIT

- Activity API
- static Rust Activity
- WASM plugin
- dylib/process plugin
- shared memory IPC
- Cranelift JIT for pure numeric/dataflow functions

## Phase 6: Advanced Presentation

- Servo native HTML backend
- Web DOM backend
- Vector/SVG normalization
- RichText and Typst bridge
- BGM authoring and adaptive music

## Phase 7: RAG / JJ / Product QA

- Jujutsu node history
- GraphRAG index
- Agent debugging at scale
- product mode capability / auth / audit

