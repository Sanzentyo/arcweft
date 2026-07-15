# Block Scopes and `{ ... }`

Arcweft supports `{ ... }` blocks as lexical scopes. They can be used in typed code, expression arms, functions, loops, line plans, and cue blocks.

## Lexical scope

```arcw
let x = {
    let a = 1
    let b = 2
    a + b
}
```

`a` and `b` are visible only inside the block.

## Expression block

In expression position, the final expression is the block value unless it is explicitly discarded with `;`.

```arcw
let x = {
    let a = 1
    a + 1
}
```

Type: `i32`.

```arcw
let unit = {
    compute();
}
```

Type: `Unit`.

The same rule applies to richer control-flow expressions inside the block:

```arcw
let label = {
    let affection = state.affection[@character.alice]
    if affection >= 3 {
        "聞いてみる"
    } else {
        "まだ聞けない"
    }
}
```

Here the block has type `String`.

`scope { ... }` is a bare scope: syntactic sugar for `scope name { ... }` with
the `name` part omitted. When a bare `{ ... }` appears as a statement, it is a
second sugar layer for that unnamed `scope { ... }` form. Its locals do not
escape, it does not add a segment to generated relative IDs, and any final
non-`Unit` value must be explicitly discarded.

```arcw
{
    let tmp = route_title(state.route)
    log.debug("route={tmp}", tmp = tmp);
}
```

## Statement block

Some blocks are statement-oriented:

```text
flow body
with: dialogue line plan
choice body
choice with: plan
while body
for body
```

They do not export a value via final expression. Use explicit transfer:

```arcw
return expr
out expr
break expr
```

Line plans and choice plans therefore use `out` for their own result values:

```arcw
let voice = alice(id=@.greeting)[
    おはよう。[p]
]
with:
    let voice = line.voice_handle()
    out voice
```

## Scope

The canonical statement form is `scope name { ... }`. The bare scope form
`scope { ... }` is the same construct with `name` omitted. A bare statement
`{ ... }` then normalizes to that unnamed `scope { ... }`. Use
`scope name { ... }` when a lexical block should also name an ID namespace,
diagnostic frame, trace frame, or LSP/debug region.

```arcw
scope rain {
    地の文(id=@.sound):
        扉の向こうから、雨の音がした。[p]

    alice(id=@.comment):
        雨、強くなってきたね。[p]
}
```

The block is still lexical: locals introduced inside the scope do not escape.
The name is also added to relative dialogue, choice, option, and text-key ID
generation inside the block.

```text
地の文(id=@.sound)
  -> @say.opening.narrator.rain.sound
  -> @text.opening.narrator.rain.sound

alice(id=@.comment)
  -> @say.opening.alice.rain.comment
  -> @text.opening.alice.rain.comment
```

If a line ID is omitted, the generated stable slot still includes the current
named-scope path:

```arcw
scope rain {
    地の文:
        扉の向こうから、雨の音がした。[p]
}
```

```text
@say.opening.narrator.rain.001
@text.opening.narrator.rain.001
```

Named scopes can nest, and the scope path is appended in order:

```arcw
scope rain {
    scope window {
        地の文(id=@.rattle):
            窓が小さく鳴った。[p]
    }
}
```

```text
@say.opening.narrator.rain.window.rattle
@text.opening.narrator.rain.window.rattle
```

`scope` can be used in expression position too. In that case, the final
expression is the value just like an ordinary `{ ... }` block.

```arcw
let can_enter = scope alice_route_check {
    let affection_ok = state.affection[@character.alice] >= 3
    let has_key = state.inventory.contains(@item.alice_key)
    affection_ok && has_key
}
```

The name may be omitted. `scope { ... }` is the bare scope expression: it has
the same lexical and value-producing behavior as `scope name { ... }`, but it
does not contribute an ID namespace segment.

```arcw
let can_enter = scope {
    let affection_ok = state.affection[@character.alice] >= 3
    affection_ok
}
```

Only ID-bearing constructs inside the named scope use the scope path for ID
generation. The value of the scope expression is still only its final
expression.

For choices, the same scope path is applied to the choice ID first, and
relative option IDs are then resolved under that normalized choice ID.

```arcw
scope dream {
    choice @.first {
        @.listen "聞いてみる" -> @flow.alice_intro
    }
}
```

```text
choice @.first -> @choice.opening.dream.first
@.listen       -> @choice.opening.dream.first.listen
```

When there is no named scope, the scope segment is omitted rather than emitted
as an empty path component:

```text
alice(id=@.greeting) -> @say.opening.alice.greeting
choice @.first       -> @choice.opening.first
```

## Borrow and lifetime

Values borrowed inside a block cannot escape if their lifetime is shorter than the destination scope.

```arcw
let slice = {
    let pixels: &'frame [u8] = &frame.bytes
    pixels
}
// error: pixels cannot escape its lexical borrow scope
```

The same rule applies to any region exit. Borrowed values cannot be returned,
exported with `out`, used as a block final value, or written into an upper
lifetime registry such as `'flow.*`:

```arcw
let escaped = {
    let pixels: &'asset [Rgba8] = bg.pixels()
    pixels
}
// error: borrowed value cannot escape through block final value

'flow.cache.pixels <- pixels
// error: borrowed value cannot escape through upper lifetime registry write
```

Borrowed locals can be ended before a suspension or region boundary with a
direct explicit drop statement. Conditional drops inside `if`, `match`, or loop
bodies are not enough unless every possible path proves the borrow has ended.
This is a semantic lifetime end for the local borrow; using the dropped binding
after this point is a checker error in later ownership passes.

```arcw
let pixels: &'asset [Rgba8] = bg.pixels()
drop(pixels)

try await load_avatar() with:
    pending p:
        progress.set(p.ratio)
```

Use owned values or handles:

```arcw
let owned = {
    let pixels: &'frame [u8] = &frame.bytes
    pixels.to_owned()
}
```

## Cue scope

Dialogue cue blocks create scopes too.

```arcw
alice[おはよう。[p]]
with:
    let voice = line.voice_handle()
    at(0.2s):
        let old = alice.current_face()
        alice.look(smile)
    out voice
```

`old` is visible only inside the `at` block. `voice` is visible throughout the `with:` block and can be returned with `out`.

