# Layered Input

Input は描画と同じ Layer Tree を基準に routing する。これにより、見えている UI、modal、HTML/Servo/DOM panel、Activity、Debug overlay、Agent 操作を同じ規則で扱える。

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
  ↓ uses LayerTree + UiTree + HitRegions
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
    UiLayout,
    Custom(EntityId),
}
```

`Mask` は object-id pass または UI hit mask を使う。`Custom` は Activity や complex UI が提供する hit test 関数を使う。

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
    pub target_ui_node: Option<UiNodeId>,
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
    hit_test = ui_layout
}

layer @layer.modal.settings: Modal {
    z = 1000
    input = modal
    backdrop = consume
}
```

UI component:

```arcw
Button("閉じる")
    .layer(@layer.modal.settings)
    .agent_target(@ui.settings.close)
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
    pub focused_ui_node: Option<UiNodeId>,
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

TextInput/IME は `TextInput` role の UI node が focus している場合だけ送る。Agent からの `TypeText` も同じ経路を使う。

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
  Servo bridge / UI metadata / panel bounds

Web DOM:
  DOM getBoundingClientRect / data-arcweft-entity / ARIA role

Headless approximate:
  HtmlPanelSpec + declared action targets
```

Observation には source を入れる。

```rust
bbox_source = UiLayoutExact | UiBridgeApprox | BackendUnavailable
```

## Test

```arcw
test @test.modal_blocks_choices scenario {
    goto @flow.opening
    ui.open(@ui.settings)

    input.click(@choice.opening.listen)
    expect.no_event(GameEvent::ChoiceSelected)

    input.click(@ui.settings.close)
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


## Hooks on layered input

Layer routing の各段階は hook phase を発火する。

```text
BeforeHitTest
AfterHitTest
BeforeInputRoute
AfterInputRoute
```

例:

```arcw
hook @hook.modal_block_check
on @layer.ui.modal
phase AfterInputRoute
when input.kind == .PointerDown
check on event
{
    if route.result == .BlockedBelow {
        signal.set(@signal.modal_blocked_input, true)
    }
}
```

これにより、modal が world layer の入力を本当に止めたかを test / Agent / log で検査できる。

## Input hooks

Input routing の各 phase では hook を実行できる。`input.capture` hook は modal や drag capture、`input.target` hook は実際に hit した object、`input.bubble` hook は親 layer への伝播に使う。

```arcw
hook @hook.choice.hover
on @choice.opening.listen
phase InputTarget
check on input PointerMove
when input.pointer.hovered
{
    event.emit(UiCommand::SetHover, target = @choice.opening.listen, value = true)
}
```

詳細: [Object Hooks](../01-language/hooks-and-memoization.md)

## Input hook phases

Layered input は hook phase を持つ。

```text
RawInputEvent
  → before_input hooks
  → layer candidate collection
  → hit_test hooks
  → target dispatch
  → on_input hooks
  → bubble phase
  → after_input hooks
```

例:

```arcw
hook @hook.choice.hit_trace
on @layer.ui.choices
phase InputHitTest
check on input PointerMove
when object.entity == @choice.opening.listen
{
    log.debug("hit choice listen")
}
```

`hit_test` hook は custom hit-test、mask 判定、Activity 独自判定、Agent 操作用の診断に使う。実際の状態変更は hook 内で直接行わず、`InputDisposition`、`GameEvent`、`Command` として返す。


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
check on change layout
{
    log.debug("choices layer layout changed")
}
```

入力 routing では hook の `InputDisposition` が routing 結果に影響する。Modal、pointer capture、debug overlay、Agent overlay はこの仕組みで共通化される。


## Input hooks

Layer routing の各 phase は hook の trigger になる。

```arcw
hook @hook.choice_click
on @choice.opening.listen
phase InputTarget
check on input PointerClick
{
    event.emit(GameEvent::ChoiceSelected, id = @choice.opening.listen)
    stop_propagation
}
```

入力 hook の戻り値は `InputDisposition` として routing trace に保存され、replay できる。

## Input hooks

Layered Input は Object Hook Runtime と接続する。`RoutedInputEvent` が生成されたあと、`OnInputTarget` phase の Hook が評価される。

```arcw
on @choice.opening.listen input click
when enabled(self)
{
    event.emit(GameEvent::ChoiceSelected, id = self)
}
```

Hook によって生成された `GameEvent` は、通常の input lowering と同じ semantic event stream に入る。Hook は state を直接書き換えない。

```text
RawInputEvent
  → InputRouter
  → RoutedInputEvent
  → OnInputTarget hooks
  → SemanticInputEvent / GameEvent
  → reducer
```

Hit-test や target resolution が高コストな場合は、frame scoped memo を使う。

```arcw
let routed = memo(scope=frame, key=(raw, layer_tree.routing_hash)) {
    route_input(raw, layer_tree, hit_regions)
}
```

関連:

- [Object Hook Runtime](hooks-memoization.md)
- [Runtime Memoization](hooks-memoization.md)


## Device and virtual controller sources

Layered input accepts physical and virtual controller sources. See [Device Profiles, Generators, and USB](../03-presentation/device-generator-and-usb.md) and [Virtual Touch Controller](../03-presentation/virtual-controller.md).

```text
USB/HID/Gamepad/Keyboard/Touch
  -> ControllerMap
  -> LayerTree routing
  -> GameEvent / Activity input
```

Virtual controller layers usually sit above scene/UI layers and consume touch input inside their hit regions before lower layers receive it.

## Touch virtual controller input

The touch virtual controller is a UI-owned input producer. It receives raw touch/pointer events through the layer tree, captures touches for its controls, and emits normalized `InputAction` and `InputAxis` events.

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

