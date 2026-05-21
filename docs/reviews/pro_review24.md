2026-05-16の前提文書に従い、Sanzentyo/arcweft向けの記法改善として見ます。
結論から言うと、**はい、`｜変な夢《へんなゆめ》` をルビの主入力形式にするのはかなりつらい**です。残す価値はありますが、作者が普段打つ形式としては不利です。

Arcweftは「コンパクトなVN記法に近い書き心地」と、型検査・安定ID・ローカライズ・音声同期などを両立する設計なので、入力記号の重さは仕様上の重要問題です。現行仕様でもルビは自然日本語形式、`[ruby rt="..."]...[/ruby]`、`#[ruby(...)]` の3形式が同じ `Content::Ruby { base, ruby }` に正規化される設計になっています。 
実装側も `DialogueToken::Ruby { base, ruby }` を持っているので、**AST/HIRを変えずに入力口だけ増やす**のがよいです。

## 改善方針

`《》` は削除しない。
ただし、**標準の作者向け入力形式はASCIIでも打てる形にする**のがよいです。

推奨する追加ルビ記法はこの3つです。

```awft
# 既存。残す。日本語組版としては読みやすいが、入力は重い。
｜変な夢《へんなゆめ》

# 追加: ASCII明示形。まずこれを推奨。
|[変な夢](へんなゆめ)

# 追加: ASCII短縮形。日本語本文向け。
|変な夢{へんなゆめ}

# 追加: タグ短縮形。長文・空白・複雑なbase向け。
[rb rt=へんなゆめ]変な夢[/rb]
```

サンプルはこう変えたいです。

```awft
alice.say(id=@say.opening.alice.001, voice=auto, look=.smile)[
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
]
```

または、短く書くならこうです。

```awft
alice:
    今日は少しだけ、|変な夢{へんなゆめ}を見たんだ。[p]
```

`|[base](ruby)` を第一推奨にしたい理由は、空白を含む語にも使えて、誤爆が少なく、`｜...《...》` と見た目の対応も分かりやすいからです。`|base{ruby}` は短いですが、ローカライズ用の `{name}` と近いので、`base` に空白・`[`・`]`・`{`・`}`・`#` を含む場合は受け付けず、`|[base](ruby)` を提案する診断にしたほうが安全です。

## ほかに欠けている記号表現

現行の対話テキストは、`[p]`、`[l]`、`[r]`、`[ruby ...]`、`#[...]`、`[call ...]`、`[mark .name]` を特別扱いします。これらは対話テキストモード限定なので、通常コードの `[]` やインデックスとは分離されています。 
この前提なら、対話内だけに入力しやすい別名を増やせます。

| 現行                                 | 問題                | 追加案                             | 意味             |
| ---------------------------------- | ----------------- | ------------------------------- | -------------- |
| `#[player_name]`                   | `#` + `[` が文章中で重い | `$(player_name)`                | 純粋な本文挿入        |
| `#[fmt(score, style="number")]`    | 長い                | `$(fmt(score, style="number"))` | 書式付き本文挿入       |
| `[call flash(...)]`                | 長い                | `[! flash(...)]`                | 副作用ありの対話安全呼び出し |
| `[mark .keyword]`                  | 長い                | `[.keyword]`                    | 行内マーカー         |
| `[w time=500ms]`                   | 属性名が重い            | `[w 500ms]`                     | 時間待ち           |
| `[p]`                              | 短いが意味が初見で不明       | `[page]`                        | ページ待ち          |
| `[l]`                              | 短いが意味が初見で不明       | `[wait]`                        | 行待ち            |
| `[r]` / `[br]`                     | 既に悪くない            | `[nl]` も許可                      | 改行             |
| `[em]夢[/em]`                       | 1語だけ強調するには閉じタグが重い | `[em:夢]`                        | 強調スパン          |
| `[strong]夢[/strong]`               | 同上                | `[strong:夢]`                    | 強調スパン          |
| `[color value="#a8b5ff"]夢[/color]` | 長い                | `[color #a8b5ff:夢]`             | 色付きスパン         |
| `[raw]...[/raw]`                   | 1行リテラルには重い        | `[raw: ...]`                    | 1スパン生テキスト      |

特に効果が高いのは、`[.keyword]`、`[! flash(...)]`、`$(expr)` です。ルビ以外ではこの3つが、文章中のノイズを大きく減らします。

## 逆に避けたい案

`漢字{かな}` のように、先頭マーカーなしで `{}` をルビにする案は避けたいです。Arcweftでは `{player_name}` がローカライズ用プレースホルダーとして使われるため、ルビとプレースホルダーの見分けが悪くなります。

Markdown風の `*強調*` や `**強調**` も、本文としての `*` と衝突しやすいので、標準にはしないほうがよいです。使うなら project config で有効化するオプション扱いが安全です。

`@marker` のような案も避けたいです。Arcweftでは `@` がエンティティ参照の中心記号なので、対話本文だけ別用途に寄せると認知負荷が増えます。

## 仕様としての落とし込み

追加仕様はこうまとめるのがきれいです。

```text
RubyInput :=
    '｜' Base '《' RubyText '》'
  | '|' '[' Base ']' '(' RubyText ')'
  | '|' CompactBase '{' RubyText '}'
  | '[ruby' RubyAttrs ']' Base '[/ruby]'
  | '[rb' RubyAttrs ']' Base '[/rb]'
  | '#[ruby(' String ',' String ')]'

All forms normalize to DialogueToken::Ruby { base, ruby }.
```

短縮形の制約はこうです。

```text
|base{ruby}

base:
  - non-empty
  - no whitespace
  - no unescaped [, ], {, }, #, |
  - if invalid, parser should keep text and diagnostic should suggest |[base](ruby)
```

エスケープ表も更新します。

```text
\|   literal |
\$   literal $
\(   literal (
\)   literal )
\{   literal {
\}   literal }
\［  literal fullwidth bracket, if accepted
```

現行のトークナイザは壊れたタグを本文として残す寛容な設計なので、ASCIIルビや `$(...)` を追加しても、失敗時に本文を壊しにくいです。

## Formatter / LSP の設定案

入力しやすさは、パーサだけでなく formatter と LSP で支えるべきです。

```toml
[dialogue.authoring]
ruby_input = ["natural", "ascii_explicit", "ascii_compact", "tag"]
ruby_emit = "preserve"   # preserve | natural | ascii_explicit | ascii_compact | tag
fullwidth_tag_brackets = "warn_and_accept"

[dialogue.snippets]
rb = "|[${base}](${ruby})"
mark = "[.${name}]"
call = "[! ${fn}(${args})]"
expr = "$(${expr})"
```

既存ソースを尊重するなら `ruby_emit = "preserve"` をデフォルトにします。日本語組版として美しく出したいプロジェクトだけ `natural` にすればよいです。

## 優先順位

P0で入れるべきものは、ルビのASCII入力です。

```awft
|[変な夢](へんなゆめ)
|変な夢{へんなゆめ}
[rb rt=へんなゆめ]変な夢[/rb]
```

P1で入れるべきものは、文章中のノイズ削減です。

```awft
$(player_name)
[.keyword]
[! flash(color=#ffffff, time=90ms)]
[w 500ms]
```

P2で入れるべきものは、スタイル系の単発短縮です。

```awft
[em:夢]
[strong:夢]
[color #a8b5ff:夢]
[raw: [p]をタグとして扱わない]
```

## 最終提案

`｜...《...》` は「自然日本語ルビ」として残す。
ただし、**ドキュメント上の推奨入力は `|[base](ruby)` に変更**するのがよいです。

```awft
alice:
    今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
```

短く書きたい作者にはこちらも許す。

```awft
alice:
    今日は少しだけ、|変な夢{へんなゆめ}を見たんだ。[p]
```

この方針なら、既存サンプルや自然日本語の読みやすさを壊さず、作者の入力負荷だけを下げられます。
