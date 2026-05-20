結論として、**次に進めるべきものは「Arcweft の最小実行前段パイプライン」**です。具体的には、`arcw check <file.awft>` と、そこから続く **dialogue line plan → Sans I/O runtime plan** の縦スライスを作るのが最優先です。

今の arcweft は、仕様追加よりも「すでにある構文・HIR・最小チェッカ・core データモデルを、1本の使える流れに閉じる」段階に見えます。

## 現状認識

workspace はすでに `arcweft-core`、`arcweft-dialogue`、`arcweft-id`、`arcweft-lang-syntax`、`arcweft-presentation`、`arcweft-cli` などを持つ Rust workspace になっています。

一方で実装ステータス上はまだ Phase 0 / Phase 1 の最小 workspace で、renderer、Servo、audio、camera、USB、MCP、Cranelift JIT などは明示的に deferred です。 つまり、ここで wgpu / Servo / 音声 / JIT に飛ぶのは早いです。

`arcweft-lang-syntax` はかなり進んでいて、`parse_source`、`lower_to_hir`、`validate_typecheck_ready`、`typecheck_hir`、`lint_id_policy` などが public surface として出ています。 さらに line plan、`thread`、`defer`、`finally`、`wait mark`、`'line.*` lifetime registry なども構文・チェック側でかなり扱われています。

最新コミットも `Implement flat blocks and scoped thread cleanup` で、`LineTaskGroup` の `finally`、`LineChildTask` の `defer_stack`、`thread` / `defer` / flat fence などを整理しています。 これは「言語表面の設計」から「runtime に渡せる line-scoped task model」へ移る合図です。

ただし `arcweft-cli` はまだ `arcw: Arcweft Engine CLI stub` を出すだけです。 ここが今いちばん大きな断絶です。

## 次に進めるべき本命

**`arcw check` + line-plan lowering の Phase 1.5** を作るべきです。

目標は「`.awft` ファイルを読む → CST/AST → HIR → reference/lint/typecheck-ready/minimal typecheck → line plan を Sans I/O runtime data に落とす」までを、renderer なし・VM 完全実装なし・外部I/Oなしで閉じることです。

Arcweft core の設計は Sans I/O で、core は副作用を実行せず、状態と effect request を返す思想です。 さらに `arcweft-core` 側にも `RuntimeStepInput` / `RuntimeStepOutput` / `LineTaskGroup` / `LineEffectRequest` の最小モデルがすでにあります。 だから次は「本物の renderer」ではなく、**言語からこの core データモデルへ落ちる変換**が一番筋が良いです。

## 具体的な PR 分割

**PR 1: `arcw check` を実装する。**
`arcw check path/to/file.awft` で `parse_source`、`lower_to_hir`、`validate_hir_references`、`lint_id_policy`、`validate_typecheck_ready`、`typecheck_hir` を順に走らせる。最初は JSON か人間向けテキストで diagnostics を出すだけでいいです。ここで重要なのは、CLI を「stub」から「開発者が毎回使う入口」に変えることです。

**PR 2: 代表的な `.awft` golden scenario を1本作る。**
題材は、今の実装が最も集中している dialogue line plan にするべきです。たとえば `alice.say(...)[ ... [mark .release_focus] ... ] with { init { ... } thread motion { wait mark .release_focus ... defer { ... } } finally { ... } }` のようなものです。仕様ドキュメントでも、line plan は `init`、`thread`、`defer`、`finally`、`wait mark`、`'line.focus` registry を含む中核例として扱われています。

**PR 3: HIR dialogue plan → `LineTaskGroup` への lowering を作る。**
ここではまだ実行しなくてよいです。`LinePlanItem::Thread` を `LineChildTask` に、`finally` を `LineTaskGroup.finally` に、`defer` を child task の `defer_stack` に、`wait mark` / `wait duration` / lifetime registry 操作を `LineEffectRequest` に変換できれば十分です。`LineTaskGroup` の構造はすでに core にあります。

**PR 4: presentation handle lifetime と line registry を同じテストで接続する。**
`arcweft-presentation` は `PresentationRegistry<T>` と `exit_scope` によって scope exit 時の cleanup を Sans I/O に表現できます。 line plan の `'line.focus <- ...` / drop / cleanup と presentation registry の scope cleanup を、実行ではなく「データモデル上の契約」としてテストするのが良いです。

## 後回しにすべきもの

**wgpu renderer、Servo/DOM、audio backend、camera/USB、MCP、Cranelift JIT はまだ後回し**がよいです。実装ステータスでもこれらは deferred であり、`AGENTS.md` でも core model がコンパイルしてテストを通る前にそれらへ進まない方針が書かれています。

また、これ以上大きな構文ファミリーを追加するのも抑えるべきです。今の構文面はすでに広いので、追加仕様より「1本の実例が parse / lower / check / plan 化まで通る」ことの価値が高いです。

## 判断

今の arcweft は、設計としてはかなり野心的で、docs も広いです。Arcweft Engine は独自 DSL、WASM、Cranelift、LayerTree、Servo/DOM UI、音声、デバイス、RAG、LLM debug まで含む大きな構想として整理されています。

だからこそ次は、構想をさらに広げるより、**「`.awft` を1本入力したら、Arcweft が何を理解し、どんな runtime plan を生成したかが見える」状態**にするべきです。

最優先名を付けるなら：

**Phase 1.5: `arcw check` + dialogue line task-plan lowering**

これが終わると、以後の renderer、VM、audio、LSP、formatter、bundle 生成のすべてが「何に接続すればよいか」を持てます。
