# Block Scopes and `{ ... }`

Arcweft supports `{ ... }` blocks as lexical scopes. They can be used in typed code, expression arms, functions, loops, line plans, and cue blocks.

## Lexical scope

```awft
let x = {
    let a = 1
    let b = 2
    a + b
}
```

`a` and `b` are visible only inside the block.

## Expression block

In expression position, the final expression is the block value unless it is explicitly discarded with `;`.

```awft
let x = {
    let a = 1
    a + 1
}
```

Type: `i32`.

```awft
let unit = {
    compute();
}
```

Type: `Unit`.

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

```awft
return expr
out expr
break expr
```

## Named scope

Use `scope name { ... }` when a lexical block should also name an ID namespace,
diagnostic frame, trace frame, or LSP/debug region.

```awft
scope rain {
    地の文(id=.sound):
        扉の向こうから、雨の音がした。[p]

    alice(id=.comment):
        雨、強くなってきたね。[p]
}
```

The block is still lexical: locals introduced inside the scope do not escape.
The name is also added to relative dialogue, choice, option, and text-key ID
generation inside the block.

```text
地の文(id=.sound)
  -> #say.opening.narrator.rain.sound
  -> #text.opening.narrator.rain.sound

alice(id=.comment)
  -> #say.opening.alice.rain.comment
  -> #text.opening.alice.rain.comment
```

Named scopes can nest, and the scope path is appended in order:

```awft
scope rain {
    scope window {
        地の文(id=.rattle):
            窓が小さく鳴った。[p]
    }
}
```

```text
#say.opening.narrator.rain.window.rattle
#text.opening.narrator.rain.window.rattle
```

`scope` can be used in expression position too. In that case, the final
expression is the value just like an ordinary `{ ... }` block.

```awft
let can_enter = scope alice_route_check {
    let affection_ok = state.affection[#character.alice] >= 3
    let has_key = state.inventory.contains(#item.alice_key)
    affection_ok && has_key
}
```

## Borrow and lifetime

Values borrowed inside a block cannot escape if their lifetime is shorter than the destination scope.

```awft
let slice = {
    borrow frame as pixels: &'frame [u8] {
        pixels
    }
}
# error: pixels cannot escape frame borrow scope
```

Use owned values or handles:

```awft
let owned = {
    borrow frame as pixels: &'frame [u8] {
        pixels.to_owned()
    }
}
```

## Cue scope

Dialogue cue blocks create scopes too.

```awft
alice[おはよう。[p]]
with:
    let voice = line.voice_handle()
    at(0.2s):
        let old = alice.current_face()
        alice.face(smile)
    out voice
```

`old` is visible only inside the `at` block. `voice` is visible throughout the `with:` block and can be returned with `out`.
