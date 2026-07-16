# Layered Input

Input は描画と同じ Layer Tree を基準に routing する。これにより、見えている View、modal、HTML/Servo/DOM panel、Activity、Debug overlay、Agent 操作を同じ規則で扱える。

関連:

- [Object Hooks and Memoization](../01-language/hooks-and-memoization.md)

- [Layered Rendering](../03-presentation/layered-rendering.md)
- [Sans I/O Core](core.md)
- [Agent Debug Bus](../04-tooling/agent-debug-mcp-cli.md)
- [Hooks and Memoization](../01-language/hooks-and-memoization.md)
- [Layer Tree schema](../schemas/layer-tree.md)

## 基本方針

```text
RawInputEvent
  ↓
InputRouter
  ↓ uses LayerTree + ViewTree + HitRegions
RoutedInputEvent
  ↓
Engine::step
```

`arcweft-core` は OS/window/browser 由来の raw event を直接解釈しない。host が `InputRouter` で layer routing し、`RoutedInputEvent` として渡す。

```rust
pub struct RuntimeStepInput {
    pub tick: TickId,
    pub dt: LogicalDuration,
    pub input: LayeredInputFrame,
    pub task_events: Vec<TaskEvent>,
    pub audio_events: Vec<AudioEvent>,
}
```

## LayeredInputFrame

```rust
pub struct LayeredInputFrame {
    pub raw: Vec<RawInputEvent>,
    pub routed: Vec<RoutedInputEvent>,
    pub focus: FocusState,
    pub captures: Vec<InputCapture>,
}
```

replay では raw と routed の両方を保存できる。

```text
raw only replay:
  LayerTree が同じなら route を再計算

routed replay:
  LayerTree 差分に影響されず、当時の操作を再現
```

通常の決定性テストでは raw + layer hash + routed result を記録し、再計算結果が一致するか検査する。

## RawInputEvent

```rust
pub enum RawInputEvent {
    PointerDown(PointerEvent),
    PointerMove(PointerEvent),
    PointerUp(PointerEvent),
    Wheel(WheelEvent),
    KeyDown(KeyEvent),
    KeyUp(KeyEvent),
    TextInput(TextInputEvent),
    Ime(ImeEvent),
    Gamepad(GamepadEvent),
    Touch(TouchEvent),
    Agent(AgentInputEvent),
}
```

`Agent` は semantic action でも physical action でもよい。

## LayerInputSpec

```rust
pub struct LayerInputSpec {
    pub policy: InputPolicy,
    pub hit_test: HitTestPolicy,
    pub focus: FocusPolicy,
    pub capture: CapturePolicy,
    pub keyboard: KeyboardPolicy,
    pub priority: InputPriority,
}
```

### InputPolicy

```rust
pub enum InputPolicy {
    Disabled,
    Passthrough,
    HitTest,
    CaptureWhenActive,
    Modal,
    ConsumeAll,
    SemanticOnly,
}
```

意味:

```text
Disabled:
  入力を受けない。

Passthrough:
  自分は入力を受けず、下位 layer に渡す。

HitTest:
  hit region に当たった場合だけ受ける。

CaptureWhenActive:
  Activity や drag 中の layer が pointer capture する。

Modal:
  自分より下位 layer への入力を遮断する。
  hit しない領域も modal backdrop として扱う。

ConsumeAll:
  debug overlay や blocking loading layer が全入力を消費する。

SemanticOnly:
  座標入力は受けず、Agent/Command 由来の semantic action だけ受ける。
```

## HitTestPolicy

```rust
pub enum HitTestPolicy {
    None,
    BBox,
    Polygon,
    Mask,
    ViewLayout,
    Custom(EntityId),
}
```

`Mask` は object-id pass または View hit mask を使う。`Custom` は Activity や complex View が提供する hit test 関数を使う。

## Routing order

入力 routing は描画順の逆を基本にする。ただし input policy が優先する。

```text
1. active capture があれば capture layer へ送る
2. modal layer があれば modal layer より上だけを対象にする
3. z が高い layer から hit test
4. target phase
5. bubble phase
6. global fallback
```

```rust
pub enum InputPhase {
    Capture,
    Target,
    Bubble,
    Global,
}
```

## RoutedInputEvent

```rust
pub struct RoutedInputEvent {
    pub raw_id: RawInputId,
    pub phase: InputPhase,
    pub target_layer: Option<LayerId>,
    pub target_entity: Option<EntityId>,
    pub target_view_node: Option<ViewNodeId>,
    pub event: InputEventKind,
    pub local_position: Option<Vec2>,
    pub route: Vec<LayerId>,
}
```

`route` は bubble 経路を表す。

```text
choice button
  → choice list
  → dialogue layer
  → root
```

## InputDisposition

handler は処理結果を返す。

```rust
pub enum InputDisposition {
    Ignored,
    Handled,
    StopPropagation,
    CapturePointer,
    ReleasePointer,
    Emit(Vec<GameEvent>),
    Command(Vec<Command>),
}
```

`Handled` は同じ phase の次候補を止めるが、bubble は続けられる。`StopPropagation` は bubble も止める。

## DSL: layer input

```arcw
layer @layer.choices: Choice {
    z = 200
    input = hit_test
    hit_test = view_layout
}

layer @layer.modal.settings: Modal {
    z = 1000
    input = modal
    backdrop = consume
}
```

View:

```arcw
Button("閉じる")
    .layer(@layer.modal.settings)
    .agent_target(@view.settings.close)
    .on_click {
        action.invoke(@action.settings.close)
    }
```

Activity:

```arcw
activity @activity.fps_arena FpsArena {
    layer @layer.activity.fps {
        input = capture_when_active
        keyboard = exclusive
        gamepad = exclusive
    }
}
```

## Focus

```rust
pub struct FocusState {
    pub focused_layer: Option<LayerId>,
    pub focused_entity: Option<EntityId>,
    pub focused_view_node: Option<ViewNodeId>,
    pub focus_scope: Option<FocusScopeId>,
}
```

Focus は layer scope を持つ。Modal が開いたら focus scope が modal 内へ移る。

```text
settings modal opened
  → focus_scope = layer.modal.settings
  → keyboard input は modal 内へ
  → lower layer は pointer/keyboard を受けない
```

## Keyboard / IME

Keyboard は pointer hit test ではなく focus layer へ送る。

```rust
pub enum KeyboardPolicy {
    None,
    FocusedOnly,
    BubbleToParents,
    Exclusive,
    GlobalShortcut,
}
```

TextInput/IME は `TextInput` role の View node が focus している場合だけ送る。Agent からの `TypeText` も同じ経路を使う。

## Semantic action

Agent や test は座標 click ではなく semantic action を優先する。

```arcw
SemanticAction::Invoke {
    target: @choice.opening.listen,
    action: "select",
    args: {},
}
```

Semantic action も layer を通る。

```text
1. target entity から layer を逆引き
2. layer policy が SemanticOnly/HitTest/Modal で許可するか検査
3. target が visible/enabled か検査
4. handler 実行
```

これにより、非表示・disabled・modal に隠れている target を Agent が誤って押すことを防ぐ。

## Modal

Modal layer は下位 layer への入力を遮断する。

```rust
pub struct ModalPolicy {
    pub backdrop: BackdropInput,
    pub escape_action: Option<GameEvent>,
    pub trap_focus: bool,
}

pub enum BackdropInput {
    Consume,
    Close,
    Passthrough, // 基本は非推奨
}
```

## Pointer capture / gesture

drag や game activity では pointer capture が必要。

```rust
pub struct InputCapture {
    pub pointer_id: PointerId,
    pub layer: LayerId,
    pub entity: Option<EntityId>,
    pub started_at: TickId,
}
```

capture 中は pointer move/up が同じ layer に届く。capture は `ReleasePointer` または pointer up で終了する。

## Transform

InputRouter は hit test 前に座標を layer local coordinate へ変換する。

```rust
screen_pos
  → root logical viewport
  → layer inverse transform
  → local_pos
  → hit_test
```

回転・scale・camera 付き layer でも入力が正しく届く。

## HTML / Servo / DOM

HTML layer は実 backend の hit test を使える場合は使う。

```text
Native Servo:
  Servo bridge / View metadata / panel bounds

Web DOM:
  DOM getBoundingClientRect / data-arcweft-entity / ARIA role

Headless approximate:
  HtmlPanelSpec + declared action targets
```

Observation には source を入れる。

```rust
bbox_source = ViewLayoutExact | ViewBridgeApprox | BackendUnavailable
```

## Test

```arcw
test @test.modal_blocks_choices scenario {
    goto @flow.opening
    view.open(@view.settings)

    input.click(@choice.opening.listen)
    expect.no_event(GameEvent::ChoiceSelected)

    input.click(@view.settings.close)
    input.click(@choice.opening.listen)
    expect.event(GameEvent::ChoiceSelected, id=@choice.opening.listen)
}
```

## Contracts

```arcw
layer @layer.modal.settings: Modal
ensures input.blocks_lower_layers
ensures focus.trapped_within(self)
{
    ...
}
```

Activity input 契約:

```arcw
activity @activity.fps_arena FpsArena
requires input_layer(@layer.activity.fps).policy == CaptureWhenActive
ensures no_lower_layer_receives_keyboard_while_active
{
    ...
}
```


## Layer routing trace

Layer routing の各段階は typed trace phase を記録する。

```text
BeforeHitTest
AfterHitTest
BeforeInputRoute
AfterInputRoute
```

これにより、modal が world layer の入力を本当に止めたかを test / Agent /
log が `AfterInputRoute` record から直接検査できる。

## Owner-local input handlers

Modal や drag capture は InputRouter、hit した object と bubble behavior は
target View/Activity tree が所有する。

```arcw
view ChoiceButton(choice: ChoiceView) {
    Button(choice.label)
        .on_pointer_enter {
            action.invoke(@action.choice.hover, choice.id)
        }
}
```

詳細: [Event Ownership and Caching](../01-language/hooks-and-memoization.md)

## Input routing phases

Layered input は次の internal routing phase を持つ。

```text
RawInputEvent
  → capture policy
  → layer candidate collection
  → hit-test
  → target dispatch
  → target-local handler
  → bubble phase
  → trace commit
```

Custom hit-test、mask 判定、Activity 独自判定は respective owner API が
実装する。Agent/debug diagnosis は trace を読む。実際の状態変更は入力処理
から直接行わず、`InputDisposition`、semantic action、`Command` として返す。


## Layer dispatch integration

Layer declaration owns routing policy; interaction remains on the target node.

```arcw
layer @layer.choices: Choice {
    z = 550
    input = hit_test
    hit_test = view_layout
}

view ChoiceButton(choice: ChoiceView) {
    Button(choice.label)
        .on_pointer_enter {
            action.invoke(@action.choice.hover, choice.id)
        }
}
```

入力 routing では local handler の `InputDisposition` が routing 結果に影響
する。Modal、pointer capture、debug overlay、Agent overlay は typed router
と trace を共有する。


## Target input handling

Layer routing は hit target の local handler を呼ぶ。

```arcw
view ChoiceButton(choice: ChoiceView) {
    Button(choice.label)
        .on_click {
            action.invoke(@action.choice.select, choice.id)
        }
}
```

handler の戻り値は `InputDisposition` として routing trace に保存され、replay
できる。

## Semantic event integration

`RoutedInputEvent` の target-local handler が生成した semantic action は通常の
input lowering と同じ event stream に入る。handler は durable state を直接
書き換えない。

```text
RawInputEvent
  → InputRouter
  → RoutedInputEvent
  → target-local handler
  → SemanticInputEvent / GameEvent
  → reducer
```

Hit-test や target resolution が高コストな場合は、InputRouter が committed
LayerTree routing hash と hit-region revision を typed cache key として管理する。
Author source does not provide a frame-cache key.

関連:

- [Runtime Dispatch and Caches](hooks-memoization.md)


## Device and virtual controller sources

Layered input accepts physical and virtual controller sources. See [Device Profiles, Generators, and USB](../03-presentation/device-generator-and-usb.md) and [Virtual Touch Controller](../03-presentation/virtual-controller.md).

```text
USB/HID/Gamepad/Keyboard/Touch
  -> ControllerMap
  -> LayerTree routing
  -> GameEvent / Activity input
```

Virtual controller layers usually sit above scene/View layers and consume touch input inside their hit regions before lower layers receive it.

## Touch virtual controller input

The touch virtual controller is a View-owned input producer. It receives raw touch/pointer events through the layer tree, captures touches for its controls, and emits normalized `InputAction` and `InputAxis` events.

```text
Touch event
  -> LayerTree hit test
  -> @layer.input.touch_controller
  -> VirtualControl state update
  -> InputAction / InputAxis
  -> routed to gameplay or narrative layer
```

This makes touch, keyboard, gamepad, USB macro pad, and Agent semantic actions converge on the same input model.

See [Touch Virtual Controller](../03-presentation/touch-virtual-controller.md).

