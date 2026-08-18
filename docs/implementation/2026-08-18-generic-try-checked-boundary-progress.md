# Generic Try checked boundary progress — 2026-08-18

## Inspected state

- Inspected Git revision: `ea32f4254d64e57103c2af919d0a0ee1fefae6c7`.
- Working tree: dirty on `main`; this note records the active generic-Try cut,
  not a completed implementation claim.
- Sequencing authority:
  [Post-Try convergence implementation order](2026-08-18-post-try-convergence-order.md).

## Performed

- Replaced the binary-expression-only `_` handling with one checked implicit
  callable owner. Placeholder facts refer to that owner, captures are retained
  by typed `LocalId`, and an explicit closure or implicit callable can be the
  nearest Try propagation boundary.
- Added one checked pipe fact. Every `^` refers to that pipe owner and receives
  the exact checked type of the once-evaluated left operand; it does not create
  a callable boundary.
- Added generation-bound runtime-plan facts for implicit callables and pipes,
  including parameter/result types, placeholder identities, captures, and
  strict admission against the accepted HIR generation.
- Projected direct and implicit-callable-body Try facts through the compiler,
  including the new function-site boundary owner.
- Admitted builder-owned parameter locals for implicit callables and one
  builder-owned local for every pipe. Non-Try implicit callable bodies lower
  through ordinary function sites, and pure/Flow pipelines bind the left value
  once before replacing every checked `^` use.
- Lowered a terminal function-site Try as one typed carrier Match: the success
  payload is rewrapped for the function result and the residual payload leaves
  through the same carrier. This closes the direct contextual `try _` body
  without adding an early-return expression variant.
- Kept `HirTryExpr { operand }`, ordinary placeholder HIR, and ordinary Pipe
  HIR as the only syntax/HIR authorities. No TryAwait, TryPipe, TryPartial,
  postfix-question, or Await propagation variant was added.

## Passed

- `cargo check -p arcweft-lang-sema --lib`.
- `cargo test -p arcweft-lang-sema --lib`: 201 passed, 0 failed.
- `cargo check -p arcweft-runtime-plan --lib`.
- `cargo test -p arcweft-runtime-plan --lib`: 47 passed, 0 failed.
- `cargo check -p arcweft-compiler --lib`.
- `cargo test -p arcweft-compiler --lib`: 52 passed, 0 failed.
- `cargo test -p arcweft-compiler --test try_pipe`: 1 passed, 0 failed;
  a source-authored pure pipeline compiled through the checked once-only local.
- Focused semantic tests establish:
  - a contextual `try _` uses its checked implicit callable as the propagation
    boundary;
  - `^` refers to one checked pipe owner without creating an implicit callable;
  - existing contextual placeholder overload selection remains accepted; and
  - existing unconstrained partial-binary inference remains accepted.

## Blocked completion

The executable item-1 matrix is not complete.

1. `RuntimeExpr` has no early-return form. A terminal function-site Try is now
   normalized directly, but a Try nested inside a larger pure helper/function
   expression still needs a continuation transform over the complete
   surrounding expression so the success continuation and residual exit both
   produce the enclosing carrier. Lowering that nested Try leaf in isolation
   would be ill-typed.
2. Runtime-plan Await currently accepts a direct checked host call and starts
   the task at that operation. It cannot await an ordinary local or function
   parameter, so `await _` cannot be an executable implicit callable.
3. The current Await/Need fact is binary `Need<T, E>`. Adding a special
   binary-Need callable path would repair the owner selected for deletion by
   the unary-Need cut.

The sequencing document now requires the independent pure continuation work,
then unary Need and generic Await-value lowering, then the Await-containing
item-1 composition matrix. This is an explicit dependency correction, not a
compatibility exception.

## Structural review triggers

- `Analyzer::check_expression` remains the sole expression-fact transaction.
  Placeholder discovery delegates to a bounded tree walk, while implicit
  callable construction, pipe checking, and Try boundary resolution remain
  separate typed operations. Call arguments stop `_` ownership at their own
  contextual boundary; explicit Pipe ownership deliberately traverses call
  arguments for `^`.
- `RuntimePlanSemanticFacts::try_new` remains the atomic generation-admission
  owner. The new families add validation and immutable maps to that transaction
  rather than creating a second fact registry.
- `reserve_function_sites` delegates implicit-callable reservation to a
  cohesive helper. Builder handles, parameter locals, captures, definitions,
  and output maps remain in the same reservation transaction.
- `FinalFlowLowerer::lower_flow_value_with_overrides` delegates Loop and Pipe
  continuations to focused helpers. Pipe binding order is explicit: lower the
  left value, bind the admitted local, then lower the right expression with
  only the checked `^` identities replaced.

No new source-string resolver, compatibility reader, detached side table, or
versioned boundary was introduced.

## Not run

- Structured/AWBC Try callable parity was not run because executable callable
  lowering is the remaining boundary described above.
- The combined strict Clippy command was attempted. It stopped first in
  unchanged dependency code (`arcweft-adapter-context` missing panic docs and
  `arcweft-lang-syntax` large enum), and `--no-deps` exposed additional
  pre-existing strict warnings in sema/compiler. The changed runtime-plan lib
  itself passed strict `--no-deps -D warnings` after cohesive helper splits.
