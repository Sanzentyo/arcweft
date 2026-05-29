According to a document from 2026-05-16, Sanzentyo/arcweft では明示がない限り、まず理念・構造・最新状態を把握する前提なので、その前提に従って整理します。

## 方針の前提

**一般的な可変長引数を導入する方針**で進めます。

つまり、`log.info` や `event.emit` だけの特別扱いではなく、最終的には普通の `fn`、`task fn`、`stream fn`、extern capability function、将来的には method-like API にも一貫して使える言語機能として設計します。

ただし、arcweft は構文・HIR・sema・runtime-plan を分けて進めているので、入れ方は段階的にするのがよさそうです。実装状況ドキュメントでも、言語責務は `arcweft-lang-syntax`、`arcweft-lang-hir`、`arcweft-lang-sema`、`arcweft-runtime-plan` に分離されています。

## 現状把握

### 1. 呼び出し側は、すでに任意個の引数を持てる

式 AST には `Expr::Call { callee, args: Vec<Expr> }` と `Expr::MethodCall { receiver, method, args: Vec<Expr> }` があり、呼び出し引数は個数固定ではなく `Vec<Expr>` で保持されています。

parser 側も `parse_call_args` で `)` までカンマ区切りの引数を読み続ける形なので、`foo(a, b, c, d)` のような呼び出し自体は現状でも自然に表現できます。

したがって、**呼び出し AST の大改造は不要**です。

必要になるのは主に以下です。

```rust
// 既存
Expr::Call {
    callee,
    args: Vec<Expr>,
}
```

ここに「この引数は spread である」という情報をどう足すか、または `Expr::SpreadArg` のようなノードを足すか、という話になります。

### 2. named argument はすでにある

`Expr::NamedArg { name, value }` があるので、`foo(x = 1, y = 2)` のような構文は既に AST 上で表現されています。

これは可変長引数の設計で重要です。一般導入するなら、最低でも次の区別が必要になります。

```arcw
foo(a, b, c)              // positional rest
foo(a, fields...)         // positional spread
foo(x = 1, y = 2)         // named args
foo(..fields)             // named spread を入れるかは別検討
```

ただし named spread まで一気に入れると設計面が重くなるので、最初は **positional rest / positional spread のみ** がよさそうです。

### 3. 宣言側は固定長 parameter model

現状の `FnSignature` は、名前、generic params、`param_groups: Vec<FnParamGroup>`、戻り値、where clauses を持っています。`FnParamGroup` は `params: Vec<FnParam>`、`FnParam` は pattern、type、default を持つ固定 parameter です。

つまり現状の宣言側には、次のような概念がありません。

```arcw
fn f(args: ...T)
fn f(args: T...)
fn f(...args: T)
fn f(args: Vec<T>...)
```

なので、一般導入で最初に触る中心は `arcweft-lang-syntax/src/types.rs` の `FnParam` 周辺になります。

必要そうな拡張は、概念的にはこうです。

```rust
pub struct FnParam {
    doc: Option<DocBlock>,
    pattern: Pattern,
    ty: TypeRef,
    default: Option<Expr>,
    variadic: bool, // 追加候補
}
```

または少し堅くするなら、

```rust
pub enum FnParamKind {
    Fixed,
    Rest,
}
```

のようにします。

### 4. curried parameter groups があるので、rest の位置制約が必要

arcweft の関数 signature は curried parameter group を持てます。既存テストでも `fn bind<'a, T>(state: &'a State)(route: T) -> ArcResult<T> where ...` のような形が確認されています。

そのため、可変長 parameter を一般導入するなら、最初はかなり明確に制限した方がよいです。

初期ルールはこれが安全です。

```text
rest parameter は signature 全体で高々 1 個。
rest parameter は最後の parameter group の最後の parameter のみ。
rest parameter には default value を付けられない。
rest parameter より後に positional parameter は置けない。
```

つまり最初は OK / NG をこう分けます。

```arcw
fn log(message: String, fields: ...LogField) -> Unit      // OK

fn f(xs: ...Int)(y: Int) -> Unit                          // NG
fn f(x: Int = 0, xs: ...Int) -> Unit                      // たぶん OK
fn f(xs: ...Int = []) -> Unit                             // NG
fn f(xs: ...Int, y: Int) -> Unit                          // NG
```

curried group ごとに rest を許す設計も可能ですが、partial application や group application の仕様が重くなるので、初期導入では避けるべきです。

### 5. 型検査は、まだ「宣言 signature と呼び出し引数の完全照合」中心ではない

現在の type checker は、top-level function を `global_functions` に登録するとき主に戻り値型を登録しています。関数 parameter を body 内の local として bind する処理はありますが、呼び出し側で signature の parameter 列と args を完全に照合する形にはまだ見えません。

呼び出しチェック側も、既知関数や well-known runtime method を見つけたら、基本的には各 arg を `check_expr` して戻り値型を返しています。

なので、一般 variadic を入れる前に、または同時に、**関数 signature を sema 側で保持して呼び出し解決に使う**必要があります。

現状のままだと、

```arcw
fn add(x: Int, y: Int) -> Int

add(1)
add(1, 2, 3)
```

のような固定長関数の arity 診断が十分に強くならない可能性があります。variadic は arity の例外なので、まず通常 arity の基準が必要です。

### 6. 既に variadic っぽい special case は存在する

`event.emit` は最初の引数を特別扱いして、それ以降の引数をチェックする流れがあります。

また `panic` / `fail` / `bail`、`ensure`、`assert`、`debug_assert` のような builtin call も、引数列をそれぞれ独自に見ています。

これは重要です。
一般 variadic を入れると、これらの special case の一部は次のように普通の signature へ寄せられます。

```arcw
intrinsic fn panic(message: String, fields: ...DiagnosticField) -> !
intrinsic fn ensure(condition: Bool, message: String = "", fields: ...DiagnosticField) -> Unit
intrinsic fn event.emit(event: Ref<Event>, payload: ...EventField) -> Unit
```

つまり一般導入は、むしろ special case を減らす方向に使えます。

## 現状のギャップ

一般導入に必要な不足点はこのあたりです。

| レイヤー             | 現状                                       | 足すもの                                                        |
| ---------------- | ---------------------------------------- | ----------------------------------------------------------- |
| Syntax / AST     | call args は `Vec<Expr>`。宣言 parameter は固定 | `FnParamKind::Rest` か `variadic: bool`                      |
| Expr             | spread arg の表現がない                        | `Expr::Spread` または `CallArg` 型                              |
| Type syntax      | `TypeRef` に rest 型はない                    | 型ではなく parameter modifier として扱うのがよい                          |
| Parser           | `...` token は専用扱いされていない                  | rest / spread 用 token または CST helper                        |
| HIR              | 要確認だが、おそらく signature を mirror            | HIR param に rest flag                                       |
| Sema             | 関数呼び出しで signature 照合が弱い                  | arity/type matching と rest packing                          |
| Runtime lowering | ordinary call はあるが rest の正規化はない          | rest は HIR/sema 段階で `Vec` 化して runtime へ渡す                   |
| Diagnostics      | まだなし                                     | “rest must be last”, “too few args”, “spread type mismatch” |

## 初期仕様案

一般導入するなら、最初の仕様はこれくらいに絞るのがよさそうです。

### 宣言構文

候補は 2 つあります。

```arcw
fn f(prefix: String, args: ...String) -> Unit
```

または、

```arcw
fn f(prefix: String, args: String...) -> Unit
```

arcweft 的には、`args: ...String` の方が「parameter modifier」として読みやすいです。ただし、`@...suffix` のような relative entity ref 表現が既にあるので、`...` をどこで token として認めるかは慎重にします。実装状況ドキュメントでも `@.suffix`、`@..suffix`、`@...suffix` 系の relative ID が扱われています。

そのため、parser では **型位置の `...T` のみ rest marker** として扱うのが安全です。

```arcw
fn info(message: String, fields: ...LogField) -> Unit
```

body 内では、

```arcw
fields: Vec<LogField>
```

として見える、という lowering が自然です。

### 呼び出し構文

通常呼び出しはそのままです。

```arcw
info("loaded", asset, elapsed)
```

spread は後から入れてもよいですが、一般 variadic なら最終的には必要になります。

候補は、

```arcw
info("loaded", fields...)
```

です。

ただし `...` token を増やすなら lexer/CST の扱いが必要です。今の expression lexer は `..=`、`..`、`.` を個別に扱っているので、`...` を使うなら専用 token を先に追加した方が安全です。

初期実装では spread なしでも始められます。

```arcw
// v1: OK
info("loaded", asset, elapsed)

// v1: まだ未対応
info("loaded", fields...)
```

### 型規則

固定 parameter を先に消費し、残りの positional args を rest parameter に pack します。

```arcw
fn f(a: A, b: B, rest: ...C) -> R
```

呼び出し解決はこうです。

```text
f(x, y)          => rest = []
f(x, y, z1)      => rest = [z1]
f(x, y, z1, z2)  => rest = [z1, z2]
f(x)             => too few args
```

`rest` の body 内型は `Vec<C>` がよいです。

```arcw
fn sum(xs: ...Int) -> Int {
    let total = 0
    for x in xs {
        total = total + x
    }
    total
}
```

### named args との初期ルール

最初は単純にするのがよいです。

```text
rest parameter は positional args のみを受ける。
named args は固定 parameter にだけ対応する。
named rest は v2 以降。
```

つまり、

```arcw
fn f(a: Int, rest: ...Int)

f(1, 2, 3)       // OK
f(a = 1, 2, 3)   // 仕様としては一旦 NG でもよい
f(1, x = 2)      // rest には入れない。固定 parameter がなければ NG
```

arcweft には `Expr::NamedArg` が既にあるため、named rest を後で足す余地は残せます。

## 実装確認チェックリスト

現状把握として、次に見るべき箇所はこの順です。

1. `arcweft-lang-syntax/src/types.rs`
   `FnParam`、`parse_fn_param`、`parse_fn_param_groups` に rest marker を足す中心。

2. `arcweft-lang-syntax/src/expr.rs`
   spread call を入れるなら `Token::DotDotDot` または `Expr::Spread` / `CallArg` が必要。

3. `arcweft-lang-hir/src/model.rs` と lowering
   syntax の `FnParamKind::Rest` を HIR に保持する。body 内では `Vec<T>` local として bind する。

4. `arcweft-lang-sema/src/checker/module.rs`
   `bind_top_level_functions` を戻り値型だけでなく function signature registry に拡張する。

5. `arcweft-lang-sema/src/checker/expr.rs`
   `check_call_expr` で fixed args と rest args を照合する。

6. `arcweft-runtime-plan`
   runtime へ行く前に rest args を `RuntimeValue::Vec` 相当へ正規化するか、call node に packed args として渡す。

7. tests
   parser、sema、runtime-plan の順で fixture を追加する。

## 現時点の結論

現状は、**呼び出し側は可変個数を受ける土台があるが、宣言側・型検査側には一般 variadic の概念がない**、という状態です。

一般導入するなら、最初の設計単位はこれです。

```arcw
fn f(required: A, rest: ...B) -> R
```

最初の制約はこうです。

```text
rest は最後の parameter group の最後だけ。
rest は positional only。
body 内では Vec<B>。
spread call は v1.1 でもよい。
named rest は v2。
```

この方針なら、既存の `args: Vec<Expr>` を活かしつつ、arcweft の構造化方針にも乗せやすいです。
