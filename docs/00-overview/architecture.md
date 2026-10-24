# 全体アーキテクチャ

このページは現在の層と仕様 owner の案内です。構文、実行形式、描画、入力、
device permission、Agent protocol の詳細は、以下の保守章がそれぞれ所有します。

## 言語と実行

`.arcw` source は parser、HIR、sema を通り、受理済み実行入力になります。
source / development 実行は受理済み `RuntimePlan` を `RuntimePlanVm` または
`StructuredAot` で実行し、product 実行は `.awfb` 内の canonical `AWBC v1` を
`AwbcProduct` で実行します。どの tier も同じ `RuntimeStep` 入出力契約を通ります。
実行 tier と product representation の
詳細は [Converged Language Surface](../01-language/converged-language-surface.md)、
[Await and Need](../01-language/await-need-result.md)、
[State, Flow, Reducer, and View](../01-language/state-flow-reducer.md)、
[RuntimeStep and Executors](../02-runtime/runtime-step-and-executors.md)、
[Executable Runtime Core](../02-runtime/executable-runtime-core.md)、
[Product AWFB Bytecode](../05-build-and-security/product-awfb-bytecode.md) が所有します。

## Core と host

`arcweft-core` は Sans I/O です。typed input を受け取り、effect と host request
をデータとして返します。filesystem、network、device access、window、renderer、
audio backend の実行は host / adapter 層が担当します。data-format model と codec
も Sans I/O とし、その crate 境界は [crate/workspace map](crate-map.md) が示します。
step 境界は
[Runtime Core](../02-runtime/core.md) と
[RuntimeStep and Executors](../02-runtime/runtime-step-and-executors.md)、
外部資源の保存規則は
[Authored resource and local state storage](../05-build-and-security/authored-resource-storage.md)
を、Activity lifecycle と snapshot は
[Activity Model](../02-runtime/activity.md) を参照してください。

## Presentation と input

Presentation の `LayerTree` は描画と input routing で共有します。OS、window、
browser の raw input は host が受け取り、routing 済みの event を Core に渡します。
詳細は [Layer System](../03-presentation/layers.md) と
[Layered Input](../02-runtime/layered-input.md)、[Game Native View](../03-presentation/view-reactive.md)、
[HTML / Servo / DOM View](../03-presentation/html-servo-dom.md) を参照してください。

Audio と microphone / camera capture はそれぞれの presentation owner が typed
contract と permission boundary を定義します: [Audio System](../03-presentation/audio.md)、
[Microphone / Camera Capture Devices](../03-presentation/capture-devices.md)。
Device ports、typed streams、generator policy、backpressure、privacy、cancellation は
[Streams, Generators, and External Capabilities](../02-runtime/streams-generators.md)、
[Device Generator / Profile System](../05-build-and-security/device-generator.md)、
[Security](../05-build-and-security/security.md) が所有します。

## Tooling と観測

CLI と MCP は共通の agent protocol を使います。observation と capture bytes の
metadata は別の維持章が所有します: [Agent Debug Bus / MCP / CLI](../04-tooling/agent-debug-mcp-cli.md)、
[Agent Observe and Capture Contract](../04-tooling/agent-observe-capture-contract.md)。

