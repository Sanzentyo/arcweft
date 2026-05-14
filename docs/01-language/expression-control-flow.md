# Expression Control Flow

## `if` as expression

```awft
let face = if state.affection[@character.alice] >= 3 {
    smile
} else {
    worried
}
```

Rules:

```text
- if used as a value, else is required.
- then and else branch types must unify.
- if used as a statement, else may be omitted and the type is Unit.
```

Statement form:

```awft
if state.flags.contains(.debug) {
    log info "debug mode"
}
```

Value form:

```awft
let route = if ready {
    @flow.alice_intro
} else {
    @flow.alice_locked
}
```

## `if let`

```awft
if let .Some(route) = state.route_override {
    goto route
} else {
    goto @flow.title
}
```

Value form:

```awft
let route = if let .Some(route) = state.route_override {
    route
} else {
    @flow.title
}
```

Optional guard:

```awft
if let .Some(route) = state.route_override when route_available(route, state) {
    goto route
}
```

## `let ... else`

```awft
let .Some(route) = state.route_override else {
    goto @flow.title
}

goto route
```

The `else` block must diverge or leave the current continuation.

Allowed:

```awft
return Err(.MissingRoute)
goto @flow.title
break value
continue
panic "missing route"
```

Not allowed:

```awft
let .Some(route) = maybe_route else {
    @flow.title
}
```

Use `match` for fallback values.

## `match` as expression

```awft
let target = match selected.id {
    @choice.opening.listen when state.affection[@character.alice] >= 3 => @flow.alice_intro
    @choice.opening.listen => @flow.alice_locked
    @choice.opening.silent => @flow.quiet_intro
    _ => @flow.title
}
```

Structured bindings:

```awft
match event {
    .ChoiceSelected { id } => handle_choice(id)
    .TruckFinished { result: TruckResult { score, rank, .. } } => handle_rank(rank)
    .Ui { event: ui_event } => handle_ui(ui_event)
    _ => ()
}
```

Rules:

```text
- match must be exhaustive.
- arms of value-producing match must have a common type.
- statement-style match may have Unit arms.
- guards use `when`.
```

## `loop` expression with `break expr`

`loop` can return a value.

```awft
let chosen = loop {
    let event = await_input_event()

    match event {
        .ChoiceSelected { id } => break id
        .TextAdvanced => continue
        _ => ()
    }
}
```

Rules:

```text
- `break expr` gives the loop its value.
- all value-carrying break expressions must unify.
- `break` without expression is `Unit`.
- a loop with no reachable break has type Never.
```

Unit loop:

```awft
loop {
    tick()
    if done { break }
}
```

Value loop:

```awft
let route = loop {
    let event = wait_event()
    if let .ChoiceSelected { id } = event {
        break route_for_choice(id)
    }
}
```

## `while`

`while` is supported but returns `Unit`.

```awft
while state.loading_tasks > 0 {
    poll_tasks()
}
```

Use `loop { break value }` if a loop must produce a value.

`break expr` is not allowed in `while`.

```awft
while cond {
    break 1  # error
}
```

Use:

```awft
let value = loop {
    if !cond { break 1 }
}
```

## `while let`

```awft
while let .Some(event) = queue.pop_front() {
    handle_event(event)
}
```

With guard:

```awft
while let .Some(event) = queue.pop_front() when event.is_relevant() {
    handle_event(event)
}
```

The expression is evaluated at the start of each iteration. If the pattern fails, the loop ends. If the guard is false, the loop ends.

## `for` and ranges

Arcweft supports iterator/range loops as statement loops.

```awft
for i in 0..10 {
    log debug "i={i}" { i = i }
}

for i in 0..=10 {
    draw_tick(i)
}
```

Range forms:

```text
0..10    half-open range
0..=10   inclusive range
..10     from start, if context supports it
0..      unbounded end, if context supports it
```

`for` returns `Unit`. Use `loop` for value-producing loops.

