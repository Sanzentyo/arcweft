# Layered Rendering

描画は `RenderSpec.layers: Vec<LayerSpec>` ではなく、**Layer Tree / Layer Stack** として扱う。レイヤーは描画順だけでなく、hit test、focus、modal、object-id pass、Agent 観測、headless capture の単位でもある。

関連:

- [Object Hooks and Memoization](../01-language/hooks-and-memoization.md)

- [wgpu renderer](wgpu-renderer.md)
- [Layered Input](../02-runtime/layered-input.md)
- [Reactive UI](ui-reactive.md)
- [Agent Debug Bus](../04-tooling/agent-debug-mcp-cli.md)
- [Layer Tree schema](../schemas/layer-tree.md)

## 基本方針

```text
Scene / View / Activity / HTML UI
  ↓
LayerTree
  ↓
RenderPlan
  ├─ color pass
  ├─ object-id pass
  ├─ mask pass
  ├─ postprocess pass
  └─ debug overlay pass
```

レイヤーは以下を持つ。

```rust
pub struct LayerNode {
    pub id: LayerId,
    pub entity: Option<EntityId>,
    pub public_id: Option<PublicId>,
    pub kind: LayerKind,

    pub parent: Option<LayerId>,
    pub children: Vec<LayerId>,

    pub order: LayerOrder,
    pub visibility: VisibilityState,
    pub transform: Transform2D,
    pub opacity: f32,
    pub clip: Option<ClipSpec>,
    pub mask: Option<MaskSpec>,
    pub blend: BlendMode,

    pub render: LayerRenderSpec,
    pub input: LayerInputSpec,
    pub observation: LayerObservationSpec,
}
```

`LayerId` は frame 内だけの一時 ID ではなく、可能なら `EntityId` に紐づく安定 ID を持つ。`say` や `choice`、UI component、Activity、HTML panel はすべて layer へ投影できる。

## LayerKind

```rust
pub enum LayerKind {
    World,
    Background,
    Character,
    Prop,
    Particle,
    Video,
    TextBox,
    Choice,
    NativeUi,
    HtmlUi,
    Activity,
    Modal,
    DebugOverlay,
    AgentOverlay,
    Offscreen,
    Group,
}
```

推奨の標準 stack:

```text
root
  world
    background
    characters
    props
    particles
  activity
  dialogue
    textbox
    choices
  ui
    hud
    menus
    modal
  html
    html_panels
  debug
    debug_overlay
  agent
    agent_overlay
```

## LayerOrder

```rust
pub struct LayerOrder {
    pub z: i32,
    pub sub_z: i32,
    pub stable_tiebreaker: EntityId,
}
```

描画順は `z, sub_z, stable_tiebreaker` で決める。`stable_tiebreaker` を入れることで、同じ z に複数要素がある場合でも replay で順序が安定する。

## LayerRenderSpec

```awft
pub enum LayerRenderSpec {
    Empty,
    Sprite(SpriteSpec),
    Text(TextSpec),
    RichText(RichTextSpec),
    Typeset(TypesetSpec),
    Vector(VectorSpec),
    Ui(UiRenderSpec),
    Html(HtmlPanelRenderSpec),
    Activity(ActivityRenderSpec),
    CustomShader(CustomMaterialSpec),
    Group,
}
```

`Group` layer は transform、opacity、clip、mask、input policy をまとめる。必要に応じて offscreen target へ描画し、shader や blend を適用する。

## Stacking context

次の条件を持つ layer は stacking context を作る。

```text
- opacity < 1.0
- clip / mask がある
- blend mode が通常でない
- custom shader がある
- postprocess がある
- render_target = offscreen
- HTML/Servo/DOM layer
```

`RenderPlan` は stacking context ごとに pass を作る。

```rust
pub struct RenderPlan {
    pub passes: Vec<RenderPassNode>,
    pub final_composite: CompositePass,
}

pub struct RenderPassNode {
    pub id: RenderPassId,
    pub target: RenderTargetSpec,
    pub layers: Vec<LayerId>,
    pub clear: Option<Color>,
    pub object_id: bool,
    pub debug_labels: Vec<String>,
}
```

## Object ID pass と mask

Layer Tree は object-id pass の入力でもある。

```rust
pub struct ObjectIdMapping {
    pub object_id: u32,
    pub layer: LayerId,
    pub entity: Option<EntityId>,
    pub kind: ObservedKind,
    pub public_id: Option<PublicId>,
}
```

`object-id` pass は visible layer だけを描画する。clip、mask、opacity、z-order の結果を反映するため、bbox や segmentation mask は画面上の実際の見え方に近い。

## Headless

headless でも Layer Tree を同じように構築する。

```text
headless:
  Engine::step
  → LayerTree
  → RenderPlan
  → offscreen wgpu texture
  → color/object-id/mask readback
  → Observation
```

HTML/Servo/DOM layer が完全に描画できない環境では、`HtmlPanelSpec` と UI bridge metadata から `UiLayoutApprox` を作る。Observation には信頼度を入れる。

```rust
pub enum ObservationSource {
    EngineExact,
    RenderObjectIdPass,
    UiLayoutExact,
    UiBridgeApprox,
    PixelDerived,
    BackendUnavailable,
}
```

## DSL: layer 宣言

```awft
layer @layer.world: World {
    z = 0
    input = passthrough
}

layer @layer.dialogue: Group {
    z = 100
    input = hit_test
}

layer @layer.modal: Modal {
    z = 1000
    input = modal
}
```

Scene 内で layer を使う。

```awft
scene.show(@scene.opening)
scope {
    layer @layer.background {
        image @asset.bg.room fit cover
    }

    layer @layer.characters {
        sprite @asset.char.alice.default at center
    }

    layer @layer.dialogue {
        TextBox(current_text())
    }

    layer @layer.choices {
        ChoiceList(choices)
    }
}
```

短く書く場合:

```awft
scene.show(@scene.opening)
scope {
    background layer @layer.background image(@asset.bg.room)
    character layer @layer.characters sprite(@asset.char.alice.default).at(center)
    ui layer @layer.dialogue TextBox(current_text())
}
```

## UI component との関係

Game Native UI component は内部的に layer subtree を生成する。

```awft
component ChoiceList(choices: Vec<ChoiceView>) -> View {
    VStack {
        ForEach(choices, id = _.id) |choice| {
            ChoiceButton(choice)
                .layer(@layer.choices)
                .agent_target(choice.id)
        }
    }
}
```

UI component の `.layer(...)` は描画先 layer と入力 policy を決める。指定しない場合は親 component の layer を継承する。

## Activity layer

Rust/WASM/外部 process の Activity も layer を持つ。

```awft
activity @activity.truck_game TruckGame {
    layer @layer.activity.truck {
        z = 50
        input = capture_when_active
        render = custom_3d
    }
}
```

Activity が portable render command を返す場合、その command は該当 layer に積まれる。trusted direct path でも、Agent 観測用には layer metadata を必須にする。

## HTML/Servo/DOM layer

HTML UI は `HtmlUi` layer として扱う。

```awft
html panel @ui.settings_html from "ui/settings.html" {
    layer @layer.html.settings
    bounds = rect(0, 0, 100vw, 100vh)
    input = modal
}
```

Native では Servo、Web では DOM に差し替えるが、Layer Tree 上では同じ `HtmlUi` layer になる。

## Shader と layer

layer 単位で shader を適用できる。

```awft
layer @layer.dialogue {
    TextBox(current_text())
}
.shader(@shader.ui.glass_panel) {
    blur_amount = 12.0
}
```

Group layer に shader を付けると、その subtree を offscreen target へ描画してから shader を適用する。

## Contracts

Layer にも契約を持てる。

```awft
layer @layer.modal: Modal
ensures input.blocks_lower_layers
ensures z > layer(@layer.dialogue).z
{
    ...
}
```

UI component の contract と組み合わせる。

```awft
component ChoiceButton(choice: ChoiceView) -> View
ensures result.layer.input.accepts_click
ensures result.has_action("select")
{
    ...
}
```

## Test

```awft
test @test.layer_order_opening visual {
    start @flow.opening

    assert layer @layer.background below @layer.characters
    assert layer @layer.choices above @layer.dialogue
    assert object @choice.opening.listen in_layer @layer.choices
}
```


## Hook との統合

Layer は hook 対象である。描画・入力・layout・Agent 観測の各 phase に hook を付けられる。

```awft
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
check on change layout
{
    log.debug("choices layer layout changed")
}
```

入力 routing では hook の `InputDisposition` が routing 結果に影響する。Modal、pointer capture、debug overlay、Agent overlay はこの仕組みで共通化される。

