はい、採用しましょう。整理すると、仕様はこうするのがよいです。

`{ ... }` は **値を返せる expression block** です。現行 docs でも「expression position では最後の式が block の値になる」と書かれているので、この方向で固定してよいです。
そのうえで、名前付き scope があれば、その名前を line ID / choice ID / text key 生成に自動反映します。

# 1. `{}` は値を返せる

これは正式採用でよいです。

```awft
let label = {
    let affection = state.affection[#character.alice]
    if affection >= 3 {
        "聞いてみる"
    } else {
        "まだ聞けない"
    }
}
```

この `{ ... }` は `String` を返します。

statement として使う場合は lexical scope です。

```awft
{
    let tmp = route_title(state.route)
    log debug "route={tmp}" { tmp = tmp }
}
```

`tmp` は外に出ません。

line plan や choice plan のような statement-oriented block では、最後の式で値を返すのではなく、明示的に `out` を使います。

```awft
let handles = alice(id=.greeting)[
    おはよう。[p]
]
with:
    let voice = line.voice_handle()
    out voice
```

# 2. `.rain` 相対 ID は採用

line option の `id=.rain` は採用でよいです。

```awft
地の文(id=.rain):
    扉の向こうから、雨の音がした。[p]
```

正規化:

```awft
#<character.narrator>.say(
    id = #say.opening.narrator.rain,
)[
    扉の向こうから、雨の音がした。[p]
]
```

キャラ台詞も同じです。

```awft
alice(id=.greeting, voice=auto):
    おはよう。[p]
```

正規化:

```awft
alice.say(
    id = #say.opening.alice.greeting,
    voice = auto,
)[
    おはよう。[p]
]
```

基本規則はこれです。

```text
id=.suffix
  -> #say.{current_flow}.{speaker}.{named_scope_path}.{suffix}
```

名前付き scope がない場合は:

```text
#say.{current_flow}.{speaker}.{suffix}
```

# 3. 名前付き `{}` scope を ID に反映する

名前付き scope の canonical syntax は、明示的に `scope` キーワードを使うのがよいです。

```awft
scope rain {
    地の文(id=.sound):
        扉の向こうから、雨の音がした。[p]

    alice(id=.comment):
        雨、強くなってきたね。[p]
}
```

正規化される ID:

```text
地の文:
  #say.opening.narrator.rain.sound
  #text.opening.narrator.rain.sound

alice:
  #say.opening.alice.rain.comment
  #text.opening.alice.rain.comment
```

`id` を省略した場合も、scope 名を自動生成 ID に入れます。

```awft
scope rain {
    地の文:
        扉の向こうから、雨の音がした。[p]
}
```

生成:

```text
#say.opening.narrator.rain.001
#text.opening.narrator.rain.001
```

入れ子も自然に扱います。

```awft
scope rain {
    scope window {
        地の文(id=.rattle):
            窓が小さく鳴った。[p]
    }
}
```

生成:

```text
#say.opening.narrator.rain.window.rattle
#text.opening.narrator.rain.window.rattle
```

つまり、line ID の prefix は次のようになります。

```text
#say.{flow}.{speaker}.{scope_path}.{line_suffix}
```

`scope_path` は空でもよいです。

# 4. choice でも同じ規則を使う

choice も相対 ID と名前付き scope を使えるようにします。

```awft
scope dream {
    choice .first {
        .listen "聞いてみる" -> #flow.alice_intro
        .silent "黙っている" -> #flow.quiet_intro
    }
}
```

正規化:

```text
choice .first
  -> #choice.opening.dream.first

.listen
  -> #choice.opening.dream.first.listen

.silent
  -> #choice.opening.dream.first.silent
```

full form:

```awft
choice #choice.opening.dream.first {
    option #choice.opening.dream.first.listen {
        label = "聞いてみる"
        select { goto #flow.alice_intro }
    }

    option #choice.opening.dream.first.silent {
        label = "黙っている"
        select { goto #flow.quiet_intro }
    }
}
```

短縮形ではこう書けます。

```awft
choice .first {
    .listen "聞いてみる" -> #flow.alice_intro
    .silent "黙っている" -> #flow.quiet_intro
}
```

`choice` 自体の ID 生成規則は:

```text
choice .suffix
  -> #choice.{current_flow}.{scope_path}.{suffix}
```

option の ID 生成規則は:

```text
option .suffix
  -> {current_choice_id}.{suffix}
```

この設計だと、選択肢の階層がかなり自然に出ます。

# 5. `scope` は expression block としても使える

名前付き scope でも値を返せます。

```awft
let can_enter = scope alice_route_check {
    let affection_ok = state.affection[#character.alice] >= 3
    let has_key = state.inventory.contains(#item.alice_key)
    affection_ok && has_key
}
```

この場合、`alice_route_check` は ID namespace に使える scope 名でもあり、block expression の debug / trace / LSP 表示名にも使えます。

ただし、ID に反映されるのは、その scope 内で生成・相対指定された line / choice / option / text key です。

```awft
scope alice_route {
    let can_enter = {
        let affection_ok = state.affection[#character.alice] >= 3
        affection_ok
    }

    choice .first {
        .listen "聞いてみる" if can_enter -> #flow.alice_intro
    }
}
```

生成:

```text
#choice.opening.alice_route.first
#choice.opening.alice_route.first.listen
#text.choice.opening.alice_route.first.listen
```

# 6. `mod` / `use` でも `self` / `super` / `crate` を採用する

現行 docs には `mod game::logic::affection`、`use game::prelude::*`、`pub(super)` などの module / visibility 構文があります。
ここに Rust 風の予約語を正式に入れます。

採用する予約語:

```text
self    current module
super   parent module
crate   current crate / package root
parent  reserved alias of super
```

canonical は `self` / `super` / `crate`。
`parent` は `super` の alias として予約してもよいですが、formatter は `super` に正規化するのがおすすめです。

## mod で使う

```awft
mod crate::game::routes::opening
```

crate root からの絶対指定。

```awft
mod self::routes::opening
```

現在 module からの相対指定。

```awft
mod super::shared
```

親 module からの相対指定。

```awft
mod parent::shared
```

これは許すなら正規化して:

```awft
mod super::shared
```

にします。

# 7. use 文でも同じ規則を使う

`use` でも `self` / `super` / `crate` を使います。

```awft
use crate::game::prelude::*
use self::characters::{alice, bob}
use super::common::{route_gate, shared_flags}
```

`parent` alias を許すなら:

```awft
use parent::common::{route_gate}
```

正規化:

```awft
use super::common::{route_gate}
```

`lazy use` / `eager use` でも同じです。

```awft
lazy use crate::mini_games::truck::{truck_game, TruckResult}
eager use self::generated::route_map::{RouteMap}
```

# 8. `.rain` と `self::foo` は別物にする

ここは混ぜない方がよいです。

```text
.rain
  ID suffix / PublicId relative suffix
  line id, choice id, option id, text key などの文脈で使う

self::foo
super::foo
crate::foo
  module path / import path で使う
```

つまり:

```awft
alice(id=.greeting):
```

は OK。

```awft
use .characters::{alice}
```

は使わない。

`use` では必ず:

```awft
use self::characters::{alice}
```

にします。

# 9. entity ref にも相対指定を入れるか

通常の entity ref は今まで通り完全形でよいです。現行 docs でも `#flow.opening` や `#choice.opening.listen` のような entity ref が基本になっています。

ただし、ID 文脈では `.suffix` を受ける。

```awft
alice(id=.greeting):
```

これは `id` option が `Ref<DialogueLine>` を期待しているので解決できます。

一方、裸の entity ref では:

```awft
goto .next
```

これは曖昧なので、最初は採用しない方がよいです。

使うなら明示的に:

```awft
goto #flow.opening.next
```

または将来:

```awft
goto #.next
```

のような `#.` relative entity ref を追加できますが、今は不要です。

# 10. docs に入れるべき grammar

```ebnf
NamedScope :=
    "scope" Ident BlockExpr

BlockExpr :=
    "{" Item* FinalExpr? "}"

RelativeId :=
    "." IdentPath

ModulePath :=
    ("crate" "::" | "self" "::" | "super" "::" | "parent" "::")? IdentPath

UseItem :=
    Visibility? ("lazy" | "eager")? "use" ModulePath UseTree?

ModItem :=
    "mod" ModulePath
```

line option:

```ebnf
LineIdOption :=
    "id" "=" (EntityRef | RelativeId)
```

choice:

```ebnf
ChoiceId :=
    EntityRef | RelativeId

ChoiceOptionId :=
    EntityRef | RelativeId
```

# 11. 最終的な採用例

```awft
mod crate::game::routes::opening

use crate::game::prelude::*
use self::characters::{alice}
use super::common::{route_gate}

pub flow #flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    scope rain {
        地の文(id=.sound):
            扉の向こうから、雨の音がした。[p]

        alice(id=.comment, voice=auto):
            雨、強くなってきたね。[p]
    }

    scope dream {
        let can_enter = {
            let affection_ok = state.affection[#character.alice] >= 3
            affection_ok
        }

        choice .first {
            .listen "聞いてみる" if can_enter -> #flow.alice_intro
            .silent "黙っている" -> #flow.quiet_intro
        }
    }
}
```

正規化される主要 ID:

```text
#say.opening.narrator.rain.sound
#text.opening.narrator.rain.sound

#say.opening.alice.rain.comment
#text.opening.alice.rain.comment
#voice.ja-JP.alice.opening.rain.comment

#choice.opening.dream.first
#choice.opening.dream.first.listen
#text.choice.opening.dream.first.listen
#choice.opening.dream.first.silent
#text.choice.opening.dream.first.silent
```

この設計なら、ソースは短く書けます。

```awft
alice(id=.comment):
choice .first { .listen "聞いてみる" -> ... }
```

一方で、registry / localization / voice / Agent / LSP では完全修飾 ID を安定して持てます。
