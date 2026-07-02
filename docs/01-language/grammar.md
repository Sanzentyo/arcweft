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
also accepted in declaration ID positions whose family is known from the
declaration keyword, such as `flow @.opening`, `flow @flow:.opening`, and
`character @.alice`. Empty declaration markers `@.` and `@family:.` are also
accepted when a declaration name follows them, so `flow @. opening` and
`character @. alice Alice` use that following name as the family-local suffix.
These normalize to the declaration family, and named flow declarations without
an explicit ID use the same implicit ID as `@.name`. It is
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

## Trait and impl substrate

Seq08.1 adds Rust-like DSL `trait` and `impl` syntax as the canonical type
abstraction surface. `protocol` remains reserved for host, wire, and Agent
protocol concepts.

```text
TraitDecl := Visibility? 'trait' Ident GenericParams? SuperTraits? WhereClause? TraitBody
SuperTraits := ':' TraitBound ('+' TraitBound)*
TraitBody := '{' TraitMember* '}'
TraitMember := AssociatedTypeReq ';'? | FnSignature ';'? | FnSignature Block

AssociatedTypeReq := 'type' Ident GenericParams? ('=' Type)?

ImplDecl := Visibility? 'impl' GenericParams? (TraitBound 'for')? Type WhereClause? ImplBody
ImplBody := '{' ImplMember* '}'
ImplMember := AssociatedTypeAssign ';'? | FnSignature Block | FnSignature ';'?
AssociatedTypeAssign := 'type' Ident GenericParams? '=' Type

GenericParam := Lifetime | Ident (':' TraitBound ('+' TraitBound)*)?
WherePredicate := Type ':' TraitBound ('+' TraitBound)*
TraitBound := TypePath GenericTraitBoundArgs?
GenericTraitBoundArgs := '<' TraitBoundArg (',' TraitBoundArg)* ','? '>'
TraitBoundArg := Type | Ident '=' Type
ProjectionType := TypePath '::' Ident
```

Associated type defaults, GAT-like associated type constructors, and default
method bodies are preserved by parsing but rejected by semantic analysis until
later sequence slices define their execution and coherence rules.

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

```arcw
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
UseDecl      := Visibility? 'use' ModulePath ('::' UseTree)?
Visibility   := 'pub'?
ModulePath   := ('crate' '::' | 'self' '::' | 'super' '::' | 'parent' '::')? IdentPath

Item         := OuterAttr* ItemDecl
ItemDecl     :=
    FlowDecl
  | FragmentDecl
  | FunctionDecl
  | SourceDecl
  | StateDecl
  | ReducerDecl
  | ViewDecl
  | ParserDecl
  | HookDecl
  | MemoDecl
  | DialogueDefaultsDecl
  | AssetDecl
  | ImageDecl
  | TypeDecl
OuterAttr    := '#[' AttrPath AttrArgs? ']'
InnerAttr    := '#![' AttrPath AttrArgs? ']'
AttrPath     := Ident ('::' Ident)*
AttrArgs     := '(' AttrToken* ')'
```

Outer attributes attach to the following item. They are not standalone module
items. Inner attributes attach to the source file or enclosing block when that
surface exists. Source-file inner attributes must appear in the source header,
before documentation comments, outer attributes, `mod`/`use` declarations, and
items; misplaced inner attributes are diagnostics and are not applied
retroactively. `#[allow(...)]` controls lint suppression; `#[generated]` and
`#![generated(...)]` mark generated or fully elaborated source and do not by
themselves silence unrelated diagnostics.
Inner and outer attributes inside flow bodies are currently diagnostics rather
than statement syntax; future block-scope attribute surfaces must add explicit
AST attachment instead of falling through as raw statements.

Asset declarations use the ordinary entity declaration surface:

```text
AssetDecl := Visibility? 'asset' DeclIdentity AssetBody?
ImageDecl := Visibility? 'image' DeclIdentity ImageObjectBody?
```

The declaration establishes the `asset` entity id. Authored asset references
should prefer family-relative spelling such as `@asset:.bg.room`; this omits
the default family from the id path while retaining the explicit asset
reference anchor. Fully qualified references such as `@asset.bg.room` remain
available for generated surfaces, manifest/tooling output, stored public-id
roundtrips, and external interfaces that need the stored public id verbatim.
They are not the recommended spelling for ordinary hand-authored asset
references.
Image payload packaging is still handled by the bundle image asset table, which
records encoded files and decoded metadata.

`image` declarations establish stable presentation-object ids such as
`@image.sample.pulse_sprite`. Their bodies use the same flat fields as bounded
`image(asset = ..., ...)` calls: `asset`, `target`, `layer`, `x`, `y`, `width`,
`height`, `fit`, `alignment.*`, `opacity`, `playback.*`, `transform.*`,
`depth`, `enabled`, `visible`, `action`, `param.*`, and `proxy.*`.
`image(@image.id)` lowers through the same semantic `ImagePresentationObject`
path as an inline bounded `image(...)` call; it is not a renderer adapter or
compatibility surface.

## Flow and fragments

```text
FlowDecl     := Visibility? 'flow' DeclIdentity GenericParams? ParamGroup* ReturnType? Contract* FlowBody
FragmentDecl := Visibility? 'fragment' DeclIdentity (':' Type)? Contract* FlowBody
DeclIdentity := Ident | EntityRef | EntityRef Ident
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

StagingExpr     := ExprStmt
StagingSet      := ('bg' | 'show') CallArgs
StagingRef      := ('bg' | 'show') '.ref' CallArgs
StagingClear    := ('bg' | 'show') '.clear' CallArgs | 'hide' CallArgs
```

`crate`, `self`, and `super` are canonical module-path roots. `parent` is a
reserved alias for `super`; formatters should normalize it to `super`.
`lazy use` and `eager use` are not part of the grammar. `use` introduces names
only; build demand and content availability are compiler and packaging policy.

Declaration identities have a hand-written canonical surface and a generated
surface. Hand-written flows should use either `flow opening(...)` or
`flow @flow.opening(...)`. The fully elaborated spelling
`flow @flow.opening opening(...)` is accepted for generated source and
roundtrips, but hand-written source reports `style::redundant_decl_identity`
unless it is covered by `#[allow(style::redundant_decl_identity)]` or
`#[generated]`. A mismatch such as `flow @flow.opening start(...)` reports
`identity::decl_binding_mismatch`; it is not a style warning.

Source and entity declarations follow the same principle. In declaration
headers, the keyword supplies the default entity family, so authored code should
omit that family prefix. Prefer
`source http_requests: Source<T, E>` or
`source @source.http_requests: Source<T, E>` over
`source @source.http_requests http_requests: ...`. Prefer
`pub character alice { display = "Alice" }`,
`asset bg_room { source = file("images/room.webp") }`, or
`content chapter_two { roots = [@flow.chapter_two] }` for hand-written source.
Fully qualified forms such as `pub character @character.alice { ... }`,
`asset @asset.bg_room { ... }`, and
`content @content.chapter_two { ... }` are accepted but are generated or
fully elaborated surfaces rather than the recommended authoring form. Avoid
putting display names or aliases in declaration headers.

## Dialogue and line plans

```text
DialogueLine :=
    SpeakerRef CallArgs? ':' DialogueText
  | Callee CallArgs? '[' DialogueContent ']'

CallArgs       := '(' CallArg (',' CallArg)* ','? ')'
CallArg        := Expr | Ident '=' Expr | Expr '...'
RelativeLineId := RelativeId | FamilyRelativeEntityRef
LineOption     := 'id' '=' (EntityRef | RelativeId | FamilyRelativeEntityRef)
                | 'text_key' '=' (EntityRef | RelativeId | FamilyRelativeEntityRef)
                | 'voice' '=' Expr
                | 'window' '=' EntityRef
                | 'source_locale' '=' Locale
                | 'hooks' '=' Expr
                | 'style' '=' Expr
                | 'rich_text' '=' Expr
                | Ident '=' Expr
DialogueText   := TextUntilLineEnd | Newline IndentedText
DialogueContent:= TextAndDialogueTags*

LinePlanAttach := 'with' Block
                | 'with' ':' Newline IndentedItems
                | 'with' ':' LinePlanItem
LinePlanItem   := InitBlock | ThreadBlock | DeferBlock | OnBlock
                | LetStmt | TimedCue | CancelRule | OutStmt | ScopeStmt | ExprStmt
InitBlock      := 'init' Block | 'init' ':' Newline IndentedItems
ThreadBlock    := 'thread' ThreadModifier? Ident? Block
                | 'thread' ThreadModifier? Ident? ':' Newline IndentedItems
ThreadModifier := 'detached'
DeferBlock     := 'defer' DeferOutcome? Block
                | 'defer' DeferOutcome? ':' Newline IndentedItems
DeferOutcome   := 'on' ('completed' | 'cancelled' | 'failed')
OnBlock        := 'on' TriggerExpr Block
                | 'on' TriggerExpr ':' Newline IndentedItems
CancelRule     := 'cancel' 'on' TriggerExpr Block
                | 'cancel' 'on' TriggerExpr '=>' ControlStmt
TriggerExpr    := 'input' '(' Pattern ')'
                | 'event' '(' Pattern ')'
                | 'signal' '(' Expr (',' Pattern)? ')'
                | 'timeout' '(' Expr ')'
                | 'mark' '(' Pattern ')'
                | 'select' '(' Pattern ')'
                | 'task' '(' Pattern ')'
                | 'scope' '(' Pattern ')'
OutStmt        := 'out' Expr
```

`with:` and flat `=== with ===` fences are sugar for the same line-plan model as
`with { ... }`. Inside a line plan, flat fences such as `=== start ===` and
`=== on mark(.name) ===` are block sugar for the same item heads as `start { ... }`
and `on mark(.name) { ... }`; unknown line-plan fence kinds, mismatched close
fences, and missing close fences are parser diagnostics. Flat `=== line ... ===`
/ `=== scope ... ===` / `=== thread ... ===` fences are dialogue authoring
sugar and lower to the corresponding canonical line, scope, or task block. A
bare `{ ... }` after a dialogue content block is an unnamed `scope` statement,
not a line plan.

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
    Visibility? 'dialogue' 'defaults' EntityRef? Block

Dialogue defaults declare a defaults profile. `pub dialogue defaults
@dialogue.defaults` is the conventional exported project-wide profile; other
profiles are selected explicitly by project/build configuration or tooling and
are not merged merely because they are visible. Product/test lowering must
diagnose multiple visible defaults profiles when no active profile can be
chosen unambiguously.

Relative IDs such as `dialogue defaults @.mobile` are not canonical and should
be rejected for defaults profiles. The mobile defaults profile is written as
`pub dialogue defaults @dialogue.defaults.mobile { ... }` or the equivalent
family-relative spelling `pub dialogue defaults @dialogue:.defaults.mobile {
... }` so the profile family is explicit.

Inside the block, structured RichText typography is written as a nested
assignment block:

```text
dialogue defaults {
  rich_text {
    text   { font = Expr, size = Expr, color = Expr, ... }
    layout { writing_mode = Expr, jlreq = Expr, vertical_latin = Expr, ... }
    ruby   { position = Expr, size = Expr, gap = Expr, overhang = Expr, ... }
  }
}
```

Structured defaults deep-merge by field through the dialogue cascade. Scalar
fields use nearest-wins semantics; collections require explicit collection
operators such as `=` or `+=`.

One-line nested assignments such as `ruby { size = 11px gap = 1px }` are not
canonical because field boundaries are ambiguous. Formatters should write
multiline blocks, or require commas if a compact single-line form is ever
accepted.

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
HookPriority := 'priority' i32
HookOnce   := 'once'
HookEffects:= 'effects' Expr (',' Expr)*
```

`check` is not part of the canonical hook header. Use `when` for conditions.
Dialogue defaults preserve structured assignment expressions for later style,
window, voice, hook, and localization lowering.

## Types

```text
Type        := TypeChoice
TypeChoice  := TypeAtom ('|' TypeAtom)+ | TypeAtom
TypeAtom    := NeverType | ConstInt | TypePath | GenericType | BorrowType | SliceType
GenericType := TypePath '<' Type (',' Type)* ','? '>'
BorrowType  := '&' Lifetime? Type
SliceType   := '[' Type ']'
```

`A | B` is an anonymous sum whose alternatives are types, not variant rows.
`Text(String) | Binary(Bytes)` is invalid; use `String | Bytes` when labels are
unnecessary, or a nominal `enum` when branch names carry meaning. Duplicate
alternatives are rejected, including alternatives that erase through transparent
aliases to the same type.

## Functions

```text
FunctionDecl  := Visibility? FunctionKind Ident GenericParams? ParamGroup+ ReturnType? WhereClause? Contract* Block
FunctionKind  := 'fn' | 'task fn' | 'dialogue fn' | 'stream fn'
GenericParams := '<' GenericParam (',' GenericParam)* ','? '>'
GenericParam  := Lifetime | IdentPath
ParamGroup    := '(' Param (',' Param)* ','? ')'
Param         := DocComment* Pattern ':' Type | DocComment* Pattern ':' '...' Type | 'self' | '&self' | '&mut self' | 'mut self'
ReturnType    := '->' Type
WhereClause   := 'where' WherePredicate (',' WherePredicate)* ','?
WherePredicate:= Type ':' Type ('+' Type)*
```

Multiple `ParamGroup` entries are curried parameter groups and are preserved as
separate syntax groups. Unexpected tokens after the return type or where clause
are syntax errors.

`param: ...T` declares one positional rest parameter. A signature may contain at
most one rest parameter, it must be the last parameter of the final parameter
group, and it cannot declare a default value. The function body sees the binding
as `Vec<T>`. Calls pass ordinary positional arguments and may splice an existing
sequence into the rest tail with `expr...`, as in `log("loaded", fields...)`.
Named rest is not part of this syntax slice. Rest element type may be an
anonymous sum such as `fields: ...(String | i64 | Duration)`.

`stream fn` must declare `-> Stream<T, E>`. Hand-written stream transforms do
not return `Source<T, E>`; live external sources use `source` declarations so
policy, replay, and privacy remain explicit.

## Source Declarations

```text
SourceDecl      := Visibility? 'source' SourceId? Ident? ':' SourceType SourceBlock
SourceId        := EntityRef | RelativeId | FamilyRelativeEntityRef
SourceType      := 'Source' '<' Type ',' Type '>'
SourceBlock     := '{' SourceBlockItem* '}'
SourceBlockItem := SourceHeader | SourceHandler | ContractClause

SourceHeader :=
    'from' Expr
  | 'backpressure' '=' BackpressurePolicy
  | 'replay' '=' ReplayPolicy
  | 'privacy' '=' PrivacyPolicy

BackpressurePolicy :=
    'latest'
  | 'bounded' '(' 'capacity' '=' IntLiteral ',' 'overflow' '=' OverflowPolicy ')'
  | 'blocking_not_allowed'

OverflowPolicy := 'drop_oldest' | 'drop_newest' | 'error' | 'coalesce'
ReplayPolicy   := 'full' | 'hash_only' | 'summary' | 'event_only' | 'none'
PrivacyPolicy  := 'transient' | 'redacted' | 'recordable' | 'private'

SourceHandler      := 'on' SourceEventPattern '=>' SourceHandlerBody
SourceEventPattern := 'item' Pattern
                    | 'error' Pattern
                    | 'progress' Pattern
                    | 'disconnected'
                    | 'permission_revoked'
                    | 'end'
SourceHandlerBody  := YieldStmt | ExprStmt | Block
```

Function-like `source name() -> Source<T, E> { ... }` is not canonical. Use
`source @source.id: Source<T, E> { ... }`.

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
YieldStmt    := 'yield' Expr
LabelRef     := '\'' Ident
```

`break expr` is allowed only in `loop`.
`out` is allowed only in line-plan, cue-block, and content-scope continuations.
`yield` parses as a statement but is semantically valid only in explicit
generation contexts: `seq { ... }`, `stream { ... }`, `stream fn`, and `source`
handlers. Flow bodies and dialogue line plans use `return`/`goto`/`out`
instead.

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
  | BracketSeqPattern
  | WholeBindingPattern

WholeBindingPattern := Ident NonBindingPattern
NonBindingPattern  := Literal | EntityRef | TuplePattern | RecordPattern | VariantPattern | BracketSeqPattern
TuplePattern  := '(' Pattern (',' Pattern)* ')'
RecordPattern := TypePath? '{' FieldPattern* '..'? '}'
FieldPattern  := Ident | Ident ':' Pattern
VariantPattern:= ('.' Ident | TypePath '::' Ident) VariantPayload?
BracketSeqPattern := '[' Pattern* RestPattern? ']'
RestPattern   := '..' Ident?
```

Examples:

```arcw
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

