# Decisions: Control Flow, Pattern Binding, and Optional Semicolon

The cross-cutting final authoring authority is
[Converged Language, Content, and Presentation Surface](../01-language/converged-language-surface.md).
Specialized chapters retain details but do not override that surface.

## Core decision

Arcweft is still a script-friendly language, but core logic should be expression-capable enough for concise, typed, Rust-like code.

Final rule:

```text
if      expression-capable
match   expression-capable, with structured patterns
loop    expression-capable via break expr
while   statement-oriented, returns Unit
for     statement-oriented, returns Unit
```

## Pattern decision

`match` supports structured binding patterns. The same complete pattern language is reused by:

```text
match
if let
while let
let ... else
let destructuring
out destructuring
function parameter destructuring, if explicitly enabled
```

This avoids having one pattern system for `match` and another for `let`.

Phase 1 parser/HIR should carry the full pattern shape, including tuple, record, variant, list/rest, literal, entity-ref, mutable binding, and whole-pattern binding forms. Type checking may stage exhaustiveness and shape validation, but the syntax model should not collapse these patterns into raw strings.

Arcweft does not use Rust's `name @ pattern` spelling. `@` is reserved for
entity references. Whole-pattern binding should use the
`Ident Pattern` form only in unambiguous pattern positions.

## Entity, attribute, and command syntax decision

Arcweft reserves `@` for entity references and uses Rust-like outer attributes.
The old `#entity` and `@command` shapes are not canonical grammar.

```text
EntityRef:
  @flow.opening
  @asset:.bg.room
  @asset.bg.room       # fully qualified public-id form for generated/tooling output
  @<asset:bg/room.ktx2>

Attribute:
  #[derive(Clone, StableHash)]
  #[link(Flow, @flow.opening)]

Effectful scenario operation:
  bg(@asset:.bg.room, fade = 300ms)
  show(@character.alice, .smile, at = .center)
```

In ordinary source code, `#` is used only by the `#[...]` attribute opener. In
Dialogue/RichText lexical mode it also begins the checked content-expression
forms `#name`, `#[expr]`, `#call(...)`, and `#call(...)[content]`. Color values
are not bare `#fff` tokens; they are string literals interpreted as `Color`
only in an expected `Color` context.

Scenario commands are ordinary effectful function calls. The parser should not
carry migration-only branches for old `@bg`, `@show`, `@choice`, or `@memo`
forms; migration belongs in formatter/CLI tooling.

General relative entity references should use family-qualified syntax such as
`@flow:.next` or `@asset:.room`. This family-relative form is the recommended
authored spelling when the family has a default public-id prefix: `@asset:.room`
keeps the `asset` anchor but does not repeat `asset` in the id path.
`@asset.room` is the fully qualified public-id spelling for generated surfaces,
manifests, stored public-id roundtrips, and tool queries that need the stored id
verbatim; it is not the recommended spelling for ordinary hand-authored asset
references. ID-bearing contexts may also accept family-qualified forms such as
`@say:.greeting`, but hand-written declarations should normally use the shorter
`@.greeting` style.

## Literal and primitive type decision

Arcweft uses explicit-width numeric primitives and no default numeric fallback.

```text
Integers:
  i8 i16 i32 i64 i128
  u8 u16 u32 u64 u128
  isize usize

Floats:
  f32 f64

Other primitives:
  () bool String Duration Color Ratio Length Angle
```

`int`, `uint`, `float`, and `Number` are not concrete standard primitive type
names. Unsuffixed numeric literals require an expected type; otherwise use a
suffix such as `10i32`, `2.0f32`, or `42usize`.

Unit-number literals are first-class syntax and resolve by expected type:

```arcw
let fade: Duration = 300ms
let size: Length = 100pt
let alpha: Ratio = 85%
let theta: Angle = 90deg
let gain = -6db
let tempo = 92bpm
```

`"#fff"`, `"#ffff"`, `"#rrggbb"`, and `"#rrggbbaa"` remain ordinary string
literals until the type checker sees an expected `Color`.

## CharacterDialogue surface ownership decision

Dialogue construction and content application are distinct typed operations.
Parentheses construct or immutably reconfigure a `CharacterDialogue`; direct
`character(args)[content]` calls and colon sugar apply `DialogueContent` and
produce a line. Ordinary calls use the shared `CallExpr` owner. There is no
method-suffix or speaker-wrapper canonical surface.

Final forms:

```text
Ref<Character>(CharacterDialoguePatch) -> CharacterDialogue
CharacterDialogue(CharacterDialoguePatch) -> CharacterDialogue
character(args)[DialogueContent] -> DialogueLine
CharacterDialogue: DialogueContent -> DialogueLine
with { line_plan }

try await expr with { pending p => ... }

at(time) { cue_body }
```

Examples:

```text
alice(voice=auto)        # CharacterDialogue construction
alice(voice=auto)[text]  # content application
alice(voice=auto): text  # colon content application

with:
  -> with { ... }
```

Formatters preserve `with:` by default in hand-written scenario files. CLI and
LSP do not expose a semantic Dialogue sugar-expansion action.

`CharacterDialogue[text] { ... }` is not a line-plan attachment. A bare trailing
`{ ... }` after content application is parsed as a separate unnamed `scope`, so
line plans must use `with { ... }` or `with:`.

## Scope and relative ID decision

`{ ... }` is a lexical block and can be a value-producing expression block in
expression position. Statement-oriented bodies such as flow bodies, line plans,
choice plans, `while`, and `for` do not export their final expression; they use
explicit transfer such as `return`, `out`, or `break`.

`scope { ... }` is a bare scope: syntactic sugar for `scope name { ... }` with
the `name` part omitted. It is lexical and does not add a scope segment to
generated IDs. As a statement, a bare `{ ... }` is one more sugar layer for
that unnamed `scope { ... }` form. As an expression, ordinary `{ ... }`
remains a value-producing block whose final expression determines the value
unless that value is discarded with `;` or `let _ = ...`.

Named lexical scopes use the `scope` keyword:

```arcw
scope rain {
    alice(id=@.comment):
        雨、強くなってきたね。[p]
}
```

The scope name contributes to relative line, text-key, choice, and option ID
generation inside the block. It is also a diagnostic, trace, and LSP/debug name.
`scope name { ... }` can be used in expression position and returns the final
expression just like `{ ... }`. The bare scope form `scope { ... }` is exactly
the same scope expression with the name omitted; it creates a lexical scope and
returns a value in expression position, but it does not add a segment to
generated IDs.

Relative IDs use `@.suffix` for the current ID scope. Each extra dot walks one
parent ID scope outward: `@..suffix` is one parent and `@...suffix` is two
parents. The explicit readable spelling is also accepted:
`@super.suffix`, `@super.super.suffix`, and so on. These forms are accepted only
in ID-bearing contexts where the entity family is known, such as dialogue line
IDs, choice IDs, option IDs, and text-key overrides. They are not general entity
references. Bare `.suffix` is not part of the core grammar, and bare `..suffix`
is not accepted because `..` already appears in range and rest-pattern syntax.

General entity references must still name their family, but authoring should
prefer family-relative spellings when that family has a default public-id
prefix. Use `@flow:.next`, `@asset:.room`, or `@view:.SideDialogue`
for normal source references. Use absolute spellings such as `@asset.bg.room`
only for generated surfaces, manifest/tooling output, and external interfaces
that need the stored public id verbatim. Unqualified `@.next` is rejected in
those reference contexts.

```text
alice(id=@.greeting)
  -> @say.opening.alice.greeting

alice(id=@.comment)
  -> @say.opening.alice.rain.comment

choice @.first
  -> @choice.opening.rain.first

@.listen
  -> @choice.opening.rain.first.listen
```

When the named scope path is empty, the scope segment is omitted. It is not
emitted as an empty path component.

For module and import paths, use Rust-like roots instead:

```arcw
use self.characters.{alice}
use super.common.{route_gate}
use crate.game.prelude.*
```

`parent.` is reserved as an alias for `super.`, but canonical formatting uses
`super.`.

## let-else decision

`let PAT = EXPR else { ... }` is supported. The `else` block must diverge or otherwise leave the current continuation.

Allowed from `else`:

```text
return
goto
break
continue
fail(...) / panic(...)
never-returning function
```

Not allowed:

```text
let .Some(x) = opt else { 0 }
```

Use `match` or `unwrap_or` for fallback values.

## while-let decision

`while let PAT = EXPR { ... }` is supported and returns `Unit`.

```arcw
while let .Some(event) = queue.pop_front() {
    handle_event(event)
}
```

If the loop must return a value, use `loop { break value }`.

## Semicolon decision

`';'` is **not required** for normal statement endings. It remains available for two cases:

```text
1. Same-line separation:
   let a = 1; let b = 2

2. Explicit value discard, especially final expressions:
   fn f() -> Unit { compute(); }
```

This is the most balanced choice after adding expression-oriented `if` / `match` / `loop`.

## Await / `try` / carrier-block decision

`Need<T>` is the unary temporal carrier and `await` returns exactly `T`.
Fallible work returns `Need<Result<T, E>>`; optional work returns
`Need<Option<T>>`. Prefix `try` is the sole propagation operator for Result and
Option, so `try await expr with:` is ordinary composition. Arcweft has no
postfix `?`, attached `await?`, or fused TryAwait owner.

```arcw
let bg_result = await asset.image(@asset:.bg.room) with:
    pending p:
        scene @scene.loading

let bg = try await asset.image(@asset:.bg.room) with:
    pending p:
        scene @scene.loading
```

`with:` belongs to the Await expression. `try await` therefore parses as
`try (await ...)`; it is not a special Await form or source-preserving sugar.

`result {}` and `option {}` create local carrier boundaries and wrap normal
tails in Ok/Some without flattening. `try {}` is ordinary Try over a block and
does not create a boundary. `need {}` is not introduced.

## Error/context decision

`ArcError` carries Arcweft source trace by default. The trace contains file/line/span, flow ID, dialogue line ID, text key, entity ID, hook/task ID, tick, and state hash when available.

`context` / `with_context` are standard APIs on `Result`, `Option`, and `Need`. They append context frames without removing the original cause.

## Never decision

Arcweft has a real bottom type `!`, shown as `Never` in diagnostics/manifests. It is required for expression-oriented `if`, `match`, `loop`, `let else`, `try`, `return`, `goto`, `break`, `continue`, `panic`, and `fail`.

## Control-transfer target decision

`out` is limited to line-plan, cue-block, and content-scope outputs. It is not a general block return.

Control-transfer diagnostics must name the continuation being exited. Scope labels are supported on blocks and loops using Rust-like labels:

```arcw
'choose: loop {
    if done {
        break 'choose route
    }
}

alice()[聞いて。[p]]
with 'line {
    cancel on input(.SkipLine):
        out 'line .Skipped
}
```

`break` and `continue` may target loop labels. `out` may target a line/cue/content label. `return` exits the nearest `fn`, `parser`, or `flow`; diagnostics should spell that target explicitly, and future syntax may allow `return from 'flow expr` only for named function/flow boundaries if needed.

