# Function Stack Status Rollup - 2026-07-09

This is the current one-page map for the active function/closure/currying/
pipeline language-stack goal. It supersedes the "where are we now" role of
`docs/implementation/function-stack-current-status-2026-07-08.md` while keeping
that file as the detailed 2026-07-08 status index.
For the current entry point and reading order, see
`docs/implementation/function-stack-current-state-2026-07-09.md`.

Status: **open**. The implemented surface is substantial, but the goal is not
complete because several requested end-to-end behaviors remain behind explicit
request/design boundaries.

## Evidence Trail

Short current gap map:

- `docs/implementation/function-stack-current-state-2026-07-09.md`
- `docs/implementation/function-stack-current-gap-map-2026-07-09.md`

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
- `docs/implementation/function-stack-pipe-control-expression-rhs-2026-07-09.md`
- `docs/implementation/function-stack-spread-rejection-boundary-2026-07-09.md`
- `docs/implementation/function-stack-function-value-fixed-spread-apply-2026-07-09.md`
- `docs/implementation/function-stack-signature-fixed-spread-apply-2026-07-09.md`
- `docs/implementation/function-stack-spread-contract-closure-2026-07-09.md`
- `docs/implementation/function-stack-unsupported-bare-source-function-values-2026-07-09.md`
- `docs/implementation/function-stack-data-last-unsupported-source-partial-2026-07-09.md`
- `docs/implementation/function-stack-data-last-callable-kind-partial-rejection-2026-07-09.md`
- `docs/implementation/function-stack-prefix-source-partial-rejection-2026-07-09.md`
- `docs/implementation/function-stack-non-helper-callable-kind-rejection-2026-07-09.md`
- `docs/implementation/function-stack-method-value-rejection-2026-07-09.md`
- `docs/implementation/function-stack-awbc-control-expression-parity-2026-07-09.md`
- `docs/implementation/function-stack-awbc-expression-apply-suspension-boundary-2026-07-09.md`
- `docs/implementation/function-stack-effect-row-partial-closure-timing-2026-07-09.md`
- `docs/implementation/function-stack-effect-row-curried-higher-order-timing-2026-07-09.md`
- `docs/implementation/current-work-status-2026-07-09.md`

Current pushed baseline:

- the function-stack baseline that rejects unsupported bare source-function
  values and data-last source-function partials without executable runtime
  candidates

Previous named baseline before the spread rejection hardening slice:

- `486738b31 Handle pipe control-expression RHS placeholders`

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
- Destructured closure parameters lower to runtime-only synthetic function
  parameters plus `RuntimeExpr::Match`, and the VM pure backend evaluates that
  pattern body through the shared runtime pattern matcher.
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
  and callback partials are tracked inside the accepted body, closure literals
  bound by those `let` aliases may use destructuring parameter patterns through
  the shared synthetic-argument plus `RuntimeExpr::Match` lowering path, and
  exact calls to already-lowered pure helpers lower as `RuntimeExpr::PureCall`.
  Candidate discovery is fixed-point based, so exact calls to already-accepted
  source-local candidates also lower through materialized runtime function
  values. Named missing-input partial calls synthesize wrapper functions that
  preserve declaration argument order, exact named pure-helper calls preserve
  helper input order, and exact named source-candidate calls preserve source
  declaration input order. Pure value-position `if`, `if let`, and `match`
  expressions in this accepted family retain `RuntimeExpr::If`,
  `RuntimeExpr::IfLet`, and `RuntimeExpr::Match` shapes; `if let` guards see
  the pattern locals they guard.
- AWBC has a non-suspending closure/apply cut using `MakeFunction` and
  `ApplyFunction`, including partial and chained apply.
- AWBC value-position `if`, `if let`, and `match` expressions now lower to
  real branch blocks, pattern scopes, jumps, returns, and pattern-mismatch
  traps instead of eager `select.bool` / `match.value` intrinsics. This makes
  destructured closure parameters and accepted source-local closure aliases
  executable through product AWBC without evaluating unselected branches.
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
- Pipe RHS `^` substitution now descends into value-position `if`, `if let`,
  and `match` expressions, and checked runtime-plan lowering keeps structured
  `RuntimeExpr::IfLet` / `RuntimeExpr::Match` scrutinees after substitution.
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
- Partial closure application is covered as effect-free until the partial value
  is invoked, including a `no_effect` regression for the eventual partial alias
  call.
- Returned closure callback effects are covered as delayed until the returned
  function value is invoked, including a `no_effect` regression for the
  returned function call.
- Curried higher-order callback effects are covered as delayed until execution
  reaches the group that invokes the callback, including a `no_effect`
  regression for the final call of a partial curried callback.
- Captured function values preserve effect rows through local aliases,
  including aliases captured by returned closures.
- Numeric fallback lints exist for inferred closure bodies.
- LSP inlays cover inferred function-valued `let` bindings.
- LSP diagnostics surface current effect traces as related information for
  returned-closure callback edges and directly performed static effects.
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

- Variable-length spread shapes in partial-call construction and data-last
  fallback are intentionally rejected by the current language contract.
  Low-level runtime `Apply` spread expansion is verified for exact,
  partial-prefix, and curried function application, and source function-value
  calls, direct fixed-parameter signature calls, and data-last method fallback
  accept inline fixed-length literal spread.
- Helper-less signature partials are rejected by checked runtime-plan lowering
  with unsupported callable family `signature_partial_without_helper`.
- Data-last pipe partials through helper-less source functions are rejected by
  checked runtime-plan lowering with the same
  `signature_partial_without_helper` family.
- Data-last partials through helper-less task, dialogue, and stream functions
  are rejected by checked runtime-plan lowering with
  `signature_partial_without_helper`.
- Positional prefix partials through helper-less source functions outside the
  accepted source-function candidate set are rejected by checked runtime-plan
  lowering with `signature_partial_without_helper`.
- Bare top-level source-function value references are rejected by checked
  runtime-plan lowering with unsupported callable family
  `source_function_value_without_runtime_candidate` when sema proves the path
  is a function value but no pure helper or accepted source-function candidate
  exists.
- Bare task/dialogue/stream function value references are rejected by checked
  runtime-plan lowering with the same unsupported callable family instead of
  being treated as ordinary locals.
- Value-position environment, inherent, and trait/impl method references are
  rejected by sema with structured `UnsupportedMethodValueReference`
  diagnostics and stable code
  `sema.typecheck.unsupported_method_value_reference`, keeping receiver
  binding explicit until 07.7 defines first-class method values.
- Runtime function values in product AWBC save/load are rejected with structured
  unsupported-runtime-value errors.
- AWBC expression-level `ApplyFunction` reports runtime traps when the applied
  function suspends or exhausts the synchronous expression-apply budget.

## Remaining Blocking Work

These are the pieces that keep the active goal open:

| Area | Current state | Blocking document |
| --- | --- | --- |
| AWBC suspension-aware dynamic apply | Non-suspending `MakeFunction` / `ApplyFunction` works, including lazy AWBC branch lowering for value-position `if` / `if let` / `match` bodies. Apply that suspends or budget-yields now has focused runtime-trap regression coverage, but accepting that behavior still needs a resumable safe-point contract. | `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`; `docs/implementation/function-stack-awbc-control-expression-parity-2026-07-09.md`; `docs/implementation/function-stack-awbc-expression-apply-suspension-boundary-2026-07-09.md` |
| Persisted closure/function snapshots | Product AWBC save/load rejects function values. Serializable closure state and versioned restore are not designed. | `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md` |
| Non-helper/effectful/suspending callable allocation | Callable families are inventoried. The first non-helper source-local `fn` expansion is implemented for expression bodies with no host/effect/suspension syntax, including multiple curried `ParamGroup`s, returned simple closure literals, direct calls to function-typed parameters, local callback aliases/partials, destructuring closure literals in function-valued local bindings, exact calls to already-lowered pure helpers, pure value control expressions, and fixed-point exact calls to already-accepted source-local candidates. Bare task/dialogue/stream function values have focused structured rejection coverage. Data-last task/dialogue/stream partials have focused structured rejection coverage. Value-position environment, inherent, and trait/impl method references have focused structured rejection coverage. Effectful/suspending bodies, host/adapter call-bearing bodies, accepted task/dialogue/stream values, accepted method values, adapter thunks, and persistence remain unaccepted. | `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`; `docs/implementation/function-stack-non-helper-source-function-values-2026-07-09.md`; `docs/implementation/function-stack-non-helper-callable-kind-rejection-2026-07-09.md`; `docs/implementation/function-stack-data-last-callable-kind-partial-rejection-2026-07-09.md`; `docs/implementation/function-stack-method-value-rejection-2026-07-09.md`; `docs/implementation/function-stack-current-gap-map-2026-07-09.md` |
| Final closure effect-row model | Current effect composition is broad and useful. The path audit is complete, `no_effect` now has focused closure-invocation, partial-closure-invocation, returned-closure-invocation, and partial-curried-higher-order invocation coverage, captured function aliases preserve rows through returned closures, borrowed captures crossing `await` preserve closed row evidence while reporting the lifetime diagnostic, current LSP diagnostics surface returned-closure and performed-effect traces as related information, closed row report projection exists, and Agent artifact verified-effects lowering consumes that projection. Source row syntax, open-row inference, row-bearing callable values, and runtime-plan/verifier/LSP consumers beyond artifact proofs are still not finalized. | `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`; `docs/implementation/function-stack-closure-effect-row-audit-2026-07-09.md`; `docs/implementation/function-stack-effect-row-partial-closure-timing-2026-07-09.md`; `docs/implementation/function-stack-effect-row-returned-closure-no-effect-2026-07-09.md`; `docs/implementation/function-stack-lsp-performed-effect-trace-2026-07-09.md`; `docs/implementation/function-stack-effect-row-curried-higher-order-timing-2026-07-09.md` |

## Deferred Non-Blocker

Runtime ID atom-table storage is deferred until profiling shows that ID
comparison, hashing, serialization, or allocation pressure justifies carrying
an atom table through runtime-plan and data-format boundaries. The typed path
API is implemented, so atom-table storage is not a current completion blocker.

## Separate Work Not To Mix Into This Goal

View/Web/text-input work is unrelated to this function-stack rollup. Rendering,
font, sample, IME/player, and runtime-driver text-input changes should be
validated as their own slice whenever active.

Those files must not become evidence for this language-stack goal and should
not be staged together with function-stack documentation.

## Practical Next Steps

For function-stack work:

1. Do not widen accepted language/runtime behavior without first answering the
   matching request boundary.
2. When a contract is available, prefer the smallest executable expansion with
   clear tests, for example a single accepted non-helper callable family,
   rather than redesigning all callable families at once.
3. In the meantime, limit function-stack implementation work to narrow
   hardening, diagnostics, fixtures, or documentation that preserves the
   current accepted/rejected boundary.
4. Keep rejection diagnostics for still-unsupported families explicit and
   structured.

For the separate View/Web/text-input work:

1. Inspect the active slice independently.
2. Decide whether to continue, split, or revert any stale parts.
3. Run targeted renderer/player/IME validation.
4. Commit separately from function-stack work.

## Validation For This Rollup

```bash
git status --short --branch
jj status
git log --oneline -8
git diff --check -- docs/implementation
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_destructured_closure_let -- --nocapture
```
