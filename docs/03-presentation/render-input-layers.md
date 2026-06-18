# Render / Input Layer System

描画システムには、単なる描画順ではなく、**入力ルーティング・hit-test・focus・modal・Agent観測・mask生成・デバッグ**まで含む `LayerTree` を導入する。

これにより、`bg`、立ち絵、テキストボックス、選択肢、Game Native UI、HTML/Servo/DOM UI、modal、debug overlay、Agent overlay を同じ概念で扱える。

関連:

- [Object Hooks and Memoization](../01-language/hooks-and-memoization.md)

- [wgpu renderer](wgpu-renderer.md)
- [Game Native UI](ui-reactive.md)
- [HTML / Servo / DOM UI](html-servo-dom.md)
- [Agent Debug Bus](../04-tooling/agent-debug-mcp-cli.md)
- [grammar](../01-language/grammar.md)

---

## 1. 設計原則

```text
Layer = Render + Input + HitTest + Debug metadata の単位
```

従来の `RenderSpec.layers` を `LayerTree` に拡張する。

```rust
pub struct RenderSpec {
    pub size: UVec2,
    pub clear: Color,
    pub layer_tree: LayerTree,
    pub layer_contents: IndexMap<LayerId, LayerContent>,
    pub postprocess: Vec<ShaderPassSpec>,
}
```

`LayerTree` は以下を満たす。

```text
1. 描画順を安定に決める
2. 入力を top-most layer から順に routing する
3. layer ごとに hit-test と action target を出す
4. modal / focus / pointer capture / keyboard scope を持つ
5. Agent Debug Bus へ bbox / polygon / mask / z-order を出す
6. headless でも同じ routing ができる
7. Servo/DOM HTML UI も tree 上の layer として扱う
```

---

## 2. LayerTree 型

```rust
pub struct LayerTree {
    pub root: LayerId,
    pub layers: IndexMap<LayerId, LayerNode>,
    pub render_order: Vec<LayerId>,
    pub input_order: Vec<LayerId>,
    pub routing_hash: RoutingHash,
}

pub struct LayerNode {
    pub parent: Option<LayerId>,
    pub children: Vec<LayerId>,
    pub spec: LayerSpec,
}

pub struct LayerSpec {
    pub id: LayerId,
    pub entity: Option<EntityId>,
    pub public_id: Option<PublicId>,
    pub root_input_policy: RootInputPolicy,
    pub composition: StackCompositionPolicy,

    pub kind: LayerKind,
    pub order: LayerOrder,
    pub visibility: LayerVisibility,
    pub transform: LayerTransform,

    pub render: LayerRenderSpec,
    pub input: LayerInputSpec,
    pub hit_test: LayerHitTestSpec,
    pub debug: LayerDebugSpec,
}
```

`LayerId` は frame 内で安定し、可能なら `EntityId` と結びつける。

```rust
pub struct LayerOrder {
    pub z: i32,
    pub stable_index: u32,
    pub phase: RenderPhase,
}

pub enum RenderPhase {
    Background,
    World,
    Characters,
    Effects,
    Dialogue,
    GameUi,
    HtmlUi,
    Modal,
    Debug,
    AgentOverlay,
}
```

描画順は以下で決定する。

```text
phase → z → stable_index
```

`stable_index` は source order ではなく、compiler が EntityId / layout registry から安定生成する。これにより、LLM patch や hot reload で意図しない描画順変化を減らす。

---

## 3. LayerKind

```rust
pub enum LayerKind {
    Background,
    World2D,
    Character,
    Effect,
    Dialogue,
    Choice,
    NativeUi,
    HtmlUi,
    Activity,
    DebugOverlay,
    AgentOverlay,
    Custom,
}
```

代表的なlayer構成:

```text
z=000  Background      背景
z=100  World2D         小物、演出
z=200  Character       立ち絵
z=300  Effect          パーティクル、前景演出
z=500  Dialogue        textbox
z=550  Choice          選択肢
z=700  NativeUi        HUD、設定ショートカット
z=800  HtmlUi          Servo/DOM panel
z=900  Modal           confirmation dialog
z=950  DebugOverlay    debug UI
z=990  AgentOverlay    bbox/label overlay
```

---

## 4. Render target / cache

layer は直接 main target に描くだけでなく、offscreen target を使える。

```rust
pub enum LayerRenderTargetPolicy {
    Main,
    Offscreen {
        size: LayerTargetSize,
        format: TextureFormat,
        cache: LayerCachePolicy,
    },
    CachedStatic,
}

pub enum LayerCachePolicy {
    None,
    UntilInvalidated,
    FixedFrames(u32),
}
```

用途:

```text
背景:
  CachedStatic

テキストボックス:
  UntilInvalidated

postprocess付きUI:
  Offscreen → layer-local shader → compose

Activity/FPS viewport:
  Offscreen → composited into visual novel scene
```

layer-local postprocess:

```rust
pub struct LayerRenderSpec {
    pub content: LayerContentSpec,
    pub target: LayerRenderTargetPolicy,
    pub blend: BlendMode,
    pub clip: Option<ClipSpec>,
    pub mask: Option<MaskSpec>,
    pub shader_passes: Vec<ShaderPassSpec>,
}
```

---

## 5. InputLayer

入力は layer を基準に routing する。

```rust
pub struct LayerInputSpec {
    pub enabled: bool,
    pub policy: InputPolicy,
    pub pointer: PointerInputPolicy,
    pub keyboard: KeyboardInputPolicy,
    pub gamepad: GamepadInputPolicy,
    pub focus: FocusPolicy,
    pub modal: ModalPolicy,
    pub capture: CapturePolicy,
}
```

```rust
pub enum InputPolicy {
    None,
    Passthrough,
    HitTest,
    Capture,
    Modal,
    SemanticOnly,
}
```

意味:

| policy | 意味 |
|---|---|
| `None` | 入力を受けない。背景など。 |
| `Passthrough` | hit しても下のlayerへ流す。装飾overlayなど。 |
| `HitTest` | hit した対象へ入力する。button/choiceなど。 |
| `Capture` | pointer capture中はこのlayerへ送る。drag/sliderなど。 |
| `Modal` | このlayerより下へ入力を流さない。dialog/menuなど。 |
| `SemanticOnly` | 座標入力は受けず、Agent/commandからのsemantic actionだけ受ける。 |

---

## 6. HitTest

hit-test は段階的に行う。

```rust
pub struct LayerHitTestSpec {
    pub source: HitTestSource,
    pub region: Option<HitRegion>,
    pub object_id_pass: bool,
    pub alpha_threshold: Option<f32>,
}

pub enum HitTestSource {
    None,
    LayoutBoxes,
    BBox,
    Polygon,
    Mask,
    ObjectIdPass,
    Custom(EntityId),
}
```

推奨:

```text
Native UI:
  LayoutBoxes / Polygon

TextBox:
  LayoutBoxes

Choice:
  LayoutBoxes + action target

Sprite:
  BBox or Mask

Vector/SVG:
  Polygon or Mask

HTML UI:
  DOM/Servo bridge bbox

Agent overlay:
  Passthrough
```

`ObjectIdPass` を有効にしたlayerは、render時にobject-id textureにも描く。これにより、headlessでもpixel-accurate mask / bbox を生成できる。

---

## 7. 入力ルーティングアルゴリズム

host は OS / browser / agent 由来の raw input をまず正規化する。

```rust
pub struct RawInputEvent {
    pub id: InputEventId,
    pub kind: RawInputKind,
    pub timestamp: HostTimestamp,
    pub viewport: ViewportInfo,
}
```

その後、`InputRouter` が semantic input に変換する。

```rust
pub struct RoutedInputEvent {
    pub raw_id: InputEventId,
    pub target_layer: Option<LayerId>,
    pub target_object: Option<EntityId>,
    pub action: Option<SemanticAction>,
    pub route: InputRoute,
}

pub enum InputRoute {
    Consumed,
    Passthrough,
    BlockedByModal { layer: LayerId },
    Captured { layer: LayerId },
    Focused { layer: LayerId },
    Unhandled,
}
```

Routing 手順:

```text
1. raw input を logical viewport 座標へ変換
2. pointer capture があれば capture layer へ送る
3. modal layer があれば modal より下の layer を無視
4. z の高い layer から順に hit-test
5. policy が Passthrough なら hit 情報だけ記録して続行
6. policy が HitTest/Capture/Modal なら ActionTarget を探す
7. semantic action があれば RoutedInputEvent として core へ渡す
8. 何もなければ Unhandled
```

この処理は windowed と headless で同じ実装を使う。headless では raw pointer の代わりに AgentAction から raw/semantic event を作る。

---

## 8. Focus / modal / capture

```rust
pub struct FocusState {
    pub focused_layer: Option<LayerId>,
    pub focused_object: Option<EntityId>,
    pub focus_scope: Option<FocusScopeId>,
}

pub struct InputCaptureState {
    pub pointer_capture: Option<LayerId>,
    pub keyboard_capture: Option<LayerId>,
    pub reason: CaptureReason,
}
```

Modal の例:

```arcw
layer @layer.confirm_dialog phase Modal z 900 {
    input {
        policy = modal
        pointer = hit_test
        keyboard = focus
        block_below = true
    }

    ui ConfirmDialog(...)
}
```

このlayerが表示中は、下のchoiceやtextboxはclickされない。

---

## 9. DSL 構文

### scene layer

```arcw
scene.show(@scene.opening)
scope {
    layer @layer.bg phase Background z 0 {
        input none
        image @asset.bg.room fit cover
    }

    layer @layer.characters phase Characters z 200 {
        input hit_test passthrough
        show(@character.alice, at = .center)
    }

    layer @layer.dialog phase Dialogue z 500 {
        input hit_test
        TextBox(current_text())
            .agent_target(@ui.textbox.main)
    }

    layer @layer.choices phase Dialogue z 550 {
        input modal_when_visible
        ChoiceList(choices)
    }
}
```

### layer block 詳細

```arcw
layer @layer.settings phase Modal z 900
requires visible => input.policy == .Modal
{
    render {
        target = offscreen(cache = until_invalidated)
        blend = alpha
        postprocess @shader.ui.glass_panel
    }

    input {
        policy = modal
        pointer = hit_test
        keyboard = focus
        gamepad = focus
        block_below = true
    }

    hit_test layout_boxes

    ui SettingsPanel(config = bind state.config)
}
```

### shorthand

```arcw
layer @layer.bg background {
    image @asset.bg.room
}

layer @layer.choice modal {
    ChoiceList(choices)
}
```

shorthand は compiler が `phase`、`z`、`input` を default から補完する。

---

## 10. Layer defaults

project config で default を決める。

```toml
[layers.defaults.background]
order = "background(0)"
z = 0
input = "none"

[layers.defaults.characters]
order = "world(200)"
z = 200
input = "passthrough"
hit_test = "bbox"

[layers.defaults.dialogue]
order = "ui(500)"
z = 500
input = "hit_test"
hit_test = "layout_boxes"

[layers.defaults.modal]
order = "modal(0)"
z = 900
input = "modal"
hit_test = "layout_boxes"
```

---

## 11. UI componentとの接続

Game Native UI component は暗黙に `NativeUi` layer を生成してもよい。

```arcw
component Hud(state: GameState) -> View {
    HStack {
        Button("設定").agent_target(@ui.settings.open)
    }
    .layer(@layer.hud, order = ui(700))
}
```

または scene 側で明示する。

```arcw
layer @layer.hud {
    order = ui(700)
    input = hit_test
    ui Hud(state)
}
```

UI tree の node は所属layerを持つ。

```rust
pub struct UiNode {
    pub layer: LayerId,
    pub entity: Option<EntityId>,
    pub role: UiRole,
    pub bbox: BBox,
    pub actions: Vec<ActionTarget>,
}
```

---

## 12. HTML / Servo / DOM layer

HTML/CSS UI は `HtmlUi` phase の layer として扱う。

```arcw
layer @layer.html_settings phase HtmlUi z 800 {
    input modal
    html_panel @ui.settings_html
}
```

Native:

```text
LayerTree
  → ServoUiHost
  → WebView bbox/action metadata
  → InputRouter
```

Web:

```text
LayerTree
  → Browser DOM overlay
  → DOM bbox/action metadata
  → InputRouter
```

headless では pixel-perfect DOM/Servo capture が利用できない場合でも、`HtmlPanelSpec` と `data-arcweft-*` metadata から semantic action target を返す。

---

## 13. Activity layer

トラックゲームやFPSミニゲームは Activity を layer content として持つ。

```arcw
layer @layer.truck_game phase World z 100 {
    input capture
    activity @activity.truck_game {
        size = fill
        input_map = @input.truck_game
    }
}
```

Activity への入力は layer を通して送る。

```rust
pub enum ActivityInput {
    Pointer(PointerEvent),
    Keyboard(KeyEvent),
    Gamepad(GamepadEvent),
    Semantic(SemanticAction),
}
```

Activity が modal/capture を要求する場合:

```arcw
activity @activity.fps_arena {
    input_layer {
        policy = capture
        pointer_capture = true
        keyboard_capture = true
        block_below = true
    }
}
```

---

## 14. Agent Debug Bus との統合

Observation には layer 情報を入れる。

```rust
pub struct Observation {
    pub layers: Vec<LayerObservation>,
    pub objects: Vec<ObservedObject>,
    pub actions: Vec<ActionTarget>,
    // existing fields...
}

pub struct LayerObservation {
    pub layer_id: LayerId,
    pub public_id: Option<PublicId>,
    pub phase: RenderPhase,
    pub z: i32,
    pub visible: bool,
    pub input_policy: InputPolicy,
    pub modal: bool,
    pub bbox: Option<BBox>,
    pub object_count: u32,
}
```

LLM向け説明:

```text
Layers top-to-bottom:
  1. layer.settings_modal z=900 policy=modal visible=true
  2. layer.hud z=700 policy=hit_test visible=true
  3. layer.choices z=550 policy=hit_test visible=false
  4. layer.dialog z=500 policy=hit_test visible=true
  5. layer.characters z=200 policy=passthrough visible=true
  6. layer.bg z=0 policy=none visible=true

Current modal layer blocks lower input: layer.settings_modal
```

Agent は `click target` のほか、layer 指定 click もできる。

```bash
arcw agent click --target choice.opening.listen
arcw agent click --layer layer.dialog --x 640 --y 620
arcw agent layers --json
```

---

## 15. Test / assert / signal

layer 状態は test と signal に使える。

```arcw
pub signal @signal.active_modal_layer: Watch<Option<Ref<Layer>>>
pub signal @signal.focused_layer: Watch<Option<Ref<Layer>>>
```

visual test:

```arcw
test @test.settings_blocks_choice visual {
    start(@flow.opening)
    invoke(@ui.settings.open)

    expect.layer(@layer.settings, state=.modal_visible)
    expect.layer(@layer.choices, blocked_by=@layer.settings)

    input.click(@choice.opening.listen)
    expect.no_event(GameEvent::ChoiceSelected)
}
```

assert:

```arcw
assert(layer(@layer.settings).input.policy == .Modal)
assert(no_layer_overlap_interactive(@layer.modal, @layer.debug_overlay))
```

---

## 16. 契約

layer には契約を付けられる。

```arcw
layer @layer.choices phase Dialogue z 550
requires choices.len() > 0
ensures visible => input.policy != .None
ensures visible => actions.len() == choices.len()
{
    input hit_test
    ChoiceList(choices)
}
```

Modal 契約:

```arcw
contract modal_layer(layer: LayerSpec) {
    requires layer.input.policy == .Modal
    ensures layer.input.modal.block_below == true
    ensures layer.order.kind == .Modal
}
```

---

## 17. crate 追加

```text
arcweft-layer-core
  LayerId, LayerSpec, LayerTree, LayerNode, LayerOrder

arcweft-layer-render
  LayerRenderTargetPolicy, layer composition, layer cache

arcweft-layer-input
  InputRouter, hit-test, focus, modal, capture

arcweft-layer-agent
  LayerObservation, layer-aware bbox/mask/action target

arcweft-layer-lsp
  layer preview, z-order diagnostics, input routing diagnostics
```

`arcweft-render`、`arcweft-ui-core`、`arcweft-agent-observe` は `arcweft-layer-core` に依存する。

---

## 18. 実装順

1. `LayerTree` / `LayerNode` / `LayerSpec` を `RenderSpec` に入れる。
2. `InputRouter` を作り、semantic action と pointer hit-test を layer top-down に統一する。
3. Game Native UI node に所属layerを持たせる。
4. Object ID pass を layer/object ID と結びつける。
5. Agent Observation に `layers` を追加する。
6. Modal/focus/capture を導入する。
7. HTML/Servo/DOM layer bridge をつなぐ。
8. Activity layer content をつなぐ。
9. LSP/visual test/contract を追加する。

---

## 19. 固定する設計判断

```text
1. Layer は描画だけでなく input routing の基本単位にする。
2. RenderSpec は layers ではなく LayerTree を持つ。
3. 入力は top-most layer から routing する。
4. modal/focus/capture は layer state として管理する。
5. Agent bbox/mask/action target は layer 情報を必ず持つ。
6. HTML/Servo/DOM UI も LayerTree 上の HtmlUi layer として扱う。
7. Activity も layer content として扱う。
8. headless と windowed で同じ InputRouter を使う。
9. `#<...>.method` のように境界が必要な参照は従来通り `#<...>` を使う。
10. visual test は layer 単位で visible / blocked / hit-test を検査できる。
```


## Hook との統合

Layer は hook 対象である。描画・入力・layout・Agent 観測の各 phase に hook を付けられる。

```arcw
layer @layer.choices: Choice {
    z = 550
    input = hit_test
    hit_test = ui_layout
}

hook @hook.layer.choices.pointer_enter
on @layer.choices
phase InputTarget
check on input PointerEnter
{
    signal.set(@signal.hovered_layer, Some(@layer.choices))
}

hook @hook.layer.choices.layout_changed
on @layer.choices
phase AfterLayout
when layout.changed
{
    log.debug("choices layer layout changed")
}
```

入力 routing では hook の `InputDisposition` が routing 結果に影響する。Modal、pointer capture、debug overlay、Agent overlay はこの仕組みで共通化される。

