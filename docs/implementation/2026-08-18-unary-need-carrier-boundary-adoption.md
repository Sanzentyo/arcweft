# Unary Need and carrier-boundary adoption — 2026-08-18

Supersedes the Try/Await and Need assumptions recorded in
`2026-08-17-converged-language-content-surface-adoption.md`.

Inspected revision: `f7832bb7f620866bce09e9eb94905e66a736ded2`

Working tree at inspection: clean before this documentation cut.

## Performed

- Replaced the stable public model `Need<T, E>` with unary `Need<T>`.
- Made Result/Option the sole owners of domain failure and absence.
- Defined Await as `Need<T> -> T`, with cancellation as non-returning control
  transfer rather than a Result error.
- Adopted local `result {}` and `option {}` carrier boundaries, no-flatten tail
  wrapping, lexical boundary-stack lookup, and exact `_`/`^` behavior.
- Removed Await-specific error/denied semantics from the stable target and
  classified timeout as an unresolved temporal race/policy contract.
- Retained `Stream<T, E>` because its error is a terminal multi-item protocol,
  not a one-shot payload carrier.

## Implementation checkpoint: carrier-block surface

- Replaced the unreleased `task {}` computation-block family directly with
  `option {}` across parser projection, attached syntax, final HIR, source
  validation, dialogue candidates, and semantic analysis.
- `option { tail }` now has the checked type `Option<T>` when `tail: T`; it does
  not construct a Need and it does not flatten an already-carried tail.
- Retained the existing computation-block owner for this atomic surface cut.
  Publishing the lexical propagation boundary and consuming one checked Try
  fact is the next cut; this checkpoint does not claim that `result {}` or
  `option {}` already intercept nested Try residuals.

### Checkpoint validation

Passed:

- `cargo test -p arcweft-lang-syntax --lib`
  (`668 passed`).
- `cargo test -p arcweft-lang-hir --lib` initially reached `829 passed`, one
  renamed dialogue-candidate fixture failure, and `8 ignored`; the exact failed
  test passed after its source fixture was corrected.
- `cargo test -p arcweft-lang-sema --lib` (`197 passed` before adding the new
  focused test), plus
  `option_block_wraps_its_tail_without_constructing_a_need`.
- `cargo check --workspace --all-targets --all-features`.
- `cargo clippy --workspace --all-targets --all-features`; it completed with
  pre-existing warnings outside this cut.
- `just structure-audit` (`0` blocking violations).
- `cargo fmt --all` and `git diff --check`.

Failed outside this cut:

- `just test-workspace` reached `arcweft-agent-runner` and failed five existing
  controller AWBC response-field tests because runtime records lacked `uri`,
  `body`, `semantic_hash`, `edge_count`, or `tick`. This cut changes no runner,
  runtime payload, Agent-field, or AWBC field-projection source.

## Current source evidence

The inspected production source has not implemented this public switch:

- `arcweft-lang-sema::TypeKind::Need` and runtime type projections still store
  separate `ready` and `error` types.
- Await checking still builds a synthetic `physical_result` and accepts
  `HirAwaitBranchKind::Error` / `Denied`.
- Runtime-plan Await lowering still owns `TaskOutcomeContract`, binds a Result
  temporary, and dispatches authored Error handlers.
- `RuntimeNeedState` and task completion still carry the old split completion
  contract.
- Syntax/HIR now have `ComputationBlock(Result | Option | Seq | Stream)`.
  `result {}` and `option {}` currently wrap their value tails, but are not yet
  the final checked residual boundaries.
- `_` and `^` remain typed syntax/HIR placeholders, but the checked lexical
  boundary stack and pipe-left binding required by the final contract are not
  yet published.

## Passed

- `git diff --check`.
- Relative `.md` link targets in every changed Markdown file, checked with a
  scoped PowerShell link-target scan.

## Not run

- Cargo checks and tests. No Rust source changed.
- Documentation rendering and Arcweft example compilation.

## Required implementation transaction

- Replace the type owner, environment projections, type digests, construction
  seeds, and checked type tables with unary Need. Do not retain a binary alias.
- Project producer failures into synchronous admission Result, Ready Result
  payload, cancellation, or runtime fault through one typed registration
  contract.
- Replace Await Error/Denied branches and Result materialization across sema,
  runtime-plan, native runtime, AWBC, save/replay, View, scheduler, adapters,
  and fixtures.
- Evolve the existing computation-block owner in place to carry Result/Option
  boundaries; delete the Task success path and avoid fused block variants.
- Publish one checked Try fact that resolves the lexical boundary stack and is
  lowered once. Await, pipe, and partial abstraction consume that fact.
- Specify timeout before retaining any timeout Await branch.
- Treat `const {}` as a separate phase-boundary contract; do not infer its full
  implementation from this Need migration.

## Explicit non-goals

- No unary Stream migration.
- No `need {}` or replacement task-construction block.
- No generic special Try block or user-defined Try trait.
- No compatibility reader, version bump, or parallel Need representation.
