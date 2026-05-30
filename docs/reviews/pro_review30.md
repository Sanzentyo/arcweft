# Arcweft 性能改善レビュー

## 前提

添付の依頼内容・制約・現在値を前提に、実装上のホットスポット候補を優先順位順で整理します。特に `009_nonuniform_map_pure_batch.arcw` は parse phase が約 3.94ms、typecheck が約 0.25ms、runtime-plan lowering が約 0.30ms、実行 median が約 16.3µs なので、最初の大きな勝ち筋は parser 側です。

Arcweft は workspace 上も `arcweft-lang-syntax`、`arcweft-lang-hir`、`arcweft-lang-sema`、`arcweft-runtime-plan`、`arcweft-core`、`arcweft-runtime-accelerator`、`arcweft-runtime-scheduler`、`arcweft-cli` などに分かれており、依頼の層分離と一致しています。workspace lint でも `unsafe_code = "forbid"` が設定されています。  また、プロジェクト方針として `arcweft-core` は Sans I/O、parser は syntax crate、HIR/sema/runtime-plan は各 crate に留めること、互換 shim や removed syntax の温存をしないことが明示されています。 VM が意味論の正本で、Cranelift JIT や生成 Rust/Wasm helper は最適化・配布形式であって意味論の置換ではない、という境界も維持すべきです。

---

## 優先順位サマリ

| 優先 | Finding                                                          | 主な狙い                                       |
| -: | ---------------------------------------------------------------- | ------------------------------------------ |
| P0 | CST punctuation scan の一回化                                        | parse phase の反復 lex / scan を削る             |
| P0 | line/block を所有 `String` ではなく source range/slice で扱う              | parse 中の clone/concat を削る                  |
| P0 | numeric bracket sequence を summary AST/HIR として保持                 | 大きな数値列の Expr/Literal 大量生成を削る               |
| P1 | lossy/recovery/wikilink の cold path 化                            | 不要な全体 scan / allocation を削る                |
| P1 | runtime-plan の pure rewrite と map/sum optimize を単一 pass 化        | lowering の再帰 walk と suffix scan を削る        |
| P1 | TypeJudgment と expected check の clone/重複削減                       | typecheck の固定費を削る                          |
| P1 | borrow state snapshot を delta 化                                  | branch/borrow-heavy な将来負荷を下げる              |
| P2 | VM/AOT/JIT の auto policy を cold/warm 分離で決める                      | JIT compile overhead と steady runtime を分ける |
| P2 | typed sequence/memory view を core data、accelerator execution に分離 | Sans I/O を保った zero-copy 境界へ進める             |
| P2 | flat batch path を最優先し row packing を観測可能にする                       | pure boundary copy をさらに削る                  |
| P2 | multithread batch threshold を実測カウンタ駆動にする                         | 小さい batch の並列化損を避ける                        |
| P3 | scheduler/host bridge counters を phase/class 別に分ける               | thread/system bench の原因分解を可能にする            |

---

## P0-1. CST punctuation scan を「呼ぶたび re-lex」から「一回 scan の typed summary」へ移す

### 1. Finding title

**CST punctuation helper の repeated `lex_cst` を廃止し、line/block 単位の punctuation summary を導入する。**

### 2. Affected crates/modules

* `crates/arcweft-lang-syntax/src/cst.rs`
* `crates/arcweft-lang-syntax/src/parser.rs`
* `crates/arcweft-lang-syntax/src/parser/helpers.rs`
* `crates/arcweft-lang-syntax/src/parser/*` の block/head/body extraction 利用箇所

### 3. Why it is likely a bottleneck

現在の CST helpers は、`find_matching_punctuation`、`punctuation_delta`、`find_top_level_punctuation`、`find_last_top_level_punctuation` などが呼ばれるたびに `lex_cst(source)` を走らせています。`find_matching_punctuation` と `punctuation_delta` はその場で `lex_cst(source)` を回しています。 `find_top_level_punctuation` や `find_last_top_level_punctuation` も同様に source fragment を再 lex しています。

さらに `collect_brace_block` は、line を連結しながら top-level punctuation や depth delta を見て、最後に蓄積した `text` 全体に対して再度 punctuation helper を呼びます。 `collect_logical_block_items` も各 raw line について `{}`、`()`、`[]` の `punctuation_delta` を3回呼んでいます。

`009` の parse が約 3.94ms で、typecheck/lowering より一桁以上重い現状では、この「同じ fragment を何度も lex/scan する」構造が最優先の疑いです。

### 4. Concrete implementation plan

小さい commit に分けるなら、次の順です。

1. `CstLine` に typed punctuation summary を持たせる。

   * 例: `CstLinePunctuationSummary`

     * `brace_delta: i16`
     * `paren_delta: i16`
     * `bracket_delta: i16`
     * `first_top_level: Vec<PunctuationOffset>`
     * `last_top_level: Vec<PunctuationOffset>`
     * `has_top_level_unclosed_open: bool`
   * 新 dependency は不要。まずは `Vec` でよいです。

2. `CstLine::from_node` で `text` 文字列から分類するだけでなく、同じタイミングで punctuation summary も作る。

   * 既に `CstLine::from_node` は node text を `String` 化して line kind を分類しています。
   * ここで一回だけ token walk する構造へ寄せます。
   * 可能なら rowan token から直接作る。難しければ、初回は line text を一回 lex するだけでも、helper ごとの再 lex より改善します。

3. `punctuation_delta(raw_line, ...)` を hot path から外す。

   * `collect_logical_block_items` では `{}`、`()`、`[]` を3回別々に見るのではなく、1回の scan で3つの delta を返す `PunctuationDeltas` を使う。
   * body fragment のように `CstLine` ではない文字列には `PunctuationScan<'a>` を導入し、1回 lex した scan object から `find_*` を呼ぶ。

4. `find_matching_punctuation` の `open/close` 文字列 allocation を消す。

   * 現在は `open.encode_utf8(...).to_owned()` / `close...to_owned()` を作っています。
   * token kind 側に `char` または小さな enum を渡す API にします。
   * API 名は `find_matching_pair(scan, PunctuationPair::Brace)` のように typed にし、stringly API を増やさない。

5. flat fence sugar はそのまま保持する。

   * `parse_flat_fence` は authoring sugar であり legacy syntax ではないので、punctuation summary 化でこの経路を壊さない regression を追加します。該当実装は `parse_flat_fence` として存在します。

### 5. Tests or benchmarks to add/update

* `arcweft-lang-syntax` unit tests:

  * string/comment 内の `{`, `}`, `[`, `]`, `(`, `)` を depth に数えない。
  * nested brace block。
  * dialogue content bracket rescue。
  * flat fence sugar。
* Parser microbench:

  * `collect_brace_block` large body。
  * `collect_logical_block_items` large multi-line body。
  * 128/512 item numeric bracket sequence を含む fixture。
* Existing:

  * `just verify`
  * `just bench-009`
* 追加 counter 案:

  * `cst_lex_passes`
  * `punctuation_scans`
  * `punctuation_scan_bytes`
  * これは bench JSON に path-free で出す。

### 6. Expected risk and expected performance impact

* Risk: **中**。punctuation semantics は parser correctness に直結するため、string/comment/brace rescue の regression が必須です。
* Expected impact: **高**。`009` parse phase の主要因が repeated lex/scan なら、parse phase の 30–60% 改善が狙えます。実測前の推定ですが、現在のコード構造上もっとも根の深い parse bottleneck です。

---

## P0-2. `CstLine` / block extraction を owned `String` 中心から source range/slice 中心へ寄せる

### 1. Finding title

**source line splitting と block extraction の clone/concat を減らすため、line events を source range 化する。**

### 2. Affected crates/modules

* `arcweft-lang-syntax/src/parser.rs`
* `arcweft-lang-syntax/src/cst.rs`
* `arcweft-lang-syntax/src/parser/top_level.rs`
* `arcweft-lang-syntax/src/parser/helpers.rs`

### 3. Why it is likely a bottleneck

`parse_source` は `parse_cst(&source)` 後に `Parser::from_syntax(source.clone(), &syntax)` を呼び、parser 側に source の clone を渡しています。 `Parser` は `source: String` と `events: CstLineEvents` を持ちます。

`CstLine` 自体も `text: String` を持ちます。 `cst_lines` は CST line node から `CstLine::from_node` を作って `Vec<CstLine>` に collect します。 parser の main loop でも `line.text().to_owned()` と `line.trimmed().to_owned()` を作っています。

block extraction 側では `collect_brace_block` が block 全体を `String` に連結し、最後に head/body をまた `to_owned()` しています。 これは大きな fixture ほど parse allocation と copy が増えます。

### 4. Concrete implementation plan

1. `CstLine` を range-based にする。

   * 例:

     * `full_range: TextRange`
     * `content_range: TextRange`
     * `trimmed_range: TextRange`
     * `kind: CstTopLevelLineKind`
     * `item_kind: CstTopLevelItemKind`
     * `punctuation_summary: CstLinePunctuationSummary`
   * `text(&self, source: &str) -> &str`
   * `trimmed(&self, source: &str) -> &str`

2. `Parser` は `source: String` を持ち続けてもよいが、`CstLineEvents` は `String` を持たない。

   * 最初の commit では parser lifetime 化まで踏み込まなくてもよいです。
   * まず `CstLineEvents` の clone/copy を削るだけで小さく検証できます。

3. block extraction は `String` を組み立てる前に range で head/body を決める。

   * `collect_brace_block` は `start_line..end_line` と source byte range を返す。
   * AST が最終的に owned string を必要とする場所だけで `to_owned()` する。
   * raw/recovery fallback も `RawItem::new(trimmed.to_owned(), ...)` の直前まで slice のままにする。現状 raw fallback は `trimmed.to_owned()` しています。

4. `source_lines(source)` の `Vec<&str>` allocation を hot path から外す。

   * `source_lines` は split result を `Vec` に collect します。
   * line count だけなら `source.lines().count()` へ。
   * line event は CST pass で range を持つため、別途 source split しない。

5. `Parser::new(source: String)` が内部で `parse_cst(&source)` する API は残してよいが、nested parser 作成の hot path では使わない。

   * `Parser::from_syntax` または `Parser::from_line_events` を使う。
   * 互換 wrapper を増やすのではなく、syntax crate 内部の call site を明示的に置換する。

### 5. Tests or benchmarks to add/update

* CRLF / LF の line range regression。
* diagnostics の range が変わらないこと。
* raw item fallback の text が従来と同じこと。
* `parse_source` の source text が `ParsedSource` に一度だけ保持されることを unit test で確認。
* `just bench-009` の parse phase と allocation proxy counter:

  * `line_owned_strings`
  * `block_owned_bytes`
  * `raw_owned_bytes`

### 6. Expected risk and expected performance impact

* Risk: **中〜高**。range off-by-one が最も危険です。
* Expected impact: **高**。repeated scan 削減と合わせると、parse phase の主要 allocation が減ります。P0-1 と同時にやると原因が混ざるため、先に counter を入れ、次に range 化を入れるのが安全です。

---

## P0-3. 大きな numeric bracket sequence を `Vec<Expr>` ではなく summary AST/HIR として保持する

### 1. Finding title

**flat literal bracket sequence fast path を parser allocation fast path まで拡張する。**

### 2. Affected crates/modules

* `arcweft-lang-syntax/src/expr.rs`
* `arcweft-lang-hir`
* `arcweft-lang-sema/src/checker/expr.rs`
* `arcweft-runtime-plan/src/expr.rs`
* 将来的に `arcweft-core/src/value.rs`

### 3. Why it is likely a bottleneck

現状の `Expr::BracketSeq(Vec<Expr>)` は、数値列でも item ごとに `Expr::Literal(Literal::Int { raw: String, ... })` を持ちます。`Expr` と `Literal` は多くの `String` / `Vec` を所有する形です。 lexer も string literal、entity、lifetime path、numeric raw/suffix、ident などを `to_owned()` します。

最近入った `parse_flat_literal_bracket_seq` は Pratt recursion を避けていますが、まだ token 内の `Literal` を `clone()` して `Expr::Literal` を item 数だけ作ります。 typecheck 側には numeric sequence fast path が入り、item ごとの recursive typecheck は減っています。 つまり次の大きな残りは parse/AST allocation です。

### 4. Concrete implementation plan

1. syntax AST に明示 variant を追加する。

   * 例:

     * `Expr::NumericBracketSeq(NumericBracketSeq)`
     * `NumericBracketSeq { kind, len, values, suffix_summary, range_summary }`
   * 最初は integer のみでよいです。`009` が i64 pure batch なら最短で効きます。
   * mixed expression、array repeat `[x; n]`、trailing comma、float、suffix error は既存 `BracketSeq` / `ArrayRepeat` に逃がす。

2. `parse_flat_literal_bracket_seq` を `Vec<Expr>` 生成ではなく summary 生成に変える。

   * `Literal::Int` の `raw: String` を item ごとに残す必要があるかを再検討。
   * diagnostics 用に full raw が必要な場合は、個別 raw string ではなく source range と suffix summary にする。

3. HIR lowering は summary を壊さず運ぶ。

   * `HirExpr::NumericBracketSeq` 相当を追加するか、syntax `Expr` を HIR が参照しているなら variant をそのまま受ける。
   * broad root re-export は増やさず、明示 module に置く。

4. sema は expected item type と summary を照合する。

   * `expected_item.is_integer()` かつ全 item が integer literal なら O(1) 〜 O(n values) の軽い path。
   * `TypeJudgment` は sequence 全体に1件だけ記録する。
   * item ごとの `check_expected_numeric_literal` を避ける。

5. runtime-plan は summary から直接 runtime sequence を作る。

   * 短期: `RuntimeValue::BracketSeq(Vec<RuntimeValue::Int>)` を一回で作る。
   * 次段階: `RuntimeValue::I64Seq(Vec<i64>)` または `RuntimeValue::TypedSeq(RuntimeSeqValue::I64(Vec<i64>))` を追加し、pure batch が `&[i64]` を借用できるようにする。
   * `arcweft-core` に置くのは pure data representation までで、JIT/Cranelift/Rayon execution は `arcweft-runtime-accelerator` に残す。

### 5. Tests or benchmarks to add/update

* Syntax:

  * `[1, 2, 3]` が summary。
  * `[1, 2,]` が summary。
  * `[1; 3]` は `ArrayRepeat`。
  * `[1, foo]` は従来の `BracketSeq`。
  * `[1_i64, 2_i64]` suffix handling。
* Sema:

  * expected `Vec<i64>` / `[i64; N]`。
  * mismatched len。
  * invalid suffix。
* Runtime:

  * `009_nonuniform_map_pure_batch.arcw` の counters が維持されること。
  * `pure calls = 128`, `batch calls = 1`, `batch items = 128`, `arg_vec_allocs = 0` を regression。
* Bench:

  * parse phase。
  * typecheck expressions/judgments。
  * runtime-plan lowering。
  * borrowed/copied arg bytes。

### 6. Expected risk and expected performance impact

* Risk: **中**。AST/HIR/sema/runtime-plan にまたがるため、段階的に進めるべきです。
* Expected impact: **高**。sequence-heavy fixture では parse allocation と Expr construction が大きく減ります。typecheck はすでに速くなっていますが、judgment/item loop もさらに減ります。

---

## P1-4. lossy/recovery/wikilink の全体 scan と allocation を cold path 化する

### 1. Finding title

**`parse_expr_lossy` と wiki/recovery 系の eager allocation を避ける。**

### 2. Affected crates/modules

* `arcweft-lang-syntax/src/parser.rs`
* `arcweft-lang-syntax/src/parser/helpers.rs`
* `arcweft-lang-syntax/src/expr.rs`

### 3. Why it is likely a bottleneck

parser は parse 開始時に `collect_wiki_links(&self.source)` を常に呼びます。 `collect_wiki_links` は全 source を scan し、body string を clone します。

`parse_expr_lossy` は必ず `normalize_dot_continuations(source)` を呼び、single-line でも `String` を作ります。 `normalize_dot_continuations` は first line を `to_owned()` してから各 line を push します。 dialogue/index rescue でも content を expression として parse できるかを見ており、成功/失敗判定のためだけに expression parser が走ります。

### 4. Concrete implementation plan

1. wiki link collection に fast precheck を入れる。

   * `if !source.contains("[[") { Vec::new() }`
   * 依存 crate は追加しない。
   * これだけで wiki を使わない bench fixture の全体 scan を消せます。

2. `normalize_dot_continuations` を `Cow<'_, str>` 返却にする。

   * 改行がない場合は borrowed。
   * 改行があっても dot continuation がない場合は必要最小限で borrowed/owned を決める。
   * `parse_expr_lossy` は `let normalized = normalize_dot_continuations(source); let source = normalized.trim();` のまま使える。

3. raw string literal path も必要になるまで clone しない。

   * 最終的に `Expr::Literal(Literal::String)` にする時だけ owned string にする。
   * raw fallback `Expr::Raw(source.to_owned())` は error path なので維持でよい。

4. dialogue/index rescue は token summary で obvious case を先に落とす。

   * `callee.contains('(') || parse_expr(content).is_err()` の前に、content が単純 text で expression になり得ない場合を token summary で判定する。
   * ただし `speaker.say()[...]` rescue は維持する。

### 5. Tests or benchmarks to add/update

* Wiki link あり/なしの parse regression。
* single-line expression が allocation-free path を通る unit test。
* dot continuation expression。
* dialogue call:

  * `speaker.say()[text]`
  * `nums[0]`
  * `alice[おはよう。[p]]`
* Bench:

  * parse phase。
  * new counters:

    * `wiki_scan_performed`
    * `dot_normalization_owned`
    * `dialogue_rescue_expr_parse_attempts`

### 6. Expected risk and expected performance impact

* Risk: **低〜中**。`Cow` 化は局所的ですが、dialogue/index rescue は regression を厚くする必要があります。
* Expected impact: **中**。P0-1/P0-2 より小さいですが、小さな安全 commit として先に入れてもよいです。

---

## P1-5. runtime-plan lowering の pure rewrite と map/sum optimization を単一 pass 化する

### 1. Finding title

**`lower_runtime_plan` 後の再帰 rewrite/optimize を減らし、lowering 時に pure-aware RuntimeExpr を作る。**

### 2. Affected crates/modules

* `arcweft-runtime-plan/src/flow.rs`
* `arcweft-runtime-plan/src/expr.rs`
* `arcweft-runtime-plan/src/pure.rs`
* `arcweft-core/src/value.rs` の `RuntimeExpr` 周辺

### 3. Why it is likely a bottleneck

`lower_runtime_plan` は flow を lower した後、pure candidates を lower し、最後に `rewrite_runtime_plan_pure_calls` で plan 全体を再走査します。 その後 `optimize_flow_ops` が nested ops を再帰的に処理し、`fuse_adjacent_map_sum_lets` と `inline_unused_sequence_lets_into_following_map_sums` をかけています。

特に fuse/inline は `flow_ops_use_local(&ops[index + 2..], name)` を呼び、suffix の ops と nested `RuntimeExpr` を再帰的に歩きます。`runtime_expr_uses_local` は多くの RuntimeExpr variant を再帰 walk します。 現在の lowering は約 0.30ms で parse より小さいですが、fixture が大きくなると O(n²) 的な suffix scan が効きます。

### 4. Concrete implementation plan

1. `lower_runtime_plan` の順序を変える。

   * 先に `lower_pure_helper_candidates(module)` を実行。
   * `pure_helper_map` を作る。
   * `FlowRuntimeLowerer` に `PureRewriteContext` を渡す。
   * `lower_runtime_expr_strict` の call lowering 時点で、callee が pure helper なら `RuntimeExpr::PureCall` を直接作る。
   * これで flow plan に対する pure rewrite pass を削れる。

2. source/stream lowering にも同じ context を渡す。

   * flow だけ先にやると source/stream の rewrite pass が残るため、最終的には `source` / `stream` lowerer も pure-aware にする。
   * 一 commit 目は flow のみでも measurable です。

3. map/sum optimize は reverse use-count pass にする。

   * 各 scope の ops を末尾から走査し、`LocalUseCounts` を作る。
   * adjacent map/sum の temp が後続で使われないことを suffix scan ではなく count で判定する。
   * `RuntimeExpr` の use count は1回だけ walk する。

4. `RuntimeExpr::MapSum` または `RuntimeExpr::ReduceMap` を検討する。

   * 現状 `Sum { source: Map { ... } }` を pattern として見ているなら、専用 variant にすると VM/JIT dispatch が単純になります。
   * ただし最初は IR 追加なしで pass 削減を優先。

### 5. Tests or benchmarks to add/update

* Runtime-plan unit tests:

  * pure call が lowering 時点で `RuntimeExpr::PureCall` になる。
  * source/stream にも pure rewrite が残らない。
  * adjacent map/sum fusion の結果が従来と同じ。
  * temp local が後続で使われる場合は inline しない。
* Bench:

  * `just bench-009`
  * `just bench-thread`
  * `007_branching_iter_pure_jit.arcw`
* Before/after data:

  * runtime plan lowering ns。
  * pure helper count。
  * plan rewrite visits。
  * runtime expr use-scan visits。
  * fused map/sum count。
  * inline sequence-let count。

### 6. Expected risk and expected performance impact

* Risk: **中**。pure helper discovery と flow lowering の順序変更は semantics には影響しないはずですが、source/stream まで含めると範囲が広がります。
* Expected impact: **中**。`009` の lowering 0.30ms に対しては小〜中ですが、plan が大きくなるほど効きます。特に repeated expression walks の削減は将来の fixture で効きます。

---

## P1-6. pure-helper candidate discovery で strict expression lowering を重複させない

### 1. Finding title

**pure helper extraction と runtime expression lowering の共通 lowered body cache を導入する。**

### 2. Affected crates/modules

* `arcweft-runtime-plan/src/pure.rs`
* `arcweft-runtime-plan/src/expr.rs`
* `arcweft-runtime-plan/src/flow.rs`

### 3. Why it is likely a bottleneck

pure helper extraction は全 function を走査し、annotated/inferred を試みます。 helper body lowering では final value に `lower_runtime_expr_strict(value)` を呼び、let statement でも各 expr に `lower_runtime_expr_strict` を呼んで `RuntimeExpr::Let` へ畳み込みます。

flow lowering 側でも expression lowering を別途行います。`lower_runtime_expr_strict` は再帰的に expression tree を lower します。 小さい helper では問題になりにくいですが、自然に書かれた pure function を自動加速する方針なら、candidate discovery が broad speculative pass にならないように抑えるべきです。

### 4. Concrete implementation plan

1. `PureHelperCandidate` に lowered body だけでなく簡易 shape summary を持たせる。

   * `input_arity`
   * `supports_scalar_i64`
   * `contains_call`
   * `contains_branch`
   * `expr_weight`
   * これは backend selection にも使えます。

2. `lower_runtime_expr_strict` の結果を helper 単位で一度だけ作る。

   * helper function body から `RuntimeExpr` を作り、`RuntimePureHelper` に clone するのは必要最小限。
   * `runtime_pure_helpers` は `candidate.expr().clone()` しています。 ここは clone されますが、候補数が少なければ許容。大きい expression では `Arc<RuntimeExpr>` も検討できます。ただしまずは clone count counter を入れる。

3. inferred helper の失敗理由は広く保持しない。

   * annotated helper の error は diagnostics。
   * inferred helper の失敗は counter のみ。
   * 互換 shim ではなく、探索コスト削減です。

4. pure helper map を name string だけでなく typed id に寄せる。

   * 現状 `BTreeMap<String, RuntimePureHelperId>` です。
   * HIR function symbol id があるなら `FunctionId -> RuntimePureHelperId` にする。
   * なければこの finding では触らず、string map の probe 回数だけ counter 化します。

### 5. Tests or benchmarks to add/update

* annotated pure helper の failure は従来通り error。
* inferred helper の unsupported syntax は diagnostics ではなく counter のみ。
* `#[pure]` + removed `Int` alias rejection は維持。既存 test はこの方針と合います。
* Counters:

  * `pure_candidate_functions_seen`
  * `pure_candidate_lower_attempts`
  * `pure_candidate_lower_failures_inferred`
  * `pure_expr_lowered_nodes`
  * `pure_expr_cloned_nodes`

### 6. Expected risk and expected performance impact

* Risk: **低〜中**。candidate discovery の semantics を変えず、まず counter/cache から始めれば安全です。
* Expected impact: **低〜中**。`009` 単体では大きくない可能性がありますが、pure helper が増えるほど効きます。

---

## P1-7. TypeJudgment と expected check の clone/重複を減らす

### 1. Finding title

**typecheck report の証跡は維持しつつ、hot path の `TypeKind` clone と duplicate expected judgment を削る。**

### 2. Affected crates/modules

* `arcweft-lang-sema/src/checker.rs`
* `arcweft-lang-sema/src/checker/expr.rs`
* `arcweft-cli/src/output.rs`

### 3. Why it is likely a bottleneck

`TypeCheckStats` は expressions/judgments/borrow snapshot 系の counters を持っています。 `TypeJudgment` は `ty: TypeKind` と `expected: Option<TypeKind>` を所有します。 `record_type_judgment` は expected を `cloned()` し、judgment を push します。

さらに `expect_expr_type` は `check_expr_with_expected` を呼んだ後、expected judgment を記録し、さらに compatibility check を行います。 一方 `check_expr_with_expected` 自身も expected があると `TypeJudgmentRule::Expected` として judgment を記録します。 これは expected 経路で証跡と clone が重複しやすい構造です。

### 4. Concrete implementation plan

1. `TypeJudgmentSubject::Expr { kind: String }` を typed enum または `&'static str` にする。

   * `expr_kind_name(expr).to_owned()` を hot path から消す。
   * CLI output の JSON 変換時だけ string 化する。
   * 既に CLI は judgment sample を string 化しています。

2. expected judgment を1箇所に統一する。

   * `expect_expr_type` は `check_expr_with_expected` に「context」を渡し、そこで1件だけ expected judgment を記録する。
   * あるいは `check_expr_with_expected` は raw inference のみ記録し、`expect_expr_type` だけが expected judgment を記録する。
   * どちらかに統一し、`expected_judgments` が意味通りになるようにする。

3. `types_compatible` の結果を expected check 内で再利用する。

   * `check_expr_with_expected` が `CheckedExpr { ty, compatible }` を返す形にする。
   * public API ではなく checker 内部型に留める。
   * `types_compatible` は Choice/Result/Option で recursive に走ります。

4. `TypeCheckReport` の full judgments と bench profile を分ける。

   * CLI profile は sample を最大8件しか出していません。
   * bench 用には full `Vec<TypeJudgment>` ではなく counters + samples だけを生成する typed mode を追加できます。
   * ただし API を広げすぎず、`TypeCheckReportOptions { judgment_mode }` のような明示型にする。

### 5. Tests or benchmarks to add/update

* Existing typecheck tests。
* New tests:

  * expected judgment count が重複しない。
  * `judgment_samples` の JSON が変わりすぎない。
  * `types_compatible_calls` counter が expected sequence/check で減る。
* Bench:

  * `009` の typecheck ns。
  * `typecheck.expressions`
  * `typecheck.judgments`
  * `typecheck.judgment_rules.expected`
  * `type_clone_count`
  * `compatibility_checks`

### 6. Expected risk and expected performance impact

* Risk: **低〜中**。diagnostics より report shape に影響しやすいので snapshot 更新が必要です。
* Expected impact: **低〜中**。`009` の typecheck はすでに約 0.25ms なので優先度は parser より低いですが、compiler pipeline 全体の固定費削減として有効です。

---

## P1-8. borrow state snapshot/merge を full `HashMap` clone から delta snapshot へ変える

### 1. Finding title

**borrow checker の branch snapshot を変更差分で表現し、clone/rebuild を減らす。**

### 2. Affected crates/modules

* `arcweft-lang-sema/src/borrow.rs`
* `arcweft-lang-sema/src/checker/borrow_state.rs`
* `arcweft-lang-sema/src/checker/stmt.rs`
* `arcweft-lang-sema/src/checker/expr.rs`

### 3. Why it is likely a bottleneck

`BorrowStateSnapshot` は `HashMap<String, BorrowLocalState>` を丸ごと持ちます。 `snapshot_borrow_state` は `borrow_local_lifetimes.clone()` を取り、cloned binding count も map 全体の長さを足しています。 restore も map 全体を戻し、active borrows を rebuild します。 merge は新しい `HashMap` を作り、base の全 key を走査して `merge_borrow_local_states` します。

`merge_borrow_local_states` は `BTreeSet` を作り、lifetime string を clone して union します。 borrow-heavy な script や branch が多い flow では効きます。

### 4. Concrete implementation plan

1. `BorrowStateCheckpoint` を追加する。

   * `borrow_local_lifetimes` の全 clone ではなく、変更前 state を journal に積む。
   * 例:

     * `checkpoint_id`
     * `journal_start`
     * `active_borrow_depth`
   * `register_borrow_bindings`、`release_borrow_local`、`clear_borrow_local` が変更前 state を一度だけ journal に記録する。

2. branch snapshot は `BorrowStateDelta` にする。

   * branch 開始 checkpoint からの touched keys のみを保持。
   * merge は base 全体ではなく、各 branch の touched key union だけを見る。
   * 変更されなかった borrow local は base を共有。

3. `active_borrows: Vec<String>` の linear remove を見直す。

   * 現状 `remove_active_borrow_lifetime` は `.position()` で探して `swap_remove` しています。
   * order が診断に不要なら `HashMap<String, usize>` count + `active_borrow_total` にする。
   * deterministic output が必要なら sorted snapshot は report 時だけ作る。

4. counters を増やす。

   * `borrow_state_delta_entries`
   * `borrow_state_full_clones`
   * `borrow_state_merge_keys`
   * `active_borrow_linear_removes`

### 5. Tests or benchmarks to add/update

* Existing borrow/lifetime tests。
* New targeted tests:

  * borrow local が branch 片側だけ drop される。
  * both branch live。
  * both branch dropped。
  * nested branch。
  * no borrow branch で `state_cloned_bindings == 0` になる。
* Bench:

  * borrow-heavy fixture を追加。
  * `009` では regression check 程度。

### 6. Expected risk and expected performance impact

* Risk: **中**。borrow checker correctness に関わるため、branch merge semantics の tests を増やす必要があります。
* Expected impact: **中**。現在の `009` では大きくないかもしれませんが、borrow/lifetime feature が増えるほど clone storm を避けられます。

---

## P2-9. VM/AOT/JIT/ batched JIT の auto policy を cold/warm 分離で決める

### 1. Finding title

**自然に書かれた pure function を、compile cost を別会計にした auto policy で加速する。**

### 2. Affected crates/modules

* `arcweft-runtime-accelerator/src/lib.rs`
* `arcweft-core/src/pure.rs`
* `arcweft-cli/src/output.rs`
* `arcweft-cli/src/main.rs`

### 3. Why it is likely a bottleneck

`RuntimePureAccelerator` は backend mode と worker policy を持ち、compile stats と runtime stats を分けています。compile stats には JIT/AOT attempts/success/failure、cache hits/misses、`compile_elapsed_ns` があります。 ただし `with_config` は helper を初期化時に全て compile し、`compile_elapsed_ns` をそこで記録します。

Auto mode は JIT、AOT、VM の順で試します。 `003` のような pure calls 16 件の scalar loop では、JIT compile cost が steady runtime に対して大きすぎる可能性があります。一方 `009` のような 128 item flat batch/sum は JIT batch が合います。

### 4. Concrete implementation plan

1. backend 選択ルールを明示する。

   * **VM**

     * dynamic `RuntimeValue` args。
     * helper が i64 scalar subset ではない。
     * call count / rows が非常に少なく、compile amortization が見込めない。
   * **AOT**

     * i64 helper。
     * call count は中程度。
     * JIT unsupported または compile overhead を避けたい。
     * scalar repeated calls。
   * **JIT**

     * i64 helper。
     * 同一 helper の call count が多い。
     * helper expression weight が高い。
     * warm cache 前提で steady run を測る。
   * **batched JIT**

     * flat contiguous input がある。
     * arity known。
     * rows が threshold 以上。
     * `sum(map(...))` は `call_i64_flat_batch_sum` を優先。

2. eager compile から lazy promotion へ段階的に移す。

   * 初回は AOT または VM。
   * `RuntimePureDispatchProfile` を helper ごとに持つ。

     * `calls`
     * `batch_rows`
     * `flat_batch_rows`
     * `estimated_expr_weight`
     * `observed_backend_ns`
     * `jit_compile_ns`
   * threshold を超えたら JIT compile。
   * user visible config は残してよいが、default Auto は profile-driven。

3. cold/warm を bench JSON で分離する。

   * 既に bench output は pure compile stats を持っています。`ScriptBenchPureHelperRuntimeBatchSummary` は config/compile/stats を持ちます。
   * 追加:

     * `cold_start_elapsed_ns`
     * `compile_elapsed_ns`
     * `steady_elapsed_ns`
     * `warm_cache_hits`
     * `warm_cache_misses`
   * 単純に `elapsed - compile_elapsed` で差し引くのではなく、compile を含む sample と含まない sample を別々に測る。

4. `reset_runtime_counters` の意味を明確化する。

   * 現状は runtime stats と cache hit/miss を reset しますが、compile attempts/elapsed は reset しません。
   * `reset_steady_counters` のような名前にするか、compile stats reset を別 API にする。
   * 互換 alias は不要。内部 cleanup として rename/update する。

### 5. Tests or benchmarks to add/update

* `003_for_pure_jit.arcw`

  * VM/AOT/JIT の cold/warm。
  * calls = 16。
* `007_branching_iter_pure_jit.arcw`

  * mixed map/for。
  * batch items = 16。
* `009_nonuniform_map_pure_batch.arcw`

  * flat batch/sum。
  * batch items = 128。
* Before/after data:

  * `compile_elapsed_ns`
  * `steady_elapsed_ns`
  * `pure_calls`
  * `batch_calls`
  * `batch_items`
  * `jit/aot/vm_calls`
  * `cache_hits/misses`
  * `arg_bytes_copied/borrowed`
  * `result_bytes_copied`
  * `thread_pool_jobs`

### 6. Expected risk and expected performance impact

* Risk: **中**。Auto policy は測定結果に影響するため、cold/warm JSON を先に足して観測可能にしてから policy を変えるべきです。
* Expected impact: **中〜高**。small scalar loop では JIT compile 損を避け、large batch では JIT batch を選べます。user-facing config を増やさず default Auto を賢くするのが目標です。

---

## P2-10. typed memory view は `arcweft-core` に pure data view、native execution は accelerator に置く

### 1. Finding title

**Sans I/O を壊さず、array/slice/repeated scalar の boundary copy を減らす typed sequence view を導入する。**

### 2. Affected crates/modules

* `arcweft-core/src/value.rs`
* `arcweft-core/src/pure.rs`
* `arcweft-runtime-plan/src/expr.rs`
* `arcweft-runtime-accelerator/src/lib.rs`

### 3. Why it is likely a bottleneck

`RuntimePureCallBackend` は scalar、borrowed slice、row batch、flat batch、flat batch sum、repeated flat batch sum の API をすでに持っています。 これは良い方向です。VM fallback の borrowed slice path は `arg_bytes_borrowed` を増やし、copy を避けています。 Accelerator も `flat_i64_inputs`, `aot_i64_slots`, `vm_scratch` を持ち、境界 state を adapter 側に置いています。

次の課題は、parser/HIR/sema/runtime-plan で得た typed sequence 情報を VM/JIT boundary まで失わないことです。

### 4. Concrete implementation plan

1. `arcweft-core` には raw pointer ではなく pure data view を置く。

   * 例:

     * `RuntimeValue::I64Seq(Vec<i64>)`
     * または `RuntimeValue::TypedSeq(RuntimeTypedSeq::I64(Vec<i64>))`
   * `&[i64]` として borrow できる API を safe Rust で提供する。
   * `unsafe` は不要。

2. `arcweft-runtime-plan` は typed sequence summary から `RuntimeValue::I64Seq` を作る。

   * まず numeric bracket sequence のみ対象。
   * mixed expression は従来 `BracketSeq`。

3. `arcweft-core` VM は `I64Seq` を deterministic value として扱う。

   * core に Cranelift/Rayon/OS resource は入れない。
   * Core は value storage と VM semantics だけ。

4. `arcweft-runtime-accelerator` は `&[i64]` を借りて flat batch API に渡す。

   * Native memory ownership や thread pool は accelerator/adapter 側。
   * 既存方針通り、accelerator crate は native acceleration state を所有し、core を Sans I/O のままにしています。

5. repeated scalar calls は `call_i64_repeated_flat_batch_sum` を優先する。

   * 既に repeated row を一回評価して rows 倍する API が存在します。
   * plan lowering で identical row を検出できる場合はここへ落とす。

### 5. Tests or benchmarks to add/update

* Runtime value tests:

  * `RuntimeValue::I64Seq` equality/debug/serialization summary。
  * VM eval of sequence。
* Pure backend tests:

  * `I64Seq` -> `call_i64_flat_batch_sum`。
  * fallback VM と JIT の conformance。
* Bench before/after:

  * `arg_bytes_copied`
  * `arg_bytes_borrowed`
  * `result_bytes_copied`
  * `flat_batch_materializations`
  * `typed_seq_values`
  * `typed_seq_borrowed_bytes`

### 6. Expected risk and expected performance impact

* Risk: **中**。RuntimeValue の variant 追加は広く影響しますが、pure data なので Sans I/O 境界は守れます。
* Expected impact: **中〜高**。large sequence + pure batch で boundary copy と RuntimeValue item allocation を減らせます。

---

## P2-11. row batch より flat batch を優先し、flatten copy を counter で見える化する

### 1. Finding title

**`RuntimeI64Args` rows を JIT 直前で flatten する経路を減らし、flat input を plan/VM から渡す。**

### 2. Affected crates/modules

* `arcweft-core/src/pure.rs`
* `arcweft-runtime-accelerator/src/lib.rs`
* `arcweft-core/src/engine/eval.rs`
* `arcweft-runtime-plan/src/flow.rs`

### 3. Why it is likely a bottleneck

Accelerator の `call_i64_batch` は `rows: &[RuntimeI64Args]` を受けた場合、JIT path では `call_jit_batch` で `flat_i64_inputs` に `extend_from_slice` してから compiled batch を呼びます。 一方 `call_i64_flat_batch` は `flat_inputs: &[i64]` を直接受け取り、borrowed bytes として count しています。

現在の `009` は borrowed arg bytes が 2048、arg vec allocs 0 なのでかなり良い状態ですが、row batch 経路に落ちる fixture では flatten copy が見えにくいです。

### 4. Concrete implementation plan

1. Runtime lowering で flat batch candidate を明示する。

   * map source が typed contiguous `I64Seq`。
   * helper arity が 1 または fixed small arity。
   * closure body が pure helper call。
   * この場合は row pack を作らず flat slice を渡す。

2. `RuntimePureCallStats` に flatten/materialization counters を追加する。

   * `flat_batch_calls`
   * `flat_batch_items`
   * `flat_batch_bytes_borrowed`
   * `row_batch_items`
   * `row_stack_packs`
   * `flatten_materializations`
   * `flatten_bytes_copied`

3. `call_jit_batch` の flatten を hot path から外す。

   * 既存 API は残してよいが、new lowering は `call_i64_flat_batch` / `call_i64_flat_batch_sum` を使う。
   * compatibility shim ではなく internal path の優先順位変更です。

4. `sum(map(...))` は output buffer を作らない。

   * 既に JIT flat batch sum は output copy を避けています。
   * counter で `result_bytes_copied == 0` を regression にする。

### 5. Tests or benchmarks to add/update

* New fixture:

  * row batch に落ちる multi-arity helper。
  * flat batch に落ちる single-arity helper。
* Existing:

  * `009_nonuniform_map_pure_batch.arcw`
* Before/after:

  * `flatten_materializations`
  * `flatten_bytes_copied`
  * `arg_bytes_borrowed`
  * `arg_bytes_copied`
  * `result_bytes_copied`
  * `pure_batch_items`
  * `elapsed_ns`

### 6. Expected risk and expected performance impact

* Risk: **低〜中**。counter 追加は低 risk。lowering の flat preference は conformance tests が必要です。
* Expected impact: **中**。既に `009` は良い path に乗っている可能性がありますが、他 fixture の row packing を避ける効果があります。

---

## P2-12. multithreaded batching は rows × expr_weight × backend で threshold を決める

### 1. Finding title

**`batch_min_len * workers` だけでなく、backend と helper cost を見て parallelize する。**

### 2. Affected crates/modules

* `arcweft-runtime-accelerator/src/lib.rs`
* `arcweft-cli/src/output.rs`
* `arcweft-cli` bench surfaces

### 3. Why it is likely a bottleneck

現在の parallel decision は `resolved_workers > 1 && len > batch_min_len * resolved_workers` です。 また batch/flat batch/sum は JIT では基本的に single compiled batch call、AOT/VM では parallelize 可能という分岐です。 

小さい rows、軽い helper、JIT flat batch では thread pool overhead が勝ちやすいです。逆に VM/AOT fallback で helper expression が重い場合は早めに並列化した方がよい可能性があります。

### 4. Concrete implementation plan

1. helper shape summary を threshold に使う。

   * `expr_weight`
   * `contains_branch`
   * `contains_call`
   * `estimated_ops_per_row`
   * `arity`

2. backend 別 threshold にする。

   * JIT flat batch: 原則 single-thread。大規模 rows だけ将来検討。
   * AOT: `rows * expr_weight` が threshold 以上。
   * VM: AOT より低い threshold で parallelize。
   * repeated row sum: parallelize しない。既存 API のように1回評価して rows 倍でよい。

3. thread pool build cost を分けて測る。

   * `ensure_thread_pool` は必要時に pool を作ります。
   * pool build が sample に混ざると threshold 評価が歪むため、

     * `thread_pool_build_elapsed_ns`
     * `thread_pool_reused`
     * `thread_pool_jobs`
     * `rows_per_job`
       を出す。

4. `batch_min_len` は config として残すが、Auto では補助値にする。

   * user-facing の visible config を増やすのではなく、Auto policy の内部判断にする。

### 5. Tests or benchmarks to add/update

* `--pure-workers 1/2/4/auto`
* `--pure-batch-min-len 16/64/256`
* Fixtures:

  * tiny helper, rows 16。
  * medium helper, rows 128。
  * heavy helper, rows 1024。
* Before/after:

  * `thread_pool_jobs`
  * `thread_pool_build_elapsed_ns`
  * `parallel_taken`
  * `parallel_skipped_reason`
  * `rows_per_worker`
  * elapsed median/min/max。

### 6. Expected risk and expected performance impact

* Risk: **低〜中**。policy change は benchmark variance に影響しますが semantics は変わりません。
* Expected impact: **中**。小さい batch の過剰並列化を避け、大きい VM/AOT batch では throughput を上げられます。

---

## P3-13. scheduler counters を sort / marker / host I/O / fanout / pressure / normalization に分割する

### 1. Finding title

**thread scheduling と host bridge の counters を「何が遅いか」まで分解する。**

### 2. Affected crates/modules

* `arcweft-runtime-scheduler/src/lib.rs`
* `arcweft-cli/src/native_task.rs`
* `arcweft-core/src/task.rs`
* `arcweft-cli/src/output.rs`

### 3. Why it is likely a bottleneck

scheduler は Sans I/O で、pending/in-flight/join/cancel を持ちます。stats には submitted/joined/dispatched/completed/failed/cancelled/in_flight/max_in_flight、dispatch/completion sort counts/items があります。 dispatch は sort count/items を記録し、completion は events を normalize して sort count/items を記録します。

CLI native bridge 側には completed/failed/read/write/system_info/bytes/parallel batches/tasks/io/system/marker/workers と scheduler stats があります。 ただし marker-only tasks、real host I/O tasks、system info tasks は parallel batch 内では数えていますが、全体の submitted/dispatched/completed class breakdown としては不十分です。

### 4. Concrete implementation plan

1. scheduler core には elapsed time を入れない。

   * Sans I/O を守るため、`arcweft-runtime-scheduler` は count のみ。
   * CLI/host adapter が `Instant` で phase time を測る。

2. `RuntimeSchedulerStats` に class breakdown を追加する。

   * `submitted_by_class: TaskClassCounts`
   * `dispatched_by_class`
   * `completed_by_class`
   * `marker_submitted`
   * `marker_dispatched`
   * `marker_completed`
   * `host_io_dispatched`
   * `system_info_dispatched`
   * `cpu_dispatched`
   * `background_dispatched`
   * `joined_waiters`
   * `joined_events_created`

3. `TaskClass` は既に typed enum として存在します。

   * `TaskClass::{Io, Cpu, Background, ...}` が core にあります。
   * `HostTaskRequest::task_class()` は request を typed class に分類しています。
   * marker-only は `HostTaskRequest::Custom { capability, operation }` の `"line_task" | "flow_thread"` + `"run_child"` として bridge 側で判定されています。
   * ここを typed `TaskKindSummary` に寄せます。ただし host I/O を core に移さない。

4. NativeTaskBridge に phase timing を追加する。

   * `scheduler_submit_elapsed_ns`
   * `scheduler_dispatch_elapsed_ns`
   * `host_complete_elapsed_ns`
   * `event_build_elapsed_ns`
   * `scheduler_complete_elapsed_ns`
   * `completion_normalize_elapsed_ns`
   * `virtual_path_normalize_elapsed_ns`
   * これらは CLI/adapter 側のみ。

5. in-flight pressure を batch ごとに出す。

   * scheduler stats は `in_flight` と `max_in_flight` を持ちます。
   * 追加:

     * `pending_before_dispatch`
     * `pending_after_dispatch`
     * `in_flight_before_submit`
     * `in_flight_after_submit`
     * `in_flight_after_complete`
     * `max_pending`
     * `max_join_waiters`

### 5. Tests or benchmarks to add/update

* `just bench-thread`
* `just bench-system`
* Native bridge unit tests:

  * marker-only tasks only。
  * system-info tasks only。
  * file read tasks only。
  * mixed marker + system-info。
  * joined same-key tasks。
* JSON regression:

  * no absolute paths。
  * virtual path remains normalized; bridge already rejects absolute/root/parent/current components.

### 6. Expected risk and expected performance impact

* Risk: **低**。まず counters/timing 追加のみなら semantics は変わりません。
* Expected impact: **低 immediate / 高 diagnostic value**。直接 throughput は変わりませんが、`001_thread_scheduling` と `004_system_info_threads` の差を「scheduler sort」「marker-only」「real host work」「normalization」へ分解できます。

---

## P3-14. host completion normalization cost を joined-event 増幅と sort cost に分ける

### 1. Finding title

**completion normalization を “sort が起きた” だけでなく “なぜ増えたか” まで観測する。**

### 2. Affected crates/modules

* `arcweft-runtime-scheduler/src/lib.rs`
* `arcweft-core/src/task.rs`
* `arcweft-cli/src/native_task.rs`

### 3. Why it is likely a bottleneck

scheduler `complete` は events を `Vec` に collect し、normalize し、joined waiter の completion events を追加した場合は再度 normalize します。 `normalize_completion_events` は events が normalized でなければ sort し、sort count/items を記録します。 core 側の `task_events_are_normalized` も windows で compare し、必要なら sort します。

現在の counters は sort count/items までは分かりますが、joined event 増幅、normalize pass count、already-normalized check cost、host completion event build cost が分かれません。

### 4. Concrete implementation plan

1. scheduler count を増やす。

   * `completion_normalization_passes`
   * `completion_normalization_checks`
   * `completion_events_in`
   * `completion_events_joined`
   * `completion_events_out`
   * `completion_sort_skipped_items`
   * `completion_sort_performed_items`

2. joined waiter の増幅を明示する。

   * 既に `joined_completed` はあります。
   * これに加えて `joined_completion_events_emitted` を出す。
   * `complete_joined_waiters` は waiter ごとに event を clone して作っています。

3. Native bridge で completion event construction を測る。

   * `complete_dispatched_tasks` は parallel/serial で tasks を complete し、items を collect します。
   * その後 `TaskCompletion` を `TaskEvent` に変換します。
   * この2つを別 timing にします。

4. Sort を避けられる入力順序を host bridge が作れるか検証する。

   * host completion result を task id / sequence の normalized order で返せれば scheduler sort が減ります。
   * ただし replay-stable ordering が最優先。semantics を変えず、sort skipped counter を見ながら調整する。

### 5. Tests or benchmarks to add/update

* scheduler unit tests:

  * already normalized completion。
  * reversed completion。
  * joined completion あり。
  * joined completion で2回目 normalize が必要な case。
* Bench:

  * `001_thread_scheduling.arcw`
  * `004_system_info_threads.arcw`
* Before/after:

  * `completion_events_in/out`
  * `joined_completion_events_emitted`
  * `completion_normalization_passes`
  * `completion_sorts`
  * `completion_sort_items`
  * `scheduler_complete_elapsed_ns`

### 6. Expected risk and expected performance impact

* Risk: **低**。counter 追加中心です。
* Expected impact: **低〜中**。sort 回避ができれば thread-heavy fixture で効きますが、まずは原因分解が主目的です。

---

## 推奨 commit 順

1. **観測 commit**

   * parser: `cst_lex_passes`, `punctuation_scans`, `owned_line_bytes`, `block_owned_bytes`
   * runtime-plan: `rewrite_expr_visits`, `local_use_scan_visits`
   * pure accelerator: `flatten_materializations`, `flatten_bytes_copied`, cold/warm compile stats
   * scheduler/native bridge: phase/class counters

2. **P0-1: punctuation summary**

   * helper re-lex を減らす。
   * semantics regression を厚くする。

3. **P0-2: line/block range 化**

   * `CstLine.text: String` を range/slice 化。
   * block extraction の concat を遅延。

4. **P0-3: numeric bracket sequence summary**

   * parser allocation を削る。
   * HIR/sema/runtime-plan へ summary を保持。

5. **P1-5/P1-6: runtime-plan single pass 化**

   * pure helper map を先に作る。
   * lowering 時点で pure call を作る。
   * map/sum suffix scan を use-count pass へ。

6. **P1-7/P1-8: typecheck/borrow fixed cost 削減**

   * duplicate expected judgment を解消。
   * borrow snapshot delta 化。

7. **P2: auto acceleration policy**

   * cold/warm を分離してから lazy JIT promotion。
   * flat batch/typed sequence を優先。

8. **P3: scheduler/host bridge 分解**

   * direct throughput より先に原因分解。
   * `bench-thread` / `bench-system` の説明力を上げる。

---

## 最終的に見るべき before/after 指標

* Parser:

  * `parse elapsed_ns`
  * `cst_lex_passes`
  * `punctuation_scan_bytes`
  * `owned_line_bytes`
  * `block_owned_bytes`
  * `expr_tokens`
  * `literal_owned_bytes`
  * `numeric_seq_summary_count`

* Typecheck/borrow:

  * `typecheck elapsed_ns`
  * `expressions`
  * `judgments`
  * `expected_judgments`
  * `type_clone_count`
  * `compatibility_checks`
  * `borrow_state_snapshots`
  * `borrow_state_cloned_bindings`
  * `borrow_state_delta_entries`

* Runtime-plan:

  * `runtime plan lowering elapsed_ns`
  * `runtime_expr_lower_visits`
  * `pure_rewrite_visits`
  * `local_use_scan_visits`
  * `map_sum_fusions`
  * `sequence_let_inlines`

* VM/JIT/AOT:

  * `compile_elapsed_ns`
  * `cold_start_elapsed_ns`
  * `steady_elapsed_ns`
  * `cache_hits/misses`
  * `jit/aot/vm_calls`
  * `batch_calls/items`
  * `arg_bytes_copied/borrowed`
  * `result_bytes_copied`
  * `flatten_materializations`
  * `thread_pool_jobs`

* Scheduler/native bridge:

  * `scheduler_submit_elapsed_ns`
  * `scheduler_dispatch_elapsed_ns`
  * `host_complete_elapsed_ns`
  * `event_build_elapsed_ns`
  * `scheduler_complete_elapsed_ns`
  * `dispatch_sorts/items`
  * `completion_sorts/items`
  * `marker_tasks`
  * `host_io_tasks`
  * `system_info_tasks`
  * `max_in_flight`
  * `max_pending`
  * `parallel_workers`

上記はすべて、互換 shim、deprecated alias、removed whitespace DSL support、unstable Rust、`unsafe` なしで進められます。最も大きい改善余地は parser の repeated lex/scan と owned string 化なので、まず P0-1/P0-2/P0-3 を path-free bench で検証するのが最短です。
