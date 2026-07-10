# Current Work Status - 2026-07-09

This note is the current repository map after the latest pushed function-stack
slice and the current status cleanup. It supersedes the operational pointers in
`docs/implementation/current-work-status-2026-07-08.md` without rewriting that
historical note. For the current active-goal entry point, see
`docs/implementation/function-stack-current-state-2026-07-09.md`.

## Repository Baseline

- Current pushed function-stack baseline:
  the function-stack baseline that includes captured function alias row
  preservation through returned closures.
- At the start of this cleanup, `main` and `origin/main` were aligned at that
  head.
- The worktree was clean at this audit point. Future unrelated View/Web/text
  input work should still be validated and committed as its own slice rather
  than mixed into function-stack language changes.

## Active Goal Status

The active function/closure/currying/pipeline language-stack goal remains
open. The implemented surface is broad: the narrow 07.7 exact pure-helper call
gap inside accepted source-local function bodies is implemented, the pipe RHS
hardening slice carries `^` substitution through value-position `if`,
`if let`, and `match` expressions, the spread rejection boundary has precise
structured diagnostics for spread-before-fixed and multiple-spread
partial/fallback shapes, and current 07.8 evidence covers captured function
alias rows plus borrowed-capture row preservation at an `await` boundary. The
low-level runtime `Apply` substrate also now has spread expansion regression
coverage for exact, partial-prefix, and curried function application, and
source function-value calls, direct fixed-parameter signature calls, and
data-last method fallback accept inline fixed-length literal spread.
Remaining completion still depends on explicit request/design areas:

1. Variable-length spread semantics.
2. General non-helper/effectful/suspending callable allocation and the final
   closure effect-row contract.

The current status entry points are:

- `docs/implementation/function-stack-current-state-2026-07-09.md`
- `docs/implementation/function-stack-current-gap-map-2026-07-09.md`
- `docs/implementation/function-stack-status-rollup-2026-07-09.md`

The detailed evidence trail remains:

- `docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`
- `docs/implementation/function-stack-current-status-2026-07-08.md`
- `docs/implementation/function-stack-goal-completion-audit-2026-07-08.md`
- `docs/implementation/function-stack-non-helper-callable-inventory-2026-07-08.md`
- `docs/implementation/function-stack-request-split-audit-2026-07-08.md`
- `docs/implementation/function-stack-expression-source-range-coverage-2026-07-08.md`
- `docs/implementation/function-stack-closure-effect-row-audit-2026-07-09.md`
- `docs/implementation/function-stack-non-helper-source-function-values-2026-07-09.md`
- `docs/implementation/function-stack-awbc-control-expression-parity-2026-07-09.md`

## Completed And Pushed Function-Stack Slices

- Call/select unification: parser surface uses neutral `Expr::Select` plus
  `Expr::Call`; runtime semantics stay in the lowered executable operation.
- Function types and curried call groups: `A -> B`, right associativity, tuple
  call groups, multiple curried `ParamGroup`s for function-like declarations,
  and rejection of curried `flow` parameters.
- Closure syntax and runtime apply: expression closures, typed/pattern
  parameters, braced closure return annotations, closure-local `return`,
  captured runtime functions, destructured closure parameters lowered through
  runtime pattern matches, exact apply, partial apply, and curried apply.
- First AWBC closure/apply cut: non-suspending generated closures lower through
  `MakeFunction` and `ApplyFunction`; snapshot persistence rejects runtime
  function values explicitly.
- AWBC control-expression parity: value-position `if`, `if let`, and `match`
  inside generated runtime functions now lower to lazy AWBC branch blocks,
  pattern scopes, jumps, returns, and pattern-mismatch traps instead of eager
  selector intrinsics.
- Placeholder and pipe behavior: expression `_` is distinct from pattern `_`;
  `^` is pipe-RHS scoped; no-`^` pipes use data-last application for the
  implemented fixed-argument paths. Named RHS calls in those pipes now preserve
  callable input-name order for pure helpers and accepted source-function
  candidates.
- Pipe RHS control-expression hardening: `^` substitution now descends into
  value-position `if`, `if let`, and `match` expressions, and checked
  runtime-plan lowering preserves structured branch/scrutinee expressions
  after substitution.
- Method-chain fallback: inherent/trait/env methods win before data-last
  callable fallback; implemented fallback cases carry deterministic argument
  ordering and ambiguity diagnostics.
- Canonical primitive spellings: accepted primitive names are canonical, and
  non-canonical aliases are rejected rather than normalized.
- Expected-type enum shorthand: user-defined unit, tuple-payload, and
  record-payload short constructors are covered in sema and runtime-plan
  lowering.
- Runtime ID boundary cleanup: runtime lookup IDs use typed `RuntimeIdPath`
  values and AWBC flow targets use typed `FlowRuntimeId` keys instead of raw
  public-label maps.
- Source identity and tooling evidence: source ranges and type inlay evidence
  cover the audited expression/statement families, with `TypeCheckStats`
  reporting source-backed and source-missing counts.
- Non-helper callable inventory: accepted, rejected, adapter-facing, and
  design-blocked callable families are classified; unsupported helper-less
  signature partials now report the explicit family marker
  `signature_partial_without_helper`.
- First non-helper source function value cut: source-local `fn` declarations
  with simple identifier parameters and expression bodies that contain no
  host/effect/suspension-capable syntax now materialize as
  `RuntimeExpr::Function` values, including named missing-input wrapper
  partials, multiple curried `ParamGroup`s lowered to nested functions, and
  returned simple closure literals lowered to nested runtime functions. Direct
  calls to function-typed parameters lower as local `RuntimeExpr::Apply`, and
  function-valued `let` aliases/partials are tracked inside that accepted body.
- Destructuring closure locals inside the accepted source-function subset:
  closure literals assigned to function-valued `let` aliases may use
  destructuring parameter patterns. Runtime-plan lowering uses the same
  synthetic closure argument plus `RuntimeExpr::Match` body shape as ordinary
  destructured closures, so the public closure pattern surface does not leak
  into runtime function parameter names.
- Exact pure-helper calls inside the accepted source-function subset:
  source-local runtime function candidates receive the existing pure-helper
  lookup, exact helper calls are accepted, and named helper arguments lower in
  helper input order.
- Exact source-local candidate calls inside the accepted source-function
  subset: runtime-plan candidate discovery now runs to a deterministic fixed
  point, so a later accepted source-local `fn` may call an already-accepted
  source-local candidate and named arguments lower in source declaration order.
- Function-value fixed spread apply: function-value calls accept inline
  fixed-length bracket sequence literal spread, including compact numeric
  bracket sequences. Runtime-plan lowering preserves `RuntimeExpr::SpreadArg`
  so the verified runtime apply spread substrate performs argument expansion.
- Signature fixed spread apply: direct fixed-parameter signature calls accept
  inline fixed-length bracket sequence literal spread for exact and
  missing-input partial calls. Runtime-plan lowering preserves
  `RuntimeExpr::SpreadArg` and uses either source-function `Apply` or pure-call
  evaluation depending on the selected executable callable.

## Remaining Function-Stack Work

The remaining items are not "forgotten implementation tasks"; they are
documented request/design boundaries that must be answered before final
implementation:

- Spread data-last fallback and variable-length spread:
  `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`
- Non-helper/effectful/suspending callable allocation:
  `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`
  The accepted source-local `fn` subset now covers curried groups, returned
  simple closures, direct function-typed parameter calls, callback aliases and
  partials, destructuring closure literals in local function bindings, and
  exact calls to already-lowered pure helpers and already-accepted
  source-local candidates. It still excludes effectful/suspending
  host/adapter call-bearing bodies and non-`fn` callable families.
- Closure effect-row final contract:
  `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`

Runtime ID atom-table storage is deliberately deferred until profiling shows
ID comparison, hashing, serialization, or allocation pressure. The typed path
API is in place; the atom table is not a current blocker by itself.

## Separate Open Tracks

The following are real open tracks, but they must be planned and validated
separately from the function-stack goal:

- Native/Web View rendering parity, radius/shadow/filter behavior, and modern
  feedback View visuals.
- Text-control editing, selection, IME handling, and focus-loss behavior.
- Web player/EditContext glue and generated `.awfb` samples.
- Scoped presentation handle save/load and rollback follow-ups.
- Parser file/module naming cleanup.
- Pinned exact visual PNG baseline promotion and Web exact readback.

## Recommended Next Order

1. Keep function-stack commits separate from any View/Web worktree slice.
2. For the function-stack goal, either receive/author a concrete design answer
   for one of the four request boundaries, or audit code for another narrow
   typed-key/diagnostic gap that is implementation-ready without changing the
   language contract.
3. Treat View/Web/text-input work as its own validation slice when active:
   inspect, decide whether to continue or revert, run targeted renderer/player
   checks, then commit separately.

## Validation For This Note

```bash
git status --short --branch
jj status
git log --oneline -8
git diff --check -- docs/implementation
```
