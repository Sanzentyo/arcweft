# Semicolon Policy

Arcweft is script-friendly: semicolons are not required for normal flow, dialogue, or scenario-style code.

```awft
flow @flow.opening {
    alice: おはよう。[p]
    alice: 今日はいい天気だね。[p]
}
```

However, semicolons remain useful after Arcweft gained expression-oriented `if`, `match`, `loop`, and block expressions.

## Why keep `;`?

Because expression-oriented blocks need a way to explicitly discard a final value.

```awft
fn answer() -> i32 {
    42
}

fn log_answer() -> Unit {
    answer();
}
```

Without `;`, the final expression `answer()` would try to return `i32`.

## Final rule

```text
newline:
  normal statement separator

;:
  optional separator
  explicit value-discard marker
  useful mainly for final expressions in value-producing blocks
```

## Statement-oriented vs value-producing blocks

Value-producing blocks:

```awft
let x = {
    let a = 1
    a + 1
}
```

The final expression is the block value.

Statement-oriented blocks:

```text
flow body
with: dialogue line plan
while body
for body
```

These do not export a value via final expression. Use explicit control transfer:

```awft
return expr
out expr
break expr
```

## Same-line statements

Allowed:

```awft
let a = 1; let b = 2
```

Formatter should rewrite to:

```awft
let a = 1
let b = 2
```

## Value discard

Two forms are valid:

```awft
expr;
let _ = expr
```

Use `let _ = expr` when discard is semantically important, especially with scoped handles:

```awft
let _ = se.play(@se.page_start)
```

If the discarded value implements `CancelOnDrop`, discarding triggers drop/cancel behavior.

Use `expr;` when the only goal is to make a final expression `Unit`:

```awft
fn debug_log() -> Unit {
    log_debug();
}
```

## Recommendation

```text
Dialogue/scenario code:
  avoid semicolons.

Typed expression-heavy helpers:
  use semicolon only for explicit value discard.

Handle/resource discard:
  prefer `let _ = expr` because it communicates drop/cancel intent.
```

Therefore Arcweft keeps `;`, but it is optional and uncommon in scripts.

