# Converged Language, Content, and Presentation Surface

This chapter is the maintained authority for the final Arcweft authoring
surface shared by expressions, Dialogue/RichText, View construction, and
presentation lifecycle. More specialized chapters retain their detailed
runtime and layout contracts, but conflicting source examples or aliases in
those chapters are obsolete and must be migrated to this surface.

## One syntax owner per operation

Arcweft separates operations by semantic role:

```text
construct a value, content value, or message
    ordinary expression / ordinary call

perform an effect without changing continuation structure
    ordinary call

suspend, propagate, branch, transfer, or transition
    keyword expression or statement

obtain persistent identity as a value
    typed @ reference

operate on an existing resource, state owner, or handle
    real inherent or trait method
```

The language must not add a second parser, callable catalog, argument mapper,
or string-based resolver for Dialogue, View, or presentation spelling. Shared
syntax is projected into the legitimate checked owner for its context.

## Lexical symbols and persistent identity

Declaration names are ordinary lexical symbols:

```arcw
SettingsPanel()
feedback.submit(value = message)
opening()
```

Persistent references are distinct typed values. Family-relative references
remain the canonical hand-authored form when a family default prefix exists:

```arcw
@view:.settings_panel
@action:.feedback.submit
@flow:.next
@asset:.bg.room
```

Fully qualified references such as `@asset.bg.room` are for generated source,
stored round trips, manifests, tooling, and external interfaces. `@.name` is
accepted only where the enclosing typed construct already owns the family,
such as a line, option, control, or mark identity. Parent-relative authored IDs
use `@super.name` and `@super.super.name` as the canonical spelling.

Persistent identity must not be derived from a lexical binding name,
`SyntaxNodeId`, HIR ID, source order, display label, or formatter output.

## Unary Need, Await, and prefix Try

`Need<T>` is the one-shot temporal carrier. It owns pending/ready/cancelled
state but no domain-error type. Await removes exactly that temporal layer:

```text
await : Need<T> -> T effects { control.suspend }
```

Fallible asynchronous work returns `Need<Result<T, E>>`; optional work returns
`Need<Option<T>>`. `try` is an ordinary prefix expression over the Result or
Option payload:

```arcw
let config = try load_config()
let image = try await load_avatar(user) with:
    pending progress:
        progress_bar.set(progress.ratio)
```

`with` belongs to the Await expression. `try await` is therefore ordinary
composition, not a special Await or Let form. Arcweft has no postfix `?`, no
`await?`, and no `HirTryForm::PostfixQuestion` compatibility carrier.

Local carrier boundaries use `result {}` and `option {}`. Normal tails are
wrapped in Ok/Some without flattening. `try { ... }` remains ordinary
`Try(Block)` and creates no boundary; `need {}` is not introduced.

`await _` constructs the ordinary `Need<T> -> T` implicit callable. Bare
`try await _` normally fails because that callable has no matching carrier
boundary. Canonical carrier-returning partials use
`result { try await _ }` or `option { try await _ }`. Inside a pipeline,
`await ^` reads the once-evaluated pipe-left value and `try await ^` uses the
nearest existing carrier boundary; `^` creates no callable boundary.

Cancellation is a non-returning cancellation-scope transfer, not Result Err.
Await-specific `error` and `denied` handlers are removed; domain outcomes are
handled through the awaited payload. Pending remains a temporal observer.

## Calls, methods, and pipelines

`receiver.method(args)` resolves only a real inherent or visible trait method.
It never falls back to a same-named free callable. Data-last application uses
the pipe explicitly:

```arcw
value |> transform(options)
raw_score |> clamp(0, ^, 100)
```

`^` denotes the current pipe-left value. `_` remains the ordinary
partial-abstraction or element placeholder. API additions must not silently
change an old free-call fallback into a method call.

## Dialogue content code boundaries

Dialogue/RichText mode uses these code escapes:

```arcw
#name
#profile.display_name
#[arbitrary_expression]
#format_score(score)
#strong()[important]
#fuga()[#qux()[text]]
```

`#call(args)` evaluates during content construction or reactive evaluation and
inserts its accepted content result. It is not required to be pure merely
because it uses `#`; the ordinary effect row of the enclosing Flow, View, or
static-certification context decides whether its effects are allowed. Its
result must satisfy the checked content-root contract. `Unit` or an arbitrary
runtime value is not silently converted into content.

`$(expr)` is removed. It is not retained as an alias or migration reader.

## Attached content roles

Every attached block is parsed losslessly with the complete content grammar.
The selected callee schema and surrounding expected role decide admission:

```text
InlineContent
    text, interpolation, Ruby, nested content calls

RichContent
    InlineContent plus hard line breaks

DialogueContent
    RichContent plus page/wait/state controls, marks, and timeline calls
```

These are checked admission roles, not three parallel runtime wire models.
They lower to one renderer-neutral checked content sequence.

The closed attached-body policies are:

```text
PreserveBodyRole
    strong, em, color, font, size, layout, transform, effect, fx

InlineOnly
    ruby

RichOnly
    object and text-proxy/hit-region owners

LiteralOnly
    raw

DeclaredRole
    project/user content callables
```

Consequently this is valid in Dialogue content:

```arcw
#strong()[text[p]]
```

Lexical modifiers apply only to displayed fragments. Timeline controls retain
their order and identity, are not styled, and do not end the lexical modifier
scope. Thus both `A` and `B` are strong below, while Page remains an unstyled
control between them:

```arcw
#strong()[A[p]B]
```

Ruby's base is `InlineContent`, so the corresponding page control is rejected
by the ordinary role check:

```arcw
#ruby("きょう")[#strong()[今日[p]]]
```

`[clear]` clears displayed state and `[reset]` resets sequential line state;
neither closes an enclosing lexical content modifier.

## Content calls, timeline calls, and marks

Content construction and reveal-time execution are different operations:

```arcw
#fuga(args)
    evaluate now and insert the checked content result

[call fuga(args)]
    execute a zero-width timeline operation when reveal reaches this point

[mark @.point]
    publish a typed zero-width line-local identity
```

`[call]` accepts only the existing typed timeline-call result/failure contract;
it does not discard arbitrary values. `[! ...]` is removed.

An unknown dot form such as `[.hoge]` never falls back to a custom call, mark,
layout, or effect. A closed zero-argument builtin such as `[.sparkle]` may exist
only when its owning enum declares that exact shorthand. Custom content,
timeline work, and marks use `#hoge()`, `[call hoge()]`, and `[mark @.hoge]`
respectively.

## Ruby

All Ruby surfaces lower to the existing single checked Ruby owner. Surface
choice is syntax/formatter information and does not enter HIR, runtime plans,
bundles, saves, renderers, or Agent values.

The retained forms are:

```arcw
#ruby("へんなゆめ")[変な夢]
｜変な夢《へんなゆめ》
|変な夢《へんなゆめ》
|[変な夢](へんなゆめ)
```

The reading is the required positional operand of `ruby`; the attached body is
the required `InlineContent` base. Only typography settings already owned by
the checked Ruby schema may be named options.

Compact curly Ruby and paired tag Ruby are removed:

```text
|base{reading}
[ruby rt=...]...[/ruby]
[rb rt=...]...[/rb]
```

No external text-service codec, interchange profile, or parallel segmented
Ruby model is added. Per-character Ruby is represented by adjacent Ruby nodes.

## View uses shared expressions and retained projection

View bodies use ordinary call, block, `if`, `match`, and `for` syntax. View
sema still projects them into retained View-owned branch, repeat, fragment, and
subscription owners; using shared syntax does not turn them into one-shot Flow
execution.

`AwaitView` is removed. Reactive `Need` observation uses ordinary `match` in a
View context. Domain Result is nested inside Ready rather than duplicated as a
Need error state:

```arcw
match load_avatar(user) {
    .pending(progress) => SkeletonCircle(progress = progress)
    .ready(.Ok(image)) => Image(image)
    .ready(.Err(error)) => ErrorMessage(error)
}
```

The checked View projection owns state coverage, subscription identity,
branch switching, cancellation, mount occurrence, save/replay, and hot reload.
Outside View, `await` remains continuation suspension; its meaning is not
changed by expected type.

## Presentation lifecycle and actions

Construction and mounting are separate:

```arcw
let value = SettingsPanel()
let panel = mount(value)
panel.release()
```

`mount` is an ordinary resolved intrinsic with a presentation effect. The
enclosing presentation scope owns mount lifetime; dropping or ignoring a
returned handle does not immediately unmount the value.

Action declarations own typed channel identity and payload schema:

```arcw
pub action feedback.submit(value: String)

emit(feedback.submit(value = message))
let submitted = await receive(feedback.submit)
```

`action.invoke` string dispatch and a parallel action schema are not retained.
The exact `emit`/`receive` continuation shape is owned by the common callable
and suspension contracts.

## Smaller retained decisions

- Choice compact arms keep `->` for `goto` and `=>` for `out`; complex behavior
  uses the full option block.
- Navigation named arguments use ordinary `=` spelling.
- Closed enum shorthand uses `.Variant` under an expected enum type; bare names
  remain lexical symbols or owner-specific property keywords.
- Typed Style property names may remain hyphenated. Type safety comes from the
  owning property enum and value schema, not from converting names to
  `snake_case`.
- The old flat-fence dialogue syntax has no stable parser success path.

## Required implementation shape

Every migration is deletion-driven and vertical:

```text
syntax -> HIR -> sema -> compiler/runtime plan -> runtime/AWBC
       -> formatter/LSP/preview -> fixtures/docs
```

Do not publish a syntax-only alias, source-string reparse, copied content
schema, fallback resolver, or renderer-side reinterpretation. Limits for
nesting, nodes, calls, arguments, and authored bytes belong to the existing
content construction authority and fail atomically.
