# Function Stack Current Gap Map - 2026-07-09

This note is the short current-state map for the active
function/closure/currying/pipeline goal. It is intentionally smaller than the
main implementation log and the status rollup: use it to decide what is done,
what remains implementation-ready, and what is still blocked on design.
For the current entry point and reading order, see
`docs/implementation/function-stack-current-state-2026-07-09.md`.

## Repository Baseline

- Current pushed function-stack baseline:
  the status refresh commit titled `Harden spread rejection diagnostics`.
- `main` and `origin/main` were aligned at that commit when this gap map was
  refreshed.
- The previous baseline before the spread rejection hardening slice was
  `486738b31 Handle pipe control-expression RHS placeholders`.
- The working copy still contains View/Web/text-input changes. They are not
  function-stack evidence and should be validated as a separate slice before
  being staged or pushed.

## Done And Pushed

The active goal has substantial implementation behind it. The pushed baseline
includes:

- formal function types, right-associative `A -> B`, tuple call groups, and
  preserved curried `ParamGroup`s for function-like declarations;
- rejection of curried `flow` parameters;
- parser-level call/select unification through neutral `Expr::Select` plus
  `Expr::Call`;
- expression closures, typed and destructuring parameters, closure-local
  `return`, capture inventory, and borrowed-capture diagnostics at checked
  suspension boundaries;
- runtime `Function` / `Apply`, exact application, partial application, and
  curried application for accepted non-suspending paths;
- AWBC `MakeFunction` / `ApplyFunction` for non-suspending generated runtime
  functions;
- lazy AWBC lowering for value-position `if`, `if let`, and `match` inside
  generated function bodies;
- expression `_` placeholder abstraction for the implemented expected-function
  and known-callable shapes;
- pipe `^` substitution and no-`^` data-last application for implemented fixed
  argument paths, including value-position `if`, `if let`, and `match`
  expressions in the pipe RHS;
- method-chain fallback after inherent/trait/env method lookup, with
  deterministic argument order and ambiguity diagnostics;
- canonical primitive spellings without compatibility aliases;
- user enum shorthand lowering through the expected-type path;
- typed runtime ID paths instead of raw public-label string newtypes;
- first source-local non-helper `fn` runtime-function materialization for the
  accepted simple-expression subset, including curried groups, returned simple
  closures, direct calls to function-typed parameters, local callback
  aliases/partials, destructuring closure literals in local function-valued
  bindings, and pure value-position `if` / `if let` / `match` expressions;
- exact calls to already-lowered pure helpers from inside that accepted
  source-local function subset, with named helper arguments lowered in helper
  input order;
- fixed-point exact calls to already-accepted source-local function candidates
  from inside that accepted subset, with named arguments lowered in source
  declaration order;
- product AWBC save/load rejection of escaped function values through the
  structured unsupported-runtime-value path.

The detailed evidence remains in:

- `docs/implementation/function-stack-status-rollup-2026-07-09.md`
- `docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`
- `docs/implementation/function-stack-pipe-control-expression-rhs-2026-07-09.md`
- `docs/implementation/function-stack-spread-rejection-boundary-2026-07-09.md`
- `docs/implementation/function-stack-awbc-control-expression-parity-2026-07-09.md`

## Design-Blocked Remaining Work

These keep the active goal open:

| Area | Why it remains open | Request |
| --- | --- | --- |
| Spread partial application and spread data-last fallback | Fixed partial/fallback paths are implemented; spread expansion semantics are not yet designed. | `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md` |
| AWBC suspension-aware dynamic apply | Non-suspending `ApplyFunction` works. Suspending or budget-yielding dynamic apply still needs explicit resume-point semantics. | `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md` |
| Persisted closure/function snapshots | Product AWBC save/load rejects runtime functions. Serializable closure state and versioned restore are not designed. | `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md` |
| Broad non-helper callable allocation | The first source-local `fn` family is implemented, including exact calls to already-lowered pure helpers, pure value control expressions, and exact calls to already-accepted source-local candidates. Task/dialogue/stream functions, trait/impl methods, adapter thunks, host/adapter call-bearing bodies, effectful bodies, and suspending bodies need a stable identity/effect/suspension/persistence contract. | `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md` |
| Final closure effect-row model | Current composition is useful and broadly covered, but source row syntax, open-row inference, row-bearing callable values, and final verifier/LSP/runtime consumers are not finalized. | `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md` |

Runtime ID atom-table storage is deferred until profiling shows it is needed.
The typed path API is already in place, so atom storage is not a current
completion blocker.

## Separate Dirty Track

The dirty View/Web/text-input files are real work, but they are a separate
track:

- native/web View rendering parity;
- radius, shadow, filter, depth, and translucent modern feedback View visuals;
- text-control editing, selection, IME, and focus-loss behavior;
- web player/EditContext glue and generated `.awfb` sample artifacts.

Those changes need their own inspect/validate/commit slice. They should not be
used as evidence for the function-stack goal.

## Recommended Order

1. Keep function-stack commits separate from the dirty View/Web worktree.
2. Treat new function-stack language/runtime behavior as blocked until one of
   the request boundaries has a concrete accepted contract.
3. Use only narrow hardening or diagnostic/test fixes when no new contract is
   available.
4. Audit and commit the View/Web/text-input dirty slice separately.

## Validation For This Map

```bash
git status --short --branch
jj status
git log --oneline -8
git diff --check -- docs/implementation
```
