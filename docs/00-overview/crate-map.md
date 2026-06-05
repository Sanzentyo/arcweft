# crate / workspace 構成

## Core

```text
arcweft-api
arcweft-core
arcweft-task
arcweft-scheduler
arcweft-need
arcweft-suspend
arcweft-borrow
arcweft-region
arcweft-module
arcweft-lazy
arcweft-adt
arcweft-result
arcweft-contract
arcweft-proof-ir
arcweft-parse
arcweft-macro
arcweft-template
arcweft-precompile
arcweft-hooks
arcweft-memo
arcweft-device-stream
```

## Language / Graph

```text
arcweft-lang-syntax
arcweft-lang-hir
arcweft-lang-sema
arcweft-runtime-plan
arcweft-lang-ir
arcweft-lang-vm
arcweft-verify-lsp
arcweft-lsp
arcweft-lang-jit-cranelift
arcweft-lang-aot-rust
arcweft-id
arcweft-ref
arcweft-graph
arcweft-graph-store
arcweft-graph-patch
arcweft-history
arcweft-rag
arcweft-agent-protocol
```

## Runtime / Extension

```text
arcweft-activity
arcweft-activity-narrative
arcweft-plugin-rust-api
arcweft-plugin-rust-macros
arcweft-plugin-dylib
arcweft-plugin-process
arcweft-plugin-wasm
arcweft-wasm-abi
arcweft-wasm-tools
arcweft-wasm-wasmtime
arcweft-wasm-browser
arcweft-memory
arcweft-ipc
arcweft-ipc-iceoryx
arcweft-ipc-mmap
arcweft-hook-runtime
arcweft-memo-runtime
```

## Presentation

```text
arcweft-render
arcweft-presentation
arcweft-layer-core
arcweft-layer-input
arcweft-layer-hooks
arcweft-render-text
arcweft-shader-core
arcweft-shader-dsl
arcweft-shader-validate
arcweft-shader-reflect
arcweft-shader-precompile
arcweft-shader-hot-reload
arcweft-ui-core
arcweft-ui-reactive
arcweft-ui-layout
arcweft-ui-render
arcweft-ui-style
arcweft-ui-widgets
arcweft-ui-servo
arcweft-ui-dom
arcweft-vector
arcweft-svg
arcweft-text-core
arcweft-text-rich
arcweft-text-typst
arcweft-audio-core
arcweft-audio-mixer
arcweft-audio-spatial
arcweft-audio-tts
arcweft-audio-bgm
arcweft-audio-authoring
arcweft-capture-core
arcweft-capture-audio-cpal
arcweft-capture-video-shiguredo
arcweft-capture-video-nokhwa
arcweft-capture-web
arcweft-capture-virtual
arcweft-capture-agent
arcweft-device-core
arcweft-device-profile
arcweft-device-generator
arcweft-device-lsp
arcweft-device-agent
arcweft-usb-core
arcweft-usb-nusb
arcweft-usb-rusb
arcweft-usb-webusb
arcweft-hid-core
arcweft-hid-hidapi
arcweft-controller-core
arcweft-controller-virtual
arcweft-controller-gamepad
arcweft-controller-usb
```

## Tooling

```text
arcweft-log
arcweft-signal
arcweft-assert
arcweft-test
arcweft-bench
arcweft-hook-lsp
arcweft-memo-lsp
arcweft-hook-debug
arcweft-memo-debug
arcweft-agent-protocol
arcweft-agent-bus
arcweft-agent-observe
arcweft-agent-action
arcweft-agent-headless
arcweft-agent-mcp
arcweft-agent-cli
arcweft-cli
arcweft-debug
arcweft-lang-hir
arcweft-lang-sema
arcweft-runtime-plan
arcweft-verify-lsp
arcweft-lsp
arcweft-verify
arcweft-verify-z3
arcweft-verify-oxiz
arcweft-jj
arcweft-tooling
arcweft-launch
```

## 依存ルール

- `arcweft-core` は runtime/data core に限定し、dialogue / presentation / syntax / verifier / CLI / LSP / wgpu / audio / Servo / DOM / filesystem / network / Cranelift / Wasmtime に依存しない。
- 広い application prelude は facade crate `arcweft` が提供する。低レイヤ crate に便利 re-export を置かない。
- `arcweft-presentation` は `bg(...)` / `show(...)` が返す scope-bound
  presentation handles、typed target/slot refs、clear operations、scope exit
  cleanup registry を持つ Sans I/O data/model crate とする。renderer、
  filesystem、asset loading、windowing、clock には依存しない。
- Data-format crate は Sans I/O を保つ。manifest、schema、bytecode、bundle、save snapshot は構造体と bytes/string codec までを担当し、path read/write、network、clock、backend resource 確保は CLI / build / player adapter に置く。
- UI / shader / audio / Activity は `Command` / `TaskSpec` / `Need` / `EffectRequest` を介する。
- Hook は phase ごとの構造化 action を返し、直接 host API を呼ばない。
- Memoization は pure computation または TaskKey deduplication に限定し、cache は決定性に影響してはならない。
- unsafe は `arcweft-memory`、`arcweft-plugin-*`、`arcweft-render`、`arcweft-audio-*` の境界に閉じ込める。
- `arcweft-agent-protocol` は CLI / MCP / test / LLM が共通利用する。
- `arcweft-agent-mcp` は `arcweft-agent-protocol` の observation/resource を
  MCP `resources/read` / tool result 互換の Sans I/O JSON shape へ変換する。
  stdio、HTTP、auth、session lifecycle、renderer readback は持たない。
- `arcweft-lang-syntax` は rowan-compatible な lossless CST と surface parser を所有する。`SyntaxKind`、`TokenKind`、green tree、`SyntaxNode`、source text / line index、error-tolerant `ParsedSource`、surface AST、expression/type/pattern parsing、syntax lint をここに集約する。HIR lowering、semantic checks、runtime-plan lowering は持たない。
- `arcweft-lang-hir` は HIR 型と `lower_to_hir` を所有し、semantic passes、verifier、CLI、LSP はこの crate を HIR 入力境界にする。
- `arcweft-lang-hir` は syntax/CST 由来の typed ID context も所有する。
  `arcweft-tooling`、CLI、LSP は dialogue ID や choice ID を独自 scan
  せず、この context から edit / hint / code action を作る。
- `arcweft-lang-sema` は name registry、symbol use collection、reference validation、typecheck readiness、minimal type checking を所有する。
- `arcweft-runtime-plan` は checked HIR から `arcweft-core` の `RuntimePlan` / line task graph へ lowering する。
- `arcweft-verify` は Sans I/O の検証中核で、proof obligation、audit manifest、SMT problem、tool diagnostics schema を所有する。ファイルI/O、process起動、watch、editor transport は持たない。
- `arcweft-verify-z3` は外部 Z3 process adapter、`arcweft-verify-oxiz` は pure Rust OxiZ adapter とする。solver依存は `arcweft-verify` や `arcweft-core` に入れない。
- `arcweft-verify-lsp` は transportなしの LSP helper crate とし、
  `arcweft-verify` report から diagnostics / code actions を作る。
  Source-aware position conversion is exposed through a mapper trait; stdio,
  client state, file watching, and request dispatch stay outside this crate.
- `arcweft-lsp` は `lsp-server` based stdio transport crate とし、
  initialize/shutdown、client capability negotiation、FULL text sync document
  cache、publish diagnostics、request dispatch、workspace command routing を
  所有する。Rust metadata generation and profile/adapter file I/O remain
  build/CLI/adapter responsibilities and are read as resolved metadata by the
  transport.
- `arcweft-test` は `test` / `bench` 宣言を HIR から Sans I/O manifest に変換する。ファイルI/O、clock、renderer/audio driving、benchmark timers、headless player 実行は CLI / player / adapter crate に置く。
- `arcweft-launch` は `arcw.toml` launch profiles を typed data と TOML codec
  として所有する Sans I/O crate とする。ファイル探索、current directory、
  process 環境、network binding、adapter execution は CLI / player adapter 側の
  責務。
- `parse_source` は `ParsedSource { syntax, typed_tree, errors, source_hash, line_index }` のように常に lossless CST と diagnostics を返す。typed source model は `TypedSyntaxTree` として CST / rowan `SyntaxNode` と区別する。内部の行単位 parser は短期 MVP であり、delimiter recovery、top-level punctuation / keyword split、binding split、multi-token punctuation sequence split などの構文走査は CST helper へ集約し、これ以上 `split_top_level` 型の ad hoc parser を拡張しない。
- Cranelift は `arcweft-lang-jit-cranelift` の native-only 最適化 backend に閉じ込める。`arcweft-core` に `jit-cranelift` feature や Cranelift 依存を置かない。
- Wasmtime は `arcweft-wasm-wasmtime` の native plugin/activity sandbox 用 adapter であり、Arcweft runtime の主実行系ではない。WIT ABI は `arcweft-wasm-abi`、Wasm validation/generation/inspection は `arcweft-wasm-tools` が担当する。

- Capture devices are permissioned live sources; scripts and Activities consume granted ports, not raw device APIs.
- USB/HID/Serial/Gamepad are also permissioned DevicePorts and expose typed Source streams.
- Touch virtual controllers are Game Native UI layers that emit logical input events and Agent action targets.

- USB / HID devices are permissioned DeviceProfiles; scripts consume typed ports and signals, not raw handles.
- The Device Profile Generator emits parsers, writers, signal bindings, test fixtures, and backend stubs from `.arcw` manifests.
- Touch virtual controllers are Game Native UI components attached to input layers and emit logical `ControllerEvent`s.

- Device streams are `Source<T, E>` values with explicit backpressure, replay, privacy, and cancellation policy; do not expose backend callbacks directly to DSL code.
- USB/HID/Gamepad/VirtualController input emits normalized `InputAction` values into the layer-based input router.

