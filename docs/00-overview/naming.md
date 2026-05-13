# 命名・拡張子・公開識別子

この章では、エンジン名、CLI 名、拡張子、crate prefix、MCP/LSP namespace、HTML bridge 属性を固定する。

## 公開名

エンジン名は **Arcweft Engine** とする。

```text
Arcweft Engine
A layered, verified, agent-native narrative engine.
```

意味づけ:

- **Arc**: 物語の弧、graph edge、scene transition、render arc。
- **Weft**: 織物の横糸。layer、hook、signal、UI、reference、shader、activity を織り込むこと。
- 既存の有名 visual novel engine、ゲーム会社、一般的な graphics library と強く混ざりにくい方向を狙う。
- crate、CLI、documentation、bundle metadata に展開しやすい。

## 短縮名

```text
arcweft  正式 prefix
arcw     CLI command / 開発者向け短縮名
awft     source extension root
```

CLI は `arcw` を使う。`arcweft` ではなく `arcw` にすることで、コマンド例を短く保つ。

```bash
arcw new my-game
arcw check
arcw run
arcw build web
arcw agent observe
arcw verify
arcw shader check
```

## ファイル拡張子

source extension は **`.awft`** とする。

```text
.awft     Arcweft DSL source
.awfb     Arcweft bundle
.awfs     Arcweft save snapshot
.awftx    Arcweft trace / replay trace
.awfagent Arcweft agent script
```

推奨 project layout:

```text
project.awft.toml
arcweft.lock

src/
  main.awft
  routes/opening.awft
  ui/settings.awft
  shaders/post.awft

.arcweft/
  entities.toml
  links.toml
  graph-cache/
```

versioned file:

```text
*.awft
project.awft.toml
.arcweft/entities.toml
.arcweft/links.toml
```

generated / local-only file:

```text
.arcweft/cache/
.arcweft/history/
.arcweft/rag/
*.awfb
*.awftx
```

## Rust workspace / crate 名

crate は `arcweft-` prefix を使う。

```text
arcweft-core
arcweft-lang-syntax
arcweft-render
arcweft-ui-core
arcweft-shader-core
arcweft-audio-core
arcweft-agent-mcp
arcweft-cli
```

root package は registry で空いていれば `arcweft`、空いていない場合は `arcweft-engine` を使う。

## URI scheme / protocol namespace

MCP / Agent resource は `arcweft://` URI scheme を使う。

```text
arcweft://session/{sid}/observation/latest.json
arcweft://session/{sid}/frame/{tick}/overlay.png
arcweft://session/{sid}/logs.ndjson
```

MCP tool は `arcweft.` namespace を使う。

```text
arcweft.observe
arcweft.click
arcweft.invoke
arcweft.choose
arcweft.wait_until
arcweft.get_state
arcweft.shader_preview
arcweft.audio_state
```

LSP custom request は `arcweft/` namespace を使う。

```text
arcweft/getGraphSlice
arcweft/previewGraphPatch
arcweft/applyGraphPatch
arcweft/getRagContext
arcweft/renderRouteMap
arcweft/parseInput
arcweft/shaderPreview
arcweft/audioCuePreview
```

## HTML bridge 属性

HTML/CSS UI backend は `data-arcweft-*` 属性を使う。

```html
<button
  data-arcweft-entity="choice.opening.listen"
  data-arcweft-action="select">
  聞いてみる
</button>
```

旧来の `data-vn-*` 形式は Arcweft docs では使わない。

## DSL 内の PublicId

PublicId はゲーム内 domain を表し、engine 名は含めない。

```text
flow.opening
choice.opening.listen
shader.post.crt
activity.truck_game
signal.loading_progress
```

engine 名は file、crate、CLI、protocol、packaging に使い、すべての in-game entity に prefix として付けない。

## Relative IDs in source

`.suffix` is allowed only in ID-bearing source contexts where the expected
entity family is known: dialogue line IDs, choice IDs, choice option IDs, and
text-key overrides. It is not a general entity reference.

```awft
alice(id=.greeting):
    おはよう。[p]

scope dream {
    choice .first {
        .listen "聞いてみる" -> #flow.alice_intro
    }
}
```

Relative IDs normalize through the current flow, speaker, choice, and named
scope path. Dialogue lines place the speaker before the scope path; choices use
the flow and scope path directly.
If the named scope path is empty, the scope segment is omitted.

```text
id=.greeting
  -> #say.opening.alice.greeting

scope rain { alice(id=.comment): ... }
  -> #say.opening.alice.rain.comment
  -> #text.opening.alice.rain.comment
  -> #voice.ja-JP.alice.opening.rain.comment

choice .first
  -> #choice.opening.dream.first

.listen
  -> #choice.opening.dream.first.listen
```

For ordinary entity references, keep the `#domain.path` form:

```awft
goto #flow.opening.next
```

Do not write `goto .next`. If general relative entity references are added
later, they should use an explicit marker such as `#.` rather than overloading
bare `.suffix`.

## 予約名

以下の prefix は engine internal として予約する。

```text
arcweft
__arcweft
builtin
core
std
```

game project は、明示的に Arcweft extension crate を実装する場合を除き、`arcweft::*` module を作らない。

## rename policy

将来、package registry、商標、OSS project などで強い衝突が見つかった場合は、この章を source of truth として、crate 名、CLI、URI scheme、MCP namespace、docs の用語を機械的に更新する。

`.awft` が単独で conflict-free なら extension は維持できるが、product name と extension の両方が混ざりやすくなった場合は同時に再検討する。
