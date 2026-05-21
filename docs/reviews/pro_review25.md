はい。`line` / `rich` だけでなく、「本来は通常の関数呼び出し・値構築・メソッド呼び出しで済むのに、独自文法っぽく残っているもの」がいくつかあります。

前提として、arcweft は `.arcw DSL` を持ちつつ、実行意味論は Typed IR / bytecode VM を正本にする構造で、Core は Sans I/O、外部実行は Host / Adapter に逃がす設計です 。また、現在のシナリオ構文は「通常の effectful function calls」「compact dialogue sugar」「canonical character method calls」を同じ `flow` の中に置く方針で、古い `@bg` / `@show` 的なコマンド家族は stable grammar ではない、と明記されています  。なので、判断基準はかなり明確で、**制御構文・宣言構文・会話 sugar 以外は、できるだけ `foo(...)` / `obj.method(...)` / `foo { ... }` の汎用形に寄せる**のがよさそうです。

## 結論

`line` と `rich` は直すべきです。

特に `rich` は現在の `docs/03-presentation/text-typesetting.md` に、

```arcw
say alice rich """
今日は少しだけ、{ruby "変な夢" "へんなゆめ"}を見たんだ。
"""
```

という形で残っており、これは `say` / `alice` / `rich` が空白区切りの専用コマンド列に見えます 。これは現在の `alice.say(...)[...]` 方針と衝突します。

修正案はこうです。

```arcw
alice.say()[
    rich("""
    今日は少しだけ、#[ruby("変な夢", "へんなゆめ")]を見たんだ。
    """)
]
```

または、dialogue content 自体が RichText として扱われるなら、さらに素直に、

```arcw
alice.say()[
    今日は少しだけ、#[ruby("変な夢", "へんなゆめ")]を見たんだ。[p]
]
```

でよいと思います。`rich(...)` は「通常文字列から RichText を作る関数」としてだけ残し、`say alice rich ...` というコマンド構文は削除するのが一貫しています。

## `line` は 2 種類残っています

1つ目は parser combinator 文書の `many line { ... }` です。

```arcw
pub parser parse_agent_script: Parser<Vec<AgentScriptCommand>, ParseError> {
    many line {
        alt {
            "observe" => AgentScriptCommand::Observe,
            "choose" ws target: ref_id<ChoiceOption>() =>
                AgentScriptCommand::Choose { target },
            "wait signal" ws sig: ref_id<Signal>() ws op: compare_op() ws value: value() =>
                AgentScriptCommand::WaitSignal { signal: sig, op, value },
        }
    }
}
```

これは `many` と `line` と `ws` と `alt` が全部「パーサ専用文法」に見えます 。`line` は最低でも `line()` にするべきです。より徹底するならこうです。

```arcw
pub parser parse_agent_script: Parser<Vec<AgentScriptCommand>, ParseError> {
    many(line(
        alt([
            pattern("observe")
                .map(|| AgentScriptCommand::Observe),

            pattern("choose")
                .then(ws())
                .then_bind("target", ref_id<ChoiceOption>())
                .map(|target| AgentScriptCommand::Choose { target }),

            pattern("wait signal")
                .then(ws())
                .then_bind("signal", ref_id<Signal>())
                .then(ws())
                .then_bind("op", compare_op())
                .then(ws())
                .then_bind("value", value())
                .map(|signal, op, value| AgentScriptCommand::WaitSignal {
                    signal,
                    op,
                    value,
                }),
        ])
    ))
}
```

ただ、これはやや重いので、arcweft らしい折衷案としては次がよいと思います。

```arcw
pub parser parse_agent_script: Parser<Vec<AgentScriptCommand>, ParseError> {
    many(line()) {
        alt {
            "observe" => AgentScriptCommand::Observe,

            "choose" ws() target: ref_id<ChoiceOption>() =>
                AgentScriptCommand::Choose { target },

            "wait signal" ws() sig: ref_id<Signal>() ws() op: compare_op() ws() value: value() =>
                AgentScriptCommand::WaitSignal { signal: sig, op, value },
        }
    }
}
```

この場合、`many(...) { ... }` は「関数呼び出し + ブロック引数」として扱えます。`line` / `ws` は関数化され、特殊さがかなり減ります。

2つ目は flat fence の `=== line ... ===` です。grammar には `FlatLine := '=== line' DialogueCallee '===' DialogueContent FlatWith? '=== /line ==='` が残っており、`FlatThread` / `FlatDefer` / `FlatScope` も同列にあります 。実装側でも `parse_flat_flow_item` が `line` / `thread` / `defer` / `scope` fence を直接 dispatch しています 。LSP も flat `=== line ... ===` の ID 挿入を扱う前提になっています 。

これは「authoring sugar」として許すにしても、stable grammar からは外した方がよいです。修正案は2段階です。

まず formatter / LSP の正規化先を、

```arcw
=== line alice(id=@.greeting) ===
おはよう。[p]
=== with ===
at(0.42s) { alice.stage.look(smile) }
=== /with ===
=== /line ===
```

から、

```arcw
alice.say(id=@.greeting)[
    おはよう。[p]
]
with {
    at(0.42s) {
        alice.stage.look(smile)
    }
}
```

にします。さらに長文 raw 入力が欲しい場合は、専用 fence ではなく関数に寄せます。

```arcw
alice.say(id=@.greeting)[
    raw_text("""
    おはよう。[p]
    """)
]
with {
    at(0.42s) {
        alice.stage.look(smile)
    }
}
```

つまり `=== line` は **取り込み用・移行用の非 stable sugar** に落とし、AST/HIR の canonical source には出さないのがよいです。

## ほかに直すべき候補

### 1. `ref bg(...)` / `clear bg(...)`

文書には次のような形が残っています。

```arcw
let previous_bg = bg(@asset.bg.room, target = @target.scene, slot = @slot.background.main)
let current_bg = ref bg(target = @target.scene, slot = @slot.background.main)
let cleared_bg = clear bg(target = @target.scene, slot = @slot.background.main)
```

これは「staging は ordinary effectful calls」と言いつつ、`ref bg` / `clear bg` が専用前置構文になっています 。grammar でも `StagingRef := 'ref' ('bg' | 'show') CallArgs`、`StagingClear := 'clear' 'bg' CallArgs | 'hide' CallArgs` と特殊扱いです 。

修正案は、ペアになる API を関数・メソッドに統一します。

```arcw
let previous_bg = bg.set(@asset.bg.room, target=@target.scene, slot=@slot.background.main)
let current_bg = bg.ref(target=@target.scene, slot=@slot.background.main)
let cleared_bg = bg.clear(target=@target.scene, slot=@slot.background.main)
```

または、既存の `bg(...)` を set として残すなら、

```arcw
let previous_bg = bg(@asset.bg.room, target=@target.scene, slot=@slot.background.main)
let current_bg = ref_bg(target=@target.scene, slot=@slot.background.main)
let cleared_bg = clear_bg(target=@target.scene, slot=@slot.background.main)
```

ただし後者は名前が増えます。型付き target/slot API なら `bg.ref(...)` / `bg.clear(...)` の方が自然です。

### 2. `memo rich_text key=(...) cache=flow`

line plan 内に、

```arcw
memo rich_text key=(line.id, locale, theme.text_hash) cache=flow
memo voice_cue key=(voice.key, locale) cache=session
```

が残っています 。実装でも `parse_line_plan_memo` が `memo ` 以降を空白分割し、最初の語を name、以降を `key=...` のような options として読んでいます  。

これはかなりコマンド DSL です。修正案は、

```arcw
memo(rich_text, key=(line.id, locale, theme.text_hash), cache=flow)
memo(voice_cue, key=(voice.key, locale), cache=session)
```

または name も enum / symbol にするなら、

```arcw
memo(.rich_text, key=(line.id, locale, theme.text_hash), cache=.flow)
memo(.voice_cue, key=(voice.key, locale), cache=.session)
```

にするのがよいです。`memo` は line-plan item ではなく普通の line-plan-safe 関数呼び出しとして扱えます。

### 3. `at(phoneme "a")` / `at(char 12)` / `wait mark .release_focus`

`at(...)` 自体は関数呼び出し形なのでよいのですが、引数の中が `phoneme "a"` / `char 12` / `word 3` になっています。

```arcw
at(phoneme "a") { alice.stage.mouth(a) }
at(char 12) { signal.set(@signal.text_reveal_hit, true) }
```

この形は grammar の `TriggerPattern` / line-plan anchor 周辺と同じく、空白区切りの小 DSL です 。修正案は、

```arcw
at(phoneme("a")) {
    alice.stage.mouth(a)
}

at(char(12)) {
    signal.set(@signal.text_reveal_hit, true)
}

at(word(3)) {
    ...
}
```

`wait mark .release_focus` も同じで、

```arcw
wait(mark(.release_focus))
wait(350ms)
```

にした方が `line()` / `rich()` 方針と合います。

### 4. `cancel on input .SkipLine` / `on .release_focus`

line plan parser は `input ` / `event ` / `signal ` / `timeout ` / `mark ` / `select ` などを文字列 prefix として読んでいます 。これは読みやすいですが、`input .SkipLine` が通常式ではありません。

現状維持してもよい候補ですが、完全に関数化するなら、

```arcw
cancel on input(.SkipLine) {
    text.flush(mode=.Instant)
    continue
}

on mark(.release_focus) {
    'line.focus |> drop
    out .Released
}
```

または、

```arcw
cancel(input(.SkipLine)) {
    text.flush(mode=.Instant)
    continue
}

on(mark(.release_focus)) {
    'line.focus |> drop
    out .Released
}
```

です。私は前者を推します。`cancel on` / `on` は制御構文として残し、trigger 部分だけ関数化する折衷案です。

### 5. `start { ... }` / `together { ... }`

これは line plan 専用 block item として残っています。実装上も `start` / `together` は `LinePlanItem::StartGroup` / `TogetherGroup` に直結しています 。ここは「制御構文」として許すか、「通常関数」として寄せるかの境目です。

関数化するなら、

```arcw
start {
    together {
        alice.stage.move(to=left, time=300ms)
        alice.stage.look(panic, crossfade=80ms)
    }
}
```

を、

```arcw
start([
    together([
        alice.stage.move(to=left, time=300ms),
        alice.stage.look(panic, crossfade=80ms),
    ])
])
```

にすると読みにくいです。ここはむしろ `with { ... }` 内の構造化制御として残してよいと思います。`line()` / `rich()` と同列に直す対象ではありません。

### 6. `option c.id c.label`

`docs/01-language/traits-seq-ranges.md` に、

```arcw
for c in choices {
    option c.id c.label
}
```

が残っています 。これは現在の grammar にある `OptionItem := 'option' OptionId OptionBody` や `OptionForSugar := 'option' Pattern 'in' Expr OptionBody` ともズレます 。

修正案は、

```arcw
for c in choices {
    option c.id {
        label = c.label
        value = c.value
    }
}
```

または choice body の式寄り API にするなら、

```arcw
for c in choices {
    option(id=c.id, label=c.label, value=c.value)
}
```

ただし `choice` の ID 抽出・localization 抽出を考えると、前者の block form の方が安全です。

### 7. `typeset @typeset.credits typst { source """...""" page width = ... }`

`Text / RichText / Typst` 文書には、

```arcw
pub typeset @typeset.credits typst {
    source """
    ...
    """
    page width = 720pt
    page height = auto
}
```

が残っています 。これは宣言文法としてはあり得ますが、`source """..."""` と `page width = ...` はかなり特殊です。

修正案は、宣言を残すなら中身だけ通常の assignment に寄せます。

```arcw
pub typeset @typeset.credits {
    engine = typst
    source = """
    @set text(font: "Noto Serif CJK JP", size: 18pt)
    #align(center)[
      = Staff

      Scenario: Alice \\
      Engine: Arcweft
    ]
    """
    page.width = 720pt
    page.height = auto
}
```

さらに関数化するなら、

```arcw
pub const credits_typeset =
    typeset(
        id=@typeset.credits,
        engine=typst,
        source="""
        ...
        """,
        page=PageSpec(width=720pt, height=auto),
    )
```

ただ、`typeset` が registry item なら宣言形を残す意味はあります。その場合でも `source = ...` / `page.width = ...` に揃えるのがよいです。

## 逆に、残してよい特殊構文

以下は「消すべき残骸」ではなく、言語の中核構文として残してよいと思います。

`flow` / `fragment` / `fn` / `source` / `hook` / `parser` などの item declaration、`if` / `match` / `for` / `while` / `loop` / `return` / `break` / `continue` / `yield`、`await ... with { ... }`、`choice { ... }` と `option { ... }`、`speaker:` と `speaker.say()[...]` の dialogue sugar です。grammar 自体もこれらを canonical surface grammar として整理しています  。

ただし `choice` 内でも `option c.id c.label` のような古い空白コマンド型は削るべきです。

## 具体的な修正方針

まず文書・grammar・parser を次のように揃えるのがよいです。

| 対象                 | 現状                                     | 修正案                                                       |
| ------------------ | -------------------------------------- | --------------------------------------------------------- |
| RichText           | `say alice rich """..."""`             | `alice.say()[ rich("""...""") ]` または dialogue content 直書き |
| Parser line        | `many line { ... }`                    | `many(line()) { ... }`                                    |
| Parser whitespace  | `"choose" ws target: ...`              | `"choose" ws() target: ...`                               |
| Flat dialogue      | `=== line alice === ... === /line ===` | 非 stable sugar。正規化先は `alice.say()[...] with { ... }`      |
| Staging read/clear | `ref bg(...)`, `clear bg(...)`         | `bg.ref(...)`, `bg.clear(...)`                            |
| Memo               | `memo rich_text key=... cache=...`     | `memo(.rich_text, key=..., cache=.flow)`                  |
| Timed anchors      | `at(phoneme "a")`, `at(char 12)`       | `at(phoneme("a"))`, `at(char(12))`                        |
| Wait marker        | `wait mark .release_focus`             | `wait(mark(.release_focus))`                              |
| Dynamic option     | `option c.id c.label`                  | `option c.id { label = c.label }`                         |
| Typeset block      | `source """..."""`, `page width = ...` | `source = """..."""`, `page.width = ...`                  |

## 実装側の変更案

最小パッチ方針はこの順番です。

1. **docs の canonical examples を先に修正する。**
   `docs/03-presentation/text-typesetting.md`, `docs/01-language/parsing.md`, `docs/01-language/grammar.md`, `docs/01-language/traits-seq-ranges.md`, `docs/01-language/dialogue-calls-scopes-cancellation.md`, `docs/01-language/scenario-surface-syntax.md` が主対象です。

2. **grammar から `FlatLine` を stable から外す。**
   残すなら「legacy/import-only sugar」と明記します。現在は grammar に `FlatLine` / `FlatThread` / `FlatDefer` / `FlatScope` が canonical っぽく載っているので、ここは誤解を生みます 。

3. **parser は当面受け付けるが diagnostic を出す。**
   `parse_flat_flow_item` は残してもよいですが、`=== line` を見たら「canonical は `speaker.say()[...]`」という warning / code action を出すのがよいです。実装ではすでに flat fence を専用 dispatch しているので、ここに migration diagnostic を入れられます 。

4. **`parse_line_plan_memo` を `memo(...)` 優先にする。**
   旧 `memo rich_text key=...` は warning にし、formatter は `memo(.rich_text, key=..., cache=.flow)` へ変換します。

5. **`ref bg` / `clear bg` を expression parser から段階的に外す。**
   まず `bg.ref(...)` / `bg.clear(...)` を追加し、旧構文に warning。formatter で自動変換。

## いちばん大事な設計線引き

`line()` / `rich()` 方針は正しいですが、全部を関数にすると `if(...)`, `match(...)`, `choice(...)` のように読みづらくなります。なので線引きはこう置くのがよいです。

**残す特殊構文:** 制御、宣言、scope、dialogue sugar、choice UI sugar。
**関数化するもの:** parser combinator、RichText constructor、staging slot operation、memo declaration、timeline anchor、marker/wait trigger、古い flat fence。

この線引きなら、arcweft の「会話は簡潔、複雑なものは通常の型付き call に展開」という現在の方針と噛み合います。
