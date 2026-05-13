# Grammar Summary: Control Flow and Patterns

This is a grammar summary for the updated control-flow subset.

## Statement / expression list

```text
BlockExpr      := '{' Item* FinalExpr? '}'
StatementBlock := '{' Item* '}' | ':' Newline IndentedItems
Item           := LetStmt | LetElseStmt | ExprStmt | ControlStmt | ScenarioStmt
ExprStmt       := Expr (';')?
```

Newlines separate statements. Semicolons are optional separators and explicit value-discard markers.

## If

```text
IfExpr := 'if' Expr BlockExpr ('else' (IfExpr | BlockExpr))?
IfLet  := 'if' 'let' Pattern '=' Expr Guard? BlockExpr ('else' (IfExpr | BlockExpr))?
Guard  := 'when' Expr
```

Value-producing `if` requires `else`.

## Match

```text
MatchExpr := 'match' Expr '{' MatchArm* '}'
MatchArm  := Pattern Guard? '=>' (Expr | BlockExpr)
Guard     := 'when' Expr
```

## Loop / while / for

```text
LoopExpr  := 'loop' BlockExpr
WhileStmt := 'while' Expr StatementBlock
WhileLet  := 'while' 'let' Pattern '=' Expr Guard? StatementBlock
ForStmt   := 'for' Pattern 'in' Expr StatementBlock
```

`loop` may be value-producing through `break expr`. `while` and `for` return `Unit`.

## Break / continue

```text
BreakStmt    := 'break' Expr?
ContinueStmt := 'continue'
```

`break expr` is allowed only in `loop`.

## Let and let-else

```text
LetStmt     := 'let' Pattern '=' Expr
LetElseStmt := 'let' Pattern '=' Expr 'else' DivergingBlock
```

`DivergingBlock` must leave the current continuation.

## Patterns

```text
Pattern :=
    '_'
  | Ident
  | 'mut' Ident
  | Literal
  | EntityRef
  | TuplePattern
  | RecordPattern
  | VariantPattern
  | ListPattern
  | Ident '@' Pattern

TuplePattern  := '(' Pattern (',' Pattern)* ')'
RecordPattern := TypePath? '{' FieldPattern* '..'? '}'
FieldPattern  := Ident | Ident ':' Pattern
VariantPattern:= ('.' Ident | TypePath '::' Ident) VariantPayload?
ListPattern   := '[' Pattern* RestPattern? ']'
RestPattern   := '..' Ident?
```

Examples:

```awft
.Some(route)
.Err(e)
.ChoiceSelected { id }
TruckResult { score, rank, .. }
ev @ .ChoiceSelected { id }
[first, ..rest]
```

## Try operator and await

```text
TryExpr      := Expr '?'
AwaitExpr    := 'await' Expr AwaitPendingBlock?
TryAwaitExpr := 'try' 'await' Expr AwaitPendingBlock?
TryAwaitExpr := 'await?' Expr AwaitPendingBlock?
AwaitPendingBlock := 'with' ':' Newline AwaitCase+
                   | 'with' '{' AwaitCase* '}'
```

`expr?` is the ordinary Rust-like postfix try operator for `Result` and `Option` expressions. `await` returns `Result<T, E>`. `try await` returns `T` and propagates errors using the same semantics as `(await ...)?`.
`await? expr with:` is sugar for `try await expr with:`.
The brace form is syntax sugar for the indentation form; formatters should prefer `with:` in hand-written code.

Only the following await grouping is rejected for ambiguity:

```text
'await' Expr '?' AwaitPendingBlock
```

Use `try await Expr AwaitPendingBlock`, `await? Expr AwaitPendingBlock`, or the explicit parenthesized form instead.

## Never

```text
NeverType := '!'
DivergingExpr := ReturnStmt | GotoStmt | BreakStmt | ContinueStmt | PanicStmt | FailStmt
```

`!` coerces to any expected type. Diagnostics should normally say "this branch never returns" rather than exposing bottom-type theory to non-expert users.
