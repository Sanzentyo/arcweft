# 結論

最優先で直すべき方針はこれです。

```text
1. `@choice` は削除し、`choice #choice... { ... }` に統一する。
2. `choice` は `@choice` から `@` を外した記法を canonical にする。
3. 旧 `say #say... alice ...` は削除する。
4. 旧 `alice voice auto:` は削除し、`alice(voice=auto):` に統一する。
5. line plan の値返しは `return` ではなく `out` に統一する。
6. `{ ... }` は expression block と lexical scope の両方として正式仕様にする。
7. `grammar.md` は今のままだと control-flow subset なので、全体 grammar へ戻すか改名する。
8. hook 構文は `on target / phase / check / when / effects` に統一し、`for`、`phase =`、`at` 系は削除する。
9. Layer 系は `LayerTree` に統一し、`LayerStack` を主要概念から消す。
10. raw block、escape、window/textbox、README 表現などの細かい不整合を直す。
```

---

# 1. `choice` は `@choice` なしの形を canonical にする

あなたの方針どおり、記法は `@choice` の中身を残して `@` だけ消すのがよいです。

canonical:

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

条件付き:

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" if can_enter_alice -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

この形は、選択肢 ID、表示ラベル、条件、遷移先が一行で見えます。`@bg` や `@show` は scenario command として `@` 付きでもよいですが、`choice` は flow 制御に近いので `@` なしの方が自然です。

現在は `syntax.md`、`scenario-surface-syntax.md`、`localization-dialogue.md` に `@choice` が残っています。
parser も現状では `@choice` を見て choice block として parse しているため、実装も直す必要があります。

削除方針なら、parser は `@choice` を受けないようにしてよいです。

```rust
if trimmed.starts_with("choice ") {
    return self.parse_choice().map(FlowItem::Choice);
}
```

`parse_choice()` も `@choice` ではなく `choice` のみを受けます。

```rust
let rest = head.trim().strip_prefix("choice")?.trim();
```

`@choice` はエラーにして、diagnostic でこう出すとよいです。

```text
error[OLD_CHOICE_SYNTAX]:
  `@choice` is not valid Arcweft syntax.
  Use `choice #choice.id { ... }`.
```

---

# 2. 旧 `choice #choice... { option ... }` も削除する

`syntax.md` と `opening-flow.md` には、次のような旧式の「値取得型 choice」が残っています。

```awft
let selected = choice #choice.opening.first {
    option #choice.opening.listen "聞いてみる"
    option #choice.opening.silent "黙っている"
}
```

これは、あなたが好む `@choice` から `@` を外した形とは別物なので、今の段階では削除した方がよいです。

削除後の最小例はこうです。

```awft
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    alice(id=#say.opening.greeting): おはよう。[p]

    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }
}
```

値として選択結果が必要な仕様は、今は入れない方がよいです。入れるなら後で別構文として追加するのが安全です。たとえば将来的に必要なら、`choose` など別名にして `choice` と分ける方が読みやすいです。

```awft
let selected = choose #choice.opening.first {
    #choice.opening.listen "聞いてみる"
    #choice.opening.silent "黙っている"
}
```

ただし、今の整理ではこれは未採用でよいです。まずは `choice` を直接分岐する flow item として固定する方がよいです。

grammar はこうします。

```ebnf
choice_block :=
    "choice" entity_ref? "{" choice_arm* "}"

choice_arm :=
    entity_ref string choice_condition? "->" entity_ref

choice_condition :=
    "if" expr
```

説明文はこう書くと明確です。

```text
`choice` displays a choice block and advances the current flow to the selected arm's target.
It is a FlowItem, not an `@` scenario command.
```

日本語なら:

```text
`choice` は選択肢を表示し、選ばれた arm の `->` 先へ flow を進める FlowItem である。
`@bg` や `@show` のような scenario command ではないため、`@choice` ではなく `choice` と書く。
```

---

# 3. 旧 `say #say... alice ...` は削除する

`syntax.md`、`state-flow-reducer.md`、`opening-flow.md`、`code-fences.md` に古い `say` statement が残っています。

削除対象:

```awft
say #say.opening.greeting alice "おはよう。"
```

削除対象:

```awft
say #say.opening.dream_hint alice rich """
今日は少しだけ、{ruby "変な夢" "へんなゆめ"}を見たんだ。
""" with voice #cue.voice.alice.001
```

canonical は `alice.say()[...]` です。`alice:` はその sugar として残します。これは dialogue 系 docs でも既に明記されています。

置換例:

```awft
alice.say(id=#say.opening.greeting)[
    おはよう。[p]
]
```

短く書くなら:

```awft
alice(id=#say.opening.greeting): おはよう。[p]
```

rich text 例はこうします。

```awft
alice.say(id=#say.opening.dream_hint, voice=#cue.voice.alice.001)[
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
]
```

または voice auto に寄せるなら:

```awft
alice.say(id=#say.opening.dream_hint, voice=auto)[
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
]
```

parser も旧 `say` statement を受けないようにして、docs から完全に消すのがよいです。

---

# 4. 旧 `alice voice auto:` は削除する

`syntax.md` と `localization-dialogue.md` に、まだこの形が残っています。

削除対象:

```awft
alice voice auto: 今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
```

canonical:

```awft
alice(voice=auto): 今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
```

line ID 付き:

```awft
alice(id=#say.opening.dream_hint, voice=auto): 今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
```

LSP inlay view もこう直します。

```text
alice(voice=auto): 今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
                   #say.opening.alice.002 / text.opening.alice.002 / voice.ja-JP.alice.opening.002
```

`alice #say... @smile voice auto:` なども同時に削除です。互換 parser で受ける必要はありません。

---

# 5. `{ ... }` は expression block と lexical scope の両方にする

この方針は採用でよいです。現在の `block-scopes.md` は `{ ... }` を lexical scope として使えると明記しており、`semicolon-policy.md` も value-producing block と statement-oriented block を分けています。

仕様としてはこうまとめるのがよいです。

```text
{ ... } は文脈によって2種類になる。

1. expression position:
   BlockExpr として扱う。
   最後の式が block の値になる。

2. statement position:
   ScopeStmt として扱う。
   新しい lexical scope を作る。
   外側へ値を返さない。
```

例:

```awft
let x = {
    let a = 1
    let b = 2
    a + b
}
```

これは `BlockExpr` です。`x` は `3` になります。

一方、これは `ScopeStmt` です。

```awft
{
    let tmp = route_title(state.route)
    log debug "route={tmp}" { tmp = tmp }
}

// tmp はここでは見えない
```

final expression の扱いは明確にした方がよいです。statement position の裸 block では、外へ値を出さないので、非 `Unit` の最後の式はエラーか warning にするのがよいです。開発中で互換不要なら、strict にする方が安全です。

```awft
{
    compute_value()
}
```

これは `compute_value()` が `Unit` 以外なら error にするのがおすすめです。

```text
error[BLOCK_VALUE_DISCARDED]:
  This scope block is used as a statement, but its final expression returns `T`.
  Use `let _ = ...` or add `;` to discard explicitly.
```

明示的に捨てるなら:

```awft
{
    compute_value();
}
```

または handle/drop 意図があるなら:

```awft
{
    let _ = bgm.play(#bgm.tension, scope=line)
}
```

grammar はこう直すとよいです。

```ebnf
Block          := "{" BlockItem* FinalExpr? "}"
ExprBlock      := Block
ScopeStmt      := Block
StatementBlock := Block | ":" Newline IndentedItems
LabeledBlock   := Label? Block

BlockItem      := LetStmt
                | LetElseStmt
                | ExprStmt
                | ControlStmt
                | ScenarioStmt
                | ScopeStmt
```

意味論:

```text
ExprBlock:
  expression position で使う。
  final expression があればその値を返す。
  final expression がない、または final expression が `;` で捨てられた場合は Unit。

ScopeStmt:
  statement position で使う。
  lexical scope を作る。
  外側へ値を返さない。
  非 Unit の final expression は明示 discard が必要。
```

重要なのは、dialogue line plan と混同しないことです。`dialogue-line-handles-and-returns.md` は、dialogue content の後ろの裸 `{ ... }` は line plan ではなく別 lexical scope、line plan には `with { ... }` を使う、と既に明記しています。
これはよい設計なので維持します。

```awft
alice.say()[おはよう。[p]] {
    debug_log()
}
```

これは line plan ではなく scope block。

line plan は必ず:

```awft
alice.say()[おはよう。[p]]
with {
    at(0.42s) { alice.stage.face(smile) }
}
```

または sugar:

```awft
alice.say()[おはよう。[p]]
with:
    at(0.42s):
        alice.stage.face(smile)
```

---

# 6. line plan の値返しは `return` ではなく `out` に統一する

新しい方針では、line plan、cue block、content scope から値を外へ出すのは `out` です。`control-transfer-return-out-yield.md` でも、`return` は nearest `fn` / `task fn` / `parser` / `flow` から抜ける、`out` は line plan / cue block / content scope から値を出す、と整理されています。
`dialogue-line-handles-and-returns.md` も `out` に寄っています。

ただし `dialogue-calls-scopes-cancellation.md` の後半には、まだ line plan で `return` を使う例が残っています。

削除対象:

```awft
return (actor, (face0, face1, voice))
```

canonical:

```awft
out (actor, (face0, face1, voice))
```

修正版:

```awft
let (actor, (face0, face1, voice)) = alice.say(
    id=#say.opening.dream_hint,
    voice=auto,
    face=smile,
)[
    今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
]
with:
    let actor = alice.stage.acquire(scope=line)
    let face0 = actor.face(smile)
    let voice = line.voice_handle()
    let face1 = at(0.42s):
        actor.face(worried, crossfade=120ms)

    out (actor, (face0, face1, voice))
```

`return` を使ってよいのは flow から抜ける場合だけです。

```awft
cancel on input .BackToTitle:
    return Ok(FlowExit::Goto(#flow.title))
```

これは正しいです。

docs には次のルールを明記するとよいです。

```text
Inside a line plan:
  `out expr` exports the line result.
  `return expr` exits the enclosing flow/function.
```

---

# 7. `grammar.md` は今のままだと「文法サマリ」ではない

`01-language/README.md` では `grammar.md` が「文法サマリ」としてリンクされています。
しかし現在の `grammar.md` は `Grammar Summary: Control Flow and Patterns` で、control-flow subset の grammar だけです。

今のままだと、次が抜けています。

```text
- module / use / pub
- flow / fragment
- dialogue speaker syntax
- choice
- scenario command
- content block
- line plan
- hook
- memo
- layer
- parser
- dialogue tag
- entity ref
- type grammar 全体
```

対応は2択です。

## 案A: `grammar.md` を全体 grammar に戻す

この構成にします。

```md
# Grammar Summary

## Lexical conventions
## Module items
## Types
## Entity references
## Flow and fragments
## Scenario commands
## Dialogue and content calls
## Choice
## Blocks and scopes
## Control flow and patterns
## Await / Need / Result
## Hooks and memoization
## Layers
## Parser items
```

現在の control-flow grammar は `## Control flow and patterns` に入れます。

## 案B: `grammar.md` を改名する

現実的にはこちらもよいです。

```text
grammar.md -> control-flow-grammar.md
```

そして `01-language/README.md` のリンクをこう直します。

```md
- [Control-flow grammar](control-flow-grammar.md)
- [Surface grammar summary](surface-grammar.md)
```

ただし、いずれにしても `choice`、dialogue、hook、layer の grammar はどこかに必要です。

---

# 8. `fragment` grammar を追加する

`scenario-surface-syntax.md` では reusable scenario snippets として `fragment` が出ています。
しかし現在の `grammar.md` は control-flow subset なので、`fragment` の grammar がありません。

追加する grammar:

```ebnf
flow_decl :=
    visibility? "flow" entity_ref ident? param_list? return_type? contract* flow_body

fragment_decl :=
    visibility? "fragment" entity_ref ident? (":" type)? contract* flow_body

flow_body :=
    "{" flow_item* "}"
```

docs では name なしの形が出ています。

```awft
pub fragment #frag.alice_enters: FlowFragment {
    @show alice normal at=right fade=220ms
    @move alice to=center time=300ms ease=cubic.out
}
```

これを正式に許すなら、`ident?` で optional にします。

canonical は name なしでよいと思います。

```awft
pub fragment #frag.alice_enters: FlowFragment {
    @show alice normal at=right fade=220ms
    @move alice to=center time=300ms ease=cubic.out
}
```

名前付きは今は削除してよいです。開発中なので、余分な variant を増やさない方がよいです。

---

# 9. Hook 構文は今すぐ一本化した方がよい

hook は現状かなり分裂しています。

`syntax.md` では:

```awft
hook #hook.opening.choice_visible
on #choice.opening.listen
phase AfterLayout
when object.visible && object.enabled
check every frame
{
    ...
}
```

という形です。

`hooks-and-memoization.md` では:

```awft
hook #hook.choice_listen_clicked
for #choice.opening.listen
on input target PointerClick
...
```

という形です。

`examples/hooks-memoization.md` では:

```awft
phase = input.hit_test
check = on_change(...)
```

のような `=` 付き構文です。

parser の現状は、hook header から `on `、`phase `、`check ` の行を探しています。`for` や `phase =` ではありません。

後方互換不要なら、canonical はこれに統一するのがよいです。

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

state watch:

```awft
hook #hook.alice_route_unlock
on state .affection[#character.alice]
phase StateChanged
check on change
when state.affection[#character.alice] >= 3
once per save
effects { signal_write }
{
    signal #signal.alice_route_unlocked <- true
}
```

layer input:

```awft
hook #hook.modal_blocks_world
on #layer.ui.modal
phase InputCapture
check on input PointerClick
when layer.visible
effects { input_disposition }
{
    stop_propagation
}
```

削除する構文:

```awft
for #choice.opening.listen
on input target PointerClick
phase = input.target
check = every_frame
at input.capture
hook on input.pointer_enter
on #layer.choice_ui appear once
```

これらは「非推奨」ではなく docs / parser / tests から消します。

hook grammar はこうします。

```ebnf
hook_item :=
    visibility? "hook" entity_ref
    hook_target
    hook_phase
    hook_check?
    hook_when?
    hook_priority?
    hook_once?
    hook_effects?
    block

hook_target :=
    "on" hook_target_expr

hook_target_expr :=
    entity_ref
  | "state" state_path
  | "signal" entity_ref
  | "query" type where_clause?

hook_phase :=
    "phase" ident

hook_check :=
    "check" check_policy

hook_when :=
    "when" expr

hook_priority :=
    "priority" int

hook_once :=
    "once" once_policy

hook_effects :=
    "effects" "{" effect_name_list "}"
```

---

# 10. Memo 構文も整理する

現在は、`memo fn ... cache session`、`memo fn ... scope bundle`、`@memo(scope = scene)`、`memo alice.stage.acquire(...)` が混ざっています。

後方互換不要なら、`cache` は消して `scope` に統一するのがよいです。

canonical item form:

```awft
memo fn route_graph(root: Ref<Flow>) -> RouteGraph
scope = bundle
depends = graph.flows
ensures deterministic(result)
{
    build_route_graph(root)
}
```

attribute form を残すなら:

```awft
@memo(scope = scene, key = (choice.id, state.affection[#character.alice]))
fn choice_enabled(state: GameState)(choice: ChoiceDef) -> Bool {
    choice.condition(state)
}
```

expression memo は、曖昧さを避けるため block 形に寄せるのがよいです。

```awft
let actor = memo(scope=scene, key=(#character.alice, pose=normal, theme=env.theme.hash)) {
    alice.stage.acquire(scope=line)
}
```

削除対象:

```awft
cache session
memo alice.stage.acquire(key=..., cache=scene)
```

---

# 11. Layer 系は `LayerTree` に統一する

現在、`layers-and-input.md` は `LayerTree` を中心に説明しています。
一方、`render-input-layers.md` は `LayerStack` を導入し、`RenderSpec` に `layer_stack: LayerStackSpec` を持たせています。

この2つは概念が被っています。今後の source of truth は `LayerTree` にするのがよいです。

理由:

```text
- parent / children を持つ
- render_order と input_order を両方持てる
- routing_hash を持てる
- Agent observation や replay と相性がよい
```

canonical Rust shape:

```rust
pub struct RenderSpec {
    pub size: UVec2,
    pub clear: Color,
    pub layer_tree: LayerTree,
    pub layer_contents: IndexMap<LayerId, LayerContent>,
    pub postprocess: Vec<ShaderPassSpec>,
}
```

`LayerStack` は概念として削除するか、使うとしても「LayerTree から得られる traversal view」としてだけ使います。

```text
LayerTree:
  canonical runtime data structure.

Render order:
  derived from LayerTree.

Input order:
  derived from LayerTree.

LayerStack:
  do not use as a public core type.
```

`render-input-layers.md` は `LayerStack` を `LayerTree` に全面置換するのがよいです。

削除対象:

```rust
pub struct LayerStackSpec
pub struct RenderSpec {
    pub layer_stack: LayerStackSpec,
}
```

置換:

```rust
pub struct LayerTree {
    pub root: LayerId,
    pub layers: IndexMap<LayerId, LayerNode>,
    pub render_order: Vec<LayerId>,
    pub input_order: Vec<LayerId>,
    pub routing_hash: RoutingHash,
}
```

Layer DSL も一本化します。

canonical:

```awft
pub layer #layer.choice_ui: GameUi {
    order = ui(20)
    input = capture_on_hit
    hit_test = ui_layout
}
```

削除対象:

```awft
layer #layer.settings phase Modal z 900 {
    ...
}
```

`phase Modal z 900` 形式は消して、`order = modal(0)` などに寄せた方が一貫します。

---

# 12. `await` は `try await` に統一し、旧 `await expr? with` を削除する

これは現状かなりよく整理されています。`await-need-result.md` は `await expr with:` は `Result<T,E>`、`try await expr with:` は `T`、`await? expr with:` は `try await` と同等、と説明しています。

旧式の曖昧な形:

```awft
await asset.image(#asset.bg.room)? with:
    pending p:
        scene #scene.loading
```

これは削除でよいです。`await-need-result.md` でも rejected ambiguous syntax とされています。

canonical:

```awft
let bg = try await asset.image(#asset.bg.room) with:
    pending p:
        scene #scene.loading:
            progress p.ratio
```

または Result を明示的に受けるなら:

```awft
let bg_result = await asset.image(#asset.bg.room) with:
    pending p:
        scene #scene.loading:
            progress p.ratio
```

`syntax.md` の entity ref 例にある `await #<activity.truck_game>.run(input)` は、pending handling がないため、例としてはやや危険です。
次のようにする方がよいです。

```awft
let result = try await #<activity.truck_game>.run(input) with:
    pending p:
        scene #scene.loading:
            progress p.ratio
```

---

# 13. dialogue raw block の例は直す

`dialogue-character-methods-and-textbox.md` と `dialogue-control-tags-and-ruby.md` に、次の raw block 例があります。

削除対象:

```awft
alice.say()[raw]
ここでは複数行にわたりタグを解釈しない。
[p] も文字として表示する。
[/raw]
```

これは `alice.say()[ ... ]` の content block がどこで閉じるのか分かりにくく、専用 grammar なしでは不安定です。

canonical:

```awft
alice.say()[
    [raw]
    ここでは複数行にわたりタグを解釈しない。
    [p] も文字として表示する。
    [/raw]
]
```

raw span:

```awft
alice.say()[
    [raw]ここでは[p]も#[expr]も解釈されない。[/raw]
]
```

`alice.say()[raw] ... [/raw]` 形式は削除でよいです。

---

# 14. escape 表は `\｜` に統一する

現在の escape 表は `\|` を「literal ruby bar `｜`」として載せています。
しかし自然 ruby の delimiter は fullwidth の `｜` です。

後方互換不要なら、ASCII pipe escape は消して、実際の対象文字を escape する方が一貫します。

削除対象:

```text
\|   literal ruby bar `｜`
```

canonical:

```text
\｜  literal ruby bar `｜`
```

escape table:

```text
\\   backslash
\[   literal [
\]   literal ]
\#   literal #
\{   literal {
\}   literal }
\:   literal :
\｜  literal ruby bar ｜
\《  literal 《
\》  literal 》
```

---

# 15. `window` / `textbox` の使い分けを明確化する

dialogue line option は `window` が canonical です。`dialogue-character-methods-and-textbox.md` でも `textbox=` は migration alias と書かれています。

開発中で互換不要なら、`textbox=` alias は削除します。

canonical:

```awft
alice.say(window=#textbox.phone_message, voice=auto)[
    スマホに通知が届いた。[p]
]
```

削除対象:

```awft
textbox = #textbox.0
```

ただし entity kind としての `textbox` は残します。

```awft
pub textbox #textbox.phone_message PhoneMessageBox {
    layer = #layer.ui.messages
    anchor = bottom_right
    width = 420
    style = #style.phone_message
}
```

用語整理:

```text
textbox:
  entity kind / UI object kind.

window:
  dialogue line option name.

dialogue window:
  prose term for the player-facing text display area.
```

`dialogue_style` 内も `window` に寄せます。

削除対象:

```awft
dialogue_style {
    textbox = #textbox.narrator
}
```

canonical:

```awft
dialogue_style {
    window = #textbox.narrator
}
```

`dialogue defaults` も:

```awft
dialogue defaults {
    window = #textbox.0
}
```

に統一します。

---

# 16. `dialogue_defaults` と `dialogue defaults` は片方にする

docs には `pub dialogue_defaults #dialogue.defaults` と `dialogue defaults { ... }` が混在しています。

おすすめは、entity を持つ宣言なら `dialogue defaults #dialogue.defaults` にすることです。

canonical:

```awft
pub dialogue defaults #dialogue.defaults {
    window = #textbox.0
    read_state_style = builtin.read_state_color(
        unread = rgb("#ffffff"),
        read = rgb("#b8b8c0"),
    )
    auto_mark_read = on_page_advance
}
```

または、project-wide config 的に使うなら entity ID なしで:

```awft
dialogue defaults {
    window = #textbox.0
}
```

どちらかに絞るべきです。個人的には、他の DSL item と揃えるなら entity ID ありの方がよいです。

---

# 17. `README.md` の「完成版設計仕様」は今の状態と合っていない

`docs/README.md` はタイトルが `Arcweft Engine 完成版設計仕様` になっています。
ただし現状は、旧構文の名残や grammar の未統合が残っています。開発中なら「完成版」は消した方がよいです。

修正案:

```md
# Arcweft Engine 設計仕様
```

または:

```md
# Arcweft Engine 統合設計仕様
```

さらに draft 感を出すなら:

```md
# Arcweft Engine 設計仕様 draft
```

今の段階では `完成版` は避けた方がよいです。

---

# 18. `inspection できる` は日本語として直す

`hooks-and-memoization.md` の最終ルールに:

```text
Agent/LSP/CLI から hook/memo を inspection できる。
```

という文があります。

修正:

```text
Agent/LSP/CLI から hook/memo を検査・可視化できる。
```

または英語で統一するなら:

```text
Agent/LSP/CLI can inspect hook and memo state.
```

---

# 19. ファイル別の具体的修正リスト

## `docs/01-language/syntax.md`

削除:

```awft
say #say.opening.greeting alice "おはよう。"
```

削除:

```awft
let selected = choice #choice.opening.first {
    option #choice.opening.listen "聞いてみる"
    option #choice.opening.silent "黙っている"
}
```

削除:

```awft
alice voice auto:
```

削除:

```awft
@choice #choice.opening.first
```

置換後:

```awft
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    alice(id=#say.opening.greeting): おはよう。[p]

    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }
}
```

hook 例も canonical に寄せます。

```awft
hook #hook.opening.choice_visible
on #choice.opening.listen
phase AfterLayout
check every frame
when object.visible && object.enabled
effects { signal_write, assert }
{
    signal #signal.choice_visible <- true
    debug_assert object.bbox.area > 0
}
```

`on object ... at visibility_changed` は削除します。

---

## `docs/01-language/scenario-surface-syntax.md`

削除:

```awft
@choice #choice.opening.first
```

置換:

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

`fragment` は残しますが、grammar に追加します。

---

## `docs/01-language/localization-dialogue.md`

削除:

```awft
alice voice auto:
@choice
```

置換:

```awft
alice(voice=auto): 今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]

choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

LSP inlay view も `alice(voice=auto):` に直します。

---

## `docs/examples/opening-flow.md`

削除:

```awft
say #say.opening.dream_hint alice rich """
...
""" with voice #cue.voice.alice.001
```

削除:

```awft
let selected = choice #choice.opening.first {
    for c in choices { option c.id c.label }
}
```

この example は、dynamic choices を使いたいなら、canonical choice だけでは書きにくいです。開発中なら一旦 static choice 例に直すのがよいです。

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" if can_enter_alice -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

dynamic choices は別章で、後から正式 grammar を設計するのがよいです。

---

## `docs/00-overview/code-fences.md`

削除:

```awft
say #say.opening.001 alice "おはよう。"
```

置換:

```awft
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    alice(id=#say.opening.001): おはよう。[p]
    Ok(FlowExit::Done)
}
```

---

## `docs/01-language/state-flow-reducer.md`

削除:

```awft
say #say.opening.greeting alice "おはよう。"
let selected = choice #choice.opening.first { ... }
```

置換:

```awft
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    alice(id=#say.opening.greeting): おはよう。[p]

    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }

    Ok(FlowExit::Done)
}
```

ただし `choice` が flow transition を発生させるなら、その後に `Ok(FlowExit::Done)` を置く意味が曖昧です。ここは説明用にするなら:

```awft
pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    alice(id=#say.opening.greeting): おはよう。[p]

    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }
}
```

として、`choice` が flow を進めることを示す方がよいです。

---

## `docs/01-language/dialogue-calls-scopes-cancellation.md`

line plan の `return` をすべて `out` に置換します。

削除:

```awft
return (actor, (face0, face1, voice))
```

置換:

```awft
out (actor, (face0, face1, voice))
```

`return Ok(FlowExit::Goto(...))` は flow を抜ける用途なので残します。

---

## `docs/01-language/dialogue-character-methods-and-textbox.md`

削除:

```text
textbox= is accepted as a deprecated alias of window=
```

互換不要なので、こう書きます。

```text
The canonical parameter name is `window`.
`textbox=` is not valid in Arcweft source.
```

raw block を修正します。

削除:

```awft
alice.say()[raw]
...
[/raw]
```

置換:

```awft
alice.say()[
    [raw]
    ...
    [/raw]
]
```

escape 表は `\｜` に直します。

---

## `docs/01-language/dialogue-control-tags-and-ruby.md`

raw block と escape 表を同様に直します。

削除:

```text
\| literal ruby bar `｜`
```

置換:

```text
\｜ literal ruby bar `｜`
```

---

## `docs/01-language/hooks-and-memoization.md`

`for` / `on input target` / `on check` の揺れを削除します。

削除:

```awft
for #choice.opening.listen
on input target PointerClick
```

置換:

```awft
on #choice.opening.listen
phase InputTarget
check on input PointerClick
```

削除:

```awft
on check state .affection[#character.alice]
```

置換:

```awft
on state .affection[#character.alice]
phase StateChanged
check on change
```

`inspection できる` も直します。

---

## `docs/examples/hooks-memoization.md`

削除:

```awft
phase = input.hit_test
check = on_change(...)
```

置換:

```awft
phase InputHitTest
check on change state.affection[#character.alice]
```

または:

```awft
phase InputTarget
check on input PointerClick
```

用途に応じて phase 名を PascalCase に統一します。

---

## `docs/03-presentation/render-input-layers.md`

`LayerStack` を削除して `LayerTree` に統一します。

削除:

```rust
pub struct RenderSpec {
    pub layer_stack: LayerStackSpec,
}
```

置換:

```rust
pub struct RenderSpec {
    pub layer_tree: LayerTree,
    pub layer_contents: IndexMap<LayerId, LayerContent>,
}
```

削除:

```text
LayerStack
LayerStackSpec
```

置換:

```text
LayerTree
LayerNode
render_order
input_order
```

---

## `docs/03-presentation/layers-and-input.md`

こちらは `LayerTree` 中心なので、source of truth として残してよいです。
ただし `Layer hooks` 周りで `phase = input.capture`、`hook on input.pointer_enter` などの旧 hook 形が残っているので、hook canonical に合わせて書き換えます。

---

# 20. 最終的な canonical syntax のまとめ

今後 docs の冒頭に載せるなら、このくらいの compact reference がよいです。

```awft
mod game::routes::opening

use game::prelude::*
use game::characters::{alice}

pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    @bg #asset.bg.room fade=300ms
    @show alice normal at=center fade=200ms

    地の文: 扉の向こうから、雨の音がした。[p]

    alice(id=#say.opening.greeting, face=smile, voice=auto):
        おはよう。[p]

    alice.say(id=#say.opening.dream_hint, voice=auto, face=normal)[
        今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]
    ]
    with {
        at(0.45s) { alice.stage.face(worried, crossfade=120ms) }
    }

    let can_enter_alice = state |> has_affection_at_least(#character.alice, 3)

    choice #choice.opening.first {
        #choice.opening.listen "聞いてみる" if can_enter_alice -> #flow.alice_intro
        #choice.opening.silent "黙っている" -> #flow.quiet_intro
    }
}
```

block scope:

```awft
{
    let tmp = route_title(state.route)
    log debug "route={tmp}" { tmp = tmp }
}
```

block expression:

```awft
let title = {
    let route = state.route
    route_title(route)
}
```

line output:

```awft
let (actor, voice) = alice.say(voice=auto)[
    聞いて。[p]
]
with:
    let actor = alice.stage.acquire(scope=line)
    let voice = line.voice_handle()
    out (actor, voice)
```

hook:

```awft
hook #hook.choice_listen_clicked
on #choice.opening.listen
phase InputTarget
check on input PointerClick
when state.flags.contains(.input_enabled)
effects { emit_event, log, input_disposition }
{
    emit GameEvent::ChoiceSelected { id = #choice.opening.listen }
    log info "choice selected {id:?}" { id = #choice.opening.listen }
    stop_propagation
}
```

layer:

```awft
pub layer #layer.choice_ui: GameUi {
    order = ui(20)
    input = capture_on_hit
    hit_test = ui_layout
}
```

---

# 最終判断

このまま進めるなら、まずは **旧表記を全部消す PR** を作るのがよいです。互換を気にしないなら、docs と parser を同時に直して、古い構文は parse error にしてください。

特に最初にやるべき削除はこれです。

```text
- `@choice`
- `say #say... alice ...`
- `alice voice auto:`
- `choice { option ... }`
- line plan 内の `return` による値返し
- `phase = input.target`
- hook の `for #target`
- `LayerStack`
- `textbox=` alias
- `alice.say()[raw] ... [/raw]`
- `\|` ruby bar escape
```

これを消すと、Arcweft の表面構文はかなり締まります。今の段階では「互換のために残す」より、「仕様として存在しない」と切り捨てる方が、parser、formatter、LSP、LLM/RAG のすべてにとって楽になります。
