# Grammar Summary

This is a compact summary of the current Arcweft surface grammar. It is intentionally canonical: removed migration forms are not part of this grammar.

## Lexical conventions

```text
Ident        := /[A-Za-z_][A-Za-z0-9_]*/
EntityRef    := '#' Ident ('.' Ident)* | '#<' EntityBody '>'
String       := '"' ... '"'
Newline      := '\n'
Comment      := '#' TextToEndOfLine
```

`#` is reserved for entity references. `@` remains available for attributes and scenario commands such as `@bg`, but `choice` is a flow item and is written without `@`.

## Module items

```text
Source       := ModuleDecl? UseDecl* Item*
ModuleDecl   := 'mod' Path
UseDecl      := 'use' Path ('::' UseTree)?
Visibility   := 'pub'?

Item         :=
    FlowDecl
  | FragmentDecl
  | FunctionDecl
  | StateDecl
  | ReducerDecl
  | ViewDecl
  | ParserDecl
  | HookDecl
  | MemoDecl
  | DialogueDefaultsDecl
  | TypeDecl
```

## Flow and fragments

```text
FlowDecl     := Visibility 'flow' EntityRef Ident? ParamList? ReturnType? Contract* FlowBody
FragmentDecl := Visibility 'fragment' EntityRef Ident? (':' Type)? Contract* FlowBody
FlowBody     := '{' FlowItem* '}'

FlowItem     :=
    LetStmt
  | ControlStmt
  | ScenarioCommand
  | DialogueLine
  | ChoiceBlock
  | AwaitExpr
  | ScopeStmt
  | ExprStmt

ScenarioCommand := '@' Ident ScenarioArgs?
```

## Dialogue and line plans

```text
DialogueLine :=
    SpeakerRef CallArgs? ':' DialogueText
  | Callee CallArgs? '[' DialogueContent ']'

CallArgs       := '(' NamedArg (',' NamedArg)* ','? ')'
NamedArg       := Ident '=' Expr
DialogueText   := TextUntilLineEnd | Newline IndentedText
DialogueContent:= TextAndDialogueTags*

LinePlanAttach := 'with' Block | 'with' ':' Newline IndentedItems
LinePlanItem   := LetStmt | TimedCue | CancelRule | OutStmt | ScopeStmt | ExprStmt
OutStmt        := 'out' Expr
```

`with:` is indentation sugar for `with { ... }`. A bare `{ ... }` after a dialogue content block is a normal lexical scope, not a line plan.

## Choice

```text
ChoiceBlock := 'choice' EntityRef? '{' ChoiceArm* '}'
ChoiceArm   := EntityRef String ChoiceCondition? '->' EntityRef
ChoiceCondition := 'if' Expr
```

`choice` displays a choice block and advances the current flow to the selected arm target. It is not an expression form; value-returning choice selection is intentionally left for a separate future construct.

## Hooks

```text
HookDecl   :=
    Visibility 'hook' EntityRef
    HookTarget
    HookPhase
    HookCheck?
    HookWhen?
    HookPriority?
    HookOnce?
    HookEffects?
    BlockExpr

HookTarget := 'on' HookTargetExpr
HookTargetExpr := EntityRef | 'state' StatePath | 'signal' EntityRef | 'query' Type WhereClause?
HookPhase  := 'phase' Ident
HookCheck  := 'check' CheckPolicy
HookWhen   := 'when' Expr
HookPriority := 'priority' Int
HookOnce   := 'once' OncePolicy?
HookEffects:= 'effects' '{' Ident (',' Ident)* ','? '}'
```

## Blocks and scopes

```text
Block          := '{' BlockItem* FinalExpr? '}'
ExprBlock      := Block
ScopeStmt      := Block
StatementBlock := Block | ':' Newline IndentedItems
LabeledBlock   := Label? Block
Label          := '\'' Ident ':'
BlockItem      := LetStmt | LetElseStmt | ExprStmt | ControlStmt | ScenarioStmt | ScopeStmt
```

In expression position, a block's final expression is its value. In statement position, a bare block creates a lexical scope and does not export a value; a non-`Unit` final expression must be discarded explicitly with `;` or `let _ = ...`.

## Statement / expression list

```text
BlockExpr      := Block
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
BreakStmt    := 'break' LabelRef? Expr?
ContinueStmt := 'continue' LabelRef?
OutStmt      := 'out' LabelRef? Expr
LabelRef     := '\'' Ident
```

`break expr` is allowed only in `loop`.
`out` is allowed only in line-plan, cue-block, and content-scope continuations.

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
TryExpr      := 'try' Expr
AwaitExpr    := 'await' Expr AwaitPendingBlock?
TryAwaitExpr := 'try' 'await' Expr AwaitPendingBlock?
TryAwaitExpr := 'await?' Expr AwaitPendingBlock?
AwaitPendingBlock := 'with' ':' Newline AwaitCase+
                   | 'with' '{' AwaitCase* '}'
```

`expr?` is the ordinary Rust-like postfix try operator for `Result` and `Option` expressions. `await` returns `Result<T, E>`. `try await` returns `T` and propagates errors using the same semantics as `(await ...)?`.
`await? expr with:` is sugar for `try await expr with:`.
The indentation form `with:` is syntax sugar for the canonical brace form `with { ... }`. Formatters may keep `with:` for scenario-like readability, but lowering should treat it as brace-block syntax.

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
