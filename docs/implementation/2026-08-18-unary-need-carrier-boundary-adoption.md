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
- Syntax/HIR already have `ComputationBlock(Result | Task | Seq | Stream)`.
  `result {}` therefore exists only as a value wrapper, not yet as the final
  checked residual boundary; `option {}` is absent; and the existing `task {}`
  block conflicts with the decision not to expose a task/Need-construction
  block.
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
