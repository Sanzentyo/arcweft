Rust Skillを末尾まで通読したうえで、現行`main`の設計文書と実装を照合しました。

## 結論

**大筋では正しい方向です。ただし、「CLIとnative playerを分ける」という判断は正しい一方、現在の分離境界はまだMVP寄りで、一段整理が必要です。**

特に、**native playerを`arcw-cli`へ統合するべきではありません**。むしろ今後は、

* `arcw-cli`
* native renderer
* native product player

を、現在より明確に分けるのがよいです。

現状を点数化するなら、方向性は **7〜8割正しい**、内部の実装分割は **5〜6割程度**です。

## 正しくできているところ

### `arcw`のバイナリエントリが薄い

`arcw`の`main.rs`は、引数をライブラリ側の`run`へ渡すだけです。CLIロジックをライブラリとして埋め込み可能にしている点は、Rust Skillの「CLI以外を再利用可能な`lib.rs`として実装する」という方針にも合っています。

また、`run_with_native_adapters`を公開しており、外部embeddingが独自native adapterを登録できる形になっています。これはCLIバイナリそのものに実行基盤を閉じ込めない、よい設計です。

### Coreとhost/playerを分離する思想は正しい

Arcweftの根本設計では、`arcweft-core`はSans I/Oであり、GPU、filesystem、network、window、audioなどはhost/adapter側に置かれます。CLIとplayer adapterがI/Oを持つのは明示された設計です。

したがって、

```text
arcweft-core
    ↑
runtime-host / renderer adapters
    ↑
arcw-cli / native player
```

という方向は正しいです。

native rendererが`arcweft-core`の外にあり、`LineDisplayFrame`を消費する設計も適切です。実装文書でも、rendererをcoreの外に置き、coreはtyped runtime eventだけを発行する方針が明記されています。

### CLIからruntime-hostを抽出したのも正しい

`arcweft-runtime-host`がbundle実行、native task bridge、runner reportなどを所有し、playerやLSP embeddingがCLI引数パーサーをリンクせず使えるようにしているのは、かなり良い改善です。

`arcweft-launch`についても、CLIサブコマンドを意味論の正本にせず、launch profileというSans I/Oデータへlowerする方針になっています。これも正しいです。

## 問題になり始めているところ

### 1. `arcweft-cli/src/app.rs`が巨大な統合点になっている

現在の`app.rs`は、少なくとも以下を一つのモジュールで扱っています。

* parse / HIR / sema
* runtime-plan lowering
* VM / AOT / JIT
* verification
* bundle
* test / bench / profile
* server
* Agent observation / MCP
* native image capture
* formatting / ID materialization

冒頭のimportだけでも、ほぼすべてのレイヤが集まっています。サブコマンドも`check`、`agent`、`verify`、`run`、`serve`、`test`、`bench`、`bundle`、`jit`、`fmt`などが単一の`CliCommand`から同じモジュールへdispatchされています。

CLIは最上位なので多くのcrateへ依存して構いません。しかし、**依存できることと、一ファイルで全責務を処理することは別**です。

少なくとも次のように、まずcrate追加なしで内部モジュールを分けるべき段階です。

```text
arcweft-cli/src/
  lib.rs
  app.rs
  context.rs
  project.rs
  commands/
    check.rs
    run.rs
    serve.rs
    bundle.rs
    verify.rs
    agent.rs
    test.rs
    bench.rs
    fmt.rs
  output/
    runtime.rs
    verify.rs
    agent.rs
```

`app.rs`は最終的に「parse argv → context構築 → command dispatch」だけにするのがよいです。

### 2. CLIがnative playerへ無条件依存している

現在、`arcweft-cli`は`arcweft-player-native`へ通常依存しています。optional dependencyではありません。CLI側のfeatureも、native playerを切り離す構成にはなっていません。

一方、`arcweft-player-native`は`wgpu`、`winit`、`glyphon`を無条件で持っています。

そのためCargo graph上は、単に

```bash
arcw check
arcw fmt
arcw verify
```

を使いたい場合でもnative GPU/window stackが付いてきます。

これは長期的にはよくありません。AGENTSの「backend-specific dependenciesはfeature flagとadapter crateの後ろに置く」という方針とも少しずれます。

望ましい形は次のどちらかです。

```toml
[features]
default = []
native-capture = ["dep:arcweft-render-native"]
```

またはnative captureを使うAgent CLIだけを別バイナリにする形です。

### 3. 現在の`arcweft-player-native`は、実質的には「完成版native player」ではない

名前は`player-native`ですが、現在の実体はかなり限定されています。

`compile_source`はplayer内部で、

```text
parse
→ HIR lowering
→ type check
→ runtime-plan lowering
```

を直接行っています。

しかし、Arcweftの最終的なnative product方針は、

```text
AOT compiled player
+ embedded .awfb / bytecode / asset bundle
```

です。native playerはplatform adapter経由でbundleとassetを読む設計です。

つまり現在のplayerは、厳密には、

> rich-text native rendererを検証するための、source compiler付きMVP player

です。

これはMVPとしては正しいですが、これをそのまま製品playerへ成長させると、製品実行ファイルにparser、HIR、sema、runtime-plan compilerまで入り続けます。

最終的には、source compilationを`arcw build`側へ移し、native playerは`.awfb`を受け取るべきです。source直接実行は`dev-player` featureに限定してもよいでしょう。

### 4. native rendererとplayer hostも分けた方がよい

`arcweft-player-native/src/native.rs`は、現在すでに次の責務を持っています。

* winit window lifecycle
* wgpu surface/offscreen rendering
* text shaping
* glyph placement
* vertical text
* ruby geometry
* effect registry
* renderer-local state
* object bounds
* mask / object-id / color capture
* Agent debug geometry

冒頭の型定義だけでも、frame capture、element bounds、debug region、visual plan、effects registryなどが同居しています。

ここは次の分離が自然です。

```text
arcweft-render-native
  wgpu / glyphon
  offscreen capture
  window surface
  native text layout submission
  renderer effects
  pixel/object geometry

arcweft-player-native
  bundle loading
  runtime-host
  scheduler
  input lifecycle
  audio lifecycle
  render-native orchestration
  save/load
  product main loop
```

すると`arcw agent observe`は、**player全体ではなく`arcweft-render-native`だけ**へoptional依存できます。

## native playerが別バイナリなのはどうか

**別バイナリで正しいです。**

native playerは、CLIとは異なる制約を持ちます。

* winit event loopをmain threadで所有する
* GPU surfaceのlifecycleを持つ
* audio/input/windowを継続駆動する
* 配布ゲームではcompilerや開発用コマンドを不要にしたい
* bundleを埋め込んだゲーム固有実行ファイルになり得る

設計文書でもnative runtimeの先頭は`winit main thread`と`wgpu RenderOwner`になっています。

ただし、**ユーザー向け入口まで二重にする必要はありません**。

理想的な役割分担は次のようになります。

```text
arcw check       静的検査
arcw build       .awfb / native product生成
arcw run         headless・決定的な開発実行
arcw play        native playerを使った対話実行
arcw agent ...   debug / observation

generated-game   配布用native player
```

`arcw play`が内部でnative player libraryをfeature付きで呼んでもよいですし、別実行ファイルを起動しても構いません。しかし、製品playerとtoolchain CLIの責務は統合しない方がよいです。

現在の`arcweft-player-native`自身にも`--headless`、`--json`、`--capture`などのCLIがあります。

これは当面のsmoke-test harnessとしては有用ですが、長期的に`arcw agent observe`と二つの正式な開発用CLIを維持すると、オプションやJSON契約がずれていきます。最終的には、

* `arcw`を開発者向けの正規CLI
* `arcweft-player-native`のargvは製品起動に必要な最小限
* rendererのテストはRust APIまたは専用test harness

にするとよいです。

## 推奨する最終構成

```text
arcweft-compiler
  source text
  → parsed/HIR/checked program
  → runtime plan / bytecode / bundle data
  Sans I/O

arcweft-runtime-host
  scheduler
  adapters
  bundle execution
  host requests

arcweft-render-native
  wgpu / winit / glyphon
  capture / native visual geometry

arcweft-player-native
  runtime-host + bundle + render-native
  product lifecycle

arcweft-cli
  filesystem/project discovery
  command orchestration
  compiler/runtime-host利用
  native renderingはoptional
```

現在のcrate mapも、presentation、player/headless、Agent CLIなどを最終的には細かく分ける構想になっています。今はMVP実装が`arcweft-cli`と`arcweft-player-native`へ圧縮されている状態だと見るのが自然です。

## 優先順位

いま最優先なのは、native playerをCLIへ統合することではなく、次の順です。

1. `app.rs`をサブコマンド単位に分割する。
2. parse→HIR→sema→runtime-planの共通driver APIを抽出する。
3. native rendering/captureをplayerから分離する。
4. CLIのnative renderer依存をoptionalにする。
5. native product playerをsource入力から`.awfb`入力へ移す。

したがって、最終判断は、

> **native playerが別なのは正しい。問題は「別であること」ではなく、現状のplayerがrenderer・compiler・debug adapterを兼務し、CLIがそれへ無条件依存していること。**

です。
