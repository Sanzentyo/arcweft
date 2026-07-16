# Layer System / Layer-based Input

描画と入力は、同じ `LayerTree` を基準に扱う。レイヤーは単なる z-index ではなく、描画、hit-test、入力伝播、Agent 観測、mask 生成、テスト、accessibility、HTML/Servo/DOM View の合成境界を表す一級の概念である。

関連章:

- [wgpu renderer](wgpu-renderer.md)
- [Game Native View](view-reactive.md)
- [Agent Debug Bus](../04-tooling/agent-debug-mcp-cli.md)
- [Core runtime](../02-runtime/core.md)
- [WGSL shader](wgsl-shaders.md)

## 基本方針

```text
RuntimeStepOutput
  ├─ layer_tree: LayerTree
  ├─ render_graph: RenderGraph
  ├─ input_routing: InputRoutingTable
  └─ observations: Object/View/Layer metadata
```

入力は「前回 commit された `LayerTree`」を使って host 側で routing し、その結果を次 tick の `RuntimeStepInput` に入れる。これにより、headless / native / web / replay で同じ入力解決ができる。

```text
tick N output:
  LayerTree + InputRoutingTable

raw pointer/key input between N and N+1:
  route using committed LayerTree(N)

tick N+1 input:
  LayerInputEvent / SemanticInputEvent
```

raw input だけを記録すると View layout 差で replay が壊れる可能性があるため、replay には原則として routed event を保存する。必要なら raw input と `routing_hash` も保存する。

### Wheel input units

Native と Web の wheel 入力は host ごとに係数を持たない。platform の
line delta または physical-pixel delta を `arcweft-player-scene` の共有境界へ
渡し、そこで logical pixel へ正規化する。line delta は Arcweft の既定 policy
として 1 line = 32 logical pixels、physical pixel は window scale factor で
除算する。非有限値、0 以下の scale factor、logical `f32` 範囲外の値は入力
エラーであり、0、clamp、saturating cast へ黙示変換しない。

## LayerTree

```rust
pub struct LayerTree {
    pub root: LayerId,
    pub layers: IndexMap<LayerId, LayerNode>,
    pub render_order: Vec<LayerId>,
    pub input_order: Vec<LayerId>,
    pub routing_hash: RoutingHash,
}

pub struct LayerNode {
    pub id: LayerId,
    pub public_id: Option<PublicId>,
    pub kind: LayerKind,
    pub parent: Option<LayerId>,
    pub children: Vec<LayerId>,

    pub order: LayerOrder,
    pub visibility: LayerVisibility,
    pub opacity: f32,
    pub transform: Transform2D,
    pub clip: Option<ClipSpec>,
    pub mask: Option<MaskSpec>,
    pub blend: BlendMode,
    pub render_target: RenderTargetPolicy,

    pub input: LayerInputPolicy,
    pub capture: LayerCapturePolicy,
    pub accessibility: LayerAccessibility,

    pub debug_name: SmolStr,
    pub source: Option<SourceAnchor>,
}
```

## LayerKind

```rust
pub enum LayerKind {
    Root,
    Background,
    World2D,
    Character,
    Effects,
    View,
    GameView,
    HtmlView,
    Activity,
    Modal,
    Overlay,
    Debug,
    Agent,
    Offscreen,
}
```

代表的なデフォルト構成:

```text
root
  background
  world
  characters
  effects
  dialogue
  game_view
  html_view
  modal
  debug_overlay
  agent_overlay
```

`modal` は入力を遮断しやすい。`debug_overlay` や `agent_overlay` は描画されても入力を通す設定にできる。

## RenderSpec の更新

従来の `RenderSpec.layers: Vec<LayerSpec>` は、`LayerTree` と layer ごとの content に分ける。

```arcw
pub struct RenderSpec {
    pub size: UVec2,
    pub clear: Color,
    pub layers: LayerTree,
    pub contents: IndexMap<LayerId, LayerContent>,
    pub postprocess: Vec<ShaderPassSpec>,
}

pub enum LayerContent {
    Empty,
    Sprites(Vec<SpriteSpec>),
    Text(Vec<TextSpec>),
    Vector(Vec<VectorSpec>),
    View(ViewRenderSpec),
    Html(HtmlPanelRenderSpec),
    Activity(ActivityRenderSpec),
    Group(Vec<LayerContent>),
    CustomShader(CustomMaterialSpec),
}
```

`LayerNode` が表示・合成・入力を定義し、`LayerContent` が実際の描画内容を持つ。

## LayerOrder

```rust
pub struct LayerOrder {
    pub phase: RenderPhase,
    pub z: i32,
    pub stable_tiebreaker: EntityId,
}

pub enum RenderPhase {
    Background,
    World,
    Foreground,
    View,
    Modal,
    Debug,
    Agent,
}
```

sort は決定的でなければならない。

```text
phase → z → stable_tiebreaker
```

同じ z に複数 layer があっても、`EntityId` により安定順序になる。これにより replay と screenshot diff が安定する。

## RenderTargetPolicy

```rust
pub enum RenderTargetPolicy {
    Direct,
    Offscreen {
        format: TextureFormatSpec,
        clear: Option<Color>,
        postprocess: Vec<ShaderPassSpec>,
        readback_allowed: bool,
    },
    Inherit,
}
```

用途:

```text
Direct:
  通常の合成。

Offscreen:
  layer 単位の blur、mask、transition、shader、readback、visual test。

Inherit:
  親 layer の render target へ描画。
```

例:

```arcw
layer @layer.view.glass_modal: Modal {
    order = view.modal(100)
    render_target = offscreen(format = rgba16f) {
        postprocess @shader.view.glass_blur
    }
}
```

## Input routing table

入力は `input_order` を上から下へ走査する。

```rust
pub struct InputRoutingTable {
    pub layers: Vec<InputLayerEntry>,
    pub hit_regions: Vec<HitRegion>,
    pub focus: Option<FocusTarget>,
    pub active_gestures: Vec<GestureState>,
    pub routing_hash: RoutingHash,
}

pub struct InputLayerEntry {
    pub layer: LayerId,
    pub policy: LayerInputPolicy,
    pub enabled: bool,
    pub visible: bool,
    pub z_order_rank: u32,
}
```

`visible = false` の layer はデフォルトでは入力も受けない。ただし screen reader や hidden-but-focusable は明示的に扱う。

## LayerInputPolicy

```rust
pub enum LayerInputPolicy {
    None,

    /// 入力を受けない。下の layer へ通す。
    PassThrough,

    /// hit した target だけ受ける。hit しなければ下へ通す。
    HitTest,

    /// hit した場合は受け、イベントを消費する。
    CaptureOnHit,

    /// layer 全体が入力を受け、下の layer へ通さない。
    CaptureAll,

    /// modal。下の layer を遮断するが、明示的な escape/cancel は通せる。
    Modal {
        allow_escape: bool,
        dismiss_on_outside_click: bool,
    },

    /// デバッグ/Agent overlay 用。観測やbboxは出すが入力は通す。
    ObserveOnly,
}
```

標準設定:

```text
background:     PassThrough
world:          HitTest
characters:     HitTest or PassThrough
dialogue:       CaptureOnHit
choice/view:    CaptureOnHit
html_view:      CaptureOnHit or Modal
modal:          Modal
debug_overlay:  ObserveOnly
agent_overlay:  ObserveOnly or CaptureOnHit
```

## HitRegion

```rust
pub struct HitRegion {
    pub id: HitRegionId,
    pub layer: LayerId,
    pub target: EntityId,
    pub role: ViewRole,
    pub enabled: bool,
    pub visible: bool,
    pub priority: i32,
    pub bbox: BBox,
    pub polygon: Option<Polygon>,
    pub mask: Option<MaskRef>,
    pub actions: Vec<ActionTarget>,
    pub source: HitRegionSource,
}

pub enum HitRegionSource {
    ViewLayout,
    TextLayout,
    SpriteBounds,
    VectorPath,
    ObjectIdPass,
    HtmlBridge,
    Manual,
}
```

hit-test は次の順で行う。

```text
1. layer input_order 上位から走査
2. layer policy を見る
3. bbox で粗判定
4. polygon があれば polygon 判定
5. mask があれば mask 判定
6. priority / z / stable id で target を決定
7. capture / bubble policy に従い event 生成
```

## Event propagation

DOM 風の capture/target/bubble を持つが、決定性のため簡略化する。

```rust
pub enum InputPhase {
    Capture,
    Target,
    Bubble,
}

pub struct LayerInputEvent {
    pub tick: TickId,
    pub raw: RawInputSummary,
    pub layer: LayerId,
    pub target: Option<EntityId>,
    pub phase: InputPhase,
    pub kind: LayerInputKind,
    pub consumed: bool,
    pub routing_hash: RoutingHash,
}
```

基本ルール:

```text
- pointer/touch は hit target に配送。
- keyboard は focused target に配送。
- gamepad は focused layer または active input scope に配送。
- Modal layer が active の間、下位 layer への pointer/key を遮断。
- Escape/Back は modal policy により dismiss/cancel へ変換可能。
```

## InputScope

Flow や Activity ごとに入力 scope を切り替えられる。

```rust
pub struct InputScope {
    pub id: EntityId,
    pub layer: LayerId,
    pub accepts: EnumSet<InputKind>,
    pub priority: i32,
    pub bindings: Vec<InputBinding>,
}
```

例:

```arcw
input_scope @input.opening on layer @layer.dialogue {
    key Enter => action AdvanceText
    key Space => action AdvanceText
    gamepad South => action AdvanceText
}

input_scope @input.choice on layer @layer.choice_view {
    pointer click target Ref<ChoiceOption> => action SelectChoice(target)
    key Up => action MoveChoice(-1)
    key Down => action MoveChoice(1)
    key Enter => action ConfirmChoice
}
```

## DSL syntax

### layer 宣言

```arcw
layer @layer.background: Background {
    order = background(0)
    input = pass_through
}

layer @layer.characters: Character {
    order = world(20)
    input = hit_test
}

layer @layer.dialogue: View {
    order = view(10)
    input = capture_on_hit
}

layer @layer.choice_view: GameView {
    order = view(20)
    input = capture_on_hit
}

layer @layer.modal: Modal {
    order = modal(0)
    input = modal(allow_escape = true, dismiss_on_outside_click = false)
}
```

### scene 内の layer 使用

```arcw
scene.show(@scene.opening)
scope {
    layer @layer.background {
        image(
            asset = @asset:.bg.room,
            id = "image.scene.room",
            target = "target.scene",
            layer = "layer.background",
            x = 0px,
            y = 0px,
            width = 1280px,
            height = 720px,
            fit = "cover"
        )
    }

    layer @layer.characters {
        sprite(@asset:.char.alice.default)
            .at(center)
            .agent_target(@character.alice)
    }

    layer @layer.dialogue {
        view(@view.MainDialogue)
            .agent_target(@view.MainDialogue)
    }

    layer @layer.choice_view if choices_visible {
        ChoiceList(choices)
    }
}
```

### View での layer 指定

```arcw
ChoiceList(choices)
    .layer(@layer.choice_view)
    .input_policy(capture_on_hit)
```

### modal

```arcw
if state.view.settings_open {
    layer @layer.modal {
        SettingsPanel(config = bind state.config)
            .agent_target(@view.settings)
    }
}
```

## Layer-based input lowering

View widget の `on_click` は、hit region と semantic action へ lowering される。

```arcw
Button("閉じる")
    .agent_target(@view.settings.close)
    .on_click { action.invoke(@action.settings.close) }
```

lowering:

```text
ViewNode
  → HitRegion(layer = layer.modal, target = view.settings.close)
  → ActionTarget(kind = invoke, action = close)
  → LayerInputEvent on click
  → SemanticAction::Invoke(@action.settings.close)
```

## RuntimeStepInput の更新

```rust
pub struct RuntimeStepInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
    pub raw_input_events: Vec<RawInputEvent>,
    pub layer_input_events: Vec<LayerInputEvent>,
    pub semantic_input_events: Vec<SemanticInputEvent>,
    pub task_events: Vec<TaskEvent>,
    pub audio_events: Vec<AudioEvent>,
}
```

通常ゲームロジックは `semantic_input_events` を見る。低レベルデバッグや独自 Activity は必要に応じて `layer_input_events` を見る。

## Replay

replay には routed event を記録する。

```rust
pub struct RecordedInputEvent {
    pub tick: TickId,
    pub raw: Option<RawInputSummary>,
    pub routed: LayerInputEvent,
    pub semantic: Option<SemanticInputEvent>,
    pub routing_hash: RoutingHash,
}
```

検証ルール:

```text
- replay 時、現在の LayerTree routing_hash と記録値を比較。
- 一致すれば semantic event をそのまま注入。
- 不一致なら warning を出し、必要なら raw input の再routingを試す。
```

## Agent / MCP integration

Observation に layer 情報を追加する。

```rust
pub struct Observation {
    pub layers: Vec<ObservedLayer>,
    pub objects: Vec<ObservedObject>,
    pub actions: Vec<ActionTarget>,
    // ...
}

pub struct ObservedLayer {
    pub id: LayerId,
    pub public_id: Option<PublicId>,
    pub kind: LayerKind,
    pub visible: bool,
    pub input_policy: LayerInputPolicy,
    pub bbox: Option<BBox>,
    pub object_count: usize,
    pub action_count: usize,
}
```

CLI:

```bash
arcw agent layers --json
arcw agent observe --layers --objects --image overlay
arcw agent click --layer layer.choice_view --target choice.opening.listen
arcw agent hit-test --x 520 --y 540 --json
arcw agent input-trace --since tick:120
```

MCP tools:

```text
arcweft.layers
arcweft.hit_test
arcweft.click_layer
arcweft.input_trace
```

## Headless behavior

headless でも layer は完全に機能する。

```text
- RenderSpec から LayerTree を構築。
- offscreen render で color/object-id を出す。
- ViewLayout / Vector / Text / ObjectIdPass から HitRegion を構築。
- raw input を仮想 viewport 座標で route。
- semantic action は座標を使わず target/entity に直接配送。
```

Servo/DOM layer は headless で実ピクセルが取れない場合でも、`HtmlPanelSpec` と bridge metadata から `ViewTree` と hit region を返す。正確な pixel capture が必要な場合は browser-attached / Servo-offscreen mode を使う。

## Accessibility / focus

focus は layer と target を持つ。

```rust
pub struct FocusTarget {
    pub layer: LayerId,
    pub target: EntityId,
    pub focus_order: u32,
}
```

keyboard/gamepad は原則として focused layer に配送される。modal がある場合、focus は modal 内に trap される。

```arcw
FocusPolicy::TrapWithin(@layer.modal)
```

screen reader 用には `ViewNode` と `LayerTree` から accessibility tree を生成する。

## Security / product flags

製品版で Agent 入力を許す場合、layer 単位の capability を持たせる。

```rust
pub struct LayerSecurityPolicy {
    pub agent_observable: bool,
    pub agent_clickable: bool,
    pub expose_text: bool,
    pub expose_masks: bool,
}
```

例:

```text
dialogue View / choice:
  observable=true, clickable=true

save data debug panel:
  observable=false in product

debug overlay:
  observable=true only debug token
```

## Tests

```arcw
test @test.choice_layer_receives_input scenario {
    goto @flow.opening
    wait.object(@choice.opening.listen, state=.visible)

    let hit = hit_test(x = 520, y = 540)
    assert_eq hit.layer, @layer.choice_view
    assert_eq hit.target, Some(@choice.opening.listen)

    input.click(layer=@layer.choice_view, target=@choice.opening.listen)
    expect.event(GameEvent::ChoiceSelected, id=@choice.opening.listen)
}
```

visual:

```arcw
test @test.layer_order_opening visual {
    goto @flow.opening
    capture.image(.overlay, path="opening_layers.png")
    assert.layer_above(@layer.dialogue, @layer.characters)
    assert.layer_input_policy(@layer.choice_view, .capture_on_hit)
}
```

## Implementation steps

1. `arcweft-layer-core`: `LayerId`, `LayerTree`, `LayerNode`, `LayerOrder`, `LayerInputPolicy`。
2. `arcweft-layer-render`: `RenderSpec` を `LayerTree + LayerContent` に更新し、render ordering を実装。
3. `arcweft-layer-input`: hit-test、InputRoutingTable、LayerInputEvent、focus、modal trap。
4. `arcweft-view-render`: View layout から HitRegion を生成。
5. `arcweft-agent-observe`: Observation に layers と hit-test endpoint を追加。
6. `arcweft-test`: layer assertion / hit-test assertion を追加。
7. `arcweft-replay`: routed input + routing_hash を記録。

## Design decisions

```text
1. レイヤーは描画と入力の共通基盤にする。
2. z-order と input-order は同じ LayerTree から決定するが、必要なら別順序にできる。
3. 入力 routing は前回 commit 済み LayerTree に基づく。
4. game logic は raw input ではなく semantic/layer input を扱う。
5. modal/focus/gesture は layer 単位で扱う。
6. headless でも LayerTree、HitRegion、semantic action は同じように機能する。
7. replay は routed event と routing_hash を保存する。
8. Agent/MCP/test は layer と hit region を共通利用する。
```

## Layer-owned routing

Modal、focus、capture は LayerTree/InputRouter が所有し、target-specific input
は View/Activity の local handler が処理する。Agent 観測と render-pass
diagnostics は同じ routing/render trace を read-only に参照する。詳細は
[Event Ownership and Caching](../01-language/hooks-and-memoization.md) を参照。


## Local handlers and layer traces

Layer declaration owns routing policy; target View nodes own interaction.

```arcw
layer @layer.choices: Choice {
    z = 550
    input = hit_test
    hit_test = view_layout
}
```

入力 routing の `InputDisposition` と layout change は typed trace に記録され、
test / Agent / logging が直接検査する。

## Retained hit-test inventory

入力 routing の hit regions は committed LayerTree と View layout inventory から
InputRouter が導出する。再利用する場合も owner が `LayerTreeHash` と layout
revision を typed key として管理し、author source に cache key を書かせない。

Layer change と routing result は `LayerTreeHash` と一緒に replay trace に記録
できる。これにより、同じ frame に同じ変化が起きたかを検査できる。

関連:

- [Event Ownership and Caching](../01-language/hooks-and-memoization.md)
- [Runtime Dispatch and Caches](../02-runtime/hooks-memoization.md)


## Device and virtual controller sources

Layered input accepts physical and virtual controller sources. See [Device Profiles, Generators, and USB](../03-presentation/device-generator-and-usb.md) and [Virtual Touch Controller](../03-presentation/virtual-controller.md).

```text
USB/HID/Gamepad/Keyboard/Touch
  -> ControllerMap
  -> LayerTree routing
  -> GameEvent / Activity input
```

Virtual controller layers usually sit above scene/View layers and consume touch input inside their hit regions before lower layers receive it.


