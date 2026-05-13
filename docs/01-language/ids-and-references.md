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

```awft
#flow.opening
#choice.opening.listen
#asset.bg.room
#state.GameState.affection
```

境界明示:

```awft
#<activity.truck_game>.run(...)
#<flow.alice_intro@jj:qtnqlkkm>
#<say.opening.dream_hint@sem:b3_9f2a1c>
#<ent:01J8X6K9XW4M9F2D7A1R8QZ6CN>
```

コメント:

```awft
/// [[flow.alice_intro]]
/// [[soft:flow.alice_intro]]
/// [[say.opening.dream_hint@sem:b3_9f2a1c]]
```

## 参照レベル

```awft
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
- alias と deprecated alias を registry に保持。

## ID 自動生成

ID は省略できる。

```awft
flow opening(state: GameState) {
    say alice "おはよう。"
}
```

LSP 表示:

```text
flow opening(...)   // #flow.opening
say alice ...       // #say.opening.001
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

`.suffix` 形式の相対 ID は、ID を期待する文脈だけで使える。通常の
entity reference ではないので、`goto .next` のような裸の相対参照は
採用しない。flow や asset を参照する場合は完全な `#flow...` /
`#asset...` を書く。

```awft
alice(id=.greeting):
    おはよう。[p]

choice .first {
    .listen "聞いてみる" -> #flow.alice_intro
}
```

名前付き scope は、lexical scope であると同時に ID namespace として使う。

```awft
scope rain {
    地の文(id=.sound):
        扉の向こうから、雨の音がした。[p]

    alice(id=.comment):
        雨、強くなってきたね。[p]
}
```

正規化規則:

```text
line id:
  id=.suffix
    -> #say.{flow}.{speaker}.{scope_path}.{suffix}

omitted line id:
  -> #say.{flow}.{speaker}.{scope_path}.{stable_slot}

omitted text key:
  -> #text.{flow}.{speaker}.{scope_path}.{line_suffix_or_slot}

voice key when voice=auto:
  -> #voice.{locale}.{speaker}.{flow}.{scope_path}.{line_suffix_or_slot}

choice id:
  choice .suffix
    -> #choice.{flow}.{scope_path}.{suffix}

choice option id:
  .suffix
    -> {current_choice_id}.{suffix}
```

例:

```text
地の文(id=.sound)
  -> #say.opening.narrator.rain.sound
  -> #text.opening.narrator.rain.sound

alice(id=.comment)
  -> #say.opening.alice.rain.comment
  -> #text.opening.alice.rain.comment
  -> #voice.ja-JP.alice.opening.rain.comment

choice .first
  -> #choice.opening.rain.first

.listen
  -> #choice.opening.rain.first.listen
  -> #text.choice.opening.rain.first.listen
```

`scope` は入れ子にでき、`scope_path` は外側から順に連結する。

```awft
scope rain {
    scope window {
        地の文(id=.rattle):
            窓が小さく鳴った。[p]
    }
}
```

```text
#say.opening.narrator.rain.window.rattle
#text.opening.narrator.rain.window.rattle
```

`scope` は expression block としても使える。この場合も名前は trace / LSP
表示名と、その中で生成または相対指定される line / choice / option /
text key の namespace に使う。`scope` 式そのものの値は通常の `{ ... }`
と同じく最後の式で決まる。

```awft
let can_enter = scope alice_route_check {
    let affection_ok = state.affection[#character.alice] >= 3
    let has_key = state.inventory.contains(#item.alice_key)
    affection_ok && has_key
}
```

ID に反映されるのは、その scope 内の ID-bearing construct だけである。

`.suffix` は module path には使わない。module / import の相対指定は
`self::`、`super::`、`crate::` を使う。`parent::` は `super::` の予約
alias で、formatter は `super::` に正規化する。
