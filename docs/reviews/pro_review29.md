# 仮にanonymousでもっと詳しいものをするなら、new type patternを使わせる方向に誘導しましょう。それ以上は普通のenumで使うようにしましょう

はい、この指摘は正しいです。前回の

```arcw
type X = Text(String) | Binary(Bytes)
```

は「匿名」と言いながら branch 名を要求しているので、実質的には enum row の別表記です。tuple が

```arcw
(String, Bytes)
```

だけで済むのに、直和だけ

```arcw
Text(String) | Binary(Bytes)
```

と書かせるなら、確かに

```arcw
enum X {
    Text(String),
    Binary(Bytes),
}
```

でよくなります。

なので設計を変えるべきです。

## 修正版の結論

anonymous sum は、**variant 名ではなく型そのもので分岐する `A | B`** にするのがよいです。

```arcw
fn read_payload(path: VirtualPath) -> String | Bytes {
    if is_text(path) {
        read_text(path)
    } else {
        read_bytes(path)
    }
}
```

値を作るときに `.Text(...)` や `.Binary(...)` は不要です。
普通に `String` または `Bytes` を返せば、期待型 `String | Bytes` に注入されます。

取り出すときは、既存の typed pattern を使います。

```arcw
let payload = read_payload(path)

match payload {
    text: String => render_text(text)
    bytes: Bytes => render_bytes(bytes)
}
```

これなら tuple に対する匿名性とかなり対応します。

```arcw
// anonymous product
(String, Bytes)

// anonymous sum
String | Bytes
```

この方向なら、**わざわざ別で enum を定義しない理由**があります。

## 現状仕様との整合

arcweft には既に nominal enum があり、`Route`, `BadEndReason` のような enum と `.Opening`, `.AliceIntro`, `.BadEnd` の pattern が明記されています。match は exhaustive check される設計です。 また `Option` / `Result` も組み込み ADT として扱われています。

したがって、役割分担はこうします。

```text
enum:
  branch 名が意味を持つ nominal sum

A | B:
  型だけで区別できる anonymous sum
```

`#` は使いません。grammar では `#` は `#[...]` attribute opener としてだけ現れ、comment や entity-ref sigil ではないと整理されています。

`.` も anonymous sum には使いません。`.` は既に enum variant pattern / contextual variant selection のためにあります。grammar 上も `VariantPattern := ('.' Ident | TypePath '::' Ident) ...` です。 つまり `.Ok`, `.Err`, `.ChoiceSelected` は enum の世界に残す。anonymous sum は型で分岐するので `.Text` のような variant 名を持たない。

## 「型だけで分岐する」基本仕様

型構文に top-level `|` を入れます。

```arcw
TypeChoice := Type ('|' Type)+
```

例:

```arcw
String | Bytes
IoError | ParseError
Ref<Asset> | Ref<Character>
Vec<String> | Seq<String>
```

現在の `TypeRef` は `Never`, `ConstInt`, `Path`, `Generic`, `Ref`, `Slice` などで、まだ choice / union 相当の variant はありません。 ここに足すなら、

```rust
pub enum TypeRef {
    ...
    Choice(Vec<TypeRef>),
}
```

semantic type 側では、

```rust
TypeKind::Choice(Vec<TypeKind>)
```

または正規化済み集合として、

```rust
TypeKind::Choice(BTreeSet<TypeKind>)
```

を持たせるのがよいです。

## 値構築

anonymous sum 専用 constructor は作りません。

```arcw
let x: String | Bytes = "hello"
let y: String | Bytes = bytes
```

期待型がある場合、式の型が alternatives のちょうど 1 つに合えば注入します。

```arcw
fn f() -> String | Bytes {
    if cond {
        "hello"
    } else {
        load_bytes()
    }
}
```

この `if` の join 型は `String | Bytes` です。

期待型がない場合は普通の型のままです。

```arcw
let x = "hello"
// x: String
// x: String | Bytes ではない
```

これは重要です。勝手に union にしない。

## pattern matching

既存の pattern system を使います。arcweft の pattern language は `match`, `if let`, `while let`, `let ... else`, destructuring `let`, function parameter で共有される設計です。 さらに syntax parser には `name: Type` 形式の typed pattern が既にあります。

なので anonymous sum の elimination はこうです。

```arcw
match value {
    s: String => use_string(s)
    b: Bytes => use_bytes(b)
}
```

`String | Bytes | Duration` に対して arm が足りなければ exhaustive check で落とします。

```arcw
match value {
    s: String => use_string(s)
    b: Bytes => use_bytes(b)
}
```

診断:

```text
error: non-exhaustive match on `String | Bytes | Duration`

missing alternative:
  Duration

help:
  d: Duration => ...
```

## enum を使うべきケース

anonymous sum は型だけで区別します。だから、**型だけでは意味が区別できない場合は enum** です。

これは anonymous sum に向いています。

```arcw
String | Bytes
IoError | ParseError
Image | AudioClip | VideoClip
```

これは enum にすべきです。

```arcw
Name(String) | Email(String)
Timeout(Duration) | Delay(Duration)
Left(String) | Right(String)
```

anonymous sum ではこう書けません。

```arcw
String | String
```

これは意味がありません。型としては `String` と同じです。

診断はこうします。

```text
error: duplicate alternative `String` in anonymous sum

help: if the two branches have different meanings, use a nominal enum:
  enum ContactField {
      Name(String),
      Email(String),
  }

help: or introduce distinct newtypes if the distinction is type-level:
  type Name = String where ...
  type Email = String where ...
```

この制約は避けられません。tuple は位置で `String, String` を区別できますが、sum は「どちらか一方」なので、同じ型が 2 回あると取り出し時に区別できません。

## エラー型での使い方

anonymous sum が一番効くのは、`Result` や `Need` の error parameter です。

```arcw
fn load_config(path: VirtualPath) -> Result<Config, FsError | ParseError> {
    let text = read_text(path)?
    parse_config(text)?
}
```

`Result` / `Option` の `?` は `TryLike` に支えられている設計です。 ここに anonymous sum を入れるなら、`?` は residual を widening します。

```arcw
read_text(path)?      // FsError を FsError | ParseError に注入
parse_config(text)?   // ParseError を FsError | ParseError に注入
```

これは enum を定義しなくても済むので、かなり tuple 的です。

従来:

```arcw
enum LoadConfigError {
    Fs(FsError),
    Parse(ParseError),
}

fn load_config(path: VirtualPath) -> Result<Config, LoadConfigError>
```

anonymous sum:

```arcw
fn load_config(path: VirtualPath) -> Result<Config, FsError | ParseError>
```

branch 名が `Fs` / `Parse` 以上の意味を持たないなら、後者で十分です。

## variadic との整合

ご提示の 607 時点メモでは variadic はまだ未実装という整理でしたが、今見えている `main` では少なくとも grammar と syntax crate には rest parameter が入っています。grammar では `param: ...T` が positional rest parameter で、最後の group の最後に高々 1 個、body では `Vec<T>` として見える、と書かれています。 syntax 側にも `FnParamKind::{Fixed, Rest}` があり、`name: ...T` が rest parameter として表現されています。

anonymous sum はこの `...T` と相性がいいです。

```arcw
fn log(message: String, fields: ...(String | i64 | Duration | Ref<Entity>)) -> Unit {
    for field in fields {
        match field {
            s: String => log_string(s)
            n: i64 => log_int(n)
            d: Duration => log_duration(d)
            e: Ref<Entity> => log_entity(e)
        }
    }
}
```

呼び出し側:

```arcw
log(
    "asset loaded",
    "bg.room",
    3i64,
    120ms,
    @asset.bg.room,
)
```

ここで `fields` は body 内では、

```arcw
Vec<String | i64 | Duration | Ref<Entity>>
```

として見えます。

これなら `LogField` enum を作らなくても異種 rest args を扱えます。

前回案:

```arcw
type LogField =
    Text(String)
  | Count(i32)
  | Duration(Duration)

log("loaded", .Text("bg.room"), .Count(3), .Duration(120ms))
```

修正版:

```arcw
log("loaded", "bg.room", 3i64, 120ms)
```

この差は大きいです。

## dynamic host request との整合

ここも、ご提示の更新後状態に合わせると、anonymous sum は **dynamic host request 基盤を作り直さない** 方向にすべきです。

今の `arcweft-core::task` では、`AwaitTarget` が `HostTaskRequestTemplate` を持ち、その template は `capability`, `operation`, `args` を持ち、各 arg は `RuntimeExpr` です。一方、`TaskSpec` は concrete `HostTaskRequest` を持っています。

runtime-plan 側も、awaited expression から `HostTaskRequestTemplate` を作ります。`Expr::Call` / `Expr::MethodCall` から capability と operation を切り出し、args を template 化しています。 named arg も `HostTaskArgTemplate::named` に落ち、`path.save` / `path.asset` / `path.temp` / `path.export` は `RuntimeExpr::Call { callee: "path.*", ... }` に変換されています。

VM 側では await 開始時に `evaluate_host_task_request` が template args の `RuntimeExpr` を評価してから concrete `HostTaskRequest` に落としています。 例えば `fs.write_text` は評価済み positional arg 0/1 から `path` と `text` を取り出す形です。

したがって anonymous sum は、host request lowering の前段、つまり **sema / HIR / runtime-plan 手前の型付き argument model** に入れるのが正しいです。

例えば将来的にこう書けるようにします。

```arcw
extern capability fs {
    fn write(path: VirtualPath, body: String | Bytes) -> Need<Unit, FsError>
}
```

呼び出し:

```arcw
try await fs.write(path.save("out.txt"), text)
try await fs.write(path.save("out.bin"), bytes)
```

`body` の期待型は `String | Bytes` なので、`text` は `String` branch、`bytes` は `Bytes` branch に注入されます。

host boundary に渡る時点で方法は 2 つあります。

1. capability lowering が branch を見て concrete request を選ぶ。

```text
String branch -> HostTaskRequest::FileWriteText
Bytes branch  -> HostTaskRequest::FileWriteBytes
```

2. custom host request なら `RuntimePayload` に type tag 付き choice として渡す。

MVP では 1 が安全です。既存の `FileWriteText` / `FileWriteBytes` に自然に合います。

## CallArg 分離との整合

今の host request lowering では `Expr::NamedArg` を template arg に変換しています。 ただし今後 `CallArg::{Positional, Named, Spread}` に分離するなら、anonymous sum はそこに素直に乗ります。

rest call の型チェックはこうです。

```arcw
fn f(prefix: String, xs: ...(A | B | C))
```

呼び出し:

```arcw
f("x", a, b, c)
```

型検査では、

```text
prefix: String
xs[0]: A | B | C
xs[1]: A | B | C
xs[2]: A | B | C
```

として trailing args それぞれに expected type を配ります。

spread が入ったら、

```arcw
let xs: Vec<A | B> = [...]
f("x", xs...)
```

は OK。

```arcw
let as_: Vec<A> = [...]
f("x", as_...)
```

も、`A` が `A | B | C` に一意に注入できるので OK にできます。

named rest は別問題です。anonymous sum と混ぜるなら、

```arcw
NamedArgs<String | i64 | Duration>
```

のように「名前」と「値の choice」を分けるのがよいです。

## Trait との統合

ここは 2 段に分けると綺麗です。

### 1. branch-lifted method call

値 `x: A | B` に対して、全 alternatives が同じ method を持つなら、method call を match に展開できます。

```arcw
let s = x.format()
```

これは内部的に、

```arcw
match x {
    a: A => a.format()
    b: B => b.format()
}
```

です。

戻り値が同じなら、その型になります。

```arcw
A.format() -> String
B.format() -> String

(A | B).format() -> String
```

戻り値が違うなら、戻り値側も anonymous sum にできます。

```arcw
A.next() -> Option<Byte>
B.next() -> Option<Frame>

(A | B).next() -> Option<Byte> | Option<Frame>
```

ただし、これは「`A | B` が Trait を実装している」という意味ではありません。あくまで branch-lifted call です。

### 2. union 自体が Trait を満たす条件

`A | B: Trait` と言えるのは、より厳しい条件のときだけです。

```text
すべての branch が Trait を実装している
associated type が一意に決まる
method signature が coherent
Self を複数引数に取る binary operation ではない
```

例えば、

```arcw
trait Format {
    fn format(self) -> String
}
```

なら、

```arcw
A: Format
B: Format
```

があれば、

```arcw
A | B: Format
```

として扱えます。

一方、

```arcw
trait IntoSeq {
    type Item
    fn seq(self) -> Seq<Self::Item>
}
```

で、

```text
A::Item = String
B::Item = i32
```

なら、

```arcw
A | B: IntoSeq
```

は不成立です。`Self::Item` が一意に決まりません。

診断:

```text
error: cannot prove `(A | B): IntoSeq`

associated type differs by alternative:
  A: IntoSeq::Item = String
  B: IntoSeq::Item = i32

help: call the method and keep the branch-lifted result:
  let ys = xs.seq()
  // ys: Seq<String> | Seq<i32>

help: or match explicitly
help: or use a nominal enum and implement IntoSeq manually
```

`Add<Self>` のような binary operator 的 Trait も自動 forwarding しません。

```arcw
x + y
```

で `x: A | B`, `y: A | B` の場合、

```text
A + A
A + B
B + A
B + B
```

の全組み合わせを定義する必要があるため、暗黙展開しない方が安全です。

## ambiguous / 不正ケース

### duplicate alternatives

```arcw
String | String
```

```text
error: duplicate alternative `String`

help: use `String` directly
help: use an enum if the two branches mean different things
```

### alias collapse

```arcw
type Name = String
type Email = String

fn f() -> Name | Email
```

もし `Name` / `Email` が transparent alias なら、これは `String | String` と同じなのでエラーです。

```text
error: alternatives `Name` and `Email` erase to the same type `String`

help: make them nominal/refined newtypes, or use an enum
```

### numeric literal ambiguity

arcweft は unsuffixed numeric literal に default `i32` / `f64` fallback を持たず、annotation や signature から expected type が必要です。 なので、

```arcw
let x: i32 | i64 = 1
```

は曖昧です。

```text
error: numeric literal can match multiple alternatives: i32, i64

help:
  let x: i32 | i64 = 1i32
```

### Trait-based implicit conversion は使わない

```arcw
String | Content
```

に対して、`String: Into<Content>` だからといって `"hello"` を `Content` branch にも注入できる、とはしません。

anonymous sum の注入は **exact type / explicit conversion only** です。

```arcw
let x: String | Content = "hello"
// String branch

let y: String | Content = Content::from("hello")
// Content branch
```

これで ambiguity をかなり潰せます。

## enum への誘導

ユーザーが前回案のようなものを書いた場合、

```arcw
type Payload = Text(String) | Binary(Bytes)
```

これは anonymous sum としては受けません。診断で分岐します。

```text
error: anonymous sum alternatives are types, not variant rows

help: if labels are unnecessary, write:
  type Payload = String | Bytes

help: if labels are part of the data model, use an enum:
  enum Payload {
      Text(String),
      Binary(Bytes),
  }
```

これが一番重要です。

**ラベルが必要なら enum。ラベルが不要なら `A | B`。**

## public ABI / save data では enum 推奨

arcweft は `.arcw` source, Typed IR, bytecode, manifest, schema, save snapshot などを data format として扱う設計です。 そのため、外部 schema、save data、安定 ABI に出る型は enum 推奨にします。

```arcw
pub enum SaveEvent {
    StartGame,
    ChoiceSelected { id: Ref<ChoiceOption> },
}
```

anonymous sum は private helper、local return、error set、variadic argument に向きます。

```arcw
fn parse_atom(token: Token) -> i64 | f64 | String
fn load_config(path: VirtualPath) -> Result<Config, FsError | ParseError>
fn log(message: String, fields: ...(String | i64 | Duration | Ref<Entity>))
```

公開 API で anonymous sum が出たら lint で軽く警告します。

```text
warning: public function exposes anonymous sum `Image | AudioClip | VideoClip`

help: public ABI and save data are more stable with a nominal enum
```

ただし、完全禁止にはしない方がよいです。library-internal public や generated code では有用です。

## 実装順

現在の方針、つまり「まず最新の理念・構造・実装状態を把握する」前提にも合わせると、作業順はこうです。

```text
1. TypeRef::Choice を追加
2. type parser で top-level `|` を split
3. TypeKind::Choice を追加し、flatten / sort / duplicate check
4. types_compatible に choice injection / widening を追加
5. if / match / block final expr の join で choice を生成
6. typed pattern `x: Type` を choice elimination として使う
7. match exhaustiveness を enum と choice で共通化
8. rest parameter の expected element type に choice を流す
9. CallArg 分離時に Positional/Named/Spread それぞれへ expected type を流す
10. RuntimeValue に必要なら Choice tag を追加
11. host request boundary では concrete branch に落とす
12. Trait branch-lifting と Trait satisfaction を別ルールで実装
```

`HostTaskRequestTemplate + RuntimeExpr args + await-start evaluation` は既に良い方向なので、そこは壊さない。anonymous sum は、その前段の型検査と runtime value 表現に足すのが正しいです。

## 最終形

前回案はこうでした。

```arcw
type LogField =
    Text(String)
  | Count(i32)
  | Duration(Duration)

fn log(message: String, fields: ...LogField)

log("loaded", .Text("bg.room"), .Count(3), .Duration(120ms))
```

修正版はこうです。

```arcw
fn log(message: String, fields: ...(String | i64 | Duration | Ref<Entity>)) -> Unit {
    for field in fields {
        match field {
            s: String => log_string(s)
            n: i64 => log_int(n)
            d: Duration => log_duration(d)
            e: Ref<Entity> => log_entity(e)
        }
    }
}

log("loaded", "bg.room", 3i64, 120ms, @asset.bg.room)
```

これなら anonymous sum は本当に anonymous です。

`Text(String) | Binary(Bytes)` のような「名前付き branch」を anonymous sum に入れるのではなく、**branch 名が欲しくなった瞬間に enum を使う**。
`String | Bytes` のように、**既に型名だけで意味が分かる場合だけ anonymous sum を使う**。

この分離が一番 arcweft らしいと思います。
