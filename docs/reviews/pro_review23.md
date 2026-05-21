# Response: Launch Profiles, Adapter Contexts, and Dedicated CLI Commands

## 前提: `awft` ではなく `arcw` に統一する

この回答では、現在の移行方針に合わせて **`awft` 表記は `arcw` へ寄せる**。  
したがって、元メモに残っている次の表記は、方針上は移行対象として扱う。

- `*.awft` → `*.arcw`
- Markdown のコードフェンス `awft` → `arcw`
- サンプルパス `samples/.../server.awft` → `samples/.../server.arcw`

ただし、プロジェクト名・クレート名としての `arcweft-*` は別軸なので、そのまま維持してよい。

## 結論

推奨は **Option B: LaunchProfile を正規モデルにし、専用コマンドを薄い alias にする** です。

```text
LaunchProfile is the canonical model.
Dedicated commands are aliases.
AdapterContext is data selected by the profile.
```

理由は、`arcw serve` / `arcw cli` / `arcw test` / `arcw bench` がそれぞれ独自に semantic path を持つと、言語 checker、LSP、verifier、formatter、runtime が同じ入力を別々の前提で解釈することになるためです。  
Arcweft の source / project metadata が runtime の意味を十分に表現できるようにするなら、実行単位は command ではなく profile に寄せるべきです。

## 決定案

| 論点 | 決定 |
|---|---|
| `arcw serve` は first-class command か | UX としては残すが、内部的には `arcw run --profile ...` へ lower する alias にする。 |
| adapter-injected symbols の所在 | canonical definition は adapter metadata / `arcweft-adapter-context`、選択と有効化は LaunchProfile、source には限定的な entry shorthand だけを置く。 |
| generic `arcw check` で adapter context を使うか | 使わない。generic check は strict。adapter context は `--profile` または明示 adapter 選択時だけ適用する。 |
| `route_params` の扱い | ambient injected binding は廃止し、明示的な route-to-flow parameter binding に寄せる。 |
| speaker option atoms | bare atom は registry / schema / expected type がある場合だけ許可する。`.smile` のような short variant atom は引き続き許可してよい。 |

## 1. Dedicated commands は alias として残す

ユーザー体験としては、次のようなコマンドは残してよい。

```bash
arcw serve --entry http --adapter native-http --listen 127.0.0.1:8787
arcw cli --entry main -- --name Alice
arcw test opening
arcw bench opening
```

ただし、実装上はすべて正規化された `LaunchProfile` に変換する。

```bash
arcw serve --entry http --adapter native-http
# lowers to:
arcw run --profile server.http.native
```

この方針により、CLI の便利さを失わずに、semantic path は一本化できる。

### 内部的な正規化イメージ

```rust
struct LaunchProfile {
    id: ProfileId,
    kind: LaunchKind,          // server | cli | game | test | bench
    entry: EntryRef,           // src/server.arcw#server
    adapter: Option<AdapterRef>,
    adapter_context: Option<AdapterContextRef>,
    runtime: RuntimeOptions,
}
```

`arcw serve` や `arcw cli` は、この構造を直接組み立てる command ではなく、manifest に定義済みの profile を選ぶ、または temporary profile を合成する alias として扱う。

## 2. AdapterContext は profile が選ぶ data にする

`route_params` や `request` のような adapter-provided symbol は、言語組み込みではない。したがって `arcweft-lang-sema` に直接入れるべきではない。

推奨する責務分担は次の通り。

| 層 | 責務 |
|---|---|
| `arcweft-lang-sema` | Arcweft core language のみを知る。ambient adapter symbol は知らない。 |
| `arcweft-adapter-context` | adapter が提供する型、注入 binding、entry capability を記述する。 |
| project manifest / LaunchProfile | どの entry にどの adapter context を適用するかを選ぶ。 |
| `.arcw` source | entry の意図、route、flow 参照など、source として自然な情報だけを持つ。 |

source 側には、次の程度の shorthand を許すのがよい。

```arcw
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello
}
```

一方で、local / dev / production adapter、port、secret、env、TLS、deployment target のような host-profile 情報は manifest / LaunchProfile 側へ逃がす。

## 3. `arcw check` は strict のままにする

現在の判断、つまり generic check では adapter-injected symbol を受け入れず、adapter context は `serve/native-http` のような明示経路でのみ適用する方針は正しい。

今後は次のように分けるとよい。

```bash
# core language check only
arcw check samples/visual-novel-mini/src/server.arcw

# selected profile with adapter context
arcw check --profile server.dev

# convenience command; internally resolves a profile
arcw serve --entry http --adapter native-http --check-only
```

`entry server` があるだけで自動的に native HTTP context を適用すると、checker が source の形から runtime environment を推測することになる。これは、LSP・formatter・verifier の再現性を悪くする。

そのため、adapter context は必ず次のどちらかで有効化する。

1. `LaunchProfile` が選ばれたとき
2. 一時的な adapter option が明示されたとき

## 4. `route_params` は明示的な flow parameter へ寄せる

`route_params.name` は、当面の sample を通すには便利だが、恒久仕様としては ambient binding にしない方がよい。

推奨は、route declaration が flow signature に対して typed binding を作る形です。具体的な syntax は未確定でよいが、意味モデルとしては次を目指す。

```arcw
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello(name = :name)
}

flow @flow.hello hello(name: String) -> String {
    return name
}
```

または、複数 param をまとめる場合は explicit object にする。

```arcw
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello(params = route.params)
}

flow @flow.hello hello(params: RouteParams<{ name: String }>) -> String {
    return params.name
}
```

重要なのは、flow 側から見ると `route_params` が突然存在するのではなく、signature に現れる値として扱えることです。これにより、flow の再利用性、testability、LSP の補完、型検査の説明可能性が上がる。

### 実装後の扱い

既存 sample も明示 binding へ更新する。profile-selected adapter context でも
`route_params` は提供しない。

```arcw
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello(name = :name)
}

flow @flow.hello hello(name: String) -> String {
    return name
}
```

## 5. speaker option atoms は registry ができるまで strict にする

`face=smile` のような bare atom を global に許すと、未定義 symbol の typo と domain atom の区別ができなくなる。

したがって、現在の方針を維持する。

```text
bare unresolved atoms: rejected
short variant atoms: accepted
```

将来の許可条件は次のいずれかに限定する。

1. option/schema registry が `face` の候補として `smile` を定義している
2. expected type が enum / variant set として `smile` を含む
3. source が明示的に atom set を import している

例:

```arcw
alice(face=.smile, voice=auto, window=@textbox:.side)
```

この形は、`.smile` が short variant-style atom であることを明示できるので、bare atom より安全です。

## Manifest / profile のたたき台

`arcw.toml` などに、次のような profile を定義する。

```toml
[profiles."server.dev"]
kind = "server"
entry = "src/server.arcw#server"
adapter = "native-http"
listen = "127.0.0.1:8787"

[profiles."cli.main"]
kind = "cli"
entry = "src/main.arcw#main"

[profiles."test.opening"]
kind = "test"
entry = "tests/opening.arcw#opening"
```

profile resolver は、これを `ResolvedLaunchProfile` に変換する。

```text
manifest profile
  + source entry
  + adapter metadata
  + runtime options
  -> ResolvedLaunchProfile
  -> semantic context
  -> runner
```

## LSP / verifier / formatter への影響

この設計では、tooling は次の二段階を明確に扱える。

1. **Core source mode**: `arcw check file.arcw` 相当。Arcweft core language だけを見る。
2. **Profile mode**: `arcw check --profile server.dev` 相当。選択された adapter context を含めて見る。

LSP は active profile を選べるようにする。profile 未選択時は core source mode として strict に振る舞い、profile 選択時だけ `request` や route params 由来の補完を出す。

## 実装順序

### Phase 0: 現在の patch を維持

- generic `arcw check` は strict
- `arcw serve --adapter native-http` では adapter context を適用
- bare unresolved atoms は reject
- short variant atoms は accept

### Phase 1: profile resolver を追加

- manifest から `LaunchProfile` を読む
- command alias から temporary `LaunchProfile` を合成する
- `arcw run --profile ...` を正規実行経路にする
- `arcw check --profile ...` を追加する

### Phase 2: dedicated commands を lower する

- `arcw serve` → `LaunchProfile(kind=server)`
- `arcw cli` → `LaunchProfile(kind=cli)`
- `arcw test` → `LaunchProfile(kind=test)`
- `arcw bench` → `LaunchProfile(kind=bench)`

この時点で、command ごとの semantic special case を削っていく。

### Phase 3: route params を explicit parameter 化する

- route pattern から param schema を抽出する
- route declaration と flow signature を照合する
- ambient `route_params` は提供せず、明示的な route-to-flow binding を必須にする
- sample を `server.arcw` に更新する

### Phase 4: option/schema/atom registry を導入する

- speaker preset の option schema を定義する
- `face=smile` のような bare form は registry がある場合だけ許可する
- typo diagnostic を出せるようにする

## 最終判断

この件は、単に `route_params` をどこに入れるかではなく、Arcweft における「実行環境つき source 解釈」をどう表現するかの問題です。

そのため、短期 patch としては現在の strict 方針を維持しつつ、設計としては次を採用するのがよい。

```text
1. Core language semantics remains strict.
2. LaunchProfile is the canonical runtime/check context.
3. AdapterContext is selected by LaunchProfile.
4. Dedicated commands are user-facing aliases only.
5. Route params become explicit flow parameters.
6. Bare atoms require schema/registry support.
7. All source-level examples should use arcw, not awft.
```

