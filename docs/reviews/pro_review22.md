はい、**結局「何を起動するか」の指定は必要**です。
ただし、それは **`.awft` に毎回 `entry` を書くべき**という意味ではありません。

Arcweft では、最新の理念・構造を確認してから判断する前提があります。 現行構造でも core は Sans I/O で、実際の起動・adapter・renderer・audio・CLI などは Host / Adapter Layer 側にあります。 つまり「どの flow をどう起動するか」は、`flow` 本体とは別の **launch / host contract** として扱うのが自然です。

## 結論

デバッグ時に毎回 `entry` を書く必要はありません。

必要なのは、次の3層を分けることです。

```text
flow
  実行される処理本体

EntrySpec
  release / normal launch 用の名前付き起動契約

LaunchOverride / LaunchProfile
  debug / test / CLI で一時的に使う起動指定
```

つまり、`entry` は「すべてのデバッグ対象 flow ごとに書くもの」ではなく、**通常起動や配布物に必要な名前付き入口**です。デバッグでは `--flow` や launch profile から **一時的な synthetic entry** を作ればよいです。

---

## なぜこの区別が必要か

現状の runtime にはすでに `RuntimePlan { entry_flow: Option<FlowRuntimeId>, ... }` があります。 しかし現在の lowering は、最初の flow を entry として選んでいます。

これは通常起動では危険です。

```awft
flow @flow.debug_start debug_start {
    goto @flow.chapter2
}

flow @flow.opening opening {
    alice: おはよう
}

flow @flow.chapter2 chapter2 {
    alice: ここからデバッグしたい
}
```

このとき、ファイルの先頭に `@flow.debug_start` を置いたせいで release 起動が変わる、というのはよくありません。
だから release / product / server / CLI では明示的な entry が必要です。

ただし、デバッグで毎回こう書くのも嫌です。

```awft
entry game @entry.debug_chapter2 {
    start @flow.chapter2
}
```

このような debug entry が source に大量に増えると、まさに負債になります。

---

## 推奨設計

### 1. 通常起動は `EntrySpec`

release や bundle には明示的な entry を持たせます。

```toml
[entries.main]
kind = "game"
start = "@flow.opening"

[entries.cli]
kind = "cli"
run = "@flow.cli_main"
```

`.awft` に書くならこうです。

```awft
entry game @entry.main {
    start @flow.opening
}
```

ただし、僕は今の段階では **`.awft` 文法より manifest / arcweft.toml 側を先に推奨**します。

---

### 2. デバッグは `LaunchOverride`

デバッグでは、entry を source に書かずに CLI で指定します。

```bash
arcw run game.awft --flow @flow.chapter2
```

または:

```bash
arcw run game.awft --entry @entry.main
```

`--flow` は内部的にはこういう一時 entry を作るだけです。

```rust
EntrySpec {
    id: EntryId("__debug"),
    kind: EntryKind::Game,
    target: EntryTarget::StartFlow(FlowRuntimeId("flow.chapter2")),
}
```

これは source に保存しません。
つまり、**指定は必要だが、entry declaration は不要**です。

---

### 3. よく使うデバッグは `LaunchProfile`

毎回 `--flow` を打つのも面倒なので、debug profile を project config に置きます。

```toml
[launch.main]
kind = "game"
entry = "@entry.main"

[launch.chapter2]
kind = "game"
flow = "@flow.chapter2"

[launch.good_end_route]
kind = "game"
flow = "@flow.chapter2"
values = [
  "affection=80",
  "route=@flow.good_end"
]

[launch.cli_smoke]
kind = "cli"
flow = "@flow.cli_main"
args = ["--help"]
```

実行:

```bash
arcw run game.awft --profile chapter2
```

または:

```bash
arcw debug game.awft --profile good_end_route
```

この方式なら、`.awft` は汚れません。

---

## `entry` と `--flow` の違い

`--flow` は開発用の一時指定です。

```bash
arcw run game.awft --flow @flow.chapter2
```

`entry` は名前付きの起動契約です。

```bash
arcw build native --entry @entry.main
arcw serve server.awft --entry @entry.http
arcw cli tool.awft --entry @entry.tool
```

区別するとこうです。

| 用途              | 指定方法                     | 保存先                                 |
| --------------- | ------------------------ | ----------------------------------- |
| release game 起動 | `entry`                  | manifest / `.awft` / `arcweft.toml` |
| CLI tool 起動     | `entry`                  | manifest / `arcweft.toml`           |
| server route 起動 | `entry`                  | manifest / `arcweft.toml`           |
| 一時デバッグ          | `--flow`                 | 保存しない                               |
| よく使うデバッグ        | `launch profile`         | `arcweft.toml`                      |
| test fixture    | `test { start @flow.x }` | `.awft` test declaration            |

---

## 「entry を明示しない場合」はどうするか

ここはルールを決めるべきです。

おすすめはこれです。

```text
1 flow だけなら暗黙にそれを起動してよい
複数 flow があるなら entry / --flow / --profile を要求する
release build では必ず entry を要求する
debug run では --flow を許す
```

つまり:

```bash
arcw run one_flow.awft
```

これはOK。

```awft
flow @flow.only only {
    return "done"
}
```

でも複数 flow なら:

```bash
arcw run game.awft
```

これは失敗させます。

```text
error: multiple runnable flows found
help: specify --flow @flow.opening or --entry @entry.main
available flows:
  @flow.opening
  @flow.chapter2
  @flow.debug_start
```

これなら暗黙の危険を避けつつ、開発時の手間も少ないです。

---

## `.awft` に `entry` を入れるべきか

改めて整理すると、僕の推奨は少し変わります。

### 必須にすべきもの

**EntrySpec という概念**は必要です。

```rust
pub struct EntrySpec {
    pub id: EntryId,
    pub kind: EntryKind,
    pub target: EntryTarget,
}
```

### すぐ入れなくてもいいもの

`.awft` top-level の `entry` 文法は、急がなくてもよいです。

むしろ最初は:

```text
arcweft.toml / .awfb manifest に entries を置く
```

で十分です。

`.awft` に書く `entry game { ... }` は、source-local に管理したい需要がはっきりしてから入れてもよいです。

---

## 最終判断

あなたの指摘どおり、**デバッグでも結局「どこから始めるか」の指定は必要**です。
ただし、それを `.awft` に `entry` として毎回追加する必要はありません。

正しい分離はこうです。

```text
release / product:
  EntrySpec を使う

debug one-shot:
  --flow で一時指定する

debug repeated:
  launch profile を使う

test:
  test/scenario 側で start @flow.x を書く

server/cli:
  EntrySpec が必要
```

なので、`entry` は **毎回書くものではなく、保存された起動契約**です。
デバッグ時は `--flow` や `launch profile` から synthetic entry を作るのが一番きれいです。

私なら、**今すぐ `.awft` に `entry` 文法を導入するほどではない**と判断します。
ただし、**`EntrySpec` という概念は必要**です。つまり、文法ではなく **manifest / `arcweft.toml` / `.awfb` 側の起動設定**として先に入れるのがよいです。

前提として、Arcweft 関連では最新の理念・構造を確認してから判断する必要があります。 現行アーキテクチャでは、core は Sans I/O で、renderer / audio / CLI / Activity host などは Host / Adapter Layer 側に置かれています。 そのため、「何を起動するか」は source language の意味論というより、host / build / launch の設定に近いです。

## 結論

`entry` は、**概念としては必要**です。

```rust
EntrySpec {
    id,
    kind: Game | Cli | Server | Activity | Test,
    target,
    capabilities,
}
```

でも、それを `.awft` の top-level 文法として:

```awft
entry game @entry.main {
    start @flow.opening
}
```

のように今すぐ入れるのは、やや重いです。

現行 grammar の top-level item には `FlowDecl`, `FunctionDecl`, `SourceDecl`, `StateDecl`, `ReducerDecl`, `ViewDecl`, `ParserDecl`, `HookDecl`, `MemoDecl`, `DialogueDefaultsDecl`, `TypeDecl` などがあり、`entry` はまだありません。 これを足すと、CST classifier、AST、HIR、sema、formatter、LSP、manifest lowering、tooling、test fixture まで全部に波及します。今の段階でそのコストを払うほど、`.awft` 内に書く必然性はまだ強くないと思います。

---

## なぜ `entry` の「概念」は必要なのか

現行 runtime にはすでに `RuntimePlan { entry_flow: Option<FlowRuntimeId>, ... }` があります。 そして現在の lowering は、最初の flow を entry として選んでいます。

これは危険です。

```awft
flow @flow.debug_start debug_start {
    goto @flow.chapter2
}

flow @flow.opening opening {
    alice: おはよう
}

flow @flow.chapter2 chapter2 {
    alice: ここから本編
}
```

この場合、ファイルの並び順で起動対象が変わってしまいます。formatter、module 分割、generated code、debug flow の追加で release 起動が壊れる可能性があります。

だから、**「何を起動するか」は明示する必要がある**。
ただし、それは `.awft` 文法である必要はありません。

---

## なぜ `.awft` 文法としてはまだ弱いのか

### 1. 起動設定は source semantics ではなく host semantics に近い

`flow` は script の中身です。
`entry` は「native player で起動する」「CLI として起動する」「server adapter で route を開く」などの host 側の話です。

Arcweft の設計では、path を開く、環境変数を見る、wall-clock を読む、backend resource を確保する処理は CLI / build tool / native/web player adapter 側に置く方針です。 その思想から見ると、launch config もまずは adapter/build 側に置く方が自然です。

### 2. デバッグ問題を文法だけでは解決しない

仮に `.awft` に `entry` を入れても、デバッグ時には結局こういう指定が欲しくなります。

```bash
arcw run game.awft --flow @flow.chapter2
arcw run game.awft --profile chapter2
arcw run game.awft --value affection=80 --flow @flow.good_end
```

debug entry を `.awft` に大量に書くと、source が汚れます。

```awft
entry game @entry.debug_chapter2 {
    start @flow.chapter2
}

entry game @entry.debug_good_end {
    start @flow.good_end
}
```

これは負債になりやすいです。

### 3. `.awft` 文法に入れると実装面積が大きい

`entry` を top-level item に入れるなら、最低でも以下が必要です。

```text
CstTopLevelItemKind::Entry
Item::Entry
EntryItem AST
HirTopLevelDecl::Entry または HirEntry
manifest lowering
reference validation
capability validation
formatter
LSP/inlay/actions
arcw check/plan/run support
test fixtures
docs/grammar update
```

このコストに対して、今得られる価値は「起動 flow を明示できる」くらいです。
それなら `arcweft.toml` や `.awfb` manifest で十分です。

---

## まず入れるべき形

### `arcweft.toml`

```toml
[entries.main]
kind = "game"
start = "@flow.opening"

[entries.debug_chapter2]
kind = "game"
start = "@flow.chapter2"

[entries.cli]
kind = "cli"
run = "@flow.cli_main"

[entries.http]
kind = "server"

[[entries.http.routes]]
method = "GET"
path = "/health"
flow = "@flow.health"
```

### CLI

```bash
arcw run game.awft --entry main
arcw run game.awft --flow @flow.chapter2
arcw run game.awft --profile debug_chapter2
arcw build native --entry main
arcw serve server.awft --entry http
```

### Runtime / manifest 側

```rust
pub struct EntrySpec {
    pub id: EntryId,
    pub kind: EntryKind,
    pub target: EntryTarget,
    pub capabilities: Vec<CapabilityUse>,
}

pub enum EntryKind {
    Game,
    Cli,
    Server,
    Activity,
    Test,
    Bench,
}

pub enum EntryTarget {
    StartFlow(FlowRuntimeId),
    RunFlow(FlowRuntimeId),
    HttpRoutes(Vec<RouteSpec>),
}
```

これなら、`entry` の概念を導入しつつ、`.awft` 文法を増やさずに済みます。

---

## `.awft` 文法に入れるべき条件

逆に、次の条件が満たされたら `.awft` 文法として `entry` を入れる価値があります。

### 1. source-local な起動定義が作品の意味論に含まれる

たとえば、同じ `.awft` bundle の中に product modes を明示したい場合。

```awft
entry game @entry.main {
    start @flow.opening
    capabilities { render.write, audio.write, save.read, save.write }
}
```

この `capabilities` が sema / verifier / build の対象になるなら、source 内にある価値があります。

### 2. server routing が source graph の一部になる

```awft
entry server @entry.http {
    route GET "/health" -> @flow.health
    route POST "/save" -> @flow.save
}
```

route が typecheck / OpenAPI generation / auth policy / Agent inspection に関わるなら、文法にする価値があります。

### 3. LSP / Agent が entry を source span 付きで編集する必要がある

「この flow を default entry にする」「route を追加する」「entry から未到達 flow を検出する」などを LSP action にしたいなら、`.awft` 内にある方が強いです。

### 4. manifest と source の二重管理がつらくなった

`arcweft.toml` と `.awft` の対応が複雑になり、source-local declaration として持つ方が保守しやすくなったら導入してよいです。

---

## いまの推奨

今はこうするのが一番よいと思います。

```text
P0:
  RuntimePlan の entry 選択を「先頭 flow」から脱却する

P1:
  arcweft.toml / .awfb manifest に EntrySpec を入れる

P2:
  arcw run --flow / --entry / --profile を整備する

P3:
  release build では entry 必須
  dev run では single-flow fallback と --flow を許可

P4:
  server/cli/product mode が十分固まったら .awft entry 文法を再検討する
```

つまり、**`entry` 文法は今は保留**。
**`EntrySpec` は今すぐ必要**。
この分離が一番負債が少ないです。

---

## ルール案

`entry` 文法なしでも、曖昧さは避けられます。

```text
dev run:
  --flow があればそれを synthetic entry として使う
  --entry があれば manifest entry を使う
  flow が1つだけならそれを使ってよい
  flow が複数なら error

release build:
  entry 必須
  先頭 flow fallback 禁止

test:
  test/scenario の start @flow.x を使う

server/cli:
  manifest entry 必須
```

複数 flow で指定なしなら:

```text
error: multiple runnable flows found
help: specify --flow @flow.opening or --entry main
available flows:
  @flow.opening
  @flow.chapter2
  @flow.debug_start
```

これなら `.awft` に `entry` 文法を入れずに、起動の曖昧さを解消できます。

---

最終判断としては、**わざわざ `.awft` 文法を導入するほどではまだない**です。
ただし、`entry` の概念を消すべきではありません。まず manifest-level `EntrySpec` と CLI の `--flow / --entry / --profile` を入れて、`entry` が source semantics として本当に必要になった段階で `.awft` 文法に昇格させるのが一番きれいです。
