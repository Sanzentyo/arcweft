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

## Local ownership and replacement

Each binding introduces a distinct local declaration and generation. Shadowing
with another `let` introduces a new declaration; assignment updates the existing
declaration when it permits assignment (`let mut`).

Move transfers the current value and makes that place uninitialized on the
affected path. Reading, borrowing, or mutating the missing value is an error.
Whole-place assignment initializes the existing declaration with a new value,
as in Rust. A subsequent read requires initialization on every reachable
incoming path. Consuming a value with drop has the same availability effect as
other moves; it does not destroy the declaration's assignable place.

```arcw
let mut items = Vec<Content>::with_capacity(0usize)
let moved = items
items = Vec<Content>::with_capacity(0usize) // initializes the existing place
```

Shadowing remains a distinct operation:

```arcw
let items = Vec<Content>::with_capacity(0usize)
let moved = items
let items = Vec<Content>::with_capacity(0usize) // new declaration
```

The assignment target is a place, not a value read. The right-hand side is
evaluated first, so `items = identity(items)` may move the old value and then
initialize the same place with the result. In-place mutation requires the
selected value to remain initialized. Active receiver loans prohibit consuming
or changing an overlapping place during operand evaluation. Initialization and borrow
obligations are checked statically, including control-flow joins and backedges.
An assignment transfers an existing old value to the owning runtime's cleanup
transaction exactly once. If the old value was moved, there is no old value to
clean up; when incoming paths differ, the retained slot's occupancy is the drop
flag, not a runtime decision about whether the source operation is legal.

The final ownership CFG seals an assignment's old-value cleanup contour after
the RHS: definite initialization, definite absence, or conditional initialization,
with schema-selected child paths for partial moves. The executable assignment
carries this same contour through the native plan and AWBC wire format. When
lowering distributes a continuation into branches, AWBC construction narrows
conditional source facts using its fixed-point CFG; definite facts cannot change.
The published artifact verifies exact initialization before admission;
runtime storage checks persisted drop flags before transferring the new value.
Only conditional facts require a dynamic drop decision. Definite source facts
must not be replaced by a conditional annotation.

Value-producing control executes in its enclosing frame. A pattern guard
borrows the candidate and materializes only its used, proven Copy bindings;
the successful arm subsequently moves the candidate into its actual bindings.
Guard fallthrough retains the candidate for later arms. Synthetic control
closures must not eagerly capture an enclosing owner before branch selection.

A local-rooted record field is its own place, selected by the admitted field
schema. Copy classification uses the selected field type. Moving a field leaves
its siblings initialized; a read or borrow of the whole record requires all its
children initialized. Field assignment can reinitialize a moved child of an
existing aggregate. It cannot initialize a child of a whole aggregate that was
moved away. Whole-record assignment replaces the remaining initialized children
and passes those owners to cleanup exactly once. A receiver reservation overlaps
its ancestors and descendants, while disjoint sibling places remain independent.
Opaque producer values retain their indivisible storage contract.

Stored record paths retain every schema-selected field coordinate, including
closed generic fields. The same complete path identifies reads, replacement,
in-place mutation, and receiver loans in native execution and AWBC. A mutable
receiver remains an address while its arguments execute; it is not read or moved
as an ordinary value. An expression whose result is discarded still executes
once, including mutations and cleanup.

Native declaration slots and AWBC registers retain partial record storage
separately from complete values. Rollback and AWBC snapshots preserve the record
header, defining field order and child initialization states. No missing child is
represented by Unit or a malformed value. Runtime occupancy elaborates cleanup;
availability is decided by the static ownership flow.

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

## Carrier and phase blocks

`result { ... }` and `option { ... }` reuse value-block scope and tail rules,
but additionally create typed Result/Option propagation boundaries. Normal
completion wraps the tail in Ok/Some; they do not flatten an already-carried
tail.

`const { ... }` also reuses the value-block shape, but is a compile-time phase
fence rather than a carrier boundary. Runtime locals and effects cannot cross
into it, and its admitted result is replaced by one typed constant before the
final RuntimePlan is published.

See [Await, unary Need, carrier blocks, and `try`](await-need-result.md) and
[Const block and compile-time phase fence](const-block.md).

## Match scope ownership

A `match` owns the semantic relationship between its scrutinee and ordered
arms, but its delimiter does not create a container-wide lexical scope. The
scrutinee is evaluated once in the inherited outer scope.

Each ordinary expression or statement arm creates its own distinct arm scope.
That scope is a direct child of the inherited outer scope and is owned by the
source-backed Match expression or statement. Pattern bindings are visible to
the guard and selected value or body of that arm only; they are not visible to
a sibling arm or after the Match.

An authored block used as an expression-arm value creates a nested Block scope
below the arm scope. The Block is not a replacement for the ordinary arm
boundary.

A braced Match arm inside a Thread body is the one exception to that two-level
shape: its single nested statement Block is itself the arm's lexical scope and
Thread-body owner. It does not also receive a parallel arm scope. These rules
never create one scope shared by all arms and never make an arm a child of a
nonexistent Match-container scope.

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
alice()[おはよう。[p]]
with:
    let voice = line.voice_handle()
    at(0.2s):
        let old = alice.current_face()
        alice.look(smile)
    out voice
```

`old` is visible only inside the `at` block. `voice` is visible throughout the `with:` block and can be returned with `out`.

