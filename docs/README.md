# Arcweft Engine 設計仕様

このフォルダは、wgpu を中核にしたノベルゲームエンジンの設計仕様です。独自 DSL、Rust 拡張、WASM、Cranelift JIT、LayerTree、Object Hook、Memoization、Servo/DOM View、リアクティブ View、Typst 級テキスト、WGSL シェーダー、音声、マイク/カメラ入力、USB/HIDデバイス、タッチ向けバーチャルコントローラー、形式検証、RAG、Jujutsu 履歴管理、LLM デバッグインターフェースを統合した実装方針をまとめています。

## 読み方

1. [全体アーキテクチャ](00-overview/architecture.md)
2. [命名・拡張子・公開識別子](00-overview/naming.md)
3. [決定事項と設計原則](00-overview/decisions.md)
4. [Runtime Boundary Refactor Direction](00-overview/runtime-boundary-refactor.md)
5. [DSL 概要](01-language/README.md)
6. [Entries and Capabilities](01-language/entries-and-capabilities.md)
7. [実行モデル](02-runtime/README.md)
8. [RuntimeStep and Executors](02-runtime/runtime-step-and-executors.md)
9. [描画・View・音声](03-presentation/README.md)
10. [ツール・検証・LLM デバッグ](04-tooling/README.md)
11. [Authored resource and local state storage](05-build-and-security/authored-resource-storage.md)

## Arcweft 固有の命名

- エンジン名: **Arcweft Engine**
- CLI: `arcw`
- ソース拡張子: `.arcw`
- バンドル: `.awfb`
- セーブ: `.awfs`
- トレース: `.arcwx`
- Agent script: `.awfagent`

詳細は [命名・拡張子・公開識別子](00-overview/naming.md) を参照してください。

## 重要な追加章

- [Layer System / Input Routing](03-presentation/layers.md)
- [Layered Input runtime](02-runtime/layered-input.md)
- [Device Streams / Generator Policy](02-runtime/device-streams.md)
- [Streams, Generators, and Live Device Sources](02-runtime/streams-generators.md)
- [Microphone / Camera Capture Devices](03-presentation/capture-devices.md)
- [USB / HID Devices](03-presentation/usb-devices.md)
- [Device I/O / USB / HID / Serial / Gamepad](03-presentation/device-io-usb.md)
- [Virtual Touch Controller](03-presentation/virtual-controller.md)
- [Flow-Integrated Scenario Syntax / Dialogue Sugar](01-language/scenario-surface-syntax.md)
- [Block Scopes and `{ ... }`](01-language/block-scopes.md)
- [module / use / pub](01-language/modules.md)
- [ID と参照](01-language/ids-and-references.md)
- [文法サマリ](01-language/grammar.md)
- [Standard Types and Prelude](01-language/standard-types-and-prelude.md)
- [Entries and Capabilities](01-language/entries-and-capabilities.md)
- [Localization for Dialogue](01-language/localization-dialogue.md)
- [Dialogue Control Tags, Ruby, Inline Formatting, and Hooks](01-language/dialogue-control-tags-and-ruby.md)
- [Rich Text Effects and Transforms](03-presentation/rich-text-effects-transforms.md)
- [Agent Observe and Capture Contract](04-tooling/agent-observe-capture-contract.md)
- [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](01-language/dialogue-calls-scopes-cancellation.md)
- [Dialogue Content Calls, `with` Blocks, Line Output Values, and Scoped Handles](01-language/dialogue-line-handles-and-returns.md)
- [Dialogue Windows, Character Styles, and Read-State Hooks](01-language/dialogue-windows-and-hooks.md)
- [Dialogue Character Methods, TextBox Targets, Interpolation, and Preload](01-language/dialogue-character-methods-and-textbox.md)
- [TextBox Manifest](schemas/textbox-manifest.md)
- [Concise Dialogue and Localization example](examples/concise-dialogue-and-localization.md)
- [Scope and Relative IDs example](examples/scope-relative-ids.md)
- [Character Stage / Sprite / Voice Timeline](03-presentation/character-stage.md)
- [Touch Virtual Controller](03-presentation/touch-virtual-controller.md)
- [Hooks and Memoization](01-language/hooks-and-memoization.md)
- [Proofs and Unsafe Lifetime Audits](01-language/proofs-and-unsafe-audits.md)
- [Runtime Hooks and Memoization](02-runtime/hooks-memoization.md)
- [Runtime Notes: Control Flow, Patterns, and Loops](02-runtime/control-flow-runtime.md)
- [Executable Runtime Core](02-runtime/executable-runtime-core.md)
- [RuntimeStep and Executors](02-runtime/runtime-step-and-executors.md)
- [Adapter Manifest](schemas/adapter-manifest.md)
- [USB Device Manifest](schemas/usb-device-manifest.md)
- [Virtual Controller Manifest](schemas/virtual-controller-manifest.md)
- [Hooks / memoization example](examples/hooks-memoization.md)
- [USB and Virtual Controller example](examples/usb-and-virtual-controller.md)

## Documentation conventions

- [Code Fence Policy](00-overview/code-fences.md)

