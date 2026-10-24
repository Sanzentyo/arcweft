# Layer System / Input Routing

レイヤーは描画順だけでなく、入力、hit-test、focus、modal、Agent 観測、test、replay を束ねる中核概念です。

関連:

- [wgpu renderer](wgpu-renderer.md)
- [Game Native View](view-reactive.md)
- [HTML / Servo / DOM View](html-servo-dom.md)
- [Agent Debug Bus / MCP / CLI](../04-tooling/agent-debug-mcp-cli.md)
- [Object Hooks / Memoization](../01-language/hooks-and-memoization.md)
- [Layer Manifest schema](../schemas/layer-manifest.md)
- [Layer Tree schema](../schemas/layer-tree.md)
- [Layered Input](../02-runtime/layered-input.md)
- [Memoization cache schema](../schemas/memo-cache.md)
- [Layer example](../examples/layers-input.md)

## 目的

描画と入力は明示的な `LayerTree` を共有し、layer identity、順序、表示、hit-test、focus、
modal routing、Agent 観測、test、replay を同じ構造に結びつけます。

```text
LayerTree
  ├─ world/background
  ├─ world/characters
  ├─ world/effects
  ├─ activity/fps-arena
  ├─ view/game
  ├─ view/html-servo-or-dom
  ├─ overlay/loading
  ├─ overlay/modal
  ├─ debug/agent
  └─ capture/object-id
```

各 layer は以下を持つ。

```text
- stable `LayerId` / optional `PublicId`
- render order / input order
- visibility / opacity / transform / clip
- camera / viewport / render target
- blend / compositing / postprocess
- input policy / hit-test policy / focus scope
- Agent 観測 metadata
```

## 中核型

`LayerId` は tree 内で安定した識別子です。Pure-data の `LayerTree` は root、
`LayerNode` 群、決定的な `render_order` / `input_order` を所有します。各 node は
layer kind と content、親子関係、`LayerOrder`、transform、visibility、input policy
を持ちます。公開識別子は任意です。現在の型と private-field API は
[`arcweft-presentation` の layer model](../../crates/arcweft-presentation/src/layer.rs)
を参照してください。

Presentation contract はさらに、各 layer に visibility、opacity、blend、transform、
clip、camera、viewport、render target、postprocess を含む描画 policy、入力・hit-test・
focus・capture policy、および Agent 観測 metadata を関連づけます。Rendered frame plan は
viewport size、clear policy、`LayerTree`、global postprocess をまとめます。

## Layer render policy

描画 policy は opacity、blend、transform、clip、camera、viewport、target、layer-local
postprocess を制御します。target は main surface、offscreen、object-id pass、mask-only
のいずれかです。View scene の group pass と compositor の実行順は
[Layered Rendering](layered-rendering.md) を参照してください。

`LayerOrder` は `(phase, z, stable_index)` の typed key で、各 RenderPhase は
Background、World、Characters、Effects、Dialogue、GameView、HtmlView、Modal、Debug、
AgentOverlay です。型定義は
[`arcweft-presentation` の layer model](../../crates/arcweft-presentation/src/layer.rs)
にあります。

### 描画順

`LayerTree` は typed `LayerOrder` の昇順で描画順を導出し、同じ key の場合は
`LayerId` を最後の tie-breaker にします。入力順は通常、描画順の逆順です。Modal、
capture、input priority は routing policy に従って入力候補と配送先を決めます。詳細は
[Layered Input](../02-runtime/layered-input.md)。

```text
z=-1000 background
z=0     character / world
z=100   effects
z=1000  game view
z=2000  html view
z=3000  modal
z=9000  debug overlay
```

### Layer cache

Offscreen layer output may use no cache, reuse until invalidated, or reuse for a fixed
number of frames. The runtime owns cache keys and invalidation: a change to a layer or
dependent View subtree invalidates the affected cached output, including after a hot
reload patch. See the [memoization cache contract](../schemas/memo-cache.md).

## Input routing

入力は `input_order` を上から下へ走査します。layer の typed input policy は有効状態、
priority、hit-test、route、focus、pointer/keyboard/gamepad capture を制御します。
Hit-test は none、bbox、polygon、mask、View tree、または owner-provided logic を使えます。
Route behavior は pass-through、hit 時に下位を止める block-below、miss でも下位を止める
modal、read-only な observe-only を含みます。

Routing results record consumed、passed-through、blocked と focus changes。実際の
`RoutedInputEvent`、`InputDisposition`、policy enum は
[Layered Input](../02-runtime/layered-input.md) が所有します。

### ルーティング手順

```text
InputEvent
  → normalize coordinates
  → collect candidate layers by input priority desc
  → skip invisible / disabled layers
  → hit-test each layer
  → if Modal layer exists above target, block below
  → deliver to layer handler
  → layer returns Consumed / PassThrough / Blocked
  → produce GameEvent / SemanticAction / ActivityInput / AgentActionResult
```

### Wheel input units

Native と Web の host は platform の line delta または physical-pixel delta を
共有する `arcweft-player-scene` 境界へ渡し、そこで logical pixel に正規化します。
既定では 1 line を 32 logical pixels とし、physical pixel は window scale factor
で除算します。非有限値、正でない scale factor、logical `f32` 範囲を超える値は
input error です。0、clamp、saturating cast へ暗黙変換しません。

## Focus scopes

Keyboard、text input、gamepad は pointer hit-test だけでは扱えないため、layer ごとに
focus scope と traversal policy を持ちます。Traversal は View tree order、明示順、
spatial、または無効です。Modal layer は独立した focus scope を持ち、閉じるまで下位
scope を停止します。

Hidden layer は既定で描画・入力の対象外です。hidden-but-focusable な内容は明示的な
policy が必要です。Screen reader 用 accessibility tree は View semantics と
LayerTree membership から生成します。

## Layer-scoped input bindings

Flow や Activity は layer に紐づく input scope を有効にできます。scope は安定した
identity、受け付ける input kind、priority、semantic action への binding を持ち、
focus と modal の規則に従って同じ routing path へ入ります。

## DSL: layer 宣言

project / module / scene で layer を宣言できる。

```arcw
pub layer @layer.world.background: World {
    z = -1000
    input = observe_only
    capture = color | object_id
}

pub layer @layer.world.characters: Character {
    z = 0
    input = pass_through
    capture = color | object_id | mask
}

pub layer @layer.view.game: NativeView {
    z = 1000
    input = block_below on_hit
    hit_test = view_tree
}

pub layer @layer.view.modal: Modal {
    z = 3000
    input = modal
    hit_test = view_tree
    focus = trap
}

pub layer @layer.debug.agent: Debug {
    z = 9000
    input = observe_only
    capture = overlay
}
```

Scene では layer に content を差し込む。

```arcw
scene.show(@scene.opening)
scope {
    layer @layer.world.background {
        image(
            asset = @asset:.bg.room,
            id = "image.scene.room",
            target = "target.scene",
            layer = "layer.world.background",
            x = 0px,
            y = 0px,
            width = 1280px,
            height = 720px,
            fit = "cover"
        )
    }

    layer @layer.world.characters {
        sprite(@asset:.char.alice.default)
            .at(center)
            .agent_target(@character.alice)
    }

    layer @layer.view.game {
        view(@view.MainDialogue)
        ChoiceList(choices)
    }
}
```

layer が省略された場合は default layer に入る。

```arcw
scene {
    bg(@asset:.bg.room)                            // writes @slot.background.default
    show(@character.alice, .normal)               // desugar: layer @layer.world.characters
    choice { ... }                                // desugar: layer @layer.view.game
}
```

Presentation calls that register visible values also choose a target/slot. The
default target is `@target.scene`; `bg(...)` writes
`@slot.background.default`, and `show(@character.alice, ...)` writes
`@slot.character.alice.default`. If a scene needs parallel backgrounds,
reflections, split-screen layers, or multiple copies of a character, the slot
or target must be explicit.

```arcw
let far = bg(@asset:.bg.city_far, slot = @slot.background.far)
let near = bg(@asset:.bg.city_near, slot = @slot.background.near)

let alice = show(@character.alice, .normal, target = @target.scene, slot = @slot.character.alice.main)
let alice_shadow = show(@character.alice, .shadow, target = @target.scene, slot = @slot.character.alice.shadow)
```

Slots are typed option-like cells. Setting a slot returns the previous value if
one was present; `bg.ref(...)` / `show.ref(...)` read a slot without changing
ownership; `bg.clear(...)` / `hide(...)` clear the slot and return the removed
value if present.

The core handle and slot model lives in the Sans I/O `arcweft-presentation`
crate. Render adapters consume the resulting state but do not own the data
format. The syntax/typecheck layer validates that background calls use
`@slot.background.*`, character calls use `@slot.character.*`, and targets use
`@target.*`.

## HTML / Servo / DOM layer

HTML/CSS View は Game Native View とは別の `HtmlView` layer として扱う。

```arcw
html panel @view.settings_html from "view/settings.html" {
    layer = @layer.view.html
    bounds = rect(0, 0, 100vw, 100vh)
    input = modal
}
```

Native では Servo、Web では DOM へ渡す。Agent 観測では同じ `LayerNode` と `ViewTree` に正規化する。

## Activity layer

FPS やトラックゲームなどは独立 Activity layer にできる。

```arcw
activity @activity.truck_game TruckGame {
    render_layer = @layer.activity.truck
    input_layer = @layer.activity.truck
}

pub layer @layer.activity.truck: Activity {
    z = 500
    input = block_below
    hit_test = bbox
}
```

Activity が modal minigame の場合:

```arcw
pub layer @layer.activity.fps: Activity {
    z = 2500
    input = modal
    keyboard_capture = all
    gamepad_capture = all
}
```

## Agent observation

Observation は layer identity、kind、描画順、visibility、input policy、bounds、
所属objectとactionを保持します。各object/actionには所属layerがあり、hit-testと
captureに必要なbbox、polygon、maskを同じ座標系で扱います。
公開観測・captureの型とCLI境界は
[Agent Observe and Capture Contract](../04-tooling/agent-observe-capture-contract.md)、
操作方法は [CLI](../04-tooling/cli.md) が所有します。

## Testing

```arcw
test @test.modal_blocks_world_input scenario {
    goto @flow.opening
    view.open(@view.settings_html)

    input.click(@character.alice)

    expect.input(blocked_by=@layer.view.modal)
    expect.no_event(GameEvent.CharacterClicked)
}
```

```arcw
test #test_choice_layer_bbox visual {
    goto @flow.opening
    wait.object(@choice.opening.listen, state=.visible)

    assert.layer(@choice.opening.listen, equals=@layer.view.game)
    assert.bbox(@choice.opening.listen, within=rect(400, 500, 500, 80))
}
```

## Contracts

layer には契約を付けられる。

```arcw
pub layer @layer.view.modal: Modal
ensures input.route == modal
ensures z > @layer.view.game.z
{
    z = 3000
    input = modal
}
```

View でも所属 layer を保証できる。

```arcw
view ChoiceList(choices: Vec<ChoiceView>)
ensures result.layer == @layer.view.game
ensures result.actions.all(_.layer == @layer.view.game)
{
    ...
}
```

## 固定する設計判断

```text
1. layer は描画・入力・Agent 観測を結びつける一級の単位。
2. render order は LayerOrder と LayerId で決め、input order はその逆順を基準とする。
3. 入力は上位 layer から下位 layer へ hit-test し、Consumed / PassThrough / Blocked を返す。
4. Modal layer は hit していない領域でも下位入力をブロックする。
5. View、HTML、Activity、Debug overlay はすべて layer に載る。
6. object-id pass と mask は layer 情報を持つ。
7. Headless と windowed で同じ LayerTree を使う。
```


## Layer trace observation

描画・layout・input routing は stable layer identity を持つ typed trace を生成する。
test、Agent、debugger は bbox、layout hash、consumed / blocked / passed-through
result を trace から read-only に検査する。観測のために author-defined global
callback を登録しない。


## Layer-owned policy and metadata

Layer declaration owns modal/input policy and Agent-visible metadata.

```arcw
layer @layer.modal.settings {
    input = modal
    priority = 1000
    block_below = true
}

layer choices {
    description = "現在表示中の選択肢レイヤー"
}
```

Local View/Activity handlers are dispatched in deterministic LayerTree order.

## Input routing phases

Input routing internally records the following phases:

```text
InputPreRoute
  → hit-test
InputHitTest
  → deliver to layer/View/activity
InputPostRoute
```

詳細は [Event Ownership and Caching](../01-language/hooks-and-memoization.md) と
[Runtime Dispatch and Caches](../02-runtime/hooks-memoization.md) を参照。

## Local interaction and read-only diagnostics

描画・入力・Agent 観測の追加処理は、それぞれ target View/Activity handler、
LayerTree policy、read-only trace consumer に置く。入力 routing に介入できる
owner-local handler は typed `InputDisposition` / semantic action だけを返し、
phase ごとの effect firewall を越えて state を直接変更しない。


## Device and virtual controller sources

Layered input accepts physical and virtual controller sources. See [Device Profiles, Generators, and USB](../03-presentation/device-generator-and-usb.md) and [Virtual Touch Controller](../03-presentation/virtual-controller.md).

```text
USB/HID/Gamepad/Keyboard/Touch
  -> ControllerMap
  -> LayerTree routing
  -> GameEvent / Activity input
```

Virtual controller layers usually sit above scene/View layers and consume touch input inside their hit regions before lower layers receive it.

## Virtual controller layer

A touch virtual controller is a renderable and input-capable layer.

```arcw
layer @layer.input.touch_controller {
    z = 900
    render = true
    input = true
    hit_test = controls_only
    pass_through = true
}
```

Controls inside this layer consume only their own hit regions and emit normalized actions. Touches outside controls pass through to lower layers. This allows visual novel View, minigame input, and mobile controls to coexist.

See [Touch Virtual Controller](touch-virtual-controller.md).


