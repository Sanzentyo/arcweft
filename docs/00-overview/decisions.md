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

`match` supports structured binding patterns. The same pattern language is reused by:

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
