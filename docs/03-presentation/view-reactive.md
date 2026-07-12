# Game Native View

Game Native View は SwiftUI 風の宣言的・リアクティブ View。HTML/CSS とは別に、ゲーム画面、選択肢、HUD、dialogue View、debug overlay、Agent 観測に使う。

## View

```arcw
pub view SettingsPanel(
    config: Binding<Config>,
    props: SettingsProps,
) {
    local state tab: SettingsTab = .Audio

    Column(spacing = 16) {
        Text("Settings").font(.title)

        Picker(
            value = bind(tab),
            options = [
                PickerOption(id=.Audio, label="Audio"),
                PickerOption(id=.Text, label="Text"),
                PickerOption(id=.Video, label="Video"),
            ],
        )

        match tab {
            .Audio => AudioSettings(config = config.audio)
            .Text  => TextSettings(config = config.text)
            .Video => VideoSettings(config = config.video)
        }

        Button("閉じる")
            .agent_target(@view.settings.close)
            .on_click { action.invoke(@action.settings.close) }
    }
    .padding(24)
    .background(.panel)
    .corner_radius(16)
}
```

## Binding

Binding は直接 state を破壊的に書き換えず、lens + event/command。

## Bundle execution contract

View program bundle は単一の暗黙 root や index-only child span を持たない。各
View 宣言を、次の閉じた定義 record として保持する。

- package/module scoped `public_id`
- 共通 instruction inventory 内の半開区間 `body`
- authored order の parameter schema（ordinal、name、scalar runtime type、
  definition-scoped value slot、typed default program）
- mount-state schema hash

`CallView` は対象 definition ID と、parameter ordinal/name に結び付いた
`ViewValueProgramId` を保持する。必須引数の欠落、未知の引数、型不一致、重複
binding、未知の View は bundle 作成または decode 時の structured failure であり、
no-op へは落とさない。空の View body は長さ 0 の正規 span として有効である。
同名 parameter でも View definition が異なれば別 slot であり、型を共有・推測
しない。local と repeat ordinal の state slot も definition ID で scope される。

flow から mount された View を root とし、View body 内の nested View call を
再帰的にたどった到達可能な定義だけを bundle に含める。`mod game.opening` 内の
`Child(...)` は `view.game.opening.Child` へ解決される。module path の区切りは
`.` である。

## Retained execution and mount identity

Runtime-driver は live presentation handle ごとに root View occurrence を一つ保持し、
nested call と keyed repeat は structural path で子 occurrence を識別する。同じ
View definition を main/side panel など複数の handle が同時に参照してよく、各
occurrence は別の monotonic `ViewMountId`、activation logical time、deterministic
seed、parameter/state revision、TextInput 値、Fx instance identity を持つ。resource
ID は definition identity であり、単一 owner を表さない。

View evaluator は bundle の `ViewValueProgram` を共通の typed value evaluator で
実行する。parameter、state projection、local、repeat ordinal は明示 slot からのみ
読み、未初期化 slot、型不一致、非有限値、budget 超過は structured diagnostic に
なる。placeholder 値を実行値として使わない。context time は mount activation から
の logical seconds、ordinal は対象内の logical instruction/item index である。
glyph-target sampler の ordinal は Fx application ごとに最初の対象 glyph を 0
として rebase し、文書全体の glyph index や UTF-8 byte offset を渡さない。
reduce-motion 時は sampler time を 0 に固定する。

`Await` の state discriminant は `pending = 0`、`ready = 1`、`error = 2`、
`denied = 3` に固定する。未知の値や branch span 不整合は no-op ではなく診断に
なる。`Branch`、keyed `Repeat`、nested `CallView`、`BindLocal`、`ApplyFx` は同じ
frame operation/value budget の下で評価される。

評価結果は mount-scoped target/image、typed text source、Fx application を保持
する。plain text 以外の localized/RichText/display-frame source を文字列へ黙って
潰さない。実際の scene resource ID は `view_mount_<id>.<authored-id>` に scope
され、同じ authored control を二つの mount で独立に操作できる。画像と scroll
element も lowering 時に concrete target ID を持つ。

View text bundle は source record と実体 store を分離する。localized store は
`(TextKey, locale)` に対する `RichTextDocument`、rich-text store は document ID に
対する `RichTextDocument`、display-frame store は frame ID に対する
`LineDisplayFrame` と stage index を保持する。参照先や stage が存在しない場合は
`VIEW014` から `VIEW017` の typed diagnostic で mount 評価を失敗させる。空文字、
debug 表現、既定 locale、plain text への暗黙 fallback は使わない。locale 未指定の
source だけは、bundle compile 時に同じ `TextKey` の canonical display catalog が
存在すれば、その document を初期 store として materialize してよい。

Player は評価済みの typed value を `ResolvedTextDocument` に解決し、通常の
`TextLayout` と frame-local `PreparedTextBatch` へ直接追加する。vertical writing、
ruby、text-combine、locale、run source、selection/scroll clip はこの境界まで型付きで
保持し、View 専用の string block や二度目の layout は作らない。

paint IR は `Element`、`Text`、`Image` と nested `Mount` の authored order を保持
する。nested mount は親の `Mount` slot で再帰展開するため、親の前後へ後置されない。
View 所有 image は通常 image pass から View scene resource table へ移し、crop UV、
affine transform、opacity を保持した `Image` primitive として同じ painter sequence に
置く。したがって Text/Image/element/child View の相対順序は native、Web、headless
で一つの scene contract になる。

save/load は logical time、mount allocator cursor、root bindings、occurrence path、
activation time、seed、typed parameter/state value と revision、初期化 slot、runtime
parameter snapshot を保存する。restore は program/schema/type/allocator と、保存済み
presentation frame が retained mount table の handle/path/View/mount identity に一致
することを代入前に検証する。

```arcw
Slider(value = bind state.config.master_volume, range = 0.0..1.0)
```

展開:

```rust
Binding<f32> {
    get = .config.master_volume,
    set = |v| GameEvent.View(.SetMasterVolume { value = v }),
}
```

## Reactive dependencies

view 評価中に読んだ依存を記録する。

```rust
pub enum ViewDependency {
    StatePath(StatePathId),
    Signal(EntityId),
    LocalState(ViewLocalId),
    Environment(EnvironmentKey),
    Resource(AssetId),
    Font(FontId),
    Locale,
}
```

変更時は該当 view だけ invalidated。

## View / Modifier

```arcw
Text("聞いてみる")
    .font(.body)
    .padding(x = 24, y = 12)
    .background(.button)
    .corner_radius(8)
    .transition(.fade(duration = 120ms))
    .animation(.spring, value = is_selected)
```

## AwaitView

View でも `Need` の暗黙 force は禁止。

```arcw
AwaitView(load_avatar(user)) {
    pending _ => SkeletonCircle()
    ready img => Image(img)
    error _ => Icon(@vector.avatar_fallback)
}
```

## Retained list virtualization

Virtualized lists are addressed by mount occurrence, not only by View program.
The implemented range/save substrate therefore keeps independent source
inventory, viewport, offset, and materialized-window state for two mounts of
the same program. Typed child-local state belongs to the future evaluator and
must use the same mount/key identity; the range planner does not claim to
serialize an opaque child state value.

The Sans I/O range contract consumes a finite ordered item set. Every item has
a stable key and a resolved non-zero primary-axis extent in logical
milli-pixels. It produces one half-open materialized window plus a complete
range table. Items outside the window remain in the table and are reported as
non-materialized. Leaving the window does not discard their stable range
identity. Whether concrete child-local state is retained, pruned, focused, or
temporarily materialized is an evaluator policy and is not invented by this
Sans I/O planner.

Live source replacement preserves a key-relative scroll anchor when source
order changes. Save/load instead restores the exact finite inventory and
absolute offset; its derived anchor is an integrity check, so contradictory or
tampered offset/anchor pairs are rejected rather than silently normalized.

`LazyRow` and `LazyColumn` authoring must not be implemented as eager Row/Column
aliases. The grammar becomes available only when the typed View evaluator can
provide finite keyed values and the layout layer can resolve off-window extents
under one deterministic measurement policy. That evaluator must also allocate
an occurrence-specific actionable Scroll identity; the current player and
Agent action path still addresses authored Scroll strings and cannot
independently route two mounts of one authored Scroll.

## Agent output

View node は bbox / polygon / mask / action target を持つ。

```rust
pub struct ViewNode {
    pub entity: Option<EntityId>,
    pub role: ViewRole,
    pub label: Option<String>,
    pub bbox: BBox,
    pub polygon: Option<Polygon>,
    pub actions: Vec<ActionTarget>,
}
```

## Memoized view and hooks

View は dependency tracking によって必要部分だけ再評価される。高価な派生値には `memo` を使う。

```arcw
view ChoiceList(state: GameState) {
    let choices = memo(scope=frame, key=(state.route, state.affection)) {
        opening_choices()
            .filter(choice_available(state))
            .map(choice_to_view(state))
            .collect<Vec<ChoiceView>>()
    }

    Column {
        for choice in choices key = choice.id { ChoiceButton(choice) }
    }
}
```

View node には hook を付けられる。

```arcw
hook @hook.choice_button_has_action
on query ViewNode where role == .Choice
phase AfterLayout
{
    assert(object.actions.contains("select"))
}
```
