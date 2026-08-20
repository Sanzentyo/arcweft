# Post-Try convergence implementation order — 2026-08-18

## Status and inspected state

- Inspected Git revision: `e0bcd7148f36ef54d166092321039e656d96043e`.
- Working tree at inspection: clean on `main`, matching `origin/main`.
- This is a temporary implementation-sequencing authority. It does not replace
  the maintained language or runtime specifications.
- For ordering only, this consolidates and supersedes the shorter remaining-work
  lists in
  [Unary Need and carrier-boundary adoption](2026-08-18-unary-need-carrier-boundary-adoption.md),
  [Const phase fence and Need timeout contract adoption](2026-08-18-const-timeout-contract-adoption.md),
  and
  [Checked prefix Try carrier CFG](2026-08-18-checked-prefix-try-carrier-cfg.md).
  Their established behavior and validation evidence remain authoritative for
  their respective dates.

## Sequencing rules

Every numbered item is a reviewable deletion-driven cut. Do not begin a later
cut by adding a compatibility path around an unfinished earlier owner. When two
adjacent items cannot leave a compiling intermediate state, land them as one
atomic commit while retaining the order below inside that transaction.

Each Rust cut must use the validation tier selected by
[Test execution policy](test-execution-policy.md). The positive Arcweft fixture
directories remain executable acceptance evidence; a filename allowlist,
manifest exclusion, source-spelling gate, or zero-test replacement is not an
acceptable closure.

## Completed baseline

The following prerequisites are already on `main`:

1. Agent protocol Await response bindings consume the canonical Result shape
   and closed typed protocol-record field coordinates.
2. Prefix Try owns one checked `Result`/`Option` carrier and lexical boundary
   fact, and Flow lowering projects that fact to generic typed CFG. Await has no
   Try-specific propagation branch, and production has no postfix-question
   carrier.

The remaining implementation sequence starts here.

## 1. Close the generic Try and implicit-callable boundary

Extend the checked Try fact beyond the current Flow CFG path without adding a
Try-specific placeholder model.

- Lower Try in every executable callable family that can own a propagation
  boundary, including pure/function execution where applicable.
- Elaborate `_` through the ordinary partial-abstraction boundary and `^`
  through the pipe-left once-only binding.
- Type `try _`, `try ^`, `await _`, `await ^`, `try await _`, and
  `try await ^` by normal composition. Accept or reject each from its inferred
  callable type; do not add spelling-specific bans or fused variants.
- Keep `HirTryExpr { operand }` as the sole Try HIR. Do not restore postfix `?`,
  `await?`, TryAwait, TryPipe, or TryPartial.

Exit evidence includes checked callable facts, one-evaluation pipe tests,
partial-abstraction boundary tests, diagnostic source ranges, and executable
Flow/callable parity.

### Dependency correction discovered during implementation

This item has two implementation faces and must not be treated as permission
to repair the binary-Need execution path.

1. The checked-expression face is independent: `_` owns one implicit callable,
   `^` owns one pipe-left binding, and Try records the nearest carrier,
   function-site, or callable boundary. These facts may be projected into the
   runtime-plan fact vocabulary before unary Need lands.
2. Pure/function execution needs a generic continuation transform that lifts
   the surrounding pure expression into the boundary carrier. A Try node alone
   cannot be represented as an ordinary `RuntimeExpr`, because its success arm
   has the unwrapped type while its residual arm exits with the enclosing
   carrier type.
3. Await compositions cannot close on the current runtime path. The current
   runtime-plan Await lowerer starts a direct checked host call; it cannot await
   a local or implicit-callable parameter. Implementing `await _` or
   `try await _` against binary `Need<T, E>` would add a success path to the
   owner that item 2 deletes.

Therefore the checked boundary and pure continuation transform proceed first,
item 2 replaces Need and makes Await consume an ordinary unary-Need value, and
then the Await-containing item-1 matrix closes before item 3 begins. The item-1
exit evidence is not complete until that final composition pass is green.
Do not add a direct-host-call exception, a binary-Need implicit callable, or a
fused TryAwait representation as an interim solution.

### 1A. Replace implicit dot fallback with explicit extension receivers

After the ordinary no-placeholder data-last pipe is verified as function-value
application, implement
[Explicit extension receiver implementation plan](2026-08-19-explicit-extension-receiver-implementation-plan.md).
This cut is independent of binary/unary Need and may land before item 2. It
adds the typed `self: Type` receiver coordinate to ordinary functions, indexes
only explicitly opted-in functions for dot lookup, migrates standard
`map`/`filter`/`fold`, and deletes the old name-and-type-matched data-last method
fallback. The pipe remains generic Apply and does not consume the extension
index.

### Pure continuation progress — 2026-08-19

The independent pure-expression face is now implemented and recorded in
[Nested pure Try continuation lowering](2026-08-19-nested-pure-try-continuation-lowering.md).
Sequential statements, strict expression composition, ordinary and carrier
blocks, branch-local If/Match/IfLet values, and short-circuit operators lower
through the checked carrier boundary without a new runtime expression family.
Match/IfLet guards remain fail-closed pending a pattern-scope-local continuation
cut. Item 1 as a whole remains open because its Await compositions still depend
on item 2; this progress does not authorize work on the binary Need path.

## 2. Replace binary Need with unary `Need<T>`

The current source owner inventory and the required compiling transaction are
recorded in
[Unary Need atomic boundary audit](2026-08-19-unary-need-atomic-boundary-audit.md).

Perform the type and runtime carrier replacement across syntax-facing types,
sema, accepted callable signatures, compiler facts, RuntimePlan, structured
runtime, AWBC, codecs, snapshots, saves, adapters, and tools. Delete the binary
owner and all aliases/readers in the same transaction.

The physical temporal contract is:

```text
Need<T>
    NotStarted | Pending(Progress) | Ready(T) | Cancelled

await Need<T>
    T, or non-returning cancellation/runtime failure
```

Await must not universally wrap `T` in Result. A fallible producer declares
`Need<Result<T, E>>`; only that declared output makes the Ready payload a
Result. Keep every Arcweft-owned version marker at `1` and evolve the
unreleased wire shapes in place.

Items 2 and 3 are consecutive migration faces of one temporal contract. If
deleting the old observer branches is required for a green build, land both in
one atomic commit rather than retaining a binary-Need compatibility surface.

## 3. Converge Await observers and reactive View observation

Delete Await-specific Error and Denied branches from parser projection, HIR,
sema, runtime plans, structured runtime, AWBC, formatter, LSP, and fixtures.
Await observers retain temporal observation such as Pending; payload domain
errors are ordinary Result values after Await.

Delete `AwaitView`. A View observes unary Need through ordinary reactive
`match` and the retained typed View projection:

```arcw
match load_avatar(user) {
    .pending(progress) => SkeletonCircle(progress = progress)
    .ready(.Ok(image)) => Image(image)
    .ready(.Err(error)) => ErrorMessage(error)
}
```

The View projection remains the owner of subscription identity, branch
coverage, cancellation, mount occurrence, save/replay, and hot reload. It does
not gain Flow-style suspension.

## 4. Make producer outcome classification authoritative

Project one checked producer contract through registration, adapters,
scheduler, runtime, AWBC, and persistence. Every outcome must have exactly one
of these meanings:

```text
synchronous admission rejection
    Result<Need<T>, AdmissionError>

asynchronous domain failure
    Need<Result<T, DomainError>>

structured cancellation
    non-returning Cancelled control outcome

runtime / transport / ABI / verifier failure
    runtime fault, never a fabricated domain error
```

Producer manifests and accepted callable facts own the exact Result and
nominal error identities. Runtimes and adapters consume those facts; they must
not reconstruct contracts from capability names, return spellings, or error
labels. Close native, host-adapter, scheduler, save/replay, and structured/AWBC
parity before moving to presentation syntax.

## 5. Converge Dialogue and RichText vertically

Implement the maintained checked-content model from syntax through HIR, sema,
compiler content plans, runtime/AWBC, formatter/LSP, renderer, Agent capture,
and fixtures. Delete the replaced surfaces rather than retaining readers:

- remove `$(...)` interpolation;
- remove compact-curly Ruby and paired Ruby tags;
- remove unknown-dot custom-call/layout/effect fallback;
- retain checked `#[expr]` interpolation;
- retain typed content calls `#name(args)` and attached content
  `#name(args)[content]`, including nested forms such as
  `#fuga()[#qux()[text]]`;
- allow lexical modifiers such as `#strong()[text[p]]` while keeping timeline
  controls ordered and unstyled;
- keep reveal-time `[call name(args)]` distinct from immediate content
  construction;
- use typed `[mark @.name]` identities. A closed `[.name]` shorthand exists
  only when its owning builtin enum declares it.

Attached body roles must be checked by the declared content schema. Parser
name tests, renderer reinterpretation, and source-text fallback are forbidden.

## 6. Connect the typed line-plan vertical slice

Status: typed syntax/HIR/sema ownership landed at `15ad861a9`; executable
runtime/AWBC handle and result ownership is blocked on
[AW-AH-009.4.4.1 line-plan runtime handle/result authority reconciliation](../reviews/requests/2026-08-21-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation.md).

Use
`tests/fixtures/arcw/spec_should_pass/run/011_dialogue_line_value_and_handle_discard.arcw`
as the primary end-to-end acceptance row. Preserve its typed meanings through
syntax, attachment, HIR/source identities, sema, runtime facts, RuntimePlan,
structured execution, AWBC, save/replay, and CLI execution:

- the Dialogue application owns its inline line plan;
- `let cue = at(0.42s): ...` retains both the typed schedule and binding;
- line-local acquired actor, cue, and voice handles keep their exact lifetimes;
- `out (voice, cue)` is the line-plan value authority;
- destructuring and discarded values do not synthesize string handles or fake
  task specifications.

Also preserve the simpler mark-triggered
`current_pass/check/011_dialogue_with_plan.arcw` row. Do not revive a detached
LinePlan model or a raw/string statement reader to make either fixture pass.

## 7. Close the positive fixture gate directory-wide

Status: `current_pass/check` rows `001` through `008` pass after the typed
LetElse closure recorded in
[2026-08-21-let-else-positive-fixture.md](2026-08-21-let-else-positive-fixture.md).
The current first failure is `009_choice_static_goto.arcw` in final semantic
expression typing.

Run and repair the positive check/run directories in deterministic path order.
Keep every remaining positive fixture active and unchanged unless the selected
stable surface explicitly requires a typed migration. The currently known
first positive blocker is
`current_pass/check/008_let_else_diverge.arcw`; fix its owning typed
transaction before advancing to the next failure.

For each exposed failure:

1. identify the exact parser/HIR/sema/runtime owner;
2. fix or migrate that owner vertically;
3. run the exact fixture and its focused owner tests;
4. resume the directory gate from the beginning.

Do not introduce a fixture manifest allowlist, expected-gap status, filename
exception, source scan, or deleted test target. Negative directories continue
to assert their exact typed diagnostic families.

## 8. Implement the Const phase fence

After the language/runtime carrier and presentation convergence above is green,
implement `const { ... }` as the documented compile-time phase fence: checked
phase facts, closed value/type admission, typed constant interning, bounded
ConstEval verification/VM, cache identity, diagnostics, and deterministic
artifacts. Do not add a `Const<T>` wrapper, runtime evaluator, or arbitrary
native callback.

## 9. Implement deterministic Need timeout

Implement `timeout(Need<T>, Duration) -> Need<Result<T, Timeout>>` only after
unary Need, producer classification, and persistence are authoritative. The cut
includes the checked standard intrinsic, logical-time reducer, wait-local
cancellation, Progress forwarding, RuntimePlan/AWBC, codec/verifier/VM,
save/replay/hot reload, and structured/AWBC parity. Do not add Await timeout
syntax or flatten nested producer Result values.

## Update rule

When one numbered cut completes, record its commit and validation in a new
dated implementation note, then update this file by removing the completed
item or marking only the established baseline. If evidence changes a dependency
or reveals an underdesigned boundary, amend this sequence explicitly before
starting an ad hoc implementation. This file should be deleted once all items
have durable implementation records and the directory-wide fixture gates are
green.
