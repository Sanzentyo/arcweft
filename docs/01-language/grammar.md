# Grammar Summary

This is a compact summary of the current Arcweft surface grammar. It is intentionally canonical: removed migration forms are not part of this grammar.

## Lexical conventions

```text
Ident        := /[A-Za-z_][A-Za-z0-9_]*/
IdentPath    := Ident ('::' Ident)*
EntityRef    := '@' Ident ('.' Ident)* | '@<' EntityBody '>'
EntityRefSyntax := EntityRef | FamilyRelativeEntityRef
FamilyRelativeEntityRef := '@' Ident ':' DotRun Ident ('.' Ident)*
RelativeId   := DotRelativeId | SuperRelativeId
DotRelativeId:= '@' DotRun Ident ('.' Ident)*
DotRun       := '.'+
SuperRelativeId := '@super' ('.' 'super')* '.' Ident ('.' Ident)*
String       := '"' ... '"'
RawString    := 'r' '#'* '"' RawText '"' '#'* 
Newline      := '\n'
Comment      := '//' TextToEndOfLine
DocComment   := '///' MarkdownTextToEndOfLine
Attribute    := '#[' AttributePath AttributeArgs? ']'
```

`@` is reserved for entity references. `#` appears only as part of the `#[...]`
attribute opener; it is not a comment introducer and not an entity-ref sigil.
`///` forms Markdown documentation comments; consecutive `///` lines attach to
the next documentable declaration or field. Scenario operations such as
background changes are ordinary effectful function calls, not `@` commands.

`RelativeId` is accepted only in ID-bearing contexts such as dialogue line IDs,
choice IDs, option IDs, and text-key overrides. ID-bearing contexts may also
accept family-relative spelling such as `@say:.greeting` or `@choice:.first`,
but hand-written code should prefer the shorter `@.greeting` form there. It is
not a general entity reference; write `goto @flow.opening.next` or the
recommended family-relative `goto @flow:.next`, not `goto .next`. In general
entity-reference contexts, relative references must include an entity family:
`@flow:.next`, `@frag:.intro`, `@asset:.room`, `@textbox:.side`.
`@.suffix` resolves in the current ID scope. Each extra dot walks one parent ID
scope outward: `@..suffix` is one parent, `@...suffix` is two parents, and so
on. The explicit spelling `@super.suffix` / `@super.super.suffix` is accepted
as the readable equivalent. These forms are ID-context-only relative IDs, not
general expression-level entity references. Bare `.suffix` and bare `..suffix`
are not part of the core grammar; `..` already appears in range and
rest-pattern syntax. Deep dot runs such as `@...suffix` are accepted for
generated or dense code, but authoring tools should prefer
`@super.super.suffix`.

## Literals and primitive spellings

```text
BoolLiteral      := 'true' | 'false'
UnitLiteral      := '()'
IntLiteral       := DecimalInt | HexInt | OctInt | BinInt
IntSuffix        := 'i8' | 'i16' | 'i32' | 'i64' | 'i128'
                  | 'u8' | 'u16' | 'u32' | 'u64' | 'u128'
                  | 'isize' | 'usize'
FloatLiteral     := Digits '.' Digits? Exponent? FloatSuffix?
                  | Digits Exponent FloatSuffix?
FloatSuffix      := 'f32' | 'f64'
UnitNumber       := Number UnitSuffix
UnitSuffix       := 'ns' | 'us' | 'ms' | 's' | 'min' | 'h'
                  | '%' | 'px' | 'pt' | 'em' | 'rem' | 'vw' | 'vh'
                  | 'deg' | 'rad' | 'turn'
                  | 'db' | 'lufs' | 'bpm' | 'bars'
```

Examples:

```awft
10i32
2.0f32
100pt
300ms
85%
-6db
"#fff"
```

Unsuffixed numeric literals require an expected type from annotation or
signature. There is no default `i32`/`f64` fallback, and there are no concrete
`int`, `uint`, `float`, or `Number` primitive type names.

## Module items

```text
Source       := ModuleDecl? UseDecl* Item*
ModuleDecl   := 'mod' ModulePath
UseDecl      := Visibility? ('lazy' | 'eager')? 'use' ModulePath ('::' UseTree)?
Visibility   := 'pub'?
ModulePath   := ('crate' '::' | 'self' '::' | 'super' '::' | 'parent' '::')? IdentPath

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
FlowDecl     := Visibility 'flow' EntityRef Ident? GenericParams? ParamGroup* ReturnType? Contract* FlowBody
FragmentDecl := Visibility 'fragment' EntityRef Ident? (':' Type)? Contract* FlowBody
FlowBody     := '{' FlowItem* '}'

FlowItem     :=
    LetStmt
  | LetElseStmt
  | ControlStmt
  | DialogueLine
  | ChoiceBlock
  | IfExpr
  | MatchExpr
  | LoopExpr
  | WhileStmt
  | WhileLet
  | ForStmt
  | AwaitExpr
  | ScopeStmt
  | ExprStmt

NamedScope      := 'scope' Ident? BlockExpr
ScopeStmt       := NamedScope | Block
ScopeExpr       := 'scope' Ident? BlockExpr

StagingExpr     := StagingSet | StagingRef | StagingClear
StagingSet      := ('bg' | 'show') CallArgs
StagingRef      := 'ref' ('bg' | 'show') CallArgs
StagingClear    := 'clear' 'bg' CallArgs | 'hide' CallArgs
```

`crate`, `self`, and `super` are canonical module-path roots. `parent` is a
reserved alias for `super`; formatters should normalize it to `super`.

## Dialogue and line plans

```text
DialogueLine :=
    SpeakerRef CallArgs? ':' DialogueText
  | Callee CallArgs? '[' DialogueContent ']'

CallArgs       := '(' NamedArg (',' NamedArg)* ','? ')'
NamedArg       := Ident '=' Expr
RelativeLineId := RelativeId | FamilyRelativeEntityRef
LineOption     := 'id' '=' (EntityRef | RelativeId | FamilyRelativeEntityRef)
                | 'text_key' '=' (EntityRef | RelativeId | FamilyRelativeEntityRef)
                | 'voice' '=' Expr
                | 'window' '=' EntityRef
                | Ident '=' Expr
                | 'source_locale' '=' Locale
                | 'hooks' '=' ListExpr
                | 'style' '=' Expr
DialogueText   := TextUntilLineEnd | Newline IndentedText
DialogueContent:= TextAndDialogueTags*

LinePlanAttach := 'with' Block
                | 'with' ':' Newline IndentedItems
                | 'with' ':' LinePlanItem
                | FlatWith
FlatWith       := '=== with ===' FlatLinePlanItem* '=== /with ==='
LinePlanItem   := InitBlock | ThreadBlock | DeferBlock | OnBlock | FinallyBlock
                | LetStmt | TimedCue | CancelRule | OutStmt | ScopeStmt | ExprStmt
InitBlock      := 'init' Block | 'init' ':' Newline IndentedItems
ThreadBlock    := 'thread' ThreadModifier? Ident? Block
                | 'thread' ThreadModifier? Ident? ':' Newline IndentedItems
ThreadModifier := 'detached'
DeferBlock     := 'defer' Block | 'defer' ':' Newline IndentedItems
OnBlock        := 'on' Expr Block | 'on' Expr ':' Newline IndentedItems
FinallyBlock   := 'finally' Block | 'finally' ':' Newline IndentedItems
OutStmt        := 'out' Expr
FlatLine       := '=== line' DialogueCallee '===' DialogueContent FlatWith? '=== /line ==='
FlatThread     := '=== thread' ThreadModifier? Ident? '===' Item* '=== /thread ==='
FlatDefer      := '=== defer ===' Item* '=== /defer ==='
FlatScope      := '=== scope' Ident? '===' Item* '=== /scope ==='
```

`with:` is indentation sugar for `with { ... }`. Flat fences are authoring
sugar and require explicit closing fences. A bare `{ ... }` after a dialogue
content block is an unnamed `scope` statement, not a line plan.

In generic expression parsing, `target[expr]` is always an index/postfix
expression. Dialogue content brackets are recognized only in dialogue-capable
contexts such as flow items and line-result bindings where the callee is known
to be a dialogue callee.

## Choice

```text
ChoiceExpr  := 'choice' ChoiceId? ChoiceBody ChoicePlan?
ChoiceBody  := '{' ChoiceItem* '}' | ':' Newline IndentedItems
ChoiceItem  := LetStmt | IfExpr | MatchExpr | ForStmt | OptionItem | OptionForSugar | ChoiceArm

OptionItem  := 'option' OptionId OptionBody
ChoiceId    := EntityRef | RelativeId | FamilyRelativeEntityRef
OptionId    := StaticOptionId | Expr
StaticOptionId := EntityRef | RelativeId | FamilyRelativeEntityRef
OptionBody  := '{' OptionField* '}' | ':' Newline IndentedItems
OptionField := 'label' '=' Expr
             | 'label' '(' 'id' '=' (EntityRef | RelativeId | FamilyRelativeEntityRef) ')' '=' Expr
             | 'value' '=' Expr
             | 'visible' '=' Expr
             | 'enabled' '=' Expr
             | 'order' '=' Expr
             | 'hotkey' '=' Expr
             | 'ui' Block
             | 'select' Block
             | LetStmt

OptionForSugar := 'option' Pattern 'in' Expr OptionBody
ChoiceArm      := StaticOptionId String ChoiceArmCondition? ChoiceArmAction
ChoiceArmCondition := 'if' Expr
ChoiceArmAction := '->' EntityRef | '=>' Expr

ChoicePlan := 'with' Block | 'with' ':' Newline IndentedItems
```

`choice` displays a choice UI and may also be used as an expression. A choice body is a lexical scope. `option` creates an option candidate. Inline arm `if` is enabled-state sugar; wrapping an option in a block `if` controls whether the option exists. `visible = expr` controls rendering. `ui { ... }` is propagated to rendering, accessibility, test, and Agent observation.

`-> target` is sugar for `select { goto target }`. `=> value` is sugar for `select { out value }`.
Compact choice arms use only static option IDs because their syntax must be
stable enough for localization and registry extraction. Full `option ... { }`
blocks may use an expression ID for dynamic data-driven choices, or the
`option pattern in expr { id = ... }` sugar when the ID is computed inside the
option body.

Relative choice IDs resolve through the current flow and named scope path.
Relative option IDs resolve under the current choice ID.
When the named scope path is empty, the scope segment is omitted from the
normalized ID.

Relative dialogue line IDs and text-key overrides resolve through the current
flow, current speaker, and named scope path:

```text
id=@.suffix
  -> @say.{flow}.{speaker}.{scope_path}.{suffix}
  -> @say.{flow}.{speaker}.{suffix} when scope_path is empty
```

```text
scope dream { choice @.first { @.listen "聞いてみる" -> @flow.alice_intro } }

choice @.first -> @choice.opening.dream.first
@.listen       -> @choice.opening.dream.first.listen

choice @.first outside a scope -> @choice.opening.first
```

## Hooks

```text
DialogueDefaultsDecl :=
    Visibility? 'dialogue' 'defaults' (EntityRef | RelativeId)? Block

HookDecl   :=
    Visibility 'hook' EntityRef
    HookTarget
    HookPhase
    HookWhen?
    HookPriority?
    HookOnce?
    HookEffects?
    BlockExpr

HookTarget := 'on' HookTargetExpr
HookTargetExpr := EntityRef | 'state' StatePath | 'signal' EntityRef | 'query' Type WhereClause?
HookPhase  := 'phase' Ident
HookWhen   := 'when' Expr
HookPriority := 'priority' Int
HookOnce   := 'once'
HookEffects:= 'effects' Expr (',' Expr)*
```

`check` is not part of the canonical hook header. Use `when` for conditions.
Dialogue defaults preserve structured assignment expressions for later style,
window, voice, hook, and localization lowering.

## Functions

```text
FunctionDecl  := Visibility? FunctionKind Ident GenericParams? ParamGroup+ ReturnType? WhereClause? Contract* Block
FunctionKind  := 'fn' | 'task fn' | 'dialogue fn' | 'stream fn'
GenericParams := '<' GenericParam (',' GenericParam)* ','? '>'
GenericParam  := Lifetime | IdentPath
ParamGroup    := '(' Param (',' Param)* ','? ')'
Param         := DocComment* Pattern ':' Type | 'self' | '&self' | '&mut self' | 'mut self'
ReturnType    := '->' Type
WhereClause   := 'where' WherePredicate (',' WherePredicate)* ','?
WherePredicate:= Type ':' Type ('+' Type)*
```

Multiple `ParamGroup` entries are curried parameter groups and are preserved as
separate syntax groups. Unexpected tokens after the return type or where clause
are syntax errors.

## Blocks and scopes

```text
Block          := '{' BlockItem* FinalExpr? '}'
ExprBlock      := Block
ScopeStmt      := 'scope' Ident? BlockExpr | Block
ScopeExpr      := 'scope' Ident? BlockExpr
StatementBlock := Block | ':' Newline IndentedItems
LabeledBlock   := Label? Block
Label          := '\'' Ident ':'
BlockItem      := LetStmt | LetElseStmt | ExprStmt | ControlStmt | ScenarioStmt | ScopeStmt
```

In expression position, a block's final expression is its value. `scope { ... }` is the bare scope form: sugar for `scope name { ... }` with the `name` part omitted. In statement position, a bare block `{ ... }` is a further sugar layer for that unnamed `scope { ... }`, creates a lexical scope, and does not export a value; a non-`Unit` final expression must be discarded explicitly with `;` or `let _ = ...`.
`scope name { ... }` behaves like a lexical block and also contributes `name` to relative line, text-key, choice, and option ID generation inside the block. `scope { ... }` is the same construct with the name omitted. Unnamed scopes do not add an ID segment. In expression position, both `scope name { ... }` and `scope { ... }` return the final expression just like `{ ... }`.
Only ID-bearing constructs inside the named scope use the scope path for ID generation; the scope expression's own value is just the final expression.

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
  | WholeBindingPattern

WholeBindingPattern := Ident NonBindingPattern
NonBindingPattern  := Literal | EntityRef | TuplePattern | RecordPattern | VariantPattern | ListPattern
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
ev .ChoiceSelected { id }
[first, ..rest]
```

Whole-pattern binding uses `Ident Pattern` only where the second token clearly
starts a non-binding pattern, such as a variant, tuple, record, list, literal, or
entity reference. `name @ pattern` is not part of Arcweft grammar.

## Expression operators and postfixes

```text
PostfixExpr := PrimaryExpr ('?' | CallArgs | '.' Ident CallArgs? | '[' Expr ']')*
PrefixExpr  := ('!' | '-') PrefixExpr | AwaitExpr | PostfixExpr
BinaryExpr  := PrefixExpr BinaryOp PrefixExpr
BinaryOp    := '*' | '/' | '%' | '+' | '-' | ComparisonOp | 'in' | '&&' | '||' | '|>' | '=>'
ComparisonOp:= '==' | '!=' | '>=' | '<=' | '>' | '<'
RangeExpr   := Expr? ('..' | '..=') Expr?
```

Field access such as `state.affection` is structured as a field expression.
Type and module paths use `::`; public IDs use the `@flow.opening` entity form.

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
NeverType := '!' | 'Never'
DivergingExpr := ReturnStmt | GotoStmt | BreakStmt | ContinueStmt | PanicStmt | FailStmt
```

`!` coerces to any expected type. Diagnostics should normally say "this branch never returns" rather than exposing bottom-type theory to non-expert users.
