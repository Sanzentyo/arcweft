# Await, unary Need, carrier blocks, and `try`

This chapter is the maintained authority for temporal values, suspension,
Result/Option propagation, and their lexical boundaries.

## Orthogonal owners

Arcweft assigns one meaning to each construct:

```text
Need<T>
    one-shot temporal carrier

await Need<T>
    remove exactly the temporal layer and produce T

Result<T, E>
    success or domain failure value

Option<T>
    presence or absence value

try
    propagate exactly one Result or Option residual

result { ... } / option { ... }
    local carrier propagation boundary and success wrapping
```

`Need` has no domain-error type parameter. A fallible asynchronous operation
returns `Need<Result<T, E>>`; an optional asynchronous operation returns
`Need<Option<T>>`.

```arcw
delay(1s)             // Need<Unit>
load_image(path)      // Need<Result<Image, LoadError>>
choose_action()       // Need<Option<Action>>
```

This is a direct replacement for unreleased `Need<T, E>`. The unary and binary
forms must not coexist behind aliases, dual readers, or fallback projection.

## Await

The type rule is stable for every payload:

```text
await : Need<T> -> T effects { control.suspend }
```

```arcw
await delay(1s)

let outcome = await load_image(path)
// outcome: Result<Image, LoadError>

let image = try await load_image(path)
// image: Image
```

`try await value` remains ordinary syntax nesting:

```text
Try(Await(value))
```

Neither syntax, HIR, sema, nor runtime plan owns a fused TryAwait operation or
an Await propagation flag.

## Temporal state and cancellation

The conceptual Need state is:

```text
NotStarted
Pending(progress)
Ready(T)
Cancelled(reason)
```

- `NotStarted` follows the producer's checked start policy.
- `Pending` suspends the current fiber/call stack and notifies a temporal
  observer when one is present.
- `Ready(value)` resumes with exactly `value: T`.
- `Cancelled` performs non-returning transfer through the cancellation scope.
  It does not fabricate `T` and is not caught by ordinary Try.
- codec corruption, verifier failure, and impossible runtime invariants are
  runtime faults, not `Result::Err` payloads.

`Ready(Result::Err(error))` means temporal completion succeeded and the
operation's domain result is an error value. It is distinct from cancellation
and runtime failure.

Producer signatures distinguish when failure occurs:

```text
synchronous admission failure
    Result<Need<T>, AdmissionError>

asynchronous domain failure
    Need<Result<T, DomainError>>

both
    Result<Need<Result<T, CompletionError>>, AdmissionError>
```

## Await observers

`with` belongs to the Await expression. It observes temporal behavior; it does
not inspect the payload's Result/Option carrier.

```arcw
let outcome = await load_image(path) with:
    pending progress:
        progress_bar.set(progress.ratio)
```

Await-specific `error` and `denied` branches are removed. Handle domain errors
after Await or use Try:

```arcw
match (await load_image(path) with:
    pending progress:
        progress_bar.set(progress.ratio)
) {
    .Ok(image) => show(image)
    .Err(error) => show_error(error)
}
```

Whether a denied operation is an admission error, domain error, or cancellation
is determined by its typed producer contract, not an Await branch label.

`timeout` remains an explicit design gap until its race result, wait
cancellation, producer cancellation, and payload projection are specified. It
must not be retained as an untyped Await error alias.

## Prefix Try

`try` is the sole propagation surface for `Result` and `Option`:

```arcw
let config = try load_config()
let selected = try state.selection
```

It has one syntax and HIR shape:

```text
TryExpr := "try" Expr
HirTryExpr { operand }
```

Arcweft has no postfix `?`, `await?`, TryAwait, TryPipe, TryPartial, or special
TryBlock carrier.

```arcw
try {
    compute_result()
}
```

is ordinary `Try(Block { tail = compute_result() })`. The block does not create
a propagation boundary.

## `result {}`

`result` creates a local Result boundary. Normal completion wraps the tail in
`Ok`; Try targeting the block exits with its `Err` residual.

```arcw
let model: Result<Model, LoadError> = result {
    let config = try parse_config(source)
    let bytes = try await fetch(config.url)
    try decode(bytes)
}
```

The block does not flatten a Result-valued tail:

```arcw
result {
    parse(source)
}
// Result<Result<T, E>, OuterE>
```

Use `try parse(source)` when one layer should be removed.

With an expected `Result<Success, Error>` type, the block is checked
bidirectionally against both arguments. Without an expected type:

```text
no Result residuals
    Error = Never

one residual type, or mutually unifiable residual types
    Error = unified type

incompatible residual types
    type error; use explicit map_err/context/project enum conversion
```

`Result<T, Never>` remains a real nominal type and is not normalized to `T`.
An empty block produces `Ok(Unit): Result<Unit, Never>`.

`Err(error): Result<alpha, E>` takes `alpha` from the expected success type.
Only when no context exists may `alpha` default to Never. No separate `fail`
keyword is needed for carrier propagation.

## `option {}`

`option` creates the corresponding Option boundary. Normal completion wraps
the tail in `Some`; Try targeting the block exits with `None`.

```arcw
let label: Option<String> = option {
    let user = try users.get(id)
    let profile = try user.profile
    profile.label
}
```

An empty block produces `Some(Unit)`. An Option-valued tail is not flattened.

## Parsing carrier blocks

At expression start, `result` or `option` immediately followed by a BlockExpr
is always a carrier block. The parser uses the existing BlockExpr owner and
does not consult semantic name resolution or backtrack over the body.

Calling a lexical function named `result` or `option` with a callback block
requires an unambiguous parenthesized call form.

The HIR uses one carrier-block family:

```text
CarrierBlock(Result | Option)
```

It does not add ResultAwait, OptionAwait, TryBlock, NeedBlock, or other fused
variants.

## Lexical propagation boundary stack

Try walks outward through typed lexical owners:

```text
ordinary block / match arm
    pass through

result / option block
    first carrier boundary; matching family binds, mismatch errors here

explicit closure / implicit `_` callable / function / method / Flow
    hard callable boundary; matching return carrier binds, otherwise error

const block
    phase fence; residual cannot cross into a runtime boundary
```

An incompatible nearest boundary is never skipped to reach a compatible outer
one. Carrier blocks catch only Result/Option residuals; `return`, `break`,
`continue`, `yield`, `goto`, and cancellation retain their ordinary owners.

## `_` and `^`

`_` creates an implicit callable. Its body is the maximum enclosing expression
up to an explicit callable boundary. A carrier block does not stop that
abstraction.

```arcw
await _
// Need<T> -> T effects { control.suspend }

result {
    try await _
}
// Need<Result<T, E>> -> Result<T, E> effects { control.suspend }

option {
    try await _
}
// Need<Option<T>> -> Option<T> effects { control.suspend }
```

Bare `try await _` normally fails because the implicit callable returns `T`
and has no matching Result/Option boundary. This is an ordinary checked
boundary error, not a syntax ban or special diagnostic.

`^` reads the enclosing pipe's once-evaluated left binding and creates no
callable boundary:

```arcw
need |> await ^

result {
    need |> try await ^
}

need |> result {
    try await ^
}
```

Carrier blocks do not hide `^`; explicit callable boundaries do not implicitly
capture it.

## No `need {}`

`need {}` is not introduced. Such a block would need to own task start policy,
fiber identity, capture ownership, cancellation, drop behavior, save/replay,
generation pinning, and hot reload. Arcweft keeps these separate:

```text
producer call
    create Need

await
    wait for Need

thread/scheduler substrate
    concurrency

result / option
    local carrier construction
```

## `const {}` is a separate future contract

`const {}` is a compile-time phase boundary, not a carrier block. The direction
is accepted, but implementation requires a separate contract covering const
callables, specialization, captures, fuel, diagnostics, constant-pool/AWBC
lowering, and `ConstValueAdmissible<T>`.

Try residuals cannot cross a const phase fence. A carrier must be completed
inside const and then handled in runtime phase:

```arcw
result {
    let parsed = const {
        result {
            try parse_schema(TEXT)
        }
    }
    try parsed
}
```

Result/Option values are valid const outputs only when their contained types
are const-admissible. A const evaluator fault is a compiler diagnostic, not an
Err value.

## Stream remains distinct

Unary Need does not imply unary Stream. `Stream<T, E>` retains `E` as a
terminal protocol error after zero or more items. `Stream<Result<T, E>>` would
instead make Err an ordinary item and is not equivalent.

A typical next operation may therefore return:

```text
Need<Result<Option<T>, E>>
```

Need, Result, and Option each own one independent layer of meaning.

## See also

- [Result / Option, `try`, and Context](result-option-context.md)
- [Converged Language, Content, and Presentation Surface](converged-language-surface.md)
- [Never / Bottom Type](never-bottom-type.md)
- [Streams, generators, and live device sources](../02-runtime/streams-generators.md)
