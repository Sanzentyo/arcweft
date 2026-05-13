# Hooks and Memoization

Object hook と memoization は、描画・入力・状態監視・lazy 評価・Agent Debug Bus をつなぐ横断機能である。

関連:

- [Layer System / Input Routing](../03-presentation/layers.md)
- [Layered Input runtime](../02-runtime/layered-input.md)
- [Runtime Hooks and Memoization](../02-runtime/hooks-memoization.md)
- [logging / signal / test / bench](../04-tooling/logging-signal-test-bench.md)
- [Hook Manifest schema](../schemas/hook-manifest.md)
- [Memo Cache schema](../schemas/memo-cache.md)
- [Hook example](../examples/hooks-memoization.md)

## 設計原則

```text
hook:
  Entity / Layer / UI node / Activity / Signal / StatePath / Need / Shader に紐づく宣言的な割り込み点。
  phase、check policy、condition、priority、effect capability を持つ。

memo:
  pure computation、hook condition、UI layout、shader reflection、text/typeset layout、TaskKey deduplication を再利用する仕組み。
```

hook は通常の callback ではない。全 hook は compile 時に `HookTable` へ lowering され、phase / priority / Layer order / EntityId で安定順に実行される。これにより replay、test、Agent debug、形式検証と整合する。

---

## Hook の基本構文

```awft
hook #hook.choice_listen_clicked
on #choice.opening.listen
phase InputTarget
check on input PointerClick
when state.flags.contains(.input_enabled)
priority 100
effects { emit_event, log, input_disposition }
{
    emit GameEvent::ChoiceSelected { id = #choice.opening.listen }
    log info "choice selected {id:?}" { id = #choice.opening.listen }
    stop_propagation
}
```

UI modifier 形式も許可するが、内部では hook に正規化する。

```awft
Button("聞いてみる")
    .agent_target(#choice.opening.listen)
    .on_input(PointerClick) {
        emit GameEvent::ChoiceSelected { id = #choice.opening.listen }
    }
```

---

## Hook target

```awft
on #choice.opening.listen
on #layer.ui.modal
on #character.alice
on #activity.truck_game
on #signal.loading_progress
on #shader.post.crt
```

query target も使える。

```awft
hook #hook.disable_all_choices
on query ChoiceOption where parent == #choice.opening.first
phase StateChanged
check on change state.ui.locked
when state.ui.locked
{
    command ui.disable(target)
}
```

---

## Hook phase

```awft
pub enum HookPhase {
    FrameStart,
    BeforeInputRoute,
    InputCapture,
    InputTarget,
    InputBubble,
    AfterInputRoute,
    BeforeReducer,
    AfterReducer,
    BeforeFlowResume,
    AfterFlowResume,
    BeforeTaskCommit,
    AfterTaskCommit,
    BeforeUiDiff,
    AfterUiDiff,
    BeforeRender,
    LayerPreRender,
    LayerPostRender,
    AfterRender,
    SignalChanged,
    StateChanged,
    NeedPending,
    NeedReady,
    ActivityMounted,
    ActivityUnmounted,
    ShaderRealized,
    DebugOnly,
}
```

例:

```awft
hook #hook.alice_affection_watch
on state .affection[#character.alice]
phase StateChanged
check on change
when state.affection[#character.alice] >= 3
once per save
{
    signal #signal.alice_route_unlocked <- true
}
```

```awft
hook #hook.modal_blocks_world
on #layer.ui.modal
phase InputCapture
check on input PointerClick
when layer.visible
{
    stop_propagation
}
```

---

## 条件チェックタイミング

`when` は pure expression のみ。チェックタイミングは明示する。

```awft
hook #hook.alice_route_unlock
on state .affection[#character.alice]
phase StateChanged
check on change
when state.affection[#character.alice] >= 3
once per save
{
    signal #signal.alice_route_unlocked <- true
}
```

利用できる `check` policy:

```awft
pub enum CheckPolicy {
    OnPhase(HookPhase),
    OnStatePath(StatePath),
    OnSignal(Ref<Signal>),
    OnInput(InputKind),
    OnLayerEvent(LayerEventKind),
    OnNeed(TaskOrNeedId),
    EveryFrame,
    EveryTicks(u32),
    EveryDuration(Duration),
    Manual,
}
```

構文例:

```awft
on check every frame
on check every 10 ticks
on check signal #signal.loading_progress
on check layer #layer.ui.modal visibility_changed
on check need #task.opening_assets ready
```

`every frame` は高コストになりやすいため、LSP は state/signal/layer-event への置換を提案する。

---

## Hook effects

phase ごとに許可される effect を制限する。

```awft
pub enum HookEffect {
    EmitEvent,
    Command,
    Log,
    SignalWrite,
    Assert,
    InputDisposition,
    MemoInvalidate,
    AgentAnnotation,
}
```

描画 phase では state mutation を禁止する。

```awft
hook #hook.bad_render_mutation
on before render
{
    state.flags += .bad // error
}
```

input hook は `InputDisposition` を返せる。

```awft
hook #hook.choice_keyboard_select
on #layer.choices
phase InputTarget
check on input KeyDown
when event.key == .Enter && focus.target.is_choice
{
    emit GameEvent::ChoiceSelected { id = focus.target.entity.as<ChoiceOption>()? }
    stop_propagation
}
```

---

## once / debounce / throttle

```awft
hook #hook.show_unlock_once
on state .affection[#character.alice]
phase StateChanged
check on change
when state.affection[#character.alice] >= 3
once per save
{
    emit GameEvent::AliceRouteUnlocked
}
```

```awft
hook #hook.log_progress_slowly
on signal #signal.loading_progress changed
throttle 250ms
{
    log debug "loading {p:f32}" { p = signal.value }
}
```

```awft
hook #hook.search_text_changed
on state changed .ui.search_text
debounce 300ms
{
    command search.rebuild_preview(state.ui.search_text)
}
```

`debounce` / `throttle` は logical time で扱い、replay 可能にする。

---

## Memoization の基本

```awft
memo fn choice_to_view(state: GameState)(choice: ChoiceDef) -> ChoiceView
scope = scene
{
    ChoiceView {
        id = choice.id,
        label = choice.label,
        enabled = choice.condition(state),
    }
}
```

明示構文:

```awft
memo fn route_graph(root: Ref<Flow>) -> RouteGraph
scope = bundle
depends = graph.flows
ensures deterministic(result)
{
    build_route_graph(root)
}
```

memoize できるものは pure / deterministic な計算に限定する。

---

## Memo scope

```awft
pub enum MemoScope {
    Frame,
    Tick,
    Scene,
    Flow,
    Session,
    Bundle,
    Persistent,
    Lease(Lifetime),
}
```

borrow を含む値は lifetime より長い scope に保存できない。

```awft
memo fn parse_header<'frame>(bytes: &'frame [u8]) -> Header
scope = scene
{ ... } // error

memo fn parse_header<'frame>(bytes: &'frame [u8]) -> Header
scope = frame
{ ... } // OK
```

---

## Memo key and dependency

デフォルト key:

```text
function EntityId
function semantic hash
type layout hash
argument stable hashes
captured environment hashes
selected dependencies
```

明示 key:

```awft
memo fn choice_enabled(state: GameState)(choice: ChoiceDef) -> Bool
scope = scene
key = (choice.id, state.affection[#character.alice])
{
    choice.condition(state)
}
```

自動依存追跡:

```awft
memo fn visible_choices(state: GameState) -> List<ChoiceView>
scope = scene
track = auto
{
    opening_choices()
        .filter(choice_available(state))
        .map(choice_to_view(state))
        .collect<List<ChoiceView>>()
}
```

---

## Invalidation

```awft
hook #hook.locale_changed
on signal #signal.locale changed
{
    memo.invalidate namespace text
    memo.invalidate entity #typeset.credits
}
```

Layer cache invalidation:

```awft
hook #hook.background_changed
on state changed .current_bg
{
    memo.invalidate layer #layer.world.background
}
```

---

## Need / Task memoization

```awft
task fn load_opening_assets() -> Result<OpeningAssets, AssetError> {
    let assets = memo(scope=scene, key=#asset_pack.opening) {
        load_opening_assets_task()
    }
    let bg = try await assets.bg with:
        pending p:
            scene #scene.loading:
                progress p.ratio
    let voice = try await assets.voice with:
        pending p:
            scene #scene.loading:
                progress p.ratio
    Ok(OpeningAssets { bg, voice })
}
```

同じ key の request は同じ in-flight task に合流する。

---

## UI / Render memoization

```awft
component SettingsPanel(props: SettingsProps) -> View {
    memo(scope=frame, key=(props, state.config.theme, env.text_scale)) {
        SettingsPanelBody(props)
    }
}
```

```awft
layer #layer.background: World {
    z = -1000
    cache = until invalidated
    depends asset #asset.bg.room
}
```

---

## Grammar additions

```ebnf
hook_item   := visibility? "hook" entity_ref? hook_target? hook_trigger hook_options? block
hook_target := "for" (entity_ref | query_expr)
hook_trigger:= "on" hook_event
hook_event  := "input" input_phase? input_kind
             | "state" "changed" state_path
             | "signal" entity_ref "changed"
             | "check" check_policy
             | "layer" entity_ref layer_event
             | "need" entity_ref need_event
             | "shader" entity_ref "realized"
             | "activity" entity_ref lifecycle_event
hook_options:= ("when" expr)? ("priority" int)? ("once" once_policy)? ("debounce" duration)? ("throttle" duration)?

memo_fn     := "memo" "fn" ident generic_params? fn_params return_type memo_options? block
memo_expr   := "memo" "(" memo_args? ")" block
memo_options:= ("scope" "=" memo_scope)? ("key" "=" expr_tuple)? ("depends" "=" dep_list)? ("track" "=" ("auto" | "manual"))?
```

## 最終ルール

```text
1. hook は callback ではなく HookTable に lowering される宣言である。
2. hook の phase / check policy / priority / effects を明示する。
3. hook 条件 `when` は pure expression のみ。
4. input hook は LayerTree routing と統合する。
5. render hook は state mutation 禁止。
6. hook 実行順は phase, priority, layer/input order, EntityId で安定化する。
7. memo は pure / deterministic な計算だけに付けられる。
8. memo scope は lifetime / save / replay と整合させる。
9. Need/Task は TaskKey で in-flight 合流し、memo cache と統合する。
10. Agent/LSP/CLI から hook/memo を検査・可視化できる。
```
