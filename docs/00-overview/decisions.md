# Decisions: Control Flow, Pattern Binding, and Optional Semicolon

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

Arcweft does not use Rust's `name @ pattern` spelling. `@` remains available for
attributes and scenario commands. Whole-pattern binding should use the
`Ident Pattern` form only in unambiguous pattern positions.

## Syntax canonicalization decision

Arcweft has script-friendly sugar, but semantic hashing, formatting, hot reload, and diagnostics use canonical lowering.

Canonical forms:

```text
speaker.say(args)[text]
with { line_plan }

try await expr with { pending p => ... }

at(time) { cue_body }
```

Sugar forms:

```text
speaker: text
speaker(args): text
speaker[text]
with:
    line_plan
await? expr with:
    pending p:
        ...
at(time):
    cue_body
```

Lowering rules:

```text
alice: text
  -> alice.say()[text]

alice(voice=auto): text
  -> alice.say(voice=auto)[text]

alice2(voice=auto): text
  -> alice2(voice=auto)[text]
     # speaker presets remain callable; do not force `.say`.

with:
  -> with { ... }

await? expr with { ... }
  -> try await expr with { ... }
```

Formatters should preserve `with:` by default in hand-written scenario files. LSP and CLI may offer an explicit expansion action that rewrites sugar to canonical form.

`speaker.say()[text] { ... }` is not a line-plan attachment. A bare trailing `{ ... }` after a dialogue call is parsed as a separate lexical block/scope, so line plans must use `with { ... }` or `with:`.

## Scope and relative ID decision

`{ ... }` is a lexical block and can be a value-producing expression block in
expression position. Statement-oriented bodies such as flow bodies, line plans,
choice plans, `while`, and `for` do not export their final expression; they use
explicit transfer such as `return`, `out`, or `break`.

Named lexical scopes use the `scope` keyword:

```awft
scope rain {
    alice(id=.comment):
        雨、強くなってきたね。[p]
}
```

The scope name contributes to relative line, text-key, choice, and option ID
generation inside the block. It is also a diagnostic, trace, and LSP/debug name.
`scope name { ... }` can be used in expression position and returns the final
expression just like `{ ... }`.

Relative `.suffix` IDs are accepted only in ID-bearing contexts where the
entity family is known. They are not general entity references.

```text
alice(id=.greeting)
  -> #say.opening.alice.greeting

alice(id=.comment)
  -> #say.opening.alice.rain.comment

choice .first
  -> #choice.opening.rain.first

.listen
  -> #choice.opening.rain.first.listen
```

When the named scope path is empty, the scope segment is omitted. It is not
emitted as an empty path component.

For module and import paths, use Rust-like roots instead:

```awft
use self::characters::{alice}
use super::common::{route_gate}
use crate::game::prelude::*
```

`parent::` is reserved as an alias for `super::`, but canonical formatting uses
`super::`.

## let-else decision

`let PAT = EXPR else { ... }` is supported. The `else` block must diverge or otherwise leave the current continuation.

Allowed from `else`:

```text
return
goto
break
continue
fail / panic
never-returning function
```

Not allowed:

```text
let .Some(x) = opt else { 0 }
```

Use `match` or `unwrap_or` for fallback values.

## while-let decision

`while let PAT = EXPR { ... }` is supported and returns `Unit`.

```awft
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

## Await / `?` decision

`await expr with:` returns `Result<T, E>`. The ergonomic propagation form is `try await expr with:`.
`await? expr with:` is accepted as syntax sugar for `try await expr with:`.

`?` remains the ordinary Rust-like postfix propagation operator for `Result` and `Option`. Arcweft also reserves prefix `try expr` as a general propagation form equivalent to `expr?`; `try await` is the important readable specialization where `await` and pending handling must group before propagation.

```awft
let bg_result = await asset.image(#asset.bg.room) with:
    pending p:
        scene #scene.loading

let bg = try await asset.image(#asset.bg.room) with:
    pending p:
        scene #scene.loading

let bg = await? asset.image(#asset.bg.room) with:
    pending p:
        scene #scene.loading
```

The parenthesized form `(await ... with: ...)?` is valid but not recommended for hand-written code.

Rejected only for await-with grouping ambiguity:

```awft
await expr? with: ...
```

Rationale: `?` must remain Rust-like, but pending handling makes postfix grouping unpleasant. `try await` is explicit sugar for awaiting and applying `?`.

## Error/context decision

`ArcError` carries Arcweft source trace by default. The trace contains file/line/span, flow ID, dialogue line ID, text key, entity ID, hook/task ID, tick, and state hash when available.

`context` / `with_context` are standard APIs on `Result`, `Option`, and `Need`. They append context frames without removing the original cause.

## Never decision

Arcweft has a real bottom type `!`, shown as `Never` in diagnostics/manifests. It is required for expression-oriented `if`, `match`, `loop`, `let else`, `?`, `return`, `goto`, `break`, `continue`, `panic`, and `fail`.

## Control-transfer target decision

`out` is limited to line-plan, cue-block, and content-scope outputs. It is not a general block return.

Control-transfer diagnostics must name the continuation being exited. Scope labels are supported on blocks and loops using Rust-like labels:

```awft
'choose: loop {
    if done {
        break 'choose route
    }
}

alice.say()[聞いて。[p]]
with 'line {
    cancel on input .SkipLine:
        out 'line .Skipped
}
```

`break` and `continue` may target loop labels. `out` may target a line/cue/content label. `return` exits the nearest `fn`, `task fn`, `parser`, or `flow`; diagnostics should spell that target explicitly, and future syntax may allow `return from 'flow expr` only for named function/flow boundaries if needed.
