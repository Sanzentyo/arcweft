# Function Stack Current Status - 2026-07-08

This note is the current implementation-status index for the active
function/closure/currying/pipeline goal. It summarizes what is implemented,
what remains implementation-ready, and what should stay in request/design space.

Primary implementation evidence lives in
`docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`.
Related slices are:

- `docs/implementation/2026-07-07-call-select-unification-refactor.md`
- `docs/implementation/relative-runtime-id-boundaries-2026-07-07.md`
- `docs/implementation/view-resource-rename-2026-07-08.md`

## Current Repository State

- At the start of this audit, `main` and `origin/main` were aligned at
  `1db72d00c Document function stack status`.
- The current implementation slice is a focused source-range follow-up for
  thread expression body statement source ranges.
- The only dirty files outside that slice were unrelated Web IME/player files
  under `web/`.

Those `web/` changes are not part of this status note and should be handled as
their own slice if they are still desired.

## Implemented Goal Surface

### Syntax, AST, HIR, and Types

- Function types parse and type-check as `A -> B`.
- Function types are right-associative.
- Tuple call-group function types such as `(A, B) -> C` are represented.
- Function-like declarations preserve multiple curried `ParamGroup` entries.
- `flow` declarations reject curried parameter groups.
- Parser call/select syntax uses neutral `Expr::Select` plus `Expr::Call`
  instead of parser-level method/field variants.
- Expression operators are represented by typed `ExprOp` values instead of raw
  operator strings for parser decisions.
- Canonical primitive spelling is enforced:
  `bool`, `char`, `String`, `Unit`, `Never`, and explicit-width numeric
  primitives are the accepted surface.
- Non-canonical spellings such as `Bool`, `Char`, `string`, `int`, `uint`,
  `float`, and `Number` are rejected rather than normalized.

### Curried Calls and Function Values

- Top-level `fn`, `task fn`, `dialogue fn`, and `stream fn` declarations
  preserve curried call-group boundaries through parser/HIR/sema.
- Trait and impl member calls preserve curried method call-group boundaries.
- `f(a)(b)` and `f(a, b)` remain distinct.
- The sample `samples/function-curried-call-groups` covers:
  - `tuple_tail(a, b)(c) -> (i64, i64, i64)`
  - `chain(a)(b)(c, d) -> i64`
- Sema accepts calls through function-valued symbols and locals.
- Runtime lowering consumes typed evidence so function-valued path calls lower
  as `RuntimeExpr::Apply` instead of adapter-facing named calls.
- Helper-backed top-level function values materialize as runtime functions.
- Prefix partial calls to checked top-level function signatures are accepted
  when an executable helper path exists.
- Checked runtime-plan lowering rejects unsupported non-helper partial
  callables instead of emitting incomplete adapter calls.

### Closures and Runtime Apply

- Closure expressions type-check as function values.
- Typed closure parameters are supported.
- Closure parameter patterns are preserved, and pattern `_` remains distinct
  from expression `_`.
- Closure return type annotations are supported for braced closure bodies.
- Closure-local `return expr` binds to the nearest closure/function-like sema
  boundary for type checking.
- `RuntimeValue::Function`, `RuntimeExpr::Function`, and `RuntimeExpr::Apply`
  execute in the core evaluator.
- Runtime functions capture deterministic local bindings.
- Partial and curried runtime function application execute in the core
  evaluator.
- AWBC has a first executable non-suspending closure/apply cut:
  `MakeFunction`, synthetic function bodies, `ApplyFunction`, exact apply,
  partial apply, and chained apply.
- Product AWBC save/load explicitly rejects persisted runtime function values
  with a structured unsupported-value error instead of claiming snapshot
  compatibility.

### Placeholders, Pipes, and Method Fallback

- `^` is scoped to the right-hand side of `|>`.
- Pipe RHS with `^` substitutes the pipe LHS before checking/lowering.
- Pipe RHS without `^` uses data-last application.
- Helper-aware no-`^` pipe lowering works for named helpers.
- Local function-valued aliases work through no-`^` data-last pipes.
- Expression `_` placeholder abstraction works with an expected function type.
- Sema infers no-expected `_` abstraction for unambiguous binary expressions.
- Sema infers partial-call abstraction for known positional callable
  signatures.
- Repeated `_` in one supported partial-call region maps to one generated
  parameter when inferred parameter types agree.
- Named fixed-input and named missing-input partial application are implemented
  for checked top-level signatures that lower through annotated or inferred
  helper input names.
- Data-last method fallback resolves after real env/inherent/trait methods.
- Positional and named data-last method fallback record deterministic runtime
  argument order.
- Ambiguous module/environment data-last fallback reports all candidates.
- Real methods keep priority, and shadowed data-last fallbacks surface as
  warnings rather than overriding the real method.

### Capture, Effects, LSP, and Source Identity

- Sema records closure capture inventory keyed by stable expression IDs.
- Borrowed local captures crossing checked suspension boundaries are diagnosed.
- Closure body effects compose when the resulting function value is actually
  invoked, including local aliases, partial aliases, immediate closure calls,
  built-in `map`/`filter`, selected user-defined higher-order calls, curried
  callback groups, tuple/record destructured callbacks, built-in
  `Option`/`Result` payloads, and module/environment enum payload metadata.
- Returned closure callback effects are delayed until the returned function is
  called.
- LSP diagnostics surface closure effect trace evidence through related
  information.
- Numeric fallback lints exist for inferred closure bodies.
- LSP inlays cover inferred function-valued `let` bindings.
- Opt-in expression type inlays exist for source-backed expression judgments.
- Source ranges are threaded for many expression and statement families,
  including `let` RHS roots, function-like body values, control expressions,
  value-producing blocks, pipe/desugared expressions, selector expressions,
  dialogue interpolation/call expressions, action/defer/assignment/control
  transfer statements, container child expressions, computation blocks, braced
  closures, guarded `if let`, effect/prefix expressions, `wait(...)`,
  dialogue-call line-plan colon blocks such as `let cue = at(...):`, and
  thread expression body statement sources.

### Runtime ID Boundary

- Runtime lookup IDs are no longer raw string newtypes such as
  `FlowRuntimeId("flow.main")`.
- Runtime IDs use typed `RuntimeIdPath` values.
- Source references, canonical runtime IDs, and public/debug labels are
  separate domains.
- The atom-table representation from the seq-07.6 package is deliberately
  deferred until profiling shows it is needed.

## Implementation-Ready Remaining Work

These items can be advanced without redesigning the goal:

1. Audit the remaining expression source-range families against
   `docs/reviews/requests/2026-07-07-seq-07.4.1-function-stack-expression-source-range-inlays.md`.
   Many families are already implemented; the remaining useful slice is now a
   matrix-style audit that lists still-untested families, closes any discovered
   local gaps, and adds internal diagnostics or stats for judgments that should
   have a source range but do not.
2. Continue typed-runtime-ID cleanup at AWBC/report boundaries where internal
   maps still use public strings, but only where the AWBC schema/data-format
   boundary allows typed keys without a larger format change.

## Request/Design Remaining Work

These should stay as request or design work before implementation:

1. Spread partial and spread data-last fallback semantics:
   `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`
   is still the correct boundary. Current behavior intentionally rejects those
   shapes with structured diagnostics.
2. AWBC suspension-aware dynamic function apply and resume points:
   `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`
   remains open for resumable dynamic apply.
3. Serializable persisted closure snapshots:
   current Product AWBC save/load behavior is explicit structured rejection.
   Snapshot-compatible function values need a versioned closure representation
   and restore contract first.
4. General non-helper/effectful/suspending top-level callable allocation as
   first-class runtime function values. Helper-backed and local-function paths
   are implemented; broader callable allocation is tied to the AWBC/apply
   design boundary.
5. Full closure effect-row integration. The implemented effect composition is
   broad, but the stable effect-row contract for closure captures remains a
   larger language/modeling decision.
6. Atom-table runtime ID storage. The typed path API is in place; interning is
   deferred until there is measured ID comparison, hashing, serialization, or
   allocation pressure.

## Not Part Of This Goal

- Pinned visual baseline promotion and exact PNG golden updates.
- Native/Web View rendering parity issues, IME player behavior, backdrop
  filters, radius/shadow rendering, and the current dirty `web/` files.
- Further View-resource naming cleanup beyond the committed View rename slice.

## Recommended Next Slice

Finish the expression source-range/inlay audit as the next implementation
slice. It is local to syntax/sema/LSP tests, does not require a new runtime
contract, and directly reduces the largest remaining implementation-ready
ambiguity in the active goal.

After that slice, either:

- keep implementing source-range coverage until 07.4.1 can be closed; or
- stop and design the spread-partial or AWBC resumable-apply contracts before
  touching those runtime surfaces.
