# Function Stack Status Rollup - 2026-07-09

This is the current one-page map for the active function/closure/currying/
pipeline language-stack goal. It supersedes the "where are we now" role of
`docs/implementation/function-stack-current-status-2026-07-08.md` while keeping
that file as the detailed 2026-07-08 status index.

Status: **open**. The implemented surface is substantial, but the goal is not
complete because several requested end-to-end behaviors remain behind explicit
request/design boundaries.

## Evidence Trail

Primary implementation log:

- `docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`

Current supporting audits:

- `docs/implementation/function-stack-current-status-2026-07-08.md`
- `docs/implementation/function-stack-goal-completion-audit-2026-07-08.md`
- `docs/implementation/function-stack-request-split-audit-2026-07-08.md`
- `docs/implementation/function-stack-non-helper-callable-inventory-2026-07-08.md`
- `docs/implementation/function-stack-expression-source-range-coverage-2026-07-08.md`
- `docs/implementation/function-stack-closure-effect-row-audit-2026-07-09.md`
- `docs/implementation/function-stack-non-helper-source-function-values-2026-07-09.md`
- `docs/implementation/current-work-status-2026-07-09.md`

Baseline before this status refresh:

- `097f694a5 Apply source callback parameters`

## Implemented And Pushed

### Function Types And Curried Groups

- Formal function types parse and type-check as `A -> B`.
- Function types are right-associative.
- Tuple call-group function types such as `(A, B) -> C` are represented.
- Function-like declarations preserve multiple curried `ParamGroup`s.
- `flow` declarations reject curried parameter groups.
- `f(a)(b)` and `f(a, b)` remain distinct through parser, sema, and runtime
  lowering.
- `samples/function-curried-call-groups` covers tuple-tail and chained group
  shapes such as `f(a, b)(c)` and `f(a)(b)(c, d)`.

### Call And Select Surface

- Parser-level `MethodCall` / field-call splits were replaced by neutral
  `Expr::Select` plus `Expr::Call`.
- Runtime IR still represents the executable operation that lowering selects;
  only the source AST no longer bakes in method-vs-field call meaning.
- Expression operators used by parser decisions are typed `ExprOp` values
  rather than ad hoc operator strings.

### Closures And Runtime Apply

- Closure expressions `|x| expr` and `|| expr` type-check as function values.
- Typed parameters, destructuring/pattern parameters, braced return
  annotations, and closure-local `return` are implemented.
- `RuntimeValue::Function`, `RuntimeExpr::Function`, and `RuntimeExpr::Apply`
  execute in the core evaluator.
- Runtime functions capture deterministic local bindings.
- Exact application, partial application, and curried runtime application are
  implemented for accepted runtime function values.
- A narrow non-helper source-local top-level `fn` family now materializes as
  `RuntimeExpr::Function` without going through the pure-helper table. It
  accepts simple-parameter expression bodies with no host/top-level calls,
  pipes, `await`, `try`, or suspension/effect-capable syntax. Multiple curried
  `ParamGroup`s lower to nested functions, returned simple closure literals
  recursively lower to nested runtime functions, direct calls to function-typed
  parameters lower as local `RuntimeExpr::Apply`, function-valued `let` aliases
  and callback partials are tracked inside the accepted body, and named
  missing-input partial calls synthesize wrapper functions that preserve
  declaration argument order.
- AWBC has a non-suspending closure/apply cut using `MakeFunction` and
  `ApplyFunction`, including partial and chained apply.
- Product AWBC save/load explicitly rejects persisted runtime function values
  instead of pretending closure snapshots are stable.

### Placeholders, Pipes, And Method Fallback

- Pattern `_` and expression `_` are distinct.
- Expression `_` abstraction works in expected-function contexts and the
  implemented inferred binary / known-callable partial-call cases.
- `^` is scoped only to the RHS of `|>`.
- Pipe RHS with `^` substitutes the pipe LHS before checking/lowering.
- Pipe RHS without `^` uses data-last application for the implemented fixed
  argument paths.
- Named RHS calls in no-`^` pipes preserve callable input-name order for pure
  helpers and accepted source-function candidates instead of lowering by
  source argument order.
- Data-last method fallback resolves only after real env/inherent/trait
  methods.
- Implemented data-last fallback records deterministic runtime argument order
  and reports ambiguity rather than selecting by environment merge order.
- Real methods keep priority; shadowed fallback candidates surface as warnings.

### Primitive Names, Enum Shorthand, And Runtime IDs

- Canonical primitive spellings are enforced without compatibility aliases or
  formatter normalization shims.
- `Unit` and `Never` remain the canonical surface spellings.
- Expected-type enum shorthand is verified for user-defined unit variants,
  tuple-payload constructors, and record-payload constructors.
- Runtime-plan lowering now emits `RuntimeExpr::Variant` for those user enum
  short constructors rather than using a `DataFormat`-only special case or
  plain record payload.
- Runtime lookup IDs use typed `RuntimeIdPath` values rather than raw string
  newtypes such as `FlowRuntimeId("flow.main")`.
- AWBC flow targets use typed `FlowRuntimeId` keys in compiler inventory.

### Capture, Effects, LSP, And Source Identity

- Sema records closure capture inventory keyed by stable expression IDs.
- Borrowed local captures crossing checked suspension boundaries are
  diagnosed.
- Closure body effects compose when the function value is invoked for the
  broad set of currently implemented closure, alias, callback, higher-order,
  curried, destructuring, and returned-closure paths.
- Numeric fallback lints exist for inferred closure bodies.
- LSP inlays cover inferred function-valued `let` bindings.
- Opt-in expression type inlays exist for source-backed expression judgments.
- The expression source-range coverage audit records the statement and
  expression families currently carrying authored source identity.
- `TypeCheckStats` records source-backed and source-missing expression judgment
  counts for report-level auditing.
- The closure-effect row audit classifies implemented effect composition paths
  into stable timing behavior, temporary evidence graph wiring, and
  diagnostics-only coverage.
- `EffectAnalysisReport::closed_effect_rows()` now projects closed inferred,
  upper-bound, and forbidden rows for every callable without exposing sema's
  temporary effect graph internals.
- Agent verified-effects manifest lowering consumes the closed row projection
  instead of reading graph summaries directly.

### Explicit Rejection Boundaries

- Spread partial/fallback shapes are intentionally rejected until their runtime
  expansion contract is designed.
- Helper-less signature partials are rejected by checked runtime-plan lowering
  with unsupported callable family `signature_partial_without_helper`.
- Runtime function values in product AWBC save/load are rejected with structured
  unsupported-runtime-value errors.

## Remaining Blocking Work

These are the pieces that keep the active goal open:

| Area | Current state | Blocking document |
| --- | --- | --- |
| Spread partial application and spread data-last fallback | Fixed partial/fallback paths are implemented; spread shapes are rejected with diagnostics. Accepted spread execution semantics are not designed. | `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md` |
| AWBC suspension-aware dynamic apply | Non-suspending `MakeFunction` / `ApplyFunction` works. Apply that suspends or budget-yields has no resumable safe-point contract. | `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md` |
| Persisted closure/function snapshots | Product AWBC save/load rejects function values. Serializable closure state and versioned restore are not designed. | `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md` |
| Non-helper/effectful/suspending callable allocation | Callable families are inventoried. The first non-helper source-local `fn` expansion is implemented for simple expression bodies with no host/effect/suspension syntax, including multiple curried `ParamGroup`s, returned simple closure literals, direct calls to function-typed parameters, and local callback aliases/partials. Effectful/suspending bodies, task/dialogue/stream functions, trait/impl methods, adapter thunks, and persistence remain unaccepted. | `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`; `docs/implementation/function-stack-non-helper-source-function-values-2026-07-09.md` |
| Final closure effect-row model | Current effect composition is broad and useful. The path audit is complete, `no_effect` now has focused closure-invocation coverage, closed row report projection exists, and Agent artifact verified-effects lowering consumes that projection. Source row syntax, open-row inference, row-bearing callable values, and runtime-plan/verifier/LSP consumers beyond artifact proofs are still not finalized. | `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`; `docs/implementation/function-stack-closure-effect-row-audit-2026-07-09.md` |

## Deferred Non-Blocker

Runtime ID atom-table storage is deferred until profiling shows that ID
comparison, hashing, serialization, or allocation pressure justifies carrying
an atom table through runtime-plan and data-format boundaries. The typed path
API is implemented, so atom-table storage is not a current completion blocker.

## Separate Work Not To Mix Into This Goal

The working copy has View/Web/text-input changes that are unrelated to this
function-stack rollup. They include rendering, font, sample, IME/player, and
runtime-driver text-input files. See
`docs/implementation/current-work-status-2026-07-09.md` for the exact list.

Those files should be validated as their own slice. They must not become
evidence for this language-stack goal and should not be staged together with
function-stack documentation.

## Practical Next Steps

For function-stack work:

1. Choose one remaining request boundary and produce a concrete accepted
   contract before implementing behavior that changes the language/runtime
   model.
2. Prefer the smallest executable expansion with clear tests, for example a
   single accepted non-helper callable family, rather than redesigning all
   callable families at once.
3. Keep rejection diagnostics for still-unsupported families explicit and
   structured.

For the separate View/Web/text-input work:

1. Inspect the dirty slice independently.
2. Decide whether to continue, split, or revert any stale parts.
3. Run targeted renderer/player/IME validation.
4. Commit separately from function-stack work.

## Validation For This Rollup

```bash
git status --short --branch
jj status
git log --oneline -8
git diff --check -- docs/implementation
```
