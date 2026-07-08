# Function Stack Current Status - 2026-07-08

Current pointer: see
`docs/implementation/function-stack-current-state-2026-07-09.md` for the latest
entry point and `docs/implementation/function-stack-status-rollup-2026-07-09.md`
for the latest goal rollup. This file remains the 2026-07-08 status index and
evidence map.

This note is the current implementation-status index for the active
function/closure/currying/pipeline goal. It summarizes what is implemented,
what remains implementation-ready, and what should stay in request/design space.

Primary implementation evidence lives in
`docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`.
Related slices are:

- `docs/implementation/2026-07-07-call-select-unification-refactor.md`
- `docs/implementation/relative-runtime-id-boundaries-2026-07-07.md`
- `docs/implementation/view-resource-rename-2026-07-08.md`
- `docs/implementation/current-work-status-2026-07-08.md`
- `docs/implementation/function-stack-expression-source-range-coverage-2026-07-08.md`
- `docs/implementation/function-stack-request-split-audit-2026-07-08.md`
- `docs/implementation/function-stack-goal-completion-audit-2026-07-08.md`
- `docs/implementation/function-stack-non-helper-callable-inventory-2026-07-08.md`

## Current Repository State

- Current pushed function-stack baseline after the latest refresh is
  `486738b31 Handle pipe control-expression RHS placeholders`.
- At the start of the enum-shorthand evidence refresh, `main` and
  `origin/main` were aligned at
  `396a6831 Audit function stack goal completion evidence`.
- The last completed implementation slices were the source-range follow-up for
  thread expression body statement ranges, the authored-payload conversion for
  `Stmt::Signal` / `Stmt::LifetimeSet`, and the expression source-range
  coverage matrix with typed statement branch source fixes.
- The current dirty files are unrelated View rendering, font, sample, Web
  IME/player, and runtime-driver text-input files. They include display-output
  encoding changes, emoji font registration, EditContext printable-key
  handling, runtime-driver text-input/session handling, and modern-feedback
  sample and sidecar-test changes around removing the dead name submit route.

Those View/Web/text-input changes are not part of this function-stack status
note and should be handled as their own rendering/IME/sample slice if they are
still desired.

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
- Expected-type enum shorthand is verified for user-defined unit variants,
  tuple payload constructors, and record payload constructors. The runtime-plan
  evidence confirms those short constructors lower to `RuntimeExpr::Variant`
  rather than through a `DataFormat`-only special case or a plain record.

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
  callables instead of emitting incomplete adapter calls. The current
  helper-less signature partial rejection is marked as unsupported callable
  family `signature_partial_without_helper`.

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
  dialogue-call line-plan colon blocks such as `let cue = at(...):`, thread
  expression body statement sources, lifetime registry write values, and typed
  statement branch payloads such as `let-else`, statement `while let` guards,
  and statement `match` arm guards/bodies.
- `TypeCheckStats` records source-backed and source-missing expression
  judgment counts for report-level auditing of expression source/inlay
  coverage.

### Runtime ID Boundary

- Runtime lookup IDs are no longer raw string newtypes such as
  `FlowRuntimeId("flow.main")`.
- Runtime IDs use typed `RuntimeIdPath` values.
- Source references, canonical runtime IDs, and public/debug labels are
  separate domains.
- AWBC flow target lookup now uses typed `FlowRuntimeId` keys in the compiler
  inventory. Static `goto`, choice targets, entries, and route targets no
  longer resolve through a public-label function map, and the old general
  public-label function index has been removed from `AwbcInventory`.
- The atom-table representation from the seq-07.6 package is deliberately
  deferred until profiling shows it is needed.

## Implementation-Ready Remaining Work

The previously identified AWBC flow-target runtime-ID cleanup is implemented.
The enum-shorthand evidence gap is also implemented with focused sema and
runtime-plan coverage. No additional concrete implementation-ready
function-stack item is currently identified from the status index without
either finding another typed-key cleanup site in code or receiving more design
for the items below.

Continue runtime-ID cleanup only for concrete lookup/index maps that still use
public strings where an owned typed key exists. Do not redesign AWBC/schema
public strings or add an atom table without profiling evidence.

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
   `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`
   also covers the versioned representation and restore contract. Current
   Product AWBC save/load behavior is explicit structured rejection.
4. General non-helper/effectful/suspending top-level callable allocation as
   first-class runtime function values. Helper-backed and local-function paths
   are implemented. The first narrow non-helper source-local `fn` expansion,
   including curried groups and returned simple closure literals in that
   accepted family plus direct calls to function-typed parameters and local
   callback aliases/partials, is recorded in
   `docs/implementation/function-stack-non-helper-source-function-values-2026-07-09.md`.
   Broader call-bearing/effectful/suspending callable allocation remains split
   to
   `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`.
5. Full closure effect-row integration. The implemented effect composition is
   broad, but the stable effect-row contract for closure captures remains a
   larger language/modeling decision split to
   `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`.
6. Atom-table runtime ID storage. The typed path API is in place; interning is
   deferred until there is measured ID comparison, hashing, serialization, or
   allocation pressure, as recorded in
   `docs/reviews/requests/2026-07-07-seq-07.6-relative-runtime-id-boundaries.md`
   and `docs/implementation/relative-runtime-id-boundaries-2026-07-07.md`.

The requirement-by-requirement completion audit is recorded in
`docs/implementation/function-stack-goal-completion-audit-2026-07-08.md`.
That audit is the current reason the active goal remains open rather than
being marked complete.

## Not Part Of This Goal

- Pinned visual baseline promotion and exact PNG golden updates.
- Native/Web View rendering parity issues, IME player behavior, backdrop
  filters, radius/shadow rendering, and the current dirty `web/` files.
- Further View-resource naming cleanup beyond the committed View rename slice.

## Recommended Next Slice

The source-range/inlay audit is represented by the coverage matrix, the known
AWBC flow-target runtime-ID cleanup has been implemented, and the enum
shorthand evidence gap now has focused sema/runtime-plan coverage. The larger
remaining goal items now have request/design files and should stay in
request/design space until those contracts are returned, unless another
specific typed-key cleanup site is found by code audit.

Do not fold the current dirty View/Web rendering, runtime-driver text-input,
and IME-player files into this goal. They belong to the View/rendering/text-input
track and need their own validation evidence before commit.
