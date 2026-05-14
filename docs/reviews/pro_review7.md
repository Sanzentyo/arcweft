結論から言うと、**Craneliftもrowanも「入れる前提」の設計ドキュメントにはなっています**。ただし実装の進み方は少し違います。

**Cranelift側は問題少なめ**です。今のコードはまだVM/Typed IR/JITに到達しておらず、Craneliftを無理に先取りしていません。むしろdocsでは「VMが正」「Cranelift JITはnative-onlyの最適化backend」「pure / deterministicな関数限定」とかなり健全な方針になっています。
一方で、`arcweft-core` のCargo featureには `jit-cranelift = []` がすでに予約されていますが、実依存としての `cranelift-*` はまだ入っていません。 これは「将来予定地」として自然です。

**rowan側は要注意**です。docsでは明確に `arcweft-lang-syntax` は `rowan`互換のlossless CST、コメント・空白・ID・リンク・reference保持、recovery parser優先、実装順も `TokenKind → GreenNodeBuilder → CST node accessor → source map → parser tests` と書かれています。
でも現状の実装は、`rowan`導入前提のCST実装というより、**かなり大きな自前AST/手書きparserが先に育っている**状態です。

現状をざっくり分類するとこうです。

| 観点                       | 現状                                                                               |
| ------------------------ | -------------------------------------------------------------------------------- |
| Cranelift導入前提か           | **はい**。docs・feature flagに痕跡あり。ただし実装はまだ未着手で妥当                                     |
| rowan導入前提か               | **docs上ははい**                                                                     |
| rowan実装がもう入っているか         | **いいえ**。`arcweft-lang-syntax` は今のところ `arcweft-source` だけに依存していて、`rowan`依存はありません。 |
| rowanで担うべきものを自前実装し始めているか | **はい、かなりあります**                                                                   |
| すぐ直すべき優先度                | **rowan/CST化を先に固めた方がいい**                                                         |

特にrowan観点で気になったのは、`SyntaxTree` が `source: String`, `module`, `uses`, `items`, `wiki_links` を持つ独自ASTになっていて、`TextRange` も独自定義されています。 これは悪いわけではありませんが、docsが求めている「lossless CSTを正にして、AST accessorをその上に載せる」構成とは逆向きです。

さらに `parse_source` は `Result<SyntaxTree, Vec<ParseError>>` で、内部は `SourceLine` を作って行単位で進める `Parser` になっています。 これはMVPとしては動きますが、rowanを入れたいなら危険です。壊れたファイルでもCSTを返したい、LSPで編集中のコードを扱いたい、formatterで空白・コメントを保持したい、という目的に対して、`Err`でtreeごと失う設計は相性が悪いです。

この点は、既存のレビュー文書でもほぼ同じ指摘がされています。`parse_source(...) -> Result<SyntaxTree, Vec<ParseError>>` と `SourceLine` ベースparserはMVPとしては十分だが、hot reload / LSP / incremental buildでは弱いので、`ParsedSource { green: GreenNode, ast: SyntaxTree, errors, file_hash, line_index }` のように、壊れたCST/ASTも返すべき、と書かれています。

「rowanでいけるのに、わざわざ自前でやっている」候補は主にこれです。

| 現在の実装                                                 | rowan導入後の自然な形                                               |
| ----------------------------------------------------- | ----------------------------------------------------------- |
| 独自 `TextRange { start, end }`                         | `rowan::TextRange` / `text_size`系に寄せる                       |
| `SyntaxTree` が直接 `Vec<Item>` を持つ                      | `GreenNode` + `SyntaxNode` を正にして、typed AST accessorを薄く載せる   |
| `parse_source -> Result<SyntaxTree, Vec<ParseError>>` | `ParsedSource { syntax, ast, errors }` のように常にtreeを返す        |
| `RawItem`, `Raw(String)` による損失的な退避                    | `ERROR`, `RAW`, `MISSING`, `TOMBSTONE`的なCST node/tokenとして保持 |
| 行単位の `SourceLine` parser                              | token stream / event parser / GreenNodeBuilderへ移行           |
| `split_top_level`, `find_matching_*` の重複              | token/CSTベースで共通化                                            |

`expr.rs` もかなり手作りです。独自 `Token`、`Lexer`、Pratt parser風の `ExprParser` を持っています。 これは「rowanが完全に置き換える」対象ではありません。rowanはパーサそのものではないので、式parserは必要です。ただし、今のように式だけ別lexerで文字列から再parseするより、共通tokenizerからCSTを作り、typed AST accessor / loweringで `Expr` にする方が、formatter・LSP・diagnosticsには強いです。

`types.rs` と `pattern.rs` も同じ傾向です。型構文は `parse_type_ref`, `parse_fn_signature`, `split_top_level`, `find_matching_angle` などを自前で持ち、patternも文字列分割ベースで `parse_pattern` しています。  ここも、意味構造としての `TypeRef` / `Pattern` 自体は残してよいですが、**入力ソースを毎回文字列分割で再解釈する層** はrowan導入時に二重実装になりやすいです。

一方で、Craneliftについては今の進め方でよさそうです。docsのロードマップではPhase 0がlossless CST parser、HIR/typecheck、Coreで、Cranelift JITはPhase 5のExtension/JITに置かれています。 機能マトリクスでもCranelift JITはAdvanced/native feature扱いです。 なので、今Craneliftを入れず、まずVM/Typed IRを作る方針は正しいです。

ただしCranelift周りで1点だけ整理した方がいいです。`arcweft-core` には `jit-cranelift` featureがありますが、docsの疑似コードでは `native-jit` feature名が使われています。  またcrate mapでは将来 `arcweft-lang-jit` が想定されています。 名前と配置は早めに統一した方がいいです。おすすめは、`arcweft-core` にCranelift依存を置かず、`arcweft-lang-jit` または `arcweft-jit-cranelift` に閉じ込める構成です。これはdocsの「coreはCranelift runtimeに直接依存しない」という依存ルールとも合っています。

## 判断

**Craneliftについて:**
開発はCranelift導入前提で進んでいます。ただし現状は「予定地を置いている」段階で、まだ入れるタイミングではありません。VM / Typed IR / purity check / type layout check ができてからでよいです。今の方針は概ね妥当です。

**rowanについて:**
開発方針としてはrowan導入前提です。しかし実装はすでに、rowanで担うべきCST・range・error-tolerant parse・source保持の領域を、自前AST/parserとして育て始めています。これは後でrowanへ移すときに手戻りが出やすいです。

## 今やるなら

優先順位はこうだと思います。

1. `arcweft-lang-syntax` に `rowan` を入れる前提で、`SyntaxKind`, `TokenKind`, `GreenNodeBuilder`, `SyntaxNode` wrapperを先に作る。
2. `parse_source` の戻り値を `Result<SyntaxTree, Vec<ParseError>>` から、常にtreeとerrorsを返す `ParsedSource` 系へ変える。
3. 既存の `Expr`, `Pattern`, `TypeRef`, `Item` は消さず、rowan CST上のtyped AST / lowering結果として使う。
4. 今の行単位parserは短期MVPとして残すなら、`legacy_parser` 的に隔離し、これ以上 `split_top_level` 系を増やさない。
5. Craneliftは今は触らず、feature名だけ `jit-cranelift` / `native-jit` / `arcweft-lang-jit` のどれを正にするか決める。

一言でいうと、**Craneliftは「予定通り後回し」でOK、rowanは「今すぐ土台を入れないと自前parserが育ちすぎる」状態**です。
