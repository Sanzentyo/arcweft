はい、そこはかなり重要です。`choice` と dialogue は同じ方針で整理するとよいです。

まず、キャラクターのセリフやモノローグは **今の方向性でも ID は付けられる** 形にすべきです。現行 docs でも `alice(id=#say.opening.greeting, face=smile, voice=auto):` のように line options を speaker に渡す設計が入っています。
ただ、locale / text key / voice key の関係がまだ曖昧なので、ここは明文化した方がいいです。現行 localization docs では、line ID、text key、voice key、speaker を別 identity として扱う方針が書かれています。

以下、`choice` と dialogue をまとめて、正規化されるべき記法と sugar を提案します。

# 1. Dialogue / monologue の ID と locale

## 結論

キャラクター台詞もモノローグも、正規形は `speaker.say(...)[...]`。短縮形は `speaker(...):`。
ID は `id=...`、text key は必要なら `text_key=...`、locale は通常 line ごとに書かず、project / source scope で決めるのがよいです。

正規形:

```awft
alice.say(
    id = #say.opening.alice.greeting,
    text_key = #text.opening.alice.greeting,
    voice = auto,
    args = { player_name = state.player_name },
)[
    {player_name}、おはよう。[p]
]
```

短縮形:

```awft
alice(
    id = #say.opening.alice.greeting,
    text_key = #text.opening.alice.greeting,
    voice = auto,
    args = { player_name = state.player_name },
):
    {player_name}、おはよう。[p]
```

もっと短く:

```awft
alice(id=#say.opening.alice.greeting, voice=auto):
    {player_name}、おはよう。[p]
```

`text_key` は省略可能にします。省略した場合は `id` から生成します。

```text
id = #say.opening.alice.greeting
  -> text_key = #text.opening.alice.greeting
  -> voice_key = #voice.{locale}.alice.opening.greeting
```

`id` も省略可能にします。省略した場合は registry に stable ID を生成し、LSP inlay で表示します。これは現行 localization docs の「compiler extracts each dialogue line, narration line, choice label, and UI label into stable text units」という方針と合っています。

---

# 2. モノローグ / 地の文も同じ line option を使う

`地の文:` は built-in narrator character の alias として扱います。なので ID は普通に付けられます。

```awft
地の文(id=#say.opening.narrator.rain):
    扉の向こうから、雨の音がした。[p]
```

これは正規化すると:

```awft
#<character.narrator>.say(
    id = #say.opening.narrator.rain,
)[
    扉の向こうから、雨の音がした。[p]
]
```

text key を明示するなら:

```awft
地の文(
    id = #say.opening.narrator.rain,
    text_key = #text.opening.narrator.rain,
):
    扉の向こうから、雨の音がした。[p]
```

`narrator:` でも同じです。

```awft
narrator(id=#say.opening.narrator.rain):
    扉の向こうから、雨の音がした。[p]
```

---

# 3. locale は line option ではなく source locale と runtime locale に分ける

ここは混ぜると危険です。

## source locale

`.awft` に直接書かれている文字列の言語です。通常は project config で決めます。現行 docs でも `[locale] source = "ja-JP"`、`default = "ja-JP"`、`fallback = ["ja-JP"]` という設定が載っています。

```toml
[locale]
source = "ja-JP"
default = "ja-JP"
fallback = ["ja-JP"]
```

つまり、普通のソースではこう書けば十分です。

```awft
alice(id=#say.opening.alice.greeting):
    おはよう。[p]
```

これは `source_locale = "ja-JP"` の source text として抽出されます。

## runtime locale

実行時に表示する言語です。これは `state.locale` や engine config で決まり、line option には書きません。

```text
state.locale = en-US
  -> #text.opening.alice.greeting の en-US translation を引く
  -> voice.en-US.alice.opening.greeting を探す
```

## per-line source locale override

まれにソース内で別言語を直接書きたい場合だけ、`source_locale` を明示できます。

```awft
alice(
    id = #say.opening.alice.english_quote,
    source_locale = en-US,
):
    Good morning.[p]
```

ただし、これは例外用です。通常は source locale を project / module / file 単位で固定します。

ブロックで source locale を切り替えるなら、こういう構文がよいです。

```awft
source locale en-US {
    alice(id=#say.opening.alice.english_quote):
        Good morning.[p]
}
```

これは lexical scope です。外に出ると元の source locale に戻ります。

---

# 4. dialogue line option の正規仕様

line option は以下にします。

```ebnf
line_option :=
    "id" "=" entity_ref
  | "text_key" "=" entity_ref
  | "voice" "=" expr
  | "window" "=" entity_ref
  | "args" "=" record_expr
  | "source_locale" "=" locale
  | "hooks" "=" list_expr
  | "style" "=" expr
```

例:

```awft
alice(
    id = #say.opening.alice.greeting,
    text_key = #text.opening.alice.greeting,
    voice = auto,
    window = #textbox.0,
    args = { player_name = state.player_name },
):
    {player_name}、おはよう。[p]
```

正規化後:

```awft
alice.say(
    id = #say.opening.alice.greeting,
    text_key = #text.opening.alice.greeting,
    voice = auto,
    window = #textbox.0,
    args = { player_name = state.player_name },
)[
    {player_name}、おはよう。[p]
]
```

`text_key` は localize 用、`id` は narrative entity 用です。現行 localization docs でも LineId、TextKey、VoiceKey は別物として説明されています。

---

# 5. `choice` は dynamic option / UI state / select action を全部扱える必要がある

ここはあなたの指摘どおりです。単なる:

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
}
```

だけだと足りません。

必要なのは次です。

```text
1. 静的 option
2. List / Seq から option 生成
3. Map / HashMap から option 生成
4. 各 option の visible / enabled / disabled_reason / badge / style / hotkey などを UI に渡す
5. option を出すかどうかの判定
6. option を disabled として表示するかどうかの判定
7. 選ばれたときの処理
8. choice 全体の layout / window / timeout / cancel / default focus
9. choice を値として受ける形
10. choice を flow action として直接実行する形
```

なので、`choice` は「選択肢 UI を表示する FlowItem」であると同時に、「選択結果を返せる expression」としても使えるようにします。

---

# 6. `choice` の正規形

正規形はこれです。

```awft
choice #choice.opening.first {
    option #choice.opening.listen {
        label = "聞いてみる"
        enabled = state.affection[#character.alice] >= 3
        visible = true

        ui {
            disabled_reason = "アリスの好感度が足りません"
            badge = "LOCKED"
            style = #style.choice.locked
        }

        select {
            goto #flow.alice_intro
        }
    }

    option #choice.opening.silent {
        label = "黙っている"

        select {
            goto #flow.quiet_intro
        }
    }
}
```

ここでの意味:

```text
option:
  UI に出る選択肢を定義する。

label:
  表示文。localization 抽出対象。

enabled:
  false の場合、表示されるが選べない。

visible:
  false の場合、UI に出ない。

ui:
  UI に伝播する表示状態。

select:
  選ばれたときに実行される flow block。
```

---

# 7. `if` と `enabled` は意味を分ける

重要です。

## `if` は option 自体を生成しない

```awft
choice #choice.opening.first {
    if state.flags.contains(.alice_route_discovered) {
        option #choice.opening.listen {
            label = "聞いてみる"
            select { goto #flow.alice_intro }
        }
    }

    option #choice.opening.silent {
        label = "黙っている"
        select { goto #flow.quiet_intro }
    }
}
```

この場合、条件が false なら `#choice.opening.listen` は UI に存在しません。

## `enabled = false` は disabled option として表示する

```awft
choice #choice.opening.first {
    let can_enter_alice = state.affection[#character.alice] >= 3

    option #choice.opening.listen {
        label = "聞いてみる"
        enabled = can_enter_alice

        ui {
            disabled_reason = if can_enter_alice {
                None
            } else {
                Some("アリスの好感度が足りません")
            }
            badge = if can_enter_alice { None } else { Some("LOCKED") }
        }

        select {
            goto #flow.alice_intro
        }
    }
}
```

この場合、選択肢は見えますが、条件を満たすまで選べません。

---

# 8. `{ ... }` を choice 内のスコープとして使う

`choice` body は lexical scope にします。現行 docs でも `{ ... }` は lexical scope / expression block として整理されています。

```awft
choice #choice.opening.first {
    let affection = state.affection[#character.alice]
    let has_key = state.inventory.contains(#item.alice_key)
    let can_enter = affection >= 3 && has_key

    option #choice.opening.listen {
        label = "聞いてみる"
        enabled = can_enter

        ui {
            disabled_reason = {
                if affection < 3 {
                    Some("アリスの好感度が足りません")
                } else if !has_key {
                    Some("鍵を持っていません")
                } else {
                    None
                }
            }
        }

        select {
            goto #flow.alice_intro
        }
    }
}
```

`enabled = { ... }` も許します。

```awft
enabled = {
    let affection_ok = state.affection[#character.alice] >= 3
    let has_key = state.inventory.contains(#item.alice_key)
    affection_ok && has_key
}
```

これは expression block なので、最後の式が値になります。

---

# 9. List / Seq から option を作る

正規形では、`choice` body の中で通常の `for` を使います。

```awft
let routes = opening_routes(state)

choice #choice.opening.routes {
    for route in routes {
        option route.choice_id {
            label = route.label
            enabled = route.enabled

            ui {
                disabled_reason = route.disabled_reason
                badge = route.badge
                style = route.style
            }

            select {
                goto route.target
            }
        }
    }
}
```

`routes` の型イメージ:

```awft
pub struct RouteChoice {
    choice_id: Ref<ChoiceOption>
    label: LocalizedText
    target: Ref<Flow>
    enabled: Bool
    disabled_reason: Option<LocalizedText>
    badge: Option<String>
    style: Ref<ChoiceStyle>
}
```

`label` が `LocalizedText` なら、そのまま UI に渡せます。`String` でも `DisplayText` として表示できますが、localization 対象にしたいなら `LocalizedText` / `TextKey` を持たせる方がよいです。

---

# 10. HashMap / Map から option を作る

Map なら `.entries()` で key/value を取り出すのが一番分かりやすいです。

```awft
let route_map: Map<Ref<ChoiceOption>, RouteChoice> = opening_route_map(state)

choice #choice.opening.routes {
    for (choice_id, route) in route_map.entries() {
        option choice_id {
            label = route.label
            enabled = route.enabled

            ui {
                disabled_reason = route.disabled_reason
                badge = route.badge
            }

            select {
                goto route.target
            }
        }
    }
}
```

`HashMap` / `Map` から直接 option を作る場合は、順序が問題になります。UI の表示順が必要なので、次のどちらかを必須にした方がよいです。

```awft
for (choice_id, route) in route_map.entries().sort_by(_.value.order) {
    option choice_id {
        label = route.label
        select { goto route.target }
    }
}
```

または:

```awft
option_order = route.order
```

```awft
choice #choice.opening.routes {
    for (choice_id, route) in route_map.entries() {
        option choice_id {
            label = route.label
            order = route.order
            select { goto route.target }
        }
    }
}
```

Map は iteration order が不安定になりやすいので、formatter / LSP は `order` か sort を要求するのがよいです。

---

# 11. UI へ伝播する option state

各 option は lowering 後、UI に `ChoiceOptionView` として渡されます。

```awft
pub struct ChoiceOptionView {
    id: Ref<ChoiceOption>
    label: RichText
    text_key: Option<Ref<Text>>
    value: Option<Value>

    visible: Bool
    enabled: Bool
    selected: Bool

    disabled_reason: Option<RichText>
    tooltip: Option<RichText>
    badge: Option<RichText>
    style: Option<Ref<ChoiceStyle>>
    hotkey: Option<InputBinding>
    order: i32

    metadata: Map<String, Value>
}
```

source syntax:

```awft
option #choice.opening.listen {
    label = "聞いてみる"
    enabled = can_enter_alice
    visible = true
    order = 10
    hotkey = key Enter

    ui {
        disabled_reason = if can_enter_alice { None } else { Some("好感度が足りません") }
        tooltip = "アリスに事情を聞きます"
        badge = if can_enter_alice { None } else { Some("LOCKED") }
        style = if can_enter_alice { #style.choice.normal } else { #style.choice.locked }
        metadata = {
            route = #flow.alice_intro,
            affection_required = 3,
        }
    }

    select {
        goto #flow.alice_intro
    }
}
```

`enabled` と `visible` は semantic state。
`ui { ... }` は rendering / accessibility / Agent observation 用の state。

Agent / test / LSP からは次のように見えるのが理想です。

```text
choice.opening.listen
  label: 聞いてみる
  visible: true
  enabled: false
  disabled_reason: アリスの好感度が足りません
  target: flow.alice_intro
```

---

# 12. `choice` の `with` block

dialogue では `with { ... }` が canonical で、`with:` は indentation sugar です。現行 docs でも `with {}` が正規形、`with:` が sugar と整理されています。
`choice` も同じにします。

`choice { ... }` は option 定義のスコープ。
`with { ... }` は choice 全体の lifecycle plan。

```awft
choice #choice.opening.first {
    option #choice.opening.listen {
        label = "聞いてみる"
        select { goto #flow.alice_intro }
    }

    option #choice.opening.silent {
        label = "黙っている"
        select { goto #flow.quiet_intro }
    }
}
with {
    window = #choice_window.main
    layout = vertical
    default_focus = #choice.opening.listen

    cancel on input .BackToTitle {
        return Ok(FlowExit::Goto(#flow.title))
    }

    timeout 10s {
        select #choice.opening.silent
    }

    on select selected {
        log info "choice selected {id:?}" { id = selected.id }
    }
}
```

indent sugar:

```awft
choice #choice.opening.first:
    option #choice.opening.listen:
        label = "聞いてみる"
        select:
            goto #flow.alice_intro

    option #choice.opening.silent:
        label = "黙っている"
        select:
            goto #flow.quiet_intro
with:
    window = #choice_window.main
    layout = vertical
    default_focus = #choice.opening.listen
```

ただし、正規化後は brace form にします。

```awft
choice #choice.opening.first {
    ...
}
with {
    ...
}
```

---

# 13. `choice` を値として使う

選択結果を値として受けたい場合があります。そのために、`select { out expr }` を許します。現行 docs では `out` は line plan / cue block / content scope から値を出す用途として定義されています。
これを choice continuation にも拡張するのが自然です。

```awft
let next_flow = choice #choice.opening.first {
    option #choice.opening.listen {
        label = "聞いてみる"
        select { out #flow.alice_intro }
    }

    option #choice.opening.silent {
        label = "黙っている"
        select { out #flow.quiet_intro }
    }
}

goto next_flow
```

これを sugar で書くなら、`=>` を使うのが分かりやすいです。

```awft
let next_flow = choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" => #flow.alice_intro
    #choice.opening.silent "黙っている" => #flow.quiet_intro
}

goto next_flow
```

ここで:

```text
->  flow action / goto sugar
=>  value output sugar
```

にします。

---

# 14. `->` と `=>` の使い分け

## `->` は選ばれたら flow を進める

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

正規化:

```awft
choice #choice.opening.first {
    option #choice.opening.listen {
        label = "聞いてみる"
        select {
            goto #flow.alice_intro
        }
    }

    option #choice.opening.silent {
        label = "黙っている"
        select {
            goto #flow.quiet_intro
        }
    }
}
```

## `=>` は選ばれた値を返す

```awft
let selected_route = choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" => #flow.alice_intro
    #choice.opening.silent "黙っている" => #flow.quiet_intro
}

goto selected_route
```

正規化:

```awft
let selected_route = choice #choice.opening.first {
    option #choice.opening.listen {
        label = "聞いてみる"
        select {
            out #flow.alice_intro
        }
    }

    option #choice.opening.silent {
        label = "黙っている"
        select {
            out #flow.quiet_intro
        }
    }
}
```

この2つを分けると、`choice` が statement なのか expression なのかが読みやすくなります。

---

# 15. static option の sugar

よく使う機能は短く書けるようにします。

## 直接 goto

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

## enabled 条件付き

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" if can_enter_alice -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

正規化:

```awft
option #choice.opening.listen {
    label = "聞いてみる"
    enabled = can_enter_alice
    select { goto #flow.alice_intro }
}
```

ここで `if` は「option を消す」ではなく、「enabled 条件」として sugar にするか、「存在条件」として使うかで迷います。
おすすめは、line sugar の `if` は **enabled 条件** にすることです。理由は VN の選択肢では「見えるが選べない」が多いからです。

存在条件は block の `if` を使います。

```awft
choice #choice.opening.first {
    if state.flags.contains(.alice_route_discovered) {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    }
}
```

整理すると:

```text
arm inline `if`:
  enabled condition

block `if`:
  option existence condition
```

---

# 16. disabled reason 付き sugar

これは頻出しそうなので sugar があると便利です。

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる"
        if can_enter_alice
        else "アリスの好感度が足りません"
        -> #flow.alice_intro
}
```

正規化:

```awft
option #choice.opening.listen {
    label = "聞いてみる"
    enabled = can_enter_alice

    ui {
        disabled_reason = if can_enter_alice {
            None
        } else {
            Some("アリスの好感度が足りません")
        }
    }

    select {
        goto #flow.alice_intro
    }
}
```

ただ、これは少し文法が重くなるので、最初は full form だけでもよいです。

```awft
option #choice.opening.listen {
    label = "聞いてみる"
    enabled = can_enter_alice
    ui {
        disabled_reason = if can_enter_alice { None } else { Some("アリスの好感度が足りません") }
    }
    select { goto #flow.alice_intro }
}
```

---

# 17. dynamic option の sugar

`option route in routes` を sugar として許すとかなり書きやすいです。

```awft
choice #choice.opening.routes {
    option route in opening_routes(state) {
        id = route.choice_id
        label = route.label
        enabled = route.enabled

        ui {
            disabled_reason = route.disabled_reason
            badge = route.badge
        }

        select {
            goto route.target
        }
    }
}
```

正規化:

```awft
choice #choice.opening.routes {
    for route in opening_routes(state) {
        option route.choice_id {
            label = route.label
            enabled = route.enabled

            ui {
                disabled_reason = route.disabled_reason
                badge = route.badge
            }

            select {
                goto route.target
            }
        }
    }
}
```

つまり、canonical は `for` + `option`。
sugar は `option x in xs`.

Map 用:

```awft
choice #choice.opening.routes {
    option (choice_id, route) in route_map.entries().sort_by(_.value.order) {
        id = choice_id
        label = route.label
        enabled = route.enabled
        select { goto route.target }
    }
}
```

正規化:

```awft
for (choice_id, route) in route_map.entries().sort_by(_.value.order) {
    option choice_id {
        label = route.label
        enabled = route.enabled
        select { goto route.target }
    }
}
```

---

# 18. choice label の ID / locale

choice label も localization 対象です。現行 localization docs でも dialogue line、narration line、choice label、UI label を抽出対象にしています。

静的 option では、option ID から text key を生成します。

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
}
```

抽出:

```text
choice id:
  #choice.opening.listen

text key:
  #text.choice.opening.listen

source text:
  聞いてみる

source locale:
  ja-JP
```

明示したい場合:

```awft
option #choice.opening.listen {
    label(id=#text.choice.opening.listen) = "聞いてみる"
    select { goto #flow.alice_intro }
}
```

ただし、通常は不要です。`#choice.opening.listen` から `#text.choice.opening.listen` を導出すればよいです。

dynamic option の場合は、label が `LocalizedText` / `TextKey` / `RichText` を持っている必要があります。

```awft
option route.choice_id {
    label = route.label  // LocalizedText
    select { goto route.target }
}
```

`route.label` がただの runtime `String` なら、翻訳抽出対象にはしません。LSP は warning を出すのがよいです。

```text
warning[CHOICE_DYNAMIC_LABEL_NOT_LOCALIZABLE]:
  Dynamic choice label is a String. Use LocalizedText or provide text_key.
```

---

# 19. `choice` の型と lowering

HIR ではこういう形に落とします。

```awft
pub struct ChoicePlan<R> {
    id: Ref<Choice>
    options: List<ChoiceOptionPlan<R>>
    window: Ref<ChoiceWindow>
    layout: ChoiceLayout
    default_focus: Option<Ref<ChoiceOption>>
    cancel_rules: List<ChoiceCancelRule>
    timeout: Option<ChoiceTimeout>
}

pub struct ChoiceOptionPlan<R> {
    id: Ref<ChoiceOption>
    label: RichText
    text_key: Option<Ref<Text>>
    value_type: Type<R>

    visible: Bool
    enabled: Bool
    order: i32

    ui: ChoiceOptionUiState
    select: ChoiceSelectBlock<R>
}

pub struct ChoiceOptionUiState {
    disabled_reason: Option<RichText>
    tooltip: Option<RichText>
    badge: Option<RichText>
    style: Option<Ref<ChoiceStyle>>
    hotkey: Option<InputBinding>
    metadata: Map<String, Value>
}
```

statement choice:

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
}
```

は `ChoicePlan<! or Unit>` 的に、選択後に flow control が発生します。

expression choice:

```awft
let next = choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" => #flow.alice_intro
}
```

は `ChoicePlan<Ref<Flow>>` です。

---

# 20. choice lifecycle の実行順

実行順は明文化した方がよいです。

```text
1. choice body の lexical scope を作る
2. let / if / for を評価して option candidates を作る
3. 各 option の visible / enabled / ui state を評価する
4. visible = true の option を UI に渡す
5. flow は choice input 待ちで suspend する
6. state / signal が変わった場合、依存する option state を再評価する
7. player / Agent / test が enabled option を選ぶ
8. choice-level `on select selected` を実行する
9. selected option の `select { ... }` を実行する
10. `goto` / `return` / `out` / normal completion に従って flow を進める
```

重要なのは 6 です。option の状態は UI に伝播される必要があるので、`enabled` や `ui.disabled_reason` は display 時に一度だけ計算するのではなく、依存 state が変われば再計算できるようにします。

高コストなら memo を使います。

```awft
let choice_state = memo(scope=frame, key=(state.flags, state.affection[#character.alice])) {
    compute_choice_state(state)
}
```

---

# 21. よく使うパターン集

## シンプルな VN 選択肢

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

## 条件付きで disabled 表示

```awft
let can_enter_alice = state.affection[#character.alice] >= 3

choice #choice.opening.first {
    option #choice.opening.listen {
        label = "聞いてみる"
        enabled = can_enter_alice

        ui {
            disabled_reason = if can_enter_alice {
                None
            } else {
                Some("アリスの好感度が足りません")
            }
            badge = if can_enter_alice { None } else { Some("LOCKED") }
        }

        select {
            goto #flow.alice_intro
        }
    }

    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

## 条件を満たすまで option 自体を出さない

```awft
choice #choice.opening.first {
    if state.flags.contains(.alice_route_discovered) {
        #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    }

    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

## List から選択肢生成

```awft
let routes = opening_routes(state)

choice #choice.opening.routes {
    for route in routes {
        option route.choice_id {
            label = route.label
            enabled = route.enabled

            ui {
                disabled_reason = route.disabled_reason
                badge = route.badge
                style = route.style
            }

            select {
                goto route.target
            }
        }
    }
}
```

## Map から選択肢生成

```awft
choice #choice.opening.routes {
    for (choice_id, route) in route_map.entries().sort_by(_.value.order) {
        option choice_id {
            label = route.label
            enabled = route.enabled
            order = route.order

            select {
                goto route.target
            }
        }
    }
}
```

## 値として受ける

```awft
let next_flow = choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" => #flow.alice_intro
    #choice.opening.silent "黙っている" => #flow.quiet_intro
}

goto next_flow
```

## 選択時に複数処理

```awft
choice #choice.opening.first {
    option #choice.opening.listen {
        label = "聞いてみる"
        enabled = state.affection[#character.alice] >= 3

        select {
            emit GameEvent::ChoiceSelected { id = #choice.opening.listen }
            state.flags += .asked_alice_about_dream
            goto #flow.alice_intro
        }
    }
}
```

## choice 全体の UI plan

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
with {
    window = #choice_window.main
    layout = vertical
    default_focus = #choice.opening.listen

    timeout 10s {
        select #choice.opening.silent
    }

    cancel on input .BackToTitle {
        return Ok(FlowExit::Goto(#flow.title))
    }

    on select selected {
        log info "selected choice {id:?}" { id = selected.id }
    }
}
```

---

# 22. 提案する grammar

```ebnf
choice_expr :=
    "choice" entity_ref? choice_body choice_plan?

choice_body :=
    "{" choice_item* "}"
  | ":" newline indent choice_item* dedent

choice_item :=
    let_stmt
  | if_stmt
  | match_stmt
  | for_stmt
  | option_item
  | option_for_sugar
  | choice_arm_sugar

option_item :=
    "option" option_id option_body

option_id :=
    entity_ref
  | expr

option_body :=
    "{" option_item_body* "}"
  | ":" newline indent option_item_body* dedent

option_item_body :=
    "label" "=" expr
  | "label" "(" "id" "=" entity_ref ")" "=" expr
  | "value" "=" expr
  | "visible" "=" expr
  | "enabled" "=" expr
  | "order" "=" expr
  | "hotkey" "=" expr
  | "ui" ui_block
  | "select" flow_block
  | let_stmt

option_for_sugar :=
    "option" pattern "in" expr option_body

choice_arm_sugar :=
    entity_ref string choice_arm_condition? choice_arm_action

choice_arm_condition :=
    "if" expr

choice_arm_action :=
    "->" entity_ref
  | "=>" expr

choice_plan :=
    "with" block
  | "with" ":" newline indent choice_plan_item* dedent

choice_plan_item :=
    "window" "=" expr
  | "layout" "=" expr
  | "default_focus" "=" expr
  | "timeout" duration flow_block
  | "cancel" "on" cancel_trigger flow_block
  | "on" "select" pattern flow_block
```

正規化:

```text
with:
  -> with { ... }

option x in xs:
  -> for x in xs { option ... }

id "label" if cond -> target:
  -> option id { label = "label"; enabled = cond; select { goto target } }

id "label" => value:
  -> option id { label = "label"; select { out value } }
```

---

# 23. docs に明記すべき設計判断

このまま仕様にするなら、次を docs に書くとブレにくいです。

```text
1. `choice` is a FlowItem and may also be used as an expression.
2. `choice #id { ... }` is canonical. `@choice` is not part of the grammar.
3. A `choice` body is a lexical scope.
4. `option` creates a candidate option.
5. `if` around an option controls existence.
6. `enabled = expr` controls selectability while preserving UI visibility.
7. `visible = expr` controls whether the option is rendered.
8. `ui { ... }` fields are propagated to ChoiceOptionView and Agent observation.
9. `select { ... }` is the selected option's flow block.
10. `-> target` is sugar for `select { goto target }`.
11. `=> value` is sugar for `select { out value }`.
12. `choice ... with { ... }` attaches a choice lifecycle plan.
13. `with:` is indentation sugar for `with { ... }`.
14. Static choice labels are localization extraction targets.
15. Dynamic labels must be LocalizedText/TextKey/RichText if they should be localizable.
```

---

# 24. dialogue と choice の対応関係

この整理にすると、dialogue と choice がきれいに対応します。

```text
dialogue:
  alice.say(args)[content]
  with { line plan }
  out expr from line plan

choice:
  choice #id { options }
  with { choice plan }
  out expr from selected option / choice continuation
```

dialogue の短縮形:

```awft
alice(id=#say.opening.greeting):
    おはよう。[p]
```

choice の短縮形:

```awft
choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
}
```

dialogue の localization:

```text
LineId  #say.opening.greeting
TextKey #text.opening.greeting
VoiceKey voice.ja-JP.alice.opening.greeting
```

choice の localization:

```text
ChoiceOptionId #choice.opening.listen
TextKey        #text.choice.opening.listen
SourceText     "聞いてみる"
```

この対応にすると、LSP inlay、locale extraction、Agent observation、UI rendering がすべて同じ考え方で扱えます。
