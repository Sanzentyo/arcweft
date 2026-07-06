# Patterns and Structured Bindings

Arcweft has one pattern language used by `match`, `if let`, `while let`, `let ... else`, destructuring `let`, and some function parameters.

## Basic patterns

```arcw
_                    // wildcard
x                    // binding
mut x                // mutable local binding, advanced use
42                   // literal
"hello"              // literal string
@choice.opening.listen // entity reference literal
.Some(x)             // enum variant, type inferred
.Ok(value)            // Result success
.Err(err)             // Result error
```

## Tuple patterns

```arcw
let (actor, voice) = pair

match result {
    .Ok((actor, voice)) => actor
    .Err(e) => return Err(e)
}
```

Nested tuple destructuring is supported:

```arcw
let (actor, (face0, _, voice)) = line_result
```

`_` discards the value. If the discarded value implements `CancelOnDrop`, it is dropped/cancelled immediately.

## Record / struct patterns

```arcw
match event {
    .ChoiceSelected { id } => handle_choice(id)
    .Ui { event: ui_event } => handle_ui(ui_event)
    .TruckFinished { result: TruckResult { score, rank, .. } } => handle_rank(rank)
    _ => ()
}
```

Rules:

```text
field        binds field with the same name
field: name  binds field to a different local name
..           ignores remaining fields
```

## Enum patterns

```arcw
match maybe_route {
    .Some(route) => goto route
    .None => goto @flow.title
}
```

When the expected enum type is known, short `.Variant` syntax is preferred:

```arcw
.Some(x)
.Err(e)
.ChoiceSelected { id }
```

Fully qualified variant names are allowed when needed:

```arcw
Option.Some(x)
Result.Err(e)
GameEvent.ChoiceSelected { id }
```

## Bracket sequence patterns

Phase 1 syntax supports fixed-length bracket sequence patterns and rest patterns.

```arcw
match items {
    [] => .Empty
    [one] => .One(one)
    [first, ..rest] => .Many(first, rest)
}
```

`Vec`, `Array`, and borrowed `Slice` destructuring use the same `[ ... ]`
surface syntax. Type checking decides which sequence-like scrutinee forms are
accepted and applies the corresponding lifetime rules.

## Binding entire pattern

Whole-pattern binding is supported without `@`. `@` remains reserved for
attributes and scenario commands, so pattern binding uses a leading name followed
by a non-binding pattern in contexts where this is unambiguous.

```arcw
match event {
    ev .ChoiceSelected { id } => {
        log.info("choice event {ev:?}", ev = ev)
        handle_choice(id)
    }
    _ => ()
}
```

The binding name captures the entire matched value, while nested fields such as
`id` are still bound by the inner pattern.

## Pattern guards

Arcweft uses `when` for pattern guards to align with hooks and contracts.

```arcw
match selected.id {
    @choice.opening.listen when state.affection[@character.alice] >= 3 => @flow.alice_intro
    @choice.opening.listen => @flow.alice_locked
    _ => @flow.quiet_intro
}
```

`when` bindings can use names bound by the pattern.

```arcw
match event {
    .ChoiceSelected { id } when choice_enabled(id, state) => handle_choice(id)
    _ => ()
}
```

## Pattern scope

Bindings introduced by a pattern are scoped to the arm/body where the pattern succeeded.

```arcw
if let .Some(route) = state.route_override {
    goto route
}

// route is not visible here
```

For `let ... else`, bindings are visible after the statement because the else branch must diverge.

```arcw
let .Some(route) = state.route_override else {
    goto @flow.title
}

goto route
```

