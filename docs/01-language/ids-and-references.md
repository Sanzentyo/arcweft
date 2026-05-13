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
