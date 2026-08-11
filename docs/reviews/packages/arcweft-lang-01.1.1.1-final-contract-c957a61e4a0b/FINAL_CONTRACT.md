# Lang-01.1.1.1 — prefix/postfix Try source and propagation final contract

**STATUS=FINAL**  
**OPEN_QUESTIONS=0**  
**BASE_MAIN=c957a61e4a0b9abf094165c41ef4038ce25324c0**  
**IMPLEMENTATION_IN_THIS_PACKAGE=NONE**

The words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative. This contract resolves every decision requested by `source/REQUEST.md`; it does not create follow-up design work.

## 1. Frozen decisions

### D1 — General prefix Try is retained

`try expr` and `expr?` both remain canonical authored Arcweft. Neither is an alias for the other, neither is deprecated, and neither receives a removed-spelling diagnostic. Both lower to the same semantic Try operation. This is the minimum correction because current production documentation and Agent, CLI, dialogue, ordinary expression, verifier, and runtime-plan consumers already use the prefix surface; no concrete defect was found in retaining the surface itself.

The grammar contract is:

```ebnf
TryExpr         = PrefixTryExpr | PostfixTryExpr ;
PrefixTryExpr   = "try" PrefixOperand ;
PostfixTryExpr  = Expr "?" ;
```

Prefix Try uses the existing prefix binding power `90`; postfix Try uses the existing postfix binding power `100`. The parser MUST keep these precedence values unless a later general precedence contract changes them for all prefix/postfix operators.

### D2 — Await substrate is not redesigned

The existing `AwaitExpr`, `AwaitPropagation`, `AwaitExprSource`, and `AwaitPropagationSource` remain the sole Await model. Their fields, constructor visibility, accessors, propagation spellings, and exact source ranges remain unchanged. The implementation MAY add consumer calls but MUST NOT add a second Await AST, Boolean spelling flag, source string, compatibility wrapper, or duplicate parser.

### D3 — One final Try AST replaces the old shape directly

`Expr::Try { expr: Box<Expr> }` is removed in the same atomic syntax/consumer cut in which `Expr::Try(TryExpr)` is introduced. No compatibility variant, deprecated constructor, public type alias, dual visitor branch, or source-text fallback survives the cut.

The exact final syntax types are:

```rust
/// Exact authored operator token for one general Try expression.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TryOperatorSource {
    /// `try operand`.
    PrefixTry { try_keyword: TextRange },
    /// `operand?`.
    PostfixQuestion { question: TextRange },
}

impl TryOperatorSource {
    /// Exact half-open UTF-8 byte range of the authored operator token.
    pub const fn range(self) -> TextRange {
        match self {
            Self::PrefixTry { try_keyword } => try_keyword,
            Self::PostfixQuestion { question } => question,
        }
    }
}

/// Exact source ownership for one general Try expression.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TryExprSource {
    whole: TextRange,
    operand: TextRange,
    operator: TryOperatorSource,
}

impl TryExprSource {
    pub(crate) const fn new(
        whole: TextRange,
        operand: TextRange,
        operator: TryOperatorSource,
    ) -> Self {
        Self { whole, operand, operator }
    }

    pub const fn whole(self) -> TextRange { self.whole }
    pub const fn operand(self) -> TextRange { self.operand }
    pub const fn operator(self) -> TryOperatorSource { self.operator }
    pub const fn operator_range(self) -> TextRange { self.operator.range() }
}

/// Semantic Result/Option propagation with authored source evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TryExpr {
    operand: Box<Expr>,
    source: TryExprSource,
}

impl TryExpr {
    pub(crate) const fn new(operand: Box<Expr>, source: TryExprSource) -> Self {
        Self { operand, source }
    }

    pub const fn operand(&self) -> &Expr { &self.operand }
    pub const fn source(&self) -> TryExprSource { self.source }

    pub fn into_parts(self) -> (Expr, TryExprSource) {
        (*self.operand, self.source)
    }
}

pub enum Expr {
    // unchanged variants omitted
    Try(TryExpr),
    Await(AwaitExpr),
    // unchanged variants omitted
}
```

The semantic operation is represented by `Expr::Try` itself. `TryOperatorSource` records authored spelling only. Semantic consumers MUST NOT branch on `PrefixTry` versus `PostfixQuestion`; only diagnostics, source mapping, formatting, refactoring, and other source-facing tooling MAY inspect that enum.

### D4 — Exact source invariants

All ranges are absolute, half-open UTF-8 byte offsets in the owning source snapshot, including any nonzero parser base offset.

1. `whole` begins at the first nontrivia token owned by the Try expression and ends immediately after its final owned token. Leading and trailing trivia are excluded; interstitial trivia is covered by `whole` but not by token or operand ranges.
2. `operand` is the complete authored syntactic operand. For a grouped postfix operand it includes the parentheses, for example `(await need)` in `(await need)?`.
3. `PrefixTry.try_keyword` owns exactly the three bytes of `try`.
4. `PostfixQuestion.question` owns exactly the one byte of `?`.
5. Prefix invariant: `whole.start == try_keyword.start`, `try_keyword.end <= operand.start`, and `whole.end == operand.end`.
6. Postfix invariant: `whole.start == operand.start`, `operand.end <= question.start`, and `whole.end == question.end`.
7. Constructors remain `pub(crate)` so parser/HIR-owned tests can build fixtures, while external callers cannot fabricate source ownership.
8. Later layers MUST consume these ranges. They MUST NOT use `strip_prefix`, `strip_suffix`, substring search, token rescans, function-name rules, role-name rules, or raw source spelling to recover Try shape.

### D5 — One parser path

The strict expression parser and the private lossless/event parser MUST agree on the same typed shape and ranges.

- Add general prefix `try` to the ordinary prefix-expression production with binding power `90` and `SyntaxKind::TryExpression`.
- Keep ordinary postfix `?` in the Pratt postfix production with binding power `100` and construct `TryExprSource::PostfixQuestion` at token consumption time.
- Keep the direct `try await` and `await?` recognizers that construct one existing `AwaitExpr`; do not first construct a Try node and then fold it.
- Delete `DialogueSurface::has_try_prefix`, the dialogue-only `TryExpression` wrapper, `strip_prefix("try ")` Try construction, and every dialogue helper that recognizes Try spelling separately. Dialogue disambiguation may still select a dialogue call, but the enclosing Try MUST be parsed by the ordinary expression grammar.
- Keep `SyntaxKind::TryExpression`; no new compatibility CST kind is added.
- The current typed bound-expression-fragment substrate is consumed as-is. It is not redesigned for this request.

### D6 — Required grouping and exact core ranges

| Source | Required top-level shape | Exact source facts |
|---|---|---|
| `value?` | `Expr::Try(PostfixQuestion)` | Try `whole=0..6`, `operand=0..5`, `question=5..6` |
| `try value` | `Expr::Try(PrefixTry)` | Try `whole=0..9`, `operand=4..9`, `try=0..3` |
| `try await need` | one `Expr::Await`, `PropagateError` | Await `whole=0..14`, `await=4..9`, `operand=10..14`, prefix `try=0..3`; **no Try node** |
| `await? need` | one `Expr::Await`, `PropagateError` | Await `whole=0..11`, `await=0..5`, `operand=7..11`, attached `?=5..6`; **no Try node** |
| `(await need)?` | outer postfix Try wrapping one `Await(PreserveResult)` | Try `whole=0..13`, `operand=0..12`, `?=12..13`; inner Await `whole=1..11`, `await=1..6`, `operand=7..11` |
| `await need?` | outer `Await(PreserveResult)` whose operand is postfix Try | Await `whole=0..11`, `await=0..5`, `operand=6..11`; inner Try `whole=6..11`, `operand=6..10`, `?=10..11` |
| `try (await need)` | outer general prefix Try wrapping `Await(PreserveResult)` | Try `whole=0..16`, `operand=4..16`, `try=0..3`; inner Await `whole=5..15`, `await=5..10`, `operand=11..15` |
| `try value?` | outer prefix Try whose operand is postfix Try | outer `whole=0..10`, `operand=4..10`, `try=0..3`; inner `whole=4..10`, `operand=4..9`, `?=9..10` |
| `try await need?` | one propagating Await whose operand is postfix Try | Await `whole=0..15`, `operand=10..15`, prefix `try=0..3`; inner Try `whole=10..15`, `operand=10..14`, `?=14..15` |
| `await? need?` | one attached propagating Await whose operand is postfix Try | Await `whole=0..12`, `operand=7..12`, attached `?=5..6`; inner Try `whole=7..12`, `operand=7..11`, `?=11..12` |

Parentheses are not required to become a new semantic AST node. Exact `whole`/`operand` ranges plus the formatter precedence rule below are sufficient.

### D7 — Malformed recovery is ordinary grammar recovery

- Strict `try` fails with the existing general expression parse diagnostic at zero-width range `3..3`; the lossless parser emits `TryExpression` with `MissingExpression`/`Operand` at `3..3`.
- Strict `await?` fails at `6..6`; `try await` fails at `9..9`.
- Trivia after a prefix operator moves the insertion point to end-of-fragment; for `try /*x*/` it is `9..9`.
- A bare `value` is valid and MUST NOT be reinterpreted as “postfix Try with missing `?`”. The parser cannot infer an omitted optional operator without a false positive.
- The required missing-operator recovery test therefore enters the existing crate-private `TryExpression` production with its postfix operator slot already selected by parser state and exercises the ordinary `MissingTokenNode`/`MissingToken` event routine. It inserts a zero-width token at `operand.end`. This is a direct typed parser-unit test, not a source scan, source gate, compatibility reader, or public alternate grammar.
- No malformed case creates an executable Try AST when the operator or operand required by the selected production is absent.

### D8 — Formatter and source-facing tooling

No canonicalization between retained spellings is allowed.

- A prefix general Try formats as `try <operand>`.
- A postfix general Try formats as `<operand>?`.
- Await `PrefixTry` formats as `try await <operand>`.
- Await `AttachedQuestion` formats as `await? <operand>` with no trivia between `await` and `?`.
- Formatting may normalize whitespace and layout only. It MUST preserve the source variant and semantic grouping.
- A postfix Try operand is formatted under precedence `100`; lower-precedence operands are parenthesized. Therefore a Try wrapping Await formats as `(await need)?`, never `await need?`.
- An Await operand is formatted under prefix precedence `90`; a postfix Try operand may remain unparenthesized. Therefore Await wrapping Try formats as `await need?`.
- A general prefix Try whose operand is Await MUST parenthesize that operand even though both are prefix expressions, because unparenthesized `try await ...` is reserved for the single-node Await propagation sugar. Thus `Try(PrefixTry, Await(PreserveResult))` formats as `try (await need)`.
- The inspected main has no dedicated production Arcweft formatter owner. This cut MUST NOT create a broad new formatter subsystem solely for Try. The rules above bind any existing expression printer/refactoring path, and the parse-format-parse tests become mandatory in the first production formatter owner that can format these expressions. Tooling in this cut MUST at minimum expose/preserve the typed source variant and MUST NOT rewrite spelling.

## 2. Propagation boundary contract

### 2.1 One lexical frame stack replaces the type-only stack

The existing `expected_returns: Vec<Option<TypeKind>>` is removed atomically. It MUST NOT coexist with the final propagation stack.

The exact sema-owned model is:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PropagationBoundaryKind {
    Function,
    Closure,
    Method,
    Flow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedReturnType {
    Known(TypeKind),
    Unconstrained,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropagationBoundaryEvidence {
    kind: PropagationBoundaryKind,
    declaration: Option<CallableDeclarationId>,
    checked_return: CheckedReturnType,
    header: SourceSpan,
    result: Option<SourceSpan>,
}

impl PropagationBoundaryEvidence {
    pub(crate) fn new(
        kind: PropagationBoundaryKind,
        declaration: Option<CallableDeclarationId>,
        checked_return: CheckedReturnType,
        header: SourceSpan,
        result: Option<SourceSpan>,
    ) -> Self {
        Self { kind, declaration, checked_return, header, result }
    }

    pub const fn kind(&self) -> PropagationBoundaryKind { self.kind }
    pub fn declaration(&self) -> Option<&CallableDeclarationId> { self.declaration.as_ref() }
    pub const fn checked_return(&self) -> &CheckedReturnType { &self.checked_return }
    pub const fn header(&self) -> &SourceSpan { &self.header }
    pub fn result(&self) -> Option<&SourceSpan> { self.result.as_ref() }
    pub fn related(&self) -> &SourceSpan { self.result.as_ref().unwrap_or(&self.header) }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropagationBarrierEvidence {
    owner: SourceSpan,
}

impl PropagationBarrierEvidence {
    pub(crate) fn new(owner: SourceSpan) -> Self { Self { owner } }
    pub const fn owner(&self) -> &SourceSpan { &self.owner }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropagationTargetEvidence {
    Boundary(PropagationBoundaryEvidence),
    GeneratorTerminal(PropagationBarrierEvidence),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReturnPropagationFrame {
    Boundary(PropagationBoundaryEvidence),
    GeneratorTerminal(PropagationBarrierEvidence),
}
```

`TypeChecker` stores exactly one `return_propagation_frames: Vec<ReturnPropagationFrame>`. Existing return-statement checking and new Try/Await propagation query this same stack. This stack is lexical execution context, not an execution-facts table or callable catalog.

### 2.2 Boundary source facts

Existing verified source substrate is reused wherever present:

- Top-level functions use the existing `FunctionSignatureSource`, `HirCallableSignatureSource`, `CallableSource`, source map, and same `CallableDeclarationId`.
- The accepted AW-AH-009.3 callable catalog remains the only project callable catalog and resolver. The checker reads the selected record; it does not publish a second record.
- Agent controller entry bindings resolve to the existing ordinary function record. The entry role does not create a new boundary and does not alter the function’s identity, return type, or source evidence.

Concrete missing source evidence is added minimally:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FlowSignatureSource {
    header: TextRange,
    result: Option<TextRange>,
}

impl FlowSignatureSource {
    pub(crate) const fn new(header: TextRange, result: Option<TextRange>) -> Self {
        Self { header, result }
    }

    pub const fn header(self) -> TextRange { self.header }
    pub const fn result(self) -> Option<TextRange> { self.result }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ClosureExprSource {
    whole: TextRange,
    header: TextRange,
    result: Option<TextRange>,
    body: TextRange,
}

impl ClosureExprSource {
    pub(crate) const fn new(
        whole: TextRange,
        header: TextRange,
        result: Option<TextRange>,
        body: TextRange,
    ) -> Self {
        Self { whole, header, result, body }
    }

    pub const fn whole(self) -> TextRange { self.whole }
    pub const fn header(self) -> TextRange { self.header }
    pub const fn result(self) -> Option<TextRange> { self.result }
    pub const fn body(self) -> TextRange { self.body }
}
```

Both types have crate-private constructors and public value accessors for every field. `Flow` and `HirFlow` retain `FlowSignatureSource`. Existing `Expr::Closure { ... }` gains one `source: ClosureExprSource` field; it is not otherwise redesigned. `ImplMember::Function` gains `signature_source: FunctionSignatureSource` and its accessors; the existing source type is reused rather than copied.

Range definitions:

- Function/method `header`: existing complete signature range, excluding attributes, contracts, body delimiter, and trailing trivia.
- Function/method `result`: exact authored return type, excluding `->` and trivia; absent when omitted.
- Flow `header`: from the `flow` keyword through the last signature token before the body delimiter/colon, excluding visibility/attributes/contracts and trailing trivia.
- Flow `result`: exact authored return type only.
- Closure `whole`: complete closure expression without surrounding trivia.
- Closure `header`: opening pipe through closing pipe and, when present, through the explicit return type; excludes body-leading trivia.
- Closure `result`: exact explicit return type only.
- Closure `body`: complete authored body expression.

Text ranges are converted to `SourceSpan` through the existing HIR source map and source snapshot. No source rescan or synthetic zero span is permitted. A propagation-bearing source-less synthetic HIR fixture must supply a real synthetic `SourceDocument` and typed spans; otherwise it fails readiness rather than emitting an imprecise diagnostic.

### 2.3 Nearest boundary selection

Selection inspects only the top lexical propagation frame. It never skips an inner frame to reach an outer compatible return type.

| Context | Frame pushed | Checked return |
|---|---|---|
| `fn`, `task fn`, `dialogue fn` | `Boundary(Function)` | explicit checked return; omitted return is `Known(Unit)` |
| closure | `Boundary(Closure)` | explicit return, otherwise expected function return when fully known, otherwise `Unconstrained` |
| impl/inherent/trait method body | `Boundary(Method)` | explicit checked return; omitted return is `Known(Unit)` |
| flow body and all flow-owned nested expressions | `Boundary(Flow)` | declared flow return; absent return is `Known(Unit)` |
| Agent controller | no additional frame | body uses its selected ordinary function frame and identical `CallableDeclarationId` |
| ordinary blocks, scopes, loops, dialogue plans, choice bodies | no frame | inherit the current top frame |
| stream function or existing `seq`/`stream`/`source` generator body | `GeneratorTerminal` | propagation stops; no outer return boundary is searched |

When the stack is empty, the target is missing. A `GeneratorTerminal`, `Unconstrained` boundary, wrong return envelope, or non-Result/non-Option return is also target-missing. This contract does not revise generator classification, Stream runtime/wire behavior, terminal encoding, AWBC, save schemas, or runtime plans; it only places a semantic barrier at already typed generator owners.

### 2.4 Try and Await propagation semantics

General Try:

- Operand `Result<T, E_actual>` yields `T` and requires nearest boundary `Result<_, E_expected>`.
- Operand `Option<T>` yields `T` and requires nearest boundary `Option<_>`.
- Result-to-Option, Option-to-Result, and propagation into any other return envelope are target-missing.
- A non-Result/non-Option operand retains the ordinary operand-type error path; it is not reported as target-missing and no boundary comparison is attempted.

Await:

- `AwaitPropagation::PreserveResult` maps `Need<T, E>` to `Result<T, E>` and performs no propagation-boundary lookup.
- `AwaitPropagation::PropagateError` yields `T` and requires nearest boundary `Result<_, E_expected>` compatible with the awaited `E_actual`.
- Non-`Need` operands retain the existing Await operand-type error path.

Prefix and postfix Try have identical semantics. Prefix/attached Await propagation spellings have identical semantics.

### 2.5 Generic substitution and error compatibility

1. The boundary return type and operand type are checked in the same lexical generic environment.
2. All already-known callable/method substitutions and projection normalization are applied before propagation comparison, using the existing resolver/type-checking path.
3. In a generic declaration, the same resolved `GenericParam` in operand and return error positions is compatible. At an instantiated use, the concrete substitution is compared.
4. Generic parameters from different binders MUST be instantiated/alpha-resolved by the existing resolver before they reach this check; textual name equality alone is not a cross-binder rule.
5. After resolution, compatibility is directional: `E_expected.accepts(&E_actual)` using the existing `TypeKind::accepts` behavior, including the existing unique Choice-branch rule.
6. If either side already failed type resolution, propagation diagnostics are suppressed to avoid a cascade. An unconstrained closure boundary is target-missing; it is not inferred from a Try/Await expression.
7. No implicit `From`, `Into`, trait conversion, `ArcError` conversion, Option/Result conversion, or function-name conversion participates.
8. An explicit typed expression such as `map_err`, `context`, `ok_or`, or another ordinary conversion participates only because it changes the operand’s checked error/envelope before Try/Await is evaluated.

## 3. Typed diagnostics

### 3.1 Exact payload types

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TryPropagationOperand {
    Result { actual_error: TypeKind },
    Option,
}

pub enum TypeCheckErrorKind {
    // existing variants
    AwaitPropagationTargetMissing {
        actual_error: TypeKind,
        operator: SourceSpan,
        target: Option<PropagationTargetEvidence>,
    },
    AwaitErrorMismatch {
        expected: TypeKind,
        actual: TypeKind,
        operator: SourceSpan,
        boundary: PropagationBoundaryEvidence,
    },
    TryPropagationTargetMissing {
        operand: TryPropagationOperand,
        operator: SourceSpan,
        target: Option<PropagationTargetEvidence>,
    },
    TryErrorMismatch {
        expected: TypeKind,
        actual: TypeKind,
        operator: SourceSpan,
        boundary: PropagationBoundaryEvidence,
    },
}
```

The variants are added to the existing `TypeCheckErrorKind`; there is no parallel error catalog. Repeated construction lives in inherent `TypeCheckError` constructors, not scattered local helpers or extension traits.

### 3.2 Stable codes and labels

| Kind | Stable code | Primary range | Related range |
|---|---|---|---|
| Await target missing | `sema.await.propagation_target_missing` | exact Await propagation operator | incompatible/unconstrained boundary `result`, else `header`; generator owner for barrier; none when no owner |
| Await error mismatch | `sema.await.error_mismatch` | exact Await propagation operator | enclosing boundary `result`, else `header` |
| General Try target missing | `sema.try.propagation_target_missing` | exact general Try operator | same target-evidence rule |
| General Try error mismatch | `sema.try.error_mismatch` | exact general Try operator | enclosing boundary `result`, else `header` |

Exact operator selection:

- Await `try await`: `AwaitPropagationSource::PrefixTry.try_keyword`.
- Await `await?`: `AwaitPropagationSource::AttachedQuestion.question`.
- General prefix `try expr`: `TryOperatorSource::PrefixTry.try_keyword`.
- General postfix `expr?`: `TryOperatorSource::PostfixQuestion.question`.

Mismatch payloads carry typed `expected` and `actual` error types. Target-missing payloads carry the actual Await error or general Try operand family plus typed target evidence. Diagnostic construction MUST use the typed source records and propagation frame; it MUST NOT inspect source spelling or callable names.

The primary is always the smallest operator token. The related label is always the exact return-type range when authored and the exact header otherwise. Message prose may evolve without changing code, typed payload, primary, or related evidence; tests assert those structured facts rather than depending on incidental sentence wording.

## 4. Accepted semantic join

The join is one-way and typed:

```text
syntax TryExpr/AwaitExpr source
        |
        v
HIR retains typed expression + function/flow/method/closure source facts
        |
        v
existing source map converts TextRange -> SourceSpan
        |
        +------------------------------+
        |                              |
existing CallableDeclarationId/catalog|  lexical closure/flow/method facts
        |                              |
        +-------------> TypeChecker.return_propagation_frames
                                      |
                                      v
                           Try/Await compatibility check
                                      |
                                      v
                         existing TypeCheckErrorKind/Diagnostic
```

The callable identity/catalog, shared resolver, and fixed-point effect analysis are unchanged. No callable record is republished, no execution-facts table is added, and no entry-role/name-specific resolver is introduced.

## 5. Owner migration/deletion rule

`OWNER_MIGRATION_INVENTORY.csv` is normative. Every listed exhaustive match migrates in the same atomic Try-shape cut. The old variant and dialogue-specific reader are deleted rather than bridged. Source-facing owners preserve `TryExprSource`; semantic/runtime owners use `operand()`/`into_parts()` and ignore spelling. Existing Await consumers continue to consume the current node.

A compiler-clean cut MUST contain zero remaining use of the removed Rust pattern because the compiler, not a source scan, exposes all stale matches. The acceptance suite MUST NOT add a repository text search to prove that fact.

## 6. Implementation order

The edit order is fixed and mirrors the request:

1. Freeze this prefix/postfix decision and the exact Try/source shapes.
2. Add exact Try parser/source-map tests first; do not change `AwaitExpr`.
3. Within the same atomic compile-clean cut, replace the Try node and migrate syntax/HIR/sema/runtime-plan/verifier/Agent/CLI/tooling consumers; delete the dialogue dual reader and Try source rescanning.
4. Add the typed function/flow/method/closure source facts and replace `expected_returns` with the one source-backed propagation-frame stack.
5. Add Try/Await target-missing and mismatch checks plus the four structured diagnostics.
6. Only after grouping tests are green, update any expression printer/refactoring/formatter and source-facing tooling. Do not create a broad formatter subsystem if none exists.
7. Run the full final validation and structural audit. Prefix Try is retained, so removed-spelling migration/rejection is not applicable; instead prove both canonical spellings, one AST, one parser path, and no compatibility behavior through typed APIs.

`IMPLEMENTATION_CUTS.md` defines the reviewable compile-clean cuts and commands.

## 7. Explicit non-goals and prohibited paths

This contract MUST NOT:

- redesign the existing Await types without a newly demonstrated defect;
- add compatibility shims, aliases, dual AST/CST readers, deprecated constructors, source-text operator recovery, permanent legacy diagnostics, or source gates;
- add CSS, Takumi, or any presentation-style path;
- revise Stream runtime/wire design, generator execution/classification, AWBC versioning, save schemas, callable catalog design, function-role deletion, or fixed-point effect analysis;
- route propagation into a generator/Stream terminal;
- create a parallel callable catalog, resolver, execution-facts table, or role-name special case;
- infer a missing optional postfix `?` from a valid bare operand.

## 8. Completion condition

Implementation is complete only when every `required` row in `TEST_MATRIX.csv` passes at one reviewable commit, all compile-clean cuts have their stated validation evidence, the structural audit has been recorded, and the final code contains one typed Try node, one general expression grammar path, one return-propagation frame stack, one callable identity/catalog, and the four stable diagnostic codes.

**OPEN_QUESTIONS=0**
