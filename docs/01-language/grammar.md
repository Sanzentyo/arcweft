# Grammar Summary

This is a compact summary of the current Arcweft surface grammar. It is intentionally canonical: removed migration forms are not part of this grammar.

## Lexical conventions

```text
AsciiDigit   := '0' .. '9'
IdentStart   := '_' | UnicodeAlphabetic
IdentContinue:= IdentStart | AsciiDigit
Ident        := IdentStart IdentContinue*
IdentPath    := Ident ('.' Ident)*
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

Regular `.arcw` and `.awfagent` project documents contain declarations at the
top level. Dialogue, choice, control-flow, and ordinary statements belong
inside an owning declaration such as `flow` or `fn`. Unrecognized top-level
text is generic recovery only and never lowers or executes. Interactive REPL
statement fragments are a separate parser entrypoint; Arcweft does not define
a second project-file script dialect.

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
`@flow:.next`, `@asset:.room`, `@view:.SideDialogue`.
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
ProjectionType := TypePath '.' Ident
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

Unsuffixed numeric literals use the expected type from an annotation,
signature, or surrounding expression when one is available. Without an expected
type, integer literals fall back to `i32` and float literals fall back to
`f64`; tooling may lint those fallbacks when they appear in inferred contracts
such as closure bodies. There are no concrete `int`, `uint`, `float`, or
`Number` primitive type names.

## Module items

```text
Source       := ModuleDecl? UseDecl* Item*
ModuleDecl   := 'mod' ModulePath
UseDecl      := Visibility? 'use' ModulePath ('.' UseTree)?
Visibility   := 'pub'?
ModulePath   := ('crate' '.' | 'self' '.' | 'super' '.' | 'parent' '.')? IdentPath

Item         := OuterAttr* ItemDecl
ItemDecl     :=
    FlowDecl
  | FunctionDecl
  | EntryDecl
  | ViewDecl
  | AssetDecl
  | ImageDecl
  | ProofDecl
  | TypeDecl
OuterAttr    := '#[' AttrPath AttrArgs? ']'
InnerAttr    := '#![' AttrPath AttrArgs? ']'
AttrPath     := Ident ('.' Ident)*
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

Proofs use the ordinary outer-attribute surface and a typed proof body:

```text
ProofDecl   := 'proof' Ident '{' ProofClause* '}'
ProofClause := ('requires' | 'ensures' | 'check') Expr
             | 'use' ProofRef
             | 'assume' Expr (','? 'proof' '=' ProofRef)
ProofRef    := EntityRef
```

The top-level proof declaration uses one ordinary local name. `ProofRef` is
the entity-reference form used inside proof clauses and must resolve to the
`proof` family. The optional
`#[verify.trusted(reason = String)]` attribute marks external evidence; its
reason must be a nonempty, non-interpolated string literal. Escape sequences
are decoded once by the syntax-owned string-literal value contract, and the
exact decoded value is retained without whitespace normalization. The
attribute is reserved for proofs and rejects missing, duplicate, positional,
unknown, non-string, and decoded-empty arguments through structured syntax
diagnostics. Trust is attached to `ProofDecl` rather than represented by a
separate declaration family.

Callable proof declarations use the same header identity policy. The canonical
authoring form is `proof hoge(...)`; a generated or fully elaborated header such
as `proof @proof:.hoge hoge() = ()` (or the absolute form
`proof @proof.hoge hoge() = ()`) is also accepted. When the explicit identity
repeats the local name, check and LSP report the warning
`style::redundant_decl_identity`; it is not a parse error. A genuinely
mismatched explicit identity remains an identity diagnostic rather than being
silently rewritten.

Entity declarations share one closed header grammar. The entity family selects
the declaration kind, but it does not make otherwise free-form header words
legal:

```text
EntityDecl       := Attribute* Visibility? EntityKind DeclIdentity EntityHeaderTail? EntityBody?
EntityHeaderTail := (CallableTail | TypeTail | RelationTail)? Alias?
CallableTail     := GenericParams? ParamGroup+ ReturnType? WhereClause*
TypeTail         := ':' Type
RelationTail     := Ident EntityRef
Alias            := 'as' Ident
```

The callable form is parsed as one typed function-signature tail, including
generic parameters. `Ident EntityRef` retains relation headers such as
`parent @bus.master`; whether a particular relation or typed tail is meaningful
for an entity kind is a semantic rule. A second unstructured identifier after
a compact declaration name is not a header extension. For example,
`character unexpected extra { ... }` is a syntax error at `extra`, does not produce an
entity declaration AST node, and recovery resumes after that declaration's
line or balanced block.

Assets do not have an authored declaration surface. Reconciled project assets
come from the selected asset catalog; a normalized virtual path such as
`bg/room.png` deterministically derives the public ID `asset.bg.room`.
Presentation-object declarations remain ordinary authored declarations:

```text
ImageDecl := Visibility? 'image' DeclIdentity ImageObjectBody?
```

Authored asset references should prefer family-relative spelling such as
`@asset:.bg.room`; this omits the default family from the id path while
retaining the explicit asset reference anchor. Fully qualified references such
as `@asset.bg.room` remain available for generated surfaces, manifest/tooling
output, stored public-id roundtrips, and external interfaces that need the
stored public id verbatim. Image payload packaging is handled by the bundle
image asset table, which records the catalog identity, encoded files, and
decoded metadata.

`image` declarations establish stable presentation-object ids such as
`@image.sample.pulse_sprite`. Their bodies use the same flat fields as bounded
`image(asset = ..., ...)` calls: `asset`, `target`, `layer`, `x`, `y`, `width`,
`height`, `fit`, `alignment.*`, `opacity`, `playback.*`, `transform.*`,
`depth`, `enabled`, `visible`, `action`, `param.*`, and `proxy.*`.
`image(@image.id)` lowers through the same semantic `ImagePresentationObject`
path as an inline bounded `image(...)` call; it remains a compiled presentation
object rather than a renderer adapter.

## Flow

```text
FlowDecl     := Visibility? 'flow' DeclIdentity GenericParams? ParamGroup? ReturnType? Contract* FlowBody
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

Flows may have no parameter group or one parameter group. Multiple
`ParamGroup` entries are rejected for `flow`; curried parameter groups belong to
function-like declarations.

`crate`, `self`, and `super` are canonical module-path roots. `parent` is a
reserved alias for `super`; formatters should normalize it to `super`.
`use` introduces names only; build demand and content availability are compiler
and packaging policy.

Declaration identities have a hand-written canonical surface and a generated
surface. Hand-written flows should use `flow opening(...)`. The fully elaborated spelling
`flow @flow.opening opening(...)` is accepted for generated source and
roundtrips, but hand-written source reports `style::redundant_decl_identity`
unless it is covered by `#[allow(style::redundant_decl_identity)]` or
`#[generated]`. A mismatch such as `flow @flow.opening start(...)` reports
`identity.decl_binding_mismatch`; it is not a style warning.

Entity declarations follow the same principle. In declaration headers, the
keyword supplies the default entity family, so authored code should omit that
family prefix. Prefer
`pub character alice { display = "Alice" }` or
`content chapter_two { roots = [@flow.chapter_two] }` for hand-written source.
Fully qualified forms such as `pub character @character.alice { ... }` and
`content @content.chapter_two { ... }` are accepted but are generated or fully
elaborated surfaces rather than the recommended authoring form. Assets are not
part of this declaration grammar; their identities come from the project asset
catalog. Avoid putting display names or aliases in declaration headers.

The same rule applies to every declaration family whose keyword supplies a
default identity family: write the local name in authored source, and retain an
explicit `@family.name` only when preserving a generated/elaborated surface or
testing the identity diagnostics. Explicit `@proof.hoge`, `@proof:.hoge`, and
`@.hoge` proof identities are accepted and normalized to `proof.hoge`; a
wrong-family identity is recovered and diagnosed rather than silently changed.

## Dialogue and line plans

```text
DialogueLine :=
    SpeakerRef CallArgs? ':' DialogueText
  | Callee CallArgs? '[' DialogueContent ']'

CallArgs       := '(' (CallArg (',' CallArg)* ','?)? ')'
CallArg        := Expr | Ident '=' Expr | Expr '...'
RelativeLineId := RelativeId | FamilyRelativeEntityRef
LineOption     := 'id' '=' (EntityRef | RelativeId | FamilyRelativeEntityRef)
                | 'text_key' '=' (EntityRef | RelativeId | FamilyRelativeEntityRef)
                | 'voice' '=' Expr
                | 'view' '=' EntityRef
                | 'source_locale' '=' Locale
                | 'hooks' '=' Expr
                | 'style' '=' Expr
                | 'rich_text' '=' Expr
                | Ident '=' Expr
DialogueText   := TextUntilLineEnd | Newline IndentedText
DialogueContent:= (TextAndDialogueTags | FxTextSpan)*
TagSpace        := UnicodeWhitespace+

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
             | 'view' Block
             | 'select' Block
             | 'id' '=' Expr
             | LetStmt

OptionForSugar := 'option' Pattern 'in' Expr OptionBody
ChoiceArm      := StaticOptionId String ChoiceArmCondition? ChoiceArmAction
ChoiceArmCondition := 'if' Expr
ChoiceArmAction := '->' EntityRef | '=>' Expr

ChoicePlan := 'with' Block | 'with' ':' Newline IndentedItems
```

`choice` displays a choice view and may also be used as an expression. A choice body is a lexical scope. `option` creates an option candidate. Inline arm `if` is enabled-state sugar; wrapping an option in a block `if` controls whether the option exists. `visible = expr` controls rendering. `view { ... }` is propagated to rendering, accessibility, test, and Agent observation.

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

## Dialogue presentation ownership

Dialogue presentation selection is project metadata, not an Arcweft source
item. The accepted launch or project profile owns a typed dialogue View, an
optional base Style, and the inline-failure policy:

```toml
[profiles.game.dialogue]
view = "view.main_dialogue"
style = "style.main_dialogue"

[profiles.game.dialogue.inline-failure]
kind = "fail_line"
```

The manifest IDs are validated as View and Style identities during project
admission. The selected View and Style become the base presentation inputs for
all dialogue in that profile. Character-local `dialogue_style`, speaker preset,
line, and inline rich-text values remain source-level inputs and override the
profile base through the ordinary cascade.

Events are declared through their owning constructs, and input decoding uses
ordinary functions; the grammar has no universal hook, memo function/block, or
parser declaration.

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
FunctionDecl  := Visibility? 'fn' Ident GenericParams? ParamGroup+ ReturnType? WhereClause? Contract* Block
GenericParams := '<' GenericParam (',' GenericParam)* ','? '>'
GenericParam  := Lifetime | IdentPath
ParamGroup    := '(' Param (',' Param)* ','? ')'
Param         := DocComment* Pattern ':' Type ParamDefault?
               | DocComment* Pattern ':' '...' Type
               | 'self' | '&self' | '&mut self' | 'mut self'
ParamDefault  := '=' Expr
ReturnType    := '->' Type
WhereClause   := 'where' WherePredicate (',' WherePredicate)* ','?
WherePredicate:= Type ':' Type ('+' Type)*
```

Multiple `ParamGroup` entries are curried parameter groups and are preserved as
separate syntax groups. Unexpected tokens after the return type or where clause
are syntax errors.

`ParamDefault` is represented on the ordinary function parameter rather than
by a second parameter AST. In the current language surface, only a simple
identifier parameter on a `#[fx]` function may have a default. Its expression
must be const-evaluable and must not refer to another parameter or runtime
state. A default on another function kind or on a pattern parameter is a
semantic error; this keeps the AST ready for a future general default-argument
decision without implicitly adding that feature today.

`param: ...T` declares one positional rest parameter. A signature may contain at
most one rest parameter, it must be the last parameter of the final parameter
group, and it cannot declare a default value. The function body sees the binding
as `Vec<T>`. Calls pass ordinary positional arguments and may splice an existing
sequence into the rest tail with `expr...`, as in `log("loaded", fields...)`.
Named rest is not part of this syntax slice. Rest element type may be an
anonymous sum such as `fields: ...(String | i64 | Duration)`.

### Presentation Fx functions

Reusable static styling, animation, transforms, filters, masks, shaders, and
transitions use one ordinary function surface:

```text
FxDecl       := '#[fx]' Visibility? 'fn' Ident FxParamGroup '->' 'Fx' Block
FxParamGroup := '(' (FxParam (',' FxParam)* ','?)? ')'
FxParam      := Ident ':' Type ('=' ConstExpr)?
FxTextSpan   := '[fx' FxCall ']' DialogueContent '[/fx]'
FxCall       := Path '(' (NamedArg (',' NamedArg)* ','?)? ')'
```

`#[fx]` is an argument-free marker on an ordinary, non-generic `fn`. It implies
that the function is a pure, deterministic factory for an immutable `Fx`
graph; writing `#[pure]` as a second attribute is neither required nor
canonical. An Fx function has exactly one parameter group, no receiver, no
rest parameter, and an explicit `Fx` return type. Public parameter types and
defaults must be representable in the bundle Fx ABI.

Fx entry calls are named-only. Required parameters have no default, optional
parameters place their const-evaluable default in the function signature, and
unknown, duplicate, missing, or positional arguments are diagnostics. The
function body composes typed constructors such as `Fx.text`, `Fx.style`,
`Fx.transform`, `Fx.filter`, `Fx.mask`, `Fx.shader`, `Fx.transition`,
`Fx.conditional`, and ordered `Fx.stack`; it is not a second declaration or
builder grammar.

View expressions apply an Fx value with `.fx(value)` and may pass reactive
argument expressions. Dialogue rich text applies the same function with
`[fx path(arg=value)]...[/fx]`, but every inline argument must be a closed,
const-evaluable value so localization, replay, and line caching remain stable.
Dynamic rich-text presentation belongs on a View `RichText(...)` value through
`.fx(...)`.

An Fx function may call ordinary pure helpers and other Fx functions. It may
not mutate state, send signals, perform I/O or capability calls, await tasks,
read nondeterministic random or wall-clock values, construct View children, or
emit actions/events. Composition cycles and graph expansion budgets are checked
on the typed Fx call graph.

The source identity is the original package plus qualified function name;
imports and `pub use` aliases resolve to that declaration rather than creating
another identity. No authored `@fx` id accompanies the function. The compiler
derives a typed `FxId`, an ABI hash from its schema and renderer requirements,
and a semantic hash from its body and referenced resources. Each application
also receives a distinct deterministic `FxInstanceId` from its retained
location/span identity and application ordinal.

An ordinary `fn` whose own body contains `yield` is a generator and must
declare `-> Stream<T, E>`. A function that returns `Stream<T, E>` without an
own-scope `yield` is an ordinary stream passthrough. External capability
members returning `Stream<T, E>` are ordinary bodyless `fn` members; they are
not declarations, roots, or a separate language role. The `source` keyword and
`Source<T, E>` type are not part of the grammar.

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
generation contexts: `seq { ... }`, `stream { ... }`, an ordinary `fn` whose
own scope yields, and `source` handlers. Flow bodies and dialogue line plans
use `return`/`goto`/`out` instead.

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
RecordPattern := TypePath? '{' FieldPattern* RestPattern? '}'
FieldPattern  := Ident | Ident ':' Pattern
VariantPattern:= ('.' Ident | TypePath '.' Ident) VariantPayload?
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
entity reference.

## Expression operators and postfixes

```text
PostfixExpr := PrimaryExpr (CallArgs | '.' Ident CallArgs? | '[' Expr ']')*
PrefixExpr  := ('!' | '-') PrefixExpr | AwaitExpr | PostfixExpr
BinaryExpr  := PrefixExpr BinaryOp PrefixExpr
BinaryOp    := '*' | '/' | '%' | '+' | '-' | '&' | ComparisonOp | 'in' | '&&' | '||' | '|>' | '=>'
ComparisonOp:= '==' | '!=' | '>=' | '<=' | '>' | '<'
RangeExpr   := Expr? ('..' | '..=') Expr?
```

In infix position, `&` is the typed merge operator. Prefix `&` remains the
shared or mutable borrow operator; Pratt position distinguishes the two
without source-text reinterpretation.

Field access such as `state.affection` is structured as a field expression.
Type, module, and selector paths use dot-separated segments; public IDs use the
`@flow.opening` entity form.

## Try operator and await

```text
TryExpr      := 'try' Expr
AwaitExpr    := 'await' Expr AwaitPendingBlock?
CarrierBlockExpr := ('result' | 'option') BlockExpr
AwaitPendingBlock := 'with' ':' Newline AwaitCase+
                   | 'with' '{' AwaitCase* '}'
```

`await Need<T>` always returns `T`. Domain failure is represented by a payload
such as `Need<Result<T, E>>`; `try` is the sole Result/Option propagation
operator. `try await expr` is ordinary prefix nesting and never creates a fused
TryAwait owner.
The indentation form `with:` is syntax sugar for the canonical brace form `with { ... }`. Formatters may keep `with:` for scenario-like readability, but lowering should treat it as brace-block syntax.

There is no postfix `?` or attached `await?` form in the final grammar. Use
`try Expr` explicitly when propagation is intended.

At expression start, `result` or `option` followed immediately by BlockExpr is
a carrier block. It wraps normal tails in Ok/Some and is the nearest matching
Try boundary. `try { ... }` is ordinary Try(Block), and `need { ... }` is not a
language form.

## Never

```text
NeverType := '!' | 'Never'
DivergingExpr := ReturnStmt | GotoStmt | BreakStmt | ContinueStmt | PanicStmt | FailStmt
```

`!` coerces to any expected type. Diagnostics should normally say "this branch never returns" rather than exposing bottom-type theory to non-expert users.
