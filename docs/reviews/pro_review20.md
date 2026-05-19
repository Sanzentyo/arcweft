# Arcweft 命名整合性監査メモ: `Vec` / `List` 周辺と関連する旧名残

作成日: 2026-05-19  
対象: `Sanzentyo/arcweft` の現行 `main` 相当で確認した、前回指摘箇所の整理  
目的: `Vec<T>` を標準の可変長順序列として採用する方針に対して、`List` という旧名残・曖昧名・関連する整合性崩れを具体的に修正可能な形に落とす。

---

## 0. 結論

`List` は **公開型名としては使わない** 方針に寄せるのがよい。

現行仕様では、標準 collection は Rust-like な名前を使い、`Vec<T>` が「default growable ordered sequence」、`Array<T, N>` が固定長 sequence、`Seq<T>` が pure lazy sequence という役割を持つ。そのため、AST や grammar に残っている `List` は、少なくとも「公開型名 `List<T>` を示すもの」には見えないようにするべき。

推奨する中心方針は次の通り。

```text
公開型 / 意味型:
  Vec<T>
  Array<T, N>
  Slice<'a, T>
  Seq<T>

構文 AST:
  Expr::BracketSeq
  Pattern::BracketSeq

grammar:
  BracketSeqExpr
  BracketSeqPattern

避ける:
  Expr::List
  Pattern::List
  ListExpr
  ListPattern
  Expr::VecLiteral     // Array<T, N> にもなれるので不正確
  Expr::SeqLiteral     // Seq<T> と紛らわしい
```

---

## 1. 判断基準

### 1.1 表面型名と構文名を分離する

`[a, b, c]` は表面構文としては bracket sequence literal だが、意味型としては期待型により分岐する。

```awft
let dynamic: Vec<i32> = [1, 2, 3]
let fixed: Array<i32, 3> = [1, 2, 3]
```

したがって AST の名前は `VecLiteral` でも `ArrayLiteral` でもなく、表面構文そのものを表す名前がよい。

### 1.2 `Seq<T>` との衝突を避ける

`Seq<T>` は Arcweft では pure lazy sequence の標準型。`Expr::SeqLiteral` のような名前にすると、`[a, b, c]` が `Seq<T>` を作るように見えるため避ける。

### 1.3 `List` は型として曖昧

`List<T>` は言語により linked list、persistent list、array-like list のいずれにも読める。Arcweft では `Vec<T>` が容量 API を持つ可変長順序列なので、`List` は旧名残に見える。

---

## 2. 優先度一覧

| ID | 優先度 | 対象 | 現状 | 推奨 |
|---:|:---:|---|---|---|
| L-01 | P0 | `Expr::List` | AST が `List` 名 | `Expr::BracketSeq` に改名 |
| L-02 | P0 | `parse_list` | `[...]` parser が `list` 名 | `parse_bracket_seq` に改名 |
| L-03 | P0 | `check_list_expr_with_expected` | 型チェック関数が `list` 名 | `check_bracket_seq_with_expected` に改名 |
| L-04 | P0 | diagnostics | `list items must...` | `sequence literal items must...` |
| L-05 | P0 | `Pattern::List` | pattern AST が `List` 名 | `Pattern::BracketSeq` に改名 |
| L-06 | P0 | `parse_list_pattern` | pattern parser が `list` 名 | `parse_bracket_seq_pattern` に改名 |
| L-07 | P0 | grammar `ListExpr` | `hooks = ListExpr` が残る | `hooks = Expr` または `BracketSeqExpr` |
| L-08 | P0 | grammar `ListPattern` | pattern grammar が `ListPattern` 名 | `BracketSeqPattern` に改名 |
| L-09 | P1 | docs `list literal` | `List<T>` を想起しやすい | `bracket sequence literal` に統一 |
| L-10 | P1 | tests | `list expression parses` など | `bracket sequence parses` へ更新 |
| M-01 | P1 | `Map` / `Set` alias | docs と prelude がズレる | alias を正式化するか削除する |
| M-02 | P1 | `TypeKind::Map` | `OrderedMap` / `BTreeMap` と区別しない | `MapKind` 付き、または明示 variant へ |
| B-01 | P0 | `iter_item_type` | `Vec<...>` 分岐重複かつ slice index バグ疑い | 分岐整理、`Vec<` は index 4 |
| D-01 | P2 | `Attribute` doc comment | `#derive(...)` と記述 | `#[derive(...)]` に修正 |
| D-02 | P1 | hook docs | `check every frame` 旧構文が残る | grammar に合わせて削除・置換 |

---

## 3. L-01: `Expr::List` を `Expr::BracketSeq` に改名

### 現状

`crates/arcweft-lang-syntax/src/expr.rs` に次の AST variant がある。

```rust
pub enum Expr {
    ...
    Tuple(Vec<Expr>),
    List(Vec<Expr>),
    ...
}
```

また、`parse_list()` が `[` ... `]` を `Expr::List(items)` にしている。

### 問題

`Expr::List` は `List<T>` という型が存在するかのように見える。だが Arcweft の正本は次の設計。

- `Vec<T>`: default growable ordered sequence
- `Array<T, N>`: fixed-length sequence
- `[a, b, c]`: expected type が `Array<T, 3>` なら `Array`、そうでなければ `Vec`

つまり AST は「list 型」ではなく「bracket sequence literal 構文」を表している。

### 推奨修正

```rust
Expr::List(Vec<Expr>)
```

を次へ改名。

```rust
Expr::BracketSeq(Vec<Expr>)
```

### 代替案

| 候補 | 評価 |
|---|---|
| `Expr::BracketSeq` | 推奨。表面構文を表し、`Vec` / `Array` / `Seq` と衝突しない |
| `Expr::SequenceLiteral` | 可。やや長いが意味は明確 |
| `Expr::ArrayLiteral` | 非推奨。`Vec<T>` にもなるため不正確 |
| `Expr::VecLiteral` | 非推奨。`Array<T, N>` にもなるため不正確 |
| `Expr::SeqLiteral` | 非推奨。標準型 `Seq<T>` と衝突 |

### 影響箇所

少なくとも次の箇所が影響する。

```text
crates/arcweft-lang-syntax/src/expr.rs
  - Expr::List
  - parse_list()
  - Expr::List(items) を返す箇所

crates/arcweft-lang-sema/src/check.rs
  - Expr::List(items) match 分岐
  - check_list_expr_with_expected
  - simple_expr_type の Expr::List 分岐

crates/arcweft-lang-sema/src/symbols.rs
  - collect_expr の Expr::List 分岐

crates/arcweft-lang-syntax/src/parser.rs
  - push_line_hooks で Expr::List を展開する箇所

テスト群
  - `list expression parses`
  - `empty list expression parses`
  - `nested list expression parses`
```

### 機械的置換案

```text
Expr::List
  -> Expr::BracketSeq

parse_list
  -> parse_bracket_seq

check_list_expr_with_expected
  -> check_bracket_seq_with_expected
```

---

## 4. L-02: `parse_list` を `parse_bracket_seq` に改名

### 現状

`parse_prefix()` が `Token::LBracket` を見たときに `parse_list()` を呼んでいる。

```rust
Token::LBracket => self.parse_list(),
```

`parse_list()` は `[]` や `[a, b, c]` をパースし、`Expr::List(items)` を返す。

### 問題

パーサの関数名が AST 旧名残と同じく `list` になっている。

### 推奨修正

```rust
fn parse_list(&mut self) -> Result<Expr, ExprParseError>
```

を次へ。

```rust
fn parse_bracket_seq(&mut self) -> Result<Expr, ExprParseError>
```

呼び出し側も変更。

```rust
Token::LBracket => self.parse_bracket_seq(),
```

### diagnostics の変更

現状のエラー文が次のようになっている。

```text
expected `]` or `,` in list
```

これも変更。

```text
expected `]` or `,` in bracket sequence literal
```

または短く。

```text
expected `]` or `,` in sequence literal
```

---

## 5. L-03: `check_list_expr_with_expected` を `check_bracket_seq_with_expected` に改名

### 現状

型チェック側では `Expr::List(items)` が expected type を受け取り、`Array<T, N>` に合えば `Array`、それ以外は `Vec` を返す。

```rust
Expr::List(items) => Some(self.check_list_expr_with_expected(items, expected)),
```

関数の挙動は概ね正しい。

### 問題

関数名だけが `list` で、意味的には bracket sequence literal の型付けをしている。

### 推奨修正

```rust
fn check_list_expr_with_expected(
    &mut self,
    items: &[Expr],
    expected: Option<&TypeKind>,
) -> TypeKind
```

を次へ。

```rust
fn check_bracket_seq_with_expected(
    &mut self,
    items: &[Expr],
    expected: Option<&TypeKind>,
) -> TypeKind
```

または、やや一般名として次も可。

```rust
fn check_sequence_literal_with_expected(...)
```

### diagnostics の変更

現状。

```text
list items must have the same type, found ...
```

推奨。

```text
sequence literal items must have the same type, found ...
```

---

## 6. L-04: `simple_expr_type` の `Expr::List` 分岐

### 現状

`simple_expr_type()` は `Expr::List(items)` を `TypeKind::Vec` にしている。

```rust
Expr::List(items) => {
    let item = items
        .first()
        .and_then(simple_expr_type)
        .unwrap_or(TypeKind::Unit);
    Some(TypeKind::Vec(Box::new(item)))
}
```

### 判断

ここは「簡易推論」なので `Vec` に倒すこと自体は許容できる。問題は `Expr::List` という名前。

### 推奨修正

```rust
Expr::BracketSeq(items) => {
    let item = items
        .first()
        .and_then(simple_expr_type)
        .unwrap_or(TypeKind::Unit);
    Some(TypeKind::Vec(Box::new(item)))
}
```

将来的に expected type を見られない簡易推論であることを comment しておくとよい。

```rust
// Without an expected fixed-length type, bracket sequence literals default to Vec<T>.
```

---

## 7. L-05: `Pattern::List` を `Pattern::BracketSeq` に改名

### 現状

`crates/arcweft-lang-syntax/src/ast.rs` に次の pattern variant がある。

```rust
pub enum Pattern {
    ...
    List {
        items: Vec<Pattern>,
        rest: Option<String>,
    },
    ...
}
```

### 問題

pattern は `Vec` だけでなく、owned list 的な値、fixed sequence、borrowed slice 的な値にも対応する構文として説明されている。`List` という名前は公開型名と同じ曖昧さを持つ。

### 推奨修正

```rust
Pattern::List { items, rest }
```

を次へ。

```rust
Pattern::BracketSeq { items, rest }
```

### 影響箇所

```text
crates/arcweft-lang-syntax/src/ast.rs
crates/arcweft-lang-syntax/src/pattern.rs
crates/arcweft-lang-sema/src/check.rs
crates/arcweft-lang-sema/src/symbols.rs
crates/arcweft-lang-sema/src/tests/patterns.rs
```

### 機械的置換案

```text
Pattern::List
  -> Pattern::BracketSeq

parse_list_pattern
  -> parse_bracket_seq_pattern
```

---

## 8. L-06: `parse_list_pattern` を `parse_bracket_seq_pattern` に改名

### 現状

`pattern.rs` では `[` ... `]` を見て `parse_list_pattern(inner)` を呼び、`Pattern::List` を返している。

### 推奨修正

```rust
fn parse_list_pattern(inner: &str) -> Pattern
```

を次へ。

```rust
fn parse_bracket_seq_pattern(inner: &str) -> Pattern
```

戻り値も変更。

```rust
Pattern::BracketSeq { items, rest }
```

### メモ

`RestPattern` の構文 `..rest` は維持でよい。ここは `List` / `Vec` 命名とは別問題。

---

## 9. L-07: grammar の `ListExpr` を修正

### 現状

grammar の dialogue line option に次がある。

```text
LineOption :=
  ...
  | 'hooks' '=' ListExpr
  ...
```

一方、実装では `hooks` の値を通常の `Expr` として parse し、値が `Expr::List` なら展開、それ以外なら単一 hook として扱っている。

```rust
"hooks" => push_line_hooks(&mut state.hooks, parse_expr_lossy(value)),
```

### 問題

grammar は `hooks` に list literal のみを許しているように見えるが、実装は単一式も許す。また `ListExpr` という名前が公開型 `List<T>` を想起させる。

### 推奨修正

実装に合わせるなら grammar を次へ。

```text
LineOption :=
  ...
  | 'hooks' '=' Expr
  ...
```

説明文で許容形を明記する。

```awft
alice(hooks=@hook.line.enter): ...
alice(hooks=[@hook.line.enter, @hook.line.exit]): ...
```

もし list literal のみ許す設計にするなら、grammar は次。

```text
'hooks' '=' BracketSeqExpr
```

ただし、この場合は実装の単一式許容を削る必要がある。現状の柔軟性を考えると **`hooks = Expr` 推奨**。

---

## 10. L-08: grammar の `ListPattern` を修正

### 現状

grammar に次がある。

```text
Pattern :=
  ...
  | ListPattern

ListPattern := '[' Pattern* RestPattern? ']'
RestPattern := '..' Ident?
```

### 推奨修正

```text
Pattern :=
  ...
  | BracketSeqPattern

BracketSeqPattern := '[' Pattern* RestPattern? ']'
RestPattern       := '..' Ident?
```

### docs の見出しも変更

現状。

```md
## List / slice patterns
```

推奨。

```md
## Bracket sequence patterns
```

説明文。

```md
Vec, Array, and borrowed Slice destructuring use the same `[ ... ]` surface syntax.
Type checking decides which sequence-like scrutinee forms are accepted.
```

---

## 11. L-09: docs の `list literal` を `bracket sequence literal` へ

### 現状

標準型 docs では、`[a, b, c]` が固定長 expected type の有無によって `Array` または `Vec` になると説明している。この説明自体は正しい。

### 修正方針

`list literal` という語を避け、次のどちらかへ統一する。

```text
bracket sequence literal
sequence literal
```

私は **bracket sequence literal** 推奨。`Seq<T>` との衝突をさらに避けられる。

### 例

変更前。

```md
`Vec<T>` is the default growable ordered sequence. It preserves authored order
and is the normal target for list literals when no fixed-size context exists.
```

変更後。

```md
`Vec<T>` is the default growable ordered sequence. It preserves authored order
and is the normal target for bracket sequence literals when no fixed-size context exists.
```

---

## 12. L-10: tests の文言と match を更新

### 対象例

`crates/arcweft-lang-sema/src/tests/expressions.rs` に次のようなテスト名・期待がある。

```rust
let list = parse_expr("[normal, smile, worried]").expect("list expression parses");
assert!(matches!(list, Expr::List(items) if items.len() == 3));

let empty_list = parse_expr("[]").expect("empty list expression parses");
assert!(matches!(empty_list, Expr::List(items) if items.is_empty()));
```

### 推奨修正

```rust
let seq = parse_expr("[normal, smile, worried]").expect("bracket sequence parses");
assert!(matches!(seq, Expr::BracketSeq(items) if items.len() == 3));

let empty_seq = parse_expr("[]").expect("empty bracket sequence parses");
assert!(matches!(empty_seq, Expr::BracketSeq(items) if items.is_empty()));
```

pattern tests も同様。

```rust
Pattern::List { items, rest }
```

を次へ。

```rust
Pattern::BracketSeq { items, rest }
```

---

## 13. M-01: `Map` / `Set` alias の扱いを決める

### 現状

標準型 docs の collections には次が並ぶ。

```text
Vec<T>
VecDeque<T>
OrderedMap<K, V>
BTreeMap<K, V>
OrderedSet<T>
BTreeSet<T>
BitSet<E>
Array<T, N>
```

一方、`arcweft-adt` には次がある。

```rust
pub type Map<K, V> = OrderedMap<K, V>;
pub type Set<T> = OrderedSet<T>;
```

facade prelude でも `Map` / `Set` が re-export されている。

また、docs の例には `Map<K, Vec<T>>` が出る。

### 問題

`Map` / `Set` が正式 prelude なのか、便利 alias なのか、旧名残なのかが曖昧。`OrderedMap` と `BTreeMap` の区別を重視する Arcweft の deterministic 方針ともやや衝突する。

### 選択肢

#### A. `Map` / `Set` を正式 alias として docs に載せる

```text
Map<K, V> = OrderedMap<K, V>
Set<T>    = OrderedSet<T>
```

メリット:

- 書きやすい。
- 既存実装・prelude を活かせる。

デメリット:

- `OrderedMap` / `BTreeMap` の意味差を隠す。
- replay-visible な順序契約を読者が意識しにくい。

#### B. `Map` / `Set` を公開 prelude から外し、docs も `OrderedMap` / `OrderedSet` に寄せる

推奨。

メリット:

- 順序契約が明示的。
- `BTreeMap` との使い分けが明確。

デメリット:

- API 名が少し長い。
- 既存例やテストの置換が必要。

### 推奨修正

Arcweft の deterministic / replay-visible 方針を優先するなら B。

変更前。

```awft
pub fn group_by<T, K>(key: T -> K)(xs: Vec<T>) -> Map<K, Vec<T>>
where
    K: Eq + Hash
{
    ...
}
```

変更後。

```awft
pub fn group_by<T, K>(key: T -> K)(xs: Vec<T>) -> OrderedMap<K, Vec<T>>
where
    K: Eq
{
    ...
}
```

canonical sorted order が必要な例なら次。

```awft
pub fn group_by<T, K>(key: T -> K)(xs: Vec<T>) -> BTreeMap<K, Vec<T>>
where
    K: Ord
{
    ...
}
```

---

## 14. M-02: `TypeKind::Map` を explicit map kind にする

### 現状

semantic checker の `TypeKind` に次がある。

```rust
Map {
    key: Box<TypeKind>,
    value: Box<TypeKind>,
}
```

`type_ref_kind()` は `Map<K, V>` だけを特別扱いしている。

```rust
TypeRef::Generic { base, args } if base == "Map" && args.len() == 2 => TypeKind::Map { ... }
```

### 問題

標準型 docs では `OrderedMap` と `BTreeMap` が区別されているのに、semantic type は `Map` だけ。これだと順序契約が型検査・診断に残らない。

### 推奨修正案 1: `MapKind` を追加

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MapKind {
    Ordered,
    BTree,
}

pub enum TypeKind {
    ...
    Map {
        kind: MapKind,
        key: Box<TypeKind>,
        value: Box<TypeKind>,
    },
    ...
}
```

`type_ref_kind()` の分岐。

```rust
TypeRef::Generic { base, args } if base == "OrderedMap" && args.len() == 2 => TypeKind::Map {
    kind: MapKind::Ordered,
    key: Box::new(type_ref_kind(&args[0])),
    value: Box::new(type_ref_kind(&args[1])),
},
TypeRef::Generic { base, args } if base == "BTreeMap" && args.len() == 2 => TypeKind::Map {
    kind: MapKind::BTree,
    key: Box::new(type_ref_kind(&args[0])),
    value: Box::new(type_ref_kind(&args[1])),
},
```

`Map` alias を残すなら。

```rust
TypeRef::Generic { base, args } if base == "Map" && args.len() == 2 => TypeKind::Map {
    kind: MapKind::Ordered,
    ...
},
```

### 推奨修正案 2: explicit variant に分ける

```rust
OrderedMap { key, value }
BTreeMap { key, value }
```

こちらは match が増えるが、診断表示はわかりやすい。

### 影響箇所

```text
crates/arcweft-lang-sema/src/check.rs
  - TypeKind::Map
  - type_ref_kind
  - collection_index_type
  - collect_type_kind_lifetimes
  - type_contains_borrow_ref
```

---

## 15. B-01: `iter_item_type` の `Vec<...>` 分岐重複と slice index バグ疑い

### 現状

`iter_item_type()` に、`Vec<...>` を扱う分岐が重複している。

```rust
Some(TypeKind::Named(name)) if name.starts_with("Vec<") && name.ends_with('>') => {
    TypeKind::Named(name[5..name.len() - 1].to_owned())
}
Some(TypeKind::Named(name)) if name.starts_with("Seq<") && name.ends_with('>') => {
    TypeKind::Named(name[4..name.len() - 1].to_owned())
}
Some(TypeKind::Named(name)) if name.starts_with("Vec<") && name.ends_with('>') => {
    TypeKind::Named(name[4..name.len() - 1].to_owned())
}
```

### 問題

1つ目の `Vec<` の slice index が `5` になっている。`"Vec<"` は4文字なので、`name[4..]` が正しい。  
さらに3つ目の `Vec<` 分岐は unreachable で、たぶん `Slice<...>` などからの変更漏れ。

### 推奨修正

```rust
Some(TypeKind::Named(name)) if name.starts_with("Vec<") && name.ends_with('>') => {
    TypeKind::Named(name[4..name.len() - 1].to_owned())
}
Some(TypeKind::Named(name)) if name.starts_with("Seq<") && name.ends_with('>') => {
    TypeKind::Named(name[4..name.len() - 1].to_owned())
}
Some(TypeKind::Named(name)) if name.starts_with("Slice<") && name.ends_with('>') => {
    TypeKind::Named(name[6..name.len() - 1].to_owned())
}
```

`Array<T, N>` 文字列表現も Named として来る可能性があるなら、次も追加候補。

```rust
Some(TypeKind::Named(name)) if name.starts_with("Array<") && name.ends_with('>') => {
    let inner = &name[6..name.len() - 1];
    let item = inner.split_once(',').map_or(inner, |(item, _)| item).trim();
    TypeKind::Named(item.to_owned())
}
```

### 優先度

P0。これは命名整合性というより、実バグの可能性が高い。

---

## 16. D-01: `Attribute` コメントの `#derive(...)` を修正

### 現状

AST の `Attribute` doc comment に次のような説明がある。

```rust
/// Attribute syntax such as `#derive(...)`.
```

### 問題

Arcweft の canonical attribute syntax は Rust 風の `#[derive(...)]`。`#derive(...)` は旧仕様か単純な typo に見える。

### 推奨修正

```rust
/// Attribute syntax such as `#[derive(...)]`.
```

---

## 17. D-02: hook docs の `check every frame` を削除・置換

### 現状

`docs/01-language/syntax.md` の hook 例に次が残っている。

```awft
hook @hook.opening.choice_visible
on @choice.opening.listen
phase AfterLayout
check every frame
when object.visible && object.enabled
...
```

一方、grammar では `check` は canonical hook header ではなく、条件には `when` を使うことになっている。

### 問題

旧仕様の hook header がサンプルに残っている。

### 推奨修正

`check every frame` を削除する。

変更後。

```awft
hook @hook.opening.choice_visible
on @choice.opening.listen
phase AfterLayout
when object.visible && object.enabled
effects { signal_write, assert }
{
    signal.set(@signal.choice_visible, true)
    debug_assert object.bbox.area > 0
}
```

もしチェック頻度を残したいなら、canonical grammar に `schedule` / `sampling` / `trigger` のような別 header を正式追加してから使うべき。

---

## 18. 置換・確認用 grep コマンド

### `List` 系

```bash
rg '\bExpr::List\b|\bPattern::List\b|parse_list\b|parse_list_pattern\b|check_list_expr_with_expected\b|ListExpr\b|ListPattern\b|list expression|empty list|nested list|list items must' crates docs
```

### docs の説明語

```bash
rg 'list literal|List / slice patterns|owned list|borrowed slice|ListExpr|ListPattern' docs crates
```

### `Map` / `Set` 系

```bash
rg '\bMap<|\bSet<|TypeKind::Map|pub type Map|pub type Set|OrderedMap|BTreeMap|OrderedSet|BTreeSet' crates docs
```

### 明確な旧仕様・typo

```bash
rg '#derive|check every frame' crates docs
```

### unreachable / 重複っぽい分岐

```bash
rg 'starts_with\("Vec<"\)|starts_with\("Seq<"\)|starts_with\("Slice<"\)|iter_item_type' crates/arcweft-lang-sema/src/check.rs
```

---

## 19. 推奨パッチ順序

### Phase 1: `List` AST 改名だけを先に入れる

目的: コンパイラエラーで影響範囲を洗い出せる、低リスクな機械的 rename。

1. `Expr::List` -> `Expr::BracketSeq`
2. `Pattern::List` -> `Pattern::BracketSeq`
3. `parse_list` -> `parse_bracket_seq`
4. `parse_list_pattern` -> `parse_bracket_seq_pattern`
5. `check_list_expr_with_expected` -> `check_bracket_seq_with_expected`
6. diagnostics を `list` から `sequence literal` へ
7. tests を更新

### Phase 2: grammar / docs を追従

1. `ListExpr` -> `Expr` or `BracketSeqExpr`
2. `ListPattern` -> `BracketSeqPattern`
3. `list literal` -> `bracket sequence literal`
4. `List / slice patterns` -> `Bracket sequence patterns`
5. hook example の `check every frame` を削除
6. `#derive(...)` comment を修正

### Phase 3: `Map` / `Set` 方針を決める

先に設計判断を入れる。

- explicit 方針なら `Map` / `Set` alias を prelude から外すか、deprecated alias にする。
- alias 方針なら standard-types docs に `Map` / `Set` を正式追加する。

### Phase 4: semantic `TypeKind::Map` を整理

1. `MapKind` 追加、または `OrderedMap` / `BTreeMap` variant 追加
2. `type_ref_kind` を `OrderedMap` / `BTreeMap` 対応にする
3. `collection_index_type` など recursive match を更新
4. tests 追加

### Phase 5: `iter_item_type` バグ修正

これは Phase 1 と独立に先に直してもよい。

---

## 20. 追加すべきテスト

### 20.1 bracket sequence literal の型付け

```rust
#[test]
fn bracket_sequence_defaults_to_vec_without_fixed_expected_type() {
    // [1, 2, 3] -> Vec<Int>
}

#[test]
fn bracket_sequence_can_type_as_array_with_fixed_expected_type() {
    // expected Array<Int, 3>, [1, 2, 3] -> Array<Int, 3>
}
```

### 20.2 pattern の命名追従

```rust
#[test]
fn bracket_sequence_pattern_keeps_rest_binding() {
    // let [first, ..rest] = items
    // Pattern::BracketSeq { items, rest }
}
```

### 20.3 hooks option

```rust
#[test]
fn line_hooks_accept_single_hook_or_bracket_sequence() {
    // hooks=@hook.a
    // hooks=[@hook.a, @hook.b]
}
```

### 20.4 Map kind

```rust
#[test]
fn ordered_map_and_btree_map_keep_distinct_type_kind() {
    // OrderedMap<K, V> != BTreeMap<K, V>
}
```

### 20.5 iter_item_type

```rust
#[test]
fn iter_item_type_extracts_named_vec_seq_slice_items() {
    // Vec<Foo> -> Foo
    // Seq<Foo> -> Foo
    // Slice<Foo> -> Foo
}
```

---

## 21. 最終的な期待状態

修正後は次のような状態が望ましい。

```rust
pub enum Expr {
    Tuple(Vec<Expr>),
    BracketSeq(Vec<Expr>),
    ...
}

pub enum Pattern {
    Tuple(Vec<Pattern>),
    BracketSeq {
        items: Vec<Pattern>,
        rest: Option<String>,
    },
    ...
}
```

grammar。

```text
BracketSeqExpr    := '[' Expr (',' Expr)* ','? ']'
BracketSeqPattern := '[' Pattern* RestPattern? ']'
RestPattern       := '..' Ident?
```

標準型 docs。

```text
`Vec<T>` is the default growable ordered sequence and the normal target for
bracket sequence literals when no fixed-size context exists.

`Array<T, N>` is a fixed-length sequence. A bracket sequence literal can be
typed as `Array<T, N>` when the expected type requires exactly N elements.
```

---

## 22. 変更しないもの

以下は `List` 名残ではないため、変更しない。

```text
Vec<T>
VecDeque<T>
Array<T, N>
Seq<T>
Slice<'a, T>
OrderedMap<K, V>
BTreeMap<K, V>
OrderedSet<T>
BTreeSet<T>
BitSet<E>
```

また、Rust 実装上の `std::vec::Vec` や `Vec<Expr>` など、Rust の container としての `Vec` はもちろん維持する。

---

## 23. ひとことで言うと

`List` は「型名」でも「意味名」でもなく、単なる `[...]` 構文の旧呼称として残っている。  
Arcweft の正本に合わせるなら、内部 AST / grammar では **`BracketSeq`**、公開型では **`Vec` / `Array` / `Slice` / `Seq`** に分離するのが最も整合的。
