# Never / Bottom Type

Arcweft has a bottom type. Its canonical name is `Never`; the advanced Rust-like alias `!` is also allowed in type position.

`Never` / `!` is the type of expressions that never produce a value in the current continuation.

```awft
fn impossible() -> Never {
    panic "unreachable"
}

fn impossible_short() -> ! {
    panic "unreachable"
}
```

Examples:

```awft
return Err(.MissingRoute)
goto @flow.title
break route
continue
panic "invalid state"
fail .InvariantBroken
loop { tick() }
```

## Why Arcweft needs `!`

Expression-oriented control flow requires a bottom type. Without it, these common forms cannot type-check cleanly:

```awft
let route = if let .Some(route) = state.route_override {
    route
} else {
    goto @flow.title
}
```

The `else` branch does not return a `Ref<Flow>`; it leaves the current continuation. Its type is `!`, and `!` coerces to `Ref<Flow>`.

Similarly:

```awft
let value = match maybe_value {
    .Some(v) => v
    .None => return Err(.MissingValue)
}
```

The `.None` arm has type `!`, so the whole `match` has the type of `v`.

## Coercion rule

```text
! <: T
```

An expression of type `!` can be used where any type `T` is expected.

This applies to:

- `if` branches
- `match` arms
- `loop` break typing
- `let ... else`
- block final expressions
- `?` propagation
- cancellation branches that leave the current line or flow

## Diverging expressions

```awft
return expr      # exits current fn / flow / parser / task fn
goto @flow.x     # exits current flow segment with FlowExit::Goto
break expr       # exits nearest loop; if loop-valued, contributes expr type
continue         # starts next loop iteration
panic "msg"      # runtime failure; type !
fail error       # construct ArcError and diverge; type !
abort line       # cancel current line and diverge from line continuation
```

## `loop` and Never

A `loop` with no reachable `break` has type `!`.

```awft
fn never_returns() -> ! {
    loop {
        wait_frame()
    }
}
```

A `loop` with `break expr` has the unified type of all break expressions.

```awft
let route = loop {
    let event = wait_event()
    match event {
        .ChoiceSelected { id } => break route_for_choice(id)
        .BackToTitle => break @flow.title
        _ => continue
    }
}
```

## `let ... else`

The `else` part must diverge.

```awft
let .Some(route) = state.route_override else {
    goto @flow.title
}
```

Invalid:

```awft
let .Some(route) = state.route_override else {
    @flow.title
}
```

Use `match` or `unwrap_or` for fallback values.

## Error propagation and `!`

The `?` operator produces `!` on the error branch and `T` on the success branch.

```awft
let image = load_image()?   # Ok(image) => image, Err(e) => return Err(e)
```

Conceptually:

```awft
let image = match load_image() {
    .Ok(v) => v
    .Err(e) => return Err(e)
}
```

The error arm has type `!`, so the expression has type `Image`.

## Manifest form

In JSON/manifests, `!` is written as `Never` for readability:

```json
{
  "type": "Never"
}
```

## Design rule

`!` is a real type in the type checker, but most users only see it in diagnostics.

Diagnostics should say:

```text
this branch never returns
```

rather than forcing users to understand bottom-type theory.

