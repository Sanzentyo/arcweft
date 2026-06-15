# Layer System / Input Routing

レイヤーは描画順だけでなく、入力、hit-test、focus、modal、Agent 観測、test、replay を束ねる中核概念です。

関連:

- [wgpu renderer](wgpu-renderer.md)
- [Game Native UI](ui-reactive.md)
- [HTML / Servo / DOM UI](html-servo-dom.md)
- [Agent Debug Bus / MCP / CLI](../04-tooling/agent-debug-mcp-cli.md)
- [Object Hooks / Memoization](../01-language/hooks-and-memoization.md)
- [Layer Manifest schema](../schemas/layer-manifest.md)
- [Layer example](../examples/layers-input.md)

## 目的

従来の `RenderSpec.layers: Vec<LayerSpec>` を、明示的な `LayerTree` に拡張する。

```text
LayerTree
  ├─ world/background
  ├─ world/characters
  ├─ world/effects
  ├─ activity/fps-arena
  ├─ ui/game
  ├─ ui/html-servo-or-dom
  ├─ overlay/loading
  ├─ overlay/modal
  ├─ debug/agent
  └─ capture/object-id
```

各 layer は以下を持つ。

```text
- EntityId / PublicId
- render order / input order
- visibility / opacity / transform / clip
- camera / viewport / render target
- blend / compositing / postprocess
- input policy / hit-test policy / focus scope
- Agent 観測 metadata
```

## 中核型

```rust
pub struct RenderSpec {
    pub size: UVec2,
    pub clear: Color,
    pub layers: LayerTree,
    pub postprocess: Vec<ShaderPassSpec>,
}

pub struct LayerTree {
    pub root: LayerId,
    pub nodes: IndexMap<LayerId, LayerNode>,
}

pub struct LayerNode {
    pub id: LayerId,
    pub entity: EntityId,
    pub public_id: PublicId,
    pub kind: LayerKind,

    pub render: LayerRenderPolicy,
    pub input: LayerInputPolicy,
    pub capture: LayerCapturePolicy,

    pub children: Vec<LayerId>,
    pub content: LayerContent,
}
```

`LayerId` は frame 内部 ID、`entity` は履歴・参照・Graph 用の安定 ID。

```arcw
pub enum LayerKind {
    World,
    Character,
    Effect,
    Activity,
    NativeUi,
    HtmlUi,
    Overlay,
    Modal,
    Debug,
    Capture,
}

pub enum LayerContent {
    Empty,
    Sprites(Vec<SpriteSpec>),
    Vectors(Vec<VectorSpec>),
    Text(Vec<TextSpec>),
    Ui(UiRenderSpec),
    Html(HtmlPanelLayerSpec),
    Activity(ActivityRenderSpec),
    Group,
    CustomShader(CustomMaterialSpec),
}
```

## Layer render policy

```rust
pub struct LayerRenderPolicy {
    pub z: i32,
    pub visible: bool,
    pub opacity: f32,
    pub blend: BlendMode,
    pub transform: Transform2D,
    pub clip: Option<ClipSpec>,
    pub camera: Option<Camera2D>,
    pub viewport: Option<ViewportSpec>,
    pub target: LayerTarget,
    pub postprocess: Vec<ShaderPassSpec>,
}

pub enum LayerTarget {
    MainSurface,
    Offscreen { format: TextureFormat, usage: LayerTargetUsage },
    ObjectIdPass,
    MaskOnly,
}
```

### 描画順

描画は `render.z` 昇順、同一 z では tree order で行う。

```text
z=-1000 background
z=0     character / world
z=100   effects
z=1000  game ui
z=2000  html ui
z=3000  modal
z=9000  debug overlay
```

## Input routing

入力は layer stack を上から下へ走査する。render z と input priority は基本連動するが、必要なら別指定できる。

```rust
pub struct LayerInputPolicy {
    pub enabled: bool,
    pub priority: i32,
    pub hit_test: HitTestPolicy,
    pub route: InputRoutePolicy,
    pub focus: FocusPolicy,
    pub pointer_capture: PointerCapturePolicy,
    pub keyboard_capture: KeyboardCapturePolicy,
    pub gamepad_capture: GamepadCapturePolicy,
}
```

```rust
pub enum HitTestPolicy {
    None,
    BBox,
    Polygon,
    Mask,
    UiTree,
    Custom(EntityId),
}

pub enum InputRoutePolicy {
    /// layer が受け取っても、消費しなければ下へ流す。
    PassThrough,

    /// hit したら下位 layer へ流さない。
    BlockBelow,

    /// modal。hit しない場所でも下位 layer をブロックする。
    Modal,

    /// 見えているが入力対象ではない。
    ObserveOnly,
}
```

### ルーティング結果

```rust
pub enum LayerInputResult {
    Consumed { by: LayerId, action: Option<ActionTargetId> },
    PassThrough,
    Blocked { by: LayerId, reason: BlockReason },
    FocusChanged { to: FocusTarget },
}
```

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
  → produce GameEvent / UiEvent / ActivityInput / AgentActionResult
```

## Focus scopes

keyboard / text input / gamepad は pointer hit-test だけでは扱えないため、layer ごとに focus scope を持つ。

```rust
pub struct FocusScope {
    pub layer: LayerId,
    pub active: Option<FocusTarget>,
    pub traversal: FocusTraversalPolicy,
}

pub enum FocusTraversalPolicy {
    UiTreeOrder,
    Explicit(Vec<FocusTarget>),
    Spatial,
    None,
}
```

modal layer は独立した focus scope を持ち、閉じるまで下位 scope を停止する。

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

pub layer @layer.ui.game: NativeUi {
    z = 1000
    input = block_below on_hit
    hit_test = ui_tree
}

pub layer @layer.ui.modal: Modal {
    z = 3000
    input = modal
    hit_test = ui_tree
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
        image(@asset.bg.room).fit(cover)
    }

    layer @layer.world.characters {
        sprite(@asset.char.alice.default)
            .at(center)
            .agent_target(@character.alice)
    }

    layer @layer.ui.game {
        TextBox(current_text())
        ChoiceList(choices)
    }
}
```

layer が省略された場合は default layer に入る。

```arcw
scene {
    background(image(@asset.bg.room))             // desugar: layer @layer.world.background
    show(@character.alice, .normal)               // desugar: layer @layer.world.characters
    choice { ... }                                // desugar: layer @layer.ui.game
}
```

Presentation calls that register visible values also choose a target/slot. The
default target is `@target.scene`; `bg(...)` writes
`@slot.background.default`, and `show(@character.alice, ...)` writes
`@slot.character.alice.default`. If a scene needs parallel backgrounds,
reflections, split-screen layers, or multiple copies of a character, the slot
or target must be explicit.

```arcw
let far = bg(@asset.bg.city_far, slot = @slot.background.far)
let near = bg(@asset.bg.city_near, slot = @slot.background.near)

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

HTML/CSS UI は Game Native UI とは別の `HtmlUi` layer として扱う。

```arcw
html panel @ui.settings_html from "ui/settings.html" {
    layer = @layer.ui.html
    bounds = rect(0, 0, 100vw, 100vh)
    input = modal
}
```

Native では Servo、Web では DOM へ渡す。Agent 観測では同じ `LayerNode` と `UiTree` に正規化する。

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

Observation は layer 情報を含む。

```rust
pub struct Observation {
    pub layers: Vec<ObservedLayer>,
    pub objects: Vec<ObservedObject>,
    pub actions: Vec<ActionTarget>,
    // ...
}

pub struct ObservedLayer {
    pub layer: PublicId,
    pub kind: LayerKind,
    pub z: i32,
    pub visible: bool,
    pub input_policy: LayerInputPolicySummary,
    pub bbox: Option<BBox>,
    pub objects: Vec<ObjectId>,
}
```

各 object/action は所属 layer を持つ。

```rust
pub struct ObservedObject {
    pub id: ObjectId,
    pub entity: Option<EntityId>,
    pub layer: LayerId,
    pub bbox: BBox,
    pub polygon: Option<Polygon>,
    pub mask: Option<MaskRef>,
}
```

CLI:

```bash
arcw agent layers
arcw agent observe --layers --objects --image overlay
arcw agent click --layer layer.ui.game --target choice.opening.listen
arcw agent click --x 520 --y 540 --layer layer.ui.modal
```

## Testing

```arcw
test @test.modal_blocks_world_input scenario {
    start(@flow.opening)
    ui.open(@ui.settings_html)

    input.click(@character.alice)

    expect.input(blocked_by=@layer.ui.modal)
    expect.no_event(GameEvent.CharacterClicked)
}
```

```arcw
test #test_choice_layer_bbox visual {
    start(@flow.opening)
    wait.object(@choice.opening.listen, state=.visible)

    assert.layer(@choice.opening.listen, equals=@layer.ui.game)
    assert.bbox(@choice.opening.listen, within=rect(400, 500, 500, 80))
}
```

## Contracts

layer には契約を付けられる。

```arcw
pub layer @layer.ui.modal: Modal
ensures input.route == modal
ensures z > @layer.ui.game.z
{
    z = 3000
    input = modal
}
```

UI component でも所属 layer を保証できる。

```arcw
component ChoiceList(choices: Vec<ChoiceView>) -> View
ensures result.layer == @layer.ui.game
ensures result.actions.all(_.layer == @layer.ui.game)
{
    ...
}
```

## 実装順序

1. `arcweft-layer-core`: `LayerTree`, `LayerNode`, `LayerRenderPolicy`, `LayerInputPolicy`。
2. `arcweft-render`: `RenderSpec.layers` を `LayerTree` に移行。
3. `arcweft-ui-layout`: UI node に layer を付与。
4. `arcweft-agent-observe`: observation に layer / object / action の対応を追加。
5. `arcweft-agent-action`: input routing を layer stack 化。
6. `arcweft-test`: layer assertion と input blocked expectation。
7. `arcweft-lsp`: layer preview、z-order warning、modal/focus diagnostics。

## 固定する設計判断

```text
1. layer は描画・入力・Agent 観測を結びつける一級 Entity。
2. render order と input priority は別指定可能だが、デフォルトは z に従う。
3. 入力は上位 layer から下位 layer へ hit-test し、Consumed / PassThrough / Blocked を返す。
4. Modal layer は hit していない領域でも下位入力をブロックする。
5. UI、HTML、Activity、Debug overlay はすべて layer に載る。
6. object-id pass と mask は layer 情報を持つ。
7. Headless と windowed で同じ LayerTree を使う。
```


## Object hooks on layers

Layer は hook target でもある。描画・layout・input routing の各 phase で hook を実行できる。

```arcw
hook @hook.ui_layer_bbox
on @layer.ui.game
phase AfterLayout
when layer.layout_hash.changed
{
    log.debug("ui layer bbox={bbox:?}", bbox = object.bbox)
}
```

`AfterInputRoute` hook では、入力がどの layer に consumed / blocked / passed-through されたかを検査できる。

```arcw
hook @hook.debug_input_route
on query Layer where input.enabled
phase AfterInputRoute
check on event
{
    log trace "input route: {layer:?} -> {result:?}" {
        layer = object.entity,
        result = route.result,
    }
}
```


## Layer hooks

Layer は hook target になれる。描画、hit-test、入力、Agent 観測の各 phase で hook を実行できる。

```arcw
hook @hook.modal.block_lower_layers
on @layer.modal.settings
at input.capture
priority 1000
check every event
{
    if ctx.layer.visible {
        block_below
    } else {
        continue
    }
}

hook @hook.layer.agent_hint
on @layer.choices
at agent.observe
check when state .affection[@character.alice] changes
{
    patch_agent_observation {
        layer @layer.choices {
            description = "現在表示中の選択肢レイヤー"
        }
    }
}
```

Layer hook は [Hook Runtime / Memoization Runtime](../02-runtime/hooks-memoization.md) で決定的順序に並べられる。

## Layer hooks

Layer は hook の重要な対象である。描画・入力・Agent 観測が同じ LayerTree を共有するため、layer phase に hook を差し込める。

```arcw
hook @hook.modal_blocks_input
on @layer.overlay.modal
phase InputPreRoute
check on event
when layer(@layer.overlay.modal).visible
effects { signal }
{
    signal.set(@signal.input_blocked, true)
}
```

Input routing は以下の hook phase を持つ。

```text
InputPreRoute
  → hit-test
InputHitTest
  → deliver to layer/UI/activity
InputPostRoute
```

詳細は [Object Hooks / Memoization](../01-language/hooks-and-memoization.md) と [Hook Runtime](../02-runtime/hooks-memoization.md) を参照。

## Layer hooks

Layer は hook target になれる。これにより、描画・入力・Agent 観測のタイミングで条件チェックや追加処理を入れられる。

```arcw
hook @hook.modal.blocks_lower_layers
on layer @layer.ui.modal
at before_input
when layer.visible
priority 1000
{
    block_below
}
```

```arcw
hook @hook.debug.layer_observed
on layer @layer.debug.agent
at after_render
check every 30 frames
{
    log.debug("debug layer visible={visible:bool}", visible = layer.visible)
}
```

Layer hook は [Object hooks](../01-language/hooks-and-memoization.md) と [Runtime hooks and memoization](../02-runtime/hooks-memoization.md) で定義される。入力 routing に介入する hook は phase ごとの effect firewall により、許可された `InputBlock` / `InputCapture` / `EmitEvent` のみ実行できる。


## Device and virtual controller sources

Layered input accepts physical and virtual controller sources. See [Device Profiles, Generators, and USB](../03-presentation/device-generator-and-usb.md) and [Virtual Touch Controller](../03-presentation/virtual-controller.md).

```text
USB/HID/Gamepad/Keyboard/Touch
  -> ControllerMap
  -> LayerTree routing
  -> GameEvent / Activity input
```

Virtual controller layers usually sit above scene/UI layers and consume touch input inside their hit regions before lower layers receive it.

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

Controls inside this layer consume only their own hit regions and emit normalized actions. Touches outside controls pass through to lower layers. This allows visual novel UI, minigame input, and mobile controls to coexist.

See [Touch Virtual Controller](touch-virtual-controller.md).


