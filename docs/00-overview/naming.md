# 命名・拡張子・公開識別子

この章では、エンジン名、CLI 名、拡張子、crate prefix、MCP/LSP namespace、HTML bridge 属性を固定する。

## 公開名

エンジン名は **Arcweft Engine** とする。

```text
Arcweft Engine
A layered, verified, agent-native narrative engine.
```

## 短縮名

```text
arcweft  正式 prefix
arcw     CLI command / 開発者向け短縮名
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

source extension は **`.arcw`** とする。

```text
.arcw     Arcweft DSL source
.awfb     Arcweft bundle
.awfs     Arcweft save snapshot
.arcwx    Arcweft trace / replay trace
.awfagent Arcweft agent script
```

Project layout, authored roots, and local state policy are defined by
[Authored resource and local state storage](../05-build-and-security/authored-resource-storage.md).

## Rust workspace / crate 名

Workspace crates use the `arcweft-` prefix, and the facade package is `arcweft`.
The member list is owned by the workspace `Cargo.toml`; cross-crate boundaries
are described in [crate-map.md](crate-map.md).

## URI scheme / protocol namespace

MCP / Agent resources use the `arcweft://` scheme, MCP tools use the
`arcweft.` prefix, and LSP custom methods use the `arcweft/` prefix. The
current resource and tool contracts are owned by
[Agent Debug Bus / MCP / CLI](../04-tooling/agent-debug-mcp-cli.md) and
[LSP](../04-tooling/lsp.md).

## HTML bridge 属性

HTML/CSS View backend は `data-arcweft-*` 属性を使う。

```html
<button
  data-arcweft-entity="choice.opening.listen"
  data-arcweft-action="select">
  聞いてみる
</button>
```


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

## IDs in source

Declaration identity, relative IDs, family-qualified references, normalization,
and asset ID derivation are defined by [ID and references](../01-language/ids-and-references.md).

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
