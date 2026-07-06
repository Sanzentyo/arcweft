# pro_review34 revised implementation plan

この計画は、Arcweft の表面構文を dot canonical に寄せ、式 path と
shorthand variant を構造化し、sema の型規則バグを同じ実装スライスで
直し切るためのもの。

## Acceptance criteria

- Module path, import path, std namespace, math namespace, capability namespace,
  static item call, receiver method, field access, shorthand variant, and
  entity reference の責務を AST と sema で分ける。
- 式 path は opaque string ではなく `DottedPath` として保持する。
- `.Json` や `.plain` のような expected-type shorthand は `ShortVariant`
  として保持する。
- `std.f32.*`, `std.f64.*`, `math.*`, `event.emit` などは typed registry
  で解決し、receiver path の個別分岐を増やさない。
- `Block`, `NamedBlock`, `MemoBlock`, `Tuple`, `IfLet`, `Match` へ expected
  type を伝播する。
- Option payload に対する postfix `?` と Result constructor payload の
  expected type を検査する。
- Numeric bracket sequence は container ではなく item expected type を
  参照し、choice item の曖昧さを診断する。
- Equality と ordering operator は operand compatibility を確認する。
- design docs と parser tests は dot canonical の表面構文だけを示す。
- removed surface は silently accepted にしない。

## Implementation order

1. sema の focused bug fix を先に入れる。
   - expected type propagation
   - branch join と `Never` handling
   - Option postfix `?`
   - expected Result constructor payload
   - numeric bracket item expected type
   - comparison compatibility

2. syntax AST を構造化する。
   - `Name`
   - `DottedPath`
   - `ShortVariant`
   - `Expr Path variant(DottedPath)`
   - shorthand selector は `Expr ShortVariant variant(Name)`

3. call sites を label access へ移行する。
   - display や runtime lowering は `as_label()` を使う。
   - shorthand は必要な場所だけ leading-dot label に戻す。
   - traversal では `ShortVariant` を leaf expression として扱う。

4. sema resolver を registry 化する。
   - std float calls and constants
   - math matrix and tensor calls
   - capability call argument policy
   - method-call path branch は generic registry lookup にする。

5. module and use surface を dot canonical にする。
   - parser-owned module path type は dot-separated segments を読む。
   - formatter/display は dot-separated source spelling を出す。
   - grouped import and wildcard import are written as `path.{item}` and
     `path.*`.
   - `parent` alias, when accepted, normalizes to `super`.

6. docs and fixtures を更新する。
   - design docs under `docs/01-language/`
   - examples under `docs/examples/`
   - parser and sema fixtures

7. validation.
   - `cargo fmt`
   - focused syntax and sema tests
   - workspace check
   - workspace clippy when feasible
   - structural audit at the reviewable cut point

## Non-goals for this slice

- Rust ABI metadata may still describe Rust symbols using Rust-native spelling.
- Historical implementation notes may preserve old text when they are clearly
  archived history rather than current Arcweft surface syntax.
