精査した限り、Arcweft は構文仕様・AST・HIR まではかなり広く整備されています。ただし現時点では「仕様に書かれているが AST/パーサーで十分に構造化されていないもの」「実装はあるが grammar.md 側が追いついていないもの」「手書き line-based parser 由来の曖昧性・非効率」がまだ残っています。特に優先度が高いのは、**関数シグネチャ、式パーサー、`#` コメント/EntityRef、`[]` の index/dialogue 判定、hook/dialogue option の構造化不足**です。

## 1. 足りていない、または仕様と実装がズレている構文

**`DialogueDefaultsDecl` が grammar.md にあるが、AST 側に対応 Item が見当たりません。**
`grammar.md` の `Item` には `DialogueDefaultsDecl` が含まれていますが、`ast.rs` の `Item` enum には `DialogueDefaultsDecl` 相当がなく、代わりに `Attribute`, `Callable`, `Trait`, `Impl`, `Enum`, `Struct`, `EntityDecl`, `ExternMod`, `Source`, `Raw` などが入っています。つまり grammar 側が古い項目を残し、実装済み項目を十分に列挙していない状態です。 

**hook ヘッダーの構造化が不足しています。**
grammar では `when`, `priority`, `once`, `effects` まで hook 構文として定義されていますが、AST の `HookItem` は `id`, `target`, `phase`, `check`, `body`, `body_statements` 程度しか保持していません。現状だと hook の実行条件・優先度・一回限り・副作用宣言が HIR/型検査/formatter で扱いにくいです。 

**dialogue line option の構造化が不足しています。**
grammar の `LineOption` には `id`, `text_key`, `voice`, `window`, `args`, `source_locale`, `hooks`, `style` が並んでいます。一方 AST の `LineOptions` は `id`, `text_key`, `source_locale` だけを構造化し、`SpeakerLine` / `ContentCall` は `args: Option<String>` として残しています。音声同期、window 解決、hook 抽出、style lint、localization manifest 生成まで考えると、`voice`, `window`, `hooks`, `style`, `args` は raw string ではなく typed option にした方がよいです。 

**関数シグネチャの parser が curried parameter を正しく扱えていない疑いがあります。**
テストには `pub fn add_affection(character: Ref<Character>, delta: i32)(state: GameState) -> GameState` という curried 形の例がありますが、`types.rs` の `parse_fn_signature` は `fn name...` の後に一つの `(...)` を読んだあと、直後の `->` だけを return type として見ています。つまり二つ目の `(state: GameState)` 以降を構造化せず、場合によっては trailing syntax を無視して成功する可能性があります。これは「受理しているが意味が欠落する」タイプの危険なズレです。 

**関数ジェネリクスが lifetime 以外に不足しています。**
`parse_fn_signature` は `fn name<'a>(...)` の lifetime list は扱いますが、型パラメータ `fn map<T, U>(...)` や where clause を構造化していません。Arcweft の型構文は `List<T>`, `Result<T, E>` を明示しており、trait/impl や GAT も実装状況に出ているので、関数側にも type/generic parameter と where clause の AST が必要です。  

**expression grammar の演算子集合と precedence が未完成です。**
`Expr` AST には binary/unary/pipe/try/await/call/index/record などがありますが、実装されている binary op は主に `=>`, `||`, `&&`, `in`, 比較、`+`, `-` です。掛け算、割り算、剰余、unary minus、cast/as、coalesce 系、代入系などは未定義です。必要ないなら明確に除外、必要なら precedence table に追加した方がよいです。

**`NeverType := !` は grammar にあるが、型 AST では専用表現がありません。**
`TypeRef` は `Path`, `Generic`, `Ref`, `Slice` だけなので、`!` は現状 `Path("!")` として扱われる可能性があります。bottom type を diagnostics や型統合で使うなら `TypeRef::Never` を追加した方が後段が楽です。 

## 2. 曖昧、または誤認識しやすい構文

**`#` が comment と EntityRef の両方に見えるのは危険です。**
grammar は `EntityRef := '#' Ident...` と `Comment := '#' TextToEndOfLine` を同時に置いています。Arcweft では `#flow.opening`, `#choice...`, `#asset...` が頻出するので、`#` comment は衝突しやすいです。コメントを `//` / `///` に寄せるか、`#` comment は「`#` の直後が whitespace のときだけ」など字句規則を厳密化した方がよいです。

**`[]` の index と dialogue call の判定が heuristic です。**
`expr.rs` は `target[content]` を見たとき、`content` が `looks_like_index_expr` なら `Index`、そうでなければ `DialogueCall` にします。これだと `arr[i + 1]` のような普通の index 式が dialogue call 側に誤分類される可能性があります。dialogue content は本来「dialogue callee として成立するものに続く `[...]`」だけに限定し、式パーサー一般では `[]` を常に index/list として扱う方が安全です。 

**`a.b` が path なのか field access なのか曖昧です。**
`expr.rs` の `split_field_access` は単純な `state.affection` を field としては扱わず、`Path("state.affection")` になりやすい設計です。一方で method call や placeholder field access は別扱いです。これは type-directed resolution で解決する方針ならよいですが、formatter・symbol collection・診断で「field access」と「qualified symbol」が区別しにくくなります。

**`=>` が複数文脈で使われています。**
`=>` は expression parser では `BinaryOp::Implies`、choice arm では value output、match arm では arm separator として使われます。文脈で分かれるとはいえ、line-based parser が文字列 split で扱うと誤爆しやすいので、tokenize 後に文脈別 parser へ渡す方式にした方が安全です。 

**`await expr? with:` の禁止は仕様化されていますが、式 parser だけでは検出しきれません。**
await docs は `await expr? with:` を曖昧として reject し、`try await expr with:` / `await? expr with:` / `(await expr with: ...)?` を推奨しています。ただし `parse_expr` 自体は `await ` を prefix として読み、その中の `expr?` も try expression として読めるため、await-with parser 側で確実に禁止診断を出す必要があります。 

**speaker colon と label/field/typed syntax の境界は今後注意が必要です。**
現状 `Label` は `'label:` なので `alice:` と衝突しにくいですが、将来的に object literal や YAML-like block を増やすと `name:` が dialogue line と取り違えられます。flow body の先頭位置で `Ident CallArgs? ':'` を speaker line とするなら、speaker symbol として解決できない場合は早めに診断する方がよいです。 

## 3. 現時点の parser の非効率・脆弱な部分

**line-based parser が source 全体を `SourceLine { text: String, start, end }` に分割し、さらに body を String として切り出しています。**
`Parser` は `source: String`, `lines: Vec<SourceLine>`, `index`, `errors`, `pending_flow_items` を持ち、`parse()` は line を clone しながら進みます。さらに `take_flow_block` / `take_brace_block` 系で header/body を String 化します。大きな `.awft` では、元ソースを保持しつつ行ごとの String と body substring を重ねて持つため、メモリとコピーが増えます。`&str` span + byte cursor + arena/green tree 方式に寄せると改善します。

**`take_flow_block` の `{` 検出が文字列ベースで脆いです。**
現在は「現在行に `{` がある」「ただし `effects` で始まる行は除外」のような判定が入っています。これは header の文字列、contract expression、将来の where clause、default expression、コメント、文字列リテラル内の `{` で誤判定する可能性があります。lexer で `{` token と string/comment を分けてから block を読むべきです。

**expression parser が演算子グループごとに何度も top-level scan します。**
`parse_binary` は `split_top_level` を演算子ごとに繰り返し、さらに再帰します。式が長くなると同じ文字列を何度も走査します。Pratt parser または precedence climbing parser にして、token stream を一回読みで処理した方が速く、precedence/associativity も明示できます。

**binary expression の再帰が不十分で、混合式や連鎖式が raw になりやすいです。**
比較演算の左右は `parse_postfix` に落ちており、`a < b + c` の右辺 `b + c` が binary として再帰処理されません。`a + b + c` も最初の `+` で split された後、右辺が `parse_postfix("b + c")` になりやすいです。これは性能以前に構文木の正確性の問題なので、式 parser は最優先で置き換える価値があります。

**文字列・escape・nesting の扱いが各 helper に分散し、不一致があります。**
`split_top_level`, `split_args`, `find_top_level_char`, `find_last_top_level_dot`, `take_balanced_bracket` などがそれぞれ独自に depth や string 状態を持っています。多くは `\"` escape や raw string を考慮していません。共通 lexer/scanner にまとめないと、同じ構文が場所によって読めたり読めなかったりします。 

**record field parsing が nested comma に弱いです。**
`parse_record_fields` は行と comma で単純分割しており、`{ a = f(x, y), b = [1, 2] }` のような nested comma を壊しやすいです。ここも `split_top_level` を使うか、lexer token stream 上で parse した方がよいです。

**`Raw(String)` による lossy recovery が多く、後段まで問題が遅延します。**
AST/HIR には `Raw` が複数残っています。実装状況ドキュメントでも、`validate_typecheck_ready` が raw expression fragments を拒否する契約になっています。回復用に Raw を持つのは良いですが、formatter/LSP/typechecker に渡す前に「どの構文が未対応で raw になったか」を syntax diagnostic として確実に出す必要があります。  

## 4. 優先度順の改善案

**P0: expression parser を Pratt parser に置き換える。**
`[]`, `?`, `await`, `try`, `|>`, `=>`, comparison, arithmetic, call, method, field, index, record, block expression を token stream で統一的に処理するのが最も効果的です。これで曖昧性と非効率の多くが同時に消えます。

**P0: `#` comment rule を変更または厳密化する。**
EntityRef が言語の中心なので、`#` comment は危険です。`//` / `///` を comment にし、`#` は EntityRef 専用にするのが一番安全です。

**P1: function signature parser を強化する。**
複数 parameter group、type parameters、where clause、effect/contract tail、trailing garbage rejection を入れるべきです。特に「受理したが後半を捨てる」は避けたいです。

**P1: hook と dialogue option を fully structured にする。**
`HookWhen`, `priority`, `once`, `effects`、および `voice/window/hooks/style/args` を AST/HIR に落とすと、formatter、localization、voice manifest、LSP diagnostics が作りやすくなります。

**P1: grammar.md を実装に合わせて更新する。**
`DialogueDefaultsDecl` の扱いを決め、`source`, `extern mod`, `trait`, `impl`, `enum`, `struct`, `entity declarations`, `task/dialogue/stream fn` などを grammar summary に反映した方がよいです。現状の実装状況ドキュメントはかなり詳しいので、それを canonical grammar に移す形で整理できます。

**P2: line-based parser から span/token parser へ段階移行する。**
最初から full parser generator にする必要はありませんが、少なくとも lexer、token cursor、span-preserving AST、共通 top-level splitter は導入した方がよいです。現在の `SourceLine` + String body 抽出方式は、今後 formatter/LSP/diagnostics を増やすほど苦しくなります。

全体として、Arcweft の構文設計自体はかなり良い方向です。次に詰めるべきは「仕様の追加」よりも、**字句規則と expression parser を固めて、既にある広い構文を lossless に読めるようにすること**だと思います。
