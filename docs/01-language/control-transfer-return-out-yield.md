# Control Transfer: `return`, `out`, `yield`, `break`, `continue`

Arcweft separates five kinds of control transfer.

## `return`

`return expr` leaves the nearest `fn`, `task fn`, `parser`, or `flow`.

```awft
pub flow @flow.title title(state: GameState) -> Result<FlowExit, FlowError> {
    if state.config.skip_title {
        return Ok(FlowExit::Goto(@flow.opening))
    }

    Ok(FlowExit::Done)
}
```

## `out`

`out expr` exports a value from a line plan, cue block, or content scope. It does not return from the enclosing flow.

```awft
let (actor, voice) = alice[おはよう。[p]]
with:
    let actor = alice.stage.acquire(scope=line)
    let voice = line.voice_handle()
    out (actor, voice)
```

Do not use `return` for line-plan values.

## `yield`

`yield expr` is reserved for `Source<T, E>` / generator-like streams.

```awft
source camera_frames() -> Source<VideoFrame, CameraError> {
    loop {
        let frame = await camera.next_frame()
        yield frame
    }
}
```

Do not use `yield` in dialogue line plans.

## `break` and `continue`

`break` leaves the nearest loop.

```awft
loop {
    if done {
        break
    }
}
```

`break expr` gives a `loop` expression its value.

```awft
let result = loop {
    if ready {
        break value
    }
}
```

`continue` starts the next iteration.

```awft
while let .Some(event) = queue.pop_front() {
    if !event.is_relevant() {
        continue
    }

    handle_event(event)
}
```

Loop scopes may be labeled when an explicit target improves diagnostics or avoids ambiguity.

```awft
'events: loop {
    let event = await_input_event()

    match event {
        .TextAdvanced => continue 'events
        .ChoiceSelected { id } => break 'events route_for_choice(id)
        _ => ()
    }
}
```

## `break expr` is loop-only

`while` and `for` are statement-oriented and return `Unit`.

```awft
while cond {
    break 1  # error
}
```

Use `loop` if you need a value.

## Interaction with cancellation

Dialogue `cancel on ...` branches may use `return`, `out`, `goto`, or `continue` depending on their target.

```awft
alice[長い台詞です。[p]]
with:
    cancel on input .SkipLine:
        text.flush(mode = .Instant)
        out .Skipped

    cancel on input .BackToTitle:
        return Ok(FlowExit::Goto(@flow.title))
```

`out` gives a line result. `return` leaves the flow. `goto` is flow-transition sugar.

`out` is only valid for line-plan, cue-block, and content-scope outputs. These scopes may also be labeled:

```awft
alice.say()[長い台詞です。[p]]
with 'line {
    cancel on input .SkipLine:
        text.flush(mode = .Instant)
        out 'line .Skipped
}
```

Diagnostics must state the continuation being exited, for example "this `return` exits flow `flow.opening`" or "this `out` exits line scope `'line`".
