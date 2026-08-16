# Control Transfer: `return`, `out`, `yield`, `break`, `continue`

Arcweft separates five kinds of control transfer.

## `return`

`return expr` leaves the nearest `fn`, `parser`, or `flow`.

```arcw
pub flow title(state: GameState) -> Result<FlowExit, FlowError> {
    if state.config.skip_title {
        return Ok(FlowExit.Goto(@flow.opening))
    }

    Ok(FlowExit.Done)
}
```

## `out`

`out expr` exports a value from a line plan, cue block, or content scope. It does not return from the enclosing flow.

```arcw
let (actor, voice) = alice[おはよう。[p]]
with:
    let actor = alice.stage.acquire(scope=line)
    let voice = line.voice_handle()
    out (actor, voice)
```

Do not use `return` for line-plan values.

## `yield`

`yield expr` produces an item from an explicit generation context. It is a
statement with `Unit` type, and the expression must match the item type of the
surrounding generator-like construct.

```arcw
fn rms_level(frames: Stream<AudioFrame, AudioError>) -> Stream<f32, AudioError> {
    for frame in frames {
        yield frame.rms()
    }
}
```

Allowed contexts:

```text
seq { ... }
stream { ... }
fn ... -> Stream<T, E> { ... yield ... }
```

`yield` is a suspension boundary like `await` and `thread`: non-`'static`
borrows, raw pointers, and borrowed callback buffers may not be captured across
it. `seq` blocks are pure lazy sequences, so they also reject runtime effects
such as `await`, `thread`, signal writes, metric writes, event emission, and
logging.

Live external inputs are ordinary external-capability operations returning
`Stream<T, E>`. They are not declarations or roots, and they do not add a
source-specific grammar role.

Do not use `yield` in a non-generator function, flow, hook, memo function, or
dialogue line plan. An ordinary `fn` becomes a generator only when its own
scope contains `yield`; use `out` for line-plan results.

```arcw
let outcome = alice.say()[長い台詞です。[p]]
with 'line {
    cancel on input(.SkipLine) {
        text.flush(mode = .Instant)
        out 'line .Skipped
    }
}
```

An external capability owns acquisition and host policy; a generator only
consumes an existing stream or granted port.

## `break` and `continue`

`break` leaves the nearest loop.

```arcw
loop {
    if done {
        break
    }
}
```

`break expr` gives a `loop` expression its value.

```arcw
let result = loop {
    if ready {
        break value
    }
}
```

`continue` starts the next iteration.

```arcw
while let .Some(event) = queue.pop_front() {
    if !event.is_relevant() {
        continue
    }

    handle_event(event)
}
```

Loop scopes may be labeled when an explicit target improves diagnostics or avoids ambiguity.

```arcw
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

```arcw
while cond {
    break 1  // error
}
```

Use `loop` if you need a value.

## Interaction with cancellation

Dialogue `cancel on ...` branches may use `return`, `out`, `goto`, or `continue` depending on their target.

```arcw
alice[長い台詞です。[p]]
with:
    cancel on input(.SkipLine):
        text.flush(mode = .Instant)
        out .Skipped

    cancel on input(.BackToTitle):
        return Ok(FlowExit.Goto(@flow.title))
```

`out` gives a line result. `return` leaves the flow. `goto` is flow-transition sugar.

`out` is only valid for line-plan, cue-block, and content-scope outputs. These scopes may also be labeled:

```arcw
alice.say()[長い台詞です。[p]]
with 'line {
    cancel on input(.SkipLine):
        text.flush(mode = .Instant)
        out 'line .Skipped
}
```

Diagnostics must state the continuation being exited, for example "this `return` exits flow `flow.opening`" or "this `out` exits line scope `'line`".

