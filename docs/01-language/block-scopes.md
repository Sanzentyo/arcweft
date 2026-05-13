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
while body
for body
```

They do not export a value via final expression. Use explicit transfer:

```awft
return expr
out expr
break expr
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
