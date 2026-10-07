# 関数、パイプ、カリー化

## 関数は data-last が標準

```arcw
fn map<A, B>(mapping: A -> B)(self: Vec<A>) -> Vec<B>
fn filter<A>(predicate: A -> bool)(self: Vec<A>) -> Vec<A>
fn fold<A, B>(init: B, step: (B, A) -> B)(self: Vec<A>) -> B
```

Function type は closed effect row を suffix として持てる。これは関数値の
生成ではなく、その関数値を apply した時に発生しうる effect を表す。

```arcw
let load_text: String -> String effects { fs.read } = read_text
let projector: (String -> String) effects { fs.read } = read_text
```

これにより、以下が自然になる。

```arcw
choices
    .filter(_.enabled)
    .map(_.label)
```

desugar:

```arcw
map(_.label)(filter(_.enabled)(choices))
```

## パイプ

```arcw
choices
    |> filter(_.enabled)
    |> map(_.label)
```

`rhs` に `^` がない場合は、`rhs` をまず関数値として評価し、その関数値へ
左辺を 1 引数の次 call group として適用する。

```arcw
x |> f(a)       // f(a)(x)
x |> f(a, b)    // f(a, b)(x)
```

この規則は既存の call group へ左辺を append する書き換えではない。
したがって `f(a)(b)` と `f(a, b)` の区別はパイプでも保たれる。左辺は
`rhs` より先に一度だけ評価され、その値を次の apply が読む。

## プレースホルダ付きパイプ

```arcw
raw_score |> clamp(0, ^, 100)
```

概念上の展開:

```arcw
let <pipe-left> = raw_score
clamp(0, <pipe-left>, 100)
```

`<pipe-left>` はソースから記述できない内部 binding である。RHS 内に `^` が
複数あればすべて同じ値を読む。左辺式を `^` の個数だけ複製してはならない。
明示的な closure は callable boundary なので、closure の内側の `^` は外側の
pipe-left binding を参照しない。必要な値は先に明示的な binding として保存して
closure から参照する。入れ子のパイプでは、内側パイプの RHS にある `^` は
内側の値を参照し、内側パイプの LHS にある `^` は外側の RHS scope を参照する。

`_` と `^` は役割が違う。

```arcw
choices.filter(_.score >= 0)
threshold |> clamp(0, ^, 100)
```

- `_`: `choices` の各要素。
- `^`: pipe 左辺の `threshold`。

概念上の展開:

```arcw
let <pipe-left> = threshold
clamp(0, <pipe-left>, 100)
```

closure 内で pipe-left の値を使う場合は、明示的に保存した binding を参照する。

```arcw
let saved_threshold = threshold
threshold |> choices.filter(|choice| choice.score >= saved_threshold)
```

## Explicit extension receiver

An ordinary `fn` may opt into dot-call syntax by declaring exactly one typed
`self` parameter. This is an explicit receiver coordinate, not a type-matched
search over free callables:

```arcw
fn normalize(self: String, locale: Locale) -> String {
    ...
}

let direct = normalize(text, locale)
let dotted = text.normalize(locale)
```

Both calls select the same declaration, body, effect row, generic
instantiation, and callable identity. An extension declaration is still an
ordinary function and remains available by its ordinary path. It does not
create a wrapper method or a second runtime function.

The receiver has one of two canonical positions:

1. the first parameter of the first call group, for receiver-first APIs; or
2. the sole parameter of the second and final call group, for curried data-last
   APIs.

```arcw
fn clamp(self: i64, min: i64, max: i64) -> i64
fn map<A, B>(mapping: A -> B)(self: Vec<A>) -> Vec<B>
```

The second declaration supports all three equivalent surfaces:

```arcw
map(project)(values)
values |> map(project)
values.map(project)
```

`self: T`, `self: &T`, and `self: &mut T` respectively declare owned, shared,
and mutable receiver modes. The receiver is required and positional-only in a
free call; it cannot have a default and cannot be a rest parameter. The short
method receiver forms `self`, `&self`, and `&mut self` remain reserved for
trait and inherent method declarations, whose owner already supplies the
receiver type.

`receiver.method(args...)` considers inherent methods, visible trait methods,
and visible functions that explicitly declare this receiver coordinate. It
never scans an ordinary free callable merely because its name and one argument
type happen to fit. If more than one distinct applicable declaration remains
across those families, resolution reports an ambiguity instead of silently
preferring one family. A qualified free call remains the explicit
disambiguation for an extension function.

The pipe is independent of extension lookup. With no `^`, it evaluates its RHS
as a function value and applies the left value as the next one-argument call
group. With `^`, it uses the explicit placeholder coordinate. Neither form
performs a method-name fallback.

## カリー化

```arcw
fn has_affection_at_least(character: Ref<Character>, min: i32)(state: GameState) -> bool {
    state.affection.get(character).unwrap_or(0) >= min
}

let alice_ready = has_affection_at_least(@character.alice, 3)
if state |> alice_ready { ... }
```

## 関数値と適用

クロージャーと bare function name は関数値である。評価時にその lexical
environment を決定的な capture binding として保持し、後続の call / apply で
引数 binding より先に復元する。

free input は式の local 読取りだけでなく、record shorthand、nominal field の
base binding、入れ子の closure や deferred callback の生成時 capture を含む。
callable body 内で導入した binding は外側の capture に含めない。record の明示
field と shorthand は記述順に入力 occurrence を持ち、同じ binding の複数の
occurrence は一つの capture packet slot に集約する。shorthand の座標は accepted
field と source ordinal、creation capture の座標は生成 owner と binding origin
に属する。式の arena ID や candidate の探索順は semantic identity に含めない。
shorthand はその記述位置から見える lexical binding を使う。未宣言の名前や
後続の binding を参照する shorthand は型検査で拒否する。

implicit callable の生成では lexical capture を保持し、適用時にはその callable
に属する本体を実行する。関数の返り値や default が callable なら、その値を生成
する。返り値であることを理由に callable 本体を実行してはならない。

```arcw
let add_with_bonus = |score: i64| score + bonus
let next = add_with_bonus(3i64)
```

Closure は返り値型を明示できる。返り値型を明示する場合、body は block
必須である。

```arcw
let is_high =
    |score: i64| -> bool {
        score >= 80i64
    }

let now_text =
    || -> String {
        clock.now().to_string()
    }
```

返り値型なしの軽い closure はそのまま使える。

```arcw
choices.filter(|choice| choice.enabled)
```

Curried closure では call group を flatten しない。`|a, b| -> C { ... }`
と `|a| |b| -> C { ... }` は別の関数型である。

```arcw
let ge =
    |min: i64| |value: i64| -> bool {
        value >= min
    }
```

`_` と `^` に直接 return type annotation は付けない。型を明示したい場合は
binding 側の `let f: A -> B = ...` か、明示 closure を使う。

```arcw
fn add(a: i64)(b: i64) -> i64 { a + b }

let f = add
let add_two = add(2i64)
let seven = add_two(5i64)
```

実装は exact arity の既知 pure function call を最適化された helper call に
落としてよい。ただし、関数が値位置に現れる場合や必要数より少ない引数で
呼ばれる場合の言語意味は、関数値の apply と同じでなければならない。

関数値に必要数より少ない引数を渡した場合は、渡した引数を capture した残り
引数の関数値になる。これにより curried call は通常の apply の連鎖として扱える。

```arcw
fn add(a: i64)(b: i64) -> i64 { a + b }

let add_two = add(2i64)
let seven = add_two(5i64)
let also_seven = add(2i64)(5i64)
```

Call group は flatten しない。`f(a, b)(c)` と `f(a)(b, c)` は別の
関数型として扱う。この curried call group は ordinary `fn` / trait member /
impl member の関数的な宣言に属する構文であり、`flow` parameter は 0 個または
1 group に限る。
`flow main(a)(b)` のような curried flow parameter は構文診断になる。

宣言本体は最後の call group を適用した時に実行する。途中の group は
評価済みの引数を保持するため、その group の関数型に付く呼び出しの effect row は
空であり、宣言本体の effect row は最後の group に付く。各 group に渡す引数式の
評価による effect は、その引数式を実行した時に発生する。引数や戻り値に含まれる
別の関数型の effect row は、それぞれの関数を apply する境界に属する。

一回の apply に現在の引数列より多くの値を渡すことはできない。余剰引数を
戻り値の関数へ自動で渡さず、次の関数は別の call group で明示的に apply する。

```arcw
fn tuple_tail(a: i64, b: i64)(c: i64) -> (i64, i64, i64) {
    (a, b, c)
}

fn chain(a: i64)(b: i64)(c: i64, d: i64) -> i64 {
    a + b + c + d
}

let tupled = tuple_tail(1i64, 2i64)(3i64)
let sum = chain(1i64)(2i64)(3i64, 4i64)
```

## ジェネリックのスコープと部分適用

nominal 型の引数は不変として比較する。同じ宣言のインスタンスであることを
確認し、各引数をそれぞれのスコープで両方向に照合する。引数が関数型の場合も、
callback の effect row を一方向の包含だけで拡大しない。省略された effect row
は、その呼び出しで選択可能な変数として解決する。

nominal の宣言型へ引数を置換するとき、引数の参照は nominal が保持する外側の
スコープに属する。内側の関数 binder を越える置換では、型・const・effect の
各参照をその binder の分だけ持ち上げ、内側で宣言された参照は維持する。
異なる宣言や owner、引数数の不一致、不正なスコープ参照は拒否する。

宣言の型パラメーターと、その宣言を呼び出す際に推論する型は別の
スコープを持つ。ジェネリック関数の本体で使う型パラメーターは固定され、
再帰呼び出しでも呼び出し元の型を書き換えない。型、配列長の定数、
effect row のパラメーターに同じスコープ規則を適用する。

後続の call group で初めて決まるパラメーターは、部分適用の結果である
関数値に束縛されたまま残る。同じ関数値を複数回呼び出す場合、残った
パラメーターは各呼び出しで独立に決まり、保持済みの引数は再評価しない。

```arcw
fn choose<A, B>(first: A)(second: B) -> B { second }

let prefix = choose(1i64)
let text = prefix("text")
let number = prefix(2i64)
```

この prefix は、後段の B を束縛した関数型を持つ。呼び出し元の固定型と
後段の束縛された型が同じ宣言位置を参照しても、両者を混同しない。
部分適用を生成する式の期待型や明示的な型引数によって、後段の型を先に
確定してもよい。一度確定した型は後続の call group に引き継ぐ。

束縛されたパラメーターを残して公開した関数値は、適用時にその
パラメーターを具体化する。通常の代入や引数適合で、単一の具体的な
関数型へ暗黙に変換しない。関数値の生成時の型推論と、共有済みの
関数値の適用を区別する。任意の let binding の多相化や、明示的な
forall 構文は導入しない。

実行する本体の具体化は、accepted な解析結果が、その結果自身の呼び出し・
宣言・catalog の証拠から発行する。閉じた型置換だけでは、別の解析世代の
本体を実行する権限にならない。呼び出し、継続、宣言値の具体化、
`DisplayText` の本体は、同じ accepted HIR 世代と登録 catalog を保持する。
外側の関数 instance の置換を使う呼び出しでは、その外側の宣言も一致する
必要がある。Copy・Move・借用の証拠は、この instance の置換で検査する。
同じソースを再解析して安定した型・本体の identity が一致しても、世代間で
実行証拠を交換しない。世代を認証するメモリ上の証拠は、保存・配布される
identity の一部にしない。

式の値を作る実行と、関数値の本体を呼び出す実行を区別する。入力証拠は、
その式の lexical owner と閉じた型環境から発行し、同じ環境で検査した
Copy・Move・place・入力の Copy 条件を保持する。別の owner や具体化へ
入力証拠を流用しない。callback の値を作るだけで、その潜在 effect を
実行したことにはならない。本体を呼び出す際には、実際の引数・effect・
制御境界をその呼び出しの環境で閉じる。

formal parameter の静的な所有区分は、同じ accepted 型環境から発行する。
共有借用および明示的な `Shared<T>` carrier は `Shared`、型だけで
unrestricted と証明できる carrier は `Value`、それ以外は `Affine` とする。
可変借用は `Affine` に含める。ジェネリックな宣言の区分は、その型を閉じた
実行 instance で再判定する。rest parameter は個別の引数型ではなく、
callee が受け取る container 型で判定する。

この区分は、実際に渡された値の Copy 保証とは別である。たとえば関数型は、
捕捉した値によって Copy 可否が変わるため `Affine` のまま保持する。本体が
その parameter を Copy する場合は、受理済みの ingress 条件が、渡された
完全な値の unrestricted 保証を検査する。未使用 parameter、wildcard、
分解 pattern にも whole formal の区分を保持し、leaf local の個数や使用回数
から区分を推測しない。

whole formal の証拠は、その実行定義の identity と、同じ閉じた環境の
local-use authority を保持する。ordinary function、明示的 closure、
`_` abstraction の実行側は、この証拠を直接保持して source role と環境の
一致を検査する。型や安定した source identity が同じでも、別の実行 instance
または解析世代の formal 証拠を流用しない。

trait/impl method の実行 fact は、受理済みの `ImplFunctionBody` invocation ABI を
必須で保持する。receiver を含む complete formal layout はその ABI に属する。
閉じた DisplayText instance では本体の local-use authority と ABI の authority が
一致しなければならない。同じ宣言の別 instance から証拠を流用せず、monomorphic
method でも受理済みの宣言 owner と解析世代を検査する。

Core の pure helper と method 入力は、receiver を含む各 formal の Local、静的な passing 区分、
物理 ABI を一つの行で保持する。passing は受理済みの complete formal から渡し、
receiver mode や scalar の格納形式から推測しない。入力の行数と ABI の行数が
別々に変わる表現は持たない。

structured function site は受理済みの lexical definition identity を必須で保持する。
同じ定義の閉じた instance や body の変更で、この identity を作り直さない。
合成 dialogue 値 callback と Rust field default wrapper は、受理済みの意味論 owner が
それぞれ authored slot と checked callable から識別し、plan の割当番号を使わない。
definition identity は body digest や runtime image の seal 証拠とは別に扱う。

pure helper、trait method、Flow の完成した実行行も、definition identity を必須で持つ。
trait method と Flow は受理済み body の証拠から投影し、Entry controller の Flow wrapper は
選択された root instance の定義を保持する。built-in helper はその種類の owner が識別する。

Line group の definition identity は受理済み content occurrence の owner が発行し、
group の完成時まで必須で保持する。group の割当位置や closed な result type は
lexical definition identity の入力に含めない。

structured function-site の parameter 入力は、受理済み formal の passing 区分を
明示的に保持する。引数位置や pattern の展開で区分を失わず、frame ingress の
`Owned`／`Unrestricted` 保証から区分を推測しない。

継続の先行引数と attached default の parameter 入力は、捕捉した whole formal
として区分を保持する。capture packet で値を運ぶことと、formal の静的な
passing 区分は独立しており、一般の lexical capture へ置き換えて区分を落とさない。

Core の入力行は ordinal と別に stable origin を必須で保持する。lexical capture は
受理済み binding coordinate、通常・捕捉済み formal は whole-parameter identity を使う。
合成 dialogue 値の入力は評価済み callback の定義で識別する。取得元種別と origin の
種別が一致しない行は、function の予約を公開する前に拒否する。

whole-formal identity は Core の専用型でも保持し、structured function-site と
direct callable の両方で同じ identity domain を使う。pure helper と trait method の
各入力行は、受理済み formal の identity・local・passing・physical ABI を一緒に保持する。
Flow の invocation schema も whole-formal identity を必須で保持し、Entry の
投影・通常 Flow の生成・保存後の復元で同じ受理済み identity を維持する。

structured input の転送行は、作成時の受理済み Copy／SnapshotClone／Move と、
抽出された実行 body の external binding、whole formal を型で区別する。
capture の転送モードは作成時の証拠から保持し、入場時の Owned／Unrestricted
保証から推測しない。取得元に合わない転送行は予約の公開前に拒否する。

runtime-domain local の semantic fact は、Sema が発行した binding coordinate を
型・scope と同じ行で保持する。発行元の local と受理済み HIR allocation を受理時に
照合し、別 binding や別 generation の座標を混ぜた入力を公開前に拒否する。
閉じた callable instance でも local の型投影はこの証拠を必須で持ち、instance の
local-use authority と照合する。型だけの local 投影や別 generation の証拠は
instance semantic fact の公開前に拒否する。

runtime-domain expression の型行も受理済み expression coordinate と発行元の
generation を必須で保持し、異なる owner や generation の証拠を公開前に拒否する。
閉じた executable instance の expression と statement は、Sema の既存の所有行で
発行された座標を保持する。同じ定義の型特殊化はその座標を維持し、生成時の
arena ID や閉じた型を lexical coordinate の代わりに使わない。

AWBC の各入力行も取得元と作成時の転送方式を必須で保持し、parameter と
captured parameter は静的な passing 区分を codec の往復後も保持する。
function site は受理済みの転送証拠を渡し、formatter operand は選択済みの
Copy／Move を、Line は共有 packet の SnapshotClone と scheduled packet の Move を
保持する。retained 入力を先行させ、
retained と現在の parameter の ordinal をそれぞれ連続した順序で検証する。
入力取得元や passing の欠落・未知タグは拒否し、frame ingress の保証は別に検証する。

## 部分適用

```arcw
let is_high = (_ >= 80)
let add_alice = add_affection(@character.alice, 1)
```

## Spread arguments in partials and staged application

Spread call arguments use `expr...`.

Ordinary exact calls may use spread only where the callee signature gives a
deterministic target. A variable-length spread may feed a rest parameter after
the required fixed parameters have already been supplied. It is not used to
infer how many fixed parameters should be filled.

Partial-call construction accepts spread only when the spread source has a
statically known inline literal length:

```arcw
let add_one = add([1i64]...)
let exact = add([1i64]..., 2i64)
```

The same rule applies to function-value calls:

```arcw
let add_one = add(1i64)
let three = add_one([2i64]...)
```

Variable-length spread remains rejected in partial-call construction and in an
extension call when it would be needed to infer a fixed receiver-adjacent
arity:

```arcw
let later = add(values...)          // error: variable-length partial spread
let later = add(values..., 1i64)    // error: spread followed by fixed arg
let ok = score.clamp(thresholds...) // error: receiver call cannot infer fixed arity
```

This is a language contract, not a temporary lowering limitation. The runtime
can expand `RuntimeExpr::SpreadArg`, but source-level partial construction
needs deterministic arity, argument order, and typed lowering evidence. Use an
inline fixed-length literal spread or write an explicit closure when the spread
length is not statically known.

## Seq と lazy pipeline

```arcw
let choices =
    opening_choices()
        .seq()
        .filter(_.enabled)
        .map(choice_to_view(state))
        .take(5)
```

`Seq<T>` は lazy。必要時に `collect<Vec<T>>()` で materialize。

## effectful map は `traverse`

```arcw
let images = try await image_paths.traverse(asset.image).parallel(limit = 4) with {
    pending p => progress.set(p.ratio)
}
```

- `map`: pure / synchronous。
- `traverse`: `Task` / `Need` を返す。
- `.parallel(limit = N)`: bounded fanout。VM は一度に最大 `N` 件の
  `TaskSpec` を出し、結果を入力順の `Vec` として返す。




