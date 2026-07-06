# ID と参照

## 4層構造

```text
EntityId     renameしても変わらない内部実体ID
PublicId     DSL上の名前。rename可能
DisplayId    LSP inlay hintやUI表示用
SemanticHash 内容・意味のfingerprint
```

`PublicId` はユーザーが扱う名前だが、履歴・RAG・GraphPatch は `EntityId + SemanticHash` を主に使う。

## 書き方

通常:

```arcw
@flow.opening
@choice.opening.listen
@asset:.bg.room
@state.GameState.affection
```

境界明示:

```arcw
@<activity.truck_game>.run(...)
@<flow.alice_intro@jj:qtnqlkkm>
@<say.opening.dream_hint@sem:b3_9f2a1c>
@<ent:01J8X6K9XW4M9F2D7A1R8QZ6CN>
```

`@` is a surface sigil only. The stored `PublicId` body does not include it:
`@flow.opening` stores `flow.opening`.

Reference expressions have two accepted surfaces. Hand-written source should
omit the default family from the referenced id path and prefer
family-relative references such as `@asset:.bg.room` instead of repeating the
absolute family in `@asset.bg.room`. Runtime calls such as `bg(...)` and
`image(asset = ...)` are examples, but the rule is not limited to typed
arguments: `@asset:.bg.room` still names the `asset` family explicitly. The
family anchor remains explicit in the reference spelling, so use
`@asset:.bg.room`, not `@.bg.room`, in expression positions. Fully qualified
forms such as `@asset.bg.room`, `@flow.opening`, or
`@state.GameState.affection` remain valid for generated surfaces,
manifest/tooling output, stored public-id roundtrips, and external interfaces
that need the stored public id verbatim. They are not the recommended spelling
for ordinary hand-authored asset references when the family has a default
public-id prefix.

Declaration headers are a separate surface. There, the declaration keyword
already supplies the family, so hand-written source should omit the default
family prefix and prefer compact declaration ids such as `asset bg_room { ... }`
and `content chapter_two { ... }`. Fully qualified declaration headers such as
`asset @asset.bg_room { ... }` are accepted for generated or fully elaborated
source and lint toward the compact authoring form.

`#` is reserved for Rust-like attributes in the `#[...]` form and is not an
entity-reference marker.

コメント:

```arcw
/// [[flow.alice_intro]]
/// [[soft:flow.alice_intro]]
/// [[say.opening.dream_hint@sem:b3_9f2a1c]]
```

## 参照レベル

```arcw
pub enum ReferenceLevel {
    Mention,
    Soft,
    Checked,
    Runtime,
    Contract,
}
```

| Level | 用途 | 壊れたとき |
|---|---|---|
| Mention | コメントリンク | ignore/info |
| Soft | 設計メモ/RAG | warning |
| Checked | 型付き注釈 | error/warning |
| Runtime | goto, asset, shader | compile error |
| Contract | requires, ensures | verify error |

## rename 方針

- Runtime / Contract / Checked は rename に追従。
- Soft は ask。
- Mention は keep literal がデフォルト。
- Canonical ID へ直接 rename する。removed aliases は registry に保持しない。

## ID 自動生成

ID は省略できる。

```arcw
flow opening(state: GameState) {
    say alice "おはよう。"
}
```

LSP 表示:

```text
flow opening(...)   // @flow.opening
say alice ...       // @say.opening.001
```

Code Action:

- Insert inferred ID
- Rename ID
- Store in registry
- Copy EntityId
- Show history

## 生成規則の設定

```toml
[id]
case = "snake"
separator = "."
collision = "append_hash"
renumber_on_format = false

[id.rules.flow]
pattern = "flow.{name}"

[id.rules.say]
pattern = "say.{flow}.{slot:03}"
slot = "stable_registry_slot"
scope = "flow"
```

`seq` は registry で保持し、挿入時に既存 ID をずらさない。

## 相対 ID と名前付き scope

`@.suffix` / `@..suffix` / `@...suffix` 形式の相対 ID は、ID を期待する
文脈だけで使える。通常の entity reference ではないので、`goto @.next`
のような相対参照を flow / asset 参照としては採用しない。

ID を期待する文脈では、一貫性のため family-relative spelling も使える。
ただし手書きでは短い `@.suffix` を推奨する。

```arcw
flow @.opening {
    goto @flow:.next
}

flow @flow:.next {
}

character @.alice Alice as alice {
}

alice(id=@.greeting):
    おはよう。[p]

alice(id=@say:.greeting):
    おはよう。[p]  // allowed, but formatter may prefer id=@.greeting

choice @choice:.first {
    @choice:.listen "聞く" -> @flow:.listen
}
```

宣言そのものが family を決める場合、`@.suffix` はその宣言 family に
正規化される。`flow @.opening` と `flow @flow:.opening` は
`flow.opening` を宣言し、`character @.alice` と
`character @character:.alice` は `character.alice` を宣言する。空の
`@.` / `@family:.` marker も宣言位置では使える。その場合は後続の宣言名を
suffix として使い、`flow @. opening { ... }`、`flow @flow:. opening { ... }`、
`character @. alice Alice { ... }` はそれぞれ `flow.opening` /
`flow.opening` / `character.alice` を宣言する。ID を書かずに
`flow opening { ... }` と書いた場合も同じ暗黙 ID `flow.opening` を持つ。

一般の entity reference 文脈で相対参照したい場合は family を明示する。
同じ flow / fragment / asset family の中を相対参照する場合や、default
public-id prefix を持つ family を参照する場合は、absolute ID を手で連結する
より `@flow:.next` や `@asset:.room` のような family-relative form を
推奨する。family が明示されるので、ID を宣言する `@.suffix` と値として
参照する `@flow:.suffix` / `@asset:.suffix` が混ざらない。

```arcw
goto @flow:.next
include @frag:.intro
window = @textbox:.side
asset.load(@asset:.room)
```

つまり `@.suffix` は `IdRef::Relative`、参照文脈の `@flow:.suffix` は
`EntityRefSyntax::FamilyRelative`、ID 文脈の `@say:.suffix` /
`@choice:.suffix` は family 付きの `IdRef` として別の AST ノードになる。

Arcweft accepts two relative-ID spellings:

```text
@.suffix    current ID scope
@..suffix   parent ID scope, analogous to `super.`
@...suffix  grandparent ID scope
@super.suffix
            parent ID scope, explicit readable spelling
@super.super.suffix
            grandparent ID scope, explicit readable spelling
```

Bare `.suffix` is not part of the core grammar. Bare `..suffix` is also not
accepted; `..` already has range/rest-pattern meanings.

Deep dot runs such as `@...suffix` are accepted, but formatter and LSP tooling
should prefer the explicit spelling `@super.super.suffix` for authored code.

Lint policy note: `@...suffix` remains valid syntax for generated code and
compact authoring, but lint/formatter tooling should recommend
`@super.super.suffix` when the parent depth is greater than one.

```arcw
alice(id=@.greeting):
    おはよう。[p]

choice @.first {
    @.listen "聞いてみる" -> @flow.alice_intro
}
```

名前付き scope は、lexical scope であると同時に ID namespace として使う。

```arcw
scope rain {
    地の文(id=@.sound):
        扉の向こうから、雨の音がした。[p]

    alice(id=@.comment):
        雨、強くなってきたね。[p]
}
```

正規化規則:

```text
line id:
  id=@.suffix
    -> @say.{flow}.{speaker}.{scope_path}.{suffix}
    -> @say.{flow}.{speaker}.{suffix}                 # scope_path が空の場合

omitted line id:
  -> @say.{flow}.{speaker}.{scope_path}.{stable_slot}
  -> @say.{flow}.{speaker}.{stable_slot}               # scope_path が空の場合

omitted text key:
  -> @text.{flow}.{speaker}.{scope_path}.{line_suffix_or_slot}
  -> @text.{flow}.{speaker}.{line_suffix_or_slot}      # scope_path が空の場合

voice key when voice=auto:
  -> @voice.{speaker}.{module_path}.{flow}.{scope_path}.{line_suffix_or_slot}
  -> @voice.{speaker}.{module_path}.{flow}.{line_suffix_or_slot}

choice id:
  choice @.suffix
    -> @choice.{flow}.{scope_path}.{suffix}
    -> @choice.{flow}.{suffix}                         # scope_path が空の場合

choice option id:
  @.suffix
    -> {current_choice_id}.{suffix}
  @..suffix
    -> {parent_choice_or_scope_id}.{suffix}
  @...suffix
    -> {grandparent_choice_or_scope_id}.{suffix}
  @super.super.suffix
    -> {grandparent_choice_or_scope_id}.{suffix}
```

The same rule applies to both narration and character dialogue. The current
speaker segment is inserted before the named-scope path for dialogue IDs:

```arcw
地の文(id=@.rain):
    扉の向こうから、雨の音がした。[p]

alice(id=@.greeting, voice=auto):
    おはよう。[p]
```

```text
地の文(id=@.rain)
  -> @say.opening.narrator.rain
  -> @text.opening.narrator.rain

alice(id=@.greeting, voice=auto)
  -> @say.opening.alice.greeting
  -> @text.opening.alice.greeting
  -> @voice.alice.game.routes.opening.opening.greeting
```

Voice IDs are logical content IDs and do not include locale. Locale is a
resource variant selected by project configuration, runtime locale, or a
fallback policy.

```text
logical voice id:
  @voice.{speaker}.{module_path}.{flow}.{scope_path}.{suffix}

resource variants:
  assets/voice/{locale}/{speaker}/{module_path}/{flow}/{scope_path}/{suffix}.ogg
  assets/voice/{locale}/{speaker}/{module_path}/{flow}/{suffix}.ogg
```

For example, `mod game.routes.opening`, `flow @flow.opening`, and
`alice(id=@.greeting, voice=auto)` derive:

```text
@voice.alice.game.routes.opening.opening.greeting
assets/voice/ja-JP/alice/game/routes/opening/opening/greeting.ogg
assets/voice/en-US/alice/game/routes/opening/opening/greeting.ogg
```

名前付き scope がない場合、`scope_path` セグメントは空文字として残さず、
ID から省略する。

## Module path, flow ID, and scope hierarchy

Relative ID lowering currently uses the enclosing flow ID plus named `scope`
segments. For example, `flow @flow.opening` and `scope rain` produce
`say.opening.alice.rain.greeting` for `alice(id=@.greeting)`.

`mod game.routes.opening` is a source/module hierarchy, not automatically part
of public entity IDs today. This keeps public IDs stable when files move.
However, projects may choose a policy that requires module paths and entity IDs
to line up, such as `mod game.routes.opening` containing `@flow.opening`.

Planned lint policy: add an ID policy lint pass that can compare module path,
flow or fragment ID, named scopes, and generated relative IDs. It should report
IDs that do not follow a configured hierarchy, while keeping the core parser/HIR
Sans I/O and policy-neutral.

```arcw
alice(id=@.greeting):
    おはよう。[p]

choice @.first {
    @.listen "聞いてみる" -> @flow.alice_intro
}
```

```text
alice(id=@.greeting)
  -> @say.opening.alice.greeting
  -> @text.opening.alice.greeting

choice @.first
  -> @choice.opening.first

@.listen
  -> @choice.opening.first.listen
```

例:

```text
地の文(id=@.sound)
  -> @say.opening.narrator.rain.sound
  -> @text.opening.narrator.rain.sound

alice(id=@.comment)
  -> @say.opening.alice.rain.comment
  -> @text.opening.alice.rain.comment
  -> @voice.alice.game.routes.opening.opening.rain.comment

choice @.first
  -> @choice.opening.rain.first

@.listen
  -> @choice.opening.rain.first.listen
  -> @text.choice.opening.rain.first.listen
```

When a line ID is omitted, the stable ordinal is generated under the same
flow/speaker/scope prefix. This keeps later insertions stable while making the
scope visible in localization and voice manifests.

```arcw
scope rain {
    地の文:
        扉の向こうから、雨の音がした。[p]
}
```

```text
@say.opening.narrator.rain.001
@text.opening.narrator.rain.001
```

`scope` は入れ子にでき、`scope_path` は外側から順に連結する。

```arcw
scope rain {
    scope window {
        地の文(id=@.rattle):
            窓が小さく鳴った。[p]
    }
}
```

```text
@say.opening.narrator.rain.window.rattle
@text.opening.narrator.rain.window.rattle
```

`scope` は expression block としても使える。この場合も名前は trace / LSP
表示名と、その中で生成または相対指定される line / choice / option /
text key の namespace に使う。`scope` 式そのものの値は通常の `{ ... }`
と同じく最後の式で決まる。

```arcw
let can_enter = scope alice_route_check {
    let affection_ok = state.affection[@character.alice] >= 3
    let has_key = state.inventory.contains(@item.alice_key)
    affection_ok && has_key
}
```

ID に反映されるのは、その scope 内の ID-bearing construct だけである。

```arcw
scope dream {
    let can_enter = {
        let affection_ok = state.affection[@character.alice] >= 3
        affection_ok
    }

    choice @.first {
        @.listen "聞いてみる" if can_enter -> @flow.alice_intro
        @.silent "黙っている" -> @flow.quiet_intro
    }
}
```

```text
choice @.first
  -> @choice.opening.dream.first

@.listen
  -> @choice.opening.dream.first.listen
  -> @text.choice.opening.dream.first.listen
```

`@.suffix` / `@..suffix` / `@super.suffix` は module path には使わない。module / import の相対指定は
`self.`、`super.`、`crate.` を使う。`parent.` は `super.` の予約
alias で、formatter は `super.` に正規化する。

```arcw
alice(id=@.greeting):       // ID context
use self.characters.alice // module path context
```

Module and import roots are deliberately separate from relative IDs:

```arcw
mod crate.game.routes.opening
mod self.routes.opening
mod super.shared

use crate.game.prelude.*
use self.characters.{alice, bob}
use super.common.{route_gate, shared_flags}
```

If a source file uses `parent.`, canonical tooling should rewrite it to
`super.`.

## Resource directory mapping

Resource scans derive public IDs from stable directory layout while keeping
`EntityId`, `PublicId`, source path, and semantic hash separate in manifests.
The examples below show canonical public IDs in manifest/tooling output. In
authored source, prefer family-relative asset references such as
`@asset:.bg.room` for the same public id.

```text
assets/bg/room.png
  -> @asset.bg.room

assets/voice/ja-JP/alice/game/routes/opening/opening/greeting.ogg
  -> @voice.alice.game.routes.opening.opening.greeting

assets/se/ui/page.ogg
  -> @se.ui.page

assets/bgm/alice_theme/main.ogg
  -> @bgm.alice_theme.main

assets/character/alice/body.png
  -> @asset.char.alice.body

assets/live2d/alice/rig.model3.json
  -> @asset.live2d.alice.rig
```

Tooling should provide resource-safe operations without changing the Sans I/O
parser/core boundary:

```bash
arcw resource scan
arcw resource check
arcw resource fix
arcw rename @voice.alice.game.routes.opening.opening.greeting @voice.alice.game.routes.opening.opening.soft_greeting
arcw resource move assets/voice/ja-JP/alice/game/routes/opening/opening/greeting.ogg assets/voice/ja-JP/alice/game/routes/opening/opening/soft_greeting.ogg --update-refs
arcw resource move assets/bg/room.png assets/bg/opening/room.png --keep-id
```

