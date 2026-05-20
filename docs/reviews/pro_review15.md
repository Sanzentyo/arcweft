結論から言うと、**LLM 用デバッグ機能は「設計にはかなり明確に組み込まれています」。ただし、現状実装としてはまだ十分ではありません。**
今の `arcweft` は、あなたが言っている「画像、bbox、多角形、セグメント、id 付き UI/オブジェクト、ボタン押下、ドラッグ、UI を操作しながらの対話的デバッグ」を実現するための設計要素は持っていますが、**実装済みランタイム / MCP / CLI / renderer / input routing まではまだ到達していない**状態です。

## 設計上は入っています

`docs/04-tooling/agent-debug-mcp-cli.md` に、まさに LLM / Agent 向けの `AgentDebugBus` が定義されています。`observe`、`act`、`resource`、`subscribe` を持つ設計で、観測、操作、リソース取得、イベント購読をまとめる形です。Observation には `images`、`objects`、`actions`、`ui_tree`、`scene_graph`、`logs`、`signals`、`diagnostics` が入る想定になっています。

画像系も明記されています。`color screenshot`、`overlay screenshot`、`object-id image`、`bbox`、`polygon`、`segmentation mask: RLE / PNG alpha / raw bitmap` が列挙されているので、あなたの「画像と bbox や多角形、セグメント情報などを取得したい」という要求は設計上カバーされています。

操作についても、物理操作として `PointerClick`、`PointerDrag`、`KeyDown / KeyUp`、`TypeText` があり、意味的操作として `Invoke`、`SelectChoice`、`AdvanceText`、`OpenMenu`、`SetSlider` などが定義されています。さらに「semantic action を優先し、座標 click は fallback」と明記されているので、LLM が座標だけで不安定に押すのではなく、`target id` ベースで押す設計思想になっています。

MCP リソースとしても、`latest.json`、`color.png`、`overlay.png`、`objects.json`、`state/current.json`、`logs.ndjson`、`signals.json`、`audio.json` が想定されています。MCP tools も `arcweft.observe`、`arcweft.click`、`arcweft.invoke`、`arcweft.choose`、`arcweft.wait_until`、`arcweft.step_frames`、`arcweft.get_state` などが定義されています。

## UI / レイヤー / hit-test との接続も設計されています

`Layer System / Input Routing` 側でも、レイヤーは「描画順だけでなく、入力、hit-test、focus、modal、Agent 観測、test、replay を束ねる中核概念」とされています。つまり、LLM が見ている画像、UI tree、hit region、入力 routing がバラバラではなく、同じ `LayerTree` に紐づく設計です。

`ObservedObject` には `id`、`entity`、`layer`、`bbox`、`polygon`、`mask` が入る想定です。これは「画面上のこのボタン/キャラ/領域を id で参照し、bbox/polygon/mask を見て、クリックやドラッグを行う」というユースケースにかなり近いです。

別ドキュメントの `layers-and-input.md` では、`HitRegion` に `id`、`layer`、`target`、`role`、`enabled`、`visible`、`priority`、`bbox`、`polygon`、`mask`、`actions`、`source` を持たせる設計になっています。hit-test の流れも bbox → polygon → mask の順で定義されているため、LLM が `objects.json` を見て「この id の対象を押す」「この drag handle を動かす」といった操作に使いやすい構造です。

UI component の `on_click` も、`UiNode → HitRegion → ActionTarget → LayerInputEvent → UiEvent` へ lowering される想定です。これは非常に重要で、LLM が「見た目の座標」ではなく「意味のある ActionTarget」を呼べるようにする設計です。

## ただし、現状実装としてはまだ未完成です

ロードマップでは、`wgpu / UI / Agent` は Phase 2 に置かれており、その中に `headless wgpu renderer`、`screenshot / object-id pass / bbox`、`Game Native UI tree`、`Agent Debug Bus`、`CLI / MCP observe & action` が含まれています。さらに Phase 7 で `Agent debugging at scale` と product mode の auth / audit が置かれています。

現在の workspace 実装を見ると、`Cargo.toml` の実装済み crates は `arcweft-core`、`arcweft-dialogue`、`arcweft-id`、`arcweft-lang-syntax`、`arcweft-need`、`arcweft-presentation`、`arcweft-source`、`arcweft-cli` だけです。`arcweft-agent-bus`、`arcweft-agent-observe`、`arcweft-agent-action`、`arcweft-agent-mcp`、`arcweft-agent-cli` などは workspace member にはまだ入っていません。

実装ステータス文書でも、未実装として `wgpu renderer`、`Servo / DOM UI`、`audio backend`、`MCP / agent protocol runtime`、`full RuntimeStepInput/RuntimeStepOutput event envelopes`、`layered input routing` などが明示されています。

また、現在の `arcweft-cli` は実質 `arcw check <file.awft>` のみで、`arcw agent observe` や `arcw agent click` はまだ実装されていません。

## 「これで十分か？」への答え

**設計の方向性としてはかなり良いです。だが、LLM デバッグ用途に対して “十分” と言うには、まだ不足があります。**

不足しているのは主に次の部分です。

1つ目は、**Observation / Action の厳密な JSON schema** です。ドキュメントには構造が書かれていますが、LLM や MCP tool が安定して使うには、`objects.json`、`actions.json`、`ui_tree.json`、`input_trace.json`、`masks/*` のスキーマを固定した方がよいです。

2つ目は、**id の設計** です。`ObjectId`、`ActionTarget`、`EntityId`、`PublicId` の概念はありますが、LLM 用には次を分けた方が安全です。

```text
entity_id        = 永続的な意味 ID。例: character.alice, choice.opening.listen
ui_node_id       = UI tree 上の node ID
object_id        = frame 内の検出/描画 object ID
hit_region_id    = click/drag/hit-test 対象の ID
action_id        = 実行可能 action の ID
mask_id          = mask resource の ID
frame_id/tick    = どの観測フレームの情報か
```

3つ目は、**座標空間の明示** です。LLM が bbox を見てクリックするには、各 bbox / polygon / mask がどの空間なのかが必要です。

```json
{
  "bbox": { "x": 420, "y": 510, "w": 280, "h": 64, "space": "viewport" },
  "layer_space_bbox": { "x": 20, "y": 10, "w": 280, "h": 64 },
  "image_size": { "w": 1280, "h": 720 },
  "device_pixel_ratio": 2.0
}
```

4つ目は、**action result / input trace** です。LLM が操作した後に、「押せたのか」「disabled だったのか」「modal に遮られたのか」「別 target に hit したのか」を返す必要があります。設計には `ActionResult` や `input_trace` の方向性がありますが、ここはかなり重要なので、最初から標準化した方がいいです。

```json
{
  "action_id": "action.choice.opening.listen.select",
  "result": "handled",
  "target": "choice.opening.listen",
  "routed_to_layer": "layer.choice_ui",
  "events_emitted": [
    { "type": "GameEvent.ChoiceSelected", "id": "choice.opening.listen" }
  ],
  "blocked_by": null,
  "tick_before": 120,
  "tick_after": 121
}
```

5つ目は、**drag / gesture の意味的 action** です。`PointerDrag` はありますが、LLM にとっては座標 drag よりも次のような action が必要になります。

```text
drag action.timeline.scrubber to value=0.42
drag action.window.resize_handle.bottom_right by dx=120 dy=40
drag action.node.graph.edge from node.a.port.out to node.b.port.in
```

つまり、単なる `from/to` ではなく、`drag_handle_id`、`allowed_axis`、`value_range`、`snap_points`、`preview_result` などを `ActionTarget` に載せるべきです。

## 追加するとよい最小セット

LLM が本当に「見て、考えて、押して、また見る」デバッグをできるようにするなら、まずこのセットを Phase 2 の MVP にした方がよいです。

```text
arcweft-agent-protocol
  - Observation schema
  - ObservedObject / ObservedLayer / HitRegion / ActionTarget schema
  - AgentAction / ActionResult schema

arcweft-agent-observe
  - color screenshot
  - overlay screenshot
  - object-id image
  - objects.json
  - actions.json
  - ui_tree.json
  - hit_regions.json

arcweft-agent-action
  - invoke(action_id)
  - click(target_id)
  - click(x,y)
  - drag(action_id or from/to)
  - type_text(target_id, text)
  - key(...)
  - wait_until(...)

arcweft-agent-mcp
  - arcweft.observe
  - arcweft.resource
  - arcweft.invoke
  - arcweft.click
  - arcweft.drag
  - arcweft.hit_test
  - arcweft.input_trace
  - arcweft.wait_until

arcweft-agent-cli
  - arcw agent observe --json
  - arcw agent observe --image overlay
  - arcw agent objects
  - arcw agent actions
  - arcw agent hit-test --x --y
  - arcw agent click --target
  - arcw agent drag --target --to
```

## 私の判断

**今の設計は、LLM デバッグ基盤としてかなり良い土台です。特に “semantic action 優先、座標 click は fallback” という方針は正しいです。**
ただし、現状はまだ「設計済み」であって「実装済みで十分」ではありません。

あなたの要求レベルまで持っていくなら、次の一文を設計原則として明文化するとよいです。

> Agent / LLM は、画像だけでなく、同一 frame に対応する object-id image、overlay image、ui tree、hit regions、action targets、input trace、state/log/signal を取得でき、すべての操作は可能な限り stable id / semantic action によって実行し、座標操作は fallback とする。

この原則に沿えば、`arcweft` はかなり強い「LLM が UI を触りながらデバッグする runtime」になります。今のドキュメントはその方向を向いていますが、次に必要なのは **Agent Protocol のスキーマ固定と、最小 MCP/CLI 実装** です。
